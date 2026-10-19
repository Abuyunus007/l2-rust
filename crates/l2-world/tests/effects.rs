#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The effect engine (M5-EFFECT-01): buff infos on the effect list, abnormal type stacking and
//! the herb rule, the slot limits, durations (`BuffFinishTask`) and ticks (`EffectTickTask`) on the
//! timer wheel, stat functions on and off, the icon packets, death, and the `Buff` handler end to
//! end with the real Shield template (skill 1040 of `01000-01099.xml`, trimmed to what the engine
//! reads). Expected behaviour comes from the Java code named in each test.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_data::{ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillTemplates};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    AbnormalStatusUpdate, ShortBuffStatusUpdate, SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, FuncOwner, Stat, StatData};
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

/// Skill 1040 is the datapack's Shield without its comments and enchant routes. The others are
/// made for one rule each.
const SKILLS: &str = r##"<list>
<skill id="1040" levels="3" name="Shield">
	<table name="#abnormalLevels">1 2 3</table>
	<table name="#effectPoints">121 243 418</table>
	<table name="#magicLevel">7 25 44</table>
	<table name="#mpConsume">8 18 31</table>
	<table name="#mpInitialConsume">2 5 8</table>
	<table name="#rate">1.08 1.12 1.15</table>
	<icon>icon.skill1040</icon>
	<operateType>A2</operateType>
	<targetType>ONE</targetType>
	<abnormalLevel>#abnormalLevels</abnormalLevel>
	<abnormalTime>1200</abnormalTime>
	<abnormalType>PD_UP</abnormalType>
	<castRange>400</castRange>
	<effectPoint>#effectPoints</effectPoint>
	<effectRange>900</effectRange>
	<hitTime>4000</hitTime>
	<isMagic>1</isMagic>
	<magicLevel>#magicLevel</magicLevel>
	<mpConsume>#mpConsume</mpConsume>
	<mpInitialConsume>#mpInitialConsume</mpInitialConsume>
	<reuseDelay>2000</reuseDelay>
	<effects>
		<effect name="Buff">
			<mul stat="pDef">#rate</mul>
		</effect>
	</effects>
</skill>
<skill id="9001" levels="1" name="Might"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9002" levels="1" name="Empower"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>MA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="mAtk" val="1.1"/></effect></effects></skill>
<skill id="9003" levels="1" name="Haste"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>ATTACK_TIME_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtkSpd" val="1.2"/></effect></effects></skill>
<skill id="9010" levels="1" name="Herb of Defence"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>10</abnormalTime><abnormalType>PD_UP</abnormalType><abnormalLevel>5</abnormalLevel>
<abnormalInstant>true</abnormalInstant>
<effects><effect name="Buff"><mul stat="pDef" val="2"/></effect></effects></skill>
<skill id="9020" levels="1" name="Dance A"><operateType>A2</operateType><targetType>SELF</targetType><isMagic>3</isMagic>
<abnormalTime>60</abnormalTime><abnormalType>CRITICAL_DMG_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="critDmg" val="1.1"/></effect></effects></skill>
<skill id="9021" levels="1" name="Dance B"><operateType>A2</operateType><targetType>SELF</targetType><isMagic>3</isMagic>
<abnormalTime>60</abnormalTime><abnormalType>CRITICAL_PROB_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="critRate" val="1.1"/></effect></effects></skill>
<skill id="9030" levels="1" name="Toggle stays"><operateType>T</operateType><targetType>SELF</targetType>
<effects><effect name="Buff" ticks="5"><mul stat="mDef" val="1.5"/></effect></effects></skill>
<skill id="9031" levels="1" name="Toggle ends"><operateType>T</operateType><targetType>SELF</targetType>
<effects><effect name="Relax" ticks="5"/></effects></skill>
<skill id="9040" levels="1" name="Weakness"><operateType>A2</operateType><targetType>SELF</targetType><isDebuff>true</isDebuff>
<abnormalTime>30</abnormalTime><abnormalType>POISON</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="0.5"/></effect></effects></skill>
<skill id="9050" levels="1" name="Potion"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>15</abnormalTime><abnormalType>HP_RECOVER</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"/></effects></skill>
<skill id="9060" levels="1" name="Lasting"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>MD_UP</abnormalType><abnormalLevel>1</abnormalLevel><stayAfterDeath>true</stayAfterDeath>
<effects><effect name="Buff"><mul stat="mDef" val="1.1"/></effect></effects></skill>
<skill id="9070" levels="1" name="Mixed"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>SPEED_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Heal"><power>10</power></effect><effect name="NoSuchEffect"/><effect name="MaxHp"><add stat="maxHp" val="7"/></effect></effects></skill>
<skill id="9080" levels="1" name="Instant"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><mul stat="pAtk" val="9"/></effect></effects></skill>
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

fn player() -> PlayerData {
    PlayerData {
        object_id: ME,
        account: "acc".into(),
        name: "Me".into(),
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
        x: 1000,
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
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn world_with(effects: EffectSettings, skills: SkillTemplates) -> (World, Log) {
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(1), Box::new(NoOutbox));
    world.set_settings(WorldSettings {
        effects,
        ..WorldSettings::default()
    });
    world.set_static_data(Box::new(Data(stat_data(skills))));
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(player()),
        sink: Box::new(Sink(log.clone())),
        ip: None,
    });
    advance(&mut world, 1000);
    log.take();
    (world, log)
}

fn world() -> (World, Log) {
    world_with(EffectSettings::default(), skills())
}

/// Runs the world for `ms` milliseconds (50 ms ticks).
fn advance(world: &mut World, ms: u64) {
    for _ in 0..(ms / 50) {
        world.tick();
    }
}

fn buff(world: &mut World, id: i32, level: i32) {
    world.apply_effects(ME, ME, SkillKey::new(id, level));
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

fn messages(packets: &[Vec<u8>]) -> Vec<SystemMessage> {
    of(packets, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p))
        .collect()
}

fn message_ids(packets: &[Vec<u8>]) -> Vec<SystemMessageId> {
    messages(packets).into_iter().map(|m| m.id).collect()
}

/// The last `AbnormalStatusUpdate` as `(display id, level, time)`.
fn last_icons(packets: &[Vec<u8>]) -> Vec<(i32, i32, i32)> {
    let asu: AbnormalStatusUpdate = decode(of(packets, "ABNORMAL_STATUS_UPDATE").last().unwrap());
    asu.effects
        .iter()
        .map(|e| (e.display_id, e.display_level, e.time))
        .collect()
}

/// The skill ids of one queue.
fn queue(world: &World, kind: ListKind) -> Vec<i32> {
    world
        .effect_list(ME)
        .map(|l| l.queue(kind).iter().map(|i| i.skill().id).collect())
        .unwrap_or_default()
}

fn p_def(world: &World) -> f64 {
    world.stats(ME).unwrap().p_def(None)
}

// ---- the Buff handler, end to end ----------------------------------------------------------------

#[test]
fn shield_buffs_pdef_shows_its_icon_and_wears_off_after_its_time() {
    let (mut world, log) = world();
    let before = p_def(&world);
    buff(&mut world, 1040, 3);
    // `Buff` keeps its stat function: P. Def. * 1.15, owned by the effect.
    assert_eq!(p_def(&world), before * 1.15);
    assert_eq!(queue(&world, ListKind::Buffs), [1040]);
    let owner = world
        .player(ME)
        .unwrap()
        .core
        .calculators
        .funcs(Stat::PowerDefence)
        .iter()
        .map(|f| f.owner)
        .find(|o| matches!(o, FuncOwner::Effect { .. }));
    assert_eq!(
        owner,
        Some(FuncOwner::Effect {
            skill_id: 1040,
            skill_level: 3,
            scope: l2_data::skill::EffectScope::General,
            index: 0
        })
    );
    // "$s1's effect can be felt." at once (the player entered the world), the stat broadcast,
    // and the icons 300 ms later.
    let sent = log.take();
    let felt = &messages(&sent)[0];
    assert_eq!(felt.id, SystemMessageId::S1SEffectCanBeFelt);
    assert_eq!(
        felt.params,
        [SystemMessageParam::SkillName { id: 1040, level: 3 }]
    );
    assert!(of(&sent, "ABNORMAL_STATUS_UPDATE").is_empty());
    advance(&mut world, 300);
    assert_eq!(last_icons(&log.take()), [(1040, 3, 1200)]);

    // `getTime()` counts whole seconds since the start.
    advance(&mut world, 10_000);
    world.update_effect_icons(ME, false);
    advance(&mut world, 300);
    assert_eq!(last_icons(&log.take()), [(1040, 3, 1190)]);

    // `BuffFinishTask`: counted once a second from the first run, removed when the count passes
    // the abnormal time.
    advance(&mut world, 1_189_000);
    assert!(world.is_affected_by_skill(ME, 1040));
    advance(&mut world, 1_000);
    assert!(!world.is_affected_by_skill(ME, 1040));
    assert_eq!(p_def(&world), before);
    let sent = log.take();
    let worn = messages(&sent);
    assert_eq!(worn.last().unwrap().id, SystemMessageId::S1HasWornOff);
    // The icon update 300 ms after the removal (within that second) lists nothing.
    assert_eq!(last_icons(&sent), []);
}

#[test]
fn an_active_skill_without_continuous_operate_type_leaves_no_buff() {
    let (mut world, _log) = world();
    let before = world.stats(ME).unwrap().p_atk(None);
    // A1: `addContinuousEffects` is false, the continuous `Buff` never joins an info.
    buff(&mut world, 9080, 1);
    assert!(world.effect_list(ME).is_none_or(EffectList::is_empty));
    assert_eq!(world.stats(ME).unwrap().p_atk(None), before);
}

#[test]
fn unported_handlers_do_nothing_and_unknown_names_are_dropped() {
    let (mut world, _log) = world();
    let before = world.stats(ME).unwrap().max_hp();
    buff(&mut world, 9070, 1);
    let list = world.effect_list(ME).unwrap();
    let info = &list.buffs()[0];
    // Heal is instant (applied, a no-op stand-in), NoSuchEffect has no handler, MaxHp is
    // continuous and keeps its stat function.
    assert_eq!(info.effects().len(), 1);
    assert_eq!(info.effects()[0].index, 2);
    assert_eq!(world.stats(ME).unwrap().max_hp(), before + 7);
}

// ---- stacking --------------------------------------------------------------------------------------

#[test]
fn a_lower_abnormal_level_does_not_replace_a_higher_one() {
    let (mut world, log) = world();
    buff(&mut world, 1040, 3);
    log.take();
    buff(&mut world, 1040, 1);
    let list = world.effect_list(ME).unwrap();
    assert_eq!(list.buffs().len(), 1);
    assert_eq!(list.buffs()[0].skill(), SkillKey::new(1040, 3));
    assert!(message_ids(&log.take()).is_empty());
}

#[test]
fn an_equal_or_higher_level_replaces_and_goes_to_the_end() {
    let (mut world, log) = world();
    buff(&mut world, 1040, 1);
    buff(&mut world, 9001, 1);
    log.take();
    buff(&mut world, 1040, 2);
    let list = world.effect_list(ME).unwrap();
    assert_eq!(
        list.buffs().iter().map(|i| i.skill()).collect::<Vec<_>>(),
        [SkillKey::new(9001, 1), SkillKey::new(1040, 2)]
    );
    // The old one ends NORMAL ("worn off"), the new one lands.
    assert_eq!(
        message_ids(&log.take()),
        [SystemMessageId::S1HasWornOff, SystemMessageId::S1SEffectCanBeFelt]
    );
    let before = {
        let (w, _) = world_with(EffectSettings::default(), skills());
        p_def(&w)
    };
    assert_eq!(p_def(&world), before * 1.12);
    assert_eq!(
        world
            .effect_list(ME)
            .unwrap()
            .info_by_abnormal_type(l2_data::stats::AbnormalType::PdUp)
            .unwrap()
            .skill(),
        SkillKey::new(1040, 2)
    );
}

#[test]
fn an_herb_hides_the_buff_of_its_type_until_it_ends() {
    let (mut world, log) = world();
    let base = p_def(&world);
    buff(&mut world, 1040, 3);
    buff(&mut world, 9010, 1);
    let list = world.effect_list(ME).unwrap();
    // The shield stays in the list, not in use, without its stats; the herb is counted.
    assert_eq!(queue(&world, ListKind::Buffs), [1040, 9010]);
    assert!(!list.buffs()[0].is_in_use());
    assert_eq!(list.hidden_buffs_count(), 1);
    assert_eq!(list.buff_count(), 1);
    assert_eq!(p_def(&world), base * 2.0);
    log.take();
    advance(&mut world, 300);
    assert_eq!(last_icons(&log.take()), [(9010, 1, 10)]);
    // The herb wears off after 10 s: the shield is back with its stats and its own time.
    advance(&mut world, 10_000);
    let list = world.effect_list(ME).unwrap();
    assert_eq!(queue(&world, ListKind::Buffs), [1040]);
    assert!(list.buffs()[0].is_in_use());
    assert_eq!(list.hidden_buffs_count(), 0);
    assert_eq!(p_def(&world), base * 1.15);
    // 10.6 s after the shield landed.
    advance(&mut world, 300);
    assert_eq!(last_icons(&log.take()), [(1040, 3, 1190)]);
}

#[test]
fn blocked_abnormal_types_are_not_added() {
    let (mut world, _log) = world();
    buff(&mut world, 9001, 1);
    world.add_blocked_abnormal_types(ME, [l2_data::stats::AbnormalType::PdUp]);
    buff(&mut world, 1040, 3);
    assert_eq!(queue(&world, ListKind::Buffs), [9001]);
    assert!(world.remove_blocked_abnormal_types(ME, [l2_data::stats::AbnormalType::PdUp]));
    buff(&mut world, 1040, 3);
    assert_eq!(queue(&world, ListKind::Buffs), [9001, 1040]);
}

// ---- limits ----------------------------------------------------------------------------------------

#[test]
fn a_full_buff_bar_drops_its_oldest_buff() {
    let (mut world, log) = world_with(
        EffectSettings {
            buffs_max_amount: 2,
            ..EffectSettings::default()
        },
        skills(),
    );
    buff(&mut world, 9001, 1);
    buff(&mut world, 9002, 1);
    log.take();
    buff(&mut world, 9003, 1);
    assert_eq!(queue(&world, ListKind::Buffs), [9002, 9003]);
    // `stopAndRemove(true, info, queue)` is REMOVED.
    assert_eq!(
        message_ids(&log.take()),
        [
            SystemMessageId::TheEffectOfS1HasBeenRemoved,
            SystemMessageId::S1SEffectCanBeFelt
        ]
    );
    // A replacement of the same type does not count against the limit (`doesStack`).
    buff(&mut world, 9003, 1);
    assert_eq!(queue(&world, ListKind::Buffs), [9002, 9003]);
    assert_eq!(world.max_buff_count(ME), 2);
}

#[test]
fn dances_have_their_own_limit() {
    let (mut world, _log) = world_with(
        EffectSettings {
            buffs_max_amount: 1,
            dances_max_amount: 1,
            ..EffectSettings::default()
        },
        skills(),
    );
    buff(&mut world, 9001, 1);
    buff(&mut world, 9020, 1);
    buff(&mut world, 9021, 1);
    assert_eq!(queue(&world, ListKind::Buffs), [9001]);
    assert_eq!(queue(&world, ListKind::Dances), [9021]);
}

#[test]
fn a_healing_potion_is_the_short_buff_not_an_icon() {
    let (mut world, log) = world_with(
        EffectSettings {
            buffs_max_amount: 1,
            ..EffectSettings::default()
        },
        skills(),
    );
    buff(&mut world, 9001, 1);
    // A potion is never limited and never drops a buff.
    buff(&mut world, 9050, 1);
    assert_eq!(queue(&world, ListKind::Buffs), [9001, 9050]);
    log.take();
    advance(&mut world, 300);
    let sent = log.take();
    let short: ShortBuffStatusUpdate = decode(of(&sent, "SHORT_BUFF_STATUS_UPDATE")[0]);
    assert_eq!((short.skill_id, short.skill_level, short.duration), (9050, 1, 15));
    assert_eq!(last_icons(&sent), [(9001, 1, 60)]);
    // The short buff does not count as a buff.
    assert_eq!(world.effect_list(ME).unwrap().buff_count(), 1);
    // When it ends the bar is reset.
    advance(&mut world, 16_000);
    let sent = log.take();
    let reset: ShortBuffStatusUpdate = decode(of(&sent, "SHORT_BUFF_STATUS_UPDATE").last().unwrap());
    assert_eq!(reset, ShortBuffStatusUpdate::RESET_SHORT_BUFF);
    assert!(world.effect_list(ME).unwrap().short_buff().is_none());
}

// ---- toggles and ticks -----------------------------------------------------------------------------

#[test]
fn a_toggle_ticks_and_stops_when_its_tick_says_so() {
    let (mut world, log) = world();
    let m_def = world.stats(ME).unwrap().m_def(None, None);
    // `Buff.onActionTime` of a toggle is true: it keeps going.
    buff(&mut world, 9030, 1);
    assert_eq!(queue(&world, ListKind::Toggles), [9030]);
    assert_eq!(world.stats(ME).unwrap().m_def(None, None), m_def * 1.5);
    advance(&mut world, 60_000);
    assert_eq!(queue(&world, ListKind::Toggles), [9030]);
    // A toggle has no abnormal time: its icon time is -1 and counts down.
    world.update_effect_icons(ME, false);
    log.take();
    advance(&mut world, 300);
    assert_eq!(last_icons(&log.take()), [(9030, 1, -61)]);

    // `Relax` is not ported: its `onActionTime` is the default `false`, so the toggle ends at
    // its first tick, 5 * 666 ms = 3330 ms later (67 ticks).
    buff(&mut world, 9031, 1);
    log.take();
    for _ in 0..66 {
        world.tick();
    }
    assert_eq!(queue(&world, ListKind::Toggles), [9030, 9031]);
    world.tick();
    assert_eq!(queue(&world, ListKind::Toggles), [9030]);
    assert_eq!(message_ids(&log.take()), [SystemMessageId::S1HasBeenAborted]);
}

#[test]
fn removing_a_toggle_skill_stops_its_effect() {
    let (mut world, _log) = world();
    buff(&mut world, 9030, 1);
    world.stop_all_toggles(ME, true);
    assert!(queue(&world, ListKind::Toggles).is_empty());
}

// ---- icons, flags, death -------------------------------------------------------------------------

#[test]
fn icons_are_buffs_then_dances_then_toggles_then_debuffs() {
    let (mut world, log) = world();
    buff(&mut world, 9040, 1);
    buff(&mut world, 9030, 1);
    buff(&mut world, 9020, 1);
    buff(&mut world, 9001, 1);
    assert_eq!(queue(&world, ListKind::Debuffs), [9040]);
    log.take();
    advance(&mut world, 300);
    let icons: Vec<i32> = last_icons(&log.take()).into_iter().map(|i| i.0).collect();
    assert_eq!(icons, [9001, 9020, 9030, 9040]);
}

#[test]
fn death_ends_everything_but_what_stays_after_death() {
    let (mut world, log) = world();
    buff(&mut world, 9001, 1);
    buff(&mut world, 9060, 1);
    buff(&mut world, 9040, 1);
    log.take();
    world.do_die(ME, None);
    assert_eq!(queue(&world, ListKind::Buffs), [9060]);
    assert!(queue(&world, ListKind::Debuffs).is_empty());
    let ids = message_ids(&log.take());
    assert_eq!(
        ids.iter()
            .filter(|i| **i == SystemMessageId::TheEffectOfS1HasBeenRemoved)
            .count(),
        2
    );
    // Nothing lands on the dead.
    buff(&mut world, 9002, 1);
    assert_eq!(queue(&world, ListKind::Buffs), [9060]);
}

#[test]
fn removing_by_skill_id_and_by_abnormal_type() {
    let (mut world, log) = world();
    buff(&mut world, 1040, 3);
    buff(&mut world, 9001, 1);
    log.take();
    world.stop_skill_effects(ME, SkillFinishType::Removed, 9001);
    assert_eq!(
        message_ids(&log.take()),
        [SystemMessageId::TheEffectOfS1HasBeenRemoved]
    );
    assert!(world.stop_skill_effects_by_abnormal(
        ME,
        SkillFinishType::Silent,
        l2_data::stats::AbnormalType::PdUp
    ));
    assert!(message_ids(&log.take()).is_empty());
    assert!(world.effect_list(ME).unwrap().is_empty());
    assert!(!world.stop_skill_effects_by_abnormal(
        ME,
        SkillFinishType::Normal,
        l2_data::stats::AbnormalType::PdUp
    ));
}

#[test]
fn leaving_removes_every_effect_and_its_timers() {
    let (mut world, _log) = world();
    buff(&mut world, 1040, 3);
    buff(&mut world, 9030, 1);
    let timers_with = world.timers().len();
    world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    assert!(world.effect_list(ME).is_none());
    // The finish task, the tick task and the icon task are gone with the player.
    assert!(world.timers().len() < timers_with);
    for _ in 0..200 {
        world.tick();
    }
}

#[test]
fn the_registry_has_every_java_handler_and_buff_is_ported() {
    let (world, _log) = world();
    let registry = world.effect_registry();
    assert_eq!(registry.len(), 165);
    assert!(registry.is_ported("Buff"));
    assert!(registry.contains("Stun") && !registry.is_ported("Stun"));
}

// ---- the real datapack ----------------------------------------------------------------------------

fn game_root() -> Option<PathBuf> {
    let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
    if root.join("data/stats/skills").is_dir() {
        Some(root)
    } else {
        eprintln!("skipped: {} has no datapack", root.display());
        None
    }
}

#[test]
fn the_datapack_shield_and_might_buff_like_java() {
    let Some(root) = game_root() else {
        return;
    };
    let table = SkillTemplates::load(&root, &l2_data::SkillLoadOptions::default());
    let (mut world, log) = world_with(EffectSettings::default(), table);
    let before = p_def(&world);
    buff(&mut world, 1040, 3);
    assert_eq!(p_def(&world), before * 1.15);
    // 1068 Might (PA_UP) lands next to it.
    buff(&mut world, 1068, 3);
    log.take();
    advance(&mut world, 300);
    let icons: Vec<(i32, i32)> = last_icons(&log.take()).into_iter().map(|i| (i.0, i.1)).collect();
    assert_eq!(icons, [(1040, 3), (1068, 3)]);
}
