//! Quest 362: Bard's Mandolin (Swan, Galion, Woodrow and Nanarin in the Town of Gludin).
// Java: dist/game/data/scripts/quests/Q00362_BardsMandolin/Q00362_BardsMandolin.java
//
// Pattern: a delivery chain over four NPCs, the memo state is the stage (`docs/design/scripts.md`,
// "Quest patterns"): 1 = Woodrow to visit, 2 = Galion to visit, 3 = Swan's letter, 4 = Nanarin's
// flute and letter, 5 = done. The reward is adena and the theme of the journey.

use l2_util::ObjectId;
use l2_world::script::STARTED;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const WOODROW: i32 = 30837;
const NANARIN: i32 = 30956;
const SWAN: i32 = 30957;
const GALION: i32 = 30958;

// Items
const SWANS_FLUTE: i32 = 4316;
const SWANS_LETTER: i32 = 4317;
const THEME_OF_JOURNEY: i32 = 4410;

// Misc
const MIN_LEVEL: i32 = 15;

/// Java `Q00362_BardsMandolin`.
pub struct Q00362BardsMandolin;

impl Script for Q00362BardsMandolin {
    fn name(&self) -> &'static str {
        "Q00362_BardsMandolin"
    }

    fn path(&self) -> &'static str {
        "quests/Q00362_BardsMandolin"
    }

    fn id(&self) -> i32 {
        362
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SWAN]);
        s.add_talk_id(&[SWAN, GALION, WOODROW, NANARIN]);
        s.register_quest_items(&[SWANS_FLUTE, SWANS_LETTER]);
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
            "30957-02.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "30957-07.html" | "30957-08.html" => {
                if qs.is_memo_state(ctx, 5) {
                    ctx.give_adena(player, 10000, true);
                    ctx.reward_items(player, THEME_OF_JOURNEY, 1);
                    qs.exit_quest(ctx, true, true);
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
        if qs.is_created(ctx) {
            if npc.id == SWAN {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30957-01.htm"
                } else {
                    "30957-03.html"
                }
                .to_owned();
            }
        } else if qs.state(ctx) == STARTED {
            match npc.id {
                SWAN => match qs.get_memo_state(ctx) {
                    1 | 2 => htmltext = "30957-04.html".to_owned(),
                    3 => {
                        qs.set_cond(ctx, 4, true);
                        qs.set_memo_state(ctx, 4);
                        ctx.give_items(player, SWANS_LETTER, 1);
                        htmltext = "30957-05.html".to_owned();
                    }
                    4 => htmltext = "30957-05.html".to_owned(),
                    5 => htmltext = "30957-06.html".to_owned(),
                    _ => {}
                },
                GALION => {
                    if qs.is_memo_state(ctx, 2) {
                        qs.set_memo_state(ctx, 3);
                        qs.set_cond(ctx, 3, true);
                        ctx.give_items(player, SWANS_FLUTE, 1);
                        htmltext = "30958-01.html".to_owned();
                    } else if qs.get_memo_state(ctx) >= 3 {
                        htmltext = "30958-02.html".to_owned();
                    }
                }
                WOODROW => {
                    if qs.is_memo_state(ctx, 1) {
                        qs.set_memo_state(ctx, 2);
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30837-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 2) {
                        htmltext = "30837-02.html".to_owned();
                    } else if qs.get_memo_state(ctx) >= 3 {
                        htmltext = "30837-03.html".to_owned();
                    }
                }
                NANARIN => {
                    if qs.is_memo_state(ctx, 4)
                        && ctx.has_all_quest_items(player, &[SWANS_FLUTE, SWANS_LETTER])
                    {
                        qs.set_memo_state(ctx, 5);
                        qs.set_cond(ctx, 5, true);
                        ctx.take_items(player, SWANS_FLUTE, -1);
                        ctx.take_items(player, SWANS_LETTER, -1);
                        htmltext = "30956-01.html".to_owned();
                    } else if qs.get_memo_state(ctx) >= 5 {
                        htmltext = "30956-02.html".to_owned();
                    }
                }
                _ => {}
            }
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

    const NAME: &str = "Q00362_BardsMandolin";

    const NPCS: &str = r#"
        <npc id="30837" type="Folk" level="50" name="Woodrow"/>
        <npc id="30956" type="Folk" level="50" name="Nanarin"/>
        <npc id="30957" type="Folk" level="50" name="Swan"/>
        <npc id="30958" type="Folk" level="50" name="Galion"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4316" type="EtcItem" name="Swan's Flute"><set name="is_questitem" val="true"/></item>
        <item id="4317" type="EtcItem" name="Swan's Letter"><set name="is_questitem" val="true"/></item>
        <item id="4410" type="EtcItem" name="Theme of Journey"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00362BardsMandolin);
        let pages = Pages::of(&Q00362BardsMandolin)
            .page("30957-01.htm", &["30957-02.htm"])
            .plain(&[
                "30957-02.htm",
                "30957-03.html",
                "30957-04.html",
                "30957-05.html",
                "30837-01.html",
                "30837-02.html",
                "30837-03.html",
                "30958-01.html",
                "30958-02.html",
                "30956-01.html",
                "30956-02.html",
            ])
            .page("30957-06.html", &["30957-07.html", "30957-08.html"])
            .plain(&["30957-07.html", "30957-08.html"])
            .npcs(&[WOODROW, NANARIN, SWAN, GALION]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let swan = t.spawn(SWAN, 20);
        let woodrow = t.spawn(WOODROW, 40);
        let galion = t.spawn(GALION, 60);
        let nanarin = t.spawn(NANARIN, 80);
        (t, swan, woodrow, galion, nanarin)
    }

    #[test]
    fn a_level_14_character_is_sent_away() {
        let (mut t, swan, ..) = setup();
        t.enter(1, 14, vec![]);
        assert!(t.quest_page(1, swan).contains("30957-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_four_visits_and_the_theme_of_the_journey() {
        let (mut t, swan, woodrow, galion, nanarin) = setup();
        t.enter(1, 15, vec![]);
        assert!(t.quest_page(1, swan).contains("30957-01.htm"));
        assert!(t.event(1, NAME, "30957-02.htm").contains("30957-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // Woodrow (memo 1 -> 2), Galion (memo 2 -> 3, the flute), Swan (memo 3 -> 4, the letter).
        assert!(t.quest_page(1, woodrow).contains("30837-01.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, galion).contains("30958-01.html"));
        assert_eq!(t.count(1, 4316), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3")
            ]
        );
        assert!(t.quest_page(1, swan).contains("30957-05.html"));
        assert_eq!(t.count(1, 4317), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "4"),
                rows::update(NAME, "memoState", "4")
            ]
        );

        // Nanarin takes the flute and the letter: memo 5, cond 5.
        assert!(t.quest_page(1, nanarin).contains("30956-01.html"));
        assert_eq!((t.count(1, 4316), t.count(1, 4317)), (0, 0));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "5")
            ]
        );

        // Swan's reward: 10000 adena, the theme of the journey, the quest ends repeatable.
        assert!(t.quest_page(1, swan).contains("30957-06.html"));
        assert!(t.event(1, NAME, "30957-07.html").contains("30957-07.html"));
        assert_eq!(t.count(1, 57), 10000);
        assert_eq!(t.count(1, THEME_OF_JOURNEY), 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
