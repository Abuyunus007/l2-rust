//! Configuration support. Reads the same `.ini` files as the Java server (D-02).
//!
//! [`IniFile`] is the port of `org.l2jmobius.commons.util.ConfigReader`. Typed config structs
//! are built on top of it (see the later config cards).

mod database;
pub mod defaults;
mod error;
pub mod game;
pub mod game_manual;
pub mod game_manual_core;
#[rustfmt::skip]
pub mod generated;
pub mod ini_edit;
mod interface;
mod login;
mod network;
mod reader;
mod threads;

pub use database::{DatabaseConfig, JdbcUrl};
pub use error::ConfigError;
pub use game::GameConfig;
pub use interface::InterfaceConfig;
pub use login::{LoginConfig, LoginConfigSet};
pub use network::NetworkConfig;
pub use reader::{IniFile, parse_duration};
pub use threads::ThreadConfig;
