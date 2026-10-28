#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Subclasses (S-SUB-01): the `Subclass` bypass of a village master, adding, switching and
//! replacing a subclass with the writes Java makes, the class index rows that follow a switch
//! (skills, buffs, hennas, shortcuts) and the level limits of a subclass. Expected values come
//! from `VillageMaster.onBypassFeedback`, `Player.addSubClass`, `modifySubClass`,
//! `setActiveClass`, `SubClassHolder` and `PlayerStat`.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, ClassList, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates,
    KarmaLoss, MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates,
    SkillTrees, SpawnData, SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- harness -----------------------------------------------------------------------------------

#[derive(Debug, Clone)]
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

struct Recorder(Log);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap_or(5),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: -1,
            weapon: t.is_weapon(),
        })
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        true
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn names(out: &[Out]) -> Vec<String> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(b) => Some(name_of(b)),
            Out::Persist(_) => None,
        })
        .collect()
}

fn decoded<P: Decode>(out: &[Out], name: &str) -> Vec<P> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(b) if name_of(b) == name => Some(b.as_slice()),
            _ => None,
        })
        .map(|raw| {
            let skip = if raw[0] == 0xFE { 3 } else { 1 };
            P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
        })
        .collect()
}

fn message_ids(out: &[Out]) -> Vec<SystemMessageId> {
    decoded::<SystemMessage>(out, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|m| m.id)
        .collect()
}

fn pages(out: &[Out]) -> Vec<String> {
    decoded::<NpcHtmlMessage>(out, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|m| m.html)
        .collect()
}

fn persisted(out: &[Out]) -> Vec<PersistCommand> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(c) => Some(c.clone()),
            Out::Packet(_) => None,
        })
        .collect()
}

// ---- data --------------------------------------------------------------------------------------

/// A player template of `class_id` (the same values for every class).
fn template(class_id: i32) -> String {
    format!(
        r#"<list><classId>{class_id}</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#
    )
}

/// Gladiator (2) is the base class, Bishop (16) and Prophet (17) the subclasses of the tests.
const GLADIATOR: i32 = 2;
const BISHOP: i32 = 16;
const PROPHET: i32 = 17;
/// The village master of the tests (`VillageMaster`: every race and class).
const VM: i32 = 30_026;

const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="30026" type="VillageMaster" level="70" name="Bitz" usingServerSideName="true">
    <status attackable="false" />
  </npc>
</list>"#;

const CLASS_LIST: &str = r#"<list>
  <class classId="2" name="Gladiator" parentClassId="1"/>
  <class classId="16" name="Bishop" parentClassId="15"/>
  <class classId="17" name="Prophet" parentClassId="15"/>
</list>"#;

/// The Bishop tree: 1011 up to level 2 by level 40 (level 3 only at 44), 1012 at 40. The Prophet
/// tree: 1013. The Gladiator knows 1001.
const TREES: &str = r#"<list>
<skillTree type="classSkillTree" classId="16">
<skill skillName="A" skillId="1011" skillLevel="1" getLevel="20" learnedByNpc="true" />
<skill skillName="A" skillId="1011" skillLevel="2" getLevel="40" learnedByNpc="true" />
<skill skillName="A" skillId="1011" skillLevel="3" getLevel="44" learnedByNpc="true" />
<skill skillName="B" skillId="1012" skillLevel="1" getLevel="40" learnedByNpc="true" />
</skillTree>
<skillTree type="classSkillTree" classId="17">
<skill skillName="C" skillId="1013" skillLevel="1" getLevel="20" learnedByNpc="true" />
</skillTree>
</list>"#;

fn passive(id: i32, levels: i32) -> String {
    format!(
        r#"<skill id="{id}" levels="{levels}" name="S{id}"><operateType>P</operateType><targetType>SELF</targetType>
        <effects><effect name="Buff"><add stat="mDef" val="1"/></effect></effects></skill>"#
    )
}

fn skills_xml() -> String {
    format!(
        r#"<list>{}{}{}{}
<skill id="9001" levels="1" name="Might"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>1500</hitTime><isMagic>1</isMagic><mpConsume>5</mpConsume><reuseDelay>60000</reuseDelay>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9002" levels="1" name="Empower"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>1000</hitTime><isMagic>1</isMagic><mpConsume>1</mpConsume>
<abnormalTime>60</abnormalTime><abnormalType>MA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="mAtk" val="1.1"/></effect></effects></skill>
</list>"#,
        passive(1001, 1),
        passive(1011, 3),
        passive(1012, 1),
        passive(1013, 1),
    )
}

/// `experience.xml` with `(level - 1)^2 * 1000` exp per level, up to 85.
fn exp_for(level: i64) -> i64 {
    (level - 1) * (level - 1) * 1000
}

fn experience() -> Arc<ExperienceTable> {
    let rows: String = (1..=86)
        .map(|level| format!(r#"<experience level="{level}" tolevel="{}" />"#, exp_for(level)))
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
        ))
        .unwrap(),
    )
}

const ME: ObjectId = ObjectId(0x2000_0001);
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

/// The pages: every one names itself and links the `Subclass` commands.
const LINKS: &str = "<a action=\"bypass -h npc_%objectId%_Subclass $cmd\">go</a>";

struct Scene {
    world: World,
    log: Log,
    vm: Option<ObjectId>,
    _root: tempfile::TempDir,
}

#[derive(Clone)]
struct Me {
    level: i32,
    noble: bool,
    class_id: i32,
    class_index: i32,
    subs: Vec<SubClassHolder>,
    skills: Vec<SkillData>,
}

impl Default for Me {
    fn default() -> Self {
        Self {
            level: 76,
            noble: true,
            class_id: GLADIATOR,
            class_index: 0,
            subs: Vec::new(),
            skills: vec![SkillData {
                skill_id: 1001,
                skill_level: 1,
            }],
        }
    }
}

impl Scene {
    fn new(me: Me, settings: SubclassSettings) -> Self {
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
        write(
            &format!("data/html/villagemaster/{VM}.htm"),
            &format!("<html><body>Master {LINKS}</body></html>"),
        );
        for page in [
            "SubClass",
            "SubClass_Add",
            "SubClass_AddOk",
            "SubClass_Change",
            "SubClass_ChangeNo",
            "SubClass_Current",
            "SubClass_Fail",
            "SubClass_Modify",
            "SubClass_ModifyChoice1",
            "SubClass_ModifyOk",
            "SubClass_ModifyEmpty",
        ] {
            write(
                &format!("data/html/villagemaster/{page}.htm"),
                &format!("<html><body>{page} %objectId% [%list%] [%name%] {LINKS}</body></html>"),
            );
        }
        write(
            "data/html/villagemaster/SubClass_Modify.htm",
            "<html><body>SubClass_Modify \
             <a action=\"bypass npc_%objectId%_Subclass 6 1\">%sub1%</a><br>\
             <a action=\"bypass npc_%objectId%_Subclass 6 2\">%sub2%</a><br>\
             <a action=\"bypass npc_%objectId%_Subclass 6 3\">%sub3%</a><br></body></html>",
        );
        for class_id in [GLADIATOR, BISHOP, PROPHET] {
            write(
                &format!("data/stats/players/templates/{class_id}.xml"),
                &template(class_id),
            );
        }

        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(20_261_028), Box::new(Recorder(log.clone())));
        world.set_settings(WorldSettings {
            subclass: settings,
            ..WorldSettings::default()
        });
        world.set_static_data(Box::new(Data(StatData {
            items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
            player_templates: Arc::new(PlayerTemplates::load(root.path(), 85)),
            armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::from_xml(
                &skills_xml(),
                &l2_data::skill::EnchantSkillGroups::default(),
                &SkillLoadOptions::default(),
            )),
        })));
        world.set_exp_data(
            experience(),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml(TREES).unwrap()),
        );
        world.set_class_list(Arc::new(ClassList::from_xml(CLASS_LIST).unwrap()));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );

        let mut profile = PlayerProfile {
            noble: me.noble,
            class_index: me.class_index,
            ..PlayerProfile::default()
        };
        for sub in &me.subs {
            profile.subclasses.subs.insert(sub.class_index, *sub);
        }
        if me.class_index > 0 {
            profile.subclasses.base = Some(ClassStats {
                level: 76,
                exp: exp_for(76),
                sp: 0,
            });
        }
        let data = PlayerData {
            object_id: ME,
            account: "acc".into(),
            name: "Hero".into(),
            title: String::new(),
            class_id: me.class_id,
            base_class: GLADIATOR,
            race: 0,
            sex: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            level: me.level,
            exp: exp_for(i64::from(me.level)),
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
            items: vec![],
            skills: me.skills,
            shortcuts: vec![],
            profile,
        };
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(data),
            sink: Box::new(Sink(log.clone())),
            ip: None,
        });
        for _ in 0..20 {
            world.tick();
        }
        log.take();
        Self {
            world,
            log,
            vm: None,
            _root: root,
        }
    }

    fn input(&mut self, input: ClientInput) -> Vec<Out> {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input,
        });
        self.world.tick();
        self.log.take()
    }

    /// Talks to the village master (spawned on first use), then sends `Subclass ...`. Waits long
    /// enough first for the flood protectors.
    fn subclass(&mut self, args: &str) -> Vec<Out> {
        for _ in 0..60 {
            self.world.tick();
        }
        let npc = match self.vm {
            Some(npc) => npc,
            None => {
                let npc = self.world.spawn_npc_at(VM, X + 50, Y, Z, 0).unwrap();
                self.vm = Some(npc);
                npc
            }
        };
        for _ in 0..2 {
            self.input(ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            });
        }
        self.log.take();
        self.input(ClientInput::Bypass {
            command: format!("npc_{}_Subclass {args}", npc.0),
        })
    }

    /// The persistence task's answer to the class index request.
    fn answer(&mut self, class_index: i32, load: ClassIndexLoad) -> Vec<Out> {
        self.world.handle(WorldCommand::ClassIndexLoaded {
            owner: ME,
            class_index,
            load: Box::new(load),
        });
        self.world.tick();
        self.log.take()
    }

    fn me(&self) -> &Player {
        self.world.player(ME).unwrap()
    }

    fn skill_ids(&self) -> Vec<i32> {
        let mut ids: Vec<i32> = self.me().core.skills.iter().map(|(s, _)| *s).collect();
        ids.sort_unstable();
        ids
    }

    fn has_effect(&self, skill_id: i32) -> bool {
        self.world
            .effect_list(ME)
            .is_some_and(|l| l.effects().any(|i| i.skill().id == skill_id))
    }
}

fn skill(skill_id: i32, skill_level: i32) -> SkillData {
    SkillData {
        skill_id,
        skill_level,
    }
}

fn holder(class_id: i32, class_index: i32, level: i32) -> SubClassHolder {
    SubClassHolder {
        class_id,
        exp: exp_for(i64::from(level)),
        sp: 0,
        level,
        class_index,
    }
}

// ---- the menu ----------------------------------------------------------------------------------

/// `Subclass 1`: the classes the master offers, as `Subclass 4 <id>` links with the class name;
/// `%objectId%` is the master.
#[test]
fn the_add_page_lists_the_classes_the_master_offers() {
    let mut s = Scene::new(Me::default(), SubclassSettings::default());
    let out = s.subclass("1");
    let page = &pages(&out)[0];
    let npc = s.vm.unwrap().0;
    assert!(page.starts_with(&format!("<html><body>SubClass_Add {npc} [")));
    assert!(page.contains(&format!(
        "<a action=\"bypass npc_{npc}_Subclass 4 16\" msg=\"1268;Bishop\">Bishop</a><br>"
    )));
    // The base class's own 2nd class is not offered.
    assert!(!page.contains("Subclass 4 2\""));
}

/// `Subclass 2` without a subclass, `Subclass 3` without one.
#[test]
fn the_change_and_modify_pages_without_a_subclass() {
    let mut s = Scene::new(Me::default(), SubclassSettings::default());
    assert!(pages(&s.subclass("2"))[0].contains("SubClass_ChangeNo"));
    assert!(pages(&s.subclass("3"))[0].contains("SubClass_ModifyEmpty"));
}

/// `Subclass 3` with a subclass in slot 1: the retail page, the empty slots' links removed.
#[test]
fn the_modify_page_names_the_subclasses() {
    let me = Me {
        subs: vec![holder(BISHOP, 1, 76)],
        ..Me::default()
    };
    let mut s = Scene::new(me, SubclassSettings::default());
    let page = &pages(&s.subclass("3"))[0];
    let npc = s.vm.unwrap().0;
    assert!(page.contains(&format!(
        "<a action=\"bypass npc_{npc}_Subclass 6 1\">Bishop</a><br>"
    )));
    assert!(!page.contains("Subclass 6 2"));
    assert!(!page.contains("%sub3%"));
}

// ---- adding ------------------------------------------------------------------------------------

/// `Subclass 4 16`: the row and the skills of the tree up to level 40 under index 1, the store,
/// then the request for the rows of index 1; nothing is shown until they come back.
#[test]
fn adding_writes_the_row_and_asks_for_the_new_index() {
    let mut s = Scene::new(Me::default(), SubclassSettings::default());
    let out = s.subclass("4 16");
    let writes = persisted(&out);
    assert_eq!(
        writes[0],
        PersistCommand::Subclass(SubclassWrite::Add {
            char_id: ME.0,
            holder: holder(BISHOP, 1, 40),
            skills: vec![skill(1011, 2), skill(1012, 1)],
        })
    );
    let PersistCommand::StorePlayer(stored) = &writes[1] else {
        panic!("expected the store, got {:?}", writes[1]);
    };
    // The store is of the base class (Java stores before it changes `_classIndex`).
    assert_eq!(
        (stored.class_id, stored.profile.class_index, stored.level),
        (GLADIATOR, 0, 76)
    );
    assert_eq!(
        writes[2],
        PersistCommand::ClassIndexRequest {
            owner: ME,
            class_index: 1
        }
    );
    assert!(pages(&out).is_empty());
    assert!(s.world.is_changing_class(ME));
    assert_eq!(s.world.total_sub_classes(ME), 1);
}

/// The answer finishes the switch: the class, index, level, the skills, buffs, hennas and
/// shortcuts of the new index, Java's packets, then the caller's message and page.
#[test]
fn the_switch_follows_the_rows_of_the_new_index() {
    let mut s = Scene::new(Me::default(), SubclassSettings::default());
    // A buff of the base class that does not stay on a subclass change.
    s.world
        .apply_effects_timed(ME, ME, SkillKey::new(9002, 1), false, 30);
    assert!(s.has_effect(9002));
    s.subclass("4 16");
    // A logout or restart during the change is refused (`canLogout`).
    let out = s.input(ClientInput::Logout);
    assert!(names(&out).contains(&"ACTION_FAIL".to_owned()));
    assert!(s.world.player(ME).is_some());
    let out = s.input(ClientInput::Restart);
    assert!(names(&out).contains(&"RESTART_RESPONSE".to_owned()));
    assert!(s.world.player(ME).is_some());

    let out = s.answer(
        1,
        ClassIndexLoad {
            skills: vec![skill(1011, 2), skill(1012, 1)],
            effects: vec![SavedEffect {
                skill_id: 9001,
                skill_level: 1,
                remaining_time: 30,
                reuse_delay: 0,
                systime: 0,
                restore_type: 0,
            }],
            hennas: [0, 0, 0],
            shortcuts: vec![ShortcutData {
                slot: 1,
                page: 0,
                kind: 2,
                shortcut_id: 1011,
                level: 2,
            }],
            ..ClassIndexLoad::default()
        },
    );
    let me = s.me();
    assert_eq!((me.data.class_id, me.data.profile.class_index), (BISHOP, 1));
    assert_eq!((me.core.level, me.data.exp), (40, exp_for(40)));
    assert_eq!(me.data.skills, [skill(1011, 2), skill(1012, 1)]);
    assert_eq!(me.data.shortcuts.len(), 1);
    // The base class values wait in the profile.
    assert_eq!(
        me.data.profile.subclasses.base,
        Some(ClassStats {
            level: 76,
            exp: exp_for(76),
            sp: 0
        })
    );
    assert_eq!(s.skill_ids(), [1011, 1012]);
    assert!(s.has_effect(9001));
    assert!(!s.has_effect(9002));
    assert!(!s.world.is_changing_class(ME));
    // Java's order: HennaInfo, the status, ShortcutInit, SocialAction, SkillCoolTime,
    // ExStorageMaxCount, then the caller's message and page.
    let order = names(&out);
    let at = |name: &str| order.iter().position(|n| n == name).unwrap();
    assert!(at("HENNA_INFO") < at("SHORT_CUT_INIT"));
    assert!(at("SHORT_CUT_INIT") < at("SOCIAL_ACTION"));
    assert!(at("SOCIAL_ACTION") < at("SKILL_COOL_TIME"));
    assert!(at("SKILL_COOL_TIME") < at("EX_STORAGE_MAX_COUNT"));
    assert!(at("EX_STORAGE_MAX_COUNT") < at("NPC_HTML_MESSAGE"));
    assert!(message_ids(&out).contains(&SystemMessageId::TheNewSubclassHasBeenAdded));
    assert!(pages(&out)[0].contains("SubClass_AddOk"));

    // The rows a store writes: the live subclass values, the base values for `characters`.
    let snapshot = s.me().snapshot();
    assert_eq!(stored_subclasses(&snapshot), [holder(BISHOP, 1, 40)]);
    assert_eq!(base_class_stats(&snapshot).map(|b| b.level), Some(76));
}

/// `Subclass 5 0` goes back to the base class: the subclass keeps what it gained, the base class
/// gets its values back.
#[test]
fn switching_back_to_the_base_class() {
    let me = Me {
        class_id: BISHOP,
        class_index: 1,
        level: 40,
        subs: vec![holder(BISHOP, 1, 40)],
        skills: vec![skill(1011, 2)],
        ..Me::default()
    };
    let mut s = Scene::new(me, SubclassSettings::default());
    assert!(s.world.is_subclass_active(ME));
    let out = s.subclass("5 0");
    let writes = persisted(&out);
    let PersistCommand::StorePlayer(stored) = &writes[0] else {
        panic!("expected the store, got {:?}", writes[0]);
    };
    assert_eq!(stored.profile.class_index, 1);
    assert_eq!(
        writes[1],
        PersistCommand::ClassIndexRequest {
            owner: ME,
            class_index: 0
        }
    );
    // The subclass gains a level before the answer comes back (it is still active).
    s.world
        .add_exp_and_sp(ME, (exp_for(41) - exp_for(40)) as f64, 0.0, false);
    assert_eq!(s.me().core.level, 41);
    s.log.take();
    let out = s.answer(
        0,
        ClassIndexLoad {
            skills: vec![skill(1001, 1)],
            ..ClassIndexLoad::default()
        },
    );
    let me = s.me();
    assert_eq!(
        (me.data.class_id, me.data.profile.class_index, me.core.level),
        (GLADIATOR, 0, 76)
    );
    assert_eq!(me.data.profile.subclasses.base, None);
    assert_eq!(me.data.profile.subclasses.subs[&1].level, 41);
    assert_eq!(s.skill_ids(), [1001]);
    assert_eq!(
        message_ids(&out).last(),
        Some(&SystemMessageId::YouHaveSuccessfullySwitchedToYourSubclass)
    );
}

/// `Subclass 5 <current>` shows the "current" page and changes nothing.
#[test]
fn switching_to_the_current_class_is_refused() {
    let mut s = Scene::new(
        Me {
            subs: vec![holder(BISHOP, 1, 76)],
            ..Me::default()
        },
        SubclassSettings::default(),
    );
    let out = s.subclass("5 0");
    assert!(pages(&out)[0].contains("SubClass_Current"));
    assert!(persisted(&out).is_empty());
}

// ---- the rules ---------------------------------------------------------------------------------

/// Below level 75, with a subclass below 75, or without the quests (not noble, quests required),
/// the fail page comes and nothing is written.
#[test]
fn the_add_rules() {
    let low = Me {
        level: 74,
        ..Me::default()
    };
    let mut s = Scene::new(low, SubclassSettings::default());
    let out = s.subclass("4 16");
    assert!(pages(&out)[0].contains("SubClass_Fail"));
    assert!(persisted(&out).is_empty());

    let low_sub = Me {
        subs: vec![holder(PROPHET, 1, 74)],
        ..Me::default()
    };
    let mut s = Scene::new(low_sub, SubclassSettings::default());
    assert!(persisted(&s.subclass("4 16")).is_empty());

    let commoner = Me {
        noble: false,
        ..Me::default()
    };
    let mut s = Scene::new(commoner.clone(), SubclassSettings::default());
    assert!(pages(&s.subclass("4 16"))[0].contains("SubClass_Fail"));
    // `AltSubClassWithoutQuests` lets him.
    let free = SubclassSettings {
        alt_game_subclass_without_quests: true,
        ..SubclassSettings::default()
    };
    let mut s = Scene::new(commoner, free);
    assert!(matches!(
        persisted(&s.subclass("4 16"))[0],
        PersistCommand::Subclass(SubclassWrite::Add { .. })
    ));
}

/// `MaxSubclass` subclasses are the limit: the add page is the fail page.
#[test]
fn all_subclasses_taken() {
    let one = SubclassSettings {
        max_subclass: 1,
        ..SubclassSettings::default()
    };
    let mut s = Scene::new(
        Me {
            subs: vec![holder(BISHOP, 1, 76)],
            ..Me::default()
        },
        one,
    );
    assert!(pages(&s.subclass("1"))[0].contains("SubClass_Fail"));
    assert!(persisted(&s.subclass("4 17")).is_empty());
}

// ---- replacing ---------------------------------------------------------------------------------

/// `Subclass 7 1 17`: the rows of index 1 go, Prophet takes the index, then the switch to it.
#[test]
fn replacing_a_subclass_deletes_its_rows_and_switches_to_the_new_one() {
    let me = Me {
        subs: vec![holder(BISHOP, 1, 78)],
        ..Me::default()
    };
    let mut s = Scene::new(me, SubclassSettings::default());
    let out = s.subclass("7 1 17");
    let writes = persisted(&out);
    assert_eq!(
        writes[0],
        PersistCommand::Subclass(SubclassWrite::Delete {
            char_id: ME.0,
            class_index: 1,
            henna_slots: vec![],
        })
    );
    assert_eq!(
        writes[1],
        PersistCommand::Subclass(SubclassWrite::Add {
            char_id: ME.0,
            holder: holder(PROPHET, 1, 40),
            skills: vec![skill(1013, 1)],
        })
    );
    assert!(matches!(writes[2], PersistCommand::StorePlayer(_)));
    assert_eq!(
        writes[3],
        PersistCommand::ClassIndexRequest {
            owner: ME,
            class_index: 1
        }
    );
    let out = s.answer(
        1,
        ClassIndexLoad {
            skills: vec![skill(1013, 1)],
            ..ClassIndexLoad::default()
        },
    );
    let me = s.me();
    assert_eq!((me.data.class_id, me.core.level), (PROPHET, 40));
    assert_eq!(s.world.sub_classes(ME), [holder(PROPHET, 1, 40)]);
    let page = &pages(&out)[0];
    assert!(page.contains("SubClass_ModifyOk") && page.contains("[Prophet]"));
}

// ---- levels ------------------------------------------------------------------------------------

/// `SubClassHolder.setLevel` / `setExp`: an active subclass stops at `MaxSubclassLevel`, its exp
/// at the last exp of that level; at the login a stored level outside the limits is corrected.
#[test]
fn a_subclass_stops_at_max_subclass_level() {
    let me = Me {
        class_id: BISHOP,
        class_index: 1,
        level: 79,
        subs: vec![holder(BISHOP, 1, 79)],
        ..Me::default()
    };
    let mut s = Scene::new(me, SubclassSettings::default());
    s.world.add_exp_and_sp(ME, 1.0e9, 0.0, false);
    assert_eq!(s.me().core.level, 80);
    assert_eq!(s.me().data.exp, exp_for(81) - 1);

    // A level above the maximum in the row is the maximum.
    let me = Me {
        class_id: BISHOP,
        class_index: 1,
        level: 84,
        subs: vec![holder(BISHOP, 1, 84)],
        ..Me::default()
    };
    let s = Scene::new(me, SubclassSettings::default());
    assert_eq!(s.me().core.level, 80);
}

/// `addSubClass` itself: index 0, an index in use and the class lock refuse.
#[test]
fn add_sub_class_refusals() {
    let mut s = Scene::new(
        Me {
            subs: vec![holder(BISHOP, 1, 76)],
            ..Me::default()
        },
        SubclassSettings::default(),
    );
    assert!(!s.world.add_sub_class(ME, PROPHET, 0));
    assert!(!s.world.add_sub_class(ME, PROPHET, 1));
    s.world.set_active_class(ME, 1);
    assert!(s.world.is_changing_class(ME));
    assert!(!s.world.add_sub_class(ME, PROPHET, 2));
    // A switch to an index without a subclass releases the lock at once.
    s.answer(1, ClassIndexLoad::default());
    s.log.take();
    s.world.set_active_class(ME, 3);
    assert!(!s.world.is_changing_class(ME));
    assert!(s.world.add_sub_class(ME, PROPHET, 2));
}
