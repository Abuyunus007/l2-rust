//! The `Auctioneer` NPC: the pages of the clan hall auction board.
// Java: entity/actor/instance/Auctioneer.java
//
// Every command is the Java one with its pages (`data/html/auction/*.htm`) and messages. Java
// catches most exceptions of a command and answers with a message ("Invalid bid!"); an exception
// it does not catch ends the command without an answer, as `None` does here.
//
// The commands that change an auction (`bid`, `doCancelBid`, `doCancelAuction`) read the auction
// again first and go on in `auction.rs` (D-50); the pages show what this world knows.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::auction::{ClanHallAuction, leader_name_of};
use super::format::{bid_date, date_short, date_time};
use crate::clan::ClanAccess;
use crate::residence::{ClanHallId, ResidenceReadKind, ResidenceReadThen};
use crate::world::{World, unix_ms_now};

/// `Auctioneer.COND_*`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Condition {
    AllFalse,
    BusyBecauseOfSiege,
    Regular,
}

/// What a page prints about an auctionable hall.
struct HallInfo {
    name: String,
    desc: String,
    location: String,
    grade: i32,
    lease: i32,
}

/// `html.replace(pattern, value)`.
fn rep(html: &mut NpcHtmlMessage, from: &str, to: impl std::fmt::Display) {
    html.html = html.html.replace(from, &to.to_string());
}

/// `bypass -h npc_<objectId>_<command>`.
fn link(npc: ObjectId, command: &str) -> String {
    format!("bypass -h npc_{}_{command}", npc.0)
}

/// The "N hours M minutes" text of `%AGIT_AUCTION_REMAIN%`.
fn remain_text(end_date: i64, now: i64) -> String {
    format!(
        "{} hours {} minutes",
        (end_date - now) / 3_600_000,
        ((end_date - now) / 60_000) % 60
    )
}

impl World {
    /// Java `Auctioneer.validateCondition()`.
    fn auctioneer_condition(&self, npc: ObjectId) -> Condition {
        // `getCastle() != null && getCastle().getResidenceId() > 0`.
        match self.npc_castle(npc) {
            // `getCastle().getSiege().isInProgress()` (M7-SIEGE-01).
            Some(castle) if castle.0 > 0 && self.is_castle_siege_in_progress(castle) => {
                Condition::BusyBecauseOfSiege
            }
            Some(castle) if castle.0 > 0 => Condition::Regular,
            _ => Condition::AllFalse,
        }
    }

    fn auctionable_hall_info(&self, id: i32) -> Option<HallInfo> {
        self.clan_hall(ClanHallId(id))
            .filter(|h| !h.is_siegable_hall())
            .map(|h| HallInfo {
                name: h.name.clone(),
                desc: h.desc.clone(),
                location: h.location.clone(),
                grade: h.grade(),
                lease: h.lease(),
            })
    }

    /// `html.setFile(player, path)` for the NPC.
    fn auction_page(&self, player: ObjectId, npc: ObjectId, path: &str) -> Option<NpcHtmlMessage> {
        self.html_from_file(player, npc.0, path)
    }

    /// Java `Auctioneer.showChatWindow(player)`.
    pub(crate) fn auctioneer_show_chat_window(&mut self, npc: ObjectId, player: ObjectId) {
        let path = if self.auctioneer_condition(npc) == Condition::BusyBecauseOfSiege {
            "data/html/auction/auction-busy.htm"
        } else {
            "data/html/auction/auction.htm"
        };
        let Some(mut html) = self.auction_page(player, npc, path) else {
            return;
        };
        let (npc_id, name) = self
            .npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| (t.id, t.name.clone()))
            .unwrap_or_default();
        rep(&mut html, "%objectId%", npc.0);
        rep(&mut html, "%npcId%", npc_id);
        rep(&mut html, "%npcname%", name);
        self.send_html(player, &html);
    }

    /// Java `Auctioneer.onBypassFeedback(player, command)`.
    pub(crate) fn auctioneer_bypass_feedback(&mut self, npc: ObjectId, player: ObjectId, command: &str) {
        match self.auctioneer_condition(npc) {
            Condition::AllFalse => {
                self.send_message(player, "Wrong conditions.");
                return;
            }
            Condition::BusyBecauseOfSiege => {
                if let Some(mut html) = self.auction_page(player, npc, "data/html/auction/auction-busy.htm") {
                    rep(&mut html, "%objectId%", npc.0);
                    self.send_html(player, &html);
                }
                return;
            }
            Condition::Regular => {}
        }
        let mut tokens = command.split(' ').filter(|t| !t.is_empty());
        let Some(actual) = tokens.next() else {
            self.generic_npc_bypass_feedback(npc, player, command);
            return;
        };
        let val = tokens.next().unwrap_or("");
        let rest: Vec<&str> = tokens.collect();
        let handled = match actual.to_lowercase().as_str() {
            "auction" => self.auctioneer_auction(npc, player, val, &rest),
            "confirmauction" => self.auctioneer_confirm_auction(npc, player),
            "bidding" => self.auctioneer_bidding(npc, player, val),
            "bid" => self.auctioneer_bid(player, val, &rest),
            "bid1" => self.auctioneer_bid1(npc, player, val),
            "list" => self.auctioneer_list(npc, player, val),
            "bidlist" => self.auctioneer_bidlist(npc, player, val),
            "selecteditems" => self.auctioneer_selected_items(npc, player),
            "cancelbid" => self.auctioneer_cancel_bid_page(npc, player),
            "docancelbid" => self.auctioneer_do_cancel_bid(player),
            "cancelauction" => self.auctioneer_cancel_auction_page(npc, player),
            "docancelauction" => self.auctioneer_do_cancel_auction(player),
            "sale2" => self.auctioneer_sale2(npc, player),
            "sale" => self.auctioneer_sale(npc, player),
            "rebid" => self.auctioneer_rebid(npc, player),
            "location" => self.auctioneer_location(npc, player),
            "start" => {
                self.auctioneer_show_chat_window(npc, player);
                true
            }
            _ => false,
        };
        if !handled {
            // `super.onBypassFeedback(player, command)`.
            self.generic_npc_bypass_feedback(npc, player, command);
        }
    }

    /// The page of "not authorized" (`data/html/auction/not_authorized.htm`).
    fn auction_not_authorized(&mut self, npc: ObjectId, player: ObjectId) {
        if let Some(mut html) = self.auction_page(player, npc, "data/html/auction/not_authorized.htm") {
            rep(&mut html, "%objectId%", npc.0);
            self.send_html(player, &html);
        }
    }

    // ------------------------------------------------------------------ selling

    /// `auction <days> [bid]`: the clan prepares to put its hall up for sale.
    fn auctioneer_auction(&mut self, npc: ObjectId, player: ObjectId, val: &str, rest: &[&str]) -> bool {
        if val.is_empty() {
            return true;
        }
        let Ok(days) = val.parse::<i32>() else {
            self.send_message(player, "Invalid auction duration!");
            return true;
        };
        let max_adena = self.settings.ground.max_adena;
        let prepared = (|| {
            let bid = match rest.first() {
                Some(token) => token.parse::<i64>().ok()?.min(max_adena),
                None => 0,
            };
            let clan = self.clan_of(player)?;
            let hall = self.auctionable_hall_by_owner(clan.id)?;
            let hall = self.clan_hall(hall)?;
            // NOTE(java-quirk): `days * 86400000` is an int product: more than 24 days wrap.
            let delay = i64::from(days.wrapping_mul(86_400_000));
            let auction = ClanHallAuction::pending(
                clan.hideout_id,
                clan.leader_id(),
                leader_name_of(clan),
                clan.name.clone(),
                delay,
                bid,
                hall.name.clone(),
            );
            Some((auction, hall.desc.clone()))
        })();
        let Some((auction, desc)) = prepared else {
            self.send_message(player, "Invalid bid!");
            return true;
        };
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitSale3.htm") else {
            return true;
        };
        rep(&mut html, "%x%", val);
        rep(&mut html, "%AGIT_AUCTION_END%", date_time(auction.end_date));
        rep(&mut html, "%AGIT_AUCTION_MINBID%", auction.starting_bid);
        rep(&mut html, "%AGIT_AUCTION_MIN%", auction.starting_bid);
        rep(&mut html, "%AGIT_AUCTION_DESC%", desc);
        rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "sale2"));
        rep(&mut html, "%objectId%", npc.0);
        // `_pendingAuctions.put(a.getId(), a)` replaces an older one.
        self.clanhall.pending_auctions.insert((npc, auction.id), auction);
        self.send_html(player, &html);
        true
    }

    /// `confirmAuction`: the prepared auction joins the list.
    fn auctioneer_confirm_auction(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let hideout = self.clan_of(player).map(|c| c.hideout_id);
        let pending = hideout.and_then(|id| self.clanhall.pending_auctions.remove(&(npc, id)));
        match pending {
            Some(auction) => self.auction_confirm(auction),
            None => self.send_message(player, "Invalid auction"),
        }
        true
    }

    /// `sale`: the first page of the sale.
    fn auctioneer_sale(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        if !self.has_clan_access(player, ClanAccess::HallAuction) {
            self.auction_not_authorized(npc, player);
            return true;
        }
        let lease = self
            .clan_of(player)
            .and_then(|c| self.auctionable_hall_by_owner(c.id))
            .and_then(|h| self.auctionable_hall_info(h.0))
            .map(|h| h.lease);
        let adena = self.clan_of(player).map(|c| self.clan_warehouse_adena(c.id));
        let (Some(lease), Some(adena)) = (lease, adena) else {
            warn!("Auctioneer: {player} has no clan hall to sell.");
            return true;
        };
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitSale1.htm") else {
            return true;
        };
        rep(&mut html, "%AGIT_DEPOSIT%", lease);
        rep(&mut html, "%AGIT_PLEDGE_ADENA%", adena);
        rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "selectedItems"));
        rep(&mut html, "%objectId%", npc.0);
        self.send_html(player, &html);
        true
    }

    /// `sale2`: the second page of the sale.
    fn auctioneer_sale2(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let lease = self
            .clan_of(player)
            .and_then(|c| self.auctionable_hall_by_owner(c.id))
            .and_then(|h| self.auctionable_hall_info(h.0))
            .map(|h| h.lease);
        let Some(lease) = lease else {
            warn!("Auctioneer: {player} has no clan hall to sell.");
            return true;
        };
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitSale2.htm") else {
            return true;
        };
        rep(&mut html, "%AGIT_LAST_PRICE%", lease);
        rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "sale"));
        rep(&mut html, "%objectId%", npc.0);
        self.send_html(player, &html);
        true
    }

    /// `cancelAuction`: the page that asks the owner.
    fn auctioneer_cancel_auction_page(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        if !self.has_clan_access(player, ClanAccess::HallAuction) {
            self.auction_not_authorized(npc, player);
            return true;
        }
        let lease = self
            .clan_of(player)
            .and_then(|c| self.auctionable_hall_by_owner(c.id))
            .and_then(|h| self.auctionable_hall_info(h.0))
            .map(|h| h.lease);
        let Some(lease) = lease else {
            warn!("Auctioneer: {player} has no clan hall.");
            return true;
        };
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitSaleCancel.htm") else {
            return true;
        };
        rep(&mut html, "%AGIT_DEPOSIT%", lease);
        rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "selectedItems"));
        rep(&mut html, "%objectId%", npc.0);
        self.send_html(player, &html);
        true
    }

    /// `doCancelAuction`: the owner's auction is cancelled (no privilege check in Java).
    fn auctioneer_do_cancel_auction(&mut self, player: ObjectId) -> bool {
        let hideout = self.clan_of(player).map(|c| c.hideout_id);
        if let Some(id) = hideout
            && self.clan_hall_auction(id).is_some()
        {
            self.request_auction_reread(
                id,
                ResidenceReadThen::AuctionCancel {
                    player,
                    auction_id: id,
                },
            );
        }
        true
    }

    // ------------------------------------------------------------------ bidding

    /// `bidding <auctionId>`: the page of one auction.
    fn auctioneer_bidding(&mut self, npc: ObjectId, player: ObjectId, val: &str) -> bool {
        if val.is_empty() {
            return true;
        }
        let Ok(auction_id) = val.parse::<i32>() else {
            self.send_message(player, "Invalid auction!");
            return true;
        };
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitAuctionInfo.htm") else {
            return true;
        };
        match self.clan_hall_auction(auction_id).cloned() {
            Some(a) => {
                let Some(hall) = self.auctionable_hall_info(a.item_id) else {
                    self.send_message(player, "Invalid auction!");
                    return true;
                };
                let now = unix_ms_now();
                rep(&mut html, "%AGIT_NAME%", &a.item_name);
                rep(&mut html, "%OWNER_PLEDGE_NAME%", &a.seller_clan_name);
                rep(&mut html, "%OWNER_PLEDGE_MASTER%", &a.seller_name);
                rep(&mut html, "%AGIT_SIZE%", hall.grade * 10);
                rep(&mut html, "%AGIT_LEASE%", hall.lease);
                rep(&mut html, "%AGIT_LOCATION%", &hall.location);
                rep(&mut html, "%AGIT_AUCTION_END%", date_time(a.end_date));
                rep(&mut html, "%AGIT_AUCTION_REMAIN%", remain_text(a.end_date, now));
                rep(&mut html, "%AGIT_AUCTION_MINBID%", a.starting_bid);
                rep(&mut html, "%AGIT_AUCTION_COUNT%", a.bidders.len());
                rep(&mut html, "%AGIT_AUCTION_DESC%", &hall.desc);
                rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "list"));
                rep(
                    &mut html,
                    "%AGIT_LINK_BIDLIST%",
                    link(npc, &format!("bidlist {}", a.id)),
                );
                rep(&mut html, "%AGIT_LINK_RE%", link(npc, &format!("bid1 {}", a.id)));
            }
            None => warn!("Auctioneer Auction null for AuctionId : {auction_id}"),
        }
        self.send_html(player, &html);
        true
    }

    /// `bid <auctionId> [bid]`: the auction is read again, then the bid is placed.
    fn auctioneer_bid(&mut self, player: ObjectId, val: &str, rest: &[&str]) -> bool {
        if val.is_empty() {
            return true;
        }
        let Ok(auction_id) = val.parse::<i32>() else {
            self.send_message(player, "Invalid auction!");
            return true;
        };
        let max_adena = self.settings.ground.max_adena;
        let bid = match rest.first() {
            Some(token) => token.parse::<i64>().ok().map(|b| b.min(max_adena)),
            None => Some(0),
        };
        let (Some(bid), true) = (bid, self.clan_hall_auction(auction_id).is_some()) else {
            self.send_message(player, "Invalid bid!");
            return true;
        };
        self.request_auction_reread(
            auction_id,
            ResidenceReadThen::AuctionBid {
                player,
                auction_id,
                bid,
            },
        );
        true
    }

    /// `bid1 <auctionId>`: the page that asks for the bid.
    fn auctioneer_bid1(&mut self, npc: ObjectId, player: ObjectId, val: &str) -> bool {
        let Some(clan) = self.clan_of(player).filter(|c| c.level >= 2) else {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::OnlyAClanLeaderWhoseClanIsOfLevel2OrHigherIsAllowedToParticipateInAClanHallAuction,
                ),
            );
            return true;
        };
        if val.is_empty() {
            return true;
        }
        // `Integer.parseInt(val)` outside the try: the exception ends the command.
        let Ok(auction_id) = val.parse::<i32>() else {
            warn!("Auctioneer: bad auction id {val} from {player}");
            return true;
        };
        if (clan.auction_bid_at > 0 && clan.auction_bid_at != auction_id) || clan.hideout_id > 0 {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::SinceYouHaveAlreadySubmittedABidYouAreNotAllowedToParticipateInAnotherAuctionAtThisTime,
                ),
            );
            return true;
        }
        let adena = self.clan_warehouse_adena(clan.id);
        let Some(minimum_bid) = self.clan_hall_auction(auction_id).map(|a| {
            if a.highest_bidder_max_bid == 0 {
                a.starting_bid
            } else {
                a.highest_bidder_max_bid
            }
        }) else {
            self.send_message(player, "Invalid auction!");
            return true;
        };
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitBid1.htm") else {
            return true;
        };
        rep(
            &mut html,
            "%AGIT_LINK_BACK%",
            link(npc, &format!("bidding {val}")),
        );
        rep(&mut html, "%PLEDGE_ADENA%", adena);
        rep(&mut html, "%AGIT_AUCTION_MINBID%", minimum_bid);
        rep(
            &mut html,
            "npc_%objectId%_bid",
            format!("npc_{}_bid {val}", npc.0),
        );
        self.send_html(player, &html);
        true
    }

    /// `rebid`: the page of a clan that raises its bid.
    fn auctioneer_rebid(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        if !self.has_clan_access(player, ClanAccess::HallAuction) {
            self.auction_not_authorized(npc, player);
            return true;
        }
        let Some((clan_id, bidded_at)) = self.clan_of(player).map(|c| (c.id, c.auction_bid_at)) else {
            self.send_message(player, "Invalid auction!");
            return true;
        };
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitBid2.htm") else {
            return true;
        };
        match self.clan_hall_auction(bidded_at).cloned() {
            Some(a) => {
                let Some(own_bid) = a.bidders.get(&clan_id).map(|b| b.bid) else {
                    self.send_message(player, "Invalid auction!");
                    return true;
                };
                rep(&mut html, "%AGIT_AUCTION_BID%", own_bid);
                rep(&mut html, "%AGIT_AUCTION_MINBID%", a.starting_bid);
                rep(&mut html, "%AGIT_AUCTION_END%", date_time(a.end_date));
                rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "selectedItems"));
                rep(
                    &mut html,
                    "npc_%objectId%_bid1",
                    format!("npc_{}_bid1 {}", npc.0, a.id),
                );
            }
            None => warn!("Auctioneer Auction null for AuctionBiddedAt : {bidded_at}"),
        }
        self.send_html(player, &html);
        true
    }

    /// `cancelBid`: the page that asks the bidder.
    fn auctioneer_cancel_bid_page(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let bid = self.clan_of(player).and_then(|c| {
            self.clan_hall_auction(c.auction_bid_at)
                .and_then(|a| a.bidders.get(&c.id))
                .map(|b| b.bid)
        });
        let Some(bid) = bid else {
            warn!("Auctioneer: {player} has no bid to cancel.");
            return true;
        };
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitBidCancel.htm") else {
            return true;
        };
        rep(&mut html, "%AGIT_BID%", bid);
        rep(&mut html, "%AGIT_BID_REMAIN%", (bid as f64 * 0.9) as i64);
        rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "selectedItems"));
        rep(&mut html, "%objectId%", npc.0);
        self.send_html(player, &html);
        true
    }

    /// `doCancelBid`: the auction is read again, then the bid is cancelled.
    fn auctioneer_do_cancel_bid(&mut self, player: ObjectId) -> bool {
        let bidded_at = self.clan_of(player).map(|c| c.auction_bid_at);
        if let Some(id) = bidded_at
            && self.clan_hall_auction(id).is_some()
        {
            self.request_auction_reread(
                id,
                ResidenceReadThen::AuctionCancelBid {
                    player,
                    auction_id: id,
                },
            );
        }
        true
    }

    // ------------------------------------------------------------------ the board

    /// `list [page]`: the auctions, 15 to a page.
    fn auctioneer_list(&mut self, npc: ObjectId, player: ObjectId, val: &str) -> bool {
        // D-50: the Java game server shares the `auction` rows, so the list is built from rows
        // read just now (M7-FU-03); a failed read shows nothing.
        self.request_auction_rows(
            0,
            ResidenceReadKind::Auctions,
            ResidenceReadThen::AuctionList {
                npc,
                player,
                val: val.to_owned(),
            },
        );
        true
    }

    /// The rows are in: the page of `list` from them.
    pub(crate) fn auctioneer_list_after_read(&mut self, npc: ObjectId, player: ObjectId, val: &str) {
        if !self.players.contains_key(&player) || self.auctioneer_condition(npc) != Condition::Regular {
            return;
        }
        self.auctioneer_list_page(npc, player, val);
    }

    fn auctioneer_list_page(&mut self, npc: ObjectId, player: ObjectId, val: &str) -> bool {
        let mut limit: usize = 15;
        let npage = self.clan_hall_auctions().len().div_ceil(limit);
        let start = if val.is_empty() {
            1
        } else {
            // `Integer.parseInt(val)` outside any try: the exception ends the command.
            let Ok(page) = val.parse::<i32>() else {
                warn!("Auctioneer: bad page {val} from {player}");
                return true;
            };
            // Negative pages and overflow behave as Java's int arithmetic does.
            let first = 15_i32.wrapping_mul(page.wrapping_sub(1)).wrapping_add(1);
            limit = usize::try_from(15_i32.wrapping_mul(page)).unwrap_or(0);
            i64::from(first)
        };
        let mut items = String::from("<table width=280 border=0><tr>");
        for page in 1..=npage {
            items.push_str(&format!(
                "<td><center><a action=\"bypass -h npc_{}_list {page}\"> Page {page} </a></center></td>",
                npc.0
            ));
        }
        items.push_str("</tr></table>");
        items.push_str("<table width=280 border=0>");
        let mut i: i64 = 1;
        let auctions: Vec<ClanHallAuction> = self.clan_hall_auctions().to_vec();
        for a in &auctions {
            if i > i64::try_from(limit).unwrap_or(i64::MAX) {
                break;
            } else if i < start {
                i += 1;
                continue;
            }
            i += 1;
            // `getAuctionableHallById(a.getItemId()).getLocation()` throws for an unknown hall.
            let Some(hall) = self.auctionable_hall_info(a.item_id) else {
                warn!(
                    "Auctioneer: the auction {} has no auctionable hall {}.",
                    a.id, a.item_id
                );
                return true;
            };
            items.push_str("<tr>");
            items.push_str(&format!("<td>{}</td>", hall.location));
            items.push_str(&format!(
                "<td><a action=\"bypass -h npc_{}_bidding {}\">{}</a></td>",
                npc.0, a.id, a.item_name
            ));
            items.push_str(&format!("<td>{}</td>", date_short(a.end_date)));
            items.push_str(&format!("<td>{}</td>", a.starting_bid));
            items.push_str("</tr>");
        }
        items.push_str("</table>");
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitAuctionList.htm") else {
            return true;
        };
        rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "start"));
        rep(&mut html, "%itemsField%", items);
        self.send_html(player, &html);
        true
    }

    /// `bidlist [auctionId]`: the bidders of an auction.
    fn auctioneer_bidlist(&mut self, npc: ObjectId, player: ObjectId, val: &str) -> bool {
        let auction_id = if val.is_empty() {
            let Some(bidded_at) = self.clan_of(player).map(|c| c.auction_bid_at) else {
                return true;
            };
            if bidded_at <= 0 {
                return true;
            }
            bidded_at
        } else if let Ok(id) = val.parse::<i32>() {
            id
        } else {
            warn!("Auctioneer: bad auction id {val} from {player}");
            return true;
        };
        let Some(auction) = self.clan_hall_auction(auction_id) else {
            warn!("Auctioneer: no auction {auction_id} for the bidder list.");
            return true;
        };
        let mut bidders = String::new();
        for b in auction.bidders.values() {
            bidders.push_str(&format!(
                "<tr><td>{}</td><td>{}</td><td>{}</td><td>{}</td></tr>",
                b.clan_name,
                b.name,
                bid_date(b.time_bid),
                b.bid
            ));
        }
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitBidderList.htm") else {
            return true;
        };
        rep(&mut html, "%AGIT_LIST%", bidders);
        rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "selectedItems"));
        rep(&mut html, "%x%", val);
        rep(&mut html, "%objectId%", npc.0);
        self.send_html(player, &html);
        true
    }

    /// `selectedItems`: what the clan has to do with the board (its bid, its sale or its hall).
    fn auctioneer_selected_items(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let Some((clan_id, clan_name, leader, hideout, bidded_at)) = self.clan_of(player).map(|c| {
            (
                c.id,
                c.name.clone(),
                leader_name_of(c),
                c.hideout_id,
                c.auction_bid_at,
            )
        }) else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotMeetTheRequirementsToParticipateInAnAuction),
            );
            return true;
        };
        let now = unix_ms_now();
        if hideout == 0 && bidded_at > 0 {
            let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitBidInfo.htm") else {
                return true;
            };
            match self.clan_hall_auction(bidded_at).cloned() {
                Some(a) => {
                    let (Some(hall), Some(own_bid)) = (
                        self.auctionable_hall_info(a.item_id),
                        a.bidders.get(&clan_id).map(|b| b.bid),
                    ) else {
                        warn!("Auctioneer: the bid of clan {clan_id} cannot be shown.");
                        return true;
                    };
                    rep(&mut html, "%AGIT_NAME%", &a.item_name);
                    rep(&mut html, "%OWNER_PLEDGE_NAME%", &a.seller_clan_name);
                    rep(&mut html, "%OWNER_PLEDGE_MASTER%", &a.seller_name);
                    rep(&mut html, "%AGIT_SIZE%", hall.grade * 10);
                    rep(&mut html, "%AGIT_LEASE%", hall.lease);
                    rep(&mut html, "%AGIT_LOCATION%", &hall.location);
                    rep(&mut html, "%AGIT_AUCTION_END%", date_time(a.end_date));
                    rep(&mut html, "%AGIT_AUCTION_REMAIN%", remain_text(a.end_date, now));
                    rep(&mut html, "%AGIT_AUCTION_MINBID%", a.starting_bid);
                    rep(&mut html, "%AGIT_AUCTION_MYBID%", own_bid);
                    rep(&mut html, "%AGIT_AUCTION_DESC%", &hall.desc);
                    rep(&mut html, "%objectId%", npc.0);
                    rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "start"));
                }
                None => warn!("Auctioneer Auction null for AuctionBiddedAt : {bidded_at}"),
            }
            self.send_html(player, &html);
        } else if let Some(a) = self.clan_hall_auction(hideout).cloned() {
            let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitSaleInfo.htm") else {
                return true;
            };
            let Some(hall) = self.auctionable_hall_info(a.item_id) else {
                warn!("Auctioneer: the auction {} has no auctionable hall.", a.id);
                return true;
            };
            rep(&mut html, "%AGIT_NAME%", &a.item_name);
            rep(&mut html, "%AGIT_OWNER_PLEDGE_NAME%", &a.seller_clan_name);
            rep(&mut html, "%OWNER_PLEDGE_MASTER%", &a.seller_name);
            rep(&mut html, "%AGIT_SIZE%", hall.grade * 10);
            rep(&mut html, "%AGIT_LEASE%", hall.lease);
            rep(&mut html, "%AGIT_LOCATION%", &hall.location);
            rep(&mut html, "%AGIT_AUCTION_END%", date_time(a.end_date));
            rep(&mut html, "%AGIT_AUCTION_REMAIN%", remain_text(a.end_date, now));
            rep(&mut html, "%AGIT_AUCTION_MINBID%", a.starting_bid);
            rep(&mut html, "%AGIT_AUCTION_BIDCOUNT%", a.bidders.len());
            rep(&mut html, "%AGIT_AUCTION_DESC%", &hall.desc);
            rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "start"));
            rep(&mut html, "%id%", a.id);
            rep(&mut html, "%objectId%", npc.0);
            self.send_html(player, &html);
        } else if hideout != 0 {
            let Some(mut html) = self.auction_page(player, npc, "data/html/auction/AgitInfo.htm") else {
                return true;
            };
            match self.auctionable_hall_info(hideout) {
                Some(hall) => {
                    rep(&mut html, "%AGIT_NAME%", &hall.name);
                    rep(&mut html, "%AGIT_OWNER_PLEDGE_NAME%", &clan_name);
                    rep(&mut html, "%OWNER_PLEDGE_MASTER%", &leader);
                    rep(&mut html, "%AGIT_SIZE%", hall.grade * 10);
                    rep(&mut html, "%AGIT_LEASE%", hall.lease);
                    rep(&mut html, "%AGIT_LOCATION%", &hall.location);
                    rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "start"));
                    rep(&mut html, "%objectId%", npc.0);
                }
                None => {
                    warn!("Clan Hall ID NULL : {hideout} Can be caused by concurent write in ClanHallManager")
                }
            }
            self.send_html(player, &html);
        } else {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::ThereAreNoOfferingsIOwnOrIMadeABidFor),
            );
        }
        true
    }

    /// `location`: the picture of the nearest town.
    fn auctioneer_location(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let Some(mut html) = self.auction_page(player, npc, "data/html/auction/location.htm") else {
            return true;
        };
        let (town_name, picture) = match (self.zoning.map_regions.as_ref(), self.position_of(player)) {
            (Some(regions), Some((x, y, _))) => (
                regions.closest_town_name(x, y).to_owned(),
                town_picture(regions.map_region_loc_id(x, y)),
            ),
            _ => (String::new(), town_picture(0)),
        };
        rep(&mut html, "%location%", town_name);
        rep(&mut html, "%LOCATION%", picture);
        rep(&mut html, "%AGIT_LINK_BACK%", link(npc, "start"));
        self.send_html(player, &html);
        true
    }
}

/// Java `Auctioneer.getPictureName`: the picture of a map region id.
fn town_picture(region: i32) -> &'static str {
    match region {
        911 => "GLUDIN",
        912 => "GLUDIO",
        916 => "DION",
        918 => "GIRAN",
        1537 => "RUNE",
        1538 => "GODARD",
        1714 => "SCHUTTGART",
        _ => "ADEN",
    }
}
