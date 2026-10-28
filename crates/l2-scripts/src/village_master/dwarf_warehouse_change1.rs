//! The 1st class transfer of dwarven fighters to scavengers (Moke, Rikadio, Ranspo, Alder).
// Java: dist/game/data/scripts/village_master/DwarfWarehouseChange1/DwarfWarehouseChange1.java

use l2_util::ObjectId;
use l2_world::subclass::CategoryType;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, Transfer, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 4] = [
    30498, // Moke
    30503, // Rikadio
    30594, // Ranspo
    32092, // Alder
];

// Items
const RING_OF_RAVEN: i32 = 1642;

// Class
const SCAVENGER: i32 = 54;
/// `PlayerClass.DWARVEN_FIGHTER`.
const DWARVEN_FIGHTER: i32 = 53;

/// The pages `onEvent` shows as they are (`30498-01.htm` ... `32092-04.htm`).
const PAGES: [i32; 4] = [1, 2, 3, 4];

/// The `else if` block of `ClassChangeRequested`.
const TRANSFERS: [Transfer; 1] = [Transfer {
    class_id: SCAVENGER,
    from: DWARVEN_FIGHTER,
    proofs: &[RING_OF_RAVEN],
    pages: [8, 9, 10, 11],
}];

/// Java `village_master.DwarfWarehouseChange1.DwarfWarehouseChange1`.
pub struct DwarfWarehouseChange1;

impl DwarfWarehouseChange1 {
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
            Some("30498-12.htm".to_owned()) // fnYouAreFourthClass
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

impl Script for DwarfWarehouseChange1 {
    fn name(&self) -> &'static str {
        "DwarfWarehouseChange1"
    }

    fn path(&self) -> &'static str {
        "village_master/DwarfWarehouseChange1"
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
            "54" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, npc?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let htmltext = if in_category(ctx, player, CategoryType::BountyHunterGroup) {
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

    const DIR: &str = "data/scripts/village_master/DwarfWarehouseChange1";

    fn setup() -> Vm {
        let mut vm = Vm::builder(
            DwarfWarehouseChange1,
            DIR,
            "DwarfWarehouseChange1",
            &[(30503, "VillageMasterDwarf")],
        );
        vm.page("30503-01.htm", &["30503-02.htm"]);
        vm.page("30503-02.htm", &["54"]);
        for n in [5, 6, 8, 9, 10, 11] {
            vm.page(&format!("30503-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_dwarven_fighter_becomes_a_scavenger() {
        let mut vm = setup();
        vm.enter(1, 19, DWARVEN_FIGHTER, vec![]);
        assert!(vm.talk(1, 30503).contains("30503-01.htm"));
        assert!(vm.click(1, "30503-02.htm").contains("30503-02.htm"));
        assert!(vm.click(1, "54").contains("30503-09.htm"));

        vm.enter(2, 20, DWARVEN_FIGHTER, vec![item(100, RING_OF_RAVEN, 1)]);
        assert!(vm.talk(2, 30503).contains("30503-01.htm"));
        vm.click(2, "30503-02.htm");
        assert!(vm.click(2, "54").contains("30503-10.htm"));
        vm.assert_class(2, SCAVENGER);
        assert_eq!(vm.t.count(2, RING_OF_RAVEN), 0);
        assert_eq!(vm.t.count(2, SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE), 15);
        // The 1st class has had its transfer.
        assert!(vm.talk(2, 30503).contains("30503-01.htm"));
        vm.click(2, "30503-02.htm");
        assert!(vm.click(2, "54").contains("30503-06.htm"));
        // A human fighter is in neither dwarven group.
        vm.enter(3, 20, 0, vec![]);
        assert!(vm.talk(3, 30503).contains("30503-05.htm"));
    }
}
