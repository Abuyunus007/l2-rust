//! Quest 10269: To the Seed of Destruction (Keucereus sends the adventurer to Officer Allenos).
// Java: dist/game/data/scripts/quests/Q10269_ToTheSeedOfDestruction/Q10269_ToTheSeedOfDestruction.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KEUCEREUS: i32 = 32548;
const ALLENOS: i32 = 32526;

// Item
const INTRODUCTION: i32 = 13812;

/// Java `Q10269_ToTheSeedOfDestruction`.
pub struct Q10269ToTheSeedOfDestruction;

impl Script for Q10269ToTheSeedOfDestruction {
    fn name(&self) -> &'static str {
        "Q10269_ToTheSeedOfDestruction"
    }

    fn path(&self) -> &'static str {
        "quests/Q10269_ToTheSeedOfDestruction"
    }

    fn id(&self) -> i32 {
        10269
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KEUCEREUS]);
        s.add_talk_id(&[KEUCEREUS, ALLENOS]);
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
            ALLENOS => match qs.state(ctx) {
                STARTED => {
                    htmltext = "32526-01.html".to_owned();
                    ctx.give_adena(player, 29174, true);
                    ctx.add_exp_and_sp(player, 176121, 7671);
                    qs.exit_quest(ctx, false, true);
                }
                COMPLETED => htmltext = "32526-02.html".to_owned(),
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

    const NAME: &str = "Q10269_ToTheSeedOfDestruction";

    const NPCS: &str = r#"
        <npc id="32548" type="Folk" level="80" name="Keucereus"/>
        <npc id="32526" type="Folk" level="80" name="Officer Allenos"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13812" type="EtcItem" name="Letter of Recommendation"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10269ToTheSeedOfDestruction);
        let pages = Pages::of(&Q10269ToTheSeedOfDestruction)
            .plain(&["32548-00.html", "32548-03.html", "32548-05.html", "32548-06.html"])
            .page("32548-01.htm", &["32548-02.htm", "32548-04.htm", "32548-03.html"])
            .page("32548-02.htm", &["32548-04.htm", "32548-03.html"])
            .page("32548-04.htm", &["32548-05.html"])
            .plain(&["32548-0a.html", "32526-01.html", "32526-02.html"])
            .npcs(&[KEUCEREUS, ALLENOS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let keucereus = t.spawn(KEUCEREUS, 20);
        let allenos = t.spawn(ALLENOS, 40);
        (t, keucereus, allenos)
    }

    #[test]
    fn a_character_below_level_75_is_sent_away() {
        let (mut t, keucereus, _) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, keucereus).contains("32548-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_letter_of_introduction_then_the_reward_from_allenos() {
        let (mut t, keucereus, allenos) = setup();
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

        // Allenos takes the letter: 29174 adena (rates applied), the exp and `exitQuest(false, true)`.
        assert!(t.quest_page(1, allenos).contains("32526-01.html"));
        assert_eq!(t.count(1, 57), 29174);
        assert_eq!(t.count(1, INTRODUCTION), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, allenos).contains("32526-02.html"));
        assert!(t.quest_page(1, keucereus).contains("32548-0a.html"));
    }
}
