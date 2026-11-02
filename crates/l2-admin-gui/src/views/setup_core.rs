//! The logic of the setup wizard (G-02), without any drawing: where the run folder is, what is
//! copied, which keys of which `.ini` files change, the text of `supervisor.ini`, and the
//! readiness checks. `views/setup.rs` only calls these.
//!
//! Two folder layouts are understood:
//!
//! ```text
//! Repo (cargo build)                   Package (tools/package.py)
//! <repo>/dist/{login,game,db_installer}  <root>/bin/   exes + supervisor.ini
//! <repo>/run/supervisor.ini              <root>/login/ config/ data/
//! <repo>/run/login/  <- copy of dist     <root>/game/  config/ data/ (datapack, geodata)
//! <repo>/run/game/config/ <- copy        <root>/db_installer/sql
//! <repo>/target/release/ exes
//! ```
//!
//! Nothing here touches a database. No token is generated: on this machine the GUI needs none and
//! the supervisor hands the servers theirs through `L2_ADMIN_TOKEN` (D-56), so `Token` and
//! `AdminToken` stay empty and only `Admin.ini: Enabled` is switched on.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use l2_config::ini_edit::{self, IniDoc, IniEditError};
use l2_config::{IniFile, JdbcUrl};
use l2_supervisor::launcher::supervisor_exe_name;

/// The part of the Java `URL` after the `?` (Java default of `Database.ini`); only host, port and
/// database name of the URL change.
pub const DEFAULT_URL_QUERY: &str = "useUnicode=true&characterEncoding=utf-8&allowPublicKeyRetrieval=true&useSSL=false&connectTimeout=10000&interactiveClient=true&sessionVariables=wait_timeout=600,interactive_timeout=600&autoReconnect=true";

/// Name of the optional Java game server in `supervisor.ini`.
pub const JAVA_SERVER_NAME: &str = "javagame";

/// Java defaults of the keys the wizard sets.
pub const DEFAULT_DB_HOST: &str = "localhost";
pub const DEFAULT_DB_PORT: &str = "3306";
pub const DEFAULT_DB_USER: &str = "root";
pub const DEFAULT_DB_NAME: &str = "l2jmobiush5";

// ---------------------------------------------------------------------------------------------
// Layout

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutKind {
    Repo,
    Package,
}

/// Where everything is. All paths are absolute when built from absolute input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Layout {
    pub kind: LayoutKind,
    /// Repo root or package root.
    pub root: PathBuf,
    /// The folder with `supervisor.ini` (`<repo>/run`, `<package>/bin`).
    pub run_folder: PathBuf,
    /// Working directory of the login server (has `config/`).
    pub login_dir: PathBuf,
    /// Working directory of the game server (has `config/`).
    pub game_dir: PathBuf,
    /// Where `data/` (the datapack) lives.
    pub datapack_root: PathBuf,
    /// The installer's `sql/` folder.
    pub sql_dir: PathBuf,
    /// Where `login-server` and `game-server` are.
    pub programs_dir: PathBuf,
    /// Repo only: what the run folder is copied from.
    pub dist: Option<PathBuf>,
}

impl Layout {
    /// A repository checkout.
    pub fn repo(root: &Path) -> Self {
        let run = root.join("run");
        Self {
            kind: LayoutKind::Repo,
            root: root.to_path_buf(),
            login_dir: run.join("login"),
            game_dir: run.join("game"),
            run_folder: run,
            datapack_root: root.join("dist").join("game"),
            sql_dir: root.join("dist").join("db_installer").join("sql"),
            programs_dir: root.join("target").join("release"),
            dist: Some(root.join("dist")),
        }
    }

    /// A release package; `bin` is the folder of the programs.
    pub fn package(root: &Path, bin: &Path) -> Self {
        Self {
            kind: LayoutKind::Package,
            root: root.to_path_buf(),
            run_folder: bin.to_path_buf(),
            login_dir: root.join("login"),
            game_dir: root.join("game"),
            datapack_root: root.join("game"),
            sql_dir: root.join("db_installer").join("sql"),
            programs_dir: bin.to_path_buf(),
            dist: None,
        }
    }

    /// Finds the layout the running GUI belongs to: a package when `<exe dir>/../login/config` and
    /// `../game/config` exist, else the repository (the exe is in `target/<profile>`, or the
    /// working directory is inside the repo).
    pub fn detect(exe: &Path, cwd: &Path) -> Option<Self> {
        let exe_dir = exe.parent();
        if let Some((bin, root)) = exe_dir.and_then(|bin| Some((bin, bin.parent()?)))
            && root.join("login").join("config").is_dir()
            && root.join("game").join("config").is_dir()
        {
            return Some(Self::package(root, bin));
        }
        let from_exe = exe_dir.and_then(Path::parent).and_then(Path::parent);
        from_exe
            .into_iter()
            .chain(cwd.ancestors())
            .find(|dir| dir.join("dist").join("login").join("config").is_dir())
            .map(Self::repo)
    }

    pub fn login_config(&self, file: &str) -> PathBuf {
        self.login_dir.join("config").join(file)
    }

    pub fn game_config(&self, file: &str) -> PathBuf {
        self.game_dir.join("config").join(file)
    }

    pub fn supervisor_ini(&self) -> PathBuf {
        self.run_folder.join(l2_supervisor::launcher::INI_NAME)
    }

    /// Command and working directory as written in `supervisor.ini` (relative to its folder).
    fn supervisor_paths(&self) -> SupervisorPaths {
        let exe = std::env::consts::EXE_SUFFIX;
        match self.kind {
            LayoutKind::Repo => SupervisorPaths {
                login_command: format!("../target/release/login-server{exe}"),
                game_command: format!("../target/release/game-server{exe}"),
                login_dir: "login".into(),
                game_dir: "game".into(),
            },
            LayoutKind::Package => SupervisorPaths {
                login_command: format!("./login-server{exe}"),
                game_command: format!("./game-server{exe}"),
                login_dir: "../login".into(),
                game_dir: "../game".into(),
            },
        }
    }

    /// `DatapackRoot` as it should stand in the game's `Server.ini` by default: the repo's
    /// `dist/game` as an absolute path, `.` in a package (the game server starts in its folder).
    pub fn default_datapack_value(&self) -> String {
        match self.kind {
            LayoutKind::Repo => fwd(&self.datapack_root),
            LayoutKind::Package => ".".to_owned(),
        }
    }

    /// A `DatapackRoot` value as a path: a relative one is relative to the game's working folder.
    pub fn resolve_datapack(&self, value: &str) -> PathBuf {
        resolve_against(&self.game_dir, value)
    }

    pub fn default_geodata(&self, datapack_root: &Path) -> PathBuf {
        datapack_root.join("data").join("geodata")
    }
}

struct SupervisorPaths {
    login_command: String,
    game_command: String,
    login_dir: String,
    game_dir: String,
}

fn resolve_against(base: &Path, value: &str) -> PathBuf {
    let value = value.trim().replace('\\', "/");
    let path = Path::new(&value);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        base.join(path)
    }
}

/// Forward slashes, and without the `\\?\` prefix Windows adds to canonical paths: the form the
/// `.ini` files use.
pub fn fwd(path: &Path) -> String {
    // `components()` drops the `.` parts in the middle and a trailing slash.
    let clean: PathBuf = path.components().collect();
    let text = clean.to_string_lossy().replace('\\', "/");
    text.strip_prefix("//?/").map(str::to_owned).unwrap_or(text)
}

// ---------------------------------------------------------------------------------------------
// Step 1: copy dist to the run folder

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyItem {
    pub from: PathBuf,
    pub to: PathBuf,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CopyPlan {
    /// Target does not exist yet.
    pub create: Vec<CopyItem>,
    /// Target exists; only copied when the user allows overwriting.
    pub existing: Vec<CopyItem>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct CopyResult {
    pub copied: usize,
    pub skipped: usize,
}

/// `dist/login` -> `<run>/login` and `dist/game/config` -> `<run>/game/config`. Empty for a
/// package (already in place).
pub fn plan_copy(layout: &Layout) -> io::Result<CopyPlan> {
    let mut plan = CopyPlan::default();
    let Some(dist) = &layout.dist else {
        return Ok(plan);
    };
    let pairs = [
        (dist.join("login"), layout.login_dir.clone()),
        (dist.join("game").join("config"), layout.game_dir.join("config")),
    ];
    for (from, to) in pairs {
        let mut items = Vec::new();
        collect_files(&from, &to, &mut items)?;
        for item in items {
            if item.to.exists() {
                plan.existing.push(item);
            } else {
                plan.create.push(item);
            }
        }
    }
    Ok(plan)
}

fn collect_files(from: &Path, to: &Path, out: &mut Vec<CopyItem>) -> io::Result<()> {
    let mut entries: Vec<_> = fs::read_dir(from)?.collect::<Result<_, _>>()?;
    entries.sort_by_key(|e| e.file_name());
    for entry in entries {
        let (src, dst) = (entry.path(), to.join(entry.file_name()));
        if entry.file_type()?.is_dir() {
            collect_files(&src, &dst, out)?;
        } else {
            out.push(CopyItem { from: src, to: dst });
        }
    }
    Ok(())
}

/// Copies the files of `plan`. Existing targets are replaced only with `overwrite`.
pub fn execute_copy(plan: &CopyPlan, overwrite: bool) -> io::Result<CopyResult> {
    let mut result = CopyResult::default();
    let items = plan
        .create
        .iter()
        .map(|i| (i, false))
        .chain(plan.existing.iter().map(|i| (i, true)));
    for (item, exists) in items {
        // Checked again here: a file may have appeared since the plan was made.
        if (exists || item.to.exists()) && !overwrite {
            result.skipped += 1;
            continue;
        }
        if let Some(dir) = item.to.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::copy(&item.from, &item.to)?;
        result.copied += 1;
    }
    Ok(result)
}

// ---------------------------------------------------------------------------------------------
// The form

/// One switch of `SideBySide.ini`, with the help text of the file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Switch {
    pub key: String,
    pub help: String,
    pub value: bool,
    /// The value in the file when it was read; only a changed switch is written.
    pub original: bool,
}

/// What the user can set. The password lives only here (and in `Database.ini`): it is not part of
/// the GUI settings and `Debug` leaves it out.
#[derive(Clone, PartialEq, Eq)]
pub struct Form {
    pub db_host: String,
    pub db_port: String,
    pub db_user: String,
    pub db_password: String,
    pub db_name: String,
    /// `LoginserverPort` of the login server (the game client connects here).
    pub login_client_port: String,
    /// `LoginPort`: where game servers connect to the login server. In both `Server.ini` files.
    pub login_server_port: String,
    pub game_port: String,
    /// `RequestServerID`.
    pub server_id: String,
    pub datapack_root: String,
    pub geodata_dir: String,
    pub java_enabled: bool,
    pub java_folder: String,
    pub switches: Vec<Switch>,
    /// Write `supervisor.ini` even if the file exists (the old one is kept as `.bak`).
    pub rewrite_supervisor: bool,
}

impl std::fmt::Debug for Form {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Form")
            .field("db_host", &self.db_host)
            .field("db_port", &self.db_port)
            .field("db_user", &self.db_user)
            .field("db_password", &"<hidden>")
            .field("db_name", &self.db_name)
            .field("login_client_port", &self.login_client_port)
            .field("login_server_port", &self.login_server_port)
            .field("game_port", &self.game_port)
            .field("server_id", &self.server_id)
            .field("datapack_root", &self.datapack_root)
            .field("geodata_dir", &self.geodata_dir)
            .field("java_enabled", &self.java_enabled)
            .finish_non_exhaustive()
    }
}

fn read_ini(path: &Path) -> Option<IniFile> {
    IniFile::load(path).ok()
}

fn text(ini: Option<&IniFile>, key: &str) -> Option<String> {
    ini.and_then(|i| i.get_value(key)).map(str::to_owned)
}

impl Form {
    /// The Java defaults, with the layout's `DatapackRoot`.
    pub fn defaults(layout: &Layout) -> Self {
        let datapack = layout.default_datapack_value();
        let geodata = layout.default_geodata(&layout.resolve_datapack(&datapack));
        Self {
            db_host: DEFAULT_DB_HOST.into(),
            db_port: DEFAULT_DB_PORT.into(),
            db_user: DEFAULT_DB_USER.into(),
            db_password: String::new(),
            db_name: DEFAULT_DB_NAME.into(),
            login_client_port: "2106".into(),
            login_server_port: "9014".into(),
            game_port: "7777".into(),
            server_id: "1".into(),
            datapack_root: datapack,
            geodata_dir: fwd(&geodata),
            java_enabled: false,
            java_folder: String::new(),
            switches: Vec::new(),
            rewrite_supervisor: true,
        }
    }

    /// The defaults, overridden by whatever the files in the run folder say now (the wizard opened
    /// again later shows the current values).
    pub fn load(layout: &Layout) -> Self {
        let mut form = Self::defaults(layout);
        let game_db = read_ini(&layout.game_config("Database.ini"));
        let login_db = read_ini(&layout.login_config("Database.ini"));
        let db = game_db.as_ref().or(login_db.as_ref());
        if let Some(url) = text(db, "URL").and_then(|u| JdbcUrl::parse(&u)) {
            form.db_host = url.host;
            form.db_port = url.port.to_string();
            if !url.database.is_empty() {
                form.db_name = url.database;
            }
        }
        if let Some(v) = text(db, "Login") {
            form.db_user = v;
        }
        if let Some(v) = text(db, "Password") {
            form.db_password = v;
        }

        let login_server = read_ini(&layout.login_config("Server.ini"));
        let game_server = read_ini(&layout.game_config("Server.ini"));
        if let Some(v) = text(login_server.as_ref(), "LoginserverPort") {
            form.login_client_port = v;
        }
        if let Some(v) =
            text(login_server.as_ref(), "LoginPort").or_else(|| text(game_server.as_ref(), "LoginPort"))
        {
            form.login_server_port = v;
        }
        if let Some(v) = text(game_server.as_ref(), "GameserverPort") {
            form.game_port = v;
        }
        if let Some(v) = text(game_server.as_ref(), "RequestServerID") {
            form.server_id = v;
        }
        // The shipped `DatapackRoot = .` would mean the (empty) run/game folder in the repo layout.
        if let Some(v) = text(game_server.as_ref(), "DatapackRoot")
            && !(layout.kind == LayoutKind::Repo && v.trim() == ".")
        {
            form.datapack_root = v;
        }
        let datapack = layout.resolve_datapack(&form.datapack_root);
        form.geodata_dir = fwd(&layout.default_geodata(&datapack));
        if let Some(v) = text(
            read_ini(&layout.game_config("GeoEngine.ini")).as_ref(),
            "GeoDataPath",
        ) {
            form.geodata_dir = fwd(&resolve_against(&datapack, &v));
        }

        form.rewrite_supervisor = !layout.supervisor_ini().is_file();
        if let Ok(config) = l2_supervisor::config::SupervisorConfig::load(&layout.supervisor_ini())
            && let Some(java) = config.servers.iter().find(|s| s.name == JAVA_SERVER_NAME)
        {
            form.java_enabled = true;
            form.java_folder = java.working_dir.as_deref().map(fwd).unwrap_or_default();
        }
        form.switches = load_switches(&layout.game_config("SideBySide.ini"));
        form
    }

    /// Checks every field. All problems at once, in words for the screen.
    pub fn parse(&self) -> Result<Values, Vec<String>> {
        let mut problems = Vec::new();
        let mut port = |label: &str, text: &str| -> u16 {
            match text.trim().parse::<u16>() {
                Ok(p) if p > 0 => p,
                _ => {
                    problems.push(format!("{label}: '{}' is not a port (1 to 65535)", text.trim()));
                    0
                }
            }
        };
        let db_port = port("Database port", &self.db_port);
        let login_client_port = port("Login client port (LoginserverPort)", &self.login_client_port);
        let login_server_port = port("Login port for game servers (LoginPort)", &self.login_server_port);
        let game_port = port("Game server port (GameserverPort)", &self.game_port);
        let server_id = match self.server_id.trim().parse::<u8>() {
            Ok(id) if (1..=127).contains(&id) => id,
            _ => {
                problems.push(format!(
                    "Server id (RequestServerID): '{}' must be 1 to 127",
                    self.server_id.trim()
                ));
                0
            }
        };
        for (label, value) in [
            ("Database host", &self.db_host),
            ("Database user", &self.db_user),
            ("Database name", &self.db_name),
            ("Datapack folder (DatapackRoot)", &self.datapack_root),
            ("Geodata folder", &self.geodata_dir),
        ] {
            if value.trim().is_empty() {
                problems.push(format!("{label} is empty"));
            }
        }
        if self.db_name.contains(['`', '/', '?', '#']) {
            problems.push("Database name must not contain ` / ? #".into());
        }
        if self.db_host.contains(['/', '?', ' ']) {
            problems.push("Database host must not contain / ? or a blank".into());
        }
        for (label, value) in [
            ("Database user", &self.db_user),
            ("Database password", &self.db_password),
            ("Datapack folder", &self.datapack_root),
            ("Geodata folder", &self.geodata_dir),
        ] {
            if value.contains(['\n', '\r']) {
                problems.push(format!("{label} must be one line"));
            }
        }
        if self.db_password.starts_with([' ', '\t']) {
            problems.push("Database password must not start with a blank".into());
        }
        if login_client_port != 0 && login_client_port == login_server_port {
            problems.push("The two login ports must differ".into());
        }
        if game_port != 0 && (game_port == login_client_port || game_port == login_server_port) {
            problems.push("The game port must differ from the login ports".into());
        }
        if self.java_enabled && self.java_folder.trim().is_empty() {
            problems.push("Java game server: the folder is empty".into());
        }
        if problems.is_empty() {
            Ok(Values {
                db_port,
                login_client_port,
                login_server_port,
                game_port,
                server_id,
            })
        } else {
            Err(problems)
        }
    }
}

/// The numeric fields of a valid [`Form`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Values {
    pub db_port: u16,
    pub login_client_port: u16,
    pub login_server_port: u16,
    pub game_port: u16,
    pub server_id: u8,
}

/// The True/False keys of `SideBySide.ini` with their help text (comment lines above the key).
pub fn load_switches(path: &Path) -> Vec<Switch> {
    let Ok(text) = fs::read_to_string(path) else {
        return Vec::new();
    };
    IniDoc::parse(&text)
        .entries()
        .into_iter()
        .filter_map(|entry| {
            let value = match entry.value.trim().to_ascii_lowercase().as_str() {
                "true" => true,
                "false" => false,
                _ => return None,
            };
            let help = entry
                .help
                .iter()
                .map(|l| l.trim())
                .filter(|l| !l.starts_with("Default:"))
                .collect::<Vec<_>>()
                .join(" ");
            Some(Switch {
                key: entry.key,
                help,
                value,
                original: value,
            })
        })
        .collect()
}

// ---------------------------------------------------------------------------------------------
// Step 4: the edits

#[derive(Debug, thiserror::Error)]
pub enum SetupError {
    #[error("{}", .0.join("; "))]
    Invalid(Vec<String>),
    #[error("{0} does not exist yet: copy the files first (step 1)")]
    MissingFile(PathBuf),
    #[error("{path}: {source}")]
    Ini { path: PathBuf, source: IniEditError },
    #[error("{path}: {source}")]
    Io { path: PathBuf, source: io::Error },
}

/// One file and the values to set in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEdit {
    pub path: PathBuf,
    pub changes: Vec<(String, String)>,
}

/// Escapes a value for the `.ini` syntax (`java.util.Properties`): only the backslash needs it
/// once leading blanks and line breaks are refused.
pub fn escape_value(value: &str) -> String {
    value.replace('\\', "\\\\")
}

/// `jdbc:mysql://host:port/name?<Java default query>`.
pub fn jdbc_url(host: &str, port: u16, name: &str) -> String {
    let host = host.trim();
    // An IPv6 literal needs its brackets in a URL.
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    format!("jdbc:mysql://{host}:{port}/{}?{DEFAULT_URL_QUERY}", name.trim())
}

fn set(changes: &mut Vec<(String, String)>, key: &str, value: impl Into<String>) {
    changes.push((key.to_owned(), value.into()));
}

/// Every key the wizard writes, per file. `form` must have passed [`Form::parse`].
pub fn plan_edits(layout: &Layout, form: &Form, values: &Values) -> Vec<FileEdit> {
    let mut edits = Vec::new();

    for file_dir in [
        layout.login_config("Database.ini"),
        layout.game_config("Database.ini"),
    ] {
        let mut c = Vec::new();
        set(
            &mut c,
            "URL",
            jdbc_url(&form.db_host, values.db_port, &form.db_name),
        );
        set(&mut c, "Login", escape_value(form.db_user.trim()));
        set(&mut c, "Password", escape_value(&form.db_password));
        edits.push(FileEdit {
            path: file_dir,
            changes: c,
        });
    }

    let mut login = Vec::new();
    set(
        &mut login,
        "LoginserverPort",
        values.login_client_port.to_string(),
    );
    set(&mut login, "LoginPort", values.login_server_port.to_string());
    edits.push(FileEdit {
        path: layout.login_config("Server.ini"),
        changes: login,
    });

    let mut game = Vec::new();
    set(&mut game, "LoginPort", values.login_server_port.to_string());
    set(&mut game, "GameserverPort", values.game_port.to_string());
    set(&mut game, "RequestServerID", values.server_id.to_string());
    set(&mut game, "DatapackRoot", escape_value(form.datapack_root.trim()));
    edits.push(FileEdit {
        path: layout.game_config("Server.ini"),
        changes: game,
    });

    // GeoDataPath is resolved against DatapackRoot, and an absolute value is used as it is. Only
    // touched when the chosen folder is not where the current value already points.
    let datapack = layout.resolve_datapack(&form.datapack_root);
    let geo_file = layout.game_config("GeoEngine.ini");
    let wanted = resolve_against(&datapack, &form.geodata_dir);
    let current = read_ini(&geo_file)
        .and_then(|ini| text(Some(&ini), "GeoDataPath"))
        .map(|v| resolve_against(&datapack, &v));
    if current.as_deref().map(normalize) != Some(normalize(&wanted)) {
        let value = if normalize(&wanted) == normalize(&layout.default_geodata(&datapack)) {
            "./data/geodata/".to_owned()
        } else {
            escape_value(&fwd(&wanted))
        };
        let mut geo = Vec::new();
        set(&mut geo, "GeoDataPath", value);
        edits.push(FileEdit {
            path: geo_file,
            changes: geo,
        });
    }

    // Enabled = True; Token stays empty: the supervisor passes the token (D-56).
    for path in [layout.login_config("Admin.ini"), layout.game_config("Admin.ini")] {
        edits.push(FileEdit {
            path,
            changes: vec![("Enabled".to_owned(), "True".to_owned())],
        });
    }

    if form.java_enabled {
        let changed: Vec<_> = form
            .switches
            .iter()
            .filter(|s| s.value != s.original)
            .map(|s| (s.key.clone(), if s.value { "True" } else { "False" }.to_owned()))
            .collect();
        if !changed.is_empty() {
            edits.push(FileEdit {
                path: layout.game_config("SideBySide.ini"),
                changes: changed,
            });
        }
    }
    edits
}

/// For comparing two folder paths: slashes, no trailing slash, no `.` parts, lower case on Windows.
fn normalize(path: &Path) -> String {
    let text = fwd(path);
    let parts: Vec<&str> = text.split('/').filter(|p| *p != "." && !p.is_empty()).collect();
    let joined = format!(
        "{}{}",
        if text.starts_with('/') { "/" } else { "" },
        parts.join("/")
    );
    if cfg!(windows) {
        joined.to_lowercase()
    } else {
        joined
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ApplyReport {
    /// Files with at least one value changed, and how many.
    pub changed: Vec<(PathBuf, usize)>,
    /// Files that already had the values.
    pub unchanged: usize,
    /// `supervisor.ini` when it was written.
    pub supervisor: Option<PathBuf>,
}

/// Validates the form, then writes the `.ini` edits and `supervisor.ini`. Every file to edit must
/// exist (step 1 copies them) or nothing is written.
pub fn apply(layout: &Layout, form: &Form) -> Result<ApplyReport, SetupError> {
    let values = form.parse().map_err(SetupError::Invalid)?;
    let edits = plan_edits(layout, form, &values);
    if let Some(edit) = edits.iter().find(|e| !e.path.is_file()) {
        return Err(SetupError::MissingFile(edit.path.clone()));
    }
    let mut report = ApplyReport::default();
    for edit in &edits {
        let count = ini_edit::save_changes(&edit.path, &edit.changes).map_err(|source| SetupError::Ini {
            path: edit.path.clone(),
            source,
        })?;
        if count > 0 {
            report.changed.push((edit.path.clone(), count));
        } else {
            report.unchanged += 1;
        }
    }

    let ini_path = layout.supervisor_ini();
    if form.rewrite_supervisor || !ini_path.is_file() {
        let io_err = |source| SetupError::Io {
            path: ini_path.clone(),
            source,
        };
        if ini_path.is_file() {
            let mut backup = ini_path.as_os_str().to_owned();
            backup.push(".bak");
            fs::copy(&ini_path, PathBuf::from(backup)).map_err(io_err)?;
        }
        let content = supervisor_ini_text(layout, form);
        ini_edit::write_atomic(&ini_path, &content).map_err(|e| match e {
            IniEditError::Io(source) => io_err(source),
            other => SetupError::Ini {
                path: ini_path.clone(),
                source: other,
            },
        })?;
        report.supervisor = Some(ini_path);
    }
    Ok(report)
}

// ---------------------------------------------------------------------------------------------
// supervisor.ini

/// Text of `supervisor.ini`. `Listen` and the admin endpoints are kept from the existing files
/// (defaults 127.0.0.1:7900, :7901 and :7902); `Token` and `AdminToken` stay empty (D-56).
pub fn supervisor_ini_text(layout: &Layout, form: &Form) -> String {
    let paths = layout.supervisor_paths();
    let listen = l2_supervisor::config::SupervisorConfig::load(&layout.supervisor_ini())
        .map(|c| c.listen.to_string())
        .unwrap_or_else(|_| "127.0.0.1:7900".to_owned());
    let bind = |file: PathBuf, default: &str| {
        text(read_ini(&file).as_ref(), "Bind")
            .map(|b| b.trim().to_owned())
            .filter(|b| !b.is_empty())
            .unwrap_or_else(|| default.to_owned())
    };
    let login_admin = bind(layout.login_config("Admin.ini"), "127.0.0.1:7901");
    let game_admin = bind(layout.game_config("Admin.ini"), "127.0.0.1:7902");
    let servers = if form.java_enabled {
        format!("login, game, {JAVA_SERVER_NAME}")
    } else {
        "login, game".to_owned()
    };

    let mut out = format!(
        "# Supervisor settings, written by the setup wizard (D-36). Same syntax as the Java .ini files.\n\
         # Relative paths are relative to this file's folder.\n\
         \n\
         # Where the admin GUI connects. Keep it on localhost.\n\
         Listen = {listen}\n\
         \n\
         # No token on this machine: the GUI needs none for a local Listen, and the supervisor hands\n\
         # each server its admin token at start (D-56). A non-local Listen needs a Token here.\n\
         Token =\n\
         \n\
         LogBufferLines = 10000\n\
         StopTimeoutSecs = 30\n\
         RestartDelaySecs = 10\n\
         \n\
         # Servers, in display order (\"Start all\" goes in this order).\n\
         Servers = {servers}\n\
         \n\
         Server.login.Command = {lc}\n\
         Server.login.Args =\n\
         Server.login.WorkingDir = {ld}\n\
         Server.login.AdminEndpoint = {login_admin}\n\
         Server.login.AdminToken =\n\
         Server.login.Autostart = False\n\
         Server.login.RestartOnExitCode = 2\n\
         \n\
         Server.game.Command = {gc}\n\
         Server.game.Args = --root .\n\
         Server.game.WorkingDir = {gd}\n\
         Server.game.AdminEndpoint = {game_admin}\n\
         Server.game.AdminToken =\n\
         Server.game.Autostart = False\n\
         Server.game.RestartOnExitCode = 2\n",
        lc = paths.login_command,
        ld = paths.login_dir,
        gc = paths.game_command,
        gd = paths.game_dir,
    );
    if form.java_enabled {
        // Same command line as GameServer.sh / GameServer.vbs; needs `java` on the PATH.
        let folder = fwd(Path::new(form.java_folder.trim()));
        out.push_str(&format!(
            "\n# The Java game server next to the Rust one (side by side, D-39). Start it by hand.\n\
             Server.{JAVA_SERVER_NAME}.Command = java\n\
             Server.{JAVA_SERVER_NAME}.Args = @java.cfg -jar ../libs/GameServer.jar\n\
             Server.{JAVA_SERVER_NAME}.WorkingDir = {folder}\n\
             Server.{JAVA_SERVER_NAME}.Autostart = False\n\
             Server.{JAVA_SERVER_NAME}.RestartOnExitCode = 2\n"
        ));
    }
    out
}

// ---------------------------------------------------------------------------------------------
// Step 5: checks

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Ok,
    /// Worth a look, does not stop the start.
    Warn,
    Missing,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Check {
    pub label: &'static str,
    pub status: Status,
    pub detail: String,
}

/// What the database test said. Nothing is tested until the user presses "Test connection".
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub enum DbTest {
    #[default]
    NotTested,
    Testing,
    Reachable {
        database_exists: bool,
    },
    Failed(String),
}

fn check(label: &'static str, status: Status, detail: impl Into<String>) -> Check {
    Check {
        label,
        status,
        detail: detail.into(),
    }
}

fn file_check(label: &'static str, path: &Path, hint: &str) -> Check {
    if path.is_file() {
        check(label, Status::Ok, fwd(path))
    } else {
        check(
            label,
            Status::Missing,
            format!("{} is missing. {hint}", fwd(path)),
        )
    }
}

/// The geodata folder of the form as a path (a relative one is relative to the datapack folder,
/// like `GeoDataPath`).
pub fn geodata_path(layout: &Layout, form: &Form) -> PathBuf {
    resolve_against(&layout.resolve_datapack(&form.datapack_root), &form.geodata_dir)
}

/// Number of `*.l2j` files directly in `dir`.
pub fn count_geodata(dir: &Path) -> usize {
    fs::read_dir(dir)
        .map(|entries| {
            entries
                .filter_map(Result::ok)
                .filter(|e| {
                    e.path()
                        .extension()
                        .is_some_and(|x| x.eq_ignore_ascii_case("l2j"))
                })
                .count()
        })
        .unwrap_or(0)
}

/// What is ready and what is missing. `gui_dir` is the folder of the GUI exe (the launcher looks
/// for `l2-supervisor` there).
pub fn run_checks(layout: &Layout, gui_dir: &Path, form: &Form, db: &DbTest) -> Vec<Check> {
    let exe = std::env::consts::EXE_SUFFIX;
    let build_hint = "Build it: cargo build --release";
    let mut list = vec![
        file_check(
            "Supervisor program",
            &gui_dir.join(supervisor_exe_name()),
            "It must be next to the admin GUI. Build it: cargo build --release -p l2-supervisor",
        ),
        file_check(
            "Login server program",
            &layout.programs_dir.join(format!("login-server{exe}")),
            build_hint,
        ),
        file_check(
            "Game server program",
            &layout.programs_dir.join(format!("game-server{exe}")),
            build_hint,
        ),
        file_check(
            "Login settings",
            &layout.login_config("Database.ini"),
            "Copy the files first (step 1).",
        ),
        file_check(
            "Game settings",
            &layout.game_config("Database.ini"),
            "Copy the files first (step 1).",
        ),
        file_check(
            "supervisor.ini",
            &layout.supervisor_ini(),
            "Press \"Apply settings\" (step 4).",
        ),
    ];

    for (label, path) in [
        ("Login admin endpoint", layout.login_config("Admin.ini")),
        ("Game admin endpoint", layout.game_config("Admin.ini")),
    ] {
        let enabled = read_ini(&path).is_some_and(|ini| ini.get_bool("Enabled", false));
        list.push(if enabled {
            check(label, Status::Ok, "Enabled = True")
        } else {
            check(
                label,
                Status::Warn,
                "Enabled = False: the GUI cannot show this server's status. Press \"Apply settings\".",
            )
        });
    }

    let datapack = layout.resolve_datapack(&form.datapack_root);
    list.push(if datapack.join("data").is_dir() {
        check("Datapack", Status::Ok, fwd(&datapack))
    } else {
        check(
            "Datapack",
            Status::Missing,
            format!("{} has no data/ folder (DatapackRoot)", fwd(&datapack)),
        )
    });

    let geodata = geodata_path(layout, form);
    let regions = count_geodata(&geodata);
    list.push(if regions > 0 {
        check(
            "Geodata",
            Status::Ok,
            format!("{regions} region files in {}", fwd(&geodata)),
        )
    } else {
        check(
            "Geodata",
            Status::Warn,
            format!(
                "No .l2j files in {}. The game server starts, but nobody can move or fight properly.",
                fwd(&geodata)
            ),
        )
    });

    list.push(match db {
        DbTest::NotTested => check(
            "Database",
            Status::Warn,
            "Not tested. Press \"Test connection\" in step 2.",
        ),
        DbTest::Testing => check("Database", Status::Warn, "Testing..."),
        DbTest::Reachable {
            database_exists: true,
        } => check(
            "Database",
            Status::Ok,
            "The server answers and the database exists",
        ),
        DbTest::Reachable {
            database_exists: false,
        } => check(
            "Database",
            Status::Warn,
            "The server answers, but the database does not exist yet. Press \"Install database\".",
        ),
        DbTest::Failed(why) => check("Database", Status::Missing, why.clone()),
    });
    list
}

/// True when nothing is `Missing`.
pub fn ready_to_start(checks: &[Check]) -> bool {
    checks.iter().all(|c| c.status != Status::Missing)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn write(path: &Path, text: &str) {
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    const DB_INI: &str = "# db\r\nDriver = com.mysql.cj.jdbc.Driver\r\nURL = jdbc:mysql://localhost/l2jmobiush5?x=1\r\nLogin = root\r\nPassword = \r\nMaximumDatabaseConnections = 5\r\n";

    /// A small fake repo: dist with the files the wizard touches, CRLF where the real dist has it.
    fn fake_repo() -> (tempfile::TempDir, Layout) {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let dist = root.join("dist");
        for side in [dist.join("login"), dist.join("game")] {
            write(&side.join("config/Database.ini"), DB_INI);
            write(
                &side.join("config/Admin.ini"),
                "Enabled = False\nBind = 127.0.0.1:7901\n\nToken =\n",
            );
        }
        write(
            &dist.join("login/config/Server.ini"),
            "# s\nLoginserverPort = 2106\nLoginPort = 9014\n",
        );
        write(&dist.join("login/data/servername.xml"), "<x/>");
        write(&dist.join("login/banned_ip.cfg"), "");
        write(
            &dist.join("game/config/Server.ini"),
            "LoginHost = 127.0.0.1\nLoginPort = 9014\nGameserverPort = 7777\nRequestServerID = 1\nDatapackRoot = .\n",
        );
        write(
            &dist.join("game/config/GeoEngine.ini"),
            "# g\nGeoDataPath = ./data/geodata/\n",
        );
        write(
            &dist.join("game/config/SideBySide.ini"),
            "# Daily reset.\n# Set True only alone.\n# Default: False\nDailyResetDatabaseWrites = False\n\nMailExpiryDatabaseWrites = False\n",
        );
        write(
            &dist.join("game/config/Custom/Banking.ini"),
            "BankingEnabled = False\n",
        );
        write(&dist.join("game/data/stats/x.xml"), "<x/>");
        let layout = Layout::repo(root);
        (dir, layout)
    }

    fn copied(layout: &Layout) {
        let plan = plan_copy(layout).unwrap();
        execute_copy(&plan, false).unwrap();
    }

    #[test]
    fn copy_creates_the_run_folder_from_dist() {
        let (_dir, layout) = fake_repo();
        let plan = plan_copy(&layout).unwrap();
        assert!(plan.existing.is_empty());
        let result = execute_copy(&plan, false).unwrap();
        assert_eq!(result.copied, plan.create.len());
        assert!(layout.login_dir.join("data/servername.xml").is_file());
        assert!(layout.login_dir.join("banned_ip.cfg").is_file());
        assert!(layout.game_dir.join("config/Custom/Banking.ini").is_file());
        // The datapack is not copied (it stays in dist).
        assert!(!layout.game_dir.join("data").exists());
        // Byte-exact, CRLF included.
        assert_eq!(
            fs::read(layout.game_config("Database.ini")).unwrap(),
            DB_INI.as_bytes()
        );
    }

    #[test]
    fn copy_never_overwrites_without_the_flag() {
        let (_dir, layout) = fake_repo();
        copied(&layout);
        fs::write(layout.game_config("Database.ini"), "mine").unwrap();
        let plan = plan_copy(&layout).unwrap();
        assert!(plan.create.is_empty());
        assert!(
            plan.existing
                .iter()
                .any(|i| i.to == layout.game_config("Database.ini"))
        );
        let result = execute_copy(&plan, false).unwrap();
        assert_eq!(result.copied, 0);
        assert_eq!(result.skipped, plan.existing.len());
        assert_eq!(
            fs::read_to_string(layout.game_config("Database.ini")).unwrap(),
            "mine"
        );
        execute_copy(&plan, true).unwrap();
        assert_eq!(
            fs::read(layout.game_config("Database.ini")).unwrap(),
            DB_INI.as_bytes()
        );
    }

    #[test]
    fn package_layout_has_nothing_to_copy() {
        let dir = tempfile::tempdir().unwrap();
        let layout = Layout::package(dir.path(), &dir.path().join("bin"));
        let plan = plan_copy(&layout).unwrap();
        assert_eq!((plan.create.len(), plan.existing.len()), (0, 0));
    }

    #[test]
    fn detects_both_layouts() {
        let (dir, _) = fake_repo();
        let root = dir.path();
        let exe = root.join("target/release/l2-admin-gui.exe");
        let layout = Layout::detect(&exe, Path::new("/nowhere")).unwrap();
        assert_eq!(layout.kind, LayoutKind::Repo);
        assert_eq!(layout.run_folder, root.join("run"));
        // Started from inside the repo with the exe elsewhere.
        let layout = Layout::detect(Path::new("/x/y/gui"), &root.join("crates")).unwrap_or_else(|| {
            // `crates` does not exist in the fake repo, ancestors still reach the root.
            panic!("cwd inside the repo must be found")
        });
        assert_eq!(layout.root, root);

        let pkg = tempfile::tempdir().unwrap();
        fs::create_dir_all(pkg.path().join("login/config")).unwrap();
        fs::create_dir_all(pkg.path().join("game/config")).unwrap();
        let layout = Layout::detect(&pkg.path().join("bin/l2-admin-gui.exe"), Path::new("/nowhere")).unwrap();
        assert_eq!(layout.kind, LayoutKind::Package);
        assert_eq!(layout.run_folder, pkg.path().join("bin"));
        assert_eq!(layout.default_datapack_value(), ".");

        let empty = tempfile::tempdir().unwrap();
        assert!(Layout::detect(&empty.path().join("a/b/c/gui"), empty.path()).is_none());
    }

    /// The real `dist/` copied to a temp run folder, edited, and read back by the server's own
    /// config loaders. Skipped when `dist/` is not there.
    #[test]
    fn the_real_dist_copies_edits_and_loads_back() {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        if !root.join("dist/login/config/Server.ini").is_file() {
            println!("skipped: dist/ not found");
            return;
        }
        let tmp = tempfile::tempdir().unwrap();
        let mut layout = Layout::repo(&root);
        layout.run_folder = tmp.path().to_path_buf();
        layout.login_dir = tmp.path().join("login");
        layout.game_dir = tmp.path().join("game");

        let plan = plan_copy(&layout).unwrap();
        assert!(plan.existing.is_empty() && plan.create.len() > 30);
        execute_copy(&plan, false).unwrap();
        assert!(layout.login_dir.join("data").is_dir());
        assert!(layout.game_dir.join("config/Custom").is_dir());

        let mut form = Form::load(&layout);
        assert_eq!(form.db_name, "l2jmobiush5");
        assert_eq!(form.datapack_root, fwd(&layout.datapack_root));
        assert!(!form.switches.is_empty(), "SideBySide.ini switches");
        form.db_host = "db.example".into();
        form.db_name = "other".into();
        form.db_password = "pw#1".into();
        form.game_port = "7778".into();
        form.server_id = "3".into();
        let before = fs::read(layout.game_config("Server.ini")).unwrap();
        apply(&layout, &form).unwrap();
        let after = fs::read(layout.game_config("Server.ini")).unwrap();
        // Same number of lines, CRLF kept: only values changed.
        let count = |b: &[u8]| b.iter().filter(|&&c| c == b'\n').count();
        assert_eq!(count(&before), count(&after));
        assert!(
            !after.windows(2).any(|w| w[0] != b'\r' && w[1] == b'\n'),
            "an LF without CR crept in"
        );

        let db =
            l2_config::DatabaseConfig::from_ini(&IniFile::load(layout.game_config("Database.ini")).unwrap());
        let url = db.jdbc_url.unwrap();
        assert_eq!(
            (url.host.as_str(), url.port, url.database.as_str()),
            ("db.example", 3306, "other")
        );
        assert_eq!(db.database_password, "pw#1");
        let server = IniFile::load(layout.game_config("Server.ini")).unwrap();
        assert_eq!(server.get_value("GameserverPort"), Some("7778"));
        assert_eq!(server.get_value("RequestServerID"), Some("3"));
        let ini = IniFile::load(layout.supervisor_ini()).unwrap();
        l2_supervisor::config::SupervisorConfig::from_ini(&ini, &layout.run_folder).unwrap();
    }

    #[test]
    fn default_url_query_is_the_java_default() {
        let dist = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../dist/login/config/Database.ini");
        let Ok(text) = fs::read_to_string(&dist) else {
            println!("skipped: dist/login/config/Database.ini not found");
            return;
        };
        let url = text
            .lines()
            .find_map(|l| l.strip_prefix("URL = "))
            .expect("URL line");
        assert_eq!(
            url,
            format!("jdbc:mysql://localhost/l2jmobiush5?{DEFAULT_URL_QUERY}")
        );
    }

    #[test]
    fn url_keeps_the_java_shape() {
        assert_eq!(
            jdbc_url(" db.local ", 3307, "l2"),
            format!("jdbc:mysql://db.local:3307/l2?{DEFAULT_URL_QUERY}")
        );
        assert!(jdbc_url("::1", 3306, "l2").starts_with("jdbc:mysql://[::1]:3306/l2?"));
        // What we write must parse back.
        let url = JdbcUrl::parse(&jdbc_url("h", 1234, "db")).unwrap();
        assert_eq!(
            (url.host.as_str(), url.port, url.database.as_str()),
            ("h", 1234, "db")
        );
    }

    #[test]
    fn apply_edits_values_and_keeps_comments_order_and_line_endings() {
        let (_dir, layout) = fake_repo();
        copied(&layout);
        let mut form = Form::load(&layout);
        form.db_host = "db.example".into();
        form.db_port = "3307".into();
        form.db_user = "l2".into();
        form.db_password = "p\\w d".into();
        form.db_name = "mydb".into();
        form.login_client_port = "2200".into();
        form.login_server_port = "9100".into();
        form.game_port = "7778".into();
        form.server_id = "3".into();
        let report = apply(&layout, &form).unwrap();
        assert!(report.supervisor.is_some());

        let db = fs::read_to_string(layout.game_config("Database.ini")).unwrap();
        assert_eq!(
            db,
            format!(
                "# db\r\nDriver = com.mysql.cj.jdbc.Driver\r\nURL = jdbc:mysql://db.example:3307/mydb?{DEFAULT_URL_QUERY}\r\nLogin = l2\r\nPassword = p\\\\w d\r\nMaximumDatabaseConnections = 5\r\n"
            )
        );
        // The login server file got the same database.
        assert_eq!(
            fs::read_to_string(layout.login_config("Database.ini")).unwrap(),
            db
        );
        // The escaped password reads back as typed.
        let ini = IniFile::load(layout.login_config("Database.ini")).unwrap();
        assert_eq!(ini.get_value("Password"), Some("p\\w d"));

        let server = fs::read_to_string(layout.game_config("Server.ini")).unwrap();
        assert!(server.contains("GameserverPort = 7778\n"));
        assert!(server.contains("RequestServerID = 3\n"));
        assert!(server.contains("LoginPort = 9100\n"));
        assert!(server.contains(&format!("DatapackRoot = {}\n", fwd(&layout.datapack_root))));
        let login = fs::read_to_string(layout.login_config("Server.ini")).unwrap();
        assert_eq!(login, "# s\nLoginserverPort = 2200\nLoginPort = 9100\n");

        // Admin: enabled, token still empty.
        let admin = fs::read_to_string(layout.game_config("Admin.ini")).unwrap();
        assert_eq!(admin, "Enabled = True\nBind = 127.0.0.1:7901\n\nToken =\n");
        // The geodata folder is the default one: GeoEngine.ini untouched.
        assert_eq!(
            fs::read_to_string(layout.game_config("GeoEngine.ini")).unwrap(),
            "# g\nGeoDataPath = ./data/geodata/\n"
        );

        // Applying the same form again changes nothing.
        let again = apply(&layout, &form).unwrap();
        assert!(again.changed.is_empty(), "{again:?}");
    }

    #[test]
    fn open_again_shows_the_current_values_and_never_the_password_in_debug() {
        let (_dir, layout) = fake_repo();
        copied(&layout);
        let mut form = Form::load(&layout);
        assert_eq!(
            (form.db_host.as_str(), form.db_port.as_str()),
            ("localhost", "3306")
        );
        form.db_host = "10.1.2.3".into();
        form.db_password = "secret-pw".into();
        form.game_port = "7800".into();
        apply(&layout, &form).unwrap();

        let loaded = Form::load(&layout);
        assert_eq!(loaded.db_host, "10.1.2.3");
        assert_eq!(loaded.db_password, "secret-pw");
        assert_eq!(loaded.game_port, "7800");
        assert!(!loaded.rewrite_supervisor);
        assert!(!format!("{loaded:?}").contains("secret-pw"));
    }

    #[test]
    fn custom_geodata_folder_is_written_as_absolute_path() {
        let (dir, layout) = fake_repo();
        copied(&layout);
        let mut form = Form::load(&layout);
        let geo = dir.path().join("my geo");
        form.geodata_dir = fwd(&geo);
        apply(&layout, &form).unwrap();
        let text = fs::read_to_string(layout.game_config("GeoEngine.ini")).unwrap();
        assert_eq!(text, format!("# g\nGeoDataPath = {}\n", fwd(&geo)));
        assert_eq!(Form::load(&layout).geodata_dir, fwd(&geo));
        // Back to the default.
        form.geodata_dir = fwd(&layout.datapack_root.join("data/geodata"));
        apply(&layout, &form).unwrap();
        assert!(
            fs::read_to_string(layout.game_config("GeoEngine.ini"))
                .unwrap()
                .contains("GeoDataPath = ./data/geodata/\n")
        );
    }

    #[test]
    fn apply_without_the_copy_writes_nothing() {
        let (_dir, layout) = fake_repo();
        let form = Form::defaults(&layout);
        assert!(matches!(apply(&layout, &form), Err(SetupError::MissingFile(_))));
        assert!(!layout.supervisor_ini().exists());
    }

    #[test]
    fn invalid_forms_list_every_problem() {
        let (_dir, layout) = fake_repo();
        let mut form = Form::defaults(&layout);
        form.db_port = "x".into();
        form.game_port = "2106".into();
        form.server_id = "0".into();
        form.db_password = " lead".into();
        form.java_enabled = true;
        let Err(problems) = form.parse() else {
            panic!("must be invalid");
        };
        assert!(problems.len() >= 5, "{problems:?}");
        assert!(matches!(apply(&layout, &form), Err(SetupError::Invalid(_))));
    }

    #[test]
    fn side_by_side_switches_come_with_help_and_only_changes_are_written() {
        let (_dir, layout) = fake_repo();
        copied(&layout);
        let mut form = Form::load(&layout);
        assert_eq!(form.switches.len(), 2);
        assert_eq!(form.switches[0].key, "DailyResetDatabaseWrites");
        assert_eq!(form.switches[0].help, "Daily reset. Set True only alone.");
        form.switches[1].value = true;
        // Not used while the Java entry is off.
        apply(&layout, &form).unwrap();
        assert!(
            fs::read_to_string(layout.game_config("SideBySide.ini"))
                .unwrap()
                .contains("MailExpiryDatabaseWrites = False")
        );
        form.java_enabled = true;
        form.java_folder = "C:/java/game".into();
        apply(&layout, &form).unwrap();
        let text = fs::read_to_string(layout.game_config("SideBySide.ini")).unwrap();
        assert!(text.contains("MailExpiryDatabaseWrites = True"));
        assert!(text.contains("DailyResetDatabaseWrites = False"));
    }

    #[test]
    fn repo_supervisor_ini_parses_and_has_no_token() {
        let (_dir, layout) = fake_repo();
        let mut form = Form::defaults(&layout);
        let text = supervisor_ini_text(&layout, &form);
        let ini = IniFile::parse("supervisor.ini", &text).unwrap();
        let config = l2_supervisor::config::SupervisorConfig::from_ini(&ini, &layout.run_folder).unwrap();
        assert_eq!(config.token, "");
        assert_eq!(config.listen.to_string(), "127.0.0.1:7900");
        let names: Vec<_> = config.servers.iter().map(|s| s.name.as_str()).collect();
        assert_eq!(names, ["login", "game"]);
        let login = &config.servers[0];
        assert!(
            login
                .command
                .replace('\\', "/")
                .contains("../target/release/login-server")
        );
        assert_eq!(
            login.working_dir.as_deref(),
            Some(layout.run_folder.join("login").as_path())
        );
        assert_eq!(login.admin_endpoint.as_deref(), Some("127.0.0.1:7901"));
        assert_eq!(config.servers[1].args, ["--root", "."]);
        assert!(!config.servers.iter().any(|s| s.autostart));
        assert!(config.servers.iter().all(|s| s.admin_token.is_empty()));

        form.java_enabled = true;
        form.java_folder = "C:\\java\\game".into();
        let text = supervisor_ini_text(&layout, &form);
        let ini = IniFile::parse("supervisor.ini", &text).unwrap();
        let config = l2_supervisor::config::SupervisorConfig::from_ini(&ini, &layout.run_folder).unwrap();
        assert_eq!(config.servers.len(), 3);
        let java = &config.servers[2];
        assert_eq!(java.name, JAVA_SERVER_NAME);
        assert_eq!(java.command, "java");
        assert_eq!(java.args, ["@java.cfg", "-jar", "../libs/GameServer.jar"]);
        assert!(text.contains("Server.javagame.WorkingDir = C:/java/game\n"));
    }

    #[test]
    fn package_supervisor_ini_points_at_the_sibling_folders() {
        let dir = tempfile::tempdir().unwrap();
        let bin = dir.path().join("bin");
        let layout = Layout::package(dir.path(), &bin);
        let text = supervisor_ini_text(&layout, &Form::defaults(&layout));
        assert!(text.contains("Server.login.WorkingDir = ../login\n"));
        assert!(text.contains("Server.game.WorkingDir = ../game\n"));
        assert!(text.contains(&format!(
            "Server.game.Command = ./game-server{}\n",
            std::env::consts::EXE_SUFFIX
        )));
    }

    #[test]
    fn rewriting_supervisor_ini_keeps_a_backup_and_the_listen_address() {
        let (_dir, layout) = fake_repo();
        copied(&layout);
        write(&layout.supervisor_ini(), "Listen = 127.0.0.1:7950\nServers =\n");
        let mut form = Form::load(&layout);
        assert!(!form.rewrite_supervisor);
        apply(&layout, &form).unwrap();
        assert_eq!(
            fs::read_to_string(layout.supervisor_ini()).unwrap(),
            "Listen = 127.0.0.1:7950\nServers =\n"
        );
        form.rewrite_supervisor = true;
        apply(&layout, &form).unwrap();
        let text = fs::read_to_string(layout.supervisor_ini()).unwrap();
        assert!(text.contains("Listen = 127.0.0.1:7950\n"));
        assert!(layout.run_folder.join("supervisor.ini.bak").is_file());
    }

    #[test]
    fn checks_report_what_is_missing_and_never_test_the_database_on_their_own() {
        let (dir, layout) = fake_repo();
        let gui_dir = dir.path().join("target/release");
        let form = Form::defaults(&layout);
        let list = run_checks(&layout, &gui_dir, &form, &DbTest::NotTested);
        let by = |label: &str| list.iter().find(|c| c.label == label).unwrap().clone();
        assert_eq!(by("Supervisor program").status, Status::Missing);
        assert_eq!(by("Login settings").status, Status::Missing);
        assert_eq!(by("Geodata").status, Status::Warn);
        assert_eq!(by("Datapack").status, Status::Ok);
        assert_eq!(by("Database").status, Status::Warn);
        assert!(!ready_to_start(&list));

        // Everything in place.
        copied(&layout);
        apply(&layout, &form).unwrap();
        for exe in [
            supervisor_exe_name().to_owned(),
            format!("login-server{}", std::env::consts::EXE_SUFFIX),
            format!("game-server{}", std::env::consts::EXE_SUFFIX),
        ] {
            write(&gui_dir.join(exe), "x");
        }
        write(&layout.datapack_root.join("data/geodata/20_18.l2j"), "x");
        let list = run_checks(
            &layout,
            &gui_dir,
            &form,
            &DbTest::Reachable {
                database_exists: true,
            },
        );
        assert!(list.iter().all(|c| c.status == Status::Ok), "{list:?}");
        assert!(ready_to_start(&list));
        let list = run_checks(
            &layout,
            &gui_dir,
            &form,
            &DbTest::Reachable {
                database_exists: false,
            },
        );
        assert!(ready_to_start(&list));
        assert_eq!(list.last().unwrap().status, Status::Warn);
        let list = run_checks(&layout, &gui_dir, &form, &DbTest::Failed("refused".into()));
        assert!(!ready_to_start(&list));
    }

    #[test]
    fn counts_only_l2j_files() {
        let dir = tempfile::tempdir().unwrap();
        write(&dir.path().join("20_18.l2j"), "");
        write(&dir.path().join("21_18.L2J"), "");
        write(&dir.path().join("readme.txt"), "");
        assert_eq!(count_geodata(dir.path()), 2);
        assert_eq!(count_geodata(&dir.path().join("none")), 0);
    }

    #[test]
    fn fwd_uses_forward_slashes_and_drops_the_verbatim_prefix() {
        assert_eq!(fwd(Path::new(r"\\?\C:\a\b")), "C:/a/b");
        assert_eq!(fwd(Path::new(r"C:\a\b")), "C:/a/b");
        assert_eq!(normalize(Path::new("a/./b/")), normalize(Path::new("a/b")));
    }
}
