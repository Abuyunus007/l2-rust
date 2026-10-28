//! The Turek Orcs: a wounded one flees to a point, then calls the others against its attacker.
// Java: dist/game/data/scripts/ai/others/TurekOrcs.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::movement::Intention;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const MOBS: [i32; 6] = [
    20494, // Turek War Hound
    20495, // Turek Orc Warlord
    20497, // Turek Orc Skirmisher
    20498, // Turek Orc Supplier
    20499, // Turek Orc Footman
    20500, // Turek Orc Sentinel
];

/// `npc.getAIValue("fleeX")`, `fleeY` and `fleeZ`: all three must be set on the spawn.
fn flee_point(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> Option<(i32, i32, i32)> {
    let w = ctx.world();
    Some((
        w.npc_ai_value(npc.object_id, "fleeX")?,
        w.npc_ai_value(npc.object_id, "fleeY")?,
        w.npc_ai_value(npc.object_id, "fleeZ")?,
    ))
}

/// Java `ai.others.TurekOrcs`.
pub struct TurekOrcs;

impl Script for TurekOrcs {
    fn name(&self) -> &'static str {
        "TurekOrcs"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&MOBS);
        s.add_event_received_id(&MOBS);
        s.add_move_finished_id(&MOBS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let npc = npc?;
        if event.eq_ignore_ascii_case("checkState")
            && !ctx.is_dead(npc.object_id)
            && ctx.world().intention_of(npc.object_id) != Intention::Attack
        {
            if ctx.npc_current_hp(npc) > f64::from(ctx.npc_max_hp(npc)) * 0.7
                && ctx
                    .world_mut()
                    .npc_variables_mut(npc.object_id)
                    .get_int("state", 0)
                    == 2
            {
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_int("state", 3);
                ctx.world_mut().return_home(npc.object_id);
            } else {
                ctx.world_mut().npc_variables_mut(npc.object_id).remove("state");
            }
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
        if !ctx
            .world_mut()
            .npc_variables_mut(npc.object_id)
            .has_variable("isHit")
        {
            ctx.world_mut()
                .npc_variables_mut(npc.object_id)
                .set_int("isHit", 1);
        } else if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * 0.5
            && ctx.npc_current_hp(npc) > f64::from(ctx.npc_max_hp(npc)) * 0.3
            && ctx
                .world()
                .creature_hp(attacker)
                .is_some_and(|(hp, max)| hp > max * 0.25)
            && let Some((fx, fy, fz)) = flee_point(ctx, npc)
            && ctx
                .world_mut()
                .npc_variables_mut(npc.object_id)
                .get_int("state", 0)
                == 0
            && ctx.get_random(100) < 10
        {
            // Say and flee
            let id = ctx.get_random_range(1000007, 1000027);
            if let Some(text) = NpcStringId::from_id(id) {
                ctx.broadcast_npc_say(npc, ChatType::General, text, &[]);
            }
            ctx.disable_core_ai(npc, true); // to avoid attacking behaviour, while flee
            ctx.set_running(npc);
            ctx.npc_move_to(npc, fx, fy, fz);
            let vars = ctx.world_mut().npc_variables_mut(npc.object_id);
            vars.set_int("state", 1);
            vars.set_int("attacker", attacker.0);
        }
    }

    fn on_event_received(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event_name: &str,
        _sender: ScriptNpc,
        receiver: ScriptNpc,
        reference: Option<ObjectId>,
    ) -> Option<String> {
        if event_name == "WARNING"
            && !ctx.is_dead(receiver.object_id)
            && ctx.world().intention_of(receiver.object_id) != Intention::Attack
        {
            if let Some(reference) = reference {
                // `reference.asPlayer()`
                if ctx.world().player(reference).is_some() && !ctx.is_dead(reference) {
                    ctx.world_mut()
                        .npc_variables_mut(receiver.object_id)
                        .set_int("state", 3);
                    ctx.add_attack_desire(receiver, reference, 99999);
                }
            }
        }

        None
    }

    fn on_move_finished(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let state = ctx
            .world_mut()
            .npc_variables_mut(npc.object_id)
            .get_int("state", 0);
        // NPC reaches flee point
        if state == 1 {
            let flee = flee_point(ctx, npc);
            let at = ctx.world().position_of(npc.object_id);
            // NOTE(java-quirk): `getAIValue` is -1 for a missing value, so an NPC without the
            // values never "arrives" and is sent to (-1, -1, -1) again.
            let (fx, fy, fz) = flee.unwrap_or((-1, -1, -1));
            if at.is_some_and(|(x, y, _)| x == fx && y == fy) {
                ctx.disable_core_ai(npc, false);
                ctx.start_quest_timer("checkState", 15000, Some(npc), None, false);
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_int("state", 2);
                let attacker = ctx
                    .world_mut()
                    .npc_variables_mut(npc.object_id)
                    .get_int("attacker", 0);
                // `World.getPlayer(id)`: null for an unknown id.
                let reference = ObjectId(attacker);
                let reference = ctx.world().player(reference).map(|_| reference);
                ctx.world_mut()
                    .broadcast_event(npc.object_id, "WARNING", 400, reference);
            } else {
                ctx.npc_move_to(npc, fx, fy, fz);
            }
        } else if state == 3 && ctx.world().npc_stays_in_spawn_loc(npc.object_id) {
            ctx.disable_core_ai(npc, false);
            ctx.world_mut().npc_variables_mut(npc.object_id).remove("state");
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="20494" type="Monster" level="30" name="Turek War Hound"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(TurekOrcs);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let orc = t.spawn(20494, 100);
        t.enter(1, 30, vec![]);
        t.take();
        (t, orc)
    }

    fn state(t: &mut ScriptTest, orc: ObjectId) -> i32 {
        t.world.npc_variables_mut(orc).get_int("state", 0)
    }

    #[test]
    fn the_first_hit_only_marks_the_orc() {
        let (mut t, orc) = setup();
        t.world.script_on_attackable_attack(orc, ObjectId(1), 10, None);
        t.world.script_drain();
        assert!(t.world.npc_variables(orc).unwrap().has_variable("isHit"));
        assert_eq!(state(&mut t, orc), 0);
    }

    #[test]
    fn a_warning_makes_a_quiet_orc_attack_the_player() {
        let (mut t, orc) = setup();
        let sender = t.spawn(20494, 120);
        t.world
            .script_send_event("WARNING", sender, orc, Some(ObjectId(1)));
        t.world.script_drain();
        assert_eq!(state(&mut t, orc), 3);
        t.world.delete_npc(sender);
    }

    #[test]
    fn check_state_clears_a_state_that_is_not_two() {
        let (mut t, orc) = setup();
        t.world.npc_variables_mut(orc).set_int("state", 1);
        t.world.run_as_script("TurekOrcs", |ctx| {
            let npc = ctx.script_npc(orc).unwrap();
            TurekOrcs.on_event(ctx, "CHECKSTATE", Some(npc), None);
        });
        assert!(!t.world.npc_variables(orc).unwrap().has_variable("state"));
    }

    #[test]
    fn a_fled_orc_that_is_back_home_gets_its_ai_back() {
        let (mut t, orc) = setup();
        t.world.npc_variables_mut(orc).set_int("state", 3);
        let (x, y, z) = t.world.position_of(orc).unwrap();
        t.world.set_npc_spawn_point(orc, x, y, z);
        t.world.npc_mut(orc).unwrap().spawn = Some(0);
        t.world.set_core_ai_disabled(orc, true);
        t.world.run_as_script("TurekOrcs", |ctx| {
            let npc = ctx.script_npc(orc).unwrap();
            TurekOrcs.on_move_finished(ctx, npc);
        });
        assert!(!t.world.npc_variables(orc).unwrap().has_variable("state"));
        assert!(!t.world.npc_core_ai_disabled(orc));
    }
}
