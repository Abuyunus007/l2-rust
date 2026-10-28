//! The 1st class transfer of dwarven fighters to artisans (Tapoy, Mendio, Opix, Bolin).
// Java: dist/game/data/scripts/village_master/DwarfBlacksmithChange1/DwarfBlacksmithChange1.java

use l2_util::ObjectId;
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, Transfer, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 4] = [
    30499, // Tapoy
    30504, // Mendio
    30595, // Opix
    32093, // Bolin
];

// Items
const FINAL_PASS_CERTIFICATE: i32 = 1635;

// Class
const ARTISAN: i32 = 56;
/// `PlayerClass.DWARVEN_FIGHTER`.
const DWARVEN_FIGHTER: i32 = 53;

/// The pages `onEvent` shows as they are (`30499-01.htm` ... `32093-04.htm`).
const PAGES: [i32; 4] = [1, 2, 3, 4];

/// The `else if` block of `ClassChangeRequested`.
const TRANSFERS: [Transfer; 1] = [Transfer {
    class_id: ARTISAN,
    from: DWARVEN_FIGHTER,
    proofs: &[FINAL_PASS_CERTIFICATE],
    pages: [8, 9, 10, 11],
}];

/// Java `village_master.DwarfBlacksmithChange1.DwarfBlacksmithChange1`.
pub struct DwarfBlacksmithChange1;

impl DwarfBlacksmithChange1 {
    /// Java `ClassChangeRequested(player, npc, classId)`.
    fn class_change_requested(
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        class_id: i32,
    ) -> Option<String> {
        if in_category(ctx, player, CategoryType::SecondClassGroup) {
            Some(format!("{}-06.htm", npc.id)) // fnYouAreSecondClass
        } else if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some(format!("{}-07.htm", npc.id)) // fnYouAreThirdClass
        } else if in_category(ctx, player, CategoryType::FourthClassGroup) {
            Some("30499-12.htm".to_owned()) // fnYouAreFourthClass
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

impl Script for DwarfBlacksmithChange1 {
    fn name(&self) -> &'static str {
        "DwarfBlacksmithChange1"
    }

    fn path(&self) -> &'static str {
        "village_master/DwarfBlacksmithChange1"
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
            "56" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, npc?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let htmltext = if in_category(ctx, player, CategoryType::WarsmithGroup) {
            format!("{}-01.htm", npc.id) // fnClassList1
        } else {
            format!("{}-05.htm", npc.id) // fnClassMismatch
        };
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/DwarfBlacksmithChange1";

    fn setup() -> Vm {
        let mut vm = Vm::builder(
            DwarfBlacksmithChange1,
            DIR,
            "DwarfBlacksmithChange1",
            &[(30504, "VillageMasterDwarf")],
        );
        vm.page("30504-01.htm", &["30504-02.htm"]);
        vm.page("30504-02.htm", &["56"]);
        for n in [5, 6, 8, 9, 10, 11] {
            vm.page(&format!("30504-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_dwarven_fighter_becomes_a_artisan() {
        let mut vm = setup();
        vm.enter(1, 19, DWARVEN_FIGHTER, vec![]);
        assert!(vm.talk(1, 30504).contains("30504-01.htm"));
        assert!(vm.click(1, "30504-02.htm").contains("30504-02.htm"));
        assert!(vm.click(1, "56").contains("30504-09.htm"));

        vm.enter(2, 20, DWARVEN_FIGHTER, vec![item(100, FINAL_PASS_CERTIFICATE, 1)]);
        assert!(vm.talk(2, 30504).contains("30504-01.htm"));
        vm.click(2, "30504-02.htm");
        assert!(vm.click(2, "56").contains("30504-10.htm"));
        vm.assert_class(2, ARTISAN);
        assert_eq!(vm.t.count(2, FINAL_PASS_CERTIFICATE), 0);
        assert_eq!(vm.t.count(2, SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE), 15);
        // The 1st class has had its transfer.
        assert!(vm.talk(2, 30504).contains("30504-01.htm"));
        vm.click(2, "30504-02.htm");
        assert!(vm.click(2, "56").contains("30504-06.htm"));
        // A human fighter is in neither dwarven group.
        vm.enter(3, 20, 0, vec![]);
        assert!(vm.talk(3, 30504).contains("30504-05.htm"));
    }
}
