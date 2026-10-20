//! `EnemyCharge`: the caster rushes to its target.
// Java: dist/game/data/scripts/handlers/skill/effects/EnemyCharge.java

use l2_data::skill::{EffectTemplate, FlyType};
use l2_geo::Location;
use l2_protocol_game::server::ValidateLocation;
use tracing::info;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::world::World;

/// Java `EnemyCharge`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct EnemyCharge;

impl EnemyCharge {
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(EnemyCharge))
    }
}

impl EffectHandler for EnemyCharge {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let effector = cx.effector;
        if world.is_movement_disabled(effector) {
            return;
        }
        // Get current position of the Creature.
        let (Some((cur_x, cur_y, cur_z)), Some((target_x, target_y, target_z))) =
            (world.position_of(effector), world.position_of(cx.effected))
        else {
            return;
        };
        // Calculate distance (dx,dy) between current position and destination.
        let dx = f64::from(target_x.wrapping_sub(cur_x));
        let dy = f64::from(target_y.wrapping_sub(cur_y));
        let distance = ((dx * dx) + (dy * dy)).sqrt();
        if distance > 2000.0 {
            info!(
                "EffectEnemyCharge was going to use invalid coordinates for characters, getEffector: {cur_x},{cur_y} and getEffected: {target_x},{target_y}"
            );
            return;
        }
        let dz = f64::from(target_z.wrapping_sub(cur_z));
        let mut offset = cx.skill.fly_radius.max(30);
        // Approximation for moving closer when z coordinates are different.
        // TODO(java): handle Z axis movement better. `offset -= abs(dz)` narrows back to `int`.
        offset = (f64::from(offset) - dz.abs()) as i32;
        if offset < 5 {
            offset = 5;
        }
        // If no distance.
        if distance < 1.0 || (distance - f64::from(offset)) <= 0.0 {
            return;
        }
        // Calculate movement angles needed.
        let sin = dy / distance;
        let cos = dx / distance;
        // Calculate the new destination with offset included.
        let x = cur_x.wrapping_add(((distance - f64::from(offset)) * cos) as i32);
        let y = cur_y.wrapping_add(((distance - f64::from(offset)) * sin) as i32);
        let z = target_z;
        let instance = world.objects.get(&effector).map_or(0, |e| e.instance_id);
        let destination = world.geo_valid_location(
            Location::new(cur_x, cur_y, cur_z),
            Location::new(x, y, z),
            instance,
        );
        world.fly_to_location(
            effector,
            destination.x,
            destination.y,
            destination.z,
            FlyType::Charge,
        );
        world.set_xyz(effector, destination.x, destination.y, destination.z);
        if let Some((px, py, pz)) = world.position_of(effector) {
            let heading = world.heading_of(effector);
            world.broadcast_creature_packet(
                effector,
                &ValidateLocation {
                    object_id: effector.0,
                    x: px,
                    y: py,
                    z: pz,
                    heading,
                },
            );
        }
    }
}
