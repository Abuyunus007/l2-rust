//! `CastleAmbassador`: the special envoy a castle sends to a fortress it can take under its wing.
// Java: dist/game/data/scripts/ai/others/CastleAmbassador/CastleAmbassador.java
// Java: mechanics/siege/Fort.java (setFortState, getCastleByAmbassador, getCastleIdByAmbassador,
//       isBorderFortress; the fortress side is `fort/spawns.rs` and `residence/fort.rs`)
// Java: entity/actor/Npc.java (broadcastEvent, deleteMe -> onDecay -> stopQuestTimers)
//
// The envoys stand at a fortress that has an owner and no decision (`Fort.spawnSpecialEnvoys`).
// The owner chooses a contract with the castle of the envoy (`signed`) or independence
// (`rejected`); after an hour without a choice the fortress becomes independent and the envoys
// leave.
//
// D-50/D-51: the fortress row is read again before the owner's choice is acted on (Sieghardt runs
// the fortress sieges, which reset the state). The choice is a player's action and is written;
// the decision of the timer is a fortress job (`Writes::Job`).

use l2_util::ObjectId;
use tracing::warn;

use super::{CastleScript, NpcThen, ResidenceNpcTimer, ScriptFailed};
use crate::residence::{FortId, ResidenceReadThen, ResidenceRef, Writes};
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `CastleAmbassador.CASTLE_AMBASSADOR`: the envoys of Gludio, Dion, Giran, Oren, Aden, Innadril,
/// Goddard, Rune and Schuttgart.
pub const AMBASSADORS: [i32; 32] = [
    36393, 36394, 36437, 36435, // Gludio
    36395, 36436, 36439, 36441, // Dion
    36396, 36440, 36444, 36449, 36451, // Giran
    36397, 36438, 36442, 36443, 36446, // Oren
    36398, 36399, 36445, 36448, // Aden
    36400, 36450, // Innadril
    36401, 36447, 36453, // Goddard
    36433, 36452, 36454, // Rune
    36434, 36455, // Schuttgart
];

/// One hour without a choice (`startQuestTimer("DESPAWN", 3600000, npc, null)`).
const IDLE_DESPAWN_MS: u64 = 3_600_000;
/// The envoys leave 3 seconds after the choice.
const CHOICE_DESPAWN_MS: u64 = 3000;
/// `npc.broadcastEvent("DESPAWN", 1000, null)`.
const BROADCAST_RADIUS: i32 = 1000;

impl World {
    /// `npc.getFort()` of an envoy.
    fn envoy_fort(&self, npc: ObjectId) -> Result<FortId, ScriptFailed> {
        self.npc_fort(npc).ok_or(ScriptFailed)
    }

    /// The page of `onFirstTalk` and of the choices: `getHtm`, `%castleName%`.
    fn envoy_page(&mut self, player: ObjectId, npc: ObjectId, file: &str) -> Result<(), ScriptFailed> {
        let fort = self.envoy_fort(npc)?;
        // `getCastleByAmbassador(npcId).getName()` is a null pointer for an NPC that is no envoy.
        let castle = self
            .fort_castle_by_ambassador(fort, self.castle_npc_id(npc))
            .ok_or(ScriptFailed)?;
        let name = self.castle(castle).map(|c| c.name.clone()).unwrap_or_default();
        self.castle_script_send_packet(
            CastleScript::Ambassador,
            player,
            npc,
            file,
            &[("%castleName%", name)],
        );
        Ok(())
    }

    /// `onFirstTalk` of an envoy: the page goes out here, the script returns `null`.
    pub(super) fn ambassador_first_talk(&mut self, npc: ObjectId, player: ObjectId) {
        let Ok(fort) = self.envoy_fort(npc) else {
            warn!("CastleAmbassador: the envoy {npc} stands at no fortress.");
            return;
        };
        let owner = self.fort(fort).map_or(0, |f| f.owner_id);
        let file = if self.is_clan_leader(player) && self.clan_id_of(player) == owner {
            if self.fort_is_border_fortress(fort) {
                "ambassador-01.html"
            } else {
                "ambassador.html"
            }
        } else {
            "ambassador-03.html"
        };
        if self.envoy_page(player, npc, file).is_err() {
            warn!("CastleAmbassador: {npc} is no envoy of {fort}.");
        }
    }

    /// `onEvent` of an envoy; the pages go out here and the script returns `null`.
    pub(super) fn ambassador_on_event(&mut self, event: &str, npc: ObjectId, player: Option<ObjectId>) -> Option<String> {
        // The owner's choice reads the fortress again first (D-50).
        if matches!(event, "signed" | "rejected")
            && let Some(player) = player
            && let Ok(fort) = self.envoy_fort(npc)
        {
            self.request_residence_reread(
                ResidenceRef::Fort(fort),
                ResidenceReadThen::Npc(NpcThen::Ambassador {
                    player,
                    npc,
                    event: event.to_owned(),
                }),
            );
            return None;
        }
        self.ambassador_run_event(event, npc, player);
        None
    }

    /// The body of `onEvent` once the fortress is fresh.
    pub(crate) fn ambassador_run_event(&mut self, event: &str, npc: ObjectId, player: Option<ObjectId>) {
        let Ok(fort) = self.envoy_fort(npc) else {
            warn!("CastleAmbassador: the envoy {npc} stands at no fortress.");
            return;
        };
        let state = self.fort(fort).map_or(0, |f| f.state);
        let castle_id = self.fort_castle_id_by_ambassador(fort, self.castle_npc_id(npc));
        let mut page = None;
        match event {
            "signed" => {
                if state == 0 {
                    self.fort_set_state(fort, 2, castle_id, Writes::Player);
                    self.envoy_restart_despawn(npc);
                    page = Some("ambassador-05.html");
                } else if state == 1 {
                    page = Some("ambassador-04.html");
                }
            }
            "rejected" => {
                if state == 0 {
                    self.fort_set_state(fort, 1, castle_id, Writes::Player);
                    self.envoy_restart_despawn(npc);
                    page = Some("ambassador-02.html");
                } else if state == 2 {
                    page = Some("ambassador-02.html");
                }
            }
            "DESPAWN" => {
                if state == 0 {
                    // The hour is over: the fortress stays independent.
                    self.fort_set_state(fort, 1, castle_id, Writes::Job);
                }
                self.envoy_cancel_despawn(npc);
                // `npc.broadcastEvent("DESPAWN", 1000, null)`: the envoys around receive it and
                // delete themselves (`onEventReceived`).
                for other in self.visible_objects_in_range(npc, BROADCAST_RADIUS) {
                    if self.castle_script_of(other) == Some(CastleScript::Ambassador) {
                        self.delete_npc(other);
                    }
                }
                self.delete_npc(npc);
            }
            _ => {}
        }
        if let (Some(file), Some(player)) = (page, player)
            && self.envoy_page(player, npc, file).is_err()
        {
            warn!("CastleAmbassador: {npc} is no envoy of {fort}.");
        }
    }

    /// `cancelQuestTimer("DESPAWN", npc, null)`.
    fn envoy_cancel_despawn(&mut self, npc: ObjectId) {
        if let Some(timer) = self.residence_npc.envoy_timers.remove(&npc) {
            self.timers.cancel(timer);
        }
    }

    /// `startQuestTimer("DESPAWN", delay, npc, null)`: a timer of the NPC that is there already
    /// stays.
    fn envoy_start_despawn(&mut self, npc: ObjectId, delay_ms: u64) {
        if self.residence_npc.envoy_timers.contains_key(&npc) {
            return;
        }
        let due = self.tick + TimerWheel::ms_to_ticks(delay_ms);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::ResidenceNpc(ResidenceNpcTimer::EnvoyDespawn(npc)));
        self.residence_npc.envoy_timers.insert(npc, timer);
    }

    /// The choice: the envoys leave in 3 seconds instead of at the end of the hour.
    fn envoy_restart_despawn(&mut self, npc: ObjectId) {
        self.envoy_cancel_despawn(npc);
        self.envoy_start_despawn(npc, CHOICE_DESPAWN_MS);
    }

    /// The `DESPAWN` quest timer fired (`onEvent("DESPAWN", npc, null)`).
    pub(super) fn ambassador_despawn_timer(&mut self, npc: ObjectId) {
        self.residence_npc.envoy_timers.remove(&npc);
        if self.npcs.contains_key(&npc) {
            self.ambassador_run_event("DESPAWN", npc, None);
        }
    }

    /// `onSpawn` of an envoy: without a castle that owns the land it speaks for, the envoy is
    /// deleted, else it leaves after an hour.
    pub(super) fn ambassador_on_spawn(&mut self, npc: ObjectId) {
        let castle = self.npc_fort(npc).and_then(|fort| {
            self.fort_castle_by_ambassador(fort, self.castle_npc_id(npc))
                .and_then(|c| self.castle(c))
        });
        if castle.is_none_or(|c| c.owner_id == 0) {
            // NOTE: Java deletes the NPC inside `doSpawn`; here one tick later, because the spawn
            // table registers the NPC after it was spawned.
            let due = self.tick + 1;
            self.timers
                .schedule_at(due, TimerKind::ResidenceNpc(ResidenceNpcTimer::EnvoyRemove(npc)));
        } else {
            self.envoy_start_despawn(npc, IDLE_DESPAWN_MS);
        }
    }
}
