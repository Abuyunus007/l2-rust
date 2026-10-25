//! Devastated Castle (clan hall 34): the clan that does the most damage to Gustav wins the hall.
// Java: dist/game/data/scripts/conquerablehalls/DevastatedCastle/DevastatedCastle.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, NpcSay};
use l2_util::ObjectId;

use super::{DEVASTATED_CASTLE, ScriptState};
use crate::clan::model::java_map_order;
use crate::effects::SkillKey;
use crate::residence::ClanHallId;
use crate::world::World;

pub(super) const GUSTAV: i32 = 35410;
pub(super) const MIKHAIL: i32 = 35409;
pub(super) const DIETRICH: i32 = 35408;
/// The skill Gustav casts when he is almost beaten.
fn gustav_last_skill() -> SkillKey {
    SkillKey::new(4235, 1)
}

/// What the script keeps.
#[derive(Debug, Default)]
pub(crate) struct Devastated {
    /// `_damageToGustav`: clan id to damage (a `ConcurrentHashMap`).
    pub(crate) damage: Vec<(i32, i32)>,
}

/// Java `getWinner()`: `Collections.max` by value, the first of the greatest in the map's order.
pub(super) fn winner(state: &Devastated) -> Option<i32> {
    let keys: Vec<i32> = state.damage.iter().map(|(clan, _)| *clan).collect();
    let mut best: Option<(i32, i32)> = None;
    for index in java_map_order(&keys, keys.len()) {
        let (clan, dam) = state.damage[index];
        if best.is_none_or(|(_, b)| dam > b) {
            best = Some((clan, dam));
        }
    }
    best.map(|(clan, _)| clan)
}

pub(super) fn on_siege_starts(world: &mut World, id: ClanHallId) {
    if let Some(ScriptState::Devastated(s)) = world.hall_siege_mut(id).map(|s| &mut s.state) {
        s.damage.clear();
    }
}

impl World {
    /// `npc.broadcastSay(type, string)`: `NpcSay` to the players who see the NPC.
    pub(crate) fn hall_npc_say(&mut self, npc: ObjectId, chat: ChatType, string: NpcStringId) {
        let Some(npc_id) = self.npcs.get(&npc).map(|n| n.npc_id) else {
            return;
        };
        self.broadcast_packet(
            npc,
            &NpcSay {
                object_id: npc.0,
                text_type: chat as i32,
                npc_id,
                npc_string: string as i32,
                text: String::new(),
                parameters: Vec::new(),
            },
        );
    }

    /// Java `onSpawn`: Gustav's guards shout.
    pub(super) fn devastated_on_spawn(&mut self, npc: ObjectId, npc_id: i32) {
        if npc_id == MIKHAIL {
            self.hall_npc_say(
                npc,
                ChatType::NpcShout,
                NpcStringId::GloryToAdenTheKingdomOfTheLionGloryToSirGustavOurImmortalLord,
            );
        } else if npc_id == DIETRICH {
            self.hall_npc_say(
                npc,
                ChatType::NpcShout,
                NpcStringId::SoldiersOfGustavGoForthAndDestroyTheInvaders,
            );
        }
    }

    /// Java `onAttack`.
    pub(super) fn devastated_on_attack(&mut self, npc: ObjectId, player: ObjectId, damage: i32) {
        let hall = ClanHallId(DEVASTATED_CASTLE);
        if !self.hall_is_in_siege(hall) {
            return;
        }
        let clan_id = self.clan_id_of(player);
        if clan_id != 0 && self.hall_siege_is_attacker(hall, clan_id) {
            if let Some(ScriptState::Devastated(s)) = self.hall_siege_mut(hall).map(|s| &mut s.state) {
                match s.damage.iter_mut().find(|(clan, _)| *clan == clan_id) {
                    Some((_, total)) => *total = total.wrapping_add(damage),
                    None => s.damage.push((clan_id, damage)),
                }
            }
        }
        let low = self
            .npcs
            .get(&npc)
            .is_some_and(|n| n.core.cur_hp < f64::from(n.core.max_hp as i32 / 12));
        if !self.is_casting_now(npc) && low {
            self.hall_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::ThisIsUnbelievableHaveIReallyBeenDefeatedIShallReturnAndTakeYourHead,
            );
            self.set_intention_cast(npc, gustav_last_skill(), Some(npc));
        }
    }

    /// Java `onKill`.
    pub(super) fn devastated_on_kill(&mut self) {
        let hall = ClanHallId(DEVASTATED_CASTLE);
        if !self.hall_is_in_siege(hall) {
            return;
        }
        if let Some(siege) = self.hall_siege_mut(hall) {
            siege.mission_accomplished = true;
        }
        self.hall_cancel_siege_task(hall);
        self.hall_end_siege(hall);
    }
}
