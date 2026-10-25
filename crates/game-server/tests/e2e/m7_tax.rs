//! Scenario: the castle tax in a shop. The smith of the town of the start stands in the zone the
//! price config ties to Gludio (`MerchantPriceConfig.xml`: base tax 10, castle 1), so every sale
//! costs 20 % more, and the castle's 10 % go to the treasury of its owner (the Lions). Java:
//! `MerchantPriceConfig`, `RequestBuyItem`, `Castle.addToTreasury`.
//!
//! The treasury of a castle is a global row: while the Java game server collects the real tax it
//! changes only with `SiegeDatabaseWrites = True` (D-51); the price the buyer pays is the same.

use l2_protocol_game::server::{BuyList, ItemList, NpcHtmlMessage, NpcInfo};

use super::*;

/// What the visit buys: 5 soulshots (20 adena) and 1 mana potion (50 adena).
const SHOTS: i64 = 5;
const POTIONS: i64 = 1;
const POTION_PRICE: i64 = 50;

pub fn opts(siege_writes: bool) -> Opts {
    // The buyer and the lord stand at the start, next to the smith; the GM stays out of it.
    let start = m3_data::START;
    Opts {
        siege_writes,
        positions: [
            (start.0 - 50, start.1 - 100, start.2),
            (start.0 + 50, start.1 - 100, start.2),
            m7_data::GM_POS,
        ],
        auction_ms: 86_400_000,
    }
}

pub struct Played {
    pub rows_start: Rows,
    pub rows_end: Rows,
    pub price_with_tax: i64,
    pub raider: Left,
}

/// `price * (1 + base tax + castle tax)`, cut to a whole number like Java's `(long)` cast.
fn with_tax(price: i64) -> i64 {
    let rate = 1.0 + f64::from(m7_data::BASE_TAX) / 100.0 + f64::from(m7_data::CASTLE_TAX) / 100.0;
    (price as f64 * rate) as i64
}

pub async fn play<H: Harness>(h: &mut H) -> Played {
    // The lord first: the world learns that the Lions own Gludio when he enters (D-50, D-51).
    h.enter(Who::Lord).await;
    let entered = h.enter(Who::Raider).await;
    let smith = all::<NpcInfo>(&entered, "NPC_INFO")
        .into_iter()
        .find(|n| n.display_id == m6_data::SMITH_ID)
        .expect("the smith is in view")
        .object_id;
    let _: ItemList = all(&entered, "ITEM_LIST").remove(0);
    h.collect_quiet(Who::Raider).await;
    h.collect_quiet(Who::Lord).await;
    let rows_start = h.rows().await;
    assert_eq!(rows_start.castle(m7_data::GLUDIO).treasury, 1_000);
    let origin = h.start_position(Who::Raider);

    // ---- the smith's page, then the buy window with the tax in the prices ----------------------------------
    h.act(Who::Raider, act::click(smith, origin)).await;
    h.collect_until(Who::Raider, &|body| is_named(body, "MY_TARGET_SELECTED")).await;
    h.act(Who::Raider, act::click(smith, origin)).await;
    let page = h
        .collect_until(Who::Raider, &|body| is_named(body, "NPC_HTML_MESSAGE"))
        .await;
    let page: NpcHtmlMessage = all(&page, "NPC_HTML_MESSAGE").remove(0);
    assert!(
        page.html
            .contains(&format!("npc_{smith}_Buy {}", m6_data::BUY_LIST)),
        "{}",
        page.html
    );
    h.act(
        Who::Raider,
        act::bypass(&format!("npc_{smith}_Buy {}", m6_data::BUY_LIST)),
    )
    .await;
    let window = h.collect_quiet(Who::Raider).await;
    let buy: BuyList = window
        .iter()
        .filter(|body| is_named(body, "EX_BUY_SELL_LIST"))
        .find_map(|body| BuyList::decode(&mut PacketReader::new(&body[3..])).ok())
        .expect("the buy tab");
    // `BuyList(.., taxRate)`: the base price times (1 + the town's base tax + the castle's tax).
    assert_eq!(
        buy.products.iter().map(|p| (p.item_id, p.price)).collect::<Vec<_>>(),
        [
            (SOULSHOT_ID, with_tax(m6_data::SOULSHOT_PRICE)),
            (m5_data::MANA_POTION, with_tax(POTION_PRICE))
        ]
    );
    assert_eq!(with_tax(m6_data::SOULSHOT_PRICE), 24);
    assert_eq!(with_tax(POTION_PRICE), 60);

    // ---- buying: the buyer pays the prices of the window, the castle gets its 10 % ------------------------------
    h.act(
        Who::Raider,
        act::buy_items(
            m6_data::BUY_LIST,
            &[(SOULSHOT_ID, SHOTS), (m5_data::MANA_POTION, POTIONS)],
        ),
    )
    .await;
    let bought = h.collect_quiet(Who::Raider).await;
    assert!(has_message(&bought, SystemMessageId::TheTransactionIsComplete));
    h.act(Who::Raider, act::shop_ui_close()).await;
    h.collect_quiet(Who::Raider).await;
    let price_with_tax = SHOTS * with_tax(m6_data::SOULSHOT_PRICE) + POTIONS * with_tax(POTION_PRICE);
    assert_eq!(price_with_tax, 180);
    let rows_end = h.rows().await;
    let raider = h.leave(Who::Raider).await;
    h.leave(Who::Lord).await;
    Played {
        rows_start,
        rows_end,
        price_with_tax,
        raider,
    }
}

pub fn assert_played(played: &Played, writes: bool) {
    // The buyer paid the taxed price in both modes.
    assert_eq!(
        played.raider.count_of(ADENA_ID),
        m6_data::START_ADENA - played.price_with_tax
    );
    assert_eq!(
        played.raider.count_of(SOULSHOT_ID),
        m5_data::SOULSHOTS_AT_START + SHOTS
    );
    let (start, end) = (played.rows_start.castle(m7_data::GLUDIO), played.rows_end.castle(m7_data::GLUDIO));
    if writes {
        // `Castle.addToTreasury(subTotal * castleTaxRate)`: 10 % of 180 = 18.
        assert_eq!(end.treasury, start.treasury + 18);
        // Nothing else of the castle changed.
        assert_eq!(
            (end.tax_percent, end.siege_date, end.ticket_buy_count),
            (start.tax_percent, start.siege_date, start.ticket_buy_count)
        );
    } else {
        // D-51: the Java game server collects the real tax.
        assert_eq!(played.rows_end, played.rows_start);
    }
}

#[tokio::test]
async fn the_castle_tax_is_paid_in_a_bare_world_without_global_writes() {
    let mut world = BareWorld::start(&opts(false));
    let played = play(&mut world).await;
    assert_played(&played, false);
}

#[tokio::test]
async fn the_castle_tax_is_paid_in_a_bare_world_and_the_treasury_row_is_written() {
    let mut world = BareWorld::start(&opts(true));
    let played = play(&mut world).await;
    assert_played(&played, true);
}

#[tokio::test]
async fn the_castle_tax_is_paid_on_the_real_servers_without_global_writes() {
    let options = opts(false);
    let Some(mut net) = Net::start("e2em7t0", &options).await else {
        return;
    };
    let played = play(&mut net).await;
    assert_played(&played, false);
    net.finish().await;
}

#[tokio::test]
async fn the_castle_tax_is_paid_on_the_real_servers_and_the_treasury_row_is_written() {
    let options = opts(true);
    let Some(mut net) = Net::start("e2em7t1", &options).await else {
        return;
    };
    let played = play(&mut net).await;
    assert_played(&played, true);
    net.finish().await;
}
