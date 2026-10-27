#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The registry of the M7-ADMIN-09 handlers: every word the card ports is known to the framework
//! (it never answers "does not exist"), and every word the card leaves out answers like a command
//! without a handler (Java's "The command '...' does not exist!"). The right of each word is not
//! checked here: an unlisted word of the highest level asks first, which is not "does not exist".

use std::sync::{Arc, Mutex};

use l2_data::NpcTemplates;
use l2_data::{NpcLoadOptions, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
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

/// Words this card registers (their handlers are in `crates/l2-world/src/admin/`).
const PORTED: &[&str] = &[
    "tracert",
    "strict_find_dualbox",
    "setcolor",
    "changelvl",
    "repair",
    "restore",
    "gmliston",
    "gmlistoff",
    "silence",
    "diet",
    "tradeoff",
    "setconfig",
    "config_server",
    // edit_char
    "current_player",
    "character_info",
    "character_list",
    "show_characters",
    "find_character",
    "find_ip",
    "find_account",
    "edit_character",
    "find_dualbox",
    "setfame",
    "rec",
    "setclass",
    "settitle",
    "changename",
    "setsex",
    "settcolor",
    "fullfood",
    "setnoble",
    "set_hp",
    "set_mp",
    "set_cp",
    // door
    "open",
    "close",
    "openall",
    "closeall",
    "showdoors",
    // destroy_items and gm_shop
    "destroy_items",
    "destroy_all_items",
    "destroyitems",
    "destroyallitems",
    "gmshop",
    "buy",
    // delete and scan
    "delete",
    "delete_group",
    "scan",
    "deleteNpcByObjectId",
    // search, fence, skill_test, admin_page
    "search",
    "addfence",
    "setfencestate",
    "removefence",
    "listfence",
    "gofence",
    "skill_test",
    "set",
    "set_mod",
    "gmon",
    // spawn_admin
    "spawnday",
    "spawnnight",
    "show_npcs",
    "npc_index",
    "spawn_index",
    "circlespawn",
    "topspawncount",
    "top_spawn_count",
    "unspawnall",
    "spawn_debug_menu",
    "spawn_debug_print",
    "spawn_debug_print_menu",
    // L-08: the fight calculator and the rest of the spawn words
    "fight_calculator",
    "fight_calculator_show",
    "fcs",
    "respawnall",
    "spawn_reload",
    "spawn_misc",
    "spawnmisc",
    "list_pools",
];

/// Words of the card's Java files that this card does not port (see the module notes): they
/// answer like a command without a handler. Olympiad, heroes and instances are M8.
const LEFT_OUT: &[&str] = &[
    // edit_char
    // change access level and repair
    // admin page flags and configuration
    // M8: Olympiad, heroes, instances
    "instance_spawns",
    "checkolympiad",
    "saveolymp",
    "sethero",
    "givehero",
    "endolympiad",
    // the coliseum doors are not in the list of AdminDoorControl
    "open1",
    "close1",
];

fn setup() -> (World, Log) {
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(8), Box::new(NullOutbox));
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (70, 0), (100, 70)] {
        rights.add_level(level, child);
    }
    world.set_static_data(Box::new(Fake { rights }));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(r#"<list></list>"#, &NpcLoadOptions::default()).unwrap());
    let dir = tempfile::tempdir().unwrap();
    let spawns = Arc::new(SpawnData::load(dir.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
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

/// The texts of the system messages with one string (what `sendMessage` sends) to the GM.
fn texts(out: &Sent) -> Vec<String> {
    out.iter()
        .filter(|(_, b)| {
            let ex = (b[0] == 0xFE).then(|| u16::from_le_bytes([b[1], b[2]]));
            ServerOpcode::from_id(b[0], ex) == Some(ServerOpcode::SystemMessage)
        })
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

fn send(world: &mut World, command: &str) {
    world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(GM).unwrap()),
        input: ClientInput::BuildCmd {
            command: command.to_owned(),
        },
    });
}

#[test]
fn every_ported_word_is_known_to_the_framework() {
    let (mut world, log) = setup();
    for word in PORTED {
        send(&mut world, word);
        let texts = texts(&log.take());
        assert!(
            texts.iter().all(|t| !t.starts_with("The command")),
            "{word} must be registered, got {texts:?}"
        );
    }
}

#[test]
fn every_left_out_word_answers_like_an_unknown_command() {
    let (mut world, log) = setup();
    for word in LEFT_OUT {
        send(&mut world, word);
        assert_eq!(
            texts(&log.take()),
            vec![format!("The command '{word}' does not exist!")],
            "{word} is left out"
        );
    }
}

/// The card's list of words and the registered handlers stay in step: the two lists above have no
/// word in common.
#[test]
fn the_lists_do_not_overlap() {
    for word in LEFT_OUT {
        assert!(!PORTED.contains(word), "{word} is in both lists");
    }
}
