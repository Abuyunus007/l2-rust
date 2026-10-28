//! The 2nd class transfer of human clerics and elven oracles (Maximilian, Hollint, Orven,
//! Squillari, Gregory, Innocentin, Baryl).
// Java: dist/game/data/scripts/village_master/ElfHumanClericChange2/ElfHumanClericChange2.java

use l2_util::ObjectId;
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, Transfer, class_of, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 7] = [
    30120, // Maximilian
    30191, // Hollint
    30857, // Orven
    30905, // Squillari
    31279, // Gregory
    31328, // Innocentin
    31968, // Baryl
];

// Items
const MARK_OF_PILGRIM: i32 = 2721; // proof11x, proof12x, proof21x
const MARK_OF_TRUST: i32 = 2734; // proof11y, proof12y
const MARK_OF_HEALER: i32 = 2820; // proof11z, proof21z
const MARK_OF_REFORMER: i32 = 2821; // proof12z
const MARK_OF_LIFE: i32 = 3140; // proof21y

// Classes
const BISHOP: i32 = 16;
const PROPHET: i32 = 17;
const ELDER: i32 = 30;
/// `PlayerClass.CLERIC`, `ORACLE`.
const CLERIC: i32 = 15;
const ORACLE: i32 = 29;

/// The pages `onEvent` shows as they are (`30120-02.htm` ... `30120-12.htm`, without `-09`).
const PAGES: [i32; 10] = [2, 3, 4, 5, 6, 7, 8, 10, 11, 12];
/// The NPC id the pages of every master start with.
const PREFIX: i32 = 30120;

/// The `else if` blocks of `ClassChangeRequested`.
const TRANSFERS: [Transfer; 3] = [
    Transfer {
        class_id: BISHOP,
        from: CLERIC,
        proofs: &[MARK_OF_PILGRIM, MARK_OF_TRUST, MARK_OF_HEALER],
        pages: [16, 17, 18, 19],
    },
    Transfer {
        class_id: PROPHET,
        from: CLERIC,
        proofs: &[MARK_OF_PILGRIM, MARK_OF_TRUST, MARK_OF_REFORMER],
        pages: [20, 21, 22, 23],
    },
    Transfer {
        class_id: ELDER,
        from: ORACLE,
        proofs: &[MARK_OF_PILGRIM, MARK_OF_LIFE, MARK_OF_HEALER],
        pages: [24, 25, 26, 27],
    },
];

/// Java `village_master.ElfHumanClericChange2.ElfHumanClericChange2`.
pub struct ElfHumanClericChange2;

impl ElfHumanClericChange2 {
    /// Java `ClassChangeRequested(player, classId)`.
    fn class_change_requested(ctx: &mut ScriptCtx<'_>, player: ObjectId, class_id: i32) -> Option<String> {
        if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some("30120-15.htm".to_owned()) // fnYouAreThirdClass
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

impl Script for ElfHumanClericChange2 {
    fn name(&self) -> &'static str {
        "ElfHumanClericChange2"
    }

    fn path(&self) -> &'static str {
        "village_master/ElfHumanClericChange2"
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
            "16" | "17" | "30" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let call = in_category(ctx, player, CategoryType::HumanCallClass)
            || in_category(ctx, player, CategoryType::ElfCallClass);
        let cleric = in_category(ctx, player, CategoryType::ClericGroup);
        let htmltext = if cleric && in_category(ctx, player, CategoryType::FourthClassGroup) && call {
            "30120-01.htm" // fnYouAreFourthClass
        } else if cleric && call {
            match class_of(ctx, player) {
                CLERIC | BISHOP | PROPHET => "30120-02.htm", // fnClassList1
                ORACLE | ELDER => "30120-09.htm",            // fnClassList2
                _ => "30120-13.htm",                         // fnYouAreFirstClass
            }
        } else {
            "30120-14.htm" // fnClassMismatch
        };
        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/ElfHumanClericChange2";

    fn setup() -> Vm {
        let mut vm = Vm::builder(
            ElfHumanClericChange2,
            DIR,
            "ElfHumanClericChange2",
            &[(30191, "VillageMasterPriest")],
        );
        vm.page("30120-02.htm", &["16", "17"]);
        vm.page("30120-09.htm", &["30120-09.htm", "30"]);
        for n in [13, 14, 15, 18, 23, 26] {
            vm.page(&format!("30120-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_cleric_becomes_a_bishop_and_an_oracle_an_elder() {
        let mut vm = setup();
        vm.enter(
            1,
            40,
            CLERIC,
            vec![
                item(100, MARK_OF_PILGRIM, 1),
                item(101, MARK_OF_TRUST, 1),
                item(102, MARK_OF_HEALER, 1),
            ],
        );
        assert!(vm.talk(1, 30191).contains("30120-02.htm"));
        assert!(vm.click(1, "17").contains("30120-23.htm"));
        assert!(vm.talk(1, 30191).contains("30120-02.htm"));
        assert!(vm.click(1, "16").contains("30120-18.htm"));
        vm.assert_class(1, BISHOP);
        assert!(vm.talk(1, 30191).contains("30120-02.htm"));
        assert!(vm.click(1, "17").contains("30120-15.htm"));

        vm.enter(
            2,
            40,
            ORACLE,
            vec![
                item(103, MARK_OF_PILGRIM, 1),
                item(104, MARK_OF_LIFE, 1),
                item(105, MARK_OF_HEALER, 1),
            ],
        );
        assert!(vm.talk(2, 30191).contains("30120-09.htm"));
        // `30120-09.htm` is not one of the pages the event shows as it is.
        assert!(vm.click(2, "30120-09.htm").is_empty());
        assert!(vm.click(2, "30").contains("30120-26.htm"));
        vm.assert_class(2, ELDER);
        // A human mage, a wizard.
        vm.enter(3, 40, 10, vec![]);
        assert!(vm.talk(3, 30191).contains("30120-13.htm"));
        vm.enter(4, 40, 11, vec![]);
        assert!(vm.talk(4, 30191).contains("30120-14.htm"));
    }
}
