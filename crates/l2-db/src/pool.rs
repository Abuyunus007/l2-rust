//! Connection pool.
// Java: org/l2jmobius/commons/database/DatabaseFactory.java

use std::time::Duration;

use l2_config::DatabaseConfig;
use sqlx::mysql::{MySqlConnectOptions, MySqlPoolOptions, MySqlSslMode};
use tracing::info;

use crate::error::{DbError, Result};

/// The pool every repository works on.
pub type Pool = sqlx::MySqlPool;

/// Java `Math.clamp(configuredMax, 4, 1000)`.
const MIN_POOL_SIZE: i32 = 4;
const MAX_POOL_SIZE: i32 = 1000;
/// Java `setConnectionTimeout(60000)`.
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(60);
/// Java `setIdleTimeout(300000)`.
const IDLE_TIMEOUT: Duration = Duration::from_secs(300);
/// Java `setMaxLifetime(600000)`.
const MAX_LIFETIME: Duration = Duration::from_secs(600);

/// Java `determineMaxPoolSize`.
pub fn pool_max_size(configured_max: i32) -> u32 {
    // The clamp keeps the value in 4..=1000, so the cast cannot truncate or go negative.
    configured_max.clamp(MIN_POOL_SIZE, MAX_POOL_SIZE) as u32
}

/// Java `determineMinimumIdle`: a tenth of the maximum, at least 2.
pub fn pool_min_idle(configured_max: i32) -> u32 {
    (pool_max_size(configured_max) / 10).max(2)
}

/// Builds the sqlx connect options from `Database.ini`.
///
/// Host, port and database come from the parsed JDBC URL; login and password from their own
/// keys, like Hikari does. JDBC URL parameters are not translated, except `useSSL=false`
/// (no TLS), because the Mobius default URL carries it and sqlx would otherwise try TLS first.
// Java: DatabaseFactory.init (setJdbcUrl / setUsername / setPassword)
pub fn connect_options(config: &DatabaseConfig) -> Result<MySqlConnectOptions> {
    let url = config
        .jdbc_url
        .as_ref()
        .ok_or_else(|| DbError::InvalidUrl(config.redacted_url()))?;
    let mut options = MySqlConnectOptions::new()
        .host(&url.host)
        .port(url.port)
        .username(&config.database_login)
        .password(&config.database_password)
        .charset("utf8mb4");
    if !url.database.is_empty() {
        options = options.database(&url.database);
    }
    if url
        .param("useSSL")
        .is_some_and(|v| v.eq_ignore_ascii_case("false"))
    {
        options = options.ssl_mode(MySqlSslMode::Disabled);
    }
    Ok(options)
}

/// Pool settings of `DatabaseFactory` on top of the given connect options.
// Java: DatabaseFactory.init (pool size and timeouts)
pub async fn connect_with(options: MySqlConnectOptions, configured_max: i32) -> Result<Pool> {
    let pool = MySqlPoolOptions::new()
        .max_connections(pool_max_size(configured_max))
        .min_connections(pool_min_idle(configured_max))
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .idle_timeout(IDLE_TIMEOUT)
        .max_lifetime(MAX_LIFETIME)
        .connect_with(options)
        .await?;
    Ok(pool)
}

/// Opens the pool from `Database.ini` and checks that one connection works.
///
/// Java starts even when the database is down (`setInitializationFailTimeout(-1)`) and only
/// logs; here a failure is returned and the server decides.
// Java: DatabaseFactory.init + testSingleConnection
pub async fn connect(config: &DatabaseConfig) -> Result<Pool> {
    let options = connect_options(config)?;
    let pool = connect_with(options, config.database_max_connections).await?;
    info!(
        "Database: connection pool ready ({} connections max, {}).",
        pool.options().get_max_connections(),
        config.redacted_url()
    );
    Ok(pool)
}

#[cfg(test)]
mod tests {
    use l2_config::IniFile;

    use super::*;

    fn config(text: &str) -> DatabaseConfig {
        DatabaseConfig::from_ini(&IniFile::parse("Database.ini", text).unwrap())
    }

    #[test]
    fn pool_size_is_clamped_like_java() {
        assert_eq!(pool_max_size(10), 10);
        assert_eq!(pool_max_size(0), 4);
        assert_eq!(pool_max_size(-5), 4);
        assert_eq!(pool_max_size(5000), 1000);
        assert_eq!(pool_min_idle(10), 2);
        assert_eq!(pool_min_idle(100), 10);
        assert_eq!(pool_min_idle(5000), 100);
        assert_eq!(pool_min_idle(1), 2);
    }

    #[test]
    fn options_come_from_the_jdbc_url_and_login() {
        let cfg =
            config("URL = jdbc:mysql://db.example:3307/l2test?useSSL=false\nLogin = gs\nPassword = secret\n");
        let options = connect_options(&cfg).unwrap();
        assert_eq!(options.get_host(), "db.example");
        assert_eq!(options.get_port(), 3307);
        assert_eq!(options.get_database(), Some("l2test"));
        assert_eq!(options.get_username(), "gs");
        assert!(matches!(options.get_ssl_mode(), MySqlSslMode::Disabled));
    }

    #[test]
    fn default_url_gives_default_port_and_tls_preferred() {
        let options = connect_options(&config("")).unwrap();
        assert_eq!(options.get_host(), "localhost");
        assert_eq!(options.get_port(), 3306);
        assert_eq!(options.get_database(), Some("l2jmobius"));
        assert_eq!(options.get_username(), "root");
        assert!(matches!(options.get_ssl_mode(), MySqlSslMode::Preferred));
    }

    #[test]
    fn bad_url_is_an_error_without_the_password() {
        let cfg = config("URL = not-a-url?password=hunter2\n");
        let err = connect_options(&cfg).unwrap_err();
        assert!(matches!(err, DbError::InvalidUrl(_)));
        assert!(!err.to_string().contains("hunter2"));
    }
}
