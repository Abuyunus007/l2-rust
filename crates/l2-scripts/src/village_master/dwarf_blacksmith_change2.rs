//! The 2nd class transfer of artisans to warsmiths (Kusto, Flutter, Vergara, Ferris, Roman, Noel, Lombert, Newyear).
// Java: dist/game/data/scripts/village_master/DwarfBlacksmithChange2/DwarfBlacksmithChange2.java

use l2_util::ObjectId;
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, Transfer, class_of, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 8] = [
    30512, // Kusto
    30677, // Flutter
    30687, // Vergara
    30847, // Ferris
    30897, // Roman
    31272, // Noel
    31317, // Lombert
    31961, // Newyear
];

// Items
const MARK_OF_GUILDSMAN: i32 = 3119; // proof11x
const MARK_OF_PROSPERITY: i32 = 3238; // proof11y
const MARK_OF_MAESTRO: i32 = 2867; // proof11z

// Class
const WARSMITH: i32 = 57;
/// `PlayerClass.ARTISAN`.
const ARTISAN: i32 = 56;

/// The pages of the first master `onEvent` shows as they are (`30512-03.htm` ...
/// `30512-05.htm`).
const PAGES: [i32; 3] = [3, 4, 5];
/// The NPC id the pages of every master start with.
const PREFIX: i32 = 30512;

/// The `else if` block of `ClassChangeRequested`.
const TRANSFERS: [Transfer; 1] = [Transfer {
    class_id: WARSMITH,
    from: ARTISAN,
    proofs: &[MARK_OF_GUILDSMAN, MARK_OF_PROSPERITY, MARK_OF_MAESTRO],
    pages: [9, 10, 11, 12],
}];

/// Java `village_master.DwarfBlacksmithChange2.DwarfBlacksmithChange2`.
pub struct DwarfBlacksmithChange2;

impl DwarfBlacksmithChange2 {
    /// Java `ClassChangeRequested(player, classId)`.
    fn class_change_requested(ctx: &mut ScriptCtx<'_>, player: ObjectId, class_id: i32) -> Option<String> {
        if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some("30512-08.htm".to_owned()) // fnYouAreThirdClass
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

impl Script for DwarfBlacksmithChange2 {
    fn name(&self) -> &'static str {
        "DwarfBlacksmithChange2"
    }

    fn path(&self) -> &'static str {
        "village_master/DwarfBlacksmithChange2"
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
            "57" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let htmltext = if in_category(ctx, player, CategoryType::FourthClassGroup) {
            "30512-01.htm" // fnYouAreFourthClass
        } else if in_category(ctx, player, CategoryType::WarsmithGroup) {
            let player_class = class_of(ctx, player);
            if player_class == ARTISAN || player_class == WARSMITH {
                "30512-02.htm" // fnClassList1
            } else {
                "30512-06.htm" // fnYouAreFirstClass
            }
        } else {
            "30512-07.htm" // fnClassMismatch
        };
        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/DwarfBlacksmithChange2";

    fn setup() -> Vm {
        let mut vm = Vm::builder(
            DwarfBlacksmithChange2,
            DIR,
            "DwarfBlacksmithChange2",
            &[(30677, "VillageMasterDwarf")],
        );
        vm.page("30512-02.htm", &["30512-03.htm", "57"]);
        for n in [1, 3, 6, 7, 8, 9, 10, 11, 12] {
            vm.page(&format!("30512-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn an_artisan_becomes_a_warsmith_with_the_three_marks() {
        let mut vm = setup();
        let marks = |base: i32| {
            vec![
                item(base, MARK_OF_GUILDSMAN, 1),
                item(base + 1, MARK_OF_PROSPERITY, 1),
                item(base + 2, MARK_OF_MAESTRO, 1),
            ]
        };
        vm.enter(1, 39, ARTISAN, marks(100));
        assert!(vm.talk(1, 30677).contains("30512-02.htm"));
        assert!(vm.click(1, "30512-03.htm").contains("30512-03.htm"));
        assert!(vm.talk(1, 30677).contains("30512-02.htm"));
        assert!(vm.click(1, "57").contains("30512-09.htm"));

        // Two of the three marks.
        vm.enter(
            2,
            40,
            ARTISAN,
            vec![item(103, MARK_OF_GUILDSMAN, 1), item(104, MARK_OF_PROSPERITY, 1)],
        );
        assert!(vm.talk(2, 30677).contains("30512-02.htm"));
        assert!(vm.click(2, "57").contains("30512-12.htm"));

        vm.enter(3, 40, ARTISAN, marks(110));
        assert!(vm.talk(3, 30677).contains("30512-02.htm"));
        assert!(vm.click(3, "57").contains("30512-11.htm"));
        vm.assert_class(3, WARSMITH);
        for mark in [MARK_OF_GUILDSMAN, MARK_OF_PROSPERITY, MARK_OF_MAESTRO] {
            assert_eq!(vm.t.count(3, mark), 0);
        }
        assert_eq!(vm.t.count(3, SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE), 15);
        // Now a 2nd class: the link says so.
        assert!(vm.talk(3, 30677).contains("30512-02.htm"));
        assert!(vm.click(3, "57").contains("30512-08.htm"));
        // A dwarven fighter has not had the 1st transfer; a human is the wrong race.
        vm.enter(4, 40, 53, vec![]);
        assert!(vm.talk(4, 30677).contains("30512-06.htm"));
        vm.enter(5, 40, 1, vec![]);
        assert!(vm.talk(5, 30677).contains("30512-07.htm"));
    }
}
