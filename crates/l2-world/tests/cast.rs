#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Casting (M5-CAST-01): `RequestMagicSkillUse` -> `useMagic` -> the CAST intention -> `doCast`,
//! the packets and their order, the `MagicUseTask` phases on the timer wheel, MP and item costs,
//! reuse and `SkillCoolTime`, the queue, interruption, moving into range, toggles, and an NPC
//! casting through the same entry point. Expected behaviour comes from the Java code named in each
//! test (`Creature.beginCast` and friends, `Player.useMagic`, `PlayerAI.thinkCast`).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    MagicSkillCanceled, MagicSkillLaunched, MagicSkillUse, SetupGauge, SkillCoolTime, StatusUpdate,
    SystemMessage, SystemMessageParam,
};
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

/// The persist commands of the world.
#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Outbox for Persisted {
    fn persist(&mut self, command: PersistCommand) {
        self.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
    /// The last full store of a character.
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

/// 1177 is the datapack's Wind Strike level 1 and 1040 Shield level 1 (comments and the other
/// levels left out). The 9xxx skills are made for one rule each.
const SKILLS: &str = r##"<list>
<skill id="1177" levels="1" name="Wind Strike">
	<icon>icon.skill1177</icon>
	<operateType>A1</operateType>
	<targetType>ONE</targetType>
	<castRange>600</castRange>
	<effectPoint>-92</effectPoint>
	<effectRange>1100</effectRange>
	<element>2</element>
	<elementPower>20</elementPower>
	<hitTime>4000</hitTime>
	<isMagic>1</isMagic>
	<magicLevel>1</magicLevel>
	<mpConsume>7</mpConsume>
	<mpInitialConsume>2</mpInitialConsume>
	<power>12</power>
	<reuseDelay>1200</reuseDelay>
	<effects>
		<effect name="MagicalDamage" />
	</effects>
</skill>
<skill id="1040" levels="1" name="Shield">
	<icon>icon.skill1040</icon>
	<operateType>A2</operateType>
	<targetType>ONE</targetType>
	<abnormalLevel>1</abnormalLevel>
	<abnormalTime>1200</abnormalTime>
	<abnormalType>PD_UP</abnormalType>
	<castRange>400</castRange>
	<effectPoint>121</effectPoint>
	<effectRange>900</effectRange>
	<hitTime>4000</hitTime>
	<isMagic>1</isMagic>
	<magicLevel>7</magicLevel>
	<mpConsume>8</mpConsume>
	<mpInitialConsume>2</mpInitialConsume>
	<reuseDelay>2000</reuseDelay>
	<effects>
		<effect name="Buff">
			<mul stat="pDef">1.08</mul>
		</effect>
	</effects>
</skill>
<skill id="9001" levels="1" name="Might"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>1500</hitTime><isMagic>1</isMagic><mpConsume>5</mpConsume><reuseDelay>60000</reuseDelay>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
<skill id="9002" levels="1" name="Empower"><operateType>A2</operateType><targetType>SELF</targetType>
<hitTime>1000</hitTime><isMagic>1</isMagic><mpConsume>1</mpConsume>
<abnormalTime>60</abnormalTime><abnormalType>MA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="mAtk" val="1.1"/></effect></effects></skill>
<skill id="9003" levels="1" name="Heal"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectRange>1100</effectRange><effectPoint>100</effectPoint>
<hitTime>1000</hitTime><isMagic>1</isMagic><mpConsume>3</mpConsume>
<effects><effect name="Heal"><param power="10"/></effect></effects></skill>
<skill id="9004" levels="1" name="Toggle"><operateType>T</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><mul stat="mDef" val="1.5"/></effect></effects></skill>
<skill id="9005" levels="1" name="Spirit Ore Skill"><operateType>A1</operateType><targetType>SELF</targetType>
<hitTime>500</hitTime><isMagic>2</isMagic><itemConsumeId>3031</itemConsumeId><itemConsumeCount>1</itemConsumeCount></skill>
<skill id="9006" levels="1" name="Power Strike"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>40</castRange><effectRange>400</effectRange><effectPoint>-100</effectPoint>
<hitTime>1080</hitTime><isMagic>0</isMagic><mpConsume>3</mpConsume>
<effects><effect name="PhysicalDamage"><param power="25"/></effect></effects></skill>
</list>"##;

/// The spirit ore of skill 9005.
const SPIRIT_ORE: i32 = 3031;

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        (item_id == SPIRIT_ORE).then(|| ItemStatic::unknown(item_id))
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        item_id == SPIRIT_ORE
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
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
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

/// 9001 a passive monster.
const NPCS: &str = r#"<?xml version="1.0"?><list><npc id="9001" type="Monster" level="1" name="Gremlin">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc></list>"#;

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const P2: ObjectId = ObjectId(0x2000_0002);

const WIND_STRIKE: i32 = 1177;
const SHIELD: i32 = 1040;
const MIGHT: i32 = 9001;
const EMPOWER: i32 = 9002;
const HEAL: i32 = 9003;
const TOGGLE: i32 = 9004;
const ORE_SKILL: i32 = 9005;
const POWER_STRIKE: i32 = 9006;

fn player_data(id: ObjectId, (x, y, z): (i32, i32, i32), items: Vec<ItemData>) -> PlayerData {
    let skills = [
        WIND_STRIKE,
        SHIELD,
        MIGHT,
        EMPOWER,
        HEAL,
        TOGGLE,
        ORE_SKILL,
        POWER_STRIKE,
    ]
    .into_iter()
    .map(|skill_id| SkillData {
        skill_id,
        skill_level: 1,
    })
    .collect();
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
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items,
        skills,
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
        Self::with_items(vec![])
    }

    fn with_items(items: Vec<ItemData>) -> Self {
        Self::with_me(player_data(ME, (X, Y, Z), items))
    }

    fn with_me(me_data: PlayerData) -> Self {
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(persisted.clone()));
        world.set_static_data(Box::new(Data(stat_data())));
        // No MP regeneration, so the costs can be read off the MP exactly.
        world.set_regen_settings(RegenSettings {
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(me_data),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let p2 = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(2),
            player: Box::new(player_data(P2, (X + 100, Y, Z), vec![])),
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

    fn monster(&mut self, dx: i32) -> ObjectId {
        self.world.spawn_npc_at(9001, X + dx, Y, Z, 0).unwrap()
    }

    fn select(&mut self, target: ObjectId) {
        self.world.set_target(ME, Some(target));
        self.me.take();
    }

    fn use_skill(&mut self, magic_id: i32, ctrl: bool, shift: bool) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::MagicSkillUse {
                magic_id,
                ctrl_pressed: ctrl,
                shift_pressed: shift,
            },
        });
    }

    fn mp(&self) -> f64 {
        self.world.player(ME).unwrap().core.cur_mp
    }

    fn template(&self, id: i32) -> l2_data::SkillTemplate {
        let data = stat_data();
        data.skills.get(id, 1).unwrap().clone()
    }

    /// The cast time `beginCast` computes: Java's `Formulas.calcAtkSpd` with the 550 ms floor.
    fn cast_time(&self, id: i32) -> i32 {
        self.world.skill_cast_time(ME, &self.template(id))
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

fn messages(packets: &[Vec<u8>]) -> Vec<SystemMessage> {
    of(packets, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p))
        .collect()
}

fn message_ids(packets: &[Vec<u8>]) -> Vec<SystemMessageId> {
    messages(packets).into_iter().map(|m| m.id).collect()
}

/// Runs the world tick by tick until the packet `name` reaches `log`; the milliseconds it took.
fn until(scene: &mut Scene, name: &str, max_ms: u64) -> (u64, Vec<Vec<u8>>) {
    let mut all = Vec::new();
    for step in 1..=(max_ms / 50) {
        scene.world.tick();
        let got = scene.me.take();
        let found = got.iter().any(|p| name_of(p) == name);
        all.extend(got);
        if found {
            return (step * 50, all);
        }
    }
    panic!("{name} never came within {max_ms} ms; got {:?}", names(&all));
}

// ---- a nuke ------------------------------------------------------------------------------------

/// `beginCast`: the initial MP, `ExRotation`, `MagicSkillUse`, `MagicSkillLaunched`, "You use
/// $s1." and the gauge, in the order of the Java capture (`java-gs-session.jsonl`, Wind Strike).
/// The hit comes at the cast time (launch at `skillTime - 400`, hit 400 ms later): MP, hate and
/// the attack-by list; the finalizer the tick after.
#[test]
fn wind_strike_on_a_monster_follows_java_packets_and_phases() {
    let mut s = Scene::new();
    let mob = s.monster(200);
    s.select(mob);
    let mp_before = s.mp();
    s.use_skill(WIND_STRIKE, false, false);
    let sent = s.me.take();
    assert_eq!(
        names(&sent),
        [
            "STATUS_UPDATE",
            "EX_ROTATION",
            "MAGIC_SKILL_USE",
            "MAGIC_SKILL_LAUNCHED",
            "SYSTEM_MESSAGE",
            "SETUP_GAUGE"
        ]
    );
    let su: StatusUpdate = decode(&sent[0]);
    assert_eq!(su.attributes, [(0x0b, (mp_before - 2.0) as i32)]);
    let time = s.cast_time(WIND_STRIKE);
    let msu: MagicSkillUse = decode(&sent[2]);
    assert_eq!(
        (msu.creature_id, msu.target_id, msu.skill_id, msu.skill_level),
        (ME.0, mob.0, WIND_STRIKE, 1)
    );
    assert_eq!((msu.hit_time, msu.reuse_delay), (time, 1200));
    assert_eq!(msu.ground_location, None);
    let msl: MagicSkillLaunched = decode(&sent[3]);
    assert_eq!(msl.target_ids, [mob.0]);
    let use_s1 = &messages(&sent)[0];
    assert_eq!(use_s1.id, SystemMessageId::UseS1);
    assert_eq!(
        use_s1.params,
        [SystemMessageParam::SkillName {
            id: WIND_STRIKE,
            level: 1
        }]
    );
    let gauge: SetupGauge = decode(&sent[5]);
    assert_eq!(gauge, SetupGauge::new(ME.0, SetupGauge::BLUE, time));
    assert!(s.world.is_casting_now(ME));
    assert!(s.world.can_abort_cast(ME));
    // The others see the cast too.
    assert_eq!(of(&s.p2.take(), "MAGIC_SKILL_USE").len(), 1);

    // Nothing is paid before the hit.
    let hit_tick = u64::try_from(time).unwrap().div_ceil(50);
    for _ in 0..hit_tick - 1 {
        s.world.tick();
    }
    assert_eq!(s.mp(), mp_before - 2.0);
    assert!(s.world.aggro_list(mob).is_empty());
    s.world.tick();
    // The hit: 7 MP, the monster hates the caster by -effectPoint and knows its attacker.
    assert_eq!(s.mp(), mp_before - 9.0);
    let hate = s
        .world
        .aggro_list(mob)
        .iter()
        .find(|a| a.attacker == ME)
        .map(|a| a.hate);
    assert!(hate.unwrap() >= 92);
    assert!(s.world.attack_by_list(mob).contains(&ME));
    assert!(s.world.is_casting_now(ME));
    // The finalizer: the cast is over, an offensive skill puts the caster in combat.
    s.world.tick();
    assert!(!s.world.is_casting_now(ME));
    assert!(s.world.current_skill(ME).is_none());
    assert!(s.world.is_auto_attacking(ME));
}

/// `onMagicHitTimer`: MP that is gone by the hit aborts the cast with "Not enough MP".
#[test]
fn missing_mp_at_the_hit_aborts_the_cast() {
    let mut s = Scene::new();
    let mob = s.monster(200);
    s.select(mob);
    s.use_skill(WIND_STRIKE, false, false);
    s.me.take();
    s.world.set_current_mp(ME, 3.0, false);
    let (_, sent) = until(&mut s, "MAGIC_SKILL_CANCELED", 6000);
    assert!(message_ids(&sent).contains(&SystemMessageId::NotEnoughMp));
    assert!(!s.world.is_casting_now(ME));
    assert_eq!(s.mp(), 3.0);
}

// ---- buffs and heals -----------------------------------------------------------------------------

/// A self buff with the reference `Buff` handler: `RequestMagicSkillUse` first stops the player
/// (`setIntentionMoveTo(location)`), the buff lands at the hit.
#[test]
fn might_buffs_the_caster_with_the_buff_handler() {
    let mut s = Scene::new();
    s.use_skill(MIGHT, false, false);
    let sent = s.me.take();
    let msu: MagicSkillUse = decode(of(&sent, "MAGIC_SKILL_USE")[0]);
    assert_eq!(msu.target_id, ME.0);
    // No rotation towards oneself.
    assert!(of(&sent, "EX_ROTATION").is_empty());
    assert!(!s.world.is_affected_by_skill(ME, MIGHT));
    s.advance(u64::try_from(s.cast_time(MIGHT)).unwrap() + 100);
    assert!(s.world.is_affected_by_skill(ME, MIGHT));
    assert!(!s.world.is_casting_now(ME));
}

/// Shield (target `ONE`) on another player: a continuous good skill casts at the first target of
/// the list, and the buff lands on that player.
#[test]
fn shield_on_another_player_lands_on_that_player() {
    let mut s = Scene::new();
    s.select(P2);
    s.use_skill(SHIELD, false, false);
    let sent = s.me.take();
    let msu: MagicSkillUse = decode(of(&sent, "MAGIC_SKILL_USE")[0]);
    assert_eq!(msu.target_id, P2.0);
    s.advance(u64::try_from(s.cast_time(SHIELD)).unwrap() + 100);
    assert!(s.world.is_affected_by_skill(P2, SHIELD));
    assert!(!s.world.is_affected_by_skill(ME, SHIELD));
    // A good skill makes no combat.
    assert!(!s.world.is_auto_attacking(ME));
}

/// A heal (effect type `HEAL`, an unported stand-in here) on another player goes through every
/// phase like any skill.
#[test]
fn heal_on_another_player_runs_every_phase() {
    let mut s = Scene::new();
    s.select(P2);
    let mp = s.mp();
    s.use_skill(HEAL, false, false);
    let msu: MagicSkillUse = decode(of(&s.me.take(), "MAGIC_SKILL_USE")[0]);
    assert_eq!(msu.target_id, P2.0);
    s.advance(u64::try_from(s.cast_time(HEAL)).unwrap() + 100);
    assert_eq!(s.mp(), mp - 3.0);
    assert!(!s.world.is_casting_now(ME));
}

/// `Player.checkUseMagicConditions`: a good skill on a monster needs ctrl.
#[test]
fn a_good_skill_on_a_monster_needs_force_use() {
    let mut s = Scene::new();
    let mob = s.monster(200);
    s.select(mob);
    s.use_skill(HEAL, false, false);
    assert_eq!(names(&s.me.take()), ["ACTION_FAIL"]);
    assert!(!s.world.is_casting_now(ME));
    s.use_skill(HEAL, true, false);
    assert_eq!(of(&s.me.take(), "MAGIC_SKILL_USE").len(), 1);
}

// ---- reuse -------------------------------------------------------------------------------------

/// A reuse above one second is a time stamp: the second use tells the seconds left
/// ("There are $s2 second(s) remaining in $s1's re-use time."), `SkillCoolTime` lists it, and it
/// ends after the delay.
#[test]
fn reuse_refuses_with_the_time_left_and_shows_in_skill_cool_time() {
    let mut s = Scene::new();
    s.use_skill(MIGHT, false, false);
    s.advance(2000);
    s.me.take();
    // 60 s reuse: 1 minute minus the time since the cast began.
    s.use_skill(MIGHT, false, false);
    let sent = s.me.take();
    let sm = &messages(&sent)[0];
    assert_eq!(sm.id, SystemMessageId::ThereAreS2SecondSRemainingInS1SReUseTime);
    assert_eq!(
        sm.params[0],
        SystemMessageParam::SkillName { id: MIGHT, level: 1 }
    );
    assert_eq!(sm.params[1], SystemMessageParam::IntNumber(58));

    s.world.handle(WorldCommand::Input {
        session: SessionId(1),
        input: ClientInput::SkillCoolTime,
    });
    let cool: SkillCoolTime = decode(of(&s.me.take(), "SKILL_COOL_TIME")[0]);
    assert_eq!(cool.entries.len(), 1);
    let e = cool.entries[0];
    assert_eq!(
        (e.skill_id, e.level, e.reuse_seconds, e.remaining_seconds),
        (MIGHT, 1, 60, 58)
    );

    s.advance(58_000);
    s.use_skill(MIGHT, false, false);
    assert_eq!(of(&s.me.take(), "MAGIC_SKILL_USE").len(), 1);
}

/// `Player.storeEffect` / `restoreEffects` (M5-SAVE-01's rows): a buff that is also in reuse is
/// a `restore_type` 0 row with its reuse and wall-clock stamp, a reuse alone a type 1 row with
/// remaining time -1; at the next enter both reuse timers come back.
#[test]
fn reuse_timers_survive_a_logout() {
    let mut s = Scene::new();
    s.use_skill(MIGHT, false, false);
    s.advance(u64::try_from(s.cast_time(MIGHT)).unwrap() + 100);
    s.select(P2);
    // Logging out during the cast: Shield's 2 s reuse runs, its buff never landed.
    s.use_skill(SHIELD, false, false);
    s.advance(500);
    assert!(s.world.is_affected_by_skill(ME, MIGHT));
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    let stored = s.persisted.last_store();
    let rows = stored.profile.effects_to_store.clone().unwrap();
    let might = rows.iter().find(|r| r.skill_id == MIGHT).unwrap();
    assert_eq!((might.restore_type, might.reuse_delay), (0, 60_000));
    assert!(might.systime > 0);
    let shield = rows.iter().find(|r| r.skill_id == SHIELD).unwrap();
    assert_eq!(
        (shield.restore_type, shield.remaining_time, shield.reuse_delay),
        (1, -1, 2000)
    );

    let mut data = player_data(ME, (X, Y, Z), vec![]);
    data.profile.effects_to_restore = rows;
    let mut s = Scene::with_me(data);
    let cool = s.world.skill_cool_time(ME);
    let ids: Vec<i32> = cool.entries.iter().map(|e| e.skill_id).collect();
    assert!(ids.contains(&MIGHT));
    s.use_skill(MIGHT, false, false);
    assert_eq!(
        message_ids(&s.me.take()),
        [SystemMessageId::ThereAreS2SecondSRemainingInS1SReUseTime]
    );
}

/// The stamps M5-SAVE-01 stores; a disabled skill (`disableSkill`, a reuse of one second or less)
/// is refused with "$s1 is not available at this time: being prepared for reuse." until its time
/// ends (0: until enabled).
#[test]
fn reuse_stamps_are_kept_for_the_store() {
    let mut s = Scene::new();
    s.use_skill(MIGHT, false, false);
    s.me.take();
    let stamps = s.world.skill_reuse_time_stamps(ME);
    assert_eq!(stamps.len(), 1, "{stamps:?}");
    assert_eq!(
        (stamps[0].skill_id, stamps[0].reuse_ms, stamps[0].remaining_ms),
        (MIGHT, 60_000, 60_000)
    );
    let template = s.template(EMPOWER);
    s.world.disable_skill(ME, &template, 500);
    assert!(s.world.is_skill_disabled(ME, &template));
    s.advance(2000);
    s.use_skill(EMPOWER, false, false);
    assert_eq!(of(&s.me.take(), "MAGIC_SKILL_USE").len(), 1);
    s.advance(u64::try_from(s.cast_time(EMPOWER)).unwrap() + 100);
    assert!(!s.world.is_casting_now(ME));
    s.me.take();
    let template = s.template(EMPOWER);
    s.world.disable_skill(ME, &template, 0);
    s.use_skill(EMPOWER, false, false);
    let sent = s.me.take();
    assert_eq!(
        message_ids(&sent),
        [SystemMessageId::S1IsNotAvailableAtThisTimeBeingPreparedForReuse]
    );
}

// ---- the queue -----------------------------------------------------------------------------------

/// `useMagic` during a cast: the same skill is refused, another is queued (`ActionFailed`) and
/// cast after the finalizer (`QueuedMagicUseTask`). Both are self buffs, so
/// `RequestMagicSkillUse` first asks to stop where the player stands, which a casting player
/// refuses with its own `ActionFailed` (and keeps as the next intention).
#[test]
fn a_skill_used_during_a_cast_is_queued_and_cast_after_it() {
    let mut s = Scene::new();
    s.use_skill(EMPOWER, false, false);
    s.me.take();
    s.use_skill(EMPOWER, false, false);
    assert_eq!(names(&s.me.take()), ["ACTION_FAIL", "ACTION_FAIL"]);
    assert!(s.world.queued_skill(ME).is_none());
    s.use_skill(MIGHT, false, false);
    assert_eq!(names(&s.me.take()), ["ACTION_FAIL", "ACTION_FAIL"]);
    assert_eq!(s.world.queued_skill(ME).map(|q| q.skill.id), Some(MIGHT));
    let (_, sent) = until(&mut s, "MAGIC_SKILL_USE", 3000);
    let msu: MagicSkillUse = decode(of(&sent, "MAGIC_SKILL_USE")[0]);
    assert_eq!(msu.skill_id, MIGHT);
    assert!(s.world.is_affected_by_skill(ME, EMPOWER));
    assert!(s.world.queued_skill(ME).is_none());
}

// ---- interruption --------------------------------------------------------------------------------

/// `breakCast` (damage, `Formulas.calcAtkBreak`): a spell before its interrupt time is aborted,
/// `MagicSkillCanceled` to everybody, `ActionFailed` and "Your casting has been interrupted.";
/// nothing lands and no MP is paid at the hit.
#[test]
fn break_cast_interrupts_a_spell() {
    let mut s = Scene::new();
    s.use_skill(MIGHT, false, false);
    s.me.take();
    s.p2.take();
    let mp = s.mp();
    s.advance(500);
    s.world.break_cast(ME);
    let sent = s.me.take();
    assert!(names(&sent).ends_with(&[
        "MAGIC_SKILL_CANCELED".to_owned(),
        "ACTION_FAIL".to_owned(),
        "SYSTEM_MESSAGE".to_owned()
    ]));
    assert_eq!(
        message_ids(&sent),
        [SystemMessageId::YourCastingHasBeenInterrupted]
    );
    let canceled: MagicSkillCanceled = decode(of(&s.p2.take(), "MAGIC_SKILL_CANCELED")[0]);
    assert_eq!(canceled.object_id, ME.0);
    assert!(!s.world.is_casting_now(ME));
    s.advance(3000);
    assert!(!s.world.is_affected_by_skill(ME, MIGHT));
    assert_eq!(s.mp(), mp);
}

/// Damage only breaks magic and static skills, and only before the interrupt time
/// (`skillTime / 100 - 2` game ticks after the start).
#[test]
fn break_cast_spares_physical_skills_and_late_casts() {
    let mut s = Scene::new();
    let mob = s.monster(30);
    s.select(mob);
    s.use_skill(POWER_STRIKE, false, false);
    assert_eq!(of(&s.me.take(), "MAGIC_SKILL_USE").len(), 1);
    s.world.break_cast(ME);
    assert!(s.world.is_casting_now(ME));
    assert!(s.me.take().is_empty());

    let mut s = Scene::new();
    s.use_skill(MIGHT, false, false);
    let time = u64::try_from(s.cast_time(MIGHT)).unwrap();
    // The last 200 ms of the cast cannot be interrupted any more.
    s.advance(time - 100);
    assert!(!s.world.can_abort_cast(ME));
    s.world.break_cast(ME);
    assert!(s.world.is_casting_now(ME));
}

/// Dying aborts the cast (`CreatureStatus.reduceHp` -> `abortCast`).
#[test]
fn death_aborts_the_cast() {
    let mut s = Scene::new();
    s.use_skill(MIGHT, false, false);
    s.me.take();
    s.world.reduce_current_hp(ME, 1000.0, None);
    assert!(!s.world.is_casting_now(ME));
    assert_eq!(of(&s.me.take(), "MAGIC_SKILL_CANCELED").len(), 1);
}

// ---- range ---------------------------------------------------------------------------------------

/// Out of cast range: `thinkCast` walks up to the target (the casting flag goes down while
/// walking) and casts on arrival.
#[test]
fn out_of_range_the_player_walks_up_then_casts() {
    let mut s = Scene::new();
    let mob = s.monster(1500);
    s.select(mob);
    s.use_skill(WIND_STRIKE, false, false);
    let sent = s.me.take();
    assert!(of(&sent, "MAGIC_SKILL_USE").is_empty());
    assert!(!s.world.is_casting_now(ME));
    assert_eq!(s.world.intention_of(ME), Intention::Cast);
    // NOTE(java-quirk): `useMagic` raised the casting flag, so the follow task's first
    // `moveToPawn` refuses (`ActionFailed`); the walk starts with its next run, 500 ms later.
    assert_eq!(names(&sent), ["ACTION_FAIL"]);
    assert!(!s.world.is_moving(ME));
    s.advance(500);
    assert!(s.world.is_moving(ME));
    let (_, sent) = until(&mut s, "MAGIC_SKILL_USE", 20_000);
    let msu: MagicSkillUse = decode(of(&sent, "MAGIC_SKILL_USE")[0]);
    assert_eq!(msu.target_id, mob.0);
    assert!(s.world.is_casting_now(ME));
    let (x, _, _) = s.world.position_of(ME).unwrap();
    // Within cast range plus both collision radii.
    assert!(X + 1500 - x <= 600 + 9 + 10);
}

/// Shift (`dontMove`) with an offensive skill out of range: "Your target is out of range.".
#[test]
fn shift_out_of_range_is_refused() {
    let mut s = Scene::new();
    let mob = s.monster(1500);
    s.select(mob);
    s.use_skill(WIND_STRIKE, false, true);
    let sent = s.me.take();
    assert_eq!(message_ids(&sent), [SystemMessageId::YourTargetIsOutOfRange]);
    assert_eq!(names(&sent).last().unwrap(), "ACTION_FAIL");
    assert!(!s.world.is_moving(ME));
}

// ---- costs ---------------------------------------------------------------------------------------

/// `checkDoCastConditions`: MP for both costs ("Not enough MP."), and the consumed item ("There
/// are not enough necessary items to use the skill."); `beginCast` destroys the item.
#[test]
fn mp_and_item_costs() {
    let mut s = Scene::new();
    s.world.set_current_mp(ME, 4.0, false);
    s.select(P2);
    s.use_skill(SHIELD, false, false);
    let sent = s.me.take();
    assert!(message_ids(&sent).contains(&SystemMessageId::NotEnoughMp));
    assert!(!s.world.is_casting_now(ME));

    s.use_skill(ORE_SKILL, false, false);
    let sent = s.me.take();
    assert_eq!(
        message_ids(&sent),
        [SystemMessageId::ThereAreNotEnoughNecessaryItemsToUseTheSkill]
    );

    let ore = ItemData {
        object_id: ObjectId(0x6000_0100),
        item_id: SPIRIT_ORE,
        count: 2,
        enchant_level: 0,
        loc: "INVENTORY".into(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
    };
    let mut s = Scene::with_items(vec![ore]);
    s.use_skill(ORE_SKILL, false, false);
    assert_eq!(of(&s.me.take(), "MAGIC_SKILL_USE").len(), 1);
    let left: i64 = s
        .world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == SPIRIT_ORE)
        .map(|i| i.count)
        .sum();
    assert_eq!(left, 1);
}

// ---- toggles -------------------------------------------------------------------------------------

/// A toggle has no `MagicSkillUse`; using it again switches it off (`checkUseMagicConditions`).
#[test]
fn a_toggle_switches_on_and_off() {
    let mut s = Scene::new();
    s.use_skill(TOGGLE, false, false);
    s.advance(100);
    let sent = s.me.take();
    assert!(of(&sent, "MAGIC_SKILL_USE").is_empty());
    assert!(s.world.is_affected_by_skill(ME, TOGGLE));
    s.use_skill(TOGGLE, false, false);
    assert!(!s.world.is_affected_by_skill(ME, TOGGLE));
    assert_eq!(names(&s.me.take()).last().unwrap(), "ACTION_FAIL");
}

// ---- refusals ------------------------------------------------------------------------------------

/// `RequestMagicSkillUse`: a dead player or an unknown skill gets `ActionFailed`; a skill of
/// target `ONE` without a target too (`checkUseMagicConditions`).
#[test]
fn dead_unknown_and_targetless_requests_fail() {
    let mut s = Scene::new();
    s.use_skill(4242, false, false);
    assert_eq!(names(&s.me.take()), ["ACTION_FAIL"]);
    s.use_skill(WIND_STRIKE, false, false);
    assert_eq!(names(&s.me.take()), ["ACTION_FAIL"]);
    s.world.set_dead(ME, true);
    s.use_skill(MIGHT, false, false);
    assert_eq!(names(&s.me.take()), ["ACTION_FAIL"]);
}

// ---- NPCs ----------------------------------------------------------------------------------------

/// An NPC casts through the same entry point as `AttackableAI`: `npc.setTarget(target)` and
/// `npc.doCast(skill)`. Players around see it; the skill lands at the hit.
#[test]
fn an_npc_casts_through_the_same_entry_point() {
    let mut s = Scene::new();
    let mob = s.monster(200);
    s.me.take();
    s.world.set_npc_target(mob, Some(ME));
    s.world.do_cast(mob, SkillKey::new(SHIELD, 1));
    let sent = s.me.take();
    let msu: MagicSkillUse = decode(of(&sent, "MAGIC_SKILL_USE")[0]);
    assert_eq!((msu.creature_id, msu.target_id), (mob.0, ME.0));
    // No gauge and no "You use" for an NPC.
    assert!(of(&sent, "SETUP_GAUGE").is_empty());
    assert!(s.world.is_casting_now(mob));
    s.advance(6000);
    assert!(!s.world.is_casting_now(mob));
    assert!(s.world.is_affected_by_skill(ME, SHIELD));
}
