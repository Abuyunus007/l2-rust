//! Monastery of Silence AI: the Solina knights train on the scarecrows and the monks do not
//! tolerate weapons.
// Java: dist/game/data/scripts/ai/areas/MonasteryOfSilence/MonasteryOfSilence.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::effects::registry::EffectType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const CAPTAIN: i32 = 18910; // Solina Knight Captain
const KNIGHT: i32 = 18909; // Solina Knights
const SCARECROW: i32 = 18912; // Scarecrow
const GUIDE: i32 = 22789; // Guide Solina
const SEEKER: i32 = 22790; // Seeker Solina
const SAVIOR: i32 = 22791; // Savior Solina
const ASCETIC: i32 = 22793; // Ascetic Solina
const DIVINITY_CLAN: [i32; 2] = [
    22794, // Divinity Judge
    22795, // Divinity Manager
];

// Skills
const ORDEAL_STRIKE: SkillKey = SkillKey { id: 6303, level: 1 }; // Trial of the Coup
const LEADER_STRIKE: SkillKey = SkillKey { id: 6304, level: 1 }; // Shock
const SAVER_STRIKE: SkillKey = SkillKey { id: 6305, level: 1 }; // Sacred Gnosis
const SAVER_BLEED: SkillKey = SkillKey { id: 6306, level: 1 }; // Solina Strike
const LEARNING_MAGIC: SkillKey = SkillKey { id: 6308, level: 1 }; // Opus of the Wave
const STUDENT_CANCEL: SkillKey = SkillKey { id: 6310, level: 1 }; // Loss of Quest
const WARRIOR_THRUSTING: SkillKey = SkillKey { id: 6311, level: 1 }; // Solina Thrust
const KNIGHT_BLESS: SkillKey = SkillKey { id: 6313, level: 1 }; // Solina Bless

// Misc
const DIVINITY_MSG: [NpcStringId; 2] = [
    NpcStringId::S1WhyWouldYouChooseThePathOfDarkness,
    NpcStringId::S1HowDareYouDefyTheWillOfEinhasad,
];
const SOLINA_KNIGHTS_MSG: [NpcStringId; 3] = [
    NpcStringId::PunishAllThoseWhoTreadFootstepsInThisPlace,
    NpcStringId::WeAreTheSwordOfTruthTheSwordOfSolina,
    NpcStringId::WeRaiseOurBladesForTheGloryOfSolina,
];

/// Java `npc.checkDoCastConditions(skill)`, `npc.setTarget(target)`, `npc.doCast(skill)`.
fn cast_at(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, target: ObjectId, skill: SkillKey) {
    if ctx.world_mut().check_do_cast_conditions_of(npc.object_id, skill) {
        ctx.world_mut().set_npc_target(npc.object_id, Some(target));
        ctx.world_mut().do_cast(npc.object_id, skill);
    }
}

/// Java `ai.areas.MonasteryOfSilence.MonasteryOfSilence`.
pub struct MonasteryOfSilence;

impl Script for MonasteryOfSilence {
    fn name(&self) -> &'static str {
        "MonasteryOfSilence"
    }

    fn path(&self) -> &'static str {
        "ai/areas/MonasteryOfSilence"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_skill_see_id(&DIVINITY_CLAN);
        s.add_attack_id(&[KNIGHT, CAPTAIN, GUIDE, SEEKER, ASCETIC]);
        s.add_npc_hate_id(&[GUIDE, SEEKER, SAVIOR, ASCETIC]);
        s.add_aggro_range_enter_id(&[GUIDE, SEEKER, SAVIOR, ASCETIC]);
        s.add_spawn_id(&[SCARECROW]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "TRAINING" => {
                let npc = npc?;
                for object in ctx.world().visible_objects_in_range(npc.object_id, 400) {
                    if !ctx.world().is_npc(object) {
                        continue;
                    }
                    let Some(character) = ctx.script_npc(object) else {
                        continue;
                    };
                    if (ctx.get_random(100) < 30) && !ctx.is_dead(object) && !ctx.is_in_combat(object) {
                        // NOTE(java-quirk): the script value tested is the scarecrow's (`npc`),
                        // the one set is the captain's (`character`).
                        if (character.id == CAPTAIN)
                            && (ctx.get_random(100) < 10)
                            && ctx.is_script_value(npc, 0)
                        {
                            if let Some(text) = ctx.get_random_entry(&SOLINA_KNIGHTS_MSG) {
                                ctx.broadcast_npc_say(character, ChatType::NpcGeneral, text, &[]);
                            }
                            ctx.set_script_value(character, 1);
                            ctx.start_quest_timer("TIMER", 10_000, Some(character), None, false);
                        } else if character.id == KNIGHT {
                            ctx.set_running(character);
                            ctx.add_attack_desire(character, npc.object_id, 100);
                        }
                    }
                }
            }
            "DO_CAST" => {
                if let (Some(npc), Some(player)) = (npc, player) {
                    if ctx.get_random(100) < 3 {
                        cast_at(ctx, npc, player, STUDENT_CANCEL);

                        ctx.set_script_value(npc, 0);
                    }
                }
            }
            "TIMER" => {
                if let Some(npc) = npc {
                    ctx.set_script_value(npc, 0);
                }
            }
            _ => {}
        }

        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        match npc.id {
            KNIGHT => {
                if (ctx.get_random(100) < 10)
                    && (ctx.most_hated(npc) == Some(player))
                    && ctx
                        .world_mut()
                        .check_do_cast_conditions_of(npc.object_id, WARRIOR_THRUSTING)
                {
                    ctx.world_mut().set_npc_target(npc.object_id, Some(player));
                    ctx.world_mut().do_cast(npc.object_id, WARRIOR_THRUSTING);
                }
            }
            CAPTAIN => {
                if (ctx.get_random(100) < 20)
                    && (ctx.npc_current_hp(npc) < (f64::from(ctx.npc_max_hp(npc)) * 0.5))
                    && ctx.is_script_value(npc, 0)
                {
                    cast_at(ctx, npc, npc.object_id, KNIGHT_BLESS);

                    ctx.set_script_value(npc, 1);
                    ctx.broadcast_npc_say(npc, ChatType::General, NpcStringId::ForTheGloryOfSolina, &[]);
                    if let Some(knight) = ctx.add_spawn_at(KNIGHT, npc.object_id, false, 0) {
                        ctx.add_attack_desire(knight, player, 999);
                    }
                }
            }
            GUIDE => {
                if (ctx.get_random(100) < 3)
                    && (ctx.most_hated(npc) == Some(player))
                    && ctx
                        .world_mut()
                        .check_do_cast_conditions_of(npc.object_id, ORDEAL_STRIKE)
                {
                    ctx.world_mut().set_npc_target(npc.object_id, Some(player));
                    ctx.world_mut().do_cast(npc.object_id, ORDEAL_STRIKE);
                }
            }
            SEEKER => {
                if (ctx.get_random(100) < 33)
                    && (ctx.most_hated(npc) == Some(player))
                    && ctx
                        .world_mut()
                        .check_do_cast_conditions_of(npc.object_id, SAVER_STRIKE)
                {
                    ctx.world_mut().set_npc_target(npc.object_id, Some(npc.object_id));
                    ctx.world_mut().do_cast(npc.object_id, SAVER_STRIKE);
                }
            }
            ASCETIC => {
                if (ctx.most_hated(npc) == Some(player)) && ctx.is_script_value(npc, 0) {
                    ctx.set_script_value(npc, 1);
                    ctx.start_quest_timer("DO_CAST", 20_000, Some(npc), Some(player), false);
                }
            }
            _ => {}
        }
    }

    fn on_npc_hate(
        &self,
        ctx: &mut ScriptCtx<'_>,
        _mob: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) -> bool {
        ctx.active_weapon_item_id(player).is_some()
    }

    fn on_aggro_range_enter(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _is_summon: bool,
    ) {
        if ctx.active_weapon_item_id(player).is_some() {
            let mut skill = None;
            match npc.id {
                GUIDE => {
                    if ctx.get_random(100) < 3 {
                        skill = Some(LEADER_STRIKE);
                    }
                }
                SEEKER => {
                    skill = Some(SAVER_BLEED);
                }
                SAVIOR => {
                    skill = Some(LEARNING_MAGIC);
                }
                ASCETIC => {
                    if ctx.get_random(100) < 3 {
                        skill = Some(STUDENT_CANCEL);
                    }

                    if ctx.is_script_value(npc, 0) {
                        ctx.set_script_value(npc, 1);
                        ctx.start_quest_timer("DO_CAST", 20_000, Some(npc), Some(player), false);
                    }
                }
                _ => {}
            }

            if let Some(skill) = skill {
                cast_at(ctx, npc, player, skill);
            }

            if !ctx.is_in_combat(npc.object_id) {
                ctx.broadcast_npc_say(
                    npc,
                    ChatType::NpcGeneral,
                    NpcStringId::YouCannotCarryAWeaponWithoutAuthorization,
                    &[],
                );
            }

            ctx.add_attack_desire(npc, player, 999);
        }
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        targets: &[ObjectId],
        _is_summon: bool,
    ) {
        if ctx
            .world_mut()
            .script_skill_has_effect_type(skill, &[EffectType::Aggression])
            && !targets.is_empty()
        {
            for obj in targets {
                if *obj == npc.object_id {
                    if let Some(text) = ctx.get_random_entry(&DIVINITY_MSG) {
                        let name = ctx.player_name(caster);
                        ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[&name]);
                    }
                    ctx.add_attack_desire(npc, caster, 999);
                    break;
                }
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().set_invul(npc.object_id, true);
        ctx.disable_core_ai(npc, true);
        ctx.cancel_quest_timer("TRAINING", Some(npc), None);
        ctx.start_quest_timer("TRAINING", 30_000, Some(npc), None, true);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="18909" type="Monster" level="70" name="Solina Knight"/>
        <npc id="18912" type="Monster" level="70" name="Scarecrow"/>
        <npc id="22791" type="Monster" level="70" name="Savior Solina"/>"#;

    #[test]
    fn the_scarecrow_is_invulnerable_and_calls_the_knights_to_train() {
        let mut registry = ScriptRegistry::new();
        registry.add(MonasteryOfSilence);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let scarecrow = t.spawn(SCARECROW, 10);
        t.ticks(2);
        assert!(t.world.is_invul(scarecrow));
        t.world.run_as_script("MonasteryOfSilence", |ctx| {
            let npc = ctx.script_npc(scarecrow).unwrap();
            assert!(ctx.has_quest_timer("TRAINING", Some(npc), None));
        });
    }

    #[test]
    fn the_monks_do_not_hate_a_player_without_a_weapon() {
        let mut registry = ScriptRegistry::new();
        registry.add(MonasteryOfSilence);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 75, vec![]);
        let savior = t.spawn(SAVIOR, 30);
        t.world.run_as_script("MonasteryOfSilence", |ctx| {
            let npc = ctx.script_npc(savior).unwrap();
            assert!(!MonasteryOfSilence.on_npc_hate(ctx, npc, ObjectId(1), false));
        });
    }
}
