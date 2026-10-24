//! The date texts of the auction board and the hall manager pages.
// Java: entity/actor/instance/Auctioneer.java (DATE_FORMAT, DATE_FORMAT_SHORT, the bidder list's
//       `Calendar` fields)
// Java: entity/actor/instance/ClanHallManager.java (DATE_FORMAT)
//
// Java formats `Instant.ofEpochMilli(x).atZone(ZoneId.systemDefault())`, so the fields are those of
// the machine's local zone (D-48). The functions take the zone offset so tests do not depend on
// the zone of the machine they run on.

use l2_util::time::local_offset_millis;

/// Year, month (1 to 12) and day of the month of a day count since 1970-01-01 (the proleptic
/// Gregorian calendar, like `java.time`).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = if month_prime < 10 {
        month_prime + 3
    } else {
        month_prime - 9
    };
    let year = year_of_era + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// The local fields of an epoch time: `(year, month, day, hour, minute)`.
fn fields(epoch_ms: i64, offset_ms: i64) -> (i64, i64, i64, i64, i64) {
    let local = epoch_ms.saturating_add(offset_ms);
    let days = local.div_euclid(86_400_000);
    let millis_of_day = local.rem_euclid(86_400_000);
    let (year, month, day) = civil_from_days(days);
    (
        year,
        month,
        day,
        millis_of_day / 3_600_000,
        (millis_of_day / 60_000) % 60,
    )
}

/// `DateTimeFormatter.ofPattern("dd/MM/yyyy HH:mm")` in the zone with this offset.
pub(crate) fn date_time_at(epoch_ms: i64, offset_ms: i64) -> String {
    let (year, month, day, hour, minute) = fields(epoch_ms, offset_ms);
    format!("{day:02}/{month:02}/{year:04} {hour:02}:{minute:02}")
}

/// `DateTimeFormatter.ofPattern("yy/MM/dd")` in the zone with this offset.
pub(crate) fn date_short_at(epoch_ms: i64, offset_ms: i64) -> String {
    let (year, month, day, _, _) = fields(epoch_ms, offset_ms);
    format!("{:02}/{month:02}/{day:02}", year.rem_euclid(100))
}

/// The bidder list's `get(YEAR) + "/" + (get(MONTH) + 1) + "/" + get(DATE)`: no padding.
pub(crate) fn bid_date_at(epoch_ms: i64, offset_ms: i64) -> String {
    let (year, month, day, _, _) = fields(epoch_ms, offset_ms);
    format!("{year}/{month}/{day}")
}

/// [`date_time_at`] in the machine's local zone.
pub(crate) fn date_time(epoch_ms: i64) -> String {
    date_time_at(epoch_ms, local_offset_millis(epoch_ms))
}

/// [`date_short_at`] in the machine's local zone.
pub(crate) fn date_short(epoch_ms: i64) -> String {
    date_short_at(epoch_ms, local_offset_millis(epoch_ms))
}

/// [`bid_date_at`] in the machine's local zone.
pub(crate) fn bid_date(epoch_ms: i64) -> String {
    bid_date_at(epoch_ms, local_offset_millis(epoch_ms))
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2023-11-14T22:13:20Z, a time with a two digit month, day and hour.
    const T: i64 = 1_700_000_000_000;

    #[test]
    fn the_pages_use_javas_patterns() {
        assert_eq!(date_time_at(T, 0), "14/11/2023 22:13");
        assert_eq!(date_short_at(T, 0), "23/11/14");
        assert_eq!(bid_date_at(T, 0), "2023/11/14");
    }

    #[test]
    fn the_zone_offset_moves_the_day() {
        // UTC+2 is already the 15th.
        assert_eq!(date_time_at(T, 2 * 3_600_000), "15/11/2023 00:13");
        assert_eq!(bid_date_at(T, 2 * 3_600_000), "2023/11/15");
        // UTC-5.5 is still the 14th.
        assert_eq!(date_time_at(T, -(5 * 3_600_000 + 1_800_000)), "14/11/2023 16:43");
    }

    #[test]
    fn single_digit_fields_are_padded_except_in_the_bidder_list() {
        // 2005-02-03T04:05:00Z
        let t = 1_107_403_500_000;
        assert_eq!(date_time_at(t, 0), "03/02/2005 04:05");
        assert_eq!(date_short_at(t, 0), "05/02/03");
        assert_eq!(bid_date_at(t, 0), "2005/2/3");
    }

    #[test]
    fn leap_days_and_the_epoch() {
        assert_eq!(date_time_at(0, 0), "01/01/1970 00:00");
        // 2000-02-29T12:00:00Z
        assert_eq!(date_time_at(951_825_600_000, 0), "29/02/2000 12:00");
    }
}
