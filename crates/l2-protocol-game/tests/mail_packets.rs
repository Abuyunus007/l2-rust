//! The mail packets (M6-MAIL-01) against byte layouts worked out by hand from the Java
//! `readImpl` / `writeImpl` code, plus round trips and the "bad packet" rules of the readers.
//!
//! No recorded Java session holds a mail yet, so the layouts below are the spec. The expected
//! bytes are built with an independent little-endian builder, not the crate's `PacketWriter`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use l2_net::{Decode, Encode, PacketReader};
use l2_protocol_game::client::{
    PostAttachment, RequestCancelPostAttachment, RequestDeleteReceivedPost, RequestDeleteSentPost,
    RequestPostAttachment, RequestPostItemList, RequestReceivedPost, RequestReceivedPostList,
    RequestRejectPostAttachment, RequestSendPost, RequestSentPost, RequestSentPostList,
};
use l2_protocol_game::server::mail::{POST_STATE_DELETED, POST_STATE_READED, POST_STATE_REJECTED};
use l2_protocol_game::server::{
    ExChangePostState, ExMailArrived, ExNoticePostArrived, ExNoticePostSent, ExReplyPostItemList,
    ExReplyReceivedPost, ExReplySentPost, ExShowReceivedPostList, ExShowSentPostList, ItemInfo,
    ReceivedPostEntry, SentPostEntry,
};
use l2_protocol_game::{ClientPacket, ConnectionState, decode_client};

#[derive(Default)]
struct Bytes(Vec<u8>);

impl Bytes {
    fn byte(mut self, v: u8) -> Self {
        self.0.push(v);
        self
    }
    fn short(mut self, v: i16) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn int(mut self, v: i32) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn long(mut self, v: i64) -> Self {
        self.0.extend(v.to_le_bytes());
        self
    }
    fn string(mut self, text: &str) -> Self {
        for unit in text.encode_utf16() {
            self.0.extend(unit.to_le_bytes());
        }
        self.0.extend([0, 0]);
        self
    }
    fn raw(mut self, bytes: &[u8]) -> Self {
        self.0.extend(bytes);
        self
    }
    fn done(self) -> Vec<u8> {
        self.0
    }
}

/// A client ex packet: `0xD0`, the ex id.
fn client_ex(ex: i16) -> Bytes {
    Bytes::default().byte(0xD0).short(ex)
}

/// A server ex packet: `0xFE`, the ex id.
fn server_ex(ex: i16) -> Bytes {
    Bytes::default().byte(0xFE).short(ex)
}

fn check_round_trip<P: Encode + Decode + PartialEq + std::fmt::Debug>(
    packet: &P,
    expected: &[u8],
    opcode_len: usize,
) {
    let bytes = packet.to_bytes();
    assert_eq!(bytes, expected, "encoded bytes");
    let decoded = P::decode(&mut PacketReader::new(&bytes[opcode_len..])).unwrap();
    assert_eq!(&decoded, packet, "round trip");
}

/// Java `writeItem(ItemInfo)` of an item with these values.
fn item_bytes(b: Bytes, item: &ItemInfo) -> Bytes {
    let mut b = b
        .int(item.object_id)
        .int(item.item_id)
        .int(item.location)
        .long(item.count)
        .short(item.type2)
        .short(item.custom_type1)
        .short(item.equipped)
        .int(item.body_part)
        .short(item.enchant)
        .short(item.custom_type2)
        .int(item.augmentation)
        .int(item.mana)
        .int(item.time)
        .short(item.attack_element_type)
        .short(item.attack_element_power);
    for def in item.element_defense {
        b = b.short(def);
    }
    for option in item.enchant_options {
        b = b.short(option);
    }
    b
}

fn sword() -> ItemInfo {
    let mut item = ItemInfo::plain(0x6000_0010, 2, 1);
    item.location = 7;
    item.type2 = 0;
    item.body_part = 0x80;
    item.enchant = 3;
    item
}

#[test]
fn the_unit_requests_are_just_the_ex_id() {
    check_round_trip(&RequestPostItemList, &client_ex(0x65).done(), 3);
    check_round_trip(&RequestReceivedPostList, &client_ex(0x67).done(), 3);
    check_round_trip(&RequestSentPostList, &client_ex(0x6C).done(), 3);
}

#[test]
fn the_message_requests_carry_one_message_id() {
    check_round_trip(
        &RequestReceivedPost {
            msg_id: 1_610_612_800,
        },
        &client_ex(0x69).int(1_610_612_800).done(),
        3,
    );
    check_round_trip(&RequestSentPost { msg_id: 5 }, &client_ex(0x6E).int(5).done(), 3);
    check_round_trip(
        &RequestPostAttachment { msg_id: 6 },
        &client_ex(0x6A).int(6).done(),
        3,
    );
    check_round_trip(
        &RequestRejectPostAttachment { msg_id: 7 },
        &client_ex(0x6B).int(7).done(),
        3,
    );
    // `REQUEST_CANCEL_SENT_POST` is the Java class `RequestCancelPostAttachment`.
    check_round_trip(
        &RequestCancelPostAttachment { msg_id: 8 },
        &client_ex(0x6F).int(8).done(),
        3,
    );
}

#[test]
fn a_mail_with_attachments_and_a_payment_request() {
    let packet = RequestSendPost {
        receiver: "Bob".into(),
        is_cod: true,
        subject: "Sword".into(),
        text: "For sale".into(),
        items: Some(vec![
            PostAttachment {
                object_id: 100,
                count: 1,
            },
            PostAttachment {
                object_id: 101,
                count: 5_000,
            },
        ]),
        req_adena: 250_000,
    };
    let expected = client_ex(0x66)
        .string("Bob")
        .int(1)
        .string("Sword")
        .string("For sale")
        .int(2)
        .int(100)
        .long(1)
        .int(101)
        .long(5_000)
        .long(250_000)
        .done();
    check_round_trip(&packet, &expected, 3);
}

#[test]
fn a_mail_without_attachments() {
    let packet = RequestSendPost {
        receiver: "Bob".into(),
        is_cod: false,
        subject: "Hi".into(),
        text: String::new(),
        items: None,
        req_adena: 0,
    };
    let expected = client_ex(0x66)
        .string("Bob")
        .int(0)
        .string("Hi")
        .string("")
        .int(0)
        .long(0)
        .done();
    check_round_trip(&packet, &expected, 3);
}

/// The body after the ex id, for the bad packet tests.
fn send_post_body(count: i32, tail: Bytes) -> Vec<u8> {
    Bytes::default()
        .string("Bob")
        .int(0)
        .string("s")
        .string("t")
        .int(count)
        .raw(&tail.done())
        .done()
}

#[test]
fn a_bad_attachment_list_leaves_a_mail_without_attachments_like_java() {
    // NOTE(java-quirk): `readImpl` returns, `runImpl` sends the mail anyway.
    // The length does not fit the count: the adena is not read either.
    let body = send_post_body(2, Bytes::default().int(100).long(1).long(500));
    let packet = RequestSendPost::decode(&mut PacketReader::new(&body)).unwrap();
    assert_eq!((packet.items, packet.req_adena), (None, 0));

    // A negative count.
    let body = send_post_body(-1, Bytes::default().long(500));
    let packet = RequestSendPost::decode(&mut PacketReader::new(&body)).unwrap();
    assert_eq!((packet.items, packet.req_adena), (None, 0));

    // An item below 1 (object id 0, or count 0): the list is dropped, the adena is not read.
    for (object_id, count) in [(0, 1), (100, 0)] {
        let body = send_post_body(1, Bytes::default().int(object_id).long(count).long(500));
        let packet = RequestSendPost::decode(&mut PacketReader::new(&body)).unwrap();
        assert_eq!((packet.items, packet.req_adena), (None, 0));
    }

    // A negative adena drops the items but keeps the value (the handler then returns).
    let body = send_post_body(1, Bytes::default().int(100).long(1).long(-5));
    let packet = RequestSendPost::decode(&mut PacketReader::new(&body)).unwrap();
    assert_eq!((packet.items, packet.req_adena), (None, -5));

    // A good list.
    let body = send_post_body(1, Bytes::default().int(100).long(1).long(500));
    let packet = RequestSendPost::decode(&mut PacketReader::new(&body)).unwrap();
    assert_eq!(
        (packet.items, packet.req_adena),
        (
            Some(vec![PostAttachment {
                object_id: 100,
                count: 1
            }]),
            500
        )
    );
}

#[test]
fn the_delete_requests_are_a_counted_list_of_ids() {
    check_round_trip(
        &RequestDeleteReceivedPost {
            msg_ids: Some(vec![11, 12, 13]),
        },
        &client_ex(0x68).int(3).int(11).int(12).int(13).done(),
        3,
    );
    check_round_trip(
        &RequestDeleteSentPost {
            msg_ids: Some(vec![21]),
        },
        &client_ex(0x6D).int(1).int(21).done(),
        3,
    );
    // A count of 0, a negative count and a length that does not fit are all "no list".
    for body in [
        Bytes::default().int(0).done(),
        Bytes::default().int(-2).int(1).int(2).done(),
        Bytes::default().int(3).int(1).int(2).done(),
        Bytes::default().int(1).int(1).int(2).done(),
    ] {
        let packet = RequestDeleteReceivedPost::decode(&mut PacketReader::new(&body)).unwrap();
        assert_eq!(packet.msg_ids, None, "{body:?}");
        let packet = RequestDeleteSentPost::decode(&mut PacketReader::new(&body)).unwrap();
        assert_eq!(packet.msg_ids, None);
    }
}

#[test]
fn the_dispatcher_knows_the_mail_packets_in_game_only() {
    let bytes = RequestReceivedPost { msg_id: 9 }.to_bytes();
    let decode = |state| decode_client(state, bytes[0], &mut PacketReader::new(&bytes[1..])).unwrap();
    assert_eq!(
        decode(ConnectionState::InGame),
        Some(ClientPacket::RequestReceivedPost(RequestReceivedPost {
            msg_id: 9
        }))
    );
    assert_eq!(decode(ConnectionState::Entering), None);
}

#[test]
fn the_notices() {
    check_round_trip(
        &ExNoticePostArrived { show_animation: true },
        &server_ex(0xA9).int(1).done(),
        3,
    );
    check_round_trip(
        &ExNoticePostArrived {
            show_animation: false,
        },
        &server_ex(0xA9).int(0).done(),
        3,
    );
    check_round_trip(
        &ExNoticePostSent { show_animation: true },
        &server_ex(0xB4).int(1).done(),
        3,
    );
    check_round_trip(&ExMailArrived, &server_ex(0x2E).done(), 3);
}

#[test]
fn the_state_change_repeats_the_state_after_each_id() {
    check_round_trip(
        &ExChangePostState {
            received_board: true,
            msg_ids: vec![4, 5],
            change_id: POST_STATE_DELETED,
        },
        &server_ex(0xB3).int(1).int(2).int(4).int(0).int(5).int(0).done(),
        3,
    );
    check_round_trip(
        &ExChangePostState {
            received_board: false,
            msg_ids: vec![9],
            change_id: POST_STATE_READED,
        },
        &server_ex(0xB3).int(0).int(1).int(9).int(1).done(),
        3,
    );
    assert_eq!(POST_STATE_REJECTED, 2);
}

#[test]
fn the_inbox_list() {
    let packet = ExShowReceivedPostList {
        time: 1_700_000_000,
        mails: vec![ReceivedPostEntry {
            msg_id: 77,
            subject: "Hello".into(),
            sender_name: "Alice".into(),
            locked: true,
            expiration: 1_700_999_999,
            unread: true,
            has_attachments: true,
            returned: false,
            send_by_system: 0,
        }],
    };
    let expected = server_ex(0xAA)
        .int(1_700_000_000)
        .int(1)
        .int(77)
        .string("Hello")
        .string("Alice")
        .int(1)
        .int(1_700_999_999)
        .int(1)
        .int(1)
        .int(1)
        .int(0)
        .int(0)
        .int(0)
        .done();
    check_round_trip(&packet, &expected, 3);
    // An empty inbox is the time and a zero count.
    check_round_trip(
        &ExShowReceivedPostList {
            time: 5,
            mails: vec![],
        },
        &server_ex(0xAA).int(5).int(0).done(),
        3,
    );
}

#[test]
fn the_outbox_list() {
    let packet = ExShowSentPostList {
        time: 1_700_000_000,
        mails: vec![SentPostEntry {
            msg_id: 78,
            subject: "Re".into(),
            receiver_name: "Bob".into(),
            locked: false,
            expiration: 42,
            unread: false,
            has_attachments: true,
        }],
    };
    let expected = server_ex(0xAC)
        .int(1_700_000_000)
        .int(1)
        .int(78)
        .string("Re")
        .string("Bob")
        .int(0)
        .int(42)
        .int(0)
        .int(1)
        .int(1)
        .done();
    check_round_trip(&packet, &expected, 3);
}

#[test]
fn a_received_mail_opened() {
    let item = sword();
    let packet = ExReplyReceivedPost {
        msg_id: 77,
        locked: true,
        sender_name: "Alice".into(),
        subject: "Hello".into(),
        content: "Pay me".into(),
        items: vec![item.clone()],
        req_adena: 1_000,
        has_attachments: true,
        send_by_system: 0,
    };
    let head = server_ex(0xAB)
        .int(77)
        .int(1)
        .int(0)
        .string("Alice")
        .string("Hello")
        .string("Pay me")
        .int(1);
    // The item block, then its object id again.
    let expected = item_bytes(head, &item)
        .int(item.object_id)
        .long(1_000)
        .int(1)
        .int(0)
        .done();
    check_round_trip(&packet, &expected, 3);

    // Without items: a zero count, the adena, the flags.
    let plain = ExReplyReceivedPost {
        msg_id: 78,
        locked: false,
        sender_name: "****".into(),
        subject: "News".into(),
        content: "Text".into(),
        items: vec![],
        req_adena: 0,
        has_attachments: false,
        send_by_system: 1,
    };
    let expected = server_ex(0xAB)
        .int(78)
        .int(0)
        .int(0)
        .string("****")
        .string("News")
        .string("Text")
        .int(0)
        .long(0)
        .int(0)
        .int(1)
        .done();
    check_round_trip(&plain, &expected, 3);
}

#[test]
fn a_sent_mail_opened() {
    let item = sword();
    let packet = ExReplySentPost {
        msg_id: 80,
        locked: false,
        receiver_name: "Bob".into(),
        subject: "Gift".into(),
        content: "Enjoy".into(),
        items: vec![item.clone()],
        req_adena: 0,
        send_by_system: 0,
    };
    let head = server_ex(0xAD)
        .int(80)
        .int(0)
        .string("Bob")
        .string("Gift")
        .string("Enjoy")
        .int(1);
    let expected = item_bytes(head, &item).int(item.object_id).long(0).int(0).done();
    check_round_trip(&packet, &expected, 3);

    // NOTE(java-quirk): without items the packet ends after the adena.
    let plain = ExReplySentPost {
        msg_id: 81,
        locked: true,
        receiver_name: "Bob".into(),
        subject: "Pay".into(),
        content: String::new(),
        items: vec![],
        req_adena: 99,
        send_by_system: 0,
    };
    let expected = server_ex(0xAD)
        .int(81)
        .int(1)
        .string("Bob")
        .string("Pay")
        .string("")
        .int(0)
        .long(99)
        .done();
    check_round_trip(&plain, &expected, 3);
}

#[test]
fn the_attachable_items() {
    let item = sword();
    let packet = ExReplyPostItemList {
        items: vec![item.clone()],
    };
    let expected = item_bytes(server_ex(0xB2).int(1), &item).done();
    check_round_trip(&packet, &expected, 3);
}
