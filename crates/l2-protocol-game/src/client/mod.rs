//! Packets the game client sends to the game server.
//!
//! One struct per packet, copied from the login crate's pattern (see `l2_net::packet`):
//! - a `// Java: path/To/Class.java` line,
//! - `OPCODE` (and `EX_OPCODE` for `0xD0` packets) taken from the generated tables, never typed
//!   by hand,
//! - `impl Decode`: the fields **after** the opcode(s), exactly what Java `readImpl` reads. A
//!   short packet gives [`PacketError::Truncated`](l2_net::PacketError); trailing bytes are
//!   ignored (the client pads packets),
//! - `impl Encode`: opcode(s) + fields, for test clients and the proxy.
//!
//! Which packets are accepted in which connection state is not decided here but by
//! [`crate::dispatch`], from the generated tables.

use l2_net::{PacketError, PacketReader, PacketWriter};

use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

/// A field type of the packets declared with [`client_packet!`]: how it is written and read.
pub(crate) trait WireField: Sized {
    fn put(&self, w: &mut PacketWriter);
    fn take(r: &mut PacketReader) -> Result<Self, PacketError>;
}

impl WireField for i32 {
    fn put(&self, w: &mut PacketWriter) {
        w.i32(*self);
    }
    fn take(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.i32()
    }
}

impl WireField for i64 {
    fn put(&self, w: &mut PacketWriter) {
        w.i64(*self);
    }
    fn take(r: &mut PacketReader) -> Result<Self, PacketError> {
        r.i64()
    }
}

/// Java `readString()`.
impl WireField for String {
    fn put(&self, w: &mut PacketWriter) {
        w.string(self);
    }
    fn take(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(r.string())
    }
}

/// Declares a client packet that is nothing but a list of ints, longs and strings in a row, which
/// is most of the clan, alliance and summon requests. Same result as writing the struct, the
/// `OPCODE` const and the `Encode`/`Decode` impls by hand (see `skills.rs`): `plain` packets use
/// `OPCODE`, `ex` packets (`0xD0` + `u16`) use `EX_OPCODE`. Put the `// Java:` line above the
/// invocation.
macro_rules! client_packet {
    ($(#[$meta:meta])* $name:ident, plain $op:ident { $($(#[$fmeta:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            $($(#[$fmeta])* pub $field: $ty,)*
        }

        impl $name {
            pub const OPCODE: $crate::generated::opcodes::ClientOpcode =
                $crate::generated::opcodes::ClientOpcode::$op;
        }

        impl l2_net::Encode for $name {
            fn encode(&self, w: &mut l2_net::PacketWriter) {
                $crate::client::write_opcode(w, Self::OPCODE);
                $($crate::client::WireField::put(&self.$field, w);)*
            }
        }

        impl l2_net::Decode for $name {
            #[allow(unused_variables)]
            fn decode(r: &mut l2_net::PacketReader) -> Result<Self, l2_net::PacketError> {
                Ok(Self {
                    $($field: <$ty as $crate::client::WireField>::take(r)?,)*
                })
            }
        }
    };
    ($(#[$meta:meta])* $name:ident, ex $op:ident { $($(#[$fmeta:meta])* $field:ident: $ty:ty),* $(,)? }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Eq)]
        pub struct $name {
            $($(#[$fmeta])* pub $field: $ty,)*
        }

        impl $name {
            pub const EX_OPCODE: $crate::generated::opcodes::ExClientOpcode =
                $crate::generated::opcodes::ExClientOpcode::$op;
        }

        impl l2_net::Encode for $name {
            fn encode(&self, w: &mut l2_net::PacketWriter) {
                $crate::client::write_ex_opcode(w, Self::EX_OPCODE);
                $($crate::client::WireField::put(&self.$field, w);)*
            }
        }

        impl l2_net::Decode for $name {
            #[allow(unused_variables)]
            fn decode(r: &mut l2_net::PacketReader) -> Result<Self, l2_net::PacketError> {
                Ok(Self {
                    $($field: <$ty as $crate::client::WireField>::take(r)?,)*
                })
            }
        }
    };
}

/// Like [`client_packet!`] for a packet without fields (a unit struct).
macro_rules! client_unit_packet {
    ($(#[$meta:meta])* $name:ident, plain $op:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            pub const OPCODE: $crate::generated::opcodes::ClientOpcode =
                $crate::generated::opcodes::ClientOpcode::$op;
        }

        impl l2_net::Encode for $name {
            fn encode(&self, w: &mut l2_net::PacketWriter) {
                $crate::client::write_opcode(w, Self::OPCODE);
            }
        }

        impl l2_net::Decode for $name {
            fn decode(_r: &mut l2_net::PacketReader) -> Result<Self, l2_net::PacketError> {
                Ok(Self)
            }
        }
    };
    ($(#[$meta:meta])* $name:ident, ex $op:ident) => {
        $(#[$meta])*
        #[derive(Debug, Clone, Copy, PartialEq, Eq)]
        pub struct $name;

        impl $name {
            pub const EX_OPCODE: $crate::generated::opcodes::ExClientOpcode =
                $crate::generated::opcodes::ExClientOpcode::$op;
        }

        impl l2_net::Encode for $name {
            fn encode(&self, w: &mut l2_net::PacketWriter) {
                $crate::client::write_ex_opcode(w, Self::EX_OPCODE);
            }
        }

        impl l2_net::Decode for $name {
            fn decode(_r: &mut l2_net::PacketReader) -> Result<Self, l2_net::PacketError> {
                Ok(Self)
            }
        }
    };
}

pub mod charscreen;
pub mod combat;
pub mod enterworld;
pub mod handshake;
pub mod ingame;
pub mod interaction;
pub mod item;
pub mod movement;
pub mod party;
pub mod pledge;
pub mod shortcut;
pub mod skills;
pub mod store;
pub mod summon;
pub mod trade;

pub use charscreen::{
    CharacterCreate, CharacterDelete, CharacterRestore, CharacterSelect, NewCharacter, RequestGotoLobby,
};
pub use combat::{AttackRequest, RequestActionUse, RequestRestartPoint};
pub use enterworld::{EnterWorld, RequestKeyMapping};
pub use handshake::{AuthLogin, ProtocolVersion};
pub use ingame::{Logout, RequestNetPing, RequestRestart, RequestSkillCoolTime, Say2};
pub use interaction::{
    Action, BypassUserCmd, DlgAnswer, RequestBypassToServer, RequestLinkHtml, RequestTargetCanceld,
    SendBypassBuildCmd,
};
pub use item::{RequestDestroyItem, RequestDropItem, RequestItemList, RequestUnEquipItem, UseItem};
pub use movement::{Appearing, CannotMoveAnymore, MoveToLocation, ValidatePosition};
pub use party::{
    RequestAnswerJoinParty, RequestChangePartyLeader, RequestJoinParty, RequestOustPartyMember,
    RequestWithDrawalParty,
};
pub use pledge::{
    AllyDismiss, AllyLeave, RequestAllyCrest, RequestAllyInfo, RequestAnswerJoinAlly,
    RequestAnswerJoinPledge, RequestDismissAlly, RequestExPledgeCrestLarge, RequestExSetPledgeCrestLarge,
    RequestJoinAlly, RequestJoinPledge, RequestOustPledgeMember, RequestPledgeCrest,
    RequestPledgeExtendedInfo, RequestPledgeInfo, RequestPledgeMemberInfo, RequestPledgeMemberList,
    RequestPledgeMemberPowerInfo, RequestPledgePower, RequestPledgePowerGradeList,
    RequestPledgeReorganizeMember, RequestPledgeSetAcademyMaster, RequestPledgeSetMemberPowerGrade,
    RequestPledgeWarList, RequestReplyStartPledgeWar, RequestReplyStopPledgeWar,
    RequestReplySurrenderPledgeWar, RequestSetAllyCrest, RequestSetPledgeCrest, RequestStartPledgeWar,
    RequestStopPledgeWar, RequestSurrenderPledgeWar, RequestWithdrawalPledge,
};
pub use shortcut::{RequestShortCutDel, RequestShortCutReg};
pub use skills::{
    RequestAcquireSkill, RequestAcquireSkillInfo, RequestAutoSoulShot, RequestDispel, RequestMagicSkillUse,
};
pub use store::{
    BuyListEntry, RequestPrivateStoreBuy, RequestPrivateStoreManageBuy, RequestPrivateStoreManageSell,
    RequestPrivateStoreQuitBuy, RequestPrivateStoreQuitSell, RequestPrivateStoreSell, SellListEntry,
    SetPrivateStoreListBuy, SetPrivateStoreListSell, SetPrivateStoreMsgBuy, SetPrivateStoreMsgSell,
    SetPrivateStoreWholeMsg, StoreBuyRequestEntry, StoreSellRequestEntry,
};
pub use summon::{
    RequestChangePetName, RequestGetItemFromPet, RequestGiveItemToPet, RequestPetGetItem, RequestPetUseItem,
};
pub use trade::{AddTradeItem, AnswerTradeRequest, TradeDone, TradeRequest};

/// Writes the id byte of a plain client packet.
pub(crate) fn write_opcode(w: &mut PacketWriter, opcode: ClientOpcode) {
    w.u8(opcode.id());
}

/// Writes `0xD0` followed by the `u16` ex id of an ex client packet.
pub(crate) fn write_ex_opcode(w: &mut PacketWriter, ex: ExClientOpcode) {
    w.u8(ClientOpcode::ExPacket.id()).u16(ex.id());
}
