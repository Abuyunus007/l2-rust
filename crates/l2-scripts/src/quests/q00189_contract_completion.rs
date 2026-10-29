//! Quest 189: Contract Completion (Blueprint Seller Luka, Lorain, Shegfield and Kusto).
// Java: dist/game/data/scripts/quests/Q00189_ContractCompletion/Q00189_ContractCompletion.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SHEGFIELD: i32 = 30068;
const HEAD_BLACKSMITH_KUSTO: i32 = 30512;
const RESEARCHER_LORAIN: i32 = 30673;
const BLUEPRINT_SELLER_LUKA: i32 = 31437;

// Items
const SCROLL_OF_DECODING: i32 = 10370;

// Misc
const MIN_LEVEL: i32 = 42;
const MAX_LEVEL_FOR_EXP_SP: i32 = 48;

/// Java `Q00186_ContractExecution.class.getSimpleName()`: the quest this one follows.
const CONTRACT_EXECUTION: &str = "Q00186_ContractExecution";

/// Java `Q00189_ContractCompletion`.
pub struct Q00189ContractCompletion;

impl Script for Q00189ContractCompletion {
    fn name(&self) -> &'static str {
        "Q00189_ContractCompletion"
    }

    fn path(&self) -> &'static str {
        "quests/Q00189_ContractCompletion"
    }

    fn id(&self) -> i32 {
        189
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[BLUEPRINT_SELLER_LUKA]);
        s.add_talk_id(&[
            BLUEPRINT_SELLER_LUKA,
            HEAD_BLACKSMITH_KUSTO,
            RESEARCHER_LORAIN,
            SHEGFIELD,
        ]);
        s.register_quest_items(&[SCROLL_OF_DECODING]);
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
            "31437-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.give_items(player, SCROLL_OF_DECODING, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30512-02.html" => {
                if qs.is_memo_state(ctx, 4) {
                    ctx.give_adena(player, 121_527, true);
                    if ctx.player_level(player) < MAX_LEVEL_FOR_EXP_SP {
                        ctx.add_exp_and_sp(player, 309_467, 20614);
                    }
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30673-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    ctx.take_items(player, SCROLL_OF_DECODING, -1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30068-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30068-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
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
            CREATED => {
                if npc.id == BLUEPRINT_SELLER_LUKA {
                    // `player.getQuestState(Q00186_ContractExecution)`: read only.
                    let contract_done = ctx
                        .player_quest_state(player, CONTRACT_EXECUTION)
                        .is_some_and(|q186| q186.is_completed());
                    if contract_done {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "31437-01.htm"
                        } else {
                            "31437-02.htm"
                        }
                        .to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                BLUEPRINT_SELLER_LUKA => {
                    if qs.get_memo_state(ctx) >= 1 {
                        htmltext = "31437-04.html".to_owned();
                    }
                }
                HEAD_BLACKSMITH_KUSTO => {
                    if qs.is_memo_state(ctx, 4) {
                        htmltext = "30512-01.html".to_owned();
                    }
                }
                RESEARCHER_LORAIN => match qs.get_cond(ctx) {
                    1 => htmltext = "30673-01.html".to_owned(),
                    2 => htmltext = "30673-03.html".to_owned(),
                    3 => {
                        qs.set_memo_state(ctx, 4);
                        qs.set_cond(ctx, 4, true);
                        htmltext = "30673-04.html".to_owned();
                    }
                    4 => htmltext = "30673-05.html".to_owned(),
                    _ => {}
                },
                SHEGFIELD => match qs.get_cond(ctx) {
                    2 => htmltext = "30068-01.html".to_owned(),
                    3 => htmltext = "30068-04.html".to_owned(),
                    _ => {}
                },
                _ => {}
            },
            COMPLETED => {
                if npc.id == BLUEPRINT_SELLER_LUKA {
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
    use l2_world::script::{COMPLETED, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00189_ContractCompletion";

    const NPCS: &str = r#"
        <npc id="30068" type="Folk" level="70" name="Shegfield"/>
        <npc id="30512" type="Folk" level="70" name="Head Blacksmith Kusto"/>
        <npc id="30673" type="Folk" level="70" name="Researcher Lorain"/>
        <npc id="31437" type="Folk" level="70" name="Blueprint Seller Luka"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10370" type="EtcItem" name="Scroll of Decoding"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00186_ContractExecution` as far as this quest reads it: a script of that name.
    struct ContractExecution;

    impl Script for ContractExecution {
        fn name(&self) -> &'static str {
            CONTRACT_EXECUTION
        }
        fn path(&self) -> &'static str {
            "quests/Q00186_ContractExecution"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        luka: Oid,
        lorain: Oid,
        shegfield: Oid,
        kusto: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(ContractExecution);
        registry.add(Q00189ContractCompletion);
        let pages = Pages::of(&Q00189ContractCompletion)
            .page("31437-01.htm", &["31437-03.htm"])
            .plain(&["31437-02.htm", "31437-03.htm", "31437-04.html"])
            .page("30673-01.html", &["30673-02.html"])
            .plain(&["30673-02.html", "30673-03.html", "30673-04.html", "30673-05.html"])
            .page("30068-01.html", &["30068-02.html"])
            .page("30068-02.html", &["30068-03.html"])
            .plain(&["30068-03.html", "30068-04.html"])
            .page("30512-01.html", &["30512-02.html"])
            .plain(&["30512-02.html"])
            .npcs(&[
                SHEGFIELD,
                HEAD_BLACKSMITH_KUSTO,
                RESEARCHER_LORAIN,
                BLUEPRINT_SELLER_LUKA,
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            luka: t.spawn(BLUEPRINT_SELLER_LUKA, 20),
            lorain: t.spawn(RESEARCHER_LORAIN, 40),
            shegfield: t.spawn(SHEGFIELD, 60),
            kusto: t.spawn(HEAD_BLACKSMITH_KUSTO, 80),
        };
        (t, npcs)
    }

    #[test]
    fn the_offer_needs_contract_execution_and_level_42() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 42, race::HUMAN, 0, vec![]);
        assert!(!t.quest_page(1, npcs.luka).contains("31437-"));
        t.world.run_as_script(CONTRACT_EXECUTION, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.luka).contains("31437-01.htm"));

        t.enter_as(2, 41, race::HUMAN, 0, vec![]);
        t.world.run_as_script(CONTRACT_EXECUTION, |ctx| {
            let qs = ctx.get_quest_state(Oid(2), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
        assert!(t.quest_page(2, npcs.luka).contains("31437-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_scroll_then_shegfield_then_lorain_then_kusto_pays() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 42, race::HUMAN, 0, vec![]);
        t.world.run_as_script(CONTRACT_EXECUTION, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();

        assert!(t.quest_page(1, npcs.luka).contains("31437-01.htm"));
        assert!(t.event(1, NAME, "31437-03.htm").contains("31437-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert_eq!(t.count(1, SCROLL_OF_DECODING), 1);
        assert!(t.quest_page(1, npcs.luka).contains("31437-04.html"));

        // Lorain takes the scroll (cond 2).
        assert!(t.quest_page(1, npcs.lorain).contains("30673-01.html"));
        assert!(t.event(1, NAME, "30673-02.html").contains("30673-02.html"));
        assert_eq!(t.count(1, SCROLL_OF_DECODING), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, npcs.lorain).contains("30673-03.html"));

        // Shegfield: memo state 3 and cond 3.
        assert!(t.quest_page(1, npcs.shegfield).contains("30068-01.html"));
        assert!(t.event(1, NAME, "30068-02.html").contains("30068-02.html"));
        assert!(t.event(1, NAME, "30068-03.html").contains("30068-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3")
            ]
        );
        assert!(t.quest_page(1, npcs.shegfield).contains("30068-04.html"));

        // Lorain again: memo state 4 and cond 4.
        assert!(t.quest_page(1, npcs.lorain).contains("30673-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "4")
            ]
        );

        // Kusto pays: adena, exp and sp (level 42 < 48), `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.kusto).contains("30512-01.html"));
        assert!(t.event(1, NAME, "30512-02.html").contains("30512-02.html"));
        assert_eq!(t.count(1, 57), 121_527);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.luka).contains("already been completed"));
    }
}
