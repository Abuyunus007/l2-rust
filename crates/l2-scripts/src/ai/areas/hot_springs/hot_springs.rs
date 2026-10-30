//! The monsters of the Hot Springs pass their diseases on to whoever hits them.
// Java: dist/game/data/scripts/ai/areas/HotSprings/HotSprings.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const BANDERSNATCHLING: i32 = 21314;
const FLAVA: i32 = 21316;
const ATROXSPAWN: i32 = 21317;
const NEPENTHES: i32 = 21319;
const ATROX: i32 = 21321;
const BANDERSNATCH: i32 = 21322;
// Skills
const RHEUMATISM: i32 = 4551;
const CHOLERA: i32 = 4552;
const FLU: i32 = 4553;
const MALARIA: i32 = 4554;
// Misc
const DISEASE_CHANCE: i32 = 10;
const MONSTERS: [i32; 6] = [
    BANDERSNATCHLING,
    FLAVA,
    ATROXSPAWN,
    NEPENTHES,
    ATROX,
    BANDERSNATCH,
];

/// Java `ai.areas.HotSprings.HotSprings`.
pub struct HotSprings;

/// Java `spreadDisease(npc, creature)`.
fn spread_disease(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
    if ctx.get_random(100) < DISEASE_CHANCE {
        try_to_infect(ctx, npc, creature, MALARIA);
    }
    if ctx.get_random(100) < DISEASE_CHANCE {
        match npc.id {
            BANDERSNATCHLING | ATROX => try_to_infect(ctx, npc, creature, RHEUMATISM),
            FLAVA | NEPENTHES => try_to_infect(ctx, npc, creature, CHOLERA),
            ATROXSPAWN | BANDERSNATCH => try_to_infect(ctx, npc, creature, FLU),
            _ => {}
        }
    }
}

/// Java `tryToInfect(npc, creature, diseaseId)`: the next level of the disease, up to 10.
fn try_to_infect(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId, disease_id: i32) {
    let skill_level = match ctx.world().script_buff_skill_level(creature, disease_id) {
        None => 1,
        Some(level) if level < 10 => level + 1,
        Some(_) => 10,
    };
    let skill = SkillKey {
        id: disease_id,
        level: skill_level,
    };
    if ctx.world().skill_template_exists(skill.id, skill.level)
        && !ctx.world().is_casting_now(npc.object_id)
        && ctx.world_mut().check_do_cast_conditions_of(npc.object_id, skill)
    {
        ctx.cast_skill(npc, creature, skill);
    }
}

impl Script for HotSprings {
    fn name(&self) -> &'static str {
        "HotSprings"
    }

    fn path(&self) -> &'static str {
        "ai/areas/HotSprings"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&MONSTERS);
        s.add_skill_see_id(&MONSTERS);
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
        spread_disease(ctx, npc, attacker);
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        _skill: SkillKey,
        targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if targets.contains(&npc.object_id) {
            spread_disease(ctx, npc, caster);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="21314" type="Monster" level="60" name="Bandersnatchling"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn a_hit_or_a_skill_on_the_monster_rolls_for_a_disease_and_casts_nothing_without_the_skill() {
        let mut registry = ScriptRegistry::new();
        registry.add(HotSprings);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 60, vec![]);
        let mob = t.spawn(BANDERSNATCHLING, 30);
        let npc = ScriptNpc {
            object_id: mob,
            id: BANDERSNATCHLING,
        };
        // The disease skills are not in the test world: the rolls happen, nothing is cast.
        for _ in 0..50 {
            t.world.run_as_script("HotSprings", |ctx| {
                HotSprings.on_attack(ctx, npc, ObjectId(1), 5, false, None);
            });
        }
        assert!(!t.world.is_casting_now(mob));
        t.world.run_as_script("HotSprings", |ctx| {
            HotSprings.on_skill_see(ctx, npc, ObjectId(1), SkillKey::new(1, 1), &[mob], false);
            HotSprings.on_skill_see(ctx, npc, ObjectId(1), SkillKey::new(1, 1), &[], false);
        });
        assert!(!t.world.is_casting_now(mob));
    }
}
