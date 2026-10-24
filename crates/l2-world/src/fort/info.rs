//! The fortress windows: the list of all fortresses, the siege window and the barracks map.
// Java: network/clientpackets/{RequestAllFortressInfo,RequestFortressSiegeInfo,
//       RequestFortressMapInfo}.java
// Java: network/serverpackets/{ExShowFortressInfo,ExShowFortressSiegeInfo,ExShowFortressMapInfo}.java

use l2_protocol_game::server::{
    ActionFailed, ExShowFortressInfo, ExShowFortressMapInfo, ExShowFortressSiegeInfo, FortressInfo,
};
use l2_util::ObjectId;
use tracing::warn;

use super::FortInput;
use crate::residence::FortId;
use crate::world::{World, unix_ms_now};

impl World {
    /// The three fortress packets.
    pub(crate) fn on_fort_input(&mut self, player: ObjectId, input: FortInput) {
        if !self.players.contains_key(&player) {
            return;
        }
        match input {
            FortInput::AllFortressInfo => {
                let packet = self.exshow_fortress_info();
                self.send(player, &packet);
            }
            FortInput::FortressSiegeInfo => {
                let forts: Vec<FortId> = self.residences.forts.iter().map(|f| f.id).collect();
                for fort in forts {
                    if self.fort_siege_in_progress(fort)
                        && let Some(packet) = self.exshow_fortress_siege_info(fort)
                    {
                        self.send(player, &packet);
                    }
                }
            }
            FortInput::FortressMapInfo { fortress_id } => {
                match self.exshow_fortress_map_info(FortId(fortress_id)) {
                    Some(packet) => {
                        self.send(player, &packet);
                    }
                    None => {
                        warn!(
                            "Fort is not found with id ({fortress_id}) in all forts with size of ({}) called by player ({player})",
                            self.residences.forts.len()
                        );
                        self.send(player, &ActionFailed);
                    }
                }
            }
        }
    }

    /// Java `ExShowFortressInfo.STATIC_PACKET` as it is written now.
    pub fn exshow_fortress_info(&self) -> ExShowFortressInfo {
        let now = unix_ms_now();
        let forts = self
            .residences
            .forts
            .iter()
            .map(|fort| FortressInfo {
                residence_id: fort.id.0,
                owner_clan_name: self
                    .clan(fort.owner_id)
                    .map(|c| c.name.clone())
                    .unwrap_or_default(),
                siege_in_progress: self.fort_siege_in_progress(fort.id),
                // Time of possession.
                owned_time: fort.owned_time(now),
            })
            .collect();
        ExShowFortressInfo { forts }
    }

    /// Java `new ExShowFortressSiegeInfo(fort)`.
    pub fn exshow_fortress_siege_info(&self, fort: FortId) -> Option<ExShowFortressSiegeInfo> {
        let size = self.fort(fort)?.fort_size();
        let spawns = self.fort.commander_lists.get(&fort).map_or(0, Vec::len);
        let alive = self.fort_commanders_alive(fort);
        Some(ExShowFortressSiegeInfo::new(fort.0, size, spawns, alive))
    }

    /// Java `new ExShowFortressMapInfo(fort)`: a commander is "spawned" while a commander of the
    /// siege has its NPC id.
    pub fn exshow_fortress_map_info(&self, fort: FortId) -> Option<ExShowFortressMapInfo> {
        let size = self.fort(fort)?.fort_size();
        let commanders = self.fort.commander_lists.get(&fort).cloned().unwrap_or_default();
        let alive: Vec<i32> = self
            .fort
            .forts
            .get(&fort)
            .map(|s| {
                s.siege
                    .commanders
                    .iter()
                    .filter_map(|spawn| self.spawn_def(*spawn).map(|d| d.npc_id))
                    .collect()
            })
            .unwrap_or_default();
        let spawned: Vec<bool> = commanders.iter().map(|h| alive.contains(&h.npc_id)).collect();
        Some(ExShowFortressMapInfo::new(
            fort.0,
            self.fort_siege_in_progress(fort),
            size,
            &spawned,
        ))
    }
}
