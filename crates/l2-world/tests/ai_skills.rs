#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! NPC skill use (M5-AI-03): the `AISkillScope` lists of a template and `AttackableAI`'s skill
//! branches, through the world with a recording client. The scenarios use small synthetic
//! templates and skills (they run in CI):
//! - the scope lists follow `NpcData` (`Everything` has one skill for each rule),
//! - a mage casts at a player in range, a monster that cannot move casts instead of attacking,
//! - an NPC heals itself when hurt, buffs itself while it is idle, uses a suicide skill below 30%
//!   HP, and the `Skill0n_*` parameters of the template gate its skills,
//! - a passive skill of the template is on from the spawn.
//!
//! The chances and orders are those of Java's `AttackableAI`; the world RNG is seeded, and the
//! scenarios run until the thing happens (they do not depend on a particular draw).
//!
//! Java: ai/AttackableAI.java, data/xml/NpcData.java, entity/actor/Npc.java (hasSkillChance).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::MagicSkillUse;
use l2_util::WorldRng;
use l2_world::ai::npc_skills::AiSkillScope;
use l2_world::stats::{BaseStatBonus, StatData};
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

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        Some(ItemStatic::unknown(item_id))
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

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

/// The `MagicSkillUse` packets in `sent` that `caster` cast, as `(skill id, target id)`.
fn casts_of(sent: &Sent, caster: ObjectId) -> Vec<(i32, i32)> {
    sent.iter()
        .filter(|(_, body)| name_of(body) == "MAGIC_SKILL_USE")
        .map(|(_, body)| decode::<MagicSkillUse>(body))
        .filter(|msu| msu.creature_id == caster.0)
        .map(|msu| (msu.skill_id, msu.target_id))
        .collect()
}

const CLASS: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>5000</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

const BOLT: i32 = 9101;
const SELF_HEAL: i32 = 9102;
const BUFF: i32 = 9103;
const SUICIDE: i32 = 9104;
const STUN: i32 = 9106;
const HEAL_OTHER: i32 = 9107;

/// One skill for each rule of `NpcData`, and for each branch of the think.
const SKILLS: &str = r##"<list>
<skill id="9101" levels="1" name="Bolt"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectRange>1100</effectRange><effectPoint>-92</effectPoint>
<hitTime>1000</hitTime><isMagic>1</isMagic><magicLevel>1</magicLevel><mpConsume>5</mpConsume>
<power>12</power><reuseDelay>2000</reuseDelay>
<effects><effect name="MagicalDamage" /></effects></skill>
<skill id="9102" levels="1" name="Self Heal"><operateType>A1</operateType><targetType>SELF</targetType>
<hitTime>500</hitTime><isMagic>1</isMagic><mpConsume>3</mpConsume><reuseDelay>2000</reuseDelay>
<effects><effect name="Heal"><param power="1000"/></effect></effects></skill>
<skill id="9103" levels="1" name="Battle Cry"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>500</hitTime><isMagic>1</isMagic><mpConsume>2</mpConsume>
<abnormalTime>3600</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9104" levels="1" name="Blast"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>40</castRange><effectRange>400</effectRange><affectRange>100</affectRange><effectPoint>-100</effectPoint>
<hitTime>500</hitTime><isMagic>0</isMagic><mpConsume>3</mpConsume><isSuicideAttack>true</isSuicideAttack>
<effects><effect name="PhysicalDamage"><param power="25"/></effect></effects></skill>
<skill id="9105" levels="1" name="Hardiness"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><mul stat="maxHp" val="2"/></effect></effects></skill>
<skill id="9106" levels="1" name="Paralyzing Gaze"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>40</castRange><effectRange>400</effectRange><hitTime>500</hitTime><isMagic>1</isMagic><mpConsume>3</mpConsume>
<abnormalTime>10</abnormalTime><abnormalType>STUN</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Stun"/></effects></skill>
<skill id="9107" levels="1" name="Heal Other"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectRange>1100</effectRange><effectPoint>100</effectPoint>
<hitTime>500</hitTime><isMagic>1</isMagic><mpConsume>3</mpConsume><reuseDelay>2000</reuseDelay>
<effects><effect name="Heal"><param power="1000"/></effect></effects></skill>
</list>"##;

/// The templates: 9201 does everything, 9202 a plain melee monster, 9203 a mage, 9204 a monster
/// that heals itself, 9205 one that buffs itself, 9206 one with a suicide skill, 9207 / 9208 mages
/// whose bolt is gated by a parameter (never / always), 9209 one with a passive skill.
const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="9201" type="Monster" level="1" name="Everything">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="500.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <skillList><skill id="9101" level="1"/><skill id="9102" level="1"/><skill id="9103" level="1"/><skill id="9104" level="1"/><skill id="9105" level="1"/><skill id="9106" level="1"/></skillList>
    <ai aggroRange="300"/>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9202" type="Monster" level="1" name="Plain">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="500.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="600"/>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9203" type="Monster" level="1" name="Mage">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="500.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <skillList><skill id="9101" level="1"/></skillList>
    <ai type="MAGE" aggroRange="600"><skill minChance="100" maxChance="100"/></ai>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9204" type="Monster" level="1" name="Healer">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="500.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <skillList><skill id="9102" level="1"/></skillList>
    <ai aggroRange="600"/>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9205" type="Monster" level="1" name="Buffer">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="500.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <skillList><skill id="9103" level="1"/></skillList>
    <ai aggroRange="300" isAggressive="false"/>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9206" type="Monster" level="1" name="Bomber">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="500.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <skillList><skill id="9104" level="1"/></skillList>
    <ai aggroRange="600"><skill minChance="100" maxChance="100"/></ai>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9207" type="Monster" level="1" name="Gated Mage">
    <parameters><param name="SetAggressiveTime" value="0" /><skill name="Skill01_ID" id="9101" level="1"/><param name="Skill01_Probability" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="500.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <skillList><skill id="9101" level="1"/></skillList>
    <ai type="MAGE" aggroRange="600"><skill minChance="100" maxChance="100"/></ai>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9208" type="Monster" level="1" name="Free Mage">
    <parameters><param name="SetAggressiveTime" value="0" /><skill name="Skill01_ID" id="9101" level="1"/><param name="Skill01_Probability" value="10000" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="500.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <skillList><skill id="9101" level="1"/></skillList>
    <ai type="MAGE" aggroRange="600"><skill minChance="100" maxChance="100"/></ai>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9210" type="Monster" level="1" name="Chief">
    <parameters><minions name="Privates"><npc id="9211" count="1" respawnTime="10"/></minions></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="0" mp="500.0" mpRegen="0" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="300" isAggressive="false"/>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9211" type="Monster" level="1" name="Medic">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="0" mp="500.0" mpRegen="0" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <skillList><skill id="9107" level="1"/></skillList>
    <ai aggroRange="600"/>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9209" type="Monster" level="1" name="Hardy">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="500.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <skillList><skill id="9105" level="1"/></skillList>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
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

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);

fn player_data(id: ObjectId, (x, y, z): (i32, i32, i32)) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
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
        cur_hp: 5000.0,
        max_hp: 5000.0,
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

struct Scene {
    world: World,
    log: Log,
    _dir: tempfile::TempDir,
}

impl Scene {
    /// A world with the synthetic templates and the given spawns (the body of a spawn file),
    /// spawned before anybody is online: the regions are off and nobody thinks.
    fn new(spawns: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let spawn_dir = dir.path().join("data/spawns");
        std::fs::create_dir_all(&spawn_dir).unwrap();
        std::fs::write(
            spawn_dir.join("test.xml"),
            format!(r#"<?xml version="1.0"?><list enabled="true">{spawns}</list>"#),
        )
        .unwrap();
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        let spawn_data = Arc::new(SpawnData::load(dir.path(), &npcs, &SpawnLoadOptions::default()));
        let mut world = World::new(WorldRng::from_seed(20_261_020), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(npcs, spawn_data);
        world.spawn_all_npcs(false);
        Self {
            world,
            log: Log::default(),
            _dir: dir,
        }
    }

    /// No spawn files: the NPCs are made with `spawn_npc_at`.
    fn empty() -> Self {
        Self::new("")
    }

    fn enter(&mut self, pos: (i32, i32, i32)) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data(ME, pos)),
            sink: Box::new(Sink(ME.0, self.log.clone())),
            ip: None,
        });
        self.log.take();
    }

    fn npc(&mut self, npc_id: i32, dx: i32) -> ObjectId {
        self.world.spawn_npc_at(npc_id, X + dx, Y, Z, 0).unwrap()
    }

    /// Runs `limit` ticks, or until `done`; the packets of the run.
    fn run_until(&mut self, limit: u32, mut done: impl FnMut(&World, &Sent) -> bool) -> (bool, Sent) {
        let mut sent = Vec::new();
        for _ in 0..limit {
            self.world.tick();
            sent.extend(self.log.take());
            if done(&self.world, &sent) {
                return (true, sent);
            }
        }
        (false, sent)
    }

    fn mp(&self, id: ObjectId) -> f64 {
        self.world.npc(id).unwrap().core.cur_mp
    }
}

fn ids(world: &mut World, npc: ObjectId, scope: AiSkillScope) -> Vec<i32> {
    let mut ids = world.ai_skill_ids(npc, scope);
    ids.sort_unstable();
    ids
}

// ------------------------------------------------------------------ scenarios

/// `NpcData` sorts the skills of a template into scopes by what they are: the table of the rules.
#[test]
fn the_scope_lists_follow_npcdata() {
    let mut s = Scene::empty();
    s.enter((X, Y, Z));
    let npc = s.npc(9201, 5000);
    let w = &mut s.world;
    // Every active skill that is not a suicide skill is GENERAL.
    assert_eq!(
        ids(w, npc, AiSkillScope::General),
        [BOLT, SELF_HEAL, BUFF, STUN],
        "GENERAL"
    );
    // A2 and not a debuff: BUFF.
    assert_eq!(ids(w, npc, AiSkillScope::Buff), [BUFF]);
    // Heal effect: HEAL (and nothing else but GENERAL).
    assert_eq!(ids(w, npc, AiSkillScope::Heal), [SELF_HEAL]);
    // A damage skill: ATTACK, UNIVERSAL and, by its cast range, LONG_RANGE.
    assert_eq!(ids(w, npc, AiSkillScope::Attack), [BOLT]);
    assert_eq!(ids(w, npc, AiSkillScope::Universal), [BOLT]);
    assert_eq!(ids(w, npc, AiSkillScope::LongRange), [BOLT]);
    // Stun: IMMOBILIZE, and SHORT_RANGE (cast range 40).
    assert_eq!(ids(w, npc, AiSkillScope::Immobilize), [STUN]);
    assert_eq!(ids(w, npc, AiSkillScope::ShortRange), [STUN]);
    // A suicide skill is only SUICIDE; a passive skill is in no list.
    assert_eq!(ids(w, npc, AiSkillScope::Suicide), [SUICIDE]);
    for scope in [
        AiSkillScope::Res,
        AiSkillScope::Cot,
        AiSkillScope::Debuff,
        AiSkillScope::Negative,
    ] {
        assert!(ids(w, npc, scope).is_empty(), "{scope:?}");
    }
    // An NPC without skills has no lists.
    let plain = s.npc(9202, 5100);
    assert!(s.world.ai_skill_ids(plain, AiSkillScope::General).is_empty());
}

/// A mage in range casts its bolt at the player (a long range skill: the mage need not walk),
/// pays for it, and does it again after the reuse delay. (The damage itself is the effect
/// handler's: `MagicalDamage` is M5-EFX-04.)
#[test]
fn a_mage_casts_at_a_player_in_range() {
    let mut s = Scene::empty();
    s.enter((X, Y, Z));
    let mage = s.npc(9203, 300);
    let mp_before = s.mp(mage);
    let (cast, sent) = s.run_until(200, |_, sent| !casts_of(sent, mage).is_empty());
    assert!(cast, "the mage never cast");
    assert_eq!(casts_of(&sent, mage)[0], (BOLT, ME.0));
    // The bolt is paid for when it lands.
    let (paid, _) = s.run_until(40, |w, _| w.npc(mage).unwrap().core.cur_mp < mp_before);
    assert!(paid, "the bolt costs MP");

    // And again (reuse delay 2 s) while the player is still there.
    let (again, sent) = s.run_until(400, |_, sent| casts_of(sent, mage).len() >= 2);
    assert!(again, "the mage cast only once: {:?}", casts_of(&sent, mage));
}

/// A monster without skills does not cast, whatever the player does.
#[test]
fn a_monster_without_skills_never_casts() {
    let mut s = Scene::empty();
    s.enter((X, Y, Z));
    let plain = s.npc(9202, 30);
    let (cast, sent) = s.run_until(300, |_, sent| !casts_of(sent, plain).is_empty());
    assert!(!cast, "{:?}", casts_of(&sent, plain));
    assert!(s.world.ai_skill_ids(plain, AiSkillScope::General).is_empty());
}

/// A hurt monster heals itself: each think has a `(100 - hp%) / 3` percent chance, so the
/// scenario runs until it happens.
#[test]
fn a_hurt_monster_heals_itself() {
    let mut s = Scene::empty();
    s.enter((X, Y, Z));
    // Next to the player, so it stands still and fights.
    let healer = s.npc(9204, 30);
    s.world.set_current_hp(healer, 600.0, false);
    let (cast, sent) = s.run_until(1500, |_, sent| !casts_of(sent, healer).is_empty());
    assert!(cast, "the monster never healed");
    assert_eq!(casts_of(&sent, healer)[0], (SELF_HEAL, healer.0));
    s.run_until(40, |_, _| false);
    let hp = s.world.npc(healer).unwrap().core.cur_hp;
    assert!(hp > 600.0, "the heal restored nothing: {hp}");
}

/// A monster at full health does not heal (the chance is `(100 - hp%) / 3` = 0).
#[test]
fn a_monster_at_full_health_does_not_heal() {
    let mut s = Scene::empty();
    s.enter((X, Y, Z));
    let healer = s.npc(9204, 30);
    let (cast, sent) = s.run_until(400, |_, sent| !casts_of(sent, healer).is_empty());
    assert!(!cast, "{:?}", casts_of(&sent, healer));
}

/// While nobody fights it, an idle monster buffs itself now and then (the BUFF loop of
/// `thinkActive`, tried when the 1-in-30 random walk roll hits), and does not repeat a buff it
/// has.
#[test]
fn an_idle_monster_buffs_itself() {
    let mut s = Scene::new(&format!(
        r#"<spawn><npc id="9205" x="{}" y="{}" z="{Z}" heading="0"/></spawn>"#,
        X + 100,
        Y
    ));
    let buffer = s.world.npc_ids()[0];
    s.enter((X, Y, Z));
    assert!(!s.world.is_affected_by_skill(buffer, BUFF));
    let (done, sent) = s.run_until(6000, |w, _| w.is_affected_by_skill(buffer, BUFF));
    assert!(done, "the monster never buffed itself");
    assert_eq!(casts_of(&sent, buffer)[0], (BUFF, buffer.0));
    // It is on now: the loop skips it, nothing is cast again while it lasts.
    let (again, sent) = s.run_until(1200, |_, sent| !casts_of(sent, buffer).is_empty());
    assert!(!again, "{:?}", casts_of(&sent, buffer));
}

/// A buff costs MP: a monster that cannot pay (`mp <= mpConsume`) skips it.
#[test]
fn a_monster_without_mp_does_not_buff() {
    let mut s = Scene::new(&format!(
        r#"<spawn><npc id="9205" x="{}" y="{}" z="{Z}" heading="0"/></spawn>"#,
        X + 100,
        Y
    ));
    let buffer = s.world.npc_ids()[0];
    // No regeneration: the monster stays broke.
    s.world.set_regen_settings(RegenSettings {
        mp_regen_multiplier: 0.0,
        ..RegenSettings::default()
    });
    s.enter((X, Y, Z));
    s.world.set_current_hp_mp(buffer, 3000.0, 1.0);
    let (done, sent) = s.run_until(3000, |w, _| w.is_affected_by_skill(buffer, BUFF));
    assert!(!done, "{:?}", casts_of(&sent, buffer));
}

/// Below 30% HP a monster with a suicide skill uses it on the creature it hates, in reach.
#[test]
fn a_monster_below_30_percent_uses_its_suicide_skill() {
    let mut s = Scene::empty();
    s.enter((X, Y, Z));
    let bomber = s.npc(9206, 30);
    // Above 30%: it fights with its sword, no cast.
    let (cast, _) = s.run_until(200, |_, sent| !casts_of(sent, bomber).is_empty());
    assert!(!cast, "it blew up while healthy");
    s.world.set_current_hp(bomber, 600.0, false);
    let (cast, sent) = s.run_until(400, |_, sent| !casts_of(sent, bomber).is_empty());
    assert!(cast, "the monster never used its suicide skill");
    assert_eq!(casts_of(&sent, bomber)[0], (SUICIDE, ME.0));
}

/// A monster that cannot move casts its attack skills instead of walking (`movementDisable`:
/// the ATTACK scope for a mage), at a target in the skill's range.
#[test]
fn an_immobilised_mage_casts_instead_of_walking() {
    let mut s = Scene::empty();
    s.enter((X, Y, Z));
    let mage = s.npc(9203, 300);
    s.world.set_immobilized(mage, true);
    let (cast, sent) = s.run_until(200, |_, sent| !casts_of(sent, mage).is_empty());
    assert!(cast, "the mage never cast");
    assert_eq!(casts_of(&sent, mage)[0], (BOLT, ME.0));
}

/// `Skill01_Probability` 0 (and no other hint) gates the skill off; 10000 lets it through.
#[test]
fn skill_parameters_gate_the_skill() {
    let mut gated = Scene::empty();
    gated.enter((X, Y, Z));
    let mage = gated.npc(9207, 300);
    let (cast, sent) = gated.run_until(300, |_, sent| !casts_of(sent, mage).is_empty());
    assert!(!cast, "the gated skill was cast: {:?}", casts_of(&sent, mage));

    let mut free = Scene::empty();
    free.enter((X, Y, Z));
    let mage = free.npc(9208, 300);
    let (cast, _) = free.run_until(300, |_, sent| !casts_of(sent, mage).is_empty());
    assert!(cast, "the free skill was never cast");
}

/// A passive skill of the template is on from the spawn: `maxHp` x2.
#[test]
fn a_passive_skill_of_the_template_is_on_at_spawn() {
    let mut s = Scene::empty();
    s.enter((X, Y, Z));
    let hardy = s.npc(9209, 5000);
    let plain = s.npc(9202, 5100);
    let max = |s: &Scene, id: ObjectId| s.world.stats(id).unwrap().max_hp();
    assert_eq!(max(&s, hardy), 2 * max(&s, plain));
    // Passives are never AI skills.
    assert!(s.world.ai_skill_ids(hardy, AiSkillScope::General).is_empty());
}

/// The lists are built once per template and shared: a second NPC of the same id adds no work
/// (the cache is by NPC id), and the NPC ids without skills are never looked up in it.
#[test]
fn the_lists_are_shared_by_template() {
    let mut s = Scene::empty();
    s.enter((X, Y, Z));
    let a = s.npc(9203, 5000);
    let b = s.npc(9203, 5100);
    assert_eq!(
        s.world.ai_skill_ids(a, AiSkillScope::LongRange),
        s.world.ai_skill_ids(b, AiSkillScope::LongRange)
    );
    assert_eq!(s.world.ai_skill_ids(a, AiSkillScope::LongRange), [BOLT]);
}

/// `setIntentionCast` (scripts, D-05) on an attackable: `thinkCast` walks into the skill's range
/// if needed, goes back to ACTIVE and casts at the cast target.
#[test]
fn think_cast_casts_at_the_cast_target() {
    let mut s = Scene::empty();
    s.enter((X, Y, Z));
    let mage = s.npc(9203, 300);
    s.log.take();
    s.world.set_intention_cast(mage, SkillKey::new(BOLT, 1), Some(ME));
    let sent = s.log.take();
    assert_eq!(casts_of(&sent, mage), [(BOLT, ME.0)]);
    assert!(s.world.is_casting_now(mage));
    assert_eq!(s.world.intention_of(mage), Intention::Active);
}

/// A minion with a heal skill for others heals its master when the master is hurt: it walks into
/// the skill's range if it must, and casts at the master (`thinkAttack`, HEAL, `isMinion`).
#[test]
fn a_minion_heals_its_hurt_master() {
    let mut s = Scene::new(&format!(
        r#"<spawn><npc id="9210" x="{}" y="{}" z="{Z}" heading="0"/></spawn>"#,
        X + 100,
        Y
    ));
    let chief = s
        .world
        .npc_ids()
        .into_iter()
        .find(|id| s.world.npc(*id).unwrap().npc_id == 9210)
        .unwrap();
    let medic = s
        .world
        .npc_ids()
        .into_iter()
        .find(|id| s.world.npc(*id).unwrap().npc_id == 9211)
        .unwrap();
    assert_eq!(s.world.minions_of(chief), [medic]);
    s.world.set_current_hp(chief, 600.0, false);
    s.enter((X, Y, Z));
    let (cast, sent) = s.run_until(600, |_, sent| !casts_of(sent, medic).is_empty());
    assert!(cast, "the medic never healed its master");
    assert_eq!(casts_of(&sent, medic)[0], (HEAL_OTHER, chief.0));
    s.run_until(40, |_, _| false);
    assert!(s.world.npc(chief).unwrap().core.cur_hp > 600.0);
}
