#![allow(clippy::unwrap_used, clippy::expect_used)]
//! L-06: the water zone (breath gauge, drowning), the falling damage, the teleport protection and
//! the item restriction check after a teleport. Expected values follow the Java code named in each
//! test (`WaterZone`, `WaterTask`, `Player.startWaterTask`, `Player.isFalling`,
//! `Formulas.calcFallDam`, `Player.onTeleported`).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, MapRegions, PlayerTemplates, SkillTemplates, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{SetupGauge, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::zones::ZoneId;
use l2_world::*;

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

fn template() -> String {
    let levels: String = (1..=20)
        .map(|l| format!(r#"<level val="{l}"><hp>1000</hp><mp>500</mp><cp>200</cp></level>"#))
        .collect();
    TEMPLATE.replace("@LEVELS@", &levels)
}

const TEMPLATE: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<baseBreath>50</baseBreath>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>@LEVELS@</lvlUpgainData></list>"#;

#[derive(Default)]
struct Data {
    stat: Option<StatData>,
}

impl StaticData for Data {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        self.stat.as_ref()
    }
}

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&template(), 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            "<list></list>",
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
/// Far from the water zone.
const DRY_X: i32 = X + 4_000;
const ME: ObjectId = ObjectId(0x2000_0001);

fn player_data() -> PlayerData {
    PlayerData {
        object_id: ME,
        account: "acc".into(),
        name: "Swimmer".into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 20,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: DRY_X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: 1000.0,
        max_hp: 1000.0,
        cur_mp: 500.0,
        max_mp: 500.0,
        cur_cp: 200.0,
        max_cp: 200.0,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

struct ZoneRoot(std::path::PathBuf);

impl ZoneRoot {
    fn new(tag: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-water-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["zones", "mapregion"] {
            std::fs::create_dir_all(dir.join("data").join(sub)).unwrap();
        }
        let zone = format!(
            "<zone id=\"1\" name=\"water\" type=\"WaterZone\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
             <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone>\n",
            X - 500,
            Y - 500,
            X + 500,
            Y + 500
        );
        std::fs::write(
            dir.join("data/zones/test.xml"),
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">{zone}</list>\n"),
        )
        .unwrap();
        std::fs::write(
            dir.join("data/mapregion/talking_island_town.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
             <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
             <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n\
             </region>\n</list>\n",
        )
        .unwrap();
        Self(dir)
    }
}

impl Drop for ZoneRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct Scene {
    world: World,
    me: Log,
    _root: ZoneRoot,
}

impl Scene {
    fn new(tag: &str, allow_water: bool, teleport_protection: i32) -> Self {
        let root = ZoneRoot::new(tag);
        let mut world = World::new(WorldRng::from_seed(7), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data {
            stat: Some(stat_data()),
        }));
        let mut settings = WorldSettings::default();
        settings.zones.allow_water = allow_water;
        settings.zones.enable_falling_damage = true;
        settings.teleport.player_teleport_protection = teleport_protection;
        world.set_settings(settings);
        world.set_regen_settings(RegenSettings {
            hp_regen_multiplier: 0.0,
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_zone_data(
            Arc::new(Zones::load(&root.0)),
            Arc::new(MapRegions::load(&root.0)),
        );
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(1),
            player: Box::new(player_data()),
            sink: Box::new(Sink(me.clone())),
            ip: None,
        });
        let mut scene = Self {
            world,
            me,
            _root: root,
        };
        scene.advance(1000);
        scene.me.take();
        scene
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    /// A teleport with the client's `Appearing` answer.
    fn teleport(&mut self, x: i32) {
        self.world.tele_to_location(ME, TeleportTarget::at(x, Y, Z));
        self.world.handle(WorldCommand::Input {
            session: SessionId(1),
            input: ClientInput::Appearing,
        });
    }

    fn hp(&self) -> f64 {
        self.world.player(ME).unwrap().core.cur_hp
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

fn messages(packets: &[Vec<u8>]) -> Vec<SystemMessage> {
    of(packets, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|p| decode::<SystemMessage>(p))
        .collect()
}

fn gauges(packets: &[Vec<u8>]) -> Vec<SetupGauge> {
    of(packets, "SETUP_GAUGE")
        .into_iter()
        .map(|p| decode::<SetupGauge>(p))
        .collect()
}

/// `WaterZone.onEnter` + `Player.revalidateZone` (`AllowWater`) -> `startWaterTask`: the breath
/// gauge (`SetupGauge(objectId, 2, 60000 * BREATH / 100)`; the test class has `baseBreath` 50),
/// then `WaterTask` after that time and every second: 1% of the max HP and the message.
#[test]
fn the_water_zone_starts_the_breath_gauge_and_drowns_the_player() {
    let mut s = Scene::new("drown", true, 0);
    let hp0 = s.hp();
    assert!(!s.world.is_water_task_running(ME));
    s.teleport(X);
    assert!(s.world.is_inside_zone(ME, ZoneId::Water));
    assert!(s.world.is_water_task_running(ME));
    let sent = s.me.take();
    let gauge = gauges(&sent);
    assert_eq!(gauge.len(), 1);
    assert_eq!(
        (gauge[0].object_id, gauge[0].color, gauge[0].time),
        (ME.0, 2, 30_000)
    );

    // Nothing happens while the breath lasts.
    s.advance(29_000);
    assert!((s.hp() - hp0).abs() < f64::EPSILON);
    // `scheduleAtFixedRate(.., timeInWater, 1000)`.
    s.advance(1_100);
    let max_hp = s.world.stats(ME).unwrap().max_hp();
    let damage = (f64::from(max_hp) / 100.0).max(1.0);
    assert!((s.hp() - (hp0 - damage)).abs() < 1e-6, "hp {}", s.hp());
    let drowned = messages(&s.me.take());
    assert_eq!(drowned.len(), 1);
    assert_eq!(
        drowned[0].id,
        SystemMessageId::YouHaveTakenS1DamageBecauseYouWereUnableToBreathe
    );
    assert_eq!(drowned[0].params, [SystemMessageParam::IntNumber(damage as i32)]);
    s.advance(2_000);
    assert!((s.hp() - (hp0 - 3.0 * damage)).abs() < 1e-6, "hp {}", s.hp());

    // `WaterZone.onExit`: the task stops, `SetupGauge(objectId, 2, 0)`.
    s.me.take();
    s.teleport(DRY_X);
    assert!(!s.world.is_inside_zone(ME, ZoneId::Water));
    assert!(!s.world.is_water_task_running(ME));
    let gauge = gauges(&s.me.take());
    assert_eq!(gauge.last().map(|g| (g.color, g.time)), Some((2, 0)));
    let hp = s.hp();
    s.advance(5_000);
    assert!((s.hp() - hp).abs() < f64::EPSILON);
}

/// Without `AllowWater` the zone flags the player but no breath task starts
/// (`if (GeneralConfig.ALLOW_WATER) checkWaterState()`).
#[test]
fn allow_water_off_starts_no_breath_task() {
    let mut s = Scene::new("nowater", false, 0);
    let hp0 = s.hp();
    s.teleport(X);
    assert!(s.world.is_inside_zone(ME, ZoneId::Water));
    assert!(!s.world.is_water_task_running(ME));
    s.advance(40_000);
    assert!((s.hp() - hp0).abs() < f64::EPSILON);
}

/// Leaving the world stops the task (`Player.deleteMe`: `stopAllTimers`).
#[test]
fn the_breath_task_stops_when_the_player_leaves() {
    let mut s = Scene::new("leave", true, 0);
    s.teleport(X);
    assert!(s.world.is_water_task_running(ME));
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    assert!(!s.world.is_water_task_running(ME));
}

/// `Formulas.calcFallDam`: `(fallHeight * maxHp) / 1000` as an int division, 0 for a negative
/// height; `isFalling` then reduces the HP 1.5 s later by `min(damage, hp - 1)` and tells the
/// player `YOU_RECEIVED_S1_FALLING_DAMAGE` with the full damage.
#[test]
fn falling_damage_lands_after_one_and_a_half_seconds() {
    let mut s = Scene::new("fall", true, 0);
    let hp0 = s.hp();
    let max_hp = s.world.stats(ME).unwrap().max_hp();
    assert_eq!(s.world.calc_fall_dam(ME, 500), f64::from(500 * max_hp / 1000));
    assert_eq!(s.world.calc_fall_dam(ME, -1), 0.0);

    s.world.schedule_falling_damage(ME, 500);
    let damage = 500 * max_hp / 1000;
    assert_eq!(s.world.pending_falling_damage(ME), damage);
    s.advance(1_400);
    assert!((s.hp() - hp0).abs() < f64::EPSILON);
    s.advance(200);
    assert!((s.hp() - (hp0 - f64::from(damage))).abs() < 1e-6, "hp {}", s.hp());
    let sent = messages(&s.me.take());
    let falling = sent
        .iter()
        .find(|m| m.id == SystemMessageId::YouReceivedS1FallingDamage)
        .expect("the falling damage message");
    assert_eq!(falling.params, [SystemMessageParam::IntNumber(damage)]);
    assert_eq!(s.world.pending_falling_damage(ME), 0);

    // A fall deeper than the HP never kills: `Math.min(_fallingDamage, getCurrentHp() - 1)`.
    s.world.schedule_falling_damage(ME, 50_000);
    s.advance(1_600);
    assert!((s.hp() - 1.0).abs() < 1e-6, "hp {}", s.hp());
}

/// `Player.onTeleported`: `PlayerTeleportProtection` seconds of invulnerability that end with the
/// time (`isTeleportProtected`).
#[test]
fn a_teleport_gives_teleport_protection() {
    let mut s = Scene::new("protect", true, 5);
    assert!(!s.world.is_teleport_protected(ME));
    s.teleport(X);
    assert!(s.world.is_teleport_protected(ME));
    assert!(s.world.is_invul(ME));
    s.advance(4_000);
    assert!(s.world.is_teleport_protected(ME));
    s.advance(1_200);
    assert!(!s.world.is_teleport_protected(ME));
    assert!(!s.world.is_invul(ME));
}

/// `PlayerTeleportProtection = 0` (the default) gives none.
#[test]
fn no_teleport_protection_by_default() {
    let mut s = Scene::new("noprotect", true, 0);
    s.teleport(X);
    assert!(!s.world.is_teleport_protected(ME));
}

/// `ConditionTargetMyPartyExceptMe`: on oneself "You cannot use this on yourself.", on a player
/// outside the party `S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS` with the skill's name, both sent
/// to the effector from the condition itself.
#[test]
fn the_party_condition_tells_the_effector_why_not() {
    use l2_data::stats::Condition;
    use l2_world::stats::CalcSkill;

    let mut s = Scene::new("cond", true, 0);
    let other = ObjectId(0x2000_0002);
    let mut data = player_data();
    data.object_id = other;
    data.name = "Friend".into();
    s.world.handle(WorldCommand::EnterWorld {
        session: SessionId(2),
        player: Box::new(data),
        sink: Box::new(Sink(Log::default())),
        ip: None,
    });
    s.advance(1000);
    s.me.take();
    let skill = CalcSkill {
        id: 1234,
        level: 3,
        magic: true,
    };
    let party = Condition::TargetMyPartyExceptMe { value: true };

    assert!(!s.world.test_condition(ME, Some(ME), Some(&skill), &party));
    let sent = messages(&s.me.take());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].id, SystemMessageId::YouCannotUseThisOnYourself);

    assert!(!s.world.test_condition(ME, Some(other), Some(&skill), &party));
    let sent = messages(&s.me.take());
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0].id, SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms);
    assert_eq!(
        sent[0].params,
        [SystemMessageParam::SkillName { id: 1234, level: 3 }]
    );

    // `_value == isPartyMember`: the negated condition holds for both, and still tells.
    let not_party = Condition::TargetMyPartyExceptMe { value: false };
    assert!(s.world.test_condition(ME, Some(other), Some(&skill), &not_party));
    assert_eq!(messages(&s.me.take()).len(), 1);
}
