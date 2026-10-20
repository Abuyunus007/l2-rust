//! The `OpenDoor` effect: the caster tries to open a door with a chance `chance`. A door that is
//! not openable by skill refuses (unless the effect is `isItem`).
// Java: dist/game/data/scripts/handlers/skill/effects/OpenDoor.java

use l2_data::door::OPEN_BY_SKILL;
use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;

use super::params;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::formulas::Rnd;
use crate::world::World;

/// Java `OpenDoor`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OpenDoor {
    chance: i32,
    is_item: bool,
}

impl OpenDoor {
    /// Java `OpenDoor(attachCond, applyCond, set, params)`: `chance` (default 0), `isItem` (false).
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Self {
            chance: params::int(template, "chance", 0)?,
            is_item: params::boolean(template, "isItem", false)?,
        }))
    }
}

impl EffectHandler for OpenDoor {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let Some(door) = world.doors.get(&cx.effected) else {
            return;
        };
        let (open_type, open) = (door.template.open_type, door.open);
        // NOTE: Java searches the caster's instance for the door when the instances differ. The
        // instance manager is not in the world yet, so a door of another instance is never found.
        let effector_instance = world.stats(cx.effector).map_or(0, |v| v.instance_id());
        let door_instance = world.objects.get(&cx.effected).map_or(0, |e| e.instance_id);
        if effector_instance != door_instance {
            return;
        }
        // NOTE: Java also refuses a fort's door (`door.getFort() != null`); forts are not in the
        // world yet, so no door is one.
        let openable_by_skill = open_type & OPEN_BY_SKILL == OPEN_BY_SKILL;
        if !openable_by_skill && !self.is_item {
            world.send(
                cx.effector,
                &SystemMessage::new(SystemMessageId::ThisDoorCannotBeUnlocked),
            );
            return;
        }
        // Java draws first, then tests the door: the draw happens even for an open door.
        let roll = world.rng().get_int(100) < self.chance;
        if roll && !open {
            world.open_door(cx.effected);
        } else {
            world.send(
                cx.effector,
                &SystemMessage::new(SystemMessageId::YouHaveFailedToUnlockTheDoor),
            );
        }
    }
}
