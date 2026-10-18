//! Server packets of the chat, logout and restart flow, plus the `ChatType` enum both directions
//! use.
//!
//! Same pattern as [`super::charscreen`]: a struct per packet, `OPCODE` from the generated
//! [`ServerOpcode`] table, `impl Encode` for the server and `impl Decode` for test clients.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ServerOpcode;

// Java: gameserver/network/enums/ChatType.java
/// Chat channels. The discriminant is the id the client uses on the wire (Java `clientId`).
#[repr(i32)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChatType {
    General = 0,
    Shout = 1,
    Whisper = 2,
    Party = 3,
    Clan = 4,
    Gm = 5,
    PetitionPlayer = 6,
    PetitionGm = 7,
    Trade = 8,
    Alliance = 9,
    Announcement = 10,
    Boat = 11,
    Friend = 12,
    MsnChat = 13,
    PartyMatchRoom = 14,
    PartyRoomCommander = 15,
    PartyRoomAll = 16,
    HeroVoice = 17,
    CriticalAnnounce = 18,
    ScreenAnnounce = 19,
    Battlefield = 20,
    MpccRoom = 21,
    NpcGeneral = 22,
    NpcShout = 23,
}

impl ChatType {
    /// Every channel, in Java declaration order.
    pub const ALL: [ChatType; 24] = [
        Self::General,
        Self::Shout,
        Self::Whisper,
        Self::Party,
        Self::Clan,
        Self::Gm,
        Self::PetitionPlayer,
        Self::PetitionGm,
        Self::Trade,
        Self::Alliance,
        Self::Announcement,
        Self::Boat,
        Self::Friend,
        Self::MsnChat,
        Self::PartyMatchRoom,
        Self::PartyRoomCommander,
        Self::PartyRoomAll,
        Self::HeroVoice,
        Self::CriticalAnnounce,
        Self::ScreenAnnounce,
        Self::Battlefield,
        Self::MpccRoom,
        Self::NpcGeneral,
        Self::NpcShout,
    ];

    /// The id the client uses (Java `getClientId()`).
    pub fn client_id(self) -> i32 {
        self as i32
    }

    /// Java `ChatType.findByClientId`: `None` for an id Java does not know.
    pub fn from_client_id(id: i32) -> Option<Self> {
        Self::ALL.into_iter().find(|t| t.client_id() == id)
    }
}

// Java: gameserver/network/serverpackets/CreatureSay.java
/// Who a [`CreatureSay`] is from. Java writes the sender's name when it has one, otherwise the
/// character id (system messages have no sender name).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SenderField {
    Name(String),
    CharId(i32),
}

/// What a [`CreatureSay`] says. Java writes the text when there is one, otherwise the NPC string
/// parameters (possibly none).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SayBody {
    Text(String),
    Parameters(Vec<String>),
}

/// A chat line or a system message shown in the chat window (opcode name `SAY2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatureSay {
    /// The sender's object id. Java writes `0` when there is no sender object.
    pub sender_object_id: i32,
    pub chat_type: ChatType,
    pub sender: SenderField,
    /// The High Five NPCString or system message id. Java uses `-1` for a plain text line.
    pub message_id: i32,
    pub body: SayBody,
}

impl CreatureSay {
    pub const OPCODE: ServerOpcode = ServerOpcode::Say2;
}

impl Encode for CreatureSay {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.sender_object_id).i32(self.chat_type.client_id());
        match &self.sender {
            SenderField::Name(name) => {
                w.string(name);
            }
            SenderField::CharId(id) => {
                w.i32(*id);
            }
        }
        w.i32(self.message_id);
        match &self.body {
            SayBody::Text(text) => {
                w.string(text);
            }
            SayBody::Parameters(parameters) => {
                for p in parameters {
                    w.string(p);
                }
            }
        }
    }
}

// No `Decode` for `CreatureSay`: the bytes do not say whether the field after the chat type is a
// name or a char id, and the body has no length, so a decoder cannot tell a text from a list of
// parameters. Only the encoder is needed by the server.

// Java: gameserver/network/serverpackets/LeaveWorld.java
/// "Log out OK": the client leaves the world and shows the character list. No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LeaveWorld;

impl LeaveWorld {
    pub const OPCODE: ServerOpcode = ServerOpcode::LogOutOk;
}

impl Encode for LeaveWorld {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for LeaveWorld {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/RestartResponse.java
/// Answer to `RequestRestart`: `true` when the client may go back to the character list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RestartResponse {
    pub result: bool,
}

impl RestartResponse {
    pub const OPCODE: ServerOpcode = ServerOpcode::RestartResponse;
}

impl Encode for RestartResponse {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // Java: `writeInt(boolean)`, 1 or 0.
        w.i32(i32::from(self.result));
    }
}

impl Decode for RestartResponse {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            result: r.i32()? != 0,
        })
    }
}

// Java: gameserver/network/serverpackets/NetPing.java
/// Answer to `RequestNetPing`: the game time in the client's ping measurement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NetPing {
    /// Java `GameTimeTaskManager.getGameTime()`, prepared by the caller.
    pub game_time: i32,
}

impl NetPing {
    pub const OPCODE: ServerOpcode = ServerOpcode::NetPing;
}

impl Encode for NetPing {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.game_time);
    }
}

impl Decode for NetPing {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { game_time: r.i32()? })
    }
}
