//! Errors of the config reader.

/// Things that make Java throw instead of logging a warning and carrying on.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    /// A `\uXXXX` escape with fewer than four hex digits.
    ///
    /// Java's `Properties.load` throws `IllegalArgumentException` here and `ConfigReader` does
    /// not catch it, so the server would crash at startup.
    #[error("malformed \\uxxxx encoding in '{file}'")]
    MalformedUnicodeEscape { file: String },

    /// `TimeUtil.parseDuration` rejected the text. Only reachable through the *default* of
    /// `get_duration` (a bad config value falls back to the default instead).
    #[error("invalid duration format: '{0}'")]
    InvalidDuration(String),

    /// `TimeUtil.parseDuration` found a number but no known unit.
    #[error("unrecognized time unit: '{0}'")]
    UnknownTimeUnit(String),

    /// A `x,y,z[,heading]` location that Java's `Integer.parseInt` / array indexing rejects.
    /// Java throws out of `FactionSystemConfig.load()`, so the server does not start.
    #[error("invalid location for config '{key}' in file '{file}': '{value}'")]
    InvalidLocation {
        file: String,
        key: String,
        value: String,
    },
}
