//! `TradeRequest` and `AnswerTradeRequest`, the transaction state of `Player` they share with the
//! party, friend and duel requests, and the trade window's start and end.
// Java: network/clientpackets/{TradeRequest,AnswerTradeRequest}.java
// Java: entity/actor/Player.java (getActiveRequester, setActiveRequester, isProcessingRequest,
//       isProcessingTransaction, onTransactionRequest, isRequestExpired, onTransactionResponse,
//       startTrade, onTradeStart, onTradeConfirm, onTradeCancel, onTradeFinish, cancelActiveTrade)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ActionFailed, SendTradeRequest, SystemMessage, SystemMessageParam, TradeDone, TradeOtherDone,
};
use l2_util::ObjectId;
use tracing::warn;

use super::{GAME_TICKS_PER_SECOND, REQUEST_TIMEOUT, TRADE_ACTION_BLOCK_ID, TRADE_RANGE, TradeList};
use crate::world::World;
use crate::zones::ZoneId;

impl World {
    // ------------------------------------------------------------------ transaction state

    /// Java `AccessLevel.allowTransaction()` of a player.
    pub(crate) fn allows_transaction(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| self.data.access_level_allows_transaction(p.data.access_level))
    }

    /// Java `Player.isRequestExpired()` of the player who asked: its `_requestExpireTime` is not
    /// after now. A requester that left the world keeps the time it had (`fallback`).
    fn requester_request_expired(&self, requester: ObjectId, fallback: i64) -> bool {
        let expire = self
            .players
            .get(&requester)
            .map_or(fallback, |p| p.trade.request_expire_time);
        expire <= i64::from(self.game_ticks())
    }

    /// Java `Player.isRequestExpired()`.
    pub(crate) fn is_request_expired(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_none_or(|p| p.trade.request_expire_time <= i64::from(self.game_ticks()))
    }

    /// Java `Player.getActiveRequester()`: who asked this player. A request that ran out is
    /// forgotten here, unless a trade is on.
    pub(crate) fn active_requester(&mut self, id: ObjectId) -> Option<ObjectId> {
        let p = self.players.get(&id)?;
        let requester = p.trade.active_requester?;
        let expired = self.requester_request_expired(requester, p.trade.requester_expire_time);
        if expired && p.trade.active.is_none() {
            self.set_active_requester(id, None);
            return None;
        }
        Some(requester)
    }

    /// Java `Player.setActiveRequester(requester)`. The requester's expire time is kept for the
    /// case it leaves before the request ends.
    pub(crate) fn set_active_requester(&mut self, id: ObjectId, requester: Option<ObjectId>) {
        let expire = requester
            .and_then(|r| self.players.get(&r))
            .map_or(0, |r| r.trade.request_expire_time);
        if let Some(p) = self.players.get_mut(&id) {
            p.trade.active_requester = requester;
            p.trade.requester_expire_time = expire;
        }
    }

    /// Java `Player.isProcessingRequest()`.
    pub(crate) fn is_processing_request(&mut self, id: ObjectId) -> bool {
        self.active_requester(id).is_some()
            || self
                .players
                .get(&id)
                .is_some_and(|p| p.trade.request_expire_time > i64::from(self.game_ticks()))
    }

    /// Java `Player.isProcessingTransaction()`.
    pub(crate) fn is_processing_transaction(&mut self, id: ObjectId) -> bool {
        self.active_requester(id).is_some()
            || self.players.get(&id).is_some_and(|p| {
                p.trade.active.is_some() || p.trade.request_expire_time > i64::from(self.game_ticks())
            })
    }

    /// Java `Player.onTransactionRequest(partner)`: `id` asked `partner`; the request is open for
    /// `REQUEST_TIMEOUT` seconds.
    pub(crate) fn on_transaction_request(&mut self, id: ObjectId, partner: ObjectId) {
        let expire =
            i64::from(self.game_ticks()) + i64::from(REQUEST_TIMEOUT) * i64::from(GAME_TICKS_PER_SECOND);
        if let Some(p) = self.players.get_mut(&id) {
            p.trade.request_expire_time = expire;
        }
        self.set_active_requester(partner, Some(id));
    }

    /// Java `Player.onTransactionResponse()`.
    pub(crate) fn on_transaction_response(&mut self, id: ObjectId) {
        if let Some(p) = self.players.get_mut(&id) {
            p.trade.request_expire_time = 0;
        }
    }

    /// Java `Player.calculateDistance3D(other)` as a plain number.
    pub(crate) fn distance_3d(&self, a: ObjectId, b: ObjectId) -> Option<f64> {
        let (ax, ay, az) = self.position_of(a)?;
        let (bx, by, bz) = self.position_of(b)?;
        let (dx, dy, dz) = (
            f64::from(ax) - f64::from(bx),
            f64::from(ay) - f64::from(by),
            f64::from(az) - f64::from(bz),
        );
        Some((dx * dx + dy * dy + dz * dz).sqrt())
    }

    fn player_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------ TradeRequest

    /// Java `TradeRequest.runImpl`.
    pub(crate) fn on_trade_request(&mut self, id: ObjectId, object_id: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.allows_transaction(id) {
            self.send_message(id, "Transactions are disabled for your current Access Level.");
            self.send(id, &ActionFailed);
            return;
        }

        // `BOT_PENALTY` effects with the trade block.
        if !self.check_bot_penalty(id, TRADE_ACTION_BLOCK_ID) {
            return;
        }

        // If there is no target, target is far away or they are in different instances (except
        // multiverse) trade request is ignored and there is no system message.
        let target = ObjectId(object_id);
        if !self.objects.contains_key(&target) || !self.is_in_surrounding_region(id, target) {
            return;
        }
        let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        let target_instance = self.objects.get(&target).map_or(0, |e| e.instance_id);
        if target_instance != instance && instance != -1 {
            return;
        }

        // If target and acting player are the same, trade request is ignored and the following
        // system message is sent to acting player.
        if target == id {
            self.send(id, &SystemMessage::new(SystemMessageId::ThatIsAnIncorrectTarget));
            return;
        }

        if !self.players.contains_key(&target) {
            self.send(id, &SystemMessage::new(SystemMessageId::InvalidTarget));
            return;
        }
        let partner = target;

        // The olympiad (M8-OLY-01).
        if self.is_in_olympiad_mode(partner) || self.is_in_olympiad_mode(id) {
            self.send_message(id, "A user currently participating in the Olympiad cannot accept or request a trade.");
            return;
        }

        if self.partner_bot_penalty_blocks(partner) {
            let name = self.player_name(partner);
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::C1HasBeenReportedAsAnIllegalProgramUserAndIsCurrentlyBeingInvestigated,
                )
                .with(SystemMessageParam::Text(name)),
            );
            self.send(id, &ActionFailed);
            return;
        }

        // L2J Customs: Karma punishment.
        let can_trade = self.settings.inventory.alt_game_karma_player_can_trade;
        if !can_trade && self.players.get(&id).is_some_and(|p| p.data.karma > 0) {
            self.send_message(id, "You cannot trade while you are in a chaotic state.");
            return;
        }
        if !can_trade && self.players.get(&partner).is_some_and(|p| p.data.karma > 0) {
            self.send_message(
                id,
                "You cannot request a trade while your target is in a chaotic state.",
            );
            return;
        }

        if self.settings.zones.jail_disable_transaction
            && (self.is_inside_zone(id, ZoneId::Jail) || self.is_inside_zone(partner, ZoneId::Jail))
        {
            self.send_message(id, "You cannot trade while you are in in Jail.");
            return;
        }

        if self.is_in_store_mode(id) || self.is_in_store_mode(partner) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::WhileOperatingAPrivateStoreOrWorkshopYouCannotDiscardDestroyOrTradeAnItem,
                ),
            );
            return;
        }

        if self.is_processing_transaction(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAreAlreadyTradingWithSomeone),
            );
            return;
        }

        if self.is_processing_request(partner) || self.is_processing_transaction(partner) {
            let name = self.player_name(partner);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::C1IsAlreadyTradingWithAnotherPersonPleaseTryAgainLater)
                    .with(SystemMessageParam::Text(name)),
            );
            return;
        }

        if self.players.get(&partner).is_some_and(|p| p.trade.refusal) {
            self.send_message(id, "That person is in trade refusal mode.");
            return;
        }

        // Java `BlockList.isBlocked(partner, player)`: the partner refuses messages or blocks the
        // player.
        if self.is_blocked(partner, id) {
            let name = self.player_name(partner);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::S1HasPlacedYouOnHisHerIgnoreList)
                    .with(SystemMessageParam::Text(name)),
            );
            return;
        }

        if self.distance_3d(id, partner).is_none_or(|d| d > TRADE_RANGE) {
            self.send(id, &SystemMessage::new(SystemMessageId::YourTargetIsOutOfRange));
            return;
        }

        self.on_transaction_request(id, partner);
        self.send(partner, &SendTradeRequest { sender_id: id.0 });
        let name = self.player_name(partner);
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouHaveRequestedATradeWithC1)
                .with(SystemMessageParam::Text(name)),
        );
    }

    /// The `BOT_PENALTY` check `TradeRequest` makes for the partner: `BlockAction.checkCondition`
    /// of each effect.
    fn partner_bot_penalty_blocks(&self, partner: ObjectId) -> bool {
        use l2_data::stats::AbnormalType;
        self.effect_list(partner)
            .and_then(|l| l.info_by_abnormal_type(AbnormalType::BotPenalty))
            .is_some_and(|info| {
                info.effects()
                    .iter()
                    .any(|e| !e.handler.check_condition(TRADE_ACTION_BLOCK_ID))
            })
    }

    // ------------------------------------------------------------------ AnswerTradeRequest

    /// Java `AnswerTradeRequest.runImpl`.
    pub(crate) fn on_answer_trade_request(&mut self, id: ObjectId, response: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.allows_transaction(id) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            self.send(id, &ActionFailed);
            return;
        }

        let partner = self.active_requester(id).filter(|p| self.players.contains_key(p));
        let Some(partner) = partner else {
            // Trade partner not found, cancel trade. (Java has two branches, for a requester that
            // is null and one that left the world; both do this.)
            self.send(id, &TradeDone { num: 0 });
            self.send(id, &SystemMessage::new(SystemMessageId::ThatPlayerIsNotOnline));
            self.set_active_requester(id, None);
            return;
        };

        if response == 1 && !self.is_request_expired(partner) {
            self.start_trade(id, partner);
        } else {
            let name = self.player_name(id);
            self.send(
                partner,
                &SystemMessage::new(SystemMessageId::C1HasDeniedYourRequestToTrade)
                    .with(SystemMessageParam::Text(name)),
            );
        }

        // Clears requesting status.
        self.set_active_requester(id, None);
        self.on_transaction_response(partner);
    }

    // ------------------------------------------------------------------ the window

    /// Java `Player.startTrade(partner)`.
    pub(crate) fn start_trade(&mut self, id: ObjectId, partner: ObjectId) {
        self.on_trade_start(id, partner);
        self.on_trade_start(partner, id);
    }

    /// Java `Player.onTradeStart(partner)`.
    fn on_trade_start(&mut self, id: ObjectId, partner: ObjectId) {
        let mut list = TradeList::new(id);
        list.partner = Some(partner);
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        p.trade.active = Some(list);

        let name = self.player_name(partner);
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouBeginTradingWithC1)
                .with(SystemMessageParam::PlayerName(name)),
        );
        if let Some(start) = self.trade_start_packet(id, partner) {
            self.send(id, &start);
        }
    }

    /// Java `Player.onTradeConfirm(partner)`: `partner` pressed OK.
    pub(crate) fn on_trade_confirm(&mut self, id: ObjectId, partner: ObjectId) {
        let name = self.player_name(partner);
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::C1HasConfirmedTheTrade)
                .with(SystemMessageParam::PlayerName(name)),
        );
        self.send(id, &TradeOtherDone);
    }

    /// Java `Player.onTradeCancel(partner)`.
    pub(crate) fn on_trade_cancel(&mut self, id: ObjectId, partner: ObjectId) {
        let Some(p) = self.players.get_mut(&id) else {
            return;
        };
        let Some(mut list) = p.trade.active.take() else {
            return;
        };
        list.lock();
        self.send(id, &TradeDone { num: 0 });
        let name = self.player_name(partner);
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::C1HasCancelledTheTrade)
                .with(SystemMessageParam::PlayerName(name)),
        );
    }

    /// Java `Player.onTradeFinish(successful)`.
    pub(crate) fn on_trade_finish(&mut self, id: ObjectId, successful: bool) {
        if let Some(p) = self.players.get_mut(&id) {
            p.trade.active = None;
        }
        self.send(id, &TradeDone { num: 1 });
        if successful {
            self.send(id, &SystemMessage::new(SystemMessageId::YourTradeWasSuccessful));
        }
    }

    /// Java `Player.cancelActiveTrade()`: both sides hear that the trade is cancelled.
    ///
    /// Java calls it from `Player.deleteMe` (see `leave.rs`), when a duel starts, from the
    /// attribute enchant, and from the trade packets. Not from movement, death, teleport or an
    /// attack.
    // NOTE(java-quirk): `onTradeCancel(this)` names the player who cancels in the message to
    // himself too ("<own name> has cancelled the trade").
    pub fn cancel_active_trade(&mut self, id: ObjectId) {
        let Some(partner) = self
            .players
            .get(&id)
            .and_then(|p| p.trade.active.as_ref())
            .map(|l| l.partner)
        else {
            return;
        };
        if let Some(partner) = partner {
            self.on_trade_cancel(partner, id);
        }
        self.on_trade_cancel(id, id);
    }

    /// Java `Player.deleteMe`: "Deals with sudden exit in the middle of transaction."
    // NOTE(java-quirk): this only runs `if (getActiveRequester() != null)`: a player who leaves in
    // the middle of an open trade (the request is answered by then, so there is no requester) does
    // not cancel it. The partner finds out the next time it touches the trade
    // (`THAT_PLAYER_IS_NOT_ONLINE` in `AddTradeItem` / `TradeDone`).
    pub(crate) fn cancel_transaction_on_leave(&mut self, id: ObjectId) {
        if self.active_requester(id).is_some() {
            self.set_active_requester(id, None);
            self.cancel_active_trade(id);
        }
    }

    /// Warns about a trade packet that arrives without a trade (Java logs it at warning level).
    pub(super) fn warn_no_trade(&self, id: ObjectId, what: &str) {
        warn!("Character: {} {what}", self.player_name(id));
    }
}
