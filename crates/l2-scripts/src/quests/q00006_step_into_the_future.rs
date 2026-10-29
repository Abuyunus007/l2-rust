//! Quest 6: Step Into the Future (Roxxy in Talking Island sends a human to Baulro and Sir Collin).
// Java: dist/game/data/scripts/quests/Q00006_StepIntoTheFuture/Q00006_StepIntoTheFuture.java
//
// Pattern: a delivery chain (`docs/design/scripts.md`, "Quest patterns"): Roxxy starts it, Baulro
// hands out the letter (cond 2), Sir Collin takes it (cond 3), and Roxxy pays the reward items.
// One-time: `exit_quest(ctx, false, true)`.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ROXXY: i32 = 30006;
const BAULRO: i32 = 30033;
const SIR_COLLIN: i32 = 30311;

// Items
const BAULRO_LETTER: i32 = 7571;
const SCROLL_OF_ESCAPE_GIRAN: i32 = 7559;
const MARK_OF_TRAVELER: i32 = 7570;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `Q00006_StepIntoTheFuture`.
pub struct Q00006StepIntoTheFuture;

impl Script for Q00006StepIntoTheFuture {
    fn name(&self) -> &'static str {
        "Q00006_StepIntoTheFuture"
    }

    fn path(&self) -> &'static str {
        "quests/Q00006_StepIntoTheFuture"
    }

    fn id(&self) -> i32 {
        6
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ROXXY]);
        s.add_talk_id(&[ROXXY, BAULRO, SIR_COLLIN]);
        s.register_quest_items(&[BAULRO_LETTER]);
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
            "30006-03.htm" => qs.start_quest(ctx),
            "30006-06.html" => {
                ctx.give_items(player, SCROLL_OF_ESCAPE_GIRAN, 1);
                ctx.give_items(player, MARK_OF_TRAVELER, 1);
                qs.exit_quest(ctx, false, true);
            }
            "30033-02.html" => {
                qs.set_cond(ctx, 2, true);
                ctx.give_items(player, BAULRO_LETTER, 1);
            }
            "30311-02.html" => {
                if !ctx.has_quest_items(player, BAULRO_LETTER) {
                    return Some("30311-03.html".to_owned());
                }

                ctx.take_items(player, BAULRO_LETTER, -1);
                qs.set_cond(ctx, 3, true);
            }
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ROXXY => match qs.state(ctx) {
                CREATED => {
                    htmltext =
                        if ctx.player_race(player) == race::HUMAN && ctx.player_level(player) >= MIN_LEVEL {
                            "30006-02.htm"
                        } else {
                            "30006-01.html"
                        }
                        .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30006-04.html".to_owned();
                    } else if qs.is_cond(ctx, 3) {
                        htmltext = "30006-05.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            BAULRO => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "30033-01.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "30033-03.html".to_owned();
                    }
                }
            }
            SIR_COLLIN => {
                if qs.is_started(ctx) {
                    if qs.is_cond(ctx, 2) {
                        htmltext = "30311-01.html".to_owned();
                    } else if qs.is_cond(ctx, 3) {
                        htmltext = "30311-04.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00006_StepIntoTheFuture";

    const NPCS: &str = r#"
        <npc id="30006" type="Folk" level="70" name="Roxxy"/>
        <npc id="30033" type="Folk" level="70" name="Baulro"/>
        <npc id="30311" type="Folk" level="70" name="Sir Collin"/>"#;

    const ITEMS: &str = r#"
        <item id="7571" type="EtcItem" name="Baulro's Letter"><set name="is_questitem" val="true"/></item>
        <item id="7559" type="EtcItem" name="Scroll of Escape: Giran"/>
        <item id="7570" type="EtcItem" name="Mark of Traveler"/>"#;

    struct Npcs {
        roxxy: Oid,
        baulro: Oid,
        sir_collin: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00006StepIntoTheFuture);
        let pages = Pages::of(&Q00006StepIntoTheFuture)
            .page("30006-02.htm", &["30006-03.htm"])
            .page("30006-05.html", &["30006-06.html"])
            .page("30033-01.html", &["30033-02.html"])
            .page("30311-01.html", &["30311-02.html"])
            .plain(&[
                "30006-01.html",
                "30006-03.htm",
                "30006-04.html",
                "30006-06.html",
                "30033-02.html",
                "30033-03.html",
                "30311-02.html",
                "30311-03.html",
                "30311-04.html",
            ])
            .npcs(&[ROXXY, BAULRO, SIR_COLLIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            roxxy: t.spawn(ROXXY, 20),
            baulro: t.spawn(BAULRO, 40),
            sir_collin: t.spawn(SIR_COLLIN, 60),
        };
        (t, npcs)
    }

    #[test]
    fn only_a_human_of_level_three_may_start() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 5, race::ELF, 0, vec![]);
        assert!(t.quest_page(1, npcs.roxxy).contains("30006-01.html"));
        t.enter_as(2, 2, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(2, npcs.roxxy).contains("30006-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn baulro_and_sir_collin_then_the_scroll_and_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.roxxy).contains("30006-02.htm"));
        assert!(t.event(1, NAME, "30006-03.htm").contains("30006-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Baulro gives the letter (cond 2).
        assert!(t.quest_page(1, npcs.baulro).contains("30033-01.html"));
        assert!(t.event(1, NAME, "30033-02.html").contains("30033-02.html"));
        assert_eq!(t.count(1, BAULRO_LETTER), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.baulro).contains("30033-03.html"));

        // Sir Collin takes the letter (cond 3).
        assert!(t.quest_page(1, npcs.sir_collin).contains("30311-01.html"));
        assert!(t.event(1, NAME, "30311-02.html").contains("30311-02.html"));
        assert_eq!(t.count(1, BAULRO_LETTER), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
        assert!(t.quest_page(1, npcs.sir_collin).contains("30311-04.html"));

        // Roxxy: the scroll and the mark, `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.roxxy).contains("30006-05.html"));
        assert!(t.event(1, NAME, "30006-06.html").contains("30006-06.html"));
        assert_eq!(
            (t.count(1, SCROLL_OF_ESCAPE_GIRAN), t.count(1, MARK_OF_TRAVELER)),
            (1, 1)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.roxxy)
                .contains("This quest has already been completed.")
        );
    }
}
