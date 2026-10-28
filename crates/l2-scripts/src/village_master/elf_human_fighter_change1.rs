//! The 1st class transfer of human and elven fighters (Pabris, Rains, Ramos, Schule).
// Java: dist/game/data/scripts/village_master/ElfHumanFighterChange1/ElfHumanFighterChange1.java

use l2_util::ObjectId;
use l2_world::subclass::{CategoryType, Race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, Transfer, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 4] = [
    30066, // Pabris
    30288, // Rains
    30373, // Ramos
    32094, // Schule
];

// Items
const MEDALLION_OF_WARRIOR: i32 = 1145;
const SWORD_OF_RITUAL: i32 = 1161;
const BEZIQUES_RECOMMENDATION: i32 = 1190;
const ELVEN_KNIGHT_BROOCH: i32 = 1204;
const REISAS_RECOMMENDATION: i32 = 1217;

// Classes
const WARRIOR: i32 = 1;
const KNIGHT: i32 = 4;
const ROGUE: i32 = 7;
const ELVEN_KNIGHT: i32 = 19;
const ELVEN_SCOUT: i32 = 22;
/// `PlayerClass.FIGHTER`, `ELVEN_FIGHTER`.
const FIGHTER: i32 = 0;
const ELVEN_FIGHTER: i32 = 18;

/// The pages `onEvent` shows as they are (`30066-01.htm` ... `32094-17.htm`).
const PAGES: [i32; 17] = [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17];

/// The `else if` blocks of `ClassChangeRequested`.
const TRANSFERS: [Transfer; 5] = [
    Transfer {
        class_id: WARRIOR,
        from: FIGHTER,
        proofs: &[MEDALLION_OF_WARRIOR],
        pages: [21, 22, 23, 24],
    },
    Transfer {
        class_id: KNIGHT,
        from: FIGHTER,
        proofs: &[SWORD_OF_RITUAL],
        pages: [25, 26, 27, 28],
    },
    Transfer {
        class_id: ROGUE,
        from: FIGHTER,
        proofs: &[BEZIQUES_RECOMMENDATION],
        pages: [29, 30, 31, 32],
    },
    Transfer {
        class_id: ELVEN_KNIGHT,
        from: ELVEN_FIGHTER,
        proofs: &[ELVEN_KNIGHT_BROOCH],
        pages: [33, 34, 35, 36],
    },
    Transfer {
        class_id: ELVEN_SCOUT,
        from: ELVEN_FIGHTER,
        proofs: &[REISAS_RECOMMENDATION],
        pages: [37, 38, 39, 40],
    },
];

/// Java `village_master.ElfHumanFighterChange1.ElfHumanFighterChange1`.
pub struct ElfHumanFighterChange1;

impl ElfHumanFighterChange1 {
    /// Java `ClassChangeRequested(player, npc, classId)`.
    fn class_change_requested(
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        class_id: i32,
    ) -> Option<String> {
        if in_category(ctx, player, CategoryType::SecondClassGroup) {
            Some(format!("{}-19.htm", npc.id)) // fnYouAreSecondClass
        } else if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some(format!("{}-20.htm", npc.id)) // fnYouAreThirdClass
        } else if in_category(ctx, player, CategoryType::FourthClassGroup) {
            Some("30066-41.htm".to_owned()) // fnYouAreFourthClass
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

impl Script for ElfHumanFighterChange1 {
    fn name(&self) -> &'static str {
        "ElfHumanFighterChange1"
    }

    fn path(&self) -> &'static str {
        "village_master/ElfHumanFighterChange1"
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
            "1" | "4" | "7" | "19" | "22" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, npc?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let player_race = ctx.world().player_race(player);
        let htmltext = if in_category(ctx, player, CategoryType::FighterGroup)
            && (player_race == Some(Race::Human) || player_race == Some(Race::Elf))
        {
            if player_race == Some(Race::Human) {
                format!("{}-01.htm", npc.id) // fnClassList1
            } else {
                format!("{}-11.htm", npc.id) // fnClassList2
            }
        } else {
            format!("{}-18.htm", npc.id) // fnClassMismatch
        };
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/ElfHumanFighterChange1";

    fn setup() -> Vm {
        let mut vm = Vm::new(
            ElfHumanFighterChange1,
            DIR,
            "ElfHumanFighterChange1",
            &[(30066, "VillageMasterFighter")],
        );
        vm.page("30066-01.htm", &["1", "4", "7"]);
        vm.page("30066-11.htm", &["19", "22"]);
        for n in [
            18, 19, 20, 21, 22, 23, 24, 25, 27, 28, 33, 35, 36, 37, 38, 39, 40, 41,
        ] {
            vm.page(&format!("30066-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_human_fighter_becomes_a_warrior_with_the_medallion() {
        let mut vm = setup();
        vm.enter(1, 20, FIGHTER, vec![item(100, MEDALLION_OF_WARRIOR, 1)]);
        assert!(vm.talk(1, 30066).contains("30066-01.htm"));
        let out = vm.click(1, "1");
        assert!(out.contains("30066-23.htm"));
        vm.assert_class(1, WARRIOR);
        assert_eq!(vm.t.count(1, MEDALLION_OF_WARRIOR), 0);
        assert_eq!(vm.t.count(1, SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE), 15);
        // Now a 1st class: the same link says so.
        assert!(vm.talk(1, 30066).contains("30066-01.htm"));
        assert!(vm.click(1, "4").contains("30066-19.htm"));
    }

    #[test]
    fn an_elven_fighter_becomes_a_scout_and_the_low_and_proofless_pages() {
        let mut vm = setup();
        // Level 19 with the recommendation: come back at 20.
        vm.enter(1, 19, ELVEN_FIGHTER, vec![item(100, REISAS_RECOMMENDATION, 1)]);
        assert!(vm.talk(1, 30066).contains("30066-11.htm"));
        assert!(vm.click(1, "22").contains("30066-37.htm"));
        // Without the brooch at 20: no proof.
        vm.enter(2, 20, ELVEN_FIGHTER, vec![item(101, REISAS_RECOMMENDATION, 1)]);
        assert!(vm.talk(2, 30066).contains("30066-11.htm"));
        assert!(vm.click(2, "19").contains("30066-36.htm"));
        assert!(vm.talk(2, 30066).contains("30066-11.htm"));
        assert!(vm.click(2, "22").contains("30066-39.htm"));
        vm.assert_class(2, ELVEN_SCOUT);
        // A mage is the wrong student.
        vm.enter(3, 20, 10, vec![]);
        assert!(vm.talk(3, 30066).contains("30066-18.htm"));
    }
}
