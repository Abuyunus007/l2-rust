//! Quest 696: Conquer the Hall of Erosion (Tepios at the Seed of Infinity, the attack on the Hall of Erosion).
// Java: dist/game/data/scripts/quests/Q00696_ConquerTheHallOfErosion/Q00696_ConquerTheHallOfErosion.java
//
// Pattern: a quest that only the Hall of Erosion attack instance can finish: its `cohemenes`
// variable is set there (`ai/areas/gracia/instances/hall_of_erosion_attack.rs`). The first
// Mark of Keucereus becomes the second one. Java's `exitQuest(true)` (no sound) ends the quest
// on every refusal too.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TEPIOS: i32 = 32603;

// Items
const MARK_OF_KEUCEREUS_STAGE_1: i32 = 13691;
const MARK_OF_KEUCEREUS_STAGE_2: i32 = 13692;

/// Java `Q00696_ConquerTheHallOfErosion`.
pub struct Q00696ConquerTheHallOfErosion;

impl Script for Q00696ConquerTheHallOfErosion {
    fn name(&self) -> &'static str {
        "Q00696_ConquerTheHallOfErosion"
    }

    fn path(&self) -> &'static str {
        "quests/Q00696_ConquerTheHallOfErosion"
    }

    fn id(&self) -> i32 {
        696
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

        if event.eq_ignore_ascii_case("32603-02.html") {
            qs.start_quest(ctx);
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if ctx.player_level(player) >= 75 {
                    if ctx.get_quest_items_count(player, MARK_OF_KEUCEREUS_STAGE_1) > 0
                        || ctx.get_quest_items_count(player, MARK_OF_KEUCEREUS_STAGE_2) > 0
                    {
                        htmltext = "32603-01.htm".to_owned();
                    } else {
                        htmltext = "32603-05.html".to_owned();
                        qs.exit_quest(ctx, true, false);
                    }
                } else {
                    htmltext = "32603-00.html".to_owned();
                    qs.exit_quest(ctx, true, false);
                }
            }
            STARTED => {
                if qs.get_int(ctx, "cohemenes") != 0 {
                    if ctx.get_quest_items_count(player, MARK_OF_KEUCEREUS_STAGE_2) < 1 {
                        ctx.take_items(player, MARK_OF_KEUCEREUS_STAGE_1, 1);
                        ctx.give_items(player, MARK_OF_KEUCEREUS_STAGE_2, 1);
                    }
                    htmltext = "32603-04.html".to_owned();
                    qs.exit_quest(ctx, true, false);
                } else {
                    htmltext = "32603-01a.html".to_owned();
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

    const NAME: &str = "Q00696_ConquerTheHallOfErosion";

    const NPCS: &str = r#"
        <npc id="32603" type="Folk" level="1" name="Tepios"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13691" type="EtcItem" name="Mark of Keucereus Stage 1"><set name="is_questitem" val="true"/></item>
        <item id="13692" type="EtcItem" name="Mark of Keucereus Stage 2"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00696ConquerTheHallOfErosion);
        let pages = Pages::of(&Q00696ConquerTheHallOfErosion)
            .page("32603-01.htm", &["32603-02.html"])
            .plain(&[
                "32603-00.html",
                "32603-01a.html",
                "32603-02.html",
                "32603-03.html",
                "32603-04.html",
                "32603-05.html",
            ])
            .npcs(&[TEPIOS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tepios = t.spawn(TEPIOS, 20);
        (t, tepios)
    }

    #[test]
    fn a_level_75_character_with_a_mark_starts_the_quest() {
        let (mut t, tepios) = setup();
        t.enter(1, 75, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), MARK_OF_KEUCEREUS_STAGE_1, 1);
        });
        assert!(t.quest_page(1, tepios).contains("32603-01.htm"));
        assert!(t.take_writes().is_empty());

        assert!(t.event(1, NAME, "32603-02.html").contains("32603-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn a_level_74_character_is_refused_and_the_quest_is_closed() {
        let (mut t, tepios) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, tepios).contains("32603-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_level_75_without_a_mark_is_refused() {
        let (mut t, tepios) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, tepios).contains("32603-05.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_hall_of_erosion_attack_turns_stage_1_into_stage_2() {
        let (mut t, tepios) = setup();
        t.enter(1, 75, vec![]);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), MARK_OF_KEUCEREUS_STAGE_1, 1);
        });
        t.quest_page(1, tepios);
        t.event(1, NAME, "32603-02.html");
        t.take_writes();

        // Without `cohemenes` Tepios sends the player back to the Hall.
        assert!(t.quest_page(1, tepios).contains("32603-01a.html"));
        assert!(t.take_writes().is_empty());

        // The attack instance set `cohemenes`: the mark is exchanged and the quest ends.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_int(ctx, "cohemenes", 1);
        });
        t.take_writes();
        assert!(t.quest_page(1, tepios).contains("32603-04.html"));
        assert_eq!(
            (
                t.count(1, MARK_OF_KEUCEREUS_STAGE_1),
                t.count(1, MARK_OF_KEUCEREUS_STAGE_2)
            ),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
