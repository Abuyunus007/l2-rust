//! Server packets of the duel: the challenge, the countdown signals and the vitals of the
//! opponents.
//!
//! Values only, like the other server packets: `ExDuelUpdateUserInfo` gets the numbers Java reads
//! from the `Player`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/ExDuelAskStart.java
server_packet!(
    /// The challenged player is asked to accept (`EX_DUEL_ASK_START`).
    ExDuelAskStart,
    ExDuelAskStart {
        /// The name of the challenger.
        requestor_name: String,
        /// `1` for a party duel.
        party_duel: i32,
    }
);

/// A server packet that is one int: Java writes the `boolean` of a party duel with `writeInt`.
macro_rules! party_duel_packet {
    ($(#[$meta:meta])* $name:ident, $op:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name {
            pub party_duel: bool,
        }

        impl $name {
            pub const OPCODE: ServerOpcode = ServerOpcode::$op;
        }

        impl Encode for $name {
            fn encode(&self, w: &mut PacketWriter) {
                write_opcode(w, Self::OPCODE);
                w.i32(i32::from(self.party_duel));
            }
        }

        impl Decode for $name {
            fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
                Ok(Self {
                    party_duel: r.i32()? != 0,
                })
            }
        }
    };
}

// Java: gameserver/network/serverpackets/ExDuelReady.java
party_duel_packet!(
    /// The duel is about to start (`EX_DUEL_READY`).
    ExDuelReady,
    ExDuelReady
);

// Java: gameserver/network/serverpackets/ExDuelStart.java
party_duel_packet!(
    /// The duel has started (`EX_DUEL_START`).
    ExDuelStart,
    ExDuelStart
);

// Java: gameserver/network/serverpackets/ExDuelEnd.java
party_duel_packet!(
    /// The duel is over (`EX_DUEL_END`).
    ExDuelEnd,
    ExDuelEnd
);

// Java: gameserver/network/serverpackets/ExDuelUpdateUserInfo.java
server_packet!(
    /// The vitals of an opponent, for the duel window (`EX_DUEL_UPDATE_USER_INFO`).
    ExDuelUpdateUserInfo,
    ExDuelUpdateUserInfo {
        name: String,
        object_id: i32,
        class_id: i32,
        level: i32,
        /// `(int) getCurrentHp()`.
        current_hp: i32,
        max_hp: i32,
        /// `(int) getCurrentMp()`.
        current_mp: i32,
        max_mp: i32,
        /// `(int) getCurrentCp()`.
        current_cp: i32,
        max_cp: i32,
    }
);
