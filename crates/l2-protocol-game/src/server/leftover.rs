//! Server packets of L-02: the wear shop preview and the item link answer.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::charscreen::paperdoll as pd;
use super::items::{ItemInfo, read_item, write_item};
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// The paperdoll slots `ShopPreviewInfo` writes, in the order Java writes them.
///
/// NOTE(java-quirk): `RHAND` is there twice (positions 7 and 14), like in `paperdoll::WIRE_ORDER`.
pub const PREVIEW_SLOTS: [usize; 19] = [
    pd::UNDER,
    pd::REAR,
    pd::LEAR,
    pd::NECK,
    pd::RFINGER,
    pd::LFINGER,
    pd::HEAD,
    pd::RHAND,
    pd::LHAND,
    pd::GLOVES,
    pd::CHEST,
    pd::LEGS,
    pd::FEET,
    pd::CLOAK,
    pd::RHAND,
    pd::HAIR,
    pd::HAIR2,
    pd::RBRACELET,
    pd::LBRACELET,
];

// Java: gameserver/network/serverpackets/ShopPreviewInfo.java
/// The items a player tries on (`SHOP_PREVIEW_INFO`, 0xF6): the item id per paperdoll slot, 0 for
/// none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ShopPreviewInfo {
    /// The item id at each position of [`PREVIEW_SLOTS`].
    pub item_ids: [i32; 19],
}

impl ShopPreviewInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ShopPreviewInfo;

    /// Java `new ShopPreviewInfo(Map<slot, itemId>)`: `getFromList` returns 0 for a slot that is
    /// not in the map, and a slot that is listed twice gets the same id twice.
    pub fn from_slots(slots: &[(usize, i32)]) -> Self {
        let mut item_ids = [0; 19];
        for (position, slot) in PREVIEW_SLOTS.iter().enumerate() {
            item_ids[position] = slots
                .iter()
                .find(|(s, _)| s == slot)
                .map_or(0, |(_, item_id)| *item_id);
        }
        Self { item_ids }
    }
}

impl Encode for ShopPreviewInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(pd::TOTAL_SLOTS as i32);
        for id in self.item_ids {
            w.i32(id);
        }
    }
}

impl Decode for ShopPreviewInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.skip(4)?;
        let mut item_ids = [0; 19];
        for id in &mut item_ids {
            *id = r.i32()?;
        }
        Ok(Self { item_ids })
    }
}

// Java: gameserver/network/serverpackets/ExRpItemLink.java
/// An item a player showed in the chat (`0xFE` 0x006C, `EX_RP_ITEM_LINK`): the same fields as one
/// item of an item list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExRpItemLink {
    pub item: ItemInfo,
}

impl ExRpItemLink {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExRpItemLink;
}

impl Encode for ExRpItemLink {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        write_item(w, &self.item);
    }
}

impl Decode for ExRpItemLink {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { item: read_item(r)? })
    }
}

// Java: gameserver/network/serverpackets/ExChangeClientEffectInfo.java
/// A change of the client's zone state, fog or post effect (`0xFE` 0x00C2,
/// `EX_CHANGE_CLIENT_EFFECT_INFO`): Freya's castle uses it to switch the zone effect.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExChangeClientEffectInfo {
    /// 0 = ChangeZoneState, 1 = SetL2Fog, 2 = postEffectData.
    pub kind: i32,
    pub key: i32,
    pub value: i32,
}

impl ExChangeClientEffectInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExChangeClientEffectInfo;
    /// Java `STATIC_FREYA_DEFAULT`.
    pub const STATIC_FREYA_DEFAULT: Self = Self::new(0, 0, 1);
    /// Java `STATIC_FREYA_DESTROYED`.
    pub const STATIC_FREYA_DESTROYED: Self = Self::new(0, 0, 2);

    pub const fn new(kind: i32, key: i32, value: i32) -> Self {
        Self { kind, key, value }
    }
}

impl Encode for ExChangeClientEffectInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.kind).i32(self.key).i32(self.value);
    }
}

impl Decode for ExChangeClientEffectInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            kind: r.i32()?,
            key: r.i32()?,
            value: r.i32()?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bytes_of(packet: &impl Encode) -> Vec<u8> {
        let mut w = PacketWriter::new();
        packet.encode(&mut w);
        w.into_bytes()
    }

    #[test]
    fn shop_preview_info_layout_and_round_trip() {
        let packet = ShopPreviewInfo::from_slots(&[(pd::CHEST, 2376), (pd::RHAND, 1)]);
        let bytes = bytes_of(&packet);
        // 0xF6, 25 (PAPERDOLL_TOTALSLOTS), then 19 ints.
        assert_eq!(bytes.len(), 1 + 4 + 19 * 4);
        assert_eq!(bytes[0], 0xF6);
        assert_eq!(&bytes[1..5], &25i32.to_le_bytes());
        let id_at = |position: usize| {
            i32::from_le_bytes(bytes[5 + position * 4..9 + position * 4].try_into().unwrap())
        };
        // Position 10 is CHEST, positions 7 and 14 are both RHAND.
        assert_eq!(id_at(10), 2376);
        assert_eq!(id_at(7), 1);
        assert_eq!(id_at(14), 1);
        assert_eq!(id_at(0), 0);
        assert_eq!(
            ShopPreviewInfo::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn ex_rp_item_link_layout_and_round_trip() {
        let mut item = ItemInfo::plain(268_500_000, 57, 1_000);
        item.type2 = 4;
        item.body_part = 0;
        let packet = ExRpItemLink { item };
        let bytes = bytes_of(&packet);
        // 0xFE, the ex id 0x006C, then the item the way an item list writes it.
        assert_eq!(&bytes[..3], &[0xFE, 0x6C, 0x00]);
        assert_eq!(&bytes[3..7], &268_500_000i32.to_le_bytes());
        assert_eq!(&bytes[7..11], &57i32.to_le_bytes());
        // location, then the count as a long.
        assert_eq!(&bytes[15..23], &1_000i64.to_le_bytes());
        // 4 + 4 + 4 + 8 + 3 * 2 + 4 + 2 * 2 + 3 * 4 + 2 * 2 + 6 * 2 + 3 * 2 after the 3 id bytes.
        assert_eq!(bytes.len(), 3 + 4 + 4 + 4 + 8 + 6 + 4 + 4 + 12 + 4 + 12 + 6);
        assert_eq!(
            ExRpItemLink::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            packet
        );
    }

    #[test]
    fn ex_change_client_effect_info_layout_and_round_trip() {
        let packet = ExChangeClientEffectInfo::STATIC_FREYA_DESTROYED;
        let bytes = bytes_of(&packet);
        // 0xFE, the ex id 0x00C2, then type, key and value.
        assert_eq!(&bytes[..3], &[0xFE, 0xC2, 0x00]);
        assert_eq!(&bytes[3..], &[0, 0, 0, 0, 0, 0, 0, 0, 2, 0, 0, 0]);
        assert_eq!(
            ExChangeClientEffectInfo::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            packet
        );
        assert_eq!(ExChangeClientEffectInfo::STATIC_FREYA_DEFAULT.value, 1);
        let fog = ExChangeClientEffectInfo::new(1, 5, 7);
        assert_eq!(
            ExChangeClientEffectInfo::decode(&mut PacketReader::new(&bytes_of(&fog)[3..])).unwrap(),
            fog
        );
    }
}
