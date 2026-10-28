//! The `CharmOfCourage` item handler and the death rule of the charm.
// Java: dist/game/data/scripts/handlers/items/CharmOfCourage.java
// Java: entity/actor/Playable.java (doDie: the `hasCharmOfCourage()` block)
// Java: entity/actor/Player.java (setCharmOfCourage, hasCharmOfCourage)

use l2_data::item::ItemTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::player::ItemData;
use crate::world::World;

/// The charm grade a level needs (the `if`/`else if` chain of `onItemUse`).
pub(crate) fn required_grade(level: i32) -> i32 {
    if level < 20 {
        0
    } else if level < 40 {
        1
    } else if level < 52 {
        2
    } else if level < 61 {
        3
    } else if level < 76 {
        4
    } else {
        5
    }
}

impl World {
    /// Java `Player.setCharmOfCourage(value)`.
    pub(crate) fn set_charm_of_courage(&mut self, id: ObjectId, value: bool) {
        if let Some(control) = self.control_mut(id) {
            control.charm_of_courage = value;
        }
    }

    /// Java `CharmOfCourage.onItemUse`.
    pub(crate) fn charm_of_courage_on_use(&mut self, id: ObjectId, item: &ItemData, template: &ItemTemplate) -> bool {
        if !self.players.contains_key(&id) {
            return false;
        }
        let level = self.players.get(&id).map_or(1, |p| p.core.level);
        let item_level = template.crystal_type.level();
        if item_level < required_grade(level) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms)
                    .with(SystemMessageParam::ItemName(item.item_id)),
            );
            return false;
        }
        if self.destroy_item_by_object_id(id, item.object_id, 1, false) {
            self.set_charm_of_courage(id, true);
            self.send_etc_status_update(id);
            return true;
        }
        false
    }

    /// The charm block of `Playable.doDie`: in a siege the player may come back at once, and the
    /// charm is used up.
    pub(crate) fn charm_of_courage_on_death(&mut self, id: ObjectId) {
        if !self.players.contains_key(&id) || !self.has_charm_of_courage(id) {
            return;
        }
        if self.is_in_siege(id) {
            self.revive_request(id, id, false, 0);
        }
        self.set_charm_of_courage(id, false);
        self.send_etc_status_update(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grade_a_level_needs() {
        let grades: Vec<i32> = [1, 19, 20, 39, 40, 51, 52, 60, 61, 75, 76, 85]
            .into_iter()
            .map(required_grade)
            .collect();
        assert_eq!(grades, [0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5]);
    }
}
