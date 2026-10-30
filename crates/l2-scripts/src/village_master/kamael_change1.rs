//! The 1st class transfer of the Kamael (Hanarin, Yeniche, Gershwin, Holst, Khadava): the
//! trooper and warder quests stand in for the proofs.
// Java: dist/game/data/scripts/village_master/KamaelChange1/KamaelChange1.java

use l2_util::ObjectId;
use l2_world::subclass::{CategoryType, Race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::class_change::{SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, class_of, in_category, is_listed_page};

// NPCs
const NPCS: [i32; 5] = [
    32191, // Hanarin
    32193, // Yeniche
    32196, // Gershwin
    32199, // Holst
    32202, // Khadava
];

// Items
const GWAINS_RECOMMENDATION: i32 = 9753;
const STEELRAZOR_EVALUATION: i32 = 9772;

/// `PlayerClass.MALE_SOLDIER`, `FEMALE_SOLDIER`, `TROOPER`, `WARDER`.
const MALE_SOLDIER: i32 = 123;
const FEMALE_SOLDIER: i32 = 124;
const TROOPER: i32 = 125;
const WARDER: i32 = 126;

/// Java `Q00062_PathOfTheTrooper.class.getSimpleName()`, `Q00063_PathOfTheWarder`.
const PATH_OF_THE_TROOPER: &str = "Q00062_PathOfTheTrooper";
const PATH_OF_THE_WARDER: &str = "Q00063_PathOfTheWarder";

/// The pages `onEvent` shows as they are (`32191-02.htm` ... `32191-09.htm`).
const PAGES: [i32; 8] = [2, 3, 4, 5, 6, 7, 8, 9];

/// Java `village_master.KamaelChange1.KamaelChange1`.
pub struct KamaelChange1;

/// `(qs != null) && qs.isCompleted()` of `player.getQuestState(name)`.
fn completed(ctx: &ScriptCtx<'_>, player: ObjectId, name: &str) -> bool {
    ctx.player_quest_state(player, name)
        .is_some_and(|qs| qs.is_completed())
}

impl KamaelChange1 {
    /// Java `ClassChangeRequested(player, classId)`.
    fn class_change_requested(ctx: &mut ScriptCtx<'_>, player: ObjectId, class_id: i32) -> Option<String> {
        let mut htmltext = None;
        if ctx
            .world()
            .id_in_category(CategoryType::KamaelSecondClassGroup, class_id)
        {
            let player_class = class_of(ctx, player);
            if in_category(ctx, player, CategoryType::KamaelSecondClassGroup) {
                htmltext = Some("32191-10.htm"); // master_all_kamael004a
            } else if in_category(ctx, player, CategoryType::KamaelThirdClassGroup) {
                htmltext = Some("32191-11.htm"); // master_all_kamael005a
            } else if in_category(ctx, player, CategoryType::KamaelFourthClassGroup) {
                htmltext = Some("32191-12.htm"); // master_all_kamael100a
            } else if class_id == TROOPER && player_class == MALE_SOLDIER {
                let done = completed(ctx, player, PATH_OF_THE_TROOPER);
                if ctx.player_level(player) < 20 {
                    if done {
                        htmltext = Some("32191-13.htm"); // master_all_kamael009ma
                    } else {
                        htmltext = Some("32191-14.htm"); // master_all_kamael008ma
                    }
                } else if !done {
                    htmltext = Some("32191-15.htm"); // master_all_kamael010ma
                } else {
                    change(ctx, player, GWAINS_RECOMMENDATION, TROOPER);
                    htmltext = Some("32191-16.htm"); // master_all_kamael011ma
                }
            } else if class_id == WARDER && player_class == FEMALE_SOLDIER {
                let done = completed(ctx, player, PATH_OF_THE_WARDER);
                if ctx.player_level(player) < 20 {
                    if done {
                        htmltext = Some("32191-17.htm"); // master_all_kamael008fa
                    } else {
                        htmltext = Some("32191-18.htm"); // master_all_kamael009fa
                    }
                } else if !done {
                    htmltext = Some("32191-19.htm"); // master_all_kamael010fa
                } else {
                    change(ctx, player, STEELRAZOR_EVALUATION, WARDER);
                    htmltext = Some("32191-20.htm"); // master_all_kamael011fa
                }
            }
        }
        htmltext.map(str::to_owned)
    }
}

/// `takeItems(player, item, -1)`, the class, `broadcastUserInfo`, the coupons ("SystemMessage and
/// cast skill is done by setClassId").
fn change(ctx: &mut ScriptCtx<'_>, player: ObjectId, item: i32, class_id: i32) {
    ctx.take_items(player, item, -1);
    ctx.world_mut().set_player_class(player, class_id);
    ctx.world_mut().set_base_class(player, class_id);
    ctx.world_mut().broadcast_user_info(player);
    ctx.give_items(player, SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE, 15);
}

impl Script for KamaelChange1 {
    fn name(&self) -> &'static str {
        "KamaelChange1"
    }

    fn path(&self) -> &'static str {
        "village_master/KamaelChange1"
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
        if is_listed_page(event, &[32191], &PAGES) {
            return Some(event.to_owned());
        }
        match event {
            "125" | "126" => {
                let class_id = event.parse().ok()?;
                Self::class_change_requested(ctx, player?, class_id)
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = None;
        if ctx.world().player_race(player) != Some(Race::Kamael) {
            htmltext = Some("32191-01.htm"); // master_all_kamael002a
        } else if in_category(ctx, player, CategoryType::KamaelFirstClassGroup) {
            let player_class = class_of(ctx, player);
            if player_class == MALE_SOLDIER {
                htmltext = Some("32191-02.htm"); // master_all_kamael003m
            } else if player_class == FEMALE_SOLDIER {
                htmltext = Some("32191-06.htm"); // master_all_kamael003f
            }
        } else if in_category(ctx, player, CategoryType::KamaelSecondClassGroup) {
            htmltext = Some("32191-10.htm"); // master_all_kamael004a
        } else if in_category(ctx, player, CategoryType::KamaelThirdClassGroup) {
            htmltext = Some("32191-11.htm"); // master_all_kamael005a
        } else {
            htmltext = Some("32191-12.htm"); // master_all_kamael100a
        }
        htmltext.map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use l2_world::QuestLoad;

    use super::*;
    use crate::village_master::testing::{Vm, character, item};

    const DIR: &str = "data/scripts/village_master/KamaelChange1";

    /// The path quests as far as this script needs them: quests of those names.
    struct PathQuest(&'static str, i32);

    impl Script for PathQuest {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests/Path"
        }
        fn id(&self) -> i32 {
            self.1
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> Vm {
        let mut vm = Vm::builder(
            KamaelChange1,
            DIR,
            "KamaelChange1",
            &[(32193, "VillageMasterKamael")],
        );
        vm.also(PathQuest(PATH_OF_THE_TROOPER, 62));
        vm.also(PathQuest(PATH_OF_THE_WARDER, 63));
        vm.page("32191-02.htm", &["32191-03.htm", "125"]);
        vm.page("32191-06.htm", &["126"]);
        for n in [1, 3, 10, 12, 13, 14, 15, 16, 17, 20] {
            vm.page(&format!("32191-{n:02}.htm"), &[]);
        }
        vm.build()
    }

    #[test]
    fn a_soldier_with_the_trooper_quest_becomes_a_trooper() {
        let mut vm = setup();
        vm.enter(1, 20, MALE_SOLDIER, vec![]);
        assert!(vm.talk(1, 32193).contains("32191-02.htm"));
        assert!(vm.click(1, "32191-03.htm").contains("32191-03.htm"));
        assert!(vm.talk(1, 32193).contains("32191-02.htm"));
        assert!(vm.click(1, "125").contains("32191-15.htm"));

        let mut data = character(2, 20, MALE_SOLDIER, vec![item(100, GWAINS_RECOMMENDATION, 1)]);
        data.profile.quests = QuestLoad {
            states: vec![(PATH_OF_THE_TROOPER.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        vm.t.enter_with(data);
        vm.t.take();
        assert!(vm.talk(2, 32193).contains("32191-02.htm"));
        assert!(vm.click(2, "125").contains("32191-16.htm"));
        vm.assert_class(2, TROOPER);
        assert_eq!(vm.t.count(2, GWAINS_RECOMMENDATION), 0);
        assert_eq!(vm.t.count(2, SHADOW_ITEM_EXCHANGE_COUPON_D_GRADE), 15);
        assert!(vm.talk(2, 32193).contains("32191-10.htm"));
    }

    #[test]
    fn a_young_female_soldier_and_other_races() {
        let mut vm = setup();
        let mut data = character(1, 19, FEMALE_SOLDIER, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(PATH_OF_THE_WARDER.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        vm.t.enter_with(data);
        vm.t.take();
        assert!(vm.talk(1, 32193).contains("32191-06.htm"));
        // Too young, with the path done.
        assert!(vm.click(1, "126").contains("32191-17.htm"));
        vm.assert_class(1, FEMALE_SOLDIER);
        vm.enter(2, 20, 0, vec![]);
        assert!(vm.talk(2, 32193).contains("32191-01.htm"));
        vm.enter(3, 80, 131, vec![]);
        assert!(vm.talk(3, 32193).contains("32191-12.htm"));
    }
}
