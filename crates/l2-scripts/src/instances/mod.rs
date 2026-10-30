//! `data/scripts/instances`: the instance zone scripts (Kamaloka, Pailaka, ...).
//!
//! One module per script, sorted by name; one `add` line per script in [`register`], in the same
//! order (Java's load order is the folder listing).

pub mod castle_dungeon;
pub mod cavern_of_the_pirate_captain;
pub(crate) mod chamber;
pub mod chamber_of_delusion_east;
pub mod chamber_of_delusion_north;
pub mod chamber_of_delusion_south;
pub mod chamber_of_delusion_square;
pub mod chamber_of_delusion_tower;
pub mod chamber_of_delusion_west;
pub mod crystal_caverns;
pub mod dark_cloud_mansion;
pub mod ice_queens_castle;
pub mod ice_queens_castle_battle;
pub(crate) mod jinia_guild_hideout;
pub mod jinia_guild_hideout1;
pub mod jinia_guild_hideout2;
pub mod jinia_guild_hideout3;
pub mod jinia_guild_hideout4;
pub mod kamaloka;
pub mod mithril_mine;
pub mod nornils_garden;
pub mod nornils_garden_quest;
pub mod pailaka_devils_legacy;
pub mod pailaka_injured_dragon;
pub mod pailaka_song_of_ice_and_fire;
pub mod rim_kamaloka;

use std::collections::HashMap;

use l2_util::ObjectId;
use l2_world::ScriptRegistry;

/// Java `Npc._scriptValue` (`getScriptValue`, `isScriptValue`, `setScriptValue`) of the NPCs one
/// script cares about. A script holds no state (D-10), so the value lives in its `ctx.state`.
#[derive(Debug, Default)]
pub(crate) struct ScriptValues(HashMap<ObjectId, i32>);

impl ScriptValues {
    /// Java `npc.getScriptValue()`: 0 until it is set.
    pub(crate) fn get(&self, npc: ObjectId) -> i32 {
        self.0.get(&npc).copied().unwrap_or(0)
    }

    /// Java `npc.isScriptValue(value)`.
    pub(crate) fn is(&self, npc: ObjectId, value: i32) -> bool {
        self.get(npc) == value
    }

    /// Java `npc.setScriptValue(value)`.
    pub(crate) fn set(&mut self, npc: ObjectId, value: i32) {
        self.0.insert(npc, value);
    }

    /// The NPC is gone (Java's object is garbage collected).
    pub(crate) fn forget(&mut self, npc: ObjectId) {
        self.0.remove(&npc);
    }
}

const DAY_MS: i64 = 86_400_000;

/// The re-entry time of Kamaloka and Rim Kamaloka: the next 06:30 on the server's clock (D-48),
/// with the seconds and milliseconds of `now` (`Calendar.set` leaves them alone).
///
/// Java: `reenter.set(MINUTE, 30); if (reenter.get(HOUR_OF_DAY) >= 6) { reenter.add(DATE, 1) }
/// (Kamaloka) or reenter.roll(DATE, true) (Rim Kamaloka); reenter.set(HOUR_OF_DAY, 6)`.
///
/// NOTE(java-quirk): `Calendar.roll(DATE, true)` does not carry into the month: on the last day
/// of a month the date becomes the 1st of the same month, a time in the past, so Rim Kamaloka
/// restricts nobody on that day after 06:00.
pub(crate) fn kamaloka_reset_time(now_utc: i64, offset_ms: i64, roll: bool) -> i64 {
    const RESET_HOUR: i64 = 6;
    const RESET_MIN: i64 = 30;
    let local = now_utc + offset_ms;
    let day_start = local.div_euclid(DAY_MS) * DAY_MS;
    let hour = (local - day_start) / 3_600_000;
    let seconds_and_millis = local.rem_euclid(60_000);
    let mut day = day_start;
    if hour >= RESET_HOUR {
        day = if roll {
            roll_date_up(day_start)
        } else {
            day_start + DAY_MS
        };
    }
    day + RESET_HOUR * 3_600_000 + RESET_MIN * 60_000 + seconds_and_millis - offset_ms
}

/// `Calendar.roll(DATE, true)` of a local day start: the next day, or the 1st of the same month
/// from the last day.
fn roll_date_up(day_start: i64) -> i64 {
    let (year, month, day) = civil_from_days(day_start.div_euclid(DAY_MS));
    if day == days_in_month(year, month) {
        day_start - (i64::from(day) - 1) * DAY_MS
    } else {
        day_start + DAY_MS
    }
}

/// (year, month 1..=12, day 1..=31) of a day number since 1970-01-01 (proleptic Gregorian).
fn civil_from_days(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = u32::try_from(doy - (153 * mp + 2) / 5 + 1).unwrap_or(1);
    let month = u32::try_from(if mp < 10 { mp + 3 } else { mp - 9 }).unwrap_or(1);
    let year = yoe + era * 400 + i64::from(month <= 2);
    (year, month, day)
}

fn days_in_month(year: i64, month: u32) -> u32 {
    match month {
        2 if year % 4 == 0 && (year % 100 != 0 || year % 400 == 0) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2026-10-28 (a Wednesday) 10:15:20.250 UTC.
    const WED: i64 = 1_793_182_520_250;
    const MIDNIGHT: i64 = WED - (10 * 3_600_000 + 15 * 60_000 + 20_250);

    #[test]
    fn the_reset_is_the_next_0630_with_the_seconds_of_now() {
        // After 06:00: tomorrow.
        assert_eq!(
            kamaloka_reset_time(WED, 0, false),
            MIDNIGHT + DAY_MS + 6 * 3_600_000 + 30 * 60_000 + 20_250
        );
        // Before 06:00: today.
        let early = MIDNIGHT + 3_600_000 + 20_250;
        assert_eq!(
            kamaloka_reset_time(early, 0, false),
            MIDNIGHT + 6 * 3_600_000 + 30 * 60_000 + 20_250
        );
        // Local time two hours ahead: 12:15 local, so tomorrow 06:30 local = 04:30 UTC.
        assert_eq!(
            kamaloka_reset_time(WED, 2 * 3_600_000, false),
            MIDNIGHT + DAY_MS + 4 * 3_600_000 + 30 * 60_000 + 20_250
        );
    }

    #[test]
    fn rolling_the_date_does_not_leave_the_month() {
        assert_eq!(
            kamaloka_reset_time(WED, 0, true),
            kamaloka_reset_time(WED, 0, false)
        );
        // 2026-10-31 12:00: the 1st of October, 06:30, in the past.
        let last_day = MIDNIGHT + 3 * DAY_MS + 12 * 3_600_000;
        let rolled = kamaloka_reset_time(last_day, 0, true);
        assert_eq!(rolled, MIDNIGHT - 27 * DAY_MS + 6 * 3_600_000 + 30 * 60_000);
        assert!(rolled < last_day);
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(MIDNIGHT / DAY_MS), (2026, 10, 28));
        assert_eq!(days_in_month(2028, 2), 29);
        assert_eq!(days_in_month(2100, 2), 28);
    }
}

/// Registers every instance script, folder by folder in Java's order.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(castle_dungeon::CastleDungeon);
    r.add(cavern_of_the_pirate_captain::CavernOfThePirateCaptain);
    r.add(chamber_of_delusion_east::ChamberOfDelusionEast);
    r.add(chamber_of_delusion_north::ChamberOfDelusionNorth);
    r.add(chamber_of_delusion_south::ChamberOfDelusionSouth);
    r.add(chamber_of_delusion_square::ChamberOfDelusionSquare);
    r.add(chamber_of_delusion_tower::ChamberOfDelusionTower);
    r.add(chamber_of_delusion_west::ChamberOfDelusionWest);
    r.add(crystal_caverns::CrystalCaverns);
    r.add(dark_cloud_mansion::DarkCloudMansion);
    r.add(ice_queens_castle::IceQueensCastle);
    r.add(ice_queens_castle_battle::IceQueensCastleBattle);
    r.add(jinia_guild_hideout1::JiniaGuildHideout1);
    r.add(jinia_guild_hideout2::JiniaGuildHideout2);
    r.add(jinia_guild_hideout3::JiniaGuildHideout3);
    r.add(jinia_guild_hideout4::JiniaGuildHideout4);
    r.add(kamaloka::Kamaloka);
    r.add(mithril_mine::MithrilMine);
    r.add(nornils_garden::NornilsGarden);
    r.add(nornils_garden_quest::NornilsGardenQuest);
    r.add(pailaka_devils_legacy::PailakaDevilsLegacy);
    r.add(pailaka_injured_dragon::PailakaInjuredDragon);
    r.add(pailaka_song_of_ice_and_fire::PailakaSongOfIceAndFire);
    r.add(rim_kamaloka::RimKamaloka);
}

/// Helpers of the instance script tests.
#[cfg(test)]
pub(crate) mod testing {
    use std::sync::Arc;

    use l2_data::InstanceTemplates;

    /// The instance names (`InstanceNames.xml`) and templates (`data/instances/*.xml`) of a test:
    /// `(id, name)` and `(id, the children of <instance>)`.
    pub(crate) fn instance_data(names: &[(i32, &str)], templates: &[(i32, &str)]) -> Arc<InstanceTemplates> {
        let mut data = InstanceTemplates::default();
        let list: String = names
            .iter()
            .map(|(id, name)| format!(r#"<instance id="{id}" name="{name}" />"#))
            .collect();
        data.add_names(&format!("<list>{list}</list>")).unwrap();
        for (id, body) in templates {
            data.add_template(
                &format!(r#"<instance id="{id}">{body}</instance>"#),
                &format!("T{id}.xml"),
            );
        }
        Arc::new(data)
    }
}
