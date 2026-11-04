//! The client side of the item auctions: the two packets, the `ItemAuction` bypass of the
//! auctioneer NPC and `ExItemAuctionInfo`.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExItemAuctionInfo, ItemAuctionNext, ItemInfo, SystemMessage};
use l2_util::ObjectId;

use super::{ItemAuction, ItemAuctionState, format_local_time};
use crate::flood::FloodKind;
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
        let next = next
            .and_then(|n| instance.auctions.get(&n))
            .map(|next| ItemAuctionNext {
                init_bid: next.item.auction_init_bid,
                start_secs: (next.starting_time / 1000) as i32,
                item: self.item_auction_item_info(next),
            });
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
            self.item_auction_show_now(player, instance_id);
        } else if cmd.eq_ignore_ascii_case("cancel") {
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
