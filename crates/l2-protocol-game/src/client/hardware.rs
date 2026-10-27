//! Client packets the client sends on its own about itself: the GameGuard answer, the hardware
//! report and the PC cafe coupon (L-04). Java reads them and keeps (or logs) the values.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::{write_ex_opcode, write_opcode};
use crate::generated::opcodes::{ClientOpcode, ExClientOpcode};

// Java: gameserver/network/clientpackets/GameGuardReply.java
/// The answer to `GameGuardQuery` (`0xCB`). Java hashes the eight answer bytes with SHA-1 and
/// compares them with a fixed value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GameGuardReply {
    /// Java `_reply[0..4]`, then (after the skipped int) `_reply[4..8]`.
    pub reply: [u8; 8],
    /// The int between the two halves, which Java reads and drops.
    pub skipped: i32,
}

impl GameGuardReply {
    pub const OPCODE: ClientOpcode = ClientOpcode::GameGuardReply;
}

impl Encode for GameGuardReply {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.bytes(&self.reply[..4])
            .i32(self.skipped)
            .bytes(&self.reply[4..]);
    }
}

impl Decode for GameGuardReply {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let mut reply = [0u8; 8];
        reply[..4].copy_from_slice(r.bytes(4)?);
        let skipped = r.i32()?;
        reply[4..].copy_from_slice(r.bytes(4)?);
        Ok(Self { reply, skipped })
    }
}

// Java: gameserver/network/clientpackets/RequestHardWareInfo.java
/// The hardware report (`0xD0` 0x0096, accepted in every connection state). Java keeps it in the
/// client (`ClientHardwareInfoHolder`); the MAC address is what the punishment manager matches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestHardWareInfo {
    pub mac_address: String,
    pub windows_platform_id: i32,
    pub windows_major_version: i32,
    pub windows_minor_version: i32,
    pub windows_build_number: i32,
    pub directx_version: i32,
    pub directx_revision: i32,
    /// 16 bytes Java skips (`readBytes(16)`).
    pub unknown1: [u8; 16],
    pub cpu_name: String,
    pub cpu_speed: i32,
    /// Java `readByte()`.
    pub cpu_core_count: i8,
    /// The int Java skips after the core count.
    pub unknown2: i32,
    pub vga_count: i32,
    pub vga_pcx_speed: i32,
    pub phys_memory_slot1: i32,
    pub phys_memory_slot2: i32,
    pub phys_memory_slot3: i32,
    /// The byte Java skips before the video memory.
    pub unknown3: u8,
    pub video_memory: i32,
    /// The int Java skips after the video memory.
    pub unknown4: i32,
    /// Java `readShort()`.
    pub vga_version: i16,
    pub vga_name: String,
    pub vga_driver_version: String,
}

impl RequestHardWareInfo {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestHardwareInfo;
}

impl Encode for RequestHardWareInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.string(&self.mac_address)
            .i32(self.windows_platform_id)
            .i32(self.windows_major_version)
            .i32(self.windows_minor_version)
            .i32(self.windows_build_number)
            .i32(self.directx_version)
            .i32(self.directx_revision)
            .bytes(&self.unknown1)
            .string(&self.cpu_name)
            .i32(self.cpu_speed)
            .i8(self.cpu_core_count)
            .i32(self.unknown2)
            .i32(self.vga_count)
            .i32(self.vga_pcx_speed)
            .i32(self.phys_memory_slot1)
            .i32(self.phys_memory_slot2)
            .i32(self.phys_memory_slot3)
            .u8(self.unknown3)
            .i32(self.video_memory)
            .i32(self.unknown4)
            .i16(self.vga_version)
            .string(&self.vga_name)
            .string(&self.vga_driver_version);
    }
}

impl Decode for RequestHardWareInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let mac_address = r.string();
        let windows_platform_id = r.i32()?;
        let windows_major_version = r.i32()?;
        let windows_minor_version = r.i32()?;
        let windows_build_number = r.i32()?;
        let directx_version = r.i32()?;
        let directx_revision = r.i32()?;
        let mut unknown1 = [0u8; 16];
        unknown1.copy_from_slice(r.bytes(16)?);
        let cpu_name = r.string();
        let cpu_speed = r.i32()?;
        let cpu_core_count = r.i8()?;
        let unknown2 = r.i32()?;
        let vga_count = r.i32()?;
        let vga_pcx_speed = r.i32()?;
        let phys_memory_slot1 = r.i32()?;
        let phys_memory_slot2 = r.i32()?;
        let phys_memory_slot3 = r.i32()?;
        let unknown3 = r.u8()?;
        let video_memory = r.i32()?;
        let unknown4 = r.i32()?;
        let vga_version = r.i16()?;
        let vga_name = r.string();
        let vga_driver_version = r.string();
        Ok(Self {
            mac_address,
            windows_platform_id,
            windows_major_version,
            windows_minor_version,
            windows_build_number,
            directx_version,
            directx_revision,
            unknown1,
            cpu_name,
            cpu_speed,
            cpu_core_count,
            unknown2,
            vga_count,
            vga_pcx_speed,
            phys_memory_slot1,
            phys_memory_slot2,
            phys_memory_slot3,
            unknown3,
            video_memory,
            unknown4,
            vga_version,
            vga_name,
            vga_driver_version,
        })
    }
}

// Java: gameserver/network/clientpackets/RequestPCCafeCouponUse.java
/// A coupon code typed into the PC cafe window (`0xD0` 0x0019). Java only logs it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequestPCCafeCouponUse {
    pub coupon: String,
}

impl RequestPCCafeCouponUse {
    pub const EX_OPCODE: ExClientOpcode = ExClientOpcode::RequestPccafeCouponUse;
}

impl Encode for RequestPCCafeCouponUse {
    fn encode(&self, w: &mut PacketWriter) {
        write_ex_opcode(w, Self::EX_OPCODE);
        w.string(&self.coupon);
    }
}

impl Decode for RequestPCCafeCouponUse {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self { coupon: r.string() })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> RequestHardWareInfo {
        RequestHardWareInfo {
            mac_address: "00-00-00-00-00-01".into(),
            windows_platform_id: 2,
            windows_major_version: 10,
            windows_minor_version: 0,
            windows_build_number: 19045,
            directx_version: 12,
            directx_revision: 1,
            unknown1: [7; 16],
            cpu_name: "Some CPU".into(),
            cpu_speed: 3600,
            cpu_core_count: 8,
            unknown2: 0,
            vga_count: 1,
            vga_pcx_speed: 16,
            phys_memory_slot1: 8192,
            phys_memory_slot2: 8192,
            phys_memory_slot3: 0,
            unknown3: 0,
            video_memory: 4096,
            unknown4: 0,
            vga_version: 12,
            vga_name: "Some GPU".into(),
            vga_driver_version: "31.0.1".into(),
        }
    }

    #[test]
    fn the_hardware_report_round_trips() {
        let packet = sample();
        let bytes = packet.to_bytes();
        assert_eq!(bytes[..3], [0xD0, 0x96, 0x00]);
        assert_eq!(
            RequestHardWareInfo::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            packet
        );
        assert!(RequestHardWareInfo::decode(&mut PacketReader::new(&bytes[3..30])).is_err());
    }

    #[test]
    fn the_game_guard_reply_skips_an_int_between_the_halves() {
        let packet = GameGuardReply {
            reply: [1, 2, 3, 4, 5, 6, 7, 8],
            skipped: 0x0A0B_0C0D,
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes, [0xCB, 1, 2, 3, 4, 0x0D, 0x0C, 0x0B, 0x0A, 5, 6, 7, 8]);
        assert_eq!(
            GameGuardReply::decode(&mut PacketReader::new(&bytes[1..])).unwrap(),
            packet
        );
        assert!(GameGuardReply::decode(&mut PacketReader::new(&bytes[1..8])).is_err());
    }

    #[test]
    fn the_coupon_round_trips() {
        let packet = RequestPCCafeCouponUse {
            coupon: "ABCD-1234".into(),
        };
        let bytes = packet.to_bytes();
        assert_eq!(bytes[..3], [0xD0, 0x19, 0x00]);
        assert_eq!(
            RequestPCCafeCouponUse::decode(&mut PacketReader::new(&bytes[3..])).unwrap(),
            packet
        );
    }
}
