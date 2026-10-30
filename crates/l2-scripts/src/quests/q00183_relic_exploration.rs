//! Quest 183: Relic Exploration (Head Blacksmith Kusto, Researcher Lorain and Maestro Nikola).
// Java: dist/game/data/scripts/quests/Q00183_RelicExploration/Q00183_RelicExploration.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HEAD_BLACKSMITH_KUSTO: i32 = 30512;
const MAESTRO_NIKOLA: i32 = 30621;
const RESEARCHER_LORAIN: i32 = 30673;

// Misc
const MIN_LEVEL: i32 = 40;
const MAX_LEVEL_FOR_EXP_SP: i32 = 46;

/// Java `Q00184_ArtOfPersuasion.class.getSimpleName()`: the quest the "Contract" choice starts.
const ART_OF_PERSUASION: &str = "Q00184_ArtOfPersuasion";
/// Java `Q00185_NikolasCooperation.class.getSimpleName()`: the quest the "Consideration" choice starts.
const NIKOLAS_COOPERATION: &str = "Q00185_NikolasCooperation";

/// Java `Q00183_RelicExploration`.
pub struct Q00183RelicExploration;

impl Script for Q00183RelicExploration {
    fn name(&self) -> &'static str {
        "Q00183_RelicExploration"
    }

    fn path(&self) -> &'static str {
        "quests/Q00183_RelicExploration"
    }

    fn id(&self) -> i32 {
        183
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HEAD_BLACKSMITH_KUSTO]);
        s.add_talk_id(&[HEAD_BLACKSMITH_KUSTO, RESEARCHER_LORAIN, MAESTRO_NIKOLA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "30512-04.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "30512-02.htm" => htmltext = Some(event.to_owned()),
            "30621-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    ctx.give_adena(player, 18_100, true);
                    if ctx.player_level(player) < MAX_LEVEL_FOR_EXP_SP {
                        ctx.add_exp_and_sp(player, 60_000, 3000);
                    }
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30673-02.html" | "30673-03.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30673-04.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "Contract" => {
                // Java: the other quest's state is `null` and its script is installed; the
                // event goes to that script (`notifyEvent`, its page is shown by the engine).
                let qs184 = ctx.player_quest_state(player, ART_OF_PERSUASION).is_some();
                let qs185 = ctx.player_quest_state(player, NIKOLAS_COOPERATION).is_some();
                if ctx.world().has_script(ART_OF_PERSUASION) && !qs184 && !qs185 {
                    let page = if ctx.player_level(player) >= MIN_LEVEL {
                        "30621-03.htm"
                    } else {
                        "30621-03a.html"
                    };
                    ctx.notify_event_of(ART_OF_PERSUASION, page, npc, Some(player));
                }
            }
            "Consideration" => {
                let qs184 = ctx.player_quest_state(player, ART_OF_PERSUASION).is_some();
                let qs185 = ctx.player_quest_state(player, NIKOLAS_COOPERATION).is_some();
                if ctx.world().has_script(NIKOLAS_COOPERATION) && !qs184 && !qs185 {
                    let page = if ctx.player_level(player) >= MIN_LEVEL {
                        "30621-03.htm"
                    } else {
                        "30621-03a.html"
                    };
                    ctx.notify_event_of(NIKOLAS_COOPERATION, page, npc, Some(player));
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
                if npc.id == HEAD_BLACKSMITH_KUSTO {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30512-01.htm"
                    } else {
                        "30512-03.html"
                    }
                    .to_owned();
                }
            }
            STARTED => match npc.id {
                HEAD_BLACKSMITH_KUSTO => htmltext = "30512-05.html".to_owned(),
                MAESTRO_NIKOLA => {
                    if qs.is_memo_state(ctx, 2) {
                        htmltext = "30621-01.html".to_owned();
                    }
                }
                RESEARCHER_LORAIN => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "30673-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 2) {
                        htmltext = "30673-05.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
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

    const NAME: &str = "Q00183_RelicExploration";

    const NPCS: &str = r#"
        <npc id="30512" type="Folk" level="70" name="Head Blacksmith Kusto"/>
        <npc id="30621" type="Folk" level="70" name="Maestro Nikola"/>
        <npc id="30673" type="Folk" level="70" name="Researcher Lorain"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    /// The events a stand-in quest was sent (`notifyEvent` of the "Contract" choice).
    #[derive(Default)]
    struct Heard(Vec<String>);

    /// A quest of the choice this one sends events to: it keeps the events it gets.
    struct Listener(&'static str);

    impl Script for Listener {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
        fn on_event(
            &self,
            ctx: &mut ScriptCtx<'_>,
            event: &str,
            _npc: Option<ScriptNpc>,
            _player: Option<ObjectId>,
        ) -> Option<String> {
            ctx.state::<Heard>().0.push(event.to_owned());
            None
        }
    }

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Listener(ART_OF_PERSUASION));
        registry.add(Listener(NIKOLAS_COOPERATION));
        registry.add(Q00183RelicExploration);
        let pages = Pages::of(&Q00183RelicExploration)
            .page("30512-01.htm", &["30512-04.htm"])
            .plain(&["30512-02.htm", "30512-03.html", "30512-04.htm", "30512-05.html"])
            .page("30673-01.html", &["30673-02.html"])
            .page("30673-02.html", &["30673-03.html"])
            .page("30673-03.html", &["30673-04.html"])
            .plain(&["30673-04.html", "30673-05.html"])
            .page("30621-01.html", &["30621-02.html"])
            .page("30621-02.html", &["Contract", "Consideration"])
            .npcs(&[HEAD_BLACKSMITH_KUSTO, MAESTRO_NIKOLA, RESEARCHER_LORAIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kusto = t.spawn(HEAD_BLACKSMITH_KUSTO, 20);
        let nikola = t.spawn(MAESTRO_NIKOLA, 40);
        let lorain = t.spawn(RESEARCHER_LORAIN, 60);
        (t, kusto, nikola, lorain)
    }

    fn heard(t: &mut ScriptTest, name: &'static str) -> Vec<String> {
        let mut out = Vec::new();
        t.world
            .run_as_script(name, |ctx| out = ctx.state::<Heard>().0.clone());
        out
    }

    #[test]
    fn the_level_40_characters_are_offered_the_quest() {
        let (mut t, kusto, _, _) = setup();
        t.enter_as(1, 39, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, kusto).contains("30512-03.html"));
        t.enter_as(2, 40, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(2, kusto).contains("30512-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_relic_then_the_adena_and_the_choice_of_the_other_quest() {
        let (mut t, kusto, nikola, lorain) = setup();
        t.enter_as(1, 40, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, kusto).contains("30512-01.htm"));
        assert!(t.event(1, NAME, "30512-04.htm").contains("30512-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert!(t.quest_page(1, kusto).contains("30512-05.html"));

        // Lorain: memo state 2 and cond 2 (`setCond(2, true)`).
        assert!(t.quest_page(1, lorain).contains("30673-01.html"));
        assert!(t.event(1, NAME, "30673-02.html").contains("30673-02.html"));
        assert!(t.event(1, NAME, "30673-03.html").contains("30673-03.html"));
        assert!(t.event(1, NAME, "30673-04.html").contains("30673-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );
        assert!(t.quest_page(1, lorain).contains("30673-05.html"));

        // Nikola's page: the reward (adena, exp and sp at level 40 < 46), `exitQuest(false, true)`.
        assert!(t.quest_page(1, nikola).contains("30621-01.html"));
        assert!(t.event(1, NAME, "30621-02.html").contains("30621-02.html"));
        assert_eq!(t.count(1, 57), 18_100);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );

        // The choice of the page sends the event to Art of Persuasion (level 40: the "a" page).
        t.take();
        assert!(t.event(1, NAME, "Contract").is_empty());
        assert_eq!(heard(&mut t, ART_OF_PERSUASION), ["30621-03.htm"]);
        assert!(heard(&mut t, NIKOLAS_COOPERATION).is_empty());
    }

    #[test]
    fn the_consideration_goes_to_nikolas_cooperation_below_level_40_with_the_a_page() {
        let (mut t, _, nikola, _) = setup();
        t.enter_as(1, 39, race::HUMAN, 0, vec![]);
        // Level 39 and no quest of this one in memo state 2: the reward runs through the memo.
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, STARTED);
            qs.set_memo_state(ctx, 2);
        });
        t.take_writes();
        assert!(t.quest_page(1, nikola).contains("30621-01.html"));
        assert!(t.event(1, NAME, "30621-02.html").contains("30621-02.html"));
        assert!(t.event(1, NAME, "Consideration").is_empty());
        assert_eq!(heard(&mut t, NIKOLAS_COOPERATION), ["30621-03a.html"]);
    }
}
