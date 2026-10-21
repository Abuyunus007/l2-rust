//! Client packets of the duel: the challenge, the answer to it and the surrender.
//!
//! These only read the packet. Every check (distance, party, who can duel) is the world's job
//! (`l2_world::duel`).

// Java: gameserver/network/clientpackets/RequestDuelStart.java
client_packet!(
    /// The player challenges another player (or that player's party) to a duel
    /// (`0xD0` 0x001B, `REQUEST_DUEL_START`).
    RequestDuelStart,
    ex RequestDuelStart {
        /// The name of the challenged player.
        player: String,
        /// `1` for a party duel, anything else for a 1 vs 1 duel.
        party_duel: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestDuelAnswerStart.java
client_packet!(
    /// The challenged player answers (`0xD0` 0x001C, `REQUEST_DUAL_ANSWER_START`: the opcode
    /// table spells it "DUAL", the Java class is `RequestDuelAnswerStart`).
    RequestDuelAnswerStart,
    ex RequestDualAnswerStart {
        /// `1` for a party duel.
        party_duel: i32,
        /// Java reads it and never uses it.
        unk1: i32,
        /// `1` accept, `-1` the client refuses duel requests, anything else declines.
        response: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestDuelSurrender.java
client_unit_packet!(
    /// The player gives up the duel (`0xD0` 0x0045, `REQUEST_DUEL_SURRENDER`).
    RequestDuelSurrender,
    ex RequestDuelSurrender
);
