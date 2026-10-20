#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Learning skills at a class trainer (M5-LEARN-01): the `SkillList` bypass, the lists, costs and
//! checks of `RequestAcquireSkillInfo` / `RequestAcquireSkill`, the `character_skills` writes, and
//! `AutoLearnSkills`.
//!
//! The expected values are worked out by hand from the Java sources: the order of the list is the
//! `HashMap` order of `skillId * 1021 + skillLevel` (bucket `(h ^ h >>> 16) & (capacity - 1)`),
//! the messages are the ones `RequestAcquireSkill` sends, the rows are what `storeSkill` writes.
//! Java: network/clientpackets/{RequestAcquireSkill,RequestAcquireSkillInfo}.java,
//! entity/actor/instance/Folk.java, data/xml/SkillTreeData.java, entity/actor/Player.java.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates, KarmaLoss,
    NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLearnData, SkillLoadOptions, SkillTemplates,
    SkillTrees, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    AcquireSkillInfo, AcquireSkillList, AcquireSkillRequirement, AcquireSkillType, ShortcutInit,
    ShortcutRegister, SkillList, SystemMessage, SystemMessageParam,
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

/// Item 9999 (the "token", stackable) and adena are stackable.
struct Data(StatData);

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
        item_id == 57 || item_id == TOKEN
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
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

// ------------------------------------------------------------------ data

const TOKEN: i32 = 9999;
const BOOK: i32 = 8618;

/// Class 0's tree. Hash order of the keys at 16 buckets (worked out by hand):
/// (1020,1) 2, (1010,1) 4, (1011,1) 7, (3,1) 8, (3,2) 9, (3,3) 10, (1405,1) 15.
const TREES: &str = r#"<list>
<skillTree type="classSkillTree" classId="0">
<skill skillName="Power Strike" skillId="3" skillLevel="1" getLevel="5" levelUpSp="100" learnedByNpc="true" />
<skill skillName="Power Strike" skillId="3" skillLevel="2" getLevel="5" levelUpSp="200" learnedByNpc="true" />
<skill skillName="Power Strike" skillId="3" skillLevel="3" getLevel="20" levelUpSp="300" learnedByNpc="true" />
<skill skillName="Tokens" skillId="1010" skillLevel="1" getLevel="5" levelUpSp="500" learnedByNpc="true">
  <item id="9999" count="2" />
</skill>
<skill skillName="Late" skillId="1011" skillLevel="1" getLevel="40" levelUpSp="10" learnedByNpc="true" />
<skill skillName="Lucky" skillId="1020" skillLevel="1" getLevel="1" autoGet="true" />
<skill skillName="Divine Inspiration" skillId="1405" skillLevel="1" getLevel="5" levelUpSp="50" learnedByNpc="true">
  <item id="8618" count="1" />
</skill>
</skillTree>
<skillTree type="classSkillTree" classId="10">
<skill skillName="Mage Skill" skillId="3" skillLevel="1" getLevel="5" levelUpSp="100" learnedByNpc="true" />
</skillTree>
</list>"#;

const SKILL_LEARN: &str = r#"<list>
<npc id="1100"><classId>0</classId></npc>
<npc id="1101"><classId>10</classId></npc>
</list>"#;

const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="1100" type="Trainer" level="10" name="Auron" usingServerSideName="true"/>
  <npc id="1101" type="Trainer" level="10" name="Zigaunt" usingServerSideName="true"/>
</list>"#;

fn passive(id: i32, levels: i32) -> String {
    format!(
        r#"<skill id="{id}" levels="{levels}" name="S{id}"><operateType>P</operateType><targetType>SELF</targetType>
        <effects><effect name="Buff"><add stat="mDef" val="1"/></effect></effects></skill>"#
    )
}

fn skills_xml() -> String {
    format!(
        "<list>{}{}{}{}{}</list>",
        passive(3, 3),
        passive(1010, 1),
        passive(1011, 1),
        passive(1020, 1),
        passive(1405, 1)
    )
}

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
        r#"<list><classId>0</classId><staticData>
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

fn synthetic_stats(skills_xml: &str) -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&class_xml(), 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            skills_xml,
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

/// Level `n` starts at `1000 * (n - 1)^2` exp.
fn exp_for(level: i32) -> i64 {
    1000 * i64::from(level - 1) * i64::from(level - 1)
}

fn experience() -> Arc<ExperienceTable> {
    let rows: String = (1..=86)
        .map(|l| format!(r#"<experience level="{l}" tolevel="{}" />"#, exp_for(l)))
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
        ))
        .unwrap(),
    )
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: i32 = 0x2000_0001;
const MAGE: i32 = 0x2000_0002;

fn token(count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(0x1000_0002),
        item_id: TOKEN,
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

fn book(count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(0x1000_0003),
        item_id: BOOK,
        ..token(count)
    }
}

fn player_data(id: i32, level: i32, sp: i64) -> PlayerData {
    let (hp, mp, cp) = (
        f64::from(100 * level),
        f64::from(50 * level),
        f64::from(20 * level),
    );
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{id}"),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
        exp: exp_for(level),
        sp,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: hp,
        max_hp: hp,
        cur_mp: mp,
        max_mp: mp,
        cur_cp: cp,
        max_cp: cp,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Self {
        Self::with(WorldSettings::default())
    }

    fn with(settings: WorldSettings) -> Self {
        Self::build(
            settings,
            Arc::new(SkillTrees::from_xml(TREES).unwrap()),
            Arc::new(SkillLearnData::from_xml(SKILL_LEARN).unwrap()),
            NPCS,
            &skills_xml(),
        )
    }

    fn build(
        settings: WorldSettings,
        trees: Arc<SkillTrees>,
        learn: Arc<SkillLearnData>,
        npcs_xml: &str,
        skills_xml: &str,
    ) -> Self {
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
        // The page of the shipped trainers (`data/html/trainer/30010.htm`), with its SkillList link.
        write(
            "data/html/trainer/30010.htm",
            "<html><body><a action=\"bypass -h npc_%objectId%_SkillList\">Learn skills.</a></body></html>",
        );
        write(
            "data/html/trainer/1100.htm",
            "<html><body><a action=\"bypass -h npc_%objectId%_SkillList\">Learn skills.</a></body></html>",
        );
        write(
            "data/html/trainer/1101.htm",
            "<html><body><a action=\"bypass -h npc_%objectId%_SkillList\">Learn skills.</a></body></html>",
        );
        write(
            "data/html/trainer/1100-noteach.htm",
            "<html><body>Wrong teacher %objectId%.</body></html>",
        );
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(
            WorldRng::from_seed(7),
            Box::new(RecordingOutbox(persisted.clone())),
        );
        world.set_settings(settings);
        world.set_static_data(Box::new(Data(synthetic_stats(skills_xml))));
        world.set_exp_data(
            experience(),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            trees,
        );
        world.set_skill_learn_data(learn);
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(npcs_xml, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        Self {
            world,
            log,
            persisted,
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
        self.log.take();
        self.persisted.clear();
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn me(&self) -> &Player {
        self.world.player(ObjectId(ME)).unwrap()
    }

    /// Spawns trainer `npc_id` next to the player and talks to it: the page is shown and its
    /// `SkillList` link allowed. Returns the NPC's object id.
    fn talk_to(&mut self, player: i32, npc_id: i32) -> ObjectId {
        let npc = self.world.spawn_npc_at(npc_id, X + 30, Y, Z, 0).unwrap();
        for _ in 0..2 {
            self.input(
                player,
                ClientInput::Action {
                    object_id: npc.0,
                    action_id: 0,
                },
            );
        }
        npc
    }

    fn skill_list(&mut self, player: i32, npc: ObjectId) {
        self.input(
            player,
            ClientInput::Bypass {
                command: format!("npc_{}_SkillList", npc.0),
            },
        );
    }

    fn info(&mut self, id: i32, level: i32) {
        self.input(
            ME,
            ClientInput::AcquireSkillInfo {
                id,
                level,
                skill_type: AcquireSkillType::Class,
            },
        );
    }

    fn learn(&mut self, id: i32, level: i32) {
        self.input(
            ME,
            ClientInput::AcquireSkill {
                id,
                level,
                skill_type: AcquireSkillType::Class,
                sub_type: 0,
            },
        );
    }
}

fn skills_of(player: &Player) -> Vec<(i32, i32)> {
    let mut skills = player.core.skills.clone();
    skills.sort_unstable();
    skills
}

/// `(id, next level, max level, sp cost, requirements)` of a list.
fn entries(list: &AcquireSkillList) -> Vec<(i32, i32, i32, i32, i32)> {
    list.skills
        .iter()
        .map(|s| (s.id, s.next_level, s.max_level, s.sp_cost, s.requirements))
        .collect()
}

fn lists(sent: &Sent) -> Vec<AcquireSkillList> {
    decoded(sent, ME, "ACQUIRE_SKILL_LIST")
}

// ------------------------------------------------------------------ the list

/// The list of a trainer: the lines the player may learn next (level 1 of a skill he does not
/// know), without auto-get skills and without the ones his level is too low for, in the hash
/// order of Java's class tree, with the SP each costs.
#[test]
fn the_trainer_lists_the_next_skills_in_java_order() {
    let mut s = Scene::new();
    s.enter(player_data(ME, 10, 1000));
    let npc = s.talk_to(ME, 1100);
    s.log.take();

    s.skill_list(ME, npc);
    let sent = s.log.take();
    let shown = lists(&sent);
    assert_eq!(shown.len(), 1);
    assert_eq!(shown[0].skill_type, AcquireSkillType::Class);
    // (1010,1) is bucket 4, (3,1) bucket 8, (1405,1) bucket 15. Divine Inspiration is listed
    // here: only the auto-learn lists hide it. (3,2) needs level 1 first, (3,3) is level 20,
    // (1011,1) level 40, Lucky is auto-get.
    assert_eq!(
        entries(&shown[0]),
        vec![(1010, 1, 1, 500, 0), (3, 1, 1, 100, 0), (1405, 1, 1, 50, 0)]
    );
    // The bypass handler of the npc_ branch ends with ActionFailed.
    assert_eq!(names(&sent, ME), vec!["ACQUIRE_SKILL_LIST", "ACTION_FAIL"]);
}

/// A player who knows Power Strike 1 is offered level 2, not level 1 or 3.
#[test]
fn a_known_skill_offers_its_next_level_only() {
    let mut s = Scene::new();
    let mut data = player_data(ME, 10, 1000);
    data.skills = vec![SkillData {
        skill_id: 3,
        skill_level: 1,
    }];
    s.enter(data);
    let npc = s.talk_to(ME, 1100);
    s.log.take();

    s.skill_list(ME, npc);
    let shown = lists(&s.log.take());
    // (1010,1) bucket 4, (3,2) bucket 9, (1405,1) bucket 15.
    assert_eq!(
        entries(&shown[0]),
        vec![(1010, 1, 1, 500, 0), (3, 2, 2, 200, 0), (1405, 1, 1, 50, 0)]
    );
}

#[test]
fn nothing_to_learn_says_when_to_come_back() {
    let mut s = Scene::new();
    // Level 4: Power Strike (get level 5) is out of reach; the next level a trainer teaches at
    // is 5.
    s.enter(player_data(ME, 4, 1000));
    let npc = s.talk_to(ME, 1100);
    s.log.take();

    s.skill_list(ME, npc);
    let sent = s.log.take();
    assert!(lists(&sent).is_empty());
    let messages = messages(&sent, ME);
    assert_eq!(
        messages[0].id,
        SystemMessageId::YouDoNotHaveAnyFurtherSkillsToLearnComeBackWhenYouHaveReachedLevelS1
    );
    assert_eq!(messages[0].params, vec![SystemMessageParam::IntNumber(5)]);
}

/// A trainer that does not teach the player's class shows its `-noteach` page; one that has none
/// shows Java's fallback text.
#[test]
fn a_trainer_of_another_class_shows_the_noteach_page() {
    let mut s = Scene::new();
    s.enter(player_data(ME, 10, 1000));
    // 1101 teaches class 10 only and has no -noteach page; 1100 would show its own page to a
    // mage, see below.
    let npc = s.talk_to(ME, 1101);
    s.log.take();
    s.skill_list(ME, npc);
    let sent = s.log.take();
    assert!(lists(&sent).is_empty());
    let html: Vec<l2_protocol_game::server::NpcHtmlMessage> = decoded(&sent, ME, "NPC_HTML_MESSAGE");
    assert_eq!(html.len(), 1);
    assert!(html[0].html.contains("I cannot teach you any skills."));

    // A mage (class 10) at the trainer of fighters.
    let mut mage = player_data(MAGE, 10, 1000);
    mage.class_id = 10;
    s.enter(mage);
    let npc = s.world.spawn_npc_at(1100, X + 30, Y, Z, 0).unwrap();
    for _ in 0..2 {
        s.input(
            MAGE,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
    }
    s.log.take();
    s.input(
        MAGE,
        ClientInput::Bypass {
            command: format!("npc_{}_SkillList", npc.0),
        },
    );
    let sent = s.log.take();
    let html: Vec<l2_protocol_game::server::NpcHtmlMessage> = decoded(&sent, MAGE, "NPC_HTML_MESSAGE");
    assert_eq!(html.len(), 1);
    assert_eq!(
        html[0].html,
        format!("<html><body>Wrong teacher {}.</body></html>", npc.0)
    );
}

// ------------------------------------------------------------------ AcquireSkillInfo

#[test]
fn the_info_of_a_skill_lists_its_sp_and_books() {
    let mut s = Scene::new();
    s.enter(player_data(ME, 10, 1000));
    let npc = s.talk_to(ME, 1100);
    s.skill_list(ME, npc);
    s.log.take();

    s.info(3, 1);
    s.info(1010, 1);
    let infos: Vec<AcquireSkillInfo> = decoded(&s.log.take(), ME, "ACQUIRE_SKILL_INFO");
    assert_eq!(infos.len(), 2);
    assert_eq!(
        (
            infos[0].skill_id,
            infos[0].level,
            infos[0].sp_cost,
            infos[0].requirements.clone()
        ),
        (3, 1, 100, vec![])
    );
    assert_eq!(
        (
            infos[1].skill_id,
            infos[1].level,
            infos[1].sp_cost,
            infos[1].requirements.clone()
        ),
        (1010, 1, 500, vec![AcquireSkillRequirement::item(TOKEN, 2)])
    );
    // 99 and 50 are the constants Java writes.
    assert_eq!(
        (infos[1].requirements[0].req_type, infos[1].requirements[0].unk),
        (99, 50)
    );

    // A skill that is not in the tree, or a bad id: nothing.
    s.info(3, 3 + 7);
    s.info(0, 1);
    s.info(77, 1);
    assert!(s.log.take().is_empty());
}

// ------------------------------------------------------------------ learning

/// Learning Power Strike 1: SP is taken, the skill is added, the row is written, the client gets
/// the message, the done packet, the new skill list (300 ms later) and the trainer's list again.
#[test]
fn learning_a_skill_takes_the_sp_and_stores_the_row() {
    let mut s = Scene::new();
    let mut data = player_data(ME, 10, 1000);
    // A shortcut of the skill shows the new level.
    data.shortcuts = vec![ShortcutData {
        slot: 3,
        page: 1,
        kind: 2,
        shortcut_id: 3,
        level: 1,
    }];
    s.enter(data);
    let npc = s.talk_to(ME, 1100);
    s.skill_list(ME, npc);
    s.log.take();

    s.learn(3, 1);
    s.ticks(10);
    let sent = s.log.take();

    assert_eq!(s.me().data.sp, 900);
    assert_eq!(skills_of(s.me()), vec![(3, 1)]);
    assert_eq!(s.persisted.skill_changes(), vec![(vec![(3, 1)], vec![])]);
    // The row Java would store at the next save.
    let row = s.me().snapshot();
    assert_eq!(
        row.skills,
        vec![SkillData {
            skill_id: 3,
            skill_level: 1
        }]
    );
    assert_eq!(row.sp, 900);

    // `setSp` + `updateUserInfo`, then `giveSkill`: the message, the done packet, the shortcut,
    // the list again; the SkillList follows 300 ms later.
    assert_eq!(
        names(&sent, ME),
        vec![
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "SYSTEM_MESSAGE",
            "ACQUIRE_SKILL_DONE",
            "SHORT_CUT_REGISTER",
            "ACQUIRE_SKILL_LIST",
            "SKILL_LIST",
        ]
    );
    let messages = messages(&sent, ME);
    assert_eq!(messages[0].id, SystemMessageId::YouHaveEarnedS12);
    assert_eq!(
        messages[0].params,
        vec![SystemMessageParam::SkillName { id: 3, level: 1 }]
    );
    // Slot 3 on page 1: the wire slot is 3 + 12.
    let register: Vec<ShortcutRegister> = decoded(&sent, ME, "SHORT_CUT_REGISTER");
    assert_eq!(
        register,
        vec![ShortcutRegister::Skill {
            slot: 15,
            id: 3,
            level: 1,
            character_type: 1
        }]
    );
    let skill_list: Vec<SkillList> = decoded(&sent, ME, "SKILL_LIST");
    assert!(skill_list[0].skills.iter().any(|e| e.id == 3 && e.level == 1));
    // Power Strike 2 is now offered, at 200 SP.
    assert_eq!(
        entries(&lists(&sent)[0]),
        vec![(1010, 1, 1, 500, 0), (3, 2, 2, 200, 0), (1405, 1, 1, 50, 0)]
    );

    // The next level: the stored row is an update of the same skill id.
    s.persisted.clear();
    s.learn(3, 2);
    assert_eq!(s.me().data.sp, 700);
    assert_eq!(skills_of(s.me()), vec![(3, 2)]);
    assert_eq!(s.persisted.skill_changes(), vec![(vec![(3, 2)], vec![])]);
    assert_eq!(s.me().data.shortcuts[0].level, 2);
}

#[test]
fn not_enough_sp_teaches_nothing_and_shows_the_list_again() {
    let mut s = Scene::new();
    s.enter(player_data(ME, 10, 99));
    let npc = s.talk_to(ME, 1100);
    s.skill_list(ME, npc);
    s.log.take();

    s.learn(3, 1);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, ME),
        vec![SystemMessageId::YouDoNotHaveEnoughSpToLearnThisSkill]
    );
    assert_eq!(names(&sent, ME), vec!["SYSTEM_MESSAGE", "ACQUIRE_SKILL_LIST"]);
    assert_eq!(s.me().data.sp, 99);
    assert!(skills_of(s.me()).is_empty());
    assert!(s.persisted.skill_changes().is_empty());

    // Exactly enough is enough: the SP goes to 0.
    let mut exact = player_data(ME + 10, 10, 100);
    exact.object_id = ObjectId(ME + 10);
    s.enter(exact);
    s.input(
        ME + 10,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.log.take();
    s.input(
        ME + 10,
        ClientInput::AcquireSkill {
            id: 3,
            level: 1,
            skill_type: AcquireSkillType::Class,
            sub_type: 0,
        },
    );
    let player = s.world.player(ObjectId(ME + 10)).unwrap();
    assert_eq!(player.data.sp, 0);
    assert_eq!(skills_of(player), vec![(3, 1)]);
}

/// A skill with a book: without it the lesson fails and nothing is taken; with it the book goes
/// first (`S2_S1_HAS_DISAPPEARED`), then the SP.
#[test]
fn a_missing_book_blocks_the_lesson_and_a_present_one_is_taken() {
    let mut s = Scene::new();
    let mut data = player_data(ME, 10, 1000);
    data.items = vec![token(1)];
    s.enter(data);
    let npc = s.talk_to(ME, 1100);
    s.skill_list(ME, npc);
    s.log.take();

    s.learn(1010, 1);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, ME),
        vec![SystemMessageId::YouDoNotHaveTheNecessaryMaterialsOrPrerequisitesToLearnThisSkill]
    );
    assert_eq!(names(&sent, ME), vec!["SYSTEM_MESSAGE", "ACQUIRE_SKILL_LIST"]);
    assert_eq!(s.me().data.sp, 1000);
    assert_eq!(s.me().data.items.iter().map(|i| i.count).sum::<i64>(), 1);
    assert!(s.persisted.skill_changes().is_empty());

    // Two tokens now.
    let mut rich = player_data(ME + 10, 10, 1000);
    rich.items = vec![token(5)];
    s.enter(rich);
    s.input(
        ME + 10,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.log.take();
    s.input(
        ME + 10,
        ClientInput::AcquireSkill {
            id: 1010,
            level: 1,
            skill_type: AcquireSkillType::Class,
            sub_type: 0,
        },
    );
    let sent = s.log.take();
    let player = s.world.player(ObjectId(ME + 10)).unwrap();
    assert_eq!(skills_of(player), vec![(1010, 1)]);
    assert_eq!(player.data.sp, 500);
    assert_eq!(
        player
            .data
            .items
            .iter()
            .map(|i| (i.item_id, i.count))
            .collect::<Vec<_>>(),
        vec![(TOKEN, 3)]
    );
    let messages = messages(&sent, ME + 10);
    assert_eq!(messages[0].id, SystemMessageId::S2S1HasDisappeared);
    assert_eq!(
        messages[0].params,
        vec![
            SystemMessageParam::ItemName(TOKEN),
            SystemMessageParam::LongNumber(2)
        ]
    );
    assert_eq!(messages[1].id, SystemMessageId::YouHaveEarnedS12);
}

/// `DivineInspirationSpBookNeeded = False`: the skill is free (Java returns before it takes the SP
/// and the book).
#[test]
fn divine_inspiration_without_book_is_free() {
    let mut settings = WorldSettings::default();
    settings.learn.divine_sp_book_needed = false;
    let mut s = Scene::with(settings);
    s.enter(player_data(ME, 10, 60));
    let npc = s.talk_to(ME, 1100);
    s.skill_list(ME, npc);
    s.log.take();

    s.learn(1405, 1);
    assert_eq!(skills_of(s.me()), vec![(1405, 1)]);
    assert_eq!(s.me().data.sp, 60);

    // With the default setting the same lesson needs the book.
    let mut s = Scene::new();
    let mut data = player_data(ME, 10, 60);
    data.items = vec![book(1)];
    s.enter(data);
    let npc = s.talk_to(ME, 1100);
    s.skill_list(ME, npc);
    s.log.take();
    s.learn(1405, 1);
    assert_eq!(skills_of(s.me()), vec![(1405, 1)]);
    assert_eq!(s.me().data.sp, 10);
    assert!(s.me().data.items.is_empty());
}

#[test]
fn the_level_requirement_and_the_previous_level_are_checked() {
    let mut s = Scene::new();
    let mut data = player_data(ME, 10, 5000);
    data.skills = vec![SkillData {
        skill_id: 3,
        skill_level: 1,
    }];
    s.enter(data);
    let npc = s.talk_to(ME, 1100);
    s.skill_list(ME, npc);
    s.log.take();

    // Level 40 skill at level 10.
    s.learn(1011, 1);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, ME),
        vec![SystemMessageId::YouDoNotMeetTheSkillLevelRequirements]
    );
    // No list afterwards: `checkPlayerSkill` returns false without showing it.
    assert_eq!(names(&sent, ME), vec!["SYSTEM_MESSAGE"]);

    // Level 3 while only level 1 is known.
    s.learn(3, 3);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, ME),
        vec![SystemMessageId::ThePreviousLevelSkillHasNotBeenLearned]
    );
    assert_eq!(names(&sent, ME), vec!["SYSTEM_MESSAGE"]);

    // The level that is already known: nothing at all.
    s.learn(3, 1);
    assert!(s.log.take().is_empty());
    assert_eq!(s.me().data.sp, 5000);
    assert!(s.persisted.skill_changes().is_empty());
}

/// Bad data, no trainer, a trainer too far away: Java ignores the request.
#[test]
fn requests_without_a_trainer_in_reach_do_nothing() {
    let mut s = Scene::new();
    s.enter(player_data(ME, 10, 1000));
    // No trainer clicked yet.
    s.learn(3, 1);
    s.info(3, 1);
    assert!(s.log.take().is_empty());

    let npc = s.talk_to(ME, 1100);
    s.skill_list(ME, npc);
    s.log.take();
    // Out of range values: `handleIllegalPlayerAction(..., DEFAULT_PUNISH)` (KICK) tells the
    // player, once per request; the kick comes five seconds later (tests/punishments.rs).
    s.learn(3, 0);
    s.learn(3, 1001);
    s.learn(0, 1);
    let sent = s.log.take();
    assert_eq!(names(&sent, ME), vec!["SYSTEM_MESSAGE"; 3]);
    // A skill that does not exist.
    s.learn(3, 9);
    // Other trees are not ported: ignored like a skill that is not in a tree.
    s.input(
        ME,
        ClientInput::AcquireSkill {
            id: 3,
            level: 1,
            skill_type: AcquireSkillType::Fishing,
            sub_type: 0,
        },
    );
    assert!(s.log.take().is_empty());
    assert!(skills_of(s.me()).is_empty());

    // The trainer is out of reach: the player walks away (a GM may ask from anywhere).
    s.world
        .tele_to_location(ObjectId(ME), TeleportTarget::at(X + 5000, Y, Z));
    s.log.take();
    s.learn(3, 1);
    assert!(skills_of(s.me()).is_empty());
}

// ------------------------------------------------------------------ AutoLearnSkills

/// `AutoLearnSkills = True`: a level up learns everything the trees offer, level after level,
/// except the lines with books (unless `AutoLearnSkillsWithoutItems`) and Divine Inspiration
/// (unless `AutoLearnDivineInspiration`), and the auto-get skill comes with it.
#[test]
fn auto_learn_gives_every_available_skill_without_books() {
    let mut settings = WorldSettings::default();
    settings.exp.auto_learn_skills = true;
    let mut s = Scene::with(settings);
    let mut data = player_data(ME, 1, 0);
    data.shortcuts = vec![ShortcutData {
        slot: 0,
        page: 0,
        kind: 2,
        shortcut_id: 3,
        level: 1,
    }];
    s.enter(data);

    // Level 1 to 10: Lucky (auto-get) and Power Strike 1 and 2 (both get level 5), not level 3
    // (get level 20), not the token skill (a book), not Divine Inspiration, not the level 40 one.
    assert!(s.world.add_level(ObjectId(ME), 9));
    s.ticks(10);
    let sent = s.log.take();
    assert_eq!(s.me().core.level, 10);
    assert_eq!(skills_of(s.me()), vec![(3, 2), (1020, 1)]);
    // One `storeSkills` batch: both rows, by skill id.
    let changes = s.persisted.skill_changes();
    assert_eq!(changes, vec![(vec![(3, 2), (1020, 1)], vec![])]);
    assert_eq!(s.me().data.shortcuts[0].level, 2);

    // "You have learned 2 new skills." and the shortcut bar.
    let texts: Vec<_> = messages(&sent, ME)
        .into_iter()
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(text)) => Some(text.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(texts, vec!["You have learned 2 new skills.".to_owned()]);
    let init: Vec<ShortcutInit> = decoded(&sent, ME, "SHORT_CUT_INIT");
    assert_eq!(init.len(), 1);
    let register: Vec<ShortcutRegister> = decoded(&sent, ME, "SHORT_CUT_REGISTER");
    // Java learns the final level of each skill in one go (the map holds Power Strike 2 only).
    assert_eq!(register.len(), 1);
    assert!(matches!(register[0], ShortcutRegister::Skill { level: 2, .. }));
    let skill_list: Vec<SkillList> = decoded(&sent, ME, "SKILL_LIST");
    assert_eq!(skill_list.len(), 1);
}

#[test]
fn auto_learn_without_items_takes_the_book_skills_too() {
    let mut settings = WorldSettings::default();
    settings.exp.auto_learn_skills = true;
    settings.learn.auto_learn_skills_without_items = true;
    settings.learn.auto_learn_divine_inspiration = true;
    let mut s = Scene::with(settings);
    s.enter(player_data(ME, 1, 0));

    assert!(s.world.add_level(ObjectId(ME), 9));
    // The token skill and Divine Inspiration come for free; SP is not charged.
    assert_eq!(skills_of(s.me()), vec![(3, 2), (1010, 1), (1020, 1), (1405, 1)]);
    assert_eq!(s.me().data.sp, 0);
}

/// Without `AutoLearnSkills` only the auto-get skills come with a level.
#[test]
fn without_auto_learn_only_auto_get_skills_come_with_a_level() {
    let mut s = Scene::new();
    s.enter(player_data(ME, 1, 0));
    assert!(s.world.add_level(ObjectId(ME), 9));
    assert_eq!(skills_of(s.me()), vec![(1020, 1)]);
}

// ------------------------------------------------------------------ SP price of other classes

/// `AltGameSkillLearn = True`: the `SkillList` link first shows a menu of classes (the classes of
/// the trainer that have something to learn), and a skill of another class costs double, triple
/// for the other kind (mage or fighter).
#[test]
fn alt_game_skill_learn_shows_a_class_menu_and_charges_more() {
    let mut settings = WorldSettings::default();
    settings.learn.alt_game_skill_learn = true;
    let mut s = Scene::with(settings);
    s.enter(player_data(ME, 10, 1000));
    // Trainer 1101 teaches class 10 (Human Mystic). The player is a Human Fighter (class 0).
    let npc = s.talk_to(ME, 1101);
    s.log.take();

    s.skill_list(ME, npc);
    let sent = s.log.take();
    let html: Vec<l2_protocol_game::server::NpcHtmlMessage> = decoded(&sent, ME, "NPC_HTML_MESSAGE");
    assert_eq!(html.len(), 1);
    // Not the player's own class: the explanation, with "fighter"/"mage" as the hardest.
    assert!(
        html[0]
            .html
            .contains("But the hardest of all to learn are the  mage")
    );
    assert!(
        html[0]
            .html
            .contains(&format!("npc_{}_SkillList 10\">Learn MAGE's class Skills", npc.0))
    );

    // The class link: the mage skill (100 SP) costs 3 times for a fighter.
    s.input(
        ME,
        ClientInput::Bypass {
            command: format!("npc_{}_SkillList 10", npc.0),
        },
    );
    let sent = s.log.take();
    assert_eq!(entries(&lists(&sent)[0]), vec![(3, 1, 1, 300, 0)]);

    s.info(3, 1);
    let infos: Vec<AcquireSkillInfo> = decoded(&s.log.take(), ME, "ACQUIRE_SKILL_INFO");
    assert_eq!(infos[0].sp_cost, 300);

    s.learn(3, 1);
    assert_eq!(s.me().data.sp, 700);
    assert_eq!(skills_of(s.me()), vec![(3, 1)]);
}

// ------------------------------------------------------------------ the real datapack

/// The shipped datapack, if it is there (`L2JMOBIUS_GAME_DIR` or the usual folder).
fn game_dir() -> Option<std::path::PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
    if dir.join("data").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: {} does not exist", dir.display());
        None
    }
}

/// Auron (30010, a trainer of the human fighter classes) and a Human Fighter of level 20 who knows
/// nothing: the list is what `HumanFighter.xml` offers (`learnedByNpc`, level 1 of each skill,
/// `getLevel <= 20`), in the order of Java's `HashMap` (56 keys, 128 buckets), with the file's
/// `levelUpSp`. The expected list was worked out from the XML with the bucket formula of
/// `HashMap.hash`, not with this code.
#[test]
fn the_real_human_fighter_tree_lists_like_java() {
    let Some(dir) = game_dir() else { return };
    let skills: String = {
        let ids = [3, 16, 56, 141, 142, 226];
        format!(
            "<list>{}</list>",
            ids.iter().map(|id| passive(*id, 1)).collect::<String>()
        )
    };
    let npcs = r#"<?xml version="1.0"?><list>
        <npc id="30010" type="Trainer" level="10" name="Auron" usingServerSideName="true"/></list>"#;
    let mut s = Scene::build(
        WorldSettings::default(),
        Arc::new(SkillTrees::load(&dir)),
        Arc::new(SkillLearnData::load(&dir)),
        npcs,
        &skills,
    );
    s.enter(player_data(ME, 20, 100_000));
    let npc = s.talk_to(ME, 30010);
    s.log.take();

    s.skill_list(ME, npc);
    let shown = lists(&s.log.take());
    assert_eq!(
        entries(&shown[0]),
        vec![
            (16, 1, 1, 50, 0),
            (142, 1, 1, 160, 0),
            (141, 1, 1, 160, 0),
            (226, 1, 1, 160, 0),
            (56, 1, 1, 50, 0),
            (3, 1, 1, 50, 0),
        ]
    );

    // Power Strike for 50 SP.
    s.learn(3, 1);
    assert_eq!(s.me().data.sp, 99_950);
    assert_eq!(s.persisted.skill_changes(), vec![(vec![(3, 1)], vec![])]);
}
