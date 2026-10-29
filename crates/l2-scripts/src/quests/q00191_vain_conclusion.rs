//! Quest 191: Vain Conclusion (Dorothy the Locksmith, Shegfield, Lorain and Kusto).
// Java: dist/game/data/scripts/quests/Q00191_VainConclusion/Q00191_VainConclusion.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SHEGFIELD: i32 = 30068;
const HEAD_BLACKSMITH_KUSTO: i32 = 30512;
const RESEARCHER_LORAIN: i32 = 30673;
const DOROTHY_LOCKSMITH: i32 = 30970;

// Items
const REPAIRED_METALLOGRAPH: i32 = 10371;

// Misc
const MIN_LEVEL: i32 = 42;
const MAX_LEVEL_FOR_EXP_SP: i32 = 48;

/// Java `Q00188_SealRemoval.class.getSimpleName()`: the quest this one follows.
const SEAL_REMOVAL: &str = "Q00188_SealRemoval";

/// Java `Q00191_VainConclusion`.
pub struct Q00191VainConclusion;

impl Script for Q00191VainConclusion {
    fn name(&self) -> &'static str {
        "Q00191_VainConclusion"
    }

    fn path(&self) -> &'static str {
        "quests/Q00191_VainConclusion"
    }

    fn id(&self) -> i32 {
        191
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DOROTHY_LOCKSMITH]);
        s.add_talk_id(&[DOROTHY_LOCKSMITH, HEAD_BLACKSMITH_KUSTO, RESEARCHER_LORAIN, SHEGFIELD]);
        s.register_quest_items(&[REPAIRED_METALLOGRAPH]);
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
            "30970-03.htm" => htmltext = Some(event.to_owned()),
            "30970-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.give_items(player, REPAIRED_METALLOGRAPH, 1);
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
            "30512-02.html" => {
                if qs.is_memo_state(ctx, 4) {
                    ctx.give_adena(player, 117_327, true);
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
                    ctx.take_items(player, REPAIRED_METALLOGRAPH, -1);
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
                if npc.id == DOROTHY_LOCKSMITH {
                    // `player.getQuestState(Q00188_SealRemoval)`: read only.
                    let seal_done = ctx
                        .player_quest_state(player, SEAL_REMOVAL)
                        .is_some_and(|q188| q188.is_completed());
                    if seal_done {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "30970-01.htm"
                        } else {
                            "30970-02.htm"
                        }
                        .to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                DOROTHY_LOCKSMITH => {
                    if qs.get_memo_state(ctx) >= 1 {
                        htmltext = "30970-05.html".to_owned();
                    }
                }
                SHEGFIELD => match qs.get_cond(ctx) {
                    2 => htmltext = "30068-01.html".to_owned(),
                    3 => htmltext = "30068-04.html".to_owned(),
                    _ => {}
                },
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
                _ => {}
            },
            COMPLETED => {
                if npc.id == DOROTHY_LOCKSMITH {
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

    const NAME: &str = "Q00191_VainConclusion";

    const NPCS: &str = r#"
        <npc id="30068" type="Folk" level="70" name="Shegfield"/>
        <npc id="30512" type="Folk" level="70" name="Head Blacksmith Kusto"/>
        <npc id="30673" type="Folk" level="70" name="Researcher Lorain"/>
        <npc id="30970" type="Folk" level="70" name="Dorothy the Locksmith"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10371" type="EtcItem" name="Repaired Metallograph"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00188_SealRemoval` as far as this quest reads it: a script of that name.
    struct SealRemoval;

    impl Script for SealRemoval {
        fn name(&self) -> &'static str {
            SEAL_REMOVAL
        }
        fn path(&self) -> &'static str {
            "quests/Q00188_SealRemoval"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    struct Npcs {
        dorothy: Oid,
        lorain: Oid,
        shegfield: Oid,
        kusto: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(SealRemoval);
        registry.add(Q00191VainConclusion);
        let pages = Pages::of(&Q00191VainConclusion)
            .page("30970-01.htm", &["30970-03.htm"])
            .page("30970-03.htm", &["30970-04.htm"])
            .plain(&["30970-02.htm", "30970-04.htm", "30970-05.html"])
            .page("30673-01.html", &["30673-02.html"])
            .plain(&["30673-02.html", "30673-03.html", "30673-04.html", "30673-05.html"])
            .page("30068-01.html", &["30068-02.html"])
            .page("30068-02.html", &["30068-03.html"])
            .plain(&["30068-03.html", "30068-04.html"])
            .page("30512-01.html", &["30512-02.html"])
            .plain(&["30512-02.html"])
            .npcs(&[SHEGFIELD, HEAD_BLACKSMITH_KUSTO, RESEARCHER_LORAIN, DOROTHY_LOCKSMITH]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            dorothy: t.spawn(DOROTHY_LOCKSMITH, 20),
            lorain: t.spawn(RESEARCHER_LORAIN, 40),
            shegfield: t.spawn(SHEGFIELD, 60),
            kusto: t.spawn(HEAD_BLACKSMITH_KUSTO, 80),
        };
        (t, npcs)
    }

    fn finish_seal_removal(t: &mut ScriptTest, player: i32) {
        t.world.run_as_script(SEAL_REMOVAL, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    #[test]
    fn the_offer_needs_seal_removal_and_the_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 42, race::HUMAN, 0, vec![]);
        assert!(!t.quest_page(1, npcs.dorothy).contains("30970-"));
        finish_seal_removal(&mut t, 1);
        assert!(t.quest_page(1, npcs.dorothy).contains("30970-01.htm"));
        t.enter_as(2, 41, race::HUMAN, 0, vec![]);
        finish_seal_removal(&mut t, 2);
        assert!(t.quest_page(2, npcs.dorothy).contains("30970-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_metallograph_then_the_steps_then_the_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 42, race::HUMAN, 0, vec![]);
        finish_seal_removal(&mut t, 1);
        assert!(t.quest_page(1, npcs.dorothy).contains("30970-01.htm"));
        assert!(t.event(1, NAME, "30970-03.htm").contains("30970-03.htm"));
        assert!(t.take_writes().is_empty());
        assert!(t.event(1, NAME, "30970-04.htm").contains("30970-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert_eq!(t.count(1, REPAIRED_METALLOGRAPH), 1);
        assert!(t.quest_page(1, npcs.dorothy).contains("30970-05.html"));

        // Lorain takes the metallograph (memo state 2, cond 2).
        assert!(t.quest_page(1, npcs.lorain).contains("30673-01.html"));
        assert!(t.event(1, NAME, "30673-02.html").contains("30673-02.html"));
        assert_eq!(t.count(1, REPAIRED_METALLOGRAPH), 0);
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "2"), rows::update(NAME, "cond", "2")]
        );
        assert!(t.quest_page(1, npcs.lorain).contains("30673-03.html"));

        // Shegfield: memo state 3 and cond 3.
        assert!(t.quest_page(1, npcs.shegfield).contains("30068-01.html"));
        assert!(t.event(1, NAME, "30068-02.html").contains("30068-02.html"));
        assert!(t.event(1, NAME, "30068-03.html").contains("30068-03.html"));
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "3"), rows::update(NAME, "cond", "3")]
        );
        assert!(t.quest_page(1, npcs.shegfield).contains("30068-04.html"));

        // Lorain again: memo state 4 and cond 4.
        assert!(t.quest_page(1, npcs.lorain).contains("30673-04.html"));
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "4"), rows::update(NAME, "cond", "4")]
        );

        // Kusto pays: adena, exp and sp (level 42 < 48), `exitQuest(false, true)`.
        assert!(t.quest_page(1, npcs.kusto).contains("30512-01.html"));
        assert!(t.event(1, NAME, "30512-02.html").contains("30512-02.html"));
        assert_eq!(t.count(1, 57), 117_327);
        assert_eq!(
            t.take_writes(),
            [rows::delete(NAME, false), rows::update(NAME, "<state>", "Completed")]
        );
        assert!(t.quest_page(1, npcs.dorothy).contains("already been completed"));
    }
}
