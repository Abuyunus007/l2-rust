//! The `items` table, the augmentation rows of `item_attributes` and the element rows of
//! `item_elementals`: what an inventory, warehouse, pet or the paperdoll restores, and what an
//! item writes when it changes.
// Java: org/l2jmobius/gameserver/entity/item/instance/Item.java (restoreFromDb, insertIntoDb,
// Java: updateInDb, removeFromDb, restoreAttributes, updateItemAttributes, updateItemElements,
// Java: clearElementAttr, removeAugmentation),
// Java: entity/itemcontainer/ItemContainer.restore, Inventory.restore,
// Java: PlayerInventory.restoreVisibleInventory, entity/item/enums/ItemLocation.java
//
// Same layout as characters.rs: verbatim SQL constants, one method per statement, row and write
// structs that mirror the Java parameter lists.

use sqlx::FromRow;

use super::sql_number::SqlNumber;
use crate::error::Result;
use crate::pool::Pool;

// Java: ItemContainer.restore
const SELECT_ITEMS_OF_LOCATION: &str = "SELECT object_id, item_id, count, enchant_level, loc, loc_data, custom_type1, custom_type2, mana_left, time FROM items WHERE owner_id=? AND (loc=?)";
// Java: Inventory.restore
const SELECT_ITEMS_OF_INVENTORY: &str = "SELECT object_id, item_id, count, enchant_level, loc, loc_data, custom_type1, custom_type2, mana_left, time FROM items WHERE owner_id=? AND (loc=? OR loc=?) ORDER BY loc_data";
// Java: PlayerInventory.restoreVisibleInventory
const SELECT_PAPERDOLL_ITEMS: &str =
    "SELECT object_id,item_id,loc_data,enchant_level FROM items WHERE owner_id=? AND loc='PAPERDOLL'";
// Java: Item.insertIntoDb
const INSERT_ITEM: &str = "INSERT INTO items (owner_id,item_id,count,loc,loc_data,enchant_level,object_id,custom_type1,custom_type2,mana_left,time) VALUES (?,?,?,?,?,?,?,?,?,?,?)";
// Java: Item.updateInDb
const UPDATE_ITEM: &str = "UPDATE items SET owner_id=?,count=?,loc=?,loc_data=?,enchant_level=?,custom_type1=?,custom_type2=?,mana_left=?,time=? WHERE object_id = ?";
// Java: Item.removeFromDb
const DELETE_ITEM: &str = "DELETE FROM items WHERE object_id = ?";
// Java: Item.removeFromDb, Item.removeAugmentation
const DELETE_ITEM_ATTRIBUTES: &str = "DELETE FROM item_attributes WHERE itemId = ?";
// Java: Item.removeFromDb, Item.updateItemElements, Item.clearElementAttr (element -1)
const DELETE_ITEM_ELEMENTALS: &str = "DELETE FROM item_elementals WHERE itemId = ?";
// Java: Item.clearElementAttr (one element)
const DELETE_ITEM_ELEMENTAL: &str = "DELETE FROM item_elementals WHERE itemId = ? AND elemType = ?";
// Java: Item.removeFromDb
const DELETE_ITEM_VARIABLES: &str = "DELETE FROM item_variables WHERE id = ?";
// Java: Item.updateItemElements
const INSERT_ITEM_ELEMENTAL: &str = "INSERT INTO item_elementals VALUES(?,?,?)";
// Java: Item.restoreAttributes
const SELECT_ITEM_ELEMENTALS: &str = "SELECT elemType,elemValue FROM item_elementals WHERE itemId=?";
// Java: Item.restoreAttributes, for every item of an owner at once (the rows `restoreAttributes`
// reads one item at a time, here in the order of the primary key).
const SELECT_AUGMENTATIONS_OF_OWNER: &str = "SELECT a.itemId, a.augAttributes FROM item_attributes a INNER JOIN items i ON i.object_id=a.itemId WHERE i.owner_id=?";
const SELECT_ELEMENTALS_OF_OWNER: &str = "SELECT e.itemId, e.elemType, e.elemValue FROM item_elementals e INNER JOIN items i ON i.object_id=e.itemId WHERE i.owner_id=? ORDER BY e.itemId, e.elemType";
// Java: Item.updateItemAttributes
const REPLACE_ITEM_ATTRIBUTES: &str = "REPLACE INTO item_attributes VALUES(?,?)";
// Java: Item.restoreAttributes
const SELECT_ITEM_AUGMENTATION: &str = "SELECT augAttributes FROM item_attributes WHERE itemId=?";

/// Where an item is kept: the `items.loc` column stores the Java `ItemLocation` name.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemLocation {
    /// Not owned by anyone (dropped, destroyed or being moved).
    Void,
    /// The inventory.
    Inventory,
    /// Equipped.
    Paperdoll,
    /// Private warehouse.
    Warehouse,
    /// Clan warehouse.
    ClanWh,
    /// Pet inventory.
    Pet,
    /// Equipped on a pet.
    PetEquip,
    /// Leased to a merchant.
    Lease,
    /// Refund (sell) list.
    Refund,
    /// Mail attachment.
    Mail,
    /// Freight.
    Freight,
}

impl ItemLocation {
    /// The text the database stores (Java `ItemLocation.name()`).
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Void => "VOID",
            Self::Inventory => "INVENTORY",
            Self::Paperdoll => "PAPERDOLL",
            Self::Warehouse => "WAREHOUSE",
            Self::ClanWh => "CLANWH",
            Self::Pet => "PET",
            Self::PetEquip => "PET_EQUIP",
            Self::Lease => "LEASE",
            Self::Refund => "REFUND",
            Self::Mail => "MAIL",
            Self::Freight => "FREIGHT",
        }
    }

    /// The location of a stored name, or `None` for an unknown name (Java's `valueOf` throws
    /// and the item is skipped).
    pub fn from_db_str(text: &str) -> Option<Self> {
        Some(match text {
            "VOID" => Self::Void,
            "INVENTORY" => Self::Inventory,
            "PAPERDOLL" => Self::Paperdoll,
            "WAREHOUSE" => Self::Warehouse,
            "CLANWH" => Self::ClanWh,
            "PET" => Self::Pet,
            "PET_EQUIP" => Self::PetEquip,
            "LEASE" => Self::Lease,
            "REFUND" => Self::Refund,
            "MAIL" => Self::Mail,
            "FREIGHT" => Self::Freight,
            _ => return None,
        })
    }
}

/// A row of the item select (`Item.restoreFromDb`), one field per selected column.
///
/// `mana_left` and `time` are `DECIMAL` columns; they arrive here as plain integers.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemRow {
    /// `object_id`: the item id.
    pub object_id: i32,
    /// `item_id`: the item template id.
    pub item_id: Option<i32>,
    /// `count`.
    pub count: u64,
    /// `enchant_level`.
    pub enchant_level: Option<i32>,
    /// `loc`: the raw `ItemLocation` name, `None` for NULL (see [`ItemRow::location`]).
    pub loc: Option<String>,
    /// `loc_data`: equipped slot, NPC id or pet id, depending on the location.
    pub loc_data: Option<i32>,
    /// `custom_type1`.
    pub custom_type1: Option<i32>,
    /// `custom_type2`.
    pub custom_type2: Option<i32>,
    /// `mana_left`: shadow weapon mana, -1 for none.
    pub mana_left: i32,
    /// `time`: time of use in milliseconds (`DECIMAL(13)`).
    pub time: i64,
}

impl ItemRow {
    /// The location, or `None` when the column is NULL or holds an unknown name.
    pub fn location(&self) -> Option<ItemLocation> {
        self.loc.as_deref().and_then(ItemLocation::from_db_str)
    }
}

/// Database shape of [`ItemRow`]: the two `DECIMAL` columns need [`SqlNumber`].
#[derive(Debug, Clone, FromRow)]
struct ItemDbRow {
    object_id: i32,
    item_id: Option<i32>,
    count: u64,
    enchant_level: Option<i32>,
    loc: Option<String>,
    loc_data: Option<i32>,
    custom_type1: Option<i32>,
    custom_type2: Option<i32>,
    mana_left: SqlNumber<i32>,
    time: SqlNumber<i64>,
}

impl From<ItemDbRow> for ItemRow {
    fn from(r: ItemDbRow) -> Self {
        Self {
            object_id: r.object_id,
            item_id: r.item_id,
            count: r.count,
            enchant_level: r.enchant_level,
            loc: r.loc,
            loc_data: r.loc_data,
            custom_type1: r.custom_type1,
            custom_type2: r.custom_type2,
            mana_left: r.mana_left.0,
            time: r.time.0,
        }
    }
}

/// A row of the paperdoll select (`PlayerInventory.restoreVisibleInventory`).
#[derive(Debug, Clone, PartialEq, Eq, FromRow)]
pub struct PaperdollItemRow {
    /// `object_id`.
    pub object_id: i32,
    /// `item_id`.
    pub item_id: Option<i32>,
    /// `loc_data`: the paperdoll slot.
    pub loc_data: Option<i32>,
    /// `enchant_level`.
    pub enchant_level: Option<i32>,
}

/// A row of `item_elementals` (Java `Elementals`): the element id (`Elementals.FIRE` 0 to
/// `DARK` 5) and its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct ItemElementalRow {
    /// `elemType`.
    #[sqlx(rename = "elemType")]
    pub elem_type: i8,
    /// `elemValue`.
    #[sqlx(rename = "elemValue")]
    pub elem_value: i32,
}

/// A row of `item_elementals` with the item it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct OwnerElementalRow {
    /// `itemId`.
    #[sqlx(rename = "itemId")]
    pub item_id: i32,
    /// `elemType`.
    #[sqlx(rename = "elemType")]
    pub elem_type: i8,
    /// `elemValue`.
    #[sqlx(rename = "elemValue")]
    pub elem_value: i32,
}

/// A row of `item_attributes` with the item it belongs to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, FromRow)]
pub struct OwnerAugmentationRow {
    /// `itemId`.
    #[sqlx(rename = "itemId")]
    pub item_id: i32,
    /// `augAttributes`: the augmentation id, -1 for "none" (Java skips such a row).
    #[sqlx(rename = "augAttributes")]
    pub aug_attributes: i32,
}

/// The values of a new item, in the order of `INSERT_ITEM`'s columns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewItem {
    /// `owner_id`: character or clan id.
    pub owner_id: i32,
    /// `item_id`.
    pub item_id: i32,
    /// `count`.
    pub count: i64,
    /// `loc`.
    pub loc: ItemLocation,
    /// `loc_data`.
    pub loc_data: i32,
    /// `enchant_level`.
    pub enchant_level: i32,
    /// `object_id`.
    pub object_id: i32,
    /// `custom_type1`.
    pub custom_type1: i32,
    /// `custom_type2`.
    pub custom_type2: i32,
    /// `mana_left`.
    pub mana_left: i32,
    /// `time`.
    pub time: i64,
}

/// The values an item writes on update, in the order of `UPDATE_ITEM`'s `SET` list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemUpdate {
    /// `owner_id`.
    pub owner_id: i32,
    /// `count`.
    pub count: i64,
    /// `loc`.
    pub loc: ItemLocation,
    /// `loc_data`.
    pub loc_data: i32,
    /// `enchant_level`.
    pub enchant_level: i32,
    /// `custom_type1`.
    pub custom_type1: i32,
    /// `custom_type2`.
    pub custom_type2: i32,
    /// `mana_left`.
    pub mana_left: i32,
    /// `time`.
    pub time: i64,
}

/// Every query on `items`, `item_attributes` and `item_elementals` the game server needs.
#[derive(Debug, Clone)]
pub struct ItemRepository {
    pool: Pool,
}

impl ItemRepository {
    /// Wraps the pool (cheap to clone).
    pub fn new(pool: Pool) -> Self {
        Self { pool }
    }

    /// The items of one location of the owner (warehouses, pets, freight, ...).
    // Java: ItemContainer.restore
    pub async fn items_of_location(&self, owner_id: i32, location: ItemLocation) -> Result<Vec<ItemRow>> {
        let rows = sqlx::query_as::<_, ItemDbRow>(SELECT_ITEMS_OF_LOCATION)
            .bind(owner_id)
            .bind(location.as_db_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(ItemRow::from).collect())
    }

    /// The owner's inventory items: the `base` location and the `equip` location together,
    /// ordered by `loc_data` (the Java inventory passes INVENTORY and PAPERDOLL).
    // Java: Inventory.restore
    pub async fn inventory_items(
        &self,
        owner_id: i32,
        base: ItemLocation,
        equip: ItemLocation,
    ) -> Result<Vec<ItemRow>> {
        let rows = sqlx::query_as::<_, ItemDbRow>(SELECT_ITEMS_OF_INVENTORY)
            .bind(owner_id)
            .bind(base.as_db_str())
            .bind(equip.as_db_str())
            .fetch_all(&self.pool)
            .await?;
        Ok(rows.into_iter().map(ItemRow::from).collect())
    }

    /// The equipped items of the owner, for the character selection screen.
    // Java: PlayerInventory.restoreVisibleInventory
    pub async fn paperdoll_items(&self, owner_id: i32) -> Result<Vec<PaperdollItemRow>> {
        let rows = sqlx::query_as::<_, PaperdollItemRow>(SELECT_PAPERDOLL_ITEMS)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Inserts a new item row. Fails on a duplicate `object_id` (primary key).
    // Java: Item.insertIntoDb
    pub async fn insert_item(&self, item: &NewItem) -> Result<()> {
        sqlx::query(INSERT_ITEM)
            .bind(item.owner_id)
            .bind(item.item_id)
            .bind(item.count)
            .bind(item.loc.as_db_str())
            .bind(item.loc_data)
            .bind(item.enchant_level)
            .bind(item.object_id)
            .bind(item.custom_type1)
            .bind(item.custom_type2)
            .bind(item.mana_left)
            .bind(item.time)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Writes the item's current values.
    // Java: Item.updateInDb
    pub async fn update_item(&self, object_id: i32, item: &ItemUpdate) -> Result<()> {
        sqlx::query(UPDATE_ITEM)
            .bind(item.owner_id)
            .bind(item.count)
            .bind(item.loc.as_db_str())
            .bind(item.loc_data)
            .bind(item.enchant_level)
            .bind(item.custom_type1)
            .bind(item.custom_type2)
            .bind(item.mana_left)
            .bind(item.time)
            .bind(object_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Deletes the item and its augmentation, element and variable rows, in Java's order.
    // Java: Item.removeFromDb
    pub async fn delete_item(&self, object_id: i32) -> Result<()> {
        // Java runs all four statements on one connection.
        let mut conn = self.pool.acquire().await?;
        sqlx::query(DELETE_ITEM)
            .bind(object_id)
            .execute(&mut *conn)
            .await?;
        sqlx::query(DELETE_ITEM_ATTRIBUTES)
            .bind(object_id)
            .execute(&mut *conn)
            .await?;
        sqlx::query(DELETE_ITEM_ELEMENTALS)
            .bind(object_id)
            .execute(&mut *conn)
            .await?;
        sqlx::query(DELETE_ITEM_VARIABLES)
            .bind(object_id)
            .execute(&mut *conn)
            .await?;
        Ok(())
    }

    /// Sets the augmentation of the item (`REPLACE`, so it also covers a second augment).
    // Java: Item.updateItemAttributes
    pub async fn set_augmentation(&self, object_id: i32, augmentation_id: i32) -> Result<()> {
        sqlx::query(REPLACE_ITEM_ATTRIBUTES)
            .bind(object_id)
            .bind(augmentation_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The stored augmentation id of the item, `None` when there is no row. Java treats -1 as
    /// "no augmentation"; the caller checks that.
    // Java: Item.restoreAttributes
    pub async fn augmentation_of(&self, object_id: i32) -> Result<Option<i32>> {
        let aug = sqlx::query_scalar::<_, i32>(SELECT_ITEM_AUGMENTATION)
            .bind(object_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(aug)
    }

    /// The element rows of the item in primary key order (`elemType` ascending), including the
    /// rows Java skips (`elemType` -1 or `elemValue` -1; the caller checks).
    // Java: Item.restoreAttributes
    pub async fn elementals_of(&self, object_id: i32) -> Result<Vec<ItemElementalRow>> {
        let rows = sqlx::query_as::<_, ItemElementalRow>(SELECT_ITEM_ELEMENTALS)
            .bind(object_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// The augmentation rows of every item of the owner, whatever its location.
    // Java: Item.restoreAttributes (all items of an owner in one query)
    pub async fn augmentations_of_owner(&self, owner_id: i32) -> Result<Vec<OwnerAugmentationRow>> {
        let rows = sqlx::query_as::<_, OwnerAugmentationRow>(SELECT_AUGMENTATIONS_OF_OWNER)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// The element rows of every item of the owner, whatever its location, ordered by item and
    /// element.
    // Java: Item.restoreAttributes (all items of an owner in one query)
    pub async fn elementals_of_owner(&self, owner_id: i32) -> Result<Vec<OwnerElementalRow>> {
        let rows = sqlx::query_as::<_, OwnerElementalRow>(SELECT_ELEMENTALS_OF_OWNER)
            .bind(owner_id)
            .fetch_all(&self.pool)
            .await?;
        Ok(rows)
    }

    /// Writes the item's elements: all its rows are deleted, then one row is inserted per
    /// element (an empty list only deletes).
    // Java: Item.updateItemElements
    pub async fn replace_elementals(&self, object_id: i32, elementals: &[ItemElementalRow]) -> Result<()> {
        let mut conn = self.pool.acquire().await?;
        sqlx::query(DELETE_ITEM_ELEMENTALS)
            .bind(object_id)
            .execute(&mut *conn)
            .await?;
        for e in elementals {
            sqlx::query(INSERT_ITEM_ELEMENTAL)
                .bind(object_id)
                .bind(e.elem_type)
                .bind(e.elem_value)
                .execute(&mut *conn)
                .await?;
        }
        Ok(())
    }

    /// Removes one element of the item, or all of them for `elem_type` -1.
    // Java: Item.clearElementAttr
    pub async fn delete_elemental(&self, object_id: i32, elem_type: i8) -> Result<()> {
        if elem_type == -1 {
            sqlx::query(DELETE_ITEM_ELEMENTALS)
                .bind(object_id)
                .execute(&self.pool)
                .await?;
        } else {
            sqlx::query(DELETE_ITEM_ELEMENTAL)
                .bind(object_id)
                .bind(elem_type)
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    /// Removes the augmentation row of the item.
    // Java: Item.removeAugmentation
    pub async fn delete_augmentation(&self, object_id: i32) -> Result<()> {
        sqlx::query(DELETE_ITEM_ATTRIBUTES)
            .bind(object_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn placeholders(sql: &str) -> usize {
        sql.matches('?').count()
    }

    #[test]
    fn statements_have_one_placeholder_per_bind() {
        assert_eq!(placeholders(SELECT_ITEMS_OF_LOCATION), 2);
        assert_eq!(placeholders(SELECT_ITEMS_OF_INVENTORY), 3);
        assert_eq!(placeholders(INSERT_ITEM), 11);
        assert_eq!(placeholders(UPDATE_ITEM), 10);
        assert_eq!(placeholders(DELETE_ITEM), 1);
        assert_eq!(placeholders(REPLACE_ITEM_ATTRIBUTES), 2);
        assert_eq!(placeholders(DELETE_ITEM_ELEMENTALS), 1);
        assert_eq!(placeholders(DELETE_ITEM_ELEMENTAL), 2);
        assert_eq!(placeholders(DELETE_ITEM_VARIABLES), 1);
        assert_eq!(placeholders(INSERT_ITEM_ELEMENTAL), 3);
        assert_eq!(placeholders(SELECT_ITEM_ELEMENTALS), 1);
        assert_eq!(placeholders(SELECT_AUGMENTATIONS_OF_OWNER), 1);
        assert_eq!(placeholders(SELECT_ELEMENTALS_OF_OWNER), 1);
    }

    #[test]
    fn location_names_round_trip() {
        for loc in [
            ItemLocation::Void,
            ItemLocation::Inventory,
            ItemLocation::Paperdoll,
            ItemLocation::Warehouse,
            ItemLocation::ClanWh,
            ItemLocation::Pet,
            ItemLocation::PetEquip,
            ItemLocation::Lease,
            ItemLocation::Refund,
            ItemLocation::Mail,
            ItemLocation::Freight,
        ] {
            assert_eq!(ItemLocation::from_db_str(loc.as_db_str()), Some(loc));
        }
        assert_eq!(ItemLocation::from_db_str("inventory"), None);
    }
}
