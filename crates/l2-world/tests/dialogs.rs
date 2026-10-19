#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Clicks, targets, NPC dialogs, bypasses and gatekeepers with fake players.
//! Java: network/clientpackets/{Action,RequestBypassToServer,RequestLinkHtml,RequestTargetCanceld},
//! handlers/actions/click/NpcClick, Npc.showChatWindow, Teleporter, TeleportHolder.
//!
//! The pages and teleport lists are small files in a temporary datapack root. The checks against
//! the recorded Java session and against the real datapack skip when the fixture or
//! `C:\l2-rust\l2jmobius` is missing.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use l2_data::{HtmCache, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions, TeleporterData};
use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{
    InventoryUpdate, MoveToPawn, MyTargetSelected, NpcHtmlMessage, StatusUpdate, SystemMessage,
    SystemMessageParam, TargetSelected, TargetUnselected, TeleportToLocation, ValidateLocation,
    visibility::status,
};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

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

/// Keeps what the persistence task would get.
#[derive(Clone, Default)]
struct Stored(Arc<Mutex<Vec<PlayerData>>>);

struct RecordingOutbox(Stored);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        if let PersistCommand::StorePlayer(data) = command {
            self.0.0.lock().unwrap().push(*data);
        }
    }

    fn ls_event(&mut self, _event: LsEvent) {}
}

/// Item names, stackability and nothing else.
struct Items;

impl StaticData for Items {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        Some(ItemStatic {
            display_id: item_id,
            type2: if item_id == 57 { 4 } else { 5 },
            body_part: 0,
            quest_item: false,
            weight: 0,
            shared_reuse_group: -1,
            weapon: false,
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        if level >= 100 {
            AccessLevelStatic {
                name_color: 0,
                title_color: 0,
                is_gm: true,
            }
        } else {
            AccessLevelStatic::USER
        }
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        match item_id {
            57 => Some("Adena".to_owned()),
            9999 => Some("Test Token".to_owned()),
            _ => None,
        }
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        item_id == 57 || item_id == 9999
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

fn names_to(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, _)| *t == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

fn decoded<P: Decode>(out: &Sent, to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

fn html_to(out: &Sent, to: i32) -> Vec<NpcHtmlMessage> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
}

fn messages_to(out: &Sent, to: i32) -> Vec<SystemMessage> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="1000" type="Teleporter" level="70" name="Roxxy" usingServerSideName="true">
    <status attackable="false" />
  </npc>
  <npc id="1001" type="Folk" level="10" name="Villager" usingServerSideName="true"/>
  <npc id="1002" type="Folk" level="10" name="Silent"><status talkable="false" /></npc>
  <npc id="1004" type="Folk" level="10" name="Ghost"><status targetable="false" /></npc>
  <npc id="2000" type="Monster" level="3" name="Rat">
    <stats><vitals hp="62" mp="10"/></stats>
  </npc>
</list>"#;

const TELEPORTERS: &str = r#"<?xml version="1.0"?><list>
  <npc id="1000">
    <teleport type="NORMAL">
      <location name="Gludin" x="-80684" y="149770" z="-3040" feeCount="18000" />
      <location name="Free Spot" x="-80000" y="149000" z="-3040" heading="777" />
    </teleport>
    <teleport type="NOBLES_ADENA">
      <location name="Arena" x="-87328" y="142266" z="-3640" feeCount="1000" />
    </teleport>
    <teleport name="tokens" type="OTHER">
      <location name="Token Spot" x="-87000" y="142000" z="-3640" feeId="9999" feeCount="2" />
      <location x="-86000" y="142000" z="-3640" />
    </teleport>
  </npc>
</list>"#;

/// A datapack root in the temp folder with the pages, templates and teleport lists above.
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
        write("data/teleporters/test.xml", TELEPORTERS);
        write(
            "data/html/teleporter/1000.htm",
            "<html><body>Hi, I am %npcname%.\r\n<a action=\"bypass npc_%objectId%_Chat 1\">more</a>\
             <a action=\"bypass npc_%objectId%_showTeleports\">Teleport</a>\
             <a action=\"bypass -h npc_%objectId%_showNoblesSelect\">Nobles</a>\
             <a action=\"bypass npc_%objectId%_showTeleports NOBLES_ADENA\">Arenas</a>\
             <a action=\"bypass npc_%objectId%_showTeleports tokens\">Tokens</a>\
             <a action=\"bypass -h Link teleporter/separatedsoul.htm\">Soul</a>\
             <a action=\"bypass -h Script Quest\">Quest</a>\
             <a action=\"link teleporter/page2.htm\">page2</a></body></html>",
        );
        write(
            "data/html/teleporter/1000-1.htm",
            "<html><body>Page one of %objectId%<a action=\"bypass npc_%objectId%_Chat 0\">back</a></body></html>",
        );
        write(
            "data/html/teleporter/1000-pk.htm",
            "<html><body>Go away, killer. %objectId%</body></html>",
        );
        write(
            "data/html/teleporter/teleports.htm",
            "<html><body>Where to?<br>%locations%</body></html>",
        );
        write(
            "data/html/teleporter/page2.htm",
            "<html><body>Second page</body></html>",
        );
        write(
            "data/html/teleporter/not_nobles.htm",
            "<html><body>Not noble %npcname%</body></html>",
        );
        write(
            "data/html/teleporter/nobles_select.htm",
            "<html><body>Noble %objectId%<a action=\"bypass npc_%objectId%_showTeleports NOBLES_ADENA\">list</a></body></html>",
        );
        write(
            "data/html/teleporter/separatedsoul.htm",
            "<html><body>Separated soul %objectId%</body></html>",
        );
        write(
            "data/html/default/1001.htm",
            "<html><body>Hello, I am %npcname% (%objectId%)</body></html>",
        );
        write(
            "data/html/signboard/pvp_signboard.htm",
            "<html><body>PvP arena sign</body></html>",
        );
        write(
            "data/html/npcdefault.htm",
            "<html><body>I have nothing to say to you.</body></html>",
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
    stored: Stored,
    ids: Arc<IdFactory>,
    _root: Root,
}

fn setup() -> Setup {
    setup_with(WorldSettings::default())
}

fn setup_with(settings: WorldSettings) -> Setup {
    let root = Root::new();
    let log = Log::default();
    let stored = Stored::default();
    let mut world = World::new(WorldRng::from_seed(11), Box::new(RecordingOutbox(stored.clone())));
    world.set_static_data(Box::new(Items));
    world.set_settings(settings);
    let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
    world.set_id_factory(Arc::clone(&ids));
    let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    world.set_teleporter_data(Arc::new(TeleporterData::load(root.path())));
    Setup {
        world,
        log,
        stored,
        ids,
        _root: root,
    }
}

fn adena(count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(0x1000_0001),
        item_id: 57,
        count,
        enchant_level: 0,
        loc: "INVENTORY".to_owned(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
    }
}

fn token(count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(0x1000_0002),
        item_id: 9999,
        count,
        ..adena(0)
    }
}

fn player(id: i32, level: i32, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
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
        level,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
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
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

impl Setup {
    fn enter_with(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn enter(&mut self, id: i32, level: i32) {
        self.enter_with(player(id, level, vec![]));
    }

    fn spawn(&mut self, npc_id: i32, dx: i32) -> ObjectId {
        self.world.spawn_npc_at(npc_id, X + dx, Y, Z, 4242).unwrap()
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    fn click(&mut self, player: i32, object: ObjectId) {
        self.input(
            player,
            ClientInput::Action {
                object_id: object.0,
                action_id: 0,
            },
        );
    }

    fn bypass(&mut self, player: i32, command: &str) {
        self.input(
            player,
            ClientInput::Bypass {
                command: command.to_owned(),
            },
        );
    }

    fn run_ticks(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.world.tick();
        }
    }
}

// ---------------------------------------------------------------------------------- targets

#[test]
fn first_click_selects_the_target_and_tells_the_neighbours() {
    let mut s = setup();
    s.enter(1, 20);
    s.enter(2, 20);
    let npc = s.spawn(1000, 50);
    s.log.take();

    s.click(1, npc);
    let out = s.log.take();
    // Java: setTarget (ValidateLocation, MyTargetSelected, StatusUpdate), then ActionFailed.
    assert_eq!(
        names_to(&out, 1),
        [
            "VALIDATE_LOCATION",
            "MY_TARGET_SELECTED",
            "STATUS_UPDATE",
            "ACTION_FAIL"
        ]
    );
    let location = decoded::<ValidateLocation>(&out, 1, "VALIDATE_LOCATION")[0];
    assert_eq!(
        (
            location.object_id,
            location.x,
            location.y,
            location.z,
            location.heading
        ),
        (npc.0, X + 50, Y, Z, 4242)
    );
    let selected = decoded::<MyTargetSelected>(&out, 1, "MY_TARGET_SELECTED")[0];
    // A gatekeeper cannot be attacked, so the colour is 0.
    assert_eq!((selected.object_id, selected.color), (npc.0, 0));
    let hp = &decoded::<StatusUpdate>(&out, 1, "STATUS_UPDATE")[0];
    assert_eq!(hp.object_id, npc.0);
    assert_eq!(
        hp.attributes.iter().map(|a| a.0).collect::<Vec<_>>(),
        [status::MAX_HP, status::CUR_HP]
    );
    assert_eq!(s.world.target_of(ObjectId(1)), Some(npc));
    // The others see who selected what, from where.
    assert_eq!(names_to(&out, 2), ["TARGET_SELECTED"]);
    let told = decoded::<TargetSelected>(&out, 2, "TARGET_SELECTED")[0];
    assert_eq!(
        (told.object_id, told.target_id, told.x, told.y, told.z),
        (1, npc.0, X, Y, Z)
    );
}

#[test]
fn clicking_the_same_target_does_not_select_again() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1001, 600);
    s.click(1, npc);
    s.log.take();
    // Too far to talk: the second click is the walk, not a new selection.
    s.click(1, npc);
    let out = s.log.take();
    assert!(!names_to(&out, 1).contains(&"MY_TARGET_SELECTED".to_owned()));
}

#[test]
fn escape_drops_the_target_for_everybody() {
    let mut s = setup();
    s.enter(1, 20);
    s.enter(2, 20);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.log.take();

    s.input(1, ClientInput::TargetCanceled { unselect: 0 });
    let out = s.log.take();
    // Java: `broadcastPacket(new TargetUnselected(this))`, with the player's own id and place.
    for to in [1, 2] {
        assert_eq!(names_to(&out, to), ["TARGET_UNSELECTED"]);
        assert_eq!(
            decoded::<TargetUnselected>(&out, to, "TARGET_UNSELECTED")[0],
            TargetUnselected {
                target_id: 1,
                x: X,
                y: Y,
                z: Z
            }
        );
    }
    assert_eq!(s.world.target_of(ObjectId(1)), None);

    // Nothing selected: nothing happens.
    s.input(1, ClientInput::TargetCanceled { unselect: 1 });
    assert!(s.log.take().is_empty());
}

#[test]
fn clicks_that_cannot_work_answer_action_failed() {
    let mut s = setup();
    s.enter(1, 20);
    let ghost = s.spawn(1004, 50);
    let far_up = s.world.spawn_npc_at(1001, X + 60, Y, Z + 4000, 0).unwrap();
    let villager = s.spawn(1001, 70);
    s.log.take();

    // Unknown object, a template that cannot be targeted, an unknown action.
    s.click(1, ObjectId(0x1234_5678));
    s.click(1, ghost);
    s.input(
        1,
        ClientInput::Action {
            object_id: villager.0,
            action_id: 7,
        },
    );
    let out = s.log.take();
    assert_eq!(names_to(&out, 1), ["ACTION_FAIL", "ACTION_FAIL", "ACTION_FAIL"]);
    assert_eq!(s.world.target_of(ObjectId(1)), None);

    // `/target` exploiting: more than 3000 above the player is no target.
    s.click(1, far_up);
    let out = s.log.take();
    assert_eq!(names_to(&out, 1), ["ACTION_FAIL"]);
    assert_eq!(s.world.target_of(ObjectId(1)), None);
}

#[test]
fn a_shift_click_by_a_player_only_selects() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1001, 50);
    s.log.take();
    for _ in 0..2 {
        s.input(
            1,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 1,
            },
        );
    }
    let out = s.log.take();
    // The first click selects; the second would talk, but a shift click never interacts.
    assert_eq!(
        names_to(&out, 1),
        [
            "VALIDATE_LOCATION",
            "MY_TARGET_SELECTED",
            "STATUS_UPDATE",
            "ACTION_FAIL",
            "ACTION_FAIL"
        ]
    );
}

#[test]
fn the_target_goes_when_the_target_leaves_view_and_when_the_player_teleports() {
    let mut s = setup();
    s.enter(1, 20);
    s.enter(2, 20);
    let a = s.spawn(1001, 50);
    s.click(1, a);
    s.log.take();

    assert!(s.world.delete_npc(a));
    let out = s.log.take();
    // `removeVisibleObject` clears the target before it sends DeleteObject.
    assert_eq!(names_to(&out, 1), ["TARGET_UNSELECTED", "DELETE_OBJECT"]);
    assert_eq!(s.world.target_of(ObjectId(1)), None);

    let b = s.spawn(1001, 60);
    s.click(1, b);
    s.log.take();
    s.world
        .tele_to_location(ObjectId(1), TeleportTarget::at(X + 30_000, Y, Z));
    let out = s.log.take();
    assert_eq!(
        names_to(&out, 1),
        [
            "STOP_MOVE",
            "TARGET_UNSELECTED",
            "DELETE_OBJECT",
            "DELETE_OBJECT",
            "TELEPORT_TO_LOCATION"
        ]
    );
    assert_eq!(s.world.target_of(ObjectId(1)), None);
    // The other player saw the target go too.
    assert!(names_to(&out, 2).contains(&"TARGET_UNSELECTED".to_owned()));
}

// ---------------------------------------------------------------------------------- talking

#[test]
fn second_click_talks_with_the_page_java_picks() {
    let mut s = setup();
    s.enter(1, 20);
    s.enter(2, 20);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.log.take();

    s.click(1, npc);
    let out = s.log.take();
    assert_eq!(
        names_to(&out, 1),
        [
            "MOVE_TO_PAWN",
            "SOCIAL_ACTION",
            "NPC_HTML_MESSAGE",
            "ACTION_FAIL",
            "ACTION_FAIL"
        ]
    );
    // The turn-to-me packet goes to the player alone; the greeting animation to everybody.
    assert_eq!(names_to(&out, 2), ["SOCIAL_ACTION"]);
    let pawn = decoded::<MoveToPawn>(&out, 1, "MOVE_TO_PAWN")[0];
    assert_eq!((pawn.object_id, pawn.target_id, pawn.distance), (1, npc.0, 100));

    let html = &html_to(&out, 1)[0];
    assert_eq!(html.npc_obj_id, npc.0);
    assert_eq!(html.item_id, 0);
    // `%npcname%` and `%objectId%` are replaced; the file's `\r` stays (HtmCache only drops tabs
    // and `\n`).
    assert!(
        html.html.starts_with("<html><body>Hi, I am Roxxy.\r"),
        "{}",
        html.html
    );
    assert!(html.html.contains(&format!("bypass npc_{}_Chat 1", npc.0)));
    assert!(!html.html.contains('%'));
    assert_eq!(
        s.world.html_action_count(ObjectId(1), HtmlActionScope::NpcHtml),
        8
    );
}

#[test]
fn a_folk_without_its_own_page_uses_the_default_folder_and_then_npcdefault() {
    let mut s = setup();
    s.enter(1, 20);
    let folk = s.spawn(1001, 50);
    s.click(1, folk);
    s.click(1, folk);
    let out = s.log.take();
    assert!(
        html_to(&out, 1)[0]
            .html
            .contains(&format!("I am Villager ({})", folk.0))
    );

    // No `default/1002.htm`: "I have nothing to say to you", but this one cannot be talked to.
    let silent = s.spawn(1002, 60);
    s.click(1, silent);
    s.log.take();
    s.click(1, silent);
    let out = s.log.take();
    assert!(html_to(&out, 1).is_empty());
    assert!(
        names_to(&out, 1)
            .iter()
            .all(|n| n == "ACTION_FAIL" || n == "MOVE_TO_PAWN" || n == "SOCIAL_ACTION")
    );
}

#[test]
fn talking_blocks_movement_when_the_config_asks_for_it() {
    let settings = WorldSettings {
        player_movement_block_time_ms: 5000,
        ..WorldSettings::default()
    };
    let mut s = setup_with(settings);
    s.enter(1, 20);
    let npc = s.spawn(1001, 50);
    s.click(1, npc);
    assert_eq!(s.world.player(ObjectId(1)).unwrap().movement.not_move_until_ms, 0);
    s.click(1, npc);
    assert_eq!(
        s.world.player(ObjectId(1)).unwrap().movement.not_move_until_ms,
        5000
    );
}

#[test]
fn a_far_npc_is_walked_to_and_then_talked_to() {
    let mut s = setup();
    s.enter(1, 20);
    s.enter(2, 20);
    let npc = s.spawn(1001, 700);
    s.click(1, npc);
    s.log.take();

    // Too far to talk: the player walks (`MoveToPawn` to everybody who sees it).
    s.click(1, npc);
    let out = s.log.take();
    assert_eq!(names_to(&out, 1), ["MOVE_TO_PAWN", "ACTION_FAIL"]);
    assert_eq!(names_to(&out, 2), ["MOVE_TO_PAWN"]);
    let walk = decoded::<MoveToPawn>(&out, 2, "MOVE_TO_PAWN")[0];
    assert_eq!((walk.object_id, walk.target_id, walk.distance), (1, npc.0, 60));
    assert_eq!(
        s.world.player(ObjectId(1)).unwrap().movement.intention,
        Intention::Interact
    );

    // 650 units at 120 per second.
    let mut dialog = None;
    for _ in 0..400 {
        s.world.tick();
        let out = s.log.take();
        if let Some(html) = html_to(&out, 1).into_iter().next() {
            dialog = Some(html);
            break;
        }
    }
    let dialog = dialog.expect("the player never got the dialog");
    assert_eq!(dialog.npc_obj_id, npc.0);
    assert!(dialog.html.contains("I am Villager"));
    let player = s.world.player(ObjectId(1)).unwrap();
    assert_eq!(player.movement.intention, Intention::Idle);
    assert!(!player.movement.is_moving());
    let (px, _, _) = s.world.position_of(ObjectId(1)).unwrap();
    // It ended up close to the NPC: the first walk stops at 55, the follow task walks on to 31.
    assert!((X + 700 - px) < 40, "stopped {} units away", X + 700 - px);
}

#[test]
fn walking_to_a_far_npc_ends_when_the_npc_goes() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1001, 700);
    s.click(1, npc);
    s.click(1, npc);
    s.run_ticks(10);
    s.log.take();
    assert!(s.world.delete_npc(npc));
    let player = s.world.player(ObjectId(1)).unwrap();
    assert_eq!(player.movement.intention, Intention::Idle);
    assert!(!player.movement.is_moving());
    s.run_ticks(100);
    assert!(html_to(&s.log.take(), 1).is_empty());
}

// ---------------------------------------------------------------------------------- bypasses

#[test]
fn chat_bypass_opens_the_numbered_page_and_back() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.click(1, npc);
    s.log.take();

    s.bypass(1, &format!("npc_{}_Chat 1", npc.0));
    let out = s.log.take();
    // The page, the ActionFailed of showChatWindow and the one of the npc_ branch.
    assert_eq!(
        names_to(&out, 1),
        ["NPC_HTML_MESSAGE", "ACTION_FAIL", "ACTION_FAIL"]
    );
    assert_eq!(
        html_to(&out, 1)[0].html,
        format!(
            "<html><body>Page one of {}<a action=\"bypass npc_{}_Chat 0\">back</a></body></html>",
            npc.0, npc.0
        )
    );

    // The new page has its own links: the old ones are gone from the cache.
    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    assert!(s.log.take().is_empty());
    s.bypass(1, &format!("npc_{}_Chat 0", npc.0));
    assert!(html_to(&s.log.take(), 1)[0].html.contains("I am Roxxy"));
}

#[test]
fn bypasses_that_the_player_was_not_sent_are_dropped() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.click(1, npc);
    s.log.take();

    for command in [
        format!("npc_{}_Chat 7", npc.0),
        "admin_teleport".to_owned(),
        "Link common/crafting_01.htm".to_owned(),
        "Script Other".to_owned(),
    ] {
        s.bypass(1, &command);
    }
    assert!(s.log.take().is_empty());
}

#[test]
fn a_bypass_from_too_far_away_is_ignored() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.click(1, npc);
    s.log.take();

    // The player runs away; the page is still open on the client.
    s.world.set_position(ObjectId(1), X + 600, Y, Z);
    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    assert!(s.log.take().is_empty());
}

#[test]
fn an_empty_bypass_disconnects_the_player() {
    let mut s = setup();
    s.enter(1, 20);
    s.log.take();
    s.bypass(1, "");
    assert!(s.world.player(ObjectId(1)).is_none());
    assert_eq!(names_to(&s.log.take(), 1), ["LOG_OUT_OK"]);
}

#[test]
fn unknown_npc_bypasses_and_script_links_are_logged_and_ignored() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.click(1, npc);
    s.log.take();

    // `bypass -h Script Quest` is in the page and bound to the NPC, but no script handler exists.
    s.bypass(1, "Script Quest");
    assert!(s.log.take().is_empty());
    assert_eq!(s.world.player(ObjectId(1)).unwrap().data.name, "P1");
}

#[test]
fn the_link_bypass_shows_a_valid_page_only_for_a_teleporter_target() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.click(1, npc);
    s.log.take();

    s.bypass(1, "Link teleporter/separatedsoul.htm");
    let out = s.log.take();
    assert_eq!(names_to(&out, 1), ["NPC_HTML_MESSAGE"]);
    assert_eq!(
        html_to(&out, 1)[0].html,
        format!("<html><body>Separated soul {}</body></html>", npc.0)
    );
}

#[test]
fn the_link_bypass_refuses_teleporter_pages_without_a_teleporter_target() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.click(1, npc);
    s.input(1, ClientInput::TargetCanceled { unselect: 0 });
    s.log.take();

    s.bypass(1, "Link teleporter/separatedsoul.htm");
    assert!(s.log.take().is_empty());
}

#[test]
fn link_html_shows_a_listed_page_and_checks_the_path() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.click(1, npc);
    s.log.take();

    s.input(
        1,
        ClientInput::LinkHtml {
            link: "teleporter/page2.htm".to_owned(),
        },
    );
    let out = s.log.take();
    assert_eq!(names_to(&out, 1), ["NPC_HTML_MESSAGE"]);
    let html = &html_to(&out, 1)[0];
    assert_eq!(html.npc_obj_id, npc.0);
    assert_eq!(html.html, "<html><body>Second page</body></html>");

    // Not in the page, with `..`, empty: all dropped.
    for link in ["teleporter/other.htm", "../x.htm", ""] {
        s.input(
            1,
            ClientInput::LinkHtml {
                link: link.to_owned(),
            },
        );
    }
    assert!(s.log.take().is_empty());
}

#[test]
fn a_missing_page_shows_javas_missing_text() {
    let mut s = setup();
    s.enter(1, 20);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.click(1, npc);
    s.log.take();
    // Page 9 of the gatekeeper: its folder is fixed, the file does not exist.
    s.world.show_chat_window(npc, ObjectId(1), 9);
    let out = s.log.take();
    assert_eq!(
        html_to(&out, 1)[0].html,
        "<html><body>My Text is missing:<br>data/html/teleporter/1000-9.htm</body></html>"
    );
}

#[test]
fn karma_players_get_the_pk_page_of_a_gatekeeper() {
    let mut s = setup();
    let mut data = player(1, 20, vec![]);
    data.karma = 100;
    s.enter_with(data);
    let npc = s.spawn(1000, 50);
    s.click(1, npc);
    s.log.take();
    s.click(1, npc);
    let out = s.log.take();
    assert_eq!(
        html_to(&out, 1)[0].html,
        format!("<html><body>Go away, killer. {}</body></html>", npc.0)
    );
}

// ---------------------------------------------------------------------------------- gatekeeper

fn open_gatekeeper(s: &mut Setup, player: PlayerData) -> ObjectId {
    let id = player.object_id.0;
    s.enter_with(player);
    let npc = s.spawn(1000, 50);
    s.click(id, npc);
    s.click(id, npc);
    s.log.take();
    npc
}

#[test]
fn the_teleport_list_shows_names_and_fees() {
    let mut s = setup();
    let npc = open_gatekeeper(&mut s, player(1, 80, vec![adena(100_000)]));

    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    let out = s.log.take();
    let html = &html_to(&out, 1)[0].html;
    let bypass = format!("npc_{}_teleport", npc.0);
    assert!(
        html.contains(&format!(
            "<a action=\"bypass -h {bypass} NORMAL 0\" msg=\"811;Gludin\">Gludin - 18000 Adena</a><br1>"
        )),
        "{html}"
    );
    // No fee set: the name alone.
    assert!(html.contains(&format!(
        "<a action=\"bypass -h {bypass} NORMAL 1\" msg=\"811;Free Spot\">Free Spot</a><br1>"
    )));
    assert!(html.starts_with("<html><body>Where to?<br>"));
    // The list's own links are now the allowed commands.
    s.bypass(1, &format!("npc_{}_teleport NORMAL 1", npc.0));
    assert!(!s.log.take().is_empty());
}

#[test]
fn a_low_level_player_teleports_for_free() {
    let mut s = setup();
    let npc = open_gatekeeper(&mut s, player(1, 40, vec![adena(100)]));
    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    let html = html_to(&s.log.take(), 1)[0].html.clone();
    // Level 40 is still free (`> MaxFreeTeleportLevel` pays): no price in the list.
    assert!(html.contains("msg=\"811;Gludin\">Gludin</a>"), "{html}");

    s.bypass(1, &format!("npc_{}_teleport NORMAL 0", npc.0));
    let out = s.log.take();
    assert_eq!(
        decoded::<TeleportToLocation>(&out, 1, "TELEPORT_TO_LOCATION")[0],
        TeleportToLocation {
            object_id: 1,
            x: -80_684,
            y: 149_770,
            z: -3_040 + 5,
            heading: 0
        }
    );
    assert_eq!(s.world.adena_of(ObjectId(1)), 100);
    assert!(messages_to(&out, 1).is_empty());
}

#[test]
fn the_fee_is_taken_told_and_stored() {
    let mut s = setup();
    let npc = open_gatekeeper(&mut s, player(1, 80, vec![adena(100_000)]));

    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    s.bypass(1, &format!("npc_{}_teleport NORMAL 0", npc.0));
    let out = s.log.take();
    // The message first, then the teleport; the inventory update comes 100 ms later.
    let names = names_to(&out, 1);
    let message = names.iter().position(|n| n == "SYSTEM_MESSAGE").unwrap();
    let tele = names.iter().position(|n| n == "TELEPORT_TO_LOCATION").unwrap();
    assert!(message < tele, "{names:?}");
    assert!(!names.contains(&"INVENTORY_UPDATE".to_owned()));
    let messages = messages_to(&out, 1);
    assert_eq!(messages.len(), 1);
    // S1_ADENA_DISAPPEARED (672) with the amount.
    assert_eq!(messages[0].id.id(), 672);
    assert_eq!(messages[0].params, [SystemMessageParam::LongNumber(18_000)]);
    assert_eq!(s.world.adena_of(ObjectId(1)), 82_000);

    s.run_ticks(1);
    assert!(s.log.take().is_empty());
    s.run_ticks(1);
    let out = s.log.take();
    let update = decoded::<InventoryUpdate>(&out, 1, "INVENTORY_UPDATE");
    assert_eq!(update.len(), 1);
    assert_eq!(update[0].updates.len(), 1);
    assert_eq!(update[0].updates[0].change, InventoryUpdate::MODIFIED);
    assert_eq!(update[0].updates[0].item.count, 82_000);
    assert_eq!(update[0].updates[0].item.object_id, 0x1000_0001);

    // The persistence task gets the new count with the next store: log out and look.
    s.input(1, ClientInput::Logout);
    let stored = s.stored.0.lock().unwrap();
    let last = stored.last().unwrap();
    assert_eq!(last.items.len(), 1);
    assert_eq!(last.items[0].count, 82_000);
    assert_eq!((last.x, last.y), (-80_684, 149_770));
}

#[test]
fn the_whole_stack_goes_when_the_fee_is_all_of_it() {
    let mut s = setup();
    let npc = open_gatekeeper(&mut s, player(1, 80, vec![adena(18_000)]));
    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    s.bypass(1, &format!("npc_{}_teleport NORMAL 0", npc.0));
    s.log.take();
    assert_eq!(s.world.adena_of(ObjectId(1)), 0);
    s.run_ticks(2);
    let out = s.log.take();
    let update = &decoded::<InventoryUpdate>(&out, 1, "INVENTORY_UPDATE")[0];
    assert_eq!(update.updates[0].change, InventoryUpdate::REMOVED);
    assert_eq!(update.updates[0].item.count, 0);
    // The stored inventory loses the row.
    s.input(1, ClientInput::Logout);
    let stored = s.stored.0.lock().unwrap();
    assert!(stored.last().unwrap().items.is_empty());
}

#[test]
fn not_enough_adena_stays_put_and_says_so_twice_like_java() {
    let mut s = setup();
    let npc = open_gatekeeper(&mut s, player(1, 80, vec![adena(17_999)]));
    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    s.bypass(1, &format!("npc_{}_teleport NORMAL 0", npc.0));
    let out = s.log.take();
    // NOTE(java-quirk): `reduceAdena` tells the player, then `doTeleport` does it again.
    let ids: Vec<i32> = messages_to(&out, 1).iter().map(|m| m.id.id()).collect();
    assert_eq!(ids, [279, 279]);
    assert!(!names_to(&out, 1).contains(&"TELEPORT_TO_LOCATION".to_owned()));
    assert_eq!(s.world.adena_of(ObjectId(1)), 17_999);
    assert_eq!(s.world.position_of(ObjectId(1)), Some((X, Y, Z)));
}

#[test]
fn monday_and_tuesday_evenings_cost_half() {
    // 1970-01-05 was a Monday: 21:00 UTC.
    let monday_evening = 4 * 86_400_000 + 21 * 3_600_000;
    let mut settings = WorldSettings::default();
    settings.talk.clock_override_unix_ms = Some(monday_evening);
    let mut s = setup_with(settings);
    let npc = open_gatekeeper(&mut s, player(1, 80, vec![adena(100_000)]));
    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    assert!(html_to(&s.log.take(), 1)[0].html.contains("Gludin - 9000 Adena"));
    s.bypass(1, &format!("npc_{}_teleport NORMAL 0", npc.0));
    s.log.take();
    assert_eq!(s.world.adena_of(ObjectId(1)), 91_000);

    // Wednesday 21:00 is full price again.
    let mut settings = WorldSettings::default();
    settings.talk.clock_override_unix_ms = Some(6 * 86_400_000 + 21 * 3_600_000);
    let mut s = setup_with(settings);
    let npc = open_gatekeeper(&mut s, player(1, 80, vec![adena(100_000)]));
    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    assert!(html_to(&s.log.take(), 1)[0].html.contains("Gludin - 18000 Adena"));
}

#[test]
fn other_lists_always_charge_and_name_the_fee_item() {
    let mut s = setup();
    // Level 1, but the `tokens` list is an OTHER list: the fee always applies.
    let npc = open_gatekeeper(&mut s, player(1, 1, vec![token(5)]));
    s.bypass(1, &format!("npc_{}_showTeleports tokens", npc.0));
    let html = html_to(&s.log.take(), 1)[0].html.clone();
    assert!(html.contains("Token Spot - 2 Test Token</a>"), "{html}");
    // A destination without a name prints as `null`, like Java's string concatenation.
    assert!(html.contains("msg=\"811;null\">null</a>"), "{html}");

    s.bypass(1, &format!("npc_{}_teleport tokens 0", npc.0));
    let out = s.log.take();
    assert_eq!(s.world.player(ObjectId(1)).unwrap().data.items[0].count, 3);
    // Two tokens: `$s2 $s1 has disappeared`, the item first, then the count.
    let message = &messages_to(&out, 1)[0];
    assert_eq!(message.id.id(), 301);
    assert_eq!(
        message.params,
        [
            SystemMessageParam::ItemName(9999),
            SystemMessageParam::LongNumber(2)
        ]
    );
    // CUR_LOAD goes out with the update.
    let load = decoded::<StatusUpdate>(&out, 1, "STATUS_UPDATE");
    assert_eq!(load[0].attributes[0].0, status::CUR_LOAD);
    assert!(names_to(&out, 1).contains(&"TELEPORT_TO_LOCATION".to_owned()));
}

#[test]
fn missing_fee_items_keep_the_player_where_it_is() {
    let mut s = setup();
    let npc = open_gatekeeper(&mut s, player(1, 1, vec![token(1)]));
    s.bypass(1, &format!("npc_{}_showTeleports tokens", npc.0));
    s.log.take();
    s.bypass(1, &format!("npc_{}_teleport tokens 0", npc.0));
    let out = s.log.take();
    // INCORRECT_ITEM_COUNT_2 from the destroy, then Java's own line.
    let messages = messages_to(&out, 1);
    assert_eq!(messages[0].id.id(), 351);
    assert_eq!(
        messages[1].params,
        [SystemMessageParam::Text(
            "You do not have enough Test Token.".to_owned()
        )]
    );
    assert!(!names_to(&out, 1).contains(&"TELEPORT_TO_LOCATION".to_owned()));
    assert_eq!(s.world.player(ObjectId(1)).unwrap().data.items[0].count, 1);
}

#[test]
fn a_free_destination_with_heading_takes_it_along() {
    let mut s = setup();
    let npc = open_gatekeeper(&mut s, player(1, 80, vec![]));
    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    s.bypass(1, &format!("npc_{}_teleport NORMAL 1", npc.0));
    let out = s.log.take();
    // `feeCount` 0: nothing is charged even above the free level.
    assert!(messages_to(&out, 1).is_empty());
    assert_eq!(
        decoded::<TeleportToLocation>(&out, 1, "TELEPORT_TO_LOCATION")[0].heading,
        777
    );
}

#[test]
fn noble_lists_need_a_noble() {
    let mut s = setup();
    let mut data = player(1, 80, vec![adena(5000)]);
    data.profile.noble = false;
    let npc = open_gatekeeper(&mut s, data);

    s.bypass(1, &format!("npc_{}_showNoblesSelect", npc.0));
    let out = s.log.take();
    assert_eq!(
        html_to(&out, 1)[0].html,
        "<html><body>Not noble Roxxy</body></html>"
    );
    s.bypass(1, &format!("npc_{}_showTeleports NOBLES_ADENA", npc.0));
    assert!(html_to(&s.log.take(), 1).is_empty());

    // A noble gets the selection page and the list.
    let mut s = setup();
    let mut data = player(1, 80, vec![adena(5000)]);
    data.profile.noble = true;
    let npc = open_gatekeeper(&mut s, data);
    s.bypass(1, &format!("npc_{}_showNoblesSelect", npc.0));
    assert_eq!(
        html_to(&s.log.take(), 1)[0].html,
        format!(
            "<html><body>Noble {0}<a action=\"bypass npc_{0}_showTeleports NOBLES_ADENA\">list</a></body></html>",
            npc.0
        )
    );
    s.bypass(1, &format!("npc_{}_showTeleports NOBLES_ADENA", npc.0));
    assert!(html_to(&s.log.take(), 1)[0].html.contains("Arena - 1000 Adena"));
}

#[test]
fn karma_players_are_sent_away_by_the_gatekeeper() {
    let mut s = setup();
    let npc = open_gatekeeper(&mut s, player(1, 80, vec![adena(100_000)]));
    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    s.log.take();
    // The player turned PK while the list was open.
    s.world.player_mut(ObjectId(1)).unwrap().data.karma = 50;
    s.bypass(1, &format!("npc_{}_teleport NORMAL 0", npc.0));
    let out = s.log.take();
    let messages = messages_to(&out, 1);
    assert_eq!(
        messages[0].params,
        [SystemMessageParam::Text(
            "Go away, you're not welcome here.".to_owned()
        )]
    );
    assert!(!names_to(&out, 1).contains(&"TELEPORT_TO_LOCATION".to_owned()));
    assert_eq!(s.world.adena_of(ObjectId(1)), 100_000);
}

#[test]
fn the_teleport_bypass_checks_its_arguments() {
    let mut s = setup();
    let npc = open_gatekeeper(&mut s, player(1, 80, vec![adena(100_000)]));
    s.bypass(1, &format!("npc_{}_showTeleports", npc.0));
    s.log.take();
    for command in [
        // wrong word count, unknown list, unknown location, not a number
        format!("npc_{}_teleport NORMAL", npc.0),
        format!("npc_{}_teleport nope 0", npc.0),
        format!("npc_{}_teleport NORMAL 9", npc.0),
        format!("npc_{}_teleport NORMAL x", npc.0),
    ] {
        s.bypass(1, &command);
    }
    // Only the cache decides first: these are not in the page, so nothing at all happens.
    assert!(s.log.take().is_empty());
    assert_eq!(s.world.adena_of(ObjectId(1)), 100_000);
}

// ---------------------------------------------------------------------------------- the recording

fn capture_path() -> PathBuf {
    let dir = std::env::var_os("L2_CAPTURE_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../fixtures/captures"));
    dir.join("java-gs-session.jsonl")
}

/// The first click on a monster, as the Java server recorded it, and what the world sends for
/// the same situation. Every click of the recording is checked: the packets Java sent after
/// `ACTION` (up to `ACTION_FAIL`) must equal ours byte for byte, apart from the object id.
#[test]
fn the_first_click_matches_the_recorded_java_session() {
    let path = capture_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: {} is not readable", path.display());
        return;
    };
    struct Row {
        dir: String,
        name: String,
        raw: Vec<u8>,
    }
    let rows: Vec<Row> = text
        .lines()
        .skip(1)
        .filter_map(|line| {
            let v: serde_json::Value = serde_json::from_str(line).ok()?;
            if v["channel"] != "game" {
                return None;
            }
            Some(Row {
                dir: v["dir"].as_str()?.to_owned(),
                name: v["name"].as_str()?.to_owned(),
                raw: hex::decode(v["raw"].as_str()?).ok()?,
            })
        })
        .collect();

    let mut checked = 0;
    for (i, row) in rows.iter().enumerate() {
        if row.dir != "c2s" || row.name != "ACTION" {
            continue;
        }
        // The answer: VALIDATE_LOCATION, MY_TARGET_SELECTED, STATUS_UPDATE, ACTION_FAIL.
        let answer: Vec<&Row> = rows[i + 1..]
            .iter()
            .filter(|r| r.dir == "s2c")
            .take_while(|r| r.name != "ACTION_FAIL")
            .collect();
        let want = ["VALIDATE_LOCATION", "MY_TARGET_SELECTED", "STATUS_UPDATE"];
        let shape: Vec<&str> = answer
            .iter()
            .filter(|r| want.contains(&r.name.as_str()))
            .map(|r| r.name.as_str())
            .collect();
        if shape != want {
            continue;
        }
        let java_location: ValidateLocation =
            decode(&answer.iter().find(|r| r.name == "VALIDATE_LOCATION").unwrap().raw);
        let java_selected: MyTargetSelected = decode(
            &answer
                .iter()
                .find(|r| r.name == "MY_TARGET_SELECTED")
                .unwrap()
                .raw,
        );
        let java_hp: StatusUpdate = decode(&answer.iter().find(|r| r.name == "STATUS_UPDATE").unwrap().raw);
        let java_hp_value = java_hp.attributes[1].1;

        // The same monster: place, heading, hit points, and a level that gives Java's colour.
        let templates = format!(
            r#"<list><npc id="3000" type="Monster" level="{}" name="Rat"><stats><vitals hp="{java_hp_value}"/></stats></npc></list>"#,
            10 - i32::from(java_selected.color)
        );
        let npcs = Arc::new(NpcTemplates::from_xml(&templates, &NpcLoadOptions::default()).unwrap());
        let root = tempfile::tempdir().unwrap();
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        let log = Log::default();
        let mut world = World::new(
            WorldRng::from_seed(5),
            Box::new(RecordingOutbox(Stored::default())),
        );
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(npcs, spawns);
        let mut data = player(1, 10, vec![]);
        data.x = java_location.x - 100;
        data.y = java_location.y;
        data.z = java_location.z;
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(data),
            sink: Box::new(Sink(1, log.clone())),
            ip: None,
        });
        let npc = world
            .spawn_npc_at(
                3000,
                java_location.x,
                java_location.y,
                java_location.z,
                java_location.heading,
            )
            .unwrap();
        log.take();
        world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        });
        let out = log.take();
        assert_eq!(
            names_to(&out, 1),
            [
                "VALIDATE_LOCATION",
                "MY_TARGET_SELECTED",
                "STATUS_UPDATE",
                "ACTION_FAIL"
            ],
            "click {i}"
        );
        // Java's packets with our object id in them must be our packets, byte for byte.
        let mut expected = Vec::new();
        for java in answer.iter().filter(|r| want.contains(&r.name.as_str())) {
            let mut raw = java.raw.clone();
            raw[1..5].copy_from_slice(&npc.0.to_le_bytes());
            expected.push(raw);
        }
        let ours: Vec<Vec<u8>> = out.iter().take(3).map(|(_, b)| b.clone()).collect();
        assert_eq!(ours, expected, "click {i}");
        checked += 1;
    }
    eprintln!("{checked} clicks of the recording matched");
    assert!(checked > 0, "the recording has no click on a monster");
}

#[test]
fn escape_matches_the_recorded_java_session() {
    let path = capture_path();
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: {} is not readable", path.display());
        return;
    };
    // The recorded answer to `REQUEST_TARGET_CANCELD`: TARGET_UNSELECTED about the player itself.
    let mut found = None;
    let mut last_was_cancel = false;
    for line in text.lines().skip(1) {
        let Ok(v) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let (Some(dir), Some(name)) = (v["dir"].as_str(), v["name"].as_str()) else {
            continue;
        };
        if dir == "c2s" && name == "REQUEST_TARGET_CANCELD" {
            last_was_cancel = true;
        } else if last_was_cancel && dir == "s2c" && name == "TARGET_UNSELECTED" {
            found = Some(hex::decode(v["raw"].as_str().unwrap()).unwrap());
            break;
        }
    }
    let Some(java) = found else {
        eprintln!("skipped: no escape in the recording");
        return;
    };
    let java_packet: TargetUnselected = decode(&java);

    let mut s = setup();
    let mut data = player(1, 20, vec![]);
    (data.x, data.y, data.z) = (java_packet.x, java_packet.y, java_packet.z);
    s.enter_with(data);
    let npc = s
        .world
        .spawn_npc_at(1001, java_packet.x + 50, java_packet.y, java_packet.z, 0)
        .unwrap();
    s.click(1, npc);
    s.log.take();
    s.input(1, ClientInput::TargetCanceled { unselect: 0 });
    let out = s.log.take();
    let ours = &out[0].1;
    let mut expected = java.clone();
    // The recorded player had another object id.
    expected[1..5].copy_from_slice(&1_i32.to_le_bytes());
    assert_eq!(ours, &expected);
}

// ---------------------------------------------------------------------------------- real data

fn datapack_root() -> Option<PathBuf> {
    let root = PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game");
    root.join("data/html/teleporter/30006.htm")
        .is_file()
        .then_some(root)
}

/// Roxxy of Talking Island with the shipped page, list and fees.
#[test]
fn roxxy_of_the_real_datapack() {
    let Some(root) = datapack_root() else {
        eprintln!("skipped: the reference datapack is not there");
        return;
    };
    let xml = r#"<list><npc id="30006" level="70" type="Teleporter" name="Roxxy" title="Gatekeeper"
        usingServerSideName="true"><status attackable="false"/></npc></list>"#;
    let npcs = Arc::new(NpcTemplates::from_xml(xml, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(
        tempfile::tempdir().unwrap().path(),
        &npcs,
        &SpawnLoadOptions::default(),
    ));
    let log = Log::default();
    let stored = Stored::default();
    let mut world = World::new(WorldRng::from_seed(3), Box::new(RecordingOutbox(stored.clone())));
    world.set_static_data(Box::new(Items));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(&root, false)));
    world.set_teleporter_data(Arc::new(TeleporterData::load(&root)));
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(player(1, 78, vec![adena(1_000_000)])),
        sink: Box::new(Sink(1, log.clone())),
        ip: None,
    });
    let npc = world.spawn_npc_at(30006, X + 40, Y, Z, 0).unwrap();
    let click = |world: &mut World| {
        world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        });
    };
    click(&mut world);
    click(&mut world);
    let out = log.take();
    let page = &html_to(&out, 1)[0].html;
    assert!(page.contains("I am Gatekeeper Roxxy"), "{page}");
    assert!(page.contains(&format!("npc_{}_showTeleports", npc.0)));

    let send = |world: &mut World, command: String| {
        world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::Bypass { command },
        });
    };
    send(&mut world, format!("npc_{}_showTeleports", npc.0));
    let out = log.take();
    let list = &html_to(&out, 1)[0].html;
    assert!(list.contains("Singing Waterfall - 770 Adena"), "{list}");
    assert!(list.contains("The Village of Gludin - 18000 Adena"));

    // Dwarven Village is location 2: 46000 adena.
    send(&mut world, format!("npc_{}_teleport NORMAL 2", npc.0));
    let out = log.take();
    let tele = decoded::<TeleportToLocation>(&out, 1, "TELEPORT_TO_LOCATION")[0];
    assert_eq!((tele.x, tele.y, tele.z - 5), (115_120, -178_224, -880));
    assert_eq!(world.adena_of(ObjectId(1)), 1_000_000 - 46_000);
}

// ---------------------------------------------------------------------------------- doors, signs, GMs

/// A door 20 wide and 100 long, `dx` east of the players.
fn door_xml(id: i32, dx: i32, extra: &str) -> String {
    let (x, y) = (X + dx, Y);
    format!(
        r#"<door id="{id}" name="door{id}" height="150" baseHpMax="1000" pos="{x},{y},{Z}"
            node1="{},{}" node2="{},{}" node3="{},{}" node4="{},{}" nodeZ="{}" {extra}/>"#,
        x - 10,
        y - 50,
        x + 10,
        y - 50,
        x + 10,
        y + 50,
        x - 10,
        y + 50,
        Z - 10
    )
}

#[test]
fn clicking_a_door_selects_it_and_shows_the_attack_colour_only_for_attackable_doors() {
    let mut s = setup();
    s.enter(1, 20);
    let templates = l2_data::DoorTemplates::from_xml(&format!(
        "<list>{}{}</list>",
        door_xml(1, 80, ""),
        door_xml(2, 120, r#"is_attackable="true" level="3""#)
    ))
    .unwrap();
    s.world.spawn_doors(&templates, &s.ids).unwrap();
    let door = |s: &Setup, door_id: i32| s.world.door_by_door_id(door_id).unwrap().object_id;
    let (plain, attackable) = (door(&s, 1), door(&s, 2));
    s.log.take();

    s.click(1, plain);
    let out = s.log.take();
    assert_eq!(
        names_to(&out, 1),
        [
            "VALIDATE_LOCATION",
            "MY_TARGET_SELECTED",
            "STATUS_UPDATE",
            "ACTION_FAIL"
        ]
    );
    assert_eq!(
        decoded::<MyTargetSelected>(&out, 1, "MY_TARGET_SELECTED")[0].color,
        0
    );
    let hp = &decoded::<StatusUpdate>(&out, 1, "STATUS_UPDATE")[0];
    assert_eq!(hp.attributes, [(status::MAX_HP, 1000), (status::CUR_HP, 1000)]);
    // A second click on a door nobody owns does nothing.
    s.click(1, plain);
    assert_eq!(names_to(&s.log.take(), 1), ["ACTION_FAIL"]);

    s.click(1, attackable);
    let out = s.log.take();
    // Player level 20 against door level 3.
    assert_eq!(
        decoded::<MyTargetSelected>(&out, 1, "MY_TARGET_SELECTED")[0].color,
        17
    );
    assert_eq!(s.world.target_of(ObjectId(1)), Some(attackable));
}

#[test]
fn signs_show_their_page_or_their_map_and_far_ones_are_walked_to() {
    let mut s = setup();
    s.enter(1, 20);
    let templates = l2_data::StaticObjectTemplates::from_xml(&format!(
        r#"<list><object id="1" name="sign" type="2" x="{}" y="{Y}" z="{Z}"/>
        <object id="2" name="map" type="0" x="{}" y="{Y}" z="{Z}" texture="gludio" map_x="3" map_y="4"/>
        <object id="3" name="far" type="2" x="{}" y="{Y}" z="{Z}"/></list>"#,
        X + 100,
        X + 120,
        X + 900
    ))
    .unwrap();
    s.world.spawn_static_objects(&templates, &s.ids).unwrap();
    let find = |s: &Setup, id: i32| {
        s.world
            .static_objects()
            .find(|o| o.static_object_id == id)
            .unwrap()
            .object_id
    };
    let (sign, map, far) = (find(&s, 1), find(&s, 2), find(&s, 3));
    s.log.take();

    // First click: a creature to the client, one hit point at most.
    s.click(1, sign);
    let out = s.log.take();
    assert_eq!(
        names_to(&out, 1),
        [
            "VALIDATE_LOCATION",
            "MY_TARGET_SELECTED",
            "STATUS_UPDATE",
            "ACTION_FAIL"
        ]
    );
    s.click(1, sign);
    let out = s.log.take();
    assert_eq!(names_to(&out, 1), ["NPC_HTML_MESSAGE", "ACTION_FAIL"]);
    let html = &html_to(&out, 1)[0];
    assert_eq!(
        (html.npc_obj_id, html.html.as_str()),
        (sign.0, "<html><body>PvP arena sign</body></html>")
    );

    s.click(1, map);
    s.log.take();
    s.click(1, map);
    let out = s.log.take();
    assert_eq!(names_to(&out, 1), ["SHOW_TOWN_MAP", "ACTION_FAIL"]);
    assert_eq!(
        decoded::<l2_protocol_game::server::ShowTownMap>(&out, 1, "SHOW_TOWN_MAP")[0],
        l2_protocol_game::server::ShowTownMap {
            texture: "town_map.gludio".to_owned(),
            x: 3,
            y: 4
        }
    );

    // Too far: the player walks there, but `thinkInteract` never clicks a sign for it.
    s.click(1, far);
    s.log.take();
    s.click(1, far);
    assert!(names_to(&s.log.take(), 1).contains(&"MOVE_TO_PAWN".to_owned()));
    s.run_ticks(400);
    let out = s.log.take();
    assert!(html_to(&out, 1).is_empty());
    let player = s.world.player(ObjectId(1)).unwrap();
    assert_eq!(player.movement.intention, Intention::Idle);
    assert!(!player.movement.is_moving());
    let (px, _, _) = s.world.position_of(ObjectId(1)).unwrap();
    assert!(X + 900 - px < 70, "stopped {} units away", X + 900 - px);
}

#[test]
fn a_gm_is_told_which_page_it_sees_and_may_click_what_players_cannot() {
    use l2_protocol_game::server::{ChatType, CreatureSay, SayBody, SenderField};
    let told = CreatureSay {
        sender_object_id: 0,
        chat_type: ChatType::General,
        sender: SenderField::Name("HTML".to_owned()),
        message_id: -1,
        body: SayBody::Text("html/default/1001.htm".to_owned()),
    }
    .to_bytes();

    let mut s = setup();
    let mut gm = player(1, 80, vec![]);
    gm.access_level = 100;
    s.enter_with(gm);
    let folk = s.spawn(1001, 50);
    let ghost = s.spawn(1004, 60);
    s.log.take();

    s.click(1, ghost);
    let out = s.log.take();
    assert!(names_to(&out, 1).contains(&"MY_TARGET_SELECTED".to_owned()));
    // A GM's shift click keeps the target (the GM windows come with the admin cards).
    s.input(
        1,
        ClientInput::Action {
            object_id: folk.0,
            action_id: 1,
        },
    );
    assert_eq!(s.world.target_of(ObjectId(1)), Some(folk));
    s.log.take();

    s.click(1, folk);
    let out = s.log.take();
    // `GMDebugHtmlPaths`: "HTML" says the path without its `data/`.
    assert!(out.iter().any(|(to, bytes)| *to == 1 && *bytes == told));

    // Off: nothing.
    let mut settings = WorldSettings::default();
    settings.talk.gm_debug_html_paths = false;
    let mut s = setup_with(settings);
    let mut gm = player(1, 80, vec![]);
    gm.access_level = 100;
    s.enter_with(gm);
    let folk = s.spawn(1001, 50);
    s.click(1, folk);
    s.log.take();
    s.click(1, folk);
    let out = s.log.take();
    assert!(!out.iter().any(|(_, bytes)| *bytes == told));
}
