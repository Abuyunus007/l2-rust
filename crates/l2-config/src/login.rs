//! `Server.ini` of the login server, and loading of the whole login config set.
// Java: org/l2jmobius/loginserver/config/LoginConfig.java
//
// REFERENCE PATTERN for the generated game configs (M2): one struct per Java config class, one
// field per Java constant (snake_case), a `/// Key: ..., default ...` doc line per field, and a
// `from_ini(&IniFile)` constructor with one getter call per field, in Java's order.

use std::path::{Component, Path, PathBuf};

use crate::database::{DATABASE_CONFIG_FILE, DatabaseConfig};
use crate::interface::{INTERFACE_CONFIG_FILE, InterfaceConfig};
use crate::network::{NETWORK_CONFIG_FILE, NetworkConfig};
use crate::threads::{THREADS_CONFIG_FILE, ThreadConfig};
use crate::{ConfigError, IniFile};

/// File name inside the config directory.
pub const SERVER_CONFIG_FILE: &str = "Server.ini";

/// Settings of `config/Server.ini` (login server).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginConfig {
    /// Key: `LoginPort`, default 9013. Port the game servers connect to.
    pub game_server_login_port: i32,
    /// Key: `LoginHostname`, default `127.0.0.1`. Address the game servers connect to.
    pub game_server_login_host: String,
    /// Key: `LoginserverHostname`, default `0.0.0.0`. Bind address for game clients.
    pub login_bind_address: String,
    /// Key: `LoginserverPort`, default 2106. Port for game clients.
    pub port_login: i32,
    /// Key: `DatapackRoot`, default `.`. Backslashes become `/`, then the path is made absolute
    /// and normalized (Java: `getCanonicalFile`, but symlinks are not resolved here).
    pub datapack_root: PathBuf,
    /// Key: `AcceptNewGameServer`, default true.
    pub accept_new_gameserver: bool,
    /// Key: `LoginTryBeforeBan`, default 5.
    pub login_try_before_ban: i32,
    /// Key: `LoginBlockAfterBan`, default 900 (seconds).
    pub login_block_after_ban: i32,
    /// Key: `LoginRestartSchedule`, default false.
    pub login_server_schedule_restart: bool,
    /// Key: `LoginRestartTime`, default 24 (hours).
    pub login_server_schedule_restart_time: i64,
    /// Key: `ShowLicence`, default true.
    pub show_licence: bool,
    /// Key: `AutoCreateAccounts`, default true.
    pub auto_create_accounts: bool,
    /// Key: `EnableFloodProtection`, default true.
    pub flood_protection: bool,
    /// Key: `FastConnectionLimit`, default 15.
    pub fast_connection_limit: i32,
    /// Key: `NormalConnectionTime`, default 700 (ms).
    pub normal_connection_time: i32,
    /// Key: `FastConnectionTime`, default 350 (ms).
    pub fast_connection_time: i32,
    /// Key: `MaxConnectionPerIP`, default 50.
    pub max_connection_per_ip: i32,
}

/// Everything the login server reads at startup: `Server.ini`, `Database.ini`, `Threads.ini`,
/// `Network.ini` and `Interface.ini`. Wrap it in an `Arc` to share it.
#[derive(Debug, Clone)]
pub struct LoginConfigSet {
    pub login: LoginConfig,
    pub database: DatabaseConfig,
    pub threads: ThreadConfig,
    pub network: NetworkConfig,
    pub interface: InterfaceConfig,
}

impl LoginConfig {
    /// Reads the values like `LoginConfig.load()`.
    pub fn from_ini(ini: &IniFile) -> Self {
        Self {
            game_server_login_host: ini.get_string("LoginHostname", "127.0.0.1"),
            game_server_login_port: ini.get_int("LoginPort", 9013),
            login_bind_address: ini.get_string("LoginserverHostname", "0.0.0.0"),
            port_login: ini.get_int("LoginserverPort", 2106),
            datapack_root: resolve_datapack_root(&ini.get_string("DatapackRoot", ".")),
            accept_new_gameserver: ini.get_bool("AcceptNewGameServer", true),
            login_try_before_ban: ini.get_int("LoginTryBeforeBan", 5),
            login_block_after_ban: ini.get_int("LoginBlockAfterBan", 900),
            login_server_schedule_restart: ini.get_bool("LoginRestartSchedule", false),
            login_server_schedule_restart_time: ini.get_long("LoginRestartTime", 24),
            show_licence: ini.get_bool("ShowLicence", true),
            auto_create_accounts: ini.get_bool("AutoCreateAccounts", true),
            flood_protection: ini.get_bool("EnableFloodProtection", true),
            fast_connection_limit: ini.get_int("FastConnectionLimit", 15),
            normal_connection_time: ini.get_int("NormalConnectionTime", 700),
            fast_connection_time: ini.get_int("FastConnectionTime", 350),
            max_connection_per_ip: ini.get_int("MaxConnectionPerIP", 50),
        }
    }

    /// Loads the five login server config files from `config_dir` (Java: `./config`).
    ///
    /// A missing file gives all defaults for that file (with warnings), like Java. The only
    /// error is a malformed `\uXXXX` escape in one of the files.
    pub fn load(config_dir: &Path) -> Result<LoginConfigSet, ConfigError> {
        let processors = std::thread::available_parallelism()
            .map_or(1, std::num::NonZero::get)
            .try_into()
            .unwrap_or(i32::MAX);
        Self::load_with_processors(config_dir, processors)
    }

    /// [`Self::load`] with an explicit `Runtime.availableProcessors()` value, for tests.
    pub fn load_with_processors(config_dir: &Path, processors: i32) -> Result<LoginConfigSet, ConfigError> {
        Ok(LoginConfigSet {
            login: Self::from_ini(&IniFile::load(config_dir.join(SERVER_CONFIG_FILE))?),
            database: DatabaseConfig::from_ini(&IniFile::load(config_dir.join(DATABASE_CONFIG_FILE))?),
            threads: ThreadConfig::from_ini(
                &IniFile::load(config_dir.join(THREADS_CONFIG_FILE))?,
                processors,
            ),
            network: NetworkConfig::from_ini(
                &IniFile::load(config_dir.join(NETWORK_CONFIG_FILE))?,
                processors,
            ),
            interface: InterfaceConfig::from_ini(&IniFile::load(config_dir.join(INTERFACE_CONFIG_FILE))?),
        })
    }
}

/// `new File(value.replace('\\', '/')).getCanonicalFile()` without touching the file system
/// beyond the current directory: absolute, `.` and `..` resolved lexically.
fn resolve_datapack_root(value: &str) -> PathBuf {
    let path = PathBuf::from(value.replace('\\', "/"));
    let absolute = std::path::absolute(&path).unwrap_or(path);
    let mut out = PathBuf::new();
    for component in absolute.components() {
        match component {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}
