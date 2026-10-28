//! The 2nd class transfer of scavengers to bounty hunters (Gesto, Croop, Baxt, Klump, Natools, Mona, Donal, Yasheni).
// Java: dist/game/data/scripts/village_master/DwarfWarehouseChange2/DwarfWarehouseChange2.java

use l2_util::ObjectId;
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, Transfer, class_of, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 8] = [
    30511, // Gesto
    30676, // Croop
    30685, // Baxt
    30845, // Klump
    30894, // Natools
    31269, // Mona
    31314, // Donal
    31958, // Yasheni
];

// Items
const MARK_OF_GUILDSMAN: i32 = 3119; // proof11x
const MARK_OF_PROSPERITY: i32 = 3238; // proof11y
const MARK_OF_SEARCHER: i32 = 2809; // proof11z

// Class
const BOUNTY_HUNTER: i32 = 55;
/// `PlayerClass.SCAVENGER`.
const SCAVENGER: i32 = 54;

/// The pages of the first master `onEvent` shows as they are (`30511-03.htm` ...
/// `30511-05.htm`).
const PAGES: [i32; 3] = [3, 4, 5];
/// The NPC id the pages of every master start with.
const PREFIX: i32 = 30511;

/// The `else if` block of `ClassChangeRequested`.
const TRANSFERS: [Transfer; 1] = [Transfer {
    class_id: BOUNTY_HUNTER,
    from: SCAVENGER,
    proofs: &[MARK_OF_GUILDSMAN, MARK_OF_PROSPERITY, MARK_OF_SEARCHER],
    pages: [9, 10, 11, 12],
}];

/// Java `village_master.DwarfWarehouseChange2.DwarfWarehouseChange2`.
pub struct DwarfWarehouseChange2;

impl DwarfWarehouseChange2 {
    /// Java `ClassChangeRequested(player, classId)`.
    fn class_change_requested(ctx: &mut ScriptCtx<'_>, player: ObjectId, class_id: i32) -> Option<String> {
        if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some("30511-08.htm".to_owned()) // fnYouAreThirdClass
        } else {
            transfer(
                ctx,
                player,
                PREFIX,
                &TRANSFERS,
                class_id,
                40,
                SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE,
            )
        }
    }
}

impl Script for DwarfWarehouseChange2 {
    fn name(&self) -> &'static str {
        "DwarfWarehouseChange2"
    }

    fn path(&self) -> &'static str {
        "village_master/DwarfWarehouseChange2"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPCS);
        s.add_talk_id(&NPCS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if is_listed_page(event, &[PREFIX], &PAGES) {
            return Some(event.to_owned());
        }
        match event {
            "55" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let htmltext = if in_category(ctx, player, CategoryType::FourthClassGroup)
            && in_category(ctx, player, CategoryType::BountyHunterGroup)
        {
            "30511-01.htm" // fnYouAreFourthClass
        } else if in_category(ctx, player, CategoryType::BountyHunterGroup) {
            let player_class = class_of(ctx, player);
            if player_class == SCAVENGER || player_class == BOUNTY_HUNTER {
                "30511-02.htm" // fnClassList1
            } else {
                "30511-06.htm" // fnYouAreFirstClass
            }
        } else {
            "30511-07.htm" // fnClassMismatch
        };
        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/DwarfWarehouseChange2";

    fn setup() -> Vm {
        let mut vm = Vm::new(
            DwarfWarehouseChange2,
            DIR,
            "DwarfWarehouseChange2",
            &[(30676, "VillageMasterDwarf")],
        );
        vm.page("30511-02.htm", &["30511-03.htm", "55"]);
        for n in [1, 3, 6, 7, 8, 9, 10, 11, 12] {
            vm.page(&format!("30511-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_scavenger_becomes_a_bounty_hunter_with_the_three_marks() {
        let mut vm = setup();
        let marks = |base: i32| {
            vec![
                item(base, MARK_OF_GUILDSMAN, 1),
                item(base + 1, MARK_OF_PROSPERITY, 1),
                item(base + 2, MARK_OF_SEARCHER, 1),
            ]
        };
        vm.enter(1, 39, SCAVENGER, marks(100));
        assert!(vm.talk(1, 30676).contains("30511-02.htm"));
        assert!(vm.click(1, "30511-03.htm").contains("30511-03.htm"));
        assert!(vm.talk(1, 30676).contains("30511-02.htm"));
        assert!(vm.click(1, "55").contains("30511-09.htm"));

        // Two of the three marks.
        vm.enter(
            2,
            40,
            SCAVENGER,
            vec![item(103, MARK_OF_GUILDSMAN, 1), item(104, MARK_OF_PROSPERITY, 1)],
        );
        assert!(vm.talk(2, 30676).contains("30511-02.htm"));
        assert!(vm.click(2, "55").contains("30511-12.htm"));

        vm.enter(3, 40, SCAVENGER, marks(110));
        assert!(vm.talk(3, 30676).contains("30511-02.htm"));
        assert!(vm.click(3, "55").contains("30511-11.htm"));
        vm.assert_class(3, BOUNTY_HUNTER);
        for mark in [MARK_OF_GUILDSMAN, MARK_OF_PROSPERITY, MARK_OF_SEARCHER] {
            assert_eq!(vm.t.count(3, mark), 0);
        }
        assert_eq!(vm.t.count(3, SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE), 15);
        // Now a 2nd class: the link says so.
        assert!(vm.talk(3, 30676).contains("30511-02.htm"));
        assert!(vm.click(3, "55").contains("30511-08.htm"));
        // A dwarven fighter has not had the 1st transfer; a human is the wrong race.
        vm.enter(4, 40, 53, vec![]);
        assert!(vm.talk(4, 30676).contains("30511-06.htm"));
        vm.enter(5, 40, 1, vec![]);
        assert!(vm.talk(5, 30676).contains("30511-07.htm"));
    }
}
