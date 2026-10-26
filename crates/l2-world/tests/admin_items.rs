#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The GM's own inventory admin commands: `//destroy_items`, `//destroy_all_items` (`admin/
//! destroy_items.rs`), and the GM shop words `//gmshop` and `//buy` (`admin/gm_shop.rs`).
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminDestroyItems,AdminShop}.java

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{HtmCache, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{BuyList, InventoryUpdate, NpcHtmlMessage, SystemMessage, SystemMessageParam};
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

/// Levels 0 and 100 (a GM), and the rights of the words of this file (level 100 in the shipped
/// `AdminCommands.xml`).
fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (70, 0), (100, 70)] {
        rights.add_level(level, child);
    }
    for command in [
        "destroy_items",
        "destroy_all_items",
        "destroyitems",
        "destroyallitems",
        "gmshop",
        "buy",
    ] {
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
            "data/html/admin/gmshops.htm",
            "<html><body>GM shops page</body></html>",
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
    let mut world = World::new(WorldRng::from_seed(5), Box::new(NullOutbox));
    world.set_static_data(Box::new(Fake { rights: rights() }));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(r#"<list></list>"#, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    Setup {
        world,
        log,
        _root: root,
    }
}

/// The GM with a worn item (object 201, paperdoll slot 0), two items in the inventory (202, 203)
/// and one in the warehouse (204, not the inventory).
fn enter_gm(s: &mut Setup) {
    let item = |object_id: i32, loc: &str, loc_data: i32| ItemData {
        object_id: ObjectId(object_id),
        item_id: 1000 + object_id,
        count: 1,
        loc: loc.to_owned(),
        loc_data,
        ..ItemData::default()
    };
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
        items: vec![
            item(201, "PAPERDOLL", 0),
            item(202, "INVENTORY", 0),
            item(203, "INVENTORY", 0),
            item(204, "WAREHOUSE", 0),
        ],
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

fn skip(body: &[u8]) -> usize {
    if body[0] == 0xFE { 3 } else { 1 }
}

fn texts(out: &Sent) -> Vec<String> {
    out.iter()
        .filter(|(_, b)| name_of(b) == ServerOpcode::SystemMessage.name())
        .filter_map(|(_, b)| {
            let m = SystemMessage::decode(&mut PacketReader::new(&b[skip(b)..])).ok()?;
            match m.params.as_slice() {
                [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
                _ => None,
            }
        })
        .collect()
}

fn names(out: &Sent, name: &str) -> usize {
    out.iter().filter(|(_, b)| name_of(b) == name).count()
}

fn item_ids(s: &Setup) -> Vec<i32> {
    let mut ids: Vec<i32> = s
        .world
        .player(ObjectId(GM))
        .unwrap()
        .data
        .items
        .iter()
        .map(|i| i.object_id.0)
        .collect();
    ids.sort_unstable();
    ids
}

#[test]
fn destroy_items_keeps_the_worn_ones_and_the_warehouse() {
    let mut s = setup();
    enter_gm(&mut s);
    s.log.take();
    cmd(&mut s, "destroy_items");
    assert_eq!(
        item_ids(&s),
        vec![201, 204],
        "the inventory goes, the worn item and the warehouse stay"
    );
    let out = s.log.take();
    assert_eq!(
        names(&out, ServerOpcode::InventoryUpdate.name()),
        1,
        "one inventory update"
    );
    let update = out
        .iter()
        .find(|(_, b)| name_of(b) == ServerOpcode::InventoryUpdate.name())
        .map(|(_, b)| InventoryUpdate::decode(&mut PacketReader::new(&b[skip(b)..])).unwrap())
        .unwrap();
    let mut removed: Vec<i32> = update.updates.iter().map(|u| u.item.object_id).collect();
    removed.sort_unstable();
    assert_eq!(removed, vec![202, 203]);
}

#[test]
fn destroy_all_items_takes_the_worn_ones_too() {
    let mut s = setup();
    enter_gm(&mut s);
    s.log.take();
    cmd(&mut s, "destroy_all_items");
    assert_eq!(
        item_ids(&s),
        vec![204],
        "the warehouse item is not in the inventory"
    );
}

#[test]
fn the_destroyallitems_spelling_is_the_same_command() {
    let mut s = setup();
    enter_gm(&mut s);
    cmd(&mut s, "destroyallitems");
    assert_eq!(item_ids(&s), vec![204]);
}

#[test]
fn gmshop_shows_the_page() {
    let mut s = setup();
    enter_gm(&mut s);
    s.log.take();
    cmd(&mut s, "gmshop");
    let out = s.log.take();
    let pages: Vec<NpcHtmlMessage> = out
        .iter()
        .filter(|(_, b)| name_of(b) == ServerOpcode::NpcHtmlMessage.name())
        .map(|(_, b)| NpcHtmlMessage::decode(&mut PacketReader::new(&b[skip(b)..])).unwrap())
        .collect();
    assert_eq!(pages.len(), 1);
    assert!(pages[0].html.contains("GM shops page"), "{}", pages[0].html);
}

#[test]
fn buy_without_a_list_number_asks_for_one() {
    let mut s = setup();
    enter_gm(&mut s);
    s.log.take();
    cmd(&mut s, "buy");
    assert_eq!(texts(&s.log.take()), vec!["Please specify buylist.".to_owned()]);
}

#[test]
fn buy_of_an_unknown_list_sends_only_action_failed() {
    let mut s = setup();
    enter_gm(&mut s);
    s.log.take();
    cmd(&mut s, "buy 77");
    let out = s.log.take();
    assert_eq!(names(&out, BuyList::OPCODE.name()), 0, "no list 77");
    assert_eq!(names(&out, ServerOpcode::ActionFail.name()), 1);
}
