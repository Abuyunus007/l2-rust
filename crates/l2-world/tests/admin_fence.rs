#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The fence admin commands (`admin/fence.rs`): `//addfence`, `//setfencestate`, `//removefence`,
//! `//listfence` and `//gofence`, with a GM at a known place.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminFence.java

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{FenceState, HtmCache, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
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

/// The chain of the shipped `AccessLevels.xml` and the fence words at level 100.
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
    for command in ["addfence", "setfencestate", "removefence", "listfence", "gofence"] {
        rights.add_command(command, 100, false);
    }
    rights
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
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

struct Root(tempfile::TempDir);

impl Root {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = dir.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        write(
            "data/html/admin/fences.htm",
            "<html><body>FENCES %pages% %fences%</body></html>",
        );
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
    let mut world = World::new(WorldRng::from_seed(4), Box::new(NullOutbox));
    world.set_static_data(Box::new(Fake { rights: rights() }));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(r#"<list></list>"#, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
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
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(GM).unwrap()),
        player: Box::new(data),
        sink: Box::new(Sink(GM, log.clone())),
        ip: None,
    });
    log.take();
    Setup {
        world,
        log,
        _root: root,
    }
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

fn pages(out: &Sent) -> Vec<String> {
    out.iter()
        .filter(|(_, b)| name_of(b) == ServerOpcode::NpcHtmlMessage.name())
        .map(|(_, b)| {
            let skip = if b[0] == 0xFE { 3 } else { 1 };
            NpcHtmlMessage::decode(&mut PacketReader::new(&b[skip..]))
                .unwrap()
                .html
        })
        .collect()
}

/// The id of the one fence in the world.
fn only_fence(s: &Setup) -> i32 {
    let ids: Vec<i32> = s.world.fences().map(|f| f.object_id.0).collect();
    assert_eq!(ids.len(), 1, "one fence");
    ids[0]
}

#[test]
fn addfence_makes_a_closed_fence_where_the_gm_stands() {
    let mut s = setup();
    cmd(&mut s, "addfence 200 300 2");
    assert_eq!(texts(&s.log.take()), vec!["Fence added successfully.".to_owned()]);
    let fence = s.world.fences().next().unwrap().clone();
    assert_eq!((fence.x, fence.y, fence.z), (X, Y, Z));
    assert_eq!((fence.width, fence.length), (200, 300));
    assert_eq!(fence.state, FenceState::Closed);
    assert_eq!(
        fence.height_fences.len(),
        1,
        "a height of two has one extra piece"
    );
}

#[test]
fn addfence_refuses_bad_numbers() {
    let mut s = setup();
    cmd(&mut s, "addfence 0 300 1");
    cmd(&mut s, "addfence 200 300 4");
    cmd(&mut s, "addfence 200 300");
    cmd(&mut s, "addfence a b c");
    assert_eq!(
        texts(&s.log.take()),
        vec![
            "Width and length values must be positive numbers.".to_owned(),
            "The range for height can only be 1-3.".to_owned(),
            "Format must be: //addfence <width> <length> <height>".to_owned(),
            "Format must be: //addfence <width> <length> <height>".to_owned(),
        ]
    );
    assert_eq!(s.world.fences().count(), 0);
}

#[test]
fn listfence_shows_the_fences_with_their_buttons() {
    let mut s = setup();
    cmd(&mut s, "addfence 200 300 1");
    s.log.take();
    cmd(&mut s, "listfence");
    let html = pages(&s.log.take());
    assert_eq!(html.len(), 1);
    let id = only_fence(&s);
    assert!(
        html[0].contains(&format!("<td>{id}</td>")),
        "a fence without a name shows its id: {}",
        html[0]
    );
    assert!(html[0].contains(&format!("admin_gofence {id}")), "{}", html[0]);
    assert!(
        html[0].contains(&format!("admin_setfencestate {id} 2")),
        "{}",
        html[0]
    );
    assert!(
        html[0].contains(&format!("admin_removefence {id}")),
        "{}",
        html[0]
    );
}

#[test]
fn listfence_with_a_bad_page_is_an_exception() {
    let mut s = setup();
    cmd(&mut s, "listfence abc");
    assert_eq!(
        texts(&s.log.take()),
        vec!["Exception during execution of  'admin_listfence abc': java.lang.NumberFormatException: For input string: \"abc\"".to_owned()]
    );
}

#[test]
fn setfencestate_changes_the_state_and_says_so() {
    let mut s = setup();
    cmd(&mut s, "addfence 200 300 1");
    s.log.take();
    let id = only_fence(&s);
    cmd(&mut s, &format!("setfencestate {id} 1"));
    assert_eq!(s.world.fences().next().unwrap().state, FenceState::Opened);
    assert_eq!(
        texts(&s.log.take()),
        vec![format!("Fence null[{id}]'s state has been changed to OPENED")]
    );
}

#[test]
fn setfencestate_refuses_a_state_out_of_range_and_a_non_fence() {
    let mut s = setup();
    cmd(&mut s, "addfence 200 300 1");
    s.log.take();
    let id = only_fence(&s);
    cmd(&mut s, &format!("setfencestate {id} 7"));
    cmd(&mut s, "setfencestate 999999 1");
    cmd(&mut s, "setfencestate");
    assert_eq!(
        texts(&s.log.take()),
        vec![
            "Specified FenceType is out of range. Only 0-3 are permitted.".to_owned(),
            "Target is not a fence.".to_owned(),
            "Format mustr be: //setfencestate <fenceObjectId> <fenceState>".to_owned(),
        ]
    );
}

#[test]
fn removefence_removes_it_and_shows_the_list() {
    let mut s = setup();
    cmd(&mut s, "addfence 200 300 1");
    s.log.take();
    let id = only_fence(&s);
    cmd(&mut s, &format!("removefence {id}"));
    assert_eq!(s.world.fences().count(), 0);
    let out = s.log.take();
    assert_eq!(texts(&out), vec!["Fence removed successfully.".to_owned()]);
    assert_eq!(pages(&out).len(), 1, "the list follows");
}

#[test]
fn removefence_of_a_non_fence_says_so() {
    let mut s = setup();
    cmd(&mut s, "removefence 999999");
    let out = s.log.take();
    assert_eq!(texts(&out), vec!["Target is not a fence.".to_owned()]);
}

#[test]
fn gofence_sends_the_gm_to_the_fence() {
    let mut s = setup();
    cmd(&mut s, "addfence 200 300 1");
    s.log.take();
    let id = only_fence(&s);
    cmd(&mut s, &format!("gofence {id}"));
    let out = s.log.take();
    assert!(
        out.iter()
            .any(|(_, b)| name_of(b) == ServerOpcode::TeleportToLocation.name()),
        "the GM is teleported"
    );
}
