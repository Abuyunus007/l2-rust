//! What a form allows and forbids: the action list, the inventory block, and the conditions
//! `canTransform` / `canUntransform` of the transformation skills.
// Java: network/serverpackets/ExBasicActionList.java (ACTIONS_ON_TRANSFORM),
//       network/clientpackets/RequestActionUse.java (the transformed check),
//       entity/itemcontainer/PlayerInventory.java (setInventoryBlock, unblock, hasInventoryBlock,
//       canManipulateWithItemId), network/serverpackets/AbstractItemPacket.java (writeInventoryBlock)
// Java: mechanics/conditions/ConditionPlayerCanTransform.java, ConditionPlayerCanUntransform.java

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{InventoryBlock as BlockPacket, SystemMessage};
use l2_util::ObjectId;

use super::InventoryBlock;
use crate::stats::StatView;
use crate::world::World;
use crate::zones::ZoneId;

/// Java `ExBasicActionList.ACTIONS_ON_TRANSFORM`: the only action ids a transformed player may
/// use (sorted: Java binary-searches it).
pub const ACTIONS_ON_TRANSFORM: [i32; 135] = [
    1, 2, 3, 4, 5, 6, 7, 8, 9, 11, 15, 16, 17, 18, 19, 21, 22, 23, 32, 36, 39, 40, 41, 42, 43, 44, 45, 46,
    47, 48, 50, 52, 53, 54, 55, 56, 57, 63, 64, 65, 70, 1000, 1001, 1003, 1004, 1005, 1006, 1007, 1008, 1009,
    1010, 1011, 1012, 1013, 1014, 1015, 1016, 1017, 1018, 1019, 1020, 1021, 1022, 1023, 1024, 1025, 1026,
    1027, 1028, 1029, 1030, 1031, 1032, 1033, 1034, 1035, 1036, 1037, 1038, 1039, 1040, 1041, 1042, 1043,
    1044, 1045, 1046, 1047, 1048, 1049, 1050, 1051, 1052, 1053, 1054, 1055, 1056, 1057, 1058, 1059, 1060,
    1061, 1062, 1063, 1064, 1065, 1066, 1067, 1068, 1069, 1070, 1071, 1072, 1073, 1074, 1075, 1076, 1077,
    1078, 1079, 1080, 1081, 1082, 1083, 1084, 1089, 1090, 1091, 1092, 1093, 1094, 1095, 1096, 1097, 1098,
];

impl World {
    /// Java `Player.isTransformed()`.
    pub fn is_transformed(&self, id: ObjectId) -> bool {
        self.stats(id).is_some_and(|v| v.is_transformed())
    }

    /// Java `Player.isInStance()`.
    pub fn is_in_stance(&self, id: ObjectId) -> bool {
        self.stats(id).is_some_and(|v| v.is_in_stance())
    }

    /// Java `Player.getTransformationId()`: 0 when not transformed.
    pub fn transformation_id(&self, id: ObjectId) -> i32 {
        self.stats(id).map_or(0, |v| v.transformation_id())
    }

    /// The check at the top of `RequestActionUse.runImpl`: a transformed player may only use the
    /// actions of [`ACTIONS_ON_TRANSFORM`]. `false` means "ActionFailed, and a warning" (see
    /// `summon/actions.rs`).
    pub fn transformed_action_allowed(&self, id: ObjectId, action_id: i32) -> bool {
        !self.is_transformed(id) || ACTIONS_ON_TRANSFORM.binary_search(&action_id).is_ok()
    }

    /// Java `PlayerInventory.setInventoryBlock(items, mode)`: the item list follows.
    pub fn set_inventory_block(&mut self, id: ObjectId, items: Vec<i32>, mode: i32) {
        if let Some(player) = self.players.get_mut(&id) {
            player.transform.block = Some(InventoryBlock { mode, items });
        }
        self.send_item_list(id, false);
    }

    /// Java `PlayerInventory.unblock()`.
    pub fn unblock_inventory(&mut self, id: ObjectId) {
        if let Some(player) = self.players.get_mut(&id) {
            player.transform.block = None;
        }
        self.send_item_list(id, false);
    }

    /// Java `PlayerInventory.canManipulateWithItemId(itemId)`.
    pub fn can_manipulate_with_item_id(&self, id: ObjectId, item_id: i32) -> bool {
        self.players
            .get(&id)
            .and_then(|p| p.transform.block.as_ref())
            .is_none_or(|block| block.can_manipulate(item_id))
    }

    /// `AbstractItemPacket.writeInventoryBlock`: the block the `ItemList` and `ExQuestItemList`
    /// carry, `None` when the inventory has no block.
    pub(crate) fn inventory_block_packet(&self, id: ObjectId) -> Option<BlockPacket> {
        let block = self.players.get(&id)?.transform.block.as_ref()?;
        block.is_active().then(|| BlockPacket {
            mode: u8::try_from(block.mode).unwrap_or(0),
            items: block.items.clone(),
        })
    }
}

/// Java `ConditionPlayerCanTransform.testImpl`: whether the effector may transform. The reason
/// goes to the player as a system message.
pub(crate) fn can_transform(a: &StatView<'_>) -> bool {
    if a.player().is_none() {
        return false;
    }
    let world = a.world();
    let id = a.object_id();
    let tell = |message: SystemMessageId| {
        world.send(id, &SystemMessage::new(message));
    };
    // `|| player.isCursedWeaponEquipped()`: cursed weapons are M8.
    if world.is_alike_dead(id) {
        return false;
    }
    if world.is_sitting(id) {
        tell(SystemMessageId::YouCannotTransformWhileSitting);
        false
    } else if a.is_transformed() || a.is_in_stance() {
        tell(SystemMessageId::YouAlreadyPolymorphedAndCannotPolymorphAgain);
        false
    } else if a.in_water() {
        tell(SystemMessageId::YouCannotPolymorphIntoTheDesiredFormInWater);
        false
    } else if a.is_flying_mounted() {
        // `|| player.isMounted()`: mounts are not modelled.
        tell(SystemMessageId::YouCannotPolymorphWhileRidingAPet);
        false
    } else {
        // `isRegisteredOnEvent()`: the events are scripts (D-05).
        true
    }
}

/// Java `ConditionPlayerCanUntransform.testImpl`.
pub(crate) fn can_untransform(a: &StatView<'_>) -> bool {
    let Some(player) = a.player() else {
        return false;
    };
    let world = a.world();
    let id = a.object_id();
    // `|| player.isCursedWeaponEquipped()`: cursed weapons are M8.
    if world.is_alike_dead(id) {
        return false;
    }
    if (a.is_transformed() || a.is_in_stance())
        && a.is_flying_mounted()
        && !player.zones.is_inside(ZoneId::Landing)
    {
        // "TODO: check if message is retail like."
        world.send(
            id,
            &SystemMessage::new(
                SystemMessageId::YouAreTooHighToPerformThisActionPleaseLowerYourAltitudeAndTryAgain,
            ),
        );
        return false;
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn action_table_is_sorted_and_java_sized() {
        assert!(ACTIONS_ON_TRANSFORM.windows(2).all(|w| w[0] < w[1]));
        assert_eq!(ACTIONS_ON_TRANSFORM.len(), 135);
        assert!(ACTIONS_ON_TRANSFORM.binary_search(&1000).is_ok());
        // Sit/stand (0), the mount action (10) and the pet attacks are not in the list.
        assert!(ACTIONS_ON_TRANSFORM.binary_search(&0).is_err());
        assert!(ACTIONS_ON_TRANSFORM.binary_search(&10).is_err());
        assert!(ACTIONS_ON_TRANSFORM.binary_search(&1099).is_err());
    }

    #[test]
    fn block_modes() {
        let only = InventoryBlock {
            mode: 1,
            items: vec![57, 9819],
        };
        assert!(only.can_manipulate(57) && !only.can_manipulate(1));
        let not = InventoryBlock {
            mode: 0,
            items: vec![57],
        };
        assert!(!not.can_manipulate(57) && not.can_manipulate(1));
        // Mode 2 (a form's "not allowed" list) restricts the client only.
        let client_only = InventoryBlock {
            mode: 2,
            items: vec![57],
        };
        assert!(client_only.can_manipulate(57) && client_only.can_manipulate(1));
        assert!(
            only.is_active()
                && !InventoryBlock {
                    mode: 1,
                    items: vec![]
                }
                .is_active()
        );
    }
}
