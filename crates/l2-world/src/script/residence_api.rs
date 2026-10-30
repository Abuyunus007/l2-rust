//! What the NPC scripts of the residences (S-AI-08: WyvernManager) read about "the castle, fortress
//! or conquerable hall of an NPC", and the Strider to Wyvern change.
// Java: entity/actor/Npc.java (getCastle, getFort, getConquerableHall),
//       mechanics/siege/{Castle,Fort,clanhalls/SiegableHall}.java, entity/actor/Player.java (mount)

use l2_util::ObjectId;

use crate::mount::MountType;
use crate::world::World;

/// The facts of one residence a script asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidenceFacts {
    /// `getResidenceId()`.
    pub id: i32,
    /// `getName()`.
    pub name: String,
    /// `getOwnerId()`; 0 without an owner.
    pub owner_id: i32,
    /// Castle and fortress: `getZone().isActive()`. Hall: `isInSiege()`.
    pub siege_active: bool,
}

impl World {
    /// Java `npc.getCastle()`.
    pub fn script_npc_castle_facts(&self, npc: ObjectId) -> Option<ResidenceFacts> {
        let id = self.npc_castle(npc)?;
        let castle = self.castle(id)?;
        Some(ResidenceFacts {
            id: castle.id.0,
            name: castle.name.clone(),
            owner_id: castle.owner_id,
            siege_active: castle.siege_zone.is_some_and(|z| self.is_siege_zone_active(z)),
        })
    }

    /// Java `npc.getFort()`.
    pub fn script_npc_fort_facts(&self, npc: ObjectId) -> Option<ResidenceFacts> {
        let id = self.npc_fort(npc)?;
        let fort = self.fort(id)?;
        Some(ResidenceFacts {
            id: fort.id.0,
            name: fort.name.clone(),
            owner_id: fort.owner_id,
            siege_active: self.fort_zone_is_active(id),
        })
    }

    /// Java `npc.getConquerableHall()`.
    pub fn script_npc_hall_facts(&self, npc: ObjectId) -> Option<ResidenceFacts> {
        let (x, y, _) = self.position_of(npc)?;
        let id = self.hall_siege_near(x, y)?;
        let hall = self.clan_hall(id)?;
        Some(ResidenceFacts {
            id: hall.id.0,
            name: hall.name.clone(),
            owner_id: hall.owner_id,
            siege_active: self.hall_is_in_siege(id),
        })
    }

    /// Java `player.isMounted() && player.getMountLevel() >= level && ArrayUtil.contains(npcIds,
    /// player.getMountNpcId())`.
    pub fn script_is_mounted_on(&self, player: ObjectId, min_level: i32, npc_ids: &[i32]) -> bool {
        self.is_mounted(player)
            && self.players.get(&player).is_some_and(|p| p.mount.level >= min_level)
            && npc_ids.contains(&self.mount_npc_id_of(player))
    }

    /// Java `player.getMountType() == MountType.STRIDER`.
    pub fn script_is_on_strider(&self, player: ObjectId) -> bool {
        self.mount_type_of(player) == MountType::Strider
    }

    /// Java `player.dismount(); player.mount(npcId, 0, true);`.
    pub fn script_dismount_and_mount(&mut self, player: ObjectId, npc_id: i32) {
        self.dismount(player);
        self.mount_npc(player, npc_id, ObjectId(0), true);
    }
}
