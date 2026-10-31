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
use l2_data::stats::AbnormalType;
use l2_data::{
    ClassList, ExperienceLoss, ExperienceTable, HtmCache, ItemTemplates, KarmaLoss, NpcLoadOptions,
    NpcTemplates, PlayerTemplates, SkillLearnData, SkillLoadOptions, SkillTemplates, SkillTrees, SpawnData,
    SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{ConfirmDlg, NpcHtmlMessage, ShortcutInit, SystemMessage, SystemMessageParam};
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
        ("show_skills", 100, false),
        ("remove_skills", 100, false),
        ("skill_race", 100, false),
        ("skill_class", 100, false),
        ("skill_add_list", 100, false),
        ("skill_index", 100, false),
        ("getbuffs_ps", 100, false),
        ("viewskilleffects", 100, false),
        ("viewskilleffects_ps", 100, false),
        ("viewblockedeffects", 100, false),
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
<skill id="3" levels="3" name="Power Strike"><icon>icon.skill0003</icon><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="mDef" val="1"/></effect></effects></skill>
<skill id="1020" levels="1" name="Lucky"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="mDef" val="1"/></effect></effects></skill>
<skill id="3100" levels="2" name="Reflect"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="reflectDam" val="50"/></effect></effects></skill>
<skill id="9001" levels="1" name="Might"><icon>icon.skill9001</icon><operateType>A2</operateType><targetType>SELF</targetType>
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
<skillTree type="classSkillTree" classId="1">
<skill skillName="Reflect" skillId="3100" skillLevel="1" getLevel="20" levelUpSp="100" learnedByNpc="true" />
<skill skillName="Reflect" skillId="3100" skillLevel="2" getLevel="30" levelUpSp="100" learnedByNpc="true" />
</skillTree>
<skillTree type="classSkillTree" classId="18">
<skill skillName="Reflect" skillId="3100" skillLevel="1" getLevel="5" levelUpSp="100" learnedByNpc="true" />
</skillTree>
</list>"#;

/// `classList.xml` with the classes of the trees above.
const CLASS_LIST: &str = r#"<list>
<class classId="0" name="Human Fighter" />
<class classId="1" name="Warrior" parentClassId="0" />
<class classId="18" name="Elven Fighter" />
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
    // Twelve passive skills for the paging of the removal page (ids 501 to 512).
    let mut skills = SKILLS.trim_end_matches("</list>").to_owned();
    for id in 501..=512 {
        skills.push_str(&format!(
            "<skill id=\"{id}\" levels=\"1\" name=\"Filler {id}\"><operateType>P</operateType><targetType>SELF</targetType></skill>"
        ));
    }
    skills.push_str("</list>");
    StatData {
        player_templates: Arc::new(PlayerTemplates::from_xml(&xml, 85).unwrap()),
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        skills: Arc::new(SkillTemplates::from_xml(
            &skills,
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

/// The html of every admin page (`NPC_HTML_MESSAGE`) sent to `to`.
fn html_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|m| m.html)
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

/// The pages the skill and buff commands fill in (`data/html/admin/*`), with the same placeholders
/// as the shipped files (a test below checks that against the datapack when it is there).
const ADMIN_PAGES: &[(&str, &str)] = &[
    (
        "charskills.htm",
        "<html><body>EDIT %name% Lv %level% %class%</body></html>",
    ),
    (
        "charskills_add.htm",
        "<html><body>ADD<table>%racelist%</table></body></html>",
    ),
    (
        "charskills_race.htm",
        "<html><title>%racename% Classes</title><body><table>%classlist%</table></body></html>",
    ),
    (
        "charskills_class.htm",
        "<html><title>%classname% Skills</title><body>%skilllist%<br>%pages%<br></body></html>",
    ),
    (
        "charskills_add_list.htm",
        "<html><title>Add Skill (%skillid%)</title><body>%classId%|%skilllist%<br>%pages%<br></body></html>",
    ),
    (
        "getbuff_skills.htm",
        "<html><body>Effects of %targetName% (%targetObjId%)|%buffs%|%pages%|\
         <button value=\"%buffsText%\" action=\"bypass admin_getbuffs%passives% %targetName%\">|\
         Total skill count: %skillSize%</body></html>",
    ),
    (
        "getbuff_effects.htm",
        // The shipped page does not use `%buffsText%` and `%passives%`, though Java replaces them.
        "<html><body>Effects of %targetName% (%targetObjId%)|%buffs%|%pages%|\
         Total effect count: %effectSize%</body></html>",
    ),
];

struct Root(tempfile::TempDir);

impl Root {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = dir.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        // The admin pages of the shipped datapack, cut down to their placeholders.
        for (name, body) in ADMIN_PAGES {
            write(&format!("data/html/admin/{name}"), body);
        }
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
    world.set_class_list(Arc::new(ClassList::from_xml(CLASS_LIST).unwrap()));
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

    /// `DlgAnswer`.
    fn answer(&mut self, player: i32, message_id: i32, answer: i32) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input: ClientInput::DlgAnswer {
                message_id,
                answer,
                requester_id: 0,
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
    // `showMainPage`: `charskills.htm` of the target, with the class name of `ClassListData`.
    assert_eq!(
        html_to(&out, GM),
        ["<html><body>EDIT P1 Lv 40 Human Fighter</body></html>"]
    );
}

/// Java catches whatever `adminAddSkill` throws (here the `NullPointerException` of a class the
/// class list does not have, in `showMainPage`) and prints the usage line.
#[test]
fn add_skill_prints_the_usage_when_the_page_throws() {
    let mut s = setup();
    s.world.set_class_list(Arc::new(ClassList::default()));
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "add_skill 3100 2");
    // The skill was given before the page failed.
    assert_eq!(s.level_of(USER, 3100), 2);
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        [
            "You gave the skill Reflect to P1.",
            "Usage: //add_skill <skill_id> <level>"
        ]
    );
    assert!(html_to(&out, GM).is_empty());

    // `//remove_skill` ends with the page of the removable skills, which also needs the class.
    s.cmd(GM, "remove_skill 3100");
    assert_eq!(s.level_of(USER, 3100), 0);
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "You removed the skill Reflect from P1.",
            "Usage: //remove_skill <skill_id>"
        ]
    );

    // `//show_skills` and the menu are not in a `try`: the dispatcher reports the exception.
    s.cmd(GM, "show_skills");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Exception during execution of  'admin_show_skills': java.lang.NullPointerException"]
    );
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

    // Three words, or none: Java opens the page of the target; nothing is given and no message
    // is sent.
    s.cmd(GM, "add_skill 3100 1 2");
    s.cmd(GM, "add_skill");
    let out = s.log.take();
    assert!(texts_to(&out, GM).is_empty());
    assert_eq!(html_to(&out, GM).len(), 2);
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
    // Java: `removeSkillsPage(activeChar, 0)`, the page of the skills the target still has.
    let pages = html_to(&out, GM);
    assert_eq!(pages.len(), 1);
    assert!(pages[0].contains("Editing <font color=\"LEVEL\">P1</font>"));
    assert!(pages[0].contains("<td>Lv: 40 Human Fighter</td>"));
}

const REMOVE_PAGE_HEAD: &str = "<html><body><table width=260><tr><td width=40><button value=\"Main\" action=\"bypass admin_admin\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td width=180><center>Character Selection Menu</center></td><td width=40><button value=\"Back\" action=\"bypass -h admin_show_skills\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br><br><center>Editing <font color=\"LEVEL\">P1</font></center><br><table width=270><tr><td>Lv: 40 Human Fighter</td></tr></table><br><table width=270><tr><td>Note: Do not forget that modifying players skills can</td></tr><tr><td>ruin the game...</td></tr></table><br><center>Click on the skill you wish to remove:</center><br><center><table width=270><tr>";
const REMOVE_PAGE_MID: &str = "</tr></table></center><br><table width=270><tr><td width=80>Name:</td><td width=60>Level:</td><td width=40>Id:</td></tr>";
const REMOVE_PAGE_TAIL: &str = "</table><br><center><table>Remove skill by ID :<tr><td>Id: </td><td><edit var=\"id_to_remove\" width=110></td></tr></table></center><center><button value=\"Remove skill\" action=\"bypass -h admin_remove_skill $id_to_remove\" width=110 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center><br><center><button value=\"Back\" action=\"bypass -h admin_current_player\" width=40 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>";

fn remove_row(skill_id: i32) -> String {
    format!(
        "<tr><td width=80><a action=\"bypass -h admin_remove_skill {skill_id}\">Filler {skill_id}</a></td><td width=60>1</td><td width=40>{skill_id}</td></tr>"
    )
}

/// `//remove_skills <page>`: ten skills a page in the order of Java's `ConcurrentHashMap` of skill
/// ids (512 lands in bucket 0 of the 32 buckets, before 501), a link per page, and Java's odd
/// clamps of the page number.
#[test]
fn remove_skills_page_lists_ten_skills_per_page() {
    let mut s = setup();
    s.enter(GM, 0);
    let mut user = player(USER, 0, 100);
    user.skills = (501..=512).map(|id| known(id, 1)).collect();
    s.enter_with(user);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    let links = "<td><a action=\"bypass -h admin_remove_skills 0\">Page 1</a></td><td><a action=\"bypass -h admin_remove_skills 1\">Page 2</a></td>";
    let first: String = [512, 501, 502, 503, 504, 505, 506, 507, 508, 509]
        .into_iter()
        .map(remove_row)
        .collect();
    s.cmd(GM, "remove_skills 0");
    assert_eq!(
        html_to(&s.log.take(), GM),
        [format!(
            "{REMOVE_PAGE_HEAD}{links}{REMOVE_PAGE_MID}{first}{REMOVE_PAGE_TAIL}"
        )]
    );

    let second: String = [510, 511].into_iter().map(remove_row).collect();
    let expected = format!("{REMOVE_PAGE_HEAD}{links}{REMOVE_PAGE_MID}{second}{REMOVE_PAGE_TAIL}");
    s.cmd(GM, "remove_skills 1");
    assert_eq!(html_to(&s.log.take(), GM), vec![expected.clone()]);
    // A page past the last one is the page `maxPages` (2): empty.
    s.cmd(GM, "remove_skills 9");
    assert_eq!(
        html_to(&s.log.take(), GM),
        [format!(
            "{REMOVE_PAGE_HEAD}{links}{REMOVE_PAGE_MID}{REMOVE_PAGE_TAIL}"
        )]
    );

    // A negative page indexes the array below 0 in Java.
    s.cmd(GM, "remove_skills -1");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Exception during execution of  'admin_remove_skills -1': java.lang.ArrayIndexOutOfBoundsException: Index -10 out of bounds for length 12"
        ]
    );
    // A command too short for `substring(20)` does nothing; an empty number is an exception.
    s.cmd(GM, "remove_skills");
    assert!(s.log.take().is_empty());
    s.cmd(GM, "remove_skills ");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Exception during execution of  'admin_remove_skills ': java.lang.NumberFormatException: For input string: \"\""
        ]
    );
}

#[test]
fn remove_skills_page_needs_a_player_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    s.cmd(GM, "remove_skills 0");
    let out = s.log.take();
    assert_eq!(
        system_ids_to(&out, GM),
        [SystemMessageId::ThatIsAnIncorrectTarget]
    );
    assert!(html_to(&out, GM).is_empty());
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

/// `Player.removeSkill` deletes the shortcuts of the skill: one `ShortcutInit` for each. The
/// skills 3080 to 3259 keep theirs (the condition in Java).
#[test]
fn remove_skill_deletes_the_shortcuts_of_the_skill() {
    let mut s = setup();
    s.enter(GM, 0);
    let mut user = player(USER, 0, 100);
    user.skills = vec![known(1020, 1), known(3100, 2)];
    let sc = |slot, page, kind, shortcut_id| ShortcutData {
        slot,
        page,
        kind,
        shortcut_id,
        level: 1,
    };
    user.shortcuts = vec![
        sc(0, 0, 2, 1020),
        sc(1, 0, 2, 3100),
        sc(2, 0, 4, 1020),
        sc(0, 1, 2, 1020),
    ];
    s.enter_with(user);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "remove_skill 1020");
    let out = s.log.take();
    // Two skill shortcuts of 1020, a list for each; the macro with the same id stays.
    assert_eq!(decoded::<ShortcutInit>(&out, USER, "SHORT_CUT_INIT").len(), 2);
    assert_eq!(
        s.world.player(ObjectId(USER)).unwrap().data.shortcuts,
        [sc(1, 0, 2, 3100), sc(2, 0, 4, 1020)]
    );

    // 3100 is in the range 3080..=3259: the shortcut stays.
    s.cmd(GM, "remove_skill 3100");
    let out = s.log.take();
    assert!(decoded::<ShortcutInit>(&out, USER, "SHORT_CUT_INIT").is_empty());
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().data.shortcuts.len(), 2);
}

// ------------------------------------------------------------------------------ //skill_list

/// The races of the classes that have a skill tree (human: classes 0 and 1, elf: class 18).
#[test]
fn skill_list_lists_the_races_with_a_skill_tree() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    s.cmd(GM, "skill_list");
    let race = |name: &str, text: &str| {
        format!("<tr><td><a action=\"bypass admin_skill_race {name}\">{text}</a></td></tr>")
    };
    let expected = format!(
        "<html><body>ADD<table>{}{}</table></body></html>",
        race("HUMAN", "Human"),
        race("ELF", "Elf")
    );
    assert_eq!(html_to(&s.log.take(), GM), vec![expected.clone()]);

    // `skill_race` and `skill_class` without a name (or with too many words for the race) are the
    // same list.
    s.cmd(GM, "skill_race");
    s.cmd(GM, "skill_race HUMAN ELF");
    s.cmd(GM, "skill_class");
    s.cmd(GM, "skill_add_list");
    assert_eq!(html_to(&s.log.take(), GM), vec![expected; 4]);
}

#[test]
fn skill_race_lists_the_classes_with_their_level_colour() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    let class = |name: &str, color: &str, text: &str| {
        format!(
            "<tr><td><a action=\"bypass admin_skill_class {name}\"><font color=\"{color}\">{text}</font></a></td></tr>"
        )
    };
    // Class level 0 is the grey default, level 1 is 999999.
    s.cmd(GM, "skill_race HUMAN");
    assert_eq!(
        html_to(&s.log.take(), GM),
        [format!(
            "<html><title>Human Classes</title><body><table>{}{}</table></body></html>",
            class("FIGHTER", "777777", "Human Fighter"),
            class("WARRIOR", "999999", "Warrior")
        )]
    );
    s.cmd(GM, "skill_race ELF");
    assert_eq!(
        html_to(&s.log.take(), GM),
        [format!(
            "<html><title>Elf Classes</title><body><table>{}</table></body></html>",
            class("ELVEN_FIGHTER", "777777", "Elven Fighter")
        )]
    );
    // A race of the enum with no classes: an empty list. `DARK_ELF` shows as `Dark Elf`.
    s.cmd(GM, "skill_race DARK_ELF");
    assert_eq!(
        html_to(&s.log.take(), GM),
        ["<html><title>Dark Elf Classes</title><body><table></table></body></html>"]
    );
    s.cmd(GM, "skill_race UNDEAD");
    assert_eq!(
        html_to(&s.log.take(), GM),
        ["<html><title>Undead Classes</title><body><table></table></body></html>"]
    );
    // Not a constant of the enum: `Enum.valueOf` throws.
    s.cmd(GM, "skill_race Elf");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Exception during execution of  'admin_skill_race Elf': java.lang.IllegalArgumentException: No enum constant org.l2jmobius.gameserver.entity.actor.enums.creature.Race.Elf"
        ]
    );
}

fn class_skill_row(icon: &str, class: &str, id: i32, name: &str) -> String {
    format!(
        "<table width=\"295\" bgcolor=\"000000\"><tr><td height=40 width=40><img src=\"{icon}\" width=32 height=32></td><td width=190><font color=\"B09878\"><a action=\"bypass admin_skill_add_list {class} {id}\">{name} ({id})</a></font></td></tr></table><img src=\"L2UI.SquareGray\" width=295 height=1>"
    )
}

fn level_row(icon: &str, id: i32, level: i32, name: &str) -> String {
    format!(
        "<table width=\"295\" bgcolor=\"000000\"><tr><td height=40 width=40><img src=\"{icon}\" width=32 height=32></td><td width=190><font color=\"B09878\"><a action=\"bypass admin_add_skill {id} {level}\">{name} ({level})</a></font></td></tr></table><img src=\"L2UI.SquareGray\" width=295 height=1>"
    )
}

#[test]
fn skill_class_and_add_list_show_the_tree_of_a_class() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    // The tree of class 0: `Power Strike` (3 levels) and `Lucky`, each skill once, by skill id.
    s.cmd(GM, "skill_class FIGHTER");
    assert_eq!(
        html_to(&s.log.take(), GM),
        [format!(
            "<html><title>Human Fighter Skills</title><body>{}{}<br><br></body></html>",
            class_skill_row("icon.skill0003", "FIGHTER", 3, "Power Strike"),
            class_skill_row("icon.skill0000", "FIGHTER", 1020, "Lucky"),
        )]
    );
    // The tree of class 1 adds `Reflect`; a page that does not exist is the last one.
    s.cmd(GM, "skill_class WARRIOR 7");
    let out = html_to(&s.log.take(), GM);
    assert_eq!(out.len(), 1);
    assert!(out[0].contains("<title>Warrior Skills</title>"));
    assert!(out[0].contains(&class_skill_row("icon.skill0000", "WARRIOR", 3100, "Reflect")));

    // The levels of one skill, lowest first.
    s.cmd(GM, "skill_add_list FIGHTER 3");
    assert_eq!(
        html_to(&s.log.take(), GM),
        [format!(
            "<html><title>Add Skill (3)</title><body>FIGHTER|{}{}{}<br><br></body></html>",
            level_row("icon.skill0003", 3, 1, "Power Strike"),
            level_row("icon.skill0003", 3, 2, "Power Strike"),
            level_row("icon.skill0003", 3, 3, "Power Strike"),
        )]
    );
    // No skill id: skill 1 (not in the tree), an empty list.
    s.cmd(GM, "skill_add_list FIGHTER");
    assert_eq!(
        html_to(&s.log.take(), GM),
        ["<html><title>Add Skill (1)</title><body>FIGHTER|<br><br></body></html>"]
    );

    // Names and numbers that Java throws on.
    s.cmd(GM, "skill_class NOBODY");
    s.cmd(GM, "skill_class FIGHTER x");
    s.cmd(GM, "skill_add_list FIGHTER 3 y");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Exception during execution of  'admin_skill_class NOBODY': java.lang.IllegalArgumentException: No enum constant org.l2jmobius.gameserver.entity.actor.enums.player.PlayerClass.NOBODY",
            "Exception during execution of  'admin_skill_class FIGHTER x': java.lang.NumberFormatException: For input string: \"x\"",
            "Exception during execution of  'admin_skill_add_list FIGHTER 3 y': java.lang.NumberFormatException: For input string: \"y\"",
        ]
    );
}

#[test]
fn skill_index_and_show_skills_open_their_pages() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    // `skill_index <name>` is `data/html/admin/skills/<name>.htm`: missing in the test datapack,
    // so Java's "My text is missing" page. A command too short for `substring(18)` does nothing.
    s.cmd(GM, "skill_index fighter");
    assert_eq!(
        html_to(&s.log.take(), GM),
        ["<html><body>My text is missing:<br>data/html/admin/skills/fighter.htm</body></html>"]
    );
    s.cmd(GM, "skill_index");
    assert!(s.log.take().is_empty());

    // `show_skills` needs a player target; with the exact word only.
    s.cmd(GM, "show_skills");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.cmd(GM, "show_skills");
    assert_eq!(
        html_to(&s.log.take(), GM),
        ["<html><body>EDIT P1 Lv 40 Human Fighter</body></html>"]
    );
    s.cmd(GM, "show_skills now");
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

    // An online name, or a target: the page of the buffs, empty here.
    s.cmd(GM, "getbuffs p1");
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.cmd(GM, "getbuffs");
    let out = s.log.take();
    assert!(texts_to(&out, GM).is_empty());
    assert_eq!(
        html_to(&out, GM),
        vec![buffs_page("P1", 1, "", "", "Show Passives", "_ps", 0); 2]
    );
}

/// `getbuff_skills.htm` of the test datapack, filled in.
fn buffs_page(
    name: &str,
    object_id: i32,
    buffs: &str,
    pages: &str,
    text: &str,
    passives: &str,
    size: usize,
) -> String {
    format!(
        "<html><body>Effects of {name} ({object_id})|{buffs}|{pages}|<button value=\"{text}\" action=\"bypass admin_getbuffs{passives} {name}\">|Total skill count: {size}</body></html>"
    )
}

/// One row of the buff page: a skill the creature is affected by, with its remaining seconds.
fn buff_row(icon: &str, name: &str, object_id: i32, id: i32, level: i32, time: &str) -> String {
    format!(
        "<table width=\"295\" bgcolor=\"000000\"><tr><td height=40 width=40><img src=\"{icon}\" width=32 height=32></td><td width=650><font color=\"B09878\"><a action=\"bypass admin_viewskilleffects {object_id} {id} {level}\">{name} Lv. {level} ({id})</a></font></td><td fixwidth=200 height=32 valign=\"center\" align=\"center\">{time}</td><td><table><tr><td height=5></td></tr><tr><td fixwidth=360><button value=\"X\" action=\"bypass admin_stopbuff {object_id} {id}\" width=30 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table></td></tr></table><img src=\"L2UI.SquareGray\" width=295 height=1>"
    )
}

#[test]
fn getbuffs_shows_the_buffs_of_a_player_or_the_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world
        .apply_effects(ObjectId(USER), ObjectId(USER), SkillKey::new(9001, 1));
    s.log.take();

    let row = buff_row("icon.skill9001", "Might", 1, 9001, 1, "60s");
    let expected = buffs_page("P1", 1, &row, "", "Show Passives", "_ps", 1);
    s.cmd(GM, "getbuffs p1");
    assert_eq!(html_to(&s.log.take(), GM), vec![expected.clone()]);

    // The target, a character: the same page, and a page number that does not exist is the page.
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.cmd(GM, "getbuffs");
    s.cmd(GM, "getbuffs P1 5");
    assert_eq!(html_to(&s.log.take(), GM), [expected.clone(), expected]);

    // `getbuffs_ps` lists the passive skills of the creature instead, with no time.
    s.cmd(GM, "getbuffs_ps");
    let out = html_to(&s.log.take(), GM);
    assert_eq!(out.len(), 1);
    assert!(out[0].contains("<button value=\"Hide Passives\" action=\"bypass admin_getbuffs P1\">"));
}

#[test]
fn getbuffs_target_can_be_a_monster() {
    let mut s = setup();
    s.enter(GM, 0);
    let rat = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    s.world.set_target(ObjectId(GM), Some(rat));
    s.log.take();

    // A monster without effects: an empty page with its name and object id.
    s.cmd(GM, "getbuffs");
    assert_eq!(
        html_to(&s.log.take(), GM),
        [buffs_page("Rat", rat.0, "", "", "Show Passives", "_ps", 0)]
    );
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

    // Java: `stopSkillEffects(REMOVED, 9001)` on the object 1, its message, then the buff page of
    // the target (now without the skill).
    s.cmd(GM, "stopbuff 1 9001");
    assert!(!s.world.is_affected_by_skill(ObjectId(USER), 9001));
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        ["Removed skill ID: 9001 effects from P1 (1)."]
    );
    assert_eq!(
        html_to(&out, GM),
        [buffs_page("P1", 1, "", "", "Show Passives", "_ps", 0)]
    );

    // Not affected any more: no message, but the page is shown again. An object that is not
    // there, or a skill id of 0: nothing at all.
    s.cmd(GM, "stopbuff 1 9001");
    let out = s.log.take();
    assert!(texts_to(&out, GM).is_empty());
    assert_eq!(html_to(&out, GM).len(), 1);
    s.cmd(GM, "stopbuff 999999 9001");
    s.cmd(GM, "stopbuff 1 0");
    assert!(s.log.take().is_empty());
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

    // The part that runs after the confirm dialog said yes.
    s.world.admin_command(ObjectId(GM), "admin_stopallbuffs 1", false);
    assert!(!s.world.is_affected_by_skill(ObjectId(USER), 9001));
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Removed all effects from P1 (1)"]);
    // Then the buff page of the target.
    assert_eq!(
        html_to(&out, GM),
        [buffs_page("P1", 1, "", "", "Show Passives", "_ps", 0)]
    );

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

    // `AdminCommands.xml` has `confirmDlg="true"` for stopallbuffs: the GM is asked first.
    s.cmd(GM, "stopallbuffs 1");
    assert!(s.world.is_affected_by_skill(ObjectId(USER), 9001));
    let out = s.log.take();
    let dialogs = decoded::<ConfirmDlg>(&out, GM, "CONFIRM_DLG");
    assert_eq!(dialogs.len(), 1);
    assert_eq!(dialogs[0].message.id, SystemMessageId::S13);
    assert_eq!(
        dialogs[0].message.params,
        [SystemMessageParam::Text(
            "Are you sure you want execute command 'stopallbuffs' ?".to_owned()
        )]
    );

    // A no changes nothing.
    s.answer(GM, SystemMessageId::S13.id(), 0);
    assert!(s.world.is_affected_by_skill(ObjectId(USER), 9001));
    s.log.take();

    // A yes runs the command: the effects go, the GM is told and gets the buff page.
    s.cmd(GM, "stopallbuffs 1");
    s.log.take();
    s.answer(GM, SystemMessageId::S13.id(), 1);
    assert!(!s.world.is_affected_by_skill(ObjectId(USER), 9001));
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Removed all effects from P1 (1)"]);
    assert_eq!(html_to(&out, GM).len(), 1);
    assert!(decoded::<ConfirmDlg>(&out, GM, "CONFIRM_DLG").is_empty());
}

// ------------------------------------------------------- //viewskilleffects, //viewblockedeffects

#[test]
fn viewskilleffects_shows_the_effects_of_one_skill() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world
        .apply_effects(ObjectId(USER), ObjectId(USER), SkillKey::new(9001, 1));
    s.log.take();

    s.cmd(GM, "viewskilleffects 1 9001 1");
    let row = "<table width=\"295\" bgcolor=\"000000\"><tr><td width=750>Might Lv. 1 (Buff)</td><td width=180><center>60s</center></td><td width=200><button value=\"X\" action=\"bypass admin_stopbuff 1 9001\" width=30 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><img src=\"L2UI.SquareGray\" width=295 height=1>";
    assert_eq!(
        html_to(&s.log.take(), GM),
        [format!(
            "<html><body>Effects of P1 (1)|{row}||Total effect count: 1</body></html>"
        )]
    );

    // Another level of the skill has nothing: an empty list, a count of 0 (and the passives list
    // is searched with `_ps`, which this buff is not in).
    s.cmd(GM, "viewskilleffects_ps 1 9001 1 3");
    assert_eq!(
        html_to(&s.log.take(), GM),
        ["<html><body>Effects of P1 (1)|||Total effect count: 0</body></html>"]
    );
}

#[test]
fn viewskilleffects_failures_give_the_java_messages() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    for (command, message) in [
        ("viewskilleffects 1 9001", "null"),
        ("viewskilleffects x 9001 1", "For input string: \"x\""),
        ("viewskilleffects 1 9001 1 z", "For input string: \"z\""),
    ] {
        s.cmd(GM, command);
        assert_eq!(
            texts_to(&s.log.take(), GM),
            [
                format!("Failed viewing skill effects: {message}"),
                "Usage: //viewskilleffects <objectId skillId skillLevel>".to_owned()
            ],
            "{command}"
        );
    }
    // An object that is not in the world.
    s.cmd(GM, "viewskilleffects 999999 1 1");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Target with object id 999999 not found."]
    );
}

#[test]
fn viewblockedeffects_lists_the_blocked_abnormal_types() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    let page = |back: &str, slots: &str| {
        format!(
            "<html><table width=\"100%\"><tr><td width=45><button value=\"Main\" action=\"bypass admin_admin\" width=45 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td><td width=180><center><font color=\"LEVEL\">Blocked effects of P1</font></td><td width=45><button value=\"Back\" action=\"bypass admin_getbuffs{back}\" width=45 height=21 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></td></tr></table><br>{slots}</html>"
        )
    };
    s.cmd(GM, "viewblockedeffects 1");
    assert_eq!(html_to(&s.log.take(), GM), [page(" P1", "")]);

    s.world
        .add_blocked_abnormal_types(ObjectId(USER), [AbnormalType::PaUp, AbnormalType::Stun]);
    s.cmd(GM, "viewblockedeffects 1");
    let out = html_to(&s.log.take(), GM);
    assert_eq!(out.len(), 1);
    assert!(out[0].contains("<br>Blocked buff slots: <br>"));
    assert!(out[0].contains("<br>PA_UP"));
    assert!(out[0].contains("<br>STUN"));

    s.cmd(GM, "viewblockedeffects");
    s.cmd(GM, "viewblockedeffects 999999");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Failed viewing blocked effects: null",
            "Usage: //viewblockedeffects <objectId>",
            "Target with object id 999999 not found."
        ]
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

/// The `%word%` placeholders of a page.
fn placeholders(page: &str) -> std::collections::BTreeSet<String> {
    let mut found = std::collections::BTreeSet::new();
    let mut rest = page;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        match after.find('%') {
            Some(end) if after[..end].chars().all(|c| c.is_ascii_alphanumeric()) && end > 0 => {
                found.insert(format!("%{}%", &after[..end]));
                rest = &after[end + 1..];
            }
            _ => rest = after,
        }
    }
    found
}

/// The pages of the test datapack fill the same placeholders as the shipped ones, so the tests
/// above stand for the real pages. Skips without the reference datapack.
#[test]
fn the_test_pages_have_the_placeholders_of_the_shipped_pages() {
    let dir = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/html/admin"
    ));
    if !dir.is_dir() {
        eprintln!("skipped: the reference datapack is not here");
        return;
    }
    for (name, body) in ADMIN_PAGES {
        let shipped = std::fs::read_to_string(dir.join(name)).unwrap();
        assert_eq!(placeholders(body), placeholders(&shipped), "{name}");
    }
    // `expsp.htm` (admin_combat.rs).
    let shipped = std::fs::read_to_string(dir.join("expsp.htm")).unwrap();
    let wanted = ["%name%", "%level%", "%class%", "%xp%", "%sp%"];
    assert_eq!(
        placeholders(&shipped),
        wanted.iter().map(|w| (*w).to_owned()).collect()
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
