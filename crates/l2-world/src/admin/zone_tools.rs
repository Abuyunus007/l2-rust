//! The zone admin commands: `//zone_check`, `//zone_visual`, `//zone_visual_clear` and the zone
//! builder (`//zone_build`, `_toggle`, `_clear`, `_delete`, `_save`).
// Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminZone,AdminZoneBuild}.java
// Java: managers/ZoneBuildManager.java, managers/ZoneManager.java (getZones, getZoneById,
// Java: getSpawnTerritories, getDebugItems, clearDebugItems)
// Java: entity/zone/ZoneForm.java (dropDebugItem), network/clientpackets/{MoveToLocation,
// Java: DlgAnswer}.java (the tele mode 3 click and the save question)
//
// # How the builder works
// `//zone_build_toggle` puts the GM in tele mode 3. While it is on, every click on the ground
// (`MoveToLocation`) records a point instead of moving ([`World::zone_build_add_point`]) and draws
// the points with `ExServerPrimitive`. `//zone_build_save` asks `ConfirmDlg`; a yes writes
// `data/zones/<name>-<millis>.xml` (a `ScriptZone` over the points) and clears the points.
//
// Left out: `NpcSpawnTerritory`s other than the ones the zone table holds (there are none in the
// shipped datapack), and the file is written right in the command (a few lines of xml, once per
// GM click on Save) instead of through the persistence task.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::PathBuf;

use l2_data::ZoneForm;
use l2_data::map_region::MapRegions;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ConfirmDlg, ExServerPrimitive, PrimitiveColor, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::{AdminHandler, AdminHandlers, JavaException, Tokens, is_numeric, parse_int};
use crate::ground_item::GroundItem;
use crate::objects::ObjectKind;
use crate::region::{WORLD_X_MAX, WORLD_X_MIN, WORLD_Y_MAX, WORLD_Y_MIN};
use crate::teleport::TeleportWhereType;
use crate::world::World;
use crate::zones::ZoneId;

type Outcome = Result<(), JavaException>;

/// `Inventory.ADENA_ID`: the item the debug markers are made of.
const ADENA_ID: i32 = 57;

/// Java `MasterHandler`: the commands of `AdminZone` and `AdminZoneBuild`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    let table: &[(&str, AdminHandler)] = &[
        ("admin_zone_check", admin_zone),
        ("admin_zone_visual", admin_zone),
        ("admin_zone_visual_clear", admin_zone),
        ("admin_zone_build", admin_zone_build),
        ("admin_zone_build_save", admin_zone_build),
        ("admin_zone_build_delete", admin_zone_build),
        ("admin_zone_build_toggle", admin_zone_build),
        ("admin_zone_build_clear", admin_zone_build),
    ];
    for (command, handler) in table {
        handlers.register(command, *handler);
    }
}

/// The zone tools' state in the world.
pub(crate) struct ZoneAdmin {
    /// Java `ZoneBuildManager.PLAYER_LOCATIONS`: the points each GM recorded.
    points: HashMap<ObjectId, Vec<(i32, i32, i32)>>,
    /// Java `PlayerAction.ADMIN_SAVE_ZONE`: the GMs the save question waits for.
    save_pending: HashSet<ObjectId>,
    /// Java `ZoneManager._debugItems`: the markers of `//zone_visual`, by object id.
    pub(crate) debug_items: BTreeSet<ObjectId>,
    /// Where `//zone_build_save` writes (Java: `data/zones` below the working directory).
    zone_dir: PathBuf,
}

impl Default for ZoneAdmin {
    fn default() -> Self {
        Self {
            points: HashMap::new(),
            save_pending: HashSet::new(),
            debug_items: BTreeSet::new(),
            zone_dir: PathBuf::from("data/zones"),
        }
    }
}

fn no_such_element() -> JavaException {
    JavaException("java.util.NoSuchElementException".to_owned())
}

fn npe() -> JavaException {
    JavaException("java.lang.NullPointerException".to_owned())
}

impl World {
    /// Sets the folder `//zone_build_save` writes its xml into (default: `data/zones` relative to
    /// the working directory, like Java). It is never a folder of the repository or of the
    /// reference datapack: the game server passes `<root>/data/zones` of the server's own copy.
    pub fn set_zone_build_dir(&mut self, dir: PathBuf) {
        self.admin.zones.zone_dir = dir;
    }

    /// Java `Player.getTeleMode()`.
    pub fn tele_mode(&self, id: ObjectId) -> i32 {
        self.players.get(&id).map_or(0, |p| p.movement.tele_mode)
    }

    /// Java `Player.setTeleMode(mode)`: the click-to-teleport mode of `//teleto` and `//instant_move`.
    pub(crate) fn set_tele_mode(&mut self, id: ObjectId, mode: i32) {
        if let Some(p) = self.players.get_mut(&id) {
            p.movement.tele_mode = mode;
        }
    }

    /// The `MoveToLocation` branch of a GM with a tele mode (Java `teleMode > 0`): `true` when the
    /// click was used up (the caller sends `ActionFailed` and stops).
    pub(crate) fn tele_mode_click(&mut self, id: ObjectId, x: i32, y: i32, z: i32) -> bool {
        let mode = self.tele_mode(id);
        if mode <= 0 {
            return false;
        }
        if mode == 3 {
            // Admin zone build.
            self.zone_build_add_point(id, (x, y, z));
            return true;
        }
        if mode == 1 {
            self.set_tele_mode(id, 0);
        }
        self.tele_to_location(id, crate::teleport::TeleportTarget::at(x, y, z));
        true
    }

    /// Java `PlayerAction.ADMIN_SAVE_ZONE` in `DlgAnswer`: the answer to "Do you wish to save the
    /// zone?". `false` when the GM was not asked (the answer is another question's).
    pub(crate) fn zone_save_answer(&mut self, id: ObjectId, answer: i32) -> bool {
        if !self.admin.zones.save_pending.remove(&id) {
            return false;
        }
        if answer != 0 {
            self.zone_build_save(id);
        }
        true
    }

    /// A GM leaves the world: Java's `Player` object, and so its tele mode, the recorded points
    /// and the open question, are gone with it.
    pub(crate) fn zone_tools_on_leave(&mut self, id: ObjectId) {
        self.admin.zones.points.remove(&id);
        self.admin.zones.save_pending.remove(&id);
    }

    // ---------------------------------------------------------------------- the debug markers

    /// Java `ZoneForm.dropDebugItem(ADENA_ID, 1, x, y, z)`: `new Item(nextId, 57)`,
    /// `spawnMe(x, y, z + 5)`, remembered for `clearDebugItems`.
    fn drop_debug_item(&mut self, x: i32, y: i32, z: i32) {
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(ADENA_ID) else {
            return;
        };
        let Some(id) = self.allocate_item_id() else {
            return;
        };
        let mut item = GroundItem::new(self.new_item_data(template, id, 1));
        // `WorldObject.spawnMe(x, y, z)`: a place outside the world is pulled back in.
        let mut spawn_x = x;
        if spawn_x > WORLD_X_MAX {
            spawn_x = WORLD_X_MAX - 5000;
        }
        if spawn_x < WORLD_X_MIN {
            spawn_x = WORLD_X_MIN + 5000;
        }
        let mut spawn_y = y;
        if spawn_y > WORLD_Y_MAX {
            spawn_y = WORLD_Y_MAX - 5000;
        }
        if spawn_y < WORLD_Y_MIN {
            spawn_y = WORLD_Y_MIN + 5000;
        }
        (item.x, item.y, item.z) = (spawn_x, spawn_y, z + 5);
        self.ground.items.insert(id, item);
        self.register_object(id, ObjectKind::Item);
        self.spawn(id);
        self.admin.zones.debug_items.insert(id);
    }

    /// Java `ZoneType.visualizeZone(z)` / `NpcSpawnTerritory.visualizeZone(z)`.
    fn visualize_form(&mut self, form: &ZoneForm, z: i32) {
        for (x, y) in form.visualize_points() {
            self.drop_debug_item(x, y, z);
        }
    }

    /// Java `ZoneManager.clearDebugItems()`: every marker vanishes.
    fn clear_debug_items(&mut self) {
        for id in std::mem::take(&mut self.admin.zones.debug_items) {
            self.decay_ground_item(id);
        }
    }
}

// ------------------------------------------------------------------------------- AdminZone

/// Java `AdminZone.onCommand`.
fn admin_zone(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut st = Tokens::spaces(command);
    let actual = st.next_token().ok_or_else(no_such_element)?;
    let (x, y, z) = world.position_of(gm).ok_or_else(npe)?;

    if actual.eq_ignore_ascii_case("admin_zone_check") {
        show_zone_html(world, gm)?;
        let (map_x, map_y, loc_id, town) = match world.zoning.map_regions.as_ref() {
            Some(regions) => (
                MapRegions::map_region_x(x),
                MapRegions::map_region_y(y),
                regions.map_region_loc_id(x, y),
                regions.closest_town_name(x, y).to_owned(),
            ),
            // No map regions installed: the tile numbers still follow from the position.
            None => (
                MapRegions::map_region_x(x),
                MapRegions::map_region_y(y),
                0,
                "Aden Castle Town".to_owned(),
            ),
        };
        world.send_sys_message(gm, &format!("MapRegion: x:{map_x} y:{map_y} ({loc_id})"));
        // getGeoRegionXY
        let geo_region_x = (((x - -327_680) >> 4) >> 11) + 10;
        let geo_region_y = (((y - -262_144) >> 4) >> 11) + 10;
        world.send_sys_message(gm, &format!("GeoRegion: {geo_region_x}_{geo_region_y}"));
        world.send_sys_message(gm, &format!("Closest Town: {town}"));

        // Prevent exit instance variable deletion.
        let in_instance = world.objects.get(&gm).is_some_and(|e| e.instance_id != 0);
        if !in_instance {
            for (label, kind) in [
                ("Castle", TeleportWhereType::Castle),
                ("ClanHall", TeleportWhereType::ClanHall),
                ("SiegeFlag", TeleportWhereType::SiegeFlag),
                ("Town", TeleportWhereType::Town),
            ] {
                let (lx, ly, lz) = world.get_tele_to_location(gm, kind).ok_or_else(npe)?;
                world.send_sys_message(gm, &format!("TeleToLocation ({label}): x:{lx} y:{ly} z:{lz}"));
            }
        }
    } else if actual.eq_ignore_ascii_case("admin_zone_visual") {
        let next = st.next_token().ok_or_else(no_such_element)?;
        if next.eq_ignore_ascii_case("all") {
            let zones = world.zoning.zones.clone();
            if let Some(zones) = zones {
                for zone in zones.zones_at(x, y, z) {
                    world.visualize_form(&zone.form, z);
                }
                for territory in zones.spawn_territories_at(x, y, z) {
                    world.visualize_form(&territory.form, z);
                }
            }
            show_zone_html(world, gm)?;
        } else {
            let zone_id = parse_int(next)?;
            let zones = world.zoning.zones.clone();
            // `getZoneById(zoneId)` is null for an unknown id and `visualizeZone` throws.
            let zone = zones.as_ref().and_then(|z| z.get(zone_id)).ok_or_else(npe)?;
            world.visualize_form(&zone.form, z);
        }
    } else if actual.eq_ignore_ascii_case("admin_zone_visual_clear") {
        world.clear_debug_items();
        show_zone_html(world, gm)?;
    }
    Ok(())
}

/// Java `AdminZone.showHtml`: the flags of the GM's zones and the list of the zones it is in.
fn show_zone_html(world: &mut World, gm: ObjectId) -> Outcome {
    let (x, y, z) = world.position_of(gm).ok_or_else(npe)?;
    let mut html = world.admin_html_file(gm, "data/html/admin/zone.htm");
    let yes_no = |zone: ZoneId| {
        if world.is_inside_zone(gm, zone) {
            "<font color=\"LEVEL\">YES</font>"
        } else {
            "NO"
        }
    };
    let flags = [
        ("%PEACE%", ZoneId::Peace),
        ("%PVP%", ZoneId::Pvp),
        ("%SIEGE%", ZoneId::Siege),
        ("%TOWN%", ZoneId::Town),
        ("%CASTLE%", ZoneId::Castle),
        ("%FORT%", ZoneId::Fort),
        ("%HQ%", ZoneId::Hq),
        ("%CLANHALL%", ZoneId::ClanHall),
        ("%LAND%", ZoneId::Landing),
        ("%NOLAND%", ZoneId::NoLanding),
        ("%NOSUMMON%", ZoneId::NoSummonFriend),
        ("%WATER%", ZoneId::Water),
        ("%SWAMP%", ZoneId::Swamp),
        ("%DANGER%", ZoneId::DangerArea),
        ("%NOSTORE%", ZoneId::NoStore),
        ("%SCRIPT%", ZoneId::Script),
    ];
    for (pattern, zone) in flags {
        html.replace(pattern, yes_no(zone));
    }

    let mut list = String::with_capacity(100);
    if let Some(zones) = world.zoning.zones.as_ref() {
        for zone in zones.zones_at(x, y, z) {
            match &zone.name {
                Some(name) => {
                    list.push_str(name);
                    if zone.id < 300_000 {
                        list.push_str(&format!(" ({})", zone.id));
                    }
                    list.push_str("<br1>");
                }
                None => list.push_str(&zone.id.to_string()),
            }
            list.push(' ');
        }
        for territory in zones.spawn_territories_at(x, y, z) {
            list.push_str(&territory.name);
            list.push_str("<br1>");
        }
    }
    html.replace("%ZLIST%", &list);
    world.send_admin_html(gm, &html);
    Ok(())
}

// --------------------------------------------------------------------------- AdminZoneBuild

/// Java `AdminZoneBuild.onCommand`.
fn admin_zone_build(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    if command == "admin_zone_build" {
        send_builder_html(world, gm);
    } else if command.starts_with("admin_zone_build_toggle") {
        if world.tele_mode(gm) == 3 {
            world.set_tele_mode(gm, 0);
            world.send_message(gm, "Draw zone disabled. You can move.");
        } else {
            world.set_tele_mode(gm, 3);
            world.send_message(gm, "Draw zone active. Click on point to record.");
        }
        send_builder_html(world, gm);
    } else if command.starts_with("admin_zone_build_clear") {
        world.zone_build_clear(gm);
        send_builder_html(world, gm);
    } else if command.starts_with("admin_zone_build_delete") {
        let mut st = Tokens::new(command);
        st.next_token();
        if let Some(next) = st.next_token()
            && is_numeric(next)
        {
            let entry = parse_int(next)?;
            world.zone_build_delete(gm, entry);
            send_builder_html(world, gm);
        }
    } else if command == "admin_zone_build_save" {
        world.admin.zones.save_pending.insert(gm);
        // `new ConfirmDlg(text)`: the `S1_3` message with the text.
        let dialog = ConfirmDlg::new(SystemMessageId::S13).with(SystemMessageParam::Text(
            "Do you wish to save the zone?".to_owned(),
        ));
        world.send(gm, &dialog);
    }
    Ok(())
}

/// Java `AdminZoneBuild.sendHtml`.
fn send_builder_html(world: &mut World, gm: ObjectId) {
    let mut html = world.admin_html_file(gm, "data/html/admin/zone_builder.htm");
    html.replace("%paths%", &world.zone_build_paths_html(gm));
    html.replace(
        "%movement%",
        if world.tele_mode(gm) == 3 {
            "Disable Draw"
        } else {
            "Enable Draw"
        },
    );
    world.send_admin_html(gm, &html);
}

/// Java `ZoneBuildManager`'s colours and packet naming.
const POINTS_PER_PACKET: usize = 10;

impl World {
    /// Java `ZoneBuildManager.getPathsForHtml`.
    fn zone_build_paths_html(&self, gm: ObjectId) -> String {
        let Some(points) = self.admin.zones.points.get(&gm).filter(|p| !p.is_empty()) else {
            return String::new();
        };
        let mut html = String::with_capacity(points.len() * 50);
        html.push_str("<table width=300>");
        for (i, (x, y, z)) in points.iter().enumerate() {
            html.push_str("<tr>");
            html.push_str(&format!("<td width=20>{i}</td>"));
            html.push_str(&format!("<td width=120>X: {x} Y: {y} Z: {z}</td>"));
            html.push_str(&format!(
                "<td width=80><button value=\"Delete\" action=\"bypass -h admin_zone_build_delete {i}\" width=65 height=21 back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"></td>"
            ));
            html.push_str("</tr>");
        }
        html.push_str("</table>");
        html
    }

    /// Java `ZoneBuildManager.addPoint`: records the click, draws, and shows the panel again.
    fn zone_build_add_point(&mut self, gm: ObjectId, point: (i32, i32, i32)) {
        self.admin.zones.points.entry(gm).or_default().push(point);
        // Java prints the `Location`, whose heading and instance are 0 here.
        self.send_message(
            gm,
            &format!(
                "Point saved [Location] X: {} Y: {} Z: {} Heading: 0 InstanceId: 0",
                point.0, point.1, point.2
            ),
        );
        self.zone_build_display(gm);
        self.admin_command(gm, "admin_zone_build", false);
    }

    /// Java `ZoneBuildManager.displayZones`: the points in red, joined by green lines, ten to a
    /// packet.
    fn zone_build_display(&mut self, gm: ObjectId) {
        let Some(points) = self
            .admin
            .zones
            .points
            .get(&gm)
            .filter(|p| !p.is_empty())
            .cloned()
        else {
            return;
        };
        let mut packet_count = 1;
        let first = points[0];
        let mut packet = ExServerPrimitive::new(
            &format!("ZoneBuilder{packet_count}"),
            first.0,
            first.1,
            65535 + first.2,
        );
        packet.add_named_point("0", PrimitiveColor::RED, true, first.0, first.1, first.2);
        for i in 1..points.len() {
            if i % POINTS_PER_PACKET == 0 {
                packet_count += 1;
                self.send(gm, &packet);
                let before = points[i - 1];
                packet = ExServerPrimitive::new(
                    &format!("ZoneBuilder{packet_count}"),
                    before.0,
                    before.1,
                    65535 + before.2,
                );
            }
            let (x, y, z) = points[i];
            packet.add_named_point(&i.to_string(), PrimitiveColor::RED, true, x, y, z);
            packet.add_line(PrimitiveColor::GREEN, points[i - 1], (x, y, z));
        }
        self.send(gm, &packet);
    }

    /// Java `ZoneBuildManager.clearZone`: forgets the points and wipes the drawing.
    fn zone_build_clear(&mut self, gm: ObjectId) {
        let Some(points) = self.admin.zones.points.remove(&gm).filter(|p| !p.is_empty()) else {
            return;
        };
        let Some((px, py, _)) = self.position_of(gm) else {
            return;
        };
        let wipe = |name: String, x: i32, y: i32| {
            let mut packet = ExServerPrimitive::new(&name, x, y, -16000);
            packet.add_point(PrimitiveColor::GREEN, 0, 0, i32::from(i16::MIN));
            packet
        };
        let mut packet_count = 1;
        let mut packet = wipe(format!("ZoneBuilder{packet_count}"), px, py);
        for i in 1..points.len() {
            if i % POINTS_PER_PACKET == 0 {
                packet_count += 1;
                self.send(gm, &packet);
                packet = wipe(
                    format!("ZoneBuilder{packet_count}"),
                    points[i - 1].0,
                    points[i - 1].1,
                );
            }
        }
        self.send(gm, &packet);
    }

    /// Java `ZoneBuildManager.deleteEntry`.
    fn zone_build_delete(&mut self, gm: ObjectId, entry: i32) {
        let Some(points) = self.admin.zones.points.get(&gm) else {
            return;
        };
        let Ok(entry) = usize::try_from(entry) else {
            return;
        };
        if entry >= points.len() {
            return;
        }
        let remaining: Vec<_> = points
            .iter()
            .enumerate()
            .filter(|(i, _)| *i != entry)
            .map(|(_, p)| *p)
            .collect();
        if remaining.len() <= 1 {
            self.zone_build_clear(gm);
        } else {
            self.admin.zones.points.insert(gm, remaining);
            self.zone_build_display(gm);
        }
    }

    /// Java `ZoneBuildManager.buildZone`: writes the xml and clears the points.
    fn zone_build_save(&mut self, gm: ObjectId) {
        let Some(points) = self
            .admin
            .zones
            .points
            .get(&gm)
            .filter(|p| !p.is_empty())
            .cloned()
        else {
            self.send_message(gm, "No path entries to save.");
            return;
        };
        let Some(name) = self.players.get(&gm).map(|p| p.data.name.clone()) else {
            return;
        };
        let now = crate::world::unix_ms_now();
        let file_name = format!("data/zones/{name}-{now}.xml");
        let z = points[0].2;
        let mut xml = String::new();
        xml.push_str("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n");
        xml.push_str("<list enabled=\"true\" xmlns:xsi=\"http://www.w3.org/2001/XMLSchema-instance\" xsi:noNamespaceSchemaLocation=\"../xsd/zones.xsd\">\n");
        xml.push_str(&format!(
            "\t<zone name=\"{name}_Zone_{now}\" type=\"ScriptZone\" shape=\"NPoly\" minZ=\"{}\" maxZ=\"{}\">\n",
            z - 1000,
            z + 1000
        ));
        for (x, y, _) in &points {
            xml.push_str(&format!("\t\t<node X=\"{x}\" Y=\"{y}\" />\n"));
        }
        xml.push_str("\t</zone>\n");
        xml.push_str("</list>");

        let path = self.admin.zones.zone_dir.join(format!("{name}-{now}.xml"));
        match std::fs::write(&path, xml) {
            Ok(()) => {
                self.send_message(gm, &format!("Zone saved at {file_name}"));
                self.admin_command(gm, "admin_zone_build_clear", false);
            }
            Err(e) => {
                warn!("ZoneBuildManager: Failed to save zone file {file_name}: {e}");
                self.send_message(gm, &format!("Failed to save zone: {e}"));
            }
        }
    }
}
