//! Fortress of the Dead (clan hall 64): the clan that does the most damage to Lidia wins the hall;
//! the siege starts only at night.
// Java: dist/game/data/scripts/conquerablehalls/FortressOfTheDead/FortressOfTheDead.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;

use super::engine::ScriptResult;
use super::{FORTRESS_OF_DEAD, HallSiegeTimer, ScriptState};
use crate::clan::model::java_hash_map_order;
use crate::residence::ClanHallId;
use crate::world::{World, game_time_minutes, unix_ms_now};

pub(super) const LIDIA: i32 = 35629;
pub(super) const ALFRED: i32 = 35630;
pub(super) const GISELLE: i32 = 35631;

/// What the script keeps.
#[derive(Debug, Default)]
pub(crate) struct Dead {
    /// `_damageToLidia`: clan id to damage (a static `HashMap`).
    // NOTE(java-quirk): static and never cleared, the damage of an earlier siege counts in the next.
    pub(crate) damage: Vec<(i32, i32)>,
}

/// Java `getWinner()`: the clan with the most damage (the first in the map's order on a tie).
pub(super) fn winner(state: &Dead) -> Option<i32> {
    let keys: Vec<i32> = state.damage.iter().map(|(clan, _)| *clan).collect();
    let mut counter = 0;
    let mut damagest = 0;
    for index in java_hash_map_order(&keys) {
        let (clan, damage) = state.damage[index];
        if damage > counter {
            counter = damage;
            damagest = clan;
        }
    }
    (damagest != 0).then_some(damagest)
}

impl World {
    /// Java `startSiege()`: "Siege must start at night".
    pub(super) fn dead_start_siege(&mut self, id: ClanHallId) -> ScriptResult {
        let hours_left = (game_time_minutes(unix_ms_now()) / 60) % 24;
        if !(0..=6).contains(&hours_left) {
            self.hall_cancel_siege_task(id);
            // A game hour is ten minutes.
            let schedule_time = i64::from(24 - hours_left) * 10 * 60_000;
            self.hall_set_task(id, HallSiegeTimer::SiegeStarts(id), schedule_time);
            Ok(())
        } else {
            self.base_start_siege(id)
        }
    }

    /// Java `onSpawn`.
    pub(super) fn dead_on_spawn(&mut self, npc: ObjectId, npc_id: i32) {
        let string = match npc_id {
            LIDIA => NpcStringId::HmmThoseWhoAreNotOfTheBloodlineAreComingThisWayToTakeOverTheCastleHumphTheBitterGrudgesOfTheDeadYouMustNotMakeLightOfTheirPower,
            ALFRED => NpcStringId::HehHehISeeThatTheFeastHasBegunBeWaryTheCurseOfTheHellmannFamilyHasPoisonedThisLand,
            GISELLE => NpcStringId::AriseMyFaithfulServantsYouMyPeopleWhoHaveInheritedTheBloodItIsTheCallingOfMyDaughterTheFeastOfBloodWillNowBegin,
            _ => return,
        };
        self.hall_npc_say(npc, ChatType::NpcShout, string);
    }

    /// Java `onAttack`.
    pub(super) fn dead_on_attack(&mut self, player: ObjectId, damage: i32) {
        let hall = ClanHallId(FORTRESS_OF_DEAD);
        if !self.hall_is_in_siege(hall) {
            return;
        }
        let clan_id = self.clan_id_of(player);
        if clan_id != 0
            && self.hall_siege_is_attacker(hall, clan_id)
            && let Some(ScriptState::Dead(s)) = self.hall_siege_mut(hall).map(|s| &mut s.state)
        {
            match s.damage.iter_mut().find(|(clan, _)| *clan == clan_id) {
                Some((_, total)) => *total = total.wrapping_add(damage),
                None => s.damage.push((clan_id, damage)),
            }
        }
    }

    /// Java `onKill`.
    pub(super) fn dead_on_kill(&mut self, npc: ObjectId, npc_id: i32) {
        let hall = ClanHallId(FORTRESS_OF_DEAD);
        if !self.hall_is_in_siege(hall) {
            return;
        }
        if npc_id == ALFRED || npc_id == GISELLE {
            self.hall_npc_say(
                npc,
                ChatType::NpcShout,
                NpcStringId::AarghIfIDieThenTheMagicForceFieldOfBloodWill,
            );
        }
        if npc_id == LIDIA {
            self.hall_npc_say(
                npc,
                ChatType::NpcShout,
                NpcStringId::GrarrForTheNext2MinutesOrSoTheGameArenaAreWillBeCleanedThrowAnyItemsYouDonTNeedToTheFloorNow,
            );
            if let Some(siege) = self.hall_siege_mut(hall) {
                siege.mission_accomplished = true;
            }
            self.hall_cancel_siege_task(hall);
            self.hall_end_siege(hall);
        }
    }
}
