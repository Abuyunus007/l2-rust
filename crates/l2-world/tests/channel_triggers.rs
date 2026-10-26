#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The skill channelizer, the weapon's `oncrit_skill` / `onmagic_skill` and the trigger skills of
//! item options (M6-FU-12). Expected behaviour comes from the Java code named in each test
//! (`SkillChannelizer`, `SkillChannelized`, `Weapon.castOnCriticalSkill` / `castOnMagicSkill`,
//! `Options.apply` / `remove`, `Creature.onHitTimer` / `callSkill` / `makeTriggerCast`).
//! The skills are synthetic; every chance is 100 so that the rolls are certain, and the world RNG
//! is seeded.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, AugmentationOption, AugmentationOptions, EnchantHpBonus, Hennas, ItemTemplates, MapRegions,
    PlayerTemplates, SkillTemplates, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll as pd;
use l2_protocol_game::server::{MagicSkillCanceled, MagicSkillLaunched, MagicSkillUse, SystemMessage};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

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

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

// ---- the datapack -------------------------------------------------------------------------------

const TEMPLATE: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>5000</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

/// 1: a sword. 2: a sword with `oncrit_skill` 9501. 3: a staff with `onmagic_skill` 9502.
const ITEMS: &str = r#"<list>
<item id="1" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat></stats></item>
<item id="2" type="Weapon" name="Crit Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat></stats>
<skills><skill id="9501" level="1" type="ON_CRITICAL_SKILL" type_chance="100"/></skills></item>
<item id="3" type="Weapon" name="Magic Staff"><set name="weapon_type" val="BLUNT" /><set name="bodypart" val="rhand" /><set name="weight" val="50" />
<stats><stat type="pAtk">50</stat></stats>
<skills><skill id="9502" level="1" type="ON_MAGIC_SKILL" type_chance="100"/></skills></item>
</list>"#;

/// Buffs have no stats: only that they land matters. Abnormal types differ so they all stay.
fn buff(id: i32, abnormal: &str, extra: &str) -> String {
    format!(
        r#"<skill id="{id}" levels="1" name="S{id}"><operateType>A2</operateType><targetType>ONE</targetType>
<abnormalTime>60</abnormalTime><abnormalType>{abnormal}</abnormalType><abnormalLevel>1</abnormalLevel>
<castRange>900</castRange><effectRange>1100</effectRange>{extra}<effects><effect name="Buff"/></effects></skill>"#
    )
}

fn skills_xml() -> String {
    let list = [
        // The weapon skills: the on-crit one, the on-magic one (a magic buff).
        buff(9501, "PD_UP", ""),
        buff(9502, "MD_UP", "<isMagic>1</isMagic>"),
        // The skills of the options: attack, critical, magic.
        buff(9503, "SPEED_UP", ""),
        buff(9504, "HP_REGEN_UP", ""),
        buff(9505, "CRITICAL_PROB_UP", "<isMagic>1</isMagic>"),
        // The skills ME casts: a magic buff, a harmful magic skill, a physical skill, a static one.
        buff(9510, "MULTI_BUFF", "<isMagic>1</isMagic><hitTime>500</hitTime>"),
        r#"<skill id="9511" levels="1" name="Harm"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>900</castRange><effectRange>1100</effectRange><effectPoint>-100</effectPoint><isMagic>1</isMagic><hitTime>500</hitTime></skill>"#
            .to_owned(),
        r#"<skill id="9512" levels="1" name="Strike"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>900</castRange><effectRange>1100</effectRange><isMagic>0</isMagic><hitTime>500</hitTime></skill>"#
            .to_owned(),
        // The channeling skill: 5 s cast, a tick every second paying 5 MP, and the skill it gives
        // the targets (two levels: one per channelizer).
        r#"<skill id="9520" levels="1" name="Channel"><operateType>CA1</operateType><targetType>ONE</targetType>
<castRange>900</castRange><effectRange>900</effectRange><isMagic>1</isMagic><hitTime>5000</hitTime>
<channelingSkillId>9521</channelingSkillId><channelingTickInterval>1</channelingTickInterval><mpPerChanneling>5</mpPerChanneling></skill>"#
            .to_owned(),
        r#"<skill id="9521" levels="2" name="Channeled"><operateType>A2</operateType><targetType>ONE</targetType>
<abnormalTime>60</abnormalTime><abnormalType>MULTI_BUFF_A</abnormalType>
<abnormalLevel level="1">1</abnormalLevel><abnormalLevel level="2">2</abnormalLevel>
<effects><effect name="Buff"/></effects></skill>"#
            .to_owned(),
    ];
    format!("<list>{}</list>", list.concat())
}

/// 11: attack 9503 (100 %). 12: critical 9504 (100 %). 13: magic 9505 (100 %). 14: attack 9503
/// with a chance of 0.
const OPTIONS: &str = r#"<list>
<option id="11"><attack_skill id="9503" level="1" chance="100"/></option>
<option id="12"><critical_skill id="9504" level="1" chance="100"/></option>
<option id="13"><magic_skill id="9505" level="1" chance="100"/></option>
<option id="14"><attack_skill id="9503" level="1" chance="0"/></option>
</list>"#;

struct Data {
    items: Arc<ItemTemplates>,
    stats: StatData,
    options: AugmentationOptions,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap(),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: t.shared_reuse_group,
            weapon: t.is_weapon(),
        })
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.stats
            .skills
            .iter()
            .any(|s| s.id == skill_id && s.is_passive())
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.items.get(item_id).is_some_and(|t| t.stackable)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
    fn augmentation_option(&self, id: i32) -> Option<&AugmentationOption> {
        self.options.get(id)
    }
}

fn data() -> Data {
    let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
    let skills = SkillTemplates::from_xml(
        &skills_xml(),
        &l2_data::skill::EnchantSkillGroups::default(),
        &l2_data::SkillLoadOptions::default(),
    );
    let options = AugmentationOptions::from_xml(OPTIONS, &skills).unwrap();
    let stats = StatData {
        items: Arc::clone(&items),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(skills),
    };
    Data {
        items,
        stats,
        options,
    }
}

// ---- helpers ------------------------------------------------------------------------------------

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const OTHER: ObjectId = ObjectId(0x2000_0002);
const THIRD: ObjectId = ObjectId(0x2000_0003);
const SWORD: i32 = 100;

const CHANNEL: i32 = 9520;
const CHANNELED: i32 = 9521;

fn worn(object_id: i32, item_id: i32, augmentation: Option<i32>) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count: 1,
        loc: "PAPERDOLL".into(),
        loc_data: i32::try_from(pd::RHAND).unwrap(),
        augmentation,
        ..Default::default()
    }
}

fn player(id: ObjectId, (x, y, z): (i32, i32, i32), items: Vec<ItemData>, mp: f64) -> PlayerData {
    let skills = [9510, 9511, 9512, CHANNEL]
        .into_iter()
        .map(|skill_id| SkillData {
            skill_id,
            skill_level: 1,
        })
        .collect();
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: format!("P{}", id.0 - 0x2000_0000),
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
        cur_mp: mp,
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
    log: Log,
    _root: tempfile::TempDir,
}

impl Scene {
    /// ME wears `items` and has `mp`; OTHER and THIRD stand 50 and 80 away.
    fn new(items: Vec<ItemData>, mp: f64) -> Self {
        Self::build(items, mp, None)
    }

    /// The same with a peace zone `(min_x, min_y, max_x, max_y)` loaded.
    fn with_peace_zone(items: Vec<ItemData>, mp: f64, rect: (i32, i32, i32, i32)) -> Self {
        Self::build(items, mp, Some(rect))
    }

    fn build(items: Vec<ItemData>, mp: f64, peace: Option<(i32, i32, i32, i32)>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(20_261_026), Box::new(NullOutbox));
        world.set_static_data(Box::new(data()));
        // No MP regeneration, so the costs can be read off the MP exactly.
        world.set_regen_settings(RegenSettings {
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        if let Some((x1, y1, x2, y2)) = peace {
            std::fs::create_dir_all(root.path().join("data/zones")).unwrap();
            std::fs::create_dir_all(root.path().join("data/mapregion")).unwrap();
            std::fs::write(
                root.path().join("data/zones/test.xml"),
                format!(
                    "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\
                     <zone id=\"1\" name=\"z1\" type=\"PeaceZone\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
                     <node X=\"{x1}\" Y=\"{y1}\"/><node X=\"{x2}\" Y=\"{y2}\"/></zone></list>\n"
                ),
            )
            .unwrap();
            world.set_zone_data(
                Arc::new(Zones::load(root.path())),
                Arc::new(MapRegions::load(root.path())),
            );
        }
        let mut scene = Self {
            world,
            log,
            _root: root,
        };
        scene.enter(player(ME, (X, Y, Z), items, mp));
        scene.enter(player(OTHER, (X + 50, Y, Z), vec![], 50.0));
        scene.enter(player(THIRD, (X, Y + 80, Z), vec![], 50.0));
        scene.advance(1000);
        scene.log.take();
        scene
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn advance(&mut self, ms: u64) -> Sent {
        let mut sent = Vec::new();
        for _ in 0..(ms / 50) {
            self.world.tick();
            sent.extend(self.log.take());
        }
        sent
    }

    fn cast(&mut self, who: ObjectId, skill: i32, target: ObjectId) {
        self.world.set_target(who, Some(target));
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who.0).unwrap()),
            input: ClientInput::MagicSkillUse {
                magic_id: skill,
                ctrl_pressed: false,
                shift_pressed: false,
            },
        });
    }

    /// `Creature.onHitTimer` of a landed hit of ME on OTHER.
    fn hit(&mut self, crit: bool) {
        self.world.on_hit_timer(HitTask {
            attacker: ME,
            target: OTHER,
            damage: 10,
            crit,
            miss: false,
            shld: 0,
            soulshot: false,
            recharge_shots: false,
        });
    }

    fn has(&self, who: ObjectId, skill: i32) -> bool {
        self.world.is_affected_by_skill(who, skill)
    }

    fn mp(&self, who: ObjectId) -> f64 {
        self.world.player(who).unwrap().core.cur_mp
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decoded<P: Decode>(out: &Sent, to: ObjectId, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(id, b)| *id == to.0 && name_of(b) == name)
        .map(|(_, b)| {
            let skip = if b[0] == 0xFE { 3 } else { 1 };
            P::decode(&mut PacketReader::new(&b[skip..])).unwrap()
        })
        .collect()
}

fn message_ids(out: &Sent, to: ObjectId) -> Vec<SystemMessageId> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|m| m.id)
        .collect()
}

// ---- the weapon's on-critical skill ---------------------------------------------------------------

/// `Weapon.castOnCriticalSkill`: after a critical hit the skill is activated on the target (no
/// animation, no cast time, no message). A hit that is not critical does nothing.
#[test]
fn a_critical_hit_activates_the_weapons_oncrit_skill() {
    let mut s = Scene::new(vec![worn(SWORD, 2, None)], 50.0);
    s.hit(false);
    assert!(!s.has(OTHER, 9501));
    s.hit(true);
    assert!(s.has(OTHER, 9501));
    // `activateSkill` only: nobody sees a `MagicSkillUse`, and the wearer reads no message.
    let sent = s.advance(100);
    assert!(decoded::<MagicSkillUse>(&sent, ME, "MAGIC_SKILL_USE").is_empty());
    assert!(!message_ids(&sent, ME).contains(&SystemMessageId::S1HasBeenActivated));
}

/// A sword without `oncrit_skill` has nothing to activate.
#[test]
fn a_plain_weapon_activates_nothing() {
    let mut s = Scene::new(vec![worn(SWORD, 1, None)], 50.0);
    s.hit(true);
    assert!(!s.has(OTHER, 9501));
}

// ---- the weapon's on-magic skill ------------------------------------------------------------------

/// `Weapon.castOnMagicSkill`: a good magic skill cast at the target also activates the staff's good
/// magic skill on it, and the wearer reads "S1 has been activated".
#[test]
fn a_magic_skill_activates_the_weapons_onmagic_skill() {
    let mut s = Scene::new(vec![worn(SWORD, 3, None)], 50.0);
    s.cast(ME, 9510, OTHER);
    let sent = s.advance(1500);
    assert!(s.has(OTHER, 9510));
    assert!(s.has(OTHER, 9502), "the on-magic skill landed");
    let messages = decoded::<SystemMessage>(&sent, ME, "SYSTEM_MESSAGE");
    assert!(
        messages
            .iter()
            .any(|m| m.id == SystemMessageId::S1HasBeenActivated)
    );
}

/// "Trigger only if both are good or bad magic": a harmful skill does not activate a good one.
#[test]
fn a_harmful_skill_does_not_activate_a_good_onmagic_skill() {
    let mut s = Scene::new(vec![worn(SWORD, 3, None)], 50.0);
    s.cast(ME, 9511, OTHER);
    s.advance(1500);
    assert!(!s.has(OTHER, 9502));
}

/// `caster.getAI().getCastTarget() != target`: with no cast running the AI has no cast target, so
/// the staff activates nothing.
#[test]
fn the_onmagic_skill_needs_the_cast_target() {
    let mut s = Scene::new(vec![worn(SWORD, 3, None)], 50.0);
    let skill = data().stats.skills.get(9510, 1).unwrap().clone();
    // No cast is running: the AI has no cast target.
    s.world.weapon_cast_on_magic_skill(ME, OTHER, &skill);
    assert!(!s.has(OTHER, 9502));
}

// ---- the trigger skills of options ----------------------------------------------------------------

/// `Options.apply` at login: the activation skills are on the creature, by skill id; taking the
/// item off (`Options.remove`) removes them.
#[test]
fn a_worn_augmentation_puts_its_trigger_skills_on_and_taking_it_off_removes_them() {
    // Options 11 (attack) and 12 (critical): low half and high half of the augmentation id.
    let mut s = Scene::new(vec![worn(SWORD, 1, Some(11 | (12 << 16)))], 50.0);
    let holders = s.world.trigger_skills(ME);
    let mut ids: Vec<i32> = holders.iter().map(|h| h.skill.id).collect();
    ids.sort_unstable();
    assert_eq!(ids, [9503, 9504]);
    assert!(s.world.trigger_skills(OTHER).is_empty());

    // Take the sword off (use it again).
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(ME.0).unwrap()),
        input: ClientInput::UseItem {
            object_id: SWORD,
            ctrl_pressed: false,
        },
    });
    s.advance(500);
    assert!(s.world.trigger_skills(ME).is_empty());
}

/// `Creature.onHitTimer`: an ATTACK holder fires on a normal hit and not on a critical one, a
/// CRITICAL holder the other way round. `makeTriggerCast` shows the cast to everybody.
#[test]
fn attack_and_critical_options_fire_on_the_matching_hit() {
    let mut s = Scene::new(vec![worn(SWORD, 1, Some(11 | (12 << 16)))], 50.0);
    s.hit(false);
    assert!(s.has(OTHER, 9503));
    assert!(!s.has(OTHER, 9504));
    let sent = s.advance(100);
    let uses = decoded::<MagicSkillUse>(&sent, OTHER, "MAGIC_SKILL_USE");
    assert_eq!(uses.len(), 1);
    assert_eq!(
        (
            uses[0].creature_id,
            uses[0].target_id,
            uses[0].skill_id,
            uses[0].hit_time
        ),
        (ME.0, OTHER.0, 9503, 0)
    );
    assert_eq!(
        decoded::<MagicSkillLaunched>(&sent, OTHER, "MAGIC_SKILL_LAUNCHED").len(),
        1
    );

    // The critical hit: the critical holder, not the attack one.
    s.world.stop_skill_effects(OTHER, SkillFinishType::Silent, 9503);
    s.hit(true);
    assert!(s.has(OTHER, 9504));
    assert!(!s.has(OTHER, 9503));
}

/// A holder with a chance of 0 never fires (`Rnd.get(100) < 0`).
#[test]
fn an_option_with_chance_zero_never_fires() {
    let mut s = Scene::new(vec![worn(SWORD, 1, Some(14))], 50.0);
    for _ in 0..20 {
        s.hit(false);
    }
    assert!(!s.has(OTHER, 9503));
}

/// `callSkill`: a MAGIC holder fires after a magic skill, not after a physical one; an ATTACK holder
/// the other way round.
#[test]
fn magic_options_fire_after_magic_skills_and_attack_options_after_physical_ones() {
    let mut s = Scene::new(vec![worn(SWORD, 1, Some(11 | (13 << 16)))], 50.0);
    s.cast(ME, 9512, OTHER);
    s.advance(1500);
    assert!(s.has(OTHER, 9503), "physical skill: attack holder");
    assert!(!s.has(OTHER, 9505), "physical skill: no magic holder");

    s.world.stop_skill_effects(OTHER, SkillFinishType::Silent, 9503);
    s.cast(ME, 9510, OTHER);
    s.advance(1500);
    assert!(s.has(OTHER, 9505), "magic skill: magic holder");
    assert!(!s.has(OTHER, 9503), "magic skill: no attack holder");
}

// ---- the channelizer ------------------------------------------------------------------------------

/// `SkillChannelizer`: the channeling starts with the cast, every tick pays `mpPerChanneling` and
/// gives the target the channeled skill (level = the number of channelizers), and `MagicSkillLaunched`
/// goes out for the target. It stops when the cast ends.
#[test]
fn a_channeled_skill_ticks_pays_mp_and_stops_with_the_cast() {
    let mut s = Scene::new(vec![], 50.0);
    s.cast(ME, CHANNEL, OTHER);
    s.advance(300);
    assert!(s.world.is_channeling(ME));
    assert!(s.world.is_casting_now(ME));
    assert_eq!(s.world.channeled_skill(ME), Some(SkillKey::new(CHANNEL, 1)));
    assert!(!s.world.has_channelized(ME), "no tick yet");
    let mp = s.mp(ME);

    // The first tick comes after the initial delay (the interval, 1 s).
    let sent = s.advance(1000);
    assert_eq!(s.mp(ME), mp - 5.0);
    assert!(s.has(OTHER, CHANNELED));
    assert!(s.world.has_channelized(ME));
    assert!(s.world.is_channelized(OTHER));
    assert_eq!(s.world.channelizers_size(OTHER, CHANNELED), 1);
    let launched = decoded::<MagicSkillLaunched>(&sent, OTHER, "MAGIC_SKILL_LAUNCHED");
    assert!(
        launched
            .iter()
            .any(|p| p.creature_id == ME.0 && p.skill_id == CHANNEL && p.target_ids == [OTHER.0])
    );

    // A second tick: MP again; the buff already has the level, so it stays.
    s.advance(1000);
    assert_eq!(s.mp(ME), mp - 10.0);

    // The cast ends at 5 s: the channelizer stops and lets the target go.
    s.advance(4000);
    assert!(!s.world.is_casting_now(ME));
    assert!(!s.world.is_channeling(ME));
    assert!(!s.world.has_channelized(ME));
    assert_eq!(s.world.channelizers_size(OTHER, CHANNELED), 0);
    assert!(!s.world.is_channelized(OTHER));
    let after = s.mp(ME);
    s.advance(3000);
    assert_eq!(s.mp(ME), after, "no tick after the channel stopped");
}

/// `SkillChannelizer.run`: "the skill level is the number of channelizers" - two casters at the
/// same target give it the second level.
#[test]
fn two_channelizers_give_the_second_level() {
    let mut s = Scene::new(vec![], 50.0);
    s.cast(ME, CHANNEL, OTHER);
    s.cast(THIRD, CHANNEL, OTHER);
    s.advance(1300);
    assert_eq!(s.world.channelizers_size(OTHER, CHANNELED), 2);
    let level = s
        .world
        .effect_list(OTHER)
        .unwrap()
        .info_by_skill_id(CHANNELED)
        .unwrap()
        .skill()
        .level;
    assert_eq!(level, 2);
}

/// `abortCast` stops the channelizer (`stopChanneling`).
#[test]
fn aborting_the_cast_stops_the_channel() {
    let mut s = Scene::new(vec![], 50.0);
    s.cast(ME, CHANNEL, OTHER);
    s.advance(1300);
    assert!(s.world.is_channelized(OTHER));
    s.world.abort_cast(ME);
    assert!(!s.world.is_channeling(ME));
    assert_eq!(s.world.channelizers_size(OTHER, CHANNELED), 0);
}

/// Not enough MP for a tick: "Your skill was deactivated due to lack of MP." and `abortCast`.
#[test]
fn a_tick_without_mp_deactivates_the_skill() {
    // 7 MP: the first tick pays 5, the second finds 2.
    let mut s = Scene::new(vec![], 7.0);
    s.cast(ME, CHANNEL, OTHER);
    s.advance(1300);
    assert_eq!(s.mp(ME), 2.0);
    let sent = s.advance(1000);
    assert!(message_ids(&sent, ME).contains(&SystemMessageId::YourSkillWasDeactivatedDueToLackOfMp));
    assert_eq!(
        decoded::<MagicSkillCanceled>(&sent, ME, "MAGIC_SKILL_CANCELED").len(),
        1
    );
    assert!(!s.world.is_casting_now(ME));
    assert!(!s.world.is_channeling(ME));
    assert_eq!(s.mp(ME), 2.0);
}

/// `Creature.doDie`: whoever channels at the dead creature stops (`abortChannelization`).
#[test]
fn the_death_of_the_target_aborts_the_channel() {
    let mut s = Scene::new(vec![], 50.0);
    s.cast(ME, CHANNEL, OTHER);
    s.advance(1300);
    assert!(s.world.is_channeling(ME));
    s.world.do_die(OTHER, Some(THIRD));
    assert!(!s.world.is_casting_now(ME));
    assert!(!s.world.is_channeling(ME));
    assert!(!s.world.is_channelized(OTHER));
}

/// `Formulas.calcAtkBreak`: a channeling creature's cast is never broken by a hit.
#[test]
fn a_channeling_creature_is_not_interrupted_by_damage() {
    let mut s = Scene::new(vec![], 50.0);
    s.cast(ME, CHANNEL, OTHER);
    s.advance(300);
    assert!(s.world.is_channeling(ME));
    let mut rng = WorldRng::from_seed(1);
    for _ in 0..50 {
        assert!(!l2_world::combat::hit::calc_atk_break_for(
            &s.world, ME, 100_000.0, &mut rng
        ));
    }
}

/// `Creature.checkDoCastConditions`: a channeling skill (a signet) is not cast where its effect
/// range reaches a peace zone, and the player reads "S1 cannot be used due to unsuitable terms".
#[test]
fn a_channeling_skill_is_not_cast_into_a_peace_zone() {
    // The zone is 800 away to the east: outside the caster, inside the effect range (900).
    let mut s = Scene::with_peace_zone(vec![], 50.0, (X + 800, Y - 100, X + 1000, Y + 100));
    s.cast(ME, CHANNEL, OTHER);
    let sent = s.advance(300);
    assert!(message_ids(&sent, ME).contains(&SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms));
    assert!(!s.world.is_casting_now(ME));
    assert!(!s.world.is_channeling(ME));
}

/// The same skill with the peace zone out of reach is cast.
#[test]
fn a_channeling_skill_is_cast_when_no_peace_zone_is_in_reach() {
    let mut s = Scene::with_peace_zone(vec![], 50.0, (X + 5000, Y - 100, X + 5200, Y + 100));
    s.cast(ME, CHANNEL, OTHER);
    s.advance(300);
    assert!(s.world.is_channeling(ME));
}
