//! The packets of the GM windows: `GMViewItemList`.
//!
//! Java: `gameserver/network/serverpackets/GMViewItemList.java` (`AbstractItemPacket.writeItem`
//! for the entries).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::items::{ItemInfo, read_item, write_item};
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/serverpackets/GMViewItemList.java
/// The item window a GM sees of another character (`GM_VIEW_ITEM_LIST`): the owner's name, the
/// inventory limit and every item of the inventory, quest items included (Java filters nothing).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GmViewItemList {
    /// `player.getName()`.
    pub player_name: String,
    /// `player.getInventoryLimit()`.
    pub limit: i32,
    pub items: Vec<ItemInfo>,
}

impl GmViewItemList {
    pub const OPCODE: ServerOpcode = ServerOpcode::GmViewItemList;
}

impl Encode for GmViewItemList {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.player_name);
        w.i32(self.limit);
        // "show window ??": Java always writes 1.
        w.u16(1);
        w.u16(self.items.len() as u16);
        for item in &self.items {
            write_item(w, item);
        }
    }
}

impl Decode for GmViewItemList {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let player_name = r.string();
        let limit = r.i32()?;
        let _show_window = r.u16()?;
        let count = r.u16()?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(read_item(r)?);
        }
        Ok(Self {
            player_name,
            limit,
            items,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> GmViewItemList {
        let mut sword = ItemInfo::plain(268_435_456, 1_000, 1);
        sword.equipped = 1;
        sword.enchant = 3;
        GmViewItemList {
            player_name: "Gm Target".to_owned(),
            limit: 80,
            items: vec![sword, ItemInfo::plain(268_435_457, 57, 5_000)],
        }
    }

    #[test]
    fn round_trips() {
        let packet = sample();
        let bytes = packet.to_bytes();
        assert_eq!(bytes[0], 0x9A);
        let decoded = GmViewItemList::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decoded, packet);
    }

    #[test]
    fn layout_is_the_java_write_impl() {
        let bytes = GmViewItemList {
            player_name: "A".to_owned(),
            limit: 0x0102_0304,
            items: vec![ItemInfo::plain(1, 2, 3)],
        }
        .to_bytes();
        // opcode, "A" + terminator (UTF-16LE), limit, show window = 1, one item.
        assert_eq!(&bytes[..1], &[0x9A]);
        assert_eq!(&bytes[1..5], &[b'A', 0, 0, 0]);
        assert_eq!(&bytes[5..9], &[4, 3, 2, 1]);
        assert_eq!(&bytes[9..11], &[1, 0]);
        assert_eq!(&bytes[11..13], &[1, 0]);
        // 4 + 4 + 4 + 8 + 2 + 2 + 2 + 4 + 2 + 2 + 4 + 4 + 4 + 2 + 2 + 12 + 6 = 68 bytes per item.
        assert_eq!(bytes.len(), 13 + 68);
    }

    #[test]
    fn empty_list_round_trips() {
        let packet = GmViewItemList::default();
        let bytes = packet.to_bytes();
        let decoded = GmViewItemList::decode(&mut PacketReader::new(&bytes[1..])).unwrap();
        assert_eq!(decoded, packet);
    }
}
