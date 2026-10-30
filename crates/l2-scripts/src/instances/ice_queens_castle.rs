//! Ice Queen's Castle instance zone, template 137: Freya's forced defeat for
//! `Q10285_MeetingSirra`.
// Java: dist/game/data/scripts/instances/IceQueensCastle/IceQueensCastle.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::effects::SkillKey;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TeleportTarget};

const Q10285_MEETING_SIRRA: &str = "Q10285_MeetingSirra";

// NPCs
const FREYA: i32 = 18847;
const BATTALION_LEADER: i32 = 18848;
const LEGIONNAIRE: i32 = 18849;
const MERCENARY_ARCHER: i32 = 18926;
const ARCHERY_KNIGHT: i32 = 22767;
const JINIA: i32 = 32781;

// Locations
const START_LOC: (i32, i32, i32) = (114000, -112357, -11200);
const EXIT_LOC: (i32, i32, i32) = (113883, -108777, -848);
const FREYA_LOC: (i32, i32, i32) = (114730, -114805, -11200);

// Skill
const ETHERNAL_BLIZZARD: SkillKey = SkillKey { id: 6276, level: 1 };

// Misc
const TEMPLATE_ID: i32 = 137;
const ICE_QUEEN_DOOR: i32 = 23140101;
const MIN_LV: i32 = 82;
/// `Movie.SC_BOSS_FREYA_FORCED_DEFEAT`.
const SC_BOSS_FREYA_FORCED_DEFEAT: i32 = 21;

/// Java `instances.IceQueensCastle.IceQueensCastle`.
pub struct IceQueensCastle;

impl Script for IceQueensCastle {
    fn name(&self) -> &'static str {
        "IceQueensCastle"
    }

    fn path(&self) -> &'static str {
        "instances/IceQueensCastle"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JINIA]);
        s.add_talk_id(&[JINIA]);
        s.add_spawn_id(&[FREYA]);
        s.add_spell_finished_id(&[FREYA]);
        s.add_creature_see_id(&[BATTALION_LEADER, LEGIONNAIRE, MERCENARY_ARCHER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "ATTACK_KNIGHT" => {
                let npc = npc?;
                for character in archery_knights_around(ctx, npc) {
                    attack(ctx, npc, character);
                }
                ctx.start_quest_timer("ATTACK_KNIGHT", 3000, Some(npc), None, false);
            }
            "TIMER_MOVING" => {
                if let Some(npc) = npc {
                    ctx.world_mut()
                        .npc_move_to_location(npc.object_id, FREYA_LOC.0, FREYA_LOC.1, FREYA_LOC.2);
                }
            }
            "TIMER_BLIZZARD" => {
                let npc = npc?;
                ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::ICanNoLongerStandBy, &[]);
                ctx.world_mut().stop_move(npc.object_id, None);
                ctx.cast_skill_at(npc, player, ETHERNAL_BLIZZARD);
            }
            "TIMER_SCENE_21" => {
                if let Some(npc) = npc {
                    // NOTE(java-quirk): `playMovie(null, ..)` would throw without a player.
                    let player = player?;
                    ctx.play_movie(player, SC_BOSS_FREYA_FORCED_DEFEAT);
                    ctx.start_quest_timer("TIMER_PC_LEAVE", 24000, None, Some(player), false);
                    ctx.delete_npc(npc);
                }
            }
            "TIMER_PC_LEAVE" => {
                let player = player?;
                let qs = ctx.quest_state_of(Q10285_MEETING_SIRRA, player, false);
                if let Some(qs) = qs {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 10, true);
                    // NOTE(java-quirk): `getPlayerWorld(player)` is dereferenced unchecked.
                    let instance_id = ctx.world().player_instance_world(player).map(|w| w.instance_id)?;
                    if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                        world.remove_allowed(player);
                    }
                    ctx.world_mut().set_instance_id(player, 0);
                    // `player.teleToLocation(EXIT_LOC, 0)`: instance 0, no random offset.
                    ctx.teleport_player(player, EXIT_LOC.0, EXIT_LOC.1, EXIT_LOC.2, 0, false);
                }
            }
            _ => {}
        }

        // `super.onEvent(event, npc, player)`.
        None
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if ctx.world().player(creature).is_some() && ctx.is_script_value(npc, 0) {
            for character in archery_knights_around(ctx, npc) {
                attack(ctx, npc, character);
                ctx.set_script_value(npc, 1);
                ctx.start_quest_timer("ATTACK_KNIGHT", 5000, Some(npc), None, false);
            }
            let name = ctx.player_name(creature);
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::S1MayTheProtectionOfTheGodsBeUponYou,
                &[name.as_str()],
            );
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.start_quest_timer("TIMER_MOVING", 60000, Some(npc), None, false);
        ctx.start_quest_timer("TIMER_BLIZZARD", 180000, Some(npc), None, false);
    }

    fn on_spell_finished(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        _player: Option<ObjectId>,
        skill: SkillKey,
    ) {
        let Some(world) = ctx.world().instance_world_of(npc.object_id) else {
            return;
        };
        // `world.getParameters().getObject("player", Player.class)`.
        let leader = world
            .parameters
            .get("player")
            .and_then(|v| v.parse::<i32>().ok())
            .map(ObjectId)
            .filter(|leader| ctx.world().player(*leader).is_some());
        if skill == ETHERNAL_BLIZZARD
            && let Some(leader) = leader
        {
            ctx.start_quest_timer("TIMER_SCENE_21", 1000, Some(npc), Some(leader), false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        enter_instance(ctx, talker);
        // `super.onTalk(npc, talker)`.
        None
    }
}

/// `World.forEachVisibleObject(npc, Creature.class, c -> c.getId() == ARCHERY_KNIGHT &&
/// !c.isDead() && !c.asAttackable().isDecayed())`.
fn archery_knights_around(ctx: &ScriptCtx<'_>, npc: ScriptNpc) -> Vec<ObjectId> {
    let world = ctx.world();
    world
        .visible_objects(npc.object_id)
        .into_iter()
        .filter(|c| {
            world
                .npc(*c)
                .is_some_and(|n| n.npc_id == ARCHERY_KNIGHT && !n.decayed)
                && !world.is_dead(*c)
        })
        .collect()
}

/// `npc.setRunning(); npc.getAI().setIntentionAttack(character);
/// npc.asAttackable().addDamageHate(character, 0, 999999)`.
fn attack(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, character: ObjectId) {
    ctx.world_mut().set_npc_running(npc.object_id);
    ctx.world_mut().set_intention_attack(npc.object_id, character);
    ctx.world_mut().add_damage_hate(npc.object_id, character, 0, 999_999);
}

/// Java `checkConditions(player)`.
fn check_conditions(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> bool {
    if ctx.player_level(player) < MIN_LV {
        ctx.send_system_message(player, SystemMessageId::C1SLevelDoesNotCorrespondToTheRequirementsForEntry);
        return false;
    }

    true
}

/// Java `enterInstance(player, TEMPLATE_ID)` with this script's `onEnterInstance`.
fn enter_instance(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    // `checkConditions` is asked only when the player has no world yet.
    let allowed = ctx.world().player_instance_world(player).is_some() || check_conditions(ctx, player);
    ctx.world_mut().enter_instance(
        player,
        TEMPLATE_ID,
        move |_, _| allowed,
        |world, player, instance_id, first_entrance| {
            if first_entrance {
                if let Some(instance_world) = world.instance_world_mut(instance_id) {
                    instance_world.add_allowed(player);
                    instance_world
                        .parameters
                        .insert("player".to_owned(), player.0.to_string());
                }
                world.instance_open_door(instance_id, ICE_QUEEN_DOOR);
            }
            world.tele_to_location(
                player,
                TeleportTarget::at(START_LOC.0, START_LOC.1, START_LOC.2).with_instance(instance_id),
            );
        },
    );
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::instances::testing::instance_data;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="18847" type="Monster" level="70" name="Freya"/>
        <npc id="18848" type="Monster" level="70" name="Battalion Leader"/>
        <npc id="22767" type="Monster" level="70" name="Archery Knight"/>
        <npc id="32781" type="Folk" level="70" name="Jinia"/>"#;

    /// Meeting Sirra: a script of that name.
    struct StandIn;
    impl Script for StandIn {
        fn name(&self) -> &'static str {
            Q10285_MEETING_SIRRA
        }
        fn path(&self) -> &'static str {
            "quests/Q10285_MeetingSirra"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(StandIn);
        registry.add(IceQueensCastle);
        let pages = Pages::of(&IceQueensCastle).file(
            "data/html/default/32781.htm",
            r#"<html><body><a action="bypass -h Script IceQueensCastle">go</a></body></html>"#,
        );
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.world.set_instance_data(instance_data(
            &[(137, "Ice Queen's Castle")],
            &[(137, r#"<exitPoint x="113883" y="-108777" z="-848" />"#)],
        ));
        let jinia = t.spawn(JINIA, 20);
        (t, jinia)
    }

    fn enter_the_castle(t: &mut ScriptTest, jinia: Oid) -> i32 {
        t.talk(1, jinia);
        t.bypass(1, "Script IceQueensCastle");
        t.ticks(60);
        t.world.instance_id_of(Oid(1))
    }

    #[test]
    fn jinia_sends_a_level_82_player_in_and_turns_a_lower_one_away() {
        let (mut t, jinia) = setup();
        t.enter(1, 81, vec![]);
        assert_eq!(enter_the_castle(&mut t, jinia), 0);

        t.enter(2, 82, vec![]);
        t.talk(2, jinia);
        t.bypass(2, "Script IceQueensCastle");
        t.ticks(60);
        let instance_id = t.world.instance_id_of(Oid(2));
        assert!(instance_id > 0);
        let world = t.world.instance_world(instance_id).unwrap();
        assert_eq!((world.template_id, world.allowed.clone()), (137, vec![Oid(2)]));
        // The leader the Blizzard scene looks for.
        assert_eq!(world.parameters.get("player").map(String::as_str), Some("2"));
        let (x, y, _) = t.world.position_of(Oid(2)).unwrap();
        assert_eq!((x, y), (START_LOC.0, START_LOC.1));
    }

    #[test]
    fn freya_spawns_with_two_timers_and_the_blizzard_ends_the_instance_visit() {
        let (mut t, jinia) = setup();
        t.enter(1, 82, vec![]);
        t.world.run_as_script(Q10285_MEETING_SIRRA, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
        });
        t.take_writes();
        let instance_id = enter_the_castle(&mut t, jinia);
        assert!(instance_id > 0);

        let freya = t.spawn(FREYA, 10);
        t.world
            .tele_to_location(freya, TeleportTarget::at(START_LOC.0, START_LOC.1, START_LOC.2).with_instance(instance_id));
        t.ticks(5);
        let npc = ScriptNpc {
            object_id: freya,
            id: FREYA,
        };
        t.world.run_as_script("IceQueensCastle", |ctx| IceQueensCastle.on_spawn(ctx, npc));
        assert_eq!(t.world.quest_timer_count(), 2);

        // The blizzard ends: the scene starts a second later, 24 seconds after that the player
        // leaves with memo state 3 and cond 10.
        t.world.run_as_script("IceQueensCastle", |ctx| {
            IceQueensCastle.on_spell_finished(ctx, npc, None, ETHERNAL_BLIZZARD);
            // Another skill does nothing.
            IceQueensCastle.on_spell_finished(ctx, npc, None, SkillKey { id: 1, level: 1 });
        });
        t.wait_ms(1_100);
        assert!(!t.in_world(freya));
        assert_eq!(t.world.instance_id_of(Oid(1)), instance_id);
        t.wait_ms(24_100);
        assert_eq!(t.world.instance_id_of(Oid(1)), 0);
        assert!(t.world.instance_world(instance_id).unwrap().allowed.is_empty());
        let (x, y, _) = t.world.position_of(Oid(1)).unwrap();
        assert_eq!((x, y), (EXIT_LOC.0, EXIT_LOC.1));
        let mut memo = (0, 0);
        t.world.run_as_script(Q10285_MEETING_SIRRA, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            memo = (qs.get_memo_state(ctx), qs.get_cond(ctx));
        });
        assert_eq!(memo, (3, 10));
    }

    #[test]
    fn a_battalion_leader_blesses_the_player_and_attacks_the_archery_knight() {
        let (mut t, _) = setup();
        t.enter(1, 82, vec![]);
        let leader = t.spawn(BATTALION_LEADER, 30);
        let knight = t.spawn(ARCHERY_KNIGHT, 40);
        let npc = ScriptNpc {
            object_id: leader,
            id: BATTALION_LEADER,
        };
        t.take();
        t.world.run_as_script("IceQueensCastle", |ctx| {
            IceQueensCastle.on_creature_see(ctx, npc, Oid(1));
        });
        // The script value 1 and the ATTACK_KNIGHT timer; the greeting went out.
        assert_eq!(t.world.npc_script_value(leader), 1);
        assert_eq!(t.world.quest_timer_count(), 1);
        assert!(t.world.npc(knight).is_some());
        // Seen again: the script value is 1 now, nothing happens.
        t.world.run_as_script("IceQueensCastle", |ctx| {
            IceQueensCastle.on_creature_see(ctx, npc, Oid(1));
        });
        assert_eq!(t.world.quest_timer_count(), 1);
        // The timer repeats itself every three seconds.
        t.wait_ms(5_100);
        assert_eq!(t.world.quest_timer_count(), 1);
    }
}
