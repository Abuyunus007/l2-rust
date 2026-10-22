//! Server packets of fishing: the cast, the fight, the fish's HP and the end.
//!
//! Values only, like the other server packets: the world works out the numbers
//! (`l2_world::fishing`). Every one of them starts with the object id of the fisher.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ExFishingStart.java
/// The line is cast (`EX_FISHING_START`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExFishingStart {
    /// The fisher.
    pub object_id: i32,
    /// `Fish.getFishGroup()` of the fish the player will fight (`-1` when it will not bite).
    pub fish_type: i32,
    /// Where the bait lands.
    pub x: i32,
    pub y: i32,
    pub z: i32,
    /// `Item.isNightLure()`.
    pub night_lure: bool,
}

impl ExFishingStart {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExFishingStart;
}

impl Encode for ExFishingStart {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.fish_type)
            .i32(self.x)
            .i32(self.y)
            .i32(self.z);
        w.bool(self.night_lure);
        // "show fish rank result button": Java always writes 0.
        w.u8(0);
    }
}

impl Decode for ExFishingStart {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let fish_type = r.i32()?;
        let x = r.i32()?;
        let y = r.i32()?;
        let z = r.i32()?;
        let night_lure = r.u8()? != 0;
        let _rank_button = r.u8()?;
        Ok(Self {
            object_id,
            fish_type,
            x,
            y,
            z,
            night_lure,
        })
    }
}

// Java: gameserver/network/serverpackets/ExFishingStartCombat.java
/// A fish took the bait (`EX_FISHING_START_COMBAT`). Java's constructor takes
/// `(creature, time, hp, mode, lureType, deceptiveMode)`; the fields are in the order it writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExFishingStartCombat {
    pub object_id: i32,
    /// The seconds the fight lasts.
    pub time: i32,
    /// The fish's HP.
    pub hp: i32,
    /// `0` resting, `1` fighting.
    pub mode: u8,
    /// `0` newbie lure, `1` normal lure, `2` night lure.
    pub lure_type: u8,
    /// `0` no, `1` yes.
    pub deceptive_mode: u8,
}

impl ExFishingStartCombat {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExFishingStartCombat;
}

impl Encode for ExFishingStartCombat {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).i32(self.time).i32(self.hp);
        w.u8(self.mode).u8(self.lure_type).u8(self.deceptive_mode);
    }
}

impl Decode for ExFishingStartCombat {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            time: r.i32()?,
            hp: r.i32()?,
            mode: r.u8()?,
            lure_type: r.u8()?,
            deceptive_mode: r.u8()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExFishingHpRegen.java
/// The fish's HP, once a second and after every pump or reel (`EX_FISHING_HP_REGEN`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExFishingHpRegen {
    pub object_id: i32,
    /// The seconds left.
    pub time: i32,
    pub fish_hp: i32,
    /// `0` HP stops, `1` HP rises.
    pub hp_mode: u8,
    /// `0` none, `1` success, `2` failed.
    pub good_use: u8,
    /// `0` none, `1` pumping, `2` reeling (Java's comment names them the other way round, its
    /// `Fishing` class sets 1 for pumping and 2 for reeling).
    pub anim: u8,
    /// The mastery penalty.
    pub penalty: i32,
    /// `0` normal HP bar, `1` purple HP bar (the deceptive mode).
    pub hp_bar_color: u8,
}

impl ExFishingHpRegen {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExFishingHpRegen;
}

impl Encode for ExFishingHpRegen {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id).i32(self.time).i32(self.fish_hp);
        w.u8(self.hp_mode).u8(self.good_use).u8(self.anim);
        w.i32(self.penalty);
        w.u8(self.hp_bar_color);
    }
}

impl Decode for ExFishingHpRegen {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            time: r.i32()?,
            fish_hp: r.i32()?,
            hp_mode: r.u8()?,
            good_use: r.u8()?,
            anim: r.u8()?,
            penalty: r.i32()?,
            hp_bar_color: r.u8()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExFishingEnd.java
/// The fishing is over (`EX_FISHING_END`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExFishingEnd {
    pub object_id: i32,
    /// The fish was caught.
    pub win: bool,
}

impl ExFishingEnd {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExFishingEnd;
}

impl Encode for ExFishingEnd {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id);
        w.bool(self.win);
    }
}

impl Decode for ExFishingEnd {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            win: r.u8()? != 0,
        })
    }
}
