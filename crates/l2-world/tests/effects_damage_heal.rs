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
    format!(
        r#"<npc id="{id}" type="Monster" level="{level}" name="N{id}">
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
/// `ManaHealByLevel`).
fn npcs() -> String {
    format!(
        "<?xml version=\"1.0\"?><list>{}{}</list>",
        npc_xml(9001, 1, 300, 40),
        npc_xml(9010, 20, 300, 400),
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
        let mut world = World::new(WorldRng::from_seed(20_261_020), Box::new(NoOutbox));
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
        let table = SkillTemplates::from_xml(
            &format!("<list>{xml}</list>"),
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        );
        Self::new(Arc::new(table))
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
