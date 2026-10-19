#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Vitality and Nevit's blessing (M4-VIT-01): the exp of each vitality level, the points a kill
//! uses up, the recovery in a peace zone and after a logout, the advent blessing, the hourglass,
//! and what the persistence task is handed.
//!
//! The expected numbers are worked out by hand from Java's code (`PlayerStat.updateVitalityPoints`,
//! `PlayerStat.getExpBonusMultiplier`, `Attackable.getVitalityPoints`, `Nevit`), not from this
//! implementation.
//! Java: entity/actor/stat/PlayerStat.java, entity/actor/Attackable.java,
//! entity/actor/holders/player/Nevit.java, entity/actor/Player.java (the Nevit hourglass)

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, ItemTemplates, KarmaLoss, MapRegions,
    NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates, SkillTrees, SpawnData,
    SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::enterworld::ExVoteSystemInfo;
use l2_protocol_game::server::{
    ExNevitAdventEffect, ExNevitAdventPointInfo, ExNevitAdventTimeChange, ExVitalityPointInfo, SystemMessage,
    UserInfo,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::rewards::nevit::nevit_reset_boundary_ms;
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

#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Persisted {
    /// The last character snapshot the world stored.
    fn last_store(&self) -> PlayerData {
        self.0
            .lock()
            .unwrap()
            .iter()
            .rev()
            .find_map(|c| match c {
                PersistCommand::StorePlayer(p) => Some((**p).clone()),
                _ => None,
            })
            .expect("no StorePlayer")
    }

    fn clear(&self) {
        self.0.lock().unwrap().clear();
    }
}

struct RecordingOutbox(Persisted);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Data(StatData);

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
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        AccessLevelStatic {
            is_gm: level >= 100,
            ..AccessLevelStatic::USER
        }
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

/// Decodes a packet with a one byte opcode.
fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

/// Decodes an `0xFE` packet (opcode and sub opcode).
fn decode_ex<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[3..])).unwrap()
}

fn names(sent: &Sent, to: i32) -> Vec<String> {
    sent.iter()
        .filter(|(id, _)| *id == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

fn of<'a>(sent: &'a Sent, to: i32, name: &str) -> Vec<&'a [u8]> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == name)
        .map(|(_, b)| b.as_slice())
        .collect()
}

/// The ids of the `SystemMessage`s one client got.
fn message_ids(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    of(sent, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw).id)
        .collect()
}

fn vitality_infos(sent: &Sent, to: i32) -> Vec<i32> {
    of(sent, to, "EX_VITALITY_POINT_INFO")
        .iter()
        .map(|raw| decode_ex::<ExVitalityPointInfo>(raw).points)
        .collect()
}

fn effect_infos(sent: &Sent, to: i32) -> Vec<i32> {
    of(sent, to, "EX_NAVIT_ADVENT_EFFECT")
        .iter()
        .map(|raw| decode_ex::<ExNevitAdventEffect>(raw).time_left)
        .collect()
}

fn point_infos(sent: &Sent, to: i32) -> Vec<i32> {
    of(sent, to, "EX_NAVIT_ADVENT_POINT_INFO")
        .iter()
        .map(|raw| decode_ex::<ExNevitAdventPointInfo>(raw).points)
        .collect()
}

/// (started, time) of each `ExNevitAdventTimeChange`.
fn time_changes(sent: &Sent, to: i32) -> Vec<(bool, i32)> {
    of(sent, to, "EX_NAVIT_ADVENT_TIME_CHANGE")
        .iter()
        .map(|raw| {
            let p = decode_ex::<ExNevitAdventTimeChange>(raw);
            (p.started, p.time)
        })
        .collect()
}

/// (time, bonus, type) of each `ExVoteSystemInfo`.
fn vote_infos(sent: &Sent, to: i32) -> Vec<(i32, i32, i32)> {
    of(sent, to, "EX_VOTE_SYSTEM_INFO")
        .iter()
        .map(|raw| {
            let p = decode_ex::<ExVoteSystemInfo>(raw);
            (p.bonus_time, p.bonus_value, p.bonus_type)
        })
        .collect()
}

// ------------------------------------------------------------------ synthetic data

/// The exp a level starts at: a round number a million apart, so a kill never levels.
fn exp_of_level(level: i32) -> i64 {
    i64::from(level - 1) * 1_000_000
}

fn experience() -> Arc<ExperienceTable> {
    let rows: String = (1..=86)
        .map(|level| {
            format!(
                r#"<experience level="{level}" tolevel="{}" />"#,
                exp_of_level(level)
            )
        })
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
        ))
        .unwrap(),
    )
}

/// One class whose HP, MP and CP are 100, 50 and 20 per level.
fn class_xml() -> String {
    let rows: String = (1..=85)
        .map(|l| {
            format!(
                "<level val=\"{l}\"><hp>{}</hp><mp>{}</mp><cp>{}</cp></level>",
                100 * l,
                50 * l,
                20 * l
            )
        })
        .collect();
    format!(
        r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{rows}</lvlUpgainData></list>"#
    )
}

/// A monster of `kind` (`Monster`, `RaidBoss`, `GrandBoss`) with 300 HP.
fn npcs_xml(kind: &str, level: i32, exp: i32, sp: i32) -> String {
    format!(
        r#"<?xml version="1.0"?><list>
  <npc id="9001" type="{kind}" level="{level}" name="Monster">
    <acquire exp="{exp}" sp="{sp}" />
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
</list>"#
    )
}

fn synthetic_stats() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&class_xml(), 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            "<list></list>",
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: i32 = 0x2000_0001;

fn player_data(level: i32, profile: PlayerProfile) -> PlayerData {
    let (hp, mp, cp) = (
        f64::from(100 * level),
        f64::from(50 * level),
        f64::from(20 * level),
    );
    PlayerData {
        object_id: ObjectId(ME),
        account: "acc".into(),
        name: "Hero".into(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
        exp: exp_of_level(level),
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: hp,
        max_hp: hp,
        cur_mp: mp,
        max_mp: mp,
        cur_cp: cp,
        max_cp: cp,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile,
    }
}

/// A profile of a character who logged out a moment ago with `points` vitality points.
fn profile(points: i32) -> PlayerProfile {
    PlayerProfile {
        vitality_points: points,
        last_access: now_ms(),
        ..PlayerProfile::default()
    }
}

fn now_ms() -> i64 {
    l2_util::time::now_millis()
}

/// A temporary datapack root with one monster spawn, a map region and the zone file.
struct Root(PathBuf);

impl Root {
    fn new(tag: &str, zones: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-vit-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["spawns", "zones", "mapregion"] {
            std::fs::create_dir_all(dir.join("data").join(sub)).unwrap();
        }
        std::fs::write(
            dir.join("data/spawns/a.xml"),
            r#"<?xml version="1.0"?><list enabled="true"><spawn><npc id="9001" x="-71338" y="258271" z="-3104" heading="77" respawnDelay="20"/></spawn></list>"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("data/zones/test.xml"),
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">{zones}</list>\n"),
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

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// A peace zone around the monster and the player.
const PEACE: &str = r#"<zone id="1" name="peace" type="PeaceZone" shape="Cuboid" minZ="-5000" maxZ="5000">
<node X="-72000" Y="257000"/><node X="-70000" Y="259000"/></zone>"#;

/// What a scene is made of.
struct Config {
    kind: &'static str,
    npc_level: i32,
    npc_exp: i32,
    settings: WorldSettings,
    zones: &'static str,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            kind: "Monster",
            npc_level: 10,
            npc_exp: 1000,
            settings: WorldSettings::default(),
            zones: "",
        }
    }
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
    _root: Root,
}

impl Scene {
    fn new(tag: &str) -> Self {
        Self::with(tag, Config::default())
    }

    fn with(tag: &str, config: Config) -> Self {
        let root = Root::new(tag, config.zones);
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(
            WorldRng::from_seed(5),
            Box::new(RecordingOutbox(persisted.clone())),
        );
        let mut settings = config.settings;
        settings.death = DeathSettings {
            disconnect_after_death: false,
            ..DeathSettings::default()
        };
        world.set_settings(settings);
        world.set_static_data(Box::new(Data(synthetic_stats())));
        world.set_exp_data(
            experience(),
            Arc::new(
                ExperienceLoss::from_xml(r#"<list><experience level="10" val="8.875" /></list>"#).unwrap(),
            ),
            Arc::new(KarmaLoss::from_xml(r#"<list><modifier level="10" val="2.0" /></list>"#).unwrap()),
            Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
        );
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_zone_data(
            Arc::new(Zones::load(&root.0)),
            Arc::new(MapRegions::load(&root.0)),
        );
        let npcs = Arc::new(
            NpcTemplates::from_xml(
                &npcs_xml(config.kind, config.npc_level, config.npc_exp, 100),
                &NpcLoadOptions::default(),
            )
            .unwrap(),
        );
        let spawns = Arc::new(SpawnData::load(&root.0, &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.spawn_all_npcs(false);
        // Java's `RaidBossSpawnManager` spawns raid bosses from its own table, the spawn files
        // skip them: place one like an admin spawn does.
        if world.npc_ids().is_empty() {
            world.spawn_npc_at(9001, X, Y, Z, 0).unwrap();
        }
        Self {
            world,
            log,
            persisted,
            _root: root,
        }
    }

    /// The player enters; what he was sent while entering is kept in the log.
    fn enter_keeping_packets(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn enter(&mut self, data: PlayerData) {
        self.enter_keeping_packets(data);
        self.log.take();
        self.persisted.clear();
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn npc(&self) -> ObjectId {
        self.world.npc_ids().into_iter().next().unwrap()
    }

    fn me(&self) -> &Player {
        self.world.player(ObjectId(ME)).unwrap()
    }

    fn id(&self) -> ObjectId {
        ObjectId(ME)
    }

    /// The player did `damage` to the monster, then it dies.
    fn kill_with(&mut self, damage: i64) {
        let npc = self.npc();
        self.world.add_damage_hate(npc, self.id(), damage, 1);
        assert!(self.world.do_die(npc, Some(self.id())));
    }

    fn variable(&self, name: &str) -> Option<String> {
        self.me().data.profile.variables.get(name).cloned()
    }

    fn logout(&mut self) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME).unwrap()),
            input: ClientInput::Logout,
        });
    }
}

fn settings(vitality: VitalitySettings) -> WorldSettings {
    WorldSettings {
        vitality,
        ..WorldSettings::default()
    }
}

// ------------------------------------------------------------------ exp by vitality level

/// A kill of the 1000 exp / 100 sp monster at each vitality level: the rate of the level
/// (1.5, 2, 2.5, 3) replaces 1.0, and `Math.round`-ed exp and the bonus message follow.
#[test]
fn a_kill_gives_the_exp_of_the_vitality_level() {
    // (points, expected exp, expected sp, level)
    let cases = [
        (1, 1000, 100, 0),
        (240, 1000, 100, 0),
        (241, 1500, 150, 1),
        (2000, 1500, 150, 1),
        (2001, 2000, 200, 2),
        (13000, 2000, 200, 2),
        (13001, 2500, 250, 3),
        (17000, 2500, 250, 3),
        (17001, 3000, 300, 4),
        (20000, 3000, 300, 4),
    ];
    for (points, exp, sp, level) in cases {
        let mut s = Scene::new("levels");
        s.enter(player_data(10, profile(points)));
        assert_eq!(s.world.vitality_level(s.id()), level, "{points} points");
        s.kill_with(100);
        assert_eq!(
            (s.me().data.exp - exp_of_level(10), s.me().data.sp),
            (exp, sp),
            "{points} points"
        );
        let sent = s.log.take();
        let ids = message_ids(&sent, ME);
        if level == 0 {
            assert!(
                ids.contains(&SystemMessageId::YouHaveEarnedS1ExperienceAndS2Sp),
                "{ids:?}"
            );
        } else {
            // The bonus makes it the "bonus" message.
            assert!(
                ids.contains(&SystemMessageId::YouHaveAcquiredS1ExpBonusS2AndS3SpBonusS4),
                "{points}: {ids:?}"
            );
        }
    }
}

/// `RateVitalityLevelN` come from the settings; a rate below 1 gives nothing (the bonus only
/// grows), and `EnableVitality = False` turns the whole system off.
#[test]
fn the_rates_and_the_switch_come_from_the_settings() {
    let mut s = Scene::with(
        "rates",
        Config {
            settings: settings(VitalitySettings {
                rate_vitality_level: [1.2, 0.5, 2.5, 3.0],
                ..VitalitySettings::default()
            }),
            ..Config::default()
        },
    );
    // Level 1 (2000 points) pays 1.2: the f32 1.2 is 1.2000000476837158 as a double.
    s.enter(player_data(10, profile(2000)));
    s.kill_with(100);
    assert_eq!(s.me().data.exp - exp_of_level(10), 1200);

    // Level 2 pays 0.5: `if (vitality > 1.0)` leaves the bonus at 1.
    let mut s = Scene::with(
        "rates2",
        Config {
            settings: settings(VitalitySettings {
                rate_vitality_level: [1.2, 0.5, 2.5, 3.0],
                ..VitalitySettings::default()
            }),
            ..Config::default()
        },
    );
    s.enter(player_data(10, profile(5000)));
    s.kill_with(100);
    assert_eq!(s.me().data.exp - exp_of_level(10), 1000);

    // Disabled: no bonus, no points used up, no points given back.
    let mut s = Scene::with(
        "disabled",
        Config {
            settings: settings(VitalitySettings {
                enable_vitality: false,
                ..VitalitySettings::default()
            }),
            ..Config::default()
        },
    );
    s.enter(player_data(10, profile(15000)));
    s.kill_with(100);
    assert_eq!(s.me().data.exp - exp_of_level(10), 1000);
    assert_eq!(s.world.vitality_points(s.id()), 15000);
    assert_eq!(s.me().data.profile.vitality_points, 15000);
}

// ------------------------------------------------------------------ points used by a kill

/// `Attackable.getVitalityPoints`: with baseHpMax 300, level 10, exp reward 1000 the divider is
/// `300 * 9 * 10 * 10 / (100 * 1000)` = 2.7, and 100 damage use up `100 / 2.7` = 37.037 points.
/// Java stores `(int)` of the float, so 15000 - 37.037 = 14962.96 is 14962.
#[test]
fn a_kill_uses_up_vitality_points_by_the_damage_done() {
    let mut s = Scene::new("use-up");
    s.enter(player_data(10, profile(15000)));
    s.kill_with(100);
    assert_eq!(s.world.vitality_points(s.id()), 14962);
    // The number `UserInfo` shows and the persistence task writes.
    assert_eq!(s.me().data.profile.vitality_points, 14962);
    assert_eq!(s.me().snapshot().profile.vitality_points, 14962);
    // `updateVitalityPoints` sends no `ExVitalityPointInfo` (only the recovery task does).
    let sent = s.log.take();
    assert!(vitality_infos(&sent, ME).is_empty());

    // The damage counts up to the monster's maximum HP (300).
    let mut s = Scene::new("use-up-max");
    s.enter(player_data(10, profile(15000)));
    s.kill_with(5000);
    // 300 / 2.7 = 111.11 -> 14888.89
    assert_eq!(s.world.vitality_points(s.id()), 14888);

    // `RateVitalityLost` multiplies the loss: 37.037 * 0.6 = 22.22 -> 14977.78.
    let mut s = Scene::with(
        "use-up-rate",
        Config {
            settings: settings(VitalitySettings {
                rate_vitality_lost: 0.6,
                ..VitalitySettings::default()
            }),
            ..Config::default()
        },
    );
    s.enter(player_data(10, profile(15000)));
    s.kill_with(100);
    assert_eq!(s.world.vitality_points(s.id()), 14977);
}

/// The vitality level falls with the points and the player is told, with the extra message for
/// "fully exhausted" at level 0.
#[test]
fn the_level_falls_with_the_messages_of_java() {
    // 2010 points: level 2 -> 1973 is level 1.
    let mut s = Scene::new("fall");
    s.enter(player_data(10, profile(2010)));
    assert_eq!(s.world.vitality_level(s.id()), 2);
    s.kill_with(100);
    assert_eq!(s.world.vitality_points(s.id()), 1972);
    assert_eq!(s.world.vitality_level(s.id()), 1);
    let ids = message_ids(&s.log.take(), ME);
    assert!(
        ids.contains(&SystemMessageId::YourVitalityHasDecreased),
        "{ids:?}"
    );
    assert!(!ids.contains(&SystemMessageId::YourVitalityIsFullyExhausted));

    // 250 points: level 1 -> 212 is level 0: decreased and fully exhausted.
    let mut s = Scene::new("exhausted");
    s.enter(player_data(10, profile(250)));
    s.kill_with(100);
    assert_eq!(s.world.vitality_level(s.id()), 0);
    let ids = message_ids(&s.log.take(), ME);
    assert!(
        ids.contains(&SystemMessageId::YourVitalityHasDecreased),
        "{ids:?}"
    );
    assert!(
        ids.contains(&SystemMessageId::YourVitalityIsFullyExhausted),
        "{ids:?}"
    );

    // The points never go below 1.
    let mut s = Scene::new("floor");
    s.enter(player_data(10, profile(10)));
    s.kill_with(100);
    assert_eq!(s.world.vitality_points(s.id()), 1);
}

/// `setVitalityPoints` clamps, announces the level and sends `ExVitalityPointInfo`.
#[test]
fn set_vitality_points_clamps_and_tells_the_client() {
    let mut s = Scene::new("set");
    s.enter(player_data(10, profile(1000)));
    s.world.set_vitality_points(s.id(), 99_999, false);
    assert_eq!(s.world.vitality_points(s.id()), 20_000);
    let sent = s.log.take();
    assert_eq!(vitality_infos(&sent, ME), vec![20_000]);
    let ids = message_ids(&sent, ME);
    assert_eq!(
        ids,
        vec![
            SystemMessageId::YourVitalityHasIncreased,
            SystemMessageId::YourVitalityIsAtMaximum
        ]
    );
    // The same value again does nothing.
    s.world.set_vitality_points(s.id(), 20_000, false);
    assert!(s.log.take().is_empty());
    // Quiet: no message, the packet still goes.
    s.world.set_vitality_points(s.id(), -5, true);
    assert_eq!(s.world.vitality_points(s.id()), 1);
    let sent = s.log.take();
    assert_eq!(vitality_infos(&sent, ME), vec![1]);
    assert!(message_ids(&sent, ME).is_empty());
}

// ------------------------------------------------------------------ bosses

/// A raid boss follows `RaidbossUseVitality`; with it off the kill gets no bonus of any kind and
/// the points stay (Java passes `useVitalityRate()` as `useBonuses` too).
#[test]
fn a_raid_boss_follows_raidboss_use_vitality() {
    let off = settings(VitalitySettings {
        raidboss_use_vitality: false,
        ..VitalitySettings::default()
    });
    let mut s = Scene::with(
        "raid-off",
        Config {
            kind: "RaidBoss",
            settings: off,
            ..Config::default()
        },
    );
    s.enter(player_data(10, profile(15000)));
    s.kill_with(100);
    assert_eq!(s.me().data.exp - exp_of_level(10), 1000, "no vitality bonus");
    assert_eq!(s.world.vitality_points(s.id()), 15000);

    // On: the bonus applies, and (NOTE(java-quirk)) a boss kill gives points back: the cost
    // -37.037 becomes +0.37037, times `RateVitalityGain`.
    let on = settings(VitalitySettings {
        raidboss_use_vitality: true,
        rate_vitality_gain: 100.0,
        ..VitalitySettings::default()
    });
    let mut s = Scene::with(
        "raid-on",
        Config {
            kind: "RaidBoss",
            settings: on,
            ..Config::default()
        },
    );
    s.enter(player_data(10, profile(15000)));
    s.kill_with(100);
    assert_eq!(s.me().data.exp - exp_of_level(10), 2500, "level 3 pays 2.5");
    // 0.37037 * 100 = 37.037 more points.
    assert_eq!(s.world.vitality_points(s.id()), 15037);
}

/// A grand boss never uses the vitality rate.
#[test]
fn a_grand_boss_ignores_vitality() {
    let mut s = Scene::with(
        "grand",
        Config {
            kind: "GrandBoss",
            ..Config::default()
        },
    );
    s.enter(player_data(10, profile(20000)));
    s.kill_with(100);
    assert_eq!(s.me().data.exp - exp_of_level(10), 1000);
    assert_eq!(s.world.vitality_points(s.id()), 20000);
}

// ------------------------------------------------------------------ recovery

/// `VitalityTask`: the first run after 1 s, then every minute; in a peace zone a player below the
/// maximum gets `RateRecoveryPeaceZone` points and an `ExVitalityPointInfo`.
#[test]
fn the_vitality_task_recovers_points_in_a_peace_zone() {
    let mut s = Scene::with(
        "peace",
        Config {
            zones: PEACE,
            settings: settings(VitalitySettings {
                rate_recovery_vitality_peace_zone: 5.0,
                ..VitalitySettings::default()
            }),
            ..Config::default()
        },
    );
    s.enter(player_data(10, profile(5000)));
    assert!(s.world.is_inside_zone(s.id(), ZoneId::Peace));
    // Nothing before the first run (20 ticks = 1 s).
    s.ticks(19);
    assert!(vitality_infos(&s.log.take(), ME).is_empty());
    s.ticks(1);
    let sent = s.log.take();
    assert_eq!(vitality_infos(&sent, ME), vec![5005]);
    assert_eq!(s.world.vitality_points(s.id()), 5005);
    assert_eq!(s.me().data.profile.vitality_points, 5005);
    // Then once a minute (1200 ticks).
    s.ticks(1199);
    assert!(vitality_infos(&s.log.take(), ME).is_empty());
    s.ticks(1);
    assert_eq!(vitality_infos(&s.log.take(), ME), vec![5010]);
}

/// The task does nothing outside a peace zone or at the maximum.
#[test]
fn the_vitality_task_does_nothing_outside_a_peace_zone_or_at_the_maximum() {
    let mut s = Scene::new("no-peace");
    s.enter(player_data(10, profile(5000)));
    s.ticks(1300);
    assert!(vitality_infos(&s.log.take(), ME).is_empty());
    assert_eq!(s.world.vitality_points(s.id()), 5000);

    let mut s = Scene::with(
        "peace-max",
        Config {
            zones: PEACE,
            ..Config::default()
        },
    );
    s.enter(player_data(10, profile(20000)));
    s.ticks(1300);
    assert!(vitality_infos(&s.log.take(), ME).is_empty());
}

/// `RecoverVitalityOnReconnect`: `RateRecoveryOnReconnect` (4) points a minute since the last
/// logout, quietly, after the first `UserInfo`.
#[test]
fn the_time_offline_gives_vitality_back_on_login() {
    let mut s = Scene::new("reconnect");
    let mut p = profile(1000);
    // Ten minutes ago: 4 * 600000 / 60000 = 40 points (a little more, the test takes some ms).
    p.last_access = now_ms() - 600_000;
    s.enter_keeping_packets(player_data(10, p));
    assert_eq!(s.world.vitality_points(s.id()), 1040);
    assert_eq!(s.me().data.profile.vitality_points, 1040);
    let sent = s.log.take();
    // Quiet: no message, no ExVitalityPointInfo.
    assert!(vitality_infos(&sent, ME).is_empty());
    assert!(!message_ids(&sent, ME).contains(&SystemMessageId::YourVitalityHasIncreased));
    // The first UserInfo is sent before the recovery, the later ones show the new number.
    let infos: Vec<i32> = of(&sent, ME, "USER_INFO")
        .iter()
        .map(|raw| decode::<UserInfo>(raw).vitality_points)
        .collect();
    assert_eq!(infos.first(), Some(&1000));
    assert_eq!(infos.last(), Some(&1040));

    // Switched off.
    let mut s = Scene::with(
        "reconnect-off",
        Config {
            settings: settings(VitalitySettings {
                recover_vitality_on_reconnect: false,
                ..VitalitySettings::default()
            }),
            ..Config::default()
        },
    );
    let mut p = profile(1000);
    p.last_access = now_ms() - 600_000;
    s.enter(player_data(10, p));
    assert_eq!(s.world.vitality_points(s.id()), 1000);

    // A long absence fills it up and stops at the maximum.
    let mut s = Scene::new("reconnect-max");
    let mut p = profile(19_990);
    p.last_access = now_ms() - 86_400_000;
    s.enter(player_data(10, p));
    assert_eq!(s.world.vitality_points(s.id()), 20_000);
}

// ------------------------------------------------------------------ the store

/// The persistence task is handed the whole number of the points, and the character leaves with
/// the timers gone.
#[test]
fn the_snapshot_carries_the_whole_number_of_points() {
    let mut s = Scene::new("store-vit");
    s.enter(player_data(10, profile(15000)));
    s.kill_with(100);
    s.persisted.clear();
    s.logout();
    assert_eq!(s.persisted.last_store().profile.vitality_points, 14962);
    // The vitality task is cancelled with the player.
    s.ticks(1300);
}

// ------------------------------------------------------------------ Nevit: the blessing

fn blessed_profile(points: i32, seconds: i32) -> PlayerProfile {
    let mut p = profile(points);
    p.variables.insert("nevit_b".into(), seconds.to_string());
    p
}

/// A player who logged out with 180 s of Nevit's blessing left gets it back at login, with the
/// packets and the message, and it counts as vitality level 4 (3x exp) and turns the vitality
/// cost into a gain (`stat = -10`).
#[test]
fn the_blessing_comes_back_at_login_and_acts_as_vitality_level_4() {
    let mut s = Scene::new("blessing");
    s.enter_keeping_packets(player_data(10, blessed_profile(5000, 180)));
    let sent = s.log.take();
    assert_eq!(effect_infos(&sent, ME), vec![180]);
    assert!(
        message_ids(&sent, ME).contains(
            &SystemMessageId::TheAngelNevitHasBlessedYouFromAboveYouAreImbuedWithFullVitalityAsWellAsAVitalityReplenishingEffectAndShouldYouDieYouWillNotLoseExp
        )
    );
    assert!(s.world.is_advent_blessing_active(s.id()));
    assert_eq!(s.world.vitality_level(s.id()), 4);

    // 3x exp although the points are level 1, and the 37.037 points are gained, not used.
    s.kill_with(100);
    assert_eq!(s.me().data.exp - exp_of_level(10), 3000);
    assert_eq!(s.world.vitality_points(s.id()), 5037);
    // The real level (from the points) is still 1: it shows when the blessing ends.
    s.ticks(3600);
    assert!(!s.world.is_advent_blessing_active(s.id()));
    assert_eq!(s.world.vitality_level(s.id()), 2);
}

/// After `time * 1000` ms the blessing ends: `ExNevitAdventEffect(0)`, the points, the message,
/// and the variable `nevit_b` goes.
#[test]
fn the_blessing_ends_after_its_time() {
    let mut s = Scene::new("blessing-end");
    s.enter(player_data(10, blessed_profile(5000, 10)));
    assert_eq!(s.variable("nevit_b").as_deref(), Some("10"));
    s.ticks(199);
    assert!(s.world.is_advent_blessing_active(s.id()));
    s.ticks(1);
    assert!(!s.world.is_advent_blessing_active(s.id()));
    let sent = s.log.take();
    assert_eq!(effect_infos(&sent, ME), vec![0]);
    assert_eq!(point_infos(&sent, ME), vec![0]);
    assert_eq!(
        message_ids(&sent, ME),
        vec![SystemMessageId::NevitSAdventBlessingHasEndedContinueYourJourneyAndYouWillSurelyMeetHisFavorAgainSometimeSoon]
    );
    assert_eq!(s.variable("nevit_b"), None);
    assert!(s.me().data.profile.variables_removed.contains("nevit_b"));
}

/// A death under the blessing costs no exp (`!isAdventBlessingActive()` in `doDie`).
#[test]
fn a_death_under_the_blessing_costs_no_exp() {
    let mut s = Scene::new("blessing-death");
    let mut data = player_data(10, blessed_profile(5000, 180));
    data.exp += 400_000;
    s.enter(data);
    let before = s.me().data.exp;
    let npc = s.npc();
    assert!(s.world.do_die(s.id(), Some(npc)));
    assert_eq!(s.me().data.exp, before);

    // Without the blessing the same death costs exp.
    let mut s = Scene::new("no-blessing-death");
    let mut data = player_data(10, profile(5000));
    data.exp += 400_000;
    s.enter(data);
    let before = s.me().data.exp;
    let npc = s.npc();
    assert!(s.world.do_die(s.id(), Some(npc)));
    assert!(s.me().data.exp < before);
}

// ------------------------------------------------------------------ Nevit: points and advent time

/// After a kill the advent task is scheduled (30 s): `ExNevitAdventTimeChange(started)` at once,
/// and when it runs the advent time grows by 30 and 72 points are earned.
#[test]
fn hunting_earns_nevit_points_in_30_second_steps() {
    let mut s = Scene::new("advent");
    s.enter(player_data(10, profile(1)));
    s.kill_with(100);
    let sent = s.log.take();
    assert_eq!(time_changes(&sent, ME), vec![(true, 0)]);
    s.ticks(599);
    assert!(point_infos(&s.log.take(), ME).is_empty());
    s.ticks(1);
    let sent = s.log.take();
    assert_eq!(s.variable("hunting_time").as_deref(), Some("30"));
    assert_eq!(s.variable("hunting_points").as_deref(), Some("72"));
    assert_eq!(point_infos(&sent, ME), vec![72]);
    // The packet with the time goes out when the minute is full only.
    assert!(time_changes(&sent, ME).is_empty());

    // The task is over after one run; the next gain of exp starts it again, and the run that
    // makes the time 60 sends the time.
    assert!(s.world.add_exp_and_sp(s.id(), 10.0, 0.0, true));
    assert_eq!(time_changes(&s.log.take(), ME), vec![(true, 30)]);
    s.ticks(600);
    let sent = s.log.take();
    assert_eq!(s.variable("hunting_time").as_deref(), Some("60"));
    assert_eq!(s.variable("hunting_points").as_deref(), Some("144"));
    assert_eq!(time_changes(&sent, ME), vec![(true, 60)]);
    assert_eq!(point_infos(&sent, ME), vec![144]);
}

/// The daily time is full at `NevitAdventTime` (14400): the task is not started any more and the
/// client is told it is paused.
#[test]
fn the_advent_time_stops_at_its_limit() {
    let mut s = Scene::new("advent-limit");
    let mut p = profile(1);
    p.variables.insert("hunting_time".into(), "14400".into());
    s.enter(player_data(10, p));
    assert!(s.world.add_exp_and_sp(s.id(), 10.0, 0.0, true));
    s.ticks(700);
    let sent = s.log.take();
    assert!(time_changes(&sent, ME).is_empty());
    assert_eq!(s.variable("hunting_points"), None);

    // 14390 s: one more run makes 14420, which is cut to the limit; the run stops with a paused
    // packet and earns no points.
    let mut s = Scene::new("advent-limit2");
    let mut p = profile(1);
    p.variables.insert("hunting_time".into(), "14390".into());
    s.enter(player_data(10, p));
    assert!(s.world.add_exp_and_sp(s.id(), 10.0, 0.0, true));
    s.log.take();
    s.ticks(600);
    let sent = s.log.take();
    assert_eq!(s.variable("hunting_time").as_deref(), Some("14400"));
    assert_eq!(s.variable("hunting_points"), None);
    assert_eq!(time_changes(&sent, ME), vec![(false, 14400)]);
}

/// A full bar (over `NevitMaxPoints`) starts the 180 s blessing and empties the bar; the messages
/// come at 45%, 50% and 75%.
#[test]
fn a_full_bar_starts_the_blessing() {
    let mut s = Scene::new("bar");
    let mut p = profile(1);
    // 3200 + 72 = 3272 points is 45.4%.
    p.variables.insert("hunting_points".into(), "3200".into());
    s.enter(player_data(10, p));
    s.world.nevit_add_points(s.id(), 72);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, ME),
        vec![SystemMessageId::YouAreStartingToFeelTheEffectsOfNevitSAdventBlessing]
    );
    assert_eq!(point_infos(&sent, ME), vec![3272]);

    // Over the maximum (7200): the bar is empty, the blessing starts.
    s.world.nevit_add_points(s.id(), 4000);
    let sent = s.log.take();
    assert_eq!(s.variable("hunting_points").as_deref(), Some("0"));
    assert_eq!(effect_infos(&sent, ME), vec![180]);
    assert_eq!(point_infos(&sent, ME), vec![0]);
    assert_eq!(s.variable("nevit_b").as_deref(), Some("180"));
    assert!(s.world.is_advent_blessing_active(s.id()));

    // During the blessing the bar stays empty.
    s.world.nevit_add_points(s.id(), 72);
    assert_eq!(s.variable("hunting_points").as_deref(), Some("0"));
}

/// A new level gives 2000 points (`PlayerStat.addLevel`) after the `ExVoteSystemInfo`.
#[test]
fn a_level_up_gives_2000_points() {
    let mut s = Scene::new("level-points");
    s.enter(player_data(10, profile(1)));
    assert!(s.world.add_exp(s.id(), 1_000_000));
    assert_eq!(s.me().core.level, 11);
    assert_eq!(s.variable("hunting_points").as_deref(), Some("2000"));
    let sent = s.log.take();
    assert_eq!(point_infos(&sent, ME), vec![2000]);
    let order = names(&sent, ME);
    let vote = order.iter().position(|n| n == "EX_VOTE_SYSTEM_INFO").unwrap();
    let points = order
        .iter()
        .position(|n| n == "EX_NAVIT_ADVENT_POINT_INFO")
        .unwrap();
    assert!(vote < points, "{order:?}");
}

/// `NevitEnabled = False`: no packets, no variables.
#[test]
fn nevit_can_be_switched_off() {
    let mut s = Scene::with(
        "nevit-off",
        Config {
            settings: WorldSettings {
                nevit_enabled: false,
                ..WorldSettings::default()
            },
            ..Config::default()
        },
    );
    s.enter_keeping_packets(player_data(10, blessed_profile(1, 180)));
    let sent = s.log.take();
    assert!(effect_infos(&sent, ME).is_empty());
    assert!(
        !names(&sent, ME)
            .iter()
            .any(|n| n.starts_with("EX_NAVIT") || n == "EX_VOTE_SYSTEM_INFO")
    );
    s.kill_with(100);
    assert!(
        !names(&s.log.take(), ME)
            .iter()
            .any(|n| n.starts_with("EX_NAVIT") || n == "EX_VOTE_SYSTEM_INFO")
    );
    assert_eq!(s.variable("hunting_points"), None);
}

/// The reset of the advent time at login happens for a character whose `lastAccess` is 0 (a
/// quirk: Java compares milliseconds with seconds), when it is past 06:30 of the server's day.
#[test]
fn the_advent_time_reset_at_login_follows_the_java_quirk() {
    let now = now_ms();
    let past_boundary = now > nevit_reset_boundary_ms(now, l2_util::time::local_offset_millis(now));

    let mut s = Scene::new("reset-0");
    let mut p = profile(1);
    p.last_access = 0;
    p.variables.insert("hunting_time".into(), "500".into());
    s.enter(player_data(10, p));
    let expected = if past_boundary { "0" } else { "500" };
    assert_eq!(s.variable("hunting_time").as_deref(), Some(expected));

    // A character with a real `lastAccess` (milliseconds, far above the boundary in seconds) is
    // never reset by it.
    let mut s = Scene::new("reset-1");
    let mut p = profile(1);
    p.variables.insert("hunting_time".into(), "500".into());
    s.enter(player_data(10, p));
    assert_eq!(s.variable("hunting_time").as_deref(), Some("500"));
}

// ------------------------------------------------------------------ Nevit: the hourglass

fn hourglass_profile(recom_have: i32, time_left_ms: i64) -> PlayerProfile {
    PlayerProfile {
        recom_have,
        recom_left: 20,
        recom_time_left_ms: time_left_ms,
        ..profile(1)
    }
}

/// An active hourglass with recommendations adds `NEVIT_HOURGLASS_BONUS[level / 10][have / 10]`
/// percent: level 45 with 25 recommendations is row 4, column 2: 25%. It adds to the vitality
/// bonus (`1 + 0.5 + 0.25`), it does not multiply it.
#[test]
fn the_hourglass_adds_its_bonus_to_the_exp() {
    let mut s = Scene::with(
        "hourglass",
        Config {
            npc_level: 45,
            ..Config::default()
        },
    );
    let mut p = hourglass_profile(25, 3_600_000);
    p.vitality_points = 1000; // level 1: 1.5
    s.enter_keeping_packets(player_data(45, p));
    // ExVoteSystemInfo at login: 3600 s, 25%, active.
    let sent = s.log.take();
    assert_eq!(vote_infos(&sent, ME).first(), Some(&(3600, 25, 0)));
    s.kill_with(100);
    // 1 + 0.5 + 0.25 = 1.75
    assert_eq!(s.me().data.exp - exp_of_level(45), 1750);

    // Without recommendations the hourglass runs but gives nothing.
    let mut s = Scene::with(
        "hourglass-no-rec",
        Config {
            npc_level: 45,
            ..Config::default()
        },
    );
    s.enter(player_data(45, hourglass_profile(0, 3_600_000)));
    s.kill_with(100);
    assert_eq!(s.me().data.exp - exp_of_level(45), 1000);
}

/// The hourglass ends after its time and tells the client; a stored time of 0 never starts one.
#[test]
fn the_hourglass_ends_and_a_zero_time_never_starts() {
    let mut s = Scene::new("hourglass-end");
    s.enter(player_data(45, hourglass_profile(25, 5_000)));
    assert_eq!(s.world.nevit_hourglass_time(s.id()), 5);
    s.ticks(99);
    assert!(vote_infos(&s.log.take(), ME).is_empty());
    s.ticks(1);
    // (time 0, bonus 0, "maintain" 10)
    assert_eq!(vote_infos(&s.log.take(), ME), vec![(0, 0, 10)]);
    assert!(!s.world.is_nevit_hourglass_active(s.id()));

    let mut s = Scene::new("hourglass-zero");
    s.enter(player_data(45, hourglass_profile(25, 0)));
    assert_eq!(s.world.nevit_hourglass_time(s.id()), 0);
}

/// A peace zone pauses the hourglass and the advent time (`PeaceZone.onEnter`), and no exp gain
/// in it starts them. The time left is what the next store writes.
#[test]
fn a_peace_zone_pauses_the_hourglass() {
    let mut s = Scene::with(
        "hourglass-peace",
        Config {
            zones: PEACE,
            ..Config::default()
        },
    );
    // Logging in inside the zone does not pause (the Java capture sends nothing for it).
    s.enter_keeping_packets(player_data(45, hourglass_profile(25, 3_600_000)));
    let infos = vote_infos(&s.log.take(), ME);
    assert!(infos.iter().all(|info| *info == (3600, 25, 0)), "{infos:?}");
    assert_eq!(s.world.nevit_hourglass_time(s.id()), 3600);
    s.logout();

    // Walking in later does: a player outside the zone teleports into it.
    let mut s = Scene::with(
        "hourglass-peace2",
        Config {
            zones: PEACE,
            ..Config::default()
        },
    );
    let mut data = player_data(45, hourglass_profile(25, 3_600_000));
    data.x = X + 10_000;
    s.enter(data);
    assert_eq!(s.world.nevit_hourglass_time(s.id()), 3600);
    s.world.tele_to_location(s.id(), TeleportTarget::at(X, Y, Z));
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(ME).unwrap()),
        input: ClientInput::Appearing,
    });
    assert!(s.world.is_inside_zone(s.id(), ZoneId::Peace));
    let sent = s.log.take();
    // `stopAdventTask(true)` tells the client the advent time is paused, `storeRecommendations`
    // cancels the hourglass, and the hourglass is "paused" from now on.
    assert_eq!(time_changes(&sent, ME), vec![(false, 0)]);
    assert!(s.world.is_nevit_hourglass_active(s.id()));
    assert_eq!(s.world.nevit_hourglass_time(s.id()), 0);
    assert_eq!(s.world.nevit_hourglass_status(s.id()), 1);
    // The cancelled hourglass leaves its time for the store.
    assert_eq!(s.me().data.profile.recom_time_left_ms, 3_600_000);
    // A pause with the bonus still shown: the multiplier is the hourglass' 1.25.
    assert!((s.world.nevit_hourglass_multiplier(s.id()) - 1.25).abs() < 1e-9);

    // No advent task in a peace zone.
    assert!(s.world.add_exp_and_sp(s.id(), 10.0, 0.0, true));
    s.ticks(700);
    assert_eq!(s.variable("hunting_time"), None);
}

/// The hourglass time is stored as the milliseconds left, and a logout stores it.
#[test]
fn the_store_carries_the_hourglass_time_and_the_nevit_variables() {
    let mut s = Scene::new("store-nevit");
    s.enter(player_data(10, hourglass_profile(3, 3_600_000)));
    // 10 s later (200 ticks).
    s.ticks(200);
    s.kill_with(100);
    s.ticks(600);
    s.persisted.clear();
    s.logout();
    let stored = s.persisted.last_store();
    let profile = &stored.profile;
    assert!(profile.reco_store);
    assert_eq!(
        (profile.recom_have, profile.recom_left, profile.recom_time_left_ms),
        (3, 20, 3_600_000 - 40_000)
    );
    assert_eq!(
        profile.variables.get("hunting_time").map(String::as_str),
        Some("30")
    );
    assert_eq!(
        profile.variables.get("hunting_points").map(String::as_str),
        Some("72")
    );
    assert!(profile.variables_changed.contains("hunting_time"));
    assert!(profile.variables_changed.contains("hunting_points"));
    assert!(profile.variables_removed.is_empty());
}

/// Logging out during the blessing keeps its time in `nevit_b`.
#[test]
fn a_logout_keeps_the_time_of_the_blessing() {
    let mut s = Scene::new("logout-blessing");
    s.enter(player_data(10, blessed_profile(1, 100)));
    // 30 s pass: 70 s left.
    s.ticks(600);
    s.persisted.clear();
    s.logout();
    let stored = s.persisted.last_store();
    assert_eq!(
        stored.profile.variables.get("nevit_b").map(String::as_str),
        Some("70")
    );
    assert!(stored.profile.variables_changed.contains("nevit_b"));
}

/// An autosave stores the recommendations (`Player.autoSave`) with the same snapshot.
#[test]
fn an_autosave_stores_the_hourglass_time() {
    let mut s = Scene::new("autosave");
    s.enter(player_data(10, hourglass_profile(3, 3_600_000)));
    // 15 minutes (the default autosave interval) = 18000 ticks.
    s.ticks(18_000);
    let stored = s.persisted.last_store();
    assert!(stored.profile.reco_store);
    assert_eq!(stored.profile.recom_time_left_ms, 3_600_000 - 900_000);
}
