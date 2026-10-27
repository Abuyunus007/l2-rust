//! The skill conditions of the sieges: who may build a headquarters, engrave the artefact or
//! summon a siege golem, and the `siegeZone` / `siegeSide` conditions.
// Java: mechanics/conditions/{ConditionPlayerCanCreateBase,ConditionPlayerCanTakeCastle,
//       ConditionPlayerCanSummonSiegeGolem,ConditionSiegeZone,ConditionPlayerSiegeSide}.java
// Java: mechanics/sevensigns/SevenSigns.java (checkSummonConditions)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::residence::{FortId, ResidenceRef};
use crate::world::World;
use crate::zones::ZoneId;

/// `ConditionSiegeZone.COND_NOT_ZONE` and its siblings.
const COND_NOT_ZONE: i32 = 0x0001;
const COND_CAST_ATTACK: i32 = 0x0002;
const COND_CAST_DEFEND: i32 = 0x0004;
const COND_CAST_NEUTRAL: i32 = 0x0008;
const COND_FORT_ATTACK: i32 = 0x0010;
const COND_FORT_DEFEND: i32 = 0x0020;
const COND_FORT_NEUTRAL: i32 = 0x0040;
const COND_TW_CHANNEL: i32 = 0x0080;
const COND_TW_PROGRESS: i32 = 0x0100;

impl World {
    /// The castle a player stands in (`CastleManager.getCastle(player)`) and the fortress
    /// (`FortManager.getFort(player)`).
    fn castle_and_fort_of(
        &self,
        id: ObjectId,
    ) -> (
        Option<crate::residence::CastleId>,
        Option<crate::residence::FortId>,
    ) {
        match self.position_of(id) {
            Some((x, y, z)) => (self.castle_at(x, y, z), self.fort_at(x, y, z)),
            None => (None, None),
        }
    }

    /// `S1_CANNOT_BE_USED_DUE_TO_UNSUITABLE_TERMS` with the skill's name.
    fn send_unsuitable_terms(&self, player: ObjectId, skill: Option<(i32, i32)>) {
        let mut message = SystemMessage::new(SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms);
        if let Some((id, level)) = skill {
            message = message.with(SystemMessageParam::SkillName { id, level });
        }
        self.send(player, &message);
    }

    /// Whether the player may build a headquarters now (the `canCreateBase` condition of Build
    /// Headquarters, with the skill whose name the messages carry).
    pub fn siege_create_headquarters_allowed(&self, player: ObjectId, skill: Option<(i32, i32)>) -> bool {
        self.cond_can_create_base(player, skill)
    }

    /// Java `ConditionPlayerCanCreateBase.testImpl`: `canCreateBase` (the player hears why not),
    /// for a castle siege and a fortress siege (M7-FORT-01's `fort_*`).
    // The `else if` chain of Java with its identical branches is kept to compare it line by line.
    #[allow(clippy::if_same_then_else)]
    pub(crate) fn cond_can_create_base(&self, player: ObjectId, skill: Option<(i32, i32)>) -> bool {
        let clan = self.clan_of(player).map(|c| c.id);
        // `isCursedWeaponEquipped()`: cursed weapons are M8.
        let mut can = !(self.is_alike_dead(player) || clan.is_none());
        let (castle, fort) = self.castle_and_fort_of(player);
        let siege = castle.and_then(|c| self.castle_siege(c));
        let fort_attacker = |f: FortId| clan.and_then(|c| self.fort_attacker_clan(f, c));
        let too_many = |flags: usize, max: i32| i32::try_from(flags).unwrap_or(i32::MAX) >= max;
        if castle.is_none() && fort.is_none() {
            self.send_unsuitable_terms(player, skill);
            can = false;
        } else if siege.is_some_and(|s| !s.in_progress)
            || fort.is_some_and(|f| !self.fort_siege_in_progress(f))
        {
            self.send_unsuitable_terms(player, skill);
            can = false;
        } else if siege.is_some_and(|s| clan.is_none_or(|c| s.attacker_clan(c).is_none()))
            || fort.is_some_and(|f| fort_attacker(f).is_none())
        {
            self.send_unsuitable_terms(player, skill);
            can = false;
        } else if !self.is_clan_leader(player) {
            self.send_unsuitable_terms(player, skill);
            can = false;
        } else if siege.is_some_and(|s| {
            clan.and_then(|c| s.attacker_clan(c))
                .is_some_and(|sc| too_many(sc.flags.len(), self.sieges.settings.flag_max_count))
        }) || fort.is_some_and(|f| {
            fort_attacker(f)
                .is_some_and(|sc| too_many(sc.flags.len(), self.fort_settings().siege.flag_max_count))
        }) {
            self.send_unsuitable_terms(player, skill);
            can = false;
        } else if !self.is_inside_zone(player, ZoneId::Hq) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouCannotSetUpABaseHere),
            );
            can = false;
        }
        can
    }

    /// Java `ConditionPlayerCanTakeCastle.testImpl`: a clan leader attacking the castle he stands
    /// in, next to (within 200, not below, at most 45 above) the castle's artefact. The defenders
    /// hear that the engraving started.
    pub fn cond_can_take_castle(
        &self,
        player: ObjectId,
        target: Option<ObjectId>,
        skill: Option<(i32, i32)>,
    ) -> bool {
        if !self.is_player(player) {
            return false;
        }
        // `isCursedWeaponEquipped()`: cursed weapons are M8.
        if self.is_alike_dead(player) || !self.is_clan_leader(player) {
            return false;
        }
        let clan = self.clan_of(player).map(|c| c.id);
        let (castle, _) = self.castle_and_fort_of(player);
        let usable = castle.filter(|c| c.0 > 0).is_some_and(|c| {
            self.castle_siege(c)
                .is_some_and(|s| s.in_progress && clan.is_some_and(|clan| s.attacker_clan(clan).is_some()))
        });
        let Some(castle) = castle.filter(|_| usable) else {
            self.send_unsuitable_terms(player, skill);
            return false;
        };
        let is_artefact = target.is_some_and(|t| {
            self.npc_template(t).is_some_and(|tpl| tpl.is_type("Artefact"))
                && self.npc_castle(t) == Some(castle)
        });
        let Some(target) = target.filter(|_| is_artefact) else {
            self.send(player, &SystemMessage::new(SystemMessageId::InvalidTarget));
            return false;
        };
        let (Some(me), Some(it)) = (self.position_of(player), self.position_of(target)) else {
            return false;
        };
        let in_range = self.check_if_in_range(200, player, target, true);
        if !in_range || me.2 < it.2 || (me.2 - it.2).abs() > 45 {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::TheDistanceIsTooFarAndSoTheCastingHasBeenStopped),
            );
            return false;
        }
        self.siege_announce_to_player(
            castle,
            &SystemMessage::new(SystemMessageId::TheOpposingClanHasStartedToEngraveTheHolyArtifact),
            false,
        );
        true
    }

    /// Java `ConditionPlayerCanTakeFort.testImpl`: a living clan leader attacking the fortress he
    /// stands in, within 200 of the fortress's flag pole. The player hears why not.
    // Java: mechanics/conditions/ConditionPlayerCanTakeFort.java
    pub fn cond_can_take_fort(
        &self,
        player: ObjectId,
        target: Option<ObjectId>,
        skill: Option<(i32, i32)>,
    ) -> bool {
        if !self.is_player(player) {
            return false;
        }
        // `isCursedWeaponEquipped()`: cursed weapons are M8.
        let mut can = !(self.is_alike_dead(player) || !self.is_clan_leader(player));
        let clan = self.clan_of(player).map(|c| c.id);
        let fort = self
            .fort_of_player(player)
            .filter(|f| f.0 > 0 && self.fort_siege_in_progress(*f))
            .filter(|f| clan.is_some_and(|c| self.fort_attacker_clan(*f, c).is_some()));
        if let Some(fort) = fort {
            // `fort.getFlagPole() != effected`
            if self.fort(fort).and_then(|f| f.flag_pole) != target {
                self.send(player, &SystemMessage::new(SystemMessageId::InvalidTarget));
                can = false;
            } else if !target.is_some_and(|t| self.check_if_in_range(200, player, t, true)) {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::TheDistanceIsTooFarAndSoTheCastingHasBeenStopped),
                );
                can = false;
            }
        } else {
            self.send_unsuitable_terms(player, skill);
            can = false;
        }
        can
    }

    /// Java `ConditionPlayerCanSummonSiegeGolem.testImpl`: `canSummonSiegeGolem` (castle and
    /// fortress sieges).
    #[allow(clippy::if_same_then_else)]
    pub(crate) fn cond_can_summon_siege_golem(&self, player: ObjectId) -> bool {
        let clan_id = self.clan_id_of(player);
        // `isCursedWeaponEquipped()`: cursed weapons are M8.
        let mut can = !(self.is_alike_dead(player) || self.clan(clan_id).is_none());
        let (castle, fort) = self.castle_and_fort_of(player);
        if castle.is_none() && fort.is_none() {
            can = false;
        }
        let siege = castle.and_then(|c| self.castle_siege(c));
        if fort.is_some_and(|f| f.0 == 0) || castle.is_some_and(|c| c.0 == 0) {
            self.send(player, &SystemMessage::new(SystemMessageId::InvalidTarget));
            can = false;
        } else if siege.is_some_and(|s| !s.in_progress)
            || fort.is_some_and(|f| !self.fort_siege_in_progress(f))
        {
            self.send(player, &SystemMessage::new(SystemMessageId::InvalidTarget));
            can = false;
        } else if clan_id != 0
            && (siege.is_some_and(|s| s.attacker_clan(clan_id).is_none())
                || fort.is_some_and(|f| self.fort_attacker_clan(f, clan_id).is_none()))
        {
            self.send(player, &SystemMessage::new(SystemMessageId::InvalidTarget));
            can = false;
        }
        // `SevenSigns.checkSummonConditions(player)`: a Dusk member while Dawn owns the Seal of
        // Strife in the seal validation period (M8-SS-01).
        else if self.ss_check_summon_conditions(player) {
            can = false;
        }
        can
    }

    /// Java `ConditionSiegeZone.testImpl` for `target` (the effector with `self="true"`).
    pub(crate) fn cond_siege_zone(&self, target: Option<ObjectId>, value: i32) -> bool {
        // `isTWInProgress()` and `isTWChannelOpen()` (M7-TW-01).
        if value & COND_TW_PROGRESS != 0 && !self.is_tw_in_progress() {
            return false;
        }
        if value & COND_TW_CHANNEL != 0 && !self.is_tw_channel_open() {
            return false;
        }
        let (castle, fort) = target.map_or((None, None), |t| self.castle_and_fort_of(t));
        if castle.is_none() && fort.is_none() {
            return value & COND_NOT_ZONE != 0;
        }
        let Some(player) = target.filter(|t| self.is_player(*t)) else {
            return false;
        };
        let (residence, zone, attack, defend, neutral) = match (castle, fort) {
            (Some(c), _) => (
                ResidenceRef::Castle(c),
                self.castle(c).and_then(|c| c.siege_zone),
                COND_CAST_ATTACK,
                COND_CAST_DEFEND,
                COND_CAST_NEUTRAL,
            ),
            (None, Some(f)) => (
                ResidenceRef::Fort(f),
                self.fort(f).and_then(|f| f.siege_zone),
                COND_FORT_ATTACK,
                COND_FORT_DEFEND,
                COND_FORT_NEUTRAL,
            ),
            (None, None) => return value & COND_NOT_ZONE != 0,
        };
        let residence_id = residence.residence_id();
        let state = self.siege_state_of(player);
        if residence_id <= 0 || !zone.is_some_and(|z| self.is_siege_zone_active(z)) {
            return value & COND_NOT_ZONE != 0;
        }
        (value & attack != 0 && self.is_registered_on_this_siege_field(player, residence_id) && state == 1)
            || (value & defend != 0
                && self.is_registered_on_this_siege_field(player, residence_id)
                && state == 2)
            || (value & neutral != 0 && state == 0)
    }

    /// Java `ConditionPlayerCanResurrect.testImpl`: `canResurrect` of the target (the caster hears
    /// why not). A skill with an affect range always passes ("Need skill rework").
    // NOTE(java-quirk): in a castle siege in progress every branch refuses, so only the Blessed
    // Scroll of Battlefield Resurrection (2393) brings a player back there.
    pub fn cond_can_resurrect(
        &self,
        effector: ObjectId,
        effected: Option<ObjectId>,
        skill: Option<(i32, i32)>,
    ) -> bool {
        let affect_range = skill
            .and_then(|(id, level)| self.skill_table()?.get(id, level).map(|s| s.affect_range))
            .unwrap_or(0);
        if affect_range > 0 {
            return true;
        }
        let Some(effected) = effected else {
            return false;
        };
        let tell = |message: SystemMessage| {
            if self.is_player(effector) {
                self.send(effector, &message);
            }
        };
        let blocked = |id| self.is_affected(id, crate::effects::effect_flag::BLOCK_RESURRECTION);
        let mut can = true;
        if self.is_player(effected) {
            if !self.is_dead(effected) {
                can = false;
                let mut message = SystemMessage::new(SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms);
                if let Some((id, level)) = skill {
                    message = message.with(SystemMessageParam::SkillName { id, level });
                }
                tell(message);
            } else if blocked(effected) {
                can = false;
                tell(SystemMessage::new(SystemMessageId::RejectResurrection));
            } else if self.is_revive_requested(effected) {
                can = false;
                tell(SystemMessage::new(
                    SystemMessageId::ResurrectionHasAlreadyBeenProposed,
                ));
            } else if skill.is_none_or(|(id, _)| id != 2393) {
                // Blessed Scroll of Battlefield Resurrection (2393) passes.
                if let Some(siege) = self.siege_of(effected).and_then(|c| self.castle_siege(c)) {
                    let clan = self.clan_of(effected).map(|c| c.id);
                    can = false;
                    let message = match clan {
                        None => SystemMessageId::ItIsNotPossibleToResurrectInBattlefieldsWhereASiegeWarIsTakingPlace,
                        Some(c) if siege.is_defender(Some(c)) && siege.control_tower_count == 0 => {
                            SystemMessageId::TheGuardianTowerHasBeenDestroyedAndResurrectionIsNotPossible
                        }
                        Some(c) if siege.attacker_clan(c).is_some_and(|sc| sc.flags.is_empty()) => {
                            SystemMessageId::IfABaseCampDoesNotExistResurrectionIsNotPossible
                        }
                        Some(_) => SystemMessageId::ItIsNotPossibleToResurrectInBattlefieldsWhereASiegeWarIsTakingPlace,
                    };
                    tell(SystemMessage::new(message));
                }
            }
        } else if self.is_summon(effected) {
            if !self.is_dead(effected) {
                can = false;
                let mut message = SystemMessage::new(SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms);
                if let Some((id, level)) = skill {
                    message = message.with(SystemMessageParam::SkillName { id, level });
                }
                tell(message);
            } else if blocked(effected) {
                can = false;
                tell(SystemMessage::new(SystemMessageId::RejectResurrection));
            } else if self
                .summon_owner(effected)
                .is_some_and(|owner| self.is_reviving_pet(owner))
            {
                can = false;
                // Resurrection is already been proposed.
                tell(SystemMessage::new(
                    SystemMessageId::ResurrectionHasAlreadyBeenProposed,
                ));
            }
        }
        can
    }

    /// Java `ConditionPlayerSiegeSide.testImpl`: the acting player's `getSiegeSide()`.
    pub(crate) fn cond_player_siege_side(&self, effector: ObjectId, side: i32) -> bool {
        self.acting_player(effector)
            .is_some_and(|p| self.siege_side_of(p) == side)
    }
}
