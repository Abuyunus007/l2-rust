//! Client packets of the residences that the clan hall card needs: the list of clan halls.
//!
//! This only reads the packet. The answer is built by the world from the clan hall table.

// Java: gameserver/network/clientpackets/RequestAllAgitInfo.java
client_unit_packet!(
    /// Asks for the list of all auctionable clan halls (`0xD0` 0x003E, `REQUEST_ALL_AGIT_INGO`:
    /// the generated table keeps Java's spelling of the name).
    RequestAllAgitInfo,
    ex RequestAllAgitIngo
);
