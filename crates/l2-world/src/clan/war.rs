//! Clan wars: declaring, answering, stopping and surrendering, and the war list window
//! (M6-CLAN-02).
// Java: data/sql/ClanTable.java (storeClanWars, deleteClanWars)
// Java: network/clientpackets/{RequestStartPledgeWar,RequestReplyStartPledgeWar,
//       RequestStopPledgeWar,RequestReplyStopPledgeWar,RequestSurrenderPledgeWar,
//       RequestReplySurrenderPledgeWar,RequestPledgeWarList}.java
// Java: network/serverpackets/PledgeReceiveWarList.java
//
// A war is one-sided: `clan1.setEnemyClan(clan2)` and `clan2.setAttackerClan(clan1)`
// (`Clan::at_war_with` and `Clan::at_war_attackers`), and `clan_wars` holds the pair. Two clans
// at war with each other have a row each way. The war bits of the relation mask and the auto
// attack rules read the clans (`pvp.rs`); the members get a fresh `UserInfo`/`CharInfo` (and with
// it the relation) from the `broadcastUserInfo` Java calls after a declaration or a stop.
//
// NOTE: Java's `RequestStartPledgeWar` declares the war at once (the other clan is not asked), so
// the `RequestReply*PledgeWar` packets only do something for a player who has an active
// requester of any other kind (a party or trade invitation): the requestor is then taken as the
// declaring clan's player, exactly as in Java.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::pledge::PledgeReceiveWarList;
use l2_protocol_game::server::{ActionFailed, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::info;

use super::model::{ClanAccess, java_map_order};
use super::{ClanWrite, WarWrite};
use crate::world::World;

impl World {
    /// Java `ClanTable.storeClanWars(clanId1, clanId2)`: `clan1` declares war on `clan2`. A clan
    /// that does not exist makes Java throw; nothing happens here.
    pub fn store_clan_wars(&mut self, clan_id1: i32, clan_id2: i32) {
        let (Some(name1), Some(name2)) = (
            self.clan(clan_id1).map(|c| c.name.clone()),
            self.clan(clan_id2).map(|c| c.name.clone()),
        ) else {
            info!("ClanTable: storeClanWars of unknown clans {clan_id1} and {clan_id2}.");
            return;
        };
        // ON_CLAN_WAR_START (S-API-04).
        self.script_fire_async(
            crate::script::ScriptEvent::ClanWarStart {
                clan1: clan_id1,
                clan2: clan_id2,
            },
            None,
        );
        if let Some(c) = self.clan_mut(clan_id1)
            && !c.at_war_with.contains(&clan_id2)
        {
            c.at_war_with.push(clan_id2);
        }
        if let Some(c) = self.clan_mut(clan_id2)
            && !c.at_war_attackers.contains(&clan_id1)
        {
            c.at_war_attackers.push(clan_id1);
        }
        self.broadcast_clan_status(clan_id1);
        self.broadcast_clan_status(clan_id2);
        self.clan_write(ClanWrite::War(WarWrite::Store {
            clan1: clan_id1,
            clan2: clan_id2,
        }));

        self.broadcast_to_clan(
            clan_id1,
            &SystemMessage::new(
                SystemMessageId::AClanWarHasBeenDeclaredAgainstTheClanS1IfYouAreKilledDuringTheClanWarByMembersOfTheOpposingClanYouWillOnlyLoseAQuarterOfTheNormalExperienceFromDeath,
            )
            .with(SystemMessageParam::Text(name2)),
        );
        // clan1 declared clan war.
        self.broadcast_to_clan(
            clan_id2,
            &SystemMessage::new(SystemMessageId::S1HasDeclaredAClanWar).with(SystemMessageParam::Text(name1)),
        );
    }

    /// Java `ClanTable.deleteClanWars(clanId1, clanId2)`.
    pub fn delete_clan_wars(&mut self, clan_id1: i32, clan_id2: i32) {
        let (Some(name1), Some(name2)) = (
            self.clan(clan_id1).map(|c| c.name.clone()),
            self.clan(clan_id2).map(|c| c.name.clone()),
        ) else {
            info!("ClanTable: deleteClanWars of unknown clans {clan_id1} and {clan_id2}.");
            return;
        };
        // ON_CLAN_WAR_FINISH (S-API-04).
        self.script_fire_async(
            crate::script::ScriptEvent::ClanWarFinish {
                clan1: clan_id1,
                clan2: clan_id2,
            },
            None,
        );
        if let Some(c) = self.clan_mut(clan_id1) {
            c.at_war_with.retain(|id| *id != clan_id2);
        }
        if let Some(c) = self.clan_mut(clan_id2) {
            c.at_war_attackers.retain(|id| *id != clan_id1);
        }
        self.broadcast_clan_status(clan_id1);
        self.broadcast_clan_status(clan_id2);

        self.clan_write(ClanWrite::War(WarWrite::Delete {
            clan1: clan_id1,
            clan2: clan_id2,
        }));

        self.broadcast_to_clan(
            clan_id1,
            &SystemMessage::new(SystemMessageId::TheWarAgainstS1ClanHasBeenStopped)
                .with(SystemMessageParam::Text(name2)),
        );
        self.broadcast_to_clan(
            clan_id2,
            &SystemMessage::new(SystemMessageId::TheClanS1HasDecidedToStopTheWar)
                .with(SystemMessageParam::Text(name1)),
        );
    }

    /// The clan reputation part of Java `Player.doDie(killer)` (outside PvP and siege zones; the
    /// caller checks them): a player killed in a clan war by a member of the enemy clan costs its
    /// clan reputation and gives it to the killer's clan.
    pub(crate) fn clan_war_kill_reputation(&mut self, dying: ObjectId, killer: ObjectId) {
        // `pk = killer.asPlayer()` (a summon's owner once summons exist).
        if !self.players.contains_key(&killer) {
            return;
        }
        let (own_id, pk_id) = (self.clan_id_of(dying), self.clan_id_of(killer));
        let (Some(own), Some(pk)) = (self.clan(own_id), self.clan(pk_id)) else {
            return;
        };
        if self.is_academy_member(dying) || self.is_academy_member(killer) {
            return;
        }
        // `|| (isInSiege() && pk.isInSiege())` (M7-SIEGE-01).
        let in_siege = self.is_in_siege(dying) && self.is_in_siege(killer);
        if !((own.is_at_war_with(pk_id) && pk.is_at_war_with(own_id)) || in_siege) {
            return;
        }
        if !self.anti_feed_check(killer, dying) {
            return;
        }
        let per_kill = self.clans.settings.reputation_score_per_kill;
        // "When your reputation score is 0 or below, the other clan cannot acquire any
        // reputation points."
        if own.reputation_score > 0 {
            self.add_reputation_score(pk_id, per_kill);
        }
        // "when the opposing sides reputation score is 0 or below, your clans reputation score
        // does not decrease" (read after the addition above, like Java)
        if self.clan(pk_id).is_some_and(|c| c.reputation_score > 0) {
            self.take_reputation_score(own_id, per_kill);
        }
    }

    /// `member.broadcastUserInfo()` for the online members of both clans.
    fn broadcast_user_info_of_clans(&mut self, clan_id1: i32, clan_id2: i32) {
        for member in self.clan_online_members(clan_id1, 0) {
            self.broadcast_user_info(member);
        }
        for member in self.clan_online_members(clan_id2, 0) {
            self.broadcast_user_info(member);
        }
    }

    /// A refusal: the message and `ActionFailed.STATIC_PACKET`.
    fn war_refused(&self, player: ObjectId, id: SystemMessageId) {
        self.send(player, &SystemMessage::new(id));
        self.send(player, &ActionFailed);
    }

    /// Java `RequestStartPledgeWar.runImpl`.
    pub(crate) fn on_request_start_pledge_war(&mut self, player: ObjectId, pledge_name: &str) {
        let clan_id = self.clan_id_of(player);
        let Some(player_clan) = self.clan(clan_id) else {
            return;
        };
        let members_for_war = i64::from(self.clans.settings.alt_clan_members_for_war);
        let enough = |level: i32, members: usize| {
            level >= 3 && i64::try_from(members).unwrap_or(i64::MAX) >= members_for_war
        };
        let (level, count, ally_id) = (
            player_clan.level,
            player_clan.members_count(),
            player_clan.ally_id,
        );
        if !enough(level, count) {
            self.war_refused(
                player,
                SystemMessageId::AClanWarCanOnlyBeDeclaredIfTheClanIsLevel3OrAboveAndTheNumberOfClanMembersIsFifteenOrGreater,
            );
            return;
        } else if !self.has_clan_access(player, ClanAccess::WarDeclaration) {
            self.war_refused(player, SystemMessageId::YouAreNotAuthorizedToDoThat);
            return;
        }

        let Some(clan) = self.clan_by_name(pledge_name) else {
            self.war_refused(
                player,
                SystemMessageId::AClanWarCannotBeDeclaredAgainstAClanThatDoesNotExist,
            );
            return;
        };
        let (target_id, target_name) = (clan.id, clan.name.clone());
        if ally_id == clan.ally_id && ally_id != 0 {
            self.war_refused(
                player,
                SystemMessageId::ADeclarationOfClanWarAgainstAnAlliedClanCanTBeMade,
            );
            return;
        } else if !enough(clan.level, clan.members_count()) {
            self.war_refused(
                player,
                SystemMessageId::AClanWarCanOnlyBeDeclaredIfTheClanIsLevel3OrAboveAndTheNumberOfClanMembersIsFifteenOrGreater,
            );
            return;
        } else if self.clan(clan_id).is_some_and(|c| c.is_at_war_with(target_id)) {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::YouHaveAlreadyBeenAtWarWithTheS1Clan5DaysMustPassBeforeYouCanDeclareWarAgain,
                )
                .with(SystemMessageParam::Text(target_name)),
            );
            self.send(player, &ActionFailed);
            return;
        }

        self.store_clan_wars(clan_id, target_id);
        self.broadcast_user_info_of_clans(clan_id, target_id);
    }

    /// Java `RequestReplyStartPledgeWar.runImpl`.
    pub(crate) fn on_request_reply_start_pledge_war(&mut self, player: ObjectId, answer: i32) {
        let Some(requestor) = self.active_requester(player) else {
            return;
        };
        if answer == 1 {
            self.store_clan_wars(self.clan_id_of(requestor), self.clan_id_of(player));
        } else {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::TheS1ClanDidNotRespondWarProclamationHasBeenRefused2),
            );
        }
        self.set_active_requester(player, None);
        self.on_transaction_response(requestor);
    }

    /// Java `RequestStopPledgeWar.runImpl`.
    pub(crate) fn on_request_stop_pledge_war(&mut self, player: ObjectId, pledge_name: &str) {
        let player_clan_id = self.clan_id_of(player);
        if self.clan(player_clan_id).is_none() {
            return;
        }
        let Some(clan_id) = self.clan_by_name(pledge_name).map(|c| c.id) else {
            self.send_message(player, "No such clan.");
            self.send(player, &ActionFailed);
            return;
        };
        if !self
            .clan(player_clan_id)
            .is_some_and(|c| c.is_at_war_with(clan_id))
        {
            self.send_message(player, "You aren't at war with this clan.");
            self.send(player, &ActionFailed);
            return;
        }
        // Check if player who does the request has the correct rights to do it.
        if !self.has_clan_access(player, ClanAccess::WarDeclaration) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouAreNotAuthorizedToDoThat),
            );
            return;
        }
        // Members that are not in the world have no player in Java either.
        let in_battle = self
            .clan(player_clan_id)
            .map(|c| c.members())
            .unwrap_or_default()
            .into_iter()
            .filter(|m| m.has_player)
            .any(|m| self.has_attack_stance_task(m.object_id));
        if in_battle {
            self.send(
                player,
                &SystemMessage::new(
                    SystemMessageId::ACeaseFireDuringAClanWarCanNotBeCalledWhileMembersOfYourClanAreEngagedInBattle,
                ),
            );
            return;
        }

        self.delete_clan_wars(player_clan_id, clan_id);
        self.broadcast_user_info_of_clans(player_clan_id, clan_id);
    }

    /// Java `RequestReplyStopPledgeWar.runImpl`.
    pub(crate) fn on_request_reply_stop_pledge_war(&mut self, player: ObjectId, answer: i32) {
        let Some(requestor) = self.active_requester(player) else {
            return;
        };
        if answer == 1 {
            self.delete_clan_wars(self.clan_id_of(requestor), self.clan_id_of(player));
        } else {
            self.send(
                requestor,
                &SystemMessage::new(SystemMessageId::RequestToEndWarHasBeenDenied),
            );
        }
        self.set_active_requester(player, None);
        self.on_transaction_response(requestor);
    }

    /// Java `RequestSurrenderPledgeWar.runImpl`.
    pub(crate) fn on_request_surrender_pledge_war(&mut self, player: ObjectId, pledge_name: &str) {
        let player_clan_id = self.clan_id_of(player);
        if self.clan(player_clan_id).is_none() {
            return;
        }
        let Some(clan_id) = self.clan_by_name(pledge_name).map(|c| c.id) else {
            self.send_message(player, "No such clan.");
            self.send(player, &ActionFailed);
            return;
        };
        info!(
            "RequestSurrenderPledgeWar by {} with {pledge_name}",
            self.clan_display(player_clan_id)
        );
        if !self
            .clan(player_clan_id)
            .is_some_and(|c| c.is_at_war_with(clan_id))
        {
            self.send_message(player, "You aren't at war with this clan.");
            self.send(player, &ActionFailed);
            return;
        }
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveSurrenderedToTheS1Clan)
                .with(SystemMessageParam::Text(pledge_name.to_owned())),
        );
        self.delete_clan_wars(player_clan_id, clan_id);
    }

    /// Java `RequestReplySurrenderPledgeWar.runImpl`.
    pub(crate) fn on_request_reply_surrender_pledge_war(
        &mut self,
        player: ObjectId,
        req_name: &str,
        answer: i32,
    ) {
        let Some(requestor) = self.active_requester(player) else {
            return;
        };
        if answer == 1 {
            self.delete_clan_wars(self.clan_id_of(requestor), self.clan_id_of(player));
        } else {
            info!(
                "RequestReplySurrenderPledgeWar: Missing implementation for answer: {answer} and name: {req_name}!"
            );
        }
        // NOTE(java-quirk): `player.onTransactionRequest(requestor)`, not the response: the
        // requestor starts a new request on the player and the active requester is not cleared.
        self.on_transaction_request(player, requestor);
    }

    /// Java `RequestPledgeWarList.runImpl`.
    pub(crate) fn on_request_pledge_war_list(&mut self, player: ObjectId, tab: i32) {
        let Some(clan) = self.clan_of(player) else {
            return;
        };
        let list = if tab == 0 {
            &clan.at_war_with
        } else {
            &clan.at_war_attackers
        };
        // `ConcurrentHashMap.newKeySet()`: the order of the hash table.
        let clan_names = java_map_order(list, list.len())
            .into_iter()
            .filter_map(|i| list.get(i))
            .filter_map(|id| self.clan(*id))
            .map(|c| c.name.clone())
            .collect();
        let packet = PledgeReceiveWarList {
            tab,
            list_size: i32::try_from(list.len()).unwrap_or(i32::MAX),
            clan_names,
        };
        self.send(player, &packet);
    }
}
