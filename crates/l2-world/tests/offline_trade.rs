#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Offline trade (M6-FU-14): a player who logs out with a store open stays in the world without a
//! client, the store keeps selling, and the shop rows follow `OfflineTradeDatabaseWrites` (D-52).
//! Expected values come from `OfflineTraderTable.java` and the classes named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillTemplates};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::client::{SellListEntry, StoreBuyRequestEntry};
use l2_protocol_game::server::CharInfo;
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(i32, Vec<u8>),
    Left(i32, LeaveReason),
    Persist(PersistCommand),
    Ls(LsEvent),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push(Out::Packet(self.0, bytes));
    }
    fn left_world(&self, reason: LeaveReason) {
        self.1.0.lock().unwrap().push(Out::Left(self.0, reason));
    }
}

struct RecOutbox(Log);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, event: LsEvent) {
        self.0.0.lock().unwrap().push(Out::Ls(event));
    }
}

// ---- the datapack -------------------------------------------------------------------------------

const TEMPLATE: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

const SWORD: i32 = 2;
const ADENA: i32 = 57;
const PILE: i32 = 23;

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="C Sword"><set name="price" val="590" /><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat><stat type="pAtkSpd">379</stat></stats></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="23" type="EtcItem" name="Pile"><set name="price" val="3" /><set name="is_stackable" val="true" /><set name="weight" val="2" /></item>
</list>"#;

struct Data {
    items: Arc<ItemTemplates>,
    stats: StatData,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap(),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: t.shared_reuse_group,
            weapon: t.is_weapon(),
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

fn data() -> Data {
    let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
    let stats = StatData {
        items: Arc::clone(&items),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            "<list></list>",
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    };
    Data { items, stats }
}

// ---- helpers ------------------------------------------------------------------------------------

const A: ObjectId = ObjectId(1);
const B: ObjectId = ObjectId(2);
const SWORD_OBJ: i32 = 100;
const PILE_OBJ: i32 = 105;
const B_ADENA: i32 = 300;

fn inv(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".into(),
        ..Default::default()
    }
}

fn player(id: ObjectId, name: &str, x: i32, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: name.into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 1,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x,
        y: 1000,
        z: 0,
        heading: 0,
        cur_hp: 50.0,
        max_hp: 100.0,
        cur_mp: 30.0,
        max_mp: 50.0,
        cur_cp: 0.0,
        max_cp: 20.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn alice_items() -> Vec<ItemData> {
    vec![inv(SWORD_OBJ, SWORD, 1), inv(PILE_OBJ, PILE, 10)]
}

fn trade_on() -> OfflineSettings {
    OfflineSettings {
        offline_trade_enable: true,
        ..OfflineSettings::default()
    }
}

fn with_rows() -> OfflineSettings {
    OfflineSettings {
        database_writes: true,
        ..trade_on()
    }
}

struct Scene {
    world: World,
    log: Log,
}

impl Scene {
    fn new(settings: OfflineSettings) -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
        world.set_static_data(Box::new(data()));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_offline_settings(settings);
        Self { world, log }
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    /// Alice (a sword and 10 piles) next to Bob (1000 adena), 50 apart.
    fn market(settings: OfflineSettings) -> Self {
        let mut s = Self::new(settings);
        s.enter(player(A, "Alice", 1000, alice_items()));
        s.enter(player(B, "Bob", 1050, vec![inv(B_ADENA, ADENA, 1000)]));
        s.log.take();
        s
    }

    fn input(&mut self, who: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input,
        });
    }

    fn store(&mut self, who: ObjectId, input: StoreInput) {
        self.input(who, ClientInput::Store(input));
    }

    /// Alice opens a sell store "Cheap" with the sword at 500 and 4 piles at 30 each.
    fn open_sell_store(&mut self) {
        self.store(A, StoreInput::ActionUse { action_id: 10 });
        self.store(A, StoreInput::MsgSell { msg: "Cheap".into() });
        self.store(
            A,
            StoreInput::ListSell {
                package_sale: false,
                items: Some(vec![sell(SWORD_OBJ, 1, 500), sell(PILE_OBJ, 4, 30)]),
            },
        );
        self.log.take();
    }

    fn logout(&mut self, who: ObjectId) {
        self.input(who, ClientInput::Logout);
    }

    fn buy(&mut self, lines: &[(i32, i64, i64)]) {
        self.store(
            B,
            StoreInput::Buy {
                store_player_id: A.0,
                items: Some(
                    lines
                        .iter()
                        .map(|&(object_id, count, price)| StoreBuyRequestEntry {
                            object_id,
                            count,
                            price,
                        })
                        .collect(),
                ),
            },
        );
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn adena(&self, who: ObjectId) -> i64 {
        self.world.adena_of(who)
    }
}

fn sell(object_id: i32, count: i64, price: i64) -> SellListEntry {
    SellListEntry {
        item_id: object_id,
        count,
        price,
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

fn packets_named(out: &[Out], to: ObjectId, name: &str) -> Vec<Vec<u8>> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, bytes) if *id == to.0 && name_of(bytes) == name => Some(bytes.clone()),
            _ => None,
        })
        .collect()
}

fn offline_writes(out: &[Out]) -> Vec<OfflineTradeWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::OfflineTrade(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn stored(out: &[Out]) -> Vec<&PlayerData> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::StorePlayer(p)) => Some(p.as_ref()),
            _ => None,
        })
        .collect()
}

fn lefts(out: &[Out]) -> Vec<(String, bool)> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::PlayerLeft { account, logout, .. }) => {
                Some((account.clone(), *logout))
            }
            _ => None,
        })
        .collect()
}

// ---- entering offline mode -----------------------------------------------------------------------

/// `Logout` with a sell store and `OfflineTradeEnable`: `enteredOfflineMode` closes the client
/// (`ServerClose`, the connection ends), the player stays in the world, the login server hears
/// `PlayerLogout` at once, and the character is stored with `online = 2` (`isOnlineInt()`).
#[test]
fn a_logout_with_a_store_leaves_the_trader_in_the_world() {
    let mut s = Scene::market(trade_on());
    s.open_sell_store();
    s.logout(A);
    let out = s.log.take();

    assert!(s.world.player(A).is_some(), "the player stays");
    assert!(s.world.is_offline_trader(A));
    assert_eq!(s.world.offline_trader_count(), 1);
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::Sell);
    // The connection: `ServerClose`, then closed.
    let close = packets_named(&out, A, "SERVER_CLOSE");
    assert_eq!(close.len(), 1, "{out:?}");
    assert!(out.contains(&Out::Left(1, LeaveReason::Logout)));
    // Side by side (switch off) the account stays on this server: no `PlayerLogout`, so the
    // owner cannot log the character in elsewhere while the trader sells its items (D-52).
    assert!(!out.iter().any(|o| matches!(o, Out::Ls(_))), "{out:?}");
    assert!(lefts(&out).is_empty(), "{out:?}");
    // `storeMe()` with `online = 2`.
    let stores = stored(&out);
    assert_eq!(stores.len(), 1);
    assert_eq!(stores[0].profile.online_status, 2);
    // No row without `OfflineTradeDatabaseWrites`.
    assert!(offline_writes(&out).is_empty());
    // With the switch on the account is free at once, like Java.
    let mut on = Scene::market(with_rows());
    on.open_sell_store();
    on.logout(A);
    assert!(on.log.take().contains(&Out::Ls(LsEvent::PlayerLogout {
        account: "acc1".into()
    })));
    // A normal player is stored as online.
    assert!(s.world.player(B).is_some() && !s.world.is_offline_trader(B));
}

/// Without `OfflineTradeEnable` (the default) a logout is a logout; so is one without a store, and
/// one outside a peace zone with `OfflineModeInPeaceZone`.
#[test]
fn only_a_player_with_a_store_and_the_key_goes_offline() {
    let mut s = Scene::market(OfflineSettings::default());
    s.open_sell_store();
    s.logout(A);
    let out = s.log.take();
    assert!(s.world.player(A).is_none());
    assert_eq!(lefts(&out), vec![("acc1".to_owned(), true)]);

    let mut s = Scene::market(trade_on());
    s.logout(A);
    assert!(s.world.player(A).is_none(), "no store: a normal logout");

    // `OfflineModeInPeaceZone`: Alice stands outside any peace zone (there are no zones here).
    let mut s = Scene::market(OfflineSettings {
        offline_mode_in_peace_zone: true,
        ..trade_on()
    });
    s.open_sell_store();
    s.logout(A);
    assert!(s.world.player(A).is_none());
}

/// A trader keeps selling: Bob buys the sword, the adena goes to Alice, the item to Bob, with the
/// same rows as a sale to an online player. `OfflineSetNameColor`: Bob sees Alice with the colour.
#[test]
fn the_store_keeps_selling_and_the_name_has_the_colour() {
    let mut s = Scene::market(OfflineSettings {
        offline_set_name_color: true,
        ..trade_on()
    });
    s.open_sell_store();
    s.logout(A);
    // `broadcastCharInfo` waits 100 ms.
    s.advance(150);
    let out = s.log.take();
    // Everybody around sees the new colour.
    let infos = packets_named(&out, B, "CHAR_INFO");
    assert_eq!(infos.len(), 1, "{out:?}");
    assert_eq!(decode::<CharInfo>(&infos[0]).name_color, 0x0080_8080);

    s.buy(&[(SWORD_OBJ, 1, 500)]);
    assert_eq!(s.adena(B), 500);
    assert_eq!(s.adena(A), 500);
    let bob_items: Vec<i32> = s
        .world
        .player(B)
        .unwrap()
        .data
        .items
        .iter()
        .map(|i| i.item_id)
        .collect();
    assert!(bob_items.contains(&SWORD));
    // The store still stands with the piles.
    assert!(s.world.is_offline_trader(A));
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::Sell);
}

/// `OfflineDisconnectFinished`: when the last item is sold the store closes
/// (`setPrivateStoreType(NONE)`), the trader leaves and is stored and deleted without a
/// `PlayerLogout` (the account was freed when it went offline).
#[test]
fn the_trader_leaves_when_the_store_is_empty() {
    let mut s = Scene::market(trade_on());
    s.open_sell_store();
    s.logout(A);
    s.log.take();

    s.buy(&[(SWORD_OBJ, 1, 500), (PILE_OBJ, 4, 30)]);
    let out = s.log.take();
    assert!(s.world.player(A).is_none(), "the empty store ends the trader");
    assert_eq!(s.world.offline_trader_count(), 0);
    assert_eq!(
        lefts(&out),
        // The account was held until now (switch off): the final store frees it.
        vec![("acc1".to_owned(), true)],
        "{out:?}"
    );
    // Bob paid for everything: 500 + 4 * 30.
    assert_eq!(s.adena(B), 1000 - 500 - 120);
    // The final store carries what Alice earned.
    let stores = stored(&out);
    assert_eq!(stores.len(), 1);
    assert_eq!(
        stores[0]
            .items
            .iter()
            .find(|i| i.item_id == ADENA)
            .map(|i| i.count),
        Some(620)
    );

    // With `OfflineDisconnectFinished = False` the trader stays (without a store).
    let mut s = Scene::market(OfflineSettings {
        offline_disconnect_finished: false,
        ..trade_on()
    });
    s.open_sell_store();
    s.logout(A);
    s.buy(&[(SWORD_OBJ, 1, 500), (PILE_OBJ, 4, 30)]);
    assert!(s.world.is_offline_trader(A));
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
}

/// `OfflineModeNoDamage`: a trader in a sell or buy store takes no damage. Without the key a hit
/// stands the trader up like any player in a store (`PlayerStatus.reduceHp`), the store closes
/// and `OfflineDisconnectFinished` removes the trader.
#[test]
fn a_trader_takes_no_damage_with_the_key() {
    let hit = |settings: OfflineSettings, go_offline: bool| -> Option<f64> {
        let mut s = Scene::market(settings);
        s.open_sell_store();
        if go_offline {
            s.logout(A);
        }
        s.world.reduce_current_hp(A, 10.0, Some(B));
        s.world.player(A).map(|p| p.core.cur_hp)
    };
    let no_damage = OfflineSettings {
        offline_mode_no_damage: true,
        ..trade_on()
    };
    assert_eq!(hit(no_damage.clone(), true), Some(50.0));
    assert_eq!(
        hit(trade_on(), true),
        None,
        "the key is off: stood up, the store closed, gone"
    );
    assert_eq!(hit(no_damage, false), Some(40.0), "an online player is hit");
}

/// The login server does not kick a trader instead of the account's connection, and a character
/// select releases it (`Disconnection.of(player).storeAndDelete()`).
#[test]
fn a_kick_leaves_the_trader_and_a_character_select_releases_it() {
    let mut s = Scene::market(trade_on());
    s.open_sell_store();
    s.logout(A);
    s.log.take();
    s.world.handle(WorldCommand::Kick {
        account: "acc1".into(),
    });
    assert!(s.world.is_offline_trader(A));

    // The login server kicked the account and no connection exists: the trader is released and
    // the final store frees the account.
    let released = Arc::new(Mutex::new(None));
    let slot = Arc::clone(&released);
    s.world.handle(WorldCommand::ReleaseOfflineAccount {
        account: "acc1".into(),
        done: Box::new(move |r| *slot.lock().unwrap() = Some(r)),
    });
    assert_eq!(*released.lock().unwrap(), Some(true));
    assert!(s.world.player(A).is_none());
    assert_eq!(lefts(&s.log.take()), vec![("acc1".to_owned(), true)]);
    // Nobody left: `false`, so the caller answers the login server itself.
    let slot = Arc::clone(&released);
    s.world.handle(WorldCommand::ReleaseOfflineAccount {
        account: "acc1".into(),
        done: Box::new(move |r| *slot.lock().unwrap() = Some(r)),
    });
    assert_eq!(*released.lock().unwrap(), Some(false));

    // A new trader for the character select (the account is in use by the new session, so the
    // login server is not told).
    let mut s = Scene::market(trade_on());
    s.open_sell_store();
    s.logout(A);
    s.log.take();
    let answer = Arc::new(Mutex::new(None));
    let slot = Arc::clone(&answer);
    s.world.handle(WorldCommand::ReleaseOfflineTrader {
        object_id: A,
        done: Box::new(move |was| *slot.lock().unwrap() = Some(was)),
    });
    assert_eq!(*answer.lock().unwrap(), Some(true));
    assert!(s.world.player(A).is_none());
    let out = s.log.take();
    assert_eq!(lefts(&out), vec![("acc1".to_owned(), false)]);

    // Somebody who is no trader: `false`, nothing happens.
    let slot = Arc::clone(&answer);
    s.world.handle(WorldCommand::ReleaseOfflineTrader {
        object_id: B,
        done: Box::new(move |was| *slot.lock().unwrap() = Some(was)),
    });
    assert_eq!(*answer.lock().unwrap(), Some(false));
    assert!(s.world.player(B).is_some());
}

/// A party invitation to a player without a client: "Player is in offline mode."
#[test]
fn a_trader_cannot_be_invited_to_a_party() {
    let mut s = Scene::market(trade_on());
    s.open_sell_store();
    s.logout(A);
    s.log.take();
    s.input(
        B,
        ClientInput::JoinParty {
            name: "Alice".into(),
            distribution_type_id: 0,
        },
    );
    let out = s.log.take();
    assert_eq!(packets_named(&out, B, "SYSTEM_MESSAGE").len(), 1, "{out:?}");
    assert!(s.world.party_of(B).is_none());
}

// ---- the rows (D-52) ---------------------------------------------------------------------------

/// With the switch off (the default) a whole cycle writes no row: going offline, a sale, the end.
#[test]
fn without_the_switch_no_row_is_written() {
    let mut s = Scene::market(trade_on());
    s.open_sell_store();
    s.logout(A);
    s.buy(&[(SWORD_OBJ, 1, 500)]);
    s.buy(&[(PILE_OBJ, 4, 30)]);
    let out = s.log.take();
    assert!(s.world.player(A).is_none());
    assert!(offline_writes(&out).is_empty(), "{out:?}");
}

/// `STORE_OFFLINE_TRADE_IN_REALTIME` with the switch on: `onTransaction(player, false, true)` at
/// the logout (the sell list by object id, the title and the type of the row), then after each
/// sale the remaining lines, and `finished` when the list is empty; `removeTrader` follows.
#[test]
fn the_realtime_rows_follow_java() {
    let mut s = Scene::market(with_rows());
    s.open_sell_store();
    s.logout(A);
    let out = s.log.take();
    let writes = offline_writes(&out);
    let line = |item, count, price| OfflineTradeLine { item, count, price };
    let started = match &writes[..] {
        [
            OfflineTradeWrite::Transaction {
                char_id,
                finished: false,
                items,
                status: Some(status),
            },
        ] => {
            assert_eq!(*char_id, A);
            assert_eq!(items, &vec![line(SWORD_OBJ, 1, 500), line(PILE_OBJ, 4, 30)]);
            assert_eq!(status.store_type, 1);
            assert_eq!(status.title.as_deref(), Some("Cheap"));
            status.time
        }
        other => panic!("{other:?}"),
    };
    assert!(started > 0);
    // The row is written before the character (Java: `onTransaction`, then `storeMe`).
    let positions: Vec<usize> = out
        .iter()
        .enumerate()
        .filter(|(_, o)| {
            matches!(
                o,
                Out::Persist(PersistCommand::OfflineTrade(_) | PersistCommand::StorePlayer(_))
            )
        })
        .map(|(i, _)| i)
        .collect();
    assert!(matches!(
        out[positions[0]],
        Out::Persist(PersistCommand::OfflineTrade(_))
    ));

    // A sale: the lines that are left, no new row (not the first call).
    s.buy(&[(SWORD_OBJ, 1, 500)]);
    let writes = offline_writes(&s.log.take());
    assert_eq!(
        writes,
        vec![OfflineTradeWrite::Transaction {
            char_id: A,
            finished: false,
            items: vec![line(PILE_OBJ, 4, 30)],
            status: None,
        }]
    );

    // The last sale: finished, then `removeTrader` from `setPrivateStoreType(NONE)`.
    s.buy(&[(PILE_OBJ, 4, 30)]);
    let writes = offline_writes(&s.log.take());
    assert_eq!(
        writes,
        vec![
            OfflineTradeWrite::Transaction {
                char_id: A,
                finished: true,
                items: vec![],
                status: None,
            },
            OfflineTradeWrite::Remove { char_id: A },
        ]
    );
}

/// `EnterWorld`: "(trade || craft) && realtime" -> `onTransaction(player, true, false)`: a player
/// who comes in has no shop row any more. Only with the switch on.
#[test]
fn entering_the_world_clears_the_shop_rows() {
    let mut s = Scene::new(with_rows());
    s.enter(player(A, "Alice", 1000, alice_items()));
    let writes = offline_writes(&s.log.take());
    assert_eq!(
        writes,
        vec![OfflineTradeWrite::Transaction {
            char_id: A,
            finished: true,
            items: vec![],
            status: None,
        }]
    );

    let mut s = Scene::new(trade_on());
    s.enter(player(A, "Alice", 1000, alice_items()));
    assert!(offline_writes(&s.log.take()).is_empty());
}

/// `Shutdown` without realtime but with `RestoreOffliners`: `storeOffliners()` before the players
/// are disconnected, one row per shop (lines first, as the statement order goes).
#[test]
fn the_shutdown_stores_the_shops_when_the_rows_are_not_realtime() {
    let mut s = Scene::market(OfflineSettings {
        store_offline_trade_in_realtime: false,
        restore_offliners: true,
        ..with_rows()
    });
    s.open_sell_store();
    s.logout(A);
    // Not realtime: nothing is written yet.
    assert!(offline_writes(&s.log.take()).is_empty());
    s.world.handle(WorldCommand::Shutdown {
        done: Box::new(|| {}),
    });
    let out = s.log.take();
    let writes = offline_writes(&out);
    assert_eq!(writes.len(), 1, "{out:?}");
    let OfflineTradeWrite::StoreAll(rows) = &writes[0] else {
        panic!("{writes:?}");
    };
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].char_id, A);
    assert_eq!(rows[0].status.store_type, 1);
    assert_eq!(rows[0].status.title.as_deref(), Some("Cheap"));
    assert_eq!(rows[0].items.len(), 2);
    // The shops come before the stores of the players.
    let first = out
        .iter()
        .position(|o| matches!(o, Out::Persist(PersistCommand::OfflineTrade(_))))
        .unwrap();
    let store = out
        .iter()
        .position(|o| matches!(o, Out::Persist(PersistCommand::StorePlayer(_))))
        .unwrap();
    assert!(first < store);

    // Without the switch, or with realtime rows, the shutdown writes no shops.
    let mut s = Scene::market(OfflineSettings {
        store_offline_trade_in_realtime: false,
        restore_offliners: true,
        ..trade_on()
    });
    s.open_sell_store();
    s.logout(A);
    s.world.handle(WorldCommand::Shutdown {
        done: Box::new(|| {}),
    });
    assert!(offline_writes(&s.log.take()).is_empty());
}

// ---- restoring ---------------------------------------------------------------------------------

/// `restoreOfflineTraders`: the player comes back without a client, sitting, with its sell list
/// (an item the character does not have is skipped), the title, the type and the start time; the
/// store sells, and the login server hears nothing.
#[test]
fn a_restored_trader_sells_again() {
    let mut s = Scene::new(trade_on());
    s.enter(player(B, "Bob", 1050, vec![inv(B_ADENA, ADENA, 1000)]));
    s.log.take();
    let row = OfflineTradeRow {
        char_id: A,
        status: OfflineTradeStatus {
            time: 1_700_000_000_000,
            store_type: 1,
            title: Some("Back again".into()),
        },
        items: vec![
            OfflineTradeLine {
                item: SWORD_OBJ,
                count: 1,
                price: 500,
            },
            // The character has no such item.
            OfflineTradeLine {
                item: 999,
                count: 1,
                price: 5,
            },
        ],
    };
    s.world
        .restore_offline_traders(vec![(player(A, "Alice", 1000, alice_items()), row)]);
    let out = s.log.take();
    assert!(s.world.is_offline_trader(A));
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::Sell);
    assert!(s.world.is_sitting(A));
    let list = s.world.player(A).unwrap().trade.sell_list.clone().unwrap();
    assert_eq!(list.title, "Back again");
    assert_eq!(list.items.len(), 1);
    assert_eq!((list.items[0].object_id.0, list.items[0].price), (SWORD_OBJ, 500));
    assert!(!out.iter().any(|o| matches!(o, Out::Ls(_))), "{out:?}");

    s.buy(&[(SWORD_OBJ, 1, 500)]);
    assert_eq!(s.adena(B), 500);
    // The only line was sold: the store closes and the trader leaves.
    assert!(s.world.player(A).is_none());
}

/// A restore with `StoreOfflineTradeInRealtime = False` empties the tables (the shops live in
/// memory and `storeOffliners` writes them at shutdown); only with the switch.
#[test]
fn a_restore_without_realtime_empties_the_tables() {
    let mut s = Scene::new(OfflineSettings {
        store_offline_trade_in_realtime: false,
        ..with_rows()
    });
    s.world.restore_offline_traders(Vec::new());
    assert_eq!(
        offline_writes(&s.log.take()),
        vec![OfflineTradeWrite::StoreAll(Vec::new())]
    );
    let mut s = Scene::new(OfflineSettings {
        store_offline_trade_in_realtime: false,
        ..trade_on()
    });
    s.world.restore_offline_traders(Vec::new());
    assert!(offline_writes(&s.log.take()).is_empty());
}

/// A buy store and a workshop come back with their lists and titles; a row of a type the table
/// does not know is skipped.
#[test]
fn buy_stores_and_workshops_are_restored_with_their_lists() {
    let mut s = Scene::new(trade_on());
    let row = |store_type, title: &str, items: Vec<OfflineTradeLine>| OfflineTradeRow {
        char_id: A,
        status: OfflineTradeStatus {
            time: 1_700_000_000_000,
            store_type,
            title: Some(title.into()),
        },
        items,
    };
    let line = |item, count, price| OfflineTradeLine { item, count, price };
    // A buy store: the item ids (not object ids) with the count and price.
    s.world.restore_offline_traders(vec![(
        player(A, "Alice", 1000, vec![inv(300, ADENA, 5000)]),
        row(3, "Buying", vec![line(PILE, 10, 7)]),
    )]);
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::Buy);
    let list = s.world.player(A).unwrap().trade.buy_list.clone().unwrap();
    assert_eq!(list.title, "Buying");
    assert_eq!(
        list.items
            .iter()
            .map(|i| (i.item.item_id, i.count, i.price))
            .collect::<Vec<_>>(),
        vec![(PILE, 10, 7)]
    );

    // A workshop: recipe id, price; the title is the store name.
    let mut s = Scene::new(trade_on());
    s.world.restore_offline_traders(vec![(
        player(A, "Alice", 1000, vec![]),
        row(5, "Crafts", vec![line(1234, 0, 800)]),
    )]);
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::Manufacture);
    assert_eq!(s.world.manufacture_items(A), vec![(1234, 800)]);
    assert_eq!(s.world.recipe_store_name(A), "Crafts");

    // An unknown type, or the buff shop, brings nobody back.
    let mut s = Scene::new(trade_on());
    s.world.restore_offline_traders(vec![
        (player(A, "Alice", 1000, vec![]), row(77, "?", vec![])),
        (player(B, "Bob", 1050, vec![]), row(9, "Buffs", vec![])),
    ]);
    assert_eq!(s.world.offline_trader_count(), 0);
    assert!(s.world.player(A).is_none() && s.world.player(B).is_none());
}

/// Switch on (Java's behaviour): the account was freed at logout, so the trader's end sends no
/// second `PlayerLogout`.
#[test]
fn with_the_switch_on_the_end_of_the_trader_does_not_free_the_account_again() {
    let mut s = Scene::market(with_rows());
    s.open_sell_store();
    s.logout(A);
    s.log.take();
    s.buy(&[(SWORD_OBJ, 1, 500), (PILE_OBJ, 4, 30)]);
    assert_eq!(lefts(&s.log.take()), vec![("acc1".to_owned(), false)]);
}
