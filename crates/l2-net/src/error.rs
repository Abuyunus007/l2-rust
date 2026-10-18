use thiserror::Error;

/// Errors while decoding a packet. Never panics on malformed network data.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum PacketError {
    /// The packet ended before a field could be read.
    #[error("packet truncated: needed {needed} more bytes at offset {offset}, {remaining} left")]
    Truncated {
        offset: usize,
        needed: usize,
        remaining: usize,
    },

    /// A field had a value the packet doesn't allow (bad length, out-of-range count, ...).
    #[error("invalid {field}: {reason}")]
    Invalid { field: &'static str, reason: String },
}

impl PacketError {
    /// Shorthand for [`PacketError::Invalid`].
    pub fn invalid(field: &'static str, reason: impl Into<String>) -> Self {
        Self::Invalid {
            field,
            reason: reason.into(),
        }
    }
}
