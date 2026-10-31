//! A throwaway database with the login schema, for integration tests.
//!
//! Gated on `L2_TEST_DATABASE_URL` (CI's MariaDB service, e.g. `mysql://root@127.0.0.1:3306/`).
//! Without it [`TestDb::create`] returns `None` and prints a skip notice, so a test starts with
//! `let Some(db) = TestDb::create("tag").await else { return };`.
//!
//! **Never point the URL at a server with real data.** Every `TestDb` creates its own database
//! named `l2_test_<tag>_<pid>_<nanos>` and drops only that one, but it needs the right to create
//! databases.

use std::path::Path;
use std::str::FromStr;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use sqlx::mysql::{MySqlConnectOptions, MySqlConnection};
use sqlx::{AssertSqlSafe, Connection};

/// The login schema, including the sample row of game server 2 in `gameservers`.
const SCHEMA: [&str; 4] = [
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/db_installer/sql/login/account_data.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/db_installer/sql/login/accounts.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/db_installer/sql/login/accounts_ipauth.sql"
    )),
    include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/db_installer/sql/login/gameservers.sql"
    )),
];

/// The hex id of the sample row in `gameservers.sql` (game server 2), as Java prints it.
pub const SAMPLE_HEXID: &str = "-2ad66b3f483c22be097019f55c8abdf0";
/// The id of the sample game server.
pub const SAMPLE_SERVER_ID: i32 = 2;

/// `Server.ini` for tests: port 0 for both listeners (never fight over 2106 / 9013), no flood
/// protection (tests reconnect quickly), accounts are created on first login.
pub const DEFAULT_SERVER_INI: &str = "LoginserverHostname = 127.0.0.1\nLoginserverPort = 0\nLoginPort = 0\n\
     EnableFloodProtection = False\nAutoCreateAccounts = True\nShowLicence = True\n";

/// `data/servername.xml` for tests.
pub const SERVER_NAMES_XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<servers_list>
	<server id="1" name="Bartz" />
	<server id="2" name="Sieghardt" />
	<server id="3" name="Kain" />
</servers_list>"#;

/// Makes names unique when tests of one process start within the same nanosecond.
static NEXT: AtomicU32 = AtomicU32::new(0);

/// A database with the login schema, dropped by [`TestDb::drop_db`].
pub struct TestDb {
    url: String,
    name: String,
}

impl TestDb {
    /// Creates `l2_test_<tag>_...` with the login schema, or `None` when
    /// `L2_TEST_DATABASE_URL` is not set (the test is skipped).
    pub async fn create(tag: &str) -> Option<Self> {
        let url = std::env::var("L2_TEST_DATABASE_URL")
            .ok()
            .filter(|u| !u.trim().is_empty());
        let Some(url) = url else {
            eprintln!("skipped: L2_TEST_DATABASE_URL is not set");
            return None;
        };
        let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let name = format!(
            "l2_test_{tag}_{}_{}_{}",
            std::process::id(),
            nanos % 1_000_000_000,
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        assert!(name.starts_with("l2_test_"));
        let mut admin = MySqlConnection::connect_with(&MySqlConnectOptions::from_str(&url).unwrap())
            .await
            .unwrap();
        sqlx::query(AssertSqlSafe(format!(
            "CREATE DATABASE `{name}` CHARACTER SET utf8mb4 COLLATE utf8mb4_unicode_ci"
        )))
        .execute(&mut admin)
        .await
        .unwrap();
        let mut setup = Self::connect_to(&url, &name).await;
        for file in SCHEMA {
            for statement in file.split(';').map(str::trim).filter(|s| !s.is_empty()) {
                sqlx::query(AssertSqlSafe(statement))
                    .execute(&mut setup)
                    .await
                    .unwrap();
            }
        }
        setup.close().await.unwrap();
        Some(Self { url, name })
    }

    async fn connect_to(url: &str, name: &str) -> MySqlConnection {
        MySqlConnection::connect_with(&MySqlConnectOptions::from_str(url).unwrap().database(name))
            .await
            .unwrap()
    }

    /// The name of the throwaway database.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// A new connection to the throwaway database (for assertions on stored rows).
    pub async fn connection(&self) -> MySqlConnection {
        Self::connect_to(&self.url, &self.name).await
    }

    /// Runs one statement.
    pub async fn exec(&self, sql: &str) {
        let mut conn = self.connection().await;
        sqlx::query(AssertSqlSafe(sql.to_owned()))
            .execute(&mut conn)
            .await
            .unwrap();
    }

    /// Drops the database. Close every pool and connection to it first.
    pub async fn drop_db(self) {
        let mut admin = MySqlConnection::connect_with(&MySqlConnectOptions::from_str(&self.url).unwrap())
            .await
            .unwrap();
        sqlx::query(AssertSqlSafe(format!("DROP DATABASE `{}`", self.name)))
            .execute(&mut admin)
            .await
            .unwrap();
    }

    /// `(user, password, host, port)` of the test URL.
    pub fn parts(&self) -> (String, String, String, u16) {
        parse_url(&self.url)
    }

    /// Writes a server folder like `dist/login` into `root`: `config/Database.ini` (pointing at
    /// this database), `config/Server.ini` with `server_ini` as its content, and
    /// `data/servername.xml` ([`SERVER_NAMES_XML`]). The folder is then used with
    /// `Paths::resolve(root, "./config", "./data")`.
    pub fn write_server_folder(&self, root: &Path, server_ini: &str) {
        let (user, password, host, port) = self.parts();
        let config = root.join("config");
        let data = root.join("data");
        std::fs::create_dir_all(&config).unwrap();
        std::fs::create_dir_all(&data).unwrap();
        std::fs::write(
            config.join("Database.ini"),
            format!(
                "URL = jdbc:mysql://{host}:{port}/{}?useSSL=false\nLogin = {user}\nPassword = {password}\n",
                self.name
            ),
        )
        .unwrap();
        std::fs::write(config.join("Server.ini"), server_ini).unwrap();
        std::fs::write(data.join("servername.xml"), SERVER_NAMES_XML).unwrap();
    }
}

/// `(user, password, host, port)` of a `mysql://user[:password]@host[:port]/` URL.
fn parse_url(url: &str) -> (String, String, String, u16) {
    let rest = url.strip_prefix("mysql://").expect("mysql:// URL");
    let rest = rest.split('/').next().unwrap();
    let (credentials, host_port) = rest.rsplit_once('@').unwrap_or(("root", rest));
    let (user, password) = credentials.split_once(':').unwrap_or((credentials, ""));
    let (host, port) = host_port.split_once(':').unwrap_or((host_port, "3306"));
    (
        user.to_owned(),
        password.to_owned(),
        host.to_owned(),
        port.parse().unwrap(),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn urls_are_split_into_their_parts() {
        assert_eq!(
            parse_url("mysql://root@127.0.0.1:3306/"),
            ("root".into(), String::new(), "127.0.0.1".into(), 3306)
        );
        assert_eq!(
            parse_url("mysql://ci:secret@db.example:3307/x"),
            ("ci".into(), "secret".into(), "db.example".into(), 3307)
        );
        assert_eq!(
            parse_url("mysql://localhost/"),
            ("root".into(), String::new(), "localhost".into(), 3306)
        );
    }

    #[test]
    fn the_server_folder_points_at_the_database() {
        let db = TestDb {
            url: "mysql://root@127.0.0.1:3306/".into(),
            name: "l2_test_unit".into(),
        };
        let root = std::env::temp_dir().join(format!("l2_testkit_folder_{}", std::process::id()));
        db.write_server_folder(&root, DEFAULT_SERVER_INI);
        let database_ini = std::fs::read_to_string(root.join("config/Database.ini")).unwrap();
        assert!(database_ini.contains("jdbc:mysql://127.0.0.1:3306/l2_test_unit?"));
        assert!(root.join("data/servername.xml").exists());
        let _ = std::fs::remove_dir_all(&root);
    }
}
