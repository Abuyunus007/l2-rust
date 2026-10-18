//! Packets the game server sends to the game client.
//!
//! Same pattern as the client packets (see [`crate::client`]): a struct per packet, `OPCODE` from
//! the generated [`ServerOpcode`] table, `impl Encode` (opcode + fields, Java `writeImpl`) and
//! `impl Decode` (fields after the opcode, for the proxy and test clients). Data that Java reads
//! from the database or from game objects is passed in as plain values: no logic in here.

use l2_net::PacketWriter;

use crate::generated::opcodes::ServerOpcode;

pub mod charscreen;
pub mod handshake;
pub mod items;
pub mod userinfo;

pub use charscreen::{
    CharCreateFail, CharCreateOk, CharDeleteFail, CharDeleteSuccess, CharSelectEntry, CharSelected,
    CharSelectionInfo, NewCharTemplate, NewCharacterSuccess, ServerClose, active_slot, delete_seconds,
    exp_percent,
};
pub use handshake::KeyPacket;
pub use items::{
    InventoryBlock, ItemInfo, ItemList, ItemUpdate, read_inventory_block, read_item, read_item_updates,
    write_inventory_block, write_item, write_item_elemental_and_enchant, write_item_updates,
};
pub use userinfo::UserInfo;

/// Writes a server packet's id the way Java `ServerPackets.writeId` does: the id byte, then the
/// `u16` ex id for ex packets (id byte `0xFE`).
///
/// NOTE(java-quirk): Java writes the ex id only when it is greater than 0, so `ExDummy`
/// (`0xFE`, ex id 0) is the single byte `0xFE`.
pub fn write_opcode(w: &mut PacketWriter, opcode: ServerOpcode) {
    w.u8(opcode.id());
    match opcode.ex_id() {
        Some(ex) if ex > 0 => {
            w.u16(ex);
        }
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn opcode_bytes(opcode: ServerOpcode) -> Vec<u8> {
        let mut w = PacketWriter::new();
        write_opcode(&mut w, opcode);
        w.into_bytes()
    }

    #[test]
    fn plain_and_ex_ids() {
        assert_eq!(opcode_bytes(ServerOpcode::VersionCheck), [0x2E]);
        assert_eq!(opcode_bytes(ServerOpcode::ServerClose), [0x20]);
        // 0xFE + u16 ex id, little endian: EX_RESTART_CLIENT is 0x48.
        assert_eq!(opcode_bytes(ServerOpcode::ExRestartClient), [0xFE, 0x48, 0x00]);
    }

    #[test]
    fn ex_dummy_is_one_byte() {
        assert_eq!(ServerOpcode::ExDummy.ex_id(), Some(0));
        assert_eq!(opcode_bytes(ServerOpcode::ExDummy), [0xFE]);
    }

    #[test]
    fn every_opcode_writes_one_or_three_bytes() {
        for op in ServerOpcode::ALL {
            let bytes = opcode_bytes(op);
            let expected = if op.ex_id().is_some_and(|ex| ex > 0) { 3 } else { 1 };
            assert_eq!(bytes.len(), expected, "{}", op.name());
            assert_eq!(bytes[0], op.id());
        }
    }
}
