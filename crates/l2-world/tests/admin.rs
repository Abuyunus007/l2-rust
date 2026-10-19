#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The admin command framework and the core commands, with a fake GM and a fake normal player.
//! Java: network/clientpackets/SendBypassBuildCmd.java, handler/AdminCommandHandler.java,
//! data/xml/AdminData.java, scripts/handlers/chat/commands/admin/{AdminAdmin,AdminSpawn,
//! AdminTeleport,AdminGoto,AdminKill,AdminHeal}.java, commands/user/Loc.java.
//!
//! The admin pages are small files in a temporary datapack root.

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{HtmCache, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ChatType, CreatureSay, NpcHtmlMessage, SayBody, SenderField, StatusUpdate, SystemMessage,
    SystemMessageParam, TeleportToLocation, visibility::status,
};
use l2_world::*;
use tracing::field::{Field, Visit};
use tracing::span::{Attributes, Id, Record};
use tracing::{Event, Metadata, Subscriber};

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

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}

    fn ls_event(&mut self, _event: LsEvent) {}
}

/// The levels of the shipped `AccessLevels.xml` (only 70 and 100 are GMs) and the rights of the
/// commands these tests use, with the levels of the shipped `AdminCommands.xml`.
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
    for (command, level, confirm) in [
        ("admin", 30, false),
        ("admin1", 30, false),
        ("admin2", 30, false),
        ("admin3", 30, false),
        ("admin6", 30, false),
        ("show_spawns", 100, false),
        ("spawn", 100, false),
        ("spawnat", 100, false),
        ("spawn_monster", 100, false),
        ("spawn_once", 100, false),
        ("list_positions", 100, false),
        ("list_spawns", 100, false),
        ("show_moves", 100, false),
        ("show_teleport", 30, false),
        ("teleport_to_character", 30, false),
        ("teleportto", 30, false),
        ("move_to", 100, false),
        ("teleport_character", 100, false),
        ("recall", 100, false),
        ("tele", 30, false),
        ("gonorth", 30, false),
        ("goup", 30, false),
        ("goto", 30, false),
        ("kill", 100, false),
        ("kill_monster", 100, false),
        ("heal", 100, false),
        // Registered by a handler of the card, but with a confirmation dialog.
        ("sendhome", 100, true),
    ] {
        rights.add_command(command, level, confirm);
    }
    rights
}

struct Fake {
    rights: AdminRights,
    stats: StatData,
}

impl Fake {
    fn new(rights: AdminRights) -> Self {
        Self {
            rights,
            stats: class_data(),
        }
    }
}

/// The class template of the test characters (class 0, level 40): 100 HP, 50 MP and 10 CP, the
/// values their character rows store. No `statBonus.xml`: every bonus is 1.
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

/// The text of every `sendMessage` (system message `S1_2` with one string) sent to `to`.
fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The ids of the system messages (those with no string) sent to `to`.
fn system_ids_to(out: &Sent, to: i32) -> Vec<SystemMessageId> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter(|m| m.id != SystemMessageId::S12)
        .map(|m| m.id)
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

const GM: i32 = 100;
const LOW_GM: i32 = 70;
const USER: i32 = 1;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="1001" type="Folk" level="10" name="Villager" usingServerSideName="true"/>
  <npc id="2000" type="Monster" level="3" name="Rat">
    <stats><vitals hp="62" mp="10"/></stats>
  </npc>
  <npc id="2001" type="Monster" level="5" name="Wolf Pup">
    <stats><vitals hp="100" mp="20"/></stats>
  </npc>
</list>"#;

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
            "data/html/admin/main_menu.htm",
            "<html><title>Main Admin Menu</title><body>\
             <button value=\"Char\" action=\"bypass admin_admin6\">\
             <button value=\"Heal\" action=\"bypass -h admin_heal\"></body></html>",
        );
        write(
            "data/html/admin/char_menu.htm",
            "<html><body>Character menu</body></html>",
        );
        write(
            "data/html/admin/game_menu.htm",
            "<html><body>Game menu</body></html>",
        );
        write(
            "data/html/admin/spawns.htm",
            "<html><body>Spawn panel</body></html>",
        );
        write(
            "data/html/admin/teleports.htm",
            "<html><body>Teleport panel</body></html>",
        );
        write("data/html/admin/move.htm", "<html><body>Move panel</body></html>");
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
    setup_with(WorldSettings::default())
}

fn setup_with(mut settings: WorldSettings) -> Setup {
    // Exact places: the random offset of a teleport has its own tests.
    settings.teleport.offset_on_teleport_enabled = false;
    settings.teleport.max_offset_on_teleport = 0;
    let root = Root::new();
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(11), Box::new(NullOutbox));
    world.set_static_data(Box::new(Fake::new(rights())));
    world.set_settings(settings);
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

fn player(id: i32, access_level: i32, dx: i32) -> PlayerData {
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
        level: 40,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level,
        x: X + dx,
        y: Y,
        z: Z,
        heading: 321,
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
    }
}

impl Setup {
    /// A player whose object id is its access level (or 1 for the plain user).
    fn enter(&mut self, id: i32, dx: i32) {
        let level = if id == USER { 0 } else { id };
        self.enter_with(player(id, level, dx));
    }

    fn enter_with(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    /// `//command`.
    fn cmd(&mut self, player: i32, command: &str) {
        self.input(
            player,
            ClientInput::BuildCmd {
                command: command.to_owned(),
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

    fn pos(&self, player: i32) -> (i32, i32, i32) {
        let core = &self.world.player(ObjectId(player)).unwrap().core;
        (core.x, core.y, core.z)
    }

    fn heading(&self, player: i32) -> i32 {
        self.world.player(ObjectId(player)).unwrap().core.heading
    }

    /// The NPCs of a template, in object id order.
    fn npcs_of(&self, npc_id: i32) -> Vec<ObjectId> {
        self.world
            .npc_ids()
            .into_iter()
            .filter(|id| self.world.npc(*id).unwrap().npc_id == npc_id)
            .collect()
    }
}

const ACCESS_REFUSED: &str = "You don't have the access rights to use this command!";

// ------------------------------------------------------------------------ audit log capture

/// Collects the events of one target.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<(String, String)>>>);

struct Message(String);

impl Visit for Message {
    fn record_debug(&mut self, field: &Field, value: &dyn std::fmt::Debug) {
        if field.name() == "message" {
            self.0 = format!("{value:?}");
        }
    }
}

impl Subscriber for Capture {
    fn enabled(&self, _metadata: &Metadata<'_>) -> bool {
        true
    }
    fn new_span(&self, _span: &Attributes<'_>) -> Id {
        Id::from_u64(1)
    }
    fn record(&self, _span: &Id, _values: &Record<'_>) {}
    fn record_follows_from(&self, _span: &Id, _follows: &Id) {}
    fn event(&self, event: &Event<'_>) {
        let mut message = Message(String::new());
        event.record(&mut message);
        self.0
            .lock()
            .unwrap()
            .push((event.metadata().target().to_owned(), message.0));
    }
    fn enter(&self, _span: &Id) {}
    fn exit(&self, _span: &Id) {}
}

// -------------------------------------------------------------------------------- framework

#[test]
fn a_normal_player_gets_no_answer_at_all() {
    let mut s = setup();
    s.enter(USER, 0);
    s.log.take();
    // Java: `if (!player.isGM()) return;` before anything else, even for a command that exists.
    for command in ["admin", "kill", "nothing", "spawn 2000", "heal"] {
        s.cmd(USER, command);
    }
    s.bypass(USER, "admin_admin");
    assert!(s.log.take().is_empty());
    assert!(s.world.npc_ids().is_empty());
}

#[test]
fn a_gm_without_the_right_is_refused_and_with_it_is_served() {
    let mut s = setup();
    s.enter(LOW_GM, 0);
    s.log.take();

    // Level 70 is a GM (it passes the first check) but its children only reach level 30.
    s.cmd(LOW_GM, "kill");
    s.cmd(LOW_GM, "spawn 2000");
    let out = s.log.take();
    assert_eq!(texts_to(&out, LOW_GM), [ACCESS_REFUSED, ACCESS_REFUSED]);
    assert!(s.world.npc_ids().is_empty());

    s.cmd(LOW_GM, "admin");
    let out = s.log.take();
    assert_eq!(html_to(&out, LOW_GM).len(), 1);
    assert!(texts_to(&out, LOW_GM).is_empty());
}

#[test]
fn an_unknown_command_is_named_back() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();
    s.cmd(GM, "nothing at all");
    // The empty command (`//` alone) is `admin_`.
    s.cmd(GM, "");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        [
            "The command 'nothing' does not exist!",
            "The command '' does not exist!"
        ]
    );
}

#[test]
fn a_command_that_needs_a_confirmation_does_not_run() {
    let mut s = setup();
    s.enter(GM, 0);
    s.world
        .register_admin_handler("admin_sendhome", |_world, _gm, _command| panic!("must not run"));
    s.log.take();
    s.cmd(GM, "sendhome");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM).len(), 1);
    assert!(texts_to(&out, GM)[0].contains("confirmation"));
}

#[test]
fn a_handler_exception_is_reported_with_javas_text() {
    let mut s = setup();
    s.enter(GM, 0);
    s.world
        .register_admin_handler("admin_kill", |_world, _gm, _command| {
            Err(JavaException::number_format("x"))
        });
    s.log.take();
    s.cmd(GM, "kill 5");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        [
            "Exception during execution of  'admin_kill 5': java.lang.NumberFormatException: For input string: \"x\""
        ]
    );
}

#[test]
fn the_gm_audit_line_is_written_per_command_when_enabled() {
    let capture = Capture::default();
    tracing::subscriber::with_default(capture.clone(), || {
        let mut s = setup_with(WorldSettings {
            gm_audit: true,
            ..WorldSettings::default()
        });
        s.enter(GM, 0);
        s.enter(USER, 100);
        s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
        s.cmd(GM, "heal");
        s.world.set_target(ObjectId(GM), None);
        s.cmd(GM, "admin");
        // Refused and unknown commands are not audited (Java logs just before running one).
        s.cmd(GM, "nothing");
    });
    let lines: Vec<String> = capture
        .0
        .lock()
        .unwrap()
        .iter()
        .filter(|(target, _)| target == "gmaudit")
        .map(|(_, message)| message.clone())
        .collect();
    assert_eq!(
        lines,
        ["P100 [100]>admin_heal>P1>", "P100 [100]>admin_admin>no-target>"]
    );

    // Off by default, like Java's `GMAudit = False`.
    let capture = Capture::default();
    tracing::subscriber::with_default(capture.clone(), || {
        let mut s = setup();
        s.enter(GM, 0);
        s.cmd(GM, "admin");
    });
    assert!(
        capture
            .0
            .lock()
            .unwrap()
            .iter()
            .all(|(target, _)| target != "gmaudit")
    );
}

// --------------------------------------------------------------------------------- //admin

#[test]
fn admin_opens_the_panels_and_the_panel_buttons_work() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    s.cmd(GM, "admin");
    let out = s.log.take();
    let pages = html_to(&out, GM);
    assert_eq!(pages.len(), 1);
    assert_eq!(pages[0].npc_obj_id, 0);
    assert!(pages[0].html.contains("Main Admin Menu"));

    // The button of the page: `bypass admin_admin6` is in the html the GM was shown.
    s.bypass(GM, "admin_admin6");
    let out = s.log.take();
    assert!(html_to(&out, GM)[0].html.contains("Character menu"));

    // `admin2`, a number that is not a panel and text after the number.
    s.cmd(GM, "admin2");
    s.cmd(GM, "admin 3");
    let out = s.log.take();
    let pages = html_to(&out, GM);
    assert!(pages[0].html.contains("Game menu"));
    // `Integer.parseInt(" 3")` fails: the main page.
    assert!(pages[1].html.contains("Main Admin Menu"));

    // A page the datapack lacks is Java's "text is missing" page.
    s.cmd(GM, "admin3");
    let out = s.log.take();
    assert!(
        html_to(&out, GM)[0]
            .html
            .contains("My text is missing:<br>data/html/admin/effects_menu.htm")
    );
}

#[test]
fn an_admin_bypass_that_no_page_offered_is_dropped() {
    let mut s = setup();
    s.enter(GM, 0);
    s.cmd(GM, "admin");
    s.log.take();
    // Java: `validateHtmlAction` -> not in the html the player got -> ignored.
    s.bypass(GM, "admin_kill_menu");
    s.bypass(GM, "admin_spawn 2000");
    assert!(s.log.take().is_empty());
    assert!(s.world.npc_ids().is_empty());
    // A button with `-h` is cached without the `-h`: this one is in the page.
    s.bypass(GM, "admin_heal");
    let out = s.log.take();
    // //heal on the GM itself (no target): its user info.
    assert!(names_to(&out, GM).contains(&"USER_INFO".to_owned()));
}

// -------------------------------------------------------------------------------- //spawn

#[test]
fn spawn_puts_an_npc_at_the_gm_and_says_so() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    s.cmd(GM, "spawn 2000");
    let npcs = s.npcs_of(2000);
    assert_eq!(npcs.len(), 1);
    let npc = s.world.npc(npcs[0]).unwrap();
    assert_eq!(
        (npc.core.x, npc.core.y, npc.core.z, npc.core.heading),
        (X, Y, Z, 321)
    );
    let out = s.log.take();
    // Java: "Created <name> on <target object id>" - the GM is its own target.
    assert_eq!(texts_to(&out, GM), [format!("Created Rat on {GM}")]);
    assert!(names_to(&out, GM).contains(&"NPC_INFO".to_owned()));
}

#[test]
fn spawn_takes_a_count_a_respawn_time_a_name_and_a_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.log.take();

    // id, count, respawn time.
    s.cmd(GM, "spawn 2000 3 120");
    assert_eq!(s.npcs_of(2000).len(), 3);
    // `spawn_once` and `spawn_monster` are the same command here (no respawn before M4).
    s.cmd(GM, "spawn_once 2000");
    s.cmd(GM, "spawn_monster 2000 2");
    assert_eq!(s.npcs_of(2000).len(), 6);

    // A name made of several words: the longest name wins, the rest are the numbers.
    s.cmd(GM, "spawn Wolf Pup 2");
    assert_eq!(s.npcs_of(2001).len(), 2);
    // A name with underscores.
    s.cmd(GM, "spawn_monster Wolf_Pup");
    assert_eq!(s.npcs_of(2001).len(), 3);
    s.cmd(GM, "spawn rat");
    assert_eq!(s.npcs_of(2000).len(), 7);

    // The target decides the place, the GM the heading; the answer names the target.
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();
    s.cmd(GM, "spawn 1001");
    let villager = s.npcs_of(1001)[0];
    let npc = s.world.npc(villager).unwrap();
    assert_eq!((npc.core.x, npc.core.y, npc.core.heading), (X + 300, Y, 321));
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [format!("Created Villager on {USER}")]
    );
}

#[test]
fn spawn_with_bad_data_shows_the_spawn_panel() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();
    for command in [
        "spawn",
        "spawn 99999",
        "spawn nobody",
        "spawn 2000 x",
        "spawn 2000 1 y",
        "spawn 99999999999",
    ] {
        s.cmd(GM, command);
        let out = s.log.take();
        let pages = html_to(&out, GM);
        assert_eq!(pages.len(), 1, "{command}");
        assert!(pages[0].html.contains("Spawn panel"), "{command}");
    }
    assert!(s.world.npc_ids().is_empty());

    s.cmd(GM, "show_spawns");
    assert!(html_to(&s.log.take(), GM)[0].html.contains("Spawn panel"));
}

#[test]
fn spawnat_uses_the_coordinates_and_the_heading() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    s.cmd(GM, "spawnat 2000 -71200 258100 -3100 77");
    let npc = s.world.npc(s.npcs_of(2000)[0]).unwrap();
    assert_eq!(
        (npc.core.x, npc.core.y, npc.core.z, npc.core.heading),
        (-71200, 258100, -3100, 77)
    );
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), [format!("Created Rat on {GM}")]);
    // `spawn.init()` shows it, `broadcastInfo()` shows it once more.
    assert_eq!(names_to(&out, GM).iter().filter(|n| *n == "NPC_INFO").count(), 2);

    // No heading: the GM's.
    s.cmd(GM, "spawnat 2001 -71200 258100 -3100");
    assert_eq!(s.world.npc(s.npcs_of(2001)[0]).unwrap().core.heading, 321);

    // Missing numbers or an unknown id: the panel.
    s.log.take();
    for command in ["spawnat 2000 1 2", "spawnat 5 1 2 3", "spawnat 2000 a 2 3"] {
        s.cmd(GM, command);
        assert!(
            html_to(&s.log.take(), GM)[0].html.contains("Spawn panel"),
            "{command}"
        );
    }
}

// ----------------------------------------------------------------------------- //move_to etc.

#[test]
fn move_to_teleports_the_gm() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    s.cmd(GM, "move_to -80000 150000 -3000");
    let out = s.log.take();
    // Java adds 5 to z in `teleToLocation`; the packet goes to the GM and to who saw it leave.
    let expected = TeleportToLocation {
        object_id: GM,
        x: -80000,
        y: 150000,
        z: -2995,
        heading: 0,
    };
    assert_eq!(
        decoded::<TeleportToLocation>(&out, GM, "TELEPORT_TO_LOCATION"),
        [expected]
    );
    assert_eq!(
        decoded::<TeleportToLocation>(&out, USER, "TELEPORT_TO_LOCATION"),
        [expected]
    );
    assert_eq!(
        texts_to(&out, GM),
        ["You have been teleported to -80000 150000 -3000"]
    );
    assert_eq!(s.pos(GM), (-80000, 150000, -2995));
}

#[test]
fn move_to_answers_bad_input() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    // `//move_to` alone: `substring(14)` throws -> the teleport panel.
    s.cmd(GM, "move_to");
    let out = s.log.take();
    assert!(html_to(&out, GM)[0].html.contains("Teleport panel"));
    assert!(texts_to(&out, GM).is_empty());
    // The panel with an empty quick box sends a trailing space: no numbers at all. A number
    // missing is the same.
    s.cmd(GM, "move_to ");
    s.cmd(GM, "move_to 1 2");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        ["Wrong or no Coordinates given.", "Wrong or no Coordinates given."]
    );
    assert!(html_to(&out, GM).is_empty());
    // A bad number: usage and the panel.
    s.cmd(GM, "move_to 1 x 3");
    s.cmd(GM, "move_to 1 2 3.5");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        ["Usage: //move_to <x> <y> <z>", "Usage: //move_to <x> <y> <z>"]
    );
    assert_eq!(html_to(&out, GM).len(), 2);
    assert_eq!(s.pos(GM), (X, Y, Z));
}

#[test]
fn teleportto_goes_to_a_player_by_name() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 400);
    s.log.take();

    s.cmd(GM, "teleportto p1");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["You have teleported to character P1."]);
    assert_eq!(s.pos(GM), (X + 400, Y, Z + 5));

    // Yourself, and nobody.
    s.cmd(GM, "teleportto P100");
    s.cmd(GM, "teleportto Nobody");
    let out = s.log.take();
    assert_eq!(
        system_ids_to(&out, GM),
        [
            SystemMessageId::YouCannotUseThisOnYourself,
            SystemMessageId::InvalidTarget
        ]
    );
    // Without a name Java does nothing.
    s.cmd(GM, "teleportto");
    assert!(s.log.take().is_empty());

    // `teleport_to_character` goes to the target.
    s.world.set_target(ObjectId(GM), None);
    s.cmd(GM, "teleport_to_character");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);
}

#[test]
fn recall_brings_a_player_to_the_gm() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 800);
    s.log.take();

    s.cmd(GM, "recall P1");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["You have recalled P1"]);
    assert_eq!(texts_to(&out, USER), ["Admin is teleporting you."]);
    let moved = decoded::<TeleportToLocation>(&out, USER, "TELEPORT_TO_LOCATION")[0];
    // The GM's place and heading.
    assert_eq!((moved.x, moved.y, moved.z, moved.heading), (X, Y, Z + 5, 321));
    assert_eq!(s.pos(USER), (X, Y, Z + 5));

    // Not exactly one name.
    s.cmd(GM, "recall");
    s.cmd(GM, "recall P1 P2");
    s.cmd(GM, "recall  P1");
    let out = s.log.take();
    // Plain `//recall` has no space after the word: Java does nothing.
    assert_eq!(
        texts_to(&out, GM),
        ["Usage: //recall <playername>", "Usage: //recall <playername>"]
    );

    // An offline name would be a database update.
    s.cmd(GM, "recall Sleeper");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["SQLException while changing offline character's position"]
    );
}

#[test]
fn teleport_character_sends_the_target_to_coordinates() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 800);
    s.log.take();

    // No target.
    s.cmd(GM, "teleport_character 1 2 3");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);

    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();
    // A coordinate missing: nothing. A bad number: Java's uncaught exception.
    s.cmd(GM, "teleport_character 1 2");
    assert!(s.log.take().is_empty());
    s.cmd(GM, "teleport_character 1 2 z");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Exception during execution of  'admin_teleport_character 1 2 z': java.lang.NumberFormatException: For input string: \"z\""
        ]
    );
    // Without coordinates: the teleport window.
    s.cmd(GM, "teleport_character");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Wrong or no Coordinates given."]);
    let page = &html_to(&out, GM)[0].html;
    assert!(page.contains("The character you will teleport is P1."));
    assert!(page.contains(&format!("admin_teleport_character {X} {Y} {Z}")));
    s.cmd(GM, "show_teleport");
    assert!(html_to(&s.log.take(), GM)[0].html.contains("P1"));

    s.cmd(GM, "teleport_character -80000 150000 -3000");
    let out = s.log.take();
    assert_eq!(texts_to(&out, USER), ["Admin is teleporting you."]);
    assert_eq!(s.pos(USER), (-80000, 150000, -2995));
    // `new Location(x, y, z)`: heading unchanged.
    assert_eq!(s.heading(USER), 321);

    // The GM itself is no target for it.
    s.world.set_target(ObjectId(GM), Some(ObjectId(GM)));
    s.cmd(GM, "teleport_character 1 2 3");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::YouCannotUseThisOnYourself]
    );
}

#[test]
fn tele_and_the_direction_commands() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    s.cmd(GM, "tele");
    assert!(html_to(&s.log.take(), GM)[0].html.contains("Move panel"));

    s.cmd(GM, "gonorth");
    let out = s.log.take();
    assert!(html_to(&out, GM)[0].html.contains("Move panel"));
    // North is -y, 150 by default.
    assert_eq!(s.pos(GM), (X, Y - 150, Z + 5));
    s.cmd(GM, "goup 1000");
    assert_eq!(s.pos(GM), (X, Y - 150, Z + 5 + 1000 + 5));
    s.log.take();
    s.cmd(GM, "gonorth far");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //go<north|south|east|west|up|down> [offset] (default 150)"]
    );
}

// -------------------------------------------------------------------------------- //goto

#[test]
fn goto_knows_places_and_players() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 900);
    s.log.take();

    s.cmd(GM, "goto Giran");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["You have been teleported."]);
    assert_eq!(s.pos(GM), (82698, 148638, -3473 + 5));

    // Spaces are dropped from the name; the later "jail" wins.
    s.cmd(GM, "goto dark elf village");
    assert_eq!(s.pos(GM), (12428, 16551, -4588 + 5));
    s.cmd(GM, "goto jail");
    assert_eq!(s.pos(GM), (80151, -15445, -1805 + 5));

    // A player: its place and heading.
    s.world.player_mut(ObjectId(USER)).unwrap().core.heading = 99;
    s.cmd(GM, "goto p1");
    assert_eq!(s.pos(GM), (X + 900, Y, Z + 5));
    assert_eq!(s.heading(GM), 99);
}

#[test]
fn goto_an_npc_goes_through_list_positions() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(LOW_GM, 0);
    s.world.spawn_npc_at(2000, X - 500, Y + 100, Z, 55).unwrap();
    s.log.take();

    // `admin_list_positions rat 1`: the first spawn of that NPC.
    s.cmd(GM, "goto rat");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["You have been teleported."]);
    assert_eq!(s.pos(GM), (X - 500, Y + 100, Z + 5));
    assert_eq!(s.heading(GM), 55);

    s.cmd(GM, "goto 2000");
    assert_eq!(texts_to(&s.log.take(), GM), ["You have been teleported."]);

    // Nothing of that name.
    s.cmd(GM, "goto nobody");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["The specified teleport target does not exist."]
    );

    // The inner `list_positions` needs level 100, the level 70 GM has //goto (30) but not that.
    s.cmd(LOW_GM, "goto rat");
    assert_eq!(texts_to(&s.log.take(), LOW_GM), [ACCESS_REFUSED]);
}

#[test]
fn list_positions_lists_and_teleports_by_index() {
    let mut s = setup();
    s.enter(GM, 0);
    s.world.spawn_npc_at(2000, X - 500, Y, Z, 55).unwrap();
    s.world.spawn_npc_at(2000, X - 600, Y, Z, 56).unwrap();
    s.log.take();

    s.cmd(GM, "list_positions Rat");
    let out = s.log.take();
    let lines = texts_to(&out, GM);
    assert_eq!(lines.len(), 2);
    assert!(lines[0].starts_with("1 - Rat (Spawn ID: 2000 X: -71838 Y: 258271 Z: -3104 Heading: 55)"));
    assert!(lines[1].starts_with("2 - Rat"));

    s.cmd(GM, "list_positions 2000 2");
    assert_eq!(texts_to(&s.log.take(), GM), ["You have been teleported."]);
    assert_eq!(s.pos(GM).0, X - 600);
}

// -------------------------------------------------------------------------------- //kill

#[test]
fn kill_removes_the_target_npc_and_only_logs_for_a_player() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    let rat = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    s.log.take();

    // No target.
    s.cmd(GM, "kill");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);

    s.world.set_target(ObjectId(GM), Some(rat));
    s.cmd(GM, "kill");
    assert!(s.world.npc(rat).is_none());
    // Everybody who saw it loses it.
    assert!(names_to(&s.log.take(), USER).contains(&"DELETE_OBJECT".to_owned()));

    // A player: death is M4, the player stays as it is.
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.cmd(GM, "kill");
    s.cmd(GM, "kill P1");
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().core.cur_hp, 100.0);
}

#[test]
fn kill_with_a_radius_or_a_player_name() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    let near = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    let far = s.world.spawn_npc_at(2000, X + 1500, Y, Z, 0).unwrap();
    s.log.take();

    s.cmd(GM, "kill 100");
    assert!(s.world.npc(near).is_none());
    assert!(s.world.npc(far).is_some());
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Killed all characters within a 100 unit radius."]
    );

    // Around another player.
    s.cmd(GM, "kill_monster p1 2000");
    assert!(s.world.npc(far).is_none());
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Killed all characters within a 2000 unit radius."]
    );

    s.cmd(GM, "kill P1 big");
    s.cmd(GM, "kill nonsense");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Invalid radius.", "Usage: //kill <player_name | radius>"]
    );
}

// -------------------------------------------------------------------------------- //heal

#[test]
fn heal_restores_hp_mp_and_cp_of_the_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.player_mut(ObjectId(USER)).unwrap().core.cur_hp = 30.0;
    s.world.player_mut(ObjectId(USER)).unwrap().core.cur_mp = 5.0;
    s.world.player_mut(ObjectId(USER)).unwrap().core.cur_cp = 0.0;
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "heal");
    let core = s.world.player(ObjectId(USER)).unwrap().core.clone();
    assert_eq!((core.cur_hp, core.cur_mp, core.cur_cp), (100.0, 50.0, 10.0));
    // `updateUserInfo()` at once, the `StatusUpdate` of `broadcastStatusUpdate()` 50 ms later.
    s.world.tick();
    let out = s.log.take();
    assert_eq!(
        names_to(&out, USER),
        ["USER_INFO", "EX_BR_EXTRA_USER_INFO", "STATUS_UPDATE"]
    );
    let update = &decoded::<StatusUpdate>(&out, USER, "STATUS_UPDATE")[0];
    assert_eq!(update.object_id, USER);
    assert_eq!(
        update.attributes,
        [
            (status::MAX_HP, 100),
            (status::CUR_HP, 100),
            (status::MAX_MP, 50),
            (status::CUR_MP, 50),
            (status::MAX_CP, 10),
            (status::CUR_CP, 10)
        ]
    );
    assert!(names_to(&out, GM).is_empty());

    // Already full: the user info is sent anyway, a status update is not.
    s.cmd(GM, "heal");
    assert_eq!(
        names_to(&s.log.take(), USER),
        ["USER_INFO", "EX_BR_EXTRA_USER_INFO"]
    );
}

#[test]
fn heal_by_name_by_radius_and_on_the_gm() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    let rat = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    let far_rat = s.world.spawn_npc_at(2000, X + 1900, Y, Z, 0).unwrap();
    for id in [rat, far_rat] {
        s.world.npc_mut(id).unwrap().core.cur_hp = 1.0;
    }
    s.world.player_mut(ObjectId(USER)).unwrap().core.cur_hp = 10.0;
    s.world.player_mut(ObjectId(GM)).unwrap().core.cur_hp = 10.0;
    s.log.take();

    // By name.
    s.cmd(GM, "heal P1");
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().core.cur_hp, 100.0);

    // Without a target: the GM.
    s.cmd(GM, "heal");
    assert_eq!(s.world.player(ObjectId(GM)).unwrap().core.cur_hp, 100.0);

    // A radius: every visible creature but the GM, whatever the number says.
    s.world.player_mut(ObjectId(USER)).unwrap().core.cur_hp = 10.0;
    s.log.take();
    s.cmd(GM, "heal 10");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Healed within 10 unit radius."]);
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().core.cur_hp, 100.0);
    assert_eq!(s.world.npc(rat).unwrap().core.cur_hp, 62.0);
    assert_eq!(s.world.npc(far_rat).unwrap().core.cur_hp, 62.0);
}

#[test]
fn heal_tells_the_players_who_target_the_npc() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    let rat = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    s.world.npc_mut(rat).unwrap().core.cur_hp = 20.0;
    s.world.set_target(ObjectId(USER), Some(rat));
    s.world.set_target(ObjectId(GM), Some(rat));
    s.log.take();

    s.cmd(GM, "heal");
    let out = s.log.take();
    for to in [GM, USER] {
        let update = &decoded::<StatusUpdate>(&out, to, "STATUS_UPDATE")[0];
        assert_eq!(update.object_id, rat.0);
        assert_eq!(update.attributes, [(status::MAX_HP, 62), (status::CUR_HP, 62)]);
    }
    // Nothing changed the second time.
    s.cmd(GM, "heal");
    assert!(s.log.take().is_empty());
}

// -------------------------------------------------------------------------------- messages

#[test]
fn gm_startup_builder_hide_turns_sys_messages_into_chat_lines() {
    let mut s = setup_with(WorldSettings {
        gm_startup_builder_hide: true,
        ..WorldSettings::default()
    });
    s.enter(GM, 0);
    s.log.take();
    s.cmd(GM, "spawn 2000");
    let out = s.log.take();
    // `sendSysMessage`: a "SYS" line in the chat window.
    let lines: Vec<&Vec<u8>> = out
        .iter()
        .filter(|(to, b)| *to == GM && name_of(b) == "SAY2")
        .map(|(_, b)| b)
        .collect();
    assert_eq!(lines.len(), 1);
    let expected = CreatureSay {
        sender_object_id: 0,
        chat_type: ChatType::General,
        sender: SenderField::Name("SYS".to_owned()),
        message_id: -1,
        body: SayBody::Text(format!("Created Rat on {GM}")),
    };
    assert_eq!(*lines[0], l2_net::Encode::to_bytes(&expected));
    // The refusal is a plain `sendMessage`, never a SYS line.
    s.enter(LOW_GM, 0);
    s.log.take();
    s.cmd(LOW_GM, "kill");
    assert_eq!(texts_to(&s.log.take(), LOW_GM), [ACCESS_REFUSED]);
}

// ------------------------------------------------------------------------------- /loc

#[test]
fn loc_without_map_regions_prints_the_coordinates() {
    let mut s = setup();
    s.enter(USER, 0);
    s.log.take();
    // Anyone may use it, GM or not.
    s.input(USER, ClientInput::UserCommand { id: 0 });
    let out = s.log.take();
    let messages = decoded::<SystemMessage>(&out, USER, "SYSTEM_MESSAGE");
    assert_eq!(messages.len(), 1);
    assert_eq!(messages[0].id, SystemMessageId::CurrentLocationS1);
    assert_eq!(
        messages[0].params,
        [SystemMessageParam::Text(format!("{X}, {Y}, {Z}"))]
    );

    // Another user command is not ported: silence.
    s.input(USER, ClientInput::UserCommand { id: 52 });
    assert!(s.log.take().is_empty());
}

#[test]
fn loc_names_the_area_with_the_real_map_regions() {
    let root = std::path::PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game");
    if !root.join("data").join("zones").is_dir() {
        eprintln!("skipped: {} has no data/zones", root.display());
        return;
    }
    let regions = Arc::new(l2_data::MapRegions::load(&root));
    let zones = Arc::new(l2_data::Zones::load(&root));
    let mut s = setup();
    s.world.set_zone_data(zones, Arc::clone(&regions));
    // Talking Island village.
    let mut data = player(USER, 0, 0);
    data.x = -84_318;
    data.y = 244_579;
    s.enter_with(data);
    s.log.take();

    s.input(USER, ClientInput::UserCommand { id: 0 });
    let out = s.log.take();
    let messages = decoded::<SystemMessage>(&out, USER, "SYSTEM_MESSAGE");
    assert_eq!(messages.len(), 1);
    // Java: `new SystemMessage(locId)`, with the player's coordinates for a 3-parameter message.
    let message = &messages[0];
    assert!(message.id.id() > 0);
    assert_ne!(message.id, SystemMessageId::CurrentLocationS1);
    assert_eq!(message.id.param_count(), 3);
    assert_eq!(
        message.params,
        [
            SystemMessageParam::IntNumber(-84_318),
            SystemMessageParam::IntNumber(244_579),
            SystemMessageParam::IntNumber(Z)
        ]
    );
}
