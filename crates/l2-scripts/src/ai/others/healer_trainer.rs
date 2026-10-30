//! Trainer healers: Cardinals, Eva's Saints and Shillien Saints learn and cleanse transfer skills.
// Java: dist/game/data/scripts/ai/others/HealerTrainer/HealerTrainer.java

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::script::player_class;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const HEALER_TRAINERS: [i32; 44] = [
    30022, 30030, 30032, 30036, 30067, 30068, 30116, 30117, 30118, 30119, 30144, 30145, 30188, 30194, 30293,
    30330, 30375, 30377, 30464, 30473, 30476, 30680, 30701, 30720, 30721, 30858, 30859, 30860, 30861, 30864,
    30906, 30908, 30912, 31280, 31281, 31287, 31329, 31330, 31335, 31969, 31970, 31976, 32155, 32162,
];

// Misc
const MIN_LEVEL: i32 = 76;
const MIN_CLASS_LEVEL: i32 = 3;

/// Java `hasTransferSkillItems(player)`: the player has the pomander of the current class.
fn has_transfer_skill_items(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    let item_id = match ctx.player_class_id(player) {
        player_class::CARDINAL => 15307,
        player_class::EVA_SAINT => 15308,
        player_class::SHILLIEN_SAINT => 15309,
        _ => -1,
    };
    ctx.get_quest_items_count(player, item_id) > 0
}

/// The level and class level check both transfer events make.
fn too_low(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.player_level(player) < MIN_LEVEL || ctx.player_class_level(player) < MIN_CLASS_LEVEL
}

/// Java `ai.others.HealerTrainer.HealerTrainer`.
pub struct HealerTrainer;

impl Script for HealerTrainer {
    fn name(&self) -> &'static str {
        "HealerTrainer"
    }

    fn path(&self) -> &'static str {
        "ai/others/HealerTrainer"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&HEALER_TRAINERS);
        s.add_talk_id(&HEALER_TRAINERS);
        s.add_first_talk_id(&HEALER_TRAINERS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = None;
        match event {
            "30864.html" | "30864-1.html" => htmltext = Some(event.to_owned()),
            "SkillTransfer" => htmltext = Some("main.html".to_owned()),
            "SkillTransferLearn" => {
                let (npc, player) = (npc?, player?);
                if !ctx.world().script_npc_can_teach(npc.id, player) {
                    return Some(format!("{}-noteach.html", npc.id));
                }

                if too_low(ctx, player) {
                    return Some("learn-lowlevel.html".to_owned());
                }

                // The `AcquireSkillList(TRANSFER)` of `getAvailableTransferSkills(player)`, or
                // "There are no other skills to learn."
                ctx.world_mut().show_transfer_skill_list(player);
            }
            "SkillTransferCleanse" => {
                let (npc, player) = (npc?, player?);
                if !ctx.world().script_npc_can_teach(npc.id, player) {
                    return Some("cleanse-no.html".to_owned());
                }

                if too_low(ctx, player) {
                    return Some("cleanse-no.html".to_owned());
                }

                if ctx.adena_of(player) < i64::from(ctx.script_settings().fee_delete_transfer_skills) {
                    ctx.send_system_message(
                        player,
                        SystemMessageId::YouCannotResetTheSkillLinkBecauseThereIsNotEnoughAdena,
                    );
                    return None;
                }

                if has_transfer_skill_items(ctx, player) {
                    // Come back when you have used all transfer skill items for this class.
                    htmltext = Some("cleanse-no_skills.html".to_owned());
                } else {
                    let fee = i64::from(ctx.script_settings().fee_delete_transfer_skills);
                    ctx.world_mut().cleanse_transfer_skills(player, fee);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to};

    const NPCS: &str = r#"<npc id="30022" type="Folk" level="70" name="Zigaunt"/>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(HealerTrainer);
        let pages = Pages::of(&HealerTrainer)
            .page("30022.html", &["SkillTransfer"])
            .page("main.html", &["SkillTransferLearn", "SkillTransferCleanse"])
            .plain(&["30022-noteach.html", "cleanse-no.html", "learn-lowlevel.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        let trainer = t.spawn(30022, 20);
        t.enter(1, 80, vec![]);
        (t, trainer)
    }

    #[test]
    fn the_first_talk_shows_the_page_of_the_npc_and_the_transfer_page_follows() {
        let (mut t, trainer) = setup();
        t.take();
        t.talk(1, trainer);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("30022.html"));
        assert!(t.event(1, "HealerTrainer", "SkillTransfer").contains("main.html"));
    }

    #[test]
    fn a_class_the_npc_does_not_teach_gets_the_noteach_pages() {
        let (mut t, trainer) = setup();
        t.talk(1, trainer);
        t.event(1, "HealerTrainer", "SkillTransfer");
        assert!(
            t.event(1, "HealerTrainer", "SkillTransferLearn")
                .contains("30022-noteach.html")
        );
        t.talk(1, trainer);
        t.event(1, "HealerTrainer", "SkillTransfer");
        assert!(
            t.event(1, "HealerTrainer", "SkillTransferCleanse")
                .contains("cleanse-no.html")
        );
    }

    #[test]
    fn the_pomander_of_the_class_is_what_blocks_the_cleansing() {
        let (mut t, _) = setup();
        t.take();
        assert!(!has_transfer_skill_items_for_test(&mut t));
        assert!(names_to(&t.take(), 1).is_empty());
    }

    // ---- the transfer tree (S-LEFT-01) ----

    use l2_protocol_game::server::{AcquireSkillInfo, AcquireSkillList, AcquireSkillType};
    use l2_world::ClientInput;

    const LEARN_ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15307" type="EtcItem" name="Pomander of Cardinal"/>"#;
    const TRANSFER_SKILLS: &str = r#"
        <skill id="1013" levels="32" name="Recharge"><operateType>A2</operateType><targetType>SELF</targetType></skill>
        <skill id="1087" levels="3" name="Agility"><operateType>A2</operateType><targetType>SELF</targetType></skill>"#;
    // The tree belongs to the Bishop (16); a Cardinal (97, third class) uses his parent's.
    const TRANSFER_TREE: &str = r#"
        <skillTree type="transferSkillTree" classId="16">
            <skill skillName="Recharge" skillId="1013" skillLevel="32" getLevel="76"><item id="15307" count="1"/></skill>
            <skill skillName="Agility" skillId="1087" skillLevel="3" getLevel="76"><item id="15307" count="1"/></skill>
        </skillTree>"#;

    fn transfer_setup(items: Vec<l2_world::ItemData>) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(HealerTrainer);
        let pages = Pages::of(&HealerTrainer)
            .page("30022.html", &["SkillTransfer"])
            .page("main.html", &["SkillTransferLearn", "SkillTransferCleanse"])
            .plain(&["cleanse-no.html", "cleanse-no_skills.html", "learn-lowlevel.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, LEARN_ITEMS, pages);
        t.set_skills(LEARN_ITEMS, TRANSFER_SKILLS);
        t.set_skill_trees(TRANSFER_TREE);
        t.set_skill_learn(r#"<npc id="30022"><classId>16</classId></npc>"#);
        let trainer = t.spawn(30022, 20);
        t.enter_as(1, 80, l2_world::script::race::HUMAN, player_class::CARDINAL, items);
        // The bypass of a link works only from the page that shows it.
        t.talk(1, trainer);
        t.event(1, "HealerTrainer", "SkillTransfer");
        t.take();
        (t, trainer)
    }

    fn list_of(out: &crate::testing::Sent) -> Option<AcquireSkillList> {
        out.iter()
            .find(|(to, b)| *to == 1 && crate::testing::name_of(b) == "ACQUIRE_SKILL_LIST")
            .map(|(_, b)| crate::testing::decode(b))
    }

    fn learn(t: &mut ScriptTest, id: i32, level: i32) {
        t.input(
            1,
            ClientInput::AcquireSkill {
                id,
                level,
                skill_type: AcquireSkillType::Transfer,
                sub_type: 0,
            },
        );
    }

    #[test]
    fn the_transfer_skills_are_listed_and_learned_with_the_pomander() {
        let (mut t, _) = transfer_setup(vec![item(100, 15307, 1), item(101, 57, 25_000_000)]);
        t.bypass(1, "Script HealerTrainer SkillTransferLearn");
        let list = list_of(&t.take()).expect("the transfer list");
        assert_eq!(list.skill_type, AcquireSkillType::Transfer);
        let ids: Vec<(i32, i32)> = list.skills.iter().map(|s| (s.id, s.next_level)).collect();
        assert_eq!(ids.len(), 2);
        assert!(ids.contains(&(1013, 32)) && ids.contains(&(1087, 3)));

        // The details cost the pomander.
        t.input(
            1,
            ClientInput::AcquireSkillInfo {
                id: 1013,
                level: 32,
                skill_type: AcquireSkillType::Transfer,
            },
        );
        let out = t.take();
        let info: AcquireSkillInfo = out
            .iter()
            .find(|(_, b)| crate::testing::name_of(b) == "ACQUIRE_SKILL_INFO")
            .map(|(_, b)| crate::testing::decode(b))
            .unwrap();
        assert_eq!((info.skill_id, info.level, info.sp_cost), (1013, 32, 0));
        assert_eq!(info.requirements.len(), 1);
        assert_eq!(info.requirements[0].item_id, 15307);

        // Learning takes the pomander (one only: the second skill is refused).
        learn(&mut t, 1013, 32);
        assert!(t.world.known_skill(ObjectId(1), 1013).is_some());
        assert_eq!(t.count(1, 15307), 0);
        t.take();
        learn(&mut t, 1087, 3);
        assert!(t.world.known_skill(ObjectId(1), 1087).is_none());

        // The learned skill is not offered again.
        t.bypass(1, "Script HealerTrainer SkillTransferLearn");
        let list = list_of(&t.take()).expect("the rest of the list");
        assert_eq!(list.skills.len(), 1);
        assert_eq!(list.skills[0].id, 1087);
    }

    #[test]
    fn a_skill_outside_the_tree_is_not_learned() {
        let (mut t, _) = transfer_setup(vec![item(100, 15307, 1)]);
        learn(&mut t, 1013, 31);
        assert!(t.world.known_skill(ObjectId(1), 1013).is_none());
        assert_eq!(t.count(1, 15307), 1);
    }

    #[test]
    fn the_cleansing_takes_the_skills_back_refunds_the_pomanders_and_takes_the_fee_once() {
        let (mut t, _) = transfer_setup(vec![item(100, 15307, 2), item(101, 57, 25_000_000)]);
        learn(&mut t, 1013, 32);
        learn(&mut t, 1087, 3);
        assert!(t.world.known_skill(ObjectId(1), 1013).is_some());
        assert!(t.world.known_skill(ObjectId(1), 1087).is_some());
        assert_eq!(t.count(1, 15307), 0);

        t.bypass(1, "Script HealerTrainer SkillTransferCleanse");
        t.take();
        assert!(t.world.known_skill(ObjectId(1), 1013).is_none());
        assert!(t.world.known_skill(ObjectId(1), 1087).is_none());
        assert_eq!(t.count(1, 15307), 2);
        assert_eq!(t.count(1, 57), 15_000_000);
    }

    #[test]
    fn a_pomander_in_the_bag_or_no_skill_costs_nothing() {
        // The pomander of the class is still in the bag: "use them first".
        let (mut t, _) = transfer_setup(vec![item(100, 15307, 1), item(101, 57, 25_000_000)]);
        assert!(
            t.event(1, "HealerTrainer", "SkillTransferCleanse")
                .contains("cleanse-no_skills.html")
        );
        assert_eq!(t.count(1, 57), 25_000_000);
        // Without the pomander and without a transfer skill there is nothing to clean: no fee.
        let (mut t, _) = transfer_setup(vec![item(101, 57, 25_000_000)]);
        t.bypass(1, "Script HealerTrainer SkillTransferCleanse");
        assert_eq!(t.count(1, 57), 25_000_000);
        // Too poor for the fee.
        let (mut t, _) = transfer_setup(vec![item(101, 57, 1_000)]);
        t.bypass(1, "Script HealerTrainer SkillTransferCleanse");
        assert!(names_to(&t.take(), 1).contains(&"SYSTEM_MESSAGE".to_owned()));
    }

    fn has_transfer_skill_items_for_test(t: &mut ScriptTest) -> bool {
        let mut result = true;
        t.world.run_as_script("HealerTrainer", |ctx| {
            result = has_transfer_skill_items(ctx, ObjectId(1));
        });
        result
    }
}
