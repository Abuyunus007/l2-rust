//! The shift click of a GM (`Action` with `actionId` 1): the info windows of NPCs, doors, static
//! objects and items, and the character and summon info pages.
// Java: dist/game/data/scripts/handlers/actions/shiftclick/{NpcShiftClick,PlayerShiftClick,
// Java: DoorShiftClick,SummonShiftClick,ItemShiftClick,StaticObjectShiftClick}.java
// Java: entity/WorldObject.java (onActionShift), handler/ActionShiftHandler.java
// Java: network/clientpackets/Action.java (the `case 1` branch)
//
// # Which handler runs
// `ActionShiftHandler.getHandler(instanceType)` walks up the instance type tree, so every NPC class
// (Monster, Folk, RaidBoss, ...) gets `NpcShiftClick`. Objects with no handler (fences, traps that
// are not NPCs) only get the `ActionFailed` every shift click ends with.
//
// # Left out
// - `AltGameViewNpc` (a player shift clicks an NPC and gets `NpcViewMod`'s page): the page is
//   `bypass/npc/NpcViewMod.java`, left for its own card (see the registry test of
//   `tests/handler_registry.rs`). `target.rs` therefore lets only GMs shift click.
// - The "Fake Player - " class prefix needs `isFakePlayer()` of the NPC: it comes from the
//   template's `fake_player` flag, which exists only with fake players on (not ported).

mod door;
mod npc;

use l2_protocol_game::server::{ActionFailed, NpcHtmlMessage};
use l2_util::ObjectId;

use crate::objects::ObjectKind;
use crate::world::World;

impl World {
    /// Java `WorldObject.onActionShift(player)`: the handler of the object's instance type, then
    /// `ActionFailed`. The caller checked that the player may shift click (a GM).
    pub(crate) fn on_action_shift(&mut self, player: ObjectId, obj: ObjectId) {
        match self.objects.get(&obj).map(|e| e.kind) {
            Some(ObjectKind::Npc) => self.npc_shift_click(player, obj),
            Some(ObjectKind::Player) => self.player_shift_click(player, obj),
            Some(ObjectKind::Summon) => self.summon_shift_click(player, obj),
            Some(ObjectKind::Door) => self.door_shift_click(player, obj),
            Some(ObjectKind::StaticObject) => self.static_object_shift_click(player, obj),
            Some(ObjectKind::Item) => self.item_shift_click(player, obj),
            _ => {}
        }
        self.send(player, &ActionFailed);
    }

    /// Java `PlayerShiftClick.onAction`: the GM targets the player and runs
    /// `admin_character_info <name>` as a bypass of its own (with the confirmation switched on).
    fn player_shift_click(&mut self, gm: ObjectId, target: ObjectId) {
        if !self.is_gm(gm) {
            return;
        }
        // Check if the GM already target this player.
        if self.target_of(gm) != Some(target) {
            self.set_target(gm, Some(target));
        }
        let name = self.object_name(target).unwrap_or_default();
        self.admin_command(gm, &format!("admin_character_info {name}"), true);
    }

    /// Java `SummonShiftClick.onAction`: the GM targets the summon and runs `admin_summon_info`.
    fn summon_shift_click(&mut self, gm: ObjectId, target: ObjectId) {
        if !self.is_gm(gm) {
            return;
        }
        if self.target_of(gm) != Some(target) {
            self.set_target(gm, Some(target));
        }
        self.admin_command(gm, "admin_summon_info", true);
    }

    /// Java `ItemShiftClick.onAction`: a small page with the ids and the place of a ground item.
    fn item_shift_click(&mut self, gm: ObjectId, item: ObjectId) {
        if !self.is_gm(gm) {
            return;
        }
        let Some(ground) = self.ground_item(item) else {
            return;
        };
        let item_id = ground.data.item_id;
        let owner_id = ground.owner.map_or(0, |o| o.0);
        let (x, y, z) = (ground.x, ground.y, ground.z);
        let instance = self.objects.get(&item).map_or(0, |e| e.instance_id);
        let name = self
            .data
            .stat_data()
            .and_then(|d| d.items.get(item_id))
            .map(|t| t.name.clone())
            .unwrap_or_default();
        let html = item_info_html(&name, item.0, item_id, owner_id, x, y, z, instance);
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html(gm, &message);
        }
    }

    /// Java `StaticObjectShiftClick.onAction`: the GM targets the object, gets its
    /// `StaticObjectInfo` and a small page.
    fn static_object_shift_click(&mut self, gm: ObjectId, target: ObjectId) {
        if !self.is_gm(gm) {
            return;
        }
        self.set_target(gm, Some(target));
        let Some(object) = self.static_object(target) else {
            return;
        };
        let packet = l2_protocol_game::server::StaticObjectInfo::static_object(
            object.static_object_id,
            object.object_id.0,
            object.mesh_index,
        );
        let html = static_object_info_html(
            (object.x, object.y, object.z),
            object.object_id.0,
            object.static_object_id,
            object.mesh_index,
        );
        self.send(gm, &packet);
        if let Some(message) = NpcHtmlMessage::new(0, &html) {
            self.send_html(gm, &message);
        }
    }
}

/// The page of `ItemShiftClick`. `Location.toString()` is `[Location] X: .. Y: .. Z: ..
/// Heading: .. InstanceId: ..`; an item on the ground has heading 0.
#[allow(clippy::too_many_arguments)]
pub(crate) fn item_info_html(
    name: &str,
    object_id: i32,
    item_id: i32,
    owner_id: i32,
    x: i32,
    y: i32,
    z: i32,
    instance_id: i32,
) -> String {
    format!(
        "<html><head><title>{name}</title></head><body><center><font color=\"LEVEL\">Item Info</font></center><br><table border=0><tr><td>Object ID: </td><td>{object_id}</td></tr><tr><td>Item ID: </td><td>{item_id}</td></tr><tr><td>Owner ID: </td><td>{owner_id}</td></tr><tr><td>Location: </td><td>[Location] X: {x} Y: {y} Z: {z} Heading: 0 InstanceId: {instance_id}</td></tr><tr><td><br></td></tr><tr><td>Class: </td><td>Item</td></tr></table></body></html>"
    )
}

/// The page of `StaticObjectShiftClick`.
pub(crate) fn static_object_info_html(
    (x, y, z): (i32, i32, i32),
    object_id: i32,
    static_object_id: i32,
    mesh_index: i32,
) -> String {
    format!(
        "<html><body><center><font color=\"LEVEL\">Static Object Info</font></center><br><table border=0><tr><td>Coords X,Y,Z: </td><td>{x}, {y}, {z}</td></tr><tr><td>Object ID: </td><td>{object_id}</td></tr><tr><td>Static Object ID: </td><td>{static_object_id}</td></tr><tr><td>Mesh Index: </td><td>{mesh_index}</td></tr><tr><td><br></td></tr><tr><td>Class: </td><td>StaticObject</td></tr></table></body></html>"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn item_page_is_the_java_string() {
        let html = item_info_html("Wooden Arrow", 7, 17, 3, 1, -2, 3, 0);
        assert_eq!(
            html,
            "<html><head><title>Wooden Arrow</title></head><body><center><font color=\"LEVEL\">Item Info</font></center><br><table border=0><tr><td>Object ID: </td><td>7</td></tr><tr><td>Item ID: </td><td>17</td></tr><tr><td>Owner ID: </td><td>3</td></tr><tr><td>Location: </td><td>[Location] X: 1 Y: -2 Z: 3 Heading: 0 InstanceId: 0</td></tr><tr><td><br></td></tr><tr><td>Class: </td><td>Item</td></tr></table></body></html>"
        );
    }

    #[test]
    fn static_object_page_is_the_java_string() {
        let html = static_object_info_html((10, -20, 30), 5, 9, 1);
        assert!(html.starts_with("<html><body><center><font color=\"LEVEL\">Static Object Info</font>"));
        assert!(html.contains("<td>Coords X,Y,Z: </td><td>10, -20, 30</td>"));
        assert!(html.contains("<td>Mesh Index: </td><td>1</td>"));
        assert!(html.ends_with("<td>Class: </td><td>StaticObject</td></tr></table></body></html>"));
    }
}
