//! Client packets of the cursed weapons and the raid boss points record.

// Java: gameserver/network/clientpackets/RequestCursedWeaponList.java
client_unit_packet!(
    /// The player asks for the ids of the cursed weapons (`0xD0` 0x002A). Java reads nothing.
    RequestCursedWeaponList,
    ex RequestCursedWeaponList
);

// Java: gameserver/network/clientpackets/RequestCursedWeaponLocation.java
client_unit_packet!(
    /// The player asks where the cursed weapons are (`0xD0` 0x002B). Java reads nothing.
    RequestCursedWeaponLocation,
    ex RequestCursedWeaponLocation
);

// Java: gameserver/network/clientpackets/RequestGetBossRecord.java
client_packet!(
    /// The player asks for the raid boss points record (`0xD0` 0x0040). Java only logs a non-zero
    /// id, which the client never sends; the record itself is the same for every id.
    RequestGetBossRecord,
    ex RequestGetBossRecord {
        /// Java `_bossId` (`readInt()`).
        boss_id: i32,
    }
);
