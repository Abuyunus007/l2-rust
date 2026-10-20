#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Punishments: the `punishments` table rows the world loads, the chat ban, the ban, the jail,
//! the penalty of an illegal action and the admin commands that set them.
//! Java: managers/PunishmentManager.java, mechanics/punishment/*, entity/actor/tasks/player/
//! IllegalPlayerActionTask.java, scripts/handlers/punishments/{BanHandler,ChatBanHandler,
//! JailHandler}.java, scripts/handlers/chat/commands/admin/AdminPunishment.java.
//!
//! The numbers and texts are the ones of the Java sources: five seconds before an illegal
//! action is punished, two seconds before a jail teleport, `JAIL_IN_LOC` / `JAIL_OUT_LOC`.

use std::sync::{Arc, Mutex};

use l2_data::HtmCache;
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ChatType, EtcStatusUpdate, NpcHtmlMessage, SystemMessage, SystemMessageParam, TeleportToLocation,
};
use l2_world::punishments::{JAIL_IN, JAIL_OUT};
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

/// Every message the world handed to the outbox, in order.
#[derive(Debug, Clone, PartialEq)]
enum Out {
    Persist(PersistCommand),
    Ls(LsEvent),
}

#[derive(Clone, Default)]
struct Outs(Arc<Mutex<Vec<Out>>>);

impl Outs {
    fn take(&self) -> Vec<Out> {
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
}

/// The reasons the connections were told the player left.
#[derive(Clone, Default)]
struct Lefts(Arc<Mutex<Vec<(i32, LeaveReason)>>>);

struct Sink(i32, Log, Lefts);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }

    fn left_world(&self, reason: LeaveReason) {
        self.2.0.lock().unwrap().push((self.0, reason));
    }
}

fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (70, 0), (100, 70)] {
        rights.add_level(level, child);
    }
    for command in [
        "punishment",
        "punishment_add",
        "punishment_remove",
        "ban_acc",
        "unban_acc",
        "ban_chat",
        "unban_chat",
        "ban_char",
        "unban_char",
        "jail",
        "unjail",
        "recall",
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

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn decoded<P: Decode>(out: &Sent, to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

fn count(out: &Sent, to: i32, name: &str) -> usize {
    out.iter().filter(|(t, b)| *t == to && name_of(b) == name).count()
}

/// The text of every `sendMessage` sent to `to`.
fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn system_ids_to(out: &Sent, to: i32) -> Vec<SystemMessageId> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter(|m| m.id != SystemMessageId::S12)
        .map(|m| m.id)
        .collect()
}

fn html_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|m| m.html)
        .collect()
}

fn etc_status_to(out: &Sent, to: i32) -> Vec<bool> {
    decoded::<EtcStatusUpdate>(out, to, "ETC_STATUS_UPDATE")
        .into_iter()
        .map(|p| p.chat_blocked)
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const GM: i32 = 100;
const P1: i32 = 1;
const P2: i32 = 2;

struct Scene {
    world: World,
    log: Log,
    outs: Outs,
    lefts: Lefts,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Self {
        Self::with_zones("")
    }

    /// A scene whose zone file holds `zone_xml` (`<zone>` elements).
    fn with_zones(zone_xml: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/html/jail_in.htm",
            "<html><body>Jailed. Reason: %reason%. By: %punishedBy%.</body></html>",
        );
        write("data/html/jail_out.htm", "<html><body>Free.</body></html>");
        write(
            "data/html/admin/punishments/punishment-info.htm",
            "<html><body>%affect_type% %player_name%: %punishments%</body></html>",
        );
        write(
            "data/zones/test.xml",
            &format!(
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n{zone_xml}\n</list>\n"
            ),
        );
        let outs = Outs::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(Recorder(outs.clone())));
        world.set_static_data(Box::new(Fake { rights: rights() }));
        let mut settings = WorldSettings::default();
        // Exact places for the jail teleports.
        settings.teleport.offset_on_teleport_enabled = false;
        settings.teleport.max_offset_on_teleport = 0;
        world.set_settings(settings);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_zone_data(
            Arc::new(l2_data::Zones::load(root.path())),
            Arc::new(l2_data::MapRegions::load(root.path())),
        );
        Self {
            world,
            log: Log::default(),
            outs,
            lefts: Lefts::default(),
            _root: root,
        }
    }

    fn enter(&mut self, id: i32) {
        self.enter_from(id, None);
    }

    fn enter_from(&mut self, id: i32, ip: Option<std::net::IpAddr>) {
        let level = if id == GM { 100 } else { 0 };
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player(id, level)),
            sink: Box::new(Sink(id, self.log.clone(), self.lefts.clone())),
            ip,
        });
    }

    fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input,
        });
    }

    fn cmd(&mut self, id: i32, command: &str) {
        self.input(
            id,
            ClientInput::BuildCmd {
                command: command.to_owned(),
            },
        );
    }

    fn say(&mut self, id: i32, chat_type: ChatType, text: &str) {
        self.input(
            id,
            ClientInput::Say {
                chat_type: chat_type.client_id(),
                text: text.to_owned(),
                target: None,
            },
        );
    }

    /// Runs the world for `ms` milliseconds and returns what the players were sent.
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
}

fn player(id: i32, access_level: i32) -> PlayerData {
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

fn now_ms() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

fn record(id: i32, key: &str, affect: &str, kind: &str, expiration: i64) -> PunishmentRecord {
    PunishmentRecord {
        id,
        key: key.into(),
        affect: affect.into(),
        kind: kind.into(),
        expiration,
        reason: "stored reason".into(),
        punished_by: "Java GM".into(),
    }
}

fn stored(outs: &[Out]) -> Vec<&StoredPunishment> {
    outs.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::StorePunishment(p)) => Some(p),
            _ => None,
        })
        .collect()
}

/// Where `teleToLocation` sends the client: Java adds 5 to the height.
fn at((x, y, z): (i32, i32, i32)) -> (i32, i32, i32) {
    (x, y, z + 5)
}

fn teleports(out: &Sent, to: i32) -> Vec<(i32, i32, i32)> {
    decoded::<TeleportToLocation>(out, to, "TELEPORT_TO_LOCATION")
        .into_iter()
        .map(|t| (t.x, t.y, t.z))
        .collect()
}

// ------------------------------------------------------------------------------ the rows

#[test]
fn loading_keeps_the_active_rows_and_skips_the_rest() {
    let mut s = Scene::new();
    let now = now_ms();
    s.world.load_punishments(vec![
        record(1, "10", "CHARACTER", "CHAT_BAN", 0),
        record(2, "11", "CHARACTER", "JAIL", now + 3_600_000),
        // Expired.
        record(3, "12", "CHARACTER", "BAN", now - 1000),
        // Not a punishment of this server.
        record(4, "13", "CLAN", "BAN", 0),
        record(5, "14", "CHARACTER", "MUTE", 0),
        // `getByName` ignores the case.
        record(6, "acc7", "account", "ban", -1),
    ]);
    let w = &s.world;
    assert!(w.has_punishment("10", PunishmentAffect::Character, PunishmentType::ChatBan));
    assert!(w.has_punishment("11", PunishmentAffect::Character, PunishmentType::Jail));
    assert!(w.has_punishment("acc7", PunishmentAffect::Account, PunishmentType::Ban));
    assert!(!w.has_punishment("12", PunishmentAffect::Character, PunishmentType::Ban));
    assert!(!w.has_punishment("13", PunishmentAffect::Character, PunishmentType::Ban));
    assert!(!w.has_punishment("14", PunishmentAffect::Character, PunishmentType::Ban));
    assert_eq!(
        w.punishment_expiration("11", PunishmentAffect::Character, PunishmentType::Jail),
        now + 3_600_000
    );
    assert_eq!(
        w.punishment_expiration("10", PunishmentAffect::Character, PunishmentType::ChatBan),
        0
    );
    // Nothing was written: loading is reading.
    assert!(s.outs.take().is_empty());
}

#[test]
fn a_loaded_punishment_ends_by_itself_and_updates_its_own_row() {
    let mut s = Scene::new();
    s.world
        .load_punishments(vec![record(42, "10", "CHARACTER", "CHAT_BAN", now_ms() + 1000)]);
    s.run(900);
    assert!(
        s.world
            .has_punishment("10", PunishmentAffect::Character, PunishmentType::ChatBan)
    );
    s.run(300);
    assert!(
        !s.world
            .has_punishment("10", PunishmentAffect::Character, PunishmentType::ChatBan)
    );
    // `UPDATE punishments SET expiration = <now> WHERE id = 42`.
    let outs = s.outs.take();
    match outs.as_slice() {
        [
            Out::Persist(PersistCommand::EndPunishment {
                id: 42,
                key,
                affect: PunishmentAffect::Character,
                kind: PunishmentType::ChatBan,
                ended_at,
            }),
        ] => {
            assert_eq!(key, "10");
            assert!((now_ms() - ended_at).abs() < 10_000);
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn a_banned_character_account_or_address_does_not_enter() {
    let ip: std::net::IpAddr = "10.1.2.3".parse().unwrap();
    for (key, affect) in [("1", "CHARACTER"), ("acc1", "ACCOUNT"), ("10.1.2.3", "IP")] {
        let mut s = Scene::new();
        s.world.load_punishments(vec![record(1, key, affect, "BAN", 0)]);
        s.enter_from(P1, Some(ip));
        assert!(!s.is_online(P1), "{affect}");
        // The client is closed and the account is released like at a logout.
        assert_eq!(count(&s.log.take(), P1, "SERVER_CLOSE"), 1, "{affect}");
        assert_eq!(s.lefts.0.lock().unwrap().as_slice(), [(P1, LeaveReason::Kicked)]);
        assert_eq!(
            s.outs.take(),
            [Out::Persist(PersistCommand::PlayerLeft {
                account: "acc1".into(),
                object_id: ObjectId(P1),
                logout: true,
            })],
            "{affect}"
        );
    }
    // Another address is let in.
    let mut s = Scene::new();
    s.world
        .load_punishments(vec![record(1, "10.9.9.9", "IP", "BAN", 0)]);
    s.enter_from(P1, Some(ip));
    assert!(s.is_online(P1));
}

// ------------------------------------------------------------------------------ chat ban

#[test]
fn the_chat_ban_command_bans_stores_and_lifts() {
    let mut s = Scene::new();
    s.enter(GM);
    s.enter(P1);
    s.enter(P2);
    s.run(200);
    s.outs.take();

    s.cmd(GM, "ban_chat P1");
    let out = s.log.take();
    // ChatBanHandler: forever (no expiration), then `EtcStatusUpdate`.
    assert_eq!(texts_to(&out, P1), ["You've been chat banned forever."]);
    // NOTE(java-quirk): the task is not in the holder yet when the icon is sent.
    assert_eq!(etc_status_to(&out, P1), [false]);
    assert_eq!(
        texts_to(&out, GM)[0],
        "Punishment CHAT_BAN have been applied to: CHARACTER P1!"
    );
    // The panel of the character comes after the command.
    assert_eq!(html_to(&out, GM).len(), 1);
    assert!(html_to(&out, GM)[0].contains("CHAT_BAN"));
    assert!(s.world.is_chat_banned(ObjectId(P1)));
    assert!(!s.world.is_chat_banned(ObjectId(P2)));
    // The row: key = the character id, as Java stores it.
    let outs = s.outs.take();
    assert_eq!(
        stored(&outs),
        [&StoredPunishment {
            key: "1".into(),
            affect: PunishmentAffect::Character,
            kind: PunishmentType::ChatBan,
            expiration: 0,
            reason: "Chat banned by admin".into(),
            punished_by: "P100".into(),
        }]
    );

    // Everything the player says is refused (`CHATTING_IS_CURRENTLY_PROHIBITED`).
    s.say(P1, ChatType::General, "hello");
    s.say(P1, ChatType::Shout, "hello");
    let out = s.run(100);
    assert_eq!(count(&out, P2, "SAY2"), 0);
    assert_eq!(
        system_ids_to(&out, P1),
        [
            SystemMessageId::ChattingIsCurrentlyProhibited,
            SystemMessageId::ChattingIsCurrentlyProhibited
        ]
    );
    // The punishment is the character's own: the other player talks.
    s.say(P2, ChatType::General, "hi");
    assert_eq!(count(&s.run(100), P1, "SAY2"), 1);

    // A second ban of the same kind is refused.
    s.cmd(GM, "ban_chat P1");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Target is already affected by that punishment."]
    );
    assert!(stored(&s.outs.take()).is_empty());

    s.cmd(GM, "unban_chat P1");
    let out = s.log.take();
    assert_eq!(system_ids_to(&out, P1), [SystemMessageId::ChatEnabled]);
    // NOTE(java-quirk): the task leaves the holder after its handler ran, so the icon still
    // shows the ban.
    assert_eq!(etc_status_to(&out, P1), [true]);
    assert!(!s.world.is_chat_banned(ObjectId(P1)));
    let outs = s.outs.take();
    assert!(matches!(
        outs.as_slice(),
        [Out::Persist(PersistCommand::EndPunishment {
            id: 0,
            affect: PunishmentAffect::Character,
            kind: PunishmentType::ChatBan,
            ..
        })]
    ));
    s.say(P1, ChatType::General, "free");
    assert_eq!(count(&s.run(100), P2, "SAY2"), 1);

    s.cmd(GM, "unban_chat P1");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Target is not affected by that punishment!"]
    );
}

#[test]
fn a_timed_chat_ban_tells_the_time_and_ends_on_the_wheel() {
    let mut s = Scene::new();
    s.enter(GM);
    s.enter(P1);
    s.run(100);
    s.log.take();
    s.outs.take();

    s.cmd(GM, "punishment_add P1 CHARACTER CHAT_BAN 1 spamming <b>");
    let out = s.log.take();
    assert_eq!(system_ids_to(&out, P1), [SystemMessageId::ChatDisabled]);
    assert_eq!(texts_to(&out, P1), ["You've been chat banned for 1 minute."]);
    let outs = s.outs.take();
    let row = stored(&outs)[0];
    // The reason has its angle brackets escaped like Java does.
    assert_eq!(row.reason, "spamming &lt;b&gt;");
    assert!(row.expiration > now_ms() && row.expiration <= now_ms() + 60_000);

    s.run(59_000);
    assert!(s.world.is_chat_banned(ObjectId(P1)));
    let out = s.run(2_000);
    assert!(!s.world.is_chat_banned(ObjectId(P1)));
    assert_eq!(system_ids_to(&out, P1), [SystemMessageId::ChatEnabled]);
    assert!(matches!(
        s.outs.take().as_slice(),
        [Out::Persist(PersistCommand::EndPunishment { .. })]
    ));
}

#[test]
fn a_long_chat_ban_keeps_javas_plurals() {
    let mut s = Scene::new();
    s.enter(GM);
    s.enter(P1);
    s.log.take();
    // NOTE(java-quirk): "2 hour's." and "1 hour." followed by " and ".
    s.cmd(GM, "punishment_add P1 CHARACTER CHAT_BAN 150 long one");
    assert_eq!(
        texts_to(&s.log.take(), P1),
        ["You've been chat banned for 2 hour's. and 30 minute's."]
    );
}

#[test]
fn an_account_chat_ban_reaches_every_character_of_the_account() {
    let mut s = Scene::new();
    s.world
        .load_punishments(vec![record(1, "acc2", "ACCOUNT", "CHAT_BAN", 0)]);
    s.enter(P1);
    let first = s.log.take();
    s.enter(P2);
    let second = s.log.take();
    assert!(!s.world.is_chat_banned(ObjectId(P1)));
    assert!(s.world.is_chat_banned(ObjectId(P2)));
    // The chat ban shows in the status icons of the enter world sequence.
    assert_eq!(etc_status_to(&first, P1), [false]);
    assert_eq!(etc_status_to(&second, P2), [true]);
}

// ------------------------------------------------------------------------------ bans

#[test]
fn banning_a_character_kicks_it_and_writes_the_access_level_after_the_store() {
    let mut s = Scene::new();
    s.enter(GM);
    s.enter(P1);
    s.run(100);
    s.log.take();
    s.outs.take();

    s.cmd(GM, "ban_char P1");
    assert!(!s.is_online(P1));
    let out = s.log.take();
    assert_eq!(count(&out, P1, "LOG_OUT_OK"), 1);
    assert_eq!(s.lefts.0.lock().unwrap().as_slice(), [(P1, LeaveReason::Logout)]);
    // Java's order: insert the row, store the character (`storeAndDeleteWith`), then
    // `changeCharAccessLevel(-1)`.
    let kinds: Vec<&str> = s
        .outs
        .take()
        .iter()
        .map(|o| match o {
            Out::Persist(PersistCommand::StorePunishment(p)) => {
                assert_eq!(
                    (p.kind, p.reason.as_str()),
                    (PunishmentType::Ban, "Banned by admin")
                );
                "row"
            }
            Out::Persist(PersistCommand::StorePlayer(_)) => "store",
            Out::Persist(PersistCommand::PlayerLeft { .. }) => "left",
            Out::Persist(PersistCommand::CharAccessLevel {
                char_id: 1,
                level: -1,
            }) => "level",
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(kinds, ["row", "store", "left", "level"]);

    // The character is offline now; it is found by name from the table of names.
    s.cmd(GM, "unban_char P1");
    let outs = s.outs.take();
    assert!(outs.contains(&Out::Persist(PersistCommand::CharAccessLevel {
        char_id: 1,
        level: 0
    })));
    assert!(
        outs.iter()
            .any(|o| matches!(o, Out::Persist(PersistCommand::EndPunishment { .. })))
    );
    assert!(
        !s.world
            .has_punishment("1", PunishmentAffect::Character, PunishmentType::Ban)
    );
}

#[test]
fn banning_an_account_tells_the_login_server() {
    let mut s = Scene::new();
    s.enter(GM);
    s.enter(P1);
    s.outs.take();
    s.cmd(GM, "ban_acc acc1");
    assert!(!s.is_online(P1));
    let outs = s.outs.take();
    assert!(outs.contains(&Out::Ls(LsEvent::AccessLevel {
        account: "acc1".into(),
        level: -1
    })));
    // No `characters` write for an account ban.
    assert!(
        !outs
            .iter()
            .any(|o| matches!(o, Out::Persist(PersistCommand::CharAccessLevel { .. })))
    );
    s.cmd(GM, "unban_acc acc1");
    assert!(s.outs.take().contains(&Out::Ls(LsEvent::AccessLevel {
        account: "acc1".into(),
        level: 0
    })));
}

#[test]
fn an_unknown_name_and_a_loopback_address_are_refused() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    s.outs.take();
    s.cmd(GM, "ban_char Nobody");
    let out = s.log.take();
    let messages = decoded::<SystemMessage>(&out, GM, "SYSTEM_MESSAGE");
    assert_eq!(messages[0].id, SystemMessageId::S1DoesNotExist);
    assert_eq!(messages[0].params, [SystemMessageParam::Text("Nobody".into())]);

    s.cmd(GM, "punishment_add 127.0.0.1 IP BAN 0 test");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "You've entered an incorrect IP address!",
            "You cannot ban any local address!"
        ]
    );
    s.cmd(GM, "punishment_add 1.2.3.4 IP BAN x test");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Incorrect value specified for expiration time!"]
    );
    s.cmd(GM, "punishment_add 1.2.3.4 IP");
    assert_eq!(texts_to(&s.log.take(), GM), ["Please fill all the fields!"]);
    s.cmd(GM, "punishment_add 1.2.3.4 CLAN BAN 0 test");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Incorrect value specified for affect/punishment type!"]
    );
    assert!(s.outs.take().is_empty());
}

#[test]
fn an_ip_ban_kicks_everybody_on_that_address() {
    let mut s = Scene::new();
    let ip: std::net::IpAddr = "10.1.2.3".parse().unwrap();
    s.enter(GM);
    s.enter_from(P1, Some(ip));
    s.enter_from(P2, Some("10.9.9.9".parse().unwrap()));
    s.cmd(GM, "punishment_add 10.1.2.3 IP BAN 0 shared");
    assert!(!s.is_online(P1));
    assert!(s.is_online(P2));
}

// ------------------------------------------------------------------------------ jail

#[test]
fn jailing_teleports_after_two_seconds_and_unjailing_frees() {
    let mut s = Scene::new();
    s.enter(GM);
    s.enter(P1);
    s.run(100);
    s.log.take();

    s.cmd(GM, "jail P1");
    let out = s.log.take();
    assert_eq!(
        html_to(&out, P1),
        ["<html><body>Jailed. Reason: Jailed by admin. By: P100.</body></html>"]
    );
    assert_eq!(texts_to(&out, P1), ["You've been jailed forever."]);
    assert!(s.world.is_jailed(ObjectId(P1)));
    assert!(teleports(&out, P1).is_empty());
    // `ThreadPool.schedule(new TeleportTask(player, JailZone.getLocationIn()), 2000)`.
    assert!(teleports(&s.run(1_900), P1).is_empty());
    let out = s.run(200);
    assert_eq!(teleports(&out, P1), [at(JAIL_IN)]);

    // A jailed player cannot whisper, shout or trade (`JailDisableChat`).
    s.say(P1, ChatType::Shout, "let me out");
    let out = s.run(100);
    assert_eq!(
        texts_to(&out, P1),
        ["You can not chat with players outside of the jail."]
    );
    assert_eq!(count(&out, GM, "SAY2"), 0);

    s.cmd(GM, "unjail P1");
    let out = s.log.take();
    assert_eq!(html_to(&out, P1), ["<html><body>Free.</body></html>"]);
    assert!(!s.world.is_jailed(ObjectId(P1)));
    assert_eq!(teleports(&s.run(2_100), P1), [at(JAIL_OUT)]);
}

#[test]
fn a_jailed_character_is_put_back_in_jail_when_it_logs_in() {
    let mut s = Scene::new();
    s.world
        .load_punishments(vec![record(9, "1", "CHARACTER", "JAIL", now_ms() + 3_600_000)]);
    s.enter(P1);
    let out = s.log.take();
    // The page of the login hook has no reason (`task == null`) and no duration line.
    assert_eq!(
        html_to(&out, P1),
        ["<html><body>Jailed. Reason: . By: .</body></html>"]
    );
    assert!(s.world.is_jailed(ObjectId(P1)));
    assert_eq!(teleports(&s.run(2_100), P1), [at(JAIL_IN)]);
}

// ------------------------------------------------------------------------------ illegal actions

fn settings_with(default_punish: IllegalActionPunishment) -> PunishmentSettings {
    PunishmentSettings {
        default_punish,
        default_punish_param: 600,
        ..PunishmentSettings::default()
    }
}

#[test]
fn kick_tells_the_player_and_kicks_after_five_seconds() {
    let mut s = Scene::new();
    s.enter(P1);
    s.log.take();
    s.world
        .handle_illegal_player_action(ObjectId(P1), "cheat", IllegalActionPunishment::Kick);
    assert_eq!(
        texts_to(&s.log.take(), P1),
        ["You will be kicked for illegal action, GM informed."]
    );
    s.run(4_900);
    assert!(s.is_online(P1));
    let out = s.run(200);
    assert!(!s.is_online(P1));
    assert_eq!(count(&out, P1, "LOG_OUT_OK"), 1);
}

#[test]
fn a_gm_is_never_punished() {
    let mut s = Scene::new();
    s.enter(GM);
    s.log.take();
    s.world
        .handle_illegal_player_action(ObjectId(GM), "cheat", IllegalActionPunishment::Kick);
    s.run(6_000);
    assert!(s.is_online(GM));
}

#[test]
fn kickban_bans_the_character_and_the_account_and_stores_the_row() {
    let mut s = Scene::new();
    s.world
        .set_punishment_settings(settings_with(IllegalActionPunishment::KickBan));
    s.enter(P1);
    s.log.take();
    s.outs.take();
    s.world
        .handle_illegal_player_action(ObjectId(P1), "bad packet", IllegalActionPunishment::KickBan);
    // The constructor: the access level of the account goes to -1 at once.
    assert_eq!(
        texts_to(&s.log.take(), P1),
        ["You are banned for illegal action, GM informed."]
    );
    assert_eq!(s.world.player(ObjectId(P1)).unwrap().data.access_level, -1);
    assert_eq!(
        s.outs.take(),
        [Out::Ls(LsEvent::AccessLevel {
            account: "acc1".into(),
            level: -1
        })]
    );

    s.run(5_100);
    assert!(!s.is_online(P1));
    let outs = s.outs.take();
    let rows = stored(&outs);
    assert_eq!(rows.len(), 1);
    let row = rows[0];
    assert_eq!(
        (row.key.as_str(), row.affect, row.kind),
        ("1", PunishmentAffect::Character, PunishmentType::Ban)
    );
    assert_eq!(row.reason, "bad packet");
    assert_eq!(row.punished_by, "IllegalPlayerActionTask");
    // `System.currentTimeMillis() + DEFAULT_PUNISH_PARAM * 1000`.
    assert!((row.expiration - (now_ms() + 600_000)).abs() < 10_000);
    assert!(outs.contains(&Out::Persist(PersistCommand::CharAccessLevel {
        char_id: 1,
        level: -1
    })));
    assert!(
        s.world
            .has_punishment("1", PunishmentAffect::Character, PunishmentType::Ban)
    );
}

#[test]
fn the_jail_penalty_jails_after_the_delay() {
    let mut s = Scene::new();
    s.enter(P1);
    s.log.take();
    s.world
        .handle_illegal_player_action(ObjectId(P1), "dupe", IllegalActionPunishment::Jail);
    assert_eq!(
        texts_to(&s.log.take(), P1),
        [
            "Illegal action performed!",
            "You will be teleported to GM Consultation Service area and jailed."
        ]
    );
    assert!(!s.world.is_jailed(ObjectId(P1)));
    s.run(5_100);
    assert!(s.world.is_jailed(ObjectId(P1)));
}

#[test]
fn broadcast_tells_the_gms_and_none_does_nothing() {
    let mut s = Scene::new();
    s.enter(GM);
    s.enter(P1);
    s.log.take();
    s.world
        .handle_illegal_player_action(ObjectId(P1), "odd skill", IllegalActionPunishment::None);
    s.world
        .handle_illegal_player_action(ObjectId(P1), "odd skill", IllegalActionPunishment::Broadcast);
    // Nothing is said to the player.
    assert!(texts_to(&s.log.take(), P1).is_empty());
    let out = s.run(5_100);
    assert_eq!(texts_to(&out, GM), ["odd skill"]);
    assert!(s.is_online(P1));
}

#[test]
fn a_negative_destroy_count_is_punished_with_the_default_punishment() {
    let mut s = Scene::new();
    s.world
        .set_punishment_settings(settings_with(IllegalActionPunishment::Kick));
    s.enter(P1);
    s.log.take();
    s.input(
        P1,
        ClientInput::DestroyItem {
            object_id: 12345,
            count: -1,
        },
    );
    let out = s.log.take();
    assert_eq!(
        system_ids_to(&out, P1),
        [SystemMessageId::YouCannotDestroyItBecauseTheNumberIsIncorrect]
    );
    assert_eq!(
        texts_to(&out, P1),
        ["You will be kicked for illegal action, GM informed."]
    );
    s.run(5_100);
    assert!(!s.is_online(P1));
}

// ------------------------------------------------------------------------------ in memory

#[test]
fn a_punishment_marked_stored_has_no_row() {
    let mut s = Scene::new();
    s.enter(P1);
    s.outs.take();
    // The bot report effects (`new PunishmentTask(0, objectId, CHARACTER, CHAT_BAN, 0, ..., true)`).
    s.world.start_punishment(NewPunishment::in_memory(
        P1,
        PunishmentType::ChatBan,
        "bot report",
    ));
    assert!(s.world.is_chat_banned(ObjectId(P1)));
    s.world.start_punishment(NewPunishment::in_memory(
        P1,
        PunishmentType::PartyBan,
        "bot report",
    ));
    assert!(s.world.is_party_banned(ObjectId(P1)));
    s.world
        .stop_punishment("1", PunishmentAffect::Character, PunishmentType::ChatBan);
    s.world
        .stop_punishment("1", PunishmentAffect::Character, PunishmentType::PartyBan);
    assert!(!s.world.is_chat_banned(ObjectId(P1)) && !s.world.is_party_banned(ObjectId(P1)));
    assert!(s.outs.take().is_empty());
}

#[test]
fn the_party_ban_is_the_characters_own() {
    let mut s = Scene::new();
    s.world
        .load_punishments(vec![record(1, "acc1", "ACCOUNT", "PARTY_BAN", 0)]);
    s.enter(P1);
    // Java `isPartyBanned()` looks at the character only.
    assert!(!s.world.is_party_banned(ObjectId(P1)));
    s.world
        .start_punishment(NewPunishment::in_memory(P1, PunishmentType::PartyBan, "x"));
    assert!(s.world.is_party_banned(ObjectId(P1)));
}

#[test]
fn recalling_a_jailed_player_is_refused() {
    let mut s = Scene::new();
    s.enter(GM);
    s.enter(P1);
    s.world
        .load_punishments(vec![record(1, "1", "CHARACTER", "JAIL", 0)]);
    s.log.take();
    s.world.admin_command(ObjectId(GM), "admin_recall P1", false);
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Sorry, player P1 is in Jail."]);
    assert!(texts_to(&out, P1).is_empty());
}

// ------------------------------------------------------------------------------ the jail zone

/// A jail zone around the players of the scene.
fn jail_zone() -> String {
    format!(
        "<zone id=\"1\" name=\"jail\" type=\"JailZone\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
         <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone>\n",
        X - 2_000,
        Y - 2_000,
        X + 2_000,
        Y + 2_000
    )
}

#[test]
fn a_jailed_player_inside_the_jail_is_told_how_long_is_left() {
    let mut s = Scene::with_zones(&jail_zone());
    s.world
        .load_punishments(vec![record(9, "1", "CHARACTER", "JAIL", now_ms() + 90_000)]);
    s.enter(P1);
    assert!(s.world.is_inside_zone(ObjectId(P1), ZoneId::Jail));
    let out = s.run(2_100);
    // No page, no teleport: the player is where the jail is. `Jail time remaining: ...`.
    assert!(html_to(&out, P1).is_empty());
    assert!(teleports(&out, P1).is_empty());
    let texts = texts_to(&out, P1);
    assert_eq!(texts.len(), 1, "{texts:?}");
    assert!(
        texts[0] == "Jail time remaining: 1 minute, 30 seconds."
            || texts[0] == "Jail time remaining: 1 minute, 29 seconds.",
        "{texts:?}"
    );
}

#[test]
fn leaving_the_jail_zone_while_jailed_sends_the_player_back() {
    let mut s = Scene::with_zones(&jail_zone());
    s.world
        .load_punishments(vec![record(9, "1", "CHARACTER", "JAIL", 0)]);
    s.enter(P1);
    s.run(100);
    s.log.take();
    // Something (an admin, a scroll) takes the player out of the zone.
    s.world
        .tele_to_location(ObjectId(P1), TeleportTarget::at(X + 5_000, Y, Z));
    s.input(P1, ClientInput::Appearing);
    let out = s.run(100);
    assert!(!s.world.is_inside_zone(ObjectId(P1), ZoneId::Jail));
    assert_eq!(
        texts_to(&out, P1),
        ["You cannot cheat your way out of here. You must wait until your jail time is over."]
    );
    assert_eq!(teleports(&s.run(2_100), P1), [at(JAIL_IN)]);
}

#[test]
fn a_free_player_found_in_the_jail_is_let_out() {
    let mut s = Scene::with_zones(&jail_zone());
    s.enter(P1);
    let out = s.run(2_100);
    assert_eq!(html_to(&out, P1), ["<html><body>Free.</body></html>"]);
    assert_eq!(teleports(&out, P1), [at(JAIL_OUT)]);
    // A GM is left alone.
    let mut s = Scene::with_zones(&jail_zone());
    s.enter(GM);
    let out = s.run(2_100);
    assert!(html_to(&out, GM).is_empty());
}
