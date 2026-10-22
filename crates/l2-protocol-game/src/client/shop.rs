//! Client packets of the NPC shops: buying, selling and refunding, and closing the shop window.
//!
//! The list packets keep Java's "bad packet" rule: when the count is out of range, the byte
//! length does not match the count, or one entry has a nonsense value, Java reads no items and
//! the handler then answers `ActionFailed`. That is `items: None` here. Java also rejects a count
//! above `PlayerConfig.MAX_ITEM_IN_PACKET`; that limit comes from the configuration, so the world
//! compares `items.len()` with it and treats a longer list as `None`.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{write_ex_opcode, write_opcode};
use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

/// Java's shared check of the three list packets: `count >= 1` and `count * BATCH_LENGTH ==
/// remaining()`. `None` means Java would drop the list.
fn read_batch<T>(
    r: &mut PacketReader,
    batch_length: usize,
    mut one: impl FnMut(&mut PacketReader) -> Result<Option<T>, PacketError>,
) -> Result<Option<Vec<T>>, PacketError> {
    let count = r.i32()?;
    // The batch lengths are small constants, so the cast cannot truncate.
    let expected = i64::from(count) * batch_length as i64;
    if count < 1 || expected != r.remaining() as i64 {
        return Ok(None);
    }
    let mut items = Vec::new();
    for _ in 0..count {
        match one(r)? {
            Some(item) => items.push(item),
            None => return Ok(None),
        }
    }
    Ok(Some(items))
}

/// A bad list is written as the count `0`, which `read_batch` drops the same way.
fn write_count<T>(w: &mut PacketWriter, items: &Option<Vec<T>>) {
    w.i32(items.as_ref().map_or(0, |v| v.len() as i32));
}

/// One wanted item of a [`RequestBuyItem`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyItemEntry {
    pub item_id: i32,
    pub count: i64,
}

// Java: gameserver/network/clientpackets/RequestBuyItem.java
/// The player buys from a merchant's list (`REQUEST_BUY_ITEM`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestBuyItem {
    pub list_id: i32,
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<BuyItemEntry>>,
}

impl RequestBuyItem {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestBuyItem;
    /// Bytes per item: an int and a long.
    pub const BATCH_LENGTH: usize = 12;
}

impl Encode for RequestBuyItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.list_id);
        write_count(w, &self.items);
        for item in self.items.iter().flatten() {
            w.i32(item.item_id).i64(item.count);
        }
    }
}

impl Decode for RequestBuyItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let list_id = r.i32()?;
        let items = read_batch(r, Self::BATCH_LENGTH, |r| {
            let item_id = r.i32()?;
            let count = r.i64()?;
            Ok((item_id >= 1 && count >= 1).then_some(BuyItemEntry { item_id, count }))
        })?;
        Ok(Self { list_id, items })
    }
}

/// One item of a [`RequestSellItem`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SellItemEntry {
    pub object_id: i32,
    pub item_id: i32,
    pub count: i64,
}

// Java: gameserver/network/clientpackets/RequestSellItem.java
/// The player sells items to a merchant (`REQUEST_SELL_ITEM`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestSellItem {
    pub list_id: i32,
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<SellItemEntry>>,
}

impl RequestSellItem {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestSellItem;
    /// Bytes per item: two ints and a long.
    pub const BATCH_LENGTH: usize = 16;
}

impl Encode for RequestSellItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.list_id);
        write_count(w, &self.items);
        for item in self.items.iter().flatten() {
            w.i32(item.object_id).i32(item.item_id).i64(item.count);
        }
    }
}

impl Decode for RequestSellItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let list_id = r.i32()?;
        let items = read_batch(r, Self::BATCH_LENGTH, |r| {
            let object_id = r.i32()?;
            let item_id = r.i32()?;
            let count = r.i64()?;
            Ok(
                (object_id >= 1 && item_id >= 1 && count >= 1).then_some(SellItemEntry {
                    object_id,
                    item_id,
                    count,
                }),
            )
        })?;
        Ok(Self { list_id, items })
    }
}

// Java: gameserver/network/clientpackets/RequestRefundItem.java
/// The player takes back items it sold (`0xD0 0x0075`, an ex packet). Each entry is the index in
/// the refund list the last `ExBuySellList` showed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestRefundItem {
    pub list_id: i32,
    /// `None`: Java's bad list (see the module notes).
    pub items: Option<Vec<i32>>,
}

impl RequestRefundItem {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestRefundItem;
    /// Bytes per item: an int.
    pub const BATCH_LENGTH: usize = 4;
}

impl Encode for RequestRefundItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.i32(self.list_id);
        write_count(w, &self.items);
        for index in self.items.iter().flatten() {
            w.i32(*index);
        }
    }
}

impl Decode for RequestRefundItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let list_id = r.i32()?;
        let items = read_batch(r, Self::BATCH_LENGTH, |r| Ok(Some(r.i32()?)))?;
        Ok(Self { list_id, items })
    }
}

// Java: gameserver/network/clientpackets/RequestBuySellUIClose.java
/// The player closed the shop window (`0xD0 0x0076`, an ex packet). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestBuySellUIClose;

impl RequestBuySellUIClose {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestBuiSellUiClose;
}

impl Encode for RequestBuySellUIClose {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
    }
}

impl Decode for RequestBuySellUIClose {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn le(parts: &[&[u8]]) -> Vec<u8> {
        parts.concat()
    }

    #[test]
    fn buy_item_layout() {
        let packet = RequestBuyItem {
            list_id: 382,
            items: Some(vec![
                BuyItemEntry {
                    item_id: 1835,
                    count: 100,
                },
                BuyItemEntry {
                    item_id: 2509,
                    count: 3_000_000_000,
                },
            ]),
        };
        let bytes = packet.to_bytes();
        let expected = le(&[
            &[0x40],
            &382_i32.to_le_bytes(),
            &2_i32.to_le_bytes(),
            &1835_i32.to_le_bytes(),
            &100_i64.to_le_bytes(),
            &2509_i32.to_le_bytes(),
            &3_000_000_000_i64.to_le_bytes(),
        ]);
        assert_eq!(bytes, expected);
        assert_eq!(
            RequestBuyItem::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn sell_item_layout() {
        let packet = RequestSellItem {
            list_id: 7,
            items: Some(vec![SellItemEntry {
                object_id: 0x1000_0005,
                item_id: 57,
                count: 2,
            }]),
        };
        let bytes = packet.to_bytes();
        let expected = le(&[
            &[0x37],
            &7_i32.to_le_bytes(),
            &1_i32.to_le_bytes(),
            &0x1000_0005_i32.to_le_bytes(),
            &57_i32.to_le_bytes(),
            &2_i64.to_le_bytes(),
        ]);
        assert_eq!(bytes, expected);
        assert_eq!(
            RequestSellItem::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
    }

    #[test]
    fn refund_item_layout() {
        let packet = RequestRefundItem {
            list_id: 7,
            items: Some(vec![0, 2]),
        };
        let bytes = packet.to_bytes();
        let expected = le(&[
            &[0xD0, 0x75, 0x00],
            &7_i32.to_le_bytes(),
            &2_i32.to_le_bytes(),
            &0_i32.to_le_bytes(),
            &2_i32.to_le_bytes(),
        ]);
        assert_eq!(bytes, expected);
        assert_eq!(
            RequestRefundItem::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            packet
        );
    }

    #[test]
    fn ui_close_is_only_the_ex_id() {
        let bytes = RequestBuySellUIClose.to_bytes();
        assert_eq!(bytes, [0xD0, 0x76, 0x00]);
        assert_eq!(
            RequestBuySellUIClose::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            RequestBuySellUIClose
        );
    }

    /// Java's `readImpl` leaves the list `null` for these; the handler answers `ActionFailed`.
    #[test]
    fn bad_lists_are_none() {
        // Count 0.
        let zero = le(&[&1_i32.to_le_bytes(), &0_i32.to_le_bytes()]);
        assert_eq!(
            RequestBuyItem::decode(&mut PacketReader::new(&zero))
                .unwrap()
                .items,
            None
        );
        // The count says 2 but there is one entry.
        let short = le(&[
            &1_i32.to_le_bytes(),
            &2_i32.to_le_bytes(),
            &1835_i32.to_le_bytes(),
            &1_i64.to_le_bytes(),
        ]);
        assert_eq!(
            RequestBuyItem::decode(&mut PacketReader::new(&short))
                .unwrap()
                .items,
            None
        );
        // An item id of 0 or a count of 0 drops the whole list.
        for (item_id, count) in [(0_i32, 1_i64), (1835, 0)] {
            let bad = le(&[
                &1_i32.to_le_bytes(),
                &1_i32.to_le_bytes(),
                &item_id.to_le_bytes(),
                &count.to_le_bytes(),
            ]);
            assert_eq!(
                RequestBuyItem::decode(&mut PacketReader::new(&bad))
                    .unwrap()
                    .items,
                None
            );
        }
        // A sell entry with object id 0.
        let bad_sell = le(&[
            &1_i32.to_le_bytes(),
            &1_i32.to_le_bytes(),
            &0_i32.to_le_bytes(),
            &57_i32.to_le_bytes(),
            &1_i64.to_le_bytes(),
        ]);
        assert_eq!(
            RequestSellItem::decode(&mut PacketReader::new(&bad_sell))
                .unwrap()
                .items,
            None
        );
        // A refund list with trailing bytes.
        let extra = le(&[
            &1_i32.to_le_bytes(),
            &1_i32.to_le_bytes(),
            &0_i32.to_le_bytes(),
            &[9],
        ]);
        assert_eq!(
            RequestRefundItem::decode(&mut PacketReader::new(&extra))
                .unwrap()
                .items,
            None
        );
        // A bad list encodes to the count 0 and decodes to the same bad list.
        let bad = RequestBuyItem {
            list_id: 3,
            items: None,
        };
        let bytes = bad.to_bytes();
        assert_eq!(
            RequestBuyItem::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            bad
        );
    }
}
