//! The load, the current and the next auction of an instance, the state timer and the end of an
//! auction.
// Java: managers/ItemAuctionManager.java
// Java: entity/item/auction/ItemAuctionInstance.java
// Java: entity/item/auction/AuctionItem.java (createNewItemInstance)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{debug, error, info, warn};

use super::{
    AuctionItemDef, Bid, ExtendState, FINISH_TIME_SPACE, Instance, ItemAuction, ItemAuctionLoad,
    ItemAuctionRecord, ItemAuctionSettings, ItemAuctionState, ItemAuctionTimer, ItemAuctionWrite,
    START_TIME_SPACE, format_local_time,
};
use crate::outbox::{ItemRowOp, PersistCommand};
use crate::player::ItemData;
use crate::timer::{TimerKind, TimerWheel};
use crate::warehouse::ContainerRef;
use crate::world::{World, unix_ms_now};

/// `ItemLocation.WAREHOUSE`.
const LOC_WAREHOUSE: &str = "WAREHOUSE";

/// Why `loadAuction` gives `null` (Java then calls `ItemAuctionManager.deleteAuction`).
enum Rejected {
    /// "Invalid starting/ending paramaters", "AuctionItem: .. not found", "Invalid
    /// auctionStateId" or a finished auction past `AltItemAuctionExpiredAfter` ("Clearing expired
    /// auction").
    Invalid,
}

impl World {
    /// Installs the item auction configuration. Call before [`World::load_item_auctions`].
    pub fn set_item_auction_settings(&mut self, settings: ItemAuctionSettings) {
        self.item_auctions.settings = settings;
    }

    /// The settings the item auctions run with.
    pub fn item_auction_settings(&self) -> &ItemAuctionSettings {
        &self.item_auctions.settings
    }

    /// Java `ItemAuctionManager`'s constructor and the `ItemAuctionInstance` constructors: the
    /// instances of the XML, the auctions of their rows, then the current and the next auction
    /// of each instance. Needs the item templates and the id factory.
    pub fn load_item_auctions(&mut self, load: ItemAuctionLoad) {
        if !self.item_auctions.settings.enabled {
            info!("ItemAuctionManager: Disabled.");
            return;
        }
        self.item_auctions.next_auction_id = load.next_auction_id.max(1);
        let now = unix_ms_now();
        info!("ItemAuctionManager: Loaded {} instance(s).", load.instances.len());
        for def in load.instances {
            let instance = Instance {
                generator: def.generator,
                items: def.items,
                auctions: Default::default(),
                current: None,
                next: None,
                state_task: None,
            };
            if self.item_auctions.instances.insert(def.id, instance).is_some() {
                warn!("ItemAuctionManager: Dublicated instanceId {}", def.id);
            }
        }
        let mut dropped: Vec<i32> = Vec::new();
        for record in &load.auctions {
            match self.item_auction_from_record(record, now) {
                Ok(auction) => {
                    if let Some(instance) = self.item_auctions.instances.get_mut(&record.instance_id) {
                        instance.auctions.insert(auction.id, auction);
                    }
                }
                Err(Rejected::Invalid) => dropped.push(record.auction_id),
            }
        }
        for auction_id in dropped {
            // `ItemAuctionManager.deleteAuction`.
            self.item_auction_write(true, ItemAuctionWrite::DeleteAuction { auction_id });
        }
        let ids: Vec<i32> = self.item_auctions.instances.keys().copied().collect();
        for id in ids {
            let (items, auctions) = self
                .item_auctions
                .instances
                .get(&id)
                .map_or((0, 0), |i| (i.items.len(), i.auctions.len()));
            info!(
                "ItemAuctionInstance: Loaded {items} item(s) and registered {auctions} auction(s) for instance {id}."
            );
            self.check_and_set_current_and_next_auction(id, true);
        }
    }

    /// Sends a row write; a global one only with `CycleDatabaseWrites` (D-53).
    pub(crate) fn item_auction_write(&mut self, global: bool, write: ItemAuctionWrite) {
        if global && !self.item_auctions.settings.cycle_database_writes {
            debug!("Item auction: {write:?} stays in memory (CycleDatabaseWrites is off).");
            return;
        }
        self.outbox.persist(PersistCommand::ItemAuction(write));
    }

    /// `loadAuction` after the rows are read: Java's checks of the row.
    fn item_auction_from_record(
        &self,
        record: &ItemAuctionRecord,
        now: i64,
    ) -> Result<ItemAuction, Rejected> {
        let auction_id = record.auction_id;
        if record.starting_time >= record.ending_time {
            warn!("ItemAuctionInstance: Invalid starting/ending paramaters for auction: {auction_id}");
            return Err(Rejected::Invalid);
        }
        let Some(item) = self
            .item_auctions
            .instances
            .get(&record.instance_id)
            .and_then(|i| {
                i.items
                    .iter()
                    .find(|item| item.auction_item_id == record.auction_item_id)
            })
            .cloned()
        else {
            warn!(
                "ItemAuctionInstance: AuctionItem: {}, not found for auction: {auction_id}",
                record.auction_item_id
            );
            return Err(Rejected::Invalid);
        };
        let Some(state) = ItemAuctionState::from_id(record.state_id) else {
            warn!(
                "ItemAuctionInstance: Invalid auctionStateId: {}, for auction: {auction_id}",
                record.state_id
            );
            return Err(Rejected::Invalid);
        };
        if state == ItemAuctionState::Finished
            && record.starting_time < now.wrapping_sub(self.item_auctions.settings.expired_after_ms())
        {
            info!("ItemAuctionInstance: Clearing expired auction: {auction_id}");
            return Err(Rejected::Invalid);
        }
        let bids = record
            .bids
            .iter()
            .map(|&(player, last_bid)| Bid { player, last_bid })
            .collect();
        let object_id = self.allocate_item_id().map_or(0, |id| id.0);
        Ok(ItemAuction::new(
            auction_id,
            record.instance_id,
            record.starting_time,
            record.ending_time,
            item,
            bids,
            state,
            object_id,
        ))
    }

    /// Java `createAuction(after)`: a random item of the instance at the next start time. Its row
    /// goes out with `storeMe` (global). The auction is added to the instance.
    fn create_item_auction(&mut self, instance_id: i32, after: i64) -> Option<i32> {
        let count = {
            let instance = self.item_auctions.instances.get(&instance_id)?;
            i32::try_from(instance.items.len()).unwrap_or(i32::MAX)
        };
        if count == 0 {
            return None;
        }
        // `_items.get(Rnd.get(_items.size()))`
        let index = usize::try_from(self.rng().below(count)).unwrap_or(0);
        let (item, starting_time) = {
            let instance = self.item_auctions.instances.get(&instance_id)?;
            (
                instance.items.get(index)?.clone(),
                instance.generator.next_date(after),
            )
        };
        let ending_time = starting_time.wrapping_add(i64::from(item.auction_length) * 60_000);
        let id = self.item_auctions.next_auction_id;
        self.item_auctions.next_auction_id = id.wrapping_add(1);
        let object_id = self.allocate_item_id().map_or(0, |id| id.0);
        let mut auction = ItemAuction::new(
            id,
            instance_id,
            starting_time,
            ending_time,
            item,
            Vec::new(),
            ItemAuctionState::Created,
            object_id,
        );
        auction.local_only = !self.item_auctions.settings.cycle_database_writes;
        self.store_item_auction(&auction);
        self.item_auctions
            .instances
            .get_mut(&instance_id)?
            .auctions
            .insert(id, auction);
        Some(id)
    }

    /// Java `ItemAuction.storeMe()`.
    fn store_item_auction(&mut self, auction: &ItemAuction) {
        self.item_auction_write(
            true,
            ItemAuctionWrite::Store {
                auction_id: auction.id,
                instance_id: auction.instance_id,
                auction_item_id: auction.item.auction_item_id,
                starting_time: auction.starting_time,
                ending_time: auction.ending_time,
                state_id: auction.state.state_id(),
            },
        );
    }

    /// Java `checkAndSetCurrentAndNextAuction()`. `announce` logs the schedule line at info level
    /// (the load); the re-reads of the side by side mode log it at debug level.
    pub(crate) fn check_and_set_current_and_next_auction(&mut self, instance_id: i32, announce: bool) {
        let now = unix_ms_now();
        let Some(instance) = self.item_auctions.instances.get(&instance_id) else {
            return;
        };
        // Java's `_auctions.values()`: a `HashMap` of ints, so the ids' order.
        let mut auctions: Vec<(i32, ItemAuctionState, i64, i64)> = instance
            .auctions
            .values()
            .map(|a| (a.id, a.state, a.starting_time, a.ending_time))
            .collect();

        let (current, next) = match auctions.len() {
            0 => (
                None,
                self.create_item_auction(instance_id, now + START_TIME_SPACE),
            ),
            1 => {
                let (id, state, starting_time, ending_time) = auctions[0];
                match state {
                    ItemAuctionState::Created => {
                        if starting_time < now + START_TIME_SPACE {
                            (
                                Some(id),
                                self.create_item_auction(instance_id, now + START_TIME_SPACE),
                            )
                        } else {
                            (None, Some(id))
                        }
                    }
                    ItemAuctionState::Started => (
                        Some(id),
                        self.create_item_auction(
                            instance_id,
                            (ending_time + FINISH_TIME_SPACE).max(now + START_TIME_SPACE),
                        ),
                    ),
                    ItemAuctionState::Finished => (
                        Some(id),
                        self.create_item_auction(instance_id, now + START_TIME_SPACE),
                    ),
                }
            }
            _ => {
                // Newest first; equal start times keep the ids' order.
                auctions.sort_by_key(|a| std::cmp::Reverse(a.2));
                // Just to make sure we won't skip any auction because of little different times.
                let current = auctions
                    .iter()
                    .find(|a| a.1 == ItemAuctionState::Started || a.2 <= now)
                    .map(|a| a.0);
                let next = auctions
                    .iter()
                    .find(|a| a.2 > now && Some(a.0) != current)
                    .map(|a| a.0);
                let next = match next {
                    Some(next) => Some(next),
                    None => self.create_item_auction(instance_id, now + START_TIME_SPACE),
                };
                (current, next)
            }
        };
        let Some(next) = next else {
            warn!("ItemAuctionInstance: instance {instance_id} has no auction to schedule.");
            return;
        };

        let (current_state, current_times, next_start) = {
            let Some(instance) = self.item_auctions.instances.get_mut(&instance_id) else {
                return;
            };
            instance.current = current;
            instance.next = Some(next);
            (
                current.and_then(|c| instance.auctions.get(&c)).map(|a| a.state),
                current
                    .and_then(|c| instance.auctions.get(&c))
                    .map(|a| (a.starting_time, a.ending_time)),
                instance.auctions.get(&next).map_or(0, |a| a.starting_time),
            )
        };

        match (current, current_state, current_times) {
            (Some(current), Some(state), Some((starting_time, ending_time)))
                if state != ItemAuctionState::Finished =>
            {
                let at = if state == ItemAuctionState::Started {
                    ending_time
                } else {
                    starting_time
                };
                self.set_item_auction_state_task(instance_id, current, (at - now).max(0));
                if announce {
                    info!(
                        "ItemAuctionInstance: Schedule current auction {current} for instance {instance_id}"
                    );
                } else {
                    debug!(
                        "ItemAuctionInstance: Schedule current auction {current} for instance {instance_id}"
                    );
                }
            }
            _ => {
                self.set_item_auction_state_task(instance_id, next, (next_start - now).max(0));
                if announce {
                    info!(
                        "ItemAuctionInstance: Schedule next auction {next} on {} for instance {instance_id}",
                        format_local_time(next_start)
                    );
                } else {
                    debug!(
                        "ItemAuctionInstance: Schedule next auction {next} on {} for instance {instance_id}",
                        format_local_time(next_start)
                    );
                }
            }
        }
    }

    /// Java `setStateTask(ThreadPool.schedule(new ScheduleAuctionTask(auction), delay))`: the old
    /// task is canceled.
    pub(crate) fn set_item_auction_state_task(&mut self, instance_id: i32, auction_id: i32, delay_ms: i64) {
        let ticks = TimerWheel::ms_to_ticks(u64::try_from(delay_ms.max(0)).unwrap_or(0));
        let timer = self.timers.schedule_at(
            self.tick + ticks,
            TimerKind::ItemAuction(ItemAuctionTimer::State {
                instance: instance_id,
                auction: auction_id,
            }),
        );
        if let Some(instance) = self.item_auctions.instances.get_mut(&instance_id)
            && let Some(old) = instance.state_task.replace(timer)
        {
            self.timers.cancel(old);
        }
    }

    pub(crate) fn on_item_auction_timer(&mut self, timer: ItemAuctionTimer) {
        match timer {
            ItemAuctionTimer::State { instance, auction } => {
                self.item_auction_schedule_task(instance, auction)
            }
        }
    }

    /// Java `ScheduleAuctionTask.runImpl`.
    fn item_auction_schedule_task(&mut self, instance_id: i32, auction_id: i32) {
        let Some((state, extend, scheduled)) = self
            .item_auctions
            .instances
            .get(&instance_id)
            .and_then(|i| i.auctions.get(&auction_id))
            .map(|a| (a.state, a.extend, a.scheduled_extend))
        else {
            // The auction went away with a re-read.
            return;
        };
        match state {
            ItemAuctionState::Created => {
                if !self.set_item_auction_state(
                    instance_id,
                    auction_id,
                    ItemAuctionState::Created,
                    ItemAuctionState::Started,
                ) {
                    error!(
                        "ItemAuctionInstance: Failed scheduling auction {auction_id}: Could not set auction state: Started, expected: Created"
                    );
                    return;
                }
                info!("ItemAuctionInstance: Auction {auction_id} has started for instance {instance_id}");
                self.check_and_set_current_and_next_auction(instance_id, false);
            }
            ItemAuctionState::Started => {
                // The auction was extended: the task runs again at the new end.
                let again = match extend {
                    ExtendState::ExtendBy5Min if scheduled == ExtendState::Initial => {
                        Some(ExtendState::ExtendBy5Min)
                    }
                    ExtendState::ExtendBy3Min if scheduled != ExtendState::ExtendBy3Min => {
                        Some(ExtendState::ExtendBy3Min)
                    }
                    ExtendState::ExtendByConfigPhaseA if scheduled != ExtendState::ExtendByConfigPhaseB => {
                        Some(ExtendState::ExtendByConfigPhaseB)
                    }
                    ExtendState::ExtendByConfigPhaseB if scheduled != ExtendState::ExtendByConfigPhaseA => {
                        Some(ExtendState::ExtendByConfigPhaseA)
                    }
                    _ => None,
                };
                if let Some(scheduled) = again {
                    let ending_time = self
                        .item_auctions
                        .instances
                        .get_mut(&instance_id)
                        .and_then(|i| i.auctions.get_mut(&auction_id))
                        .map_or(0, |a| {
                            a.scheduled_extend = scheduled;
                            a.ending_time
                        });
                    self.set_item_auction_state_task(
                        instance_id,
                        auction_id,
                        (ending_time - unix_ms_now()).max(0),
                    );
                    return;
                }
                if !self.set_item_auction_state(
                    instance_id,
                    auction_id,
                    ItemAuctionState::Started,
                    ItemAuctionState::Finished,
                ) {
                    error!(
                        "ItemAuctionInstance: Failed scheduling auction {auction_id}: Could not set auction state: Finished, expected: Started"
                    );
                    return;
                }
                self.on_item_auction_finished(instance_id, auction_id);
                self.check_and_set_current_and_next_auction(instance_id, false);
            }
            ItemAuctionState::Finished => {
                error!(
                    "ItemAuctionInstance: Failed scheduling auction {auction_id}: Invalid state: Finished"
                );
            }
        }
    }

    /// Java `ItemAuction.setAuctionState(expected, wanted)`: the new state is written (`storeMe`).
    fn set_item_auction_state(
        &mut self,
        instance_id: i32,
        auction_id: i32,
        expected: ItemAuctionState,
        wanted: ItemAuctionState,
    ) -> bool {
        let Some(auction) = self
            .item_auctions
            .instances
            .get_mut(&instance_id)
            .and_then(|i| i.auctions.get_mut(&auction_id))
        else {
            return false;
        };
        if auction.state != expected {
            return false;
        }
        auction.state = wanted;
        let auction = auction.clone();
        self.store_item_auction(&auction);
        true
    }

    /// The online bidders of an auction, in Java's order for `broadcastToAllBidders` (the bid list
    /// from the end).
    pub(crate) fn item_auction_online_bidders(&self, instance_id: i32, auction_id: i32) -> Vec<ObjectId> {
        let Some(auction) = self
            .item_auctions
            .instances
            .get(&instance_id)
            .and_then(|i| i.auctions.get(&auction_id))
        else {
            return Vec::new();
        };
        auction
            .bids
            .iter()
            .rev()
            .map(|b| ObjectId(b.player))
            .filter(|p| self.players.contains_key(p))
            .collect()
    }

    /// Java `ItemAuction.broadcastToAllBiddersInternal`.
    pub(crate) fn item_auction_broadcast(&self, instance_id: i32, auction_id: i32, packet: &SystemMessage) {
        for player in self.item_auction_online_bidders(instance_id, auction_id) {
            self.send(player, packet);
        }
    }

    /// Java `onAuctionFinished(auction)`.
    ///
    /// The "auction has ended" message goes to the online bidders in both modes. The winner's item
    /// and the clearing of the canceled bids are the Java GS's job while `CycleDatabaseWrites` is
    /// off (D-53): it reads the same bid rows.
    fn on_item_auction_finished(&mut self, instance_id: i32, auction_id: i32) {
        self.item_auction_broadcast(
            instance_id,
            auction_id,
            &SystemMessage::new(SystemMessageId::S1SAuctionHasEnded)
                .with(SystemMessageParam::IntNumber(auction_id)),
        );
        if !self.item_auctions.settings.cycle_database_writes {
            info!(
                "ItemAuctionInstance: Auction {auction_id} has finished for instance {instance_id}; the winner's item is left to the Java game server (CycleDatabaseWrites is off)."
            );
            return;
        }
        let Some((highest, def)) = self
            .item_auctions
            .instances
            .get(&instance_id)
            .and_then(|i| i.auctions.get(&auction_id))
            .map(|a| (a.highest, a.item.clone()))
        else {
            return;
        };
        let Some(winner) = highest else {
            info!(
                "ItemAuctionInstance: Auction {auction_id} has finished. There have not been any bid for instance {instance_id}"
            );
            return;
        };
        match self.create_auction_item(&def) {
            Some(item) => {
                let winner_id = ObjectId(winner);
                if self.players.contains_key(&winner_id) {
                    self.item_auction_add_to_warehouse(winner_id, item);
                    self.send(
                        winner_id,
                        &SystemMessage::new(
                            SystemMessageId::YouHaveBidTheHighestPriceAndHaveWonTheItemTheItemCanBeFoundInYourPersonalWarehouse,
                        ),
                    );
                    info!(
                        "ItemAuctionInstance: Auction {auction_id} has finished. Highest bid by {} for instance {instance_id}",
                        self.creature_name(winner_id)
                    );
                } else {
                    // `item.setOwnerId(..); setItemLocation(WAREHOUSE); updateDatabase()`.
                    let mut item = item;
                    item.loc = LOC_WAREHOUSE.to_owned();
                    item.loc_data = 0;
                    self.outbox.persist(PersistCommand::ItemRows {
                        tracked: None,
                        ops: vec![ItemRowOp::Insert {
                            owner_id: winner,
                            item,
                        }],
                    });
                    info!(
                        "ItemAuctionInstance: Auction {auction_id} has finished. Highest bid by character {winner} for instance {instance_id}"
                    );
                }
            }
            None => warn!(
                "ItemAuctionInstance: Auction {auction_id} has finished, but the item {} could not be created.",
                def.item_id
            ),
        }
        // Clean all canceled bids.
        self.item_auction_clear_canceled_bids(instance_id, auction_id);
    }

    /// Java `ItemAuction.clearCanceledBids()`.
    fn item_auction_clear_canceled_bids(&mut self, instance_id: i32, auction_id: i32) {
        let canceled: Vec<i32> = self
            .item_auctions
            .instances
            .get(&instance_id)
            .and_then(|i| i.auctions.get(&auction_id))
            .map(|a| {
                a.bids
                    .iter()
                    .filter(|b| b.is_canceled())
                    .map(|b| b.player)
                    .collect()
            })
            .unwrap_or_default();
        for player_obj_id in canceled {
            self.item_auction_write(
                false,
                ItemAuctionWrite::DeleteBid {
                    auction_id,
                    player_obj_id,
                },
            );
        }
    }

    /// Java `AuctionItem.createNewItemInstance()`: a new item of the template with the default
    /// enchant level and the augmentation of `<extra>`. `None` without a template or an id.
    pub(crate) fn create_auction_item(&self, def: &AuctionItemDef) -> Option<ItemData> {
        let id = self.allocate_item_id()?;
        Some(self.auction_item_data(def, id))
    }

    /// The item as `createNewItemInstance` makes it, with the given object id.
    pub(crate) fn auction_item_data(&self, def: &AuctionItemDef, object_id: ObjectId) -> ItemData {
        let template = self
            .item_templates()
            .and_then(|templates| templates.get(def.item_id).cloned());
        let mut item = match &template {
            Some(template) => self.new_item_data(template, object_id, def.item_count),
            None => ItemData {
                object_id,
                item_id: def.item_id,
                count: def.item_count,
                ..ItemData::default()
            },
        };
        item.enchant_level = template.as_ref().map_or(0, |t| t.default_enchant_level);
        if def.augmentation_id > 0 {
            item.augmentation = Some(def.augmentation_id);
        }
        item
    }

    /// Java `PlayerWarehouse.addItem(BUY, item, null, null)`: a stackable item joins the stack the
    /// warehouse has, anything else is added. The rows go out as item rows (D-39).
    fn item_auction_add_to_warehouse(&mut self, player: ObjectId, mut item: ItemData) {
        let container = ContainerRef::Private(player);
        let owner_id = player.0;
        let stackable = self.data.item_stackable(item.item_id);
        let existing = if stackable {
            self.container_items(container)
                .and_then(|items| items.iter().find(|i| i.item_id == item.item_id))
                .map(|i| i.object_id)
        } else {
            None
        };
        let op = match existing {
            Some(existing) => {
                let Some(items) = self.container_items_mut(container) else {
                    return;
                };
                let Some(stack) = items.iter_mut().find(|i| i.object_id == existing) else {
                    return;
                };
                stack.count = stack.count.wrapping_add(item.count);
                let stack = stack.clone();
                // The new item is destroyed, its id is free again.
                self.release_item_id(item.object_id);
                ItemRowOp::Update {
                    owner_id,
                    item: stack,
                }
            }
            None => {
                item.loc = LOC_WAREHOUSE.to_owned();
                item.loc_data = 0;
                let Some(items) = self.container_items_mut(container) else {
                    return;
                };
                items.push(item.clone());
                ItemRowOp::Insert { owner_id, item }
            }
        };
        self.outbox.persist(PersistCommand::ItemRows {
            tracked: None,
            ops: vec![op],
        });
    }
}
