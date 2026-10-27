//! Server packets of the party matching rooms and of the loot rule vote (L-03).
//!
//! Values only, like [`super::party`]: the world decides who is in a room and builds the records.

use l2_net::{Decode, Encode, PacketError, PacketReader, PacketWriter};

use super::party::PartyDistributionType;
use super::{read_count, write_opcode};
use crate::generated::opcodes::ServerOpcode;

/// What Java writes at the end of `PartyMatchDetail`: `writeShort(59064)`, "?" for the client.
const PARTY_ROOM_INFO_TAIL: u16 = 59064;

// Java: gameserver/network/serverpackets/PartyMatchDetail.java
/// The window of one room (`PARTY_ROOM_INFO`, 0x9D).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartyRoomInfo {
    /// `getId()`.
    pub room_id: i32,
    /// `getMaxMembers()`.
    pub max_members: i32,
    /// `getMinLevel()`.
    pub min_level: i32,
    /// `getMaxLevel()`.
    pub max_level: i32,
    /// `getLootType()`.
    pub loot_type: i32,
    /// `getLocation()`: the bbs id of the owner's map region.
    pub location: i32,
    /// `getTitle()`.
    pub title: String,
}

impl PartyRoomInfo {
    pub const OPCODE: ServerOpcode = ServerOpcode::PartyRoomInfo;
}

impl Encode for PartyRoomInfo {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.room_id)
            .i32(self.max_members)
            .i32(self.min_level)
            .i32(self.max_level)
            .i32(self.loot_type)
            .i32(self.location)
            .string(&self.title)
            .u16(PARTY_ROOM_INFO_TAIL);
    }
}

impl Decode for PartyRoomInfo {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let packet = Self {
            room_id: r.i32()?,
            max_members: r.i32()?,
            min_level: r.i32()?,
            max_level: r.i32()?,
            loot_type: r.i32()?,
            location: r.i32()?,
            title: r.string(),
        };
        let _tail = r.u16()?;
        Ok(packet)
    }
}

/// One member of a room in [`ExPartyRoomMember`] and [`ExManagePartyRoomMember`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoomMember {
    /// `getObjectId()`.
    pub object_id: i32,
    /// `getName()`.
    pub name: String,
    /// `getActiveClass()`.
    pub class_id: i32,
    /// `getLevel()`.
    pub level: i32,
    /// `room.getLocation()`.
    pub location: i32,
    /// 1 the owner, 2 in the party of the owner, 0 otherwise.
    pub role: i32,
}

impl RoomMember {
    fn write(&self, w: &mut PacketWriter) {
        w.i32(self.object_id)
            .string(&self.name)
            .i32(self.class_id)
            .i32(self.level)
            .i32(self.location)
            .i32(self.role);
    }

    fn read(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            object_id: r.i32()?,
            name: r.string(),
            class_id: r.i32()?,
            level: r.i32()?,
            location: r.i32()?,
            role: r.i32()?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExPartyRoomMember.java
/// All the members of the room (`EX_PARTY_ROOM_MEMBER`, `0xFE` 0x08). `mode` 0 = a member entered,
/// 1 = the room was created, 2 = the owner looks at its room again.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExPartyRoomMember {
    pub mode: i32,
    pub members: Vec<RoomMember>,
}

impl ExPartyRoomMember {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExPartyRoomMember;
}

impl Encode for ExPartyRoomMember {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.mode).i32(self.members.len() as i32);
        for member in &self.members {
            member.write(w);
            // "TODO: Instance datas there is more if that is not 0!"
            w.i32(0);
        }
    }
}

impl Decode for ExPartyRoomMember {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let mode = r.i32()?;
        let count = read_count(r, "room members")?;
        let mut members = Vec::new();
        for _ in 0..count {
            members.push(RoomMember::read(r)?);
            let _instances = r.i32()?;
        }
        Ok(Self { mode, members })
    }
}

// Java: gameserver/network/serverpackets/ExManagePartyRoomMember.java
/// One member added (`mode` 0), changed (1) or removed (2) (`EX_MANAGE_PARTY_ROOM_MEMBER`,
/// `0xFE` 0x0A).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExManagePartyRoomMember {
    pub mode: i32,
    pub member: RoomMember,
}

impl ExManagePartyRoomMember {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExManagePartyRoomMember;
}

impl Encode for ExManagePartyRoomMember {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.mode);
        self.member.write(w);
    }
}

impl Decode for ExManagePartyRoomMember {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            mode: r.i32()?,
            member: RoomMember::read(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExClosePartyRoom.java
/// Closes the room window (`EX_CLOSE_PARTY_ROOM`, `0xFE` 0x09). No fields.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExClosePartyRoom;

impl ExClosePartyRoom {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExClosePartyRoom;
}

impl Encode for ExClosePartyRoom {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
    }
}

impl Decode for ExClosePartyRoom {
    fn decode(_r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self)
    }
}

// Java: gameserver/network/serverpackets/ExAskJoinPartyRoom.java
/// An invitation into the room of `requestor_name` (`EX_ASK_JOIN_PARTY_ROOM`, `0xFE` 0x35).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExAskJoinPartyRoom {
    pub requestor_name: String,
}

impl ExAskJoinPartyRoom {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExAskJoinPartyRoom;
}

impl Encode for ExAskJoinPartyRoom {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.requestor_name);
    }
}

impl Decode for ExAskJoinPartyRoom {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            requestor_name: r.string(),
        })
    }
}

/// One room of [`ListPartyWaiting`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaitingRoom {
    pub room_id: i32,
    pub title: String,
    pub location: i32,
    pub min_level: i32,
    pub max_level: i32,
    pub max_members: i32,
    /// `getOwner().getName()`.
    pub owner_name: String,
    /// `(class id, name)` of each member, owner first.
    pub members: Vec<(i32, String)>,
}

// Java: gameserver/network/serverpackets/ListPartyWating.java
/// The list of rooms (`LIST_PARTY_WAITING`, 0x9C).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ListPartyWaiting {
    pub rooms: Vec<WaitingRoom>,
}

impl ListPartyWaiting {
    pub const OPCODE: ServerOpcode = ServerOpcode::ListPartyWaiting;
}

impl Encode for ListPartyWaiting {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        // 1 when there is at least one room, else 0.
        w.i32(i32::from(!self.rooms.is_empty()));
        w.i32(self.rooms.len() as i32);
        for room in &self.rooms {
            w.i32(room.room_id)
                .string(&room.title)
                .i32(room.location)
                .i32(room.min_level)
                .i32(room.max_level)
                .i32(room.max_members)
                .string(&room.owner_name)
                .i32(room.members.len() as i32);
            for (class_id, name) in &room.members {
                w.i32(*class_id).string(name);
            }
        }
    }
}

impl Decode for ListPartyWaiting {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let _any = r.i32()?;
        let count = read_count(r, "rooms")?;
        let mut rooms = Vec::new();
        for _ in 0..count {
            let room_id = r.i32()?;
            let title = r.string();
            let location = r.i32()?;
            let min_level = r.i32()?;
            let max_level = r.i32()?;
            let max_members = r.i32()?;
            let owner_name = r.string();
            let member_count = read_count(r, "room members")?;
            let mut members = Vec::new();
            for _ in 0..member_count {
                members.push((r.i32()?, r.string()));
            }
            rooms.push(WaitingRoom {
                room_id,
                title,
                location,
                min_level,
                max_level,
                max_members,
                owner_name,
                members,
            });
        }
        Ok(Self { rooms })
    }
}

// Java: gameserver/network/serverpackets/ExListPartyMatchingWaitingRoom.java
/// The players who wait for a room (`EX_LIST_PARTY_MATCHING_WAITING_ROOM`, `0xFE` 0x36). The
/// answer to mode 0 is page 0 and no players (Java writes two zero ints).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ExListPartyMatchingWaitingRoom {
    pub page: i32,
    /// `(name, active class, level)`.
    pub members: Vec<(String, i32, i32)>,
}

impl ExListPartyMatchingWaitingRoom {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExListPartyMatchingWaitingRoom;
}

impl Encode for ExListPartyMatchingWaitingRoom {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.page).i32(self.members.len() as i32);
        for (name, class_id, level) in &self.members {
            w.string(name).i32(*class_id).i32(*level);
        }
    }
}

impl Decode for ExListPartyMatchingWaitingRoom {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        let page = r.i32()?;
        let count = read_count(r, "waiting players")?;
        let mut members = Vec::new();
        for _ in 0..count {
            members.push((r.string(), r.i32()?, r.i32()?));
        }
        Ok(Self { page, members })
    }
}

// Java: gameserver/network/serverpackets/ExAskModifyPartyLooting.java
/// Asks the members to agree to a new loot rule (`EX_ASK_MODIFY_PARTY_LOOTING`, `0xFE` 0xBF).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExAskModifyPartyLooting {
    /// The leader's name.
    pub requestor: String,
    pub distribution_type: PartyDistributionType,
}

impl ExAskModifyPartyLooting {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExAskModifyPartyLooting;
}

impl Encode for ExAskModifyPartyLooting {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.string(&self.requestor).i32(self.distribution_type.id());
    }
}

impl Decode for ExAskModifyPartyLooting {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            requestor: r.string(),
            distribution_type: read_distribution(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExSetPartyLooting.java
/// The result of the vote (`EX_SET_PARTY_LOOTING`, `0xFE` 0xC0): `result` 1 = changed, 0 = not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExSetPartyLooting {
    pub result: i32,
    pub distribution_type: PartyDistributionType,
}

impl ExSetPartyLooting {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExSetPartyLooting;
}

impl Encode for ExSetPartyLooting {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.result).i32(self.distribution_type.id());
    }
}

impl Decode for ExSetPartyLooting {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            result: r.i32()?,
            distribution_type: read_distribution(r)?,
        })
    }
}

// Java: gameserver/network/serverpackets/ExRequestChangeNicknameColor.java
/// Opens the title colour window for the item `item_object_id`
/// (`EX_REQUEST_CHANGE_NICKNAME_COLOR`, `0xFE` 0x83).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ExRequestChangeNicknameColor {
    pub item_object_id: i32,
}

impl ExRequestChangeNicknameColor {
    pub const OPCODE: ServerOpcode = ServerOpcode::ExRequestChangeNicknameColor;
}

impl Encode for ExRequestChangeNicknameColor {
    fn encode(&self, w: &mut PacketWriter) {
        write_opcode(w, Self::OPCODE);
        w.i32(self.item_object_id);
    }
}

impl Decode for ExRequestChangeNicknameColor {
    fn decode(r: &mut PacketReader) -> Result<Self, PacketError> {
        Ok(Self {
            item_object_id: r.i32()?,
        })
    }
}

fn read_distribution(r: &mut PacketReader) -> Result<PartyDistributionType, PacketError> {
    let id = r.i32()?;
    PartyDistributionType::from_id(id)
        .ok_or_else(|| PacketError::invalid("party distribution type", format!("unknown type {id}")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn member(role: i32) -> RoomMember {
        RoomMember {
            object_id: 0x1000_0001,
            name: "Aa".into(),
            class_id: 10,
            level: 40,
            location: 6,
            role,
        }
    }

    fn round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: &P, head: usize) {
        let bytes = packet.to_bytes();
        let decoded = P::decode(&mut PacketReader::new(&bytes[head..])).unwrap();
        assert_eq!(&decoded, packet);
    }

    #[test]
    fn every_packet_round_trips() {
        round_trip(
            &PartyRoomInfo {
                room_id: 3,
                max_members: 9,
                min_level: 20,
                max_level: 50,
                loot_type: 1,
                location: 6,
                title: "Cruma".into(),
            },
            1,
        );
        round_trip(
            &ExPartyRoomMember {
                mode: 1,
                members: vec![member(1), member(0)],
            },
            3,
        );
        round_trip(
            &ExManagePartyRoomMember {
                mode: 2,
                member: member(2),
            },
            3,
        );
        round_trip(&ExClosePartyRoom, 3);
        round_trip(
            &ExAskJoinPartyRoom {
                requestor_name: "Leader".into(),
            },
            3,
        );
        round_trip(
            &ListPartyWaiting {
                rooms: vec![WaitingRoom {
                    room_id: 1,
                    title: "T".into(),
                    location: 2,
                    min_level: 1,
                    max_level: 85,
                    max_members: 9,
                    owner_name: "Owner".into(),
                    members: vec![(10, "Owner".into()), (11, "Second".into())],
                }],
            },
            1,
        );
        round_trip(&ListPartyWaiting::default(), 1);
        round_trip(
            &ExListPartyMatchingWaitingRoom {
                page: 1,
                members: vec![("Bob".into(), 20, 45)],
            },
            3,
        );
        round_trip(
            &ExAskModifyPartyLooting {
                requestor: "Leader".into(),
                distribution_type: PartyDistributionType::ByTurn,
            },
            3,
        );
        round_trip(
            &ExSetPartyLooting {
                result: 1,
                distribution_type: PartyDistributionType::Random,
            },
            3,
        );
        round_trip(
            &ExRequestChangeNicknameColor {
                item_object_id: 0x1000_0002,
            },
            3,
        );
    }

    /// `PartyMatchDetail.writeImpl`: six ints, the title, then the short 59064 (0xE6B8).
    #[test]
    fn party_room_info_layout() {
        let bytes = PartyRoomInfo {
            room_id: 1,
            max_members: 2,
            min_level: 3,
            max_level: 4,
            loot_type: 5,
            location: 6,
            title: "A".into(),
        }
        .to_bytes();
        assert_eq!(bytes[0], 0x9D);
        assert_eq!(&bytes[1..5], &[1, 0, 0, 0]);
        assert_eq!(&bytes[21..25], &[6, 0, 0, 0]);
        assert_eq!(&bytes[25..29], &[b'A', 0, 0, 0]);
        assert_eq!(&bytes[29..], &[0xB8, 0xE6]);
    }

    /// `ListPartyWating.writeImpl`: the "any room" int, the count, then the rooms.
    #[test]
    fn list_party_waiting_layout() {
        let empty = ListPartyWaiting::default().to_bytes();
        assert_eq!(empty, [0x9C, 0, 0, 0, 0, 0, 0, 0, 0]);
        let one = ListPartyWaiting {
            rooms: vec![WaitingRoom {
                room_id: 7,
                title: String::new(),
                location: 0,
                min_level: 0,
                max_level: 0,
                max_members: 0,
                owner_name: String::new(),
                members: vec![],
            }],
        }
        .to_bytes();
        assert_eq!(&one[1..9], &[1, 0, 0, 0, 1, 0, 0, 0]);
        assert_eq!(&one[9..13], &[7, 0, 0, 0]);
    }

    /// The ex ids: `ExPartyRoomMember` 0x08, `ExClosePartyRoom` 0x09, `ExManagePartyRoomMember` 0x0A,
    /// `ExAskJoinPartyRoom` 0x35, `ExListPartyMatchingWaitingRoom` 0x36, the loot vote 0xBF/0xC0.
    #[test]
    fn the_ex_ids_are_javas() {
        let ex = |b: Vec<u8>| (b[0], u16::from_le_bytes([b[1], b[2]]));
        assert_eq!(ex(ExClosePartyRoom.to_bytes()), (0xFE, 0x09));
        assert_eq!(
            ex(ExPartyRoomMember {
                mode: 0,
                members: vec![]
            }
            .to_bytes()),
            (0xFE, 0x08)
        );
        assert_eq!(
            ex(ExManagePartyRoomMember {
                mode: 0,
                member: member(0)
            }
            .to_bytes()),
            (0xFE, 0x0A)
        );
        assert_eq!(
            ex(ExAskJoinPartyRoom {
                requestor_name: String::new()
            }
            .to_bytes()),
            (0xFE, 0x35)
        );
        assert_eq!(
            ex(ExListPartyMatchingWaitingRoom::default().to_bytes()),
            (0xFE, 0x36)
        );
        let ask = ExAskModifyPartyLooting {
            requestor: String::new(),
            distribution_type: PartyDistributionType::FindersKeepers,
        };
        assert_eq!(ex(ask.to_bytes()), (0xFE, 0xBF));
        let set = ExSetPartyLooting {
            result: 0,
            distribution_type: PartyDistributionType::FindersKeepers,
        };
        assert_eq!(ex(set.to_bytes()), (0xFE, 0xC0));
        assert_eq!(
            ex(ExRequestChangeNicknameColor { item_object_id: 0 }.to_bytes()),
            (0xFE, 0x83)
        );
    }

    /// `ExPartyRoomMember` writes one extra int (the instance data) after every member, which
    /// `ExManagePartyRoomMember` does not.
    #[test]
    fn room_member_sizes() {
        let one = ExPartyRoomMember {
            mode: 0,
            members: vec![member(1)],
        }
        .to_bytes();
        let manage = ExManagePartyRoomMember {
            mode: 0,
            member: member(1),
        }
        .to_bytes();
        // opcode(3) + mode + count + (id + "Aa\0" + 4 ints + instance) vs opcode + mode + member
        assert_eq!(one.len(), 3 + 4 + 4 + 4 + 6 + 16 + 4);
        assert_eq!(manage.len(), 3 + 4 + 4 + 6 + 16);
    }
}
