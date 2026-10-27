//! The date generator of an auction instance: when the next auction starts.
// Java: entity/item/auction/AuctionDateGenerator.java
//
// Java sets the fields of a `Calendar` in the JVM's zone and then moves the result by whole weeks
// (or `interval` days) until it is not in the past. The same is done here on "local wall-clock
// milliseconds" (D-48): the offset of the machine's zone is added, the day maths is plain integer
// maths, and the result goes back to UTC.
//
// NOTE: Java's `Calendar.set(DAY_OF_WEEK, ..)` stays inside the week the locale defines (Sunday or
// Monday first). Moving the result by whole weeks until it is not in the past makes that choice
// irrelevant: both give the first instant on that weekday, hour and minute that is not before the
// given date. Only the hour of a daylight saving change differs (the week step is 7 * 24 hours in
// Java as well, and the zone is applied once here).

const MILLIS_IN_DAY: i64 = 86_400_000;
const MILLIS_IN_WEEK: i64 = 7 * MILLIS_IN_DAY;

/// Java `AuctionDateGenerator`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DateGenerator {
    /// `interval`: days between two auctions, used when there is no day of the week.
    interval: i32,
    /// Java `Calendar` numbering (1 Sunday .. 7 Saturday) or -1 for "use the interval".
    day_of_week: i32,
    hour_of_day: i32,
    minute_of_hour: i32,
}

impl DateGenerator {
    /// The attributes of an `<instance>`: `interval`, `day_of_week` (1 Monday .. 7 Sunday),
    /// `hour_of_day` and `minute_of_hour`; `None` for an attribute the XML does not have (Java
    /// reads it with the default -1).
    pub fn new(
        interval: Option<i32>,
        day_of_week: Option<i32>,
        hour_of_day: Option<i32>,
        minute_of_hour: Option<i32>,
    ) -> Result<Self, String> {
        let interval = interval.unwrap_or(-1);
        // NC week start in Monday.
        let fixed_day_week = day_of_week.unwrap_or(-1).wrapping_add(1);
        let mut day_of_week = if fixed_day_week > 7 { 1 } else { fixed_day_week };
        let hour_of_day = hour_of_day.unwrap_or(-1);
        let mut minute_of_hour = minute_of_hour.unwrap_or(-1);

        // checkDayOfWeek(-1)
        if !(1..=7).contains(&day_of_week) {
            if interval < 1 {
                return Err(format!(
                    "Illegal params for 'day_of_week': {}",
                    if day_of_week == -1 {
                        "not found".to_owned()
                    } else {
                        day_of_week.to_string()
                    }
                ));
            }
            day_of_week = -1;
        } else if interval > 1 {
            return Err(
                "Illegal params for 'interval' and 'day_of_week': you can use only one, not both".to_owned(),
            );
        }
        // checkHourOfDay(-1)
        if !(0..=23).contains(&hour_of_day) {
            return Err(format!(
                "Illegal params for 'hour_of_day': {}",
                if hour_of_day == -1 {
                    "not found".to_owned()
                } else {
                    hour_of_day.to_string()
                }
            ));
        }
        // checkMinuteOfHour(0)
        if !(0..=59).contains(&minute_of_hour) {
            minute_of_hour = 0;
        }
        Ok(Self {
            interval,
            day_of_week,
            hour_of_day,
            minute_of_hour,
        })
    }

    /// Java `nextDate(date)` in the machine's zone (D-48).
    pub fn next_date(&self, date: i64) -> i64 {
        self.next_date_in_zone(date, l2_util::time::local_offset_millis)
    }

    /// Java `nextDate(date)` with the zone given as `offset_at(utc_millis)`, for tests.
    pub fn next_date_in_zone(&self, date: i64, offset_at: impl Fn(i64) -> i64) -> i64 {
        let local = date.wrapping_add(offset_at(date));
        let day_start = local.div_euclid(MILLIS_IN_DAY) * MILLIS_IN_DAY;
        let time_of_day = i64::from(self.hour_of_day) * 3_600_000 + i64::from(self.minute_of_hour) * 60_000;
        let mut base_local = day_start + time_of_day;
        if self.day_of_week > 0 {
            // Calendar numbering: 1970-01-01 was a Thursday (5).
            let today = i32::try_from((local.div_euclid(MILLIS_IN_DAY) + 4).rem_euclid(7)).unwrap_or(0) + 1;
            let ahead = (self.day_of_week - today).rem_euclid(7);
            base_local += i64::from(ahead) * MILLIS_IN_DAY;
        }
        // The zone offset of the instant the local fields name.
        let guess = base_local.wrapping_sub(offset_at(base_local));
        let base = base_local.wrapping_sub(offset_at(guess));
        if self.day_of_week > 0 {
            Self::calc_dest_time(base, date, MILLIS_IN_WEEK)
        } else {
            Self::calc_dest_time(base, date, i64::from(self.interval) * MILLIS_IN_DAY)
        }
    }

    /// Java `calcDestTime`.
    fn calc_dest_time(time_value: i64, date: i64, add: i64) -> i64 {
        let mut time = time_value;
        if time < date {
            time += ((date - time) / add) * add;
            if time < date {
                time += add;
            }
        }
        time
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HOUR: i64 = 3_600_000;

    fn utc(_: i64) -> i64 {
        0
    }

    /// 2026-10-27 (a Tuesday) 12:00:00 UTC.
    const TUESDAY_NOON: i64 = 1_793_102_400_000;

    #[test]
    fn the_settings_are_checked_like_java() {
        // 5 = Friday of the XML (1 = Monday), Calendar 6.
        let friday = DateGenerator::new(None, Some(5), Some(17), None).unwrap();
        assert_eq!((friday.day_of_week, friday.minute_of_hour), (6, 0));
        // 7 = Sunday of the XML wraps to Calendar 1.
        assert_eq!(
            DateGenerator::new(None, Some(7), Some(0), Some(0))
                .unwrap()
                .day_of_week,
            1
        );
        // No day and no interval, or both, or a bad hour: refused.
        assert!(DateGenerator::new(None, None, Some(1), None).is_err());
        assert!(DateGenerator::new(Some(3), Some(2), Some(1), None).is_err());
        assert!(DateGenerator::new(None, Some(2), Some(24), None).is_err());
        assert!(DateGenerator::new(None, Some(2), None, None).is_err());
        // An interval of 1 together with a day is accepted (Java checks `interval > 1`).
        assert!(DateGenerator::new(Some(1), Some(2), Some(1), None).is_ok());
        // A bad minute falls back to 0.
        assert_eq!(
            DateGenerator::new(Some(1), None, Some(1), Some(99))
                .unwrap()
                .minute_of_hour,
            0
        );
    }

    #[test]
    fn a_weekday_gives_the_next_such_instant() {
        let friday = DateGenerator::new(None, Some(5), Some(17), Some(30)).unwrap();
        // Tuesday noon: Friday 17:30 of the same week, 3 days and 5.5 hours later.
        assert_eq!(
            friday.next_date_in_zone(TUESDAY_NOON, utc),
            TUESDAY_NOON + 3 * MILLIS_IN_DAY + 5 * HOUR + HOUR / 2
        );
        // The same Friday at 18:00: a week later.
        let friday_18 = TUESDAY_NOON + 3 * MILLIS_IN_DAY + 6 * HOUR;
        assert_eq!(
            friday.next_date_in_zone(friday_18, utc),
            TUESDAY_NOON + 10 * MILLIS_IN_DAY + 5 * HOUR + HOUR / 2
        );
        // Exactly on the instant: itself (Java returns a time that is not before the date).
        let on = TUESDAY_NOON + 3 * MILLIS_IN_DAY + 5 * HOUR + HOUR / 2;
        assert_eq!(friday.next_date_in_zone(on, utc), on);
        // A Monday target (XML 1) on a Tuesday: six days ahead.
        let monday = DateGenerator::new(None, Some(1), Some(12), None).unwrap();
        assert_eq!(
            monday.next_date_in_zone(TUESDAY_NOON, utc),
            TUESDAY_NOON + 6 * MILLIS_IN_DAY
        );
        // Sunday (XML 7).
        let sunday = DateGenerator::new(None, Some(7), Some(12), None).unwrap();
        assert_eq!(
            sunday.next_date_in_zone(TUESDAY_NOON, utc),
            TUESDAY_NOON + 5 * MILLIS_IN_DAY
        );
    }

    #[test]
    fn an_interval_steps_from_today() {
        let daily = DateGenerator::new(Some(1), None, Some(6), Some(0)).unwrap();
        assert_eq!(
            daily.next_date_in_zone(TUESDAY_NOON, utc),
            TUESDAY_NOON + 18 * HOUR
        );
        let every_third_day = DateGenerator::new(Some(3), None, Some(13), Some(0)).unwrap();
        // 13:00 today has not passed.
        assert_eq!(
            every_third_day.next_date_in_zone(TUESDAY_NOON, utc),
            TUESDAY_NOON + HOUR
        );
        // 13:00 today has passed: three days from today's 13:00.
        assert_eq!(
            every_third_day.next_date_in_zone(TUESDAY_NOON + 2 * HOUR, utc),
            TUESDAY_NOON + HOUR + 3 * MILLIS_IN_DAY
        );
    }

    #[test]
    fn the_zone_moves_the_wall_clock() {
        // UTC+2: Tuesday 12:00 UTC is 14:00 there, so "17:00 every day" is 15:00 UTC the same day.
        let daily = DateGenerator::new(Some(1), None, Some(17), Some(0)).unwrap();
        assert_eq!(
            daily.next_date_in_zone(TUESDAY_NOON, |_| 2 * HOUR),
            TUESDAY_NOON + 3 * HOUR
        );
    }
}
