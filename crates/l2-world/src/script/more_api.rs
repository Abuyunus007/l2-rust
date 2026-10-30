//! Small `pub` doors the quest batches asked for (S-API-05): the raw radar packet, an `NpcSay` to
//! one player, the clan info update, `isMyLord`.
// Java: network/serverpackets/RadarControl.java, network/serverpackets/NpcSay.java,
//       entity/clan/Clan.java (broadcastToOnlineMembers(new PledgeShowInfoUpdate(clan))),
//       entity/actor/Npc.java (isMyLord)

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, NpcSay, RadarControl};
use l2_util::ObjectId;

use super::api::ScriptNpc;
use super::ctx::ScriptCtx;
use crate::world::World;

impl World {
    /// Java `npc.isMyLord(player)`: the player leads the clan that owns the NPC's castle or
    /// fortress (see `fort_is_my_lord` for the `||` quirk).
    pub fn is_my_lord(&self, npc: ObjectId, player: ObjectId) -> bool {
        self.fort_is_my_lord(npc, player)
    }

    /// Java `clan.broadcastToOnlineMembers(new PledgeShowInfoUpdate(clan))`; nothing for an
    /// unknown clan.
    pub fn broadcast_pledge_info_update(&self, clan_id: i32) {
        self.broadcast_clan_info_update(clan_id);
    }
}

impl ScriptCtx<'_> {
    /// Java `player.sendPacket(new RadarControl(showRadar, type, x, y, z))`, without the radar
    /// list of `addRadar` (Q00307 `RadarControl(0, 2, ..)`).
    pub fn send_radar_control(
        &self,
        player: ObjectId,
        show_radar: i32,
        radar_type: i32,
        x: i32,
        y: i32,
        z: i32,
    ) {
        self.world().send(
            player,
            &RadarControl {
                show_radar,
                radar_type,
                x,
                y,
                z,
            },
        );
    }

    /// Java `player.sendPacket(new NpcSay(npc, chatType, npcStringId))`: the NPC's line to one
    /// player only (`broadcast_npc_say` sends it to everybody around).
    pub fn npc_say_to(
        &self,
        player: ObjectId,
        npc: ScriptNpc,
        chat: ChatType,
        npc_string: NpcStringId,
        params: &[&str],
    ) {
        let npc_id = self
            .world()
            .npc_world
            .template(npc.id)
            .map_or(npc.id, |t| t.display_id);
        self.world().send(
            player,
            &NpcSay {
                object_id: npc.object_id.0,
                text_type: chat.client_id(),
                npc_id,
                npc_string: npc_string.id(),
                text: String::new(),
                parameters: params.iter().map(|p| (*p).to_owned()).collect(),
            },
        );
    }

    /// Java `player.getClan().broadcastToOnlineMembers(new PledgeShowInfoUpdate(clan))` for the
    /// clan of `player` (nothing without a clan).
    pub fn broadcast_clan_info_update(&self, player: ObjectId) {
        let clan_id = self.world().clan_id_of(player);
        if clan_id != 0 {
            self.world().broadcast_pledge_info_update(clan_id);
        }
    }

    /// Java `npc.setWalking()`: the NPC walks (its run flag off, broadcast when it changes).
    pub fn set_walking(&mut self, npc: ScriptNpc) {
        self.world_mut().set_running(npc.object_id, false);
    }

    /// Java `SpawnTable.getInstance().getSpawns(npcId)` with each spawn's `getLastSpawn()`.
    pub fn spawns_of(&self, npc_id: i32) -> Vec<super::area_spawns::NamedSpawn> {
        self.world().spawns_of(npc_id)
    }

    /// Java `npc.isMyLord(player)`.
    pub fn is_my_lord(&self, npc: ScriptNpc, player: ObjectId) -> bool {
        self.world().is_my_lord(npc.object_id, player)
    }

    /// Java `addSpawn(summoner, npcId, pos, randomOffset, despawnDelay)` with `pos` the place
    /// and heading of `at`: the new NPC joins `summoner`'s list (`getSummonedNpcCount`). Java
    /// passes instance 0 here, whatever the instance of `at`.
    pub fn add_spawn_summoned_at(
        &mut self,
        summoner: ScriptNpc,
        npc_id: i32,
        at: ObjectId,
        random_offset: bool,
        despawn_delay_ms: i64,
    ) -> Option<ScriptNpc> {
        let (x, y, z) = self.world().position_of(at)?;
        let heading = self.world().heading_of(at);
        self.add_spawn_summoned(
            summoner,
            npc_id,
            x,
            y,
            z,
            heading,
            random_offset,
            despawn_delay_ms,
            0,
        )
    }
}
