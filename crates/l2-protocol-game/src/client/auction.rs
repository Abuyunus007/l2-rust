//! Client packets of the item auction (`ItemAuctionManager`): the bid and the info request.
//!
//! These only read the packet. The flood check, the bid range and the auction state are the
//! world's job.

// Java: gameserver/network/clientpackets/RequestBidItemAuction.java
client_packet!(
    /// The player bids on the running auction of an auction instance (`0xD0` 0x0039,
    /// `REQUEST_BID_ITEM_AUCTION`).
    RequestBidItemAuction,
    ex RequestBidItemAuction {
        /// The auction instance id.
        instance_id: i32,
        /// The bid in adena.
        bid: i64,
    }
);

// Java: gameserver/network/clientpackets/RequestInfoItemAuction.java
client_packet!(
    /// Asks for the running and the next auction of an instance (`0xD0` 0x003A,
    /// `REQUEST_INFO_ITEM_AUCTION`).
    RequestInfoItemAuction,
    ex RequestInfoItemAuction {
        /// The auction instance id.
        instance_id: i32,
    }
);
