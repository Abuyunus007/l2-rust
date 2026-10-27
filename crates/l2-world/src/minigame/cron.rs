//! The cron-like pattern of `UndergroundColiseum.ini` (`BattleStartTime`).
// Java: commons/time/SchedulingPattern.java (`next`, `CronExpression.getNextMatch`)
//
// Only what the coliseum needs: five fields (minute hour day month weekday), `*`, lists, ranges
// (with the wrap-around of Java), `/step`, month and weekday names, `L` (last day of the month)
// and `|` for alternatives. The extended modifiers of Java (`~N`, `+N`, the sixth week-offset
// field) are refused: a pattern that uses them does not parse and the caller falls back to the
// default pattern with a warning.
//
// The zone is the machine's (`TimeZone.getDefault()`), through the offset the caller hands in.
//
// NOTE(java-quirk): Java accepts any number in a field, so the day of week 7 of the shipped
// default (`5,6,7`) is a value that no day has (Calendar days run 0..=6 here): Sunday never
// matches.

use std::collections::BTreeSet;

const LAST_DAY_MARKER: i32 = 32;
const SEARCH_LIMIT_YEARS: i64 = 4;
const MS_PER_MINUTE: i64 = 60_000;
const MS_PER_DAY: i64 = 86_400_000;

/// One field: `None` is the wildcard.
type Field = Option<BTreeSet<i32>>;

/// One cron expression (the alternatives of `|` are one each).
#[derive(Debug, Clone, PartialEq, Eq)]
struct Expression {
    minute: Field,
    hour: Field,
    day: Field,
    month: Field,
    day_of_week: Field,
}

/// Java `SchedulingPattern`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SchedulingPattern {
    expressions: Vec<Expression>,
}

/// Why a pattern did not parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternError(pub String);

impl SchedulingPattern {
    /// A pattern that never matches (`next` answers -1).
    pub fn never() -> Self {
        Self {
            expressions: Vec::new(),
        }
    }

    /// Java `new SchedulingPattern(pattern)`.
    pub fn parse(pattern: &str) -> Result<Self, PatternError> {
        let mut expressions = Vec::new();
        for alternative in pattern.split('|') {
            let fields: Vec<&str> = alternative.split_whitespace().collect();
            if fields.len() != 5 {
                return Err(PatternError(format!(
                    "pattern must have 5 fields (modifiers are not supported): {alternative}"
                )));
            }
            expressions.push(Expression {
                minute: parse_field(fields[0], 0, 59, &[])?,
                hour: parse_field(fields[1], 0, 23, &[])?,
                day: parse_field(fields[2], 1, 31, &[])?,
                month: parse_field(fields[3], 1, 12, &MONTHS)?,
                day_of_week: parse_field(fields[4], 0, 6, &DAYS)?,
            });
        }
        Ok(Self { expressions })
    }

    /// Java `next(millis)`: the first matching minute after `after_ms` (UTC milliseconds), or -1.
    /// `offset_ms` is the local zone's offset from UTC.
    pub fn next(&self, after_ms: i64, offset_ms: i64) -> i64 {
        let mut earliest = -1;
        for expression in &self.expressions {
            let next = expression.next_match(after_ms, offset_ms);
            if next > after_ms && (earliest == -1 || next < earliest) {
                earliest = next;
            }
        }
        earliest
    }
}

const MONTHS: [(&str, i32); 12] = [
    ("jan", 1),
    ("feb", 2),
    ("mar", 3),
    ("apr", 4),
    ("may", 5),
    ("jun", 6),
    ("jul", 7),
    ("aug", 8),
    ("sep", 9),
    ("oct", 10),
    ("nov", 11),
    ("dec", 12),
];

const DAYS: [(&str, i32); 7] = [
    ("sun", 0),
    ("mon", 1),
    ("tue", 2),
    ("wed", 3),
    ("thu", 4),
    ("fri", 5),
    ("sat", 6),
];

fn parse_field(text: &str, min: i32, max: i32, aliases: &[(&str, i32)]) -> Result<Field, PatternError> {
    if text.contains([':', '~', '+']) {
        return Err(PatternError(format!("modifier not supported: {text}")));
    }
    if text == "*" {
        return Ok(None);
    }
    let mut values = BTreeSet::new();
    for part in text.split(',') {
        parse_part(part.trim(), min, max, aliases, &mut values)?;
    }
    Ok(Some(values))
}

fn parse_part(
    part: &str,
    min: i32,
    max: i32,
    aliases: &[(&str, i32)],
    values: &mut BTreeSet<i32>,
) -> Result<(), PatternError> {
    let mut pieces = part.split('/');
    let range = pieces.next().unwrap_or("");
    let step = match pieces.next() {
        Some(step) => step
            .parse::<i32>()
            .map_err(|_| PatternError(format!("bad step: {part}")))?,
        None => 1,
    };
    if step < 1 {
        return Err(PatternError(format!("bad step: {part}")));
    }
    let step = step as usize;
    if range == "*" {
        values.extend((min..=max).step_by(step));
    } else if let Some((from, to)) = range.split_once('-') {
        let start = parse_value(from, aliases)?;
        let end = parse_value(to, aliases)?;
        if start <= end {
            values.extend((start..=end).step_by(step));
        } else {
            // Wrap-around range (fri-mon).
            values.extend((start..=max).step_by(step));
            values.extend((min..=end).step_by(step));
        }
    } else {
        values.insert(parse_value(range, aliases)?);
    }
    Ok(())
}

fn parse_value(text: &str, aliases: &[(&str, i32)]) -> Result<i32, PatternError> {
    if text.eq_ignore_ascii_case("L") {
        return Ok(LAST_DAY_MARKER);
    }
    let lower = text.to_ascii_lowercase();
    if let Some((_, value)) = aliases.iter().find(|(name, _)| *name == lower) {
        return Ok(*value);
    }
    text.parse::<i32>()
        .map_err(|_| PatternError(format!("invalid value: {text}")))
}

/// Days since 1970-01-01 to (year, month 1..=12, day 1..=31).
fn civil_from_days(days: i64) -> (i64, i32, i32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = (doy - (153 * mp + 2) / 5 + 1) as i32;
    let month = if mp < 10 { mp + 3 } else { mp - 9 } as i32;
    (if month <= 2 { year + 1 } else { year }, month, day)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

fn days_in_month(year: i64, month: i32) -> i32 {
    match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        _ if is_leap(year) => 29,
        _ => 28,
    }
}

fn matches_field(field: &Field, value: i32, last_day: bool) -> bool {
    match field {
        None => true,
        Some(values) => values.contains(&value) || (last_day && values.contains(&LAST_DAY_MARKER)),
    }
}

impl Expression {
    fn day_matches(&self, year: i64, month: i32, day: i32, day_of_week: i32) -> bool {
        let last_day = day == days_in_month(year, month);
        matches_field(&self.day, day, last_day)
            && matches_field(&self.month, month, false)
            && matches_field(&self.day_of_week, day_of_week, false)
    }

    /// Java `CronExpression.getNextMatch`, a day at a time instead of a minute at a time.
    fn next_match(&self, after_ms: i64, offset_ms: i64) -> i64 {
        // `calendar.add(MINUTE, 1)` then `set(SECOND, 0)`: the next whole minute.
        let local = after_ms + offset_ms;
        let start = (local.div_euclid(MS_PER_MINUTE) + 1) * MS_PER_MINUTE;
        let end = local + SEARCH_LIMIT_YEARS * 366 * MS_PER_DAY;
        let first_day = start.div_euclid(MS_PER_DAY);
        let mut day_index = first_day;
        while day_index * MS_PER_DAY < end {
            let (year, month, day) = civil_from_days(day_index);
            // 1970-01-01 was a Thursday (4, with Sunday 0).
            let day_of_week = (day_index + 4).rem_euclid(7) as i32;
            if self.day_matches(year, month, day, day_of_week) {
                let from_minute = if day_index == first_day {
                    ((start - first_day * MS_PER_DAY) / MS_PER_MINUTE) as i32
                } else {
                    0
                };
                for minute_of_day in from_minute..1440 {
                    let (hour, minute) = (minute_of_day / 60, minute_of_day % 60);
                    if matches_field(&self.hour, hour, false) && matches_field(&self.minute, minute, false) {
                        let local_match = day_index * MS_PER_DAY + i64::from(minute_of_day) * MS_PER_MINUTE;
                        return local_match - offset_ms;
                    }
                }
            }
            day_index += 1;
        }
        -1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-27 (a Tuesday) 12:00:30 UTC.
    const TUESDAY_NOON: i64 = 1_793_102_430_000;

    #[test]
    fn the_next_minute_boundary_is_found() {
        let pattern = SchedulingPattern::parse("*/15 * * * *").unwrap();
        // 12:00:30 -> 12:15:00
        assert_eq!(pattern.next(TUESDAY_NOON, 0), TUESDAY_NOON - 30_000 + 15 * 60_000);
    }

    #[test]
    fn a_daily_pattern_goes_to_the_next_day_when_today_has_passed() {
        let pattern = SchedulingPattern::parse("0 11 * * *").unwrap();
        // Today 11:00 is gone: tomorrow 11:00.
        assert_eq!(
            pattern.next(TUESDAY_NOON, 0),
            TUESDAY_NOON - 30_000 - 3_600_000 + MS_PER_DAY
        );
    }

    #[test]
    fn the_zone_offset_moves_the_wall_clock() {
        let pattern = SchedulingPattern::parse("0 14 * * *").unwrap();
        // UTC+2: 14:00 local is 12:00 UTC, which has just passed (12:00:30), so tomorrow.
        let next = pattern.next(TUESDAY_NOON, 2 * 3_600_000);
        assert_eq!(next, TUESDAY_NOON - 30_000 + MS_PER_DAY);
    }

    #[test]
    fn weekday_lists_and_the_seven_that_never_matches() {
        // Java default: Friday, Saturday and "7".
        let pattern = SchedulingPattern::parse("0 21 * * 5,6,7").unwrap();
        // The next Friday 21:00 after Tuesday 2026-10-27 is 2026-10-30.
        let next = pattern.next(TUESDAY_NOON, 0);
        let (year, month, day) = civil_from_days(next.div_euclid(MS_PER_DAY));
        assert_eq!((year, month, day), (2026, 10, 30));
        assert_eq!(next.rem_euclid(MS_PER_DAY), 21 * 3_600_000);
        // Sunday (0) is not in the set.
        let sunday = SchedulingPattern::parse("0 21 * * 0").unwrap();
        let next_sunday = sunday.next(TUESDAY_NOON, 0);
        assert_eq!(civil_from_days(next_sunday.div_euclid(MS_PER_DAY)), (2026, 11, 1));
        let seven = SchedulingPattern::parse("0 21 * * 7").unwrap();
        assert_eq!(seven.next(TUESDAY_NOON, 0), -1);
    }

    #[test]
    fn names_ranges_and_alternatives() {
        let pattern = SchedulingPattern::parse("30 6 1 jan *|0 0 L * sat-sun").unwrap();
        assert_eq!(pattern.expressions.len(), 2);
        let wrap = SchedulingPattern::parse("0 0 * * sat-mon").unwrap();
        let days = wrap.expressions[0].day_of_week.clone().unwrap();
        assert_eq!(days.into_iter().collect::<Vec<_>>(), vec![0, 1, 6]);
    }

    #[test]
    fn the_last_day_marker_matches_the_last_day_only() {
        let pattern = SchedulingPattern::parse("0 12 L * *").unwrap();
        let next = pattern.next(TUESDAY_NOON, 0);
        assert_eq!(civil_from_days(next.div_euclid(MS_PER_DAY)), (2026, 10, 31));
    }

    #[test]
    fn modifiers_and_bad_patterns_do_not_parse() {
        assert!(SchedulingPattern::parse("~5:0 0 * * *").is_err());
        assert!(SchedulingPattern::parse("0 0 * * * +1").is_err());
        assert!(SchedulingPattern::parse("0 0 *").is_err());
        assert!(SchedulingPattern::parse("x 0 * * *").is_err());
    }
}
