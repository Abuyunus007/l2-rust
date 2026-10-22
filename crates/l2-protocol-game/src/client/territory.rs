//! Client packets of the Territory War: the registration of a clan or a mercenary and the
//! territory info request.
//!
//! These only read the packet. The registration period, the owner's exemption, the level check
//! and the registration lists are the world's job (M7 territory cards).

// Java: gameserver/network/clientpackets/RequestJoinDominionWar.java
client_packet!(
    /// The clan or the player registers for a territory war, or takes the registration back
    /// (`0xD0` 0x0057, `REQUEST_EX_JOIN_DOMINION_WAR`).
    RequestJoinDominionWar,
    ex RequestExJoinDominionWar {
        /// The territory id: the castle id plus 80 (Java subtracts 80 to get the castle).
        territory_id: i32,
        /// `1` a clan registration, anything else a mercenary one (Java compares with 1).
        is_clan: i32,
        /// `1` register, anything else take the registration back (Java compares with 1).
        is_joining: i32,
    }
);

// Java: gameserver/network/clientpackets/RequestDominionInfo.java
client_unit_packet!(
    /// Asks for the territory list (`0xD0` 0x0058, `REQUEST_EX_DOMINION_INFO`).
    RequestDominionInfo,
    ex RequestExDominionInfo
);
