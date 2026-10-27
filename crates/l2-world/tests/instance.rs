#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Instances (M8-INST-01): the instance id on every object (who sees whom, the doors of an
//! instance), entering with the script API and the re-entry times, the duration and empty-destroy
//! countdowns, the eject of a dead player, the timer of the client, the enter and leave of a
//! character, the destroy, and the party duel arena.
//!
//! The expected behaviour is read off the Java sources, not off this code:
//! `managers/InstanceManager.java`, `entity/instancezone/{Instance,InstanceWorld}.java`,
//! `entity/WorldObject.java (setInstanceId)`, `mechanics/script/InstanceScript.java`,
//! `entity/actor/holders/player/Duel.java (teleportPlayers)`, `handlers/.../user/InstanceZone.java`.

mod party_scene;

use std::sync::Arc;

use l2_data::InstanceTemplates;
use l2_geo::{DynamicObstacles, Location};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    DeleteObject, ExSendUiEvent, SocialAction, SystemMessageParam, TeleportToLocation,
};
use l2_world::teleport::TeleportTarget;
use l2_world::*;
use party_scene::*;

/// The wall clock of the instance timers at world tick 0 (2026-10-28 10:15:20 UTC).
const BASE: i64 = 1_793_182_520_000;
/// The plain test template: one monster of the "general" group next to the scene's center, the
/// arena door, an enter point and an exit point.
const PLAIN: i32 = 500;
const ZONE_NAME: &str = "Test Zone";
const ENTER: (i32, i32, i32) = (X + 50, Y + 50, Z);
const EXIT: (i32, i32, i32) = (X - 3_000, Y, Z);

fn template(id: i32, extra: &str) -> String {
    format!(
        r#"<instance id="{id}" ejectTime="60">
        {extra}
        <doorlist><door id="{ARENA_DOOR}" /></doorlist>
        <spawnlist><group name="general"><npc id="9001" x="{}" y="{Y}" z="{Z}" heading="100" /></group>
        <group name="later"><npc id="9001" x="{}" y="{Y}" z="{Z}" respawn="30" /></group></spawnlist>
        <spawnPoints startPosType="FIXED"><Location x="{}" y="{}" z="{}" /></spawnPoints>
        <exitPoint x="{}" y="{}" z="{}" />
        </instance>"#,
        X + 100,
        X + 120,
        ENTER.0,
        ENTER.1,
        ENTER.2,
        EXIT.0,
        EXIT.1,
        EXIT.2
    )
}

/// The scene with the arena (door, zones, the Olympiad templates) and the test templates.
fn scene_with(templates: &[(i32, &str)]) -> Scene {
    let mut s = Scene::new();
    s.world.set_regen_settings(RegenSettings {
        hp_regen_multiplier: 0.0,
        mp_regen_multiplier: 0.0,
        cp_regen_multiplier: 0.0,
        ..RegenSettings::default()
    });
    install_arena(&mut s);
    let mut table = InstanceTemplates::default();
    table
        .add_names(&format!(
            r#"<list><instance id="1" name="Party Duel"/><instance id="{PLAIN}" name="{ZONE_NAME}"/></list>"#
        ))
        .unwrap();
    for (id, extra) in templates {
        table.add_template(&template(*id, extra), &format!("test/{id}.xml"));
    }
    // The arena templates of `install_arena` stay available.
    for id in 147..=150 {
        table.add_template(
            &format!(
                r#"<instance id="{id}"><PvPInstance val="true" /><doorlist><door id="{ARENA_DOOR}" /></doorlist>
                <spawnlist><group name="general">
                <npc id="{OLYMPIAD_HOST}" x="{}" y="{Y}" z="{Z}" heading="0" respawn="60" />
                <npc id="{OLYMPIAD_HOST}" x="{}" y="{Y}" z="{Z}" heading="0" respawn="60" />
                </group></spawnlist></instance>"#,
                X - 1_300,
                X + 1_300
            ),
            &format!("Olympiad/arena{id}.xml"),
        );
    }
    s.world.set_instance_data(Arc::new(table));
    s.world.set_instance_settings(InstanceSettings {
        clock_override_unix_ms: Some(BASE),
        ..InstanceSettings::default()
    });
    s
}

fn scene() -> Scene {
    scene_with(&[(PLAIN, "")])
}

/// The instance clock now.
fn now(s: &Scene) -> i64 {
    BASE + i64::try_from(s.world.tick_count()).unwrap() * 50
}

/// Teleports a player into `instance` at `at` and lets the client load the place.
fn move_into(s: &mut Scene, who: i32, instance: i32, (x, y, z): (i32, i32, i32)) {
    s.world
        .tele_to_location(id(who), TeleportTarget::at(x, y, z).with_instance(instance));
    s.input(who, ClientInput::Appearing);
}

/// The NPCs of an instance, in id order.
fn npcs_of(s: &Scene, instance: i32) -> Vec<ObjectId> {
    s.world.instance(instance).unwrap().npcs.keys().copied().collect()
}

fn instance_writes(s: &Scene) -> Vec<InstanceTimeWrite> {
    s.persisted
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            PersistCommand::InstanceTime(w) => Some(*w),
            _ => None,
        })
        .collect()
}

fn decode_ex<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[3..])).unwrap()
}

/// The `CreatureSay` lines with a sender name one client got: (name, text). The packet has no
/// decoder; the layout is the one of Java's `CreatureSay` (sender id, chat type, name, message
/// id, text).
fn says(sent: &Sent, to: i32) -> Vec<(String, String)> {
    of(sent, to, "SAY2")
        .iter()
        .map(|raw| {
            let mut r = PacketReader::new(&raw[1..]);
            let _sender = r.i32().unwrap();
            let _chat_type = r.i32().unwrap();
            let name = r.string();
            let _message_id = r.i32().unwrap();
            (name, r.string())
        })
        .collect()
}

// ------------------------------------------------------------------ the instance id on objects

/// `World.forEachVisibleObject` only finds objects of the same instance id: two copies of one
/// template at the same place do not see each other, nor does the normal world see them.
#[test]
fn two_instances_of_one_template_do_not_see_each_other() {
    let mut s = scene();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    s.ticks(2);
    assert!(s.world.visible_objects(id(P1)).contains(&id(P2)));

    let a = s.world.create_dynamic_instance(PLAIN);
    let b = s.world.create_dynamic_instance(PLAIN);
    assert_eq!((a, b), (FIRST_DYNAMIC_INSTANCE_ID, FIRST_DYNAMIC_INSTANCE_ID + 1));
    let (monster_a, monster_b) = (npcs_of(&s, a)[0], npcs_of(&s, b)[0]);
    assert_eq!(s.world.instance_id_of(monster_a), a);
    assert_eq!(s.world.instance_id_of(monster_b), b);
    // Same template, same place, two objects.
    assert_ne!(monster_a, monster_b);
    assert_eq!(s.world.position_of(monster_a), s.world.position_of(monster_b));
    s.log.take();

    move_into(&mut s, P1, a, (X, Y, Z));
    let sent = s.log.take();
    // Bravo and Charlie lose Alpha when he leaves the normal world.
    for who in [P2, P3] {
        assert!(
            of(&sent, who, "DELETE_OBJECT")
                .iter()
                .any(|raw| decode::<DeleteObject>(raw).object_id == P1),
            "{who:#x}"
        );
    }
    move_into(&mut s, P2, b, (X, Y, Z));
    s.log.take();

    let seen = |s: &Scene, who: i32| s.world.visible_objects(id(who));
    assert!(seen(&s, P1).contains(&monster_a));
    assert!(!seen(&s, P1).contains(&monster_b));
    assert!(!seen(&s, P1).contains(&id(P2)));
    assert!(!seen(&s, P1).contains(&id(P3)));
    assert!(seen(&s, P2).contains(&monster_b));
    assert!(!seen(&s, P2).contains(&monster_a));
    assert!(!seen(&s, P2).contains(&id(P1)));
    assert!(
        !seen(&s, P3)
            .iter()
            .any(|o| [monster_a, monster_b, id(P1), id(P2)].contains(o))
    );
    // The instance lists.
    assert!(s.world.instance(a).unwrap().contains_player(id(P1)));
    assert!(s.world.instance(b).unwrap().contains_player(id(P2)));
    assert_eq!(s.world.instance_of_player(id(P2)), b);

    // A third player who comes into the same instance sees Alpha and the monster of `a` only.
    move_into(&mut s, P3, a, (X, Y, Z));
    let sent = s.log.take();
    assert!(!of(&sent, P3, "NPC_INFO").is_empty());
    assert!(!of(&sent, P3, "CHAR_INFO").is_empty());
    assert!(seen(&s, P3).contains(&id(P1)) && seen(&s, P3).contains(&monster_a));
    assert!(!seen(&s, P3).contains(&monster_b) && !seen(&s, P3).contains(&id(P2)));
    // The broadcasts follow: what Alpha does reaches Charlie, not Bravo.
    s.world.broadcast_packet(
        id(P1),
        &SocialAction {
            object_id: P1,
            action_id: 2,
        },
    );
    let sent = s.log.take();
    assert_eq!(of(&sent, P3, "SOCIAL_ACTION").len(), 1);
    assert!(of(&sent, P2, "SOCIAL_ACTION").is_empty());
}

/// The doors of an instance are copies that only block inside it (`DoorData.checkIfDoorsBetween`
/// with an instance id reads `Instance.getDoors()`), and open on their own.
#[test]
fn the_doors_of_an_instance_are_its_own() {
    let mut s = scene();
    let a = s.world.create_dynamic_instance(PLAIN);
    let b = s.world.create_dynamic_instance(PLAIN);
    let world_door = s.world.door_by_door_id(ARENA_DOOR).unwrap().object_id;
    let door_a = s.world.instance_door(a, ARENA_DOOR).unwrap();
    let door_b = s.world.instance_door(b, ARENA_DOOR).unwrap();
    assert!(door_a != world_door && door_a != door_b);
    assert_eq!(s.world.instance_id_of(door_a), a);
    // Across the door (it stands at X + 200).
    let (from, to) = (Location::new(X + 100, Y, Z), Location::new(X + 300, Y, Z));
    let blocked = |s: &Scene, instance| s.world.obstacles().doors_between(from, to, instance, false);
    assert!(blocked(&s, 0) && blocked(&s, a) && blocked(&s, b));
    assert!(!blocked(&s, 777), "no such instance: no doors");

    s.world.instance_open_door(a, ARENA_DOOR);
    assert!(s.world.door(door_a).unwrap().open);
    assert!(!s.world.door(door_b).unwrap().open && !s.world.door(world_door).unwrap().open);
    assert!(!blocked(&s, a) && blocked(&s, b) && blocked(&s, 0));
    // The snapshot the path workers get says the same.
    let snapshot = s.world.door_snapshot();
    assert!(!snapshot.doors_between(from, to, a, false));
    assert!(snapshot.doors_between(from, to, b, false));
    assert!(snapshot.doors_between(from, to, 0, false));
    s.world.instance_close_door(a, ARENA_DOOR);
    assert!(blocked(&s, a));
}

// ------------------------------------------------------------------ enter, re-entry, destroy

/// `InstanceScript.enterInstance`: a new world for a player who has none, the re-entry time of
/// an `ON_ENTER` template, the enter point; the same world for a player it allows; `/instancezone`;
/// then the destroy puts him out at the exit point.
#[test]
fn enter_reentry_time_and_destroy() {
    let mut s = scene_with(&[(
        PLAIN,
        r#"<reenter additionStyle="ON_ENTER"><reset time="3600000" /></reenter>"#,
    )]);
    s.enter(P1, "Alpha");
    s.ticks(2);
    s.log.take();

    let mut firsts = Vec::new();
    let outcome = s.world.enter_instance(
        id(P1),
        PLAIN,
        |_, _| true,
        |world, player, instance, first| {
            assert!(first);
            world.instance_world_mut(instance).unwrap().add_allowed(player);
        },
    );
    let EnterOutcome::New(instance) = outcome else {
        panic!("{outcome:?}");
    };
    firsts.push(instance);
    let sent = s.log.take();
    // The re-entry time is written for the allowed player, and he is told.
    let reenter = now(&s) + 3_600_000;
    assert_eq!(
        instance_writes(&s),
        [InstanceTimeWrite::Set {
            char_id: P1,
            instance_id: PLAIN,
            time: reenter
        }]
    );
    assert_eq!(s.world.instance_time(id(P1), PLAIN), reenter);
    assert_eq!(
        messages(&sent, P1)
            .into_iter()
            .map(|m| (m.id, m.params))
            .collect::<Vec<_>>(),
        [(
            SystemMessageId::InstantZoneS1SEntryHasBeenRestrictedYouCanCheckTheNextPossibleEntryTimeByUsingTheCommandInstancezone,
            vec![SystemMessageParam::Text(ZONE_NAME.into())]
        )]
    );
    // Teleported to the enter point, into the instance.
    let teleport = decode::<TeleportToLocation>(of(&sent, P1, "TELEPORT_TO_LOCATION")[0]);
    assert_eq!((teleport.x, teleport.y), (ENTER.0, ENTER.1));
    assert_eq!(s.world.instance_id_of(id(P1)), instance);
    s.input(P1, ClientInput::Appearing);
    s.log.take();

    // `/instancezone`: the instance he is in, and the time left (59 minutes, the clock went on).
    s.ticks(20);
    s.input(P1, ClientInput::UserCommand { id: 114 });
    let sent = s.log.take();
    let remaining = (reenter - now(&s)) / 1000;
    let (hours, minutes) = ((remaining / 3600) as i32, ((remaining % 3600) / 60) as i32);
    assert_eq!((hours, minutes), (0, 59));
    assert_eq!(
        messages(&sent, P1)
            .into_iter()
            .map(|m| (m.id, m.params))
            .collect::<Vec<_>>(),
        [
            (
                SystemMessageId::InstantZoneCurrentlyInUseS1,
                vec![SystemMessageParam::InstanceName(PLAIN)]
            ),
            (SystemMessageId::InstanceZoneTimeLimit, vec![]),
            (
                SystemMessageId::S1WillBeAvailableForReUseAfterS2HourSS3MinuteS,
                vec![
                    SystemMessageParam::InstanceName(PLAIN),
                    SystemMessageParam::IntNumber(hours),
                    SystemMessageParam::IntNumber(minutes)
                ]
            )
        ]
    );

    // Entering again finds the same world (no second re-entry write).
    let outcome = s.world.enter_instance(
        id(P1),
        PLAIN,
        |_, _| panic!("no conditions for a player who has a world"),
        |_, _, _, first| assert!(!first),
    );
    assert_eq!(outcome, EnterOutcome::Existing(instance));
    assert_eq!(instance_writes(&s).len(), 1);
    // Another template while his world has a player: refused.
    let outcome = s.world.enter_instance(id(P1), 147, |_, _| true, |_, _, _, _| {});
    assert_eq!(outcome, EnterOutcome::OtherInstanceBusy);
    assert_eq!(
        message_ids(&s.log.take(), P1),
        [SystemMessageId::YouHaveEnteredAnotherInstanceZoneThereforeYouCannotEnterCorrespondingDungeon]
    );

    // The destroy: NPCs and doors go, the player is put out at the exit point, instance 0.
    let monster = npcs_of(&s, instance)[0];
    let door = s.world.instance_door(instance, ARENA_DOOR).unwrap();
    s.world.destroy_instance(instance);
    assert!(s.world.instance(instance).is_none());
    assert!(s.world.instance_world(instance).is_none());
    assert!(s.world.npc(monster).is_none());
    assert!(s.world.door(door).is_none());
    assert_eq!(s.world.instance_id_of(id(P1)), 0);
    let sent = s.log.take();
    let out = decode::<TeleportToLocation>(of(&sent, P1, "TELEPORT_TO_LOCATION")[0]);
    assert!(
        (out.x - EXIT.0).abs() <= 50 && (out.y - EXIT.1).abs() <= 50,
        "{out:?}"
    );
    // 0 and below are never destroyed.
    s.world.destroy_instance(0);
    assert!(s.world.instance(0).is_some() && s.world.instance(-1).is_some());
}

/// `restoreInstanceTimes`: the rows the session read come with the character; the ones that are
/// over are deleted.
#[test]
fn the_reentry_times_of_a_character_come_with_him() {
    let mut s = scene();
    let mut data = player_data(P1, "Alpha", 10, (X, Y, Z));
    data.profile.instance_times = vec![(10, BASE - 1), (PLAIN, BASE + 7_200_000)];
    s.enter_data(data);
    assert_eq!(
        instance_writes(&s),
        [InstanceTimeWrite::Delete {
            char_id: P1,
            instance_id: 10
        }]
    );
    assert_eq!(s.world.instance_time(id(P1), 10), -1);
    assert_eq!(s.world.instance_time(id(P1), PLAIN), BASE + 7_200_000);
    // A restriction of a minute or less is deleted by `/instancezone`, the others are shown.
    s.world.set_instance_time(id(P1), 11, BASE + 30_000);
    s.input(P1, ClientInput::UserCommand { id: 114 });
    let ids = message_ids(&s.log.take(), P1);
    assert_eq!(
        ids,
        [
            SystemMessageId::InstanceZoneTimeLimit,
            SystemMessageId::S1WillBeAvailableForReUseAfterS2HourSS3MinuteS
        ]
    );
    assert_eq!(s.world.instance_time(id(P1), 11), -1);
    assert_eq!(
        instance_writes(&s)[1..],
        [
            InstanceTimeWrite::Set {
                char_id: P1,
                instance_id: 11,
                time: BASE + 30_000
            },
            InstanceTimeWrite::Delete {
                char_id: P1,
                instance_id: 11
            }
        ]
    );
    // Nothing left under a time limit... but the two hours of the plain one.
    s.world.delete_instance_time(id(P1), PLAIN);
    s.input(P1, ClientInput::UserCommand { id: 114 });
    assert_eq!(
        message_ids(&s.log.take(), P1),
        [SystemMessageId::ThereIsNoInstanceZoneUnderATimeLimit]
    );
}

/// `finishInstance` of an `ON_FINISH` template writes the re-entry time and starts the end.
#[test]
fn finishing_an_on_finish_instance() {
    let mut s = scene_with(&[(
        PLAIN,
        r#"<reenter additionStyle="ON_FINISH"><reset time="600000" /></reenter>"#,
    )]);
    s.enter(P1, "Alpha");
    let outcome = s.world.enter_instance(
        id(P1),
        PLAIN,
        |_, _| true,
        |world, player, instance, _| world.instance_world_mut(instance).unwrap().add_allowed(player),
    );
    let EnterOutcome::New(instance) = outcome else {
        panic!("{outcome:?}");
    };
    assert!(instance_writes(&s).is_empty(), "not on enter");
    s.world.finish_instance_in(instance, 30_000);
    assert_eq!(
        instance_writes(&s),
        [InstanceTimeWrite::Set {
            char_id: P1,
            instance_id: PLAIN,
            time: now(&s) + 600_000
        }]
    );
    assert_eq!(s.world.instance(instance).unwrap().empty_destroy_time_ms, 0);
    // 500 ms, then 30 s of notices, then the end.
    s.ticks(10 + 600 + 1);
    assert!(s.world.instance(instance).is_none());
    assert_eq!(s.world.instance_id_of(id(P1)), 0);
}

// ------------------------------------------------------------------ the countdowns

/// `doCheckTimeUp` with a player inside: "expires in N minutes" every five minutes, then every
/// minute, then "N seconds left." notices every 30 and 10 seconds, then `TimeUp`.
#[test]
fn the_duration_counts_down_with_messages() {
    let mut s = scene_with(&[(PLAIN, r#"<activityTime val="6" />"#)]);
    s.enter(P1, "Alpha");
    let instance = s.world.create_dynamic_instance(PLAIN);
    assert_eq!(
        s.world.instance(instance).unwrap().end_time_ms,
        now(&s) + 6 * 60_000 + 15_000
    );
    move_into(&mut s, P1, instance, (X, Y, Z));
    s.log.take();

    // 15 s: 6 minutes left.
    s.ticks(300);
    let expire =
        SystemMessageId::ThisDungeonWillExpireInS1MinuteSYouWillBeForcedOutOfTheDungeonWhenTheTimeExpires;
    assert_eq!(
        messages(&s.log.take(), P1)
            .into_iter()
            .map(|m| (m.id, m.params))
            .collect::<Vec<_>>(),
        [(expire, vec![SystemMessageParam::Text("6".into())])]
    );
    // 5 minutes later 60 000 ms are left: not more than 60 000, so "60 seconds left."
    s.ticks(6_000);
    assert_eq!(
        says(&s.log.take(), P1),
        [("Notice".to_owned(), "60 seconds left.".to_owned())]
    );
    s.ticks(600);
    assert_eq!(
        says(&s.log.take(), P1),
        [("Notice".to_owned(), "30 seconds left.".to_owned())]
    );
    s.ticks(200);
    assert_eq!(
        says(&s.log.take(), P1),
        [("Notice".to_owned(), "20 seconds left.".to_owned())]
    );
    s.ticks(200);
    assert_eq!(
        says(&s.log.take(), P1),
        [("Notice".to_owned(), "10 seconds left.".to_owned())]
    );
    assert!(s.world.instance(instance).is_some());
    s.ticks(200);
    assert!(s.world.instance(instance).is_none());
    assert_eq!(s.world.instance_id_of(id(P1)), 0);
}

/// `removePlayer`: when the last player leaves, an instance with an empty-destroy time goes once
/// that time is over (`doCheckTimeUp`'s empty branch: one 10 s step, then the end).
#[test]
fn an_empty_instance_goes_after_its_empty_destroy_time() {
    let mut s = scene_with(&[(PLAIN, r#"<activityTime val="60" /><emptyDestroyTime val="10" />"#)]);
    s.enter(P1, "Alpha");
    let instance = s.world.create_dynamic_instance(PLAIN);
    move_into(&mut s, P1, instance, (X, Y, Z));
    s.ticks(20);
    // Out of the instance: the last one out starts the countdown.
    move_into(&mut s, P1, 0, (X, Y, Z));
    assert!(s.world.instance(instance).unwrap().players.is_empty());
    assert_eq!(s.world.instance(instance).unwrap().last_left_ms, now(&s));
    s.ticks(200);
    assert!(s.world.instance(instance).is_some(), "10 s are not over");
    s.ticks(15);
    assert!(s.world.instance(instance).is_none());
}

/// `notifyDeath`: a player who dies inside is told and put out after the eject time, unless he is
/// revived before (`cancelEjectDeadPlayer`).
#[test]
fn a_dead_player_is_put_out_after_the_eject_time() {
    let mut s = scene();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    let instance = s.world.create_dynamic_instance(PLAIN);
    move_into(&mut s, P1, instance, (X, Y, Z));
    move_into(&mut s, P2, instance, (X, Y, Z));
    s.log.take();

    s.world.reduce_current_hp(id(P1), 1_000_000.0, None);
    assert!(s.world.is_dead(id(P1)));
    let sent = s.log.take();
    assert!(messages(&sent, P1).iter().any(|m| m.id
        == SystemMessageId::IfYouAreNotResurrectedWithinS1MinutesYouWillBeExpelledFromTheInstantZone
        && m.params == [SystemMessageParam::IntNumber(1)]));
    // Bravo dies and is revived: he stays.
    s.world.reduce_current_hp(id(P2), 1_000_000.0, None);
    s.world.do_revive(id(P2));
    s.ticks(1_199);
    assert_eq!(s.world.instance_id_of(id(P1)), instance);
    s.ticks(1);
    assert_eq!(s.world.instance_id_of(id(P1)), 0);
    let out = decode::<TeleportToLocation>(of(&s.log.take(), P1, "TELEPORT_TO_LOCATION")[0]);
    assert!((out.x - EXIT.0).abs() <= 50, "{out:?}");
    assert_eq!(s.world.instance_id_of(id(P2)), instance);
}

/// `sendInstanceUpdate`: an instance with `<showTimer>` shows its timer to a player who comes in
/// and hides it from one who leaves.
#[test]
fn the_timer_of_the_client() {
    let mut s = scene_with(&[(
        PLAIN,
        r#"<activityTime val="10" /><showTimer val="true" increase="true" text="Time left" />"#,
    )]);
    s.enter(P1, "Alpha");
    let instance = s.world.create_dynamic_instance(PLAIN);
    s.ticks(40);
    s.log.take();
    s.world.set_instance_id(id(P1), instance);
    let sent = s.log.take();
    let shown: ExSendUiEvent = decode_ex(of(&sent, P1, "EX_SEND_UI_EVENT")[0]);
    assert_eq!(
        shown,
        ExSendUiEvent {
            object_id: P1,
            hide: false,
            count_up: true,
            start_time: 2,
            end_time: 10 * 60 + 15,
            npc_string_id: -1,
            params: vec!["Time left".into()],
        }
    );
    s.world.set_instance_id(id(P1), 0);
    let hidden: ExSendUiEvent = decode_ex(of(&s.log.take(), P1, "EX_SEND_UI_EVENT")[0]);
    assert!(hidden.hide);
}

/// `Creature.isInsideZone` and `isInsidePeaceZone`: a PvP instance is a PvP zone everywhere in
/// it and never a peace zone.
#[test]
fn a_pvp_instance_is_a_pvp_zone() {
    let mut s = scene_with(&[(PLAIN, r#"<PvPInstance val="true" />"#)]);
    s.enter(P1, "Alpha");
    assert!(!s.world.is_inside_zone(id(P1), ZoneId::Pvp));
    let instance = s.world.create_dynamic_instance(PLAIN);
    move_into(&mut s, P1, instance, (X, Y, Z));
    assert!(s.world.is_inside_zone(id(P1), ZoneId::Pvp));
    assert!(!s.world.is_inside_zone(id(P1), ZoneId::Peace));
}

// ------------------------------------------------------------------ enter world and leave

/// `Logout` without `RestorePlayerInstance`: the player leaves the instance and the next login
/// starts at the exit point (`RESTORE_LOCATION`); with it, `EnterWorld` puts him back in.
#[test]
fn logging_out_of_an_instance() {
    let mut s = scene();
    s.enter(P1, "Alpha");
    let instance = s.world.create_dynamic_instance(PLAIN);
    move_into(&mut s, P1, instance, (X, Y, Z));
    s.input(P1, ClientInput::Logout);
    assert!(s.world.player(id(P1)).is_none());
    assert!(s.world.instance(instance).unwrap().players.is_empty());
    let stored = s
        .persisted
        .0
        .lock()
        .unwrap()
        .iter()
        .rev()
        .find_map(|c| match c {
            PersistCommand::StorePlayer(p) => Some(p.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        stored
            .profile
            .variables
            .get("RESTORE_LOCATION")
            .map(String::as_str),
        Some(format!("{};{};{}", EXIT.0, EXIT.1, EXIT.2).as_str())
    );

    // With `RestorePlayerInstance` the list keeps the player and the login puts him back.
    s.world.set_instance_settings(InstanceSettings {
        restore_player_instance: true,
        clock_override_unix_ms: Some(BASE),
        ..InstanceSettings::default()
    });
    s.enter(P2, "Bravo");
    move_into(&mut s, P2, instance, (X, Y, Z));
    s.input(P2, ClientInput::Logout);
    assert!(s.world.instance(instance).unwrap().contains_player(id(P2)));
    s.enter(P2, "Bravo");
    assert_eq!(s.world.instance_id_of(id(P2)), instance);
}

// ------------------------------------------------------------------ the party duel arena

/// `Duel.teleportPlayers` / `startDuel` / `ScheduleDuelTask` / `teleportBack`: a party duel gets a
/// new arena instance (the Olympiad hosts go), its doors open when the fight starts, the
/// instance goes when the fight is over, and the duelists are put back where they stood.
#[test]
fn a_party_duel_in_its_arena() {
    let mut s = scene();
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    s.enter(P4, "Delta");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    s.form_party(P3, &[(P4, "Delta")], 0);
    let before: Vec<(i32, i32, i32)> = [P1, P2, P3, P4]
        .iter()
        .map(|p| s.world.position_of(id(*p)).unwrap())
        .collect();
    s.input(
        P1,
        ClientInput::DuelStart {
            name: "Charlie".into(),
            party_duel: 1,
        },
    );
    s.input(
        P3,
        ClientInput::DuelAnswerStart {
            party_duel: 1,
            response: 1,
        },
    );
    // 3 s: the arena.
    s.ticks(60);
    let arena = s.world.instance_id_of(id(P1));
    assert!(arena >= FIRST_DYNAMIC_INSTANCE_ID);
    let template = s.world.instance(arena).unwrap().template_id;
    assert!((147..=150).contains(&template), "{template}");
    assert!(s.world.instance(arena).unwrap().pvp);
    // The two Olympiad hosts are out of view.
    for host in npcs_of(&s, arena) {
        assert!(!s.world.object(host).unwrap().spawned);
    }
    for who in [P1, P2, P3, P4] {
        assert_eq!(s.world.instance_id_of(id(who)), arena);
        let (x, y, z) = s.world.position_of(id(who)).unwrap();
        let spawn = ARENA_SPAWNS.iter().position(|p| *p == (x, y, z - 5));
        let spawn = spawn.unwrap_or_else(|| panic!("{who:#x} at {x} {y} {z}"));
        assert_eq!(spawn < 3, who == P1 || who == P2, "{who:#x}");
        s.input(who, ClientInput::Appearing);
    }
    let door = s.world.instance_door(arena, ARENA_DOOR).unwrap();
    assert!(!s.world.door(door).unwrap().open);
    // 20 s and 3, 2, 1: the fight starts, the arena door opens.
    s.ticks(400 + 60);
    assert!(s.world.is_in_duel(id(P1)));
    assert!(s.world.door(door).unwrap().open);

    // Charlie's party is beaten: the arena goes at once, everybody is in the normal world.
    s.world.reduce_current_hp(id(P3), 1_000_000.0, Some(id(P1)));
    s.world.reduce_current_hp(id(P4), 1_000_000.0, Some(id(P2)));
    s.ticks(21);
    assert!(s.world.instance(arena).is_none());
    for who in [P1, P2, P3, P4] {
        assert_eq!(s.world.instance_id_of(id(who)), 0);
    }
    // Five seconds later the duel ends and each duelist goes back to his place.
    s.log.take();
    s.ticks(101);
    let sent = s.log.take();
    assert!(!s.world.is_in_duel(id(P1)));
    for (who, (x, y, _)) in [P1, P2, P3, P4].into_iter().zip(before) {
        let back = of(&sent, who, "TELEPORT_TO_LOCATION")
            .iter()
            .map(|raw| decode::<TeleportToLocation>(raw))
            .find(|t| t.object_id == who)
            .unwrap_or_else(|| panic!("{who:#x} not sent back"));
        assert_eq!((back.x, back.y), (x, y), "{who:#x}");
    }
}
