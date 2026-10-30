//! The player's own actions of `RequestActionUse`: sit and stand, walk and run, the social
//! emotes, the couple actions and the answer to one (`AnswerCoupleAction`).
// Java: network/clientpackets/RequestActionUse.java (the sit, run, social and couple branches,
//       useSit, tryBroadcastSocial, useCoupleSocial)
// Java: network/clientpackets/AnswerCoupleAction.java
// Java: entity/actor/Player.java (canMakeSocialAction, setMultiSocialAction)
// Java: ai/NextAction.java, ai/AbstractAI.java (setNextAction), ai/CreatureAI.java
//       (notifyActionArrived, notifyActionFinishCasting, setIntentionCast: the triggers and the
//       removal of a `NextAction`)
//
// # Where it plugs in
// - `summon/actions.rs` runs the checks every action goes through (`RequestActionUse.runImpl`)
//   and hands every action it does not know as a summon action to [`World::on_player_action`].
//   The store actions (10, 28, 61) are `trade/store.rs`.
// - `AnswerCoupleAction` is `ClientInput::AnswerCoupleAction`.
//
// # NextAction
// Java keeps one `NextAction` in the AI of a creature: a callback that runs when an event happens
// (`ARRIVED`, `FINISH_CASTING`), removed when the AI gets a given intention. The callbacks of
// this module are the two of `RequestActionUse` (sit when the walk is over, ask for the couple
// action when the walk or the cast is over), so they are an enum, kept in the creature's control
// state (`CreatureControl::next_action`). The callbacks of the skill packets (`NextAction` of
// `RequestMagicSkillUse`, ...) are not ported (see `ai/mod.rs`).
//
// # Left out (markers)
// - Action 38 (mount/dismount) is `mount.rs`; the airship actions 67 to 70 are `vehicle/input.rs`.
// - Action 65 (the bot report button) is `bot_report.rs`.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ChairSit, ExAskCoupleAction, ExRotation, SocialAction, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;

use crate::effects::EffectType;
use crate::movement::{Intention, heading_from_points};
use crate::target::distance_2d;
use crate::world::World;

/// `StaticObject.INTERACTION_DISTANCE`.
const STATIC_OBJECT_INTERACTION_DISTANCE: f64 = 150.0;
/// `StaticObject.getType()` of a throne, bench or chair.
const STATIC_OBJECT_TYPE_CHAIR: i32 = 1;
/// The farthest a couple action partner may be (`distance > 125`).
const COUPLE_MAX_DISTANCE: i32 = 125;
/// The closest a couple action partner may be (`distance < 15`).
const COUPLE_MIN_DISTANCE: i32 = 15;
/// `RequestActionUse` ids 71 to 73 are the couple social actions 16 to 18 (`_actionId - 55`).
const COUPLE_ACTION_OFFSET: i32 = 55;

/// What `RequestActionUse` ids 12 to 66 play as a `SocialAction`: `(action id, social id)`.
const SOCIAL_ACTIONS: [(i32, i32); 14] = [
    (12, 2),  // Greeting
    (13, 3),  // Victory
    (14, 4),  // Advance
    (24, 6),  // Yes
    (25, 5),  // No
    (26, 7),  // Bow
    (29, 8),  // Unaware
    (30, 9),  // Social Waiting
    (31, 10), // Laugh
    (33, 11), // Applaud
    (34, 12), // Dance
    (35, 13), // Sorrow
    (62, 14), // Charm
    (66, 15), // Shyness
];

/// Java `Action`: the AI events a `NextAction` waits for (only the two this module uses).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ActionTrigger {
    Arrived,
    FinishCasting,
}

/// The callback of a `NextAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NextActionCall {
    /// `() -> useSit(player, target)`: the target is the one the player had when it asked.
    Sit { target: Option<ObjectId> },
    /// `() -> partner.sendPacket(new ExAskCoupleAction(player.getObjectId(), id))`.
    AskCouple { partner: ObjectId, action_id: i32 },
}

/// Java `NextAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct NextAction {
    /// `_action`: what triggers it.
    pub trigger: ActionTrigger,
    /// `_intention`: the intention that removes it.
    pub removed_by: Intention,
    pub call: NextActionCall,
}

/// Java `Player._multiSociaAction` and `_multiSocialTarget` (the couple action asked for).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct MultiSocial {
    pub action_id: i32,
    pub target: i32,
}

/// What `RequestActionUse` does with the actions of this module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlayerAction {
    SitStand,
    WalkRun,
    Social(i32),
    CoupleSocial(i32),
    /// 65: `BotReportTable.reportBot` (or "This feature is disabled.").
    BotReport,
}

fn player_action(action_id: i32) -> Option<PlayerAction> {
    match action_id {
        0 => Some(PlayerAction::SitStand),
        1 => Some(PlayerAction::WalkRun),
        65 => Some(PlayerAction::BotReport),
        71..=73 => Some(PlayerAction::CoupleSocial(action_id - COUPLE_ACTION_OFFSET)),
        other => SOCIAL_ACTIONS
            .iter()
            .find(|(id, _)| *id == other)
            .map(|(_, social)| PlayerAction::Social(*social)),
    }
}

fn pc_name(name: &str) -> SystemMessageParam {
    SystemMessageParam::PlayerName(name.to_owned())
}

impl World {
    /// The branches of `RequestActionUse.runImpl` that belong to the player: sit/stand (0),
    /// walk/run (1), the socials and the couple actions. `false` when the id is none of them.
    pub(crate) fn on_player_action(
        &mut self,
        player: ObjectId,
        action_id: i32,
        target: Option<ObjectId>,
    ) -> bool {
        let Some(action) = player_action(action_id) else {
            return false;
        };
        match action {
            // Sit/Stand
            PlayerAction::SitStand => {
                if self.is_sitting(player) || !self.is_moving(player) || self.is_fake_death(player) {
                    self.use_sit(player, target);
                } else {
                    // Sit when arrive using next action.
                    self.set_next_action(
                        player,
                        NextAction {
                            trigger: ActionTrigger::Arrived,
                            removed_by: Intention::MoveTo,
                            call: NextActionCall::Sit { target },
                        },
                    );
                }
            }
            // Walk/Run
            PlayerAction::WalkRun => {
                let running = self.is_running(player);
                self.set_running_effect(player, !running);
            }
            PlayerAction::Social(social_id) => self.try_broadcast_social(player, social_id),
            PlayerAction::CoupleSocial(social_id) => self.use_couple_social(player, social_id),
            PlayerAction::BotReport => self.on_bot_report_action(player, self.target_of(player)),
        }
        true
    }

    // ------------------------------------------------------------------ sit

    /// Java `RequestActionUse.useSit`: sit on a throne, stand a fake death up, stand up or sit
    /// down.
    fn use_sit(&mut self, player: ObjectId, target: Option<ObjectId>) {
        // `player.getMountType() != MountType.NONE` returns false.
        if self.is_mounted(player) {
            return;
        }
        let chair = target.and_then(|t| {
            let object = self.static_objects.get(&t)?;
            (object.kind == STATIC_OBJECT_TYPE_CHAIR).then_some((object.static_object_id, t))
        });
        if let Some((static_id, chair)) = chair
            && !self.is_sitting(player)
            && self.is_inside_radius_2d(player, chair, STATIC_OBJECT_INTERACTION_DISTANCE)
        {
            let packet = ChairSit {
                object_id: player.0,
                static_object_id: static_id,
            };
            self.send(player, &packet);
            self.sit_down(player, true);
            // NOTE(java-quirk): `broadcastPacket` of a player also sends it to the player, who
            // gets the packet twice.
            self.broadcast_creature_packet(player, &packet);
            return;
        }
        if self.is_fake_death(player) {
            self.stop_effects(player, EffectType::FakeDeath);
        } else if self.is_sitting(player) {
            self.stand_up(player);
        } else {
            self.sit_down(player, true);
        }
    }

    /// Java `Creature.isInsideRadius2D(object, radius)`: strictly inside.
    pub(crate) fn is_inside_radius_2d(&self, id: ObjectId, other: ObjectId, radius: f64) -> bool {
        match (self.position_of(id), self.position_of(other)) {
            (Some(a), Some(b)) => distance_2d(a, b) < radius,
            _ => false,
        }
    }

    // ------------------------------------------------------------------ socials

    /// Java `Player.canMakeSocialAction`.
    fn can_make_social_action(&mut self, player: ObjectId) -> bool {
        !self.is_in_store_mode(player)
            && self.active_requester(player).is_none()
            && !self.is_alike_dead(player)
            && !self.is_all_skills_disabled(player)
            && !self.is_casting_now(player)
            && !self.is_casting_simultaneously_now(player)
            && self.intention_of(player) == Intention::Idle
    }

    /// Java `RequestActionUse.tryBroadcastSocial`.
    fn try_broadcast_social(&mut self, player: ObjectId, social_id: i32) {
        if self.is_fishing(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCannotDoThatWhileFishing3),
            );
            return;
        }
        if self.can_make_social_action(player) {
            self.broadcast_creature_packet(
                player,
                &SocialAction {
                    object_id: player.0,
                    action_id: social_id,
                },
            );
        }
    }

    // ------------------------------------------------------------------ couple actions

    fn action_system_message(&self, to: ObjectId, id: SystemMessageId) {
        self.send(to, &SystemMessage::new(id));
    }

    /// `sm.addPcName(who); player.sendPacket(sm)`.
    fn couple_refusal(&self, to: ObjectId, id: SystemMessageId, who: ObjectId) {
        let name = self.creature_name(who);
        self.send(to, &SystemMessage::new(id).with(pc_name(&name)));
    }

    /// Java `Player.getMultiSociaAction`.
    fn multi_social_action_of(&self, id: ObjectId) -> i32 {
        self.control(id).map_or(0, |c| c.multi_social.action_id)
    }

    /// Java `Player.setMultiSocialAction(id, targetId)`.
    fn set_multi_social_action(&mut self, id: ObjectId, action_id: i32, target: i32) {
        if let Some(control) = self.control_mut(id) {
            control.multi_social = MultiSocial { action_id, target };
        }
    }

    /// Java `RequestActionUse.useCoupleSocial`.
    fn use_couple_social(&mut self, player: ObjectId, action_id: i32) {
        use SystemMessageId as M;
        let target = self.target_of(player).filter(|t| self.is_player(*t));
        let Some(partner) = target else {
            self.action_system_message(player, M::InvalidTarget);
            return;
        };
        let distance = match (self.position_of(player), self.position_of(partner)) {
            (Some(a), Some(b)) => distance_2d(a, b) as i32,
            _ => return,
        };
        if !(COUPLE_MIN_DISTANCE..=COUPLE_MAX_DISTANCE).contains(&distance) || player == partner {
            self.action_system_message(
                player,
                M::TheRequestCannotBeCompletedBecauseTheTargetDoesNotMeetLocationRequirements,
            );
            return;
        }

        if self.is_in_store_mode(player) || self.is_crafting(player) {
            self.couple_refusal(
                player,
                M::C1IsInPrivateShopModeOrInABattleAndCannotBeRequestedForACoupleAction,
                player,
            );
            return;
        }
        if self.is_in_combat(player) || self.is_in_duel(player) || self.has_attack_stance_task(player) {
            self.couple_refusal(
                player,
                M::C1IsInABattleAndCannotBeRequestedForACoupleAction,
                player,
            );
            return;
        }
        if self.is_fishing(player) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCannotDoThatWhileFishing3),
            );
            return;
        }
        if self.karma_of_player(player) > 0 {
            self.couple_refusal(
                player,
                M::C1IsInAChaoticStateAndCannotBeRequestedForACoupleAction,
                player,
            );
            return;
        }
        // `player.isInOlympiadMode()` (M8-OLY-01).
        if self.is_in_olympiad_mode(player) {
            self.couple_refusal(
                player,
                M::C1IsParticipatingInTheOlympiadAndCannotBeRequestedForACoupleAction,
                player,
            );
            return;
        }
        if self.is_in_siege(player) {
            self.couple_refusal(
                player,
                M::C1IsInACastleSiegeAndCannotBeRequestedForACoupleAction,
                player,
            );
            return;
        }
        // NOTE(java-quirk): the `isInHideoutSiege()` branch of the player has no `return`, the
        // checks go on after the message.
        if self.is_in_hideout_siege(player) {
            self.couple_refusal(
                player,
                M::C1IsParticipatingInAHideoutSiegeAndCannotBeRequestedForACoupleAction,
                player,
            );
        }
        // `isMounted() || isFlyingMounted() || isInBoat() || isInAirShip()`.
        if self.is_in_boat(player)
            || self.is_in_air_ship(player)
            || self
                .stats(player)
                .is_some_and(|v| v.is_flying_mounted() || v.is_mounted())
        {
            self.couple_refusal(
                player,
                M::C1IsRidingAShipSteedOrStriderAndCannotBeRequestedForACoupleAction,
                player,
            );
            return;
        }
        if self.is_transformed(player) {
            self.couple_refusal(
                player,
                M::C1IsCurrentlyTransformingAndCannotBeRequestedForACoupleAction,
                player,
            );
            return;
        }
        if self.is_alike_dead(player) {
            self.couple_refusal(
                player,
                M::C1IsCurrentlyDeadAndCannotBeRequestedForACoupleAction,
                player,
            );
            return;
        }

        // Checks for partner.
        if self.is_in_store_mode(partner) || self.is_crafting(partner) {
            self.couple_refusal(
                player,
                M::C1IsInPrivateShopModeOrInABattleAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }
        if self.is_in_combat(partner) || self.is_in_duel(partner) || self.has_attack_stance_task(partner) {
            self.couple_refusal(
                player,
                M::C1IsInABattleAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }
        if self.multi_social_action_of(partner) > 0 {
            self.couple_refusal(
                player,
                M::C1IsAlreadyParticipatingInACoupleActionAndCannotBeRequestedForAnotherCoupleAction,
                partner,
            );
            return;
        }
        if self.is_fishing(partner) {
            self.couple_refusal(
                player,
                M::C1IsFishingAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }
        if self.karma_of_player(partner) > 0 {
            self.couple_refusal(
                player,
                M::C1IsInAChaoticStateAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }
        // `partner.isInOlympiadMode()` (M8-OLY-01).
        if self.is_in_olympiad_mode(partner) {
            self.couple_refusal(
                player,
                M::C1IsParticipatingInTheOlympiadAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }
        if self.is_in_hideout_siege(partner) {
            self.couple_refusal(
                player,
                M::C1IsParticipatingInAHideoutSiegeAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }
        if self.is_in_siege(partner) {
            self.couple_refusal(
                player,
                M::C1IsInACastleSiegeAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }
        // `partner.isMounted() || partner.isFlyingMounted() || partner.isInBoat() ||
        // partner.isInAirShip()`.
        if self.is_in_boat(partner)
            || self.is_in_air_ship(partner)
            || self
                .stats(partner)
                .is_some_and(|v| v.is_flying_mounted() || v.is_mounted())
        {
            self.couple_refusal(
                player,
                M::C1IsRidingAShipSteedOrStriderAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }
        if self.is_teleporting(partner) {
            self.couple_refusal(
                player,
                M::C1IsCurrentlyTeleportingAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }
        if self.is_transformed(partner) {
            self.couple_refusal(
                player,
                M::C1IsCurrentlyTransformingAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }
        if self.is_alike_dead(partner) {
            self.couple_refusal(
                player,
                M::C1IsCurrentlyDeadAndCannotBeRequestedForACoupleAction,
                partner,
            );
            return;
        }

        if self.is_all_skills_disabled(player) || self.is_all_skills_disabled(partner) {
            self.action_system_message(player, M::TheCoupleActionWasCancelled);
            return;
        }

        self.set_multi_social_action(player, action_id, partner.0);
        self.couple_refusal(player, M::YouHaveRequestedACoupleActionWithC1, partner);

        let ask = NextActionCall::AskCouple { partner, action_id };
        if self.intention_of(player) != Intention::Idle || self.intention_of(partner) != Intention::Idle {
            self.set_next_action(
                player,
                NextAction {
                    trigger: ActionTrigger::Arrived,
                    removed_by: Intention::MoveTo,
                    call: ask,
                },
            );
            return;
        }
        if self.is_casting_now(player) || self.is_casting_simultaneously_now(player) {
            self.set_next_action(
                player,
                NextAction {
                    trigger: ActionTrigger::FinishCasting,
                    removed_by: Intention::Cast,
                    call: ask,
                },
            );
            return;
        }
        self.run_next_action_call(player, ask);
    }

    /// Java `AnswerCoupleAction.runImpl`.
    pub(crate) fn on_answer_couple_action(
        &mut self,
        player: ObjectId,
        action_id: i32,
        answer: i32,
        object_id: i32,
    ) {
        let target = ObjectId(object_id);
        if !self.players.contains_key(&player) || !self.players.contains_key(&target) {
            return;
        }
        let asked = self
            .control(target)
            .map_or(MultiSocial::default(), |c| c.multi_social);
        if asked.target != player.0 || asked.action_id != action_id {
            return;
        }
        match answer {
            // cancel
            0 => self.action_system_message(target, SystemMessageId::TheCoupleActionWasDenied),
            // approve
            1 => {
                let (Some(from), Some(to)) = (self.position_of(player), self.position_of(target)) else {
                    return;
                };
                let distance = distance_2d(from, to) as i32;
                if !(COUPLE_MIN_DISTANCE..=COUPLE_MAX_DISTANCE).contains(&distance) || player == target {
                    let id = SystemMessageId::TheRequestCannotBeCompletedBecauseTheTargetDoesNotMeetLocationRequirements;
                    self.action_system_message(player, id);
                    self.action_system_message(target, id);
                    // NOTE(java-quirk): this `return` skips the `setMultiSocialAction(0, 0)` at
                    // the end, the asker keeps the request until the next answer or relog.
                    return;
                }
                let heading = heading_from_points(from.0, from.1, to.0, to.1);
                self.broadcast_creature_packet(
                    player,
                    &ExRotation {
                        char_id: player.0,
                        heading,
                    },
                );
                self.set_heading(player, heading);
                let heading = heading_from_points(to.0, to.1, from.0, from.1);
                self.set_heading(target, heading);
                self.broadcast_creature_packet(
                    target,
                    &ExRotation {
                        char_id: target.0,
                        heading,
                    },
                );
                self.broadcast_creature_packet(
                    player,
                    &SocialAction {
                        object_id: player.0,
                        action_id,
                    },
                );
                self.broadcast_creature_packet(target, &SocialAction { object_id, action_id });
            }
            // refused
            -1 => self.couple_refusal(
                target,
                SystemMessageId::C1IsSetToRefuseCoupleActionsAndCannotBeRequestedForACoupleAction,
                player,
            ),
            _ => {}
        }
        self.set_multi_social_action(target, 0, 0);
    }

    // ------------------------------------------------------------------ NextAction

    /// Java `AbstractAI.setNextAction`.
    fn set_next_action(&mut self, id: ObjectId, next_action: NextAction) {
        if let Some(control) = self.control_mut(id) {
            control.next_action = Some(next_action);
        }
    }

    /// Java `CreatureAI.notifyActionArrived`, last lines (`finally`): the `ARRIVED` callback runs
    /// once. `canHandleAction()`: spawned (or teleporting).
    pub(crate) fn fire_arrived_next_action(&mut self, id: ObjectId) {
        if !self.can_handle_next_action(id) {
            return;
        }
        let Some(next) = self.control(id).and_then(|c| c.next_action) else {
            return;
        };
        if next.trigger != ActionTrigger::Arrived {
            return;
        }
        if let Some(control) = self.control_mut(id) {
            control.next_action = None;
        }
        self.run_next_action_call(id, next.call);
    }

    /// Java `CreatureAI.notifyActionFinishCasting`: the `FINISH_CASTING` callback runs.
    // NOTE(java-quirk): unlike `ARRIVED`, Java does not clear the action when it runs it. It stays
    // until a cast intention removes it (`setIntentionCast`) or another action replaces it.
    pub(crate) fn fire_finish_casting_next_action(&mut self, id: ObjectId) {
        if !self.can_handle_next_action(id) {
            return;
        }
        let Some(next) = self.control(id).and_then(|c| c.next_action) else {
            return;
        };
        if next.trigger == ActionTrigger::FinishCasting {
            self.run_next_action_call(id, next.call);
        }
    }

    /// `if (nextAction != null && nextAction.isRemovedBy(intention)) setNextAction(null)`, the
    /// `finally` of the `setIntention*` methods of `CreatureAI`.
    pub(crate) fn clear_next_action_removed_by(&mut self, id: ObjectId, intention: Intention) {
        if let Some(control) = self.effects.control.get_mut(&id)
            && control.next_action.is_some_and(|n| n.removed_by == intention)
        {
            control.next_action = None;
        }
    }

    /// Java `canHandleAction()`: `(isSpawned() || isTeleporting()) && hasAI()`.
    fn can_handle_next_action(&self, id: ObjectId) -> bool {
        let spawned = self.objects.get(&id).is_some_and(|e| e.spawned);
        spawned || self.is_teleporting(id)
    }

    fn run_next_action_call(&mut self, player: ObjectId, call: NextActionCall) {
        match call {
            NextActionCall::Sit { target } => self.use_sit(player, target),
            NextActionCall::AskCouple { partner, action_id } => {
                self.send(
                    partner,
                    &ExAskCoupleAction {
                        object_id: player.0,
                        action_id,
                    },
                );
            }
        }
    }
}
