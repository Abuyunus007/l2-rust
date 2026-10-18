//! `Database.ini`.
// Java: org/l2jmobius/commons/config/DatabaseConfig.java

use std::fmt;

use tracing::warn;

use crate::IniFile;

/// File name inside the config directory.
pub const DATABASE_CONFIG_FILE: &str = "Database.ini";

/// Default port of a JDBC MySQL URL without an explicit port.
const DEFAULT_MYSQL_PORT: u16 = 3306;

/// Database connection settings.
///
/// `Debug` never prints the password (the repo is public and logs get shared).
#[derive(Clone)]
pub struct DatabaseConfig {
    /// Key: `Driver`, default `com.mysql.cj.jdbc.Driver`. Unused by Rust (sqlx picks the driver).
    pub database_driver: String,
    /// Key: `URL`, default `jdbc:mysql://localhost/l2jmobius`. The raw text; may contain
    /// credentials in its parameters, so show it only via [`Self::redacted_url`].
    pub database_url: String,
    /// `database_url` split into parts; `None` (plus a warning) if it is not a
    /// `jdbc:mysql://host[:port]/db[?params]` URL.
    pub jdbc_url: Option<JdbcUrl>,
    /// Key: `Login`, default `root`.
    pub database_login: String,
    /// Key: `Password`, default empty.
    pub database_password: String,
    /// Key: `MaximumDatabaseConnections`, default 10.
    pub database_max_connections: i32,
    /// Key: `TestDatabaseConnections`, default false.
    pub database_test_connections: bool,
    /// Key: `BackupDatabase`, default false. Backups are not ported yet.
    pub backup_database: bool,
    /// Key: `MySqlBinLocation`, default `C:/xampp/mysql/bin/`. Unused for now.
    pub mysql_bin_path: String,
    /// Key: `BackupPath`, default `../backup/`. Unused for now.
    pub backup_path: String,
    /// Key: `BackupDays`, default 30. Unused for now.
    pub backup_days: i32,
}

impl DatabaseConfig {
    /// Reads the values like `DatabaseConfig.load()`.
    pub fn from_ini(ini: &IniFile) -> Self {
        let database_url = ini.get_string("URL", "jdbc:mysql://localhost/l2jmobius");
        let jdbc_url = JdbcUrl::parse(&database_url);
        if jdbc_url.is_none() {
            warn!(
                "Config 'URL' in file '{}' is not a jdbc:mysql://host[:port]/database URL.",
                ini.file_name()
            );
        }
        Self {
            database_driver: ini.get_string("Driver", "com.mysql.cj.jdbc.Driver"),
            database_url,
            jdbc_url,
            database_login: ini.get_string("Login", "root"),
            database_password: ini.get_string("Password", ""),
            database_max_connections: ini.get_int("MaximumDatabaseConnections", 10),
            database_test_connections: ini.get_bool("TestDatabaseConnections", false),
            backup_database: ini.get_bool("BackupDatabase", false),
            mysql_bin_path: ini.get_string("MySqlBinLocation", "C:/xampp/mysql/bin/"),
            backup_path: ini.get_string("BackupPath", "../backup/"),
            backup_days: ini.get_int("BackupDays", 30),
        }
    }

    /// The URL with the value of every `password` / `user` parameter masked, safe to log.
    pub fn redacted_url(&self) -> String {
        match self.database_url.split_once('?') {
            None => self.database_url.clone(),
            Some((base, query)) => {
                let params: Vec<String> = query
                    .split('&')
                    .map(|pair| match pair.split_once('=') {
                        Some((k, _)) if is_secret_param(k) => format!("{k}=***"),
                        _ => pair.to_owned(),
                    })
                    .collect();
                format!("{base}?{}", params.join("&"))
            }
        }
    }
}

impl fmt::Debug for DatabaseConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DatabaseConfig")
            .field("database_driver", &self.database_driver)
            .field("database_url", &self.redacted_url())
            .field("jdbc_url", &self.jdbc_url)
            .field("database_login", &self.database_login)
            .field("database_password", &"<redacted>")
            .field("database_max_connections", &self.database_max_connections)
            .field("database_test_connections", &self.database_test_connections)
            .field("backup_database", &self.backup_database)
            .field("mysql_bin_path", &self.mysql_bin_path)
            .field("backup_path", &self.backup_path)
            .field("backup_days", &self.backup_days)
            .finish()
    }
}

fn is_secret_param(key: &str) -> bool {
    key.eq_ignore_ascii_case("password") || key.eq_ignore_ascii_case("user")
}

/// A parsed `jdbc:mysql://host[:port]/database[?key=value&key=value]` URL.
#[derive(Clone, PartialEq, Eq)]
pub struct JdbcUrl {
    /// Host name or address; an IPv6 literal without its brackets.
    pub host: String,
    /// Port, 3306 when the URL has none.
    pub port: u16,
    /// Database (schema) name; empty when the URL has none.
    pub database: String,
    /// Query parameters in file order. Split at the first `=` of each `&`-separated piece, so
    /// `sessionVariables=wait_timeout=600,interactive_timeout=600` keeps its inner `=`. A piece
    /// without `=` has an empty value. No percent-decoding.
    pub params: Vec<(String, String)>,
}

impl JdbcUrl {
    /// Parses the URL; `None` if it does not have the expected shape.
    ///
    /// Accepts the scheme `jdbc:mysql:` and `jdbc:mariadb:` (case-insensitive).
    pub fn parse(url: &str) -> Option<Self> {
        let rest = strip_scheme(url)?;
        let (authority_and_path, query) = match rest.split_once('?') {
            Some((a, q)) => (a, Some(q)),
            None => (rest, None),
        };
        let (authority, database) = authority_and_path
            .split_once('/')
            .unwrap_or((authority_and_path, ""));
        let (host, port) = split_host_port(authority)?;
        let params = query
            .map(|q| {
                q.split('&')
                    .filter(|p| !p.is_empty())
                    .map(|p| match p.split_once('=') {
                        Some((k, v)) => (k.to_owned(), v.to_owned()),
                        None => (p.to_owned(), String::new()),
                    })
                    .collect()
            })
            .unwrap_or_default();
        Some(Self {
            host,
            port,
            database: database.to_owned(),
            params,
        })
    }

    /// The first parameter called `key` (case-sensitive).
    pub fn param(&self, key: &str) -> Option<&str> {
        self.params
            .iter()
            .find(|(k, _)| k == key)
            .map(|(_, v)| v.as_str())
    }
}

impl fmt::Debug for JdbcUrl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Parameter values can hold credentials (`user=`, `password=`).
        let params: Vec<(&str, &str)> = self
            .params
            .iter()
            .map(|(k, v)| {
                if is_secret_param(k) {
                    (k.as_str(), "***")
                } else {
                    (k.as_str(), v.as_str())
                }
            })
            .collect();
        f.debug_struct("JdbcUrl")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("database", &self.database)
            .field("params", &params)
            .finish()
    }
}

fn strip_scheme(url: &str) -> Option<&str> {
    ["jdbc:mysql://", "jdbc:mariadb://"].iter().find_map(|scheme| {
        let head = url.get(..scheme.len())?;
        head.eq_ignore_ascii_case(scheme).then(|| &url[scheme.len()..])
    })
}

fn split_host_port(authority: &str) -> Option<(String, u16)> {
    if let Some(bracketed) = authority.strip_prefix('[') {
        // IPv6 literal: `[::1]` or `[::1]:3307`
        let (host, after) = bracketed.split_once(']')?;
        let port = match after {
            "" => DEFAULT_MYSQL_PORT,
            _ => after.strip_prefix(':')?.parse().ok()?,
        };
        return (!host.is_empty()).then(|| (host.to_owned(), port));
    }
    let (host, port) = match authority.split_once(':') {
        Some((host, port)) => (host, port.parse().ok()?),
        None => (authority, DEFAULT_MYSQL_PORT),
    };
    (!host.is_empty()).then(|| (host.to_owned(), port))
}
