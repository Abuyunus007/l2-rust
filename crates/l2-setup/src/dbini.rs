//! Writes the database connection into `Database.ini` (login and game): only the values of
//! `URL`, `Login` and `Password` change; comments, order and line endings stay. The servers read
//! the file with `java.util.Properties` rules (`l2_config::IniFile`), so the values are escaped
//! the way `Properties.store` does it: a password with a `\` or a leading blank reads back as
//! typed.

use std::fs;
use std::io;
use std::path::Path;

use l2_config::{DatabaseConfig, IniFile};

/// Where a server finds its database.
#[derive(Clone, PartialEq, Eq)]
pub struct DbTarget {
    pub host: String,
    pub port: u16,
    pub user: String,
    pub password: String,
    pub database: String,
}

// The password stays out of logs and panics.
impl std::fmt::Debug for DbTarget {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("DbTarget")
            .field("host", &self.host)
            .field("port", &self.port)
            .field("user", &self.user)
            .field("password", &"<redacted>")
            .field("database", &self.database)
            .finish()
    }
}

/// The connection a `Database.ini` holds now (`None`: no file, or no usable URL).
pub fn read_database_ini(path: &Path) -> Option<DbTarget> {
    let text = fs::read_to_string(path).ok()?;
    let ini = IniFile::parse("Database.ini", &text).ok()?;
    let config = DatabaseConfig::from_ini(&ini);
    let url = config.jdbc_url?;
    Some(DbTarget {
        host: url.host,
        port: url.port,
        user: config.database_login,
        password: config.database_password,
        database: url.database,
    })
}

/// `current` (a `jdbc:mysql://host[:port]/db?params` URL) with `host`, `port` and `database`
/// replaced; the parameters after `?` are kept (the Java defaults of the shipped file). An IPv6
/// address goes into brackets.
pub fn jdbc_url(current: &str, host: &str, port: u16, database: &str) -> String {
    let params = current.split_once('?').map(|(_, p)| p).unwrap_or("");
    let host = if host.contains(':') && !host.starts_with('[') {
        format!("[{host}]")
    } else {
        host.to_owned()
    };
    let base = format!("jdbc:mysql://{host}:{port}/{database}");
    if params.is_empty() {
        base
    } else {
        format!("{base}?{params}")
    }
}

/// A value as `Properties.store` writes it: `\` and the control characters escaped, a leading
/// blank kept with `\ `.
pub fn escape_value(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for (i, c) in value.chars().enumerate() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{c}' => out.push_str("\\f"),
            ' ' if i == 0 => out.push_str("\\ "),
            c => out.push(c),
        }
    }
    out
}

/// `text` with the values of `values` set (`Key = value`, the value written as given); a key
/// that is missing is added at the end. Comments and line endings are kept.
pub fn set_keys(text: &str, values: &[(&str, String)]) -> String {
    let mut out = String::with_capacity(text.len());
    let mut seen = vec![false; values.len()];
    for line in text.split_inclusive('\n') {
        let body = line.trim_end_matches(['\r', '\n']);
        let end = &line[body.len()..];
        let key = (!body.trim_start().starts_with('#'))
            .then(|| body.split_once('=').map(|(k, _)| k.trim()))
            .flatten();
        match key.and_then(|k| values.iter().position(|(name, _)| *name == k)) {
            Some(i) => {
                out.push_str(&format!("{} = {}{end}", values[i].0, values[i].1));
                seen[i] = true;
            }
            None => out.push_str(line),
        }
    }
    for (i, (key, value)) in values.iter().enumerate() {
        if !seen[i] {
            if !out.is_empty() && !out.ends_with('\n') {
                out.push('\n');
            }
            out.push_str(&format!("{key} = {value}\n"));
        }
    }
    out
}

/// Writes `target` into the `Database.ini` at `path`.
pub fn write_database_ini(path: &Path, target: &DbTarget) -> io::Result<()> {
    let text = fs::read_to_string(path)?;
    let current_url = text
        .lines()
        .find_map(|l| {
            let (k, v) = l.split_once('=')?;
            (k.trim() == "URL").then(|| v.trim().to_owned())
        })
        .unwrap_or_default();
    let url = jdbc_url(&current_url, &target.host, target.port, &target.database);
    let text = set_keys(
        &text,
        &[
            ("URL", escape_value(&url)),
            ("Login", escape_value(&target.user)),
            ("Password", escape_value(&target.password)),
        ],
    );
    fs::write(path, text)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHIPPED: &str = "# Database settings\r\nDriver = com.mysql.cj.jdbc.Driver\r\n\
URL = jdbc:mysql://localhost/l2jmobiush5?useUnicode=true&characterEncoding=utf-8\r\n\
# The user\r\nLogin = root\r\nPassword = \r\nMaximumDatabaseConnections = 5\r\n";

    fn target(host: &str, password: &str) -> DbTarget {
        DbTarget {
            host: host.to_owned(),
            port: 3306,
            user: "l2".to_owned(),
            password: password.to_owned(),
            database: "my_h5".to_owned(),
        }
    }

    #[test]
    fn the_url_keeps_its_parameters_and_brackets_ipv6() {
        assert_eq!(
            jdbc_url(
                "jdbc:mysql://localhost/l2jmobiush5?useUnicode=true&x=1",
                "db.lan",
                3307,
                "h5"
            ),
            "jdbc:mysql://db.lan:3307/h5?useUnicode=true&x=1"
        );
        assert_eq!(
            jdbc_url("", "localhost", 3306, "l2"),
            "jdbc:mysql://localhost:3306/l2"
        );
        assert_eq!(jdbc_url("", "::1", 3306, "l2"), "jdbc:mysql://[::1]:3306/l2");
    }

    #[test]
    fn only_the_values_change() {
        let text = set_keys(
            SHIPPED,
            &[("Login", "l2".to_owned()), ("Password", "s=cret".to_owned())],
        );
        assert_eq!(
            text,
            "# Database settings\r\nDriver = com.mysql.cj.jdbc.Driver\r\n\
URL = jdbc:mysql://localhost/l2jmobiush5?useUnicode=true&characterEncoding=utf-8\r\n\
# The user\r\nLogin = l2\r\nPassword = s=cret\r\nMaximumDatabaseConnections = 5\r\n"
        );
        // A key the file does not have is added; a commented one is not taken for it.
        let text = set_keys("# Login = x\nA = 1", &[("Login", "y".to_owned())]);
        assert_eq!(text, "# Login = x\nA = 1\nLogin = y\n");
    }

    #[test]
    fn what_is_written_reads_back_as_typed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("Database.ini");
        for (host, password) in [
            ("127.0.0.1", "pw"),
            ("db.lan", "Pa\\ss1"),
            ("::1", " lead and trail "),
            ("localhost", "C:\\u00x=:#!"),
            ("localhost", ""),
        ] {
            fs::write(&path, SHIPPED).unwrap();
            let written = target(host, password);
            write_database_ini(&path, &written).unwrap();
            assert_eq!(read_database_ini(&path), Some(written), "{password:?}");
        }
        let text = fs::read_to_string(&path).unwrap();
        assert!(text.contains("?useUnicode=true&characterEncoding=utf-8\r\n"));
        assert!(!format!("{:?}", target("h", "secret")).contains("secret"));
    }
}
