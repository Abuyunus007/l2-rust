//! What the M3 end-to-end test adds to the small datapack of `datapack.rs`: a gatekeeper
//! (a `Teleporter` NPC with its dialog pages and destination list) standing next to the place
//! where new characters appear, a peace zone and a town zone around that place, the map region
//! that goes with it, and a tiny synthetic geodata region with one wall.
//!
//! CI has neither the Java datapack nor the geodata, so everything is written into the
//! temporary folder of the test. The numbers are made up; the shapes are those of the real
//! files (`data/spawns`, `data/teleporters`, `data/zones`, `data/mapregion`, `data/html`).

use std::path::Path;

use l2_geo::coords::{geo_x, geo_y, world_x, world_y};

/// Where both fighters appear (the creation point of the test datapack's human fighter).
pub const START: (i32, i32, i32) = (-71338, 258271, -3104);
/// Height of the whole synthetic geodata region (and so of the ground everywhere in it).
pub const GROUND_Z: i32 = -3104;

/// Template id of the gatekeeper (the real Roxxy).
pub const GATEKEEPER_ID: i32 = 30006;
/// Where the gatekeeper stands: 100 units from the start, well inside talking range (250).
pub const GATEKEEPER_POS: (i32, i32, i32) = (START.0, START.1 + 100, GROUND_Z);
/// The name of the gatekeeper, as the dialog shows it.
pub const GATEKEEPER_NAME: &str = "Roxxy";

/// What the first destination of the gatekeeper costs. Characters pay it when
/// `MaxFreeTeleportLevel` is below their level (the test sets it to 0).
pub const TELEPORT_FEE: i64 = 150;
/// The first destination: far from the start (more than a world region away), but inside the
/// geodata region, so the ground height is known there.
pub const DESTINATION: (i32, i32, i32) = (START.0 - 10_000, START.1, GROUND_Z);
/// The second destination of the list.
pub const DESTINATION_2: (i32, i32, i32) = (START.0 - 12_000, START.1 + 600, GROUND_Z);

/// The `X_Y` of the geodata tile that holds [`START`] (Talking Island).
pub const GEO_TILE: (i32, i32) = (17, 25);
/// How many geo cells east of the start cell the wall is (one cell thick, completely closed).
pub const WALL_CELLS_EAST: i32 = 5;

fn write(root: &Path, relative: &str, text: &str) {
    let path = root.join(relative);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

/// `data/stats/statBonus.xml` with the DEX, WIT and INT columns too. The base datapack has only
/// the columns its experience and HP tests need, but the DEX bonus is the movement multiplier:
/// without it every speed is 0 and nobody can walk. The values are the real ones.
const STAT_BONUS: &str = r#"<list>
	<STR><stat value="40" bonus="1.2" /></STR>
	<INT><stat value="21" bonus="0.81" /><stat value="44" bonus="1.28" /></INT>
	<DEX><stat value="23" bonus="1.03" /><stat value="30" bonus="1.1" /></DEX>
	<WIT><stat value="11" bonus="0.64" /><stat value="19" bonus="0.95" /></WIT>
	<CON><stat value="24" bonus="0.9" /><stat value="32" bonus="1.1" /><stat value="43" bonus="1.58" /></CON>
	<MEN><stat value="25" bonus="1.28" /><stat value="26" bonus="1.3" /><stat value="37" bonus="1.45" /></MEN>
</list>"#;

/// Adds the world data below `root` (the folder that holds `data/`; the base datapack of
/// `datapack.rs` is written there too). Also writes the geodata to `root/geodata`.
pub fn write_world_data(root: &Path) {
    write(root, "data/stats/statBonus.xml", STAT_BONUS);

    // The gatekeeper: a Teleporter that is not attackable.
    write(
        root,
        "data/stats/npcs/30000-30099.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<npc id="{GATEKEEPER_ID}" level="70" type="Teleporter" name="{GATEKEEPER_NAME}" title="Gatekeeper">
		<race>HUMAN</race>
		<sex>FEMALE</sex>
		<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
			<vitals hp="2444.46819" hpRegen="7.5" mp="1345.8" mpRegen="2.7" />
			<attack physical="688.86373" magical="470.40463" random="30" critical="4" accuracy="5" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
			<defence physical="295.91597" magical="216.53847" />
			<speed>
				<walk ground="50" />
				<run ground="120" />
			</speed>
		</stats>
		<status attackable="false" />
	</npc>
</list>"#
        ),
    );
    write(
        root,
        "data/spawns/17_25.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<spawn name="17_25">
		<npc id="{GATEKEEPER_ID}" x="{}" y="{}" z="{}" heading="40960" respawnDelay="60" />
	</spawn>
</list>"#,
            GATEKEEPER_POS.0, GATEKEEPER_POS.1, GATEKEEPER_POS.2
        ),
    );

    // The destinations. The names and numbers are made up; the layout is the real one.
    write(
        root,
        "data/teleporters/e2e.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<npc id="{GATEKEEPER_ID}">
		<teleport type="NORMAL">
			<location name="Test Village" x="{}" y="{}" z="{}" feeCount="{TELEPORT_FEE}" />
			<location name="Test Ruins" x="{}" y="{}" z="{}" feeCount="300" />
		</teleport>
	</npc>
</list>"#,
            DESTINATION.0, DESTINATION.1, DESTINATION.2, DESTINATION_2.0, DESTINATION_2.1, DESTINATION_2.2
        ),
    );

    // The pages: what a click shows, and the list page the `showTeleports` link opens.
    write(
        root,
        &format!("data/html/teleporter/{GATEKEEPER_ID}.htm"),
        "<html><body>Gatekeeper %npcname%:<br>\n\
         Where do you want to go?<br>\n\
         <a action=\"bypass npc_%objectId%_showTeleports\">Teleport</a><br>\n\
         </body></html>",
    );
    write(
        root,
        "data/html/teleporter/teleports.htm",
        "<html><body>Region where teleporting is possible<br>\n<br>\n%locations%\n<br>\n</body></html>",
    );

    // Peace and town zones around the start, a box of 2400 x 2000 units.
    let (x, y) = (START.0, START.1);
    let corners = format!(
        "<node X=\"{}\" Y=\"{}\" /><node X=\"{}\" Y=\"{}\" />",
        x - 1200,
        y - 1000,
        x + 1200,
        y + 1000
    );
    write(
        root,
        "data/zones/e2e.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<zone name="E2E Town" id="11022" type="TownZone" shape="Cuboid" minZ="-3500" maxZ="-2700">
		<stat name="townId" val="2" />
		{corners}
	</zone>
	<zone name="E2E Peace" id="11023" type="PeaceZone" shape="Cuboid" minZ="-3500" maxZ="-2700">
		{corners}
	</zone>
</list>"#
        ),
    );

    // The map region of the start tile (restart points for "nearest town").
    write(
        root,
        "data/mapregion/e2e.xml",
        &format!(
            r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<region name="talking_island_town" town="Talking Island Town" castle="1" locId="910" bbs="1">
		<respawnPoint X="{}" Y="{}" Z="{}" />
		<map X="{}" Y="{}" />
	</region>
</list>"#,
            START.0,
            START.1 - 300,
            GROUND_Z,
            GEO_TILE.0,
            GEO_TILE.1
        ),
    );

    write_geodata(&root.join("geodata"));
}

/// The cell word of a flat-ground cell: the height in bits 4..15 (doubled), the four NSWE bits
/// open (`15`) or closed (`0`).
fn cell_word(open: bool) -> u16 {
    let height = (GROUND_Z * 2) as i16 as u16 & 0xFFF0;
    height | if open { 0x0F } else { 0 }
}

/// Where the wall is, in geo cells: the column east of the start cell, on the row of the start.
pub fn wall_cell() -> (i32, i32) {
    (geo_x(START.0) + WALL_CELLS_EAST, geo_y(START.1))
}

/// The world position of the middle of the last open cell before the wall, on the row of the
/// start: where a walk east from the start stops, because the next cell is closed.
pub fn last_free_spot() -> (i32, i32, i32) {
    let (wall_x, row) = wall_cell();
    (world_x(wall_x - 1), world_y(row), GROUND_Z)
}

/// A world position in the cell right behind the wall (east of it), on the row of the start. A
/// walk to it is cut short at the wall; it is close enough behind it that Java does not start a
/// path search (`original distance - corrected distance <= 30`).
pub fn behind_the_wall() -> (i32, i32, i32) {
    let (wall_x, _) = wall_cell();
    (world_x(wall_x + 1) - 2, START.1, GROUND_Z)
}

/// A world position in the wall cell itself: clicking it is refused (`isCompletelyBlocked`).
pub fn inside_the_wall() -> (i32, i32, i32) {
    let (wall_x, _) = wall_cell();
    (world_x(wall_x), START.1, GROUND_Z)
}

/// The bytes of the `.l2j` tile: every block flat at [`GROUND_Z`], except the one block with
/// the wall, which is a complex block (8 x 8 cells) whose column of the wall is closed in all
/// four directions.
pub fn geodata_tile() -> Vec<u8> {
    const REGION_BLOCKS_SIDE: i32 = 256;
    const REGION_CELLS_SIDE: i32 = 2048;
    let (wall_x, row) = wall_cell();
    // Cells inside the tile, then the block and the cell inside the block (cells of the file
    // are `cx * 8 + cy`, blocks `bx * 256 + by`).
    let local_x = wall_x - GEO_TILE.0 * REGION_CELLS_SIDE;
    let local_y = row - GEO_TILE.1 * REGION_CELLS_SIDE;
    let (block_x, wall_cx) = (local_x >> 3, local_x & 7);
    let block_y = local_y >> 3;
    let wall_block = (block_x * REGION_BLOCKS_SIDE + block_y) as usize;

    let mut out = Vec::with_capacity(65_536 * 3);
    for block in 0..65_536usize {
        if block == wall_block {
            out.push(1); // complex
            for cx in 0..8 {
                for _cy in 0..8 {
                    out.extend_from_slice(&cell_word(cx != wall_cx).to_le_bytes());
                }
            }
        } else {
            out.push(0); // flat
            out.extend_from_slice(&(GROUND_Z as i16).to_le_bytes());
        }
    }
    out
}

/// Writes the tile as `<dir>/17_25.l2j`.
pub fn write_geodata(dir: &Path) {
    std::fs::create_dir_all(dir).unwrap();
    std::fs::write(
        dir.join(format!("{}_{}.l2j", GEO_TILE.0, GEO_TILE.1)),
        geodata_tile(),
    )
    .unwrap();
}
