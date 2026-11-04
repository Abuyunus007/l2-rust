//! Client packets of item enchanting and element attributes: the scroll window, the support item
//! and the attribute stone window.
//!
//! These only read the packet. Every check (is the scroll in use, is the item valid) is the
//! world's job (`l2_world::enchant`).

client_packet!(
    /// The player pressed "Enchant" in the scroll window (`REQUEST_ENCHANT_ITEM`, `0x5F`).
    RequestEnchantItem,
    plain RequestEnchantItem {
        /// The item to enchant. Java has a second int (the support item) commented out; the
        /// support item is the one the player put in with `RequestExTryToPutEnchantSupportItem`.
        object_id: i32,
    }
);

client_packet!(
    /// The player put the item to enchant into the window (`0xD0` 0x004C).
    RequestExTryToPutEnchantTargetItem,
    ex RequestExTryToPutEnchantTargetItem {
        object_id: i32,
    }
);

client_packet!(
    /// The player put a support item (a "scroll of enchant rate") into the window (`0xD0` 0x004D).
    RequestExTryToPutEnchantSupportItem,
    ex RequestExTryToPutEnchantSupportItem {
        support_object_id: i32,
        enchant_object_id: i32,
    }
);

client_unit_packet!(
    /// The player closed the enchant window (`0xD0` 0x004E).
    RequestExCancelEnchantItem,
    ex RequestExCancelEnchantItem
);

client_packet!(
    /// The player picked the item to put the element on (`0xD0` 0x0035). `-1` (Java compares
    /// with `0xFFFFFFFF`) means the window was cancelled.
    RequestExEnchantItemAttribute,
    ex RequestExEnchantItemAttribute {
        object_id: i32,
    }
);

client_packet!(
    /// The player asks an NPC to take an element off an item (`0xD0` 0x0023).
    RequestExRemoveItemAttribute,
    ex RequestExRemoveItemAttribute {
        object_id: i32,
        /// Java reads an int and casts it to a `byte`.
        element: i32,
    }
);
