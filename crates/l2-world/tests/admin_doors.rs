#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The door admin commands (`admin/door.rs`): `//open`, `//close`, `//openall`, `//closeall` and
//! `//showdoors` with a GM and two doors in view.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminDoorControl.java

use std::sync::{Arc, Mutex};

use l2_data::DoorTemplates;
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::debug_draw::PrimitiveColor;
use l2_protocol_game::server::{ExServerPrimitive, SystemMessage, SystemMessageParam};
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

/// The rights of the door words (level 100, as the shipped `AdminCommands.xml`).
fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (70, 0), (100, 70)] {
        rights.add_level(level, child);
    }
    for command in ["open", "close", "openall", "closeall", "showdoors"] {
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

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const GM: i32 = 100;
const PLAYER: i32 = 1;

/// A wall 20 wide (x) and 100 long (y) around (X, Y), 150 high from `Z - 10`.
fn door_xml(id: i32) -> String {
    format!(
        r#"<door id="{id}" name="door{id}" height="150" baseHpMax="1000" pos="{X},{Y},{Z}"
            node1="{},{}" node2="{},{}" node3="{},{}" node4="{},{}" nodeZ="{}"/>"#,
        X - 10,
        Y - 50,
        X + 10,
        Y - 50,
        X + 10,
        Y + 50,
        X - 10,
        Y + 50,
        Z - 10
    )
}

struct Setup {
    world: World,
    log: Log,
}

/// Two doors (ids 10 and 11) at the same place, so one GM sees both.
fn setup() -> Setup {
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(3), Box::new(NullOutbox));
    world.set_static_data(Box::new(Fake { rights: rights() }));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let xml = format!("<list>{}{}</list>", door_xml(10), door_xml(11));
    let templates = DoorTemplates::from_xml(&xml).unwrap();
    world
        .spawn_doors(&templates, &IdFactory::new(Vec::<i32>::new()).unwrap())
        .unwrap();
    Setup { world, log }
}

fn enter(s: &mut Setup, id: i32, name: &str, access_level: i32) {
    let data = PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: name.to_owned(),
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
        access_level,
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
        session: SessionId(u64::try_from(id).unwrap()),
        player: Box::new(data),
        sink: Box::new(Sink(id, s.log.clone())),
        ip: None,
    });
}

fn cmd(s: &mut Setup, player: i32, command: &str) {
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(player).unwrap()),
        input: ClientInput::BuildCmd {
            command: command.to_owned(),
        },
    });
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == ServerOpcode::SystemMessage.name())
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

fn primitives_to(out: &Sent, to: i32) -> Vec<ExServerPrimitive> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == ServerOpcode::ExServerPrimitive.name())
        .map(|(_, b)| {
            let skip = if b[0] == 0xFE { 3 } else { 1 };
            ExServerPrimitive::decode(&mut PacketReader::new(&b[skip..])).unwrap()
        })
        .collect()
}

fn door_open(s: &Setup, door_id: i32) -> bool {
    s.world.door_by_door_id(door_id).unwrap().open
}

#[test]
fn open_and_close_by_id_change_the_door() {
    let mut s = setup();
    enter(&mut s, GM, "Gamemaster", 100);
    s.log.take();
    cmd(&mut s, GM, "open 10");
    assert!(door_open(&s, 10));
    assert!(!door_open(&s, 11), "only the named door opens");
    cmd(&mut s, GM, "close 10");
    assert!(!door_open(&s, 10));
}

#[test]
fn open_without_a_target_is_incorrect_target() {
    let mut s = setup();
    enter(&mut s, GM, "Gamemaster", 100);
    s.log.take();
    cmd(&mut s, GM, "open");
    assert_eq!(texts_to(&s.log.take(), GM), vec!["Incorrect target.".to_owned()]);
}

#[test]
fn open_and_close_the_target_door() {
    let mut s = setup();
    enter(&mut s, GM, "Gamemaster", 100);
    let door = s.world.door_by_door_id(11).unwrap().object_id;
    s.world.set_target(ObjectId(GM), Some(door));
    cmd(&mut s, GM, "open");
    assert!(door_open(&s, 11));
    cmd(&mut s, GM, "close");
    assert!(!door_open(&s, 11));
}

#[test]
fn openall_and_closeall_change_every_door() {
    let mut s = setup();
    enter(&mut s, GM, "Gamemaster", 100);
    cmd(&mut s, GM, "openall");
    assert!(door_open(&s, 10) && door_open(&s, 11));
    cmd(&mut s, GM, "closeall");
    assert!(!door_open(&s, 10) && !door_open(&s, 11));
}

#[test]
fn a_plain_player_cannot_open_doors() {
    let mut s = setup();
    enter(&mut s, PLAYER, "Plain", 0);
    s.log.take();
    cmd(&mut s, PLAYER, "openall");
    assert!(!door_open(&s, 10));
}

#[test]
fn showdoors_draws_the_doors_in_view_and_off_erases_them() {
    let mut s = setup();
    enter(&mut s, GM, "Gamemaster", 100);
    s.log.take();
    cmd(&mut s, GM, "showdoors");
    let out = s.log.take();
    let texts = texts_to(&out, GM);
    assert!(texts.contains(&"Found door 10".to_owned()), "{texts:?}");
    assert!(texts.contains(&"Found door 11".to_owned()), "{texts:?}");
    let drawn = primitives_to(&out, GM);
    assert_eq!(drawn.len(), 2, "one primitive per door");
    assert!(drawn.iter().any(|p| p.name == "Door10"));
    // Each door is 12 lines (the box, its two boxes' edges and the diagonals).
    assert!(drawn.iter().all(|p| p.lines().len() == 12));

    // A second `showdoors` draws nothing new: the doors are already shown.
    cmd(&mut s, GM, "showdoors");
    let again = s.log.take();
    assert!(primitives_to(&again, GM).is_empty());

    cmd(&mut s, GM, "showdoors off");
    let erased = primitives_to(&s.log.take(), GM);
    assert_eq!(erased.len(), 2, "every shown door is erased");
    assert!(
        erased
            .iter()
            .all(|p| p.lines().len() == 1 && p.lines()[0].color == PrimitiveColor::BLACK)
    );
}

#[test]
fn showdoors_off_without_shown_doors_sends_nothing() {
    let mut s = setup();
    enter(&mut s, GM, "Gamemaster", 100);
    s.log.take();
    cmd(&mut s, GM, "showdoors off");
    assert!(primitives_to(&s.log.take(), GM).is_empty());
}
