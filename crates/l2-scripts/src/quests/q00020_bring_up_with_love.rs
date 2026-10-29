//! Quest 20: Bring Up With Love (Tunatun in the Gludio beast farm, for a level 82 character).
// Java: dist/game/data/scripts/quests/Q00020_BringUpWithLove/Q00020_BringUpWithLove.java
//
// Pattern: a long talk chain, then a drop-chance item found while hunting (`docs/design/scripts.md`,
// "Quest patterns"): Tunatun explains the beast farm over many pages, the quest starts on
// `31537-11.html`, the Innocence Jewel comes from beast farm kills (`check_jewel_of_innocence`),
// and Tunatun pays the Water Crystal for the jewel. One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TUNATUN: i32 = 31537;

// Items
const WATER_CRYSTAL: i32 = 9553;
const INNOCENCE_JEWEL: i32 = 15533;

// Misc
const MIN_LEVEL: i32 = 82;

/// Java `Q00020_BringUpWithLove`.
pub struct Q00020BringUpWithLove;

/// Java `Q00020_BringUpWithLove.checkJewelOfInnocence(player)`: a 5% chance of the Innocence
/// Jewel for a player at cond 1 who does not have it yet; the jewel moves the quest to cond 2.
///
/// FOLLOW-UP(engine): Java calls this from the beast farm kills (`ai/areas/BeastFarm/BeastFarm.java`
/// and `FeedableBeasts.java`, the AI batch). Nothing calls it on the Rust side yet.
pub fn check_jewel_of_innocence(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    if let Some(qs) = ctx.get_quest_state(player, false)
        && qs.is_cond(ctx, 1)
        && !ctx.has_quest_items(player, INNOCENCE_JEWEL)
        && ctx.get_random(100) < 5
    {
        ctx.give_items(player, INNOCENCE_JEWEL, 1);
        qs.set_cond(ctx, 2, true);
    }
}

impl Script for Q00020BringUpWithLove {
    fn name(&self) -> &'static str {
        "Q00020_BringUpWithLove"
    }

    fn path(&self) -> &'static str {
        "quests/Q00020_BringUpWithLove"
    }

    fn id(&self) -> i32 {
        20
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TUNATUN]);
        s.add_talk_id(&[TUNATUN]);
        s.register_quest_items(&[INNOCENCE_JEWEL]);
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

        match event {
            "31537-02.htm" | "31537-03.htm" | "31537-04.htm" | "31537-05.htm" | "31537-06.htm"
            | "31537-07.htm" | "31537-08.htm" | "31537-09.htm" | "31537-10.htm" | "31537-12.htm" => {
                Some(event.to_owned())
            }
            "31537-11.html" => {
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            "31537-16.html" => {
                if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, INNOCENCE_JEWEL) {
                    ctx.give_items(player, WATER_CRYSTAL, 1);
                    ctx.take_items(player, INNOCENCE_JEWEL, -1);
                    qs.exit_quest(ctx, false, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "31537-01.htm"
                } else {
                    "31537-13.html"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "31537-14.html".to_owned(),
                2 => {
                    htmltext = if ctx.has_quest_items(player, INNOCENCE_JEWEL) {
                        "31537-15.html"
                    } else {
                        "31537-14.html"
                    }
                    .to_owned();
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

    const NAME: &str = "Q00020_BringUpWithLove";

    const NPCS: &str = r#"
        <npc id="31537" type="Folk" level="70" name="Tunatun"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9553" type="EtcItem" name="Water Crystal"/>
        <item id="15533" type="EtcItem" name="Innocence Jewel"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00020BringUpWithLove);
        let pages = Pages::of(&Q00020BringUpWithLove)
            .page("31537-01.htm", &["31537-02.htm"])
            .page("31537-02.htm", &["31537-03.htm"])
            .page("31537-03.htm", &["31537-04.htm", "31537-05.htm"])
            .page("31537-05.htm", &["31537-06.htm"])
            .page("31537-06.htm", &["31537-07.htm"])
            .page("31537-07.htm", &["31537-08.htm", "31537-12.htm"])
            .page("31537-08.htm", &["31537-09.htm"])
            .page("31537-09.htm", &["31537-10.htm"])
            .page("31537-10.htm", &["31537-11.html"])
            .page("31537-15.html", &["31537-16.html"])
            .plain(&[
                "31537-04.htm",
                "31537-11.html",
                "31537-12.htm",
                "31537-13.html",
                "31537-14.html",
                "31537-16.html",
            ])
            .npcs(&[TUNATUN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tunatun = t.spawn(TUNATUN, 20);
        (t, tunatun)
    }

    /// Runs `check_jewel_of_innocence` until the 5% chance gives the jewel (the test world's RNG
    /// is seeded, so this takes the same number of rolls every run).
    fn roll_until_the_jewel(t: &mut ScriptTest, player: i32) {
        for _ in 0..1_000 {
            if t.count(player, INNOCENCE_JEWEL) > 0 {
                return;
            }
            t.world.run_as_script(NAME, |ctx| {
                check_jewel_of_innocence(ctx, ObjectId(player));
            });
        }
    }

    #[test]
    fn a_character_below_level_82_is_sent_away() {
        let (mut t, tunatun) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, tunatun).contains("31537-13.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_talk_then_the_jewel_then_the_water_crystal() {
        let (mut t, tunatun) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, tunatun).contains("31537-01.htm"));
        for event in [
            "31537-02.htm",
            "31537-03.htm",
            "31537-05.htm",
            "31537-06.htm",
            "31537-07.htm",
            "31537-08.htm",
            "31537-09.htm",
            "31537-10.htm",
        ] {
            assert!(t.event(1, NAME, event).contains(event), "{event}");
        }
        assert!(t.event(1, NAME, "31537-11.html").contains("31537-11.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, tunatun).contains("31537-14.html"));

        // The beast farm drops the jewel (cond 2), and Tunatun gives the crystal.
        roll_until_the_jewel(&mut t, 1);
        assert_eq!(t.count(1, INNOCENCE_JEWEL), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, tunatun).contains("31537-15.html"));
        assert!(t.event(1, NAME, "31537-16.html").contains("31537-16.html"));
        assert_eq!((t.count(1, WATER_CRYSTAL), t.count(1, INNOCENCE_JEWEL)), (1, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, tunatun)
                .contains("This quest has already been completed.")
        );
    }
}
