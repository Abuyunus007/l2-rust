//! The gatekeepers of the catacombs and necropolises (`DungeonGatekeeper`): who may go in, and
//! the Seven Signs dungeon flag of the player.
// Java: entity/actor/instance/DungeonGatekeeper.java (onBypassFeedback, doTeleport)

use l2_data::TeleportType;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::{CABAL_DAWN, CABAL_DUSK, CABAL_NULL, SEAL_AVARICE, SEAL_GNOSIS, SEVEN_SIGNS_HTML_PATH};
use crate::world::World;

impl World {
    fn is_dungeon_gatekeeper(&self, npc: ObjectId) -> bool {
        self.npcs
            .get(&npc)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .is_some_and(|t| t.npc_type == "DungeonGatekeeper")
    }

    /// Java `DungeonGatekeeper.onBypassFeedback`: `true` when the NPC is one (the command was
    /// handled here).
    pub(crate) fn ss_dungeon_gatekeeper_bypass(
        &mut self,
        npc: ObjectId,
        player: ObjectId,
        command: &str,
    ) -> bool {
        if !self.is_dungeon_gatekeeper(npc) {
            return false;
        }
        self.send(player, &ActionFailed);
        let mut st = command.split(' ').filter(|t| !t.is_empty());
        let Some(actual) = st.next() else {
            // `st.nextToken()` throws on an empty command.
            warn!("DungeonGatekeeper: empty bypass of {player}");
            return true;
        };
        let avarice_owner = self.ss_seal_owner(SEAL_AVARICE);
        let gnosis_owner = self.ss_seal_owner(SEAL_GNOSIS);
        let player_cabal = self.ss_player_cabal(player.0);
        let validation = self.ss_is_seal_validation_period();
        let comp_winner = self.ss_cabal_highest_score();
        let next_value = |st: &mut dyn Iterator<Item = &str>| st.next().and_then(|t| t.parse::<i32>().ok());

        let (is_necro, is_cata) = (actual.starts_with("necro"), actual.starts_with("cata"));
        if is_necro || is_cata {
            let (open, seal_owner, no_page) = if is_necro {
                (
                    self.seven_signs.settings.open_necropolis,
                    avarice_owner,
                    "necro_no.htm",
                )
            } else {
                (
                    self.seven_signs.settings.open_catacombs,
                    gnosis_owner,
                    "cata_no.htm",
                )
            };
            let mut can_port = true;
            if !open {
                if validation {
                    if comp_winner == CABAL_DAWN && (player_cabal != CABAL_DAWN || seal_owner != CABAL_DAWN) {
                        self.send(
                            player,
                            &SystemMessage::new(SystemMessageId::OnlyALordOfDawnMayUseThis),
                        );
                        can_port = false;
                    } else if comp_winner == CABAL_DUSK
                        && (player_cabal != CABAL_DUSK || seal_owner != CABAL_DUSK)
                    {
                        self.send(
                            player,
                            &SystemMessage::new(SystemMessageId::OnlyARevolutionaryOfDuskMayUseThis),
                        );
                        can_port = false;
                    } else if comp_winner == CABAL_NULL && player_cabal != CABAL_NULL {
                        can_port = true;
                    } else if player_cabal == CABAL_NULL {
                        can_port = false;
                    }
                } else if player_cabal == CABAL_NULL {
                    can_port = false;
                }
            }
            if !can_port {
                if let Some(html) =
                    self.html_from_file(player, npc.0, &format!("{SEVEN_SIGNS_HTML_PATH}{no_page}"))
                {
                    self.send_html(player, &html);
                }
            } else {
                let Some(value) = next_value(&mut st) else {
                    warn!("DungeonGatekeeper: bad bypass {command:?} of {player}");
                    return true;
                };
                self.ss_dungeon_teleport(npc, player, value);
                // NOTE(java-quirk): set even when the teleport did not happen (no fee, dead).
                self.set_in_7s_dungeon(player, true);
            }
        } else if actual.starts_with("exit") {
            let Some(value) = next_value(&mut st) else {
                warn!("DungeonGatekeeper: bad bypass {command:?} of {player}");
                return true;
            };
            self.ss_dungeon_teleport(npc, player, value);
            self.set_in_7s_dungeon(player, false);
        } else if actual.starts_with("goto") {
            let Some(value) = next_value(&mut st) else {
                warn!("DungeonGatekeeper: bad bypass {command:?} of {player}");
                return true;
            };
            self.ss_dungeon_teleport(npc, player, value);
        } else {
            self.generic_npc_bypass_feedback(npc, player, command);
        }
        true
    }

    /// Java `DungeonGatekeeper.doTeleport(player, value)`: the `OTHER` list of the NPC.
    fn ss_dungeon_teleport(&mut self, npc: ObjectId, player: ObjectId, value: i32) {
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let data = self.talk.teleporters.clone();
        match data
            .as_ref()
            .and_then(|d| d.get_holder(npc_id, TeleportType::Other.name()))
        {
            Some(holder) => self.do_teleport(npc, player, holder, value),
            None => warn!("No teleport destination with id:{value}"),
        }
        self.send(player, &ActionFailed);
    }
}
