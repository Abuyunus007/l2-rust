//! Quest 423: Take Your Best Shot (Johnny and Batracos at the Ugoros gate, Tanta Guards).
// Java: dist/game/data/scripts/quests/Q00423_TakeYourBestShot/Q00423_TakeYourBestShot.java
//
// Pattern: a one-kill quest (`on_kill` moves to cond 2) with a first-talk page of the start NPC
// (`add_first_talk_id`) that depends on the position of the NPC.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const BATRACOS: i32 = 32740;
const JOHNNY: i32 = 32744;

// Monster
const TANTA_GUARD: i32 = 18862;

// Item
const SEER_UGOROS_PASS: i32 = 15496;

// Misc
const MIN_LEVEL: i32 = 82;
/// Java `Q00249_PoisonedPlainsOfTheLizardmen.class.getSimpleName()`.
const POISONED_PLAINS_OF_THE_LIZARDMEN: &str = "Q00249_PoisonedPlainsOfTheLizardmen";

/// Java `Q00423_TakeYourBestShot`.
pub struct Q00423TakeYourBestShot;

impl Script for Q00423TakeYourBestShot {
    fn name(&self) -> &'static str {
        "Q00423_TakeYourBestShot"
    }

    fn path(&self) -> &'static str {
        "quests/Q00423_TakeYourBestShot"
    }

    fn id(&self) -> i32 {
        423
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JOHNNY, BATRACOS]);
        s.add_talk_id(&[JOHNNY, BATRACOS]);
        s.add_first_talk_id(&[BATRACOS]);
        s.add_kill_id(&[TANTA_GUARD]);
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
            "32740.html" | "32740-01.html" | "32744-02.html" | "32744-03.htm" => {}
            "32744-04.htm" => qs.start_quest(ctx),
            "32744-quit.html" => qs.exit_quest(ctx, true, false),
            _ => return None,
        }
        Some(event.to_owned())
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        // Java `npc.isInsideRadius2D(96782, 85918, 0, 100)`: the Ugoros gate has its own page.
        if ctx.is_inside_radius_2d_xy(npc.object_id, 96_782, 85_918, 100) {
            Some("32740-ugoros.html".to_owned())
        } else {
            Some("32740.html".to_owned())
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(qs) = ctx.get_quest_state(killer, false)
            && qs.is_cond(ctx, 1)
        {
            qs.set_cond(ctx, 2, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            JOHNNY => match qs.state(ctx) {
                CREATED => {
                    let prev_completed = ctx
                        .player_quest_state(player, POISONED_PLAINS_OF_THE_LIZARDMEN)
                        .is_some_and(|prev| prev.is_completed());
                    htmltext = if prev_completed && ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.has_quest_items(player, SEER_UGOROS_PASS) {
                            "32744-07.htm"
                        } else {
                            "32744-01.htm"
                        }
                    } else {
                        "32744-00.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "32744-05.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        htmltext = "32744-06.html".to_owned();
                    }
                }
                _ => {}
            },
            BATRACOS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.has_quest_items(player, SEER_UGOROS_PASS) {
                        "32740-05.html"
                    } else {
                        "32740-00.html"
                    }
                    .to_owned();
                }
                STARTED => {
                    if qs.is_cond(ctx, 1) {
                        htmltext = "32740-02.html".to_owned();
                    } else if qs.is_cond(ctx, 2) {
                        ctx.give_items(player, SEER_UGOROS_PASS, 1);
                        qs.exit_quest(ctx, true, true);
                        htmltext = "32740-04.html".to_owned();
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
    use l2_world::{QuestLoad, Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00423_TakeYourBestShot";
    const PLAINS: &str = "Q00249_PoisonedPlainsOfTheLizardmen";

    const NPCS: &str = r#"
        <npc id="32740" type="Folk" level="70" name="Batracos"/>
        <npc id="32744" type="Folk" level="70" name="Johnny"/>
        <npc id="18862" type="Monster" level="85" name="Tanta Guard"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15496" type="EtcItem" name="Seer Ugoros' Pass"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00249_PoisonedPlainsOfTheLizardmen` as far as this quest needs it: a script of that name.
    struct PoisonedPlains;

    impl Script for PoisonedPlains {
        fn name(&self) -> &'static str {
            PLAINS
        }
        fn path(&self) -> &'static str {
            "quests/Q00249_PoisonedPlainsOfTheLizardmen"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    /// Enters the character 1 of `level`; `plains_done` gives it the finished Poisoned Plains row.
    fn enter(t: &mut ScriptTest, level: i32, plains_done: bool, items: Vec<l2_world::ItemData>) {
        let mut data = player_data(1, level, items);
        if plains_done {
            data.profile.quests = QuestLoad {
                states: vec![(PLAINS.to_owned(), Some("Completed".to_owned()))],
                variables: vec![],
            };
        }
        t.enter_with(data);
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(PoisonedPlains);
        registry.add(Q00423TakeYourBestShot);
        let pages = Pages::of(&Q00423TakeYourBestShot)
            // The first-talk page: its own `bypass Script` opens the quest window.
            .file(
                "data/scripts/quests/Q00423_TakeYourBestShot/32740.html",
                r#"<html><body>32740.html<a action="bypass Script Q00423_TakeYourBestShot 32740-01.html">x</a><a action="bypass Script">Quest</a></body></html>"#,
            )
            .page("32740-01.html", &["32740.html"])
            .plain(&["32740-ugoros.html"])
            .page("32744-01.htm", &["32744-02.html"])
            .page("32744-02.html", &["32744-03.htm"])
            .page("32744-03.htm", &["32744-04.htm", "32744-quit.html"])
            .plain(&[
                "32740-00.html",
                "32740-02.html",
                "32740-04.html",
                "32740-05.html",
                "32744-00.htm",
                "32744-04.htm",
                "32744-05.html",
                "32744-06.html",
                "32744-07.htm",
                "32744-quit.html",
            ])
            .npcs(&[BATRACOS, JOHNNY]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let johnny = t.spawn(JOHNNY, 20);
        let batracos = t.spawn(BATRACOS, 40);
        (t, johnny, batracos)
    }

    #[test]
    fn a_character_who_has_not_finished_the_plains_is_sent_away() {
        let (mut t, johnny, _) = setup();
        enter(&mut t, 82, false, vec![]);
        assert!(t.quest_page(1, johnny).contains("32744-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_character_below_level_82_is_sent_away() {
        let (mut t, johnny, _) = setup();
        enter(&mut t, 81, true, vec![]);
        assert!(t.quest_page(1, johnny).contains("32744-00.htm"));
    }

    #[test]
    fn the_first_page_of_batracos_depends_on_the_gate_position() {
        let (mut t, _, _) = setup();
        enter(&mut t, 82, true, vec![]);
        // The first-talk hook is called directly: the gate is far from the test character.
        let gate = t
            .world
            .spawn_npc_at(BATRACOS, 96_782, 85_918, crate::testing::Z, 0)
            .unwrap();
        let far = t.spawn(BATRACOS, 60);
        let mut pages = Vec::new();
        t.world.run_as_script(NAME, |ctx| {
            let npc = ctx.script_npc(gate).unwrap();
            pages.push(Q00423TakeYourBestShot.on_first_talk(ctx, npc, Oid(1)));
            let npc = ctx.script_npc(far).unwrap();
            pages.push(Q00423TakeYourBestShot.on_first_talk(ctx, npc, Oid(1)));
        });
        assert_eq!(
            pages,
            [
                Some("32740-ugoros.html".to_owned()),
                Some("32740.html".to_owned())
            ]
        );
    }

    #[test]
    fn the_whole_quest_from_johnny_to_the_pass() {
        let (mut t, johnny, batracos) = setup();
        enter(&mut t, 82, true, vec![]);
        assert!(t.quest_page(1, johnny).contains("32744-01.htm"));
        assert!(t.event(1, NAME, "32744-02.html").contains("32744-02.html"));
        assert!(t.event(1, NAME, "32744-03.htm").contains("32744-03.htm"));

        // Accept: `startQuest()`.
        assert!(t.event(1, NAME, "32744-04.htm").contains("32744-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, johnny).contains("32744-05.html"));
        assert!(t.quest_page(1, batracos).contains("32740-02.html"));

        // The Tanta Guard moves the quest to cond 2.
        let guard = t.spawn(TANTA_GUARD, 60);
        t.kill(1, guard);
        t.wait_ms(3_000);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, johnny).contains("32744-06.html"));

        // Batracos gives the pass and ends the quest (repeatable).
        assert!(t.quest_page(1, batracos).contains("32740-04.html"));
        assert_eq!(t.count(1, SEER_UGOROS_PASS), 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn a_finished_character_with_the_pass_gets_the_other_page() {
        let (mut t, johnny, _) = setup();
        enter(&mut t, 82, true, vec![item(900, SEER_UGOROS_PASS, 1)]);
        assert!(t.quest_page(1, johnny).contains("32744-07.htm"));
    }

    #[test]
    fn quitting_before_the_accept_writes_nothing() {
        // `exitQuest(true)` returns at once when the quest is not started (Java
        // `QuestState.exitQuest`), and the quit link is only on the page before the accept.
        let (mut t, johnny, _) = setup();
        enter(&mut t, 82, true, vec![]);
        assert!(t.quest_page(1, johnny).contains("32744-01.htm"));
        assert!(t.event(1, NAME, "32744-02.html").contains("32744-02.html"));
        assert!(t.event(1, NAME, "32744-03.htm").contains("32744-03.htm"));
        assert!(t.take_writes().is_empty());
        assert!(t.event(1, NAME, "32744-quit.html").contains("32744-quit.html"));
        assert!(t.take_writes().is_empty());
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
