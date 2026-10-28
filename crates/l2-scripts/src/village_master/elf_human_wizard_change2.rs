//! The 2nd class transfer of human and elven wizards (Jurek, Arkenias, Valleria, Scraide,
//! Drikiyan, Valdis, Halaster, Javier).
// Java: dist/game/data/scripts/village_master/ElfHumanWizardChange2/ElfHumanWizardChange2.java

use l2_util::ObjectId;
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, Transfer, class_of, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 8] = [
    30115, // Jurek
    30174, // Arkenias
    30176, // Valleria
    30694, // Scraide
    30854, // Drikiyan
    31331, // Valdis
    31755, // Halaster
    31996, // Javier
];

// Items
const MARK_OF_SCHOLAR: i32 = 2674; // proof11x, proof12x, proof13x, proof21x, proof22x
const MARK_OF_TRUST: i32 = 2734; // proof11y, proof12y, proof13y
const MARK_OF_MAGUS: i32 = 2840; // proof11z, proof21z
const MARK_OF_WITCHCRAFT: i32 = 3307; // proof12z
const MARK_OF_SUMMONER: i32 = 3336; // proof13z, proof22z
const MARK_OF_LIFE: i32 = 3140; // proof21y, proof22y

// Classes
const SORCERER: i32 = 12;
const NECROMANCER: i32 = 13;
const WARLOCK: i32 = 14;
const SPELLSINGER: i32 = 27;
const ELEMENTAL_SUMMONER: i32 = 28;
/// `PlayerClass.WIZARD`, `ELVEN_WIZARD`.
const WIZARD: i32 = 11;
const ELVEN_WIZARD: i32 = 26;

/// The pages `onEvent` shows as they are (`30115-02.htm` ... `30115-18.htm`).
const PAGES: [i32; 17] = [2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18];
/// The NPC id the pages of every master start with.
const PREFIX: i32 = 30115;

/// The `else if` blocks of `ClassChangeRequested`.
const TRANSFERS: [Transfer; 5] = [
    Transfer {
        class_id: SORCERER,
        from: WIZARD,
        proofs: &[MARK_OF_SCHOLAR, MARK_OF_TRUST, MARK_OF_MAGUS],
        pages: [22, 23, 24, 25],
    },
    Transfer {
        class_id: NECROMANCER,
        from: WIZARD,
        proofs: &[MARK_OF_SCHOLAR, MARK_OF_TRUST, MARK_OF_WITCHCRAFT],
        pages: [26, 27, 28, 29],
    },
    Transfer {
        class_id: WARLOCK,
        from: WIZARD,
        proofs: &[MARK_OF_SCHOLAR, MARK_OF_TRUST, MARK_OF_SUMMONER],
        pages: [30, 31, 32, 33],
    },
    Transfer {
        class_id: SPELLSINGER,
        from: ELVEN_WIZARD,
        proofs: &[MARK_OF_SCHOLAR, MARK_OF_LIFE, MARK_OF_MAGUS],
        pages: [34, 35, 36, 37],
    },
    Transfer {
        class_id: ELEMENTAL_SUMMONER,
        from: ELVEN_WIZARD,
        proofs: &[MARK_OF_SCHOLAR, MARK_OF_LIFE, MARK_OF_SUMMONER],
        pages: [38, 39, 40, 41],
    },
];

/// Java `village_master.ElfHumanWizardChange2.ElfHumanWizardChange2`.
pub struct ElfHumanWizardChange2;

impl ElfHumanWizardChange2 {
    /// Java `ClassChangeRequested(player, classId)`.
    fn class_change_requested(ctx: &mut ScriptCtx<'_>, player: ObjectId, class_id: i32) -> Option<String> {
        if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some("30115-21.htm".to_owned()) // fnYouAreThirdClass
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

impl Script for ElfHumanWizardChange2 {
    fn name(&self) -> &'static str {
        "ElfHumanWizardChange2"
    }

    fn path(&self) -> &'static str {
        "village_master/ElfHumanWizardChange2"
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
            "12" | "13" | "14" | "27" | "28" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mall = in_category(ctx, player, CategoryType::HumanMallClass)
            || in_category(ctx, player, CategoryType::ElfMallClass);
        let wizard = in_category(ctx, player, CategoryType::WizardGroup);
        let htmltext = if wizard && in_category(ctx, player, CategoryType::FourthClassGroup) && mall {
            "30115-01.htm" // fnYouAreFourthClass
        } else if wizard && mall {
            match class_of(ctx, player) {
                WIZARD | SORCERER | NECROMANCER | WARLOCK => "30115-02.htm", // fnClassList1
                ELVEN_WIZARD | SPELLSINGER | ELEMENTAL_SUMMONER => "30115-12.htm", // fnClassList2
                _ => "30115-19.htm",                                         // fnYouAreFirstClass
            }
        } else {
            "30115-20.htm" // fnClassMismatch
        };
        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/ElfHumanWizardChange2";

    fn setup() -> Vm {
        let mut vm = Vm::builder(
            ElfHumanWizardChange2,
            DIR,
            "ElfHumanWizardChange2",
            &[(30694, "VillageMasterMystic")],
        );
        vm.page("30115-02.htm", &["12", "13", "14"]);
        vm.page("30115-12.htm", &["27", "28"]);
        for n in [19, 20, 21, 32, 33, 35, 36] {
            vm.page(&format!("30115-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_wizard_becomes_a_warlock_and_an_elven_wizard_waits_for_level_40() {
        let mut vm = setup();
        let marks = vec![
            item(100, MARK_OF_SCHOLAR, 1),
            item(101, MARK_OF_TRUST, 1),
            item(102, MARK_OF_SUMMONER, 1),
        ];
        vm.enter(1, 52, WIZARD, marks);
        assert!(vm.talk(1, 30694).contains("30115-02.htm"));
        assert!(vm.click(1, "14").contains("30115-32.htm"));
        vm.assert_class(1, WARLOCK);
        assert!(vm.talk(1, 30694).contains("30115-02.htm"));
        assert!(vm.click(1, "12").contains("30115-21.htm"));

        vm.enter(2, 39, ELVEN_WIZARD, vec![item(103, MARK_OF_SCHOLAR, 1)]);
        assert!(vm.talk(2, 30694).contains("30115-12.htm"));
        assert!(vm.click(2, "27").contains("30115-35.htm"));
        // A human mage (no 1st class yet) and a cleric.
        vm.enter(3, 40, 10, vec![]);
        assert!(vm.talk(3, 30694).contains("30115-19.htm"));
        vm.enter(4, 40, 15, vec![]);
        assert!(vm.talk(4, 30694).contains("30115-20.htm"));
    }
}
