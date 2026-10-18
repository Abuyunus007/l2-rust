//! Client packets of the in-game chat, logout, restart and the net ping.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;
use crate::server::chat::ChatType;

// Java: gameserver/network/clientpackets/Say2.java
/// A chat line the player typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Say2 {
    pub text: String,
    /// The channel as the client sent it. Kept as a number: Java rejects unknown channels in
    /// `runImpl`, not while reading, so use [`Self::chat_type`] to look it up.
    pub chat_type: i32,
    /// The receiver's name. Only on the wire for whispers, so `None` for every other channel.
    pub target: Option<String>,
}

impl Say2 {
    pub const OPCODE: ClientOpcode = ClientOpcode::Say2;

    /// The channel, or `None` when Java would reject the packet.
    pub fn chat_type(&self) -> Option<ChatType> {
        ChatType::from_client_id(self.chat_type)
    }
}

impl Encode for Say2 {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.text).i32(self.chat_type);
        if self.chat_type == ChatType::Whisper.client_id() {
            w.string(self.target.as_deref().unwrap_or(""));
        }
    }
}

impl Decode for Say2 {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let text = r.string();
        let chat_type = r.i32()?;
        // Java: `_target = (_type == WHISPER) ? readString() : null`.
        let target = if chat_type == ChatType::Whisper.client_id() {
            Some(r.string())
        } else {
            None
        };
        Ok(Self {
            text,
            chat_type,
            target,
        })
    }
}

// Java: gameserver/network/clientpackets/Logout.java
/// "Exit game" button. No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Logout;

impl Logout {
    pub const OPCODE: ClientOpcode = ClientOpcode::Logout;
}

impl Encode for Logout {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for Logout {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/clientpackets/RequestRestart.java
/// "Back to character selection" button. No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestRestart;

impl RequestRestart {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestRestart;
}

impl Encode for RequestRestart {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for RequestRestart {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/clientpackets/RequestNetPing.java
/// The client's periodic net ping (opcode `NET_PING`). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestNetPing;

impl RequestNetPing {
    pub const OPCODE: ClientOpcode = ClientOpcode::NetPing;
}

impl Encode for RequestNetPing {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for RequestNetPing {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/clientpackets/RequestSkillCoolTime.java
/// Asks for the skills that are still in their reuse delay. No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestSkillCoolTime;

impl RequestSkillCoolTime {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestSkillCoolTime;
}

impl Encode for RequestSkillCoolTime {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for RequestSkillCoolTime {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}
