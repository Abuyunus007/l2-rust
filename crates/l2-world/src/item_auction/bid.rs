//! Bidding and taking a canceled bid back.
// Java: entity/item/auction/ItemAuction.java (registerBid, onPlayerBid, cancelBid,
//       reduceItemCount, increaseItemCount)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::{
    Bid, ENDING_TIME_EXTEND_3, ENDING_TIME_EXTEND_5, ExtendState, ItemAuctionState, ItemAuctionWrite, MAX_BID,
};
use crate::world::{World, unix_ms_now};

impl World {
    /// Java `reduceItemCount(player, count)`: the adena of the bid.
    ///
    /// NOTE(java-quirk): `reduceAdena` already tells the player there is not enough adena, then
    /// this sends the message of the bid, and `registerBid` sends it a third time.
    fn item_auction_reduce_item_count(&mut self, player: ObjectId, count: i64) -> bool {
        if self.reduce_adena(player, count, true) {
            return true;
        }
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdenaForThisBid),
        );
        false
    }

    /// Java `registerBid(player, newBid)` of the running auction of an instance.
    pub(crate) fn item_auction_register_bid(
        &mut self,
        instance_id: i32,
        auction_id: i32,
        player: ObjectId,
        new_bid: i64,
    ) {
        let Some((init_bid, state)) = self
            .item_auctions
            .instances
            .get(&instance_id)
            .and_then(|i| i.auctions.get(&auction_id))
            .map(|a| (a.item.auction_init_bid, a.state))
        else {
            return;
        };
        if new_bid < init_bid {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YourBidPriceMustBeHigherThanTheMinimumPriceCurrentlyBeingBid,
                ),
            );
            return;
        }
        if new_bid > MAX_BID {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::BiddingIsNotAllowedBecauseTheMaximumBiddingPriceExceeds100Billion,
                ),
            );
            return;
        }
        if state != ItemAuctionState::Started {
            return;
        }

        let player_obj_id = player.0;
        let (highest, own) = {
            let Some(auction) = self
                .item_auctions
                .instances
                .get(&instance_id)
                .and_then(|i| i.auctions.get(&auction_id))
            else {
                return;
            };
            (
                auction.highest_bid_value(),
                auction
                    .bid_for(player_obj_id)
                    .map(|b| (b.last_bid, b.is_canceled())),
            )
        };
        if let Some(highest) = highest
            && new_bid < highest
        {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YourBidMustBeHigherThanTheCurrentHighestBid),
            );
            return;
        }

        let not_enough = |world: &World| {
            world.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughAdenaForThisBid),
            );
        };
        match own {
            None => {
                if !self.item_auction_reduce_item_count(player, new_bid) {
                    not_enough(self);
                    return;
                }
                if let Some(auction) = self.item_auction_mut(instance_id, auction_id) {
                    auction.bids.push(Bid {
                        player: player_obj_id,
                        last_bid: new_bid,
                    });
                }
            }
            Some((last_bid, canceled)) => {
                if !canceled {
                    if new_bid < last_bid {
                        // just another check
                        self.send(
                            player,
                            &SystemMessage::new(SystemMessageId::YourBidMustBeHigherThanTheCurrentHighestBid),
                        );
                        return;
                    }
                    if !self.item_auction_reduce_item_count(player, new_bid - last_bid) {
                        not_enough(self);
                        return;
                    }
                } else if !self.item_auction_reduce_item_count(player, new_bid) {
                    not_enough(self);
                    return;
                }
                if let Some(bid) = self
                    .item_auction_mut(instance_id, auction_id)
                    .and_then(|a| a.bid_for_mut(player_obj_id))
                {
                    bid.last_bid = new_bid;
                }
            }
        }

        let extension = self.item_auction_on_player_bid(instance_id, auction_id, player);
        // `updatePlayerBid(bid, false)`: a per-player row, written in both modes.
        self.item_auction_write(
            false,
            ItemAuctionWrite::Bid {
                auction_id,
                player_obj_id,
                bid: new_bid,
            },
        );
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveSubmittedABidForTheAuctionOfS1)
                .with(SystemMessageParam::LongNumber(new_bid)),
        );
        // Java hands this to a thread pool task, which runs after the message above.
        if let Some(id) = extension {
            self.item_auction_broadcast(instance_id, auction_id, &SystemMessage::new(id));
        }
    }

    fn item_auction_mut(&mut self, instance_id: i32, auction_id: i32) -> Option<&mut super::ItemAuction> {
        self.item_auctions
            .instances
            .get_mut(&instance_id)?
            .auctions
            .get_mut(&auction_id)
    }

    /// Java `onPlayerBid(player, bid)`: the new highest bid, the outbid message and the extension
    /// of the end. Returns the message to broadcast to the bidders, if the end was extended.
    fn item_auction_on_player_bid(
        &mut self,
        instance_id: i32,
        auction_id: i32,
        player: ObjectId,
    ) -> Option<SystemMessageId> {
        let extends_on_bid = self.item_auctions.settings.time_extends_on_bid_ms;
        let now = unix_ms_now();
        let player_obj_id = player.0;
        let mut outbid: Option<ObjectId> = None;
        let mut message = None;
        {
            let auction = self.item_auction_mut(instance_id, auction_id)?;
            let new_value = auction.bid_for(player_obj_id).map_or(0, |b| b.last_bid);
            match auction.highest_bid_value() {
                None => auction.highest = Some(player_obj_id),
                Some(old_value) if old_value < new_value => {
                    outbid = auction.highest.map(ObjectId);
                    auction.highest = Some(player_obj_id);
                }
                Some(_) => {}
            }

            if auction.ending_time - now <= 1000 * 60 * 10 {
                // 10 minutes
                // `getAndSetLastBidPlayerObjectId(..) != player`
                let other_bidder = |auction: &mut super::ItemAuction| {
                    let last = auction.last_bid_player;
                    auction.last_bid_player = player_obj_id;
                    last != player_obj_id
                };
                match auction.extend {
                    ExtendState::Initial => {
                        auction.extend = ExtendState::ExtendBy5Min;
                        auction.ending_time += ENDING_TIME_EXTEND_5;
                        message = Some(SystemMessageId::BidderExistsTheAuctionTimeHasBeenExtendedBy5Minutes);
                    }
                    ExtendState::ExtendBy5Min => {
                        if other_bidder(auction) {
                            auction.extend = ExtendState::ExtendBy3Min;
                            auction.ending_time += ENDING_TIME_EXTEND_3;
                            message = Some(SystemMessageId::BidderExistsAuctionTimeHasBeenExtendedBy3Minutes);
                        }
                    }
                    ExtendState::ExtendBy3Min => {
                        if extends_on_bid > 0 && other_bidder(auction) {
                            auction.extend = ExtendState::ExtendByConfigPhaseA;
                            auction.ending_time += extends_on_bid;
                        }
                    }
                    ExtendState::ExtendByConfigPhaseA => {
                        if other_bidder(auction)
                            && auction.scheduled_extend == ExtendState::ExtendByConfigPhaseB
                        {
                            auction.extend = ExtendState::ExtendByConfigPhaseB;
                            auction.ending_time += extends_on_bid;
                        }
                    }
                    ExtendState::ExtendByConfigPhaseB => {
                        if other_bidder(auction)
                            && auction.scheduled_extend == ExtendState::ExtendByConfigPhaseA
                        {
                            auction.ending_time += extends_on_bid;
                            auction.extend = ExtendState::ExtendByConfigPhaseA;
                        }
                    }
                }
            }
        }
        if let Some(old) = outbid
            && self.players.contains_key(&old)
        {
            self.send(old, &SystemMessage::new(SystemMessageId::YouHaveBeenOutbid));
        }
        message
    }

    /// Java `cancelBid(player)`: `true` when the bypass has something to report (a bid returned or
    /// the "reserve not met" message).
    pub(crate) fn item_auction_cancel_bid(
        &mut self,
        instance_id: i32,
        auction_id: i32,
        player: ObjectId,
    ) -> bool {
        let expired_after = self.item_auctions.settings.expired_after_ms();
        let now = unix_ms_now();
        let player_obj_id = player.0;
        let Some((state, starting_time, highest, own)) = self
            .item_auctions
            .instances
            .get(&instance_id)
            .and_then(|i| i.auctions.get(&auction_id))
            .map(|a| {
                (
                    a.state,
                    a.starting_time,
                    a.highest,
                    a.bid_for(player_obj_id).map(|b| (b.last_bid, b.is_canceled())),
                )
            })
        else {
            return false;
        };
        match state {
            ItemAuctionState::Created => return false,
            ItemAuctionState::Finished => {
                if starting_time < now.wrapping_sub(expired_after) {
                    return false;
                }
            }
            ItemAuctionState::Started => {}
        }
        let Some(highest) = highest else {
            return false;
        };
        let Some((last_bid, canceled)) = own else {
            return false;
        };
        if player_obj_id == highest {
            // Can't return winning bid.
            if state == ItemAuctionState::Finished {
                return false;
            }
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCurrentlyHaveTheHighestBidButTheReserveHasNotBeenMet),
            );
            return true;
        }
        if canceled {
            return false;
        }

        // increaseItemCount: `addAdena(REFUND, ..)`.
        self.add_adena(player, last_bid, true);
        if let Some(bid) = self
            .item_auction_mut(instance_id, auction_id)
            .and_then(|a| a.bid_for_mut(player_obj_id))
        {
            bid.last_bid = -1;
        }
        // Delete bid from database if auction already finished.
        let write = if state == ItemAuctionState::Finished {
            ItemAuctionWrite::DeleteBid {
                auction_id,
                player_obj_id,
            }
        } else {
            ItemAuctionWrite::Bid {
                auction_id,
                player_obj_id,
                bid: -1,
            }
        };
        self.item_auction_write(false, write);
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveCanceledYourBid),
        );
        true
    }
}
