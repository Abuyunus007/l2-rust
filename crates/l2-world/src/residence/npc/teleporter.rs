//! `CastleTeleporter`: the mass gatekeeper that sends the defenders into the inner castle.
// Java: dist/game/data/scripts/ai/others/CastleTeleporter/CastleTeleporter.java
//
// The first click shows "ready" (`teleporter-01.html`), or, when all control towers of a running
// siege fell, "the defenders may be teleported in 8 minutes" (`teleporter-02.html`); the button
// starts the countdown (30 seconds, 8 minutes after the fall of the towers) and the NPC shouts
// when it is over.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, NpcSay};
use l2_util::ObjectId;

use super::{ResidenceNpcTimer, Reply, ScriptFailed};
use crate::residence::CastleId;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// Time in ms of the countdown after the control towers fell in a running siege.
const AFTER_TOWERS_MS: u64 = 480_000;
/// Time in ms of the normal countdown.
const NORMAL_MS: u64 = 30_000;

impl World {
    /// `siege.isInProgress() && siege.getControlTowerCount() == 0`.
    fn teleporter_towers_fell(&self, castle: CastleId) -> bool {
        self.is_castle_siege_in_progress(castle)
            && self.castle_siege(castle).is_some_and(|s| s.control_tower_count == 0)
    }

    /// `onFirstTalk` of the mass gatekeeper.
    pub(super) fn castle_teleporter_first_talk(&self, npc: ObjectId) -> String {
        let castle = self.npc_castle(npc);
        if self.residence_npc.teleporting.contains(&npc) {
            return "teleporter-03.html".to_owned();
        }
        if castle.is_some_and(|c| self.teleporter_towers_fell(c)) {
            "teleporter-02.html"
        } else {
            "teleporter-01.html"
        }
        .to_owned()
    }

    /// `onEvent` of the mass gatekeeper (`TELEPORT_START`; `TELEPORT_EXECUTE` is its timer).
    pub(super) fn castle_teleporter_on_event(&mut self, event: &str, npc: ObjectId) -> Reply {
        if !event.eq_ignore_ascii_case("TELEPORT_START") {
            return Ok(None);
        }
        let castle = self.npc_castle(npc).ok_or(ScriptFailed)?;
        if !self.residence_npc.teleporting.contains(&npc) {
            let time = if self.teleporter_towers_fell(castle) {
                AFTER_TOWERS_MS
            } else {
                NORMAL_MS
            };
            let due = self.tick + TimerWheel::ms_to_ticks(time);
            self.timers
                .schedule_at(due, TimerKind::ResidenceNpc(ResidenceNpcTimer::TeleporterExecute(npc)));
            self.residence_npc.teleporting.insert(npc);
        }
        Ok(Some("teleporter-03.html".to_owned()))
    }

    /// The `TELEPORT_EXECUTE` timer: the defenders of the castle go inside, the NPC shouts to
    /// everybody in its map region.
    pub(super) fn castle_teleporter_execute(&mut self, npc: ObjectId) {
        if !self.npcs.contains_key(&npc) {
            return;
        }
        let Some(castle) = self.npc_castle(npc) else {
            self.residence_npc.teleporting.remove(&npc);
            return;
        };
        let Some((x, y, _)) = self.position_of(npc) else {
            return;
        };
        let region = self
            .zoning
            .map_regions
            .as_ref()
            .map_or(0, |regions| regions.map_region_loc_id(x, y));
        let castle_name = self.castle(castle).map(|c| c.name.clone()).unwrap_or_default();
        let display_id = self.npc_template(npc).map_or(0, |t| t.display_id);
        let message = NpcSay {
            object_id: npc.0,
            text_type: ChatType::NpcShout as i32,
            npc_id: display_id,
            npc_string: NpcStringId::TheDefendersOfS1CastleWillBeTeleportedToTheInnerCastle as i32,
            text: String::new(),
            parameters: vec![castle_name],
        };
        self.castle_oust_all_players(castle);
        self.residence_npc.teleporting.remove(&npc);
        // TODO(java): "Is it possible to get all the players for that region, instead of all
        // players?"
        let mut players: Vec<ObjectId> = self.players.keys().copied().collect();
        players.sort_unstable();
        for player in players {
            let there = self.players.get(&player).map_or(0, |p| {
                self.zoning
                    .map_regions
                    .as_ref()
                    .map_or(0, |regions| regions.map_region_loc_id(p.core.x, p.core.y))
            });
            if there == region {
                self.send(player, &message);
            }
        }
    }
}
