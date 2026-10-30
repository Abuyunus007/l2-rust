//! Quest 907: Dragon Trophy - Valakas (Klein sends the party after Valakas, 30 medals after the kill).
// Java: dist/game/data/scripts/quests/Q00907_DragonTrophyValakas/Q00907_DragonTrophyValakas.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{QuestWrite, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KLEIN: i32 = 31540;

// Monster
const VALAKAS: i32 = 29028;

// Items
const MEDAL_OF_GLORY: i32 = 21874;
const VACUALITE_FLOATING_STONE: i32 = 7267;

// Misc
const MIN_LEVEL: i32 = 84;

/// Java `Q00907_DragonTrophyValakas`.
pub struct Q00907DragonTrophyValakas;

impl Script for Q00907DragonTrophyValakas {
    fn name(&self) -> &'static str {
        "Q00907_DragonTrophyValakas"
    }

    fn path(&self) -> &'static str {
        "quests/Q00907_DragonTrophyValakas"
    }

    fn id(&self) -> i32 {
        907
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KLEIN]);
        s.add_talk_id(&[KLEIN]);
        s.add_kill_id(&[VALAKAS]);
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
        if ctx.player_level(player) >= MIN_LEVEL && ctx.has_quest_items(player, VACUALITE_FLOATING_STONE) {
            match event {
                "31540-05.htm" | "31540-06.htm" => htmltext = Some(event.to_owned()),
                "31540-07.html" => {
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
                    "31540-02.html"
                } else if !ctx.has_quest_items(player, VACUALITE_FLOATING_STONE) {
                    "31540-04.html"
                } else {
                    "31540-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "31540-08.html".to_owned(),
                2 => {
                    ctx.give_items(player, MEDAL_OF_GLORY, 30);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                    htmltext = "31540-09.html".to_owned();
                }
                _ => {}
            },
            COMPLETED => {
                if !qs.is_now_available(ctx) {
                    htmltext = "31540-03.html".to_owned();
                } else {
                    qs.set_state(ctx, CREATED);
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "31540-02.html"
                    } else if !ctx.has_quest_items(player, VACUALITE_FLOATING_STONE) {
                        "31540-04.html"
                    } else {
                        "31540-01.htm"
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

    const NAME: &str = "Q00907_DragonTrophyValakas";

    const NPCS: &str = r#"
        <npc id="31540" type="Folk" level="80" name="Klein"/>
        <npc id="29028" type="Monster" level="80" name="Valakas"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7267" type="EtcItem" name="Vacualite Floating Stone"/>
        <item id="21874" type="EtcItem" name="Medal of Glory"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00907DragonTrophyValakas);
        let pages = Pages::of(&Q00907DragonTrophyValakas)
            .page("31540-01.htm", &["31540-05.htm"])
            .page("31540-05.htm", &["31540-06.htm"])
            .page("31540-06.htm", &["31540-07.html"])
            .plain(&["31540-02.html", "31540-03.html", "31540-04.html"])
            .plain(&["31540-07.html", "31540-08.html", "31540-09.html"])
            .npcs(&[KLEIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let klein = t.spawn(KLEIN, 20);
        (t, klein)
    }

    #[test]
    fn a_character_below_level_84_or_without_the_stone_is_sent_away() {
        let (mut t, klein) = setup();
        t.enter(1, 83, vec![item(2, VACUALITE_FLOATING_STONE, 1)]);
        assert!(t.quest_page(1, klein).contains("31540-02.html"));
        t.enter(2, 84, vec![]);
        assert!(t.quest_page(2, klein).contains("31540-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_party_kills_valakas_and_the_medals_come_once_a_day() {
        let (mut t, klein) = setup();
        t.enter(1, 84, vec![item(2, VACUALITE_FLOATING_STONE, 1)]);
        assert!(t.quest_page(1, klein).contains("31540-01.htm"));
        assert!(t.event(1, NAME, "31540-05.htm").contains("31540-05.htm"));
        assert!(t.event(1, NAME, "31540-06.htm").contains("31540-06.htm"));
        assert!(t.event(1, NAME, "31540-07.html").contains("31540-07.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, klein).contains("31540-08.html"));

        // Valakas dies: the killer's party gets cond 2 (`executeForEachPlayer` with the command
        // channel too), the killer has the party in range.
        let valakas = t.spawn(VALAKAS, 60);
        t.kill(1, valakas);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The medals, the daily exit (`exitQuest(DAILY, true)`), `restartTime`.
        assert!(t.quest_page(1, klein).contains("31540-09.html"));
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
        assert!(t.quest_page(1, klein).contains("31540-03.html"));
    }
}
