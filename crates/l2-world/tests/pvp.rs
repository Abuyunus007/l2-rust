#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The PvP flag, karma, PK and PvP kills, the relation packets and `isAutoAttackable` between
//! players (M6-PVP-01).
//!
//! Expected behaviour comes from the Java code named in each test (`Player.updatePvPStatus`,
//! `PvpFlagTaskManager`, `Player.onKillUpdatePvPKarma`, `Player.setKarma`, `Player.getRelation`,
//! `Player.isAutoAttackable`, `Creature.callSkill`), the karma numbers from the Java vector file
//! `java-vectors/out/formulas_karma_gain.json`.
//! Java: entity/actor/{Player,Playable,Creature}.java, taskmanagers/PvpFlagTaskManager.java

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, MapRegions, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{NicknameChanged, RelationChanged, RelationEntry, StatusUpdate, UserInfo};
use l2_world::pvp::calculate_karma_gain;
use l2_world::stats::{BaseStatBonus, StatData};
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

impl Outbox for Persisted {
    fn persist(&mut self, command: PersistCommand) {
        self.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
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

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap_or(5),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: -1,
            weapon: t.is_weapon(),
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        if level >= 100 {
            AccessLevelStatic {
                is_gm: true,
                ..AccessLevelStatic::USER
            }
        } else {
            AccessLevelStatic::USER
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

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
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

/// Every `RelationChanged` entry `to` got, in order.
fn relations(sent: &Sent, to: i32) -> Vec<RelationEntry> {
    of(sent, to, "RELATION_CHANGED")
        .into_iter()
        .flat_map(|raw| decode::<RelationChanged>(raw).relations)
        .collect()
}

/// The entries of `subject` only.
fn relations_of(sent: &Sent, to: i32, subject: i32) -> Vec<RelationEntry> {
    relations(sent, to)
        .into_iter()
        .filter(|e| e.object_id == subject)
        .collect()
}

fn karma_updates(sent: &Sent, to: i32) -> Vec<i32> {
    of(sent, to, "STATUS_UPDATE")
        .iter()
        .flat_map(|raw| decode::<StatusUpdate>(raw).attributes)
        .filter(|(attr, _)| *attr == status::KARMA)
        .map(|(_, value)| value)
        .collect()
}

// ------------------------------------------------------------------ data

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

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />
<stats><stat type="pAtk">50</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat><stat type="pAtkRange">40</stat></stats></item>
</list>"#;

const WIND_STRIKE: i32 = 1177;
const HEAL: i32 = 9003;

/// Wind Strike (offensive, effect point -92) and a heal (effect point 100), as in `cast.rs`.
const SKILLS: &str = r##"<list>
<skill id="1177" levels="1" name="Wind Strike">
	<operateType>A1</operateType><targetType>ONE</targetType><castRange>600</castRange>
	<effectPoint>-92</effectPoint><effectRange>1100</effectRange><element>2</element><elementPower>20</elementPower>
	<hitTime>4000</hitTime><isMagic>1</isMagic><magicLevel>1</magicLevel><mpConsume>7</mpConsume>
	<mpInitialConsume>2</mpInitialConsume><power>12</power><reuseDelay>1200</reuseDelay>
	<effects><effect name="MagicalDamage" /></effects>
</skill>
<skill id="9003" levels="1" name="Heal"><operateType>A1</operateType><targetType>ONE</targetType>
<castRange>600</castRange><effectRange>1100</effectRange><effectPoint>100</effectPoint>
<hitTime>1000</hitTime><isMagic>1</isMagic><mpConsume>3</mpConsume>
<effects><effect name="Heal"><param power="10"/></effect></effects></skill>
</list>"##;

/// 9001 a monster, 9002 a raid boss, 9003 a folk NPC, 9004 a guard.
const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="9001" type="Monster" level="1" name="Gremlin">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9002" type="RaidBoss" level="1" name="Boss">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="30000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9003" type="Folk" level="10" name="Villager">
    <collision><radius normal="8"/><height normal="23"/></collision>
  </npc>
  <npc id="9004" type="Guard" level="10" name="Guard">
    <collision><radius normal="8"/><height normal="23"/></collision>
  </npc>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
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

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x2000_0001);
const P2: ObjectId = ObjectId(0x2000_0002);
const P3: ObjectId = ObjectId(0x2000_0003);

fn player_data(id: ObjectId, name: &str, (x, y, z): (i32, i32, i32), items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: id,
        account: format!("acc{}", id.0),
        name: name.into(),
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
        skills: [WIND_STRIKE, HEAL]
            .into_iter()
            .map(|skill_id| SkillData {
                skill_id,
                skill_level: 1,
            })
            .collect(),
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn sword() -> Vec<ItemData> {
    vec![ItemData {
        object_id: ObjectId(0x3000_0001),
        item_id: 2,
        count: 1,
        enchant_level: 0,
        loc: "PAPERDOLL".into(),
        loc_data: i32::try_from(paperdoll::RHAND).unwrap(),
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }]
}

/// A datapack root with one zone file and the default map region.
struct Root(PathBuf);

impl Root {
    fn new(tag: &str, zone_xml: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-pvp-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["zones", "mapregion"] {
            std::fs::create_dir_all(dir.join("data").join(sub)).unwrap();
        }
        std::fs::write(
            dir.join("data/zones/test.xml"),
            format!("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">{zone_xml}</list>\n"),
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

fn cuboid(id: i32, ty: &str) -> String {
    format!(
        "<zone id=\"{id}\" name=\"z{id}\" type=\"{ty}\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
         <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone>\n",
        X - 2_000,
        Y - 2_000,
        X + 2_000,
        Y + 2_000
    )
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
    _root: Root,
}

impl Scene {
    fn new(tag: &str) -> Self {
        Self::build(tag, PvpSettings::default(), "")
    }

    fn with_pvp(tag: &str, pvp: PvpSettings) -> Self {
        Self::build(tag, pvp, "")
    }

    fn with_zone(tag: &str, zone_type: &str) -> Self {
        Self::build(tag, PvpSettings::default(), &cuboid(1, zone_type))
    }

    fn build(tag: &str, pvp: PvpSettings, zones: &str) -> Self {
        let root = Root::new(tag, zones);
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(persisted.clone()));
        world.set_settings(WorldSettings {
            pvp,
            death: DeathSettings {
                disconnect_after_death: false,
                ..DeathSettings::default()
            },
            ..WorldSettings::default()
        });
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_regen_settings(RegenSettings {
            mp_regen_multiplier: 0.0,
            hp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_zone_data(
            Arc::new(Zones::load(&root.0)),
            Arc::new(MapRegions::load(&root.0)),
        );
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        Self {
            world,
            log,
            persisted,
            _root: root,
        }
    }

    fn enter(&mut self, id: ObjectId, dx: i32, items: Vec<ItemData>) {
        self.enter_data(player_data(id, &format!("P{:X}", id.0), (X + dx, Y, Z), items));
    }

    fn enter_data(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    /// Three players next to each other; the log is emptied after the first second.
    fn three(&mut self) {
        self.enter(ME, 0, sword());
        self.enter(P2, 40, vec![]);
        self.enter(P3, 80, vec![]);
        self.ticks(20);
        self.log.take();
    }

    fn input(&mut self, id: ObjectId, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id.0).unwrap()),
            input,
        });
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn advance_ms(&mut self, ms: u64) {
        self.ticks(u32::try_from(ms / 50).unwrap());
    }

    fn flag(&self, id: ObjectId) -> i32 {
        self.world.pvp_flag_of(id)
    }

    fn data(&self, id: ObjectId) -> &PlayerData {
        &self.world.player(id).unwrap().data
    }

    fn kill(&mut self, victim: ObjectId, killer: ObjectId) {
        self.world.reduce_current_hp(victim, 1_000_000.0, Some(killer));
    }

    fn use_skill(&mut self, id: ObjectId, magic_id: i32, ctrl: bool) {
        self.input(
            id,
            ClientInput::MagicSkillUse {
                magic_id,
                ctrl_pressed: ctrl,
                shift_pressed: false,
            },
        );
    }
}

// ------------------------------------------------------------------ the formula

/// `Formulas.calculateKarmaGain` against the real class (`FormulaVectors.writeKarmaGain`).
#[test]
fn karma_gain_matches_the_java_vectors() {
    let path =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../java-vectors/out/formulas_karma_gain.json");
    let Ok(text) = std::fs::read_to_string(&path) else {
        eprintln!("skipped: {} is missing", path.display());
        return;
    };
    let json: serde_json::Value = serde_json::from_str(&text).unwrap();
    let cases = json["cases"].as_array().unwrap();
    assert!(cases.len() >= 30);
    for case in cases {
        let pk = i32::try_from(case["pk_count"].as_i64().unwrap()).unwrap();
        let summon = case["is_summon"].as_bool().unwrap();
        let want = i32::try_from(case["result"].as_i64().unwrap()).unwrap();
        assert_eq!(calculate_karma_gain(pk, summon), want, "pk {pk} summon {summon}");
    }
}

// ------------------------------------------------------------------ the flag

/// `updatePvPStatus(target)`: attacking a player without karma flags the attacker for
/// `PvPVsNormalTime`, or `PvPVsPvPTime` when the target is flagged too (`checkIfPvP`). The flag is
/// told to the player (`UserInfo`) and to everybody who sees it (`RelationChanged`).
#[test]
fn flagging_by_an_attack_on_a_player_uses_the_normal_or_the_pvp_time() {
    let mut s = Scene::new("flag-times");
    s.three();
    let now = s.world.world_ms();

    s.world.update_pvp_status_target(ME, P2);
    assert_eq!(s.flag(ME), 1);
    assert_eq!(s.world.pvp_flag_lasts(ME), now + 120_000);
    let sent = s.log.take();
    // Player.updatePvPFlag: updateUserInfo, then a RelationChanged for everybody who sees it.
    assert_eq!(names(&sent, ME.0), ["USER_INFO", "EX_BR_EXTRA_USER_INFO"]);
    let user_info: UserInfo = decode(of(&sent, ME.0, "USER_INFO")[0]);
    assert_eq!(user_info.pvp_flag, 1);
    for viewer in [P2, P3] {
        let entries = relations_of(&sent, viewer.0, ME.0);
        assert_eq!(
            entries,
            [RelationEntry {
                object_id: ME.0,
                relation: 0,
                auto_attackable: true,
                karma: 0,
                pvp_flag: 1,
            }]
        );
    }

    // A flagged target: the shorter PvP time.
    let mut other = Scene::new("flag-times-2");
    other.three();
    other.world.update_pvp_status(P2);
    let now = other.world.world_ms();
    other.world.update_pvp_status_target(ME, P2);
    assert_eq!(other.world.pvp_flag_lasts(ME), now + 60_000);
}

/// No flag against a target with karma, against oneself, or when both stand in a PvP zone.
#[test]
fn some_attacks_do_not_flag() {
    let mut s = Scene::new("noflag");
    let mut chaotic = player_data(P2, "Chaotic", (X + 40, Y, Z), vec![]);
    chaotic.karma = 100;
    s.enter(ME, 0, sword());
    s.enter_data(chaotic);
    s.world.update_pvp_status_target(ME, P2);
    assert_eq!(s.flag(ME), 0);
    s.world.update_pvp_status_target(ME, ME);
    assert_eq!(s.flag(ME), 0);
    // A monster is not a player: `target.asPlayer()` is null.
    s.world.update_pvp_status_target(ME, ObjectId(0x7000_0001));
    assert_eq!(s.flag(ME), 0);

    let mut z = Scene::with_zone("noflag-zone", "ArenaZone");
    z.three();
    assert!(z.world.is_inside_zone(ME, ZoneId::Pvp));
    z.world.update_pvp_status_target(ME, P2);
    z.world.update_pvp_status(ME);
    assert_eq!(z.flag(ME), 0);
}

/// `PvpFlagTaskManager`: the flag stays 1, blinks (2) for the last 20 seconds and ends when the
/// time is over; every change is a `RelationChanged` for those who see the player. After the end
/// nothing is left on the timer wheel for the player.
#[test]
fn the_flag_task_blinks_and_ends() {
    let mut s = Scene::with_pvp(
        "task",
        PvpSettings {
            pvp_normal_time_ms: 30_000,
            ..PvpSettings::default()
        },
    );
    s.three();
    s.world.update_pvp_status(ME);
    s.advance_ms(9_000);
    assert_eq!(s.flag(ME), 1);
    s.advance_ms(2_000); // 11 s: more than 20 s before the end (30 s)
    assert_eq!(s.flag(ME), 2);
    s.advance_ms(18_000); // 29 s
    assert_eq!(s.flag(ME), 2);
    s.advance_ms(2_000); // 31 s: over
    assert_eq!(s.flag(ME), 0);
    let flags: Vec<i32> = relations_of(&s.log.take(), P3.0, ME.0)
        .iter()
        .map(|e| e.pvp_flag)
        .collect();
    assert_eq!(flags, [1, 2, 0]);
    // Flagged again later: it starts a new pass.
    s.world.update_pvp_status(ME);
    assert_eq!(s.flag(ME), 1);
}

/// A new attack while flagged only moves the end (no second timer, no packets); the blink goes
/// back to 1 on the next pass.
#[test]
fn flagging_again_extends_the_flag() {
    let mut s = Scene::with_pvp(
        "extend",
        PvpSettings {
            pvp_normal_time_ms: 30_000,
            ..PvpSettings::default()
        },
    );
    s.three();
    s.world.update_pvp_status(ME);
    s.advance_ms(15_000);
    assert_eq!(s.flag(ME), 2);
    s.log.take();
    s.world.update_pvp_status(ME);
    assert_eq!(s.world.pvp_flag_lasts(ME), s.world.world_ms() + 30_000);
    assert!(s.log.take().is_empty(), "no packets for a refresh");
    s.advance_ms(1_100);
    assert_eq!(s.flag(ME), 1);
    // 30 s after the refresh it is over, once.
    s.advance_ms(35_000);
    assert_eq!(s.flag(ME), 0);
}

/// Logging out stops the task (`stopAllTimers` -> `stopPvpRegTask`): the timer is gone with the
/// player and nothing fires for it later.
#[test]
fn leaving_the_world_drops_the_flag_timer() {
    let mut s = Scene::new("leave");
    s.three();
    s.world.update_pvp_status(ME);
    s.input(ME, ClientInput::Logout);
    s.log.take();
    s.advance_ms(130_000);
    assert!(s.log.take().is_empty());
    assert_eq!(s.flag(ME), 0);
}

/// A real attack with the sword on a flagged player flags the attacker, with `PvPVsPvPTime`.
#[test]
fn a_melee_attack_on_a_flagged_player_flags_the_attacker() {
    let mut s = Scene::new("melee");
    s.three();
    s.world.update_pvp_status(P2);
    s.log.take();
    for _ in 0..2 {
        s.input(
            ME,
            ClientInput::Action {
                object_id: P2.0,
                action_id: 0,
            },
        );
    }
    let mut flagged = false;
    for _ in 0..60 {
        s.ticks(1);
        if s.flag(ME) == 1 {
            flagged = true;
            break;
        }
    }
    assert!(flagged, "the attack flags the attacker");
    // P2 was flagged when the attack began: the PvP time.
    let left = s.world.pvp_flag_lasts(ME) - s.world.world_ms();
    assert!((59_000..=60_000).contains(&left), "{left}");
    let sent = s.log.take();
    assert!(
        relations_of(&sent, P3.0, ME.0)
            .iter()
            .any(|e| e.pvp_flag == 1 && e.auto_attackable)
    );
}

/// An attack on a raid boss flags the attacker through the plain `updatePvPStatus()`
/// (`FlagPlayerOnRaidAttack`); with the option off a monster does not flag anyone.
#[test]
fn attacking_a_raid_boss_flags_the_attacker_when_configured() {
    for (raid_flag, want) in [(true, 1), (false, 0)] {
        let mut s = Scene::with_pvp(
            "raid",
            PvpSettings {
                flag_player_on_raid_attack: raid_flag,
                ..PvpSettings::default()
            },
        );
        s.three();
        let boss = s.world.spawn_npc_at(9002, X + 30, Y + 30, Z, 0).unwrap();
        for _ in 0..2 {
            s.input(
                ME,
                ClientInput::Action {
                    object_id: boss.0,
                    action_id: 0,
                },
            );
        }
        s.ticks(80);
        assert_eq!(s.flag(ME), want, "FlagPlayerOnRaidAttack = {raid_flag}");
    }
}

// ------------------------------------------------------------------ casting

/// `Creature.callSkill`: an offensive skill on a player flags the caster after the cast, a skill
/// on a raid boss flags it (raid option), a heal flags only when the target is flagged, has karma
/// or is an attackable NPC.
#[test]
fn casting_flags_like_call_skill() {
    // Wind Strike on a plain player (force use).
    let mut s = Scene::new("cast-offense");
    s.three();
    s.world.set_target(ME, Some(P2));
    s.use_skill(ME, WIND_STRIKE, true);
    assert_eq!(s.flag(ME), 0, "flagged when the spell lands, not when it starts");
    s.advance_ms(5_000);
    assert_eq!(s.flag(ME), 1);
    let left = s.world.pvp_flag_lasts(ME) - s.world.world_ms();
    assert!(left > 110_000 && left <= 120_000, "{left}");

    // Wind Strike on a raid boss, raid flagging off: a monster does not flag.
    let mut s = Scene::with_pvp(
        "cast-raid-off",
        PvpSettings {
            flag_player_on_raid_attack: false,
            ..PvpSettings::default()
        },
    );
    s.three();
    let boss = s.world.spawn_npc_at(9002, X + 30, Y + 30, Z, 0).unwrap();
    s.world.set_target(ME, Some(boss));
    s.use_skill(ME, WIND_STRIKE, false);
    s.advance_ms(5_000);
    assert_eq!(s.flag(ME), 0);

    // ... and on, through `updatePvPStatus()`.
    let mut s = Scene::new("cast-raid-on");
    s.three();
    let boss = s.world.spawn_npc_at(9002, X + 30, Y + 30, Z, 0).unwrap();
    s.world.set_target(ME, Some(boss));
    s.use_skill(ME, WIND_STRIKE, false);
    s.advance_ms(5_000);
    assert_eq!(s.flag(ME), 1);

    // Heal on a plain player: no flag. On a flagged player or one with karma: flag.
    let mut s = Scene::new("cast-heal");
    s.three();
    s.world.set_target(ME, Some(P2));
    s.use_skill(ME, HEAL, false);
    s.advance_ms(3_000);
    assert_eq!(s.flag(ME), 0, "healing a peaceful player");
    s.world.update_pvp_status(P2);
    s.use_skill(ME, HEAL, false);
    s.advance_ms(3_000);
    assert_eq!(s.flag(ME), 1, "healing a flagged player");

    let mut s = Scene::new("cast-heal-karma");
    s.three();
    s.world.set_karma(P2, 10);
    s.world.set_target(ME, Some(P2));
    s.use_skill(ME, HEAL, false);
    s.advance_ms(3_000);
    assert_eq!(s.flag(ME), 1, "healing a player with karma");

    // A good skill on yourself never flags.
    let mut s = Scene::new("cast-heal-self");
    s.three();
    s.world.update_pvp_status(P2);
    s.world.set_target(ME, Some(ME));
    s.use_skill(ME, HEAL, false);
    s.advance_ms(3_000);
    assert_eq!(s.flag(ME), 0);
}

// ------------------------------------------------------------------ karma

/// `onKillUpdatePvPKarma` -> `increasePkKillsAndKarma`: killing a player who is neither flagged
/// nor chaotic gives karma (`Formulas.calculateKarmaGain`, before the PK count rises) and a PK;
/// the killer gets the karma `StatusUpdate` and a `UserInfo`, everybody around a relation.
#[test]
fn a_pk_kill_gives_karma_and_a_pk() {
    let mut s = Scene::new("pk");
    s.three();
    s.kill(P2, ME);
    assert_eq!(
        (s.data(ME).karma, s.data(ME).pk_kills, s.data(ME).pvp_kills),
        (720, 1, 0)
    );
    let sent = s.log.take();
    assert_eq!(karma_updates(&sent, ME.0), [720]);
    let me = names(&sent, ME.0);
    let karma_at = me.iter().position(|n| n == "STATUS_UPDATE").unwrap();
    let info_at = me.iter().rposition(|n| n == "USER_INFO").unwrap();
    assert!(
        karma_at < info_at,
        "the karma first, the UserInfo of increasePkKillsAndKarma last: {me:?}"
    );
    let user_info: UserInfo = decode(of(&sent, ME.0, "USER_INFO").last().unwrap());
    assert_eq!((user_info.karma, user_info.pk_kills), (720, 1));
    // The others see a chaotic player they may attack at will.
    let seen = relations_of(&sent, P3.0, ME.0);
    assert_eq!(seen.last().unwrap().karma, 720);
    assert!(seen.last().unwrap().auto_attackable);

    // The second one: `(1 * 0.5 + 1) * 60 * 12` = 1080 on top.
    s.kill(P3, ME);
    assert_eq!((s.data(ME).karma, s.data(ME).pk_kills), (720 + 1080, 2));
    assert_eq!(calculate_karma_gain(1, false), 1080);
}

/// Killing a flagged player is a PvP kill: no karma, no PK.
#[test]
fn a_pvp_kill_counts_as_pvp() {
    let mut s = Scene::new("pvp-kill");
    s.three();
    s.world.update_pvp_status(P2);
    s.log.take();
    s.kill(P2, ME);
    assert_eq!(
        (s.data(ME).karma, s.data(ME).pk_kills, s.data(ME).pvp_kills),
        (0, 0, 1)
    );
    let sent = s.log.take();
    assert!(karma_updates(&sent, ME.0).is_empty());
    assert!(names(&sent, ME.0).contains(&"USER_INFO".to_string()));
}

/// A target with karma is no reward unless `AwardPKKillPVPPoint`; a kill in a PvP zone counts
/// for nothing.
#[test]
fn killing_a_chaotic_player_or_in_an_arena() {
    for (award, pvp_kills) in [(false, 0), (true, 1)] {
        let mut s = Scene::with_pvp(
            "chaotic",
            PvpSettings {
                karma_award_pk_kill: award,
                ..PvpSettings::default()
            },
        );
        s.three();
        s.world.set_karma(P2, 50);
        s.kill(P2, ME);
        assert_eq!(
            (s.data(ME).karma, s.data(ME).pk_kills, s.data(ME).pvp_kills),
            (0, 0, pvp_kills)
        );
    }
    let mut s = Scene::with_zone("arena-kill", "ArenaZone");
    s.three();
    s.kill(P2, ME);
    assert_eq!(
        (s.data(ME).karma, s.data(ME).pk_kills, s.data(ME).pvp_kills),
        (0, 0, 0)
    );
}

/// A monster, or yourself, as the "killer" changes nothing.
#[test]
fn only_a_player_killer_counts() {
    let mut s = Scene::new("monster-kill");
    s.three();
    let monster = s.world.spawn_npc_at(9001, X + 30, Y + 30, Z, 0).unwrap();
    s.kill(P2, monster);
    s.kill(P3, P3);
    assert_eq!(s.data(ME).karma, 0);
    assert_eq!(
        (s.data(P3).karma, s.data(P3).pk_kills, s.data(P3).pvp_kills),
        (0, 0, 0)
    );
}

/// `Player.setKarma`: when the karma ends, `setKarmaFlag()` runs *before* the value changes, so
/// the `UserInfo` and the first `RelationChanged` still show the old karma; `broadcastKarma()`
/// then sends the `StatusUpdate` and the relation with the new value.
#[test]
fn karma_ending_is_told_in_javas_order() {
    let mut s = Scene::new("karma-end");
    let mut data = player_data(ME, "Chaotic", (X, Y, Z), vec![]);
    data.karma = 50;
    s.enter_data(data);
    s.enter(P3, 80, vec![]);
    s.ticks(20);
    s.log.take();

    s.world.set_karma(ME, 0);
    let sent = s.log.take();
    let me = names(&sent, ME.0);
    assert_eq!(me, ["USER_INFO", "EX_BR_EXTRA_USER_INFO", "STATUS_UPDATE"]);
    let user_info: UserInfo = decode(of(&sent, ME.0, "USER_INFO")[0]);
    assert_eq!(user_info.karma, 50, "NOTE(java-quirk): the old karma");
    assert_eq!(karma_updates(&sent, ME.0), [0]);
    let seen: Vec<(i32, bool)> = relations_of(&sent, P3.0, ME.0)
        .iter()
        .map(|e| (e.karma, e.auto_attackable))
        .collect();
    assert_eq!(seen, [(50, true), (0, false)]);

    // A negative value is 0.
    s.world.set_karma(ME, -5);
    assert_eq!(s.data(ME).karma, 0);
}

// ------------------------------------------------------------------ relations and auto attack

/// `Player.isAutoAttackable(attacker)`: a monster always, another player only against karma or a
/// PvP flag, never in a peace zone, always when both stand in an arena; the invisible GM and the
/// player itself never.
#[test]
fn is_auto_attackable_follows_the_player_rules() {
    let mut s = Scene::new("auto");
    s.three();
    let monster = s.world.spawn_npc_at(9001, X + 30, Y + 30, Z, 0).unwrap();
    let folk = s.world.spawn_npc_at(9003, X + 30, Y + 40, Z, 0).unwrap();
    let guard = s.world.spawn_npc_at(9004, X + 30, Y + 50, Z, 0).unwrap();

    assert!(s.world.is_auto_attackable(P2, monster));
    assert!(!s.world.is_auto_attackable(P2, ME));
    assert!(!s.world.is_auto_attackable(P2, P2));
    assert!(!s.world.is_auto_attackable_by(P2, ME));
    assert!(
        !s.world.is_auto_attackable(P2, folk),
        "a folk NPC with a peaceful player"
    );
    assert!(!s.world.is_auto_attackable(P2, guard));

    s.world.update_pvp_status(P2);
    assert!(s.world.is_auto_attackable(P2, ME), "flagged");
    assert!(s.world.is_auto_attackable_by(P2, ME));
    // NOTE(java-quirk): any other creature falls through to the same karma/flag test.
    assert!(s.world.is_auto_attackable(P2, folk));
    // Guards attack only players with karma.
    assert!(!s.world.is_auto_attackable(P2, guard));
    s.world.set_karma(P2, 1);
    assert!(s.world.is_auto_attackable(P2, guard));
    s.world.set_karma(P2, 0);

    s.world.set_karma(P3, 100);
    assert!(s.world.is_auto_attackable(P3, ME), "karma");

    // The zones.
    let mut z = Scene::with_zone("auto-peace", "PeaceZone");
    z.three();
    z.world.set_karma(P2, 100);
    assert!(!z.world.is_auto_attackable(P2, ME), "peace zone");
    let mut a = Scene::with_zone("auto-arena", "ArenaZone");
    a.three();
    assert!(a.world.is_auto_attackable(P2, ME), "both in an arena");
}

/// A player with karma that comes into view arrives with a `RelationChanged` that carries the
/// karma and says it may be attacked (the M3 stub sent a mask of 0 and `false`).
#[test]
fn a_chaotic_player_in_view_comes_with_its_relation() {
    let mut s = Scene::new("enter-relation");
    s.enter(ME, 0, vec![]);
    s.ticks(20);
    s.log.take();
    let mut data = player_data(P2, "Chaotic", (X + 40, Y, Z), vec![]);
    data.karma = 7;
    s.enter_data(data);
    let entries = relations_of(&s.log.take(), ME.0, P2.0);
    assert_eq!(
        entries,
        [RelationEntry {
            object_id: P2.0,
            relation: 0,
            auto_attackable: true,
            karma: 7,
            pvp_flag: 0,
        }]
    );
}

// ------------------------------------------------------------------ title colours

/// `Custom/PvpTitleColor.ini`: the PvP kill that reaches a tier gives the title and the colour,
/// told to the player and (as `NicknameChanged`) to those who see it; it is stored.
#[test]
fn pvp_kills_give_a_title_and_a_colour() {
    let settings = PvpSettings {
        title_color: PvpTitleColorSettings {
            enabled: true,
            amounts: [500, 1000, 1500, 2500, 5000],
            colors: [0x9C9C9C, 0x696969, 0x4F4F4F, 0x363636, 0x1C1C1C],
            titles: ["Sergeant", "Lieutenant", "Captain", "Major", "General"].map(String::from),
        },
        ..PvpSettings::default()
    };
    let mut s = Scene::with_pvp("title", settings);
    let mut data = player_data(ME, "Killer", (X, Y, Z), sword());
    data.pvp_kills = 499;
    s.enter_data(data);
    s.enter(P2, 40, vec![]);
    s.enter(P3, 80, vec![]);
    s.ticks(20);
    s.log.take();
    s.world.update_pvp_status(P2);
    s.log.take();

    s.kill(P2, ME);
    assert_eq!(s.data(ME).pvp_kills, 500);
    assert_eq!(s.data(ME).title, "\u{AE} Sergeant \u{AE}");
    assert_eq!(s.data(ME).profile.title_color, 0x9C9C9C);
    let sent = s.log.take();
    let user_info: UserInfo = decode(of(&sent, ME.0, "USER_INFO")[0]);
    assert_eq!(user_info.title, "\u{AE} Sergeant \u{AE}");
    assert_eq!(user_info.title_color, 0x9C9C9C);
    for viewer in [ME, P3] {
        let changed: NicknameChanged = decode(of(&sent, viewer.0, "NICK_NAME_CHANGED")[0]);
        assert_eq!(changed.object_id, ME.0);
        assert_eq!(changed.title, "\u{AE} Sergeant \u{AE}");
    }

    // Stored with the character (the title colour is the profile's).
    s.input(ME, ClientInput::Logout);
    let stored = s.persisted.last_store();
    assert_eq!(stored.title, "\u{AE} Sergeant \u{AE}");
    assert_eq!(stored.profile.title_color, 0x9C9C9C);
}

/// The title is also set when a character with enough kills enters (`EnterWorld`), but not for
/// a GM, and not when the system is off.
#[test]
fn the_title_is_applied_at_login_for_players_only() {
    let settings = PvpSettings {
        title_color: PvpTitleColorSettings {
            enabled: true,
            amounts: [10, 20, 30, 40, 50],
            colors: [1, 2, 3, 4, 5],
            titles: ["A", "B", "C", "D", "E"].map(String::from),
        },
        ..PvpSettings::default()
    };
    let mut s = Scene::with_pvp("title-login", settings);
    let mut player = player_data(ME, "Killer", (X, Y, Z), vec![]);
    player.pvp_kills = 25;
    s.enter_data(player);
    let mut gm = player_data(P2, "Gm", (X + 40, Y, Z), vec![]);
    gm.pvp_kills = 25;
    gm.access_level = 100;
    s.enter_data(gm);
    let mut few = player_data(P3, "Few", (X + 80, Y, Z), vec![]);
    few.pvp_kills = 9;
    s.enter_data(few);
    assert_eq!(s.data(ME).title, "\u{AE} B \u{AE}");
    assert_eq!(s.data(P2).title, "");
    assert_eq!(s.data(P3).title, "");

    let mut off = Scene::new("title-off");
    let mut player = player_data(ME, "Killer", (X, Y, Z), vec![]);
    player.pvp_kills = 100_000;
    off.enter_data(player);
    assert_eq!(off.data(ME).title, "");
}

// ------------------------------------------------------------------ storing

/// Karma, PK and PvP kills are the columns of the `characters` row: they are in the snapshot the
/// persistence task writes.
#[test]
fn karma_and_the_kill_counts_are_stored() {
    let mut s = Scene::new("store");
    s.three();
    // NOTE: once the killer has karma `checkIfPvP` is false, so the PvP kill comes first.
    s.world.update_pvp_status(P3);
    s.kill(P3, ME);
    s.kill(P2, ME);
    s.input(ME, ClientInput::Logout);
    let stored = s.persisted.last_store();
    assert_eq!((stored.karma, stored.pk_kills, stored.pvp_kills), (720, 1, 1));
}
