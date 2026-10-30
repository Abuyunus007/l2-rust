//! Quest 10268: To the Seed of Infinity (Keucereus sends the adventurer to Officer Tepios).
// Java: dist/game/data/scripts/quests/Q10268_ToTheSeedOfInfinity/Q10268_ToTheSeedOfInfinity.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KEUCEREUS: i32 = 32548;
const TEPIOS: i32 = 32603;

// Item
const INTRODUCTION: i32 = 13811;

/// Java `Q10268_ToTheSeedOfInfinity`.
pub struct Q10268ToTheSeedOfInfinity;

impl Script for Q10268ToTheSeedOfInfinity {
    fn name(&self) -> &'static str {
        "Q10268_ToTheSeedOfInfinity"
    }

    fn path(&self) -> &'static str {
        "quests/Q10268_ToTheSeedOfInfinity"
    }

    fn id(&self) -> i32 {
        10268
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KEUCEREUS]);
        s.add_talk_id(&[KEUCEREUS, TEPIOS]);
        s.register_quest_items(&[INTRODUCTION]);
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

        if event == "32548-05.html" {
            qs.start_quest(ctx);
            ctx.give_items(player, INTRODUCTION, 1);
        }

        Some(event.to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            KEUCEREUS => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) < 75 {
                        "32548-00.html"
                    } else {
                        "32548-01.htm"
                    }
                    .to_owned();
                }
                STARTED => htmltext = "32548-06.html".to_owned(),
                COMPLETED => htmltext = "32548-0a.html".to_owned(),
                _ => {}
            },
            TEPIOS => match qs.state(ctx) {
                STARTED => {
                    htmltext = "32530-01.html".to_owned();
                    ctx.give_adena(player, 16671, true);
                    ctx.add_exp_and_sp(player, 100640, 10098);
                    qs.exit_quest(ctx, false, true);
                }
                COMPLETED => htmltext = "32530-02.html".to_owned(),
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q10268_ToTheSeedOfInfinity";

    const NPCS: &str = r#"
        <npc id="32548" type="Folk" level="80" name="Keucereus"/>
        <npc id="32603" type="Folk" level="80" name="Officer Tepios"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13811" type="EtcItem" name="Letter of Introduction"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10268ToTheSeedOfInfinity);
        let pages = Pages::of(&Q10268ToTheSeedOfInfinity)
            .plain(&["32548-00.html", "32548-03.html", "32548-05.html", "32548-06.html"])
            .page("32548-01.htm", &["32548-02.htm", "32548-04.htm", "32548-03.html"])
            .page("32548-02.htm", &["32548-04.htm", "32548-03.html"])
            .page("32548-04.htm", &["32548-05.html"])
            .plain(&["32548-0a.html", "32530-01.html", "32530-02.html"])
            .npcs(&[KEUCEREUS, TEPIOS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let keucereus = t.spawn(KEUCEREUS, 20);
        let tepios = t.spawn(TEPIOS, 40);
        (t, keucereus, tepios)
    }

    #[test]
    fn a_character_below_level_75_is_sent_away() {
        let (mut t, keucereus, _) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, keucereus).contains("32548-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_letter_of_introduction_then_the_reward_from_tepios() {
        let (mut t, keucereus, tepios) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, keucereus).contains("32548-01.htm"));
        assert!(t.event(1, NAME, "32548-04.htm").contains("32548-04.htm"));

        // Accept: `startQuest()` and the letter.
        assert!(t.event(1, NAME, "32548-05.html").contains("32548-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert_eq!(t.count(1, INTRODUCTION), 1);
        assert!(t.quest_page(1, keucereus).contains("32548-06.html"));

        // Tepios takes the letter: 16671 adena (rates applied), the exp and `exitQuest(false, true)`.
        assert!(t.quest_page(1, tepios).contains("32530-01.html"));
        assert_eq!(t.count(1, 57), 16671);
        assert_eq!(t.count(1, INTRODUCTION), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, tepios).contains("32530-02.html"));
        assert!(t.quest_page(1, keucereus).contains("32548-0a.html"));
    }
}
