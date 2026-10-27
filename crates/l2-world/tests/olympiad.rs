#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The Grand Olympiad (M8-OLY-01): a classed and a non-classed match from the registration to the
//! points, a spectator, the period end with `CycleDatabaseWrites` on, and the same with it off.
//!
//! The expected behaviour is read off the Java sources, not off this code:
//! `mechanics/olympiad/{Olympiad,OlympiadManager,OlympiadGameManager,OlympiadGameTask,
//! AbstractOlympiadGame,OlympiadGameNormal}.java`, `entity/zone/type/OlympiadStadiumZone.java`,
//! `entity/actor/Player.java (enterOlympiadObserverMode, leaveOlympiadObserverMode)`,
//! `handlers/bypass/npc/{OlympiadManagerLink,OlympiadObservation}.java`,
//! `handlers/chat/commands/user/OlympiadStat.java`.

mod party_scene;

use std::sync::Arc;

use l2_data::{NpcLoadOptions, NpcTemplates, SpawnData};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExOlympiadMatchList, ExOlympiadMatchResult, ExOlympiadMode, ExOlympiadUserInfo,
};
use l2_world::*;
use party_scene::*;

/// 2026-10-28 18:30:00 UTC: half an hour into the day's competition (18:00, six hours).
const BASE: i64 = 1_793_212_200_000;
const MINUTE: i64 = 60_000;
const DAY: i64 = 24 * 60 * MINUTE;
/// The manager of the towns (`OlympiadAnnouncer.OLY_MANAGER`).
const OLYMPIAD_MANAGER: i32 = 31688;
/// The base class of the classed matches (Duelist).
const DUELIST: i32 = 88;

fn npc_xml(id: i32, name: &str) -> String {
    format!(
        r#"<npc id="{id}" level="70" type="OlympiadManager" name="{name}">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="2444" hpRegen="7.5" mp="1345" mpRegen="2.7" />
      <attack physical="688" magical="470" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="295" magical="216" />
      <speed><walk ground="50" /><run ground="120" /></speed>
    </stats>
    <collision><radius normal="8"/><height normal="23"/></collision>
  </npc>"#
    )
}

/// The scene of `party_scene::install_arena` with a stadium Java's game manager knows by name
/// ("Grassy Arena", template 147) and the town manager's template.
fn stadium_scene() -> Scene {
    let mut s = Scene::new();
    s.world.set_regen_settings(RegenSettings {
        hp_regen_multiplier: 0.0,
        mp_regen_multiplier: 0.0,
        cp_regen_multiplier: 0.0,
        ..RegenSettings::default()
    });
    install_arena(&mut s);
    // The town manager next to the host template.
    let npcs = Arc::new(
        NpcTemplates::from_xml(
            &format!(
                "<list>{}{}{}</list>",
                monster_xml(10, 1000, 100),
                npc_xml(OLYMPIAD_HOST, "Olympiad Host"),
                npc_xml(OLYMPIAD_MANAGER, "Grand Olympiad Manager")
            ),
            &NpcLoadOptions::default(),
        )
        .unwrap(),
    );
    s.world.set_npc_data(npcs, Arc::new(SpawnData::default()));
    // The arena zone again, named like a real one.
    let spawns: String = ARENA_SPAWNS
        .iter()
        .map(|(x, y, z)| format!(r#"<spawn X="{x}" Y="{y}" Z="{z}" />"#))
        .collect();
    let zone = format!(
        r#"<zone name="Grassy Arena" type="OlympiadStadiumZone" shape="Cylinder" minZ="{}" maxZ="{}" rad="2000">
        <node X="{X}" Y="{Y}" />{spawns}<spawn X="{X}" Y="{Y}" Z="{Z}" type="spectatorSpawn" /></zone>"#,
        Z - 300,
        Z + 4_000
    );
    let dir = tempfile::tempdir().unwrap();
    for sub in ["zones", "mapregion"] {
        std::fs::create_dir_all(dir.path().join("data").join(sub)).unwrap();
    }
    std::fs::write(
        dir.path().join("data/zones/arena.xml"),
        format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">{zone}</list>\n"),
    )
    .unwrap();
    s.world.set_zone_data(
        Arc::new(l2_data::Zones::load(dir.path())),
        Arc::new(l2_data::MapRegions::load(dir.path())),
    );
    s
}

/// The settings of the tests: the clock at [`BASE`] in UTC, two players start a match, a short
/// wait before the teleport, Adena as the reward (the scene knows that item).
fn settings(cycle_database_writes: bool) -> OlympiadSettings {
    OlympiadSettings {
        classed: 2,
        non_classed: 2,
        wait_time: 3,
        classed_reward: vec![[57, 50]],
        non_classed_reward: vec![[57, 40]],
        cycle_database_writes,
        clock_override_unix_ms: Some(BASE),
        zone_offset_override_ms: Some(0),
        ..OlympiadSettings::default()
    }
}

/// A world in the competition period of cycle 1 (a period end far away).
fn olympiad_scene(cycle_database_writes: bool) -> Scene {
    let mut s = stadium_scene();
    s.world.set_olympiad_settings(settings(cycle_database_writes));
    s.world.load_olympiad(OlympiadLoad {
        data: Some(OlympiadData {
            current_cycle: 1,
            period: 0,
            olympiad_end: BASE + 10 * DAY,
            validation_end: 0,
            next_weekly_change: BASE + 5 * DAY,
        }),
        ..OlympiadLoad::default()
    });
    s
}

fn noble(s: &mut Scene, who: i32, name: &str, base_class: i32) {
    let mut data = player_data(who, name, 40, (X - 3_000, Y + (who & 0xF) * 100, Z));
    data.class_id = 0;
    data.base_class = base_class;
    data.profile.noble = true;
    s.enter_data(data);
}

/// Selects the NPC, then talks to it (the second click).
fn talk(s: &mut Scene, who: i32, npc: ObjectId) {
    for _ in 0..2 {
        s.input(
            who,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
    }
}

fn secs(s: &mut Scene, n: u32) {
    s.ticks(n * 20);
}

fn decode_ex<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[3..])).unwrap()
}

fn olympiad_writes(s: &Scene) -> Vec<OlympiadWrite> {
    s.persisted
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            PersistCommand::Olympiad(w) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn state(s: &Scene) -> GameState {
    s.world.olympiad().stadiums()[0].task.state
}

/// Ticks until the stadium's task is in `wanted` (at most `max_secs`), sending the client's
/// `Appearing` after every teleport so the zones see the players arrive.
fn run_until(s: &mut Scene, wanted: GameState, max_secs: u32, players: &[i32]) {
    for _ in 0..max_secs * 20 {
        if state(s) == wanted {
            return;
        }
        s.world.tick();
        for p in players {
            if s.world.is_teleporting(id(*p)) {
                s.input(*p, ClientInput::Appearing);
            }
        }
    }
    panic!("the task did not reach {wanted:?}, it is in {:?}", state(s));
}

#[test]
fn the_olympiad_loads_its_stadiums_and_starts_the_competition() {
    let mut s = olympiad_scene(false);
    // `init()`: today's competition runs (18:00 + 6 h), the start task fires at once.
    assert_eq!(s.world.olympiad().comp_start(), BASE - 30 * MINUTE);
    assert_eq!(
        s.world.olympiad().comp_end(),
        BASE - 30 * MINUTE + 6 * 60 * MINUTE
    );
    assert!(!s.world.olympiad().in_comp_period());
    noble(&mut s, P1, "Alpha", DUELIST);
    s.log.take();
    s.ticks(2);
    assert!(s.world.olympiad().in_comp_period());
    assert!(message_ids(&s.log.take(), P1).contains(
        &SystemMessageId::SharpenYourSwordsTightenTheStitchingInYourArmorAndMakeHasteToAGrandOlympiadManagerBattlesInTheGrandOlympiadGamesAreNowTakingPlace
    ));
    // One stadium in a copy of the Grassy Arena (template 147).
    let stadium = &s.world.olympiad().stadiums()[0];
    assert_eq!(stadium.name, "Grassy Arena");
    assert!(stadium.instance_id >= 300_000);
    assert_eq!(s.world.instance(stadium.instance_id).unwrap().template_id, 147);
    // A non-noble is refused, a noble gets the waiting list.
    s.enter(P3, "Gamma");
    assert!(
        !s.world
            .olympiad_register_noble(id(P3), CompetitionType::NonClassed)
    );
    assert_eq!(
        message_ids(&s.log.take(), P3),
        [SystemMessageId::C1DoesNotMeetTheParticipationRequirementsOnlyNoblesseCharactersCanParticipateInTheOlympiad]
    );
    assert!(
        s.world
            .olympiad_register_noble(id(P1), CompetitionType::NonClassed)
    );
    assert!(!s.world.olympiad_register_noble(id(P1), CompetitionType::Classed));
    assert_eq!(
        message_ids(&s.log.take(), P1),
        [
            SystemMessageId::YouAreCurrentlyRegisteredForA1v1ClassIrrelevantMatch,
            SystemMessageId::C1IsAlreadyRegisteredOnTheWaitingListForTheClassIrrelevantIndividualMatch,
        ]
    );
    // The new noble starts with `OlympiadStartPoints`, written with INSERT later.
    let noble_stats = s.world.olympiad().noble(P1).unwrap();
    assert_eq!(
        (noble_stats.points, noble_stats.class_id, noble_stats.to_save),
        (10, DUELIST, true)
    );
    // Unregister, then the user command.
    assert!(s.world.olympiad_unregister_noble(id(P1)));
    s.input(P1, ClientInput::UserCommand { id: 109 });
    let sent = s.log.take();
    let messages = messages(&sent, P1);
    assert_eq!(
        messages[0].id,
        SystemMessageId::YouHaveBeenRemovedFromTheGrandOlympiadWaitingList
    );
    assert_eq!(
        messages[1].id,
        SystemMessageId::ForTheCurrentGrandOlympiadYouHaveParticipatedInS1MatchEsS2WinSAndS3DefeatSYouCurrentlyHaveS4OlympiadPointS
    );
    assert_eq!(
        messages[2].id,
        SystemMessageId::YouHaveS1MatchEsRemainingThatYouCanParticipateInThisWeekS21Vs1ClassMatchesS31Vs1MatchesS43Vs3TeamMatches
    );
}

#[test]
fn a_classed_match_runs_from_the_registration_to_the_points() {
    let mut s = olympiad_scene(false);
    noble(&mut s, P1, "Alpha", DUELIST);
    noble(&mut s, P2, "Beta", DUELIST);
    let home1 = s.world.position_of(id(P1)).unwrap();
    s.ticks(2);
    s.log.take();
    assert!(s.world.olympiad_register_noble(id(P1), CompetitionType::Classed));
    assert!(s.world.olympiad_register_noble(id(P2), CompetitionType::Classed));
    assert_eq!(
        message_ids(&s.log.take(), P1),
        [SystemMessageId::YouHaveBeenRegisteredForTheGrandOlympiadWaitingListForAClassSpecificMatch]
    );
    assert_eq!(s.world.olympiad().registered_classed()[&DUELIST], [P1, P2]);

    // The game manager runs 30 s after the start: the two leave the list for stadium 0.
    secs(&mut s, 31);
    assert!(s.world.olympiad().registered_classed()[&DUELIST].is_empty());
    let game = s.world.olympiad().stadiums()[0].task.game.clone().unwrap();
    assert_eq!(game.kind, CompetitionType::Classed);
    // `createListOfParticipants` draws both at random (the world RNG): side 1 and side 2.
    let (one, two) = (game.team_one[0].object_id, game.team_two[0].object_id);
    assert_eq!([one.min(two), one.max(two)], [P1, P2]);
    let name_of = |who: i32| if who == P1 { "Alpha" } else { "Beta" };
    assert_eq!(s.world.olympiad_game_id(id(P1)), 0);
    assert!(s.world.is_registered_in_comp(id(P1)));

    // The countdown, then the teleport into the stadium's instance.
    run_until(&mut s, GameState::BattleCountdownFirst, 10, &[P1, P2]);
    let sent = s.log.take();
    let moved: Vec<i32> = messages(&sent, P1)
        .iter()
        .filter(|m| m.id == SystemMessageId::YouWillBeMovedToTheOlympiadStadiumInS1SecondS)
        .map(|m| match &m.params[0] {
            l2_protocol_game::server::SystemMessageParam::IntNumber(n) => *n,
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(moved, [3, 2, 1]);
    let instance = s.world.olympiad().stadiums()[0].instance_id;
    for (who, side) in [(one, 1), (two, 2)] {
        assert_eq!(s.world.instance_id_of(id(who)), instance);
        let st = s.world.player_olympiad(id(who)).unwrap();
        assert!(st.in_olympiad_mode && !st.olympiad_start);
        assert_eq!((st.side, st.buff_count, st.game_id), (side, 5, 0));
        let mode: Vec<ExOlympiadMode> = of(&sent, who, "EX_OLYMPIAD_MODE")
            .iter()
            .map(|raw| decode_ex::<ExOlympiadMode>(raw))
            .collect();
        assert_eq!(mode, [ExOlympiadMode { mode: 2 }]);
        // In the stadium zone: a combat zone.
        assert!(s.world.is_inside_zone(id(who), ZoneId::Pvp));
    }
    assert_eq!(s.world.position_of(id(one)).unwrap().0, ARENA_SPAWNS[0].0);
    assert_eq!(s.world.position_of(id(two)).unwrap().0, ARENA_SPAWNS[3].0);
    assert_eq!(s.world.player_olympiad(id(P1)).unwrap().last_loc, home1);

    // The battle starts after the 60 + 10 s countdown; the fighters see each other's info.
    run_until(&mut s, GameState::BattleInProgress, 80, &[P1, P2]);
    let sent = s.log.take();
    let ids = message_ids(&sent, P1);
    assert!(ids.contains(&SystemMessageId::TheMatchWillStartInS1SecondS));
    assert!(ids.contains(&SystemMessageId::TheMatchHasStartedFight));
    assert!(s.world.is_olympiad_start(id(P1)));
    assert!(!s.world.is_invul(id(P1)));
    assert_eq!(of(&sent, P1, "EX_OLYMPIAD_USER_INFO").len(), 2);

    // Side 2 falls: dead without dying, the task sees the winner.
    s.world.reduce_current_hp(id(two), 1_000_000.0, Some(id(one)));
    assert!(s.world.is_dead(id(two)));
    run_until(&mut s, GameState::TeleportToTown, 3, &[P1, P2]);
    let sent = s.log.take();
    let ids = message_ids(&sent, one);
    assert!(ids.contains(&SystemMessageId::CongratulationsC1YouWinTheMatch));
    assert!(ids.contains(&SystemMessageId::C1HasEarnedS2PointsInTheGrandOlympiadGames));
    assert!(ids.contains(&SystemMessageId::C1HasLostS2PointsInTheGrandOlympiadGames));
    assert!(ids.contains(&SystemMessageId::YouHaveEarnedS2S1S));
    let result: ExOlympiadMatchResult = decode_ex(of(&sent, one, "EX_RECEIVE_OLYMPIAD")[0]);
    assert!(!result.tie);
    assert_eq!((result.win_team, result.lose_team), (1, 2));
    assert_eq!(result.winner_name, name_of(one));
    // `pointDiff = min(10, 10) / OlympiadDividerClassed (5)`.
    assert_eq!(
        (result.winners[0].current_points, result.winners[0].diff_points),
        (12, 2)
    );
    assert_eq!(
        (result.losers[0].current_points, result.losers[0].diff_points),
        (8, -2)
    );
    let alpha = s.world.olympiad().noble(one).unwrap().clone();
    let beta = s.world.olympiad().noble(two).unwrap().clone();
    assert_eq!(
        (
            alpha.points,
            alpha.comp_done,
            alpha.comp_won,
            alpha.comp_done_week,
            alpha.comp_done_week_classed
        ),
        (12, 1, 1, 1, 1)
    );
    assert_eq!((beta.points, beta.comp_done, beta.comp_lost), (8, 1, 1));
    assert_eq!(s.inventory_count(one, 57), 50);
    assert_eq!(s.inventory_count(two, 57), 0);
    // `cleanEffects`: the loser is alive again and both are healed.
    assert!(!s.world.is_dead(id(two)));
    // The match row (written whatever `CycleDatabaseWrites` says); no global write.
    let writes = olympiad_writes(&s);
    assert_eq!(writes.len(), 1);
    match &writes[0] {
        OlympiadWrite::Fight(f) => {
            assert_eq!(
                (f.char_one_id, f.char_two_id, f.winner, f.classed),
                (one, two, 1, true)
            );
            assert_eq!((f.char_one_class, f.char_two_class), (DUELIST, DUELIST));
        }
        other => panic!("{other:?}"),
    }

    // 40 s later both are back where they stood, out of the olympiad mode.
    run_until(&mut s, GameState::Idle, 45, &[P1, P2]);
    let sent = s.log.take();
    for who in [P1, P2] {
        assert_eq!(s.world.instance_id_of(id(who)), 0);
        assert!(!s.world.is_in_olympiad_mode(id(who)));
        assert_eq!(s.world.olympiad_game_id(id(who)), -1);
        let mode: Vec<ExOlympiadMode> = of(&sent, who, "EX_OLYMPIAD_MODE")
            .iter()
            .map(|raw| decode_ex::<ExOlympiadMode>(raw))
            .collect();
        assert_eq!(mode, [ExOlympiadMode { mode: 0 }]);
    }
    let (x, y, _) = s.world.position_of(id(P1)).unwrap();
    assert_eq!((x, y), (home1.0, home1.1));
    assert!(s.world.olympiad().stadiums()[0].task.game.is_none());
}

#[test]
fn a_non_classed_match_that_times_out_is_a_tie() {
    let mut s = stadium_scene();
    let mut set = settings(true);
    set.battle_ms = 5_000;
    s.world.set_olympiad_settings(set);
    s.world.load_olympiad(OlympiadLoad {
        data: Some(OlympiadData {
            current_cycle: 1,
            period: 0,
            olympiad_end: BASE + 10 * DAY,
            validation_end: 0,
            next_weekly_change: BASE + 5 * DAY,
        }),
        ..OlympiadLoad::default()
    });
    // Different base classes: only the class-free list can match them.
    noble(&mut s, P1, "Alpha", DUELIST);
    noble(&mut s, P2, "Beta", 89);
    s.ticks(2);
    assert!(
        s.world
            .olympiad_register_noble(id(P1), CompetitionType::NonClassed)
    );
    assert!(
        s.world
            .olympiad_register_noble(id(P2), CompetitionType::NonClassed)
    );
    secs(&mut s, 31);
    assert_eq!(
        s.world.olympiad().stadiums()[0].task.game.as_ref().unwrap().kind,
        CompetitionType::NonClassed
    );
    run_until(&mut s, GameState::BattleInProgress, 100, &[P1, P2]);
    s.log.take();
    // Nobody wins within `OlympiadBattle`: both lose min(points / 5, max) = 2.
    run_until(&mut s, GameState::TeleportToTown, 10, &[P1, P2]);
    let sent = s.log.take();
    assert!(message_ids(&sent, P1).contains(&SystemMessageId::ThereIsNoVictorTheMatchEndsInATie));
    let result: ExOlympiadMatchResult = decode_ex(of(&sent, P2, "EX_RECEIVE_OLYMPIAD")[0]);
    assert!(result.tie);
    for who in [P1, P2] {
        let n = s.world.olympiad().noble(who).unwrap();
        assert_eq!(
            (n.points, n.comp_done, n.comp_done_week_non_classed, n.comp_drawn),
            (8, 1, 1, 0)
        );
    }
    let writes = olympiad_writes(&s);
    assert!(matches!(
        writes.as_slice(),
        [OlympiadWrite::Fight(OlympiadFight {
            winner: 0,
            classed: false,
            ..
        })]
    ));
    assert_eq!(s.inventory_count(P1, 57), 0);
}

#[test]
fn a_spectator_watches_a_match_and_leaves() {
    let mut s = olympiad_scene(false);
    noble(&mut s, P1, "Alpha", DUELIST);
    noble(&mut s, P2, "Beta", DUELIST);
    s.enter_at(P3, "Gamma", 20, (X - 3_000, Y - 400, Z));
    let home3 = s.world.position_of(id(P3)).unwrap();
    let manager = s
        .world
        .spawn_npc_at(OLYMPIAD_MANAGER, X - 3_050, Y - 400, Z, 0)
        .unwrap();
    s.ticks(2);
    s.world.olympiad_register_noble(id(P1), CompetitionType::Classed);
    s.world.olympiad_register_noble(id(P2), CompetitionType::Classed);
    secs(&mut s, 31);
    run_until(&mut s, GameState::BattleInProgress, 100, &[P1, P2]);

    // Gamma talks to the manager, then picks arena 1 in the observer window.
    s.input(
        P3,
        ClientInput::Action {
            object_id: manager.0,
            action_id: 0,
        },
    );
    s.input(
        P3,
        ClientInput::Action {
            object_id: manager.0,
            action_id: 0,
        },
    );
    s.input(P3, ClientInput::Olympiad(OlympiadInput::MatchListRefresh));
    let sent = s.log.take();
    let list: ExOlympiadMatchList = decode_ex(of(&sent, P3, "EX_RECEIVE_OLYMPIAD")[0]);
    assert_eq!(list.games.len(), 1);
    assert_eq!(
        (
            list.games[0].stadium_id,
            list.games[0].game_kind,
            list.games[0].state
        ),
        (0, 2, 2)
    );
    let mut names = [list.games[0].player_1.clone(), list.games[0].player_2.clone()];
    names.sort();
    assert_eq!(names, ["Alpha", "Beta"]);

    s.input(
        P3,
        ClientInput::Bypass {
            command: "_olympiad?command=move_op_field&field=1".to_owned(),
        },
    );
    assert!(s.world.in_observer_mode(id(P3)));
    assert_eq!(s.world.olympiad_game_id(id(P3)), 0);
    assert!(s.world.invul_flag(id(P3)));
    let instance = s.world.olympiad().stadiums()[0].instance_id;
    assert_eq!(s.world.instance_id_of(id(P3)), instance);
    s.input(P3, ClientInput::Appearing);
    let sent = s.log.take();
    let mode: Vec<ExOlympiadMode> = of(&sent, P3, "EX_OLYMPIAD_MODE")
        .iter()
        .map(|raw| decode_ex::<ExOlympiadMode>(raw))
        .collect();
    assert_eq!(mode, [ExOlympiadMode { mode: 3 }]);
    // An observer inside the running battle gets the fighters' info.
    assert_eq!(of(&sent, P3, "EX_OLYMPIAD_USER_INFO").len(), 2);
    // Not kicked: the KickPlayer task would move him to a town.
    s.ticks(2);
    assert_eq!(s.world.instance_id_of(id(P3)), instance);

    // A fighter is hit: the observer sees the new HP and CP.
    s.world.reduce_current_hp(id(P2), 50.0, Some(id(P1)));
    s.ticks(2);
    let sent = s.log.take();
    let infos: Vec<ExOlympiadUserInfo> = of(&sent, P3, "EX_OLYMPIAD_USER_INFO")
        .iter()
        .map(|raw| decode_ex::<ExOlympiadUserInfo>(raw))
        .collect();
    let side = s.world.olympiad_side(id(P2));
    assert!(side == 1 || side == 2);
    assert_eq!(
        infos.last().map(|i| (i.object_id, i.side)),
        Some((P2, u8::try_from(side).unwrap()))
    );
    // The fighter does not get his own info, the other side does.
    assert_eq!(of(&sent, P1, "EX_OLYMPIAD_USER_INFO").len(), 1);
    assert!(of(&sent, P2, "EX_OLYMPIAD_USER_INFO").is_empty());

    // An observer cannot register.
    // `RequestOlympiadObserverEnd`: back where he stood.
    s.input(P3, ClientInput::Olympiad(OlympiadInput::ObserverEnd));
    assert!(!s.world.in_observer_mode(id(P3)));
    assert_eq!(s.world.instance_id_of(id(P3)), 0);
    assert!(!s.world.invul_flag(id(P3)));
    let sent = s.log.take();
    let mode: Vec<ExOlympiadMode> = of(&sent, P3, "EX_OLYMPIAD_MODE")
        .iter()
        .map(|raw| decode_ex::<ExOlympiadMode>(raw))
        .collect();
    assert_eq!(mode, [ExOlympiadMode { mode: 0 }]);
    let (x, y, _) = s.world.position_of(id(P3)).unwrap();
    // `teleToLocation(_lastLoc, true)`: the random offset of `MaxOffsetOnTeleport`.
    assert!((x - home3.0).abs() <= 50 && (y - home3.1).abs() <= 50, "{x},{y}");
}

/// Nobles of the load: (char id, class, points, matches, wins).
fn noble_rows(rows: &[(i32, i32, i32, i32, i32)]) -> Vec<NobleRow> {
    rows.iter()
        .map(|&(char_id, class_id, points, done, won)| NobleRow {
            char_id,
            stats: NobleStats {
                class_id,
                char_name: format!("N{char_id}"),
                points,
                comp_done: done,
                comp_won: won,
                comp_lost: done - won,
                ..NobleStats::default()
            },
        })
        .collect()
}

/// A world one minute before the end of cycle 3, with a short validation period.
fn period_end_scene(cycle_database_writes: bool) -> Scene {
    let mut s = stadium_scene();
    let mut set = settings(cycle_database_writes);
    set.validation_period_ms = 2 * MINUTE;
    s.world.set_olympiad_settings(set);
    s.world.load_olympiad(OlympiadLoad {
        data: Some(OlympiadData {
            current_cycle: 3,
            period: 0,
            olympiad_end: BASE + MINUTE,
            validation_end: 0,
            next_weekly_change: BASE + 5 * DAY,
        }),
        nobles: noble_rows(&[
            // Two Duelists with enough matches: the one with more points is the hero to be.
            (P1, DUELIST, 120, 20, 12),
            (0x2000_0010, DUELIST, 150, 18, 9),
            // A Dreadnought without a win, one with too few matches.
            (0x2000_0011, 89, 90, 30, 0),
            (0x2000_0012, 89, 200, 3, 3),
        ]),
        ..OlympiadLoad::default()
    });
    s
}

#[test]
fn the_period_end_with_cycle_database_writes_on() {
    let mut s = period_end_scene(true);
    let mut data = player_data(P1, "N1", 40, (X - 3_000, Y, Z));
    data.profile.noble = true;
    data.base_class = DUELIST;
    s.enter_data(data);
    s.log.take();
    secs(&mut s, 61);
    // `OlympiadEndTask`: the message, the period, the heroes to be.
    assert!(
        message_ids(&s.log.take(), P1).contains(&SystemMessageId::RoundS1OfTheGrandOlympiadGamesHasNowEnded)
    );
    let data = s.world.olympiad().data();
    assert_eq!((data.period, data.current_cycle), (1, 3));
    assert_eq!(data.validation_end, BASE + MINUTE + 2 * MINUTE);
    let heroes: Vec<(i32, i32)> = s
        .world
        .olympiad()
        .heroes_to_be()
        .iter()
        .map(|h| (h.class_id, h.char_id))
        .collect();
    assert_eq!(heroes, [(DUELIST, 0x2000_0010)]);
    // The writes in Java's order: `saveNobleData`, `saveOlympiadStatus` (the nobles again, the
    // period row), `updateMonthlyData`, then the passes of the offline nobles.
    let writes = olympiad_writes(&s);
    assert!(matches!(writes[0], OlympiadWrite::SaveNobles(ref n) if n.len() == 4));
    assert!(matches!(writes[1], OlympiadWrite::SaveNobles(_)));
    assert!(matches!(
        writes[2],
        OlympiadWrite::SaveData(OlympiadData {
            period: 1,
            current_cycle: 3,
            ..
        })
    ));
    assert_eq!(writes[3], OlympiadWrite::UpdateMonthlyData);
    // The ranking of the three classified nobles (15 matches or more), best first: 0x10 (150),
    // Alpha (120), 0x11 (90). `round(3 * 0.01)` is 0, so rank 1 is the first and every other
    // limit moves up one: rank 2 to 1, rank 3 to 2, rank 4 to 3.
    // Passes: rank points + 10 with a win (5 without): 0x10 100 + 10, Alpha 55 + 10, 0x11 40 + 5.
    let passes: Vec<(i32, i32)> = writes[4..]
        .iter()
        .filter_map(|w| match w {
            OlympiadWrite::UnclaimedPoints { char_id, points } => Some((*char_id, *points)),
            _ => None,
        })
        .collect();
    assert_eq!(passes, [(0x2000_0010, 110), (0x2000_0011, 45)]);
    // The online noble gets the variable instead of a row.
    let online = s
        .world
        .player(id(P1))
        .unwrap()
        .data
        .profile
        .variables
        .get("UNCLAIMED_OLYMPIAD_POINTS")
        .cloned();
    assert_eq!(online.as_deref(), Some("65"));
    // "This is a one time calculation": the points are gone.
    assert_eq!(s.world.olympiad().noble(P1).unwrap().points, 0);

    // The validation period ends: cycle 4, the nobles go (the table too).
    secs(&mut s, 121);
    let data = s.world.olympiad().data();
    assert_eq!((data.period, data.current_cycle), (0, 4));
    assert!(s.world.olympiad().nobles().is_empty());
    let writes = olympiad_writes(&s);
    assert!(writes.contains(&OlympiadWrite::DeleteNobles));
    // The next period ends on the first of the next month at noon (2026-11-01 12:00 UTC).
    assert_eq!(data.olympiad_end, 1_793_534_400_000);
    let sent = s.log.take();
    assert!(message_ids(&sent, P1).contains(&SystemMessageId::RoundS1OfTheGrandOlympiadGamesHasStarted));
}

#[test]
fn the_period_end_with_cycle_database_writes_off_stays_in_memory() {
    let mut s = period_end_scene(false);
    let mut data = player_data(P1, "N1", 40, (X - 3_000, Y, Z));
    data.profile.noble = true;
    data.base_class = DUELIST;
    s.enter_data(data);
    secs(&mut s, 61);
    // The same period change in memory ...
    let data = s.world.olympiad().data();
    assert_eq!((data.period, data.current_cycle), (1, 3));
    assert_eq!(s.world.olympiad().heroes_to_be().len(), 1);
    // ... but no global row: the Java game server runs the cycle (D-53), and it hands the passes
    // out.
    assert!(olympiad_writes(&s).is_empty());
    assert!(
        !s.world
            .player(id(P1))
            .unwrap()
            .data
            .profile
            .variables
            .contains_key("UNCLAIMED_OLYMPIAD_POINTS")
    );
    assert_eq!(s.world.olympiad().noble(P1).unwrap().points, 120);
    secs(&mut s, 121);
    let data = s.world.olympiad().data();
    assert_eq!((data.period, data.current_cycle), (0, 4));
    assert!(s.world.olympiad().nobles().is_empty());
    assert!(olympiad_writes(&s).is_empty());
    // The shutdown saves nothing either.
    s.world.save_olympiad_status();
    assert!(olympiad_writes(&s).is_empty());
}

#[test]
fn the_shutdown_saves_the_status_with_the_switch_on() {
    let mut s = olympiad_scene(true);
    noble(&mut s, P1, "Alpha", DUELIST);
    s.ticks(2);
    s.world.olympiad_register_noble(id(P1), CompetitionType::Classed);
    s.world.save_olympiad_status();
    let writes = olympiad_writes(&s);
    match writes.as_slice() {
        [OlympiadWrite::SaveNobles(nobles), OlympiadWrite::SaveData(data)] => {
            // A new noble is inserted (`to_save`), then known.
            assert_eq!(nobles.len(), 1);
            assert!(nobles[0].1.to_save);
            assert_eq!((data.current_cycle, data.period), (1, 0));
        }
        other => panic!("{other:?}"),
    }
    assert!(!s.world.olympiad().noble(P1).unwrap().to_save);
}

#[test]
fn the_class_ranking_and_the_passes_are_read_again() {
    let mut s = olympiad_scene(false);
    let mut data = player_data(P1, "Alpha", 80, (X - 3_000, Y, Z));
    data.profile.noble = true;
    data.class_id = DUELIST;
    data.base_class = DUELIST;
    s.enter_data(data);
    // The manager's page with the two links the test clicks.
    let root = tempfile::tempdir().unwrap();
    let dir = root.path().join("data/html/olympiad");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("noble_main.htm"),
        r#"<html><body><a action="bypass -h npc_%objectId%_olympiad 2_88">Rank</a>
<a action="bypass -h npc_%objectId%_OlympiadNoble 10">Passes</a></body></html>"#,
    )
    .unwrap();
    std::fs::write(
        dir.join("olympiad_ranking.htm"),
        "<html><body>%place1% %rank1% %place2% %rank2% %place3% %rank3%</body></html>",
    )
    .unwrap();
    s.world
        .set_html_cache(Arc::new(l2_data::HtmCache::load(root.path(), false)));
    let manager = s
        .world
        .spawn_npc_at(OLYMPIAD_MANAGER, X - 3_050, Y, Z, 0)
        .unwrap();
    s.ticks(2);
    // The ranking page of a class: Java reads the table, so the world asks for it.
    talk(&mut s, P1, manager);
    s.persisted.0.lock().unwrap().clear();
    s.input(
        P1,
        ClientInput::Bypass {
            command: format!("npc_{}_olympiad 2_88", manager.0),
        },
    );
    let reads: Vec<PersistCommand> = s
        .persisted
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|c| matches!(c, PersistCommand::OlympiadRereadRequest { .. }))
        .cloned()
        .collect();
    let [PersistCommand::OlympiadRereadRequest { request, then }] = reads.as_slice() else {
        panic!("{reads:?}");
    };
    assert_eq!(
        *request,
        OlympiadRereadRequest::ClassLeaderBoard {
            class_id: DUELIST,
            monthly: true,
            min_matches: 15
        }
    );
    s.log.take();
    s.world.handle(WorldCommand::OlympiadReread {
        result: Ok(OlympiadRereadRows::Names(vec!["Top".into(), "Second".into()])),
        then: *then,
    });
    let sent = s.log.take();
    let pages: Vec<l2_protocol_game::server::NpcHtmlMessage> = of(&sent, P1, "NPC_HTML_MESSAGE")
        .iter()
        .map(|raw| decode::<l2_protocol_game::server::NpcHtmlMessage>(raw))
        .collect();
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].html, "<html><body>1 Top 2 Second  </body></html>");
    // Back to the manager's page for the next link.
    talk(&mut s, P1, manager);

    // The passes (switch off): read again, handed out once, the row deleted at once (D-50).
    s.persisted.0.lock().unwrap().clear();
    s.input(
        P1,
        ClientInput::Bypass {
            command: format!("npc_{}_OlympiadNoble 10", manager.0),
        },
    );
    let then = s
        .persisted
        .0
        .lock()
        .unwrap()
        .iter()
        .find_map(|c| match c {
            PersistCommand::OlympiadRereadRequest {
                request: OlympiadRereadRequest::UnclaimedPasses { char_id },
                then,
            } if *char_id == P1 => Some(*then),
            _ => None,
        })
        .unwrap();
    s.world.handle(WorldCommand::OlympiadReread {
        result: Ok(OlympiadRereadRows::Passes(Some("3".into()))),
        then,
    });
    // 3 passes * `OlympiadGPPerPoint` (1000) of `OlympiadCompRewItem`: the scene does not know
    // the token, so only the deletion of the row is seen.
    let removed = s.persisted.0.lock().unwrap().iter().any(|c| {
        matches!(c, PersistCommand::PlayerVariables { owner, removed, .. }
            if *owner == id(P1) && removed == &vec!["UNCLAIMED_OLYMPIAD_POINTS".to_owned()])
    });
    assert!(removed);
    // A failed read hands nothing out.
    s.persisted.0.lock().unwrap().clear();
    s.world.handle(WorldCommand::OlympiadReread {
        result: Err(()),
        then,
    });
    assert!(s.persisted.0.lock().unwrap().is_empty());
}
