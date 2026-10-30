//! Quest 691: Matras' Suspicious Request (Matras at the Steel Citadel, the guards of Bate Tower).
// Java: dist/game/data/scripts/quests/Q00691_MatrasSuspiciousRequest/Q00691_MatrasSuspiciousRequest.java
//
// Pattern: a repeatable quest whose progress lives in a quest variable (`submitted_gems`): the
// Red Stones of the tower guards are handed in, counted into the variable and paid per stone
// (or 744 stones for the Dynasty Soul). The pages show the count with `%itemcount%`
// (`get_htm` + `replace`, Java `getHtm(..).replace(..)`).

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MATRAS: i32 = 32245;

// Items
const RED_GEM: i32 = 10372;
const DYNASTY_SOUL_II: i32 = 10413;

/// Java `REWARD_CHANCES`: the tower guard and its Red Stone chance (per mille).
const REWARD_CHANCES: [(i32, i32); 10] = [
    (22363, 890),
    (22364, 261),
    (22365, 560),
    (22366, 560),
    (22367, 190),
    (22368, 129),
    (22369, 210),
    (22370, 787),
    (22371, 257),
    (22372, 656),
];

// Misc
const MIN_LEVEL: i32 = 76;

/// Java `Q00691_MatrasSuspiciousRequest`.
pub struct Q00691MatrasSuspiciousRequest;

impl Script for Q00691MatrasSuspiciousRequest {
    fn name(&self) -> &'static str {
        "Q00691_MatrasSuspiciousRequest"
    }

    fn path(&self) -> &'static str {
        "quests/Q00691_MatrasSuspiciousRequest"
    }

    fn id(&self) -> i32 {
        691
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MATRAS]);
        s.add_talk_id(&[MATRAS]);
        s.add_kill_id(&REWARD_CHANCES.map(|(id, _)| id));
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
        match event {
            "32245-02.htm" | "32245-11.html" => htmltext = Some(event.to_owned()),
            "32245-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "take_reward" => {
                if qs.is_started(ctx) {
                    let gems_count = qs.get_int(ctx, "submitted_gems");
                    if gems_count >= 744 {
                        qs.set(ctx, "submitted_gems", &(gems_count - 744).to_string());
                        ctx.give_items(player, DYNASTY_SOUL_II, 1);
                        htmltext = Some("32245-09.html".to_owned());
                    } else {
                        let submitted = qs.get(ctx, "submitted_gems").unwrap_or_default();
                        htmltext = ctx
                            .get_htm(player, "32245-10.html")
                            .map(|page| page.replace("%itemcount%", &submitted));
                    }
                }
            }
            "32245-08.html" => {
                if qs.is_started(ctx) {
                    let submitted_count = qs.get_int(ctx, "submitted_gems");
                    let brought_count = ctx.get_quest_items_count(player, RED_GEM);
                    let final_count = i64::from(submitted_count) + brought_count;
                    ctx.take_items(player, RED_GEM, brought_count);
                    qs.set(ctx, "submitted_gems", &final_count.to_string());
                    htmltext = ctx
                        .get_htm(player, "32245-08.html")
                        .map(|page| page.replace("%itemcount%", &final_count.to_string()));
                }
            }
            "32245-12.html" => {
                if qs.is_started(ctx) {
                    let submitted = i64::from(qs.get_int(ctx, "submitted_gems"));
                    ctx.give_adena(player, submitted * 10_000, true);
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_random_party_member_cond(killer, 1).is_none() {
            return;
        }
        let Some((_, chance)) = REWARD_CHANCES.iter().find(|(id, _)| *id == npc.id) else {
            return;
        };
        // Java: `numItems = max(chance / 1000, 1)` and `chance %= 1000`; every chance is below
        // 1000, so one stone at most, and the roll is `<=`.
        let num_items = i64::from((chance / 1000).max(1));
        let chance = chance % 1000;
        if ctx.get_random(1000) <= chance {
            // NOTE(java-quirk): the stone goes to the killer, not to the member the random pick
            // returned (Java passes `player`, not `pl`).
            ctx.give_items(killer, RED_GEM, num_items);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = Some(ctx.get_no_quest_msg(player));
        match qs.state(ctx) {
            CREATED => {
                htmltext = Some(
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "32245-01.htm"
                    } else {
                        "32245-03.html"
                    }
                    .to_owned(),
                );
            }
            STARTED => {
                if ctx.has_quest_items(player, RED_GEM) {
                    htmltext = Some("32245-05.html".to_owned());
                } else if qs.get_int(ctx, "submitted_gems") > 0 {
                    let submitted = qs.get(ctx, "submitted_gems").unwrap_or_default();
                    htmltext = ctx
                        .get_htm(player, "32245-07.html")
                        .map(|page| page.replace("%itemcount%", &submitted));
                } else {
                    htmltext = Some("32245-06.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00691_MatrasSuspiciousRequest";
    const DIR: &str = "data/scripts/quests/Q00691_MatrasSuspiciousRequest";

    const NPCS: &str = r#"
        <npc id="32245" type="Folk" level="1" name="Matras"/>
        <npc id="22363" type="Monster" level="80" name="Tower Guard"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10372" type="EtcItem" name="Red Stone"><set name="is_stackable" val="true"/></item>
        <item id="10413" type="EtcItem" name="Dynasty Soul II"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00691MatrasSuspiciousRequest);
        // The counting pages hold `%itemcount%` and the links of the real page.
        let counting = |head: &str| {
            format!(
                r#"<html><body>Matras:<br>{head} %itemcount% Red Stones.<br>
<a action="bypass -h Script {NAME} take_reward">take</a><br>
<a action="bypass Script {NAME} 32245-11.html">more</a><br>
<a action="bypass Script {NAME} 32245-12.html">quit</a></body></html>"#
            )
        };
        let pages = Pages::of(&Q00691MatrasSuspiciousRequest)
            .page("32245-01.htm", &["32245-02.htm"])
            .page("32245-02.htm", &["32245-04.htm"])
            .plain(&["32245-03.html", "32245-04.htm", "32245-06.html", "32245-09.html"])
            .page("32245-05.html", &["32245-08.html"])
            .file(&format!("{DIR}/32245-07.html"), &counting("You have brought me"))
            .file(&format!("{DIR}/32245-08.html"), &counting("You have brought"))
            .file(
                &format!("{DIR}/32245-10.html"),
                "<html><body>Matras:<br>You've given me only %itemcount% so far.</body></html>",
            )
            .plain(&["32245-11.html", "32245-12.html"])
            .npcs(&[MATRAS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let matras = t.spawn(MATRAS, 20);
        (t, matras)
    }

    fn give_stones(t: &mut ScriptTest, count: i64) {
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), RED_GEM, count);
        });
    }

    #[test]
    fn a_level_75_character_is_refused() {
        let (mut t, matras) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, matras).contains("32245-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_level_76_character_starts_the_quest() {
        let (mut t, matras) = setup();
        t.enter(1, 76, vec![]);
        assert!(t.quest_page(1, matras).contains("32245-01.htm"));
        t.event(1, NAME, "32245-02.htm");
        assert!(t.event(1, NAME, "32245-04.htm").contains("32245-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    #[test]
    fn the_guards_drop_stones_and_the_hand_in_counts_them() {
        let (mut t, matras) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, matras);
        t.event(1, NAME, "32245-02.htm");
        t.event(1, NAME, "32245-04.htm");
        t.take_writes();

        // Nothing to hand in yet: 06.
        assert!(t.quest_page(1, matras).contains("32245-06.html"));

        let mut kills = 0;
        while t.count(1, RED_GEM) == 0 {
            let guard = t.spawn(22363, 60);
            t.kill(1, guard);
            kills += 1;
            assert!(kills < 50, "the guards never dropped a stone");
        }
        let brought = t.count(1, RED_GEM);
        assert!(t.quest_page(1, matras).contains("32245-05.html"));

        // The hand-in moves the stones into the quest variable and shows the count.
        let page = t.event(1, NAME, "32245-08.html");
        assert!(
            page.contains(&format!("You have brought {brought} Red Stones")),
            "{page}"
        );
        assert_eq!(t.count(1, RED_GEM), 0);
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "submitted_gems", &brought.to_string())]
        );

        // Less than 744: the reward page shows the count and gives nothing.
        let page = t.event(1, NAME, "take_reward");
        assert!(page.contains(&format!("only {brought} so far")), "{page}");
        assert_eq!(t.count(1, DYNASTY_SOUL_II), 0);
    }

    #[test]
    fn seven_hundred_and_forty_four_stones_get_the_soul_and_the_rest_is_paid_on_quit() {
        let (mut t, matras) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, matras);
        t.event(1, NAME, "32245-02.htm");
        t.event(1, NAME, "32245-04.htm");
        t.take_writes();

        give_stones(&mut t, 744);
        assert!(t.quest_page(1, matras).contains("32245-05.html"));
        assert!(
            t.event(1, NAME, "32245-08.html")
                .contains("You have brought 744 Red Stones")
        );
        assert_eq!(t.count(1, RED_GEM), 0);

        // 744 gives the soul and takes the 744 from the variable.
        assert!(t.event(1, NAME, "take_reward").contains("32245-09.html"));
        assert_eq!(t.count(1, DYNASTY_SOUL_II), 1);

        // Three more stones, then "I've had enough": 3 x 10 000 adena and the end.
        give_stones(&mut t, 3);
        assert!(t.quest_page(1, matras).contains("32245-05.html"));
        assert!(
            t.event(1, NAME, "32245-08.html")
                .contains("You have brought 3 Red Stones")
        );
        assert!(t.event(1, NAME, "32245-12.html").contains("32245-12.html"));
        assert_eq!(t.count(1, 57), 30_000);
        assert_eq!(t.take_writes().last(), Some(&rows::delete(NAME, true)));
    }
}
