//! `DoorShiftClick`: the GM window of a door (`doorinfo.htm`).
// Java: dist/game/data/scripts/handlers/actions/shiftclick/DoorShiftClick.java

use l2_data::door::OPEN_BY_SKILL;
use l2_util::ObjectId;

use crate::world::World;

impl World {
    /// Java `DoorShiftClick.onAction`: the GM targets the door, gets its `StaticObjectInfo` (doors
    /// are always targetable for a GM) and the page with the id, the hit points and the corners.
    pub(super) fn door_shift_click(&mut self, gm: ObjectId, target: ObjectId) {
        if !self.is_gm(gm) {
            return;
        }
        self.set_target(gm, Some(target));
        let Some(door) = self.doors.get(&target) else {
            return;
        };
        let (info, _, _) = door.info_packets(true);
        let values = [
            ("%class%", "Door".to_owned()),
            ("%hp%", (door.current_hp() as i32).to_string()),
            ("%hpmax%", door.max_hp().to_string()),
            ("%objid%", target.0.to_string()),
            ("%doorid%", door.door_id.to_string()),
            ("%minx%", door.template.node_x[0].to_string()),
            ("%miny%", door.template.node_y[0].to_string()),
            ("%minz%", door.z_min().to_string()),
            ("%maxx%", door.template.node_x[2].to_string()),
            ("%maxy%", door.template.node_y[2].to_string()),
            ("%maxz%", door.z_max().to_string()),
            (
                "%unlock%",
                if door.template.open_type & OPEN_BY_SKILL == OPEN_BY_SKILL {
                    "<font color=00FF00>YES<font>".to_owned()
                } else {
                    "<font color=FF0000>NO</font>".to_owned()
                },
            ),
        ];
        self.send(gm, &info);
        let mut html = self.admin_html_file(gm, "data/html/admin/doorinfo.htm");
        for (pattern, value) in &values {
            html.replace(pattern, value);
        }
        self.send_admin_html(gm, &html);
    }
}
