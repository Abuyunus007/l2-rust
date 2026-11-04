//! Client packets of the fortresses: the fortress list, the siege status and the barracks map.
//!
//! These only read the packet. The answers are built by the world from the fortress tables.

client_unit_packet!(
    /// Asks for the list of all fortresses (`0xD0` 0x003D, `REQUEST_ALL_FORTRESS_INFO`).
    RequestAllFortressInfo,
    ex RequestAllFortressInfo
);

client_unit_packet!(
    /// Asks for the siege window of every fortress that is under siege (`0xD0` 0x003F,
    /// `REQUEST_FORTRESS_SIEGE_INFO`).
    RequestFortressSiegeInfo,
    ex RequestFortressSiegeInfo
);

client_packet!(
    /// Asks for the barracks map of one fortress (`0xD0` 0x0048, `REQUEST_FORTRESS_MAP_INFO`).
    RequestFortressMapInfo,
    ex RequestFortressMapInfo {
        /// The fortress id.
        fortress_id: i32,
    }
);
