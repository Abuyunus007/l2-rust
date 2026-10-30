//! Quest 551: Olympiad Starter (the Grand Olympiad Manager rewards 3, 5 and 10 games, daily).
// Java: dist/game/data/scripts/quests/Q00551_OlympiadStarter/Q00551_OlympiadStarter.java
//
// `addOlympiadMatchFinishId()` is the global `ON_OLYMPIAD_MATCH_RESULT` listener. Java's
// `onOlympiadLose` is never called by anything, so it is not ported.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, QuestType};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const MANAGER: i32 = 31688;

const CERT_3: i32 = 17238;
const CERT_5: i32 = 17239;
const CERT_10: i32 = 17240;

const OLY_CHEST: i32 = 17169;
const MEDAL_OF_GLORY: i32 = 21874;

/// Java `Q00551_OlympiadStarter`.
pub struct Q00551OlympiadStarter;

/// The body shared by the winner and the loser branch of `onOlympiadMatchFinish`.
fn count_match(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let Some(qs) = ctx.get_quest_state(player, false) else {
        return;
    };
    if qs.is_started(ctx) && qs.is_memo_state(ctx, 1) {
        let memo_state_ex = qs.get_memo_state_ex(ctx, 1);
        if memo_state_ex == 9 {
            qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + 1);
            qs.set_memo_state(ctx, 2);
            qs.set_cond(ctx, 2, true);
            ctx.give_items(player, CERT_10, 1);
        } else if memo_state_ex < 9 {
            if qs.is_memo_state_ex(ctx, 1, 2) {
                ctx.give_items(player, CERT_3, 1);
            } else if qs.is_memo_state_ex(ctx, 1, 4) {
                ctx.give_items(player, CERT_5, 1);
            }
            qs.set_memo_state_ex(ctx, 1, qs.get_memo_state_ex(ctx, 1) + 1);
            ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
        }
    }
}

impl Script for Q00551OlympiadStarter {
    fn name(&self) -> &'static str {
        "Q00551_OlympiadStarter"
    }

    fn path(&self) -> &'static str {
        "quests/Q00551_OlympiadStarter"
    }

    fn id(&self) -> i32 {
        551
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MANAGER]);
        s.add_talk_id(&[MANAGER]);
        s.register_quest_items(&[CERT_3, CERT_5, CERT_10]);
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
        let htmltext = Some(event.to_owned());
        match event {
            "31688-03.html" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                qs.set_memo_state_ex(ctx, 1, 0);
            }
            "31688-04.html" => {
                if ctx.get_quest_items_count(player, CERT_3) + ctx.get_quest_items_count(player, CERT_5) > 0 {
                    if ctx.has_quest_items(player, CERT_3) {
                        ctx.give_items(player, OLY_CHEST, 1);
                        ctx.take_items(player, CERT_3, -1);
                    }
                    if ctx.has_quest_items(player, CERT_5) {
                        ctx.give_items(player, OLY_CHEST, 1);
                        ctx.give_items(player, MEDAL_OF_GLORY, 3);
                        ctx.take_items(player, CERT_5, -1);
                    }
                    qs.exit_quest_type(ctx, QuestType::Daily, true);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_olympiad_match_result(
        &self,
        ctx: &mut ScriptCtx<'_>,
        winner: Option<i32>,
        loser: i32,
        _competition_type: i32,
    ) {
        // NOTE(java-quirk): an offline winner ends the method, so the loser is not counted then.
        if let Some(winner) = winner {
            let player = ObjectId(winner);
            if ctx.world().player(player).is_none() {
                return;
            }
            count_match(ctx, player);
        }
        let player = ObjectId(loser);
        if ctx.world().player(player).is_none() {
            return;
        }
        count_match(ctx, player);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
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
        } else if qs.is_started(ctx) {
            if qs.is_memo_state(ctx, 1) {
                let any = ctx.get_quest_items_count(player, CERT_3)
                    + ctx.get_quest_items_count(player, CERT_5)
                    + ctx.get_quest_items_count(player, CERT_10)
                    > 0;
                htmltext = if any { "31688-07.html" } else { "31688-06.html" }.to_owned();
            } else if qs.is_memo_state(ctx, 2) {
                ctx.give_items(player, OLY_CHEST, 4);
                ctx.give_items(player, MEDAL_OF_GLORY, 5);
                qs.exit_quest_type(ctx, QuestType::Daily, true);
                htmltext = "31688-04.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00551_OlympiadStarter";

    const NPCS: &str = r#"<npc id="31688" type="Folk" level="70" name="Manager"/>"#;

    const ITEMS: &str = r#"
        <item id="17169" type="EtcItem" name="Olympiad Chest"><set name="is_stackable" val="true"/></item>
        <item id="21874" type="EtcItem" name="Medal of Glory"><set name="is_stackable" val="true"/></item>
        <item id="17238" type="EtcItem" name="Cert 3"><set name="is_questitem" val="true"/></item>
        <item id="17239" type="EtcItem" name="Cert 5"><set name="is_questitem" val="true"/></item>
        <item id="17240" type="EtcItem" name="Cert 10"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00551OlympiadStarter);
        let pages = Pages::of(&Q00551OlympiadStarter)
            .page("31688-01.htm", &["31688-03.html", "31688-02.htm"])
            .page("31688-02.htm", &["31688-03.html"])
            .page("31688-07.html", &["31688-04.html"])
            .plain(&["31688-00.htm", "31688-03.html", "31688-05.html", "31688-06.html"])
            .plain(&["31688-04.html"])
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
            Q00551OlympiadStarter.on_olympiad_match_result(ctx, winner, loser, 0);
        });
    }

    #[test]
    fn only_nobles_of_level_75_start() {
        let (mut t, manager) = setup();
        t.enter(1, 80, vec![]);
        assert!(t.quest_page(1, manager).contains("31688-00.htm"));
        t.enter(2, 74, vec![]);
        t.world.set_noble(Oid(2), true);
        let pg = t.quest_page(2, manager);
        assert!(pg.contains("31688-00.htm"), "{pg}");
        t.enter(3, 80, vec![]);
        t.world.set_noble(Oid(3), true);
        assert!(t.quest_page(3, manager).contains("31688-01.htm"));
    }

    #[test]
    fn ten_matches_then_the_final_reward() {
        let (mut t, manager) = setup();
        t.enter(1, 80, vec![]);
        t.world.set_noble(Oid(1), true);
        assert!(t.quest_page(1, manager).contains("31688-01.htm"));
        assert!(t.event(1, NAME, "31688-03.html").contains("31688-03.html"));
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::insert(NAME, "cond", "1"));
        assert!(t.quest_page(1, manager).contains("31688-06.html"));

        for _ in 0..10 {
            finish(&mut t, None, 1);
        }
        assert_eq!(t.count(1, CERT_3), 1);
        assert_eq!(t.count(1, CERT_5), 1);
        assert_eq!(t.count(1, CERT_10), 1);
        t.take_writes();
        assert!(t.quest_page(1, manager).contains("31688-04.html"));
        assert_eq!((t.count(1, OLY_CHEST), t.count(1, MEDAL_OF_GLORY)), (4, 5));
        let writes = t.take_writes();
        assert_eq!(writes[0], rows::delete(NAME, false));
        let pg = t.quest_page(1, manager);
        assert!(pg.contains("31688-05.html"), "{pg}");
    }

    #[test]
    fn early_exit_with_the_certificates() {
        let (mut t, manager) = setup();
        t.enter(1, 80, vec![item(100, CERT_3, 1), item(101, CERT_5, 1)]);
        t.world.set_noble(Oid(1), true);
        t.quest_page(1, manager);
        t.event(1, NAME, "31688-03.html");
        t.take_writes();
        assert!(t.quest_page(1, manager).contains("31688-07.html"));
        assert!(t.event(1, NAME, "31688-04.html").contains("31688-04.html"));
        assert_eq!((t.count(1, OLY_CHEST), t.count(1, MEDAL_OF_GLORY)), (2, 3));
        assert_eq!((t.count(1, CERT_3), t.count(1, CERT_5)), (0, 0));
    }
}
