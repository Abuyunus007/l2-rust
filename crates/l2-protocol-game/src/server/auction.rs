//! Server packets of the item auction (`ItemAuctionManager`): the running auction and the next one.
//!
//! Same rules as [`super::party`]: values only. The auction state, the highest bid and the time
//! left are the world's; this packet writes them as given.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::items::{ItemInfo, read_item, write_item};
use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

/// The next auction of an instance, written after the running one when there is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemAuctionNext {
    /// Java `getAuctionInitBid()`.
    pub init_bid: i64,
    /// Java `getStartingTime()` in unix seconds.
    pub start_secs: i32,
    /// The item of the next auction.
    pub item: ItemInfo,
}

// Java: gameserver/network/serverpackets/ExItemAuctionInfoPacket.java
/// The running item auction of an instance, and the next one (`EX_ITEM_AUCTION_INFO`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExItemAuctionInfo {
    /// Java `refresh`: the packet is a refresh of a window that is open. Written as the inverse
    /// byte (Java `writeByte(!_refresh)`).
    pub refresh: bool,
    /// The auction instance id.
    pub instance_id: i32,
    /// Java: the highest bid, or the initial bid when nobody bid yet.
    pub bid: i64,
    /// Seconds left in the running auction (0 unless it is started).
    pub time_remaining: i32,
    /// The item of the running auction.
    pub item: ItemInfo,
    /// The next auction, when there is one (the packet ends after the item without it).
    pub next: Option<ItemAuctionNext>,
}

impl ExItemAuctionInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExItemAuctionInfo;
}

impl Encode for ExItemAuctionInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.bool(!self.refresh)
            .i32(self.instance_id)
            .i64(self.bid)
            .i32(self.time_remaining);
        write_item(w, &self.item);
        if let Some(next) = &self.next {
            w.i64(next.init_bid).i32(next.start_secs);
            write_item(w, &next.item);
        }
    }
}

impl Decode for ExItemAuctionInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let refresh = !r.bool()?;
        let instance_id = r.i32()?;
        let bid = r.i64()?;
        let time_remaining = r.i32()?;
        let item = read_item(r)?;
        // The next auction is optional: the packet simply ends without it.
        let next = if r.remaining() > 0 {
            Some(ItemAuctionNext {
                init_bid: r.i64()?,
                start_secs: r.i32()?,
                item: read_item(r)?,
            })
        } else {
            None
        };
        Ok(Self {
            refresh,
            instance_id,
            bid,
            time_remaining,
            item,
            next,
        })
    }
}
