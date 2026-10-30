//! Porta and Perum pull a far-away attacker next to them with Summon Party Member.
// Java: dist/game/data/scripts/ai/areas/CrumaTower/SummonPc.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey, TeleportTarget};

// NPCs
const PORTA: i32 = 20213;
const PERUM: i32 = 20221;
// Skill
const SUMMON_PC: SkillKey = SkillKey { id: 4161, level: 1 };

/// Java `ai.areas.CrumaTower.SummonPc`.
pub struct SummonPc;

/// `SUMMON_PC.getSkill().getMpConsume() < npc.getCurrentMp() && getHpConsume() < npc.getCurrentHp()
/// && !npc.isSkillDisabled(skill)`.
fn can_cast(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) -> bool {
    let (mp, hp) = ctx
        .world()
        .script_skill_mp_hp_consume(SUMMON_PC)
        .unwrap_or((0, 0));
    f64::from(mp) < ctx.world().script_npc_current_mp(npc.object_id)
        && f64::from(hp) < ctx.npc_current_hp(npc)
        && !ctx.world_mut().is_skill_disabled_of(npc.object_id, SUMMON_PC)
}

fn attacked(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> bool {
    ctx.world()
        .npc_variables(npc.object_id)
        .is_some_and(|v| v.get_bool("attacked", false))
}

impl Script for SummonPc {
    fn name(&self) -> &'static str {
        "SummonPc"
    }

    fn path(&self) -> &'static str {
        "ai/areas/CrumaTower"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[PORTA, PERUM]);
        s.add_spell_finished_id(&[PORTA, PERUM]);
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
        let chance = ctx.get_random(100);
        let attacked = attacked(ctx, npc);
        let distance = ctx.distance_3d(npc.object_id, attacker).unwrap_or(0.0);
        if distance > 300.0 && !attacked {
            if chance < 50 {
                if can_cast(ctx, npc) {
                    ctx.cast_skill(npc, attacker, SUMMON_PC);
                }
                // NOTE(java-quirk): the same condition is tested and the skill cast a second time;
                // only this second cast sets `attacked`.
                if can_cast(ctx, npc) {
                    ctx.cast_skill(npc, attacker, SUMMON_PC);
                    ctx.world_mut()
                        .npc_variables_mut(npc.object_id)
                        .set_bool("attacked", true);
                }
            }
        } else if distance > 100.0 && !attacked {
            if let Some(most_hated) = ctx.most_hated(npc)
                && ((most_hated == attacker && chance < 50) || chance < 10)
                && can_cast(ctx, npc)
            {
                ctx.cast_skill(npc, attacker, SUMMON_PC);
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_bool("attacked", true);
            }
        }
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        if skill.id == SUMMON_PC.id && !ctx.is_dead(npc.object_id) && attacked(ctx, npc) {
            // NOTE(java-quirk): Java dereferences `player` here; a cast on a non-player target
            // throws and leaves `attacked` set.
            let Some(player) = player else {
                return;
            };
            // `player.teleToLocation(npc)`: the NPC's place, heading and instance, no offset.
            if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                let target = TeleportTarget::at(x, y, z)
                    .with_heading(ctx.heading_of(npc.object_id))
                    .with_instance(ctx.world().instance_id_of(npc.object_id));
                ctx.world_mut().tele_to_location(player, target);
            }
            ctx.world_mut()
                .npc_variables_mut(npc.object_id)
                .set_bool("attacked", false);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, X};

    const NPCS: &str = r#"<npc id="20213" type="Monster" level="30" name="Porta"><stats><vitals hp="1000" mp="500"/></stats></npc>"#;

    fn setup() -> (ScriptTest, ScriptNpc) {
        let mut registry = ScriptRegistry::new();
        registry.add(SummonPc);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let porta = t.spawn(PORTA, 1000);
        t.enter(1, 30, vec![]);
        let npc = ScriptNpc {
            object_id: porta,
            id: PORTA,
        };
        (t, npc)
    }

    #[test]
    fn the_spell_pulls_the_attacker_to_the_npc_once() {
        let (mut t, npc) = setup();
        // Not summoned yet: the finished spell does nothing.
        t.world.run_as_script("SummonPc", |ctx| {
            SummonPc.on_spell_finished(ctx, npc, Some(ObjectId(1)), SUMMON_PC);
        });
        assert_eq!(t.world.position_of(ObjectId(1)).unwrap().0, X);
        t.world
            .npc_variables_mut(npc.object_id)
            .set_bool("attacked", true);
        t.world.run_as_script("SummonPc", |ctx| {
            SummonPc.on_spell_finished(ctx, npc, Some(ObjectId(1)), SUMMON_PC);
        });
        assert_eq!(t.world.position_of(ObjectId(1)).unwrap().0, X + 1000);
        assert!(
            !t.world
                .npc_variables(npc.object_id)
                .unwrap()
                .get_bool("attacked", true)
        );
    }

    #[test]
    fn a_far_attacker_sets_the_attacked_flag() {
        let (mut t, npc) = setup();
        // The seeded RNG: hit until the "chance < 50" branch took the cast.
        for _ in 0..20 {
            t.world.run_as_script("SummonPc", |ctx| {
                SummonPc.on_attack(ctx, npc, ObjectId(1), 5, false, None);
            });
            if t.world
                .npc_variables(npc.object_id)
                .is_some_and(|v| v.get_bool("attacked", false))
            {
                return;
            }
        }
        panic!("the attacker was never summoned");
    }
}
