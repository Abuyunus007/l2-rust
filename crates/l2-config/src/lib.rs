//! Configuration support. Reads the same `.ini` files as the Java server (D-02).
//!
//! [`IniFile`] is the port of `org.l2jmobius.commons.util.ConfigReader`. Typed config structs
//! are built on top of it (see the later config cards).

mod error;
mod reader;

pub use error::ConfigError;
pub use reader::{IniFile, parse_duration};
