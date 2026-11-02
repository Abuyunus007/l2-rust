#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The console GM of G-03: the admin GUI runs `//commands` as a virtual GM called "GUI" and gets
//! the system messages back as text.
//! Java: handler/AdminCommandHandler.java, scripts/handlers/chat/commands/admin/{AdminKick,
//! AdminPunishment,AdminAnnouncements,AdminShutdown,AdminServerInfo}.java, gameserver/Shutdown.java.

use std::sync::{Arc, Mutex, mpsc};

use l2_data::HtmCache;
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::TeleportToLocation;
use l2_world::punishments::{PunishmentAffect, PunishmentType};
use l2_world::*;

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

#[derive(Clone, Default)]
struct Outs(Arc<Mutex<Vec<Out>>>);

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Persist(PersistCommand),
    Control(ControlRequest),
}

struct Recorder(Outs);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }

    fn ls_event(&mut self, _event: LsEvent) {}

    fn control(&mut self, request: ControlRequest) {
        self.0.0.lock().unwrap().push(Out::Control(request));
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }

    fn left_world(&self, _reason: LeaveReason) {}
}

struct Fake {
    rights: AdminRights,
    master: i32,
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
    fn master_access_level(&self) -> i32 {
        self.master
    }
}

fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (70, 0), (100, 70)] {
        rights.add_level(level, child);
    }
    for command in [
        "kick",
        "announce",
        "announce_crit",
        "serverinfo",
        "server_shutdown",
        "server_restart",
        "server_abort",
        "punishment_add",
        "teleport_character",
    ] {
        rights.add_command(command, 100, false);
    }
    rights
}

const P1: i32 = 1;
const P2: i32 = 2;

struct Scene {
    world: World,
    log: Log,
    outs: Outs,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Self {
        Self::with_master(100)
    }

    fn with_master(master: i32) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write("data/html/jail_in.htm", "<html><body>Jailed.</body></html>");
        write("data/html/jail_out.htm", "<html><body>Free.</body></html>");
        write(
            "data/html/admin/serverinfo.htm",
            "<html><body>Slots: %slots%<br>Online: %onlineAll%</body></html>",
        );
        let outs = Outs::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(Recorder(outs.clone())));
        world.set_static_data(Box::new(Fake {
            rights: rights(),
            master,
        }));
        let mut settings = WorldSettings::default();
        settings.teleport.offset_on_teleport_enabled = false;
        settings.teleport.max_offset_on_teleport = 0;
        world.set_settings(settings);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        Self {
            world,
            log: Log::default(),
            outs,
            _root: root,
        }
    }

    fn enter(&mut self, id: i32) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player(id)),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    /// Sends a request through the world's command channel and returns the answer.
    fn gui(&mut self, request: GuiRequest) -> GuiReply {
        let (tx, rx) = mpsc::channel();
        self.world.handle(WorldCommand::Admin(AdminQuery::Gui(
            request,
            Box::new(move |reply| {
                let _ = tx.send(reply);
            }),
        )));
        rx.try_recv().expect("the world answers at once")
    }

    fn command(&mut self, command: &str, with_html: bool) -> Vec<String> {
        match self.gui(GuiRequest::Command {
            command: command.to_owned(),
            with_html,
        }) {
            GuiReply::Lines(lines) => lines,
            other => panic!("{other:?}"),
        }
    }

    fn console(&mut self, command: &str) -> Vec<String> {
        self.command(command, true)
    }

    fn info(&mut self) -> ServerInfo {
        match self.gui(GuiRequest::Info) {
            GuiReply::Info(info) => info,
            other => panic!("{other:?}"),
        }
    }

    fn run(&mut self, ms: u64) -> Sent {
        let mut sent = self.log.take();
        for _ in 0..(ms / 50) {
            self.world.tick();
            sent.extend(self.log.take());
        }
        sent
    }

    fn is_online(&self, id: i32) -> bool {
        self.world.player(ObjectId(id)).is_some()
    }

    fn controls(&self) -> Vec<ControlRequest> {
        self.outs
            .0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|o| match o {
                Out::Control(c) => Some(c.clone()),
                Out::Persist(_) => None,
            })
            .collect()
    }
}

fn player(id: i32) -> PlayerData {
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
        access_level: 0,
        x: -71_338 + id,
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
    }
}

fn opcode_name(body: &[u8]) -> String {
    ServerOpcode::from_id(body[0], None).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn says_to(sent: &Sent, to: i32) -> Vec<&Vec<u8>> {
    sent.iter()
        .filter(|(t, b)| *t == to && opcode_name(b) == "SAY2")
        .map(|(_, b)| b)
        .collect()
}

// ------------------------------------------------------------------------------ tests

#[test]
fn the_console_kicks_and_hears_the_answer() {
    let mut s = Scene::new();
    s.enter(P1);
    s.enter(P2);
    assert_eq!(s.console("kick P2"), ["You kicked P2 from the game."]);
    assert!(!s.is_online(P2));
    assert!(s.is_online(P1));
    // A leading slash pair is accepted: the box is for people who type `//kick`.
    assert_eq!(s.console("//kick Nobody"), Vec::<String>::new());
}

#[test]
fn the_virtual_gm_is_never_left_in_the_world() {
    let mut s = Scene::new();
    s.enter(P1);
    s.console("serverinfo");
    assert_eq!(s.world.online_count(), 1);
    assert!(s.world.player(GUI_GM_ID).is_none());
    s.command("kick P1", false);
    assert_eq!(s.world.online_count(), 0);
}

#[test]
fn an_unknown_command_gets_javas_answer() {
    let mut s = Scene::new();
    assert_eq!(s.console("nonsense"), ["The command 'nonsense' does not exist!"]);
    assert_eq!(s.console("  "), ["Enter a command, for example: server_abort"]);
}

#[test]
fn without_a_master_level_the_gui_says_so() {
    let mut s = Scene::with_master(0);
    let lines = s.console("serverinfo");
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("The GUI has no GM rights"));
}

#[test]
fn html_pages_come_back_as_plain_text() {
    let mut s = Scene::new();
    s.enter(P1);
    // The first line is Java's GM debug line for an html file; the virtual GM counts as online
    // while the command runs, like a real GM does.
    assert_eq!(
        s.console("serverinfo"),
        ["HTML: html/admin/serverinfo.htm", "Slots: 2/2000", "Online: 2"]
    );
    // The buttons do not want the page, only the messages around it.
    assert_eq!(
        s.command("serverinfo", false),
        ["HTML: html/admin/serverinfo.htm"]
    );
}

#[test]
fn announcements_reach_every_player() {
    let mut s = Scene::new();
    s.enter(P1);
    s.enter(P2);
    s.log.take();
    s.console("announce Hello all");
    s.console("announce_crit Big news");
    let sent = s.log.take();
    assert_eq!(says_to(&sent, P1).len(), 2);
    assert_eq!(says_to(&sent, P2).len(), 2);
}

#[test]
fn jail_and_bans_use_punishment_add_with_gui_as_the_actor() {
    let mut s = Scene::new();
    s.enter(P1);
    s.enter(P2);
    s.run(100);
    s.console("punishment_add P1 CHARACTER JAIL 30 caught botting");
    assert!(s.world.is_jailed(ObjectId(P1)));
    let expiry = s
        .world
        .punishment_expiration("1", PunishmentAffect::Character, PunishmentType::Jail);
    assert!(expiry > 0);

    s.console("punishment_add P2 CHARACTER BAN 0 swearing");
    assert!(!s.is_online(P2));
    let rows: Vec<_> = s
        .outs
        .0
        .lock()
        .unwrap()
        .iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::StorePunishment(p)) => Some(p.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(rows.len(), 2);
    assert!(rows.iter().all(|r| r.punished_by == GUI_GM_NAME));
    assert_eq!(rows[0].reason, "caught botting");
    assert_eq!(
        (rows[0].kind, rows[1].kind),
        (PunishmentType::Jail, PunishmentType::Ban)
    );
}

#[test]
fn teleport_moves_the_named_character() {
    let mut s = Scene::new();
    s.enter(P1);
    s.run(100);
    s.log.take();
    let reply = s.gui(GuiRequest::Teleport {
        name: "p1".into(),
        x: 82698,
        y: 148638,
        z: -3473,
    });
    assert_eq!(reply, GuiReply::Lines(vec![]));
    let sent = s.log.take();
    let moved: Vec<(i32, i32, i32)> = sent
        .iter()
        .filter(|(t, b)| *t == P1 && opcode_name(b) == "TELEPORT_TO_LOCATION")
        .map(|(_, b)| {
            let p = TeleportToLocation::decode(&mut PacketReader::new(&b[1..])).unwrap();
            (p.x, p.y, p.z)
        })
        .collect();
    // `teleToLocation` adds 5 to the height the client is told.
    assert_eq!(moved, [(82698, 148638, -3473 + 5)]);
    assert_eq!(
        s.gui(GuiRequest::Teleport {
            name: "Nobody".into(),
            x: 0,
            y: 0,
            z: 0
        }),
        GuiReply::Lines(vec!["Nobody is not online.".into()])
    );
}

#[test]
fn a_private_message_is_a_whisper_from_the_gm() {
    let mut s = Scene::new();
    s.enter(P1);
    s.log.take();
    let reply = s.gui(GuiRequest::Message {
        name: "P1".into(),
        text: "hello".into(),
    });
    assert_eq!(reply, GuiReply::Lines(vec!["Message sent to P1.".into()]));
    let sent = s.log.take();
    let says = says_to(&sent, P1);
    assert_eq!(says.len(), 1);
    // The UTF-16 sender "[GM]" is in the packet.
    let gm: Vec<u8> = "[GM]".encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert!(says[0].windows(gm.len()).any(|w| w == gm.as_slice()));
}

#[test]
fn the_shutdown_countdown_announces_and_ends_with_a_control_request() {
    let mut s = Scene::new();
    s.enter(P1);
    s.log.take();
    let lines = s.console("server_shutdown 3");
    assert_eq!(
        lines,
        ["The server will be coming down in 3 second(s). Please find a safe place to log out."]
    );
    assert_eq!(s.info().shutdown_in_secs, Some(2));
    assert!(s.controls().is_empty());
    s.run(4_500);
    assert_eq!(s.controls(), [ControlRequest::Shutdown { restart: false }]);

    let mut s = Scene::new();
    s.console("server_restart 1");
    s.run(2_500);
    assert_eq!(s.controls(), [ControlRequest::Shutdown { restart: true }]);
}

#[test]
fn abort_stops_the_countdown() {
    let mut s = Scene::new();
    s.enter(P1);
    s.console("server_shutdown 120");
    s.run(2_000);
    let lines = s.console("server_abort");
    // NOTE(java-quirk): the mode is already ABORT when Java builds the text.
    assert_eq!(
        lines,
        ["[announcement] Server aborts aborting and continues normal operation!"]
    );
    s.run(3_000);
    assert!(s.controls().is_empty());
    assert_eq!(s.info().shutdown_in_secs, None);
}

#[test]
fn server_info_counts_the_players() {
    let mut s = Scene::new();
    s.enter(P1);
    s.enter(P2);
    s.run(1_000);
    let info = s.info();
    assert_eq!(info.online, 2);
    assert_eq!(info.tick_ms, 50);
    assert_eq!(info.ticks, 20);
    assert_eq!(info.uptime_secs, 1);
}

#[test]
fn the_player_list_carries_clan_and_place() {
    let mut s = Scene::new();
    s.enter(P1);
    let (tx, rx) = mpsc::channel();
    s.world
        .handle(WorldCommand::Admin(AdminQuery::OnlinePlayers(Box::new(
            move |list| {
                let _ = tx.send(list);
            },
        ))));
    let list = rx.try_recv().unwrap();
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].clan, None);
    assert_eq!((list[0].x, list[0].y, list[0].z), (-71_337, 258_271, -3_104));
}
