#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Enchanting and element attributes (M6-ENCHANT-01): the scroll window, the enchant with its
//! success, failure, blessed and safe results, the support item, the announcements, the element
//! stones and their removal, and the over-enchant protection. The expected values come from the
//! Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, ElementalAttributeData, EnchantHpBonus, EnchantItemData, EnchantItemGroups, Hennas,
    ItemTemplates, PlayerTemplates, SkillTemplates,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    EnchantResult, ExAttributeEnchantResult, ExBaseAttributeCancelResult, ExChooseInventoryAttributeItem,
    ExPutEnchantTargetItemResult, ExShowBaseAttributeCancelWindow, InventoryUpdate, MagicSkillUse,
    SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
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

// Item ids.
const C_SWORD: i32 = 2;
const C_HELMET: i32 = 4;
const C_NECKLACE: i32 = 5;
const S_SWORD: i32 = 6;
const S_CUIRASS: i32 = 7;
const D_SWORD: i32 = 8;
const SCROLL_WEAPON_C: i32 = 951;
const SCROLL_ARMOR_C: i32 = 952;
const SCROLL_WEAPON_D: i32 = 955;
const SCROLL_BLESSED_WEAPON_C: i32 = 6573;
const SCROLL_ANCIENT_WEAPON_C: i32 = 22014;
const SUPPORT_WEAPON_C: i32 = 12364;
const FIRE_STONE: i32 = 9546;
const DARK_STONE: i32 = 9550;
const FIRE_CRYSTAL: i32 = 9552;
const ADENA: i32 = 57;
const C_CRYSTAL: i32 = 1459;

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="C Sword"><set name="crystal_type" val="C" /><set name="crystal_count" val="100" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="enchant_enabled" val="true" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat></stats></item>
<item id="3" type="Weapon" name="C Staff"><set name="crystal_type" val="C" /><set name="crystal_count" val="100" /><set name="weapon_type" val="BLUNT" /><set name="bodypart" val="lrhand" /><set name="is_magic_weapon" val="true" /><set name="enchant_enabled" val="true" /><set name="weight" val="50" /></item>
<item id="4" type="Armor" name="C Helmet"><set name="crystal_type" val="C" /><set name="crystal_count" val="40" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="head" /><set name="enchant_enabled" val="true" /><set name="weight" val="10" />
<skills><skill id="3007" level="1" type="ON_ENCHANT_4" /></skills></item>
<item id="5" type="Armor" name="C Necklace"><set name="crystal_type" val="C" /><set name="crystal_count" val="30" /><set name="bodypart" val="neck" /><set name="enchant_enabled" val="true" /><set name="weight" val="5" /></item>
<item id="6" type="Weapon" name="S Sword"><set name="crystal_type" val="S" /><set name="crystal_count" val="200" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="enchant_enabled" val="true" /><set name="element_enabled" val="true" /><set name="weight" val="50" /></item>
<item id="7" type="Armor" name="S Cuirass"><set name="crystal_type" val="S" /><set name="crystal_count" val="200" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="enchant_enabled" val="true" /><set name="element_enabled" val="true" /><set name="weight" val="10" /></item>
<item id="8" type="Weapon" name="D Sword"><set name="crystal_type" val="D" /><set name="crystal_count" val="20" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="enchant_enabled" val="true" /><set name="element_enabled" val="true" /><set name="weight" val="50" /></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="1459" type="EtcItem" name="Crystal: C Grade"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="951" type="EtcItem" name="Scroll: Enchant Weapon (C-grade)"><set name="etcitem_type" val="SCRL_ENCHANT_WP" /><set name="handler" val="EnchantScrolls" /><set name="is_stackable" val="true" /></item>
<item id="952" type="EtcItem" name="Scroll: Enchant Armor (C-grade)"><set name="etcitem_type" val="SCRL_ENCHANT_AM" /><set name="handler" val="EnchantScrolls" /><set name="is_stackable" val="true" /></item>
<item id="955" type="EtcItem" name="Scroll: Enchant Weapon (D-grade)"><set name="etcitem_type" val="SCRL_ENCHANT_WP" /><set name="handler" val="EnchantScrolls" /><set name="is_stackable" val="true" /></item>
<item id="6573" type="EtcItem" name="Blessed Scroll: Enchant Weapon (C-grade)"><set name="etcitem_type" val="BLESS_SCRL_ENCHANT_WP" /><set name="handler" val="EnchantScrolls" /><set name="is_stackable" val="true" /></item>
<item id="22014" type="EtcItem" name="Ancient Crystal: Enchant Weapon (C-grade)"><set name="etcitem_type" val="ANCIENT_CRYSTAL_ENCHANT_WP" /><set name="handler" val="EnchantScrolls" /><set name="is_stackable" val="true" /></item>
<item id="12364" type="EtcItem" name="Enchant rate booster (C-grade)"><set name="etcitem_type" val="SCRL_INC_ENCHANT_PROP_WP" /><set name="is_stackable" val="true" /></item>
<item id="9546" type="EtcItem" name="Fire Stone"><set name="etcitem_type" val="SCRL_ENCHANT_ATTR" /><set name="handler" val="EnchantAttribute" /><set name="is_stackable" val="true" /></item>
<item id="9547" type="EtcItem" name="Water Stone"><set name="etcitem_type" val="SCRL_ENCHANT_ATTR" /><set name="handler" val="EnchantAttribute" /><set name="is_stackable" val="true" /></item>
<item id="9550" type="EtcItem" name="Dark Stone"><set name="etcitem_type" val="SCRL_ENCHANT_ATTR" /><set name="handler" val="EnchantAttribute" /><set name="is_stackable" val="true" /></item>
<item id="9552" type="EtcItem" name="Fire Crystal"><set name="etcitem_type" val="SCRL_ENCHANT_ATTR" /><set name="handler" val="EnchantAttribute" /><set name="is_stackable" val="true" /></item>
</list>"#;

const SKILLS: &str = r##"<list>
<skill id="3007" levels="1" name="Passive"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="critRate" val="11"/></effect></effects></skill>
<skill id="5965" levels="1" name="Firework"><operateType>A1</operateType><targetType>SELF</targetType><hitTime>500</hitTime><reuseDelay>1000</reuseDelay></skill>
</list>"##;

/// The scrolls of the shipped `EnchantItemData.xml`, in the form the tests need. 22014 is a safe
/// scroll (an ancient crystal), 6573 a blessed one.
const ENCHANT_DATA: &str = r#"<list>
<enchant id="951" targetGrade="C" maxEnchant="16" />
<enchant id="952" targetGrade="C" maxEnchant="16" />
<enchant id="955" targetGrade="D" maxEnchant="16" />
<enchant id="6573" targetGrade="C" maxEnchant="16" />
<enchant id="22014" targetGrade="C" maxEnchant="16" />
<support id="12364" targetGrade="C" maxEnchant="9" bonusRate="15" />
</list>"#;

/// Armor: 100 % up to +5, then 0. Weapon: 100 % up to +2, 0 at +3, 100 % at +4 to +6, 66 % from +7.
const GROUPS: &str = r#"<list>
<enchantRateGroup name="ARMOR_GROUP"><current enchant="0-5" chance="100" /><current enchant="6-15" chance="0" /></enchantRateGroup>
<enchantRateGroup name="FIGHTER_WEAPON_GROUP"><current enchant="0-2" chance="100" /><current enchant="3" chance="0" /><current enchant="4-6" chance="100" /><current enchant="7-15" chance="66" /></enchantRateGroup>
<enchantRateGroup name="MAGE_WEAPON_GROUP"><current enchant="0-15" chance="100" /></enchantRateGroup>
<enchantScrollGroup id="0">
<enchantRate group="ARMOR_GROUP"><item slot="head" /><item slot="chest" /><item slot="neck" /></enchantRate>
<enchantRate group="FIGHTER_WEAPON_GROUP"><item slot="rhand" magicWeapon="false" /><item slot="lrhand" magicWeapon="false" /></enchantRate>
<enchantRate group="MAGE_WEAPON_GROUP"><item slot="rhand" magicWeapon="true" /><item slot="lrhand" magicWeapon="true" /></enchantRate>
</enchantScrollGroup>
</list>"#;

const ELEMENTS: &str = r#"<list>
<item id="9546" elemental="FIRE" type="STONE" />
<item id="9547" elemental="WATER" type="STONE" />
<item id="9550" elemental="DARK" type="STONE" />
<item id="9552" elemental="FIRE" type="CRYSTAL" />
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
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.stats
            .skills
            .iter()
            .any(|s| s.id == skill_id && s.is_passive())
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

fn data(items: &Arc<ItemTemplates>) -> Data {
    let stats = StatData {
        items: Arc::clone(items),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    };
    Data {
        items: Arc::clone(items),
        stats,
    }
}

// ---- helpers ------------------------------------------------------------------------------------

const A: ObjectId = ObjectId(1);
const B: ObjectId = ObjectId(2);

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

fn worn(object_id: i32, item_id: i32, slot: i32) -> ItemData {
    let mut item = inv(object_id, item_id, 1);
    item.loc = "PAPERDOLL".into();
    item.loc_data = slot;
    item
}

fn with_element(mut item: ItemData, element: i32, value: i32) -> ItemData {
    item.elementals.push(ItemElemental { element, value });
    item
}

fn player(id: ObjectId, name: &str, x: i32, access_level: i32, items: Vec<ItemData>) -> PlayerData {
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
        access_level,
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
    /// A world with the enchant data of the constants above. `settings` may change the config.
    fn with_settings(settings: impl FnOnce(&mut EnchantSettings)) -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
        let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
        world.set_static_data(Box::new(data(&items)));
        let mut enchant = EnchantSettings::default();
        settings(&mut enchant);
        world.set_settings(WorldSettings {
            enchant,
            ..WorldSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_enchant_data(
            Arc::new(EnchantItemData::from_xml(ENCHANT_DATA, &items).unwrap()),
            Arc::new(EnchantItemGroups::from_xml(GROUPS).unwrap()),
            Arc::new(ElementalAttributeData::from_xml(ELEMENTS, &items).unwrap()),
        );
        Self { world, log }
    }

    fn new() -> Self {
        Self::with_settings(|_| {})
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

    /// Alice with `items`, and nobody else; what the enter sent is dropped.
    fn alone(items: Vec<ItemData>) -> Self {
        let mut s = Self::new();
        s.enter(player(A, "Alice", 1000, 0, items));
        s.log.take();
        s
    }

    fn input(&mut self, who: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input,
        });
    }

    fn enchant(&mut self, who: ObjectId, input: EnchantInput) {
        self.input(who, ClientInput::Enchant(input));
    }

    fn use_item(&mut self, who: ObjectId, object_id: i32) {
        self.input(
            who,
            ClientInput::UseItem {
                object_id,
                ctrl_pressed: false,
            },
        );
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    /// Alice uses `scroll`, puts `item` in and waits out the auto enchant check.
    fn open(&mut self, scroll: i32, item: i32) {
        self.use_item(A, scroll);
        self.enchant(A, EnchantInput::PutTargetItem { object_id: item });
        self.advance(2000);
        self.log.take();
    }

    fn press(&mut self, item: i32) {
        self.enchant(A, EnchantInput::Enchant { object_id: item });
    }

    fn item(&self, who: ObjectId, object_id: i32) -> Option<ItemData> {
        self.world
            .player(who)?
            .data
            .items
            .iter()
            .find(|i| i.object_id == ObjectId(object_id))
            .cloned()
    }

    fn count_of(&self, who: ObjectId, item_id: i32) -> i64 {
        self.world
            .player(who)
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id)
            .map(|i| i.count)
            .sum()
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

fn results(out: &[Out], to: ObjectId) -> Vec<(i32, i32, i64)> {
    of(out, to, "ENCHANT_RESULT")
        .iter()
        .map(|raw| {
            let r: EnchantResult = decode(raw);
            (r.result, r.crystal, r.count)
        })
        .collect()
}

/// `(change, object id, count, enchant)` of every entry of every `InventoryUpdate`.
fn updates(out: &[Out], to: ObjectId) -> Vec<(i16, i32, i64, i16)> {
    of(out, to, "INVENTORY_UPDATE")
        .iter()
        .flat_map(|raw| decode::<InventoryUpdate>(raw).updates)
        .map(|u| (u.change, u.item.object_id, u.item.count, u.item.enchant))
        .collect()
}

/// `(owner, upserted object ids, removed object ids)` of every `ItemChanges`.
fn item_changes(out: &[Out]) -> Vec<(i32, Vec<i32>, Vec<i32>)> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemChanges {
                owner,
                upserts,
                removed,
            }) => Some((
                owner.0,
                upserts.iter().map(|i| i.object_id.0).collect(),
                removed.iter().map(|o| o.0).collect(),
            )),
            _ => None,
        })
        .collect()
}

const SWORD: i32 = 100;
const SCROLL: i32 = 200;

/// A C-grade sword at `level` and a stack of 5 scrolls.
fn sword_scene(level: i32, scroll_item: i32) -> Scene {
    Scene::alone(vec![
        enchanted(inv(SWORD, C_SWORD, 1), level),
        inv(SCROLL, scroll_item, 5),
    ])
}

// ---- the scroll window --------------------------------------------------------------------------

#[test]
fn using_a_scroll_opens_the_window_and_a_second_one_waits_for_the_item() {
    // Java `EnchantScrolls.onItemUse`: setActiveEnchantItemId, ChooseInventoryItem(item.getId()),
    // SELECT_ITEM_TO_ENCHANT.
    let mut s = sword_scene(0, SCROLL_WEAPON_C);
    s.use_item(A, SCROLL);
    let out = s.log.take();
    let choose: l2_protocol_game::server::ChooseInventoryItem =
        decode(of(&out, A, "CHOOSE_INVENTORY_ITEM")[0]);
    assert_eq!(choose.item_id, SCROLL_WEAPON_C);
    assert_eq!(message_ids(&out, A), [SystemMessageId::SelectItemToEnchant]);
    assert_eq!(s.world.active_enchant_item(A), Some(ObjectId(SCROLL)));
    assert!(!s.world.is_enchanting(A));

    // Before the item is put in the window another click only reopens it.
    s.use_item(A, SCROLL);
    assert_eq!(names(&s.log.take(), A).len(), 2);

    // After it, another scroll waits: ANOTHER_ENCHANTMENT_IS_IN_PROGRESS.
    s.enchant(A, EnchantInput::PutTargetItem { object_id: SWORD });
    assert!(s.world.is_enchanting(A));
    s.log.take();
    s.use_item(A, SCROLL);
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::AnotherEnchantmentIsInProgressPleaseCompleteThePreviousTaskThenTryAgain]
    );
}

#[test]
fn the_item_put_in_must_fit_the_scroll() {
    let mut s = Scene::alone(vec![
        inv(SWORD, C_SWORD, 1),
        inv(101, C_HELMET, 1),
        inv(SCROLL, SCROLL_WEAPON_C, 1),
    ]);
    // A weapon scroll takes the sword: the answer is its object id.
    s.use_item(A, SCROLL);
    s.log.take();
    s.enchant(A, EnchantInput::PutTargetItem { object_id: SWORD });
    let out = s.log.take();
    assert_eq!(
        decode::<ExPutEnchantTargetItemResult>(of(&out, A, "EX_PUT_ENCHANT_TARGET_ITEM_RESULT")[0]).result,
        SWORD
    );
    assert!(s.world.is_enchanting(A));

    // Putting it in again does nothing (`isEnchanting`).
    s.enchant(A, EnchantInput::PutTargetItem { object_id: 101 });
    assert!(s.log.take().is_empty());

    // The helmet does not fit: the message, a 0 and the scroll is no longer in use.
    s.enchant(A, EnchantInput::CancelEnchant);
    s.use_item(A, SCROLL);
    s.log.take();
    s.enchant(A, EnchantInput::PutTargetItem { object_id: 101 });
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::DoesNotFitStrengtheningConditionsOfTheScroll]
    );
    assert_eq!(
        decode::<ExPutEnchantTargetItemResult>(of(&out, A, "EX_PUT_ENCHANT_TARGET_ITEM_RESULT")[0]).result,
        0
    );
    assert_eq!(s.world.active_enchant_item(A), None);
    assert!(!s.world.is_enchanting(A));
}

#[test]
fn an_object_id_the_player_does_not_have_is_an_illegal_action() {
    let mut s = sword_scene(0, SCROLL_WEAPON_C);
    s.use_item(A, SCROLL);
    s.log.take();
    // RequestExTryToPutEnchantTargetItem: handleIllegalPlayerAction (the default punishment: KICK).
    s.enchant(A, EnchantInput::PutTargetItem { object_id: 987_654 });
    let texts = messages_text(&s.log.take(), A);
    assert_eq!(texts, ["You will be kicked for illegal action, GM informed."]);
    // No scroll in use: the packet is ignored.
    s.enchant(A, EnchantInput::CancelEnchant);
    s.log.take();
    s.enchant(A, EnchantInput::PutTargetItem { object_id: 987_654 });
    assert!(s.log.take().is_empty());
}

/// The `sendMessage` texts (a `SystemMessage` of the id `S12` with one string).
fn messages_text(out: &[Out], to: ObjectId) -> Vec<String> {
    messages(out, to)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::S12)
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(text)) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

#[test]
fn the_support_item_must_fit_the_scroll_and_the_item() {
    let mut s = Scene::alone(vec![
        inv(SWORD, C_SWORD, 1),
        inv(SCROLL, SCROLL_WEAPON_C, 1),
        inv(201, SCROLL_BLESSED_WEAPON_C, 1),
        inv(202, SUPPORT_WEAPON_C, 1),
    ]);
    // Without the item in the window the support packet is ignored.
    s.enchant(
        A,
        EnchantInput::PutSupportItem {
            support_object_id: 202,
            enchant_object_id: SWORD,
        },
    );
    assert!(s.log.take().is_empty());

    s.use_item(A, SCROLL);
    s.enchant(A, EnchantInput::PutTargetItem { object_id: SWORD });
    s.log.take();
    s.enchant(
        A,
        EnchantInput::PutSupportItem {
            support_object_id: 202,
            enchant_object_id: SWORD,
        },
    );
    let out = s.log.take();
    let accepted: l2_protocol_game::server::ExPutEnchantSupportItemResult =
        decode(of(&out, A, "EX_PUT_ENCHANT_SUPPORT_ITEM_RESULT")[0]);
    assert_eq!(accepted.result, 202);
    assert_eq!(
        s.world.enchant_state(A).unwrap().active_support,
        Some(ObjectId(202))
    );

    // An object the player does not have: INAPPROPRIATE_ENCHANT_CONDITIONS and no support item.
    s.enchant(
        A,
        EnchantInput::PutSupportItem {
            support_object_id: 999,
            enchant_object_id: SWORD,
        },
    );
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::InappropriateEnchantConditions]
    );
    assert!(of(&out, A, "EX_PUT_ENCHANT_SUPPORT_ITEM_RESULT").is_empty());
    assert_eq!(s.world.enchant_state(A).unwrap().active_support, None);

    // A blessed scroll takes no support item: the message and a 0.
    s.enchant(A, EnchantInput::CancelEnchant);
    s.use_item(A, 201);
    s.enchant(A, EnchantInput::PutTargetItem { object_id: SWORD });
    s.log.take();
    s.enchant(
        A,
        EnchantInput::PutSupportItem {
            support_object_id: 202,
            enchant_object_id: SWORD,
        },
    );
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::InappropriateEnchantConditions]
    );
    let refused: l2_protocol_game::server::ExPutEnchantSupportItemResult =
        decode(of(&out, A, "EX_PUT_ENCHANT_SUPPORT_ITEM_RESULT")[0]);
    assert_eq!(refused.result, 0);
}

#[test]
fn cancelling_closes_the_window_with_an_error_result() {
    let mut s = sword_scene(0, SCROLL_WEAPON_C);
    s.use_item(A, SCROLL);
    s.enchant(A, EnchantInput::PutTargetItem { object_id: SWORD });
    s.log.take();
    s.enchant(A, EnchantInput::CancelEnchant);
    assert_eq!(results(&s.log.take(), A), [(2, 0, 0)]);
    assert_eq!(s.world.active_enchant_item(A), None);
    assert!(!s.world.is_enchanting(A));
}

// ---- the enchant --------------------------------------------------------------------------------

#[test]
fn a_successful_enchant_uses_the_scroll_and_raises_the_level() {
    let mut s = sword_scene(0, SCROLL_WEAPON_C);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    let out = s.log.take();

    assert_eq!(s.item(A, SWORD).unwrap().enchant_level, 1);
    assert_eq!(s.count_of(A, SCROLL_WEAPON_C), 4);
    // EnchantResult(0), then the InventoryUpdate (scroll modified, item modified) and the user info.
    assert_eq!(results(&out, A), [(0, 0, 0)]);
    let mut changes = updates(&out, A);
    changes.sort_unstable();
    assert_eq!(changes, [(2, SWORD, 1, 1), (2, SCROLL, 4, 0)]);
    assert!(!of(&out, A, "USER_INFO").is_empty());
    // The item row is written at once (`item.updateDatabase()`); the scroll count is not.
    assert!(item_changes(&out).iter().any(|c| c.1 == [SWORD]));
    // The window is closed.
    assert_eq!(s.world.active_enchant_item(A), None);
    assert!(!s.world.is_enchanting(A));
}

#[test]
fn the_last_scroll_is_removed_from_the_list() {
    let mut s = Scene::alone(vec![inv(SWORD, C_SWORD, 1), inv(SCROLL, SCROLL_WEAPON_C, 1)]);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    let out = s.log.take();
    let mut changes = updates(&out, A);
    changes.sort_unstable();
    // `iu.addRemovedItem(scroll)` (change 3), the sword modified.
    assert_eq!(changes, [(2, SWORD, 1, 1), (3, SCROLL, 0, 0)]);
    assert!(s.item(A, SCROLL).is_none());
    assert!(item_changes(&out).iter().any(|c| c.2 == [SCROLL]));
}

#[test]
fn pressing_enchant_less_than_two_seconds_after_the_item_is_an_auto_enchant_attempt() {
    let mut s = sword_scene(0, SCROLL_WEAPON_C);
    s.use_item(A, SCROLL);
    s.enchant(A, EnchantInput::PutTargetItem { object_id: SWORD });
    s.advance(1950);
    s.log.take();
    s.press(SWORD);
    let out = s.log.take();
    // The illegal action message, EnchantResult(2), nothing used up.
    assert_eq!(
        messages_text(&out, A),
        ["You will be kicked for illegal action, GM informed."]
    );
    assert_eq!(results(&out, A), [(2, 0, 0)]);
    assert_eq!(s.count_of(A, SCROLL_WEAPON_C), 5);
    assert_eq!(s.item(A, SWORD).unwrap().enchant_level, 0);
    assert_eq!(s.world.active_enchant_item(A), None);

    // Exactly two seconds is fine.
    s.use_item(A, SCROLL);
    s.enchant(A, EnchantInput::PutTargetItem { object_id: SWORD });
    s.advance(2000);
    s.log.take();
    s.press(SWORD);
    assert_eq!(results(&s.log.take(), A), [(0, 0, 0)]);
}

#[test]
fn pressing_enchant_without_the_window_cancels_it() {
    let mut s = sword_scene(0, SCROLL_WEAPON_C);
    s.press(SWORD);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::YouHaveCancelledTheEnchantingProcess]
    );
    assert_eq!(results(&out, A), [(2, 0, 0)]);

    // An item the player does not have, with a scroll open: the same.
    s.use_item(A, SCROLL);
    s.log.take();
    s.press(12345);
    assert_eq!(results(&s.log.take(), A), [(2, 0, 0)]);
    assert_eq!(s.world.active_enchant_item(A), None);
}

#[test]
fn a_scroll_that_no_longer_fits_the_item_is_refused() {
    // The D scroll was opened for a sword that has become a C sword's twin: use the wrong grade
    // directly. `isValid` fails at the first check of `RequestEnchantItem`.
    let mut s = Scene::alone(vec![inv(SWORD, C_SWORD, 1), inv(SCROLL, SCROLL_WEAPON_D, 1)]);
    s.use_item(A, SCROLL);
    s.advance(2000);
    s.log.take();
    s.press(SWORD);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::InappropriateEnchantConditions]
    );
    assert_eq!(results(&out, A), [(2, 0, 0)]);
    assert_eq!(s.count_of(A, SCROLL_WEAPON_D), 1);
}

#[test]
fn a_failed_enchant_destroys_the_item_and_pays_crystals() {
    // +3 has chance 0. Java: item destroyed, crystals = getCrystalCount() - (crystalCount + 1) / 2
    // = (100 + 45 * 3) - 50 = 185 C-grade crystals, EnchantResult(1, crystal, count).
    let mut s = sword_scene(3, SCROLL_WEAPON_C);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    let out = s.log.take();

    assert!(s.item(A, SWORD).is_none());
    assert_eq!(s.count_of(A, C_CRYSTAL), 185);
    assert_eq!(s.count_of(A, SCROLL_WEAPON_C), 4);
    assert_eq!(results(&out, A), [(1, C_CRYSTAL, 185)]);
    // YOU_HAVE_EARNED_S2_S1_S: the crystal and the count.
    let earned = messages(&out, A)
        .into_iter()
        .find(|m| m.id == SystemMessageId::YouHaveEarnedS2S1S)
        .unwrap();
    assert_eq!(
        earned.params,
        [
            SystemMessageParam::ItemName(C_CRYSTAL),
            SystemMessageParam::LongNumber(185)
        ]
    );
    // The sword is removed (change 3), the new crystals are an addition (change 1).
    let changes = updates(&out, A);
    assert!(changes.contains(&(3, SWORD, 0, 3)));
    assert!(changes.iter().any(|c| c.0 == 1 && c.2 == 185));
    assert!(item_changes(&out).iter().any(|c| c.2.contains(&SWORD)));
}

#[test]
fn crystals_stack_onto_the_ones_the_player_has() {
    let mut s = Scene::alone(vec![
        enchanted(inv(SWORD, C_SWORD, 1), 3),
        inv(SCROLL, SCROLL_WEAPON_C, 1),
        inv(300, C_CRYSTAL, 10),
    ]);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    let out = s.log.take();
    assert_eq!(s.count_of(A, C_CRYSTAL), 195);
    // Reported as modified (change 2), not added.
    assert!(updates(&out, A).contains(&(2, 300, 195, 0)));
}

#[test]
fn armor_crystals_follow_the_armor_formula() {
    // The helmet at +5 has a 100 % chance: it succeeds.
    let mut s = Scene::alone(vec![
        enchanted(inv(SWORD, C_HELMET, 1), 5),
        inv(SCROLL, SCROLL_ARMOR_C, 1),
    ]);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    assert_eq!(s.item(A, SWORD).unwrap().enchant_level, 6);

    // At +6 the chance is 0: it breaks. Java `getCrystalCount(6)` for armor is
    // 40 + 6 * (3 * 6 - 6) = 112, minus (40 + 1) / 2 = 20, so 92 crystals.
    let mut s = Scene::alone(vec![
        enchanted(inv(SWORD, C_HELMET, 1), 6),
        inv(SCROLL, SCROLL_ARMOR_C, 1),
    ]);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    assert_eq!(s.count_of(A, C_CRYSTAL), 92);
}

#[test]
fn a_blessed_scroll_keeps_the_item_at_level_zero() {
    let mut s = sword_scene(3, SCROLL_BLESSED_WEAPON_C);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::TheBlessedEnchantFailedTheEnchantValueOfTheItemBecame0]
    );
    assert_eq!(results(&out, A), [(3, 0, 0)]);
    assert_eq!(s.item(A, SWORD).unwrap().enchant_level, 0);
    assert_eq!(s.count_of(A, C_CRYSTAL), 0);
    assert!(updates(&out, A).contains(&(2, SWORD, 1, 0)));
    assert!(item_changes(&out).iter().any(|c| c.1 == [SWORD]));
}

#[test]
fn a_safe_scroll_keeps_the_level() {
    let mut s = sword_scene(3, SCROLL_ANCIENT_WEAPON_C);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::EnchantFailedTheEnchantLevelForTheCorrespondingItemWillBeExactlyRetained]
    );
    assert_eq!(results(&out, A), [(5, 0, 0)]);
    assert_eq!(s.item(A, SWORD).unwrap().enchant_level, 3);
    assert_eq!(s.count_of(A, SCROLL_ANCIENT_WEAPON_C), 4);
}

#[test]
fn a_worn_item_comes_off_when_the_enchant_fails() {
    let mut s = Scene::alone(vec![
        enchanted(worn(SWORD, C_SWORD, 7), 3),
        inv(SCROLL, SCROLL_BLESSED_WEAPON_C, 1),
    ]);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    let out = s.log.take();
    // THE_EQUIPMENT_S1_S2_HAS_BEEN_REMOVED: the level and the item.
    let removed = messages(&out, A)
        .into_iter()
        .find(|m| m.id == SystemMessageId::TheEquipmentS1S2HasBeenRemoved)
        .unwrap();
    assert_eq!(
        removed.params,
        [
            SystemMessageParam::IntNumber(3),
            SystemMessageParam::ItemName(C_SWORD)
        ]
    );
    assert_eq!(s.item(A, SWORD).unwrap().loc, "INVENTORY");
    // The unequip update goes out on its own, before the result update.
    let unequip = of(&out, A, "INVENTORY_UPDATE")
        .into_iter()
        .map(decode::<InventoryUpdate>)
        .find(|u| {
            u.updates.len() == 1 && u.updates[0].item.object_id == SWORD && u.updates[0].item.equipped == 0
        });
    assert!(unequip.is_some());
}

#[test]
fn a_safe_scroll_takes_nothing_off() {
    let mut s = Scene::alone(vec![
        enchanted(worn(SWORD, C_SWORD, 7), 3),
        inv(SCROLL, SCROLL_ANCIENT_WEAPON_C, 1),
    ]);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    assert_eq!(s.item(A, SWORD).unwrap().loc, "PAPERDOLL");
    assert_eq!(s.item(A, SWORD).unwrap().enchant_level, 3);
}

#[test]
fn an_equipped_item_gets_the_full_item_list_after_a_success() {
    let mut s = Scene::alone(vec![worn(SWORD, C_SWORD, 7), inv(SCROLL, SCROLL_WEAPON_C, 1)]);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    s.log.take();
    // `player.sendItemList(false)` is 250 ms later.
    s.advance(300);
    assert!(!of(&s.log.take(), A, "ITEM_LIST").is_empty());
}

#[test]
fn the_dice_are_one_next_double_times_100_against_the_chance() {
    // +7 has a 66 % chance. `100 * Rnd.nextDouble() < chance`: replay the same seed.
    let mut expected = WorldRng::from_seed(1);
    let mut s = Scene::alone(vec![
        enchanted(inv(SWORD, C_SWORD, 1), 7),
        inv(SCROLL, SCROLL_WEAPON_C, 5),
    ]);
    for _ in 0..4 {
        let roll = 100.0 * expected.next_f64();
        let level = s.item(A, SWORD).map(|i| i.enchant_level);
        let Some(level) = level else { break };
        s.open(SCROLL, SWORD);
        s.press(SWORD);
        let out = s.log.take();
        let chance = if level >= 7 { 66.0 } else { 100.0 };
        let success = roll < chance;
        assert_eq!(results(&out, A)[0].0 == 0, success, "level {level}");
        if !success {
            break;
        }
        assert_eq!(s.item(A, SWORD).unwrap().enchant_level, level + 1);
    }
}

#[test]
fn a_support_item_adds_its_rate_and_is_used_up() {
    // +7 is 66 %; the support item adds 15: 81 %. Replay the seed to find a roll in between.
    let mut seed = 0u64;
    let roll = loop {
        seed += 1;
        let roll = 100.0 * WorldRng::from_seed(seed).next_f64();
        if (66.0..81.0).contains(&roll) {
            break roll;
        }
    };
    let log = Log::default();
    let mut s = Scene::new();
    s.log = log;
    // The scene's world has seed 1: rebuild it with the seed found.
    let mut world = World::new(WorldRng::from_seed(seed), Box::new(RecOutbox(s.log.clone())));
    let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
    world.set_static_data(Box::new(data(&items)));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_enchant_data(
        Arc::new(EnchantItemData::from_xml(ENCHANT_DATA, &items).unwrap()),
        Arc::new(EnchantItemGroups::from_xml(GROUPS).unwrap()),
        Arc::new(ElementalAttributeData::from_xml(ELEMENTS, &items).unwrap()),
    );
    s.world = world;
    s.enter(player(
        A,
        "Alice",
        1000,
        0,
        vec![
            enchanted(inv(SWORD, C_SWORD, 1), 7),
            inv(SCROLL, SCROLL_WEAPON_C, 1),
            inv(201, SUPPORT_WEAPON_C, 1),
        ],
    ));
    s.use_item(A, SCROLL);
    s.enchant(A, EnchantInput::PutTargetItem { object_id: SWORD });
    s.enchant(
        A,
        EnchantInput::PutSupportItem {
            support_object_id: 201,
            enchant_object_id: SWORD,
        },
    );
    s.advance(2000);
    s.log.take();
    s.press(SWORD);
    let out = s.log.take();
    assert!(roll >= 66.0);
    // With the support item the roll below 81 succeeds.
    assert_eq!(results(&out, A), [(0, 0, 0)]);
    assert_eq!(s.item(A, SWORD).unwrap().enchant_level, 8);
    // Both the scroll and the support item are gone.
    assert!(s.item(A, SCROLL).is_none() && s.item(A, 201).is_none());
    let changes = updates(&out, A);
    assert!(changes.contains(&(3, SCROLL, 0, 0)) && changes.contains(&(3, 201, 0, 0)));
}

#[test]
fn a_weapon_plus_seven_is_announced_with_fireworks_and_armor_at_plus_six() {
    // Alice's +6 sword becomes +7: the others in sight and Alice get C1_HAS_SUCCESSFULLY_ENCHANTED.
    let mut s = Scene::new();
    s.enter(player(
        A,
        "Alice",
        1000,
        0,
        vec![
            enchanted(inv(SWORD, C_SWORD, 1), 6),
            inv(SCROLL, SCROLL_WEAPON_C, 1),
        ],
    ));
    s.enter(player(B, "Bob", 1050, 0, vec![]));
    s.advance(100);
    s.log.take();
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    let out = s.log.take();
    for who in [A, B] {
        let announce: Vec<SystemMessage> = messages(&out, who)
            .into_iter()
            .filter(|m| m.id == SystemMessageId::C1HasSuccessfullyEnchantedAS2S3)
            .collect();
        assert_eq!(announce.len(), 1, "{who:?}");
        assert_eq!(
            announce[0].params,
            [
                SystemMessageParam::Text("Alice".into()),
                SystemMessageParam::IntNumber(7),
                SystemMessageParam::ItemName(C_SWORD)
            ]
        );
        // The firework: skill 5965 level 1 with the skill's hit time and reuse delay.
        let firework: MagicSkillUse = decode(of(&out, who, "MAGIC_SKILL_USE")[0]);
        assert_eq!(
            (
                firework.skill_id,
                firework.skill_level,
                firework.hit_time,
                firework.reuse_delay
            ),
            (5965, 1, 500, 1000)
        );
    }

    // Armor: +6, not +7.
    let mut s = Scene::alone(vec![
        enchanted(inv(SWORD, C_HELMET, 1), 4),
        inv(SCROLL, SCROLL_ARMOR_C, 3),
    ]);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    assert!(
        !messages(&s.log.take(), A)
            .iter()
            .any(|m| m.id == SystemMessageId::C1HasSuccessfullyEnchantedAS2S3)
    );
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    let out = s.log.take();
    assert_eq!(s.item(A, SWORD).unwrap().enchant_level, 6);
    assert!(
        messages(&out, A)
            .iter()
            .any(|m| m.id == SystemMessageId::C1HasSuccessfullyEnchantedAS2S3)
    );
}

#[test]
fn worn_armor_that_reaches_plus_four_gets_its_enchant_skill() {
    let mut s = Scene::alone(vec![
        enchanted(worn(SWORD, C_HELMET, 1), 3),
        inv(SCROLL, SCROLL_ARMOR_C, 1),
    ]);
    assert_eq!(s.world.stats(A).unwrap().skill_level(3007), 0);
    s.open(SCROLL, SWORD);
    s.press(SWORD);
    assert_eq!(s.item(A, SWORD).unwrap().enchant_level, 4);
    assert_eq!(s.world.stats(A).unwrap().skill_level(3007), 1);
    // `player.sendSkillList()` is 300 ms later.
    s.log.take();
    s.advance(400);
    assert!(!of(&s.log.take(), A, "SKILL_LIST").is_empty());
}

#[test]
fn the_scroll_in_use_cannot_be_dropped_and_blocks_logout_and_restart() {
    let mut s = Scene::alone(vec![inv(SWORD, C_SWORD, 1), inv(SCROLL, SCROLL_WEAPON_C, 3)]);
    s.use_item(A, SCROLL);
    s.log.take();

    // RequestDropItem: validateItemManipulation refuses the scroll in use (ITEM_CANNOT_BE_DISCARDED).
    s.input(
        A,
        ClientInput::DropItem {
            object_id: SCROLL,
            count: 1,
            x: 1000,
            y: 1000,
            z: 0,
        },
    );
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::ThisItemCannotBeDiscarded]
    );
    assert_eq!(s.count_of(A, SCROLL_WEAPON_C), 3);

    // Logout: ActionFailed. Restart: RestartResponse(false) and ActionFailed.
    s.input(A, ClientInput::Logout);
    assert_eq!(names(&s.log.take(), A), ["ACTION_FAIL"]);
    s.input(A, ClientInput::Restart);
    assert_eq!(names(&s.log.take(), A), ["RESTART_RESPONSE", "ACTION_FAIL"]);
    assert!(s.world.player(A).is_some());

    // After cancelling the player can leave.
    s.enchant(A, EnchantInput::CancelEnchant);
    s.input(A, ClientInput::Logout);
    assert!(s.world.player(A).is_none());
}

#[test]
fn using_up_the_scroll_stack_closes_the_window() {
    // `PlayerInventory.removeItem`: the scroll leaves the inventory, the window is closed.
    let mut s = Scene::alone(vec![inv(SWORD, C_SWORD, 1), inv(SCROLL, SCROLL_WEAPON_C, 1)]);
    s.use_item(A, SCROLL);
    s.enchant(A, EnchantInput::PutTargetItem { object_id: SWORD });
    assert!(s.world.is_enchanting(A));
    s.input(
        A,
        ClientInput::DestroyItem {
            object_id: SCROLL,
            count: 1,
        },
    );
    // The scroll in use cannot be destroyed through the client packet either?  Java's
    // RequestDestroyItem has no such check, so it goes: and the window closes.
    assert!(s.item(A, SCROLL).is_none());
    assert_eq!(s.world.active_enchant_item(A), None);
    assert!(!s.world.is_enchanting(A));
}

#[test]
fn a_trade_cannot_be_confirmed_while_a_player_has_a_scroll_open() {
    let mut s = Scene::new();
    s.enter(player(
        A,
        "Alice",
        1000,
        0,
        vec![
            inv(SWORD, C_SWORD, 1),
            inv(SCROLL, SCROLL_WEAPON_C, 1),
            inv(300, ADENA, 100),
        ],
    ));
    s.enter(player(B, "Bob", 1050, 0, vec![]));
    s.use_item(A, SCROLL);
    s.input(B, ClientInput::TradeRequest { object_id: A.0 });
    s.input(A, ClientInput::AnswerTradeRequest { response: 1 });
    s.log.take();
    // `TradeDone` of either side does nothing: no TradeDone packet, the trade stays open.
    s.input(A, ClientInput::TradeDone { response: 1 });
    s.input(B, ClientInput::TradeDone { response: 1 });
    let out = s.log.take();
    assert!(of(&out, A, "TRADE_DONE").is_empty() && of(&out, B, "TRADE_DONE").is_empty());
    assert!(s.world.player(A).unwrap().trade.active.is_some());
}

#[test]
fn nothing_is_enchanted_while_a_private_store_is_open() {
    let mut s = sword_scene(0, SCROLL_WEAPON_C);
    s.open(SCROLL, SWORD);
    s.input(A, ClientInput::Store(StoreInput::ActionUse { action_id: 10 }));
    s.log.take();
    s.press(SWORD);
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::YouCannotEnchantWhileOperatingAPrivateStoreOrPrivateWorkshop]
    );
    assert_eq!(s.world.active_enchant_item(A), None);
    assert_eq!(s.count_of(A, SCROLL_WEAPON_C), 5);
}

// ---- the over-enchant protection ----------------------------------------------------------------

#[test]
fn an_item_past_the_maximum_is_destroyed_when_used() {
    // GROUPS: weapons reach +15 with a chance, so the maximum is 16 (+1). A +17 sword breaks when
    // the player tries to equip it, with the messages and the punishment of OverEnchantPunishment.
    // Entering the world would destroy it already: the protection is switched on after the enter.
    let mut s = Scene::with_settings(|e| e.over_enchant_protection = false);
    s.enter(player(
        A,
        "Alice",
        1000,
        0,
        vec![enchanted(inv(SWORD, C_SWORD, 1), 17)],
    ));
    s.world.set_settings(WorldSettings::default());
    s.log.take();
    s.use_item(A, SWORD);
    let out = s.log.take();
    assert!(s.item(A, SWORD).is_none());
    // `Inventory.destroyItem` tells the client nothing.
    assert!(updates(&out, A).is_empty());
    let texts = messages_text(&out, A);
    assert!(texts.contains(&"[Server]: You have over-enchanted items!".to_owned()));
    assert!(texts.contains(&"[Server]: Respect our server rules.".to_owned()));
    assert!(!of(&out, A, "EX_SHOW_SCREEN_MESSAGE").is_empty());
    assert!(texts.contains(&"Illegal action performed!".to_owned()));

    // +16 is the maximum itself: allowed.
    let mut s = Scene::alone(vec![enchanted(inv(SWORD, C_SWORD, 1), 16)]);
    s.use_item(A, SWORD);
    assert_eq!(s.item(A, SWORD).unwrap().loc, "PAPERDOLL");
}

#[test]
fn the_protection_is_off_for_gms_and_when_the_config_says_so() {
    let mut s = Scene::new();
    s.enter(player(
        A,
        "Gm",
        1000,
        100,
        vec![enchanted(inv(SWORD, C_SWORD, 1), 30)],
    ));
    s.log.take();
    s.use_item(A, SWORD);
    assert_eq!(s.item(A, SWORD).unwrap().loc, "PAPERDOLL");

    let mut s = Scene::with_settings(|e| e.over_enchant_protection = false);
    s.enter(player(
        A,
        "Alice",
        1000,
        0,
        vec![enchanted(inv(SWORD, C_SWORD, 1), 30)],
    ));
    s.log.take();
    s.use_item(A, SWORD);
    assert_eq!(s.item(A, SWORD).unwrap().loc, "PAPERDOLL");
}

#[test]
fn entering_the_world_destroys_every_over_enchanted_item_and_punishes_once() {
    let mut s = Scene::new();
    s.enter(player(
        A,
        "Alice",
        1000,
        0,
        vec![
            enchanted(worn(SWORD, C_SWORD, 7), 20),
            enchanted(inv(101, C_HELMET, 1), 20),
            enchanted(inv(102, C_NECKLACE, 1), 20),
            enchanted(inv(103, C_SWORD, 1), 16),
        ],
    ));
    let out = s.log.take();
    // The armor maximum is 6 + 1 = 7; weapons 16; accessories (no group: the armor maximum) 7.
    assert!(s.item(A, SWORD).is_none());
    assert!(s.item(A, 101).is_none());
    assert!(s.item(A, 102).is_none());
    assert!(s.item(A, 103).is_some());
    let texts = messages_text(&out, A);
    assert_eq!(
        texts
            .iter()
            .filter(|t| *t == "[Server]: You have over-enchanted items!")
            .count(),
        1
    );
}

// ---- the element stones -------------------------------------------------------------------------

fn stone_scene(items: Vec<ItemData>, chance: f64) -> Scene {
    let mut s = Scene::with_settings(|e| {
        e.enchant_chance_element_stone = chance;
        e.enchant_chance_element_crystal = chance;
    });
    s.enter(player(A, "Alice", 1000, 0, items));
    s.log.take();
    s
}

#[test]
fn using_a_stone_opens_the_attribute_window_of_its_element() {
    let mut s = stone_scene(
        vec![
            inv(110, S_SWORD, 1),
            inv(210, FIRE_STONE, 2),
            inv(211, DARK_STONE, 1),
        ],
        100.0,
    );
    s.use_item(A, 210);
    let out = s.log.take();
    let window: ExChooseInventoryAttributeItem = decode(of(&out, A, "EX_CHOOSE_INVENTORY_ATTRIBUTE_ITEM")[0]);
    assert_eq!(window, ExChooseInventoryAttributeItem::new(FIRE_STONE, 0, 3));
    assert_eq!(s.world.enchant_state(A).unwrap().active_attr, Some(ObjectId(210)));
    // The stone in use blocks logout.
    s.input(A, ClientInput::Logout);
    assert!(s.world.player(A).is_some());
    s.log.take();

    // The dark stone: flag 5, level 3. A crystal would say 6.
    s.use_item(A, 211);
    let out = s.log.take();
    let window: ExChooseInventoryAttributeItem = decode(of(&out, A, "EX_CHOOSE_INVENTORY_ATTRIBUTE_ITEM")[0]);
    assert_eq!(window.elements, [false, false, false, false, false, true]);
}

#[test]
fn a_stone_gives_a_weapon_twenty_then_five() {
    // Java: FIRST_WEAPON_BONUS 20 when the weapon has none of that element, NEXT_WEAPON_BONUS 5
    // after. A stone (level 3) stops at WEAPON_VALUES[3] = 150.
    let mut s = stone_scene(vec![inv(110, S_SWORD, 1), inv(210, FIRE_STONE, 3)], 100.0);
    s.use_item(A, 210);
    s.log.take();
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 110 });
    let out = s.log.take();

    assert_eq!(
        s.item(A, 110).unwrap().elementals,
        [ItemElemental {
            element: 0,
            value: 20
        }]
    );
    assert_eq!(s.count_of(A, FIRE_STONE), 2);
    // S2_ELEMENTAL_POWER_HAS_BEEN_ADDED_SUCCESSFULLY_TO_S1: the item and the element.
    let added = messages(&out, A)
        .into_iter()
        .find(|m| m.id == SystemMessageId::S2ElementalPowerHasBeenAddedSuccessfullyToS1)
        .unwrap();
    assert_eq!(
        added.params,
        [
            SystemMessageParam::ItemName(S_SWORD),
            SystemMessageParam::ElementName(0)
        ]
    );
    assert_eq!(
        decode::<ExAttributeEnchantResult>(of(&out, A, "EX_ATTRIBUTE_ENCHANT_RESULT")[0]).result,
        20
    );
    assert!(!of(&out, A, "USER_INFO").is_empty());
    assert!(item_changes(&out).iter().any(|c| c.1.contains(&110)));
    assert_eq!(s.world.enchant_state(A).unwrap().active_attr, None);

    // The second stone: +5.
    s.use_item(A, 210);
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 110 });
    assert_eq!(s.item(A, 110).unwrap().elemental(0).unwrap().value, 25);
    assert_eq!(
        decode::<ExAttributeEnchantResult>(of(&s.log.take(), A, "EX_ATTRIBUTE_ENCHANT_RESULT")[0]).result,
        5
    );
}

#[test]
fn the_attribute_stops_at_the_limit_of_the_stone() {
    // 145 + 5 = 150: the limit of a level 3 stone for a weapon.
    let mut s = stone_scene(
        vec![
            with_element(inv(110, S_SWORD, 1), 0, 145),
            inv(210, FIRE_STONE, 2),
        ],
        100.0,
    );
    s.use_item(A, 210);
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 110 });
    assert_eq!(s.item(A, 110).unwrap().elemental(0).unwrap().value, 150);
    s.log.take();

    // At the limit nothing can be added: ATTRIBUTE_ITEM_USAGE_HAS_BEEN_CANCELLED, the stone stays.
    s.use_item(A, 210);
    s.log.take();
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 110 });
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::AttributeItemUsageHasBeenCancelled]
    );
    assert_eq!(s.count_of(A, FIRE_STONE), 1);
    assert_eq!(s.item(A, 110).unwrap().elemental(0).unwrap().value, 150);
    assert_eq!(s.world.enchant_state(A).unwrap().active_attr, None);
}

#[test]
fn a_crystal_reaches_further_than_a_stone() {
    // Crystal: level 6 -> 300 for a weapon. 295 + 5 = 300.
    let mut s = stone_scene(
        vec![
            with_element(inv(110, S_SWORD, 1), 0, 295),
            inv(210, FIRE_CRYSTAL, 1),
        ],
        100.0,
    );
    s.use_item(A, 210);
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 110 });
    assert_eq!(s.item(A, 110).unwrap().elemental(0).unwrap().value, 300);
}

#[test]
fn armor_takes_the_opposite_element_and_six_points() {
    // The fire stone on armor gives water resistance (the opposite element), 6 points
    // (`ARMOR_BONUS`), and the message names the water and then fire.
    let mut s = stone_scene(vec![inv(111, S_CUIRASS, 1), inv(210, FIRE_STONE, 2)], 100.0);
    s.use_item(A, 210);
    s.log.take();
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 111 });
    let out = s.log.take();
    assert_eq!(
        s.item(A, 111).unwrap().elementals,
        [ItemElemental { element: 1, value: 6 }]
    );
    let added = messages(&out, A)
        .into_iter()
        .find(|m| {
            m.id == SystemMessageId::TheS2SAttributeWasSuccessfullyBestowedOnS1AndResistanceToS3WasIncreased
        })
        .unwrap();
    // addItemName, addElemental(realElement = the stone's fire), addElemental(opposite = water).
    assert_eq!(
        added.params,
        [
            SystemMessageParam::ItemName(S_CUIRASS),
            SystemMessageParam::ElementName(0),
            SystemMessageParam::ElementName(1)
        ]
    );
}

#[test]
fn an_enchanted_item_uses_the_longer_message() {
    let mut s = stone_scene(
        vec![enchanted(inv(111, S_CUIRASS, 1), 4), inv(210, FIRE_STONE, 1)],
        100.0,
    );
    s.use_item(A, 210);
    s.log.take();
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 111 });
    let out = s.log.take();
    let added = messages(&out, A)
        .into_iter()
        .find(|m| {
            m.id == SystemMessageId::TheS3SAttributeWasSuccessfullyBestowedOnS1S2AndResistanceToS4WasIncreased
        })
        .unwrap();
    assert_eq!(added.params[0], SystemMessageParam::IntNumber(4));
}

#[test]
fn armor_refuses_a_fourth_element_and_an_opposite_one() {
    // Three different elements are the most: the water stone adds a fourth... (earth, wind, holy
    // are not stones here; use three plain entries and the fire stone's opposite).
    let mut three = with_element(inv(111, S_CUIRASS, 1), 2, 6);
    three = with_element(three, 3, 6);
    three = with_element(three, 4, 6);
    let mut s = stone_scene(vec![three, inv(210, FIRE_STONE, 1)], 100.0);
    s.use_item(A, 210);
    s.log.take();
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 111 });
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::AnotherElementalPowerHasAlreadyBeenAddedThisElementalPowerCannotBeAdded]
    );
    assert_eq!(s.count_of(A, FIRE_STONE), 1);

    // An armor that has fire (the opposite of the water the fire stone would give): illegal action.
    let mut s = stone_scene(
        vec![
            with_element(inv(111, S_CUIRASS, 1), 0, 6),
            inv(210, FIRE_STONE, 1),
        ],
        100.0,
    );
    s.use_item(A, 210);
    s.log.take();
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 111 });
    let out = s.log.take();
    assert_eq!(
        messages_text(&out, A),
        ["You will be kicked for illegal action, GM informed."]
    );
    assert_eq!(s.count_of(A, FIRE_STONE), 1);
    assert_eq!(s.world.enchant_state(A).unwrap().active_attr, None);
}

#[test]
fn a_failed_stone_is_used_up_for_nothing() {
    let mut s = stone_scene(vec![inv(110, S_SWORD, 1), inv(210, FIRE_STONE, 1)], 0.0);
    s.use_item(A, 210);
    s.log.take();
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 110 });
    let out = s.log.take();
    assert!(s.item(A, 110).unwrap().elementals.is_empty());
    assert_eq!(s.count_of(A, FIRE_STONE), 0);
    assert!(message_ids(&out, A).contains(&SystemMessageId::YouHaveFailedToAddElementalPower));
    // The result still carries the power that would have been added.
    assert_eq!(
        decode::<ExAttributeEnchantResult>(of(&out, A, "EX_ATTRIBUTE_ENCHANT_RESULT")[0]).result,
        20
    );
}

#[test]
fn the_stone_dice_are_rnd_get_100_against_the_chance() {
    // `Rnd.get(100) < ENCHANT_CHANCE_ELEMENT_STONE` (50): replay the seed.
    let mut expected = WorldRng::from_seed(1);
    let success = expected.below(100) < 50;
    let mut s = stone_scene(vec![inv(110, S_SWORD, 1), inv(210, FIRE_STONE, 1)], 50.0);
    s.use_item(A, 210);
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 110 });
    assert_eq!(!s.item(A, 110).unwrap().elementals.is_empty(), success);
}

#[test]
fn the_attribute_window_can_be_cancelled_and_refuses_what_cannot_hold_an_element() {
    let mut s = stone_scene(
        vec![
            inv(110, S_SWORD, 1),
            inv(210, FIRE_STONE, 1),
            inv(300, C_SWORD, 1),
        ],
        100.0,
    );
    s.use_item(A, 210);
    s.log.take();
    // -1 (0xFFFFFFFF): cancelled.
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: -1 });
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::AttributeItemUsageHasBeenCancelled]
    );
    assert_eq!(s.world.enchant_state(A).unwrap().active_attr, None);

    // The C sword has no `element_enabled`.
    s.use_item(A, 210);
    s.log.take();
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 300 });
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::ElementalPowerEnhancerUsageRequirementIsNotSufficient]
    );
    assert_eq!(s.world.enchant_state(A).unwrap().active_attr, None);

    // An object the player does not have: cancelled.
    s.use_item(A, 210);
    s.log.take();
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 4242 });
    assert_eq!(
        message_ids(&s.log.take(), A),
        [SystemMessageId::AttributeItemUsageHasBeenCancelled]
    );
}

#[test]
fn a_stone_cancels_the_open_trade_request() {
    // `getActiveRequester() != null`: `cancelActiveTrade()`, the message, the stone is free.
    let mut s = Scene::new();
    s.enter(player(
        A,
        "Alice",
        1000,
        0,
        vec![inv(110, S_SWORD, 1), inv(210, FIRE_STONE, 1)],
    ));
    s.enter(player(B, "Bob", 1050, 0, vec![]));
    s.use_item(A, 210);
    s.input(B, ClientInput::TradeRequest { object_id: A.0 });
    s.log.take();
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 110 });
    let out = s.log.take();
    assert_eq!(
        messages_text(&out, A),
        ["You cannot add elemental power while trading."]
    );
    assert_eq!(s.count_of(A, FIRE_STONE), 1);
    assert_eq!(s.world.enchant_state(A).unwrap().active_attr, None);
    // NOTE(java-quirk): `cancelActiveTrade()` only cancels a trade list; a request that was not
    // answered yet stays, so Alice can still accept it.
    assert_eq!(s.world.player(A).unwrap().trade.active_requester, Some(B));
}

#[test]
fn a_worn_item_gets_its_element_stats() {
    // The bonus of an equipped weapon: FuncAdd(FIRE_POWER, 0x40, 20) from the new element.
    let mut s = stone_scene(vec![worn(110, S_SWORD, 7), inv(210, FIRE_STONE, 2)], 100.0);
    assert_eq!(s.world.stats(A).unwrap().attack_element_value(0), 0);
    s.use_item(A, 210);
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 110 });
    assert_eq!(s.world.stats(A).unwrap().attack_element_value(0), 20);

    // `updateElementAttrBonus`: the next stone replaces the bonus, it does not add a second one.
    s.use_item(A, 210);
    s.enchant(A, EnchantInput::EnchantAttribute { object_id: 110 });
    assert_eq!(s.world.stats(A).unwrap().attack_element_value(0), 25);
}

// ---- removing an element ------------------------------------------------------------------------

#[test]
fn an_npc_takes_the_element_off_for_the_price_of_the_grade() {
    // S weapon 50000, S armor 40000 (`RequestExRemoveItemAttribute.getPrice`).
    let mut s = stone_scene(
        vec![
            with_element(inv(110, S_SWORD, 1), 0, 20),
            with_element(inv(111, S_CUIRASS, 1), 1, 6),
            inv(300, ADENA, 100_000),
        ],
        100.0,
    );
    s.enchant(
        A,
        EnchantInput::RemoveAttribute {
            object_id: 110,
            element: 0,
        },
    );
    let out = s.log.take();
    assert_eq!(s.count_of(A, ADENA), 50_000);
    assert!(s.item(A, 110).unwrap().elementals.is_empty());
    // S1_S_S2_ATTRIBUTE_HAS_BEEN_REMOVED: the item and the element.
    let removed = messages(&out, A)
        .into_iter()
        .find(|m| m.id == SystemMessageId::S1SS2AttributeHasBeenRemoved)
        .unwrap();
    assert_eq!(
        removed.params,
        [
            SystemMessageParam::ItemName(S_SWORD),
            SystemMessageParam::ElementName(0)
        ]
    );
    assert_eq!(
        decode::<ExBaseAttributeCancelResult>(of(&out, A, "EX_BASE_ATTRIBUTE_CANCEL_RESULT")[0]),
        ExBaseAttributeCancelResult {
            object_id: 110,
            attribute: 0
        }
    );
    assert!(!of(&out, A, "USER_INFO").is_empty());
    assert!(item_changes(&out).iter().any(|c| c.1.contains(&110)));

    // The armor: 40000, the message names the opposite element, then the other.
    s.enchant(
        A,
        EnchantInput::RemoveAttribute {
            object_id: 111,
            element: 1,
        },
    );
    let out = s.log.take();
    assert_eq!(s.count_of(A, ADENA), 10_000);
    let removed = messages(&out, A)
        .into_iter()
        .find(|m| m.id == SystemMessageId::S1SS2AttributeWasRemovedAndResistanceToS3WasDecreased)
        .unwrap();
    assert_eq!(
        removed.params,
        [
            SystemMessageParam::ItemName(S_CUIRASS),
            SystemMessageParam::ElementName(0),
            SystemMessageParam::ElementName(1)
        ]
    );
}

#[test]
fn without_the_adena_the_element_stays() {
    let mut s = stone_scene(
        vec![with_element(inv(110, S_SWORD, 1), 0, 20), inv(300, ADENA, 49_999)],
        100.0,
    );
    s.enchant(
        A,
        EnchantInput::RemoveAttribute {
            object_id: 110,
            element: 0,
        },
    );
    let out = s.log.take();
    // `reduceAdena(.., sendMessage = true)` says so first.
    assert_eq!(
        message_ids(&out, A),
        [
            SystemMessageId::YouDoNotHaveEnoughAdena,
            SystemMessageId::YouDoNotHaveEnoughFundsToCancelThisAttribute
        ]
    );
    assert_eq!(s.item(A, 110).unwrap().elementals.len(), 1);
    assert_eq!(s.count_of(A, ADENA), 49_999);

    // Nothing to remove: silent. Another element than the item has: silent.
    s.enchant(
        A,
        EnchantInput::RemoveAttribute {
            object_id: 110,
            element: 3,
        },
    );
    s.enchant(
        A,
        EnchantInput::RemoveAttribute {
            object_id: 555,
            element: 0,
        },
    );
    assert!(s.log.take().is_empty());
}

#[test]
fn a_grade_without_a_price_is_removed_for_free() {
    // NOTE(java-quirk): `_price` stays 0 for a grade other than S, S80 and S84.
    let mut s = stone_scene(vec![with_element(inv(110, D_SWORD, 1), 0, 20)], 100.0);
    s.enchant(
        A,
        EnchantInput::RemoveAttribute {
            object_id: 110,
            element: 0,
        },
    );
    assert!(s.item(A, 110).unwrap().elementals.is_empty());
    assert!(
        messages(&s.log.take(), A)
            .iter()
            .any(|m| m.id == SystemMessageId::S1SS2AttributeHasBeenRemoved)
    );
}

#[test]
fn the_cancel_window_lists_the_items_with_an_element_and_keeps_the_last_price() {
    // NOTE(java-quirk): the packet's `_price` carries over to an item whose grade has none.
    let s = stone_scene(
        vec![
            with_element(inv(110, S_SWORD, 1), 0, 20),
            inv(112, C_SWORD, 1),
            with_element(inv(113, D_SWORD, 1), 1, 5),
            with_element(inv(111, S_CUIRASS, 1), 1, 6),
        ],
        100.0,
    );
    s.world.send_base_attribute_cancel_window(A);
    let out = s.log.take();
    let window: ExShowBaseAttributeCancelWindow =
        decode(of(&out, A, "EX_SHOW_BASE_ATTRIBUTE_CANCEL_WINDOW")[0]);
    assert_eq!(window.items, [(110, 50_000), (113, 50_000), (111, 40_000)]);
}

#[test]
fn removing_the_element_of_a_worn_item_keeps_the_other_elements_active() {
    // The bonus functions follow the elements: removing fire leaves water.
    let worn_cuirass = with_element(with_element(worn(111, S_CUIRASS, 10), 0, 6), 2, 6);
    let mut s = stone_scene(vec![worn_cuirass, inv(300, ADENA, 100_000)], 100.0);
    s.enchant(
        A,
        EnchantInput::RemoveAttribute {
            object_id: 111,
            element: 0,
        },
    );
    assert_eq!(
        s.item(A, 111).unwrap().elementals,
        [ItemElemental { element: 2, value: 6 }]
    );
    let stats = s.world.stats(A).unwrap();
    assert_eq!(stats.defense_element_value(0), 0);
    assert_eq!(stats.defense_element_value(2), 6);
}

// ---- the warehouse checks -----------------------------------------------------------------------

#[test]
fn a_warehouse_deposit_cancels_the_scroll_and_a_package_send_is_an_exploit() {
    let mut s = sword_scene(0, SCROLL_WEAPON_C);
    // No scroll: nothing happens.
    assert!(!s.world.warehouse_cancels_enchant(A));
    assert!(!s.world.package_send_enchant_exploit(A));
    assert!(!s.world.enchant_blocks_warehouse_link(A));

    // `SendWareHouseDepositList`: the message, EnchantResult(2), the scroll is free.
    s.use_item(A, SCROLL);
    s.enchant(A, EnchantInput::PutTargetItem { object_id: SWORD });
    assert!(s.world.enchant_blocks_warehouse_link(A));
    s.log.take();
    assert!(s.world.warehouse_cancels_enchant(A));
    let out = s.log.take();
    assert_eq!(
        message_ids(&out, A),
        [SystemMessageId::YouHaveCancelledTheEnchantingProcess]
    );
    assert_eq!(results(&out, A), [(2, 0, 0)]);
    assert_eq!(s.world.active_enchant_item(A), None);

    // `RequestPackageSend`: an illegal action, the scroll stays.
    s.use_item(A, SCROLL);
    s.log.take();
    assert!(s.world.package_send_enchant_exploit(A));
    assert_eq!(
        messages_text(&s.log.take(), A),
        ["You will be kicked for illegal action, GM informed."]
    );
    assert_eq!(s.world.active_enchant_item(A), Some(ObjectId(SCROLL)));
}

#[test]
fn the_maximum_enchant_of_create_item_follows_the_item_kind() {
    // `AdminCreateItem.getMaxEnchant`: weapon, accessory, then armor. GROUPS: weapons 15 + 1,
    // armor 5 + 1... the armor group's 0-5 range has a chance, 6-15 has none, so 5 + 1 = 6; no
    // accessory group, so the accessories take the armor value.
    let s = Scene::new();
    let items = ItemTemplates::from_xml(ITEMS).unwrap();
    assert_eq!(s.world.max_enchant_of(items.get(C_SWORD).unwrap()), 16);
    assert_eq!(s.world.max_enchant_of(items.get(C_HELMET).unwrap()), 6);
    assert_eq!(s.world.max_enchant_of(items.get(C_NECKLACE).unwrap()), 6);
}
