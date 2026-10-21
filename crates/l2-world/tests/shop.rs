#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! NPC shops (M6-SHOP-01): the `Buy` bypass, `RequestBuyItem`, `RequestSellItem`,
//! `RequestRefundItem`, limited stock and its restock, with the messages, packets and numbers of
//! Java's `Merchant`, `RequestBuyItem`, `RequestSellItem`, `RequestRefundItem`, `Product` and
//! `BuyListTaskManager`. The expected values come from the Java code named in each test.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, BuyListOptions, BuyLists, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, MapRegions,
    MerchantPriceConfigs, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillTemplates, SpawnData,
    SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::client::{
    BuyItemEntry, RequestBuyItem, RequestRefundItem, RequestSellItem, SellItemEntry,
};
use l2_protocol_game::server::{BuyList, ExBuySellList, StatusUpdate, SystemMessage, SystemMessageParam};
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

const ADENA: i32 = 57;
const SHOT: i32 = 1835;
const SWORD: i32 = 2;
const CUIRASS: i32 = 4;
const LIMITED: i32 = 99;
const BOUND: i32 = 98;
const ANVIL: i32 = 20;
const FREE: i32 = 55;
const GUARD: i32 = 3960;
const HAT: i32 = 5;

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="C Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /><set name="price" val="1000" /></item>
<item id="4" type="Armor" name="Cuirass"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="weight" val="10" /><set name="price" val="2000" /></item>
<item id="5" type="Armor" name="Hat"><set name="armor_type" val="LIGHT" /><set name="bodypart" val="head" /><set name="weight" val="2" /><set name="price" val="400" /></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /><set name="is_sellable" val="false" /><set name="price" val="1" /></item>
<item id="1835" type="EtcItem" name="Soulshot"><set name="is_stackable" val="true" /><set name="weight" val="1" /><set name="price" val="20" /></item>
<item id="99" type="EtcItem" name="Limited"><set name="is_stackable" val="true" /><set name="weight" val="1" /><set name="price" val="100" /></item>
<item id="98" type="EtcItem" name="Bound"><set name="is_sellable" val="false" /><set name="weight" val="1" /><set name="price" val="40" /></item>
<item id="20" type="EtcItem" name="Anvil"><set name="is_stackable" val="true" /><set name="weight" val="100000" /><set name="price" val="10" /></item>
<item id="55" type="EtcItem" name="Free thing"><set name="is_stackable" val="true" /><set name="weight" val="1" /><set name="price" val="0" /></item>
<item id="3960" type="EtcItem" name="Siege guard"><set name="is_stackable" val="true" /><set name="weight" val="1" /><set name="price" val="100" /></item>
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

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="30001" type="Merchant" level="10" name="Trader" usingServerSideName="true"/>
  <npc id="30002" type="Merchant" level="10" name="Other trader" usingServerSideName="true"/>
  <npc id="30003" type="Folk" level="10" name="Villager" usingServerSideName="true"/>
</list>"#;

/// List 100 is Trader's; 101 belongs to the other trader; 423 is the community board list.
const BUY_LISTS: [(&str, &str); 3] = [
    (
        "0000100.xml",
        r#"<list><npcs><npc>30001</npc></npcs>
            <item id="1835" />
            <item id="2" price="1500" />
            <item id="99" price="100" count="5" restock_delay="2" />
            <item id="98" price="60" />
            <item id="20" price="10" />
            <item id="55" price="0" />
            <item id="3960" price="100" />
            <item id="4" price="2000" />
            <item id="5" price="400" />
        </list>"#,
    ),
    (
        "0000101.xml",
        r#"<list><npcs><npc>30002</npc></npcs><item id="1835" /></list>"#,
    ),
    ("0000423.xml", r#"<list><item id="1835" price="20" /></list>"#),
];

const PRICE_CONFIGS: &str = r#"<merchantPriceConfig defaultPriceConfig="18">
    <priceConfig id="0" name="Giran" baseTax="25" castleId="3" zoneId="11020" />
    <priceConfig id="5" name="Gludin" baseTax="30" castleId="1" zoneId="11025" />
    <priceConfig id="18" name="Neutral" baseTax="50" />
</merchantPriceConfig>"#;

// ---- helpers ------------------------------------------------------------------------------------

const A: ObjectId = ObjectId(1);
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const T0: i64 = 1_700_000_000_000;

/// What `handleIllegalPlayerAction` tells the player with the default `Kick` punishment.
const KICK: &str = "You will be kicked for illegal action, GM informed.";

fn inv(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".into(),
        ..Default::default()
    }
}

fn player(id: ObjectId, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: format!("P{}", id.0),
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
        x: X,
        y: Y,
        z: Z,
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

struct Root(PathBuf);

impl Root {
    fn new(tag: &str, zone_xml: Option<&str>) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-shop-{tag}-{}", std::process::id()));
        let write = |path: &str, text: &str| {
            let file = dir.join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        write(
            "data/html/merchant/30001.htm",
            "<html><body>Buy: <a action=\"bypass -h npc_%objectId%_Buy 100\">shop</a>\
             <a action=\"bypass -h npc_%objectId%_Buy 101\">other</a>\
             <a action=\"bypass -h npc_%objectId%_Buy 999\">missing</a>\
             <a action=\"bypass -h npc_%objectId%_Buy x\">bad</a></body></html>",
        );
        write(
            "data/html/default/30003.htm",
            "<html><body><a action=\"bypass -h npc_%objectId%_Buy 100\">nope</a></body></html>",
        );
        write(
            "data/html/npcdefault.htm",
            "<html><body>I have nothing to say to you.</body></html>",
        );
        if let Some(zone_xml) = zone_xml {
            write(
                "data/zones/test.xml",
                &format!("<?xml version=\"1.0\"?>\n<list enabled=\"true\">\n{zone_xml}\n</list>\n"),
            );
        }
        Self(dir)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Scene {
    world: World,
    log: Log,
    ids: Arc<IdFactory>,
    settings: WorldSettings,
    _root: Root,
}

impl Scene {
    fn new(tag: &str) -> Self {
        Self::with(tag, WorldSettings::default(), None)
    }

    fn with(tag: &str, mut settings: WorldSettings, zone_xml: Option<&str>) -> Self {
        settings.shop.clock_override_unix_ms.get_or_insert(T0);
        let root = Root::new(tag, zone_xml);
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
        let data = data();
        let items = Arc::clone(&data.items);
        world.set_static_data(Box::new(data));
        world.set_settings(settings.clone());
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        if zone_xml.is_some() {
            world.set_zone_data(
                Arc::new(Zones::load(root.path())),
                Arc::new(MapRegions::load(root.path())),
            );
        }
        let lists = BuyLists::from_xml(&BUY_LISTS, &items, &BuyListOptions::default());
        world.set_buy_lists(
            Arc::new(lists),
            Arc::new(MerchantPriceConfigs::from_xml(PRICE_CONFIGS).unwrap()),
        );
        Self {
            world,
            log,
            ids,
            settings,
            _root: root,
        }
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

    /// One player with `items` and a trader (30001) 50 away, targeted by one click.
    fn with_trader(tag: &str, items: Vec<ItemData>) -> (Self, ObjectId) {
        let mut s = Self::new(tag);
        let npc = s.start(items);
        (s, npc)
    }

    fn start(&mut self, items: Vec<ItemData>) -> ObjectId {
        self.enter(player(A, items));
        let npc = self.world.spawn_npc_at(30001, X + 50, Y, Z, 0).unwrap();
        // The first click selects the trader, the second talks to it (which registers the bypasses
        // of its page).
        self.input(
            A,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
        self.input(
            A,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
        self.log.take();
        npc
    }

    /// The `Buy <list>` link of the trader's page.
    fn open_window(&mut self, npc: ObjectId, list_id: i32) {
        self.input(
            A,
            ClientInput::Bypass {
                command: format!("npc_{}_Buy {list_id}", npc.0),
            },
        );
    }

    fn input(&mut self, who: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input,
        });
    }

    fn buy(&mut self, list_id: i32, items: &[(i32, i64)]) {
        let items = items
            .iter()
            .map(|&(item_id, count)| BuyItemEntry { item_id, count })
            .collect();
        self.input(
            A,
            ClientInput::Shop(ShopInput::Buy(RequestBuyItem {
                list_id,
                items: Some(items),
            })),
        );
    }

    fn sell(&mut self, list_id: i32, items: &[(i32, i32, i64)]) {
        let items = items
            .iter()
            .map(|&(object_id, item_id, count)| SellItemEntry {
                object_id,
                item_id,
                count,
            })
            .collect();
        self.input(
            A,
            ClientInput::Shop(ShopInput::Sell(RequestSellItem {
                list_id,
                items: Some(items),
            })),
        );
    }

    fn refund(&mut self, list_id: i32, indexes: &[i32]) {
        self.input(
            A,
            ClientInput::Shop(ShopInput::Refund(RequestRefundItem {
                list_id,
                items: Some(indexes.to_vec()),
            })),
        );
    }

    fn advance(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.world.tick();
        }
    }

    fn set_clock(&mut self, ms: i64) {
        self.settings.shop.clock_override_unix_ms = Some(ms);
        self.world.set_settings(self.settings.clone());
    }

    fn items(&self) -> Vec<(i32, i32, i64, String)> {
        let mut items: Vec<_> = self
            .world
            .player(A)
            .unwrap()
            .data
            .items
            .iter()
            .map(|i| (i.object_id.0, i.item_id, i.count, i.loc.clone()))
            .collect();
        items.sort_unstable();
        items
    }

    fn adena(&self) -> i64 {
        self.world.adena_of(A)
    }

    fn refund_list(&self) -> Vec<(i32, i32, i64)> {
        self.world
            .player(A)
            .unwrap()
            .shop
            .refund
            .iter()
            .map(|i| (i.object_id.0, i.item_id, i.count))
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

fn packets(out: &[Out], to: ObjectId) -> Vec<&[u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, bytes) if *id == to.0 => Some(bytes.as_slice()),
            _ => None,
        })
        .collect()
}

fn names(out: &[Out]) -> Vec<String> {
    packets(out, A).into_iter().map(name_of).collect()
}

/// The `BuyList`s (merchant side: the first int is 0) of the log.
fn buy_lists(out: &[Out]) -> Vec<BuyList> {
    packets(out, A)
        .into_iter()
        .filter(|b| name_of(b) == "EX_BUY_SELL_LIST" && b[3..7] == [0, 0, 0, 0])
        .map(decode::<BuyList>)
        .collect()
}

/// The `ExBuySellList`s (player side: the first int is 1) of the log.
fn sell_lists(out: &[Out]) -> Vec<ExBuySellList> {
    packets(out, A)
        .into_iter()
        .filter(|b| name_of(b) == "EX_BUY_SELL_LIST" && b[3..7] == [1, 0, 0, 0])
        .map(decode::<ExBuySellList>)
        .collect()
}

fn system_ids(out: &[Out]) -> Vec<SystemMessageId> {
    packets(out, A)
        .into_iter()
        .filter(|b| name_of(b) == "SYSTEM_MESSAGE")
        .map(decode::<SystemMessage>)
        .filter(|m| m.id != SystemMessageId::S12)
        .map(|m| m.id)
        .collect()
}

fn texts(out: &[Out]) -> Vec<String> {
    packets(out, A)
        .into_iter()
        .filter(|b| name_of(b) == "SYSTEM_MESSAGE")
        .map(decode::<SystemMessage>)
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn product_rows(out: &[Out]) -> Vec<ProductRow> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::BuyList(row)) => Some(row.clone()),
            _ => None,
        })
        .collect()
}

/// `(owner, upserted (object id, loc, count), removed object ids)` of every `ItemChanges`.
type Rows = (Vec<(i32, String, i64)>, Vec<i32>);

fn item_changes(out: &[Out]) -> Vec<Rows> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemChanges { upserts, removed, .. }) => Some((
                upserts
                    .iter()
                    .map(|i| (i.object_id.0, i.loc.clone(), i.count))
                    .collect(),
                removed.iter().map(|o| o.0).collect(),
            )),
            _ => None,
        })
        .collect()
}

fn ends_with(names: &[String], tail: &[&str]) {
    assert!(
        names.len() >= tail.len() && names[names.len() - tail.len()..] == *tail,
        "{names:?} does not end with {tail:?}"
    );
}

// ---- the Buy bypass -----------------------------------------------------------------------------

/// `Merchant.showBuyWindow`: `BuyList`, `ExBuySellList(player, false)`, `ActionFailed`; the town
/// tax is added to every price; the inventory is blocked for 1.5 s.
#[test]
fn the_buy_bypass_opens_the_window_with_the_base_tax_and_blocks_the_inventory() {
    let (mut s, npc) = Scene::with_trader("bypass", vec![inv(1001, ADENA, 5000), inv(1002, SWORD, 1)]);
    s.open_window(npc, 100);
    let out = s.log.take();
    // The handler's ActionFailed, then the `npc_` branch's own.
    assert_eq!(
        names(&out),
        [
            "EX_BUY_SELL_LIST",
            "EX_BUY_SELL_LIST",
            "ACTION_FAIL",
            "ACTION_FAIL"
        ]
    );

    let list = &buy_lists(&out)[0];
    assert_eq!((list.money, list.list_id, list.size), (5000, 100, 9));
    // No config has the NPC's zone: the default one adds 50% (`(long) (price * (1 + 0.5))`).
    let price = |item_id: i32| list.products.iter().find(|p| p.item_id == item_id).unwrap().price;
    assert_eq!(price(SHOT), 30); // reference price 20
    assert_eq!(price(SWORD), 2250);
    assert_eq!(price(LIMITED), 150);
    assert_eq!(price(FREE), 0);
    // The limited product shows its stock, the others 0; the item data is the template's.
    let limited = list.products.iter().find(|p| p.item_id == LIMITED).unwrap();
    assert_eq!((limited.count, limited.type2, limited.type1), (5, 5, 4));
    let sword = list.products.iter().find(|p| p.item_id == SWORD).unwrap();
    assert_eq!(
        (sword.count, sword.type2, sword.type1, sword.body_part),
        (0, 0, 0, 0x80)
    );

    // The sell side: the sword (adena is not for sale), no refund yet, not done.
    let sides = &sell_lists(&out)[0];
    assert_eq!(sides.sell_list.len(), 1);
    assert_eq!(
        (sides.sell_list[0].item.item_id, sides.sell_list[0].price),
        (SWORD, 500)
    );
    assert!(sides.refund_list.is_empty() && !sides.done);

    // `setInventoryBlockingStatus(true)`: `RequestItemList` is dropped for 1.5 s (30 ticks).
    s.input(A, ClientInput::ItemList);
    s.advance(10);
    assert!(!names(&s.log.take()).contains(&"ITEM_LIST".to_owned()));
    s.advance(30);
    s.log.take();
    s.input(A, ClientInput::ItemList);
    s.advance(6);
    assert!(names(&s.log.take()).contains(&"ITEM_LIST".to_owned()));
}

/// `RequestBuySellUIClose`: `sendItemList(true)`, unless the inventory is still disabled.
#[test]
fn closing_the_shop_window_refreshes_the_item_list_once_the_inventory_is_enabled() {
    let (mut s, npc) = Scene::with_trader("close", vec![inv(1001, ADENA, 100)]);
    s.open_window(npc, 100);
    s.log.take();
    s.input(A, ClientInput::Shop(ShopInput::UiClose));
    s.advance(10);
    assert!(!names(&s.log.take()).contains(&"ITEM_LIST".to_owned()));

    s.advance(30);
    s.input(A, ClientInput::Shop(ShopInput::UiClose));
    s.advance(6);
    let out = s.log.take();
    let lists: Vec<l2_protocol_game::server::ItemList> = packets(&out, A)
        .into_iter()
        .filter(|b| name_of(b) == "ITEM_LIST")
        .map(decode)
        .collect();
    assert_eq!(lists.len(), 1);
    assert!(lists[0].show_window);
}

#[test]
fn the_buy_bypass_refuses_unknown_lists_foreign_npcs_and_bad_numbers() {
    let (mut s, npc) = Scene::with_trader("bypass_refused", vec![inv(1001, ADENA, 5000)]);

    // List 101 belongs to another trader, 999 does not exist: `ActionFailed` twice, no window.
    for list in ["101", "999"] {
        s.input(
            A,
            ClientInput::Bypass {
                command: format!("npc_{}_Buy {list}", npc.0),
            },
        );
        assert_eq!(
            names(&s.log.take()),
            ["ACTION_FAIL", "ACTION_FAIL"],
            "list {list}"
        );
    }
    // `Integer.parseInt` fails: the handler returns false, only the `npc_` ActionFailed remains.
    s.input(
        A,
        ClientInput::Bypass {
            command: format!("npc_{}_Buy x", npc.0),
        },
    );
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
    assert!(!s.world.player(A).unwrap().shop.inventory_disabled);
}

#[test]
fn the_buy_bypass_needs_a_merchant() {
    let mut s = Scene::new("bypass_folk");
    s.enter(player(A, vec![inv(1001, ADENA, 5000)]));
    let npc = s.world.spawn_npc_at(30003, X + 50, Y, Z, 0).unwrap();
    s.input(
        A,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.input(
        A,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.log.take();
    // `Buy.onCommand`: `target instanceof Merchant` fails for a villager.
    s.input(
        A,
        ClientInput::Bypass {
            command: format!("npc_{}_Buy 100", npc.0),
        },
    );
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
}

// ---- RequestBuyItem -----------------------------------------------------------------------------

#[test]
fn buying_charges_the_taxed_price_and_adds_the_items() {
    let (mut s, _npc) = Scene::with_trader("buy", vec![inv(1001, ADENA, 10_000)]);
    s.buy(100, &[(SHOT, 10), (SWORD, 1)]);
    let out = s.log.take();
    // 10 x 30 + 2250
    assert_eq!(s.adena(), 10_000 - 300 - 2250);
    let items = s.items();
    assert!(items.contains(&(1001, ADENA, 7450, "INVENTORY".into())));
    assert_eq!(items.iter().filter(|i| i.1 == SHOT).map(|i| i.2).sum::<i64>(), 10);
    assert_eq!(items.iter().filter(|i| i.1 == SWORD).count(), 1);
    // `StatusUpdate(CUR_LOAD)`, `ExBuySellList(player, true)`, THE_TRANSACTION_IS_COMPLETE.
    ends_with(
        &names(&out),
        &["STATUS_UPDATE", "EX_BUY_SELL_LIST", "SYSTEM_MESSAGE"],
    );
    assert_eq!(system_ids(&out), [SystemMessageId::TheTransactionIsComplete]);
    let done = &sell_lists(&out)[0];
    assert!(done.done);
    let status = packets(&out, A)
        .into_iter()
        .rev()
        .find(|b| name_of(b) == "STATUS_UPDATE")
        .map(decode::<StatusUpdate>)
        .unwrap();
    assert_eq!(status.object_id, 1);
    // The new items are not written yet (Java: the next store inserts them).
    assert!(item_changes(&out).is_empty());
}

#[test]
fn buying_siege_guard_items_uses_the_guard_rate() {
    let mut settings = WorldSettings::default();
    settings.shop.rate_siege_guards_price = 2.0;
    let mut s = Scene::with("guard", settings, None);
    s.start(vec![inv(1001, ADENA, 10_000)]);
    s.input(A, ClientInput::ItemList);
    s.buy(100, &[(GUARD, 2)]);
    // `price *= rate` (200), then the tax: 200 * 1.5 = 300 each.
    assert_eq!(s.adena(), 10_000 - 600);
}

#[test]
fn buying_needs_a_merchant_in_range_and_a_list_it_may_sell() {
    let (mut s, npc) = Scene::with_trader("buy_target", vec![inv(1001, ADENA, 10_000)]);
    // The other trader's list: `ActionFailed`.
    s.buy(101, &[(SHOT, 1)]);
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
    // Out of `INTERACTION_DISTANCE` (250): `isInsideRadius3D` is a strict `<`.
    s.world.set_position(npc, X + 250, Y, Z);
    s.log.take();
    s.buy(100, &[(SHOT, 1)]);
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
    s.world.set_position(npc, X + 249, Y, Z);
    s.log.take();
    s.buy(100, &[(SHOT, 1)]);
    assert!(system_ids(&s.log.take()).contains(&SystemMessageId::TheTransactionIsComplete));
    // No target at all.
    s.input(A, ClientInput::TargetCanceled { unselect: 0 });
    s.log.take();
    s.buy(100, &[(SHOT, 1)]);
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
}

#[test]
fn a_bad_packet_or_a_chaotic_player_gets_action_failed() {
    let (mut s, _npc) = Scene::with_trader("buy_bad", vec![inv(1001, ADENA, 10_000)]);
    // `readImpl` left the list null.
    s.input(
        A,
        ClientInput::Shop(ShopInput::Buy(RequestBuyItem {
            list_id: 100,
            items: None,
        })),
    );
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
    // More entries than `MAX_ITEM_IN_PACKET` (the largest inventory limit, 250 by default).
    let many: Vec<(i32, i64)> = vec![(SHOT, 1); 251];
    s.buy(100, &many);
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);

    // `AltKarmaPlayerCanShop = False` and karma > 0.
    let mut settings = WorldSettings::default();
    settings.talk.karma_player_can_shop = false;
    let mut s = Scene::with("buy_karma", settings, None);
    let mut data = player(A, vec![inv(1001, ADENA, 10_000)]);
    data.karma = 5;
    s.enter(data);
    let npc = s.world.spawn_npc_at(30001, X + 50, Y, Z, 0).unwrap();
    s.input(
        A,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.log.take();
    s.buy(100, &[(SHOT, 1)]);
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
    assert_eq!(s.adena(), 10_000);
}

/// Each of these is `PunishmentManager.handleIllegalPlayerAction(..., DEFAULT_PUNISH)` (Kick by
/// default: "You will be kicked for illegal action, GM informed."), and nothing is bought.
#[test]
fn buying_exploits_are_illegal_actions() {
    let (mut s, _npc) = Scene::with_trader("buy_illegal", vec![inv(1001, ADENA, 10_000_000)]);

    // A list that does not exist.
    s.buy(999, &[(SHOT, 1)]);
    assert_eq!(texts(&s.log.take()), [KICK]);
    // An item the list does not sell.
    s.buy(100, &[(1234, 1)]);
    assert_eq!(texts(&s.log.take()), [KICK]);
    // Several non-stackable items at once: the illegal action, then the message.
    s.buy(100, &[(SWORD, 2)]);
    let out = s.log.take();
    assert_eq!(texts(&out), [KICK]);
    assert_eq!(
        system_ids(&out),
        [SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted]
    );
    // A price of 0 is for GMs only (`OnlyGMItemsFree`).
    s.buy(100, &[(FREE, 1)]);
    let out = s.log.take();
    assert_eq!(
        texts(&out),
        ["Ohh Cheat does not work? You have a problem now!", KICK]
    );
    // More than MaxAdena worth of goods: `MAX_ADENA / count < price`.
    s.buy(100, &[(SHOT, 5_000_000_000_000)]);
    assert_eq!(texts(&s.log.take()), [KICK]);
    assert_eq!(s.adena(), 10_000_000);
    assert_eq!(s.items().len(), 1);
}

#[test]
fn buying_checks_adena_weight_and_slots() {
    let settings = WorldSettings {
        inventory_maximum_no_dwarf: 3,
        ..WorldSettings::default()
    };
    let mut s = Scene::with("buy_limits", settings, None);
    s.start(vec![inv(1001, ADENA, 1000)]);

    // 1000 adena is not enough for 40 shots at 30.
    s.buy(100, &[(SHOT, 40)]);
    let out = s.log.take();
    assert_eq!(system_ids(&out), [SystemMessageId::YouDoNotHaveEnoughAdena]);
    ends_with(&names(&out), &["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert_eq!(s.adena(), 1000);

    // Weight: two anvils are 200000, far above the load limit of a level 1 character.
    s.buy(100, &[(ANVIL, 2)]);
    let out = s.log.take();
    assert_eq!(system_ids(&out), [SystemMessageId::YouHaveExceededTheWeightLimit]);
    assert_eq!(s.adena(), 1000);

    // Slots: the limit is 3, adena holds one, two new kinds fit, three do not.
    s.buy(100, &[(SHOT, 1), (HAT, 1), (CUIRASS, 1)]);
    let out = s.log.take();
    assert_eq!(system_ids(&out), [SystemMessageId::YourInventoryIsFull]);
    assert_eq!(s.adena(), 1000);
    s.buy(100, &[(SHOT, 1), (HAT, 1)]);
    assert_eq!(
        system_ids(&s.log.take()),
        [SystemMessageId::TheTransactionIsComplete]
    );
}

#[test]
fn a_gm_buys_without_a_merchant_and_without_tax() {
    let mut s = Scene::new("buy_gm");
    let mut gm = player(A, vec![inv(1001, ADENA, 10_000)]);
    gm.access_level = 100;
    s.enter(gm);
    s.log.take();
    // No target, and list 101 is not this player's merchant's: a GM skips both checks, and
    // `merchant == null` means no tax.
    s.buy(101, &[(SHOT, 10)]);
    assert_eq!(s.adena(), 10_000 - 200);
    // A price of 0 is fine for a GM.
    s.buy(100, &[(FREE, 3)]);
    assert_eq!(s.adena(), 10_000 - 200);
    // The community board list needs no merchant for anybody.
    let mut s = Scene::new("buy_cb");
    s.enter(player(A, vec![inv(1001, ADENA, 10_000)]));
    s.log.take();
    s.buy(423, &[(SHOT, 10)]);
    assert_eq!(s.adena(), 10_000 - 200);
}

// ---- limited stock --------------------------------------------------------------------------

/// `Product.decreaseCount`: the stock drops, the row is saved with the time the product restocks
/// (`now + restock_delay`, set by the first purchase only), the product vanishes from the window
/// when it is gone, and the `BuyListTaskManager` scan (60 s) and save task (50 ms) refill it.
#[test]
fn limited_stock_counts_down_and_restocks() {
    let (mut s, _npc) = Scene::with_trader("stock", vec![inv(1001, ADENA, 100_000)]);
    assert_eq!(s.world.product_count(100, LIMITED), 5);

    // More than there is: ActionFailed.
    s.buy(100, &[(LIMITED, 6)]);
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
    assert_eq!(s.world.product_count(100, LIMITED), 5);

    s.buy(100, &[(LIMITED, 3)]);
    let out = s.log.take();
    assert_eq!(
        product_rows(&out),
        [ProductRow {
            list_id: 100,
            item_id: LIMITED,
            count: 2,
            next_restock_time: T0 + 120_000,
        }]
    );
    assert_eq!(s.world.product_count(100, LIMITED), 2);
    // 3 x 150
    assert_eq!(s.adena(), 100_000 - 450);

    // The second purchase keeps the restock time of the first (`putIfAbsent`).
    s.set_clock(T0 + 60_000);
    s.buy(100, &[(LIMITED, 2)]);
    assert_eq!(
        product_rows(&s.log.take()),
        [ProductRow {
            list_id: 100,
            item_id: LIMITED,
            count: 0,
            next_restock_time: T0 + 120_000,
        }]
    );
    // The scans after 1 s and after 61 s do nothing before the time.
    s.advance(20 + 1200);
    assert!(product_rows(&s.log.take()).is_empty());
    s.set_clock(T0 + 120_001);
    // The scan at 121 s queues it; the save task restocks it on the next tick.
    s.advance(1202);
    // The scan found it due; the save task restocked it on the next tick.
    assert_eq!(
        product_rows(&s.log.take()),
        [ProductRow {
            list_id: 100,
            item_id: LIMITED,
            count: 5,
            next_restock_time: 0,
        }]
    );
    assert_eq!(s.world.product_count(100, LIMITED), 5);
}

#[test]
fn a_sold_out_product_is_left_out_of_the_buy_window_but_counted_in_its_size() {
    let (mut s, npc) = Scene::with_trader("soldout", vec![inv(1001, ADENA, 100_000)]);
    s.buy(100, &[(LIMITED, 5)]);
    s.log.take();
    s.open_window(npc, 100);
    let out = s.log.take();
    let list = &buy_lists(&out)[0];
    assert_eq!(list.size, 9);
    assert_eq!(list.products.len(), 8);
    assert!(list.products.iter().all(|p| p.item_id != LIMITED));
}

/// `BuyListData.load`: a row with less than the full stock sets the count and calls
/// `restartRestockTask`.
#[test]
fn stock_rows_of_the_last_run_come_back_with_javas_quirk() {
    let mut s = Scene::new("stock_rows");
    s.log.take();
    s.world.load_buy_list_stock(vec![
        // Unknown list and unknown product: skipped with a warning.
        ProductRow {
            list_id: 555,
            item_id: LIMITED,
            count: 1,
            next_restock_time: 0,
        },
        ProductRow {
            list_id: 100,
            item_id: 1234,
            count: 1,
            next_restock_time: 0,
        },
        // Full stock or more: ignored.
        ProductRow {
            list_id: 100,
            item_id: LIMITED,
            count: 5,
            next_restock_time: T0 + 5000,
        },
    ]);
    assert!(s.log.take().is_empty());
    assert_eq!(s.world.product_count(100, LIMITED), 5);

    // Stock 1, due in 5 s: the count is set, nothing is written.
    s.world.load_buy_list_stock(vec![ProductRow {
        list_id: 100,
        item_id: LIMITED,
        count: 1,
        next_restock_time: T0 + 5000,
    }]);
    assert_eq!(s.world.product_count(100, LIMITED), 1);
    assert!(product_rows(&s.log.take()).is_empty());
    // NOTE(java-quirk): `update(product, remainTime)` stores 5000, not a time, so the first scan
    // (1 s later) finds it long overdue and restocks it.
    s.advance(20);
    s.advance(2);
    assert_eq!(
        product_rows(&s.log.take()),
        [ProductRow {
            list_id: 100,
            item_id: LIMITED,
            count: 5,
            next_restock_time: 0,
        }]
    );
    assert_eq!(s.world.product_count(100, LIMITED), 5);

    // A restock time that already passed restocks at once.
    s.world.load_buy_list_stock(vec![ProductRow {
        list_id: 100,
        item_id: LIMITED,
        count: 2,
        next_restock_time: T0 - 1,
    }]);
    assert_eq!(
        product_rows(&s.log.take()),
        [ProductRow {
            list_id: 100,
            item_id: LIMITED,
            count: 5,
            next_restock_time: 0,
        }]
    );
}

// ---- selling and refunding -------------------------------------------------------------------

fn sell_scene(tag: &str) -> Scene {
    let (s, _npc) = Scene::with_trader(
        tag,
        vec![
            inv(1001, ADENA, 1000),
            inv(10, SWORD, 1),
            inv(11, SHOT, 100),
            inv(12, BOUND, 1),
        ],
    );
    s
}

/// `RequestSellItem`: the price is the reference price / 2; with `AllowRefund` the items go to the
/// refund list (a whole item keeps its object id, part of a stack becomes a new item), the adena
/// is added, and `ExBuySellList(player, true)` lists the refund items with their index.
#[test]
fn selling_moves_items_to_the_refund_list_and_pays_half_the_reference_price() {
    let mut s = sell_scene("sell");
    // The sword, 40 shots, an item that cannot be sold, and an object the player does not have.
    s.sell(
        100,
        &[(10, SWORD, 1), (11, SHOT, 40), (12, BOUND, 1), (999, SHOT, 1)],
    );
    let out = s.log.take();

    // 500 + 40 x 10
    assert_eq!(s.adena(), 1000 + 900);
    let items = s.items();
    assert!(items.iter().any(|i| i.0 == 11 && i.2 == 60));
    assert!(items.iter().all(|i| i.0 != 10));
    assert!(items.iter().any(|i| i.0 == 12));

    let refund = s.refund_list();
    assert_eq!(refund.len(), 2);
    assert_eq!(refund[0], (10, SWORD, 1));
    assert_eq!((refund[1].1, refund[1].2), (SHOT, 40));
    assert_ne!(refund[1].0, 11);
    assert_eq!(s.world.player(A).unwrap().shop.refund[0].loc, "REFUND");

    ends_with(
        &names(&out),
        &["STATUS_UPDATE", "EX_BUY_SELL_LIST", "SYSTEM_MESSAGE"],
    );
    assert_eq!(system_ids(&out), [SystemMessageId::TheTransactionIsComplete]);
    let window = &sell_lists(&out)[0];
    assert!(window.done);
    // The sell tab: the stack left (60 shots at 10), the bound item (no adena).
    let sold: Vec<(i32, i64)> = window
        .sell_list
        .iter()
        .map(|e| (e.item.item_id, e.price))
        .collect();
    assert_eq!(sold, [(SHOT, 10), (BOUND, 20)]);
    // The refund tab: index in the list, `(price / 2) * count`.
    let refunds: Vec<(i32, i32, i64)> = window
        .refund_list
        .iter()
        .map(|e| (e.index, e.item.item_id, e.price))
        .collect();
    assert_eq!(refunds, [(0, SWORD, 500), (1, SHOT, 400)]);

    // Rows: the sword's row leaves the inventory, the stack is updated.
    let rows = item_changes(&out);
    assert!(rows.contains(&(vec![], vec![10])));
    assert!(rows.contains(&(vec![(11, "INVENTORY".to_owned(), 60)], vec![])));
}

#[test]
fn selling_to_a_list_the_merchant_does_not_have_is_refused() {
    let mut s = sell_scene("sell_refused");
    s.sell(101, &[(11, SHOT, 1)]);
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
    s.sell(999, &[(11, SHOT, 1)]);
    assert_eq!(texts(&s.log.take()), [KICK]);
    assert_eq!(s.adena(), 1000);
    assert!(s.refund_list().is_empty());
}

/// `totalPrice > MAX_ADENA`: the message, and `return` after the items before it were moved; the
/// adena is never added.
#[test]
fn selling_past_the_adena_limit_keeps_the_moved_items_and_pays_nothing() {
    let mut settings = WorldSettings::default();
    settings.ground.max_adena = 600;
    let mut s = Scene::with("sell_limit", settings, None);
    s.start(vec![
        inv(1001, ADENA, 1000),
        inv(10, SWORD, 1),
        inv(11, SHOT, 100),
    ]);
    s.sell(100, &[(10, SWORD, 1), (11, SHOT, 40)]);
    let out = s.log.take();
    assert_eq!(
        system_ids(&out),
        [SystemMessageId::YouHaveExceededYourOutOfPocketAdenaLimit]
    );
    assert_eq!(s.adena(), 1000);
    assert_eq!(s.refund_list(), [(10, SWORD, 1)]);
    assert!(sell_lists(&out).is_empty());
}

#[test]
fn selling_without_refund_destroys_and_zero_sell_price_pays_nothing() {
    let mut settings = WorldSettings::default();
    settings.shop.allow_refund = false;
    let mut s = Scene::with("sell_destroy", settings, None);
    s.start(vec![
        inv(1001, ADENA, 1000),
        inv(10, SWORD, 1),
        inv(11, SHOT, 100),
    ]);
    s.sell(100, &[(10, SWORD, 1), (11, SHOT, 100)]);
    let out = s.log.take();
    assert_eq!(s.adena(), 1000 + 500 + 1000);
    assert!(s.refund_list().is_empty());
    assert_eq!(s.items(), [(1001, ADENA, 2500, "INVENTORY".to_owned())]);
    // Both rows are gone from the database.
    let rows = item_changes(&out);
    assert!(rows.contains(&(vec![], vec![10])));
    assert!(rows.contains(&(vec![], vec![11])));
    assert!(sell_lists(&out)[0].refund_list.is_empty());

    let mut settings = WorldSettings::default();
    settings.shop.merchant_zero_sell_price = true;
    let mut s = Scene::with("sell_zero", settings, None);
    s.start(vec![inv(1001, ADENA, 1000), inv(10, SWORD, 1)]);
    s.sell(100, &[(10, SWORD, 1)]);
    let out = s.log.take();
    assert_eq!(s.adena(), 1000);
    // The prices of the window are 0 too.
    assert_eq!(sell_lists(&out)[0].refund_list[0].price, 0);
}

/// `PlayerRefund.addItem`: with more than 12 items the first one is destroyed (its id is given
/// back).
#[test]
fn the_refund_list_keeps_twelve_items() {
    let mut s = Scene::new("sell_twelve");
    let mut items = vec![inv(1001, ADENA, 1000)];
    items.extend((0..14).map(|n| inv(100 + n, HAT, 1)));
    s.start(items);
    for n in 0..14 {
        s.sell(100, &[(100 + n, HAT, 1)]);
    }
    let refund = s.refund_list();
    assert_eq!(refund.len(), 12);
    // The oldest two went.
    assert_eq!(refund.first().map(|r| r.0), Some(102));
    assert_eq!(refund.last().map(|r| r.0), Some(113));
    assert_eq!(s.adena(), 1000 + 14 * 200);
}

#[test]
fn refunding_buys_the_items_back_for_the_price_they_were_sold_at() {
    let mut s = sell_scene("refund");
    s.sell(100, &[(10, SWORD, 1), (11, SHOT, 40)]);
    let refund_id = s.refund_list()[1].0;
    let used = s.ids.used_count();
    s.log.take();

    // Index 1 then 0: the shots join the stack, the sword is back under its old object id.
    s.refund(100, &[1, 0]);
    let out = s.log.take();
    // 400 + 500 adena back to the merchant.
    assert_eq!(s.adena(), 1000 + 900 - 900);
    let items = s.items();
    assert!(items.contains(&(10, SWORD, 1, "INVENTORY".into())));
    assert!(items.contains(&(11, SHOT, 100, "INVENTORY".into())));
    assert!(s.refund_list().is_empty());
    // The refund item's object id went back to the factory.
    assert!(!s.ids.is_allocated(ObjectId(refund_id)));
    assert_eq!(s.ids.used_count(), used - 1);

    // `StatusUpdate`, `ExBuySellList(player, true)`; no THE_TRANSACTION_IS_COMPLETE.
    ends_with(&names(&out), &["STATUS_UPDATE", "EX_BUY_SELL_LIST"]);
    assert!(system_ids(&out).is_empty());
    assert!(sell_lists(&out)[0].refund_list.is_empty());
    // Rows: the sword is written back as an inventory item, the stack gets its count.
    let rows = item_changes(&out);
    assert!(rows.contains(&(vec![(10, "INVENTORY".to_owned(), 1)], vec![])));
    assert!(rows.contains(&(vec![(11, "INVENTORY".to_owned(), 100)], vec![])));

    // Nothing left to refund: `!player.hasRefund()`.
    s.refund(100, &[0]);
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
}

#[test]
fn refund_requests_are_checked_like_java() {
    let mut s = sell_scene("refund_checks");
    s.sell(100, &[(10, SWORD, 1), (11, SHOT, 40)]);
    s.log.take();

    // An index outside the list, a negative one, a repeated one.
    for indexes in [vec![2], vec![-1], vec![0, 0]] {
        s.refund(100, &indexes);
        assert_eq!(texts(&s.log.take()), [KICK], "{indexes:?}");
    }
    // The list the merchant does not have, and a merchant not allowed for it.
    s.refund(101, &[0]);
    assert_eq!(names(&s.log.take()), ["ACTION_FAIL"]);
    s.refund(999, &[0]);
    assert_eq!(texts(&s.log.take()), [KICK]);
    assert_eq!(s.refund_list().len(), 2);

    // Not enough adena for the 900 it costs.
    let player = s.world.player_mut(A).unwrap();
    player
        .data
        .items
        .iter_mut()
        .find(|i| i.item_id == ADENA)
        .unwrap()
        .count = 899;
    s.refund(100, &[0, 1]);
    let out = s.log.take();
    assert_eq!(system_ids(&out), [SystemMessageId::YouDoNotHaveEnoughAdena]);
    ends_with(&names(&out), &["SYSTEM_MESSAGE", "ACTION_FAIL"]);
    assert_eq!(s.refund_list().len(), 2);
}

/// `Player.deleteMe` -> `clearRefund`: what the player sold is gone for good and the ids come back.
#[test]
fn leaving_destroys_the_refund_list() {
    let mut s = sell_scene("refund_leave");
    s.sell(100, &[(11, SHOT, 40)]);
    let new_id = s.refund_list()[0].0;
    assert!(s.ids.is_allocated(ObjectId(new_id)));
    s.input(A, ClientInput::Logout);
    assert!(!s.ids.is_allocated(ObjectId(new_id)));
    // The 40 shots are not in the stored snapshot either.
    let log = s.log.take();
    let stored = log
        .iter()
        .find_map(|o| match o {
            Out::Persist(PersistCommand::StorePlayer(data)) => Some(data.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        stored.items.iter().find(|i| i.item_id == SHOT).map(|i| i.count),
        Some(60)
    );
    assert!(stored.items.iter().all(|i| i.loc != "REFUND"));
}

// ---- the sell list ----------------------------------------------------------------------------

#[test]
fn the_sell_tab_lists_what_getavailableitems_lists() {
    let mut s = Scene::new("sell_tab");
    let mut worn = inv(13, CUIRASS, 1);
    worn.loc = "PAPERDOLL".into();
    worn.loc_data = 10;
    let npc = s.start(vec![
        inv(1001, ADENA, 1000),
        inv(10, SWORD, 1),
        inv(11, SHOT, 100),
        worn,
    ]);
    s.open_window(npc, 100);
    // Not equipped, no adena: the sword and the shots.
    let window = &sell_lists(&s.log.take())[0];
    let ids: Vec<i32> = window.sell_list.iter().map(|e| e.item.item_id).collect();
    assert_eq!(ids, [SWORD, SHOT]);
}

// ---- the price config of a town -----------------------------------------------------------------

/// `MerchantPriceConfigTable.getMerchantPriceConfig(npc)`: a config whose town zone touches the
/// NPC's zone region wins over the default; the lowest config id first.
#[test]
fn the_town_zone_picks_the_base_tax() {
    let zone = format!(
        "<zone id=\"11025\" name=\"gludin\" type=\"TownZone\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
         <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone>",
        X - 500,
        Y - 500,
        X + 500,
        Y + 500
    );
    let mut s = Scene::with("zone", WorldSettings::default(), Some(&zone));
    let npc = s.start(vec![inv(1001, ADENA, 5000)]);
    // Gludin (config 5, zone 11025): 30%.
    s.open_window(npc, 100);
    let out = s.log.take();
    let list = &buy_lists(&out)[0];
    let price = |item_id: i32| list.products.iter().find(|p| p.item_id == item_id).unwrap().price;
    // `(long) (100 * (1 + 0.3))`
    assert_eq!(price(LIMITED), 130);
    assert_eq!(price(SHOT), 26);
    // Buying charges the same: 2 x 26.
    s.buy(100, &[(SHOT, 2)]);
    assert_eq!(s.adena(), 5000 - 52);
}
