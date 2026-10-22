#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Flood protectors (M6-FU-06): the interval of every kind in game ticks of 100 ms, the GM
//! exception, the punishments and the places that ask.
//! Java: util/FloodProtectors.java, util/FloodProtectorAction.java, config/FloodProtectorConfig.java,
//! and the packets that call `getClient().getFloodProtectors()`.

use std::sync::{Arc, Mutex};

use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{ChatType, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
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

struct Fake;

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
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

/// The text of every `sendMessage` sent to `to`.
fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == "SYSTEM_MESSAGE")
        .map(|(_, b)| decode::<SystemMessage>(b))
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const GM: i32 = 100;
const P1: i32 = 1;
const P2: i32 = 2;

const TOO_FAST: &str = "You are trading too fast.";

struct Scene {
    world: World,
    log: Log,
    outs: Outs,
    lefts: Lefts,
}

impl Scene {
    /// A world with the flood protectors on, Java's defaults.
    fn new() -> Self {
        Self::with_flood(FloodSettings::java_defaults())
    }

    fn with_flood(flood: FloodSettings) -> Self {
        let outs = Outs::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(Recorder(outs.clone())));
        world.set_static_data(Box::new(Fake));
        world.set_flood_settings(flood);
        Self {
            world,
            log: Log::default(),
            outs,
            lefts: Lefts::default(),
        }
    }

    fn enter(&mut self, id: i32) {
        let level = if id == GM { 100 } else { 0 };
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player(id, level)),
            sink: Box::new(Sink(id, self.log.clone(), self.lefts.clone())),
            ip: None,
        });
    }

    fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input,
        });
    }

    /// `TradeDone`: the `Transaction` protector answers "You are trading too fast.".
    fn trade_done(&mut self, id: i32) -> Sent {
        self.log.take();
        self.input(id, ClientInput::TradeDone { response: 0 });
        self.log.take()
    }

    fn refused(&mut self, id: i32) -> bool {
        texts_to(&self.trade_done(id), id) == [TOO_FAST]
    }

    /// Runs the world for `ms` milliseconds.
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

fn config(interval: i32) -> FloodConfig {
    FloodConfig::with_interval(interval)
}

fn stored(outs: &[Out]) -> Vec<&StoredPunishment> {
    outs.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::StorePunishment(p)) => Some(p),
            _ => None,
        })
        .collect()
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

// ------------------------------------------------------------------------------ the interval

/// `FloodProtectorTransactionInterval` = 10 ticks of 100 ms: a second request within one second
/// is refused, one after it is let through (the first request of a player always passes).
#[test]
fn the_interval_is_counted_in_game_ticks_of_100_ms() {
    let mut s = Scene::new();
    s.enter(P1);
    assert!(!s.refused(P1), "the first request passes");
    assert!(s.refused(P1), "the same tick");
    s.run(900);
    assert!(s.refused(P1), "0.9 s later");
    s.run(200);
    assert!(!s.refused(P1), "1.1 s later");
    assert!(s.refused(P1), "the pass started a new interval");
}

/// A request that was refused does not start a new interval (`_nextGameTick` is only set when
/// the request passes).
#[test]
fn a_refused_request_does_not_extend_the_interval() {
    let mut s = Scene::new();
    s.enter(P1);
    assert!(!s.refused(P1));
    s.run(600);
    assert!(s.refused(P1));
    s.run(500);
    assert!(
        !s.refused(P1),
        "1.1 s after the first one, not after the refused one"
    );
}

#[test]
fn every_player_has_their_own_state() {
    let mut s = Scene::new();
    s.enter(P1);
    s.enter(P2);
    assert!(!s.refused(P1));
    assert!(!s.refused(P2));
    assert!(s.refused(P1));
    assert!(s.refused(P2));
}

#[test]
fn every_kind_has_its_own_state() {
    let mut s = Scene::new();
    s.enter(P1);
    assert!(!s.refused(P1));
    // `canPerformPlayerAction` (an `Action` on nothing) is another protector: the refused
    // transaction does not matter to it, and the action does not matter to the next transaction.
    s.input(
        P1,
        ClientInput::Action {
            object_id: 1,
            action_id: 0,
        },
    );
    assert!(s.refused(P1));
}

/// A GM is never refused (`player.isGM()` first in `canPerformAction`).
#[test]
fn a_gm_is_never_refused() {
    let mut s = Scene::new();
    s.enter(GM);
    for _ in 0..5 {
        assert!(!s.refused(GM));
    }
}

/// Interval 0 (the world without configuration, or `Interval = 0` in the file) never refuses.
#[test]
fn interval_zero_never_refuses() {
    let mut s = Scene::with_flood(FloodSettings::disabled());
    s.enter(P1);
    for _ in 0..5 {
        assert!(!s.refused(P1));
    }
    let mut s = Scene::with_flood(FloodSettings::java_defaults().with(FloodKind::Transaction, config(0)));
    s.enter(P1);
    for _ in 0..5 {
        assert!(!s.refused(P1));
    }
}

/// A logout starts over (a new `GameClient` has new protectors); going back to the character
/// screen keeps the client, so the state stays.
#[test]
fn a_new_session_starts_over_but_a_restart_does_not() {
    let mut s = Scene::new();
    s.enter(P1);
    assert!(!s.refused(P1));
    s.input(P1, ClientInput::Restart);
    s.enter(P1);
    assert!(s.refused(P1), "the same client");

    let mut s = Scene::new();
    s.enter(P1);
    assert!(!s.refused(P1));
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    s.enter(P1);
    assert!(!s.refused(P1), "a new client");
}

// ------------------------------------------------------------------------------ punishments

fn punishing(kind: FloodPunishment, limit: i32, minutes: i64) -> FloodSettings {
    FloodSettings::java_defaults().with(
        FloodKind::Transaction,
        FloodConfig {
            protection_interval: 10,
            log_flooding: true,
            punishment_limit: limit,
            punishment_type: kind,
            punishment_time: minutes * 60_000,
        },
    )
}

/// `kick`: the third refused request throws the player out (`Disconnection.storeAndDeleteWith`).
#[test]
fn the_kick_punishment_logs_the_player_out_at_the_limit() {
    let mut s = Scene::with_flood(punishing(FloodPunishment::Kick, 3, 0));
    s.enter(P1);
    s.trade_done(P1);
    s.trade_done(P1);
    s.trade_done(P1);
    assert!(s.is_online(P1), "two refused requests");
    s.trade_done(P1);
    assert!(!s.is_online(P1), "the third refused request");
    assert!(s.lefts.0.lock().unwrap().contains(&(P1, LeaveReason::Logout)));
}

/// `ban`: an account ban of `now + time` by `FloodProtectorAction`, the login server hears of it.
#[test]
fn the_ban_punishment_bans_the_account() {
    let mut s = Scene::with_flood(punishing(FloodPunishment::Ban, 1, 5));
    s.enter(P1);
    s.outs.take();
    s.trade_done(P1);
    let before = now_ms();
    s.trade_done(P1);
    let outs = s.outs.take();
    let rows = stored(&outs);
    assert_eq!(rows.len(), 1);
    let row = rows[0];
    assert_eq!(row.key, "acc1");
    assert_eq!(row.affect, PunishmentAffect::Account);
    assert_eq!(row.kind, PunishmentType::Ban);
    assert_eq!(row.reason, "");
    assert_eq!(row.punished_by, "FloodProtectorAction");
    assert!((before + 300_000..=now_ms() + 300_000).contains(&row.expiration));
    assert!(outs.contains(&Out::Ls(LsEvent::AccessLevel {
        account: "acc1".into(),
        level: -1
    })));
}

/// `jail`: a jail of the character.
#[test]
fn the_jail_punishment_jails_the_character() {
    let mut s = Scene::with_flood(punishing(FloodPunishment::Jail, 1, 10));
    s.enter(P1);
    s.outs.take();
    s.trade_done(P1);
    s.trade_done(P1);
    let outs = s.outs.take();
    let rows = stored(&outs);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].key, "1");
    assert_eq!(rows[0].affect, PunishmentAffect::Character);
    assert_eq!(rows[0].kind, PunishmentType::Jail);
    assert_eq!(rows[0].punished_by, "FloodProtectorAction");
    assert!(
        s.world
            .has_punishment("1", PunishmentAffect::Character, PunishmentType::Jail)
    );
}

/// The default `none` (and any other word, case sensitive) is "Unknown punishment type": the
/// request is refused and nothing else happens.
#[test]
fn an_unknown_punishment_type_only_refuses() {
    for kind in ["none", "Kick", "BAN"] {
        let mut s = Scene::with_flood(punishing(FloodPunishment::parse(kind), 1, 5));
        s.enter(P1);
        s.outs.take();
        s.trade_done(P1);
        assert!(s.refused(P1));
        assert!(s.refused(P1));
        assert!(s.is_online(P1));
        assert!(stored(&s.outs.take()).is_empty(), "{kind}");
    }
}

/// Limit 0 never punishes, however many requests are refused.
#[test]
fn limit_zero_never_punishes() {
    let mut s = Scene::with_flood(punishing(FloodPunishment::Kick, 0, 0));
    s.enter(P1);
    for _ in 0..20 {
        s.trade_done(P1);
    }
    assert!(s.is_online(P1));
}

/// A GM is not punished, and the count of an allowed request starts over.
#[test]
fn the_count_starts_over_when_a_request_passes() {
    let mut s = Scene::with_flood(punishing(FloodPunishment::Kick, 3, 0));
    s.enter(P1);
    s.trade_done(P1);
    s.trade_done(P1);
    s.trade_done(P1);
    s.run(1100);
    s.trade_done(P1);
    s.trade_done(P1);
    s.trade_done(P1);
    assert!(s.is_online(P1), "two refused since the pass");
    s.enter(GM);
    for _ in 0..10 {
        s.trade_done(GM);
    }
    assert!(s.is_online(GM));
}

// ------------------------------------------------------------------------------ the places

/// `ChatShout` / `ChatTrade` in the `GLOBAL` mode: "Do not spam shout channel." / "Do not spam
/// trade channel.", one `GlobalChat` protector for both. A GM is not limited; the other modes do
/// not ask.
#[test]
fn global_shout_and_trade_chat_share_the_global_chat_protector() {
    let mut s = Scene::new();
    let mut settings = WorldSettings::default();
    settings.global_chat = ChatBroadcast::Global;
    settings.trade_chat = ChatBroadcast::Global;
    s.world.set_settings(settings);
    s.enter(P1);
    s.log.take();
    let say = |s: &mut Scene, chat_type: ChatType| {
        s.input(
            P1,
            ClientInput::Say {
                chat_type: chat_type.client_id(),
                text: "hello".into(),
                target: None,
            },
        );
        texts_to(&s.log.take(), P1)
    };
    assert!(say(&mut s, ChatType::Shout).is_empty());
    assert_eq!(say(&mut s, ChatType::Shout), ["Do not spam shout channel."]);
    assert_eq!(say(&mut s, ChatType::Trade), ["Do not spam trade channel."]);
    // 5 ticks.
    s.run(600);
    assert!(say(&mut s, ChatType::Trade).is_empty());

    s.enter(GM);
    s.log.take();
    for _ in 0..3 {
        s.input(
            GM,
            ClientInput::Say {
                chat_type: ChatType::Shout.client_id(),
                text: "hello".into(),
                target: None,
            },
        );
    }
    assert!(texts_to(&s.log.take(), GM).is_empty());
}

#[test]
fn shout_in_the_region_mode_is_not_flood_protected() {
    let mut s = Scene::new();
    s.enter(P1);
    s.log.take();
    for _ in 0..3 {
        s.input(
            P1,
            ClientInput::Say {
                chat_type: ChatType::Shout.client_id(),
                text: "hello".into(),
                target: None,
            },
        );
    }
    assert!(texts_to(&s.log.take(), P1).is_empty());
}

/// `canPerformPlayerAction` guards `Action` and `AttackRequest`: with the protector at a big
/// interval the second click does nothing at all (no `ActionFailed`).
#[test]
fn a_refused_click_is_silent() {
    let mut s = Scene::with_flood(FloodSettings::java_defaults().with(FloodKind::PlayerAction, config(100)));
    s.enter(P1);
    s.input(
        P1,
        ClientInput::Action {
            object_id: 9999,
            action_id: 0,
        },
    );
    let first = s.log.take();
    assert!(!first.is_empty(), "an unknown object answers ActionFailed");
    s.input(
        P1,
        ClientInput::Action {
            object_id: 9999,
            action_id: 0,
        },
    );
    s.input(
        P1,
        ClientInput::AttackRequest {
            object_id: 9999,
            attack_id: 0,
        },
    );
    assert!(s.log.take().is_empty());
}

/// The `Transaction` protector at the packets that answer with a message: the second request of
/// the interval gets Java's text, the first one does not.
#[test]
fn the_transaction_packets_answer_with_javas_texts() {
    use l2_protocol_game::client::{RequestBuyItem, RequestRefundItem, RequestSellItem};
    let cases: Vec<(&str, ClientInput)> = vec![
        (
            "You are buying too fast.",
            ClientInput::Shop(ShopInput::Buy(RequestBuyItem {
                list_id: 1,
                items: None,
            })),
        ),
        (
            "You are selling too fast.",
            ClientInput::Shop(ShopInput::Sell(RequestSellItem {
                list_id: 1,
                items: None,
            })),
        ),
        (
            "You are using refund too fast.",
            ClientInput::Shop(ShopInput::Refund(RequestRefundItem {
                list_id: 1,
                items: None,
            })),
        ),
        (
            "You are destroying items too fast.",
            ClientInput::DestroyItem {
                object_id: 5,
                count: 1,
            },
        ),
        (
            "You are buying items too fast.",
            ClientInput::Store(StoreInput::Buy {
                store_player_id: 999,
                items: Some(vec![]),
            }),
        ),
        (
            "You are selling items too fast.",
            ClientInput::Store(StoreInput::Sell {
                store_player_id: 999,
                items: Some(vec![]),
            }),
        ),
        (
            "You are depositing items too fast.",
            ClientInput::Warehouse(WarehouseInput::Deposit { items: Some(vec![]) }),
        ),
        (
            "You are withdrawing items too fast.",
            ClientInput::Warehouse(WarehouseInput::Withdraw { items: Some(vec![]) }),
        ),
    ];
    for (text, input) in cases {
        let mut s = Scene::new();
        s.enter(P1);
        s.log.take();
        s.input(P1, input.clone());
        assert!(
            !texts_to(&s.log.take(), P1).contains(&text.to_owned()),
            "{text}: first"
        );
        s.input(P1, input.clone());
        assert!(
            texts_to(&s.log.take(), P1).contains(&text.to_owned()),
            "{text}: second"
        );
        // One protector for all of them: the other packets are refused too, and a GM is not.
        s.run(1100);
        s.input(P1, input);
        assert!(
            !texts_to(&s.log.take(), P1).contains(&text.to_owned()),
            "{text}: later"
        );
        s.enter(GM);
        s.log.take();
        s.input(
            GM,
            ClientInput::DestroyItem {
                object_id: 5,
                count: 1,
            },
        );
        s.input(
            GM,
            ClientInput::DestroyItem {
                object_id: 5,
                count: 1,
            },
        );
        assert!(
            !texts_to(&s.log.take(), GM)
                .iter()
                .any(|t| t.ends_with("too fast."))
        );
    }
}

/// All the `Transaction` packets share one state: after a destroy, a buy is "too fast".
#[test]
fn the_transaction_packets_share_one_protector() {
    use l2_protocol_game::client::RequestBuyItem;
    let mut s = Scene::new();
    s.enter(P1);
    s.log.take();
    s.input(
        P1,
        ClientInput::DestroyItem {
            object_id: 5,
            count: 1,
        },
    );
    s.log.take();
    s.input(
        P1,
        ClientInput::Shop(ShopInput::Buy(RequestBuyItem {
            list_id: 1,
            items: None,
        })),
    );
    assert_eq!(texts_to(&s.log.take(), P1), ["You are buying too fast."]);
}

/// `canDropItem`: refused silently.
#[test]
fn a_refused_drop_is_silent() {
    let mut s = Scene::new();
    s.enter(P1);
    let drop = ClientInput::DropItem {
        object_id: 5,
        count: 1,
        x: X,
        y: Y,
        z: Z,
    };
    s.log.take();
    s.input(P1, drop.clone());
    assert!(
        !s.log.take().is_empty(),
        "an item the player does not have is answered"
    );
    s.input(P1, drop);
    assert!(s.log.take().is_empty());
}
