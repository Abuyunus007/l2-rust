//! Items on the ground: `Item.dropMe` / `pickupMe` / `decayMe`, the drop protection and loot owner
//! timers, `ItemsAutoDestroyTaskManager` and `ItemsOnGroundManager` (M4-DROP-01).
// Java: entity/item/instance/Item.java (dropMe, pickupMe, sendInfo, decayMe, setOwnerId,
//       setProtected, getDropTime)
// Java: entity/item/holders/DropProtection.java
// Java: managers/ItemManager.java (createItem's loot privilege, ResetOwner)
// Java: taskmanagers/ItemsAutoDestroyTaskManager.java
// Java: managers/ItemsOnGroundManager.java
// Java: entity/actor/Npc.java (dropItem)
//
// # Model
// A ground item is a world object of kind [`ObjectKind::Item`](crate::ObjectKind): its row in
// [`GroundWorld::items`] holds what Java keeps in the `Item` while it lies there (the item row as
// it will be stored, the position, the drop time, the loot owner, the drop protection). It has an
// entry in `World::objects` like every other object, so it lives in a region, is seen by the
// players around and is found by `Action` (D-11: ids, no references). An item in an inventory is
// not a world object here: picking it up removes it from this table (`pickup_me`) and the owner's
// item list gets the row.
//
// # Clocks
// Java reads `System.currentTimeMillis()` for the drop time. Here every time is the world clock
// (`World::world_ms`, tick * 50), so the auto destroy and the timers follow the tick and tests can
// run them. The `itemsonground.drop_time` column holds real unix milliseconds; the two are
// converted when rows are loaded and saved.
//
// # What is left out, with the system that brings it
// - Merc tickets (`MercTicketManager`), cursed weapons and combat flags (sieges, M5).
// - `ON_PLAYER_ITEM_DROP` / `ON_PLAYER_ITEM_PICKUP` script events (D-05).

mod pickup;

use std::collections::{BTreeSet, HashMap};

use l2_data::item::ItemTemplate;
use l2_geo::Location;
use l2_protocol_game::server::{DropItem, SpawnItem};
use l2_util::ObjectId;
use tracing::{info, warn};

use crate::objects::ObjectKind;
use crate::outbox::PersistCommand;
use crate::player::ItemData;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// `ItemLocation.VOID` as the `items.loc` column spells it: the location of an item on the ground.
pub const LOC_VOID: &str = "VOID";

/// `Npc.RANDOM_ITEM_DROP_LIMIT`: a monster scatters its drops this far around itself.
pub const RANDOM_ITEM_DROP_LIMIT: i32 = 70;
/// `ItemsAutoDestroyTaskManager` runs every 5 seconds.
const AUTO_DESTROY_INTERVAL_MS: u64 = 5000;
/// `DropProtection.PROTECTED_MILLIS_TIME`.
const DROP_PROTECTION_MS: u64 = 15_000;
/// `ItemManager.createItem` resets the loot owner after 15 seconds.
const LOOT_OWNER_MS: u64 = 15_000;

/// The values of `General.ini` and `Player.ini` that ground items and their pickup read.
/// `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundItemSettings {
    /// `GeneralConfig.AUTODESTROY_ITEM_AFTER` (`AutoDestroyDroppedItemAfter`), seconds.
    pub autodestroy_item_after: i32,
    /// `GeneralConfig.HERB_AUTO_DESTROY_TIME` (`AutoDestroyHerbTime` * 1000), milliseconds.
    pub herb_auto_destroy_time: i32,
    /// `GeneralConfig.LIST_PROTECTED_ITEMS` (`ListOfProtectedItems`).
    pub list_protected_items: Vec<i32>,
    /// `GeneralConfig.DESTROY_DROPPED_PLAYER_ITEM` (`DestroyPlayerDroppedItem`).
    pub destroy_dropped_player_item: bool,
    /// `GeneralConfig.DESTROY_EQUIPABLE_PLAYER_ITEM` (`DestroyEquipableItem`).
    pub destroy_equipable_player_item: bool,
    /// `GeneralConfig.SAVE_DROPPED_ITEM` (`SaveDroppedItem`).
    pub save_dropped_item: bool,
    /// `GeneralConfig.SAVE_DROPPED_ITEM_INTERVAL` (`SaveDroppedItemInterval` * 60000),
    /// milliseconds; 0 or less means "only at shutdown".
    pub save_dropped_item_interval_ms: i64,
    /// `GeneralConfig.MULTIPLE_ITEM_DROP` (`MultipleItemDrop`).
    pub multiple_item_drop: bool,
    /// `PlayerConfig.AUTO_LOOT` (`AutoLoot`).
    pub auto_loot: bool,
    /// `PlayerConfig.AUTO_LOOT_RAIDS` (`AutoLootRaids`).
    pub auto_loot_raids: bool,
    /// `PlayerConfig.AUTO_LOOT_HERBS` (`AutoLootHerbs`).
    pub auto_loot_herbs: bool,
    /// `PlayerConfig.AUTO_LOOT_ITEM_IDS` (`AutoLootItemIds`; 0 and unparsable entries are left out).
    pub auto_loot_item_ids: Vec<i32>,
    /// `PlayerConfig.MAX_ADENA` (`MaxAdena`, after Java's "negative means no limit").
    pub max_adena: i64,
}

impl Default for GroundItemSettings {
    fn default() -> Self {
        Self {
            autodestroy_item_after: 600,
            herb_auto_destroy_time: 60_000,
            list_protected_items: vec![0],
            destroy_dropped_player_item: false,
            destroy_equipable_player_item: false,
            save_dropped_item: false,
            save_dropped_item_interval_ms: 60 * 60_000,
            multiple_item_drop: true,
            auto_loot: false,
            auto_loot_raids: false,
            auto_loot_herbs: false,
            auto_loot_item_ids: Vec::new(),
            max_adena: 99_900_000_000,
        }
    }
}

impl GroundItemSettings {
    /// `LIST_PROTECTED_ITEMS.contains(item_id)`.
    pub fn is_protected_item(&self, item_id: i32) -> bool {
        self.list_protected_items.contains(&item_id)
    }
}

/// Java `DropProtection` while it protects: who may pick the item up, and the timer that ends it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DropProtection {
    pub owner: ObjectId,
    timer: TimerId,
}

/// An `Item` lying on the ground.
#[derive(Debug, Clone, PartialEq)]
pub struct GroundItem {
    /// The item as it will be stored: object id, item id, count, enchant, custom types, mana and
    /// time. `loc` is `VOID` and `loc_data` 0.
    pub data: ItemData,
    /// Java `getX()`, `getY()`, `getZ()`.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Java `_dropTime` on the world clock (`World::world_ms`); `None` is Java's 0, "never dropped"
    /// (the world clock itself starts at 0, so 0 cannot be the marker here).
    pub drop_time_ms: Option<i64>,
    /// Java `_protected`: written as `drop_time -1` and never auto destroyed by the manager's
    /// player-item rules.
    pub protected: bool,
    /// Java `_ownerId` while on the ground: the player who has the loot privilege.
    pub owner: Option<ObjectId>,
    /// Java `_itemLootShedule`: the pending `ResetOwner`.
    owner_reset: Option<TimerId>,
    /// Java `getDropProtection()`.
    pub drop_protection: Option<DropProtection>,
    /// Java `_dropperObjectId`: set only while `dropMe` shows the item (it picks `DropItem` over
    /// `SpawnItem`).
    pub dropper: Option<ObjectId>,
}

impl GroundItem {
    /// A fresh item that is not in the world yet (Java `new Item(objectId, itemId)` with the count
    /// `ItemManager.createItem` gives it).
    pub fn new(data: ItemData) -> Self {
        Self {
            data,
            x: 0,
            y: 0,
            z: 0,
            drop_time_ms: None,
            protected: false,
            owner: None,
            owner_reset: None,
            drop_protection: None,
            dropper: None,
        }
    }

    /// Java `getObjectId()`.
    pub fn object_id(&self) -> ObjectId {
        self.data.object_id
    }
}

/// One row of `itemsonground`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GroundItemRow {
    pub object_id: ObjectId,
    pub item_id: i32,
    pub count: i64,
    pub enchant_level: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// Unix milliseconds, or -1 for a protected item.
    pub drop_time: i64,
    pub equipable: bool,
}

/// Everything about ground items that is not a creature or a region: the table itself, the auto
/// destroy set and the manager's timers.
#[derive(Debug, Default)]
pub struct GroundWorld {
    pub(crate) items: HashMap<ObjectId, GroundItem>,
    /// Java `ItemsAutoDestroyTaskManager.ITEMS`, in id order (Java's set has no order; this keeps
    /// the destroy order, and so the packet order, stable).
    pub(crate) auto_destroy: BTreeSet<ObjectId>,
    pub(crate) auto_destroy_timer: Option<TimerId>,
    pub(crate) save_timer: Option<TimerId>,
}

impl GroundWorld {
    /// How many items lie on the ground.
    pub fn len(&self) -> usize {
        self.items.len()
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }
}

impl World {
    // ------------------------------------------------------------------ queries

    /// A ground item by object id.
    pub fn ground_item(&self, id: ObjectId) -> Option<&GroundItem> {
        self.ground.items.get(&id)
    }

    pub(crate) fn ground_item_mut(&mut self, id: ObjectId) -> Option<&mut GroundItem> {
        self.ground.items.get_mut(&id)
    }

    /// The object ids of all ground items, ascending.
    pub fn ground_item_ids(&self) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = self.ground.items.keys().copied().collect();
        ids.sort_unstable_by_key(|id| id.0);
        ids
    }

    /// Whether the auto destroy manager has the item on its list.
    pub fn is_in_auto_destroy(&self, id: ObjectId) -> bool {
        self.ground.auto_destroy.contains(&id)
    }

    // ------------------------------------------------------------------ item rows

    /// Java `new Item(objectId, itemId)` + `setCount`, as `ItemManager.createItem` makes it: the
    /// mana and time come from the template.
    pub(crate) fn new_item_data(&self, template: &ItemTemplate, object_id: ObjectId, count: i64) -> ItemData {
        ItemData {
            object_id,
            item_id: template.id,
            // `if (item.isStackable() && (count > 1)) item.setCount(count)`.
            count: if template.stackable && count > 1 { count } else { 1 },
            enchant_level: 0,
            loc: LOC_VOID.to_string(),
            loc_data: 0,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: template.duration,
            time: if template.time == -1 {
                -1
            } else {
                unix_ms_now().wrapping_add(template.time.wrapping_mul(60 * 1000))
            },
            ..Default::default()
        }
    }

    /// Takes an object id from the id factory (D-39: the Rust range). `None` without a factory or
    /// when the range is used up.
    pub(crate) fn allocate_item_id(&self) -> Option<ObjectId> {
        let Some(ids) = self.npc_world.ids.as_ref() else {
            tracing::debug!("No id factory installed: a new item cannot be created.");
            return None;
        };
        match ids.allocate() {
            Ok(id) => Some(id),
            Err(e) => {
                warn!("No object id for a new item: {e}");
                None
            }
        }
    }

    /// Gives an item's object id back (Java `IdManager.releaseId`).
    pub(crate) fn release_item_id(&self, id: ObjectId) {
        if let Some(ids) = self.npc_world.ids.as_ref() {
            ids.release(id);
        }
    }

    // ------------------------------------------------------------------ dropMe

    /// Java `Item.dropMe(dropper, x, y, z)`: the item appears at the position (moved by the
    /// geodata when somebody drops it), everybody around gets `DropItem` (or `SpawnItem` for a
    /// dropper-less item), and the managers learn about it. Returns `false` when the position is
    /// outside the world and the item was thrown away.
    pub(crate) fn drop_me(
        &mut self,
        mut item: GroundItem,
        dropper: Option<ObjectId>,
        x: i32,
        y: i32,
        z: i32,
    ) -> bool {
        let id = item.object_id();
        let (mut x, mut y, mut z) = (x, y, z);
        let mut instance = 0;
        if let Some(dropper) = dropper {
            instance = self.objects.get(&dropper).map_or(0, |e| e.instance_id);
            // `GeoEngine.getValidLocation(dropper, x, y, z)`.
            if let (Some(geo), Some((dx, dy, dz))) = (self.geo.as_ref(), self.position_of(dropper)) {
                let valid = geo.valid_location_with(
                    &self.obstacles(),
                    Location::new(dx, dy, dz),
                    Location::new(x, y, z),
                    instance,
                );
                (x, y, z) = (valid.x, valid.y, valid.z);
            }
        }
        (item.x, item.y, item.z) = (x, y, z);
        item.drop_time_ms = Some(self.world_ms());
        // The dropper id picks the `DropItem` packet in `sendInfo`; it is cleared again below.
        item.dropper = dropper;
        self.ground.items.insert(id, item);
        self.register_object(id, ObjectKind::Item);
        if let Some(entry) = self.objects.get_mut(&id) {
            entry.instance_id = instance;
        }
        // Add the Item dropped in the world as a visible object (region, then everybody around).
        self.spawn(id);
        match self.ground.items.get_mut(&id) {
            // Set the dropper Id back to 0 so it no longer shows the drop packet.
            Some(item) => item.dropper = None,
            None => return false,
        }
        true
    }

    /// Java `Item.sendInfo(player)`: `DropItem` while `dropMe` runs, `SpawnItem` for everybody
    /// who comes into view later.
    pub(crate) fn send_ground_item_info(&mut self, subject: ObjectId, viewer: ObjectId) {
        let Some(item) = self.ground.items.get(&subject) else {
            return;
        };
        let display_id = self
            .data
            .item(item.data.item_id)
            .map_or(item.data.item_id, |t| t.display_id);
        let stackable = self.data.item_stackable(item.data.item_id);
        match item.dropper {
            Some(dropper) => {
                let packet = DropItem {
                    player_id: dropper.0,
                    object_id: subject.0,
                    display_id,
                    x: item.x,
                    y: item.y,
                    z: item.z,
                    stackable,
                    count: item.data.count,
                };
                self.send(viewer, &packet);
            }
            None => {
                let packet = SpawnItem {
                    object_id: subject.0,
                    display_id,
                    x: item.x,
                    y: item.y,
                    z: item.z,
                    stackable,
                    count: item.data.count,
                };
                self.send(viewer, &packet);
            }
        }
    }

    // ------------------------------------------------------------------ pickupMe / decayMe

    /// Java `Item.pickupMe(creature)` without the `GetItem` broadcast (the caller sends it, it
    /// knows who sees the picker): the item leaves the world and the ground table. Returns the
    /// item as it lay there.
    pub(crate) fn take_ground_item(&mut self, id: ObjectId) -> Option<GroundItem> {
        let item = self.ground.items.remove(&id)?;
        self.ground.auto_destroy.remove(&id);
        self.cancel_item_timers(&item);
        // `setSpawned(false)`, `World.removeVisibleObject(this, oldregion)`; the object leaves the
        // world table too (the item is somebody's inventory row from now on).
        self.decay(id);
        Some(item)
    }

    /// Java `Item.decayMe()` (with `ItemsOnGroundManager.removeObject`): the item vanishes for
    /// everybody around and is gone. The object id goes back to the factory.
    pub(crate) fn decay_ground_item(&mut self, id: ObjectId) -> bool {
        let Some(item) = self.take_ground_item(id) else {
            return false;
        };
        self.release_item_id(item.object_id());
        true
    }

    fn cancel_item_timers(&mut self, item: &GroundItem) {
        if let Some(timer) = item.owner_reset {
            self.timers.cancel(timer);
        }
        if let Some(protection) = item.drop_protection {
            self.timers.cancel(protection.timer);
        }
    }

    // ------------------------------------------------------------------ owner and protection

    /// `item.setOwnerId(owner)` with the 15 second `ResetOwner` of `ItemManager.createItem`.
    pub(crate) fn set_loot_owner(&mut self, item: &mut GroundItem, owner: ObjectId) {
        self.set_loot_owner_for(item, owner, LOOT_OWNER_MS);
    }

    /// `item.setOwnerId(owner)` with a `ResetOwner` after `ms` (a raid's loot right lasts
    /// `RaidLootRightsInterval`).
    pub(crate) fn set_loot_owner_for(&mut self, item: &mut GroundItem, owner: ObjectId, ms: u64) {
        item.owner = Some(owner);
        let due = self.tick + TimerWheel::ms_to_ticks(ms);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::ItemOwnerReset(item.object_id()));
        item.owner_reset = Some(timer);
    }

    /// Java `ResetOwner.run()`: the loot privilege ends (the item is on the ground, so its
    /// location is `VOID`).
    pub(crate) fn on_item_owner_reset(&mut self, id: ObjectId) {
        if let Some(item) = self.ground.items.get_mut(&id) {
            item.owner = None;
            item.owner_reset = None;
        }
    }

    /// Java `DropProtection.protect(creature)`: replaces a running protection.
    pub(crate) fn protect_item(&mut self, item: &mut GroundItem, creature: ObjectId) {
        // `unprotect()`.
        if let Some(old) = item.drop_protection.take() {
            self.timers.cancel(old.timer);
        }
        let due = self.tick + TimerWheel::ms_to_ticks(DROP_PROTECTION_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::ItemDropProtection(item.object_id()));
        item.drop_protection = Some(DropProtection {
            owner: creature,
            timer,
        });
    }

    /// Java `DropProtection.run()`.
    pub(crate) fn on_item_drop_protection_end(&mut self, id: ObjectId) {
        if let Some(item) = self.ground.items.get_mut(&id) {
            item.drop_protection = None;
        }
    }

    /// Java `DropProtection.tryPickUp(actor)`: no protection, the protected creature itself, or
    /// a member of its party.
    pub(crate) fn try_pick_up(&self, item: &GroundItem, actor: ObjectId) -> bool {
        item.drop_protection
            .is_none_or(|p| p.owner == actor || self.is_in_party_with(p.owner, actor))
    }

    // ------------------------------------------------------------------ auto destroy

    /// Java `ItemsAutoDestroyTaskManager.addItem(item)`: the drop time is now and the item is on
    /// the list.
    pub(crate) fn add_to_auto_destroy(&mut self, id: ObjectId) {
        let now = self.world_ms();
        let Some(item) = self.ground.items.get_mut(&id) else {
            return;
        };
        item.drop_time_ms = Some(now);
        self.ground.auto_destroy.insert(id);
        self.ensure_auto_destroy_timer();
    }

    /// The manager runs every 5 seconds, from the first item on.
    // NOTE: Java's task starts when the manager is first used and never stops; here it runs while
    // the list is not empty, so the 5 second phase starts at the first item.
    fn ensure_auto_destroy_timer(&mut self) {
        if self.ground.auto_destroy_timer.is_some() {
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(AUTO_DESTROY_INTERVAL_MS);
        let timer = self.timers.schedule_at(due, TimerKind::ItemsAutoDestroy);
        self.ground.auto_destroy_timer = Some(timer);
    }

    /// Java `ItemsAutoDestroyTaskManager.run()`.
    pub(crate) fn on_items_auto_destroy(&mut self) {
        self.ground.auto_destroy_timer = None;
        if self.ground.auto_destroy.is_empty() {
            return;
        }
        let now = self.world_ms();
        let settings = self.settings.ground.clone();
        let templates = self.item_templates();
        let ids: Vec<ObjectId> = self.ground.auto_destroy.iter().copied().collect();
        for id in ids {
            let Some(item) = self.ground.items.get(&id) else {
                // The item is not on the ground any more (its location is not `VOID`).
                self.ground.auto_destroy.remove(&id);
                continue;
            };
            let Some(drop_time) = item.drop_time_ms else {
                self.ground.auto_destroy.remove(&id);
                continue;
            };
            let template = templates.as_deref().and_then(|t| t.get(item.data.item_id));
            let auto_destroy_time: i64 = match template {
                Some(t) if t.auto_destroy_time > 0 => i64::from(t.auto_destroy_time),
                Some(t) if t.ex_immediate_effect => i64::from(settings.herb_auto_destroy_time),
                _ if settings.autodestroy_item_after == 0 => 3_600_000,
                _ => i64::from(settings.autodestroy_item_after) * 1000,
            };
            if now - drop_time > auto_destroy_time {
                // `ItemManager.destroyItem`: "If it's a pet control item, delete the pet as well."
                if template.is_some_and(|t| t.is_pet_item()) {
                    self.pet_row_deleted(id);
                }
                self.decay_ground_item(id);
                self.ground.auto_destroy.remove(&id);
            }
        }
        if !self.ground.auto_destroy.is_empty() {
            self.ensure_auto_destroy_timer();
        }
    }

    // ------------------------------------------------------------------ Npc.dropItem

    /// Java `Npc.dropItem(creature, itemId, itemCount)`: the monster's drop lands within 70 of it
    /// (`RANDOM_ITEM_DROP_LIMIT`) and is protected for `creature` for 15 seconds. Returns the
    /// last item dropped.
    pub(crate) fn npc_drop_item(
        &mut self,
        npc: ObjectId,
        creature: Option<ObjectId>,
        item_id: i32,
        item_count: i64,
    ) -> Option<ObjectId> {
        let templates = self.item_templates()?;
        let settings = self.settings.ground.clone();
        let mut last = None;
        let mut i: i64 = 0;
        while i < item_count {
            i += 1;
            let Some(template) = templates.get(item_id) else {
                tracing::error!("Item doesn't exist so cannot be dropped. Item ID: {item_id}");
                return None;
            };
            let id = self.allocate_item_id()?;
            let data = self.new_item_data(template, id, item_count);
            let mut item = GroundItem::new(data);
            // `ItemManager.createItem(LOOT, ...)`: the loot privilege.
            self.loot_privilege(&mut item, template, creature, npc);
            if let Some(creature) = creature {
                self.protect_item(&mut item, creature);
            }

            // Randomize drop position.
            let (nx, ny, nz) = self.position_of(npc)?;
            let new_x = (nx + self.rng().below(RANDOM_ITEM_DROP_LIMIT * 2 + 1)) - RANDOM_ITEM_DROP_LIMIT;
            let new_y = (ny + self.rng().below(RANDOM_ITEM_DROP_LIMIT * 2 + 1)) - RANDOM_ITEM_DROP_LIMIT;
            let new_z = nz + 20;

            if !self.drop_me(item, Some(npc), new_x, new_y, new_z) {
                self.release_item_id(id);
                return None;
            }
            last = Some(id);

            // Add drop to auto destroy item task.
            if !settings.is_protected_item(item_id)
                && ((settings.autodestroy_item_after > 0 && !template.ex_immediate_effect)
                    || (settings.herb_auto_destroy_time > 0 && template.ex_immediate_effect))
            {
                self.add_to_auto_destroy(id);
            }

            if let Some(item) = self.ground.items.get_mut(&id) {
                item.protected = false;
            }

            // If stackable, end loop as entire count is included in 1 instance of item.
            if template.stackable || !settings.multiple_item_drop {
                break;
            }
        }
        last
    }

    /// The loot privilege `ItemManager.createItem` gives a `LOOT` item.
    fn loot_privilege(
        &mut self,
        item: &mut GroundItem,
        template: &ItemTemplate,
        actor: Option<ObjectId>,
        reference: ObjectId,
    ) {
        let settings = &self.settings.ground;
        if settings.auto_loot_item_ids.contains(&template.id) {
            return;
        }
        let raid = self.npcs.get(&reference).is_some_and(|n| n.class.is_raid());
        if raid {
            // Loot privilege for raids: the leader of the first command channel that attacked
            // owns the item for `RaidLootRightsInterval`; without one (or with `AutoLootRaids`)
            // nobody does.
            if !settings.auto_loot_raids
                && let Some(owner) = self.raid_loot_owner(reference)
            {
                let interval =
                    u64::try_from(self.settings.party.loot_raids_privilege_interval_ms).unwrap_or(0);
                self.set_loot_owner_for(item, owner, interval);
            }
            return;
        }
        // NOTE: `(reference instanceof EventMonster) && eventDropOnGround()` is a script switch
        // (D-05) that is off by default.
        if !settings.auto_loot
            && let Some(actor) = actor
        {
            self.set_loot_owner(item, actor);
        }
    }

    // ------------------------------------------------------------------ ItemsOnGroundManager

    /// Java `ItemsOnGroundManager`'s constructor: the rows `load()` read go back into the world,
    /// and the periodic save starts. Call once at startup, after the id factory is installed.
    pub fn load_ground_items(&mut self, rows: Vec<GroundItemRow>) {
        let settings = self.settings.ground.clone();
        let templates = self.item_templates();
        let now_unix = unix_ms_now();
        let now_world = self.world_ms();
        let mut count = 0;
        for row in rows {
            let Some(template) = templates.as_deref().and_then(|t| t.get(row.item_id)) else {
                // Java: `new Item(...)` throws and the rest of the table is lost.
                warn!(
                    "ItemsOnGroundManager: item {} of row {} is unknown; row skipped.",
                    row.item_id, row.object_id.0
                );
                continue;
            };
            let mut data = self.new_item_data(template, row.object_id, 1);
            // This check and..
            if template.stackable && row.count > 1 {
                data.count = row.count;
            }
            // This, are really necessary?
            if row.enchant_level > 0 {
                data.enchant_level = row.enchant_level;
            }
            let mut item = GroundItem::new(data);
            (item.x, item.y, item.z) = (row.x, row.y, row.z);
            // `drop_time` is unix time in the table; the world counts its own milliseconds. A
            // protected row (-1) has no drop time in the sense of the auto destroy.
            item.drop_time_ms = (row.drop_time != -1).then(|| now_world - (now_unix - row.drop_time));
            item.protected = row.drop_time == -1;
            let id = row.object_id;
            self.ground.items.insert(id, item);
            self.register_object(id, ObjectKind::Item);
            self.spawn(id);
            if !self.ground.items.contains_key(&id) {
                continue;
            }
            count += 1;
            // Add to ItemsAutoDestroy only items not protected.
            // NOTE(java-quirk): `ItemsAutoDestroyTaskManager.addItem` sets the drop time to now,
            // so the clock of a saved item starts again at every server start.
            if !settings.is_protected_item(template.id)
                && row.drop_time > -1
                && ((settings.autodestroy_item_after > 0 && !template.ex_immediate_effect)
                    || (settings.herb_auto_destroy_time > 0 && template.ex_immediate_effect))
            {
                self.add_to_auto_destroy(id);
            }
        }
        if settings.save_dropped_item {
            info!("ItemsOnGroundManager: Loaded {count} items.");
        }
        self.start_ground_item_saves();
    }

    /// `ThreadPool.scheduleAtFixedRate(this, interval, interval)` when `SaveDroppedItemInterval`
    /// is above zero.
    fn start_ground_item_saves(&mut self) {
        let settings = &self.settings.ground;
        if !settings.save_dropped_item || settings.save_dropped_item_interval_ms <= 0 {
            return;
        }
        self.schedule_ground_item_save();
    }

    fn schedule_ground_item_save(&mut self) {
        let interval = u64::try_from(self.settings.ground.save_dropped_item_interval_ms).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(interval);
        let timer = self.timers.schedule_at(due, TimerKind::SaveGroundItems);
        self.ground.save_timer = Some(timer);
    }

    /// The periodic run of `ItemsOnGroundManager`.
    pub(crate) fn on_save_ground_items_timer(&mut self) {
        self.save_ground_items();
        self.schedule_ground_item_save();
    }

    /// Java `ItemsOnGroundManager.run()` / `saveInDb()`: the table is emptied and every item that
    /// lies on the ground is written. Nothing happens with `SaveDroppedItem` off.
    pub fn save_ground_items(&mut self) {
        if !self.settings.ground.save_dropped_item {
            return;
        }
        let rows = self.ground_item_rows();
        self.outbox.persist(PersistCommand::SaveGroundItems(rows));
    }

    /// The rows `ItemsOnGroundManager.run()` inserts, by object id.
    pub fn ground_item_rows(&self) -> Vec<GroundItemRow> {
        let templates = self.item_templates();
        let now_unix = unix_ms_now();
        let now_world = self.world_ms();
        self.ground_item_ids()
            .into_iter()
            // The markers of `//zone_visual` are `spawnMe`d, never `dropMe`d: the manager does not
            // know them.
            .filter(|id| !self.admin.zones.debug_items.contains(id))
            .filter_map(|id| self.ground.items.get(&id))
            // "Cursed Items not saved to ground, prevent double save."
            .filter(|item| !self.is_cursed_item(item.data.item_id))
            .map(|item| GroundItemRow {
                object_id: item.object_id(),
                item_id: item.data.item_id,
                count: item.data.count,
                enchant_level: item.data.enchant_level,
                x: item.x,
                y: item.y,
                z: item.z,
                // Item is protected or AutoDestroyed.
                drop_time: match item.drop_time_ms {
                    _ if item.protected => -1,
                    Some(dropped) => now_unix - (now_world - dropped),
                    None => 0,
                },
                // Set equip-able.
                equipable: templates
                    .as_deref()
                    .and_then(|t| t.get(item.data.item_id))
                    .is_some_and(crate::inventory::is_equipable),
            })
            .collect()
    }

    /// Debug aid for tests: the ground items around a position.
    pub fn ground_items_near(&self, x: i32, y: i32, range: i32) -> Vec<ObjectId> {
        let range = f64::from(range);
        self.ground_item_ids()
            .into_iter()
            .filter(|id| {
                self.ground.items.get(id).is_some_and(|i| {
                    let (dx, dy) = (f64::from(i.x - x), f64::from(i.y - y));
                    (dx * dx + dy * dy).sqrt() <= range
                })
            })
            .collect()
    }
}
