//! Time helpers with Java semantics.

use std::time::{SystemTime, UNIX_EPOCH};

/// Milliseconds since the Unix epoch, like Java `System.currentTimeMillis()`.
///
/// The DB stores many timestamps in this form (e.g. `accounts.lastactive`), so it is an `i64`.
/// A clock before 1970 returns 0 instead of failing.
pub fn now_millis() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_millis_is_after_2020() {
        // 2020-01-01T00:00:00Z
        assert!(now_millis() > 1_577_836_800_000);
    }
}
