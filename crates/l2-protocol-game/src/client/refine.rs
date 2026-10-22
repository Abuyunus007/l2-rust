//! Client packets of augmentation (the "variation" windows of the life stone NPCs): picking the
//! item, the life stone and the gemstones, pressing "Augment", and the removal of an augmentation.
//!
//! These only read the packet. Every check (is the item valid, does the player have the
//! gemstones) is the world's job (`l2_world::augment`).

// Java: gameserver/network/clientpackets/RequestConfirmTargetItem.java
client_packet!(
    /// The player put the item to augment into the window (`0xD0` 0x0026).
    RequestConfirmTargetItem,
    ex RequestConfirmTargetItem {
        object_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestConfirmRefinerItem.java
client_packet!(
    /// The player put the life stone into the window (`0xD0` 0x0027).
    RequestConfirmRefinerItem,
    ex RequestConfirmRefinerItem {
        target_item_object_id: i32,
        refiner_item_object_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestConfirmGemStone.java
client_packet!(
    /// The player put the gemstones into the window (`0xD0` 0x0028).
    RequestConfirmGemStone,
    ex RequestConfirmGemstone {
        target_item_object_id: i32,
        refiner_item_object_id: i32,
        gemstone_item_object_id: i32,
        gemstone_count: i64,
    }
);

// Java: gameserver/network/clientpackets/RequestRefine.java
client_packet!(
    /// The player pressed "Augment" (`0xD0` 0x0041).
    RequestRefine,
    ex RequestRefine {
        target_item_object_id: i32,
        refiner_item_object_id: i32,
        gemstone_item_object_id: i32,
        gemstone_count: i64,
    }
);

// Java: gameserver/network/clientpackets/RequestConfirmCancelItem.java
client_packet!(
    /// The player put an augmented item into the removal window (`0xD0` 0x0042).
    RequestConfirmCancelItem,
    ex RequestConfirmCancelItem {
        object_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestRefineCancel.java
client_packet!(
    /// The player pressed "Remove augmentation" (`0xD0` 0x0043).
    RequestRefineCancel,
    ex RequestRefineCancel {
        target_item_object_id: i32,
    }
);
