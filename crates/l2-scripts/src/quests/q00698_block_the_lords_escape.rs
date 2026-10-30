//! Quest 698: Block the Lord's Escape (Tepios at the Seed of Infinity, the stage 5 defence of Ekimus's escape).
// Java: dist/game/data/scripts/quests/Q00698_BlockTheLordsEscape/Q00698_BlockTheLordsEscape.java
//
// Pattern: a level 75 to 85 quest offered only while the Seed of Infinity is at stage 5. The
// defence sets `defenceDone` to 1 (`ai/areas/gracia/instances`); Tepios then pays 5 to 8 Vesper
// Stones with the quest rates (`reward_items`, Java `rewardItems`).

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::gracia::seeds::soi_current_stage;

// NPC
const TEPIOS: i32 = 32603;

// Item
const VESPER_STONE: i32 = 14052;

/// Java `Q00698_BlockTheLordsEscape`.
pub struct Q00698BlockTheLordsEscape;

impl Script for Q00698BlockTheLordsEscape {
    fn name(&self) -> &'static str {
        "Q00698_BlockTheLordsEscape"
    }

    fn path(&self) -> &'static str {
        "quests/Q00698_BlockTheLordsEscape"
    }

    fn id(&self) -> i32 {
        698
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TEPIOS]);
        s.add_talk_id(&[TEPIOS]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        // NOTE(java-quirk): every event is echoed back as a page, even one for a player with no
        // quest state (Java returns `event` before it checks anything else).
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(event.to_owned());
        };

        if event.eq_ignore_ascii_case("32603-03.html") {
            qs.start_quest(ctx);
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                let level = ctx.player_level(player);
                if !(75..=85).contains(&level) {
                    htmltext = "32603-00.html".to_owned();
                    qs.exit_quest(ctx, true, false);
                } else if soi_current_stage(ctx) != 5 {
                    htmltext = "32603-00a.html".to_owned();
                    qs.exit_quest(ctx, true, false);
                } else {
                    htmltext = "32603-01.htm".to_owned();
                }
            }
            STARTED => {
                if qs.is_cond(ctx, 1) && qs.get_int(ctx, "defenceDone") == 1 {
                    let count = ctx.get_random_range(5, 8);
                    ctx.reward_items(player, VESPER_STONE, i64::from(count));
                    qs.exit_quest(ctx, true, false);
                    htmltext = "32603-05.html".to_owned();
                } else {
                    htmltext = "32603-04.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00698_BlockTheLordsEscape";

    const NPCS: &str = r#"
        <npc id="32603" type="Folk" level="1" name="Tepios"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14052" type="EtcItem" name="Vesper Stone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00698BlockTheLordsEscape);
        let pages = Pages::of(&Q00698BlockTheLordsEscape)
            .page("32603-01.htm", &["32603-03.html"])
            .plain(&[
                "32603-00.html",
                "32603-00a.html",
                "32603-03.html",
                "32603-04.html",
                "32603-05.html",
            ])
            .npcs(&[TEPIOS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tepios = t.spawn(TEPIOS, 20);
        (t, tepios)
    }

    fn set_stage(t: &mut ScriptTest, stage: &str) {
        t.world
            .run_as_script(NAME, |ctx| ctx.set_global_variable("SoI_stage", stage));
    }

    #[test]
    fn the_quest_opens_at_stage_5_for_levels_75_to_85() {
        let (mut t, tepios) = setup();
        t.enter(1, 85, vec![]);
        set_stage(&mut t, "4");
        assert!(t.quest_page(1, tepios).contains("32603-00a.html"));
        assert!(t.take_writes().is_empty());

        set_stage(&mut t, "5");
        assert!(t.quest_page(1, tepios).contains("32603-01.htm"));
        assert!(t.event(1, NAME, "32603-03.html").contains("32603-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn a_level_86_character_is_refused() {
        let (mut t, tepios) = setup();
        t.enter(1, 86, vec![]);
        set_stage(&mut t, "5");
        assert!(t.quest_page(1, tepios).contains("32603-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_block_pays_5_to_8_vesper_stones_once_defended() {
        let (mut t, tepios) = setup();
        t.enter(1, 80, vec![]);
        set_stage(&mut t, "5");
        t.quest_page(1, tepios);
        t.event(1, NAME, "32603-03.html");
        t.take_writes();

        assert!(t.quest_page(1, tepios).contains("32603-04.html"));
        assert!(t.take_writes().is_empty());

        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_int(ctx, "defenceDone", 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, tepios).contains("32603-05.html"));
        let stones = t.count(1, VESPER_STONE);
        assert!((5..=8).contains(&stones), "{stones}");
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
