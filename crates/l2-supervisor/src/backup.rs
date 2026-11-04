//! Database backups (M9-H01): `mysqldump` on a schedule and on request, gzip-compressed into a
//! folder, the newest `Keep` files kept.
//!
//! the connection settings). Java's own `BackupDatabase`/`BackupDays` keys are not used: this is
//! Rust-only and lives in `Backup.ini` next to `supervisor.ini` (D-36).
//!
//! Safety:
//! * The password is read from `Database.ini` when a backup starts, handed to `mysqldump` through
//!   the `MYSQL_PWD` environment variable and never appears on a command line, in a log line, in
//!   an error message or in `Debug` output.
//! * The tests run a stand-in binary (`backup-test-dump`) and never touch a database.
//!
//! The pieces are small functions so they can be tested alone: [`backup_file_name`],
//! [`dump_args`], [`dump_env`], [`rotate`], [`next_delay`], [`find_mysqldump`].

use std::ffi::OsString;
use std::fmt;
use std::fs;
use std::io::{self, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use chrono::{Local, NaiveDateTime, Utc};
use flate2::Compression;
use flate2::write::GzEncoder;
use l2_admin_api::{BackupDatabase, BackupInfo, BackupResult, BackupStatus};
use l2_config::{DatabaseConfig, IniFile};
use thiserror::Error;
use tokio::task::JoinHandle;

/// The settings file, next to `supervisor.ini`.
pub const BACKUP_INI: &str = "Backup.ini";
pub const DEFAULT_INTERVAL_HOURS: u32 = 24;
pub const DEFAULT_KEEP: u32 = 7;
pub const DEFAULT_FOLDER: &str = "backups";
pub const DEFAULT_DATABASE_INI: &str = "../login/config/Database.ini";
/// Backups are named `<db>-YYYYMMDD-HHMMSS` plus this.
pub const BACKUP_SUFFIX: &str = ".sql.gz";
/// A dump is written to `<name>.part` first, so a half-written file never looks like a backup.
const PART_SUFFIX: &str = ".part";
/// A scheduled run never starts sooner than this after the supervisor came up: the database
/// server may still be starting.
const MIN_FIRST_DELAY: Duration = Duration::from_secs(60);
/// How much of mysqldump's stderr goes into an error message.
const STDERR_LIMIT: usize = 2000;

#[derive(Debug, Error)]
pub enum BackupError {
    #[error("{0}")]
    Config(String),
    #[error(
        "mysqldump was not found (set MysqldumpPath in {BACKUP_INI}; looked on the PATH and in the usual MariaDB/MySQL folders)"
    )]
    NoMysqldump,
    #[error("a backup is already running")]
    AlreadyRunning,
    #[error("{0}")]
    Io(String),
    #[error("mysqldump failed ({status}): {stderr}")]
    Dump { status: String, stderr: String },
}

/// `Backup.ini`. Rust-only (D-36); a missing file means all defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackupConfig {
    /// Key `Enabled`, default False: only the schedule. "Backup now" works either way.
    pub enabled: bool,
    /// Key `IntervalHours`, default 24.
    pub interval_hours: u32,
    /// Key `Keep`, default 7: how many backup files stay.
    pub keep: u32,
    /// Key `Folder`, default `backups`, relative to the folder of `Backup.ini`.
    pub folder: PathBuf,
    /// Key `MysqldumpPath`, default empty: the `mysqldump` executable, or the folder holding it.
    /// Empty = look on the PATH, then in the usual install folders.
    pub mysqldump_path: Option<PathBuf>,
    /// Key `DatabaseIni`, default `../login/config/Database.ini`, relative to the folder of
    /// `Backup.ini`: where host, port, user, password and database name come from.
    pub database_ini: PathBuf,
}

impl BackupConfig {
    /// All defaults; relative paths are relative to `base_dir`.
    pub fn defaults(base_dir: &Path) -> Self {
        Self {
            enabled: false,
            interval_hours: DEFAULT_INTERVAL_HOURS,
            keep: DEFAULT_KEEP,
            folder: base_dir.join(DEFAULT_FOLDER),
            mysqldump_path: None,
            database_ini: base_dir.join(DEFAULT_DATABASE_INI),
        }
    }

    /// Reads `<supervisor.ini folder>/Backup.ini`; a missing file gives the defaults.
    pub fn load_next_to(supervisor_ini: &Path) -> Result<Self, BackupError> {
        let base = supervisor_ini
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or_else(|| Path::new("."));
        let path = base.join(BACKUP_INI);
        match fs::read_to_string(&path) {
            Ok(text) => {
                let ini =
                    IniFile::parse(BACKUP_INI, &text).map_err(|e| BackupError::Config(e.to_string()))?;
                Self::from_ini(&ini, base)
            }
            Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(Self::defaults(base)),
            Err(e) => Err(BackupError::Config(format!(
                "cannot read {}: {e}",
                path.display()
            ))),
        }
    }

    pub fn from_ini(ini: &IniFile, base_dir: &Path) -> Result<Self, BackupError> {
        let text = |key: &str| ini.get_value(key).map(str::trim).filter(|v| !v.is_empty());
        let count = |key: &str, default: u32| -> Result<u32, BackupError> {
            match text(key) {
                None => Ok(default),
                Some(v) => v.parse::<u32>().ok().filter(|n| *n >= 1).ok_or_else(|| {
                    BackupError::Config(format!("{key}: '{v}' is not a number of at least 1"))
                }),
            }
        };
        let resolve = |value: &str| {
            let path = Path::new(value);
            if path.is_absolute() {
                path.to_path_buf()
            } else {
                base_dir.join(path)
            }
        };
        Ok(Self {
            enabled: ini.get_bool("Enabled", false),
            interval_hours: count("IntervalHours", DEFAULT_INTERVAL_HOURS)?,
            keep: count("Keep", DEFAULT_KEEP)?,
            folder: resolve(text("Folder").unwrap_or(DEFAULT_FOLDER)),
            mysqldump_path: text("MysqldumpPath").map(resolve),
            database_ini: resolve(text("DatabaseIni").unwrap_or(DEFAULT_DATABASE_INI)),
        })
    }
}

/// The connection settings of `Database.ini`. `Debug` hides the password.
#[derive(Clone, PartialEq, Eq)]
pub struct DbSettings {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
    /// `MySqlBinLocation` of the same file: one more place to look for `mysqldump`.
    pub mysql_bin: String,
}

impl fmt::Debug for DbSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DbSettings")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("user", &self.user)
            .field("password", &"<redacted>")
            .field("database", &self.database)
            .finish()
    }
}

impl DbSettings {
    pub fn from_database_ini(path: &Path) -> Result<Self, BackupError> {
        let text = fs::read_to_string(path)
            .map_err(|e| BackupError::Config(format!("cannot read {}: {e}", path.display())))?;
        let ini = IniFile::parse("Database.ini", &text).map_err(|e| BackupError::Config(e.to_string()))?;
        Self::from_config(&DatabaseConfig::from_ini(&ini))
    }

    pub fn from_config(config: &DatabaseConfig) -> Result<Self, BackupError> {
        let url = config.jdbc_url.as_ref().ok_or_else(|| {
            BackupError::Config("Database.ini: URL is not a jdbc:mysql://host[:port]/database URL".into())
        })?;
        if url.database.is_empty() {
            return Err(BackupError::Config("Database.ini: URL names no database".into()));
        }
        Ok(Self {
            host: url.host.clone(),
            port: url.port,
            user: config.database_login.clone(),
            password: config.database_password.clone(),
            database: url.database.clone(),
            mysql_bin: config.mysql_bin_path.clone(),
        })
    }

    fn public(&self) -> BackupDatabase {
        BackupDatabase {
            host: self.host.clone(),
            port: self.port,
            user: self.user.clone(),
            database: self.database.clone(),
        }
    }
}

/// `<database>-YYYYMMDD-HHMMSS.sql.gz`. Characters of the database name that a file name may not
/// hold become `_`.
pub fn backup_file_name(database: &str, at: NaiveDateTime) -> String {
    format!(
        "{}-{}{BACKUP_SUFFIX}",
        file_prefix(database),
        at.format("%Y%m%d-%H%M%S")
    )
}

fn file_prefix(database: &str) -> String {
    database
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '$') {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// Whether `name` is `<prefix>-YYYYMMDD-HHMMSS.sql.gz` for this database.
fn is_backup_of(database: &str, name: &str) -> bool {
    let prefix = format!("{}-", file_prefix(database));
    let Some(rest) = name
        .strip_prefix(&prefix)
        .and_then(|r| r.strip_suffix(BACKUP_SUFFIX))
    else {
        return false;
    };
    let b = rest.as_bytes();
    b.len() == 15 && b[8] == b'-' && b.iter().enumerate().all(|(i, c)| i == 8 || c.is_ascii_digit())
}

/// The `mysqldump` command line, without the password.
pub fn dump_args(db: &DbSettings) -> Vec<String> {
    vec![
        "--single-transaction".into(),
        "--routines".into(),
        "--events".into(),
        format!("--host={}", db.host),
        format!("--port={}", db.port),
        format!("--user={}", db.user),
        db.database.clone(),
    ]
}

/// Environment variables for `mysqldump`: the password goes here, not on the command line
/// (a command line is visible to every user of the machine). Empty password = nothing set.
pub fn dump_env(db: &DbSettings) -> Vec<(&'static str, String)> {
    if db.password.is_empty() {
        Vec::new()
    } else {
        vec![("MYSQL_PWD", db.password.clone())]
    }
}

/// The first `mysqldump` found: `configured` (a file, or a folder holding it), else the folders in
/// `dirs` in order.
pub fn find_mysqldump(configured: Option<&Path>, dirs: &[PathBuf]) -> Option<PathBuf> {
    let exe = if cfg!(windows) {
        "mysqldump.exe"
    } else {
        "mysqldump"
    };
    if let Some(path) = configured {
        let file = if path.is_dir() {
            path.join(exe)
        } else {
            path.to_path_buf()
        };
        return file.is_file().then_some(file);
    }
    dirs.iter().map(|d| d.join(exe)).find(|f| f.is_file())
}

/// Where to look when `MysqldumpPath` is empty: the `PATH`, `MySqlBinLocation` of `Database.ini`,
/// then the usual MariaDB/MySQL/XAMPP folders on Windows.
pub fn default_search_dirs(mysql_bin: Option<&str>) -> Vec<PathBuf> {
    let mut dirs: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|p| std::env::split_paths(&p).collect())
        .unwrap_or_default();
    if let Some(bin) = mysql_bin.map(str::trim).filter(|b| !b.is_empty()) {
        dirs.push(PathBuf::from(bin));
    }
    if cfg!(windows) {
        for root in ["C:\\Program Files", "C:\\Program Files (x86)"] {
            let Ok(entries) = fs::read_dir(root) else { continue };
            let mut found: Vec<PathBuf> = entries
                .filter_map(Result::ok)
                .filter(|e| {
                    let name = e.file_name().to_string_lossy().to_ascii_lowercase();
                    name.starts_with("mariadb") || name.starts_with("mysql")
                })
                .flat_map(|e| {
                    // `MariaDB 11.4\bin`, and `MySQL\MySQL Server 8.0\bin`.
                    let top = e.path();
                    let mut bins = vec![top.join("bin")];
                    if let Ok(inner) = fs::read_dir(&top) {
                        bins.extend(inner.filter_map(Result::ok).map(|i| i.path().join("bin")));
                    }
                    bins
                })
                .collect();
            // Newest version first: "MariaDB 11.4" sorts after "MariaDB 10.6".
            found.sort();
            found.reverse();
            dirs.extend(found);
        }
        dirs.push(PathBuf::from("C:\\xampp\\mysql\\bin"));
    }
    dirs
}

/// Deletes all but the newest `keep` backups of `database` in `folder`; other files stay.
/// The timestamp in the name sorts like the time. Returns the names removed.
pub fn rotate(folder: &Path, database: &str, keep: usize) -> io::Result<Vec<String>> {
    let mut names: Vec<String> = fs::read_dir(folder)?
        .filter_map(Result::ok)
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| is_backup_of(database, n))
        .collect();
    names.sort();
    let excess = names.len().saturating_sub(keep);
    let old: Vec<String> = names.drain(..excess).collect();
    for name in &old {
        fs::remove_file(folder.join(name))?;
    }
    Ok(old)
}

/// The `*.sql.gz` files of `folder`, newest first. A missing folder is an empty list.
pub fn list_backups(folder: &Path) -> Vec<BackupInfo> {
    let Ok(entries) = fs::read_dir(folder) else {
        return Vec::new();
    };
    let mut list: Vec<BackupInfo> = entries
        .filter_map(Result::ok)
        .filter_map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let meta = e.metadata().ok()?;
            (meta.is_file() && name.ends_with(BACKUP_SUFFIX)).then(|| BackupInfo {
                name,
                size: meta.len(),
                modified_ms: meta.modified().ok().map_or(0, unix_ms),
            })
        })
        .collect();
    list.sort_by(|a, b| {
        b.modified_ms
            .cmp(&a.modified_ms)
            .then_with(|| b.name.cmp(&a.name))
    });
    list
}

fn unix_ms(time: std::time::SystemTime) -> i64 {
    time.duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

fn now_ms() -> i64 {
    Utc::now().timestamp_millis()
}

/// How long the scheduler waits before the next run: one interval after the newest backup (so a
/// restart of the supervisor does not shift the schedule), one interval from now when there is
/// none, and never less than [`MIN_FIRST_DELAY`].
pub fn next_delay(newest_backup_ms: Option<i64>, now_ms: i64, interval: Duration) -> Duration {
    let interval_ms = i64::try_from(interval.as_millis()).unwrap_or(i64::MAX);
    let due = newest_backup_ms.map_or(now_ms.saturating_add(interval_ms), |t| {
        t.saturating_add(interval_ms)
    });
    let wait = u64::try_from(due.saturating_sub(now_ms)).unwrap_or(0);
    Duration::from_millis(wait).max(MIN_FIRST_DELAY)
}

/// Runs `mysqldump` and writes its output gzip-compressed to `target` (through `<target>.part`).
/// Returns the size of the file.
fn dump_to_file(exe: &Path, db: &DbSettings, target: &Path) -> Result<u64, BackupError> {
    let io_err = |what: &str, e: io::Error| BackupError::Io(format!("{what}: {e}"));
    let mut part_name: OsString = target.as_os_str().to_owned();
    part_name.push(PART_SUFFIX);
    let part = PathBuf::from(part_name);

    let mut command = Command::new(exe);
    command
        .args(dump_args(db))
        .env_remove("MYSQL_PWD")
        .envs(dump_env(db))
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        // The supervisor runs without a console; do not flash one for the dump.
        command.creation_flags(0x0800_0000);
    }
    let mut child = command
        .spawn()
        .map_err(|e| io_err(&format!("cannot start {}", exe.display()), e))?;
    let (Some(mut stdout), Some(mut stderr)) = (child.stdout.take(), child.stderr.take()) else {
        return Err(BackupError::Io("mysqldump has no output pipes".into()));
    };
    // Read stderr on its own thread: a full pipe would block mysqldump while we wait on stdout.
    let stderr_reader = std::thread::spawn(move || {
        let mut text = String::new();
        let mut bytes = Vec::new();
        let _ = stderr.by_ref().take(64 * 1024).read_to_end(&mut bytes);
        text.push_str(&String::from_utf8_lossy(&bytes));
        text
    });

    let written = (|| -> io::Result<()> {
        let file = fs::File::create(&part)?;
        let mut gz = GzEncoder::new(BufWriter::new(file), Compression::default());
        io::copy(&mut stdout, &mut gz)?;
        gz.finish()?.flush()
    })();
    if let Err(e) = written {
        let _ = child.kill();
        let _ = child.wait();
        let _ = fs::remove_file(&part);
        return Err(io_err("cannot write the backup", e));
    }
    let status = child.wait().map_err(|e| io_err("waiting for mysqldump", e))?;
    let stderr_text = stderr_reader.join().unwrap_or_default();
    if !status.success() {
        let _ = fs::remove_file(&part);
        return Err(BackupError::Dump {
            status: status.to_string(),
            stderr: clean_stderr(&stderr_text, &db.password),
        });
    }
    fs::rename(&part, target).map_err(|e| io_err("cannot finish the backup file", e))?;
    fs::metadata(target)
        .map(|m| m.len())
        .map_err(|e| io_err("cannot read the backup file", e))
}

/// mysqldump's stderr for an error message: trimmed, shortened, password blanked out (it should
/// never be in there, but it is cheap to be sure).
fn clean_stderr(text: &str, password: &str) -> String {
    let mut text = text.trim().to_string();
    if !password.is_empty() {
        text = text.replace(password, "<redacted>");
    }
    if text.len() > STDERR_LIMIT {
        let mut cut = STDERR_LIMIT;
        while !text.is_char_boundary(cut) {
            cut -= 1;
        }
        text.truncate(cut);
        text.push_str("...");
    }
    text
}

/// One complete backup: read `Database.ini`, find `mysqldump`, dump, rotate. Blocking.
/// `now` names the file.
pub fn run_backup(config: &BackupConfig, now: NaiveDateTime) -> Result<BackupInfo, BackupError> {
    let db = DbSettings::from_database_ini(&config.database_ini)?;
    let exe = find_mysqldump(
        config.mysqldump_path.as_deref(),
        &default_search_dirs(Some(&db.mysql_bin)),
    )
    .ok_or(BackupError::NoMysqldump)?;
    fs::create_dir_all(&config.folder)
        .map_err(|e| BackupError::Io(format!("cannot create {}: {e}", config.folder.display())))?;
    let name = backup_file_name(&db.database, now);
    let size = dump_to_file(&exe, &db, &config.folder.join(&name))?;
    match rotate(&config.folder, &db.database, config.keep as usize) {
        Ok(removed) => {
            for old in removed {
                tracing::info!("backup {old} removed (Keep = {})", config.keep);
            }
        }
        Err(e) => tracing::warn!("cannot remove old backups in {}: {e}", config.folder.display()),
    }
    Ok(BackupInfo {
        name,
        size,
        modified_ms: now_ms(),
    })
}

#[derive(Default)]
struct State {
    running: bool,
    last: Option<BackupResult>,
    next_run_ms: Option<i64>,
}

/// The backup service of a supervisor: runs one backup at a time, remembers how the last one
/// ended and (when enabled) works the schedule.
pub struct BackupService {
    config: BackupConfig,
    state: Mutex<State>,
}

impl BackupService {
    pub fn new(config: BackupConfig) -> Arc<Self> {
        Arc::new(Self {
            config,
            state: Mutex::default(),
        })
    }

    pub fn config(&self) -> &BackupConfig {
        &self.config
    }

    fn state(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn list(&self) -> Vec<BackupInfo> {
        list_backups(&self.config.folder)
    }

    pub fn status(&self) -> BackupStatus {
        let state = self.state();
        BackupStatus {
            enabled: self.config.enabled,
            running: state.running,
            interval_hours: self.config.interval_hours,
            keep: self.config.keep,
            folder: self.config.folder.to_string_lossy().into_owned(),
            database: DbSettings::from_database_ini(&self.config.database_ini)
                .ok()
                .map(|d| d.public()),
            last: state.last.clone(),
            next_run_ms: if self.config.enabled {
                state.next_run_ms
            } else {
                None
            },
        }
    }

    /// Runs a backup and waits for it (the switch of M9-S01 uses this). `AlreadyRunning` when
    /// another one is in progress.
    pub async fn run_now(self: &Arc<Self>) -> Result<BackupInfo, BackupError> {
        self.claim()?;
        self.run_claimed().await
    }

    /// Marks a backup as running. Done before anything is spawned, so two quick requests cannot
    /// both pass.
    fn claim(&self) -> Result<(), BackupError> {
        let mut state = self.state();
        if state.running {
            return Err(BackupError::AlreadyRunning);
        }
        state.running = true;
        Ok(())
    }

    async fn run_claimed(self: &Arc<Self>) -> Result<BackupInfo, BackupError> {
        let config = self.config.clone();
        let result = tokio::task::spawn_blocking(move || run_backup(&config, Local::now().naive_local()))
            .await
            .unwrap_or_else(|e| Err(BackupError::Io(format!("the backup task failed: {e}"))));
        let mut state = self.state();
        state.running = false;
        state.last = Some(match &result {
            Ok(info) => {
                tracing::info!("backup written: {} ({} bytes)", info.name, info.size);
                BackupResult {
                    ok: true,
                    message: info.name.clone(),
                    finished_ms: now_ms(),
                }
            }
            Err(e) => {
                tracing::warn!("backup failed: {e}");
                BackupResult {
                    ok: false,
                    message: e.to_string(),
                    finished_ms: now_ms(),
                }
            }
        });
        result
    }

    /// Starts a backup in the background (`BackupNow`). The error is only "already running".
    pub fn start_now(self: &Arc<Self>) -> Result<(), BackupError> {
        self.claim()?;
        let me = Arc::clone(self);
        tokio::spawn(async move {
            // The result is in `status().last`.
            let _ = me.run_claimed().await;
        });
        Ok(())
    }

    /// The schedule task; `None` when `Enabled` is False.
    pub fn spawn_scheduler(self: &Arc<Self>) -> Option<JoinHandle<()>> {
        if !self.config.enabled {
            return None;
        }
        let me = Arc::clone(self);
        Some(tokio::spawn(async move {
            let interval = Duration::from_secs(u64::from(me.config.interval_hours) * 3600);
            tracing::info!(
                "database backups every {} h into {}, keeping {}",
                me.config.interval_hours,
                me.config.folder.display(),
                me.config.keep
            );
            loop {
                let newest = me.list().first().map(|b| b.modified_ms);
                let wait = next_delay(newest, now_ms(), interval);
                me.state().next_run_ms = Some(now_ms() + i64::try_from(wait.as_millis()).unwrap_or(0));
                tokio::time::sleep(wait).await;
                match me.run_now().await {
                    // A manual backup is running or just finished: look at the folder again.
                    Ok(_) | Err(BackupError::AlreadyRunning) => {}
                    // Failure is already logged and stored; try again after a full interval.
                    Err(_) => tokio::time::sleep(interval).await,
                }
            }
        }))
    }
}

#[cfg(test)]
mod tests {
    use chrono::NaiveDate;

    use super::*;

    fn at(h: u32, m: u32, s: u32) -> NaiveDateTime {
        NaiveDate::from_ymd_opt(2026, 11, 2)
            .unwrap()
            .and_hms_opt(h, m, s)
            .unwrap()
    }

    fn settings(password: &str) -> DbSettings {
        DbSettings {
            host: "db.local".into(),
            port: 3307,
            user: "root".into(),
            password: password.into(),
            database: "l2jmobiush5".into(),
            mysql_bin: String::new(),
        }
    }

    #[test]
    fn file_names_follow_the_pattern() {
        assert_eq!(
            backup_file_name("l2jmobiush5", at(3, 5, 9)),
            "l2jmobiush5-20261102-030509.sql.gz"
        );
        // Anything a file name must not hold is replaced.
        assert_eq!(
            backup_file_name("a/b c", at(0, 0, 0)),
            "a_b_c-20261102-000000.sql.gz"
        );
        assert!(is_backup_of("l2jmobiush5", "l2jmobiush5-20261102-030509.sql.gz"));
        assert!(!is_backup_of("l2jmobiush5", "other-20261102-030509.sql.gz"));
        assert!(!is_backup_of(
            "l2jmobiush5",
            "l2jmobiush5-20261102-030509.sql.gz.part"
        ));
        assert!(!is_backup_of("l2jmobiush5", "l2jmobiush5-notes.sql.gz"));
    }

    #[test]
    fn command_line_has_no_password() {
        let db = settings("s3cret!");
        let args = dump_args(&db);
        assert_eq!(
            args,
            [
                "--single-transaction",
                "--routines",
                "--events",
                "--host=db.local",
                "--port=3307",
                "--user=root",
                "l2jmobiush5"
            ]
        );
        assert!(
            !args
                .iter()
                .any(|a| a.contains("s3cret!") || a.contains("--password"))
        );
        assert_eq!(dump_env(&db), vec![("MYSQL_PWD", "s3cret!".to_string())]);
        assert!(dump_env(&settings("")).is_empty());
        assert!(!format!("{db:?}").contains("s3cret!"));
    }

    #[test]
    fn rotation_keeps_the_newest_and_other_files() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "db-20261101-010000.sql.gz",
            "db-20261102-010000.sql.gz",
            "db-20261103-010000.sql.gz",
            "db-20261104-010000.sql.gz",
            "other-20250101-010000.sql.gz",
            "notes.txt",
        ] {
            fs::write(dir.path().join(name), b"x").unwrap();
        }
        let removed = rotate(dir.path(), "db", 2).unwrap();
        assert_eq!(
            removed,
            ["db-20261101-010000.sql.gz", "db-20261102-010000.sql.gz"]
        );
        let mut left: Vec<String> = fs::read_dir(dir.path())
            .unwrap()
            .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        left.sort();
        assert_eq!(
            left,
            [
                "db-20261103-010000.sql.gz",
                "db-20261104-010000.sql.gz",
                "notes.txt",
                "other-20250101-010000.sql.gz"
            ]
        );
        assert!(rotate(dir.path(), "db", 2).unwrap().is_empty());
    }

    #[test]
    fn schedule_waits_one_interval_after_the_newest_backup() {
        let hour = Duration::from_secs(3600);
        let now = 1_000_000_000_000;
        // 10 h ago, 24 h interval: 14 h left.
        let newest = now - 10 * 3_600_000;
        assert_eq!(next_delay(Some(newest), now, 24 * hour), 14 * hour);
        // Overdue: runs soon, but not before the database had a minute to come up.
        assert_eq!(
            next_delay(Some(now - 48 * 3_600_000), now, 24 * hour),
            MIN_FIRST_DELAY
        );
        // No backup yet: a full interval.
        assert_eq!(next_delay(None, now, 24 * hour), 24 * hour);
    }

    #[test]
    fn config_defaults_and_keys() {
        let base = Path::new("base");
        let parse = |text: &str| BackupConfig::from_ini(&IniFile::parse("Backup.ini", text).unwrap(), base);
        assert_eq!(parse("").unwrap(), BackupConfig::defaults(base));
        let d = BackupConfig::defaults(base);
        assert!(!d.enabled);
        assert_eq!((d.interval_hours, d.keep), (24, 7));
        assert_eq!(d.folder, base.join("backups"));
        assert_eq!(d.database_ini, base.join("../login/config/Database.ini"));

        let c = parse("Enabled = True\nIntervalHours = 6\nKeep = 3\nFolder = dumps\nMysqldumpPath = tools\nDatabaseIni = db.ini")
            .unwrap();
        assert!(c.enabled);
        assert_eq!((c.interval_hours, c.keep), (6, 3));
        assert_eq!(c.folder, base.join("dumps"));
        assert_eq!(c.mysqldump_path, Some(base.join("tools")));
        assert_eq!(c.database_ini, base.join("db.ini"));
        assert!(parse("Keep = 0").is_err());
        assert!(parse("IntervalHours = soon").is_err());
    }

    #[test]
    fn example_config_parses() {
        let text = include_str!("../Backup.example.ini");
        let ini = IniFile::parse("Backup.ini", text).unwrap();
        let c = BackupConfig::from_ini(&ini, Path::new("base")).unwrap();
        assert_eq!(c, BackupConfig::defaults(Path::new("base")));
    }

    #[test]
    fn settings_come_from_the_jdbc_url() {
        let text = "URL = jdbc:mariadb://dbhost:3310/l2jmobiush5?useSSL=false\nLogin = gm\nPassword = pw\nMySqlBinLocation = D:/db/bin/\n";
        let ini = IniFile::parse("Database.ini", text).unwrap();
        let db = DbSettings::from_config(&DatabaseConfig::from_ini(&ini)).unwrap();
        assert_eq!((db.host.as_str(), db.port), ("dbhost", 3310));
        assert_eq!((db.user.as_str(), db.password.as_str()), ("gm", "pw"));
        assert_eq!(db.database, "l2jmobiush5");
        assert_eq!(db.mysql_bin, "D:/db/bin/");
        let no_db = IniFile::parse("Database.ini", "URL = jdbc:mysql://localhost\n").unwrap();
        assert!(DbSettings::from_config(&DatabaseConfig::from_ini(&no_db)).is_err());
    }

    #[test]
    fn mysqldump_is_looked_up_in_order() {
        let exe = if cfg!(windows) {
            "mysqldump.exe"
        } else {
            "mysqldump"
        };
        let (a, b) = (tempfile::tempdir().unwrap(), tempfile::tempdir().unwrap());
        let dirs = vec![a.path().to_path_buf(), b.path().to_path_buf()];
        assert_eq!(find_mysqldump(None, &dirs), None);
        fs::write(b.path().join(exe), b"").unwrap();
        assert_eq!(find_mysqldump(None, &dirs), Some(b.path().join(exe)));
        fs::write(a.path().join(exe), b"").unwrap();
        assert_eq!(find_mysqldump(None, &dirs), Some(a.path().join(exe)));
        // A configured folder or file wins and is not searched further.
        assert_eq!(find_mysqldump(Some(b.path()), &dirs), Some(b.path().join(exe)));
        assert_eq!(
            find_mysqldump(Some(&b.path().join(exe)), &dirs),
            Some(b.path().join(exe))
        );
        assert_eq!(find_mysqldump(Some(&b.path().join("nope")), &dirs), None);
    }

    #[test]
    fn stderr_is_trimmed_and_cleaned() {
        assert_eq!(clean_stderr("  boom pw boom\n", "pw"), "boom <redacted> boom");
        let long = "x".repeat(5000);
        assert_eq!(clean_stderr(&long, "").len(), STDERR_LIMIT + 3);
    }
}
