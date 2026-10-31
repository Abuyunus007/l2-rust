//! The installation itself, shared by the GUI and the console front ends.
// Java: org/l2jmobius/tools/DatabaseInstaller.java (`installDatabase`, `createDatabase`,
// `executeDatabaseScripts`, `executeSQLScript`)
//
// The front ends implement [`Ui`]; this module never prints or draws anything. Nothing here
// touches the database before [`run`] is called (the Install button / the console confirmation).

use std::fs;
use std::path::{Path, PathBuf};

use sqlx::mysql::MySqlConnectOptions;
use sqlx::{AssertSqlSafe, ConnectOptions, Connection, MySqlConnection};

use crate::splitter::split_statements;

/// Longest SQL text echoed into the log (some game scripts have statements of many kilobytes).
const LOG_SQL_LIMIT: usize = 200;

/// The two script folders below the SQL root, in the order Java installs them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Schema {
    Login,
    Game,
}

impl Schema {
    /// Folder name below the SQL root (Java: `sql/<dbType>`).
    pub fn dir_name(self) -> &'static str {
        match self {
            Schema::Login => "login",
            Schema::Game => "game",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Info,
    Success,
    Error,
}

/// The answer to "the database already exists" (Java: Delete and Recreate / Install on Existing
/// Database / Cancel).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ExistsChoice {
    Reset,
    Keep,
    Cancel,
}

/// The answer to "a statement failed" (Java: Continue / Abort).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorChoice {
    Continue,
    Abort,
}

/// What a front end provides. The questions block until the user answered.
pub trait Ui {
    fn log(&mut self, level: Level, message: &str);
    /// Overall progress, `done` of `total` statements.
    fn progress(&mut self, done: usize, total: usize);
    fn ask_exists(&mut self, db_name: &str) -> ExistsChoice;
    fn ask_error(&mut self, sql: &str, error: &str) -> ErrorChoice;
}

/// Everything [`run`] needs. The password lives only inside `options` (never printed or stored).
pub struct Params {
    /// Host, port, user and password; no database selected.
    pub options: MySqlConnectOptions,
    pub db_name: String,
    pub sql_dir: PathBuf,
    pub login: bool,
    pub game: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Summary {
    pub files: usize,
    pub statements_ok: usize,
    pub statements_failed: usize,
}

#[derive(Debug, thiserror::Error)]
pub enum InstallError {
    #[error("Database name cannot be empty.")]
    EmptyName,
    #[error("Database name cannot contain a backtick.")]
    BadName,
    #[error("Select at least one of Login and Game.")]
    NothingSelected,
    #[error("Directory not found: {0}")]
    MissingDir(PathBuf),
    #[error("No SQL files found in directory: {0}")]
    NoFiles(PathBuf),
    #[error("Error reading SQL file {0}: {1}")]
    Read(PathBuf, std::io::Error),
    #[error("{0}")]
    Database(#[from] sqlx::Error),
    #[error("Installation cancelled.")]
    Cancelled,
    #[error("Installation aborted after an SQL error.")]
    Aborted,
}

/// One `.sql` file with its statements.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Script {
    pub schema: Schema,
    pub name: String,
    pub statements: Vec<String>,
}

/// The `.sql` files of one schema folder, in install order, already split.
///
/// NOTE(java-quirk): Java sorts by `String.CASE_INSENSITIVE_ORDER` on the file name and only
/// takes names ending in exactly `.sql`; same here (ties broken by the plain name so the order
/// does not depend on the file system).
pub fn load_scripts(sql_dir: &Path, schema: Schema) -> Result<Vec<Script>, InstallError> {
    let dir = sql_dir.join(schema.dir_name());
    if !dir.is_dir() {
        return Err(InstallError::MissingDir(dir));
    }
    let entries = fs::read_dir(&dir).map_err(|e| InstallError::Read(dir.clone(), e))?;
    let mut files: Vec<(String, PathBuf)> = Vec::new();
    for entry in entries {
        let entry = entry.map_err(|e| InstallError::Read(dir.clone(), e))?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.ends_with(".sql") && entry.path().is_file() {
            files.push((name, entry.path()));
        }
    }
    if files.is_empty() {
        return Err(InstallError::NoFiles(dir));
    }
    files.sort_by(|a, b| {
        a.0.to_lowercase()
            .cmp(&b.0.to_lowercase())
            .then_with(|| a.0.cmp(&b.0))
    });
    let mut scripts = Vec::with_capacity(files.len());
    for (name, path) in files {
        let bytes = fs::read(&path).map_err(|e| InstallError::Read(path.clone(), e))?;
        scripts.push(Script {
            schema,
            name,
            statements: split_statements(&String::from_utf8_lossy(&bytes)),
        });
    }
    Ok(scripts)
}

/// Installs the selected schemas. Order as in the Java GUI: create/reset the database, then
/// login, then game.
///
/// All script files are read first, so a wrong SQL folder fails before any connection is made.
pub async fn run(params: &Params, ui: &mut dyn Ui) -> Result<Summary, InstallError> {
    let db_name = params.db_name.trim();
    if db_name.is_empty() {
        return Err(InstallError::EmptyName);
    }
    // The name goes into `CREATE DATABASE` between backticks.
    if db_name.contains('`') {
        return Err(InstallError::BadName);
    }
    if !params.login && !params.game {
        return Err(InstallError::NothingSelected);
    }

    let mut scripts = Vec::new();
    if params.login {
        scripts.extend(load_scripts(&params.sql_dir, Schema::Login)?);
    }
    if params.game {
        scripts.extend(load_scripts(&params.sql_dir, Schema::Game)?);
    }
    let total: usize = scripts.iter().map(|s| s.statements.len()).sum();

    prepare_database(params, db_name, ui).await?;

    let mut conn = params.options.clone().database(db_name).connect().await?;
    let mut summary = Summary::default();
    let mut done = 0;
    ui.progress(0, total);
    let mut current_schema = None;
    for script in &scripts {
        if current_schema != Some(script.schema) {
            ui.log(
                Level::Info,
                &format!("Installing {} database...", script.schema.dir_name()),
            );
            current_schema = Some(script.schema);
        }
        ui.log(Level::Info, &format!("Installing {}", script.name));
        summary.files += 1;
        for sql in &script.statements {
            match sqlx::raw_sql(AssertSqlSafe(sql.clone())).execute(&mut conn).await {
                Ok(_) => summary.statements_ok += 1,
                Err(e) => {
                    summary.statements_failed += 1;
                    ui.log(
                        Level::Error,
                        &format!("Error executing SQL: {}\nError: {e}", shorten(sql)),
                    );
                    // NOTE(java-quirk): Java only asks in the GUI and then calls
                    // `System.exit(0)` on Abort; the console version just goes on. Both ask here.
                    if ui.ask_error(&shorten(sql), &e.to_string()) == ErrorChoice::Abort {
                        let _ = conn.close().await;
                        return Err(InstallError::Aborted);
                    }
                }
            }
            done += 1;
            ui.progress(done, total);
        }
    }
    let _ = conn.close().await;
    Ok(summary)
}

/// Creates the database, or asks what to do when it exists.
async fn prepare_database(params: &Params, db_name: &str, ui: &mut dyn Ui) -> Result<(), InstallError> {
    let mut conn: MySqlConnection = params.options.clone().connect().await?;
    ui.log(Level::Info, "Connected.");
    // Java asks `SHOW DATABASES LIKE '<name>'`, where `_` and `%` are wildcards; an exact
    // comparison is what was meant.
    let exists: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM information_schema.SCHEMATA WHERE SCHEMA_NAME = ?")
            .bind(db_name)
            .fetch_one(&mut conn)
            .await?;
    let result = if exists > 0 {
        match ui.ask_exists(db_name) {
            ExistsChoice::Reset => {
                // NOTE(java-quirk): Java dumps the old database to `dumps/` first; this port
                // does not (see the card report). The question warns about the data loss.
                reset_database(&mut conn, db_name, ui).await
            }
            ExistsChoice::Keep => {
                ui.log(Level::Info, "Proceeding with installation on existing database.");
                Ok(())
            }
            ExistsChoice::Cancel => {
                ui.log(Level::Info, "Installation cancelled.");
                Err(InstallError::Cancelled)
            }
        }
    } else {
        create_database(&mut conn, db_name).await.map(|()| {
            ui.log(
                Level::Info,
                &format!("Database '{db_name}' created successfully."),
            )
        })
    };
    let _ = conn.close().await;
    result?;
    ui.log(Level::Info, &format!("Database '{db_name}' is ready."));
    Ok(())
}

async fn create_database(conn: &mut MySqlConnection, db_name: &str) -> Result<(), InstallError> {
    sqlx::raw_sql(AssertSqlSafe(format!("CREATE DATABASE `{db_name}`")))
        .execute(conn)
        .await?;
    Ok(())
}

async fn reset_database(
    conn: &mut MySqlConnection,
    db_name: &str,
    ui: &mut dyn Ui,
) -> Result<(), InstallError> {
    sqlx::raw_sql(AssertSqlSafe(format!("DROP DATABASE `{db_name}`")))
        .execute(&mut *conn)
        .await?;
    ui.log(Level::Info, &format!("Database '{db_name}' deleted."));
    create_database(conn, db_name).await?;
    ui.log(Level::Info, &format!("Database '{db_name}' created."));
    Ok(())
}

fn shorten(sql: &str) -> String {
    if sql.chars().count() <= LOG_SQL_LIMIT {
        return sql.to_owned();
    }
    let cut: String = sql.chars().take(LOG_SQL_LIMIT).collect();
    format!("{cut}...")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, text: &str) {
        let path = dir.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    #[test]
    fn scripts_are_sorted_case_insensitively_and_filtered() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "login/b.sql", "SELECT 2;");
        write(dir.path(), "login/A.sql", "SELECT 1;");
        write(dir.path(), "login/c.SQL", "SELECT 9;");
        write(dir.path(), "login/notes.txt", "x");
        let scripts = load_scripts(dir.path(), Schema::Login).unwrap();
        let names: Vec<_> = scripts.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["A.sql", "b.sql"]);
        assert_eq!(scripts[0].statements, ["SELECT 1;"]);
    }

    #[test]
    fn missing_folder_and_empty_folder_are_errors() {
        let dir = tempfile::tempdir().unwrap();
        assert!(matches!(
            load_scripts(dir.path(), Schema::Game),
            Err(InstallError::MissingDir(_))
        ));
        fs::create_dir_all(dir.path().join("game")).unwrap();
        assert!(matches!(
            load_scripts(dir.path(), Schema::Game),
            Err(InstallError::NoFiles(_))
        ));
    }

    #[test]
    fn long_sql_is_shortened_for_the_log() {
        let long = "x".repeat(500);
        let short = shorten(&long);
        assert_eq!(short.chars().count(), LOG_SQL_LIMIT + 3);
        assert_eq!(shorten("SELECT 1;"), "SELECT 1;");
    }

    struct NoUi;
    impl Ui for NoUi {
        fn log(&mut self, _: Level, _: &str) {}
        fn progress(&mut self, _: usize, _: usize) {}
        fn ask_exists(&mut self, _: &str) -> ExistsChoice {
            panic!("must not be asked");
        }
        fn ask_error(&mut self, _: &str, _: &str) -> ErrorChoice {
            panic!("must not be asked");
        }
    }

    fn params(sql_dir: PathBuf, db: &str, login: bool, game: bool) -> Params {
        Params {
            // Port 1 on loopback: a connect attempt would fail, but the checks below must fail
            // before any attempt is made.
            options: MySqlConnectOptions::new().host("127.0.0.1").port(1),
            db_name: db.to_owned(),
            sql_dir,
            login,
            game,
        }
    }

    #[tokio::test]
    async fn bad_input_fails_before_any_connection() {
        let dir = tempfile::tempdir().unwrap();
        let sql = dir.path().to_path_buf();
        let mut ui = NoUi;
        assert!(matches!(
            run(&params(sql.clone(), " ", true, true), &mut ui).await,
            Err(InstallError::EmptyName)
        ));
        assert!(matches!(
            run(&params(sql.clone(), "a`b", true, true), &mut ui).await,
            Err(InstallError::BadName)
        ));
        assert!(matches!(
            run(&params(sql.clone(), "db", false, false), &mut ui).await,
            Err(InstallError::NothingSelected)
        ));
        // The SQL folder is checked before connecting.
        assert!(matches!(
            run(&params(sql, "db", true, false), &mut ui).await,
            Err(InstallError::MissingDir(_))
        ));
    }
}
