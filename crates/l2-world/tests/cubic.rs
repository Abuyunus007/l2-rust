#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Cubics (M6-CUBIC-01): the `SummonCubic` and `CubicMastery` effects, the cubics in `UserInfo` and
//! `CharInfo`, the action task (target choice, skill choice, the attack, drain, debuff, cure and
//! heal casts), the heal cubic, the end of a cubic (duration, `maxCount` with `useUp`, death, the
//! owner leaving). Expected values follow the Java code named in each test (`Cubic.java`,
//! `CubicAction.java`, `CubicHeal.java`, `Formulas.calcMagicDam(Cubic, ..)`).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, CubicData, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, ItemTemplates, KarmaLoss,
    NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillTemplates, SkillTrees, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{CharInfo, MagicSkillUse, SystemMessage, UserInfo};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::effects::SkillFinishType;
use l2_world::effects::SkillKey;
use l2_world::effects::apply::ApplyMode;
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

/// The 92xx skills summon the cubics of `CUBICS`; 143 is the datapack's Cubic Mastery (level 1:
/// two cubics); 4049 (Cubic Drain), 4050 (Cubic Vampiric Touch shape), 4051 (Cubic Heal), 4052
/// (Poison), 4165 (Icy Air) and 5579 (Cubic Cure) have the shape of the datapack's skills with
/// the numbers made for one rule each; 9301 is a dispellable debuff.
const SKILLS: &str = r##"<list>
<skill id="9201" levels="1" name="Summon Storm Cubic"><operateType>A1</operateType><targetType>SELF</targetType>
<isMagic>1</isMagic>
<effects><effect name="SummonCubic">
	<cubicId>1</cubicId><cubicDelay>10</cubicDelay><cubicDuration>900</cubicDuration>
	<cubicMaxCount>2</cubicMaxCount><cubicSkillChance>100</cubicSkillChance><cubicPower>282</cubicPower>
</effect></effects></skill>
<skill id="9202" levels="1" name="Summon Life Cubic"><operateType>A1</operateType><targetType>SELF</targetType>
<isMagic>1</isMagic>
<effects><effect name="SummonCubic"><cubicId>3</cubicId><cubicDelay>5</cubicDelay><cubicDuration>900</cubicDuration></effect></effects></skill>
<skill id="9203" levels="1" name="Summon Vampiric Cubic"><operateType>A1</operateType><targetType>SELF</targetType>
<isMagic>1</isMagic>
<effects><effect name="SummonCubic"><cubicId>2</cubicId><cubicSkillChance>100</cubicSkillChance></effect></effects></skill>
<skill id="9204" levels="1" name="Summon Smart Cubic"><operateType>A1</operateType><targetType>SELF</targetType>
<isMagic>1</isMagic>
<effects><effect name="SummonCubic">
	<cubicId>10</cubicId><cubicSkillChance>100</cubicSkillChance>
</effect></effects></skill>
<skill id="9205" levels="1" name="Summon Viper Cubic"><operateType>A1</operateType><targetType>SELF</targetType>
<isMagic>1</isMagic>
<effects><effect name="SummonCubic"><cubicId>4</cubicId><cubicSkillChance>100</cubicSkillChance><cubicPower>10</cubicPower></effect></effects></skill>
<skill id="9206" levels="1" name="Short Cubic"><operateType>A1</operateType><targetType>SELF</targetType>
<isMagic>1</isMagic>
<effects><effect name="SummonCubic"><cubicId>1</cubicId><cubicDuration>20</cubicDuration><cubicSkillChance>100</cubicSkillChance></effect></effects></skill>
<skill id="9207" levels="1" name="Storm Cubic by level"><operateType>A1</operateType><targetType>SELF</targetType>
<isMagic>1</isMagic>
<effects><effect name="SummonCubic"><cubicId>1</cubicId></effect></effects></skill>
<skill id="143" levels="2" name="Cubic Mastery"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="CubicMastery"><cubicCount>2</cubicCount></effect></effects></skill>
<skill id="4049" levels="1" name="Cubic Drain"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectPoint>-100</effectPoint><magicLevel>1</magicLevel><power>600</power>
<activateRate>100</activateRate><isMagic>1</isMagic>
<effects><effect name="MagicalDamage"/></effects></skill>
<skill id="4050" levels="1" name="Cubic Vampiric Touch"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectPoint>-100</effectPoint><magicLevel>1</magicLevel><power>600</power>
<activateRate>100</activateRate><isMagic>1</isMagic>
<effects><effect name="HpDrain"><power>0.4</power></effect></effects></skill>
<skill id="4051" levels="1" name="Cubic Heal"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectPoint>100</effectPoint><isMagic>1</isMagic>
<effects><effect name="Heal"><power>40</power></effect></effects></skill>
<skill id="4052" levels="1" name="Poison"><operateType>A2</operateType><targetType>ONE</targetType>
<abnormalLevel>5</abnormalLevel><abnormalTime>30</abnormalTime><abnormalType>POISON</abnormalType>
<activateRate>-1</activateRate><effectPoint>-100</effectPoint><isDebuff>true</isDebuff><isMagic>1</isMagic>
<effects><effect name="DamOverTime" ticks="3"><power>5</power></effect></effects></skill>
<skill id="4165" levels="1" name="Icy Air"><operateType>A2</operateType><targetType>ONE</targetType>
<abnormalLevel>5</abnormalLevel><abnormalTime>15</abnormalTime><abnormalType>DOT_ATTR</abnormalType>
<activateRate>-1</activateRate><effectPoint>-100</effectPoint><isDebuff>true</isDebuff><isMagic>1</isMagic>
<effects><effect name="DamOverTime" ticks="3"><power>5</power></effect></effects></skill>
<skill id="5579" levels="1" name="Cubic Cure"><operateType>A1</operateType><targetType>SELF</targetType>
<effectPoint>408</effectPoint><isMagic>1</isMagic>
<effects><effect name="DispelByCategory"><slot>debuff</slot><rate>100</rate><max>10</max></effect></effects></skill>
<skill id="9302" levels="1" name="Freeze"><operateType>A2</operateType><targetType>ONE</targetType>
<abnormalTime>3600</abnormalTime><abnormalType>STUN</abnormalType><abnormalLevel>1</abnormalLevel>
<activateRate>-1</activateRate><effectPoint>-1</effectPoint><isDebuff>true</isDebuff>
<effects><effect name="Stun"/></effects></skill>
<skill id="9301" levels="1" name="Weakness"><operateType>A2</operateType><targetType>ONE</targetType>
<abnormalTime>60</abnormalTime><abnormalType>PA_DOWN</abnormalType><abnormalLevel>1</abnormalLevel>
<activateRate>-1</activateRate><effectPoint>-100</effectPoint><isDebuff>true</isDebuff>
<effects><effect name="Buff"><mul stat="pAtk" val="0.5"/></effect></effects></skill>
</list>"##;

/// Cubic 1 (storm): 2 uses then `useUp`; 2 (vampiric); 3 (life, heal); 4 (viper, a debuff);
/// 10 (smart: Icy Air on the target, the cure on the master first).
const CUBICS: &str = r##"<list>
<cubic id="1" level="1" slot="1" duration="900" delay="10" maxCount="30" useUp="1" power="1000.0" selectionMode="RANDOM" targetType="TARGET">
	<skills><skill id="4049" level="1" /></skills>
</cubic>
<cubic id="2" level="1" slot="2" duration="600" delay="7" maxCount="5" useUp="0" power="300.0" selectionMode="RANDOM" targetType="TARGET">
	<skills><skill id="4050" level="1" /></skills>
</cubic>
<cubic id="3" level="1" slot="3" duration="900" delay="5" maxCount="30" useUp="0" power="0.0" selectionMode="RANDOM" targetType="HEAL">
	<skills><skill id="4051" level="1" /></skills>
</cubic>
<cubic id="4" level="1" slot="4" duration="900" delay="10" maxCount="30" useUp="0" power="0.0" selectionMode="RANDOM" targetType="TARGET">
	<skills><skill id="4052" level="1" /></skills>
</cubic>
<cubic id="10" level="1" slot="10" duration="900" delay="10" maxCount="50" useUp="0" power="2106.0" selectionMode="RANDOM" targetType="BY_SKILL">
	<skills>
		<skill id="4165" level="1" target="TARGET" />
		<skill id="5579" level="1" target="MASTER" targetDebuff="true" priority="true" />
	</skills>
</cubic>
</list>"##;

/// 9002 is a monster that survives a few hits (the scene stuns it so that it does not fight the
/// owner, whose stance would start the cubics).
const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="9002" type="Monster" level="1" name="Tank">
    <race>FAIRY</race>
    <acquire exp="1000" sp="100" />
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="100000" hpRegen="0" mp="40.0" mpRegen="0" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc></list>"#;

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        Some(ItemStatic::unknown(item_id))
    }
    fn item_stackable(&self, _item_id: i32) -> bool {
        false
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        skill_id == 143
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

fn stat_data(skills: Arc<SkillTemplates>) -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills,
    }
}

fn test_skills() -> Arc<SkillTemplates> {
    Arc::new(SkillTemplates::from_xml(
        SKILLS,
        &l2_data::skill::EnchantSkillGroups::default(),
        &l2_data::SkillLoadOptions::default(),
    ))
}

fn experience() -> Arc<ExperienceTable> {
    let rows: String = (1..=5)
        .map(|level| format!(r#"<experience level="{level}" tolevel="{}" />"#, level * 100))
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(r#"<table maxLevel="5" maxPetLevel="5">{rows}</table>"#)).unwrap(),
    )
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const P2: ObjectId = ObjectId(0x2000_0002);

const STORM: i32 = 9201;
const LIFE: i32 = 9202;
const VAMPIRIC: i32 = 9203;
const SMART: i32 = 9204;
const VIPER: i32 = 9205;
const SHORT: i32 = 9206;
const BY_LEVEL: i32 = 9207;
const MASTERY: i32 = 143;
const WEAKNESS: i32 = 9301;

fn player_data(id: ObjectId, x: i32) -> PlayerData {
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
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    me: Log,
    p2: Log,
}

impl Scene {
    fn new() -> Self {
        Self::with(test_skills(), Arc::new(CubicData::from_xml(CUBICS).unwrap()))
    }

    fn with(skills: Arc<SkillTemplates>, cubics: Arc<CubicData>) -> Self {
        let mut world = World::new(WorldRng::from_seed(7), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data(stat_data(Arc::clone(&skills)))));
        world.set_regen_settings(RegenSettings {
            hp_regen_multiplier: 0.0,
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        world.set_exp_data(
            experience(),
            Arc::new(ExperienceLoss::default()),
            Arc::new(KarmaLoss::default()),
            Arc::new(SkillTrees::default()),
        );
        world.set_cubic_data(cubics);
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data(ME, X)),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let p2 = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(2),
            player: Box::new(player_data(P2, X + 300)),
            sink: Box::new(Sink(p2.clone())),
            ip: None,
        });
        let mut scene = Self { world, me, p2 };
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

    /// Time passes while the owner keeps fighting (the stance is refreshed every 5 s).
    fn fight(&mut self, ms: u64) {
        for i in 0..(ms / 50) {
            if i % 100 == 0 {
                self.world.add_attack_stance_task(ME);
            }
            self.world.tick();
        }
    }

    /// The skill's `SummonCubic` effect on the owner (`Skill.applyEffects`).
    fn summon(&mut self, skill: i32) {
        self.world.apply_effects(ME, ME, SkillKey::new(skill, 1));
    }

    /// A monster in range that has aggro on the owner, targeted by it.
    fn target_mob(&mut self, x: i32) -> ObjectId {
        let mob = self.world.spawn_npc_at(9002, x, Y, Z, 0).unwrap();
        self.world.add_damage_hate(mob, ME, 0, 100);
        self.world.apply_effects(ME, mob, SkillKey::new(9302, 1));
        assert!(self.world.is_affected_by_skill(mob, 9302));
        self.world.set_target(ME, Some(mob));
        mob
    }

    fn mob_hp(&self, mob: ObjectId) -> f64 {
        self.world.npc(mob).unwrap().core.cur_hp
    }

    fn hp(&self, id: ObjectId) -> f64 {
        self.world.player(id).unwrap().core.cur_hp
    }

    fn set_hp(&mut self, id: ObjectId, hp: f64) {
        self.world.set_current_hp(id, hp, false);
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(packet: &[u8]) -> P {
    let skip = if packet[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&packet[skip..])).unwrap()
}

fn of<'a>(packets: &'a [Vec<u8>], name: &str) -> Vec<&'a Vec<u8>> {
    packets.iter().filter(|p| name_of(p) == name).collect()
}

fn user_infos(packets: &[Vec<u8>]) -> Vec<UserInfo> {
    of(packets, "USER_INFO").into_iter().map(|p| decode(p)).collect()
}

/// The `MagicSkillUse` packets as `(skill id, level, target id)`.
fn skill_uses(packets: &[Vec<u8>]) -> Vec<(i32, i32, i32)> {
    of(packets, "MAGIC_SKILL_USE")
        .into_iter()
        .map(|p| decode::<MagicSkillUse>(p))
        .map(|u| (u.skill_id, u.skill_level, u.target_id))
        .collect()
}

fn message_ids(packets: &[Vec<u8>]) -> Vec<SystemMessageId> {
    of(packets, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p).id)
        .collect()
}

/// The ids of the owner's cubics.
fn ids(world: &World) -> Vec<i32> {
    world.cubics_of(ME).iter().map(|c| c.id).collect()
}

// ---- summoning ---------------------------------------------------------------------------------

/// `SummonCubic.onStart`: the cubic is added with the effect's numbers (`cubicDelay` seconds as
/// ms) and `broadcastUserInfo()` shows it: the owner's `UserInfo` lists the cubic id at once,
/// the `CharInfo` the others see follows 100 ms later.
#[test]
fn summon_cubic_adds_the_cubic_and_shows_it_in_user_and_char_info() {
    let mut s = Scene::new();
    s.summon(STORM);
    let cubic = s.world.cubic(ME, 1).expect("the storm cubic");
    assert_eq!(cubic.owner, ME);
    assert_eq!(
        (cubic.power, cubic.delay_ms, cubic.skill_chance, cubic.max_count),
        (282, 10_000, 100, 2)
    );
    assert!(!cubic.given_by_other);
    assert!(!cubic.is_acting(), "an attack cubic waits for the first fight");
    assert_eq!(cubic.template.skills[0].skill_id, 4049);
    assert_eq!(s.world.cubic_ids_of(ME), vec![1]);

    let mine = s.me.take();
    let infos = user_infos(&mine);
    assert_eq!(
        infos.len(),
        1,
        "{:?}",
        mine.iter().map(|p| name_of(p)).collect::<Vec<_>>()
    );
    assert_eq!(infos[0].cubics, vec![1]);
    // Nobody sees the cubic before the 100 ms of `broadcastCharInfo`.
    assert!(of(&s.p2.take(), "CHAR_INFO").is_empty());
    s.advance(150);
    let seen = s.p2.take();
    let chars: Vec<CharInfo> = of(&seen, "CHAR_INFO").into_iter().map(|p| decode(p)).collect();
    let mine = chars.iter().find(|c| c.object_id == ME.0).expect("P2 sees ME");
    assert_eq!(mine.cubics, vec![1]);
}

/// `new Cubic(..)`: a power, delay, max count or duration of 0 (or -1 for the count) come from the
/// template. The skill level is the template level; a level above 100 maps to the enchant routes
/// `((level - 100) / 7) + 8`.
#[test]
fn the_template_fills_what_the_effect_leaves_out() {
    let mut s = Scene::new();
    s.summon(VAMPIRIC);
    let cubic = s.world.cubic(ME, 2).unwrap();
    assert_eq!(
        (cubic.power, cubic.delay_ms, cubic.max_count, cubic.skill_chance),
        (300, 7_000, 5, 100)
    );
    // The disappear timer is the template's 600 s.
    s.advance(599_000);
    assert!(s.world.cubic(ME, 2).is_some());
    s.advance(2_000);
    assert!(s.world.cubic(ME, 2).is_none());

    // Enchant routes: skill level 115 -> template level ((115 - 100) / 7) + 8 = 10.
    let cubics = Arc::new(
        CubicData::from_xml(
            r#"<list>
<cubic id="1" level="1" slot="1" duration="900" delay="10" maxCount="30" useUp="1" power="1.0" targetType="TARGET"/>
<cubic id="1" level="10" slot="1" duration="900" delay="10" maxCount="30" useUp="1" power="2.0" targetType="TARGET"/>
</list>"#,
        )
        .unwrap(),
    );
    let skills = Arc::new(SkillTemplates::from_xml(
        r##"<list><skill id="9207" levels="115" name="Enchanted">
<operateType>A1</operateType><targetType>SELF</targetType><isMagic>1</isMagic>
<effects><effect name="SummonCubic"><cubicId>1</cubicId></effect></effects></skill></list>"##,
        &l2_data::skill::EnchantSkillGroups::default(),
        &l2_data::SkillLoadOptions::default(),
    ));
    let mut s = Scene::with(skills, cubics);
    s.world.apply_effects(ME, ME, SkillKey::new(BY_LEVEL, 2));
    // Level 2 is below 101: template level 2 does not exist, so nothing is summoned.
    assert!(s.world.cubic(ME, 1).is_none());
    s.world.apply_effects(ME, ME, SkillKey::new(BY_LEVEL, 1));
    assert_eq!(s.world.cubic(ME, 1).unwrap().power, 1);
    s.world.apply_effects(ME, ME, SkillKey::new(BY_LEVEL, 115));
    assert_eq!(s.world.cubic(ME, 1).unwrap().power, 2);
}

/// `SummonCubic`: the same cubic id again replaces the cubic (and its timers); a different id
/// takes a free place or, when `PlayerStat.getMaxCubicCount()` (1) is used up, the cubic that
/// was summoned first.
#[test]
fn a_new_cubic_replaces_its_own_id_or_the_oldest_one() {
    let mut s = Scene::new();
    s.summon(STORM);
    let first_timers = s.world.timers().len();
    s.summon(STORM);
    assert_eq!(ids(&s.world), vec![1]);
    assert_eq!(
        s.world.timers().len(),
        first_timers,
        "the old disappear timer is gone"
    );
    s.summon(VAMPIRIC);
    assert_eq!(ids(&s.world), vec![2], "the one place is given to the newest");
    s.summon(LIFE);
    assert_eq!(ids(&s.world), vec![3]);
}

/// `CubicMastery.onStart` sets `setMaxCubicCount(cubicCount)`, `onExit` takes it back to 1. With two
/// places the third summon takes the cubic summoned first, not the lowest id, and the ids are
/// listed ascending.
#[test]
fn cubic_mastery_raises_the_number_of_places() {
    let mut s = Scene::new();
    assert_eq!(s.world.max_cubic_count(ME), 1);
    s.world
        .apply_effects_with(ME, ME, SkillKey::new(MASTERY, 1), ApplyMode::PASSIVE);
    assert_eq!(s.world.max_cubic_count(ME), 2);
    s.summon(VAMPIRIC);
    s.summon(STORM);
    // Ascending by id, though 2 was summoned first.
    assert_eq!(ids(&s.world), vec![1, 2]);
    s.me.take();
    s.summon(LIFE);
    // The oldest (2) went.
    assert_eq!(ids(&s.world), vec![1, 3]);
    assert_eq!(user_infos(&s.me.take()).last().unwrap().cubics, vec![1, 3]);
    // The passive ends: one place again (the cubics stay until the next summon).
    s.world.stop_skill_effects(ME, SkillFinishType::Removed, MASTERY);
    assert_eq!(s.world.max_cubic_count(ME), 1);
    assert_eq!(ids(&s.world), vec![1, 3]);
    s.summon(VAMPIRIC);
    assert_eq!(
        ids(&s.world),
        vec![2],
        "all places are used: every older cubic goes"
    );
}

/// `CubicDisappear`: `cubicDuration` seconds after the summon the cubic goes and the owner's
/// `UserInfo` is sent again.
#[test]
fn the_cubic_disappears_after_its_duration() {
    let mut s = Scene::new();
    s.summon(SHORT);
    s.me.take();
    s.advance(19_900);
    assert!(s.world.cubic(ME, 1).is_some());
    assert!(user_infos(&s.me.take()).is_empty());
    s.advance(200);
    assert!(s.world.cubic(ME, 1).is_none());
    let infos = user_infos(&s.me.take());
    assert_eq!(infos.len(), 1);
    assert!(infos[0].cubics.is_empty());
    // Nothing is left on the wheel for it.
    s.advance(60_000);
    assert!(s.world.cubics_of(ME).is_empty());
}

/// The effect is for a player who lives; the one who casts it on somebody else gives the cubic
/// away (`givenByOther`), which `stopCubicsByOthers` takes off again.
#[test]
fn a_cubic_given_by_another_player_can_be_taken_off() {
    let mut s = Scene::new();
    s.world.apply_effects(P2, ME, SkillKey::new(STORM, 1));
    assert!(s.world.cubic(ME, 1).unwrap().given_by_other);
    s.summon(VAMPIRIC);
    assert!(!s.world.cubic(ME, 2).unwrap().given_by_other);
    // One place: the cubic given by P2 was replaced by the own one. With the mastery both stay.
    s.world
        .apply_effects_with(ME, ME, SkillKey::new(MASTERY, 1), ApplyMode::PASSIVE);
    s.world.apply_effects(P2, ME, SkillKey::new(STORM, 1));
    assert_eq!(ids(&s.world), vec![1, 2]);
    s.me.take();
    s.world.stop_cubics_by_others(ME);
    assert_eq!(ids(&s.world), vec![2]);
    assert_eq!(user_infos(&s.me.take()).len(), 1);
    // Nothing given: no packet.
    s.world.stop_cubics_by_others(ME);
    assert!(user_infos(&s.me.take()).is_empty());
}

// ---- the action task ---------------------------------------------------------------------------

/// `AttackStanceTaskManager.addAttackStanceTask` calls `cubic.doAction()` of a playable's cubics
/// (but not the life cubic's): until the owner fights, the attack cubic does nothing. A summon's
/// stance counts for its owner.
#[test]
fn an_attack_cubic_starts_with_the_first_fight() {
    let mut s = Scene::new();
    let mob = s.target_mob(X + 150);
    s.summon(STORM);
    s.advance(30_000);
    assert!(!s.world.cubic(ME, 1).unwrap().is_acting());
    assert_eq!(s.mob_hp(mob), 100_000.0);
    s.world.add_attack_stance_task(ME);
    assert!(s.world.cubic(ME, 1).unwrap().is_acting());
    // A creature that is no player's does not start anything.
    let mut other = Scene::new();
    let mob2 = other.target_mob(X + 150);
    other.summon(STORM);
    other.world.add_attack_stance_task(mob2);
    assert!(!other.world.cubic(ME, 1).unwrap().is_acting());
}

/// `Cubic.getCubicTarget` + `useCubicMdam`: the owner's target that has aggro on the owner is hit
/// for `91 * ((cubicPower + skillPower) / mDef)` (a magic critical hit triples it), the owner is
/// told the damage, and everybody in view sees `MagicSkillUse(owner, target, 4049, 1, 0, 0)`.
#[test]
fn an_attack_cubic_hits_the_target_with_the_cubic_formula() {
    let mut s = Scene::new();
    let mob = s.target_mob(X + 150);
    s.summon(STORM);
    s.world.add_attack_stance_task(ME);
    s.me.take();
    s.p2.take();
    s.advance(100);
    let dealt = 100_000.0 - s.mob_hp(mob);
    let m_def = s.world.stats(mob).unwrap().m_def(Some(ME), None);
    let normal = (91.0 * ((282.0 + 600.0) / m_def)) as i32;
    let crit = (91.0 * ((282.0 + 600.0) / m_def) * 3.0) as i32;
    assert!(
        dealt == f64::from(normal) || dealt == f64::from(crit),
        "dealt {dealt}, expected {normal} or {crit}"
    );
    assert_eq!(s.world.cubic(ME, 1).unwrap().action_count(), 1);
    assert_eq!(s.world.cubic(ME, 1).unwrap().target, Some(mob));
    assert_eq!(skill_uses(&s.me.take()), vec![(4049, 1, mob.0)]);
    assert_eq!(skill_uses(&s.p2.take()), vec![(4049, 1, mob.0)]);
}

/// `CubicAction`: no skill without a valid target — a monster without aggro on the owner, or
/// farther than `MAX_MAGIC_RANGE` (900) in 3D, is not hit.
#[test]
fn an_attack_cubic_needs_a_target_with_aggro_in_range() {
    let mut s = Scene::new();
    // No aggro: the monster is the owner's target but not hostile to it.
    let mob = s.world.spawn_npc_at(9002, X + 150, Y, Z, 0).unwrap();
    s.world.apply_effects(ME, mob, SkillKey::new(9302, 1));
    s.world.set_target(ME, Some(mob));
    s.summon(STORM);
    s.fight(25_000);
    assert_eq!(s.mob_hp(mob), 100_000.0);
    assert_eq!(s.world.cubic(ME, 1).unwrap().action_count(), 0);
    // With aggro but 901 away (x only): out of range; at 900 it is in.
    s.world.add_damage_hate(mob, ME, 0, 100);
    s.world.set_position(mob, X + 901, Y, Z);
    s.fight(12_000);
    assert_eq!(s.mob_hp(mob), 100_000.0);
    assert!(s.world.cubic(ME, 1).unwrap().target.is_none());
    s.world.set_position(mob, X + 900, Y, Z);
    s.fight(10_500);
    assert!(s.mob_hp(mob) < 100_000.0);
}

/// `CubicAction` stops when the owner is out of combat; a stance that comes later starts it again.
/// `maxCount` 2 with `useUp`: after two uses the third run stops the action and removes the cubic
/// (`removeCubic`, `broadcastUserInfo`).
#[test]
fn a_used_up_cubic_goes_after_max_count_uses() {
    let mut s = Scene::new();
    let mob = s.target_mob(X + 150);
    s.summon(STORM);
    s.world.add_attack_stance_task(ME);
    s.me.take();
    // The first run on the next tick, the second 10 s later.
    s.fight(10_500);
    assert_eq!(s.world.cubic(ME, 1).unwrap().action_count(), 2);
    let hp = s.mob_hp(mob);
    assert!(hp < 100_000.0);
    // The third run: `currentCount >= maxCount` -> stop, `useUp > 0` -> removed.
    s.fight(10_000);
    assert!(s.world.cubic(ME, 1).is_none());
    let infos = user_infos(&s.me.take());
    assert!(infos.last().unwrap().cubics.is_empty());
    assert_eq!(s.mob_hp(mob), hp, "no third hit");
}

/// Without a fight the action task stops itself on its next run (`hasAttackStanceTask` is false and
/// there is no summon): the cubic stays, and a later stance restarts it with a count of 0.
#[test]
fn the_action_stops_when_the_fight_is_over_and_restarts_with_the_next_one() {
    let mut s = Scene::new();
    s.target_mob(X + 150);
    s.summon(STORM);
    s.world.add_attack_stance_task(ME);
    s.advance(100);
    assert_eq!(s.world.cubic(ME, 1).unwrap().action_count(), 1);
    // The stance ends after 15 s; the run at 10 s still saw it, the one at 20 s does not.
    s.advance(21_000);
    let cubic = s.world.cubic(ME, 1).unwrap();
    assert!(!cubic.is_acting());
    assert!(cubic.target.is_none());
    s.world.add_attack_stance_task(ME);
    let cubic = s.world.cubic(ME, 1).unwrap();
    assert!(cubic.is_acting());
    assert_eq!(cubic.action_count(), 0, "a new CubicAction counts from 0");
}

/// `useCubicDrain`: the owner gets 40% of the damage back (up to the maximum) and the target
/// loses the damage; the skill's own `HpDrain` effect is not run for a cubic.
#[test]
fn a_drain_cubic_gives_the_owner_part_of_the_damage_back() {
    let mut s = Scene::new();
    let mob = s.target_mob(X + 150);
    s.set_hp(ME, 20.0);
    s.summon(VAMPIRIC);
    s.world.add_attack_stance_task(ME);
    s.advance(100);
    let dealt = 100_000.0 - s.mob_hp(mob);
    let m_def = s.world.stats(mob).unwrap().m_def(Some(ME), None);
    let normal = (91.0 * ((300.0 + 600.0) / m_def)) as i32;
    let crit = (91.0 * ((300.0 + 600.0) / m_def) * 3.0) as i32;
    assert!(
        dealt == f64::from(normal) || dealt == f64::from(crit),
        "dealt {dealt}"
    );
    // 20 + 0.4 * damage, at most 100.
    assert_eq!(s.hp(ME), (20.0 + 0.4 * dealt).min(100.0));
}

/// PvP: a flagged owner's cubic attacks a flagged enemy player (not a party member, not in a
/// peace zone); an enemy who is not flagged is left alone.
#[test]
fn a_flagged_owner_attacks_a_flagged_enemy() {
    let mut s = Scene::new();
    s.summon(VAMPIRIC);
    s.world.set_target(ME, Some(P2));
    s.world.update_pvp_status(ME);
    s.world.add_attack_stance_task(ME);
    // The enemy has no flag: not a valid target.
    s.advance(100);
    assert_eq!(s.hp(P2), 100.0);
    assert!(s.world.cubic(ME, 2).unwrap().target.is_none());
    // Flagged, the enemy is hit: the next run is 7 s later.
    s.world.update_pvp_status(P2);
    s.fight(7_500);
    assert!(s.hp(P2) < 100.0, "P2 has {}", s.hp(P2));
    assert_eq!(s.world.cubic(ME, 2).unwrap().target, Some(P2));
}

/// `useCubicContinuous`: a debuff skill that is not resisted (`activateRate` -1 always lands)
/// goes on the target through `applyCubicEffects` (a buff info with the owner as effector).
#[test]
fn a_debuff_cubic_puts_the_effect_on_the_target() {
    let mut s = Scene::new();
    let mob = s.target_mob(X + 150);
    s.summon(VIPER);
    s.world.add_attack_stance_task(ME);
    s.advance(100);
    assert!(s.world.is_affected_by_skill(mob, 4052));
    assert_eq!(s.world.cubic(ME, 4).unwrap().action_count(), 1);
    assert_eq!(skill_uses(&s.me.take()), vec![(4052, 1, mob.0)]);
}

/// `BY_SKILL` with a `priority` skill: the cure (target `MASTER`, `targetDebuff`) comes first on
/// every run, and only when the owner has a dispellable debuff; without one the normal skill is
/// tried (`Rnd` picks Icy Air). Priority uses count too.
#[test]
fn a_smart_cubic_cures_the_owner_before_its_normal_skill() {
    let mut s = Scene::new();
    let mob = s.target_mob(X + 150);
    s.summon(SMART);
    s.world.add_attack_stance_task(ME);
    s.world.apply_effects(mob, ME, SkillKey::new(WEAKNESS, 1));
    assert!(s.world.is_affected_by_skill(ME, WEAKNESS));
    s.me.take();
    s.advance(100);
    assert!(!s.world.is_affected_by_skill(ME, WEAKNESS), "cured");
    assert_eq!(skill_uses(&s.me.take()), vec![(5579, 1, ME.0)]);
    assert!(
        !s.world.is_affected_by_skill(mob, 4165),
        "the priority run was the whole run"
    );
    // The next run: nothing to cure, so the normal skill is used.
    s.fight(10_100);
    assert!(s.world.is_affected_by_skill(mob, 4165));
    assert_eq!(skill_uses(&s.me.take()), vec![(4165, 1, mob.0)]);
    assert_eq!(s.world.cubic(ME, 10).unwrap().action_count(), 2);
}

/// `Cubic.getCubicTarget` in a duel: only the other duelist is a target (the owner's target
/// being a monster gives none), and `useCubicContinuous` tells `DuelManager.onBuff` about a
/// negative effect on a duelist, so the duel takes the debuff off when it ends.
#[test]
fn in_a_duel_the_cubic_attacks_the_other_duelist_and_the_debuff_is_removed_at_the_end() {
    let mut s = Scene::new();
    let mob = s.target_mob(X + 150);
    s.summon(VIPER);
    // `RequestDuelStart`: the players may not be more than 250 apart.
    s.world.set_position(P2, X + 100, Y, Z);
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::DuelStart {
            name: format!("P{}", P2.0),
            party_duel: 0,
        },
    });
    s.world.handle(WorldCommand::Input {
        session: SessionId(2),
        input: ClientInput::DuelAnswerStart {
            party_duel: 0,
            response: 1,
        },
    });
    for _ in 0..400 {
        s.world.tick();
        if s.world.is_in_duel(ME) {
            break;
        }
    }
    assert!(s.world.is_in_duel(ME) && s.world.is_in_duel_with(ME, P2));
    // The monster that has aggro on the owner is no target of a duelist's cubic.
    s.world.add_attack_stance_task(ME);
    s.advance(200);
    assert!(s.world.cubic(ME, 4).unwrap().target.is_none());
    assert!(!s.world.is_affected_by_skill(mob, 4052));
    // The other duelist is.
    s.world.set_target(ME, Some(P2));
    s.fight(10_500);
    assert_eq!(s.world.cubic(ME, 4).unwrap().target, Some(P2));
    assert!(s.world.is_affected_by_skill(P2, 4052));
    // The duel ends (the owner gives up): the registered debuff goes with it.
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::DuelSurrender,
    });
    s.advance(10_000);
    assert!(!s.world.is_in_duel(ME));
    assert!(!s.world.is_affected_by_skill(P2, 4052));
}

/// `Player.stopCubics()`: all cubics go, with the owner's `UserInfo`; with none left it does
/// nothing (no packet).
#[test]
fn stop_cubics_removes_them_all_and_sends_the_user_info_once() {
    let mut s = Scene::new();
    s.world
        .apply_effects_with(ME, ME, SkillKey::new(MASTERY, 1), ApplyMode::PASSIVE);
    s.summon(STORM);
    s.summon(LIFE);
    let timers = s.world.timers().len();
    s.me.take();
    s.world.stop_cubics(ME);
    assert!(s.world.cubics_of(ME).is_empty());
    let infos = user_infos(&s.me.take());
    assert_eq!(infos.len(), 1);
    assert!(infos[0].cubics.is_empty());
    assert!(s.world.timers().len() < timers);
    s.world.stop_cubics(ME);
    assert!(user_infos(&s.me.take()).is_empty());
}

// ---- the heal cubic ----------------------------------------------------------------------------

/// `new Cubic(..)` starts a `HEAL` cubic at once (no fight needed); `CubicHeal` heals the owner
/// while it is hurt, from its own HP fraction, and sends `MagicSkillUse(owner, target, 4051, 1)`
/// after the heal. A healthy owner is left alone.
#[test]
fn a_heal_cubic_heals_the_hurt_owner() {
    let mut s = Scene::new();
    s.set_hp(ME, 50.0);
    s.summon(LIFE);
    assert!(s.world.cubic(ME, 3).unwrap().is_acting());
    s.me.take();
    s.advance(100);
    assert!(s.hp(ME) > 50.0, "healed to {}", s.hp(ME));
    assert_eq!(skill_uses(&s.me.take()), vec![(4051, 1, ME.0)]);
    assert_eq!(s.world.cubic(ME, 3).unwrap().action_count(), 1);
    // Full health: the following runs do nothing.
    s.set_hp(ME, 100.0);
    let count = s.world.cubic(ME, 3).unwrap().action_count();
    s.advance(20_000);
    assert_eq!(s.world.cubic(ME, 3).unwrap().action_count(), count);
    assert!(skill_uses(&s.me.take()).is_empty());
}

/// `cubicTargetForHeal` with a party: the member with the lowest HP fraction in range, not the
/// owner. A member out of range (more than 900 away in 3D) is skipped.
#[test]
fn a_heal_cubic_picks_the_party_member_with_the_lowest_hp_fraction() {
    let mut s = Scene::new();
    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::JoinParty {
            name: format!("P{}", P2.0),
            distribution_type_id: 0,
        },
    });
    s.world.handle(WorldCommand::Input {
        session: SessionId(2),
        input: ClientInput::AnswerJoinParty { response: 1 },
    });
    assert!(s.world.is_in_party_with(ME, P2));
    s.set_hp(ME, 50.0);
    s.set_hp(P2, 30.0);
    s.summon(LIFE);
    s.advance(100);
    assert_eq!(s.world.cubic(ME, 3).unwrap().target, Some(P2));
    assert!(s.hp(P2) > 30.0);
    assert_eq!(s.hp(ME), 50.0);
    // Now the owner is the one in need and P2 is out of range.
    s.set_hp(P2, 10.0);
    s.world.set_position(P2, X + 901, Y, Z);
    s.advance(5_100);
    assert_eq!(s.world.cubic(ME, 3).unwrap().target, Some(ME));
    assert!(s.hp(ME) > 50.0);
    assert_eq!(s.hp(P2), 10.0);
}

// ---- the end -----------------------------------------------------------------------------------

/// `Player.doDie`: "Unsummon Cubics" — the tasks end and the table is cleared, without a
/// `UserInfo` of its own.
#[test]
fn death_unsummons_the_cubics() {
    let mut s = Scene::new();
    let mob = s.target_mob(X + 150);
    s.summon(STORM);
    s.world
        .apply_effects_with(ME, ME, SkillKey::new(MASTERY, 1), ApplyMode::PASSIVE);
    s.summon(LIFE);
    s.world.add_attack_stance_task(ME);
    let timers = s.world.timers().len();
    assert_eq!(ids(&s.world), vec![1, 3]);
    s.world.reduce_current_hp(ME, 1_000_000.0, Some(mob));
    assert!(s.world.is_dead(ME));
    assert!(s.world.cubics_of(ME).is_empty());
    assert!(s.world.timers().len() < timers, "the cubic timers are cancelled");
    s.advance(30_000);
    assert!(s.world.cubics_of(ME).is_empty());
}

/// `Player.deleteMe`: `stopCubics()`; the player's stat (the number of places) goes with the
/// object, and nothing of the cubics stays on the wheel.
#[test]
fn leaving_the_world_removes_the_cubics_and_their_timers() {
    let mut s = Scene::new();
    s.world
        .apply_effects_with(ME, ME, SkillKey::new(MASTERY, 1), ApplyMode::PASSIVE);
    s.summon(STORM);
    s.summon(LIFE);
    let timers = s.world.timers().len();
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    assert!(s.world.cubics_of(ME).is_empty());
    assert_eq!(s.world.max_cubic_count(ME), 1);
    assert!(s.world.timers().len() < timers);
    s.advance(60_000);
    // The other player is unharmed by timers of a cubic that is gone.
    assert_eq!(s.hp(P2), 100.0);
}

/// `calcCubicSkillSuccess` says "Your attack has failed." for a negative skill that does not
/// land: a perfect shield block is the one outcome that needs no roll; here a debuff target that is
/// immune (`DEBUFF_IMMUNITY` > 0) is the other. The skill is not applied.
#[test]
fn a_resisted_debuff_tells_the_owner_and_applies_nothing() {
    let skills = Arc::new(SkillTemplates::from_xml(
        r##"<list>
<skill id="9201" levels="1" name="Summon Viper Cubic"><operateType>A1</operateType><targetType>SELF</targetType><isMagic>1</isMagic>
<effects><effect name="SummonCubic"><cubicId>4</cubicId><cubicSkillChance>100</cubicSkillChance></effect></effects></skill>
<skill id="9302" levels="1" name="Freeze"><operateType>A2</operateType><targetType>ONE</targetType>
<abnormalTime>3600</abnormalTime><abnormalType>STUN</abnormalType><abnormalLevel>1</abnormalLevel>
<activateRate>-1</activateRate><effectPoint>-1</effectPoint><isDebuff>true</isDebuff>
<effects><effect name="Stun"/></effects></skill>
<skill id="4052" levels="1" name="Poison"><operateType>A2</operateType><targetType>ONE</targetType>
<abnormalLevel>5</abnormalLevel><abnormalTime>30</abnormalTime><abnormalType>POISON</abnormalType>
<activateRate>40</activateRate><basicProperty>CON</basicProperty><effectPoint>-100</effectPoint><isDebuff>true</isDebuff><isMagic>1</isMagic>
<magicLevel>1</magicLevel>
<effects><effect name="DamOverTime" ticks="3"><power>5</power></effect></effects></skill>
</list>"##,
        &l2_data::skill::EnchantSkillGroups::default(),
        &l2_data::SkillLoadOptions::default(),
    ));
    let mut s = Scene::with(skills, Arc::new(CubicData::from_xml(CUBICS).unwrap()));
    let mob = s.target_mob(X + 150);
    s.summon(STORM);
    s.world.add_attack_stance_task(ME);
    // Over many runs a 40 rate (divided by the CON modifier) both lands and fails; every failure
    // is announced, every success applies the effect.
    let (mut failed, mut landed) = (0, 0);
    for _ in 0..25 {
        s.me.take();
        s.world.stop_skill_effects(mob, SkillFinishType::Removed, 4052);
        s.fight(10_000);
        let affected = s.world.is_affected_by_skill(mob, 4052);
        let told = message_ids(&s.me.take()).contains(&SystemMessageId::YourAttackHasFailed);
        assert_ne!(affected, told, "either it landed or the owner was told");
        if affected {
            landed += 1;
        } else {
            failed += 1;
        }
        // A cubic without a count limit would not stop; this one is the viper with 30 uses.
        if s.world.cubic(ME, 4).is_none() {
            break;
        }
    }
    assert!(landed > 0 && failed > 0, "landed {landed}, failed {failed}");
}

// ---- the datapack ------------------------------------------------------------------------------

fn game_root() -> Option<PathBuf> {
    let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")));
    if root.join("data/stats/skills").is_dir() && root.join("data/stats/cubics").is_dir() {
        Some(root)
    } else {
        eprintln!("skipped: {} has no datapack", root.display());
        None
    }
}

/// Every skill the datapack's cubics cast exists at the level named, and every `SummonCubic`
/// skill finds its template through Java's level rule (`cubicLvl`, else the skill level, with the
/// enchant routes mapped to `((level - 100) / 7) + 8`).
#[test]
fn the_datapack_cubics_and_skills_fit_together() {
    let Some(root) = game_root() else {
        return;
    };
    let cubics = CubicData::load(&root);
    let skills = SkillTemplates::load(&root, &l2_data::SkillLoadOptions::default());
    for id in 1..=14 {
        for level in 1..=12 {
            let Some(template) = cubics.get(id, level) else {
                continue;
            };
            for cs in &template.skills {
                assert!(
                    skills.get(cs.skill_id, cs.skill_level).is_some(),
                    "cubic {id} level {level}: skill {} level {} is missing",
                    cs.skill_id,
                    cs.skill_level
                );
            }
        }
    }
    let mut summoning = 0;
    for skill in skills.iter() {
        for scope in l2_data::skill::EffectScope::ALL.iter().copied() {
            for effect in skill.effects.get(scope) {
                if effect.name != "SummonCubic" {
                    continue;
                }
                summoning += 1;
                let param = |key: &str| effect.param(key).and_then(|t| t.parse::<i32>().ok());
                let cubic_id = param("cubicId").expect("cubicId");
                let cubic_level = param("cubicLvl")
                    .or_else(|| param("cubicSkillLevel"))
                    .unwrap_or(0);
                let mut level = if cubic_level > 0 { cubic_level } else { skill.level };
                if cubic_level <= 0 && level > 100 {
                    level = ((skill.level - 100) / 7) + 8;
                }
                assert!(
                    cubics.get(cubic_id, level).is_some(),
                    "skill {} level {}: no template for cubic {cubic_id} level {level}",
                    skill.id,
                    skill.level
                );
            }
        }
    }
    assert!(summoning > 50, "only {summoning} SummonCubic effects found");
}
