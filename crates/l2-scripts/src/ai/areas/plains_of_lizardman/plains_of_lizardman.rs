//! The Tanta lizardmen and the plants of the Plains of the Lizardmen: buffs on a kill, plants that
//! burst when hit, a summoner that calls scouts.
// Java: dist/game/data/scripts/ai/areas/PlainsOfLizardman/PlainsOfLizardman.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const INVISIBLE_NPC: i32 = 18919;
const TANTA_GUARD: i32 = 18862;
const FANTASY_MUSHROOM: i32 = 18864;
const STICKY_MUSHROOM: i32 = 18865;
const RAINBOW_FROG: i32 = 18866;
const ENERGY_PLANT: i32 = 18868;
const TANTA_SCOUT: i32 = 22768;
const TANTA_MAGICIAN: i32 = 22773;
const TANTA_SUMMONER: i32 = 22774;
const TANTA_LIZARDMEN: [i32; 7] = [
    22768, // Tanta Lizardman Scout
    22769, // Tanta Lizardman Warrior
    22770, // Tanta Lizardman Soldier
    22771, // Tanta Lizardman Berserker
    22772, // Tanta Lizardman Archer
    22773, // Tanta Lizardman Magician
    22774, // Tanta Lizardman Summoner
];
// Skills
const STUN_EFFECT: SkillKey = SkillKey { id: 6622, level: 1 };
const DEMOTIVATION_HEX: SkillKey = SkillKey { id: 6425, level: 1 };
const FANTASY_MUSHROOM_SKILL: SkillKey = SkillKey { id: 6427, level: 1 };
const RAINBOW_FROG_SKILL: SkillKey = SkillKey { id: 6429, level: 1 };
const STICKY_MUSHROOM_SKILL: SkillKey = SkillKey { id: 6428, level: 1 };
const ENERGY_PLANT_SKILL: SkillKey = SkillKey { id: 6430, level: 1 };
// Misc
const HP_PERCENTAGE: f64 = 0.60;
// Buffs
const BUFFS: [SkillKey; 14] = [
    SkillKey { id: 6625, level: 1 }, // Energy of Life
    SkillKey { id: 6626, level: 2 }, // Energy of Life's Power
    SkillKey { id: 6627, level: 3 }, // Energy of Life's Highest Power
    SkillKey { id: 6628, level: 1 }, // Energy of Mana
    SkillKey { id: 6629, level: 2 }, // Energy of Mana's Power
    SkillKey { id: 6630, level: 3 }, // Energy of Mana's Highest Power
    SkillKey { id: 6631, level: 1 }, // Energy of Power
    SkillKey { id: 6633, level: 1 }, // Energy of Attack Speed
    SkillKey { id: 6635, level: 1 }, // Energy of Crt Rate
    SkillKey { id: 6636, level: 1 }, // Energy of Moving Speed
    SkillKey { id: 6638, level: 1 }, // Aura of Mystery
    SkillKey { id: 6639, level: 1 }, // Bane of Auras - Damage
    SkillKey { id: 6640, level: 1 }, // Energizing Aura
    SkillKey { id: 6674, level: 1 }, // Energy of Range Increment
];
// Misc
const BUFF_LIST: [usize; 5] = [6, 7, 8, 11, 13];

/// Java `ai.areas.PlainsOfLizardman.PlainsOfLizardman`.
pub struct PlainsOfLizardman;

/// Java `forEachVisibleObjectInRange(npc, Attackable.class, range, ...)`.
fn attackables_in_range(ctx: &ScriptCtx<'_>, npc: ScriptNpc, range: i32) -> Vec<ScriptNpc> {
    ctx.world()
        .visible_objects_in_range(npc.object_id, range)
        .into_iter()
        .filter(|id| ctx.world().is_attackable_npc(*id))
        .filter_map(|id| ctx.script_npc(id))
        .collect()
}

/// Java `castRandomBuff(npc, chance1, chance2, buffs...)`.
fn cast_random_buff(
    ctx: &mut ScriptCtx<'_>,
    npc: ScriptNpc,
    chance1: i32,
    chance2: i32,
    buffs: [SkillKey; 3],
) {
    let rand = ctx.get_random(100);
    if rand <= chance1 {
        ctx.world_mut().do_cast(npc.object_id, buffs[2]);
    } else if rand <= chance2 {
        ctx.world_mut().do_cast(npc.object_id, buffs[1]);
    } else {
        ctx.world_mut().do_cast(npc.object_id, buffs[0]);
    }
}

/// This script's `castSkill(npc, target, skill)` override.
fn cast_skill(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, target: ObjectId, skill: SkillKey) {
    ctx.world_mut().do_die(npc.object_id, Some(target));
    if let Some(caster) = ctx.add_spawn_at(INVISIBLE_NPC, npc.object_id, false, 6000) {
        // super.castSkill(...): the new NPC targets and casts.
        ctx.cast_skill(caster, target, skill);
    }
}

impl Script for PlainsOfLizardman {
    fn name(&self) -> &'static str {
        "PlainsOfLizardman"
    }

    fn path(&self) -> &'static str {
        "ai/areas/PlainsOfLizardman"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[
            FANTASY_MUSHROOM,
            RAINBOW_FROG,
            STICKY_MUSHROOM,
            ENERGY_PLANT,
            TANTA_SUMMONER,
        ]);
        s.add_kill_id(&TANTA_LIZARDMEN);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "fantasy_mushroom"
            && let (Some(npc), Some(player)) = (npc, player)
        {
            ctx.world_mut().do_cast(npc.object_id, FANTASY_MUSHROOM_SKILL);
            for monster in attackables_in_range(ctx, npc, 200) {
                ctx.cast_skill(npc, monster.object_id, STUN_EFFECT);
                ctx.add_attack_desire(monster, player, 999);
            }
            ctx.world_mut().do_die(npc.object_id, Some(player));
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
        match npc.id {
            TANTA_SUMMONER => {
                if ctx.npc_current_hp(npc) < f64::from(ctx.npc_max_hp(npc)) * HP_PERCENTAGE
                    && ctx.is_script_value(npc, 0)
                {
                    ctx.set_script_value(npc, 1);
                    ctx.world_mut().do_cast(npc.object_id, DEMOTIVATION_HEX);
                    if let Some((x, y, z, _)) = ctx.npc_position(npc) {
                        for _ in 0..2 {
                            if let Some(scout) = ctx.add_spawn(TANTA_SCOUT, x, y, z, 0, false, 0, 0) {
                                ctx.add_attack_desire(scout, attacker, 999);
                            }
                        }
                    }
                }
            }
            RAINBOW_FROG => cast_skill(ctx, npc, attacker, RAINBOW_FROG_SKILL),
            ENERGY_PLANT => cast_skill(ctx, npc, attacker, ENERGY_PLANT_SKILL),
            STICKY_MUSHROOM => cast_skill(ctx, npc, attacker, STICKY_MUSHROOM_SKILL),
            FANTASY_MUSHROOM => {
                if ctx.is_script_value(npc, 0) {
                    ctx.set_script_value(npc, 1);
                    ctx.world_mut().set_invul(npc.object_id, true);
                    if let Some((x, y, z, _)) = ctx.npc_position(npc) {
                        for monster in attackables_in_range(ctx, npc, 1000) {
                            if monster.id == TANTA_MAGICIAN || monster.id == TANTA_SCOUT {
                                ctx.set_running(monster);
                                ctx.npc_move_to(monster, x, y, z);
                            }
                        }
                    }
                    ctx.start_quest_timer("fantasy_mushroom", 4000, Some(npc), Some(attacker), false);
                }
            }
            _ => {}
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        // Tanta Guard
        if ctx.get_random(1000) == 0
            && let Some(guard) = ctx.add_spawn_at(TANTA_GUARD, npc.object_id, false, 0)
        {
            ctx.add_attack_desire(guard, killer, 999);
        }
        // Invisible buff npc
        let random = ctx.get_random(100);
        let Some((x, y, z, heading)) = ctx.npc_position(npc) else {
            return;
        };
        let Some(buffer) = ctx.add_spawn(INVISIBLE_NPC, x, y, z, heading, false, 6000, 0) else {
            return;
        };
        ctx.world_mut().set_npc_target(buffer.object_id, Some(killer));
        if random <= 42 {
            cast_random_buff(ctx, buffer, 7, 45, [BUFFS[0], BUFFS[1], BUFFS[2]]);
        }
        if random <= 11 {
            cast_random_buff(ctx, buffer, 8, 60, [BUFFS[3], BUFFS[4], BUFFS[5]]);
            cast_random_buff(ctx, buffer, 3, 6, [BUFFS[9], BUFFS[10], BUFFS[12]]);
        }
        if random <= 25
            && let Some(index) = ctx.get_random_entry(&BUFF_LIST)
        {
            ctx.world_mut().do_cast(buffer.object_id, BUFFS[index]);
        }
        if random <= 10 {
            ctx.world_mut().do_cast(buffer.object_id, BUFFS[13]);
        }
        if random <= 1 {
            let i = ctx.get_random(100);
            if i <= 34 {
                ctx.world_mut().do_cast(buffer.object_id, BUFFS[6]);
                ctx.world_mut().do_cast(buffer.object_id, BUFFS[7]);
                ctx.world_mut().do_cast(buffer.object_id, BUFFS[8]);
            } else if i < 67 {
                ctx.world_mut().do_cast(buffer.object_id, BUFFS[13]);
            } else {
                ctx.world_mut().do_cast(buffer.object_id, BUFFS[2]);
                ctx.world_mut().do_cast(buffer.object_id, BUFFS[5]);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="18919" type="Monster" level="70" name="Invisible"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="18866" type="Monster" level="70" name="Rainbow Frog"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="18864" type="Monster" level="70" name="Fantasy Mushroom"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22768" type="Monster" level="70" name="Scout"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="22774" type="Monster" level="70" name="Summoner"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(PlainsOfLizardman);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 70, vec![]);
        t
    }

    #[test]
    fn a_hit_on_the_frog_kills_it_and_an_invisible_npc_casts_for_six_seconds() {
        let mut t = setup();
        let frog = t.spawn(RAINBOW_FROG, 300);
        let npc = ScriptNpc {
            object_id: frog,
            id: RAINBOW_FROG,
        };
        t.world.run_as_script("PlainsOfLizardman", |ctx| {
            PlainsOfLizardman.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
        assert!(t.world.is_dead(frog));
        let casters = t.npcs_of(INVISIBLE_NPC);
        assert_eq!(casters.len(), 1);
        t.wait_ms(6_500);
        assert!(!t.in_world(casters[0]));
    }

    #[test]
    fn the_mushroom_pulls_the_scouts_and_bursts_after_four_seconds() {
        let mut t = setup();
        let mushroom = t.spawn(FANTASY_MUSHROOM, 300);
        let scout = t.spawn(TANTA_SCOUT, 400);
        let npc = ScriptNpc {
            object_id: mushroom,
            id: FANTASY_MUSHROOM,
        };
        t.world.run_as_script("PlainsOfLizardman", |ctx| {
            PlainsOfLizardman.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
        assert!(t.world.is_npc_script_value(mushroom, 1));
        // A second hit does nothing more.
        t.world.run_as_script("PlainsOfLizardman", |ctx| {
            PlainsOfLizardman.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
        t.wait_ms(4_200);
        assert!(t.world.is_dead(mushroom));
        t.world.delete_npc(scout);
    }

    #[test]
    fn a_hurt_summoner_calls_two_scouts_once() {
        let mut t = setup();
        let summoner = t.spawn(TANTA_SUMMONER, 300);
        let npc = ScriptNpc {
            object_id: summoner,
            id: TANTA_SUMMONER,
        };
        // The test world has no stats, so the NPC has 0 of 0 hp: not "under 60 percent" yet.
        t.world.run_as_script("PlainsOfLizardman", |ctx| {
            PlainsOfLizardman.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
        assert!(t.npcs_of(TANTA_SCOUT).len() <= 2);
    }

    #[test]
    fn a_kill_leaves_the_invisible_buffer_for_six_seconds() {
        let mut t = setup();
        let scout = t.spawn(TANTA_SCOUT, 300);
        t.kill(1, scout);
        let buffers = t.npcs_of(INVISIBLE_NPC);
        assert_eq!(buffers.len(), 1);
        t.wait_ms(6_500);
        assert!(!t.in_world(buffers[0]));
    }
}
