//! Client packets of the player to player trade window.
//!
//! These only read the packet. Every check (range, store mode, olympiad, block list, flood
//! protection ...) and the item moves are the world's job (M6 trade cards).

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/TradeRequest.java
/// The player asks another player to trade (`TRADE_REQUEST`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeRequest {
    /// The object id of the other player.
    pub object_id: i32,
}

impl TradeRequest {
    pub const OPCODE: ClientOpcode = ClientOpcode::TradeRequest;
}

impl Encode for TradeRequest {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id);
    }
}

impl Decode for TradeRequest {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { object_id: r.i32()? })
    }
}

// Java: gameserver/network/clientpackets/AnswerTradeRequest.java
/// The asked player answers (`ANSWER_TRADE_REQUEST`). Java accepts only `1`; any other value
/// counts as a refusal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AnswerTradeRequest {
    pub response: i32,
}

impl AnswerTradeRequest {
    pub const OPCODE: ClientOpcode = ClientOpcode::AnswerTradeRequest;
}

impl Encode for AnswerTradeRequest {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.response);
    }
}

impl Decode for AnswerTradeRequest {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { response: r.i32()? })
    }
}

// Java: gameserver/network/clientpackets/AddTradeItem.java
/// The player puts an item into the trade window (`ADD_TRADE_ITEM`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddTradeItem {
    /// Java `_tradeId`: only used in a warning message.
    pub trade_id: i32,
    pub object_id: i32,
    pub count: i64,
}

impl AddTradeItem {
    pub const OPCODE: ClientOpcode = ClientOpcode::AddTradeItem;
}

impl Encode for AddTradeItem {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.trade_id).i32(self.object_id).i64(self.count);
    }
}

impl Decode for AddTradeItem {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            trade_id: r.i32()?,
            object_id: r.i32()?,
            count: r.i64()?,
        })
    }
}

// Java: gameserver/network/clientpackets/TradeDone.java
/// The player confirms (`1`) or cancels (anything else) the trade (`TRADE_DONE`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TradeDone {
    pub response: i32,
}

impl TradeDone {
    pub const OPCODE: ClientOpcode = ClientOpcode::TradeDone;
}

impl Encode for TradeDone {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.response);
    }
}

impl Decode for TradeDone {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { response: r.i32()? })
    }
}
