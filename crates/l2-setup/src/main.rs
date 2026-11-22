//! `l2-setup`: see the crate doc. Asks on the terminal; every question has a flag, so the setup
//! can also run unattended (`--yes` takes the defaults). The database password comes from
//! `L2_DB_PASSWORD`, the prompt, or the folder's own `Database.ini`, and is written only into
//! the two `Database.ini` files the servers read (never logged).

use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, bail};
use clap::{Parser, ValueEnum};
use l2_db_installer::installer::{
    self, ErrorChoice, ExistsChoice, Level, Params, Schema as Side, Script, Ui, connect_options, load_scripts,
};
use l2_setup::dbini::{DbTarget, read_database_ini, write_database_ini};
use l2_setup::layout::{self, assemble, missing_programs};
use l2_setup::migrate;
use sqlx::mysql::MySqlConnection;
use sqlx::{ConnectOptions, Connection};

#[derive(Clone, Copy, PartialEq, Eq, ValueEnum)]
enum DbMode {
    /// Create a new database with the schema.
    New,
    /// Bring an existing L2JMobius H5 database up to the schema, keeping its data.
    Migrate,
    /// Leave the database alone (set it up later).
    Skip,
}

#[derive(Parser)]
#[command(
    version,
    about = "Builds a ready-to-run L2 Rust server folder and sets up its database"
)]
struct Cli {
    /// The repository (default: found from this program's place, or the current folder).
    #[arg(long, value_name = "DIR")]
    repo: Option<PathBuf>,
    /// The server folder to create or update (default: <repo>/release/L2RustServer).
    #[arg(long, value_name = "DIR")]
    out: Option<PathBuf>,
    /// A folder with the geodata (*.l2j files); none: the server runs without geodata.
    #[arg(long, value_name = "DIR")]
    geodata: Option<PathBuf>,
    /// What to do with the database.
    #[arg(long, value_enum)]
    db: Option<DbMode>,
    #[arg(long, value_name = "HOST")]
    db_host: Option<String>,
    #[arg(long, value_name = "PORT")]
    db_port: Option<u16>,
    #[arg(long, value_name = "USER")]
    db_user: Option<String>,
    /// The game database name.
    #[arg(long, value_name = "NAME")]
    db_name: Option<String>,
    /// The login database name, when the accounts are in their own database (default: the game
    /// database, as in L2JMobius).
    #[arg(long, value_name = "NAME")]
    login_db: Option<String>,
    /// The address players connect to (written into game/config/ipconfig.xml); none: this PC only.
    #[arg(long, value_name = "ADDRESS")]
    address: Option<String>,
    /// Migration: the old (Java) game server folder; its config/hexid.txt keeps the server's id,
    /// its config/ipconfig.xml the network setup.
    #[arg(long, value_name = "DIR")]
    java_server: Option<PathBuf>,
    /// Take the default for every question (the password from L2_DB_PASSWORD).
    #[arg(long)]
    yes: bool,
}

/// The questions on the terminal (or their defaults with `--yes`).
struct Prompt {
    yes: bool,
}

impl Prompt {
    fn ask(&self, question: &str, default: &str) -> String {
        if self.yes {
            return default.to_owned();
        }
        if default.is_empty() {
            print!("{question}: ");
        } else {
            print!("{question} [{default}]: ");
        }
        self.read(default)
    }

    /// Like [`Prompt::ask`], without showing the default (a password).
    fn ask_secret(&self, question: &str, default: &str) -> String {
        if self.yes {
            return default.to_owned();
        }
        print!("{question}: ");
        self.read(default)
    }

    fn read(&self, default: &str) -> String {
        let _ = io::stdout().flush();
        let mut line = String::new();
        if io::stdin().lock().read_line(&mut line).unwrap_or(0) == 0 {
            return default.to_owned();
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line.trim().is_empty() {
            return default.to_owned();
        }
        // A path dragged into the window or copied "as path" comes in quotes.
        let line = line.trim();
        line.strip_prefix('"')
            .and_then(|l| l.strip_suffix('"'))
            .unwrap_or(line)
            .to_owned()
    }

    /// Yes or no; `--yes` takes `default`.
    fn confirm(&self, question: &str, default: bool) -> bool {
        let answer = self.ask(&format!("{question} (y/n)"), if default { "y" } else { "n" });
        matches!(answer.to_ascii_lowercase().as_str(), "y" | "yes" | "j" | "ja")
    }

    /// A dangerous step: only the typed word confirms it (never `--yes`).
    fn confirm_word(&self, question: &str, word: &str) -> bool {
        if self.yes {
            return false;
        }
        self.ask(&format!("{question} Type {word} to confirm"), "") == word
    }
}

fn find_repo(cli: Option<PathBuf>) -> anyhow::Result<PathBuf> {
    let is_repo = |p: &Path| p.join("dist/game/data").is_dir() && p.join("Cargo.toml").is_file();
    if let Some(repo) = cli {
        return if is_repo(&repo) {
            Ok(repo)
        } else {
            bail!(
                "{} is not the l2-rust repository (no dist/ and Cargo.toml)",
                repo.display()
            )
        };
    }
    // `target/release/l2-setup`: the repository is two folders up.
    let from_exe = std::env::current_exe()
        .ok()
        .and_then(|e| e.parent()?.parent()?.parent().map(Path::to_path_buf));
    for candidate in from_exe.into_iter().chain(std::env::current_dir().ok()) {
        if is_repo(&candidate) {
            return Ok(candidate);
        }
    }
    bail!("the repository was not found: run l2-setup from it, or give --repo")
}

/// A database name goes between backticks into the SQL.
fn check_db_name(name: &str) -> anyhow::Result<()> {
    if name.trim().is_empty() || name.contains('`') || name.contains('/') || name.contains('\\') {
        bail!("'{name}' is not a usable database name");
    }
    Ok(())
}

#[tokio::main(flavor = "current_thread")]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::new("warn"))
        .init();
    let cli = Cli::parse();
    let prompt = Prompt { yes: cli.yes };
    let repo = find_repo(cli.repo.clone())?;
    let release = repo.join("target/release");
    let missing = missing_programs(&release);
    if !missing.is_empty() {
        bail!(
            "missing programs in {}: {} (setup.cmd / setup.sh builds them: cargo build --release {})",
            release.display(),
            missing.join(", "),
            layout::PROGRAMS
                .iter()
                .map(|p| format!("-p {p}"))
                .collect::<Vec<_>>()
                .join(" ")
        );
    }

    println!("== L2 Rust server setup ==\n");
    let out = match &cli.out {
        Some(out) => out.clone(),
        None => PathBuf::from(prompt.ask(
            "Server folder (created, or updated with your settings kept)",
            &repo.join("release").join("L2RustServer").display().to_string(),
        )),
    };
    let geodata = match &cli.geodata {
        Some(dir) => Some(dir.clone()),
        None => Some(prompt.ask(
            "Geodata folder with the *.l2j files (empty: none, movement and line of sight without it)",
            "",
        ))
        .filter(|d| !d.is_empty())
        .map(PathBuf::from),
    };
    if let Some(dir) = &geodata
        && !dir.is_dir()
    {
        bail!("the geodata folder {} does not exist", dir.display());
    }

    println!("\nBuilding {} ...", out.display());
    let report = assemble(&release, &repo.join("dist"), &out, geodata.as_deref())
        .with_context(|| format!("building {}", out.display()))?;
    println!(
        "{} {}: {} settings files kept, {} old datapack files removed, {} geodata files copied ({} in the folder).",
        if report.created { "Created" } else { "Updated" },
        out.display(),
        report.kept.len(),
        report.removed.len(),
        report.geodata_copied,
        report.geodata_total
    );
    if report.geodata_total == 0 {
        println!("No geodata: monsters walk through walls until you add *.l2j files to game/data/geodata.");
    }

    let mode = match cli.db {
        Some(mode) => mode,
        None => {
            println!("\nDatabase (MariaDB or MySQL):");
            println!("  1  new database (a fresh server)");
            println!("  2  existing L2JMobius H5 database (migration: your accounts and characters stay)");
            println!("  3  skip (set it up later)");
            match prompt.ask("Choose", "1").as_str() {
                "2" => DbMode::Migrate,
                "3" => DbMode::Skip,
                _ => DbMode::New,
            }
        }
    };
    let mut db_ready = true;
    match mode {
        DbMode::Skip => db_ready = false,
        DbMode::New | DbMode::Migrate => {
            let (login, game) = ask_targets(&cli, &prompt, &out)?;
            // Nothing is written before the server answers.
            let conn = connect(&game).await?;
            let _ = conn.close().await;
            if mode == DbMode::New {
                new_database(&out, &login, &game, &prompt).await?;
                write_targets(&out, &login, &game)?;
            } else {
                migrate_database(&out, &login, &game, &prompt).await?;
                java_server_files(&out, &prompt, cli.java_server.as_deref())?;
            }
        }
    }
    network_address(&out, &prompt, cli.address.as_deref())?;

    let start = if cfg!(windows) {
        "Start-AdminGUI.cmd (or Start-Servers.cmd without the GUI)"
    } else {
        "start-servers.sh (or start-admin-gui.sh on a desktop)"
    };
    if !db_ready {
        let install = if cfg!(windows) {
            "Install-Database.cmd"
        } else {
            "install-database.sh"
        };
        println!(
            "\nThe database is not set up yet: run the setup again and choose 1 or 2, or use {install} and set the connection in login/ and game/config/Database.ini."
        );
    }
    println!(
        "\nDone. Start the server with {start} in {}.\nThe client connects to this machine, port 2106 (open ports 2106 and 7777 for players from outside).",
        out.display()
    );
    Ok(())
}

/// The login and the game database: the defaults are what the folder's `Database.ini` files
/// hold now.
fn ask_targets(cli: &Cli, prompt: &Prompt, out: &Path) -> anyhow::Result<(DbTarget, DbTarget)> {
    let current_game = read_database_ini(&out.join("game/config/Database.ini"));
    let current_login = read_database_ini(&out.join("login/config/Database.ini"));
    let current = current_game.unwrap_or(DbTarget {
        host: "localhost".to_owned(),
        port: 3306,
        user: "root".to_owned(),
        password: String::new(),
        database: "l2jmobiush5".to_owned(),
    });
    let host = cli
        .db_host
        .clone()
        .unwrap_or_else(|| prompt.ask("Database host", &current.host));
    let port = match cli.db_port {
        Some(port) => port,
        None => prompt
            .ask("Database port", &current.port.to_string())
            .parse()
            .context("the port is not a number")?,
    };
    let user = cli
        .db_user
        .clone()
        .unwrap_or_else(|| prompt.ask("Database user", &current.user));
    let password = match std::env::var("L2_DB_PASSWORD") {
        Ok(password) => password,
        Err(_) => prompt.ask_secret(
            "Database password (shown while typing; Enter keeps the current one; or set L2_DB_PASSWORD)",
            &current.password,
        ),
    };
    let game_name = cli
        .db_name
        .clone()
        .unwrap_or_else(|| prompt.ask("Game database name", &current.database));
    let login_default = current_login
        .map(|l| l.database)
        .filter(|l| !l.is_empty())
        .unwrap_or_else(|| game_name.clone());
    let login_name = cli.login_db.clone().unwrap_or_else(|| {
        prompt.ask(
            "Login database name (the accounts; usually the same)",
            &login_default,
        )
    });
    check_db_name(&game_name)?;
    check_db_name(&login_name)?;
    let game = DbTarget {
        host,
        port,
        user,
        password,
        database: game_name,
    };
    let login = DbTarget {
        database: login_name,
        ..game.clone()
    };
    Ok((login, game))
}

fn write_targets(out: &Path, login: &DbTarget, game: &DbTarget) -> anyhow::Result<()> {
    write_database_ini(&out.join("login/config/Database.ini"), login)
        .context("writing login/config/Database.ini")?;
    write_database_ini(&out.join("game/config/Database.ini"), game)
        .context("writing game/config/Database.ini")?;
    println!("Database settings written to login/ and game/config/Database.ini.");
    Ok(())
}

/// The installer's console front end, with the answer to "the database exists" decided before.
struct InstallUi {
    exists: ExistsChoice,
    yes: bool,
}

impl Ui for InstallUi {
    fn log(&mut self, level: Level, message: &str) {
        match level {
            Level::Error => eprintln!("[ERROR] {message}"),
            _ => println!("{message}"),
        }
    }

    fn progress(&mut self, _done: usize, _total: usize) {}

    fn ask_exists(&mut self, _db_name: &str) -> ExistsChoice {
        self.exists
    }

    fn ask_error(&mut self, _sql: &str, _error: &str) -> ErrorChoice {
        if !self.yes && (Prompt { yes: false }).confirm("Continue with the next statement?", false) {
            ErrorChoice::Continue
        } else {
            ErrorChoice::Abort
        }
    }
}

async fn connect(target: &DbTarget) -> anyhow::Result<MySqlConnection> {
    match connect_options(&target.host, target.port, &target.user, &target.password)
        .connect()
        .await
    {
        Ok(conn) => Ok(conn),
        Err(e) => {
            let mut text = format!(
                "cannot connect to the database server {}:{} as {}: {e}",
                target.host, target.port, target.user
            );
            if text.contains("Access denied") && !cfg!(windows) && target.user == "root" {
                text.push_str(
                    "\nOn Debian/Ubuntu, root logs in only through the system's socket. Create a user for the server:\n  \
                     sudo mariadb -e \"CREATE USER 'l2'@'localhost' IDENTIFIED BY 'a-password'; GRANT ALL PRIVILEGES ON *.* TO 'l2'@'localhost';\"\n\
                     and run the setup again with that user.",
                );
            }
            bail!("{text}")
        }
    }
}

async fn database_exists(conn: &mut MySqlConnection, name: &str) -> anyhow::Result<bool> {
    let n: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = ?")
        .bind(name)
        .fetch_one(conn)
        .await?;
    Ok(n > 0)
}

/// A new database (two when login and game have their own): the installer's scripts. An
/// existing one is only deleted after the owner typed RESET; migration is the way for a database
/// in use.
async fn new_database(out: &Path, login: &DbTarget, game: &DbTarget, prompt: &Prompt) -> anyhow::Result<()> {
    let jobs: Vec<(&DbTarget, bool, bool)> = if login.database == game.database {
        vec![(game, true, true)]
    } else {
        vec![(login, true, false), (game, false, true)]
    };
    for (target, with_login, with_game) in jobs {
        let mut conn = connect(target).await?;
        let exists = database_exists(&mut conn, &target.database).await?;
        let _ = conn.close().await;
        let exists_choice = if exists {
            println!(
                "\nThe database '{}' exists already. Choose \"existing database\" (2) to keep its data.",
                target.database
            );
            if !prompt.confirm_word("Delete it and everything in it, and install a new one?", "RESET") {
                bail!("nothing was changed in '{}'", target.database);
            }
            ExistsChoice::Reset
        } else {
            ExistsChoice::Cancel
        };
        let params = Params {
            options: connect_options(&target.host, target.port, &target.user, &target.password),
            db_name: target.database.clone(),
            sql_dir: out.join("db_installer/sql"),
            login: with_login,
            game: with_game,
        };
        let mut ui = InstallUi {
            exists: exists_choice,
            yes: prompt.yes,
        };
        let summary = installer::run(&params, &mut ui).await?;
        println!(
            "Database '{}' installed: {} files, {} statements ({} failed).",
            target.database, summary.files, summary.statements_ok, summary.statements_failed
        );
        if summary.statements_failed > 0 {
            bail!(
                "{} statements failed in '{}': the database is incomplete (run the setup again to retry)",
                summary.statements_failed,
                target.database
            );
        }
    }
    Ok(())
}

/// One database of a migration: what it gets and what it must have.
struct Part<'a> {
    target: &'a DbTarget,
    scripts: Vec<Script>,
    /// The login tables must be there (this is the accounts' database).
    needs_login: bool,
    /// The game tables must be there.
    needs_game: bool,
}

/// An existing L2JMobius database (or two): checks, plan, backup, apply on a yes. Any failure
/// stops with the name of the backup.
async fn migrate_database(
    out: &Path,
    login: &DbTarget,
    game: &DbTarget,
    prompt: &Prompt,
) -> anyhow::Result<()> {
    let sql = out.join("db_installer/sql");
    let login_scripts = load_scripts(&sql, Side::Login)?;
    let game_scripts = load_scripts(&sql, Side::Game)?;
    let parts: Vec<Part<'_>> = if login.database == game.database {
        vec![Part {
            target: game,
            scripts: login_scripts.into_iter().chain(game_scripts).collect(),
            needs_login: true,
            needs_game: true,
        }]
    } else {
        vec![
            Part {
                target: login,
                scripts: login_scripts,
                needs_login: true,
                needs_game: false,
            },
            Part {
                target: game,
                scripts: game_scripts,
                needs_login: false,
                needs_game: true,
            },
        ]
    };

    let mut plans = Vec::new();
    for part in &parts {
        let target = part.target;
        let mut conn = connect(target).await?;
        if !database_exists(&mut conn, &target.database).await? {
            let _ = conn.close().await;
            bail!(
                "there is no database '{}' on {}:{}: check the name, or choose a new database (1)",
                target.database,
                target.host,
                target.port
            );
        }
        let existing = migrate::existing_schema(&mut conn, &target.database).await?;
        // The accounts must be where the login server will look: an empty accounts table next to
        // old characters would let anybody create an old account name anew and take them over.
        if part.needs_login && !migrate::has_login_tables(&existing) {
            let _ = conn.close().await;
            bail!(
                "'{}' has no accounts table. If your accounts are in another database, run the setup again and give it as the login database (--login-db)",
                target.database
            );
        }
        if part.needs_game && !migrate::has_game_tables(&existing) {
            let _ = conn.close().await;
            bail!(
                "'{}' has no characters table: it is not an L2JMobius game database",
                target.database
            );
        }
        let tables: Vec<String> = part
            .scripts
            .iter()
            .filter_map(migrate::script_table)
            .filter(|t| existing.contains_key(t))
            .collect();
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("USE `{}`", target.database)))
            .execute(&mut conn)
            .await?;
        let with_rows = migrate::tables_with_rows(&mut conn, tables).await?;
        let _ = conn.close().await;
        let plan = migrate::plan(&part.scripts, &existing, &with_rows);
        println!(
            "\nMigration of '{}': {} tables complete, {} tables to add, {} columns to add, {} default-row statements for empty tables. Nothing is deleted or overwritten.",
            target.database,
            plan.complete_tables,
            plan.new_tables.len(),
            plan.new_columns.len(),
            plan.steps
                .iter()
                .filter(|s| matches!(s, migrate::Step::Run { .. }))
                .count()
        );
        for table in &plan.new_tables {
            println!("  + table {table}");
        }
        for (table, column) in &plan.new_columns {
            println!("  + column {table}.{column}");
        }
        plans.push(plan);
    }

    // The settings, then the backup of each database (it reads them from Database.ini).
    write_targets(out, login, game)?;
    let mut backups = Vec::new();
    let inis: Vec<PathBuf> = if login.database == game.database {
        vec![out.join("game/config/Database.ini")]
    } else {
        vec![
            out.join("login/config/Database.ini"),
            out.join("game/config/Database.ini"),
        ]
    };
    for ini in inis {
        let backup = l2_supervisor::backup::BackupConfig {
            folder: out.join("backups"),
            database_ini: ini,
            keep: 100,
            ..l2_supervisor::backup::BackupConfig::defaults(&out.join("bin"))
        };
        let now = chrono::Local::now().naive_local();
        match tokio::task::spawn_blocking(move || l2_supervisor::backup::run_backup(&backup, now)).await? {
            Ok(info) => {
                println!("Backup: backups/{} ({} bytes).", info.name, info.size);
                backups.push(info.name);
            }
            Err(e) => {
                println!("No backup could be made: {e}");
                if !prompt.confirm_word("Migrate without a backup?", "NOBACKUP") {
                    bail!("nothing was changed (back the database up yourself, then run the setup again)");
                }
            }
        }
    }

    if plans.iter().all(migrate::Plan::is_empty) {
        println!("The database is up to date.");
        return Ok(());
    }
    if !prompt.confirm("Apply the migration now?", true) {
        bail!("the migration was not applied");
    }
    let restore = if backups.is_empty() {
        "your own backup".to_owned()
    } else {
        format!("backups/{}", backups.join(", backups/"))
    };
    for (part, plan) in parts.iter().zip(&plans) {
        let target = part.target;
        let mut conn = connect(target).await?;
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!("USE `{}`", target.database)))
            .execute(&mut conn)
            .await?;
        let yes = prompt.yes;
        let applied = migrate::apply(&mut conn, plan, |sql, error| {
            eprintln!("[ERROR] {sql}\n        {error}");
            !yes && (Prompt { yes: false }).confirm("Continue with the next step?", false)
        })
        .await?;
        let _ = conn.close().await;
        println!(
            "Migration of '{}': {} steps ok, {} failed, {} not run.",
            target.database, applied.ok, applied.failed, applied.not_run
        );
        if !applied.complete() {
            bail!(
                "the migration of '{}' is incomplete: do not start the server on it. Fix the error and run the setup again (it finishes the remaining steps), or restore {restore}",
                target.database
            );
        }
    }
    Ok(())
}

/// The old Java game server's registration (`hexid.txt`: the login server knows this server as
/// before) and network setup (`ipconfig.xml`).
fn java_server_files(out: &Path, prompt: &Prompt, java_server: Option<&Path>) -> anyhow::Result<()> {
    let dir = match java_server {
        Some(dir) => dir.to_path_buf(),
        None => {
            let answer = prompt.ask(
                "Old Java game server folder, to keep its server id and network setup (empty: skip)",
                "",
            );
            if answer.is_empty() {
                return Ok(());
            }
            PathBuf::from(answer)
        }
    };
    for file in ["hexid.txt", "ipconfig.xml"] {
        let from = [dir.join("config").join(file), dir.join(file)]
            .into_iter()
            .find(|p| p.is_file());
        let to = out.join("game/config").join(file);
        match from {
            Some(from)
                if !to.exists()
                    || prompt.confirm(&format!("game/config/{file} exists: replace it"), false) =>
            {
                std::fs::copy(&from, &to).with_context(|| format!("copying {}", from.display()))?;
                println!("{file} copied from the old server.");
            }
            Some(_) => println!("game/config/{file} kept."),
            None => println!("No {file} in {}.", dir.display()),
        }
    }
    Ok(())
}

/// `game/config/ipconfig.xml`: without it the game server tells every client 127.0.0.1, so only
/// this PC can enter the game.
fn network_address(out: &Path, prompt: &Prompt, address: Option<&str>) -> anyhow::Result<()> {
    let path = out.join("game/config/ipconfig.xml");
    if path.exists() && address.is_none() {
        return Ok(());
    }
    let address = match address {
        Some(a) => a.to_owned(),
        None => prompt.ask(
            "Address players connect to: this PC's LAN address, or the internet address (empty: only this PC)",
            "",
        ),
    };
    let address = address.trim();
    if address.is_empty() {
        return Ok(());
    }
    if address.contains(['"', '<', '>', '&', ' ']) {
        bail!("'{address}' is not an address");
    }
    std::fs::write(
        &path,
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n\
<!-- Written by the setup. Players get the address of the first matching subnet, everybody else\n     the gameserver address. See default-ipconfig.xml. -->\n\
<gameserver address=\"{address}\">\n\
\t<define subnet=\"127.0.0.0/8\" address=\"127.0.0.1\" />\n\
</gameserver>\n"
        ),
    )
    .context("writing game/config/ipconfig.xml")?;
    println!("game/config/ipconfig.xml written: players are sent to {address}.");
    Ok(())
}
