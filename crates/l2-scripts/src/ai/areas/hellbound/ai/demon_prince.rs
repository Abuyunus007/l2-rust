//! The Demon Prince: Ultimate Defense and the Fiends he calls at half and a tenth of his HP.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/DemonPrince.java

use std::collections::HashMap;

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const DEMON_PRINCE: i32 = 25540;
const FIEND: i32 = 25541;

// Skills
const UD: (i32, i32) = (5044, 2);
const AOE: [(i32, i32); 3] = [(5376, 4), (5376, 5), (5376, 6)];

/// Java `ATTACK_STATE`: the Demon Princes that called their first Fiends, and whether they called
/// the second wave (a `static` map of the Java class).
#[derive(Default)]
struct DemonPrinceState {
    attack_state: HashMap<ObjectId, bool>,
}

/// Java `ai.areas.Hellbound.AI.DemonPrince`.
pub struct DemonPrince;

impl DemonPrince {
    /// Java `spawnMinions(master)`.
    fn spawn_minions(ctx: &mut ScriptCtx<'_>, master: ScriptNpc) {
        if !ctx.is_dead(master.object_id) {
            let instance_id = ctx.world().instance_id_of(master.object_id);
            let Some((x, y, z)) = ctx.world().position_of(master.object_id) else {
                return;
            };
            ctx.add_spawn(FIEND, x + 200, y, z, 0, false, 0, instance_id);
            ctx.add_spawn(FIEND, x - 200, y, z, 0, false, 0, instance_id);
            ctx.add_spawn(FIEND, x - 100, y - 140, z, 0, false, 0, instance_id);
            ctx.add_spawn(FIEND, x - 100, y + 140, z, 0, false, 0, instance_id);
            ctx.add_spawn(FIEND, x + 100, y - 140, z, 0, false, 0, instance_id);
            ctx.add_spawn(FIEND, x + 100, y + 140, z, 0, false, 0, instance_id);
        }
    }
}

impl Script for DemonPrince {
    fn name(&self) -> &'static str {
        "DemonPrince"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[DEMON_PRINCE]);
        s.add_kill_id(&[DEMON_PRINCE]);
        s.add_spawn_id(&[FIEND]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("cast")
            && let Some(npc) = npc
            && npc.id == FIEND
            && !ctx.is_dead(npc.object_id)
        {
            let (id, level) = ctx.get_random_entry(&AOE)?;
            ctx.world_mut().do_cast(npc.object_id, SkillKey::new(id, level));
        }

        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        if !ctx.is_dead(npc.object_id) {
            let hp = ctx.npc_current_hp(npc);
            let max_hp = f64::from(ctx.npc_max_hp(npc));
            let state = ctx.state::<DemonPrinceState>().attack_state.get(&npc.object_id).copied();
            if state.is_none() && hp < max_hp * 0.5 {
                ctx.world_mut().do_cast(npc.object_id, SkillKey::new(UD.0, UD.1));
                Self::spawn_minions(ctx, npc);
                ctx.state::<DemonPrinceState>().attack_state.insert(npc.object_id, false);
            } else if hp < max_hp * 0.1 && state == Some(false) {
                ctx.world_mut().do_cast(npc.object_id, SkillKey::new(UD.0, UD.1));
                Self::spawn_minions(ctx, npc);
                ctx.state::<DemonPrinceState>().attack_state.insert(npc.object_id, true);
            }

            if ctx.get_random(1000) < 10 {
                Self::spawn_minions(ctx, npc);
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        ctx.state::<DemonPrinceState>().attack_state.remove(&npc.object_id);
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == FIEND {
            ctx.start_quest_timer("cast", 15000, Some(npc), None, false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25540" type="Monster" level="80" name="Demon Prince"/>
        <npc id="25541" type="Monster" level="80" name="Fiend"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(DemonPrince);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t
    }

    #[test]
    fn a_fiend_casts_fifteen_seconds_after_it_spawns() {
        let mut t = setup();
        let fiend = crate::ai::areas::hellbound::testing::spawn(&mut t, 25541, 20);
        t.world.run_as_script("DemonPrince", |ctx| {
            assert!(ctx.has_quest_timer("cast", ctx.script_npc(fiend), None));
        });
    }

    #[test]
    fn the_prince_calls_six_fiends_at_half_hp_once() {
        let mut t = setup();
        let prince = crate::ai::areas::hellbound::testing::spawn(&mut t, 25540, 20);
        // The test world has no stat tables (max HP 0): HP 0 is not below 50% of 0, so only the
        // 1% roll can call Fiends; call the hook with the wave by hand.
        let before = t.npcs_of(25541).len();
        t.world.run_as_script("DemonPrince", |ctx| {
            let npc = ctx.script_npc(prince).unwrap();
            DemonPrince::spawn_minions(ctx, npc);
        });
        assert_eq!(t.npcs_of(25541).len(), before + 6);
        // Killing the prince forgets its state.
        t.world.run_as_script("DemonPrince", |ctx| {
            let npc = ctx.script_npc(prince).unwrap();
            ctx.state::<DemonPrinceState>().attack_state.insert(prince, false);
            DemonPrince.on_kill(ctx, npc, ObjectId(1), false);
            assert!(ctx.state::<DemonPrinceState>().attack_state.is_empty());
        });
    }
}
