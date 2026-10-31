#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The castle NPCs (M7-NPC-01): the chamberlain, the siege manager, the mercenary manager and the
//! mercenary captain, the vault keeper, the mass gatekeeper, the blacksmith, the court magician,
//! the fortress envoys and the castle doormen, with the re-read of the castle before the
//! chamberlain acts (D-50) and the `SiegeDatabaseWrites` switch (D-51).
//! Java: dist/game/data/scripts/ai/others/{CastleChamberlain,CastleSiegeManager,
//! CastleMercenaryManager,CastleWarehouse,CastleTeleporter,CastleBlacksmith,CastleCourtMagician,
//! CastleAmbassador,MercenaryCaptain}/*.java, entity/actor/instance/{CastleDoorman,Doorman}.java.
//! The expected values come from the Java code named in each test.

use std::collections::BTreeMap;
use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, DoorTemplates, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache,
    ItemTemplates, KarmaLoss, MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions,
    SkillTemplates, SkillTrees, SpawnData, SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::siege::SiegeInfo;
use l2_protocol_game::server::territory::ExShowDominionRegistry;
use l2_protocol_game::server::{NpcHtmlMessage, NpcSay, SystemMessage};
use l2_protocol_game::{NpcStringId, ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::fort::{FortSettings, FortSiegeFortLoad, FortSiegeLoad, FortSpawnRecord};
use l2_world::residence::npc::{CastleScript, ResidenceNpcSettings};
use l2_world::residence::{CastleRecord, FortRecord, FunctionRecord, ResidenceReadThen, ResidenceRereadRows};
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

/// Wind Walk (2 levels, 50 and 100 mana) and Decrease Weight (a level that costs more mana than
/// the chamberlain has).
const SKILLS: &str = r##"<list>
<skill id="4342" levels="2" name="Wind Walk"><table name="#mpConsume">50 100</table>
  <operateType>A2</operateType><targetType>ONE</targetType><castRange>400</castRange>
  <isMagic>1</isMagic><magicLevel>1</magicLevel><mpConsume>#mpConsume</mpConsume></skill>
<skill id="4343" levels="3" name="Decrease Weight"><table name="#mpConsume">10 20 1000</table>
  <operateType>A2</operateType><targetType>ONE</targetType><castRange>400</castRange>
  <isMagic>1</isMagic><magicLevel>1</magicLevel><mpConsume>#mpConsume</mpConsume></skill>
</list>"##;

const ADENA: i32 = 57;
const CROWN: i32 = 6841;
const BLOOD_OATH: i32 = 9910;
const BLOOD_ALLIANCE: i32 = 9911;
const EPAULETTE: i32 = 9912;
const STRIDER_WIND: i32 = 4422;
const BADGE: i32 = 13757;

/// Every talisman the court magician can give is an item of the test.
fn talisman_ids() -> Vec<i32> {
    (9914..=9965).chain(10416..=10424).chain(10518..=10519).collect()
}

fn items_xml() -> String {
    let mut ids = vec![
        (ADENA, "Adena"),
        (CROWN, "Lord's Crown"),
        (BLOOD_OATH, "Proof of Blood"),
        (BLOOD_ALLIANCE, "Proof of Alliance"),
        (EPAULETTE, "Knight's Epaulette"),
        (STRIDER_WIND, "Wind Strider"),
        (BADGE, "Territory Badge"),
    ];
    let talismans: Vec<(i32, String)> = talisman_ids()
        .into_iter()
        .map(|id| (id, format!("Talisman {id}")))
        .collect();
    let mut text = String::from("<list>");
    for (id, name) in ids.drain(..) {
        let stackable = matches!(id, ADENA | BLOOD_OATH | BLOOD_ALLIANCE | EPAULETTE | BADGE);
        text.push_str(&format!(
            r#"<item id="{id}" type="EtcItem" name="{name}"><set name="is_stackable" val="{stackable}" /></item>"#
        ));
    }
    for (id, name) in talismans {
        text.push_str(&format!(r#"<item id="{id}" type="EtcItem" name="{name}" />"#));
    }
    text.push_str("</list>");
    text
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
            weapon: false,
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
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
        matches!(item_id, ADENA | BLOOD_OATH | BLOOD_ALLIANCE | EPAULETTE | BADGE)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.1)
    }
}

const X: i32 = -18_000;
const Y: i32 = 110_000;
const Z: i32 = -2_500;
/// The fortress stands far from the castle.
const FX: i32 = X + 30_000;
/// Lions own the castle Gludio: P1 (leader) and P2; Wolves own the fortress Shanty: P3 (leader);
/// Bears have nothing: P5 (leader). P9 is a GM without a clan.
const P1: i32 = 0x6000_0001;
const P2: i32 = 0x6000_0002;
const P3: i32 = 0x6000_0003;
const P5: i32 = 0x6000_0005;
const GM: i32 = 0x6000_0009;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
const BEARS: i32 = 700;
const GLUDIO: CastleId = CastleId(1);
const SHANTY: FortId = FortId(101);
const DOOR: i32 = 19_210_001;
const DOOR_2: i32 = 19_210_002;

const CHAMBERLAIN: i32 = 35_100;
const SIEGE_MANAGER: i32 = 35_104;
const MERC_MANAGER: i32 = 35_102;
const WAREHOUSE: i32 = 35_099;
const TELEPORTER: i32 = 35_095;
const BLACKSMITH: i32 = 35_098;
const MAGICIAN: i32 = 35_648;
const DOORMAN: i32 = 35_097;
const CAPTAIN: i32 = 36_481;
const ENVOY: i32 = 36_393;
const FLAME_TOWER: i32 = 13_004;

fn npc(id: i32, npc_type: &str, name: &str) -> String {
    format!(
        r#"<npc id="{id}" type="{npc_type}" level="70" name="{name}">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="500.0" mpRegen="0.0" />
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
        npc(CHAMBERLAIN, "Merchant", "Sayres"),
        npc(SIEGE_MANAGER, "Folk", "Tyron"),
        npc(MERC_MANAGER, "Merchant", "Greenspan"),
        npc(WAREHOUSE, "Warehouse", "Warehouse Keeper"),
        npc(TELEPORTER, "Folk", "Mass Gatekeeper"),
        npc(BLACKSMITH, "Folk", "Blacksmith"),
        npc(MAGICIAN, "Folk", "Court Magician"),
        npc(DOORMAN, "CastleDoorman", "Inner Doorman"),
        npc(CAPTAIN, "Folk", "Mercenary Captain"),
        npc(ENVOY, "Folk", "Special Envoy"),
        npc(FLAME_TOWER, "FlameTower", "Flame Control Tower"),
    ]
    .join("\n");
    format!(r#"<?xml version="1.0"?><list>{list}</list>"#)
}

fn zones_xml() -> String {
    let cuboid = |x: i32, w: i32| {
        format!(
            r#"shape="Cuboid" minZ="-5000" maxZ="5000"><node X="{}" Y="{}" /><node X="{}" Y="{}" />"#,
            x - w,
            Y - w,
            x + w,
            Y + w
        )
    };
    format!(
        r#"<?xml version="1.0"?>
<list enabled="true">
  <zone name="gludio_castle_battlefield_territory" type="SiegeZone" {}<stat name="castleId" val="1" /></zone>
  <zone name="shanty_fortress_siege" type="SiegeZone" {}<stat name="fortId" val="101" /></zone>
  <zone name="shanty_fortress" type="FortZone" {}<stat name="fortId" val="101" /></zone>
</list>"#,
        cuboid(X, 3000),
        cuboid(FX, 3000),
        cuboid(FX, 1500),
    )
}

const TELEPORTERS: &str = r#"<?xml version="1.0"?><list>
  <npc id="35100">
    <teleport type="OTHER">
      <location name="Out" x="-17000" y="111000" z="-2500" />
      <location name="In" x="-17100" y="111100" z="-2500" />
    </teleport>
  </npc>
  <npc id="35097">
    <teleport type="OTHER">
      <location name="Out of the gate" x="-17500" y="111500" z="-2500" />
      <location name="Into the gate" x="-17600" y="111600" z="-2500" />
    </teleport>
  </npc>
</list>"#;

/// The pages of the scripts; the body of each is its own name unless it says more.
fn script_pages() -> Vec<(String, String, String)> {
    let chamberlain: Vec<&str> = vec![
        "chamberlain-01",
        "chamberlain-02",
        "chamberlain-03",
        "chamberlain-04",
        "chamberlain-05",
        "chamberlain-06",
        "chamberlain-07",
        "chamberlain-08",
        "chamberlain-09",
        "chamberlain-10",
        "chamberlain-11",
        "chamberlain-12",
        "chamberlain-13",
        "chamberlain-14",
        "chamberlain-15",
        "chamberlain-16",
        "chamberlain-17",
        "chamberlain-17a",
        "chamberlain-18",
        "chamberlain-19",
        "chamberlain-20",
        "chamberlain-21",
        "chamberlain-22",
        "chamberlain-23",
        "chamberlain-24",
        "chamberlain-25",
        "chamberlain-26",
        "chamberlain-27",
        "chamberlain-28",
        "35100-d",
        "35100-du",
        "35100-tu",
        "35100-t1",
        "castleaftersettaxrate",
        "castlesettaxrate",
        "castletoohightaxrate",
        "castlemanagevault",
        "castlenotenoughbalance",
        "castledeco-AR01",
        "castledeco-AE01",
        "castledeco-01",
        "castledeco-02",
        "castledeco-03",
        "castledeco-04",
        "castledeco-05",
        "castleresetdeco",
        "castledecoalreadyset",
        "castledecofunction",
        "castlefuncdisabled",
        "castletdecomanage",
        "castlebuff-05",
        "castleafterbuff",
        "castlenotenoughmp",
        "manor",
        "manor-help-01",
        "ssq_notdawnorevent",
    ];
    let mut pages: Vec<(String, String, String)> = chamberlain
        .into_iter()
        .map(|n| ("CastleChamberlain".to_owned(), format!("{n}.html"), n.to_owned()))
        .collect();
    // Pages with placeholders.
    let mut set = |script: &str, file: &str, body: &str| {
        pages.retain(|(s, f, _)| !(s == script && f == file));
        pages.push((script.to_owned(), file.to_owned(), body.to_owned()));
    };
    set(
        "CastleChamberlain",
        "castlesettaxrate.html",
        "SETTAX %tax_rate% %next_tax_rate% %tax_limit%",
    );
    set(
        "CastleChamberlain",
        "chamberlain-03.html",
        "TAXVIEW %tax_rate% %next_tax_rate%",
    );
    set(
        "CastleChamberlain",
        "castleaftersettaxrate.html",
        "AFTERTAX %next_tax_rate%",
    );
    set(
        "CastleChamberlain",
        "castletoohightaxrate.html",
        "TOOHIGH %tax_limit%",
    );
    set(
        "CastleChamberlain",
        "castlemanagevault.html",
        "VAULT %tax_income% %tax_income_reserved% %seed_income%",
    );
    set(
        "CastleChamberlain",
        "castlenotenoughbalance.html",
        "BALANCE %tax_income% %withdraw_amount%",
    );
    set(
        "CastleChamberlain",
        "chamberlain-13.html",
        "DOORS type=%type% doors=[%doors%]",
    );
    set(
        "CastleChamberlain",
        "chamberlain-14.html",
        "PRICE %gate_price% EVENT=%event%",
    );
    set(
        "CastleChamberlain",
        "chamberlain-15.html",
        "DOORLEVEL %doorlevel%",
    );
    set(
        "CastleChamberlain",
        "chamberlain-18.html",
        "TRAP %trapIndex% %level% %dmgzone_price%",
    );
    set("CastleChamberlain", "chamberlain-19.html", "DMGLEVEL %dmglevel%");
    set(
        "CastleChamberlain",
        "chamberlain-02.html",
        "REPORT %clanleadername% %clanname% %castlename% %ss_event% %ss_avarice% %ss_gnosis% %ss_strife%",
    );
    set(
        "CastleChamberlain",
        "chamberlain-25.html",
        "CROWN %owner_name% %feud_name%",
    );
    set("CastleChamberlain", "chamberlain-28.html", "FORTS %list%");
    set(
        "CastleChamberlain",
        "castledeco-AR01.html",
        "AR %HPDepth%|%HPCost%|%HPExpire%|%HPReset%|%MPDepth%|%XPDepth%",
    );
    set(
        "CastleChamberlain",
        "castledeco-AE01.html",
        "AE %TPDepth%|%BFDepth%|%BFReset%",
    );
    set(
        "CastleChamberlain",
        "castledeco-02.html",
        "DECO2 %AgitDecoCost% %AgitDecoEffect% %AgitDecoSubmit%",
    );
    set(
        "CastleChamberlain",
        "castleresetdeco.html",
        "RESET %AgitDecoSubmit%",
    );
    set(
        "CastleChamberlain",
        "castledecoalreadyset.html",
        "ALREADY %AgitDecoEffect%",
    );
    set(
        "CastleChamberlain",
        "castledecofunction.html",
        "FUNCS %HPDepth% %MPDepth% %XPDepth%",
    );
    set("CastleChamberlain", "castlebuff-05.html", "BUFF mp=%MPLeft%");
    set(
        "CastleChamberlain",
        "castleafterbuff.html",
        "AFTERBUFF mp=%MPLeft%",
    );
    set("CastleChamberlain", "castlenotenoughmp.html", "NOMP mp=%MPLeft%");
    set("CastleChamberlain", "chamberlain-22.html", "PRODUCTS %npcId%");
    set(
        "CastleChamberlain",
        "chamberlain-01.html",
        "HOME %objectId% %playername%",
    );
    pages.extend(
        [
            ("CastleSiegeManager", "CastleSiegeManager.html"),
            ("CastleSiegeManager", "CastleSiegeManager-01.html"),
            ("CastleSiegeManager", "CastleSiegeManager-02.html"),
            ("CastleMercenaryManager", "mercmanager.html"),
            ("CastleMercenaryManager", "mercmanager-no.html"),
            ("CastleMercenaryManager", "mercmanager-siege.html"),
            ("CastleMercenaryManager", "mercmanager-dusk.html"),
            ("CastleMercenaryManager", "mercmanager-dawn.html"),
            ("CastleMercenaryManager", "mercmanager-limit.html"),
            ("CastleMercenaryManager", "mercmanager-aden-limit.html"),
            ("CastleMercenaryManager", "mercmanager-01.html"),
            ("CastleWarehouse", "warehouse-01.html"),
            ("CastleWarehouse", "warehouse-02.html"),
            ("CastleWarehouse", "warehouse-no.html"),
            ("CastleWarehouse", "warehouse-05.html"),
            ("CastleWarehouse", "warehouse-06.html"),
            ("CastleWarehouse", "warehouse-07.html"),
            ("CastleWarehouse", "warehouse-08.html"),
            ("CastleTeleporter", "teleporter-01.html"),
            ("CastleTeleporter", "teleporter-02.html"),
            ("CastleTeleporter", "teleporter-03.html"),
            ("CastleBlacksmith", "35098-01.html"),
            ("CastleBlacksmith", "35098-02.html"),
            ("CastleBlacksmith", "no.html"),
            ("CastleCourtMagician", "courtmagician.html"),
            ("CastleCourtMagician", "courtmagician-01.html"),
            ("CastleCourtMagician", "courtmagician-02.html"),
            ("CastleCourtMagician", "courtmagician-03.html"),
            ("CastleCourtMagician", "courtmagician-04.html"),
            ("CastleCourtMagician", "courtmagician-05.html"),
            ("CastleCourtMagician", "courtmagician-06.html"),
            ("MercenaryCaptain", "36481-01.html"),
            ("MercenaryCaptain", "36481-02.html"),
            ("MercenaryCaptain", "36481-04.html"),
            ("MercenaryCaptain", "36481-05.html"),
            ("MercenaryCaptain", "36481-06.html"),
            ("MercenaryCaptain", "36481-07.html"),
            ("MercenaryCaptain", "36481-08.html"),
            ("MercenaryCaptain", "36481-09.html"),
            ("MercenaryCaptain", "36481-10.html"),
        ]
        .map(|(s, f)| (s.to_owned(), f.to_owned(), f.trim_end_matches(".html").to_owned())),
    );
    pages.push((
        "CastleMercenaryManager".to_owned(),
        "mercmanager-limit.html".to_owned(),
        "LIMIT %feud_name%".to_owned(),
    ));
    pages.push((
        "CastleWarehouse".to_owned(),
        "warehouse-04.html".to_owned(),
        "<html><body>BLOOD %blood% %objectId%</body></html>".to_owned(),
    ));
    pages.push((
        "MercenaryCaptain".to_owned(),
        "36481-03.html".to_owned(),
        "STRIDERS %strider% %gstrider%".to_owned(),
    ));
    for f in [
        "ambassador",
        "ambassador-01",
        "ambassador-02",
        "ambassador-03",
        "ambassador-04",
        "ambassador-05",
    ] {
        pages.push((
            "CastleAmbassador".to_owned(),
            format!("{f}.html"),
            format!("{} castle=%castleName%", f.to_uppercase()),
        ));
    }
    pages
}

const DOORMAN_PAGES: [(&str, &str); 3] = [
    ("data/html/doorman/35097.htm", "DOORMAN OWNER %objectId%"),
    ("data/html/doorman/35097-no.htm", "DOORMAN NO %objectId%"),
    ("data/html/doorman/35097-busy.htm", "DOORMAN BUSY"),
];

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

struct Scene {
    world: World,
    log: Log,
    /// The NPCs by template id.
    npcs: BTreeMap<i32, ObjectId>,
    _root: tempfile::TempDir,
}

fn member(id: i32, power_grade: i32) -> ClanMemberRecord {
    ClanMemberRecord {
        object_id: id,
        name: format!("P{}", id - 0x6000_0000),
        level: 40,
        power_grade,
        ..ClanMemberRecord::default()
    }
}

fn clan(clan_id: i32, name: &str, castle_id: i32, leader: i32, others: &[i32]) -> ClanRecord {
    let mut members = vec![member(leader, 1)];
    members.extend(others.iter().map(|m| member(*m, 5)));
    ClanRecord {
        clan_id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level: 5,
            castle_id,
            leader_id: leader,
            ..ClanDataRecord::default()
        }),
        members,
        ..ClanRecord::default()
    }
}

fn pdata(id: i32, clan_id: i32, items: Vec<(i32, i64)>) -> PlayerData {
    let mut data = PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{}", id - 0x6000_0000),
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
        access_level: 0,
        x: X + 50,
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
        items: items
            .into_iter()
            .enumerate()
            .map(|(n, (item_id, count))| ItemData {
                object_id: ObjectId(id + 0x0100_0000 + i32::try_from(n).unwrap()),
                item_id,
                count,
                loc: "INVENTORY".to_owned(),
                ..Default::default()
            })
            .collect(),
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

impl Scene {
    fn new(siege_database_writes: bool) -> Self {
        Self::build(siege_database_writes, 0)
    }

    /// The Lions have `blood_alliance` proofs of alliance to receive.
    fn build(siege_database_writes: bool, blood_alliance: i32) -> Self {
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
        write("data/zones/castle.xml", &zones_xml());
        // A bypass is only accepted when a page the player was shown has it, so every page
        // carries a link that takes any event of its script (`$e` is a variable parameter).
        for (script, file, body) in script_pages() {
            let link = format!(r#"<a action="bypass -h Script {script} $e">x</a>"#);
            let body = if body.starts_with("<html") {
                body.replace("</body>", &format!("{link}</body>"))
            } else {
                format!("<html><body>{body}{link}</body></html>")
            };
            write(&format!("data/scripts/ai/others/{script}/{file}"), &body);
        }
        let doorman_links = ["open_doors", "close_doors", "tele"]
            .map(|c| format!(r#"<a action="bypass -h npc_%objectId%_{c} $e">x</a>"#))
            .concat();
        for (path, body) in DOORMAN_PAGES {
            write(path, &format!("<html><body>{body}{doorman_links}</body></html>"));
        }
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Recorder(log.clone())));
        let mut rights = AdminRights::default();
        rights.add_level(100, 0);
        rights.add_level(0, 0);
        world.set_static_data(Box::new(Data(stat_data(), rights)));
        world.set_exp_data(
            Arc::new(ExperienceTable::from_xml(r#"<table maxLevel="85" maxPetLevel="85"></table>"#).unwrap()),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
        );
        let ids = Arc::new(IdFactory::new(vec![P1, P2, P3, P5, GM]).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        let templates = Arc::new(NpcTemplates::from_xml(&npc_xml(), &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(
            root.path(),
            &templates,
            &SpawnLoadOptions::default(),
        ));
        world.set_npc_data(templates, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_teleporter_data(Arc::new(l2_data::TeleporterData::from_xml(TELEPORTERS).unwrap()));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
        let door = |id: i32, dx: i32| {
            format!(
                r#"<door id="{id}" name="gate{id}" height="150" baseHpMax="1000" pos="{},{},{Z}"
            node1="{},{}" node2="{},{}" node3="{},{}" node4="{},{}" nodeZ="{}" />"#,
                X + dx,
                Y,
                X + dx - 10,
                Y - 50,
                X + dx + 10,
                Y - 50,
                X + dx + 10,
                Y + 50,
                X + dx - 10,
                Y + 50,
                Z - 10
            )
        };
        let doors =
            DoorTemplates::from_xml(&format!("<list>{}{}</list>", door(DOOR, 500), door(DOOR_2, 600)))
                .unwrap();
        world.spawn_doors(&doors, &ids).unwrap();
        // The flag pole `loadFlagPoles` looks for.
        let pole = format!(
            r#"<list><object id="18220500" name="Shanty_flagpole" type="3" x="{FX}" y="{Y}" z="{Z}" /></list>"#
        );
        world
            .spawn_static_objects(&l2_data::StaticObjectTemplates::from_xml(&pole).unwrap(), &ids)
            .unwrap();
        world.set_residence_settings(ResidenceSettings {
            siege_database_writes,
            ..ResidenceSettings::default()
        });
        world.set_fort_settings(FortSettings::default());
        world.load_clans(
            vec![
                {
                    let mut lions = clan(LIONS, "Lions", 1, P1, &[P2]);
                    if let Some(data) = lions.data.as_mut() {
                        data.blood_alliance_count = blood_alliance;
                    }
                    lions
                },
                clan(WOLVES, "Wolves", 0, P3, &[]),
                clan(BEARS, "Bears", 0, P5, &[]),
            ],
            Vec::new(),
        );
        world.load_residences(ResidenceLoad {
            castles: vec![CastleRecord {
                id: 1,
                found: true,
                name: "Gludio".to_owned(),
                owner_id: LIONS,
                tax_percent: 7,
                treasury: 1_234_567,
                time_registration_over: true,
                ..CastleRecord::default()
            }],
            forts: vec![FortRecord {
                id: 101,
                name: "Shanty".to_owned(),
                owner_id: WOLVES,
                last_owned_time: now_ms() - 3_600_000,
                ..FortRecord::default()
            }],
            ..ResidenceLoad::default()
        });
        // The manor lists of the castles (the vault page sums the seed sales).
        world.load_manor(ManorLoad::default());
        let mut siege = SiegeSettings::default();
        siege.castles.insert(
            1,
            CastleSiegeConfig {
                control_towers: vec![],
                flame_towers: vec![TowerSpawn {
                    npc_id: FLAME_TOWER,
                    x: X + 200,
                    y: Y,
                    z: Z,
                    zone_list: vec![],
                    upgrade_level: 0,
                }],
                max_mercenaries: 100,
            },
        );
        world.set_siege_settings(siege);
        world.load_sieges(SiegeLoad::default());
        // The envoy of Gludio stands at Shanty, which has an owner and no decision.
        world.load_fort_data(FortSiegeLoad {
            forts: vec![FortSiegeFortLoad {
                fort_id: 101,
                spawns: vec![FortSpawnRecord {
                    id: 1,
                    npc_id: ENVOY,
                    x: FX + 10,
                    y: Y,
                    z: Z,
                    heading: 0,
                    spawn_type: 3,
                    castle_id: 1,
                }],
                ..FortSiegeFortLoad::default()
            }],
        });
        world.set_residence_npc_settings(ResidenceNpcSettings::default());
        let mut npcs = BTreeMap::new();
        for (npc_id, dx) in [
            (CHAMBERLAIN, 0),
            (SIEGE_MANAGER, 5),
            (MERC_MANAGER, 10),
            (WAREHOUSE, 15),
            (TELEPORTER, 20),
            (BLACKSMITH, 25),
            (MAGICIAN, 30),
            (DOORMAN, 35),
            (CAPTAIN, 40),
        ] {
            let id = world.spawn_npc_at(npc_id, X + dx, Y, Z, 0).unwrap();
            npcs.insert(npc_id, id);
        }
        let envoy = world
            .npc_ids()
            .into_iter()
            .find(|id| world.npc(*id).is_some_and(|n| n.npc_id == ENVOY))
            .expect("the envoy spawned at the fortress");
        npcs.insert(ENVOY, envoy);
        log.take();
        Self {
            world,
            log,
            npcs,
            _root: root,
        }
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.world.tick();
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input: ClientInput::Appearing,
        });
        self.run_ms(100);
    }

    /// The Lions' leader with 10 million adena, a Lions member, a Wolves leader and a GM.
    fn enter_all(&mut self) {
        self.enter(pdata(P1, LIONS, vec![(ADENA, 10_000_000), (EPAULETTE, 10)]));
        self.enter(pdata(P2, LIONS, vec![(ADENA, 10_000_000), (EPAULETTE, 5)]));
        let mut wolves = pdata(P3, WOLVES, vec![(ADENA, 1000)]);
        wolves.x = FX;
        self.enter(wolves);
        self.enter(pdata(P5, BEARS, vec![(ADENA, 1000)]));
        let mut gm = pdata(GM, 0, vec![(ADENA, 1000)]);
        gm.access_level = 100;
        self.enter(gm);
        self.log.take();
    }

    fn run_ms(&mut self, ms: u64) {
        for _ in 0..ms.div_ceil(50) {
            self.world.tick();
        }
    }

    fn take(&mut self) -> Vec<Out> {
        self.log.take()
    }

    fn npc(&self, npc_id: i32) -> ObjectId {
        self.npcs[&npc_id]
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    /// The click that opens the NPC's window (it becomes the player's last folk).
    fn talk(&mut self, player: i32, npc_id: i32) {
        let npc = self.npc(npc_id);
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

    /// `bypass -h Script <name> <event>`.
    fn script(&mut self, player: i32, name: &str, event: &str) {
        self.input(
            player,
            ClientInput::Bypass {
                command: format!("Script {name} {event}"),
            },
        );
    }

    fn chamberlain(&mut self, player: i32, event: &str) {
        self.script(player, "CastleChamberlain", event);
    }

    /// The bypass `npc_<id>_<command>`.
    fn npc_bypass(&mut self, player: i32, npc_id: i32, command: &str) {
        let npc = self.npc(npc_id);
        self.input(
            player,
            ClientInput::Bypass {
                command: format!("npc_{}_{command}", npc.0),
            },
        );
    }

    fn castle(&self) -> &l2_world::residence::Castle {
        self.world.castle(GLUDIO).unwrap()
    }

    /// Answers the last castle re-read the world asked for with the castle as it is now, after
    /// `change` (Sieghardt changed something).
    fn answer_castle(&mut self, out: &[Out], change: impl FnOnce(&mut CastleRecord)) {
        let then = reread_requests(out).pop().expect("a re-read request");
        let castle = self.castle().clone();
        let mut record = CastleRecord {
            id: 1,
            found: true,
            name: castle.name.clone(),
            tax_percent: castle.tax_percent,
            treasury: castle.treasury,
            siege_date: castle.siege_date,
            time_registration_over: castle.time_registration_over,
            ticket_buy_count: castle.ticket_buy_count,
            owner_id: castle.owner_id,
            functions: self
                .world
                .residence_functions(ResidenceRef::Castle(GLUDIO))
                .into_iter()
                .map(|f| FunctionRecord {
                    function_type: f.function_type,
                    lvl: f.lvl,
                    lease: f.lease,
                    rate: f.rate,
                    end_time: f.end_time,
                })
                .collect(),
            ..CastleRecord::default()
        };
        change(&mut record);
        self.world.handle(WorldCommand::ResidenceReread {
            result: Ok(ResidenceRereadRows::Castle(record)),
            then,
        });
    }

    /// A vault keeper command; "Receive" and the count page read the castle again first, and the
    /// answer carries the owner's proofs of alliance as `blood` (D-50).
    fn warehouse(&mut self, player: i32, event: &str, blood: Option<i32>) {
        self.take();
        self.script(player, "CastleWarehouse", event);
        let asked = self.take();
        if !reread_requests(&asked).is_empty() {
            self.answer_castle(&asked, |c| c.owner_blood_alliance_count = blood);
        }
    }

    /// A chamberlain command and the re-read it asks for, answered with the unchanged castle.
    fn chamberlain_fresh(&mut self, player: i32, event: &str) -> Vec<Out> {
        self.take();
        self.chamberlain(player, event);
        let asked = self.take();
        self.answer_castle(&asked, |_| {});
        let mut out = asked;
        out.extend(self.take());
        out
    }

    /// A siege that is called off without attackers never starts: the Bears attack.
    fn start_siege(&mut self) {
        self.world.siege_register_attacker(GLUDIO, ObjectId(P5), true);
        self.world.start_siege(GLUDIO);
        assert!(self.world.is_castle_siege_in_progress(GLUDIO));
        self.take();
    }

    fn close_door(&mut self, door_id: i32) {
        let object = self.world.door_by_door_id(door_id).unwrap().object_id;
        self.world.close_door(object);
    }

    fn adena(&self, player: i32) -> i64 {
        item_count(self, player, ADENA)
    }
}

fn item_count(s: &Scene, player: i32, item_id: i32) -> i64 {
    s.world
        .player(ObjectId(player))
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == item_id)
        .map(|i| i.count)
        .sum()
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

fn pages_to(out: &[Out], to: i32) -> Vec<String> {
    packets_to(out, to, "NPC_HTML_MESSAGE")
        .iter()
        .map(|b| decode::<NpcHtmlMessage>(b).html)
        .collect()
}

/// The text between the tags of the last page: `<html><body>TEXT</body></html>`.
#[track_caller]
fn last_page(out: &[Out], to: i32) -> String {
    let Some(page) = pages_to(out, to).pop() else {
        panic!("no page to {to:#x}");
    };
    let text = page
        .trim_start_matches("<html><body>")
        .trim_end_matches("</body></html>");
    // Without the link every fixture page carries.
    text.split("<a action").next().unwrap().to_owned()
}

fn message_ids(out: &[Out], to: i32) -> Vec<SystemMessageId> {
    packets_to(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|b| decode::<SystemMessage>(b).id)
        .collect()
}

fn residence_writes(out: &[Out]) -> Vec<ResidenceWrite> {
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

// ---------------------------------------------------------------------------------- scripts

/// The scripts know their NPCs by template id (`addFirstTalkId`).
#[test]
fn every_npc_id_belongs_to_one_script() {
    assert_eq!(CastleScript::of_npc_id(35100), Some(CastleScript::Chamberlain));
    assert_eq!(CastleScript::of_npc_id(35639), Some(CastleScript::SiegeManager));
    assert_eq!(CastleScript::of_npc_id(35420), Some(CastleScript::SiegeManager));
    assert_eq!(CastleScript::of_npc_id(35555), Some(CastleScript::Chamberlain));
    assert_eq!(CastleScript::of_npc_id(35547), Some(CastleScript::Teleporter));
    assert_eq!(CastleScript::of_npc_id(36455), Some(CastleScript::Ambassador));
    assert_eq!(
        CastleScript::of_npc_id(36489),
        Some(CastleScript::MercenaryCaptain)
    );
    assert_eq!(CastleScript::of_npc_id(35656), Some(CastleScript::CourtMagician));
    assert_eq!(CastleScript::of_npc_id(35097), None);
    assert_eq!(
        CastleScript::by_name("CastleBlacksmith"),
        Some(CastleScript::Blacksmith)
    );
    assert_eq!(CastleScript::by_name("castleblacksmith"), None);
}

/// Every `Script <Name> <event>` link of the real pages names an event the port handles (the
/// switch labels of the Java scripts). Skipped without the datapack.
#[test]
fn the_links_of_the_real_pages_are_events_of_the_scripts() {
    let root = std::path::Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/scripts/ai/others"
    ));
    if !root.exists() {
        eprintln!("skipped: the datapack is not here");
        return;
    }
    let known: BTreeMap<&str, Vec<&str>> = BTreeMap::from([
        (
            "CastleChamberlain",
            vec![
                "chamberlain-01.html",
                "manor-help-01.html",
                "manor-help-02.html",
                "manor-help-03.html",
                "manor-help-04.html",
                "fort_status",
                "siege_functions",
                "manage_doors",
                "upgrade_doors",
                "upgrade_doors_confirm",
                "manage_trap",
                "upgrade_trap",
                "upgrade_trap_confirm",
                "receive_report",
                "manage_tax",
                "set_tax",
                "manage_vault",
                "deposit",
                "withdraw",
                "manage_functions",
                "banish_foreigner_show",
                "banish_foreigner",
                "doors",
                "operate_door",
                "additional_functions",
                "recovery",
                "other",
                "HP",
                "MP",
                "XP",
                "TP",
                "BF",
                "set_func",
                "functions",
                "teleport",
                "goto",
                "buffer",
                "cast_buff",
                "list_siege_clans",
                "list_territory_clans",
                "manor",
                "products",
                "buy",
                "give_crown",
                "manors_cert",
                "manors_cert_confirm",
            ],
        ),
        (
            "CastleMercenaryManager",
            vec!["limit", "buy", "main", "mercmanager-01.html"],
        ),
        (
            "CastleWarehouse",
            vec![
                "warehouse-01.html",
                "warehouse-02.html",
                "warehouse-03.html",
                "warehouse-04.html",
                "Receive",
                "Exchange",
            ],
        ),
        ("CastleTeleporter", vec!["TELEPORT_START"]),
        (
            "CastleBlacksmith",
            vec![
                "35098-02.html",
                "35140-02.html",
                "35182-02.html",
                "35224-02.html",
                "35272-02.html",
                "35314-02.html",
                "35361-02.html",
                "35507-02.html",
                "35553-02.html",
            ],
        ),
        (
            "CastleCourtMagician",
            vec![
                "courtmagician.html",
                "courtmagician-03.html",
                "giveTalisman",
                "squadSkill",
                "clanTeleport",
            ],
        ),
        ("CastleAmbassador", vec!["signed", "rejected"]),
        (
            "MercenaryCaptain",
            vec![
                "36481-02.html",
                "36481-03.html",
                "territory",
                "strider",
                "elite",
                "top-elite",
            ],
        ),
    ]);
    let mut checked = 0;
    for entry in std::fs::read_dir(root).unwrap().flatten() {
        let script = entry.file_name().to_string_lossy().into_owned();
        if !known.contains_key(script.as_str()) {
            continue;
        }
        for page in std::fs::read_dir(entry.path()).unwrap().flatten() {
            if page.path().extension().is_none_or(|e| e != "html") {
                continue;
            }
            let text = std::fs::read_to_string(page.path()).unwrap();
            for part in text.split("bypass").skip(1) {
                let Some(link) = part
                    .trim_start_matches(" -h")
                    .trim_start()
                    .strip_prefix("Script ")
                else {
                    continue;
                };
                let link = link.split('"').next().unwrap();
                let mut words = link.split_whitespace();
                let name = words.next().unwrap();
                let Some(event) = words.next() else { continue };
                assert!(
                    known.get(name).is_some_and(|events| events.contains(&event)),
                    "{script}/{}: {link}",
                    page.file_name().to_string_lossy()
                );
                checked += 1;
            }
        }
    }
    assert!(checked > 100, "{checked} links");
}

// ---------------------------------------------------------------------------------- chamberlain

/// `onFirstTalk`: the owner clan (and a GM) get the lord's page, everybody else the stranger's;
/// `showResult` replaces `%objectId%` and `%playername%` and ends with an `ActionFailed`; the NPC
/// is a start NPC, so it becomes the quest NPC.
#[test]
fn the_chamberlain_greets_the_owners_and_strangers() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P2, CHAMBERLAIN);
    let out = s.take();
    let npc = s.npc(CHAMBERLAIN);
    assert_eq!(last_page(&out, P2), format!("HOME {} P2", npc.0));
    // `showResult` ends with an `ActionFailed`.
    assert!(!packets_to(&out, P2, "ACTION_FAIL").is_empty());
    assert_eq!(
        s.world.player(ObjectId(P2)).unwrap().talk.last_quest_npc_object,
        npc.0
    );
    s.talk(P5, CHAMBERLAIN);
    assert_eq!(last_page(&s.take(), P5), "chamberlain-04");
    s.talk(GM, CHAMBERLAIN);
    assert_eq!(last_page(&s.take(), GM), format!("HOME {} P9", npc.0));
}

/// A link of the page that needs no row of the castle answers at once; the rest asks for the
/// castle to be read again first (D-50) and does nothing until the answer; a failed read refuses
/// the command.
#[test]
fn the_chamberlain_reads_the_castle_again_before_it_acts() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    s.take();
    s.chamberlain(P1, "chamberlain-01.html");
    let out = s.take();
    assert!(reread_requests(&out).is_empty());
    assert_eq!(pages_to(&out, P1).len(), 1);
    s.chamberlain(P1, "manage_tax");
    let out = s.take();
    assert!(pages_to(&out, P1).is_empty());
    assert_eq!(reread_requests(&out).len(), 1);
    // A failed read refuses the command.
    let then = reread_requests(&out).pop().unwrap();
    s.world.handle(WorldCommand::ResidenceReread {
        result: Err(()),
        then,
    });
    assert!(pages_to(&s.take(), P1).is_empty());
    // Sieghardt gave the castle away in the meantime: the old lord is a stranger.
    s.chamberlain(P1, "manage_tax");
    let asked = s.take();
    s.answer_castle(&asked, |c| c.owner_id = 0);
    let out = s.take();
    assert_eq!(last_page(&out, P1), "chamberlain-21");
    assert_eq!(s.castle().owner_id, 0);
}

/// `manage_tax` and `set_tax`: the page shows the rate of the castle and the limit; the new rate
/// is written (a player's action: always), a rate over the limit is refused with the limit.
#[test]
fn the_tax_rate_is_shown_set_and_limited() {
    let mut s = Scene::new(false);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    let out = s.chamberlain_fresh(P1, "manage_tax");
    assert_eq!(last_page(&out, P1), "SETTAX 7 0 15");
    // The tax rate of a fresh read replaces the one in memory.
    s.chamberlain(P1, "manage_tax");
    let asked = s.take();
    s.answer_castle(&asked, |c| c.tax_percent = 9);
    assert_eq!(last_page(&s.take(), P1), "SETTAX 9 0 15");
    let out = s.chamberlain_fresh(P1, "set_tax 16");
    assert_eq!(last_page(&out, P1), "TOOHIGH 15");
    assert_eq!(s.castle().tax_percent, 9);
    let out = s.chamberlain_fresh(P1, "set_tax 12");
    assert_eq!(last_page(&out, P1), "AFTERTAX 12");
    assert_eq!(s.castle().tax_percent, 12);
    assert!(
        residence_writes(&out).contains(&ResidenceWrite::CastleTaxPercent {
            castle_id: 1,
            tax_percent: 12
        })
    );
    // No number means 0; a member without the vault right only sees the rate.
    let out = s.chamberlain_fresh(P1, "set_tax");
    assert_eq!(last_page(&out, P1), "AFTERTAX 0");
    s.talk(P2, CHAMBERLAIN);
    let out = s.chamberlain_fresh(P2, "manage_tax");
    assert_eq!(last_page(&out, P2), "TAXVIEW 0 0");
    let out = s.chamberlain_fresh(P2, "set_tax 3");
    assert_eq!(last_page(&out, P2), "chamberlain-21");
    assert_eq!(s.castle().tax_percent, 0);
}

/// `deposit`, `withdraw` and `manage_vault`: the treasury moves with the player's adena, the
/// rows are written whatever the switch says (the lord's own action), and Java's quirks stay.
#[test]
fn the_treasury_takes_deposits_and_gives_withdrawals() {
    for switch in [false, true] {
        let mut s = Scene::new(switch);
        s.enter_all();
        s.talk(P1, CHAMBERLAIN);
        let out = s.chamberlain_fresh(P1, "manage_vault");
        assert_eq!(last_page(&out, P1), "VAULT 1,234,567 0 0");
        let out = s.chamberlain_fresh(P1, "deposit 1000");
        assert_eq!(s.adena(P1), 9_999_000);
        assert_eq!(s.castle().treasury, 1_235_567);
        // The script answers with "chamberlain-01.html".
        assert_eq!(last_page(&out, P1), format!("HOME {} P1", s.npc(CHAMBERLAIN).0));
        assert!(residence_writes(&out).contains(&ResidenceWrite::CastleTreasury {
            castle_id: 1,
            treasury: 1_235_567
        }));
        let out = s.chamberlain_fresh(P1, "withdraw 500000");
        assert_eq!(s.adena(P1), 10_499_000);
        assert_eq!(s.castle().treasury, 735_567);
        assert!(residence_writes(&out).contains(&ResidenceWrite::CastleTreasury {
            castle_id: 1,
            treasury: 735_567
        }));
        // More than the treasury holds.
        let out = s.chamberlain_fresh(P1, "withdraw 99999999");
        assert_eq!(last_page(&out, P1), "BALANCE 735,567 99,999,999");
        assert_eq!(s.castle().treasury, 735_567);
        // Not enough adena for the deposit.
        let out = s.chamberlain_fresh(P1, "deposit 20000000");
        assert!(message_ids(&out, P1).contains(&SystemMessageId::YouDoNotHaveEnoughAdena));
        assert_eq!(s.castle().treasury, 735_567);
        // NOTE(java-quirk): a negative withdrawal puts adena into the treasury for nothing.
        s.chamberlain_fresh(P1, "withdraw -1000");
        assert_eq!(s.castle().treasury, 736_567);
        assert_eq!(s.adena(P1), 10_499_000);
        // The member without the vault right is sent away.
        s.talk(P2, CHAMBERLAIN);
        let out = s.chamberlain_fresh(P2, "withdraw 1");
        assert_eq!(last_page(&out, P2), "chamberlain-21");
    }
}

/// A siege in progress closes the pages that change the castle ("chamberlain-08").
#[test]
fn nothing_is_managed_during_a_siege() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    s.start_siege();
    for event in [
        "manage_tax",
        "set_tax 5",
        "manage_functions",
        "doors",
        "banish_foreigner",
        "operate_door 1 19210001",
    ] {
        let out = s.chamberlain_fresh(P1, event);
        assert_eq!(last_page(&out, P1), "chamberlain-08", "{event}");
    }
    let out = s.chamberlain_fresh(P1, "receive_report");
    assert_eq!(last_page(&out, P1), "chamberlain-07");
}

/// `manage_doors`, `upgrade_doors` and `upgrade_doors_confirm`: the price page, the purchase
/// (adena, both doors, the row), the refusal for a level not above the current one, the lack of
/// adena.
#[test]
fn the_doors_are_upgraded_for_adena() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    let out = s.chamberlain_fresh(P1, "manage_doors");
    assert_eq!(last_page(&out, P1), "35100-du");
    let out = s.chamberlain_fresh(P1, "manage_doors 1 19210001 19210002");
    assert_eq!(last_page(&out, P1), "DOORS type=1 doors=[ 19210001 19210002]");
    let out = s.chamberlain_fresh(P1, "upgrade_doors 1 2 19210001 19210002");
    assert_eq!(last_page(&out, P1), "PRICE 3000000 EVENT=1 2 19210001 19210002");
    let out = s.chamberlain_fresh(P1, "upgrade_doors_confirm 1 2 19210001 19210002");
    assert_eq!(last_page(&out, P1), "chamberlain-16");
    assert_eq!(s.adena(P1), 7_000_000);
    for door in [DOOR, DOOR_2] {
        assert_eq!(s.world.door_by_door_id(door).unwrap().upgrade_hp_ratio, 2);
        assert!(
            residence_writes(&out).contains(&ResidenceWrite::CastleDoorUpgrade {
                door_id: door,
                ratio: 2,
                castle_id: 1
            })
        );
    }
    // The same level again: "doorlevel" is the current one.
    let out = s.chamberlain_fresh(P1, "upgrade_doors_confirm 1 2 19210001 19210002");
    assert_eq!(last_page(&out, P1), "DOORLEVEL 2");
    assert_eq!(s.adena(P1), 7_000_000);
    // Level 5 of the outer gate costs 5 million; two million adena is not enough.
    s.world
        .destroy_item_by_item_id(ObjectId(P1), ADENA, 5_000_000, false);
    let out = s.chamberlain_fresh(P1, "upgrade_doors_confirm 1 5 19210001 19210002");
    assert_eq!(last_page(&out, P1), "chamberlain-09");
    // NOTE(java-quirk): four door ids throw before anything is taken.
    let before = s.adena(P1);
    s.chamberlain_fresh(P1, "upgrade_doors_confirm 1 3 19210001 19210002 1 2");
    assert_eq!(s.adena(P1), before);
}

/// `manage_trap`, `upgrade_trap` and `upgrade_trap_confirm`: the flame tower's level and the row.
#[test]
fn the_traps_are_upgraded_for_adena() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    assert_eq!(last_page(&s.chamberlain_fresh(P1, "manage_trap"), P1), "35100-tu");
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "manage_trap 0"), P1),
        "chamberlain-17"
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "upgrade_trap 0 2"), P1),
        "TRAP 0 2 4000000"
    );
    let out = s.chamberlain_fresh(P1, "upgrade_trap_confirm 0 2");
    assert_eq!(last_page(&out, P1), "chamberlain-20");
    assert_eq!(s.adena(P1), 6_000_000);
    assert_eq!(s.world.castle_trap_upgrade_level(GLUDIO, 0), 2);
    assert!(
        residence_writes(&out).contains(&ResidenceWrite::CastleTrapUpgrade {
            castle_id: 1,
            tower_index: 0,
            level: 2
        })
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "upgrade_trap_confirm 0 1"), P1),
        "DMGLEVEL 2"
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "upgrade_trap_confirm 0 4"), P1),
        "chamberlain-20"
    );
    assert_eq!(s.adena(P1), 0);
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "upgrade_trap_confirm 0 4"), P1),
        "DMGLEVEL 4"
    );
}

/// The functions: the confirm page with the fee of `Feature.ini`, the purchase (the adena goes,
/// the function exists with its fee and period), the reset, the listing pages with their
/// `<fstring>`s, and the teleport page of the level.
#[test]
fn the_functions_are_bought_listed_and_reset() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    // Nothing yet: `<fstring>4</fstring>` ("none") and no link to deactivate.
    let out = s.chamberlain_fresh(P1, "recovery");
    assert_eq!(
        last_page(&out, P1),
        "AR <fstring>4</fstring>||<fstring>4</fstring>||<fstring>4</fstring>|<fstring>4</fstring>"
    );
    // HP 300% costs 12000 for a day (`CastleHpRegenerationFeeLvl1`, ratio 86400000 ms).
    let out = s.chamberlain_fresh(P1, "HP 300");
    assert!(pages_to(&out, P1).pop().unwrap().contains("12000"));
    let out = s.chamberlain_fresh(P1, "set_func 2 300");
    assert!(pages_to(&out, P1).is_empty());
    let hp = s
        .world
        .residence_function(ResidenceRef::Castle(GLUDIO), 2)
        .unwrap()
        .clone();
    assert_eq!((hp.lvl, hp.lease, hp.rate), (300, 12_000, 86_400_000));
    assert_eq!(s.adena(P1), 10_000_000 - 12_000);
    // Now it is listed with a link to deactivate it.
    // (The first fee task ran: the period ends a day from now.)
    s.run_ms(100);
    let out = s.chamberlain_fresh(P1, "recovery");
    let page = pages_to(&out, P1).pop().unwrap();
    assert!(page.starts_with("<html><body>AR <fstring p1=\"300\">10</fstring>|<fstring p1=\"12000\" p2=\"1\">6</fstring>|<fstring p1=\""), "{page}");
    assert!(page.contains(" p3=\"20"), "{page}");
    assert!(
        page.contains("[<a action=\"bypass -h Script CastleChamberlain HP 0\">Deactivate</a>]"),
        "{page}"
    );
    // The same level again, and the reset page.
    let out = s.chamberlain_fresh(P1, "HP 300");
    assert_eq!(last_page(&out, P1), "ALREADY <fstring p1=\"300\">10</fstring>");
    let out = s.chamberlain_fresh(P1, "HP 0");
    assert_eq!(last_page(&out, P1), "RESET 2");
    s.chamberlain_fresh(P1, "set_func 2 0");
    assert!(
        s.world
            .residence_function(ResidenceRef::Castle(GLUDIO), 2)
            .is_none()
    );
    // MP level 2 on the confirm page of a function that is not there: the submit link carries both.
    let out = s.chamberlain_fresh(P1, "MP 50");
    assert!(pages_to(&out, P1).len() == 1);
    // A member without the right to manage functions.
    s.talk(P2, CHAMBERLAIN);
    assert_eq!(
        last_page(&s.chamberlain_fresh(P2, "HP 300"), P2),
        "chamberlain-21"
    );
    // Without enough adena the purchase fails and the page says so.
    s.world
        .destroy_item_by_item_id(ObjectId(P1), ADENA, 9_980_000, false);
    let out = s.chamberlain_fresh(P1, "set_func 3 40");
    assert_eq!(last_page(&out, P1), "chamberlain-09");
    assert!(
        s.world
            .residence_function(ResidenceRef::Castle(GLUDIO), 3)
            .is_none()
    );
}

/// The sub pages of the owner: functions, the teleport level page, the disabled pages.
#[test]
fn the_function_pages_follow_the_functions() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "teleport"), P1),
        "castlefuncdisabled"
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "buffer"), P1),
        "castlefuncdisabled"
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "cast_buff 0"), P1),
        "castlefuncdisabled"
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "goto 11"), P1),
        "castlefuncdisabled"
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "functions"), P1),
        "FUNCS 0 0 0"
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "additional_functions"), P1),
        "castletdecomanage"
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "products"), P1),
        "PRODUCTS 35100"
    );
    s.chamberlain_fresh(P1, "set_func 1 1");
    s.run_ms(100);
    assert_eq!(last_page(&s.chamberlain_fresh(P1, "teleport"), P1), "35100-t1");
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "other"), P1),
        "AE <fstring p1=\"1\">9</fstring>|<fstring>4</fstring>|"
    );
    // A wrong level in the list index does nothing.
    s.chamberlain_fresh(P1, "goto 21");
    s.run_ms(1000);
    assert_eq!(
        s.world.position_of(ObjectId(P1)).map(|p| (p.0, p.1)),
        Some((X + 50, Y))
    );
    // `goto <level><location>`: the lord of the level-1 list goes to location 1.
    let out = s.chamberlain_fresh(P1, "goto 11");
    assert!(pages_to(&out, P1).is_empty(), "{:?}", pages_to(&out, P1));
    s.run_ms(1000);
    s.input(P1, ClientInput::Appearing);
    s.run_ms(100);
    let (x, y, _) = s.world.position_of(ObjectId(P1)).unwrap();
    assert_eq!((x, y), (-17_100, 111_100));
}

/// `cast_buff`: the buff when the chamberlain has the mana (the page shows what is left), the
/// "not enough mana" page when the skill costs more than it has.
#[test]
fn the_support_function_casts_with_the_mana_of_the_chamberlain() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    s.chamberlain_fresh(P1, "set_func 5 5");
    s.run_ms(100);
    assert_eq!(last_page(&s.chamberlain_fresh(P1, "buffer"), P1), "BUFF mp=500");
    // BUFFS[0] is Wind Walk 2 (100 mana), BUFFS[1] Decrease Weight 3 (1000 mana).
    let out = s.chamberlain_fresh(P1, "cast_buff 0");
    assert!(last_page(&out, P1).starts_with("AFTERBUFF mp="));
    let out = s.chamberlain_fresh(P1, "cast_buff 1");
    assert_eq!(last_page(&out, P1), "NOMP mp=400");
    // An index past the list shows nothing.
    let out = s.chamberlain_fresh(P1, "cast_buff 28");
    assert!(pages_to(&out, P1).is_empty());
}

/// `operate_door`: the owner clan opens and closes the castle doors; a stranger and the clan
/// without the right get "chamberlain-21".
#[test]
fn the_doors_open_and_close_for_the_owner_clan() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    s.close_door(DOOR);
    s.close_door(DOOR_2);
    let out = s.chamberlain_fresh(P1, "operate_door 1 19210001 19210002");
    assert_eq!(last_page(&out, P1), "chamberlain-05");
    assert!(s.world.door_by_door_id(DOOR).unwrap().open);
    assert!(s.world.door_by_door_id(DOOR_2).unwrap().open);
    let out = s.chamberlain_fresh(P1, "operate_door 0 19210001");
    assert_eq!(last_page(&out, P1), "chamberlain-06");
    assert!(!s.world.door_by_door_id(DOOR).unwrap().open);
    assert!(s.world.door_by_door_id(DOOR_2).unwrap().open);
    s.talk(P2, CHAMBERLAIN);
    assert_eq!(
        last_page(&s.chamberlain_fresh(P2, "operate_door 1 19210001"), P2),
        "chamberlain-21"
    );
    assert_eq!(last_page(&s.chamberlain_fresh(P2, "doors"), P2), "chamberlain-21");
    assert_eq!(last_page(&s.chamberlain_fresh(P1, "doors"), P1), "35100-d");
}

/// `receive_report`, `fort_status`, `give_crown` and the lists: only the lord, the report with
/// the clan, the castle and the (neutral) Seven Signs strings, one crown, the siege information.
#[test]
fn the_lord_gets_the_report_the_crown_and_the_lists() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    let out = s.chamberlain_fresh(P1, "receive_report");
    assert_eq!(
        last_page(&out, P1),
        "REPORT P1 Lions 1001001 1000508 1000512 1000512 1000512"
    );
    // A member who is not the leader is no lord.
    s.talk(P2, CHAMBERLAIN);
    assert_eq!(
        last_page(&s.chamberlain_fresh(P2, "receive_report"), P2),
        "chamberlain-21"
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P2, "give_crown"), P2),
        "chamberlain-21"
    );
    // The crown once.
    let out = s.chamberlain_fresh(P1, "give_crown");
    assert_eq!(last_page(&out, P1), "CROWN P1 1001001");
    assert_eq!(item_count(&s, P1, CROWN), 1);
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "give_crown"), P1),
        "chamberlain-24"
    );
    assert_eq!(item_count(&s, P1, CROWN), 1);
    // The domain: Gludio's fortresses are 101, 102, 112 and 113; only 101 and 102 exist here,
    // and `getFortById(102)` is null, so the list fails like Java.
    assert!(pages_to(&s.chamberlain_fresh(P1, "fort_status"), P1).is_empty());
    // The siege list is the `SiegeInfo` window; the territory list the dominion registry.
    let out = s.chamberlain_fresh(P1, "list_siege_clans");
    assert_eq!(packets_to(&out, P1, "CASTLE_SIEGE_INFO").len(), 1);
    let info: SiegeInfo = decode(&packets_to(&out, P1, "CASTLE_SIEGE_INFO")[0]);
    assert_eq!(info.residence_id, 1);
    let out = s.chamberlain_fresh(P1, "list_territory_clans");
    let packet: ExShowDominionRegistry = decode(&packets_to(&out, P1, "EX_SHOW_DOMINION_REGISTRY")[0]);
    assert_eq!(packet.castle_id, 1);
    assert_eq!(packet.owner_clan_name, "No Owner");
    assert_eq!(packet.owner_ally_name, "No Ally");
    // The manor link is a page link only.
    assert_eq!(last_page(&s.chamberlain_fresh(P1, "manor"), P1), "manor");
}

/// `manors_cert`: the dawn tickets are for a Dawn member in the competition period; with the
/// neutral Seven Signs of this server everybody gets the page that says so.
#[test]
fn the_dawn_tickets_need_seven_signs() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "manors_cert"), P1),
        "ssq_notdawnorevent"
    );
    assert_eq!(
        last_page(&s.chamberlain_fresh(P1, "manors_cert_confirm"), P1),
        "ssq_notdawnorevent"
    );
    s.talk(P2, CHAMBERLAIN);
    assert_eq!(
        last_page(&s.chamberlain_fresh(P2, "manors_cert"), P2),
        "chamberlain-21"
    );
}

/// A command that does not parse is a Java exception: logged, nothing shown, nothing taken.
#[test]
fn a_bad_number_shows_nothing() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, CHAMBERLAIN);
    for event in ["set_tax abc", "deposit x", "upgrade_doors 1", "HP"] {
        let out = s.chamberlain_fresh(P1, event);
        assert!(pages_to(&out, P1).is_empty(), "{event}");
    }
    assert_eq!(s.adena(P1), 10_000_000);
}

// ---------------------------------------------------------------------------------- the rest

/// `CastleSiegeManager.onFirstTalk`.
#[test]
fn the_siege_manager_answers_by_lord_and_siege() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, SIEGE_MANAGER);
    assert_eq!(last_page(&s.take(), P1), "CastleSiegeManager-01");
    // A stranger gets the registration window (the siege list) and no page.
    s.talk(P5, SIEGE_MANAGER);
    let out = s.take();
    assert!(pages_to(&out, P5).is_empty());
    assert_eq!(packets_to(&out, P5, "CASTLE_SIEGE_INFO").len(), 1);
    // It is not a start NPC.
    assert_eq!(
        s.world.player(ObjectId(P5)).unwrap().talk.last_quest_npc_object,
        0
    );
    s.start_siege();
    s.take();
    s.talk(P1, SIEGE_MANAGER);
    assert_eq!(last_page(&s.take(), P1), "CastleSiegeManager");
    s.talk(P5, SIEGE_MANAGER);
    assert_eq!(last_page(&s.take(), P5), "CastleSiegeManager-02");
}

/// `CastleMercenaryManager`: the lord with the mercenaries right (and a GM) get the pages, the
/// others "no"; the siege page; the limit page names the castle; `main` is the first talk again.
#[test]
fn the_mercenary_manager_answers_the_lord() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, MERC_MANAGER);
    assert_eq!(last_page(&s.take(), P1), "mercmanager");
    s.talk(P2, MERC_MANAGER);
    assert_eq!(last_page(&s.take(), P2), "mercmanager-no");
    s.talk(P5, MERC_MANAGER);
    assert_eq!(last_page(&s.take(), P5), "mercmanager-no");
    s.talk(GM, MERC_MANAGER);
    assert_eq!(last_page(&s.take(), GM), "mercmanager");
    s.script(P1, "CastleMercenaryManager", "main");
    assert_eq!(last_page(&s.take(), P1), "mercmanager");
    s.script(P1, "CastleMercenaryManager", "mercmanager-01.html");
    assert_eq!(last_page(&s.take(), P1), "mercmanager-01");
    // NOTE(java-quirk): "aden" and "rune" are compared case sensitive; the castle is "Gludio"
    // anyway, so the general page with the feud name (1001000 + castle id) comes.
    s.script(P1, "CastleMercenaryManager", "limit");
    assert_eq!(last_page(&s.take(), P1), "LIMIT 1001001");
    // `buy` with a list the buy lists do not have: nothing but `ActionFailed`.
    s.script(P1, "CastleMercenaryManager", "buy 1");
    let out = s.take();
    assert!(!packets_to(&out, P1, "ACTION_FAIL").is_empty());
    s.start_siege();
    s.take();
    s.talk(P1, MERC_MANAGER);
    assert_eq!(last_page(&s.take(), P1), "mercmanager-siege");
}

/// `CastleWarehouse`: the blood alliance proofs of the lord's clan.
#[test]
fn the_vault_keeper_hands_out_the_blood_alliance_items() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, WAREHOUSE);
    assert_eq!(last_page(&s.take(), P1), "warehouse-01");
    s.script(P1, "CastleWarehouse", "warehouse-02.html");
    assert_eq!(last_page(&s.take(), P1), "warehouse-02");
    // The whole page is sent as it is: `%blood%` is replaced, `%objectId%` is not.
    s.warehouse(P1, "warehouse-04.html", Some(0));
    assert_eq!(last_page(&s.take(), P1), "BLOOD 0 %objectId%");
    // A stranger and a member who is no leader.
    for p in [P5, P2] {
        s.talk(p, WAREHOUSE);
        s.take();
        s.warehouse(p, "Receive", Some(0));
        assert_eq!(last_page(&s.take(), p), "warehouse-no");
    }
    s.warehouse(P1, "Receive", Some(0));
    assert_eq!(last_page(&s.take(), P1), "warehouse-05");
    // Sieges gave the clan three alliance proofs.
    let mut s = Scene::build(true, 3);
    s.enter_all();
    s.talk(P1, WAREHOUSE);
    s.take();
    s.warehouse(P1, "warehouse-04.html", Some(3));
    assert_eq!(last_page(&s.take(), P1), "BLOOD 3 %objectId%");
    s.warehouse(P1, "Receive", Some(3));
    let out = s.take();
    assert_eq!(last_page(&out, P1), "warehouse-06");
    assert_eq!(item_count(&s, P1, BLOOD_ALLIANCE), 3);
    assert_eq!(s.world.clan(LIONS).unwrap().blood_alliance_count, 0);
    assert!(
        residence_writes(&out).contains(&ResidenceWrite::ClanBloodAllianceCount {
            clan_id: LIONS,
            count: 0
        })
    );
    // One proof of alliance is thirty proofs of blood.
    s.script(P1, "CastleWarehouse", "Exchange");
    assert_eq!(last_page(&s.take(), P1), "warehouse-07");
    assert_eq!(
        (item_count(&s, P1, BLOOD_ALLIANCE), item_count(&s, P1, BLOOD_OATH)),
        (2, 30)
    );
    s.world
        .destroy_item_by_item_id(ObjectId(P1), BLOOD_ALLIANCE, -1, false);
    s.script(P1, "CastleWarehouse", "Exchange");
    assert_eq!(last_page(&s.take(), P1), "warehouse-08");
    // Any other event is `null`.
    s.script(P1, "CastleWarehouse", "bogus");
    assert!(pages_to(&s.take(), P1).is_empty());
}

/// D-50: the proofs of alliance are read again before they are shown or handed out, because the
/// Java game server adds to them at the end of its sieges. The fresh count is what counts (the
/// same proofs are never given twice), and a failed read gives nothing.
#[test]
fn the_vault_keeper_reads_the_blood_alliance_count_again() {
    let mut s = Scene::build(true, 3);
    s.enter_all();
    s.talk(P1, WAREHOUSE);
    s.take();
    // Sieghardt's siege added two: the page and the handout show five.
    s.warehouse(P1, "warehouse-04.html", Some(5));
    assert_eq!(last_page(&s.take(), P1), "BLOOD 5 %objectId%");
    s.warehouse(P1, "Receive", Some(5));
    let out = s.take();
    assert_eq!(item_count(&s, P1, BLOOD_ALLIANCE), 5);
    assert!(
        residence_writes(&out).contains(&ResidenceWrite::ClanBloodAllianceCount {
            clan_id: LIONS,
            count: 0
        })
    );
    // Sieghardt's lord took them in the meantime: this world says 5, the row says 0.
    let mut stale = Scene::build(true, 5);
    stale.enter_all();
    stale.talk(P1, WAREHOUSE);
    stale.take();
    stale.warehouse(P1, "Receive", Some(0));
    assert_eq!(last_page(&stale.take(), P1), "warehouse-05");
    assert_eq!(item_count(&stale, P1, BLOOD_ALLIANCE), 0);
    // A failed read refuses the command: nothing is shown or given.
    s.script(P1, "CastleWarehouse", "Receive");
    let asked = s.take();
    let then = reread_requests(&asked).pop().expect("a re-read request");
    s.world.handle(WorldCommand::ResidenceReread {
        result: Err(()),
        then,
    });
    let out = s.take();
    assert!(pages_to(&out, P1).is_empty());
    assert_eq!(item_count(&s, P1, BLOOD_ALLIANCE), 5);
}

/// `CastleBlacksmith`: the lord, a member with the manor right and a GM; the second page only
/// by the event named after the NPC.
#[test]
fn the_blacksmith_serves_the_owner() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, BLACKSMITH);
    assert_eq!(last_page(&s.take(), P1), "35098-01");
    s.talk(P5, BLACKSMITH);
    assert_eq!(last_page(&s.take(), P5), "no");
    s.talk(P2, BLACKSMITH);
    assert_eq!(last_page(&s.take(), P2), "no");
    s.script(P1, "CastleBlacksmith", "35098-02.html");
    assert_eq!(last_page(&s.take(), P1), "35098-02");
    s.script(P1, "CastleBlacksmith", "35098-03.html");
    assert!(pages_to(&s.take(), P1).is_empty());
    s.script(P5, "CastleBlacksmith", "35098-02.html");
    assert!(pages_to(&s.take(), P5).is_empty());
    s.talk(GM, BLACKSMITH);
    assert_eq!(last_page(&s.take(), GM), "35098-01");
}

/// `CastleTeleporter`: the countdown shout, the script value, the long wait after the towers.
#[test]
fn the_mass_gatekeeper_counts_down_and_shouts() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P1, TELEPORTER);
    assert_eq!(last_page(&s.take(), P1), "teleporter-01");
    s.script(P1, "CastleTeleporter", "TELEPORT_START");
    assert_eq!(last_page(&s.take(), P1), "teleporter-03");
    // While it counts, the first talk says so, and a second start does not restart the timer.
    s.talk(P1, TELEPORTER);
    assert_eq!(last_page(&s.take(), P1), "teleporter-03");
    s.script(P1, "CastleTeleporter", "TELEPORT_START");
    s.take();
    s.run_ms(29_000);
    assert!(packets_to(&s.take(), P1, "NPC_SAY").is_empty());
    s.run_ms(2000);
    let out = s.take();
    let said: NpcSay = decode(&packets_to(&out, P1, "NPC_SAY")[0]);
    assert_eq!(
        said.npc_string,
        NpcStringId::TheDefendersOfS1CastleWillBeTeleportedToTheInnerCastle as i32
    );
    assert_eq!(said.parameters, ["Gludio"]);
    // Ready again.
    s.talk(P1, TELEPORTER);
    assert_eq!(last_page(&s.take(), P1), "teleporter-01");
    // A siege whose control towers are all gone: the page says eight minutes, and so does the timer.
    s.start_siege();
    s.take();
    s.talk(P1, TELEPORTER);
    assert_eq!(last_page(&s.take(), P1), "teleporter-02");
    s.script(P1, "CastleTeleporter", "TELEPORT_START");
    s.take();
    s.run_ms(60_000);
    assert!(packets_to(&s.take(), P1, "NPC_SAY").is_empty());
    s.run_ms(430_000);
    assert_eq!(packets_to(&s.take(), P1, "NPC_SAY").len(), 1);
}

/// `CastleCourtMagician`: strangers are sent away, the talismans cost ten epaulettes and the
/// inventory must not be exactly full, the squad skills need the leader or the fame right.
#[test]
fn the_court_magician_trades_talismans_for_epaulettes() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.talk(P5, MAGICIAN);
    assert_eq!(last_page(&s.take(), P5), "courtmagician-01");
    s.script(P5, "CastleCourtMagician", "giveTalisman");
    assert_eq!(last_page(&s.take(), P5), "courtmagician-01");
    s.talk(P1, MAGICIAN);
    assert_eq!(last_page(&s.take(), P1), "courtmagician");
    s.script(P1, "CastleCourtMagician", "courtmagician-03.html");
    assert_eq!(last_page(&s.take(), P1), "courtmagician-03");
    // Five epaulettes are not enough.
    s.script(P2, "CastleCourtMagician", "giveTalisman");
    s.talk(P2, MAGICIAN);
    s.take();
    s.script(P2, "CastleCourtMagician", "giveTalisman");
    assert_eq!(last_page(&s.take(), P2), "courtmagician-06");
    assert_eq!(item_count(&s, P2, EPAULETTE), 5);
    // Ten are: one talisman of the table comes, the epaulettes go.
    s.script(P1, "CastleCourtMagician", "giveTalisman");
    assert_eq!(last_page(&s.take(), P1), "courtmagician-04");
    assert_eq!(item_count(&s, P1, EPAULETTE), 0);
    let talismans: i64 = talisman_ids().into_iter().map(|id| item_count(&s, P1, id)).sum();
    assert_eq!(talismans, 1);
    // A member who may not use the squad skills.
    s.script(P2, "CastleCourtMagician", "squadSkill");
    assert_eq!(last_page(&s.take(), P2), "courtmagician-05");
    // The leader without a Clan Gate: "courtmagician-02".
    s.script(P2, "CastleCourtMagician", "clanTeleport");
    assert_eq!(last_page(&s.take(), P2), "courtmagician-02");
}

/// `MercenaryCaptain`: the first talk by level and class, the strider purchase with the badge,
/// the registration window, the pages with the badge prices.
#[test]
fn the_mercenary_captain_sells_striders_for_badges() {
    let mut s = Scene::new(true);
    s.enter_all();
    // Level 40 but a first class: "36481-08".
    s.talk(P1, CAPTAIN);
    assert_eq!(last_page(&s.take(), P1), "36481-08");
    s.script(P1, "MercenaryCaptain", "36481-02.html");
    assert_eq!(last_page(&s.take(), P1), "36481-02");
    s.script(P1, "MercenaryCaptain", "36481-03.html");
    assert_eq!(last_page(&s.take(), P1), "STRIDERS 50 80");
    // Not enough badges.
    s.script(P1, "MercenaryCaptain", "strider 0");
    assert_eq!(last_page(&s.take(), P1), "36481-07");
    s.world.add_item(ObjectId(P1), BADGE, 60, false);
    s.script(P1, "MercenaryCaptain", "strider 0");
    assert_eq!(last_page(&s.take(), P1), "36481-09");
    assert_eq!(item_count(&s, P1, BADGE), 10);
    assert_eq!(item_count(&s, P1, STRIDER_WIND), 1);
    // The great strider costs 80.
    s.script(P1, "MercenaryCaptain", "strider 3");
    assert_eq!(last_page(&s.take(), P1), "36481-07");
    // An unknown type gives nothing (and costs nothing).
    s.world.add_item(ObjectId(P1), BADGE, 100, false);
    s.script(P1, "MercenaryCaptain", "strider 9");
    assert!(pages_to(&s.take(), P1).is_empty());
    assert_eq!(item_count(&s, P1, BADGE), 110);
    // Without the certificates the elite lists are closed.
    s.script(P1, "MercenaryCaptain", "elite");
    assert_eq!(last_page(&s.take(), P1), "36481-10");
    s.script(P1, "MercenaryCaptain", "top-elite");
    assert_eq!(last_page(&s.take(), P1), "36481-10");
    s.script(P1, "MercenaryCaptain", "territory");
    let out = s.take();
    let packet: ExShowDominionRegistry = decode(&packets_to(&out, P1, "EX_SHOW_DOMINION_REGISTRY")[0]);
    assert_eq!(packet.castle_id, 1);
}

/// `CastleDoorman`: the window by clan and right, the doors by the owner clan, the busy page while
/// the siege zone is active, the teleport through the gate.
#[test]
fn the_castle_doorman_serves_the_owner_clan() {
    let mut s = Scene::new(true);
    s.enter_all();
    let npc = s.npc(DOORMAN);
    s.talk(P1, DOORMAN);
    assert_eq!(last_page(&s.take(), P1), format!("DOORMAN OWNER {}", npc.0));
    s.talk(P5, DOORMAN);
    assert_eq!(last_page(&s.take(), P5), format!("DOORMAN NO {}", npc.0));
    // A member of the owner clan without the right to open doors.
    s.talk(P2, DOORMAN);
    assert_eq!(last_page(&s.take(), P2), format!("DOORMAN NO {}", npc.0));
    s.close_door(DOOR);
    s.close_door(DOOR_2);
    s.npc_bypass(P1, DOORMAN, "open_doors 1, 19210001, 19210002");
    assert!(s.world.door_by_door_id(DOOR).unwrap().open && s.world.door_by_door_id(DOOR_2).unwrap().open);
    s.npc_bypass(P1, DOORMAN, "close_doors 1, 19210001");
    assert!(!s.world.door_by_door_id(DOOR).unwrap().open);
    assert!(s.world.door_by_door_id(DOOR_2).unwrap().open);
    s.npc_bypass(P5, DOORMAN, "close_doors 1, 19210002");
    s.npc_bypass(P2, DOORMAN, "close_doors 1, 19210002");
    assert!(s.world.door_by_door_id(DOOR_2).unwrap().open);
    // The siege zone is active: the doors are not touched and the busy page comes.
    s.start_siege();
    s.take();
    // (The start of the siege closed the doors.)
    assert!(!s.world.door_by_door_id(DOOR_2).unwrap().open);
    s.npc_bypass(P1, DOORMAN, "open_doors 1, 19210002");
    let out = s.take();
    assert_eq!(last_page(&out, P1), "DOORMAN BUSY");
    assert!(!s.world.door_by_door_id(DOOR_2).unwrap().open);
    // The teleport list `OTHER` of the doorman (it is not closed by the siege). The busy page has
    // no links (Java's has none), so the window is opened again first.
    s.talk(P1, DOORMAN);
    s.npc_bypass(P1, DOORMAN, "tele 1");
    s.run_ms(1000);
    s.input(P1, ClientInput::Appearing);
    s.run_ms(100);
    assert_eq!(
        s.world.position_of(ObjectId(P1)).map(|p| (p.0, p.1)),
        Some((-17_600, 111_600))
    );
}

/// `CastleAmbassador`: the owner's choice at the envoy reads the fortress again, is written as a
/// player's action, and the envoys leave; the hour without a choice makes the fortress
/// independent (a job: only with the switch).
#[test]
fn the_envoy_takes_the_choice_of_the_fortress_owner() {
    let mut s = Scene::new(false);
    s.enter_all();
    let envoy = s.npc(ENVOY);
    // Only the owner of the fortress sees the choice; the castle is Gludio.
    s.world.set_target(ObjectId(P3), Some(envoy));
    s.input(
        P3,
        ClientInput::Action {
            object_id: envoy.0,
            action_id: 0,
        },
    );
    s.run_ms(100);
    assert_eq!(last_page(&s.take(), P3), "AMBASSADOR castle=Gludio");
    s.world
        .tele_to_location(ObjectId(P5), TeleportTarget::at(FX, Y, Z));
    s.input(P5, ClientInput::Appearing);
    s.run_ms(100);
    s.world.set_target(ObjectId(P5), Some(envoy));
    s.input(
        P5,
        ClientInput::Action {
            object_id: envoy.0,
            action_id: 0,
        },
    );
    s.run_ms(100);
    assert_eq!(last_page(&s.take(), P5), "AMBASSADOR-03 castle=Gludio");
    // The owner signs: the fortress is read again first.
    s.script(P3, "CastleAmbassador", "signed");
    let asked = s.take();
    assert_eq!(reread_requests(&asked).len(), 1);
    assert_eq!(s.world.fort(SHANTY).unwrap().state, 0);
    let then = reread_requests(&asked).pop().unwrap();
    s.world.handle(WorldCommand::ResidenceReread {
        result: Ok(ResidenceRereadRows::Fort(FortRecord {
            id: 101,
            name: "Shanty".to_owned(),
            owner_id: WOLVES,
            ..FortRecord::default()
        })),
        then,
    });
    let out = s.take();
    assert_eq!(last_page(&out, P3), "AMBASSADOR-05 castle=Gludio");
    let fort = s.world.fort(SHANTY).unwrap();
    assert_eq!((fort.state, fort.castle_id), (2, 1));
    assert!(residence_writes(&out).contains(&ResidenceWrite::FortState {
        fort_id: 101,
        state: 2,
        castle_id: 1
    }));
    // A second choice: signed again says "already", rejected after signing repeats the refusal page.
    s.script(P3, "CastleAmbassador", "rejected");
    let asked = s.take();
    let then = reread_requests(&asked).pop().unwrap();
    s.world.handle(WorldCommand::ResidenceReread {
        result: Ok(ResidenceRereadRows::Fort(FortRecord {
            id: 101,
            name: "Shanty".to_owned(),
            owner_id: WOLVES,
            state: 2,
            castle_id: 1,
            ..FortRecord::default()
        })),
        then,
    });
    assert_eq!(last_page(&s.take(), P3), "AMBASSADOR-02 castle=Gludio");
    // The envoy leaves three seconds after the choice.
    assert!(s.world.npc(envoy).is_some());
    s.run_ms(3500);
    assert!(s.world.npc(envoy).is_none());
}

/// The hour without a choice: the fortress becomes independent, written only with the switch.
#[test]
fn the_envoys_leave_after_an_hour_and_the_fortress_stays_independent() {
    for switch in [false, true] {
        let mut s = Scene::new(switch);
        s.enter_all();
        let envoy = s.npc(ENVOY);
        s.run_ms(3_599_000);
        assert!(s.world.npc(envoy).is_some());
        assert_eq!(s.world.fort(SHANTY).unwrap().state, 0);
        s.take();
        s.run_ms(2000);
        assert!(s.world.npc(envoy).is_none());
        let fort = s.world.fort(SHANTY).unwrap();
        assert_eq!((fort.state, fort.castle_id), (1, 1));
        let written = residence_writes(&s.take()).contains(&ResidenceWrite::FortState {
            fort_id: 101,
            state: 1,
            castle_id: 1,
        });
        assert_eq!(written, switch, "D-51");
    }
}
