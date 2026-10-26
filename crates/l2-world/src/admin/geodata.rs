//! The geodata, world region and path admin commands: `//geo_pos`, `//geocell`, `//geoedit`,
//! `//geogrid`, `//region`, `//path_find` and friends.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminGeodata,AdminRegions,
// Java: AdminPathNode}.java
// Java: util/GeoUtils.java (debugGrid, hideDebugGrid), util/HtmlUtil.java (sendCBHtml)
//
// The geodata edits (L-07): `geoenablenorth`/`en`, `geodisablenorth`/`dn` (and south, east,
// west), `geosave`, `geosaveall`, `geomap_reload`. The world's engine is shared with the path
// workers (D-14), so it is edited copy-on-write ([`World::edit_geo`]): a search that is running
// keeps the geodata it started with, the next path request carries the edited engine.
// Saving writes on the world thread like Java writes on the command thread (`GeoEditPath`).
//
// `admin_geo_spawn_pos` is registered: Java gives it the same body as `admin_geo_pos`.

use std::sync::Arc;

use l2_geo::cell::{NSWE_EAST, NSWE_NORTH, NSWE_SOUTH, NSWE_WEST};
use l2_geo::coords::{geo_x, geo_y, world_x, world_y};
use l2_geo::{GeoEngine, Location, PathFinder, PathOptions};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExServerPrimitive, PrimitiveColor, ShowBoard, SystemMessage};
use l2_util::ObjectId;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens, parse_int};
use crate::dialog::{HtmlActionScope, build_html_action_cache};
use crate::region::{
    OFFSET_X, OFFSET_Y, SHIFT_BY, TILE_SIZE, TILE_X_MAX, TILE_X_MIN, TILE_Y_MAX, TILE_Y_MIN,
    TILE_ZERO_COORD_X, TILE_ZERO_COORD_Y, WORLD_X_MIN, WORLD_Y_MIN, WORLD_Z_MIN, Z_REGION_SIZE,
};
use crate::world::World;

type Outcome = Result<(), JavaException>;

/// Java `MasterHandler`: the commands of `AdminGeodata` (the read-only ones), `AdminRegions` and
/// `AdminPathNode`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_geo_pos", admin_geodata),
        ("admin_geo_spawn_pos", admin_geodata),
        ("admin_geo_can_move", admin_geodata),
        ("admin_geo_can_see", admin_geodata),
        ("admin_geogrid", admin_geodata),
        ("admin_geomap", admin_geodata),
        ("admin_geomap_reload", admin_geodata),
        ("admin_geocell", admin_geodata),
        ("admin_geosave", admin_geodata),
        ("admin_geosaveall", admin_geodata),
        ("admin_geoenablenorth", admin_geodata),
        ("admin_en", admin_geodata),
        ("admin_geodisablenorth", admin_geodata),
        ("admin_dn", admin_geodata),
        ("admin_geoenablesouth", admin_geodata),
        ("admin_es", admin_geodata),
        ("admin_geodisablesouth", admin_geodata),
        ("admin_ds", admin_geodata),
        ("admin_geoenableeast", admin_geodata),
        ("admin_ee", admin_geodata),
        ("admin_geodisableeast", admin_geodata),
        ("admin_de", admin_geodata),
        ("admin_geoenablewest", admin_geodata),
        ("admin_ew", admin_geodata),
        ("admin_geodisablewest", admin_geodata),
        ("admin_dw", admin_geodata),
        ("admin_geoedit", admin_geodata),
        ("admin_ge", admin_geodata),
        ("admin_region", admin_regions),
        ("admin_regions", admin_regions),
        ("admin_show_region", admin_regions),
        ("admin_show_regions", admin_regions),
        ("admin_showregion", admin_regions),
        ("admin_showregions", admin_regions),
        ("admin_region_clear", admin_regions),
        ("admin_regions_clear", admin_regions),
        ("admin_regionclear", admin_regions),
        ("admin_regionsclear", admin_regions),
        ("admin_path_find", admin_path_find),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// The path search state of `//path_find`: Java calls `PathFinding.findPath` right in the
/// command thread, so the search runs here too, once per command.
#[derive(Default)]
pub(crate) struct PathAdmin {
    options: Option<PathOptions>,
    finder: Option<PathFinder>,
}

fn npe() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

/// `NoSuchElementException` of `StringTokenizer.nextToken()`.
fn no_such_element() -> JavaException {
    JavaException("java.util.NoSuchElementException".to_owned())
}

// ------------------------------------------------------------------------------ the geoengine

/// The geoengine as the commands see it: a world without geodata behaves like Java's engine with
/// only null regions (no geodata anywhere, the ground where the caller said, every direction open).
struct Geo<'a>(Option<&'a GeoEngine>);

impl Geo<'_> {
    fn has_pos(&self, gx: i32, gy: i32) -> bool {
        self.0.is_some_and(|g| g.has_geo_pos(gx, gy))
    }

    fn nearest_z(&self, gx: i32, gy: i32, z: i32) -> i32 {
        self.0.map_or(z, |g| g.nearest_z(gx, gy, z))
    }

    fn check_nswe(&self, gx: i32, gy: i32, z: i32, nswe: u8) -> bool {
        self.0.is_none_or(|g| g.check_nearest_nswe(gx, gy, z, nswe))
    }

    fn height(&self, x: i32, y: i32, z: i32) -> i32 {
        self.0.map_or(z, |g| g.height(x, y, z))
    }
}

impl World {
    fn geo_view(&self) -> Geo<'_> {
        Geo(self.geo.as_deref())
    }

    /// Changes the world's geodata (L-07, D-14). The engine is shared with the path workers and
    /// edited copy-on-write: when a running search still holds it, it is cloned first (cheap, the
    /// regions are shared) and [`l2_geo::GeoSet::region_mut`] copies only the edited region; the
    /// search finishes on the old data and the next request carries the new engine. Without
    /// another holder the engine is edited in place. `None` when the world has no engine.
    pub(crate) fn edit_geo<R>(&mut self, edit: impl FnOnce(&mut GeoEngine) -> R) -> Option<R> {
        let mut geo = self.geo.take()?;
        // The zone context holds a second handle on the same engine (`set_geo`); let go of it so
        // that it does not force a copy, and hand it the edited engine afterwards.
        let shared_with_zoning = self.zoning.geo.as_ref().is_some_and(|z| Arc::ptr_eq(z, &geo));
        if shared_with_zoning {
            self.zoning.geo = None;
        }
        let result = edit(Arc::make_mut(&mut geo));
        if shared_with_zoning {
            self.zoning.geo = Some(Arc::clone(&geo));
        }
        self.geo = Some(geo);
        Some(result)
    }

    fn gm_position(&self, gm: ObjectId) -> Result<(i32, i32, i32), JavaException> {
        self.position_of(gm).ok_or_else(npe)
    }

    fn gm_heading(&self, gm: ObjectId) -> i32 {
        self.players.get(&gm).map_or(0, |p| p.core.heading)
    }

    /// Java `HtmlUtil.sendCBHtml(player, html)`: the page on the community board window, in up to
    /// three packets of at most 16250 characters.
    pub(crate) fn send_cb_html(&mut self, to: ObjectId, html: &str) {
        const PART: usize = 16250;
        if let Some(player) = self.players.get_mut(&to) {
            player.talk.clear(HtmlActionScope::CommBoardHtml);
            // Origin 0: the bypasses are not bound to an NPC.
            build_html_action_cache(&mut player.talk, HtmlActionScope::CommBoardHtml, 0, html);
        }
        // Java strings are UTF-16: lengths and cuts count code units.
        let units: Vec<u16> = html.encode_utf16().collect();
        let part = |from: usize, to: usize| String::from_utf16_lossy(&units[from..to.min(units.len())]);
        let len = units.len();
        let packets = if len < PART {
            [
                ShowBoard::page(Some(html), "101"),
                ShowBoard::page(None, "102"),
                ShowBoard::page(None, "103"),
            ]
        } else if len < PART * 2 {
            [
                ShowBoard::page(Some(&part(0, PART)), "101"),
                ShowBoard::page(Some(&part(PART, len)), "102"),
                ShowBoard::page(None, "103"),
            ]
        } else if len < PART * 3 {
            [
                ShowBoard::page(Some(&part(0, PART)), "101"),
                ShowBoard::page(Some(&part(PART, PART * 2)), "102"),
                ShowBoard::page(Some(&part(PART * 2, len)), "103"),
            ]
        } else {
            [
                ShowBoard::page(
                    Some("<html><body><br><center>Error: HTML was too long!</center></body></html>"),
                    "101",
                ),
                ShowBoard::page(None, "102"),
                ShowBoard::page(None, "103"),
            ]
        };
        for packet in &packets {
            self.send(to, packet);
        }
    }
}

// ------------------------------------------------------------------------------ AdminGeodata

/// Java `AdminGeodata.onCommand`, the branches that only read the geodata.
fn admin_geodata(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    let actual = st.next_token().ok_or_else(no_such_element)?;
    match actual.to_lowercase().as_str() {
        // NOTE(java-quirk): `admin_geo_spawn_pos` has the very same body as `admin_geo_pos`.
        "admin_geo_pos" | "admin_geo_spawn_pos" => {
            let (world_x, world_y, world_z) = world.gm_position(gm)?;
            let (gx, gy) = (geo_x(world_x), geo_y(world_y));
            let geo = world.geo_view();
            let text = if geo.has_pos(gx, gy) {
                format!(
                    "WorldX: {world_x}, WorldY: {world_y}, WorldZ: {world_z}, GeoX: {gx}, GeoY: {gy}, GeoZ: {}",
                    geo.height(world_x, world_y, world_z)
                )
            } else {
                "There is no geodata at this position.".to_owned()
            };
            world.send_sys_message(gm, &text);
        }
        // NOTE(java-quirk): `geo_can_move` checks the line of sight, like `geo_can_see`, but
        // answers with other texts.
        "admin_geo_can_move" => match world.target_of(gm) {
            Some(target) => {
                let text = if world.can_see_target(gm, target) {
                    "Can move beeline."
                } else {
                    "Can not move beeline!"
                };
                world.send_sys_message(gm, text);
            }
            None => {
                world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
            }
        },
        "admin_geo_can_see" => match world.target_of(gm) {
            Some(target) => {
                if world.can_see_target(gm, target) {
                    world.send_sys_message(gm, "Can see target.");
                } else {
                    world.send(gm, &SystemMessage::new(SystemMessageId::CannotSeeTarget));
                }
            }
            None => {
                world.send(gm, &SystemMessage::new(SystemMessageId::InvalidTarget));
            }
        },
        "admin_geogrid" => {
            let hide = st.next_token().is_some_and(|t| t.eq_ignore_ascii_case("off"));
            if hide {
                hide_debug_grid(world, gm)?;
            } else {
                debug_grid(world, gm)?;
            }
        }
        "admin_geomap" => {
            let (x, y, _) = world.gm_position(gm)?;
            let tile_x = ((x - WORLD_X_MIN) >> 15) + TILE_X_MIN;
            let tile_y = ((y - WORLD_Y_MIN) >> 15) + TILE_Y_MIN;
            let min_x = (tile_x - TILE_ZERO_COORD_X) * TILE_SIZE;
            let min_y = (tile_y - TILE_ZERO_COORD_Y) * TILE_SIZE;
            let text = format!(
                "GeoMap: {tile_x}_{tile_y} ({min_x},{min_y} to {},{})",
                min_x + TILE_SIZE - 1,
                min_y + TILE_SIZE - 1
            );
            world.send_sys_message(gm, &text);
        }
        "admin_geomap_reload" => {
            let (x, y) = match st.next_token() {
                Some(token) => {
                    // `token.split("_")` + `Integer.parseInt` of the first two parts; any failure
                    // (also a missing second part) is the same message.
                    let mut split = token.split('_');
                    let parsed = (
                        split.next().map(super::parse_int),
                        split.next().map(super::parse_int),
                    );
                    let (Some(Ok(x)), Some(Ok(y))) = parsed else {
                        world.send_sys_message(gm, "Could not parse geomap_reload command parameters.");
                        return Ok(());
                    };
                    (x, y)
                }
                None => {
                    let (px, py, _) = world.gm_position(gm)?;
                    (
                        ((px - WORLD_X_MIN) >> 15) + TILE_X_MIN,
                        ((py - WORLD_Y_MIN) >> 15) + TILE_Y_MIN,
                    )
                }
            };
            let reloaded = world.edit_geo(|geo| geo.reload_region(x, y)).unwrap_or(false);
            let text = if reloaded {
                format!("GeoMap: {x}_{y} reloaded successfully.")
            } else {
                format!("Failed to reload GeoMap: {x}_{y}")
            };
            world.send_sys_message(gm, &text);
        }
        "admin_geosave" => geo_save(world, gm)?,
        "admin_geosaveall" => geo_save_all(world, gm)?,
        "admin_geoenablenorth" | "admin_en" => edit_cell(world, gm, &mut st, actual, NSWE_NORTH, true)?,
        "admin_geodisablenorth" | "admin_dn" => edit_cell(world, gm, &mut st, actual, NSWE_NORTH, false)?,
        "admin_geoenablesouth" | "admin_es" => edit_cell(world, gm, &mut st, actual, NSWE_SOUTH, true)?,
        "admin_geodisablesouth" | "admin_ds" => edit_cell(world, gm, &mut st, actual, NSWE_SOUTH, false)?,
        "admin_geoenableeast" | "admin_ee" => edit_cell(world, gm, &mut st, actual, NSWE_EAST, true)?,
        "admin_geodisableeast" | "admin_de" => edit_cell(world, gm, &mut st, actual, NSWE_EAST, false)?,
        "admin_geoenablewest" | "admin_ew" => edit_cell(world, gm, &mut st, actual, NSWE_WEST, true)?,
        "admin_geodisablewest" | "admin_dw" => edit_cell(world, gm, &mut st, actual, NSWE_WEST, false)?,
        "admin_geocell" => {
            let (x, y, z) = world.gm_position(gm)?;
            let (gx, gy) = (geo_x(x), geo_y(y));
            let geo_z = world.geo_view().nearest_z(gx, gy, z);
            let text = format!(
                "GeoCell: {gx}, {gy}. XYZ ({}, {}, {geo_z})",
                world_x(gx),
                world_y(gy)
            );
            world.send_sys_message(gm, &text);
        }
        "admin_geoedit" => geo_edit(world, gm)?,
        "admin_ge" => {
            if st.has_more() {
                geo_edit_cell(world, gm, &mut st)?;
            } else {
                // `onCommand(activeChar, "admin_geoedit", false)`.
                world.admin_command(gm, "admin_geoedit", false);
            }
        }
        _ => {}
    }
    Ok(())
}

/// Java `admin_geoenable*` / `admin_geodisable*` and the short forms (`en`, `dn`, ...): opens or
/// closes one side of a cell (the GM's cell, or `<geoX> <geoY>`) at the GM's Z. The short forms
/// show the `//ge` cell page again afterwards.
fn edit_cell(
    world: &mut World,
    gm: ObjectId,
    st: &mut Tokens<'_>,
    actual: &str,
    nswe: u8,
    enable: bool,
) -> Outcome {
    let (px, py, pz) = world.gm_position(gm)?;
    let gx = match st.next_token() {
        Some(token) => parse_int(token)?,
        // NOTE(java-quirk): `admin_geodisablenorth` / `admin_dn` take the default X from
        // `GeoEngine.getGeoY(activeChar.getX())`, so without coordinates they edit another cell.
        None if nswe == NSWE_NORTH && !enable => geo_y(px),
        None => geo_x(px),
    };
    let gy = match st.next_token() {
        Some(token) => parse_int(token)?,
        None => geo_y(py),
    };
    if world.geo_view().has_pos(gx, gy) {
        world.edit_geo(|geo| {
            if enable {
                geo.set_nearest_nswe(gx, gy, pz, nswe);
            } else {
                geo.unset_nearest_nswe(gx, gy, pz, nswe);
            }
        });
        if !actual.contains("geo") {
            world.admin_command(gm, &format!("admin_ge {gx} {gy}"), false);
        }
    } else {
        world.send_sys_message(gm, "There is no geodata at this position.");
    }
    Ok(())
}

/// `Files.createDirectories(GeoEngineConfig.GEOEDIT_PATH)`: the save folder, `None` (and the
/// Java message) when it cannot be made. A world without an engine, or an engine without a
/// configured `GeoEditPath` (tests, tools), has none.
fn geo_save_dir(world: &mut World, gm: ObjectId) -> Option<std::path::PathBuf> {
    let dir = world
        .geo
        .as_ref()
        .and_then(|g| g.geoedit_path().map(std::path::Path::to_path_buf));
    match dir {
        Some(dir) if std::fs::create_dir_all(&dir).is_ok() => Some(dir),
        _ => {
            world.send_sys_message(gm, "Could not create output directory.");
            None
        }
    }
}

/// Java `admin_geosave`: the region under the GM to `GeoEditPath/X_Y.l2j`.
fn geo_save(world: &mut World, gm: ObjectId) -> Outcome {
    // Create the saves directory if it does not exist.
    let Some(dir) = geo_save_dir(world, gm) else {
        return Ok(());
    };
    let (px, py, _) = world.gm_position(gm)?;
    let x = ((px - WORLD_X_MIN) >> 15) + TILE_X_MIN;
    let y = ((py - WORLD_Y_MIN) >> 15) + TILE_Y_MIN;
    let file_name = format!("{x}_{y}.l2j");
    let text = match world.geo.as_ref().and_then(|g| g.region_at(geo_x(px), geo_y(py))) {
        None => format!("Could not find region: {x}_{y}"),
        Some(region) => save_region(region, &dir, &file_name, x, y).0,
    };
    world.send_sys_message(gm, &text);
    Ok(())
}

/// `Region.saveToFile(fileName)` and the message of the result.
fn save_region(
    region: &l2_geo::Region,
    dir: &std::path::Path,
    file_name: &str,
    x: i32,
    y: i32,
) -> (String, bool) {
    match region.save_to_file(&dir.join(file_name)) {
        Ok(()) => (format!("Saved region {x}_{y} at {file_name}"), true),
        Err(e) => {
            tracing::warn!("GeoEngine: could not save region {x}_{y}: {e}");
            (format!("Could not save region {x}_{y}"), false)
        }
    }
}

/// Java `admin_geosaveall`: every loaded region of the tiles `10..=26` x `9..=26`, one message
/// per region, then the count.
fn geo_save_all(world: &mut World, gm: ObjectId) -> Outcome {
    let Some(dir) = geo_save_dir(world, gm) else {
        return Ok(());
    };
    let mut messages = Vec::new();
    let mut count = 0;
    if let Some(geo) = world.geo.clone() {
        // Top left Gracia coordinates.
        let mut world_x_pos = -327680;
        for _region_x in TILE_X_MIN - 1..=TILE_X_MAX {
            let mut world_y_pos = -262144;
            for _region_y in TILE_Y_MIN - 1..=TILE_Y_MAX {
                if let Some(region) = geo.region_at(geo_x(world_x_pos), geo_y(world_y_pos)) {
                    let x = ((world_x_pos - WORLD_X_MIN) >> 15) + TILE_X_MIN;
                    let y = ((world_y_pos - WORLD_Y_MIN) >> 15) + TILE_Y_MIN;
                    let (text, saved) = save_region(region, &dir, &format!("{x}_{y}.l2j"), x, y);
                    if saved {
                        count += 1;
                    }
                    messages.push(text);
                }
                world_y_pos += TILE_SIZE;
            }
            world_x_pos += TILE_SIZE;
        }
    }
    for text in &messages {
        world.send_sys_message(gm, text);
    }
    world.send_sys_message(gm, &format!("Saved {count} regions."));
    Ok(())
}

/// Java `getPlayerDirection`: 0 north, 1 east, 2 south, 3 west, from the heading.
fn player_direction(heading: i32) -> i32 {
    if !(8192..=57344).contains(&heading) {
        0 // North.
    } else if heading < 24576 {
        1 // East.
    } else if heading < 40960 {
        2 // South.
    } else {
        3 // West.
    }
}

fn geo_page(world: &World, gm: ObjectId, path: &str) -> Result<String, JavaException> {
    // `HtmCache.getHtm` gives null for a missing page and the first `replace` throws.
    world.get_htm(gm, path).ok_or_else(npe)
}

/// The compass letters `N`, `E`, `S`, `W` of the page, turned so that the top is where the player
/// looks.
fn compass(direction: i32) -> [&'static str; 4] {
    match direction {
        0 => ["N", "E", "S", "W"],
        1 => ["E", "S", "W", "N"],
        2 => ["S", "W", "N", "E"],
        _ => ["W", "N", "E", "S"],
    }
}

const ARROW_ON: &str = "L2UI_CH3.minibar_food";
const ARROW_OFF: &str = "L2UI_CH3.minibar_arrow";

/// Java `admin_geoedit`: the 19 x 19 cells around the player on the community board.
fn geo_edit(world: &mut World, gm: ObjectId) -> Outcome {
    let content = geo_page(world, gm, "data/html/admin/geoedit.htm")?;
    let (x, y, z) = world.gm_position(gm)?;
    let direction = player_direction(world.gm_heading(gm));
    let page = geo_edit_page(content, direction, &world.geo_view(), (geo_x(x), geo_y(y)), z);
    world.send_cb_html(gm, &page);
    Ok(())
}

/// The page of `admin_geoedit` for a player at geo cell `cell` looking `direction`.
fn geo_edit_page(
    mut content: String,
    direction: i32,
    geo: &Geo<'_>,
    cell: (i32, i32),
    player_z: i32,
) -> String {
    // Follow player heading.
    let [n, e, s, w] = compass(direction);
    content = content.replace("%N%", n);
    content = content.replace("%E%", e);
    content = content.replace("%S%", s);
    content = content.replace("%W%", w);

    const GEO_RADIUS: i32 = 9;
    for dx in -GEO_RADIUS..=GEO_RADIUS {
        for dy in -GEO_RADIUS..=GEO_RADIUS {
            let (translated_dx, translated_dy) = match direction {
                0 => (dx, dy),
                1 => (dy, -dx),
                2 => (-dx, -dy),
                _ => (-dy, dx),
            };
            let (gx, gy) = (cell.0 + dx, cell.1 + dy);
            content = content.replace(
                &format!("xy_{translated_dx}_{translated_dy}"),
                &format!("{gx} {gy}"),
            );
            let z = geo.nearest_z(gx, gy, player_z);
            let all_open = [NSWE_NORTH, NSWE_EAST, NSWE_SOUTH, NSWE_WEST]
                .iter()
                .all(|nswe| geo.check_nswe(gx, gy, z, *nswe));
            content = content.replace(
                &format!("bg_{translated_dx}_{translated_dy}"),
                if all_open { ARROW_ON } else { ARROW_OFF },
            );
        }
    }
    content
}

/// Java `admin_ge <geoX> <geoY>`: the four sides of one cell.
fn geo_edit_cell(world: &mut World, gm: ObjectId, st: &mut Tokens<'_>) -> Outcome {
    let content = geo_page(world, gm, "data/html/admin/geoedit_cell.htm")?;
    let direction = player_direction(world.gm_heading(gm));
    let gx = parse_int(st.next_token().ok_or_else(no_such_element)?)?;
    let gy = parse_int(st.next_token().ok_or_else(no_such_element)?)?;
    let (_, _, z) = world.gm_position(gm)?;
    let page = geo_edit_cell_page(content, direction, &world.geo_view(), (gx, gy), z);
    world.send_cb_html(gm, &page);
    Ok(())
}

/// The page of `admin_ge` for the cell `(gx, gy)`.
fn geo_edit_cell_page(
    mut content: String,
    direction: i32,
    geo: &Geo<'_>,
    cell: (i32, i32),
    player_z: i32,
) -> String {
    // Follow player heading: the placeholders turn with it.
    if direction == 0 {
        content = content.replace('%', "");
    } else {
        let [n, e, s, w] = compass(direction);
        for (from, to) in [("%N%", n), ("%E%", e), ("%S%", s), ("%W%", w)] {
            content = content.replace(from, to);
        }
        let side = |letter: &str| letter.to_ascii_lowercase();
        let turned = [side(n), side(e), side(s), side(w)];
        for (i, from) in ["n", "e", "s", "w"].iter().enumerate() {
            content = content.replace(&format!("%bg_{from}%"), &format!("bg_{}", turned[i]));
        }
        for (i, from) in ["n", "e", "s", "w"].iter().enumerate() {
            content = content.replace(&format!("%cmd_{from}%"), &format!("cmd_{}", turned[i]));
        }
    }

    let (gx, gy) = cell;
    let z = geo.nearest_z(gx, gy, player_z);
    // (nswe flag, the letter of the side, disable command, enable command)
    let sides: [(u8, &str, &str, &str); 4] = [
        (NSWE_NORTH, "n", "dn", "en"),
        (NSWE_EAST, "e", "de", "ee"),
        (NSWE_SOUTH, "s", "ds", "es"),
        (NSWE_WEST, "w", "dw", "ew"),
    ];
    for (nswe, letter, disable, enable) in sides {
        let (bg, cmd) = if geo.check_nswe(gx, gy, z, nswe) {
            (ARROW_ON, disable)
        } else {
            (ARROW_OFF, enable)
        };
        content = content.replace(&format!("bg_{letter}"), bg);
        content = content.replace(&format!("cmd_{letter}"), &format!("{cmd} {gx} {gy}"));
    }
    content
}

// ------------------------------------------------------------------------------ GeoUtils

/// `GeoUtils.getDirectionColor`: green when the direction is open, red otherwise.
fn direction_color(geo: &Geo<'_>, gx: i32, gy: i32, z: i32, nswe: u8) -> PrimitiveColor {
    if geo.check_nswe(gx, gy, z, nswe) {
        PrimitiveColor::GREEN
    } else {
        PrimitiveColor::RED
    }
}

const GRID_RADIUS: i32 = 20;
const BLOCKS_PER_PACKET: i32 = 40;

/// Java `GeoUtils.debugGrid`: the four arrows of every cell within 20 cells, 40 cells a packet.
fn debug_grid(world: &mut World, gm: ObjectId) -> Outcome {
    let (px, py, pz) = world.gm_position(gm)?;
    let (player_gx, player_gy) = (geo_x(px), geo_y(py));
    let mut packets = Vec::new();
    let mut current: Option<ExServerPrimitive> = None;
    let mut i_block = BLOCKS_PER_PACKET;
    let mut i_packet = 0;
    {
        let geo = world.geo_view();
        for dx in -GRID_RADIUS..=GRID_RADIUS {
            for dy in -GRID_RADIUS..=GRID_RADIUS {
                if i_block >= BLOCKS_PER_PACKET {
                    i_block = 0;
                    if let Some(done) = current.take() {
                        i_packet += 1;
                        packets.push(done);
                    }
                    current = Some(ExServerPrimitive::new(
                        &format!("DebugGrid_{i_packet}"),
                        px,
                        py,
                        -16000,
                    ));
                }
                let Some(exsp) = current.as_mut() else {
                    continue;
                };
                let (gx, gy) = (player_gx + dx, player_gy + dy);
                let (x, y) = (world_x(gx), world_y(gy));
                let z = geo.nearest_z(gx, gy, pz);

                // north arrow
                let col = direction_color(&geo, gx, gy, z, NSWE_NORTH);
                exsp.add_line(col, (x - 1, y - 7, z), (x + 1, y - 7, z));
                exsp.add_line(col, (x - 2, y - 6, z), (x + 2, y - 6, z));
                exsp.add_line(col, (x - 3, y - 5, z), (x + 3, y - 5, z));
                exsp.add_line(col, (x - 4, y - 4, z), (x + 4, y - 4, z));
                // east arrow
                let col = direction_color(&geo, gx, gy, z, NSWE_EAST);
                exsp.add_line(col, (x + 7, y - 1, z), (x + 7, y + 1, z));
                exsp.add_line(col, (x + 6, y - 2, z), (x + 6, y + 2, z));
                exsp.add_line(col, (x + 5, y - 3, z), (x + 5, y + 3, z));
                exsp.add_line(col, (x + 4, y - 4, z), (x + 4, y + 4, z));
                // south arrow
                let col = direction_color(&geo, gx, gy, z, NSWE_SOUTH);
                exsp.add_line(col, (x - 1, y + 7, z), (x + 1, y + 7, z));
                exsp.add_line(col, (x - 2, y + 6, z), (x + 2, y + 6, z));
                exsp.add_line(col, (x - 3, y + 5, z), (x + 3, y + 5, z));
                exsp.add_line(col, (x - 4, y + 4, z), (x + 4, y + 4, z));
                // west arrow
                let col = direction_color(&geo, gx, gy, z, NSWE_WEST);
                exsp.add_line(col, (x - 7, y - 1, z), (x - 7, y + 1, z));
                exsp.add_line(col, (x - 6, y - 2, z), (x - 6, y + 2, z));
                exsp.add_line(col, (x - 5, y - 3, z), (x - 5, y + 3, z));
                exsp.add_line(col, (x - 4, y - 4, z), (x - 4, y + 4, z));

                i_block += 1;
            }
        }
    }
    packets.extend(current);
    for packet in &packets {
        world.send(gm, packet);
    }
    Ok(())
}

/// Java `GeoUtils.hideDebugGrid`: the same packet names with a black dot per cell, which replaces
/// the drawing.
fn hide_debug_grid(world: &mut World, gm: ObjectId) -> Outcome {
    let (px, py, _) = world.gm_position(gm)?;
    let (player_gx, player_gy) = (geo_x(px), geo_y(py));
    let mut packets = Vec::new();
    let mut current: Option<ExServerPrimitive> = None;
    let mut i_block = BLOCKS_PER_PACKET;
    let mut i_packet = 0;
    for dx in -GRID_RADIUS..=GRID_RADIUS {
        for dy in -GRID_RADIUS..=GRID_RADIUS {
            if i_block >= BLOCKS_PER_PACKET {
                i_block = 0;
                if let Some(done) = current.take() {
                    i_packet += 1;
                    packets.push(done);
                }
                current = Some(ExServerPrimitive::new(
                    &format!("DebugGrid_{i_packet}"),
                    px,
                    py,
                    -16000,
                ));
            }
            let Some(exsp) = current.as_mut() else {
                continue;
            };
            let (x, y) = (world_x(player_gx + dx), world_y(player_gy + dy));
            // Nothing.
            exsp.add_line(PrimitiveColor::BLACK, (x, y, -16000), (x, y, -16000));
            i_block += 1;
        }
    }
    packets.extend(current);
    for packet in &packets {
        world.send(gm, packet);
    }
    Ok(())
}

// ------------------------------------------------------------------------------ AdminRegions

/// Java `AdminRegions.onCommand`: the boxes of the world regions around the GM, or their removal.
fn admin_regions(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let (x, y, _) = world.gm_position(gm)?;
    if command.contains("clear") {
        let mut exsp = ExServerPrimitive::new("DebugRegion", x, y, -16000);
        exsp.add_line(PrimitiveColor::BLACK, (x, y, -16000), (x, y, -16000));
        world.send(gm, &exsp);
        return Ok(());
    }
    let Some(region) = world.objects.get(&gm).and_then(|e| e.region) else {
        return Ok(());
    };
    let mut exsp = ExServerPrimitive::new("DebugRegion", x, y, -16000);
    for wr in region.surrounding() {
        // Convert region coordinates to world coordinates.
        let min_x = (wr.x - OFFSET_X) << SHIFT_BY;
        let max_x = min_x + (1 << SHIFT_BY);
        let min_y = (wr.y - OFFSET_Y) << SHIFT_BY;
        let max_y = min_y + (1 << SHIFT_BY);
        // Calculate Z boundaries from region Z index.
        let min_z = WORLD_Z_MIN + (wr.z * Z_REGION_SIZE);
        let max_z = min_z + Z_REGION_SIZE;
        // Determine color - highlight current region differently.
        let color = if wr == region {
            PrimitiveColor::ORANGE
        } else {
            PrimitiveColor::GREEN
        };
        // Draw bottom rectangle (minZ).
        exsp.add_line(color, (min_x, min_y, min_z), (max_x, min_y, min_z));
        exsp.add_line(color, (max_x, min_y, min_z), (max_x, max_y, min_z));
        exsp.add_line(color, (max_x, max_y, min_z), (min_x, max_y, min_z));
        exsp.add_line(color, (min_x, max_y, min_z), (min_x, min_y, min_z));
        // Draw top rectangle (maxZ).
        exsp.add_line(color, (min_x, min_y, max_z), (max_x, min_y, max_z));
        exsp.add_line(color, (max_x, min_y, max_z), (max_x, max_y, max_z));
        exsp.add_line(color, (max_x, max_y, max_z), (min_x, max_y, max_z));
        exsp.add_line(color, (min_x, max_y, max_z), (min_x, min_y, max_z));
        // Draw vertical edges connecting bottom and top.
        exsp.add_line(color, (min_x, min_y, min_z), (min_x, min_y, max_z));
        exsp.add_line(color, (max_x, min_y, min_z), (max_x, min_y, max_z));
        exsp.add_line(color, (max_x, max_y, min_z), (max_x, max_y, max_z));
        exsp.add_line(color, (min_x, max_y, min_z), (min_x, max_y, max_z));
    }
    world.send(gm, &exsp);
    Ok(())
}

// ------------------------------------------------------------------------------ AdminPathNode

impl World {
    /// Installs the `GeoEngine.ini` weights and buffers the `//path_find` search uses (the same
    /// the path workers use). Without them the Java defaults apply.
    pub fn set_admin_path_options(&mut self, options: PathOptions) {
        self.admin.path.options = Some(options);
        self.admin.path.finder = None;
    }
}

/// Java `AdminPathNode.onCommand`: the path from the GM to the target, one message per node.
fn admin_path_find(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command != "admin_path_find" {
        return Ok(());
    }
    // `GeoEngineConfig.PATHFINDING < 1` (an engine without regions has it switched off).
    let pathfinding = world.geo.as_ref().map_or(0, |g| g.options().pathfinding);
    if pathfinding < 1 {
        world.send_sys_message(gm, "PathFinding is disabled.");
        return Ok(());
    }
    let Some(target) = world.target_of(gm) else {
        world.send_sys_message(gm, "No Target!");
        return Ok(());
    };
    let (x, y, z) = world.gm_position(gm)?;
    let (tx, ty, tz) = world.position_of(target).ok_or_else(npe)?;
    let instance = world.objects.get(&gm).map_or(0, |e| e.instance_id);
    // `(short) z`.
    let from = Location::new(x, y, i32::from(z as i16));
    let to = Location::new(tx, ty, i32::from(tz as i16));

    let Some(geo) = world.geo.clone() else {
        return Ok(());
    };
    let options = world.admin.path.options.clone().unwrap_or_default();
    let mut finder = match world.admin.path.finder.take() {
        Some(finder) => finder,
        None => PathFinder::new(options)
            .map_err(|e| JavaException(format!("java.lang.IllegalStateException: {e}")))?,
    };
    let path = finder.find_path(&geo, &world.obstacles(), from, to, instance, true);
    world.admin.path.finder = Some(finder);

    let Some(path) = path else {
        world.send_sys_message(gm, "No Route!");
        return Ok(());
    };
    for node in path {
        world.send_sys_message(gm, &format!("x:{} y:{} z:{}", node.x(), node.y(), node.z()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn directions_follow_the_heading() {
        assert_eq!(player_direction(0), 0);
        assert_eq!(player_direction(8191), 0);
        assert_eq!(player_direction(8192), 1);
        assert_eq!(player_direction(24575), 1);
        assert_eq!(player_direction(24576), 2);
        assert_eq!(player_direction(40960), 3);
        // NOTE(java-quirk): 57344 itself is still north (`heading > 57344` is the west test).
        assert_eq!(player_direction(57344), 3);
        assert_eq!(player_direction(57345), 0);
    }

    #[test]
    fn the_cell_page_turns_with_the_player() {
        let template = "%N%|%bg_n%|%cmd_n%|%bg_e%|%cmd_e%".to_owned();
        // Facing north: the percent signs just go.
        let geo = Geo(None);
        let page = geo_edit_cell_page(template.clone(), 0, &geo, (5, 6), 0);
        // No geodata: every side is open, so the buttons switch it off.
        assert_eq!(
            page,
            "N|L2UI_CH3.minibar_food|dn 5 6|L2UI_CH3.minibar_food|de 5 6"
        );
        // Facing east: the north button is the east side, shown as "E".
        let page = geo_edit_cell_page(template, 1, &geo, (5, 6), 0);
        assert!(page.starts_with("E|"));
    }
}
