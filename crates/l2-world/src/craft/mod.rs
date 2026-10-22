//! Crafting: the recipe books, making an item, and the manufacture (dwarven workshop) shop
//! (M6-CRAFT-01).
// Java: managers/RecipeManager.java, data/xml/RecipeData.java
// Java: network/clientpackets/{RequestRecipeBookOpen,RequestRecipeBookDestroy,
//       RequestRecipeItemMakeInfo,RequestRecipeItemMakeSelf,RequestRecipeShopMessageSet,
//       RequestRecipeShopListSet,RequestRecipeShopManageQuit,RequestRecipeShopMakeInfo,
//       RequestRecipeShopMakeItem,RequestRecipeShopManagePrev}.java
// Java: network/serverpackets/{RecipeBookItemList,RecipeItemMakeInfo,RecipeShopItemInfo,
//       RecipeShopManageList,RecipeShopMsg,RecipeShopSellList}.java
// Java: entity/actor/Player.java (the recipe books, the manufacture items, isCrafting,
//       restoreRecipeBook, storeRecipeShopList, restoreRecipeShopList)
// Java: dist/game/data/scripts/handlers/items/Recipes.java
// Java: dist/game/data/scripts/handlers/skill/effects/{OpenDwarfRecipeBook,OpenCommonRecipeBook}.java
//
// # Where things are
// - `book.rs`: the two recipe books of a player, `character_recipebook` rows, the book window
//   packets and the `Recipes` item handler.
// - `shop.rs`: the manufacture shop (`RequestRecipeShop*`, the manage and sell lists, the titles,
//   `character_recipeshoplist`).
// - `maker.rs`: `RecipeItemMaker`, the craft itself (MP and HP, materials, chance, rare
//   products, the payment of a customer and the optional "alt" crafting with its passes).
//
// # Model
// The recipes are `l2_data::RecipeData` (immutable, handed in at startup like the cubics). A
// player's books hold recipe list ids; Java keeps two `ConcurrentHashMap<Integer, RecipeList>`,
// whose iteration order decides the order of the book window, so the books keep the insertion
// order and the packets apply the same bucket order (`java_concurrent_map_order`, the rule the
// skill list uses). The manufacture items are a `LinkedHashMap`: insertion order.
//
// `RecipeManager._activeMakers` (crafter object id to the maker being run) is [`CraftWorld`];
// the maker of the "alt" crafting waits on the timer wheel between its passes. Without
// `AltGameCreation` (the default) a craft is one synchronous call and nothing is kept.
//
// # Rows (D-39)
// A recipe joining or leaving a book is written at once, like Java (`PersistCommand::Recipe`).
// The manufacture list is written with the character's store, with Java's statements, and only
// with `StoreRecipeShopList` (`PlayerProfile::recipe_shop_to_store`).
//
// # Left out on purpose (each has a comment where it belongs)
// - The flood protectors `canManufacture` / `canPerformTransaction`.
// - `isSellingBuffs` (`SellBuffsManager`), `isMounted()` (no mounts yet), the olympiad.
// - Offline craft (`OfflineCraftEnable`).
// - `ItemProcessType` logging (`LogItems`).
// - The subclass change that re-reads the dwarven book (`restoreRecipeBook(false)`): there is no
//   subclass switching yet.

mod book;
mod maker;
mod shop;

use std::collections::HashMap;
use std::sync::Arc;

use l2_data::RecipeData;
use l2_protocol_game::client::{
    RequestRecipeBookDestroy, RequestRecipeBookOpen, RequestRecipeItemMakeInfo, RequestRecipeItemMakeSelf,
    RequestRecipeShopListSet, RequestRecipeShopMakeInfo, RequestRecipeShopMakeItem,
    RequestRecipeShopMessageSet,
};
use l2_util::ObjectId;

pub(crate) use maker::{CraftTimer, ItemMaker};

use crate::world::World;

/// Java `CommonSkill.CREATE_DWARVEN`.
pub const CREATE_DWARVEN: i32 = 172;
/// Java `CommonSkill.CREATE_COMMON`.
pub const CREATE_COMMON: i32 = 1320;

/// The values of `Player.ini` that crafting reads. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct CraftSettings {
    /// `PlayerConfig.IS_CRAFTING_ENABLED` (`CraftingEnabled`).
    pub is_crafting_enabled: bool,
    /// `PlayerConfig.CRAFTING_IN_COMBAT` (`CraftingInCombat`).
    pub crafting_in_combat: bool,
    /// `PlayerConfig.CRAFT_MASTERWORK` (`CraftMasterwork`).
    pub craft_masterwork: bool,
    /// `PlayerConfig.CRAFT_MASTERWORK_CHANCE_RATE` (`CraftMasterworkChanceRate`).
    pub craft_masterwork_chance_rate: f64,
    /// `PlayerConfig.ALT_GAME_CREATION` (`AltGameCreation`).
    pub alt_game_creation: bool,
    /// `PlayerConfig.ALT_GAME_CREATION_SPEED` (`AltGameCreationSpeed`).
    pub alt_game_creation_speed: f64,
    /// `PlayerConfig.ALT_GAME_CREATION_XP_RATE` (`AltGameCreationXpRate`).
    pub alt_game_creation_xp_rate: f64,
    /// `PlayerConfig.ALT_GAME_CREATION_SP_RATE` (`AltGameCreationSpRate`).
    pub alt_game_creation_sp_rate: f64,
    /// `PlayerConfig.ALT_GAME_CREATION_RARE_XPSP_RATE` (`AltGameCreationRareXpSpRate`).
    pub alt_game_creation_rare_xpsp_rate: f64,
    /// `PlayerConfig.STORE_RECIPE_SHOPLIST` (`StoreRecipeShopList`).
    pub store_recipe_shoplist: bool,
}

impl Default for CraftSettings {
    fn default() -> Self {
        Self {
            is_crafting_enabled: true,
            crafting_in_combat: false,
            craft_masterwork: true,
            craft_masterwork_chance_rate: 1.0,
            alt_game_creation: false,
            alt_game_creation_speed: 1.0,
            alt_game_creation_xp_rate: 1.0,
            alt_game_creation_sp_rate: 1.0,
            alt_game_creation_rare_xpsp_rate: 2.0,
            store_recipe_shoplist: false,
        }
    }
}

/// One row of `character_recipebook` as the session read it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RecipeBookEntry {
    /// `id`: the recipe list id.
    pub recipe_id: i32,
    /// `type == 1`: a dwarven recipe.
    pub dwarven: bool,
    /// `classIndex`.
    pub class_index: i32,
}

/// What the session read for crafting before the character entered the world
/// (`PlayerProfile::recipes`): the rows of `character_recipebook` in the database's order and,
/// with `StoreRecipeShopList`, the rows of `character_recipeshoplist` in `index` order. The
/// world resolves them against the recipe data when the player is created, so snapshots never
/// carry them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RecipeLoad {
    pub book: Vec<RecipeBookEntry>,
    /// `(recipeId, price)`.
    pub shop: Vec<(i32, i64)>,
}

/// A recipe for sale in a manufacture shop (Java `ManufactureItem`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManufactureItem {
    pub recipe_id: i32,
    pub cost: i64,
}

/// The crafting state of a player (Java `_dwarvenRecipeBook`, `_commonRecipeBook`,
/// `_manufactureItems`, `_storeName`, `_isCrafting`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerCraft {
    /// The dwarven book in insertion order (see the module notes for the packet order).
    pub(crate) dwarven: Vec<i32>,
    pub(crate) common: Vec<i32>,
    /// Java `_manufactureItems` (a `LinkedHashMap`): insertion order, one entry per recipe.
    pub(crate) manufacture: Vec<ManufactureItem>,
    /// Java `_storeName`.
    pub(crate) store_name: String,
    /// Java `_isCrafting`.
    pub(crate) is_crafting: bool,
    /// The rows read at the character select, until the world resolves them.
    pub(crate) pending: Option<RecipeLoad>,
}

impl PlayerCraft {
    /// The state of a player that has just been read from the database.
    pub fn from_load(load: RecipeLoad) -> Self {
        Self {
            pending: Some(load),
            ..Self::default()
        }
    }
}

/// A write of the recipe book rows (Java `insertNewRecipeData`, `deleteRecipeData`), see
/// [`PersistCommand::Recipe`](crate::PersistCommand::Recipe).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecipeWrite {
    /// `INSERT INTO character_recipebook (charId, id, classIndex, type)`.
    Insert {
        char_id: i32,
        recipe_id: i32,
        class_index: i32,
        dwarven: bool,
    },
    /// `DELETE FROM character_recipebook WHERE charId=? AND id=? AND classIndex=?`.
    Delete {
        char_id: i32,
        recipe_id: i32,
        class_index: i32,
    },
}

/// The crafting packets as the world gets them (`ClientInput::Craft`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CraftInput {
    BookOpen(RequestRecipeBookOpen),
    BookDestroy(RequestRecipeBookDestroy),
    ItemMakeInfo(RequestRecipeItemMakeInfo),
    ItemMakeSelf(RequestRecipeItemMakeSelf),
    ShopMessageSet(RequestRecipeShopMessageSet),
    ShopListSet(RequestRecipeShopListSet),
    ShopManageQuit,
    ShopMakeInfo(RequestRecipeShopMakeInfo),
    ShopMakeItem(RequestRecipeShopMakeItem),
    ShopManagePrev,
}

/// The crafting state of the world: the recipes and `RecipeManager._activeMakers`.
#[derive(Debug, Default)]
pub struct CraftWorld {
    pub(crate) data: Option<Arc<RecipeData>>,
    /// The makers waiting on the timer wheel, by serial number.
    pub(crate) makers: HashMap<u64, ItemMaker>,
    /// Java `_activeMakers`: crafter to the serial of its maker.
    pub(crate) active: HashMap<ObjectId, u64>,
    pub(crate) next_serial: u64,
}

impl World {
    /// Installs `data/Recipes.xml`. Call before players enter.
    pub fn set_recipe_data(&mut self, data: Arc<RecipeData>) {
        self.craft.data = Some(data);
    }

    /// The recipes, shared so a function can read them while it changes the world.
    pub(crate) fn recipe_data(&self) -> Option<Arc<RecipeData>> {
        self.craft.data.clone()
    }

    /// Routes one crafting packet.
    pub(crate) fn on_craft_input(&mut self, id: ObjectId, input: CraftInput) {
        if !self.players.contains_key(&id) {
            return;
        }
        match input {
            CraftInput::BookOpen(p) => self.on_request_recipe_book_open(id, p.recipe_type),
            CraftInput::BookDestroy(p) => self.on_request_recipe_book_destroy(id, p.recipe_id),
            CraftInput::ItemMakeInfo(p) => self.on_request_recipe_item_make_info(id, p.id),
            CraftInput::ItemMakeSelf(p) => self.on_request_recipe_item_make_self(id, p.id),
            CraftInput::ShopMessageSet(p) => self.on_request_recipe_shop_message_set(id, &p.name),
            CraftInput::ShopListSet(p) => self.on_request_recipe_shop_list_set(id, p.items),
            CraftInput::ShopManageQuit => self.on_request_recipe_shop_manage_quit(id),
            CraftInput::ShopMakeInfo(p) => {
                self.on_request_recipe_shop_make_info(id, p.player_object_id, p.recipe_id);
            }
            CraftInput::ShopMakeItem(p) => self.on_request_recipe_shop_make_item(id, p.id, p.recipe_id),
            CraftInput::ShopManagePrev => self.on_request_recipe_shop_manage_prev(id),
        }
    }

    /// Java `Player.isCrafting()`.
    pub fn is_crafting(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.craft.is_crafting)
    }

    pub(crate) fn set_crafting(&mut self, id: ObjectId, crafting: bool) {
        if let Some(p) = self.players.get_mut(&id) {
            p.craft.is_crafting = crafting;
        }
    }

    /// The recipe shop of the snapshot of a store: Java `storeRecipeShopList`, which writes only
    /// when `StoreRecipeShopList` is on and the player has a shop.
    pub(crate) fn recipe_shop_to_store(&self, id: ObjectId) -> Option<Vec<(i32, i64)>> {
        if !self.settings.craft.store_recipe_shoplist {
            return None;
        }
        let craft = &self.players.get(&id)?.craft;
        // NOTE(java-quirk): `if (hasManufactureShop())` guards the whole method, so a shop that
        // was emptied never deletes its old rows.
        if craft.manufacture.is_empty() {
            return None;
        }
        Some(craft.manufacture.iter().map(|m| (m.recipe_id, m.cost)).collect())
    }

    /// Java `Player.deleteMe`: `RecipeManager.requestMakeItemAbort(this)`.
    pub(crate) fn craft_on_player_leave(&mut self, id: ObjectId) {
        self.craft.active.remove(&id);
    }
}
