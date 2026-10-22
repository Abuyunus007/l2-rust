//! Server packets of crafting: the recipe book, the craft window and the manufacture (dwarven
//! workshop) shop (M6-CRAFT-01).
//!
//! Values only, like the other server packet modules: Java builds these from `Player` and the
//! recipe books, the caller passes the numbers. A recipe list is written as pairs of the recipe
//! list id and its position (1, 2, 3, ...); the position is not stored, it is the index.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// Reads `count` then `count` pairs of (recipe list id, position); the position is dropped.
fn read_recipe_ids(r: &mut PacketReader) -> Result<Vec<i32>, PacketError> {
    let count = read_count(r, "recipes")?;
    let mut ids = Vec::new();
    for _ in 0..count {
        ids.push(r.i32()?);
        let _position = r.i32()?;
    }
    Ok(ids)
}

/// Writes the recipes the way Java does: the count, then each id with its 1-based position.
fn write_recipe_ids(w: &mut PacketWriter, ids: &[i32]) {
    w.i32(ids.len() as i32);
    for (index, id) in ids.iter().enumerate() {
        w.i32(*id).i32(index as i32 + 1);
    }
}

/// One recipe of a manufacture shop with its price.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManufactureEntry {
    /// The recipe list id.
    pub recipe_id: i32,
    /// `ManufactureItem.getCost()`.
    pub cost: i64,
}

fn read_manufacture(r: &mut PacketReader) -> Result<Vec<ManufactureEntry>, PacketError> {
    let count = read_count(r, "manufacture items")?;
    let mut items = Vec::new();
    for _ in 0..count {
        let recipe_id = r.i32()?;
        let _unknown = r.i32()?;
        items.push(ManufactureEntry {
            recipe_id,
            cost: r.i64()?,
        });
    }
    Ok(items)
}

fn write_manufacture(w: &mut PacketWriter, items: &[ManufactureEntry]) {
    w.i32(items.len() as i32);
    for item in items {
        w.i32(item.recipe_id).i32(0).i64(item.cost);
    }
}

// Java: gameserver/network/serverpackets/RecipeBookItemList.java
/// The recipes of a recipe book (`RECIPE_BOOK_ITEM_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeBookItemList {
    /// Java writes `!_isDwarvenCraft`: 0 is the dwarven book, 1 the common one.
    pub is_common: bool,
    /// `getMaxMp()`: the book shows the MP bar of the crafter.
    pub max_mp: i32,
    /// The recipe list ids in the order of the book.
    pub recipes: Vec<i32>,
}

impl RecipeBookItemList {
    pub const OPCODE: ServerOpcode = ServerOpcode::RecipeBookItemList;
}

impl Encode for RecipeBookItemList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(i32::from(self.is_common)).i32(self.max_mp);
        write_recipe_ids(w, &self.recipes);
    }
}

impl Decode for RecipeBookItemList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            is_common: r.i32()? != 0,
            max_mp: r.i32()?,
            recipes: read_recipe_ids(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/RecipeItemMakeInfo.java
/// The craft window of a recipe: the crafter's MP and whether the last craft worked
/// (`RECIPE_ITEM_MAKE_INFO`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeItemMakeInfo {
    /// The recipe list id.
    pub id: i32,
    /// Java writes `!recipe.isDwarvenRecipe()`: 0 is dwarven, 1 common.
    pub is_common: bool,
    /// `(int) getCurrentMp()`.
    pub cur_mp: i32,
    pub max_mp: i32,
    /// "item creation success/failed".
    pub success: bool,
}

impl RecipeItemMakeInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::RecipeItemMakeInfo;
}

impl Encode for RecipeItemMakeInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.id)
            .i32(i32::from(self.is_common))
            .i32(self.cur_mp)
            .i32(self.max_mp)
            .i32(i32::from(self.success));
    }
}

impl Decode for RecipeItemMakeInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            id: r.i32()?,
            is_common: r.i32()? != 0,
            cur_mp: r.i32()?,
            max_mp: r.i32()?,
            success: r.i32()? != 0,
        })
    }
}

// Java: gameserver/network/serverpackets/RecipeShopItemInfo.java
/// A recipe of a manufacture shop as the customer sees it, with the manufacturer's MP
/// (`RECIPE_SHOP_ITEM_INFO`). Java ends it with the constant `0xffffffff`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeShopItemInfo {
    /// The manufacturer's object id.
    pub object_id: i32,
    pub recipe_id: i32,
    /// `(int) getCurrentMp()` of the manufacturer.
    pub cur_mp: i32,
    pub max_mp: i32,
}

impl RecipeShopItemInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::RecipeShopItemInfo;
}

impl Encode for RecipeShopItemInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.recipe_id)
            .i32(self.cur_mp)
            .i32(self.max_mp)
            .i32(-1);
    }
}

impl Decode for RecipeShopItemInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let packet = Self {
            object_id: r.i32()?,
            recipe_id: r.i32()?,
            cur_mp: r.i32()?,
            max_mp: r.i32()?,
        };
        let _always_minus_one = r.i32()?;
        Ok(packet)
    }
}

// Java: gameserver/network/serverpackets/RecipeShopManageList.java
/// The window where a crafter sets up its manufacture shop (`RECIPE_SHOP_MANAGE_LIST`): the
/// recipes it knows and the ones already for sale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeShopManageList {
    pub seller_id: i32,
    /// `(int) getAdena()`: Java narrows the long.
    pub adena: i32,
    /// Java writes `!_isDwarven`: 0 is the dwarven window, 1 the common one.
    pub is_common: bool,
    /// The recipe list ids of the book shown.
    pub recipes: Vec<i32>,
    /// The recipes already for sale, in the order of the shop.
    pub items: Vec<ManufactureEntry>,
}

impl RecipeShopManageList {
    pub const OPCODE: ServerOpcode = ServerOpcode::RecipeShopManageList;
}

impl Encode for RecipeShopManageList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.seller_id)
            .i32(self.adena)
            .i32(i32::from(self.is_common));
        write_recipe_ids(w, &self.recipes);
        write_manufacture(w, &self.items);
    }
}

impl Decode for RecipeShopManageList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            seller_id: r.i32()?,
            adena: r.i32()?,
            is_common: r.i32()? != 0,
            recipes: read_recipe_ids(r)?,
            items: read_manufacture(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/RecipeShopMsg.java
/// The title over a manufacture shop (`RECIPE_SHOP_MSG`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeShopMsg {
    pub object_id: i32,
    pub message: String,
}

impl RecipeShopMsg {
    pub const OPCODE: ServerOpcode = ServerOpcode::RecipeShopMsg;
}

impl Encode for RecipeShopMsg {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).string(&self.message);
    }
}

impl Decode for RecipeShopMsg {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            message: r.string(),
        })
    }
}

// Java: gameserver/network/serverpackets/RecipeShopSellList.java
/// A manufacture shop as a customer sees it (`RECIPE_SHOP_SELL_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecipeShopSellList {
    /// The manufacturer's object id.
    pub manufacturer_id: i32,
    /// `(int) getCurrentMp()` of the manufacturer.
    pub cur_mp: i32,
    pub max_mp: i32,
    /// The buyer's adena.
    pub buyer_adena: i64,
    pub items: Vec<ManufactureEntry>,
}

impl RecipeShopSellList {
    pub const OPCODE: ServerOpcode = ServerOpcode::RecipeShopSellList;
}

impl Encode for RecipeShopSellList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.manufacturer_id)
            .i32(self.cur_mp)
            .i32(self.max_mp)
            .i64(self.buyer_adena);
        write_manufacture(w, &self.items);
    }
}

impl Decode for RecipeShopSellList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            manufacturer_id: r.i32()?,
            cur_mp: r.i32()?,
            max_mp: r.i32()?,
            buyer_adena: r.i64()?,
            items: read_manufacture(r)?,
        })
    }
}
