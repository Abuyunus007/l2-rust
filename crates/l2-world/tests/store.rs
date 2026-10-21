#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Private stores (M6-STORE-01): opening a store, the lists, the titles, buying from a sell store
//! or package sale and selling to a buy store, with the messages, packets and item rows of Java's
//! `Player`, `TradeList` and the `PrivateStore*` packets. The expected values come from the Java
//! code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillTemplates};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::client::{BuyListEntry, SellListEntry, StoreBuyRequestEntry, StoreSellRequestEntry};
use l2_protocol_game::server::{
    ChangeWaitType, CharInfo, ExPrivateStoreSetWholeMsg, InventoryUpdate, PrivateStoreListBuy,
    PrivateStoreListSell, PrivateStoreManageListBuy, PrivateStoreManageListSell, PrivateStoreMsgBuy,
    PrivateStoreMsgSell, SystemMessage, SystemMessageParam, UserInfo,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(i32, Vec<u8>),
    Persist(PersistCommand),
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
    fn left_world(&self, _reason: LeaveReason) {}
}

struct RecOutbox(Log);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
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
const BOUND: i32 = 3;
const ADENA: i32 = 57;
const PILE: i32 = 23;
const QUEST: i32 = 21;
const CUIRASS: i32 = 4;

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="C Sword"><set name="price" val="590" /><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat><stat type="pAtkSpd">379</stat></stats></item>
<item id="3" type="EtcItem" name="Bound"><set name="is_tradable" val="false" /><set name="weight" val="1" /></item>
<item id="4" type="Armor" name="Cuirass"><set name="price" val="100" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="weight" val="10" /></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="20" type="EtcItem" name="Anvil"><set name="weight" val="1000" /><set name="is_stackable" val="true" /></item>
<item id="21" type="EtcItem" name="Quest thing"><set name="is_questitem" val="true" /><set name="is_stackable" val="true" /></item>
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
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        AccessLevelStatic {
            is_gm: level >= 100,
            ..AccessLevelStatic::USER
        }
    }
    fn access_level_allows_transaction(&self, level: i32) -> bool {
        // `AccessLevels.xml`: levels 20 to 50 are the GMs that cannot trade.
        !(20..=50).contains(&level)
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
const C: ObjectId = ObjectId(3);

fn item(object_id: i32, item_id: i32, count: i64, loc: &str) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: loc.into(),
        ..Default::default()
    }
}

fn inv(object_id: i32, item_id: i32, count: i64) -> ItemData {
    item(object_id, item_id, count, "INVENTORY")
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

struct Scene {
    world: World,
    log: Log,
}

impl Scene {
    fn new() -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
        world.set_static_data(Box::new(data()));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
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

    /// Alice (1) and Bob (2) stand 50 apart; what the enter sent is dropped.
    fn pair(alice: Vec<ItemData>, bob: Vec<ItemData>) -> Self {
        let mut s = Self::new();
        s.enter(player(A, "Alice", 1000, alice));
        s.enter(player(B, "Bob", 1050, bob));
        s.log.take();
        s
    }

    fn input(&mut self, who: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input,
        });
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn items_of(&self, who: ObjectId) -> Vec<(i32, i32, i64)> {
        let mut items: Vec<(i32, i32, i64)> = self
            .world
            .player(who)
            .unwrap()
            .data
            .items
            .iter()
            .map(|i| (i.object_id.0, i.item_id, i.count))
            .collect();
        items.sort_unstable();
        items
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

/// The packets one client got, in order.
fn packets(out: &[Out], to: ObjectId) -> Vec<&[u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, bytes) if *id == to.0 => Some(bytes.as_slice()),
            _ => None,
        })
        .collect()
}

fn names(out: &[Out], to: ObjectId) -> Vec<String> {
    packets(out, to).into_iter().map(name_of).collect()
}

fn of<'a>(out: &'a [Out], to: ObjectId, name: &str) -> Vec<&'a [u8]> {
    packets(out, to)
        .into_iter()
        .filter(|b| name_of(b) == name)
        .collect()
}

fn messages(out: &[Out], to: ObjectId) -> Vec<SystemMessage> {
    of(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw))
        .collect()
}

fn message_ids(out: &[Out], to: ObjectId) -> Vec<SystemMessageId> {
    messages(out, to).into_iter().map(|m| m.id).collect()
}

/// `(owner, upserted (object id, loc), removed object ids)` of every `ItemChanges`.
type Rows = (i32, Vec<(i32, String)>, Vec<i32>);

fn item_changes(out: &[Out]) -> Vec<Rows> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemChanges {
                owner,
                upserts,
                removed,
            }) => Some((
                owner.0,
                upserts.iter().map(|i| (i.object_id.0, i.loc.clone())).collect(),
                removed.iter().map(|o| o.0).collect(),
            )),
            _ => None,
        })
        .collect()
}

// ---- scenes -------------------------------------------------------------------------------------

const SWORD_OBJ: i32 = 100;
const PILE_OBJ: i32 = 105;
const A_ADENA: i32 = 104;
const BOUND_OBJ: i32 = 106;
const QUEST_OBJ: i32 = 107;
const WORN_OBJ: i32 = 108;
const B_ADENA: i32 = 300;

fn worn(object_id: i32, item_id: i32) -> ItemData {
    let mut item = inv(object_id, item_id, 1);
    item.loc = "PAPERDOLL".into();
    item.loc_data = 10;
    item
}

impl Scene {
    fn store(&mut self, who: ObjectId, input: StoreInput) {
        self.input(who, ClientInput::Store(input));
    }

    /// Alice (a sword, 10 piles, 10 adena and things a store cannot take) next to Bob (1000
    /// adena), 50 apart.
    fn market() -> Self {
        Self::pair(
            vec![
                inv(SWORD_OBJ, SWORD, 1),
                inv(PILE_OBJ, PILE, 10),
                inv(A_ADENA, ADENA, 10),
                inv(BOUND_OBJ, BOUND, 1),
                inv(QUEST_OBJ, QUEST, 3),
                worn(WORN_OBJ, CUIRASS),
            ],
            vec![inv(B_ADENA, ADENA, 1000)],
        )
    }

    /// Alice opens a sell store (or package sale) with the sword at 500 and 4 piles at 30 each.
    fn open_sell_store(&mut self, package_sale: bool) {
        self.store(
            A,
            StoreInput::ActionUse {
                action_id: if package_sale { 61 } else { 10 },
            },
        );
        self.store(
            A,
            StoreInput::ListSell {
                package_sale,
                items: Some(vec![sell(SWORD_OBJ, 1, 500), sell(PILE_OBJ, 4, 30)]),
            },
        );
        self.log.take();
    }

    fn buy(&mut self, who: ObjectId, store: ObjectId, lines: &[(i32, i64, i64)]) {
        self.store(
            who,
            StoreInput::Buy {
                store_player_id: store.0,
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

    fn sell_to(&mut self, who: ObjectId, store: ObjectId, lines: &[(i32, i32, i64, i64)]) {
        self.store(
            who,
            StoreInput::Sell {
                store_player_id: store.0,
                items: Some(
                    lines
                        .iter()
                        .map(|&(object_id, item_id, count, price)| StoreSellRequestEntry {
                            object_id,
                            item_id,
                            unknown: [0, 0],
                            count,
                            price,
                        })
                        .collect(),
                ),
            },
        );
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

fn want(item_id: i32, count: i64, price: i64) -> BuyListEntry {
    BuyListEntry {
        item_id,
        unknown1: 0,
        count,
        price,
        unknown2: [0; 4],
    }
}

fn op_name(op: ServerOpcode) -> &'static str {
    op.name()
}

/// The texts of the system messages one client got (the ones Java sends with `sendMessage`).
fn texts(out: &[Out], to: ObjectId) -> Vec<String> {
    messages(out, to)
        .into_iter()
        .flat_map(|m| m.params)
        .filter_map(|p| match p {
            SystemMessageParam::Text(text) => Some(text),
            _ => None,
        })
        .collect()
}

/// `(object id, change, count)` of the entries of every `InventoryUpdate` one client got, sorted.
fn inventory_updates(out: &[Out], to: ObjectId) -> Vec<(i32, i16, i64)> {
    let mut entries: Vec<(i32, i16, i64)> = of(out, to, "INVENTORY_UPDATE")
        .iter()
        .flat_map(|raw| decode::<InventoryUpdate>(raw).updates)
        .map(|u| (u.item.object_id, u.change, u.item.count))
        .collect();
    entries.sort_unstable();
    entries
}

// ---- opening a store ----------------------------------------------------------------------------

#[test]
fn the_sell_window_lists_what_a_store_can_take() {
    let mut s = Scene::market();
    // `RequestActionUse` 10: `tryOpenPrivateSellStore(false)`.
    s.store(A, StoreInput::ActionUse { action_id: 10 });
    let out = s.log.take();
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::SellManage);
    assert_eq!(
        names(&out, A),
        [op_name(ServerOpcode::PrivateStoreSellManageList)]
    );
    let window: PrivateStoreManageListSell =
        decode(of(&out, A, op_name(ServerOpcode::PrivateStoreSellManageList))[0]);
    assert_eq!(window.object_id, 1);
    assert!(!window.package_sale);
    assert_eq!(window.player_adena, 10);
    // `getAvailableItems(sellList)`: no adena, no worn item, no quest item, no bound item. Each
    // with `getReferencePrice() * 2`.
    let listed: Vec<(i32, i64, i64)> = window
        .item_list
        .iter()
        .map(|c| (c.item.object_id, c.item.count, c.reference_price))
        .collect();
    assert_eq!(listed, [(SWORD_OBJ, 1, 1180), (PILE_OBJ, 10, 6)]);
    assert!(window.sell_list.is_empty());

    // The package sale (action 61) opens the same window with the flag.
    s.store(A, StoreInput::ActionUse { action_id: 61 });
    let out = s.log.take();
    let window: PrivateStoreManageListSell =
        decode(of(&out, A, op_name(ServerOpcode::PrivateStoreSellManageList))[0]);
    assert!(window.package_sale);
}

#[test]
fn the_buy_window_lists_one_item_per_id_and_not_adena() {
    let mut s = Scene::market();
    s.world.player_mut(A).unwrap().data.items.push(inv(109, PILE, 2));
    s.store(A, StoreInput::ActionUse { action_id: 28 });
    let out = s.log.take();
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::BuyManage);
    let window: PrivateStoreManageListBuy =
        decode(of(&out, A, op_name(ServerOpcode::PrivateStoreBuyManageList))[0]);
    // `getUniqueItems(false, true, true, false)`: the first item of each id, not adena.
    let listed: Vec<i32> = window.item_list.iter().map(|c| c.item.object_id).collect();
    assert_eq!(listed, [SWORD_OBJ, PILE_OBJ]);
    assert_eq!(window.player_adena, 10);
    assert!(window.buy_list.is_empty());
}

#[test]
fn opening_a_store_twice_closes_it_and_the_manage_packet_only_refuses_the_dead() {
    let mut s = Scene::market();
    s.store(A, StoreInput::ActionUse { action_id: 10 });
    s.log.take();
    // Same kind again: NONE, then a new SELL_MANAGE (the window comes again).
    s.store(A, StoreInput::ActionUse { action_id: 10 });
    assert_eq!(
        names(&s.log.take(), A),
        [op_name(ServerOpcode::PrivateStoreSellManageList)]
    );
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::SellManage);
    // The other kind while SELL_MANAGE: neither branch of Java's `tryOpenPrivateBuyStore` fits, so
    // nothing happens at all.
    s.store(A, StoreInput::ActionUse { action_id: 28 });
    assert!(s.log.take().is_empty());
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::SellManage);

    // `RequestPrivateStoreManageSell` only refuses a dead player; `ManageBuy` opens the window.
    let mut s = Scene::market();
    s.store(A, StoreInput::ManageSell);
    assert!(s.log.take().is_empty());
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
    s.store(A, StoreInput::ManageBuy);
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::BuyManage);
}

#[test]
fn a_store_cannot_be_opened_next_to_a_sitting_player() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    // Alice sits at 50: `ShopMinRangeFromPlayer = 50` plus both collision radii (9 + 9).
    s.store(B, StoreInput::ActionUse { action_id: 28 });
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, B),
        [SystemMessageId::YouCannotOpenAPrivateStoreHere]
    );
    assert_eq!(names(&out, B), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert_eq!(s.world.private_store_type(B), PrivateStoreType::None);

    // 69 apart is outside 50 + 18.
    s.world.set_position(B, 1069, 1000, 0);
    s.log.take();
    s.store(B, StoreInput::ActionUse { action_id: 28 });
    assert_eq!(s.world.private_store_type(B), PrivateStoreType::BuyManage);
}

// ---- the sell list ------------------------------------------------------------------------------

#[test]
fn confirming_a_sell_list_sits_down_and_announces_the_store() {
    let mut s = Scene::market();
    s.store(A, StoreInput::ActionUse { action_id: 10 });
    s.store(
        A,
        StoreInput::MsgSell {
            msg: "Cheap swords".into(),
        },
    );
    // Only the owner gets the title when it is set.
    let out = s.log.take();
    let title: PrivateStoreMsgSell = decode(of(&out, A, op_name(ServerOpcode::PrivateStoreSellMsg))[0]);
    assert_eq!((title.object_id, title.message.as_str()), (1, "Cheap swords"));
    assert!(of(&out, B, op_name(ServerOpcode::PrivateStoreSellMsg)).is_empty());

    s.store(
        A,
        StoreInput::ListSell {
            package_sale: false,
            items: Some(vec![sell(SWORD_OBJ, 1, 500), sell(PILE_OBJ, 4, 30)]),
        },
    );
    let out = s.log.take();
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::Sell);
    assert!(s.world.is_sitting(A));
    let list = s.world.player(A).unwrap().trade.sell_list.clone().unwrap();
    assert!(!list.packaged);
    let lines: Vec<(i32, i64, i64)> = list
        .items
        .iter()
        .map(|t| (t.object_id.0, t.count, t.price))
        .collect();
    assert_eq!(lines, [(SWORD_OBJ, 1, 500), (PILE_OBJ, 4, 30)]);

    // `sitDown()`, `broadcastUserInfo()` (the own `UserInfo` at once), then
    // `broadcastPacket(PrivateStoreMsgSell)` to the owner and to everybody who sees it.
    let wait: ChangeWaitType = decode(of(&out, A, op_name(ServerOpcode::ChangeWaitType))[0]);
    assert_eq!(wait.move_type, ChangeWaitType::WT_SITTING);
    let user_info: UserInfo = decode(of(&out, A, op_name(ServerOpcode::UserInfo))[0]);
    assert_eq!(user_info.private_store_type, 1);
    for who in [A, B] {
        let msg: PrivateStoreMsgSell = decode(of(&out, who, op_name(ServerOpcode::PrivateStoreSellMsg))[0]);
        assert_eq!((msg.object_id, msg.message.as_str()), (1, "Cheap swords"));
    }
    // The `CharInfo` follows 100 ms later, with the store type.
    s.advance(100);
    let later = s.log.take();
    let char_info: CharInfo = decode(of(&later, B, op_name(ServerOpcode::CharInfo))[0]);
    assert_eq!(char_info.private_store_type, 1);
    assert!(char_info.sitting);
}

#[test]
fn a_package_sale_announces_itself_with_the_whole_message() {
    let mut s = Scene::market();
    s.store(A, StoreInput::ActionUse { action_id: 61 });
    s.store(
        A,
        StoreInput::WholeMsg {
            msg: "Everything".into(),
        },
    );
    let out = s.log.take();
    let title: ExPrivateStoreSetWholeMsg =
        decode(of(&out, A, op_name(ServerOpcode::ExPrivateStorePackageMsg))[0]);
    assert_eq!(title.message, "Everything");
    s.store(
        A,
        StoreInput::ListSell {
            package_sale: true,
            items: Some(vec![sell(SWORD_OBJ, 1, 500)]),
        },
    );
    let out = s.log.take();
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::PackageSell);
    assert!(
        s.world
            .player(A)
            .unwrap()
            .trade
            .sell_list
            .as_ref()
            .unwrap()
            .packaged
    );
    for who in [A, B] {
        let msg: ExPrivateStoreSetWholeMsg =
            decode(of(&out, who, op_name(ServerOpcode::ExPrivateStorePackageMsg))[0]);
        assert_eq!(msg.message, "Everything");
    }
    let user_info: UserInfo = decode(of(&out, A, op_name(ServerOpcode::UserInfo))[0]);
    assert_eq!(user_info.private_store_type, 8);
}

#[test]
fn a_new_viewer_gets_the_title_with_the_player_info() {
    let mut s = Scene::market();
    s.store(A, StoreInput::ActionUse { action_id: 10 });
    s.store(A, StoreInput::MsgSell { msg: "Swords".into() });
    s.store(
        A,
        StoreInput::ListSell {
            package_sale: false,
            items: Some(vec![sell(SWORD_OBJ, 1, 500)]),
        },
    );
    s.log.take();
    // Cleo arrives next to Alice: `sendInfo` ends with the store message.
    s.enter(player(C, "Cleo", 1200, vec![]));
    let out = s.log.take();
    let msgs = of(&out, C, op_name(ServerOpcode::PrivateStoreSellMsg));
    assert_eq!(msgs.len(), 1);
    assert_eq!(decode::<PrivateStoreMsgSell>(msgs[0]).message, "Swords");
}

#[test]
fn bad_lists_and_limits_are_answered_like_java() {
    let mut s = Scene::market();
    s.store(A, StoreInput::ActionUse { action_id: 10 });
    s.log.take();

    // A bad packet: INCORRECT_ITEM_COUNT, the store is closed, `broadcastUserInfo()`.
    s.store(
        A,
        StoreInput::ListSell {
            package_sale: false,
            items: None,
        },
    );
    let out = s.log.take();
    assert_eq!(message_ids(&out, A), [SystemMessageId::IncorrectItemCount]);
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
    assert_eq!(of(&out, A, op_name(ServerOpcode::UserInfo)).len(), 1);

    // Four items for a human (`MaxPvtStoreSellSlotsOther = 3`): the window again and the message.
    s.store(A, StoreInput::ActionUse { action_id: 10 });
    s.log.take();
    s.store(
        A,
        StoreInput::ListSell {
            package_sale: false,
            items: Some(vec![
                sell(SWORD_OBJ, 1, 1),
                sell(PILE_OBJ, 1, 1),
                sell(PILE_OBJ, 2, 1),
                sell(PILE_OBJ, 3, 1),
            ]),
        },
    );
    let out = s.log.take();
    assert_eq!(
        names(&out, A),
        [
            op_name(ServerOpcode::PrivateStoreSellManageList),
            "SYSTEM_MESSAGE"
        ]
    );
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted]
    );
    assert!(!s.world.is_sitting(A));

    // A total above `MaxAdena` (the owner's own adena counts): the out-of-pocket message, and the
    // store stays in SELL_MANAGE.
    s.world.set_settings(WorldSettings {
        ground: GroundItemSettings {
            max_adena: 100,
            ..GroundItemSettings::default()
        },
        ..WorldSettings::default()
    });
    s.store(
        A,
        StoreInput::ListSell {
            package_sale: false,
            items: Some(vec![sell(PILE_OBJ, 4, 30)]),
        },
    );
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::YouHaveExceededYourOutOfPocketAdenaLimit]
    );
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::SellManage);
}

#[test]
fn items_the_list_refuses_are_skipped_without_a_word() {
    let mut s = Scene::market();
    s.store(A, StoreInput::ActionUse { action_id: 10 });
    s.log.take();
    // The bound item, a quest item and an object Alice does not own: `addItem` returns null and
    // `SetPrivateStoreListSell` ignores that.
    s.store(
        A,
        StoreInput::ListSell {
            package_sale: false,
            items: Some(vec![
                sell(BOUND_OBJ, 1, 5),
                sell(QUEST_OBJ, 1, 5),
                sell(B_ADENA, 1, 5),
            ]),
        },
    );
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::Sell);
    assert!(
        s.world
            .player(A)
            .unwrap()
            .trade
            .sell_list
            .as_ref()
            .unwrap()
            .items
            .is_empty()
    );
}

// ---- the customer's windows ---------------------------------------------------------------------

#[test]
fn clicking_a_sell_store_twice_shows_its_list() {
    let mut s = Scene::market();
    s.store(A, StoreInput::MsgSell { msg: "Shop".into() });
    s.open_sell_store(false);
    // Bob targets Alice, then clicks her again: `setIntentionInteract`, and `doInteract` once he
    // is close enough (50 apart is inside 36 + the radii).
    for _ in 0..2 {
        s.input(
            B,
            ClientInput::Action {
                object_id: A.0,
                action_id: 0,
            },
        );
    }
    s.advance(500);
    let out = s.log.take();
    let list: PrivateStoreListSell = decode(of(&out, B, op_name(ServerOpcode::PrivateStoreSellList))[0]);
    assert_eq!(list.seller_object_id, 1);
    assert!(!list.package_sale);
    assert_eq!(list.player_adena, 1000);
    let lines: Vec<(i32, i64, i64, i64)> = list
        .items
        .iter()
        .map(|e| (e.item.object_id, e.item.count, e.price, e.reference_price))
        .collect();
    assert_eq!(lines, [(SWORD_OBJ, 1, 500, 1180), (PILE_OBJ, 4, 30, 6)]);
    // `Player.doInteract`: an `ActionFailed` goes before the list.
    let names = names(&out, B);
    let fail = names.iter().position(|n| n == "ACTION_FAIL").unwrap();
    let window = names
        .iter()
        .position(|n| n == op_name(ServerOpcode::PrivateStoreSellList))
        .unwrap();
    assert!(fail < window);
}

#[test]
fn the_buy_list_a_customer_sees_is_matched_to_its_inventory() {
    let mut s = Scene::market();
    s.world.player_mut(B).unwrap().data.items.push(inv(310, PILE, 3));
    // Bob wants 5 piles at 20 and a sword at 300.
    s.store(B, StoreInput::ActionUse { action_id: 28 });
    s.store(
        B,
        StoreInput::ListBuy {
            items: Some(vec![want(PILE, 5, 20), want(SWORD, 1, 300)]),
        },
    );
    assert_eq!(s.world.private_store_type(B), PrivateStoreType::Buy);
    s.log.take();
    // Alice (10 piles, one sword) clicks him twice.
    for _ in 0..2 {
        s.input(
            A,
            ClientInput::Action {
                object_id: B.0,
                action_id: 0,
            },
        );
    }
    s.advance(500);
    let out = s.log.take();
    let list: PrivateStoreListBuy = decode(of(&out, A, op_name(ServerOpcode::PrivateStoreBuyList))[0]);
    assert_eq!(list.store_object_id, 2);
    assert_eq!(list.player_adena, 10);
    // The item block carries what Alice can sell (her object ids, her counts, at most what is
    // wanted); the last number is what the store still wants.
    let lines: Vec<(i32, i64, i64, i64)> = list
        .items
        .iter()
        .map(|e| (e.item.object_id, e.item.count, e.price, e.count))
        .collect();
    assert_eq!(lines, [(PILE_OBJ, 5, 20, 5), (SWORD_OBJ, 1, 300, 1)]);
}

// ---- buying from a sell store -------------------------------------------------------------------

#[test]
fn a_customer_buys_from_a_sell_store() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    s.buy(B, A, &[(SWORD_OBJ, 1, 500), (PILE_OBJ, 2, 30)]);
    let out = s.log.take();

    // 500 + 2 * 30 adena change hands; the sword moves whole, 2 of the 4 offered piles become a
    // new item of Bob's.
    assert_eq!(s.adena(B), 440);
    assert_eq!(s.adena(A), 570);
    let b_items = s.items_of(B);
    assert_eq!(b_items[0], (SWORD_OBJ, SWORD, 1));
    assert_eq!(b_items[1], (B_ADENA, ADENA, 440));
    assert_eq!((b_items[2].1, b_items[2].2), (PILE, 2));
    assert!(b_items[2].0 >= 0x6000_0000, "a new id from the Rust range");
    assert!(s.items_of(A).contains(&(PILE_OBJ, PILE, 8)));
    assert!(!s.items_of(A).iter().any(|i| i.0 == SWORD_OBJ));
    // The list lost the sword and 2 piles.
    let list = s.world.player(A).unwrap().trade.sell_list.clone().unwrap();
    let left: Vec<(i32, i64)> = list.items.iter().map(|t| (t.object_id.0, t.count)).collect();
    assert_eq!(left, [(PILE_OBJ, 2)]);
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::Sell);

    // The messages, in the order of the items (hash order of the object ids: 100 before 105).
    let to_owner = messages(&out, A);
    assert_eq!(
        to_owner.iter().map(|m| m.id).collect::<Vec<_>>(),
        [SystemMessageId::C1PurchasedS2, SystemMessageId::C1PurchasedS3S2S]
    );
    assert_eq!(
        to_owner[0].params,
        [
            SystemMessageParam::Text("Bob".into()),
            SystemMessageParam::ItemName(SWORD)
        ]
    );
    assert_eq!(
        to_owner[1].params,
        [
            SystemMessageParam::Text("Bob".into()),
            SystemMessageParam::ItemName(PILE),
            SystemMessageParam::LongNumber(2)
        ]
    );
    let to_buyer = messages(&out, B);
    assert_eq!(
        to_buyer.iter().map(|m| m.id).collect::<Vec<_>>(),
        [
            SystemMessageId::YouHavePurchasedS2FromC1,
            SystemMessageId::YouHavePurchasedS3S2SFromC1
        ]
    );
    assert_eq!(
        to_buyer[1].params,
        [
            SystemMessageParam::Text("Alice".into()),
            SystemMessageParam::ItemName(PILE),
            SystemMessageParam::LongNumber(2)
        ]
    );

    // The rows: the sword's row leaves Alice and joins Bob.
    let rows = item_changes(&out);
    assert!(rows.contains(&(1, vec![], vec![SWORD_OBJ])));
    assert!(rows.contains(&(2, vec![(SWORD_OBJ, "INVENTORY".to_owned())], vec![])));

    // The inventory updates go out 100 ms later.
    s.advance(100);
    let later = s.log.take();
    const ADDED: i16 = InventoryUpdate::ADDED;
    const MODIFIED: i16 = InventoryUpdate::MODIFIED;
    const REMOVED: i16 = InventoryUpdate::REMOVED;
    let b_updates = inventory_updates(&later, B);
    assert!(b_updates.contains(&(B_ADENA, MODIFIED, 440)));
    assert!(b_updates.contains(&(SWORD_OBJ, ADDED, 1)));
    assert!(b_updates.iter().any(|u| u.1 == ADDED && u.2 == 2));
    let a_updates = inventory_updates(&later, A);
    assert!(a_updates.contains(&(A_ADENA, MODIFIED, 570)));
    assert!(a_updates.contains(&(SWORD_OBJ, REMOVED, 1)));
    assert!(a_updates.contains(&(PILE_OBJ, MODIFIED, 8)));
}

#[test]
fn the_last_item_sold_closes_the_store() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    s.buy(B, A, &[(SWORD_OBJ, 1, 500), (PILE_OBJ, 4, 30)]);
    let out = s.log.take();
    assert!(
        s.world
            .player(A)
            .unwrap()
            .trade
            .sell_list
            .as_ref()
            .unwrap()
            .items
            .is_empty()
    );
    // `setPrivateStoreType(NONE)`, `broadcastUserInfo()`: Alice stays sitting.
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
    let user_info: UserInfo = decode(of(&out, A, op_name(ServerOpcode::UserInfo))[0]);
    assert_eq!(user_info.private_store_type, 0);
    assert!(s.world.is_sitting(A));
    assert_eq!(s.adena(A), 10 + 500 + 120);
}

#[test]
fn a_wrong_price_or_too_little_adena_stops_the_deal() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    // A price that is not the listed one: the line is not found, so the total is 0, and
    // `reduceAdena(0)` is false: YOU_DO_NOT_HAVE_ENOUGH_ADENA, ActionFailed.
    s.buy(B, A, &[(SWORD_OBJ, 1, 499)]);
    let out = s.log.take();
    assert_eq!(message_ids(&out, B), [SystemMessageId::YouDoNotHaveEnoughAdena]);
    assert_eq!(names(&out, B), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert_eq!(s.adena(B), 1000);
    assert!(s.items_of(A).contains(&(SWORD_OBJ, SWORD, 1)));

    // Not enough adena for the right price.
    s.world.player_mut(B).unwrap().data.items[0].count = 499;
    s.buy(B, A, &[(SWORD_OBJ, 1, 500)]);
    let out = s.log.take();
    assert_eq!(message_ids(&out, B), [SystemMessageId::YouDoNotHaveEnoughAdena]);
    assert_eq!(names(&out, B), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert_eq!(s.adena(B), 499);
}

#[test]
fn a_customer_cannot_buy_from_afar_or_from_somebody_without_a_store() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    // 250 away is not inside `INTERACTION_DISTANCE` (250): silence.
    s.world.set_position(B, 1250, 1000, 0);
    s.log.take();
    s.buy(B, A, &[(SWORD_OBJ, 1, 500)]);
    assert!(s.log.take().is_empty());
    s.world.set_position(B, 1249, 1000, 0);
    s.log.take();
    s.buy(B, A, &[(SWORD_OBJ, 1, 500)]);
    assert_eq!(s.items_of(B)[0], (SWORD_OBJ, SWORD, 1));

    // Alice has no store open, or the object is no player.
    let mut s = Scene::market();
    s.buy(B, A, &[(SWORD_OBJ, 1, 500)]);
    assert!(s.log.take().is_empty());
    s.buy(B, ObjectId(987_654), &[(SWORD_OBJ, 1, 500)]);
    assert!(s.log.take().is_empty());
    // A bad list: ActionFailed.
    s.store(
        B,
        StoreInput::Buy {
            store_player_id: A.0,
            items: None,
        },
    );
    assert_eq!(names(&s.log.take(), B), ["ACTION_FAIL"]);
    // A level that may not trade: the message and ActionFailed.
    s.open_sell_store(false);
    s.world.player_mut(B).unwrap().data.access_level = 30;
    s.buy(B, A, &[(SWORD_OBJ, 1, 500)]);
    assert_eq!(names(&s.log.take(), B), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);
}

#[test]
fn paying_with_every_adena_loses_it_like_in_java() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    s.world.player_mut(B).unwrap().data.items[0].count = 500;
    s.buy(B, A, &[(SWORD_OBJ, 1, 500)]);
    // NOTE(java-quirk): `playerInventory.getAdenaInstance()` is null once the adena is used up and
    // `playerIU.addItem(null)` throws a NullPointerException: the adena is gone, the seller is
    // not paid and nothing moves.
    assert_eq!(s.adena(B), 0);
    assert_eq!(s.adena(A), 10);
    assert!(s.items_of(A).contains(&(SWORD_OBJ, SWORD, 1)));
    assert!(!s.items_of(B).iter().any(|i| i.1 == SWORD));
}

#[test]
fn a_package_must_be_bought_whole() {
    let mut s = Scene::market();
    s.open_sell_store(true);
    // Fewer lines than the package has: `handleIllegalPlayerAction` (the default punishment tells
    // the player it will be kicked), and no deal.
    s.buy(B, A, &[(SWORD_OBJ, 1, 500)]);
    let out = s.log.take();
    assert!(
        texts(&out, B)
            .iter()
            .any(|t| t.contains("You will be kicked for illegal action"))
    );
    assert_eq!(s.adena(B), 1000);

    // The right number of lines but one that is not in the package: the same, with a failed deal.
    let mut s = Scene::market();
    s.open_sell_store(true);
    s.buy(B, A, &[(SWORD_OBJ, 1, 500), (PILE_OBJ, 4, 31)]);
    let out = s.log.take();
    assert!(
        texts(&out, B)
            .iter()
            .any(|t| t.contains("You will be kicked for illegal action"))
    );
    assert_eq!(s.adena(B), 1000);

    // The whole package.
    let mut s = Scene::market();
    s.open_sell_store(true);
    s.buy(B, A, &[(SWORD_OBJ, 1, 500), (PILE_OBJ, 4, 30)]);
    assert_eq!(s.adena(B), 1000 - 620);
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
}

#[test]
fn a_line_asking_for_more_than_listed_gets_what_is_listed() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    s.buy(B, A, &[(PILE_OBJ, 99, 30)]);
    // `if (ti.getCount() < item.getCount()) item.setCount(ti.getCount())`: 4 piles, 120 adena.
    assert_eq!(s.adena(B), 880);
    assert!(s.items_of(B).iter().any(|i| i.1 == PILE && i.2 == 4));
}

#[test]
fn nothing_is_counted_for_weight_and_slots_of_a_purchase() {
    // NOTE(java-quirk): a buy request has no item id, so the weight and the slots are never
    // counted: a customer who is 20 below the weight limit still receives the 50 weight sword.
    let mut s = Scene::market();
    s.open_sell_store(false);
    let max_load = i64::from(s.world.stats(B).unwrap().max_load());
    let load = i64::from(s.world.stats(B).unwrap().current_load());
    // A pile weighs 2.
    let piles = (max_load - load - 20) / 2;
    s.world
        .player_mut(B)
        .unwrap()
        .data
        .items
        .push(inv(320, PILE, piles));
    let free =
        i64::from(s.world.stats(B).unwrap().max_load()) - i64::from(s.world.stats(B).unwrap().current_load());
    assert!((20..=21).contains(&free), "free weight {free}");
    s.buy(B, A, &[(SWORD_OBJ, 1, 500)]);
    assert!(s.items_of(B).contains(&(SWORD_OBJ, SWORD, 1)));
}

// ---- the buy list and selling to a buy store ----------------------------------------------------

#[test]
fn confirming_a_buy_list_checks_the_funds_and_sits_down() {
    let mut s = Scene::market();
    s.store(B, StoreInput::ActionUse { action_id: 28 });
    s.log.take();
    // 100 piles at 20 are 2000 adena; Bob has 1000.
    s.store(
        B,
        StoreInput::ListBuy {
            items: Some(vec![want(PILE, 100, 20)]),
        },
    );
    let out = s.log.take();
    assert_eq!(
        names(&out, B),
        [op_name(ServerOpcode::PrivateStoreBuyManageList), "SYSTEM_MESSAGE"]
    );
    assert_eq!(
        message_ids(&out, B),
        [SystemMessageId::ThePurchasePriceIsHigherThanTheAmountOfMoneyThatYouHaveAndSoYouCannotOpenAPersonalStore]
    );
    assert_eq!(s.world.private_store_type(B), PrivateStoreType::BuyManage);

    s.store(B, StoreInput::MsgBuy { msg: "Piles".into() });
    s.store(
        B,
        StoreInput::ListBuy {
            items: Some(vec![want(PILE, 5, 20)]),
        },
    );
    let out = s.log.take();
    assert_eq!(s.world.private_store_type(B), PrivateStoreType::Buy);
    assert!(s.world.is_sitting(B));
    for who in [A, B] {
        let msg: PrivateStoreMsgBuy = decode(of(&out, who, op_name(ServerOpcode::PrivateStoreBuyMsg))[0]);
        assert_eq!((msg.object_id, msg.message.as_str()), (2, "Piles"));
    }
    let buy = s.world.player(B).unwrap().trade.buy_list.clone().unwrap();
    assert_eq!(buy.items.len(), 1);
    assert_eq!(
        (
            buy.items[0].object_id.0,
            buy.items[0].item.item_id,
            buy.items[0].count,
            buy.items[0].price
        ),
        (0, PILE, 5, 20)
    );
}

#[test]
fn the_buy_list_refuses_items_a_store_cannot_buy() {
    let mut s = Scene::market();
    s.store(B, StoreInput::ActionUse { action_id: 28 });
    // A quest item, an item that cannot be traded and an unknown id: `addItemByItemId` returns
    // null and the handler goes on.
    s.store(
        B,
        StoreInput::ListBuy {
            items: Some(vec![want(QUEST, 1, 1), want(BOUND, 1, 1), want(9999, 1, 1)]),
        },
    );
    assert_eq!(s.world.private_store_type(B), PrivateStoreType::Buy);
    assert!(
        s.world
            .player(B)
            .unwrap()
            .trade
            .buy_list
            .as_ref()
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn a_price_that_overflows_the_adena_limit_is_an_illegal_action() {
    let mut s = Scene::market();
    s.store(B, StoreInput::ActionUse { action_id: 28 });
    s.log.take();
    // `MAX_ADENA / count < price` (99 900 000 000 / 3).
    s.store(
        B,
        StoreInput::ListBuy {
            items: Some(vec![want(PILE, 3, 33_300_000_001)]),
        },
    );
    let out = s.log.take();
    assert!(
        texts(&out, B)
            .iter()
            .any(|t| t.contains("You will be kicked for illegal action"))
    );
    assert_eq!(s.world.private_store_type(B), PrivateStoreType::BuyManage);
}

#[test]
fn a_customer_sells_to_a_buy_store() {
    let mut s = Scene::market();
    s.world.player_mut(B).unwrap().data.items.push(inv(310, PILE, 3));
    s.store(B, StoreInput::ActionUse { action_id: 28 });
    s.store(
        B,
        StoreInput::ListBuy {
            items: Some(vec![want(PILE, 5, 20)]),
        },
    );
    s.log.take();
    s.sell_to(A, B, &[(PILE_OBJ, PILE, 3, 20)]);
    let out = s.log.take();
    // 3 piles at 20: Alice 10 -> 70, Bob 1000 -> 940; his stack of 3 piles grows to 6.
    assert_eq!(s.adena(A), 70);
    assert_eq!(s.adena(B), 940);
    assert!(s.items_of(A).contains(&(PILE_OBJ, PILE, 7)));
    assert!(s.items_of(B).contains(&(310, PILE, 6)));
    // The store still wants 2.
    let buy = s.world.player(B).unwrap().trade.buy_list.clone().unwrap();
    assert_eq!(buy.items[0].count, 2);
    assert_eq!(s.world.private_store_type(B), PrivateStoreType::Buy);

    // The owner reads "You purchased", the seller "<owner> purchased".
    let to_owner = messages(&out, B);
    assert_eq!(to_owner[0].id, SystemMessageId::YouHavePurchasedS3S2SFromC1);
    assert_eq!(
        to_owner[0].params,
        [
            SystemMessageParam::Text("Alice".into()),
            SystemMessageParam::ItemName(PILE),
            SystemMessageParam::LongNumber(3)
        ]
    );
    let to_seller = messages(&out, A);
    assert_eq!(to_seller[0].id, SystemMessageId::C1PurchasedS3S2S);
    assert_eq!(
        to_seller[0].params,
        [
            SystemMessageParam::Text("Bob".into()),
            SystemMessageParam::ItemName(PILE),
            SystemMessageParam::LongNumber(3)
        ]
    );

    // Selling the rest (asking for more than wanted is cut) empties the list and closes the store.
    s.sell_to(A, B, &[(PILE_OBJ, PILE, 50, 20)]);
    assert_eq!(s.adena(A), 110);
    assert_eq!(s.world.private_store_type(B), PrivateStoreType::None);
    assert!(s.items_of(A).contains(&(PILE_OBJ, PILE, 5)));
}

#[test]
fn selling_a_whole_stack_and_a_wrong_price() {
    let mut s = Scene::market();
    s.world.player_mut(B).unwrap().data.items[0].count = 5000;
    s.store(B, StoreInput::ActionUse { action_id: 28 });
    s.store(
        B,
        StoreInput::ListBuy {
            items: Some(vec![want(PILE, 50, 20), want(SWORD, 1, 300)]),
        },
    );
    assert_eq!(s.world.private_store_type(B), PrivateStoreType::Buy);
    s.log.take();
    // The wrong price: no line is found, `ok` stays false: ActionFailed.
    s.sell_to(A, B, &[(PILE_OBJ, PILE, 10, 19)]);
    assert_eq!(names(&s.log.take(), A), ["ACTION_FAIL"]);
    assert_eq!(s.adena(A), 10);

    // The whole stack of 10 piles and the sword (a whole item): both go.
    s.sell_to(A, B, &[(PILE_OBJ, PILE, 10, 20), (SWORD_OBJ, SWORD, 1, 300)]);
    assert_eq!(s.adena(A), 10 + 200 + 300);
    assert_eq!(s.adena(B), 4500);
    assert!(!s.items_of(A).iter().any(|i| i.1 == PILE || i.1 == SWORD));
    assert!(s.items_of(B).iter().any(|i| i.1 == SWORD));
    s.advance(100);
    let later = s.log.take();
    let a_updates = inventory_updates(&later, A);
    assert!(
        a_updates
            .iter()
            .any(|u| u.0 == PILE_OBJ && u.1 == InventoryUpdate::REMOVED)
    );
    assert!(a_updates.contains(&(A_ADENA, InventoryUpdate::MODIFIED, 510)));
}

#[test]
fn a_buy_store_that_spends_all_its_adena_reports_the_adena_as_removed() {
    let mut s = Scene::market();
    s.store(B, StoreInput::ActionUse { action_id: 28 });
    s.world.player_mut(B).unwrap().data.items[0].count = 60;
    s.store(
        B,
        StoreInput::ListBuy {
            items: Some(vec![want(PILE, 3, 20)]),
        },
    );
    s.log.take();
    s.sell_to(A, B, &[(PILE_OBJ, PILE, 3, 20)]);
    assert_eq!(s.adena(B), 0);
    assert_eq!(s.adena(A), 70);
    // `ownerIU.addItem(adenaItem)` was taken before the adena item was used up: it is a removal
    // with count 0 and the owner still hears about the change.
    s.advance(100);
    let later = s.log.take();
    assert!(inventory_updates(&later, B).contains(&(B_ADENA, InventoryUpdate::REMOVED, 0)));
}

// ---- titles -------------------------------------------------------------------------------------

#[test]
fn a_title_of_thirty_characters_is_an_illegal_action() {
    let mut s = Scene::market();
    s.store(A, StoreInput::ActionUse { action_id: 10 });
    s.log.take();
    let ok = "x".repeat(29);
    s.store(A, StoreInput::MsgSell { msg: ok.clone() });
    let out = s.log.take();
    assert_eq!(
        decode::<PrivateStoreMsgSell>(of(&out, A, op_name(ServerOpcode::PrivateStoreSellMsg))[0]).message,
        ok
    );
    s.store(A, StoreInput::MsgSell { msg: "y".repeat(30) });
    let out = s.log.take();
    assert!(of(&out, A, op_name(ServerOpcode::PrivateStoreSellMsg)).is_empty());
    assert!(
        texts(&out, A)
            .iter()
            .any(|t| t.contains("You will be kicked for illegal action"))
    );
    // The title was not changed.
    assert_eq!(
        s.world.player(A).unwrap().trade.sell_list.as_ref().unwrap().title,
        ok
    );
}

// ---- closing a store ----------------------------------------------------------------------------

#[test]
fn quitting_a_store_closes_it_and_stands_up() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    // The sit animation takes 2.5 s; standing up waits for it.
    s.advance(3000);
    s.log.take();
    s.store(A, StoreInput::QuitSell);
    let out = s.log.take();
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
    let wait: ChangeWaitType = decode(of(&out, A, op_name(ServerOpcode::ChangeWaitType))[0]);
    assert_eq!(wait.move_type, ChangeWaitType::WT_STANDING);
    let user_info: UserInfo = decode(of(&out, A, op_name(ServerOpcode::UserInfo))[0]);
    assert_eq!(user_info.private_store_type, 0);
    // The list is kept, as in Java.
    assert_eq!(
        s.world
            .player(A)
            .unwrap()
            .trade
            .sell_list
            .as_ref()
            .unwrap()
            .items
            .len(),
        2
    );
}

#[test]
fn a_player_with_a_store_does_not_stand_up_on_its_own() {
    // `standUp()` does nothing in store mode.
    let mut s = Scene::market();
    s.open_sell_store(false);
    s.advance(3000);
    s.log.take();
    s.world.stand_up(A);
    assert!(s.log.take().is_empty());
    assert!(s.world.is_sitting(A));
}

// ---- the markers other systems left for the stores ---------------------------------------------

#[test]
fn a_store_blocks_using_destroying_dropping_trading_and_restarting() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    s.advance(3000);
    s.log.take();

    // `UseItem`: YOU_MAY_NOT_USE_ITEMS_IN_A_PRIVATE_STORE_OR_PRIVATE_WORK_SHOP, ActionFailed.
    s.input(
        A,
        ClientInput::UseItem {
            object_id: PILE_OBJ,
            ctrl_pressed: false,
        },
    );
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::YouMayNotUseItemsInAPrivateStoreOrPrivateWorkShop]
    );
    assert_eq!(names(&out, A), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);

    // `RequestDestroyItem` and `RequestDropItem`.
    s.input(
        A,
        ClientInput::DestroyItem {
            object_id: PILE_OBJ,
            count: 1,
        },
    );
    s.input(
        A,
        ClientInput::DropItem {
            object_id: PILE_OBJ,
            count: 1,
            x: 1000,
            y: 1000,
            z: 0,
        },
    );
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [
            SystemMessageId::WhileOperatingAPrivateStoreOrWorkshopYouCannotDiscardDestroyOrTradeAnItem,
            SystemMessageId::WhileOperatingAPrivateStoreOrWorkshopYouCannotDiscardDestroyOrTradeAnItem
        ]
    );
    assert!(s.items_of(A).contains(&(PILE_OBJ, PILE, 10)));

    // `TradeRequest`, from either side.
    s.input(B, ClientInput::TradeRequest { object_id: A.0 });
    assert_eq!(
        message_ids(&s.log.take(), B),
        [SystemMessageId::WhileOperatingAPrivateStoreOrWorkshopYouCannotDiscardDestroyOrTradeAnItem]
    );
    s.input(A, ClientInput::TradeRequest { object_id: B.0 });
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::WhileOperatingAPrivateStoreOrWorkshopYouCannotDiscardDestroyOrTradeAnItem]
    );

    // `RequestRestart`: "Cannot restart while trading." and RestartResponse(false).
    s.input(A, ClientInput::Restart);
    let out = s.log.take();
    assert_eq!(names(&out, A), ["SYSTEM_MESSAGE", "RESTART_RESPONSE"]);
    assert!(s.world.player(A).is_some());
}

#[test]
fn a_store_owner_does_not_attack_on_a_second_click() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    s.advance(3000);
    // Select Bob, then ask for the attack: `AttackRequest` refuses (`!isInStoreMode()`).
    s.input(
        A,
        ClientInput::Action {
            object_id: B.0,
            action_id: 0,
        },
    );
    s.log.take();
    s.input(
        A,
        ClientInput::AttackRequest {
            object_id: B.0,
            attack_id: 0,
        },
    );
    assert_eq!(names(&s.log.take(), A), ["ACTION_FAIL"]);
}

#[test]
fn a_closed_store_cannot_be_bought_from() {
    let mut s = Scene::market();
    s.open_sell_store(false);
    s.store(A, StoreInput::QuitSell);
    s.log.take();
    s.buy(B, A, &[(SWORD_OBJ, 1, 500)]);
    assert!(s.log.take().is_empty());
}

#[test]
fn damage_closes_the_store_and_stands_up() {
    // `PlayerStatus.reduceHp`: "Attacked players in craft/shops stand up."
    let mut s = Scene::market();
    s.open_sell_store(false);
    s.advance(3000);
    s.log.take();
    s.world.reduce_current_hp(A, 5.0, Some(B));
    let out = s.log.take();
    assert_eq!(s.world.private_store_type(A), PrivateStoreType::None);
    let wait: ChangeWaitType = decode(of(&out, A, op_name(ServerOpcode::ChangeWaitType))[0]);
    assert_eq!(wait.move_type, ChangeWaitType::WT_STANDING);
    let user_info: UserInfo = decode(of(&out, A, op_name(ServerOpcode::UserInfo))[0]);
    assert_eq!(user_info.private_store_type, 0);
}
