//! Quest 906: The Call of Valakas (Klein gives the Valakas scroll after the Lavasaurus Alpha kill a day).
// Java: dist/game/data/scripts/quests/Q00906_TheCallOfValakas/Q00906_TheCallOfValakas.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KLEIN: i32 = 31540;

// Monster
const LAVASAURUS_ALPHA: i32 = 29029;

// Items
const LAVASAURUS_ALPHA_FRAGMENT: i32 = 21993;
const SCROLL_VALAKAS_CALL: i32 = 21895;
const VACUALITE_FLOATING_STONE: i32 = 7267;

// Misc
const MIN_LEVEL: i32 = 83;

/// Java `Q00906_TheCallOfValakas`.
pub struct Q00906TheCallOfValakas;

impl Script for Q00906TheCallOfValakas {
    fn name(&self) -> &'static str {
        "Q00906_TheCallOfValakas"
    }

    fn path(&self) -> &'static str {
        "quests/Q00906_TheCallOfValakas"
    }

    fn id(&self) -> i32 {
        906
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KLEIN]);
        s.add_talk_id(&[KLEIN]);
        s.add_kill_id(&[LAVASAURUS_ALPHA]);
        s.register_quest_items(&[LAVASAURUS_ALPHA_FRAGMENT]);
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
                "31540-05.htm" => htmltext = Some(event.to_owned()),
                "31540-06.html" => {
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
            ctx.give_items(player, LAVASAURUS_ALPHA_FRAGMENT, 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            qs.set_cond(ctx, 2, true);
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
                    "31540-03.html"
                } else if !ctx.has_quest_items(player, VACUALITE_FLOATING_STONE) {
                    "31540-04.html"
                } else {
                    "31540-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "31540-07.html".to_owned(),
                2 => {
                    ctx.give_items(player, SCROLL_VALAKAS_CALL, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                    htmltext = "31540-08.html".to_owned();
                }
                _ => {}
            },
            COMPLETED => {
                if !qs.is_now_available(ctx) {
                    htmltext = "31540-02.html".to_owned();
                } else {
                    qs.set_state(ctx, CREATED);
                    htmltext = if ctx.player_level(player) < MIN_LEVEL {
                        "31540-03.html"
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
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00906_TheCallOfValakas";

    const NPCS: &str = r#"
        <npc id="31540" type="Folk" level="80" name="Klein"/>
        <npc id="29029" type="Monster" level="80" name="Lavasaurus Alpha"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7267" type="EtcItem" name="Vacualite Floating Stone"/>
        <item id="21993" type="EtcItem" name="Lavasaurus Alpha Fragment"><set name="is_questitem" val="true"/></item>
        <item id="21895" type="EtcItem" name="Scroll: Call of Valakas"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00906TheCallOfValakas);
        let pages = Pages::of(&Q00906TheCallOfValakas)
            .page("31540-01.htm", &["31540-05.htm"])
            .page("31540-05.htm", &["31540-06.html"])
            .plain(&["31540-06.html", "31540-07.html", "31540-08.html"])
            .plain(&["31540-02.html", "31540-03.html", "31540-04.html"])
            .npcs(&[KLEIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let klein = t.spawn(KLEIN, 20);
        (t, klein)
    }

    #[test]
    fn a_character_below_level_83_or_without_the_stone_is_sent_away() {
        let (mut t, klein) = setup();
        t.enter(1, 82, vec![item(2, VACUALITE_FLOATING_STONE, 1)]);
        assert!(t.quest_page(1, klein).contains("31540-03.html"));
        t.enter(2, 83, vec![]);
        assert!(t.quest_page(2, klein).contains("31540-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_lavasaurus_alpha_fragment_sets_cond_2_then_the_scroll_once_a_day() {
        let (mut t, klein) = setup();
        t.enter(1, 83, vec![item(2, VACUALITE_FLOATING_STONE, 1)]);
        assert!(t.quest_page(1, klein).contains("31540-01.htm"));
        assert!(t.event(1, NAME, "31540-05.htm").contains("31540-05.htm"));
        assert!(t.event(1, NAME, "31540-06.html").contains("31540-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );
        assert!(t.quest_page(1, klein).contains("31540-07.html"));

        // A kill in range gives the fragment and `setCond(2, true)`.
        let alpha = t.spawn(LAVASAURUS_ALPHA, 60);
        t.kill(1, alpha);
        assert_eq!(t.count(1, LAVASAURUS_ALPHA_FRAGMENT), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The scroll; the daily exit with `restartTime`.
        assert!(t.quest_page(1, klein).contains("31540-08.html"));
        assert_eq!(t.count(1, SCROLL_VALAKAS_CALL), 1);
        assert_eq!(t.count(1, LAVASAURUS_ALPHA_FRAGMENT), 0);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(matches!(&writes[2], QuestWrite::InsertVar { var, .. } if var == "restartTime"));
        assert!(t.quest_page(1, klein).contains("31540-02.html"));
    }
}
