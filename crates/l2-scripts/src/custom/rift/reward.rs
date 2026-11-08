//! The rewards of a rift, without the world: who earned how many shards, the weekly limit and
//! the week it counts in.

use std::collections::BTreeMap;

use l2_util::ObjectId;

/// One wave, or the boss, of a rift: what each character did against it.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Stage {
    pub boss: bool,
    /// Every monster of it died.
    pub cleared: bool,
    /// Damage dealt to its monsters, heals of participants while it was out, and presence.
    pub contribution: BTreeMap<ObjectId, f64>,
}

/// The shard settings of `L2R.ini`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rules {
    pub per_wave: i64,
    pub boss: i64,
    pub top_bonus: i64,
    /// The share of a stage's total a character needs for its shards (anti-AFK).
    pub min_percent: f64,
}

/// The characters with at least `min_percent` of the stage's total contribution.
pub fn eligible(stage: &Stage, min_percent: f64) -> Vec<ObjectId> {
    let total: f64 = stage.contribution.values().sum();
    if total <= 0.0 {
        return Vec::new();
    }
    stage
        .contribution
        .iter()
        .filter(|(_, c)| **c > 0.0 && **c * 100.0 / total >= min_percent)
        .map(|(p, _)| *p)
        .collect()
}

/// The shards each character earned: the shards of every beaten stage it was eligible for,
/// and, when the boss died, the bonus for the three with the most contribution in the whole rift
/// (among those who earned shards; a tie goes to the lower object id).
pub fn awards(stages: &[Stage], rules: &Rules, completed: bool) -> BTreeMap<ObjectId, i64> {
    let mut shards: BTreeMap<ObjectId, i64> = BTreeMap::new();
    for stage in stages.iter().filter(|s| s.cleared) {
        let amount = if stage.boss { rules.boss } else { rules.per_wave };
        for player in eligible(stage, rules.min_percent) {
            *shards.entry(player).or_default() += amount;
        }
    }
    if completed && rules.top_bonus > 0 {
        for player in top_contributors(stages, &shards, 3) {
            *shards.entry(player).or_default() += rules.top_bonus;
        }
    }
    shards.retain(|_, n| *n > 0);
    shards
}

/// The `n` characters of `among` with the most contribution over every stage.
pub fn top_contributors(stages: &[Stage], among: &BTreeMap<ObjectId, i64>, n: usize) -> Vec<ObjectId> {
    let mut totals: BTreeMap<ObjectId, f64> = BTreeMap::new();
    for stage in stages {
        for (player, c) in &stage.contribution {
            if among.contains_key(player) {
                *totals.entry(*player).or_default() += c;
            }
        }
    }
    let mut ranked: Vec<(ObjectId, f64)> = totals.into_iter().collect();
    ranked.sort_by(|a, b| b.1.total_cmp(&a.1).then(a.0.cmp(&b.0)));
    ranked.into_iter().take(n).map(|(p, _)| p).collect()
}

/// What may still be given of `amount` this week, `already` given and a weekly `cap` (0: none).
pub fn capped(amount: i64, already: i64, cap: i64) -> i64 {
    if cap <= 0 {
        return amount.max(0);
    }
    amount.clamp(0, (cap - already).max(0))
}

const DAY_MS: i64 = 86_400_000;
const WEEK_MS: i64 = 7 * DAY_MS;

/// The start of the week `local_ms` lies in (local wall-clock milliseconds): the last weekly
/// reset at or before it, on `day` (1 Monday ... 7 Sunday) at `hour`.
pub fn week_start(local_ms: i64, day: i32, hour: i32) -> i64 {
    let days = local_ms.div_euclid(DAY_MS);
    // 1970-01-01 was a Thursday (ISO day 4).
    let weekday = (days + 3).rem_euclid(7) + 1;
    let back = (weekday - i64::from(day)).rem_euclid(7);
    let mut reset = (days - back) * DAY_MS + i64::from(hour) * 3_600_000;
    if reset > local_ms {
        reset -= WEEK_MS;
    }
    reset
}

#[cfg(test)]
mod tests {
    use super::*;

    const RULES: Rules = Rules {
        per_wave: 5,
        boss: 15,
        top_bonus: 5,
        min_percent: 1.0,
    };

    fn stage(boss: bool, cleared: bool, of: &[(i32, f64)]) -> Stage {
        Stage {
            boss,
            cleared,
            contribution: of.iter().map(|(p, c)| (ObjectId(*p), *c)).collect(),
        }
    }

    fn of(map: &BTreeMap<ObjectId, i64>) -> Vec<(i32, i64)> {
        map.iter().map(|(p, n)| (p.0, *n)).collect()
    }

    #[test]
    fn the_minimum_share_keeps_afk_characters_out() {
        // 1 has 98.5 %, 2 has 1 % (just enough), 3 has 0.5 % (presence only).
        let s = stage(false, true, &[(1, 985.0), (2, 10.0), (3, 5.0)]);
        assert_eq!(eligible(&s, 1.0), vec![ObjectId(1), ObjectId(2)]);
        assert_eq!(eligible(&s, 0.0), vec![ObjectId(1), ObjectId(2), ObjectId(3)]);
        assert!(eligible(&stage(false, true, &[]), 1.0).is_empty());
        assert!(eligible(&stage(false, true, &[(1, 0.0)]), 0.0).is_empty());
    }

    #[test]
    fn beaten_waves_and_the_boss_pay_and_the_top_three_get_the_bonus() {
        let stages = [
            stage(
                false,
                true,
                &[(1, 500.0), (2, 300.0), (3, 100.0), (4, 100.0), (5, 1.0)],
            ),
            stage(false, true, &[(1, 500.0), (2, 300.0)]),
            // Not beaten: nothing for it.
            stage(false, false, &[(3, 900.0)]),
            stage(true, true, &[(1, 100.0), (4, 900.0)]),
        ];
        let won = awards(&stages, &RULES, true);
        // 5 has 0.1 % of wave 1: nothing. Totals: 1 1100, 4 1000, 3 1000 (wave 3 counts for
        // the ranking), 2 600: the top three are 1, 3 and 4.
        assert_eq!(
            of(&won),
            vec![(1, 5 + 5 + 15 + 5), (2, 10), (3, 5 + 5), (4, 5 + 15 + 5)]
        );
        // Failed: the beaten waves still pay; no bonus.
        let mut failed = stages.clone();
        failed[3].cleared = false;
        assert_eq!(
            of(&awards(&failed, &RULES, false)),
            vec![(1, 10), (2, 10), (3, 5), (4, 5)]
        );
    }

    #[test]
    fn a_tie_for_the_top_goes_to_the_lower_id() {
        let stages = [stage(
            true,
            true,
            &[(7, 100.0), (3, 100.0), (5, 100.0), (9, 100.0)],
        )];
        let won = awards(&stages, &RULES, true);
        assert_eq!(of(&won), vec![(3, 20), (5, 20), (7, 20), (9, 15)]);
    }

    #[test]
    fn the_weekly_cap_limits_what_is_given() {
        assert_eq!(capped(30, 0, 400), 30);
        assert_eq!(capped(30, 390, 400), 10);
        assert_eq!(capped(30, 400, 400), 0);
        assert_eq!(capped(30, 450, 400), 0);
        // 0: no cap.
        assert_eq!(capped(30, 10_000, 0), 30);
    }

    #[test]
    fn the_week_starts_at_the_reset_day_and_hour() {
        const HOUR: i64 = 3_600_000;
        // 2026-11-04 is a Wednesday (ISO 3); 2026-11-04 00:00 local = day 20_761.
        let wednesday = 20_761 * DAY_MS;
        assert_eq!((20_761 + 3) % 7 + 1, 3);
        // Wednesday 06:00 starts the week; 05:59 still belongs to the week before.
        let reset = wednesday + 6 * HOUR;
        assert_eq!(week_start(reset, 3, 6), reset);
        assert_eq!(week_start(reset - 60_000, 3, 6), reset - WEEK_MS);
        assert_eq!(week_start(reset + 3 * DAY_MS, 3, 6), reset);
        // Tuesday 23:00 of the next week: still this week.
        assert_eq!(week_start(reset + 6 * DAY_MS + 17 * HOUR, 3, 6), reset);
        assert_eq!(week_start(reset + WEEK_MS, 3, 6), reset + WEEK_MS);
        // Sunday 0:00 resets.
        let sunday = wednesday + 4 * DAY_MS;
        assert_eq!(week_start(sunday + HOUR, 7, 0), sunday);
        assert_eq!(week_start(sunday - 1, 7, 0), sunday - WEEK_MS);
        // Before 1970 too.
        assert!(week_start(-5 * DAY_MS, 1, 0) <= -5 * DAY_MS);
    }
}
