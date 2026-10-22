#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Skill enchanting (M6-ENCHANT-02): the info packets, the costs and chances of the enchant, the
//! safe enchant, the untrain and the route change, the `character_skills` rows, and the
//! `SkillList` flag of an enchantable skill.
//!
//! The expected values are worked out by hand from the Java sources:
//! `RequestExEnchantSkill*`, `ExEnchantSkillInfo`, `ExEnchantSkillInfoDetail`,
//! `EnchantSkillGroupsData`, `EnchantSkillLearn`, `Player.isAllowedToEnchantSkills`.
//! The group numbers below are small and made up; the shape (`chance76`..`chance99`, `adena`,
//! `sp`) is the one of `EnchantSkillGroups.xml`.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, EnchantSkillGroupsData, Hennas, ItemTemplates, PlayerTemplates,
    SkillLoadOptions, SkillTemplates,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    ExEnchantSkillInfo, ExEnchantSkillInfoDetail, ExEnchantSkillResult, ShortcutRegister, SkillList,
    SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::*;

// ------------------------------------------------------------------ harness

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

/// The skills a `SkillChanges` command wrote (id, level) and deleted (id).
type SkillChange = (Vec<(i32, i32)>, Vec<i32>);

#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Persisted {
    fn skill_changes(&self) -> Vec<SkillChange> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|c| match c {
                PersistCommand::SkillChanges { upserts, removed, .. } => Some((
                    upserts.iter().map(|s| (s.skill_id, s.skill_level)).collect(),
                    removed.clone(),
                )),
                _ => None,
            })
            .collect()
    }

    fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

struct RecordingOutbox(Persisted);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Data(StatData, Arc<EnchantSkillGroupsData>);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        Some(ItemStatic {
            display_id: item_id,
            type2: 5,
            body_part: 0,
            quest_item: false,
            weight: 0,
            shared_reuse_group: -1,
            weapon: false,
        })
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.0.skills.iter().any(|s| s.id == skill_id && s.is_passive())
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
    fn item_stackable(&self, item_id: i32) -> bool {
        item_id == ADENA
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        Some(format!("Item{item_id}"))
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
    fn enchant_skill_groups(&self) -> Option<Arc<EnchantSkillGroupsData>> {
        Some(Arc::clone(&self.1))
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

fn names(sent: &Sent, to: i32) -> Vec<String> {
    sent.iter()
        .filter(|(id, _)| *id == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

fn decoded<P: Decode>(sent: &Sent, to: i32, name: &str) -> Vec<P> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

fn messages(sent: &Sent, to: i32) -> Vec<SystemMessage> {
    decoded(sent, to, "SYSTEM_MESSAGE")
}

fn message_ids(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    messages(sent, to).into_iter().map(|m| m.id).collect()
}

fn infos(sent: &Sent, to: i32) -> Vec<ExEnchantSkillInfo> {
    decoded(sent, to, "EX_ENCHANT_SKILL_INFO")
}

fn details(sent: &Sent, to: i32) -> Vec<ExEnchantSkillInfoDetail> {
    decoded(sent, to, "EX_ENCHANT_SKILL_INFO_DETAIL")
}

fn results(sent: &Sent, to: i32) -> Vec<bool> {
    decoded::<ExEnchantSkillResult>(sent, to, "EX_ENCHANT_SKILL_RESULT")
        .into_iter()
        .map(|r| r.enchanted)
        .collect()
}

// ------------------------------------------------------------------ data

const ADENA: i32 = 57;
const NORMAL_BOOK: i32 = 6622;
const SAFE_BOOK: i32 = 9627;
const CHANGE_BOOK: i32 = 9626;
const UNTRAIN_BOOK: i32 = 9625;
/// The enchantable test skill: 3 plain levels, route 1 has 3 steps (group 1), route 2 has 2 (group 2).
const SKILL: i32 = 50;
const PLAIN_SKILL: i32 = 60;

/// Step 1 of route 1 is 100 % at level 80, step 2 50 %, step 3 0 % (and 10 % at level 76).
const GROUPS: &str = r#"<list>
<group id="1">
<enchant level="1" adena="1000" exp="0" sp="10000" chance76="10" chance80="100" />
<enchant level="2" adena="2000" exp="0" sp="20000" chance80="50" />
<enchant level="3" adena="3000" exp="0" sp="30000" chance80="0" />
</group>
<group id="2">
<enchant level="1" adena="500" exp="0" sp="7000" chance80="100" />
<enchant level="2" adena="600" exp="0" sp="8000" chance80="100" />
</group>
</list>"#;

const SKILLS: &str = r#"<list>
<skill id="50" levels="3" name="Test Slash" enchantGroup1="1" enchantGroup2="2"><operateType>A1</operateType><targetType>SELF</targetType></skill>
<skill id="60" levels="1" name="Plain"><operateType>A1</operateType><targetType>SELF</targetType></skill>
</list>"#;

fn class_xml() -> String {
    let rows: String = (1..=85)
        .map(|l| {
            format!(
                "<level val=\"{l}\"><hp>{}</hp><mp>{}</mp><cp>{}</cp></level>",
                100 * l,
                50 * l,
                20 * l
            )
        })
        .collect();
    format!(
        r#"<list><classId>88</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{rows}</lvlUpgainData></list>"#
    )
}

fn stats() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&class_xml(), 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &EnchantSkillGroups::new([(1, 3), (2, 2)]),
            &SkillLoadOptions::default(),
        )),
    }
}

const ME: i32 = 0x2000_0001;
const BOOK_OBJECT: i32 = 0x1000_0010;
const ADENA_OBJECT: i32 = 0x1000_0001;
const START_SP: i64 = 1_000_000;
const START_ADENA: i64 = 100_000;

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

/// A third class (Duelist) of level 80 with the skill at its base level, 1,000,000 SP, 100,000
/// adena and one of every skill book.
fn player_data(level: i32, class_id: i32) -> PlayerData {
    PlayerData {
        object_id: ObjectId(ME),
        account: "acc".to_owned(),
        name: "Alice".to_owned(),
        title: String::new(),
        class_id,
        base_class: class_id,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
        exp: 0,
        sp: START_SP,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: 0,
        y: 0,
        z: 0,
        heading: 0,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items: vec![
            item(ADENA_OBJECT, ADENA, START_ADENA),
            item(BOOK_OBJECT, NORMAL_BOOK, 1),
            item(BOOK_OBJECT + 1, SAFE_BOOK, 1),
            item(BOOK_OBJECT + 2, CHANGE_BOOK, 1),
            item(BOOK_OBJECT + 3, UNTRAIN_BOOK, 1),
        ],
        skills: vec![
            SkillData {
                skill_id: SKILL,
                skill_level: 3,
            },
            SkillData {
                skill_id: PLAIN_SKILL,
                skill_level: 1,
            },
        ],
        shortcuts: vec![ShortcutData {
            slot: 2,
            page: 0,
            kind: 2,
            shortcut_id: SKILL,
            level: 3,
        }],
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
}

impl Scene {
    fn new() -> Self {
        Self::build(1, |_| {})
    }

    fn with_seed(seed: u64) -> Self {
        Self::build(seed, |_| {})
    }

    fn build(seed: u64, tweak: impl FnOnce(&mut EnchantSettings)) -> Self {
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(
            l2_util::WorldRng::from_seed(seed),
            Box::new(RecordingOutbox(persisted.clone())),
        );
        let mut enchant = EnchantSettings::default();
        tweak(&mut enchant);
        world.set_settings(WorldSettings {
            enchant,
            ..WorldSettings::default()
        });
        world.set_static_data(Box::new(Data(
            stats(),
            Arc::new(EnchantSkillGroupsData::from_xml(GROUPS, SKILLS)),
        )));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        Self {
            world,
            log,
            persisted,
        }
    }

    fn enter_keeping_the_log(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.advance(1000);
    }

    fn enter(&mut self, data: PlayerData) {
        self.enter_keeping_the_log(data);
        self.log.take();
        self.persisted.clear();
    }

    /// A level 80 Duelist in the world.
    fn alone() -> Self {
        Self::with_player(Self::new(), player_data(80, 88))
    }

    fn with_player(mut scene: Self, data: PlayerData) -> Self {
        scene.enter(data);
        scene
    }

    /// A level 80 Duelist who knows the test skill at `skill_level`.
    fn at(skill_level: i32) -> Self {
        let mut data = player_data(80, 88);
        data.skills[0].skill_level = skill_level;
        data.shortcuts[0].level = skill_level;
        Self::with_player(Self::new(), data)
    }

    fn input(&mut self, input: EnchantInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME).unwrap()),
            input: ClientInput::Enchant(input),
        });
    }

    fn enchant(&mut self, level: i32) {
        self.input(EnchantInput::EnchantSkill {
            skill_id: SKILL,
            skill_level: level,
        });
    }

    fn safe(&mut self, level: i32) {
        self.input(EnchantInput::EnchantSkillSafe {
            skill_id: SKILL,
            skill_level: level,
        });
    }

    fn untrain(&mut self, level: i32) {
        self.input(EnchantInput::UntrainSkill {
            skill_id: SKILL,
            skill_level: level,
        });
    }

    fn route(&mut self, level: i32) {
        self.input(EnchantInput::ChangeSkillRoute {
            skill_id: SKILL,
            skill_level: level,
        });
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn me(&self) -> &Player {
        self.world.player(ObjectId(ME)).unwrap()
    }

    fn skill_level(&self, skill_id: i32) -> i32 {
        self.me()
            .core
            .skills
            .iter()
            .find(|(id, _)| *id == skill_id)
            .map_or(0, |&(_, level)| level)
    }

    fn stored_skill_level(&self, skill_id: i32) -> i32 {
        self.me()
            .data
            .skills
            .iter()
            .find(|s| s.skill_id == skill_id)
            .map_or(0, |s| s.skill_level)
    }

    fn count_of(&self, item_id: i32) -> i64 {
        self.me()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id)
            .map(|i| i.count)
            .sum()
    }

    fn sp(&self) -> i64 {
        self.me().data.sp
    }

    /// Everything sent to the player since the last call, after the delayed packets (the skill
    /// list waits 300 ms) went out.
    fn sent(&mut self) -> Sent {
        self.advance(500);
        self.log.take()
    }
}

// ------------------------------------------------------------------ the info packets

#[test]
fn info_lists_the_first_step_of_every_route() {
    let mut s = Scene::alone();
    s.input(EnchantInput::SkillInfo {
        skill_id: SKILL,
        skill_level: 3,
    });
    let sent = s.sent();
    assert_eq!(names(&sent, ME), vec!["EX_ENCHANT_SKILL_INFO"]);
    assert_eq!(
        infos(&sent, ME),
        vec![ExEnchantSkillInfo {
            skill_id: SKILL,
            level: 3,
            max_enchanted: false,
            routes: vec![101, 201],
        }]
    );
}

#[test]
fn info_of_an_enchanted_skill_lists_the_same_step_of_every_route() {
    let mut s = Scene::at(102);
    s.input(EnchantInput::SkillInfo {
        skill_id: SKILL,
        skill_level: 102,
    });
    // The current level first, then the same step of the other routes. Step 2 is not the last of
    // route 1 (3 steps).
    assert_eq!(
        infos(&s.sent(), ME),
        vec![ExEnchantSkillInfo {
            skill_id: SKILL,
            level: 102,
            max_enchanted: false,
            routes: vec![102, 202],
        }]
    );

    // The last step of route 1: "max enchanted" (the packet writes 0 for "can enchant more").
    let mut s = Scene::at(103);
    s.input(EnchantInput::SkillInfo {
        skill_id: SKILL,
        skill_level: 103,
    });
    let sent = s.sent();
    assert_eq!(infos(&sent, ME).len(), 1);
    assert!(infos(&sent, ME)[0].max_enchanted);
    // Route 2 has two steps, Java still lists its step 3 (the holder clamps to the last one).
    assert_eq!(infos(&sent, ME)[0].routes, vec![103, 203]);
}

#[test]
fn info_is_silent_for_requests_that_do_not_fit() {
    let mut s = Scene::alone();
    // A level the player does not have, an unknown skill, a skill without routes.
    for (skill_id, level) in [
        (SKILL, 2),
        (SKILL, 101),
        (999, 1),
        (PLAIN_SKILL, 1),
        (SKILL, 0),
        (0, 3),
    ] {
        s.input(EnchantInput::SkillInfo {
            skill_id,
            skill_level: level,
        });
    }
    assert!(s.sent().is_empty());

    // Below level 76 nothing is answered either.
    let mut low = Scene::with_player(Scene::new(), player_data(75, 88));
    low.input(EnchantInput::SkillInfo {
        skill_id: SKILL,
        skill_level: 3,
    });
    assert!(low.sent().is_empty());
}

#[test]
fn detail_shows_the_cost_of_each_kind() {
    let mut s = Scene::alone();
    let ask = |s: &mut Scene, kind: i32, skill_level: i32| {
        s.input(EnchantInput::SkillInfoDetail {
            kind,
            skill_id: SKILL,
            skill_level,
        });
        details(&s.sent(), ME)
    };

    // A normal enchant to 101: the cost of step 1, x1, the book is needed at the first step. The
    // chance is `chance80`.
    assert_eq!(
        ask(&mut s, 0, 101),
        vec![ExEnchantSkillInfoDetail {
            kind: 0,
            skill_id: SKILL,
            skill_level: 101,
            sp: 10_000,
            chance: 100,
            adena: 1_000,
            book_id: NORMAL_BOOK,
            book_required: true,
        }]
    );

    // A safe enchant costs 5 times the SP and adena (`SafeEnchantCostMultipiler`), and always
    // needs its book.
    assert_eq!(
        ask(&mut s, 1, 101),
        vec![ExEnchantSkillInfoDetail {
            kind: 1,
            skill_id: SKILL,
            skill_level: 101,
            sp: 50_000,
            chance: 100,
            adena: 5_000,
            book_id: SAFE_BOOK,
            book_required: true,
        }]
    );

    // From 101 the same asks for the step to 102 (the player has to know 101).
    let mut s = Scene::at(101);
    let step2 = ask(&mut s, 0, 102);
    assert_eq!(step2.len(), 1);
    // Only the first step uses a book (`(skillLevel % 100) < 2`).
    assert_eq!(
        (
            step2[0].sp,
            step2[0].chance,
            step2[0].adena,
            step2[0].book_required
        ),
        (20_000, 50, 2_000, false)
    );

    // Untrain: 80 % of the SP, no multiplier, and the book of untraining.
    let untrain = ask(&mut s, 2, 100);
    assert_eq!(untrain.len(), 1);
    assert_eq!(
        (
            untrain[0].sp,
            untrain[0].adena,
            untrain[0].book_id,
            untrain[0].book_required
        ),
        (8_000, 1_000, UNTRAIN_BOOK, true)
    );

    // Change route: the same step of the other route, normal costs, its own book.
    let mut s = Scene::at(102);
    let change = ask(&mut s, 3, 202);
    assert_eq!(change.len(), 1);
    assert_eq!(
        (
            change[0].sp,
            change[0].adena,
            change[0].book_id,
            change[0].book_required
        ),
        (8_000, 600, CHANGE_BOOK, true)
    );
}

#[test]
fn detail_without_books_does_not_ask_for_them() {
    let mut s = Scene::build(1, |e| e.es_sp_book_needed = false);
    s.enter(player_data(80, 88));
    s.input(EnchantInput::SkillInfoDetail {
        kind: 0,
        skill_id: SKILL,
        skill_level: 101,
    });
    assert!(!details(&s.sent(), ME)[0].book_required);
    // The safe enchant has no config: its book stays required.
    s.input(EnchantInput::SkillInfoDetail {
        kind: 1,
        skill_id: SKILL,
        skill_level: 101,
    });
    assert!(details(&s.sent(), ME)[0].book_required);
}

#[test]
fn detail_checks_the_levels_like_java() {
    let mut s = Scene::alone();
    // The skill must be known at all.
    s.input(EnchantInput::SkillInfoDetail {
        kind: 0,
        skill_id: 999,
        skill_level: 101,
    });
    // A skill without enchant data and a base step (`101 - 1 = 100`).
    s.input(EnchantInput::SkillInfoDetail {
        kind: 0,
        skill_id: PLAIN_SKILL,
        skill_level: 101,
    });
    // `101 - 1` is a base step: the player's level must be the base level, not 1 below.
    s.input(EnchantInput::SkillInfoDetail {
        kind: 0,
        skill_id: SKILL,
        skill_level: 101,
    });
    assert_eq!(details(&s.sent(), ME).len(), 1);
    let mut low = Scene::at(2);
    low.input(EnchantInput::SkillInfoDetail {
        kind: 0,
        skill_id: SKILL,
        skill_level: 101,
    });
    assert!(low.sent().is_empty());

    // NOTE(java-quirk): a normal enchant detail for a step the player is not at (the check
    // `playerSkillLevel != reqSkillLevel` is for the route change only) is answered.
    s.input(EnchantInput::SkillInfoDetail {
        kind: 0,
        skill_id: SKILL,
        skill_level: 103,
    });
    assert_eq!(details(&s.sent(), ME).len(), 1);

    // A route change to a different step of another route is refused.
    let mut s = Scene::at(102);
    s.input(EnchantInput::SkillInfoDetail {
        kind: 3,
        skill_id: SKILL,
        skill_level: 201,
    });
    assert!(details(&s.sent(), ME).is_empty());
}

// ------------------------------------------------------------------ the SkillList flag

#[test]
fn the_skill_list_marks_enchantable_skills() {
    // The list sent when the player enters the world.
    let flag = |skill_level: i32, id: i32| {
        let mut data = player_data(80, 88);
        data.skills[0].skill_level = skill_level;
        let mut s = Scene::new();
        s.enter_keeping_the_log(data);
        let lists: Vec<SkillList> = decoded(&s.log.take(), ME, "SKILL_LIST");
        assert_eq!(lists.len(), 1);
        lists[0].skills.iter().find(|e| e.id == id).map(|e| e.enchanted)
    };
    // Level 3 is the last plain level, the base level of the routes.
    assert_eq!(flag(3, SKILL), Some(true));
    assert_eq!(flag(3, PLAIN_SKILL), Some(false));
    // Below the base level the skill cannot be enchanted yet.
    assert_eq!(flag(2, SKILL), Some(false));
    // An enchanted level is above the base level.
    assert_eq!(flag(101, SKILL), Some(true));
}

// ------------------------------------------------------------------ the normal enchant

#[test]
fn a_successful_enchant_takes_the_costs_and_writes_the_row() {
    let mut s = Scene::alone();
    s.enchant(101);
    let sent = s.sent();

    assert_eq!(s.skill_level(SKILL), 101);
    assert_eq!(s.stored_skill_level(SKILL), 101);
    // 10,000 SP, 1,000 adena and the book (the first step uses one).
    assert_eq!(s.sp(), START_SP - 10_000);
    assert_eq!(s.count_of(ADENA), START_ADENA - 1_000);
    assert_eq!(s.count_of(NORMAL_BOOK), 0);
    // The other books stay.
    assert_eq!(s.count_of(SAFE_BOOK), 1);
    // The `character_skills` row of the enchanted level (D-39).
    assert_eq!(s.persisted.skill_changes(), vec![(vec![(SKILL, 101)], vec![])]);

    assert_eq!(results(&sent, ME), vec![true]);
    let ids = message_ids(&sent, ME);
    assert!(ids.contains(&SystemMessageId::SkillEnchantWasSuccessfulS1HasBeenEnchanted));
    let success = messages(&sent, ME)
        .into_iter()
        .find(|m| m.id == SystemMessageId::SkillEnchantWasSuccessfulS1HasBeenEnchanted)
        .unwrap();
    assert_eq!(
        success.params,
        vec![SystemMessageParam::SkillName { id: SKILL, level: 1 }]
    );

    // The info of the new level, and what the next step costs (102: 50 %, 2,000 adena, no book).
    assert_eq!(
        infos(&sent, ME),
        vec![ExEnchantSkillInfo {
            skill_id: SKILL,
            level: 101,
            max_enchanted: false,
            routes: vec![101, 201],
        }]
    );
    assert_eq!(
        details(&sent, ME),
        vec![ExEnchantSkillInfoDetail {
            kind: 0,
            skill_id: SKILL,
            skill_level: 102,
            sp: 20_000,
            chance: 50,
            adena: 2_000,
            book_id: NORMAL_BOOK,
            book_required: false,
        }]
    );
    // The shortcut of the skill shows the new level.
    let shortcuts: Vec<ShortcutRegister> = decoded(&sent, ME, "SHORT_CUT_REGISTER");
    assert_eq!(shortcuts.len(), 1);
    assert_eq!(s.me().data.shortcuts[0].level, 101);
    // The order of Java: the result, the message, the user info, the skill list, the infos.
    let order = names(&sent, ME);
    let position = |name: &str| order.iter().position(|n| n == name).unwrap();
    let last_message = order.iter().rposition(|n| n == "SYSTEM_MESSAGE").unwrap();
    assert!(position("EX_ENCHANT_SKILL_RESULT") < last_message);
    assert!(last_message < position("USER_INFO"));
    // The adena and the book went with their own "disappeared" messages, before the result.
    assert!(ids.contains(&SystemMessageId::S1AdenaDisappeared));
    assert!(ids.contains(&SystemMessageId::S1HasDisappeared));
    assert!(position("SYSTEM_MESSAGE") < position("EX_ENCHANT_SKILL_RESULT"));
    assert!(position("EX_ENCHANT_SKILL_INFO") < position("EX_ENCHANT_SKILL_INFO_DETAIL"));
    assert!(position("EX_ENCHANT_SKILL_INFO_DETAIL") < position("SHORT_CUT_REGISTER"));
    assert!(order.contains(&"SKILL_LIST".to_owned()));
}

#[test]
fn a_later_step_needs_no_book() {
    let mut s = Scene::at(101);
    s.enchant(102);
    let _ = s.sent();
    // Step 2 (50 %): the book of the first step was not needed, and is not used up. With the seed
    // used the roll decides success or failure; either way the costs are taken.
    assert_eq!(s.sp(), START_SP - 20_000);
    assert_eq!(s.count_of(ADENA), START_ADENA - 2_000);
    assert_eq!(s.count_of(NORMAL_BOOK), 1);
    assert!(matches!(s.skill_level(SKILL), 102 | 3));
}

#[test]
fn a_failed_enchant_goes_back_to_the_base_level() {
    // Step 3 has a 0 % chance, and Java still lets `Rnd.get(100) <= 0` win one time in 100: the
    // first seed that loses.
    for seed in 0..50 {
        let mut data = player_data(80, 88);
        data.skills[0].skill_level = 102;
        data.shortcuts[0].level = 102;
        let mut s = Scene::with_player(Scene::with_seed(seed), data);
        s.enchant(103);
        let sent = s.sent();
        if s.skill_level(SKILL) == 103 {
            continue;
        }
        assert_eq!(s.skill_level(SKILL), 3);
        assert_eq!(s.stored_skill_level(SKILL), 3);
        assert_eq!(s.persisted.skill_changes(), vec![(vec![(SKILL, 3)], vec![])]);
        // The costs are gone all the same: 30,000 SP and 3,000 adena.
        assert_eq!(s.sp(), START_SP - 30_000);
        assert_eq!(s.count_of(ADENA), START_ADENA - 3_000);
        assert_eq!(results(&sent, ME), vec![false]);
        let ids = message_ids(&sent, ME);
        assert!(ids.contains(&SystemMessageId::SkillEnchantFailedTheSkillWillBeInitialized));
        assert!(!ids.contains(&SystemMessageId::SkillEnchantWasSuccessfulS1HasBeenEnchanted));
        // The info of the base level: every route starts again.
        assert_eq!(infos(&sent, ME)[0].routes, vec![101, 201]);
        assert_eq!(infos(&sent, ME)[0].level, 3);
        assert_eq!(s.me().data.shortcuts[0].level, 3);
        return;
    }
    panic!("no seed in 0..50 failed a 0 % enchant");
}

#[test]
fn an_enchant_refuses_what_it_cannot_pay() {
    // Not enough SP.
    let mut data = player_data(80, 88);
    data.sp = 9_999;
    let mut s = Scene::with_player(Scene::new(), data);
    s.enchant(101);
    let sent = s.sent();
    assert_eq!(
        message_ids(&sent, ME),
        vec![SystemMessageId::YouDoNotHaveEnoughSpToEnchantThatSkill]
    );
    assert_eq!(s.skill_level(SKILL), 3);
    assert_eq!(s.sp(), 9_999);

    // No book at the first step.
    let mut data = player_data(80, 88);
    data.items.retain(|i| i.item_id != NORMAL_BOOK);
    let mut s = Scene::with_player(Scene::new(), data);
    s.enchant(101);
    assert_eq!(
        message_ids(&s.sent(), ME),
        vec![SystemMessageId::YouDoNotHaveAllOfTheItemsNeededToEnchantThatSkill]
    );
    assert_eq!(s.sp(), START_SP);

    // Not enough adena: nothing was taken before the check.
    let mut data = player_data(80, 88);
    data.items[0].count = 999;
    let mut s = Scene::with_player(Scene::new(), data);
    s.enchant(101);
    assert_eq!(
        message_ids(&s.sent(), ME),
        vec![SystemMessageId::YouDoNotHaveAllOfTheItemsNeededToEnchantThatSkill]
    );
    assert_eq!(s.sp(), START_SP);
    assert_eq!(s.count_of(NORMAL_BOOK), 1);
    assert!(s.persisted.skill_changes().is_empty());
}

#[test]
fn an_enchant_without_the_book_rule_keeps_the_book() {
    let mut s = Scene::build(1, |e| e.es_sp_book_needed = false);
    let mut data = player_data(80, 88);
    data.items.retain(|i| i.item_id != NORMAL_BOOK);
    s.enter(data);
    s.enchant(101);
    let _ = s.sent();
    // `EnchantSkillSpBookNeeded = False`: no book needed, none used.
    assert_eq!(s.skill_level(SKILL), 101);
    assert_eq!(s.sp(), START_SP - 10_000);
}

#[test]
fn the_cost_multiplier_of_the_config_applies() {
    let mut s = Scene::build(1, |e| e.normal_enchant_cost_multiplier = 3);
    s.enter(player_data(80, 88));
    s.enchant(101);
    let _ = s.sent();
    assert_eq!(s.sp(), START_SP - 30_000);
    assert_eq!(s.count_of(ADENA), START_ADENA - 3_000);
}

#[test]
fn an_enchant_needs_the_step_before() {
    let mut s = Scene::alone();
    // The player has level 3; 102 needs 101.
    s.enchant(102);
    // A level that is on no route (`getMinSkillLevel` is 99 for 100).
    s.enchant(100);
    // A skill that is not enchantable at all.
    s.input(EnchantInput::EnchantSkill {
        skill_id: PLAIN_SKILL,
        skill_level: 101,
    });
    assert!(s.sent().is_empty());
    assert_eq!(s.sp(), START_SP);
}

#[test]
fn enchanting_needs_a_third_class_level_76_and_a_calm_player() {
    // A second class (Warrior, 1st class change 0, ... class 2 is Gladiator, level 2).
    let mut s = Scene::with_player(Scene::new(), player_data(80, 2));
    s.enchant(101);
    s.safe(101);
    s.untrain(101);
    s.route(101);
    let sent = s.sent();
    assert_eq!(
        message_ids(&sent, ME),
        vec![
            SystemMessageId::YouCannotUseTheSkillEnhancingFunctionInThisClassYouCanUseCorrespondingFunctionWhenCompletingTheThirdClassChange;
            4
        ]
    );

    let mut s = Scene::with_player(Scene::new(), player_data(75, 88));
    s.enchant(101);
    assert_eq!(
        message_ids(&s.sent(), ME),
        vec![
            SystemMessageId::YouCannotUseTheSkillEnhancingFunctionOnThisLevelYouCanUseTheCorrespondingFunctionOnLevelsHigherThan76lv
        ]
    );

    // In combat (an attack stance).
    let mut s = Scene::alone();
    s.world.add_attack_stance_task(ObjectId(ME));
    s.log.take();
    s.enchant(101);
    s.safe(101);
    let ids = message_ids(&s.sent(), ME);
    assert_eq!(
        ids,
        vec![
            SystemMessageId::YouCannotUseTheSkillEnhancingFunctionInThisClassYouCanUseTheSkillEnhancingFunctionUnderOffBattleStatusAndCannotUseTheFunctionWhileTransformingBattlingAndOnBoard;
            2
        ]
    );
    assert_eq!(s.skill_level(SKILL), 3);
    assert!(!s.world.is_allowed_to_enchant_skills(ObjectId(ME)));
}

// ------------------------------------------------------------------ the safe enchant

#[test]
fn a_safe_enchant_costs_five_times_and_keeps_the_level_on_failure() {
    // Step 3 of route 1 at 0 %: look for a seed that loses.
    for seed in 0..50 {
        let mut data = player_data(80, 88);
        data.skills[0].skill_level = 102;
        let mut s = Scene::with_player(Scene::with_seed(seed), data);
        s.safe(103);
        let sent = s.sent();
        // 5 x 30,000 SP, 5 x 3,000 adena, the safe book.
        assert_eq!(s.sp(), START_SP - 150_000);
        assert_eq!(s.count_of(ADENA), START_ADENA - 15_000);
        assert_eq!(s.count_of(SAFE_BOOK), 0);
        if s.skill_level(SKILL) == 103 {
            continue;
        }
        assert_eq!(s.skill_level(SKILL), 102);
        assert_eq!(results(&sent, ME), vec![false]);
        let failed = messages(&sent, ME)
            .into_iter()
            .find(|m| {
                m.id == SystemMessageId::SkillEnchantFailedCurrentLevelOfEnchantSkillS1WillRemainUnchanged
            })
            .unwrap();
        assert_eq!(
            failed.params,
            vec![SystemMessageParam::SkillName { id: SKILL, level: 1 }]
        );
        // The row is not rewritten, the level did not change.
        assert!(s.persisted.skill_changes().is_empty());
        // `ExEnchantSkillInfoDetail(1, id, after + 1)`: the safe prices of step 103.
        assert_eq!(
            details(&sent, ME)
                .iter()
                .map(|d| (d.kind, d.skill_level, d.sp, d.adena, d.book_id))
                .collect::<Vec<_>>(),
            vec![(1, 103, 150_000, 15_000, SAFE_BOOK)]
        );
        return;
    }
    panic!("no seed in 0..50 failed a 0 % enchant");
}

#[test]
fn a_safe_enchant_can_succeed() {
    let mut s = Scene::alone();
    s.safe(101);
    let sent = s.sent();
    // Step 1 is 100 %.
    assert_eq!(s.skill_level(SKILL), 101);
    assert_eq!(s.persisted.skill_changes(), vec![(vec![(SKILL, 101)], vec![])]);
    assert_eq!(results(&sent, ME), vec![true]);
    assert_eq!(s.sp(), START_SP - 50_000);
    assert_eq!(s.count_of(ADENA), START_ADENA - 5_000);
    assert_eq!(s.count_of(SAFE_BOOK), 0);
    assert_eq!(s.count_of(NORMAL_BOOK), 1);
}

#[test]
fn a_safe_enchant_always_needs_its_book() {
    let mut s = Scene::build(1, |e| e.es_sp_book_needed = false);
    let mut data = player_data(80, 88);
    data.items.retain(|i| i.item_id != SAFE_BOOK);
    s.enter(data);
    s.safe(101);
    assert_eq!(
        message_ids(&s.sent(), ME),
        vec![SystemMessageId::YouDoNotHaveAllOfTheItemsNeededToEnchantThatSkill]
    );
    assert_eq!(s.skill_level(SKILL), 3);
}

// ------------------------------------------------------------------ the untrain

#[test]
fn an_untrain_goes_one_level_back_and_gives_most_of_the_sp_back() {
    let mut s = Scene::at(102);
    s.untrain(101);
    let sent = s.sent();
    assert_eq!(s.skill_level(SKILL), 101);
    assert_eq!(s.persisted.skill_changes(), vec![(vec![(SKILL, 101)], vec![])]);
    // The step being left is 102: 20,000 SP, 80 % of it back (16,000); 2,000 adena (no
    // multiplier); the untrain book.
    assert_eq!(s.sp(), START_SP + 16_000);
    assert_eq!(s.count_of(ADENA), START_ADENA - 2_000);
    assert_eq!(s.count_of(UNTRAIN_BOOK), 0);
    assert_eq!(results(&sent, ME), vec![true]);
    let message = messages(&sent, ME)
        .into_iter()
        .find(|m| {
            m.id == SystemMessageId::UntrainOfEnchantSkillWasSuccessfulCurrentLevelOfEnchantSkillS1HasBeenDecreasedBy1
        })
        .unwrap();
    assert_eq!(
        message.params,
        vec![SystemMessageParam::SkillName { id: SKILL, level: 1 }]
    );
    // `ExEnchantSkillInfoDetail(2, id, after - 1)`: the untrain of the step below.
    let detail = &details(&sent, ME)[0];
    assert_eq!((detail.kind, detail.skill_level), (2, 100));
    assert_eq!(s.me().data.shortcuts[0].level, 101);
}

#[test]
fn untraining_the_first_step_gives_the_base_level() {
    let mut s = Scene::at(101);
    // The client asks for level 100 (`x00`): Java takes the base level.
    s.untrain(100);
    let sent = s.sent();
    assert_eq!(s.skill_level(SKILL), 3);
    assert_eq!(s.persisted.skill_changes(), vec![(vec![(SKILL, 3)], vec![])]);
    assert!(
        message_ids(&sent, ME).contains(
            &SystemMessageId::UntrainOfEnchantSkillWasSuccessfulCurrentLevelOfEnchantSkillS1Became0AndEnchantSkillWillBeInitialized
        )
    );
    // 10,000 SP of step 1, 80 % back.
    assert_eq!(s.sp(), START_SP + 8_000);
}

#[test]
fn an_untrain_checks_the_level_and_the_costs() {
    let mut s = Scene::alone();
    // The player is at 3; there is nothing to untrain.
    s.untrain(2);
    s.untrain(101);
    assert!(s.sent().is_empty());

    // A request for the wrong level: the player has 103, asks to go to 101.
    let mut s = Scene::at(103);
    s.untrain(101);
    assert!(s.sent().is_empty());

    // No book.
    let mut data = player_data(80, 88);
    data.items.retain(|i| i.item_id != UNTRAIN_BOOK);
    data.skills[0].skill_level = 102;
    let mut s = Scene::with_player(Scene::new(), data);
    s.untrain(101);
    assert_eq!(
        message_ids(&s.sent(), ME),
        vec![SystemMessageId::YouDoNotHaveAllOfTheItemsNeededToEnchantThatSkill]
    );
    assert_eq!(s.skill_level(SKILL), 102);

    // Not enough adena for the step.
    let mut data = player_data(80, 88);
    data.items[0].count = 1_999;
    data.skills[0].skill_level = 102;
    let mut s = Scene::with_player(Scene::new(), data);
    s.untrain(101);
    assert_eq!(
        message_ids(&s.sent(), ME),
        vec![SystemMessageId::YouDoNotHaveAllOfTheItemsNeededToEnchantThatSkill]
    );
    assert_eq!(s.count_of(UNTRAIN_BOOK), 1);
}

// ------------------------------------------------------------------ the route change

#[test]
fn a_route_change_keeps_the_step_or_loses_up_to_three() {
    // Step 2 on route 1 to step 2 on route 2: `Rnd.get(min(4, 2))` is 0 or 1 lost steps. Both
    // outcomes are told apart by the message, and both are reached by some seed.
    let mut seen = [false; 2];
    for seed in 0..40 {
        let mut data = player_data(80, 88);
        data.skills[0].skill_level = 102;
        let mut s = Scene::with_player(Scene::with_seed(seed), data);
        s.route(202);
        let sent = s.sent();
        // The costs of step 2 of route 2: 8,000 SP and 600 adena (no multiplier) and the book.
        assert_eq!(s.sp(), START_SP - 8_000);
        assert_eq!(s.count_of(ADENA), START_ADENA - 600);
        assert_eq!(s.count_of(CHANGE_BOOK), 0);
        assert_eq!(results(&sent, ME), vec![true]);
        let lost = 202 - s.skill_level(SKILL);
        assert!(lost == 0 || lost == 1, "lost {lost}");
        assert_eq!(
            s.persisted.skill_changes(),
            vec![(vec![(SKILL, 202 - lost)], vec![])]
        );
        let ids = message_ids(&sent, ME);
        if lost == 0 {
            seen[0] = true;
            assert!(ids.contains(
                &SystemMessageId::EnchantSkillRouteChangeWasSuccessfulLvOfEnchantSkillS1WillRemain
            ));
        } else {
            seen[1] = true;
            let message = messages(&sent, ME)
                .into_iter()
                .find(|m| {
                    m.id
                        == SystemMessageId::EnchantSkillRouteChangeWasSuccessfulLvOfEnchantSkillS1HasBeenDecreasedByS2
                })
                .unwrap();
            assert_eq!(
                message.params,
                vec![
                    SystemMessageParam::SkillName { id: SKILL, level: 1 },
                    SystemMessageParam::IntNumber(1)
                ]
            );
        }
        // `ExEnchantSkillInfoDetail(3, id, after)`: the same level, not one more.
        let detail = &details(&sent, ME)[0];
        assert_eq!((detail.kind, detail.skill_level), (3, 202 - lost));
    }
    assert_eq!(seen, [true, true]);
}

#[test]
fn losing_the_first_step_of_a_route_lands_on_the_base_level() {
    // Step 1 on route 1 to step 1 of route 2: `Rnd.get(min(4, 1))` is 0, nothing is lost.
    let mut s = Scene::at(101);
    s.route(201);
    let _ = s.sent();
    assert_eq!(s.skill_level(SKILL), 201);
}

#[test]
fn a_route_change_checks_what_it_needs() {
    let mut s = Scene::alone();
    // Not enchanted: nothing to change.
    s.route(201);
    assert!(s.sent().is_empty());

    // The step has to be the same: the player is at step 2, the request is for step 1.
    let mut s = Scene::at(102);
    s.route(201);
    assert!(s.sent().is_empty());
    assert_eq!(s.sp(), START_SP);

    // Not enough SP.
    let mut data = player_data(80, 88);
    data.sp = 7_999;
    data.skills[0].skill_level = 102;
    let mut s = Scene::with_player(Scene::new(), data);
    s.route(202);
    assert_eq!(
        message_ids(&s.sent(), ME),
        vec![SystemMessageId::YouDoNotHaveEnoughSpToEnchantThatSkill]
    );

    // No book: the message of the route change.
    let mut data = player_data(80, 88);
    data.items.retain(|i| i.item_id != CHANGE_BOOK);
    data.skills[0].skill_level = 102;
    let mut s = Scene::with_player(Scene::new(), data);
    s.route(202);
    assert_eq!(
        message_ids(&s.sent(), ME),
        vec![SystemMessageId::YouDoNotHaveAllOfTheItemsNeededToEnchantSkillRouteChange]
    );
    assert_eq!(s.skill_level(SKILL), 102);
}

// ------------------------------------------------------------------ the log

#[test]
fn the_rows_survive_a_second_change() {
    // 3 -> 101 -> 102 (maybe) : every change writes the level it ends at.
    let mut s = Scene::alone();
    s.enchant(101);
    s.advance(100);
    s.enchant(102);
    let _ = s.sent();
    let rows = s.persisted.skill_changes();
    assert_eq!(rows[0], (vec![(SKILL, 101)], vec![]));
    assert_eq!(rows.len(), 2);
    assert!(matches!(rows[1].0.as_slice(), [(SKILL, 102)] | [(SKILL, 3)]));
}
