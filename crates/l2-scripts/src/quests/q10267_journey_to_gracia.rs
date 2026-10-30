//! Quest 10267: Journey To Gracia (High Priest Orven sends the adventurer to Papiku and Keucereus).
// Java: dist/game/data/scripts/quests/Q10267_JourneyToGracia/Q10267_JourneyToGracia.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ORVEN: i32 = 30857;
const KEUCEREUS: i32 = 32548;
const PAPIKU: i32 = 32564;

// Item
const LETTER: i32 = 13810;

/// Java `Q10267_JourneyToGracia`.
pub struct Q10267JourneyToGracia;

impl Script for Q10267JourneyToGracia {
    fn name(&self) -> &'static str {
        "Q10267_JourneyToGracia"
    }

    fn path(&self) -> &'static str {
        "quests/Q10267_JourneyToGracia"
    }

    fn id(&self) -> i32 {
        10267
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ORVEN]);
        s.add_talk_id(&[ORVEN, KEUCEREUS, PAPIKU]);
        s.register_quest_items(&[LETTER]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        match event {
            "30857-06.html" => {
                qs.start_quest(ctx);
                ctx.give_items(player, LETTER, 1);
            }
            "32564-02.html" => qs.set_cond(ctx, 2, true),
            "32548-02.html" => {
                ctx.give_adena(player, 92_500, true);
                ctx.add_exp_and_sp(player, 75_480, 7_570);
                qs.exit_quest(ctx, false, true);
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            ORVEN => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) < 75 {
                        "30857-00.html"
                    } else {
                        "30857-01.htm"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "30857-07.html".to_owned(),
                COMPLETED => htmltext = "30857-0a.html".to_owned(),
                _ => {}
            },
            PAPIKU => {
                if qs.is_started(ctx) {
                    htmltext = if qs.is_cond(ctx, 1) {
                        "32564-01.html"
                    } else {
                        "32564-03.html"
                    }
                    .to_owned();
                }
            }
            KEUCEREUS => {
                if qs.is_started(ctx) && qs.is_cond(ctx, 2) {
                    htmltext = "32548-01.html".to_owned();
                } else if qs.is_completed(ctx) {
                    htmltext = "32548-03.html".to_owned();
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

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q10267_JourneyToGracia";

    const NPCS: &str = r#"
        <npc id="30857" type="Folk" level="80" name="High Priest Orven"/>
        <npc id="32548" type="Folk" level="80" name="Keucereus"/>
        <npc id="32564" type="Folk" level="80" name="Magister Papiku"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13810" type="EtcItem" name="Letter of Orven"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10267JourneyToGracia);
        let pages = Pages::of(&Q10267JourneyToGracia)
            .plain(&["30857-00.html", "30857-04.html", "30857-07.html", "30857-0a.html"])
            .page("30857-01.htm", &["30857-02.htm", "30857-05.htm", "30857-04.html"])
            .page("30857-02.htm", &["30857-03.htm"])
            .page("30857-03.htm", &["30857-05.htm", "30857-04.html"])
            .page("30857-05.htm", &["30857-06.html"])
            .plain(&["30857-06.html"])
            .page("32564-01.html", &["32564-02.html"])
            .plain(&["32564-02.html", "32564-03.html"])
            .page("32548-01.html", &["32548-02.html"])
            .plain(&["32548-02.html", "32548-03.html"])
            .npcs(&[ORVEN, KEUCEREUS, PAPIKU]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let orven = t.spawn(ORVEN, 20);
        let keucereus = t.spawn(KEUCEREUS, 40);
        let papiku = t.spawn(PAPIKU, 60);
        (t, orven, keucereus, papiku)
    }

    #[test]
    fn a_character_below_level_75_is_sent_away() {
        let (mut t, orven, _, _) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, orven).contains("30857-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_letter_from_orven_to_keucereus_and_the_reward() {
        let (mut t, orven, keucereus, papiku) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, orven).contains("30857-01.htm"));
        assert!(t.event(1, NAME, "30857-05.htm").contains("30857-05.htm"));

        // Accept: `startQuest()` and the letter.
        assert!(t.event(1, NAME, "30857-06.html").contains("30857-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert_eq!(t.count(1, LETTER), 1);
        assert!(t.quest_page(1, orven).contains("30857-07.html"));

        // Papiku reads the letter: `setCond(2, true)`.
        assert!(t.quest_page(1, papiku).contains("32564-01.html"));
        assert!(t.event(1, NAME, "32564-02.html").contains("32564-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, papiku).contains("32564-03.html"));

        // Keucereus: the reward and `exitQuest(false, true)`.
        assert!(t.quest_page(1, keucereus).contains("32548-01.html"));
        assert!(t.event(1, NAME, "32548-02.html").contains("32548-02.html"));
        assert_eq!(t.count(1, 57), 92_500);
        assert_eq!(t.count(1, LETTER), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, keucereus).contains("32548-03.html"));
        assert!(t.quest_page(1, orven).contains("30857-0a.html"));
    }
}
