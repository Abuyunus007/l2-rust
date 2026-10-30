//! Quest 904: Dragon Trophy - Antharas (Theodric sends the party after Antharas, 30 medals after the kill).
// Java: dist/game/data/scripts/quests/Q00904_DragonTrophyAntharas/Q00904_DragonTrophyAntharas.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{QuestWrite, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const THEODRIC: i32 = 30755;

// Monster
const ANTHARAS: i32 = 29068;

// Items
const MEDAL_OF_GLORY: i32 = 21874;
const PORTAL_STONE: i32 = 3865;

// Misc
const MIN_LEVEL: i32 = 84;

/// Java `Q00904_DragonTrophyAntharas`.
pub struct Q00904DragonTrophyAntharas;

impl Script for Q00904DragonTrophyAntharas {
    fn name(&self) -> &'static str {
        "Q00904_DragonTrophyAntharas"
    }

    fn path(&self) -> &'static str {
        "quests/Q00904_DragonTrophyAntharas"
    }

    fn id(&self) -> i32 {
        904
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[THEODRIC]);
        s.add_talk_id(&[THEODRIC]);
        s.add_kill_id(&[ANTHARAS]);
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
        if qs.is_cond(ctx, 1) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false) {
            qs.set_cond(ctx, 2, true);
        }
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
                "30755-05.htm" | "30755-06.htm" => htmltext = Some(event.to_owned()),
                "30755-07.html" => {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
                _ => {}
            }
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        ctx.execute_for_each_player(killer, npc, is_summon, true, true);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "30755-02.html"
                } else if !ctx.has_quest_items(player, PORTAL_STONE) {
                    "30755-04.html"
                } else {
                    "30755-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30755-08.html".to_owned(),
                2 => {
                    ctx.give_items(player, MEDAL_OF_GLORY, 30);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                    htmltext = "30755-09.html".to_owned();
                }
                _ => {}
            },
            COMPLETED => {
                if !qs.is_now_available(ctx) {
                    htmltext = "30755-03.html".to_owned();
                } else {
                    qs.set_state(ctx, CREATED);
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "30755-02.html"
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
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00904_DragonTrophyAntharas";

    const NPCS: &str = r#"
        <npc id="30755" type="Folk" level="80" name="Theodric"/>
        <npc id="29068" type="Monster" level="80" name="Antharas"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="3865" type="EtcItem" name="Portal Stone"/>
        <item id="21874" type="EtcItem" name="Medal of Glory"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00904DragonTrophyAntharas);
        let pages = Pages::of(&Q00904DragonTrophyAntharas)
            .page("30755-01.htm", &["30755-05.htm"])
            .page("30755-05.htm", &["30755-06.htm"])
            .page("30755-06.htm", &["30755-07.html"])
            .plain(&["30755-02.html", "30755-03.html", "30755-04.html"])
            .plain(&["30755-07.html", "30755-08.html", "30755-09.html"])
            .npcs(&[THEODRIC]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let theodric = t.spawn(THEODRIC, 20);
        (t, theodric)
    }

    #[test]
    fn a_character_below_level_84_or_without_the_stone_is_sent_away() {
        let (mut t, theodric) = setup();
        t.enter(1, 83, vec![item(2, PORTAL_STONE, 1)]);
        assert!(t.quest_page(1, theodric).contains("30755-02.html"));
        t.enter(2, 84, vec![]);
        assert!(t.quest_page(2, theodric).contains("30755-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_party_kills_antharas_and_the_medals_come_once_a_day() {
        let (mut t, theodric) = setup();
        t.enter(1, 84, vec![item(2, PORTAL_STONE, 1)]);
        assert!(t.quest_page(1, theodric).contains("30755-01.htm"));
        assert!(t.event(1, NAME, "30755-05.htm").contains("30755-05.htm"));
        assert!(t.event(1, NAME, "30755-06.htm").contains("30755-06.htm"));
        assert!(t.event(1, NAME, "30755-07.html").contains("30755-07.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, theodric).contains("30755-08.html"));

        // Antharas dies: the killer's party gets cond 2 (`executeForEachPlayer` with the command
        // channel too), the killer has the party in range.
        let antharas = t.spawn(ANTHARAS, 60);
        t.kill(1, antharas);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The medals, the daily exit (`exitQuest(DAILY, true)`), `restartTime`.
        assert!(t.quest_page(1, theodric).contains("30755-09.html"));
        assert_eq!(t.count(1, MEDAL_OF_GLORY), 30);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.quest_page(1, theodric).contains("30755-03.html"));
    }
}
