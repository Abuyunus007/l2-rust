#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The NPC admin commands of this card: `//delete [range]` and `//delete_group` (`admin/delete.rs`).
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminDelete.java

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

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}

    fn ls_event(&mut self, _event: LsEvent) {}
}

/// Levels 0, 70 and 100 (a GM), and the rights of the words of this file (level 100).
fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    // The shipped `AccessLevels.xml` chain: 100 inherits 70, 70 inherits 60 ... down to 30.
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
    for command in ["spawn", "delete"] {
        rights.add_command(command, 100, false);
    }
    // `delete_group` and `deleteNpcByObjectId` have `confirmDlg="true"` in the shipped
    // `AdminCommands.xml`; `scan` is level 30.
    rights.add_command("delete_group", 100, true);
    rights.add_command("deleteNpcByObjectId", 100, true);
    rights.add_command("scan", 30, false);
    rights
}

/// The class template of the test characters (class 0), as `admin.rs` uses.
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
const RAT: i32 = 2000;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="2000" type="Monster" level="3" name="Rat">
    <stats><vitals hp="62" mp="10"/></stats>
  </npc>
</list>"#;

struct Root(tempfile::TempDir);

impl Root {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("data/spawns/none.xml");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, r#"<?xml version="1.0"?><list enabled="true"></list>"#).unwrap();
        // The scan page: its placeholders are what the handler fills.
        let page = dir.path().join("data/html/admin/scan.htm");
        std::fs::create_dir_all(page.parent().unwrap()).unwrap();
        std::fs::write(page, "<html><body>SCAN %pages% %data%</body></html>").unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        self.0.path()
    }
}

struct Setup {
    world: World,
    log: Log,
    _root: Root,
}

fn setup() -> Setup {
    let root = Root::new();
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(9), Box::new(NullOutbox));
    world.set_static_data(Box::new(Fake {
        rights: rights(),
        stats: class_data(),
    }));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    Setup {
        world,
        log,
        _root: root,
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

/// The NPCs of the rat template, in object id order.
fn rats(s: &Setup) -> Vec<ObjectId> {
    s.world
        .npc_ids()
        .into_iter()
        .filter(|id| s.world.npc(*id).is_some_and(|n| n.npc_id == RAT))
        .collect()
}

/// `//spawn 2000` at the GM's place: one rat for the target to point at.
fn spawn_rat(s: &mut Setup) -> ObjectId {
    cmd(s, "spawn 2000");
    let rat = *rats(s).last().expect("the rat is spawned");
    rat
}

#[test]
fn delete_removes_the_target_npc_and_says_so() {
    let mut s = setup();
    enter_gm(&mut s);
    let rat = spawn_rat(&mut s);
    s.log.take();
    s.world.set_target(ObjectId(GM), Some(rat));
    cmd(&mut s, "delete");
    assert!(rats(&s).is_empty(), "the rat is gone");
    assert_eq!(
        texts(&s.log.take()),
        vec![format!("Deleted Rat from {}.", rat.0)]
    );
}

#[test]
fn delete_without_a_target_is_the_incorrect_target() {
    let mut s = setup();
    enter_gm(&mut s);
    s.log.take();
    cmd(&mut s, "delete");
    assert_eq!(texts(&s.log.take()), vec!["Incorrect target.".to_owned()]);
}

#[test]
fn delete_with_a_range_removes_the_npcs_near_the_gm() {
    let mut s = setup();
    enter_gm(&mut s);
    spawn_rat(&mut s);
    spawn_rat(&mut s);
    assert_eq!(rats(&s).len(), 2);
    cmd(&mut s, "delete 500");
    assert!(rats(&s).is_empty(), "both rats are in the range");
}

#[test]
fn delete_group_of_an_npc_without_a_territory_is_the_incorrect_target() {
    let mut s = setup();
    enter_gm(&mut s);
    let rat = spawn_rat(&mut s);
    s.log.take();
    s.world.set_target(ObjectId(GM), Some(rat));
    // `delete_group` asks first (`confirmDlg="true"`): the typed command only shows the dialog.
    cmd(&mut s, "delete_group");
    assert!(texts(&s.log.take()).is_empty(), "the dialog comes first");
    s.world.admin_command(ObjectId(GM), "admin_delete_group", false);
    assert_eq!(texts(&s.log.take()), vec!["Incorrect target.".to_owned()]);
    assert_eq!(rats(&s).len(), 1, "the rat stays");
}

// ----------------------------------------------------------------------------- //scan

fn pages(out: &Sent) -> Vec<String> {
    out.iter()
        .filter(|(_, b)| name_of(b) == ServerOpcode::NpcHtmlMessage.name())
        .map(|(_, b)| {
            let skip = if b[0] == 0xFE { 3 } else { 1 };
            NpcHtmlMessage::decode(&mut PacketReader::new(&b[skip..])).unwrap().html
        })
        .collect()
}

#[test]
fn scan_lists_the_npcs_around_with_their_delete_links() {
    let mut s = setup();
    enter_gm(&mut s);
    let rat = spawn_rat(&mut s);
    s.log.take();
    cmd(&mut s, "scan");
    let raw = s.log.take();
    let html = pages(&raw);
    assert_eq!(html.len(), 1);
    assert!(html[0].starts_with("<html><body>SCAN "), "{}", html[0]);
    assert!(html[0].contains("<td width=\"45\">2000</td>"), "the template id: {}", html[0]);
    assert!(html[0].contains("Rat"), "{}", html[0]);
    assert!(
        html[0].contains(&format!("admin_deleteNpcByObjectId objectId={}", rat.0)),
        "the delete link: {}",
        html[0]
    );
    assert!(!html[0].contains("%data%"), "every placeholder is replaced");
}

#[test]
fn scan_filters_by_template_id_and_by_name() {
    let mut s = setup();
    enter_gm(&mut s);
    spawn_rat(&mut s);
    s.log.take();
    cmd(&mut s, "scan id=1001");
    let html = pages(&s.log.take());
    assert!(!html[0].contains("admin_move_to"), "no NPC has id 1001: {}", html[0]);
    cmd(&mut s, "scan name=ra");
    let html = pages(&s.log.take());
    assert!(html[0].contains("admin_move_to"), "the rat starts with ra: {}", html[0]);
    cmd(&mut s, "scan name=wolf");
    let html = pages(&s.log.take());
    assert!(!html[0].contains("admin_move_to"), "{}", html[0]);
}

#[test]
fn delete_npc_by_object_id_removes_the_npc() {
    let mut s = setup();
    enter_gm(&mut s);
    let rat = spawn_rat(&mut s);
    s.log.take();
    // The typed word asks first; the answer (here `admin_command` without the question) runs it.
    cmd(&mut s, &format!("deleteNpcByObjectId objectId={}", rat.0));
    assert_eq!(rats(&s).len(), 1, "the dialog comes first");
    s.world.admin_command(
        ObjectId(GM),
        &format!("admin_deleteNpcByObjectId objectId={}", rat.0),
        false,
    );
    assert!(rats(&s).is_empty());
    assert_eq!(texts(&s.log.take()), vec!["Rat has been deleted.".to_owned()]);
}

#[test]
fn delete_npc_by_object_id_refuses_a_bad_id() {
    let mut s = setup();
    enter_gm(&mut s);
    s.log.take();
    // The confirmed word: the answer runs it (here `admin_command` with the confirmation off).
    s.world.admin_command(ObjectId(GM), "admin_deleteNpcByObjectId", false);
    s.world
        .admin_command(ObjectId(GM), "admin_deleteNpcByObjectId objectId=0", false);
    s.world
        .admin_command(ObjectId(GM), "admin_deleteNpcByObjectId objectId=999999", false);
    assert_eq!(
        texts(&s.log.take()),
        vec![
            "Usage: //deletenpcbyobjectid objectId=<object_id>".to_owned(),
            "objectId is not set or invalid!".to_owned(),
            "NPC does not exist or object_id does not belong to an NPC".to_owned(),
        ]
    );
}
