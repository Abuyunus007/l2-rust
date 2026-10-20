//! The position change of the instant movement effects (`Blink`, `ThrowUp`, `TeleportToTarget`):
//! the creature is set down at the destination and everybody who sees it gets `ValidateLocation`.
// Java: Creature.setXYZ, and the `ValidateLocation` broadcast of Blink.java, ThrowUp.java and
// TeleportToTarget.java. The `FlyToLocation` before it is `World::fly_to_location`
// (`effects/control.rs`), called by the three handlers.

use l2_geo::Location;
use l2_protocol_game::server::ValidateLocation;
use l2_util::ObjectId;

use crate::world::World;

/// Java `Creature.setXYZ(destination)` followed by `broadcastPacket(new ValidateLocation(this))`.
pub(crate) fn set_position_validated(world: &mut World, id: ObjectId, destination: Location) {
    world.set_xyz(id, destination.x, destination.y, destination.z);
    let packet = ValidateLocation {
        object_id: id.0,
        x: destination.x,
        y: destination.y,
        z: destination.z,
        heading: world.heading_of(id),
    };
    world.broadcast_with_self(id, &packet);
}

/// Java `GeoEngine.getValidLocation(fromX, fromY, fromZ, toX, toY, toZ, instanceId)` for the
/// creature `id`: the farthest point of the line that the geodata lets it reach.
pub(crate) fn valid_location(world: &World, id: ObjectId, from: Location, to: Location) -> Location {
    let instance = world.stats(id).map_or(0, |v| v.instance_id());
    world.geo_valid_location(from, to, instance)
}
