//! Quest 190: Lost Dream (Head Blacksmith Kusto, Juris, Researcher Lorain and Maestro Nikola).
// Java: dist/game/data/scripts/quests/Q00190_LostDream/Q00190_LostDream.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const JURIS: i32 = 30113;
const HEAD_BLACKSMITH_KUSTO: i32 = 30512;
const MAESTRO_NIKOLA: i32 = 30621;
const RESEARCHER_LORAIN: i32 = 30673;

// Misc
const MIN_LEVEL: i32 = 42;
const MAX_LEVEL_FOR_EXP_SP: i32 = 48;

/// Java `Q00187_NikolasHeart.class.getSimpleName()`: the quest this one follows.
const NIKOLAS_HEART: &str = "Q00187_NikolasHeart";

/// Java `Q00190_LostDream`.
pub struct Q00190LostDream;

impl Script for Q00190LostDream {
    fn name(&self) -> &'static str {
        "Q00190_LostDream"
    }

    fn path(&self) -> &'static str {
        "quests/Q00190_LostDream"
    }

    fn id(&self) -> i32 {
        190
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HEAD_BLACKSMITH_KUSTO]);
        s.add_talk_id(&[HEAD_BLACKSMITH_KUSTO, RESEARCHER_LORAIN, MAESTRO_NIKOLA, JURIS]);
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
            "30512-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30512-06.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30113-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30113-03.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
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
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == HEAD_BLACKSMITH_KUSTO {
                    // `player.getQuestState(Q00187_NikolasHeart)`: read only.
                    let nikolas_heart_done = ctx
                        .player_quest_state(player, NIKOLAS_HEART)
                        .is_some_and(|q187| q187.is_completed());
                    if nikolas_heart_done {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "30512-01.htm"
                        } else {
                            "30512-02.htm"
                        }
                        .to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                HEAD_BLACKSMITH_KUSTO => {
                    if memo_state == 1 {
                        htmltext = "30512-04.html".to_owned();
                    } else if memo_state == 2 {
                        htmltext = "30512-05.html".to_owned();
                    } else if (3..=4).contains(&memo_state) {
                        htmltext = "30512-07.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "30512-08.html".to_owned();
                        ctx.give_adena(player, 109_427, true);
                        if ctx.player_level(player) < MAX_LEVEL_FOR_EXP_SP {
                            ctx.add_exp_and_sp(player, 309_467, 20614);
                        }
                        qs.exit_quest(ctx, false, true);
                    }
                }
                JURIS => {
                    if memo_state == 1 {
                        htmltext = "30113-01.html".to_owned();
                    } else if memo_state == 2 {
                        htmltext = "30113-04.html".to_owned();
                    }
                }
                MAESTRO_NIKOLA => {
                    if memo_state == 4 {
                        qs.set_memo_state(ctx, 5);
                        qs.set_cond(ctx, 5, true);
                        htmltext = "30621-01.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "30621-02.html".to_owned();
                    }
                }
                RESEARCHER_LORAIN => {
                    if memo_state == 3 {
                        qs.set_memo_state(ctx, 4);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30673-01.html".to_owned();
                    } else if memo_state == 4 {
                        htmltext = "30673-02.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => {
                if npc.id == HEAD_BLACKSMITH_KUSTO {
                    htmltext = ctx.get_already_completed_msg(player);
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
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00190_LostDream";

    const NPCS: &str = r#"
        <npc id="30113" type="Folk" level="70" name="Juris"/>
        <npc id="30512" type="Folk" level="70" name="Head Blacksmith Kusto"/>
        <npc id="30621" type="Folk" level="70" name="Maestro Nikola"/>
        <npc id="30673" type="Folk" level="70" name="Researcher Lorain"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    /// Java `Q00187_NikolasHeart` as far as this quest reads it: a script of that name.
    struct NikolasHeart;

    impl Script for NikolasHeart {
        fn name(&self) -> &'static str {
            NIKOLAS_HEART
        }
        fn path(&self) -> &'static str {
            "quests/Q00187_NikolasHeart"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        kusto: Oid,
        juris: Oid,
        lorain: Oid,
        nikola: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(NikolasHeart);
        registry.add(Q00190LostDream);
        let pages = Pages::of(&Q00190LostDream)
            .page("30512-01.htm", &["30512-03.htm"])
            .plain(&["30512-02.htm", "30512-03.htm", "30512-04.html", "30512-07.html", "30512-08.html"])
            .page("30512-05.html", &["30512-06.html"])
            .plain(&["30512-06.html"])
            .page("30113-01.html", &["30113-02.html"])
            .page("30113-02.html", &["30113-03.html"])
            .plain(&["30113-03.html", "30113-04.html"])
            .plain(&["30673-01.html", "30673-02.html", "30621-01.html", "30621-02.html"])
            .npcs(&[JURIS, HEAD_BLACKSMITH_KUSTO, MAESTRO_NIKOLA, RESEARCHER_LORAIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            kusto: t.spawn(HEAD_BLACKSMITH_KUSTO, 20),
            juris: t.spawn(JURIS, 40),
            lorain: t.spawn(RESEARCHER_LORAIN, 60),
            nikola: t.spawn(MAESTRO_NIKOLA, 80),
        };
        (t, npcs)
    }

    fn finish_nikolas_heart(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(NIKOLAS_HEART, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
    }

    #[test]
    fn the_offer_needs_nikolas_heart_and_level_42() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 42, race::HUMAN, 0, vec![]);
        assert!(!t.quest_page(1, npcs.kusto).contains("30512-"));
        finish_nikolas_heart(&mut t, 1);
        t.take_writes();
        assert!(t.quest_page(1, npcs.kusto).contains("30512-01.htm"));
        t.enter_as(2, 41, race::HUMAN, 0, vec![]);
        finish_nikolas_heart(&mut t, 2);
        t.take_writes();
        assert!(t.quest_page(2, npcs.kusto).contains("30512-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_memo_states_walk_juris_kusto_lorain_nikola_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 42, race::HUMAN, 0, vec![]);
        finish_nikolas_heart(&mut t, 1);
        t.take_writes();

        assert!(t.quest_page(1, npcs.kusto).contains("30512-01.htm"));
        assert!(t.event(1, NAME, "30512-03.htm").contains("30512-03.htm"));
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "cond", "1"), rows::insert(NAME, "<state>", "Started"), rows::insert(NAME, "memoState", "1")]
        );
        assert!(t.quest_page(1, npcs.kusto).contains("30512-04.html"));

        // Juris: memo state 2 and cond 2.
        assert!(t.quest_page(1, npcs.juris).contains("30113-01.html"));
        assert!(t.event(1, NAME, "30113-02.html").contains("30113-02.html"));
        assert!(t.event(1, NAME, "30113-03.html").contains("30113-03.html"));
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "2"), rows::update(NAME, "cond", "2")]
        );
        assert!(t.quest_page(1, npcs.juris).contains("30113-04.html"));

        // Kusto: memo state 3 and cond 3 (`30512-06.html`), then the waiting page.
        assert!(t.quest_page(1, npcs.kusto).contains("30512-05.html"));
        assert!(t.event(1, NAME, "30512-06.html").contains("30512-06.html"));
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "3"), rows::update(NAME, "cond", "3")]
        );
        assert!(t.quest_page(1, npcs.kusto).contains("30512-07.html"));

        // Lorain: memo state 4 and cond 4.
        assert!(t.quest_page(1, npcs.lorain).contains("30673-01.html"));
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "4"), rows::update(NAME, "cond", "4")]
        );
        assert!(t.quest_page(1, npcs.lorain).contains("30673-02.html"));

        // Nikola: memo state 5 and cond 5.
        assert!(t.quest_page(1, npcs.nikola).contains("30621-01.html"));
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "5"), rows::update(NAME, "cond", "5")]
        );

        // Kusto pays on talk: adena, exp and sp (level 42 < 48), `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.kusto).contains("30512-08.html"));
        assert_eq!(t.count(1, 57), 109_427);
        assert_eq!(
            t.take_writes(),
            [rows::delete(NAME, false), rows::update(NAME, "<state>", "Completed")]
        );
        assert!(t.quest_page(1, npcs.kusto).contains("already been completed"));
    }
}
