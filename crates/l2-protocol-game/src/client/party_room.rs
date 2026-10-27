//! Client packets of the party matching rooms, the loot rule vote, the nickname items and the GM
//! windows (L-03).
//!
//! Plain readers; the rules are the world's job (`l2-world/src/party/matching.rs`,
//! `party/loot_vote.rs`, `party/nickname.rs`, `admin/gm_view.rs`).

client_packet!(
    // Java: gameserver/network/clientpackets/RequestPartyMatchConfig.java
    /// The party matching button / the list of rooms (`REQUEST_PARTY_MATCH_CONFIG`, 0x7F).
    RequestPartyMatchConfig,
    plain RequestPartyMatchConfig {
        /// "auto": Java reads it and the list packet ignores it.
        auto: i32,
        /// The location filter (a `MapRegionData` bbs id, 0 = all).
        location: i32,
        /// The level filter of the list: 0 shows only the rooms the player's level fits.
        level: i32,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestPartyMatchList.java
    /// Creates a room (`room_id` 0) or changes the room `room_id` (`REQUEST_PARTY_MATCH_LIST`,
    /// 0x80).
    RequestPartyMatchList,
    plain RequestPartyMatchList {
        room_id: i32,
        members_max: i32,
        min_level: i32,
        max_level: i32,
        loot: i32,
        room_title: String,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestPartyMatchDetail.java
    /// A click on a room of the list (`REQUEST_PARTY_MATCH_DETAIL`, 0x81).
    RequestPartyMatchDetail,
    plain RequestPartyMatchDetail {
        room_id: i32,
        /// Java reads three more ints and ignores them (0 0 0 on a click, -1 1 1 on auto join).
        unk1: i32,
        unk2: i32,
        unk3: i32,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestOustFromPartyRoom.java
    /// The room owner throws a member out (`REQUEST_OUST_FROM_PARTY_ROOM`, `0xD0` 0x0009).
    RequestOustFromPartyRoom,
    ex RequestOustFromPartyRoom {
        /// The object id of the member.
        char_id: i32,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestDismissPartyRoom.java
    /// Closes the room (`REQUEST_DISMISS_PARTY_ROOM`, `0xD0` 0x000A).
    RequestDismissPartyRoom,
    ex RequestDismissPartyRoom {
        room_id: i32,
        /// Read and ignored by Java.
        data2: i32,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestWithdrawPartyRoom.java
    /// A member leaves the room (`REQUEST_WITHDRAW_PARTY_ROOM`, `0xD0` 0x000B).
    RequestWithdrawPartyRoom,
    ex RequestWithdrawPartyRoom {
        room_id: i32,
        /// Read and ignored by Java.
        unk1: i32,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestAskJoinPartyRoom.java
    /// Invites the player `name` into the room (`REQUEST_ASK_JOIN_PARTY_ROOM`, `0xD0` 0x002F).
    RequestAskJoinPartyRoom,
    ex RequestAskJoinPartyRoom {
        name: String,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/AnswerJoinPartyRoom.java
    /// The invited player answers (`ANSWER_JOIN_PARTY_ROOM`, `0xD0` 0x0030): 1 accepts.
    AnswerJoinPartyRoom,
    ex AnswerJoinPartyRoom {
        answer: i32,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestListPartyMatchingWaitingRoom.java
    /// The list of the players who wait for a room
    /// (`REQUEST_LIST_PARTY_MATCHING_WAITING_ROOM`, `0xD0` 0x0031).
    RequestListPartyMatchingWaitingRoom,
    ex RequestListPartyMatchingWaitingRoom {
        page: i32,
        min_level: i32,
        max_level: i32,
        /// 1 = the waiting list, 0 = "room waiting list" (Java answers it with an empty list).
        mode: i32,
    }
);

client_unit_packet!(
    // Java: gameserver/network/clientpackets/RequestExitPartyMatchingWaitingRoom.java
    /// The player leaves the waiting list (`REQUEST_EXIT_PARTY_MATCHING_WAITING_ROOM`, `0xD0`
    /// 0x0025). No fields.
    RequestExitPartyMatchingWaitingRoom,
    ex RequestExitPartyMatchingWaitingRoom
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestPartyLootModification.java
    /// The leader asks to change the loot rule (`REQUEST_PARTY_LOOT_MODIFICATION`, `0xD0` 0x0078).
    /// The raw `PartyDistributionType` id; Java ignores an unknown one.
    RequestPartyLootModification,
    ex RequestPartyLootModification {
        distribution_type_id: i32,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/AnswerPartyLootModification.java
    /// A member answers the loot rule vote (`ANSWER_PARTY_LOOT_MODIFICATION`, `0xD0` 0x0079):
    /// 1 agrees.
    AnswerPartyLootModification,
    ex AnswerPartyLootModification {
        answer: i32,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestGiveNickName.java
    /// A title for a clan member or, for a noblesse, for itself (`REQUEST_GIVE_NICK_NAME`, 0x0B).
    RequestGiveNickName,
    plain RequestGiveNickName {
        target: String,
        title: String,
    }
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestChangeNicknameColor.java
    /// Uses a title colour item (`REQUEST_CHANGE_NICKNAME_COLOR`, `0xD0` 0x004F).
    RequestChangeNicknameColor,
    ex RequestChangeNicknameColor {
        color_num: i32,
        title: String,
        item_object_id: i32,
    }
);

client_unit_packet!(
    // Java: gameserver/network/clientpackets/RequestResetNickname.java
    /// Resets the title and its colour (`REQUEST_RESET_NICKNAME`, `0xD0` 0x0050). No fields.
    RequestResetNickname,
    ex RequestResetNickname
);

client_unit_packet!(
    // Java: gameserver/network/clientpackets/RequestGmList.java
    /// The `/gmlist` command (`REQUEST_GM_LIST`, 0x8B). No fields.
    RequestGmList,
    plain RequestGmList
);

client_packet!(
    // Java: gameserver/network/clientpackets/RequestGMCommand.java
    /// The Alt+G window of a GM (`REQUEST_GM_COMMAND`, 0x7E): which view of `target_name`.
    RequestGmCommand,
    plain RequestGmCommand {
        target_name: String,
        /// 1 status, 2 clan, 3 skills, 4 quests, 5 inventory, 6 warehouse. Java reads one more
        /// int it does not use.
        command: i32,
    }
);

#[cfg(test)]
mod tests {
    use l2_net::{Decode, Encode, PacketReader};

    use super::*;

    fn round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(packet: P, head: usize) {
        let bytes = packet.to_bytes();
        let decoded = P::decode(&mut PacketReader::new(&bytes[head..])).unwrap();
        assert_eq!(decoded, packet);
    }

    #[test]
    fn every_packet_round_trips() {
        round_trip(
            RequestPartyMatchConfig {
                auto: 1,
                location: 2,
                level: 3,
            },
            1,
        );
        round_trip(
            RequestPartyMatchList {
                room_id: 0,
                members_max: 9,
                min_level: 10,
                max_level: 80,
                loot: 1,
                room_title: "Looking for a healer".into(),
            },
            1,
        );
        round_trip(
            RequestPartyMatchDetail {
                room_id: 4,
                unk1: -1,
                unk2: 1,
                unk3: 1,
            },
            1,
        );
        round_trip(RequestOustFromPartyRoom { char_id: 7 }, 3);
        round_trip(RequestDismissPartyRoom { room_id: 7, data2: 0 }, 3);
        round_trip(RequestWithdrawPartyRoom { room_id: 7, unk1: 0 }, 3);
        round_trip(RequestAskJoinPartyRoom { name: "Bob".into() }, 3);
        round_trip(AnswerJoinPartyRoom { answer: 1 }, 3);
        round_trip(
            RequestListPartyMatchingWaitingRoom {
                page: 1,
                min_level: 1,
                max_level: 85,
                mode: 1,
            },
            3,
        );
        round_trip(RequestExitPartyMatchingWaitingRoom, 3);
        round_trip(
            RequestPartyLootModification {
                distribution_type_id: 2,
            },
            3,
        );
        round_trip(AnswerPartyLootModification { answer: 1 }, 3);
        round_trip(
            RequestGiveNickName {
                target: "Bob".into(),
                title: "Boss".into(),
            },
            1,
        );
        round_trip(
            RequestChangeNicknameColor {
                color_num: 3,
                title: "Pink".into(),
                item_object_id: 0x1000_0001,
            },
            3,
        );
        round_trip(RequestResetNickname, 3);
        round_trip(RequestGmList, 1);
        round_trip(
            RequestGmCommand {
                target_name: "Target".into(),
                command: 5,
            },
            1,
        );
    }

    /// The ids of the Java `ClientPackets` / `ExClientPackets` tables.
    #[test]
    fn the_ids_are_javas() {
        assert_eq!(RequestPartyMatchConfig::OPCODE.id(), 0x7F);
        assert_eq!(RequestPartyMatchList::OPCODE.id(), 0x80);
        assert_eq!(RequestPartyMatchDetail::OPCODE.id(), 0x81);
        assert_eq!(RequestGiveNickName::OPCODE.id(), 0x0B);
        assert_eq!(RequestGmList::OPCODE.id(), 0x8B);
        assert_eq!(RequestGmCommand::OPCODE.id(), 0x7E);
        let ex = |p: &[u8]| (p[0], u16::from_le_bytes([p[1], p[2]]));
        assert_eq!(
            ex(&RequestOustFromPartyRoom { char_id: 0 }.to_bytes()),
            (0xD0, 0x09)
        );
        assert_eq!(
            ex(&RequestDismissPartyRoom { room_id: 0, data2: 0 }.to_bytes()),
            (0xD0, 0x0A)
        );
        assert_eq!(ex(&RequestExitPartyMatchingWaitingRoom.to_bytes()), (0xD0, 0x25));
        assert_eq!(
            ex(&RequestAskJoinPartyRoom { name: String::new() }.to_bytes()),
            (0xD0, 0x2F)
        );
        assert_eq!(ex(&AnswerJoinPartyRoom { answer: 0 }.to_bytes()), (0xD0, 0x30));
        assert_eq!(
            ex(&RequestPartyLootModification {
                distribution_type_id: 0
            }
            .to_bytes()),
            (0xD0, 0x78)
        );
        assert_eq!(
            ex(&AnswerPartyLootModification { answer: 0 }.to_bytes()),
            (0xD0, 0x79)
        );
        assert_eq!(ex(&RequestResetNickname.to_bytes()), (0xD0, 0x50));
    }

    /// `RequestPartyMatchList.readImpl`: five ints, then the title.
    #[test]
    fn party_match_list_layout() {
        let mut body = Vec::new();
        for v in [0_i32, 9, 20, 40, 1] {
            body.extend(v.to_le_bytes());
        }
        body.extend([b'A', 0, 0, 0]);
        let p = RequestPartyMatchList::decode(&mut PacketReader::new(&body)).unwrap();
        assert_eq!(p.members_max, 9);
        assert_eq!(p.min_level, 20);
        assert_eq!(p.max_level, 40);
        assert_eq!(p.loot, 1);
        assert_eq!(p.room_title, "A");
    }
}
