#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The boss managers (M8-BOSS-01): a raid boss's kill and its respawn time, the raid points and
//! their ranking, the weekly reset, a grand boss state read and written, the D-53 switch off case,
//! `RequestGetBossRecord`, the boss zones and the admin commands.
//!
//! The expected numbers are read off the Java sources, not off this code. Java:
//! managers/{GrandBossManager,RaidBossSpawnManager,RaidBossPointsManager}.java,
//! entity/actor/instance/{RaidBoss,GrandBoss}.java, entity/zone/type/BossZone.java,
//! network/clientpackets/RequestGetBossRecord.java, network/serverpackets/ExGetBossRecord.java.

mod party_scene;

use std::sync::Arc;

use l2_data::{NpcLoadOptions, NpcTemplates, SpawnData};
use l2_net::Encode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{BossRecordEntry, ExGetBossRecord};
use l2_world::*;
use party_scene::*;

const RAID: i32 = 9002;
const GRAND: i32 = 9003;
/// A raid boss the scene has no row for.
const OTHER_RAID: i32 = 9004;

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap())
        .unwrap()
}

/// The scene's monster plus a raid boss, a grand boss and a second raid boss, all with 300 HP.
fn templates() -> Arc<NpcTemplates> {
    let base = monster_xml(10, 1000, 100);
    let as_type = |id: i32, kind: &str, name: &str| {
        base.replace(
            r#"id="9001" type="Monster""#,
            &format!(r#"id="{id}" type="{kind}""#),
        )
        .replace(r#"name="Loot Monster""#, &format!(r#"name="{name}""#))
    };
    Arc::new(
        NpcTemplates::from_xml(
            &format!(
                "<list>{base}{}{}{}</list>",
                as_type(RAID, "RaidBoss", "Raid One"),
                as_type(GRAND, "GrandBoss", "Grand One"),
                as_type(OTHER_RAID, "RaidBoss", "Raid Two"),
            ),
            &NpcLoadOptions::default(),
        )
        .unwrap(),
    )
}

fn scene(settings: BossSettings) -> Scene {
    let mut s = Scene::new();
    s.world.set_npc_data(templates(), Arc::new(SpawnData::default()));
    s.world.set_boss_settings(settings);
    s
}

fn writes_on() -> BossSettings {
    BossSettings {
        cycle_database_writes: true,
        ..BossSettings::default()
    }
}

fn raid_row(boss_id: i32, respawn_time: i64) -> RaidBossData {
    RaidBossData {
        boss_id,
        amount: 1,
        loc_x: X,
        loc_y: Y,
        loc_z: Z,
        heading: 0,
        // 24 hours +- 12 hours, as the shipped rows.
        respawn_delay: 86_400,
        respawn_random: 43_200,
        respawn_time,
        current_hp: 123.0,
        current_mp: 45.0,
    }
}

fn grand_row(boss_id: i32, status: i32, respawn_time: i64) -> GrandBossData {
    GrandBossData {
        boss_id,
        loc_x: 1,
        loc_y: 2,
        loc_z: 3,
        heading: 4,
        respawn_time,
        current_hp: 1000.0,
        current_mp: 500.0,
        status,
    }
}

/// The boss writes the outbox got, in order.
fn boss_writes(s: &Scene) -> Vec<BossWrite> {
    s.persisted
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            PersistCommand::Boss(w) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn reads(s: &Scene) -> Vec<(BossReadKind, BossReadThen)> {
    s.persisted
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            PersistCommand::BossRereadRequest { kind, then } => Some((*kind, *then)),
            _ => None,
        })
        .collect()
}

fn clear_persisted(s: &Scene) {
    s.persisted.0.lock().unwrap().clear();
}

/// The raid points rows written for `player`, in order: `(boss id, new total)`.
fn point_writes(s: &Scene, player: i32) -> Vec<(i32, i32)> {
    boss_writes(s)
        .into_iter()
        .filter_map(|w| match w {
            BossWrite::RaidPoints {
                char_id,
                boss_id,
                points,
            } if char_id == player => Some((boss_id, points)),
            _ => None,
        })
        .collect()
}

fn kill(s: &mut Scene, boss: ObjectId, by: i32) {
    s.hit(boss, by, 1);
    assert!(s.world.do_die(boss, Some(id(by))));
}

fn alive_npcs_of(s: &Scene, npc_id: i32) -> usize {
    s.world
        .npc_ids()
        .into_iter()
        .filter(|n| s.world.npc(*n).is_some_and(|n| n.npc_id == npc_id) && !s.world.is_dead(*n))
        .count()
}

// ------------------------------------------------------------------ the raid boss spawn list

/// `RaidBossSpawnManager.load`: a row with no respawn time stands in the world at its place with
/// the HP and MP of the row, and `isDefined` is true for it.
#[test]
fn a_raid_boss_row_stands_in_the_world_with_its_hp() {
    let mut s = scene(writes_on());
    s.world.spawn_raid_bosses(vec![raid_row(RAID, 0)]);
    let boss = s.world.raid_boss_npc(RAID).unwrap();
    let npc = s.world.npc(boss).unwrap();
    assert_eq!((npc.core.x, npc.core.y), (X, Y));
    assert_eq!((npc.core.cur_hp, npc.core.cur_mp), (123.0, 40.0f64.min(45.0)));
    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Alive);
    assert_eq!(s.world.raid_boss_respawn_time(RAID), Some(0));
    assert!(s.world.raid_boss_defined(RAID));
    assert!(!s.world.raid_boss_defined(OTHER_RAID));
    assert_eq!(s.world.raid_boss_status(OTHER_RAID), RaidBossStatus::Undefined);
    assert_eq!(s.world.raid_boss_status_lines(), ["Raid One: ALIVE"]);
}

/// `RaidBoss.doDie`: the kill sets `DEAD`, picks a respawn time between the minimum and the
/// maximum delay (`respawn_delay` -+ `respawn_random`, in seconds, times the multipliers) from now,
/// stores full HP and MP, schedules the respawn and writes the rows.
#[test]
fn a_raid_boss_kill_sets_the_respawn_time_and_writes_the_rows() {
    let mut s = scene(writes_on());
    s.world.spawn_raid_bosses(vec![raid_row(RAID, 0)]);
    s.enter(P1, "Alpha");
    s.log.take();
    clear_persisted(&s);
    let boss = s.world.raid_boss_npc(RAID).unwrap();

    let before = now_ms();
    kill(&mut s, boss, P1);
    let after = now_ms();

    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Dead);
    let respawn = s.world.raid_boss_respawn_time(RAID).unwrap();
    // 86400 - 43200 = 43200 s and 86400 + 43200 = 129600 s.
    assert!(
        respawn >= before + 43_200_000 && respawn <= after + 129_600_000,
        "{respawn}"
    );
    assert!(respawn > before + 43_200_000 - 1);

    let updates: Vec<_> = boss_writes(&s)
        .into_iter()
        .filter_map(|w| match w {
            BossWrite::RaidBossUpdate(rows) => Some(rows),
            _ => None,
        })
        .collect();
    assert_eq!(updates.len(), 1, "one updateDb() for the schedule");
    assert_eq!(
        updates[0],
        [RaidBossUpdateRow {
            boss_id: RAID,
            respawn_time: respawn,
            // A dead boss is written with its maximum HP and MP.
            current_hp: 300.0,
            current_mp: 40.0,
        }]
    );
    // The player got the raid message and points of level / 2 + Rnd.get(-5, 5) = 0..10.
    let ids = message_ids(&s.log.take(), P1);
    assert!(ids.contains(&SystemMessageId::CongratulationsYourRaidWasSuccessful));
    let writes = point_writes(&s, P1);
    assert_eq!(writes.len(), 1);
    assert_eq!(writes[0].0, RAID);
    assert!((0..=10).contains(&writes[0].1), "{writes:?}");
    assert_eq!(s.world.raid_points_of(P1), writes[0].1);
}

/// The respawn timer (`SpawnSchedule`) brings the boss back once with full vitals and no respawn
/// time; the old corpse decays without a regular respawn (`Npc.onDecay`: `isDefined`).
#[test]
fn the_respawn_timer_brings_the_boss_back_once() {
    let mut s = scene(BossSettings {
        // 43200000 ms and 129600000 ms times 0.00001: about 432 ms to 1296 ms.
        raid_min_respawn_multiplier: 0.00001,
        raid_max_respawn_multiplier: 0.00001,
        ..writes_on()
    });
    s.world.spawn_raid_bosses(vec![raid_row(RAID, 0)]);
    s.enter(P1, "Alpha");
    let old = s.world.raid_boss_npc(RAID).unwrap();
    let before = now_ms();
    kill(&mut s, old, P1);
    let respawn = s.world.raid_boss_respawn_time(RAID).unwrap();
    // `(int) (43200000 * 0.00001f)` is 432, `(int) (129600000 * 0.00001f)` is 1296.
    assert!(
        respawn - before >= 432 && respawn - before <= 1296 + 200,
        "{}",
        respawn - before
    );
    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Dead);
    assert_eq!(alive_npcs_of(&s, RAID), 0);

    // Before the timer: still dead. Two seconds later: back.
    s.ticks(5);
    assert_eq!(alive_npcs_of(&s, RAID), 0);
    s.ticks(40);
    assert_eq!(alive_npcs_of(&s, RAID), 1);
    let back = s.world.raid_boss_npc(RAID).unwrap();
    assert_ne!(back, old);
    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Alive);
    assert_eq!(s.world.raid_boss_respawn_time(RAID), Some(0));
    assert_eq!(s.world.npc(back).unwrap().core.cur_hp, 300.0);

    // The corpse decays (7 s corpse time or less) and does not bring a second boss.
    s.ticks(400);
    assert!(!s.world.has_pending_respawn(old));
    assert_eq!(alive_npcs_of(&s, RAID), 1);
    assert_eq!(s.world.raid_boss_npc(RAID), Some(back));
}

/// A boss whose row has a respawn time in the future is not in the world: it counts as `DEAD`
/// and spawns when the time comes.
#[test]
fn a_boss_waiting_for_its_respawn_time_spawns_when_it_is_due() {
    let mut s = scene(BossSettings::default());
    s.world.spawn_raid_bosses(vec![raid_row(RAID, now_ms() + 3_000)]);
    assert_eq!(s.world.raid_boss_npc(RAID), None);
    assert_eq!(alive_npcs_of(&s, RAID), 0);
    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Dead);
    assert!(s.world.raid_boss_defined(RAID));
    s.ticks(20);
    assert_eq!(alive_npcs_of(&s, RAID), 0);
    s.ticks(60);
    assert_eq!(alive_npcs_of(&s, RAID), 1);
    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Alive);
    // `SpawnSchedule` spawns a fresh boss: full HP.
    let boss = s.world.raid_boss_npc(RAID).unwrap();
    assert_eq!(s.world.npc(boss).unwrap().core.cur_hp, 300.0);

    // A respawn time in the past stands in the world at once.
    let mut s = scene(BossSettings::default());
    s.world.spawn_raid_bosses(vec![raid_row(RAID, now_ms() - 1)]);
    assert_eq!(alive_npcs_of(&s, RAID), 1);
}

/// `RaidBoss.doDie` with a party: everybody gets the message and their own points.
#[test]
fn a_party_kill_gives_every_member_points_and_the_message() {
    let mut s = scene(BossSettings::default());
    s.world.spawn_raid_bosses(vec![raid_row(RAID, 0)]);
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    s.form_party(P1, &[(P2, "Bravo")], 0);
    s.log.take();
    let boss = s.world.raid_boss_npc(RAID).unwrap();
    kill(&mut s, boss, P1);
    let sent = s.log.take();
    for who in [P1, P2] {
        assert!(
            message_ids(&sent, who).contains(&SystemMessageId::CongratulationsYourRaidWasSuccessful),
            "{who:x}"
        );
        assert_eq!(point_writes(&s, who).len(), 1, "{who:x}");
        assert!((0..=10).contains(&point_writes(&s, who)[0].1));
    }
    assert!(!message_ids(&sent, P3).contains(&SystemMessageId::CongratulationsYourRaidWasSuccessful));
    assert!(point_writes(&s, P3).is_empty());
}

/// A kill by a monster (no player) gives no points and no message but still sets the respawn.
#[test]
fn a_kill_without_a_player_still_sets_the_respawn() {
    let mut s = scene(BossSettings::default());
    s.world.spawn_raid_bosses(vec![raid_row(RAID, 0)]);
    let boss = s.world.raid_boss_npc(RAID).unwrap();
    assert!(s.world.do_die(boss, None));
    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Dead);
    assert!(point_writes(&s, P1).is_empty());
    assert!(s.world.raid_boss_respawn_time(RAID).unwrap() > 0);
}

/// The switch off case (D-53): the world keeps the respawn time and the schedule in memory but
/// no row of `raidboss_spawnlist` or `grandboss_data` is written. The raid points of a player are
/// written as Java writes them.
#[test]
fn with_the_switch_off_the_boss_rows_stay_in_memory() {
    let mut s = scene(BossSettings::default());
    s.world.spawn_raid_bosses(vec![raid_row(RAID, 0)]);
    s.world.load_bosses(BossLoad {
        grand_bosses: vec![grand_row(GRAND, 0, 0)],
        ..BossLoad::default()
    });
    s.enter(P1, "Alpha");
    clear_persisted(&s);
    let boss = s.world.raid_boss_npc(RAID).unwrap();
    kill(&mut s, boss, P1);

    // The respawn time is in memory and the boss is scheduled.
    assert!(s.world.raid_boss_respawn_time(RAID).unwrap() > now_ms());
    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Dead);
    // Only the raid points row of the player went out.
    let writes = boss_writes(&s);
    assert_eq!(writes.len(), 1, "{writes:?}");
    assert!(matches!(writes[0], BossWrite::RaidPoints { .. }));

    // A grand boss status changes in memory only; the 5 minute store writes nothing.
    s.world.set_grand_boss_status(GRAND, 3);
    assert_eq!(s.world.grand_boss_status(GRAND), Some(3));
    s.ticks(6_100);
    assert_eq!(boss_writes(&s).len(), 1);

    // A raid boss added by a GM is not written either, and neither is its deletion.
    s.world.raid_delete_spawn(RAID, true);
    assert!(!s.world.raid_boss_defined(RAID));
    assert_eq!(boss_writes(&s).len(), 1);
    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Undefined);
}

/// `deleteSpawn(spawn, true)` with the switch on deletes the row; the schedule goes with it.
#[test]
fn deleting_a_raid_spawn_removes_its_schedule_and_row() {
    let mut s = scene(writes_on());
    s.world.spawn_raid_bosses(vec![raid_row(RAID, now_ms() + 60_000)]);
    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Dead);
    s.world.raid_delete_spawn(RAID, true);
    assert_eq!(boss_writes(&s), [BossWrite::RaidBossDelete { boss_id: RAID }]);
    s.ticks(1300);
    assert_eq!(alive_npcs_of(&s, RAID), 0, "the cancelled timer never fires");
}

/// `RaidBossSpawnManager.cleanUp` (shutdown, `//unspawnall`): the rows of the bosses are written
/// and everything is forgotten.
#[test]
fn clean_up_writes_the_rows_and_forgets_the_bosses() {
    let mut s = scene(writes_on());
    s.world.spawn_raid_bosses(vec![raid_row(RAID, 0)]);
    clear_persisted(&s);
    s.world.raid_clean_up();
    let writes = boss_writes(&s);
    assert_eq!(writes.len(), 1);
    match &writes[0] {
        BossWrite::RaidBossUpdate(rows) => {
            assert_eq!(rows.len(), 1);
            // `updateStatus(boss, false)` refreshed the info from the live boss.
            assert_eq!((rows[0].current_hp, rows[0].respawn_time), (123.0, 0));
        }
        other => panic!("{other:?}"),
    }
    assert!(!s.world.raid_boss_defined(RAID));
}

// ------------------------------------------------------------------ the raid points

/// `addPoints` merges per boss and writes the new total; the sum, the ranking (best first) and the
/// list in Java's `HashMap` order follow.
#[test]
fn raid_points_add_up_rank_and_list_in_java_order() {
    let mut s = scene(BossSettings::default());
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.enter(P3, "Charlie");
    s.world.add_raid_points(id(P1), 29001, 10);
    s.world.add_raid_points(id(P1), 25001, 5);
    s.world.add_raid_points(id(P1), 29001, 3);
    s.world.add_raid_points(id(P2), 25001, 12);
    // REPLACE INTO character_raid_points with the new total of the boss.
    assert_eq!(point_writes(&s, P1), [(29001, 10), (25001, 5), (29001, 13)]);
    assert_eq!(s.world.raid_points_of(P1), 18);
    assert_eq!(s.world.raid_points_of(P2), 12);
    assert_eq!(s.world.raid_points_of(P3), 0);
    assert_eq!(s.world.raid_ranking_of(P1), 1);
    assert_eq!(s.world.raid_ranking_of(P2), 2);
    assert_eq!(s.world.raid_ranking_of(P3), 0);
    // 29001 & 15 = 9 and 25001 & 15 = 9: the same bucket, insertion order; so [29001, 25001].
    assert_eq!(s.world.raid_points_list(P1), Some(vec![(29001, 13), (25001, 5)]));
    // 29006 & 15 = 14 comes after 29001's bucket 9 although it was added first.
    s.world.add_raid_points(id(P3), 29006, 1);
    s.world.add_raid_points(id(P3), 29001, 2);
    assert_eq!(s.world.raid_points_list(P3), Some(vec![(29001, 2), (29006, 1)]));
    assert_eq!(s.world.raid_points_list(0x2000_0099), None);
}

/// `RequestGetBossRecord`: `ExGetBossRecord` with the ranking, the points and a boss line each;
/// a player without points gets Java's null list (the count and three more zeros).
#[test]
fn the_boss_record_packet_has_the_ranking_the_points_and_the_list() {
    let mut s = scene(BossSettings::default());
    s.enter(P1, "Alpha");
    s.enter(P2, "Bravo");
    s.world.add_raid_points(id(P1), 29001, 10);
    s.world.add_raid_points(id(P1), 25001, 5);
    s.log.take();

    s.input(P1, ClientInput::GetBossRecord { boss_id: 0 });
    let sent = s.log.take();
    let packets = of(&sent, P1, "EX_GET_BOSS_RECORD");
    assert_eq!(packets.len(), 1);
    let expected = ExGetBossRecord {
        ranking: 1,
        total_points: 15,
        records: vec![
            BossRecordEntry {
                boss_id: 29001,
                points: 10,
            },
            BossRecordEntry {
                boss_id: 25001,
                points: 5,
            },
        ],
    };
    assert_eq!(packets[0], expected.to_bytes().as_slice());

    // No points at all: `getList` is null, Java writes four zeros after the two numbers.
    s.input(P2, ClientInput::GetBossRecord { boss_id: 0 });
    let sent = s.log.take();
    let packets = of(&sent, P2, "EX_GET_BOSS_RECORD");
    assert_eq!(packets.len(), 1);
    let mut expected = ExGetBossRecord {
        ranking: 0,
        total_points: 0,
        records: Vec::new(),
    }
    .to_bytes();
    expected.extend_from_slice(&[0; 12]);
    assert_eq!(packets[0], expected.as_slice());
}

fn clan_of(members: &[i32]) -> ClanRecord {
    use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
    ClanRecord {
        clan_id: 268_500_000,
        data: Some(ClanDataRecord {
            name: Some("Lions".to_owned()),
            level: 5,
            leader_id: members[0],
            reputation_score: 100,
            ..ClanDataRecord::default()
        }),
        members: members
            .iter()
            .map(|m| ClanMemberRecord {
                object_id: *m,
                name: format!("M{m:x}"),
                level: 40,
                class_id: 0,
                title: String::new(),
                power_grade: 1,
                pledge_type: 0,
                apprentice: 0,
                sponsor: 0,
                female: false,
                race: 0,
            })
            .collect(),
        ..ClanRecord::default()
    }
}

/// 1970-01-01 was a Thursday: day 4 is a Monday, day 5 a Tuesday.
const MONDAY: i64 = 4;
const TUESDAY: i64 = 5;

/// `resetRaidPoints` on a Monday: the clans of the best players get reputation (1st 1250, 2nd 900
/// by default), the table is deleted and the memory cleared. Another day nothing happens.
#[test]
fn the_monday_reset_pays_the_clans_and_clears_the_points() {
    let mut s = scene(writes_on());
    // P1 and P2 are in the clan; P3 (rank 3) is not.
    s.world.load_clans(vec![clan_of(&[P1, P2])], Vec::new());
    s.world.add_raid_points(id(P1), RAID, 20);
    s.world.add_raid_points(id(P2), RAID, 15);
    s.world.add_raid_points(id(P3), RAID, 10);
    clear_persisted(&s);

    s.world.reset_raid_points_on_day(TUESDAY);
    assert_eq!(s.world.raid_points_of(P1), 20, "not a Monday");
    assert!(boss_writes(&s).is_empty());

    s.world.reset_raid_points_on_day(MONDAY);
    assert_eq!(boss_writes(&s), [BossWrite::RaidPointsDeleteAll]);
    assert_eq!(s.world.raid_points_of(P1), 0);
    assert_eq!(s.world.raid_ranking_of(P1), 0);
    // 100 + 1250 (rank 1) + 900 (rank 2).
    assert_eq!(s.world.clan(268_500_000).unwrap().reputation_score, 2250);
}

/// Without the switch the reset is a global job the Java server does: nothing is written, no
/// reputation is given, but the points in memory go.
#[test]
fn the_reset_with_the_switch_off_only_clears_the_memory() {
    let mut s = scene(BossSettings::default());
    s.world.load_clans(vec![clan_of(&[P1])], Vec::new());
    s.world.add_raid_points(id(P1), RAID, 20);
    clear_persisted(&s);
    s.world.reset_raid_points_on_day(MONDAY);
    assert!(boss_writes(&s).is_empty());
    assert_eq!(s.world.raid_points_of(P1), 0);
    assert_eq!(s.world.clan(268_500_000).unwrap().reputation_score, 100);
}

/// The reset task is one timer on the wheel that puts itself back every 24 hours (Java's fixed
/// rate); the first run is the next 00:10 (`TimeUtil.getNextTime(0, 10)`, unit-tested).
#[test]
fn the_reset_task_is_one_repeating_timer() {
    let mut s = scene(BossSettings::default());
    assert_eq!(s.world.timers().len(), 0);
    // 1970-01-05 (a Monday) 00:09:00 UTC: the first run is one minute away.
    s.world
        .start_raid_points_reset_task_at(4 * 86_400_000 + 9 * 60_000, |_| 0);
    assert_eq!(s.world.timers().len(), 1);
    // Starting again replaces the timer.
    s.world
        .start_raid_points_reset_task_at(4 * 86_400_000 + 9 * 60_000, |_| 0);
    assert_eq!(s.world.timers().len(), 1);
    // It ran at tick 1200 and is waiting for the next day.
    s.ticks(1_300);
    assert_eq!(s.world.timers().len(), 1);
}

/// D-50: the rows of a character are read again when it enters the world, and the answer replaces
/// what this server held of it.
#[test]
fn entering_the_world_reads_the_points_again() {
    let mut s = scene(BossSettings::default());
    s.world.load_bosses(BossLoad::default());
    s.world.add_raid_points(id(P1), RAID, 20);
    clear_persisted(&s);
    s.enter(P1, "Alpha");
    assert_eq!(reads(&s), [(BossReadKind::RaidPoints(P1), BossReadThen::Nothing)]);
    // The Java server gave the character points of another boss meanwhile.
    s.world.handle(WorldCommand::BossReread {
        result: Ok(BossRereadRows::RaidPoints {
            char_id: P1,
            rows: vec![
                RaidPointsRecord {
                    char_id: P1,
                    boss_id: RAID,
                    points: 20,
                },
                RaidPointsRecord {
                    char_id: P1,
                    boss_id: 25001,
                    points: 7,
                },
            ],
        }),
        then: BossReadThen::Nothing,
    });
    assert_eq!(s.world.raid_points_of(P1), 27);
    // A failed read changes nothing.
    s.world.handle(WorldCommand::BossReread {
        result: Err(()),
        then: BossReadThen::Nothing,
    });
    assert_eq!(s.world.raid_points_of(P1), 27);
}

// ------------------------------------------------------------------ the grand boss states

/// `GrandBossManager.init`: the status and the info of each row are in memory; a boss spawns on
/// this server only when the status says alive (D-53).
#[test]
fn a_grand_boss_state_is_read_at_start() {
    let mut s = scene(BossSettings::default());
    s.world.load_bosses(BossLoad {
        grand_bosses: vec![grand_row(29020, 0, 0), grand_row(29028, 3, 1_792_000_000_000)],
        ..BossLoad::default()
    });
    assert_eq!(s.world.grand_boss_status(29020), Some(0));
    assert_eq!(s.world.grand_boss_status(29028), Some(3));
    assert_eq!(s.world.grand_boss_status(1), None);
    assert!(s.world.grand_boss_may_spawn(29020));
    assert!(!s.world.grand_boss_may_spawn(29028));
    assert!(!s.world.grand_boss_may_spawn(1));
    let info = s.world.grand_boss_info(29028).unwrap();
    assert_eq!(info.respawn_time, 1_792_000_000_000);
    assert_eq!((info.loc_x, info.loc_y, info.loc_z, info.heading), (1, 2, 3, 4));
    assert_eq!((info.current_hp, info.current_mp), (1000.0, 500.0));
}

/// With the switch on the setters write Java's statements: `setStatus` the status, `setStatSet`
/// the whole row from the boss NPC (dead: full HP), `storeMe` every row and the zone lists each
/// five minutes.
#[test]
fn with_the_switch_on_the_grand_boss_rows_are_written() {
    let mut s = scene(writes_on());
    s.world.load_bosses(BossLoad {
        grand_bosses: vec![grand_row(GRAND, 0, 0), grand_row(29028, 0, 0)],
        ..BossLoad::default()
    });
    // `setStatus`: the status only.
    s.world.set_grand_boss_status(GRAND, 2);
    assert_eq!(
        boss_writes(&s),
        [BossWrite::GrandBossStatus {
            boss_id: GRAND,
            status: 2
        }]
    );

    // A boss NPC a script registered: the whole row follows the NPC.
    let npc = s.world.spawn_npc_at(GRAND, X, Y, Z, 77).unwrap();
    s.world.add_grand_boss(GRAND, npc);
    assert_eq!(s.world.grand_boss_npc(GRAND), Some(npc));
    clear_persisted(&s);
    let mut info = s.world.grand_boss_info(GRAND).unwrap().clone();
    info.respawn_time = 1_792_000_000_123;
    s.world.set_grand_boss_info(GRAND, info);
    let writes = boss_writes(&s);
    assert_eq!(writes.len(), 1);
    match &writes[0] {
        BossWrite::GrandBoss(row) => {
            assert_eq!((row.loc_x, row.loc_y, row.loc_z, row.heading), (X, Y, Z, 77));
            assert_eq!((row.respawn_time, row.status), (1_792_000_000_123, 2));
            assert_eq!((row.current_hp, row.current_mp), (300.0, 40.0));
        }
        other => panic!("{other:?}"),
    }

    // The boss dies: HP and MP are written as the maximum.
    s.world.set_current_hp(npc, 10.0, false);
    assert!(s.world.do_die(npc, None));
    clear_persisted(&s);
    s.world.store_grand_bosses();
    let writes = boss_writes(&s);
    assert_eq!(writes.len(), 1);
    match &writes[0] {
        BossWrite::GrandBossStoreAll { list, rows } => {
            assert!(list.is_empty());
            // The boss without an NPC: status only; the one with: the whole row.
            assert!(rows.contains(&GrandBossStoreRow::StatusOnly {
                boss_id: 29028,
                status: 0
            }));
            assert!(rows.iter().any(|r| matches!(
                r,
                GrandBossStoreRow::Full(row) if row.boss_id == GRAND && row.current_hp == 300.0
            )));
        }
        other => panic!("{other:?}"),
    }

    // Every 5 minutes (6000 ticks).
    clear_persisted(&s);
    s.ticks(6_001);
    assert_eq!(boss_writes(&s).len(), 1);
    s.ticks(6_000);
    assert_eq!(boss_writes(&s).len(), 2);
}

/// D-50: the answer of the read replaces the state in memory; a failed read changes nothing and
/// the page is refused.
#[test]
fn a_grand_boss_state_is_read_again_before_the_page_shows_it() {
    let mut s = scene(BossSettings::default());
    s.world.load_bosses(BossLoad {
        grand_bosses: vec![grand_row(29028, 0, 0)],
        ..BossLoad::default()
    });
    // The Java server killed Valakas meanwhile.
    s.world.handle(WorldCommand::BossReread {
        result: Ok(BossRereadRows::GrandBoss {
            boss_id: 29028,
            row: Some(grand_row(29028, 3, 1_792_000_000_000)),
        }),
        then: BossReadThen::Nothing,
    });
    assert_eq!(s.world.grand_boss_status(29028), Some(3));
    assert_eq!(
        s.world.grand_boss_info(29028).unwrap().respawn_time,
        1_792_000_000_000
    );
    // A failed read changes nothing.
    s.world.handle(WorldCommand::BossReread {
        result: Err(()),
        then: BossReadThen::Nothing,
    });
    assert_eq!(s.world.grand_boss_status(29028), Some(3));
}

// ------------------------------------------------------------------ the boss zones

const MAP_REGION: &str = r#"<list enabled="true">
  <region name="test_town" town="Test Town" castle="3" locId="911" bbs="7">
    <respawnPoint X="-84141" Y="244623" Z="-3730" />
    <map X="17" Y="25" />
  </region>
</list>"#;

const ZONES: &str = r#"<list enabled="true">
  <zone id="12010" name="Test Boss" type="BossZone" shape="Cuboid" minZ="-3500" maxZ="-2800">
    <stat name="InvadeTime" val="1800000" />
    <stat name="oustX" val="-71000" />
    <stat name="oustY" val="258000" />
    <stat name="oustZ" val="-3100" />
    <node X="-71438" Y="258171" />
    <node X="-71238" Y="258371" />
  </zone>
</list>"#;

fn zone_scene() -> (Scene, tempfile::TempDir) {
    let root = tempfile::tempdir().unwrap();
    for (path, text) in [
        ("data/zones/boss.xml", ZONES),
        ("data/mapregion/test.xml", MAP_REGION),
    ] {
        let file = root.path().join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    }
    let mut s = scene(BossSettings::default());
    s.world.set_zone_data(
        Arc::new(l2_data::Zones::load(root.path())),
        Arc::new(l2_data::MapRegions::load(root.path())),
    );
    (s, root)
}

/// `BossZone.onEnter`: a player who is not listed is teleported out (to the oust place when the
/// zone has one); a listed player may stay; the lists come from `grandboss_list`.
#[test]
fn a_boss_zone_ousts_players_who_were_not_let_in() {
    let (mut s, _root) = zone_scene();
    s.world.load_bosses(BossLoad {
        grand_boss_list: vec![(P2, 12010)],
        ..BossLoad::default()
    });
    assert_eq!(s.world.boss_zone_allowed(12010), Some(&[P2][..]));
    s.enter_at(P1, "Alpha", 10, (X, Y, Z));
    s.enter_at(P2, "Bravo", 10, (X, Y, Z));
    let p1 = s.world.position_of(id(P1)).unwrap();
    let p2 = s.world.position_of(id(P2)).unwrap();
    assert_ne!(
        (p1.0, p1.1),
        (X, Y),
        "the stranger was sent to the oust place: {p1:?}"
    );
    assert_eq!((p2.0, p2.1), (X, Y), "the listed player stays");
    assert_eq!(s.world.grand_boss_zone_at(X, Y, Z), Some(12010));
    assert_eq!(s.world.grand_boss_zone_at(0, 0, 0), None);
}

/// `allowPlayerEntry`: an AI lets a player in for some seconds; `removePlayer` and the logout
/// take the place away; a lost connection keeps it for `InvadeTime`.
#[test]
fn a_player_let_in_by_an_ai_may_enter_and_keeps_the_place_after_a_lost_connection() {
    let (mut s, _root) = zone_scene();
    s.world.load_bosses(BossLoad::default());
    // Not online yet: the entry is for the character that comes.
    s.world.allow_boss_zone_entry(12010, id(P3), 60);
    assert_eq!(s.world.boss_zone_allowed(12010), Some(&[P3][..]));
    // The entry is legal (the expiration time is in the future): no oust.
    s.enter_at(P3, "Charlie", 10, (X, Y, Z));
    let p = s.world.position_of(id(P3)).unwrap();
    assert_eq!((p.0, p.1), (X, Y));

    // A lost connection (`Disconnected`) keeps the place.
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(P3).unwrap()),
    });
    assert_eq!(s.world.boss_zone_allowed(12010), Some(&[P3][..]));

    // `removePlayer` (and a logout) takes it away.
    s.world.allow_boss_zone_entry(12010, id(P4), 60);
    assert_eq!(s.world.boss_zone_allowed(12010), Some(&[P3, P4][..]));
    s.world.remove_from_boss_zones(id(P4));
    assert_eq!(s.world.boss_zone_allowed(12010), Some(&[P3][..]));
}

/// `oustAllPlayers` sends everybody out and clears the lists.
#[test]
fn oust_all_players_clears_the_lists() {
    let (mut s, _root) = zone_scene();
    s.world.load_bosses(BossLoad {
        grand_boss_list: vec![(P1, 12010)],
        ..BossLoad::default()
    });
    s.enter_at(P1, "Alpha", 10, (X, Y, Z));
    assert_eq!(s.world.position_of(id(P1)).map(|p| (p.0, p.1)), Some((X, Y)));
    s.world.oust_all_boss_zone_players(12010);
    assert_eq!(s.world.boss_zone_allowed(12010), Some(&[][..]));
    assert_ne!(s.world.position_of(id(P1)).map(|p| (p.0, p.1)), Some((X, Y)));
}
