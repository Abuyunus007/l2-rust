//! The client side of the item auctions: the two packets, the `ItemAuction` bypass of the
//! auctioneer NPC, `ExItemAuctionInfo`, and the re-read of the shared rows (D-50, D-53).
// Java: network/clientpackets/{RequestInfoItemAuction,RequestBidItemAuction}.java
// Java: network/serverpackets/ExItemAuctionInfoPacket.java
// Java: dist/game/data/scripts/handlers/bypass/npc/ItemAuctionLink.java
// Java: entity/actor/Player.java (updateLastItemAuctionRequest, isItemAuctionPolling)

use std::collections::BTreeSet;

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExItemAuctionInfo, ItemAuctionNext, ItemInfo, SystemMessage};
use l2_util::ObjectId;
use tracing::{debug, info, warn};

use super::{ItemAuction, ItemAuctionRecord, ItemAuctionState, ItemAuctionThen, format_local_time};
use crate::flood::FloodKind;
use crate::outbox::PersistCommand;
use crate::world::{World, unix_ms_now};

/// A client packet of the item auctions, with the raw wire values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ItemAuctionInput {
    /// Java `RequestInfoItemAuction`.
    Info { instance_id: i32 },
    /// Java `RequestBidItemAuction`.
    Bid { instance_id: i32, bid: i64 },
}

impl World {
    /// Whether the rows of the instance have to be read again before they are shown or used
    /// (D-50): the Java game server writes them while `CycleDatabaseWrites` is off (D-53).
    fn item_auction_needs_reread(&self) -> bool {
        !self.item_auctions.settings.cycle_database_writes
    }

    fn request_item_auction_reread(&mut self, instance_id: i32, then: ItemAuctionThen) {
        self.outbox
            .persist(PersistCommand::ItemAuctionRequest { instance_id, then });
    }

    pub(crate) fn on_item_auction_input(&mut self, player: ObjectId, input: ItemAuctionInput) {
        match input {
            ItemAuctionInput::Info { instance_id } => self.on_request_info_item_auction(player, instance_id),
            ItemAuctionInput::Bid { instance_id, bid } => {
                self.on_request_bid_item_auction(player, instance_id, bid)
            }
        }
    }

    /// Java `RequestInfoItemAuction.runImpl`.
    fn on_request_info_item_auction(&mut self, player: ObjectId, instance_id: i32) {
        if !self.players.contains_key(&player) {
            return;
        }
        if !self.flood_allows(player, FloodKind::ItemAuction) {
            return;
        }
        if !self.item_auctions.instances.contains_key(&instance_id) {
            return;
        }
        if self.item_auction_needs_reread() {
            self.request_item_auction_reread(instance_id, ItemAuctionThen::Info { player });
            return;
        }
        self.item_auction_info_now(player, instance_id);
    }

    fn item_auction_info_now(&mut self, player: ObjectId, instance_id: i32) {
        let Some(instance) = self.item_auctions.instances.get(&instance_id) else {
            return;
        };
        let Some(current) = instance.current else {
            return;
        };
        let next = instance.next;
        self.item_auction_update_last_request(player);
        if let Some(packet) = self.item_auction_info_packet(true, instance_id, current, next) {
            self.send(player, &packet);
        }
    }

    /// Java `RequestBidItemAuction.runImpl`.
    fn on_request_bid_item_auction(&mut self, player: ObjectId, instance_id: i32, bid: i64) {
        if !self.players.contains_key(&player) {
            return;
        }
        // Can't perform transaction here.
        if !self.flood_allows(player, FloodKind::Transaction) {
            self.send_message(player, "You are bidding too fast.");
            return;
        }
        if bid < 0 || bid > self.settings.ground.max_adena {
            return;
        }
        if !self.item_auctions.instances.contains_key(&instance_id) {
            return;
        }
        if self.item_auction_needs_reread() {
            self.request_item_auction_reread(instance_id, ItemAuctionThen::Bid { player, bid });
            return;
        }
        self.item_auction_bid_now(player, instance_id, bid);
    }

    fn item_auction_bid_now(&mut self, player: ObjectId, instance_id: i32, bid: i64) {
        let Some(current) = self
            .item_auctions
            .instances
            .get(&instance_id)
            .and_then(|i| i.current)
        else {
            return;
        };
        self.item_auction_register_bid(instance_id, current, player, bid);
    }

    /// Java `Player.updateLastItemAuctionRequest()`.
    fn item_auction_update_last_request(&mut self, player: ObjectId) {
        let now = unix_ms_now();
        let requests = &mut self.item_auctions.last_request;
        if requests.len() > 256 {
            requests.retain(|_, at| now - *at < 2000);
        }
        requests.insert(player, now);
    }

    /// Java `Player.isItemAuctionPolling()`: the last request was less than 2 seconds ago.
    fn item_auction_is_polling(&self, player: ObjectId) -> bool {
        self.item_auctions
            .last_request
            .get(&player)
            .is_some_and(|at| unix_ms_now() - *at < 2000)
    }

    /// Java `new ExItemAuctionInfoPacket(refresh, current, next)`.
    fn item_auction_info_packet(
        &self,
        refresh: bool,
        instance_id: i32,
        current: i32,
        next: Option<i32>,
    ) -> Option<ExItemAuctionInfo> {
        let instance = self.item_auctions.instances.get(&instance_id)?;
        let current = instance.auctions.get(&current)?;
        let time_remaining = if current.state != ItemAuctionState::Started {
            0
        } else {
            // `getFinishingTimeRemaining() / 1000`, as an int.
            ((current.ending_time - unix_ms_now()).max(0) / 1000) as i32
        };
        let item = self.item_auction_item_info(current);
        let next = match next.and_then(|n| instance.auctions.get(&n)) {
            Some(next) => Some(ItemAuctionNext {
                init_bid: next.item.auction_init_bid,
                start_secs: (next.starting_time / 1000) as i32,
                item: self.item_auction_item_info(next),
            }),
            None => None,
        };
        Some(ExItemAuctionInfo {
            refresh,
            instance_id: current.instance_id,
            bid: current
                .highest_bid_value()
                .unwrap_or(current.item.auction_init_bid),
            time_remaining,
            item,
            next,
        })
    }

    /// `new ItemInfo(item)` of the item the auction sells.
    fn item_auction_item_info(&self, auction: &ItemAuction) -> ItemInfo {
        let item = self.auction_item_data(&auction.item, ObjectId(auction.item_object_id));
        let stat = self
            .data
            .item(item.item_id)
            .unwrap_or_else(|| crate::data::ItemStatic::unknown(item.item_id));
        let templates = self.item_templates();
        let template = templates.as_ref().and_then(|t| t.get(item.item_id));
        crate::enter::item_info(&item, &stat, template, unix_ms_now())
    }

    // ------------------------------------------------------------------ the bypass

    /// Java `ItemAuctionLink.onCommand(command, player, target)`.
    pub(crate) fn item_auction_link(
        &mut self,
        player: ObjectId,
        command: &str,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(npc) = target.filter(|t| self.npcs.contains_key(t)) else {
            return false;
        };
        if !self.item_auctions.settings.enabled {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ItIsNotAnAuctionPeriod),
            );
            return true;
        }
        let Some(instance_id) = self.npcs.get(&npc).map(|n| n.npc_id) else {
            return false;
        };
        if !self.item_auctions.instances.contains_key(&instance_id) {
            return false;
        }
        // `new StringTokenizer(command)`: the whitespace separated words.
        let mut words = command.split_whitespace();
        words.next(); // bypass "ItemAuction"
        let Some(cmd) = words.next() else {
            return false;
        };
        if cmd.eq_ignore_ascii_case("show") {
            if !self.flood_allows(player, FloodKind::ItemAuction) {
                return false;
            }
            if self.item_auction_is_polling(player) {
                return false;
            }
            if self.item_auction_needs_reread() {
                self.request_item_auction_reread(instance_id, ItemAuctionThen::Show { player });
                return true;
            }
            self.item_auction_show_now(player, instance_id);
        } else if cmd.eq_ignore_ascii_case("cancel") {
            if self.item_auction_needs_reread() {
                self.request_item_auction_reread(instance_id, ItemAuctionThen::Cancel { player });
                return true;
            }
            self.item_auction_cancel_now(player, instance_id);
        } else {
            return false;
        }
        true
    }

    fn item_auction_show_now(&mut self, player: ObjectId, instance_id: i32) {
        let Some(instance) = self.item_auctions.instances.get(&instance_id) else {
            return;
        };
        let (current, next) = (instance.current, instance.next);
        let Some(current) = current else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ItIsNotAnAuctionPeriod),
            );
            if let Some(next) = next.and_then(|n| instance.auctions.get(&n)) {
                let text = format!(
                    "The next auction will begin on the {}.",
                    format_local_time(next.starting_time)
                );
                self.send_message(player, &text);
            }
            return;
        };
        if let Some(packet) = self.item_auction_info_packet(false, instance_id, current, next) {
            self.send(player, &packet);
        }
    }

    fn item_auction_cancel_now(&mut self, player: ObjectId, instance_id: i32) {
        // `getAuctionsByBidder(playerObjId)`: not created, and the player has a bid.
        let player_obj_id = player.0;
        let ids: Vec<i32> = self
            .item_auctions
            .instances
            .get(&instance_id)
            .map(|i| {
                i.auctions
                    .values()
                    .filter(|a| a.state != ItemAuctionState::Created && a.bid_for(player_obj_id).is_some())
                    .map(|a| a.id)
                    .collect()
            })
            .unwrap_or_default();
        let mut returned = false;
        for auction_id in ids {
            if self.item_auction_cancel_bid(instance_id, auction_id, player) {
                returned = true;
            }
        }
        if !returned {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThereAreNoOfferingsIOwnOrIMadeABidFor),
            );
        }
    }

    // ------------------------------------------------------------------ the re-read

    /// The answer to `PersistCommand::ItemAuctionRequest`: the rows of the instance as they are in
    /// the database now, or `Err` when the read failed (the action is then refused, D-50).
    pub(crate) fn on_item_auction_reread(
        &mut self,
        instance_id: i32,
        result: Result<Vec<ItemAuctionRecord>, ()>,
        then: ItemAuctionThen,
    ) {
        let player = match then {
            ItemAuctionThen::Info { player }
            | ItemAuctionThen::Show { player }
            | ItemAuctionThen::Bid { player, .. }
            | ItemAuctionThen::Cancel { player } => player,
        };
        let Ok(rows) = result else {
            warn!(
                "ItemAuctionInstance: the auctions of instance {instance_id} could not be read; the request of {player} is refused."
            );
            return;
        };
        self.apply_item_auction_rows(instance_id, rows);
        if !self.players.contains_key(&player) {
            return;
        }
        match then {
            ItemAuctionThen::Info { .. } => self.item_auction_info_now(player, instance_id),
            ItemAuctionThen::Show { .. } => self.item_auction_show_now(player, instance_id),
            ItemAuctionThen::Bid { bid, .. } => self.item_auction_bid_now(player, instance_id, bid),
            ItemAuctionThen::Cancel { .. } => self.item_auction_cancel_now(player, instance_id),
        }
    }

    /// Brings the auctions of an instance up to the rows the database has (only with
    /// `CycleDatabaseWrites` off, where the Java game server writes the states):
    /// - the state never goes back, the end never gets earlier (this server's own timers may be
    ///   ahead, and an extension is not stored),
    /// - the bids are the rows' (this server's own bids were written before the read),
    /// - an auction the rows do not have any more is dropped, unless this server made it up,
    /// - an auction this server does not know is added.
    /// Then the current and the next auction are chosen again.
    pub(crate) fn apply_item_auction_rows(&mut self, instance_id: i32, rows: Vec<ItemAuctionRecord>) {
        let now = unix_ms_now();
        let mut seen: BTreeSet<i32> = BTreeSet::new();
        for record in rows {
            let known = self
                .item_auctions
                .instances
                .get(&instance_id)
                .is_some_and(|i| i.auctions.contains_key(&record.auction_id));
            let Some(state) = ItemAuctionState::from_id(record.state_id) else {
                warn!(
                    "ItemAuctionInstance: Invalid auctionStateId: {}, for auction: {}",
                    record.state_id, record.auction_id
                );
                continue;
            };
            if known {
                if let Some(auction) = self
                    .item_auctions
                    .instances
                    .get_mut(&instance_id)
                    .and_then(|i| i.auctions.get_mut(&record.auction_id))
                {
                    auction.state = auction.state.max(state);
                    auction.ending_time = auction.ending_time.max(record.ending_time);
                    auction.bids = record
                        .bids
                        .iter()
                        .map(|&(player, last_bid)| super::Bid { player, last_bid })
                        .collect();
                    auction.find_highest();
                    auction.local_only = false;
                }
                seen.insert(record.auction_id);
                continue;
            }
            // A row this server does not know (a new auction of the Java game server).
            if record.starting_time >= record.ending_time {
                warn!(
                    "ItemAuctionInstance: Invalid starting/ending paramaters for auction: {}",
                    record.auction_id
                );
                continue;
            }
            if state == ItemAuctionState::Finished
                && record.starting_time < now.wrapping_sub(self.item_auctions.settings.expired_after_ms())
            {
                continue;
            }
            let Some(item) = self
                .item_auctions
                .instances
                .get(&instance_id)
                .and_then(|i| {
                    i.items
                        .iter()
                        .find(|i| i.auction_item_id == record.auction_item_id)
                })
                .cloned()
            else {
                warn!(
                    "ItemAuctionInstance: AuctionItem: {}, not found for auction: {}",
                    record.auction_item_id, record.auction_id
                );
                continue;
            };
            let object_id = self.allocate_item_id().map_or(0, |id| id.0);
            let auction = ItemAuction::new(
                record.auction_id,
                instance_id,
                record.starting_time,
                record.ending_time,
                item,
                record
                    .bids
                    .iter()
                    .map(|&(player, last_bid)| super::Bid { player, last_bid })
                    .collect(),
                state,
                object_id,
            );
            if let Some(instance) = self.item_auctions.instances.get_mut(&instance_id) {
                instance.auctions.insert(auction.id, auction);
            }
            seen.insert(record.auction_id);
            // The ids of this server come after the ones in the table.
            if record.auction_id >= self.item_auctions.next_auction_id {
                self.item_auctions.next_auction_id = record.auction_id.wrapping_add(1);
            }
        }
        if let Some(instance) = self.item_auctions.instances.get_mut(&instance_id) {
            let before = instance.auctions.len();
            instance
                .auctions
                .retain(|id, a| seen.contains(id) || a.local_only);
            if instance.auctions.len() != before {
                debug!(
                    "ItemAuctionInstance: instance {instance_id} lost auctions the database no longer has."
                );
            }
        }
        info!(
            "ItemAuctionInstance: instance {instance_id} read again: {} auction(s).",
            self.item_auctions
                .instances
                .get(&instance_id)
                .map_or(0, |i| i.auctions.len())
        );
        self.check_and_set_current_and_next_auction(instance_id, false);
    }
}

/// Java `ItemAuctionLink` (`COMMANDS = { "ItemAuction" }`).
pub(crate) fn item_auction_link_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    target: Option<ObjectId>,
) -> bool {
    world.item_auction_link(player, command, target)
}
