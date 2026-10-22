//! Scenario: enchanting. The player reads a scroll of weapon enchant, puts the blade into the
//! window, waits out Java's two seconds against auto-enchant programs and enchants; twice.

use l2_protocol_game::server::{
    ChooseInventoryItem, EnchantResult, ExPutEnchantTargetItemResult, InventoryUpdate, ItemList,
};

use super::*;

/// How many times the scenario enchants. The rate groups of `m6_data.rs` give 100 % up to +3.
const ENCHANTS: i64 = 2;

pub struct Played {
    left: Left,
    blade_object_id: i32,
}

pub async fn play<H: Harness>(h: &mut H) -> Played {
    let entered = h.enter(Who::A).await;
    let list: ItemList = all(&entered, "ITEM_LIST").remove(0);
    let object_of = |item_id: i32| {
        list.items
            .iter()
            .find(|i| i.item_id == item_id)
            .unwrap_or_else(|| panic!("item {item_id} in {list:?}"))
            .object_id
    };
    let (scroll, blade) = (
        object_of(m6_data::ENCHANT_SCROLL_D),
        object_of(m6_data::TEST_BLADE),
    );
    h.collect_quiet(Who::A).await;

    for round in 1..=ENCHANTS {
        // ---- the scroll opens the window -------------------------------------------------------------
        h.act(Who::A, act::use_item(scroll)).await;
        let opened = h.collect_quiet(Who::A).await;
        let choose: ChooseInventoryItem = all(&opened, "CHOOSE_INVENTORY_ITEM").remove(0);
        assert_eq!(choose.item_id, m6_data::ENCHANT_SCROLL_D);
        assert!(message_ids(&opened).contains(&SystemMessageId::SelectItemToEnchant));

        // ---- the blade goes into it; the client may only ask to enchant two seconds later -------------------
        h.act(Who::A, act::put_enchant_target(blade)).await;
        let put = h.collect_quiet(Who::A).await;
        // The answer is the object id of the accepted item.
        let accepted: ExPutEnchantTargetItemResult = all(&put, "EX_PUT_ENCHANT_TARGET_ITEM_RESULT").remove(0);
        assert_eq!(accepted.result, blade);
        h.wait_seconds(3).await;
        h.collect_quiet(Who::A).await;

        // ---- the enchant: one scroll is used up, the blade is one better ---------------------------------------
        h.act(Who::A, act::enchant_item(blade)).await;
        let done = h.collect_quiet(Who::A).await;
        let result: EnchantResult = all(&done, "ENCHANT_RESULT").remove(0);
        // 0 is `EnchantResult.SUCCESS`.
        assert_eq!(result.result, 0, "round {round}: {done:?}");
        let updates: Vec<InventoryUpdate> = all(&done, "INVENTORY_UPDATE");
        // The entries come in Java's `InventoryUpdate` hash order, which depends on the object
        // ids (the bare world and the real servers hand out different ones): compare them sorted.
        let mut changed: Vec<(i32, i32, i64, i16)> = updates
            .iter()
            .flat_map(|u| u.updates.iter())
            .map(|u| (u.item.object_id, u.item.item_id, u.item.count, u.item.enchant))
            .collect();
        changed.sort_unstable();
        let mut expected = [
            (blade, m6_data::TEST_BLADE, 1, i16::try_from(round).unwrap()),
            (
                scroll,
                m6_data::ENCHANT_SCROLL_D,
                m6_data::SCROLLS_AT_START - round,
                0,
            ),
        ];
        expected.sort_unstable();
        assert_eq!(changed, expected, "round {round}");
    }

    let left = h.leave(Who::A).await;
    Played {
        left,
        blade_object_id: blade,
    }
}

pub fn assert_played(played: &Played) {
    let left = &played.left;
    // The rows as Java writes them: the same item row, `enchant_level` raised; the stack of
    // scrolls shorter.
    let blade = left.row_of(m6_data::TEST_BLADE, "INVENTORY").expect("the blade");
    assert_eq!(blade.object_id, played.blade_object_id);
    assert_eq!(i64::from(blade.enchant_level), ENCHANTS);
    assert_eq!(
        left.count_of(m6_data::ENCHANT_SCROLL_D),
        m6_data::SCROLLS_AT_START - ENCHANTS
    );
}

#[tokio::test]
async fn enchanting_a_blade_twice_in_a_bare_world() {
    let mut world = BareWorld::start(false);
    let played = play(&mut world).await;
    assert_played(&played);
}

#[tokio::test]
async fn enchanting_a_blade_twice_on_the_real_servers() {
    let Some(mut net) = Net::start("e2em6e", false).await else {
        return;
    };
    let played = play(&mut net).await;
    assert_played(&played);
    net.finish().await;
}
