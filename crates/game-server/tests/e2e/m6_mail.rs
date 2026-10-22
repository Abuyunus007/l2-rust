//! Scenario: mail with an attachment. The leader sends the member a letter with gems, the member
//! sees the arrival, lists and reads the letter and takes the gems.

use l2_protocol_game::server::{
    ExNoticePostArrived, ExNoticePostSent, ExReplyReceivedPost, ExShowReceivedPostList, ItemList,
};

use super::*;

const SUBJECT: &str = "Hello";
const TEXT: &str = "Some gems for you";
const GEMS_SENT: i64 = 7;

pub struct Played {
    left_a: Left,
    left_b: Left,
    mail_after_sending: Vec<MailRow>,
    mail_after_taking: Vec<MailRow>,
    message_id: i32,
}

pub async fn play<H: Harness>(h: &mut H) -> Played {
    let (a, b) = (h.object_id(Who::A), h.object_id(Who::B));
    let entered_a = h.enter(Who::A).await;
    let list: ItemList = all(&entered_a, "ITEM_LIST").remove(0);
    let gems = list
        .items
        .iter()
        .find(|i| i.item_id == m6_data::GEM)
        .expect("gems")
        .object_id;
    h.enter(Who::B).await;
    h.collect_quiet(Who::A).await;

    // ---- A writes the letter in the peace zone and attaches the gems -------------------------------------
    h.act(
        Who::A,
        act::send_post(client::RequestSendPost {
            receiver: NAME_B.to_owned(),
            is_cod: false,
            subject: SUBJECT.to_owned(),
            text: TEXT.to_owned(),
            items: Some(vec![client::PostAttachment {
                object_id: gems,
                count: GEMS_SENT,
            }]),
            req_adena: 0,
        }),
    )
    .await;
    let sent = h.collect_quiet(Who::A).await;
    assert!(message_ids(&sent).contains(&SystemMessageId::MailSuccessfullySent));
    assert!(all::<ExNoticePostSent>(&sent, "EX_NOTICE_POST_SENT")[0].show_animation);
    // The gems left the stack at once.
    let updates: Vec<l2_protocol_game::server::InventoryUpdate> = all(&sent, "INVENTORY_UPDATE");
    assert!(
        updates
            .iter()
            .flat_map(|u| u.updates.iter())
            .any(|u| u.item.item_id == m6_data::GEM && u.item.count == m6_data::GEMS_AT_START - GEMS_SENT),
        "{updates:?}"
    );
    let arrived = h.collect_quiet(Who::B).await;
    assert!(all::<ExNoticePostArrived>(&arrived, "EX_NOTICE_POST_ARRIVED")[0].show_animation);
    let mail_after_sending = h.mail_rows().await;

    // ---- B lists the received mail, reads the letter, takes the gems --------------------------------------------
    h.act(Who::B, act::received_post_list()).await;
    let listed = h.collect_quiet(Who::B).await;
    let post_list: ExShowReceivedPostList = all(&listed, "EX_SHOW_RECEIVED_POST_LIST").remove(0);
    assert_eq!(post_list.mails.len(), 1, "{post_list:?}");
    let entry = &post_list.mails[0];
    assert_eq!(
        (
            entry.subject.as_str(),
            entry.sender_name.as_str(),
            entry.unread,
            entry.has_attachments
        ),
        (SUBJECT, NAME_A, true, true)
    );
    let message_id = entry.msg_id;

    h.act(Who::B, act::received_post(message_id)).await;
    let read = h.collect_quiet(Who::B).await;
    let reply: ExReplyReceivedPost = all(&read, "EX_REPLY_RECEIVED_POST").remove(0);
    assert_eq!(
        (
            reply.msg_id,
            reply.sender_name.as_str(),
            reply.subject.as_str(),
            reply.content.as_str()
        ),
        (message_id, NAME_A, SUBJECT, TEXT)
    );
    assert_eq!(
        reply
            .items
            .iter()
            .map(|i| (i.item_id, i.count))
            .collect::<Vec<_>>(),
        [(m6_data::GEM, GEMS_SENT)]
    );

    h.act(Who::B, act::post_attachment(message_id)).await;
    let taken = h.collect_quiet(Who::B).await;
    assert!(
        names(&taken).contains(&"INVENTORY_UPDATE".to_owned()),
        "{:?}",
        names(&taken)
    );
    assert!(
        message_ids(&taken).contains(&SystemMessageId::YouHaveAcquiredS2S1),
        "{:?}",
        system_messages(&taken)
    );
    let mail_after_taking = h.mail_rows().await;

    let left_a = h.leave(Who::A).await;
    let left_b = h.leave(Who::B).await;
    let _ = (a, b);
    Played {
        left_a,
        left_b,
        mail_after_sending,
        mail_after_taking,
        message_id,
    }
}

pub fn assert_played(played: &Played, a: i32, b: i32) {
    // The row Java inserts: unread, with attachments, nobody deleted it.
    let letter = |has_attachments: bool, unread: bool| MailRow {
        message_id: played.message_id,
        sender_id: a,
        receiver_id: b,
        subject: SUBJECT.to_owned(),
        content: TEXT.to_owned(),
        req_adena: 0,
        has_attachments,
        unread,
        deleted_by_sender: false,
        deleted_by_receiver: false,
    };
    assert_eq!(played.mail_after_sending, [letter(true, true)]);
    // Reading marks it read; taking the items clears `hasAttachments`.
    assert_eq!(played.mail_after_taking, [letter(false, false)]);
    assert!(
        played.message_id >= l2_world::RUST_FIRST_OBJECT_ID,
        "{}",
        played.message_id
    );

    // The gems: 7 left A's stack, and B's stack grew by 7 (no row of the mail is left).
    assert_eq!(
        played.left_a.count_of(m6_data::GEM),
        m6_data::GEMS_AT_START - GEMS_SENT
    );
    assert_eq!(
        played.left_b.count_of(m6_data::GEM),
        m6_data::GEMS_AT_START + GEMS_SENT
    );
    for left in [&played.left_a, &played.left_b] {
        assert!(left.items.iter().all(|i| i.loc != "MAIL"), "{:?}", left.items);
        let rows = left.items.iter().filter(|i| i.item_id == m6_data::GEM).count();
        assert_eq!(rows, 1, "{:?}", left.items);
    }
}

#[tokio::test]
async fn a_letter_with_an_attachment_in_a_bare_world() {
    let mut world = BareWorld::start(false);
    let (a, b) = (world.object_id(Who::A), world.object_id(Who::B));
    let played = play(&mut world).await;
    assert_played(&played, a, b);
}

#[tokio::test]
async fn a_letter_with_an_attachment_on_the_real_servers() {
    let Some(mut net) = Net::start("e2em6m", false).await else {
        return;
    };
    let (a, b) = (net.object_id(Who::A), net.object_id(Who::B));
    let played = play(&mut net).await;
    assert_played(&played, a, b);
    net.finish().await;
}
