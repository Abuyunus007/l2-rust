//! What the M7 end-to-end test adds to the datapack of `datapack.rs`, `m3_data.rs` to `m6_data.rs`:
//! the castle Gludio with its siege zone, headquarters zone, gate and holy artefact, the price
//! config that ties the town of the start (and its smith) to Gludio's tax, a clan hall with its
//! zone, auctioneer and manager, a fortress with its zones, doors and flag pole, the siege skills
//! and the admin rights.
//!
//! CI has no datapack, so everything is written into the temporary folder of the test. The shapes
//! are those of the shipped files (`data/zones/castle_*.xml`, `data/Doors.xml`,
//! `data/StaticObjects.xml`, `data/stats/npcs`, `config/SiegeSchedule.xml`, ...); the numbers are
//! made up and small, and where a skill differs from the shipped one the comment says so.
//!
//! Everything stands in the geodata tile of the start (17_25) next to the town of `m3_data.rs`:
//! the castle west of it, the fortress south, the hall east. A siege sends the attackers who
//! stand in its zone to the closest town (the start), and they walk back from there.

use std::path::Path;

use super::m3_data::{GROUND_Z, START};

/// Z of every place of this scenario (the geodata is flat).
pub const Z: i32 = GROUND_Z;

// ---- the castle ----------------------------------------------------------------------------------

pub const GLUDIO: i32 = 1;
/// The fortress and the hall of the scenarios (`m7_seed.rs` has the rows).
pub const FORT_ID: i32 = 101;
pub const HALL_ID: i32 = 22;
/// The middle of Gludio's siege zone: 3800 west of the start, so the zone ends just outside the
/// town box of the start (1200 west of it).
pub const CASTLE: (i32, i32) = (START.0 - 3_800, START.1);
/// Half the side of the siege zone and of the castle zone.
const SIEGE_ZONE_HALF: i32 = 2_500;
const CASTLE_ZONE_HALF: i32 = 1_000;
/// The castle gate (the shipped Gludio gates are `19210001` and up).
pub const GATE_ID: i32 = 19_210_001;
pub const GATE_POS: (i32, i32, i32) = (CASTLE.0 + 800, CASTLE.1, Z);
/// The gate's HP. A level 10 fighter with the Squire's Sword hits for 7 to 10, so a few swings
/// break it.
pub const GATE_HP: i32 = 40;
/// Its physical defence: with it a hit of the sword does a few points of damage.
pub const GATE_PDEF: i32 = 100;
/// `35063` Holy Artifact of Gludio.
pub const ARTEFACT_ID: i32 = 35_063;
pub const ARTEFACT_POS: (i32, i32, i32) = (CASTLE.0 + 1_700, CASTLE.1, Z);
/// `35062` the headquarters of the clan leader who attacks.
pub const HQ_ID: i32 = 35_062;
/// `3960` Mercenary Posting Ticket (Sword/Stationary) of Gludio and `35010`, the mercenary it
/// hires. The lord uses the ticket inside the castle; the mercenary stands as a guard when a siege
/// starts.
pub const MERC_TICKET: i32 = 3_960;
pub const MERC_ID: i32 = 35_010;
/// The lord stands inside the castle and posts the mercenary where he stands.
pub const LORD_POS: (i32, i32, i32) = (CASTLE.0 - 600, CASTLE.1 + 300, Z);
/// Where the attacker's leader stands when he enters: 100 east of the siege zone, so the start of
/// the siege does not send him to town. He walks west from there.
pub const RAIDER_START: (i32, i32, i32) = (CASTLE.0 + SIEGE_ZONE_HALF + 100, CASTLE.1, Z);
/// Inside the headquarters zone and within 200 of the artefact: where he builds the headquarters
/// and engraves.
pub const RAIDER_POS: (i32, i32, i32) = (CASTLE.0 + 1_600, CASTLE.1, Z);
/// 25 east of the gate: within reach of the sword. The test sword has no `pAtkRange`, so a hit
/// needs the distance below the player's radius (9) plus the gate's (20).
pub const GATE_APPROACH: (i32, i32, i32) = (CASTLE.0 + 825, CASTLE.1, Z);
/// Where the GM stands.
pub const GM_POS: (i32, i32, i32) = (CASTLE.0 - 300, CASTLE.1 + 100, Z);
/// The next siege of Gludio: ten days from the start of the test (set when the rows are seeded).
pub const DAY_MS: i64 = 86_400_000;

// ---- skills --------------------------------------------------------------------------------------

/// `246` Seal of Ruler. NOTE: the shipped one casts for 180 s; here 2 s.
pub const SEAL_OF_RULER: i32 = 246;
pub const SEAL_CAST_MS: i32 = 2_000;
/// `247` Build Headquarters. NOTE: the shipped one consumes 300 C-grade gemstones; here none.
pub const BUILD_HQ: i32 = 247;
pub const BUILD_HQ_CAST_MS: i32 = 1_000;

// ---- the town of the start and the castle's tax -------------------------------------------------

/// The town zone of the start (`m3_data.rs`), which the shipped `MerchantPriceConfig.xml` ties to
/// Gludio ("Talking Island", `castleId` 1).
pub const TOWN_ZONE_ID: i32 = 11_022;
/// `baseTax` of the town (the shipped one has 15) and the tax percent of the castle row.
pub const BASE_TAX: i32 = 10;
pub const CASTLE_TAX: i32 = 10;

// ---- the fortress --------------------------------------------------------------------------------

/// The middle of the siege zone of Shanty fortress (`fortId` 101).
pub const FORT: (i32, i32) = (START.0, START.1 - 5_000);
const FORT_SIEGE_HALF: i32 = 2_000;
const FORT_ZONE_HALF: i32 = 800;
/// `35663` the foreman (a `FortManager`), always there.
pub const FORT_FOREMAN_ID: i32 = 35_663;
pub const FORT_FOREMAN_POS: (i32, i32, i32) = (FORT.0 - 300, FORT.1 + 300, Z);
/// `35659` the suspicious merchant of Shanty, who registers a clan for the siege
/// (`FortressSiegeManager`). He stands outside the siege zone.
pub const FORT_MERCHANT_ID: i32 = 35_659;
pub const FORT_MERCHANT_POS: (i32, i32, i32) = (FORT.0 - 2_400, FORT.1, Z);
/// Where the raider stands to talk to the merchant.
pub const FORT_REGISTER_POS: (i32, i32, i32) = (FORT.0 - 2_350, FORT.1, Z);
/// `35670` a guard of the siege.
pub const FORT_GUARD_ID: i32 = 35_670;
pub const FORT_GUARD_POS: (i32, i32, i32) = (FORT.0 - 100, FORT.1 - 300, Z);
/// The commanders of the siege (`FortSiege.ini`) and the fortress gate.
pub const FORT_COMMANDER_1: i32 = 35_683;
pub const FORT_COMMANDER_1_POS: (i32, i32, i32) = (FORT.0 + 300, FORT.1 + 200, Z);
pub const FORT_COMMANDER_2: i32 = 35_677;
pub const FORT_COMMANDER_2_POS: (i32, i32, i32) = (FORT.0 + 300, FORT.1 - 200, Z);
pub const FORT_GATE_ID: i32 = 22_140_001;
pub const FORT_GATE_POS: (i32, i32, i32) = (FORT.0 + 450, FORT.1, Z);
/// `9819` the combat flag, lying at this place after the last commander fell, and `3318` Flag
/// Display, the skill that hangs it on the flag pole.
pub const COMBAT_FLAG_ITEM: i32 = 9_819;
pub const FORT_FLAG_POS: (i32, i32, i32) = (FORT.0 + 600, FORT.1 + 100, Z);
pub const FLAG_DISPLAY: i32 = 3_318;
/// NOTE: the shipped skill casts for 180 s; here 2 s.
pub const FLAG_DISPLAY_CAST_MS: i32 = 2_000;
/// The flag pole (a static object of type 3 named like the fortress).
pub const FLAG_POLE_POS: (i32, i32, i32) = (FORT.0 + 900, FORT.1, Z);
pub const FLAG_POLE_STATIC_ID: i32 = 18_220_500;
/// Where the GM stands in the fortress.
pub const FORT_GM_POS: (i32, i32, i32) = (FORT.0 + 400, FORT.1, Z);

// ---- the clan hall -------------------------------------------------------------------------------

/// The middle of the zone of the hall `22`.
pub const HALL_CENTER: (i32, i32) = (START.0 + 3_000, START.1);
/// `35437` the hall's manager (`ClanHallManager`), inside the hall's zone.
pub const HALL_MANAGER_ID: i32 = 35_437;
pub const HALL_MANAGER_POS: (i32, i32, i32) = (HALL_CENTER.0, HALL_CENTER.1 + 100, Z);
/// `30767` the auctioneer.
pub const AUCTIONEER_ID: i32 = 30_767;
pub const AUCTIONEER_POS: (i32, i32, i32) = (HALL_CENTER.0 + 150, HALL_CENTER.1 - 100, Z);
/// Where the bidder (and later the owner) stands: within talking range of the auctioneer and of
/// the manager.
pub const BIDDER_POS: (i32, i32, i32) = (HALL_CENTER.0 + 80, HALL_CENTER.1, Z);

// ---- the admin rights ----------------------------------------------------------------------------

/// The GM's access level.
pub const GM_LEVEL: i32 = 100;
/// Every admin word of the scenario.
const ADMIN_COMMANDS: &[&str] = &[
    "admin_siege",
    "admin_add_attacker",
    "admin_add_defender",
    "admin_add_guard",
    "admin_list_siege_clans",
    "admin_clear_siege_list",
    "admin_move_defenders",
    "admin_spawn_doors",
    "admin_endsiege",
    "admin_startsiege",
    "admin_setsiegetime",
    "admin_setcastle",
    "admin_removecastle",
    "admin_fortsiege",
    "admin_add_fortattacker",
    "admin_list_fortsiege_clans",
    "admin_clear_fortsiege_list",
    "admin_spawn_fortdoors",
    "admin_endfortsiege",
    "admin_startfortsiege",
    "admin_setfort",
    "admin_removefort",
    "admin_clanhall",
    "admin_clanhallset",
    "admin_clanhalldel",
    "admin_clanhallopendoors",
    "admin_clanhallclosedoors",
    "admin_clanhallteleportself",
    "admin_move_to",
    "admin_recall",
    "admin_teleport_character",
    "admin_kill",
    "admin_create_item",
    "admin_itemcreate",
];

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// `AccessLevels.xml` and `AdminCommands.xml` of the config folder.
pub fn write_admin_config(config_dir: &Path) {
    write(
        config_dir,
        "AccessLevels.xml",
        &format!(
            "<list>\n\t<access level=\"0\" name=\"User\" nameColor=\"FFFFFF\" titleColor=\"ECF9A2\" isGM=\"false\" />\n\
             \t<access level=\"{GM_LEVEL}\" name=\"Master\" nameColor=\"00CCFF\" titleColor=\"0FF000\" isGM=\"true\" />\n</list>"
        ),
    );
    let commands: String = ADMIN_COMMANDS
        .iter()
        .map(|word| {
            // `AdminCommands.xml` names the words without `admin_` (Java `AdminData`).
            let word = word.trim_start_matches("admin_");
            format!("\t<admin command=\"{word}\" accessLevel=\"{GM_LEVEL}\" />\n")
        })
        .collect();
    write(config_dir, "AdminCommands.xml", &format!("<list>\n{commands}</list>"));
}

// ---- NPCs ----------------------------------------------------------------------------------------

/// One NPC template in the shape of the shipped files. `attackable` is the `<status>` of the NPC.
fn npc(id: i32, kind: &str, name: &str, attackable: bool) -> String {
    format!(
        r#"	<npc id="{id}" level="70" type="{kind}" name="{name}" title="">
		<race>HUMAN</race>
		<sex>MALE</sex>
		<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
			<vitals hp="3000" hpRegen="2" mp="400" mpRegen="0.9" />
			<attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
			<defence physical="44.44444" magical="29.5" />
			<speed><walk ground="40" /><run ground="100" /></speed>
		</stats>
		<ai aggroRange="0" isAggressive="false" />
		<status attackable="{attackable}" />
		<collision><radius normal="10" /><height normal="20" /></collision>
	</npc>
"#
    )
}

fn spawn(id: i32, pos: (i32, i32, i32)) -> String {
    format!(
        "\t<spawn name=\"m7_{id}\">\n\t\t<npc id=\"{id}\" x=\"{}\" y=\"{}\" z=\"{}\" heading=\"0\" respawnDelay=\"60\" />\n\t</spawn>\n",
        pos.0, pos.1, pos.2
    )
}

// ---- zones ---------------------------------------------------------------------------------------

/// A box zone around `(x, y)`: the two corners of a `Cuboid`.
fn cuboid(x: i32, y: i32, half_x: i32, half_y: i32) -> String {
    format!(
        "<node X=\"{}\" Y=\"{}\" /><node X=\"{}\" Y=\"{}\" />",
        x - half_x,
        y - half_y,
        x + half_x,
        y + half_y
    )
}

fn zones() -> String {
    let (cx, cy) = CASTLE;
    let hq_x = cx + 1_600;
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<zone name="gludio_castle_battlefield_territory" type="SiegeZone" shape="Cuboid" minZ="-3500" maxZ="-2700">
		<stat name="castleId" val="{GLUDIO}" />
		{siege}
	</zone>
	<zone name="gludio_castle" type="CastleZone" shape="Cuboid" minZ="-3500" maxZ="-2700">
		<stat name="castleId" val="{GLUDIO}" />
		{castle}
		<spawn X="{owner_x}" Y="{cy}" Z="{Z}" />
		<spawn X="{town_x}" Y="{cy}" Z="{Z}" type="other" />
		<spawn X="{town_x}" Y="{cy}" Z="{Z}" type="chaotic" />
	</zone>
	<zone name="gludio_castle_headquarter_territory1" type="HqZone" shape="Cuboid" minZ="-3500" maxZ="-2700">
		<stat name="castleId" val="{GLUDIO}" />
		{hq}
	</zone>
	<zone name="gludio_castle_owner_restart_territory" type="ResidenceTeleportZone" shape="Cuboid" minZ="-3500" maxZ="-2700">
		<stat name="residenceId" val="{GLUDIO}" />
		{tele}
		<spawn X="{owner_x}" Y="{cy}" Z="{Z}" />
	</zone>
	<zone name="shanty_fort_siege" type="SiegeZone" shape="Cuboid" minZ="-3500" maxZ="-2700">
		<stat name="fortId" val="{FORT_ID}" />
		{fort_siege}
	</zone>
	<zone name="shanty_fort" type="FortZone" shape="Cuboid" minZ="-3500" maxZ="-2700">
		<stat name="fortId" val="{FORT_ID}" />
		{fort_zone}
		<spawn X="{fort_x}" Y="{fort_y}" Z="{Z}" />
		<spawn X="{town_x}" Y="{cy}" Z="{Z}" type="other" />
		<spawn X="{town_x}" Y="{cy}" Z="{Z}" type="chaotic" />
	</zone>
	<zone name="gludio_castle_agit_001" type="ClanHallZone" shape="Cuboid" minZ="-3500" maxZ="-2700">
		<stat name="clanHallId" val="{HALL_ID}" />
		{hall_zone}
		<spawn X="{hall_x}" Y="{hall_y}" Z="{Z}" />
		<spawn X="{hall_out_x}" Y="{hall_y}" Z="{Z}" type="other" />
		<spawn X="{hall_out_x}" Y="{hall_y}" Z="{Z}" type="banish" />
	</zone>
</list>"#,
        siege = cuboid(cx, cy, SIEGE_ZONE_HALF, SIEGE_ZONE_HALF),
        castle = cuboid(cx - 500, cy, CASTLE_ZONE_HALF, CASTLE_ZONE_HALF),
        owner_x = cx - 500,
        town_x = START.0,
        hq = cuboid(hq_x, cy, 400, 400),
        tele = cuboid(cx - 800, cy, 100, 100),
        fort_siege = cuboid(FORT.0, FORT.1, FORT_SIEGE_HALF, FORT_SIEGE_HALF),
        fort_zone = cuboid(FORT.0, FORT.1, FORT_ZONE_HALF, FORT_ZONE_HALF),
        fort_x = FORT.0 - 500,
        fort_y = FORT.1,
        hall_zone = cuboid(HALL_CENTER.0, HALL_CENTER.1, 300, 300),
        hall_x = HALL_CENTER.0,
        hall_y = HALL_CENTER.1,
        hall_out_x = HALL_CENTER.0 + 450,
    )
}

// ---- skills --------------------------------------------------------------------------------------

fn skills() -> String {
    format!(
        r##"<list>
	<skill id="{SEAL_OF_RULER}" levels="1" name="Seal of Ruler">
		<icon>icon.skill0246</icon>
		<operateType>A1</operateType>
		<targetType>HOLY</targetType>
		<castRange>85</castRange>
		<effectRange>400</effectRange>
		<hitTime>{SEAL_CAST_MS}</hitTime>
		<isMagic>2</isMagic>
		<magicLevel>30</magicLevel>
		<mpInitialConsume>50</mpInitialConsume>
		<reuseDelay>5000</reuseDelay>
		<conditions>
			<player canTakeCastle="true" />
		</conditions>
		<effects>
			<effect name="TakeCastle" />
		</effects>
	</skill>
	<skill id="{BUILD_HQ}" levels="1" name="Build Headquarters">
		<icon>icon.skill0247</icon>
		<operateType>A1</operateType>
		<targetType>SELF</targetType>
		<hitTime>{BUILD_HQ_CAST_MS}</hitTime>
		<isMagic>2</isMagic>
		<magicLevel>30</magicLevel>
		<mpInitialConsume>30</mpInitialConsume>
		<reuseDelay>1800000</reuseDelay>
		<conditions>
			<player canCreateBase="true" />
		</conditions>
		<effects>
			<effect name="HeadquarterCreate" />
		</effects>
	</skill>
	<skill id="{FLAG_DISPLAY}" levels="1" name="Flag Display">
		<icon>icon.weapon_fort_flag_i00</icon>
		<operateType>CA1</operateType>
		<targetType>FLAGPOLE</targetType>
		<castRange>85</castRange>
		<effectRange>400</effectRange>
		<hitTime>{FLAG_DISPLAY_CAST_MS}</hitTime>
		<isMagic>2</isMagic>
		<magicLevel>80</magicLevel>
		<reuseDelay>5000</reuseDelay>
		<conditions>
			<player canTakeFort="true" />
		</conditions>
		<startEffects>
			<effect name="TakeFortStart" />
		</startEffects>
		<effects>
			<effect name="TakeFort" />
		</effects>
	</skill>
</list>"##
    )
}

// ---- the writer ----------------------------------------------------------------------------------

/// Adds the M7 data below `root` (the folder that holds `data/`). Call after `write_social_data`.
pub fn write_residence_data(root: &Path) {
    write(root, "data/zones/m7.xml", &zones());
    write(root, "data/stats/skills/m7_skills.xml", &skills());

    // The gates: doors with an HP bar (`hp_showable` defaults to true), so they are enemies while a
    // siege runs.
    let door = |id: i32, name: &str, (gx, gy, gz): (i32, i32, i32)| {
        format!(
            "\t<door id=\"{id}\" name=\"{name}\" height=\"150\" baseHpMax=\"{GATE_HP}\" basePDef=\"{GATE_PDEF}\" baseMDef=\"0\"\n\
             \t\tpos=\"{gx},{gy},{gz}\" node1=\"{},{}\" node2=\"{},{}\" node3=\"{},{}\" node4=\"{},{}\" nodeZ=\"{}\" />\n",
            gx - 10,
            gy - 50,
            gx + 10,
            gy - 50,
            gx + 10,
            gy + 50,
            gx - 10,
            gy + 50,
            gz - 10
        )
    };
    write(
        root,
        "data/Doors.xml",
        &format!(
            "<list>\n{}{}</list>",
            door(GATE_ID, "gludio_castle_gate", GATE_POS),
            door(FORT_GATE_ID, "shanty_fort_gate", FORT_GATE_POS)
        ),
    );

    // The flag pole of the fortress: a static object of type 3 whose name starts with the name of
    // the fortress (`Fort.loadFlagPoles`).
    write(
        root,
        "data/StaticObjects.xml",
        &format!(
            "<list>\n\t<object id=\"{FLAG_POLE_STATIC_ID}\" name=\"Shanty_flagpole\" type=\"3\" x=\"{}\" y=\"{}\" z=\"{}\" />\n</list>",
            FLAG_POLE_POS.0, FLAG_POLE_POS.1, FLAG_POLE_POS.2
        ),
    );

    // The combat flag: worn at once, and it carries the flag skill (`isForceEquip`).
    write(
        root,
        "data/stats/items/m7_items.xml",
        &format!(
            r#"<list>
	<item id="{COMBAT_FLAG_ITEM}" type="Weapon" name="Combat flag">
		<set name="weapon_type" val="FLAG" />
		<set name="bodypart" val="lrhand" />
		<set name="immediate_effect" val="true" />
		<set name="weight" val="500" />
		<set name="is_tradable" val="false" />
		<set name="is_dropable" val="false" />
		<set name="is_destroyable" val="false" />
		<set name="is_sellable" val="false" />
		<set name="is_depositable" val="false" />
		<set name="isForceEquip" val="true" />
		<set name="isAttackWeapon" val="false" />
		<set name="useWeaponSkillsOnly" val="true" />
		<skills>
			<skill id="{FLAG_DISPLAY}" level="1" />
		</skills>
	</item>
	<item id="{MERC_TICKET}" type="EtcItem" name="Mercenary Posting Ticket (Sword/Stationary)">
		<set name="etcitem_type" val="CASTLE_GUARD" />
		<set name="immediate_effect" val="true" />
		<set name="is_stackable" val="true" />
		<set name="handler" val="MercTicket" />
	</item>
</list>"#
        ),
    );

    // The NPCs of the castle: the artefact stands there from the start (a spawn of the datapack),
    // the headquarters and the guards come with the siege.
    write(
        root,
        "data/stats/npcs/35000-35099.xml",
        &format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list>\n{}{}{}{}{}{}{}</list>",
            npc(ARTEFACT_ID, "Artefact", "Holy Artifact", false),
            npc(HQ_ID, "Folk", "Headquarters", true),
            npc(MERC_ID, "Defender", "Mercenary", true),
            npc(FORT_GUARD_ID, "Defender", "Shanty Guard", true),
            npc(FORT_COMMANDER_1, "FortCommander", "Archer Commander", true),
            npc(FORT_COMMANDER_2, "FortCommander", "Guard Commander", true),
            npc(HALL_MANAGER_ID, "ClanHallManager", "Hall Manager", false),
        ),
    );
    write(
        root,
        "data/stats/npcs/35600-35699.xml",
        &format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list>\n{}{}</list>",
            npc(FORT_FOREMAN_ID, "FortManager", "Shanty Foreman", false),
            npc(FORT_MERCHANT_ID, "Folk", "Suspicious Merchant", false),
        ),
    );
    write(
        root,
        "data/stats/npcs/30760-30769.xml",
        &format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list>\n{}</list>",
            npc(AUCTIONEER_ID, "Auctioneer", "Auctioneer", false)
        ),
    );
    write(
        root,
        "data/spawns/m7_castle.xml",
        &format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n{}{}{}</list>",
            spawn(ARTEFACT_ID, ARTEFACT_POS),
            spawn(HALL_MANAGER_ID, HALL_MANAGER_POS),
            spawn(AUCTIONEER_ID, AUCTIONEER_POS),
        ),
    );
    write_pages(root);

    // The siege schedule (`config/SiegeSchedule.xml` of the datapack root): Gludio sieges on
    // Sunday at 16:00.
    write(
        root,
        "config/SiegeSchedule.xml",
        r#"<list>
	<schedule castleId="1" castleName="Gludio" siegeEnabled="true" day="SUNDAY" hour="16" maxConcurrent="5" />
</list>"#,
    );

    // The price config that ties the town of the start (and the smith of `m6_data.rs` who stands
    // in it) to Gludio: a purchase costs the base price times (1 + baseTax/100 + castleTax/100)
    // and the castle gets its share.
    write(
        root,
        "data/MerchantPriceConfig.xml",
        &format!(
            r#"<merchantPriceConfig defaultPriceConfig="1">
	<priceConfig id="0" name="Talking Island" baseTax="{BASE_TAX}" castleId="{GLUDIO}" zoneId="{TOWN_ZONE_ID}" />
	<priceConfig id="1" name="Neutral Territory" baseTax="50" />
</merchantPriceConfig>"#
        ),
    );
}

// ---- the pages -----------------------------------------------------------------------------------

/// The pages of the auctioneer, the hall manager and the fortress's suspicious merchant. The
/// placeholders are the ones the code replaces (the Java files have the same words); the markup is
/// cut down to what the client sends back.
fn write_pages(root: &Path) {
    let pages: &[(&str, &str)] = &[
        (
            "data/html/auction/auction.htm",
            "<html><body>Auctioneer %npcname%:<br>\n\
             <a action=\"bypass -h npc_%objectId%_list\">Auctions</a><br>\n</body></html>",
        ),
        (
            "data/html/auction/auction-busy.htm",
            "<html><body>The auction is closed during a siege.</body></html>",
        ),
        (
            "data/html/auction/AgitAuctionList.htm",
            "<html><body>%itemsField%<br><a action=\"%AGIT_LINK_BACK%\">Back</a></body></html>",
        ),
        (
            "data/html/auction/AgitAuctionInfo.htm",
            "<html><body>%AGIT_NAME% of %OWNER_PLEDGE_NAME% (%OWNER_PLEDGE_MASTER%), %AGIT_LOCATION%, \
             lease %AGIT_LEASE%, minimum bid %AGIT_AUCTION_MINBID%, %AGIT_AUCTION_COUNT% bids, ends \
             %AGIT_AUCTION_END% (%AGIT_AUCTION_REMAIN%)<br>\n\
             <a action=\"%AGIT_LINK_RE%\">Bid</a><br><a action=\"%AGIT_LINK_BIDLIST%\">Bidders</a><br>\n\
             <a action=\"%AGIT_LINK_BACK%\">Back</a></body></html>",
        ),
        (
            "data/html/auction/AgitBid1.htm",
            "<html><body>Clan adena %PLEDGE_ADENA%, minimum bid %AGIT_AUCTION_MINBID%<br>\n\
             <edit var=\"bid\" width=120 height=12>\n\
             <button value=\"Bid\" action=\"bypass -h npc_%objectId%_bid $bid\" width=70 height=21>\n\
             <a action=\"%AGIT_LINK_BACK%\">Back</a></body></html>",
        ),
        (
            "data/html/auction/not_authorized.htm",
            "<html><body>You are not authorized.</body></html>",
        ),
        (
            "data/html/clanHallManager/chamberlain.htm",
            "<html><body>Hall %npcname%:<br>\n\
             <a action=\"bypass -h npc_%objectId%_manage recovery\">Recovery</a><br>\n</body></html>",
        ),
        (
            "data/html/clanHallManager/chamberlain-no.htm",
            "<html><body>I serve no one here.</body></html>",
        ),
        (
            "data/html/clanHallManager/chamberlain-of.htm",
            "<html><body>I serve another clan.</body></html>",
        ),
        (
            "data/html/clanHallManager/not_authorized.htm",
            "<html><body>You are not authorized.</body></html>",
        ),
        (
            "data/html/clanHallManager/manage.htm",
            "<html><body><a action=\"bypass -h npc_%objectId%_manage recovery\">Recovery</a></body></html>",
        ),
        (
            "data/html/clanHallManager/edit_recovery.htm",
            "<html><body>HP: %hp_recovery% %hp_period% %change_hp%<br>MP: %mp_recovery% %mp_period% %change_mp%<br>\n\
             EXP: %exp_recovery% %exp_period% %change_exp%</body></html>",
        ),
        (
            "data/html/clanHallManager/functions-apply.htm",
            "<html><body>%name%: %use% (%cost%<br>\n\
             <a action=\"bypass -h npc_%objectId%_manage %apply%\">Apply</a></body></html>",
        ),
        (
            "data/html/clanHallManager/functions-apply_confirmed.htm",
            "<html><body>The function is done.</body></html>",
        ),
        (
            "data/html/clanHallManager/functions-cancel.htm",
            "<html><body><a action=\"bypass -h npc_%objectId%_manage %apply%\">Stop</a></body></html>",
        ),
        (
            "data/html/clanHallManager/functions-cancel_confirmed.htm",
            "<html><body>The function is stopped.</body></html>",
        ),
        (
            "data/html/clanHallManager/functions-used.htm",
            "<html><body>The function is in use at %val%.</body></html>",
        ),
        (
            "data/html/clanHallManager/low_adena.htm",
            "<html><body>You do not have enough adena.</body></html>",
        ),
        (
            "data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager.html",
            "<html><body>Merchant: want to take the fortress?<br>\n\
             <a action=\"bypass Script FortressSiegeManager register\">Register</a><br>\n\
             <a action=\"bypass Script FortressSiegeManager cancel\">Cancel</a></body></html>",
        ),
        (
            "data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager-01.html",
            "<html><body>The fortress belongs to %clanName%.</body></html>",
        ),
        (
            "data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager-02.html",
            "<html><body>You have no clan.</body></html>",
        ),
        (
            "data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager-03.html",
            "<html><body>You cannot pay 250000 adena.</body></html>",
        ),
        (
            "data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager-04.html",
            "<html><body>Your clan is too small.</body></html>",
        ),
        (
            "data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager-05.html",
            "<html><body>Your clan is registered.</body></html>",
        ),
        (
            "data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager-06.html",
            "<html><body>Your clan is registered already.</body></html>",
        ),
        (
            "data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager-08.html",
            "<html><body>Your registration is cancelled.</body></html>",
        ),
        (
            "data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager-09.html",
            "<html><body>You were not registered.</body></html>",
        ),
        (
            "data/scripts/ai/others/FortressSiegeManager/FortressSiegeManager-10.html",
            "<html><body>You may not register.</body></html>",
        ),
    ];
    for (path, html) in pages {
        write(root, path, html);
    }
}
