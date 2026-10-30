//! Quest 250: Watch What You Eat (Sally in the Hunter's Village collects the three kinds of
//! monster meat from the Hunting Grounds, one of each).
// Java: dist/game/data/scripts/quests/Q00250_WatchWhatYouEat/Q00250_WatchWhatYouEat.java
//
// Pattern: one item per kind of monster, each taken once (`docs/design/scripts.md`, "Kills,
// drops and parties"), and a first-talk page of the start NPC (`add_first_talk_id`).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const SALLY: i32 = 32743;

// Mobs and their items: (monster, item)
const MOBS: [(i32, i32); 3] = [(18864, 15493), (18865, 15494), (18868, 15495)];

/// Java `Q00250_WatchWhatYouEat`.
pub struct Q00250WatchWhatYouEat;

impl Script for Q00250WatchWhatYouEat {
    fn name(&self) -> &'static str {
        "Q00250_WatchWhatYouEat"
    }

    fn path(&self) -> &'static str {
        "quests/Q00250_WatchWhatYouEat"
    }

    fn id(&self) -> i32 {
        250
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SALLY]);
        s.add_first_talk_id(&[SALLY]);
        s.add_talk_id(&[SALLY]);
        for (mob, _) in MOBS {
            s.add_kill_id(&[mob]);
        }
        s.register_quest_items(&[15493, 15494, 15495]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = event.to_owned();
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(htmltext);
        };
        // Java reads `npc.getId()` without a null check (a bypass always has the NPC).
        let npc = npc?;
        if npc.id == SALLY {
            if event.eq_ignore_ascii_case("32743-03.htm") {
                qs.start_quest(ctx);
            } else if event.eq_ignore_ascii_case("32743-end.htm") {
                // NOTE(java-quirk): the reward is not checked against the cond or the items: any
                // click on this link pays (`exitQuest` does nothing for a quest not started).
                ctx.give_adena(player, 135661, true);
                ctx.add_exp_and_sp(player, 698334, 76369);
                qs.exit_quest(ctx, false, true);
            } else if event.eq_ignore_ascii_case("32743-22.html") && qs.is_completed(ctx) {
                htmltext = "32743-23.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        if npc.id == SALLY {
            Some("32743-20.html".to_owned())
        } else {
            None
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx) && qs.is_cond(ctx, 1) {
            for (mob, item) in MOBS {
                if npc.id == mob && !ctx.has_quest_items(killer, item) {
                    ctx.give_items(killer, item, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
            if MOBS.iter().all(|(_, item)| ctx.has_quest_items(killer, *item)) {
                qs.set_cond(ctx, 2, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == SALLY {
            match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= 82 {
                        "32743-01.htm"
                    } else {
                        "32743-00.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "32743-04.htm".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        if MOBS.iter().all(|(_, item)| ctx.has_quest_items(player, *item)) {
                            htmltext = "32743-05.htm".to_owned();
                            for (_, item) in MOBS {
                                ctx.take_items(player, item, -1);
                            }
                        } else {
                            htmltext = "32743-06.htm".to_owned();
                        }
                    }
                }
                COMPLETED => htmltext = "32743-done.htm".to_owned(),
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
    use crate::testing::{Pages, ScriptTest, pages_to, rows};

    const NAME: &str = "Q00250_WatchWhatYouEat";

    const NPCS: &str = r#"
        <npc id="32743" type="Folk" level="70" name="Sally"/>
        <npc id="18864" type="Monster" level="60" name="Meat Monster"/>
        <npc id="18865" type="Monster" level="60" name="Meat Monster"/>
        <npc id="18868" type="Monster" level="60" name="Meat Monster"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15493" type="EtcItem" name="Meat 1"><set name="is_questitem" val="true"/></item>
        <item id="15494" type="EtcItem" name="Meat 2"><set name="is_questitem" val="true"/></item>
        <item id="15495" type="EtcItem" name="Meat 3"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00250WatchWhatYouEat);
        let pages = Pages::of(&Q00250WatchWhatYouEat)
            .page("32743-01.htm", &["32743-02.htm"])
            .page("32743-02.htm", &["32743-03.htm"])
            .page("32743-06.htm", &["32743-end.htm"])
            .page("32743-22.html", &["32743-20.html"])
            .plain(&[
                "32743-00.htm",
                "32743-03.htm",
                "32743-04.htm",
                "32743-05.htm",
                "32743-23.html",
                "32743-end.htm",
                "32743-done.htm",
            ])
            .file(
                "data/scripts/quests/Q00250_WatchWhatYouEat/32743-20.html",
                // The first-talk page: its own `bypass Script` opens the quest window.
                r#"<html><body>32743-20.html<a action="bypass Script Q00250_WatchWhatYouEat 32743-22.html">x</a><a action="bypass Script">Quest</a></body></html>"#,
            )
            .npcs(&[SALLY]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let sally = t.spawn(SALLY, 20);
        (t, sally)
    }

    #[test]
    fn a_character_below_level_82_is_sent_away() {
        let (mut t, sally) = setup();
        t.enter(1, 81, vec![]);
        assert!(t.quest_page(1, sally).contains("32743-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_first_talk_page_comes_before_the_quest_window() {
        let (mut t, sally) = setup();
        t.enter(1, 82, vec![]);
        t.talk(1, sally);
        let first = pages_to(&t.take(), 1);
        assert!(first.iter().any(|p| p.contains("32743-20.html")), "{first:?}");
    }

    #[test]
    fn the_three_meats_then_the_reward() {
        let (mut t, sally) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, sally).contains("32743-01.htm"));
        assert!(t.event(1, NAME, "32743-02.htm").contains("32743-02.htm"));
        assert!(t.event(1, NAME, "32743-03.htm").contains("32743-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, sally).contains("32743-04.htm"));

        // Each monster gives its meat once, the same monster again gives nothing.
        for mob in [18864, 18864, 18865] {
            let m = t.spawn(mob, 20);
            t.kill(1, m);
        }
        assert_eq!((t.count(1, 15493), t.count(1, 15494)), (1, 1));
        assert!(t.take_writes().is_empty());

        // The third kind sets cond 2.
        let m = t.spawn(18868, 20);
        t.kill(1, m);
        assert_eq!(t.count(1, 15495), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Sally takes the meat, then pays when the reward link is clicked.
        assert!(t.quest_page(1, sally).contains("32743-05.htm"));
        assert_eq!(
            (t.count(1, 15493), t.count(1, 15494), t.count(1, 15495)),
            (0, 0, 0)
        );
        assert!(t.quest_page(1, sally).contains("32743-06.htm"));
        assert!(t.event(1, NAME, "32743-end.htm").contains("32743-end.htm"));
        assert_eq!(t.count(1, 57), 135661);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, sally).contains("32743-done.htm"));
    }
}
