//! Client packets of the Cube game (the minigame in the cube arenas) and the movie end.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_ex_opcode;
use crate::generated::opcodes::ExClientOpcode;

// Java: gameserver/network/clientpackets/RequestExCubeGameChangeTeam.java
/// The player asks to switch team in the Cube game waiting room (`0xD0` 0x005A).
///
/// Java's `readImpl` adds 1 to the arena the client sends (`-1, 0, 1, 2`), so `arena` holds the
/// Java field (`0..=3`). Encoding takes the 1 off again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestExCubeGameChangeTeam {
    /// Java `_arena` (the wire value plus 1).
    pub arena: i32,
    /// Java `_team`.
    pub team: i32,
}

impl RequestExCubeGameChangeTeam {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestExCubeGameChangeTeam;
}

impl Encode for RequestExCubeGameChangeTeam {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.arena.wrapping_sub(1)).i32(self.team);
    }
}

impl Decode for RequestExCubeGameChangeTeam {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            arena: r.i32()?.wrapping_add(1),
            team: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestExCubeGameReadyAnswer.java
/// The answer to the Cube game "ready?" question (`0xD0` 0x005C).
///
/// Like [`RequestExCubeGameChangeTeam`], `arena` is the Java field (wire value plus 1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestExCubeGameReadyAnswer {
    /// Java `_arena`.
    pub arena: i32,
    /// Java `_answer`: `1` confirm, `0` cancel.
    pub answer: i32,
}

impl RequestExCubeGameReadyAnswer {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestExCubeGameReadyAnswer;
}

impl Encode for RequestExCubeGameReadyAnswer {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.arena.wrapping_sub(1)).i32(self.answer);
    }
}

impl Decode for RequestExCubeGameReadyAnswer {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            arena: r.i32()?.wrapping_add(1),
            answer: r.i32()?,
        })
    }
}

// Java: gameserver/network/clientpackets/EndScenePlayer.java
client_packet!(
    /// The client reports that a movie (scene) has ended (`0xD0` 0x005B).
    EndScenePlayer,
    ex EndScenePlayer {
        /// Java `_movieId` (`readInt()`), the id of `ExStartScenePlayer`.
        movie_id: i32,
    }
);
