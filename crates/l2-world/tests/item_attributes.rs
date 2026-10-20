#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Augmentations and element attributes of item instances (M5-FU-05): what the item packets
//! show, what the equipped item adds to the stats and which skills it gives, and the item rows
//! that leave the world when they change.
//!
//! Expected numbers come from the Java code named in each test (`Item.java`, `Elementals.java`,
//! `Augmentation.java`, `Options.java`, `Inventory.ItemSkillsListener`).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, AugmentationOption, AugmentationOptions, EnchantHpBonus, Hennas, ItemTemplates,
    PlayerTemplates, SkillTemplates,
};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::charscreen::paperdoll as pd;
use l2_util::WorldRng;
use l2_world::effects::item_extras::ItemElemental;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(Vec<u8>),
    Persist(PersistCommand),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.0.0.lock().unwrap().push(Out::Packet(bytes));
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

const ITEMS: &str = r#"<list>
<item id="1" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat></stats></item>
<item id="2" type="Armor" name="Breastplate"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="weight" val="10" />
<stats><stat type="pDef">40</stat></stats></item>
<item id="3" type="Weapon" name="Fire Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat><stat type="firePower">20</stat></stats></item>
<item id="4" type="Armor" name="Helmet"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="head" /><set name="weight" val="5" />
<stats><stat type="pDef">10</stat></stats></item>
<item id="5" type="Armor" name="Water Helmet"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="head" /><set name="weight" val="5" />
<stats><stat type="pDef">10</stat><stat type="waterRes">7</stat></stats></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
</list>"#;

const SKILLS: &str = r##"<list>
<skill id="3007" levels="1" name="Passive"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="critRate" val="11"/></effect></effects></skill>
<skill id="3008" levels="1" name="Active"><operateType>A1</operateType><targetType>SELF</targetType></skill>
</list>"##;

/// Option 1: +10 P. Atk. Option 2: +5 P. Def. and the passive skill 3007. Option 3: the active
/// skill 3008. Option 4: nothing a player can see.
const OPTIONS: &str = r#"<list>
<option id="1"><stats><add stat="pAtk" val="10"/></stats></option>
<option id="2"><stats><add stat="pDef" val="5"/></stats><passive_skill id="3007" level="1"/></option>
<option id="3"><active_skill id="3008" level="1"/></option>
<option id="4"><attack_skill id="3008" level="1" chance="5"/></option>
</list>"#;

/// Augmentation id with option 1 (low) and option 2 (high).
const AUG_ATK_DEF: i32 = (2 << 16) | 1;
/// Option 2 twice over (low 2, high 0: no option 0).
const AUG_DEF: i32 = 2;

struct Data {
    items: Arc<ItemTemplates>,
    stats: StatData,
    options: AugmentationOptions,
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
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
    fn augmentation_option(&self, id: i32) -> Option<&AugmentationOption> {
        self.options.get(id)
    }
}

fn data() -> Data {
    let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
    let skills = SkillTemplates::from_xml(
        SKILLS,
        &l2_data::skill::EnchantSkillGroups::default(),
        &l2_data::SkillLoadOptions::default(),
    );
    let options = AugmentationOptions::from_xml(OPTIONS, &skills).unwrap();
    let stats = StatData {
        items: Arc::clone(&items),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(skills),
    };
    Data {
        items,
        stats,
        options,
    }
}

// ---- helpers ------------------------------------------------------------------------------------

const ME: ObjectId = ObjectId(1);
const SESSION: SessionId = SessionId(1);
const SWORD: ObjectId = ObjectId(100);
const CHEST: ObjectId = ObjectId(101);
const HELMET: ObjectId = ObjectId(102);

fn item(object_id: i32, item_id: i32, loc: &str, loc_data: i32) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count: 1,
        loc: loc.into(),
        loc_data,
        ..Default::default()
    }
}

fn worn(object_id: i32, item_id: i32, slot: usize) -> ItemData {
    item(object_id, item_id, "PAPERDOLL", i32::try_from(slot).unwrap())
}

fn augmented(mut item: ItemData, augmentation: i32) -> ItemData {
    item.augmentation = Some(augmentation);
    item
}

fn with_element(mut item: ItemData, element: i32, value: i32) -> ItemData {
    item.apply_attribute(element, value);
    item
}

fn player(items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: ME,
        account: "acc".into(),
        name: "Me".into(),
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

fn world_with(p: PlayerData) -> (World, Log) {
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(1), Box::new(RecOutbox(log.clone())));
    world.set_static_data(Box::new(data()));
    world.handle(WorldCommand::EnterWorld {
        session: SESSION,
        player: Box::new(p),
        sink: Box::new(Sink(log.clone())),
        ip: None,
    });
    log.take();
    (world, log)
}

fn use_item(world: &mut World, object_id: ObjectId) {
    world.handle(WorldCommand::Input {
        session: SESSION,
        input: ClientInput::UseItem {
            object_id: object_id.0,
            ctrl_pressed: false,
        },
    });
}

fn advance(world: &mut World, ms: u64) {
    for _ in 0..(ms / 50) {
        world.tick();
    }
}

fn p_atk(world: &World) -> f64 {
    world.stats(ME).unwrap().p_atk(None)
}

fn p_def(world: &World) -> f64 {
    world.stats(ME).unwrap().p_def(None)
}

fn knows(world: &World, skill_id: i32) -> bool {
    world.stats(ME).unwrap().skill_level(skill_id) > 0
}

fn sent(out: &[Out], name: &str) -> usize {
    out.iter()
        .filter(|o| {
            matches!(o, Out::Packet(b) if ServerOpcode::from_id(b[0], None).is_some_and(|op| op.name() == name))
        })
        .count()
}

/// The `ItemInfo` of one inventory item as the item list shows it.
fn item_info_of(world: &World, object_id: ObjectId) -> l2_protocol_game::server::ItemInfo {
    let player = world.player(ME).unwrap();
    let list = l2_world::enter::item_list(&player.data, &data(), 0);
    list.items
        .into_iter()
        .find(|i| i.object_id == object_id.0)
        .unwrap()
}

// ---- augmentation ---------------------------------------------------------------------------------

/// `AugmentationStatBonus.applyBonus` at login (`Inventory.restore` runs the listeners) and on a
/// later equip, `removeBonus` on unequip: the functions of both options and their skills.
#[test]
fn the_options_of_a_worn_augmented_weapon_give_stats_and_skills() {
    let (plain, _) = world_with(player(vec![worn(100, 1, pd::RHAND), worn(101, 2, pd::CHEST)]));
    let (base_atk, base_def) = (p_atk(&plain), p_def(&plain));
    assert!(!knows(&plain, 3007));

    let (mut world, _) = world_with(player(vec![
        augmented(worn(100, 1, pd::RHAND), AUG_ATK_DEF),
        worn(101, 2, pd::CHEST),
    ]));
    // Option 1: add pAtk 10. Option 2: add pDef 5 and the passive skill 3007.
    assert_eq!(p_atk(&world), base_atk + 10.0);
    assert_eq!(p_def(&world), base_def + 5.0);
    assert!(knows(&world, 3007));

    // Taking it off removes both.
    use_item(&mut world, SWORD);
    assert!(
        world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .any(|i| i.object_id == SWORD && i.loc == "INVENTORY")
    );
    assert_eq!(p_def(&world), base_def);
    assert!(!knows(&world, 3007));

    // And putting it on again brings them back.
    use_item(&mut world, SWORD);
    assert_eq!(p_def(&world), base_def + 5.0);
    assert!(knows(&world, 3007));
}

/// `Options.apply`: an active skill of an option is added like any skill.
#[test]
fn an_active_skill_option_adds_the_skill_while_worn() {
    // Option 3 in the high half.
    let (mut world, _) = world_with(player(vec![augmented(worn(100, 1, pd::RHAND), 3 << 16)]));
    assert!(knows(&world, 3008));
    use_item(&mut world, SWORD);
    assert!(!knows(&world, 3008));
}

/// `Options.apply` / `remove` send the skill list for every option; an item without a known
/// option does not.
#[test]
fn equipping_an_augmented_item_sends_the_skill_list() {
    let mut inv_sword = item(100, 1, "INVENTORY", 0);
    inv_sword.augmentation = Some(AUG_ATK_DEF);
    let mut inv_plain = item(101, 1, "INVENTORY", 0);
    // Option 99 does not exist.
    inv_plain.augmentation = Some(99);
    let (mut world, log) = world_with(player(vec![inv_sword, inv_plain]));

    use_item(&mut world, SWORD);
    advance(&mut world, 1000);
    assert_eq!(sent(&log.take(), "SKILL_LIST"), 1);

    use_item(&mut world, SWORD);
    advance(&mut world, 1000);
    assert_eq!(sent(&log.take(), "SKILL_LIST"), 1);

    // An unknown option adds nothing, and the plain sword sends no skill list.
    use_item(&mut world, ObjectId(101));
    advance(&mut world, 1000);
    assert_eq!(sent(&log.take(), "SKILL_LIST"), 0);
}

/// NOTE(java-quirk): the owner of an option's functions is the shared `Options` object, so taking
/// off one item removes the option's functions of the other worn item as well.
#[test]
fn taking_off_one_item_strips_the_shared_option_from_the_other() {
    let (plain, _) = world_with(player(vec![worn(100, 1, pd::RHAND), worn(101, 2, pd::CHEST)]));
    let base_def = p_def(&plain);

    let (mut world, _) = world_with(player(vec![
        augmented(worn(100, 1, pd::RHAND), AUG_DEF),
        augmented(worn(101, 2, pd::CHEST), AUG_DEF),
    ]));
    assert_eq!(p_def(&world), base_def + 10.0);
    use_item(&mut world, SWORD);
    // The breastplate is still worn with its option, but its +5 went with the sword's.
    assert_eq!(world.paperdoll_object(ME, pd::CHEST), Some(CHEST));
    assert_eq!(p_def(&world), base_def);
}

// ---- elements -------------------------------------------------------------------------------------

/// `Elementals.ElementalStatBonus.applyBonus`: a weapon adds `<ELEMENT>_POWER`, an armor
/// `<ELEMENT>_RES`, both as `FuncAdd` of order 0x40; `Item.getAttackElementType` reads the
/// instance when the template has no element.
#[test]
fn element_attributes_of_worn_items_change_the_element_stats() {
    let (mut world, _) = world_with(player(vec![
        with_element(worn(100, 1, pd::RHAND), 1, 40),
        with_element(worn(102, 4, pd::HEAD), 0, 30),
    ]));
    let s = world.stats(ME).unwrap();
    // Water weapon: attack element Water (1) with power 40.
    assert_eq!(s.attack_element(), 1);
    assert_eq!(s.attack_element_value(1), 40);
    // Fire armor: 30 fire resistance, nothing else.
    assert_eq!(s.defense_element_value(0), 30);
    assert_eq!(s.defense_element_value(1), 0);

    // Taking the armor off takes its resistance away.
    use_item(&mut world, HELMET);
    assert_eq!(world.stats(ME).unwrap().defense_element_value(0), 0);
    assert_eq!(world.stats(ME).unwrap().attack_element_value(1), 40);
}

/// `Item.getAttackElementType` reads the template first: a weapon with a template element keeps
/// it, and the instance element still adds its own power.
#[test]
fn a_template_element_wins_over_the_instance_for_the_attack_element() {
    let (world, _) = world_with(player(vec![with_element(worn(100, 3, pd::RHAND), 1, 40)]));
    let s = world.stats(ME).unwrap();
    assert_eq!(s.attack_element(), 0);
    assert_eq!(s.attack_element_value(0), 20);
    assert_eq!(s.attack_element_value(1), 40);
}

// ---- the item packets -----------------------------------------------------------------------------

/// `AbstractItemPacket.writeItem`: the augmentation id and the element fields.
#[test]
fn item_packets_show_the_augmentation_and_the_elements() {
    let (world, _) = world_with(player(vec![
        augmented(with_element(worn(100, 1, pd::RHAND), 1, 40), AUG_ATK_DEF),
        with_element(worn(102, 4, pd::HEAD), 4, 30),
        worn(101, 2, pd::CHEST),
        // The template's element (fire 20) wins; the water 15 of the instance is not shown.
        with_element(item(103, 3, "INVENTORY", 0), 1, 15),
        // A template resistance (water 7) hides the instance's holy 12 for that element.
        with_element(item(104, 5, "INVENTORY", 0), 4, 12),
    ]));

    let sword = item_info_of(&world, SWORD);
    assert_eq!(sword.augmentation, AUG_ATK_DEF);
    assert_eq!((sword.attack_element_type, sword.attack_element_power), (1, 40));
    assert_eq!(sword.element_defense, [0; 6]);

    let helmet = item_info_of(&world, HELMET);
    assert_eq!(helmet.augmentation, 0);
    // An armor has no attack element; its defence element is the instance's holy (4).
    assert_eq!((helmet.attack_element_type, helmet.attack_element_power), (-2, 0));
    assert_eq!(helmet.element_defense, [0, 0, 0, 0, 30, 0]);

    let chest = item_info_of(&world, CHEST);
    assert_eq!((chest.attack_element_type, chest.attack_element_power), (-2, 0));
    assert_eq!(chest.element_defense, [0; 6]);

    let fire = item_info_of(&world, ObjectId(103));
    assert_eq!((fire.attack_element_type, fire.attack_element_power), (0, 20));

    // NOTE(java-quirk): `getElementDefAttr` stops at the template when it has any element.
    let water = item_info_of(&world, ObjectId(104));
    assert_eq!(water.element_defense, [0, 7, 0, 0, 0, 0]);
}

// ---- writing the rows -----------------------------------------------------------------------------

fn upserts(out: &[Out]) -> Vec<ItemData> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemChanges { upserts, .. }) => Some(upserts.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// `Item.setAugmentation` refuses a second augmentation, `removeAugmentation` clears it; both
/// leave the changed item for the persistence task (`item_attributes`).
#[test]
fn augmenting_and_clearing_write_the_item() {
    let (mut world, log) = world_with(player(vec![item(100, 1, "INVENTORY", 0)]));

    assert!(world.set_item_augmentation(SWORD, Some(AUG_DEF)));
    assert!(world.is_item_augmented(SWORD));
    assert_eq!(world.item_augmentation(SWORD), Some(AUG_DEF));
    let written = upserts(&log.take());
    assert_eq!(written.len(), 1);
    assert_eq!(written[0].augmentation, Some(AUG_DEF));

    // "Warning: Augment set for ..." and nothing changes.
    assert!(!world.set_item_augmentation(SWORD, Some(AUG_ATK_DEF)));
    assert_eq!(world.item_augmentation(SWORD), Some(AUG_DEF));
    assert!(upserts(&log.take()).is_empty());

    assert!(world.set_item_augmentation(SWORD, None));
    assert!(!world.is_item_augmented(SWORD));
    assert_eq!(upserts(&log.take())[0].augmentation, None);
}

/// `Item.setElementAttr` / `clearElementAttr`: a new element goes last, an old one is replaced;
/// clearing an element the item does not have writes nothing.
#[test]
fn element_changes_write_the_item() {
    let (mut world, log) = world_with(player(vec![item(100, 1, "INVENTORY", 0)]));

    world.set_item_elemental(SWORD, 2, 30);
    world.set_item_elemental(SWORD, 4, 20);
    world.set_item_elemental(SWORD, 2, 45);
    let written = upserts(&log.take());
    assert_eq!(written.len(), 3);
    assert_eq!(
        written[2].elementals,
        [
            ItemElemental {
                element: 2,
                value: 45
            },
            ItemElemental {
                element: 4,
                value: 20
            }
        ]
    );
    assert_eq!(
        world.item_elemental(SWORD),
        Some(ItemElemental {
            element: 2,
            value: 45
        })
    );

    world.clear_item_elemental(SWORD, 5);
    assert!(upserts(&log.take()).is_empty());
    world.clear_item_elemental(SWORD, 2);
    assert_eq!(
        upserts(&log.take())[0].elementals,
        [ItemElemental {
            element: 4,
            value: 20
        }]
    );
    world.clear_item_elemental(SWORD, -1);
    assert!(upserts(&log.take())[0].elementals.is_empty());
    assert_eq!(world.item_elemental(SWORD), None);
}

// ---- dropping and destroying ------------------------------------------------------------------------

/// The ids of the system messages in the log.
fn message_ids(out: &[Out]) -> Vec<i32> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(b) if b[0] == 0x62 => Some(i32::from_le_bytes([b[1], b[2], b[3], b[4]])),
            _ => None,
        })
        .collect()
}

/// `RequestDropItem`: an augmented item cannot be dropped (`Item.isDropable` follows
/// `AltAllowAugmentTrade`) and says so with its own message.
#[test]
fn an_augmented_item_cannot_be_dropped_unless_trade_is_allowed() {
    let drop = |world: &mut World, object_id: i32| {
        world.handle(WorldCommand::Input {
            session: SESSION,
            input: ClientInput::DropItem {
                object_id,
                count: 1,
                x: 1000,
                y: 1000,
                z: 0,
            },
        });
    };
    let mut bag = item(100, 1, "INVENTORY", 0);
    bag.augmentation = Some(AUG_DEF);
    let plain = item(101, 1, "INVENTORY", 0);
    let (mut world, log) = world_with(player(vec![bag, plain]));

    drop(&mut world, 100);
    assert_eq!(
        message_ids(&log.take()),
        [l2_protocol_game::SystemMessageId::TheAugmentedItemCannotBeDiscarded.id()]
    );
    assert!(
        world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .any(|i| i.object_id == SWORD)
    );
}

/// `RequestDestroyItem`: `Item.isDestroyable` follows `AltAllowAugmentDestroy` for an augmented
/// item (default true).
#[test]
fn an_augmented_item_is_destroyed_only_if_the_config_allows() {
    let destroy = |world: &mut World| {
        world.handle(WorldCommand::Input {
            session: SESSION,
            input: ClientInput::DestroyItem {
                object_id: 100,
                count: 1,
            },
        });
    };
    let mut bag = item(100, 1, "INVENTORY", 0);
    bag.augmentation = Some(AUG_DEF);
    // The test item is not marked undestroyable, so the config is the only thing in the way.
    let (mut world, log) = world_with(player(vec![bag.clone()]));
    world.set_settings(WorldSettings {
        inventory: InventorySettings {
            alt_allow_augment_destroy: false,
            ..InventorySettings::default()
        },
        ..WorldSettings::default()
    });
    destroy(&mut world);
    assert_eq!(
        message_ids(&log.take()),
        [l2_protocol_game::SystemMessageId::ThisItemCannotBeDiscarded.id()]
    );
    assert!(
        world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .any(|i| i.object_id == SWORD)
    );

    let (mut world, log) = world_with(player(vec![bag]));
    destroy(&mut world);
    log.take();
    assert!(
        !world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .any(|i| i.object_id == SWORD)
    );
}
