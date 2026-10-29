//! Quest 192: Seven Signs, Series of Doubt (Croop, Hector, Stan, Hollint and the unidentified body).
// Java: dist/game/data/scripts/quests/Q00192_SevenSignsSeriesOfDoubt/Q00192_SevenSignsSeriesOfDoubt.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HOLLINT: i32 = 30191;
const HECTOR: i32 = 30197;
const STAN: i32 = 30200;
const CROOP: i32 = 30676;
const UNIDENTIFIED_BODY: i32 = 32568;

// Items
const CROOPS_INTRODUCTION: i32 = 13813;
const JACOBS_NECKLACE: i32 = 13814;
const CROOPS_LETTER: i32 = 13815;

// Misc
const MIN_LEVEL: i32 = 79;
/// Java `Movie.SSQ_SUSPICIOUS_DEATHS`.
const SSQ_SUSPICIOUS_DEATHS: i32 = 8;

/// Java `player.teleToLocation(81654, 54851, -1513)` (the 3-argument overload: instance -1, no offset).
const TELEPORT_X: i32 = 81_654;
const TELEPORT_Y: i32 = 54_851;
const TELEPORT_Z: i32 = -1_513;

/// Java `Q00192_SevenSignsSeriesOfDoubt`.
pub struct Q00192SevenSignsSeriesOfDoubt;

impl Script for Q00192SevenSignsSeriesOfDoubt {
    fn name(&self) -> &'static str {
        "Q00192_SevenSignsSeriesOfDoubt"
    }

    fn path(&self) -> &'static str {
        "quests/Q00192_SevenSignsSeriesOfDoubt"
    }

    fn id(&self) -> i32 {
        192
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[CROOP, UNIDENTIFIED_BODY]);
        s.add_talk_id(&[CROOP, STAN, UNIDENTIFIED_BODY, HECTOR, HOLLINT]);
        s.register_quest_items(&[CROOPS_INTRODUCTION, JACOBS_NECKLACE, CROOPS_LETTER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "30676-02.htm" => htmltext = Some(event.to_owned()),
            "30676-03.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "video" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_cond(ctx, 2, true);
                    ctx.play_movie(player, SSQ_SUSPICIOUS_DEATHS);
                    ctx.start_quest_timer("back", 32_000, npc, Some(player), false);
                    // Java `return ""`: a page with no text, nothing is shown.
                    return Some(String::new());
                }
            }
            "back" => {
                ctx.teleport_player(player, TELEPORT_X, TELEPORT_Y, TELEPORT_Z, -1, false);
                return Some(String::new());
            }
            "30676-10.html" | "30676-11.html" | "30676-12.html" | "30676-13.html" => {
                if qs.is_cond(ctx, 6) && ctx.has_quest_items(player, JACOBS_NECKLACE) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30676-14.html" => {
                if qs.is_cond(ctx, 6) && ctx.has_quest_items(player, JACOBS_NECKLACE) {
                    ctx.give_items(player, CROOPS_LETTER, 1);
                    ctx.take_items(player, JACOBS_NECKLACE, -1);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30200-02.html" | "30200-03.html" => {
                if qs.is_cond(ctx, 4) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30200-04.html" => {
                if qs.is_cond(ctx, 4) {
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32568-02.html" => {
                if qs.is_cond(ctx, 5) {
                    ctx.give_items(player, JACOBS_NECKLACE, 1);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30197-02.html" => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, CROOPS_INTRODUCTION) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30197-03.html" => {
                if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, CROOPS_INTRODUCTION) {
                    ctx.take_items(player, CROOPS_INTRODUCTION, -1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30191-02.html" => {
                if qs.is_cond(ctx, 7) && ctx.has_quest_items(player, CROOPS_LETTER) {
                    htmltext = Some(event.to_owned());
                }
            }
            "reward" => {
                if qs.is_cond(ctx, 7) && ctx.has_quest_items(player, CROOPS_LETTER) {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        ctx.add_exp_and_sp(player, 52_518_015, 5_817_677);
                        qs.exit_quest(ctx, false, true);
                        htmltext = Some("30191-03.html".to_owned());
                    } else {
                        htmltext = Some("level_check.html".to_owned());
                    }
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
                if npc.id == CROOP {
                    htmltext = "30676-05.html".to_owned();
                } else if npc.id == UNIDENTIFIED_BODY {
                    htmltext = "32568-04.html".to_owned();
                }
            }
            CREATED => {
                if npc.id == CROOP {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30676-01.htm"
                    } else {
                        "30676-04.html"
                    }
                    .to_owned();
                } else if npc.id == UNIDENTIFIED_BODY {
                    htmltext = "32568-04.html".to_owned();
                }
            }
            STARTED => match npc.id {
                CROOP => match qs.get_cond(ctx) {
                    1 => htmltext = "30676-06.html".to_owned(),
                    2 => {
                        ctx.give_items(player, CROOPS_INTRODUCTION, 1);
                        qs.set_cond(ctx, 3, true);
                        htmltext = "30676-07.html".to_owned();
                    }
                    3..=5 => htmltext = "30676-08.html".to_owned(),
                    6 => {
                        if ctx.has_quest_items(player, JACOBS_NECKLACE) {
                            htmltext = "30676-09.html".to_owned();
                        }
                    }
                    _ => {}
                },
                HECTOR => {
                    if qs.is_cond(ctx, 3) {
                        if ctx.has_quest_items(player, CROOPS_INTRODUCTION) {
                            htmltext = "30197-01.html".to_owned();
                        }
                    } else if qs.get_cond(ctx) > 3 {
                        htmltext = "30197-04.html".to_owned();
                    }
                }
                STAN => {
                    if qs.is_cond(ctx, 4) {
                        htmltext = "30200-01.html".to_owned();
                    } else if qs.get_cond(ctx) > 4 {
                        htmltext = "30200-05.html".to_owned();
                    }
                }
                UNIDENTIFIED_BODY => {
                    if qs.is_cond(ctx, 5) {
                        htmltext = "32568-01.html".to_owned();
                    } else if qs.get_cond(ctx) < 5 {
                        htmltext = "32568-03.html".to_owned();
                    }
                }
                HOLLINT => {
                    if qs.is_cond(ctx, 7) && ctx.has_quest_items(player, CROOPS_LETTER) {
                        htmltext = "30191-01.html".to_owned();
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
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00192_SevenSignsSeriesOfDoubt";

    const NPCS: &str = r#"
        <npc id="30191" type="Folk" level="80" name="Hollint"/>
        <npc id="30197" type="Folk" level="80" name="Hector"/>
        <npc id="30200" type="Folk" level="80" name="Stan"/>
        <npc id="30676" type="Folk" level="80" name="Croop"/>
        <npc id="32568" type="Folk" level="80" name="Unidentified Body"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13813" type="EtcItem" name="Croop's Introduction"><set name="is_questitem" val="true"/></item>
        <item id="13814" type="EtcItem" name="Jacob's Necklace"><set name="is_questitem" val="true"/></item>
        <item id="13815" type="EtcItem" name="Croop's Letter"><set name="is_questitem" val="true"/></item>"#;

    struct Npcs {
        croop: Oid,
        hector: Oid,
        stan: Oid,
        body: Oid,
        hollint: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00192SevenSignsSeriesOfDoubt);
        let pages = Pages::of(&Q00192SevenSignsSeriesOfDoubt)
            .page("30676-01.htm", &["30676-02.htm"])
            .page("30676-02.htm", &["30676-03.html"])
            .page("30676-03.html", &["video"])
            .page("30676-06.html", &["video"])
            .plain(&[
                "30676-04.html",
                "30676-05.html",
                "30676-07.html",
                "30676-08.html",
                "30676-09.html",
            ])
            .page("30676-09.html", &["30676-10.html"])
            .page("30676-10.html", &["30676-11.html"])
            .page("30676-11.html", &["30676-12.html"])
            .page("30676-12.html", &["30676-13.html"])
            .page("30676-13.html", &["30676-14.html"])
            .plain(&["30676-14.html", "30197-01.html", "30197-04.html", "30200-05.html"])
            .page("30197-01.html", &["30197-02.html"])
            .page("30197-02.html", &["30197-03.html"])
            .plain(&["30197-03.html"])
            .page("30200-01.html", &["30200-02.html"])
            .page("30200-02.html", &["30200-03.html"])
            .page("30200-03.html", &["30200-04.html"])
            .plain(&["30200-04.html"])
            .page("32568-01.html", &["32568-02.html"])
            .plain(&["32568-02.html", "32568-03.html", "32568-04.html"])
            .page("30191-01.html", &["30191-02.html"])
            .page("30191-02.html", &["reward"])
            .plain(&["30191-03.html", "level_check.html"])
            .npcs(&[CROOP, HECTOR, STAN, UNIDENTIFIED_BODY, HOLLINT]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            croop: t.spawn(CROOP, 20),
            hector: t.spawn(HECTOR, 40),
            stan: t.spawn(STAN, 60),
            body: t.spawn(UNIDENTIFIED_BODY, 80),
            hollint: t.spawn(HOLLINT, 100),
        };
        (t, npcs)
    }

    #[test]
    fn only_a_character_of_level_79_is_offered_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 78, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.croop).contains("30676-04.html"));
        t.enter_as(2, 79, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(2, npcs.croop).contains("30676-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_series_from_croop_to_hollint_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 79, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.croop).contains("30676-01.htm"));
        assert!(t.event(1, NAME, "30676-02.htm").contains("30676-02.htm"));
        assert!(t.event(1, NAME, "30676-03.html").contains("30676-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // The video: cond 2, and after 32 s the player is taken to the Hideout.
        assert!(t.quest_page(1, npcs.croop).contains("30676-06.html"));
        assert!(t.event(1, NAME, "video").is_empty());
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        t.wait_ms(32_000);
        // The z is corrected to the ground by the teleport (no geodata here): x and y are the target's.
        assert_eq!(
            t.world.position_of(Oid(1)).map(|(x, y, _)| (x, y)),
            Some((TELEPORT_X, TELEPORT_Y))
        );
        // Back to the NPCs for the rest of the test.
        t.world.run_as_script(NAME, |ctx| {
            ctx.teleport_player(
                Oid(1),
                crate::testing::X,
                crate::testing::Y,
                crate::testing::Z,
                -1,
                false,
            );
        });
        t.take();

        // Croop: the introduction, cond 3.
        assert!(t.quest_page(1, npcs.croop).contains("30676-07.html"));
        assert_eq!(t.count(1, CROOPS_INTRODUCTION), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Hector takes the introduction, cond 4.
        assert!(t.quest_page(1, npcs.hector).contains("30197-01.html"));
        assert!(t.event(1, NAME, "30197-02.html").contains("30197-02.html"));
        assert!(t.event(1, NAME, "30197-03.html").contains("30197-03.html"));
        assert_eq!(t.count(1, CROOPS_INTRODUCTION), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Stan: cond 5.
        assert!(t.quest_page(1, npcs.stan).contains("30200-01.html"));
        assert!(t.event(1, NAME, "30200-02.html").contains("30200-02.html"));
        assert!(t.event(1, NAME, "30200-03.html").contains("30200-03.html"));
        assert!(t.event(1, NAME, "30200-04.html").contains("30200-04.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);

        // The unidentified body: Jacob's necklace, cond 6.
        assert!(t.quest_page(1, npcs.body).contains("32568-01.html"));
        assert!(t.event(1, NAME, "32568-02.html").contains("32568-02.html"));
        assert_eq!(t.count(1, JACOBS_NECKLACE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Croop takes the necklace back and gives the letter, cond 7.
        assert!(t.quest_page(1, npcs.croop).contains("30676-09.html"));
        for page in [
            "30676-10.html",
            "30676-11.html",
            "30676-12.html",
            "30676-13.html",
            "30676-14.html",
        ] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        assert_eq!((t.count(1, JACOBS_NECKLACE), t.count(1, CROOPS_LETTER)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);

        // Hollint: the reward (level 79), `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.hollint).contains("30191-01.html"));
        assert!(t.event(1, NAME, "30191-02.html").contains("30191-02.html"));
        assert!(t.event(1, NAME, "reward").contains("30191-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.croop).contains("30676-05.html"));
    }
}
