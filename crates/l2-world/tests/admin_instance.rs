#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The instance admin words (M8-INST-01, `admin/instance.rs`): `//createinstance`,
//! `//listinstances`, `//setinstance`, `//destroyinstance`, `//instancezone`,
//! `//instancezone_clear`, `//instance_spawns` and `//instancezones`, with their Java texts.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminInstance,AdminInstanceZone,
//! AdminSpawn,AdminHelp}.java

use std::sync::{Arc, Mutex};

use l2_data::{InstanceTemplates, NpcLoadOptions, NpcTemplates, SpawnData};
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

struct RecordingOutbox(Persisted);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }

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
const PLAYER: i32 = 101;
const TEMPLATE: i32 = 77;

struct Setup {
    world: World,
    log: Log,
    persisted: Persisted,
}

fn enter(world: &mut World, log: &Log, id: i32, name: &str, access_level: i32) {
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(id).unwrap()),
        player: Box::new(PlayerData {
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
            x: 1000,
            y: 1000,
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
        sink: Box::new(Sink(id, log.clone())),
        ip: None,
    });
}

/// A GM, a player and one empty template ("Test Zone", id 77).
fn setup() -> Setup {
    let log = Log::default();
    let persisted = Persisted::default();
    let mut world = World::new(
        WorldRng::from_seed(8),
        Box::new(RecordingOutbox(persisted.clone())),
    );
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (70, 0), (100, 70)] {
        rights.add_level(level, child);
    }
    world.set_static_data(Box::new(Fake { rights }));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(r#"<list></list>"#, &NpcLoadOptions::default()).unwrap());
    world.set_npc_data(npcs, Arc::new(SpawnData::default()));
    let mut templates = InstanceTemplates::default();
    templates
        .add_names(&format!(
            r#"<list><instance id="{TEMPLATE}" name="Test Zone"/></list>"#
        ))
        .unwrap();
    templates.add_template(&format!(r#"<instance id="{TEMPLATE}"/>"#), "test.xml");
    world.set_instance_data(Arc::new(templates));
    enter(&mut world, &log, GM, "Gamemaster", 100);
    enter(&mut world, &log, PLAYER, "Player", 0);
    log.take();
    Setup {
        world,
        log,
        persisted,
    }
}

/// The texts of the system messages with one string (what `sendMessage` sends) to `to`.
fn texts(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(id, b)| {
            let ex = (b[0] == 0xFE).then(|| u16::from_le_bytes([b[1], b[2]]));
            *id == to && ServerOpcode::from_id(b[0], ex) == Some(ServerOpcode::SystemMessage)
        })
        .filter_map(|(_, b)| {
            let m = SystemMessage::decode(&mut PacketReader::new(&b[1..])).ok()?;
            match m.params.as_slice() {
                [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
                _ => None,
            }
        })
        .collect()
}

/// The html pages `to` got.
fn pages(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(id, b)| {
            *id == to && ServerOpcode::from_id(b[0], None) == Some(ServerOpcode::NpcHtmlMessage)
        })
        .map(|(_, b)| {
            NpcHtmlMessage::decode(&mut PacketReader::new(&b[1..]))
                .unwrap()
                .html
        })
        .collect()
}

fn run(s: &mut Setup, command: &str) -> Sent {
    s.world.admin_command(ObjectId(GM), command, false);
    s.log.take()
}

#[test]
fn create_list_and_destroy() {
    let mut s = setup();
    let usage =
        "Example: //createinstance <id> <templateId> - ids => 300000 are reserved for dynamic instances";
    assert_eq!(texts(&run(&mut s, "admin_createinstance"), GM), [usage]);
    assert_eq!(texts(&run(&mut s, "admin_createinstance 5 x"), GM), [usage]);
    assert_eq!(
        texts(&run(&mut s, "admin_createinstance 5 77"), GM),
        ["Instance created."]
    );
    assert_eq!(s.world.instance(5).unwrap().template_id, TEMPLATE);
    // Taken, or a dynamic id.
    assert_eq!(
        texts(&run(&mut s, "admin_createinstance 5 77"), GM),
        ["Failed to create instance."]
    );
    assert_eq!(
        texts(&run(&mut s, "admin_createinstance 300000 77"), GM),
        ["Failed to create instance."]
    );
    assert_eq!(
        texts(&run(&mut s, "admin_createinstance x 77"), GM),
        ["Failed loading: x 77"]
    );
    // Only instances with a script world are listed.
    assert_eq!(
        texts(&run(&mut s, "admin_listinstances"), GM),
        ["No active instances."]
    );
    s.world.add_instance_world(InstanceWorld::new(5, TEMPLATE));
    assert_eq!(
        texts(&run(&mut s, "admin_listinstances"), GM),
        ["Id: 5 Name: Test Zone"]
    );
    assert_eq!(
        texts(&run(&mut s, "admin_destroyinstance"), GM),
        ["Use //destroyinstance id"]
    );
    assert_eq!(
        texts(&run(&mut s, "admin_destroyinstance 5"), GM),
        ["Instance destroyed"]
    );
    assert!(s.world.instance(5).is_none());
}

#[test]
fn set_instance() {
    let mut s = setup();
    let instance = s.world.create_dynamic_instance(TEMPLATE);
    assert_eq!(
        texts(&run(&mut s, "admin_setinstance"), GM),
        ["Use //setinstance id"]
    );
    assert_eq!(
        texts(&run(&mut s, "admin_setinstance 9"), GM),
        ["Instance 9 does not exist."]
    );
    assert_eq!(
        texts(&run(&mut s, &format!("admin_setinstance {instance}")), GM),
        ["Incorrect target."]
    );
    s.world.set_target(ObjectId(GM), Some(ObjectId(PLAYER)));
    s.log.take();
    let out = run(&mut s, &format!("admin_setinstance {instance}"));
    assert_eq!(texts(&out, GM), [format!("Moved Player to instance {instance}.")]);
    assert_eq!(
        texts(&out, PLAYER),
        [format!("Admin set your instance to:{instance}")]
    );
    assert_eq!(s.world.instance_id_of(ObjectId(PLAYER)), instance);
    assert!(
        s.world
            .instance(instance)
            .unwrap()
            .contains_player(ObjectId(PLAYER))
    );
    // The player is teleported in place (`teleToLocation(getLocation())`).
    assert!(out.iter().any(|(id, b)| *id == PLAYER
        && ServerOpcode::from_id(b[0], None) == Some(ServerOpcode::TeleportToLocation)));
}

#[test]
fn instance_zone_times() {
    let mut s = setup();
    let far = 4_000_000_000_000;
    s.world.set_instance_time(ObjectId(PLAYER), TEMPLATE, far);
    let out = run(&mut s, "admin_instancezone Player");
    let page = &pages(&out, GM)[0];
    assert!(
        page.contains("<font color=\"LEVEL\">Instances for Player</font>"),
        "{page}"
    );
    assert!(page.contains("<tr><td>Test Zone</td><td>"), "{page}");
    assert!(
        page.contains("bypass -h admin_instancezone_clear Player 77"),
        "{page}"
    );
    assert_eq!(
        texts(&run(&mut s, "admin_instancezone Nobody"), GM),
        [
            "The player Nobody is not online",
            "Usage: //instancezone [playername]"
        ]
    );
    // Without a name: the GM's own (no times).
    let page = &pages(&run(&mut s, "admin_instancezone"), GM)[0];
    assert!(page.contains("Instances for Gamemaster"));

    let usage = "Usage: //instancezone_clear <playername> [instanceId]";
    assert_eq!(
        texts(&run(&mut s, "admin_instancezone_clear Player"), GM),
        ["Failed clearing instance time: null", usage]
    );
    assert_eq!(
        texts(&run(&mut s, "admin_instancezone_clear Player x"), GM),
        ["Failed clearing instance time: For input string: \"x\"", usage]
    );
    assert_eq!(
        texts(&run(&mut s, "admin_instancezone_clear Nobody 77"), GM),
        [
            "Failed clearing instance time: Cannot invoke \"org.l2jmobius.gameserver.entity.actor.Player.getObjectId()\" because \"player\" is null",
            usage
        ]
    );
    let out = run(&mut s, "admin_instancezone_clear Player 77");
    assert_eq!(
        texts(&out, GM),
        ["Instance zone Test Zone cleared for player Player"]
    );
    assert_eq!(
        texts(&out, PLAYER),
        ["Admin cleared instance zone Test Zone for you"]
    );
    assert_eq!(s.world.instance_time(ObjectId(PLAYER), TEMPLATE), -1);
    let writes: Vec<InstanceTimeWrite> = s
        .persisted
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|c| match c {
            PersistCommand::InstanceTime(w) => Some(*w),
            _ => None,
        })
        .collect();
    assert_eq!(
        writes,
        [
            InstanceTimeWrite::Set {
                char_id: PLAYER,
                instance_id: TEMPLATE,
                time: far
            },
            InstanceTimeWrite::Delete {
                char_id: PLAYER,
                instance_id: TEMPLATE
            }
        ]
    );
}

#[test]
fn instance_spawns_and_the_template_list() {
    let mut s = setup();
    assert_eq!(
        texts(&run(&mut s, "admin_instance_spawns"), GM),
        ["Usage //instance_spawns <instance_number>"]
    );
    assert_eq!(
        texts(&run(&mut s, "admin_instance_spawns 5"), GM),
        ["Invalid instance number."]
    );
    assert_eq!(
        texts(&run(&mut s, "admin_instance_spawns 300009"), GM),
        ["Cannot find instance 300009"]
    );
    let instance = s.world.create_dynamic_instance(TEMPLATE);
    let page = &pages(&run(&mut s, &format!("admin_instance_spawns {instance}")), GM)[0];
    assert!(page.contains(&format!("Spawns for {instance}")));
    assert!(
        page.ends_with("<tr><td>Skipped:</td><td>0</td></tr></table></body></html>"),
        "{page}"
    );
    // `//instancezones`: the page of the help folder (missing here: Java's missing page text).
    let page = &pages(&run(&mut s, "admin_instancezones"), GM)[0];
    assert!(page.contains("data/html/admin/help/instancezones.htm"), "{page}");
}
