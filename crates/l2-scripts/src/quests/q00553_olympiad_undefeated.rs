//! Quest 553: Olympiad Undefeated (ten Olympiad wins in a row, daily).
// Java: dist/game/data/scripts/quests/Q00553_OlympiadUndefeated/Q00553_OlympiadUndefeated.java

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestType};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MANAGER: i32 = 31688;

// Items
const WIN_CONF_2: i32 = 17244;
const WIN_CONF_5: i32 = 17245;
const WIN_CONF_10: i32 = 17246;
const OLY_CHEST: i32 = 17169;
const MEDAL_OF_GLORY: i32 = 21874;

/// Java `Q00553_OlympiadUndefeated`.
pub struct Q00553OlympiadUndefeated;

impl Script for Q00553OlympiadUndefeated {
    fn name(&self) -> &'static str {
        "Q00553_OlympiadUndefeated"
    }

    fn path(&self) -> &'static str {
        "quests/Q00553_OlympiadUndefeated"
    }

    fn id(&self) -> i32 {
        553
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MANAGER]);
        s.add_talk_id(&[MANAGER]);
        s.register_quest_items(&[WIN_CONF_2, WIN_CONF_5, WIN_CONF_10]);
        s.set_on_olympiad_match_result();
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
        let mut htmltext = event.to_owned();
        if event.eq_ignore_ascii_case("31688-03.html") {
            qs.start_quest(ctx);
        } else if event.eq_ignore_ascii_case("31688-04.html") {
            let count =
                ctx.get_quest_items_count(player, WIN_CONF_2) + ctx.get_quest_items_count(player, WIN_CONF_5);
            if count > 0 {
                ctx.give_items(player, OLY_CHEST, count);
                if count == 2 {
                    ctx.give_items(player, MEDAL_OF_GLORY, 3);
                }
                qs.exit_quest_type(ctx, QuestType::Daily, true);
            } else {
                htmltext = ctx.get_no_quest_msg(player);
            }
        }
        Some(htmltext)
    }

    fn on_olympiad_match_result(
        &self,
        ctx: &mut ScriptCtx<'_>,
        winner: Option<i32>,
        loser: i32,
        _competition_type: i32,
    ) {
        // NOTE(java-quirk): an offline winner ends the method, so the loser is not handled then.
        if let Some(winner) = winner {
            let player = ObjectId(winner);
            if ctx.world().player(player).is_none() {
                return;
            }
            if let Some(qs) = ctx.get_quest_state(player, false)
                && qs.is_started(ctx)
                && qs.is_cond(ctx, 1)
            {
                let matches = qs.get_int(ctx, "undefeatable") + 1;
                qs.set(ctx, "undefeatable", &matches.to_string());
                match matches {
                    2 => {
                        if !ctx.has_quest_items(player, WIN_CONF_2) {
                            ctx.give_items(player, WIN_CONF_2, 1);
                        }
                    }
                    5 => {
                        if !ctx.has_quest_items(player, WIN_CONF_5) {
                            ctx.give_items(player, WIN_CONF_5, 1);
                        }
                    }
                    10 => {
                        if !ctx.has_quest_items(player, WIN_CONF_10) {
                            ctx.give_items(player, WIN_CONF_10, 1);
                            qs.set_cond(ctx, 2, false);
                        }
                    }
                    _ => {}
                }
            }
        }

        let player = ObjectId(loser);
        if ctx.world().player(player).is_none() {
            return;
        }
        if let Some(qs) = ctx.get_quest_state(player, false)
            && qs.is_started(ctx)
            && qs.is_cond(ctx, 1)
        {
            qs.unset(ctx, "undefeatable");
            ctx.take_items(player, WIN_CONF_2, -1);
            ctx.take_items(player, WIN_CONF_5, -1);
            ctx.take_items(player, WIN_CONF_10, -1);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let htmltext;
        if ctx.player_level(player) < 75 || !ctx.world().is_noble(player) {
            htmltext = "31688-00.htm".to_owned();
        } else if qs.is_created(ctx) {
            htmltext = "31688-01.htm".to_owned();
        } else if qs.is_completed(ctx) {
            if qs.is_now_available(ctx) {
                qs.set_state(ctx, CREATED);
                // The gate was passed above, so this is always the first page.
                htmltext = "31688-01.htm".to_owned();
            } else {
                htmltext = "31688-05.html".to_owned();
            }
        } else {
            let count = ctx.get_quest_items_count(player, WIN_CONF_2)
                + ctx.get_quest_items_count(player, WIN_CONF_5)
                + ctx.get_quest_items_count(player, WIN_CONF_10);
            if count == 3 && qs.is_cond(ctx, 2) {
                ctx.give_items(player, OLY_CHEST, 4);
                ctx.give_items(player, MEDAL_OF_GLORY, 5);
                qs.exit_quest_type(ctx, QuestType::Daily, true);
                htmltext = "31688-04.html".to_owned();
            } else {
                htmltext = format!("31688-w{count}.html");
            }
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

    const NAME: &str = "Q00553_OlympiadUndefeated";

    const NPCS: &str = r#"<npc id="31688" type="Folk" level="70" name="Manager"/>"#;

    const ITEMS: &str = r#"
        <item id="17169" type="EtcItem" name="Olympiad Chest"><set name="is_stackable" val="true"/></item>
        <item id="21874" type="EtcItem" name="Medal of Glory"><set name="is_stackable" val="true"/></item>
        <item id="17244" type="EtcItem" name="Conf 2"><set name="is_questitem" val="true"/></item>
        <item id="17245" type="EtcItem" name="Conf 5"><set name="is_questitem" val="true"/></item>
        <item id="17246" type="EtcItem" name="Conf 10"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00553OlympiadUndefeated);
        let pages = Pages::of(&Q00553OlympiadUndefeated)
            .page("31688-01.htm", &["31688-03.html", "31688-02.htm"])
            .page("31688-02.htm", &["31688-03.html"])
            .page("31688-w1.html", &["31688-04.html"])
            .page("31688-w2.html", &["31688-04.html"])
            .plain(&[
                "31688-00.htm",
                "31688-03.html",
                "31688-04.html",
                "31688-05.html",
                "31688-w0.html",
            ])
            .file(
                "data/html/olympiad/noble_main.htm",
                r#"<html><body>noble_main <a action="bypass Script">Quest</a></body></html>"#,
            )
            .npcs(&[MANAGER]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let manager = t.spawn(MANAGER, 20);
        (t, manager)
    }

    fn finish(t: &mut ScriptTest, winner: Option<i32>, loser: i32) {
        t.world.run_as_script(NAME, |ctx| {
            Q00553OlympiadUndefeated.on_olympiad_match_result(ctx, winner, loser, 0);
        });
    }

    fn noble(t: &mut ScriptTest, id: i32, level: i32) {
        t.enter(id, level, vec![]);
        t.world.set_noble(Oid(id), true);
    }

    #[test]
    fn only_nobles_of_level_75_start() {
        let (mut t, manager) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, manager).contains("31688-00.htm"));
        noble(&mut t, 2, 74);
        assert!(t.quest_page(2, manager).contains("31688-00.htm"));
        noble(&mut t, 3, 75);
        assert!(t.quest_page(3, manager).contains("31688-01.htm"));
    }

    #[test]
    fn ten_wins_in_a_row_then_the_reward() {
        let (mut t, manager) = setup();
        noble(&mut t, 1, 80);
        noble(&mut t, 2, 80);
        assert!(t.quest_page(1, manager).contains("31688-01.htm"));
        assert!(t.event(1, NAME, "31688-03.html").contains("31688-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, manager).contains("31688-w0.html"));

        for _ in 0..2 {
            finish(&mut t, Some(1), 2);
        }
        assert_eq!(t.count(1, WIN_CONF_2), 1);
        assert!(t.quest_page(1, manager).contains("31688-w1.html"));
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::insert(NAME, "undefeatable", "1"));
        assert_eq!(writes[1], rows::update(NAME, "undefeatable", "2"));
        for _ in 0..8 {
            finish(&mut t, Some(1), 2);
        }
        assert_eq!((t.count(1, WIN_CONF_5), t.count(1, WIN_CONF_10)), (1, 1));
        assert_eq!(t.take_writes().last(), Some(&rows::update(NAME, "cond", "2")));

        assert!(t.quest_page(1, manager).contains("31688-04.html"));
        assert_eq!((t.count(1, OLY_CHEST), t.count(1, MEDAL_OF_GLORY)), (4, 5));
        assert_eq!(t.take_writes()[0], rows::delete(NAME, false));
        assert!(t.quest_page(1, manager).contains("31688-05.html"));
    }

    #[test]
    fn a_defeat_resets_the_streak() {
        let (mut t, manager) = setup();
        noble(&mut t, 1, 80);
        noble(&mut t, 2, 80);
        t.quest_page(1, manager);
        t.event(1, NAME, "31688-03.html");
        finish(&mut t, Some(1), 2);
        finish(&mut t, Some(1), 2);
        assert_eq!(t.count(1, WIN_CONF_2), 1);
        t.take_writes();
        finish(&mut t, Some(2), 1);
        assert_eq!(t.count(1, WIN_CONF_2), 0);
        assert_eq!(t.take_writes(), [rows::delete_var(NAME, "undefeatable")]);
        // The incomplete reward with two wins: one chest per confirmation and the medals at two.
        finish(&mut t, Some(1), 2);
        finish(&mut t, Some(1), 2);
        assert!(t.quest_page(1, manager).contains("31688-w1.html"));
        assert!(t.event(1, NAME, "31688-04.html").contains("31688-04.html"));
        assert_eq!((t.count(1, OLY_CHEST), t.count(1, MEDAL_OF_GLORY)), (1, 0));
    }
}
