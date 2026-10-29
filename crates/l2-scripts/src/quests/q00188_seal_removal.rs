//! Quest 188: Seal Removal (Lorain, Maestro Nikola and Dorothy the Locksmith).
// Java: dist/game/data/scripts/quests/Q00188_SealRemoval/Q00188_SealRemoval.java

use l2_util::ObjectId;
use l2_world::script::COMPLETED;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MAESTRO_NIKOLA: i32 = 30621;
const RESEARCHER_LORAIN: i32 = 30673;
const DOROTHY_LOCKSMITH: i32 = 30970;

// Items
const LORAINES_CERTIFICATE: i32 = 10362;
const BROKEN_METAL_PIECES: i32 = 10369;

// Misc
const MIN_LEVEL: i32 = 41;
const MAX_LEVEL_FOR_EXP_SP: i32 = 47;

// The quests before this one (`getQuestState(name)` by class name).
const ART_OF_PERSUASION: &str = "Q00184_ArtOfPersuasion";
const NIKOLAS_COOPERATION: &str = "Q00185_NikolasCooperation";
const CONTRACT_EXECUTION: &str = "Q00186_ContractExecution";
const NIKOLAS_HEART: &str = "Q00187_NikolasHeart";

/// Java `Q00188_SealRemoval`.
pub struct Q00188SealRemoval;

impl Script for Q00188SealRemoval {
    fn name(&self) -> &'static str {
        "Q00188_SealRemoval"
    }

    fn path(&self) -> &'static str {
        "quests/Q00188_SealRemoval"
    }

    fn id(&self) -> i32 {
        188
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RESEARCHER_LORAIN]);
        s.add_talk_id(&[RESEARCHER_LORAIN, MAESTRO_NIKOLA, DOROTHY_LOCKSMITH]);
        s.register_quest_items(&[BROKEN_METAL_PIECES]);
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
                    ctx.give_items(player, BROKEN_METAL_PIECES, 1);
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
            "30621-04.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30970-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30970-03.html" => {
                if qs.is_memo_state(ctx, 2) {
                    ctx.give_adena(player, 98_583, true);
                    if ctx.player_level(player) < MAX_LEVEL_FOR_EXP_SP {
                        ctx.add_exp_and_sp(player, 285_935, 18_711);
                    }
                    qs.exit_quest(ctx, false, true);
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
        if qs.is_created(ctx) {
            if npc.id == RESEARCHER_LORAIN && !ctx.has_quest_items(player, LORAINES_CERTIFICATE) {
                // Java reads the four quest states by name: read only, a missing state is `null`.
                let done = |name: &str| ctx.player_quest_state(player, name).is_some_and(|q| q.is_completed());
                let missing = |name: &str| ctx.player_quest_state(player, name).is_none();
                let q184_done = done(ART_OF_PERSUASION);
                let q185_done = done(NIKOLAS_COOPERATION);
                let no_186 = missing(CONTRACT_EXECUTION);
                let no_187 = missing(NIKOLAS_HEART);
                if q184_done || (q185_done && no_186 && no_187) {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30673-01.htm"
                    } else {
                        "30673-02.htm"
                    }
                    .to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                RESEARCHER_LORAIN => htmltext = "30673-04.html".to_owned(),
                MAESTRO_NIKOLA => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "30621-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 2) {
                        htmltext = "30621-05.html".to_owned();
                    }
                }
                DOROTHY_LOCKSMITH => {
                    if qs.is_memo_state(ctx, 2) {
                        htmltext = "30970-01.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == RESEARCHER_LORAIN {
            htmltext = ctx.get_already_completed_msg(player);
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

    const NAME: &str = "Q00188_SealRemoval";

    const NPCS: &str = r#"
        <npc id="30621" type="Folk" level="70" name="Maestro Nikola"/>
        <npc id="30673" type="Folk" level="70" name="Researcher Lorain"/>
        <npc id="30970" type="Folk" level="70" name="Dorothy the Locksmith"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10362" type="EtcItem" name="Loraine's Certificate"><set name="is_questitem" val="true"/></item>
        <item id="10369" type="EtcItem" name="Broken Metal Pieces"><set name="is_questitem" val="true"/></item>"#;

    /// A quest of the chain this one reads by name (`getQuestState(name)`): a script of that name.
    struct StandIn(&'static str);

    impl Script for StandIn {
        fn name(&self) -> &'static str {
            self.0
        }
        fn path(&self) -> &'static str {
            "quests"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        for name in [ART_OF_PERSUASION, NIKOLAS_COOPERATION, CONTRACT_EXECUTION, NIKOLAS_HEART] {
            registry.add(StandIn(name));
        }
        registry.add(Q00188SealRemoval);
        let pages = Pages::of(&Q00188SealRemoval)
            .page("30673-01.htm", &["30673-03.htm"])
            .plain(&["30673-02.htm", "30673-03.htm", "30673-04.html"])
            .page("30621-01.html", &["30621-02.html"])
            .page("30621-02.html", &["30621-03.html"])
            .page("30621-03.html", &["30621-04.html"])
            .plain(&["30621-04.html", "30621-05.html"])
            .page("30970-01.html", &["30970-02.html"])
            .page("30970-02.html", &["30970-03.html"])
            .plain(&["30970-03.html"])
            .npcs(&[MAESTRO_NIKOLA, RESEARCHER_LORAIN, DOROTHY_LOCKSMITH]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let nikola = t.spawn(MAESTRO_NIKOLA, 20);
        let lorain = t.spawn(RESEARCHER_LORAIN, 40);
        let dorothy = t.spawn(DOROTHY_LOCKSMITH, 60);
        (t, nikola, lorain, dorothy)
    }

    /// Completes a quest of the chain (the stand-in of it).
    fn complete(t: &mut ScriptTest, name: &'static str, player: i32) {
        t.world.run_as_script(name, |ctx| {
            let qs = ctx.get_quest_state(Oid(player), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
    }

    #[test]
    fn the_offer_needs_art_of_persuasion_or_nikolas_cooperation_alone() {
        let (mut t, _, lorain, _) = setup();
        t.enter_as(1, 41, race::HUMAN, 0, vec![]);
        complete(&mut t, NIKOLAS_COOPERATION, 1);
        assert!(t.quest_page(1, lorain).contains("30673-01.htm"));

        // Nikolas' Heart has a state of this character: the 185 path is closed.
        t.enter_as(2, 41, race::HUMAN, 0, vec![]);
        complete(&mut t, NIKOLAS_COOPERATION, 2);
        complete(&mut t, NIKOLAS_HEART, 2);
        assert!(!t.quest_page(2, lorain).contains("30673-01.htm"));

        // Art of Persuasion alone is enough, and a level 40 character gets the other page.
        t.enter_as(3, 40, race::HUMAN, 0, vec![]);
        complete(&mut t, ART_OF_PERSUASION, 3);
        assert!(t.quest_page(3, lorain).contains("30673-02.htm"));
    }

    #[test]
    fn the_memo_states_then_the_reward() {
        let (mut t, nikola, lorain, dorothy) = setup();
        t.enter_as(1, 41, race::HUMAN, 0, vec![]);
        complete(&mut t, ART_OF_PERSUASION, 1);
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
        assert_eq!(t.count(1, BROKEN_METAL_PIECES), 1);
        assert!(t.quest_page(1, lorain).contains("30673-04.html"));

        assert!(t.quest_page(1, nikola).contains("30621-01.html"));
        assert!(t.event(1, NAME, "30621-02.html").contains("30621-02.html"));
        assert!(t.event(1, NAME, "30621-03.html").contains("30621-03.html"));
        assert_eq!(
            t.take_writes(),
            [rows::update(NAME, "memoState", "2"), rows::update(NAME, "cond", "2")]
        );
        assert!(t.quest_page(1, nikola).contains("30621-05.html"));

        // Dorothy pays: adena, exp and sp (level 41 < 47), `exitQuest(false, true)`.
        assert!(t.quest_page(1, dorothy).contains("30970-01.html"));
        assert!(t.event(1, NAME, "30970-02.html").contains("30970-02.html"));
        assert!(t.event(1, NAME, "30970-03.html").contains("30970-03.html"));
        assert_eq!(t.count(1, 57), 98_583);
        assert_eq!(
            t.take_writes(),
            [rows::delete(NAME, false), rows::update(NAME, "<state>", "Completed")]
        );
        assert!(t.quest_page(1, lorain).contains("already been completed"));
    }
}
