#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Item handlers (M5-ITEM-01): `UseItem` for items that are not worn. Potions heal over time and
//! share a reuse group, a herb works when it is picked up (or auto-looted) and never enters the
//! inventory, a scroll goes through `useMagic` and pays with its skill, a box gives its
//! `<capsuled_items>`, a book shows its page.
//!
//! The items and skills are copies of the datapack's (comments and unrelated levels left out) so
//! the tests run without it; `the_whole_datapack_loads_its_capsuled_items` checks the real files
//! when they are there. Expected behaviour comes from the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    ExUseSharedGroupItem, InventoryUpdate, NpcHtmlMessage, ShowCalculator, ShowMiniMap, SystemMessage,
    SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Vec<u8>>>>);

impl Log {
    fn take(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.0.0.lock().unwrap().push(bytes);
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct NoPersist;

impl Outbox for NoPersist {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

/// Keeps what the world persists (the item reuse tests read the stores).
#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Outbox for Persisted {
    fn persist(&mut self, command: PersistCommand) {
        self.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
    /// The last full store of a character.
    fn last_store(&self) -> PlayerData {
        let commands = self.0.lock().unwrap();
        commands
            .iter()
            .rev()
            .find_map(|c| match c {
                PersistCommand::StorePlayer(data) => Some((**data).clone()),
                _ => None,
            })
            .expect("a store")
    }
}

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

/// 1060 and 736 are the datapack's Lesser Healing Potion and Scroll of Escape, 8606 its Herb of
/// Power and 8600 its Herb of Life. 1061 is the Greater Healing Potion with a shared reuse group
/// added (the shipped one has none) and 1062 another item of that group, 9100 to 9103 are boxes made for one rule each, 5000 a book.
const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="2" type="Weapon" name="C Sword"><set name="crystal_type" val="C" /><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" /></item>
<item id="1060" type="EtcItem" name="Lesser Healing Potion">
	<set name="default_action" val="SKILL_REDUCE" />
	<set name="etcitem_type" val="POTION" />
	<set name="immediate_effect" val="true" />
	<set name="weight" val="5" />
	<set name="reuse_delay" val="10000" />
	<set name="is_stackable" val="true" />
	<set name="handler" val="ItemSkills" />
	<set name="for_npc" val="true" />
	<conditions msgId="113" addName="1"><player flyMounted="false" /></conditions>
	<skills><skill id="2031" level="1" /></skills>
</item>
<item id="1061" type="EtcItem" name="Greater Healing Potion">
	<set name="default_action" val="SKILL_REDUCE" />
	<set name="etcitem_type" val="POTION" />
	<set name="immediate_effect" val="true" />
	<set name="weight" val="5" />
	<set name="reuse_delay" val="10000" />
	<set name="shared_reuse_group" val="4" />
	<set name="is_stackable" val="true" />
	<set name="handler" val="ItemSkills" />
	<skills><skill id="2032" level="1" /></skills>
</item>
<item id="1062" type="EtcItem" name="Other Potion of the Group">
	<set name="default_action" val="SKILL_REDUCE" />
	<set name="etcitem_type" val="POTION" />
	<set name="immediate_effect" val="true" />
	<set name="reuse_delay" val="10000" />
	<set name="shared_reuse_group" val="4" />
	<set name="is_stackable" val="true" />
	<set name="handler" val="ItemSkills" />
	<skills><skill id="2031" level="1" /></skills>
</item>
<item id="736" type="EtcItem" name="Scroll of Escape">
	<set name="default_action" val="SKILL_REDUCE" />
	<set name="etcitem_type" val="SCROLL" />
	<set name="weight" val="120" />
	<set name="is_stackable" val="true" />
	<set name="handler" val="ItemSkills" />
	<skills><skill id="2013" level="1" /></skills>
</item>
<item id="8606" type="EtcItem" name="Herb of Power">
	<set name="immediate_effect" val="true" />
	<set name="ex_immediate_effect" val="true" />
	<set name="is_depositable" val="false" />
	<set name="handler" val="ItemSkills" />
	<set name="for_npc" val="true" />
	<skills><skill id="2280" level="1" /></skills>
</item>
<item id="8600" type="EtcItem" name="Herb of Life">
	<set name="immediate_effect" val="true" />
	<set name="ex_immediate_effect" val="true" />
	<set name="handler" val="ItemSkills" />
	<skills><skill id="2278" level="1" /></skills>
</item>
<item id="1665" type="EtcItem" name="Map of Gludin">
	<set name="immediate_effect" val="true" />
	<set name="handler" val="Maps" />
</item>
<item id="4393" type="EtcItem" name="Calculator">
	<set name="default_action" val="CALC" />
	<set name="handler" val="Calculator" />
</item>
<item id="9100" type="EtcItem" name="Everything Box">
	<set name="default_action" val="PEEL" />
	<set name="immediate_effect" val="true" />
	<set name="handler" val="ExtractableItems" />
	<capsuled_items>
		<item id="57" min="100" max="100" chance="100" />
		<item id="1060" min="5" max="5" chance="100" />
		<item id="2" min="2" max="2" chance="100" minEnchant="3" maxEnchant="3" />
	</capsuled_items>
</item>
<item id="9101" type="EtcItem" name="Pick One Box">
	<set name="default_action" val="PEEL" />
	<set name="immediate_effect" val="true" />
	<set name="handler" val="ExtractableItems" />
	<set name="extractableCountMax" val="1" />
	<capsuled_items>
		<item id="57" min="10" max="10" chance="100" />
		<item id="1060" min="5" max="5" chance="100" />
	</capsuled_items>
</item>
<item id="9102" type="EtcItem" name="Two Different Box">
	<set name="default_action" val="PEEL" />
	<set name="immediate_effect" val="true" />
	<set name="handler" val="ExtractableItems" />
	<set name="extractableCountMin" val="2" />
	<set name="extractableCountMax" val="2" />
	<capsuled_items>
		<item id="57" min="10" max="10" chance="30" />
		<item id="1060" min="5" max="5" chance="30" />
		<item id="1061" min="1" max="1" chance="30" />
	</capsuled_items>
</item>
<item id="9103" type="EtcItem" name="Empty Box">
	<set name="default_action" val="PEEL" />
	<set name="handler" val="ExtractableItems" />
</item>
<item id="5000" type="EtcItem" name="Help Book"><set name="handler" val="Book" /></item>
<item id="5001" type="EtcItem" name="Odd Thing"><set name="handler" val="NoSuchHandler" /></item>
</list>"#;

/// 2031 and 2032 are the datapack's potions (HP recover, 15 s, three ticks), 2013 the Scroll of
/// Escape with a `Teleport` effect instead of `Escape` (which needs the map regions), 2280 a
/// herb, 2278 the Herb of Life (`HealPercent`).
const SKILLS: &str = r##"<list>
<skill id="2031" levels="1" name="Lesser Healing Potion">
	<operateType>A2</operateType><targetType>SELF</targetType>
	<abnormalLevel>1</abnormalLevel><abnormalTime>15</abnormalTime><abnormalType>HP_RECOVER</abnormalType>
	<isMagic>2</isMagic><magicLevel>1</magicLevel><reuseDelay>10000</reuseDelay>
	<effects><effect name="HealOverTime" ticks="3"><power>8</power></effect></effects>
</skill>
<skill id="2032" levels="1" name="Healing Potion">
	<operateType>A2</operateType><targetType>SELF</targetType>
	<abnormalLevel>2</abnormalLevel><abnormalTime>15</abnormalTime><abnormalType>HP_RECOVER</abnormalType>
	<isMagic>2</isMagic><magicLevel>1</magicLevel><reuseDelay>10000</reuseDelay>
	<effects><effect name="HealOverTime" ticks="3"><power>14</power></effect></effects>
</skill>
<skill id="2013" levels="1" name="Scroll of Escape">
	<operateType>A1</operateType><targetType>SELF</targetType>
	<hitTime>20000</hitTime><isMagic>2</isMagic>
	<itemConsumeCount>1</itemConsumeCount><itemConsumeId>736</itemConsumeId>
	<magicLevel>1</magicLevel>
	<effects><effect name="Teleport"><x>-70000</x><y>258000</y><z>-3100</z></effect></effects>
</skill>
<skill id="2280" levels="1" name="Herb of Power">
	<operateType>A2</operateType><targetType>SELF</targetType>
	<abnormalInstant>true</abnormalInstant><abnormalLevel>3</abnormalLevel><abnormalTime>300</abnormalTime>
	<abnormalType>PA_UP</abnormalType><effectPoint>100</effectPoint><hitTime>100</hitTime>
	<isMagic>2</isMagic><magicLevel>40</magicLevel><reuseDelay>2000</reuseDelay>
	<effects><effect name="Buff"><mul stat="pAtk" val="1.25" /></effect></effects>
</skill>
<skill id="2278" levels="1" name="Superior Herb of Life">
	<operateType>A1</operateType><targetType>SELF</targetType>
	<effectPoint>100</effectPoint><hitTime>100</hitTime><isMagic>2</isMagic><isRecoveryHerb>true</isRecoveryHerb>
	<magicLevel>1</magicLevel><reuseDelay>2000</reuseDelay>
	<effects><effect name="HealPercent"><power>15</power></effect></effects>
</skill>
</list>"##;

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
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    };
    Data { items, stats }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);

fn item(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        enchant_level: 0,
        loc: "INVENTORY".to_owned(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

fn player_data(items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: ME,
        account: "acc".to_owned(),
        name: "Hero".to_owned(),
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
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
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
    me: Log,
    /// What the character was sent while it entered (the log is emptied after that).
    entered: Vec<Vec<u8>>,
    /// Keeps the html folder of `with_help_page` alive.
    _html: Option<tempfile::TempDir>,
}

impl Scene {
    fn new(items: Vec<ItemData>) -> Self {
        Self::build(player_data(items), WorldSettings::default(), None)
    }

    fn with_items(items: Vec<ItemData>, settings: WorldSettings, html: Option<tempfile::TempDir>) -> Self {
        Self::build(player_data(items), settings, html)
    }

    fn build(me_data: PlayerData, settings: WorldSettings, html: Option<tempfile::TempDir>) -> Self {
        Self::build_with(me_data, settings, html, Box::new(NoPersist))
    }

    fn build_with(
        me_data: PlayerData,
        settings: WorldSettings,
        html: Option<tempfile::TempDir>,
        outbox: Box<dyn Outbox>,
    ) -> Self {
        let mut world = World::new(WorldRng::from_seed(7), outbox);
        world.set_static_data(Box::new(data()));
        world.set_settings(settings);
        world.set_regen_settings(RegenSettings {
            mp_regen_multiplier: 0.0,
            hp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml("<list></list>", &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        if let Some(dir) = &html {
            world.set_html_cache(Arc::new(l2_data::HtmCache::load(dir.path(), false)));
        }
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(me_data),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let mut scene = Self {
            world,
            me,
            entered: Vec::new(),
            _html: html,
        };
        scene.advance(1000);
        scene.entered = scene.me.take();
        scene
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn use_item(&mut self, object_id: i32) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::UseItem {
                object_id,
                ctrl_pressed: false,
            },
        });
    }

    fn hp(&self) -> f64 {
        self.world.player(ME).unwrap().core.cur_hp
    }

    /// `(item id, count)` of the inventory, sorted.
    fn inventory(&self) -> Vec<(i32, i64)> {
        let mut items: Vec<(i32, i64)> = self
            .world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.loc == "INVENTORY")
            .map(|i| (i.item_id, i.count))
            .collect();
        items.sort_unstable();
        items
    }

    fn count_of(&self, item_id: i32) -> i64 {
        self.inventory()
            .iter()
            .find(|(i, _)| *i == item_id)
            .map_or(0, |(_, c)| *c)
    }

    fn position(&self) -> (i32, i32) {
        let core = &self.world.player(ME).unwrap().core;
        (core.x, core.y)
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn names(packets: &[Vec<u8>]) -> Vec<String> {
    packets.iter().map(|p| name_of(p)).collect()
}

fn decode<P: Decode>(packet: &[u8]) -> P {
    let skip = if packet[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&packet[skip..])).unwrap()
}

fn of(packets: &[Vec<u8>], opcode: ServerOpcode) -> Vec<&Vec<u8>> {
    packets.iter().filter(|p| name_of(p) == opcode.name()).collect()
}

fn messages(packets: &[Vec<u8>]) -> Vec<SystemMessage> {
    of(packets, ServerOpcode::SystemMessage)
        .into_iter()
        .map(|p| decode::<SystemMessage>(p))
        .collect()
}

fn message_ids(packets: &[Vec<u8>]) -> Vec<SystemMessageId> {
    messages(packets).into_iter().map(|m| m.id).collect()
}

// ---- potions -----------------------------------------------------------------------------------

/// `ItemSkillsTemplate`: the potion's static skill is cast simultaneously (an immediate-effect
/// item with a static skill), `HealOverTime` heals every tick of 3 * 666 ms / 1000 = 1.998 s, and
/// `checkConsume` uses the potion up (`SKILL_REDUCE`, immediate effect, no consuming skill).
#[test]
fn a_healing_potion_heals_over_time_and_is_used_up() {
    let mut s = Scene::new(vec![item(0x100, 1060, 3)]);
    s.world.set_current_hp(ME, 10.0, false);
    s.me.take();

    s.use_item(0x100);
    s.advance(100);
    let sent = s.me.take();
    // The potion is used up at once, not when the effect ends.
    assert_eq!(s.count_of(1060), 2);
    assert!(
        names(&sent).contains(&ServerOpcode::MagicSkillUse.name().to_owned()),
        "{:?}",
        names(&sent)
    );
    assert!(message_ids(&sent).contains(&SystemMessageId::UseS1));
    // The effect is on, and nothing is healed yet.
    assert!(s.world.is_affected_by_skill(ME, 2031));
    assert_eq!(s.hp(), 10.0);

    let per_tick = 8.0 * f64::from((3_i64 * 666) as f32 / 1000.0_f32);
    s.advance(2_100);
    assert_eq!(s.hp(), 10.0 + per_tick);
    s.advance(2_000);
    assert_eq!(s.hp(), 10.0 + 2.0 * per_tick);
}

/// `UseItem`: the item's reuse delay (10 s) is stamped after the handler used it; a second click
/// is refused with the remaining time and costs nothing.
#[test]
fn a_potion_in_reuse_tells_the_time_left_and_is_kept() {
    let mut s = Scene::new(vec![item(0x100, 1060, 3)]);
    s.use_item(0x100);
    s.advance(100);
    s.me.take();

    s.advance(3_000);
    s.me.take();
    s.use_item(0x100);
    let sent = s.me.take();
    assert_eq!(s.count_of(1060), 2);
    let all = messages(&sent);
    assert_eq!(all.len(), 1, "{all:?}");
    // Java: remaining 6900 ms: "There are $s2 second(s) remaining in $s1's re-use time".
    assert_eq!(
        all[0].id,
        SystemMessageId::ThereAreS2SecondSRemainingInS1SReUseTime
    );
    assert_eq!(all[0].params[0], SystemMessageParam::ItemName(1060));
    let SystemMessageParam::IntNumber(seconds) = all[0].params[1] else {
        panic!("{:?}", all[0].params);
    };
    assert!((6..=7).contains(&seconds), "{seconds}");

    // After the 10 s it works again.
    s.advance(8_000);
    s.me.take();
    s.use_item(0x100);
    assert_eq!(s.count_of(1060), 1);
}

/// `UseItem.sendSharedGroupUpdate`: an item of a shared reuse group starts the group for every
/// item of it, and the client's timer (`ExUseSharedGroupItem`) is told, also when another item of
/// the group is refused.
#[test]
fn a_shared_reuse_group_blocks_the_other_items_of_the_group() {
    let mut s = Scene::new(vec![item(0x100, 1061, 2), item(0x101, 1062, 2)]);
    s.me.take();
    s.use_item(0x100);
    let sent = s.me.take();
    let group: Vec<ExUseSharedGroupItem> = of(&sent, ServerOpcode::ExUseSharedGroupItem)
        .into_iter()
        .map(|p| decode(p))
        .collect();
    assert_eq!(
        group,
        [ExUseSharedGroupItem {
            item_id: 1061,
            group_id: 4,
            remaining_secs: 10,
            total_secs: 10
        }]
    );
    assert_eq!(s.count_of(1061), 1);

    // The other item of the group is refused with the time left and the group update.
    s.advance(2_000);
    s.me.take();
    s.use_item(0x101);
    let sent = s.me.take();
    assert_eq!(s.count_of(1062), 2);
    let all = messages(&sent);
    assert_eq!(
        all.iter().map(|m| m.id).collect::<Vec<_>>(),
        [SystemMessageId::ThereAreS2SecondSRemainingInS1SReUseTime]
    );
    assert_eq!(all[0].params[0], SystemMessageParam::ItemName(1062));
    let group: ExUseSharedGroupItem = decode(of(&sent, ServerOpcode::ExUseSharedGroupItem)[0]);
    assert_eq!((group.item_id, group.group_id, group.total_secs), (1062, 4, 10));
    assert!((7..=8).contains(&group.remaining_secs), "{group:?}");

    // After the 10 s the group is free again.
    s.advance(9_000);
    s.me.take();
    s.use_item(0x101);
    assert_eq!(s.count_of(1062), 1);
}

/// A potion can be drunk while a spell is cast (`ItemSkillsTemplate`: potions, elixirs and scrolls
/// skip the `isCastingNow` refusal): the scroll below is being cast for 20 s.
#[test]
fn a_potion_works_while_a_scroll_is_being_cast() {
    let mut s = Scene::new(vec![item(0x100, 1060, 1), item(0x101, 736, 3)]);
    s.world.set_current_hp(ME, 10.0, false);
    s.use_item(0x101);
    s.advance(500);
    assert!(s.world.is_casting_now(ME));
    s.me.take();

    s.use_item(0x100);
    s.advance(100);
    assert_eq!(s.count_of(1060), 0);
    assert!(s.world.is_affected_by_skill(ME, 2031));
}

// ---- scrolls -----------------------------------------------------------------------------------

/// A scroll goes through `useMagic`: the cast takes the skill's 20 s (a static skill has no
/// speed bonus), the skill takes the scroll when the cast begins (`itemConsumeId`), the handler
/// does not use it up a second time (`hasConsumeSkill`), and the effect lands when the cast ends.
#[test]
fn a_scroll_casts_its_skill_and_the_skill_takes_the_scroll() {
    let mut s = Scene::new(vec![item(0x100, 736, 3)]);
    s.use_item(0x100);
    s.advance(200);
    let sent = s.me.take();
    let ids = message_ids(&sent);
    assert!(ids.contains(&SystemMessageId::UseS1), "{ids:?}");
    let use_packet: l2_protocol_game::server::MagicSkillUse =
        decode(of(&sent, ServerOpcode::MagicSkillUse)[0]);
    assert_eq!((use_packet.skill_id, use_packet.hit_time), (2013, 20_000));
    // One scroll, taken by the skill.
    assert_eq!(s.count_of(736), 2);
    assert_eq!(s.position(), (X, Y));

    s.advance(19_000);
    assert_eq!(s.position(), (X, Y), "still casting");
    s.advance(1_500);
    // `Teleport` without a random offset of its own lands within 50 of the point (`teleToLocation`
    // spreads the arrival).
    let (x, y) = s.position();
    assert!((x + 70_000).abs() <= 50 && (y - 258_000).abs() <= 50, "{x} {y}");
    assert_eq!(s.count_of(736), 2);
}

/// `UseItem`: with `AltKarmaPlayerCanTeleport` off, a player with karma cannot use an item whose
/// skill has a teleport effect: nothing happens, no message.
#[test]
fn a_player_with_karma_cannot_use_a_teleport_scroll_when_the_config_forbids_it() {
    let mut settings = WorldSettings::default();
    settings.cast.alt_game_karma_player_can_teleport = false;
    let mut me = player_data(vec![item(0x100, 736, 3)]);
    me.karma = 5;
    let mut s = Scene::build(me, settings, None);
    s.me.take();
    s.use_item(0x100);
    s.advance(200);
    assert_eq!(s.count_of(736), 3);
    assert!(s.me.take().is_empty());
}

// ---- herbs -------------------------------------------------------------------------------------

fn put_on_the_ground(s: &mut Scene, object_id: i32, item_id: i32) -> ObjectId {
    s.world.load_ground_items(vec![GroundItemRow {
        object_id: ObjectId(object_id),
        item_id,
        count: 1,
        enchant_level: 0,
        x: X,
        y: Y,
        z: Z,
        drop_time: -1,
        equipable: false,
    }]);
    ObjectId(object_id)
}

/// `Player.doPickupItem`: a herb with `ex_immediate_effect` runs its handler on the spot, the
/// buff lands (a simultaneous cast of 100 ms) and the herb never enters the inventory.
#[test]
fn a_herb_gives_its_buff_when_it_is_picked_up_and_vanishes() {
    let mut s = Scene::new(vec![]);
    let herb = put_on_the_ground(&mut s, 0x6000_0100, 8606);
    s.me.take();

    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::Action {
            object_id: herb.0,
            action_id: 0,
        },
    });
    s.advance(1_000);
    let sent = s.me.take();
    assert!(names(&sent).contains(&ServerOpcode::GetItem.name().to_owned()));
    assert!(s.world.ground_item(herb).is_none());
    assert_eq!(s.inventory(), []);
    // The herb's skill ran as a cast of its own (`MagicSkillUse`), and its buff is on.
    let skill_use: l2_protocol_game::server::MagicSkillUse =
        decode(of(&sent, ServerOpcode::MagicSkillUse)[0]);
    assert_eq!(skill_use.skill_id, 2280);
    assert!(s.world.is_affected_by_skill(ME, 2280));
}

/// Java `Player.addItem(process, itemId, count, ...)`: loot that goes straight into the
/// inventory (auto loot, quest rewards) casts the herb's skills instead of adding it.
#[test]
fn a_herb_added_to_the_inventory_is_cast_instead() {
    let mut s = Scene::new(vec![]);
    s.world.set_current_hp(ME, 10.0, false);
    s.me.take();
    assert!(s.world.add_item(ME, 8600, 1, true).is_none());
    s.advance(1_000);
    let sent = s.me.take();
    assert_eq!(s.inventory(), []);
    // "You have obtained Herb of Life" was sent, and 15 % of the maximum HP healed.
    assert!(message_ids(&sent).contains(&SystemMessageId::YouHaveObtainedS1));
    assert_eq!(s.hp(), 10.0 + 15.0);
}

// ---- boxes -------------------------------------------------------------------------------------

/// `ExtractableItems`: every product that rolls is created (`Rnd.get(100000) <= chance`; 100 % is
/// 100000), a stackable product as one stack and a non-stackable one as separate items with the
/// enchant level of the product; the box is used up with "$s1 has disappeared", and every product
/// is named once.
#[test]
fn a_box_gives_its_products() {
    let mut s = Scene::new(vec![item(0x100, 9100, 2)]);
    s.me.take();
    s.use_item(0x100);
    s.advance(300);
    let sent = s.me.take();

    assert_eq!(s.count_of(9100), 1);
    assert_eq!(s.count_of(57), 100);
    assert_eq!(s.count_of(1060), 5);
    // Two swords, not one stack, each +3.
    let swords: Vec<_> = s
        .world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == 2)
        .collect();
    assert_eq!(swords.len(), 2);
    assert!(swords.iter().all(|i| i.enchant_level == 3 && i.count == 1));

    let all = messages(&sent);
    let ids: Vec<SystemMessageId> = all.iter().map(|m| m.id).collect();
    assert_eq!(ids[0], SystemMessageId::S1HasDisappeared);
    assert_eq!(all[0].params[0], SystemMessageParam::ItemName(9100));
    // 100 Adena, 5 potions, then the two +3 swords, one message each.
    assert_eq!(
        ids[1..],
        [
            SystemMessageId::YouHaveObtainedS2S1,
            SystemMessageId::YouHaveObtainedS2S1,
            SystemMessageId::YouHaveObtainedAS1S2,
            SystemMessageId::YouHaveObtainedAS1S2,
        ]
    );
    assert_eq!(all[1].params[1], SystemMessageParam::LongNumber(100));
    assert_eq!(all[3].params[0], SystemMessageParam::IntNumber(3));
    // The enchanted swords are sent as modified items.
    let update: InventoryUpdate = decode(of(&sent, ServerOpcode::InventoryUpdate).last().unwrap());
    assert!(update.updates.iter().any(|u| u.item.enchant == 3));
}

/// `extractableCountMax`: the loop stops once that many different items came out.
#[test]
fn a_box_with_a_count_max_stops_there() {
    let mut s = Scene::new(vec![item(0x100, 9101, 1)]);
    s.use_item(0x100);
    s.advance(300);
    assert_eq!(s.count_of(57), 10);
    assert_eq!(s.count_of(1060), 0);
}

/// `extractableCountMin`: the products are rolled again until that many different items came
/// out, never the same item twice (`alreadyExtracted`); a seed with a low chance still ends up
/// with two.
#[test]
fn a_box_with_a_count_min_rolls_until_it_has_that_many_items() {
    for seed in 0..20 {
        let mut s = Scene::new(vec![item(0x100, 9102, 1)]);
        s.world = {
            // A different draw each time: the world's seed is its RNG.
            let mut w = s.world;
            *w.rng() = WorldRng::from_seed(seed);
            w
        };
        s.use_item(0x100);
        s.advance(300);
        let got: Vec<i32> = [57, 1060, 1061]
            .into_iter()
            .filter(|id| s.count_of(*id) > 0)
            .collect();
        assert_eq!(got.len(), 2, "seed {seed}: {got:?}");
    }
}

/// `RateExtractable` multiplies the amounts (`(long) (min * rate)`).
#[test]
fn the_extractable_rate_multiplies_the_amounts() {
    let mut settings = WorldSettings::default();
    settings.inventory.rate_extractable = 2.5;
    let mut s = Scene::with_items(vec![item(0x100, 9101, 1)], settings, None);
    s.use_item(0x100);
    s.advance(300);
    assert_eq!(s.count_of(57), 25);
}

/// A box without `<capsuled_items>` is not used and keeps its item (Java logs it).
#[test]
fn a_box_without_data_is_not_used() {
    let mut s = Scene::new(vec![item(0x100, 9103, 1)]);
    s.use_item(0x100);
    s.advance(300);
    assert_eq!(s.count_of(9103), 1);
}

/// `isInventoryUnder80(false)`: a nearly full inventory refuses to open the box.
#[test]
fn a_box_is_refused_when_the_inventory_is_nearly_full() {
    let mut items = vec![item(0x100, 9101, 1)];
    // 64 more slots: with 65 of the 80 slots a player without the dwarf bonus is at 81 %.
    for n in 0..64 {
        items.push(item(0x200 + n, 2, 1));
    }
    let mut s = Scene::new(items);
    s.me.take();
    s.use_item(0x100);
    s.advance(300);
    assert_eq!(s.count_of(9101), 1);
    assert_eq!(s.count_of(57), 0);
}

// ---- other handlers ----------------------------------------------------------------------------

/// `Book`: the page `data/html/help/<itemId>.htm` as an `NpcHtmlMessage` of the item, then
/// `ActionFailed`; without the file Java shows a "My Text is missing" page.
/// Java `Maps.onItemUse` and `Calculator.onItemUse`: the item's id goes to the client, which opens
/// the window. Neither item has a reuse delay or is used up.
#[test]
fn a_map_and_a_calculator_open_their_windows() {
    let mut s = Scene::new(vec![item(0x100, 1665, 1), item(0x101, 4393, 1)]);
    s.me.take();

    s.use_item(0x100);
    let sent = s.me.take();
    let maps = of(&sent, ServerOpcode::ShowMinimap);
    assert_eq!(maps.len(), 1, "{:?}", names(&sent));
    assert_eq!(
        decode::<ShowMiniMap>(maps[0]),
        ShowMiniMap {
            map_id: 1665,
            seven_signs_period: 0
        }
    );

    s.use_item(0x101);
    let sent = s.me.take();
    let calc = of(&sent, ServerOpcode::ShowCalc);
    assert_eq!(calc.len(), 1, "{:?}", names(&sent));
    assert_eq!(
        decode::<ShowCalculator>(calc[0]),
        ShowCalculator { calculator_id: 4393 }
    );

    assert_eq!(s.inventory(), [(1665, 1), (4393, 1)]);
}

#[test]
fn a_book_shows_its_help_page() {
    let dir = tempfile::tempdir().unwrap();
    let help = dir.path().join("data/html/help");
    std::fs::create_dir_all(&help).unwrap();
    std::fs::write(help.join("5000.htm"), "<html><body>Read me</body></html>").unwrap();
    let mut s = Scene::with_items(vec![item(0x100, 5000, 1)], WorldSettings::default(), Some(dir));
    s.me.take();
    s.use_item(0x100);
    let sent = s.me.take();
    let page: NpcHtmlMessage = decode(of(&sent, ServerOpcode::NpcHtmlMessage)[0]);
    assert_eq!((page.npc_obj_id, page.item_id), (0, 5000));
    assert!(page.html.contains("Read me"));
    assert_eq!(names(&sent).last().unwrap(), ServerOpcode::ActionFail.name());
    // A book is not used up.
    assert_eq!(s.count_of(5000), 1);
}

#[test]
fn a_book_without_a_page_says_so() {
    let dir = tempfile::tempdir().unwrap();
    let mut s = Scene::with_items(vec![item(0x100, 5000, 1)], WorldSettings::default(), Some(dir));
    s.me.take();
    s.use_item(0x100);
    let sent = s.me.take();
    let page: NpcHtmlMessage = decode(of(&sent, ServerOpcode::NpcHtmlMessage)[0]);
    assert!(
        page.html
            .contains("My Text is missing:<br>data/html/help/5000.htm")
    );
}

/// An item whose handler name nothing is registered under does nothing (Java logs a warning).
#[test]
fn an_item_with_an_unknown_handler_does_nothing() {
    let mut s = Scene::new(vec![item(0x100, 5001, 1)]);
    s.me.take();
    s.use_item(0x100);
    assert!(s.me.take().is_empty());
    assert_eq!(s.count_of(5001), 1);
}

// ---- item reuse over a logout (M5-FIX-03) -----------------------------------------------------

fn wall_ms() -> i64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    i64::try_from(now.as_millis()).unwrap()
}

fn saved(item_id: i32, item_obj_id: i32, reuse_delay: i64, systime: i64) -> SavedItemReuse {
    SavedItemReuse {
        item_id,
        item_obj_id,
        reuse_delay,
        systime,
    }
}

/// Java `storeItemReuseDelay`: the timers that still run are saved with the item id, the object
/// id, the full delay and the wall clock time they end (`TimeStamp.getStamp()`).
#[test]
fn an_item_in_reuse_is_saved_with_its_end_time() {
    let persisted = Persisted::default();
    let mut s = Scene::build_with(
        player_data(vec![item(0x100, 1060, 3), item(0x101, 1061, 3)]),
        WorldSettings::default(),
        None,
        Box::new(persisted.clone()),
    );
    s.use_item(0x100);
    s.advance(4_000);
    let before = wall_ms();
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    let after = wall_ms();

    // Only the item that was used has a timer; it ends 10 s after the use (about 6 s left).
    let rows = persisted.last_store().profile.item_reuse_to_store.unwrap();
    assert_eq!(rows.len(), 1, "{rows:?}");
    assert_eq!(
        (rows[0].item_id, rows[0].item_obj_id, rows[0].reuse_delay),
        (1060, 0x100, 10_000)
    );
    assert!(
        (before + 5_800..=after + 6_100).contains(&rows[0].systime),
        "{} not in {before}+6000",
        rows[0].systime
    );
}

/// A timer that ran out is not saved (`currentTime < ts.getStamp()`), but the store still
/// replaces the rows (an empty list, not `None`).
#[test]
fn a_finished_timer_is_not_saved() {
    let persisted = Persisted::default();
    let mut s = Scene::build_with(
        player_data(vec![item(0x100, 1060, 3)]),
        WorldSettings::default(),
        None,
        Box::new(persisted.clone()),
    );
    s.use_item(0x100);
    s.advance(11_000);
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    assert_eq!(persisted.last_store().profile.item_reuse_to_store, Some(vec![]));
}

/// Java `restoreItemReuse`: a saved timer with time left is a stamp again, so the item is refused
/// with the time left, and a shared-group item tells the client's timer (`ExUseSharedGroupItem`).
#[test]
fn a_saved_timer_comes_back_after_a_relog() {
    let mut data = player_data(vec![item(0x100, 1060, 3), item(0x101, 1061, 3)]);
    data.profile.item_reuse_to_restore = vec![
        saved(1060, 0x100, 10_000, wall_ms() + 6_900),
        saved(1061, 0x101, 10_000, wall_ms() + 7_900),
    ];
    let mut s = Scene::build(data, WorldSettings::default(), None);

    let group: Vec<ExUseSharedGroupItem> = of(&s.entered, ServerOpcode::ExUseSharedGroupItem)
        .into_iter()
        .map(|p| decode(p))
        .collect();
    // Only the group item (1061, group 4) is announced; the 1 s the scene ran after the enter is
    // not in the packet.
    assert_eq!(group.len(), 1, "{group:?}");
    assert_eq!(
        (group[0].item_id, group[0].group_id, group[0].total_secs),
        (1061, 4, 10)
    );
    assert!((6..=7).contains(&group[0].remaining_secs), "{group:?}");

    // Both are still in reuse: using them is refused and costs nothing.
    s.use_item(0x100);
    s.use_item(0x101);
    let sent = s.me.take();
    assert_eq!(s.count_of(1060), 3);
    assert_eq!(s.count_of(1061), 3);
    assert_eq!(
        message_ids(&sent),
        [
            SystemMessageId::ThereAreS2SecondSRemainingInS1SReUseTime,
            SystemMessageId::ThereAreS2SecondSRemainingInS1SReUseTime
        ]
    );

    // Once the time has passed the item works.
    s.advance(8_000);
    s.me.take();
    s.use_item(0x100);
    assert_eq!(s.count_of(1060), 2);
}

/// `restoreItemReuse` keeps only a timer of an item the character has, whose template has a reuse
/// delay, and with more than 10 ms left.
#[test]
fn a_timer_that_does_not_apply_is_dropped_on_restore() {
    let mut data = player_data(vec![item(0x100, 1060, 3), item(0x101, 57, 10)]);
    data.profile.item_reuse_to_restore = vec![
        // Almost over (10 ms or less left; the second the scene runs has passed anyway).
        saved(1060, 0x100, 10_000, wall_ms() + 5),
        // The character does not have this item.
        saved(1061, 0x200, 10_000, wall_ms() + 60_000),
        // Adena has no reuse delay.
        saved(57, 0x101, 10_000, wall_ms() + 60_000),
    ];
    let mut s = Scene::build(data, WorldSettings::default(), None);
    assert!(of(&s.entered, ServerOpcode::ExUseSharedGroupItem).is_empty());
    s.use_item(0x100);
    assert_eq!(s.count_of(1060), 2);
}

// ---- the datapack ------------------------------------------------------------------------------

/// The shipped item files keep all their `<capsuled_items>` (no box fails to load) and every
/// box with a handler has data. Skipped without the datapack.
#[test]
fn the_whole_datapack_loads_its_capsuled_items() {
    let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
    let root = root.as_path();
    if !root.join("data/stats/items").is_dir() {
        eprintln!("skipped: {} has no item files", root.display());
        return;
    }
    let items = ItemTemplates::load(root, false);
    let mut boxes = 0;
    for template in items.etc_items() {
        let etc = template.etc().unwrap();
        if etc.handler.as_deref() == Some("ExtractableItems") {
            boxes += 1;
            assert!(
                !etc.extractable_items.is_empty(),
                "{} has the ExtractableItems handler but no products",
                template.label()
            );
        }
    }
    assert!(boxes > 500, "only {boxes} boxes");
}
