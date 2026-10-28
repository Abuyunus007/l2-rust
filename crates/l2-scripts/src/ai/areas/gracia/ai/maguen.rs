//! The Maguen of the Seed of Annihilation: the Wild Maguen that follows a character and gives
//! plasma buffs and pet collars.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/Maguen.java
//
// Java keeps the Maguen's `getVariables()` (`SUMMON_PLAYER`, `IS_NEAR_PLAYER`, `NPC_EFFECT`,
// `BLOCKED_SKILLSEE`, `TEST_MAGUEN`) on the NPC; they are the script state by NPC object here.
// Nemo's practice Maguen is set up by the event `PRACTICE_MAGUEN` (Java: Nemo writes the
// variables and starts `DIST_CHECK_TIMER` of this script itself).

use std::collections::HashMap;

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

use crate::ai::areas::gracia::util::{broadcast_social_action, distance_3d, set_display_effect};

// NPC
const MAGUEN: i32 = 18839; // Wild Maguen
const ELITES: [i32; 10] = [
    22750, // Elite Bgurent (Bistakon)
    22751, // Elite Brakian (Bistakon)
    22752, // Elite Groikan (Bistakon)
    22753, // Elite Treykan (Bistakon)
    22757, // Elite Turtlelian (Reptilikon)
    22758, // Elite Krajian (Reptilikon)
    22759, // Elite Tardyon (Reptilikon)
    22763, // Elite Kanibi (Kokracon)
    22764, // Elite Kiriona (Kokracon)
    22765, // Elite Kaiona (Kokracon)
];

// Item
const MAGUEN_PET: i32 = 15488; // Maguen Pet Collar
const ELITE_MAGUEN_PET: i32 = 15489; // Elite Maguen Pet Collar

// Skills
const MACHINE: SkillKey = SkillKey { id: 9060, level: 1 }; // Maguen Machine
const B_BUFF_1: SkillKey = SkillKey { id: 6343, level: 1 }; // Maguen Plasma - Power
const B_BUFF_2: SkillKey = SkillKey { id: 6343, level: 2 };
const C_BUFF_1: SkillKey = SkillKey { id: 6365, level: 1 }; // Maguen Plasma - Speed
const C_BUFF_2: SkillKey = SkillKey { id: 6365, level: 2 };
const R_BUFF_1: SkillKey = SkillKey { id: 6366, level: 1 }; // Maguen Plasma - Critical
const R_BUFF_2: SkillKey = SkillKey { id: 6366, level: 2 };
const B_PLASMA1: SkillKey = SkillKey { id: 6367, level: 1 }; // Maguen Plasma - Bistakon
const B_PLASMA2: SkillKey = SkillKey { id: 6367, level: 2 };
const B_PLASMA3: SkillKey = SkillKey { id: 6367, level: 3 };
const C_PLASMA1: SkillKey = SkillKey { id: 6368, level: 1 }; // Maguen Plasma - Cokrakon
const C_PLASMA2: SkillKey = SkillKey { id: 6368, level: 2 };
const C_PLASMA3: SkillKey = SkillKey { id: 6368, level: 3 };
const R_PLASMA1: SkillKey = SkillKey { id: 6369, level: 1 }; // Maguen Plasma - Reptilikon
const R_PLASMA2: SkillKey = SkillKey { id: 6369, level: 2 };
const R_PLASMA3: SkillKey = SkillKey { id: 6369, level: 3 };

/// The abnormal types of the three plasma skills (`B_PLASMA1.getSkill().getAbnormalType()` and so
/// on, from `stats/skills/06300-06399.xml`).
const B_ABNORMAL: &str = "SOA_BUFF1";
const C_ABNORMAL: &str = "SOA_BUFF2";
const R_ABNORMAL: &str = "SOA_BUFF3";

/// The Java variables of one Maguen.
#[derive(Default)]
struct MaguenVars {
    summon_player: Option<ObjectId>,
    is_near_player: bool,
    npc_effect: i32,
    blocked_skillsee: bool,
    test_maguen: bool,
}

#[derive(Default)]
struct MaguenState {
    maguens: HashMap<ObjectId, MaguenVars>,
}

/// Java `ai.areas.Gracia.AI.Maguen`.
pub struct Maguen;

impl Maguen {
    /// The `b`, `c`, `r` plasma levels of a player (0 without the buff).
    fn plasma_levels(ctx: &ScriptCtx<'_>, player: ObjectId) -> (i32, i32, i32) {
        let level = |abnormal| {
            ctx.world()
                .script_buff_abnormal_level(player, abnormal)
                .unwrap_or(0)
        };
        (level(B_ABNORMAL), level(C_ABNORMAL), level(R_ABNORMAL))
    }

    fn stop_plasmas(ctx: &mut ScriptCtx<'_>, player: ObjectId, b: bool, c: bool, r: bool) {
        if b {
            ctx.world_mut().script_stop_buffs_by_abnormal(player, B_ABNORMAL);
        }
        if c {
            ctx.world_mut().script_stop_buffs_by_abnormal(player, C_ABNORMAL);
        }
        if r {
            ctx.world_mut().script_stop_buffs_by_abnormal(player, R_ABNORMAL);
        }
    }

    fn maguen_pet_chance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        let chance1 = ctx.get_random(10000);
        let chance2 = ctx.get_random(20);
        if chance1 == 0 && chance2 != 0 {
            ctx.give_items(player, MAGUEN_PET, 1);
        } else if chance1 == 0 {
            ctx.give_items(player, ELITE_MAGUEN_PET, 1);
        }
    }

    /// `npc.setTarget(player); npc.doCast(70% skill1 : skill2)`.
    fn cast_buff(
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        first: SkillKey,
        second: SkillKey,
    ) {
        let skill = if ctx.get_random(100) < 70 { first } else { second };
        ctx.cast_skill(npc, player, skill);
    }

    fn vars<'a>(ctx: &'a mut ScriptCtx<'_>, npc: ObjectId) -> &'a mut MaguenVars {
        ctx.state::<MaguenState>().maguens.entry(npc).or_default()
    }
}

impl Script for Maguen {
    fn name(&self) -> &'static str {
        "Maguen"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_kill_id(&ELITES);
        s.add_skill_see_id(&[MAGUEN]);
        s.add_spell_finished_id(&[MAGUEN]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let (npc, player) = (npc?, player?);

        match event {
            "SPAWN_MAGUEN" => {
                let maguen = ctx.add_spawn_at(MAGUEN, npc.object_id, true, 60000)?;
                Self::vars(ctx, maguen.object_id).summon_player = Some(player);
                let name = ctx.player_name(player);
                let world = ctx.world_mut();
                world.script_set_npc_title(maguen.object_id, &name);
                world.script_set_running(maguen.object_id, true);
                world.script_start_follow(maguen.object_id, player);
                world.broadcast_status_update(maguen.object_id);
                ctx.show_on_screen_npc_string(player, NpcStringId::MaguenAppearance, 2, 4000, &[]);
                ctx.start_quest_timer("DIST_CHECK_TIMER", 1000, Some(maguen), Some(player), false);
            }
            // Java: Nemo's `summonMaguen` sets these variables on the practice Maguen and starts
            // the timer through `maguenAi().startQuestTimer(...)`.
            "PRACTICE_MAGUEN" => {
                let vars = Self::vars(ctx, npc.object_id);
                vars.summon_player = Some(player);
                vars.test_maguen = true;
                ctx.start_quest_timer("DIST_CHECK_TIMER", 1000, Some(npc), Some(player), false);
            }
            "DIST_CHECK_TIMER" => {
                let near = distance_3d(ctx, npc.object_id, player) < 100.0;
                if near && !Self::vars(ctx, npc.object_id).is_near_player {
                    Self::vars(ctx, npc.object_id).is_near_player = true;
                    ctx.start_quest_timer("FIRST_TIMER", 4000, Some(npc), Some(player), false);
                } else {
                    ctx.start_quest_timer("DIST_CHECK_TIMER", 1000, Some(npc), Some(player), false);
                }
            }
            "FIRST_TIMER" => {
                ctx.world_mut().script_stop_follow(npc.object_id);
                let random_effect = ctx.get_random_range(1, 3);
                set_display_effect(ctx, npc.object_id, random_effect);
                Self::vars(ctx, npc.object_id).npc_effect = random_effect;
                let delay = 5000 + i64::from(ctx.get_random(300));
                ctx.start_quest_timer("SECOND_TIMER", delay, Some(npc), Some(player), false);
                let action = ctx.get_random_range(1, 3);
                broadcast_social_action(ctx, npc.object_id, action);
            }
            "SECOND_TIMER" => {
                let random_effect = ctx.get_random_range(1, 3);
                set_display_effect(ctx, npc.object_id, 4);
                set_display_effect(ctx, npc.object_id, random_effect);
                Self::vars(ctx, npc.object_id).npc_effect = random_effect;
                let delay = 4600 + i64::from(ctx.get_random(600));
                ctx.start_quest_timer("THIRD_TIMER", delay, Some(npc), Some(player), false);
                let action = ctx.get_random_range(1, 3);
                broadcast_social_action(ctx, npc.object_id, action);
            }
            "THIRD_TIMER" => {
                let random_effect = ctx.get_random_range(1, 3);
                set_display_effect(ctx, npc.object_id, 4);
                set_display_effect(ctx, npc.object_id, random_effect);
                Self::vars(ctx, npc.object_id).npc_effect = random_effect;
                let delay = 4200 + i64::from(ctx.get_random(900));
                ctx.start_quest_timer("FORTH_TIMER", delay, Some(npc), Some(player), false);
                let action = ctx.get_random_range(1, 3);
                broadcast_social_action(ctx, npc.object_id, action);
            }
            "FORTH_TIMER" => {
                Self::vars(ctx, npc.object_id).npc_effect = 0;
                set_display_effect(ctx, npc.object_id, 4);
                ctx.start_quest_timer("END_TIMER", 500, Some(npc), Some(player), false);
                let action = ctx.get_random_range(1, 3);
                broadcast_social_action(ctx, npc.object_id, action);
            }
            "END_TIMER" => {
                if Self::vars(ctx, npc.object_id).test_maguen {
                    Self::stop_plasmas(ctx, player, true, true, true);
                    ctx.notify_event_of("Nemo", "DECREASE_COUNT", Some(npc), Some(player));
                }

                ctx.world_mut().do_die(npc.object_id, None);
                ctx.state::<MaguenState>().maguens.remove(&npc.object_id);
            }
            _ => {}
        }

        // `super.onEvent(event, npc, player)`
        None
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: Option<ObjectId>,
        _skill: SkillKey,
    ) {
        let Some(player) = player else {
            return;
        };
        let (b, c, r) = Self::plasma_levels(ctx, player);
        if b == 3 && c == 0 && r == 0 {
            ctx.show_on_screen_npc_string(
                player,
                NpcStringId::EnoughMaguenPlasmaBistakonHaveGathered,
                2,
                4000,
                &[],
            );
            Self::stop_plasmas(ctx, player, true, false, false);
            Self::cast_buff(ctx, npc, player, B_BUFF_1, B_BUFF_2);
            Self::maguen_pet_chance(ctx, player);
            ctx.start_quest_timer("END_TIMER", 3000, Some(npc), Some(player), false);
        } else if b == 0 && c == 3 && r == 0 {
            ctx.show_on_screen_npc_string(
                player,
                NpcStringId::EnoughMaguenPlasmaCokrakonHaveGathered,
                2,
                4000,
                &[],
            );
            Self::stop_plasmas(ctx, player, false, true, false);
            Self::cast_buff(ctx, npc, player, C_BUFF_1, C_BUFF_2);
            Self::maguen_pet_chance(ctx, player);
            ctx.start_quest_timer("END_TIMER", 3000, Some(npc), Some(player), false);
        } else if b == 0 && c == 0 && r == 3 {
            ctx.show_on_screen_npc_string(
                player,
                NpcStringId::EnoughMaguenPlasmaLeptilikonHaveGathered,
                2,
                4000,
                &[],
            );
            Self::stop_plasmas(ctx, player, false, false, true);
            Self::cast_buff(ctx, npc, player, R_BUFF_1, R_BUFF_2);
            Self::maguen_pet_chance(ctx, player);
            ctx.start_quest_timer("END_TIMER", 3000, Some(npc), Some(player), false);
        } else if b + c + r == 3 {
            if b == 1 && c == 1 && r == 1 {
                Self::stop_plasmas(ctx, player, true, true, true);
                ctx.show_on_screen_npc_string(
                    player,
                    NpcStringId::ThePlasmasHaveFilledTheAeroscopeAndAreHarmonized,
                    2,
                    4000,
                    &[],
                );
                match ctx.get_random(3) {
                    0 => Self::cast_buff(ctx, npc, player, B_BUFF_1, B_BUFF_2),
                    1 => Self::cast_buff(ctx, npc, player, C_BUFF_1, C_BUFF_2),
                    2 => Self::cast_buff(ctx, npc, player, R_BUFF_1, R_BUFF_2),
                    _ => {}
                }

                Self::maguen_pet_chance(ctx, player);
                ctx.start_quest_timer("END_TIMER", 3000, Some(npc), Some(player), false);
            } else {
                ctx.show_on_screen_npc_string(
                    player,
                    NpcStringId::ThePlasmasHaveFilledTheAeroscopeButTheyAreRammingIntoEachOtherExplodingAndDying,
                    2,
                    4000,
                    &[],
                );
                Self::stop_plasmas(ctx, player, true, true, true);
            }
        } else {
            ctx.start_quest_timer("END_TIMER", 1000, Some(npc), Some(player), false);
        }

        set_display_effect(ctx, npc.object_id, 4);
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        _targets: &[ObjectId],
        _is_summon: bool,
    ) {
        let vars = Self::vars(ctx, npc.object_id);
        if skill == MACHINE
            && vars.summon_player == Some(caster)
            && vars.npc_effect != 0
            && !vars.blocked_skillsee
        {
            let npc_effect = vars.npc_effect;
            let (i1, i2, i3) = Self::plasma_levels(ctx, caster);
            Self::stop_plasmas(ctx, caster, true, true, true);
            ctx.cancel_quest_timer("FIRST_TIMER", Some(npc), Some(caster));
            ctx.cancel_quest_timer("SECOND_TIMER", Some(npc), Some(caster));
            ctx.cancel_quest_timer("THIRD_TIMER", Some(npc), Some(caster));
            ctx.cancel_quest_timer("FORTH_TIMER", Some(npc), Some(caster));
            Self::vars(ctx, npc.object_id).blocked_skillsee = true;
            let skill_to_cast = match npc_effect {
                1 => match i1 {
                    0 => Some(B_PLASMA1),
                    1 => Some(B_PLASMA2),
                    2 => Some(B_PLASMA3),
                    _ => None,
                },
                2 => match i2 {
                    0 => Some(C_PLASMA1),
                    1 => Some(C_PLASMA2),
                    2 => Some(C_PLASMA3),
                    _ => None,
                },
                3 => match i3 {
                    0 => Some(R_PLASMA1),
                    1 => Some(R_PLASMA2),
                    2 => Some(R_PLASMA3),
                    _ => None,
                },
                _ => None,
            };

            if let Some(skill) = skill_to_cast {
                ctx.cast_skill(npc, caster, skill);
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.world().is_in_party(killer) {
            let Some(party_member) = ctx.get_random_party_member(killer) else {
                return;
            };
            let members = i32::try_from(ctx.world().party_members_of(killer).len()).unwrap_or(0);
            let i0 = 10 + (10 * members);
            if ctx.get_random(1000) < i0
                && distance_3d(ctx, npc.object_id, killer) < 2000.0
                && distance_3d(ctx, npc.object_id, party_member) < 2000.0
            {
                ctx.notify_event("SPAWN_MAGUEN", Some(npc), Some(party_member));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="18839" type="Monster" level="80" name="Wild Maguen"/>
        <npc id="22750" type="Monster" level="80" name="Elite Bgurent"/>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Maguen);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        let elite = t.spawn(22750, 20);
        t.take();
        (t, elite)
    }

    fn summon(t: &mut ScriptTest, elite: ObjectId) {
        t.world.run_as_script("Maguen", |ctx| {
            let npc = ctx.script_npc(elite);
            ctx.notify_event("SPAWN_MAGUEN", npc, Some(ObjectId(1)));
        });
    }

    #[test]
    fn a_maguen_appears_follows_the_character_and_leaves_after_its_dance() {
        let (mut t, elite) = setup();
        let before = t.world.npc_count();
        summon(&mut t, elite);
        let out = names_to(&t.take(), 1);
        assert!(out.contains(&"EX_SHOW_SCREEN_MESSAGE".to_owned()), "{out:?}");
        assert_eq!(t.world.npc_count(), before + 1);
        let maguen = t
            .world
            .npc_ids()
            .into_iter()
            .find(|id| t.world.npc(*id).is_some_and(|n| n.npc_id == MAGUEN))
            .unwrap();
        // It stands by the character at once: the four dance timers and the end follow.
        t.ticks(100);
        let out = names_to(&t.take(), 1);
        assert!(out.contains(&"EX_CHANGE_NPC_STATE".to_owned()), "{out:?}");
        assert!(out.contains(&"SOCIAL_ACTION".to_owned()), "{out:?}");
        assert!(!t.world.is_dead(maguen));
        // About 20 seconds in all (4000 + 5000..5300 + 4600..5200 + 4200..5100 + 500 ms).
        t.ticks(500);
        assert!(t.world.is_dead(maguen) || t.world.npc(maguen).is_none());
    }

    #[test]
    fn the_machine_is_ignored_without_a_dance_effect() {
        let (mut t, elite) = setup();
        summon(&mut t, elite);
        let maguen = t
            .world
            .npc_ids()
            .into_iter()
            .find(|id| t.world.npc(*id).is_some_and(|n| n.npc_id == MAGUEN))
            .unwrap();
        // `NPC_EFFECT` is 0 before the first timer: the skill sighting changes nothing.
        t.world.run_as_script("Maguen", |ctx| {
            let n = ctx.script_npc(maguen).unwrap();
            Maguen.on_skill_see(ctx, n, ObjectId(1), MACHINE, &[], false);
            assert!(!ctx.state::<MaguenState>().maguens[&maguen].blocked_skillsee);
        });
    }
}
