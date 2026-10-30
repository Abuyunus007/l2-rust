//! Siege guards: castle and fortress guards and mercenaries that look for targets themselves
//! every 3 seconds, ignore the attackers of other sieges and leave when their siege is over.
// Java: dist/game/data/scripts/ai/others/SiegeGuards.java
//
// The native `SiegeGuardAI` / `FortSiegeGuardAI` (`l2-world/src/ai/siege_guard.rs`) already make
// a guard attack the enemies of its siege within its attack range, heal friends and call its
// clan. This script is Mobius's addition on top: the 3 s aggro task over the *aggro range*, the
// hate removal for attackers of another siege field, the no-walk set-up and the clean-up at the
// end of a siege, so it is ported whole.

use std::collections::HashMap;

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const CASTLE_GUARDS: [i32; 114] = [
    35064, 35065, 35066, 35067, 35068, 35069, 35071, 35072, 35079, 35080, 35081, 35082, 35083, 35084, 35085,
    35106, 35107, 35108, 35109, 35110, 35111, 35113, 35114, 35121, 35122, 35123, 35124, 35125, 35126, 35127,
    35150, 35151, 35152, 35153, 35155, 35156, 35163, 35164, 35165, 35166, 35167, 35168, 35169, 35192, 35193,
    35194, 35195, 35197, 35198, 35205, 35206, 35207, 35208, 35209, 35210, 35211, 35234, 35239, 35240, 35248,
    35249, 35250, 35251, 35252, 35253, 35254, 35280, 35281, 35282, 35283, 35284, 35285, 35287, 35288, 35295,
    35296, 35297, 35298, 35299, 35300, 35301, 35324, 35325, 35326, 35327, 35328, 35330, 35339, 35340, 35341,
    35343, 35350, 35351, 35475, 35477, 35480, 35484, 35486, 35487, 35488, 35489, 35490, 35516, 35517, 35518,
    35519, 35520, 35522, 35531, 35532, 35533, 35535, 35542, 35543,
];

const FORT_GUARDS: [i32; 307] = [
    35670, 35671, 35672, 35673, 35674, 35678, 35679, 35681, 35682, 35684, 35685, 35702, 35703, 35704, 35705,
    35706, 35707, 35708, 35709, 35710, 35711, 35712, 35714, 35715, 35717, 35718, 35720, 35722, 35723, 35739,
    35740, 35741, 35742, 35743, 35747, 35748, 35750, 35751, 35753, 35754, 35758, 35767, 35771, 35772, 35773,
    35774, 35775, 35776, 35777, 35778, 35779, 35780, 35781, 35783, 35784, 35786, 35787, 35789, 35791, 35792,
    35808, 35809, 35810, 35811, 35812, 35816, 35817, 35819, 35820, 35822, 35823, 35839, 35840, 35841, 35842,
    35843, 35847, 35848, 35850, 35851, 35853, 35854, 35868, 35869, 35871, 35872, 35873, 35874, 35875, 35876,
    35877, 35878, 35879, 35880, 35881, 35883, 35884, 35886, 35887, 35889, 35891, 35892, 35908, 35909, 35910,
    35911, 35912, 35916, 35917, 35919, 35920, 35922, 35923, 35940, 35941, 35942, 35943, 35944, 35945, 35946,
    35947, 35948, 35949, 35950, 35952, 35953, 35955, 35956, 35958, 35960, 35961, 35978, 35979, 35980, 35981,
    35982, 35983, 35984, 35985, 35986, 35987, 35988, 35990, 35991, 35993, 35994, 35996, 35998, 35999, 36015,
    36016, 36017, 36018, 36019, 36023, 36024, 36026, 36027, 36029, 36030, 36047, 36048, 36049, 36050, 36051,
    36052, 36053, 36054, 36055, 36056, 36057, 36059, 36060, 36062, 36063, 36065, 36067, 36068, 36468, 36079,
    36085, 36086, 36087, 36088, 36089, 36090, 36091, 36092, 36093, 36094, 36095, 36097, 36098, 36100, 36101,
    36103, 36105, 36106, 36122, 36123, 36124, 36125, 36126, 36130, 36131, 36133, 36134, 36136, 36137, 36153,
    36154, 36155, 36156, 36157, 36161, 36162, 36164, 36165, 36167, 36168, 36185, 36186, 36187, 36188, 36189,
    36190, 36191, 36192, 36193, 36194, 36195, 36197, 36198, 36200, 36201, 36203, 36205, 36206, 36223, 36224,
    36225, 36226, 36227, 36228, 36229, 36230, 36231, 36232, 36233, 36235, 36236, 36238, 36239, 36241, 36243,
    36244, 36261, 36262, 36263, 36264, 36265, 36266, 36267, 36268, 36269, 36270, 36271, 36273, 36274, 36276,
    36277, 36279, 36281, 36282, 36298, 36299, 36300, 36301, 36302, 36306, 36307, 36309, 36310, 36312, 36313,
    36330, 36331, 36332, 36333, 36334, 36342, 36343, 36345, 36346, 36348, 36351, 36368, 36369, 36370, 36371,
    36372, 36380, 36381, 36383, 36384, 36386, 36389,
];

const MERCENARIES: [i32; 27] = [
    35015, 35016, 35017, 35018, 35019, 35025, 35026, 35027, 35028, 35029, 35035, 35036, 35037, 35038, 35039,
    35045, 35046, 35047, 35048, 35049, 35055, 35056, 35057, 35058, 35059, 35060, 35061,
];

const STATIONARY_MERCENARIES: [i32; 56] = [
    35010, 35011, 35012, 35013, 35014, 35020, 35021, 35022, 35023, 35024, 35030, 35031, 35032, 35033, 35034,
    35040, 35041, 35042, 35043, 35044, 35050, 35051, 35052, 35053, 35054, 35092, 35093, 35094, 35134, 35135,
    35136, 35176, 35177, 35178, 35218, 35219, 35220, 35261, 35262, 35263, 35264, 35265, 35308, 35309, 35310,
    35352, 35353, 35354, 35497, 35498, 35499, 35500, 35501, 35544, 35545, 35546,
];

/// Java `RESIDENCE_GUARD_MAP`: the guards of each residence (0 = unknown residences), and
/// `RESIDENCE_WORKING` has no counterpart (one world loop, no concurrent runs).
#[derive(Default)]
pub struct SiegeGuardsState {
    guards: HashMap<i32, Vec<ObjectId>>,
}

/// The quest timer of one residence's `AggroCheckTask`.
fn timer_name(residence_id: i32) -> String {
    format!("AGGRO_CHECK_{residence_id}")
}

/// Java `AggroCheckTask.run()`.
fn aggro_check(ctx: &mut ScriptCtx<'_>, residence_id: i32) {
    let guards = ctx
        .state::<SiegeGuardsState>()
        .guards
        .get(&residence_id)
        .cloned()
        .unwrap_or_default();
    for guard in guards {
        let Some(npc) = ctx.script_npc(guard) else {
            continue;
        };
        // Should never happen.
        if !ctx.is_attackable(npc) {
            continue;
        }

        // Ignore dead guards.
        if ctx.is_dead(guard) {
            continue;
        }

        // Skip if guard is currently attacking.
        if ctx.is_in_combat(guard) {
            continue;
        }

        let aggro_range = ctx.world().script_npc_aggro_range(guard);

        // Skip if guard has an active target (not dead/invis/invul and is within range).
        if let Some(target) = ctx.world().creature_target(guard)
            && (ctx.world().player(target).is_some() || ctx.world().npc(target).is_some())
            && !ctx.is_dead(target)
            && !ctx.world().script_is_invisible(target)
            && !ctx.world().is_invul(target)
            && distance_2d(ctx, guard, target) < f64::from(aggro_range)
        {
            continue;
        }

        // Find first valid target within aggro range and start attacking it.
        let script_value = ctx.get_script_value(npc);
        let nearby = ctx.world().visible_objects_in_range(guard, aggro_range);
        for object in nearby {
            let player = if ctx.world().player(object).is_some() {
                Some(object)
            } else {
                ctx.world().summon_owner(object)
            };
            let Some(player) = player else {
                continue;
            };
            if ctx.is_dead(object)
                || ctx.world().script_is_invisible(object)
                || ctx.world().is_invul(object)
                || !ctx.world().can_see_target(guard, object)
            {
                continue;
            }
            if ctx.world().siege_state_of(player) == 2
                && ctx
                    .world()
                    .is_registered_on_this_siege_field(player, script_value)
            {
                continue;
            }
            ctx.world_mut().add_damage_hate(guard, object, 0, 999);
            break;
        }
    }

    ctx.start_quest_timer(&timer_name(residence_id), 3000, None, None, false);
}

/// Java `calculateDistance2D(target)`.
fn distance_2d(ctx: &ScriptCtx<'_>, a: ObjectId, b: ObjectId) -> f64 {
    let (Some(a), Some(b)) = (ctx.world().position_of(a), ctx.world().position_of(b)) else {
        return f64::MAX;
    };
    let (dx, dy) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1));
    (dx * dx + dy * dy).sqrt()
}

/// Java `onCastleSiegeFinish` / `onFortSiegeFinish`: the guards of the residence go.
fn siege_finished(ctx: &mut ScriptCtx<'_>, residence_id: i32) {
    // NOTE(java-quirk): Java looks the list up in a 122 slot array; a residence without a list
    // (no task was made for it) does nothing.
    let Some(guards) = ctx
        .state::<SiegeGuardsState>()
        .guards
        .get_mut(&residence_id)
        .map(std::mem::take)
    else {
        return;
    };
    for guard in guards {
        ctx.world_mut().script_stop_npc_respawn(guard);
        if let Some(npc) = ctx.script_npc(guard) {
            ctx.delete_npc(npc);
        }
    }
}

/// Java `ai.others.SiegeGuards`.
pub struct SiegeGuards;

impl SiegeGuards {
    fn all_ids() -> Vec<i32> {
        [
            CASTLE_GUARDS.as_slice(),
            FORT_GUARDS.as_slice(),
            MERCENARIES.as_slice(),
            STATIONARY_MERCENARIES.as_slice(),
        ]
        .concat()
    }
}

impl Script for SiegeGuards {
    fn name(&self) -> &'static str {
        "SiegeGuards"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let ids = Self::all_ids();
        s.add_attack_id(&ids);
        s.add_spawn_id(&ids);
        s.add_kill_id(&ids);
        s.set_on_castle_siege_finish();
        s.set_on_fort_siege_finish();
    }

    // The Java constructor: one aggro task for unknown residences, one for each castle and fort.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        // Start task for unknown residences.
        ctx.state::<SiegeGuardsState>().guards.insert(0, Vec::new());
        ctx.start_quest_timer(&timer_name(0), 1, None, None, false);

        // Start tasks for castles.
        let castles: Vec<i32> = ctx.world().castles().iter().map(|c| c.id.0).collect();
        for residence_id in castles {
            ctx.state::<SiegeGuardsState>()
                .guards
                .insert(residence_id, Vec::new());
            ctx.start_quest_timer(
                &timer_name(residence_id),
                i64::from(residence_id) * 100 + 1,
                None,
                None,
                false,
            );
        }

        // Start tasks for fortresses.
        let forts: Vec<i32> = ctx.world().forts().iter().map(|f| f.id.0).collect();
        for residence_id in forts {
            ctx.state::<SiegeGuardsState>()
                .guards
                .insert(residence_id, Vec::new());
            ctx.start_quest_timer(
                &timer_name(residence_id),
                900 + i64::from(residence_id - 100) * 100 + 1,
                None,
                None,
                false,
            );
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(id) = event.strip_prefix("AGGRO_CHECK_")
            && let Ok(residence_id) = id.parse::<i32>()
        {
            aggro_check(ctx, residence_id);
        }
        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let script_value = ctx.get_script_value(npc);
        if ctx.world().siege_state_of(attacker) == 2
            && !ctx
                .world()
                .is_registered_on_this_siege_field(attacker, script_value)
        {
            ctx.stop_hating(npc, attacker);
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        let residence_id = ctx.get_script_value(npc);
        if residence_id == 0 {
            if let Some(list) = ctx.state::<SiegeGuardsState>().guards.get_mut(&0) {
                list.retain(|g| *g != npc.object_id);
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.set_random_walking(npc, false);
        if !ctx.world().script_npc_attacks_with_sword_or_pole(npc.object_id) {
            ctx.set_immobilized(npc, true);
        }

        let fort = ctx.world().script_npc_fort_facts(npc.object_id);
        let castle = ctx.world().script_npc_castle_facts(npc.object_id);
        let residence_id = match (fort, castle) {
            (Some(fort), _) => fort.id,
            (None, Some(castle)) => castle.id,
            (None, None) => 0,
        };
        ctx.set_script_value(npc, residence_id);
        let state = ctx.state::<SiegeGuardsState>();
        if residence_id > 0 && state.guards.contains_key(&residence_id) {
            let list = state.guards.entry(residence_id).or_default();
            if !list.contains(&npc.object_id) {
                list.push(npc.object_id);
            }
        } else {
            // Residence id not found.
            state.guards.entry(0).or_default().push(npc.object_id);
        }
    }

    fn on_castle_siege_finish(&self, ctx: &mut ScriptCtx<'_>, castle_id: i32) {
        siege_finished(ctx, castle_id);
    }

    fn on_fort_siege_finish(&self, ctx: &mut ScriptCtx<'_>, fort_id: i32) {
        siege_finished(ctx, fort_id);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    // 35064: a Gludio castle guard, 35010: a stationary mercenary.
    const NPCS: &str = r#"
        <npc id="35064" type="Monster" level="70" name="Guard"/>
        <npc id="35010" type="Monster" level="70" name="Mercenary"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(SiegeGuards);
        ScriptTest::new(registry, NPCS, "", &[])
    }

    fn guards(t: &mut ScriptTest, residence: i32) -> Vec<ObjectId> {
        let mut list = Vec::new();
        t.world.run_as_script("SiegeGuards", |ctx| {
            list = ctx
                .state::<SiegeGuardsState>()
                .guards
                .get(&residence)
                .cloned()
                .unwrap_or_default();
        });
        list
    }

    #[test]
    fn the_id_lists_are_java_s() {
        assert_eq!(CASTLE_GUARDS.len(), 114);
        assert_eq!(FORT_GUARDS.len(), 307);
        assert_eq!(MERCENARIES.len(), 27);
        assert_eq!(STATIONARY_MERCENARIES.len(), 56);
        assert!(SiegeGuards::all_ids().contains(&35010));
    }

    #[test]
    fn a_guard_without_a_residence_joins_the_unknown_list_and_stops_walking() {
        let mut t = setup();
        let guard = t.spawn(35064, 20);
        t.ticks(2);
        assert_eq!(guards(&mut t, 0), [guard]);
        assert_eq!(t.world.npc_script_value(guard), 0);
        assert!(t.world.npc_random_walking_forbidden(guard));
    }

    #[test]
    fn a_killed_guard_of_the_unknown_list_is_taken_out() {
        let mut t = setup();
        let guard = t.spawn(35064, 20);
        t.ticks(2);
        assert_eq!(guards(&mut t, 0), [guard]);
        t.enter(1, 70, vec![]);
        t.kill(1, guard);
        assert!(guards(&mut t, 0).is_empty());
    }

    #[test]
    fn the_end_of_a_siege_removes_the_guards_of_the_residence() {
        let mut t = setup();
        let guard = t.spawn(35064, 20);
        let other = t.spawn(35010, 30);
        t.ticks(2);
        assert_eq!(guards(&mut t, 0).len(), 2);
        t.world.run_as_script("SiegeGuards", |ctx| {
            SiegeGuards.on_castle_siege_finish(ctx, 0);
        });
        assert!(!t.in_world(guard));
        assert!(!t.in_world(other));
        assert!(guards(&mut t, 0).is_empty());
    }

    #[test]
    fn the_aggro_task_keeps_itself_on_the_wheel() {
        let mut t = setup();
        t.ticks(5);
        t.world.run_as_script("SiegeGuards", |ctx| {
            assert!(ctx.has_quest_timer("AGGRO_CHECK_0", None, None));
        });
        t.wait_ms(3_200);
        t.world.run_as_script("SiegeGuards", |ctx| {
            assert!(ctx.has_quest_timer("AGGRO_CHECK_0", None, None));
        });
    }
}
