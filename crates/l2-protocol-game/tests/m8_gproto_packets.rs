//! The M8 packets of M8-GPROTO-11 (Olympiad, hero, Seven Signs, boats and air ships, Cube game,
//! Underground Coliseum record, cursed weapons, boss records, movie) against byte layouts worked
//! out by hand from the Java `readImpl` / `writeImpl` code, plus round trips and the dispatcher.
//!
//! The expected bytes are built with an independent little-endian builder (not the crate's
//! `PacketWriter`). The opcode ids come from the Java `ClientPackets` / `ExClientPackets` /
//! `ServerPackets` tables through the generated enums.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    CannotMoveAnymoreInVehicle, EndScenePlayer, MoveToLocationAirShip, MoveToLocationInAirShip,
    ObserverReturn, RequestCursedWeaponList, RequestCursedWeaponLocation, RequestExCubeGameChangeTeam,
    RequestExCubeGameReadyAnswer, RequestExOlympiadMatchListRefresh, RequestGetBossRecord,
    RequestGetOffVehicle, RequestGetOnVehicle, RequestMoveToLocationInVehicle, RequestOlympiadMatchList,
    RequestOlympiadObserverEnd, RequestSeedPhase, RequestSsqStatus, RequestWriteHeroWords,
};
use l2_protocol_game::server::boss::{
    BossRecordEntry, CursedWeaponLocation, ExCursedWeaponList, ExCursedWeaponLocation, ExGetBossRecord,
};
use l2_protocol_game::server::minigame::{
    CubePlayer, ExCubeGameAddPlayer, ExCubeGameChangePoints, ExCubeGameChangeTeam,
    ExCubeGameChangeTimeToStart, ExCubeGameCloseUi, ExCubeGameEnd, ExCubeGameExtendedChangePoints,
    ExCubeGameRemovePlayer, ExCubeGameRequestReady, ExCubeGameTeamList, ExPvpMatchRecord, ExPvpMatchUserDie,
    ExSendUiEvent, ExStartScenePlayer, PvpMember,
};
use l2_protocol_game::server::olympiad::{
    ExHeroList, ExOlympiadMatchEnd, ExOlympiadMatchList, ExOlympiadMatchResult, ExOlympiadMode,
    ExOlympiadSpelledInfo, ExOlympiadUserInfo, HeroEntry, OlympiadMatchEntry, OlympiadResultEntry,
    OlympiadSpelledEffect,
};
use l2_protocol_game::server::ssq::{
    ExShowSeedMapInfo, FestivalHighScore, SevenSignsView, SsqCycle, SsqFestival, SsqPredicted, SsqScore,
    SsqSealOutcome, SsqSealRow, SsqStatus, SsqStatusBody,
};
use l2_protocol_game::server::vehicle::{
    AirShipTeleport, ExAirShipInfo, ExAirShipStopMove, ExAirShipTeleportList, ExGetOffAirShip,
    ExGetOnAirShip, ExMoveToLocationAirShip, ExMoveToLocationInAirShip, ExStopMoveAirShip,
    ExStopMoveInAirShip, ExValidateLocationInAirShip, GetOffVehicle, GetOnVehicle, MoveToLocationInVehicle,
    StopMoveInVehicle, ValidateLocationInVehicle, VehicleCheckLocation, VehicleDeparture, VehicleInfo,
    VehicleStarted,
};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

/// Java `writeInt`, little endian, for each value.
fn ints(values: &[i32]) -> Vec<u8> {
    values.iter().flat_map(|v| v.to_le_bytes()).collect()
}

/// Java `writeString`: UTF-16LE code units and a `0x0000` terminator.
fn text(value: &str) -> Vec<u8> {
    let mut out: Vec<u8> = value.encode_utf16().flat_map(|u| u.to_le_bytes()).collect();
    out.extend([0, 0]);
    out
}

/// Joins byte parts.
fn cat(parts: &[Vec<u8>]) -> Vec<u8> {
    parts.concat()
}

/// A plain client opcode (one byte).
fn plain_client(id: u8) -> Vec<u8> {
    vec![id]
}

/// An ex client packet: `0xD0` and the `u16` ex id.
fn ex_client(id: u16) -> Vec<u8> {
    let mut out = vec![0xD0];
    out.extend(id.to_le_bytes());
    out
}

/// An ex server packet: `0xFE` and the `u16` ex id.
fn ex_server(id: u16) -> Vec<u8> {
    let mut out = vec![0xFE];
    out.extend(id.to_le_bytes());
    out
}

/// Encodes, compares with `expected`, then decodes the bytes after the `head` opcode bytes and
/// expects the same packet back.
fn check<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P, expected: &[u8], head: usize) {
    let bytes = packet.to_bytes();
    assert_eq!(bytes, expected, "encoded bytes");
    let decoded = P::decode(&mut PacketReader::new(&bytes[head..])).unwrap();
    assert_eq!(&decoded, packet, "round trip");
}

// ----------------------------------------------------------------------------------------------
// Client packets
// ----------------------------------------------------------------------------------------------

#[test]
fn client_unit_packets_are_only_their_opcode() {
    // Java reads nothing for these: `RequestOlympiadMatchList` (0x2E), `RequestOlympiadObserverEnd`
    // (0x29), `RequestExOlympiadMatchListRefresh` (0x88), `RequestSeedPhase` (0x63),
    // `RequestCursedWeaponList` (0x2A), `RequestCursedWeaponLocation` (0x2B) are ex packets,
    // `ObserverReturn` (0xC1) a plain one.
    check(&RequestOlympiadMatchList, &ex_client(0x2E), 3);
    check(&RequestOlympiadObserverEnd, &ex_client(0x29), 3);
    check(&RequestExOlympiadMatchListRefresh, &ex_client(0x88), 3);
    check(&RequestSeedPhase, &ex_client(0x63), 3);
    check(&RequestCursedWeaponList, &ex_client(0x2A), 3);
    check(&RequestCursedWeaponLocation, &ex_client(0x2B), 3);
    check(&ObserverReturn, &plain_client(0xC1), 1);
}

#[test]
fn request_write_hero_words_is_a_string() {
    let packet = RequestWriteHeroWords {
        hero_words: "Glory".to_owned(),
    };
    check(&packet, &cat(&[ex_client(0x05), text("Glory")]), 3);
}

#[test]
fn request_ssq_status_reads_a_signed_byte() {
    // Java `readByte()` is signed: page 4 and -1 both come through.
    check(&RequestSsqStatus { page: 4 }, &[0xC8, 4], 1);
    check(&RequestSsqStatus { page: -1 }, &[0xC8, 0xFF], 1);
}

#[test]
fn request_get_boss_record_reads_the_boss_id() {
    check(
        &RequestGetBossRecord { boss_id: 0 },
        &cat(&[ex_client(0x40), ints(&[0])]),
        3,
    );
}

#[test]
fn cube_game_arena_is_shifted_by_one() {
    // Java adds 1 to the arena the client sends (-1..2); the field holds the Java value.
    let packet = RequestExCubeGameChangeTeam { arena: 0, team: 1 };
    check(&packet, &cat(&[ex_client(0x5A), ints(&[-1, 1])]), 3);
    let packet = RequestExCubeGameReadyAnswer { arena: 2, answer: 1 };
    check(&packet, &cat(&[ex_client(0x5C), ints(&[1, 1])]), 3);
}

#[test]
fn end_scene_player_reads_the_movie_id() {
    check(
        &EndScenePlayer { movie_id: 7 },
        &cat(&[ex_client(0x5B), ints(&[7])]),
        3,
    );
}

#[test]
fn vehicle_client_packets_read_java_order() {
    check(
        &RequestGetOnVehicle {
            boat_id: 0x1000_0001,
            x: 10,
            y: -20,
            z: 30,
        },
        &cat(&[plain_client(0x53), ints(&[0x1000_0001, 10, -20, 30])]),
        1,
    );
    check(
        &RequestGetOffVehicle {
            boat_id: 5,
            x: 1,
            y: 2,
            z: 3,
        },
        &cat(&[plain_client(0x54), ints(&[5, 1, 2, 3])]),
        1,
    );
    check(
        &RequestMoveToLocationInVehicle {
            boat_id: 9,
            target_x: 1,
            target_y: 2,
            target_z: 3,
            origin_x: 4,
            origin_y: 5,
            origin_z: 6,
        },
        &cat(&[plain_client(0x75), ints(&[9, 1, 2, 3, 4, 5, 6])]),
        1,
    );
    check(
        &CannotMoveAnymoreInVehicle {
            boat_id: 9,
            x: 1,
            y: 2,
            z: 3,
            heading: 4,
        },
        &cat(&[plain_client(0x76), ints(&[9, 1, 2, 3, 4])]),
        1,
    );
}

#[test]
fn move_to_location_air_ship_param2_is_optional() {
    let packet = MoveToLocationAirShip {
        command: 1,
        param1: 2,
        param2: 3,
    };
    check(&packet, &cat(&[ex_client(0x38), ints(&[1, 2, 3])]), 3);
    // Without the last int Java keeps `_param2 = 0`.
    let short = cat(&[ex_client(0x38), ints(&[1, 2])]);
    let decoded = MoveToLocationAirShip::decode(&mut PacketReader::new(&short[3..])).unwrap();
    assert_eq!(
        decoded,
        MoveToLocationAirShip {
            command: 1,
            param1: 2,
            param2: 0
        }
    );
}

#[test]
fn move_to_location_in_air_ship_reads_ship_then_positions() {
    let packet = MoveToLocationInAirShip {
        ship_id: 77,
        target_x: 1,
        target_y: 2,
        target_z: 3,
        origin_x: 4,
        origin_y: 5,
        origin_z: 6,
    };
    check(
        &packet,
        &cat(&[ex_client(0x20), ints(&[77, 1, 2, 3, 4, 5, 6])]),
        3,
    );
}

#[test]
fn client_dispatch_reaches_the_new_packets() {
    let body = ints(&[0x1000, 1, 2, 3]);
    let packet = decode_client(ConnectionState::InGame, 0x53, &mut PacketReader::new(&body)).unwrap();
    assert!(matches!(packet, Some(ClientPacket::RequestGetOnVehicle(_))));

    // Ex packets: the `u16` ex id is read by the dispatcher, the body is what follows it.
    let ex = [0x88, 0x00];
    let packet = decode_client(ConnectionState::InGame, 0xD0, &mut PacketReader::new(&ex)).unwrap();
    assert_eq!(
        packet,
        Some(ClientPacket::RequestExOlympiadMatchListRefresh(
            RequestExOlympiadMatchListRefresh
        ))
    );

    let ssq = [0x04];
    let packet = decode_client(ConnectionState::InGame, 0xC8, &mut PacketReader::new(&ssq)).unwrap();
    assert_eq!(
        packet,
        Some(ClientPacket::RequestSsqStatus(RequestSsqStatus { page: 4 }))
    );
}

// ----------------------------------------------------------------------------------------------
// Olympiad and hero server packets
// ----------------------------------------------------------------------------------------------

#[test]
fn olympiad_match_end_and_mode() {
    check(&ExOlympiadMatchEnd, &ex_server(0x2D), 3);
    check(&ExOlympiadMode { mode: 3 }, &cat(&[ex_server(0x7C), vec![3]]), 3);
}

#[test]
fn olympiad_match_list_writes_type_count_and_games() {
    let packet = ExOlympiadMatchList {
        games: vec![OlympiadMatchEntry {
            stadium_id: 0,
            game_kind: 1,
            state: 2,
            player_1: "Alpha".to_owned(),
            player_2: "Beta".to_owned(),
        }],
    };
    let expected = cat(&[
        ex_server(0xD4),
        ints(&[0, 1, 0]),
        ints(&[0, 1, 2]),
        text("Alpha"),
        text("Beta"),
    ]);
    check(&packet, &expected, 3);
}

fn result_entry(name: &str) -> OlympiadResultEntry {
    OlympiadResultEntry {
        name: name.to_owned(),
        clan_name: "Clan".to_owned(),
        clan_id: 4,
        class_id: 88,
        damage: 300,
        current_points: 1700,
        diff_points: 12,
    }
}

#[test]
fn olympiad_match_result_layout() {
    // Java: type 1, tie flag, first winner's name, winning team, winners, losing team, losers.
    let packet = ExOlympiadMatchResult::new(false, 1, vec![result_entry("Win")], vec![result_entry("Lose")]);
    let entry = |name: &str| cat(&[text(name), text("Clan"), ints(&[4, 88, 300, 1700, 12])]);
    let expected = cat(&[
        ex_server(0xD4),
        ints(&[1, 0]),
        text("Win"),
        ints(&[1, 1]),
        entry("Win"),
        ints(&[2, 1]),
        entry("Lose"),
    ]);
    check(&packet, &expected, 3);
}

#[test]
fn olympiad_match_result_team_mapping() {
    // Java: winning team 2 means losing team 1; winning team 0 is shown as team 1.
    let packet = ExOlympiadMatchResult::new(true, 2, Vec::new(), Vec::new());
    assert_eq!((packet.win_team, packet.lose_team), (2, 1));
    assert!(packet.tie);
    assert_eq!(packet.winner_name, "");
    let packet = ExOlympiadMatchResult::new(false, 0, vec![result_entry("A")], Vec::new());
    assert_eq!((packet.win_team, packet.lose_team), (1, 2));
    assert_eq!(packet.winner_name, "A");
}

#[test]
fn olympiad_user_info_layout() {
    let packet = ExOlympiadUserInfo {
        side: 1,
        object_id: 0x1234,
        name: "Fighter".to_owned(),
        class_id: 88,
        cur_hp: 900,
        max_hp: 1000,
        cur_cp: 0,
        max_cp: 100,
    };
    let expected = cat(&[
        ex_server(0x7A),
        vec![1],
        ints(&[0x1234]),
        text("Fighter"),
        ints(&[88, 900, 1000, 0, 100]),
    ]);
    check(&packet, &expected, 3);
}

#[test]
fn olympiad_spelled_info_writes_skill_display_id_level_and_time() {
    let packet = ExOlympiadSpelledInfo {
        player_id: 42,
        effects: vec![OlympiadSpelledEffect {
            skill_id: 1204,
            level: 2,
            time: 600,
        }],
    };
    let mut effect = ints(&[1204]);
    effect.extend(2_i16.to_le_bytes());
    effect.extend(ints(&[600]));
    let expected = cat(&[ex_server(0x7B), ints(&[42, 1]), effect]);
    check(&packet, &expected, 3);
}

#[test]
fn hero_list_layout() {
    let packet = ExHeroList {
        heroes: vec![HeroEntry {
            name: "Hero".to_owned(),
            class_id: 88,
            clan_name: String::new(),
            clan_crest: 0,
            ally_name: "Ally".to_owned(),
            ally_crest: 9,
            count: 3,
        }],
    };
    let expected = cat(&[
        ex_server(0x79),
        ints(&[1]),
        text("Hero"),
        ints(&[88]),
        text(""),
        ints(&[0]),
        text("Ally"),
        ints(&[9, 3]),
    ]);
    check(&packet, &expected, 3);
}

// ----------------------------------------------------------------------------------------------
// Seven Signs and the Seed map
// ----------------------------------------------------------------------------------------------

/// A Seven Signs state with nothing in it, to change in each test.
fn quiet_view() -> SevenSignsView {
    SevenSignsView {
        winning_cabal: 0,
        current_period: 1,
        current_cycle: 3,
        player_cabal: 0,
        player_seal: 0,
        player_stone_contrib: 0,
        player_adena_collect: 0,
        dawn_stone_score: 0.0,
        dusk_stone_score: 0.0,
        dawn_festival_score: 0,
        dusk_festival_score: 0,
        dawn_total_score: 0,
        dusk_total_score: 0,
        total_dawn_members: 0,
        total_dusk_members: 0,
        seal_owner: [0; 4],
        dawn_proportion: [0; 4],
        dusk_proportion: [0; 4],
        festivals: Vec::new(),
    }
}

#[test]
fn ssq_status_page_one_writes_cycle_messages_and_scores() {
    let mut view = quiet_view();
    view.current_period = 2; // COMP_RESULTS: statistics message, "until today" deadline.
    view.player_cabal = 2;
    view.player_seal = 3;
    view.player_stone_contrib = 40;
    view.player_adena_collect = 500;
    view.dawn_total_score = 300;
    view.dusk_total_score = 100;
    view.dawn_stone_score = 3.0;
    view.dusk_stone_score = 1.0;
    let status = SsqStatus::new(1, &view);
    let SsqStatusBody::Cycle(cycle) = &status.body else {
        panic!("page 1 has a cycle body");
    };
    // Java: 75% of the total is dawn, the stone share of 500 is 375 and 125.
    assert_eq!(cycle.dawn.percent, 75);
    assert_eq!(cycle.dusk.percent, 25);
    assert_eq!(cycle.dawn.stone_score, 375);
    assert_eq!(cycle.dusk.stone_score, 125);
    assert_eq!(cycle.period_message, 1184);
    assert_eq!(cycle.deadline_message, 1287);

    let bytes = status.to_bytes();
    assert_eq!(&bytes[..4], &[0xFB, 1, 2, 3]);
    assert_eq!(
        SsqStatus::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
        status
    );

    let explicit = SsqStatus {
        page: 1,
        period: 2,
        body: SsqStatusBody::Cycle(SsqCycle {
            cycle: 3,
            period_message: 1184,
            deadline_message: 1287,
            player_cabal: 2,
            player_seal: 3,
            stone_contrib: 40,
            adena_collect: 500,
            dusk: SsqScore {
                stone_score: 125,
                festival_score: 0,
                total_score: 100,
                percent: 25,
            },
            dawn: SsqScore {
                stone_score: 375,
                festival_score: 0,
                total_score: 300,
                percent: 75,
            },
        }),
    };
    let expected = cat(&[
        vec![0xFB, 1, 2],
        ints(&[3, 1184, 1287]),
        vec![2, 3],
        40_i64.to_le_bytes().to_vec(),
        500_i64.to_le_bytes().to_vec(),
        125_i64.to_le_bytes().to_vec(),
        0_i64.to_le_bytes().to_vec(),
        100_i64.to_le_bytes().to_vec(),
        vec![25],
        375_i64.to_le_bytes().to_vec(),
        0_i64.to_le_bytes().to_vec(),
        300_i64.to_le_bytes().to_vec(),
        vec![75],
    ]);
    check(&explicit, &expected, 1);
}

#[test]
fn ssq_status_page_two_writes_five_festivals() {
    let festival = |level: i32, dusk: i64, dawn: i64| SsqFestival {
        level_score: level,
        dusk_score: dusk,
        dusk_members: vec!["Dusk".to_owned()],
        dawn_score: dawn,
        dawn_members: Vec::new(),
    };
    let festivals = vec![
        festival(60, 10, 0),
        festival(70, 0, 20),
        festival(100, 0, 0),
        festival(120, 0, 0),
        festival(150, 0, 0),
    ];
    let packet = SsqStatus {
        page: 2,
        period: 1,
        body: SsqStatusBody::Festivals(festivals.clone()),
    };
    let bytes = packet.to_bytes();
    // Java: short 1, then the festival count byte 5, then per festival: id, level score, dusk.
    assert_eq!(&bytes[..6], &[0xFB, 2, 1, 1, 0, 5]);
    assert_eq!(bytes[6], 1);
    assert_eq!(
        SsqStatus::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
        packet
    );
}

#[test]
fn ssq_status_page_three_writes_seal_percentages() {
    let mut view = quiet_view();
    view.total_dawn_members = 4;
    view.total_dusk_members = 0;
    view.seal_owner = [0, 2, 0, 0];
    view.dawn_proportion = [0, 2, 0, 0];
    let status = SsqStatus::new(3, &view);
    let SsqStatusBody::Seals(seals) = &status.body else {
        panic!("page 3 has seals");
    };
    assert_eq!((seals.retain_limit, seals.claim_limit), (10, 35));
    // No dusk members: 0%. Dawn: 2 of 4 is 50%.
    assert_eq!(
        seals.rows[0],
        SsqSealRow {
            seal: 1,
            owner: 2,
            dusk_percent: 0,
            dawn_percent: 50
        }
    );
    assert_eq!(seals.rows.len(), 3);
    let bytes = status.to_bytes();
    assert_eq!(&bytes[..7], &[0xFB, 3, 1, 10, 35, 3, 1]);
    assert_eq!(
        SsqStatus::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
        status
    );
}

#[test]
fn ssq_status_page_four_predicts_the_seals() {
    let mut view = quiet_view();
    view.winning_cabal = 2; // dawn
    view.total_dawn_members = 10;
    view.seal_owner = [0, 0, 0, 0];
    view.dawn_proportion = [0, 7, 0, 0];
    let status = SsqStatus::new(4, &view);
    let SsqStatusBody::Predicted(predicted) = &status.body else {
        panic!("page 4 has predictions");
    };
    assert_eq!(predicted.winner, 2);
    // 7 of 10 dawn members is 70%, at least 35: dawn gets seal 1 "although not owned 35%".
    assert_eq!(
        predicted.seals[0],
        SsqSealOutcome {
            seal: 1,
            owner: 2,
            message: 1290
        }
    );
    // 0% for dawn: the seal stays with nobody, "less than 35 percent voted".
    assert_eq!(
        predicted.seals[1],
        SsqSealOutcome {
            seal: 2,
            owner: 0,
            message: 1292
        }
    );
    let bytes = status.to_bytes();
    assert_eq!(&bytes[..4], &[0xFB, 4, 1, 2]);
    assert_eq!(
        SsqStatus::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
        status
    );
}

#[test]
fn ssq_status_other_pages_have_no_body() {
    let view = quiet_view();
    let status = SsqStatus::new(9, &view);
    assert_eq!(status.body, SsqStatusBody::None);
    assert_eq!(status.to_bytes(), vec![0xFB, 9, 1]);
    let predicted = SsqPredicted {
        winner: 0,
        seals: Vec::new(),
    };
    assert_eq!(predicted.seals.len(), 0);
}

#[test]
fn seed_map_info_writes_two_entrances_with_states() {
    // Seed of Destruction state 1 is 2771; Seed of Infinity stage 2 is 2767 (Java constants).
    let packet = ExShowSeedMapInfo::new(1, 2);
    let expected = cat(&[
        ex_server(0xA1),
        ints(&[2, -246_857, 251_960, 4331, 2771, -213_770, 210_760, 4400, 2767]),
    ]);
    check(&packet, &expected, 3);
    assert_eq!(packet.entries.len(), 2);
}

#[test]
fn festival_high_score_is_plain_data() {
    // The world fills the view; the packet only keeps what Java writes.
    let high = FestivalHighScore {
        dusk_score: 1,
        dusk_members: vec!["A".to_owned()],
        dawn_score: 2,
        dawn_members: Vec::new(),
    };
    assert_eq!(high.clone(), high);
}

// ----------------------------------------------------------------------------------------------
// Boats and air ships
// ----------------------------------------------------------------------------------------------

#[test]
fn vehicle_info_and_departure_layouts() {
    check(
        &VehicleInfo {
            object_id: 1,
            x: 2,
            y: 3,
            z: 4,
            heading: 5,
        },
        &cat(&[vec![0x60], ints(&[1, 2, 3, 4, 5])]),
        1,
    );
    // Java writes the object id, the move speed, the rotation speed, then the destination.
    check(
        &VehicleDeparture {
            object_id: 1,
            move_speed: 60,
            rotation_speed: 30,
            x: 10,
            y: 20,
            z: 30,
        },
        &cat(&[vec![0x6C], ints(&[1, 60, 30, 10, 20, 30])]),
        1,
    );
    check(
        &VehicleStarted {
            object_id: 1,
            state: 2,
        },
        &cat(&[vec![0xC0], ints(&[1, 2])]),
        1,
    );
    check(
        &VehicleCheckLocation {
            object_id: 1,
            x: 2,
            y: 3,
            z: 4,
            heading: 5,
        },
        &cat(&[vec![0x6D], ints(&[1, 2, 3, 4, 5])]),
        1,
    );
}

#[test]
fn vehicle_boarding_and_moving_layouts() {
    check(
        &GetOnVehicle {
            object_id: 1,
            boat_object_id: 2,
            x: 3,
            y: 4,
            z: 5,
        },
        &cat(&[vec![0x6E], ints(&[1, 2, 3, 4, 5])]),
        1,
    );
    check(
        &GetOffVehicle {
            object_id: 1,
            boat_object_id: 2,
            x: 3,
            y: 4,
            z: 5,
        },
        &cat(&[vec![0x6F], ints(&[1, 2, 3, 4, 5])]),
        1,
    );
    check(
        &MoveToLocationInVehicle {
            object_id: 1,
            boat_id: 2,
            destination_x: 3,
            destination_y: 4,
            destination_z: 5,
            origin_x: 6,
            origin_y: 7,
            origin_z: 8,
        },
        &cat(&[vec![0x7E], ints(&[1, 2, 3, 4, 5, 6, 7, 8])]),
        1,
    );
    check(
        &StopMoveInVehicle {
            object_id: 1,
            boat_id: 2,
            x: 3,
            y: 4,
            z: 5,
            heading: 6,
        },
        &cat(&[vec![0x7F], ints(&[1, 2, 3, 4, 5, 6])]),
        1,
    );
    check(
        &ValidateLocationInVehicle {
            object_id: 1,
            boat_object_id: 2,
            x: 3,
            y: 4,
            z: 5,
            heading: 6,
        },
        &cat(&[vec![0x80], ints(&[1, 2, 3, 4, 5, 6])]),
        1,
    );
}

#[test]
fn air_ship_info_writes_helm_points_and_fuel() {
    let packet = ExAirShipInfo {
        ship_object_id: 100,
        x: 1,
        y: 2,
        z: 3,
        heading: 4,
        captain_id: 5,
        move_speed: 6,
        rotation_speed: 7,
        helm_object_id: 8,
        helm_points: ExAirShipInfo::HELM_POINTS,
        fuel: 600,
        max_fuel: 600,
    };
    let expected = cat(&[
        ex_server(0x60),
        ints(&[100, 1, 2, 3, 4, 5, 6, 7, 8]),
        ints(&[0x16E, 0, 0x6B, 0x15C, 0, 0x69]),
        ints(&[600, 600]),
    ]);
    check(&packet, &expected, 3);
}

#[test]
fn air_ship_teleport_list_writes_index_fuel_and_last_point() {
    let packet = ExAirShipTeleportList {
        dock_id: 4,
        teleports: vec![AirShipTeleport {
            index: -1,
            fuel: 10,
            x: 1,
            y: 2,
            z: 3,
        }],
    };
    check(
        &packet,
        &cat(&[ex_server(0x9A), ints(&[4, 1, -1, 10, 1, 2, 3])]),
        3,
    );
    let empty = ExAirShipTeleportList {
        dock_id: 4,
        teleports: Vec::new(),
    };
    check(&empty, &cat(&[ex_server(0x9A), ints(&[4, 0])]), 3);
}

#[test]
fn air_ship_stop_moves_share_one_opcode() {
    // Both use `EX_STOP_MOVE_AIRSHIP` (0x66) with five ints, in different orders.
    check(
        &ExAirShipStopMove {
            air_ship_id: 1,
            player_id: 2,
            x: 3,
            y: 4,
            z: 5,
        },
        &cat(&[ex_server(0x66), ints(&[1, 2, 3, 4, 5])]),
        3,
    );
    check(
        &ExStopMoveAirShip {
            object_id: 1,
            x: 2,
            y: 3,
            z: 4,
            heading: 5,
        },
        &cat(&[ex_server(0x66), ints(&[1, 2, 3, 4, 5])]),
        3,
    );
}

#[test]
fn air_ship_passenger_packets_layouts() {
    check(
        &ExStopMoveInAirShip {
            player_id: 1,
            ship_object_id: 2,
            x: 3,
            y: 4,
            z: 5,
            heading: 6,
        },
        &cat(&[ex_server(0x6E), ints(&[1, 2, 3, 4, 5, 6])]),
        3,
    );
    check(
        &ExValidateLocationInAirShip {
            player_id: 1,
            ship_id: 2,
            x: 3,
            y: 4,
            z: 5,
            heading: 6,
        },
        &cat(&[ex_server(0x6F), ints(&[1, 2, 3, 4, 5, 6])]),
        3,
    );
    check(
        &ExGetOnAirShip {
            player_id: 1,
            air_ship_id: 2,
            x: 3,
            y: 4,
            z: 5,
        },
        &cat(&[ex_server(0x63), ints(&[1, 2, 3, 4, 5])]),
        3,
    );
    check(
        &ExGetOffAirShip {
            player_id: 1,
            air_ship_id: 2,
            x: 3,
            y: 4,
            z: 5,
        },
        &cat(&[ex_server(0x64), ints(&[1, 2, 3, 4, 5])]),
        3,
    );
    check(
        &ExMoveToLocationAirShip {
            object_id: 1,
            destination_x: 2,
            destination_y: 3,
            destination_z: 4,
            x: 5,
            y: 6,
            z: 7,
        },
        &cat(&[ex_server(0x65), ints(&[1, 2, 3, 4, 5, 6, 7])]),
        3,
    );
    check(
        &ExMoveToLocationInAirShip {
            object_id: 1,
            air_ship_id: 2,
            destination_x: 3,
            destination_y: 4,
            destination_z: 5,
            heading: 6,
        },
        &cat(&[ex_server(0x6D), ints(&[1, 2, 3, 4, 5, 6])]),
        3,
    );
}

// ----------------------------------------------------------------------------------------------
// Cube game, Underground Coliseum, movie and UI timer
// ----------------------------------------------------------------------------------------------

#[test]
fn cube_game_player_lists() {
    check(
        &ExCubeGameAddPlayer {
            red_team: true,
            object_id: 5,
            name: "Pl".to_owned(),
        },
        &cat(&[ex_server(0x97), ints(&[1, -1, 1, 5]), text("Pl")]),
        3,
    );
    check(
        &ExCubeGameRemovePlayer {
            red_team: false,
            object_id: 5,
        },
        &cat(&[ex_server(0x97), ints(&[2, -1, 0, 5])]),
        3,
    );
    // Java writes the "from red" flag and its negation.
    check(
        &ExCubeGameChangeTeam {
            object_id: 5,
            from_red_team: true,
        },
        &cat(&[ex_server(0x97), ints(&[5, 5, 1, 0])]),
        3,
    );
}

#[test]
fn cube_game_points_and_end() {
    check(
        &ExCubeGameChangePoints {
            time_left: 300,
            blue_points: 2,
            red_points: 3,
        },
        &cat(&[ex_server(0x98), ints(&[2, 300, 2, 3])]),
        3,
    );
    check(
        &ExCubeGameExtendedChangePoints {
            time_left: 300,
            blue_points: 2,
            red_points: 3,
            red_team: true,
            object_id: 5,
            player_points: 7,
        },
        &cat(&[ex_server(0x98), ints(&[0, 300, 2, 3, 1, 5, 7])]),
        3,
    );
    check(
        &ExCubeGameEnd { red_team_win: true },
        &cat(&[ex_server(0x98), ints(&[1, 1])]),
        3,
    );
}

#[test]
fn cube_game_team_list_writes_blue_then_red() {
    let packet = ExCubeGameTeamList {
        room: 1,
        blue: vec![CubePlayer {
            object_id: 10,
            name: "B".to_owned(),
        }],
        red: Vec::new(),
    };
    let expected = cat(&[ex_server(0x97), ints(&[0, 1, -1, 1, 10]), text("B"), ints(&[0])]);
    check(&packet, &expected, 3);
}

#[test]
fn cube_game_small_lists_and_timer() {
    check(&ExCubeGameCloseUi, &cat(&[ex_server(0x97), ints(&[-1])]), 3);
    check(&ExCubeGameRequestReady, &cat(&[ex_server(0x97), ints(&[4])]), 3);
    check(
        &ExCubeGameChangeTimeToStart { seconds: 20 },
        &cat(&[ex_server(0x97), ints(&[3, 20])]),
        3,
    );
}

#[test]
fn pvp_match_record_layout() {
    let packet = ExPvpMatchRecord {
        record_type: 1,
        winner_team: 2,
        blue_kills: 3,
        red_kills: 4,
        blue: vec![PvpMember {
            name: "Blue".to_owned(),
            kills: 3,
            deaths: 1,
        }],
        red: Vec::new(),
    };
    // Java writes 0 for no winner, 2 for team 1 and 1 for the others, after the team itself.
    assert_eq!(ExPvpMatchRecord::winner_code(2), 1);
    let expected = cat(&[
        ex_server(0x7E),
        ints(&[1, 2, 1, 3, 4, 1]),
        text("Blue"),
        ints(&[3, 1, 0]),
    ]);
    check(&packet, &expected, 3);
}

#[test]
fn pvp_user_die_writes_red_then_blue() {
    check(
        &ExPvpMatchUserDie {
            red_kills: 4,
            blue_kills: 3,
        },
        &cat(&[ex_server(0x7F), ints(&[4, 3])]),
        3,
    );
}

#[test]
fn cube_record_packets_layout() {
    use l2_protocol_game::server::minigame::{
        CC_RECORD_UPDATE, CcRecordEntry, ExPvpMatchCcMyRecord, ExPvpMatchCcRecord, ExPvpMatchCcRetire,
    };
    let record = ExPvpMatchCcRecord {
        state: CC_RECORD_UPDATE,
        players: vec![
            CcRecordEntry {
                name: "A".to_owned(),
                points: 9,
            },
            CcRecordEntry {
                name: "B".to_owned(),
                points: 3,
            },
        ],
    };
    check(
        &record,
        &cat(&[
            ex_server(0x89),
            ints(&[1, 2]),
            text("A"),
            ints(&[9]),
            text("B"),
            ints(&[3]),
        ]),
        3,
    );
    check(
        &ExPvpMatchCcMyRecord { points: 12 },
        &cat(&[ex_server(0x8A), ints(&[12])]),
        3,
    );
    check(&ExPvpMatchCcRetire, &ex_server(0x8B), 3);
}

#[test]
fn cube_record_is_cut_at_25_players() {
    use l2_protocol_game::server::minigame::{CcRecordEntry, ExPvpMatchCcRecord};
    let players: Vec<CcRecordEntry> = (0..30)
        .map(|i| CcRecordEntry {
            name: format!("P{i}"),
            points: i,
        })
        .collect();
    let bytes = ExPvpMatchCcRecord { state: 0, players }.to_bytes();
    let decoded = ExPvpMatchCcRecord::decode(&mut PacketReader::new(&bytes[3..])).unwrap();
    assert_eq!(decoded.players.len(), 25);
    assert_eq!(decoded.players[24].name, "P24");
}

#[test]
fn movie_start_writes_the_client_id() {
    check(
        &ExStartScenePlayer { movie_id: 9 },
        &cat(&[ex_server(0x99), ints(&[9])]),
        3,
    );
}

#[test]
fn send_ui_event_writes_minutes_and_seconds_as_strings() {
    let packet = ExSendUiEvent {
        object_id: 5,
        hide: false,
        count_up: true,
        start_time: 65,
        end_time: 125,
        npc_string_id: -1,
        params: vec!["Go".to_owned()],
    };
    // 65 s is "1" and "5"; 125 s is "2" and "5"; the count-up flag is the text "1".
    let expected = cat(&[
        ex_server(0x8E),
        ints(&[5, 0, 0, 0]),
        text("1"),
        text("1"),
        text("5"),
        text("2"),
        text("5"),
        ints(&[-1]),
        text("Go"),
    ]);
    check(&packet, &expected, 3);
}

// ----------------------------------------------------------------------------------------------
// Boss records and cursed weapons
// ----------------------------------------------------------------------------------------------

#[test]
fn boss_record_writes_a_zero_after_each_boss() {
    let packet = ExGetBossRecord {
        ranking: 2,
        total_points: 150,
        records: vec![BossRecordEntry {
            boss_id: 25001,
            points: 40,
        }],
    };
    check(
        &packet,
        &cat(&[ex_server(0x34), ints(&[2, 150, 1, 25001, 40, 0])]),
        3,
    );
}

#[test]
fn cursed_weapon_list_and_location() {
    check(
        &ExCursedWeaponList {
            ids: vec![3110, 3111],
        },
        &cat(&[ex_server(0x46), ints(&[2, 3110, 3111])]),
        3,
    );
    check(
        &ExCursedWeaponLocation {
            locations: vec![CursedWeaponLocation {
                id: 3110,
                activated: 1,
                x: 1,
                y: 2,
                z: 3,
            }],
        },
        &cat(&[ex_server(0x47), ints(&[1, 3110, 1, 1, 2, 3])]),
        3,
    );
    // An empty list is two zeros in Java.
    check(
        &ExCursedWeaponLocation {
            locations: Vec::new(),
        },
        &cat(&[ex_server(0x47), ints(&[0, 0])]),
        3,
    );
}

// ----------------------------------------------------------------------------------------------
// Ids of the M8 ex packets, as Java declares them
// ----------------------------------------------------------------------------------------------

#[test]
fn m8_packets_use_the_generated_opcode_ids() {
    use l2_protocol_game::generated::opcodes::{ClientOpcode, ExClientOpcode, ServerOpcode};
    assert_eq!(ObserverReturn.to_bytes(), vec![ClientOpcode::ObserverReturn.id()]);
    assert_eq!(
        RequestSeedPhase.to_bytes(),
        vec![0xD0, ExClientOpcode::RequestSeedPhase.id() as u8, 0]
    );
    assert_eq!(
        ExOlympiadMatchEnd.to_bytes(),
        vec![ServerOpcode::ExOlympiadMatchEnd.id(), 0x2D, 0]
    );
    assert_eq!(ExSendUiEvent::OPCODE, ServerOpcode::ExSendUiEvent);
}
