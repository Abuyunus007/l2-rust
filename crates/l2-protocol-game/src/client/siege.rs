//! Client packets of the castle sieges and the clan hall siege: the siege window, the attacker and
//! defender lists, joining or leaving a siege and the lord's approval of a defender.
//!
//! These only read the packet. Who may do what (castle owner leader, clan access, dissolving
//! clan, registration period) is the world's job (M7 siege cards).

// Java: gameserver/network/clientpackets/RequestSiegeInfo.java
client_unit_packet!(
    /// Asks for the siege window. The packet has no fields; Java answers nothing in `runImpl`
    /// (`REQUEST_SIEGE_INFO`).
    RequestSiegeInfo,
    plain RequestSiegeInfo
);

// Java: gameserver/network/clientpackets/RequestAllCastleInfo.java
client_unit_packet!(
    /// Asks for the castle list of the map window (`0xD0` 0x003C, `REQUEST_ALL_CASTLE_INFO`).
    RequestAllCastleInfo,
    ex RequestAllCastleInfo
);

// Java: gameserver/network/clientpackets/RequestSiegeAttackerList.java
client_packet!(
    /// Asks for the attacker list of a castle or clan hall (`REQUEST_SIEGE_ATTACKER_LIST`).
    RequestSiegeAttackerList,
    plain RequestSiegeAttackerList {
        /// The residence id (castle or siegable hall).
        castle_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestSiegeDefenderList.java
client_packet!(
    /// Asks for the defender list of a castle (`REQUEST_SIEGE_DEFENDER_LIST`).
    RequestSiegeDefenderList,
    plain RequestSiegeDefenderList {
        /// The castle id.
        castle_id: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestJoinSiege.java
client_packet!(
    /// The clan registers as attacker or defender, or takes its registration back
    /// (`REQUEST_JOIN_SIEGE`).
    RequestJoinSiege,
    plain RequestJoinSiege {
        /// The residence id.
        castle_id: i32,
        /// `1` attacker, anything else defender (Java compares with 1).
        is_attacker: i32,
        /// `1` register, anything else take the registration back (Java compares with 1).
        is_joining: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestConfirmSiegeWaitingList.java
client_packet!(
    /// The castle lord approves or refuses a waiting defender clan
    /// (`REQUEST_CONFIRM_SIEGE_WAITING_LIST`). Java reads the castle id, then the clan id, then the
    /// answer.
    RequestConfirmSiegeWaitingList,
    plain RequestConfirmSiegeWaitingList {
        /// The castle id.
        castle_id: i32,
        /// The clan that asks to defend.
        clan_id: i32,
        /// `1` approve, anything else refuse (Java compares with 1).
        approved: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestSetCastleSiegeTime.java
client_packet!(
    /// The castle lord picks the hour of the next siege (`REQUEST_SET_CASTLE_SIEGE_TIME`).
    RequestSetCastleSiegeTime,
    plain RequestSetCastleSiegeTime {
        /// The castle id.
        castle_id: i32,
        /// The chosen time in unix seconds (Java multiplies it by 1000).
        time: i32,
    }
);
