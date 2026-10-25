//! The schedule of a conquerable hall: `schedule_config` and `SiegableHall.updateNextSiege`, and
//! the date texts of the scripts' pages.
// Java: mechanics/siege/clanhalls/SiegableHall.java (constructor: `_scheduleConfig`,
//       updateNextSiege)
// Java: commons/time/TimeUtil.java (formatDate), java.util.Date.toString
//
// # Calendars
// Java computes the dates with `Calendar.getInstance()` in the JVM's zone (D-48). The functions
// here take the offset of that zone as a function of the UTC time, so tests can fix it (the
// castle schedule in `siege/schedule.rs` does the same).

use tracing::warn;

const SECOND_MS: i64 = 1000;
const MINUTE_MS: i64 = 60 * SECOND_MS;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

/// `SiegableHall._scheduleConfig`'s initial value: every 7 days at 12:00.
pub(crate) const DEFAULT_SCHEDULE: [i32; 5] = [7, 0, 0, 12, 0];

/// `String.split(";")`: the empty strings at the end are dropped.
fn java_split_semicolon(text: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = text.split(';').collect();
    while parts.len() > 1 && parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    // `"".split(";")` is `[""]`.
    parts
}

/// The `_scheduleConfig` of a hall from its `schedule_config` column (`DD;MM;YY;HH;mm`: days,
/// months and years to add, then the hour and the minute). A value that is not five numbers keeps
/// the default; a part that is not a number keeps the default of that part.
pub(crate) fn parse_schedule_config(hall_name: &str, raw: Option<&str>) -> [i32; 5] {
    let mut config = DEFAULT_SCHEDULE;
    // NOTE(java-quirk): a NULL column makes `StatSet.getString` throw in Java and the hall is not
    // loaded at all; here it keeps the default.
    let Some(raw) = raw else {
        warn!("{hall_name}: Wrong schedule_config value in siegable_halls table, using default (7 days)");
        return config;
    };
    let parts = java_split_semicolon(raw);
    if parts.len() == 5 {
        for (slot, part) in config.iter_mut().zip(parts) {
            match part.parse::<i32>() {
                Ok(value) => *slot = value,
                Err(_) => warn!("SiegableHall - {hall_name}: Wrong schedule_config parameters!"),
            }
        }
    } else {
        warn!("{hall_name}: Wrong schedule_config value in siegable_halls table, using default (7 days)");
    }
    config
}

/// Year, month (1 to 12) and day of the month of a day count since 1970-01-01 (the proleptic
/// Gregorian calendar).
fn civil_from_days(days: i64) -> (i64, i64, i64) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

/// Days since 1970-01-01 of a civil date.
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

fn days_in_month(year: i64, month: i64) -> i64 {
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    days_from_civil(next_year, next_month, 1) - days_from_civil(year, month, 1)
}

/// Back from local milliseconds to UTC (the offset of the result, like `Calendar` resolving the
/// fields).
fn from_local(local: i64, offset_at: &dyn Fn(i64) -> i64) -> i64 {
    let guess = local - offset_at(local);
    local - offset_at(guess)
}

/// `Calendar.add(MONTH, months)`: the day of the month is cut to the length of the new month.
fn add_months(day: i64, months: i64) -> i64 {
    let (year, month, dom) = civil_from_days(day);
    let index = year * 12 + (month - 1) + months;
    let (new_year, new_month) = (index.div_euclid(12), index.rem_euclid(12) + 1);
    days_from_civil(new_year, new_month, dom.min(days_in_month(new_year, new_month)))
}

/// `Calendar.add(YEAR, years)`: 29 February becomes the 28th.
fn add_years(day: i64, years: i64) -> i64 {
    add_months(day, years * 12)
}

/// `SiegableHall.updateNextSiege()`: from `now`, add the days, months and years of the schedule,
/// then set the hour and the minute and a second of 0 (the milliseconds stay).
pub(crate) fn next_siege_after(now: i64, config: [i32; 5], offset_at: &dyn Fn(i64) -> i64) -> i64 {
    let local_now = now + offset_at(now);
    let mut day = local_now.div_euclid(DAY_MS);
    let millis = local_now.rem_euclid(DAY_MS).rem_euclid(SECOND_MS);
    day += i64::from(config[0]);
    day = add_months(day, i64::from(config[1]));
    day = add_years(day, i64::from(config[2]));
    // `set(HOUR_OF_DAY, h)`, `set(MINUTE, m)` and `set(SECOND, 0)` are lenient: 24 is the next day.
    let local = day * DAY_MS + i64::from(config[3]) * HOUR_MS + i64::from(config[4]) * MINUTE_MS + millis;
    from_local(local, offset_at)
}

/// The epoch time of local calendar fields (`Calendar.set(YEAR/MONTH/DAY_OF_MONTH/HOUR_OF_DAY/
/// MINUTE/SECOND)` on a calendar of `now`: the second is 0, the milliseconds stay). The fields are
/// lenient: a day or an hour past the end rolls over. `month` counts from 1.
pub(crate) fn date_at(
    now: i64,
    (year, month, day): (i64, i64, i64),
    (hour, minute): (i64, i64),
    offset_at: &dyn Fn(i64) -> i64,
) -> i64 {
    let millis = (now + offset_at(now)).rem_euclid(DAY_MS).rem_euclid(SECOND_MS);
    let local = days_from_civil(year, month, day) * DAY_MS + hour * HOUR_MS + minute * MINUTE_MS + millis;
    from_local(local, offset_at)
}

/// `Calendar.get(YEAR)` of an epoch time.
pub(crate) fn year_of(epoch_ms: i64, offset_ms: i64) -> i64 {
    civil_from_days((epoch_ms + offset_ms).div_euclid(DAY_MS)).0
}

/// The local fields of an epoch time: `(year, month, day, hour, minute, second, weekday 0 = Sunday)`.
fn fields(epoch_ms: i64, offset_ms: i64) -> (i64, i64, i64, i64, i64, i64, usize) {
    let local = epoch_ms.saturating_add(offset_ms);
    let days = local.div_euclid(DAY_MS);
    let ms = local.rem_euclid(DAY_MS);
    let (year, month, day) = civil_from_days(days);
    // 1970-01-01 was a Thursday.
    let weekday = usize::try_from((days + 4).rem_euclid(7)).unwrap_or(0);
    (
        year,
        month,
        day,
        ms / HOUR_MS,
        (ms / MINUTE_MS) % 60,
        (ms / SECOND_MS) % 60,
        weekday,
    )
}

/// `TimeUtil.formatDate(date, "yyyy-MM-dd HH:mm:ss")` in the zone with this offset.
pub(crate) fn format_date_time(epoch_ms: i64, offset_ms: i64) -> String {
    let (year, month, day, hour, minute, second, _) = fields(epoch_ms, offset_ms);
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

/// `Instant.ofEpochMilli(x).atZone(...).format(ofPattern("HH:mm dd.MM.yyyy"))`.
pub(crate) fn format_hour_day(epoch_ms: i64, offset_ms: i64) -> String {
    let (year, month, day, hour, minute, _, _) = fields(epoch_ms, offset_ms);
    format!("{hour:02}:{minute:02} {day:02}.{month:02}.{year:04}")
}

/// Java `Date.toString()`: `EEE MMM dd HH:mm:ss zzz yyyy`. The zone is written as `GMT+hh:mm` (the
/// machine's zone name is not known here).
pub(crate) fn java_date_string(epoch_ms: i64, offset_ms: i64) -> String {
    const DAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    let (year, month, day, hour, minute, second, weekday) = fields(epoch_ms, offset_ms);
    let month_name = usize::try_from(month - 1)
        .ok()
        .and_then(|m| MONTHS.get(m))
        .copied()
        .unwrap_or("");
    let sign = if offset_ms < 0 { '-' } else { '+' };
    let offset_minutes = (offset_ms / MINUTE_MS).abs();
    format!(
        "{} {month_name} {day:02} {hour:02}:{minute:02}:{second:02} GMT{sign}{:02}:{:02} {year}",
        DAYS[weekday],
        offset_minutes / 60,
        offset_minutes % 60,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-24T09:41:07.321Z, a Saturday.
    const NOW: i64 = 1_792_834_867_321;

    fn utc(_: i64) -> i64 {
        0
    }

    #[test]
    fn the_default_schedule_is_a_week_later_at_noon() {
        let next = next_siege_after(NOW, DEFAULT_SCHEDULE, &utc);
        // 2026-10-31T12:00:00.321Z: the milliseconds stay, the second is 0.
        assert_eq!(next, 1_793_448_000_321);
        assert_eq!(format_date_time(next, 0), "2026-10-31 12:00:00");
    }

    #[test]
    fn months_and_years_are_added_after_the_days_and_cut_to_the_month() {
        // 2026-01-31 + 0 days + 1 month = 2026-02-28.
        let jan31 = days_from_civil(2026, 1, 31) * DAY_MS + 10 * HOUR_MS;
        let next = next_siege_after(jan31, [0, 1, 0, 6, 30], &utc);
        assert_eq!(format_date_time(next, 0), "2026-02-28 06:30:00");
        // 2024-02-29 + 1 year = 2025-02-28.
        let leap = days_from_civil(2024, 2, 29) * DAY_MS;
        let next = next_siege_after(leap, [0, 0, 1, 0, 0], &utc);
        assert_eq!(format_date_time(next, 0), "2025-02-28 00:00:00");
        // Days are added first: 2026-12-30 + 5 days + 1 month = 2027-02-04.
        let dec30 = days_from_civil(2026, 12, 30) * DAY_MS;
        let next = next_siege_after(dec30, [5, 1, 0, 12, 0], &utc);
        assert_eq!(format_date_time(next, 0), "2027-02-04 12:00:00");
    }

    #[test]
    fn the_zone_decides_the_day() {
        // At UTC+2 it is 11:41 on the 24th; a week later at 12:00 local is 10:00 UTC.
        let plus_two = |_: i64| 2 * HOUR_MS;
        let next = next_siege_after(NOW, DEFAULT_SCHEDULE, &plus_two);
        assert_eq!(format_date_time(next, 0), "2026-10-31 10:00:00");
        // Late evening UTC is already the next day at UTC+3: 2026-10-24T22:00Z is the 25th.
        let evening = days_from_civil(2026, 10, 24) * DAY_MS + 22 * HOUR_MS;
        let plus_three = |_: i64| 3 * HOUR_MS;
        let next = next_siege_after(evening, [1, 0, 0, 0, 0], &plus_three);
        assert_eq!(format_date_time(next, 3 * HOUR_MS), "2026-10-26 00:00:00");
    }

    #[test]
    fn schedule_config_parses_like_the_siegable_hall_constructor() {
        assert_eq!(
            parse_schedule_config("x", Some("14;0;0;12;00")),
            [14, 0, 0, 12, 0]
        );
        assert_eq!(parse_schedule_config("x", Some("1;2;3;4;5")), [1, 2, 3, 4, 5]);
        // Not five parts: the default.
        assert_eq!(parse_schedule_config("x", Some("14;0;0;12")), DEFAULT_SCHEDULE);
        // `split` drops the empty strings at the end, so this has four parts.
        assert_eq!(parse_schedule_config("x", Some("14;0;0;12;")), DEFAULT_SCHEDULE);
        // A part that is not a number keeps its default.
        assert_eq!(
            parse_schedule_config("x", Some("14;a;0;12;30")),
            [14, 0, 0, 12, 30]
        );
        assert_eq!(parse_schedule_config("x", None), DEFAULT_SCHEDULE);
    }

    #[test]
    fn local_fields_make_a_date() {
        let when = date_at(NOW, (2026, 12, 25), (18, 30), &utc);
        assert_eq!(format_date_time(when, 0), "2026-12-25 18:30:00");
        assert_eq!(when % 1000, 321);
        // A day past the end of the month rolls over.
        let when = date_at(NOW, (2026, 2, 30), (1, 0), &utc);
        assert_eq!(format_date_time(when, 0), "2026-03-02 01:00:00");
        assert_eq!(year_of(NOW, 0), 2026);
    }

    #[test]
    fn the_date_texts() {
        assert_eq!(format_date_time(NOW, 0), "2026-10-24 09:41:07");
        assert_eq!(format_hour_day(NOW, 0), "09:41 24.10.2026");
        assert_eq!(java_date_string(NOW, 0), "Sat Oct 24 09:41:07 GMT+00:00 2026");
        assert_eq!(
            java_date_string(NOW, -(5 * HOUR_MS + 30 * MINUTE_MS)),
            "Sat Oct 24 04:11:07 GMT-05:30 2026"
        );
    }
}
