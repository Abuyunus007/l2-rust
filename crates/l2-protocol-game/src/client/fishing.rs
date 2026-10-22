//! Client packets of fishing.

// Java: gameserver/network/clientpackets/RequestExFishRanking.java
client_unit_packet!(
    /// The player asked for the fishing ranking of the week (`0xD0` 0x0018). Java reads nothing;
    /// with `AltFishChampionshipEnabled` it sends the mid result page.
    RequestExFishRanking,
    ex RequestExFishRanking
);
