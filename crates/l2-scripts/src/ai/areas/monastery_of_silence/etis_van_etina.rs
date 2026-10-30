//! Etis van Etina (Head of the Embryo) AI: below half HP he calls his two phantoms.
// Java: dist/game/data/scripts/ai/areas/MonasteryOfSilence/EtisVanEtina.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ETIS_VAN_ETINA: i32 = 18949;
const PHANTOM_REAL: i32 = 18950;
const PHANTOM_FAKE: i32 = 18951;
// Misc
const THINK_TIMER: &str = "THINK";
const EVT_PHANTOM_REAL_DEAD: &str = "ETIS_PHANTOM_REAL_DEAD";
const EVT_BOSS_DEAD: &str = "ETIS_BOSS_DEAD";
const PHASE_NORMAL: i32 = 0;
const PHASE_TRANSFORM: i32 = 1;
const THINK_INTERVAL: i64 = 5000;
const PHANTOM_BROADCAST_RADIUS: i32 = 5000;

/// Java `ai.areas.MonasteryOfSilence.EtisVanEtina`.
pub struct EtisVanEtina;

impl Script for EtisVanEtina {
    fn name(&self) -> &'static str {
        "EtisVanEtina"
    }

    fn path(&self) -> &'static str {
        "ai/areas/MonasteryOfSilence"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[ETIS_VAN_ETINA]);
        s.add_kill_id(&[ETIS_VAN_ETINA]);
        s.add_event_received_id(&[ETIS_VAN_ETINA]);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        let vars = ctx.world_mut().npc_variables_mut(npc.object_id);
        vars.set_int("phase", PHASE_NORMAL);
        vars.set_bool("phantomsAlive", false);
        ctx.start_quest_timer(THINK_TIMER, THINK_INTERVAL, Some(npc), None, false);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event == THINK_TIMER
            && let Some(npc) = npc
            && !ctx.is_dead(npc.object_id)
        {
            let phase = ctx
                .world_mut()
                .npc_variables_mut(npc.object_id)
                .get_int("phase", PHASE_NORMAL);
            // NOTE: `getMaxRecoverableHp()` is the maximum HP less the penalty of effects that
            // lower it; the world has no such penalty for an NPC, so it is `getMaxHp()` here.
            if (phase == PHASE_NORMAL) && (ctx.npc_current_hp(npc) < (f64::from(ctx.npc_max_hp(npc)) * 0.5)) {
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_int("phase", PHASE_TRANSFORM);
            }

            let vars = ctx.world_mut().npc_variables_mut(npc.object_id);
            if (vars.get_int("phase", PHASE_NORMAL) == PHASE_TRANSFORM)
                && !vars.get_bool("phantomsAlive", false)
            {
                let dr = ctx.add_spawn_at(PHANTOM_REAL, npc.object_id, true, 0);
                let df = ctx.add_spawn_at(PHANTOM_FAKE, npc.object_id, true, 0);
                let target = ctx.target_of(npc.object_id);
                if let Some(target) = target
                    && ctx.world().player(target).is_some()
                {
                    for phantom in [dr, df].into_iter().flatten() {
                        ctx.world_mut()
                            .npc_variables_mut(phantom.object_id)
                            .set_int("etisTargetId", target.0);
                    }
                }
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_bool("phantomsAlive", true);
            }

            ctx.start_quest_timer(THINK_TIMER, THINK_INTERVAL, Some(npc), None, false);
        }

        None
    }

    fn on_event_received(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event_name: &str,
        _sender: ScriptNpc,
        receiver: ScriptNpc,
        _reference: Option<ObjectId>,
    ) -> Option<String> {
        if event_name == EVT_PHANTOM_REAL_DEAD
            && (receiver.id == ETIS_VAN_ETINA)
            && !ctx.is_dead(receiver.object_id)
        {
            ctx.world_mut()
                .npc_variables_mut(receiver.object_id)
                .set_bool("phantomsAlive", false);
        }

        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.cancel_quest_timer(THINK_TIMER, Some(npc), None);
        ctx.world_mut()
            .broadcast_event(npc.object_id, EVT_BOSS_DEAD, PHANTOM_BROADCAST_RADIUS, None);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="18949" type="Monster" level="80" name="Etis van Etina"/>
        <npc id="18950" type="Monster" level="80" name="Etis van Etina Phantom"/>
        <npc id="18951" type="Monster" level="80" name="Etis van Etina Phantom"/>"#;

    #[test]
    fn the_think_timer_starts_at_spawn_and_stops_at_death() {
        let mut registry = ScriptRegistry::new();
        registry.add(EtisVanEtina);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let etis = t.spawn(ETIS_VAN_ETINA, 20);
        t.ticks(2);
        t.world.run_as_script("EtisVanEtina", |ctx| {
            let npc = ctx.script_npc(etis).unwrap();
            assert!(ctx.has_quest_timer(THINK_TIMER, Some(npc), None));
        });
        assert_eq!(
            t.world.npc_variables(etis).unwrap().get_int("phase", -1),
            PHASE_NORMAL
        );
        t.kill(1, etis);
        t.world.run_as_script("EtisVanEtina", |ctx| {
            let npc = ctx.script_npc(etis).unwrap();
            assert!(!ctx.has_quest_timer(THINK_TIMER, Some(npc), None));
        });
    }
}
