//! Fortress of Resistance (clan hall 21): the clan that does the most damage to Bloody Lord
//! Nurka wins the hall.
// Java: dist/game/data/scripts/conquerablehalls/FortressOfResistance/FortressOfResistance.java

use l2_protocol_game::server::NpcHtmlMessage;
use l2_util::ObjectId;

use super::schedule::format_date_time;
use super::{FORTRESS_RESISTANCE, ScriptState};
use crate::clan::model::java_hash_map_order;
use crate::residence::ClanHallId;
use crate::world::World;

/// `MESSENGER`.
pub(super) const MESSENGER: i32 = 35382;
/// `BLOODY_LORD_NURKA`.
pub(super) const BLOODY_LORD_NURKA: i32 = 35375;
/// `NURKA_COORDS[0]` (30%): the others are commented out in Java.
const NURKA_COORDS: (i32, i32, i32) = (45109, 112124, -1900);
/// `_nurka.setRespawnDelay(10800)`.
const NURKA_RESPAWN_SECONDS: i32 = 10800;
const MESSENGER_PAGE: &str =
    "data/scripts/conquerablehalls/FortressOfResistance/partisan_ordery_brakel001.htm";

/// What the script keeps.
#[derive(Debug)]
pub(crate) struct Resistance {
    /// `_nurka`: the spawn (an index into the runtime spawns).
    pub(crate) nurka_spawn: usize,
    /// `_damageToNurka`: clan id to damage, in the order the clans first hit (a `HashMap`).
    // NOTE(java-quirk): never cleared, the damage of an earlier siege counts in the next one.
    pub(crate) damage: Vec<(i32, i64)>,
    /// `_messengerMsg`: the page with the date of the siege when it was built.
    pub(crate) messenger_page: Option<String>,
}

/// Java `getWinner()`: the clan with the most damage (the first in the map's order on a tie).
pub(super) fn winner(state: &Resistance) -> Option<i32> {
    let keys: Vec<i32> = state.damage.iter().map(|(clan, _)| *clan).collect();
    let mut winner_id = 0;
    let mut counter = 0;
    for index in java_hash_map_order(&keys) {
        let (clan, dam) = state.damage[index];
        if dam > counter {
            winner_id = clan;
            counter = dam;
        }
    }
    (winner_id != 0).then_some(winner_id)
}

pub(super) fn on_siege_starts(world: &mut World, id: ClanHallId) {
    // `_nurka.init()`.
    if let Some(ScriptState::Resistance(s)) = world.hall_siege(id).map(|s| &s.state) {
        let spawn = s.nurka_spawn;
        world.fort_do_spawn(spawn);
    }
}

pub(super) fn on_siege_ends(world: &mut World, id: ClanHallId) {
    world.resistance_build_messenger(id);
}

impl World {
    /// The spawn of Nurka: `new Spawn(BLOODY_LORD_NURKA)`, one unit, `setLocation(NURKA_COORDS[0])`.
    pub(super) fn resistance_new_nurka_spawn(&mut self) -> usize {
        self.fort_new_spawn(BLOODY_LORD_NURKA, NURKA_COORDS, 0, NURKA_RESPAWN_SECONDS)
    }

    /// Java `buildMessengerMessage()`: the page with `%nextSiege%`.
    pub(super) fn resistance_build_messenger(&mut self, id: ClanHallId) {
        let next = self.hall_next_siege_time(id);
        let date = format_date_time(next, self.siege_offset_at(next));
        let page = self
            .get_htm(ObjectId(0), MESSENGER_PAGE)
            .map(|html| html.replace("%nextSiege%", &date));
        if let Some(ScriptState::Resistance(s)) = self.hall_siege_mut(id).map(|s| &mut s.state) {
            s.messenger_page = page;
        }
    }

    /// Java `onFirstTalk`: the cached page, without an object id.
    pub(super) fn resistance_first_talk(&mut self, player: ObjectId) {
        let page = match self.hall_siege(ClanHallId(FORTRESS_RESISTANCE)).map(|s| &s.state) {
            Some(ScriptState::Resistance(s)) => s.messenger_page.clone(),
            _ => None,
        };
        if let Some(message) = page.and_then(|html| NpcHtmlMessage::new(0, &html)) {
            self.send_html(player, &message);
        }
    }

    /// Java `onAttack(npc, player, damage, isSummon)`.
    pub(super) fn resistance_on_attack(&mut self, player: ObjectId, damage: i32) {
        let hall = ClanHallId(FORTRESS_RESISTANCE);
        if !self.hall_is_in_siege(hall) {
            return;
        }
        let clan_id = self.clan_id_of(player);
        if clan_id > 0
            && let Some(ScriptState::Resistance(s)) = self.hall_siege_mut(hall).map(|s| &mut s.state)
        {
            match s.damage.iter_mut().find(|(clan, _)| *clan == clan_id) {
                Some((_, total)) => *total += i64::from(damage),
                None => s.damage.push((clan_id, i64::from(damage))),
            }
        }
    }

    /// Java `onKill`: the siege ends with Nurka.
    pub(super) fn resistance_on_kill(&mut self, npc: ObjectId, spawn: Option<usize>) {
        let hall = ClanHallId(FORTRESS_RESISTANCE);
        if !self.hall_is_in_siege(hall) {
            return;
        }
        if let Some(siege) = self.hall_siege_mut(hall) {
            siege.mission_accomplished = true;
        }
        if let Some(spawn) = spawn {
            self.stop_respawn(spawn);
        }
        self.delete_npc(npc);
        self.hall_cancel_siege_task(hall);
        self.hall_end_siege(hall);
    }
}
