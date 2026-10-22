//! The `FishShots` item handler (fishing soulshots) and the lure going to the left hand.
// Java: dist/game/data/scripts/handlers/items/FishShots.java
// Java: network/clientpackets/UseItem.java (the fishing rod branch of the handler part)

use l2_data::ItemTemplate;
use l2_data::item::types::{ActionType, WeaponType};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_protocol_game::server::charscreen::paperdoll as pd;
use l2_util::ObjectId;
use tracing::warn;

use crate::inventory::handlers::shots::ShotType;
use crate::inventory::weapon_type;
use crate::player::ItemData;
use crate::world::World;

/// Java `UseItem`: the ids of the lures (the green, purple and yellow ones of every grade, the
/// box lures and the prize-winning ones).
pub(crate) fn is_lure_item(item_id: i32) -> bool {
    (6519..=6527).contains(&item_id)
        || (7610..=7613).contains(&item_id)
        || (7807..=7809).contains(&item_id)
        || (8484..=8486).contains(&item_id)
        || (8505..=8513).contains(&item_id)
}

/// Java `UseItem`: the only items usable while fishing are the fishing shots.
pub(crate) fn is_usable_while_fishing(item_id: i32) -> bool {
    (6535..=6540).contains(&item_id)
}

impl World {
    /// The rod branch of `UseItem`: with a fishing rod in hand, a lure goes to `PAPERDOLL_LHAND`
    /// and the client gets the new user info and the item list. `true` when it did.
    pub(crate) fn put_lure_on_rod(&mut self, id: ObjectId, item: &ItemData) -> bool {
        let rod = self
            .stats(id)
            .and_then(|v| v.active_weapon_item())
            .and_then(weapon_type)
            == Some(WeaponType::FishingRod);
        if !(rod && is_lure_item(item.item_id)) {
            return false;
        }
        let mut changed = Vec::new();
        self.set_paperdoll_item(id, pd::LHAND, Some(item.object_id), &mut changed);
        self.broadcast_user_info(id);
        // Send a Server->Client packet ItemList to this Player to update left hand equipment.
        self.send_item_list(id, false);
        true
    }

    /// Java `FishShots.onItemUse`: charges the rod with a fishing soulshot, which doubles the
    /// damage of the next pump or reel.
    pub(crate) fn fish_shots_on_use(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
    ) -> bool {
        // The weapon instance and its template: a rod in the right hand.
        let Some(weapon_object) = self.paperdoll_object(id, pd::RHAND) else {
            return false;
        };
        let weapon = self.stats(id).and_then(|v| v.active_weapon_item());
        if weapon.and_then(weapon_type) != Some(WeaponType::FishingRod) {
            return false;
        }
        let weapon_grade = weapon.map(ItemTemplate::crystal_type_plus);

        if self.is_charged_shot(id, ShotType::FishSoulshots) {
            return false;
        }

        let Some(skill) = template
            .skills
            .item_skills
            .as_ref()
            .and_then(|s| s.first().copied())
        else {
            warn!("FishShots: is missing skills!");
            return false;
        };

        let grade_check = template.is_etc_item()
            && template.default_action == ActionType::FishingShot
            && weapon_grade == Some(template.crystal_type_plus());
        if !grade_check {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ThatIsTheWrongGradeOfSoulshotForThatFishingPole),
            );
            return false;
        }

        if item.count < 1 {
            return false;
        }

        self.set_charged_shot(id, ShotType::FishSoulshots, true);
        self.destroy_item_by_object_id(id, item.object_id, 1, false);
        // `player.setTarget(player)` around the broadcast sends nothing: not modelled.
        let _ = weapon_object;
        self.broadcast_self_skill_use(id, skill.id, skill.level);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lure_ids_are_the_ones_of_use_item() {
        for id in [6519, 6527, 7610, 7613, 7807, 7809, 8484, 8486, 8505, 8513] {
            assert!(is_lure_item(id), "{id}");
        }
        for id in [6518, 6528, 7609, 7614, 7806, 7810, 8483, 8487, 8504, 8514] {
            assert!(!is_lure_item(id), "{id}");
        }
    }

    #[test]
    fn only_the_fishing_shots_are_usable_while_fishing() {
        assert!(!is_usable_while_fishing(6534));
        assert!(is_usable_while_fishing(6535));
        assert!(is_usable_while_fishing(6540));
        assert!(!is_usable_while_fishing(6541));
    }
}
