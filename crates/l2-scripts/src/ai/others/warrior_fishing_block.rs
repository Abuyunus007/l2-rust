//! The monsters a Warrior Fishing Block call out of the water: they shout, attack the fisher and
//! vanish after 50 seconds.
// Java: dist/game/data/scripts/ai/others/WarriorFishingBlock.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// Monsters
const MONSTERS: [i32; 8] = [
    18319, // Caught Frog
    18320, // Caught Undine
    18321, // Caught Rakul
    18322, // Caught Sea Giant
    18323, // Caught Sea Horse Soldier
    18324, // Caught Homunculus
    18325, // Caught Flava
    18326, // Caught Gigantic Eye
];

// NPC Strings
const NPC_STRINGS_ON_SPAWN: [NpcStringId; 3] = [
    NpcStringId::CroakCroakFoodLikeS1InThisPlace,
    NpcStringId::S1HowLuckyIAm,
    NpcStringId::PrayThatYouCaughtAWrongFishS1,
];
const NPC_STRINGS_ON_ATTACK: [NpcStringId; 3] = [
    NpcStringId::DoYouKnowWhatAFrogTastesLike,
    NpcStringId::IWillShowYouThePowerOfAFrog,
    NpcStringId::IWillSwallowAtAMouthful,
];
const NPC_STRINGS_ON_KILL: [NpcStringId; 3] = [
    NpcStringId::UghNoChanceHowCouldThisElderPassAwayLikeThis,
    NpcStringId::CroakCroakAFrogIsDying,
    NpcStringId::AFrogTastesBadYuck,
];

// Misc
const CHANCE_TO_SHOUT_ON_ATTACK: i32 = 33;
const DESPAWN_TIME: i64 = 50; // 50 seconds to despawn

/// Java `ai.others.WarriorFishingBlock`.
pub struct WarriorFishingBlock;

impl Script for WarriorFishingBlock {
    fn name(&self) -> &'static str {
        "WarriorFishingBlock"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&MONSTERS);
        s.add_kill_id(&MONSTERS);
        s.add_spawn_id(&MONSTERS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        let npc = npc?;
        match event {
            "SPAWN" => {
                let target = ctx
                    .world()
                    .creature_target(npc.object_id)
                    .filter(|&t| ctx.world().player(t).is_some());
                match target {
                    None => ctx.world_mut().decay(npc.object_id),
                    Some(target) => {
                        let name = ctx.player_name(target);
                        if let Some(text) = ctx.get_random_entry(&NPC_STRINGS_ON_SPAWN) {
                            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[name.as_str()]);
                        }
                        ctx.world_mut().add_damage_hate(npc.object_id, target, 0, 2000);
                        ctx.world_mut().notify_action_attacked(npc.object_id, target);
                        ctx.world_mut().add_attacker_to_attack_by(npc.object_id, target);

                        ctx.start_quest_timer("DESPAWN", DESPAWN_TIME * 1000, Some(npc), Some(target), false);
                    }
                }
            }
            "DESPAWN" => {
                ctx.world_mut().decay(npc.object_id);
            }
            _ => {}
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
        if ctx.get_random(100) < CHANCE_TO_SHOUT_ON_ATTACK {
            if let Some(text) = ctx.get_random_entry(&NPC_STRINGS_ON_ATTACK) {
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(text) = ctx.get_random_entry(&NPC_STRINGS_ON_KILL) {
            ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, text, &[]);
        }
        ctx.cancel_quest_timer("DESPAWN", Some(npc), Some(killer));
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.cancel_quest_timer("SPAWN", Some(npc), None);
        ctx.cancel_quest_timer("DESPAWN", Some(npc), None);
        ctx.start_quest_timer("SPAWN", 2000, Some(npc), None, false);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPC_DEFS: &str = r#"<npc id="18319" type="Monster" level="30" name="Caught Frog"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;

    #[test]
    fn a_frog_without_a_player_target_vanishes() {
        let mut registry = ScriptRegistry::new();
        registry.add(WarriorFishingBlock);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let frog = t.spawn(18319, 20);
        t.enter(1, 30, vec![]);
        t.world.script_drain();
        t.wait_ms(2_500);
        assert!(!t.world.is_object_spawned(frog));
    }

    #[test]
    fn a_frog_with_a_target_attacks_and_vanishes_after_fifty_seconds() {
        let mut registry = ScriptRegistry::new();
        registry.add(WarriorFishingBlock);
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &[]);
        let frog = t.spawn(18319, 20);
        t.enter(1, 30, vec![]);
        t.world.set_npc_target(frog, Some(ObjectId(1)));
        t.world.script_drain();
        t.wait_ms(2_500);
        assert!(t.world.is_object_spawned(frog));
        t.wait_ms(50_000);
        assert!(!t.world.is_object_spawned(frog));
    }
}
