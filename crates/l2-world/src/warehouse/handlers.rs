//! The warehouse bypasses (`PrivateWarehouse`, `ClanWarehouse`, `Freight`) and the packets
//! `SendWareHouseDepositList`, `SendWareHouseWithDrawList`, `RequestPackageSend` and
//! `RequestPackageSendableItemList`.
// Java: dist/game/data/scripts/handlers/bypass/npc/{PrivateWarehouse,ClanWarehouse,Freight}.java
// Java: network/clientpackets/{SendWareHouseDepositList,SendWareHouseWithDrawList,
//       RequestPackageSend,RequestPackageSendableItemList}.java

use l2_data::item::ADENA_ID;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::client::WarehouseListEntry;
use l2_protocol_game::server::warehouse::{WH_TYPE_CLAN, WH_TYPE_FREIGHT, WH_TYPE_PRIVATE};
use l2_protocol_game::server::{ActionFailed, InventoryUpdate, NpcHtmlMessage, PackageToList, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::container::ContainerRef;
use super::moves::Place;
use super::sorted::{A2Z, WarehouseListType, get_order};
use crate::bypass::BypassHandlers;
use crate::clan::model::ClanAccess;
use crate::dialog::INTERACTION_DISTANCE;
use crate::outbox::ItemRowOp;
use crate::target::distance_2d;
use crate::world::{World, unix_ms_now};

/// The fee per item of a warehouse deposit: `_items.size() * 30`, not configurable.
const DEPOSIT_FEE_PER_ITEM: i64 = 30;

pub(super) fn register(handlers: &mut BypassHandlers) {
    for command in ["WithdrawP", "WithdrawSortedP", "DepositP"] {
        handlers.register(command, private_warehouse_handler);
    }
    for command in ["WithdrawC", "WithdrawSortedC", "DepositC"] {
        handlers.register(command, clan_warehouse_handler);
    }
    for command in ["package_withdraw", "package_deposit"] {
        handlers.register(command, freight_handler);
    }
}

/// Java `String.split(" ")`: the pieces between the spaces, without the empty pieces at the end.
fn java_split_space(text: &str) -> Vec<&str> {
    let mut parts: Vec<&str> = text.split(' ').collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// What `WithdrawSortedP/C <type> <order>` asks for: `None` for a type Java's
/// `WarehouseListType.valueOf` throws on.
fn sorted_request(command: &str) -> Option<(WarehouseListType, i8)> {
    let param = java_split_space(command);
    if param.len() > 2 {
        Some((WarehouseListType::from_name(param[1])?, get_order(param[2])))
    } else if param.len() > 1 {
        Some((WarehouseListType::from_name(param[1])?, A2Z))
    } else {
        Some((WarehouseListType::All, A2Z))
    }
}

impl World {
    /// `target.isNpc()` of the bypass handlers.
    fn warehouse_npc(&self, target: Option<ObjectId>) -> Option<ObjectId> {
        target.filter(|t| self.npcs.contains_key(t))
    }

    /// The `npc_type` of the template of an NPC.
    fn warehouse_npc_type(&self, npc: ObjectId) -> Option<&str> {
        let n = self.npcs.get(&npc)?;
        Some(self.npc_world.template(n.npc_id)?.npc_type.as_str())
    }

    /// Java `Npc.isWarehouse()`: the `Warehouse` class, and the clan hall and fortress managers.
    fn is_warehouse_npc(&self, npc: ObjectId) -> bool {
        matches!(
            self.warehouse_npc_type(npc),
            Some("Warehouse" | "ClanHallManager" | "FortManager")
        )
    }

    /// The page with the sorting buttons (`WhSortedP.htm` / `WhSortedC.htm`).
    fn show_sorting_page(&mut self, player: ObjectId, npc: ObjectId, file: &str) {
        let Some(html) = self.get_htm(player, file) else {
            return;
        };
        let html = html.replace("%objectId%", &npc.0.to_string());
        if let Some(message) = NpcHtmlMessage::new(npc.0, &html) {
            self.send_html(player, &message);
        }
    }

    /// Java `setActiveWarehouse(warehouse)`.
    fn set_active_warehouse(&mut self, player: ObjectId, container: Option<ContainerRef>) {
        if let Some(p) = self.players.get_mut(&player) {
            p.warehouse.active = container;
        }
    }

    /// Java `Player.setClan(null)`: `_activeWarehouse = null`.
    pub(crate) fn clear_active_warehouse(&mut self, player: ObjectId) {
        self.set_active_warehouse(player, None);
    }

    /// The expired items of a container are destroyed when the withdraw window opens
    /// (`isTimeLimitedItem() && getRemainingTime() <= 0`, `destroyItem(DESTROY, ...)`).
    fn destroy_expired_items(&mut self, player: ObjectId, container: ContainerRef) {
        let now = unix_ms_now();
        let expired: Vec<ObjectId> = self
            .container_items(container)
            .unwrap_or(&[])
            .iter()
            .filter(|i| i.time > 0 && i.time - now <= 0)
            .map(|i| i.object_id)
            .collect();
        let mut ops = Vec::new();
        for object_id in expired {
            if let Some(items) = self.container_items_mut(container)
                && let Some(index) = items.iter().position(|i| i.object_id == object_id)
            {
                items.remove(index);
                // `ItemManager.destroyItem` releases the id, `updateDatabase()` deletes the row.
                self.release_item_id(object_id);
                ops.push(ItemRowOp::Delete { object_id });
            }
        }
        self.flush_item_rows(player, ops);
    }

    // ------------------------------------------------------------------ private warehouse

    /// Java `PrivateWarehouse.showWithdrawWindow(player, itemtype, sortorder)`.
    fn show_private_withdraw_window(&mut self, player: ObjectId, sorted: Option<(WarehouseListType, i8)>) {
        self.send(player, &ActionFailed);
        let container = ContainerRef::Private(player);
        self.set_active_warehouse(player, Some(container));
        if self.container_size(container) == 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveNotDepositedAnyItemsInYourWarehouse),
            );
            return;
        }
        self.send_withdraw_window(player, WH_TYPE_PRIVATE, sorted);
    }

    /// The sorted or the plain withdraw list of the active warehouse.
    fn send_withdraw_window(
        &mut self,
        player: ObjectId,
        wh_type: i16,
        sorted: Option<(WarehouseListType, i8)>,
    ) {
        match sorted {
            Some((list_type, order)) => {
                if let Some(packet) = self.sorted_warehouse_list(player, wh_type, list_type, order) {
                    self.send(player, &packet);
                }
            }
            None => {
                if let Some(packet) = self.warehouse_withdrawal_list(player, wh_type) {
                    self.send(player, &packet);
                }
            }
        }
    }

    // ------------------------------------------------------------------ clan warehouse

    /// Java `ClanWarehouse.showWithdrawWindow(player, itemtype, sortorder)`.
    fn show_clan_withdraw_window(&mut self, player: ObjectId, sorted: Option<(WarehouseListType, i8)>) {
        self.send(player, &ActionFailed);
        if !self.has_clan_access(player, ClanAccess::AccessWarehouse) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveTheRightToUseTheClanWarehouse),
            );
            return;
        }
        let clan_id = self.clan_id_of(player);
        let container = ContainerRef::Clan(clan_id);
        self.set_active_warehouse(player, Some(container));
        if self.container_size(container) == 0 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveNotDepositedAnyItemsInYourWarehouse),
            );
            return;
        }
        self.destroy_expired_items(player, container);
        self.send_withdraw_window(player, WH_TYPE_CLAN, sorted);
    }

    // ------------------------------------------------------------------ the packets

    /// Java's first lines of the three packets that act on the active warehouse: the manager
    /// (`getLastFolkNPC`) must be a warehouse the player can talk to, unless the player is a GM.
    fn warehouse_manager_in_reach(&self, player: ObjectId) -> bool {
        let manager = self.players.get(&player).and_then(|p| p.talk.last_folk_npc);
        let in_reach = manager.is_some_and(|m| self.is_warehouse_npc(m) && self.can_interact(m, player));
        if !in_reach && !self.is_gm(player) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YouFailedAtSendingThePackageBecauseYouAreTooFarFromTheWarehouse,
                ),
            );
            return false;
        }
        true
    }

    /// `PlayerConfig.MAX_ITEM_IN_PACKET`: the largest inventory.
    fn max_item_in_packet(&self) -> usize {
        let s = &self.settings;
        let max = s
            .inventory_maximum_no_dwarf
            .max(s.inventory_maximum_dwarf.max(s.inventory_maximum_gm));
        usize::try_from(max).unwrap_or(0)
    }

    /// The list of a packet if Java keeps it: not dropped by the reader and not above
    /// `MAX_ITEM_IN_PACKET`.
    fn checked_warehouse_list(
        &self,
        items: Option<Vec<WarehouseListEntry>>,
    ) -> Option<Vec<WarehouseListEntry>> {
        items.filter(|list| list.len() <= self.max_item_in_packet())
    }

    /// `PunishmentManager.handleIllegalPlayerAction(...)` with the default punishment.
    fn warehouse_illegal_action(&mut self, player: ObjectId, what: &str) {
        let name = self.players.get(&player).map_or("?", |p| p.data.name.as_str());
        let message = format!(
            "Warning!! Character {name} of account {} {what}",
            self.account_of(player)
        );
        let punishment = self.default_punish();
        self.handle_illegal_player_action(player, &message, punishment);
    }

    /// The `if (!ALT_GAME_KARMA_PLAYER_CAN_USE_WAREHOUSE && getKarma() > 0) return` of every packet.
    fn karma_blocks_warehouse(&self, player: ObjectId) -> bool {
        !self.settings.talk.karma_player_can_use_warehouse
            && self.players.get(&player).is_some_and(|p| p.data.karma > 0)
    }

    /// Sends the `InventoryUpdate` entries and the load that end every deposit and withdraw.
    fn finish_warehouse_packet(
        &mut self,
        player: ObjectId,
        entries: Vec<l2_protocol_game::server::ItemUpdate>,
    ) {
        // `player.sendInventoryUpdate(playerIU)`
        for entry in entries {
            self.send_inventory_update(player, entry);
        }
        // `StatusUpdate` with `CUR_LOAD`.
        self.send_cur_load(player);
    }

    /// Dispatches the warehouse packets of `ClientInput::Warehouse`.
    pub(crate) fn on_warehouse_input(&mut self, id: ObjectId, input: super::WarehouseInput) {
        if !self.players.contains_key(&id) {
            return;
        }
        match input {
            super::WarehouseInput::Deposit { items } => self.on_warehouse_deposit(id, items),
            super::WarehouseInput::Withdraw { items } => self.on_warehouse_withdraw(id, items),
            super::WarehouseInput::PackageSend { object_id, items } => {
                self.on_request_package_send(id, object_id, items);
            }
            super::WarehouseInput::PackageSendableList { object_id } => {
                // Java `RequestPackageSendableItemList.runImpl`.
                let packet = self.package_sendable_list(id, object_id);
                self.send(id, &packet);
            }
        }
    }

    /// Java `SendWareHouseDepositList.runImpl`.
    fn on_warehouse_deposit(&mut self, id: ObjectId, items: Option<Vec<WarehouseListEntry>>) {
        let Some(items) = self.checked_warehouse_list(items) else {
            return;
        };
        // FOLLOW-UP: the flood protector `canPerformTransaction` ("You are depositing items too
        // fast.").
        let Some(warehouse) = self.players.get(&id).and_then(|p| p.warehouse.active) else {
            return;
        };
        if !self.warehouse_manager_in_reach(id) {
            return;
        }
        let is_private = warehouse.is_private();
        if !is_private && !self.player_allows_transaction(id) {
            self.send_message(id, "Transactions are disabled for your Access Level.");
            return;
        }
        // M6-ENCHANT: `getActiveEnchantItemId() != ID_NONE` cancels the enchanting
        // (`YOU_HAVE_CANCELLED_THE_ENCHANTING_PROCESS`, `EnchantResult(2, 0, 0)`) and returns.
        // Alt game - Karma punishment.
        if self.karma_blocks_warehouse(id) {
            return;
        }

        // Freight price from config or normal price per item slot (30).
        let fee = i64::try_from(items.len()).unwrap_or(i64::MAX) * DEPOSIT_FEE_PER_ITEM;
        let mut current_adena = self.adena_of(id);
        let mut slots: i64 = 0;
        for entry in &items {
            let Some(item) = self.check_item_manipulation(id, ObjectId(entry.object_id), entry.count) else {
                warn!(
                    "Error depositing a warehouse object for char {} (validity check)",
                    self.players.get(&id).map_or("?", |p| p.data.name.as_str())
                );
                return;
            };
            // Calculate needed adena and slots.
            if item.item_id == ADENA_ID {
                current_adena -= entry.count;
            }
            if !self.data.item_stackable(item.item_id) {
                slots += entry.count;
            } else if self.container_item_by_item_id(warehouse, item.item_id).is_none() {
                slots += 1;
            }
        }

        // Item Max Limit Check.
        if !self.container_validate_capacity(warehouse, slots) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted),
            );
            return;
        }

        // Check if enough adena and charge the fee.
        if current_adena < fee || !self.reduce_adena(id, fee, false) {
            self.send(id, &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena));
            return;
        }

        // Get current tradelist if any.
        // NOTE(java-quirk): the fee is already paid when a trade list stops the deposit.
        if self.players.get(&id).is_some_and(|p| p.trade.active.is_some()) {
            return;
        }

        // Proceed to the transfer.
        let mut ops = Vec::new();
        let mut entries = Vec::new();
        let templates = self.item_templates();
        for entry in &items {
            // Check validity of requested item.
            let Some(old_item) = self.check_item_manipulation(id, ObjectId(entry.object_id), entry.count)
            else {
                warn!(
                    "Error depositing a warehouse object for char {} (olditem == null)",
                    self.players.get(&id).map_or("?", |p| p.data.name.as_str())
                );
                break;
            };
            let allowed = templates
                .as_deref()
                .and_then(|t| t.get(old_item.item_id))
                .is_some_and(|t| {
                    self.item_is_depositable(&old_item, t, is_private)
                        && self.is_item_available(id, &old_item, t, true, is_private)
                });
            if !allowed {
                continue;
            }
            let Some(moved) = self.move_item(
                Place::Inventory(id),
                old_item.object_id,
                entry.count,
                Place::Container(warehouse),
                id,
                &mut ops,
            ) else {
                warn!("Error depositing a warehouse object for char {id} (newitem == null)");
                continue;
            };
            let change = if moved.old.count > 0 && !moved.moved_whole {
                InventoryUpdate::MODIFIED
            } else {
                InventoryUpdate::REMOVED
            };
            entries.push(self.update_entry(&moved.old, change));
        }
        self.flush_item_rows(id, ops);
        self.finish_warehouse_packet(id, entries);
    }

    /// Java `SendWareHouseWithDrawList.runImpl`.
    fn on_warehouse_withdraw(&mut self, id: ObjectId, items: Option<Vec<WarehouseListEntry>>) {
        let Some(items) = self.checked_warehouse_list(items) else {
            return;
        };
        // FOLLOW-UP: the flood protector `canPerformTransaction` ("You are withdrawing items too
        // fast.").
        let Some(warehouse) = self.players.get(&id).and_then(|p| p.warehouse.active) else {
            return;
        };
        if !self.warehouse_manager_in_reach(id) {
            return;
        }
        if !warehouse.is_private() && !self.player_allows_transaction(id) {
            self.send_message(id, "Transactions are disabled for your Access Level.");
            return;
        }
        // M6-ENCHANT: the active enchant scroll (see the deposit).
        if self.karma_blocks_warehouse(id) {
            return;
        }

        if self.settings.warehouse.members_can_withdraw_from_clanwh {
            if warehouse.is_clan() && !self.has_clan_access(id, ClanAccess::AccessWarehouse) {
                return;
            }
        } else if warehouse.is_clan() && !self.is_clan_leader(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::ItemsLeftAtTheClanHallWarehouseCanOnlyBeRetrievedByTheClanLeaderDoYouWantToContinue,
                ),
            );
            return;
        }

        // Java's `weight` and `slots` are ints: `+=` of a long narrows.
        let mut weight: i32 = 0;
        let mut slots: i32 = 0;
        for entry in &items {
            // Calculate needed slots.
            let item = self.container_item(warehouse, ObjectId(entry.object_id));
            let Some(item) = item.filter(|i| i.count >= entry.count).cloned() else {
                self.warehouse_illegal_action(id, "tried to withdraw non-existent item from warehouse.");
                return;
            };
            let template_weight = self.data.item(item.item_id).map_or(0, |t| t.weight);
            weight = weight.wrapping_add(entry.count.wrapping_mul(i64::from(template_weight)) as i32);
            if !self.data.item_stackable(item.item_id) {
                slots = slots.wrapping_add(entry.count as i32);
            } else if self.inventory_item_by_item_id(id, item.item_id).is_none() {
                slots = slots.wrapping_add(1);
            }
        }

        // Item Max Limit Check.
        if !self.validate_capacity(id, i64::from(slots), false) {
            self.send(id, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
            return;
        }
        // Weight limit Check
        if !self.validate_weight(id, i64::from(weight)) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheWeightLimit),
            );
            return;
        }

        // Proceed to the transfer.
        let mut ops = Vec::new();
        let mut entries = Vec::new();
        let mut aborted = false;
        for entry in &items {
            let old_item = self.container_item(warehouse, ObjectId(entry.object_id));
            if !old_item.is_some_and(|i| i.count >= entry.count) {
                warn!("Error withdrawing a warehouse object for char {id} (olditem == null)");
                aborted = true;
                break;
            }
            let Some(moved) = self.move_item(
                Place::Container(warehouse),
                ObjectId(entry.object_id),
                entry.count,
                Place::Inventory(id),
                id,
                &mut ops,
            ) else {
                warn!("Error withdrawing a warehouse object for char {id} (newitem == null)");
                aborted = true;
                break;
            };
            let change = if moved.new.count > entry.count {
                InventoryUpdate::MODIFIED
            } else {
                InventoryUpdate::ADDED
            };
            entries.push(self.update_entry(&moved.new, change));
        }
        self.flush_item_rows(id, ops);
        // NOTE(java-quirk): a failed item ends the packet without the `InventoryUpdate` of the
        // items moved before it.
        if !aborted {
            self.finish_warehouse_packet(id, entries);
        }
    }

    /// Java `RequestPackageSend.runImpl`: the items go to the freight of another character of
    /// the account.
    fn on_request_package_send(
        &mut self,
        id: ObjectId,
        receiver: i32,
        items: Option<Vec<WarehouseListEntry>>,
    ) {
        let Some(items) = self.checked_warehouse_list(items) else {
            return;
        };
        let receiver = ObjectId(receiver);
        let is_account_char = self
            .players
            .get(&id)
            .is_some_and(|p| p.warehouse.account_chars.iter().any(|(c, _)| *c == receiver));
        if !is_account_char {
            return;
        }
        // FOLLOW-UP: the flood protector ("You are sending freight packages too fast.").
        let manager = self.players.get(&id).and_then(|p| p.talk.last_folk_npc);
        let in_reach = manager
            .and_then(|m| Some((self.position_of(m)?, self.position_of(id)?)))
            .is_some_and(|(npc, me)| distance_2d(me, npc) < f64::from(INTERACTION_DISTANCE));
        if !in_reach {
            return;
        }
        // M6-ENCHANT: an active enchant scroll is an illegal action here ("tried to use enchant
        // Exploit!").
        // Get current tradelist if any.
        if self.players.get(&id).is_some_and(|p| p.trade.active.is_some()) {
            return;
        }
        // Alt game - Karma punishment.
        if self.karma_blocks_warehouse(id) {
            return;
        }

        // Freight price from config per item slot (an int).
        let fee = i64::from((items.len() as i32).wrapping_mul(self.settings.warehouse.freight_price));
        let mut current_adena = self.adena_of(id);
        let mut slots: i64 = 0;
        let warehouse = ContainerRef::Freight {
            holder: id,
            owner: receiver,
        };
        for entry in &items {
            // Check validity of requested item.
            let Some(item) = self.check_item_manipulation(id, ObjectId(entry.object_id), entry.count) else {
                warn!("Error depositing a warehouse object for char {id} (validity check)");
                return;
            };
            let freightable = self
                .item_templates()
                .as_deref()
                .and_then(|t| t.get(item.item_id))
                .is_some_and(|t| t.freightable);
            if !freightable {
                return;
            }
            // Calculate needed adena and slots.
            if item.item_id == ADENA_ID {
                current_adena -= entry.count;
            } else if !self.data.item_stackable(item.item_id) {
                slots += entry.count;
            } else if self.container_item_by_item_id(warehouse, item.item_id).is_none() {
                slots += 1;
            }
        }

        // Item Max Limit Check.
        if !self.container_validate_capacity(warehouse, slots) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted),
            );
            return;
        }

        // Check if enough adena and charge the fee.
        if current_adena < fee || !self.reduce_adena(id, fee, false) {
            self.send(id, &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdena));
            return;
        }

        // Proceed to the transfer.
        let mut ops = Vec::new();
        let mut entries = Vec::new();
        for entry in &items {
            // Check validity of requested item.
            let Some(old_item) = self.check_item_manipulation(id, ObjectId(entry.object_id), entry.count)
            else {
                warn!("Error depositing a warehouse object for char {id} (olditem == null)");
                self.flush_item_rows(id, ops);
                return;
            };
            let Some(moved) = self.move_item(
                Place::Inventory(id),
                old_item.object_id,
                entry.count,
                Place::Container(warehouse),
                id,
                &mut ops,
            ) else {
                warn!("Error depositing a warehouse object for char {id} (newitem == null)");
                continue;
            };
            let change = if moved.old.count > 0 && !moved.moved_whole {
                InventoryUpdate::MODIFIED
            } else {
                InventoryUpdate::REMOVED
            };
            entries.push(self.update_entry(&moved.old, change));
        }
        self.flush_item_rows(id, ops);
        self.finish_warehouse_packet(id, entries);
    }

    /// `AccessLevel.allowTransaction()` of the player.
    fn player_allows_transaction(&self, player: ObjectId) -> bool {
        self.players
            .get(&player)
            .is_some_and(|p| self.data.access_level_allows_transaction(p.data.access_level))
    }
}

// ---------------------------------------------------------------------- the bypass handlers

/// Java `PrivateWarehouse.onCommand`.
fn private_warehouse_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    target: Option<ObjectId>,
) -> bool {
    let Some(npc) = world.warehouse_npc(target) else {
        return false;
    };
    // `player.isEnchanting()` is not modelled (M6-ENCHANT).
    let lower = command.to_lowercase();
    if lower.starts_with("withdrawp") {
        if world.settings.warehouse.sorting_private {
            world.show_sorting_page(player, npc, "data/html/mods/WhSortedP.htm");
        } else {
            world.show_private_withdraw_window(player, None);
        }
        true
    } else if lower.starts_with("withdrawsortedp") {
        // `WarehouseListType.valueOf` throws on a bad type; Java logs it and returns false.
        let Some(sorted) = sorted_request(command) else {
            warn!("Exception in PrivateWarehouse: bad warehouse list type in [{command}]");
            return false;
        };
        world.show_private_withdraw_window(player, Some(sorted));
        true
    } else if lower.starts_with("depositp") {
        world.send(player, &ActionFailed);
        world.set_active_warehouse(player, Some(ContainerRef::Private(player)));
        // `setInventoryBlockingStatus(true)`: not modelled (nothing reads the lock).
        if let Some(packet) = world.warehouse_deposit_list(player, WH_TYPE_PRIVATE) {
            world.send(player, &packet);
        }
        true
    } else {
        false
    }
}

/// Java `ClanWarehouse.onCommand`.
fn clan_warehouse_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    target: Option<ObjectId>,
) -> bool {
    // `target instanceof Warehouse` or `ClanHallManager`.
    let Some(npc) = world.warehouse_npc(target) else {
        return false;
    };
    if !matches!(
        world.warehouse_npc_type(npc),
        Some("Warehouse" | "ClanHallManager")
    ) {
        return false;
    }
    // `player.isEnchanting()` is not modelled (M6-ENCHANT).
    let Some(clan) = world.clan_of(player) else {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::YouDoNotHaveTheRightToUseTheClanWarehouse),
        );
        return false;
    };
    let clan_id = clan.id;
    if clan.level == 0 {
        world.send(
            player,
            &SystemMessage::new(SystemMessageId::OnlyClansOfClanLevel1OrHigherCanUseAClanWarehouse),
        );
        return false;
    }

    let lower = command.to_lowercase();
    if lower.starts_with("withdrawc") {
        if world.settings.warehouse.sorting_clan {
            world.show_sorting_page(player, npc, "data/html/mods/WhSortedC.htm");
        } else {
            world.show_clan_withdraw_window(player, None);
        }
        true
    } else if lower.starts_with("withdrawsortedc") {
        let Some(sorted) = sorted_request(command) else {
            warn!("Exception in ClanWarehouse: bad warehouse list type in [{command}]");
            return false;
        };
        world.show_clan_withdraw_window(player, Some(sorted));
        true
    } else if lower.starts_with("depositc") {
        world.send(player, &ActionFailed);
        world.set_active_warehouse(player, Some(ContainerRef::Clan(clan_id)));
        // `setInventoryBlockingStatus(true)`: not modelled.
        if let Some(packet) = world.warehouse_deposit_list(player, WH_TYPE_CLAN) {
            world.send(player, &packet);
        }
        true
    } else {
        false
    }
}

/// Java `Freight.onCommand`.
fn freight_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    if world.warehouse_npc(target).is_none() {
        return false;
    }
    if command.eq_ignore_ascii_case("package_withdraw") {
        let freight = ContainerRef::Freight {
            holder: player,
            owner: player,
        };
        if world.container_size(freight) > 0 {
            world.set_active_warehouse(player, Some(freight));
            world.destroy_expired_items(player, freight);
            if let Some(packet) = world.warehouse_withdrawal_list(player, WH_TYPE_FREIGHT) {
                world.send(player, &packet);
            }
        } else {
            world.send(
                player,
                &SystemMessage::new(SystemMessageId::YouHaveNotDepositedAnyItemsInYourWarehouse),
            );
        }
    } else if command.eq_ignore_ascii_case("package_deposit") {
        let chars = world
            .players
            .get(&player)
            .map(|p| p.warehouse.account_chars.clone())
            .unwrap_or_default();
        if chars.is_empty() {
            world.send(
                player,
                &SystemMessage::new(SystemMessageId::ThatCharacterDoesNotExist),
            );
        } else {
            let packet = PackageToList {
                players: chars.into_iter().map(|(id, name)| (id.0, name)).collect(),
            };
            world.send(player, &packet);
        }
    }
    false
}
