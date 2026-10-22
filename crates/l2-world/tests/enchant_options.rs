#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Enchant options (M6-ENCHANT-02): what a worn item gives for its enchant level
//! (`Item.applyEnchantStats`), the option ids in the item packets (`Item.getEnchantOptions`), and
//! the `//enchant` page that sets the enchant level of worn items (`AdminEnchant`).
//!
//! Expected numbers come from the Java code named in each test (`Item.java`, `Options.java`,
//! `Inventory.ItemSkillsListener`, `AdminEnchant.java`).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, AugmentationOption, AugmentationOptions, ElementalAttributeData, EnchantHpBonus,
    EnchantItemData, EnchantItemGroups, EnchantItemOptions, Hennas, HtmCache, ItemTemplates, PlayerTemplates,
    SkillTemplates,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll as pd;
use l2_protocol_game::server::{
    ConfirmDlg, InventoryUpdate, NpcHtmlMessage, SystemMessage, SystemMessageParam, UserInfo,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
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
<item id="1" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /><set name="enchant_enabled" val="true" />
<stats><stat type="pAtk">50</stat></stats></item>
<item id="2" type="Armor" name="Breastplate"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="chest" /><set name="weight" val="10" /><set name="enchant_enabled" val="true" />
<stats><stat type="pDef">40</stat></stats></item>
<item id="4" type="Armor" name="Helmet"><set name="armor_type" val="HEAVY" /><set name="bodypart" val="head" /><set name="weight" val="5" /><set name="enchant_enabled" val="true" />
<stats><stat type="pDef">10</stat></stats></item>
<item id="6" type="Armor" name="Necklace"><set name="bodypart" val="neck" /><set name="weight" val="5" /><set name="enchant_enabled" val="true" /></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
</list>"#;

const SKILLS: &str = r##"<list>
<skill id="3007" levels="1" name="Passive"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="critRate" val="11"/></effect></effects></skill>
<skill id="3008" levels="1" name="Active"><operateType>A1</operateType><targetType>SELF</targetType></skill>
</list>"##;

/// Option 1: +10 P. Atk. Option 2: +5 P. Def. and the passive skill 3007. Option 3: the active
/// skill 3008.
const OPTIONS: &str = r#"<list>
<option id="1"><stats><add stat="pAtk" val="10"/></stats></option>
<option id="2"><stats><add stat="pDef" val="5"/></stats><passive_skill id="3007" level="1"/></option>
<option id="3"><active_skill id="3008" level="1"/></option>
</list>"#;

/// The sword has option 1 at +4, and all three at +6. The helmet has option 2 at +3.
const ENCHANT_OPTIONS: &str = r#"<list>
<item id="1">
	<options level="4" option1="1" />
	<options level="6" option1="1" option2="2" option3="3" />
</item>
<item id="4"><options level="3" option2="2" /></item>
</list>"#;

/// Armor: 100 % up to +5. Weapon: 100 % up to +2, then 66 %. The maximum is worked out from them.
const GROUPS: &str = r#"<list>
<enchantRateGroup name="ARMOR_GROUP"><current enchant="0-5" chance="100" /><current enchant="6-15" chance="0" /></enchantRateGroup>
<enchantRateGroup name="FIGHTER_WEAPON_GROUP"><current enchant="0-2" chance="100" /><current enchant="3-15" chance="66" /></enchantRateGroup>
<enchantScrollGroup id="0">
<enchantRate group="ARMOR_GROUP"><item slot="head" /><item slot="chest" /><item slot="neck" /></enchantRate>
<enchantRate group="FIGHTER_WEAPON_GROUP"><item slot="rhand" magicWeapon="false" /></enchantRate>
</enchantScrollGroup>
</list>"#;

struct Data {
    items: Arc<ItemTemplates>,
    stats: StatData,
    options: AugmentationOptions,
    rights: AdminRights,
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
            is_gm: level >= 70,
            ..AccessLevelStatic::USER
        }
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.items.get(item_id).is_some_and(|t| t.stackable)
    }
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.rights)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
    fn augmentation_option(&self, id: i32) -> Option<&AugmentationOption> {
        self.options.get(id)
    }
}

fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (70, 0), (100, 70)] {
        rights.add_level(level, child);
    }
    for command in [
        "enchant", "seteh", "setec", "seteg", "setel", "seteb", "setew", "setes", "setle", "setre", "setlf",
        "setrf", "seten", "setun", "setba", "setbe",
    ] {
        rights.add_command(command, 100, false);
    }
    rights
}

fn data() -> Data {
    data_with(true)
}

/// `with_table` false is a datapack whose `EnchantItemOptions.xml` is empty: the reference for
/// the numbers (the enchant level changes P. Atk by itself).
fn data_with(with_table: bool) -> Data {
    let mut items = ItemTemplates::from_xml(ITEMS).unwrap();
    let skills = SkillTemplates::from_xml(
        SKILLS,
        &l2_data::skill::EnchantSkillGroups::default(),
        &l2_data::SkillLoadOptions::default(),
    );
    let options = AugmentationOptions::from_xml(OPTIONS, &skills).unwrap();
    let enchant_options = EnchantItemOptions::from_xml(
        if with_table {
            ENCHANT_OPTIONS
        } else {
            "<list></list>"
        },
        &items,
        &options,
    );
    items.attach_enchant_options(&enchant_options);
    let items = Arc::new(items);
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
        rights: rights(),
    }
}

// ---- helpers ------------------------------------------------------------------------------------

const ME: ObjectId = ObjectId(1);
const OTHER: ObjectId = ObjectId(2);
const SWORD: ObjectId = ObjectId(100);
const HELMET: ObjectId = ObjectId(102);
const NECKLACE: ObjectId = ObjectId(103);

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

fn enchanted(mut item: ItemData, level: i32) -> ItemData {
    item.enchant_level = level;
    item
}

/// The same options through an augmentation (low half option 1, high half option 2): the numbers
/// are those of `Options.apply`, whatever the stat formulas make of them.
fn augmented(mut item: ItemData, augmentation: i32) -> ItemData {
    item.augmentation = Some(augmentation);
    item
}

fn player(id: ObjectId, name: &str, access_level: i32, items: Vec<ItemData>) -> PlayerData {
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
    _root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Self {
        Self::with_data(data())
    }

    fn with_data(data: Data) -> Self {
        let root = tempfile::tempdir().unwrap();
        let page = root.path().join("data/html/admin/enchant.htm");
        std::fs::create_dir_all(page.parent().unwrap()).unwrap();
        std::fs::write(page, "<html><body>Enchant page</body></html>").unwrap();
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(1), Box::new(NullOutbox));
        world.set_static_data(Box::new(data));
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
        world.set_enchant_data(
            Arc::new(EnchantItemData::from_xml("<list></list>", &items).unwrap()),
            Arc::new(EnchantItemGroups::from_xml(GROUPS).unwrap()),
            Arc::new(ElementalAttributeData::from_xml("<list></list>", &items).unwrap()),
        );
        Self {
            world,
            log,
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
        self.advance(1000);
        self.log.take();
    }

    /// `ME` is a GM (level 100) with `items`.
    fn gm(items: Vec<ItemData>) -> Self {
        let mut s = Self::new();
        s.enter(player(ME, "Me", 100, items));
        s
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn use_item(&mut self, object_id: ObjectId) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::UseItem {
                object_id: object_id.0,
                ctrl_pressed: false,
            },
        });
    }

    fn command(&mut self, command: &str) {
        self.world.admin_command(ME, command, false);
    }

    fn sent(&mut self) -> Sent {
        self.advance(500);
        self.log.take()
    }

    fn p_atk(&self) -> f64 {
        self.world.stats(ME).unwrap().p_atk(None)
    }

    fn p_def(&self) -> f64 {
        self.world.stats(ME).unwrap().p_def(None)
    }

    fn knows(&self, skill_id: i32) -> bool {
        self.world.stats(ME).unwrap().skill_level(skill_id) > 0
    }

    fn enchant_of(&self, who: ObjectId, object_id: ObjectId) -> i32 {
        self.world
            .player(who)
            .unwrap()
            .data
            .items
            .iter()
            .find(|i| i.object_id == object_id)
            .map(|i| i.enchant_level)
            .unwrap()
    }
}

/// `(P. Atk, P. Def)` of a character wearing `items`, in a datapack without enchant options.
fn stats_of(items: Vec<ItemData>) -> (f64, f64) {
    let mut s = Scene::with_data(data_with(false));
    s.enter(player(ME, "Me", 100, items));
    (s.p_atk(), s.p_def())
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn count(out: &Sent, to: ObjectId, name: &str) -> usize {
    out.iter()
        .filter(|(id, b)| *id == to.0 && name_of(b) == name)
        .count()
}

fn decoded<P: Decode>(out: &Sent, to: ObjectId, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(id, b)| *id == to.0 && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

/// The text of every `sendMessage` / `sendSysMessage` (a system message with one string).
fn texts_to(out: &Sent, to: ObjectId) -> Vec<String> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn pages_to(out: &Sent, to: ObjectId) -> Vec<String> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|m| m.html)
        .collect()
}

// ---- the options of a worn item -------------------------------------------------------------------

/// `Item.applyEnchantStats` at login (`Inventory.restore` runs the listeners): the options of the
/// enchant level, nothing at a level without options.
#[test]
fn a_worn_item_gives_the_options_of_its_enchant_level() {
    // The enchant itself adds P. Atk (`FuncEnchant`), so the references wear the same enchant
    // level in a datapack without enchant options and get the options of the table through an
    // augmentation instead (option 1 is +10 P. Atk, option 2 +5 P. Def. and a passive skill,
    // option 3 an active skill).
    let chest = || worn(101, 2, pd::CHEST);
    let sword = || worn(100, 1, pd::RHAND);
    let actual = |sword: ItemData| {
        let s = Scene::gm(vec![sword, chest()]);
        (s.p_atk(), s.p_def())
    };
    let reference = |sword: ItemData| stats_of(vec![sword, chest()]);

    // +4: option 1 (+10 P. Atk).
    assert_eq!(
        actual(enchanted(sword(), 4)),
        reference(enchanted(augmented(sword(), 1), 4))
    );
    assert!(actual(enchanted(sword(), 4)).0 > reference(enchanted(sword(), 4)).0);

    // +5 has no entry in the table.
    assert_eq!(actual(enchanted(sword(), 5)), reference(enchanted(sword(), 5)));

    // +6: all three options: +10 P. Atk, +5 P. Def and the passive skill, the active skill.
    let s = Scene::gm(vec![enchanted(sword(), 6), chest()]);
    assert_eq!(
        (s.p_atk(), s.p_def()),
        reference(enchanted(augmented(sword(), (2 << 16) | 1), 6))
    );
    assert!(s.p_def() > reference(enchanted(sword(), 6)).1);
    assert!(s.knows(3007));
    assert!(s.knows(3008));

    // An item that is not worn gives nothing.
    let s = Scene::gm(vec![enchanted(item(100, 1, "INVENTORY", 0), 6), chest()]);
    assert_eq!((s.p_atk(), s.p_def()), stats_of(vec![chest()]));
    assert!(!s.knows(3007));
}

#[test]
fn putting_the_item_on_and_taking_it_off_applies_and_removes_the_options() {
    let bare = stats_of(vec![]);
    // Options 1 and 2 as an augmentation gives them (the third is an active skill).
    let with_options = stats_of(vec![enchanted(
        augmented(worn(100, 1, pd::RHAND), (2 << 16) | 1),
        6,
    )]);
    let mut s = Scene::gm(vec![enchanted(item(100, 1, "INVENTORY", 0), 6)]);
    assert_eq!((s.p_atk(), s.p_def()), bare);

    s.use_item(SWORD);
    let sent = s.sent();
    assert_eq!((s.p_atk(), s.p_def()), with_options);
    assert!(s.knows(3007) && s.knows(3008));
    // `Options.apply` sends the skill list.
    assert_eq!(count(&sent, ME, "SKILL_LIST"), 1);

    s.use_item(SWORD);
    let sent = s.sent();
    assert_eq!((s.p_atk(), s.p_def()), bare);
    assert!(!s.knows(3007) && !s.knows(3008));
    assert_eq!(count(&sent, ME, "SKILL_LIST"), 1);
}

#[test]
fn an_item_without_options_sends_no_skill_list() {
    let mut s = Scene::gm(vec![enchanted(item(100, 1, "INVENTORY", 0), 5)]);
    s.use_item(SWORD);
    assert_eq!(count(&s.sent(), ME, "SKILL_LIST"), 0);
}

/// `Item.setEnchantLevel` on a worn item: `clearEnchantStats()` and `applyEnchantStats()` around
/// the change, each sending the skill list.
#[test]
fn changing_the_enchant_of_a_worn_item_changes_its_options() {
    let chest = || worn(101, 2, pd::CHEST);
    let plain = stats_of(vec![worn(100, 1, pd::RHAND), chest()]);
    let option_1 = stats_of(vec![enchanted(augmented(worn(100, 1, pd::RHAND), 1), 4), chest()]);
    let options_12 = stats_of(vec![
        enchanted(augmented(worn(100, 1, pd::RHAND), (2 << 16) | 1), 6),
        chest(),
    ]);
    let mut s = Scene::gm(vec![worn(100, 1, pd::RHAND), chest()]);
    assert_eq!((s.p_atk(), s.p_def()), plain);

    s.command("admin_setew 6");
    let sent = s.sent();
    assert_eq!(s.enchant_of(ME, SWORD), 6);
    assert_eq!((s.p_atk(), s.p_def()), options_12);
    assert!(s.knows(3007) && s.knows(3008));
    assert!(count(&sent, ME, "SKILL_LIST") >= 1);

    // +4 keeps option 1 only.
    s.command("admin_setew 4");
    let _ = s.sent();
    assert_eq!((s.p_atk(), s.p_def()), option_1);
    assert!(!s.knows(3007) && !s.knows(3008));

    // +0: nothing.
    s.command("admin_setew 0");
    let _ = s.sent();
    assert_eq!((s.p_atk(), s.p_def()), plain);
}

/// `AbstractItemPacket.writeItem`: the three option ids, low 16 bits each.
#[test]
fn the_item_packets_carry_the_option_ids() {
    let s = Scene::gm(vec![
        enchanted(item(100, 1, "INVENTORY", 0), 6),
        enchanted(item(101, 1, "INVENTORY", 0), 5),
        enchanted(item(102, 4, "INVENTORY", 0), 3),
    ]);
    let player = s.world.player(ME).unwrap();
    let list = l2_world::enter::item_list(&player.data, &data(), 0);
    let options_of = |object_id: i32| {
        list.items
            .iter()
            .find(|i| i.object_id == object_id)
            .map(|i| i.enchant_options)
            .unwrap()
    };
    assert_eq!(options_of(100), [1, 2, 3]);
    // No entry for +5, no options.
    assert_eq!(options_of(101), [0, 0, 0]);
    assert_eq!(options_of(102), [0, 2, 0]);
}

// ---- //enchant ------------------------------------------------------------------------------------

#[test]
fn enchant_shows_the_page() {
    let mut s = Scene::gm(vec![]);
    s.command("admin_enchant");
    assert_eq!(
        pages_to(&s.sent(), ME),
        vec!["<html><body>Enchant page</body></html>"]
    );
}

#[test]
fn a_slot_command_sets_the_enchant_of_the_worn_item() {
    let mut s = Scene::gm(vec![worn(102, 4, pd::HEAD)]);
    s.command("admin_seteh 6");
    let sent = s.sent();
    assert_eq!(s.enchant_of(ME, HELMET), 6);

    // The GM and the owner are the same player: both texts.
    assert_eq!(
        texts_to(&sent, ME),
        vec![
            "Changed enchantment of Me's Helmet from 0 to 6.",
            "Admin has changed the enchantment of your Helmet from 0 to 6."
        ]
    );
    // The inventory update (the modified item) and the user info, then the page.
    let updates: Vec<InventoryUpdate> = decoded(&sent, ME, "INVENTORY_UPDATE");
    assert_eq!(updates.len(), 1);
    assert_eq!(updates[0].updates.len(), 1);
    assert_eq!(updates[0].updates[0].item.object_id, HELMET.0);
    assert_eq!(updates[0].updates[0].item.enchant, 6);
    assert!(count(&sent, ME, "USER_INFO") >= 1);
    let _: Vec<UserInfo> = decoded(&sent, ME, "USER_INFO");
    assert_eq!(pages_to(&sent, ME).len(), 1);
    // The item is worn again, in its slot.
    assert_eq!(s.world.paperdoll_object(ME, pd::HEAD), Some(HELMET));
}

#[test]
fn every_word_sets_its_slot() {
    let mut s = Scene::gm(vec![
        worn(100, 1, pd::RHAND),
        worn(103, 6, pd::NECK),
        worn(102, 4, pd::HEAD),
    ]);
    s.command("admin_setew 3");
    s.command("admin_seten 2");
    s.command("admin_seteh 1");
    assert_eq!(s.enchant_of(ME, SWORD), 3);
    assert_eq!(s.enchant_of(ME, NECKLACE), 2);
    assert_eq!(s.enchant_of(ME, HELMET), 1);
}

#[test]
fn bad_values_are_told_and_the_page_still_shows() {
    let mut s = Scene::gm(vec![worn(102, 4, pd::HEAD)]);
    for (command, text) in [
        ("admin_seteh", "Please specify a new enchant value."),
        ("admin_seteh ", "Please specify a valid new enchant value."),
        ("admin_seteh abc", "Please specify a valid new enchant value."),
        ("admin_seteh 6 ", "Please specify a valid new enchant value."),
        (
            "admin_seteh 99999999999",
            "Please specify a valid new enchant value.",
        ),
        (
            "admin_seteh -1",
            "You must set the enchant level to be between 0-65535.",
        ),
        (
            "admin_seteh 65536",
            "You must set the enchant level to be between 0-65535.",
        ),
    ] {
        s.command(command);
        let sent = s.sent();
        assert_eq!(texts_to(&sent, ME), vec![text], "{command}");
        assert_eq!(pages_to(&sent, ME).len(), 1, "{command}");
        assert_eq!(s.enchant_of(ME, HELMET), 0, "{command}");
    }
}

#[test]
fn an_empty_slot_only_shows_the_page() {
    let mut s = Scene::gm(vec![]);
    s.command("admin_seteh 6");
    let sent = s.sent();
    assert!(texts_to(&sent, ME).is_empty());
    assert_eq!(pages_to(&sent, ME).len(), 1);
}

#[test]
fn the_over_enchant_protection_caps_a_player_but_not_a_gm() {
    let groups = EnchantItemGroups::from_xml(GROUPS).unwrap();
    let (weapon, armor) = (groups.max_weapon_enchant(), groups.max_armor_enchant());

    // A GM's own items are not capped.
    let mut s = Scene::gm(vec![worn(100, 1, pd::RHAND)]);
    s.command(&format!("admin_setew {}", weapon + 5));
    assert_eq!(s.enchant_of(ME, SWORD), weapon + 5);

    // The target is a player: capped, with the message for the kind of item.
    let mut s = Scene::gm(vec![]);
    s.enter(player(
        OTHER,
        "Bob",
        0,
        vec![worn(100, 1, pd::RHAND), worn(102, 4, pd::HEAD)],
    ));
    s.world.set_target(ME, Some(OTHER));
    s.command(&format!("admin_setew {}", weapon + 5));
    let sent = s.sent();
    assert_eq!(s.enchant_of(OTHER, SWORD), weapon);
    assert_eq!(
        texts_to(&sent, ME),
        vec![
            format!("Maximum enchantment for weapon items is {weapon}."),
            format!("Changed enchantment of Bob's Sword from 0 to {weapon}."),
        ]
    );
    assert_eq!(
        texts_to(&sent, OTHER),
        vec![format!(
            "Admin has changed the enchantment of your Sword from 0 to {weapon}."
        )]
    );

    s.command(&format!("admin_seteh {}", armor + 1));
    let sent = s.sent();
    assert_eq!(s.enchant_of(OTHER, HELMET), armor);
    assert_eq!(
        texts_to(&sent, ME)[0],
        format!("Maximum enchantment for armor items is {armor}.")
    );
}

#[test]
fn an_accessory_has_its_own_maximum() {
    let groups = EnchantItemGroups::from_xml(GROUPS).unwrap();
    let accessory = groups.max_accessory_enchant();
    let mut s = Scene::gm(vec![]);
    s.enter(player(OTHER, "Bob", 0, vec![worn(103, 6, pd::NECK)]));
    s.world.set_target(ME, Some(OTHER));
    s.command(&format!("admin_seten {}", accessory + 3));
    let sent = s.sent();
    assert_eq!(s.enchant_of(OTHER, NECKLACE), accessory);
    assert_eq!(
        texts_to(&sent, ME)[0],
        format!("Maximum enchantment for accessory items is {accessory}.")
    );
}

#[test]
fn the_commands_need_the_right_to_run() {
    // A player (level 0) who is no GM cannot run them at all.
    let mut s = Scene::new();
    s.enter(player(ME, "Me", 0, vec![worn(102, 4, pd::HEAD)]));
    s.command("admin_seteh 6");
    assert_eq!(s.enchant_of(ME, HELMET), 0);
    let _ = ConfirmDlg::new(SystemMessageId::S13);
}
