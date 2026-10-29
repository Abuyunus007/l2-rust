//! Quest 15: Sweet Whispers (Vladimir in the Town of Dion sends a level 60 character to a Necromancer and the Hierarch).
// Java: dist/game/data/scripts/quests/Q00015_SweetWhispers/Q00015_SweetWhispers.java
//
// Pattern: a two-step errand with no items (`docs/design/scripts.md`, "Quest patterns"): Vladimir
// starts it, the Necromancer moves it to cond 2 with a page, and the Hierarch pays the reward
// (exp only). One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const VLADIMIR: i32 = 31302;
const HIERARCH: i32 = 31517;
const M_NECROMANCER: i32 = 31518;

// Misc
const MIN_LEVEL: i32 = 60;

/// Java `Q00015_SweetWhispers`.
pub struct Q00015SweetWhispers;

impl Script for Q00015SweetWhispers {
    fn name(&self) -> &'static str {
        "Q00015_SweetWhispers"
    }

    fn path(&self) -> &'static str {
        "quests/Q00015_SweetWhispers"
    }

    fn id(&self) -> i32 {
        15
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VLADIMIR]);
        s.add_talk_id(&[VLADIMIR, HIERARCH, M_NECROMANCER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): without a quest state `onEvent` returns the event itself (not null),
        // so the event's page is shown anyway.
        let Some(player) = player else {
            return Some(event.to_owned());
        };
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(event.to_owned());
        };

        match event {
            "31302-01.html" => qs.start_quest(ctx),
            "31518-01.html" => {
                if qs.is_cond(ctx, 1) {
                    // `setCond(2)`: no sound.
                    qs.set_cond(ctx, 2, false);
                }
            }
            "31517-01.html" => {
                if qs.is_cond(ctx, 2) {
                    ctx.add_exp_and_sp(player, 350_531, 28_204);
                    qs.exit_quest(ctx, false, true);
                }
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let npc_id = npc.id;

        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                if npc_id == VLADIMIR {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "31302-00.htm"
                    } else {
                        "31302-00a.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc_id {
                VLADIMIR => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "31302-01a.html".to_owned();
                    }
                }
                M_NECROMANCER => {
                    let cond = qs.get_cond(ctx);
                    if cond == 1 {
                        htmltext = "31518-00.html".to_owned();
                    } else if cond == 2 {
                        htmltext = "31518-01a.html".to_owned();
                    }
                }
                HIERARCH => {
                    if qs.is_cond(ctx, 2) {
                        htmltext = "31517-00.html".to_owned();
                    }
                }
                _ => {}
            },
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

    const NAME: &str = "Q00015_SweetWhispers";

    const NPCS: &str = r#"
        <npc id="31302" type="Folk" level="70" name="Vladimir"/>
        <npc id="31517" type="Folk" level="70" name="Hierarch"/>
        <npc id="31518" type="Folk" level="70" name="Necromancer"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        vladimir: Oid,
        necromancer: Oid,
        hierarch: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00015SweetWhispers);
        let pages = Pages::of(&Q00015SweetWhispers)
            .page("31302-00.htm", &["31302-01.html"])
            .page("31518-00.html", &["31518-01.html"])
            .page("31517-00.html", &["31517-01.html"])
            .plain(&[
                "31302-00a.html",
                "31302-01.html",
                "31302-01a.html",
                "31518-01.html",
                "31518-01a.html",
                "31517-01.html",
            ])
            .npcs(&[VLADIMIR, HIERARCH, M_NECROMANCER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            vladimir: t.spawn(VLADIMIR, 20),
            necromancer: t.spawn(M_NECROMANCER, 40),
            hierarch: t.spawn(HIERARCH, 60),
        };
        (t, npcs)
    }

    #[test]
    fn a_character_below_level_60_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 59, vec![]);
        assert!(t.quest_page(1, npcs.vladimir).contains("31302-00a.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_necromancer_then_the_hierarch_pays_the_exp() {
        let (mut t, npcs) = setup();
        t.enter(1, 60, vec![]);
        assert!(t.quest_page(1, npcs.vladimir).contains("31302-00.htm"));
        assert!(t.event(1, NAME, "31302-01.html").contains("31302-01.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.vladimir).contains("31302-01a.html"));

        // The Necromancer sends the player on (cond 2).
        assert!(t.quest_page(1, npcs.necromancer).contains("31518-00.html"));
        assert!(t.event(1, NAME, "31518-01.html").contains("31518-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.necromancer).contains("31518-01a.html"));

        // The Hierarch: the exp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.hierarch).contains("31517-00.html"));
        assert!(t.event(1, NAME, "31517-01.html").contains("31517-01.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.vladimir)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_hierarch_says_nothing_before_cond_two() {
        let (mut t, npcs) = setup();
        t.enter(1, 60, vec![]);
        t.quest_page(1, npcs.vladimir);
        t.event(1, NAME, "31302-01.html");
        t.take_writes();
        assert!(!t.quest_page(1, npcs.hierarch).contains("31517-00.html"));
        assert!(t.take_writes().is_empty());
    }
}
