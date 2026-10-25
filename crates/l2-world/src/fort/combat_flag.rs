//! The combat flag of a fortress siege: it lies in the command post, the clan leader of an
//! attacker picks it up (and is forced to wear it), and hangs it on the flag pole with the flag
//! skills to take the fortress.
// Java: mechanics/siege/CombatFlag.java
// Java: managers/FortSiegeManager.java (isCombat, activateCombatFlag, checkIfCanPickup,
//       dropCombatFlag, getFlagList)
// Java: entity/actor/Player.java (addItem: the combat flag branch, doPickupItem, doDie, deleteMe)
// Java: network/clientpackets/EnterWorld.java ("Remove combat flag before teleporting")
// Java: dist/game/data/scripts/handlers/skill/effects/{TakeFort,TakeFortStart}.java
//
// # Model
// A [`CombatFlag`] is one of the up to three flags a fortress has (`FortSiege.ini`
// `NameFlagN`). It knows the item on the ground (`_itemInstance`, an object id of the ground
// table), the player who wears it (`_playerId`) and the worn item (`_item`). Whether a player
// wears a flag (Java `Player._combatFlagEquippedId`) is read from the flags, not kept twice.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{InventoryUpdate, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use super::zone::COMBAT_FLAG_ITEM_ID;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::ground_item::GroundItem;
use crate::inventory::item_name_param;
use crate::residence::FortId;
use crate::world::World;

/// `PlayerInventory` location names.
const LOC_PAPERDOLL: &str = "PAPERDOLL";

/// One combat flag (Java `CombatFlag`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CombatFlag {
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `_itemId`: 9819 in the shipped `FortSiege.ini`.
    pub item_id: i32,
    /// `_playerId`: who wears it.
    pub(crate) player: Option<ObjectId>,
    /// `_itemInstance`: the item lying on the ground.
    pub(crate) ground_item: Option<ObjectId>,
    /// `_item`: the worn item.
    pub(crate) worn_item: Option<ObjectId>,
}

impl CombatFlag {
    /// Java `new CombatFlag(fortId, x, y, z, 0, itemId)`.
    pub fn new(x: i32, y: i32, z: i32, item_id: i32) -> Self {
        Self {
            x,
            y,
            z,
            item_id,
            player: None,
            ground_item: None,
            worn_item: None,
        }
    }

    /// Java `getPlayerObjectId()`: 0 for nobody.
    pub fn player_object_id(&self) -> i32 {
        self.player.map_or(0, |p| p.0)
    }

    /// Java `getCombatFlagInstance()`.
    pub fn ground_item(&self) -> Option<ObjectId> {
        self.ground_item
    }
}

impl World {
    /// Java `FortManager.getFort(player)`: the fortress whose siege zone holds the player, by the
    /// player's own coordinates (the player may not be spawned yet, as at `EnterWorld`).
    pub(crate) fn fort_of_player(&self, player: ObjectId) -> Option<FortId> {
        let p = self.players.get(&player)?;
        self.fort_at(p.core.x, p.core.y, p.core.z)
    }

    /// Java `FortSiegeManager.isCombat(itemId)`.
    pub fn is_combat_flag_item(item_id: i32) -> bool {
        item_id == COMBAT_FLAG_ITEM_ID
    }

    /// Java `Player.isCombatFlagEquipped()`: a fortress's combat flag or a territory ward
    /// (M7-TW-01).
    pub fn is_combat_flag_equipped(&self, player: ObjectId) -> bool {
        self.fort
            .flags
            .values()
            .flatten()
            .any(|flag| flag.player == Some(player))
            || self.is_ward_carrier(player)
    }

    // ------------------------------------------------------------------ on the ground

    /// Java `CombatFlag.spawnMe()`: the item is created and dropped at the flag's place.
    pub(crate) fn fort_flag_spawn_me(&mut self, fort: FortId, index: usize) {
        let Some((x, y, z, item_id)) = self
            .fort
            .flags
            .get(&fort)
            .and_then(|f| f.get(index))
            .map(|f| (f.x, f.y, f.z, f.item_id))
        else {
            return;
        };
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(item_id) else {
            warn!("{fort}: the combat flag item {item_id} does not exist.");
            return;
        };
        let Some(object_id) = self.allocate_item_id() else {
            return;
        };
        let data = self.new_item_data(template, object_id, 1);
        if self.drop_me(GroundItem::new(data), None, x, y, z) {
            if let Some(flag) = self.fort.flags.get_mut(&fort).and_then(|f| f.get_mut(index)) {
                flag.ground_item = Some(object_id);
            }
        }
    }

    /// Java `CombatFlag.unSpawnMe()`.
    pub(crate) fn fort_flag_unspawn_me(&mut self, fort: FortId, index: usize) {
        let Some(flag) = self.fort.flags.get(&fort).and_then(|f| f.get(index)) else {
            return;
        };
        let (holder, ground) = (flag.player, flag.ground_item);
        if holder.is_some() {
            self.fort_flag_drop_it(fort, index);
        }
        if let Some(item) = ground {
            self.decay_ground_item(item);
        }
    }

    /// `for (CombatFlag cf : getFlagList(id)) cf.spawnMe()`.
    pub(crate) fn fort_spawn_flags(&mut self, fort: FortId) {
        let count = self.fort.flags.get(&fort).map_or(0, Vec::len);
        for index in 0..count {
            self.fort_flag_spawn_me(fort, index);
        }
    }

    /// `FortSiege.unSpawnFlags()`.
    pub(crate) fn fort_unspawn_flags(&mut self, fort: FortId) {
        let count = self.fort.flags.get(&fort).map_or(0, Vec::len);
        for index in 0..count {
            self.fort_flag_unspawn_me(fort, index);
        }
    }

    // ------------------------------------------------------------------ pickup

    /// Java `FortSiegeManager.checkIfCanPickup(player)`.
    pub fn fort_check_if_can_pickup(&mut self, player: ObjectId) -> bool {
        if self.is_combat_flag_equipped(player) {
            return false;
        }
        if !self.is_clan_leader(player) {
            self.send_message(player, "The flag can only be picked up by a clan leader.");
            return false;
        }
        let clan = self.clan_id_of(player);
        let fort = self.fort_of_player(player);
        let Some(fort) = fort.filter(|f| f.0 > 0) else {
            return false;
        };
        if !self.fort_check_is_attacker(fort, clan) {
            return false;
        }
        if !self.fort_siege_in_progress(fort) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheFortressBattleOfS1HasFinished)
                    .with(SystemMessageParam::ItemName(COMBAT_FLAG_ITEM_ID)),
            );
            return false;
        }
        true
    }

    /// Java `FortSiegeManager.activateCombatFlag(player, item)`: `false` when the player may not
    /// pick the flag up. The flag that lay on the ground as `item` is worn.
    pub fn fort_activate_combat_flag(&mut self, player: ObjectId, item: ObjectId) -> bool {
        if !self.fort_check_if_can_pickup(player) {
            return false;
        }
        let fort = self.fort_of_player(player);
        // `getFort(player).getResidenceId()` fails for no fortress: checkIfCanPickup refused.
        let Some(fort) = fort else {
            return false;
        };
        let indices: Vec<usize> = self
            .fort
            .flags
            .get(&fort)
            .map(|flags| {
                flags
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.ground_item == Some(item))
                    .map(|(i, _)| i)
                    .collect()
            })
            .unwrap_or_default();
        for index in indices {
            self.fort_flag_activate(fort, index, player, item);
        }
        true
    }

    /// Java `CombatFlag.activate(player, item)`.
    fn fort_flag_activate(&mut self, fort: FortId, index: usize, player: ObjectId, item: ObjectId) -> bool {
        if self.is_mounted(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotMeetTheRequiredConditionToEquipThatItem),
            );
            return false;
        }
        // Player holding it data.
        if let Some(flag) = self.fort.flags.get_mut(&fort).and_then(|f| f.get_mut(index)) {
            flag.player = Some(player);
            flag.ground_item = None;
            flag.worn_item = Some(item);
        }
        // Equip with the weapon.
        self.equip_item_and_record(player, item);
        let item_id = self
            .inventory_item(player, item)
            .map_or(COMBAT_FLAG_ITEM_ID, |i| i.item_id);
        let templates = self.item_templates();
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveEquippedYourS1)
                .with(item_name_param(templates.as_deref(), item_id)),
        );
        // Refresh inventory.
        if let Some(entry) = self.modified_entry(player, item) {
            self.send_inventory_update(player, entry);
        }
        // Refresh player stats.
        self.broadcast_user_info(player);
        true
    }

    // ------------------------------------------------------------------ dropping

    /// Java `FortSiegeManager.dropCombatFlag(player, fortId)`.
    pub fn fort_drop_combat_flag(&mut self, player: ObjectId, fort: FortId) {
        let indices: Vec<usize> = self
            .fort
            .flags
            .get(&fort)
            .map(|flags| {
                flags
                    .iter()
                    .enumerate()
                    .filter(|(_, f)| f.player == Some(player))
                    .map(|(i, _)| i)
                    .collect()
            })
            .unwrap_or_default();
        for index in indices {
            self.fort_flag_drop_it(fort, index);
            if self.fort_siege_in_progress(fort) {
                self.fort_flag_spawn_me(fort, index);
            }
        }
    }

    /// Java `CombatFlag.dropIt()`: the flag comes off and is destroyed.
    fn fort_flag_drop_it(&mut self, fort: FortId, index: usize) {
        let Some(flag) = self.fort.flags.get_mut(&fort).and_then(|f| f.get_mut(index)) else {
            return;
        };
        let (Some(player), item) = (flag.player.take(), flag.worn_item.take()) else {
            return;
        };
        if let Some(item) = item {
            self.fort_destroy_worn_flag(player, item);
        }
        self.broadcast_user_info(player);
    }

    /// `unEquipItemInBodySlot(bodyPart)` and `destroyItem(DESTROY, item, null, true)`.
    fn fort_destroy_worn_flag(&mut self, player: ObjectId, item: ObjectId) {
        let Some(worn) = self.inventory_item(player, item).cloned() else {
            return;
        };
        if worn.loc == LOC_PAPERDOLL {
            self.unequip_item_in_slot_and_record(player, usize::try_from(worn.loc_data).unwrap_or(0));
        }
        let Some(removed) = self.destroy_inventory_item(player, item, worn.count) else {
            return;
        };
        let entry = self.update_entry(&removed, InventoryUpdate::REMOVED);
        self.send_inventory_update_now(player, vec![entry]);
        self.send_cur_load(player);
        let templates = self.item_templates();
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::S1HasDisappeared)
                .with(item_name_param(templates.as_deref(), removed.item_id)),
        );
    }

    /// The combat flag block of `EnterWorld`, `Player.deleteMe` and `Player.doDie`: with a flag in
    /// the inventory, inside a fortress `dropCombatFlag`, elsewhere the flag is unequipped and
    /// destroyed.
    pub(crate) fn fort_remove_combat_flag(&mut self, player: ObjectId) {
        let Some(item) = self
            .inventory_item_by_item_id(player, COMBAT_FLAG_ITEM_ID)
            .map(|i| i.object_id)
        else {
            return;
        };
        let fort = self.fort_of_player(player);
        match fort {
            Some(fort) => self.fort_drop_combat_flag(player, fort),
            None => self.fort_destroy_worn_flag(player, item),
        }
    }

    /// The hook of `Player.addItem`: a combat flag that came into the inventory is worn and the
    /// siege is told ("C1 has acquired the flag").
    pub(crate) fn fort_on_combat_flag_added(&mut self, player: ObjectId, item: ObjectId) {
        if !self.fort_activate_combat_flag(player, item) {
            return;
        }
        let Some(fort) = self.fort_of_player(player) else {
            return;
        };
        let name = self
            .players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let message =
            SystemMessage::new(SystemMessageId::C1HasAcquiredTheFlag).with(SystemMessageParam::Text(name));
        self.fort_siege_announce(fort, &message);
    }
}

/// Java `TakeFort`: the flag is hung on the pole, the fortress changes hands.
// Java: dist/game/data/scripts/handlers/skill/effects/TakeFort.java
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TakeFort;

impl TakeFort {
    pub(crate) fn create(
        _template: &l2_data::skill::EffectTemplate,
    ) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(TakeFort))
    }
}

impl EffectHandler for TakeFort {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let player = cx.effector;
        if !world.is_player(player) {
            return;
        }
        let Some(fort) = world.fort_of_player(player) else {
            return;
        };
        if let Some(clan) = world.clan_of(player).map(|c| c.id) {
            world.fort_end_of_siege(fort, clan);
        }
    }
}

/// Java `TakeFortStart`: the siege is told a clan is displaying the flag.
// Java: dist/game/data/scripts/handlers/skill/effects/TakeFortStart.java
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct TakeFortStart;

impl TakeFortStart {
    pub(crate) fn create(
        _template: &l2_data::skill::EffectTemplate,
    ) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(TakeFortStart))
    }
}

impl EffectHandler for TakeFortStart {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let player = cx.effector;
        if !world.is_player(player) {
            return;
        }
        let fort = world.fort_of_player(player);
        let clan = world.clan_of(player).map(|c| c.name.clone());
        if let (Some(fort), Some(name)) = (fort, clan) {
            let message = SystemMessage::new(SystemMessageId::S1ClanIsTryingToDisplayAFlag)
                .with(SystemMessageParam::Text(name));
            world.fort_siege_announce(fort, &message);
        }
    }
}
