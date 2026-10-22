//! Scenario: a visit to the town's NPCs. The player talks to the smith, buys soulshots and mana
//! potions, sells gems, exchanges ore for ingots, and then deposits gems with the warehouse keeper
//! and takes some back.

use l2_protocol_game::server::{
    BuyList, ExBuySellList, MultiSellList, NpcHtmlMessage, NpcInfo, WareHouseDepositList,
    WareHouseWithdrawalList,
};

use super::*;

/// What the visit buys and sells.
const SHOTS_BOUGHT: i64 = 5;
const POTIONS_BOUGHT: i64 = 2;
const POTION_PRICE: i64 = 50;
const GEMS_SOLD: i64 = 6;
/// The exchange is run twice.
const EXCHANGES: i64 = 2;
const GEMS_DEPOSITED: i64 = 10;
const GEMS_WITHDRAWN: i64 = 4;
/// Java `SendWareHouseDepositList`: 30 adena for every item put into the private warehouse.
const DEPOSIT_FEE: i64 = 30;

pub struct Played {
    left: Left,
}

/// The buy-tab packets in `batch` (the sell tab shares their opcode and is skipped).
fn buy_lists(batch: &[Vec<u8>]) -> Vec<BuyList> {
    batch
        .iter()
        .filter(|body| is_named(body, "EX_BUY_SELL_LIST"))
        .filter_map(|body| BuyList::decode(&mut PacketReader::new(&body[3..])).ok())
        .collect()
}

/// The sell-tab packets in `batch`.
fn sell_lists(batch: &[Vec<u8>]) -> Vec<ExBuySellList> {
    batch
        .iter()
        .filter(|body| is_named(body, "EX_BUY_SELL_LIST"))
        .filter_map(|body| ExBuySellList::decode(&mut PacketReader::new(&body[3..])).ok())
        .collect()
}

/// Clicks `npc` twice (select, then talk) and returns the page it shows.
async fn talk_to<H: Harness>(h: &mut H, npc: i32) -> NpcHtmlMessage {
    h.act(Who::A, act::click(npc)).await;
    h.collect_until(Who::A, &|body| is_named(body, "MY_TARGET_SELECTED")).await;
    h.act(Who::A, act::click(npc)).await;
    let page = h.collect_until(Who::A, &|body| is_named(body, "NPC_HTML_MESSAGE")).await;
    all(&page, "NPC_HTML_MESSAGE").remove(0)
}

pub async fn play<H: Harness>(h: &mut H) -> Played {
    let entered = h.enter(Who::A).await;
    let npcs: Vec<NpcInfo> = all(&entered, "NPC_INFO");
    let npc_of = |template: i32| {
        npcs.iter()
            .find(|n| n.display_id == template)
            .unwrap_or_else(|| panic!("NPC {template} in view: {npcs:?}"))
            .object_id
    };
    let (smith, keeper) = (npc_of(m6_data::SMITH_ID), npc_of(m6_data::KEEPER_ID));
    let list: l2_protocol_game::server::ItemList = all(&entered, "ITEM_LIST").remove(0);
    let object_of = |item_id: i32| {
        list.items
            .iter()
            .find(|i| i.item_id == item_id)
            .unwrap_or_else(|| panic!("item {item_id} in {list:?}"))
            .object_id
    };
    h.collect_quiet(Who::A).await;

    // ---- the smith's page: its links are what the client may send back ---------------------------
    let page = talk_to(h, smith).await;
    assert!(
        page.html.contains(&format!("npc_{smith}_Buy {}", m6_data::BUY_LIST)),
        "{}",
        page.html
    );
    assert!(page.html.contains(&format!("npc_{smith}_multisell {}", m6_data::MULTISELL_LIST)));

    // ---- the buy window: the products with their prices, the sell tab with the gems -----------------
    h.act(
        Who::A,
        act::bypass(&format!("npc_{smith}_Buy {}", m6_data::BUY_LIST)),
    )
    .await;
    let window = h.collect_quiet(Who::A).await;
    let buy = buy_lists(&window).remove(0);
    assert_eq!((buy.list_id, buy.money), (m6_data::BUY_LIST, m6_data::START_ADENA));
    assert_eq!(
        buy.products.iter().map(|p| (p.item_id, p.price)).collect::<Vec<_>>(),
        [
            (SOULSHOT_ID, m6_data::SOULSHOT_PRICE),
            (m5_data::MANA_POTION, POTION_PRICE)
        ]
    );
    let sell_tab = sell_lists(&window).remove(0);
    let gem_entry = sell_tab
        .sell_list
        .iter()
        .find(|e| e.item.item_id == m6_data::GEM)
        .expect("the gems can be sold");
    // Java `Item.getReferencePrice() / 2`: a merchant pays half the template price.
    assert_eq!(gem_entry.price, m6_data::GEM_PRICE / 2);
    assert!(
        sell_tab.sell_list.iter().all(|e| e.item.item_id != ADENA_ID),
        "adena is not for sale: {:?}",
        sell_tab.sell_list
    );

    // ---- buying ------------------------------------------------------------------------------------------
    h.act(
        Who::A,
        act::buy_items(
            m6_data::BUY_LIST,
            &[(SOULSHOT_ID, SHOTS_BOUGHT), (m5_data::MANA_POTION, POTIONS_BOUGHT)],
        ),
    )
    .await;
    let bought = h.collect_quiet(Who::A).await;
    assert!(message_ids(&bought).contains(&SystemMessageId::TheTransactionIsComplete));
    assert!(names(&bought).contains(&"INVENTORY_UPDATE".to_owned()));
    assert!(sell_lists(&bought).iter().any(|l| l.done), "the window is told it is done");

    // ---- selling: the gems go to the refund list, the adena comes ------------------------------------------
    h.act(
        Who::A,
        act::sell_items(
            m6_data::BUY_LIST,
            &[(object_of(m6_data::GEM), m6_data::GEM, GEMS_SOLD)],
        ),
    )
    .await;
    let sold = h.collect_quiet(Who::A).await;
    assert!(message_ids(&sold).contains(&SystemMessageId::TheTransactionIsComplete));
    let after_sale = sell_lists(&sold).pop().expect("the sell tab is sent again");
    assert!(after_sale.done);
    let refund = &after_sale.refund_list;
    assert_eq!(refund.len(), 1, "{refund:?}");
    assert_eq!(
        (refund[0].item.item_id, refund[0].item.count, refund[0].price),
        (m6_data::GEM, GEMS_SOLD, GEMS_SOLD * m6_data::GEM_PRICE / 2)
    );
    h.act(Who::A, act::shop_ui_close()).await;
    h.collect_quiet(Who::A).await;

    // ---- the exchange list: 5 ore and 100 adena for 2 ingots, twice --------------------------------------
    h.act(
        Who::A,
        act::bypass(&format!("npc_{smith}_multisell {}", m6_data::MULTISELL_LIST)),
    )
    .await;
    let shown = h.collect_quiet(Who::A).await;
    let exchange: MultiSellList = all(&shown, "MULTI_SELL_LIST").remove(0);
    assert_eq!((exchange.list_id, exchange.entries.len()), (m6_data::MULTISELL_LIST, 1));
    let entry = &exchange.entries[0];
    assert_eq!(
        entry.products.iter().map(|p| (p.item_id, p.count)).collect::<Vec<_>>(),
        [(m6_data::INGOT, m6_data::EXCHANGE_INGOTS)]
    );
    assert_eq!(
        entry.ingredients.iter().map(|i| (i.item_id, i.count)).collect::<Vec<_>>(),
        [
            (m6_data::ORE, m6_data::EXCHANGE_ORE),
            (ADENA_ID, m6_data::EXCHANGE_ADENA)
        ]
    );
    h.act(
        Who::A,
        act::multisell_choose(m6_data::MULTISELL_LIST, entry.entry_id, EXCHANGES),
    )
    .await;
    let exchanged = h.collect_quiet(Who::A).await;
    let messages = system_messages(&exchanged);
    // The ingredients go first, then the products come ("You have earned 4 Test Ingot(s)").
    assert_eq!(
        messages.iter().map(|m| (m.id, m.params.clone())).collect::<Vec<_>>(),
        [
            (
                SystemMessageId::S2S1HasDisappeared,
                vec![
                    SystemMessageParam::ItemName(m6_data::ORE),
                    SystemMessageParam::LongNumber(m6_data::EXCHANGE_ORE * EXCHANGES)
                ]
            ),
            (
                SystemMessageId::S2S1HasDisappeared,
                vec![
                    SystemMessageParam::ItemName(ADENA_ID),
                    SystemMessageParam::LongNumber(m6_data::EXCHANGE_ADENA * EXCHANGES)
                ]
            ),
            (
                SystemMessageId::YouHaveEarnedS2S1S,
                vec![
                    SystemMessageParam::ItemName(m6_data::INGOT),
                    SystemMessageParam::LongNumber(m6_data::EXCHANGE_INGOTS * EXCHANGES)
                ]
            ),
        ]
    );

    // ---- the warehouse: put 10 gems in (30 adena), take 4 out (free) ---------------------------------------------
    let page = talk_to(h, keeper).await;
    assert!(page.html.contains(&format!("npc_{keeper}_DepositP")), "{}", page.html);
    h.act(Who::A, act::bypass(&format!("npc_{keeper}_DepositP"))).await;
    let deposit_window = h.collect_quiet(Who::A).await;
    let deposit_list: WareHouseDepositList = all(&deposit_window, "WAREHOUSE_DEPOSIT_LIST").remove(0);
    assert_eq!(deposit_list.wh_type, l2_protocol_game::server::warehouse::WH_TYPE_PRIVATE);
    assert!(deposit_list.items.iter().any(|i| i.item_id == m6_data::GEM));
    h.act(
        Who::A,
        act::warehouse_deposit(&[(object_of(m6_data::GEM), GEMS_DEPOSITED)]),
    )
    .await;
    let deposited = h.collect_quiet(Who::A).await;
    assert!(names(&deposited).contains(&"INVENTORY_UPDATE".to_owned()), "{:?}", names(&deposited));

    h.act(Who::A, act::bypass(&format!("npc_{keeper}_WithdrawP"))).await;
    let withdraw_window = h.collect_quiet(Who::A).await;
    let withdraw_list: WareHouseWithdrawalList = all(&withdraw_window, "WAREHOUSE_WITHDRAW_LIST").remove(0);
    assert_eq!(withdraw_list.wh_type, l2_protocol_game::server::warehouse::WH_TYPE_PRIVATE);
    assert_eq!(withdraw_list.items.len(), 1, "{withdraw_list:?}");
    let stored = &withdraw_list.items[0];
    assert_eq!((stored.item_id, stored.count), (m6_data::GEM, GEMS_DEPOSITED));
    h.act(
        Who::A,
        act::warehouse_withdraw(&[(stored.object_id, GEMS_WITHDRAWN)]),
    )
    .await;
    let withdrawn = h.collect_quiet(Who::A).await;
    assert!(names(&withdrawn).contains(&"INVENTORY_UPDATE".to_owned()), "{:?}", names(&withdrawn));

    let left = h.leave(Who::A).await;
    Played { left }
}

pub fn assert_played(played: &Played) {
    let left = &played.left;
    // Adena: -200 (buy) +300 (sell) -200 (exchange) -30 (deposit fee).
    let adena = m6_data::START_ADENA
        - (SHOTS_BOUGHT * m6_data::SOULSHOT_PRICE + POTIONS_BOUGHT * POTION_PRICE)
        + GEMS_SOLD * m6_data::GEM_PRICE / 2
        - m6_data::EXCHANGE_ADENA * EXCHANGES
        - DEPOSIT_FEE;
    assert_eq!(adena, 3_999_870);
    assert_eq!(left.count_of(ADENA_ID), adena);
    assert_eq!(left.count_of(SOULSHOT_ID), m5_data::SOULSHOTS_AT_START + SHOTS_BOUGHT);
    assert_eq!(
        left.count_of(m5_data::MANA_POTION),
        m5_data::POTIONS_AT_START + POTIONS_BOUGHT
    );
    // The ore is used up and the ingots are new items (D-39: ids of the Rust range).
    assert_eq!(left.count_of(m6_data::ORE), m6_data::ORE_AT_START - m6_data::EXCHANGE_ORE * EXCHANGES);
    assert_eq!(left.count_of(m6_data::INGOT), m6_data::EXCHANGE_INGOTS * EXCHANGES);
    let ingots = left.row_of(m6_data::INGOT, "INVENTORY").expect("ingots");
    assert!(ingots.object_id >= l2_world::RUST_FIRST_OBJECT_ID);
    // Gems: 20 - 6 sold - 10 deposited + 4 taken back in the pack, 6 in the warehouse. The
    // sold gems are in no table (they sit in the refund list, which is not stored).
    let in_pack = m6_data::GEMS_AT_START - GEMS_SOLD - GEMS_DEPOSITED + GEMS_WITHDRAWN;
    assert_eq!(left.count_in(m6_data::GEM, "INVENTORY"), in_pack);
    let kept = left.row_of(m6_data::GEM, "WAREHOUSE").expect("the gems in the warehouse");
    assert_eq!(kept.count, GEMS_DEPOSITED - GEMS_WITHDRAWN);
    assert_eq!(kept.loc_data, 0);
    assert!(kept.object_id >= l2_world::RUST_FIRST_OBJECT_ID, "{}", kept.object_id);
}

#[tokio::test]
async fn a_shopping_trip_in_a_bare_world() {
    let mut world = BareWorld::start(false);
    let played = play(&mut world).await;
    assert_played(&played);
}

#[tokio::test]
async fn a_shopping_trip_on_the_real_servers() {
    let Some(mut net) = Net::start("e2em6s", false).await else {
        return;
    };
    let played = play(&mut net).await;
    assert_played(&played);
    net.finish().await;
}
