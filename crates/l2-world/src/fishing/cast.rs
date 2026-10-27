//! Casting the line: the `Fishing` skill effect, `Player.startFishing`, the bite task and
//! `Player.endFishing`.
// Java: dist/game/data/scripts/handlers/skill/effects/Fishing.java
// Java: entity/actor/Player.java (startFishing, stop/startLookingForFishTask, getRandomFishGrade,
//       getRandomFishGroup, getRandomFishLvl, startFishCombat, endFishing)
// Java: entity/actor/tasks/player/LookingForFishTask.java

use l2_data::item::types::{EtcItemType, WeaponType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::charscreen::paperdoll as pd;
use l2_protocol_game::server::{ExFishingEnd, ExFishingStart, PlaySound, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::{FISHERMANS_POTION_SKILL, FISHING_EXPERTISE_SKILL, FixedRate, LookingForFish};
use crate::inventory::{etc_type, weapon_type};
use crate::timer::{TimerId, TimerKind};
use crate::world::World;
use crate::zones::ZoneId;

/// `Fishing.MIN_BAIT_DISTANCE`.
const MIN_BAIT_DISTANCE: i32 = 90;
/// `Fishing.MAX_BAIT_DISTANCE`.
const MAX_BAIT_DISTANCE: i32 = 250;
/// `LookingForFishTask`: the first bite check comes 10 seconds after the cast, and the bait is
/// lost 10 seconds after `startCombatTime`.
const FIRST_CHECK_MS: u64 = 10_000;

/// Java `Item.isNightLure()`.
pub(crate) fn is_night_lure(item_id: i32) -> bool {
    (8505..=8513).contains(&item_id) || item_id == 8485
}

/// Java `Player.getRandomFishGrade`: `0` for the beginner lures, `2` for the luminous ones.
fn random_fish_grade(lure_id: i32) -> i32 {
    match lure_id {
        // green, purple, yellow and prize-winning for beginners
        7807 | 7808 | 7809 | 8486 => 0,
        // prize-winning luminous, green, purple and yellow luminous
        8485 | 8506 | 8509 | 8512 => 2,
        _ => 1,
    }
}

/// The three-way draw of `getRandomFishGroup`: the preferred type up to `first`, the next up to
/// `second`, else the last.
fn pick(check: i32, first: i32, second: i32, types: [i32; 3]) -> i32 {
    if check <= first {
        types[0]
    } else if check <= second {
        types[1]
    } else {
        types[2]
    }
}

/// Java `Player.getRandomFishGroup(group)` for the `check = Rnd.get(100)` already drawn. `group`
/// is the grade of `getRandomFishGrade`.
// NOTE(java-quirk): a lure that is none of the listed ones leaves the type at 1, whatever the
// grade, so a beginner fish (grade 0) of type 1 does not exist and the cast ends with "Fish are
// not defined".
fn random_fish_group_for(grade: i32, lure_id: i32, check: i32) -> i32 {
    let mut kind = 1;
    match grade {
        // fish for novices
        0 => match lure_id {
            7807 => kind = pick(check, 54, 77, [5, 4, 6]),
            7808 => kind = pick(check, 54, 77, [4, 6, 5]),
            7809 => kind = pick(check, 54, 77, [6, 5, 4]),
            8486 => kind = pick(check, 33, 66, [4, 5, 6]),
            _ => {}
        },
        // normal fish
        1 => match lure_id {
            7610..=7613 => kind = 3,
            // the green lures prefer fast-moving (nimble) fish (type 1)
            6519 | 8505 | 6520 | 6521 | 8507 => {
                kind = if check <= 54 {
                    1
                } else if check <= 74 {
                    0
                } else if check <= 94 {
                    2
                } else {
                    3
                };
            }
            // the purple lures prefer fat fish (type 0)
            6522 | 8508 | 6523 | 6524 | 8510 => {
                kind = if check <= 54 {
                    0
                } else if check <= 74 {
                    1
                } else if check <= 94 {
                    2
                } else {
                    3
                };
            }
            // the yellow lures prefer ugly fish (type 2); this one is 55, not 54
            6525 | 8511 | 6526 | 6527 | 8513 => {
                kind = if check <= 55 {
                    2
                } else if check <= 74 {
                    1
                } else if check <= 94 {
                    0
                } else {
                    3
                };
            }
            // prize-winning fishing lure
            8484 => kind = pick(check, 33, 66, [0, 1, 2]),
            _ => {}
        },
        // upper grade fish, luminous lure
        2 => match lure_id {
            8506 => kind = pick(check, 54, 77, [8, 7, 9]),
            8509 => kind = pick(check, 54, 77, [7, 9, 8]),
            8512 => kind = pick(check, 54, 77, [9, 8, 7]),
            8485 => kind = pick(check, 33, 66, [7, 8, 9]),
            _ => {}
        },
        _ => {}
    }
    kind
}

/// `startLookingForFishTask`: the delay between two bite checks for a lure.
// NOTE(java-quirk): the ranges overlap. 8507, 8510 and 8513 are in the `8505..=8513` range of the
// second branch, so the third branch (x66) is never reached for them.
fn check_delay_ms(lure_id: i32, guts_check_time: i32) -> i32 {
    if matches!(lure_id, 6519 | 6522 | 6525 | 8505 | 8508 | 8511) {
        guts_check_time.wrapping_mul(133)
    } else if matches!(lure_id, 6520 | 6523 | 6526)
        || (8505..=8513).contains(&lure_id)
        || (7610..=7613).contains(&lure_id)
        || (7807..=7809).contains(&lure_id)
        || (8484..=8486).contains(&lure_id)
    {
        guts_check_time.wrapping_mul(100)
    } else if matches!(lure_id, 6521 | 6524 | 6527 | 8507 | 8510 | 8513) {
        guts_check_time.wrapping_mul(66)
    } else {
        0
    }
}

impl World {
    /// Java `Fishing.onStart(effector, effected, skill)`: the `Fishing` skill (1312) landed.
    pub(crate) fn fishing_effect_start(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.settings.fishing.allow_fishing && !self.is_gm(id) {
            self.send_message(id, "Fishing is disabled!");
            return;
        }

        if self.is_fishing(id) {
            if self.has_fish_combat(id) {
                self.fish_do_die(id, false);
            } else {
                self.end_fishing(id, false);
            }
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourAttemptAtFishingHasBeenCancelled),
            );
            return;
        }

        // Check for equipped fishing rod.
        let has_rod = self
            .stats(id)
            .and_then(|v| v.active_weapon_item())
            .and_then(weapon_type)
            == Some(WeaponType::FishingRod);
        if !has_rod {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveAFishingPoleEquipped),
            );
            return;
        }

        // Check for equipped lure.
        let templates = self.item_templates();
        let lure = self
            .paperdoll_object(id, pd::LHAND)
            .and_then(|o| self.inventory_item(id, o))
            .filter(|item| {
                templates
                    .as_deref()
                    .and_then(|t| t.get(item.item_id))
                    .and_then(etc_type)
                    == Some(EtcItemType::Lure)
            })
            .map(|item| (item.object_id, item.item_id));
        let Some((lure_object, lure_item_id)) = lure else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouMustPutBaitOnYourHookBeforeYouCanFish),
            );
            return;
        };

        if self.is_in_boat(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouCannotFishWhileRidingAsAPassengerOfABoatItSAgainstTheRules,
                ),
            );
            return;
        }

        if self.is_crafting(id) || self.is_in_store_mode(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouCannotFishWhileUsingARecipeBookPrivateManufactureOrPrivateStore,
                ),
            );
            return;
        }

        if self.is_inside_zone(id, ZoneId::Water) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouCannotFishWhileUnderWater),
            );
            return;
        }

        // Calculate a position in front of the player with a random distance.
        let Some((px, py, _)) = self.position_of(id) else {
            return;
        };
        let heading = self.players.get(&id).map_or(0, |p| p.core.heading);
        let mut distance = self.rng().get(MIN_BAIT_DISTANCE, MAX_BAIT_DISTANCE);
        // `LocationUtil.convertHeadingToDegree`, `Math.toRadians`.
        let radian = (f64::from(heading) / 182.044_444_444).to_radians();
        let (sin, cos) = radian.sin_cos();
        let bait_at = |distance: i32| {
            (
                (f64::from(px) + cos * f64::from(distance)) as i32,
                (f64::from(py) + sin * f64::from(distance)) as i32,
            )
        };
        let (mut bait_x, mut bait_y) = bait_at(distance);

        // Search for fishing and water zone.
        let mut bait_z = self.compute_bait_z(id, bait_x, bait_y);
        if bait_z.is_none() {
            distance = MAX_BAIT_DISTANCE;
            while distance >= MIN_BAIT_DISTANCE {
                (bait_x, bait_y) = bait_at(distance);
                bait_z = self.compute_bait_z(id, bait_x, bait_y);
                if bait_z.is_some() {
                    break;
                }
                distance -= 1;
            }
            if bait_z.is_none() {
                self.send(id, &SystemMessage::new(SystemMessageId::YouCanTFishHere));
                return;
            }
        }
        let Some(bait_z) = bait_z else {
            return;
        };

        if !self.destroy_item_by_object_id(id, lure_object, 1, false) {
            self.send(id, &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughBait));
            return;
        }

        if let Some(player) = self.players.get_mut(&id) {
            player.fishing.lure_item_id = Some(lure_item_id);
        }
        self.start_fishing(id, bait_x, bait_y, bait_z);
    }

    /// The zones at the bait: the fishing zone and the water zone (Java walks
    /// `ZoneManager.getZones(x, y)` and keeps the last of each class, stopping when it has both),
    /// and then `computeBaitZ`. `None` is Java's `Integer.MIN_VALUE`: you cannot fish here.
    fn compute_bait_z(&self, id: ObjectId, bait_x: i32, bait_y: i32) -> Option<i32> {
        let zones = self.zoning.zones.as_ref()?;
        let mut fishing_zone = false;
        let mut water_z = None;
        for zone in zones.zones_at_xy(bait_x, bait_y) {
            match zone.zone_type.as_str() {
                "FishingZone" => fishing_zone = true,
                "WaterZone" => water_z = Some(zone.form.high_z()),
                _ => {}
            }
            if fishing_zone && water_z.is_some() {
                break;
            }
        }
        if !fishing_zone {
            return None;
        }
        // Always use the water zone, the fishing zone's high Z is high in the air...
        let bait_z = water_z?;
        if !self.can_see_point(id, (bait_x, bait_y, bait_z)) {
            return None;
        }
        if let Some(geo) = &self.geo
            && geo.has_geo(bait_x, bait_y)
        {
            if geo.height(bait_x, bait_y, bait_z) > bait_z {
                return None;
            }
            let player_z = self.position_of(id).map_or(0, |p| p.2);
            if geo.height(bait_x, bait_y, player_z) > bait_z {
                return None;
            }
        }
        Some(bait_z)
    }

    /// Java `GeoEngine.canSeeTarget(creature, position)`: no door or fence on the line and the
    /// geodata lets sight through.
    fn can_see_point(&self, id: ObjectId, to: (i32, i32, i32)) -> bool {
        use l2_geo::DynamicObstacles;

        let Some(from) = self.position_of(id) else {
            return false;
        };
        let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        let from = l2_geo::Location::new(from.0, from.1, from.2);
        let to = l2_geo::Location::new(to.0, to.1, to.2);
        let obstacles = self.obstacles();
        if obstacles.doors_between(from, to, instance, true) || obstacles.fence_between(from, to, instance) {
            return false;
        }
        self.geo.as_ref().is_none_or(|geo| geo.can_see_target(from, to))
    }

    /// Java `Player.startFishing(x, y, z)`.
    fn start_fishing(&mut self, id: ObjectId, x: i32, y: i32, z: i32) {
        self.stop_move(id, None);
        self.set_immobilized(id, true);
        if let Some(player) = self.players.get_mut(&id) {
            player.fishing.fishing = true;
            player.fishing.fish_x = x;
            player.fishing.fish_y = y;
            player.fishing.fish_z = z;
        }

        // Starts fishing.
        let Some(lure_id) = self.players.get(&id).and_then(|p| p.fishing.lure_item_id) else {
            return;
        };
        let level = self.random_fish_lvl(id);
        let grade = random_fish_grade(lure_id);
        let check = self.rng().below(100);
        let group = random_fish_group_for(grade, lure_id, check);
        let candidates: Vec<l2_data::Fish> = self
            .fishing
            .fish
            .as_ref()
            .map(|data| data.get_fish(level, group, grade).into_iter().cloned().collect())
            .unwrap_or_default();
        if candidates.is_empty() {
            self.send_message(id, "Error - Fish are not defined");
            self.end_fishing(id, false);
            return;
        }

        // Use a copy, else the fish data may be over-written below.
        let pick = usize::try_from(
            self.rng()
                .below(i32::try_from(candidates.len()).unwrap_or(i32::MAX)),
        )
        .unwrap_or(0);
        let Some(mut fish) = candidates.into_iter().nth(pick) else {
            return;
        };
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouCastYourLineAndStartToFish),
        );
        // Night lures only catch at night, ordinary lures only during the day.
        let night_lure = is_night_lure(lure_id);
        let night = self
            .settings
            .fishing
            .clock_override_night
            .or(self.game_clock_night)
            .unwrap_or(false);
        if night != night_lure {
            fish.fish_group = -1;
        }
        let fish_group = fish.fish_group;
        if let Some(player) = self.players.get_mut(&id) {
            player.fishing.fish = Some(fish);
        }

        self.broadcast_creature_packet(
            id,
            &ExFishingStart {
                object_id: id.0,
                fish_type: fish_group,
                x,
                y,
                z,
                night_lure,
            },
        );
        self.send(id, &PlaySound::with_unknown1(1, "SF_P_01"));
        self.start_looking_for_fish_task(id);
    }

    /// Java `Player.getRandomFishLvl`.
    fn random_fish_lvl(&mut self, id: ObjectId) -> i32 {
        let mut skill_level = self
            .stats(id)
            .map_or(0, |v| v.skill_level(FISHING_EXPERTISE_SKILL));
        // Fisherman's Potion: the skill's power replaces the expertise level.
        let potion = self
            .effect_list(id)
            .and_then(|l| l.info_by_skill_id(FISHERMANS_POTION_SKILL))
            .map(|info| info.skill());
        if let Some(key) = potion
            && let Some(skill) = self.skill_table().and_then(|t| t.get(key.id, key.level).cloned())
        {
            skill_level = skill.power as i32;
        }
        if skill_level <= 0 {
            return 1;
        }
        let check = self.rng().below(100);
        if check <= 50 {
            skill_level
        } else if check <= 85 {
            (skill_level - 1).max(1)
        } else {
            (skill_level + 1).min(27)
        }
    }

    /// Java `Player.stopLookingForFishTask`.
    pub(crate) fn stop_looking_for_fish_task(&mut self, id: ObjectId) {
        let timer = self
            .players
            .get_mut(&id)
            .and_then(|p| p.fishing.look.take())
            .and_then(|l| l.timer);
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    /// Java `Player.startLookingForFishTask`.
    fn start_looking_for_fish_task(&mut self, id: ObjectId) {
        if self.is_dead(id) {
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if player.fishing.look.is_some() {
            return;
        }
        let (Some(fish), Some(lure_id)) = (player.fishing.fish.as_ref(), player.fishing.lure_item_id) else {
            return;
        };
        let is_noob = fish.fish_grade == 0;
        let is_upper_grade = fish.fish_grade == 2;
        let check_delay = check_delay_ms(lure_id, fish.guts_check_time);
        let start_combat_time = fish.start_combat_time;
        let Ok(period) = u64::try_from(check_delay) else {
            return;
        };
        if period == 0 {
            // NOTE(java-quirk): `ThreadPool.scheduleAtFixedRate` refuses a period of 0, logs it and
            // returns null: no bite task runs and only the `Fishing` skill ends the cast.
            warn!(
                "ThreadPool: Failed to schedule recurring task LookingForFishTask: lure {lure_id} has no bite delay."
            );
            return;
        }
        let end_time_ms = self.world_ms()
            + i64::from(start_combat_time) * 1000
            + i64::try_from(FIRST_CHECK_MS).unwrap_or(10_000);
        let mut rate = FixedRate::new(self.tick, FIRST_CHECK_MS, period);
        let due = rate.next_due(self.tick);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Fishing(super::FishingTimer::LookForFish(id)));
        if let Some(player) = self.players.get_mut(&id) {
            player.fishing.look = Some(LookingForFish {
                timer: Some(timer),
                rate,
                end_time_ms,
                is_noob,
                is_upper_grade,
            });
        }
    }

    /// Java `LookingForFishTask.run`.
    pub(crate) fn on_look_for_fish_timer(&mut self, id: ObjectId) {
        let now_ms = self.world_ms();
        let tick = self.tick;
        // The task repeats at a fixed rate until it is cancelled.
        let Some(look) = self.players.get_mut(&id).and_then(|p| p.fishing.look.as_mut()) else {
            return;
        };
        let due = look.rate.next_due(tick);
        let (end_time_ms, is_noob, is_upper_grade) = (look.end_time_ms, look.is_noob, look.is_upper_grade);
        let timer: TimerId = self
            .timers
            .schedule_at(due, TimerKind::Fishing(super::FishingTimer::LookForFish(id)));
        if let Some(look) = self.players.get_mut(&id).and_then(|p| p.fishing.look.as_mut()) {
            look.timer = Some(timer);
        }

        if now_ms >= end_time_ms {
            self.end_fishing(id, false);
            return;
        }
        let Some((fish_group, fish_guts)) = self
            .players
            .get(&id)
            .and_then(|p| p.fishing.fish.as_ref())
            .map(|f| (f.fish_group, f.fish_guts))
        else {
            return;
        };
        if fish_group == -1 {
            return;
        }
        let check = self.rng().below(100);
        if fish_guts > f64::from(check) {
            self.stop_looking_for_fish_task(id);
            self.begin_fish_combat(id, is_noob, is_upper_grade);
        }
    }

    /// Java `Player.endFishing(win)`.
    pub(crate) fn end_fishing(&mut self, id: ObjectId, win: bool) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.fishing.fishing = false;
        player.fishing.fish_x = 0;
        player.fishing.fish_y = 0;
        player.fishing.fish_z = 0;
        let combat = player.fishing.combat.take();
        player.fishing.lure_item_id = None;

        // `_fishCombat == null`: the fish never took the bait.
        if combat.is_none() {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheBaitHasBeenLostBecauseTheFishGotAway),
            );
        }
        if let Some(timer) = combat.and_then(|c| c.timer) {
            self.timers.cancel(timer);
        }

        // Ends fishing.
        self.broadcast_creature_packet(id, &ExFishingEnd { object_id: id.0, win });
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouReelYourLineInAndStopFishing),
        );
        self.set_immobilized(id, false);
        self.stop_looking_for_fish_task(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn night_lures_are_the_luminous_ones() {
        for id in [8485, 8505, 8509, 8513] {
            assert!(is_night_lure(id), "{id}");
        }
        for id in [8484, 8486, 8504, 8514, 6519, 7807] {
            assert!(!is_night_lure(id), "{id}");
        }
    }

    #[test]
    fn grade_follows_the_lure() {
        assert_eq!(random_fish_grade(7807), 0);
        assert_eq!(random_fish_grade(8486), 0);
        assert_eq!(random_fish_grade(8485), 2);
        assert_eq!(random_fish_grade(8512), 2);
        assert_eq!(random_fish_grade(6519), 1);
        assert_eq!(random_fish_grade(8484), 1);
    }

    #[test]
    fn group_draw_uses_the_thresholds_of_java() {
        // Green lure for beginners: 54 -> type 5, 77 -> type 4, else type 6.
        assert_eq!(random_fish_group_for(0, 7807, 54), 5);
        assert_eq!(random_fish_group_for(0, 7807, 55), 4);
        assert_eq!(random_fish_group_for(0, 7807, 77), 4);
        assert_eq!(random_fish_group_for(0, 7807, 78), 6);
        // Prize-winning lure for beginners: thirds.
        assert_eq!(random_fish_group_for(0, 8486, 33), 4);
        assert_eq!(random_fish_group_for(0, 8486, 66), 5);
        assert_eq!(random_fish_group_for(0, 8486, 67), 6);
        // Normal green lure: 54 -> 1, 74 -> 0, 94 -> 2, else 3.
        assert_eq!(random_fish_group_for(1, 6519, 54), 1);
        assert_eq!(random_fish_group_for(1, 6519, 74), 0);
        assert_eq!(random_fish_group_for(1, 6519, 94), 2);
        assert_eq!(random_fish_group_for(1, 6519, 95), 3);
        // The yellow lure's first threshold is 55.
        assert_eq!(random_fish_group_for(1, 6525, 55), 2);
        assert_eq!(random_fish_group_for(1, 6525, 56), 1);
        assert_eq!(random_fish_group_for(1, 6525, 75), 0);
        assert_eq!(random_fish_group_for(1, 6525, 95), 3);
        // Purple.
        assert_eq!(random_fish_group_for(1, 8510, 54), 0);
        assert_eq!(random_fish_group_for(1, 8510, 74), 1);
        // The box lures always give type 3.
        assert_eq!(random_fish_group_for(1, 7611, 99), 3);
        assert_eq!(random_fish_group_for(1, 7611, 0), 3);
        // Prize-winning normal lure.
        assert_eq!(random_fish_group_for(1, 8484, 33), 0);
        assert_eq!(random_fish_group_for(1, 8484, 66), 1);
        assert_eq!(random_fish_group_for(1, 8484, 67), 2);
        // Luminous lures.
        assert_eq!(random_fish_group_for(2, 8506, 0), 8);
        assert_eq!(random_fish_group_for(2, 8506, 77), 7);
        assert_eq!(random_fish_group_for(2, 8506, 78), 9);
        assert_eq!(random_fish_group_for(2, 8512, 54), 9);
        assert_eq!(random_fish_group_for(2, 8485, 66), 8);
        // An unlisted lure leaves the type at 1.
        assert_eq!(random_fish_group_for(0, 6519, 10), 1);
        assert_eq!(random_fish_group_for(2, 6519, 10), 1);
    }

    #[test]
    fn bite_delay_follows_the_lure_with_java_overlaps() {
        assert_eq!(check_delay_ms(6519, 5), 665);
        assert_eq!(check_delay_ms(8505, 5), 665);
        assert_eq!(check_delay_ms(6520, 5), 500);
        assert_eq!(check_delay_ms(7807, 4), 400);
        assert_eq!(check_delay_ms(6521, 5), 330);
        assert_eq!(check_delay_ms(6527, 5), 330);
        // 8507 is in 8505..=8513, which the second branch takes before the third.
        assert_eq!(check_delay_ms(8507, 5), 500);
        assert_eq!(check_delay_ms(8513, 5), 500);
        // A lure Java does not list: no delay, no task.
        assert_eq!(check_delay_ms(1, 5), 0);
    }
}
