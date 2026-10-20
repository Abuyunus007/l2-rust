//! Client packets about the player's pet: picking things up, moving items to and from its
//! inventory, using an item on it and renaming it.
//!
//! These only read the packet. The rules (is it a pet, is it close, can it carry that much) are
//! the world's job (the M6 summon and pet cards). The summon *commands* (attack, follow, unsummon
//! ...) arrive as `RequestActionUse` (see [`super::combat`]).

// Java: gameserver/network/clientpackets/RequestPetGetItem.java
client_packet!(
    /// The player orders its pet to pick up an item on the ground (`REQUEST_PET_GET_ITEM`).
    RequestPetGetItem,
    plain RequestPetGetItem {
        /// The object id of the ground item.
        object_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestGetItemFromPet.java
client_packet!(
    /// The player takes an item out of the pet's inventory (`REQUEST_GET_ITEM_FROM_PET`).
    RequestGetItemFromPet,
    plain RequestGetItemFromPet {
        object_id: i32,
        amount: i64,
        /// Java: "= 0 for most trades".
        unknown: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestGiveItemToPet.java
client_packet!(
    /// The player puts an item into the pet's inventory (`REQUEST_GIVE_ITEM_TO_PET`).
    RequestGiveItemToPet,
    plain RequestGiveItemToPet {
        object_id: i32,
        amount: i64,
    }
);

// Java: gameserver/network/clientpackets/RequestPetUseItem.java
client_packet!(
    /// The player uses an item from the pet's inventory (`REQUEST_PET_USE_ITEM`). Java's
    /// `readImpl` has a TODO: it reads the object id and nothing more.
    RequestPetUseItem,
    plain RequestPetUseItem {
        object_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestChangePetName.java
client_packet!(
    /// The player names its pet (`REQUEST_CHANGE_PET_NAME`).
    RequestChangePetName,
    plain RequestChangePetName {
        name: String,
    }
);
