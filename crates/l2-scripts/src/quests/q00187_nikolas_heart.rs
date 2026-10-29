//! Quest 187: Nikola's Heart (Lorain, Maestro Nikola and Head Blacksmith Kusto).
// Java: dist/game/data/scripts/quests/Q00187_NikolasHeart/Q00187_NikolasHeart.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HEAD_BLACKSMITH_KUSTO: i32 = 30512;
const MAESTRO_NIKOLA: i32 = 30621;
const RESEARCHER_LORAIN: i32 = 30673;

// Items
const LORAINES_CERTIFICATE: i32 = 10362;
const METALLOGRAPH: i32 = 10368;

// Misc
const MIN_LEVEL: i32 = 41;
const MAX_LEVEL_FOR_EXP_SP: i32 = 47;

/// Java `Q00185_NikolasCooperation.class.getSimpleName()`: the quest this one follows.
const NIKOLAS_COOPERATION: &str = "Q00185_NikolasCooperation";

/// Java `Q00187_NikolasHeart`.
pub struct Q00187NikolasHeart;

impl Script for Q00187NikolasHeart {
    fn name(&self) -> &'static str {
        "Q00187_NikolasHeart"
    }

    fn path(&self) -> &'static str {
        "quests/Q00187_NikolasHeart"
    }

    fn id(&self) -> i32 {
        187
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RESEARCHER_LORAIN]);
        s.add_talk_id(&[HEAD_BLACKSMITH_KUSTO, RESEARCHER_LORAIN, MAESTRO_NIKOLA]);
        s.register_quest_items(&[METALLOGRAPH]);
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
            "30673-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.give_items(player, METALLOGRAPH, 1);
                    ctx.take_items(player, LORAINES_CERTIFICATE, -1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30512-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30512-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    ctx.give_adena(player, 93_383, true);
                    if ctx.player_level(player) < MAX_LEVEL_FOR_EXP_SP {
                        ctx.add_exp_and_sp(player, 285_935, 18_711);
                    }
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30621-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30621-03.html" => {
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
                if npc.id == RESEARCHER_LORAIN {
                    // `player.getQuestState(Q00185_NikolasCooperation)`: read only.
                    let previous_done = ctx
                        .player_quest_state(player, NIKOLAS_COOPERATION)
                        .is_some_and(|q185| q185.is_completed());
                    if previous_done && ctx.has_quest_items(player, LORAINES_CERTIFICATE) {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "30673-01.htm"
                        } else {
                            "30673-02.htm"
                        }
                        .to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                RESEARCHER_LORAIN => {
                    if memo_state >= 1 {
                        htmltext = "30673-04.html".to_owned();
                    }
                }
                HEAD_BLACKSMITH_KUSTO => {
                    if memo_state == 2 {
                        htmltext = "30512-01.html".to_owned();
                    }
                }
                MAESTRO_NIKOLA => {
                    if memo_state == 1 {
                        htmltext = "30621-01.html".to_owned();
                    } else if memo_state == 2 {
                        htmltext = "30621-04.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => {
                if npc.id == RESEARCHER_LORAIN {
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00187_NikolasHeart";

    const NPCS: &str = r#"
        <npc id="30512" type="Folk" level="70" name="Head Blacksmith Kusto"/>
        <npc id="30621" type="Folk" level="70" name="Maestro Nikola"/>
        <npc id="30673" type="Folk" level="70" name="Researcher Lorain"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10362" type="EtcItem" name="Loraine's Certificate"><set name="is_questitem" val="true"/></item>
        <item id="10368" type="EtcItem" name="Metallograph"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00185_NikolasCooperation` as far as this quest reads it: a script of that name.
    struct NikolasCooperation;

    impl Script for NikolasCooperation {
        fn name(&self) -> &'static str {
            NIKOLAS_COOPERATION
        }
        fn path(&self) -> &'static str {
            "quests/Q00185_NikolasCooperation"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NikolasCooperation);
        registry.add(Q00187NikolasHeart);
        let pages = Pages::of(&Q00187NikolasHeart)
            .page("30673-01.htm", &["30673-03.htm"])
            .plain(&["30673-02.htm", "30673-03.htm", "30673-04.html"])
            .page("30512-01.html", &["30512-02.html"])
            .page("30512-02.html", &["30512-03.html"])
            .plain(&["30512-03.html"])
            .page("30621-01.html", &["30621-02.html"])
            .page("30621-02.html", &["30621-03.html"])
            .plain(&["30621-03.html", "30621-04.html"])
            .npcs(&[HEAD_BLACKSMITH_KUSTO, MAESTRO_NIKOLA, RESEARCHER_LORAIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kusto = t.spawn(HEAD_BLACKSMITH_KUSTO, 20);
        let nikola = t.spawn(MAESTRO_NIKOLA, 40);
        let lorain = t.spawn(RESEARCHER_LORAIN, 60);
        (t, kusto, nikola, lorain)
    }

    fn finish_nikolas_cooperation(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(NIKOLAS_COOPERATION, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
    }

    #[test]
    fn only_a_character_of_level_41_with_the_certificate_is_offered_the_quest() {
        let (mut t, _, _, lorain) = setup();
        t.enter(1, 41, vec![item(100, LORAINES_CERTIFICATE, 1)]);
        t.enter(2, 40, vec![item(101, LORAINES_CERTIFICATE, 1)]);
        t.enter(3, 41, vec![]);
        for player in [1, 2, 3] {
            finish_nikolas_cooperation(&mut t, player);
        }
        t.take_writes();
        // Player 3 has no certificate: the no-quest message, not a page of this quest.
        assert!(!t.quest_page(3, lorain).contains("30673-"));
        assert!(t.quest_page(2, lorain).contains("30673-02.htm"));
        assert!(t.quest_page(1, lorain).contains("30673-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_memo_states_walk_the_three_npcs_and_the_reward() {
        let (mut t, kusto, nikola, lorain) = setup();
        t.enter_as(1, 41, race::HUMAN, 0, vec![item(100, LORAINES_CERTIFICATE, 1)]);
        finish_nikolas_cooperation(&mut t, 1);
        t.take_writes();

        assert!(t.quest_page(1, lorain).contains("30673-01.htm"));
        assert!(t.event(1, NAME, "30673-03.htm").contains("30673-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert_eq!((t.count(1, METALLOGRAPH), t.count(1, LORAINES_CERTIFICATE)), (1, 0));
        assert!(t.quest_page(1, lorain).contains("30673-04.html"));

        // Nikola: the memo state 2 and cond 2.
        assert!(t.quest_page(1, nikola).contains("30621-01.html"));
        assert!(t.event(1, NAME, "30621-02.html").contains("30621-02.html"));
        assert!(t.event(1, NAME, "30621-03.html").contains("30621-03.html"));
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "2"), rows::update(NAME, "cond", "2")]
        );
        assert!(t.quest_page(1, nikola).contains("30621-04.html"));

        // Kusto pays: adena, exp and sp (level 41 < 47), `exitQuest(false, true)`.
        assert!(t.quest_page(1, kusto).contains("30512-01.html"));
        assert!(t.event(1, NAME, "30512-02.html").contains("30512-02.html"));
        assert!(t.event(1, NAME, "30512-03.html").contains("30512-03.html"));
        assert_eq!(t.count(1, 57), 93_383);
        assert_eq!(
            t.take_writes(),
            [rows::delete(NAME, false), rows::update(NAME, "<state>", "Completed")]
        );
        assert!(t.quest_page(1, lorain).contains("already been completed"));
    }
}
