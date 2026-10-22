#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Warehouses and freight (M6-WH-01): the bypasses of the warehouse NPCs, `SendWareHouseDepositList`,
//! `SendWareHouseWithDrawList`, `RequestPackageSend`, `RequestPackageSendableItemList`, the fees,
//! the sorted lists and the item rows. The expected values come from the Java code named in each
//! test.
//! Java: dist/game/data/scripts/handlers/bypass/npc/{PrivateWarehouse,ClanWarehouse,Freight}.java,
//! network/clientpackets/{SendWareHouseDepositList,SendWareHouseWithDrawList,RequestPackageSend,
//! RequestPackageSendableItemList}.java, entity/itemcontainer/{ItemContainer,PlayerWarehouse,
//! ClanWarehouse,PlayerFreight}.java, network/serverpackets/SortedWareHouseWithdrawalList.java.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillLoadOptions, SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::client::WarehouseListEntry;
use l2_protocol_game::server::{
    InventoryUpdate, PackageSendableList, PackageToList, SortedWareHouseWithdrawalList, SystemMessage,
    WareHouseDepositList, WareHouseWithdrawalList,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::clan::{ClanAccess, ClanDataRecord, ClanMemberRecord};
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
const CUIRASS: i32 = 4;
const SEALED: i32 = 5;
const SHADOW: i32 = 6;
const BOW: i32 = 7;
const BOLT: i32 = 8;
const AMULET: i32 = 9;
const ORE: i32 = 10;
const DAGGER: i32 = 11;
const HELMET: i32 = 12;
const COBWEB: i32 = 13;
const ADENA: i32 = 57;
const ANVIL: i32 = 20;
const QUEST: i32 = 21;
const PILE: i32 = 23;
const GHOST: i32 = 99_999;

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="C Sword"><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /><set name="is_freightable" val="true" /></item>
<item id="3" type="EtcItem" name="Bound"><set name="is_tradable" val="false" /><set name="weight" val="1" /></item>
<item id="4" type="Armor" name="B Cuirass"><set name="crystal_type" val="B" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="weight" val="10" /></item>
<item id="5" type="EtcItem" name="Sealed"><set name="is_depositable" val="false" /><set name="weight" val="1" /></item>
<item id="6" type="Weapon" name="Shadow Blade"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="duration" val="100" /></item>
<item id="7" type="Weapon" name="D Bow"><set name="crystal_type" val="D" /><set name="weapon_type" val="BOW" /><set name="bodypart" val="lrhand" /><set name="weight" val="30" /></item>
<item id="8" type="EtcItem" name="Bolt"><set name="etcitem_type" val="ARROW" /><set name="is_stackable" val="true" /></item>
<item id="9" type="EtcItem" name="Amulet of Luck"><set name="weight" val="1" /></item>
<item id="10" type="EtcItem" name="Ore"><set name="etcitem_type" val="MATERIAL" /><set name="material" val="SILVER" /><set name="is_stackable" val="true" /></item>
<item id="11" type="Weapon" name="A Dagger"><set name="crystal_type" val="A" /><set name="weapon_type" val="DAGGER" /><set name="bodypart" val="rhand" /></item>
<item id="13" type="EtcItem" name="Aged Cobweb"><set name="etcitem_type" val="MATERIAL" /><set name="material" val="COBWEB" /><set name="is_stackable" val="true" /></item>
<item id="12" type="Armor" name="Aegis Helmet"><set name="crystal_type" val="D" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="head" /></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="20" type="EtcItem" name="Anvil"><set name="weight" val="1000" /><set name="is_stackable" val="true" /></item>
<item id="21" type="EtcItem" name="Quest thing"><set name="is_questitem" val="true" /><set name="is_stackable" val="true" /></item>
<item id="23" type="EtcItem" name="Pile"><set name="is_stackable" val="true" /><set name="weight" val="2" /><set name="is_freightable" val="true" /></item>
</list>"#;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="30001" type="Warehouse" level="70" name="Trionell" usingServerSideName="true">
    <status attackable="false" />
  </npc>
  <npc id="30002" type="Folk" level="70" name="Villager" usingServerSideName="true">
    <status attackable="false" />
  </npc>
</list>"#;

/// The page of the warehouse keeper: every command the tests send, as the html lists them.
const PAGE: &str = "<html><body>\
    <a action=\"bypass -h npc_%objectId%_WithdrawP\">w</a>\
    <a action=\"bypass -h npc_%objectId%_DepositP\">d</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawC\">wc</a>\
    <a action=\"bypass -h npc_%objectId%_DepositC\">dc</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawSortedP\">s</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawSortedP ALL\">s</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawSortedP WEAPON A2Z\">s</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawSortedP WEAPON Z2A\">s</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawSortedP ARMOR GRADE\">s</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawSortedP ARMOR WEAR\">s</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawSortedP MATERIAL TYPE\">s</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawSortedP AMULETT A2Z\">s</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawSortedP BOGUS A2Z\">s</a>\
    <a action=\"bypass -h npc_%objectId%_WithdrawSortedC ALL Z2A\">sc</a>\
    <a action=\"bypass -h npc_%objectId%_package_withdraw\">pw</a>\
    <a action=\"bypass -h npc_%objectId%_package_deposit\">pd</a>\
    </body></html>";

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
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    };
    Data { items, stats }
}

// ---- helpers ------------------------------------------------------------------------------------

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const A: ObjectId = ObjectId(0x6000_0001);
const B: ObjectId = ObjectId(0x6000_0002);
const C: ObjectId = ObjectId(0x6000_0003);
const CLAN: i32 = 500;
/// Where the Rust server's new object ids start (D-39).
const RUST_IDS: i32 = 0x6000_0000;

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

fn wh(object_id: i32, item_id: i32, count: i64) -> ItemData {
    item(object_id, item_id, count, "WAREHOUSE")
}

fn player(id: ObjectId, name: &str, items: Vec<ItemData>) -> PlayerData {
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
        level: 40,
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

/// A character with these items in its private warehouse and freight.
fn keeper_client(id: ObjectId, inventory: Vec<ItemData>, private: Vec<ItemData>) -> PlayerData {
    let mut data = player(id, "Alice", inventory);
    data.profile.warehouse = WarehouseLoad {
        private,
        ..WarehouseLoad::default()
    };
    data
}

struct Scene {
    world: World,
    log: Log,
    npc: ObjectId,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Self {
        Self::with(WorldSettings::default())
    }

    fn with(settings: WorldSettings) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        write("data/html/warehouse/30001.htm", PAGE);
        write("data/html/default/30002.htm", "<html><body>hello</body></html>");
        write(
            "data/html/mods/WhSortedC.htm",
            "<html><body><a action=\"bypass -h npc_%objectId%_WithdrawSortedC ALL\">sorted</a></body></html>",
        );
        write(
            "data/html/mods/WhSortedP.htm",
            "<html><body><a action=\"bypass -h npc_%objectId%_WithdrawSortedP ALL\">sorted</a></body></html>",
        );
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(RecOutbox(log.clone())));
        world.set_static_data(Box::new(data()));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_settings(settings);
        let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        let npc = world.spawn_npc_at(30001, X + 50, Y, Z, 0).unwrap();
        Self {
            world,
            log,
            npc,
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

    fn input(&mut self, who: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input,
        });
    }

    /// The player clicks the warehouse keeper twice (select, then talk): its page is cached, so the
    /// bypasses of the page are accepted, and the keeper is `getLastFolkNPC()`.
    fn talk(&mut self, who: ObjectId) {
        for _ in 0..2 {
            self.input(
                who,
                ClientInput::Action {
                    object_id: self.npc.0,
                    action_id: 0,
                },
            );
        }
        self.log.take();
    }

    /// `npc_<keeper>_<command>`. A clan warehouse window asks the database for the rows of the
    /// clan first (M6-SYNC-01); here the database has what the world has, so the request is
    /// answered at once. The tests of the refresh use [`bypass_unanswered`](Self::bypass_unanswered).
    fn bypass(&mut self, who: ObjectId, command: &str) {
        self.bypass_unanswered(who, command);
        for (player, clan_id) in self.take_clan_requests() {
            let items = self
                .world
                .clan(clan_id)
                .map(|c| c.warehouse.clone())
                .unwrap_or_default();
            self.world.handle(WorldCommand::ClanWarehouseLoaded {
                player,
                clan_id,
                result: Ok(items),
            });
        }
    }

    fn bypass_unanswered(&mut self, who: ObjectId, command: &str) {
        self.input(
            who,
            ClientInput::Bypass {
                command: format!("npc_{}_{command}", self.npc.0),
            },
        );
    }

    /// The `ClanWarehouseRequest`s in the log, taken out of it.
    fn take_clan_requests(&self) -> Vec<(ObjectId, i32)> {
        let mut found = Vec::new();
        self.log.0.lock().unwrap().retain(|o| match o {
            Out::Persist(PersistCommand::ClanWarehouseRequest { player, clan_id }) => {
                found.push((*player, *clan_id));
                false
            }
            _ => true,
        });
        found
    }

    fn deposit(&mut self, who: ObjectId, items: &[(i32, i64)]) {
        self.input(
            who,
            ClientInput::Warehouse(WarehouseInput::Deposit {
                items: Some(entries(items)),
            }),
        );
    }

    fn withdraw(&mut self, who: ObjectId, items: &[(i32, i64)]) {
        self.input(
            who,
            ClientInput::Warehouse(WarehouseInput::Withdraw {
                items: Some(entries(items)),
            }),
        );
    }

    fn send_package(&mut self, who: ObjectId, receiver: ObjectId, items: &[(i32, i64)]) {
        self.input(
            who,
            ClientInput::Warehouse(WarehouseInput::PackageSend {
                object_id: receiver.0,
                items: Some(entries(items)),
            }),
        );
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn inventory_of(&self, who: ObjectId) -> Vec<(i32, i32, i64)> {
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

    fn private_of(&self, who: ObjectId) -> Vec<ItemData> {
        self.world.player(who).unwrap().warehouse.private.clone()
    }

    fn freight_of(&self, who: ObjectId, owner: ObjectId) -> Vec<ItemData> {
        self.world
            .player(who)
            .unwrap()
            .warehouse
            .freight
            .get(&owner)
            .cloned()
            .unwrap_or_default()
    }

    fn adena(&self, who: ObjectId) -> i64 {
        self.world.adena_of(who)
    }
}

fn entries(items: &[(i32, i64)]) -> Vec<WarehouseListEntry> {
    items
        .iter()
        .map(|(object_id, count)| WarehouseListEntry {
            object_id: *object_id,
            count: *count,
        })
        .collect()
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

fn message_ids(out: &[Out], to: ObjectId) -> Vec<SystemMessageId> {
    of(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw).id)
        .collect()
}

/// The `ItemRows` commands of a log, as `(tracked, ops)`.
fn rows(out: &[Out]) -> Vec<(Option<ObjectId>, Vec<ItemRowOp>)> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemRows { tracked, ops }) => Some((*tracked, ops.clone())),
            _ => None,
        })
        .collect()
}

/// One op as `(statement, owner, object id, item id, count, loc)`.
type Row = (&'static str, i32, i32, i32, i64, String);

fn flat(ops: &[ItemRowOp]) -> Vec<Row> {
    ops.iter()
        .map(|op| match op {
            ItemRowOp::Write { owner_id, item } => (
                "write",
                *owner_id,
                item.object_id.0,
                item.item_id,
                item.count,
                item.loc.clone(),
            ),
            ItemRowOp::Update { owner_id, item } => (
                "update",
                *owner_id,
                item.object_id.0,
                item.item_id,
                item.count,
                item.loc.clone(),
            ),
            ItemRowOp::Insert { owner_id, item } => (
                "insert",
                *owner_id,
                item.object_id.0,
                item.item_id,
                item.count,
                item.loc.clone(),
            ),
            ItemRowOp::Delete { object_id } => ("delete", 0, object_id.0, 0, 0, String::new()),
        })
        .collect()
}

/// The entries of the `InventoryUpdate` the client got, as `(change, object id, count)`.
fn inventory_update(out: &[Out], to: ObjectId) -> Vec<(i16, i32, i64)> {
    of(out, to, "INVENTORY_UPDATE")
        .iter()
        .flat_map(|raw| decode::<InventoryUpdate>(raw).updates)
        .map(|u| (u.change, u.item.object_id, u.item.count))
        .collect()
}

// ---- the private warehouse: deposit -------------------------------------------------------------

#[test]
fn deposit_p_shows_what_can_go_in_and_remembers_the_warehouse() {
    let mut s = Scene::new();
    // The sword and adena can go in; the sealed item (`is_depositable` false), the worn cuirass and
    // the quest item cannot (`Item.isDepositable`, `isAvailable`, `getAvailableItems`).
    let mut shadow = inv(108, SHADOW, 1);
    shadow.mana_left = 100;
    let mut worn = inv(104, CUIRASS, 1);
    worn.loc = "PAPERDOLL".into();
    worn.loc_data = 10;
    s.enter(keeper_client(
        A,
        vec![
            inv(100, SWORD, 1),
            inv(101, ADENA, 1000),
            inv(102, SEALED, 1),
            worn,
            inv(105, QUEST, 3),
            inv(106, BOUND, 1),
            shadow,
        ],
        vec![],
    ));
    s.talk(A);
    s.bypass(A, "DepositP");
    let out = s.log.take();
    // `player.sendPacket(ActionFailed)` first, then the window; `RequestBypassToServer` sends
    // another ActionFailed after every `npc_` command.
    assert_eq!(
        names(&out, A),
        ["ACTION_FAIL", "WAREHOUSE_DEPOSIT_LIST", "ACTION_FAIL"]
    );
    let list: WareHouseDepositList = decode(of(&out, A, "WAREHOUSE_DEPOSIT_LIST")[0]);
    assert_eq!(list.wh_type, 1);
    assert_eq!(list.player_adena, 1000);
    let mut shown: Vec<i32> = list.items.iter().map(|i| i.object_id).collect();
    shown.sort_unstable();
    // The bound item and the shadow item are fine for a private warehouse (`isPrivateWareHouse`
    // skips `isTradeable` and `isShadowItem`).
    assert_eq!(shown, [100, 101, 106, 108]);
    assert_eq!(
        s.world.player(A).unwrap().warehouse.active,
        Some(ContainerRef::Private(A))
    );
}

#[test]
fn a_whole_item_moves_to_the_warehouse_and_the_fee_is_thirty_per_item() {
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(100, SWORD, 1), inv(101, ADENA, 1000)],
        vec![],
    ));
    s.talk(A);
    s.bypass(A, "DepositP");
    s.log.take();
    s.deposit(A, &[(100, 1)]);
    s.advance(150);
    let out = s.log.take();

    // `fee = _items.size() * 30`, taken with `reduceAdena`.
    assert_eq!(s.adena(A), 970);
    assert_eq!(s.inventory_of(A), [(101, ADENA, 970)]);
    let stored = s.private_of(A);
    assert_eq!(stored.len(), 1);
    assert_eq!(
        (stored[0].object_id.0, stored[0].loc.as_str(), stored[0].loc_data),
        (100, "WAREHOUSE", 0)
    );

    // `transferItem`: the item changes `loc`, `updateDatabase(true)`.
    let commands = rows(&out);
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, Some(A));
    assert_eq!(
        flat(&commands[0].1),
        [("write", A.0, 100, SWORD, 1, "WAREHOUSE".to_string())]
    );
    // `playerIU.addRemovedItem(oldItem)` for the moved item, and the adena the fee changed.
    let update = inventory_update(&out, A);
    assert!(update.contains(&(InventoryUpdate::REMOVED, 100, 1)), "{update:?}");
    assert!(
        update.contains(&(InventoryUpdate::MODIFIED, 101, 970)),
        "{update:?}"
    );
    // `StatusUpdate` with `CUR_LOAD`.
    assert!(names(&out, A).contains(&"STATUS_UPDATE".to_string()));
}

#[test]
fn a_stack_moves_in_parts_and_joins_the_stack_in_the_warehouse() {
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(100, PILE, 500), inv(101, ADENA, 1000)],
        vec![],
    ));
    s.talk(A);
    s.bypass(A, "DepositP");
    s.log.take();

    // 100 of 500: the source keeps 400, a new row (a new id of the Rust range) holds 100.
    s.deposit(A, &[(100, 100)]);
    let out = s.log.take();
    let stored = s.private_of(A);
    assert_eq!(stored.len(), 1);
    let new_id = stored[0].object_id.0;
    assert!(new_id >= RUST_IDS, "{new_id:#x} is not in the Rust range");
    assert_eq!((stored[0].count, stored[0].loc.as_str()), (100, "WAREHOUSE"));
    assert_eq!(
        flat(&rows(&out)[0].1),
        [
            ("write", A.0, 100, PILE, 400, "INVENTORY".to_string()),
            ("insert", A.0, new_id, PILE, 100, "WAREHOUSE".to_string()),
        ]
    );

    // 50 more: the stack in the warehouse grows (`targetitem.changeCount`), one `UPDATE`.
    s.deposit(A, &[(100, 50)]);
    let out = s.log.take();
    assert_eq!(s.private_of(A)[0].count, 150);
    assert_eq!(
        flat(&rows(&out)[0].1),
        [
            ("write", A.0, 100, PILE, 350, "INVENTORY".to_string()),
            ("update", A.0, new_id, PILE, 150, "WAREHOUSE".to_string()),
        ]
    );
    assert_eq!(s.adena(A), 940);

    // The rest: the source stack is destroyed (its row goes), the target grows again.
    s.deposit(A, &[(100, 350)]);
    let out = s.log.take();
    assert_eq!(s.private_of(A)[0].count, 500);
    assert_eq!(s.inventory_of(A), [(101, ADENA, 910)]);
    let ops = flat(&rows(&out)[0].1);
    assert_eq!(ops, [("update", A.0, new_id, PILE, 500, "WAREHOUSE".to_string())]);
}

#[test]
fn deposit_refusals_follow_java() {
    // Not enough adena for the fee: YOU_DO_NOT_HAVE_ENOUGH_ADENA, nothing moves.
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(100, SWORD, 1), inv(101, ADENA, 20)],
        vec![],
    ));
    s.talk(A);
    s.bypass(A, "DepositP");
    s.log.take();
    s.deposit(A, &[(100, 1)]);
    let out = s.log.take();
    assert_eq!(message_ids(&out, A), [SystemMessageId::YouDoNotHaveEnoughAdena]);
    assert!(rows(&out).is_empty());
    assert_eq!(s.adena(A), 20);

    // Depositing the adena itself counts against the fee: 100 adena, deposit 80 + fee 30.
    let mut s = Scene::new();
    s.enter(keeper_client(A, vec![inv(101, ADENA, 100)], vec![]));
    s.talk(A);
    s.bypass(A, "DepositP");
    s.log.take();
    s.deposit(A, &[(101, 80)]);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouDoNotHaveEnoughAdena]
    );
    assert_eq!(s.adena(A), 100);

    // The warehouse is full: a non-stackable item needs a slot, a stack that is already there
    // does not.
    let settings = WorldSettings {
        warehouse_slots_no_dwarf: 1,
        ..WorldSettings::default()
    };
    let mut s = Scene::with(settings);
    s.enter(keeper_client(
        A,
        vec![inv(100, SWORD, 1), inv(101, ADENA, 1000), inv(102, CUIRASS, 1)],
        vec![wh(300, PILE, 5)],
    ));
    s.talk(A);
    s.bypass(A, "DepositP");
    s.log.take();
    s.deposit(A, &[(100, 1)]);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted]
    );
    assert_eq!(s.adena(A), 1000);

    // An item that cannot be deposited is skipped without a word, but the fee is paid.
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(100, SEALED, 1), inv(101, ADENA, 1000), inv(102, SWORD, 1)],
        vec![],
    ));
    s.talk(A);
    s.bypass(A, "DepositP");
    s.log.take();
    s.deposit(A, &[(100, 1), (102, 1)]);
    let out = s.log.take();
    assert_eq!(s.adena(A), 940);
    assert_eq!(s.private_of(A).len(), 1);
    assert_eq!(s.private_of(A)[0].object_id.0, 102);
    assert!(s.inventory_of(A).contains(&(100, SEALED, 1)));
    assert_eq!(rows(&out).len(), 1);
}

#[test]
fn the_keeper_must_be_in_reach_unless_the_player_is_a_gm() {
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(100, SWORD, 1), inv(101, ADENA, 1000)],
        vec![],
    ));
    s.talk(A);
    s.bypass(A, "DepositP");
    s.log.take();
    // 250 away or more: `canInteract` fails.
    s.world.set_position(A, X - 300, Y, Z);
    s.log.take();
    s.deposit(A, &[(100, 1)]);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::YouFailedAtSendingThePackageBecauseYouAreTooFarFromTheWarehouse]
    );
    assert!(s.private_of(A).is_empty());

    // A GM does not need the keeper.
    let mut gm = keeper_client(B, vec![inv(100, SWORD, 1), inv(101, ADENA, 1000)], vec![]);
    gm.access_level = 200;
    s.enter(gm);
    s.talk(B);
    s.bypass(B, "DepositP");
    s.world.set_position(B, X - 300, Y, Z);
    s.log.take();
    s.deposit(B, &[(100, 1)]);
    assert_eq!(s.private_of(B).len(), 1);
}

#[test]
fn packets_without_an_open_warehouse_or_with_bad_lists_do_nothing() {
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(100, SWORD, 1), inv(101, ADENA, 1000)],
        vec![wh(300, PILE, 5)],
    ));
    s.talk(A);
    // No `_activeWarehouse` yet.
    s.deposit(A, &[(100, 1)]);
    s.withdraw(A, &[(300, 1)]);
    assert!(rows(&s.log.take()).is_empty());
    assert_eq!(s.private_of(A).len(), 1);

    s.bypass(A, "DepositP");
    s.log.take();
    // A list Java drops.
    s.input(A, ClientInput::Warehouse(WarehouseInput::Deposit { items: None }));
    // More entries than `MAX_ITEM_IN_PACKET` (250 here).
    let many: Vec<(i32, i64)> = (0..251).map(|i| (100 + i, 1)).collect();
    s.deposit(A, &many);
    assert!(s.log.take().is_empty());
    assert_eq!(s.adena(A), 1000);

    // The sword that is not there: `checkItemManipulation` fails and the packet ends.
    s.deposit(A, &[(777, 1)]);
    assert!(rows(&s.log.take()).is_empty());
    assert_eq!(s.adena(A), 1000);
}

#[test]
fn karma_and_a_trade_list_stop_the_deposit_where_java_does() {
    // `!ALT_GAME_KARMA_PLAYER_CAN_USE_WAREHOUSE && getKarma() > 0`: no word.
    let mut settings = WorldSettings::default();
    settings.talk.karma_player_can_use_warehouse = false;
    let mut s = Scene::with(settings);
    let mut chaotic = keeper_client(A, vec![inv(100, SWORD, 1), inv(101, ADENA, 1000)], vec![]);
    chaotic.karma = 100;
    s.enter(chaotic);
    s.talk(A);
    s.bypass(A, "DepositP");
    s.log.take();
    s.deposit(A, &[(100, 1)]);
    assert!(s.log.take().is_empty());
    assert_eq!(s.adena(A), 1000);
}

// ---- the private warehouse: withdraw ------------------------------------------------------------

#[test]
fn withdraw_p_lists_the_warehouse_and_tells_when_it_is_empty() {
    let mut s = Scene::new();
    s.enter(keeper_client(A, vec![inv(101, ADENA, 77)], vec![]));
    s.talk(A);
    s.bypass(A, "WithdrawP");
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::YouHaveNotDepositedAnyItemsInYourWarehouse]
    );
    assert_eq!(names(&out, A), ["ACTION_FAIL", "SYSTEM_MESSAGE", "ACTION_FAIL"]);

    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(101, ADENA, 77)],
        vec![wh(300, SWORD, 1), wh(301, PILE, 100)],
    ));
    s.talk(A);
    s.bypass(A, "WithdrawP");
    let out = s.log.take();
    assert_eq!(
        names(&out, A),
        ["ACTION_FAIL", "WAREHOUSE_WITHDRAW_LIST", "ACTION_FAIL"]
    );
    let list: WareHouseWithdrawalList = decode(of(&out, A, "WAREHOUSE_WITHDRAW_LIST")[0]);
    assert_eq!((list.wh_type, list.player_adena), (1, 77));
    let shown: Vec<(i32, i64)> = list.items.iter().map(|i| (i.object_id, i.count)).collect();
    assert_eq!(shown, [(300, 1), (301, 100)]);
}

#[test]
fn a_withdraw_moves_whole_items_and_parts_of_stacks() {
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(101, ADENA, 77)],
        vec![wh(300, SWORD, 1), wh(301, PILE, 100)],
    ));
    s.talk(A);
    s.bypass(A, "WithdrawP");
    s.log.take();
    s.withdraw(A, &[(300, 1), (301, 40)]);
    s.advance(150);
    let out = s.log.take();

    // The sword changes owner and place, the stack is split: 60 stay, a new row holds 40.
    let pile_row = s
        .world
        .player(A)
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.item_id == PILE)
        .unwrap()
        .clone();
    assert!(pile_row.object_id.0 >= RUST_IDS);
    assert_eq!((pile_row.count, pile_row.loc.as_str()), (40, "INVENTORY"));
    assert_eq!(s.private_of(A).len(), 1);
    assert_eq!(
        (s.private_of(A)[0].object_id.0, s.private_of(A)[0].count),
        (301, 60)
    );
    assert!(s.inventory_of(A).contains(&(300, SWORD, 1)));
    assert_eq!(
        flat(&rows(&out)[0].1),
        [
            ("update", A.0, 300, SWORD, 1, "INVENTORY".to_string()),
            ("update", A.0, 301, PILE, 60, "WAREHOUSE".to_string()),
            (
                "insert",
                A.0,
                pile_row.object_id.0,
                PILE,
                40,
                "INVENTORY".to_string()
            ),
        ]
    );
    // `newItem.getCount() > i.getCount()` is false for both: new items.
    let update = inventory_update(&out, A);
    assert!(update.contains(&(InventoryUpdate::ADDED, 300, 1)), "{update:?}");
    assert!(
        update.contains(&(InventoryUpdate::ADDED, pile_row.object_id.0, 40)),
        "{update:?}"
    );

    // 10 more join the stack in the inventory: the entry is a modification.
    s.withdraw(A, &[(301, 10)]);
    s.advance(150);
    let out = s.log.take();
    let update = inventory_update(&out, A);
    assert_eq!(update, [(InventoryUpdate::MODIFIED, pile_row.object_id.0, 50)]);
    assert_eq!(s.private_of(A)[0].count, 50);

    // The rest of the stack: the row in the warehouse is deleted, the inventory stack grows.
    s.withdraw(A, &[(301, 50)]);
    let out = s.log.take();
    assert!(s.private_of(A).is_empty());
    assert_eq!(
        flat(&rows(&out)[0].1),
        [
            ("delete", 0, 301, 0, 0, String::new()),
            (
                "write",
                A.0,
                pile_row.object_id.0,
                PILE,
                100,
                "INVENTORY".to_string()
            ),
        ]
    );
}

#[test]
fn a_withdraw_checks_slots_and_weight() {
    // The inventory is full (`validateCapacity(slots)`): 1 slot, 1 item allowed.
    let settings = WorldSettings {
        inventory_maximum_no_dwarf: 1,
        ..WorldSettings::default()
    };
    let mut s = Scene::with(settings);
    s.enter(keeper_client(
        A,
        vec![inv(101, ADENA, 77)],
        vec![wh(300, SWORD, 1)],
    ));
    s.talk(A);
    s.bypass(A, "WithdrawP");
    s.log.take();
    s.withdraw(A, &[(300, 1)]);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YourInventoryIsFull]
    );
    assert_eq!(s.private_of(A).len(), 1);

    // Too heavy (`validateWeight`): a hundred thousand anvils of weight 1000.
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(101, ADENA, 77)],
        vec![wh(300, ANVIL, 100_000)],
    ));
    s.talk(A);
    s.bypass(A, "WithdrawP");
    s.log.take();
    s.withdraw(A, &[(300, 100_000)]);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouHaveExceededTheWeightLimit]
    );
    assert_eq!(s.private_of(A)[0].count, 100_000);
}

#[test]
fn a_withdraw_of_something_that_is_not_there_is_an_illegal_action() {
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(101, ADENA, 77)],
        vec![wh(300, PILE, 5)],
    ));
    s.talk(A);
    s.bypass(A, "WithdrawP");
    s.log.take();
    // More than the stack holds, then an object that is not in the warehouse.
    s.withdraw(A, &[(300, 6)]);
    s.withdraw(A, &[(555, 1)]);
    let out = s.log.take();
    assert!(rows(&out).is_empty());
    assert_eq!(s.private_of(A)[0].count, 5);
    assert_eq!(s.inventory_of(A), [(101, ADENA, 77)]);
}

#[test]
fn a_withdraw_count_of_zero_moves_nothing_and_ends_the_packet() {
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(101, ADENA, 77)],
        vec![wh(300, SWORD, 1), wh(301, PILE, 5), wh(302, CUIRASS, 1)],
    ));
    s.talk(A);
    s.bypass(A, "WithdrawP");
    s.log.take();
    // The sword moves; the 0 of the stack finds nothing to return (`newItem == null`), so the
    // packet ends there: the cuirass stays, and no `InventoryUpdate` is sent.
    s.withdraw(A, &[(300, 1), (301, 0), (302, 1)]);
    s.advance(150);
    let out = s.log.take();
    assert!(s.inventory_of(A).contains(&(300, SWORD, 1)));
    assert_eq!(s.private_of(A).len(), 2);
    assert_eq!(rows(&out).len(), 1);
    assert!(inventory_update(&out, A).is_empty());
    assert!(!names(&out, A).contains(&"STATUS_UPDATE".to_string()));
}

#[test]
fn items_with_an_unknown_template_never_join_a_container() {
    // `Item.restoreFromDb` returns null for a template the datapack does not have.
    let mut s = Scene::new();
    let mut data = keeper_client(A, vec![], vec![wh(300, SWORD, 1), wh(301, GHOST, 1)]);
    data.profile.warehouse.freight = vec![(
        A,
        vec![item(600, GHOST, 1, "FREIGHT"), item(601, PILE, 2, "FREIGHT")],
    )];
    s.enter(data);
    assert_eq!(s.private_of(A).len(), 1);
    assert_eq!(s.freight_of(A, A).len(), 1);
    // The own freight exists even when the character has none.
    let mut s = Scene::new();
    s.enter(player(B, "Bob", vec![]));
    assert!(s.world.player(B).unwrap().warehouse.freight.contains_key(&B));
}

// ---- the sorted lists ---------------------------------------------------------------------------

fn sorted(out: &[Out], to: ObjectId) -> SortedWareHouseWithdrawalList {
    decode(of(out, to, "WAREHOUSE_WITHDRAW_LIST")[0])
}

fn sorted_ids(list: &SortedWareHouseWithdrawalList) -> Vec<i32> {
    list.items.iter().map(|i| i.object_id).collect()
}

fn sorted_scene() -> Scene {
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(101, ADENA, 77)],
        vec![
            wh(300, SWORD, 1),
            wh(301, ADENA, 1_000),
            wh(302, CUIRASS, 1),
            wh(303, BOW, 1),
            wh(304, BOLT, 500),
            wh(305, AMULET, 1),
            wh(306, ORE, 30),
            wh(307, DAGGER, 1),
            wh(308, HELMET, 1),
            wh(309, PILE, 4),
            wh(310, COBWEB, 8),
        ],
    ));
    s.talk(A);
    s
}

#[test]
fn the_sorted_weapon_list_is_filtered_and_sorted_by_name() {
    let mut s = sorted_scene();
    s.bypass(A, "WithdrawSortedP WEAPON A2Z");
    let out = s.log.take();
    let list = sorted(&out, A);
    // Weapons, arrows and the money; adena first for A..Z (`WarehouseItemNameComparator`), then by
    // name: "A Dagger", "C Sword", "D Bow", "Bolt" is by `String.compareTo`: A < B < C < D.
    assert_eq!(sorted_ids(&list), [301, 307, 304, 300, 303]);
    // `WareHouseWithdrawalList.PRIVATE`, not the sorted constants.
    assert_eq!((list.wh_type, list.player_adena), (1, 77));
    // Z..A puts adena last.
    s.bypass(A, "WithdrawSortedP WEAPON Z2A");
    let list = sorted(&s.log.take(), A);
    assert_eq!(sorted_ids(&list), [303, 300, 304, 307, 301]);
}

#[test]
fn the_sorted_armor_list_sorts_by_grade_and_by_body_part() {
    let mut s = sorted_scene();
    // GRADE: by name first, then (stable) by grade: D helmet, B cuirass; adena first.
    s.bypass(A, "WithdrawSortedP ARMOR GRADE");
    let list = sorted(&s.log.take(), A);
    assert_eq!(sorted_ids(&list), [301, 308, 302]);
    // WEAR: by body part mask: the helmet (head 0x40) before the cuirass (chest 0x400).
    s.bypass(A, "WithdrawSortedP ARMOR WEAR");
    let list = sorted(&s.log.take(), A);
    assert_eq!(sorted_ids(&list), [301, 308, 302]);
    assert_eq!(list.items[1].body_part, 0x40);
    assert_eq!(list.items[2].body_part, 0x400);
}

#[test]
fn the_other_sorted_lists_follow_their_filters() {
    let mut s = sorted_scene();
    // By name "Aged Cobweb" comes before "Ore"; TYPE sorts by the material (silver before
    // cobweb in `MaterialType`), after the name sort.
    s.bypass(A, "WithdrawSortedP MATERIAL TYPE");
    assert_eq!(sorted_ids(&sorted(&s.log.take(), A)), [301, 306, 310]);
    // `toUpperCase().startsWith("AMULET")` on an etc item.
    s.bypass(A, "WithdrawSortedP AMULETT A2Z");
    assert_eq!(sorted_ids(&sorted(&s.log.take(), A)), [301, 305]);
    // A type without a parameter order is A..Z; the type alone, and no parameter at all, is ALL.
    s.bypass(A, "WithdrawSortedP ALL");
    let list = sorted(&s.log.take(), A);
    assert_eq!(list.items.len(), 11);
    assert_eq!(list.items[0].object_id, 301);
    s.bypass(A, "WithdrawSortedP");
    assert_eq!(sorted(&s.log.take(), A).items.len(), 11);
}

#[test]
fn an_unknown_list_type_sends_nothing() {
    // `WarehouseListType.valueOf` throws; the handler logs it and returns false.
    let mut s = sorted_scene();
    s.bypass(A, "WithdrawSortedP BOGUS A2Z");
    let out = s.log.take();
    assert!(of(&out, A, "WAREHOUSE_WITHDRAW_LIST").is_empty());
}

#[test]
fn with_sorting_on_withdraw_p_shows_the_sorting_page() {
    let mut settings = WorldSettings::default();
    settings.warehouse.sorting_private = true;
    let mut s = Scene::with(settings);
    s.enter(keeper_client(A, vec![], vec![wh(300, SWORD, 1)]));
    s.talk(A);
    s.bypass(A, "WithdrawP");
    let out = s.log.take();
    assert_eq!(names(&out, A), ["NPC_HTML_MESSAGE", "ACTION_FAIL"]);
    // The page's own link is accepted afterwards.
    s.bypass(A, "WithdrawSortedP ALL");
    assert_eq!(sorted_ids(&sorted(&s.log.take(), A)), [300]);
}

// ---- the clan warehouse -------------------------------------------------------------------------

fn clan_member(id: ObjectId, power_grade: i32) -> ClanMemberRecord {
    ClanMemberRecord {
        object_id: id.0,
        name: format!("P{}", id.0 - 0x6000_0000),
        level: 40,
        class_id: 0,
        title: String::new(),
        power_grade,
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
        female: false,
        race: 0,
    }
}

/// Clan 500 of `level`: A leads, B is rank 5 without the warehouse privilege, C is rank 6 with it.
fn lions(level: i32, warehouse: Vec<ItemData>) -> ClanRecord {
    ClanRecord {
        clan_id: CLAN,
        data: Some(ClanDataRecord {
            name: Some("Lions".to_owned()),
            level,
            leader_id: A.0,
            ..ClanDataRecord::default()
        }),
        members: vec![clan_member(A, 1), clan_member(B, 5), clan_member(C, 6)],
        rank_privs: vec![(5, 0, 0), (6, 0, ClanAccess::AccessWarehouse.mask())],
        warehouse,
        ..ClanRecord::default()
    }
}

fn clan_data(id: ObjectId, power_grade: i32, items: Vec<ItemData>) -> PlayerData {
    let mut data = player(id, &format!("P{}", id.0 - 0x6000_0000), items);
    data.profile.clan = Some(PlayerClanState {
        clan_id: CLAN,
        power_grade,
        ..PlayerClanState::default()
    });
    data
}

fn shadow_item() -> ItemData {
    let mut shadow = inv(108, SHADOW, 1);
    shadow.mana_left = 100;
    shadow
}

fn clan_wh(object_id: i32, item_id: i32, count: i64) -> ItemData {
    item(object_id, item_id, count, "CLANWH")
}

fn clan_scene(level: i32, settings: WorldSettings) -> Scene {
    let mut s = Scene::with(settings);
    s.world.load_clans(
        vec![lions(level, vec![clan_wh(400, SWORD, 1), clan_wh(401, PILE, 50)])],
        Vec::new(),
    );
    s
}

#[test]
fn the_clan_warehouse_needs_a_clan_of_level_one() {
    let mut s = Scene::new();
    s.enter(keeper_client(A, vec![], vec![]));
    s.talk(A);
    s.bypass(A, "DepositC");
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouDoNotHaveTheRightToUseTheClanWarehouse]
    );

    let mut s = clan_scene(0, WorldSettings::default());
    s.enter(clan_data(A, 1, vec![]));
    s.talk(A);
    s.bypass(A, "DepositC");
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::OnlyClansOfClanLevel1OrHigherCanUseAClanWarehouse]
    );
}

#[test]
fn the_clan_warehouse_is_read_with_the_clan() {
    let s = clan_scene(3, WorldSettings::default());
    let clan = s.world.clan(CLAN).unwrap();
    assert_eq!(clan.warehouse.len(), 2);
    // An item whose template the datapack does not know never joins a container.
    let mut s = Scene::new();
    s.world.load_clans(
        vec![lions(3, vec![clan_wh(400, SWORD, 1), clan_wh(402, GHOST, 1)])],
        Vec::new(),
    );
    assert_eq!(s.world.clan(CLAN).unwrap().warehouse.len(), 1);
}

#[test]
fn a_deposit_to_the_clan_warehouse_writes_the_clan_as_owner() {
    let mut s = clan_scene(3, WorldSettings::default());
    s.enter(clan_data(
        B,
        5,
        vec![
            inv(100, CUIRASS, 1),
            inv(101, ADENA, 1000),
            inv(106, BOUND, 1),
            inv(107, PILE, 20),
            shadow_item(),
        ],
    ));
    s.talk(B);
    s.bypass(B, "DepositC");
    let out = s.log.take();
    // Anybody in a clan of level 1 or more may deposit; `CLAN` = 4. The bound item is not
    // tradeable and the shadow item is a shadow item, so they are not offered
    // (`isDepositable(false)`).
    // The bypass dispatcher's own ActionFailed now comes before the window: the window waits for
    // the rows of the clan (M6-SYNC-01).
    assert_eq!(
        names(&out, B),
        ["ACTION_FAIL", "ACTION_FAIL", "WAREHOUSE_DEPOSIT_LIST"]
    );
    let list: WareHouseDepositList = decode(of(&out, B, "WAREHOUSE_DEPOSIT_LIST")[0]);
    assert_eq!(list.wh_type, 4);
    let mut shown: Vec<i32> = list.items.iter().map(|i| i.object_id).collect();
    shown.sort_unstable();
    assert_eq!(shown, [100, 101, 107]);
    assert_eq!(
        s.world.player(B).unwrap().warehouse.active,
        Some(ContainerRef::Clan(CLAN))
    );

    // The cuirass moves whole: the row's owner becomes the clan, `loc` is CLANWH. Five of the pile
    // join the clan's stack of 50 (an `UPDATE` of the clan's row).
    s.deposit(B, &[(100, 1), (107, 5)]);
    let out = s.log.take();
    assert_eq!(
        flat(&rows(&out)[0].1),
        [
            ("write", CLAN, 100, CUIRASS, 1, "CLANWH".to_string()),
            ("write", B.0, 107, PILE, 15, "INVENTORY".to_string()),
            ("update", CLAN, 401, PILE, 55, "CLANWH".to_string()),
        ]
    );
    assert_eq!(rows(&out)[0].0, Some(B));
    assert_eq!(s.world.clan(CLAN).unwrap().warehouse.len(), 3);
    assert_eq!(s.adena(B), 940);
}

#[test]
fn only_members_with_the_privilege_see_the_clan_warehouse() {
    let mut s = clan_scene(3, WorldSettings::default());
    s.enter(clan_data(B, 5, vec![]));
    s.talk(B);
    s.bypass(B, "WithdrawC");
    let out = s.log.take();
    // `ACCESS_WAREHOUSE` missing: ActionFailed, then the message, no list.
    // (The dispatcher's ActionFailed comes first: the window waits for the rows, M6-SYNC-01.)
    assert_eq!(names(&out, B), ["ACTION_FAIL", "ACTION_FAIL", "SYSTEM_MESSAGE"]);
    assert_eq!(
        message_ids(&out, B),
        [SystemMessageId::YouDoNotHaveTheRightToUseTheClanWarehouse]
    );

    s.enter(clan_data(C, 6, vec![]));
    s.talk(C);
    s.bypass(C, "WithdrawC");
    let out = s.log.take();
    assert_eq!(
        names(&out, C),
        ["ACTION_FAIL", "ACTION_FAIL", "WAREHOUSE_WITHDRAW_LIST"]
    );
    let list: WareHouseWithdrawalList = decode(of(&out, C, "WAREHOUSE_WITHDRAW_LIST")[0]);
    assert_eq!(list.wh_type, 4);
    assert_eq!(list.items.len(), 2);
}

#[test]
fn a_member_withdraws_only_when_the_setting_allows_it() {
    // `AltMembersCanWithdrawFromClanWH = False` (the default): only the leader.
    let mut s = clan_scene(3, WorldSettings::default());
    s.enter(clan_data(C, 6, vec![inv(101, ADENA, 10)]));
    s.talk(C);
    s.bypass(C, "WithdrawC");
    s.log.take();
    s.withdraw(C, &[(400, 1)]);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, C),
        [SystemMessageId::ItemsLeftAtTheClanHallWarehouseCanOnlyBeRetrievedByTheClanLeaderDoYouWantToContinue]
    );
    assert_eq!(s.world.clan(CLAN).unwrap().warehouse.len(), 2);

    // With the setting on, `ACCESS_WAREHOUSE` is enough; the rows get the member as owner.
    let mut settings = WorldSettings::default();
    settings.warehouse.members_can_withdraw_from_clanwh = true;
    let mut s = clan_scene(3, settings);
    s.enter(clan_data(C, 6, vec![inv(101, ADENA, 10)]));
    s.talk(C);
    s.bypass(C, "WithdrawC");
    s.log.take();
    s.withdraw(C, &[(400, 1)]);
    let out = s.log.take();
    assert_eq!(
        flat(&rows(&out)[0].1),
        [("update", C.0, 400, SWORD, 1, "INVENTORY".to_string())]
    );
    assert!(s.inventory_of(C).contains(&(400, SWORD, 1)));
    assert_eq!(s.world.clan(CLAN).unwrap().warehouse.len(), 1);

    // ... but not for a member without the privilege.
    let mut settings = WorldSettings::default();
    settings.warehouse.members_can_withdraw_from_clanwh = true;
    let mut s = clan_scene(3, settings);
    s.enter(clan_data(B, 5, vec![]));
    s.talk(B);
    s.bypass(B, "DepositC");
    s.log.take();
    s.withdraw(B, &[(400, 1)]);
    assert!(s.log.take().is_empty());
    assert_eq!(s.world.clan(CLAN).unwrap().warehouse.len(), 2);
}

#[test]
fn the_leader_withdraws_and_the_clan_row_changes_owner() {
    let mut s = clan_scene(3, WorldSettings::default());
    s.enter(clan_data(A, 1, vec![inv(101, ADENA, 10)]));
    s.talk(A);
    s.bypass(A, "WithdrawC");
    s.log.take();
    s.withdraw(A, &[(400, 1), (401, 20)]);
    let out = s.log.take();
    let new_pile = s
        .world
        .player(A)
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.item_id == PILE)
        .unwrap()
        .object_id
        .0;
    assert_eq!(
        flat(&rows(&out)[0].1),
        [
            ("update", A.0, 400, SWORD, 1, "INVENTORY".to_string()),
            ("update", CLAN, 401, PILE, 30, "CLANWH".to_string()),
            ("insert", A.0, new_pile, PILE, 20, "INVENTORY".to_string()),
        ]
    );
    assert_eq!(s.world.clan(CLAN).unwrap().warehouse.len(), 1);
}

#[test]
fn time_limited_items_are_destroyed_when_the_clan_window_opens() {
    let mut s = Scene::new();
    let mut expired = clan_wh(402, SWORD, 1);
    expired.time = 1;
    let mut lasting = clan_wh(403, PILE, 5);
    lasting.time = i64::MAX / 2;
    s.world.load_clans(
        vec![lions(3, vec![clan_wh(400, CUIRASS, 1), expired, lasting])],
        Vec::new(),
    );
    s.enter(clan_data(A, 1, vec![]));
    s.talk(A);
    s.bypass(A, "WithdrawC");
    let out = s.log.take();
    // `isTimeLimitedItem() && getRemainingTime() <= 0`: destroyed, its row deleted.
    let removed = flat(&rows(&out)[0].1);
    assert_eq!(removed, [("delete", 0, 402, 0, 0, String::new())]);
    let list: WareHouseWithdrawalList = decode(of(&out, A, "WAREHOUSE_WITHDRAW_LIST")[0]);
    let shown: Vec<i32> = list.items.iter().map(|i| i.object_id).collect();
    assert_eq!(shown, [400, 403]);

    // The private warehouse does not check (Java only does for the clan warehouse and the freight).
    let mut s = Scene::new();
    let mut old = wh(300, SWORD, 1);
    old.time = 1;
    s.enter(keeper_client(A, vec![], vec![old]));
    s.talk(A);
    s.bypass(A, "WithdrawP");
    let out = s.log.take();
    assert!(rows(&out).is_empty());
    assert_eq!(s.private_of(A).len(), 1);
}

#[test]
fn leaving_the_clan_forgets_the_active_warehouse() {
    let mut s = clan_scene(3, WorldSettings::default());
    s.enter(clan_data(C, 6, vec![inv(100, CUIRASS, 1), inv(101, ADENA, 100)]));
    s.talk(C);
    s.bypass(C, "DepositC");
    assert!(s.world.player(C).unwrap().warehouse.active.is_some());
    s.input(C, ClientInput::Clan(ClanInput::WithdrawalPledge));
    assert_eq!(s.world.player(C).unwrap().warehouse.active, None);
    s.log.take();
    s.deposit(C, &[(100, 1)]);
    assert!(rows(&s.log.take()).is_empty());
}

#[test]
fn a_dissolved_clan_destroys_its_warehouse_rows_when_it_writes() {
    let mut s = clan_scene(3, WorldSettings::default());
    s.log.take();
    // D-49: without the maintenance switch the rows are left to the Java game server.
    s.world.destroy_clan(CLAN, false);
    assert!(rows(&s.log.take()).is_empty());

    let mut s = clan_scene(3, WorldSettings::default());
    s.log.take();
    s.world.destroy_clan(CLAN, true);
    let commands = rows(&s.log.take());
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].0, None);
    assert_eq!(
        flat(&commands[0].1),
        [
            ("delete", 0, 400, 0, 0, String::new()),
            ("delete", 0, 401, 0, 0, String::new()),
        ]
    );
}

// ---- the freight --------------------------------------------------------------------------------

/// Alice with Bob ("Bobby") and Cleo as the other characters of her account; Bobby's freight holds
/// a stack of pile.
fn freight_client(inventory: Vec<ItemData>, own_freight: Vec<ItemData>) -> PlayerData {
    let mut data = player(A, "Alice", inventory);
    data.profile.warehouse = WarehouseLoad {
        private: vec![],
        freight: vec![(A, own_freight), (B, vec![item(500, PILE, 10, "FREIGHT")])],
        account_chars: vec![(B, "Bobby".to_string()), (C, "Cleo".to_string())],
    };
    data
}

#[test]
fn package_deposit_lists_the_other_characters_of_the_account() {
    let mut s = Scene::new();
    s.enter(freight_client(vec![], vec![]));
    s.talk(A);
    s.bypass(A, "package_deposit");
    let out = s.log.take();
    let list: PackageToList = decode(of(&out, A, "PACKAGE_TO_LIST")[0]);
    assert_eq!(
        list.players,
        [(B.0, "Bobby".to_string()), (C.0, "Cleo".to_string())]
    );

    // An account with one character: THAT_CHARACTER_DOES_NOT_EXIST.
    let mut s = Scene::new();
    s.enter(player(A, "Alice", vec![]));
    s.talk(A);
    s.bypass(A, "package_deposit");
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::ThatCharacterDoesNotExist]
    );
}

#[test]
fn the_sendable_list_is_the_freightable_inventory() {
    let mut s = Scene::new();
    s.enter(freight_client(
        vec![
            inv(100, SWORD, 1),
            inv(101, ADENA, 1000),
            inv(102, CUIRASS, 1),
            inv(103, PILE, 7),
        ],
        vec![],
    ));
    s.talk(A);
    s.input(
        A,
        ClientInput::Warehouse(WarehouseInput::PackageSendableList { object_id: B.0 }),
    );
    let out = s.log.take();
    let list: PackageSendableList = decode(of(&out, A, "PACKAGE_SENDABLE_LIST")[0]);
    assert_eq!((list.player_object_id, list.adena), (B.0, 1000));
    // `getAvailableItems(true, true, true)`: only the items the template calls freightable.
    let mut shown: Vec<i32> = list.items.iter().map(|i| i.object_id).collect();
    shown.sort_unstable();
    assert_eq!(shown, [100, 103]);
}

#[test]
fn a_package_costs_the_freight_price_per_item_and_joins_the_stacks_of_the_receiver() {
    let mut s = Scene::new();
    s.enter(freight_client(
        vec![inv(100, SWORD, 1), inv(101, ADENA, 5000), inv(103, PILE, 7)],
        vec![],
    ));
    s.talk(A);
    s.log.take();
    s.send_package(A, B, &[(100, 1), (103, 3)]);
    s.advance(150);
    let out = s.log.take();

    // `_items.length * ALT_FREIGHT_PRICE` = 2000.
    assert_eq!(s.adena(A), 3000);
    // The sword's row is now Bobby's freight; 3 of the pile join Bobby's stack of 10.
    let freight = s.freight_of(A, B);
    assert_eq!(freight.len(), 2);
    let pile = freight.iter().find(|i| i.item_id == PILE).unwrap();
    assert_eq!((pile.object_id.0, pile.count), (500, 13));
    let sword = freight.iter().find(|i| i.item_id == SWORD).unwrap();
    assert_eq!((sword.object_id.0, sword.loc.as_str()), (100, "FREIGHT"));
    assert_eq!(
        flat(&rows(&out)[0].1),
        [
            ("write", B.0, 100, SWORD, 1, "FREIGHT".to_string()),
            ("write", A.0, 103, PILE, 4, "INVENTORY".to_string()),
            ("update", B.0, 500, PILE, 13, "FREIGHT".to_string()),
        ]
    );
    assert_eq!(rows(&out)[0].0, Some(A));
    let update = inventory_update(&out, A);
    assert!(update.contains(&(InventoryUpdate::REMOVED, 100, 1)), "{update:?}");
    assert!(
        update.contains(&(InventoryUpdate::MODIFIED, 103, 4)),
        "{update:?}"
    );
}

#[test]
fn a_package_stops_at_the_checks_of_request_package_send() {
    let mut s = Scene::new();
    s.enter(freight_client(
        vec![
            inv(100, SWORD, 1),
            inv(101, ADENA, 1500),
            inv(102, CUIRASS, 1),
            inv(103, PILE, 7),
        ],
        vec![],
    ));
    s.talk(A);
    s.log.take();

    // Not a character of the account: ignored without a word.
    s.send_package(A, ObjectId(0x6000_0777), &[(100, 1)]);
    assert!(s.log.take().is_empty());

    // The cuirass is not freightable: the packet ends before anything is charged.
    s.send_package(A, B, &[(100, 1), (102, 1)]);
    assert!(s.log.take().is_empty());
    assert_eq!(s.adena(A), 1500);

    // Two items cost 2000 and the player has 1500.
    s.send_package(A, B, &[(100, 1), (103, 1)]);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouDoNotHaveEnoughAdena]
    );
    assert_eq!(s.adena(A), 1500);

    // Too far from the keeper (`isInsideRadius2D(manager, 250)` is strict).
    s.world.set_position(A, X - 201, Y, Z);
    s.log.take();
    s.send_package(A, B, &[(100, 1)]);
    assert!(s.log.take().is_empty());
    assert_eq!(s.adena(A), 1500);
    s.world.set_position(A, X, Y, Z);
    s.send_package(A, B, &[(100, 1)]);
    assert_eq!(s.adena(A), 500);

    // Without a keeper talked to, nothing happens either.
    let mut s = Scene::new();
    s.enter(freight_client(
        vec![inv(100, SWORD, 1), inv(101, ADENA, 5000)],
        vec![],
    ));
    s.log.take();
    s.send_package(A, B, &[(100, 1)]);
    assert!(s.log.take().is_empty());
    assert_eq!(s.adena(A), 5000);
}

#[test]
fn the_freight_of_a_receiver_has_its_own_limit() {
    // `new PlayerFreight(objectId)`: `getSize() + slots <= ALT_FREIGHT_SLOTS`.
    let mut settings = WorldSettings::default();
    settings.warehouse.freight_slots = 1;
    let mut s = Scene::with(settings);
    s.enter(freight_client(
        vec![inv(100, SWORD, 1), inv(101, ADENA, 5000)],
        vec![],
    ));
    s.talk(A);
    s.log.take();
    // Bobby's freight already holds one stack: a sword needs a second slot.
    s.send_package(A, B, &[(100, 1)]);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted]
    );
    assert_eq!(s.adena(A), 5000);
}

#[test]
fn package_withdraw_opens_the_own_freight() {
    let mut s = Scene::new();
    s.enter(freight_client(vec![inv(101, ADENA, 50)], vec![]));
    s.talk(A);
    s.bypass(A, "package_withdraw");
    // No ActionFailed of its own here, just the message (and the one after every `npc_` command).
    assert_eq!(names(&s.log.take(), A), ["SYSTEM_MESSAGE", "ACTION_FAIL"]);

    let mut s = Scene::new();
    s.enter(freight_client(
        vec![inv(101, ADENA, 50)],
        vec![item(600, SWORD, 1, "FREIGHT"), item(601, PILE, 9, "FREIGHT")],
    ));
    s.talk(A);
    s.bypass(A, "package_withdraw");
    let out = s.log.take();
    assert_eq!(names(&out, A), ["WAREHOUSE_WITHDRAW_LIST", "ACTION_FAIL"]);
    let list: WareHouseWithdrawalList = decode(of(&out, A, "WAREHOUSE_WITHDRAW_LIST")[0]);
    assert_eq!((list.wh_type, list.items.len()), (1, 2));
    assert_eq!(
        s.world.player(A).unwrap().warehouse.active,
        Some(ContainerRef::Freight { holder: A, owner: A })
    );

    // Withdrawing is the same packet as for the warehouse; the rows go back to the inventory.
    s.withdraw(A, &[(600, 1), (601, 9)]);
    let out = s.log.take();
    assert!(s.freight_of(A, A).is_empty());
    assert_eq!(
        flat(&rows(&out)[0].1),
        [
            ("update", A.0, 600, SWORD, 1, "INVENTORY".to_string()),
            ("delete", 0, 601, 0, 0, String::new()),
            (
                "insert",
                A.0,
                s.world
                    .player(A)
                    .unwrap()
                    .data
                    .items
                    .iter()
                    .find(|i| i.item_id == PILE)
                    .unwrap()
                    .object_id
                    .0,
                PILE,
                9,
                "INVENTORY".to_string()
            ),
        ]
    );
}

#[test]
fn only_a_warehouse_keeper_serves_the_warehouse_bypasses() {
    let mut s = Scene::new();
    s.enter(keeper_client(
        A,
        vec![inv(101, ADENA, 50)],
        vec![wh(300, SWORD, 1)],
    ));
    // A villager has no warehouse page: the bypass is not in the player's html cache either.
    let villager = s.world.spawn_npc_at(30002, X + 20, Y, Z, 0).unwrap();
    for _ in 0..2 {
        s.input(
            A,
            ClientInput::Action {
                object_id: villager.0,
                action_id: 0,
            },
        );
    }
    s.log.take();
    s.input(
        A,
        ClientInput::Bypass {
            command: format!("npc_{}_WithdrawP", villager.0),
        },
    );
    assert!(of(&s.log.take(), A, "WAREHOUSE_WITHDRAW_LIST").is_empty());
}

// ---- one database, two servers (M6-SYNC-01) -----------------------------------------------------

/// The window of the clan warehouse waits for the rows of the clan (D-15: the world asks, the
/// persistence task answers) and shows the refreshed container: an item the Java GS took is not in
/// the list, one it added is.
#[test]
fn the_clan_warehouse_window_shows_the_refreshed_rows() {
    let mut s = clan_scene(3, WorldSettings::default());
    s.enter(clan_data(A, 1, vec![]));
    s.talk(A);

    s.bypass_unanswered(A, "WithdrawC");
    assert_eq!(s.take_clan_requests(), [(A, CLAN)]);
    let waiting = s.log.take();
    assert!(
        of(&waiting, A, "WAREHOUSE_WITHDRAW_LIST").is_empty(),
        "no list before the rows"
    );
    assert_eq!(s.world.player(A).unwrap().warehouse.active, None);
    assert_eq!(s.world.clan(CLAN).unwrap().warehouse.len(), 2);

    // The sword (400) was taken at the Java GS, a cuirass (405) was deposited there.
    s.world.handle(WorldCommand::ClanWarehouseLoaded {
        player: A,
        clan_id: CLAN,
        result: Ok(vec![clan_wh(401, PILE, 50), clan_wh(405, CUIRASS, 1)]),
    });
    let out = s.log.take();
    let list: WareHouseWithdrawalList = decode(of(&out, A, "WAREHOUSE_WITHDRAW_LIST")[0]);
    let mut shown: Vec<i32> = list.items.iter().map(|i| i.object_id).collect();
    shown.sort_unstable();
    assert_eq!(shown, [401, 405]);
    assert_eq!(
        s.world.player(A).unwrap().warehouse.active,
        Some(ContainerRef::Clan(CLAN))
    );
    let ids: Vec<i32> = s
        .world
        .clan(CLAN)
        .unwrap()
        .warehouse
        .iter()
        .map(|i| i.object_id.0)
        .collect();
    assert_eq!(ids, [401, 405]);
}

/// A withdraw only moves what the refreshed container still has: the item taken elsewhere is
/// refused like any item that is not there, and no row is written for it.
#[test]
fn a_withdraw_does_not_move_an_item_taken_elsewhere() {
    let mut s = clan_scene(3, WorldSettings::default());
    s.enter(clan_data(A, 1, vec![]));
    s.talk(A);
    s.bypass_unanswered(A, "WithdrawC");
    s.take_clan_requests();
    s.world.handle(WorldCommand::ClanWarehouseLoaded {
        player: A,
        clan_id: CLAN,
        result: Ok(vec![clan_wh(401, PILE, 50)]),
    });
    s.log.take();

    s.withdraw(A, &[(400, 1)]);
    let out = s.log.take();
    assert!(rows(&out).is_empty(), "{:?}", rows(&out));
    assert!(s.inventory_of(A).iter().all(|(_, item, _)| *item != SWORD));
    assert_eq!(s.world.clan(CLAN).unwrap().warehouse.len(), 1);

    // What is still there moves.
    s.withdraw(A, &[(401, 20)]);
    let moved = rows(&s.log.take());
    assert_eq!(moved.len(), 1);
    assert!(
        s.inventory_of(A)
            .iter()
            .any(|(_, item, count)| *item == PILE && *count == 20)
    );
}

/// A failed read opens no window and never shows the container in memory.
#[test]
fn a_failed_clan_warehouse_read_opens_nothing() {
    let mut s = clan_scene(3, WorldSettings::default());
    s.enter(clan_data(A, 1, vec![inv(100, CUIRASS, 1)]));
    s.talk(A);
    s.bypass_unanswered(A, "WithdrawC");
    s.take_clan_requests();
    s.log.take();
    s.world.handle(WorldCommand::ClanWarehouseLoaded {
        player: A,
        clan_id: CLAN,
        result: Err(()),
    });
    let out = s.log.take();
    assert_eq!(names(&out, A), ["ACTION_FAIL"]);
    assert_eq!(s.world.player(A).unwrap().warehouse.active, None);
    assert_eq!(
        s.world.clan(CLAN).unwrap().warehouse.len(),
        2,
        "the old rows are not replaced"
    );

    // The request is over: a late answer does not open the window.
    s.world.handle(WorldCommand::ClanWarehouseLoaded {
        player: A,
        clan_id: CLAN,
        result: Ok(vec![]),
    });
    assert!(s.log.take().is_empty());
    assert_eq!(s.world.clan(CLAN).unwrap().warehouse.len(), 2);
}

/// `WithdrawC`, `WithdrawSortedC` and `DepositC` all ask first (with the sorting page on,
/// `WithdrawC` only shows the page); the window continues with the answer.
#[test]
fn every_clan_warehouse_window_asks_for_the_rows() {
    let mut settings = WorldSettings::default();
    settings.warehouse.sorting_clan = true;
    let mut s = clan_scene(3, settings);
    s.enter(clan_data(A, 1, vec![inv(100, CUIRASS, 1)]));
    s.talk(A);

    s.bypass_unanswered(A, "DepositC");
    assert_eq!(s.take_clan_requests(), [(A, CLAN)]);
    s.world.handle(WorldCommand::ClanWarehouseLoaded {
        player: A,
        clan_id: CLAN,
        result: Ok(vec![clan_wh(401, PILE, 50)]),
    });
    let out = s.log.take();
    assert_eq!(of(&out, A, "WAREHOUSE_DEPOSIT_LIST").len(), 1);
    assert_eq!(
        s.world.player(A).unwrap().warehouse.active,
        Some(ContainerRef::Clan(CLAN))
    );
    assert_eq!(s.world.clan(CLAN).unwrap().warehouse.len(), 1);

    // The sorting page is html only; the sorted list asks.
    s.bypass_unanswered(A, "WithdrawC");
    assert!(s.take_clan_requests().is_empty());
    assert_eq!(of(&s.log.take(), A, "NPC_HTML_MESSAGE").len(), 1);
    s.bypass_unanswered(A, "WithdrawSortedC ALL");
    assert_eq!(s.take_clan_requests(), [(A, CLAN)]);
    s.log.take();
    s.world.handle(WorldCommand::ClanWarehouseLoaded {
        player: A,
        clan_id: CLAN,
        result: Ok(vec![clan_wh(405, CUIRASS, 1), clan_wh(406, PILE, 3)]),
    });
    assert_eq!(sorted_ids(&sorted(&s.log.take(), A)).len(), 2);
}

/// The private warehouse is not shared with other players, so it is not read again.
#[test]
fn the_private_warehouse_does_not_ask() {
    let mut s = Scene::new();
    s.enter(keeper_client(A, vec![], vec![wh(300, SWORD, 1)]));
    s.talk(A);
    s.bypass_unanswered(A, "WithdrawP");
    assert!(s.take_clan_requests().is_empty());
    assert_eq!(of(&s.log.take(), A, "WAREHOUSE_WITHDRAW_LIST").len(), 1);
}
