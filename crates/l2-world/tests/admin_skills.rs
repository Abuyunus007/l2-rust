#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The skill and buff admin commands (M5-ADMIN-04): `//add_skill`, `//remove_skill`,
//! `//give_all_skills`, `//remove_all_skills`, `//skill_list`, `//getbuffs`, `//stopbuff`,
//! `//stopallbuffs` and `//areacancel`, with a fake GM, fake players and a fake monster.
//! Java: scripts/handlers/chat/commands/admin/{AdminSkill,AdminBuffs}.java.
//!
//! The harness is the one of `admin_combat.rs`, with a recording outbox (the `character_skills`
//! rows) and a class skill tree (the skills `//give_all_skills` gives).

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ExperienceLoss, ExperienceTable, HtmCache, ItemTemplates, KarmaLoss, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillLearnData, SkillLoadOptions, SkillTemplates, SkillTrees, SpawnData,
    SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

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

/// The levels of the shipped `AccessLevels.xml` (only 70 and 100 are GMs) and the rights of the
/// commands these tests use, with the levels and the confirm flags of the shipped
/// `AdminCommands.xml`.
fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [
        (-1, 0),
        (0, 0),
        (10, 0),
        (20, 10),
        (30, 20),
        (40, 30),
        (50, 40),
        (60, 50),
        (70, 60),
        (100, 70),
    ] {
        rights.add_level(level, child);
    }
    for (command, level, confirm) in [
        ("add_skill", 100, false),
        ("remove_skill", 100, false),
        ("skill_list", 100, false),
        ("give_all_skills", 100, false),
        ("remove_all_skills", 100, false),
        ("getbuffs", 100, false),
        ("stopbuff", 100, false),
        ("stopallbuffs", 100, true),
        ("areacancel", 100, false),
    ] {
        rights.add_command(command, level, confirm);
    }
    rights
}

/// A passive skill (`Power Strike`, 3 levels, the tree of class 0), an auto-get passive (`Lucky`),
/// a passive with a reflect effect (`Reflect`, 2 levels), and a timed buff (`Might`, 60 seconds).
const SKILLS: &str = r#"<list>
<skill id="3" levels="3" name="Power Strike"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="mDef" val="1"/></effect></effects></skill>
<skill id="1020" levels="1" name="Lucky"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="mDef" val="1"/></effect></effects></skill>
<skill id="3100" levels="2" name="Reflect"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="reflectDam" val="50"/></effect></effects></skill>
<skill id="9001" levels="1" name="Might"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
</list>"#;

/// Class 0's tree: `Power Strike` levels 1 to 3 (levels 5, 5 and 20) and the auto-get `Lucky`.
const TREES: &str = r#"<list>
<skillTree type="classSkillTree" classId="0">
<skill skillName="Power Strike" skillId="3" skillLevel="1" getLevel="5" levelUpSp="100" learnedByNpc="true" />
<skill skillName="Power Strike" skillId="3" skillLevel="2" getLevel="5" levelUpSp="200" learnedByNpc="true" />
<skill skillName="Power Strike" skillId="3" skillLevel="3" getLevel="20" levelUpSp="300" learnedByNpc="true" />
<skill skillName="Lucky" skillId="1020" skillLevel="1" getLevel="1" autoGet="true" />
</skillTree>
</list>"#;

const SKILL_LEARN: &str = r#"<list>
<npc id="1100"><classId>0</classId></npc>
</list>"#;

fn class_data() -> StatData {
    let mut levels = String::new();
    for level in 1..=40 {
        let (hp, mp, cp) = if level == 40 {
            (100, 50, 10)
        } else {
            (level, level, level)
        };
        levels.push_str(&format!(
            "<level val=\"{level}\"><hp>{hp}</hp><mp>{mp}</mp><cp>{cp}</cp></level>"
        ));
    }
    let xml = format!(
        r#"<list><classId>0</classId><staticData>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{levels}</lvlUpgainData></list>"#
    );
    StatData {
        player_templates: Arc::new(PlayerTemplates::from_xml(&xml, 85).unwrap()),
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
        ..StatData::default()
    }
}

struct Fake {
    rights: AdminRights,
    stats: StatData,
}

impl Fake {
    fn new(rights: AdminRights) -> Self {
        Self {
            rights,
            stats: class_data(),
        }
    }
}

impl StaticData for Fake {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        if level >= 70 {
            AccessLevelStatic {
                name_color: 0,
                title_color: 0,
                is_gm: true,
            }
        } else {
            AccessLevelStatic::USER
        }
    }
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.rights)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.stats
            .skills
            .iter()
            .any(|s| s.id == skill_id && s.is_passive())
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

fn decoded<P: Decode>(out: &Sent, to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

/// The text of every `sendMessage` (system message `S1_2` with one string) sent to `to`.
fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The ids of the system messages (those with no string) sent to `to`.
fn system_ids_to(out: &Sent, to: i32) -> Vec<SystemMessageId> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter(|m| m.id != SystemMessageId::S12)
        .map(|m| m.id)
        .collect()
}

/// `experience.xml` for levels 1 to 88 (`maxLevel` 85), as the other admin tests have it.
fn experience_table() -> Arc<ExperienceTable> {
    let rows: String = (1..=88)
        .map(|level| {
            format!(
                r#"<experience level="{level}" tolevel="{}" />"#,
                level * level * 1000
            )
        })
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

const GM: i32 = 100;
const USER: i32 = 1;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="1001" type="Folk" level="10" name="Villager" usingServerSideName="true"/>
  <npc id="2000" type="Monster" level="3" name="Rat">
    <stats><vitals hp="62" mp="10"/></stats>
  </npc>
</list>"#;

struct Root(tempfile::TempDir);

impl Root {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("data/spawns/none.xml");
        std::fs::create_dir_all(file.parent().unwrap()).unwrap();
        std::fs::write(file, r#"<?xml version="1.0"?><list enabled="true"></list>"#).unwrap();
        Self(dir)
    }

    fn path(&self) -> &Path {
        self.0.path()
    }
}

struct Setup {
    world: World,
    log: Log,
    persisted: Persisted,
    _root: Root,
}

fn setup() -> Setup {
    let root = Root::new();
    let log = Log::default();
    let persisted = Persisted::default();
    let mut world = World::new(
        WorldRng::from_seed(11),
        Box::new(RecordingOutbox(persisted.clone())),
    );
    world.set_static_data(Box::new(Fake::new(rights())));
    world.set_settings(WorldSettings::default());
    world.set_exp_data(
        experience_table(),
        Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
        Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
        Arc::new(SkillTrees::from_xml(TREES).unwrap()),
    );
    world.set_skill_learn_data(Arc::new(SkillLearnData::from_xml(SKILL_LEARN).unwrap()));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    Setup {
        world,
        log,
        persisted,
        _root: root,
    }
}

fn player(id: i32, access_level: i32, dx: i32) -> PlayerData {
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
        level: 40,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level,
        x: X + dx,
        y: Y,
        z: Z,
        heading: 321,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 10.0,
        max_cp: 10.0,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

/// A known skill of a character, as stored in `character_skills`.
fn known(skill_id: i32, skill_level: i32) -> SkillData {
    SkillData {
        skill_id,
        skill_level,
    }
}

impl Setup {
    /// A player whose object id is its access level (or 1 for the plain user).
    fn enter(&mut self, id: i32, dx: i32) {
        let level = if id == USER { 0 } else { id };
        self.enter_with(player(id, level, dx));
    }

    fn enter_with(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    /// `//command`.
    fn cmd(&mut self, player: i32, command: &str) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input: ClientInput::BuildCmd {
                command: command.to_owned(),
            },
        });
    }

    /// The skill level of a character (0 for none), through the stat view.
    fn level_of(&self, id: i32, skill_id: i32) -> i32 {
        self.world.stats(ObjectId(id)).unwrap().skill_level(skill_id)
    }
}

// ------------------------------------------------------------------------- //add_skill

#[test]
fn add_skill_gives_the_skill_and_stores_its_row() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "add_skill 3100 2");
    assert_eq!(s.level_of(USER, 3100), 2);
    // Java: `addSkill(skill, true)` stores the row.
    assert_eq!(s.persisted.skill_changes(), [(vec![(3100, 2)], vec![])]);
    let out = s.log.take();
    assert_eq!(texts_to(&out, USER), ["Admin gave you the skill Reflect."]);
    assert_eq!(texts_to(&out, GM), ["You gave the skill Reflect to P1."]);
}

#[test]
fn add_skill_level_defaults_to_one_and_a_high_level_gets_the_highest() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    // No level: 1.
    s.cmd(GM, "add_skill 3100");
    assert_eq!(s.level_of(USER, 3100), 1);
    assert_eq!(s.persisted.skill_changes(), [(vec![(3100, 1)], vec![])]);

    // `SkillData.getSkill` gives the highest level (2) for a level above it.
    s.persisted.clear();
    s.cmd(GM, "add_skill 3100 9");
    assert_eq!(s.level_of(USER, 3100), 2);
    assert_eq!(s.persisted.skill_changes(), [(vec![(3100, 2)], vec![])]);
}

#[test]
fn add_skill_with_a_bad_skill_says_there_is_no_such_skill() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    for command in ["add_skill 9999 1", "add_skill abc", "add_skill 3100 x"] {
        s.cmd(GM, command);
        assert_eq!(
            texts_to(&s.log.take(), GM),
            ["Error: there is no such skill."],
            "{command}"
        );
    }
    assert_eq!(s.level_of(USER, 3100), 0);
    assert!(s.persisted.skill_changes().is_empty());

    // Three words, or none: Java opens the page of the target, which is not ported; nothing is
    // given and nothing is sent.
    s.cmd(GM, "add_skill 3100 1 2");
    s.cmd(GM, "add_skill");
    assert!(texts_to(&s.log.take(), GM).is_empty());
    assert_eq!(s.level_of(USER, 3100), 0);
}

#[test]
fn add_skill_without_a_target_says_invalid_target_twice() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    // Java: `INVALID_TARGET`, then `showMainPage` checks the target and says it again.
    s.cmd(GM, "add_skill 3100 1");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::InvalidTarget, SystemMessageId::InvalidTarget]
    );
    assert!(s.persisted.skill_changes().is_empty());
}

// ---------------------------------------------------------------------------- //remove_skill

#[test]
fn remove_skill_takes_the_skill_and_stores_the_removal() {
    let mut s = setup();
    s.enter(GM, 0);
    let mut user = player(USER, 0, 100);
    user.skills = vec![known(3100, 2)];
    s.enter_with(user);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "remove_skill 3100");
    assert_eq!(s.level_of(USER, 3100), 0);
    assert_eq!(s.persisted.skill_changes(), [(vec![], vec![3100])]);
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, USER),
        ["Admin removed the skill Reflect from your skills list."]
    );
    assert_eq!(texts_to(&out, GM), ["You removed the skill Reflect from P1."]);
}

#[test]
fn remove_skill_errors_and_the_usage_line() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    // Not known: `getSkill(id, 0)` is null.
    s.cmd(GM, "remove_skill 3100");
    assert_eq!(texts_to(&s.log.take(), GM), ["Error: there is no such skill."]);

    // Not a number, or no number: the usage line.
    s.cmd(GM, "remove_skill x");
    s.cmd(GM, "remove_skill");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Usage: //remove_skill <skill_id>",
            "Usage: //remove_skill <skill_id>"
        ]
    );
}

// ---------------------------------------------------------------- //give_all_skills and //remove_all_skills

#[test]
fn give_all_skills_learns_what_the_tree_offers() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    // Level 40, nothing known: `Power Strike` at its highest level (3) and `Lucky`. Java's
    // `giveAvailableSkills(false, true, false)` counts two new skills.
    s.cmd(GM, "give_all_skills");
    assert_eq!(s.level_of(USER, 3), 3);
    assert_eq!(s.level_of(USER, 1020), 1);
    let mut rows = s.persisted.skill_changes();
    assert_eq!(rows.len(), 1);
    let (mut upserts, _) = rows.remove(0);
    upserts.sort_unstable();
    assert_eq!(upserts, [(3, 3), (1020, 1)]);
    let out = s.log.take();
    assert_eq!(texts_to(&out, USER), ["You have learned 2 new skills."]);
    assert_eq!(texts_to(&out, GM), ["You gave 2 skills to P1"]);
}

#[test]
fn give_and_remove_all_need_a_player_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), None);
    s.log.take();

    // Without a target.
    s.cmd(GM, "give_all_skills");
    s.cmd(GM, "remove_all_skills");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::InvalidTarget, SystemMessageId::InvalidTarget]
    );

    // A monster is not a player: the same answer, and nothing changes.
    let rat = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    s.world.set_target(ObjectId(GM), Some(rat));
    s.cmd(GM, "give_all_skills");
    s.cmd(GM, "remove_all_skills");
    s.cmd(GM, "remove_skill 3100");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [
            SystemMessageId::InvalidTarget,
            SystemMessageId::InvalidTarget,
            SystemMessageId::InvalidTarget
        ]
    );
    assert!(s.persisted.skill_changes().is_empty());
}

#[test]
fn remove_all_skills_removes_every_skill_and_stores_the_rows() {
    let mut s = setup();
    s.enter(GM, 0);
    let mut user = player(USER, 0, 100);
    user.skills = vec![known(3100, 2), known(1020, 1)];
    s.enter_with(user);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "remove_all_skills");
    assert_eq!(s.level_of(USER, 3100), 0);
    assert_eq!(s.level_of(USER, 1020), 0);
    let mut rows = s.persisted.skill_changes();
    assert_eq!(rows.len(), 1);
    let (upserts, mut removed) = rows.remove(0);
    assert!(upserts.is_empty());
    removed.sort_unstable();
    assert_eq!(removed, [1020, 3100]);
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["You have removed all skills from P1."]);
    assert_eq!(texts_to(&out, USER), ["Admin removed all skills from you."]);
}

// ------------------------------------------------------------------------------ //skill_list

#[test]
fn skill_list_is_registered_but_its_page_is_not_ported() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    // Java: `charskills_add.htm` with the race list. The page is FOLLOW-UP: nothing is sent.
    s.cmd(GM, "skill_list");
    assert!(s.log.take().is_empty());
}

// ------------------------------------------------------------------------------ //getbuffs

#[test]
fn getbuffs_checks_the_target_and_the_online_name() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    // No target: `THAT_IS_AN_INCORRECT_TARGET`.
    s.cmd(GM, "getbuffs");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::ThatIsAnIncorrectTarget]
    );

    // A name that is not online.
    s.cmd(GM, "getbuffs nobody");
    assert_eq!(texts_to(&s.log.take(), GM), ["The player nobody is not online."]);

    // A page that is not a number is the exception of the command (checked for an online player).
    s.cmd(GM, "getbuffs p1 x");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Exception during execution of  'admin_getbuffs p1 x': java.lang.NumberFormatException: For input string: \"x\""
        ]
    );

    // An online name, or a target: the page of the buffs is FOLLOW-UP, so nothing is sent.
    s.cmd(GM, "getbuffs p1");
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.cmd(GM, "getbuffs");
    assert!(texts_to(&s.log.take(), GM).is_empty());
}

// ---------------------------------------------------------------------------- //stopbuff

#[test]
fn stopbuff_removes_the_effect_of_one_skill_of_the_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world
        .apply_effects(ObjectId(USER), ObjectId(USER), SkillKey::new(9001, 1));
    assert!(s.world.is_affected_by_skill(ObjectId(USER), 9001));
    s.log.take();

    // Java: `stopSkillEffects(REMOVED, 9001)` on the object 1 and its message.
    s.cmd(GM, "stopbuff 1 9001");
    assert!(!s.world.is_affected_by_skill(ObjectId(USER), 9001));
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Removed skill ID: 9001 effects from P1 (1)."]
    );

    // Not affected any more, an object that is not there, or a skill id of 0: nothing is said.
    s.cmd(GM, "stopbuff 1 9001");
    s.cmd(GM, "stopbuff 999999 9001");
    s.cmd(GM, "stopbuff 1 0");
    assert!(texts_to(&s.log.take(), GM).is_empty());
}

#[test]
fn stopbuff_with_bad_words_gives_the_java_messages() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    // A missing skill id: `NoSuchElementException` has no message, so "null".
    s.cmd(GM, "stopbuff 1");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Failed removing effect: null",
            "Usage: //stopbuff <objectId> <skillId>"
        ]
    );

    s.cmd(GM, "stopbuff abc 9001");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Failed removing effect: For input string: \"abc\"",
            "Usage: //stopbuff <objectId> <skillId>"
        ]
    );
}

// ------------------------------------------------------------------------- //stopallbuffs

#[test]
fn stopallbuffs_stops_every_effect_of_the_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world
        .apply_effects(ObjectId(USER), ObjectId(USER), SkillKey::new(9001, 1));
    s.log.take();

    // The part that runs after the confirm dialog (the framework does not run the dialog yet).
    s.world.admin_command(ObjectId(GM), "admin_stopallbuffs 1", false);
    assert!(!s.world.is_affected_by_skill(ObjectId(USER), 9001));
    assert_eq!(texts_to(&s.log.take(), GM), ["Removed all effects from P1 (1)"]);

    // A missing object id: the Java failure lines (the handler, past the confirm dialog).
    s.world.admin_command(ObjectId(GM), "admin_stopallbuffs", false);
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Failed removing all effects: null",
            "Usage: //stopallbuffs <objectId>"
        ]
    );
}

#[test]
fn stopallbuffs_from_the_chat_waits_for_the_confirm_dialog() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world
        .apply_effects(ObjectId(USER), ObjectId(USER), SkillKey::new(9001, 1));
    s.log.take();

    // `AdminCommands.xml` has `confirmDlg="true"` for stopallbuffs: the framework tells the GM and
    // runs nothing until the ConfirmDlg card.
    s.cmd(GM, "stopallbuffs 1");
    assert!(s.world.is_affected_by_skill(ObjectId(USER), 9001));
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["The command 'stopallbuffs' needs a confirmation dialog, which is not available yet."]
    );
}

// ------------------------------------------------------------------------------ //areacancel

#[test]
fn areacancel_stops_the_players_in_the_radius_and_not_the_gm() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.enter(2, 1500);
    for id in [GM, USER, 2] {
        s.world
            .apply_effects(ObjectId(id), ObjectId(id), SkillKey::new(9001, 1));
    }
    s.log.take();

    // 500 units: the plain user (100 away) is in, the player 1500 away is not, the GM is the centre.
    s.cmd(GM, "areacancel 500");
    assert!(!s.world.is_affected_by_skill(ObjectId(USER), 9001));
    assert!(s.world.is_affected_by_skill(ObjectId(2), 9001));
    assert!(s.world.is_affected_by_skill(ObjectId(GM), 9001));
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Effects canceled for 1 player(s) within radius 500"]
    );
}

#[test]
fn areacancel_with_a_bad_radius() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    s.cmd(GM, "areacancel");
    s.cmd(GM, "areacancel x");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //areacancel <radius>", "Invalid radius value."]
    );
}

// ------------------------------------------------------------------------------ rights

#[test]
fn a_gm_below_the_level_of_the_command_is_refused() {
    let mut s = setup();
    s.enter(70, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(70), Some(ObjectId(USER)));
    s.log.take();

    // Level 70 has no right to `add_skill` (100): the framework says so and nothing is given.
    s.cmd(70, "add_skill 3100 1");
    assert_eq!(
        texts_to(&s.log.take(), 70),
        ["You don't have the access rights to use this command!"]
    );
    assert_eq!(s.level_of(USER, 3100), 0);
}
