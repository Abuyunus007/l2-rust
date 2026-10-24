//! Server packets of the fortresses: the fortress list, the siege window and the barracks map.
//!
//! Same rules as [`super::party`]: values only. The barracks lists are the packet's own format
//! and are derived in the constructors from the commander counts (or statuses) the world passes,
//! so the decoder can read them back as they are.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// The barracks list of a fortress with `fort_size` barracks and no barracks status to show.
fn zero_barracks(fort_size: i32) -> Vec<i32> {
    vec![0; usize::try_from(fort_size).unwrap_or(0)]
}

/// Reads the trailing barracks ints of a fortress packet: everything left in the packet.
fn read_barracks(r: &mut PacketReader) -> Result<Vec<i32>, PacketError> {
    let mut barracks = Vec::new();
    while r.remaining() >= 4 {
        barracks.push(r.i32()?);
    }
    Ok(barracks)
}

/// One fortress line of [`ExShowFortressInfo`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FortressInfo {
    pub residence_id: i32,
    /// The owner clan's name, empty when there is no owner clan.
    pub owner_clan_name: String,
    /// Java `fort.getSiege().isInProgress()`.
    pub siege_in_progress: bool,
    /// Java `fort.getOwnedTime()`: how long the current owner has held it.
    pub owned_time: i32,
}

// Java: gameserver/network/serverpackets/ExShowFortressInfo.java
/// The fortress list (`EX_SHOW_FORTRESS_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowFortressInfo {
    pub forts: Vec<FortressInfo>,
}

impl ExShowFortressInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowFortressInfo;
}

impl Encode for ExShowFortressInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.forts.len() as i32);
        for fort in &self.forts {
            w.i32(fort.residence_id)
                .string(&fort.owner_clan_name)
                .i32(i32::from(fort.siege_in_progress))
                .i32(fort.owned_time);
        }
    }
}

impl Decode for ExShowFortressInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let count = read_count(r, "fortress count")?;
        let mut forts = Vec::new();
        for _ in 0..count {
            forts.push(FortressInfo {
                residence_id: r.i32()?,
                owner_clan_name: r.string(),
                siege_in_progress: r.i32()? != 0,
                owned_time: r.i32()?,
            });
        }
        Ok(Self { forts })
    }
}

// Java: gameserver/network/serverpackets/ExShowFortressSiegeInfo.java
/// The siege window of a fortress under siege (`EX_SHOW_FORTRESS_SIEGE_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowFortressSiegeInfo {
    /// The fortress id.
    pub fort_id: i32,
    /// Java `fort.getFortSize()`: the number of barracks.
    pub barracks_total: i32,
    /// The barracks status ints Java writes: none, or one for a 3 or 4 commander fortress.
    pub barracks: Vec<i32>,
}

impl ExShowFortressSiegeInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowFortressSiegeInfo;

    /// Java's constructor and `writeImpl` together. `commander_spawns` is the number of commander
    /// spawns the fortress has (`FortSiegeManager.getCommanderSpawnList`, 0 for none), and
    /// `commanders` the number of commanders alive in the siege. A commander count of 3 or 4
    /// gives one status int (the control room of a 4 commander fortress is not supported yet, as
    /// in Java); a combination Java has no case for writes nothing.
    pub fn new(fort_id: i32, fort_size: i32, commander_spawns: usize, commanders: usize) -> Self {
        let barracks = if commander_spawns > 0 {
            match (commander_spawns, commanders) {
                (3, c @ 0..=3) => vec![3 - c as i32],
                (4, c @ 0..=4) => vec![5 - c as i32],
                _ => Vec::new(),
            }
        } else {
            zero_barracks(fort_size)
        };
        Self {
            fort_id,
            barracks_total: fort_size,
            barracks,
        }
    }
}

impl Encode for ExShowFortressSiegeInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.fort_id).i32(self.barracks_total);
        for value in &self.barracks {
            w.i32(*value);
        }
    }
}

impl Decode for ExShowFortressSiegeInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            fort_id: r.i32()?,
            barracks_total: r.i32()?,
            barracks: read_barracks(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExShowFortressMapInfo.java
/// The barracks map of one fortress (`EX_SHOW_FORTRESS_MAP_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExShowFortressMapInfo {
    /// The fortress id.
    pub fort_id: i32,
    /// Java `siege.isInProgress()`.
    pub siege_in_progress: bool,
    /// Java `fort.getFortSize()`: the number of barracks.
    pub barracks_total: i32,
    /// One status int per barracks: 0 taken (a commander is alive there), 1 free. Three or four
    /// commanders give their list; with a fortress of 4 commanders Java adds a 1 for the control
    /// room before the fourth. Otherwise the list is zeros, or empty for a commander count Java
    /// has no case for.
    pub barracks: Vec<i32>,
}

impl ExShowFortressMapInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExShowFortressMapInfo;

    /// Java's constructor and `writeImpl` together. `commander_spawned` holds one flag per
    /// commander spawn of the fortress, true when that spawn has a commander alive in the siege.
    /// An empty list (no commander spawns) or a siege that is not in progress gives zeros.
    pub fn new(fort_id: i32, siege_in_progress: bool, fort_size: i32, commander_spawned: &[bool]) -> Self {
        // Java: 0 when the commander is spawned, 1 when not.
        let status = |spawned: bool| i32::from(!spawned);
        let barracks = if !commander_spawned.is_empty() && siege_in_progress {
            match commander_spawned.len() {
                3 => commander_spawned.iter().map(|&spawned| status(spawned)).collect(),
                4 => {
                    let mut barracks = Vec::new();
                    for (index, &spawned) in commander_spawned.iter().enumerate() {
                        if index == 3 {
                            // The control room, which Java writes as free (1) before the fourth
                            // commander; the control room is not supported in Java yet.
                            barracks.push(1);
                        }
                        barracks.push(status(spawned));
                    }
                    barracks
                }
                _ => Vec::new(),
            }
        } else {
            zero_barracks(fort_size)
        };
        Self {
            fort_id,
            siege_in_progress,
            barracks_total: fort_size,
            barracks,
        }
    }
}

impl Encode for ExShowFortressMapInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.fort_id)
            .i32(i32::from(self.siege_in_progress))
            .i32(self.barracks_total);
        for value in &self.barracks {
            w.i32(*value);
        }
    }
}

impl Decode for ExShowFortressMapInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            fort_id: r.i32()?,
            siege_in_progress: r.i32()? != 0,
            barracks_total: r.i32()?,
            barracks: read_barracks(r)?,
        })
    }
}
