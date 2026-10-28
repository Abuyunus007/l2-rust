//! The 2nd class transfer of human and elven fighters (Hannavalt, Klaus, Siria, Sedrick, Marcus,
//! Bernhard, Siegmund, Hector).
// Java: dist/game/data/scripts/village_master/ElfHumanFighterChange2/ElfHumanFighterChange2.java

use l2_util::ObjectId;
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, Transfer, class_of, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 8] = [
    30109, // Hannavalt
    30187, // Klaus
    30689, // Siria
    30849, // Sedrick
    30900, // Marcus
    31276, // Bernhard
    31321, // Siegmund
    31965, // Hector
];

// Items
const MARK_OF_CHALLENGER: i32 = 2627; // proof11x, proof12x, proof42x
const MARK_OF_DUTY: i32 = 2633; // proof21x, proof22x, proof41x
const MARK_OF_SEEKER: i32 = 2673; // proof31x, proof32x, proof51x, proof52x
const MARK_OF_TRUST: i32 = 2734; // proof11y, proof12y, proof21y, proof22y, proof31y, proof32y
const MARK_OF_DUELIST: i32 = 2762; // proof11z, proof42z
const MARK_OF_SEARCHER: i32 = 2809; // proof31z, proof51z
const MARK_OF_HEALER: i32 = 2820; // proof21z, proof41z
const MARK_OF_LIFE: i32 = 3140; // proof41y, proof42y, proof51y, proof52y
const MARK_OF_CHAMPION: i32 = 3276; // proof12z
const MARK_OF_SAGITTARIUS: i32 = 3293; // proof32z, proof52z
const MARK_OF_WITCHCRAFT: i32 = 3307; // proof22z

// Classes
const GLADIATOR: i32 = 2;
const WARLORD: i32 = 3;
const PALADIN: i32 = 5;
const DARK_AVENGER: i32 = 6;
const TREASURE_HUNTER: i32 = 8;
const HAWKEYE: i32 = 9;
const TEMPLE_KNIGHT: i32 = 20;
const SWORDSINGER: i32 = 21;
const PLAINS_WALKER: i32 = 23;
const SILVER_RANGER: i32 = 24;
/// `PlayerClass.WARRIOR`, `KNIGHT`, `ROGUE`, `ELVEN_KNIGHT`, `ELVEN_SCOUT`.
const WARRIOR: i32 = 1;
const KNIGHT: i32 = 4;
const ROGUE: i32 = 7;
const ELVEN_KNIGHT: i32 = 19;
const ELVEN_SCOUT: i32 = 22;

/// The pages `onEvent` shows as they are (`30109-02.htm` ... `30109-36.htm`).
const PAGES: [i32; 35] = [
    2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20, 21, 22, 23, 24, 25, 26, 27, 28, 29,
    30, 31, 32, 33, 34, 35, 36,
];
/// The NPC id the pages of every master start with.
const PREFIX: i32 = 30109;

/// The `else if` blocks of `ClassChangeRequested`.
const TRANSFERS: [Transfer; 10] = [
    Transfer {
        class_id: GLADIATOR,
        from: WARRIOR,
        proofs: &[MARK_OF_CHALLENGER, MARK_OF_TRUST, MARK_OF_DUELIST],
        pages: [40, 41, 42, 43],
    },
    Transfer {
        class_id: WARLORD,
        from: WARRIOR,
        proofs: &[MARK_OF_CHALLENGER, MARK_OF_TRUST, MARK_OF_CHAMPION],
        pages: [44, 45, 46, 47],
    },
    Transfer {
        class_id: PALADIN,
        from: KNIGHT,
        proofs: &[MARK_OF_DUTY, MARK_OF_TRUST, MARK_OF_HEALER],
        pages: [48, 49, 50, 51],
    },
    Transfer {
        class_id: DARK_AVENGER,
        from: KNIGHT,
        proofs: &[MARK_OF_DUTY, MARK_OF_TRUST, MARK_OF_WITCHCRAFT],
        pages: [52, 53, 54, 55],
    },
    Transfer {
        class_id: TREASURE_HUNTER,
        from: ROGUE,
        proofs: &[MARK_OF_SEEKER, MARK_OF_TRUST, MARK_OF_SEARCHER],
        pages: [56, 57, 58, 59],
    },
    Transfer {
        class_id: HAWKEYE,
        from: ROGUE,
        proofs: &[MARK_OF_SEEKER, MARK_OF_TRUST, MARK_OF_SAGITTARIUS],
        pages: [60, 61, 62, 63],
    },
    Transfer {
        class_id: TEMPLE_KNIGHT,
        from: ELVEN_KNIGHT,
        proofs: &[MARK_OF_DUTY, MARK_OF_LIFE, MARK_OF_HEALER],
        pages: [64, 65, 66, 67],
    },
    Transfer {
        class_id: SWORDSINGER,
        from: ELVEN_KNIGHT,
        proofs: &[MARK_OF_CHALLENGER, MARK_OF_LIFE, MARK_OF_DUELIST],
        pages: [68, 69, 70, 71],
    },
    Transfer {
        class_id: PLAINS_WALKER,
        from: ELVEN_SCOUT,
        proofs: &[MARK_OF_SEEKER, MARK_OF_LIFE, MARK_OF_SEARCHER],
        pages: [72, 73, 74, 75],
    },
    Transfer {
        class_id: SILVER_RANGER,
        from: ELVEN_SCOUT,
        proofs: &[MARK_OF_SEEKER, MARK_OF_LIFE, MARK_OF_SAGITTARIUS],
        pages: [76, 77, 78, 79],
    },
];

/// Java `village_master.ElfHumanFighterChange2.ElfHumanFighterChange2`.
pub struct ElfHumanFighterChange2;

impl ElfHumanFighterChange2 {
    /// Java `ClassChangeRequested(player, classId)`.
    fn class_change_requested(ctx: &mut ScriptCtx<'_>, player: ObjectId, class_id: i32) -> Option<String> {
        if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some("30109-39.htm".to_owned()) // fnYouAreThirdClass
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

impl Script for ElfHumanFighterChange2 {
    fn name(&self) -> &'static str {
        "ElfHumanFighterChange2"
    }

    fn path(&self) -> &'static str {
        "village_master/ElfHumanFighterChange2"
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
            "2" | "3" | "5" | "6" | "8" | "9" | "20" | "21" | "23" | "24" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let fall = in_category(ctx, player, CategoryType::HumanFallClass)
            || in_category(ctx, player, CategoryType::ElfFallClass);
        let fighter = in_category(ctx, player, CategoryType::FighterGroup);
        let htmltext = if fighter && in_category(ctx, player, CategoryType::FourthClassGroup) && fall {
            "30109-01.htm" // fnYouAreFourthClass
        } else if fighter && fall {
            match class_of(ctx, player) {
                WARRIOR | GLADIATOR | WARLORD => "30109-02.htm", // fnClassList1
                KNIGHT | PALADIN | DARK_AVENGER => "30109-09.htm", // fnClassList2
                ROGUE | TREASURE_HUNTER | HAWKEYE => "30109-16.htm", // fnClassList3
                ELVEN_KNIGHT | TEMPLE_KNIGHT | SWORDSINGER => "30109-23.htm", // fnClassList4
                ELVEN_SCOUT | PLAINS_WALKER | SILVER_RANGER => "30109-30.htm", // fnClassList5
                _ => "30109-37.htm",                             // fnYouAreFirstClass
            }
        } else {
            "30109-38.htm" // fnClassMismatch
        };
        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/ElfHumanFighterChange2";

    fn setup() -> Vm {
        let mut vm = Vm::builder(
            ElfHumanFighterChange2,
            DIR,
            "ElfHumanFighterChange2",
            &[(30187, "VillageMasterFighter")],
        );
        vm.page("30109-02.htm", &["2", "3"]);
        vm.page("30109-23.htm", &["20", "21"]);
        for n in [1, 37, 38, 39, 40, 43, 66, 67, 71] {
            vm.page(&format!("30109-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn an_elven_knight_becomes_a_temple_knight() {
        let mut vm = setup();
        let marks = vec![
            item(100, MARK_OF_DUTY, 1),
            item(101, MARK_OF_LIFE, 1),
            item(102, MARK_OF_HEALER, 1),
        ];
        vm.enter(1, 40, ELVEN_KNIGHT, marks);
        assert!(vm.talk(1, 30187).contains("30109-23.htm"));
        assert!(vm.click(1, "21").contains("30109-71.htm"));
        assert!(vm.talk(1, 30187).contains("30109-23.htm"));
        assert!(vm.click(1, "20").contains("30109-66.htm"));
        vm.assert_class(1, TEMPLE_KNIGHT);
        assert_eq!(vm.t.count(1, SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE), 15);
        assert!(vm.talk(1, 30187).contains("30109-23.htm"));
        assert!(vm.click(1, "21").contains("30109-39.htm"));
    }

    #[test]
    fn a_low_warrior_the_first_class_the_fourth_class_and_a_mystic() {
        let mut vm = setup();
        vm.enter(
            1,
            39,
            WARRIOR,
            vec![
                item(100, MARK_OF_CHALLENGER, 1),
                item(101, MARK_OF_TRUST, 1),
                item(102, MARK_OF_DUELIST, 1),
            ],
        );
        assert!(vm.talk(1, 30187).contains("30109-02.htm"));
        assert!(vm.click(1, "2").contains("30109-40.htm"));
        vm.enter(2, 40, 0, vec![]);
        assert!(vm.talk(2, 30187).contains("30109-37.htm"));
        vm.enter(3, 80, 88, vec![]);
        assert!(vm.talk(3, 30187).contains("30109-01.htm"));
        vm.enter(4, 40, 11, vec![]);
        assert!(vm.talk(4, 30187).contains("30109-38.htm"));
    }
}
