#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Fortresses and fortress sieges (M7-FORT-01): loading the fort spawns and the sieges, the
//! registration of a clan, the countdown, the start and the end of a siege, the commanders and the
//! barracks, the combat flag, the change of owner, `FortUpdater`, the fortress NPCs and scripts,
//! the fortress windows and `//fortsiege`, with the `SiegeDatabaseWrites` switch (D-51) and the
//! re-read of the registrations (D-50).
//! Java: managers/{FortManager,FortSiegeManager,FortSiegeGuardManager}.java,
//! mechanics/siege/{Fort,FortSiege,CombatFlag,FortUpdater}.java,
//! entity/actor/instance/{FortCommander,FortDoorman,FortLogistics,FortManager}.java,
//! dist/game/data/scripts/ai/others/{FortressSiegeManager,FortressArcherCaptain},
//! dist/game/data/scripts/handlers/chat/commands/admin/AdminFortSiege.java. The expected values
//! come from the Java code named in each test.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, DoorTemplates, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates,
    KarmaLoss, MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates,
    SkillTrees, SpawnData, SpawnLoadOptions, StaticObjectTemplates, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    ExShowFortressInfo, ExShowFortressMapInfo, ExShowFortressSiegeInfo, NpcHtmlMessage, NpcSay, SystemMessage,
    SystemMessageParam,
};
use l2_protocol_game::{NpcStringId, ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::fort::{
    FortSettings, FortSiegeFortLoad, FortSiegeGuardRecord, FortSiegeLoad, FortSiegeRows, FortSiegeWrite,
    FortSpawnRecord, FortThen, LogisticsCommand,
};
use l2_world::residence::{FortRecord, FunctionRecord, ResidenceReadThen, ResidenceRereadRows, Writes};
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
<skill id="591" levels="1" name="Fortress Skill"><operateType>P</operateType></skill>
</list>"#;

const TREES: &str = r#"<list>
<skillTree type="pledgeSkillTree">
  <skill skillName="Fortress Skill" skillId="591" skillLevel="1" getLevel="4" residenceSkill="true">
    <socialClass>APPRENTICE</socialClass><residenceId>101</residenceId></skill>
</skillTree>
</list>"#;

const ADENA: i32 = 57;
/// The combat flag (`FortSiegeManager.isCombat`).
const FLAG_ITEM: i32 = 9819;
/// The blood oath reward of `FortLogistics`.
const BLOOD_OATH_ITEM: i32 = 9910;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="9819" type="Weapon" name="Combat flag"><set name="weapon_type" val="FLAG" /><set name="bodypart" val="lrhand" />
  <set name="weight" val="500" /><set name="is_dropable" val="false" /><set name="is_destroyable" val="false" />
  <set name="isForceEquip" val="true" /></item>
<item id="9910" type="EtcItem" name="Proof of Blood"><set name="is_stackable" val="true" /></item>
</list>"#;

const NPC_PEACE: i32 = 35_658;
const NPC_MERCHANT: i32 = 35_659;
const NPC_CAPTAIN_GUARD: i32 = 35_660;
const NPC_CAPTAIN_ARCHER: i32 = 35_661;
const NPC_LOGISTICS: i32 = 35_662;
const NPC_FOREMAN: i32 = 35_663;
const NPC_DOORMAN: i32 = 35_667;
const NPC_BOX: i32 = 35_665;
const NPC_COMMANDER_1: i32 = 35_683;
const NPC_COMMANDER_2: i32 = 35_677;
const NPC_COMMANDER_3: i32 = 35_680;
const NPC_ENVOY: i32 = 36_393;
const NPC_SIEGE_GUARD: i32 = 35_670;

fn npc(id: i32, npc_type: &str, name: &str) -> String {
    format!(
        r#"<npc id="{id}" type="{npc_type}" level="1" name="{name}">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="400.0" mpRegen="0.9" />
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
    let list = [
        npc(NPC_PEACE, "Folk", "Gatekeeper"),
        npc(NPC_MERCHANT, "Folk", "Suspicious Merchant"),
        npc(NPC_CAPTAIN_GUARD, "Folk", "Guard Captain"),
        npc(NPC_CAPTAIN_ARCHER, "Folk", "Archer Captain"),
        npc(NPC_LOGISTICS, "FortLogistics", "Logistics Officer"),
        npc(NPC_FOREMAN, "FortManager", "Foreman"),
        npc(NPC_DOORMAN, "FortDoorman", "Doorman"),
        npc(NPC_BOX, "Monster", "Supply Box"),
        npc(NPC_COMMANDER_1, "FortCommander", "Archer Commander"),
        npc(NPC_COMMANDER_2, "FortCommander", "Guard Commander"),
        npc(NPC_COMMANDER_3, "FortCommander", "Support Commander"),
        npc(NPC_ENVOY, "Folk", "Envoy"),
        npc(NPC_SIEGE_GUARD, "Defender", "Fortress Guard"),
    ]
    .join("\n");
    format!(r#"<?xml version="1.0"?><list>{list}</list>"#)
}

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
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
        item_id == ADENA || item_id == BLOOD_OATH_ITEM
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
        "fortsiege",
        "add_fortattacker",
        "list_fortsiege_clans",
        "clear_fortsiege_list",
        "spawn_fortdoors",
        "endfortsiege",
        "startfortsiege",
        "setfort",
        "removefort",
    ] {
        rights.add_command(command, 100, false);
    }
    rights
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
/// Lions (the owner of Shanty): P1 (leader) and P2; Wolves (the attackers): P3 (leader) and P4;
/// Bears: P5 (leader). P9 is a GM without a clan.
const P1: i32 = 0x2000_0001;
const P2: i32 = 0x2000_0002;
const P3: i32 = 0x2000_0003;
const P4: i32 = 0x2000_0004;
const P5: i32 = 0x2000_0005;
const GM: i32 = 0x2000_0009;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
const BEARS: i32 = 700;
const SHANTY: FortId = FortId(101);

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
    format!(
        r#"<?xml version="1.0"?>
<list enabled="true">
  <zone name="shanty_fortress_siege" type="SiegeZone" {}<stat name="fortId" val="101" /></zone>
  <zone name="shanty_fortress" type="FortZone" {}<stat name="fortId" val="101" /></zone>
  <zone name="gludio_castle_battlefield_territory" type="SiegeZone" {}<stat name="castleId" val="1" /></zone>
</list>"#,
        cuboid(X, Y, 3000),
        cuboid(X, Y, 1500),
        cuboid(X + 20_000, Y, 3000),
    )
}

const STATIC_OBJECTS: &str = r#"<list>
  <object id="18220500" name="Shanty_flagpole" type="3" x="-71338" y="258271" z="-3104" />
</list>"#;

fn door_xml(id: i32, extra: &str) -> String {
    format!(
        r#"<door id="{id}" name="door{id}" height="150" baseHpMax="1000" pos="{X},{Y},{Z}"
            node1="{},{}" node2="{},{}" node3="{},{}" node4="{},{}" nodeZ="{}" {extra}/>"#,
        X - 10,
        Y - 50,
        X + 10,
        Y - 50,
        X + 10,
        Y + 50,
        X - 10,
        Y + 50,
        Z - 10
    )
}

/// Pages the NPCs and scripts show, as `(path, text)`; a `%word%` the code replaces is in the text.
const PAGES: &[(&str, &str)] = &[
    ("data/html/fortress/foreman.htm", "<html><body>FOREMAN %objectId% %npcname%</body></html>"),
    ("data/html/fortress/foreman-no.htm", "<html><body>FOREMAN NO</body></html>"),
    ("data/html/fortress/foreman-busy.htm", "<html><body>FOREMAN BUSY</body></html>"),
    ("data/html/fortress/foreman-noprivs.htm", "<html><body>NOPRIVS</body></html>"),
    ("data/html/fortress/foreman-report.htm", "<html><body>REPORT %hr% %min%</body></html>"),
    (
        "data/html/fortress/foreman-castlereport.htm",
        "<html><body>CASTLEREPORT %hr% %min% %castle% %hr2% %min2%</body></html>",
    ),
    ("data/html/fortress/foreman-expel.htm", "<html><body>EXPEL</body></html>"),
    ("data/html/fortress/foreman-expeled.htm", "<html><body>EXPELED</body></html>"),
    ("data/html/fortress/foreman-opened.htm", "<html><body>OPENED</body></html>"),
    ("data/html/fortress/foreman-closed.htm", "<html><body>CLOSED</body></html>"),
    ("data/html/fortress/foreman-functions.htm", "<html><body>FUNCTIONS %xp_regen% %hp_regen% %mp_regen%</body></html>"),
    ("data/html/fortress/manage.htm", "<html><body>MANAGE</body></html>"),
    (
        "data/html/fortress/edit_recovery.htm",
        "<html><body>RECOVERY %hp_recovery% | %hp_period% | %exp_recovery% | %mp_recovery%</body></html>",
    ),
    (
        "data/html/fortress/functions-apply.htm",
        "<html><body>APPLY %name% %cost% %use% %apply%</body></html>",
    ),
    (
        "data/html/fortress/functions-apply_confirmed.htm",
        "<html><body>APPLY CONFIRMED</body></html>",
    ),
    ("data/html/fortress/functions-cancel.htm", "<html><body>CANCEL %apply%</body></html>"),
    (
        "data/html/fortress/functions-cancel_confirmed.htm",
        "<html><body>CANCEL CONFIRMED</body></html>",
    ),
    ("data/html/fortress/functions-used.htm", "<html><body>USED %val%</body></html>"),
    ("data/html/fortress/low_adena.htm", "<html><body>LOW ADENA</body></html>"),
    ("data/html/fortress/35658-d.htm", "<html><body>DOORS %npcname%</body></html>"),
    ("data/html/fortress/logistics.htm", "<html><body>LOGISTICS %clanname% %npcId%</body></html>"),
    ("data/html/fortress/logistics-rewards.htm", "<html><body>REWARDS %bloodoath%</body></html>"),
    ("data/html/fortress/logistics-blood.htm", "<html><body>BLOOD</body></html>"),
    ("data/html/fortress/logistics-noblood.htm", "<html><body>NOBLOOD</body></html>"),
    ("data/html/fortress/logistics-noprivs.htm", "<html><body>LOGISTICS NOPRIVS</body></html>"),
    ("data/html/fortress/logistics-supply.htm", "<html><body>SUPPLY</body></html>"),
    ("data/html/fortress/logistics-nosupply.htm", "<html><body>NOSUPPLY</body></html>"),
    ("data/html/fortress/logistics-siege.htm", "<html><body>SIEGE</body></html>"),
    ("data/html/fortress/logistics-supplylvl.htm", "<html><body>SUPPLYLVL %supplylvl%</body></html>"),
    ("data/html/fortress/logistics-1.htm", "<html><body>LOGISTICS ONE</body></html>"),
    ("data/html/doorman/35667.htm", "<html><body>DOORMAN OWNER %objectId%</body></html>"),
    ("data/html/doorman/35667-no.htm", "<html><body>DOORMAN NO</body></html>"),
    ("data/html/doorman/35667-busy.htm", "<html><body>DOORMAN BUSY</body></html>"),
    ("data/html/admin/forts.htm", "<html><body>FORTS %forts%</body></html>"),
    ("data/html/admin/fort.htm", "<html><body>FORT %fortName% %fortId%</body></html>"),
];

/// The pages of the two scripts (`FortressSiegeManager-NN.html` ... ).
fn script_pages() -> Vec<(String, String)> {
    let manager = "data/scripts/ai/others/FortressSiegeManager";
    let mut pages = vec![
        (format!("{manager}/FortressSiegeManager.html"), "<html><body>MERCHANT FREE</body></html>".to_owned()),
        (
            format!("{manager}/FortressSiegeManager-01.html"),
            "<html><body>MERCHANT OF %clanName% %objectId%</body></html>".to_owned(),
        ),
        (
            format!("{manager}/FortressSiegeManager-12.html"),
            "<html><body>OWNER PAGE %clanName%</body></html>".to_owned(),
        ),
    ];
    for n in [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 14, 15, 16, 17, 18, 19, 20, 21] {
        pages.push((
            format!("{manager}/FortressSiegeManager-{n:02}.html"),
            format!("<html><body>PAGE {n:02}</body></html>"),
        ));
    }
    let captain = "data/scripts/ai/others/FortressArcherCaptain";
    pages.push((
        format!("{captain}/FortressArcherCaptain.html"),
        "<html><body>CAPTAIN OWN</body></html>".to_owned(),
    ));
    pages.push((
        format!("{captain}/FortressArcherCaptain-01.html"),
        "<html><body>CAPTAIN OTHER</body></html>".to_owned(),
    ));
    pages
}

struct Scene {
    world: World,
    log: Log,
    /// What loading sent to the persistence task and the clients.
    loaded: Vec<Out>,
    _root: tempfile::TempDir,
}

/// Places: the peace NPC and the others stand around (X, Y) inside the siege zone.
fn at(dx: i32) -> (i32, i32, i32) {
    (X + dx, Y, Z)
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

fn clan_record(clan_id: i32, name: &str, level: i32, leader: i32, other: i32, adena: i64) -> ClanRecord {
    let warehouse = if adena > 0 {
        vec![ItemData {
            object_id: ObjectId(0x2000_1000 + clan_id),
            item_id: ADENA,
            count: adena,
            loc: "CLANWH".to_owned(),
            ..Default::default()
        }]
    } else {
        Vec::new()
    };
    ClanRecord {
        clan_id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level,
            leader_id: leader,
            ..ClanDataRecord::default()
        }),
        members: vec![member(leader, 1), member(other, 5)],
        warehouse,
        ..ClanRecord::default()
    }
}

fn commander_properties() -> BTreeMap<String, String> {
    [
        ("ShantyCommander1", format!("{},{},{},0,{NPC_COMMANDER_1}", X + 100, Y, Z)),
        ("ShantyCommander2", format!("{},{},{},0,{NPC_COMMANDER_2}", X + 200, Y, Z)),
        ("ShantyCommander3", format!("{},{},{},0,{NPC_COMMANDER_3}", X + 300, Y, Z)),
        ("ShantyFlag1", format!("{},{},{},{FLAG_ITEM}", X + 400, Y, Z)),
        ("ShantyFlag2", format!("{},{},{},{FLAG_ITEM}", X + 410, Y, Z)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect()
}

fn settings() -> FortSettings {
    let properties = commander_properties();
    FortSettings {
        siege: l2_world::fort::FortSiegeIni::from_properties(&properties),
        properties,
        ..FortSettings::default()
    }
}

fn shanty(owner: i32) -> FortRecord {
    FortRecord {
        id: 101,
        name: "Shanty".to_owned(),
        owner_id: owner,
        last_owned_time: if owner == 0 { 0 } else { now_ms() - 3_600_000 },
        fort_type: 0,
        ..FortRecord::default()
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn spawn_row(id: i32, npc_id: i32, dx: i32, spawn_type: i32, castle_id: i32) -> FortSpawnRecord {
    FortSpawnRecord {
        id,
        npc_id,
        x: X + dx,
        y: Y,
        z: Z,
        heading: 0,
        spawn_type,
        castle_id,
    }
}

/// The rows of `fort_spawnlist` and `fort_siege_guards` of Shanty in these tests.
fn sieges(clans: Vec<i32>) -> FortSiegeLoad {
    FortSiegeLoad {
        forts: vec![FortSiegeFortLoad {
            fort_id: 101,
            spawns: vec![
                spawn_row(1, NPC_PEACE, 10, 0, 0),
                spawn_row(2, NPC_MERCHANT, 20, 2, 0),
                spawn_row(3, NPC_CAPTAIN_GUARD, 30, 1, 0),
                spawn_row(4, NPC_CAPTAIN_ARCHER, 40, 1, 0),
                spawn_row(5, NPC_ENVOY, 50, 3, 1),
            ],
            siege_guards: vec![
                FortSiegeGuardRecord {
                    npc_id: NPC_SIEGE_GUARD,
                    x: X + 600,
                    y: Y,
                    z: Z,
                    heading: 0,
                    respawn_delay: 600,
                },
                FortSiegeGuardRecord {
                    npc_id: NPC_SIEGE_GUARD,
                    x: X + 700,
                    y: Y,
                    z: Z,
                    heading: 0,
                    respawn_delay: 0,
                },
            ],
            siege_clans: clans,
        }],
    }
}

impl Scene {
    /// A world with the clans (Lions level 5, Wolves level 5 and Bears level 3), the fortress
    /// Shanty owned by `owner`, and the rows of `sieges` loaded.
    fn new(siege_database_writes: bool, owner: i32, siege_clans: Vec<i32>, siege_date: i64) -> Self {
        let mut fort = shanty(owner);
        fort.siege_date = siege_date;
        Self::with(siege_database_writes, fort, siege_clans, settings())
    }

    fn with(siege_database_writes: bool, fort: FortRecord, siege_clans: Vec<i32>, settings: FortSettings) -> Self {
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
        write("data/zones/forts.xml", &zones_xml());
        write(
            "data/mapregion/talking_island_town.xml",
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
             <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
             <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n</region>\n</list>\n",
        );
        for (path, text) in PAGES {
            write(path, text);
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
            Arc::new(SkillTrees::from_xml(TREES).unwrap()),
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
            .spawn_static_objects(&StaticObjectTemplates::from_xml(STATIC_OBJECTS).unwrap(), &ids)
            .unwrap();
        let doors = DoorTemplates::from_xml(&format!(
            "<list>{}{}</list>",
            door_xml(18_220_001, r#"hp_showable="false""#),
            door_xml(18_220_002, "")
        ))
        .unwrap();
        world.spawn_doors(&doors, &ids).unwrap();
        world.set_residence_settings(ResidenceSettings {
            siege_database_writes,
            ..ResidenceSettings::default()
        });
        world.set_fort_settings(settings);
        world.load_clans(
            vec![
                clan_record(LIONS, "Lions", 5, P1, P2, 100_000),
                clan_record(WOLVES, "Wolves", 5, P3, P4, 100_000),
                {
                    // The Bears own the castle Gludio.
                    let mut bears = clan_record(BEARS, "Bears", 3, P5, 0x2000_0006, 0);
                    if let Some(data) = bears.data.as_mut() {
                        data.castle_id = 1;
                    }
                    bears
                },
            ],
            Vec::new(),
        );
        world.load_residences(ResidenceLoad {
            castles: vec![l2_world::residence::CastleRecord {
                id: 1,
                found: true,
                name: "Gludio".to_owned(),
                owner_id: BEARS,
                ..l2_world::residence::CastleRecord::default()
            }],
            forts: vec![fort],
            ..ResidenceLoad::default()
        });
        world.load_fort_data(sieges(siege_clans));
        let loaded = log.take();
        Self {
            world,
            log,
            loaded,
            _root: root,
        }
    }

    fn enter(&mut self, id: i32, clan_id: i32, access_level: i32) {
        self.enter_at(id, clan_id, X, access_level);
    }

    fn enter_at(&mut self, id: i32, clan_id: i32, x: i32, access_level: i32) {
        self.enter_with(pdata(id, clan_id, x, access_level));
    }

    fn enter_with(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.world.tick();
        // The client answers with `Appearing`: the player is in the world.
        self.appear(id);
    }

    fn take(&mut self) -> Vec<Out> {
        self.log.take()
    }

    /// The client's `Appearing` after a teleport.
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

    fn clan(&self, id: i32) -> &Clan {
        self.world.clan(id).unwrap()
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

    fn one_npc(&self, npc_id: i32) -> ObjectId {
        let found = self.npcs_of(npc_id);
        assert_eq!(found.len(), 1, "one NPC {npc_id}: {found:?}");
        found[0]
    }

    /// The bypass `npc_<id>_<command>`, after the player was near the NPC and clicked it (so the
    /// NPC is the last folk the player talked to).
    fn bypass(&mut self, player: i32, npc: ObjectId, command: &str) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input: ClientInput::Bypass {
                command: format!("npc_{}_{command}", npc.0),
            },
        });
    }

    fn script(&mut self, player: i32, command: &str) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input: ClientInput::Bypass {
                command: format!("Script {command}"),
            },
        });
    }

    /// Makes `npc` the last folk NPC of the player (the click that opened the window).
    fn talk(&mut self, player: i32, npc: ObjectId) {
        self.world.set_target(ObjectId(player), Some(npc));
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input: ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        });
        self.run_ms(100);
    }
}

fn pdata(id: i32, clan_id: i32, x: i32, access_level: i32) -> PlayerData {
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
        y: Y,
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

/// The ids of the system messages a player got.
fn message_ids(out: &[Out], to: i32) -> Vec<SystemMessageId> {
    messages_to(out, to).iter().map(|m| m.id).collect()
}

fn pages_to(out: &[Out], to: i32) -> Vec<String> {
    packets_to(out, to, "NPC_HTML_MESSAGE")
        .iter()
        .map(|b| decode::<NpcHtmlMessage>(b).html)
        .collect()
}

fn fort_writes(out: &[Out]) -> Vec<FortSiegeWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Residence(l2_world::ResidenceWrite::FortSiege(w))) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn residence_writes(out: &[Out]) -> Vec<l2_world::ResidenceWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Residence(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn reread_requests(out: &[Out]) -> Vec<ResidenceReadThen> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ResidenceRereadRequest { then, .. }) => Some(then.clone()),
            _ => None,
        })
        .collect()
}

/// A text param of a system message.
fn text_param(message: &SystemMessage, index: usize) -> String {
    match &message.params[index] {
        SystemMessageParam::Text(text) => text.clone(),
        other => panic!("not a text: {other:?}"),
    }
}

fn int_param(message: &SystemMessage, index: usize) -> i32 {
    match &message.params[index] {
        SystemMessageParam::IntNumber(n) => *n,
        SystemMessageParam::CastleName(n) => *n,
        SystemMessageParam::ItemName(n) => *n,
        other => panic!("not an int: {other:?}"),
    }
}

/// Answers a re-read request of the fortress with the rows `clans` and the owner's blood oaths.
fn answer_reread(s: &mut Scene, then: ResidenceReadThen, owner: i32, clans: Vec<i32>, blood: Option<i32>) {
    let rows = FortSiegeRows {
        fort: FortRecord {
            id: 101,
            name: "Shanty".to_owned(),
            owner_id: owner,
            last_owned_time: s.world.fort(SHANTY).unwrap().last_owned_time,
            state: s.world.fort(SHANTY).unwrap().state,
            castle_id: s.world.fort(SHANTY).unwrap().castle_id,
            supply_level: s.world.fort(SHANTY).unwrap().supply_level,
            siege_date: s.world.fort(SHANTY).unwrap().siege_date,
            ..FortRecord::default()
        },
        clans,
        owner_blood_oath_count: blood,
    };
    s.world.handle(WorldCommand::ResidenceReread {
        result: Ok(ResidenceRereadRows::FortSiege(Box::new(rows))),
        then,
    });
}

// ---------------------------------------------------------------------------------- loading

/// `new Fort(id)` and `new FortSiege(fort)`: the NPCs that always stand there and the NPC
/// commanders (the captains) spawn, the suspicious merchant and the envoys do not (the envoys only
/// after the fortress is taken, `_state == 0`), the siege guards are loaded and not spawned.
#[test]
fn loading_spawns_the_peace_time_npcs_like_java() {
    let s = Scene::new(true, 0, vec![], 0);
    assert_eq!(s.npcs_of(NPC_PEACE).len(), 1);
    assert_eq!(s.npcs_of(NPC_CAPTAIN_GUARD).len(), 1);
    assert_eq!(s.npcs_of(NPC_CAPTAIN_ARCHER).len(), 1);
    // `checkAutoTask` with the date in the past: the merchant spawns at once.
    assert_eq!(s.npcs_of(NPC_MERCHANT).len(), 1);
    assert!(s.npcs_of(NPC_ENVOY).is_empty(), "no owner, no envoys");
    assert!(s.npcs_of(NPC_SIEGE_GUARD).is_empty());
    assert!(s.npcs_of(NPC_COMMANDER_1).is_empty());
    // The commanders of the siege come from FortSiege.ini.
    assert!(!s.world.fort_siege_in_progress(SHANTY));
    // The envoys of a fortress that has an owner and no contract.
    let s = Scene::new(true, LIONS, vec![], 0);
    assert_eq!(s.npcs_of(NPC_ENVOY).len(), 1);
    // A contract was decided: no envoys.
    assert_eq!(s.world.fort_castle_id_by_ambassador(SHANTY, NPC_ENVOY), 1);
    assert_eq!(s.world.fort_castle_id_by_ambassador(SHANTY, 1), -1);
    assert!(!s.world.fort_is_border_fortress(SHANTY));
}

/// `FortSiege.checkAutoTask`: a siege time in the past clears the registrations and the date
/// (`saveFortSiege`, `clearSiegeClan`); the rows go out with the switch only (D-51).
#[test]
fn a_siege_time_in_the_past_clears_the_registrations_and_writes_with_the_switch() {
    let on = Scene::new(true, LIONS, vec![WOLVES], 0);
    assert!(on.world.fort_attacker_clans(SHANTY).is_empty());
    assert_eq!(
        fort_writes(&on.loaded),
        [
            FortSiegeWrite::SiegeDate {
                fort_id: 101,
                siege_date: 0
            },
            // The owner's own registrations go too.
            FortSiegeWrite::ClearSiegeClans {
                fort_id: 101,
                owner_clan_id: Some(LIONS)
            },
        ]
    );
    let off = Scene::new(false, LIONS, vec![WOLVES], 0);
    assert!(off.world.fort_attacker_clans(SHANTY).is_empty(), "cleared in this world");
    assert!(fort_writes(&off.loaded).is_empty(), "but nothing is written without the switch");
}

/// `checkAutoTask` with a date ahead and a registered clan: the registration stays, the countdown
/// is armed, and the suspicious merchant stands there; without a registration the merchant
/// waits for the date.
#[test]
fn a_registered_clan_keeps_its_registration_for_a_date_ahead() {
    let date = now_ms() + 20 * 60_000;
    let s = Scene::new(true, LIONS, vec![WOLVES], date);
    assert_eq!(s.world.fort_attacker_clans(SHANTY), [WOLVES]);
    assert!(fort_writes(&s.loaded).is_empty());
    // Delay over 10 minutes: "spawn suspicious merchant".
    assert_eq!(s.npcs_of(NPC_MERCHANT).len(), 1);
    let s = Scene::new(true, LIONS, vec![], date);
    assert!(s.npcs_of(NPC_MERCHANT).is_empty());
}

// ---------------------------------------------------------------------------------- registration

fn adena_of(s: &Scene, player: i32) -> i64 {
    s.world.adena_of(ObjectId(player))
}

/// `FortSiege.addAttacker(player, true)`: 0 without a clan, 1 when the first clan cannot pay
/// 250,000 adena, 4 on success (the first clan pays, the next ones do not), 3 for a clan that is
/// registered already or an owner whose fortress waits for a siege; the registration row, the
/// siege date (an hour from now) and the warning to the owner go out; the countdown is armed.
#[test]
fn registering_follows_javas_codes_and_charges_the_first_clan() {
    let mut s = Scene::new(true, LIONS, vec![], 0);
    s.take();
    s.enter(P1, LIONS, 0);
    s.enter(P3, WOLVES, 0);
    s.enter(P5, BEARS, 0);
    s.enter(GM, 0, 100);
    let mut poor = pdata(P4, WOLVES, X, 0);
    poor.items[0].count = 1000;
    s.enter_with(poor);
    s.take();

    // No clan.
    assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(GM), true), 0);
    // The first clan needs 250,000 adena.
    assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(P4), true), 1);
    assert!(s.world.fort_attacker_clans(SHANTY).is_empty());

    let before = adena_of(&s, P3);
    assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(P3), true), 4);
    assert_eq!(adena_of(&s, P3), before - 250_000);
    assert_eq!(s.world.fort_attacker_clans(SHANTY), [WOLVES]);
    let out = s.take();
    let writes = fort_writes(&out);
    assert_eq!(writes.len(), 2);
    assert_eq!(
        writes[0],
        FortSiegeWrite::AddAttacker {
            fort_id: 101,
            clan_id: WOLVES
        }
    );
    // `setSiegeDateTime(false)`: an hour from now.
    let FortSiegeWrite::SiegeDate { fort_id: 101, siege_date } = writes[1] else {
        panic!("{writes:?}");
    };
    let expected = now_ms() + 3_600_000;
    assert!((siege_date - expected).abs() < 60_000, "{siege_date} vs {expected}");
    assert_eq!(s.world.fort(SHANTY).unwrap().siege_date, siege_date);
    // The owner is told ("A fortress is under attack").
    assert!(message_ids(&out, P1).contains(&SystemMessageId::AFortressIsUnderAttack));

    // The second clan does not pay.
    let before = adena_of(&s, P5);
    assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(P5), true), 4);
    assert_eq!(adena_of(&s, P5), before);
    assert_eq!(s.world.fort_attacker_clans(SHANTY), [WOLVES, BEARS]);
    // Registered already.
    assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(P3), true), 3);
    // The owner of a fortress that waits for a siege cannot register.
    assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(P1), true), 3);
}

/// `AttackerMaxClans`: a clan beyond the limit is not added, and the registration still answers 4
/// (Java's `saveSiegeClan` returns quietly).
#[test]
fn the_attacker_limit_is_quiet() {
    let mut settings = settings();
    settings.siege.attacker_max_clans = 1;
    let mut s = Scene::with(true, shanty(LIONS), vec![], settings);
    s.enter(P3, WOLVES, 0);
    s.enter(P5, BEARS, 0);
    assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(P3), true), 4);
    assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(P5), true), 4);
    assert_eq!(s.world.fort_attacker_clans(SHANTY), [WOLVES]);
}

/// D-51: without the switch a registration lives in this world only, and nothing is written.
#[test]
fn a_registration_without_the_switch_writes_nothing() {
    let mut s = Scene::new(false, LIONS, vec![], 0);
    s.enter(P3, WOLVES, 0);
    s.take();
    assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(P3), true), 4);
    assert_eq!(s.world.fort_attacker_clans(SHANTY), [WOLVES]);
    assert!(fort_writes(&s.take()).is_empty());
    // `removeAttacker` of a clan that is registered.
    s.world.fort_remove_attacker(SHANTY, WOLVES);
    assert!(s.world.fort_attacker_clans(SHANTY).is_empty());
    assert!(fort_writes(&s.take()).is_empty());
}

/// `removeAttacker(clan)`: the row goes; with no clan left the date is cleared (`saveFortSiege`);
/// the clan of the fortress itself is not removed (`clan.getFortId() == getResidenceId()`).
#[test]
fn removing_the_last_attacker_clears_the_date() {
    let mut s = Scene::new(true, LIONS, vec![], 0);
    s.enter(P3, WOLVES, 0);
    s.world.fort_add_attacker(SHANTY, ObjectId(P3), true);
    s.take();
    s.world.fort_remove_attacker(SHANTY, LIONS);
    assert_eq!(s.world.fort_attacker_clans(SHANTY), [WOLVES]);
    s.world.fort_remove_attacker(SHANTY, WOLVES);
    assert!(s.world.fort_attacker_clans(SHANTY).is_empty());
    assert_eq!(
        fort_writes(&s.take()),
        [
            FortSiegeWrite::RemoveAttackers {
                fort_id: 101,
                clan_id: WOLVES
            },
            FortSiegeWrite::SiegeDate {
                fort_id: 101,
                siege_date: 0
            },
        ]
    );
}

/// `ClanTable.destroyClan` removes the clan from the fortress sieges (`fortId == 0`).
#[test]
fn a_dissolved_clan_leaves_the_fortress_sieges() {
    let mut s = Scene::new(true, LIONS, vec![], 0);
    s.enter(P3, WOLVES, 0);
    s.world.fort_add_attacker(SHANTY, ObjectId(P3), true);
    assert!(s.world.fort_clan_is_registered(WOLVES));
    s.take();
    s.world.destroy_clan(WOLVES, true);
    assert!(!s.world.fort_clan_is_registered(WOLVES));
    assert!(s.world.fort_attacker_clans(SHANTY).is_empty());
    let writes = fort_writes(&s.take());
    assert_eq!(
        writes[0],
        FortSiegeWrite::RemoveAttackers {
            fort_id: 101,
            clan_id: WOLVES
        }
    );
}

// ---------------------------------------------------------------------------------- the siege

const TICKS_PER_MINUTE: u32 = 1200;

/// Whether a player stands near the restart point of the map region (`MaxOffsetOnTeleport`).
fn in_town(s: &Scene, player: i32) -> bool {
    let (x, y, _) = s.world.position_of(ObjectId(player)).unwrap();
    (x + 84_318).abs() <= 60 && (y - 244_579).abs() <= 60
}

fn run_ticks(s: &mut Scene, ticks: u32) {
    for _ in 0..ticks {
        s.world.tick();
    }
}

/// `ScheduleStartSiegeTask`: an hour, 10, 5 and 1 minute and 30, 10, 5 and 1 second before the
/// start the attackers and the owner hear it, the merchant goes away 10 minutes before, and at
/// 0 the siege starts. The chain runs with `SiegeDatabaseWrites` only (D-51).
#[test]
fn the_countdown_announces_and_starts_the_siege() {
    let mut s = Scene::new(true, LIONS, vec![], 0);
    s.enter(P1, LIONS, 0);
    s.enter(P3, WOLVES, 0);
    s.enter(P5, BEARS, 0);
    assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(P3), true), 4);
    s.take();
    // The merchant stands there until 10 minutes before.
    assert_eq!(s.npcs_of(NPC_MERCHANT).len(), 1);
    run_ticks(&mut s, 60 * TICKS_PER_MINUTE - 50);
    let out = s.take();
    assert!(!s.world.fort_siege_in_progress(SHANTY));
    let minutes: Vec<i32> = messages_to(&out, P3)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::S1MinuteSUntilTheFortressBattleStarts)
        .map(|m| int_param(&m, 0))
        .collect();
    assert_eq!(minutes, [10, 5, 1]);
    assert!(s.npcs_of(NPC_MERCHANT).is_empty(), "the merchant leaves 10 minutes before");
    // The owner hears it too, the bystander does not.
    assert!(
        message_ids(&out, P1).contains(&SystemMessageId::S1MinuteSUntilTheFortressBattleStarts),
        "{:?}",
        message_ids(&out, P1)
    );
    assert!(message_ids(&out, P5).is_empty());
    run_ticks(&mut s, 100);
    let later = s.take();
    let mut all: Vec<(SystemMessageId, i32)> = Vec::new();
    for out in [&out, &later] {
        all.extend(
            messages_to(out, P3)
                .iter()
                .filter(|m| !m.params.is_empty() && m.id != SystemMessageId::S1MinuteSUntilTheFortressBattleStarts)
                .map(|m| (m.id, int_param(m, 0))),
        );
    }
    use SystemMessageId::S1SecondSUntilTheFortressBattleStarts as Sec;
    assert_eq!(
        all,
        [
            (Sec, 30),
            (Sec, 10),
            (Sec, 5),
            (Sec, 1),
            (SystemMessageId::TheFortressBattleS1HasBegun, 101)
        ]
    );
    assert!(s.world.fort_siege_in_progress(SHANTY));
}

/// D-51: the countdown does nothing without the switch.
#[test]
fn the_countdown_does_not_run_without_the_switch() {
    let mut s = Scene::new(false, LIONS, vec![], 0);
    s.enter(P3, WOLVES, 0);
    s.world.fort_add_attacker(SHANTY, ObjectId(P3), true);
    s.take();
    run_ticks(&mut s, 75 * TICKS_PER_MINUTE);
    assert!(!s.world.fort_siege_in_progress(SHANTY));
    assert!(messages_to(&s.take(), P3).is_empty());
    // A GM starts a test siege, which runs here whatever the switch says.
    s.world.fort_start_siege(SHANTY);
    assert!(s.world.fort_siege_in_progress(SHANTY));
}

/// A scene with the Wolves registered and the siege started by hand.
fn siege_scene() -> Scene {
    let date = now_ms() + 30 * 60_000;
    let mut s = Scene::new(true, LIONS, vec![WOLVES], date);
    s.enter(P1, LIONS, 0);
    s.enter(P4, WOLVES, 0);
    s.enter(P5, BEARS, 0);
    s.take();
    s
}

/// `startSiege()`: the zone turns active, the attackers inside go to town, the siege states are
/// set, the commanders of the siege and the guards appear, the captains of peace time and the
/// merchant go, the flag comes down, the message and the date go out.
#[test]
fn starting_a_siege_sets_up_the_fortress() {
    let mut s = siege_scene();
    // The attacker leader stands in the fortress when the siege starts.
    s.enter(P3, WOLVES, 0);
    s.take();
    assert_eq!(s.npcs_of(NPC_MERCHANT).len(), 1);
    s.world.fort_start_siege(SHANTY);
    s.run_ms(200);
    let out = s.take();
    assert!(s.world.fort_siege_in_progress(SHANTY));
    assert!(s.world.fort_zone_is_active(SHANTY));
    // Siege states.
    assert_eq!(s.world.fort_siege_state_of(ObjectId(P3)), 1);
    assert_eq!(s.world.fort_siege_state_of(ObjectId(P4)), 1);
    assert_eq!(s.world.fort_siege_state_of(ObjectId(P1)), 2);
    assert_eq!(s.world.fort_siege_state_of(ObjectId(P5)), 0);
    // The attackers inside the zone go to the nearest town.
    assert!(in_town(&s, P3), "P3 was brought to town");
    // The owner's members inside the zone are marked as in the siege.
    assert!(s.world.fort_player_in_siege(ObjectId(P1)));
    // The commanders of the siege, the guards; no captains and no merchant.
    assert_eq!(s.npcs_of(NPC_COMMANDER_1).len(), 1);
    assert_eq!(s.npcs_of(NPC_COMMANDER_2).len(), 1);
    assert_eq!(s.npcs_of(NPC_COMMANDER_3).len(), 1);
    assert_eq!(s.world.fort_commanders_alive(SHANTY), 3);
    assert_eq!(s.npcs_of(NPC_SIEGE_GUARD).len(), 2);
    assert!(s.npcs_of(NPC_CAPTAIN_GUARD).is_empty());
    assert!(s.npcs_of(NPC_CAPTAIN_ARCHER).is_empty());
    assert!(s.npcs_of(NPC_MERCHANT).is_empty());
    // The flag pole shows no flag.
    let pole = s.world.fort(SHANTY).unwrap().flag_pole.unwrap();
    assert_eq!(s.world.static_object(pole).unwrap().mesh_index, 0);
    // "The fortress battle of S1 has begun": the participants only.
    for p in [P1, P4] {
        let begun: Vec<SystemMessage> = messages_to(&out, p)
            .into_iter()
            .filter(|m| m.id == SystemMessageId::TheFortressBattleS1HasBegun)
            .collect();
        assert_eq!(begun.len(), 1, "P{p:x}");
        assert_eq!(int_param(&begun[0], 0), 101);
    }
    assert!(!message_ids(&out, P5).contains(&SystemMessageId::TheFortressBattleS1HasBegun));
    // The bystander in the zone is told it is a combat zone.
    assert!(message_ids(&out, P5).contains(&SystemMessageId::YouHaveEnteredACombatZone));
    assert!(s.world.player(ObjectId(P5)).unwrap().zones.is_inside(ZoneId::Siege));
    // saveFortSiege: the date goes to 0.
    assert!(fort_writes(&out).contains(&FortSiegeWrite::SiegeDate {
        fort_id: 101,
        siege_date: 0
    }));
    // A second start does nothing.
    s.world.fort_start_siege(SHANTY);
    assert_eq!(s.npcs_of(NPC_COMMANDER_1).len(), 1);
}

/// A siege without attackers does not start.
#[test]
fn a_siege_without_attackers_does_not_start() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    s.world.fort_start_siege(SHANTY);
    assert!(!s.world.fort_siege_in_progress(SHANTY));
    assert!(!s.world.fort_zone_is_active(SHANTY));
}

/// `ScheduleEndSiegeTask`: after `SiegeLength` minutes the siege ends; the rows of the
/// registrations go, the date is set to the merchant's return, the commanders and guards go, the
/// captains come back, outsiders are brought to town, the states clear and the flag shows again.
#[test]
fn the_siege_ends_after_its_length() {
    let mut s = siege_scene();
    s.world.fort_start_siege(SHANTY);
    s.take();
    run_ticks(&mut s, 60 * TICKS_PER_MINUTE - 20);
    assert!(s.world.fort_siege_in_progress(SHANTY));
    run_ticks(&mut s, 40);
    let out = s.take();
    assert!(!s.world.fort_siege_in_progress(SHANTY));
    assert!(!s.world.fort_zone_is_active(SHANTY));
    assert_eq!(s.world.fort_siege_state_of(ObjectId(P4)), 0);
    assert_eq!(s.world.fort_siege_state_of(ObjectId(P1)), 0);
    assert!(s.world.fort_attacker_clans(SHANTY).is_empty());
    assert!(s.npcs_of(NPC_COMMANDER_1).is_empty());
    assert_eq!(s.npcs_of(NPC_CAPTAIN_GUARD).len(), 1);
    assert_eq!(s.npcs_of(NPC_CAPTAIN_ARCHER).len(), 1);
    // The guards were killed (`doDie`), not deleted.
    for guard in s.npcs_of(NPC_SIEGE_GUARD) {
        assert!(s.world.is_dead(guard));
    }
    // The bystander is brought to town, the owner stays.
    s.appear(P5);
    assert!(in_town(&s, P5));
    assert_eq!(s.world.position_of(ObjectId(P1)).unwrap().0, X);
    assert!(message_ids(&out, P5).contains(&SystemMessageId::YouHaveLeftACombatZone));
    let pole = s.world.fort(SHANTY).unwrap().flag_pole.unwrap();
    assert_eq!(s.world.static_object(pole).unwrap().mesh_index, 1);
    // Rows: the date to 0, the registrations (the owner's too), the merchant's return.
    let writes = fort_writes(&out);
    assert_eq!(
        writes[..2],
        [
            FortSiegeWrite::SiegeDate {
                fort_id: 101,
                siege_date: 0
            },
            FortSiegeWrite::ClearSiegeClans {
                fort_id: 101,
                owner_clan_id: Some(LIONS)
            },
        ]
    );
    let FortSiegeWrite::SiegeDate { siege_date, .. } = writes[2] else {
        panic!("{writes:?}");
    };
    let expected = now_ms() + 180 * 60_000;
    assert!((siege_date - expected).abs() < 60_000);
}

/// The suspicious merchant comes back `SuspiciousMerchantRespawnDelay` minutes after a siege.
#[test]
fn the_merchant_returns_after_the_siege() {
    let mut settings = settings();
    settings.siege.suspicious_merchant_respawn_delay = 2;
    let date = now_ms() + 30 * 60_000;
    let mut fort = shanty(LIONS);
    fort.siege_date = date;
    let mut s = Scene::with(true, fort, vec![WOLVES], settings);
    s.world.fort_start_siege(SHANTY);
    s.world.fort_end_siege(SHANTY);
    assert!(s.npcs_of(NPC_MERCHANT).is_empty());
    run_ticks(&mut s, 2 * TICKS_PER_MINUTE + 5);
    assert_eq!(s.npcs_of(NPC_MERCHANT).len(), 1);
}
