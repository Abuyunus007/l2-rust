//! The spawn file the admin spawn commands write: one xml per map tile in `data/spawns/Others/`,
//! with one `<npc>` line per spawn, in the format Java writes it.
// Java: gameserver/data/xml/SpawnData.java (addNewSpawn)
//
// Lines end with `\n` (Java uses the platform's separator; the repo keeps LF). The tile numbers
// come from `World.WORLD_X_MIN`/`WORLD_Y_MIN` and `TILE_X_MIN`/`TILE_Y_MIN`.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

/// `World.TILE_X_MIN`, `TILE_Y_MIN`, `TILE_ZERO_COORD_X/Y` and `TILE_SIZE` (32768, a shift of 15).
const TILE_X_MIN: i32 = 11;
const TILE_Y_MIN: i32 = 10;
const WORLD_X_MIN: i32 = (TILE_X_MIN - 20) * 32768;
const WORLD_Y_MIN: i32 = (TILE_Y_MIN - 18) * 32768;

/// The spawn `//spawn` and its relatives made, as `SpawnData.addNewSpawn` reads it from the
/// `Spawn`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewSpawn {
    pub npc_id: i32,
    /// `getAmount()`; written as `count` only above 1.
    pub amount: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `getHeading()`; written only above 0.
    pub heading: i32,
    /// `getRespawnDelay() / 1000`: seconds (Java's integer division of milliseconds).
    pub respawn_delay_secs: i32,
    /// The comment after the line: the NPC name, or `name - title`.
    pub label: String,
}

/// The tile of a position, the `x_y` in the file name.
pub fn spawn_tile(x: i32, y: i32) -> (i32, i32) {
    (
        ((x.wrapping_sub(WORLD_X_MIN)) >> 15) + TILE_X_MIN,
        ((y.wrapping_sub(WORLD_Y_MIN)) >> 15) + TILE_Y_MIN,
    )
}

/// The `<npc .../>` line with its comment (no indentation, no line end).
fn npc_line(spawn: &NewSpawn) -> String {
    let mut line = format!("<npc id=\"{}", spawn.npc_id);
    if spawn.amount > 1 {
        line.push_str(&format!("\" count=\"{}", spawn.amount));
    }
    line.push_str(&format!("\" x=\"{}\" y=\"{}\" z=\"{}", spawn.x, spawn.y, spawn.z));
    if spawn.heading > 0 {
        line.push_str(&format!("\" heading=\"{}", spawn.heading));
    }
    line.push_str(&format!(
        "\" respawnDelay=\"{}\" /> <!-- {} -->",
        spawn.respawn_delay_secs, spawn.label
    ));
    line
}

/// Java `addNewSpawn`: adds the spawn to `<dir>/<tileX>_<tileY>.xml`. An existing file gets the
/// line before its `</spawn>`; a new file gets the whole document. Returns the file.
pub fn add_new_spawn(dir: &Path, spawn: &NewSpawn) -> io::Result<PathBuf> {
    fs::create_dir_all(dir)?;
    let (tile_x, tile_y) = spawn_tile(spawn.x, spawn.y);
    let file = dir.join(format!("{tile_x}_{tile_y}.xml"));
    let line = npc_line(spawn);
    if file.exists() {
        let text = fs::read_to_string(&file)?;
        let mut out = String::with_capacity(text.len() + line.len() + 4);
        for current in text.lines() {
            if current.contains("</spawn>") {
                out.push_str("\t\t");
                out.push_str(&line);
                out.push('\n');
            }
            out.push_str(current);
            out.push('\n');
        }
        // Java writes to `<file>.tmp`, deletes the file and renames.
        let temp = dir.join(format!("{tile_x}_{tile_y}.xml.tmp"));
        fs::write(&temp, out)?;
        fs::remove_file(&file)?;
        fs::rename(&temp, &file)?;
    } else {
        let text = format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"../../xsd/spawns.xsd\">\n\t<spawn name=\"{tile_x}_{tile_y}\">\n\t\t{line}\n\t</spawn>\n</list>\n"
        );
        fs::write(&file, text)?;
    }
    Ok(file)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spawn(npc_id: i32, amount: i32, heading: i32) -> NewSpawn {
        NewSpawn {
            npc_id,
            amount,
            x: -71301,
            y: 258259,
            z: -3123,
            heading,
            respawn_delay_secs: 60,
            label: "Gatekeeper - Town".to_owned(),
        }
    }

    #[test]
    fn the_tile_of_a_position() {
        // Talking Island is tile 17_25 in Java's numbering.
        assert_eq!(spawn_tile(-84_000, 244_000), (17, 25));
        assert_eq!(spawn_tile(-71_301, 258_259), (17, 25));
        // The arithmetic shift rounds toward minus infinity, west of the map's first tile too.
        assert_eq!(spawn_tile(WORLD_X_MIN - 1, WORLD_Y_MIN - 1), (10, 9));
    }

    #[test]
    fn a_new_file_gets_the_whole_document() {
        let dir = tempfile::tempdir().unwrap();
        let others = dir.path().join("data/spawns/Others");
        let file = add_new_spawn(&others, &spawn(30006, 1, 0)).unwrap();
        assert_eq!(file.file_name().unwrap(), "17_25.xml");
        assert_eq!(
            fs::read_to_string(&file).unwrap(),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"../../xsd/spawns.xsd\">\n\t<spawn name=\"17_25\">\n\t\t<npc id=\"30006\" x=\"-71301\" y=\"258259\" z=\"-3123\" respawnDelay=\"60\" /> <!-- Gatekeeper - Town -->\n\t</spawn>\n</list>\n"
        );
    }

    #[test]
    fn an_existing_file_gets_the_line_before_the_spawn_end_and_count_and_heading_show() {
        let dir = tempfile::tempdir().unwrap();
        add_new_spawn(dir.path(), &spawn(30006, 1, 0)).unwrap();
        let file = add_new_spawn(dir.path(), &spawn(20101, 3, 16384)).unwrap();
        let text = fs::read_to_string(file).unwrap();
        let lines: Vec<&str> = text.lines().collect();
        assert_eq!(
            lines[4],
            "\t\t<npc id=\"20101\" count=\"3\" x=\"-71301\" y=\"258259\" z=\"-3123\" heading=\"16384\" respawnDelay=\"60\" /> <!-- Gatekeeper - Town -->"
        );
        assert_eq!(lines[5], "\t</spawn>");
        assert_eq!(lines.len(), 7);
        assert!(!dir.path().join("17_25.xml.tmp").exists());
    }
}
