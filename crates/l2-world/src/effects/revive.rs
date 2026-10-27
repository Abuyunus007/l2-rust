//! Resurrection requests: a player who is dead gets a "do you want to be resurrected" dialog
//! (`ConfirmDlg`) from the `Resurrection` and `ResurrectionSpecial` effects and answers it with
//! `DlgAnswer`.
// Java: entity/actor/Player.java (reviveRequest, reviveAnswer, isReviveRequested, isRevivingPet,
//       removeReviving, hasCharmOfCourage), network/clientpackets/DlgAnswer.java (the two
//       resurrection questions), mechanics/stats/Formulas.java (calculateSkillResurrectRestorePercent)
//
// A request "for the pet" revives the player's dead pet (`summon/pet.rs`); without one it does
// nothing, as in Java.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ConfirmDlg, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::formulas::calculate_skill_resurrect_restore_percent;
use crate::stats::{BaseStat, java_round};
use crate::world::World;

/// How long the charm of courage question stays on the screen (`dlg.addTime(60000)`).
const CHARM_DIALOG_MS: i32 = 60_000;

impl World {
    /// Java `Formulas.calculateSkillResurrectRestorePercent(baseRestorePercent, caster)`.
    pub(crate) fn skill_resurrect_restore_percent(&self, base: f64, caster: ObjectId) -> f64 {
        let wit_bonus = self.stats(caster).map_or(1.0, |v| v.bonus(BaseStat::Wit));
        calculate_skill_resurrect_restore_percent(base, wit_bonus)
    }

    /// Java `Player.hasCharmOfCourage()`.
    // NOTE: the charm of courage item handler (`CharmOfCourage`) is not ported, so nobody has one.
    pub fn has_charm_of_courage(&self, _id: ObjectId) -> bool {
        false
    }

    /// Java `Player.isReviveRequested()`.
    pub fn is_revive_requested(&self, id: ObjectId) -> bool {
        self.control(id).is_some_and(|c| c.revive_requested)
    }

    /// Java `Player.isRevivingPet()`.
    pub fn is_reviving_pet(&self, id: ObjectId) -> bool {
        self.control(id).is_some_and(|c| c.revive_pet)
    }

    /// Java `Player.removeReviving()`.
    pub fn remove_reviving(&mut self, id: ObjectId) {
        if let Some(control) = self.control_mut(id) {
            control.revive_requested = false;
            control.revive_power = 0.0;
        }
    }

    /// Java `Player.reviveRequest(reviver, pet, power)`: asks the player whether to come back.
    pub fn revive_request(&mut self, player: ObjectId, reviver: ObjectId, pet: bool, power: i32) {
        if !self.is_player(player) {
            return;
        }
        // Java `isResurrectionBlocked()`.
        if self.is_affected(player, crate::effects::effect_flag::BLOCK_RESURRECTION) {
            return;
        }
        if let Some(control) = self.control(player)
            && control.revive_requested
        {
            let message = if control.revive_pet == pet {
                // Resurrection is already been proposed.
                SystemMessageId::ResurrectionHasAlreadyBeenProposed
            } else if pet {
                SystemMessageId::APetCannotBeResurrectedWhileItSOwnerIsInTheProcessOfResurrecting
            } else {
                SystemMessageId::WhileAPetIsBeingResurrectedItCannotHelpInResurrectingItsMaster
            };
            self.send(reviver, &SystemMessage::new(message));
            return;
        }
        // `(pet && hasPet() && _summon.isDead()) || (!pet && isDead())`.
        let valid = if pet {
            self.summon_of(player)
                .is_some_and(|s| self.is_pet(s) && self.is_dead(s))
        } else {
            self.is_dead(player)
        };
        if !valid {
            return;
        }
        let revive_power = self.skill_resurrect_restore_percent(f64::from(power), reviver);
        let exp_before_death = self
            .players
            .get(&player)
            .map_or(0, |p| p.data.profile.exp_before_death);
        let exp = self.players.get(&player).map_or(0, |p| p.data.exp);
        // `(int) Math.round(((_expBeforeDeath - getExp()) * _revivePower) / 100)`.
        let restore_exp = java_round(((exp_before_death - exp) as f64 * revive_power) / 100.0) as i32;
        if let Some(control) = self.control_mut(player) {
            control.revive_requested = true;
            control.revive_power = revive_power;
            control.revive_pet = pet;
        }
        if self.has_charm_of_courage(player) {
            let dialog = ConfirmDlg::new(
                SystemMessageId::YourCharmOfCourageIsTryingToResurrectYouWouldYouLikeToResurrectNow,
            )
            .with_time(CHARM_DIALOG_MS);
            self.send(player, &dialog);
            return;
        }
        let dialog = ConfirmDlg::new(
            SystemMessageId::C1IsMakingAnAttemptToResurrectYouIfYouChooseThisPathS2ExperiencePointsWillBeReturnedToYouDoYouWantToBeResurrected,
        )
        .with(SystemMessageParam::PlayerName(self.creature_name(reviver)))
        .with(SystemMessageParam::Text(restore_exp.to_string()));
        self.send(player, &dialog);
    }

    /// Java `Player.reviveAnswer(answer)`.
    pub fn revive_answer(&mut self, player: ObjectId, answer: i32) {
        let Some(control) = self.control(player) else {
            return;
        };
        let (requested, power, pet) = (control.revive_requested, control.revive_power, control.revive_pet);
        // `(!isDead() && !_revivePet) || (_revivePet && hasPet() && !_summon.isDead())`.
        let living_pet = self
            .summon_of(player)
            .is_some_and(|s| self.is_pet(s) && !self.is_dead(s));
        if !requested || (!self.is_dead(player) && !pet) || (pet && living_pet) {
            return;
        }
        if answer == 1 {
            if !pet {
                if power != 0.0 {
                    self.do_revive_with_power(player, power);
                } else {
                    self.do_revive(player);
                }
            } else if let Some(summon) = self.summon_of(player).filter(|s| self.is_pet(*s)) {
                if power != 0.0 {
                    self.pet_do_revive_with_power(summon, power);
                } else {
                    self.pet_do_revive(summon);
                }
            }
        }
        if let Some(control) = self.control_mut(player) {
            control.revive_pet = false;
            control.revive_requested = false;
            control.revive_power = 0.0;
        }
    }

    /// Java `DlgAnswer.runImpl`: the answers the world handles (the two resurrection questions and
    /// the confirmation of an admin command, the gate questions of M7-FU-03). The dialogs of the
    /// other branches (offline commands, engagement, summon requests) are not sent yet.
    pub(crate) fn on_dlg_answer(
        &mut self,
        player: ObjectId,
        message_id: i32,
        answer: i32,
        _requester_id: i32,
    ) {
        if !self.is_player(player) {
            return;
        }
        // ON_PLAYER_DLG_ANSWER script listeners: D-05.
        if message_id == SystemMessageId::S13.id() {
            // The offline and zone-builder questions share this id (not sent yet); the
            // engagement (`wedding.rs`) and the admin command confirmation (`admin/mod.rs`) are.
            if self.wedding_dlg_answer(player, answer) {
                return;
            }
            self.admin_confirm_answer(player, answer);
        } else if message_id
            == SystemMessageId::C1IsMakingAnAttemptToResurrectYouIfYouChooseThisPathS2ExperiencePointsWillBeReturnedToYouDoYouWantToBeResurrected
                .id()
            || message_id == SystemMessageId::YourCharmOfCourageIsTryingToResurrectYouWouldYouLikeToResurrectNow.id()
        {
            self.revive_answer(player, answer);
        } else if message_id == SystemMessageId::WouldYouLikeToOpenTheGate.id() {
            self.door_dialog_answer(player, answer, true);
        } else if message_id == SystemMessageId::WouldYouLikeToCloseTheGate.id() {
            self.door_dialog_answer(player, answer, false);
        }
    }
}
