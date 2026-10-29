//! Quest 156: Millennium Love (Lilith sends the player's letter to Baenedes and back).
// Java: dist/game/data/scripts/quests/Q00156_MillenniumLove/Q00156_MillenniumLove.java
//
// Pattern: a delivery chain over two NPCs. Lilith starts the quest (the level gate is on the
// accept link, not on the first page); Baenedes either takes the letter for the diary (cond 2)
// or ends the quest for exp at once; Lilith takes the diary for the package and exp. One-time.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LILITH: i32 = 30368;
const BAENEDES: i32 = 30369;

// Items
const LILITHS_LETTER: i32 = 1022;
const THEONS_DIARY: i32 = 1023;
const GREATER_COMP_SOULSHOUT_PACKAGE_NO_GRADE: i32 = 5250;

// Misc
const MIN_LEVEL: i32 = 15;

/// Java `Q00156_MillenniumLove`.
pub struct Q00156MillenniumLove;

impl Script for Q00156MillenniumLove {
    fn name(&self) -> &'static str {
        "Q00156_MillenniumLove"
    }

    fn path(&self) -> &'static str {
        "quests/Q00156_MillenniumLove"
    }

    fn id(&self) -> i32 {
        156
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LILITH]);
        s.add_talk_id(&[LILITH, BAENEDES]);
        s.register_quest_items(&[LILITHS_LETTER, THEONS_DIARY]);
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
        match event {
            "30368-02.html" | "30368-03.html" => Some(event.to_owned()),
            "30368-05.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    ctx.give_items(player, LILITHS_LETTER, 1);
                    Some(event.to_owned())
                } else {
                    Some("30368-04.htm".to_owned())
                }
            }
            "30369-02.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, LILITHS_LETTER) {
                    ctx.take_items(player, LILITHS_LETTER, 1);
                    ctx.give_items(player, THEONS_DIARY, 1);
                    qs.set_cond(ctx, 2, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            "30369-03.html" => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, LILITHS_LETTER) {
                    ctx.add_exp_and_sp(player, 3_000, 0);
                    qs.exit_quest(ctx, false, true);
                    Some(event.to_owned())
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            LILITH => match qs.state(ctx) {
                CREATED => htmltext = "30368-01.htm".to_owned(),
                STARTED => {
                    if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, LILITHS_LETTER) {
                        htmltext = "30368-06.html".to_owned();
                    } else if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, THEONS_DIARY) {
                        ctx.give_items(player, GREATER_COMP_SOULSHOUT_PACKAGE_NO_GRADE, 1);
                        ctx.add_exp_and_sp(player, 3_000, 0);
                        qs.exit_quest(ctx, false, true);
                        htmltext = "30368-07.html".to_owned();
                    }
                }
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            BAENEDES => match qs.get_cond(ctx) {
                1 => {
                    if ctx.has_quest_items(player, LILITHS_LETTER) {
                        htmltext = "30369-01.html".to_owned();
                    }
                }
                2 => {
                    if ctx.has_quest_items(player, THEONS_DIARY) {
                        htmltext = "30369-04.html".to_owned();
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
    use l2_world::{QuestLoad, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, player_data, rows};

    const NAME: &str = "Q00156_MillenniumLove";

    const NPCS: &str = r#"
        <npc id="30368" type="Folk" level="1" name="Lilith"/>
        <npc id="30369" type="Folk" level="1" name="Baenedes"/>"#;

    const ITEMS: &str = r#"
        <item id="1022" type="EtcItem" name="Lilith's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1023" type="EtcItem" name="Theon's Diary"><set name="is_questitem" val="true"/></item>
        <item id="5250" type="EtcItem" name="Greater Combat Soulshout Package"/>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00156MillenniumLove);
        let pages = Pages::of(&Q00156MillenniumLove)
            .page("30368-01.htm", &["30368-02.html"])
            .page("30368-02.html", &["30368-03.html"])
            .page("30368-03.html", &["30368-05.htm"])
            .page("30368-05.htm", &[])
            .page("30369-01.html", &["30369-02.html", "30369-03.html"])
            .plain(&[
                "30368-04.htm",
                "30368-06.html",
                "30368-07.html",
                "30369-02.html",
                "30369-03.html",
                "30369-04.html",
            ])
            .npcs(&[LILITH, BAENEDES]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let lilith = t.spawn(LILITH, 20);
        let baenedes = t.spawn(BAENEDES, 40);
        (t, lilith, baenedes)
    }

    #[test]
    fn a_character_below_level_15_is_refused_at_the_accept_link() {
        let (mut t, lilith, _) = setup();
        t.enter(1, 14, vec![]);
        assert!(t.quest_page(1, lilith).contains("30368-01.htm"));
        t.event(1, NAME, "30368-02.html");
        t.event(1, NAME, "30368-03.html");
        assert!(t.event(1, NAME, "30368-05.htm").contains("30368-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_letter_the_diary_and_the_package() {
        let (mut t, lilith, baenedes) = setup();
        t.enter(1, 15, vec![]);
        assert!(t.quest_page(1, lilith).contains("30368-01.htm"));
        assert!(t.event(1, NAME, "30368-02.html").contains("30368-02.html"));
        assert!(t.event(1, NAME, "30368-03.html").contains("30368-03.html"));
        assert!(t.event(1, NAME, "30368-05.htm").contains("30368-05.htm"));
        assert_eq!(t.count(1, LILITHS_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, lilith).contains("30368-06.html"));

        // Baenedes takes the letter for the diary: cond 2.
        assert!(t.quest_page(1, baenedes).contains("30369-01.html"));
        assert!(t.event(1, NAME, "30369-02.html").contains("30369-02.html"));
        assert_eq!(t.count(1, LILITHS_LETTER), 0);
        assert_eq!(t.count(1, THEONS_DIARY), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, baenedes).contains("30369-04.html"));

        // Lilith takes the diary: the package and 3000 exp, `exitQuest(false, true)`.
        assert!(t.quest_page(1, lilith).contains("30368-07.html"));
        assert_eq!(t.count(1, GREATER_COMP_SOULSHOUT_PACKAGE_NO_GRADE), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn baenedes_ends_the_quest_for_exp_at_once() {
        let (mut t, _, baenedes) = setup();
        let mut data = player_data(1, 20, vec![item(5, LILITHS_LETTER, 1)]);
        data.profile.quests = QuestLoad {
            states: vec![(NAME.to_owned(), Some("Started".to_owned()))],
            variables: vec![(NAME.to_owned(), "cond".to_owned(), Some("1".to_owned()))],
        };
        t.enter_with(data);
        assert!(t.quest_page(1, baenedes).contains("30369-01.html"));
        assert!(t.event(1, NAME, "30369-03.html").contains("30369-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
