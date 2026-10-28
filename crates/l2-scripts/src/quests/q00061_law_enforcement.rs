//! Quest 61: Law Enforcement (Liane the Inspector's trainee, Kekropus and Eindburgh in the Town
//! of Schuttgart; the Judicator class is the reward).
// Java: dist/game/data/scripts/quests/Q00061_LawEnforcement/Q00061_LawEnforcement.java
//
// Pattern: a story quest with the memo state as its step (`docs/design/scripts.md`, "Quest
// patterns"): the memo state counts the talks with Kekropus and Eindburgh, the page each NPC
// shows follows the memo state, and the reward is a class change to Judicator with 26000 adena.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LIANE: i32 = 32222;
const KEKROPUS: i32 = 32138;
const EINDBURGH: i32 = 32469;

// Misc
const MIN_LEVEL: i32 = 76;

/// Java `giveAdena(player, 26000, true)`.
const ADENA_REWARD: i64 = 26_000;

/// Java `Q00061_LawEnforcement`.
pub struct Q00061LawEnforcement;

impl Script for Q00061LawEnforcement {
    fn name(&self) -> &'static str {
        "Q00061_LawEnforcement"
    }

    fn path(&self) -> &'static str {
        "quests/Q00061_LawEnforcement"
    }

    fn id(&self) -> i32 {
        61
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LIANE]);
        s.add_talk_id(&[LIANE, KEKROPUS, EINDBURGH]);
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
            "32222-02.htm" => {
                htmltext = Some(event.to_owned());
            }
            "32222-03.htm" => {
                qs.set_memo_state(ctx, 1);
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "32138-01.html" | "32138-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-03.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-04.html" | "32138-05.html" | "32138-06.html" | "32138-07.html" => {
                if qs.is_memo_state(ctx, 2) || qs.is_memo_state(ctx, 3) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-08.html" => {
                if qs.is_memo_state(ctx, 2) || qs.is_memo_state(ctx, 3) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32138-09.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "32469-02.html" => {
                if qs.is_memo_state(ctx, 4) {
                    qs.set_memo_state(ctx, 5);
                    htmltext = Some(event.to_owned());
                }
            }
            "32469-03.html" | "32469-04.html" | "32469-05.html" | "32469-06.html" | "32469-07.html" => {
                if qs.is_memo_state(ctx, 5) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32469-08.html" | "32469-09.html" => {
                if qs.is_memo_state(ctx, 5) {
                    // `player.setPlayerClass(136)`: the Judicator.
                    ctx.set_player_class(player, player_class::JUDICATOR);
                    // SystemMessage and cast skill is done by setClassId.
                    ctx.broadcast_user_info(player);
                    ctx.give_adena(player, ADENA_REWARD, true);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = Some(ctx.get_no_quest_msg(player));
        if qs.state(ctx) == COMPLETED && npc.id == LIANE {
            htmltext = Some(ctx.get_already_completed_msg(player));
        } else if qs.state(ctx) == CREATED {
            if ctx.player_level(player) >= MIN_LEVEL {
                if ctx.player_class_id(player) == player_class::INSPECTOR {
                    // Java: `getHtm(player, "32222-01.htm").replace("%name%", player.getName())`.
                    let name = ctx.player_name(player);
                    return ctx
                        .get_htm(player, "32222-01.htm")
                        .map(|page| page.replace("%name%", &name));
                }
                htmltext = Some("32222-04.htm".to_owned());
            } else {
                htmltext = Some("32222-05.htm".to_owned());
            }
        } else if qs.state(ctx) == STARTED {
            match npc.id {
                LIANE => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = Some("32222-06.html".to_owned());
                    }
                }
                KEKROPUS => match qs.get_memo_state(ctx) {
                    1 => htmltext = Some("32138-01.html".to_owned()),
                    2 => htmltext = Some("32138-03.html".to_owned()),
                    3 => htmltext = Some("32138-10.html".to_owned()),
                    4 => htmltext = Some("32138-10.html".to_owned()),
                    _ => {}
                },
                EINDBURGH => {
                    if qs.is_memo_state(ctx, 4) {
                        let name = ctx.player_name(player);
                        return ctx
                            .get_htm(player, "32469-01.html")
                            .map(|page| page.replace("%name%", &name));
                    }
                    if qs.is_memo_state(ctx, 5) {
                        htmltext = Some("32469-02.html".to_owned());
                    }
                }
                _ => {}
            }
        }
        htmltext
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00061_LawEnforcement";

    const NPCS: &str = r#"
        <npc id="32222" type="Folk" level="70" name="Liane"/>
        <npc id="32138" type="Folk" level="70" name="Kekropus"/>
        <npc id="32469" type="Folk" level="70" name="Eindburgh"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        liane: Oid,
        kekropus: Oid,
        eindburgh: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00061LawEnforcement);
        let pages = Pages::of(&Q00061LawEnforcement)
            .page("32222-01.htm", &["32222-02.htm"])
            .page("32222-02.htm", &["32222-03.htm"])
            .page("32138-01.html", &["32138-02.html"])
            .page("32138-02.html", &["32138-03.html", "32138-09.html"])
            .page("32138-03.html", &["32138-04.html"])
            .page("32138-04.html", &["32138-05.html"])
            .page("32138-05.html", &["32138-06.html"])
            .page("32138-06.html", &["32138-07.html"])
            .page("32138-07.html", &["32138-08.html"])
            .page("32138-09.html", &["32138-04.html"])
            .page("32469-01.html", &["32469-02.html"])
            .page("32469-02.html", &["32469-03.html"])
            .page("32469-03.html", &["32469-04.html"])
            .page("32469-04.html", &["32469-05.html"])
            .page("32469-05.html", &["32469-06.html"])
            .page("32469-06.html", &["32469-07.html"])
            .page("32469-07.html", &["32469-08.html", "32469-09.html"])
            .plain(&[
                "32222-03.htm",
                "32222-04.htm",
                "32222-05.htm",
                "32222-06.html",
                "32138-08.html",
                "32138-10.html",
                "32469-08.html",
                "32469-09.html",
            ])
            .npcs(&[LIANE, KEKROPUS, EINDBURGH]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            liane: t.spawn(LIANE, 20),
            kekropus: t.spawn(KEKROPUS, 40),
            eindburgh: t.spawn(EINDBURGH, 60),
        };
        (t, npcs)
    }

    /// Calls a script event directly (for the refusals whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00061LawEnforcement.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Sets the player's memo state directly (for the branches reached without the chain).
    fn set_memo(t: &mut ScriptTest, memo: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, memo);
            }
        });
        t.take_writes();
    }

    /// An inspector (`PlayerClass.INSPECTOR`, id 135) of `level`, human.
    fn enter_inspector(t: &mut ScriptTest, level: i32) {
        t.enter_as(1, level, 0, player_class::INSPECTOR, vec![]);
    }

    fn memo(value: &str) -> l2_world::QuestWrite {
        rows::update(NAME, "memoState", value)
    }

    #[test]
    fn the_level_and_the_inspector_class_decide_the_first_page() {
        let (mut t, npcs) = setup();
        enter_inspector(&mut t, 75);
        assert!(t.quest_page(1, npcs.liane).contains("32222-05.htm"));
        assert!(t.take_writes().is_empty());

        // Level 76 but a fighter: the other page.
        let (mut t, npcs) = setup();
        t.enter_as(1, 76, 0, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.liane).contains("32222-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_inspector_story_to_the_judicator_class() {
        let (mut t, npcs) = setup();
        enter_inspector(&mut t, 76);
        assert!(t.quest_page(1, npcs.liane).contains("32222-01.htm"));
        assert!(t.event(1, NAME, "32222-02.htm").contains("32222-02.htm"));
        assert!(t.event(1, NAME, "32222-03.htm").contains("32222-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "memoState", "1"),
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, npcs.liane).contains("32222-06.html"));

        // Kekropus: "I know" moves the memo state to 2; the story pages do not write.
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-01.html"));
        assert!(t.event(1, NAME, "32138-02.html").contains("32138-02.html"));
        assert!(t.event(1, NAME, "32138-03.html").contains("32138-03.html"));
        assert_eq!(t.take_writes(), [memo("2")]);
        assert!(t.quest_page(1, npcs.kekropus).contains("32138-03.html"));
        for page in ["32138-04.html", "32138-05.html", "32138-06.html", "32138-07.html"] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        assert!(t.take_writes().is_empty());

        // "Ask what to do": memo state 4 and cond 2.
        assert!(t.event(1, NAME, "32138-08.html").contains("32138-08.html"));
        assert_eq!(t.take_writes(), [memo("4"), rows::update(NAME, "cond", "2")]);

        // Eindburgh's page names the player; the chain of his pages moves the memo state to 5.
        assert!(t.quest_page(1, npcs.eindburgh).contains("32469-01.html"));
        assert!(t.event(1, NAME, "32469-02.html").contains("32469-02.html"));
        assert_eq!(t.take_writes(), [memo("5")]);
        for page in [
            "32469-03.html",
            "32469-04.html",
            "32469-05.html",
            "32469-06.html",
            "32469-07.html",
        ] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        assert!(t.take_writes().is_empty());

        // "Nod your head yes": the Judicator class, 26000 adena, the end.
        assert!(t.event(1, NAME, "32469-08.html").contains("32469-08.html"));
        assert_eq!(
            t.world.player(Oid(1)).unwrap().data.class_id,
            player_class::JUDICATOR
        );
        assert_eq!(t.count(1, 57), 26_000);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, npcs.liane)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_kekropus_pages_need_the_right_memo_state() {
        let (mut t, npcs) = setup();
        enter_inspector(&mut t, 76);
        t.quest_page(1, npcs.liane);
        t.event(1, NAME, "32222-02.htm");
        t.event(1, NAME, "32222-03.htm");
        t.quest_page(1, npcs.liane);
        t.take_writes();

        // At memo state 2 the "I know" page is not offered any more (`isMemoState(1)` fails).
        set_memo(&mut t, 2);
        assert_eq!(direct(&mut t, "32138-03.html"), None);
        assert!(t.take_writes().is_empty());
        let page = t.quest_page(1, npcs.kekropus);
        assert!(page.contains("32138-03.html"), "{page}");
    }
}
