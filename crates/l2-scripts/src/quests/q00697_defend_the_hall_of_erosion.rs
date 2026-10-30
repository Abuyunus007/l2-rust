//! Quest 697: Defend the Hall of Erosion (Tepios at the Seed of Infinity, the defence of the Hall of Erosion).
// Java: dist/game/data/scripts/quests/Q00697_DefendTheHallOfErosion/Q00697_DefendTheHallOfErosion.java
//
// Pattern: a level 75+ quest offered only while the Seed of Infinity is at stage 4. The Hall of
// Erosion defence instance sets `defenceDone` (`ai/areas/gracia/instances/hall_of_erosion_defence.rs`);
// Tepios then pays 12 to 20 Vesper Stones (a random range) and clears the variable.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::gracia::seeds::soi_current_stage;

// NPC
const TEPIOS: i32 = 32603;

// Item
const VESPER_STONE: i32 = 14052;

/// Java `Q00697_DefendTheHallOfErosion`.
pub struct Q00697DefendTheHallOfErosion;

impl Script for Q00697DefendTheHallOfErosion {
    fn name(&self) -> &'static str {
        "Q00697_DefendTheHallOfErosion"
    }

    fn path(&self) -> &'static str {
        "quests/Q00697_DefendTheHallOfErosion"
    }

    fn id(&self) -> i32 {
        697
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
                if ctx.player_level(player) < 75 {
                    htmltext = "32603-00.html".to_owned();
                    qs.exit_quest(ctx, true, false);
                } else if soi_current_stage(ctx) != 4 {
                    htmltext = "32603-00a.html".to_owned();
                    qs.exit_quest(ctx, true, false);
                } else {
                    htmltext = "32603-01.htm".to_owned();
                }
            }
            STARTED => {
                if qs.is_cond(ctx, 1) && qs.get_int(ctx, "defenceDone") == 0 {
                    htmltext = "32603-04.html".to_owned();
                } else if qs.is_cond(ctx, 1) && qs.get_int(ctx, "defenceDone") != 0 {
                    let count = ctx.get_random_range(12, 20);
                    ctx.give_items(player, VESPER_STONE, i64::from(count));
                    qs.unset(ctx, "defenceDone");
                    qs.exit_quest(ctx, true, false);
                    htmltext = "32603-05.html".to_owned();
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

    const NAME: &str = "Q00697_DefendTheHallOfErosion";

    const NPCS: &str = r#"
        <npc id="32603" type="Folk" level="1" name="Tepios"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14052" type="EtcItem" name="Vesper Stone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00697DefendTheHallOfErosion);
        let pages = Pages::of(&Q00697DefendTheHallOfErosion)
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
    fn the_quest_opens_at_stage_4_for_level_75() {
        let (mut t, tepios) = setup();
        t.enter(1, 75, vec![]);
        set_stage(&mut t, "3");
        assert!(t.quest_page(1, tepios).contains("32603-00a.html"));
        assert!(t.take_writes().is_empty());

        set_stage(&mut t, "4");
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
    fn a_level_74_character_is_refused() {
        let (mut t, tepios) = setup();
        t.enter(1, 74, vec![]);
        set_stage(&mut t, "4");
        assert!(t.quest_page(1, tepios).contains("32603-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_defence_pays_12_to_20_vesper_stones() {
        let (mut t, tepios) = setup();
        t.enter(1, 75, vec![]);
        set_stage(&mut t, "4");
        t.quest_page(1, tepios);
        t.event(1, NAME, "32603-03.html");
        t.take_writes();

        // Not defended yet.
        assert!(t.quest_page(1, tepios).contains("32603-04.html"));
        assert!(t.take_writes().is_empty());

        // The defence instance set `defenceDone`.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_int(ctx, "defenceDone", 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, tepios).contains("32603-05.html"));
        let stones = t.count(1, VESPER_STONE);
        assert!((12..=20).contains(&stones), "{stones}");
        assert_eq!(
            t.take_writes(),
            [rows::delete_var(NAME, "defenceDone"), rows::delete(NAME, true)]
        );
    }
}
