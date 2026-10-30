#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! S-API-05: `//gracia_seeds` (Java `AdminGraciaSeeds`) and the `.hellbound` voiced command
//! (Java `handlers.chat.commands.voiced.Hellbound`), with a fake GM and stand-in manager scripts.

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

/// The events the stand-in managers heard.
static EVENTS: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// A stand-in of one seed manager (or of the Hellbound engine): it logs its events.
struct Manager(&'static str);

impl Script for Manager {
    fn name(&self) -> &'static str {
        self.0
    }
    fn path(&self) -> &'static str {
        "ai/areas/Gracia"
    }
    fn setup(&self, _s: &mut ScriptSetup) {}
    fn on_event(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        EVENTS.lock().unwrap().push(format!("{} {event}", self.0));
        None
    }
}

const GM: i32 = 100;

const PAGE: &str = "<html><body>sod %sodstage% tiat %sodtiatkill% time %sodtime% soi %soistage% \
    twin %twinkills% coh %cohemeneskills% eki %ekimuskills% soitime %soitime%</body></html>";

struct Setup {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

fn setup(scripts: &[&'static str]) -> Setup {
    let root = tempfile::tempdir().unwrap();
    let write = |path: &str, text: &str| {
        let file = root.path().join(path);
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, text).unwrap();
    };
    write(
        "data/spawns/none.xml",
        r#"<?xml version="1.0"?><list enabled="true"></list>"#,
    );
    write("data/html/admin/graciaseeds.htm", PAGE);
    let mut rights = AdminRights::default();
    rights.add_level(0, 0);
    rights.add_level(100, 0);
    for command in ["gracia_seeds", "kill_tiat", "set_sodstate", "set_soistage"] {
        rights.add_command(command, 100, false);
    }
    let mut world = World::new(WorldRng::from_seed(5), Box::new(NullOutbox));
    world.set_static_data(Box::new(Fake {
        rights,
        stats: StatData::default(),
    }));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml("<list></list>", &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    let mut registry = ScriptRegistry::new();
    for name in scripts {
        registry.add(Manager(name));
    }
    world.install_scripts(registry);
    let log = Log::default();
    let mut s = Setup {
        world,
        log,
        _root: root,
    };
    for (id, level) in [(GM, 100), (1, 0)] {
        let data = PlayerData {
            object_id: ObjectId(id),
            account: format!("acc{id}"),
            name: format!("P{id}"),
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
            access_level: level,
            x: -71_338,
            y: 258_271,
            z: -3_104,
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
    s.log.take();
    s
}

impl Setup {
    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    fn cmd(&mut self, command: &str) -> Vec<String> {
        self.input(
            GM,
            ClientInput::BuildCmd {
                command: command.to_owned(),
            },
        );
        self.log
            .take()
            .iter()
            .filter(|(to, b)| *to == GM && name_of(b) == "NPC_HTML_MESSAGE")
            .map(|(_, b)| decode::<NpcHtmlMessage>(b).html)
            .collect()
    }

    fn say(&mut self, player: i32, text: &str) -> Vec<String> {
        self.input(
            player,
            ClientInput::Say {
                chat_type: 0,
                text: text.to_owned(),
                target: None,
            },
        );
        self.log
            .take()
            .iter()
            .filter(|(to, b)| *to == player && name_of(b) == "SYSTEM_MESSAGE")
            .filter_map(|(_, b)| {
                let m = decode::<SystemMessage>(b);
                match m.params.as_slice() {
                    [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
                    _ => None,
                }
            })
            .collect()
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn take_events() -> Vec<String> {
    std::mem::take(&mut EVENTS.lock().unwrap())
}

#[test]
fn gracia_seeds_shows_the_menu_and_hands_the_words_to_the_managers() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    take_events();
    let mut s = setup(&["SeedOfDestructionManager", "SeedOfInfinityManager"]);
    let pages = s.cmd("gracia_seeds");
    assert_eq!(
        pages,
        ["<html><body>sod 1 tiat 0 time -1 soi 1 twin N/A coh 0 eki 0 soitime N/A</body></html>"]
    );
    assert!(take_events().is_empty());

    s.cmd("kill_tiat");
    s.cmd("set_sodstate 2");
    s.cmd("set_soistage 4");
    assert_eq!(
        take_events(),
        [
            "SeedOfDestructionManager increaseSoDTiatKilled",
            "SeedOfDestructionManager setSoDState 2",
            "SeedOfInfinityManager setCurrentStage 4",
        ]
    );

    // The stored values show; stage 2 has a time for its end.
    s.world.set_global_variables(vec![
        ("SoDState".to_owned(), Some("2".to_owned())),
        ("SoDTiatKilled".to_owned(), Some("7".to_owned())),
        ("SoDLSCDate".to_owned(), Some(now_ms().to_string())),
        ("SoI_stage".to_owned(), Some("3".to_owned())),
        ("SoI_CohemenesCount".to_owned(), Some("2".to_owned())),
        ("SoI_EkimusCount".to_owned(), Some("1".to_owned())),
    ]);
    let page = s.cmd("gracia_seeds").pop().unwrap();
    assert!(page.contains("sod 2 tiat 7 time "), "{page}");
    assert!(!page.contains("time -1"), "{page}");
    assert!(page.contains("soi 3 twin N/A coh 2 eki 1"), "{page}");

    // A word without a number: Java's `NumberFormatException`, no event.
    s.cmd("set_sodstate x");
    assert!(take_events().is_empty());
}

#[test]
fn hellbound_tells_the_level_and_the_trust() {
    let _guard = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    take_events();
    // Without the Hellbound scripts the word is plain chat.
    let mut s = setup(&[]);
    assert!(s.say(1, ".hellbound").is_empty());

    let mut s = setup(&["HellboundEngine"]);
    assert_eq!(s.say(1, ".hellbound"), ["Hellbound is currently locked."]);
    s.world.set_global_variables(vec![
        ("HBLevel".to_owned(), Some("3".to_owned())),
        ("HBTrust".to_owned(), Some("123".to_owned())),
    ]);
    assert_eq!(s.say(1, ".hellbound"), ["Hellbound level: 3 trust: 123/1000000"]);
    // Level 11 has no maximum (`MAX_TRUST[11]` is 0).
    s.world
        .set_global_variables(vec![("HBLevel".to_owned(), Some("11".to_owned()))]);
    assert_eq!(s.say(1, ".hellbound"), ["Hellbound level: 11 trust: 0"]);
}

fn now_ms() -> i64 {
    let since = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    i64::try_from(since.as_millis()).unwrap()
}

/// The two tests share the static event log.
static SERIAL: Mutex<()> = Mutex::new(());
