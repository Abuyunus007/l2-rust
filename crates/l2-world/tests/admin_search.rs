#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! `//search <text> [page]` (`admin/search.rs`): the item search of the GM panel, with its pages,
//! its "no items" line and the exceptions of Java's `subList`.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminSearch.java

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::item::ItemTemplates;
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

/// The levels of the shipped `AccessLevels.xml` chain, and `search` at level 30 (GM 100 inherits it).
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
    rights.add_command("search", 30, false);
    rights
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

const GM: i32 = 100;

/// The items: one crystal grade A, a stackable Adena, and twenty "Iron Ring" items (2001 to 2020)
/// for the paging.
fn items_xml() -> String {
    let mut xml = String::from(
        r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /><set name="price" val="1" /></item>
<item id="4" type="Armor" name="Cuirass of Iron"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="crystal_type" val="A" /><set name="weight" val="10" /><set name="price" val="2000" /></item>
"#,
    );
    for id in 2001..=2020 {
        xml.push_str(&format!(
            r#"<item id="{id}" type="EtcItem" name="Iron Ring {id}"><set name="weight" val="1" /><set name="price" val="5" /></item>
"#
        ));
    }
    xml.push_str("</list>");
    xml
}

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
            "data/html/admin/search.htm",
            "<html><body>SEARCH [%items%] [%pages%]</body></html>",
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
    let mut world = World::new(WorldRng::from_seed(2), Box::new(NullOutbox));
    let stats = StatData {
        items: Arc::new(ItemTemplates::from_xml(&items_xml()).unwrap()),
        ..StatData::default()
    };
    world.set_static_data(Box::new(Fake {
        rights: rights(),
        stats,
    }));
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

fn pages(out: &Sent) -> Vec<String> {
    out.iter()
        .filter(|(_, b)| name_of(b) == ServerOpcode::NpcHtmlMessage.name())
        .map(|(_, b)| {
            let skip = if b[0] == 0xFE { 3 } else { 1 };
            NpcHtmlMessage::decode(&mut PacketReader::new(&b[skip..])).unwrap().html
        })
        .collect()
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

#[test]
fn search_alone_shows_the_empty_page() {
    let mut s = setup();
    cmd(&mut s, "search");
    let html = pages(&s.log.take());
    assert_eq!(html, vec!["<html><body>SEARCH [] []</body></html>".to_owned()]);
}

#[test]
fn search_lists_the_matching_items_with_their_add_button() {
    let mut s = setup();
    cmd(&mut s, "search adena");
    let html = pages(&s.log.take());
    assert_eq!(html.len(), 1);
    let text = &html[0];
    assert!(text.contains("admin_create_item 57"), "{text}");
    assert!(text.contains("<font color=\"LEVEL\">Adena</font>"), "{text}");
    assert!(text.contains("color=008000 size=-1>ST"), "stackable is green: {text}");
    assert!(!text.contains("%items%") && !text.contains("%pages%"), "{text}");
}

#[test]
fn a_crystal_grade_is_written_with_java_markup() {
    let mut s = setup();
    cmd(&mut s, "search Cuirass");
    let html = pages(&s.log.take());
    // The grade of crystal A, with the backslash-escaped quotes Java writes.
    assert!(
        html[0].contains("<font name=\\\"hs8\\\" color=ae9977 size=-1>A</font>"),
        "{}",
        html[0]
    );
    assert!(html[0].contains("color=ff0000 size=-1>ST"), "not stackable: {}", html[0]);
}

#[test]
fn an_unknown_word_says_so() {
    let mut s = setup();
    cmd(&mut s, "search nothing");
    let html = pages(&s.log.take());
    assert!(html[0].contains("No items found with word nothing."), "{}", html[0]);
}

#[test]
fn many_matches_are_paged_fifteen_a_page() {
    let mut s = setup();
    // Twenty "Iron Ring" items and "Cuirass of Iron": 21 matches, two pages.
    cmd(&mut s, "search Iron");
    let first = pages(&s.log.take());
    assert_eq!(first.len(), 1);
    let rows = first[0].matches("admin_create_item").count();
    assert_eq!(rows, 15, "the first page has fifteen items");
    assert!(first[0].contains("admin_search Iron 2"), "the link to page 2: {}", first[0]);
    assert!(first[0].contains("1&nbsp;"), "the current page is plain: {}", first[0]);

    cmd(&mut s, "search Iron 2");
    let second = pages(&s.log.take());
    assert_eq!(second[0].matches("admin_create_item").count(), 6, "the last six items");
    assert!(second[0].contains("admin_search Iron 1"));
}

#[test]
fn a_page_before_the_first_is_a_java_exception() {
    let mut s = setup();
    cmd(&mut s, "search Iron 0");
    assert_eq!(
        texts(&s.log.take()),
        vec!["Exception during execution of  'admin_search Iron 0': java.lang.IndexOutOfBoundsException: fromIndex = -15".to_owned()]
    );
}

#[test]
fn a_page_past_the_end_is_a_java_exception() {
    let mut s = setup();
    cmd(&mut s, "search Iron 9");
    assert_eq!(
        texts(&s.log.take()),
        vec!["Exception during execution of  'admin_search Iron 9': java.lang.IllegalArgumentException: fromIndex(120) > toIndex(21)".to_owned()]
    );
}

#[test]
fn a_page_that_is_not_a_number_is_the_first() {
    let mut s = setup();
    cmd(&mut s, "search Iron x");
    let html = pages(&s.log.take());
    assert_eq!(html[0].matches("admin_create_item").count(), 15);
}
