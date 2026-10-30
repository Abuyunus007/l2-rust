//! Trainer healers: Cardinals, Eva's Saints and Shillien Saints learn and cleanse transfer skills.
// Java: dist/game/data/scripts/ai/others/HealerTrainer/HealerTrainer.java
//
// FOLLOW-UP(engine): the transfer skill tree (`transferSkillTree` of `skillTrees/`) is not loaded
// (`l2_data::skill_tree` keeps the class trees only, see `skills/learn.rs`). With the tree empty,
// `SkillTransferLearn` finds no skill ("there are no other skills to learn") and
// `SkillTransferCleanse` has nothing to remove or refund, which is Java's code with an empty tree.
// Once the tree is loaded, the two loops below (`getAvailableTransferSkills`,
// `getTransferSkillTree(..).values()` with the refund of the required items) belong where the
// `// FOLLOW-UP(engine)` marks are.

use l2_protocol_game::SystemMessageId;
use l2_util::ObjectId;
use l2_world::script::player_class;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const HEALER_TRAINERS: [i32; 44] = [
    30022, 30030, 30032, 30036, 30067, 30068, 30116, 30117, 30118, 30119, 30144, 30145, 30188, 30194,
    30293, 30330, 30375, 30377, 30464, 30473, 30476, 30680, 30701, 30720, 30721, 30858, 30859, 30860,
    30861, 30864, 30906, 30908, 30912, 31280, 31281, 31287, 31329, 31330, 31335, 31969, 31970, 31976,
    32155, 32162,
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

                // FOLLOW-UP(engine): `getAvailableTransferSkills(player)` fills the
                // `AcquireSkillList(TRANSFER)`; no tree, so `count` stays 0.
                ctx.send_system_message(player, SystemMessageId::ThereAreNoOtherSkillsToLearn);
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
                }
                // FOLLOW-UP(engine): else the transfer tree loop removes the known skills,
                // refunds their items (`ItemProcessType.REFUND`) and takes the fee once.
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
    use crate::testing::{Pages, ScriptTest, names_to, pages_to};

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
        assert!(t.event(1, "HealerTrainer", "SkillTransferLearn").contains("30022-noteach.html"));
        t.talk(1, trainer);
        t.event(1, "HealerTrainer", "SkillTransfer");
        assert!(t.event(1, "HealerTrainer", "SkillTransferCleanse").contains("cleanse-no.html"));
    }

    #[test]
    fn the_pomander_of_the_class_is_what_blocks_the_cleansing() {
        let (mut t, _) = setup();
        t.take();
        assert!(!has_transfer_skill_items_for_test(&mut t));
        assert!(names_to(&t.take(), 1).is_empty());
    }

    fn has_transfer_skill_items_for_test(t: &mut ScriptTest) -> bool {
        let mut result = true;
        t.world.run_as_script("HealerTrainer", |ctx| {
            result = has_transfer_skill_items(ctx, ObjectId(1));
        });
        result
    }
}
