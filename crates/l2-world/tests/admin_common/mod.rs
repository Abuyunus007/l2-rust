#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers

//! The harness of the M7-ADMIN-10 tests: a world with a recording outbox, recording connections,
//! the rights of every command of the handlers, and small admin pages in a temporary datapack.

use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use l2_data::HtmCache;
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ChatType, NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_world::*;

pub type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
pub struct Log(pub Arc<Mutex<Sent>>);

impl Log {
    pub fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

/// Everything the world handed to the outbox, in order.
#[derive(Debug, Clone, PartialEq)]
pub enum Out {
    Persist(PersistCommand),
    Ls(LsEvent),
    Control(ControlRequest),
}

#[derive(Clone, Default)]
pub struct Outs(pub Arc<Mutex<Vec<Out>>>);

impl Outs {
    pub fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Recorder(Outs);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }

    fn ls_event(&mut self, event: LsEvent) {
        self.0.0.lock().unwrap().push(Out::Ls(event));
    }

    fn control(&mut self, request: ControlRequest) {
        self.0.0.lock().unwrap().push(Out::Control(request));
    }
}

/// The reasons the connections were told the player left.
#[derive(Clone, Default)]
pub struct Lefts(pub Arc<Mutex<Vec<(i32, LeaveReason)>>>);

impl Lefts {
    pub fn take(&self) -> Vec<(i32, LeaveReason)> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log, Lefts);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }

    fn left_world(&self, reason: LeaveReason) {
        self.2.0.lock().unwrap().push((self.0, reason));
    }
}

/// Every command word the handlers of the card register.
pub const COMMANDS: &[&str] = &[
    "kick",
    "kick_non_gm",
    "character_disconnect",
    "gm",
    "gmchat",
    "gmchat_menu",
    "snoop",
    "online",
    "serverinfo",
    "server_gm_only",
    "server_all",
    "server_max_player",
    "server_list_type",
    "server_list_age",
    "server_login",
    "server_shutdown",
    "server_restart",
    "server_abort",
    "announce",
    "announce_crit",
    "announce_screen",
    "announces",
    "view_petitions",
    "view_petition",
    "accept_petition",
    "reject_petition",
    "reset_petitions",
    "force_peti",
    "reload",
    "reload_menu",
    "reload_menu_page",
    "forge",
    "forge_values",
    "forge_send",
    "debug",
];

pub fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (70, 0), (100, 70)] {
        rights.add_level(level, child);
    }
    for command in COMMANDS {
        rights.add_command(command, 100, false);
    }
    // For the commands of other cards the tests register stand-ins for.
    for command in ["showdoors", "geogrid", "gmliston"] {
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

pub fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

pub fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

pub fn decoded<P: Decode>(out: &Sent, to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

pub fn count(out: &Sent, to: i32, name: &str) -> usize {
    out.iter().filter(|(t, b)| *t == to && name_of(b) == name).count()
}

/// The text of every `sendMessage` (system message `S1_2` with one string) sent to `to`.
pub fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The system messages (not the plain texts) sent to `to`.
pub fn system_to(out: &Sent, to: i32) -> Vec<SystemMessage> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter(|m| m.id != SystemMessageId::S12)
        .collect()
}

pub fn html_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|m| m.html)
        .collect()
}

/// A `CreatureSay` read back: `(chat type id, sender object id, sender name, text)`. The packet
/// has an encoder only. The "HTML" lines a GM gets with `GMDebugHtmlPaths` are left out.
pub fn says_to(out: &Sent, to: i32) -> Vec<(i32, i32, String, String)> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == "SAY2")
        .map(|(_, b)| {
            let mut r = PacketReader::new(&b[1..]);
            let sender = r.i32().unwrap();
            let chat_type = r.i32().unwrap();
            let name = r.string();
            let _message_id = r.i32().unwrap();
            let text = r.string();
            (chat_type, sender, name, text)
        })
        .filter(|(_, _, name, _)| name != "HTML")
        .collect()
}

pub const X: i32 = -71_338;
pub const Y: i32 = 258_271;
pub const Z: i32 = -3_104;
pub const GM: i32 = 100;
pub const GM2: i32 = 101;
pub const P1: i32 = 1;
pub const P2: i32 = 2;
pub const P3: i32 = 3;

pub const PAGES: &[(&str, &str)] = &[
    (
        "login.htm",
        "<html><body>%server_name%|%status%|%clock%|%brackets%|%max_players%</body></html>",
    ),
    (
        "shutdown.htm",
        "<html><body>count=%count% used=%used% time=%time%</body></html>",
    ),
    ("gm_menu.htm", "<html><body>GM menu</body></html>"),
    (
        "serverinfo.htm",
        "<html><body>%os_name%|%slots%|%gameTime%|%dayNight%|%geodata%|%onlineAll%|%offlineTrade%|%onlineGM%|%onlineReal%|%serverUpTime%</body></html>",
    ),
    (
        "announces-list.htm",
        "<html><body>%announcements%|%pages%</body></html>",
    ),
    (
        "announces-show.htm",
        "<html><body>show %id% %type% %initial% %delay% %repeat% %author% %content%</body></html>",
    ),
    (
        "announces-edit.htm",
        "<html><body>edit %id% %type% %initial% %delay% %repeat% %author% %content%</body></html>",
    ),
    ("announces-add.htm", "<html><body>add</body></html>"),
    (
        "petition.htm",
        "<html><body>%petition%|%time%|%type%|%petitioner%|%online%|%text%</body></html>",
    ),
    ("reload.htm", "<html><body>%buttons%</body></html>"),
    (
        "debug.htm",
        "<html><body>%packets_status%|%packets%|%doors_status%|%doors%|%geodata_status%|%geodata%|%movement_status%|%movement%</body></html>",
    ),
    ("pforge/main.htm", "<html><body>forge main</body></html>"),
    (
        "pforge/values.htm",
        "<html><body>%opformat%|%opcodes%|%format%|%editors%|%send_bypass%</body></html>",
    ),
    ("pforge/inc/editor.htm", "<tr>%format%:%editor_index%</tr>"),
];

pub struct Scene {
    pub world: World,
    pub log: Log,
    pub outs: Outs,
    pub lefts: Lefts,
    pub root: tempfile::TempDir,
}

impl Scene {
    pub fn new() -> Self {
        Self::with(AdminSettings::default())
    }

    pub fn with(admin: AdminSettings) -> Self {
        let root = tempfile::tempdir().unwrap();
        for (path, text) in PAGES {
            let file = root.path().join("data/html/admin").join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        let outs = Outs::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(Recorder(outs.clone())));
        world.set_static_data(Box::new(Fake { rights: rights() }));
        let mut settings = WorldSettings::default();
        settings.teleport.offset_on_teleport_enabled = false;
        settings.teleport.max_offset_on_teleport = 0;
        world.set_settings(settings);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_admin_settings(admin);
        Self {
            world,
            log: Log::default(),
            outs,
            lefts: Lefts::default(),
            root,
        }
    }

    pub fn enter(&mut self, id: i32) {
        self.enter_from(id, None);
    }

    pub fn enter_from(&mut self, id: i32, ip: Option<IpAddr>) {
        let level = if id >= GM { 100 } else { 0 };
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player(id, level)),
            sink: Box::new(Sink(id, self.log.clone(), self.lefts.clone())),
            ip,
        });
    }

    pub fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input,
        });
    }

    /// `//command`.
    pub fn cmd(&mut self, id: i32, command: &str) {
        self.input(
            id,
            ClientInput::BuildCmd {
                command: command.to_owned(),
            },
        );
    }

    /// An `admin_` bypass of a panel.
    pub fn bypass(&mut self, id: i32, command: &str) {
        self.input(
            id,
            ClientInput::Bypass {
                command: command.to_owned(),
            },
        );
    }

    pub fn say(&mut self, id: i32, chat_type: ChatType, text: &str) {
        self.input(
            id,
            ClientInput::Say {
                chat_type: chat_type.client_id(),
                text: text.to_owned(),
                target: None,
            },
        );
    }

    /// `id` clicks `other`, which targets it.
    pub fn target(&mut self, id: i32, other: i32) {
        self.input(
            id,
            ClientInput::Action {
                object_id: other,
                action_id: 0,
            },
        );
    }

    /// Runs the world for `ms` milliseconds and returns what the players were sent.
    pub fn run(&mut self, ms: u64) -> Sent {
        let mut sent = self.log.take();
        for _ in 0..(ms / 50) {
            self.world.tick();
            sent.extend(self.log.take());
        }
        sent
    }

    pub fn is_online(&self, id: i32) -> bool {
        self.world.player(ObjectId(id)).is_some()
    }

    /// The LS events since the last call.
    pub fn ls_events(&self) -> Vec<LsEvent> {
        self.outs
            .take()
            .into_iter()
            .filter_map(|o| match o {
                Out::Ls(e) => Some(e),
                _ => None,
            })
            .collect()
    }
}

pub fn player(id: i32, access_level: i32) -> PlayerData {
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
        x: X + id,
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
    }
}
