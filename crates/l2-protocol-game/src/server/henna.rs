//! Server packets of the symbol maker (hennas): the draw list, the remove list and the two
//! previews (L-01). `HennaInfo` (the summary the client gets at login and after every change)
//! is in `enterworld.rs`.
//!
//! Values only, like the other server packet modules: Java builds these from `Player` and
//! `Henna`, the caller passes the numbers.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/HennaEquipList.java (one row)
/// One dye of [`HennaEquipList`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HennaEquipEntry {
    pub dye_id: i32,
    /// Item id of the dye.
    pub dye_item_id: i32,
    /// Dye items needed (`getWearCount`).
    pub wear_count: i64,
    /// Adena needed (`getWearFee`).
    pub wear_fee: i64,
    /// `isAllowedClass(playerClass)`: whether the player's class can wear it.
    pub allowed: bool,
}

// Java: gameserver/network/serverpackets/HennaEquipList.java
/// The dyes the character can draw with the dye items it carries (`HENNA_EQUIP_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HennaEquipList {
    /// `Player.getAdena()`.
    pub adena: i64,
    /// The count Java writes: the size of the whole henna list of the class, not of `entries`.
    // NOTE(java-quirk): Java writes `_hennaEquipList.size()` and then only the dyes the player has
    // an item of, so the count is usually larger than the number of rows that follow.
    pub total: i32,
    pub entries: Vec<HennaEquipEntry>,
}

impl HennaEquipList {
    pub const OPCODE: ServerOpcode = ServerOpcode::HennaEquipList;
    /// Java writes the constant 3 ("available equip slot").
    const SLOTS: i32 = 3;
    /// One row: dye id, dye item id (ints), count, fee (longs), allowed (int).
    const ENTRY_LEN: usize = 4 + 4 + 8 + 8 + 4;
}

impl Encode for HennaEquipList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i64(self.adena).i32(Self::SLOTS).i32(self.total);
        for e in &self.entries {
            w.i32(e.dye_id)
                .i32(e.dye_item_id)
                .i64(e.wear_count)
                .i64(e.wear_fee)
                .i32(i32::from(e.allowed));
        }
    }
}

impl Decode for HennaEquipList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let adena = r.i64()?;
        r.skip(4)?; // slots
        let total = r.i32()?;
        // The row count is not on the wire (see `total`): the rows run to the end.
        let mut entries = Vec::new();
        while r.remaining() >= Self::ENTRY_LEN {
            entries.push(HennaEquipEntry {
                dye_id: r.i32()?,
                dye_item_id: r.i32()?,
                wear_count: r.i64()?,
                wear_fee: r.i64()?,
                allowed: r.i32()? != 0,
            });
        }
        Ok(Self { adena, total, entries })
    }
}

// Java: gameserver/network/serverpackets/HennaRemoveList.java (one row)
/// One worn dye of [`HennaRemoveList`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HennaRemoveEntry {
    pub dye_id: i32,
    pub dye_item_id: i32,
    /// Dye items given back (`getCancelCount`).
    pub cancel_count: i32,
    /// Adena to pay (`getCancelFee`).
    pub cancel_fee: i32,
}

// Java: gameserver/network/serverpackets/HennaRemoveList.java
/// The dyes the character wears and can remove (`HENNA_UNEQUIP_LIST`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HennaRemoveList {
    pub adena: i64,
    /// The count Java writes: `3 - getHennaEmptySlots()`.
    // NOTE(java-quirk): `getHennaEmptySlots` is 0 for a class of level 0 and 2 minus the dyes for
    // level 1, so this count is not the number of rows that follow.
    pub count: i32,
    pub entries: Vec<HennaRemoveEntry>,
}

impl HennaRemoveList {
    pub const OPCODE: ServerOpcode = ServerOpcode::HennaUnequipList;
    /// Seven ints per row.
    const ENTRY_LEN: usize = 7 * 4;
}

impl Encode for HennaRemoveList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i64(self.adena).i32(0).i32(self.count);
        for e in &self.entries {
            w.i32(e.dye_id)
                .i32(e.dye_item_id)
                .i32(e.cancel_count)
                .i32(0)
                .i32(e.cancel_fee)
                .i32(0)
                .i32(1);
        }
    }
}

impl Decode for HennaRemoveList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let adena = r.i64()?;
        r.skip(4)?;
        let count = r.i32()?;
        let mut entries = Vec::new();
        while r.remaining() >= Self::ENTRY_LEN {
            let dye_id = r.i32()?;
            let dye_item_id = r.i32()?;
            let cancel_count = r.i32()?;
            r.skip(4)?;
            let cancel_fee = r.i32()?;
            r.skip(8)?;
            entries.push(HennaRemoveEntry {
                dye_id,
                dye_item_id,
                cancel_count,
                cancel_fee,
            });
        }
        Ok(Self { adena, count, entries })
    }
}

/// The six base stats before and after a dye, in Java's wire order INT, STR, CON, MEN, DEX, WIT.
/// `now` is `Player.getINT()` ..., `after` the value Java writes with `writeByte` (its low byte).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct HennaStatPreview {
    pub now: i32,
    pub after: i8,
}

/// The common body of [`HennaItemInfo`] and [`HennaItemRemoveInfo`]; only the opcode and the
/// meaning of the count and fee differ.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HennaPreview {
    pub dye_id: i32,
    pub dye_item_id: i32,
    /// `getWearCount` / `getCancelCount`.
    pub count: i64,
    /// `getWearFee` / `getCancelFee`.
    pub fee: i64,
    /// `isAllowedClass(playerClass)`.
    pub allowed: bool,
    pub adena: i64,
    pub int: HennaStatPreview,
    pub str: HennaStatPreview,
    pub con: HennaStatPreview,
    pub men: HennaStatPreview,
    pub dex: HennaStatPreview,
    pub wit: HennaStatPreview,
}

impl HennaPreview {
    fn write(&self, w: &mut PacketWriter) {
        w.i32(self.dye_id)
            .i32(self.dye_item_id)
            .i64(self.count)
            .i64(self.fee)
            .i32(i32::from(self.allowed))
            .i64(self.adena);
        for s in [&self.int, &self.str, &self.con, &self.men, &self.dex, &self.wit] {
            w.i32(s.now).i8(s.after);
        }
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        let mut p = Self {
            dye_id: r.i32()?,
            dye_item_id: r.i32()?,
            count: r.i64()?,
            fee: r.i64()?,
            allowed: r.i32()? != 0,
            adena: r.i64()?,
            ..Self::default()
        };
        for s in [&mut p.int, &mut p.str, &mut p.con, &mut p.men, &mut p.dex, &mut p.wit] {
            s.now = r.i32()?;
            s.after = r.i8()?;
        }
        Ok(p)
    }
}

// Java: gameserver/network/serverpackets/HennaItemDrawInfo.java
/// The preview of drawing a dye: the stats with the dye added (`HENNA_ITEM_INFO`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HennaItemInfo(pub HennaPreview);

impl HennaItemInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::HennaItemInfo;
}

impl Encode for HennaItemInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        self.0.write(w);
    }
}

impl Decode for HennaItemInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self(HennaPreview::read(r)?))
    }
}

// Java: gameserver/network/serverpackets/HennaItemRemoveInfo.java
/// The preview of removing a dye: the stats with the dye taken off (`HENNA_UNEQUIP_INFO`).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HennaItemRemoveInfo(pub HennaPreview);

impl HennaItemRemoveInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::HennaUnequipInfo;
}

impl Encode for HennaItemRemoveInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        self.0.write(w);
    }
}

impl Decode for HennaItemRemoveInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self(HennaPreview::read(r)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip<T: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &T) {
        let bytes = packet.to_bytes();
        let decoded = T::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(&decoded, packet);
    }

    #[test]
    fn equip_list_layout_and_round_trip() {
        let packet = HennaEquipList {
            adena: 0x0102,
            total: 5,
            entries: vec![HennaEquipEntry {
                dye_id: 3,
                dye_item_id: 4445,
                wear_count: 10,
                wear_fee: 37000,
                allowed: true,
            }],
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0xEE);
        // adena, slots 3, total 5 (the count is not the number of rows: Java quirk)
        assert_eq!(&bytes[1..9], &[2, 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bytes[9..17], &[3, 0, 0, 0, 5, 0, 0, 0]);
        assert_eq!(bytes.len(), 17 + 28);
        round_trip(&packet);
        round_trip(&HennaEquipList {
            adena: 0,
            total: 0,
            entries: vec![],
        });
    }

    #[test]
    fn remove_list_layout_and_round_trip() {
        let packet = HennaRemoveList {
            adena: 7,
            count: 2,
            entries: vec![HennaRemoveEntry {
                dye_id: 9,
                dye_item_id: 4446,
                cancel_count: 5,
                cancel_fee: 7400,
            }],
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0xE6);
        assert_eq!(&bytes[1..9], &[7, 0, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&bytes[9..17], &[0, 0, 0, 0, 2, 0, 0, 0]);
        // dyeId, itemId, count, 0, fee, 0, 1
        assert_eq!(&bytes[17..21], &[9, 0, 0, 0]);
        assert_eq!(&bytes[41..45], &[1, 0, 0, 0]);
        assert_eq!(bytes.len(), 17 + 28);
        round_trip(&packet);
    }

    fn preview() -> HennaPreview {
        let stat = |now, after| HennaStatPreview { now, after };
        HennaPreview {
            dye_id: 1,
            dye_item_id: 4445,
            count: 10,
            fee: 37000,
            allowed: true,
            adena: 99,
            int: stat(40, 41),
            str: stat(60, 61),
            con: stat(43, 40),
            men: stat(39, 39),
            dex: stat(30, 30),
            wit: stat(11, 11),
        }
    }

    #[test]
    fn previews_layout_and_round_trip() {
        let packet = HennaItemInfo(preview());
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0xE4);
        // opcode + 2 ints + 2 longs + int + long + 6 * (int + byte)
        assert_eq!(bytes.len(), 1 + 8 + 16 + 4 + 8 + 30);
        // the first stat is INT: current 40, byte 41
        assert_eq!(&bytes[37..42], &[40, 0, 0, 0, 41]);
        round_trip(&packet);
        let remove = HennaItemRemoveInfo(preview());
        assert_eq!(remove.to_bytes()[0], 0xE7);
        round_trip(&remove);
        assert!(HennaItemInfo::decode(&mut PacketReader::new(&[0; 40])).is_err());
    }
}
