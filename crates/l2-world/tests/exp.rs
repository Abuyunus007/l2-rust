#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Exp, SP and levels (M4-EXP-01): the damage shares of a kill, the level difference, `RateXp`,
//! levelling up with the skills a level brings, and the exp a death costs.
//!
//! The expected numbers are worked out by hand from Java's formulas (`Attackable.calculateExpAndSp`,
//! `PlayableStat.addExp`, `Player.calculateDeathExpPenalty`) and the real `experience.xml` /
//! `experienceLoss.xml` values, not from this code.
//! Java: entity/actor/{Attackable,Player}.java, entity/actor/stat/{PlayableStat,PlayerStat}.java

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, ItemTemplates, KarmaLoss, MapRegions,
    NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates, SkillTrees, SpawnData,
    SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{StatusUpdate, SystemMessage, SystemMessageParam};
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

/// The access level that may not gain exp (`gainExp="false"`).
const NO_EXP_LEVEL: i32 = 7;

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
    fn access_level_can_gain_exp(&self, level: i32) -> bool {
        level != NO_EXP_LEVEL
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

fn of<'a>(sent: &'a Sent, to: i32, name: &str) -> Vec<&'a [u8]> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == name)
        .map(|(_, b)| b.as_slice())
        .collect()
}

/// The `SystemMessage`s one client got, as (id, parameters).
fn messages(sent: &Sent, to: i32) -> Vec<SystemMessage> {
    of(sent, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw))
        .collect()
}

fn long(value: i64) -> SystemMessageParam {
    SystemMessageParam::LongNumber(value)
}

// ------------------------------------------------------------------ synthetic data

/// `experience.xml` of the shipped datapack: the exp each level 1 to 86 starts at.
const EXP: [i64; 86] = [
    0,
    68,
    363,
    1168,
    2884,
    6038,
    11287,
    19423,
    31378,
    48229,
    71202,
    101677,
    141193,
    191454,
    254330,
    331867,
    426288,
    540000,
    675596,
    835862,
    1023784,
    1242546,
    1495543,
    1786379,
    2118876,
    2497077,
    2925250,
    3407897,
    3949754,
    4555796,
    5231246,
    5981576,
    6812513,
    7730044,
    8740422,
    9850166,
    11066072,
    12395215,
    13844951,
    15422929,
    17137087,
    18995665,
    21007203,
    23180550,
    25524868,
    28049635,
    30764654,
    33680052,
    36806289,
    40154162,
    45525133,
    51262490,
    57383988,
    63907911,
    70853089,
    80700831,
    91162654,
    102265881,
    114038596,
    126509653,
    146308200,
    167244337,
    189364894,
    212717908,
    237352644,
    271975263,
    308443198,
    346827154,
    387199547,
    429634523,
    474207979,
    532694979,
    606322775,
    696381369,
    804225364,
    931275828,
    1151275834,
    1511275834,
    2044287599,
    3075966164,
    4295351949,
    5766985062,
    7793077345,
    10235368963,
    13180481103,
    16890558728,
];

fn experience() -> Arc<ExperienceTable> {
    let rows: String = EXP
        .iter()
        .enumerate()
        .map(|(i, exp)| format!(r#"<experience level="{}" tolevel="{exp}" />"#, i + 1))
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
        ))
        .unwrap(),
    )
}

/// `experienceLoss.xml` of the shipped datapack, the levels the tests use (the others are 1.0).
fn experience_loss() -> Arc<ExperienceLoss> {
    Arc::new(
        ExperienceLoss::from_xml(
            r#"<list>
            <experience level="5" val="9.5" />
            <experience level="9" val="9.0" />
            <experience level="10" val="8.875" />
            <experience level="20" val="7.625" />
            </list>"#,
        )
        .unwrap(),
    )
}

fn karma_loss() -> Arc<KarmaLoss> {
    Arc::new(KarmaLoss::from_xml(r#"<list><modifier level="10" val="2.0" /></list>"#).unwrap())
}

/// A class tree with auto-get skills at levels 2, 3 and 5, a skill only an NPC teaches, and
/// two skills a high level one may have lost the right to.
const TREES: &str = r#"<list><skillTree type="classSkillTree" classId="0">
<skill skillName="A" skillId="1001" skillLevel="1" getLevel="2" autoGet="true" />
<skill skillName="B" skillId="1002" skillLevel="1" getLevel="3" autoGet="true" />
<skill skillName="B" skillId="1002" skillLevel="2" getLevel="5" autoGet="true" />
<skill skillName="C" skillId="1003" skillLevel="1" getLevel="4" learnedByNpc="true" />
<skill skillName="D" skillId="1004" skillLevel="1" getLevel="5" autoGet="true" />
<skill skillName="E" skillId="1005" skillLevel="1" getLevel="20" autoGet="true" />
<skill skillName="F" skillId="1006" skillLevel="1" getLevel="5" autoGet="true" />
<skill skillName="F" skillId="1006" skillLevel="2" getLevel="20" autoGet="true" />
</skillTree></list>"#;

fn passive(id: i32, levels: i32) -> String {
    format!(
        r#"<skill id="{id}" levels="{levels}" name="S{id}"><operateType>P</operateType><targetType>SELF</targetType>
        <effects><effect name="Buff"><add stat="mDef" val="1"/></effect></effects></skill>"#
    )
}

fn skills_xml() -> String {
    format!(
        "<list>{}{}{}{}{}{}{}</list>",
        passive(194, 1),
        passive(1001, 1),
        passive(1002, 2),
        passive(1003, 1),
        passive(1004, 1),
        passive(1005, 1),
        passive(1006, 2),
    )
}

/// One class whose HP, MP and CP are 100, 50 and 20 per level, so a level up is easy to see.
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

fn npcs_xml(level: i32, exp: i32, sp: i32) -> String {
    format!(
        r#"<?xml version="1.0"?><list>
  <npc id="9001" type="Monster" level="{level}" name="Monster">
    <acquire exp="{exp}" sp="{sp}" />
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
</list>"#
    )
}

fn synthetic_stats() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&class_xml(), 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            &skills_xml(),
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

fn player_data(id: i32, name: &str, level: i32) -> PlayerData {
    let (hp, mp, cp) = (
        f64::from(100 * level),
        f64::from(50 * level),
        f64::from(20 * level),
    );
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: name.into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
        exp: EXP[usize::try_from(level).unwrap() - 1],
        sp: 0,
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

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: i32 = 0x2000_0001;
const OTHER: i32 = 0x2000_0002;
const SCRATCH: i32 = 0x2000_0003;
const ONE_MONSTER: &str =
    r#"<spawn><npc id="9001" x="-71338" y="258271" z="-3104" heading="77" respawnDelay="20"/></spawn>"#;

/// A temporary datapack root with a spawn file, one empty zone file and one map region.
struct Root(PathBuf);

impl Root {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-exp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["spawns", "zones", "mapregion"] {
            std::fs::create_dir_all(dir.join("data").join(sub)).unwrap();
        }
        std::fs::write(
            dir.join("data/spawns/a.xml"),
            format!(r#"<?xml version="1.0"?><list enabled="true">{ONE_MONSTER}</list>"#),
        )
        .unwrap();
        std::fs::write(
            dir.join("data/zones/test.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\"></list>\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("data/mapregion/talking_island_town.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
             <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
             <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n\
             </region>\n</list>\n",
        )
        .unwrap();
        Self(dir)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// What a scene is made of.
struct Config {
    npc_level: i32,
    npc_exp: i32,
    npc_sp: i32,
    exp: ExpSettings,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            npc_level: 10,
            npc_exp: 1000,
            npc_sp: 100,
            exp: ExpSettings::default(),
        }
    }
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
    _root: Root,
}

impl Scene {
    fn new(tag: &str) -> Self {
        Self::with(tag, Config::default())
    }

    fn with(tag: &str, config: Config) -> Self {
        let root = Root::new(tag);
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(
            WorldRng::from_seed(5),
            Box::new(RecordingOutbox(persisted.clone())),
        );
        world.set_settings(WorldSettings {
            death: DeathSettings {
                disconnect_after_death: false,
                ..DeathSettings::default()
            },
            exp: config.exp,
            ..WorldSettings::default()
        });
        world.set_static_data(Box::new(Data(synthetic_stats())));
        world.set_exp_data(
            experience(),
            experience_loss(),
            karma_loss(),
            Arc::new(SkillTrees::from_xml(TREES).unwrap()),
        );
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_zone_data(
            Arc::new(Zones::load(&root.0)),
            Arc::new(MapRegions::load(&root.0)),
        );
        let npcs = Arc::new(
            NpcTemplates::from_xml(
                &npcs_xml(config.npc_level, config.npc_exp, config.npc_sp),
                &NpcLoadOptions::default(),
            )
            .unwrap(),
        );
        let spawns = Arc::new(SpawnData::load(&root.0, &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.spawn_all_npcs(false);
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

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn npc(&self) -> ObjectId {
        self.world.npc_ids().into_iter().next().unwrap()
    }

    /// `attacker` did `damage` to the monster (hate does not matter here).
    fn hit(&mut self, attacker: i32, damage: i64) {
        let npc = self.npc();
        self.world.add_damage_hate(npc, ObjectId(attacker), damage, 1);
    }

    fn me(&self) -> &Player {
        self.world.player(ObjectId(ME)).unwrap()
    }

    /// The monster dies, last hit by `killer`.
    fn kill(&mut self, killer: i32) {
        let npc = self.npc();
        assert!(self.world.do_die(npc, Some(ObjectId(killer))));
    }
}

fn skills_of(player: &Player) -> Vec<(i32, i32)> {
    let mut skills = player.core.skills.clone();
    skills.sort_unstable();
    skills
}

// ------------------------------------------------------------------ kill rewards

/// Two players share a monster: each gets `reward * damage / total` in `long` arithmetic (cut,
/// not rounded), with `RateXp` and `RateSp` applied to the reward first (`(long) (exp * rate)`).
#[test]
fn a_kill_is_shared_by_damage_with_java_integer_arithmetic() {
    let mut s = Scene::with(
        "share",
        Config {
            exp: ExpSettings {
                rate_xp: 1.5,
                rate_sp: 2.0,
                ..ExpSettings::default()
            },
            ..Config::default()
        },
    );
    s.enter(player_data(ME, "Hero", 10));
    s.enter(player_data(OTHER, "Helper", 10));
    s.enter(player_data(SCRATCH, "Scratch", 10));
    let (start_me, start_other) = (s.me().data.exp, s.world.player(ObjectId(OTHER)).unwrap().data.exp);
    s.hit(ME, 2);
    s.hit(OTHER, 5);
    // A damage of 1 does not count, nor does it get a share: Java's `if (damage > 1)`.
    s.hit(SCRATCH, 1);
    s.kill(OTHER);
    let scratch = s.world.player(ObjectId(SCRATCH)).unwrap();
    assert_eq!((scratch.data.exp, scratch.data.sp), (start_me, 0));

    // exp reward (long) (1000 * 1.5) = 1500, sp (int) (100 * 2.0) = 200; total damage 7:
    // 1500 * 2 / 7 = 428 and 200 * 2 / 7 = 57; 1500 * 5 / 7 = 1071 and 200 * 5 / 7 = 142.
    assert_eq!(s.me().data.exp, start_me + 428);
    assert_eq!(s.me().data.sp, 57);
    let other = s.world.player(ObjectId(OTHER)).unwrap();
    assert_eq!(other.data.exp, start_other + 1071);
    assert_eq!(other.data.sp, 142);

    let sent = s.log.take();
    // PlayerStat.addExpAndSp: addExp -> UserInfo; addSp -> StatusUpdate(SP); then the message.
    let me = names(&sent, ME);
    // The rewards come before the `StopMove` and the `Die` of `Creature.doDie`; in between,
    // `stopAllEffects()` of a monster that respawns ends with `updateAbnormalEffect()`, the
    // `NpcInfo` of the dead monster.
    assert_eq!(
        me.iter()
            .filter(|n| *n != "STOP_MOVE")
            .cloned()
            .collect::<Vec<_>>(),
        vec![
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "STATUS_UPDATE",
            "SYSTEM_MESSAGE",
            "NPC_INFO",
            "DIE"
        ]
    );
    let update: StatusUpdate = decode(of(&sent, ME, "STATUS_UPDATE")[0]);
    assert_eq!(update.object_id, ME);
    assert_eq!(update.attributes, vec![(status::SP, 57)]);
    let msgs = messages(&sent, ME);
    assert_eq!(msgs[0].id, SystemMessageId::YouHaveEarnedS1ExperienceAndS2Sp);
    assert_eq!(msgs[0].params, vec![long(428), long(57)]);
}

/// Exp and SP follow the level difference: nothing from `MonsterExpMaxLevelDifference` (11) up,
/// and from level 85 on the table of `calculateExpAndSp` (levelDiff -3 and below).
#[test]
fn the_level_difference_decides_the_reward() {
    // (player level, monster level, expected exp of the 1000 exp / 100 sp monster).
    let cases = [
        (10, 10, 1000),
        (30, 20, 1000), // +10: still rewarded
        (31, 20, 0),    // +11: the limit
        (10, 20, 1000), // -10
        (9, 20, 0),     // -11
        (85, 87, 1000), // -2: no table below level -3
        (85, 88, 970),  // -3
        (85, 89, 670),
        (85, 90, 420),
        (85, 95, 30),   // -10
        (84, 90, 1000), // the table is for level 85 and above
    ];
    for (player_level, npc_level, expected) in cases {
        let mut s = Scene::with(
            "diff",
            Config {
                npc_level,
                ..Config::default()
            },
        );
        s.enter(player_data(ME, "Hero", player_level));
        let start = s.me().data.exp;
        s.hit(ME, 100);
        s.kill(ME);
        assert_eq!(
            s.me().data.exp - start,
            expected,
            "player level {player_level} against monster level {npc_level}"
        );
        // SP is the same share: 100 * mul, cut.
        let sp_expected = if expected == 0 { 0 } else { expected / 10 };
        assert_eq!(s.me().data.sp, sp_expected, "sp at {player_level}/{npc_level}");
    }
}

/// Players who are too far (`AltPartyRange` 1500), dead, or not in the aggro list's players get
/// nothing, and their damage is not part of the total.
#[test]
fn only_players_in_range_and_alive_are_rewarded() {
    let mut s = Scene::new("range");
    s.enter(player_data(ME, "Hero", 10));
    let mut far = player_data(OTHER, "Far", 10);
    far.x += 1501;
    s.enter(far);
    s.hit(ME, 3);
    s.hit(OTHER, 1000);
    s.kill(ME);
    // The far player's damage is ignored: the hero did all of it.
    assert_eq!(s.me().data.exp - EXP[9], 1000);
    assert_eq!(s.world.player(ObjectId(OTHER)).unwrap().data.exp, EXP[9]);

    // A dead one gets nothing, the other share is still computed from the total.
    let mut s = Scene::new("dead");
    s.enter(player_data(ME, "Hero", 10));
    s.enter(player_data(OTHER, "Ghost", 10));
    s.hit(ME, 5);
    s.hit(OTHER, 5);
    s.world.do_die(ObjectId(OTHER), None);
    s.kill(ME);
    assert_eq!(s.me().data.exp - EXP[9], 500);
}

/// An access level with `gainExp="false"` gets neither exp nor SP.
#[test]
fn an_access_level_that_cannot_gain_exp_gets_nothing() {
    let mut s = Scene::new("no-exp");
    let mut data = player_data(ME, "Hero", 10);
    data.access_level = NO_EXP_LEVEL;
    s.enter(data);
    s.hit(ME, 10);
    s.kill(ME);
    assert_eq!((s.me().data.exp, s.me().data.sp), (EXP[9], 0));
    let sent = s.log.take();
    let me = names(&sent, ME);
    assert!(
        !me.iter()
            .any(|n| n == "USER_INFO" || n == "SYSTEM_MESSAGE" || n == "STATUS_UPDATE"),
        "{me:?}"
    );
}

/// `PlayableStat.addExp` never goes past the last level: one point below `expForLevel(86)`.
#[test]
fn exp_stops_below_the_last_levels_end() {
    let mut s = Scene::new("cap");
    s.enter(player_data(ME, "Hero", 85));
    let id = ObjectId(ME);
    assert!(s.world.add_exp(id, 1_000_000_000_000));
    assert_eq!(s.me().data.exp, EXP[85] - 1);
    assert_eq!(s.me().core.level, 85);
    // At the cap nothing is added any more, but it still reports success (Java returns true).
    assert!(s.world.add_exp(id, 5));
    assert_eq!(s.me().data.exp, EXP[85] - 1);
}

/// `MaxSp` caps the SP, and an SP that could not be added is the "earned experience" message.
#[test]
fn sp_stops_at_max_sp() {
    let mut s = Scene::with(
        "max-sp",
        Config {
            exp: ExpSettings {
                max_sp: 150,
                ..ExpSettings::default()
            },
            ..Config::default()
        },
    );
    s.enter(player_data(ME, "Hero", 10));
    s.hit(ME, 10);
    s.kill(ME);
    assert_eq!(s.me().data.sp, 100);
    let id = ObjectId(ME);
    assert!(s.world.add_sp(id, 100));
    assert_eq!(s.me().data.sp, 150, "capped: 100 + 100 would be 200");
    assert!(!s.world.add_sp(id, 1), "already at the cap");
    assert!(!s.world.add_sp(id, -1), "negative amounts are refused");
}

/// A karma player loses karma for the exp he gets (`Formulas.calculateKarmaLost`).
#[test]
fn exp_lowers_karma() {
    let mut s = Scene::new("karma");
    let mut data = player_data(ME, "Chaotic", 10);
    data.karma = 100;
    s.enter(data);
    s.hit(ME, 10);
    s.kill(ME);
    // |1000 / RateKarmaLost 1.0| / modifier 2.0 / 30 = 16.67 -> 16.
    assert_eq!(s.me().data.karma, 84);
    let sent = s.log.take();
    let msgs = messages(&sent, ME);
    assert!(
        msgs.iter()
            .any(|m| m.id == SystemMessageId::YourKarmaHasBeenChangedToS1
                && m.params == vec![SystemMessageParam::IntNumber(84)]),
        "{msgs:?}"
    );
}

// ------------------------------------------------------------------ levelling

/// Level 1 to 2 (the exp of level 2 exactly), then 2 to 5 in one go: the maxima grow, HP, MP and
/// CP are full, everybody sees the level up, and the auto-get skills of the levels are learned
/// (and stored at once), the skill only an NPC teaches is not.
#[test]
fn levelling_up_from_1_to_2_to_5_learns_the_skills_of_the_levels() {
    let mut s = Scene::new("levelling");
    let mut data = player_data(ME, "Hero", 1);
    // Wounded, so "full" means something.
    data.cur_hp = 30.0;
    data.cur_mp = 10.0;
    data.cur_cp = 3.0;
    s.enter(data);
    s.enter(player_data(OTHER, "Watcher", 1));
    let id = ObjectId(ME);

    // 68 exp = `expForLevel(2)`: level 2.
    assert!(s.world.add_exp_and_sp(id, 68.0, 5.0, false));
    assert_eq!((s.me().core.level, s.me().data.exp, s.me().data.sp), (2, 68, 5));
    assert_eq!(
        (s.me().core.cur_hp, s.me().core.cur_mp, s.me().core.cur_cp),
        (200.0, 100.0, 40.0)
    );
    assert_eq!(
        (s.me().core.max_hp, s.me().core.max_mp, s.me().core.max_cp),
        (200.0, 100.0, 40.0)
    );
    assert_eq!(skills_of(s.me()), vec![(1001, 1)]);
    assert_eq!(s.persisted.skill_changes(), vec![(vec![(1001, 1)], vec![])]);
    // The row Java would store.
    let row = s.me().snapshot();
    assert_eq!((row.level, row.exp, row.sp), (2, 68, 5));
    assert_eq!((row.max_hp, row.max_mp, row.max_cp), (200.0, 100.0, 40.0));
    assert_eq!(
        row.skills,
        vec![SkillData {
            skill_id: 1001,
            skill_level: 1
        }]
    );

    let sent = s.log.take();
    // The order of `PlayerStat.addLevel`: HP and MP full (PlayableStat), CP full, the
    // `SocialAction` for everybody, the message, the new maxima, `UserInfo`, `ExVoteSystemInfo`;
    // then `addExp`'s own `UserInfo` and `addSp`'s `StatusUpdate`.
    assert_eq!(
        names(&sent, ME),
        vec![
            // HP, MP and CP full: the vitals go out 50 ms later, once (`broadcastStatusUpdate`);
            // the HP crossing 30% of the maximum is a `UserInfo` now (`setCurrentHp`).
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "SOCIAL_ACTION",
            "SYSTEM_MESSAGE", // YOUR_LEVEL_HAS_INCREASED
            "STATUS_UPDATE",  // level, max CP, max HP, max MP
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "EX_VOTE_SYSTEM_INFO",
            "USER_INFO",
            "EX_BR_EXTRA_USER_INFO",
            "STATUS_UPDATE",  // SP
            "SYSTEM_MESSAGE", // exp and SP
        ]
    );
    let level_update: StatusUpdate = decode(of(&sent, ME, "STATUS_UPDATE")[0]);
    assert_eq!(
        level_update.attributes,
        vec![
            (status::LEVEL, 2),
            (status::MAX_CP, 40),
            (status::MAX_HP, 200),
            (status::MAX_MP, 100)
        ]
    );
    let msgs = messages(&sent, ME);
    assert_eq!(msgs[0].id, SystemMessageId::YourLevelHasIncreased);
    assert_eq!(msgs[1].id, SystemMessageId::YouHaveEarnedS1ExperienceAndS2Sp);
    // The watcher sees the animation, and nothing private.
    assert!(names(&sent, OTHER).contains(&"SOCIAL_ACTION".to_owned()));
    assert!(!names(&sent, OTHER).contains(&"SYSTEM_MESSAGE".to_owned()));

    // The vitals come 50 ms after the change, all six at the new values.
    s.ticks(1);
    let later = s.log.take();
    let vitals: StatusUpdate = decode(of(&later, ME, "STATUS_UPDATE")[0]);
    assert_eq!(
        vitals.attributes,
        vec![
            (status::MAX_HP, 200),
            (status::CUR_HP, 200),
            (status::MAX_MP, 100),
            (status::CUR_MP, 100),
            (status::MAX_CP, 40),
            (status::CUR_CP, 40),
        ]
    );
    // The skill list comes 300 ms after the level up (Java `sendSkillList`).
    assert!(of(&sent, ME, "SKILL_LIST").is_empty() && of(&later, ME, "SKILL_LIST").is_empty());
    s.ticks(5);
    let later = s.log.take();
    assert_eq!(of(&later, ME, "SKILL_LIST").len(), 1);

    // Level 2 to 5 at once: 2884 exp is the start of level 5.
    s.persisted.clear();
    assert!(s.world.add_exp_and_sp(id, 2884.0 - 68.0, 0.0, false));
    assert_eq!((s.me().core.level, s.me().data.exp), (5, 2884));
    assert_eq!((s.me().core.max_hp, s.me().core.cur_hp), (500.0, 500.0));
    // 1002 is listed in two levels (3 and 5) because the player knew neither: both are given, in
    // order, so the highest stays. The skill a level 4 NPC teaches is not an auto-get skill.
    assert_eq!(
        skills_of(s.me()),
        vec![(1001, 1), (1002, 2), (1004, 1), (1006, 1)],
        "{:?}",
        s.me().core.skills
    );
    let changes = s.persisted.skill_changes();
    assert_eq!(changes.len(), 1);
    let mut written = changes[0].0.clone();
    written.sort_unstable();
    assert_eq!(written, vec![(1002, 2), (1004, 1), (1006, 1)]);
    assert!(changes[0].1.is_empty());
    let row = s.me().snapshot();
    assert_eq!((row.level, row.exp), (5, 2884));
    assert_eq!(row.skills.len(), 4);
    // Only one level up message, whatever the number of levels.
    let sent = s.log.take();
    let level_ups = messages(&sent, ME)
        .iter()
        .filter(|m| m.id == SystemMessageId::YourLevelHasIncreased)
        .count();
    assert_eq!(level_ups, 1);
}

/// A kill that brings the exp of the next level is the whole chain at once.
#[test]
fn a_kill_can_level_the_killer_up() {
    let mut s = Scene::with(
        "kill-level",
        Config {
            npc_level: 1,
            npc_exp: 68,
            npc_sp: 7,
            ..Config::default()
        },
    );
    s.enter(player_data(ME, "Hero", 1));
    s.hit(ME, 50);
    s.kill(ME);
    assert_eq!((s.me().core.level, s.me().data.exp, s.me().data.sp), (2, 68, 7));
    assert_eq!(skills_of(s.me()), vec![(1001, 1)]);
}

/// A skill the class tree gives, once known at that level, is not given again, and a higher level
/// of a known skill replaces it (`UPDATE` of the level, not a second row).
#[test]
fn known_skills_are_only_upgraded() {
    let mut s = Scene::new("upgrade");
    let mut data = player_data(ME, "Hero", 3);
    data.skills = vec![SkillData {
        skill_id: 1002,
        skill_level: 1,
    }];
    s.enter(data);
    assert_eq!(skills_of(s.me()), vec![(1002, 1)]);
    assert!(s.world.add_exp(ObjectId(ME), EXP[4] - EXP[2]));
    assert_eq!(s.me().core.level, 5);
    // 1002 is raised to level 2 (not given twice), the others are new at level 5.
    assert_eq!(
        skills_of(s.me()),
        vec![(1001, 1), (1002, 2), (1004, 1), (1006, 1)]
    );
    assert_eq!(
        s.me().data.skills,
        vec![
            SkillData {
                skill_id: 1002,
                skill_level: 2
            },
            SkillData {
                skill_id: 1001,
                skill_level: 1
            },
            SkillData {
                skill_id: 1004,
                skill_level: 1
            },
            SkillData {
                skill_id: 1006,
                skill_level: 1
            },
        ]
    );
    let writes = s.persisted.skill_changes();
    assert_eq!(writes.len(), 1);
    let mut written = writes[0].0.clone();
    written.sort_unstable();
    assert_eq!(written, vec![(1001, 1), (1002, 2), (1004, 1), (1006, 1)]);
}

// ------------------------------------------------------------------ death

fn die_to_monster(s: &mut Scene) {
    let npc = s.npc();
    assert!(s.world.do_die(ObjectId(ME), Some(npc)));
}

/// Level 20, 50000 exp into the level: a death costs `round(level exp * 7.625 / 100)` of the level
/// (187922 exp wide: 14329), the old exp is kept for a resurrection.
#[test]
fn a_death_costs_exp_and_remembers_it() {
    let mut s = Scene::new("penalty");
    let mut data = player_data(ME, "Hero", 20);
    data.exp = EXP[19] + 50_000;
    s.enter(data);
    let before = s.me().data.exp;
    die_to_monster(&mut s);
    // 187922 * 7.625 / 100 = 14329.05 -> 14329; the cap is 187922 / 10 = 18792.
    assert_eq!(s.me().data.exp, before - 14_329);
    assert_eq!(s.me().data.profile.exp_before_death, before);
    assert_eq!(s.me().core.level, 20);
    // The column Java writes with the next store.
    assert_eq!(s.me().snapshot().profile.exp_before_death, before);
}

/// The loss is at most a tenth of the level's width, and a loss that takes the exp below the
/// level's start takes the level too (HP maxima follow, the death is not a level up).
#[test]
fn a_death_can_take_a_level() {
    let mut s = Scene::new("delevel");
    let mut data = player_data(ME, "Hero", 20);
    data.exp = EXP[19] + 100;
    s.enter(data);
    let before = s.me().data.exp;
    die_to_monster(&mut s);
    assert_eq!(s.me().data.exp, before - 14_329);
    assert_eq!(s.me().core.level, 19);
    assert_eq!(s.me().core.max_hp, 1900.0);
    // No level up message or animation for a lost level.
    let sent = s.log.take();
    assert!(!names(&sent, ME).contains(&"SOCIAL_ACTION".to_owned()));
    assert!(
        messages(&sent, ME)
            .iter()
            .all(|m| m.id != SystemMessageId::YourLevelHasIncreased)
    );
}

/// Never below 1 exp, and `RateKarmaExpLost` multiplies the share of a karma player.
#[test]
fn the_loss_keeps_one_exp_and_karma_multiplies_it() {
    let mut s = Scene::new("floor");
    let mut data = player_data(ME, "Hero", 5);
    data.exp = 100; // level 3, but the player is "level 5" in the data: the loss is bigger
    s.enter(data);
    die_to_monster(&mut s);
    // Level 5: width 6038 - 2884 = 3154; 3154 * 9.5 / 100 = 299.63 -> 300; cap 315. 100 - 300 < 0.
    assert_eq!(s.me().data.exp, 1);
    assert_eq!(s.me().data.profile.exp_before_death, 100);

    let mut s = Scene::with(
        "karma-loss",
        Config {
            exp: ExpSettings {
                rate_karma_exp_lost: 2.0,
                ..ExpSettings::default()
            },
            ..Config::default()
        },
    );
    let mut data = player_data(ME, "Chaotic", 20);
    data.exp = EXP[19] + 50_000;
    data.karma = 500;
    s.enter(data);
    let before = s.me().data.exp;
    die_to_monster(&mut s);
    // 187922 * (7.625 * 2.0) / 100 = 28658.1 -> 28658, capped at 18792.
    assert_eq!(s.me().data.exp, before - 18_792);
}

/// Level 9 and below with the passive skill Lucky lose nothing; `Delevel = False` too.
#[test]
fn lucky_and_delevel_off_spare_the_exp() {
    let mut s = Scene::new("lucky");
    let mut data = player_data(ME, "Rookie", 9);
    data.skills = vec![SkillData {
        skill_id: 194,
        skill_level: 1,
    }];
    s.enter(data);
    let before = s.me().data.exp;
    die_to_monster(&mut s);
    assert_eq!(s.me().data.exp, before);
    assert_eq!(
        s.me().data.profile.exp_before_death,
        0,
        "Java returns before it sets it"
    );

    // Lucky only counts up to level 9.
    let mut s = Scene::new("lucky-10");
    let mut data = player_data(ME, "Older", 10);
    data.exp += 40_000;
    data.skills = vec![SkillData {
        skill_id: 194,
        skill_level: 1,
    }];
    s.enter(data);
    let before = s.me().data.exp;
    die_to_monster(&mut s);
    assert!(s.me().data.exp < before);

    let mut s = Scene::with(
        "no-delevel",
        Config {
            exp: ExpSettings {
                player_delevel: false,
                ..ExpSettings::default()
            },
            ..Config::default()
        },
    );
    let mut data = player_data(ME, "Hero", 20);
    data.exp += 50_000;
    s.enter(data);
    let before = s.me().data.exp;
    die_to_monster(&mut s);
    assert_eq!(s.me().data.exp, before);
}

/// A resurrection with the power of 100 percent gives the lost exp back and forgets it.
#[test]
fn a_resurrection_restores_the_lost_exp() {
    let mut s = Scene::new("restore");
    let mut data = player_data(ME, "Hero", 20);
    data.exp = EXP[19] + 50_000;
    s.enter(data);
    let before = s.me().data.exp;
    die_to_monster(&mut s);
    assert_eq!(s.me().data.exp, before - 14_329);

    // Half of it: round((before - exp) * 50 / 100) = round(7164.5) = 7165.
    s.world.do_revive_with_power(ObjectId(ME), 50.0);
    assert_eq!(s.me().data.exp, before - 14_329 + 7_165);
    assert_eq!(
        s.me().data.profile.exp_before_death,
        0,
        "the resurrection spends it"
    );

    // A second resurrection has nothing to restore.
    s.world.do_die(ObjectId(ME), None);
    let after_second = s.me().data.exp;
    s.world.do_revive_with_power(ObjectId(ME), 100.0);
    // A death without a killer is no exp loss, so nothing was remembered either.
    assert_eq!(s.me().data.exp, after_second);
}

/// Taking a level takes the skills the lower level may not have: 9 levels of grace, a lower level
/// of the skill if the tree has one, else the skill.
#[test]
fn a_lost_level_takes_the_skills_it_no_longer_allows() {
    let mut s = Scene::new("skills-down");
    // Level 10 with exp just above the start; skill 1005 needs 20 (not allowed below 11), 1006 has
    // a level for 5 and one for 20, 1001 needs only 2.
    let mut data = player_data(ME, "Hero", 10);
    data.exp = EXP[9] + 10;
    data.skills = vec![
        SkillData {
            skill_id: 1001,
            skill_level: 1,
        },
        SkillData {
            skill_id: 1002,
            skill_level: 2,
        },
        SkillData {
            skill_id: 1004,
            skill_level: 1,
        },
        SkillData {
            skill_id: 1005,
            skill_level: 1,
        },
        SkillData {
            skill_id: 1006,
            skill_level: 2,
        },
    ];
    s.enter(data);
    assert_eq!(
        skills_of(s.me()),
        vec![(1001, 1), (1002, 2), (1004, 1), (1005, 1), (1006, 2)]
    );
    die_to_monster(&mut s);
    // 10 -> 9: 9 < 20 - 9 for both, so 1005 goes and 1006 drops to its level 1 (needs 5).
    assert_eq!(s.me().core.level, 9);
    assert_eq!(
        skills_of(s.me()),
        vec![(1001, 1), (1002, 2), (1004, 1), (1006, 1)]
    );
    assert_eq!(
        s.me().data.skills,
        vec![
            SkillData {
                skill_id: 1001,
                skill_level: 1
            },
            SkillData {
                skill_id: 1002,
                skill_level: 2
            },
            SkillData {
                skill_id: 1004,
                skill_level: 1
            },
            SkillData {
                skill_id: 1006,
                skill_level: 1
            },
        ]
    );
    let changes = s.persisted.skill_changes();
    assert!(
        changes
            .iter()
            .any(|(up, rem)| up.contains(&(1006, 1)) && rem.contains(&1005)),
        "{changes:?}"
    );
}
