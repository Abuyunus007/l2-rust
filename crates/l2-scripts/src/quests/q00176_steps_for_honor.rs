//! Quest 176: Steps for Honor (Rapidus, the Territory War quartermaster).
// Java: dist/game/data/scripts/quests/Q00176_StepsForHonor/Q00176_StepsForHonor.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const RAPIDUS: i32 = 36479;

// Item
const CLOAK: i32 = 14603;

// Misc
const MIN_LEVEL: i32 = 80;

/// Java `Q00176_StepsForHonor`.
pub struct Q00176StepsForHonor;

impl Script for Q00176StepsForHonor {
    fn name(&self) -> &'static str {
        "Q00176_StepsForHonor"
    }

    fn path(&self) -> &'static str {
        "quests/Q00176_StepsForHonor"
    }

    fn id(&self) -> i32 {
        176
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAPIDUS]);
        s.add_talk_id(&[RAPIDUS]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        if event.eq_ignore_ascii_case("36479-04.html") {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "36479-03.html"
                } else {
                    "36479-02.html"
                }
                .to_owned();
            }
            STARTED => {
                // Java `TerritoryWarManager.getInstance().isTWInProgress()`: the quest waits.
                if ctx.world().is_tw_in_progress() {
                    return Some("36479-05.html".to_owned());
                }
                match qs.get_cond(ctx) {
                    1 => htmltext = "36479-06.html".to_owned(),
                    2 => {
                        qs.set_cond(ctx, 3, true);
                        htmltext = "36479-07.html".to_owned();
                    }
                    3 => htmltext = "36479-08.html".to_owned(),
                    4 => {
                        qs.set_cond(ctx, 5, true);
                        htmltext = "36479-09.html".to_owned();
                    }
                    5 => htmltext = "36479-10.html".to_owned(),
                    6 => {
                        qs.set_cond(ctx, 7, true);
                        htmltext = "36479-11.html".to_owned();
                    }
                    7 => htmltext = "36479-12.html".to_owned(),
                    8 => {
                        ctx.give_items(player, CLOAK, 1);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "36479-13.html".to_owned();
                    }
                    _ => {}
                }
            }
            COMPLETED => htmltext = "36479-01.html".to_owned(),
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

    const NAME: &str = "Q00176_StepsForHonor";

    const NPCS: &str = r#"
        <npc id="36479" type="Folk" level="80" name="Rapidus"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="14603" type="Armor" name="Cloak of Territory War"><set name="bodypart" val="back"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00176StepsForHonor);
        let pages = Pages::of(&Q00176StepsForHonor)
            .page("36479-03.html", &["36479-04.html"])
            .plain(&[
                "36479-01.html",
                "36479-02.html",
                "36479-04.html",
                "36479-05.html",
                "36479-06.html",
                "36479-07.html",
                "36479-08.html",
                "36479-09.html",
                "36479-10.html",
                "36479-11.html",
                "36479-12.html",
                "36479-13.html",
            ])
            .npcs(&[RAPIDUS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rapidus = t.spawn(RAPIDUS, 20);
        (t, rapidus)
    }

    /// Sets the quest's cond (the quest middle steps are the Territory War's business).
    fn set_cond(t: &mut ScriptTest, cond: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, cond, false);
        });
    }

    #[test]
    fn only_a_character_of_level_80_is_offered_the_quest() {
        let (mut t, rapidus) = setup();
        t.enter(1, 79, vec![]);
        assert!(t.quest_page(1, rapidus).contains("36479-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn eight_steps_then_the_cloak() {
        let (mut t, rapidus) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, rapidus).contains("36479-03.html"));
        assert!(t.event(1, NAME, "36479-04.html").contains("36479-04.html"));
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "cond", "1"), rows::insert(NAME, "<state>", "Started")]
        );
        assert!(t.quest_page(1, rapidus).contains("36479-06.html"));

        // Cond 2 moves to 3 (`setCond(3, true)`), the way Java does it at talk time.
        set_cond(&mut t, 2);
        t.take_writes();
        assert!(t.quest_page(1, rapidus).contains("36479-07.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, rapidus).contains("36479-08.html"));

        set_cond(&mut t, 8);
        t.take_writes();
        assert!(t.quest_page(1, rapidus).contains("36479-13.html"));
        assert_eq!(t.count(1, CLOAK), 1);
        assert_eq!(
            t.take_writes(),
            [rows::delete(NAME, false), rows::update(NAME, "<state>", "Completed")]
        );
        assert!(t.quest_page(1, rapidus).contains("36479-01.html"));
    }
}
