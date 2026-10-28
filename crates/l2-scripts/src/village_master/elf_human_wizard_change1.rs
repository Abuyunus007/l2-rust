//! The 1st class transfer of human and elven mystics (Levian, Sylvain, Raymond, Marie, Celes).
// Java: dist/game/data/scripts/village_master/ElfHumanWizardChange1/ElfHumanWizardChange1.java

use l2_util::ObjectId;
use l2_world::subclass::{CategoryType, Race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, Transfer, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 5] = [
    30037, // Levian
    30070, // Sylvain
    30289, // Raymond
    32095, // Marie
    32098, // Celes
];

// Items
const MARK_OF_FAITH: i32 = 1201;
const ETERNITY_DIAMOND: i32 = 1230;
const LEAF_OF_ORACLE: i32 = 1235;
const BEAD_OF_SEASON: i32 = 1292;

// Classes
const WIZARD: i32 = 11;
const CLERIC: i32 = 15;
const ELVEN_WIZARD: i32 = 26;
const ORACLE: i32 = 29;
/// `PlayerClass.MAGE`, `ELVEN_MAGE`.
const MAGE: i32 = 10;
const ELVEN_MAGE: i32 = 25;

/// The pages `onEvent` shows as they are (`30037-01.htm` ... `32098-14.htm`).
const PAGES: [i32; 14] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14];

/// The `else if` blocks of `ClassChangeRequested`.
const TRANSFERS: [Transfer; 4] = [
    Transfer {
        class_id: WIZARD,
        from: MAGE,
        proofs: &[BEAD_OF_SEASON],
        pages: [18, 19, 20, 21],
    },
    Transfer {
        class_id: CLERIC,
        from: MAGE,
        proofs: &[MARK_OF_FAITH],
        pages: [22, 23, 24, 25],
    },
    Transfer {
        class_id: ELVEN_WIZARD,
        from: ELVEN_MAGE,
        proofs: &[ETERNITY_DIAMOND],
        pages: [26, 27, 28, 29],
    },
    Transfer {
        class_id: ORACLE,
        from: ELVEN_MAGE,
        proofs: &[LEAF_OF_ORACLE],
        pages: [30, 31, 32, 33],
    },
];

/// Java `village_master.ElfHumanWizardChange1.ElfHumanWizardChange1`.
pub struct ElfHumanWizardChange1;

impl ElfHumanWizardChange1 {
    /// Java `ClassChangeRequested(player, npc, classId)`.
    fn class_change_requested(
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        class_id: i32,
    ) -> Option<String> {
        if in_category(ctx, player, CategoryType::SecondClassGroup) {
            Some(format!("{}-16.htm", npc.id)) // fnYouAreSecondClass
        } else if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some(format!("{}-17.htm", npc.id)) // fnYouAreThirdClass
        } else if in_category(ctx, player, CategoryType::FourthClassGroup) {
            Some("30037-34.htm".to_owned()) // fnYouAreFourthClass
        } else {
            transfer(
                ctx,
                player,
                npc.id,
                &TRANSFERS,
                class_id,
                20,
                SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE,
            )
        }
    }
}

impl Script for ElfHumanWizardChange1 {
    fn name(&self) -> &'static str {
        "ElfHumanWizardChange1"
    }

    fn path(&self) -> &'static str {
        "village_master/ElfHumanWizardChange1"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPCS);
        s.add_talk_id(&NPCS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if is_listed_page(event, &NPCS, &PAGES) {
            return Some(event.to_owned());
        }
        match event {
            "11" | "15" | "26" | "29" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, npc?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let player_race = ctx.world().player_race(player);
        let htmltext = if in_category(ctx, player, CategoryType::MageGroup)
            && (player_race == Some(Race::Human) || player_race == Some(Race::Elf))
        {
            if player_race == Some(Race::Human) {
                format!("{}-01.htm", npc.id) // fnClassList1
            } else {
                format!("{}-08.htm", npc.id) // fnClassList2
            }
        } else {
            format!("{}-15.htm", npc.id) // fnClassMismatch
        };
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/ElfHumanWizardChange1";

    fn setup() -> Vm {
        let mut vm = Vm::new(
            ElfHumanWizardChange1,
            DIR,
            "ElfHumanWizardChange1",
            &[(30070, "VillageMasterMystic")],
        );
        vm.page("30070-01.htm", &["30070-02.htm", "11", "15"]);
        vm.page("30070-08.htm", &["26", "29"]);
        for n in [2, 15, 16, 18, 19, 20, 21, 24, 25, 28, 30, 32] {
            vm.page(&format!("30070-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_human_mage_becomes_a_cleric_and_an_elven_mage_an_oracle() {
        let mut vm = setup();
        vm.enter(1, 25, MAGE, vec![item(100, MARK_OF_FAITH, 1)]);
        assert!(vm.talk(1, 30070).contains("30070-01.htm"));
        // A listed page is shown as it is.
        assert!(vm.click(1, "30070-02.htm").contains("30070-02.htm"));
        assert!(vm.talk(1, 30070).contains("30070-01.htm"));
        assert!(vm.click(1, "11").contains("30070-21.htm"));
        assert!(vm.talk(1, 30070).contains("30070-01.htm"));
        assert!(vm.click(1, "15").contains("30070-24.htm"));
        vm.assert_class(1, CLERIC);
        assert_eq!(vm.t.count(1, SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE), 15);

        vm.enter(2, 20, ELVEN_MAGE, vec![item(101, LEAF_OF_ORACLE, 1)]);
        assert!(vm.talk(2, 30070).contains("30070-08.htm"));
        assert!(vm.click(2, "29").contains("30070-32.htm"));
        vm.assert_class(2, ORACLE);
        assert_eq!(vm.t.count(2, LEAF_OF_ORACLE), 0);
        // An orc mystic is sent away.
        vm.enter(3, 20, 49, vec![]);
        assert!(vm.talk(3, 30070).contains("30070-15.htm"));
    }
}
