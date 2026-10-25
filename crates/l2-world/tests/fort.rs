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
    ArmorSets, DoorTemplates, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache,
    ItemTemplates, KarmaLoss, MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions,
    SkillTemplates, SkillTrees, SpawnData, SpawnLoadOptions, StaticObjectTemplates, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    ExShowFortressInfo, ExShowFortressMapInfo, ExShowFortressSiegeInfo, NpcHtmlMessage, NpcSay,
    SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{NpcStringId, ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::fort::{
    FortSettings, FortSiegeFortLoad, FortSiegeGuardRecord, FortSiegeLoad, FortSiegeRows, FortSiegeWrite,
    FortSpawnRecord, FortThen, LogisticsCommand,
};
use l2_world::residence::{FortRecord, ResidenceReadThen, ResidenceRereadRows};
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
/// The headquarters of the castle sieges' `SiegeFlag` (M7-SIEGE-01).
const NPC_HQ: i32 = 35_062;

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
        npc(NPC_HQ, "Folk", "Headquarters"),
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

/// The lists of the foreman (`tel1`) and of the doorman (`OTHER`).
const TELEPORTERS: &str = r#"<?xml version="1.0"?><list>
  <npc id="35663">
    <teleport name="tel1" type="OTHER">
      <location name="Barracks" x="-70000" y="258000" z="-3100" />
    </teleport>
  </npc>
  <npc id="35667">
    <teleport type="OTHER">
      <location name="Out of the gate" x="-70500" y="258500" z="-3100" />
      <location name="Into the gate" x="-70600" y="258600" z="-3100" />
    </teleport>
  </npc>
</list>"#;

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
  <zone name="shanty_fortress" type="FortZone" {}<stat name="fortId" val="101" /><spawn X="-84000" Y="244000" Z="-3700" /></zone>
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
    (
        "data/html/fortress/foreman.htm",
        r#"<html><body>FOREMAN %objectId% %npcname%
<a action="bypass -h npc_%objectId%_receive_report">Receive Report.</a>
<a action="bypass -h npc_%objectId%_operate_door">Open/Close Door.</a>
<a action="bypass -h npc_%objectId%_expel">Expel Outsiders.</a>
<a action="bypass -h npc_%objectId%_functions">Fortress Functions.</a>
<a action="bypass -h npc_%objectId%_manage_vault">Clan Warehouse.</a>
<a action="bypass -h npc_%objectId%_manage">Fortress Management.</a></body></html>"#,
    ),
    (
        "data/html/fortress/foreman-no.htm",
        "<html><body>FOREMAN NO</body></html>",
    ),
    (
        "data/html/fortress/foreman-busy.htm",
        "<html><body>FOREMAN BUSY</body></html>",
    ),
    (
        "data/html/fortress/foreman-nac.htm",
        "<html><body>FOREMAN NAC</body></html>",
    ),
    (
        "data/html/fortress/foreman-noprivs.htm",
        "<html><body>NOPRIVS</body></html>",
    ),
    (
        "data/html/fortress/foreman-report.htm",
        "<html><body>REPORT %hr% %min%</body></html>",
    ),
    (
        "data/html/fortress/foreman-castlereport.htm",
        "<html><body>CASTLEREPORT %hr% %min% %castle% %hr2% %min2%</body></html>",
    ),
    (
        "data/html/fortress/foreman-expel.htm",
        r#"<html><body>EXPEL <a action="bypass -h npc_%objectId%_banish_foreigner">Expel.</a></body></html>"#,
    ),
    (
        "data/html/fortress/foreman-expeled.htm",
        "<html><body>EXPELED</body></html>",
    ),
    (
        "data/html/fortress/foreman-opened.htm",
        "<html><body>OPENED</body></html>",
    ),
    (
        "data/html/fortress/foreman-closed.htm",
        "<html><body>CLOSED</body></html>",
    ),
    (
        "data/html/fortress/foreman-functions.htm",
        r#"<html><body>FUNCTIONS hp %hp_regen% mp %mp_regen% xp %xp_regen%
<a action="bypass -h npc_%objectId%_functions tele">Teleport</a>
<a action="bypass -h npc_%objectId%_functions support">Supplemental Magic</a></body></html>"#,
    ),
    (
        "data/html/fortress/foreman-vault.htm",
        r#"<html><body>VAULT
<a action="bypass -h npc_%objectId%_manage_vault deposit">Deposit</a>
<a action="bypass -h npc_%objectId%_manage_vault withdraw">Redeem</a></body></html>"#,
    ),
    (
        "data/html/fortress/manage.htm",
        r#"<html><body>MANAGE
<a action="bypass -h npc_%objectId%_manage recovery">Recovery</a>
<a action="bypass -h npc_%objectId%_manage other">Other</a></body></html>"#,
    ),
    (
        "data/html/fortress/edit_recovery.htm",
        "<html><body>RECOVERY hp [%hp_recovery%] [%hp_period%] [%change_hp%] mp [%mp_recovery%] [%change_mp%] exp [%exp_recovery%] [%change_exp%]</body></html>",
    ),
    (
        "data/html/fortress/edit_other.htm",
        "<html><body>OTHER tele [%tele%] [%change_tele%] support [%support%] [%change_support%]</body></html>",
    ),
    (
        "data/html/fortress/functions-apply.htm",
        r#"<html><body>APPLY %name% | %cost% | %use% <a action="bypass -h npc_%objectId%_manage %apply%">OK</a></body></html>"#,
    ),
    (
        "data/html/fortress/functions-apply_confirmed.htm",
        "<html><body>APPLY CONFIRMED</body></html>",
    ),
    (
        "data/html/fortress/functions-cancel.htm",
        r#"<html><body>CANCEL <a action="bypass -h npc_%objectId%_manage %apply%">Confirm</a></body></html>"#,
    ),
    (
        "data/html/fortress/functions-cancel_confirmed.htm",
        "<html><body>CANCEL CONFIRMED</body></html>",
    ),
    (
        "data/html/fortress/functions-used.htm",
        "<html><body>USED %val%</body></html>",
    ),
    (
        "data/html/fortress/low_adena.htm",
        "<html><body>LOW ADENA</body></html>",
    ),
    (
        "data/html/fortress/support1.htm",
        r#"<html><body>SUPPORT1 mp %mp% <a action="bypass -h npc_%objectId%_support 4342 2">Wind Walk</a></body></html>"#,
    ),
    (
        "data/html/fortress/support-done.htm",
        "<html><body>SUPPORT DONE %mp%</body></html>",
    ),
    (
        "data/html/fortress/support-no_mana.htm",
        "<html><body>NO MANA %mp%</body></html>",
    ),
    (
        "data/html/fortress/35663-d.htm",
        r#"<html><body>DOORS %npcname%
<a action="bypass -h npc_%objectId%_operate_door 1 18220001 18220002">Open the gate</a>
<a action="bypass -h npc_%objectId%_operate_door 0 18220001 18220002">Close the gate</a></body></html>"#,
    ),
    (
        "data/html/fortress/logistics.htm",
        r#"<html><body>LOGISTICS %clanname% %npcId%
<a action="bypass -h npc_%objectId%_supplylvl">Check supplies.</a>
<a action="bypass -h npc_%objectId%_rewards">Check rewards.</a></body></html>"#,
    ),
    (
        "data/html/fortress/logistics-rewards.htm",
        r#"<html><body>REWARDS %bloodoath% <a action="bypass -h npc_%objectId%_blood">Receive</a></body></html>"#,
    ),
    (
        "data/html/fortress/logistics-blood.htm",
        "<html><body>BLOOD</body></html>",
    ),
    (
        "data/html/fortress/logistics-noblood.htm",
        "<html><body>NOBLOOD</body></html>",
    ),
    (
        "data/html/fortress/logistics-noprivs.htm",
        "<html><body>LOGISTICS NOPRIVS</body></html>",
    ),
    (
        "data/html/fortress/logistics-supply.htm",
        "<html><body>SUPPLY</body></html>",
    ),
    (
        "data/html/fortress/logistics-nosupply.htm",
        "<html><body>NOSUPPLY</body></html>",
    ),
    (
        "data/html/fortress/logistics-siege.htm",
        "<html><body>SIEGE</body></html>",
    ),
    (
        "data/html/fortress/logistics-supplylvl.htm",
        r#"<html><body>SUPPLYLVL %supplylvl% <a action="bypass -h npc_%objectId%_supply">Receive supplies.</a></body></html>"#,
    ),
    (
        "data/html/fortress/logistics-1.htm",
        "<html><body>LOGISTICS ONE</body></html>",
    ),
    (
        "data/html/doorman/35667.htm",
        r#"<html><body>DOORMAN OWNER
<a action="bypass -h npc_%objectId%_open_doors 5, 18220001, 18220002">Open</a>
<a action="bypass -h npc_%objectId%_close_doors 5, 18220001, 18220002">Close</a>
<a action="bypass -h npc_%objectId%_open_doors 18220001, 18220002">Open without a leading number</a>
<a action="bypass -h npc_%objectId%_tele 1">Teleport</a></body></html>"#,
    ),
    (
        "data/html/doorman/35667-no.htm",
        "<html><body>DOORMAN NO</body></html>",
    ),
    (
        "data/html/doorman/35667-busy.htm",
        "<html><body>DOORMAN BUSY</body></html>",
    ),
    (
        "data/html/teleporter/teleports.htm",
        "<html><body>TELEPORTS %locations%</body></html>",
    ),
    (
        "data/html/admin/forts.htm",
        "<html><body>FORTS %forts%</body></html>",
    ),
    (
        "data/html/admin/fort.htm",
        "<html><body>FORT %fortName% %fortId%</body></html>",
    ),
];

/// The pages of the two scripts (`FortressSiegeManager-NN.html` ... ).
fn script_pages() -> Vec<(String, String)> {
    let manager = "data/scripts/ai/others/FortressSiegeManager";
    let mut pages = vec![
        (format!("{manager}/FortressSiegeManager.html"), "<html><body>MERCHANT FREE</body></html>".to_owned()),
        (
            format!("{manager}/FortressSiegeManager-01.html"),
            "<html><body>MERCHANT OF %clanName% %objectId% <a action=\"bypass Script FortressSiegeManager FortressSiegeManager-11.html\">Yes</a></body></html>".to_owned(),
        ),
        (
            format!("{manager}/FortressSiegeManager-12.html"),
            "<html><body>OWNER PAGE %clanName%</body></html>".to_owned(),
        ),
    ];
    pages.push((
        format!("{manager}/FortressSiegeManager-11.html"),
        "<html><body>PAGE 11 <a action=\"bypass -h Script FortressSiegeManager register\">Register</a> <a action=\"bypass -h Script FortressSiegeManager cancel\">Cancel</a> <a action=\"bypass -h Script FortressSiegeManager warInfo\">War info</a></body></html>".to_owned(),
    ));
    for n in [2, 3, 4, 5, 6, 7, 8, 9, 10, 13, 14, 15, 16, 17, 18, 19, 20, 21] {
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
        (
            "ShantyCommander1",
            format!("{},{},{},0,{NPC_COMMANDER_1}", X + 100, Y, Z),
        ),
        (
            "ShantyCommander2",
            format!("{},{},{},0,{NPC_COMMANDER_2}", X + 200, Y, Z),
        ),
        (
            "ShantyCommander3",
            format!("{},{},{},0,{NPC_COMMANDER_3}", X + 300, Y, Z),
        ),
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

    fn with(
        siege_database_writes: bool,
        fort: FortRecord,
        siege_clans: Vec<i32>,
        settings: FortSettings,
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
        world.set_teleporter_data(Arc::new(l2_data::TeleporterData::from_xml(TELEPORTERS).unwrap()));
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

    /// A teleport and the client's `Appearing`.
    fn teleport(&mut self, id: i32, x: i32, y: i32, z: i32) {
        self.world
            .tele_to_location(ObjectId(id), TeleportTarget::at(x, y, z));
        self.appear(id);
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
            Out::Persist(PersistCommand::Residence(l2_world::ResidenceWrite::FortSiege(w))) => {
                Some(w.clone())
            }
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
    assert!(
        off.world.fort_attacker_clans(SHANTY).is_empty(),
        "cleared in this world"
    );
    assert!(
        fort_writes(&off.loaded).is_empty(),
        "but nothing is written without the switch"
    );
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
    let FortSiegeWrite::SiegeDate {
        fort_id: 101,
        siege_date,
    } = writes[1]
    else {
        panic!("{writes:?}");
    };
    let expected = now_ms() + 3_600_000;
    assert!(
        (siege_date - expected).abs() < 60_000,
        "{siege_date} vs {expected}"
    );
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
    assert!(
        s.npcs_of(NPC_MERCHANT).is_empty(),
        "the merchant leaves 10 minutes before"
    );
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
                .filter(|m| {
                    !m.params.is_empty() && m.id != SystemMessageId::S1MinuteSUntilTheFortressBattleStarts
                })
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
    assert!(
        s.world
            .player(ObjectId(P5))
            .unwrap()
            .zones
            .is_inside(ZoneId::Siege)
    );
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

// ---------------------------------------------------------------------------------- commanders

fn says_to(out: &[Out], to: i32) -> Vec<NpcSay> {
    packets_to(out, to, "NPC_SAY")
        .iter()
        .map(|b| decode::<NpcSay>(b))
        .collect()
}

/// `FortCommander.doDie` -> `FortSiege.killedCommander`: the commander shouts the string of its
/// number, the barracks are seized (and come back after `CountDownLength` minutes, with the doors
/// repaired); the last one opens the doors of the main building, shows the flags and ends the
/// timer.
#[test]
fn killing_the_commanders_follows_javas_barracks() {
    let mut s = siege_scene();
    s.world.fort_start_siege(SHANTY);
    s.enter(P3, WOLVES, 0);
    s.take();
    // The first commander dies: "You may have broken our arrows ...".
    let c1 = s.one_npc(NPC_COMMANDER_1);
    s.world.do_die(c1, Some(ObjectId(P3)));
    let out = s.take();
    let says = says_to(&out, P5);
    assert_eq!(says.len(), 1);
    assert_eq!(
        says[0].npc_string,
        NpcStringId::YouMayHaveBrokenOurArrowsButYouWillNeverBreakOurWillArchersRetreat as i32
    );
    assert_eq!(
        (says[0].text_type, says[0].npc_id, says[0].object_id),
        (23, NPC_COMMANDER_1, c1.0)
    );
    for p in [P1, P3, P4] {
        assert!(
            message_ids(&out, p).contains(&SystemMessageId::TheBarracksHaveBeenSeized),
            "P{p:x}"
        );
    }
    assert!(!message_ids(&out, P5).contains(&SystemMessageId::TheBarracksHaveBeenSeized));
    assert_eq!(s.world.fort_commanders_alive(SHANTY), 2);
    // `CountDownLength` (10 minutes) later the barracks function again.
    run_ticks(&mut s, 10 * TICKS_PER_MINUTE - 5);
    assert_eq!(s.world.fort_commanders_alive(SHANTY), 2);
    run_ticks(&mut s, 10);
    let out = s.take();
    assert_eq!(s.world.fort_commanders_alive(SHANTY), 3);
    assert!(message_ids(&out, P4).contains(&SystemMessageId::TheBarracksFunctionHasBeenRestored));
    let c1 = s.npcs_of(NPC_COMMANDER_1);
    assert_eq!(c1.len(), 1);
    assert!(!s.world.is_dead(c1[0]));

    // All three die: the second and the third shout, the main building opens, the flags appear.
    s.take();
    for (npc, expected) in [
        (
            NPC_COMMANDER_2,
            NpcStringId::AiieeeeCommandCenterThisIsGuardUnitWeNeedBackupRightAway,
        ),
        (
            NPC_COMMANDER_3,
            NpcStringId::AtLastTheMagicFieldThatProtectsTheFortressHasWeakenedVolunteersStandBack,
        ),
    ] {
        let c = s.one_npc(npc);
        s.world.do_die(c, Some(ObjectId(P3)));
        let says = says_to(&s.take(), P5);
        assert_eq!(says[0].npc_string, expected as i32);
    }
    assert!(flags_on_ground(&s).is_empty());
    let last = s.one_npc(NPC_COMMANDER_1);
    s.world.do_die(last, Some(ObjectId(P3)));
    let out = s.take();
    assert_eq!(s.world.fort_commanders_alive(SHANTY), 0);
    assert!(message_ids(&out, P3).contains(&SystemMessageId::AllBarracksAreOccupied));
    assert_eq!(flags_on_ground(&s).len(), 2, "the flags of FortSiege.ini appear");
    // The door that shows no hp is the main building's: it opens; the other stays.
    assert!(s.world.door_by_door_id(18_220_001).unwrap().open);
    assert!(!s.world.door_by_door_id(18_220_002).unwrap().open);
    // The restore timer is cancelled: the barracks stay taken.
    run_ticks(&mut s, 12 * TICKS_PER_MINUTE);
    assert_eq!(s.world.fort_commanders_alive(SHANTY), 0);
}

fn flags_on_ground(s: &Scene) -> Vec<ObjectId> {
    s.world
        .ground_item_ids()
        .into_iter()
        .filter(|id| s.world.ground_item(*id).unwrap().data.item_id == FLAG_ITEM)
        .collect()
}

/// `FortCommander.addDamage`: the first commander calls for the attack and then keeps quiet for
/// 10 seconds; the second names the attacker (the owner of a summon).
#[test]
fn a_commander_speaks_when_hit_and_then_keeps_quiet() {
    let mut s = siege_scene();
    s.world.fort_start_siege(SHANTY);
    s.enter(P3, WOLVES, 0);
    s.take();
    let c1 = s.one_npc(NPC_COMMANDER_1);
    s.world.reduce_current_hp(c1, 10.0, Some(ObjectId(P3)));
    let says = says_to(&s.take(), P5);
    assert_eq!(says.len(), 1);
    assert_eq!(
        says[0].npc_string,
        NpcStringId::AttackingTheEnemySReinforcementsIsNecessaryTimeToDie as i32
    );
    s.world.reduce_current_hp(c1, 10.0, Some(ObjectId(P3)));
    assert!(says_to(&s.take(), P5).is_empty(), "silent for 10 seconds");
    run_ticks(&mut s, 205);
    s.world.reduce_current_hp(c1, 10.0, Some(ObjectId(P3)));
    assert_eq!(says_to(&s.take(), P5).len(), 1);
    // The second commander names the attacker.
    let c2 = s.one_npc(NPC_COMMANDER_2);
    s.world.reduce_current_hp(c2, 10.0, Some(ObjectId(P3)));
    let says = says_to(&s.take(), P5);
    assert_eq!(
        says[0].npc_string,
        NpcStringId::EveryoneConcentrateYourAttacksOnS1ShowTheEnemyYourResolve as i32
    );
    assert_eq!(says[0].parameters, ["P3"]);
}

/// `FortCommander.isAutoAttackable`: during the siege, by a player who is not of the owning clan.
#[test]
fn commanders_are_attackable_for_attackers_only_during_the_siege() {
    let mut s = siege_scene();
    s.enter(P3, WOLVES, 0);
    s.world.fort_start_siege(SHANTY);
    let c1 = s.one_npc(NPC_COMMANDER_1);
    assert!(s.world.is_auto_attackable_by(c1, ObjectId(P3)));
    assert!(s.world.is_auto_attackable_by(c1, ObjectId(P5)), "anybody else");
    assert!(!s.world.is_auto_attackable_by(c1, ObjectId(P1)), "not the owner");
    s.world.fort_end_siege(SHANTY);
    s.world.fort_start_siege(SHANTY);
}

// ---------------------------------------------------------------------------------- combat flag

/// A siege with all barracks taken, the Wolves' leader inside the fortress.
fn flag_scene() -> Scene {
    let mut s = siege_scene();
    s.world.fort_start_siege(SHANTY);
    // An attacker who logs in inside a siege zone goes to town (`EnterWorld`, M7-SIEGE-01): the
    // leader logs in outside and comes in.
    s.enter_at(P3, WOLVES, X + 10_000, 0);
    s.teleport(P3, X, Y, Z);
    for npc in [NPC_COMMANDER_1, NPC_COMMANDER_2, NPC_COMMANDER_3] {
        let c = s.one_npc(npc);
        s.world.do_die(c, Some(ObjectId(P3)));
    }
    s.take();
    assert_eq!(flags_on_ground(&s).len(), 2);
    s
}

/// `Player.doPickupItem` / `addItem` / `CombatFlag.activate`: only the clan leader of an
/// attacker picks the flag up; it is worn at once (the message, the paperdoll), the participants
/// hear who has it.
#[test]
fn the_leader_of_an_attacker_picks_up_and_wears_the_flag() {
    let mut s = flag_scene();
    let flags = flags_on_ground(&s);
    // A member of the clan, a leader of a clan that does not take part, the owner.
    for who in [P4, P5, P1] {
        s.world.do_pickup_item(ObjectId(who), flags[0]);
        assert!(!s.world.is_combat_flag_equipped(ObjectId(who)), "P{who:x}");
        assert_eq!(flags_on_ground(&s).len(), 2);
    }
    s.take();
    s.world.do_pickup_item(ObjectId(P3), flags[0]);
    let out = s.take();
    assert!(s.world.is_combat_flag_equipped(ObjectId(P3)));
    assert_eq!(flags_on_ground(&s), [flags[1]]);
    // The flag is worn.
    let worn = s
        .world
        .paperdoll_object(
            ObjectId(P3),
            l2_protocol_game::server::charscreen::paperdoll::RHAND,
        )
        .expect("worn in the right hand");
    assert_eq!(worn, flags[0], "the item that lay there is the item worn");
    let messages = messages_to(&out, P3);
    let equipped: Vec<&SystemMessage> = messages
        .iter()
        .filter(|m| m.id == SystemMessageId::YouHaveEquippedYourS1)
        .collect();
    assert_eq!(equipped.len(), 1);
    assert_eq!(int_param(equipped[0], 0), FLAG_ITEM);
    // "C1 has acquired the flag" to the siege's participants.
    for p in [P1, P3, P4] {
        let acquired: Vec<SystemMessage> = messages_to(&out, p)
            .into_iter()
            .filter(|m| m.id == SystemMessageId::C1HasAcquiredTheFlag)
            .collect();
        assert_eq!(acquired.len(), 1, "P{p:x}");
        assert_eq!(text_param(&acquired[0], 0), "P3");
    }
    assert!(!message_ids(&out, P5).contains(&SystemMessageId::C1HasAcquiredTheFlag));
    // Only one flag per player.
    s.world.do_pickup_item(ObjectId(P3), flags[1]);
    assert_eq!(flags_on_ground(&s), [flags[1]]);
}

/// The flag is lost with the life of its bearer: it is destroyed and appears at its place again
/// while the siege runs (`dropCombatFlag`); a bearer who leaves the world loses it too.
#[test]
fn the_flag_returns_when_the_bearer_dies_or_leaves() {
    let mut s = flag_scene();
    let flag = flags_on_ground(&s)[0];
    s.world.do_pickup_item(ObjectId(P3), flag);
    assert!(s.world.is_combat_flag_equipped(ObjectId(P3)));
    s.world.do_die(ObjectId(P3), Some(ObjectId(P1)));
    assert!(!s.world.is_combat_flag_equipped(ObjectId(P3)));
    assert_eq!(flags_on_ground(&s).len(), 2, "back at its place");
    // Wear it again and leave the world.
    s.world.do_revive(ObjectId(P3));
    let flag = flags_on_ground(&s)[0];
    s.world.do_pickup_item(ObjectId(P3), flag);
    assert!(s.world.is_combat_flag_equipped(ObjectId(P3)));
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(P3).unwrap()),
    });
    s.run_ms(200);
    assert!(s.world.player(ObjectId(P3)).is_none());
    assert!(!s.world.is_combat_flag_equipped(ObjectId(P3)));
    // NOTE(java-quirk): `onPlayerLogoutInside` brings the bearer to town before the flag is
    // taken away, so `getFort(player)` finds no fortress and the flag is destroyed without
    // `dropCombatFlag`: it does not come back until the siege ends.
    assert_eq!(flags_on_ground(&s).len(), 1);
}

/// `FortSiege.endSiege` takes the flags away, those on the ground and the worn one.
#[test]
fn the_end_of_the_siege_removes_every_flag() {
    let mut s = flag_scene();
    let flag = flags_on_ground(&s)[0];
    s.world.do_pickup_item(ObjectId(P3), flag);
    s.world.fort_end_siege(SHANTY);
    assert!(!s.world.is_combat_flag_equipped(ObjectId(P3)));
    assert!(flags_on_ground(&s).is_empty());
    assert_eq!(
        s.world.paperdoll_object(
            ObjectId(P3),
            l2_protocol_game::server::charscreen::paperdoll::RHAND
        ),
        None
    );
}

// ---------------------------------------------------------------------------------- the owner

/// `TakeFort` -> `Fort.endOfSiege(clan)` -> `setOwner(clan, true)`: the siege ends, the old owner
/// loses the fortress and `LooseFortPoints`, the new one gets it and `TakeFortPoints`, everybody
/// hears the winner, the clan hears the sound, the skills change and the rows go out.
#[test]
fn taking_the_fortress_changes_the_owner() {
    let mut s = flag_scene();
    s.enter(P2, LIONS, 0);
    s.take();
    s.world.fort_end_of_siege(SHANTY, WOLVES);
    let out = s.take();
    assert!(!s.world.fort_siege_in_progress(SHANTY));
    let fort = s.world.fort(SHANTY).unwrap();
    assert_eq!(fort.owner_id, WOLVES);
    assert_eq!((fort.state, fort.castle_id, fort.supply_level), (0, 0, 0));
    assert_eq!((s.clan(WOLVES).fort_id, s.clan(LIONS).fort_id), (101, 0));
    // TAKE_FORT_POINTS.
    assert_eq!(s.clan(WOLVES).reputation_score, 200);
    // The Wolves' members got the fortress skill, the Lions' lost it.
    let skills = |p: i32| -> Vec<i32> {
        s.world
            .player(ObjectId(p))
            .unwrap()
            .core
            .skills
            .iter()
            .map(|k| k.0)
            .collect()
    };
    assert!(skills(P3).contains(&591));
    assert!(!skills(P2).contains(&591));
    // "The fortress battle of S1 has finished" to the participants (before the owner changes).
    for p in [P1, P3, P4] {
        let finished: Vec<SystemMessage> = messages_to(&out, p)
            .into_iter()
            .filter(|m| m.id == SystemMessageId::TheFortressBattleOfS1HasFinished)
            .collect();
        assert_eq!(finished.len(), 1, "P{p:x}");
        assert_eq!(int_param(&finished[0], 0), 101);
    }
    // "S1 is victorious in the fortress battle of S2" to everybody.
    for p in [P1, P2, P3, P4, P5] {
        let victory: Vec<SystemMessage> = messages_to(&out, p)
            .into_iter()
            .filter(|m| m.id == SystemMessageId::S1IsVictoriousInTheFortressBattleOfS2)
            .collect();
        assert_eq!(victory.len(), 1, "P{p:x}");
        assert_eq!(text_param(&victory[0], 0), "Wolves");
        assert_eq!(int_param(&victory[0], 1), 101);
    }
    assert_eq!(packets_to(&out, P3, "PLAY_SOUND").len(), 1);
    assert!(packets_to(&out, P5, "PLAY_SOUND").is_empty());
    // The siege is over for everybody.
    assert_eq!(s.world.fort_siege_state_of(ObjectId(P3)), 0);
    assert!(!s.world.is_combat_flag_equipped(ObjectId(P3)));
    let pole = fort.flag_pole.unwrap();
    assert_eq!(s.world.static_object(pole).unwrap().mesh_index, 1);
    // `FortUpdater` runs for the new owner every 360 minutes.
    assert_eq!(s.world.fort_time_till_next_update(SHANTY), 360 * 60);
    // The rows.
    let writes = residence_writes(&out);
    assert!(
        writes.iter().any(|w| matches!(
            w,
            l2_world::ResidenceWrite::FortOwner { fort_id: 101, owner: WOLVES, last_owned_time } if *last_owned_time > 0
        )),
        "{writes:?}"
    );
    assert!(writes.contains(&l2_world::ResidenceWrite::FortState {
        fort_id: 101,
        state: 0,
        castle_id: 0
    }));
    assert!(writes.contains(&l2_world::ResidenceWrite::FortOwner {
        fort_id: 101,
        owner: 0,
        last_owned_time: 0
    }));
}

/// D-51: the same owner change writes no fortress row without the switch.
#[test]
fn taking_the_fortress_without_the_switch_writes_nothing() {
    let mut s = Scene::new(false, LIONS, vec![WOLVES], now_ms() + 30 * 60_000);
    s.enter(P3, WOLVES, 0);
    s.world.fort_start_siege(SHANTY);
    s.take();
    s.world.fort_end_of_siege(SHANTY, WOLVES);
    let out = s.take();
    assert_eq!(s.world.fort(SHANTY).unwrap().owner_id, WOLVES);
    assert!(residence_writes(&out).is_empty(), "{:?}", residence_writes(&out));
}

/// A clan that owns a castle cannot keep the fortress: the old owner loses it, the siege goes on,
/// "The rebel army recaptured the fortress".
#[test]
fn a_clan_with_a_castle_does_not_take_the_fortress() {
    let mut s = Scene::new(true, LIONS, vec![BEARS], now_ms() + 30 * 60_000);
    s.enter(P5, BEARS, 0);
    s.world.fort_start_siege(SHANTY);
    s.take();
    s.world.fort_end_of_siege(SHANTY, BEARS);
    let out = s.take();
    assert_eq!(s.world.fort(SHANTY).unwrap().owner_id, 0);
    assert_eq!(s.clan(LIONS).fort_id, 0);
    assert_eq!(s.clan(BEARS).fort_id, 0);
    assert!(s.world.fort_siege_in_progress(SHANTY));
    assert!(message_ids(&out, P5).contains(&SystemMessageId::TheRebelArmyRecapturedTheFortress));
}

// ---------------------------------------------------------------------------------- FortUpdater

fn blood_oaths(s: &Scene, clan: i32) -> i32 {
    s.clan(clan).blood_oath_count
}

fn warehouse_adena(s: &Scene, clan: i32) -> i64 {
    s.clan(clan)
        .warehouse
        .iter()
        .filter(|i| i.item_id == ADENA)
        .map(|i| i.count)
        .sum()
}

fn fast_updater() -> FortSettings {
    let mut settings = settings();
    settings.fs_update_frq = 1;
    settings
}

/// `FortUpdater` `PERIODIC_UPDATE`: every `FortressPeriodicUpdateFrequency` minutes the owner gets
/// `FortressBloodOathCount` blood oaths (the clan's row is written with the switch).
#[test]
fn the_periodic_update_counts_blood_oaths() {
    for switch in [true, false] {
        let mut s = Scene::with(switch, shanty(LIONS), vec![], fast_updater());
        s.take();
        // The runs are aligned to the time the owner took the fortress (an hour ago): the first
        // one is due within a minute.
        s.run_ms(61_000);
        assert_eq!(blood_oaths(&s, LIONS), 1);
        let writes = fort_writes(&s.take());
        if switch {
            assert_eq!(
                writes,
                [FortSiegeWrite::BloodOathCount {
                    clan_id: LIONS,
                    count: 1
                }]
            );
        } else {
            assert!(writes.is_empty());
        }
        s.run_ms(60_000);
        assert_eq!(blood_oaths(&s, LIONS), 2);
        assert_eq!(blood_oaths(&s, WOLVES), 0);
    }
}

fn contracted_shanty() -> FortRecord {
    FortRecord {
        state: 2,
        castle_id: 1,
        supply_level: 0,
        ..shanty(LIONS)
    }
}

/// With a contract (`getFortState() == 2`) the update charges `FortressFeeForCastle` from the
/// clan warehouse for the castle's treasury and raises the supply level; a clan that cannot pay
/// becomes independent. Both move adena, so they run with the switch only.
#[test]
fn the_periodic_update_charges_the_contract_with_the_switch() {
    let mut s = Scene::with(true, contracted_shanty(), vec![], fast_updater());
    s.take();
    s.run_ms(61_000);
    let out = s.take();
    assert_eq!(warehouse_adena(&s, LIONS), 75_000);
    assert_eq!(s.world.castle(CastleId(1)).unwrap().treasury, 25_000);
    assert_eq!(s.world.fort(SHANTY).unwrap().supply_level, 1);
    let writes = residence_writes(&out);
    assert!(writes.contains(&l2_world::ResidenceWrite::CastleTreasury {
        castle_id: 1,
        treasury: 25_000
    }));
    assert!(writes.contains(&l2_world::ResidenceWrite::FortSupplyLevel {
        fort_id: 101,
        supply_level: 1
    }));
    // 100,000 adena pay four fees.
    s.run_ms(60_000 * 3);
    assert_eq!(s.world.fort(SHANTY).unwrap().supply_level, 4);
    assert_eq!(warehouse_adena(&s, LIONS), 0);
    // The next fee cannot be paid: the contract ends.
    s.take();
    s.run_ms(60_000);
    assert_eq!(s.world.fort(SHANTY).unwrap().state, 1);
    assert!(
        residence_writes(&s.take()).contains(&l2_world::ResidenceWrite::FortState {
            fort_id: 101,
            state: 1,
            castle_id: 0
        })
    );
}

#[test]
fn the_periodic_update_moves_no_adena_without_the_switch() {
    let mut s = Scene::with(false, contracted_shanty(), vec![], fast_updater());
    s.take();
    s.run_ms(61_000);
    s.run_ms(60_000 * 3);
    assert_eq!(warehouse_adena(&s, LIONS), 100_000);
    assert_eq!(s.world.castle(CastleId(1)).unwrap().treasury, 0);
    assert_eq!(s.world.fort(SHANTY).unwrap().supply_level, 0);
    assert_eq!(s.world.fort(SHANTY).unwrap().state, 2);
    assert!(residence_writes(&s.take()).is_empty());
}

/// `FortUpdater` `MAX_OWN_TIME`: an owner that held the fortress longer than
/// `FortressMaximumOwnTime` hours loses it a minute after the start (`Fort.load`), and the state
/// is reset.
#[test]
fn the_owner_loses_the_fortress_after_the_maximum_time() {
    let mut fort = shanty(LIONS);
    fort.last_owned_time = now_ms() - 170 * 3_600_000;
    fort.state = 1;
    let mut s = Scene::with(true, fort, vec![], settings());
    assert_eq!(s.world.fort(SHANTY).unwrap().owner_id, LIONS);
    s.take();
    s.run_ms(59_000);
    assert_eq!(s.world.fort(SHANTY).unwrap().owner_id, LIONS);
    s.run_ms(2_000);
    let out = s.take();
    let fort = s.world.fort(SHANTY).unwrap();
    assert_eq!((fort.owner_id, fort.state), (0, 0));
    assert_eq!(s.clan(LIONS).fort_id, 0);
    let writes = residence_writes(&out);
    assert!(writes.contains(&l2_world::ResidenceWrite::FortOwner {
        fort_id: 101,
        owner: 0,
        last_owned_time: 0
    }));
    // No task is left for the missing owner.
    assert_eq!(s.world.fort_time_till_next_update(SHANTY), 0);
}

// ---------------------------------------------------------------------------------- the NPCs

fn place(s: &mut Scene, npc_id: i32, dx: i32) -> ObjectId {
    let (x, y, z) = at(dx);
    s.world
        .spawn_npc_at(npc_id, x, y, z, 0)
        .expect("the NPC template exists")
}

/// The foreman's windows: the owner's page, the other clans' "no" page, the busy page during a
/// siege; the report, with the owner's time and the castle of the contract.
#[test]
fn the_foreman_shows_a_page_by_the_clan_and_the_report() {
    let mut s = Scene::with(true, contracted_shanty(), vec![], settings());
    let foreman = place(&mut s, NPC_FOREMAN, 80);
    s.enter(P1, LIONS, 0);
    s.enter(P3, WOLVES, 0);
    s.take();
    s.talk(P1, foreman);
    let pages = pages_to(&s.take(), P1);
    assert_eq!(pages.len(), 1, "{pages:?}");
    assert!(
        pages[0].contains(&format!("FOREMAN {} Foreman", foreman.0)),
        "{}",
        pages[0]
    );
    s.talk(P3, foreman);
    assert!(pages_to(&s.take(), P3)[0].contains("FOREMAN NO"));

    // The report of a contract: the time till the rebel army, the castle, the next update.
    s.bypass(P1, foreman, "receive_report");
    let page = pages_to(&s.take(), P1).remove(0);
    let rebel = s.world.fort_time_till_rebel_army(SHANTY);
    let (hour, minutes) = (rebel / 3600, (rebel - rebel / 3600 * 3600) / 60);
    let next = s.world.fort_time_till_next_update(SHANTY);
    let (hour2, minutes2) = (next / 3600, (next - next / 3600 * 3600) / 60);
    assert!(
        page.contains(&format!(
            "CASTLEREPORT {hour} {minutes} Gludio {hour2} {minutes2}"
        )),
        "{page}"
    );
    // Without a contract the other report.
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let foreman = place(&mut s, NPC_FOREMAN, 80);
    s.enter(P1, LIONS, 0);
    s.talk(P1, foreman);
    s.take();
    s.bypass(P1, foreman, "receive_report");
    let page = pages_to(&s.take(), P1).remove(0);
    let rebel = s.world.fort_time_till_rebel_army(SHANTY);
    let (hour, minutes) = (rebel / 3600, (rebel - rebel / 3600 * 3600) / 60);
    assert!(page.contains(&format!("REPORT {hour} {minutes}")), "{page}");
    // During a siege the foreman is busy.
    s.enter(P3, WOLVES, 0);
    s.world.fort_add_attacker(SHANTY, ObjectId(P3), false);
    s.world.fort_start_siege(SHANTY);
    s.take();
    s.talk(P1, foreman);
    assert!(pages_to(&s.take(), P1)[0].contains("FOREMAN BUSY"));
}

/// `expel` / `banish_foreigner`: only with the privilege; the outsiders in the fortress go to
/// the zone's spawn point, the clan stays.
#[test]
fn the_foreman_expels_outsiders_for_those_with_the_privilege() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let foreman = place(&mut s, NPC_FOREMAN, 80);
    s.enter(P1, LIONS, 0);
    s.enter(P2, LIONS, 0);
    s.enter(P5, BEARS, 0);
    s.talk(P1, foreman);
    s.talk(P2, foreman);
    s.take();
    // A member without the privilege is told so.
    s.bypass(P2, foreman, "expel");
    assert!(pages_to(&s.take(), P2)[0].contains("NOPRIVS"));
    s.bypass(P1, foreman, "expel");
    assert!(pages_to(&s.take(), P1)[0].contains("EXPEL"));
    s.bypass(P1, foreman, "banish_foreigner");
    let out = s.take();
    assert!(pages_to(&out, P1)[0].contains("EXPELED"));
    s.appear(P5);
    let (x, y, _) = s.world.position_of(ObjectId(P5)).unwrap();
    assert!((x + 84_000).abs() <= 60 && (y - 244_000).abs() <= 60, "{x} {y}");
    assert_eq!(s.world.position_of(ObjectId(P1)).unwrap().0, X);
    assert_eq!(s.world.position_of(ObjectId(P2)).unwrap().0, X);
}

/// `operate_door`: the owner opens and closes the doors of the page; the page without a
/// parameter is `<npcId>-d.htm`.
#[test]
fn the_foreman_opens_and_closes_doors() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let foreman = place(&mut s, NPC_FOREMAN, 80);
    s.enter(P1, LIONS, 0);
    s.enter(P3, WOLVES, 0);
    s.talk(P1, foreman);
    s.take();
    s.bypass(P1, foreman, "operate_door");
    assert!(pages_to(&s.take(), P1)[0].contains("DOORS Foreman"));
    s.bypass(P1, foreman, "operate_door 1 18220001 18220002");
    assert!(pages_to(&s.take(), P1)[0].contains("OPENED"));
    assert!(s.world.door_by_door_id(18_220_001).unwrap().open);
    assert!(s.world.door_by_door_id(18_220_002).unwrap().open);
    // Back to the page of the doors, then close them.
    s.talk(P1, foreman);
    s.bypass(P1, foreman, "operate_door");
    s.take();
    s.bypass(P1, foreman, "operate_door 0 18220001 18220002");
    assert!(pages_to(&s.take(), P1)[0].contains("CLOSED"));
    assert!(!s.world.door_by_door_id(18_220_001).unwrap().open);
    assert!(!s.world.door_by_door_id(18_220_002).unwrap().open);
    // The page of another clan has no such link: the bypass is refused before the foreman hears
    // of it.
    s.talk(P3, foreman);
    s.take();
    s.bypass(P3, foreman, "operate_door 1 18220001 18220002");
    assert!(pages_to(&s.take(), P3).is_empty());
    assert!(!s.world.door_by_door_id(18_220_002).unwrap().open);
}

/// The clan vault: the rows of the clan warehouse are read again first (D-50).
#[test]
fn the_vault_reads_the_clan_warehouse_again() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let foreman = place(&mut s, NPC_FOREMAN, 80);
    s.enter(P1, LIONS, 0);
    s.talk(P1, foreman);
    s.take();
    s.bypass(P1, foreman, "manage_vault");
    assert!(pages_to(&s.take(), P1)[0].contains("VAULT"));
    s.bypass(P1, foreman, "manage_vault deposit");
    let out = s.take();
    assert!(
        out.iter().any(|o| matches!(
            o,
            Out::Persist(PersistCommand::ClanWarehouseRequest { clan_id: LIONS, .. })
        )),
        "the CLANWH rows are asked for first"
    );
}

/// Walks the pages of the foreman: each command is a link of the page before it.
fn walk(s: &mut Scene, player: i32, npc: ObjectId, commands: &[&str]) -> String {
    let mut page = String::new();
    for command in commands {
        s.take();
        s.bypass(player, npc, command);
        page = pages_to(&s.take(), player).pop().unwrap_or_default();
    }
    page
}

/// `manage recovery ...`: buying, keeping, changing and cancelling a function; the first fee is
/// paid by the leader at once.
#[test]
fn the_foreman_sells_and_cancels_functions() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let foreman = place(&mut s, NPC_FOREMAN, 80);
    s.enter(P1, LIONS, 0);
    s.enter(P2, LIONS, 0);
    s.talk(P1, foreman);
    s.talk(P2, foreman);
    s.take();
    // A member without `CASTLE_MANAGE_FUNCTIONS`.
    assert!(walk(&mut s, P2, foreman, &["manage"]).contains("NOPRIVS"));
    assert!(walk(&mut s, P1, foreman, &["manage"]).contains("MANAGE"));
    let page = walk(&mut s, P1, foreman, &["manage recovery"]);
    assert!(page.contains("hp [none] [none]"), "{page}");
    assert!(page.contains("edit_hp 300") && page.contains("edit_hp 400"));
    // The offer: 300% costs `FortressHpRegenerationFeeLvl1` per day.
    let page = walk(&mut s, P1, foreman, &["manage recovery edit_hp 300"]);
    assert!(page.contains("2000</font>Adena /1 Day</font>)"), "{page}");
    assert!(page.contains("(HP Recovery Device)"));
    assert!(page.contains("recovery hp 300") && page.contains("<font color=\"00FFFF\">300%</font>"));
    // Buy it.
    let before = adena_of(&s, P1);
    assert!(walk(&mut s, P1, foreman, &["manage recovery hp 300"]).contains("APPLY CONFIRMED"));
    assert_eq!(adena_of(&s, P1), before - 2000);
    let f = s
        .world
        .residence_function(ResidenceRef::Fort(SHANTY), 2)
        .expect("the function exists");
    assert_eq!((f.lvl, f.lease, f.rate), (300, 2000, 86_400_000));
    // The same level again.
    s.talk(P1, foreman);
    let page = walk(
        &mut s,
        P1,
        foreman,
        &[
            "manage",
            "manage recovery",
            "manage recovery edit_hp 300",
            "manage recovery hp 300",
        ],
    );
    assert!(page.contains("USED 300%"), "{page}");
    assert_eq!(adena_of(&s, P1), before - 2000);
    // The page of the foreman shows it.
    s.talk(P1, foreman);
    assert!(walk(&mut s, P1, foreman, &["functions"]).contains("hp 300 mp 0 xp 0"));
    s.talk(P1, foreman);
    let page = walk(&mut s, P1, foreman, &["manage", "manage recovery"]);
    assert!(
        page.contains("hp [300%</font> (<font color=\"FFAABB\">2000</font>Adena /1 Day)] [Withdraw the fee for the next time at"),
        "{page}"
    );
    assert!(page.contains("recovery hp_cancel"));
    // Cancel it: level 0 and lease 0 remove the function.
    assert!(walk(&mut s, P1, foreman, &["manage recovery hp_cancel"]).contains("manage recovery hp 0"));
    assert!(walk(&mut s, P1, foreman, &["manage recovery hp 0"]).contains("CANCEL CONFIRMED"));
    assert!(
        s.world
            .residence_function(ResidenceRef::Fort(SHANTY), 2)
            .is_none()
    );
}

/// A leader that cannot pay gets "low adena" twice (the page object is sent again at the end of
/// the Java branch).
#[test]
fn a_purchase_without_adena_sends_the_low_adena_page_twice() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let foreman = place(&mut s, NPC_FOREMAN, 80);
    let mut poor = pdata(P1, LIONS, X, 0);
    poor.items[0].count = 100;
    s.enter_with(poor);
    s.talk(P1, foreman);
    walk(
        &mut s,
        P1,
        foreman,
        &["manage", "manage recovery", "manage recovery edit_hp 400"],
    );
    s.take();
    s.bypass(P1, foreman, "manage recovery hp 400");
    let pages = pages_to(&s.take(), P1);
    assert_eq!(pages.len(), 2, "{pages:?}");
    assert!(pages.iter().all(|p| p.contains("LOW ADENA")));
    assert!(
        s.world
            .residence_function(ResidenceRef::Fort(SHANTY), 2)
            .is_none()
    );
}

/// The teleport and support functions: no function, "nac" page; the support page shows the
/// foreman's mana.
#[test]
fn the_foreman_has_no_teleport_or_support_without_the_function() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let foreman = place(&mut s, NPC_FOREMAN, 80);
    s.enter(P1, LIONS, 0);
    s.talk(P1, foreman);
    walk(&mut s, P1, foreman, &["functions"]);
    assert!(walk(&mut s, P1, foreman, &["functions tele"]).contains("FOREMAN NAC"));
    s.talk(P1, foreman);
    walk(&mut s, P1, foreman, &["functions"]);
    assert!(walk(&mut s, P1, foreman, &["functions support"]).contains("FOREMAN NAC"));
    // Buy the support function and see its page.
    s.talk(P1, foreman);
    walk(
        &mut s,
        P1,
        foreman,
        &["manage", "manage other", "manage other edit_support 1"],
    );
    assert!(walk(&mut s, P1, foreman, &["manage other support 1"]).contains("APPLY CONFIRMED"));
    let f = s.world.residence_function(ResidenceRef::Fort(SHANTY), 5).unwrap();
    assert_eq!((f.lvl, f.lease, f.rate), (1, 7000, 86_400_000));
    s.talk(P1, foreman);
    walk(&mut s, P1, foreman, &["functions"]);
    assert!(walk(&mut s, P1, foreman, &["functions support"]).contains("SUPPORT1 mp "));
}

/// The doorman: the owner clan sees its page, the others the "no" page, during a siege the
/// "busy" page; the commands open and close the doors after the first number.
#[test]
fn the_doorman_opens_the_doors_for_the_owner_when_there_is_no_siege() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let doorman = place(&mut s, NPC_DOORMAN, 60);
    s.enter(P1, LIONS, 0);
    s.enter(P3, WOLVES, 0);
    s.talk(P1, doorman);
    s.talk(P3, doorman);
    assert!(pages_to(&s.take(), P1)[0].contains("DOORMAN OWNER"));
    s.talk(P3, doorman);
    s.take();
    s.bypass(P1, doorman, "open_doors 5, 18220001, 18220002");
    assert!(s.world.door_by_door_id(18_220_001).unwrap().open);
    assert!(s.world.door_by_door_id(18_220_002).unwrap().open);
    s.bypass(P1, doorman, "close_doors 5, 18220001, 18220002");
    assert!(!s.world.door_by_door_id(18_220_001).unwrap().open);
    assert!(!s.world.door_by_door_id(18_220_002).unwrap().open);
    // Not the owner.
    s.bypass(P3, doorman, "open_doors 5, 18220001, 18220002");
    assert!(!s.world.door_by_door_id(18_220_001).unwrap().open);
    // NOTE(java-quirk): the first number is skipped, even when it is a door.
    s.bypass(P1, doorman, "open_doors 18220001, 18220002");
    assert!(!s.world.door_by_door_id(18_220_001).unwrap().open);
    assert!(s.world.door_by_door_id(18_220_002).unwrap().open);
    // The siege: the busy page, the doors stay.
    s.bypass(P1, doorman, "close_doors 5, 18220002");
    s.enter(P4, WOLVES, 0);
    s.world.fort_add_attacker(SHANTY, ObjectId(P4), false);
    s.world.fort_start_siege(SHANTY);
    s.take();
    s.bypass(P1, doorman, "open_doors 5, 18220001, 18220002");
    assert!(pages_to(&s.take(), P1)[0].contains("DOORMAN BUSY"));
    assert!(!s.world.door_by_door_id(18_220_001).unwrap().open);
}

/// Sends the command of a logistics page, answers the re-read it asks for with `blood` oaths for
/// the owner, and returns the page the player got.
fn logistics(s: &mut Scene, player: i32, officer: ObjectId, command: &str, blood: i32) -> Vec<String> {
    s.take();
    s.bypass(player, officer, command);
    let requests = reread_requests(&s.take());
    assert_eq!(requests.len(), 1, "{command}: one re-read");
    answer_reread(s, requests[0].clone(), LIONS, vec![], Some(blood));
    pages_to(&s.take(), player)
}

/// `FortLogistics`: its window names the owner (or "NPC"); the rewards, the blood oaths, the
/// supply level and the supply box. Each of them reads the fortress and the clan again first
/// (D-50). `isMyLord` is Java's `||` of the castle and the fortress ids.
#[test]
fn logistics_hands_out_blood_oaths() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let officer = place(&mut s, NPC_LOGISTICS, 90);
    s.enter(P1, LIONS, 0);
    s.enter(P3, WOLVES, 0);
    s.enter(P5, BEARS, 0);
    s.talk(P1, officer);
    let page = pages_to(&s.take(), P1).remove(0);
    assert!(
        page.contains(&format!("LOGISTICS Lions {NPC_LOGISTICS}")),
        "{page}"
    );

    // The rewards: the blood oaths are read again.
    s.bypass(P1, officer, "rewards");
    let requests = reread_requests(&s.take());
    assert_eq!(
        requests,
        [ResidenceReadThen::Fort(FortThen::Logistics {
            player: ObjectId(P1),
            npc: officer,
            kind: LogisticsCommand::Rewards
        })]
    );
    answer_reread(&mut s, requests[0].clone(), LIONS, vec![], Some(5));
    assert!(pages_to(&s.take(), P1)[0].contains("REWARDS 5"));
    // The blood oaths become items and the count is written, whatever the switch says.
    let pages = logistics(&mut s, P1, officer, "blood", 5);
    assert!(pages[0].contains("BLOOD"));
    assert_eq!(item_count(&s, P1, BLOOD_OATH_ITEM), 5);
    assert_eq!(blood_oaths(&s, LIONS), 0);
    // The write went out with the answer; look at it by asking again from the start.
    s.talk(P1, officer);
    logistics(&mut s, P1, officer, "rewards", 0);
    let pages = logistics(&mut s, P1, officer, "blood", 0);
    assert!(pages[0].contains("NOBLOOD"));

    // The clan leader of another clan is not the lord of this fortress.
    s.talk(P3, officer);
    assert!(logistics(&mut s, P3, officer, "rewards", 5)[0].contains("LOGISTICS NOPRIVS"));
    // NOTE(java-quirk): `isMyLord` compares the castle ids too: the lord of the castle that is
    // nearest to the officer passes.
    s.talk(P5, officer);
    assert!(logistics(&mut s, P5, officer, "rewards", 5)[0].contains("REWARDS"));
}

#[test]
fn the_blood_oath_count_is_written_for_the_player_whatever_the_switch() {
    for switch in [true, false] {
        let mut s = Scene::new(switch, LIONS, vec![], now_ms() + 30 * 60_000);
        let officer = place(&mut s, NPC_LOGISTICS, 90);
        s.enter(P1, LIONS, 0);
        s.talk(P1, officer);
        logistics(&mut s, P1, officer, "rewards", 5);
        s.take();
        s.bypass(P1, officer, "blood");
        let requests = reread_requests(&s.take());
        answer_reread(&mut s, requests[0].clone(), LIONS, vec![], Some(5));
        let out = s.take();
        assert_eq!(
            fort_writes(&out),
            [FortSiegeWrite::BloodOathCount {
                clan_id: LIONS,
                count: 0
            }]
        );
    }
}

#[test]
fn logistics_gives_the_supply_box_and_resets_the_level() {
    let fort = FortRecord {
        supply_level: 1,
        state: 2,
        castle_id: 1,
        ..shanty(LIONS)
    };
    let mut s = Scene::with(false, fort, vec![], settings());
    let officer = place(&mut s, NPC_LOGISTICS, 90);
    s.enter(P1, LIONS, 0);
    s.talk(P1, officer);
    // The supply level page (a contract: the leader sees the level).
    assert!(logistics(&mut s, P1, officer, "supplylvl", 0)[0].contains("SUPPLYLVL 1"));
    s.take();
    s.bypass(P1, officer, "supply");
    let requests = reread_requests(&s.take());
    answer_reread(&mut s, requests[0].clone(), LIONS, vec![], Some(0));
    let out = s.take();
    assert!(pages_to(&out, P1)[0].contains("SUPPLY"));
    // The box of level 1 appears next to the officer, the level is 0 and the row is written (a
    // player's own action, whatever the switch).
    let boxes = s.npcs_of(NPC_BOX);
    assert_eq!(boxes.len(), 1);
    let officer_pos = s.world.position_of(officer).unwrap();
    assert_eq!(
        s.world.position_of(boxes[0]).unwrap(),
        (officer_pos.0 - 23, officer_pos.1 + 41, officer_pos.2)
    );
    assert_eq!(s.world.fort(SHANTY).unwrap().supply_level, 0);
    assert!(
        residence_writes(&out).contains(&l2_world::ResidenceWrite::FortSupplyLevel {
            fort_id: 101,
            supply_level: 0
        })
    );
    // No more supplies: the answer of the read says level 0 now.
    s.talk(P1, officer);
    logistics(&mut s, P1, officer, "supplylvl", 0);
    let pages = logistics(&mut s, P1, officer, "supply", 0);
    assert!(pages[0].contains("NOSUPPLY"), "{pages:?}");
}

/// A supply level and a siege: the officer sends the lord away.
#[test]
fn logistics_gives_no_supplies_during_a_siege() {
    let fort = FortRecord {
        supply_level: 2,
        state: 2,
        castle_id: 1,
        siege_date: now_ms() + 30 * 60_000,
        ..shanty(LIONS)
    };
    let mut s = Scene::with(false, fort, vec![WOLVES], settings());
    let officer = place(&mut s, NPC_LOGISTICS, 90);
    s.enter(P1, LIONS, 0);
    s.world.fort_start_siege(SHANTY);
    s.talk(P1, officer);
    logistics(&mut s, P1, officer, "supplylvl", 0);
    let pages = logistics(&mut s, P1, officer, "supply", 0);
    assert!(pages[0].contains("SIEGE"), "{pages:?}");
    assert!(s.npcs_of(NPC_BOX).is_empty());
}

#[test]
fn logistics_supply_level_without_a_contract_is_the_other_page() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let officer = place(&mut s, NPC_LOGISTICS, 90);
    s.enter(P1, LIONS, 0);
    s.talk(P1, officer);
    assert!(logistics(&mut s, P1, officer, "supplylvl", 0)[0].contains("LOGISTICS ONE"));
}

/// A failed read refuses the command: nothing is shown (D-50: never fall back to memory).
#[test]
fn a_failed_reread_refuses_the_logistics_command() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let officer = place(&mut s, NPC_LOGISTICS, 90);
    s.enter(P1, LIONS, 0);
    s.talk(P1, officer);
    s.take();
    s.bypass(P1, officer, "rewards");
    let requests = reread_requests(&s.take());
    s.world.handle(WorldCommand::ResidenceReread {
        result: Err(()),
        then: requests[0].clone(),
    });
    assert!(pages_to(&s.take(), P1).is_empty());
}

// ---------------------------------------------------------------------------------- the scripts

/// A siege manager (the suspicious merchant) of Shanty, which the Lions own, and the page it shows.
fn manager_scene(fort: FortRecord, settings: FortSettings) -> (Scene, ObjectId) {
    let mut fort = fort;
    fort.siege_date = now_ms() + 30 * 60_000;
    let mut s = Scene::with(true, fort, vec![], settings);
    let merchant = place(&mut s, NPC_MERCHANT, 40);
    (s, merchant)
}

/// Sends `Script FortressSiegeManager <command>`, answers the re-read it asks for with the rows
/// of the database, and returns the pages the player got.
fn siege_command(s: &mut Scene, player: i32, command: &str, clans: Vec<i32>) -> Vec<String> {
    s.take();
    s.script(player, &format!("FortressSiegeManager {command}"));
    let requests = reread_requests(&s.take());
    assert_eq!(requests.len(), 1, "{command}: one re-read");
    let owner = s.world.fort(SHANTY).unwrap().owner_id;
    answer_reread(s, requests[0].clone(), owner, clans, None);
    pages_to(&s.take(), player)
}

/// Talks to the merchant and opens the page with the registration links.
fn open_registration_page(s: &mut Scene, player: i32, merchant: ObjectId) {
    s.talk(player, merchant);
    s.script(player, "FortressSiegeManager FortressSiegeManager-11.html");
}

/// The suspicious merchant opens with a page by the owner: the clan name and the way to the
/// registration pages; the pages pass through without a read.
#[test]
fn the_siege_manager_talks_and_shows_its_pages() {
    let (mut s, merchant) = manager_scene(shanty(0), settings());
    s.enter(P3, WOLVES, 0);
    s.talk(P3, merchant);
    assert!(pages_to(&s.take(), P3)[0].contains("MERCHANT FREE"));
    let (mut s, merchant) = manager_scene(shanty(LIONS), settings());
    s.enter(P3, WOLVES, 0);
    s.talk(P3, merchant);
    let page = pages_to(&s.take(), P3).remove(0);
    assert!(
        page.contains(&format!("MERCHANT OF Lions {}", merchant.0)),
        "{page}"
    );
    // The link to the story needs no read.
    s.script(P3, "FortressSiegeManager FortressSiegeManager-11.html");
    let out = s.take();
    assert!(reread_requests(&out).is_empty());
    assert!(pages_to(&out, P3)[0].contains("PAGE 11"));
}

#[test]
fn the_siege_manager_registers_a_clan_after_reading_the_rows_again() {
    let (mut s, merchant) = manager_scene(shanty(LIONS), settings());
    s.enter(P3, WOLVES, 0);
    open_registration_page(&mut s, P3, merchant);
    let before = adena_of(&s, P3);
    let pages = siege_command(&mut s, P3, "register", vec![]);
    assert!(pages[0].contains("PAGE 05"), "{pages:?}");
    assert_eq!(s.world.fort_attacker_clans(SHANTY), [WOLVES]);
    assert_eq!(adena_of(&s, P3), before - 250_000);
}

#[test]
fn registering_tells_the_clan_and_writes_the_row() {
    let (mut s, merchant) = manager_scene(shanty(LIONS), settings());
    s.enter(P3, WOLVES, 0);
    open_registration_page(&mut s, P3, merchant);
    s.take();
    s.script(P3, "FortressSiegeManager register");
    let requests = reread_requests(&s.take());
    assert_eq!(
        requests,
        [ResidenceReadThen::Fort(FortThen::Register {
            player: ObjectId(P3),
            npc: merchant
        })]
    );
    answer_reread(&mut s, requests[0].clone(), LIONS, vec![], None);
    let out = s.take();
    let registered: Vec<SystemMessage> = messages_to(&out, P3)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::YourClanHasBeenRegisteredToS1SFortressBattle)
        .collect();
    assert_eq!(registered.len(), 1);
    assert_eq!(text_param(&registered[0], 0), "Shanty");
    assert_eq!(
        fort_writes(&out)[0],
        FortSiegeWrite::AddAttacker {
            fort_id: 101,
            clan_id: WOLVES
        }
    );
}

/// The refusals of `onEvent` "register", in Java's order.
#[test]
fn the_siege_manager_refuses_a_clan_for_the_reasons_of_java() {
    // No clan.
    let (mut s, merchant) = manager_scene(shanty(LIONS), settings());
    s.enter(GM, 0, 0);
    open_registration_page(&mut s, GM, merchant);
    assert!(siege_command(&mut s, GM, "register", vec![])[0].contains("PAGE 02"));
    // A member without the siege privilege.
    s.enter(P4, WOLVES, 0);
    open_registration_page(&mut s, P4, merchant);
    assert!(siege_command(&mut s, P4, "register", vec![])[0].contains("PAGE 10"));
    // A clan below `SiegeClanMinLevel` (4): the Bears are level 3.
    s.enter(P5, BEARS, 0);
    open_registration_page(&mut s, P5, merchant);
    assert!(siege_command(&mut s, P5, "register", vec![])[0].contains("PAGE 04"));
    // The owner's clan: the owner page.
    s.enter(P1, LIONS, 0);
    open_registration_page(&mut s, P1, merchant);
    assert!(siege_command(&mut s, P1, "register", vec![])[0].contains("OWNER PAGE Lions"));
    // Not enough adena for the first clan.
    let mut poor = pdata(P3, WOLVES, X, 0);
    poor.items[0].count = 1000;
    s.enter_with(poor);
    open_registration_page(&mut s, P3, merchant);
    assert!(siege_command(&mut s, P3, "register", vec![])[0].contains("PAGE 03"));
}

#[test]
fn the_siege_manager_refuses_the_lord_of_the_contracted_castle_and_a_late_registration() {
    // The Bears own Gludio, the castle of the contract.
    let mut low = settings();
    low.siege.siege_clan_min_level = 3;
    let fort = FortRecord {
        state: 2,
        castle_id: 1,
        ..shanty(LIONS)
    };
    let (mut s, merchant) = manager_scene(fort, low.clone());
    s.enter(P5, BEARS, 0);
    open_registration_page(&mut s, P5, merchant);
    assert!(siege_command(&mut s, P5, "register", vec![])[0].contains("PAGE 18"));
    // Seven days after the owner took it, the rebel army comes within two hours.
    let fort = FortRecord {
        last_owned_time: now_ms() - 167 * 3_600_000,
        ..shanty(LIONS)
    };
    let (mut s, merchant) = manager_scene(fort, settings());
    s.enter(P3, WOLVES, 0);
    open_registration_page(&mut s, P3, merchant);
    assert!(siege_command(&mut s, P3, "register", vec![])[0].contains("PAGE 19"));
    // Without a contract the castle's lord registers like everybody else.
    let (mut s, merchant) = manager_scene(shanty(LIONS), low);
    s.enter(P5, BEARS, 0);
    open_registration_page(&mut s, P5, merchant);
    assert!(siege_command(&mut s, P5, "register", vec![])[0].contains("PAGE 05"));
}

/// A clan that the rows say is registered already gets page 06 and pays nothing.
#[test]
fn the_siege_manager_reads_the_registrations_before_it_registers() {
    let (mut s, merchant) = manager_scene(shanty(LIONS), settings());
    s.enter(P3, WOLVES, 0);
    open_registration_page(&mut s, P3, merchant);
    let before = adena_of(&s, P3);
    // The database (the Java game server) registered the Wolves meanwhile.
    let pages = siege_command(&mut s, P3, "register", vec![WOLVES]);
    assert!(pages[0].contains("PAGE 06"), "{pages:?}");
    assert_eq!(adena_of(&s, P3), before);
    assert_eq!(s.world.fort_attacker_clans(SHANTY), [WOLVES]);
}

#[test]
fn the_siege_manager_cancels_and_tells_the_war_info() {
    let (mut s, merchant) = manager_scene(shanty(LIONS), settings());
    s.enter(P3, WOLVES, 0);
    s.enter(P4, WOLVES, 0);
    open_registration_page(&mut s, P3, merchant);
    // Not registered.
    assert!(siege_command(&mut s, P3, "cancel", vec![])[0].contains("PAGE 09"));
    open_registration_page(&mut s, P3, merchant);
    assert!(siege_command(&mut s, P3, "warInfo", vec![])[0].contains("PAGE 20"));
    open_registration_page(&mut s, P3, merchant);
    assert!(siege_command(&mut s, P3, "warInfo", vec![WOLVES])[0].contains("PAGE 21"));
    // Registered: a member without the privilege cannot cancel, the leader can.
    open_registration_page(&mut s, P4, merchant);
    assert!(siege_command(&mut s, P4, "cancel", vec![WOLVES])[0].contains("PAGE 10"));
    open_registration_page(&mut s, P3, merchant);
    let pages = siege_command(&mut s, P3, "cancel", vec![WOLVES]);
    assert!(pages[0].contains("PAGE 08"), "{pages:?}");
    assert!(s.world.fort_attacker_clans(SHANTY).is_empty());
}

/// A registration this world holds without a row (the switch is off) survives the read of the
/// rows; with the switch on the rows are the truth.
#[test]
fn a_local_registration_survives_a_reread_only_without_the_switch() {
    for switch in [false, true] {
        let mut fort = shanty(LIONS);
        fort.siege_date = now_ms() + 30 * 60_000;
        let mut s = Scene::with(switch, fort, vec![], settings());
        s.enter(P3, WOLVES, 0);
        assert_eq!(s.world.fort_add_attacker(SHANTY, ObjectId(P3), false), 4);
        // The rows know the Bears only.
        answer_reread(&mut s, ResidenceReadThen::Nothing, LIONS, vec![BEARS], None);
        let clans = s.world.fort_attacker_clans(SHANTY);
        if switch {
            assert_eq!(clans, [BEARS]);
        } else {
            assert_eq!(clans, [BEARS, WOLVES]);
        }
    }
}

/// `FortressArcherCaptain`: a page for the owner's clan, another for everybody else.
#[test]
fn the_archer_captain_has_two_pages() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let captain = s.one_npc(NPC_CAPTAIN_ARCHER);
    s.enter(P1, LIONS, 0);
    s.enter(P3, WOLVES, 0);
    s.enter(GM, 0, 0);
    s.talk(P1, captain);
    assert!(pages_to(&s.take(), P1)[0].contains("CAPTAIN OWN"));
    s.talk(P3, captain);
    assert!(pages_to(&s.take(), P3)[0].contains("CAPTAIN OTHER"));
    s.talk(GM, captain);
    assert!(pages_to(&s.take(), GM)[0].contains("CAPTAIN OTHER"));
}

// ---------------------------------------------------------------------------------- windows

/// `ExShowFortressInfo`: every fortress with its owner's name, whether it is under siege and for
/// how long the owner has held it.
#[test]
fn the_fortress_list_names_owner_siege_and_time() {
    let mut s = Scene::new(true, LIONS, vec![WOLVES], now_ms() + 30 * 60_000);
    s.enter(P1, LIONS, 0);
    s.take();
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(P1).unwrap()),
        input: ClientInput::Fort(l2_world::FortInput::AllFortressInfo),
    });
    let out = s.take();
    let list = decode::<ExShowFortressInfo>(&packets_to(&out, P1, "EX_SHOW_FORTRESS_INFO")[0]);
    assert_eq!(list.forts.len(), 1);
    let fort = &list.forts[0];
    assert_eq!(
        (
            fort.residence_id,
            fort.owner_clan_name.as_str(),
            fort.siege_in_progress
        ),
        (101, "Lions", false)
    );
    assert!((fort.owned_time - 3600).abs() <= 2, "{}", fort.owned_time);
    s.world.fort_start_siege(SHANTY);
    assert!(s.world.exshow_fortress_info().forts[0].siege_in_progress);
}

/// `ExShowFortressSiegeInfo` and `ExShowFortressMapInfo`: the barracks, taken (1) or not (0).
#[test]
fn the_barracks_windows_follow_the_commanders() {
    let mut s = Scene::new(true, LIONS, vec![WOLVES], now_ms() + 30 * 60_000);
    s.enter(P1, LIONS, 0);
    let request = |s: &mut Scene, input| {
        s.take();
        s.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(P1).unwrap()),
            input: ClientInput::Fort(input),
        });
        s.take()
    };
    // No siege: no siege windows, and the map is all zeros.
    let out = request(&mut s, l2_world::FortInput::FortressSiegeInfo);
    assert!(packets_to(&out, P1, "EX_SHOW_FORTRESS_SIEGE_INFO").is_empty());
    let out = request(&mut s, l2_world::FortInput::FortressMapInfo { fortress_id: 101 });
    let map = decode::<ExShowFortressMapInfo>(&packets_to(&out, P1, "EX_SHOW_FORTRESS_MAP_INFO")[0]);
    assert_eq!(
        (map.fort_id, map.siege_in_progress, map.barracks_total),
        (101, false, 3)
    );
    assert_eq!(map.barracks, [0, 0, 0]);
    // The siege: three commanders alive.
    s.world.fort_start_siege(SHANTY);
    let out = request(&mut s, l2_world::FortInput::FortressSiegeInfo);
    let info = decode::<ExShowFortressSiegeInfo>(&packets_to(&out, P1, "EX_SHOW_FORTRESS_SIEGE_INFO")[0]);
    assert_eq!(
        (info.fort_id, info.barracks_total, info.barracks.clone()),
        (101, 3, vec![0])
    );
    let out = request(&mut s, l2_world::FortInput::FortressMapInfo { fortress_id: 101 });
    let map = decode::<ExShowFortressMapInfo>(&packets_to(&out, P1, "EX_SHOW_FORTRESS_MAP_INFO")[0]);
    assert_eq!(map.barracks, [0, 0, 0]);
    // The second commander dies: one barracks is taken.
    let c2 = s.one_npc(NPC_COMMANDER_2);
    s.world.do_die(c2, Some(ObjectId(P1)));
    let out = request(&mut s, l2_world::FortInput::FortressSiegeInfo);
    let info = decode::<ExShowFortressSiegeInfo>(&packets_to(&out, P1, "EX_SHOW_FORTRESS_SIEGE_INFO")[0]);
    assert_eq!(info.barracks, [1]);
    let out = request(&mut s, l2_world::FortInput::FortressMapInfo { fortress_id: 101 });
    let map = decode::<ExShowFortressMapInfo>(&packets_to(&out, P1, "EX_SHOW_FORTRESS_MAP_INFO")[0]);
    assert!(map.siege_in_progress);
    assert_eq!(map.barracks, [0, 1, 0]);
    // A fortress that does not exist: `ActionFailed`.
    let out = request(&mut s, l2_world::FortInput::FortressMapInfo { fortress_id: 5 });
    assert_eq!(packets_to(&out, P1, "ACTION_FAIL").len(), 1);
    assert!(packets_to(&out, P1, "EX_SHOW_FORTRESS_MAP_INFO").is_empty());
}

// ---------------------------------------------------------------------------------- //fortsiege

fn gm_command(s: &mut Scene, command: &str) -> Vec<Out> {
    s.take();
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(GM).unwrap()),
        input: ClientInput::BuildCmd {
            command: command.to_owned(),
        },
    });
    s.take()
}

/// `AdminFortSiege`: the fortress list, the fortress page, and the commands on a fortress.
#[test]
fn the_gm_runs_a_test_siege_with_fortsiege() {
    let mut s = Scene::new(false, LIONS, vec![], now_ms() + 30 * 60_000);
    s.enter(GM, 0, 100);
    s.enter(P3, WOLVES, 0);
    s.enter(P1, LIONS, 0);
    // The list of fortresses.
    let out = gm_command(&mut s, "fortsiege");
    let page = pages_to(&out, GM).remove(0);
    assert!(
        page.contains("FORTS ") && page.contains("Shanty id: 101"),
        "{page}"
    );
    // A fortress that does not exist shows the list too.
    assert!(pages_to(&gm_command(&mut s, "fortsiege 7"), GM)[0].contains("FORTS "));
    let page = pages_to(&gm_command(&mut s, "fortsiege 101"), GM).remove(0);
    assert!(page.contains("FORT Shanty 101"), "{page}");
    // The targeted player's clan is added as an attacker ...
    let out = gm_command(&mut s, "add_fortattacker 101");
    assert!(message_ids(&out, GM).contains(&SystemMessageId::ThatIsAnIncorrectTarget));
    s.world.set_target(ObjectId(GM), Some(ObjectId(P3)));
    let out = gm_command(&mut s, "add_fortattacker 101");
    assert_eq!(s.world.fort_attacker_clans(SHANTY), [WOLVES]);
    assert!(message_ids(&out, P3).contains(&SystemMessageId::YourClanHasBeenRegisteredToS1SFortressBattle));
    // ... the siege starts and ends at the GM's word, whatever the switch says ...
    gm_command(&mut s, "startfortsiege 101");
    assert!(s.world.fort_siege_in_progress(SHANTY));
    assert_eq!(s.npcs_of(NPC_COMMANDER_1).len(), 1);
    gm_command(&mut s, "endfortsiege 101");
    assert!(!s.world.fort_siege_in_progress(SHANTY));
    // ... the targeted clan takes the fortress, and loses it again (the teleport of the attackers
    // dropped the GM's target).
    s.appear(P3);
    s.world.set_target(ObjectId(GM), Some(ObjectId(P3)));
    gm_command(&mut s, "setfort 101");
    assert_eq!(s.world.fort(SHANTY).unwrap().owner_id, WOLVES);
    gm_command(&mut s, "removefort 101");
    assert_eq!(s.world.fort(SHANTY).unwrap().owner_id, 0);
    // Registrations are cleared.
    gm_command(&mut s, "add_fortattacker 101");
    assert_eq!(s.world.fort_attacker_clans(SHANTY), [WOLVES]);
    gm_command(&mut s, "clear_fortsiege_list 101");
    assert!(s.world.fort_attacker_clans(SHANTY).is_empty());
    // A plain player cannot.
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(P3).unwrap()),
        input: ClientInput::BuildCmd {
            command: "startfortsiege 101".to_owned(),
        },
    });
    assert!(!s.world.fort_siege_in_progress(SHANTY));
}

// ---------------------------------------------------------------------------------- the rest

/// `EnterWorld`: a member of a clan in a siege that is going on gets the siege state; entering
/// the active zone marks the player as in the siege.
#[test]
fn entering_the_world_during_a_siege_sets_the_state() {
    let mut s = siege_scene();
    s.world.fort_start_siege(SHANTY);
    // An attacker who logs in inside a siege zone goes to town (`EnterWorld`, M7-SIEGE-01), so
    // the player logs in outside and comes in.
    s.enter_at(P3, WOLVES, X + 10_000, 0);
    assert_eq!(s.world.fort_siege_state_of(ObjectId(P3)), 1);
    assert!(!s.world.fort_player_in_siege(ObjectId(P3)));
    s.teleport(P3, X, Y, Z);
    assert!(s.world.fort_player_in_siege(ObjectId(P3)));
    assert!(s.world.is_registered_on_this_siege_field(ObjectId(P3), 101));
    assert!(!s.world.is_registered_on_this_siege_field(ObjectId(P3), 102));
    // The state is gone with the player.
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(P3).unwrap()),
    });
    assert_eq!(s.world.fort_siege_state_of(ObjectId(P3)), 0);
}

/// `SiegeZone.onPlayerLogoutInside`: whoever logs out in the zone of a fortress is brought to
/// town before the character is stored.
#[test]
fn logging_out_inside_the_zone_stores_the_position_in_town() {
    let mut s = siege_scene();
    s.world.fort_start_siege(SHANTY);
    s.take();
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(P5).unwrap()),
    });
    let out = s.take();
    let stored = out
        .iter()
        .find_map(|o| match o {
            Out::Persist(PersistCommand::StorePlayer(data)) => Some(data.clone()),
            _ => None,
        })
        .expect("the character is stored");
    assert!(
        (stored.x + 84_318).abs() <= 60 && (stored.y - 244_579).abs() <= 60,
        "{} {}",
        stored.x,
        stored.y
    );
}

/// D-50: the rows of a fortress read again replace its owner; the owner's updater follows.
#[test]
fn a_reread_that_moves_the_owner_moves_the_updater_too() {
    let mut s = Scene::new(false, LIONS, vec![], now_ms() + 30 * 60_000);
    s.enter(P3, WOLVES, 0);
    assert!(s.world.fort_time_till_next_update(SHANTY) > 0);
    // Sieghardt decided: the Wolves own Shanty.
    answer_reread(&mut s, ResidenceReadThen::Nothing, WOLVES, vec![], Some(3));
    assert_eq!(s.world.fort(SHANTY).unwrap().owner_id, WOLVES);
    assert_eq!((s.clan(WOLVES).fort_id, s.clan(LIONS).fort_id), (101, 0));
    assert_eq!(blood_oaths(&s, WOLVES), 3);
    assert!(s.world.fort_time_till_next_update(SHANTY) > 0);
    // The owner is the same afterwards: the tasks run on.
    let after = s.world.fort_time_till_next_update(SHANTY);
    answer_reread(&mut s, ResidenceReadThen::Nothing, WOLVES, vec![], Some(3));
    assert_eq!(s.world.fort_time_till_next_update(SHANTY), after);
    // No owner at all.
    answer_reread(&mut s, ResidenceReadThen::Nothing, 0, vec![], None);
    assert_eq!(s.world.fort(SHANTY).unwrap().owner_id, 0);
    assert_eq!(s.world.fort_time_till_next_update(SHANTY), 0);
}

/// `Defender.isAutoAttackable` in a fortress: during the siege by everybody but the owners
/// registered on this field.
#[test]
fn siege_guards_are_attackable_during_the_siege_except_for_the_owners() {
    let mut s = siege_scene();
    s.enter(P3, WOLVES, 0);
    s.world.fort_start_siege(SHANTY);
    let guard = s.npcs_of(NPC_SIEGE_GUARD)[0];
    assert!(s.world.is_auto_attackable_by(guard, ObjectId(P3)), "an attacker");
    assert!(s.world.is_auto_attackable_by(guard, ObjectId(P5)), "a bystander");
    assert!(!s.world.is_auto_attackable_by(guard, ObjectId(P1)), "an owner");
    s.world.fort_end_siege(SHANTY);
    assert!(!s.world.is_auto_attackable_by(guard, ObjectId(P3)), "no siege");
}

/// `Player.getRelation`: the siege bits of a fortress siege (attacker against defender).
#[test]
fn the_siege_relation_bits_follow_the_fortress_siege() {
    const IN_SIEGE: i32 = 0x200;
    const ATTACKER: i32 = 0x400;
    const ALLY: i32 = 0x800;
    const ENEMY: i32 = 0x1000;
    let mut s = siege_scene();
    s.enter(P3, WOLVES, 0);
    assert_eq!(
        s.world.relation(ObjectId(P3), ObjectId(P1)) & (IN_SIEGE | ENEMY),
        0
    );
    s.world.fort_start_siege(SHANTY);
    let attacker_seen_by_owner = s.world.relation(ObjectId(P3), ObjectId(P1));
    assert_eq!(
        attacker_seen_by_owner & (IN_SIEGE | ENEMY | ATTACKER | ALLY),
        IN_SIEGE | ENEMY | ATTACKER
    );
    let owner_seen_by_attacker = s.world.relation(ObjectId(P1), ObjectId(P3));
    assert_eq!(
        owner_seen_by_attacker & (IN_SIEGE | ENEMY | ATTACKER | ALLY),
        IN_SIEGE | ENEMY
    );
    // Two attackers are allies.
    let other = s.world.relation(ObjectId(P3), ObjectId(P4));
    assert_eq!(
        other & (IN_SIEGE | ENEMY | ATTACKER | ALLY),
        IN_SIEGE | ALLY | ATTACKER
    );
}

/// The teleport function: the list of the level (`tel<level>`), and `goto` (only with the
/// function) takes the player to the place.
#[test]
fn the_foreman_teleports_with_the_function() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let foreman = place(&mut s, NPC_FOREMAN, 80);
    s.enter(P1, LIONS, 0);
    s.talk(P1, foreman);
    walk(
        &mut s,
        P1,
        foreman,
        &["manage", "manage other", "manage other edit_tele 1"],
    );
    assert!(walk(&mut s, P1, foreman, &["manage other tele 1"]).contains("APPLY CONFIRMED"));
    let f = s.world.residence_function(ResidenceRef::Fort(SHANTY), 1).unwrap();
    assert_eq!((f.lvl, f.lease, f.rate), (1, 1000, 604_800_000));
    s.talk(P1, foreman);
    walk(&mut s, P1, foreman, &["functions"]);
    let page = walk(&mut s, P1, foreman, &["functions tele"]);
    assert!(
        page.contains(&format!("npc_{}_goto tel1 0", foreman.0)) && page.contains("Barracks"),
        "{page}"
    );
    walk(&mut s, P1, foreman, &["goto tel1 0"]);
    s.appear(P1);
    assert_eq!(
        s.world.position_of(ObjectId(P1)).map(|p| (p.0, p.1)),
        Some((-70_000, 258_000))
    );
}

/// `Doorman` `tele <n>`: the list named after the type, for the owner clan.
#[test]
fn the_doorman_teleports_the_owner() {
    let mut s = Scene::new(true, LIONS, vec![], now_ms() + 30 * 60_000);
    let doorman = place(&mut s, NPC_DOORMAN, 60);
    s.enter(P1, LIONS, 0);
    s.talk(P1, doorman);
    s.take();
    s.bypass(P1, doorman, "tele 1");
    s.appear(P1);
    assert_eq!(
        s.world.position_of(ObjectId(P1)).map(|p| (p.0, p.1)),
        Some((-70_600, 258_600))
    );
}

/// The headquarters (`HeadquarterCreate`, `SiegeFlag`) serves the fortress sieges as well
/// (M7-SIEGE-01): it joins the attacker's flags (`fort_siege_flags`), a dead member of the clan
/// gets the "to siege HQ" button and restarts at it, and it leaves the list when it dies.
#[test]
fn a_headquarters_of_a_fortress_siege_takes_the_dead_back() {
    let mut s = siege_scene();
    s.world.fort_start_siege(SHANTY);
    s.enter_at(P3, WOLVES, X + 10_000, 0);
    s.teleport(P3, X, Y, Z);
    s.take();
    s.world.siege_create_headquarters(ObjectId(P3), false);
    let flags = s.world.fort_siege_flags(SHANTY, WOLVES);
    assert_eq!(flags, s.npcs_of(NPC_HQ));
    assert_eq!(flags.len(), 1);
    assert_eq!(s.world.npc_custom_title(flags[0]), Some("Wolves"));

    s.teleport(P4, X + 200, Y, Z);
    s.world.do_die(ObjectId(P4), Some(ObjectId(P1)));
    let die = s.world.die_packet(ObjectId(P4));
    assert!(die.to_siege_hq);
    assert!(!die.to_fortress);
    // The owner's members defend the fortress.
    s.world.do_die(ObjectId(P1), Some(ObjectId(P4)));
    let die = s.world.die_packet(ObjectId(P1));
    assert!(die.to_fortress && !die.to_siege_hq);
    s.take();
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(P4).unwrap()),
        input: ClientInput::RestartPoint { point_type: 4 },
    });
    s.run_ms(50);
    let out = s.take();
    assert!(!packets_to(&out, P4, "TELEPORT_TO_LOCATION").is_empty());

    s.world.do_die(flags[0], Some(ObjectId(P1)));
    assert!(s.world.fort_siege_flags(SHANTY, WOLVES).is_empty());
    assert!(s.npcs_of(NPC_HQ).is_empty());
}

/// `Defender.initAI`: no castle and no siegable hall within 10000, so a fortress guard thinks
/// with `FortSiegeGuardAI` (M7-SIEGE-02). Its `attackPrepare` never attacks a defender of the
/// fortress (`getFort().getSiege().checkIsDefender`, the owner's clan): sent at the owner it stops
/// hating him and gives up; sent at a bystander it keeps him as its target.
#[test]
fn fortress_guards_think_with_the_fort_guard_ai_and_spare_the_owners() {
    use l2_world::ai::siege_guard::SiegeGuardAiKind;
    use l2_world::movement::Intention;
    let mut s = siege_scene();
    s.world.fort_start_siege(SHANTY);
    s.take();
    let guard = s.npcs_of(NPC_SIEGE_GUARD)[0];
    let state = s
        .world
        .siege_guard_state(guard)
        .expect("the players around made its AI");
    assert_eq!(state.kind, SiegeGuardAiKind::Fort);
    s.world.set_intention_attack(guard, ObjectId(P1));
    assert_ne!(s.world.intention_of(guard), Intention::Attack, "never the owner");
    assert_ne!(s.world.attack_target_of(guard), Some(ObjectId(P1)));
    s.world.set_intention_attack(guard, ObjectId(P5));
    assert_eq!(s.world.intention_of(guard), Intention::Attack);
    assert_eq!(s.world.attack_target_of(guard), Some(ObjectId(P5)));
}

// ---------------------------------------------------------------------------------- canTakeFort

/// `ConditionPlayerCanTakeFort`: the leader of an attacker, on the fortress's own flag pole.
/// Everything else tells the player why not (`INVALID_TARGET`, "unsuitable terms").
#[test]
fn can_take_fort_wants_the_leader_of_an_attacker_at_the_flag_pole() {
    let mut s = flag_scene();
    let pole = s.world.fort(SHANTY).unwrap().flag_pole.unwrap();
    s.take();
    // The leader of the Wolves, next to the pole.
    assert!(s.world.cond_can_take_fort(ObjectId(P3), Some(pole), Some((3318, 1))));
    assert!(s.take().is_empty());
    // Not the pole: "Invalid target".
    let other = s.npcs_of(NPC_SIEGE_GUARD)[0];
    assert!(!s.world.cond_can_take_fort(ObjectId(P3), Some(other), Some((3318, 1))));
    assert!(!s.world.cond_can_take_fort(ObjectId(P3), None, Some((3318, 1))));
    let out = s.take();
    assert_eq!(
        message_ids(&out, P3),
        [SystemMessageId::InvalidTarget, SystemMessageId::InvalidTarget]
    );
    // Too far from the pole.
    s.teleport(P3, X + 500, Y, Z);
    s.take();
    assert!(!s.world.cond_can_take_fort(ObjectId(P3), Some(pole), Some((3318, 1))));
    assert_eq!(
        message_ids(&s.take(), P3),
        [SystemMessageId::TheDistanceIsTooFarAndSoTheCastingHasBeenStopped]
    );
    // A clan that does not attack: "unsuitable terms", with the skill's name.
    s.enter(P5, BEARS, 0);
    s.take();
    assert!(!s.world.cond_can_take_fort(ObjectId(P5), Some(pole), Some((3318, 1))));
    let out = s.take();
    let unsuitable: Vec<SystemMessage> = messages_to(&out, P5);
    assert_eq!(unsuitable.len(), 1);
    assert_eq!(unsuitable[0].id, SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms);
}
