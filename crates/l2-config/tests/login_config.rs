#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Tests of the login server config structs (`Server.ini`, `Database.ini`, `Threads.ini`,
//! `Network.ini`, `Interface.ini`).

use std::path::{Path, PathBuf};

use l2_config::{
    ConfigError, DatabaseConfig, IniFile, InterfaceConfig, JdbcUrl, LoginConfig, NetworkConfig, ThreadConfig,
};

const PROCESSORS: i32 = 8;

fn dist_login_config() -> PathBuf {
    std::env::var_os("L2JMOBIUS_DIST")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist"))
        .join("login")
        .join("config")
}

fn ini(text: &str) -> IniFile {
    IniFile::parse("test.ini", text).unwrap()
}

fn write(dir: &Path, name: &str, text: &str) {
    std::fs::write(dir.join(name), text).unwrap();
}

#[test]
fn empty_dir_gives_all_java_defaults() {
    let dir = tempfile::tempdir().unwrap();
    let set = LoginConfig::load_with_processors(dir.path(), PROCESSORS).unwrap();

    let login = &set.login;
    assert_eq!(login.game_server_login_host, "127.0.0.1");
    assert_eq!(login.game_server_login_port, 9013);
    assert_eq!(login.login_bind_address, "0.0.0.0");
    assert_eq!(login.port_login, 2106);
    assert_eq!(login.datapack_root, std::env::current_dir().unwrap());
    assert!(login.accept_new_gameserver);
    assert_eq!(login.login_try_before_ban, 5);
    assert_eq!(login.login_block_after_ban, 900);
    assert!(!login.login_server_schedule_restart);
    assert_eq!(login.login_server_schedule_restart_time, 24);
    assert!(login.show_licence);
    assert!(login.auto_create_accounts);
    assert!(login.flood_protection);
    assert_eq!(login.fast_connection_limit, 15);
    assert_eq!(login.normal_connection_time, 700);
    assert_eq!(login.fast_connection_time, 350);
    assert_eq!(login.max_connection_per_ip, 50);

    let db = &set.database;
    assert_eq!(db.database_driver, "com.mysql.cj.jdbc.Driver");
    assert_eq!(db.database_url, "jdbc:mysql://localhost/l2jmobius");
    assert_eq!(
        db.jdbc_url,
        Some(JdbcUrl {
            host: "localhost".into(),
            port: 3306,
            database: "l2jmobius".into(),
            params: vec![],
        })
    );
    assert_eq!(db.database_login, "root");
    assert_eq!(db.database_password, "");
    assert_eq!(db.database_max_connections, 10);
    assert!(!db.database_test_connections);
    assert!(!db.backup_database);
    assert_eq!(db.mysql_bin_path, "C:/xampp/mysql/bin/");
    assert_eq!(db.backup_path, "../backup/");
    assert_eq!(db.backup_days, 30);

    let threads = &set.threads;
    assert_eq!(threads.scheduled_thread_pool_size, 32);
    assert_eq!(threads.instant_thread_pool_size, 16);
    assert_eq!(threads.high_priority_scheduled_thread_pool_size, 8);
    assert!(!threads.threads_for_loading);

    let net = &set.network;
    assert_eq!(net.shutdown_wait_time, 5000);
    assert_eq!(net.thread_pool_size, 2);
    assert_eq!(net.thread_priority, 5);
    assert!(net.auto_expand_pool_capacity);
    assert_eq!(net.init_buffer_pool_factor, 0.0);
    assert!(!net.drop_packets);
    assert_eq!(net.drop_packet_threshold, 250);
    assert_eq!(net.buffer_segment_size, 64);

    assert!(set.interface.enable_gui);
    assert!(set.interface.dark_theme);
}

#[test]
fn loads_the_java_distribution_files() {
    let dir = dist_login_config();
    if !dir.is_dir() {
        eprintln!("skipped: {} does not exist", dir.display());
        return;
    }
    let set = LoginConfig::load_with_processors(&dir, PROCESSORS).unwrap();

    // Values taken from dist/login/config; LoginPort differs from the Java default (9013).
    assert_eq!(set.login.game_server_login_host, "127.0.0.1");
    assert_eq!(set.login.game_server_login_port, 9014);
    assert_eq!(set.login.login_bind_address, "0.0.0.0");
    assert_eq!(set.login.port_login, 2106);
    assert_eq!(set.login.login_try_before_ban, 5);
    assert_eq!(set.login.login_block_after_ban, 900);
    assert!(set.login.accept_new_gameserver);
    assert!(set.login.flood_protection);
    assert_eq!(set.login.max_connection_per_ip, 50);
    assert_eq!(set.login.login_server_schedule_restart_time, 24);

    let jdbc = set.database.jdbc_url.as_ref().expect("dist URL parses");
    assert_eq!(jdbc.host, "localhost");
    assert_eq!(jdbc.port, 3306);
    assert_eq!(jdbc.database, "l2jmobiush5");
    assert_eq!(jdbc.param("useSSL"), Some("false"));
    assert_eq!(jdbc.param("connectTimeout"), Some("10000"));
    assert_eq!(
        jdbc.param("sessionVariables"),
        Some("wait_timeout=600,interactive_timeout=600")
    );
    assert_eq!(set.database.database_login, "root");
    assert_eq!(set.database.database_max_connections, 5);

    // Both pool sizes are 2, so there is no high priority pool.
    assert_eq!(set.threads.scheduled_thread_pool_size, 2);
    assert_eq!(set.threads.instant_thread_pool_size, 2);
    assert_eq!(set.threads.high_priority_scheduled_thread_pool_size, 0);

    assert_eq!(set.network.thread_pool_size, 4);
    assert_eq!(set.network.shutdown_wait_time, 5000);
    assert_eq!(set.network.drop_packet_threshold, 250);

    assert!(set.interface.enable_gui);
}

#[test]
fn file_values_override_defaults() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "Server.ini",
        "LoginHostname=10.0.0.1\nLoginPort=1\nLoginserverHostname=1.2.3.4\nLoginserverPort=2\n\
         AcceptNewGameServer=false\nLoginTryBeforeBan=3\nLoginBlockAfterBan=60\n\
         LoginRestartSchedule=true\nLoginRestartTime=12\nShowLicence=false\n\
         AutoCreateAccounts=false\nEnableFloodProtection=false\nFastConnectionLimit=1\n\
         NormalConnectionTime=2\nFastConnectionTime=3\nMaxConnectionPerIP=4\n",
    );
    let set = LoginConfig::load_with_processors(dir.path(), PROCESSORS).unwrap();
    let l = &set.login;
    assert_eq!(l.game_server_login_host, "10.0.0.1");
    assert_eq!(l.game_server_login_port, 1);
    assert_eq!(l.login_bind_address, "1.2.3.4");
    assert_eq!(l.port_login, 2);
    assert!(!l.accept_new_gameserver);
    assert_eq!(l.login_try_before_ban, 3);
    assert_eq!(l.login_block_after_ban, 60);
    assert!(l.login_server_schedule_restart);
    assert_eq!(l.login_server_schedule_restart_time, 12);
    assert!(!l.show_licence);
    assert!(!l.auto_create_accounts);
    assert!(!l.flood_protection);
    assert_eq!(l.fast_connection_limit, 1);
    assert_eq!(l.normal_connection_time, 2);
    assert_eq!(l.fast_connection_time, 3);
    assert_eq!(l.max_connection_per_ip, 4);
}

#[test]
fn reader_quirks_carry_over_to_the_structs() {
    let dir = tempfile::tempdir().unwrap();
    // Trailing blank: the int does not parse, so the default stays. A bad boolean is false,
    // not the default (true).
    write(
        dir.path(),
        "Server.ini",
        "LoginserverPort = 7777 \nShowLicence = yes\nAcceptNewGameServer=True\n",
    );
    let set = LoginConfig::load_with_processors(dir.path(), PROCESSORS).unwrap();
    assert_eq!(set.login.port_login, 2106);
    assert!(!set.login.show_licence);
    assert!(set.login.accept_new_gameserver);
}

#[test]
fn malformed_unicode_escape_is_an_error() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Database.ini", "Login=\\u12\n");
    assert_eq!(
        LoginConfig::load_with_processors(dir.path(), PROCESSORS).unwrap_err(),
        ConfigError::MalformedUnicodeEscape {
            file: "Database.ini".into()
        }
    );
}

#[test]
fn datapack_root_is_absolute_and_normalized() {
    let cwd = std::env::current_dir().unwrap();
    let root = |value: &str| LoginConfig::from_ini(&ini(&format!("DatapackRoot={value}"))).datapack_root;

    assert_eq!(root("."), cwd);
    assert_eq!(root("./data/../data/./x"), cwd.join("data").join("x"));
    // Backslashes are converted before the path is resolved (Windows-style config on Linux too).
    // The .ini escape rules apply first: two backslashes are one, a single backslash before an
    // `s` is dropped.
    assert_eq!(root(r"data\\sub"), cwd.join("data").join("sub"));
    assert_eq!(root(r"data\sub"), cwd.join("datasub"));
    #[cfg(windows)]
    assert_eq!(root(r"C:\\a\\b\\..\\c"), PathBuf::from(r"C:\a\c"));
    #[cfg(not(windows))]
    assert_eq!(root("/a/b/../c"), PathBuf::from("/a/c"));
}

#[test]
fn thread_pool_derivations() {
    let t = |text: &str, cpus| ThreadConfig::from_ini(&ini(text), cpus);

    // -1 (or missing) means processors * 4 / * 2.
    let d = t("ScheduledThreadPoolSize=-1\nInstantThreadPoolSize=-1\n", 4);
    assert_eq!(
        (
            d.scheduled_thread_pool_size,
            d.instant_thread_pool_size,
            d.high_priority_scheduled_thread_pool_size
        ),
        (16, 8, 4)
    );
    // max(2, scheduled / 4)
    let d = t("ScheduledThreadPoolSize=5\nInstantThreadPoolSize=3\n", 4);
    assert_eq!(d.high_priority_scheduled_thread_pool_size, 2);
    // both must be above 2
    let d = t("ScheduledThreadPoolSize=40\nInstantThreadPoolSize=2\n", 4);
    assert_eq!(d.high_priority_scheduled_thread_pool_size, 0);
    let d = t("ScheduledThreadPoolSize=2\nInstantThreadPoolSize=40\n", 4);
    assert_eq!(d.high_priority_scheduled_thread_pool_size, 0);
    // only -1 triggers the processor based value; 0 stays 0
    let d = t("ScheduledThreadPoolSize=0\nInstantThreadPoolSize=-2\n", 4);
    assert_eq!(
        (d.scheduled_thread_pool_size, d.instant_thread_pool_size),
        (0, -2)
    );

    assert!(t("ThreadsForLoading=true\n", 4).threads_for_loading);
    assert!(!t("ThreadsForLoading=nope\n", 4).threads_for_loading);
    assert!(!t("", 4).threads_for_loading);
}

#[test]
fn network_derivations() {
    let n = |text: &str| NetworkConfig::from_ini(&ini(text), PROCESSORS);

    assert_eq!(n("ShutdownWaitTime=7\n").shutdown_wait_time, 7000);
    // 2147483647 s does not overflow because Java multiplies as long
    assert_eq!(
        n("ShutdownWaitTime=2147483647\n").shutdown_wait_time,
        2_147_483_647_000
    );
    // ThreadPoolSize < 1 -> processors * 4
    assert_eq!(n("ThreadPoolSize=0\n").thread_pool_size, 32);
    assert_eq!(n("ThreadPoolSize=-5\n").thread_pool_size, 32);
    assert_eq!(n("ThreadPoolSize=6\n").thread_pool_size, 6);
    let all = n(
        "DropPackets=true\nDropPacketThreshold=9\nThreadPriority=7\nBufferSegmentSize=128\n\
         BufferPool.AutoExpandCapacity=false\nBufferPool.InitFactor=0.5\n",
    );
    assert!(all.drop_packets);
    assert_eq!(all.drop_packet_threshold, 9);
    assert_eq!(all.thread_priority, 7);
    assert_eq!(all.buffer_segment_size, 128);
    assert!(!all.auto_expand_pool_capacity);
    assert_eq!(all.init_buffer_pool_factor, 0.5);
}

#[test]
fn interface_dark_theme_only_read_with_gui() {
    let gui = InterfaceConfig::from_ini(&ini("EnableGUI=true\nDarkTheme=false\n"));
    assert!(gui.enable_gui);
    assert!(!gui.dark_theme);
    let headless = InterfaceConfig::from_ini(&ini("EnableGUI=false\nDarkTheme=true\n"));
    assert!(!headless.enable_gui);
    assert!(!headless.dark_theme);
}

#[test]
fn jdbc_url_parsing() {
    let p = |u: &str| JdbcUrl::parse(u);

    let url = p("jdbc:mysql://db.example.org:3307/l2?a=1&b=x=y&flag").unwrap();
    assert_eq!(url.host, "db.example.org");
    assert_eq!(url.port, 3307);
    assert_eq!(url.database, "l2");
    assert_eq!(
        url.params,
        [
            ("a".to_owned(), "1".to_owned()),
            ("b".to_owned(), "x=y".to_owned()),
            ("flag".to_owned(), String::new())
        ]
    );
    assert_eq!(url.param("b"), Some("x=y"));
    assert_eq!(url.param("missing"), None);

    let url = p("jdbc:mysql://localhost/db").unwrap();
    assert_eq!((url.host.as_str(), url.port), ("localhost", 3306));
    assert!(url.params.is_empty());

    let url = p("JDBC:MariaDB://127.0.0.1:3306/db?").unwrap();
    assert_eq!(
        (url.host.as_str(), url.port, url.database.as_str()),
        ("127.0.0.1", 3306, "db")
    );

    let url = p("jdbc:mysql://[::1]:3310/db").unwrap();
    assert_eq!((url.host.as_str(), url.port), ("::1", 3310));
    let url = p("jdbc:mysql://[::1]/db").unwrap();
    assert_eq!((url.host.as_str(), url.port), ("::1", 3306));

    // no database is allowed (the driver connects without a default schema)
    let url = p("jdbc:mysql://localhost").unwrap();
    assert_eq!(url.database, "");

    for bad in [
        "",
        "mysql://localhost/db",
        "jdbc:postgresql://localhost/db",
        "jdbc:mysql:///db",
        "jdbc:mysql://:3306/db",
        "jdbc:mysql://host:notaport/db",
        "jdbc:mysql://host:99999/db",
        "jdbc:mysql://[::1/db",
    ] {
        assert_eq!(p(bad), None, "{bad:?}");
    }
}

#[test]
fn unparsable_url_keeps_the_raw_text() {
    let db = DatabaseConfig::from_ini(&ini("URL=not a url\n"));
    assert_eq!(db.database_url, "not a url");
    assert_eq!(db.jdbc_url, None);
}

#[test]
fn debug_never_shows_the_password() {
    let db = DatabaseConfig::from_ini(&ini("Login=admin\nPassword=hunter2-secret\n\
         URL=jdbc:mysql://localhost/db?user=bob&password=urlsecret&useSSL=false\n"));
    assert_eq!(db.database_password, "hunter2-secret");

    let shown = format!("{db:?}");
    let pretty = format!("{db:#?}");
    for text in [&shown, &pretty] {
        assert!(!text.contains("hunter2-secret"), "{text}");
        assert!(!text.contains("urlsecret"), "{text}");
        assert!(!text.contains("bob"), "{text}");
        assert!(text.contains("<redacted>"));
        assert!(text.contains("useSSL"));
        assert!(text.contains("admin"));
    }
    assert_eq!(
        db.redacted_url(),
        "jdbc:mysql://localhost/db?user=***&password=***&useSSL=false"
    );
}

#[test]
fn debug_of_the_whole_set_hides_the_password() {
    let dir = tempfile::tempdir().unwrap();
    write(dir.path(), "Database.ini", "Password=topsecret\n");
    let set = LoginConfig::load_with_processors(dir.path(), PROCESSORS).unwrap();
    assert!(!format!("{set:?}").contains("topsecret"));
}
