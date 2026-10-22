//! Client packets of crafting: the recipe book and the manufacture (dwarven workshop) shop
//! (M6-CRAFT-01).
//!
//! Two opcodes have no packet here on purpose. `REQUEST_RECIPE_SHOP_MANAGE_LIST` (0xB9) and
//! `REQUEST_RECIPE_SHOP_MANAGE_CANCEL` (0xBD) map to `null` in Java's `ClientPackets`: the Java
//! class `RequestRecipeShopManageList` exists, but nothing creates it, so the server drops both
//! packets. The client opens the manufacture window with `RequestActionUse` (actions 37 and 51)
//! instead.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/RequestRecipeBookOpen.java
client_packet!(
    /// The player opens its recipe book (`REQUEST_RECIPE_BOOK_OPEN`).
    RequestRecipeBookOpen,
    plain RequestRecipeBookOpen {
        /// Java reads `readInt() == 0` as "dwarven"; any other value is the common book.
        recipe_type: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestRecipeBookDestroy.java
client_packet!(
    /// The player forgets a recipe (`REQUEST_RECIPE_BOOK_DESTROY`).
    RequestRecipeBookDestroy,
    plain RequestRecipeBookDestroy {
        /// The recipe list id (`RecipeList.getId()`), not the item id of the recipe.
        recipe_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestRecipeItemMakeInfo.java
client_packet!(
    /// The player looks at a recipe in its own book (`REQUEST_RECIPE_ITEM_MAKE_INFO`).
    RequestRecipeItemMakeInfo,
    plain RequestRecipeItemMakeInfo {
        id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestRecipeItemMakeSelf.java
client_packet!(
    /// The player crafts an item for itself (`REQUEST_RECIPE_ITEM_MAKE_SELF`).
    RequestRecipeItemMakeSelf,
    plain RequestRecipeItemMakeSelf {
        id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestRecipeShopMessageSet.java
client_packet!(
    /// The title of the manufacture shop (`REQUEST_RECIPE_SHOP_MESSAGE_SET`). The handler
    /// punishes a title longer than 29 characters.
    RequestRecipeShopMessageSet,
    plain RequestRecipeShopMessageSet {
        name: String,
    }
);

// Java: gameserver/network/clientpackets/RequestRecipeShopManageQuit.java
client_unit_packet!(
    /// The player closes its manufacture shop (`REQUEST_RECIPE_SHOP_MANAGE_QUIT`).
    RequestRecipeShopManageQuit,
    plain RequestRecipeShopManageQuit
);

// Java: gameserver/network/clientpackets/RequestRecipeShopMakeInfo.java
client_packet!(
    /// A customer looks at a recipe of a manufacture shop (`REQUEST_RECIPE_SHOP_MAKE_INFO`).
    RequestRecipeShopMakeInfo,
    plain RequestRecipeShopMakeInfo {
        /// The object id of the manufacturer.
        player_object_id: i32,
        recipe_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestRecipeShopMakeItem.java
client_packet!(
    /// A customer orders an item from a manufacture shop (`REQUEST_RECIPE_SHOP_MAKE_ITEM`).
    RequestRecipeShopMakeItem,
    plain RequestRecipeShopMakeItem {
        /// The object id of the manufacturer.
        id: i32,
        recipe_id: i32,
        /// Java reads a long it never uses.
        unknown: i64,
    }
);

// Java: gameserver/network/clientpackets/RequestRecipeShopManagePrev.java
client_unit_packet!(
    /// The player previews the shop of its target (`REQUEST_RECIPE_SHOP_MANAGE_PREV`).
    RequestRecipeShopManagePrev,
    plain RequestRecipeShopManagePrev
);

/// Bytes per entry of [`RequestRecipeShopListSet`]: an int recipe id and a long price.
pub const BATCH_LENGTH: usize = 12;

/// One recipe of the shop the player sets (Java `ManufactureItem(recipeId, cost)`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeShopListEntry {
    /// The recipe list id.
    pub recipe_id: i32,
    /// The price of one craft.
    pub cost: i64,
}

// Java: gameserver/network/clientpackets/RequestRecipeShopListSet.java
/// The player confirms the recipes and prices of its manufacture shop
/// (`REQUEST_RECIPE_SHOP_LIST_SET`).
///
/// Java keeps its "bad packet" rule: when the count is out of range, the byte length does not
/// match the count or one price is negative, it reads no items and the handler closes the shop.
/// That is `items: None` here. Java also rejects a count above `PlayerConfig.MAX_ITEM_IN_PACKET`;
/// that limit comes from the configuration, so the world compares `items.len()` with it and
/// treats a longer list as `None`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestRecipeShopListSet {
    pub items: Option<Vec<RecipeShopListEntry>>,
}

impl RequestRecipeShopListSet {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestRecipeShopListSet;
}

impl Encode for RequestRecipeShopListSet {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // A list Java drops is the count 0, which it drops the same way.
        w.i32(self.items.as_ref().map_or(0, |v| v.len() as i32));
        for item in self.items.iter().flatten() {
            w.i32(item.recipe_id).i64(item.cost);
        }
    }
}

impl Decode for RequestRecipeShopListSet {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = r.i32()?;
        // Java multiplies in `int`; a count that would wrap is far above MAX_ITEM_IN_PACKET,
        // which the world rejects, so the product is computed without wrapping here.
        let expected = i64::from(count) * BATCH_LENGTH as i64;
        if count < 1 || expected != r.remaining() as i64 {
            return Ok(Self { items: None });
        }
        let mut items = Vec::new();
        for _ in 0..count {
            let entry = RecipeShopListEntry {
                recipe_id: r.i32()?,
                cost: r.i64()?,
            };
            // `if (cost < 0) { _items = null; return; }`
            if entry.cost < 0 {
                return Ok(Self { items: None });
            }
            items.push(entry);
        }
        Ok(Self { items: Some(items) })
    }
}
