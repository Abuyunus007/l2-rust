#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The dispel, hate, movement and misc effect handlers (M5-EFX-03), end to end through the real
//! effect engine: a skill with the handler's `<effect>` is applied with `apply_effects`, and the
//! world's state is checked. The Java classes are named in each test. The doors as targets
//! (`OpenDoor`, `DetectHiddenObjects`) and the hate chances of M5-EFX-04 are at the end.

use std::sync::{Arc, Mutex};

use l2_data::stats::AbnormalType;
use l2_data::{
    ArmorSets, DoorTemplates, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::SystemMessage;
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

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
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

/// One skill per handler, each with the parameters the handler reads. The dispel skills take the
/// full chance (`minChance` and `maxChance` 100) so their rolls always land.
const SKILLS: &str = r##"<list>
<skill id="9001" levels="1" name="Might"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9010" levels="1" name="Herb of Defence"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>PD_UP</abnormalType><abnormalLevel>3</abnormalLevel>
<effects><effect name="Buff"><mul stat="pDef" val="1.1"/></effect></effects></skill>
<skill id="9040" levels="1" name="Weakness"><operateType>A2</operateType><targetType>SELF</targetType><isDebuff>true</isDebuff>
<abnormalTime>60</abnormalTime><abnormalType>POISON</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="0.5"/></effect></effects></skill>
<skill id="9100" levels="1" name="Dispel All"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="DispelAll"/></effects></skill>
<skill id="9101" levels="1" name="Dispel buffs"><operateType>A1</operateType><targetType>SELF</targetType>
<minChance>100</minChance><maxChance>100</maxChance>
<effects><effect name="DispelByCategory"><slot>buff</slot><rate>100</rate><max>10</max></effect></effects></skill>
<skill id="9102" levels="1" name="Dispel debuffs"><operateType>A1</operateType><targetType>SELF</targetType>
<minChance>100</minChance><maxChance>100</maxChance>
<effects><effect name="DispelByCategory"><slot>debuff</slot><rate>100</rate><max>10</max></effect></effects></skill>
<skill id="9103" levels="1" name="Dispel PD 2"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="DispelBySlot"><dispel>PD_UP,2</dispel></effect></effects></skill>
<skill id="9104" levels="1" name="Dispel PD 3"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="DispelBySlot"><dispel>PD_UP,3</dispel></effect></effects></skill>
<skill id="9105" levels="1" name="Dispel PD never"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="DispelBySlotProbability"><dispel>PD_UP</dispel><rate>0</rate></effect></effects></skill>
<skill id="9106" levels="1" name="Dispel PD always"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="DispelBySlotProbability"><dispel>PD_UP</dispel><rate>100</rate></effect></effects></skill>
<skill id="9107" levels="1" name="Steal buff"><operateType>A1</operateType><targetType>ONE</targetType>
<minChance>100</minChance><maxChance>100</maxChance>
<effects><effect name="StealAbnormal"><slot>buff</slot><rate>100</rate><max>1</max></effect></effects></skill>
<skill id="9110" levels="1" name="Hate 100"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="AddHate"><power>100</power></effect></effects></skill>
<skill id="9111" levels="1" name="Hate -40"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="AddHate"><power>-40</power></effect></effects></skill>
<skill id="9112" levels="1" name="Get agro"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="GetAgro"><chance>100</chance></effect></effects></skill>
<skill id="9120" levels="1" name="Teleport"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="Teleport"><x>1200</x><y>1300</y><z>0</z></effect></effects></skill>
<skill id="9121" levels="1" name="Escape"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="Escape"><escapeType>TOWN</escapeType></effect></effects></skill>
<skill id="9122" levels="1" name="Blink"><operateType>A1</operateType><targetType>SELF</targetType>
<flyRadius>100</flyRadius><flyCourse>0</flyCourse>
<effects><effect name="Blink"/></effects></skill>
<skill id="9123" levels="1" name="Throw up"><operateType>A1</operateType><targetType>ONE</targetType>
<flyRadius>100</flyRadius>
<effects><effect name="ThrowUp"/></effects></skill>
<skill id="9124" levels="1" name="Teleport to target"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="TeleportToTarget"/></effects></skill>
<skill id="9125" levels="1" name="Recovery"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="Recovery"/></effects></skill>
<skill id="9126" levels="1" name="Restoration"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="Restoration"><itemId>0</itemId><itemCount>1</itemCount></effect></effects></skill>
<skill id="9127" levels="1" name="Give SP"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="GiveSp"><sp>50</sp></effect></effects></skill>
<skill id="9128" levels="1" name="Vitality"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="VitalityPointUp"><value>10</value></effect></effects></skill>
<skill id="9129" levels="1" name="Apply skills"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="ApplySkillEffects"><skills>9001,1</skills></effect></effects></skill>
<skill id="9130" levels="1" name="Teach"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="SetSkill"><skillId>9001</skillId><skillLevel>1</skillLevel></effect></effects></skill>
<skill id="9131" levels="1" name="Detection"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="Detection"/></effects></skill>
<skill id="9132" levels="1" name="Mana drain"><operateType>T</operateType><targetType>SELF</targetType>
<effects><effect name="MpConsumePerLevel" ticks="2"><power>1</power></effect></effects></skill>
<skill id="9133" levels="1" name="Face"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="ChangeFace"><value>5</value></effect></effects></skill>
<skill id="9134" levels="1" name="Hair color"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="ChangeHairColor"><value>6</value></effect></effects></skill>
<skill id="9135" levels="1" name="Hair style"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="ChangeHairStyle"><value>7</value></effect></effects></skill>
<skill id="9136" levels="1" name="Hide"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>HIDE</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"/></effects></skill>
<skill id="9137" levels="1" name="Consume body"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="ConsumeBody"/></effects></skill>
<skill id="9200" levels="1" name="Open door"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="OpenDoor"><chance>100</chance></effect></effects></skill>
<skill id="9201" levels="1" name="Open door (item)"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="OpenDoor"><chance>100</chance><isItem>true</isItem></effect></effects></skill>
<skill id="9202" levels="1" name="Detect hidden objects"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="DetectHiddenObjects"/></effects></skill>
<skill id="9210" levels="1" name="Delete hate"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="DeleteHate"><chance>1000</chance></effect></effects></skill>
<skill id="9211" levels="1" name="Delete hate of me"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="DeleteHateOfMe"><chance>1000</chance></effect></effects></skill>
<skill id="9212" levels="1" name="Randomize hate"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="RandomizeHate"><chance>1000</chance></effect></effects></skill>
<skill id="9213" levels="1" name="Transfer hate"><operateType>A1</operateType><targetType>ONE</targetType>
<affectRange>1000</affectRange>
<effects><effect name="TransferHate"><chance>1000</chance></effect></effects></skill>
<skill id="9214" levels="1" name="Target me"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="TargetMeProbability"><chance>1000</chance></effect></effects></skill>
<skill id="9215" levels="1" name="Resurrection"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="Resurrection"><power>50</power></effect></effects></skill>
<skill id="9216" levels="1" name="Delete hate (never)"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="DeleteHate"><chance>-1000</chance></effect></effects></skill>
<skill id="9217" levels="1" name="Target me (never)"><operateType>A1</operateType><targetType>ONE</targetType>
<effects><effect name="TargetMeProbability"><chance>-1000</chance></effect></effects></skill>
</list>"##;

struct Data(StatData);

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
}

fn stat_data(skills: SkillTemplates) -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(skills),
    }
}

fn skills() -> SkillTemplates {
    SkillTemplates::from_xml(
        SKILLS,
        &l2_data::skill::EnchantSkillGroups::default(),
        &l2_data::SkillLoadOptions::default(),
    )
}

const ME: ObjectId = ObjectId(1);
const OTHER: ObjectId = ObjectId(2);

fn player(id: ObjectId, x: i32, y: i32, z: i32) -> PlayerData {
    PlayerData {
        object_id: id,
        account: "acc".into(),
        name: format!("P{}", id.0),
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
        x,
        y,
        z,
        heading: 0,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn enter(world: &mut World, log: &Log, session: u64, data: PlayerData) {
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(session),
        player: Box::new(data),
        sink: Box::new(Sink(log.clone())),
        ip: None,
    });
}

/// A world with the two players of the tests: `ME` at (1000, 1000) and `OTHER` at (1050, 1000).
/// Teleports land exactly (no random offset) unless a test says otherwise.
fn world() -> (World, Log) {
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(1), Box::new(NoOutbox));
    let mut settings = WorldSettings::default();
    settings.teleport.max_offset_on_teleport = 0;
    world.set_settings(settings);
    world.set_static_data(Box::new(Data(stat_data(skills()))));
    world.set_id_factory(Arc::new(IdFactory::new([]).unwrap()));
    enter(&mut world, &log, 1, player(ME, 1000, 1000, 0));
    enter(&mut world, &log, 2, player(OTHER, 1050, 1000, 0));
    advance(&mut world, 1000);
    log.take();
    (world, log)
}

/// Runs the world for `ms` milliseconds (50 ms ticks).
fn advance(world: &mut World, ms: u64) {
    for _ in 0..(ms / 50) {
        world.tick();
    }
}

/// Applies skill `id` (level 1) from `caster` to `target`.
fn cast(world: &mut World, caster: ObjectId, target: ObjectId, id: i32) {
    world.apply_effects(caster, target, SkillKey::new(id, 1));
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(packet: &[u8]) -> P {
    let skip = if packet[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&packet[skip..])).unwrap()
}

fn message_ids(packets: &[Vec<u8>]) -> Vec<SystemMessageId> {
    packets
        .iter()
        .filter(|p| name_of(p) == "SYSTEM_MESSAGE")
        .map(|p| decode::<SystemMessage>(p).id)
        .collect()
}

/// The skill ids of one queue of `id`.
fn queue(world: &World, id: ObjectId, kind: ListKind) -> Vec<i32> {
    world
        .effect_list(id)
        .map(|l| l.queue(kind).iter().map(|i| i.skill().id).collect())
        .unwrap_or_default()
}

fn position(world: &World, id: ObjectId) -> (i32, i32, i32) {
    world.position_of(id).unwrap()
}

/// A world with two attackable monsters of clan WOLF, 50 units apart at (1100, 1000) and
/// (1150, 1000). Returns the world and the ids of the monster 2000 and the monster 2002.
fn world_with_monsters() -> (World, ObjectId, ObjectId) {
    const TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="2000" type="Monster" level="20"><ai clanHelpRange="300"><clanList><clan>WOLF</clan></clanList></ai>
    <collision><radius normal="9"/><height normal="20"/></collision></npc>
  <npc id="2002" type="Monster" level="20"><ai clanHelpRange="300"><clanList><clan>WOLF</clan></clanList></ai>
    <collision><radius normal="9"/><height normal="20"/></collision></npc>
</list>"#;
    let dir = tempfile::tempdir().unwrap();
    let spawns_dir = dir.path().join("data/spawns");
    std::fs::create_dir_all(&spawns_dir).unwrap();
    std::fs::write(
        spawns_dir.join("a.xml"),
        r#"<?xml version="1.0"?><list enabled="true"><spawn><npc id="2000" x="1100" y="1000" z="0" heading="0"/><npc id="2002" x="1150" y="1000" z="0" heading="0"/></spawn></list>"#,
    )
    .unwrap();
    let npcs = Arc::new(NpcTemplates::from_xml(TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(dir.path(), &npcs, &SpawnLoadOptions::default()));
    let (mut world, _log) = world();
    world.set_npc_data(npcs, spawns);
    let _ = world.spawn_all_npcs(false);
    let of = |template: i32| {
        world
            .npc_ids()
            .into_iter()
            .find(|id| world.npc(*id).is_some_and(|n| n.npc_id == template))
            .expect("the monster spawned")
    };
    let (first, second) = (of(2000), of(2002));
    (world, first, second)
}

// ---- dispel ---------------------------------------------------------------------------------

#[test]
fn dispel_all_takes_every_buff_and_debuff_off_the_creature() {
    // Java DispelAll.onStart: `effected.stopAllEffects()` (the CancelReturn path is off).
    let (mut world, _log) = world();
    cast(&mut world, ME, ME, 9001);
    cast(&mut world, ME, ME, 9040);
    assert_eq!(queue(&world, ME, ListKind::Buffs), [9001]);
    assert_eq!(queue(&world, ME, ListKind::Debuffs), [9040]);
    cast(&mut world, ME, ME, 9100);
    assert!(queue(&world, ME, ListKind::Buffs).is_empty());
    assert!(queue(&world, ME, ListKind::Debuffs).is_empty());
}

#[test]
fn dispel_by_category_takes_buffs_or_debuffs_by_slot() {
    // Java DispelByCategory.onStart with `Formulas.calcCancelStealEffects`: a "buff" slot takes
    // buffs, a "debuff" slot takes debuffs.
    let (mut world, _log) = world();
    cast(&mut world, ME, ME, 9001);
    cast(&mut world, ME, ME, 9040);
    cast(&mut world, ME, ME, 9101);
    assert!(queue(&world, ME, ListKind::Buffs).is_empty());
    assert_eq!(
        queue(&world, ME, ListKind::Debuffs),
        [9040],
        "a debuff stays for the buff slot"
    );
    cast(&mut world, ME, ME, 9102);
    assert!(queue(&world, ME, ListKind::Debuffs).is_empty());
}

#[test]
fn dispel_by_slot_takes_the_buff_only_when_its_level_is_at_or_below_the_given_one() {
    // Java DispelBySlot.onStart: `value < 0 || value >= abnormalLevel` takes the buff.
    let (mut world, _log) = world();
    cast(&mut world, ME, ME, 9010);
    cast(&mut world, ME, ME, 9103); // PD_UP up to level 2: the level-3 buff stays.
    assert_eq!(queue(&world, ME, ListKind::Buffs), [9010]);
    cast(&mut world, ME, ME, 9104); // PD_UP at level 3: taken.
    assert!(queue(&world, ME, ListKind::Buffs).is_empty());
}

#[test]
fn dispel_by_slot_probability_rolls_each_type_against_the_rate() {
    // Java DispelBySlotProbability.normalCancel: `Rnd.get(100) < rate` per type.
    let (mut world, _log) = world();
    cast(&mut world, ME, ME, 9010);
    cast(&mut world, ME, ME, 9105); // rate 0: never.
    assert_eq!(queue(&world, ME, ListKind::Buffs), [9010]);
    cast(&mut world, ME, ME, 9106); // rate 100: always.
    assert!(queue(&world, ME, ListKind::Buffs).is_empty());
}

#[test]
fn steal_abnormal_moves_the_buff_of_a_player_to_the_caster() {
    // Java StealAbnormal.onStart: the stolen buff is applied again to the thief with the time it
    // had left, and the victim loses it.
    let (mut world, _log) = world();
    cast(&mut world, OTHER, OTHER, 9001);
    assert_eq!(queue(&world, OTHER, ListKind::Buffs), [9001]);
    cast(&mut world, ME, OTHER, 9107);
    assert!(queue(&world, OTHER, ListKind::Buffs).is_empty());
    assert_eq!(queue(&world, ME, ListKind::Buffs), [9001]);
    let stolen = world.effect_list(ME).unwrap().info_by_skill_id(9001).unwrap();
    assert!(stolen.abnormal_time() > 0, "the time left comes along");
    assert_eq!(stolen.effected(), ME);
}

// ---- hate -------------------------------------------------------------------------------------

#[test]
fn add_hate_raises_and_then_lowers_the_hate_of_an_attackable_for_the_caster() {
    // Java AddHate.onStart: a positive power is `addDamageHate(effector, 0, power)`, a negative one
    // `reduceHate(effector, -power)`, and only for an attackable.
    let (mut world, monster, _) = world_with_monsters();
    cast(&mut world, ME, monster, 9110);
    let hate = world
        .aggro_list(monster)
        .iter()
        .find(|a| a.attacker == ME)
        .unwrap()
        .hate;
    assert_eq!(hate, 100);
    // A negative power is `reduceHate(effector, -power)`, and `reduceHate` adds its amount: the
    // hate grows by 40 (Java's naming, kept as it is).
    cast(&mut world, ME, monster, 9111);
    let hate = world
        .aggro_list(monster)
        .iter()
        .find(|a| a.attacker == ME)
        .unwrap()
        .hate;
    assert_eq!(hate, 140);
}

#[test]
fn add_hate_does_nothing_to_a_player() {
    // Java AddHate.onStart: `if (!effected.isAttackable()) return`.
    let (mut world, _log) = world();
    cast(&mut world, ME, OTHER, 9110);
    assert!(world.effect_list(OTHER).is_none_or(|l| l.is_empty()));
}

#[test]
fn get_agro_calls_the_clan_members_in_range_to_help() {
    // Java GetAgro.onStart: the target attacks the caster, and every clan member that can move
    // within `clanHelpRange` gets `addDamageHate(effector, 1, 200)`.
    let (mut world, first, second) = world_with_monsters();
    cast(&mut world, ME, first, 9112);
    let helper = world
        .aggro_list(second)
        .iter()
        .find(|a| a.attacker == ME)
        .unwrap();
    assert_eq!(helper.hate, 200);
    assert_eq!(helper.damage, 1);
}

// ---- movement and teleports -----------------------------------------------------------------

#[test]
fn teleport_moves_the_creature_to_the_skill_point() {
    // Java Teleport.onStart: `teleToLocation(loc, true)`; with no random offset it lands exactly.
    let (mut world, _log) = world();
    cast(&mut world, ME, ME, 9120);
    let (x, y, _) = position(&world, ME);
    assert_eq!((x, y), (1200, 1300));
}

#[test]
fn escape_without_a_restart_point_changes_nothing() {
    // Java Escape.onStart: `getTeleToLocation(effected, TOWN)`. The test world has no map regions,
    // so there is no restart point and nothing moves (Java would throw here).
    let (mut world, _log) = world();
    cast(&mut world, ME, ME, 9121);
    assert_eq!(position(&world, ME), (1000, 1000, 0));
}

#[test]
fn blink_jumps_flyradius_away_from_the_heading() {
    // Java Blink.onStart: heading 0 and course 0 give `cos(PI) * radius` on x.
    let (mut world, _log) = world();
    cast(&mut world, ME, ME, 9122);
    assert_eq!(position(&world, ME), (900, 1000, 0));
}

#[test]
fn throw_up_throws_the_target_past_the_caster() {
    // Java ThrowUp.onStart: distance 50 from the caster plus the radius 100 gives offset 150 along
    // the line from the caster through the target: x = 1000 + 150.
    let (mut world, _log) = world();
    cast(&mut world, ME, OTHER, 9123);
    assert_eq!(position(&world, OTHER), (1150, 1000, 0));
}

#[test]
fn teleport_to_target_stands_25_units_behind_the_target() {
    // Java TeleportToTarget.onStart: the target faces heading 0 (east), so the point behind it is
    // 25 units west of it, at (1100 - 25, 1000).
    let (mut world, log) = world();
    enter(&mut world, &log, 4, player(ObjectId(4), 1100, 1000, 0));
    advance(&mut world, 1000);
    cast(&mut world, ME, ObjectId(4), 9124);
    assert_eq!(position(&world, ME), (1075, 1000, 0));
}

// ---- recovery, restoration, sp, vitality ----------------------------------------------------

#[test]
fn recovery_lowers_the_death_penalty_level_and_tells_the_player() {
    // Java Recovery.onStart -> Player.reduceDeathPenaltyBuffLevel: level 2 goes to 1 with the
    // "now level" message; level 1 goes to 0 with "has been lifted".
    let log = Log::default();
    let (mut world, _) = world();
    let mut data = player(ObjectId(3), 1500, 1500, 0);
    data.profile.death_penalty_level = 2;
    enter(&mut world, &log, 3, data);
    advance(&mut world, 1000);
    log.take();
    cast(&mut world, ObjectId(3), ObjectId(3), 9125);
    assert_eq!(
        world
            .player(ObjectId(3))
            .unwrap()
            .data
            .profile
            .death_penalty_level,
        1
    );
    assert!(message_ids(&log.take()).contains(&SystemMessageId::YourDeathPenaltyIsNowLevelS1));
    cast(&mut world, ObjectId(3), ObjectId(3), 9125);
    assert_eq!(
        world
            .player(ObjectId(3))
            .unwrap()
            .data
            .profile
            .death_penalty_level,
        0
    );
    assert!(message_ids(&log.take()).contains(&SystemMessageId::YourDeathPenaltyHasBeenLifted));
}

#[test]
fn restoration_without_an_item_reports_that_nothing_was_found() {
    // Java Restoration.onStart: `itemId <= 0 || itemCount <= 0` sends THERE_WAS_NOTHING_FOUND_INSIDE.
    let (mut world, log) = world();
    cast(&mut world, ME, ME, 9126);
    assert_eq!(
        message_ids(&log.take()),
        [SystemMessageId::ThereWasNothingFoundInside]
    );
}

#[test]
fn give_sp_gives_the_caster_sp() {
    // Java GiveSp.onStart: `effector.asPlayer().addExpAndSp(0, sp)`.
    let (mut world, _log) = world();
    cast(&mut world, ME, ME, 9127);
    assert_eq!(world.player(ME).unwrap().data.sp, 50);
}

#[test]
fn vitality_point_up_adds_vitality_points_to_the_player() {
    // Java VitalityPointUp.onStart: `updateVitalityPoints(value, false, false)`.
    let (mut world, _log) = world();
    // A new character has full vitality, which the gain cannot pass: start from 100.
    world.set_vitality_points(ME, 100, true);
    cast(&mut world, ME, ME, 9128);
    assert_eq!(world.vitality_points(ME), 110);
}

// ---- skills and appearance ------------------------------------------------------------------

#[test]
fn apply_skill_effects_applies_the_listed_skills_to_the_target() {
    // Java ApplySkillEffects.onStart: `skill.applyEffects(effector, effected)` for each entry.
    let (mut world, _log) = world();
    cast(&mut world, ME, OTHER, 9129);
    assert_eq!(queue(&world, OTHER, ListKind::Buffs), [9001]);
}

#[test]
fn set_skill_teaches_the_player_the_skill() {
    // Java SetSkill.onStart: `player.addSkill(skill, true)` and `sendSkillList()`.
    let (mut world, _log) = world();
    assert_eq!(world.stats(ME).unwrap().skill_level(9001), 0);
    cast(&mut world, ME, ME, 9130);
    assert_eq!(world.stats(ME).unwrap().skill_level(9001), 1);
}

#[test]
fn detection_leaves_a_visible_player_hidden_effect_alone() {
    // Java Detection.onStart: only an invisible target loses its Hide. A visible one keeps it.
    let (mut world, _log) = world();
    cast(&mut world, OTHER, OTHER, 9136);
    cast(&mut world, ME, OTHER, 9131);
    let hidden = world
        .effect_list(OTHER)
        .is_some_and(|l| l.info_by_abnormal_type(AbnormalType::Hide).is_some());
    assert!(hidden);
}

#[test]
fn mp_consume_per_level_switches_a_toggle_off_without_mp() {
    // Java MpConsumePerLevel.onActionTime: not enough MP ends the toggle (and tells the player).
    let (mut world, log) = world();
    let mut data = player(ObjectId(5), 1500, 1500, 0);
    data.level = 16; // The level factor of the consumption is (level - 1) / 7.5: zero at level 1.
    enter(&mut world, &log, 5, data);
    advance(&mut world, 1000);
    log.take();
    let _ = world.set_current_mp(ObjectId(5), 0.0, false);
    cast(&mut world, ObjectId(5), ObjectId(5), 9132);
    advance(&mut world, 2000);
    assert!(queue(&world, ObjectId(5), ListKind::Toggles).is_empty());
    assert!(message_ids(&log.take()).contains(&SystemMessageId::YourSkillWasDeactivatedDueToLackOfMp));
}

#[test]
fn change_face_and_hair_set_the_caster_appearance() {
    // Java ChangeFace / ChangeHairColor / ChangeHairStyle: `effector.getAppearance().set...`.
    let (mut world, _log) = world();
    cast(&mut world, ME, ME, 9133);
    cast(&mut world, ME, ME, 9134);
    cast(&mut world, ME, ME, 9135);
    let data = &world.player(ME).unwrap().data;
    assert_eq!((data.face, data.hair_color, data.hair_style), (5, 6, 7));
}

#[test]
fn consume_body_takes_a_dead_npc_away_at_once_without_start_effects() {
    // Java ConsumeBody.onStart: no start effects, so `endDecayTask()` runs now.
    let (mut world, monster, _) = world_with_monsters();
    let _ = world.do_die(monster, None);
    assert!(world.is_dead(monster));
    let count = world.npc_count();
    cast(&mut world, ME, monster, 9137);
    assert_eq!(world.npc_count(), count - 1);
}

// ---- doors as targets (M5-EFX-04: `Door extends Creature`) ----------------------------------------

/// A world with one door of the given `<door>` attributes, 20 units from `ME`; returns its object
/// id and the log of `ME`.
fn world_with_door(attributes: &str) -> (World, ObjectId, Log) {
    let xml = format!(
        r#"<list><door id="77" name="door77" height="150" baseHpMax="1000" pos="1020,1000,0"
            node1="1010,950" node2="1030,950" node3="1030,1050" node4="1010,1050" nodeZ="-10" {attributes}/></list>"#
    );
    let templates = DoorTemplates::from_xml(&xml).unwrap();
    let (mut world, log) = world();
    world
        .spawn_doors(&templates, &IdFactory::new([]).unwrap())
        .unwrap();
    let door = world.doors().next().unwrap().object_id;
    log.take();
    (world, door, log)
}

#[test]
fn open_door_opens_a_door_that_opens_by_skill() {
    // Java OpenDoor.onStart: `Rnd.get(100) < chance` and the door is closed: `door.openMe()`.
    let (mut world, door, _log) = world_with_door(r#"open_method="8""#);
    assert!(!world.doors().next().unwrap().open);
    cast(&mut world, ME, door, 9200);
    assert!(world.doors().next().unwrap().open);
}

#[test]
fn open_door_refuses_a_door_that_does_not_open_by_skill_unless_the_effect_is_for_an_item() {
    // Java OpenDoor.onStart: `!door.isOpenableBySkill() && !_isItem` tells THIS_DOOR_CANNOT_BE_UNLOCKED.
    let (mut world, door, log) = world_with_door(r#"open_method="1""#);
    cast(&mut world, ME, door, 9200);
    assert!(!world.doors().next().unwrap().open);
    assert!(message_ids(&log.take()).contains(&SystemMessageId::ThisDoorCannotBeUnlocked));
    // The item variant skips the check.
    cast(&mut world, ME, door, 9201);
    assert!(world.doors().next().unwrap().open);
}

#[test]
fn detect_hidden_objects_shows_a_stealth_door() {
    // Java DetectHiddenObjects.onStart: a stealth door gets mesh 1 and is targetable when its
    // open type is not 0.
    let (mut world, door, _log) = world_with_door(r#"stealth="true" targetable="false" open_method="1""#);
    assert!(!world.doors().next().unwrap().targetable);
    cast(&mut world, ME, door, 9202);
    let door = world.doors().next().unwrap();
    assert_eq!(door.mesh_index, 1);
    assert!(door.targetable);
}

// ---- the hate chances (M5-EFX-04) -----------------------------------------------------------------

fn hate_of(world: &mut World, npc: ObjectId, who: ObjectId) -> i64 {
    world.aggro_hating(npc, who)
}

#[test]
fn delete_hate_empties_the_aggro_list() {
    // Java DeleteHate.onStart: `clearAggroList()`, walking, intention active.
    let (mut world, monster, _) = world_with_monsters();
    world.add_damage_hate(monster, ME, 0, 50);
    world.add_damage_hate(monster, OTHER, 0, 20);
    cast(&mut world, ME, monster, 9210);
    assert!(world.aggro_list(monster).is_empty());
}

#[test]
fn a_hate_chance_that_fails_changes_nothing() {
    // `calcSuccess` is `Formulas.calcProbability`: a chance of -1000 never lands.
    let (mut world, monster, _) = world_with_monsters();
    world.add_damage_hate(monster, ME, 0, 50);
    cast(&mut world, ME, monster, 9216);
    assert_eq!(hate_of(&mut world, monster, ME), 50);
    world.set_target(OTHER, Some(monster));
    cast(&mut world, ME, OTHER, 9217);
    assert_eq!(world.target_of(OTHER), Some(monster));
}

#[test]
fn delete_hate_of_me_clears_only_the_casters_hate() {
    // Java DeleteHateOfMe.onStart: `stopHating(effector)` keeps the entry with hate 0.
    let (mut world, monster, _) = world_with_monsters();
    world.add_damage_hate(monster, ME, 0, 50);
    world.add_damage_hate(monster, OTHER, 0, 20);
    cast(&mut world, ME, monster, 9211);
    assert_eq!(hate_of(&mut world, monster, ME), 0);
    assert_eq!(hate_of(&mut world, monster, OTHER), 20);
    assert_eq!(world.aggro_list(monster).len(), 2);
}

#[test]
fn randomize_hate_hands_the_casters_hate_to_another_attacker() {
    // Java RandomizeHate.onStart: with one other attacker on the list, it gets all of the hate.
    let (mut world, monster, _) = world_with_monsters();
    world.add_damage_hate(monster, ME, 0, 50);
    world.add_damage_hate(monster, OTHER, 0, 20);
    cast(&mut world, ME, monster, 9212);
    assert_eq!(hate_of(&mut world, monster, ME), 0);
    assert_eq!(hate_of(&mut world, monster, OTHER), 70);
}

#[test]
fn randomize_hate_needs_somebody_else_on_the_list() {
    let (mut world, monster, _) = world_with_monsters();
    world.add_damage_hate(monster, ME, 0, 50);
    cast(&mut world, ME, monster, 9212);
    assert_eq!(hate_of(&mut world, monster, ME), 50);
}

#[test]
fn transfer_hate_moves_the_hate_of_every_nearby_attackable_to_the_target() {
    // Java TransferHate.onStart: each visible attackable in `affectRange` with hate for the caster
    // loses it (`reduceHate(effector, -hate)`) and gives it to the effected.
    let (mut world, first, second) = world_with_monsters();
    world.add_damage_hate(first, ME, 0, 50);
    world.add_damage_hate(second, ME, 0, 30);
    cast(&mut world, ME, OTHER, 9213);
    assert_eq!(hate_of(&mut world, first, ME), 0);
    assert_eq!(hate_of(&mut world, second, ME), 0);
    assert_eq!(hate_of(&mut world, first, OTHER), 50);
    assert_eq!(hate_of(&mut world, second, OTHER), 30);
}

#[test]
fn target_me_probability_makes_a_player_target_the_caster() {
    // Java TargetMeProbability.onStart: a playable that targets somebody else targets the caster
    // (a skill that is no debuff passes `checkPvpSkill`).
    let (mut world, monster, _) = world_with_monsters();
    world.set_target(OTHER, Some(monster));
    assert_eq!(world.target_of(OTHER), Some(monster));
    cast(&mut world, ME, OTHER, 9214);
    assert_eq!(world.target_of(OTHER), Some(ME));
}

#[test]
fn target_me_probability_from_an_npc_clears_the_target() {
    // NOTE(java-quirk): an NPC effector has no `asPlayer()`, so `setTarget(null)` runs.
    let (mut world, monster, other_monster) = world_with_monsters();
    world.set_target(OTHER, Some(other_monster));
    cast(&mut world, monster, OTHER, 9214);
    assert_eq!(world.target_of(OTHER), None);
}

// ---- Resurrection (the branch of an NPC caster) -----------------------------------------------------

#[test]
fn an_npc_caster_revives_a_dead_player_and_a_dead_monster() {
    // Java Resurrection.onStart: a caster that is no player cancels the decay task and calls
    // `effected.doRevive(percent)`.
    let (mut world, monster, other_monster) = world_with_monsters();
    world.set_dead(OTHER, true);
    assert!(world.is_dead(OTHER));
    cast(&mut world, monster, OTHER, 9215);
    assert!(!world.is_dead(OTHER));

    let _ = world.do_die(other_monster, None);
    assert!(world.is_dead(other_monster));
    cast(&mut world, monster, other_monster, 9215);
    assert!(!world.is_dead(other_monster));
}

#[test]
fn a_player_caster_only_proposes_the_resurrection() {
    // The player branch is `Player.reviveRequest` (a dialog): M5-EFX-05. Until then nothing happens.
    let (mut world, _log) = world();
    world.set_dead(OTHER, true);
    cast(&mut world, ME, OTHER, 9215);
    assert!(world.is_dead(OTHER));
}
