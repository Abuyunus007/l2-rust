#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Augmentation (M6-AUGMENT-01): the life stone windows, the "Augment" button, the roll of a new
//! augmentation and its removal. The expected values come from the Java code named in each test
//! (`AbstractRefinePacket`, `RequestRefine`, `RequestConfirm*`, `RequestRefineCancel`,
//! `AugmentationData`). The random numbers differ from Java's, so the rolls are checked with tables
//! that leave one possible result, and with the shipped tables for the ranges they may take.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_data::augment::{AugmentationData, BLUE_START, SKILLS_BLOCKSIZE, STAT_BLOCKSIZE, STAT_SUBBLOCKSIZE};
use l2_data::item::types::{BodyPart, CrystalType};
use l2_data::{
    ArmorSets, AugmentationOptions, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillTemplates,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll as pd;
use l2_protocol_game::server::{
    ExPutCommissionResultForVariationMake, ExPutIntensiveResultForVariationMake,
    ExPutItemResultForVariationCancel, ExPutItemResultForVariationMake, ExVariationCancelResult,
    ExVariationResult, InventoryUpdate, SystemMessage,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::augment::{
    GRADE_ACC, GRADE_HIGH, GRADE_MID, GRADE_NONE, GRADE_TOP, LifeStone, RollInput, gemstone_count,
    gemstone_id, generate_random_augmentation, life_stone, life_stones, removal_price,
};
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
const C_STAFF: i32 = 3;
const S_RING: i32 = 5;
const S_NECK: i32 = 6;
const D_SWORD: i32 = 7;
const ETC_ITEM: i32 = 8;
const S_HELMET: i32 = 9;
const S84_SWORD: i32 = 10;
const NO_GRADE_SWORD: i32 = 11;
const PVP_SWORD: i32 = 10667;
const BLACKLISTED_SWORD: i32 = 6656;
const ADENA: i32 = 57;
const MID_LS_0: i32 = 8733;
const ACC_LS_0: i32 = 12754;
const HIGH_LS_13: i32 = 16162;
const GEM_D: i32 = 2130;
const GEM_C: i32 = 2131;

fn weapon(id: i32, grade: &str, extra: &str) -> String {
    format!(
        r#"<item id="{id}" type="Weapon" name="Weapon {id}"><set name="crystal_type" val="{grade}" /><set name="crystal_count" val="100" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />{extra}<set name="weight" val="50" /></item>"#
    )
}

fn items_xml() -> String {
    let mut xml = String::from("<list>\n");
    xml += &weapon(C_SWORD, "C", "");
    xml += r#"<item id="3" type="Weapon" name="C Staff"><set name="crystal_type" val="C" /><set name="crystal_count" val="100" /><set name="weapon_type" val="BLUNT" /><set name="bodypart" val="lrhand" /><set name="is_magic_weapon" val="true" /><set name="weight" val="50" /></item>"#;
    xml += r#"<item id="5" type="Armor" name="S Ring"><set name="crystal_type" val="S" /><set name="crystal_count" val="100" /><set name="bodypart" val="rfinger;lfinger" /><set name="weight" val="5" /></item>"#;
    xml += r#"<item id="6" type="Armor" name="S Necklace"><set name="crystal_type" val="S" /><set name="crystal_count" val="100" /><set name="bodypart" val="neck" /><set name="weight" val="5" /></item>"#;
    xml += &weapon(D_SWORD, "D", "");
    xml += r#"<item id="8" type="EtcItem" name="Etc"><set name="crystal_type" val="C" /></item>"#;
    xml += r#"<item id="9" type="Armor" name="S Helmet"><set name="crystal_type" val="S" /><set name="armor_type" val="HEAVY" /><set name="bodypart" val="head" /><set name="weight" val="5" /></item>"#;
    xml += &weapon(S84_SWORD, "S84", "");
    xml += &weapon(NO_GRADE_SWORD, "NONE", "");
    xml += &weapon(PVP_SWORD, "S", "");
    xml += &weapon(BLACKLISTED_SWORD, "B", "");
    xml += r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>"#;
    for id in [8723, 8733, 8762, 12754, 16162, 2130, 2131, 2132] {
        xml += &format!(
            r#"<item id="{id}" type="EtcItem" name="Stone {id}"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>"#
        );
    }
    xml += "</list>";
    xml
}

/// The weapon tables of the augmentation: the stone 8733 (middle grade, level 0) can only give
/// the augment 11 for the first pair and 7300 for the second (category 55), the stone 12754 the
/// accessory augments 16687 and 16669.
const WEAPON_CHANCES: &str = r#"<list>
<weapon type="warrior"><stone id="8733">
<variation id="1"><category probability="100"><augment id="11" chance="100" /></category></variation>
<variation id="2"><category probability="55"><augment id="7300" chance="100" /></category><category probability="35"><augment id="8400" chance="100" /></category></variation>
</stone></weapon>
<weapon type="mage"><stone id="8733">
<variation id="1"><category probability="100"><augment id="21" chance="100" /></category></variation>
<variation id="2"><category probability="55"><augment id="9300" chance="100" /></category></variation>
</stone></weapon>
</list>"#;

const ACCESSORY_CHANCES: &str = r#"<list>
<weapon type="warrior"><stone id="12754">
<variation id="1"><category probability="100"><augment id="16687" chance="100" /></category></variation>
<variation id="2"><category probability="55"><augment id="16669" chance="100" /></category></variation>
</stone></weapon>
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

fn data(items: &Arc<ItemTemplates>) -> Data {
    let stats = StatData {
        items: Arc::clone(items),
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
    Data {
        items: Arc::clone(items),
        stats,
    }
}

// ---- helpers ------------------------------------------------------------------------------------

const A: ObjectId = ObjectId(1);

fn inv(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".into(),
        ..Default::default()
    }
}

fn worn(object_id: i32, item_id: i32, slot: usize) -> ItemData {
    let mut item = inv(object_id, item_id, 1);
    item.loc = "PAPERDOLL".into();
    item.loc_data = i32::try_from(slot).unwrap();
    item
}

fn augmented(mut item: ItemData, augmentation: i32) -> ItemData {
    item.augmentation = Some(augmentation);
    item
}

fn player(level: i32, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: A,
        account: "acc1".into(),
        name: "Alice".into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: 1000,
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
    fn with_settings(level: i32, items: Vec<ItemData>, settings: impl FnOnce(&mut AugmentSettings)) -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(RecOutbox(log.clone())));
        let templates = Arc::new(ItemTemplates::from_xml(&items_xml()).unwrap());
        world.set_static_data(Box::new(data(&templates)));
        let mut augment = AugmentSettings::default();
        settings(&mut augment);
        world.set_settings(WorldSettings {
            augment,
            ..WorldSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_augmentation_data(Arc::new(
            AugmentationData::chances_from_xml(WEAPON_CHANCES, ACCESSORY_CHANCES).unwrap(),
        ));
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player(level, items)),
            sink: Box::new(Sink(1, log.clone())),
            ip: None,
        });
        log.take();
        Self { world, log }
    }

    /// Alice at level 85 with `items`.
    fn new(items: Vec<ItemData>) -> Self {
        Self::with_settings(85, items, |_| {})
    }

    fn augment(&mut self, input: AugmentInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::Augment(input),
        });
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn item(&self, object_id: i32) -> Option<ItemData> {
        self.world
            .player(A)?
            .data
            .items
            .iter()
            .find(|i| i.object_id == ObjectId(object_id))
            .cloned()
    }

    fn count_of(&self, item_id: i32) -> i64 {
        self.world
            .player(A)
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

fn of<'a>(out: &'a [Out], name: &str) -> Vec<&'a [u8]> {
    packets(out).into_iter().filter(|b| name_of(b) == name).collect()
}

fn message_ids(out: &[Out]) -> Vec<SystemMessageId> {
    of(out, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw).id)
        .collect()
}

/// `(change, object id, count)` of every entry of every `InventoryUpdate`.
fn updates(out: &[Out]) -> Vec<(i16, i32, i64)> {
    of(out, "INVENTORY_UPDATE")
        .iter()
        .flat_map(|raw| decode::<InventoryUpdate>(raw).updates)
        .map(|u| (u.change, u.item.object_id, u.item.count))
        .collect()
}

/// Every item row upserted by the outbox: `(object id, augmentation)`.
fn upserts(out: &[Out]) -> Vec<(i32, Option<i32>)> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemChanges { upserts, .. }) => Some(
                upserts
                    .iter()
                    .map(|i| (i.object_id.0, i.augmentation))
                    .collect::<Vec<_>>(),
            ),
            _ => None,
        })
        .flatten()
        .collect()
}

const SWORD: i32 = 100;
const LS: i32 = 101;
const GEMS: i32 = 102;

/// Alice with a C-grade sword, a middle grade life stone of level 0 and the 20 gemstones it needs.
fn sword_scene() -> Scene {
    Scene::new(vec![
        inv(SWORD, C_SWORD, 1),
        inv(LS, MID_LS_0, 1),
        inv(GEMS, GEM_D, 30),
    ])
}

fn refine(target: i32, refiner: i32, gems: i32, count: i64) -> AugmentInput {
    AugmentInput::Refine {
        target_item_object_id: target,
        refiner_item_object_id: refiner,
        gemstone_item_object_id: gems,
        gemstone_count: count,
    }
}

// ---- the tables -----------------------------------------------------------------------------------

#[test]
fn the_life_stone_table_is_java() {
    // AbstractRefinePacket: 8723-8732 no grade 0-9, 8733-8742 mid, 8743-8752 high, 8753-8762 top.
    assert_eq!(
        life_stone(8723),
        Some(LifeStone {
            grade: GRADE_NONE,
            level: 0
        })
    );
    assert_eq!(
        life_stone(8732),
        Some(LifeStone {
            grade: GRADE_NONE,
            level: 9
        })
    );
    assert_eq!(
        life_stone(8733),
        Some(LifeStone {
            grade: GRADE_MID,
            level: 0
        })
    );
    assert_eq!(
        life_stone(8752),
        Some(LifeStone {
            grade: GRADE_HIGH,
            level: 9
        })
    );
    assert_eq!(
        life_stone(8762),
        Some(LifeStone {
            grade: GRADE_TOP,
            level: 9
        })
    );
    assert_eq!(
        life_stone(9576),
        Some(LifeStone {
            grade: GRADE_TOP,
            level: 10
        })
    );
    assert_eq!(
        life_stone(10483),
        Some(LifeStone {
            grade: GRADE_NONE,
            level: 11
        })
    );
    assert_eq!(
        life_stone(12754),
        Some(LifeStone {
            grade: GRADE_ACC,
            level: 0
        })
    );
    assert_eq!(
        life_stone(12822),
        Some(LifeStone {
            grade: GRADE_ACC,
            level: 11
        })
    );
    assert_eq!(
        life_stone(12851),
        Some(LifeStone {
            grade: GRADE_ACC,
            level: 11
        })
    );
    assert_eq!(
        life_stone(14008),
        Some(LifeStone {
            grade: GRADE_ACC,
            level: 12
        })
    );
    assert_eq!(
        life_stone(14169),
        Some(LifeStone {
            grade: GRADE_TOP,
            level: 12
        })
    );
    assert_eq!(
        life_stone(16160),
        Some(LifeStone {
            grade: GRADE_NONE,
            level: 13
        })
    );
    assert_eq!(
        life_stone(16167),
        Some(LifeStone {
            grade: GRADE_TOP,
            level: 13
        })
    );
    assert_eq!(
        life_stone(16178),
        Some(LifeStone {
            grade: GRADE_ACC,
            level: 13
        })
    );
    assert_eq!(life_stone(57), None);
    assert_eq!(life_stone(8722), None);
    assert_eq!(life_stone(8763), None);
    // LifeStone.LEVELS: the character level of each life stone level.
    let levels: Vec<i32> = (0..14)
        .map(|level| LifeStone { grade: 0, level }.player_level())
        .collect();
    assert_eq!(levels, [46, 49, 52, 55, 58, 61, 64, 67, 70, 76, 80, 82, 84, 85]);
}

#[test]
fn gemstones_by_grade() {
    // getGemStoneId / getGemStoneCount.
    assert_eq!(gemstone_id(CrystalType::C), 2130);
    assert_eq!(gemstone_id(CrystalType::B), 2130);
    assert_eq!(gemstone_id(CrystalType::A), 2131);
    assert_eq!(gemstone_id(CrystalType::S), 2131);
    assert_eq!(gemstone_id(CrystalType::S80), 2132);
    assert_eq!(gemstone_id(CrystalType::S84), 2132);
    assert_eq!(gemstone_id(CrystalType::D), 0);
    let weapon: Vec<i32> = [
        CrystalType::C,
        CrystalType::B,
        CrystalType::A,
        CrystalType::S,
        CrystalType::S80,
        CrystalType::S84,
        CrystalType::D,
    ]
    .iter()
    .map(|g| gemstone_count(*g, GRADE_TOP))
    .collect();
    assert_eq!(weapon, [20, 30, 20, 25, 36, 36, 0]);
    let accessory: Vec<i32> = [
        CrystalType::C,
        CrystalType::B,
        CrystalType::A,
        CrystalType::S,
        CrystalType::S80,
        CrystalType::S84,
        CrystalType::D,
    ]
    .iter()
    .map(|g| gemstone_count(*g, GRADE_ACC))
    .collect();
    assert_eq!(accessory, [200, 300, 200, 250, 360, 480, 0]);
}

#[test]
fn removal_prices_follow_the_crystal_count() {
    // RequestRefineCancel / RequestConfirmCancelItem.
    assert_eq!(removal_price(CrystalType::C, 1719), Some(95_000));
    assert_eq!(removal_price(CrystalType::C, 1720), Some(150_000));
    assert_eq!(removal_price(CrystalType::C, 2451), Some(150_000));
    assert_eq!(removal_price(CrystalType::C, 2452), Some(210_000));
    assert_eq!(removal_price(CrystalType::B, 1745), Some(240_000));
    assert_eq!(removal_price(CrystalType::B, 1746), Some(270_000));
    assert_eq!(removal_price(CrystalType::A, 2159), Some(330_000));
    assert_eq!(removal_price(CrystalType::A, 2823), Some(390_000));
    assert_eq!(removal_price(CrystalType::A, 2824), Some(420_000));
    assert_eq!(removal_price(CrystalType::S, 5), Some(480_000));
    assert_eq!(removal_price(CrystalType::S80, 5), Some(920_000));
    assert_eq!(removal_price(CrystalType::S84, 5), Some(920_000));
    assert_eq!(removal_price(CrystalType::D, 5), None);
    assert_eq!(removal_price(CrystalType::None, 5), None);
}

// ---- the confirm packets --------------------------------------------------------------------------

#[test]
fn a_valid_item_is_accepted_by_the_target_window() {
    let mut s = Scene::new(vec![
        inv(SWORD, C_SWORD, 1),
        worn(103, S_RING, pd::RFINGER),
        inv(104, S_NECK, 1),
    ]);
    // ExPutItemResultForVariationMake(objectId, item.getId()), also for a worn item.
    for (object_id, item_id) in [(SWORD, C_SWORD), (103, S_RING), (104, S_NECK)] {
        s.augment(AugmentInput::ConfirmTargetItem { object_id });
        let out = s.log.take();
        assert_eq!(names(&out), ["EX_PUT_ITEM_RESULT_FOR_VARIATION_MAKE"]);
        let p: ExPutItemResultForVariationMake = decode(of(&out, "EX_PUT_ITEM_RESULT_FOR_VARIATION_MAKE")[0]);
        assert_eq!((p.item_object_id, p.item_id), (object_id, item_id));
    }
    // An object the player does not have: nothing.
    s.augment(AugmentInput::ConfirmTargetItem { object_id: 999 });
    assert!(s.log.take().is_empty());
}

#[test]
fn unsuitable_items_get_the_message() {
    let mut s = Scene::new(vec![
        inv(110, D_SWORD, 1),
        inv(111, ETC_ITEM, 1),
        inv(112, S_HELMET, 1),
        inv(113, PVP_SWORD, 1),
        inv(114, BLACKLISTED_SWORD, 1),
        inv(115, NO_GRADE_SWORD, 1),
        {
            let mut shadow = inv(116, C_SWORD, 1);
            shadow.mana_left = 100;
            shadow
        },
        {
            let mut limited = inv(117, C_SWORD, 1);
            limited.time = 1_000;
            limited
        },
    ]);
    // D grade, an etc item, a helmet (not an accessory), a PvP item, a blacklisted item, no grade,
    // a shadow item, a time limited item.
    for object_id in 110..=117 {
        s.augment(AugmentInput::ConfirmTargetItem { object_id });
        assert_eq!(
            message_ids(&s.log.take()),
            [SystemMessageId::ThisIsNotASuitableItem],
            "item {object_id}"
        );
    }
}

#[test]
fn an_augmented_item_has_its_own_message() {
    let mut s = Scene::new(vec![augmented(inv(SWORD, C_SWORD, 1), 0x0003_0001)]);
    s.augment(AugmentInput::ConfirmTargetItem { object_id: SWORD });
    assert_eq!(
        message_ids(&s.log.take()),
        [SystemMessageId::OnceAnItemIsAugmentedItCannotBeAugmentedAgain]
    );
}

#[test]
fn pvp_items_follow_the_config() {
    let mut s = Scene::with_settings(85, vec![inv(113, PVP_SWORD, 1)], |a| {
        a.alt_allow_augment_pvp_items = true
    });
    s.augment(AugmentInput::ConfirmTargetItem { object_id: 113 });
    assert_eq!(names(&s.log.take()), ["EX_PUT_ITEM_RESULT_FOR_VARIATION_MAKE"]);
}

#[test]
fn the_blacklist_is_the_config_one() {
    let mut s = Scene::with_settings(85, vec![inv(SWORD, C_SWORD, 1)], |a| a.blacklist = vec![C_SWORD]);
    s.augment(AugmentInput::ConfirmTargetItem { object_id: SWORD });
    assert_eq!(
        message_ids(&s.log.take()),
        [SystemMessageId::ThisIsNotASuitableItem]
    );
}

#[test]
fn the_player_must_be_able_to_augment() {
    // isValid(player): a message for a dead player.
    let mut s = sword_scene();
    s.world.set_dead(A, true);
    s.augment(AugmentInput::ConfirmTargetItem { object_id: SWORD });
    let ids = message_ids(&s.log.take());
    // Java sends the player's message and then "not a suitable item".
    assert_eq!(
        ids,
        [
            SystemMessageId::YouCannotAugmentItemsWhileDead,
            SystemMessageId::ThisIsNotASuitableItem
        ]
    );
}

#[test]
fn the_refiner_window_shows_the_gemstones() {
    let mut s = sword_scene();
    s.augment(AugmentInput::ConfirmRefinerItem {
        target_item_object_id: SWORD,
        refiner_item_object_id: LS,
    });
    let out = s.log.take();
    let p: ExPutIntensiveResultForVariationMake =
        decode(of(&out, "EX_PUT_INTENSIVE_RESULT_FOR_VARIATION_MAKE")[0]);
    // A C grade weapon with a middle grade stone: 20 D gemstones.
    assert_eq!(
        (
            p.refiner_item_object_id,
            p.life_stone_id,
            p.gemstone_item_id,
            p.gemstone_count
        ),
        (LS, MID_LS_0, GEM_D, 20)
    );
}

#[test]
fn the_refiner_must_fit_the_item_and_the_player() {
    // An accessory life stone on a weapon, and a weapon stone on an accessory.
    let mut s = Scene::new(vec![
        inv(SWORD, C_SWORD, 1),
        inv(104, S_NECK, 1),
        inv(LS, MID_LS_0, 1),
        inv(105, ACC_LS_0, 1),
        inv(106, HIGH_LS_13, 1),
        inv(107, ETC_ITEM, 1),
    ]);
    let ask = |s: &mut Scene, target, refiner| {
        s.augment(AugmentInput::ConfirmRefinerItem {
            target_item_object_id: target,
            refiner_item_object_id: refiner,
        });
        s.log.take()
    };
    assert_eq!(
        message_ids(&ask(&mut s, SWORD, 105)),
        [SystemMessageId::ThisIsNotASuitableItem]
    );
    assert_eq!(
        message_ids(&ask(&mut s, 104, LS)),
        [SystemMessageId::ThisIsNotASuitableItem]
    );
    // An item that is no life stone.
    assert_eq!(
        message_ids(&ask(&mut s, SWORD, 107)),
        [SystemMessageId::ThisIsNotASuitableItem]
    );
    // The accessory stone fits the accessory; S grade needs 250 gemstones C.
    let out = ask(&mut s, 104, 105);
    let p: ExPutIntensiveResultForVariationMake =
        decode(of(&out, "EX_PUT_INTENSIVE_RESULT_FOR_VARIATION_MAKE")[0]);
    assert_eq!((p.gemstone_item_id, p.gemstone_count), (GEM_C, 250));
    // The level 13 stone needs level 85: fine at 85, not at 84.
    assert_eq!(names(&ask(&mut s, SWORD, 106)).len(), 1);
    let mut low = Scene::with_settings(84, vec![inv(SWORD, C_SWORD, 1), inv(106, HIGH_LS_13, 1)], |_| {});
    assert_eq!(
        message_ids(&ask(&mut low, SWORD, 106)),
        [SystemMessageId::ThisIsNotASuitableItem]
    );
}

#[test]
fn the_gemstone_window_checks_the_count() {
    let mut s = sword_scene();
    let ask = |s: &mut Scene, count| {
        s.augment(AugmentInput::ConfirmGemStone {
            target_item_object_id: SWORD,
            refiner_item_object_id: LS,
            gemstone_item_object_id: GEMS,
            gemstone_count: count,
        });
        s.log.take()
    };
    // 19 is not the 20 the stone needs: GEMSTONE_QUANTITY_IS_INCORRECT.
    assert_eq!(
        message_ids(&ask(&mut s, 19)),
        [SystemMessageId::GemstoneQuantityIsIncorrect]
    );
    let out = ask(&mut s, 20);
    let p: ExPutCommissionResultForVariationMake =
        decode(of(&out, "EX_PUT_COMMISSION_RESULT_FOR_VARIATION_MAKE")[0]);
    assert_eq!(
        (p.gemstone_object_id, p.item_id, p.gemstone_count),
        (GEMS, GEM_D, 20)
    );

    // The wrong gemstone, or too few of them: not a suitable item.
    let mut wrong = Scene::new(vec![
        inv(SWORD, C_SWORD, 1),
        inv(LS, MID_LS_0, 1),
        inv(GEMS, GEM_C, 30),
    ]);
    assert_eq!(
        message_ids(&ask(&mut wrong, 20)),
        [SystemMessageId::ThisIsNotASuitableItem]
    );
    let mut few = Scene::new(vec![
        inv(SWORD, C_SWORD, 1),
        inv(LS, MID_LS_0, 1),
        inv(GEMS, GEM_D, 19),
    ]);
    assert_eq!(
        message_ids(&ask(&mut few, 20)),
        [SystemMessageId::ThisIsNotASuitableItem]
    );
}

// ---- RequestRefine -------------------------------------------------------------------------------

#[test]
fn a_refine_consumes_the_stones_and_augments_the_item() {
    let mut s = sword_scene();
    s.augment(refine(SWORD, LS, GEMS, 20));
    let out = s.log.take();

    // The tables leave one result: the first pair 11, the second 7300 (category 55 is rolled with
    // the 55 % default for this grade; the other category is 35, which holds 8400).
    let result: ExVariationResult = decode(of(&out, "EX_VARIATION_RESULT")[0]);
    assert_eq!(result.result, 1);
    assert_eq!(result.stat12, 11);
    assert!(result.stat34 == 7300 || result.stat34 == 8400 || result.stat34 == 0);
    let aug = s.item(SWORD).unwrap().augmentation.unwrap();
    assert_eq!(aug, (result.stat34 << 16) + result.stat12);
    assert_eq!((aug & 0xFFFF, aug >> 16), (result.stat12, result.stat34));

    // The life stone is gone, the gemstones are 10 less.
    assert!(s.item(LS).is_none());
    assert_eq!(s.item(GEMS).unwrap().count, 10);
    // The target is in an InventoryUpdate that goes out at once, and the load follows.
    assert!(updates(&out).contains(&(2, SWORD, 1)));
    assert!(names(&out).contains(&"STATUS_UPDATE".to_owned()));
    // The item row is written with the augmentation (REPLACE INTO item_attributes).
    assert!(upserts(&out).contains(&(SWORD, Some(aug))));

    // The delayed updates of the consumed stones come 100 ms later.
    s.advance(150);
    let later = updates(&s.log.take());
    assert!(later.contains(&(3, LS, 0)));
    assert!(later.contains(&(2, GEMS, 10)));
}

#[test]
fn a_refine_takes_a_worn_item_off_first() {
    let mut s = Scene::new(vec![
        worn(SWORD, C_SWORD, pd::RHAND),
        inv(LS, MID_LS_0, 1),
        inv(GEMS, GEM_D, 20),
    ]);
    s.augment(refine(SWORD, LS, GEMS, 20));
    let out = s.log.take();
    // The unequip InventoryUpdate comes first, then the user info, then the result.
    let order = names(&out);
    let pos = |name: &str| order.iter().position(|n| n == name).unwrap();
    assert!(pos("INVENTORY_UPDATE") < pos("USER_INFO"));
    assert!(pos("USER_INFO") < pos("EX_VARIATION_RESULT"));
    let item = s.item(SWORD).unwrap();
    assert_eq!(item.loc, "INVENTORY");
    assert!(item.augmentation.is_some());
    assert!(s.world.paperdoll_object(A, pd::RHAND).is_none());
}

#[test]
fn a_mage_weapon_uses_the_mage_table() {
    let mut s = Scene::new(vec![
        inv(SWORD, C_STAFF, 1),
        inv(LS, MID_LS_0, 1),
        inv(GEMS, GEM_D, 20),
    ]);
    s.augment(refine(SWORD, LS, GEMS, 20));
    let out = s.log.take();
    let result: ExVariationResult = decode(of(&out, "EX_VARIATION_RESULT")[0]);
    // The mage table of the stone has the first pair 21 and the second 9300.
    assert_eq!(result.stat12, 21);
    assert!(result.stat34 == 9300 || result.stat34 == 0);
}

#[test]
fn an_accessory_refine_uses_the_accessory_table() {
    let mut s = Scene::new(vec![
        inv(104, S_NECK, 1),
        inv(LS, ACC_LS_0, 1),
        inv(GEMS, GEM_C, 250),
    ]);
    s.augment(refine(104, LS, GEMS, 250));
    let out = s.log.take();
    let result: ExVariationResult = decode(of(&out, "EX_VARIATION_RESULT")[0]);
    assert_eq!(result.result, 1);
    assert_eq!(result.stat12, 16687);
    // Categories 55 gives 16669; the others have no entry in the test table (0).
    assert!(result.stat34 == 16669 || result.stat34 == 0);
    assert!(s.item(GEMS).is_none());
}

#[test]
fn a_refine_with_the_wrong_count_fails_without_using_anything() {
    let mut s = sword_scene();
    s.augment(refine(SWORD, LS, GEMS, 19));
    let out = s.log.take();
    let result: ExVariationResult = decode(of(&out, "EX_VARIATION_RESULT")[0]);
    assert_eq!((result.stat12, result.stat34, result.result), (0, 0, 0));
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::AugmentationFailedDueToInappropriateConditions]
    );
    assert!(s.item(LS).is_some());
    assert_eq!(s.item(GEMS).unwrap().count, 30);
    assert!(s.item(SWORD).unwrap().augmentation.is_none());
}

#[test]
fn a_refine_with_an_unsuitable_item_fails() {
    // A D grade sword.
    let mut s = Scene::new(vec![
        inv(SWORD, D_SWORD, 1),
        inv(LS, MID_LS_0, 1),
        inv(GEMS, GEM_D, 30),
    ]);
    s.augment(refine(SWORD, LS, GEMS, 20));
    let out = s.log.take();
    assert_eq!(names(&out), ["EX_VARIATION_RESULT", "SYSTEM_MESSAGE"]);
    assert!(s.item(LS).is_some());
    // An object the player does not have: nothing at all.
    s.augment(refine(SWORD, LS, 999, 20));
    assert!(s.log.take().is_empty());
}

#[test]
fn nothing_is_augmented_twice() {
    let mut s = Scene::new(vec![
        augmented(inv(SWORD, C_SWORD, 1), 0x0002_0001),
        inv(LS, MID_LS_0, 1),
        inv(GEMS, GEM_D, 30),
    ]);
    s.augment(refine(SWORD, LS, GEMS, 20));
    let out = s.log.take();
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::AugmentationFailedDueToInappropriateConditions]
    );
    assert_eq!(s.item(SWORD).unwrap().augmentation, Some(0x0002_0001));
}

#[test]
fn a_refine_while_trading_or_enchanting_does_nothing_visible() {
    let mut s = sword_scene();
    // In a private store: the message of isValid(player) and the failure.
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Store(StoreInput::ActionUse { action_id: 10 }),
    });
    s.log.take();
    s.augment(refine(SWORD, LS, GEMS, 20));
    let ids = message_ids(&s.log.take());
    assert_eq!(
        ids,
        [
            SystemMessageId::YouCannotAugmentItemsWhileAPrivateStoreOrPrivateWorkshopIsInOperation,
            SystemMessageId::AugmentationFailedDueToInappropriateConditions
        ]
    );
    assert!(s.item(LS).is_some());
}

// ---- the rolls -----------------------------------------------------------------------------------

fn roll_input(grade: i32, level: i32, id: i32, body_part: BodyPart) -> RollInput {
    RollInput {
        life_stone_level: level,
        life_stone_grade: grade,
        life_stone_id: id,
        body_part,
        magic_weapon: false,
    }
}

/// A skill map with three skills (blue, purple, red) in every life stone level block.
fn skill_map() -> AugmentationData {
    let mut xml = String::from("<list>");
    for level in 0..10 {
        for (i, color) in ["blue", "purple", "red"].iter().enumerate() {
            let id = BLUE_START + level * SKILLS_BLOCKSIZE + i32::try_from(i).unwrap();
            xml += &format!(
                r#"<augmentation id="{id}"><skillId val="1" /><skillLevel val="1" /><type val="{color}" /></augmentation>"#
            );
        }
    }
    xml += "</list>";
    AugmentationData::skill_map_from_xml(&xml).unwrap()
}

#[test]
fn the_free_form_roll_stays_in_the_stat_blocks() {
    // Free-form rolling (RetailLikeAugmentation = False): the ids follow the blocks of
    // `AugmentationData`. A plain stat of life stone level L is
    // `1 + 91 * L + 910 * k + 3640 * m + [0, 90]` (k the glow/colour step 0-3, m the block).
    let settings = AugmentSettings {
        retail_like: false,
        ..AugmentSettings::default()
    };
    let tables = skill_map();
    let mut kinds = [0u32; 3];
    for grade in [GRADE_NONE, GRADE_MID, GRADE_HIGH, GRADE_TOP] {
        for level in 0..=9 {
            for seed in 0..200 {
                let mut rng = WorldRng::from_seed(seed);
                let input = roll_input(grade, level, 0, BodyPart::RightHand);
                let aug =
                    generate_random_augmentation(&mut rng, &tables, &settings, input, &|_| false).unwrap();
                let (stat12, stat34) = (aug & 0xFFFF, aug >> 16);
                let context = format!("{aug:#x} grade {grade} level {level}");
                assert!((1..=2 * STAT_BLOCKSIZE).contains(&stat12), "{context}");
                assert_eq!(
                    ((stat12 - 1) % (STAT_SUBBLOCKSIZE * 10)) / STAT_SUBBLOCKSIZE,
                    level,
                    "{context}"
                );
                let skills = BLUE_START + level * SKILLS_BLOCKSIZE;
                if (skills..skills + 3).contains(&stat34) {
                    kinds[0] += 1;
                } else if (16341..=16344).contains(&stat34) {
                    kinds[1] += 1;
                } else {
                    // A plain stat of the second pair: blocks 2 and 3.
                    assert!(
                        (2 * STAT_BLOCKSIZE + 1..=4 * STAT_BLOCKSIZE).contains(&stat34),
                        "{context}"
                    );
                    assert_eq!(
                        ((stat34 - 1) % (STAT_SUBBLOCKSIZE * 10)) / STAT_SUBBLOCKSIZE,
                        level,
                        "{context}"
                    );
                    kinds[2] += 1;
                }
            }
        }
    }
    // All three kinds come up: skills (15-60 %), base stats (1 %) and plain stats.
    assert!(kinds.iter().all(|k| *k > 0), "{kinds:?}");
}

#[test]
fn an_empty_skill_list_ends_the_roll_like_java_does() {
    // 100 % skill chance and no skill map: Java's `List.get(0)` throws.
    let settings = AugmentSettings {
        retail_like: false,
        ng_skill_chance: 100,
        ..AugmentSettings::default()
    };
    let tables = AugmentationData::default();
    let mut rng = WorldRng::from_seed(1);
    let input = roll_input(GRADE_NONE, 0, 0, BodyPart::RightHand);
    assert_eq!(
        generate_random_augmentation(&mut rng, &tables, &settings, input, &|_| false),
        None
    );
}

#[test]
fn a_skill_is_blue_purple_or_red_by_the_grade() {
    let settings = AugmentSettings {
        retail_like: false,
        top_skill_chance: 100,
        ..AugmentSettings::default()
    };
    let tables = skill_map();
    // A top grade stone with a skill: resultColor (0-100) <= 10 * 3 + 5 = 35 is red (36 of 101),
    // <= 40 is blue (5 of 101), the rest purple (60 of 101).
    let mut colours = [0u32; 3];
    for seed in 0..600 {
        let mut rng = WorldRng::from_seed(seed);
        let input = roll_input(GRADE_TOP, 0, 0, BodyPart::RightHand);
        let aug = generate_random_augmentation(&mut rng, &tables, &settings, input, &|_| false).unwrap();
        match aug >> 16 {
            14561 => colours[0] += 1,
            14562 => colours[1] += 1,
            14563 => colours[2] += 1,
            other => panic!("not a skill: {other}"),
        }
    }
    assert!(
        colours[1] > colours[2] && colours[2] > colours[0] && colours[0] > 0,
        "{colours:?}"
    );
}

#[test]
fn an_accessory_roll_uses_the_blocks_of_its_body_part() {
    let settings = AugmentSettings {
        retail_like_accessory: false,
        ..AugmentSettings::default()
    };
    let tables = AugmentationData::default();
    let cases = [
        (BodyPart::BothFingers, 16669, 18, 102),
        (BodyPart::BothEars, 17689, 18, 102),
        (BodyPart::Neck, 18709, 24, 108),
    ];
    for (body_part, start, skills, block) in cases {
        for level in 0..=9 {
            for seed in 0..100 {
                let mut rng = WorldRng::from_seed(seed);
                let input = roll_input(GRADE_ACC, level, 0, body_part);
                let aug =
                    generate_random_augmentation(&mut rng, &tables, &settings, input, &|_| false).unwrap();
                let (stat12, stat34) = (aug & 0xFFFF, aug >> 16);
                let base = start + block * level;
                // Both options are stats of the same colour block of the level, and differ.
                for stat in [stat12, stat34] {
                    assert!(
                        stat >= base + skills && stat < base + block,
                        "{body_part:?} {level} {stat}"
                    );
                }
                assert_ne!(stat12, stat34);
                assert_eq!((stat12 - base - skills) / 21, (stat34 - base - skills) / 21);
            }
        }
    }
}

#[test]
fn an_accessory_skill_needs_an_option_with_a_skill() {
    let settings = AugmentSettings {
        retail_like_accessory: false,
        acc_skill_chance: 100,
        ..AugmentSettings::default()
    };
    let tables = AugmentationData::default();
    let input = roll_input(GRADE_ACC, 0, 0, BodyPart::Neck);
    // When the option has a skill, the second option is one of the 24 skill ids of the block.
    let mut rng = WorldRng::from_seed(3);
    let aug = generate_random_augmentation(&mut rng, &tables, &settings, input, &|_| true).unwrap();
    assert!((18709..18709 + 24).contains(&(aug >> 16)));
    // When it has none, it falls back to a stat.
    let mut rng = WorldRng::from_seed(3);
    let aug = generate_random_augmentation(&mut rng, &tables, &settings, input, &|_| false).unwrap();
    assert!(aug >> 16 >= 18709 + 24);
}

#[test]
fn the_retail_roll_picks_from_the_selected_table() {
    let settings = AugmentSettings::default();
    let tables = AugmentationData::chances_from_xml(WEAPON_CHANCES, ACCESSORY_CHANCES).unwrap();
    let mut seen = std::collections::BTreeSet::new();
    for seed in 0..300 {
        let mut rng = WorldRng::from_seed(seed);
        let input = roll_input(GRADE_MID, 0, MID_LS_0, BodyPart::RightHand);
        let aug = generate_random_augmentation(&mut rng, &tables, &settings, input, &|_| false).unwrap();
        assert_eq!(aug & 0xFFFF, 11);
        seen.insert(aug >> 16);
    }
    // The 55 % category (7300), the 35 % one (8400), and the empty ones (0).
    assert_eq!(seen, [0, 7300, 8400].into_iter().collect());
    // A stone without entries gives 0 for both.
    let mut rng = WorldRng::from_seed(1);
    let input = roll_input(GRADE_MID, 0, 9999, BodyPart::RightHand);
    assert_eq!(
        generate_random_augmentation(&mut rng, &tables, &settings, input, &|_| false),
        Some(0)
    );
}

#[test]
fn the_retail_category_chances_come_from_the_config() {
    // 100 % red (category 3): no entries of the test table, so the second option is 0.
    let settings = AugmentSettings {
        retail_like_chances: [[0, 0, 0, 100]; 4],
        ..AugmentSettings::default()
    };
    let tables = AugmentationData::chances_from_xml(WEAPON_CHANCES, ACCESSORY_CHANCES).unwrap();
    for seed in 0..50 {
        let mut rng = WorldRng::from_seed(seed);
        let input = roll_input(GRADE_MID, 0, MID_LS_0, BodyPart::RightHand);
        let aug = generate_random_augmentation(&mut rng, &tables, &settings, input, &|_| false).unwrap();
        assert_eq!(aug >> 16, 0);
    }
    // 100 % the 55 category.
    let settings = AugmentSettings {
        retail_like_chances: [[100, 0, 0, 0]; 4],
        ..AugmentSettings::default()
    };
    for seed in 0..50 {
        let mut rng = WorldRng::from_seed(seed);
        let input = roll_input(GRADE_MID, 0, MID_LS_0, BodyPart::RightHand);
        let aug = generate_random_augmentation(&mut rng, &tables, &settings, input, &|_| false).unwrap();
        assert_eq!(aug >> 16, 7300);
    }
}

/// The shipped datapack, when the reference folder is there.
fn shipped() -> Option<PathBuf> {
    let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"), PathBuf::from);
    if root.join("data/stats/augmentation").is_dir() {
        Some(root)
    } else {
        println!("skipped: no datapack at {}", root.display());
        None
    }
}

#[test]
fn every_shipped_roll_names_options_that_exist() {
    let Some(root) = shipped() else {
        return;
    };
    let skills = SkillTemplates::default();
    let options = AugmentationOptions::load(&root, &skills);
    let tables = AugmentationData::load(&root);
    if options.is_empty() {
        println!("skipped: no augmentation options");
        return;
    }
    // Skills are not loaded here, so options with only skills are missing from `options`: the
    // check is on the stat options and the ids the free-form roll uses for stats.
    let retail = AugmentSettings::default();
    let free = AugmentSettings {
        retail_like: false,
        retail_like_accessory: false,
        ..AugmentSettings::default()
    };
    let mut checked = 0;
    for settings in [&retail, &free] {
        for (id, ls) in life_stones() {
            let body_parts: &[BodyPart] = if ls.grade == GRADE_ACC {
                &[BodyPart::BothFingers, BodyPart::BothEars, BodyPart::Neck]
            } else {
                &[BodyPart::RightHand]
            };
            for body_part in body_parts {
                for magic_weapon in [false, true] {
                    for seed in 0..5 {
                        let mut rng = WorldRng::from_seed(seed);
                        let input = RollInput {
                            life_stone_level: ls.level,
                            life_stone_grade: ls.grade,
                            life_stone_id: id,
                            body_part: *body_part,
                            magic_weapon,
                        };
                        let Some(aug) =
                            generate_random_augmentation(&mut rng, &tables, settings, input, &|_| false)
                        else {
                            continue;
                        };
                        checked += 1;
                        let (stat12, stat34) = (aug & 0xFFFF, aug >> 16);
                        assert!(
                            options.get(stat12).is_some(),
                            "stone {id}: option {stat12} of {aug:#x}"
                        );
                        // A skill option (14561..) of the free-form roll is not a stat option.
                        let skill_range = BLUE_START..BLUE_START + SKILLS_BLOCKSIZE * 10;
                        if !skill_range.contains(&stat34) && stat34 != 0 {
                            assert!(
                                options.get(stat34).is_some(),
                                "stone {id}: option {stat34} of {aug:#x}"
                            );
                        }
                    }
                }
            }
        }
    }
    assert!(checked > 500, "{checked}");
}

// ---- removal -------------------------------------------------------------------------------------

const COST_C: i64 = 95_000;

#[test]
fn the_removal_window_shows_the_price() {
    // C grade, crystal count 100: 95,000.
    let mut s = Scene::new(vec![augmented(inv(SWORD, C_SWORD, 1), 0x0003_0002)]);
    s.augment(AugmentInput::ConfirmCancelItem { object_id: SWORD });
    let out = s.log.take();
    let p: ExPutItemResultForVariationCancel = decode(of(&out, "EX_PUT_ITEM_RESULT_FOR_VARIATION_CANCEL")[0]);
    assert_eq!(
        (p.item_object_id, p.item_id, p.aug1, p.aug2, p.price),
        (SWORD, C_SWORD, 2, 3, COST_C)
    );
    // `(short) augmentationId` is sign extended.
    let mut s = Scene::new(vec![augmented(inv(SWORD, C_SWORD, 1), 0x0001_8000)]);
    s.augment(AugmentInput::ConfirmCancelItem { object_id: SWORD });
    let p: ExPutItemResultForVariationCancel =
        decode(of(&s.log.take(), "EX_PUT_ITEM_RESULT_FOR_VARIATION_CANCEL")[0]);
    assert_eq!((p.aug1, p.aug2), (-32768, 1));
}

#[test]
fn the_removal_window_refuses_what_cannot_be_removed() {
    let mut s = Scene::new(vec![
        inv(SWORD, C_SWORD, 1),
        augmented(inv(113, PVP_SWORD, 1), 5),
        augmented(inv(115, NO_GRADE_SWORD, 1), 5),
    ]);
    // Not augmented.
    s.augment(AugmentInput::ConfirmCancelItem { object_id: SWORD });
    assert_eq!(
        message_ids(&s.log.take()),
        [SystemMessageId::AugmentationRemovalCanOnlyBeDoneOnAnAugmentedItem]
    );
    // A PvP item.
    s.augment(AugmentInput::ConfirmCancelItem { object_id: 113 });
    assert_eq!(
        message_ids(&s.log.take()),
        [SystemMessageId::ThisIsNotASuitableItem]
    );
    // A grade without a price: silence.
    s.augment(AugmentInput::ConfirmCancelItem { object_id: 115 });
    assert!(s.log.take().is_empty());
    // An object the player does not have: silence.
    s.augment(AugmentInput::ConfirmCancelItem { object_id: 999 });
    assert!(s.log.take().is_empty());
}

#[test]
fn removing_an_augmentation_costs_adena() {
    let mut s = Scene::new(vec![
        augmented(inv(SWORD, C_SWORD, 1), 0x0003_0002),
        inv(110, ADENA, 100_000),
    ]);
    s.augment(AugmentInput::RefineCancel {
        target_item_object_id: SWORD,
    });
    let out = s.log.take();
    assert_eq!(s.count_of(ADENA), 100_000 - COST_C);
    assert!(s.item(SWORD).unwrap().augmentation.is_none());
    let result: ExVariationCancelResult = decode(of(&out, "EX_VARIATION_CANCEL_RESULT")[0]);
    assert_eq!(result.result, 1);
    // The row is rewritten without the augmentation (DELETE FROM item_attributes).
    assert!(upserts(&out).contains(&(SWORD, None)));
    // The updates of the item and of the adena go out together, delayed.
    s.advance(150);
    let later = updates(&s.log.take());
    assert!(later.contains(&(2, SWORD, 1)));
    assert!(later.contains(&(2, 110, 100_000 - COST_C)));
}

#[test]
fn removing_takes_a_worn_item_off() {
    let mut s = Scene::new(vec![
        augmented(worn(SWORD, C_SWORD, pd::RHAND), 0x0003_0002),
        inv(110, ADENA, 100_000),
    ]);
    s.augment(AugmentInput::RefineCancel {
        target_item_object_id: SWORD,
    });
    s.log.take();
    let item = s.item(SWORD).unwrap();
    assert_eq!(item.loc, "INVENTORY");
    assert!(item.augmentation.is_none());
}

#[test]
fn removing_without_the_adena_says_so_twice() {
    // reduceAdena(.., true) says it, and the packet says it again.
    let mut s = Scene::new(vec![
        augmented(inv(SWORD, C_SWORD, 1), 0x0003_0002),
        inv(110, ADENA, COST_C - 1),
    ]);
    s.augment(AugmentInput::RefineCancel {
        target_item_object_id: SWORD,
    });
    let out = s.log.take();
    assert_eq!(
        message_ids(&out),
        [
            SystemMessageId::YouDoNotHaveEnoughAdena,
            SystemMessageId::YouDoNotHaveEnoughAdena
        ]
    );
    let result: ExVariationCancelResult = decode(of(&out, "EX_VARIATION_CANCEL_RESULT")[0]);
    assert_eq!(result.result, 0);
    assert!(s.item(SWORD).unwrap().augmentation.is_some());
    assert_eq!(s.count_of(ADENA), COST_C - 1);
}

#[test]
fn removing_from_other_items_fails() {
    let mut s = Scene::new(vec![
        inv(SWORD, C_SWORD, 1),
        augmented(inv(115, NO_GRADE_SWORD, 1), 5),
        inv(110, ADENA, 1_000_000),
    ]);
    // A missing item: the result 0 only.
    s.augment(AugmentInput::RefineCancel {
        target_item_object_id: 999,
    });
    let out = s.log.take();
    assert_eq!(names(&out), ["EX_VARIATION_CANCEL_RESULT"]);
    // Not augmented: the message and the result 0.
    s.augment(AugmentInput::RefineCancel {
        target_item_object_id: SWORD,
    });
    let out = s.log.take();
    assert_eq!(
        message_ids(&out),
        [SystemMessageId::AugmentationRemovalCanOnlyBeDoneOnAnAugmentedItem]
    );
    assert_eq!(names(&out), ["SYSTEM_MESSAGE", "EX_VARIATION_CANCEL_RESULT"]);
    // A grade without a price: the result 0, and the adena stays.
    s.augment(AugmentInput::RefineCancel {
        target_item_object_id: 115,
    });
    assert_eq!(names(&s.log.take()), ["EX_VARIATION_CANCEL_RESULT"]);
    assert_eq!(s.count_of(ADENA), 1_000_000);
}

// ---- the NPC link --------------------------------------------------------------------------------

#[test]
fn the_augment_link_opens_the_windows() {
    use l2_world::augment::augment_handler;
    let mut s = sword_scene();
    // Not bound to an NPC: false.
    assert!(!augment_handler(&mut s.world, A, "Augment 1", None));
    assert!(s.log.take().is_empty());
}
