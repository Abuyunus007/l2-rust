//! The Treasure Chest of the Ancient Giants and its guards in the Silent Valley.
// Java: dist/game/data/scripts/ai/areas/SilentValley/SilentValley.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// Skills
const BETRAYAL: SkillKey = SkillKey { id: 6033, level: 1 }; // Treasure Seeker's Betrayal
const BLAZE: SkillKey = SkillKey { id: 4157, level: 10 }; // NPC Blaze - Magic
// Item
const SACK: i32 = 13799; // Treasure Sack of the Ancient Giants
// Chance
const SPAWN_CHANCE: i32 = 2;
const CHEST_DIE_CHANCE: i32 = 5;
// Monsters
const CHEST: i32 = 18693; // Treasure Chest of the Ancient Giants
const GUARD1: i32 = 18694; // Treasure Chest Guard
const GUARD2: i32 = 18695; // Treasure Chest Guard
const MOBS: [i32; 9] = [
    20965, // Chimera Piece
    20966, // Changed Creation
    20967, // Past Creature
    20968, // Nonexistent Man
    20969, // Giant's Shadow
    20970, // Soldier of Ancient Times
    20971, // Warrior of Ancient Times
    20972, // Shaman of Ancient Times
    20973, // Forgotten Ancient People
];

/// Java `ai.areas.SilentValley.SilentValley`.
pub struct SilentValley;

/// The player of a playable: a summon's owner, the player itself.
fn player_of(ctx: &ScriptCtx<'_>, creature: ObjectId) -> Option<ObjectId> {
    if ctx.world().is_summon(creature) {
        ctx.world().summon_owner(creature)
    } else {
        ctx.world().player(creature).map(|_| creature)
    }
}

impl Script for SilentValley {
    fn name(&self) -> &'static str {
        "SilentValley"
    }

    fn path(&self) -> &'static str {
        "ai/areas/SilentValley"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&MOBS);
        s.add_attack_id(&[CHEST, GUARD1, GUARD2]);
        s.add_event_received_id(&[GUARD1, GUARD2]);
        s.add_kill_id(&MOBS);
        s.add_spawn_id(&[CHEST, GUARD2]);
        s.add_creature_see_id(&[GUARD1, GUARD2]);
        s.add_creature_see_id(&MOBS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(npc) = npc
            && !ctx.is_dead(npc.object_id)
        {
            match event {
                "CLEAR" => {
                    ctx.world_mut().do_die(npc.object_id, None);
                }
                "CLEAR_EVENT" => {
                    ctx.world_mut()
                        .broadcast_event(npc.object_id, "CLEAR_ALL_INSTANT", 2000, None);
                    ctx.world_mut().do_die(npc.object_id, None);
                }
                "SPAWN_CHEST" => {
                    if let Some((x, y, z)) = ctx.world().position_of(npc.object_id) {
                        ctx.add_spawn(CHEST, x - 100, y, z - 100, 0, false, 0, 0);
                    }
                }
                _ => {}
            }
        }
        None
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        player: ObjectId,
        _damage: i32,
        is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        match npc.id {
            CHEST => {
                if !is_summon && ctx.is_script_value(npc, 0) {
                    ctx.set_script_value(npc, 1);
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::YouWillBeCursedForSeekingTheTreasure,
                        &[],
                    );
                    ctx.cast_skill(npc, player, BETRAYAL);
                } else if is_summon || ctx.get_random(100) < CHEST_DIE_CHANCE {
                    ctx.drop_item(npc, Some(player), SACK, 1);
                    ctx.world_mut()
                        .broadcast_event(npc.object_id, "CLEAR_ALL", 2000, None);
                    ctx.world_mut().do_die(npc.object_id, None);
                    ctx.cancel_quest_timer("CLEAR_EVENT", Some(npc), None);
                }
            }
            GUARD1 | GUARD2 => {
                ctx.cast_skill(npc, player, BLAZE);
                ctx.add_attack_desire(npc, player, 999);
            }
            _ => {
                if is_summon {
                    ctx.add_attack_desire(npc, player, 999);
                }
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        if ctx.get_random(1000) < SPAWN_CHANCE
            && let Some((x, y, z)) = ctx.world().position_of(npc.object_id)
        {
            let new_z = z + 100;
            ctx.add_spawn(GUARD2, x + 100, y, new_z, 0, false, 0, 0);
            ctx.add_spawn(GUARD1, x - 100, y, new_z, 0, false, 0, 0);
            ctx.add_spawn(GUARD1, x, y + 100, new_z, 0, false, 0, 0);
            ctx.add_spawn(GUARD1, x, y - 100, new_z, 0, false, 0, 0);
        }
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if let Some(player) = player_of(ctx, creature) {
            if npc.id == GUARD1 || npc.id == GUARD2 {
                ctx.cast_skill(npc, player, BLAZE);
                ctx.add_attack_desire(npc, player, 999);
            } else if ctx.is_affected_by_skill(creature, BETRAYAL.id) {
                ctx.add_attack_desire(npc, player, 999);
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        if npc.id == CHEST {
            ctx.world_mut().set_invul(npc.object_id, true);
            ctx.start_quest_timer("CLEAR_EVENT", 300000, Some(npc), None, false);
        } else {
            ctx.start_quest_timer("SPAWN_CHEST", 10000, Some(npc), None, false);
        }
    }

    fn on_event_received(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event_name: &str,
        _sender: ScriptNpc,
        receiver: ScriptNpc,
        _reference: Option<ObjectId>,
    ) -> Option<String> {
        if !ctx.is_dead(receiver.object_id) {
            match event_name {
                "CLEAR_ALL" => {
                    ctx.start_quest_timer("CLEAR", 60000, Some(receiver), None, false);
                }
                "CLEAR_ALL_INSTANT" => {
                    ctx.world_mut().do_die(receiver.object_id, None);
                }
                _ => {}
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"
        <npc id="18693" type="Monster" level="70" name="Chest"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="18694" type="Monster" level="70" name="Guard 1"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="18695" type="Monster" level="70" name="Guard 2"><stats><vitals hp="1000" mp="100"/></stats></npc>
        <npc id="20965" type="Monster" level="70" name="Chimera Piece"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;
    const ITEMS: &str = r#"<item id="13799" type="EtcItem" name="Treasure Sack"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(SilentValley);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        t.enter(1, 70, vec![]);
        t
    }

    #[test]
    fn a_guard_spawns_a_chest_after_ten_seconds_and_the_chest_goes_after_five_minutes() {
        let mut t = setup();
        let guard = t.spawn(GUARD2, 300);
        t.ticks(2);
        t.wait_ms(10_500);
        let chests = t.npcs_of(CHEST);
        assert_eq!(chests.len(), 1);
        t.world.delete_npc(guard);
        // The chest dies by itself (and tells the guards to go) after 5 minutes.
        t.wait_ms(300_500);
        assert!(t.world.is_dead(chests[0]));
    }

    #[test]
    fn the_first_hit_curses_the_attacker_and_a_summon_breaks_the_chest() {
        let mut t = setup();
        let chest = t.spawn(CHEST, 300);
        t.ticks(2);
        let npc = ScriptNpc {
            object_id: chest,
            id: CHEST,
        };
        t.world.run_as_script("SilentValley", |ctx| {
            SilentValley.on_attack(ctx, npc, ObjectId(1), 5, false, None);
        });
        assert!(t.world.is_npc_script_value(chest, 1));
        assert!(!t.world.is_dead(chest));
        t.world.run_as_script("SilentValley", |ctx| {
            SilentValley.on_attack(ctx, npc, ObjectId(1), 5, true, None);
        });
        assert!(t.world.is_dead(chest));
        assert!(t.world.nearest_ground_item(ObjectId(1), SACK, 2000).is_some());
    }

    #[test]
    fn a_kill_now_and_then_calls_four_guards() {
        let mut t = setup();
        let mob = t.spawn(20965, 300);
        for _ in 0..3000 {
            t.world.run_as_script("SilentValley", |ctx| {
                let npc = ctx.script_npc(mob).unwrap();
                SilentValley.on_kill(ctx, npc, ObjectId(1), false);
            });
            if !t.npcs_of(GUARD1).is_empty() {
                assert_eq!(t.npcs_of(GUARD1).len() + t.npcs_of(GUARD2).len(), 4);
                return;
            }
        }
        panic!("no guards in 3000 kills");
    }

    #[test]
    fn the_events_clear_the_guards() {
        let mut t = setup();
        let guard = t.spawn(GUARD1, 300);
        let npc = ScriptNpc {
            object_id: guard,
            id: GUARD1,
        };
        let sender = ScriptNpc {
            object_id: t.spawn(CHEST, 310),
            id: CHEST,
        };
        t.world.run_as_script("SilentValley", |ctx| {
            SilentValley.on_event_received(ctx, "CLEAR_ALL", sender, npc, None);
            assert!(ctx.has_quest_timer("CLEAR", Some(npc), None));
        });
        t.wait_ms(60_500);
        assert!(t.world.is_dead(guard));
    }
}
