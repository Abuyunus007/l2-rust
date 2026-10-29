//! Quest 119: Last Imperial Prince (the Nameless Spirit, Nurse Devorin in Rune Township).
// Java: dist/game/data/scripts/quests/Q00119_LastImperialPrince/Q00119_LastImperialPrince.java
//
// Pattern: a quest that asks for an item the player already has (the Antique Brooch of "Four
// Goblets") and answers with a page in both states; `onEvent` returns null for the events it
// does not know (no echo), and a player without the state gets null too. The `brooch` event is
// the link of Devorin's page (`bypass -h Script ... brooch`), so it answers with a page only.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NAMELESS_SPIRIT: i32 = 31453;
const DEVORIN: i32 = 32009;

// Item
const ANTIQUE_BROOCH: i32 = 7262;

// Misc
const MIN_LEVEL: i32 = 74;

/// Java `Q00119_LastImperialPrince`.
pub struct Q00119LastImperialPrince;

impl Script for Q00119LastImperialPrince {
    fn name(&self) -> &'static str {
        "Q00119_LastImperialPrince"
    }

    fn path(&self) -> &'static str {
        "quests/Q00119_LastImperialPrince"
    }

    fn id(&self) -> i32 {
        119
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NAMELESS_SPIRIT]);
        s.add_talk_id(&[NAMELESS_SPIRIT, DEVORIN]);
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

        let mut htmltext = None;
        match event {
            "31453-02.htm" | "31453-03.htm" | "31453-10.html" => htmltext = Some(event.to_owned()),
            "31453-04.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "31453-11.html" => {
                if qs.is_cond(ctx, 2) {
                    ctx.give_adena(player, 150_292, true);
                    ctx.add_exp_and_sp(player, 902_439, 90_067);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "brooch" => {
                htmltext = Some(
                    if ctx.has_quest_items(player, ANTIQUE_BROOCH) {
                        "32009-02.html"
                    } else {
                        "32009-03.html"
                    }
                    .to_owned(),
                );
            }
            "32009-04.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, ANTIQUE_BROOCH) {
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => {
                if npc.id == NAMELESS_SPIRIT {
                    htmltext = "31453-06.html".to_owned();
                }
            }
            CREATED => {
                htmltext =
                    if ctx.player_level(player) >= MIN_LEVEL && ctx.has_quest_items(player, ANTIQUE_BROOCH) {
                        "31453-01.htm"
                    } else {
                        "31453-05.html"
                    }
                    .to_owned();
            }
            STARTED => {
                if npc.id == NAMELESS_SPIRIT {
                    if qs.is_cond(ctx, 1) {
                        if ctx.has_quest_items(player, ANTIQUE_BROOCH) {
                            htmltext = "31453-07.html".to_owned();
                        } else {
                            htmltext = "31453-08.html".to_owned();
                            qs.exit_quest(ctx, true, false);
                        }
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "31453-09.html".to_owned();
                    }
                } else if npc.id == DEVORIN {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "32009-01.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "32009-05.html".to_owned();
                    }
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest, item, pages_to};

    const NPCS: &str = r#"
        <npc id="31453" type="Folk" level="74" name="Nameless Spirit"/>
        <npc id="32009" type="Folk" level="74" name="Nurse Devorin"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7262" type="EtcItem" name="Antique Brooch"><set name="is_questitem" val="true"/></item>"#;

    const NAME: &str = "Q00119_LastImperialPrince";

    struct Npcs {
        spirit: Oid,
        devorin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00119LastImperialPrince);
        let pages = Pages::of(&Q00119LastImperialPrince)
            .page("31453-01.htm", &["31453-02.htm"])
            .page("31453-02.htm", &["31453-03.htm"])
            .page("31453-03.htm", &["31453-04.html"])
            .plain(&["31453-04.html", "31453-05.html", "31453-06.html", "31453-07.html", "31453-08.html"])
            .page("31453-09.html", &["31453-10.html"])
            .page("31453-10.html", &["31453-11.html"])
            .plain(&["31453-11.html"])
            .file(
                "data/scripts/quests/Q00119_LastImperialPrince/32009-01.html",
                r#"<html><body>32009-01<a action="bypass -h Script Q00119_LastImperialPrince brooch">x</a></body></html>"#,
            )
            .plain(&["32009-03.html", "32009-05.html"])
            .page("32009-02.html", &["32009-04.html"])
            .plain(&["32009-04.html"])
            .npcs(&[NAMELESS_SPIRIT, DEVORIN])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            spirit: t.spawn(NAMELESS_SPIRIT, 20),
            devorin: t.spawn(DEVORIN, 40),
        };
        (t, npcs)
    }

    /// Clicks Devorin's link `bypass -h Script <name> brooch` and returns the page. The client
    /// sends the text after `-h`, so the world gets `Script <name> brooch`.
    fn show_brooch(t: &mut ScriptTest, player: i32) -> String {
        t.bypass(player, &format!("Script {NAME} brooch"));
        pages_to(&t.take(), player).pop().unwrap_or_default()
    }

    #[test]
    fn a_character_without_the_brooch_or_below_level_74_is_sent_away() {
        let (mut t, npcs) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, npcs.spirit).contains("31453-05.html"));
        t.enter(2, 73, vec![item(100, ANTIQUE_BROOCH, 1)]);
        assert!(t.quest_page(2, npcs.spirit).contains("31453-05.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn devorin_has_nothing_to_say_before_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 74, vec![item(100, ANTIQUE_BROOCH, 1)]);
        assert!(t.quest_page(1, npcs.devorin).contains("noquest"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_from_the_spirit_to_the_reward() {
        let (mut t, npcs) = setup();
        t.enter(1, 74, vec![item(100, ANTIQUE_BROOCH, 1)]);
        assert!(t.quest_page(1, npcs.spirit).contains("31453-01.htm"));
        assert!(t.event(1, NAME, "31453-02.htm").contains("31453-02.htm"));
        assert!(t.event(1, NAME, "31453-03.htm").contains("31453-03.htm"));

        // Accept: `startQuest()`.
        assert!(t.event(1, NAME, "31453-04.html").contains("31453-04.html"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );

        // Devorin shows the brooch and asks for the words (cond 1 stays until the song is told).
        assert!(t.quest_page(1, npcs.devorin).contains("32009-01"));
        assert!(show_brooch(&mut t, 1).contains("32009-02.html"));
        assert!(t.event(1, NAME, "32009-04.html").contains("32009-04.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.devorin).contains("32009-05.html"));

        // The spirit gets the lyrics, then the reward.
        assert!(t.quest_page(1, npcs.spirit).contains("31453-09.html"));
        assert!(t.event(1, NAME, "31453-10.html").contains("31453-10.html"));
        assert!(t.event(1, NAME, "31453-11.html").contains("31453-11.html"));
        assert_eq!(t.count(1, 57), 150_292);
        assert_eq!(
            t.take_writes(),
            [delete(NAME, false), update(NAME, "<state>", "Completed")]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(t.quest_page(1, npcs.spirit).contains("31453-06.html"));
    }

    #[test]
    fn without_the_brooch_devorin_says_so_and_the_spirit_gives_up_the_quest() {
        let (mut t, npcs) = setup();
        t.enter(1, 74, vec![item(100, ANTIQUE_BROOCH, 1)]);
        t.quest_page(1, npcs.spirit);
        t.event(1, NAME, "31453-02.htm");
        t.event(1, NAME, "31453-03.htm");
        t.event(1, NAME, "31453-04.html");
        t.take_writes();

        // The brooch is gone: Devorin refuses, the spirit's `exitQuest(true)` is repeatable.
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), ANTIQUE_BROOCH, -1);
        });
        assert!(t.quest_page(1, npcs.devorin).contains("32009-01"));
        assert!(show_brooch(&mut t, 1).contains("32009-03.html"));
        assert!(t.quest_page(1, npcs.spirit).contains("31453-08.html"));
        assert_eq!(t.take_writes(), [delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
