#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Multisell (M6-MULTISELL-01): the `multisell` / `exc_multisell` bypasses, the `MultiSellList`
//! pages, `MultiSellChoose` and the special ingredients. The expected values come from the Java
//! code named in each test (`MultisellData`, `PreparedListContainer`, `PreparedEntry`,
//! `MultiSellChoose`, `Multisell`).

use std::sync::{Arc, Mutex};

use l2_data::multisell::{MultisellData, MultisellLoadOptions};
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{MultiSellList, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::effects::item_extras::ItemElemental;
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
const GEM: i32 = 100;
const SWORD: i32 = 200;
const AXE: i32 = 201;
const BOOTS: i32 = 300;
const BOX: i32 = 400;
const HEAVY: i32 = 500;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="100" type="EtcItem" name="Gem"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="200" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat><stat type="pAtkSpd">379</stat></stats></item>
<item id="201" type="Weapon" name="Axe"><set name="weapon_type" val="BLUNT" /><set name="bodypart" val="rhand" /><set name="weight" val="60" />
<stats><stat type="pAtk">60</stat><stat type="pAtkSpd">379</stat></stats></item>
<item id="300" type="Armor" name="Boots"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="feet" /><set name="weight" val="10" /></item>
<item id="400" type="EtcItem" name="Box"><set name="is_stackable" val="true" /><set name="weight" val="2" /></item>
<item id="500" type="EtcItem" name="Heavy"><set name="is_stackable" val="true" /><set name="weight" val="1000000" /></item>
</list>"#;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="5000" type="Merchant" level="10" name="Trader" usingServerSideName="true"/>
  <npc id="9999" type="Merchant" level="10" name="Other" usingServerSideName="true"/>
</list>"#;

/// Which lists exist: see the comments on each.
fn lists() -> Vec<(i32, String)> {
    let many: String = (1..=95)
        .map(|n| {
            format!(r#"<item><ingredient count="{n}" id="100"/><production count="1" id="400"/></item>"#)
        })
        .collect();
    vec![
        // The shop of the tests, for the NPC 5000.
        (
            1,
            r#"<list><npcs><npc>5000</npc></npcs>
  <item><ingredient count="1000" id="57"/><ingredient count="5" id="100"/><production count="2" id="400"/></item>
  <item><ingredient count="1" id="200"/><production count="1" id="201"/></item>
  <item><ingredient count="50" id="-300"/><production count="1" id="100"/></item>
  <item><ingredient count="10" id="57"/><production count="30" id="-300"/></item>
  <item><ingredient count="1" id="57"/><production count="1" id="500"/></item>
  <item><ingredient count="1" id="200"/><ingredient count="1" id="200"/><production count="1" id="201"/></item>
  <item><ingredient count="100" id="-200"/><production count="1" id="400"/></item>
</list>"#
                .to_owned(),
        ),
        // Keeps the enchantment of the sword.
        (
            2,
            r#"<list maintainEnchantment="true"><npcs><npc>5000</npc></npcs>
  <item><ingredient count="1" id="200"/><ingredient count="10" id="57"/><production count="1" id="201"/></item>
</list>"#
                .to_owned(),
        ),
        // Every NPC (and none) may open it.
        (
            3,
            r#"<list><npcs><npc>-1</npc></npcs><item><ingredient count="5" id="57"/><production count="1" id="400"/></item></list>"#
                .to_owned(),
        ),
        // Another NPC's list.
        (
            4,
            r#"<list><npcs><npc>9999</npc></npcs><item><ingredient count="5" id="57"/><production count="1" id="400"/></item></list>"#
                .to_owned(),
        ),
        (5, r#"<list><npcs><npc>5000</npc></npcs></list>"#.to_owned()),
        (6, format!(r#"<list><npcs><npc>5000</npc></npcs>{many}</list>"#)),
        // The sword stays when it is a `maintainIngredient`.
        (
            7,
            r#"<list><npcs><npc>5000</npc></npcs>
  <item><ingredient count="1" id="200" maintainIngredient="true"/><ingredient count="10" id="57"/><production count="1" id="400"/></item>
</list>"#
                .to_owned(),
        ),
        // The inventory list without enchantments: ids are unique, so a worn sword hides the rest.
        (
            8,
            r#"<list><npcs><npc>5000</npc></npcs>
  <item><ingredient count="5" id="100"/><production count="1" id="400"/></item>
  <item><ingredient count="1" id="200"/><production count="1" id="201"/></item>
</list>"#
                .to_owned(),
        ),
    ]
}

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

fn items() -> Arc<ItemTemplates> {
    Arc::new(ItemTemplates::from_xml(ITEMS).unwrap())
}

fn data() -> Data {
    let items = items();
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
const X: i32 = 1000;
const Y: i32 = 1000;

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

fn enchanted(mut item: ItemData, level: i32) -> ItemData {
    item.enchant_level = level;
    item
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
        level: 1,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X,
        y: Y,
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
    npc: ObjectId,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Self {
        Self::with_settings(MultisellSettings::default())
    }

    fn with_settings(settings: MultisellSettings) -> Self {
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
        // The page of the trader: the bypass commands must be on a page the player was shown.
        let links: String = (1..=8)
            .map(|n| {
                format!(
                    "<a action=\"bypass -h npc_%objectId%_multisell {n}\">m{n}</a>\
                     <a action=\"bypass -h npc_%objectId%_exc_multisell {n}\">e{n}</a>"
                )
            })
            .collect();
        write(
            "data/html/merchant/5000.htm",
            &format!(
                "<html><body>Hello {links}<a action=\"bypass -h npc_%objectId%_multisell\">x</a><a action=\"bypass -h npc_%objectId%_multisell abc\">y</a></body></html>"
            ),
        );
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
        world.set_static_data(Box::new(data()));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        let owned = lists();
        let files: Vec<(i32, &str)> = owned.iter().map(|(id, xml)| (*id, xml.as_str())).collect();
        let multisell = MultisellData::from_xml(
            &files,
            &ItemTemplates::from_xml(ITEMS).unwrap(),
            &MultisellLoadOptions {
                correct_prices: false,
                ..Default::default()
            },
        );
        world.set_multisell_data(Arc::new(multisell), settings);
        let npc = world.spawn_npc_at(5000, X + 50, Y, 0, 0).unwrap();
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

    /// Alice with these items, standing next to the trader, who she has clicked twice (the second
    /// click shows the page, which is where the bypass commands come from).
    fn with_player(items: Vec<ItemData>) -> Self {
        Self::with_player_data(player(A, "Alice", items))
    }

    fn with_player_data(data: PlayerData) -> Self {
        let mut s = Self::new();
        s.enter(data);
        s.talk();
        s
    }

    fn talk(&mut self) {
        for _ in 0..2 {
            self.input(
                A,
                ClientInput::Action {
                    object_id: self.npc.0,
                    action_id: 0,
                },
            );
        }
        self.log.take();
    }

    fn bypass(&mut self, command: &str) {
        let command = format!("npc_{}_{command}", self.npc.0);
        self.input(A, ClientInput::Bypass { command });
    }

    fn choose(&mut self, list_id: i32, entry_id: i32, amount: i64) {
        self.input(
            A,
            ClientInput::MultiSellChoose {
                list_id,
                entry_id,
                amount,
            },
        );
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn items(&self) -> Vec<(i32, i32, i64)> {
        let mut items: Vec<(i32, i32, i64)> = self
            .world
            .player(A)
            .unwrap()
            .data
            .items
            .iter()
            .map(|i| (i.object_id.0, i.item_id, i.count))
            .collect();
        items.sort_unstable();
        items
    }

    fn count_of(&self, item_id: i32) -> i64 {
        self.items().iter().filter(|i| i.1 == item_id).map(|i| i.2).sum()
    }

    fn open_list(&self) -> Option<&PreparedList> {
        self.world.player(A).unwrap().multisell.as_ref()
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

fn packets(out: &[Out]) -> Vec<&[u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(_, bytes) => Some(bytes.as_slice()),
            _ => None,
        })
        .collect()
}

fn names(out: &[Out]) -> Vec<String> {
    packets(out).into_iter().map(name_of).collect()
}

fn lists_sent(out: &[Out]) -> Vec<MultiSellList> {
    packets(out)
        .into_iter()
        .filter(|b| name_of(b) == "MULTI_SELL_LIST")
        .map(decode::<MultiSellList>)
        .collect()
}

fn messages(out: &[Out]) -> Vec<SystemMessage> {
    packets(out)
        .into_iter()
        .filter(|b| name_of(b) == "SYSTEM_MESSAGE")
        .map(decode::<SystemMessage>)
        .collect()
}

fn message_ids(out: &[Out]) -> Vec<SystemMessageId> {
    messages(out).into_iter().map(|m| m.id).collect()
}

/// `(upserted object ids, removed object ids)` of every `ItemChanges`.
fn item_changes(out: &[Out]) -> Vec<(Vec<i32>, Vec<i32>)> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemChanges { upserts, removed, .. }) => Some((
                upserts.iter().map(|i| i.object_id.0).collect(),
                removed.iter().map(|o| o.0).collect(),
            )),
            _ => None,
        })
        .collect()
}

fn stock() -> Vec<ItemData> {
    vec![
        inv(10, ADENA, 5000),
        inv(11, GEM, 20),
        inv(12, SWORD, 1),
        enchanted(inv(13, SWORD, 1), 3),
        enchanted(inv(14, SWORD, 1), 5),
    ]
}

// ---- opening a list -----------------------------------------------------------------------------

#[test]
fn the_multisell_bypass_sends_the_whole_list() {
    let mut s = Scene::with_player(stock());
    s.bypass("multisell 1");
    let out = s.log.take();
    let sent = lists_sent(&out);
    assert_eq!(sent.len(), 1);
    let list = &sent[0];
    assert_eq!((list.list_id, list.page, list.finished), (1, 1, true));
    let ids: Vec<i32> = list.entries.iter().map(|e| e.entry_id).collect();
    // `PreparedEntry`: the template id times 100000.
    assert_eq!(
        ids,
        [100_000, 200_000, 300_000, 400_000, 500_000, 600_000, 700_000]
    );
    // Entry 1: 2 boxes for 5 gems and 1000 adena. The adena is moved behind the other
    // ingredients (`PreparedEntry` adds it last).
    let first = &list.entries[0];
    assert!(first.stackable);
    assert_eq!(
        first
            .ingredients
            .iter()
            .map(|i| (i.item_id, i.count))
            .collect::<Vec<_>>(),
        [(GEM, 5), (ADENA, 1000)]
    );
    assert_eq!(first.products[0].item_id, BOX);
    assert_eq!(first.products[0].count, 2);
    // The template's `type2` and body part come with the item.
    let template = items().get(BOX).unwrap().type2;
    assert_eq!(i32::from(first.products[0].type2), template);
    // A weapon product is not stackable, and shows its body part (right hand, 0x80).
    let swap = &list.entries[1];
    assert!(!swap.stackable);
    assert_eq!(swap.products[0].body_part, 0x80);
    // The special ids have no template: the raw id and type2 65535.
    let fame = &list.entries[2];
    assert_eq!(
        (fame.ingredients[0].item_id, fame.ingredients[0].type2),
        (-300, 65535)
    );
    assert_eq!(fame.ingredients[0].count, 50);
    assert_eq!(
        (
            list.entries[3].products[0].item_id,
            list.entries[3].products[0].body_part
        ),
        (-300, 0)
    );
    // The prepared list is the one the next `MultiSellChoose` is checked against.
    let open = s.open_list().unwrap();
    assert_eq!(
        (open.list_id, open.entries.len(), open.npc_object_id),
        (1, 7, s.npc.0)
    );
}

#[test]
fn exc_multisell_lists_what_the_inventory_can_start() {
    // The worn sword hides the +0 one that follows; a list that keeps enchantments tells the
    // levels apart, so the +3 sword is the first one that is not worn.
    let mut worn = enchanted(item(12, SWORD, 1, "PAPERDOLL"), 0);
    worn.loc_data = 5;
    let mut s = Scene::with_player(vec![
        inv(10, ADENA, 5000),
        worn,
        enchanted(inv(13, SWORD, 1), 3),
        enchanted(inv(14, SWORD, 1), 5),
    ]);
    s.bypass("exc_multisell 2");
    let sent = lists_sent(&s.log.take());
    assert_eq!(sent.len(), 1);
    // NOTE(java-quirk): one entry, not one per enchant level: the loop stops at the first match.
    assert_eq!(sent[0].entries.len(), 1);
    let entry = &sent[0].entries[0];
    assert_eq!(entry.entry_id, 100_003);
    // The sword ingredient shows the item's enchant level, the adena comes last.
    assert_eq!(entry.ingredients[0].item_id, SWORD);
    assert_eq!(entry.ingredients[0].detail.enchant, 3);
    assert_eq!(
        (entry.ingredients[1].item_id, entry.ingredients[1].count),
        (ADENA, 10)
    );
    // The product weapon takes the same level.
    assert_eq!(entry.products[0].detail.enchant, 3);

    // A list without enchantments: items are unique by id, so the worn sword (first) is the only
    // sword and an entry needs one that is not worn.
    s.bypass("exc_multisell 8");
    let sent = lists_sent(&s.log.take());
    // Only the gem entry is missing too: there are no gems. Nothing at all is listed, but the
    // packet is still sent once.
    assert_eq!(sent.len(), 1);
    assert!(sent[0].entries.is_empty());
    assert!(sent[0].finished);
}

#[test]
fn exc_multisell_shows_the_info_of_the_inventory_item() {
    let mut sword = enchanted(inv(14, SWORD, 1), 5);
    sword.augmentation = Some(77);
    sword.elementals = vec![ItemElemental {
        element: 0,
        value: 20,
    }];
    let mut s = Scene::with_player(vec![inv(10, ADENA, 100), sword]);
    s.bypass("exc_multisell 2");
    let sent = lists_sent(&s.log.take());
    let ingredient = &sent[0].entries[0].ingredients[0];
    assert_eq!(ingredient.detail.enchant, 5);
    assert_eq!(ingredient.detail.augment_id, 77);
    // Fire attack 20 (`getAttackElementType` / `getAttackElementPower`).
    assert_eq!(
        (
            ingredient.detail.attack_element,
            ingredient.detail.attack_element_power
        ),
        (0, 20)
    );
}

#[test]
fn the_bypass_needs_a_number_and_an_npc() {
    let mut s = Scene::with_player(stock());
    // `Integer.parseInt` fails for "" and "abc": a warning, no list.
    s.bypass("multisell");
    s.bypass("multisell abc");
    assert!(lists_sent(&s.log.take()).is_empty());
    assert!(s.open_list().is_none());
    // A list that does not exist: a warning, no list.
    s.bypass("multisell 4242");
    assert!(lists_sent(&s.log.take()).is_empty());
}

#[test]
fn a_list_of_another_npc_is_refused_unless_the_player_is_a_gm() {
    let mut s = Scene::with_player(stock());
    s.bypass("multisell 4");
    // The page has no link for list 4, so the bypass itself is not accepted; open it the way a
    // script does, through the NPC.
    assert!(lists_sent(&s.log.take()).is_empty());
    s.world.open_multisell(4, A, Some(s.npc), false);
    let out = s.log.take();
    assert!(lists_sent(&out).is_empty());
    assert!(s.open_list().is_none());

    // List 3 allows every NPC (-1), with or without one.
    s.world.open_multisell(3, A, Some(s.npc), false);
    assert_eq!(lists_sent(&s.log.take()).len(), 1);
    s.world.open_multisell(3, A, None, false);
    assert_eq!(lists_sent(&s.log.take()).len(), 1);
    // A list with NPCs needs one of them.
    s.world.open_multisell(1, A, None, false);
    assert!(lists_sent(&s.log.take()).is_empty());

    // A GM gets a message and the list.
    let mut gm = player(ObjectId(2), "Gamemaster", vec![]);
    gm.access_level = 100;
    s.enter(gm);
    s.log.take();
    s.world.open_multisell(4, ObjectId(2), Some(s.npc), false);
    let out = s.log.take();
    assert_eq!(lists_sent(&out).len(), 1);
    assert_eq!(message_ids(&out).len(), 1);
}

#[test]
fn a_long_list_comes_in_pages_of_forty() {
    let mut s = Scene::with_player(stock());
    s.world.open_multisell(6, A, Some(s.npc), false);
    let sent = lists_sent(&s.log.take());
    let pages: Vec<(i32, bool, usize)> = sent
        .iter()
        .map(|l| (l.page, l.finished, l.entries.len()))
        .collect();
    assert_eq!(pages, [(1, false, 40), (2, false, 40), (3, true, 15)]);
    // The ids run on over the pages.
    assert_eq!(sent[1].entries[0].entry_id, 4_100_000);
    assert_eq!(sent[2].entries[14].entry_id, 9_500_000);
    assert_eq!(s.open_list().unwrap().entries.len(), 95);

    // A list with no entries is still sent once.
    s.world.open_multisell(5, A, Some(s.npc), false);
    let sent = lists_sent(&s.log.take());
    assert_eq!(sent.len(), 1);
    assert_eq!(
        (sent[0].page, sent[0].finished, sent[0].entries.len()),
        (1, true, 0)
    );
}

#[test]
fn multipliers_scale_the_counts_but_never_to_zero() {
    let mut s = Scene::with_player(stock());
    s.world
        .multisell_separate_and_send(1, A, Some(s.npc), false, 0.5, 2.0);
    let sent = lists_sent(&s.log.take());
    let first = &sent[0].entries[0];
    // Products x0.5 (2 -> 1), ingredients x2.
    assert_eq!(first.products[0].count, 1);
    assert_eq!(
        first.ingredients.iter().map(|i| i.count).collect::<Vec<_>>(),
        [10, 2000]
    );
    // 1 * 0.5 would be 0: at least 1.
    assert_eq!(sent[0].entries[1].products[0].count, 1);
}

// ---- MultiSellChoose ----------------------------------------------------------------------------

#[test]
fn choosing_an_entry_takes_the_ingredients_and_gives_the_products() {
    let mut s = Scene::with_player(stock());
    s.bypass("multisell 1");
    s.log.take();
    s.choose(1, 100_000, 1);
    let out = s.log.take();
    assert_eq!(s.count_of(ADENA), 4000);
    assert_eq!(s.count_of(GEM), 15);
    assert_eq!(s.count_of(BOX), 2);
    // Two ingredients used up ("S2 S1 has disappeared"), then "You have earned 2 Box(es)".
    let m = messages(&out);
    assert_eq!(
        m.iter().map(|m| m.id).collect::<Vec<_>>(),
        [
            SystemMessageId::S2S1HasDisappeared,
            SystemMessageId::S2S1HasDisappeared,
            SystemMessageId::YouHaveEarnedS2S1S
        ]
    );
    assert_eq!(
        m[2].params,
        [
            SystemMessageParam::ItemName(BOX),
            SystemMessageParam::LongNumber(2)
        ]
    );
    // The list stays open for the next exchange.
    assert!(s.open_list().is_some());
    // The item list follows 250 ms later, and the load straight away.
    assert!(names(&out).contains(&"STATUS_UPDATE".to_owned()));
    s.advance(300);
    let later = s.log.take();
    assert!(names(&later).contains(&"ITEM_LIST".to_owned()));
    assert!(names(&later).contains(&"INVENTORY_UPDATE".to_owned()));
}

#[test]
fn the_amount_multiplies_ingredients_and_products() {
    let mut s = Scene::with_player(stock());
    s.bypass("multisell 1");
    s.log.take();
    s.choose(1, 100_000, 3);
    assert_eq!(s.count_of(ADENA), 2000);
    assert_eq!(s.count_of(GEM), 5);
    assert_eq!(s.count_of(BOX), 6);
    // 3 x 2 boxes in one message.
    let m = messages(&s.log.take());
    assert_eq!(
        m.last().unwrap().params,
        [
            SystemMessageParam::ItemName(BOX),
            SystemMessageParam::LongNumber(6)
        ]
    );
}

#[test]
fn missing_ingredients_are_named_and_nothing_is_taken() {
    let mut s = Scene::with_player(vec![inv(10, ADENA, 5000), inv(11, GEM, 4)]);
    s.bypass("multisell 1");
    s.log.take();
    s.choose(1, 100_000, 1);
    let m = messages(&s.log.take());
    assert_eq!(m.len(), 1);
    assert_eq!(m[0].id, SystemMessageId::S2UnitSOfTheItemS1IsAreRequired);
    assert_eq!(
        m[0].params,
        [
            SystemMessageParam::ItemName(GEM),
            SystemMessageParam::LongNumber(5)
        ]
    );
    assert_eq!(
        (s.count_of(ADENA), s.count_of(GEM), s.count_of(BOX)),
        (5000, 4, 0)
    );
    assert!(s.open_list().is_some());
}

#[test]
fn a_bad_amount_or_list_closes_the_list() {
    let mut s = Scene::with_player(stock());
    for amount in [0, -4, 10_001] {
        s.bypass("multisell 1");
        s.log.take();
        s.choose(1, 100_000, amount);
        assert!(s.open_list().is_none(), "amount {amount}");
        assert_eq!(s.count_of(BOX), 0);
    }
    // The limit itself is allowed (and then fails on the ingredients, keeping the list).
    s.bypass("multisell 1");
    s.choose(1, 100_000, 10_000);
    assert!(s.open_list().is_some());
    // Another list id than the open one.
    s.choose(2, 100_000, 1);
    assert!(s.open_list().is_none());
    // No list open at all.
    s.choose(1, 100_000, 1);
    assert_eq!(s.count_of(BOX), 0);
}

#[test]
fn an_unknown_entry_does_nothing_and_keeps_the_list() {
    let mut s = Scene::with_player(stock());
    s.bypass("multisell 1");
    s.log.take();
    s.choose(1, 123, 1);
    assert!(s.log.take().is_empty());
    assert!(s.open_list().is_some());
}

#[test]
fn a_non_stackable_entry_cannot_be_run_twice() {
    let mut s = Scene::with_player(stock());
    s.bypass("multisell 1");
    s.log.take();
    // Entry 2 gives a weapon: the list is closed, nothing is taken.
    s.choose(1, 200_000, 2);
    assert!(s.open_list().is_none());
    assert_eq!(s.count_of(AXE), 0);
    assert_eq!(s.count_of(SWORD), 3);
}

#[test]
fn without_enchantment_the_lowest_item_is_taken() {
    let mut s = Scene::with_player(stock());
    s.bypass("multisell 1");
    s.log.take();
    s.choose(1, 200_000, 1);
    let out = s.log.take();
    // The +0 sword (12) went, the +3 and +5 stay, a plain axe came.
    let items = s.items();
    assert!(!items.iter().any(|i| i.0 == 12));
    assert!(items.iter().any(|i| i.0 == 13) && items.iter().any(|i| i.0 == 14));
    let axe = s
        .world
        .player(A)
        .unwrap()
        .data
        .items
        .iter()
        .find(|i| i.item_id == AXE)
        .unwrap()
        .clone();
    assert_eq!(axe.enchant_level, 0);
    assert_eq!(axe.count, 1);
    assert_eq!(
        message_ids(&out),
        [
            SystemMessageId::S1HasDisappeared,
            SystemMessageId::YouHaveEarnedS1
        ]
    );
    // The row of the used sword is deleted (D-39).
    assert!(item_changes(&out).contains(&(vec![], vec![12])));

    // Again: now the +3 is the lowest.
    s.choose(1, 200_000, 1);
    let items = s.items();
    assert!(!items.iter().any(|i| i.0 == 13) && items.iter().any(|i| i.0 == 14));
}

#[test]
fn two_of_the_same_ingredient_are_merged_for_the_check() {
    let mut s = Scene::with_player(vec![inv(10, ADENA, 5000), inv(12, SWORD, 1)]);
    s.bypass("multisell 1");
    s.log.take();
    // Entry 6 wants two swords (two ingredients of one sword each) and the player has one.
    s.choose(1, 600_000, 1);
    let m = messages(&s.log.take());
    assert_eq!(m[0].id, SystemMessageId::S2UnitSOfTheItemS1IsAreRequired);
    assert_eq!(
        m[0].params,
        [
            SystemMessageParam::ItemName(SWORD),
            SystemMessageParam::LongNumber(2)
        ]
    );
    assert_eq!(s.count_of(SWORD), 1);
}

#[test]
fn a_list_that_keeps_enchantments_gives_the_enchantment_back() {
    let mut sword = enchanted(inv(14, SWORD, 1), 5);
    sword.augmentation = Some(77);
    sword.elementals = vec![ItemElemental {
        element: 0,
        value: 20,
    }];
    let mut s = Scene::with_player(vec![inv(10, ADENA, 100), sword]);
    s.bypass("exc_multisell 2");
    s.log.take();
    s.choose(2, 100_005, 1);
    let out = s.log.take();
    let items = s.world.player(A).unwrap().data.items.clone();
    assert!(!items.iter().any(|i| i.item_id == SWORD));
    let axe = items.iter().find(|i| i.item_id == AXE).unwrap();
    // The enchant level, the augmentation and the elements move to the new weapon.
    assert_eq!(axe.enchant_level, 5);
    assert_eq!(axe.augmentation, Some(77));
    assert_eq!(
        axe.elementals,
        [ItemElemental {
            element: 0,
            value: 20
        }]
    );
    assert_eq!(s.count_of(ADENA), 90);
    // "Acquired +5 Axe".
    let m = messages(&out);
    let last = m.last().unwrap();
    assert_eq!(last.id, SystemMessageId::AcquiredS1S2);
    assert_eq!(
        last.params,
        [
            SystemMessageParam::LongNumber(5),
            SystemMessageParam::ItemName(AXE)
        ]
    );
    // `product.updateDatabase()`: the new row is written with its enchant level.
    assert!(
        item_changes(&out)
            .iter()
            .any(|(up, _)| up.contains(&axe.object_id.0))
    );
}

#[test]
fn a_maintain_ingredient_stays_unless_recipes_are_used_up() {
    for (use_recipes, sword_left) in [(true, 0), (false, 1)] {
        let mut s = Scene::with_settings(MultisellSettings {
            alt_blacksmith_use_recipes: use_recipes,
            ..MultisellSettings::default()
        });
        s.enter(player(A, "Alice", vec![inv(10, ADENA, 100), inv(12, SWORD, 1)]));
        s.talk();
        s.bypass("multisell 7");
        s.log.take();
        s.choose(7, 100_000, 1);
        assert_eq!(
            s.count_of(SWORD),
            sword_left,
            "AltBlacksmithUseRecipes = {use_recipes}"
        );
        assert_eq!((s.count_of(ADENA), s.count_of(BOX)), (90, 1));
    }
}

#[test]
fn too_heavy_or_too_many_products_are_refused() {
    let mut s = Scene::with_player(stock());
    s.bypass("multisell 1");
    s.log.take();
    // Entry 5 gives one very heavy item.
    s.choose(1, 500_000, 1);
    assert_eq!(
        message_ids(&s.log.take()),
        [SystemMessageId::YouHaveExceededTheWeightLimit]
    );
    assert_eq!(s.count_of(HEAVY), 0);
    assert_eq!(s.count_of(ADENA), 5000);
    assert!(s.open_list().is_some());

    // 80 pairs of boots fill the inventory (80 slots): the axe has no room.
    let mut full = vec![inv(10, ADENA, 5000), inv(12, SWORD, 1)];
    full.extend((0..80).map(|n| inv(1000 + n, BOOTS, 1)));
    let mut s = Scene::with_player(full);
    s.bypass("multisell 1");
    s.log.take();
    s.choose(1, 200_000, 1);
    assert_eq!(message_ids(&s.log.take()), [SystemMessageId::YourInventoryIsFull]);
    assert_eq!(s.count_of(AXE), 0);
}

#[test]
fn leaving_the_npc_closes_the_list_for_everybody_but_a_gm() {
    let mut s = Scene::with_player(stock());
    s.bypass("multisell 1");
    s.log.take();
    s.world.set_position(A, X - 400, Y, 0);
    s.choose(1, 100_000, 1);
    assert!(s.open_list().is_none());
    assert_eq!(s.count_of(BOX), 0);

    // A list for every NPC is not checked.
    s.world.set_position(A, X, Y, 0);
    s.world.open_multisell(3, A, Some(s.npc), false);
    s.world.set_position(A, X - 400, Y, 0);
    s.choose(3, 100_000, 1);
    assert_eq!(s.count_of(BOX), 1);

    // A GM may go on, and is told.
    let mut gm = player(
        ObjectId(2),
        "Gamemaster",
        vec![inv(20, ADENA, 5000), inv(21, GEM, 20)],
    );
    gm.access_level = 100;
    s.enter(gm);
    let g = ObjectId(2);
    s.input(
        g,
        ClientInput::Action {
            object_id: s.npc.0,
            action_id: 0,
        },
    );
    s.world.open_multisell(1, g, Some(s.npc), false);
    s.world.set_position(g, X - 400, Y, 0);
    s.log.take();
    s.input(
        g,
        ClientInput::MultiSellChoose {
            list_id: 1,
            entry_id: 100_000,
            amount: 1,
        },
    );
    let out = s.log.take();
    assert!(message_ids(&out).contains(&SystemMessageId::YouHaveEarnedS2S1S));
}

// ---- special ingredients ------------------------------------------------------------------------

#[test]
fn fame_is_paid_and_earned_within_the_limits() {
    let mut data = player(A, "Alice", vec![inv(10, ADENA, 100)]);
    data.profile.fame = 120;
    let mut s = Scene::with_player_data(data);
    s.bypass("multisell 1");
    s.log.take();
    // Entry 3: 50 fame for a gem.
    s.choose(1, 300_000, 1);
    let out = s.log.take();
    assert_eq!(s.world.player(A).unwrap().data.profile.fame, 70);
    assert_eq!(s.count_of(GEM), 1);
    // `updateUserInfo`.
    assert!(names(&out).contains(&"USER_INFO".to_owned()));
    // Twice more: 70 -> 20, then not enough.
    s.choose(1, 300_000, 1);
    assert_eq!(s.world.player(A).unwrap().data.profile.fame, 20);
    s.log.take();
    s.choose(1, 300_000, 1);
    assert_eq!(
        message_ids(&s.log.take()),
        [SystemMessageId::YouDonTHaveEnoughReputationToDoThat]
    );
    assert_eq!(s.count_of(GEM), 2);

    // Entry 4: 30 fame for 10 adena; `MaxPersonalFamePoints` (100000) is the top.
    s.choose(1, 400_000, 1);
    assert_eq!(s.world.player(A).unwrap().data.profile.fame, 50);
    s.world.open_multisell(1, A, Some(s.npc), false);
    s.choose(1, 400_000, 1);
    assert_eq!(s.world.player(A).unwrap().data.profile.fame, 80);
    // Fame is not an item: no row is written for it.
    assert_eq!(s.count_of(-300), 0);
}

#[test]
fn fame_stops_at_the_maximum() {
    let mut data = player(A, "Alice", vec![inv(10, ADENA, 100)]);
    data.profile.fame = 99_990;
    let mut s = Scene::with_player_data(data);
    s.bypass("multisell 1");
    s.choose(1, 400_000, 1);
    assert_eq!(s.world.player(A).unwrap().data.profile.fame, 100_000);
}

const CLAN: i32 = 500;
const LEADER: ObjectId = ObjectId(1);
const MEMBER: ObjectId = ObjectId(2);

fn clan_record(reputation: i32) -> l2_world::ClanRecord {
    let member = |id: ObjectId, power_grade| ClanMemberRecord {
        object_id: id.0,
        name: format!("P{}", id.0),
        level: 40,
        power_grade,
        ..ClanMemberRecord::default()
    };
    l2_world::ClanRecord {
        clan_id: CLAN,
        data: Some(ClanDataRecord {
            name: Some("Lions".to_owned()),
            level: 5,
            reputation_score: reputation,
            leader_id: LEADER.0,
            ..ClanDataRecord::default()
        }),
        members: vec![member(LEADER, 1), member(MEMBER, 5)],
        rank_privs: vec![(5, 0, 0)],
        ..l2_world::ClanRecord::default()
    }
}

fn clan_player(id: ObjectId, power_grade: i32) -> PlayerData {
    let mut data = player(id, &format!("P{}", id.0), vec![inv(id.0 * 100, ADENA, 100)]);
    data.profile.clan = Some(PlayerClanState {
        clan_id: CLAN,
        power_grade,
        ..PlayerClanState::default()
    });
    data
}

#[test]
fn clan_reputation_is_paid_by_the_clan_leader_only() {
    let mut s = Scene::new();
    s.world.load_clans(vec![clan_record(150)], Vec::new());
    s.enter(clan_player(LEADER, 1));
    s.enter(clan_player(MEMBER, 5));
    s.talk();
    s.bypass("multisell 1");
    s.log.take();

    // The leader pays 100 of 150 for a box.
    s.choose(1, 700_000, 1);
    let out = s.log.take();
    assert_eq!(s.world.clan(CLAN).unwrap().reputation_score, 50);
    assert_eq!(s.count_of(BOX), 1);
    let m = messages(&out);
    assert_eq!(
        m[0].id,
        SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore
    );
    assert_eq!(m[0].params, [SystemMessageParam::LongNumber(100)]);

    // 50 left is too low.
    s.choose(1, 700_000, 1);
    assert_eq!(
        message_ids(&s.log.take()),
        [SystemMessageId::TheClanReputationScoreIsTooLow]
    );
    assert_eq!(s.count_of(BOX), 1);
}

#[test]
fn clan_reputation_needs_a_clan_and_its_leader() {
    let mut s = Scene::new();
    s.world.load_clans(vec![clan_record(1000)], Vec::new());
    // A member who is not the leader.
    s.enter(clan_player(MEMBER, 5));
    s.world.open_multisell(1, MEMBER, Some(s.npc), false);
    s.input(
        MEMBER,
        ClientInput::Action {
            object_id: s.npc.0,
            action_id: 0,
        },
    );
    s.input(
        MEMBER,
        ClientInput::Action {
            object_id: s.npc.0,
            action_id: 0,
        },
    );
    s.world.open_multisell(1, MEMBER, Some(s.npc), false);
    s.log.take();
    s.input(
        MEMBER,
        ClientInput::MultiSellChoose {
            list_id: 1,
            entry_id: 700_000,
            amount: 1,
        },
    );
    assert_eq!(
        message_ids(&s.log.take()),
        [SystemMessageId::OnlyTheClanLeaderIsEnabled]
    );
    assert_eq!(s.world.clan(CLAN).unwrap().reputation_score, 1000);

    // Nobody's clan.
    s.enter(player(ObjectId(3), "Loner", vec![]));
    let loner = ObjectId(3);
    for _ in 0..2 {
        s.input(
            loner,
            ClientInput::Action {
                object_id: s.npc.0,
                action_id: 0,
            },
        );
    }
    s.world.open_multisell(1, loner, Some(s.npc), false);
    s.log.take();
    s.input(
        loner,
        ClientInput::MultiSellChoose {
            list_id: 1,
            entry_id: 700_000,
            amount: 1,
        },
    );
    assert_eq!(
        message_ids(&s.log.take()),
        [SystemMessageId::YouAreNotAClanMemberAndCannotPerformThisAction]
    );
}
