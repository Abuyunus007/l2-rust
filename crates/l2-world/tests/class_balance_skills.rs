#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The `ClassBalance.ini` skill tables (M5-FIX-03): each table reaches the formula or the effect
//! that Java multiplies it into. Every test runs one skill twice with the same seed, once with
//! the tables at 1.0 and once with one entry changed, so the draws are the same and the result
//! differs by exactly that factor. Expected behaviour comes from `mechanics/stats/Formulas.java`
//! and `dist/game/data/scripts/handlers/skill/effects/{Heal,EnergyDamage}.java`.
//!
//! The scene: two level 1 players of class 0 next to each other and monsters from a small
//! synthetic NPC table (the same scaffolding as `effects_damage_heal.rs`).

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillLoadOptions, SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::SystemMessage;
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, ClassBalance, StatData};
use l2_world::*;

// ---- the scene ----

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

const CLASS: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

fn npc_xml(id: i32, level: i32, hp: i32, mp: i32) -> String {
    npc_xml_of("Monster", id, level, hp, mp)
}

fn npc_xml_of(kind: &str, id: i32, level: i32, hp: i32, mp: i32) -> String {
    format!(
        r#"<npc id="{id}" type="{kind}" level="{level}" name="N{id}">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="{hp}" hpRegen="2" mp="{mp}.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>"#
    )
}

/// 9001: level 1 monster with 300 HP. 9010: level 20 with 400 MP (for the level penalty of
/// `ManaHealByLevel`). 9011: level 10 (within 9 levels of a level 1 player: `calcMagicDam`'s
/// half-damage rule). 9020: a raid boss. 9030: 100000 HP, to take a whole hit.
fn npcs() -> String {
    format!(
        "<?xml version=\"1.0\"?><list>{}{}{}{}{}</list>",
        npc_xml(9001, 1, 300, 40),
        npc_xml(9010, 20, 300, 400),
        npc_xml(9011, 10, 300, 40),
        npc_xml_of("RaidBoss", 9020, 1, 300, 40),
        npc_xml(9030, 1, 100_000, 40),
    )
}

fn stat_data(skills: Arc<SkillTemplates>) -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills,
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const P2: ObjectId = ObjectId(0x2000_0002);

fn player_data(id: ObjectId, level: i32, dx: i32) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: format!("P{}", id.0 & 0xF),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X + dx,
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
}

impl Scene {
    /// `cb` changes the class balance tables; `max_mcrit_rate` caps the magic critical rate (0
    /// makes it impossible).
    fn build(xml: &str, seed: u64, cb: impl FnOnce(&mut ClassBalance), max_mcrit_rate: i32) -> Self {
        let skills = Arc::new(SkillTemplates::from_xml(
            &format!("<list>{xml}</list>"),
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        ));
        let mut settings = WorldSettings::default();
        settings.stats.max_mcrit_rate = max_mcrit_rate;
        cb(&mut settings.stats.class_balance);
        let mut world = World::new(WorldRng::from_seed(seed), Box::new(NoOutbox));
        world.set_settings(settings);
        // No natural regeneration, so the numbers are the effects' alone.
        world.set_regen_settings(RegenSettings {
            hp_regen_multiplier: 0.0,
            mp_regen_multiplier: 0.0,
            cp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_static_data(Box::new(Data(stat_data(skills))));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(&npcs(), &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        let mut scene = Self {
            world,
            me: Log::default(),
        };
        for (id, dx) in [(ME, 0), (P2, 40)] {
            let log = if id == ME {
                scene.me.clone()
            } else {
                Log::default()
            };
            scene.world.handle(WorldCommand::EnterWorld {
                session: SessionId(u64::try_from(id.0).unwrap()),
                player: Box::new(player_data(id, 1, dx)),
                sink: Box::new(Sink(log)),
                ip: None,
            });
        }
        for _ in 0..20 {
            scene.world.tick();
        }
        scene.me.take();
        scene
    }

    fn npc(&mut self) -> ObjectId {
        self.world.spawn_npc_at(9030, X + 80, Y, Z, 0).unwrap()
    }

    fn apply(&mut self, effector: ObjectId, effected: ObjectId, id: i32) {
        self.world.apply_effects(effector, effected, SkillKey::new(id, 1));
    }

    /// HP and CP together (a hit on a player takes the CP first).
    fn life(&self, id: ObjectId) -> f64 {
        let core = self.world.stats(id).unwrap().core().clone();
        core.cur_hp + core.cur_cp
    }

    /// The ids of the system messages the first player got since the last call.
    fn messages(&self) -> Vec<SystemMessageId> {
        self.me
            .take()
            .iter()
            .filter(|p| name_of(p) == "SYSTEM_MESSAGE")
            .map(|p| decode::<SystemMessage>(p).id)
            .collect()
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

const BIG: f64 = 100_000.0;

fn strike(id: i32, effect: &str, extra: &str) -> String {
    format!(
        r#"<skill id="{id}" levels="1" name="S{id}"><operateType>A1</operateType><targetType>ONE</targetType>
        <isMagic>0</isMagic><castRange>40</castRange><power>100</power>{extra}
        <effects>{effect}</effects></skill>"#
    )
}

fn spell(id: i32, effect: &str, extra: &str) -> String {
    format!(
        r#"<skill id="{id}" levels="1" name="M{id}"><operateType>A1</operateType><targetType>ONE</targetType>
        <isMagic>1</isMagic><power>50</power>{extra}
        <effects>{effect}</effects></skill>"#
    )
}

/// The damage the skill does to `target` with the tables at 1.0 and with `cb` applied.
fn damage_ratio(
    xml: &str,
    seed: u64,
    cb: impl Fn(&mut ClassBalance),
    attacker: ObjectId,
    target: Option<ObjectId>,
    skill: i32,
) -> (f64, f64) {
    let run = |changed: bool| {
        let mut s = Scene::build(
            xml,
            seed,
            |t| {
                if changed {
                    cb(t);
                }
            },
            0,
        );
        let target = target.unwrap_or_else(|| s.npc());
        let before = s.life(target);
        s.apply(attacker, target, skill);
        before - s.life(target)
    };
    (run(false), run(true))
}

/// `changed` is `plain` times `factor`. Physical and magical skill damage is cut to an `int` in
/// Java (and the energy damage is not), so a whole point of rounding is allowed.
fn assert_factor(plain: f64, changed: f64, factor: f64) {
    assert!(plain > 0.0, "the plain hit did nothing");
    assert!(
        (changed - plain * factor).abs() <= 1.0 + factor,
        "{changed} is not {factor} times {plain}"
    );
}

// ---- the energy skills (EnergyDamage.java) -----------------------------------------------------

const ENERGY: &str =
    r#"<effect name="EnergyDamage"><power>10</power><criticalChance>0</criticalChance></effect>"#;

#[test]
fn energy_damage_reads_the_pve_damage_table_of_the_attacker_class() {
    let xml = strike(9701, ENERGY, "");
    let (plain, changed) = damage_ratio(&xml, 5, |t| t.pve_energy_skill_damage[0] = 2.0, ME, None, 9701);
    assert_factor(plain, changed, 2.0);
    // The PvP table is not used against a monster.
    let (plain, changed) = damage_ratio(&xml, 5, |t| t.pvp_energy_skill_damage[0] = 2.0, ME, None, 9701);
    assert_factor(plain, changed, 1.0);
}

#[test]
fn energy_damage_reads_the_pvp_tables_against_a_player() {
    let xml = strike(9701, ENERGY, "");
    // Damage by the attacker's class, defence by the target's class (both class 0 here).
    let (plain, changed) = damage_ratio(
        &xml,
        5,
        |t| t.pvp_energy_skill_damage[0] = 3.0,
        P2,
        Some(ME),
        9701,
    );
    assert_factor(plain, changed, 3.0);
    let (plain, changed) = damage_ratio(
        &xml,
        5,
        |t| t.pvp_energy_skill_defence[0] = 2.0,
        P2,
        Some(ME),
        9701,
    );
    assert_factor(plain, changed, 0.5);
    let (plain, changed) = damage_ratio(
        &xml,
        5,
        |t| t.pve_energy_skill_defence[0] = 2.0,
        P2,
        Some(ME),
        9701,
    );
    assert_factor(plain, changed, 1.0);
}

// ---- the physical skills (calcPhysDam, calcCrit) -----------------------------------------------

#[test]
fn a_physical_skill_reads_the_pve_damage_table() {
    let xml = strike(9702, r#"<effect name="PhysicalDamage"/>"#, "");
    let (plain, changed) = damage_ratio(&xml, 7, |t| t.pve_physical_skill_damage[0] = 2.0, ME, None, 9702);
    assert_factor(plain, changed, 2.0);
}

#[test]
fn a_physical_skill_reads_the_pvp_damage_table_against_a_player() {
    let xml = strike(9702, r#"<effect name="PhysicalDamage"/>"#, "");
    let (plain, changed) = damage_ratio(
        &xml,
        7,
        |t| t.pvp_physical_skill_damage[0] = 0.5,
        P2,
        Some(ME),
        9702,
    );
    assert_factor(plain, changed, 0.5);
}

#[test]
fn the_critical_chance_tables_decide_whether_a_physical_skill_crits() {
    // `baseCritRate` 1000 makes the rate certain; the table scales it before the roll.
    let xml = strike(
        9703,
        r#"<effect name="PhysicalDamage"/>"#,
        "<baseCritRate>1000</baseCritRate>",
    );
    let crits = |zero: bool| {
        (0..8)
            .filter(|seed| {
                let mut s = Scene::build(
                    &xml,
                    *seed,
                    |t| {
                        if zero {
                            t.pve_physical_skill_critical_chance[0] = 0.0;
                        }
                    },
                    0,
                );
                let npc = s.npc();
                s.apply(ME, npc, 9703);
                s.messages().contains(&SystemMessageId::C1LandedACriticalHit)
            })
            .count()
    };
    assert_eq!(crits(false), 8);
    assert_eq!(crits(true), 0);
}

#[test]
fn the_critical_damage_table_raises_a_critical_physical_skill() {
    let xml = strike(
        9703,
        r#"<effect name="PhysicalDamage"/>"#,
        "<baseCritRate>1000</baseCritRate>",
    );
    let (plain, changed) = damage_ratio(
        &xml,
        7,
        |t| t.pve_physical_skill_critical_damage[0] = 2.0,
        ME,
        None,
        9703,
    );
    assert_factor(plain, changed, 2.0);
}

// ---- the blows ---------------------------------------------------------------------------------

#[test]
fn a_blow_reads_the_blow_tables() {
    let xml = strike(
        9704,
        r#"<effect name="FatalBlow"/>"#,
        "<blowChance>1000</blowChance>",
    );
    let (plain, changed) = damage_ratio(&xml, 3, |t| t.pve_blow_skill_damage[0] = 2.0, ME, None, 9704);
    assert_factor(plain, changed, 2.0);
    let (plain, changed) = damage_ratio(&xml, 3, |t| t.pvp_blow_skill_damage[0] = 0.5, P2, Some(ME), 9704);
    assert_factor(plain, changed, 0.5);
    let (plain, changed) = damage_ratio(&xml, 3, |t| t.pvp_blow_skill_defence[0] = 2.0, P2, Some(ME), 9704);
    assert_factor(plain, changed, 0.5);
    // A blow does not read the physical skill table.
    let (plain, changed) = damage_ratio(&xml, 3, |t| t.pve_physical_skill_damage[0] = 2.0, ME, None, 9704);
    assert_factor(plain, changed, 1.0);
}

// ---- the magic skills (calcMagicDam, calcManaDam) ----------------------------------------------

#[test]
fn a_magic_skill_reads_the_magical_damage_and_defence_tables() {
    let xml = spell(9705, r#"<effect name="MagicalDamage"/>"#, "");
    let (plain, changed) = damage_ratio(&xml, 9, |t| t.pve_magical_skill_damage[0] = 2.0, ME, None, 9705);
    assert_factor(plain, changed, 2.0);
    // A weaker spell against a player, so the player survives the plain hit.
    let weak = xml.replace("<power>50</power>", "<power>5</power>");
    let (plain, changed) = damage_ratio(
        &weak,
        9,
        |t| t.pvp_magical_skill_damage[0] = 2.0,
        P2,
        Some(ME),
        9705,
    );
    assert_factor(plain, changed, 2.0);
    let (plain, changed) = damage_ratio(
        &weak,
        9,
        |t| t.pvp_magical_skill_defence[0] = 2.0,
        P2,
        Some(ME),
        9705,
    );
    assert_factor(plain, changed, 0.5);
}

#[test]
fn a_magic_skill_critical_damage_table_applies_to_a_critical_hit() {
    // A magic critical buff and an uncapped rate make every hit critical.
    let xml = format!(
        "{}{}",
        spell(9705, r#"<effect name="MagicalDamage"/>"#, ""),
        r#"<skill id="9706" levels="1" name="Crit"><operateType>A2</operateType><targetType>SELF</targetType>
        <abnormalTime>600</abnormalTime><abnormalType>MAGIC_CRITICAL_UP</abnormalType><abnormalLevel>1</abnormalLevel>
        <effects><effect name="Buff"><add stat="mCritRate" val="500"/></effect></effects></skill>"#
    );
    let run = |changed: bool| {
        let mut s = Scene::build(
            &xml,
            9,
            |t| {
                if changed {
                    t.pve_magical_skill_critical_damage[0] = 2.0;
                }
            },
            100_000,
        );
        s.apply(ME, ME, 9706);
        let npc = s.npc();
        s.apply(ME, npc, 9705);
        assert!(s.messages().contains(&SystemMessageId::MagicCriticalHit));
        BIG - s.life(npc)
    };
    assert_factor(run(false), run(true), 2.0);
}

// ---- Heal.java ---------------------------------------------------------------------------------

#[test]
fn heal_reads_the_player_healing_table_of_the_healer_class() {
    let xml = spell(9707, r#"<effect name="Heal"><power>20</power></effect>"#, "");
    let run = |multiplier: f32| {
        let mut s = Scene::build(&xml, 1, |t| t.player_healing_skill[0] = multiplier, 0);
        s.world.set_current_hp(ME, 1.0, false);
        s.apply(P2, ME, 9707);
        s.world.stats(ME).unwrap().core().cur_hp - 1.0
    };
    let (plain, doubled) = (run(1.0), run(2.0));
    assert_factor(plain, doubled, 2.0);
}

// ---- Formulas.calcSkillMastery -----------------------------------------------------------------

#[test]
fn skill_mastery_reads_the_chance_table_of_the_actor_class() {
    // `skillMastery` 1 rolls against the STR bonus, which a huge table entry makes certain: the
    // mastered buff lasts twice as long.
    let xml = r#"<skill id="9708" levels="1" name="Mastery"><operateType>A2</operateType><targetType>SELF</targetType>
        <abnormalTime>600</abnormalTime><abnormalType>MA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
        <effects><effect name="Buff"><add stat="skillMastery" val="1"/></effect></effects></skill>
        <skill id="9709" levels="1" name="Plain"><operateType>A2</operateType><targetType>SELF</targetType>
        <abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
        <effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>"#;
    let time_of = |multiplier: f32| {
        let mut s = Scene::build(xml, 1, |t| t.skill_mastery_chance[0] = multiplier, 0);
        s.apply(ME, ME, 9708);
        s.apply(ME, ME, 9709);
        s.world
            .effect_list(ME)
            .unwrap()
            .buffs()
            .iter()
            .find(|b| b.skill().id == 9709)
            .map(|b| b.abnormal_time())
            .unwrap()
    };
    assert_eq!(time_of(0.0), 60);
    assert_eq!(time_of(1_000_000.0), 120);
}
