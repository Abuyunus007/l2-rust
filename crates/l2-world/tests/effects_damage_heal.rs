#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The damage and healing effect handlers (M5-EFX-01), each on a real skill of the datapack
//! (skipped when `C:\l2-rust\l2jmobius` is missing) and, where a rule needs exact numbers, on a
//! small synthetic skill. Expected behaviour comes from the Java handlers in
//! `dist/game/data/scripts/handlers/skill/effects/`.
//!
//! The scene: two level 1 players (100 HP, 50 MP, 20 CP before the base stat bonuses) next to
//! each other, and monsters from a small synthetic NPC table.

use std::path::PathBuf;
use std::sync::{Arc, Mutex, OnceLock};

use l2_data::skill::{EffectScope, EnchantSkillGroups};
use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillLoadOptions, SkillTemplate, SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{ExRegenMax, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- the scene -----------------------------------------------------------------------------------

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
    p2: Log,
}

impl Scene {
    fn new(skills: Arc<SkillTemplates>) -> Self {
        Self::seeded(skills, 20_261_020)
    }

    /// A scene whose world RNG starts at `seed`: two scenes with one seed make the same draws.
    fn seeded(skills: Arc<SkillTemplates>, seed: u64) -> Self {
        Self::build(skills, seed, WorldSettings::default())
    }

    fn build(skills: Arc<SkillTemplates>, seed: u64, settings: WorldSettings) -> Self {
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
            p2: Log::default(),
        };
        let (me, p2) = (scene.me.clone(), scene.p2.clone());
        scene.enter(ME, me, 0);
        scene.enter(P2, p2, 40);
        scene.advance(1000);
        scene.me.take();
        scene.p2.take();
        scene
    }

    /// A scene with the skills of an inline XML.
    fn inline(xml: &str) -> Self {
        Self::inline_seeded(xml, 20_261_020)
    }

    fn inline_seeded(xml: &str, seed: u64) -> Self {
        Self::build(inline_table(xml), seed, WorldSettings::default())
    }

    fn enter(&mut self, id: ObjectId, log: Log, dx: i32) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id.0).unwrap()),
            player: Box::new(player_data(id, 1, dx)),
            sink: Box::new(Sink(log)),
            ip: None,
        });
    }

    fn npc(&mut self, npc_id: i32) -> ObjectId {
        self.world.spawn_npc_at(npc_id, X + 80, Y, Z, 0).unwrap()
    }

    /// Runs the world for `ms` milliseconds (50 ms ticks).
    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn apply(&mut self, effector: ObjectId, effected: ObjectId, skill: SkillKey) {
        self.world.apply_effects(effector, effected, skill);
    }

    fn hp(&self, id: ObjectId) -> f64 {
        self.world.stats(id).unwrap().core().cur_hp
    }

    fn mp(&self, id: ObjectId) -> f64 {
        self.world.stats(id).unwrap().core().cur_mp
    }

    fn cp(&self, id: ObjectId) -> f64 {
        self.world.stats(id).unwrap().core().cur_cp
    }
}

fn inline_table(xml: &str) -> Arc<SkillTemplates> {
    Arc::new(SkillTemplates::from_xml(
        &format!("<list>{xml}</list>"),
        &EnchantSkillGroups::default(),
        &SkillLoadOptions::default(),
    ))
}

// ---- the datapack --------------------------------------------------------------------------------

fn datapack() -> Option<Arc<SkillTemplates>> {
    static TABLE: OnceLock<Option<Arc<SkillTemplates>>> = OnceLock::new();
    TABLE
        .get_or_init(|| {
            let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
                .map(PathBuf::from)
                .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
            if !root.join("data/stats/skills").is_dir() {
                eprintln!("skipped: {} has no datapack", root.display());
                return None;
            }
            Some(Arc::new(SkillTemplates::load(
                &root,
                &SkillLoadOptions::default(),
            )))
        })
        .clone()
}

/// The `power`-like parameter of the first effect named `handler` in the skill's general effects.
fn param(skill: &SkillTemplate, handler: &str, name: &str) -> f64 {
    skill
        .effects
        .get(EffectScope::General)
        .iter()
        .find(|e| e.name == handler)
        .and_then(|e| e.param(name))
        .unwrap_or_else(|| panic!("skill {} has no {handler}.{name}", skill.id))
        .parse()
        .unwrap()
}

/// The first skill (by id and level) with a general `handler` effect that `keep` accepts.
fn find<'a>(
    table: &'a SkillTemplates,
    handler: &str,
    keep: impl Fn(&SkillTemplate) -> bool,
) -> &'a SkillTemplate {
    table
        .iter()
        .find(|s| {
            s.effects
                .get(EffectScope::General)
                .iter()
                .any(|e| e.name == handler)
                && keep(s)
        })
        .unwrap_or_else(|| panic!("no skill with {handler}"))
}

fn key(skill: &SkillTemplate) -> SkillKey {
    SkillKey::new(skill.id, skill.level)
}

// ---- reading what was sent -----------------------------------------------------------------------

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(packet: &[u8]) -> P {
    let skip = if packet[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&packet[skip..])).unwrap()
}

fn messages(packets: &[Vec<u8>]) -> Vec<SystemMessage> {
    packets
        .iter()
        .filter(|p| name_of(p) == "SYSTEM_MESSAGE")
        .map(|p| decode::<SystemMessage>(p))
        .collect()
}

/// The system messages of a log as `(id, params)`, without the "effect can be felt" noise.
fn restored(packets: &[Vec<u8>]) -> Vec<(SystemMessageId, Vec<SystemMessageParam>)> {
    messages(packets)
        .into_iter()
        .filter(|m| m.id != SystemMessageId::S1SEffectCanBeFelt)
        .map(|m| (m.id, m.params))
        .collect()
}

fn int(value: i32) -> SystemMessageParam {
    SystemMessageParam::IntNumber(value)
}

fn name(text: &str) -> SystemMessageParam {
    SystemMessageParam::Text(text.to_owned())
}

// ---- HP heals --------------------------------------------------------------------------------------

#[test]
fn heal_percent_revival_fills_the_hp_and_tells_the_amount() {
    let Some(table) = datapack() else { return };
    // 181 Revival: `HealPercent` power 100.
    let skill = table.get(181, 1).unwrap().clone();
    assert_eq!(param(&skill, "HealPercent", "power"), 100.0);
    let mut s = Scene::new(table);
    s.world.set_current_hp(ME, 10.0, false);
    let max = s.world.stats(ME).unwrap().max_hp();
    s.me.take();
    s.apply(ME, ME, key(&skill));
    assert_eq!(s.hp(ME), f64::from(max));
    // `S1_HP_HAS_BEEN_RESTORED` with (int) amount.
    assert_eq!(
        restored(&s.me.take()),
        [(SystemMessageId::S1HpHasBeenRestored, vec![int(max - 10)])]
    );
}

#[test]
fn heal_percent_from_another_player_names_the_healer() {
    let Some(table) = datapack() else { return };
    let skill = table.get(181, 1).unwrap().clone();
    let mut s = Scene::new(table);
    s.world.set_current_hp(ME, 40.0, false);
    let max = s.world.stats(ME).unwrap().max_hp();
    s.me.take();
    s.apply(P2, ME, key(&skill));
    assert_eq!(
        restored(&s.me.take()),
        [(
            SystemMessageId::S2HpHasBeenRestoredByC1,
            vec![name("P2"), int(max - 40)]
        )]
    );
}

#[test]
fn heal_percent_takes_a_share_of_the_maximum() {
    let mut s = Scene::inline(
        r#"<skill id="9101" levels="1" name="Quarter"><operateType>A1</operateType><targetType>SELF</targetType>
        <effects><effect name="HealPercent"><power>25</power></effect></effects></skill>"#,
    );
    s.world.set_current_hp(ME, 1.0, false);
    let max = f64::from(s.world.stats(ME).unwrap().max_hp());
    s.apply(ME, ME, SkillKey::new(9101, 1));
    // `(maxHp * power) / 100.0`, no rounding.
    assert_eq!(s.hp(ME), 1.0 + (max * 25.0) / 100.0);
}

#[test]
fn a_dead_target_is_not_healed() {
    let mut s = Scene::inline(
        r#"<skill id="9101" levels="1" name="Quarter"><operateType>A1</operateType><targetType>SELF</targetType>
        <effects><effect name="HealPercent"><power>25</power></effect></effects></skill>"#,
    );
    s.world.set_dead(ME, true);
    s.me.take();
    s.apply(ME, ME, SkillKey::new(9101, 1));
    assert!(s.me.take().is_empty());
}

#[test]
fn hp_by_level_gives_the_effector_hp_up_to_the_maximum() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "HpByLevel", |_| true).clone();
    let power = param(&skill, "HpByLevel", "power");
    let mut s = Scene::new(table);
    s.world.set_current_hp(ME, 20.0, false);
    let max = f64::from(s.world.stats(ME).unwrap().max_hp());
    s.me.take();
    // The HP goes to the effector (ME) even though P2 is the effected.
    s.apply(ME, P2, key(&skill));
    let expected = (20.0 + power).min(max);
    assert_eq!(s.hp(ME), expected);
    assert_eq!(
        restored(&s.me.take()),
        [(
            SystemMessageId::S1HpHasBeenRestored,
            vec![int((expected - 20.0) as i32)]
        )]
    );
}

#[test]
fn heal_over_time_heals_each_tick_by_power_times_the_tick_multiplier() {
    let mut s = Scene::inline(
        r#"<skill id="9102" levels="1" name="Regen"><operateType>A2</operateType><targetType>SELF</targetType>
        <abnormalTime>60</abnormalTime><abnormalType>HP_RECOVER</abnormalType><abnormalLevel>1</abnormalLevel>
        <effects><effect name="HealOverTime" ticks="5"><power>5</power></effect></effects></skill>"#,
    );
    s.world.set_current_hp(ME, 10.0, false);
    s.me.take();
    s.apply(ME, ME, SkillKey::new(9102, 1));
    // HP_RECOVER and ticks > 0: the client gets its regeneration bar.
    let sent = s.me.take();
    let bars: Vec<ExRegenMax> = sent
        .iter()
        .filter(|p| name_of(p) == "EX_REGEN_MAX")
        .map(|p| decode(p))
        .collect();
    assert_eq!(
        bars,
        [ExRegenMax {
            time: 60,
            tick_interval: 5,
            amount_per_tick: 5.0
        }]
    );
    // `getTicksMultiplier()` = (5 * 666) / 1000f = 3.33 (a float), the task runs every 3.33 s.
    s.advance(3_400);
    let multiplier = f64::from((5_i64 * 666) as f32 / 1000.0_f32);
    assert_eq!(s.hp(ME), 10.0 + 5.0 * multiplier);
}

#[test]
fn heal_over_time_without_hp_recover_sends_no_bar() {
    let Some(table) = datapack() else { return };
    // 441 Force Meditation: `HealOverTime` ticks 5, abnormal type FORCE_MEDITATION.
    let skill = table.get(441, 1).unwrap().clone();
    let mut s = Scene::new(table);
    s.me.take();
    s.apply(ME, ME, key(&skill));
    assert!(s.me.take().iter().all(|p| name_of(p) != "EX_REGEN_MAX"));
}

#[test]
fn heal_over_time_at_full_hp_ends_a_toggle() {
    // NOTE(java-quirk): `HealOverTime` returns false at the maximum, which ends a toggle.
    let mut s = Scene::inline(
        r#"<skill id="9103" levels="1" name="Toggle regen"><operateType>T</operateType><targetType>SELF</targetType>
        <effects><effect name="HealOverTime" ticks="5"><power>1</power></effect></effects></skill>"#,
    );
    s.apply(ME, ME, SkillKey::new(9103, 1));
    assert!(s.world.is_affected_by_skill(ME, 9103));
    s.advance(3_400);
    assert!(!s.world.is_affected_by_skill(ME, 9103));
}

#[test]
fn a_heal_over_time_toggle_keeps_going_while_hp_is_missing() {
    let mut s = Scene::inline(
        r#"<skill id="9103" levels="1" name="Toggle regen"><operateType>T</operateType><targetType>SELF</targetType>
        <effects><effect name="HealOverTime" ticks="5"><power>1</power></effect></effects></skill>"#,
    );
    s.world.set_current_hp(ME, 10.0, false);
    s.apply(ME, ME, SkillKey::new(9103, 1));
    s.advance(3_400);
    assert!(s.world.is_affected_by_skill(ME, 9103));
    assert!(s.hp(ME) > 10.0);
}

// ---- MP heals ---------------------------------------------------------------------------------------

#[test]
fn mana_heal_restores_power_up_to_the_room() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "ManaHeal", |_| true).clone();
    let power = param(&skill, "ManaHeal", "power");
    let mut s = Scene::new(table);
    s.world.set_current_mp(ME, 5.0, false);
    let max = f64::from(s.world.stats(ME).unwrap().max_mp());
    s.me.take();
    s.apply(ME, ME, key(&skill));
    let expected = (5.0 + power).min(max);
    assert_eq!(s.mp(ME), expected);
    assert_eq!(
        restored(&s.me.take()),
        [(
            SystemMessageId::S1MpHasBeenRestored,
            vec![int((expected - 5.0) as i32)]
        )]
    );
}

#[test]
fn mana_heal_from_another_player_names_the_healer() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "ManaHeal", |_| true).clone();
    let mut s = Scene::new(table);
    s.world.set_current_mp(ME, 1.0, false);
    s.me.take();
    s.apply(P2, ME, key(&skill));
    let sent = restored(&s.me.take());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, SystemMessageId::S2MpHasBeenRestoredByC1);
    assert_eq!(sent[0].1[0], name("P2"));
}

#[test]
fn mana_heal_by_level_gives_less_to_a_target_far_above_the_magic_level() {
    // Level 20 target, magic level 10: a difference of 10 gives 50%.
    let mut s = Scene::inline(
        r#"<skill id="9104" levels="1" name="Recharge"><operateType>A1</operateType><targetType>ONE</targetType>
        <magicLevel>10</magicLevel>
        <effects><effect name="ManaHealByLevel"><power>100</power></effect></effects></skill>"#,
    );
    let npc = s.npc(9010);
    s.world.set_current_mp(npc, 0.0, false);
    s.apply(ME, npc, SkillKey::new(9104, 1));
    assert_eq!(s.mp(npc), 50.0);
}

#[test]
fn mana_heal_by_level_has_no_penalty_within_five_levels_and_none_left_at_fifteen() {
    let mut s = Scene::inline(
        r##"<skill id="9104" levels="2" name="Recharge"><operateType>A1</operateType><targetType>ONE</targetType>
        <table name="#ml">15 5</table><magicLevel>#ml</magicLevel>
        <effects><effect name="ManaHealByLevel"><power>100</power></effect></effects></skill>"##,
    );
    let npc = s.npc(9010);
    // The level-1 row names magic level 15 (target level 20: a difference of 5, no penalty).
    s.world.set_current_mp(npc, 0.0, false);
    s.apply(ME, npc, SkillKey::new(9104, 1));
    assert_eq!(s.mp(npc), 100.0);
    // Magic level 5 (a difference of 15): nothing.
    s.world.set_current_mp(npc, 0.0, false);
    s.apply(ME, npc, SkillKey::new(9104, 2));
    assert_eq!(s.mp(npc), 0.0);
}

#[test]
fn mana_heal_by_level_of_the_datapack_recharges_a_player() {
    let Some(table) = datapack() else { return };
    // 1013 Recharge.
    let skill = table.get(1013, 1).unwrap().clone();
    let power = param(&skill, "ManaHealByLevel", "power");
    let mut s = Scene::new(table);
    s.world.set_current_mp(P2, 0.0, false);
    s.p2.take();
    s.apply(ME, P2, key(&skill));
    let max = f64::from(s.world.stats(P2).unwrap().max_mp());
    assert_eq!(s.mp(P2), power.min(max));
    let sent = restored(&s.p2.take());
    assert_eq!(sent[0].0, SystemMessageId::S2MpHasBeenRestoredByC1);
}

#[test]
fn mana_heal_over_time_returns_true_at_full_mp_so_a_toggle_stays() {
    // NOTE(java-quirk): unlike `HealOverTime`, `ManaHealOverTime` returns true at the maximum.
    let mut s = Scene::inline(
        r#"<skill id="9105" levels="1" name="Toggle mana"><operateType>T</operateType><targetType>SELF</targetType>
        <effects><effect name="ManaHealOverTime" ticks="5"><power>1</power></effect></effects></skill>"#,
    );
    s.apply(ME, ME, SkillKey::new(9105, 1));
    s.advance(3_400);
    assert!(s.world.is_affected_by_skill(ME, 9105));
}

#[test]
fn mana_heal_over_time_restores_each_tick() {
    let Some(table) = datapack() else { return };
    // 441 Force Meditation has `ManaHealOverTime` power 10, ticks 5 next to the HP one.
    let skill = table.get(441, 1).unwrap().clone();
    assert_eq!(param(&skill, "ManaHealOverTime", "power"), 10.0);
    let mut s = Scene::new(table);
    s.world.set_current_mp(ME, 5.0, false);
    s.apply(ME, ME, key(&skill));
    s.advance(3_400);
    let multiplier = f64::from((5_i64 * 666) as f32 / 1000.0_f32);
    assert_eq!(s.mp(ME), 5.0 + 10.0 * multiplier);
}

#[test]
fn mana_heal_percent_takes_a_share_of_the_maximum_mp() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "ManaHealPercent", |_| true).clone();
    let power = param(&skill, "ManaHealPercent", "power");
    let mut s = Scene::new(table);
    s.world.set_current_mp(ME, 0.0, false);
    let max = f64::from(s.world.stats(ME).unwrap().max_mp());
    s.apply(ME, ME, key(&skill));
    let expected = if power == 100.0 {
        max
    } else {
        (max * power) / 100.0
    };
    assert_eq!(s.mp(ME), expected.min(max));
}

// ---- CP heals ---------------------------------------------------------------------------------------

#[test]
fn cp_heal_restores_cp_and_tells_the_amount() {
    let Some(table) = datapack() else { return };
    // 440 Braveheart.
    let skill = table.get(440, 1).unwrap().clone();
    let power = param(&skill, "CpHeal", "power");
    let mut s = Scene::new(table);
    s.world.set_current_cp(ME, 1.0, false);
    let max = f64::from(s.world.stats(ME).unwrap().max_cp());
    s.me.take();
    s.apply(ME, ME, key(&skill));
    let expected = (1.0 + power).min(max);
    assert_eq!(s.cp(ME), expected);
    assert_eq!(
        restored(&s.me.take()),
        [(
            SystemMessageId::S1CpHasBeenRestored,
            vec![int((expected - 1.0) as i32)]
        )]
    );
}

#[test]
fn cp_heal_from_another_player_names_the_healer() {
    let Some(table) = datapack() else { return };
    let skill = table.get(440, 1).unwrap().clone();
    let mut s = Scene::new(table);
    s.world.set_current_cp(ME, 1.0, false);
    s.me.take();
    s.apply(P2, ME, key(&skill));
    let sent = restored(&s.me.take());
    assert_eq!(sent[0].0, SystemMessageId::S2CpHasBeenRestoredByC1);
    assert_eq!(sent[0].1[0], name("P2"));
}

#[test]
fn cp_heal_percent_restores_a_share_of_the_maximum_cp() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "CpHealPercent", |_| true).clone();
    let power = param(&skill, "CpHealPercent", "power");
    let mut s = Scene::new(table);
    s.world.set_current_cp(ME, 0.0, false);
    let max = f64::from(s.world.stats(ME).unwrap().max_cp());
    s.apply(ME, ME, key(&skill));
    let expected = if power == 100.0 {
        max
    } else {
        (max * power) / 100.0
    };
    assert_eq!(s.cp(ME), expected.min(max));
}

#[test]
fn cp_heal_over_time_restores_each_tick_and_always_returns_true() {
    // No datapack skill uses `CpHealOverTime`.
    let mut s = Scene::inline(
        r#"<skill id="9106" levels="1" name="Toggle cp"><operateType>T</operateType><targetType>SELF</targetType>
        <effects><effect name="CpHealOverTime" ticks="5"><power>2</power></effect></effects></skill>"#,
    );
    s.world.set_current_cp(ME, 1.0, false);
    s.apply(ME, ME, SkillKey::new(9106, 1));
    s.advance(3_400);
    let multiplier = f64::from((5_i64 * 666) as f32 / 1000.0_f32);
    assert_eq!(s.cp(ME), 1.0 + 2.0 * multiplier);
    // NOTE(java-quirk): at the maximum it returns false (a toggle ends), below it true.
    assert!(s.world.is_affected_by_skill(ME, 9106));
}

// ---- the maximum HP and CP ----------------------------------------------------------------------

#[test]
fn max_hp_diff_raises_the_maximum_heals_and_wears_off() {
    let mut s = Scene::inline(
        r#"<skill id="9110" levels="1" name="Boost"><operateType>A2</operateType><targetType>SELF</targetType>
        <abnormalTime>5</abnormalTime><abnormalType>MAX_HP_UP</abnormalType><abnormalLevel>1</abnormalLevel>
        <effects><effect name="MaxHp"><power>50</power><type>DIFF</type><heal>true</heal></effect></effects></skill>"#,
    );
    let before = s.world.stats(ME).unwrap().max_hp();
    s.me.take();
    s.apply(ME, ME, SkillKey::new(9110, 1));
    assert_eq!(s.world.stats(ME).unwrap().max_hp(), before + 50);
    assert_eq!(s.hp(ME), 150.0);
    assert_eq!(
        restored(&s.me.take()),
        [(SystemMessageId::S1HpHasBeenRestored, vec![int(50)])]
    );
    // `onExit` removes the function and pulls the HP down to the old maximum.
    s.advance(7_000);
    assert_eq!(s.world.stats(ME).unwrap().max_hp(), before);
    assert_eq!(s.hp(ME), f64::from(before));
}

#[test]
fn max_hp_per_multiplies_and_heals_the_difference() {
    let mut s = Scene::inline(
        r#"<skill id="9111" levels="1" name="Boost"><operateType>A2</operateType><targetType>ONE</targetType>
        <abnormalTime>5</abnormalTime><abnormalType>MAX_HP_UP</abnormalType><abnormalLevel>1</abnormalLevel>
        <effects><effect name="MaxHp"><power>50</power><type>PER</type><heal>true</heal></effect></effects></skill>"#,
    );
    let before = s.world.stats(P2).unwrap().max_hp();
    s.p2.take();
    s.apply(ME, P2, SkillKey::new(9111, 1));
    assert_eq!(s.world.stats(P2).unwrap().max_hp(), before * 3 / 2);
    // `(power - 1) * maxHp`, announced "by C1" because the caster is another player.
    assert_eq!(s.hp(P2), 100.0 + 50.0);
    assert_eq!(
        restored(&s.p2.take()),
        [(
            SystemMessageId::S2HpHasBeenRestoredByC1,
            vec![name("P1"), int(50)]
        )]
    );
}

#[test]
fn max_hp_without_heal_only_moves_the_maximum() {
    let mut s = Scene::inline(
        r#"<skill id="9112" levels="1" name="Boost"><operateType>A2</operateType><targetType>SELF</targetType>
        <abnormalTime>60</abnormalTime><abnormalType>MAX_HP_UP</abnormalType><abnormalLevel>1</abnormalLevel>
        <effects><effect name="MaxHp"><power>50</power></effect></effects></skill>"#,
    );
    let before = s.world.stats(ME).unwrap().max_hp();
    s.me.take();
    s.apply(ME, ME, SkillKey::new(9112, 1));
    assert_eq!(s.world.stats(ME).unwrap().max_hp(), before + 50);
    assert_eq!(s.hp(ME), f64::from(before));
    assert!(restored(&s.me.take()).is_empty());
}

#[test]
fn max_hp_of_the_datapack_battle_roar_raises_the_maximum() {
    let Some(table) = datapack() else { return };
    // 121 Battle Roar: `MaxHp` PER (heal false) and `HealPercent`.
    let skill = table.get(121, 1).unwrap().clone();
    let percent = param(&skill, "MaxHp", "power");
    let mut s = Scene::new(table);
    let before = s.world.stats(ME).unwrap().max_hp();
    s.apply(ME, ME, key(&skill));
    let after = s.world.stats(ME).unwrap().max_hp();
    let expected = (f64::from(before) * (1.0 + percent / 100.0)) as i32;
    assert!((after - expected).abs() <= 1, "{after} vs {expected}");
}

#[test]
fn max_cp_diff_raises_the_maximum_and_leaves_the_cp_above_it_when_it_ends() {
    let mut s = Scene::inline(
        r#"<skill id="9113" levels="1" name="Boost"><operateType>A2</operateType><targetType>SELF</targetType>
        <abnormalTime>5</abnormalTime><abnormalType>MAX_HP_UP</abnormalType><abnormalLevel>1</abnormalLevel>
        <effects><effect name="MaxCp"><power>10</power><type>DIFF</type><heal>true</heal></effect></effects></skill>"#,
    );
    let before = s.world.stats(ME).unwrap().max_cp();
    s.me.take();
    s.apply(ME, ME, SkillKey::new(9113, 1));
    assert_eq!(s.world.stats(ME).unwrap().max_cp(), before + 10);
    assert_eq!(s.cp(ME), f64::from(before) + 10.0);
    assert_eq!(
        restored(&s.me.take()),
        [(SystemMessageId::S1CpHasBeenRestored, vec![int(10)])]
    );
    s.advance(7_000);
    assert_eq!(s.world.stats(ME).unwrap().max_cp(), before);
    // NOTE(java-quirk): `MaxCp.onExit` does not pull the CP down like `MaxHp.onExit` does.
    assert_eq!(s.cp(ME), f64::from(before) + 10.0);
}

#[test]
fn max_cp_of_the_datapack_raises_the_maximum() {
    let Some(table) = datapack() else { return };
    // 6106 Winter Holiday Festival: `MaxCp` and `MaxHp`, PER 5.
    let skill = table.get(6106, 1).unwrap().clone();
    assert_eq!(param(&skill, "MaxCp", "power"), 5.0);
    let mut s = Scene::new(table);
    let (hp, cp) = {
        let view = s.world.stats(ME).unwrap();
        (view.max_hp(), view.max_cp())
    };
    s.apply(ME, ME, key(&skill));
    let view = s.world.stats(ME).unwrap();
    assert_eq!(view.max_cp(), (f64::from(cp) * 1.05) as i32);
    assert_eq!(view.max_hp(), (f64::from(hp) * 1.05) as i32);
    // Without `heal` the current values stay.
    assert_eq!(s.cp(ME), 20.0);
}

#[test]
fn a_max_cp_effect_with_an_unknown_parameter_changes_nothing() {
    let Some(table) = datapack() else { return };
    // NOTE(java-quirk): 5743 gives `MaxCp` an `<amount>` that the class never reads, and it is
    // an A1 skill: no continuous effect lands.
    let skill = table.get(5743, 1).unwrap().clone();
    let mut s = Scene::new(table);
    let before = s.world.stats(ME).unwrap().max_cp();
    s.apply(ME, ME, key(&skill));
    assert_eq!(s.world.stats(ME).unwrap().max_cp(), before);
}

// ---- damage over time -----------------------------------------------------------------------------

/// `getTicksMultiplier()` of `ticks="5"`: `(5 * 666) / 1000f`, a float widened to a double.
const TICK_MULTIPLIER: f64 = (3330_f32 / 1000.0_f32) as f64;

#[test]
fn the_tick_multiplier_is_a_float() {
    assert_eq!(f64::from((5_i64 * 666) as f32 / 1000.0_f32), TICK_MULTIPLIER);
    assert_ne!(TICK_MULTIPLIER, 3.33);
}

fn dot_scene(handler_params: &str) -> Scene {
    Scene::inline(&format!(
        r#"<skill id="9120" levels="1" name="Poison"><operateType>A2</operateType><targetType>ONE</targetType>
        <abnormalTime>60</abnormalTime><abnormalType>POISON</abnormalType><abnormalLevel>1</abnormalLevel>
        <isDebuff>true</isDebuff><effectPoint>-100</effectPoint>
        <effects><effect name="DamOverTime" ticks="5">{handler_params}</effect></effects></skill>"#
    ))
}

#[test]
fn dam_over_time_hurts_a_monster_by_power_times_the_tick_multiplier() {
    let mut s = dot_scene("<power>10</power>");
    let npc = s.npc(9001);
    s.apply(ME, npc, SkillKey::new(9120, 1));
    s.advance(3_400);
    assert_eq!(s.hp(npc), 300.0 - 10.0 * TICK_MULTIPLIER);
    s.advance(3_400);
    assert_eq!(s.hp(npc), 300.0 - 2.0 * 10.0 * TICK_MULTIPLIER);
}

#[test]
fn dam_over_time_on_a_player_takes_the_cp_first_when_a_player_cast_it() {
    let mut s = dot_scene("<power>10</power>");
    s.p2.take();
    s.apply(ME, P2, SkillKey::new(9120, 1));
    s.advance(3_400);
    // CP 20 first, then the rest of 33.3 from the HP; a DOT is not announced to the victim.
    assert_eq!(s.cp(P2), 0.0);
    assert_eq!(s.hp(P2), 100.0 - (10.0 * TICK_MULTIPLIER - 20.0));
    assert!(
        messages(&s.p2.take())
            .iter()
            .all(|m| m.id != SystemMessageId::C1HasReceivedS3DamageFromC2)
    );
}

#[test]
fn dam_over_time_cannot_kill_unless_can_kill_is_set() {
    let mut s = dot_scene("<power>100</power>");
    let npc = s.npc(9001);
    s.world.set_current_hp(npc, 20.0, false);
    s.apply(ME, npc, SkillKey::new(9120, 1));
    s.advance(3_400);
    // `damage >= hp - 1` and no `canKill`: the damage is cut to `hp - 1`.
    assert_eq!(s.hp(npc), 1.0);
    assert!(!s.world.is_dead(npc));
    // At 1 HP the tick does nothing.
    s.advance(3_400);
    assert_eq!(s.hp(npc), 1.0);
    assert!(!s.world.is_dead(npc));
}

#[test]
fn dam_over_time_with_can_kill_kills() {
    let mut s = dot_scene("<power>100</power><canKill>true</canKill>");
    let npc = s.npc(9001);
    s.world.set_current_hp(npc, 20.0, false);
    s.apply(ME, npc, SkillKey::new(9120, 1));
    s.advance(3_400);
    assert!(s.world.is_dead(npc));
}

#[test]
fn a_dam_over_time_toggle_is_cancelled_when_the_hp_runs_out() {
    let mut s = Scene::inline(
        r#"<skill id="9121" levels="1" name="Drain toggle"><operateType>T</operateType><targetType>SELF</targetType>
        <effects><effect name="DamOverTime" ticks="5"><power>100</power></effect></effects></skill>"#,
    );
    s.world.set_current_hp(ME, 5.0, false);
    s.apply(ME, ME, SkillKey::new(9121, 1));
    s.me.take();
    s.advance(3_400);
    assert!(
        messages(&s.me.take())
            .iter()
            .any(|m| m.id == SystemMessageId::YourSkillHasBeenCanceledDueToLackOfHp)
    );
    assert!(!s.world.is_affected_by_skill(ME, 9121));
    assert_eq!(s.hp(ME), 5.0);
}

#[test]
fn a_dam_over_time_toggle_costs_hp_but_cannot_kill() {
    let mut s = Scene::inline(
        r#"<skill id="9121" levels="1" name="Drain toggle"><operateType>T</operateType><targetType>SELF</targetType>
        <effects><effect name="DamOverTime" ticks="5"><power>10</power></effect></effects></skill>"#,
    );
    s.apply(ME, ME, SkillKey::new(9121, 1));
    s.advance(3_400);
    assert_eq!(s.hp(ME), 100.0 - 10.0 * TICK_MULTIPLIER);
    assert!(s.world.is_affected_by_skill(ME, 9121));
}

#[test]
fn dam_over_time_of_the_datapack_hurts_a_monster() {
    let Some(table) = datapack() else { return };
    // The first skill that has a `DamOverTime` and nothing else among its general effects.
    let skill = find(&table, "DamOverTime", |s| {
        s.effects.get(EffectScope::General).len() == 1 && !s.is_toggle()
    })
    .clone();
    let power = param(&skill, "DamOverTime", "power");
    let ticks = skill.effects.get(EffectScope::General)[0].ticks;
    let mut s = Scene::new(table);
    let npc = s.npc(9001);
    s.apply(ME, npc, key(&skill));
    s.advance(u64::try_from(ticks).unwrap() * 666 + 100);
    let multiplier = f64::from((i64::from(ticks) * 666) as f32 / 1000.0_f32);
    assert_eq!(s.hp(npc), 300.0 - power * multiplier);
}

#[test]
fn dam_over_time_percent_takes_a_share_of_the_current_hp() {
    let mut s = Scene::inline(
        r#"<skill id="9122" levels="1" name="Wither"><operateType>A2</operateType><targetType>ONE</targetType>
        <abnormalTime>60</abnormalTime><abnormalType>POISON</abnormalType><abnormalLevel>1</abnormalLevel>
        <isDebuff>true</isDebuff><effectPoint>-100</effectPoint>
        <effects><effect name="DamOverTimePercent" ticks="5"><power>0.1</power></effect></effects></skill>"#,
    );
    let npc = s.npc(9001);
    s.apply(ME, npc, SkillKey::new(9122, 1));
    s.advance(3_400);
    // `getCurrentHp() * _power * getTicksMultiplier()`.
    assert_eq!(s.hp(npc), 300.0 - 300.0 * 0.1 * TICK_MULTIPLIER);
}

#[test]
fn dam_over_time_percent_of_the_datapack_applies() {
    let Some(table) = datapack() else { return };
    // 6435 Curse of the Plains.
    let skill = table.get(6435, 1).unwrap().clone();
    let power = param(&skill, "DamOverTimePercent", "power");
    let mut s = Scene::new(table);
    let npc = s.npc(9001);
    s.apply(ME, npc, key(&skill));
    s.advance(3_400);
    let hp = s.hp(npc);
    assert!(hp < 300.0, "{hp}");
    assert_eq!(hp, 300.0 - 300.0 * power * TICK_MULTIPLIER);
}

#[test]
fn mana_dam_over_time_drains_mp_each_tick() {
    let mut s = Scene::inline(
        r#"<skill id="9123" levels="1" name="Mana burn"><operateType>A2</operateType><targetType>ONE</targetType>
        <abnormalTime>60</abnormalTime><abnormalType>POISON</abnormalType><abnormalLevel>1</abnormalLevel>
        <isDebuff>true</isDebuff><effectPoint>-100</effectPoint>
        <effects><effect name="ManaDamOverTime" ticks="5"><power>3</power></effect></effects></skill>"#,
    );
    s.apply(ME, P2, SkillKey::new(9123, 1));
    s.advance(3_400);
    assert_eq!(s.mp(P2), 50.0 - 3.0 * TICK_MULTIPLIER);
}

#[test]
fn a_mana_dam_over_time_toggle_is_deactivated_without_mp() {
    let mut s = Scene::inline(
        r#"<skill id="9124" levels="1" name="Mana toggle"><operateType>T</operateType><targetType>SELF</targetType>
        <effects><effect name="ManaDamOverTime" ticks="5"><power>100</power></effect></effects></skill>"#,
    );
    s.apply(ME, ME, SkillKey::new(9124, 1));
    s.me.take();
    s.advance(3_400);
    assert!(
        messages(&s.me.take())
            .iter()
            .any(|m| m.id == SystemMessageId::YourSkillWasDeactivatedDueToLackOfMp)
    );
    assert!(!s.world.is_affected_by_skill(ME, 9124));
}

#[test]
fn mana_dam_over_time_of_the_datapack_applies() {
    let Some(table) = datapack() else { return };
    // 1001 Soul Cry: a toggle that costs MP per tick from level 3.
    let skill = table.get(1001, 3).unwrap().clone();
    let power = param(&skill, "ManaDamOverTime", "power");
    let mut s = Scene::new(table);
    s.apply(ME, ME, key(&skill));
    s.advance(3_400);
    assert_eq!(s.mp(ME), 50.0 - power * TICK_MULTIPLIER);
    assert!(s.world.is_affected_by_skill(ME, 1001));
}

#[test]
fn cp_dam_percent_takes_a_share_of_the_cp() {
    let Some(table) = datapack() else { return };
    // 320 Wrath.
    let skill = table.get(320, 1).unwrap().clone();
    let power = param(&skill, "CpDamPercent", "power");
    let mut s = Scene::new(table);
    s.apply(ME, P2, key(&skill));
    let damage = ((20.0 * power) / 100.0) as i32;
    assert_eq!(s.cp(P2), 20.0 - f64::from(damage));
}

#[test]
fn cp_dam_percent_leaves_monsters_alone() {
    let mut s = Scene::inline(
        r#"<skill id="9125" levels="1" name="Wrath"><operateType>A1</operateType><targetType>ONE</targetType>
        <effects><effect name="CpDamPercent"><power>50</power></effect></effects></skill>"#,
    );
    let npc = s.npc(9001);
    s.apply(ME, npc, SkillKey::new(9125, 1));
    assert_eq!(s.hp(npc), 300.0);
}

// ---- the rest ----------------------------------------------------------------------------------------

#[test]
fn static_damage_hurts_by_exactly_its_power_and_tells_the_caster() {
    let Some(table) = datapack() else { return };
    // 3260 Forgiveness.
    let skill = table.get(3260, 1).unwrap().clone();
    let power = param(&skill, "StaticDamage", "power");
    let mut s = Scene::new(table);
    let npc = s.npc(9001);
    s.me.take();
    s.apply(ME, npc, key(&skill));
    assert_eq!(s.hp(npc), 300.0 - power);
    let sent = restored(&s.me.take());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].0, SystemMessageId::C1HasDoneS3PointsOfDamageToC2);
    assert_eq!(sent[0].1[2], int(power as i32));
}

#[test]
fn static_damage_from_a_dead_caster_does_nothing() {
    let mut s = Scene::inline(
        r#"<skill id="9126" levels="1" name="Static"><operateType>A1</operateType><targetType>ONE</targetType>
        <effects><effect name="StaticDamage"><power>25</power></effect></effects></skill>"#,
    );
    let npc = s.npc(9001);
    s.world.set_dead(ME, true);
    s.apply(ME, npc, SkillKey::new(9126, 1));
    assert_eq!(s.hp(npc), 300.0);
}

#[test]
fn rebalance_hp_does_nothing_without_a_party() {
    let Some(table) = datapack() else { return };
    // 1335 Balance Life.
    let skill = table.get(1335, 1).unwrap().clone();
    let mut s = Scene::new(table);
    s.world.set_current_hp(ME, 10.0, false);
    s.apply(ME, ME, key(&skill));
    assert_eq!(s.hp(ME), 10.0);
    assert_eq!(s.hp(P2), 100.0);
}

#[test]
fn transfer_damage_links_the_effected_to_the_caster_until_it_ends() {
    let Some(table) = datapack() else { return };
    // 528 Shield of Faith (15 seconds).
    let skill = table.get(528, 1).unwrap().clone();
    let mut s = Scene::new(table);
    assert_eq!(s.world.transfer_damage_to(ME), None);
    s.apply(P2, ME, key(&skill));
    assert_eq!(s.world.transfer_damage_to(ME), Some(P2));
    s.advance(17_000);
    assert_eq!(s.world.transfer_damage_to(ME), None);
}

#[test]
fn the_ported_handlers_are_registered() {
    let s = Scene::inline("");
    let registry = s.world.effect_registry();
    for name in [
        "CpDamPercent",
        "CpHeal",
        "CpHealOverTime",
        "CpHealPercent",
        "DamOverTime",
        "DamOverTimePercent",
        "HealOverTime",
        "HealPercent",
        "HpByLevel",
        "ManaDamOverTime",
        "ManaHeal",
        "ManaHealByLevel",
        "ManaHealOverTime",
        "ManaHealPercent",
        "MaxCp",
        "MaxHp",
        "RebalanceHP",
        "StaticDamage",
        "TransferDamage",
    ] {
        assert!(registry.is_ported(name), "{name}");
    }
}

#[test]
fn a_bad_number_in_a_parameter_drops_the_effect_like_java() {
    // `StatSet.getDouble` throws; `createEffect` logs it and the effect is dropped.
    let mut s = Scene::inline(
        r#"<skill id="9127" levels="1" name="Broken"><operateType>A1</operateType><targetType>SELF</targetType>
        <effects><effect name="HealPercent"><power>lots</power></effect></effects></skill>"#,
    );
    s.world.set_current_hp(ME, 10.0, false);
    s.apply(ME, ME, SkillKey::new(9127, 1));
    assert_eq!(s.hp(ME), 10.0);
}

// ==== M5-EFX-04: damage, Heal, Lethal ====================================================================
//
// The numbers of the damage effects have a random part (the weapon's random damage, drawn twice for a
// physical skill), so a test either bounds the damage by the formula, compares two scenes that start
// from one RNG seed (the draws are the same, only the input under test differs), or makes the roll
// certain with a stat (`pSkillEvas` 100, `mCritRate`, `magicSuccRes`, `vengeance*`).

use l2_world::combat::attacker_position;
use l2_world::formulas::Position;

/// A passive-looking buff that adds `value` to `stat` (an `A2` skill on its own abnormal type).
fn stat_buff(id: i32, abnormal: &str, stat: &str, op: &str, value: &str) -> String {
    format!(
        r#"<skill id="{id}" levels="1" name="B{id}"><operateType>A2</operateType><targetType>SELF</targetType>
        <abnormalTime>600</abnormalTime><abnormalType>{abnormal}</abnormalType><abnormalLevel>1</abnormalLevel>
        <effects><effect name="Buff"><{op} stat="{stat}" val="{value}"/></effect></effects></skill>"#
    )
}

/// A melee physical skill with the given effect XML (`power` 100, `castRange` 40).
fn strike(id: i32, effect: &str, extra: &str) -> String {
    format!(
        r#"<skill id="{id}" levels="1" name="S{id}"><operateType>A1</operateType><targetType>ONE</targetType>
        <isMagic>0</isMagic><castRange>40</castRange><power>100</power>{extra}
        <effects>{effect}</effects></skill>"#
    )
}

/// A magic skill with the given effect XML (`power` 50).
fn spell(id: i32, effect: &str, extra: &str) -> String {
    format!(
        r#"<skill id="{id}" levels="1" name="M{id}"><operateType>A1</operateType><targetType>ONE</targetType>
        <isMagic>1</isMagic><power>50</power>{extra}
        <effects>{effect}</effects></skill>"#
    )
}

const STUN: &str = r#"<skill id="9390" levels="1" name="Stun"><operateType>A2</operateType><targetType>SELF</targetType>
    <abnormalTime>60</abnormalTime><abnormalType>STUN</abnormalType><abnormalLevel>1</abnormalLevel>
    <effects><effect name="Stun"/></effects></skill>"#;

/// A static skill (`isMagic` 2) healing exactly `power`.
fn static_heal(id: i32, power: i32) -> String {
    spell(
        id,
        &format!(r#"<effect name="Heal"><power>{power}</power></effect>"#),
        "",
    )
    .replace("<isMagic>1</isMagic>", "<isMagic>2</isMagic>")
}

fn k(id: i32) -> SkillKey {
    SkillKey::new(id, 1)
}

fn ids_of(packets: &[Vec<u8>]) -> Vec<SystemMessageId> {
    messages(packets).into_iter().map(|m| m.id).collect()
}

/// The damage numbers of the `C1_HAS_DONE_S3_POINTS_OF_DAMAGE_TO_C2` messages.
fn damage_numbers(packets: &[Vec<u8>]) -> Vec<i32> {
    messages(packets)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::C1HasDoneS3PointsOfDamageToC2)
        .filter_map(|m| match m.params.last() {
            Some(SystemMessageParam::IntNumber(n)) => Some(*n),
            _ => None,
        })
        .collect()
}

/// The first value `f` gives for a seed of 0..500 (some outcomes are a matter of the dice).
fn first_seed<T>(f: impl FnMut(u64) -> Option<T>) -> T {
    (0..500).find_map(f).expect("no seed made the case happen")
}

/// Scene with the skills of `xml`, a seed and settings that make the magic critical hit certain
/// (`max_mcrit_rate` high) or impossible (`0`).
fn scene_with_mcrit_cap(xml: &str, seed: u64, cap: i32) -> Scene {
    let mut settings = WorldSettings::default();
    settings.stats.max_mcrit_rate = cap;
    Scene::build(inline_table(xml), seed, settings)
}

/// The HP of npc 9030.
const BIG: f64 = 100_000.0;

fn loss(before: f64, now: f64) -> f64 {
    before - now
}

#[test]
fn the_damage_heal_and_hate_handlers_are_registered() {
    let s = Scene::inline("");
    let registry = s.world.effect_registry();
    for name in [
        "Backstab",
        "DeathLink",
        "DeleteHate",
        "DeleteHateOfMe",
        "EnergyDamage",
        "FatalBlow",
        "Heal",
        "HpDrain",
        "Lethal",
        "MagicalDamage",
        "MagicalDamageByAbnormal",
        "MagicalDamageMp",
        "MagicalSoulDamage",
        "PhysicalDamage",
        "PhysicalDamageHpLink",
        "PhysicalSoulDamage",
        "RandomizeHate",
        "Resurrection",
        "SoulBlow",
        "TargetMeProbability",
        "TransferHate",
    ] {
        assert!(registry.is_ported(name), "{name}");
    }
}

// ---- Heal ----------------------------------------------------------------------------------------------

#[test]
fn heal_of_a_static_skill_restores_exactly_its_power() {
    // Java Heal.onStart: a static skill (potions, scrolls) skips the M. Atk part and the critical.
    let mut s = Scene::inline(&static_heal(9401, 30));
    s.world.set_current_hp(ME, 10.0, false);
    s.me.take();
    s.apply(P2, ME, k(9401));
    assert_eq!(s.hp(ME), 40.0);
    assert_eq!(
        restored(&s.me.take()),
        [(
            SystemMessageId::S2HpHasBeenRestoredByC1,
            vec![name("P2"), int(30)]
        )]
    );
}

#[test]
fn heal_of_a_magic_skill_adds_the_square_root_of_the_magic_attack() {
    // `amount = power + sqrt(mAtkMul * mAtk)`; no weapon and no shot: `mAtkMul = 1 + 1`.
    let mut s = scene_with_mcrit_cap(
        &spell(9401, r#"<effect name="Heal"><power>20</power></effect>"#, ""),
        1,
        0,
    );
    s.world.set_current_hp(ME, 1.0, false);
    let m_atk = s.world.stats(P2).unwrap().m_atk(Some(P2), None);
    s.apply(P2, ME, k(9401));
    let amount = 20.0 + (2.0 * m_atk).sqrt();
    assert_eq!(s.hp(ME), 1.0 + amount);
}

#[test]
fn heal_of_a_magic_skill_triples_on_a_magic_critical_hit() {
    // "Heal critic, since CT2.3 Gracia Final": x3 when the caster's magic critical rolls.
    let xml = format!(
        "{}{}",
        spell(9401, r#"<effect name="Heal"><power>20</power></effect>"#, ""),
        stat_buff(9402, "MAGIC_CRITICAL_UP", "mCritRate", "add", "500"),
    );
    let mut s = scene_with_mcrit_cap(&xml, 1, 100_000);
    s.apply(P2, P2, k(9402));
    s.world.set_current_hp(ME, 1.0, false);
    let m_atk = s.world.stats(P2).unwrap().m_atk(Some(P2), None);
    s.apply(P2, ME, k(9401));
    let amount = (20.0 + (2.0 * m_atk).sqrt()) * 3.0;
    assert_eq!(s.hp(ME), 1.0 + amount);
}

#[test]
fn heal_never_goes_above_the_room_left() {
    let mut s = Scene::inline(&static_heal(9401, 500));
    s.world.set_current_hp(ME, 90.0, false);
    s.me.take();
    s.apply(ME, ME, k(9401));
    let max = f64::from(s.world.stats(ME).unwrap().max_hp());
    assert_eq!(s.hp(ME), max);
    // The own heal is the plain message with `(int) amount`.
    assert_eq!(
        restored(&s.me.take()),
        [(
            SystemMessageId::S1HpHasBeenRestored,
            vec![int((max - 90.0) as i32)]
        )]
    );
}

#[test]
fn heal_skill_4051_tells_rejuvenating_hp() {
    // NOTE(java-quirk): skill 4051 has its own message whoever heals.
    let mut s = Scene::inline(&static_heal(4051, 10));
    s.world.set_current_hp(ME, 10.0, false);
    s.me.take();
    s.apply(P2, ME, k(4051));
    assert_eq!(
        restored(&s.me.take()),
        [(SystemMessageId::RejuvenatingHp, vec![])]
    );
    assert_eq!(s.hp(ME), 20.0);
}

#[test]
fn heal_from_an_npc_uses_the_plain_message_and_skips_dead_and_invulnerable_targets() {
    let mut s = Scene::inline(&static_heal(9401, 10));
    let npc = s.npc(9030);
    s.world.set_current_hp(ME, 10.0, false);
    s.me.take();
    s.apply(npc, ME, k(9401));
    // `effector.isPlayer() && effector != effected` is false for an NPC healer.
    assert_eq!(
        restored(&s.me.take()),
        [(SystemMessageId::S1HpHasBeenRestored, vec![int(10)])]
    );
    // An invulnerable target is not healed.
    s.world.set_invul(ME, true);
    s.me.take();
    s.apply(P2, ME, k(9401));
    assert_eq!(s.hp(ME), 20.0);
    assert!(restored(&s.me.take()).is_empty());
    // Neither is a dead one.
    s.world.set_invul(ME, false);
    s.world.set_dead(ME, true);
    s.apply(P2, ME, k(9401));
    assert_eq!(s.hp(ME), 20.0);
}

#[test]
fn heal_of_the_datapack_restores_hp() {
    let Some(table) = datapack() else { return };
    // 1011 Heal.
    let skill = table.get(1011, 1).unwrap().clone();
    assert!(skill.is_magic());
    let mut s = Scene::new(table);
    s.world.set_current_hp(ME, 1.0, false);
    s.me.take();
    s.apply(P2, ME, key(&skill));
    assert!(s.hp(ME) > 1.0);
    let sent = restored(&s.me.take());
    assert_eq!(sent[0].0, SystemMessageId::S2HpHasBeenRestoredByC1);
}

// ---- PhysicalDamage ------------------------------------------------------------------------------------

/// The bounds `calcPhysDam` gives a plain (not critical) skill hit on a monster, from the
/// creatures' numbers: `76 * (pAtk + power) * proximity / pDef`, the random multiplier twice.
fn physical_bounds(s: &Scene, npc: ObjectId, power: f64) -> (f64, f64) {
    let a = s.world.stats(ME).unwrap();
    let t = s.world.stats(npc).unwrap();
    let proximity = match attacker_position(&s.world, ME, npc) {
        Position::Back => 1.2,
        Position::Front => 1.0,
        Position::Side => 1.1,
    };
    let base = (76.0 * (a.p_atk(Some(npc)) + power) * proximity) / t.p_def(Some(ME));
    // Level 1: the random damage without a weapon is `5 + (int) sqrt(1)` percent.
    (base * 0.94 * 0.94 - 1.0, base * 1.06 * 1.06 + 1.0)
}

#[test]
fn physical_damage_hurts_by_the_formula_and_tells_the_damage() {
    let mut s = Scene::inline(&strike(9410, r#"<effect name="PhysicalDamage"/>"#, ""));
    let npc = s.npc(9030);
    // The monster walks to its attacker once hit: measure first.
    let (low, high) = physical_bounds(&s, npc, 100.0);
    s.me.take();
    s.apply(ME, npc, k(9410));
    let taken = loss(BIG, s.hp(npc));
    assert!(taken >= low && taken <= high, "{taken} not in {low}..{high}");
    assert_eq!(damage_numbers(&s.me.take()), [taken as i32]);
}

#[test]
fn physical_damage_of_the_datapack_hurts() {
    let Some(table) = datapack() else { return };
    // A skill whose only general effect is `PhysicalDamage`, with no flying.
    let skill = table
        .iter()
        .find(|s| {
            let general = s.effects.get(EffectScope::General);
            general.len() == 1
                && general[0].name == "PhysicalDamage"
                && s.fly_radius == 0
                && s.fly_type.is_none()
                && s.base_crit_rate == 0
                && s.is_physical()
        })
        .expect("a plain physical skill")
        .clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9030);
    s.me.take();
    s.apply(ME, npc, key(&skill));
    let taken = loss(BIG, s.hp(npc));
    assert!(taken > 0.0);
    assert_eq!(damage_numbers(&s.me.take()), [taken as i32]);
}

#[test]
fn a_physical_critical_hit_is_announced() {
    // `baseCritRate` 1000 makes the STR based rate certain.
    let mut s = Scene::inline(&strike(
        9410,
        r#"<effect name="PhysicalDamage"/>"#,
        "<baseCritRate>1000</baseCritRate>",
    ));
    let npc = s.npc(9030);
    s.me.take();
    s.apply(ME, npc, k(9410));
    let sent = s.me.take();
    let ids = ids_of(&sent);
    assert!(ids.contains(&SystemMessageId::C1LandedACriticalHit));
    assert_eq!(damage_numbers(&sent), [loss(BIG, s.hp(npc)) as i32]);
}

#[test]
fn a_physical_skill_that_does_no_damage_says_the_attack_failed() {
    let mut s = Scene::inline(
        &strike(9410, r#"<effect name="PhysicalDamage"/>"#, "")
            .replace("<power>100</power>", "<power>-100000</power>"),
    );
    let npc = s.npc(9030);
    s.me.take();
    s.apply(ME, npc, k(9410));
    assert_eq!(s.hp(npc), BIG);
    assert_eq!(ids_of(&s.me.take()), [SystemMessageId::YourAttackHasFailed]);
}

#[test]
fn a_target_that_evades_the_skill_takes_nothing_and_both_players_are_told() {
    let xml = format!(
        "{}{}",
        strike(9410, r#"<effect name="PhysicalDamage"/>"#, ""),
        stat_buff(9411, "AVOID_UP", "pSkillEvas", "add", "100"),
    );
    let mut s = Scene::inline(&xml);
    s.apply(P2, P2, k(9411));
    let before = s.hp(P2);
    s.me.take();
    s.p2.take();
    s.apply(ME, P2, k(9410));
    assert_eq!(s.hp(P2), before);
    assert_eq!(
        restored(&s.me.take()),
        [(SystemMessageId::C1DodgesTheAttack, vec![name("P2")])]
    );
    assert_eq!(
        restored(&s.p2.take()),
        [(SystemMessageId::YouHaveAvoidedC1SAttack2, vec![name("P1")])]
    );
}

#[test]
fn a_suicide_skill_kills_its_caster() {
    let mut s = Scene::inline(&strike(
        9410,
        r#"<effect name="PhysicalDamage"/>"#,
        "<isSuicideAttack>true</isSuicideAttack>",
    ));
    let npc = s.npc(9030);
    s.apply(ME, npc, k(9410));
    assert!(s.world.is_dead(ME));
    assert!(s.hp(npc) < BIG);
}

#[test]
fn a_flying_skill_needs_a_caster_that_can_move() {
    // Java PhysicalDamage.onStart: `(flyRadius > 0 || flyType != null) && isMovementDisabled()`.
    let xml = format!(
        "{}{}",
        strike(
            9410,
            r#"<effect name="PhysicalDamage"/>"#,
            "<flyRadius>100</flyRadius>"
        ),
        STUN
    );
    let mut s = Scene::inline(&xml);
    let npc = s.npc(9030);
    s.apply(ME, ME, k(9390));
    assert!(s.world.is_movement_disabled(ME));
    s.me.take();
    s.apply(ME, npc, k(9410));
    assert_eq!(s.hp(npc), BIG);
    assert_eq!(
        ids_of(&s.me.take()),
        [SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms]
    );
}

#[test]
fn a_melee_skill_can_be_countered() {
    // Java Formulas.calcDamageReflected: VENGEANCE_SKILL_PHYSICAL_DAMAGE 100 always counters.
    // (A weak hit: a target that dies loses its buffs before the counter is rolled.)
    let xml = format!(
        "{}{}",
        strike(9410, r#"<effect name="PhysicalDamage"/>"#, "")
            .replace("<power>100</power>", "<power>5</power>"),
        stat_buff(9412, "CRITICAL_PROB_UP", "vengeancePdam", "add", "100"),
    );
    let mut s = Scene::inline(&xml);
    s.apply(P2, P2, k(9412));
    let my_total = s.hp(ME) + s.cp(ME);
    s.me.take();
    s.p2.take();
    s.apply(ME, P2, k(9410));
    assert!(s.hp(ME) + s.cp(ME) < my_total, "the counter hurt the attacker");
    assert!(ids_of(&s.me.take()).contains(&SystemMessageId::C1IsPerformingACounterattack));
    assert!(ids_of(&s.p2.take()).contains(&SystemMessageId::YouCounteredC1SAttack));
}

#[test]
fn a_ranged_or_magic_skill_is_not_countered() {
    let xml = format!(
        "{}{}",
        strike(9410, r#"<effect name="PhysicalDamage"/>"#, "")
            .replace("<castRange>40</castRange>", "<castRange>600</castRange>")
            .replace("<power>100</power>", "<power>5</power>"),
        stat_buff(9412, "CRITICAL_PROB_UP", "vengeancePdam", "add", "100"),
    );
    let mut s = Scene::inline(&xml);
    s.apply(P2, P2, k(9412));
    let my_total = s.hp(ME) + s.cp(ME);
    s.apply(ME, P2, k(9410));
    assert_eq!(s.hp(ME) + s.cp(ME), my_total);
}

#[test]
fn physical_damage_hp_link_grows_as_the_targets_hp_falls() {
    // Java Skill.getPower: PHYSICAL_ATTACK_HP_LINK scales the power by `2 - 2 * hp / maxHp`.
    let xml = strike(9420, r#"<effect name="PhysicalDamageHpLink"/>"#, "");
    let taken = |hp: f64| {
        let mut s = Scene::inline_seeded(&xml, 7);
        let npc = s.npc(9030);
        s.world.set_current_hp(npc, hp, false);
        s.apply(ME, npc, k(9420));
        loss(hp, s.hp(npc))
    };
    // At full HP the power is 0 and the hit is the plain P. Atk one; at 1 HP it nearly doubles.
    let (healthy, dying) = (taken(BIG), taken(BIG / 2.0));
    assert!(dying > healthy, "{dying} should be above {healthy}");
}

#[test]
fn physical_damage_hp_link_refuses_a_caster_that_cannot_move() {
    let xml = format!(
        "{}{}",
        strike(9420, r#"<effect name="PhysicalDamageHpLink"/>"#, ""),
        STUN
    );
    let mut s = Scene::inline(&xml);
    let npc = s.npc(9030);
    s.apply(ME, ME, k(9390));
    s.me.take();
    s.apply(ME, npc, k(9420));
    assert_eq!(s.hp(npc), BIG);
    assert_eq!(
        ids_of(&s.me.take()),
        [SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms]
    );
}

#[test]
fn physical_damage_hp_link_of_the_datapack_hurts() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "PhysicalDamageHpLink", |s| s.is_physical()).clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9001);
    s.world.set_current_hp(npc, 100.0, false);
    s.apply(ME, npc, key(&skill));
    assert!(s.hp(npc) < 100.0);
}

#[test]
fn physical_soul_damage_grows_by_4_8_percent_per_soul_up_to_the_skills_limit() {
    // Java PhysicalSoulDamage.onStart: `damage *= 1 + (souls * 0.048)`, souls capped by
    // `soulMaxConsumeCount`; the same seed makes the same base damage.
    let xml = strike(
        9430,
        r#"<effect name="PhysicalSoulDamage"/>"#,
        "<soulMaxConsumeCount>2</soulMaxConsumeCount>",
    );
    let damage_with = |souls: i32| {
        let mut s = Scene::inline_seeded(&xml, 11);
        let npc = s.npc(9030);
        if souls > 0 {
            s.world.increase_souls(ME, souls);
        }
        s.apply(ME, npc, k(9430));
        loss(BIG, s.hp(npc)) as i32
    };
    let base = damage_with(0);
    assert!(base > 0);
    assert_eq!(damage_with(1), (f64::from(base) * (1.0 + 0.048)) as i32);
    assert_eq!(damage_with(2), (f64::from(base) * (1.0 + 2.0 * 0.048)) as i32);
    // Three souls held, two usable.
    assert_eq!(damage_with(3), damage_with(2));
}

#[test]
fn physical_soul_damage_of_the_datapack_hurts() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "PhysicalSoulDamage", |s| {
        s.fly_radius == 0 && s.fly_type.is_none()
    })
    .clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9030);
    s.apply(ME, npc, key(&skill));
    assert!(s.hp(npc) < BIG);
}

#[test]
fn energy_damage_grows_with_the_force_charges() {
    // Java EnergyDamage: `energyChargesBoost = ((charges + chargeConsume) - 1) * 0.2 + 1`. The same
    // seed makes the same draws, so the damage is in the ratio of the boosts.
    let xml = strike(
        9440,
        r#"<effect name="EnergyDamage"><power>10</power><criticalChance>0</criticalChance></effect>"#,
        "",
    );
    let taken = |charges: i32| {
        let mut s = Scene::inline_seeded(&xml, 5);
        let npc = s.npc(9030);
        if charges > 0 {
            s.world.increase_charges(ME, charges, 8);
        }
        s.apply(ME, npc, k(9440));
        loss(BIG, s.hp(npc))
    };
    let (none, three) = (taken(0), taken(3));
    let boost = |charges: f64| ((charges - 1.0) * 0.2) + 1.0;
    assert!(none > 0.0);
    assert!(
        (three / none - boost(3.0) / boost(0.0)).abs() < 1e-9,
        "{three} / {none}"
    );
}

#[test]
fn energy_damage_is_for_players_only() {
    let mut s = Scene::inline(&strike(
        9440,
        r#"<effect name="EnergyDamage"><power>10</power></effect>"#,
        "",
    ));
    let npc = s.npc(9030);
    s.apply(npc, ME, k(9440));
    assert_eq!(s.hp(ME), 100.0);
}

#[test]
fn energy_damage_of_the_datapack_hurts() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "EnergyDamage", |_| true).clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9030);
    s.world.increase_charges(ME, 3, 8);
    s.apply(ME, npc, key(&skill));
    assert!(s.hp(npc) < BIG);
}

// ---- the blows -----------------------------------------------------------------------------------------

#[test]
fn a_fatal_blow_hurts_and_is_announced_as_critical() {
    // Java FatalBlow: `calcBlowSuccess` (blowChance 1000 always lands), then damage and the
    // critical message whatever the roll said.
    let mut s = Scene::inline(&strike(
        9450,
        r#"<effect name="FatalBlow"/>"#,
        "<blowChance>1000</blowChance>",
    ));
    let npc = s.npc(9030);
    s.me.take();
    s.apply(ME, npc, k(9450));
    let taken = loss(BIG, s.hp(npc));
    assert!(taken > 0.0);
    let sent = s.me.take();
    assert!(ids_of(&sent).contains(&SystemMessageId::C1LandedACriticalHit));
    assert_eq!(damage_numbers(&sent), [taken as i32]);
}

#[test]
fn a_blow_that_does_not_land_does_nothing() {
    let mut s = Scene::inline(&strike(
        9450,
        r#"<effect name="FatalBlow"/>"#,
        "<blowChance>0</blowChance>",
    ));
    let npc = s.npc(9030);
    s.apply(ME, npc, k(9450));
    assert_eq!(s.hp(npc), BIG);
}

#[test]
fn fatal_blow_of_the_datapack_hurts() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "FatalBlow", |s| s.blow_chance >= 100).clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9030);
    // The blow chance is 100 or more of a base rate that the side of the attack doubles or halves.
    for _ in 0..20 {
        s.apply(ME, npc, key(&skill));
    }
    assert!(s.hp(npc) < BIG);
}

#[test]
fn a_backstab_needs_the_attacker_not_in_front() {
    // Java Backstab.calcSuccess: `!effector.isInFrontOf(effected)`. From one side of a monster the
    // attacker is in front of it, from the other behind it.
    let xml = strike(
        9460,
        r#"<effect name="Backstab"/>"#,
        "<blowChance>1000</blowChance>",
    );
    let hit_with_heading = |heading: i32| {
        let mut s = Scene::inline(&xml);
        let npc = s.world.spawn_npc_at(9030, X + 80, Y, Z, heading).unwrap();
        // The monster turns to its attacker once hit: read the position first.
        let position = attacker_position(&s.world, ME, npc);
        s.apply(ME, npc, k(9460));
        (position, s.hp(npc) < BIG)
    };
    let (a, b) = (hit_with_heading(0), hit_with_heading(32768));
    for (position, hit) in [a, b] {
        assert_eq!(hit, position != Position::Front, "{position:?}");
    }
    assert_ne!(a.0, b.0);
}

#[test]
fn backstab_of_the_datapack_hurts_from_behind() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "Backstab", |_| true).clone();
    let mut s = Scene::new(table);
    // Whichever of the two headings puts the attacker behind the monster.
    let npcs: Vec<ObjectId> = [0, 32768]
        .into_iter()
        .map(|heading| s.world.spawn_npc_at(9030, X + 80, Y, Z, heading).unwrap())
        .collect();
    let behind = npcs
        .into_iter()
        .find(|npc| attacker_position(&s.world, ME, *npc) == Position::Back)
        .expect("one heading puts the attacker behind");
    for _ in 0..30 {
        s.apply(ME, behind, key(&skill));
    }
    assert!(s.hp(behind) < BIG);
}

#[test]
fn a_soul_blow_grows_with_the_souls() {
    let xml = strike(
        9470,
        r#"<effect name="SoulBlow"/>"#,
        "<blowChance>1000</blowChance><soulMaxConsumeCount>2</soulMaxConsumeCount>",
    );
    let taken = |souls: i32| {
        let mut s = Scene::inline_seeded(&xml, 3);
        let npc = s.npc(9030);
        if souls > 0 {
            s.world.increase_souls(ME, souls);
        }
        s.apply(ME, npc, k(9470));
        loss(BIG, s.hp(npc))
    };
    let (none, two, three) = (taken(0), taken(2), taken(3));
    assert!(none > 0.0);
    assert!((two / none - (1.0 + 2.0 * 0.048)).abs() < 1e-9);
    assert_eq!(two, three);
}

#[test]
fn soul_blow_of_the_datapack_hurts() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "SoulBlow", |_| true).clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9030);
    for _ in 0..20 {
        s.apply(ME, npc, key(&skill));
    }
    assert!(s.hp(npc) < BIG);
}

// ---- magical damage -------------------------------------------------------------------------------------

#[test]
fn magical_damage_hurts_and_tells_the_damage() {
    let mut s = Scene::inline(&spell(9500, r#"<effect name="MagicalDamage"/>"#, ""));
    let npc = s.npc(9030);
    s.me.take();
    s.apply(ME, npc, k(9500));
    let taken = loss(BIG, s.hp(npc));
    assert!(taken > 0.0);
    let sent = s.me.take();
    assert_eq!(damage_numbers(&sent), [taken as i32]);
}

#[test]
fn magical_damage_of_the_datapack_hurts() {
    let Some(table) = datapack() else { return };
    let skill = table
        .iter()
        .find(|s| {
            let general = s.effects.get(EffectScope::General);
            general.len() == 1 && general[0].name == "MagicalDamage" && !s.is_suicide_attack
        })
        .expect("a plain magic skill")
        .clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9030);
    s.me.take();
    s.apply(ME, npc, key(&skill));
    let taken = loss(BIG, s.hp(npc));
    assert!(taken > 0.0);
    assert_eq!(damage_numbers(&s.me.take()), [taken as i32]);
}

#[test]
fn a_magic_critical_hit_triples_the_damage_against_a_monster() {
    // The same seed, one scene whose magic critical cannot happen and one where it always does.
    let xml = format!(
        "{}{}",
        spell(9500, r#"<effect name="MagicalDamage"/>"#, ""),
        stat_buff(9501, "MAGIC_CRITICAL_UP", "mCritRate", "add", "500"),
    );
    let hit = |cap: i32, buffed: bool| {
        let mut s = scene_with_mcrit_cap(&xml, 9, cap);
        if buffed {
            s.apply(ME, ME, k(9501));
        }
        let npc = s.npc(9030);
        s.me.take();
        s.apply(ME, npc, k(9500));
        (loss(BIG, s.hp(npc)), ids_of(&s.me.take()))
    };
    let (plain, plain_ids) = hit(0, false);
    let (crit, crit_ids) = hit(100_000, true);
    assert!(!plain_ids.contains(&SystemMessageId::MagicCriticalHit));
    assert!(crit_ids.contains(&SystemMessageId::MagicCriticalHit));
    assert!((crit - 3.0 * plain).abs() <= 3.0, "{crit} vs {plain}");
}

#[test]
fn a_resisted_spell_tells_both_players() {
    // `magicSuccRes` 200 puts the success rate under 0: the check fails, and fails again for the
    // second roll, so the damage is 1 and both are told.
    let xml = format!(
        "{}{}",
        spell(9500, r#"<effect name="MagicalDamage"/>"#, ""),
        stat_buff(9502, "MD_UP", "magicSuccRes", "mul", "200"),
    );
    let mut s = Scene::inline(&xml);
    s.apply(P2, P2, k(9502));
    s.me.take();
    s.p2.take();
    s.apply(ME, P2, k(9500));
    let mine = messages(&s.me.take());
    assert_eq!(mine[0].id, SystemMessageId::C1HasResistedYourS2);
    assert_eq!(mine[0].params[0], name("P2"));
    let theirs = restored(&s.p2.take());
    assert!(theirs.contains(&(SystemMessageId::YouResistedC1SMagic, vec![name("P1")])));
}

#[test]
fn a_half_resisted_spell_says_the_attack_failed() {
    // `magicSuccRes` 50 gives a rate of 50: the first check fails half the time, the second
    // succeeds half the time (and the target is within 9 levels): "YOUR_ATTACK_HAS_FAILED", half damage.
    let xml = format!(
        "{}{}",
        spell(9500, r#"<effect name="MagicalDamage"/>"#, ""),
        stat_buff(9502, "MD_UP", "magicSuccRes", "mul", "50"),
    );
    let (mine, theirs) = first_seed(|seed| {
        let mut s = Scene::inline_seeded(&xml, seed);
        s.apply(P2, P2, k(9502));
        s.me.take();
        s.p2.take();
        s.apply(ME, P2, k(9500));
        let mine = ids_of(&s.me.take());
        mine.contains(&SystemMessageId::YourAttackHasFailed)
            .then(|| (mine, ids_of(&s.p2.take())))
    });
    assert!(!mine.contains(&SystemMessageId::C1HasResistedYourS2));
    assert!(theirs.contains(&SystemMessageId::YouResistedC1SMagic));
}

#[test]
fn magical_damage_by_abnormal_grows_with_the_targets_buffs() {
    // `damage *= ((buffs * 0.3) + 1.3) / 4`.
    let xml = format!(
        "{}{}{}",
        spell(9510, r#"<effect name="MagicalDamageByAbnormal"/>"#, ""),
        stat_buff(9511, "PA_UP", "pAtk", "mul", "1.0"),
        stat_buff(9512, "PD_UP", "pDef", "mul", "1.0"),
    );
    let taken = |buffs: i32| {
        let mut s = scene_with_mcrit_cap(&xml, 21, 0);
        let npc = s.npc(9030);
        for id in [9511, 9512].into_iter().take(usize::try_from(buffs).unwrap()) {
            // The buffs are put on the monster (the skills name their own caster as target).
            s.apply(npc, npc, k(id));
        }
        s.apply(ME, npc, k(9510));
        loss(BIG, s.hp(npc))
    };
    let (none, two) = (taken(0), taken(2));
    assert!(none > 0.0);
    // 0.475 / 0.325, give or take the two integer casts.
    assert!((two - none * (0.475 / 0.325)).abs() <= 2.0, "{two} vs {none}");
}

#[test]
fn magical_damage_by_abnormal_of_the_datapack_hurts() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "MagicalDamageByAbnormal", |_| true).clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9030);
    s.apply(ME, npc, key(&skill));
    assert!(s.hp(npc) < BIG);
}

#[test]
fn magical_soul_damage_grows_by_4_8_percent_per_soul() {
    let xml = spell(
        9520,
        r#"<effect name="MagicalSoulDamage"/>"#,
        "<soulMaxConsumeCount>2</soulMaxConsumeCount>",
    );
    let damage_with = |souls: i32| {
        let mut s = Scene::inline_seeded(&xml, 13);
        let npc = s.npc(9030);
        if souls > 0 {
            s.world.increase_souls(ME, souls);
        }
        s.apply(ME, npc, k(9520));
        loss(BIG, s.hp(npc)) as i32
    };
    let base = damage_with(0);
    assert!(base > 0);
    assert_eq!(damage_with(2), (f64::from(base) * (1.0 + 2.0 * 0.048)) as i32);
    assert_eq!(damage_with(5), damage_with(2));
}

#[test]
fn magical_soul_damage_of_the_datapack_hurts() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "MagicalSoulDamage", |_| true).clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9030);
    s.apply(ME, npc, key(&skill));
    assert!(s.hp(npc) < BIG);
}

#[test]
fn death_link_hits_harder_the_less_hp_the_caster_has() {
    // Java Skill.getPower: DEATH_LINK scales the power by `2 - 2 * hp / maxHp` of the caster:
    // nothing at full HP.
    let xml = spell(9530, r#"<effect name="DeathLink"/>"#, "");
    let taken = |hp: f64| {
        let mut s = Scene::inline_seeded(&xml, 17);
        let npc = s.npc(9030);
        s.world.set_current_hp(ME, hp, false);
        s.apply(ME, npc, k(9530));
        loss(BIG, s.hp(npc))
    };
    assert_eq!(taken(100.0), 0.0);
    assert!(taken(10.0) > 0.0);
}

#[test]
fn death_link_of_the_datapack_hurts_a_weak_caster() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "DeathLink", |s| !s.is_suicide_attack).clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9030);
    s.world.set_current_hp(ME, 5.0, false);
    s.apply(ME, npc, key(&skill));
    assert!(s.hp(npc) < BIG);
}

#[test]
fn hp_drain_heals_the_caster_by_power_per_hp_taken() {
    let mut s = Scene::inline(&spell(
        9540,
        r#"<effect name="HpDrain"><power>0.5</power></effect>"#,
        "",
    ));
    let npc = s.npc(9030);
    s.world.set_current_hp(ME, 10.0, false);
    s.me.take();
    s.apply(ME, npc, k(9540));
    let taken = loss(BIG, s.hp(npc));
    assert!(taken > 0.0);
    // No CP on a monster: the drain is what the hit took.
    assert_eq!(s.hp(ME), (10.0 + 0.5 * taken).min(100.0));
}

#[test]
fn hp_drain_leaves_the_cp_of_a_player_target_out_of_the_drain() {
    // `cp > 0`: the drain is `damage - cp` (0 when the hit is below the CP).
    // A weak spell, so the target survives it and the HP and CP taken add up to the damage.
    let mut s = Scene::inline(
        &spell(9540, r#"<effect name="HpDrain"><power>1</power></effect>"#, "")
            .replace("<power>50</power>", "<power>8</power>"),
    );
    s.world.set_current_hp(ME, 10.0, false);
    s.apply(ME, P2, k(9540));
    let cp_taken = 20.0 - s.cp(P2);
    let hp_taken = 100.0 - s.hp(P2);
    let damage = cp_taken + hp_taken;
    assert!(
        s.hp(P2) > 0.0 && damage > 20.0,
        "the hit is above the CP and below the HP"
    );
    assert_eq!(s.hp(ME), 10.0 + (damage - 20.0));
}

#[test]
fn a_half_resisted_drain_says_so() {
    let xml = format!(
        "{}{}",
        spell(9540, r#"<effect name="HpDrain"><power>0.5</power></effect>"#, ""),
        stat_buff(9502, "MD_UP", "magicSuccRes", "mul", "50"),
    );
    let (mine, theirs) = first_seed(|seed| {
        let mut s = Scene::inline_seeded(&xml, seed);
        s.apply(P2, P2, k(9502));
        s.me.take();
        s.p2.take();
        s.apply(ME, P2, k(9540));
        let mine = ids_of(&s.me.take());
        mine.contains(&SystemMessageId::DrainWasOnly50PercentSuccessful)
            .then(|| (mine, ids_of(&s.p2.take())))
    });
    assert!(!mine.contains(&SystemMessageId::YourAttackHasFailed));
    assert!(theirs.contains(&SystemMessageId::YouResistedC1SDrain));
}

#[test]
fn hp_drain_of_the_datapack_hurts() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "HpDrain", |_| true).clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9030);
    s.apply(ME, npc, key(&skill));
    assert!(s.hp(npc) < BIG);
}

#[test]
fn magical_damage_mp_drains_mp_and_tells_both() {
    let xml = spell(9550, r#"<effect name="MagicalDamageMp"/>"#, "");
    let (mp_taken, mine, theirs) = first_seed(|seed| {
        // `calcMagicAffected` rolls a gaussian: rarely it fails.
        let mut s = Scene::inline_seeded(&xml, seed);
        s.me.take();
        s.p2.take();
        s.apply(ME, P2, k(9550));
        let mine = restored(&s.me.take());
        let theirs = restored(&s.p2.take());
        let taken = 50.0 - s.mp(P2);
        (taken > 0.0).then_some((taken, mine, theirs))
    });
    assert!(mine.contains(&(
        SystemMessageId::YourOpponentSMpWasReducedByS1,
        vec![int(mp_taken as i32)]
    )));
    assert!(theirs.contains(&(
        SystemMessageId::S2SMpHasBeenDrainedByC1,
        vec![name("P1"), int(mp_taken as i32)]
    )));
}

#[test]
fn magical_damage_mp_skips_an_invulnerable_target_and_a_debuff_it_is_immune_to() {
    let xml = format!(
        "{}{}",
        spell(9550, r#"<effect name="MagicalDamageMp"/>"#, ""),
        stat_buff(9551, "MD_UP", "debuffImmunity", "add", "1"),
    );
    let mut s =
        Scene::inline(&xml.replace("<power>50</power>", "<power>50</power><isDebuff>true</isDebuff>"));
    s.world.set_invul(P2, true);
    s.apply(ME, P2, k(9550));
    assert_eq!(s.mp(P2), 50.0);
    s.world.set_invul(P2, false);
    s.apply(P2, P2, k(9551));
    s.me.take();
    s.p2.take();
    s.apply(ME, P2, k(9550));
    assert_eq!(s.mp(P2), 50.0);
    assert_eq!(ids_of(&s.me.take()), [SystemMessageId::YourAttackHasFailed]);
    assert_eq!(
        restored(&s.p2.take()),
        [(SystemMessageId::C1ResistedC2SMagic, vec![name("P2"), name("P1")])]
    );
}

#[test]
fn magical_damage_mp_of_the_datapack_drains() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "MagicalDamageMp", |s| !s.is_debuff).clone();
    // `calcMagicAffected` rolls a gaussian: some seeds fail it.
    first_seed(|seed| {
        let mut s = Scene::seeded(table.clone(), seed);
        s.apply(ME, P2, key(&skill));
        (s.mp(P2) < 50.0).then_some(())
    });
}

#[test]
fn the_shield_deflect_stat_turns_a_spell_on_its_caster() {
    // Java: `VENGEANCE_SKILL_MAGIC_DAMAGE > Rnd.get(100)` (100 always) reflects all the damage.
    let xml = format!(
        "{}{}",
        spell(9500, r#"<effect name="MagicalDamage"/>"#, ""),
        stat_buff(9560, "MD_UP", "vengeanceMdam", "add", "100"),
    );
    let mut s = Scene::inline(&xml);
    s.apply(P2, P2, k(9560));
    let (mine, theirs) = (s.hp(ME) + s.cp(ME), s.hp(P2) + s.cp(P2));
    s.apply(ME, P2, k(9500));
    assert!(s.hp(ME) + s.cp(ME) < mine);
    assert_eq!(s.hp(P2) + s.cp(P2), theirs);
}

// ---- Lethal -----------------------------------------------------------------------------------------------

fn lethal(full: i32, half: i32) -> String {
    spell(
        9600,
        &format!(
            r#"<effect name="Lethal"><fullLethal>{full}</fullLethal><halfLethal>{half}</halfLethal></effect>"#
        ),
        "",
    )
}

#[test]
fn a_lethal_strike_leaves_a_monster_with_1_hp() {
    let mut s = Scene::inline(&lethal(100, 0));
    let npc = s.npc(9001);
    s.me.take();
    s.apply(ME, npc, k(9600));
    assert_eq!(s.hp(npc), 1.0);
    assert_eq!(
        ids_of(&s.me.take()),
        [SystemMessageId::YourLethalStrikeWasSuccessful]
    );
}

#[test]
fn a_lethal_strike_leaves_a_player_with_1_hp_and_1_cp() {
    let mut s = Scene::inline(&lethal(100, 0));
    s.me.take();
    s.p2.take();
    s.apply(ME, P2, k(9600));
    assert_eq!((s.hp(P2), s.cp(P2)), (1.0, 1.0));
    assert!(ids_of(&s.p2.take()).contains(&SystemMessageId::LethalStrike));
    assert_eq!(
        ids_of(&s.me.take()),
        [SystemMessageId::YourLethalStrikeWasSuccessful]
    );
}

#[test]
fn a_half_kill_halves_a_monsters_hp_and_takes_a_players_cp() {
    let mut s = Scene::inline(&lethal(0, 100));
    let npc = s.npc(9001);
    s.me.take();
    s.apply(ME, npc, k(9600));
    assert_eq!(s.hp(npc), 150.0);
    assert_eq!(ids_of(&s.me.take()), [SystemMessageId::HalfKill]);
    s.p2.take();
    s.apply(ME, P2, k(9600));
    assert_eq!((s.hp(P2), s.cp(P2)), (100.0, 1.0));
    assert_eq!(
        ids_of(&s.p2.take()),
        [
            SystemMessageId::HalfKill,
            SystemMessageId::YourCpWasDrainedBecauseYouWereHitWithAHalfKillSkill
        ]
    );
}

#[test]
fn lethal_does_nothing_without_a_chance_or_against_what_it_cannot_kill() {
    let mut s = Scene::inline(&lethal(0, 0));
    let npc = s.npc(9001);
    s.apply(ME, npc, k(9600));
    assert_eq!(s.hp(npc), 300.0);

    // A raid boss, an invulnerable player and a target six levels above the skill are all safe.
    let mut s = Scene::inline(&lethal(100, 100));
    let boss = s.npc(9020);
    s.apply(ME, boss, k(9600));
    assert_eq!(s.hp(boss), 300.0);
    s.world.set_invul(P2, true);
    s.apply(ME, P2, k(9600));
    assert_eq!((s.hp(P2), s.cp(P2)), (100.0, 20.0));
    let high = s.npc(9010);
    s.apply(ME, high, k(9600));
    assert_eq!(s.hp(high), 300.0);
}

#[test]
fn lethal_of_the_datapack_runs() {
    let Some(table) = datapack() else { return };
    let skill = find(&table, "Lethal", |_| true).clone();
    let mut s = Scene::new(table);
    let npc = s.npc(9001);
    for _ in 0..30 {
        s.apply(ME, npc, key(&skill));
    }
    assert!(s.hp(npc) < 300.0);
}
