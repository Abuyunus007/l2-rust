//! The Seed of Annihilation: the weekly buff of its three regions, the elite monsters with their
//! minions, the Annihilation Furnaces and the guarded teleport zones.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/SeedOfAnnihilation/SeedOfAnnihilation.java
//
// NOT PORTED: `ZoneManager.getZoneById(buffZone, EffectZone.class).addSkill(..)` / `clearSkills()`.
// The world's effect zones apply no skills yet (`zones.rs`), so the Strength / Agility / Wisdom
// buff zones (60006 to 60008) have no effect; the furnaces' display effect, the weekly draw and
// the global variables are ported.

use l2_util::ObjectId;
use l2_util::time::local_offset_millis;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// (zone id, x, y, z) of the zones that throw a creature out.
const TELEPORT_ZONES: [(i32, (i32, i32, i32)); 4] = [
    (60002, (-213175, 182648, -10992)),
    (60003, (-181217, 186711, -10528)),
    (60004, (-180211, 182984, -15152)),
    (60005, (-179275, 186802, -10720)),
];

const ANNIHILATION_FURNACE: i32 = 18928;

// Strength, Agility, Wisdom
#[allow(dead_code)] // for the effect zones, see the top
const ZONE_BUFFS: [i32; 4] = [0, 6443, 6444, 6442];

const ZONE_BUFFS_LIST: [[i32; 3]; 6] = [[1, 2, 3], [1, 3, 2], [2, 1, 3], [2, 3, 1], [3, 2, 1], [3, 1, 2]];

/// Java's `SeedRegion`: 0 Bistakon, 1 Reptilikon, 2 Cokrakon.
struct SeedRegion {
    elite_mob_ids: &'static [i32],
    minion_lists: &'static [&'static [i32]],
    #[allow(dead_code)] // for the effect zones, see the top
    buff_zone: i32,
    af_spawns: [(i32, i32, i32, i32); 2],
}

const REGIONS: [SeedRegion; 3] = [
    // Bistakon data
    SeedRegion {
        elite_mob_ids: &[22750, 22751, 22752, 22753],
        minion_lists: &[
            &[22746, 22746, 22746],
            &[22747, 22747, 22747],
            &[22748, 22748, 22748],
            &[22749, 22749, 22749],
        ],
        buff_zone: 60006,
        af_spawns: [(-180450, 185507, -10544, 11632), (-180005, 185489, -10544, 11632)],
    },
    // Reptilikon data
    SeedRegion {
        elite_mob_ids: &[22757, 22758, 22759],
        minion_lists: &[&[22754, 22755, 22756]],
        buff_zone: 60007,
        af_spawns: [(-179600, 186998, -10704, 11632), (-179295, 186444, -10704, 11632)],
    },
    // Cokrakon data
    SeedRegion {
        elite_mob_ids: &[22763, 22764, 22765],
        minion_lists: &[
            &[22760, 22760, 22761],
            &[22760, 22760, 22762],
            &[22761, 22761, 22760],
            &[22761, 22761, 22762],
            &[22762, 22762, 22760],
            &[22762, 22762, 22761],
        ],
        buff_zone: 60008,
        af_spawns: [(-180971, 186361, -10528, 11632), (-180758, 186739, -10528, 11632)],
    },
];

const DAY_MS: i64 = 86_400_000;

/// Java's fields `_regionsData[i].activeBuff` / `af_npcs` and `_seedsNextStatusChange`.
#[derive(Default)]
struct SeedState {
    active_buff: [i32; 3],
    af_npcs: [[Option<ObjectId>; 2]; 3],
    seeds_next_status_change: i64,
}

/// Java `getNextSeedsStatusChangeTime()` on the local wall clock (D-48): the Monday of this week
/// at 13:00:00 (plus the milliseconds of now), a week later if that has passed. Returned in epoch
/// milliseconds.
// NOTE(java-quirk): `Calendar.set(DAY_OF_WEEK, MONDAY)` picks the Monday of the week as the JVM's
// locale numbers it. The first day of the week is taken as Sunday here (the `en_US` default), so
// a Sunday gets the Monday after it; the milliseconds of now stay in the result.
fn next_seeds_status_change_time(now_ms: i64) -> i64 {
    let offset = local_offset_millis(now_ms);
    let local = now_ms + offset;
    // `Calendar.DAY_OF_WEEK`: Sunday 1 .. Saturday 7; the epoch day 0 was a Thursday.
    let dow = (local.div_euclid(DAY_MS) + 4).rem_euclid(7) + 1;
    let day_start = local.div_euclid(DAY_MS) * DAY_MS;
    let mut reenter = day_start + (2 - dow) * DAY_MS + 13 * 3_600_000 + local.rem_euclid(1000);
    if reenter - offset <= now_ms {
        reenter += 7 * DAY_MS;
    }
    reenter - offset
}

/// Java `ai.areas.Gracia.AI.SeedOfAnnihilation.SeedOfAnnihilation`.
pub struct SeedOfAnnihilation;

impl SeedOfAnnihilation {
    /// Java `loadSeedRegionData()` (the part that reads and draws the global variables).
    fn load_seed_region_data(ctx: &mut ScriptCtx<'_>) {
        let var = ctx.global_variables().get_long("SeedNextStatusChange", 0);
        let buffs_now;
        let next;
        if var < ctx.now_ms() {
            buffs_now = ctx.get_random(ZONE_BUFFS_LIST.len() as i32);
            ctx.set_global_variable("SeedBuffsList", &buffs_now.to_string());
            next = next_seeds_status_change_time(ctx.now_ms());
            ctx.set_global_variable("SeedNextStatusChange", &next.to_string());
        } else {
            next = var;
            buffs_now = ctx.global_variables().get_int("SeedBuffsList", 0);
        }

        let state = ctx.state::<SeedState>();
        state.seeds_next_status_change = next;
        let row = ZONE_BUFFS_LIST[usize::try_from(buffs_now).unwrap_or(0) % ZONE_BUFFS_LIST.len()];
        for (i, buff) in row.iter().enumerate() {
            state.active_buff[i] = *buff;
        }
    }

    /// Java `startEffectZonesControl()`.
    fn start_effect_zones_control(ctx: &mut ScriptCtx<'_>) {
        for (i, region) in REGIONS.iter().enumerate() {
            for (j, spawn) in region.af_spawns.iter().enumerate() {
                let furnace = ctx.add_spawn(
                    ANNIHILATION_FURNACE,
                    spawn.0,
                    spawn.1,
                    spawn.2,
                    spawn.3,
                    false,
                    0,
                    0,
                );
                let active = ctx.state::<SeedState>().active_buff[i];
                ctx.state::<SeedState>().af_npcs[i][j] = furnace.map(|f| f.object_id);
                if let Some(furnace) = furnace {
                    crate::ai::areas::gracia::util::set_display_effect(ctx, furnace.object_id, active);
                }
            }
            // The buff zone `addSkill(ZONE_BUFFS[activeBuff], 1)` is not ported (see the top).
        }

        let next = ctx.state::<SeedState>().seeds_next_status_change;
        ctx.start_quest_timer("ChangeSeedsStatus", next - ctx.now_ms(), None, None, false);
    }
}

impl Script for SeedOfAnnihilation {
    fn name(&self) -> &'static str {
        "SeedOfAnnihilation"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI/SeedOfAnnihilation"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let zones: Vec<i32> = TELEPORT_ZONES.iter().map(|(zone, _)| *zone).collect();
        s.add_enter_zone_id(&zones);
        for region in &REGIONS {
            s.add_spawn_id(region.elite_mob_ids);
        }

        s.add_start_npc(&[32739]);
        s.add_talk_id(&[32739]);
    }

    // The Java constructor: `loadSeedRegionData()` first, then the listeners, then
    // `startEffectZonesControl()`.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        Self::load_seed_region_data(ctx);
        Self::start_effect_zones_control(ctx);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        for region in &REGIONS {
            if region.elite_mob_ids.contains(&npc.id) {
                // `spawnGroupOfMinion(npc.asMonster(), getRandomEntry(element.minion_lists))`
                let index = ctx.get_random(region.minion_lists.len() as i32);
                if let Some(list) = region.minion_lists.get(usize::try_from(index).unwrap_or(0)) {
                    for mob_id in *list {
                        ctx.spawn_minion(npc, *mob_id);
                    }
                }
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("ChangeSeedsStatus") {
            let buffs_now = ctx.get_random(ZONE_BUFFS_LIST.len() as i32);
            ctx.set_global_variable("SeedBuffsList", &buffs_now.to_string());
            let next = next_seeds_status_change_time(ctx.now_ms());
            ctx.state::<SeedState>().seeds_next_status_change = next;
            ctx.set_global_variable("SeedNextStatusChange", &next.to_string());
            let row = ZONE_BUFFS_LIST[usize::try_from(buffs_now).unwrap_or(0) % ZONE_BUFFS_LIST.len()];
            for (i, active) in row.into_iter().enumerate() {
                ctx.state::<SeedState>().active_buff[i] = active;

                let furnaces = ctx.state::<SeedState>().af_npcs[i];
                for furnace in furnaces.into_iter().flatten() {
                    crate::ai::areas::gracia::util::set_display_effect(ctx, furnace, active);
                }
                // `zone.clearSkills()` and `zone.addSkill(..)` are not ported (see the top).
            }

            ctx.start_quest_timer("ChangeSeedsStatus", next - ctx.now_ms(), None, None, false);
        } else if event.eq_ignore_ascii_case("transform") {
            let (npc, player) = (npc?, player?);
            if ctx.world().is_affected_by_skill(player, 6408) {
                ctx.world_mut().show_chat_window(npc.object_id, player, 2);
            } else {
                ctx.cast_skill(npc, player, SkillKey::new(6408, 1));
                ctx.cast_skill(npc, player, SkillKey::new(6649, 1));
                ctx.world_mut().show_chat_window(npc.object_id, player, 1);
            }
        }

        None
    }

    fn on_enter_zone(&self, ctx: &mut ScriptCtx<'_>, creature: ObjectId, zone_id: i32) {
        if let Some((_, (x, y, z))) = TELEPORT_ZONES.iter().find(|(zone, _)| *zone == zone_id) {
            ctx.teleport_player(creature, *x, *y, *z, 0, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="18928" type="Folk" level="70" name="Annihilation Furnace"/>
        <npc id="22750" type="Monster" level="80" name="Elite Bgurent"/>
        <npc id="22746" type="Monster" level="80" name="Bistakon minion"/>
        <npc id="22747" type="Monster" level="80" name="Bistakon minion"/>
        <npc id="22748" type="Monster" level="80" name="Bistakon minion"/>
        <npc id="22749" type="Monster" level="80" name="Bistakon minion"/>"#;

    #[test]
    fn the_next_change_is_the_monday_at_13_in_the_future() {
        // 2024-01-03 (a Wednesday), 10:00 UTC.
        let now = (19725 * DAY_MS) + 10 * 3_600_000;
        let next = next_seeds_status_change_time(now);
        assert!(next > now);
        assert!(next - now <= 7 * DAY_MS + 1000);
        // It is a Monday 13:00 on the local wall clock (plus the milliseconds of now).
        let local = next + local_offset_millis(next);
        let dow = (local.div_euclid(DAY_MS) + 4).rem_euclid(7) + 1;
        assert_eq!(dow, 2);
        assert_eq!(local.rem_euclid(DAY_MS) / 1000, 13 * 3600);
    }

    #[test]
    fn the_load_draws_the_buffs_once_and_spawns_the_furnaces() {
        let mut registry = ScriptRegistry::new();
        registry.add(SeedOfAnnihilation);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t.take();
        // `install_scripts` ran `on_load`: six furnaces, the draw in the global variables.
        let furnaces = t
            .world
            .npc_ids()
            .into_iter()
            .filter(|id| t.world.npc(*id).is_some_and(|n| n.npc_id == ANNIHILATION_FURNACE))
            .count();
        assert_eq!(furnaces, 6);
        let next = t.world.global_variables().get_long("SeedNextStatusChange", 0);
        assert!(next > 0);
        let list = t.world.global_variables().get_int("SeedBuffsList", -1);
        assert!((0..6).contains(&list), "{list}");
    }

    #[test]
    fn the_status_change_redraws_and_reschedules() {
        let mut registry = ScriptRegistry::new();
        registry.add(SeedOfAnnihilation);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t.take();
        t.world.run_as_script("SeedOfAnnihilation", |ctx| {
            ctx.notify_event("ChangeSeedsStatus", None, None);
            assert!(ctx.has_quest_timer("ChangeSeedsStatus", None, None));
        });
        let list = t.world.global_variables().get_int("SeedBuffsList", -1);
        assert!((0..6).contains(&list), "{list}");
    }

    #[test]
    fn an_elite_monster_comes_with_a_group_of_minions() {
        let mut registry = ScriptRegistry::new();
        registry.add(SeedOfAnnihilation);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let before = t.world.npc_count();
        let elite = t.spawn(22750, 20);
        t.ticks(2);
        let _ = names_to(&t.take(), 1);
        let minions = t.world.minions_of(elite).len();
        assert_eq!(minions, 3);
        assert_eq!(t.world.npc_count(), before + 4);
    }
}
