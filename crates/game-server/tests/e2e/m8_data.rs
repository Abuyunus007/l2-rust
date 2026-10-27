//! What the M8 end-to-end test adds to the datapack of `datapack.rs` and `m3_data.rs` to
//! `m6_data.rs`: an instance template, a raid boss, a priest of the Seven Signs with its pages and
//! the seal stones, an item auction, the arenas of the Olympiad and the admin rights.
//!
//! CI has no datapack, so everything is written into the temporary folder of the test. The shapes
//! are those of the shipped files; the numbers are made up and small.
//!
//! Everything stands in the geodata tile of the start (17_25) next to the town of `m3_data.rs`.

use std::path::Path;

use super::m3_data::{GROUND_Z, START};
use super::m6_data;

pub const Z: i32 = GROUND_Z;

/// The GM's access level.
pub const GM_LEVEL: i32 = 100;

/// A class that went through the first class transfer (Human Warrior): the priests of the Seven
/// Signs refuse a join from a class of level 0.
pub const CLASS_WARRIOR: i32 = 1;

// ---- the instance ----------------------------------------------------------------------------

/// The template of the instance (`InstanceNames.xml`, `data/instances`).
pub const INSTANCE_TEMPLATE: i32 = 77;
pub const INSTANCE_NAME: &str = "E2E Zone";
/// The id the GM gives the instance made from the template (`//createinstance`).
pub const INSTANCE_ID: i32 = 500;
/// Where the instance sends a player who leaves (`<exitPoint>`).
pub const INSTANCE_EXIT: (i32, i32, i32) = (START.0 - 400, START.1 + 200, Z);

// ---- the raid boss ---------------------------------------------------------------------------

pub const RAID_ID: i32 = 29_901;
pub const RAID_NAME: &str = "Raid One";
pub const RAID_LEVEL: i32 = 70;
pub const RAID_POS: (i32, i32, i32) = (START.0 + 250, START.1, Z);
/// `respawn_delay` and `respawn_random` of the row, in seconds (a day, plus or minus half of it).
pub const RAID_RESPAWN: (i32, i32) = (86_400, 43_200);

// ---- the Seven Signs -------------------------------------------------------------------------

pub const PRIEST_ID: i32 = 31_078;
pub const PRIEST_POS: (i32, i32, i32) = (START.0 + 60, START.1 - 40, Z);
pub const BLUE_STONE: i32 = 6_360;
pub const GREEN_STONE: i32 = 6_361;
pub const RED_STONE: i32 = 6_362;
pub const ANCIENT_ADENA: i32 = 5_575;
pub const CERTIFICATE: i32 = 6_388;
/// The stones the GM contributes: ten red (10 points each) and two blue (3 points each).
pub const RED_COUNT: i64 = 10;
pub const BLUE_COUNT: i64 = 2;
pub const CONTRIBUTION: i64 = 106;

// ---- the item auction ------------------------------------------------------------------------

/// The auction instance (the id of the auctioneer NPC in the shipped file).
pub const AUCTION_INSTANCE: i32 = 31_900;
/// The item of the auction: the Test Blade of `m6_data.rs`.
pub const AUCTION_ITEM: i32 = m6_data::TEST_BLADE;
pub const AUCTION_ITEM_ID: i32 = 1;
pub const AUCTION_INIT_BID: i64 = 100;
/// The `auctionId` of the row the test puts in the database.
pub const AUCTION_ID: i32 = 1;

// ---- the Olympiad ----------------------------------------------------------------------------

/// Where the arena "Grassy Arena" (template 147) is.
pub const ARENA: (i32, i32, i32) = (START.0 - 9_000, START.1 + 6_000, Z);
/// The base class of the classed matches (Duelist).
pub const DUELIST: i32 = 88;

// ---- the boat --------------------------------------------------------------------------------

/// The ticket of the Talking Island to Gludin ferry.
pub const BOAT_TICKET: i32 = 1_074;
/// `BoatTalkingGludin`'s dock at Talking Island, the boat's spawn.
pub const TALKING_DOCK: (i32, i32, i32) = (-96_622, 261_660, -3_610);

/// Every admin word of the scenarios.
const ADMIN_COMMANDS: &[&str] = &[
    "admin_createinstance",
    "admin_setinstance",
    "admin_destroyinstance",
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
    write(
        config_dir,
        "AdminCommands.xml",
        &format!("<list>\n{commands}</list>"),
    );
}

/// One NPC template in the shape of the shipped files.
fn npc(id: i32, kind: &str, name: &str, level: i32) -> String {
    format!(
        r#"	<npc id="{id}" level="{level}" type="{kind}" name="{name}" title="">
		<race>HUMAN</race>
		<sex>MALE</sex>
		<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
			<vitals hp="3000" hpRegen="2" mp="400" mpRegen="0.9" />
			<attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
			<defence physical="44.44444" magical="29.5" />
			<speed><walk ground="40" /><run ground="100" /></speed>
		</stats>
		<ai aggroRange="0" isAggressive="false" />
		<collision><radius normal="10" /><height normal="20" /></collision>
	</npc>
"#
    )
}

/// The pages the priest of the Dawn shows (the names Java's `SignsPriest` and `DawnPriest` use),
/// each with the links the next steps come through: a bypass is only taken when the page the
/// player saw offers it.
const PRIEST_PAGES: &[&str] = &[
    "dawn_priest_1a",
    "dawn_priest_1b",
    "dawn_priest_2a",
    "dawn_priest_2c",
    "dawn_priest_2d",
    "dawn_priest_3a",
    "dawn_priest_4",
    "signs_4_dawn",
    "signs_6_dawn",
    "signs_6_dawn_no_stones",
    "signs_9_dawn_a",
    "signs_9_dawn_b",
    "signs_33_dawn_fee",
    "signs_33_dawn_member",
    "dawn_firstclass",
    "signs_33_dawn_firstclass",
];

const PRIEST_LINKS: &str = r#"<a action="bypass -h npc_%objectId%_SevenSigns 4 2 1">join</a><a action="bypass -h npc_%objectId%_SevenSigns 6 4">all stones</a><a action="bypass -h npc_%objectId%_SevenSigns 9">reward</a>"#;

fn items() -> String {
    let etc = |id: i32, name: &str, stackable: bool| {
        format!(
            "<item id=\"{id}\" type=\"EtcItem\" name=\"{name}\"><set name=\"is_stackable\" val=\"{stackable}\" /></item>\n"
        )
    };
    let mut xml = String::from("<list>\n");
    xml.push_str(&etc(BLUE_STONE, "Blue Seal Stone", true));
    xml.push_str(&etc(GREEN_STONE, "Green Seal Stone", true));
    xml.push_str(&etc(RED_STONE, "Red Seal Stone", true));
    xml.push_str(&etc(ANCIENT_ADENA, "Ancient Adena", true));
    xml.push_str(&etc(CERTIFICATE, "Lord's Certificate", true));
    xml.push_str(&etc(BOAT_TICKET, "Gludio Ticket", false));
    xml.push_str("</list>");
    xml
}

/// The arena zone and the four arena templates (147 to 150) the Olympiad stadiums copy.
fn olympiad_zone() -> String {
    let (x, y, z) = ARENA;
    let spawns: String = [-1_400, -1_415, -1_430, 1_400, 1_415, 1_430]
        .iter()
        .map(|dx| format!(r#"<spawn X="{}" Y="{y}" Z="{z}" />"#, x + dx))
        .collect();
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<zone name="Grassy Arena" type="OlympiadStadiumZone" shape="Cylinder" minZ="{}" maxZ="{}" rad="2000">
		<node X="{x}" Y="{y}" />{spawns}<spawn X="{x}" Y="{y}" Z="{z}" type="spectatorSpawn" />
	</zone>
</list>"#,
        z - 300,
        z + 4_000
    )
}

/// Adds the M8 data below `root` (the folder that holds `data/`). Call after the writers of M3
/// to M6: it reads the fighter's template they wrote.
pub fn write_m8_data(root: &Path) {
    // The instance.
    write(
        root,
        "data/instances/E2E/zone.xml",
        &format!(
            r#"<instance id="{INSTANCE_TEMPLATE}">
	<exitPoint x="{}" y="{}" z="{}" />
</instance>"#,
            INSTANCE_EXIT.0, INSTANCE_EXIT.1, INSTANCE_EXIT.2
        ),
    );
    for id in 147..=150 {
        write(
            root,
            &format!("data/instances/Olympiad/arena{id}.xml"),
            &format!(r#"<instance id="{id}"><PvPInstance val="true" /></instance>"#),
        );
    }
    write(
        root,
        "data/InstanceNames.xml",
        &format!(
            r#"<list><instance id="{INSTANCE_TEMPLATE}" name="{INSTANCE_NAME}" />{}</list>"#,
            (147..=150)
                .map(|id| format!(r#"<instance id="{id}" name="Olympiad Arena {id}" />"#))
                .collect::<String>()
        ),
    );
    write(root, "data/zones/m8_olympiad.xml", &olympiad_zone());

    // The raid boss and the priest.
    write(
        root,
        "data/stats/npcs/29900-29999.xml",
        &format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list>\n{}{}</list>",
            npc(RAID_ID, "RaidBoss", RAID_NAME, RAID_LEVEL),
            npc(PRIEST_ID, "DawnPriest", "Priest of Dawn", 70)
        ),
    );
    write(
        root,
        "data/spawns/m8.xml",
        &format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\t<spawn name=\"m8_{PRIEST_ID}\">\n\t\t<npc id=\"{PRIEST_ID}\" x=\"{}\" y=\"{}\" z=\"{}\" heading=\"0\" respawnDelay=\"60\" />\n\t</spawn>\n</list>",
            PRIEST_POS.0, PRIEST_POS.1, PRIEST_POS.2
        ),
    );
    for page in PRIEST_PAGES {
        write(
            root,
            &format!("data/html/seven_signs/{page}.htm"),
            &format!("<html><body>{page} {PRIEST_LINKS}</body></html>"),
        );
    }
    write(root, "data/stats/items/m8_items.xml", &items());

    // The first class transfer: a copy of the fighter's template for class 1.
    let fighter =
        std::fs::read_to_string(root.join("data/stats/players/templates/StartingClass/HumanFighter.xml"))
            .unwrap();
    assert!(fighter.contains("<classId>0</classId>"), "the fighter's template");
    write(
        root,
        "data/stats/players/templates/FirstClass/Warrior.xml",
        &fighter.replace(
            "<classId>0</classId>",
            &format!("<classId>{CLASS_WARRIOR}</classId>"),
        ),
    );

    // The item auction: one instance, one item, a day between two auctions.
    write(
        root,
        "data/ItemAuctions.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<instance id="{AUCTION_INSTANCE}" interval="1" hour_of_day="12" minute_of_hour="0">
		<item auctionItemId="{AUCTION_ITEM_ID}" itemId="{AUCTION_ITEM}" itemCount="1" auctionInitBid="{AUCTION_INIT_BID}" auctionLength="60" />
	</instance>
</list>"#
        ),
    );
}
