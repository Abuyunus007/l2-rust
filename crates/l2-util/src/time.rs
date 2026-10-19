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

/// The offset of the server's local time zone from UTC at `utc_millis`, in milliseconds.
///
/// Java reads calendar fields (`Calendar.getInstance()`, `new java.sql.Date(..)` written to a
/// `DATE` column) in the JVM's default time zone, which is the machine's local zone. 0 when the
/// zone cannot tell (a time outside its range).
pub fn local_offset_millis(utc_millis: i64) -> i64 {
    use chrono::{Offset, TimeZone};
    chrono::Local
        .timestamp_millis_opt(utc_millis)
        .single()
        .map_or(0, |dt| i64::from(dt.offset().fix().local_minus_utc()) * 1000)
}

/// "Local wall-clock milliseconds": now, shifted by the local offset, so plain day/hour math on
/// the value gives the fields Java's `Calendar.getInstance()` shows on this machine.
pub fn local_now_millis() -> i64 {
    let now = now_millis();
    now + local_offset_millis(now)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn now_millis_is_after_2020() {
        // 2020-01-01T00:00:00Z
        assert!(now_millis() > 1_577_836_800_000);
    }

    #[test]
    fn the_local_offset_is_a_whole_quarter_hour_within_a_day() {
        let offset = local_offset_millis(now_millis());
        assert!(offset.abs() < 86_400_000);
        assert_eq!(offset % (15 * 60 * 1000), 0);
    }
}
