//! Quest 10284: Acquisition of Divine Sword (Rafforty, Jinia in the Jinia guild hideout, Krun/Tarun in the mithril mine).
// Java: dist/game/data/scripts/quests/Q10284_AcquisitionOfDivineSword/Q10284_AcquisitionOfDivineSword.java
//
// Pattern: a memo-state quest like Q10286: memo state 1 = the hideout (the three story pages are
// remembered in `ex1`..`ex3`), 2 = out of the hideout, 3 = the mine is done (set by the
// `MithrilMine` instance script, which is not this quest).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const RAFFORTY: i32 = 32020;
const KRUN: i32 = 32653;
const TARUN: i32 = 32654;
const JINIA: i32 = 32760;

// Misc
const MIN_LEVEL: i32 = 82;

// Item
const COLD_RESISTANCE_POTION: i32 = 15514;

// Location (Java `EXIT_LOC`)
const EXIT_LOC: (i32, i32, i32) = (113_793, -109_342, -845);

/// Java `Q10283_RequestOfIceMerchant.class.getSimpleName()`.
const Q10283: &str = "Q10283_RequestOfIceMerchant";

/// Java `Q10284_AcquisitionOfDivineSword`.
pub struct Q10284AcquisitionOfDivineSword;

impl Script for Q10284AcquisitionOfDivineSword {
    fn name(&self) -> &'static str {
        "Q10284_AcquisitionOfDivineSword"
    }

    fn path(&self) -> &'static str {
        "quests/Q10284_AcquisitionOfDivineSword"
    }

    fn id(&self) -> i32 {
        10284
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAFFORTY]);
        s.add_talk_id(&[RAFFORTY, JINIA, TARUN, KRUN]);
        s.register_quest_items(&[COLD_RESISTANCE_POTION]);
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
            "32020-02.html" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "32020-03.html" | "32760-02a.html" | "32760-02b.html" | "32760-03a.html" | "32760-03b.html"
            | "32760-04a.html" | "32760-04b.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-02c.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_int(ctx, "ex1", 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "another_story" => {
                if qs.is_memo_state(ctx, 1) {
                    let ex = (qs.get_int(ctx, "ex1"), qs.get_int(ctx, "ex2"), qs.get_int(ctx, "ex3"));
                    htmltext = match ex {
                        (1, 0, 0) => Some("32760-05a.html"),
                        (0, 1, 0) => Some("32760-05b.html"),
                        (0, 0, 1) => Some("32760-05c.html"),
                        (0, 1, 1) => Some("32760-05d.html"),
                        (1, 0, 1) => Some("32760-05e.html"),
                        (1, 1, 0) => Some("32760-05f.html"),
                        (1, 1, 1) => Some("32760-05g.html"),
                        _ => None,
                    }
                    .map(str::to_owned);
                }
            }
            "32760-03c.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_int(ctx, "ex2", 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-04c.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_int(ctx, "ex3", 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-06.html" => {
                if qs.is_memo_state(ctx, 1)
                    && qs.get_int(ctx, "ex1") == 1
                    && qs.get_int(ctx, "ex2") == 1
                    && qs.get_int(ctx, "ex3") == 1
                {
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-07.html" => {
                if qs.is_memo_state(ctx, 1)
                    && qs.get_int(ctx, "ex1") == 1
                    && qs.get_int(ctx, "ex2") == 1
                    && qs.get_int(ctx, "ex3") == 1
                {
                    qs.unset(ctx, "ex1");
                    qs.unset(ctx, "ex2");
                    qs.unset(ctx, "ex3");
                    qs.set_cond(ctx, 3, true);
                    qs.set_memo_state(ctx, 2);
                    // Java: `InstanceManager.getPlayerWorld(player).removeAllowed(player)` then
                    // `setInstanceId(0)`. The hideout's world belongs to the instance script.
                    if let Some(id) = ctx.world().player_instance_world(player).map(|w| w.instance_id)
                        && let Some(world) = ctx.world_mut().instance_world_mut(id)
                    {
                        world.remove_allowed(player);
                    }
                    ctx.world_mut().set_instance_id(player, 0);
                    htmltext = Some(event.to_owned());
                }
            }
            "exit_instance" => {
                if qs.is_memo_state(ctx, 2) {
                    ctx.teleport_player(player, EXIT_LOC.0, EXIT_LOC.1, EXIT_LOC.2, 0, false);
                }
            }
            "32654-02.html" | "32654-03.html" | "32653-02.html" | "32653-03.html" => {
                if qs.is_memo_state(ctx, 2) {
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
                if npc.id == RAFFORTY {
                    htmltext = "32020-05.html".to_owned();
                }
            }
            CREATED => {
                if npc.id == RAFFORTY {
                    let done = ctx
                        .player_quest_state(player, Q10283)
                        .is_some_and(|q| q.is_completed());
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL && done {
                        "32020-01.htm"
                    } else {
                        "32020-04.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                RAFFORTY => match qs.get_memo_state(ctx) {
                    1 => {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "32020-06.html"
                        } else {
                            "32020-08.html"
                        }
                        .to_owned();
                    }
                    2 => htmltext = "32020-07.html".to_owned(),
                    _ => {}
                },
                JINIA => {
                    if qs.is_memo_state(ctx, 1) {
                        let ex = (qs.get_int(ctx, "ex1"), qs.get_int(ctx, "ex2"), qs.get_int(ctx, "ex3"));
                        match ex {
                            (0, 0, 0) => htmltext = "32760-01.html".to_owned(),
                            (1, 0, 0) => htmltext = "32760-01a.html".to_owned(),
                            (0, 1, 0) => htmltext = "32760-01b.html".to_owned(),
                            (0, 0, 1) => htmltext = "32760-01c.html".to_owned(),
                            (0, 1, 1) => htmltext = "32760-01d.html".to_owned(),
                            (1, 0, 1) => htmltext = "32760-01e.html".to_owned(),
                            (1, 1, 0) => htmltext = "32760-01f.html".to_owned(),
                            (1, 1, 1) => htmltext = "32760-01g.html".to_owned(),
                            _ => {}
                        }
                    }
                }
                TARUN | KRUN => match qs.get_memo_state(ctx) {
                    2 => {
                        let (ok, low) = if npc.id == TARUN {
                            ("32654-01.html", "32654-05.html")
                        } else {
                            ("32653-01.html", "32653-05.html")
                        };
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL { ok } else { low }.to_owned();
                    }
                    3 => {
                        ctx.give_adena(player, 296_425, true);
                        ctx.add_exp_and_sp(player, 921_805, 82_230);
                        qs.exit_quest(ctx, false, true);
                        htmltext = if npc.id == TARUN { "32654-04.html" } else { "32653-04.html" }.to_owned();
                    }
                    _ => {}
                },
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, player_data, rows};

    const NAME: &str = "Q10284_AcquisitionOfDivineSword";

    const NPCS: &str = r#"
        <npc id="32020" type="Folk" level="80" name="Rafforty"/>
        <npc id="32653" type="Folk" level="80" name="Krun"/>
        <npc id="32654" type="Folk" level="80" name="Tarun"/>
        <npc id="32760" type="Folk" level="80" name="Jinia"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    /// Stands in for Q10283 (its completion is read through `player_quest_state` only).
    struct IceMerchantStandIn;

    impl Script for IceMerchantStandIn {
        fn name(&self) -> &'static str {
            Q10283
        }

        fn path(&self) -> &'static str {
            "quests/Q10283_RequestOfIceMerchant"
        }

        fn id(&self) -> i32 {
            10283
        }

        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, ObjectId, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10284AcquisitionOfDivineSword);
        registry.add(IceMerchantStandIn);
        let pages = Pages::of(&Q10284AcquisitionOfDivineSword)
            .page("32020-01.htm", &["32020-02.html"])
            .page("32020-02.html", &["32020-03.html"])
            .plain(&["32020-03.html", "32020-04.html", "32020-05.html", "32020-06.html", "32020-07.html", "32020-08.html"])
            .page("32760-01.html", &["32760-02a.html"])
            .page("32760-02a.html", &["32760-02b.html"])
            .page("32760-02b.html", &["32760-02c.html"])
            .page("32760-02c.html", &["another_story"])
            .page("32760-05a.html", &["32760-03a.html"])
            .page("32760-03a.html", &["32760-03b.html"])
            .page("32760-03b.html", &["32760-03c.html"])
            .page("32760-03c.html", &["another_story"])
            .page("32760-05f.html", &["32760-04a.html"])
            .page("32760-04a.html", &["32760-04b.html"])
            .page("32760-04b.html", &["32760-04c.html"])
            .page("32760-04c.html", &["another_story"])
            .page("32760-05g.html", &["32760-06.html"])
            .page("32760-06.html", &["32760-07.html"])
            .page("32760-07.html", &["exit_instance"])
            .page("32760-01g.html", &["32760-06.html"])
            .plain(&["32653-01.html", "32653-04.html", "32654-01.html", "32654-04.html"])
            .npcs(&[RAFFORTY, JINIA, KRUN, TARUN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rafforty = t.spawn(RAFFORTY, 20);
        let jinia = t.spawn(JINIA, 30);
        let krun = t.spawn(KRUN, 40);
        (t, rafforty, jinia, krun)
    }

    fn loaded(t: &mut ScriptTest, level: i32, memo: Option<&str>, vars: &[(&str, &str)]) {
        let mut data = player_data(1, level, vec![]);
        let mut states = vec![(Q10283.to_owned(), Some("Completed".to_owned()))];
        let mut variables = vec![];
        if let Some(memo) = memo {
            states.push((NAME.to_owned(), Some("Started".to_owned())));
            variables.push((NAME.to_owned(), "cond".to_owned(), Some("1".to_owned())));
            variables.push((NAME.to_owned(), "memoState".to_owned(), Some(memo.to_owned())));
        }
        for (k, v) in vars {
            variables.push((NAME.to_owned(), (*k).to_owned(), Some((*v).to_owned())));
        }
        data.profile.quests = QuestLoad { states, variables };
        t.enter_with(data);
    }

    #[test]
    fn the_start_needs_82_and_the_ice_merchant() {
        let (mut t, rafforty, _, _) = setup();
        loaded(&mut t, 81, None, &[]);
        assert!(t.quest_page(1, rafforty).contains("32020-04.html"));
        t.enter(2, 82, vec![]);
        assert!(t.quest_page(2, rafforty).contains("32020-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn rafforty_starts_and_jinia_tells_the_stories() {
        let (mut t, rafforty, jinia, _) = setup();
        loaded(&mut t, 82, None, &[]);
        assert!(t.quest_page(1, rafforty).contains("32020-01.htm"));
        assert!(t.event(1, NAME, "32020-02.html").contains("32020-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert!(t.quest_page(1, jinia).contains("32760-01.html"));
        t.event(1, NAME, "32760-02a.html");
        t.event(1, NAME, "32760-02b.html");
        t.event(1, NAME, "32760-02c.html");
        assert_eq!(t.take_writes(), [rows::insert(NAME, "ex1", "1")]);
        assert!(t.event(1, NAME, "another_story").contains("32760-05a.html"));
    }

    #[test]
    fn the_third_story_lets_jinia_send_the_character_out() {
        let (mut t, _, jinia, _) = setup();
        loaded(&mut t, 82, Some("1"), &[("ex1", "1"), ("ex2", "1"), ("ex3", "1")]);
        assert!(t.quest_page(1, jinia).contains("32760-01g.html"));
        assert!(t.event(1, NAME, "32760-06.html").contains("32760-06.html"));
        assert!(t.event(1, NAME, "32760-07.html").contains("32760-07.html"));
        let writes = t.take_writes();
        assert!(writes.contains(&rows::update(NAME, "memoState", "2")));
        assert!(writes.contains(&rows::delete_var(NAME, "ex1")));
        t.event(1, NAME, "exit_instance");
        let (x, y, _) = t.world.position_of(ObjectId(1)).unwrap();
        assert_eq!((x, y), (113_793, -109_342));
    }

    #[test]
    fn krun_pays_when_the_mine_is_done() {
        let (mut t, _, _, krun) = setup();
        loaded(&mut t, 82, Some("3"), &[]);
        assert!(t.quest_page(1, krun).contains("32653-04.html"));
        assert!(t.count(1, 57) > 0);
        assert_eq!(
            t.take_writes(),
            [rows::delete(NAME, false), rows::update(NAME, "<state>", "Completed")]
        );
    }
}
