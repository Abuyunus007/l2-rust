//! The 2nd class transfer of orcs (Penatus, Karia, Garvarentz, Ladanza, Tushku, Aklan, Lambac,
//! Rahorakti, Shaka).
// Java: dist/game/data/scripts/village_master/OrcChange2/OrcChange2.java

use l2_util::ObjectId;
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE, Transfer, class_of, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 9] = [
    30513, // Penatus
    30681, // Karia
    30704, // Garvarentz
    30865, // Ladanza
    30913, // Tushku
    31288, // Aklan
    31326, // Lambac
    31336, // Rahorakti
    31977, // Shaka
];

// Items
const MARK_OF_CHALLENGER: i32 = 2627; // proof11x, proof21x
const MARK_OF_PILGRIM: i32 = 2721; // proof31x, proof32x
const MARK_OF_DUELIST: i32 = 2762; // proof21z
const MARK_OF_WARSPIRIT: i32 = 2879; // proof32z
const MARK_OF_GLORY: i32 = 3203; // proof11y, proof21y, proof31y, proof32y
const MARK_OF_CHAMPION: i32 = 3276; // proof11z
const MARK_OF_LORD: i32 = 3390; // proof31z

// Classes
const DESTROYER: i32 = 46;
const TYRANT: i32 = 48;
const OVERLORD: i32 = 51;
const WARCRYER: i32 = 52;
/// `PlayerClass.ORC_RAIDER`, `ORC_MONK`, `ORC_SHAMAN`.
const ORC_RAIDER: i32 = 45;
const ORC_MONK: i32 = 47;
const ORC_SHAMAN: i32 = 50;

/// The pages `onEvent` shows as they are (`30513-03.htm` ... `30513-16.htm`, without `-06`).
const PAGES: [i32; 13] = [3, 4, 5, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16];
/// The NPC id the pages of every master start with.
const PREFIX: i32 = 30513;

/// The `else if` blocks of `ClassChangeRequested`.
const TRANSFERS: [Transfer; 4] = [
    Transfer {
        class_id: DESTROYER,
        from: ORC_RAIDER,
        proofs: &[MARK_OF_CHALLENGER, MARK_OF_GLORY, MARK_OF_CHAMPION],
        pages: [20, 21, 22, 23],
    },
    Transfer {
        class_id: TYRANT,
        from: ORC_MONK,
        proofs: &[MARK_OF_CHALLENGER, MARK_OF_GLORY, MARK_OF_DUELIST],
        pages: [24, 25, 26, 27],
    },
    Transfer {
        class_id: OVERLORD,
        from: ORC_SHAMAN,
        proofs: &[MARK_OF_PILGRIM, MARK_OF_GLORY, MARK_OF_LORD],
        pages: [28, 29, 30, 31],
    },
    Transfer {
        class_id: WARCRYER,
        from: ORC_SHAMAN,
        proofs: &[MARK_OF_PILGRIM, MARK_OF_GLORY, MARK_OF_WARSPIRIT],
        pages: [32, 33, 34, 35],
    },
];

/// Java `village_master.OrcChange2.OrcChange2`.
pub struct OrcChange2;

impl OrcChange2 {
    /// Java `ClassChangeRequested(player, classId)`.
    fn class_change_requested(ctx: &mut ScriptCtx<'_>, player: ObjectId, class_id: i32) -> Option<String> {
        if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some("30513-19.htm".to_owned()) // fnYouAreThirdClass
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

impl Script for OrcChange2 {
    fn name(&self) -> &'static str {
        "OrcChange2"
    }

    fn path(&self) -> &'static str {
        "village_master/OrcChange2"
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
            "46" | "48" | "51" | "52" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let orc = in_category(ctx, player, CategoryType::OrcMallClass)
            || in_category(ctx, player, CategoryType::OrcFallClass);
        let htmltext = if in_category(ctx, player, CategoryType::FourthClassGroup) && orc {
            "30513-01.htm" // fnYouAreFourthClass
        } else if orc {
            match class_of(ctx, player) {
                ORC_RAIDER | DESTROYER => "30513-02.htm",           // fnClassList1
                ORC_MONK | TYRANT => "30513-06.htm",                // fnClassList2
                ORC_SHAMAN | OVERLORD | WARCRYER => "30513-10.htm", // fnClassList3
                _ => "30513-17.htm",                                // fnYouAreFirstClass
            }
        } else {
            "30513-18.htm" // fnClassMismatch
        };
        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/OrcChange2";

    fn setup() -> Vm {
        let mut vm = Vm::builder(OrcChange2, DIR, "OrcChange2", &[(30681, "VillageMasterOrc")]);
        vm.page("30513-02.htm", &["46"]);
        vm.page("30513-10.htm", &["51", "52"]);
        for n in [1, 17, 18, 19, 22, 31, 34] {
            vm.page(&format!("30513-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_shaman_becomes_a_warcryer_and_a_raider_a_destroyer() {
        let mut vm = setup();
        vm.enter(
            1,
            44,
            ORC_SHAMAN,
            vec![
                item(100, MARK_OF_PILGRIM, 1),
                item(101, MARK_OF_GLORY, 1),
                item(102, MARK_OF_WARSPIRIT, 1),
            ],
        );
        assert!(vm.talk(1, 30681).contains("30513-10.htm"));
        assert!(vm.click(1, "51").contains("30513-31.htm"));
        assert!(vm.talk(1, 30681).contains("30513-10.htm"));
        assert!(vm.click(1, "52").contains("30513-34.htm"));
        vm.assert_class(1, WARCRYER);

        vm.enter(
            2,
            40,
            ORC_RAIDER,
            vec![
                item(103, MARK_OF_CHALLENGER, 1),
                item(104, MARK_OF_GLORY, 1),
                item(105, MARK_OF_CHAMPION, 1),
            ],
        );
        assert!(vm.talk(2, 30681).contains("30513-02.htm"));
        assert!(vm.click(2, "46").contains("30513-22.htm"));
        vm.assert_class(2, DESTROYER);
        assert_eq!(vm.t.count(2, SHADOW_ITEM_EXCHANGE_COUPON_C_GRADE), 15);

        vm.enter(3, 40, 44, vec![]);
        assert!(vm.talk(3, 30681).contains("30513-17.htm"));
        vm.enter(4, 80, 113, vec![]);
        assert!(vm.talk(4, 30681).contains("30513-01.htm"));
        vm.enter(5, 40, 1, vec![]);
        assert!(vm.talk(5, 30681).contains("30513-18.htm"));
    }
}
