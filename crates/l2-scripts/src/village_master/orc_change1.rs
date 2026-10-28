//! The 1st class transfer of orcs (Osborn, Drikus, Castor, Finker).
// Java: dist/game/data/scripts/village_master/OrcChange1/OrcChange1.java

use l2_util::ObjectId;
use l2_world::subclass::{CategoryType, Race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{
    SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, Transfer, in_category, is_listed_page, transfer,
};

// NPCs
const NPCS: [i32; 4] = [
    30500, // Osborn
    30505, // Drikus
    30508, // Castor
    32097, // Finker
];

// Items
const MARK_OF_RAIDER: i32 = 1592;
const KHAVATARI_TOTEM: i32 = 1615;
const MASK_OF_MEDIUM: i32 = 1631;

/// `PlayerClass.ORC_FIGHTER`, `ORC_RAIDER`, `ORC_MONK`, `ORC_MAGE`, `ORC_SHAMAN`.
const ORC_FIGHTER: i32 = 44;
const ORC_RAIDER: i32 = 45;
const ORC_MONK: i32 = 47;
const ORC_MAGE: i32 = 49;
const ORC_SHAMAN: i32 = 50;

/// The pages `onEvent` shows as they are (`30500-01.htm` ... `32097-08.htm`).
const PAGES: [i32; 8] = [1, 2, 3, 4, 5, 6, 7, 8];

/// The `else if` blocks of `ClassChangeRequested`. NOTE: the "after" page comes before the
/// "no proof" page in the numbering here (fnAfterClassChange11 is 14, fnNoProof11 is 13).
const TRANSFERS: [Transfer; 3] = [
    Transfer {
        class_id: ORC_RAIDER,
        from: ORC_FIGHTER,
        proofs: &[MARK_OF_RAIDER],
        pages: [11, 12, 14, 13],
    },
    Transfer {
        class_id: ORC_MONK,
        from: ORC_FIGHTER,
        proofs: &[KHAVATARI_TOTEM],
        pages: [15, 16, 18, 17],
    },
    Transfer {
        class_id: ORC_SHAMAN,
        from: ORC_MAGE,
        proofs: &[MASK_OF_MEDIUM],
        pages: [19, 20, 22, 21],
    },
];

/// Java `village_master.OrcChange1.OrcChange1`.
pub struct OrcChange1;

impl OrcChange1 {
    /// Java `ClassChangeRequested(player, npc, classId)`.
    fn class_change_requested(
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        class_id: i32,
    ) -> Option<String> {
        if in_category(ctx, player, CategoryType::SecondClassGroup) {
            Some(format!("{}-09.htm", npc.id)) // fnYouAreSecondClass
        } else if in_category(ctx, player, CategoryType::ThirdClassGroup) {
            Some(format!("{}-10.htm", npc.id)) // fnYouAreThirdClass
        } else if in_category(ctx, player, CategoryType::FourthClassGroup) {
            Some("30500-24.htm".to_owned()) // fnYouAreFourthClass
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

impl Script for OrcChange1 {
    fn name(&self) -> &'static str {
        "OrcChange1"
    }

    fn path(&self) -> &'static str {
        "village_master/OrcChange1"
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
            "45" | "47" | "50" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, npc?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = None;
        if ctx.world().player_race(player) == Some(Race::Orc) {
            if in_category(ctx, player, CategoryType::FighterGroup) {
                htmltext = Some(format!("{}-01.htm", npc.id)); // fnClassList1
            } else if in_category(ctx, player, CategoryType::MageGroup) {
                htmltext = Some(format!("{}-06.htm", npc.id)); // fnClassList2
            }
        } else {
            htmltext = Some(format!("{}-23.htm", npc.id)); // fnClassMismatch
        }
        htmltext
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::village_master::testing::{Vm, item};

    const DIR: &str = "data/scripts/village_master/OrcChange1";

    fn setup() -> Vm {
        let mut vm = Vm::builder(OrcChange1, DIR, "OrcChange1", &[(30508, "VillageMasterOrc")]);
        vm.page("30508-01.htm", &["45", "47"]);
        vm.page("30508-06.htm", &["50"]);
        for n in [9, 11, 12, 13, 14, 18, 22, 23] {
            vm.page(&format!("30508-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn orc_fighters_and_mystics_change_class_with_their_proofs() {
        let mut vm = setup();
        vm.enter(1, 20, ORC_FIGHTER, vec![item(100, KHAVATARI_TOTEM, 1)]);
        assert!(vm.talk(1, 30508).contains("30508-01.htm"));
        assert!(vm.click(1, "45").contains("30508-13.htm"));
        assert!(vm.talk(1, 30508).contains("30508-01.htm"));
        assert!(vm.click(1, "47").contains("30508-18.htm"));
        vm.assert_class(1, ORC_MONK);

        vm.enter(2, 30, ORC_MAGE, vec![item(101, MASK_OF_MEDIUM, 1)]);
        assert!(vm.talk(2, 30508).contains("30508-06.htm"));
        assert!(vm.click(2, "50").contains("30508-22.htm"));
        vm.assert_class(2, ORC_SHAMAN);
        assert_eq!(vm.t.count(2, SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE), 15);

        vm.enter(3, 20, 31, vec![]);
        assert!(vm.talk(3, 30508).contains("30508-23.htm"));
    }
}
