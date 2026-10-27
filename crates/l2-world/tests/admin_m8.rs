#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The M8-ADMIN-07 words: `//checkolympiad`, `//saveolymp`, `//endolympiad`, `//mammon_find` and
//! `//mammon_respawn`.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminAdmin,AdminMammon}.java

use std::sync::{Arc, Mutex};

use l2_data::{NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::sevensigns::{
    AutoSpawnGroup, PERIOD_COMPETITION, PERIOD_SEAL_VALIDATION, SevenSignsLoad, SevenSignsStatus,
};
use l2_world::*;

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }

    fn left_world(&self, _reason: LeaveReason) {}
}

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}

    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Fake {
    rights: AdminRights,
}

impl StaticData for Fake {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        if level >= 70 {
            AccessLevelStatic {
                name_color: 0,
                title_color: 0,
                is_gm: true,
            }
        } else {
            AccessLevelStatic::USER
        }
    }
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.rights)
    }
}

const GM: i32 = 100;
const MERCHANT: i32 = 31113;
const BLACKSMITH: i32 = 31126;
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const WEDNESDAY: i64 = 1_767_571_200_000 + 2 * 86_400_000 + 12 * 3_600_000;

fn utc(_ms: i64) -> i64 {
    0
}

const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>"#;

fn setup(period: i32, register_groups: bool) -> (World, Log) {
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(8), Box::new(NullOutbox));
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (70, 0), (100, 70)] {
        rights.add_level(level, child);
    }
    for command in [
        "checkolympiad",
        "saveolymp",
        "endolympiad",
        "mammon_find",
        "mammon_respawn",
    ] {
        rights.add_command(command, 100, false);
    }
    world.set_static_data(Box::new(Fake { rights }));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(
        NpcTemplates::from_xml(
            &format!(
                r#"<list>
<npc id="{MERCHANT}" type="SignsPriest" level="70" name="Merchant of Mammon">{STATS}</npc>
<npc id="{BLACKSMITH}" type="SignsPriest" level="70" name="Blacksmith of Mammon">{STATS}</npc>
</list>"#
            ),
            &NpcLoadOptions::default(),
        )
        .unwrap(),
    );
    let dir = tempfile::tempdir().unwrap();
    let spawns = Arc::new(SpawnData::load(dir.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.load_seven_signs_at(
        SevenSignsLoad {
            status: Some(SevenSignsStatus {
                current_cycle: 4,
                active_period: period,
                last_save: WEDNESDAY - 3_600_000,
                ..SevenSignsStatus::default()
            }),
            ..SevenSignsLoad::default()
        },
        WEDNESDAY,
        utc,
    );
    if register_groups {
        world.load_auto_spawns(
            [MERCHANT, BLACKSMITH]
                .into_iter()
                .map(|npc_id| AutoSpawnGroup {
                    npc_id,
                    count: 1,
                    initial_delay: -1,
                    respawn_delay: -1,
                    despawn_delay: -1,
                    broadcast: false,
                    random_spawn: true,
                    locations: vec![(X + npc_id % 100, Y, Z, -1)],
                })
                .collect(),
        );
    }
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(GM).unwrap()),
        player: Box::new(PlayerData {
            object_id: ObjectId(GM),
            account: "acc100".to_owned(),
            name: "Gamemaster".to_owned(),
            title: String::new(),
            class_id: 0,
            base_class: 0,
            race: 0,
            sex: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            level: 40,
            exp: 0,
            sp: 0,
            karma: 0,
            pvp_kills: 0,
            pk_kills: 0,
            access_level: 100,
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
            cur_hp: 100.0,
            max_hp: 100.0,
            cur_mp: 50.0,
            max_mp: 50.0,
            cur_cp: 10.0,
            max_cp: 10.0,
            online_time: 0,
            items: vec![],
            skills: vec![],
            shortcuts: vec![],
            profile: PlayerProfile::default(),
        }),
        sink: Box::new(Sink(GM, log.clone())),
        ip: None,
    });
    log.take();
    (world, log)
}

fn send(world: &mut World, command: &str) {
    world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(GM).unwrap()),
        input: ClientInput::BuildCmd {
            command: command.to_owned(),
        },
    });
}

fn opcode(b: &[u8]) -> Option<ServerOpcode> {
    let ex = (b[0] == 0xFE).then(|| u16::from_le_bytes([b[1], b[2]]));
    ServerOpcode::from_id(b[0], ex)
}

/// The system messages to the GM: (id, the text of a one-string message).
fn messages(out: &Sent) -> Vec<(SystemMessageId, Option<String>)> {
    out.iter()
        .filter(|(_, b)| opcode(b) == Some(ServerOpcode::SystemMessage))
        .filter_map(|(_, b)| {
            let skip = if b[0] == 0xFE { 3 } else { 1 };
            let m = SystemMessage::decode(&mut PacketReader::new(&b[skip..])).ok()?;
            let text = match m.params.as_slice() {
                [SystemMessageParam::Text(text)] => Some(text.clone()),
                _ => None,
            };
            Some((m.id, text))
        })
        .collect()
}

fn texts(out: &Sent) -> Vec<String> {
    messages(out).into_iter().filter_map(|(_, t)| t).collect()
}

fn pages(out: &Sent) -> Vec<String> {
    out.iter()
        .filter(|(_, b)| opcode(b) == Some(ServerOpcode::NpcHtmlMessage))
        .filter_map(|(_, b)| NpcHtmlMessage::decode(&mut PacketReader::new(&b[1..])).ok())
        .map(|m| m.html)
        .collect()
}

#[test]
fn checkolympiad_sends_the_olympiad_info_page() {
    let (mut world, log) = setup(PERIOD_COMPETITION, false);
    send(&mut world, "checkolympiad");
    let pages = pages(&log.take());
    assert_eq!(pages.len(), 1);
    let html = &pages[0];
    assert!(html.starts_with("<html><title>Olympiad Period</title><body>"));
    for label in [
        "Olympiad Info",
        "Current Time:",
        "Olympiad Period Ends:",
        "Next Weekly Change:",
        "bypass admin_admin2",
    ] {
        assert!(html.contains(label), "{label} missing in {html}");
    }
    assert!(html.ends_with("</body></html>"));
}

#[test]
fn saveolymp_answers_saved() {
    let (mut world, log) = setup(PERIOD_COMPETITION, false);
    send(&mut world, "saveolymp");
    assert_eq!(texts(&log.take()), vec!["olympiad system saved."]);
}

#[test]
fn endolympiad_answers_heroes_formed() {
    let (mut world, log) = setup(PERIOD_COMPETITION, false);
    send(&mut world, "endolympiad");
    assert_eq!(texts(&log.take()), vec!["Heroes formed."]);
}

#[test]
fn mammon_words_wait_for_the_seal_validation_period() {
    let (mut world, log) = setup(PERIOD_COMPETITION, false);
    for word in ["mammon_find", "mammon_respawn"] {
        send(&mut world, word);
        assert_eq!(
            messages(&log.take()),
            vec![(SystemMessageId::TheSsqCompetitionPeriodIsUnderway, None)],
            "{word}"
        );
    }
}

#[test]
fn mammon_words_report_groups_that_are_not_registered() {
    let (mut world, log) = setup(PERIOD_SEAL_VALIDATION, false);
    send(&mut world, "mammon_find");
    assert_eq!(
        texts(&log.take()),
        vec![
            "Blacksmith of Mammon isn't registered for spawn.",
            "Merchant of Mammon isn't registered for spawn."
        ]
    );
    send(&mut world, "mammon_respawn");
    assert_eq!(
        texts(&log.take()),
        vec![
            "Merchant of Mammon isn't registered for spawn.",
            "Blacksmith of Mammon isn't registered for spawn."
        ]
    );
}

#[test]
fn mammon_find_has_a_usage_for_a_bad_index() {
    let (mut world, log) = setup(PERIOD_SEAL_VALIDATION, false);
    send(&mut world, "mammon_find x");
    assert_eq!(
        texts(&log.take()),
        vec!["Usage: //mammon_find [teleportIndex] (where 1 = Blacksmith, 2 = Merchant)"]
    );
}

#[test]
fn mammon_respawn_counts_the_minutes_to_the_next_spawn() {
    let (mut world, log) = setup(PERIOD_SEAL_VALIDATION, true);
    send(&mut world, "mammon_respawn");
    // The first spawn is the default initial delay (30 seconds) away: 0 minutes.
    assert_eq!(
        texts(&log.take()),
        vec![
            "The Merchant of Mammon will respawn in 0 minute(s).",
            "The Blacksmith of Mammon will respawn in 0 minute(s)."
        ]
    );
    // After the first spawn the task repeats every hour (the default respawn delay).
    for _ in 0..601 {
        world.tick();
    }
    log.take();
    send(&mut world, "mammon_respawn");
    assert_eq!(
        texts(&log.take()),
        vec![
            "The Merchant of Mammon will respawn in 59 minute(s).",
            "The Blacksmith of Mammon will respawn in 59 minute(s)."
        ]
    );
}

#[test]
fn mammon_find_lists_the_spawned_npcs() {
    let (mut world, log) = setup(PERIOD_SEAL_VALIDATION, true);
    // Registered, but nothing spawned yet: Java prints nothing for a group without an NPC.
    send(&mut world, "mammon_find");
    assert!(texts(&log.take()).is_empty());
    for _ in 0..601 {
        world.tick();
    }
    log.take();
    send(&mut world, "mammon_find");
    assert_eq!(
        texts(&log.take()),
        vec![
            format!("Blacksmith of Mammon: {} {Y} {Z}", X + BLACKSMITH % 100),
            format!("Merchant of Mammon: {} {Y} {Z}", X + MERCHANT % 100),
        ]
    );
}

#[test]
fn mammon_find_with_an_index_teleports_to_that_npc() {
    let (mut world, log) = setup(PERIOD_SEAL_VALIDATION, true);
    for _ in 0..601 {
        world.tick();
    }
    log.take();
    send(&mut world, "mammon_find 2");
    let sent = log.take();
    assert_eq!(texts(&sent).len(), 2);
    // The teleport sends the GM a TeleportToLocation packet.
    assert!(
        sent.iter()
            .any(|(_, b)| opcode(b) == Some(ServerOpcode::TeleportToLocation)),
        "no teleport packet"
    );
}
