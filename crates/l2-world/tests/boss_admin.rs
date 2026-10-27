#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The admin side of the bosses (M8-BOSS-01): `//grandboss*` and the raid boss branches of
//! `//spawn`, `//delete` and `//recall_npc`.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminGrandBoss,AdminSpawn,
//! AdminDelete,AdminTeleport}.java

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{HtmCache, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
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

#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

struct Recorder(Persisted);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }

    fn ls_event(&mut self, _event: LsEvent) {}
}

fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [
        (-1, 0),
        (0, 0),
        (10, 0),
        (20, 10),
        (30, 20),
        (40, 30),
        (50, 40),
        (60, 50),
        (70, 60),
        (100, 70),
    ] {
        rights.add_level(level, child);
    }
    for command in [
        "spawn",
        "spawn_once",
        "delete",
        "recall_npc",
        "grandboss",
        "grandboss_skip",
        "grandboss_respawn",
        "grandboss_minions",
        "grandboss_abort",
    ] {
        rights.add_command(command, 100, false);
    }
    rights
}

fn class_data() -> StatData {
    let mut levels = String::new();
    for level in 1..=40 {
        let (hp, mp, cp) = if level == 40 {
            (100, 50, 10)
        } else {
            (level, level, level)
        };
        levels.push_str(&format!(
            "<level val=\"{level}\"><hp>{hp}</hp><mp>{mp}</mp><cp>{cp}</cp></level>"
        ));
    }
    let xml = format!(
        r#"<list><classId>0</classId><staticData>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{levels}</lvlUpgainData></list>"#
    );
    StatData {
        player_templates: Arc::new(l2_data::PlayerTemplates::from_xml(&xml, 85).unwrap()),
        ..StatData::default()
    }
}

struct Fake {
    rights: AdminRights,
    stats: StatData,
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
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const GM: i32 = 100;
const RAID: i32 = 9002;
const RAID_TWO: i32 = 9004;
const BAIUM: i32 = 29020;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="9002" type="RaidBoss" level="10" name="Raid One">
    <stats><vitals hp="300" mp="40"/></stats>
  </npc>
  <npc id="9004" type="RaidBoss" level="10" name="Raid Two">
    <stats><vitals hp="300" mp="40"/></stats>
  </npc>
</list>"#;

const BAIUM_PAGE: &str = "<html><body>%bossStatus% %bossColor% %respawnTime% %playersInside%</body></html>";

fn write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

struct Setup {
    world: World,
    log: Log,
    persisted: Persisted,
    _root: tempfile::TempDir,
}

fn setup(settings: BossSettings) -> Setup {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "data/spawns/none.xml",
        r#"<?xml version="1.0"?><list enabled="true"></list>"#,
    );
    write(
        root.path(),
        "data/html/admin/grandboss/grandboss.htm",
        "<html>GRANDBOSS MENU</html>",
    );
    write(
        root.path(),
        "data/html/admin/grandboss/grandboss_baium.htm",
        BAIUM_PAGE,
    );
    let log = Log::default();
    let persisted = Persisted::default();
    let mut world = World::new(WorldRng::from_seed(9), Box::new(Recorder(persisted.clone())));
    world.set_static_data(Box::new(Fake {
        rights: rights(),
        stats: class_data(),
    }));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    world.set_boss_settings(settings);
    Setup {
        world,
        log,
        persisted,
        _root: root,
    }
}

fn writes_on() -> BossSettings {
    BossSettings {
        cycle_database_writes: true,
        ..BossSettings::default()
    }
}

fn enter_gm(s: &mut Setup) {
    let data = PlayerData {
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
        x: X,
        y: Y,
        z: Z,
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
    };
    s.world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(GM).unwrap()),
        player: Box::new(data),
        sink: Box::new(Sink(GM, s.log.clone())),
        ip: None,
    });
}

fn cmd(s: &mut Setup, command: &str) {
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(GM).unwrap()),
        input: ClientInput::BuildCmd {
            command: command.to_owned(),
        },
    });
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn texts(out: &Sent) -> Vec<String> {
    out.iter()
        .filter(|(_, b)| name_of(b) == ServerOpcode::SystemMessage.name())
        .filter_map(|(_, b)| {
            let skip = if b[0] == 0xFE { 3 } else { 1 };
            let m = SystemMessage::decode(&mut PacketReader::new(&b[skip..])).ok()?;
            match m.params.as_slice() {
                [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
                _ => None,
            }
        })
        .collect()
}

fn pages(out: &Sent) -> Vec<NpcHtmlMessage> {
    out.iter()
        .filter(|(_, b)| name_of(b) == ServerOpcode::NpcHtmlMessage.name())
        .map(|(_, b)| NpcHtmlMessage::decode(&mut PacketReader::new(&b[1..])).unwrap())
        .collect()
}

fn boss_writes(s: &Setup) -> Vec<BossWrite> {
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

fn reads(s: &Setup) -> Vec<(BossReadKind, BossReadThen)> {
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

fn raids(s: &Setup, npc_id: i32) -> Vec<ObjectId> {
    s.world
        .npc_ids()
        .into_iter()
        .filter(|id| s.world.npc(*id).is_some_and(|n| n.npc_id == npc_id) && !s.world.is_dead(*id))
        .collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| i64::try_from(d.as_millis()).unwrap())
        .unwrap()
}

// ------------------------------------------------------------------------------ //spawn

/// `//spawn <raid boss>` goes to `RaidBossSpawnManager.addNewSpawn`: the boss stands at the GM,
/// the row is stored (permanent, with the switch on) with the template's HP and MP, and a second
/// `//spawn` of the same boss is refused.
#[test]
fn spawn_of_a_raid_boss_goes_through_the_manager() {
    let mut s = setup(writes_on());
    enter_gm(&mut s);
    s.log.take();
    cmd(&mut s, "spawn 9002");
    let sent = s.log.take();
    assert_eq!(texts(&sent), [format!("Created Raid One on {GM}")]);
    assert_eq!(raids(&s, RAID).len(), 1);
    assert!(s.world.raid_boss_defined(RAID));
    assert_eq!(s.world.raid_boss_status(RAID), RaidBossStatus::Alive);
    let writes = boss_writes(&s);
    assert_eq!(writes.len(), 1);
    match &writes[0] {
        BossWrite::RaidBossInsert(row) => {
            assert_eq!((row.boss_id, row.amount), (RAID, 1));
            assert_eq!((row.loc_x, row.loc_y, row.loc_z), (X, Y, Z));
            assert_eq!(
                (row.current_hp, row.current_mp, row.respawn_time),
                (300.0, 40.0, 0)
            );
        }
        other => panic!("{other:?}"),
    }

    cmd(&mut s, "spawn 9002");
    assert_eq!(
        texts(&s.log.take()),
        ["You cannot spawn another instance of Raid One."]
    );
    assert_eq!(raids(&s, RAID).len(), 1);
}

/// With the switch off the same command spawns the boss but writes no row.
#[test]
fn spawn_of_a_raid_boss_writes_no_row_with_the_switch_off() {
    let mut s = setup(BossSettings::default());
    enter_gm(&mut s);
    cmd(&mut s, "spawn 9002");
    assert_eq!(raids(&s, RAID).len(), 1);
    assert!(boss_writes(&s).is_empty());
}

/// NOTE(java-quirk): `setRespawnMinDelay(43200)` and `setRespawnMaxDelay(129600)` are
/// milliseconds, so a GM's raid boss is back after 43 to 130 seconds; a boss spawned with
/// `//spawn_once` (not permanent) had `stopRespawn()` called and never comes back.
#[test]
fn a_gm_raid_boss_respawns_in_seconds_and_a_spawn_once_never() {
    let mut s = setup(BossSettings::default());
    enter_gm(&mut s);
    cmd(&mut s, "spawn 9002 1 60");
    cmd(&mut s, "spawn_once 9004");
    let permanent = raids(&s, RAID)[0];
    let once = raids(&s, RAID_TWO)[0];
    let before = now_ms();
    assert!(s.world.do_die(permanent, None));
    assert!(s.world.do_die(once, None));
    let respawn = s.world.raid_boss_respawn_time(RAID).unwrap();
    assert!(
        respawn - before >= 43_200 && respawn - before <= 129_600 + 500,
        "{}",
        respawn - before
    );
    // 130 seconds later (2600 ticks): the first is back, the second is not.
    s.log.take();
    for _ in 0..2_650 {
        s.world.tick();
    }
    assert_eq!(raids(&s, RAID).len(), 1);
    assert_ne!(raids(&s, RAID)[0], permanent);
    assert!(raids(&s, RAID_TWO).is_empty());
    assert_eq!(s.world.raid_boss_status(RAID_TWO), RaidBossStatus::Dead);
}

// ------------------------------------------------------------------------------ //delete

/// `//delete` of a raid boss: `RaidBossSpawnManager.deleteSpawn(spawn, true)`: the spawn and its
/// row go.
#[test]
fn delete_of_a_raid_boss_removes_the_spawn_and_its_row() {
    let mut s = setup(writes_on());
    enter_gm(&mut s);
    cmd(&mut s, "spawn 9002");
    let boss = raids(&s, RAID)[0];
    s.log.take();
    s.world.set_target(ObjectId(GM), Some(boss));
    cmd(&mut s, "delete");
    assert_eq!(
        texts(&s.log.take()),
        [format!("Deleted Raid One from {}.", boss.0)]
    );
    assert!(raids(&s, RAID).is_empty());
    assert!(!s.world.raid_boss_defined(RAID));
    assert_eq!(
        boss_writes(&s).last(),
        Some(&BossWrite::RaidBossDelete { boss_id: RAID })
    );
}

// ------------------------------------------------------------------------------ //recall_npc

/// `//recall_npc` of a raid boss: the spawn and its row are replaced by a spawn at the GM with
/// the boss's HP and MP. NOTE(java-quirk): the old boss is not removed from the world.
#[test]
fn recall_npc_of_a_raid_boss_makes_a_new_spawn_at_the_gm() {
    let mut s = setup(writes_on());
    enter_gm(&mut s);
    cmd(&mut s, "spawn 9002");
    let boss = raids(&s, RAID)[0];
    s.world.set_current_hp(boss, 200.0, false);
    // The GM walks away.
    s.world
        .tele_to_location(ObjectId(GM), TeleportTarget::at(X + 400, Y, Z));
    s.persisted.0.lock().unwrap().clear();
    s.world.set_target(ObjectId(GM), Some(boss));
    s.log.take();
    cmd(&mut s, "recall_npc");
    assert!(s.world.raid_boss_defined(RAID));
    let writes = boss_writes(&s);
    assert_eq!(writes.len(), 2, "{writes:?}");
    assert_eq!(writes[0], BossWrite::RaidBossDelete { boss_id: RAID });
    match &writes[1] {
        BossWrite::RaidBossInsert(row) => {
            assert_eq!((row.current_hp, row.current_mp), (200.0, 40.0));
            assert_ne!(row.loc_x, X);
        }
        other => panic!("{other:?}"),
    }
    assert_eq!(raids(&s, RAID).len(), 2, "the old boss stays");
}

// ------------------------------------------------------------------------------ //grandboss

fn baium_row(status: i32, respawn_time: i64) -> GrandBossData {
    GrandBossData {
        boss_id: BAIUM,
        loc_x: 1,
        loc_y: 2,
        loc_z: 3,
        heading: 4,
        respawn_time,
        current_hp: 10.0,
        current_mp: 5.0,
        status,
    }
}

/// `//grandboss` alone: the menu page, as `NpcHtmlMessage(0, 1)`.
#[test]
fn grandboss_without_an_id_shows_the_menu() {
    let mut s = setup(BossSettings::default());
    enter_gm(&mut s);
    s.log.take();
    cmd(&mut s, "grandboss");
    let page = pages(&s.log.take());
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].html, "<html>GRANDBOSS MENU</html>");
    assert_eq!((page[0].npc_obj_id, page[0].item_id), (0, 1));
}

/// `//grandboss <id>` reads the row first (D-50) and shows the page when the answer is in: status
/// text, colour, the respawn time (only while the boss is dead) and the players in the zone.
#[test]
fn grandboss_page_shows_the_state_read_from_the_database() {
    let mut s = setup(BossSettings::default());
    s.world.load_bosses(BossLoad {
        grand_bosses: vec![baium_row(0, 0)],
        ..BossLoad::default()
    });
    enter_gm(&mut s);
    s.log.take();
    s.persisted.0.lock().unwrap().clear();
    cmd(&mut s, "grandboss 29020");
    assert!(pages(&s.log.take()).is_empty(), "the page waits for the read");
    assert_eq!(
        reads(&s),
        [(
            BossReadKind::GrandBoss(BAIUM),
            BossReadThen::ShowGrandBoss {
                gm: ObjectId(GM),
                boss_id: BAIUM
            }
        )]
    );

    // The Java server killed Baium meanwhile: status 3 (dead for Baium).
    let respawn = 1_792_000_000_000;
    s.world.handle(WorldCommand::BossReread {
        result: Ok(BossRereadRows::GrandBoss {
            boss_id: BAIUM,
            row: Some(baium_row(3, respawn)),
        }),
        then: BossReadThen::ShowGrandBoss {
            gm: ObjectId(GM),
            boss_id: BAIUM,
        },
    });
    let page = pages(&s.log.take());
    assert_eq!(page.len(), 1);
    let html = &page[0].html;
    // `yyyy-MM-dd HH:mm:ss` in the machine's zone (the day depends on the zone, the shape does not).
    let date = html
        .strip_prefix("<html><body>Dead FF0000 ")
        .and_then(|rest| rest.strip_suffix(" Zone not found!</body></html>"))
        .unwrap_or_else(|| panic!("{html}"));
    let shape: Vec<char> = date.chars().collect();
    assert_eq!(shape.len(), 19, "{date}");
    for (i, c) in shape.iter().enumerate() {
        match i {
            4 | 7 => assert_eq!(*c, '-', "{date}"),
            10 => assert_eq!(*c, ' ', "{date}"),
            13 | 16 => assert_eq!(*c, ':', "{date}"),
            _ => assert!(c.is_ascii_digit(), "{date}"),
        }
    }
    assert!(date.starts_with("2026-"), "{date}");
    assert_eq!(page[0].item_id, 1);

    // Alive: "Already respawned!".
    s.world.handle(WorldCommand::BossReread {
        result: Ok(BossRereadRows::GrandBoss {
            boss_id: BAIUM,
            row: Some(baium_row(0, respawn)),
        }),
        then: BossReadThen::ShowGrandBoss {
            gm: ObjectId(GM),
            boss_id: BAIUM,
        },
    });
    let page = pages(&s.log.take());
    assert_eq!(
        page[0].html,
        "<html><body>Alive 00FF00 Already respawned! Zone not found!</body></html>"
    );

    // A failed read refuses the action and shows nothing.
    s.world.handle(WorldCommand::BossReread {
        result: Err(()),
        then: BossReadThen::ShowGrandBoss {
            gm: ObjectId(GM),
            boss_id: BAIUM,
        },
    });
    let sent = s.log.take();
    assert!(pages(&sent).is_empty());
    assert_eq!(
        texts(&sent),
        ["The boss state could not be read from the database."]
    );
}

/// The words around the page: the id check, the usage lines, a bad number.
#[test]
fn grandboss_commands_check_their_arguments() {
    let mut s = setup(BossSettings::default());
    enter_gm(&mut s);
    s.log.take();
    for (command, expected) in [
        ("grandboss 5", "Wrong ID!"),
        ("grandboss_skip", "Usage: //grandboss_skip Id"),
        ("grandboss_skip 29020", "Wrong ID!"),
        ("grandboss_respawn", "Usage: //grandboss_respawn Id"),
        ("grandboss_respawn 29001", "Wrong ID!"),
        ("grandboss_minions", "Usage: //grandboss_minions Id"),
        ("grandboss_minions 1", "Wrong ID!"),
        ("grandboss_abort", "Usage: //grandboss_abort Id"),
        ("grandboss_abort 1", "Wrong ID!"),
    ] {
        cmd(&mut s, command);
        assert_eq!(texts(&s.log.take()), [expected], "{command}");
    }
    cmd(&mut s, "grandboss abc");
    let said = texts(&s.log.take());
    assert_eq!(said.len(), 1);
    assert!(said[0].contains("NumberFormatException"), "{said:?}");
    assert!(reads(&s).is_empty());

    // Baium's events go to the script, which is not ported: the GM is told and the page follows.
    cmd(&mut s, "grandboss_respawn 29020");
    let said = texts(&s.log.take());
    assert_eq!(
        said,
        ["The boss scripts are not ported yet; the event was not sent."]
    );
    assert_eq!(reads(&s).len(), 1);
}
