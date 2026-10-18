//! Client packets of clicking and talking: `Action`, `RequestTargetCanceld`,
//! `RequestBypassToServer` and `RequestLinkHtml`.
//!
//! These only read the packet. What a click or a bypass command does is the world's job (the
//! handlers of M3-TALK-01): the bypass text stays raw, unparsed.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::write_opcode;
use crate::generated::opcodes::ClientOpcode;

// Java: gameserver/network/clientpackets/Action.java
/// A click on an object: a target, an NPC, a door, an item on the ground.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Action {
    pub object_id: i32,
    /// The client's position at the click. Java reads them and never uses them.
    pub origin_x: i32,
    pub origin_y: i32,
    pub origin_z: i32,
    /// [`Self::SIMPLE_CLICK`] or [`Self::SHIFT_CLICK`]. Java reads a signed byte, so any other
    /// value reaches Java's "invalid action" branch.
    pub action_id: i8,
}

impl Action {
    pub const OPCODE: ClientOpcode = ClientOpcode::Action;
    pub const SIMPLE_CLICK: i8 = 0;
    pub const SHIFT_CLICK: i8 = 1;
}

impl Encode for Action {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.object_id)
            .i32(self.origin_x)
            .i32(self.origin_y)
            .i32(self.origin_z)
            .i8(self.action_id);
    }
}

impl Decode for Action {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let object_id = r.i32()?;
        let origin_x = r.i32()?;
        let origin_y = r.i32()?;
        let origin_z = r.i32()?;
        let action_id = r.i8()?;
        Ok(Self {
            object_id,
            origin_x,
            origin_y,
            origin_z,
            action_id,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestTargetCanceld.java
/// The client drops its target (Escape key or the "cancel target" button).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RequestTargetCanceld {
    /// Java reads a signed short. `0` is the Escape key, which aborts a cast first when the
    /// player is casting. Any other value only clears the target.
    pub unselect: i16,
}

impl RequestTargetCanceld {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestTargetCanceld;
}

impl Encode for RequestTargetCanceld {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i16(self.unselect);
    }
}

impl Decode for RequestTargetCanceld {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { unselect: r.i16()? })
    }
}

// Java: gameserver/network/clientpackets/RequestBypassToServer.java
/// A bypass command from an html link, e.g. `npc_%objectId%_Chat 0` or `admin_...`.
///
/// `command` is the raw text. The world parses it. Java's rules the world has to keep:
/// NOTE(java-quirk): an empty command makes Java disconnect the player (it is not just ignored).
/// Commands that do not start with one of the non-html prefixes (`item_`, `_bbs`, `bbs`,
/// `_mail`, `_friend`, `_match`, `_diary`, `_olympiad?command`, `manor_menu_select`, `report`)
/// must match html the player was sent (Java `validateHtmlAction`), or they are dropped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestBypassToServer {
    pub command: String,
}

impl RequestBypassToServer {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestBypassToServer;
}

impl Encode for RequestBypassToServer {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.command);
    }
}

impl Decode for RequestBypassToServer {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { command: r.string() })
    }
}

// Java: gameserver/network/clientpackets/RequestLinkHtml.java
/// A link to another html page of the same dialog, e.g. `teleports/gatekeeper/x.htm`.
///
/// `link` is the raw path. Java rejects an empty link and any link with `..` in it, and checks
/// the link against the html the player was sent. Those checks are the world's job.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestLinkHtml {
    pub link: String,
}

impl RequestLinkHtml {
    pub const OPCODE: ClientOpcode = ClientOpcode::RequestLinkHtml;
}

impl Encode for RequestLinkHtml {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.link);
    }
}

impl Decode for RequestLinkHtml {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { link: r.string() })
    }
}
