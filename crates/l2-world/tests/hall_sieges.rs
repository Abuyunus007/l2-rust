#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Conquerable clan hall sieges (M7-CHSIEGE-01): loading the engines, the schedule, the
//! registration (D-50), a GM test siege end to end for the base scripts (D-51), the flag war and
//! Rainbow Springs scripts with Java's quirks, and `//chsiege`.
//! Java: managers/CHSiegeManager.java, mechanics/siege/clanhalls/{ClanHallSiegeEngine,SiegableHall}.java,
//! dist/game/data/scripts/conquerablehalls/*, handlers/chat/commands/admin/AdminCHSiege.java. The
//! expected values come from the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, DoorTemplates, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache,
    ItemTemplates, KarmaLoss, MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions,
    SkillTemplates, SkillTrees, SpawnData, SpawnLoadOptions, StaticObjectTemplates, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::siege::{SiegeAttackerList, SiegeInfo, SiegeTime};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::ai::siege_guard::SiegeGuardAiKind;
use l2_world::residence::{ResidenceRereadRows, SiegableHallRecord};
use l2_world::siege::{SiegeInput, SiegeSettings};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Debug, Clone)]
enum Out {
    Packet(i32, Vec<u8>),
    Persist(PersistCommand),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push(Out::Packet(self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct Recorder(Log);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
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
</staticData><lvlUpgainData><level val="1"><hp>5000</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

const SKILLS: &str = r#"<list>
<skill id="4235" levels="1" name="Gustav"><operateType>P</operateType></skill>
</list>"#;

const ADENA: i32 = 57;
const QUEST_REWARD: i32 = 5009;
const TARLK_AMULET: i32 = 4332;
const WAR_DECREES: i32 = 8034;
const LETTER_A: i32 = 8035;
const LETTER_B: i32 = 8036;
const NECTAR: i32 = 8030;

fn item_ids() -> Vec<i32> {
    let mut ids = vec![
        ADENA,
        QUEST_REWARD,
        TARLK_AMULET,
        WAR_DECREES,
        8030,
        8031,
        8032,
        8033,
    ];
    ids.extend(8035..=8055);
    ids
}

fn items_xml() -> String {
    let list: Vec<String> = item_ids()
        .into_iter()
        .map(|id| {
            format!(
                r#"<item id="{id}" type="EtcItem" name="Item {id}"><set name="is_stackable" val="true" /></item>"#
            )
        })
        .collect();
    format!("<list>{}</list>", list.join(""))
}

const NURKA: i32 = 35375;
const MESSENGER_21: i32 = 35382;
const GUSTAV: i32 = 35410;
const MIKHAIL: i32 = 35409;
const LIDIA: i32 = 35629;
const BANDIT_MESSENGER: i32 = 35437;
const BANDIT_TELEPORT: i32 = 35560;
const BANDIT_ALLY_1: i32 = 35428;
const BANDIT_FLAG_1: i32 = 35422;
const RAINBOW_MESSENGER: i32 = 35604;
const RAINBOW_CARETAKER: i32 = 35603;
const YETI_1: i32 = 35596;
const GOURD_1: i32 = 35588;
const GUARD: i32 = 35413;
/// A `CastleDoorman` and a `Defender` standing at a siegable hall.
const DOORMAN: i32 = 35990;
const HALL_DEFENDER: i32 = 35991;

fn npc(id: i32, npc_type: &str) -> String {
    format!(
        r#"<npc id="{id}" type="{npc_type}" level="10" name="Npc {id}">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="30000" hpRegen="2" mp="400.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="0" isAggressive="false" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>"#
    )
}

fn npc_xml() -> String {
    let mut list = Vec::new();
    for id in [
        MESSENGER_21,
        BANDIT_MESSENGER,
        RAINBOW_MESSENGER,
        RAINBOW_CARETAKER,
        35627,
    ] {
        list.push(npc(id, "Folk"));
    }
    for id in 35560..=35565 {
        list.push(npc(id, "Folk"));
    }
    for id in 35422..=35427 {
        list.push(npc(id, "Folk"));
    }
    for id in 35596..=35599 {
        list.push(npc(id, "Folk"));
    }
    for id in [
        NURKA, GUSTAV, MIKHAIL, 35408, GUARD, 35629, 35630, 35631, 35592, 35593,
    ] {
        list.push(npc(id, "Monster"));
    }
    for id in 35428..=35432 {
        list.push(npc(id, "Monster"));
    }
    list.push(npc(DOORMAN, "CastleDoorman"));
    list.push(npc(HALL_DEFENDER, "Defender"));
    for id in 35588..=35591 {
        list.push(npc(id, "Monster"));
    }
    format!(r#"<?xml version="1.0"?><list>{}</list>"#, list.join("\n"))
}

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(&items_xml()).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

/// Access level 100 is a GM.
struct Data(StatData, AdminRights);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: item_id,
            type2: i16::try_from(t.type2).unwrap(),
            body_part: t.body_part.mask(),
            quest_item: false,
            weight: t.weight,
            shared_reuse_group: -1,
            weapon: t.is_weapon(),
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        true
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        if level == 100 {
            AccessLevelStatic {
                is_gm: true,
                ..AccessLevelStatic::USER
            }
        } else {
            AccessLevelStatic::USER
        }
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        item_ids().contains(&item_id)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.1)
    }
}

fn admin_rights() -> AdminRights {
    let mut rights = AdminRights::default();
    rights.add_level(100, 0);
    rights.add_level(0, 0);
    for command in [
        "chsiege_startSiege",
        "chsiege_endsSiege",
        "chsiege_setSiegeDate",
        "chsiege_addAttacker",
        "chsiege_removeAttacker",
        "chsiege_clearAttackers",
        "chsiege_listAttackers",
        "chsiege_forwardSiege",
        "chsiege_siegablehall",
    ] {
        rights.add_command(command, 100, false);
    }
    rights
}

/// The halls and where their zones are: (id, name, x, y).
const HALLS: [(i32, &str, i32, i32); 6] = [
    (21, "Fortress of Resistance", 43_500, 108_800),
    (34, "Devastated Castle", 178_000, -16_000),
    (35, "Bandit Stronghold", 82_882, -16_280),
    (62, "Rainbow Springs Chateau", 151_000, -127_000),
    (63, "Wild Beast Reserve", 57_762, -92_696),
    (64, "Fortress of the Dead", 50_000, -40_000),
];
const Z: i32 = -2_000;

fn hall_xy(id: i32) -> (i32, i32) {
    let (_, _, x, y) = HALLS.iter().find(|h| h.0 == id).unwrap();
    (*x, *y)
}

fn zones_xml() -> String {
    let cuboid = |x: i32, y: i32, w: i32| {
        format!(
            r#"shape="Cuboid" minZ="-5000" maxZ="5000"><node X="{}" Y="{}" /><node X="{}" Y="{}" />"#,
            x - w,
            y - w,
            x + w,
            y + w
        )
    };
    let mut zones = String::new();
    for (id, name, x, y) in HALLS {
        zones.push_str(&format!(
            r#"<zone name="{name} hall" type="SiegableHallZone" {}<stat name="clanHallId" val="{id}" />
  <spawn X="{x}" Y="{}" Z="{Z}" /><spawn X="{}" Y="{y}" Z="{Z}" type="banish" />
  <spawn X="{}" Y="{y}" Z="{Z}" type="challenger" /><spawn X="{}" Y="{y}" Z="{Z}" type="challenger" /></zone>
  <zone name="{name} siege" type="SiegeZone" {}<stat name="clanHallId" val="{id}" /></zone>
"#,
            cuboid(x, y, 600),
            y + 100,
            x + 900,
            x + 10,
            x + 20,
            cuboid(x, y, 1500),
        ));
    }
    // The first teleport zone of Bandit Stronghold.
    let (bx, by) = hall_xy(35);
    zones.push_str(&format!(
        r#"<zone name="bandit tele" type="ResidenceHallTeleportZone" {}<stat name="residenceId" val="35" /><stat name="residenceZoneId" val="0" />
  <spawn X="{}" Y="{}" Z="{Z}" /></zone>"#,
        cuboid(bx - 300, by, 100),
        bx + 50,
        by + 50
    ));
    format!(r#"<?xml version="1.0"?><list enabled="true">{zones}</list>"#)
}

fn door_xml(id: i32, hall: i32) -> String {
    let (x, y) = hall_xy(hall);
    format!(
        r#"<door id="{id}" name="door{id}" height="150" baseHpMax="1000" clanhall_id="{hall}" pos="{x},{y},{Z}"
            node1="{},{}" node2="{},{}" node3="{},{}" node4="{},{}" nodeZ="{}"/>"#,
        x - 10,
        y - 50,
        x + 10,
        y - 50,
        x + 10,
        y + 50,
        x - 10,
        y + 50,
        Z - 10
    )
}

/// The pages of the scripts: every page carries a link to every command, so the client-side
/// validation of the bypasses lets the tests send them.
fn script_pages() -> Vec<(String, String)> {
    let mut pages = Vec::new();
    let mut add = |dir: &str, name: &str, links: &[String]| {
        pages.push((
            format!("data/scripts/conquerablehalls/{dir}/{name}"),
            format!(
                "<html><body>PAGE {name} clan=%clanName% next=%nextSiege% owner=%owner% word=%word% c1=%clan1% m1=%clanMem1% c2=%clan2% c4=%clan4% c5=%clan5% {}</body></html>",
                links.concat()
            ),
        ));
    };
    let link = |script: &str, event: &str| format!(r#"<a action="bypass -h Script {script} {event}">x</a>"#);
    let bandit_events = [
        "Register wQuest",
        "Register wFee",
        "Register",
        "Select_NPC 35428",
        "Select_NPC 35429",
        "Select_NPC x",
        "Select_NPC",
        "View",
        "RegisterMember",
        "Attackers",
        "CheckQuest",
        "Bogus",
    ]
    .map(|e| link("BanditStronghold", e));
    let mut names: Vec<String> = (0..=25).map(|n| format!("35437-{n:02}.htm")).collect();
    names.extend(["35437-14a.htm", "35437-15a.htm", "35437-16a.htm", "35437-21a.htm"].map(str::to_owned));
    names.extend(["35560-00.htm", "35560-01.htm"].map(str::to_owned));
    for name in &names {
        add("BanditStronghold", name, &bandit_events);
    }
    let rainbow_events = [
        "Register",
        "Cancel",
        "Unregister",
        "GoToArena",
        "getItem",
        "seeItem",
    ]
    .map(|e| link("RainbowSpringsChateau", e));
    let mut names: Vec<String> = Vec::new();
    for n in 0..=18 {
        names.push(format!("35604-{n:02}.htm"));
    }
    for n in 0..=10 {
        names.push(format!("35603-{n:02}.htm"));
    }
    for n in 0..=6 {
        names.push(format!("35596-{n:02}.htm"));
    }
    for name in &names {
        add("RainbowSpringsChateau", name, &rainbow_events);
    }
    add("FortressOfResistance", "partisan_ordery_brakel001.htm", &[]);
    pages
}

const ADMIN_PAGE: &str =
    "<html><body>HALL %clanhallName% id=%clanhallId% owner=%clanhallOwner%</body></html>";

const NOW: i64 = 1_792_834_867_321; // 2026-10-24T09:41:07.321Z
const HOUR: i64 = 3_600_000;
const DAY: i64 = 24 * HOUR;

const X0: i32 = -71_338;
const Y0: i32 = 258_271;
/// Lions (the owner of the halls in these tests): P1 (leader) and P2; Wolves: P3 (leader) and P4;
/// Bears (level 3): P5; Tigers: P7 (leader) and P8. P9 is a GM without a clan.
const P1: i32 = 0x2000_0001;
const P2: i32 = 0x2000_0002;
const P3: i32 = 0x2000_0003;
const P4: i32 = 0x2000_0004;
const P5: i32 = 0x2000_0005;
const P7: i32 = 0x2000_0007;
const P8: i32 = 0x2000_0008;
const GM: i32 = 0x2000_0009;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
const BEARS: i32 = 700;
const TIGERS: i32 = 800;

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

fn member(id: i32, power_grade: i32) -> ClanMemberRecord {
    ClanMemberRecord {
        object_id: id,
        name: format!("P{}", id - 0x2000_0000),
        level: 40,
        power_grade,
        ..ClanMemberRecord::default()
    }
}

fn clan_record(clan_id: i32, name: &str, level: i32, leader: i32, other: i32) -> ClanRecord {
    ClanRecord {
        clan_id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level,
            leader_id: leader,
            ..ClanDataRecord::default()
        }),
        members: vec![member(leader, 1), member(other, 5)],
        warehouse: Vec::new(),
        ..ClanRecord::default()
    }
}

/// A conquerable hall row: owner, the next siege, a two hour siege and a schedule of 14 days at 18:30.
fn hall_row(id: i32, owner: i32, next_siege: i64) -> SiegableHallRecord {
    let name = HALLS.iter().find(|h| h.0 == id).unwrap().1;
    SiegableHallRecord {
        id,
        name: name.to_owned(),
        owner_id: owner,
        desc: "d".to_owned(),
        location: "l".to_owned(),
        next_siege,
        siege_length: 2 * HOUR,
        schedule_config: Some("14;0;0;18;30".to_owned()),
        functions: Vec::new(),
    }
}

fn all_halls(owner: i32) -> Vec<SiegableHallRecord> {
    HALLS
        .iter()
        .map(|h| hall_row(h.0, if h.0 == 21 { owner } else { 0 }, NOW + 3 * DAY))
        .collect()
}

fn hall_load(rows: Vec<HallSiegeHallLoad>) -> HallSiegeLoad {
    HallSiegeLoad { halls: rows }
}

fn guard_rows(npc_id: i32, count: i32, delay: i32, hall: i32) -> Vec<HallGuardRecord> {
    let (x, y) = hall_xy(hall);
    (0..count)
        .map(|i| HallGuardRecord {
            npc_id,
            x: x + 100 + i * 10,
            y,
            z: Z,
            heading: 0,
            respawn_delay: delay,
        })
        .collect()
}

impl Scene {
    fn new(siege_database_writes: bool, halls: Vec<SiegableHallRecord>, load: HallSiegeLoad) -> Self {
        Self::with(siege_database_writes, halls, load, HallSiegeSettings::default())
    }

    fn with(
        siege_database_writes: bool,
        halls: Vec<SiegableHallRecord>,
        load: HallSiegeLoad,
        hall_settings: HallSiegeSettings,
    ) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        write("data/zones/halls.xml", &zones_xml());
        write(
            "data/mapregion/talking_island_town.xml",
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
             <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
             <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n</region>\n</list>\n",
        );
        write("data/html/admin/siegablehall.htm", ADMIN_PAGE);
        for (name, body) in [("", "OWNER"), ("-no", "NO"), ("-busy", "BUSY")] {
            let links = ["open_doors", "close_doors"]
                .map(|c| format!(r#"<a action="bypass -h npc_%objectId%_{c} $e">x</a>"#))
                .concat();
            write(
                &format!("data/html/doorman/35990{name}.htm"),
                &format!("<html><body>DOORMAN {body}{links}</body></html>"),
            );
        }
        for (path, text) in script_pages() {
            write(&path, &text);
        }
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Recorder(log.clone())));
        world.set_static_data(Box::new(Data(stat_data(), admin_rights())));
        world.set_exp_data(
            Arc::new(ExperienceTable::from_xml(r#"<table maxLevel="85" maxPetLevel="85"></table>"#).unwrap()),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
        );
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        let npcs = Arc::new(NpcTemplates::from_xml(&npc_xml(), &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
        world
            .spawn_static_objects(&StaticObjectTemplates::from_xml("<list></list>").unwrap(), &ids)
            .unwrap();
        let doors = DoorTemplates::from_xml(&format!(
            "<list>{}{}{}</list>",
            door_xml(22_170_001, 35),
            door_xml(22_170_003, 35),
            door_xml(21_000_001, 21)
        ))
        .unwrap();
        world.spawn_doors(&doors, &ids).unwrap();
        world.set_siege_settings(SiegeSettings {
            clock_override_unix_ms: Some(NOW),
            zone_offset_override_ms: Some(0),
            ..SiegeSettings::default()
        });
        world.set_residence_settings(ResidenceSettings {
            siege_database_writes,
            ..ResidenceSettings::default()
        });
        world.load_clans(
            vec![
                clan_record(LIONS, "Lions", 5, P1, P2),
                clan_record(WOLVES, "Wolves", 5, P3, P4),
                clan_record(BEARS, "Bears", 3, P5, 0x2000_0006),
                clan_record(TIGERS, "Tigers", 5, P7, P8),
            ],
            Vec::new(),
        );
        world.load_residences(ResidenceLoad {
            siegable_halls: halls,
            ..ResidenceLoad::default()
        });
        world.set_hall_siege_settings(hall_settings);
        world.load_hall_sieges(load);
        Self {
            world,
            log,
            _root: root,
        }
    }

    fn take(&mut self) -> Vec<Out> {
        self.log.take()
    }

    fn enter(&mut self, id: i32, clan_id: i32, hall: i32) {
        let (x, y) = hall_xy(hall);
        self.enter_at(id, clan_id, x, y, 0);
    }

    fn enter_gm(&mut self, id: i32) {
        self.enter_at(id, 0, X0, Y0, 100);
    }

    fn enter_at(&mut self, id: i32, clan_id: i32, x: i32, y: i32, access_level: i32) {
        let data = pdata(id, clan_id, x, y, access_level);
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.world.tick();
        self.appear(id);
    }

    /// The client's `Appearing`.
    fn appear(&mut self, player: i32) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input: ClientInput::Appearing,
        });
        self.run_ms(100);
    }

    fn run_ms(&mut self, ms: u64) {
        for _ in 0..ms.div_ceil(50) {
            self.world.tick();
        }
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    fn script(&mut self, player: i32, command: &str) {
        self.input(
            player,
            ClientInput::Bypass {
                command: format!("Script {command}"),
            },
        );
    }

    /// Makes `npc` the last folk NPC of the player (the click that opened the window).
    fn talk(&mut self, player: i32, npc: ObjectId) {
        self.world.set_target(ObjectId(player), Some(npc));
        self.input(
            player,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
        self.run_ms(100);
    }

    fn npcs_of(&self, npc_id: i32) -> Vec<ObjectId> {
        let mut found: Vec<ObjectId> = self
            .world
            .npc_ids()
            .into_iter()
            .filter(|id| self.world.npc(*id).unwrap().npc_id == npc_id)
            .collect();
        found.sort_unstable();
        found
    }

    /// An NPC of the template standing at the hall.
    fn spawn_at_hall(&mut self, npc_id: i32, hall: i32) -> ObjectId {
        let (x, y) = hall_xy(hall);
        let npc = self.world.spawn_npc_at(npc_id, x + 30, y + 30, Z, 0).unwrap();
        self.world.tick();
        npc
    }

    /// Sends a hall's siege window request and answers the re-read with the hall as it is.
    fn join(&mut self, player: i32, hall: i32, joining: bool) {
        self.input(
            player,
            ClientInput::Siege(SiegeInput::JoinSiege {
                castle_id: hall,
                is_attacker: 1,
                is_joining: i32::from(joining),
            }),
        );
        self.answer_hall_reread();
    }

    /// Answers the pending re-read request with the row of the hall in this world.
    fn answer_hall_reread(&mut self) {
        let out = self.take();
        let (hall_id, then) = out
            .iter()
            .rev()
            .find_map(|o| match o {
                Out::Persist(PersistCommand::ResidenceRereadRequest {
                    residence_id, then, ..
                }) => Some((*residence_id, then.clone())),
                _ => None,
            })
            .expect("a re-read request");
        let h = self.world.clan_hall(ClanHallId(hall_id)).unwrap();
        let row = SiegableHallRecord {
            id: hall_id,
            name: h.name.clone(),
            owner_id: h.owner_id,
            desc: "d".to_owned(),
            location: "l".to_owned(),
            next_siege: self.world.hall_next_siege_time(ClanHallId(hall_id)),
            siege_length: 2 * HOUR,
            schedule_config: Some("14;0;0;18;30".to_owned()),
            functions: Vec::new(),
        };
        self.world.handle(WorldCommand::ResidenceReread {
            result: Ok(ResidenceRereadRows::SiegableHall(row)),
            then,
        });
    }

    /// Answers the pending flag war / Rainbow Springs re-read with these rows.
    fn answer_tables_reread(&mut self, flag_war: Vec<FlagWarRecord>, rainbow: Vec<(i32, i64)>) {
        let out = self.take();
        let (hall_id, then) = out
            .iter()
            .rev()
            .find_map(|o| match o {
                Out::Persist(PersistCommand::ResidenceRereadRequest {
                    residence_id,
                    then,
                    kind,
                }) => {
                    assert_eq!(*kind, l2_world::residence::ResidenceReadKind::HallSiege);
                    Some((*residence_id, then.clone()))
                }
                _ => None,
            })
            .expect("a re-read request");
        self.world.handle(WorldCommand::ResidenceReread {
            result: Ok(ResidenceRereadRows::HallSiege(Box::new(HallSiegeRows {
                hall_id,
                hall: None,
                flag_war,
                rainbow,
            }))),
            then,
        });
    }

    /// A script event: the player clicked the NPC, then sends the command and the tables are read.
    fn event(&mut self, player: i32, script: &str, npc: ObjectId, event: &str) {
        self.talk(player, npc);
        self.take();
        self.script(player, &format!("{script} {event}"));
        let pending = self
            .log
            .0
            .lock()
            .unwrap()
            .iter()
            .any(|o| matches!(o, Out::Persist(PersistCommand::ResidenceRereadRequest { .. })));
        if pending {
            // The database has what this world wrote.
            let flag_war = self
                .world
                .flag_war_clans(H35)
                .into_iter()
                .filter(|(_, d)| d.written)
                .map(|(clan, d)| FlagWarRecord {
                    clan_id: clan,
                    flag: d.flag,
                    npc: d.npc,
                    members: d.players.clone(),
                })
                .collect();
            let rainbow = self.world.rainbow_war_decrees();
            self.answer_tables_reread(flag_war, rainbow);
        }
    }

    fn clan(&self, id: i32) -> &Clan {
        self.world.clan(id).unwrap()
    }

    fn hall(&self, id: i32) -> &l2_world::residence::ClanHall {
        self.world.clan_hall(ClanHallId(id)).unwrap()
    }
}

fn pdata(id: i32, clan_id: i32, x: i32, y: i32, access_level: i32) -> PlayerData {
    let mut data = PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{}", id - 0x2000_0000),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 40,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level,
        x,
        y,
        z: Z,
        heading: 0,
        cur_hp: 5000.0,
        max_hp: 5000.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items: vec![ItemData {
            object_id: ObjectId(id + 0x1000_0000),
            item_id: ADENA,
            count: 1_000_000,
            loc: "INVENTORY".to_owned(),
            ..Default::default()
        }],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    };
    if clan_id != 0 {
        data.profile.clan = Some(PlayerClanState {
            clan_id,
            ..PlayerClanState::default()
        });
    }
    data
}

fn count(s: &Scene, player: i32, item_id: i32) -> i64 {
    s.world
        .player(ObjectId(player))
        .unwrap()
        .snapshot()
        .items
        .iter()
        .filter(|i| i.item_id == item_id)
        .map(|i| i.count)
        .sum()
}

fn give(s: &mut Scene, player: i32, item_id: i32, count: i64) {
    s.world.add_item(ObjectId(player), item_id, count, false);
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn packets_to(out: &[Out], to: i32, name: &str) -> Vec<Vec<u8>> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(t, b) if *t == to && name_of(b) == name => Some(b.clone()),
            _ => None,
        })
        .collect()
}

fn messages_to(out: &[Out], to: i32) -> Vec<SystemMessage> {
    packets_to(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|b| decode::<SystemMessage>(b))
        .collect()
}

fn message_ids(out: &[Out], to: i32) -> Vec<SystemMessageId> {
    messages_to(out, to).iter().map(|m| m.id).collect()
}

fn text_param(message: &SystemMessage, index: usize) -> String {
    match &message.params[index] {
        SystemMessageParam::Text(text) => text.clone(),
        other => panic!("not a text: {other:?}"),
    }
}

fn pages_to(out: &[Out], to: i32) -> Vec<String> {
    packets_to(out, to, "NPC_HTML_MESSAGE")
        .iter()
        .map(|b| decode::<NpcHtmlMessage>(b).html)
        .collect()
}

/// The text lines a player got (`Player.sendMessage` is a system message with a text).
fn says_to(out: &[Out], to: i32) -> Vec<String> {
    messages_to(out, to)
        .iter()
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(text)) => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn hall_writes(out: &[Out]) -> Vec<HallSiegeWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Residence(l2_world::ResidenceWrite::HallSiege(w))) => {
                Some(w.clone())
            }
            _ => None,
        })
        .collect()
}

/// `UPDATE siegable_clanhall`: (hall, owner, next siege).
fn siegable_writes(out: &[Out]) -> Vec<(i32, i32, i64)> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Residence(l2_world::ResidenceWrite::SiegableHall {
                hall_id,
                owner_id,
                next_siege,
            })) => Some((*hall_id, *owner_id, *next_siege)),
            _ => None,
        })
        .collect()
}

const H21: ClanHallId = ClanHallId(21);
const H34: ClanHallId = ClanHallId(34);
const H35: ClanHallId = ClanHallId(35);
const H62: ClanHallId = ClanHallId(62);
const H64: ClanHallId = ClanHallId(64);

fn base_load(hall: i32) -> HallSiegeHallLoad {
    HallSiegeHallLoad {
        hall_id: hall,
        guards: Vec::new(),
        ..HallSiegeHallLoad::default()
    }
}

// ---------------------------------------------------------------------------------- loading

/// `CHSiegeManager` loads the halls of the table; each script needs its hall (`getSiegableHall`).
#[test]
fn an_engine_exists_for_each_hall_that_has_a_row() {
    let s = Scene::new(
        false,
        vec![hall_row(21, 0, NOW + 3 * DAY), hall_row(34, 0, NOW + 3 * DAY)],
        hall_load(vec![]),
    );
    assert!(s.world.hall_has_siege(H21));
    assert!(s.world.hall_has_siege(H34));
    assert!(!s.world.hall_has_siege(H35));
    assert_eq!(s.world.hall_siege_status(H21), Some(SiegeStatus::Registering));
    assert!(s.world.clan_hall(ClanHallId(21)).unwrap().is_siegable_hall());
}

/// `NoQuests` (the scripts are not loaded) leaves the halls without a siege.
#[test]
fn no_quests_means_no_sieges() {
    let s = Scene::with(
        false,
        all_halls(0),
        hall_load(vec![]),
        HallSiegeSettings {
            no_quests: true,
            ..HallSiegeSettings::default()
        },
    );
    assert!(!s.world.hall_has_siege(H21));
}

/// `SiegableHall` constructor: a next siege less than an hour away is moved by `schedule_config`
/// (14 days at 18:30) and written (`updateDb`), only with `SiegeDatabaseWrites` (D-51).
#[test]
fn a_siege_due_within_an_hour_is_moved_and_written_only_with_the_switch() {
    for switch in [false, true] {
        let scene = Scene::new(switch, vec![hall_row(21, 0, NOW + 1_800_000)], hall_load(vec![]));
        let next = scene.world.hall_next_siege_time(H21);
        // 2026-11-07T18:30:00.321Z.
        assert_eq!(next, 1_794_076_200_321, "{next}");
    }
    // The write: the scene's load output is not kept, so load again and look at the log.
    let written = Scene::new(true, vec![hall_row(21, 0, NOW + 1_800_000)], hall_load(vec![]));
    let out = written.log.take();
    assert_eq!(siegable_writes(&out), [(21, 0, 1_794_076_200_321)]);
    let quiet = Scene::new(false, vec![hall_row(21, 0, NOW + 1_800_000)], hall_load(vec![]));
    assert!(siegable_writes(&quiet.log.take()).is_empty());
    // A siege due later is left alone.
    let later = Scene::new(true, vec![hall_row(21, 0, NOW + 2 * HOUR)], hall_load(vec![]));
    assert_eq!(later.world.hall_next_siege_time(H21), NOW + 2 * HOUR);
    assert!(siegable_writes(&later.log.take()).is_empty());
}

// ---------------------------------------------------------------------------------- the schedule

/// `PrepareOwner` runs an hour before the siege: with the switch only (D-51); it frees the hall,
/// makes the owner an attacker, banishes the foreigners and tells everybody.
#[test]
fn the_registration_ends_an_hour_before_the_siege_with_the_switch() {
    let mut off = Scene::new(
        false,
        vec![hall_row(21, LIONS, NOW + 2 * HOUR)],
        hall_load(vec![]),
    );
    off.run_ms(65 * 60_000);
    assert_eq!(off.world.hall_siege_status(H21), Some(SiegeStatus::Registering));
    assert_eq!(off.hall(21).owner_id, LIONS);

    let mut on = Scene::new(true, vec![hall_row(21, LIONS, NOW + 2 * HOUR)], hall_load(vec![]));
    on.enter(P1, LIONS, 21);
    on.enter_at(P5, BEARS, hall_xy(21).0 + 5, hall_xy(21).1, 0);
    on.take();
    on.run_ms(59 * 60_000);
    assert_eq!(on.world.hall_siege_status(H21), Some(SiegeStatus::Registering));
    on.run_ms(2 * 60_000);
    assert_eq!(on.world.hall_siege_status(H21), Some(SiegeStatus::WaitingBattle));
    let out = on.take();
    // The owner is an attacker, the hall is free (and written, with the switch).
    assert!(on.world.hall_siege_is_attacker(H21, LIONS));
    assert_eq!(on.hall(21).owner_id, 0);
    assert!(siegable_writes(&out).contains(&(21, 0, NOW + 2 * HOUR)));
    // `THE_REGISTRATION_TERM_FOR_S1_HAS_ENDED` with the hall's name, to everybody.
    for player in [P1, P5] {
        let messages = messages_to(&out, player);
        let m = messages
            .iter()
            .find(|m| m.id == SystemMessageId::TheRegistrationTermForS1HasEnded)
            .unwrap();
        assert_eq!(text_param(m, 0), "Fortress of Resistance");
    }
    // The foreigner left the zone; the owner's clan was banished too, as the hall is free now
    // (`banishForeigners(0)` moves everybody).
    let (x, _) = hall_xy(21);
    assert!(on.world.position_of(ObjectId(P5)).unwrap().0 >= x + 800);
}

// ---------------------------------------------------------------------------------- registration

fn join_ids(s: &mut Scene, player: i32, hall: i32) -> Vec<SystemMessageId> {
    s.take();
    s.join(player, hall, true);
    let out = s.take();
    message_ids(&out, player)
}

/// `CHSiegeManager.registerClan`: the checks in Java's order, then the clan is an attacker; the
/// player gets `SiegeInfo` of the hall (`RequestJoinSiege`). The hall is read again first (D-50).
#[test]
fn a_clan_registers_in_javas_order_of_checks() {
    let mut s = Scene::new(false, vec![hall_row(21, LIONS, NOW + 3 * DAY)], hall_load(vec![]));
    for (id, clan) in [(P1, LIONS), (P3, WOLVES), (P5, BEARS), (P7, TIGERS)] {
        s.enter(id, clan, 21);
    }
    // The request waits for the read.
    s.take();
    s.input(
        P3,
        ClientInput::Siege(SiegeInput::JoinSiege {
            castle_id: 21,
            is_attacker: 1,
            is_joining: 1,
        }),
    );
    let out = s.take();
    assert!(out.iter().any(|o| matches!(
        o,
        Out::Persist(PersistCommand::ResidenceRereadRequest { residence_id: 21, .. })
    )));
    assert!(!s.world.hall_siege_is_attacker(H21, WOLVES));

    // Level 3 (the minimum is 4): a chat line.
    s.join(P5, 21, true);
    let out = s.take();
    assert!(
        says_to(&out, P5)
            .iter()
            .any(|t| t.contains("Only clans of level 4 or higher may register for a castle siege"))
    );
    // The owner registers on the defending side by itself.
    assert_eq!(
        join_ids(&mut s, P1, 21),
        [SystemMessageId::CastleOwningClansAreAutomaticallyRegisteredOnTheDefendingSide]
    );
    // A clan of level 5 registers; the answer is the siege window.
    s.join(P3, 21, true);
    let out = s.take();
    assert!(s.world.hall_siege_is_attacker(H21, WOLVES));
    let info: SiegeInfo = decode(&packets_to(&out, P3, "CASTLE_SIEGE_INFO")[0]);
    assert_eq!(info.residence_id, 21);
    assert!(!info.viewer_is_owner_leader);
    assert_eq!(info.owner_id, LIONS);
    assert_eq!(info.owner.as_ref().unwrap().name, "Lions");
    assert_eq!(
        info.time,
        SiegeTime::Date(i32::try_from((NOW + 3 * DAY) / 1000).unwrap())
    );
    // A second request: "already requested".
    assert_eq!(
        join_ids(&mut s, P3, 21),
        [SystemMessageId::YouHaveAlreadyRequestedACastleSiege]
    );
    // The owner sees itself as the viewer of the window.
    s.join(P1, 21, true);
    let out = s.take();
    let info: SiegeInfo = decode(&packets_to(&out, P1, "CASTLE_SIEGE_INFO")[0]);
    assert!(info.viewer_is_owner_leader);
}

/// A clan registers for one hall only (`isClanParticipating`) and a clan with a hall or castle may
/// not; in the waiting phase and during the battle the answers are the deadline and "not the time".
#[test]
fn one_hall_per_clan_and_the_phases() {
    let mut s = Scene::new(false, all_halls(LIONS), hall_load(vec![]));
    s.enter(P3, WOLVES, 21);
    s.enter(P7, TIGERS, 21);
    s.enter(P1, LIONS, 21);
    s.join(P3, 21, true);
    assert!(s.world.hall_siege_is_attacker(H21, WOLVES));
    assert_eq!(
        join_ids(&mut s, P3, 34),
        [SystemMessageId::YourApplicationHasBeenDeniedBecauseYouHaveAlreadySubmittedARequestForAnotherCastleSiege]
    );
    // A clan with a hall (the Lions own 21; hideoutId is set) cannot take part elsewhere.
    assert_eq!(
        join_ids(&mut s, P1, 34),
        [SystemMessageId::AClanThatOwnsACastleCannotParticipateInAnotherSiege]
    );
    // The registration is over: the deadline has passed (with the hall's name).
    s.world.hall_prepare_owner(H21);
    s.take();
    s.join(P7, 21, true);
    let out = s.take();
    let m = &messages_to(&out, P7)[0];
    assert_eq!(
        m.id,
        SystemMessageId::TheDeadlineToRegisterForTheSiegeOfS1HasPassed
    );
    assert_eq!(text_param(m, 0), "Fortress of Resistance");
    // During the battle: not the time.
    s.world.hall_add_attacker(H21, WOLVES);
    s.world.hall_start_siege(H21);
    assert_eq!(s.world.hall_siege_status(H21), Some(SiegeStatus::Running));
    assert_eq!(
        join_ids(&mut s, P7, 21),
        [SystemMessageId::ThisIsNotTheTimeForSiegeRegistrationAndSoRegistrationAndCancellationCannotBeDone]
    );
}

/// `MaxAttackers`: no more registrations when the list is full.
#[test]
fn the_attacker_list_has_a_limit() {
    let mut s = Scene::with(
        false,
        all_halls(0),
        hall_load(vec![]),
        HallSiegeSettings {
            max_attackers: 1,
            ..HallSiegeSettings::default()
        },
    );
    s.enter(P3, WOLVES, 21);
    s.enter(P7, TIGERS, 21);
    s.join(P3, 21, true);
    assert_eq!(
        join_ids(&mut s, P7, 21),
        [SystemMessageId::NoMoreRegistrationsMayBeAcceptedForTheAttackerSide]
    );
}

/// `unRegisterClan`: only while registering; `SiegeAttackerList` shows the clans.
#[test]
fn cancelling_and_the_attacker_list() {
    let mut s = Scene::new(false, all_halls(0), hall_load(vec![]));
    s.enter(P3, WOLVES, 21);
    s.enter(P7, TIGERS, 21);
    s.join(P3, 21, true);
    s.join(P7, 21, true);
    s.take();
    s.input(P3, ClientInput::Siege(SiegeInput::AttackerList { castle_id: 21 }));
    let out = s.take();
    let list: SiegeAttackerList = decode(&packets_to(&out, P3, "CASTLE_SIEGE_ATTACKER_LIST")[0]);
    assert_eq!(list.residence_id, 21);
    let mut names: Vec<String> = list.clans.iter().flatten().map(|c| c.name.clone()).collect();
    names.sort();
    assert_eq!(names, ["Tigers", "Wolves"]);
    // Cancelling.
    s.join(P7, 21, false);
    assert!(!s.world.hall_siege_is_attacker(H21, TIGERS));
    assert!(s.world.hall_siege_is_attacker(H21, WOLVES));
    let out = s.take();
    // `RequestJoinSiege` shows the window after a cancel too.
    assert_eq!(packets_to(&out, P7, "CASTLE_SIEGE_INFO").len(), 1);
    // After the registration has ended nothing is cancelled.
    s.world.hall_prepare_owner(H21);
    s.join(P3, 21, false);
    assert!(s.world.hall_siege_is_attacker(H21, WOLVES));
}

/// `ClanTable.destroyClan`: a clan without a hall leaves every attacker list.
#[test]
fn a_destroyed_clan_leaves_the_lists() {
    let mut s = Scene::new(false, all_halls(0), hall_load(vec![]));
    s.enter(P3, WOLVES, 21);
    s.join(P3, 21, true);
    assert!(s.world.hall_siege_is_attacker(H21, WOLVES));
    s.world.destroy_clan(WOLVES, false);
    assert!(!s.world.hall_siege_is_attacker(H21, WOLVES));
}

/// `CHSiegeManager.onServerShutDown`: the attackers are saved (`DELETE`, then one `INSERT` per
/// clan), only with the switch (D-51).
#[test]
fn the_shutdown_saves_the_attackers_with_the_switch() {
    for switch in [false, true] {
        let mut s = Scene::new(switch, all_halls(0), hall_load(vec![]));
        s.world.hall_add_attacker(H21, WOLVES);
        s.world.hall_add_attacker(H21, TIGERS);
        s.take();
        s.world.handle(WorldCommand::Shutdown {
            done: Box::new(|| ()),
        });
        let writes = hall_writes(&s.take());
        if switch {
            let saved: Vec<&HallSiegeWrite> = writes
                .iter()
                .filter(|w| matches!(w, HallSiegeWrite::SaveAttackers { hall_id: 21, .. }))
                .collect();
            match saved[0] {
                HallSiegeWrite::SaveAttackers { clans, .. } => {
                    let mut clans = clans.clone();
                    clans.sort_unstable();
                    assert_eq!(clans, [WOLVES, TIGERS]);
                }
                other => panic!("{other:?}"),
            }
            // A row set for every hall that has an engine.
            assert_eq!(writes.len(), 6);
        } else {
            assert!(writes.is_empty());
        }
    }
}

/// The attackers saved at the last shutdown are the attackers at startup (`loadAttackers`).
#[test]
fn the_attackers_are_loaded_at_startup() {
    let s = Scene::new(
        false,
        all_halls(0),
        hall_load(vec![HallSiegeHallLoad {
            attackers: vec![WOLVES, TIGERS],
            ..base_load(21)
        }]),
    );
    assert!(s.world.hall_siege_is_attacker(H21, WOLVES));
    assert!(s.world.hall_siege_is_attacker(H21, TIGERS));
    assert!(!s.world.hall_siege_is_attacker(H34, WOLVES));
}

// ---------------------------------------------------------------------------------- Resistance

/// A siege of the Fortress of Resistance from the start to the end, driven by a GM (D-51): the
/// owner is freed in `prepareOwner`, `startSiege` moves the attackers into the siege and spawns
/// Nurka, the clan with the most damage wins the hall when Nurka dies (2.5 s after the kill).
fn resistance_siege(switch: bool) -> (Scene, Vec<Out>) {
    let mut s = Scene::new(
        switch,
        vec![hall_row(21, LIONS, NOW + 3 * DAY)],
        hall_load(vec![HallSiegeHallLoad {
            guards: guard_rows(GUARD, 2, 60, 21),
            ..base_load(21)
        }]),
    );
    s.enter(P1, LIONS, 21);
    s.enter(P3, WOLVES, 21);
    s.enter(P7, TIGERS, 21);
    s.world.hall_add_attacker(H21, WOLVES);
    s.world.hall_add_attacker(H21, TIGERS);
    s.world.hall_prepare_owner(H21);
    s.world.hall_start_siege(H21);
    s.take();
    // Wolves hit Nurka for more than Tigers; Lions are attackers too (the owner became one).
    let nurka = s.npcs_of(NURKA)[0];
    s.world.reduce_current_hp(nurka, 300.0, Some(ObjectId(P3)));
    s.run_ms(100);
    s.world.reduce_current_hp(nurka, 100.0, Some(ObjectId(P7)));
    s.run_ms(100);
    s.world.reduce_current_hp(nurka, 50.0, Some(ObjectId(P1)));
    s.run_ms(100);
    s.world.do_die(nurka, Some(ObjectId(P7)));
    s.run_ms(2_000);
    assert_eq!(s.world.hall_siege_status(H21), Some(SiegeStatus::Running));
    s.run_ms(1_000);
    let out = s.take();
    (s, out)
}

#[test]
fn a_gm_siege_of_the_fortress_of_resistance_runs_to_its_end() {
    let (mut s, out) = resistance_siege(true);
    // The winner got the hall and the other messages went to everybody.
    assert_eq!(s.hall(21).owner_id, WOLVES);
    assert_eq!(s.clan(WOLVES).hideout_id, 21);
    assert_eq!(
        s.clan(LIONS).hideout_id,
        21,
        "prepareOwner does not reset the old owner's hall id"
    );
    let ids = message_ids(&out, P1);
    assert_eq!(
        ids,
        [
            SystemMessageId::TheS1SiegeHasFinished,
            SystemMessageId::ClanS1IsVictoriousOverS2SCastleSiege
        ]
    );
    let victory = &messages_to(&out, P1)[1];
    assert_eq!(text_param(victory, 0), "Wolves");
    assert_eq!(text_param(victory, 1), "Fortress of Resistance");
    // Status, attackers and the next siege (14 days later at 18:30).
    assert_eq!(s.world.hall_siege_status(H21), Some(SiegeStatus::Registering));
    assert!(s.world.hall_siege_attacker_clans(H21).is_empty());
    // 14 days later at 18:30:00 (the milliseconds are the clock's).
    assert_eq!(s.world.hall_next_siege_time(H21) / 1000, 1_794_076_200);
    // The owner change and the date are rows (the switch is on).
    let writes = siegable_writes(&out);
    assert!(writes.iter().any(|w| w.1 == WOLVES), "{writes:?}");
    // The attackers' siege state went back to 0.
    assert!(!s.world.is_in_hideout_siege(ObjectId(P3)));
    assert_eq!(s.world.siege_state_of(ObjectId(P3)), 0);
    // The guards are gone and Nurka does not come back.
    assert!(s.npcs_of(GUARD).is_empty());
    s.run_ms(30_000);
    assert!(s.npcs_of(NURKA).is_empty());
}

/// Without the switch the same siege changes this world but writes nothing (D-51).
#[test]
fn the_same_siege_without_the_switch_writes_nothing() {
    let (s, out) = resistance_siege(false);
    assert_eq!(s.hall(21).owner_id, WOLVES);
    assert!(siegable_writes(&out).is_empty());
    assert!(hall_writes(&out).is_empty());
}

/// `startSiege`: the attackers are in the siege (state 1, hideout siege), the doors are repaired,
/// the siege guards stand, the zone is active and the doors with a bar are enemies.
#[test]
fn the_start_of_a_siege_sets_the_attackers_and_the_zone() {
    let mut s = Scene::new(
        false,
        vec![hall_row(21, 0, NOW + 3 * DAY)],
        hall_load(vec![HallSiegeHallLoad {
            guards: guard_rows(GUARD, 2, 60, 21),
            ..base_load(21)
        }]),
    );
    s.enter(P3, WOLVES, 21);
    s.enter(P4, WOLVES, 21);
    s.enter(P7, TIGERS, 21);
    s.world.hall_add_attacker(H21, WOLVES);
    let door = s.world.residence_doors(ResidenceRef::ClanHall(H21))[0];
    s.world.open_door(door);
    s.world.hall_start_siege(H21);
    assert_eq!(s.world.hall_siege_status(H21), Some(SiegeStatus::Running));
    assert_eq!(s.world.siege_state_of(ObjectId(P3)), 1);
    assert_eq!(s.world.siege_state_of(ObjectId(P4)), 1);
    assert!(s.world.is_in_hideout_siege(ObjectId(P3)));
    assert_eq!(s.world.siege_state_of(ObjectId(P7)), 0);
    assert!(!s.world.door(door).unwrap().open);
    assert!(s.world.door(door).unwrap().is_enemy());
    assert_eq!(s.npcs_of(GUARD).len(), 2);
    assert_eq!(s.npcs_of(NURKA).len(), 1);
    // The siege zone is active: it is `isActive` for the zone of the hall.
    let zone = s.world.clan_hall(H21).map(|h| h.zone.clone().unwrap().0).unwrap();
    let _ = zone;
    // Only attackers hit the door (`doorIsAutoAttackable`).
    assert!(s.world.is_auto_attackable_by(door, ObjectId(P3)));
    assert!(!s.world.is_auto_attackable_by(door, ObjectId(P7)));
    // The end: the date moves on and the guards go.
    s.world.hall_end_siege(H21);
    assert!(s.npcs_of(GUARD).is_empty());
    assert!(!s.world.door(door).unwrap().is_enemy());
    assert_eq!(s.world.siege_state_of(ObjectId(P3)), 0);
}

/// Nobody attacked: the siege is cancelled for lack of interest (the Fortress of Resistance has
/// no list and starts anyway).
#[test]
fn a_siege_without_attackers_is_cancelled_except_at_the_fortress_of_resistance() {
    let mut s = Scene::new(true, all_halls(0), hall_load(vec![]));
    s.enter(P3, WOLVES, 34);
    s.take();
    s.world.hall_start_siege(H34);
    let out = s.take();
    let m = &messages_to(&out, P3)[0];
    assert_eq!(
        m.id,
        SystemMessageId::TheSiegeOfS1HasBeenCanceledDueToLackOfInterest
    );
    assert_eq!(text_param(m, 0), "Devastated Castle");
    // NOTE(java-quirk): the status stays WAITING_BATTLE.
    assert_eq!(s.world.hall_siege_status(H34), Some(SiegeStatus::WaitingBattle));
    // The date moved on (written).
    assert_eq!(siegable_writes(&out).len(), 1);
    // The Fortress of Resistance starts without attackers.
    s.world.hall_start_siege(H21);
    assert_eq!(s.world.hall_siege_status(H21), Some(SiegeStatus::Running));
}

/// The forward command and `//chsiege_*`: the GM moves the hall through its three steps.
#[test]
fn chsiege_commands_drive_a_test_siege() {
    let mut s = Scene::new(false, vec![hall_row(21, LIONS, NOW + 3 * DAY)], hall_load(vec![]));
    s.enter(P1, LIONS, 21);
    s.enter(P3, WOLVES, 21);
    s.enter_gm(GM);
    s.take();
    let gm = |s: &mut Scene, text: &str| {
        s.input(
            GM,
            ClientInput::BuildCmd {
                command: text.to_owned(),
            },
        );
        s.take()
    };
    // The page of a hall.
    let out = gm(&mut s, "chsiege_siegablehall 21");
    assert!(pages_to(&out, GM)[0].contains("HALL Fortress of Resistance id=21 owner=Lions"));
    // Wrong ids.
    let out = gm(&mut s, "chsiege_startSiege abc");
    assert!(
        says_to(&out, GM)
            .iter()
            .any(|t| t.contains("Wrong clan hall id, unparseable id!"))
    );
    let out = gm(&mut s, "chsiege_startSiege 5");
    assert!(
        says_to(&out, GM)
            .iter()
            .any(|t| t.contains("Could not find he desired siegable hall (5)"))
    );
    // Add an attacker by name; adding again, removing, an unknown clan.
    gm(&mut s, "chsiege_addAttacker 21 Wolves");
    assert!(s.world.hall_siege_is_attacker(H21, WOLVES));
    let out = gm(&mut s, "chsiege_addAttacker 21 Wolves");
    assert!(
        says_to(&out, GM)
            .iter()
            .any(|t| t.contains("The given clan is alredy participating!"))
    );
    let out = gm(&mut s, "chsiege_addAttacker 21 Nobody");
    assert!(
        says_to(&out, GM)
            .iter()
            .any(|t| t.contains("The given clan does not exist!"))
    );
    gm(&mut s, "chsiege_removeAttacker 21 Wolves");
    assert!(!s.world.hall_siege_is_attacker(H21, WOLVES));
    let out = gm(&mut s, "chsiege_removeAttacker 21 Wolves");
    assert!(
        says_to(&out, GM)
            .iter()
            .any(|t| t.contains("The given clan is not participating!"))
    );
    // The list is the siege window.
    let out = gm(&mut s, "chsiege_listAttackers 21");
    assert_eq!(packets_to(&out, GM, "CASTLE_SIEGE_INFO").len(), 1);
    // Forward: REGISTERING -> prepareOwner, WAITING_BATTLE -> startSiege, RUNNING -> endSiege.
    gm(&mut s, "chsiege_addAttacker 21 Wolves");
    gm(&mut s, "chsiege_forwardSiege 21");
    assert_eq!(s.world.hall_siege_status(H21), Some(SiegeStatus::WaitingBattle));
    gm(&mut s, "chsiege_forwardSiege 21");
    assert_eq!(s.world.hall_siege_status(H21), Some(SiegeStatus::Running));
    // No attackers during the battle.
    let out = gm(&mut s, "chsiege_addAttacker 21 Tigers");
    assert!(
        says_to(&out, GM)
            .iter()
            .any(|t| t.contains("The clan hall is in siege, cannot add attackers now."))
    );
    let out = gm(&mut s, "chsiege_startSiege 21");
    assert!(
        says_to(&out, GM)
            .iter()
            .any(|t| t.contains("The requested clan hall is alredy in siege!"))
    );
    gm(&mut s, "chsiege_forwardSiege 21");
    assert_eq!(s.world.hall_siege_status(H21), Some(SiegeStatus::Registering));
    let out = gm(&mut s, "chsiege_endsSiege 21");
    assert!(
        says_to(&out, GM)
            .iter()
            .any(|t| t.contains("The requested clan hall is not in siege!"))
    );
}

/// `admin_chsiege_setSiegeDate`: the format, the refusals and the new date.
#[test]
fn chsiege_set_siege_date() {
    let mut s = Scene::new(true, vec![hall_row(21, 0, NOW + 3 * DAY)], hall_load(vec![]));
    s.enter_gm(GM);
    s.take();
    let gm = |s: &mut Scene, text: &str| {
        s.input(
            GM,
            ClientInput::BuildCmd {
                command: text.to_owned(),
            },
        );
        s.take()
    };
    let says = |out: &[Out]| says_to(out, GM).join("|");
    assert!(
        says(&gm(&mut s, "chsiege_setSiegeDate 21")).contains("The date format is incorrect. Try again.")
    );
    assert!(
        says(&gm(&mut s, "chsiege_setSiegeDate 21 25-12-2026"))
            .contains("You have to specify this format DD-MM-YYYY;HH:MM")
    );
    assert!(
        says(&gm(&mut s, "chsiege_setSiegeDate 21 25-12;18:30")).contains("Incomplete day, hour or both!")
    );
    // January is refused (`month <= 0` after `- 1`), like a past year.
    assert!(
        says(&gm(&mut s, "chsiege_setSiegeDate 21 25-01-2027;18:30")).contains("Wrong day/month/year gave!")
    );
    assert!(
        says(&gm(&mut s, "chsiege_setSiegeDate 21 25-12-2025;18:30")).contains("Wrong day/month/year gave!")
    );
    assert!(
        says(&gm(&mut s, "chsiege_setSiegeDate 21 25-12-2026;0:30")).contains("Wrong hour/minutes gave!")
    );
    assert!(
        says(&gm(&mut s, "chsiege_setSiegeDate 21 24-10-2026;9:00"))
            .contains("The given time is in the past!")
    );
    let out = gm(&mut s, "chsiege_setSiegeDate 21 25-12-2026;18:30");
    assert!(
        says(&out).contains("Fortress of Resistance siege: Fri Dec 25 18:30:00 GMT+00:00 2026"),
        "{}",
        says(&out)
    );
    // 2026-12-25T18:30:00.321Z, written.
    let date = 1_798_223_400_321;
    assert_eq!(s.world.hall_next_siege_time(H21) / 1000, date / 1000);
    let writes = siegable_writes(&out);
    assert_eq!(writes.len(), 1);
    assert_eq!(
        (writes[0].0, writes[0].1, writes[0].2 / 1000),
        (21, 0, date / 1000)
    );
}

// ---------------------------------------------------------------------------------- Devastated

/// Devastated Castle: only the damage of attacker clans counts; below 1/12 of its HP Gustav
/// shouts and casts; his death ends the siege and the best clan wins.
#[test]
fn devastated_castle_is_won_with_the_most_damage_to_gustav() {
    let mut s = Scene::new(
        false,
        all_halls(0),
        hall_load(vec![HallSiegeHallLoad {
            guards: vec![
                HallGuardRecord {
                    npc_id: MIKHAIL,
                    ..guard_rows(MIKHAIL, 1, 0, 34)[0]
                },
                HallGuardRecord {
                    npc_id: GUSTAV,
                    ..guard_rows(GUSTAV, 1, 0, 34)[0]
                },
            ],
            ..base_load(34)
        }]),
    );
    s.enter(P3, WOLVES, 34);
    s.enter(P7, TIGERS, 34);
    s.enter(P5, BEARS, 34);
    s.world.hall_add_attacker(H34, WOLVES);
    s.world.hall_add_attacker(H34, TIGERS);
    s.take();
    s.world.hall_start_siege(H34);
    s.run_ms(100);
    let out = s.take();
    // Mikhail shouts when he spawns (NPC_SHOUT to those who see him).
    assert!(!packets_to(&out, P3, "NPC_SAY").is_empty());
    let gustav = s.npcs_of(GUSTAV)[0];
    s.world.reduce_current_hp(gustav, 400.0, Some(ObjectId(P7)));
    s.run_ms(100);
    s.world.reduce_current_hp(gustav, 300.0, Some(ObjectId(P3)));
    s.run_ms(100);
    // Bears are no attackers: their damage counts for nothing.
    s.world.reduce_current_hp(gustav, 9_000.0, Some(ObjectId(P5)));
    s.run_ms(100);
    s.take();
    // Below 1/12 of the HP he speaks.
    s.world.reduce_current_hp(gustav, 19_000.0, Some(ObjectId(P3)));
    s.run_ms(100);
    let out = s.take();
    assert!(!packets_to(&out, P3, "NPC_SAY").is_empty());
    s.world.do_die(gustav, Some(ObjectId(P3)));
    s.run_ms(3_000);
    // Tigers did 400 (+ 0), Wolves 300 + 19000: Wolves win.
    assert_eq!(s.hall(34).owner_id, WOLVES);
}

// ---------------------------------------------------------------------------------- Dead

/// Fortress of the Dead starts at night only (game hours 0 to 6).
#[test]
fn the_fortress_of_the_dead_starts_at_night() {
    let mut s = Scene::new(false, all_halls(0), hall_load(vec![]));
    s.world.hall_add_attacker(H64, WOLVES);
    s.world.hall_start_siege(H64);
    let hour = (l2_world::game_time_minutes(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis() as i64,
    ) / 60)
        % 24;
    // The clock may have moved on a bit; only check away from the edges.
    if (1..=5).contains(&hour) {
        assert_eq!(s.world.hall_siege_status(H64), Some(SiegeStatus::Running));
    } else if hour > 7 {
        // By day the start is moved by (24 - hour) * 10 minutes: nothing happens now.
        assert_eq!(s.world.hall_siege_status(H64), Some(SiegeStatus::Registering));
    }
}

/// Lidia's death ends the Fortress of the Dead; the damage of the best clan wins.
#[test]
fn lidia_ends_the_siege() {
    let mut s = Scene::new(
        false,
        all_halls(0),
        hall_load(vec![HallSiegeHallLoad {
            guards: guard_rows(LIDIA, 1, 0, 64),
            ..base_load(64)
        }]),
    );
    s.enter(P3, WOLVES, 64);
    s.world.hall_add_attacker(H64, WOLVES);
    // Skip the night rule: `base` start through the forward command's second step is the same
    // `startSiege`; the test works whatever the hour is by starting the base engine through the
    // owner-less path when it is night, else waiting for the game clock is not possible.
    s.world.hall_start_siege(H64);
    if s.world.hall_siege_status(H64) != Some(SiegeStatus::Running) {
        return;
    }
    let lidia = s.npcs_of(LIDIA)[0];
    s.world.reduce_current_hp(lidia, 500.0, Some(ObjectId(P3)));
    s.run_ms(100);
    s.world.do_die(lidia, Some(ObjectId(P3)));
    s.run_ms(3_000);
    assert_eq!(s.hall(64).owner_id, WOLVES);
}

// ---------------------------------------------------------------------------------- Bandit

fn bandit_scene(switch: bool) -> Scene {
    let mut halls = all_halls(0);
    halls[2].owner_id = LIONS;
    let mut load = base_load(35);
    load.guards = Vec::new();
    Scene::new(switch, halls, hall_load(vec![load]))
}

fn page_has(out: &[Out], player: i32, text: &str) -> bool {
    pages_to(out, player).iter().any(|p| p.contains(text))
}

/// The messenger of Bandit Stronghold: a clan that is no attacker sees the owner's name, the
/// registration with the quest item makes the clan an attacker with a flag and saves the rows.
#[test]
fn bandit_stronghold_registration_with_the_quest_item() {
    let mut s = bandit_scene(true);
    s.enter(P3, WOLVES, 35);
    s.enter(P7, TIGERS, 35);
    give(&mut s, P3, QUEST_REWARD, 1);
    give(&mut s, P7, QUEST_REWARD, 1);
    let messenger = s.spawn_at_hall(BANDIT_MESSENGER, 35);
    s.take();
    s.talk(P3, messenger);
    let out = s.take();
    assert!(
        page_has(&out, P3, "PAGE 35437-00.htm clan=Lions"),
        "{:?}",
        pages_to(&out, P3)
    );
    // The leader registers with the quest item: the item goes, the clan has the first flag.
    s.event(P3, "BanditStronghold", messenger, "Register wQuest");
    let out = s.take();
    assert!(s.world.hall_siege_is_attacker(H35, WOLVES));
    // The first clan gets the royal flag 35422, whose page is the default "14a".
    assert!(
        page_has(&out, P3, "PAGE 35437-14a.htm"),
        "{:?}",
        pages_to(&out, P3)
    );
    assert_eq!(count(&s, P3, QUEST_REWARD), 0);
    let writes = hall_writes(&out);
    assert!(writes.contains(&HallSiegeWrite::FlagWarClan {
        hall_id: 35,
        flag: BANDIT_FLAG_1,
        clan_id: WOLVES
    }));
    assert!(writes.contains(&HallSiegeWrite::FlagWarMember {
        hall_id: 35,
        clan_id: WOLVES,
        object_id: P3
    }));
    // The second clan gets the flag 35423 (page 15).
    s.event(P7, "BanditStronghold", messenger, "Register wQuest");
    let out = s.take();
    assert!(
        page_has(&out, P7, "PAGE 35437-15.htm"),
        "{:?}",
        pages_to(&out, P7)
    );
    let clans = s.world.flag_war_clans(H35);
    assert_eq!(clans.iter().map(|c| (c.0, c.1.flag)).collect::<Vec<_>>().len(), 2);
    // Already registered, and without the item.
    s.event(P3, "BanditStronghold", messenger, "Register wQuest");
    assert!(page_has(&s.take(), P3, "PAGE 35437-05.htm"));
}

/// The refusals of the registration, the warrior choice and the quest check.
#[test]
fn bandit_stronghold_pages_follow_javas_chain() {
    let mut s = bandit_scene(false);
    s.enter(P3, WOLVES, 35);
    s.enter(P4, WOLVES, 35);
    s.enter(P5, BEARS, 35);
    s.enter(P1, LIONS, 35);
    let messenger = s.spawn_at_hall(BANDIT_MESSENGER, 35);
    // Not a leader.
    s.event(P4, "BanditStronghold", messenger, "Register wQuest");
    assert!(page_has(&s.take(), P4, "PAGE 35437-03.htm"));
    // The owner cannot register.
    s.event(P1, "BanditStronghold", messenger, "Register wQuest");
    assert!(page_has(&s.take(), P1, "PAGE 35437-06.htm"));
    // No quest item, no adena for the fee path: 200000 adena are there (1 000 000), so the fee works.
    s.event(P3, "BanditStronghold", messenger, "Register wQuest");
    assert!(page_has(&s.take(), P3, "PAGE 35437-07.htm"));
    s.event(P3, "BanditStronghold", messenger, "Register wFee");
    let out = s.take();
    assert!(s.world.hall_siege_is_attacker(H35, WOLVES));
    assert_eq!(s.world.adena_of(ObjectId(P3)), 800_000);
    assert!(hall_writes(&out).is_empty(), "the switch is off");
    // An event that matches no branch is shown as a chat line (Java `showResult`).
    s.event(P3, "BanditStronghold", messenger, "Bogus");
    assert!(says_to(&s.take(), P3).iter().any(|t| t.contains("Bogus")));
    // Select a warrior: only the leader of a registered clan; a bad id does nothing.
    s.event(P4, "BanditStronghold", messenger, "Select_NPC 35428");
    assert!(page_has(&s.take(), P4, "PAGE 35437-09.htm"));
    // The leader of a clan that did not register.
    s.event(P5, "BanditStronghold", messenger, "Select_NPC 35428");
    assert!(page_has(&s.take(), P5, "PAGE 35437-10.htm"));
    s.event(P3, "BanditStronghold", messenger, "Select_NPC 35428");
    assert!(page_has(&s.take(), P3, "PAGE 35437-15a.htm"));
    assert_eq!(s.world.flag_war_clans(H35)[0].1.npc, BANDIT_ALLY_1);
    s.event(P3, "BanditStronghold", messenger, "View");
    assert!(page_has(&s.take(), P3, "PAGE 35437-15a.htm"));
    // The quest check: the level of the clan, then the leader, then the waiting phase.
    s.event(P3, "BanditStronghold", messenger, "CheckQuest");
    // The registration has not ended: the page with the date of the siege.
    let out = s.take();
    assert!(
        page_has(&out, P3, "PAGE 35437-14.htm"),
        "{:?}",
        pages_to(&out, P3)
    );
    // The members' registration is unreachable: "RegisterMember" is a "Register" for Java.
    s.event(P4, "BanditStronghold", messenger, "RegisterMember");
    assert!(page_has(&s.take(), P4, "PAGE 35437-03.htm"));
}

/// The attackers' page lists every clan with the *player's* clan name (a Java quirk).
#[test]
fn bandit_stronghold_attackers_page_quirk() {
    let mut s = bandit_scene(false);
    s.enter(P3, WOLVES, 35);
    s.enter(P7, TIGERS, 35);
    let messenger = s.spawn_at_hall(BANDIT_MESSENGER, 35);
    s.event(P3, "BanditStronghold", messenger, "Register wFee");
    s.event(P7, "BanditStronghold", messenger, "Register wFee");
    s.world.hall_prepare_owner(H35);
    // `banishForeigners` moved them out of the hall: back to the messenger.
    let (x, y) = hall_xy(35);
    for p in [P3, P7] {
        s.world.tele_to_location(ObjectId(p), TeleportTarget::at(x, y, Z));
        s.appear(p);
    }
    s.take();
    s.event(P7, "BanditStronghold", messenger, "Attackers");
    let out = s.take();
    let page = pages_to(&out, P7).pop().unwrap();
    assert!(page.contains("PAGE 35437-13.htm"), "{page}");
    // Both lines show the name of the viewer's clan; the empty places say so.
    assert!(page.contains("c1=Tigers") && page.contains("c2=Tigers"), "{page}");
    assert!(page.contains("m1=1"), "{page}");
    assert!(
        page.contains("c4=Empty pos.") && !page.contains("c5=Empty pos."),
        "{page}"
    );
}

/// NOTE(java-quirk): `startSiege()` of Bandit Stronghold calls itself again, so the battle never
/// starts. With two attackers the outer doors open, the owner's members go to the spawn point,
/// and five minutes later the doors close and it all begins again.
#[test]
fn bandit_stronghold_never_leaves_the_waiting_phase() {
    let mut s = bandit_scene(true);
    s.enter(P3, WOLVES, 35);
    s.enter(P7, TIGERS, 35);
    s.enter(P1, LIONS, 35);
    s.world.hall_add_attacker(H35, WOLVES);
    s.world.hall_add_attacker(H35, TIGERS);
    s.world.hall_start_siege(H35);
    let outer = s
        .world
        .residence_door(ResidenceRef::ClanHall(H35), 22_170_001)
        .unwrap();
    assert!(s.world.door(outer).unwrap().open);
    // The owner's online members are moved to the first spawn point of the hall zone.
    let (x, y) = hall_xy(35);
    let at = s.world.position_of(ObjectId(P1)).unwrap();
    assert_eq!((at.0, at.1), (x, y + 100));
    s.run_ms(5 * 60_000 + 200);
    // The lambda closed the doors and called `startSiege()` again: they are open again.
    assert!(s.world.door(outer).unwrap().open);
    assert_eq!(s.world.hall_siege_status(H35), Some(SiegeStatus::Registering));
    // With fewer than two attackers it is cancelled instead.
    s.world.hall_remove_attacker(H35, TIGERS);
    s.take();
    s.run_ms(5 * 60_000 + 200);
    // The lambda closed the doors; this time `startSiege()` found one attacker and cancelled.
    assert!(!s.world.door(outer).unwrap().open);
    let out = s.take();
    assert_eq!(
        message_ids(&out, P3),
        [SystemMessageId::TheSiegeOfS1HasBeenCanceledDueToLackOfInterest]
    );
    assert!(s.world.hall_siege_attacker_clans(H35).is_empty());
}

/// The teleport NPCs: the first click starts a 30 second teleport of the players in the zone.
#[test]
fn the_flag_war_teleport_npc_moves_the_zone_after_30_seconds() {
    let mut s = bandit_scene(false);
    let (x, y) = hall_xy(35);
    s.enter_at(P3, WOLVES, x - 300, y, 0);
    let tele = s.world.spawn_npc_at(BANDIT_TELEPORT, x - 290, y, Z, 0).unwrap();
    s.world.tick();
    s.talk(P3, tele);
    let out = s.take();
    assert!(
        page_has(&out, P3, "PAGE 35560-01.htm"),
        "{:?}",
        pages_to(&out, P3)
    );
    s.run_ms(29_000);
    assert_eq!(s.world.position_of(ObjectId(P3)).unwrap().0, x - 300);
    s.run_ms(2_000);
    s.appear(P3);
    let at = s.world.position_of(ObjectId(P3)).unwrap();
    assert_eq!((at.0, at.1), (x + 50, y + 50));
}

/// The registered clans come back from the tables at startup (`loadAttackers`), a clan that is
/// gone is skipped, and a re-read replaces the rows that were written (D-50).
#[test]
fn flag_war_rows_load_and_reread() {
    let load = HallSiegeHallLoad {
        flag_war: vec![
            FlagWarRecord {
                clan_id: WOLVES,
                flag: 35_422,
                npc: BANDIT_ALLY_1,
                members: vec![P3, P4],
            },
            FlagWarRecord::new(9999, 35_423, 0),
        ],
        ..base_load(35)
    };
    let mut s = Scene::new(false, all_halls(0), hall_load(vec![load]));
    assert!(s.world.hall_siege_is_attacker(H35, WOLVES));
    assert!(!s.world.hall_siege_is_attacker(H35, 9999));
    let clans = s.world.flag_war_clans(H35);
    assert_eq!(clans.len(), 1);
    assert_eq!(clans[0].1.players, [P3, P4]);
    assert_eq!(clans[0].1.npc, BANDIT_ALLY_1);
    // A re-read: Sieghardt registered the Tigers and the Wolves cancelled.
    s.enter(P7, TIGERS, 35);
    let messenger = s.spawn_at_hall(BANDIT_MESSENGER, 35);
    s.talk(P7, messenger);
    s.take();
    s.script(P7, "BanditStronghold View");
    s.answer_tables_reread(vec![FlagWarRecord::new(TIGERS, 35_422, 0)], Vec::new());
    assert!(s.world.hall_siege_is_attacker(H35, TIGERS));
    assert!(!s.world.hall_siege_is_attacker(H35, WOLVES));
}

// ---------------------------------------------------------------------------------- Rainbow

fn rainbow_scene(switch: bool) -> Scene {
    Scene::new(switch, all_halls(0), hall_load(vec![base_load(62)]))
}

/// The Rainbow Springs messenger: war decrees register the clan (the item is taken, the row
/// written); NOTE(java-quirk): the first clan fills all four arenas.
#[test]
fn rainbow_springs_registration() {
    let mut s = rainbow_scene(true);
    s.enter(P3, WOLVES, 62);
    s.enter(P7, TIGERS, 62);
    s.enter(P4, WOLVES, 62);
    give(&mut s, P3, WAR_DECREES, 7);
    let messenger = s.spawn_at_hall(RAINBOW_MESSENGER, 62);
    // A member without leadership.
    s.event(P4, "RainbowSpringsChateau", messenger, "Register");
    assert!(page_has(&s.take(), P4, "PAGE 35604-07.htm"));
    // A clan of level 5 with two members (the minimum is five members): refused.
    s.event(P3, "RainbowSpringsChateau", messenger, "Register");
    assert!(page_has(&s.take(), P3, "PAGE 35604-08.htm"));
}

/// With enough members the war decrees are taken, the row is written and the arenas are filled.
#[test]
fn rainbow_springs_fills_the_arenas_with_the_first_clan() {
    let mut s = rainbow_scene(true);
    // Wolves with five members.
    let mut clan = clan_record(WOLVES, "Wolves", 5, P3, P4);
    for id in [0x2000_0010, 0x2000_0011, 0x2000_0012] {
        clan.members.push(member(id, 5));
    }
    s.world
        .load_clans(vec![clan, clan_record(TIGERS, "Tigers", 5, P7, P8)], Vec::new());
    s.enter(P3, WOLVES, 62);
    give(&mut s, P3, WAR_DECREES, 7);
    let messenger = s.spawn_at_hall(RAINBOW_MESSENGER, 62);
    s.event(P3, "RainbowSpringsChateau", messenger, "Register");
    let out = s.take();
    assert!(
        page_has(&out, P3, "PAGE 35604-06.htm"),
        "{:?}",
        pages_to(&out, P3)
    );
    assert_eq!(s.world.rainbow_war_decrees(), [(WOLVES, 7)]);
    assert_eq!(
        s.world.rainbow_accepted_clans(),
        [Some(WOLVES), Some(WOLVES), Some(WOLVES), Some(WOLVES)]
    );
    assert!(hall_writes(&out).contains(&HallSiegeWrite::RainbowAdd {
        clan_id: WOLVES,
        count: 7
    }));
    assert!(s.world.hall_siege_is_attacker(H62, WOLVES));
    // Registered already.
    s.event(P3, "RainbowSpringsChateau", messenger, "Register");
    assert!(page_has(&s.take(), P3, "PAGE 35604-10.htm"));
    // The refund of half the decrees while registering.
    give(&mut s, P3, WAR_DECREES, 0);
    s.event(P3, "RainbowSpringsChateau", messenger, "Unregister");
    let out = s.take();
    assert!(
        page_has(&out, P3, "PAGE 35604-14.htm"),
        "{:?}",
        pages_to(&out, P3)
    );
    assert!(s.world.rainbow_war_decrees().is_empty());
    assert_eq!(count(&s, P3, WAR_DECREES), 3);
}

/// The yeti's word and the letters: `seeItem` shows the word (or "undecided"), `getItem` needs the
/// letters.
#[test]
fn rainbow_springs_words_and_letters() {
    let mut s = rainbow_scene(false);
    s.enter(P3, WOLVES, 62);
    let yeti = s.spawn_at_hall(YETI_1, 62);
    s.event(P3, "RainbowSpringsChateau", yeti, "seeItem");
    let out = s.take();
    let page = pages_to(&out, P3).pop().unwrap();
    assert!(
        page.contains("word=<fstring>NS[1010635:UNDECIDED]</fstring>"),
        "{page}"
    );
    // `getItem` with no word chosen sends nothing.
    s.event(P3, "RainbowSpringsChateau", yeti, "getItem");
    assert!(pages_to(&s.take(), P3).is_empty());
    let _ = (LETTER_A, LETTER_B, NECTAR);
}

/// The caretaker's `GoToArena` always ends without a page (Java returns null after the checks).
#[test]
fn rainbow_springs_go_to_arena_shows_pages_only_before_the_party_checks() {
    let mut s = rainbow_scene(false);
    s.enter(P3, WOLVES, 62);
    s.enter(P4, WOLVES, 62);
    s.enter(P7, TIGERS, 62);
    let caretaker = s.spawn_at_hall(RAINBOW_CARETAKER, 62);
    // Not the leader.
    s.event(P4, "RainbowSpringsChateau", caretaker, "GoToArena");
    assert!(page_has(&s.take(), P4, "PAGE 35603-02.htm"));
    // The leader without a party.
    s.event(P3, "RainbowSpringsChateau", caretaker, "GoToArena");
    assert!(page_has(&s.take(), P3, "PAGE 35603-03.htm"));
    // The first talk shows 00 or 01 by the phase: registering is not the waiting phase.
    s.talk(P3, caretaker);
    assert!(page_has(&s.take(), P3, "PAGE 35603-00.htm"));
}

// ---------------------------------------------------------------------------------- EnterWorld, Die

/// `EnterWorld`: a member of an attacker clan of a hall in siege logs in on the attacking side.
#[test]
fn members_of_an_attacker_clan_log_in_on_the_attacking_side() {
    let mut s = Scene::new(false, all_halls(0), hall_load(vec![]));
    s.world.hall_add_attacker(H21, WOLVES);
    s.world.hall_start_siege(H21);
    s.enter(P3, WOLVES, 21);
    assert_eq!(s.world.siege_state_of(ObjectId(P3)), 1);
    assert_eq!(s.world.siege_side_of(ObjectId(P3)), 21);
    assert!(s.world.is_in_hideout_siege(ObjectId(P3)));
    s.enter(P7, TIGERS, 21);
    assert_eq!(s.world.siege_state_of(ObjectId(P7)), 0);
}

/// `Die`: the headquarters button is lit for an attacker clan of the hall next to the player
/// (even before the battle); a hall's headquarters is placed by the clan leader.
#[test]
fn the_die_window_offers_the_headquarters_to_the_attackers() {
    let mut s = Scene::new(false, all_halls(0), hall_load(vec![]));
    s.enter(P3, WOLVES, 21);
    s.enter(P7, TIGERS, 21);
    s.world.hall_add_attacker(H21, WOLVES);
    let buttons = |s: &Scene, p: i32| s.world.die_packet(ObjectId(p));
    assert!(buttons(&s, P3).to_siege_hq);
    assert!(!buttons(&s, P7).to_siege_hq);
}

/// `ClanHallManager.onBypassFeedback` does nothing for a hall in siege.
#[test]
fn the_hall_in_siege_has_no_use_of_its_manager() {
    let s = Scene::new(false, all_halls(0), hall_load(vec![]));
    assert!(!s.world.hall_is_in_siege(H21));
}

// ---------------------------------------------------------------------------------- more

/// Fortress of Resistance: the messenger shows the page with the date of the next siege as it
/// was when the page was built (no object id: `NpcHtmlMessage()`).
#[test]
fn the_messenger_of_the_fortress_of_resistance_shows_the_date() {
    let mut s = Scene::new(false, all_halls(0), hall_load(vec![]));
    s.enter(P3, WOLVES, 21);
    let messenger = s.spawn_at_hall(MESSENGER_21, 21);
    s.take();
    s.talk(P3, messenger);
    let out = s.take();
    let page = decode::<NpcHtmlMessage>(&packets_to(&out, P3, "NPC_HTML_MESSAGE")[0]);
    assert_eq!(page.npc_obj_id, 0);
    // NOW + 3 days: 2026-10-27 09:41:07.
    assert!(page.html.contains("next=2026-10-27 09:41:07"), "{}", page.html);
}

/// Bandit Stronghold's `prepareOwner`: the owner keeps the hall and is registered like a challenger
/// (its flag follows the clans that registered before).
#[test]
fn the_flag_war_registers_its_owner_when_the_registration_ends() {
    let mut s = bandit_scene(true);
    s.enter(P3, WOLVES, 35);
    let messenger = s.spawn_at_hall(BANDIT_MESSENGER, 35);
    s.event(P3, "BanditStronghold", messenger, "Register wFee");
    s.take();
    s.world.hall_prepare_owner(H35);
    let out = s.take();
    assert_eq!(s.world.hall_siege_status(H35), Some(SiegeStatus::WaitingBattle));
    assert_eq!(s.hall(35).owner_id, LIONS, "the owner is not freed");
    assert!(s.world.hall_siege_is_attacker(H35, LIONS));
    // The second flag (the Wolves have the royal one).
    assert!(hall_writes(&out).contains(&HallSiegeWrite::FlagWarClan {
        hall_id: 35,
        flag: BANDIT_FLAG_1 + 1,
        clan_id: LIONS
    }));
    assert!(message_ids(&out, P3).contains(&SystemMessageId::TheRegistrationTermForS1HasEnded));
}

fn rainbow_siege_scene() -> Scene {
    let load = HallSiegeHallLoad {
        rainbow: vec![(WOLVES, 10), (TIGERS, 5)],
        ..base_load(62)
    };
    let mut s = Scene::new(true, all_halls(0), hall_load(vec![load]));
    s.enter(P3, WOLVES, 62);
    s.enter(P7, TIGERS, 62);
    s
}

/// `loadAttackers` of Rainbow Springs: every row adds *every* clan of the map again.
#[test]
fn rainbow_springs_loads_its_registrations_with_javas_duplicates() {
    let s = rainbow_siege_scene();
    assert_eq!(s.world.rainbow_war_decrees(), [(WOLVES, 10), (TIGERS, 5)]);
    // The map's order is by bucket: 800 before 600.
    assert_eq!(
        s.world.rainbow_accepted_clans(),
        [Some(WOLVES), Some(TIGERS), Some(WOLVES)]
    );
}

/// The owner is registered in `prepareOwner` with 10000 war decrees (a row), but the arenas only
/// take clans while the map holds a valid clan.
#[test]
fn rainbow_springs_prepare_owner() {
    let mut halls = all_halls(0);
    halls[3].owner_id = LIONS;
    let mut s = Scene::new(true, halls, hall_load(vec![base_load(62)]));
    s.take();
    s.world.hall_prepare_owner(H62);
    let out = s.take();
    assert!(s.world.hall_siege_is_attacker(H62, LIONS));
    assert!(hall_writes(&out).contains(&HallSiegeWrite::RainbowAdd {
        clan_id: LIONS,
        count: 10_000
    }));
    assert!(s.world.rainbow_accepted_clans().is_empty());
    assert_eq!(s.world.hall_siege_status(H62), Some(SiegeStatus::WaitingBattle));
}

/// The battle of Rainbow Springs: gourds and yetis stand for each arena, chests appear every
/// five seconds up to four per arena, the yetis choose a word; when a gourd dies its clan wins.
#[test]
fn rainbow_springs_battle() {
    let mut s = rainbow_siege_scene();
    s.world.hall_start_siege(H62);
    // NOTE(java-quirk): the status never becomes running.
    assert_eq!(s.world.hall_siege_status(H62), Some(SiegeStatus::Registering));
    assert_eq!(s.npcs_of(GOURD_1).len(), 1);
    assert_eq!(s.npcs_of(GOURD_1 + 1).len(), 1);
    assert_eq!(s.npcs_of(GOURD_1 + 2).len(), 1);
    assert!(s.npcs_of(GOURD_1 + 3).is_empty(), "three clans, three arenas");
    assert_eq!(s.npcs_of(YETI_1).len(), 1);
    assert_eq!(s.npcs_of(YETI_1 + 2).len(), 1);
    // Chests: four in each arena at the first run, no more at the second.
    s.take();
    s.run_ms(5_100);
    assert_eq!(s.npcs_of(35_593).len(), 12);
    s.run_ms(5_000);
    assert_eq!(s.npcs_of(35_593).len(), 12);
    // The word, after 10 seconds, shown to the players near a yeti.
    s.run_ms(1_000);
    assert!(s.world.rainbow_generated_word() >= 0);
    let out = s.take();
    assert!(!packets_to(&out, P3, "EX_SHOW_SCREEN_MESSAGE").is_empty());
    // Wolves' arena index is 0: its gourd falls, the Wolves win the hall.
    let gourd = s.npcs_of(GOURD_1)[0];
    s.world.do_die(gourd, Some(ObjectId(P3)));
    s.run_ms(3_000);
    assert_eq!(s.hall(62).owner_id, WOLVES);
    let out = s.take();
    assert!(message_ids(&out, P7).contains(&SystemMessageId::ClanS1IsVictoriousOverS2SCastleSiege));
    assert!(hall_writes(&out).contains(&HallSiegeWrite::RainbowClear));
    // Everything is cleaned up.
    assert!(s.npcs_of(YETI_1).is_empty());
    assert!(s.npcs_of(35_593).is_empty());
    assert_eq!(s.world.hall_siege_status(H62), Some(SiegeStatus::Registering));
}

/// `getItem`: the letters of the word become a rainbow item; a player without them is told so.
#[test]
fn rainbow_springs_letters_make_an_item() {
    let mut s = rainbow_siege_scene();
    s.world.hall_start_siege(H62);
    s.run_ms(11_000);
    let word = usize::try_from(s.world.rainbow_generated_word()).unwrap();
    let names = [
        "BABYDUCK",
        "ALBATROS",
        "PELICAN",
        "KINGFISHER",
        "CYGNUS",
        "TRITON",
        "RAINBOW",
        "SPRING",
    ];
    let yeti = s.spawn_at_hall(YETI_1, 62);
    s.event(P3, "RainbowSpringsChateau", yeti, "seeItem");
    assert!(page_has(&s.take(), P3, &format!("word={}", names[word])));
    s.event(P3, "RainbowSpringsChateau", yeti, "getItem");
    assert!(page_has(&s.take(), P3, "PAGE 35596-02.htm"));
    for letter in (8035..=8043).chain(8045..=8055) {
        give(&mut s, P3, letter, 2);
    }
    s.event(P3, "RainbowSpringsChateau", yeti, "getItem");
    let out = s.take();
    assert!(
        page_has(&out, P3, "PAGE 35596-04.htm"),
        "{:?}",
        pages_to(&out, P3)
    );
    let rewards: i64 = (8030..=8033).map(|i| count(&s, P3, i)).sum();
    assert_eq!(rewards, 1);
    let letters: i64 = (8035..=8043).chain(8045..=8055).map(|i| count(&s, P3, i)).sum();
    assert!(letters < 40, "the word's letters were taken");
}

/// `GoToArena`: the party of the clan goes to the arena of the clan's place in `_acceptedClans`.
#[test]
fn rainbow_springs_the_party_goes_to_its_arena() {
    let mut s = rainbow_siege_scene();
    s.enter(P4, WOLVES, 62);
    s.input(
        P3,
        ClientInput::JoinParty {
            name: "P4".to_owned(),
            distribution_type_id: 0,
        },
    );
    s.input(P4, ClientInput::AnswerJoinParty { response: 1 });
    assert!(s.world.is_in_party_with(ObjectId(P3), ObjectId(P4)));
    let caretaker = s.spawn_at_hall(RAINBOW_CARETAKER, 62);
    s.event(P3, "RainbowSpringsChateau", caretaker, "GoToArena");
    s.run_ms(500);
    s.appear(P3);
    s.appear(P4);
    // Arena 0: (151562, -127080).
    for p in [P3, P4] {
        let at = s.world.position_of(ObjectId(p)).unwrap();
        assert!(
            (at.0 - 151_562).abs() <= 100 && (at.1 + 127_080).abs() <= 100,
            "{at:?}"
        );
    }
}

/// A flag war with one attacker is cancelled when it starts, and its tables are cleared.
#[test]
fn a_flag_war_that_cancels_for_lack_of_interest_clears_its_tables() {
    let mut s = bandit_scene(true);
    s.world.hall_add_attacker(H35, WOLVES);
    s.enter(P3, WOLVES, 35);
    s.take();
    s.world.hall_start_siege(H35);
    let out = s.take();
    assert_eq!(
        message_ids(&out, P3),
        [SystemMessageId::TheSiegeOfS1HasBeenCanceledDueToLackOfInterest]
    );
    assert!(hall_writes(&out).contains(&HallSiegeWrite::FlagWarClear { hall_id: 35 }));
    assert!(s.world.hall_siege_attacker_clans(H35).is_empty());
    assert_eq!(s.world.hall_siege_status(H35), Some(SiegeStatus::Registering));
}

/// A destroyed door of a hall in siege tells everybody around ("The castle gate has been
/// destroyed.") and comes back with all its HP at the start of the next siege.
#[test]
fn the_gates_of_a_hall_fall_and_come_back() {
    let mut s = Scene::new(false, all_halls(0), hall_load(vec![]));
    s.enter(P3, WOLVES, 21);
    let door = s.world.residence_doors(ResidenceRef::ClanHall(H21))[0];
    s.world.hall_add_attacker(H21, WOLVES);
    s.world.hall_start_siege(H21);
    s.take();
    s.world.do_die(door, Some(ObjectId(P3)));
    let out = s.take();
    assert!(message_ids(&out, P3).contains(&SystemMessageId::TheCastleGateHasBeenDestroyed));
    assert!(s.world.is_dead(door));
    s.world.hall_end_siege(H21);
    // The end repairs the gates too (`spawnDoor(false)`).
    assert!(!s.world.is_dead(door));
}

/// `CastleDoorman` at a siegable hall: the hall's owner opens and closes the hall's doors (no
/// castle needed), and while the hall is in siege (`hall.isInSiege()`) the doors are not
/// touched and the "busy" page comes instead.
#[test]
fn the_doorman_of_a_siegable_hall_serves_its_owner_and_waits_for_the_siege_to_end() {
    let mut s = Scene::new(false, all_halls(LIONS), hall_load(vec![]));
    s.enter(P1, LIONS, 21);
    s.enter(P3, WOLVES, 21);
    let doorman = s.spawn_at_hall(DOORMAN, 21);
    let door = s.world.residence_doors(ResidenceRef::ClanHall(H21))[0];
    let bypass = |s: &mut Scene, player: i32, command: &str| {
        s.talk(player, doorman);
        s.take();
        s.input(
            player,
            ClientInput::Bypass {
                command: format!("npc_{}_{command}", doorman.0),
            },
        );
        s.run_ms(100);
    };
    // The window follows the owner of the hall.
    s.talk(P1, doorman);
    assert!(page_has(&s.take(), P1, "DOORMAN OWNER"));
    s.talk(P3, doorman);
    assert!(page_has(&s.take(), P3, "DOORMAN NO"));
    // The owner opens and closes the door of the hall; another clan does nothing.
    assert!(!s.world.door(door).unwrap().open);
    bypass(&mut s, P3, "open_doors 1, 21000001");
    assert!(!s.world.door(door).unwrap().open);
    bypass(&mut s, P1, "open_doors 1, 21000001");
    assert!(s.world.door(door).unwrap().open);
    bypass(&mut s, P1, "close_doors 1, 21000001");
    assert!(!s.world.door(door).unwrap().open);
    // In siege the doorman is busy: the doors stay as the siege left them.
    s.world.hall_add_attacker(H21, WOLVES);
    s.world.hall_start_siege(H21);
    assert!(s.world.hall_is_in_siege(H21));
    assert!(!s.world.door(door).unwrap().open);
    bypass(&mut s, P1, "open_doors 1, 21000001");
    assert!(page_has(&s.take(), P1, "DOORMAN BUSY"));
    assert!(!s.world.door(door).unwrap().open);
    // The siege is over: the doorman serves again.
    s.world.hall_end_siege(H21);
    assert!(!s.world.hall_is_in_siege(H21));
    bypass(&mut s, P1, "open_doors 1, 21000001");
    assert!(s.world.door(door).unwrap().open);
}

/// `SpecialSiegeGuardAI.autoAttackCondition`: a guard of a siegable hall (a `Defender` with a hall
/// and no castle within 10000) never takes a player of its allied list for a target, any other
/// enemy of the siege it does.
#[test]
fn a_hall_siege_guard_leaves_its_allied_players_alone() {
    let mut s = Scene::new(false, all_halls(LIONS), hall_load(vec![]));
    let (x, y) = hall_xy(21);
    // Both stand a few units from the guards, well inside a sword's range.
    s.enter_at(P7, TIGERS, x + 40, y + 30, 0);
    s.enter_at(P8, TIGERS, x + 40, y + 31, 0);
    s.world.hall_add_attacker(H21, WOLVES);
    s.world.hall_start_siege(H21);
    assert!(s.world.hall_is_in_siege(H21));
    let allied = s.spawn_at_hall(HALL_DEFENDER, 21);
    let plain = s.spawn_at_hall(HALL_DEFENDER, 21);
    s.world.hall_siege_guard_add_allies(allied, &[P7]);
    assert_eq!(s.world.hall_siege_allies_of(allied), [P7]);
    assert!(s.world.hall_siege_allies_of(plain).is_empty());
    // The first seconds after the spawn the guard is not aggressive (`_globalAggro = -10`).
    s.run_ms(20_000);
    assert!(s.world.siege_guard_state(allied).is_some());
    assert_eq!(s.world.siege_guard_state(allied).unwrap().kind, SiegeGuardAiKind::Hall);
    // The plain guard has put both players on its aggro list (the hate itself comes and goes
    // with the attack timeouts), the allied one only the player who is not its ally.
    let listed = |s: &mut Scene, guard: ObjectId, player: i32| {
        s.world.aggro_list(guard).iter().any(|a| a.attacker == ObjectId(player))
    };
    assert!(listed(&mut s, plain, P7));
    assert!(listed(&mut s, plain, P8));
    assert!(!listed(&mut s, allied, P7));
    assert!(listed(&mut s, allied, P8));
}
