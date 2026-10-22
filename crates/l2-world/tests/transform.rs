#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Transformations (M6-TRANS-01): the `Transformation` effect, the stats of a form, the skill and
//! action lists, the inventory block, `TransformDispel` and the end of a form on death and over a
//! relog. Expected behaviour comes from the Java code named in each test
//! (`Player.transform` / `untransform` / `sendSkillList`, `Transform.onTransform`,
//! `CreatureStat.calcStat`, `PlayerStat.getBaseMoveSpeed`, `DispelBySlot`, ...).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, ItemTemplates, KarmaLoss,
    NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillTemplates, SkillTrees, SpawnData, Transforms,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{CharInfo, ExBasicActionList, ItemList, SkillList, SystemMessage, UserInfo};
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

#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Outbox for Persisted {
    fn persist(&mut self, command: PersistCommand) {
        self.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
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

// ---- the data ----------------------------------------------------------------------------------

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

/// 101 is a combat form with level data for level 1, 102 a non-combat one with an allowed-items
/// list, 103 a flying one, 104 a stance, 105 a form with both item lists, 106 renames the player
/// like the cursed weapons do.
const FORMS: &str = r#"<list>
<transform id="101" displayId="150" type="COMBAT" can_swim="0" normal_attackable="0">
  <Male>
    <common>
      <base range="33" attackSpeed="500" attackType="BOW" critRate="7" mAtk="6" pAtk="500" randomDamage="10"/>
      <stats str="50" int="21" con="43" dex="30" wit="11" men="25"/>
      <defense chest="31" legs="18" head="12" feet="7" gloves="8" underwear="3" cloak="1"/>
      <magicDefense rear="9" lear="9" rfinger="5" lfinger="5" neck="13"/>
      <collision radius="12" height="14.5"/>
      <moving walk="30" run="200" waterWalk="50" waterRun="51" flyWalk="0" flyRun="0"/>
    </common>
    <actions>1 2 3 1000</actions>
    <skills><skill id="9300" level="1"/><skill id="9399" level="1"/></skills>
    <additionalSkills><skill id="9302" level="2" minLevel="5"/></additionalSkills>
    <levels><level val="99" levelMod="2.0" hp="1000" mp="500" cp="300" hpRegen="3" mpRegen="1" cpRegen="2"/></levels>
  </Male>
</transform>
<transform id="107" type="COMBAT">
  <Male>
    <common>
      <base range="33" attackSpeed="500" attackType="BOW" critRate="7" mAtk="6" pAtk="500" randomDamage="10"/>
      <collision radius="12" height="14.5"/>
      <moving walk="30" run="200"/>
    </common>
    <levels><level val="1" levelMod="2.0" hp="1000" mp="500" cp="300" hpRegen="3" mpRegen="1" cpRegen="2"/></levels>
  </Male>
</transform>
<transform id="102" type="NON_COMBAT" can_swim="1">
  <Male>
    <common><base range="20" attackType="FIST"/><collision radius="8" height="9"/><moving walk="10" run="20"/></common>
    <skills><skill id="9300" level="1"/></skills>
    <items><item id="57" allowed="true"/></items>
  </Male>
</transform>
<transform id="103" type="FLYING">
  <Male>
    <common><collision radius="8" height="9"/><moving walk="96" run="176" flyWalk="96" flyRun="176"/></common>
  </Male>
</transform>
<transform id="104" type="MODE_CHANGE">
  <Male>
    <common><base pAtk="777"/><collision radius="7" height="8"/><moving walk="40" run="140"/></common>
  </Male>
</transform>
<transform id="105" type="COMBAT">
  <Male>
    <common><collision radius="8" height="9"/><moving walk="40" run="140"/></common>
    <items><item id="57" allowed="true"/><item id="1" allowed="false"/></items>
  </Male>
</transform>
<transform id="106" type="CURSED" setName="Zariche" setTitle="">
  <Male>
    <common><collision radius="8" height="9"/><moving walk="40" run="140"/></common>
  </Male>
</transform>
</list>"#;

const SKILLS: &str = r##"<list>
<skill id="9101" levels="1" name="Transform 101"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>500</hitTime><abnormalTime>60</abnormalTime><abnormalType>TRANSFORM</abnormalType><abnormalLevel>1</abnormalLevel>
<conditions><player canTransform="true"/></conditions>
<effects><effect name="Transformation"><id>101</id><mul stat="mDef" val="2"/></effect></effects></skill>
<skill id="9102" levels="1" name="Transform 102"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>TRANSFORM</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Transformation"><id>102</id></effect></effects></skill>
<skill id="9103" levels="1" name="Transform 103"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>TRANSFORM</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Transformation"><id>103</id></effect></effects></skill>
<skill id="9104" levels="1" name="Transform 104"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>TRANSFORM</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Transformation"><id>104</id></effect></effects></skill>
<skill id="9105" levels="1" name="Transform 105"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>TRANSFORM</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Transformation"><id>105</id></effect></effects></skill>
<skill id="9106" levels="1" name="Transform 106"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>TRANSFORM</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Transformation"><id>106</id></effect></effects></skill>
<skill id="9107" levels="1" name="Transform 999"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>TRANSFORM</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Transformation"><id>999</id></effect></effects></skill>
<skill id="9108" levels="1" name="Transform 107"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>TRANSFORM</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Transformation"><id>107</id></effect></effects></skill>
<skill id="9110" levels="1" name="Transform Dispel"><operateType>A1</operateType><targetType>SELF</targetType>
<hitTime>500</hitTime>
<conditions><player canUntransform="true"/></conditions>
<effects><effect name="DispelBySlot"><dispel>TRANSFORM,-1</dispel></effect></effects></skill>
<skill id="9111" levels="1" name="Dispel transform (chance)"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="DispelBySlotProbability"><dispel>TRANSFORM</dispel><rate>100</rate></effect></effects></skill>
<skill id="9112" levels="1" name="Only in a form"><operateType>A1</operateType><targetType>SELF</targetType>
<hitTime>100</hitTime>
<conditions><player transformationId="-1"/></conditions>
<effects><effect name="Buff"/></effects></skill>
<skill id="9300" levels="1" name="Form skill"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>100</hitTime><abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9302" levels="2" name="Level skill"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>100</hitTime><abnormalTime>60</abnormalTime><abnormalType>MA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"/></effects></skill>
<skill id="9400" levels="1" name="Own active"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>100</hitTime><abnormalTime>60</abnormalTime><abnormalType>PD_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"/></effects></skill>
<skill id="9401" levels="1" name="Own passive"><operateType>P</operateType><targetType>SELF</targetType></skill>
</list>"##;

const ITEMS: &str = r#"<list>
<item id="4000" type="EtcItem" name="Thing"><set name="is_stackable" val="false" /></item>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
</list>"#;

/// A weak monster to attack.
const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="9001" type="Monster" level="1" name="Weak Monster">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
</list>"#;

struct Data(StatData, Arc<Transforms>);

impl StaticData for Data {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.0.skills.iter().any(|s| s.id == skill_id && s.is_passive())
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
    fn transforms(&self) -> Option<&Arc<Transforms>> {
        Some(&self.1)
    }
}

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
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
    }
}

const ME: ObjectId = ObjectId(1);
const P2: ObjectId = ObjectId(2);

fn player(id: ObjectId, x: i32, skills: &[i32]) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: format!("P{}", id.0),
        title: "Title".into(),
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
        x,
        y: 1000,
        z: 0,
        heading: 0,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items: vec![],
        skills: skills
            .iter()
            .map(|&skill_id| SkillData {
                skill_id,
                skill_level: 1,
            })
            .collect(),
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    me: Log,
    p2: Log,
    persisted: Persisted,
}

impl Scene {
    fn new() -> Self {
        Self::at(1000, PlayerProfile::default())
    }

    fn at(x: i32, profile: PlayerProfile) -> Self {
        Self::build(x, profile, vec![])
    }

    fn build(x: i32, profile: PlayerProfile, items: Vec<ItemData>) -> Self {
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(persisted.clone()));
        world.set_static_data(Box::new(Data(
            stat_data(),
            Arc::new(Transforms::from_xml(FORMS).unwrap()),
        )));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        let me = Log::default();
        let mut data = player(ME, x, &[9101, 9110, 9112, 9400, 9401]);
        data.profile = profile;
        data.items = items;
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(data),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let p2 = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(2),
            player: Box::new(player(P2, x + 50, &[])),
            sink: Box::new(Sink(p2.clone())),
            ip: None,
        });
        let mut scene = Self {
            world,
            me,
            p2,
            persisted,
        };
        scene.advance(1000);
        scene.me.take();
        scene.p2.take();
        scene
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    /// The skill lands on ME without the cast (`Skill.applyEffects`).
    fn apply(&mut self, skill: i32) {
        self.world.apply_effects(ME, ME, SkillKey::new(skill, 1));
    }

    fn use_skill(&mut self, magic_id: i32) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::MagicSkillUse {
                magic_id,
                ctrl_pressed: false,
                shift_pressed: false,
            },
        });
    }

    fn view(&self) -> l2_world::stats::StatView<'_> {
        self.world.stats(ME).unwrap()
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

fn of<'a>(packets: &'a [Vec<u8>], name: &str) -> Vec<&'a Vec<u8>> {
    packets.iter().filter(|p| name_of(p) == name).collect()
}

fn message_ids(packets: &[Vec<u8>]) -> Vec<SystemMessageId> {
    of(packets, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p).id)
        .collect()
}

// ---- the stats of a form -----------------------------------------------------------------------

/// `CreatureStat.calcStat` takes the form's value as the start value, `getBaseMoveSpeed` /
/// `getPhysicalAttackRange` / `getAttackType` / the collision come from the template, and
/// `Player.getLevelMod` from the level data of the player's level.
#[test]
fn a_form_replaces_the_base_values() {
    let mut s = Scene::new();
    let before = (
        s.view().p_atk(None),
        s.view().run_speed(),
        s.view().level_mod(),
        s.view().collision_radius(),
    );
    assert_eq!(before.2, 0.9);
    assert_eq!(before.3, 9.0);

    s.apply(9101);
    assert!(s.world.is_transformed(ME));
    assert_eq!(s.world.transformation_id(ME), 101);
    let v = s.view();
    assert_eq!(v.transformation_display_id(), 150);
    // NOTE(java-quirk): the level data of the datapack's forms is the one of level 99 (see
    // `l2_data::transform`), so a level-1 player gets the class' level modifier and HP.
    assert_eq!(v.level_mod(), 0.9);
    assert_eq!(v.max_hp(), 100);
    // pAtk 500 (template) * STR bonus 1 * level mod 0.9.
    assert_eq!(v.p_atk(None), 450.0);
    assert_eq!(v.run_speed(), 200.0);
    assert_eq!(v.walk_speed(), 30.0);
    assert_eq!(v.swim_run_speed(), 51.0);
    assert_eq!(v.physical_attack_range(), 33);
    assert_eq!(v.attack_type(), l2_data::item::WeaponType::Bow);
    assert_eq!(v.collision_radius(), 12.0);
    assert_eq!(v.collision_height(), 14.5);
    // The attack speed (500) and the critical rate (7) come from the form too.
    assert_eq!(v.p_atk_spd(), 500.0);
    assert_eq!(v.str(), 50);

    // The form ends: everything is the class' again.
    s.apply(9110);
    assert!(!s.world.is_transformed(ME));
    let v = s.view();
    assert_eq!(v.p_atk(None), before.0);
    assert_eq!(v.run_speed(), before.1);
    assert_eq!(v.level_mod(), before.2);
    assert_eq!(v.collision_radius(), before.3);
    assert_eq!(v.max_hp(), 100);
}

/// `Transform.getStat`: the level data of the player's level replaces the template's value,
/// and it knows HP, MP, CP and their regeneration only (0 for the rest, which `calcStat` ignores);
/// `Player.getLevelMod` takes its `levelMod`.
#[test]
fn level_data_takes_over_the_stats_it_lists() {
    let mut s = Scene::new();
    s.apply(9108);
    let v = s.view();
    assert_eq!(v.level_mod(), 2.0);
    assert_eq!(v.max_hp(), 1000);
    assert_eq!(v.max_mp(), 500);
    assert_eq!(v.max_cp(), 300);
    // pAtk is not in the level data: the template's 500 is not used any more. Class pAtk 4 * 2.0.
    assert_eq!(v.p_atk(None), 8.0);
    // The speeds and the collision do not go through `calcStat`.
    assert_eq!(v.run_speed(), 200.0);
    assert_eq!(v.collision_radius(), 12.0);
}

/// A stance is not `isTransformed()` (no id, no display id, the class' speeds and collision), but
/// `getTransformation()` is set, so its stats replace the start values.
#[test]
fn a_stance_changes_the_stats_but_is_not_a_transformation() {
    let mut s = Scene::new();
    let run = s.view().run_speed();
    s.apply(9104);
    assert!(s.world.is_in_stance(ME));
    assert!(!s.world.is_transformed(ME));
    let v = s.view();
    assert_eq!(v.transformation_id(), 0);
    assert_eq!(v.transformation_display_id(), 0);
    assert_eq!(v.run_speed(), run);
    assert_eq!(v.collision_radius(), 9.0);
    // `calcStat` uses `getTransformation()`: pAtk 777 (not 4).
    assert_eq!(v.p_atk(None), 777.0 * 0.9);
}

// ---- the packets of a transformation -----------------------------------------------------------

/// `Transform.onTransform` sends the action list, `Player.transform` the skill list (300 ms
/// later), `SkillCoolTime` and the user info; the others get a `CharInfo` (twice: the second is
/// the one "required for showing mount transformations to players that just entered the game").
#[test]
fn transforming_sends_the_lists_and_the_user_info() {
    let mut s = Scene::new();
    s.apply(9101);
    let sent = s.me.take();
    let n = names(&sent);
    assert!(n.contains(&"EX_BASIC_ACTION_LIST".to_string()), "{n:?}");
    assert!(n.contains(&"SKILL_COOL_TIME".to_string()), "{n:?}");
    let actions: ExBasicActionList = decode(of(&sent, "EX_BASIC_ACTION_LIST")[0]);
    assert_eq!(actions.action_ids, [1, 2, 3, 1000]);
    let info: UserInfo = decode(of(&sent, "USER_INFO").last().unwrap());
    assert_eq!(info.transformation_display_id, 150);
    assert_eq!(info.run_speed, 200);
    assert_eq!(info.collision_radius, 12.0);

    s.advance(400);
    let later = s.me.take();
    let list: SkillList = decode(of(&later, "SKILL_LIST")[0]);
    // The passive skill stays, the active one is hidden, the form's skills follow by id: 9300
    // (9399 is no skill and the additional skill needs level 5).
    let ids: Vec<(i32, bool)> = list.skills.iter().map(|e| (e.id, e.passive)).collect();
    assert_eq!(ids, [(9401, true), (9300, false)]);
    assert!(s.world.has_transform_skill(ME, 9300));
    assert!(!s.world.has_transform_skill(ME, 9400));

    let seen = s.p2.take();
    let infos: Vec<CharInfo> = of(&seen, "CHAR_INFO").into_iter().map(|p| decode(p)).collect();
    assert!(!infos.is_empty(), "{:?}", names(&seen));
    assert!(infos.iter().all(|i| i.transformation_display_id == 150));
    // `broadcastCharInfo` sends one `CharInfo`; the second is `sendInfo`'s, see the next test.
    assert_eq!(infos.len(), 1);

    // Untransforming brings the standard action list and the whole skill list back.
    s.apply(9110);
    let sent = s.me.take();
    let actions: ExBasicActionList = decode(of(&sent, "EX_BASIC_ACTION_LIST").last().unwrap());
    assert_eq!(actions.action_ids, ExBasicActionList::default_list().action_ids);
    let info: UserInfo = decode(of(&sent, "USER_INFO").last().unwrap());
    assert_eq!(info.transformation_display_id, 0);
    s.advance(400);
    let later = s.me.take();
    let list: SkillList = decode(of(&later, "SKILL_LIST")[0]);
    let mut ids: Vec<i32> = list.skills.iter().map(|e| e.id).collect();
    ids.sort_unstable();
    assert_eq!(ids, [9101, 9110, 9112, 9400, 9401]);
    assert!(!s.world.has_transform_skill(ME, 9300));
}

/// `RequestMagicSkillUse` finds a skill of the form through `getTransformSkill` (after the skill
/// list made the map), and a non-combat form allows nothing else.
#[test]
fn a_non_combat_form_casts_its_own_skills_only() {
    let mut s = Scene::new();
    s.apply(9102);
    s.advance(400);
    s.me.take();
    // 9400 is the player's own skill: refused silently (ActionFailed).
    s.use_skill(9400);
    let sent = s.me.take();
    assert_eq!(names(&sent), ["ACTION_FAIL"], "{:?}", names(&sent));
    // 9300 is the form's skill: it is cast.
    s.use_skill(9300);
    let sent = s.me.take();
    assert!(!of(&sent, "MAGIC_SKILL_USE").is_empty(), "{:?}", names(&sent));
    s.advance(600);
    assert!(s.world.is_affected_by_skill(ME, 9300));
}

/// A combat form (and a stance) may cast the skills the player knows.
#[test]
fn a_combat_form_keeps_the_players_skills() {
    let mut s = Scene::new();
    s.apply(9101);
    s.advance(400);
    s.me.take();
    s.use_skill(9400);
    let sent = s.me.take();
    assert!(!of(&sent, "MAGIC_SKILL_USE").is_empty(), "{:?}", names(&sent));
}

/// `Creature.doAttack`: a transformed player of a form with `normal_attackable="0"` sends
/// `ActionFailed` and does not attack; the same player attacks without the form.
#[test]
fn a_form_that_cannot_attack_does_not_attack() {
    let click = |s: &mut Scene, npc: ObjectId| {
        s.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        });
    };
    let attacks = |s: &mut Scene| {
        let sent = s.me.take();
        of(&sent, "ATTACK").len()
    };

    let mut s = Scene::new();
    let npc = s.world.spawn_npc_at(9001, 1040, 1000, 0, 0).unwrap();
    s.me.take();
    click(&mut s, npc);
    click(&mut s, npc);
    s.advance(1000);
    assert!(attacks(&mut s) > 0, "the player attacks without the form");

    let mut s = Scene::new();
    let npc = s.world.spawn_npc_at(9001, 1040, 1000, 0, 0).unwrap();
    s.apply(9101);
    s.me.take();
    click(&mut s, npc);
    click(&mut s, npc);
    s.advance(1000);
    let sent = s.me.take();
    assert!(of(&sent, "ATTACK").is_empty(), "{:?}", names(&sent));
}

// ---- Player.transform's refusals ---------------------------------------------------------------

/// "You already polymorphed and cannot polymorph again."
#[test]
fn a_second_form_is_refused_with_a_message() {
    let mut s = Scene::new();
    s.apply(9101);
    s.me.take();
    assert!(s.world.transform_player(102, ME));
    let sent = s.me.take();
    assert_eq!(
        message_ids(&sent),
        [SystemMessageId::YouAlreadyPolymorphedAndCannotPolymorphAgain]
    );
    assert_eq!(s.world.transformation_id(ME), 101);
    // `TransformData.transformPlayer` answers `false` for an id without a form.
    assert!(!s.world.transform_player(999, ME));
}

/// `Player.transform`: a flying form does not start east of `GRACIA_MAX_X` (-166168).
#[test]
fn a_flying_form_needs_the_gracia_area() {
    let mut s = Scene::new();
    s.apply(9103);
    assert!(!s.world.is_transformed(ME), "x = 1000 is outside Gracia");
    assert!(!s.world.is_affected_by_skill(ME, 9103) || s.world.transformation_id(ME) == 0);

    let mut s = Scene::at(-170_000, PlayerProfile::default());
    s.apply(9103);
    assert!(s.world.is_transformed(ME));
    let v = s.view();
    assert!(v.is_flying() && v.is_flying_mounted());
    s.me.take();
    s.world.untransform(ME);
    assert!(!s.view().is_flying());
}

/// The flying form's user info: fly speeds are the run and walk speeds, the water/flying byte 2.
#[test]
fn a_flying_form_shows_fly_speeds() {
    let mut s = Scene::at(-170_000, PlayerProfile::default());
    s.apply(9103);
    let sent = s.me.take();
    let info: UserInfo = decode(of(&sent, "USER_INFO").last().unwrap());
    assert!(info.fly_run_speed > 0 && info.fly_walk_speed > 0);
    assert_eq!(info.fly_run_speed, info.run_speed);
    assert_eq!(info.water_or_flying, 2);
}

// ---- the inventory block -----------------------------------------------------------------------

/// `Transform.onTransform`: an allowed-items list is block mode 1 (only those), the item list
/// carries the block, `untransform` removes it.
#[test]
fn an_allowed_items_list_blocks_every_other_item() {
    let mut s = Scene::new();
    s.apply(9102);
    assert!(s.world.can_manipulate_with_item_id(ME, 57));
    assert!(!s.world.can_manipulate_with_item_id(ME, 1));
    s.advance(300);
    let sent = s.me.take();
    let list: ItemList = decode(of(&sent, "ITEM_LIST")[0]);
    let block = list.block.expect("the block");
    assert_eq!((block.mode, block.items), (1, vec![57]));

    s.world.untransform(ME);
    assert!(s.world.can_manipulate_with_item_id(ME, 1));
    s.advance(300);
    let sent = s.me.take();
    let list: ItemList = decode(of(&sent, "ITEM_LIST").last().unwrap());
    assert!(list.block.is_none());
}

/// NOTE(java-quirk): a form with both lists sets mode 1 and then mode 2, which replaces it; mode 2
/// only greys the items out in the client.
#[test]
fn both_item_lists_keep_the_second_block_only() {
    let mut s = Scene::new();
    s.apply(9105);
    s.advance(300);
    let sent = s.me.take();
    let list: ItemList = decode(of(&sent, "ITEM_LIST").last().unwrap());
    let block = list.block.expect("the block");
    assert_eq!((block.mode, block.items), (2, vec![1]));
    assert!(s.world.can_manipulate_with_item_id(ME, 1));
}

// ---- the rules ---------------------------------------------------------------------------------

/// `RequestActionUse`: a transformed player may use the actions of `ACTIONS_ON_TRANSFORM` only.
#[test]
fn a_transformed_player_has_a_short_action_list() {
    let mut s = Scene::new();
    assert!(s.world.transformed_action_allowed(ME, 0));
    s.apply(9101);
    assert!(!s.world.transformed_action_allowed(ME, 0));
    assert!(s.world.transformed_action_allowed(ME, 1));
    assert!(s.world.transformed_action_allowed(ME, 1000));
    assert!(!s.world.transformed_action_allowed(ME, 1099));
}

/// `UserInfo` writes the visible name (`Zariche`), `CharInfo` the visible name and title (empty);
/// UserInfo's title stays the real one. They come back with the form.
#[test]
fn a_form_can_rename_the_player() {
    let mut s = Scene::new();
    s.apply(9106);
    let sent = s.me.take();
    let info: UserInfo = decode(of(&sent, "USER_INFO").last().unwrap());
    assert_eq!((info.name.as_str(), info.title.as_str()), ("Zariche", "Title"));
    s.advance(300);
    let seen = s.p2.take();
    let char_info: CharInfo = decode(of(&seen, "CHAR_INFO")[0]);
    assert_eq!(
        (char_info.name.as_str(), char_info.title.as_str()),
        ("Zariche", "")
    );

    s.world.untransform(ME);
    let sent = s.me.take();
    let info: UserInfo = decode(of(&sent, "USER_INFO").last().unwrap());
    assert_eq!(info.name, "P1");
}

// ---- the conditions of the skills ---------------------------------------------------------------

/// `ConditionPlayerCanTransform` / `ConditionPlayerTransformationId` on a real cast: a transformed
/// player cannot start a transformation again (with the message), `transformationId="-1"`
/// (`isTransformed()`) opens a skill only inside a form, and `ConditionPlayerCanUntransform` lets
/// the dispel skill through.
#[test]
fn the_skill_conditions_follow_the_form_state() {
    let mut s = Scene::new();
    // Not transformed: the form-only skill is refused, the transformation starts.
    s.use_skill(9112);
    let sent = s.me.take();
    assert!(of(&sent, "MAGIC_SKILL_USE").is_empty(), "{:?}", names(&sent));
    s.use_skill(9101);
    let sent = s.me.take();
    assert!(!of(&sent, "MAGIC_SKILL_USE").is_empty(), "{:?}", names(&sent));
    s.advance(700);
    assert!(s.world.is_transformed(ME));

    // Transformed: it cannot be done again ...
    s.me.take();
    s.use_skill(9101);
    let sent = s.me.take();
    assert!(
        message_ids(&sent).contains(&SystemMessageId::YouAlreadyPolymorphedAndCannotPolymorphAgain),
        "{:?}",
        names(&sent)
    );
    assert!(of(&sent, "MAGIC_SKILL_USE").is_empty());
    // ... the form-only skill works, and so does the dispel.
    s.use_skill(9112);
    let sent = s.me.take();
    assert!(!of(&sent, "MAGIC_SKILL_USE").is_empty(), "{:?}", names(&sent));
    s.advance(400);
    s.use_skill(9110);
    s.advance(700);
    assert!(!s.world.is_transformed(ME));
}

/// A mounted player cannot start the skill (`ConditionPlayerCanTransform`: `isMounted()` gives
/// `YOU_CANNOT_POLYMORPH_WHILE_RIDING_A_PET`), and `TransformData.transformPlayer` gets the rider
/// off the mount first (`Player.transform` calls `dismount()`).
#[test]
fn a_mount_blocks_the_skill_and_transform_player_dismounts() {
    let mut s = Scene::new();
    assert!(s.world.mount_npc(ME, 12526, ObjectId(0), false));
    s.me.take();
    s.use_skill(9101);
    let sent = s.me.take();
    assert!(
        message_ids(&sent).contains(&SystemMessageId::YouCannotPolymorphWhileRidingAPet),
        "{:?}",
        names(&sent)
    );
    assert!(of(&sent, "MAGIC_SKILL_USE").is_empty());
    assert!(s.world.is_mounted(ME) && !s.world.is_transformed(ME));

    // The admin way in: the rider lands first, then the form starts.
    assert!(s.world.transform_player(101, ME));
    assert!(!s.world.is_mounted(ME));
    assert_eq!(s.world.transformation_id(ME), 101);
}

// ---- how a form ends ---------------------------------------------------------------------------

/// `DispelBySlotProbability` (with a TRANSFORM slot) also stops the transformation.
#[test]
fn dispel_by_slot_probability_stops_the_transformation() {
    let mut s = Scene::new();
    s.apply(9101);
    assert!(s.world.is_transformed(ME));
    s.apply(9111);
    assert!(!s.world.is_transformed(ME));
    assert!(!s.world.is_affected_by_skill(ME, 9101));
}

/// The transformation is a buff: when it runs out (`abnormalTime` 60 s), `Transformation.onExit`
/// ends the form.
#[test]
fn the_form_ends_with_its_buff() {
    let mut s = Scene::new();
    s.apply(9101);
    s.advance(59_000);
    assert!(s.world.is_transformed(ME));
    s.advance(2_000);
    assert!(!s.world.is_transformed(ME));
    assert!(!s.world.is_affected_by_skill(ME, 9101));
}

/// Death stops the buffs and with them the form (`Creature.doDie` ->
/// `stopAllEffectsExceptThoseThatLastThroughDeath`).
#[test]
fn death_ends_the_form() {
    let mut s = Scene::new();
    s.apply(9101);
    s.world.reduce_current_hp(ME, 1_000_000.0, Some(P2));
    assert!(s.world.is_dead(ME));
    assert!(!s.world.is_transformed(ME));
}

/// The buff is stored with the others and restored when the character enters again, which
/// transforms the player again (`Player.restoreEffects` -> `Skill.applyEffects`).
#[test]
fn the_form_comes_back_after_a_relog() {
    let mut s = Scene::new();
    s.apply(9101);
    s.advance(10_000);
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    let stored = s.persisted.last_store();
    let rows = stored.profile.effects_to_store.clone().expect("rows");
    assert!(rows.iter().any(|r| r.skill_id == 9101 && r.restore_type == 0));

    let profile = PlayerProfile {
        effects_to_restore: rows,
        ..PlayerProfile::default()
    };
    let s = Scene::at(1000, profile);
    assert!(s.world.is_transformed(ME));
    assert_eq!(s.world.transformation_id(ME), 101);
}

/// A transformation skill whose form id is not in the datapack does nothing (Java's
/// `transformPlayer` answers false).
#[test]
fn an_unknown_form_changes_nothing() {
    let mut s = Scene::new();
    s.apply(9107);
    assert!(!s.world.is_transformed(ME));
    assert!(!s.world.is_in_stance(ME));
}

/// `WaterZone.onEnter`: a form that cannot swim ends, one that can stays.
#[test]
fn water_ends_a_form_that_cannot_swim() {
    let mut s = Scene::new();
    s.apply(9102);
    s.world.transformed_player_entered_water(ME);
    assert!(s.world.is_transformed(ME), "form 102 can swim");

    let mut s = Scene::new();
    s.apply(9101);
    s.world.transformed_player_entered_water(ME);
    assert!(!s.world.is_transformed(ME), "form 101 cannot");
    assert!(!s.world.is_affected_by_skill(ME, 9101));
}

// ---- levelling up ------------------------------------------------------------------------------

/// `Transform.onLevelUp`: the additional skills of the new level are added to the player and to
/// its transformation skills (the skill list then shows them).
#[test]
fn a_level_up_adds_the_additional_skills_of_the_form() {
    let mut s = Scene::new();
    let rows: String = (1..=86)
        .map(|level| format!(r#"<experience level="{level}" tolevel="{}" />"#, level * 1000))
        .collect();
    let table = ExperienceTable::from_xml(&format!(
        r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
    ))
    .unwrap();
    s.world.set_exp_data(
        Arc::new(table),
        Arc::new(ExperienceLoss::default()),
        Arc::new(KarmaLoss::default()),
        Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
    );
    s.apply(9101);
    s.advance(400);
    assert!(!s.world.has_transform_skill(ME, 9302));
    assert!(s.world.add_level(ME, 3));
    // Level 4: the additional skill needs level 5.
    assert_eq!(s.view().level(), 4);
    assert_eq!(s.view().skill_level(9302), 0);
    assert!(s.world.add_level(ME, 1));
    assert_eq!(s.view().level(), 5);
    assert_eq!(s.view().skill_level(9302), 2);
    assert!(s.world.has_transform_skill(ME, 9302));

    // Without a form the level up gives nothing of the sort.
    let mut other = Scene::new();
    other.world.set_exp_data(
        Arc::new(
            ExperienceTable::from_xml(&format!(
                r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
            ))
            .unwrap(),
        ),
        Arc::new(ExperienceLoss::default()),
        Arc::new(KarmaLoss::default()),
        Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
    );
    assert!(other.world.add_level(ME, 4));
    assert_eq!(other.view().skill_level(9302), 0);
}

/// `ValidatePosition`: "Don't allow flying transformations outside gracia area!"
#[test]
fn a_flying_form_ends_when_the_client_reports_a_position_outside_gracia() {
    let mut s = Scene::at(-170_000, PlayerProfile::default());
    s.apply(9103);
    assert!(s.world.is_transformed(ME));
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::ValidatePosition {
            x: -170_000,
            y: 1000,
            z: 0,
            heading: 0,
        },
    });
    assert!(s.world.is_transformed(ME), "inside Gracia the form stays");
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::ValidatePosition {
            x: -100_000,
            y: 1000,
            z: 0,
            heading: 0,
        },
    });
    assert!(!s.world.is_transformed(ME));
    assert!(!s.view().is_flying());
}

// ---- the defence functions ---------------------------------------------------------------------

/// `FuncPDefMod`: the chest takes the form's defence off as a value, every other slot hands the
/// form's value to the class template as a slot number (NOTE(java-quirk)). Form 101 has head 12:
/// slot 12 is the feet (7) of the class, not the head (12).
#[test]
fn the_defence_functions_use_the_forms_values_like_java() {
    const HEAD_SLOT: i32 = 1;
    let item = ItemData {
        object_id: ObjectId(0x3000_0001),
        item_id: 9999,
        count: 1,
        loc: "PAPERDOLL".into(),
        loc_data: HEAD_SLOT,
        mana_left: -1,
        time: -1,
        ..Default::default()
    };
    let mut s = Scene::build(1000, PlayerProfile::default(), vec![item]);
    let plain = s.view().p_def(None);
    // Class pDef 31 + 18 + 12 + 7 + 8 + 3 + 1 = 80; the helmet takes its 12 off; level mod 0.9.
    assert!((plain - (80.0 - 12.0) * 0.9).abs() < 1e-9, "{plain}");
    s.apply(9101);
    let transformed = s.view().p_def(None);
    assert!((transformed - (80.0 - 7.0) * 0.9).abs() < 1e-9, "{transformed}");
}

// ---- the shortcuts -----------------------------------------------------------------------------

/// `Player.removeSkill(skill)` returns before it deletes the shortcuts while
/// `getTransformationId() > 0`; a stance does not count.
#[test]
fn removing_a_skill_keeps_its_shortcuts_while_transformed() {
    let shortcut = ShortcutData {
        slot: 0,
        page: 0,
        kind: 2,
        shortcut_id: 9400,
        level: 1,
    };
    let mut s = Scene::new();
    s.world.player_mut(ME).unwrap().data.shortcuts = vec![shortcut];
    s.apply(9101);
    s.world.delete_skill_shortcuts(ME, 9400);
    assert_eq!(s.world.player(ME).unwrap().data.shortcuts.len(), 1);

    s.world.untransform(ME);
    s.apply(9104);
    s.world.delete_skill_shortcuts(ME, 9400);
    assert!(s.world.player(ME).unwrap().data.shortcuts.is_empty());
}

// ---- the inventory block in the item requests ---------------------------------------------------

/// `UseItem` and `RequestDestroyItem` refuse an item the block forbids (messages as in Java), and
/// accept it again once the form is gone.
#[test]
fn a_blocked_item_cannot_be_used_or_destroyed() {
    let thing = ItemData {
        object_id: ObjectId(0x3000_0002),
        item_id: 4000,
        count: 1,
        loc: "INVENTORY".into(),
        loc_data: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    };
    let mut s = Scene::build(1000, PlayerProfile::default(), vec![thing]);
    s.apply(9102);
    s.advance(400);
    s.me.take();
    let object_id = 0x3000_0002;

    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::UseItem {
            object_id,
            ctrl_pressed: false,
        },
    });
    let sent = s.me.take();
    assert_eq!(
        message_ids(&sent),
        [SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms],
        "{:?}",
        names(&sent)
    );

    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::DestroyItem { object_id, count: 1 },
    });
    let sent = s.me.take();
    let texts: Vec<SystemMessage> = of(&sent, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p))
        .collect();
    assert_eq!(texts.len(), 1, "{:?}", names(&sent));
    assert_eq!(
        texts[0].params,
        [l2_protocol_game::server::SystemMessageParam::Text(
            "You cannot use this item.".into()
        )]
    );
    assert!(
        s.world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .any(|i| i.item_id == 4000)
    );

    // Without the form the destroy goes through.
    s.world.untransform(ME);
    s.me.take();
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::DestroyItem { object_id, count: 1 },
    });
    assert!(
        !s.world
            .player(ME)
            .unwrap()
            .data
            .items
            .iter()
            .any(|i| i.item_id == 4000)
    );
}

/// `Player.sendInfo`: a transformed player coming into view sends its `CharInfo` twice ("Required
/// for showing mount transformations to players that just entered the game").
#[test]
fn a_transformed_player_comes_into_view_with_two_char_infos() {
    let mut s = Scene::new();
    s.apply(9101);
    s.advance(400);
    let p3 = Log::default();
    s.world.handle(WorldCommand::EnterWorld {
        session: SessionId(3),
        player: Box::new(player(ObjectId(3), 1060, &[])),
        sink: Box::new(Sink(p3.clone())),
        ip: None,
    });
    s.advance(1000);
    let seen = p3.take();
    let of_me: Vec<CharInfo> = of(&seen, "CHAR_INFO")
        .into_iter()
        .map(|p| decode::<CharInfo>(p))
        .filter(|i| i.object_id == ME.0)
        .collect();
    assert_eq!(of_me.len(), 2, "{:?}", names(&seen));
    assert!(of_me.iter().all(|i| i.transformation_display_id == 150));
}
