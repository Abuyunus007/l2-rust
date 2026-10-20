#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The buffs kept over a logout (M5-SAVE-01): Java `Player.storeEffect` decides which buffs are
//! saved and with how much time left, `Player.restoreEffects` applies them when the character
//! enters. The rows are the `character_skills_save` columns (`SavedEffect`); the database side is
//! in `game-server/tests/persist_db.rs`. Expected behaviour comes from the Java code named in each
//! test.

use std::sync::{Arc, Mutex};

use l2_data::{ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, PlayerTemplates, SkillTemplates};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a world that records what it persists ----------------------------------------------------

#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Outbox for Persisted {
    fn persist(&mut self, command: PersistCommand) {
        self.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
    /// The last full store of the character (what `PlayerLeft` follows).
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

#[derive(Default)]
struct Sink;

impl PacketSink for Sink {
    fn send(&self, _bytes: Vec<u8>) {}
    fn left_world(&self, _reason: LeaveReason) {}
}

// ---- skills -----------------------------------------------------------------------------------

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

/// Each skill is made for one rule of the save: a 60 s buff, a second 60 s buff, a dance, a toggle
/// and a heal (`LIFE_FORCE_OTHERS`, which Java never saves).
const SKILLS: &str = r##"<list>
<skill id="9001" levels="1" name="Might"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9002" levels="1" name="Empower"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>MA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="mAtk" val="1.1"/></effect></effects></skill>
<skill id="9020" levels="1" name="Dance A"><operateType>A2</operateType><targetType>SELF</targetType><isMagic>3</isMagic>
<abnormalTime>60</abnormalTime><abnormalType>CRITICAL_DMG_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="critDmg" val="1.1"/></effect></effects></skill>
<skill id="9030" levels="1" name="Toggle stays"><operateType>T</operateType><targetType>SELF</targetType>
<effects><effect name="Buff" ticks="5"><mul stat="mDef" val="1.5"/></effect></effects></skill>
<skill id="9090" levels="1" name="Life Force"><operateType>A2</operateType><targetType>SELF</targetType>
<abnormalTime>60</abnormalTime><abnormalType>LIFE_FORCE_OTHERS</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pDef" val="1.1"/></effect></effects></skill>
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

fn stat_data() -> StatData {
    let skills = SkillTemplates::from_xml(
        SKILLS,
        &l2_data::skill::EnchantSkillGroups::default(),
        &l2_data::SkillLoadOptions::default(),
    );
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

// ---- a character -----------------------------------------------------------------------------

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

/// A world with the character in it (entered with `data`), a second after the enter.
fn enter(settings: EffectSettings, data: PlayerData) -> (World, Persisted) {
    let persisted = Persisted::default();
    let mut world = World::new(WorldRng::from_seed(1), Box::new(persisted.clone()));
    world.set_settings(WorldSettings {
        effects: settings,
        ..WorldSettings::default()
    });
    world.set_static_data(Box::new(Data(stat_data())));
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(data),
        sink: Box::new(Sink),
        ip: None,
    });
    advance(&mut world, 1000);
    (world, persisted)
}

/// Runs the world for `ms` milliseconds (50 ms ticks).
fn advance(world: &mut World, ms: u64) {
    for _ in 0..(ms / 50) {
        world.tick();
    }
}

fn buff(world: &mut World, id: i32) {
    world.apply_effects(ME, ME, SkillKey::new(id, 1));
}

/// The character leaves (the connection is gone): its final store.
fn leave(world: &mut World, persisted: &Persisted) -> PlayerData {
    world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    persisted.last_store()
}

/// The ids of the buffs (`Buffs` queue) in order.
fn buff_ids(world: &World) -> Vec<i32> {
    world
        .effect_list(ME)
        .map(|list| list.buffs().iter().map(|b| b.skill().id).collect())
        .unwrap_or_default()
}

fn row(skill_id: i32, remaining_time: i32) -> SavedEffect {
    SavedEffect {
        skill_id,
        skill_level: 1,
        remaining_time,
        reuse_delay: 0,
        systime: 0,
        restore_type: 0,
    }
}

// ---- storeEffect -----------------------------------------------------------------------------

/// Java `storeEffect`: a buff is saved with the seconds it has left (`BuffInfo.getTime()`), and the
/// rows are in the order of `getEffects()`.
#[test]
fn a_buff_is_saved_with_the_time_it_has_left() {
    let (mut world, persisted) = enter(EffectSettings::default(), player());
    buff(&mut world, 9001);
    advance(&mut world, 10_000);
    let stored = leave(&mut world, &persisted);
    assert_eq!(stored.profile.effects_to_store, Some(vec![row(9001, 50)]));
}

/// Java `storeEffect`: dances are skipped unless `AltStoreDances`, toggles unless
/// `AltStoreToggles`. Heals (`LIFE_FORCE_OTHERS`) are never saved.
#[test]
fn dances_and_toggles_follow_their_settings_and_heals_are_not_saved() {
    let settings = |dances, toggles| EffectSettings {
        alt_store_dances: dances,
        alt_store_toggles: toggles,
        ..EffectSettings::default()
    };

    let (mut world, persisted) = enter(settings(false, false), player());
    buff(&mut world, 9001);
    buff(&mut world, 9020);
    buff(&mut world, 9030);
    buff(&mut world, 9090);
    assert_eq!(buff_ids(&world), [9001, 9090]);
    let stored = leave(&mut world, &persisted);
    assert_eq!(stored.profile.effects_to_store, Some(vec![row(9001, 60)]));

    let (mut world, persisted) = enter(settings(true, true), player());
    buff(&mut world, 9001);
    buff(&mut world, 9020);
    buff(&mut world, 9030);
    buff(&mut world, 9090);
    let stored = leave(&mut world, &persisted);
    assert_eq!(
        stored.profile.effects_to_store,
        Some(vec![row(9001, 60), row(9020, 60), row(9030, -1)])
    );
}

/// Java `storeEffect` returns before it touches the table when `StoreSkillCooltime` is off.
#[test]
fn with_store_skill_cooltime_off_the_saved_buffs_are_left_alone() {
    let settings = EffectSettings {
        store_skill_cooltime: false,
        ..EffectSettings::default()
    };
    let (mut world, persisted) = enter(settings, player());
    buff(&mut world, 9001);
    let stored = leave(&mut world, &persisted);
    assert_eq!(stored.profile.effects_to_store, None);
}

// ---- restoreEffects --------------------------------------------------------------------------

/// The buffs of the last logout come back with the time they had left, in the saved order, and
/// they run out when that time is over.
#[test]
fn a_saved_buff_comes_back_with_its_remaining_time_and_wears_off_then() {
    let mut data = player();
    data.profile.effects_to_restore = vec![row(9001, 30), row(9002, 45)];
    let (mut world, _) = enter(EffectSettings::default(), data);
    assert_eq!(buff_ids(&world), [9001, 9002]);
    let abnormal: Vec<i32> = world
        .effect_list(ME)
        .unwrap()
        .buffs()
        .iter()
        .map(|b| b.abnormal_time())
        .collect();
    assert_eq!(abnormal, [30, 45]);

    // `enter` ran one second already: 28 s from the enter, both still run.
    advance(&mut world, 27_000);
    assert_eq!(buff_ids(&world), [9001, 9002]);
    // 31 s from the enter: the 30 s buff is over, the 45 s one runs on.
    advance(&mut world, 3_000);
    assert_eq!(buff_ids(&world), [9002]);
}

/// What is stored comes back the same way: a logout, then an enter with the stored rows, then the
/// next logout stores the same rows again with the time still left.
#[test]
fn a_buff_survives_a_logout_and_an_enter_with_the_time_it_had_left() {
    let (mut world, persisted) = enter(EffectSettings::default(), player());
    buff(&mut world, 9001);
    buff(&mut world, 9002);
    advance(&mut world, 20_000);
    let stored = leave(&mut world, &persisted);
    assert_eq!(
        stored.profile.effects_to_store,
        Some(vec![row(9001, 40), row(9002, 40)])
    );

    // The database hands the rows back at character select.
    let mut data = player();
    data.profile.effects_to_restore = stored.profile.effects_to_store.clone().unwrap();
    let (mut world, persisted) = enter(EffectSettings::default(), data);
    assert_eq!(buff_ids(&world), [9001, 9002]);
    let stored = leave(&mut world, &persisted);
    assert_eq!(
        stored.profile.effects_to_store,
        Some(vec![row(9001, 39), row(9002, 39)])
    );
}

/// Java `restoreEffects`: a skill the datapack does not have is dropped, and a reuse-only row
/// (restore type 1) restores no buff.
#[test]
fn unknown_skills_and_reuse_only_rows_restore_no_buff() {
    let mut reuse_only = row(9001, -1);
    reuse_only.restore_type = 1;
    reuse_only.reuse_delay = 60_000;
    reuse_only.systime = 1;
    let mut data = player();
    data.profile.effects_to_restore = vec![reuse_only, row(424_242, 30), row(9002, 30)];
    let (world, _) = enter(EffectSettings::default(), data);
    assert_eq!(buff_ids(&world), [9002]);
}

/// Java `skill.applyEffects(this, this, false, 0)`: a row with no time left restores the skill's
/// own abnormal time (`abnormalTime` of the skill, here 60 s), since a zero custom time is ignored.
// NOTE(java-quirk): a buff that had no time left at logout comes back with its full time.
#[test]
fn a_row_without_time_left_restores_the_skill_time() {
    let mut data = player();
    data.profile.effects_to_restore = vec![row(9001, 0)];
    let (world, _) = enter(EffectSettings::default(), data);
    let abnormal: Vec<i32> = world
        .effect_list(ME)
        .unwrap()
        .buffs()
        .iter()
        .map(|b| b.abnormal_time())
        .collect();
    assert_eq!(abnormal, [60]);
}

/// Java `restoreEffects` runs only with `StoreSkillCooltime`: the rows are not applied otherwise.
#[test]
fn with_store_skill_cooltime_off_nothing_is_restored() {
    let settings = EffectSettings {
        store_skill_cooltime: false,
        ..EffectSettings::default()
    };
    let mut data = player();
    data.profile.effects_to_restore = vec![row(9001, 30)];
    let (world, _) = enter(settings, data);
    assert!(buff_ids(&world).is_empty());
}
