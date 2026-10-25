//! Scenario: a clan hall auction and a hall function. The Wolves bid on the free hall `22` at the
//! auctioneer, wait for the end of the auction, win the hall and buy the HP recovery function from
//! the hall's manager. Java: `Auctioneer`, `ClanHallAuction`, `ClanHallManager`,
//! `ClanHallAuctionManager`, `AuctionableHall`.
//!
//! The bid and the function are a player's own actions and are written whatever the switch says.
//! The end of the auction (the hall changes hands, the auction and its bids are deleted) is a job:
//! its rows only with `SiegeDatabaseWrites = True` (D-51).

use l2_protocol_game::server::{NpcHtmlMessage, NpcInfo, PledgeShowInfoUpdate};

use super::*;

const WOLVES: i32 = m7_seed::WOLVES;
const HALL: i32 = m7_seed::HALL;
/// The bid: more than the starting bid of the NPC's auction.
const BID: i64 = 300_000;
/// `CH_HPREG2_FEE` (`ClanHallHpRegenerationFeeLvl2`): the fee of the 40 % level of the HP function
/// of a grade 1 hall, per day. The manager's slot table: `case 40: fee = CH_HPREG2_FEE`.
const HP_FEE: i32 = 800;
const HP_LEVEL: i32 = 40;
/// `ClanHallHpRegenerationFunctionFeeRatio`: one day.
const FUNCTION_RATE: i64 = 86_400_000;

pub fn opts(siege_writes: bool, auction_ms: i64) -> Opts {
    Opts {
        siege_writes,
        positions: [m7_data::GM_POS, m7_data::BIDDER_POS, m7_data::GM_POS],
        auction_ms,
        flag_condition: false,
    }
}

pub struct Played {
    pub rows_won: Rows,
    pub rows_end: Rows,
    pub raider: Left,
    pub won_at: i64,
}

/// Clicks `npc` twice (select, then talk) and returns the page it shows.
async fn talk_to<H: Harness>(h: &mut H, who: Who, npc: i32) -> NpcHtmlMessage {
    let origin = h.start_position(who);
    h.act(who, act::click(npc, origin)).await;
    h.collect_until(who, &|body| is_named(body, "MY_TARGET_SELECTED"))
        .await;
    h.act(who, act::click(npc, origin)).await;
    let page = h
        .collect_until(who, &|body| is_named(body, "NPC_HTML_MESSAGE"))
        .await;
    all(&page, "NPC_HTML_MESSAGE").remove(0)
}

/// Sends a command the page offered and returns the page that answers.
async fn follow<H: Harness>(h: &mut H, who: Who, page: &NpcHtmlMessage, command: &str) -> NpcHtmlMessage {
    assert!(
        page.html.contains(command),
        "the page does not offer `{command}`: {}",
        page.html
    );
    h.act(who, act::bypass(command)).await;
    let answer = h
        .collect_until(who, &|body| is_named(body, "NPC_HTML_MESSAGE"))
        .await;
    all(&answer, "NPC_HTML_MESSAGE").remove(0)
}

pub async fn play<H: Harness>(h: &mut H, writes: bool) -> Played {
    // The clan and its warehouse come from the tables at CharacterSelect (D-50).
    let entered = h.enter(Who::Raider).await;
    let npcs = all::<NpcInfo>(&entered, "NPC_INFO");
    let npc_of = |template: i32| {
        npcs.iter()
            .find(|n| n.display_id == template)
            .unwrap_or_else(|| panic!("NPC {template} in view: {npcs:?}"))
            .object_id
    };
    let (auctioneer, manager) = (npc_of(m7_data::AUCTIONEER_ID), npc_of(m7_data::HALL_MANAGER_ID));
    h.collect_quiet(Who::Raider).await;
    let rows_start = h.rows().await;
    assert_eq!(rows_start.hall(HALL).owner_id, 0);
    assert_eq!(rows_start.adena_of(WOLVES), m7_seed::WOLVES_WAREHOUSE_ADENA);
    let auction_end = rows_start.auctions[0].end_date;

    // ---- the manager serves only the owner: the Wolves are a clan, but not the hall's owner ---------------------
    let page = talk_to(h, Who::Raider, manager).await;
    assert!(page.html.contains("I serve another clan."), "{}", page.html);

    // ---- the auctioneer's pages, each offering the next command ---------------------------------------------------
    let page = talk_to(h, Who::Raider, auctioneer).await;
    let list = follow(h, Who::Raider, &page, &format!("npc_{auctioneer}_list")).await;
    // The list names the auction: the hall, its bidding link.
    assert!(list.html.contains(HALL_NAME_TEXT), "{}", list.html);
    let info = follow(h, Who::Raider, &list, &format!("npc_{auctioneer}_bidding {HALL}")).await;
    assert!(info.html.contains(HALL_NAME_TEXT), "{}", info.html);
    assert!(
        info.html
            .contains(&format!("minimum bid {}", m7_seed::HALL_STARTING_BID)),
        "{}",
        info.html
    );
    let bid_page = follow(h, Who::Raider, &info, &format!("npc_{auctioneer}_bid1 {HALL}")).await;
    assert!(
        bid_page
            .html
            .contains(&format!("Clan adena {}", m7_seed::WOLVES_WAREHOUSE_ADENA)),
        "{}",
        bid_page.html
    );
    // The edit field's `$bid` is what the player typed.
    assert!(
        bid_page
            .html
            .contains(&format!("npc_{auctioneer}_bid {HALL} $bid")),
        "{}",
        bid_page.html
    );

    // ---- a bid below the starting bid is refused, then the real one is placed ---------------------------------------
    h.act(
        Who::Raider,
        act::bypass(&format!("npc_{auctioneer}_bid {HALL} 1000")),
    )
    .await;
    let refused = h.collect_quiet(Who::Raider).await;
    assert!(has_message(
        &refused,
        SystemMessageId::YourBidPriceMustBeHigherThanTheMinimumPriceCurrentlyBeingBid
    ));
    h.act(
        Who::Raider,
        act::bypass(&format!("npc_{auctioneer}_bid {HALL} {BID}")),
    )
    .await;
    let placed = h.collect_quiet(Who::Raider).await;
    assert!(has_message(
        &placed,
        SystemMessageId::YourBidHasBeenSuccessfullyPlaced
    ));
    // The adena of the bid comes out of the clan warehouse (read again first, D-50); the bid and
    // the clan's mark are the player's own rows.
    let rows_bid = h.rows().await;
    assert_eq!(rows_bid.adena_of(WOLVES), m7_seed::WOLVES_WAREHOUSE_ADENA - BID);
    assert_eq!(
        rows_bid.bids,
        [m7_db::BidRow {
            auction_id: HALL,
            bidder_id: WOLVES,
            bidder_name: Seed::name_of(Who::Raider).to_owned(),
            clan_name: m7_seed::WOLVES_NAME.to_owned(),
            max_bid: BID,
        }]
    );
    assert_eq!(rows_bid.clan(WOLVES).auction_bid_at, HALL);
    assert_eq!(rows_bid.halls, rows_start.halls);

    // ---- the auction ends: the highest bid wins the hall --------------------------------------------------------------
    h.wait_until(auction_end).await;
    let won = h.collect_quiet(Who::Raider).await;
    let won_at = now_ms();
    let clan_info: PledgeShowInfoUpdate = all(&won, "PLEDGE_SHOW_INFO_UPDATE")
        .pop()
        .expect("the clan window");
    assert_eq!(clan_info.hideout_id, HALL, "the clan owns the hall in this world");
    let rows_won = h.rows().await;
    if writes {
        // `AuctionableHall.setOwner` -> `updateDb`, `deleteAuctionFromDB`, `removeBids`,
        // `setAuctionBiddedAt(0)`.
        let hall = rows_won.hall(HALL);
        assert_eq!(hall.owner_id, WOLVES);
        assert!(hall.paid_until > won_at, "the rent is paid for a week: {hall:?}");
        assert!(rows_won.auctions.is_empty() && rows_won.bids.is_empty());
        assert_eq!(rows_won.clan(WOLVES).auction_bid_at, 0);
        // The bid is the price; the rent task of the new owner takes the first week's lease from
        // the clan warehouse at once (`AuctionableHall.FeeTask`).
        assert_eq!(
            rows_won.adena_of(WOLVES),
            m7_seed::WOLVES_WAREHOUSE_ADENA - BID - i64::from(m7_seed::HALL_LEASE)
        );
        assert!(hall.paid, "{hall:?}");
        // `_paidUntil = now + _chRate` (a week).
        assert!(
            (hall.paid_until - (won_at + 7 * m7_data::DAY_MS)).abs() < 60_000,
            "{hall:?}"
        );
    } else {
        // D-51: nothing of the end is written; the bid of the player stays as it was.
        assert_eq!(rows_won, rows_bid);
    }

    // ---- the manager serves the owner: the hall function ------------------------------------------------------------------
    let page = talk_to(h, Who::Raider, manager).await;
    let overview = follow(h, Who::Raider, &page, &format!("npc_{manager}_manage recovery")).await;
    assert!(
        overview
            .html
            .contains(&format!("npc_{manager}_manage recovery edit_hp {HP_LEVEL}")),
        "{}",
        overview.html
    );
    let apply = follow(
        h,
        Who::Raider,
        &overview,
        &format!("npc_{manager}_manage recovery edit_hp {HP_LEVEL}"),
    )
    .await;
    assert!(
        apply.html.contains(&format!("{HP_FEE}")),
        "the page names the fee: {}",
        apply.html
    );
    let done = follow(
        h,
        Who::Raider,
        &apply,
        &format!("npc_{manager}_manage recovery hp {HP_LEVEL}"),
    )
    .await;
    assert!(done.html.contains("The function is done."), "{}", done.html);
    let rows_end = h.rows().await;
    let raider = h.leave(Who::Raider).await;
    Played {
        rows_won,
        rows_end,
        raider,
        won_at,
    }
}

/// The text of the auction pages that names the hall.
const HALL_NAME_TEXT: &str = m7_seed::HALL_NAME;

pub fn assert_played(played: &Played, writes: bool) {
    // `CH_HPREG*` fee: the owner paid one day from his own adena (an item of the player).
    assert_eq!(
        played.raider.count_of(ADENA_ID),
        m6_data::START_ADENA - i64::from(HP_FEE),
        "the function is paid from the leader's adena"
    );
    // `ClanHallFunction.dbSave`: `REPLACE INTO clanhall_functions`, a player's write.
    let functions = &played.rows_end.functions;
    assert_eq!(functions.len(), 1, "{functions:?}");
    let function = &functions[0];
    assert_eq!(
        (
            function.table,
            function.residence_id,
            function.function_type,
            function.lvl,
            function.lease,
            function.rate
        ),
        (
            "clanhall",
            HALL,
            l2_world::residence::HALL_FUNC_RESTORE_HP,
            HP_LEVEL,
            HP_FEE,
            FUNCTION_RATE
        )
    );
    assert!(
        function.end_time > played.won_at,
        "the next fee is due later: {function:?}"
    );
    // The rows of the auction and the hall are in the state the end of the auction left them.
    assert_eq!(played.rows_end.halls, played.rows_won.halls);
    assert_eq!(played.rows_end.bids, played.rows_won.bids);
    if !writes {
        // NOTE: the function row is a player's write and goes out; the owner row of the hall is a
        // job's and does not, so the database holds a function of a hall it thinks is free (the
        // first owner Sieghardt gives the hall would find it, `ClanHall.load` reads the functions
        // of an owned hall). Reported as a finding of M7-E2E-01.
        assert_eq!(played.rows_end.hall(HALL).owner_id, 0);
    }
}

/// D-50: the adena of a bid comes out of the clan warehouse as the database has it *now*. The Java
/// game server took most of it out; the world, which remembers 5,000,000, reads the stack again,
/// finds 100,000 and refuses the bid. When the adena is back the same bid is accepted.
pub async fn play_sieghardt_took_the_adena<H: Harness>(h: &mut H) {
    let entered = h.enter(Who::Raider).await;
    let auctioneer = all::<NpcInfo>(&entered, "NPC_INFO")
        .into_iter()
        .find(|n| n.display_id == m7_data::AUCTIONEER_ID)
        .expect("the auctioneer is in view")
        .object_id;
    h.collect_quiet(Who::Raider).await;
    let page = talk_to(h, Who::Raider, auctioneer).await;
    let list = follow(h, Who::Raider, &page, &format!("npc_{auctioneer}_list")).await;
    let info = follow(h, Who::Raider, &list, &format!("npc_{auctioneer}_bidding {HALL}")).await;
    let bid_page = follow(h, Who::Raider, &info, &format!("npc_{auctioneer}_bid1 {HALL}")).await;
    let bid = format!("npc_{auctioneer}_bid {HALL} {BID}");
    assert!(
        bid_page
            .html
            .contains(&format!("npc_{auctioneer}_bid {HALL} $bid"))
    );

    h.sieghardt(Sieghardt::WolvesAdenaLeft { count: 100_000 }).await;
    h.act(Who::Raider, act::bypass(&bid)).await;
    let refused = h.collect_quiet(Who::Raider).await;
    assert!(has_message(
        &refused,
        SystemMessageId::ThereIsNotEnoughAdenaInTheClanHallWarehouse
    ));
    assert!(!has_message(
        &refused,
        SystemMessageId::YourBidHasBeenSuccessfullyPlaced
    ));
    let rows = h.rows().await;
    assert!(rows.bids.is_empty());
    assert_eq!(rows.adena_of(WOLVES), 100_000, "what Sieghardt left");
    assert_eq!(rows.clan(WOLVES).auction_bid_at, 0);

    h.sieghardt(Sieghardt::WolvesAdenaLeft {
        count: m7_seed::WOLVES_WAREHOUSE_ADENA,
    })
    .await;
    h.act(Who::Raider, act::bypass(&bid)).await;
    let placed = h.collect_quiet(Who::Raider).await;
    assert!(has_message(
        &placed,
        SystemMessageId::YourBidHasBeenSuccessfullyPlaced
    ));
    let rows = h.rows().await;
    assert_eq!(rows.bids.len(), 1);
    assert_eq!(rows.adena_of(WOLVES), m7_seed::WOLVES_WAREHOUSE_ADENA - BID);
    h.leave(Who::Raider).await;
}

#[tokio::test]
async fn a_bid_reads_the_clan_warehouse_again_in_a_bare_world() {
    let mut world = BareWorld::start(&opts(true, 600_000));
    play_sieghardt_took_the_adena(&mut world).await;
}

#[tokio::test]
async fn a_bid_reads_the_clan_warehouse_again_on_the_real_servers() {
    let options = opts(true, 600_000);
    let Some(mut net) = Net::start("e2em7h2", &options).await else {
        return;
    };
    play_sieghardt_took_the_adena(&mut net).await;
    net.finish().await;
}

#[tokio::test]
async fn a_hall_auction_and_function_run_in_a_bare_world_without_global_writes() {
    let mut world = BareWorld::start(&opts(false, 6_000));
    let played = play(&mut world, false).await;
    assert_played(&played, false);
}

#[tokio::test]
async fn a_hall_auction_and_function_run_in_a_bare_world_and_write_javas_rows() {
    let mut world = BareWorld::start(&opts(true, 6_000));
    let played = play(&mut world, true).await;
    assert_played(&played, true);
}

#[tokio::test]
async fn a_hall_auction_and_function_run_on_the_real_servers_without_global_writes() {
    let options = opts(false, 90_000);
    let Some(mut net) = Net::start("e2em7h0", &options).await else {
        return;
    };
    let played = play(&mut net, false).await;
    assert_played(&played, false);
    net.finish().await;
}

#[tokio::test]
async fn a_hall_auction_and_function_run_on_the_real_servers_and_write_javas_rows() {
    let options = opts(true, 90_000);
    let Some(mut net) = Net::start("e2em7h1", &options).await else {
        return;
    };
    let played = play(&mut net, true).await;
    assert_played(&played, true);
    net.finish().await;
}
