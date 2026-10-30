//! Small world reads and writes the area AI scripts of S-AI-03 need (Beast Farm, Pagan Temple,
//! Dragon Valley, Monastery of Silence, Primeval Isle, ...).
// Java: config/PlayerConfig.java (AUTO_LOOT), entity/actor/Npc.java (broadcastSocialAction),
//       entity/actor/Creature.java (getTemplate().getParameters())

use l2_protocol_game::server::{SocialAction, ValidateLocation};
use l2_util::ObjectId;

use super::ScriptNpc;
use super::ctx::ScriptCtx;

impl ScriptCtx<'_> {
    /// Java `PlayerConfig.AUTO_LOOT` (`AutoLoot` of `Player.ini`).
    pub fn auto_loot(&self) -> bool {
        self.world().settings.ground.auto_loot
    }

    /// Java `npc.broadcastSocialAction(id)`: the animation goes to every player who sees the NPC.
    pub fn broadcast_npc_social_action(&mut self, npc: ScriptNpc, action_id: i32) {
        self.world().broadcast_packet(
            npc.object_id,
            &SocialAction {
                object_id: npc.object_id.0,
                action_id,
            },
        );
    }

    /// Java `npc.setHeading(heading); npc.broadcastPacket(new ValidateLocation(npc))`.
    pub fn set_heading_and_validate(&mut self, npc: ScriptNpc, heading: i32) {
        self.world_mut().set_heading(npc.object_id, heading);
        if let Some((x, y, z)) = self.world().position_of(npc.object_id) {
            self.world().broadcast_packet(
                npc.object_id,
                &ValidateLocation {
                    object_id: npc.object_id.0,
                    x,
                    y,
                    z,
                    heading,
                },
            );
        }
    }

    /// Java `npc.asAttackable().returnHome()`.
    pub fn return_home(&mut self, npc: ScriptNpc) {
        self.world_mut().return_home(npc.object_id);
    }

    /// Java `player.sendMessage(text)`: a system message with a plain text.
    pub fn send_message(&mut self, player: ObjectId, text: &str) {
        self.world().send_message(player, text);
    }
}
