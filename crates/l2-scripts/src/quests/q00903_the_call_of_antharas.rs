//! Quest 903: The Call of Antharas (Theodric gives the Antharas scroll after two dragon kills a day).
// Java: dist/game/data/scripts/quests/Q00903_TheCallOfAntharas/Q00903_TheCallOfAntharas.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const THEODRIC: i32 = 30755;

// Monsters
const BEHEMOTH_DRAGON: i32 = 29069;
const TARASK_DRAGON: i32 = 29190;

// Items
const TARASK_DRAGONS_LEATHER_FRAGMENT: i32 = 21991;
const BEHEMOTH_DRAGON_LEATHER: i32 = 21992;
const SCROLL_ANTHARAS_CALL: i32 = 21897;
const PORTAL_STONE: i32 = 3865;

// Misc
const MIN_LEVEL: i32 = 83;

/// Java `Q00903_TheCallOfAntharas`.
pub struct Q00903TheCallOfAntharas;

impl Script for Q00903TheCallOfAntharas {
    fn name(&self) -> &'static str {
        "Q00903_TheCallOfAntharas"
    }

    fn path(&self) -> &'static str {
        "quests/Q00903_TheCallOfAntharas"
    }

    fn id(&self) -> i32 {
        903
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[THEODRIC]);
        s.add_talk_id(&[THEODRIC]);
        s.add_kill_id(&[BEHEMOTH_DRAGON, TARASK_DRAGON]);
        s.register_quest_items(&[TARASK_DRAGONS_LEATHER_FRAGMENT, BEHEMOTH_DRAGON_LEATHER]);
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
        if ctx.player_level(player) >= MIN_LEVEL && ctx.has_quest_items(player, PORTAL_STONE) {
            match event {
                "30755-05.htm" => htmltext = Some(event.to_owned()),
                "30755-06.html" => {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
                _ => {}
            }
        }
        htmltext
    }

    fn action_for_each_player(
        &self,
        ctx: &mut ScriptCtx<'_>,
        player: ObjectId,
        npc: ScriptNpc,
        _is_summon: bool,
    ) {
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return;
        };
        if ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false) {
            match npc.id {
                BEHEMOTH_DRAGON => {
                    ctx.give_items(player, BEHEMOTH_DRAGON_LEATHER, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                }
                TARASK_DRAGON => {
                    ctx.give_items(player, TARASK_DRAGONS_LEATHER_FRAGMENT, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                }
                _ => {}
            }

            if ctx.has_quest_items(player, BEHEMOTH_DRAGON_LEATHER)
                && ctx.has_quest_items(player, TARASK_DRAGONS_LEATHER_FRAGMENT)
            {
                qs.set_cond(ctx, 2, true);
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        ctx.execute_for_each_player(killer, npc, is_summon, true, false);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30755-03.html"
                } else if !ctx.has_quest_items(player, PORTAL_STONE) {
                    "30755-04.html"
                } else {
                    "30755-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30755-07.html".to_owned(),
                2 => {
                    ctx.give_items(player, SCROLL_ANTHARAS_CALL, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                    htmltext = "30755-08.html".to_owned();
                }
                _ => {}
            },
            COMPLETED => {
                if !qs.is_now_available(ctx) {
                    htmltext = "30755-02.html".to_owned();
                } else {
                    qs.set_state(ctx, CREATED);
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "30755-03.html"
                    } else if !ctx.has_quest_items(player, PORTAL_STONE) {
                        "30755-04.html"
                    } else {
                        "30755-01.htm"
                    }
                    .to_owned();
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
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00903_TheCallOfAntharas";

    const NPCS: &str = r#"
        <npc id="30755" type="Folk" level="80" name="Theodric"/>
        <npc id="29069" type="Monster" level="80" name="Behemoth Dragon"/>
        <npc id="29190" type="Monster" level="80" name="Tarask Dragon"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3865" type="EtcItem" name="Portal Stone"/>
        <item id="21991" type="EtcItem" name="Tarask Dragon's Leather Fragment"><set name="is_questitem" val="true"/></item>
        <item id="21992" type="EtcItem" name="Behemoth Dragon Leather"><set name="is_questitem" val="true"/></item>
        <item id="21897" type="EtcItem" name="Scroll: Call of Antharas"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00903TheCallOfAntharas);
        let pages = Pages::of(&Q00903TheCallOfAntharas)
            .plain(&["30755-02.html", "30755-03.html", "30755-04.html", "30755-07.html"])
            .plain(&["30755-08.html", "30755-06.html"])
            .page("30755-05.htm", &["30755-06.html"])
            .page("30755-01.htm", &["30755-05.htm"])
            .npcs(&[THEODRIC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let theodric = t.spawn(THEODRIC, 20);
        (t, theodric)
    }

    /// A Portal Stone in the inventory.
    fn stone() -> Vec<l2_world::ItemData> {
        vec![crate::testing::item(2, PORTAL_STONE, 1)]
    }

    #[test]
    fn a_character_below_level_83_or_without_the_stone_is_sent_away() {
        let (mut t, theodric) = setup();
        t.enter(1, 82, stone());
        assert!(t.quest_page(1, theodric).contains("30755-03.html"));
        t.enter(2, 83, vec![]);
        assert!(t.quest_page(2, theodric).contains("30755-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn two_dragon_kills_then_the_scroll_once_a_day() {
        let (mut t, theodric) = setup();
        t.enter(1, 83, stone());
        assert!(t.quest_page(1, theodric).contains("30755-01.htm"));
        assert!(t.event(1, NAME, "30755-05.htm").contains("30755-05.htm"));
        assert!(t.event(1, NAME, "30755-06.html").contains("30755-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, theodric).contains("30755-07.html"));

        // The Behemoth Dragon gives its leather; the Tarask Dragon the fragment, then cond 2.
        let behemoth = t.spawn(BEHEMOTH_DRAGON, 60);
        t.kill(1, behemoth);
        assert_eq!(t.count(1, BEHEMOTH_DRAGON_LEATHER), 1);
        assert!(t.take_writes().is_empty());
        let tarask = t.spawn(TARASK_DRAGON, 60);
        t.kill(1, tarask);
        assert_eq!(t.count(1, TARASK_DRAGONS_LEATHER_FRAGMENT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The scroll; the quest is daily from now on.
        assert!(t.quest_page(1, theodric).contains("30755-08.html"));
        assert_eq!(t.count(1, SCROLL_ANTHARAS_CALL), 1);
        assert_eq!(t.count(1, BEHEMOTH_DRAGON_LEATHER), 0);
        // `exitQuest(DAILY)`: the rows go, the state is completed and `restartTime` is set (its
        // value is the next 6:30, so only its name is checked).
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.quest_page(1, theodric).contains("30755-02.html"));
    }
}
