//! Quest 10283: Request of Ice Merchant (Rafforty and Kier send the adventurer to Jinia, one at a time).
// Java: dist/game/data/scripts/quests/Q10283_RequestOfIceMerchant/Q10283_RequestOfIceMerchant.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const RAFFORTY: i32 = 32020;
const KIER: i32 = 32022;
const JINIA: i32 = 32760;

// Misc
const MIN_LEVEL: i32 = 82;

/// Java `Q00115_TheOtherSideOfTruth.class.getSimpleName()`.
const Q00115: &str = "Q00115_TheOtherSideOfTruth";

/// Java `isBusy` and `talker`: one Jinia at a time, for the player who got her.
#[derive(Default)]
struct JiniaState {
    is_busy: bool,
    talker: Option<ObjectId>,
}

/// Java `Q10283_RequestOfIceMerchant`.
pub struct Q10283RequestOfIceMerchant;

impl Script for Q10283RequestOfIceMerchant {
    fn name(&self) -> &'static str {
        "Q10283_RequestOfIceMerchant"
    }

    fn path(&self) -> &'static str {
        "quests/Q10283_RequestOfIceMerchant"
    }

    fn id(&self) -> i32 {
        10283
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RAFFORTY]);
        s.add_talk_id(&[RAFFORTY, KIER, JINIA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(npc) = npc
            && npc.id == JINIA
            && event == "DESPAWN"
        {
            let state = ctx.state::<JiniaState>();
            state.is_busy = false;
            state.talker = None;
            ctx.delete_npc(npc);
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "32020-03.htm" => htmltext = Some(event.to_owned()),
            "32020-04.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "32020-05.html" | "32020-06.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32020-07.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, false);
                    htmltext = Some(event.to_owned());
                }
            }
            "32022-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    if !ctx.state::<JiniaState>().is_busy {
                        let state = ctx.state::<JiniaState>();
                        state.is_busy = true;
                        state.talker = Some(player);
                        qs.set_cond(ctx, 3, false);
                        ctx.add_spawn(JINIA, 104_476, -107_535, -3688, 44954, false, 0, 0);
                    } else {
                        let talker = ctx.state::<JiniaState>().talker;
                        htmltext = Some(
                            if talker == Some(player) {
                                event
                            } else {
                                "32022-03.html"
                            }
                            .to_owned(),
                        );
                    }
                }
            }
            "32760-02.html" | "32760-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32760-04.html" => {
                if qs.is_memo_state(ctx, 2) {
                    ctx.give_adena(player, 190_000, true);
                    ctx.add_exp_and_sp(player, 627_000, 50_300);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                    ctx.start_quest_timer("DESPAWN", 2000, npc, None, false);
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
                    htmltext = "32020-02.html".to_owned();
                } else if npc.id == JINIA {
                    htmltext = "32760-06.html".to_owned();
                }
            }
            CREATED => {
                let shadow_done = ctx
                    .player_quest_state(player, Q00115)
                    .is_some_and(|s| s.is_completed());
                htmltext = if ctx.player_level(player) >= MIN_LEVEL && shadow_done {
                    "32020-01.htm"
                } else {
                    "32020-08.htm"
                }
                .to_owned();
            }
            STARTED => match npc.id {
                RAFFORTY => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32020-09.html".to_owned();
                    } else if qs.is_memo_state(ctx, 2) {
                        htmltext = "32020-10.html".to_owned();
                    }
                }
                KIER => {
                    if qs.is_memo_state(ctx, 2) {
                        htmltext = "32022-01.html".to_owned();
                    }
                }
                JINIA => {
                    if qs.is_memo_state(ctx, 2) {
                        htmltext = if ctx.state::<JiniaState>().talker == Some(player) {
                            "32760-01.html"
                        } else {
                            "32760-05.html"
                        }
                        .to_owned();
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
    use l2_world::script::COMPLETED;
    use l2_world::{QuestWrite, Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q10283_RequestOfIceMerchant";

    const NPCS: &str = r#"
        <npc id="32020" type="Folk" level="80" name="Rafforty"/>
        <npc id="32022" type="Folk" level="80" name="Kier"/>
        <npc id="32760" type="Folk" level="80" name="Jinia"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00115_TheOtherSideOfTruth`, as far as this quest reads it: its state.
    struct OtherSideOfTruth;

    impl Script for OtherSideOfTruth {
        fn name(&self) -> &'static str {
            Q00115
        }
        fn path(&self) -> &'static str {
            "quests/Q00115_TheOtherSideOfTruth"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10283RequestOfIceMerchant);
        registry.add(OtherSideOfTruth);
        let pages = Pages::of(&Q10283RequestOfIceMerchant)
            .page("32020-01.htm", &["32020-03.htm"])
            .page("32020-03.htm", &["32020-04.htm"])
            .page("32020-04.htm", &["32020-05.html"])
            .page("32020-05.html", &["32020-06.html"])
            .page("32020-06.html", &["32020-07.html"])
            .plain(&["32020-07.html", "32020-08.htm", "32020-09.html", "32020-10.html"])
            .plain(&["32020-02.html", "32022-03.html"])
            .page("32022-01.html", &["32022-02.html"])
            .plain(&["32022-02.html"])
            .page("32760-01.html", &["32760-02.html"])
            .page("32760-02.html", &["32760-03.html"])
            .page("32760-03.html", &["32760-04.html"])
            .plain(&["32760-04.html", "32760-05.html", "32760-06.html"])
            .npcs(&[RAFFORTY, KIER, JINIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let rafforty = t.spawn(RAFFORTY, 20);
        let kier = t.spawn(KIER, 40);
        (t, rafforty, kier)
    }

    fn complete_other_side_of_truth(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(Q00115, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(player), true) {
                qs.set_state(ctx, COMPLETED);
            }
        });
    }

    #[test]
    fn the_quest_needs_the_other_side_of_truth_and_level_82() {
        let (mut t, rafforty, _) = setup();
        t.enter(1, 82, vec![]);
        assert!(t.quest_page(1, rafforty).contains("32020-08.htm"));
        complete_other_side_of_truth(&mut t, 1);
        assert!(t.quest_page(1, rafforty).contains("32020-01.htm"));
    }

    #[test]
    fn rafforty_kier_and_jinia_then_the_reward_and_the_despawn() {
        let (mut t, rafforty, kier) = setup();
        t.enter(1, 82, vec![]);
        complete_other_side_of_truth(&mut t, 1);
        t.take_writes();
        assert!(t.quest_page(1, rafforty).contains("32020-01.htm"));
        assert!(t.event(1, NAME, "32020-03.htm").contains("32020-03.htm"));

        // Accept: `startQuest()`, memo state 1.
        assert!(t.event(1, NAME, "32020-04.htm").contains("32020-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert!(t.event(1, NAME, "32020-05.html").contains("32020-05.html"));
        assert!(t.event(1, NAME, "32020-06.html").contains("32020-06.html"));

        // Memo state 2 and cond 2 (no sound).
        assert!(t.event(1, NAME, "32020-07.html").contains("32020-07.html"));
        assert!(t.quest_page(1, rafforty).contains("32020-10.html"));
        assert!(t.quest_page(1, kier).contains("32022-01.html"));

        // Kier: Jinia comes, cond 3, one at a time.
        // Java returns null here (only the busy branch sets a page): no page comes back.
        assert_eq!(t.event(1, NAME, "32022-02.html"), "");
        let jinia = t.npcs_of(JINIA);
        assert_eq!(jinia.len(), 1);
        // The memo state 2 and cond 2 of `32020-07.html` came before this (not taken yet).
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2"),
                rows::update(NAME, "cond", "3"),
            ]
        );

        // Jinia stands far from the test player, so her dialogue is driven as events: the
        // reward (`32760-04.html`): 190000 adena, the exp, `exitQuest(false, true)`; Jinia
        // leaves 2 s later (`startQuestTimer("DESPAWN", 2000, npc, null)`).
        let jinia_id = jinia[0];
        t.world.run_as_script(NAME, |ctx| {
            let jinia = ctx.script_npc(jinia_id);
            ctx.notify_event("32760-04.html", jinia, Some(Oid(1)));
        });
        assert_eq!(t.count(1, 57), 190_000);
        let writes = t.take_writes();
        assert!(matches!(
            &writes[0],
            QuestWrite::DeleteQuest {
                repeatable: false,
                ..
            }
        ));
        t.wait_ms(2_000);
        assert!(!t.in_world(jinia_id));
    }
}
