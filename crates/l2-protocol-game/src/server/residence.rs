//! Server packets of residences (castles, fortresses, clan halls): `AgitDecoInfo`.
//!
//! Values only, like [`super::fence`]: the world works out what Java reads from the clan hall
//! and its functions (M7-RES-01).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/AgitDecoInfo.java
/// The decorations of a clan hall (`AGIT_DECO_INFO`), sent when a player walks into its zone.
/// Each byte is 0 (no function), 1 (a low level) or 2 (a high level); which level counts as high
/// depends on the hall's grade, see `l2_world::residence`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct AgitDecoInfo {
    pub hall_id: i32,
    /// `FUNC_RESTORE_HP`.
    pub fireplace: u8,
    /// `FUNC_RESTORE_MP`, written twice (carpet and statue).
    pub carpet_statue: u8,
    /// `FUNC_RESTORE_EXP`.
    pub chandelier: u8,
    /// `FUNC_TELEPORT`.
    pub mirror: u8,
    /// Always 0 in Java.
    pub crystal: u8,
    /// `FUNC_DECO_CURTAINS`.
    pub curtain: u8,
    /// `FUNC_ITEM_CREATE`.
    pub magic_curtain: u8,
    /// `FUNC_SUPPORT`, written twice ("Support? - Flag").
    pub flag: u8,
    /// `FUNC_DECO_FRONTPLATEFORM`.
    pub front_platform: u8,
    /// `FUNC_ITEM_CREATE` again ("Item create?").
    pub item_create: u8,
}

impl AgitDecoInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::AgitDecoInfo;
}

impl Encode for AgitDecoInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.hall_id)
            .u8(self.fireplace)
            .u8(self.carpet_statue)
            .u8(self.carpet_statue)
            .u8(self.chandelier)
            .u8(self.mirror)
            .u8(self.crystal)
            .u8(self.curtain)
            .u8(self.magic_curtain)
            .u8(self.flag)
            .u8(self.flag)
            .u8(self.front_platform)
            .u8(self.item_create);
    }
}

impl Decode for AgitDecoInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let hall_id = r.i32()?;
        let fireplace = r.u8()?;
        let carpet_statue = r.u8()?;
        // The statue byte repeats the carpet byte.
        r.u8()?;
        let chandelier = r.u8()?;
        let mirror = r.u8()?;
        let crystal = r.u8()?;
        let curtain = r.u8()?;
        let magic_curtain = r.u8()?;
        let flag = r.u8()?;
        r.u8()?;
        let front_platform = r.u8()?;
        let item_create = r.u8()?;
        Ok(Self {
            hall_id,
            fireplace,
            carpet_statue,
            chandelier,
            mirror,
            crystal,
            curtain,
            magic_curtain,
            flag,
            front_platform,
            item_create,
        })
    }
}
