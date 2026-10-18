//! Configuration support. Reads the same `.ini` files as the Java server (D-02).
//!
//! [`IniFile`] is the port of `org.l2jmobius.commons.util.ConfigReader`. Typed config structs
//! are built on top of it (see the later config cards).

mod database;
mod error;
mod interface;
mod login;
mod network;
mod reader;
mod threads;

pub use database::{DatabaseConfig, JdbcUrl};
pub use error::ConfigError;
pub use interface::InterfaceConfig;
pub use login::{LoginConfig, LoginConfigSet};
pub use network::NetworkConfig;
pub use reader::{IniFile, parse_duration};
pub use threads::ThreadConfig;
