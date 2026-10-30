//! Quest 179: Into the Large Cavern (Kekropus in the Kamael village, the Menacing Machine).
// Java: dist/game/data/scripts/quests/Q00179_IntoTheLargeCavern/Q00179_IntoTheLargeCavern.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KEKROPUS: i32 = 32138;
const MENACING_MACHINE: i32 = 32258;

// Misc
const MIN_LEVEL: i32 = 17;
const MAX_LEVEL: i32 = 21;

/// Java `Q00178_IconicTrinity.class.getSimpleName()`: the quest this one follows.
const ICONIC_TRINITY: &str = "Q00178_IconicTrinity";

/// Java `Q00179_IntoTheLargeCavern`.
pub struct Q00179IntoTheLargeCavern;

impl Script for Q00179IntoTheLargeCavern {
    fn name(&self) -> &'static str {
        "Q00179_IntoTheLargeCavern"
    }

    fn path(&self) -> &'static str {
        "quests/Q00179_IntoTheLargeCavern"
    }

    fn id(&self) -> i32 {
        179
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KEKROPUS]);
        s.add_talk_id(&[KEKROPUS, MENACING_MACHINE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // Java returns the event as the page even when there is no quest state (no `null`).
        let player = player?;
        let htmltext = Some(event.to_owned());
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return htmltext;
        };
        // Java reads `npc.getId()` without a null check (a bypass always has the NPC).
        let npc = npc?;
        if npc.id == KEKROPUS {
            if event.eq_ignore_ascii_case("32138-03.html") {
                qs.start_quest(ctx);
            }
        } else if npc.id == MENACING_MACHINE {
            if event.eq_ignore_ascii_case("32258-08.html") {
                ctx.give_items(player, 391, 1);
                ctx.give_items(player, 413, 1);
                qs.exit_quest(ctx, false, true);
            } else if event.eq_ignore_ascii_case("32258-09.html") {
                ctx.give_items(player, 847, 2);
                ctx.give_items(player, 890, 2);
                ctx.give_items(player, 910, 1);
                qs.exit_quest(ctx, false, true);
            }
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == KEKROPUS {
            match qs.state(ctx) {
                CREATED => {
                    if ctx.player_race(player) != race::KAMAEL {
                        htmltext = "32138-00b.html".to_owned();
                    } else {
                        // `player.getQuestState(Q00178_IconicTrinity)`: read only.
                        let prev_done = ctx
                            .player_quest_state(player, ICONIC_TRINITY)
                            .is_some_and(|prev| prev.is_completed());
                        let level = ctx.player_level(player);
                        htmltext = if prev_done
                            && (MIN_LEVEL..=MAX_LEVEL).contains(&level)
                            && ctx.player_class_level(player) == 0
                        {
                            "32138-01.htm"
                        } else if level < MIN_LEVEL {
                            "32138-00.html"
                        } else {
                            "32138-00c.html"
                        }
                        .to_owned();
                    }
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        // NOTE(java-quirk): Java asks for `32138-03.htm`, but the folder has only
                        // `32138-03.html` (the event page). The stand-in of the port follows the file.
                        htmltext = "32138-03.htm".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            }
        } else if npc.id == MENACING_MACHINE && qs.is_started(ctx) {
            htmltext = "32258-01.html".to_owned();
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::COMPLETED;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00179_IntoTheLargeCavern";

    const NPCS: &str = r#"
        <npc id="32138" type="Folk" level="70" name="Kekropus"/>
        <npc id="32258" type="Folk" level="70" name="Menacing Machine"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="391" type="EtcItem" name="Item 391"><set name="is_stackable" val="true"/></item>
        <item id="413" type="EtcItem" name="Item 413"><set name="is_stackable" val="true"/></item>
        <item id="847" type="EtcItem" name="Item 847"><set name="is_stackable" val="true"/></item>
        <item id="890" type="EtcItem" name="Item 890"><set name="is_stackable" val="true"/></item>
        <item id="910" type="EtcItem" name="Item 910"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00178_IconicTrinity` as far as this quest reads it: a script of that name.
    struct IconicTrinity;

    impl Script for IconicTrinity {
        fn name(&self) -> &'static str {
            ICONIC_TRINITY
        }
        fn path(&self) -> &'static str {
            "quests/Q00178_IconicTrinity"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(IconicTrinity);
        registry.add(Q00179IntoTheLargeCavern);
        let pages = Pages::of(&Q00179IntoTheLargeCavern)
            .page("32138-01.htm", &["32138-02.htm"])
            .page("32138-02.htm", &["32138-03.html"])
            .page("32138-03.html", &["32138-04.html"])
            .plain(&[
                "32138-00.html",
                "32138-00b.html",
                "32138-00c.html",
                "32138-04.html",
            ])
            .page("32258-01.html", &["32258-02.html"])
            .page("32258-02.html", &["32258-03.html"])
            .page("32258-03.html", &["32258-04.html"])
            .page("32258-04.html", &["32258-05.html"])
            .page("32258-05.html", &["32258-06.html"])
            .page("32258-06.html", &["32258-07.html"])
            .page("32258-07.html", &["32258-08.html", "32258-09.html"])
            .plain(&["32258-08.html", "32258-09.html"])
            .npcs(&[KEKROPUS, MENACING_MACHINE]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kekropus = t.spawn(KEKROPUS, 20);
        let machine = t.spawn(MENACING_MACHINE, 40);
        (t, kekropus, machine)
    }

    fn finish_iconic_trinity(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(ICONIC_TRINITY, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
    }

    /// Starts the quest and walks the machine's pages up to the last one (`32258-07.html`).
    fn to_the_machine(t: &mut ScriptTest, kekropus: Oid, machine: Oid) {
        assert!(t.quest_page(1, kekropus).contains("32138-01.htm"));
        assert!(t.event(1, NAME, "32138-02.htm").contains("32138-02.htm"));
        assert!(t.event(1, NAME, "32138-03.html").contains("32138-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, machine).contains("32258-01.html"));
        for page in [
            "32258-02.html",
            "32258-03.html",
            "32258-04.html",
            "32258-05.html",
            "32258-06.html",
            "32258-07.html",
        ] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        t.take();
    }

    #[test]
    fn a_kamael_of_level_17_to_21_after_iconic_trinity_is_offered_the_quest() {
        let (mut t, kekropus, _) = setup();
        t.enter_as(1, 19, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, kekropus).contains("32138-00b.html"));

        t.enter_as(2, 19, race::KAMAEL, 123, vec![]);
        assert!(t.quest_page(2, kekropus).contains("32138-00c.html"));
        t.enter_as(3, 16, race::KAMAEL, 123, vec![]);
        assert!(t.quest_page(3, kekropus).contains("32138-00.html"));
        assert!(t.take_writes().is_empty());

        finish_iconic_trinity(&mut t, 2);
        t.take_writes();
        assert!(t.quest_page(2, kekropus).contains("32138-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_machine_gives_the_first_reward() {
        let (mut t, kekropus, machine) = setup();
        t.enter_as(1, 19, race::KAMAEL, 123, vec![]);
        finish_iconic_trinity(&mut t, 1);
        t.take_writes();
        to_the_machine(&mut t, kekropus, machine);

        assert!(t.event(1, NAME, "32258-08.html").contains("32258-08.html"));
        assert_eq!((t.count(1, 391), t.count(1, 413)), (1, 1));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, kekropus).contains("already been completed"));
    }

    #[test]
    fn the_machine_gives_the_second_reward() {
        let (mut t, kekropus, machine) = setup();
        t.enter_as(1, 19, race::KAMAEL, 123, vec![]);
        finish_iconic_trinity(&mut t, 1);
        t.take_writes();
        to_the_machine(&mut t, kekropus, machine);

        assert!(t.event(1, NAME, "32258-09.html").contains("32258-09.html"));
        assert_eq!((t.count(1, 847), t.count(1, 890), t.count(1, 910)), (2, 2, 1));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
