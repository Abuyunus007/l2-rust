//! Scenario: a trade and a private store. The leader and the member trade gems and adena for ore,
//! then the member walks to a quiet spot, sits down with a store of gems and the leader buys two.

use l2_protocol_game::server::{
    ChangeWaitType, ItemList, PrivateStoreListSell, PrivateStoreMsgSell, PrivateStoreManageListSell, SendTradeRequest,
    TradeOtherAdd, TradeOwnAdd, TradeStart,
};

use super::*;

/// What the trade moves: the leader gives 5 gems and 1000 adena, the member 3 ore.
const GEMS_TRADED: i64 = 5;
const ADENA_TRADED: i64 = 1_000;
const ORE_TRADED: i64 = 3;
/// What the store sells: the 5 gems the member got, at 200 each; the leader buys 2.
const STORE_PRICE: i64 = 200;
const STORE_GEMS: i64 = 5;
const GEMS_BOUGHT: i64 = 2;

pub struct Played {
    left_a: Left,
    left_b: Left,
}

/// The object id of the stack of `item_id` in an `ItemList`.
fn object_of(list: &ItemList, item_id: i32) -> i32 {
    list.items
        .iter()
        .find(|i| i.item_id == item_id)
        .unwrap_or_else(|| panic!("item {item_id} in {list:?}"))
        .object_id
}

pub async fn play<H: Harness>(h: &mut H) -> Played {
    let (a, b) = (h.object_id(Who::A), h.object_id(Who::B));
    let entered_a = h.enter(Who::A).await;
    let list_a: ItemList = all(&entered_a, "ITEM_LIST").remove(0);
    let entered_b = h.enter(Who::B).await;
    let list_b: ItemList = all(&entered_b, "ITEM_LIST").remove(0);
    h.collect_quiet(Who::A).await;

    // ---- the request: A asks, B is asked -------------------------------------------------------
    h.act(Who::A, act::trade_request(b)).await;
    let asked_a = h.collect_quiet(Who::A).await;
    let asked_b = h.collect_quiet(Who::B).await;
    assert_eq!(
        message_of(&asked_a, SystemMessageId::YouHaveRequestedATradeWithC1).params,
        [text(NAME_B)]
    );
    assert_eq!(all::<SendTradeRequest>(&asked_b, "TRADE_REQUEST")[0].sender_id, a);

    // ---- the answer: the trade window opens on both sides ---------------------------------------
    h.act(Who::B, act::answer_trade(1)).await;
    let started_a = h.collect_quiet(Who::A).await;
    let started_b = h.collect_quiet(Who::B).await;
    for (batch, partner, name) in [(&started_a, b, NAME_B), (&started_b, a, NAME_A)] {
        assert_eq!(
            message_of(batch, SystemMessageId::YouBeginTradingWithC1).params,
            [SystemMessageParam::PlayerName(name.to_owned())]
        );
        let start: TradeStart = all(batch, "TRADE_START").remove(0);
        assert_eq!(start.partner_object_id, partner);
        // The window lists what can be traded: the starting items without the worn ones.
        assert!(start.items.iter().any(|i| i.item_id == m6_data::GEM), "{start:?}");
        assert!(start.items.iter().all(|i| i.equipped == 0), "{start:?}");
    }

    // ---- both fill their side: the other side sees it as it is added ----------------------------
    h.act(
        Who::A,
        act::add_trade_item(b, object_of(&list_a, m6_data::GEM), GEMS_TRADED),
    )
    .await;
    h.act(
        Who::A,
        act::add_trade_item(b, object_of(&list_a, ADENA_ID), ADENA_TRADED),
    )
    .await;
    h.act(
        Who::B,
        act::add_trade_item(a, object_of(&list_b, m6_data::ORE), ORE_TRADED),
    )
    .await;
    let added_a = h.collect_quiet(Who::A).await;
    let added_b = h.collect_quiet(Who::B).await;
    let own_a: Vec<(i32, i64)> = all::<TradeOwnAdd>(&added_a, "TRADE_OWN_ADD")
        .iter()
        .map(|p| (p.item.item_id, p.item.count))
        .collect();
    assert_eq!(
        own_a,
        [(m6_data::GEM, GEMS_TRADED), (ADENA_ID, ADENA_TRADED)],
        "A's own side"
    );
    let other_b: Vec<(i32, i64)> = all::<TradeOtherAdd>(&added_b, "TRADE_OTHER_ADD")
        .iter()
        .map(|p| (p.item.item_id, p.item.count))
        .collect();
    assert_eq!(other_b, own_a, "what B sees of A's side");
    let other_a: Vec<(i32, i64)> = all::<TradeOtherAdd>(&added_a, "TRADE_OTHER_ADD")
        .iter()
        .map(|p| (p.item.item_id, p.item.count))
        .collect();
    assert_eq!(other_a, [(m6_data::ORE, ORE_TRADED)], "what A sees of B's side");

    // ---- A confirms, then B: the exchange happens when the second one confirms --------------------
    h.act(Who::A, act::trade_done(1)).await;
    let confirmed_a = h.collect_quiet(Who::A).await;
    let confirmed_b = h.collect_quiet(Who::B).await;
    assert!(!names(&confirmed_a).contains(&"TRADE_DONE".to_owned()), "not yet: {:?}", names(&confirmed_a));
    assert_eq!(
        message_of(&confirmed_b, SystemMessageId::C1HasConfirmedTheTrade).params,
        [SystemMessageParam::PlayerName(NAME_A.to_owned())]
    );
    assert!(names(&confirmed_b).contains(&"TRADE_PRESS_OTHER_OK".to_owned()));
    h.act(Who::B, act::trade_done(1)).await;
    let done_a = h.collect_quiet(Who::A).await;
    let done_b = h.collect_quiet(Who::B).await;
    for batch in [&done_a, &done_b] {
        assert_in_order(
            &names(batch),
            &["TRADE_DONE", "SYSTEM_MESSAGE", "INVENTORY_UPDATE", "ITEM_LIST"],
        );
        assert!(message_ids(batch).contains(&SystemMessageId::YourTradeWasSuccessful));
    }

    // ---- the store: B walks to a quiet spot, opens the manage window, lists the gems ----------------
    h.act(Who::B, act::move_to(m6_data::STORE_SPOT, m3_data::START)).await;
    h.wait_seconds(4).await;
    h.collect_quiet(Who::B).await;
    h.act(Who::B, act::action_use(10)).await;
    let manage = h.collect_quiet(Who::B).await;
    let window: PrivateStoreManageListSell = all(&manage, "PRIVATE_STORE_SELL_MANAGE_LIST").remove(0);
    assert_eq!((window.object_id, window.package_sale), (b, false));
    assert!(window.sell_list.is_empty());
    h.act(
        Who::B,
        act::store_list_sell(false, &[(object_of(&list_b, m6_data::GEM), STORE_GEMS, STORE_PRICE)]),
    )
    .await;
    let listed = h.collect_quiet(Who::B).await;
    // Java `SetPrivateStoreListSell`: sit down, the title of the (still empty) store.
    assert_in_order(&names(&listed), &["CHANGE_WAIT_TYPE", "USER_INFO"]);
    let sit: ChangeWaitType = all(&listed, "CHANGE_WAIT_TYPE").remove(0);
    assert_eq!((sit.object_id, sit.move_type), (b, ChangeWaitType::WT_SITTING));
    h.act(Who::B, act::store_msg_sell("Gems")).await;
    let titled = h.collect_quiet(Who::B).await;
    let title: PrivateStoreMsgSell = all(&titled, "PRIVATE_STORE_SELL_MSG").remove(0);
    assert_eq!((title.object_id, title.message.as_str()), (b, "Gems"));
    // A, at the start, is told: the member sits and the store's title shows over its head.
    let seen_by_a = h.collect_quiet(Who::A).await;
    assert!(names(&seen_by_a).contains(&"CHANGE_WAIT_TYPE".to_owned()), "{:?}", names(&seen_by_a));
    // NOTE(java-quirk): `SetPrivateStoreMsgSell` sends the typed title to its owner only, so A
    // saw the (still empty) title of the moment the list was set.
    let titles: Vec<PrivateStoreMsgSell> = all(&seen_by_a, "PRIVATE_STORE_SELL_MSG");
    assert_eq!(
        titles.iter().map(|t| (t.object_id, t.message.as_str())).collect::<Vec<_>>(),
        [(b, "")]
    );

    // ---- the customer: two clicks (select, then walk up and look), then the purchase ------------------
    h.act(Who::A, act::click(b)).await;
    h.collect_until(Who::A, &|body| is_named(body, "MY_TARGET_SELECTED")).await;
    h.act(Who::A, act::click(b)).await;
    let shown = h
        .collect_until(Who::A, &|body| is_named(body, "PRIVATE_STORE_SELL_LIST"))
        .await;
    let offered: PrivateStoreListSell = all(&shown, "PRIVATE_STORE_SELL_LIST").remove(0);
    assert_eq!((offered.seller_object_id, offered.package_sale), (b, false));
    assert_eq!(offered.items.len(), 1, "{offered:?}");
    let entry = &offered.items[0];
    assert_eq!(
        (entry.item.item_id, entry.item.count, entry.price),
        (m6_data::GEM, STORE_GEMS, STORE_PRICE)
    );
    // The reference price is twice the sell price of the item (Java `getReferencePrice() * 2`).
    assert_eq!(entry.reference_price, m6_data::GEM_PRICE * 2);

    h.act(
        Who::A,
        act::store_buy(b, &[(entry.item.object_id, GEMS_BOUGHT, STORE_PRICE)]),
    )
    .await;
    let bought_a = h.collect_quiet(Who::A).await;
    let sold_b = h.collect_quiet(Who::B).await;
    assert_eq!(
        message_of(&bought_a, SystemMessageId::YouHavePurchasedS3S2SFromC1).params,
        [
            text(NAME_B),
            SystemMessageParam::ItemName(m6_data::GEM),
            SystemMessageParam::LongNumber(GEMS_BOUGHT)
        ]
    );
    assert_eq!(
        message_of(&sold_b, SystemMessageId::C1PurchasedS3S2S).params,
        [
            text(NAME_A),
            SystemMessageParam::ItemName(m6_data::GEM),
            SystemMessageParam::LongNumber(GEMS_BOUGHT)
        ]
    );
    assert!(names(&bought_a).contains(&"INVENTORY_UPDATE".to_owned()));
    assert!(names(&sold_b).contains(&"INVENTORY_UPDATE".to_owned()));

    let left_a = h.leave(Who::A).await;
    let left_b = h.leave(Who::B).await;
    Played { left_a, left_b }
}

pub fn assert_played(played: &Played) {
    let (a, b) = (&played.left_a, &played.left_b);
    let start_gems = m6_data::GEMS_AT_START;
    let start_ore = m6_data::ORE_AT_START;
    let paid = STORE_PRICE * GEMS_BOUGHT;
    // A: gave 5 gems and 1000 adena, got 3 ore; bought 2 gems for 400.
    assert_eq!(a.count_of(m6_data::GEM), start_gems - GEMS_TRADED + GEMS_BOUGHT);
    assert_eq!(a.count_of(ADENA_ID), m6_data::START_ADENA - ADENA_TRADED - paid);
    assert_eq!(a.count_of(m6_data::ORE), start_ore + ORE_TRADED);
    // B: the mirror image; the gems it sold left the stack, the adena came.
    assert_eq!(b.count_of(m6_data::GEM), start_gems + GEMS_TRADED - GEMS_BOUGHT);
    assert_eq!(b.count_of(ADENA_ID), m6_data::START_ADENA + ADENA_TRADED + paid);
    assert_eq!(b.count_of(m6_data::ORE), start_ore - ORE_TRADED);
    // Stacks merge: nobody got a second row of a stackable item.
    for left in [a, b] {
        for item_id in [m6_data::GEM, m6_data::ORE, ADENA_ID] {
            let rows = left.items.iter().filter(|i| i.item_id == item_id).count();
            assert_eq!(rows, 1, "item {item_id}: {:?}", left.items);
        }
    }
}

#[tokio::test]
async fn a_trade_and_a_private_store_in_a_bare_world() {
    let mut world = BareWorld::start(false);
    let played = play(&mut world).await;
    assert_played(&played);
}

#[tokio::test]
async fn a_trade_and_a_private_store_on_the_real_servers() {
    let Some(mut net) = Net::start("e2em6t", false).await else {
        return;
    };
    let played = play(&mut net).await;
    assert_played(&played);
    net.finish().await;
}
