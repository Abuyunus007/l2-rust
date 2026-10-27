//! `CastleCourtMagician`: the court magician of a castle (talismans for the knight's epaulettes,
//! the squad skills, the teleport to the clan leader's Clan Gate).
// Java: dist/game/data/scripts/ai/others/CastleCourtMagician/CastleCourtMagician.java
// Java: dist/game/data/scripts/handlers/skill/effects/CallPc.java (checkSummonTargetStatus)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::{Reply, ScriptFailed};
use crate::clan::model::ClanAccess;
use crate::effects::SkillKey;
use crate::teleport::TeleportTarget;
use crate::world::World;
use crate::zones::ZoneId;

/// `CLAN_GATE`: Clan Gate, the skill the leader casts.
const CLAN_GATE: i32 = 3632;
/// `DISPLAY_CLAN_GATE`: Production - Clan Gate, what the magician casts at the member.
const DISPLAY_CLAN_GATE: (i32, i32) = (5109, 1);

/// `EPAULETTE`: Knight's Epaulette.
const EPAULETTE: i32 = 9912;
/// `RED_MEDITATION`.
const RED_MEDITATION: i32 = 9931;
/// `BLUE_DIV_PROTECTION`.
const BLUE_DIV_PROTECTION: i32 = 9932;
/// `BLUE_EXPLOSION`.
const BLUE_EXPLOSION: i32 = 10416;
/// `BLUE_M_EXPLOSION`.
const BLUE_M_EXPLOSION: i32 = 10417;
/// `RED_MIN_CLARITY`.
const RED_MIN_CLARITY: i32 = 9917;
/// `RED_MAX_CLARITY`.
const RED_MAX_CLARITY: i32 = 9918;
/// `RED_MENTAL_REG`.
const RED_MENTAL_REG: i32 = 9928;
/// `BLUE_PROTECTION`.
const BLUE_PROTECTION: i32 = 9929;
/// `BLUE_INVIS`.
const BLUE_INVIS: i32 = 9920;
/// `BLUE_DEFENSE`.
const BLUE_DEFENSE: i32 = 9916;
/// `BLACK_ESCAPE`.
const BLACK_ESCAPE: i32 = 9923;
/// `BLUE_HEALING`.
const BLUE_HEALING: i32 = 9924;
/// `RED_RECOVERY`.
const RED_RECOVERY: i32 = 9925;
/// `BLUE_DEFENSE2`.
const BLUE_DEFENSE2: i32 = 9926;
/// `BLUE_M_DEFENSE`.
const BLUE_M_DEFENSE: i32 = 9927;
/// `RED_LIFE_FORCE`.
const RED_LIFE_FORCE: i32 = 10518;
/// `BLUE_GREAT_HEAL`.
const BLUE_GREAT_HEAL: i32 = 10424;
/// `WHITE_FIRE`.
const WHITE_FIRE: i32 = 10421;
/// `COMMON_TALISMANS`.
const COMMON_TALISMANS: [i32; 43] = [
    9914, 9915, 9920, 9921, 9922, 9933, 9934, 9935, 9936, 9937, 9938, 9939, 9940, 9941, 9942, 9943, 9944,
    9945, 9946, 9947, 9948, 9949, 9950, 9952, 9953, 9954, 9955, 9956, 9957, 9958, 9959, 9960, 9961, 9962,
    9963, 9964, 9965, 10418, 10420, 10519, 10422, 10423, 10419,
];

/// The talisman for the two rolls of `giveTalisman`: `category` is `getRandom(100)` and `chance`
/// is `getRandom(100)` (`getRandom(46)` for the common talismans).
pub(super) fn pick_talisman(category: i32, chance: i32) -> i32 {
    if category <= 5 {
        match chance {
            ..=25 => RED_MEDITATION,
            26..=50 => BLUE_DIV_PROTECTION,
            51..=75 => BLUE_EXPLOSION,
            _ => BLUE_M_EXPLOSION,
        }
    } else if category <= 15 {
        match chance {
            ..=20 => RED_MIN_CLARITY,
            21..=40 => RED_MAX_CLARITY,
            41..=60 => RED_MENTAL_REG,
            61..=80 => BLUE_PROTECTION,
            _ => BLUE_INVIS,
        }
    } else if category <= 30 {
        match chance {
            ..=12 => BLUE_DEFENSE,
            13..=25 => BLACK_ESCAPE,
            26..=37 => BLUE_HEALING,
            38..=50 => RED_RECOVERY,
            51..=62 => BLUE_DEFENSE2,
            63..=75 => BLUE_M_DEFENSE,
            76..=87 => RED_LIFE_FORCE,
            _ => BLUE_GREAT_HEAL,
        }
    } else if chance <= 42 {
        COMMON_TALISMANS[usize::try_from(chance)
            .unwrap_or(0)
            .min(COMMON_TALISMANS.len() - 1)]
    } else {
        WHITE_FIRE
    }
}

impl World {
    /// `onEvent` of the court magician.
    pub(super) fn court_magician_on_event(&mut self, event: &str, npc: ObjectId, player: ObjectId) -> Reply {
        let in_owner_clan =
            self.clan_of(player).is_some() && self.clan_id_of(player) == self.castle_owner_of_npc(npc);
        if !in_owner_clan {
            return Ok(Some("courtmagician-01.html".to_owned()));
        }
        match event {
            "courtmagician.html" | "courtmagician-03.html" => Ok(Some(event.to_owned())),
            "giveTalisman" => Ok(self.court_magician_give_talisman(player)),
            "squadSkill" => {
                if self.is_clan_leader(player) || self.has_clan_access(player, ClanAccess::MemberFame) {
                    self.show_sub_unit_skill_list(player);
                    Ok(None)
                } else {
                    Ok(Some("courtmagician-05.html".to_owned()))
                }
            }
            "clanTeleport" => self.court_magician_clan_teleport(npc, player),
            _ => Ok(None),
        }
    }

    /// `case "giveTalisman"`.
    fn court_magician_give_talisman(&mut self, player: ObjectId) -> Option<String> {
        if self.inventory_item_count(player, EPAULETTE) < 10 {
            return Some("courtmagician-06.html".to_owned());
        }
        // NOTE(java-quirk): `getSize() == getInventoryLimit()`; an inventory over its limit
        // passes.
        let size = self.players.get(&player).map_or(0, |p| {
            p.data
                .items
                .iter()
                .filter(|i| i.loc == "INVENTORY" || i.loc == "PAPERDOLL")
                .count()
        });
        let limit = self.stats(player).map_or(0, |v| v.inventory_limit());
        if i32::try_from(size).is_ok_and(|size| size == limit) {
            self.send(player, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
            return None;
        }
        let category = self.rng().below(100);
        // The common talismans are rolled out of 46, the others out of 100.
        let chance = if category > 30 {
            self.rng().below(46)
        } else {
            self.rng().below(100)
        };
        let item_id = pick_talisman(category, chance);
        self.script_take_items(player, EPAULETTE, 10);
        self.script_give_items(player, item_id, 1);
        Some("courtmagician-04.html".to_owned())
    }

    /// `case "clanTeleport"`: the member goes to the leader, who must have the Clan Gate up.
    fn court_magician_clan_teleport(&mut self, npc: ObjectId, player: ObjectId) -> Reply {
        if self.clan_id_of(player) != self.castle_owner_of_npc(npc) {
            return Ok(None);
        }
        let leader = self
            .clan_of(player)
            .and_then(|c| c.leader)
            .filter(|l| self.players.contains_key(l));
        let Some(leader) = leader.filter(|l| self.is_affected_by_skill(*l, CLAN_GATE)) else {
            return Ok(Some("courtmagician-02.html".to_owned()));
        };
        // TODO(java): "Custom one, retail does not check it".
        if self.check_summon_target_status(player, leader) {
            self.set_npc_target(npc, Some(player));
            self.do_cast(npc, SkillKey::new(DISPLAY_CLAN_GATE.0, DISPLAY_CLAN_GATE.1));
            let (Some((x, y, z)), heading) = (self.position_of(leader), self.heading_of(leader)) else {
                return Err(ScriptFailed);
            };
            let instance = self.objects.get(&leader).map_or(0, |e| e.instance_id);
            let offset = self.settings.teleport.max_offset_on_teleport;
            self.tele_to_location(
                player,
                TeleportTarget::at(x, y, z)
                    .with_heading(heading)
                    .with_instance(instance)
                    .with_random_offset(offset),
            );
        }
        Ok(None)
    }

    /// `CallPc.checkSummonTargetStatus(target, activeChar)`: whether `target` may be summoned to
    /// `active`; the reason goes to `active`.
    fn check_summon_target_status(&mut self, target: ObjectId, active: ObjectId) -> bool {
        if target == active {
            return false;
        }
        let name = self
            .players
            .get(&target)
            .map(|p| p.data.name.clone())
            .unwrap_or_default();
        let refuse = |world: &Self, id: SystemMessageId, with_name: bool| {
            let mut message = SystemMessage::new(id);
            if with_name {
                message = message.with(SystemMessageParam::PlayerName(name.clone()));
            }
            world.send(active, &message);
            false
        };
        if self.is_alike_dead(target) {
            return refuse(
                self,
                SystemMessageId::C1IsDeadAtTheMomentAndCannotBeSummoned,
                true,
            );
        }
        if self.is_in_store_mode(target) {
            return refuse(
                self,
                SystemMessageId::C1IsCurrentlyTradingOrOperatingAPrivateStoreAndCannotBeSummoned,
                true,
            );
        }
        if self.is_rooted(target) || self.is_in_combat(target) {
            return refuse(
                self,
                SystemMessageId::C1IsEngagedInCombatAndCannotBeSummoned,
                true,
            );
        }
        // M8: `isInOlympiadMode()` and `isRegisteredInComp()`
        // (YOU_CANNOT_SUMMON_PLAYERS_WHO_ARE_CURRENTLY_PARTICIPATING_IN_THE_GRAND_OLYMPIAD),
        // `isFestivalParticipant()`; `isOnEvent()` and `inObserverMode()` are not modelled.
        let flying = self.stats(target).is_some_and(|v| v.is_flying_mounted());
        if flying || self.is_combat_flag_equipped(target) {
            return refuse(
                self,
                SystemMessageId::YourTargetIsInAnAreaWhichBlocksSummoning,
                false,
            );
        }
        if self.is_inside_zone(target, ZoneId::NoSummonFriend) || self.is_inside_zone(target, ZoneId::Jail) {
            return refuse(self, SystemMessageId::C1IsInAnAreaWhichBlocksSummoning, true);
        }
        // A summoner inside an instance: `AllowSummonInInstance` and the instance's
        // `isSummonAllowed()` (M8-INST-01).
        if self.is_in_instance(active) {
            let allowed = self
                .instance(self.instance_id_of(active))
                .is_some_and(|i| i.allow_summon);
            if !self.instances.settings.allow_summon_in_instance || !allowed {
                return refuse(
                    self,
                    SystemMessageId::YouMayNotSummonFromYourCurrentLocation,
                    false,
                );
            }
        }
        // M8: `activeChar.isIn7sDungeon()`.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_rare_category_picks_one_of_four() {
        assert_eq!(pick_talisman(0, 0), RED_MEDITATION);
        assert_eq!(pick_talisman(5, 25), RED_MEDITATION);
        assert_eq!(pick_talisman(5, 26), BLUE_DIV_PROTECTION);
        assert_eq!(pick_talisman(5, 75), BLUE_EXPLOSION);
        assert_eq!(pick_talisman(5, 99), BLUE_M_EXPLOSION);
    }

    #[test]
    fn the_category_edges_follow_the_script() {
        // `categoryChance <= 15` and `<= 30`.
        assert_eq!(pick_talisman(6, 0), RED_MIN_CLARITY);
        assert_eq!(pick_talisman(15, 99), BLUE_INVIS);
        assert_eq!(pick_talisman(16, 0), BLUE_DEFENSE);
        assert_eq!(pick_talisman(30, 99), BLUE_GREAT_HEAL);
        assert_eq!(pick_talisman(31, 0), COMMON_TALISMANS[0]);
        assert_eq!(pick_talisman(99, 42), COMMON_TALISMANS[42]);
        assert_eq!(pick_talisman(99, 43), WHITE_FIRE);
        assert_eq!(pick_talisman(99, 45), WHITE_FIRE);
        assert_eq!(COMMON_TALISMANS.len(), 43);
    }

    #[test]
    fn the_middle_category_thresholds() {
        assert_eq!(pick_talisman(20, 12), BLUE_DEFENSE);
        assert_eq!(pick_talisman(20, 13), BLACK_ESCAPE);
        assert_eq!(pick_talisman(20, 37), BLUE_HEALING);
        assert_eq!(pick_talisman(20, 50), RED_RECOVERY);
        assert_eq!(pick_talisman(20, 62), BLUE_DEFENSE2);
        assert_eq!(pick_talisman(20, 75), BLUE_M_DEFENSE);
        assert_eq!(pick_talisman(20, 87), RED_LIFE_FORCE);
        assert_eq!(pick_talisman(10, 20), RED_MIN_CLARITY);
        assert_eq!(pick_talisman(10, 40), RED_MAX_CLARITY);
        assert_eq!(pick_talisman(10, 60), RED_MENTAL_REG);
        assert_eq!(pick_talisman(10, 80), BLUE_PROTECTION);
    }
}
