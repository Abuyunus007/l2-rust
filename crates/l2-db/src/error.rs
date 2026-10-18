//! Errors of the database layer.

/// `Result` with [`DbError`].
pub type Result<T, E = DbError> = std::result::Result<T, E>;

/// Everything that can go wrong in `l2-db`.
#[derive(Debug, thiserror::Error)]
pub enum DbError {
    /// `Database.ini` has a `URL` that is not `jdbc:mysql://host[:port]/database`.
    #[error("Database URL is not a jdbc:mysql://host[:port]/database URL: {0}")]
    InvalidUrl(String),
    /// The server rejected a statement or the connection failed.
    #[error("database error: {0}")]
    Sqlx(#[from] sqlx::Error),
    /// A `gameservers.hexid` value that Java's `new BigInteger(s, 16)` would reject.
    #[error("invalid hexid: {0:?}")]
    InvalidHexId(String),
}
