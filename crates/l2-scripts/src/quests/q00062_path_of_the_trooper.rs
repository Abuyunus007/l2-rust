//! Quest 62: Path of the Trooper (Master Gwain and Master Shubain, the Kamael soldiers' masters).
// Java: dist/game/data/scripts/quests/Q00062_PathOfTheTrooper/Q00062_PathOfTheTrooper.java
//
// Pattern: a kill chain with the memo state as a step counter (`docs/design/scripts.md`, "Quest
// patterns"): five felim heads, ten venomous spider legs and a tumran heart, each killed by the
// killer while the cond fits, then the recommendation and the reward.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MASTER_SHUBAIN: i32 = 32194;
const MASTER_GWAIN: i32 = 32197;

// Items
const FELIM_LIZARDMAN_HEAD: i32 = 9749;
const VENOMOUS_SPIDERS_LEG: i32 = 9750;
const TUMRAN_BUGBEAR_HEART: i32 = 9751;
const SHUBAINS_RECOMMENDATION: i32 = 9752;

// Reward
const GWAINS_RECOMMENDATION: i32 = 9753;

// Monsters
const FELIM_LIZARDMAN_WARRIOR: i32 = 20014;
const VENOMOUS_SPIDER: i32 = 20038;
const TUMRAN_BUGBEAR: i32 = 20062;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00062_PathOfTheTrooper`.
pub struct Q00062PathOfTheTrooper;

impl Script for Q00062PathOfTheTrooper {
    fn name(&self) -> &'static str {
        "Q00062_PathOfTheTrooper"
    }

    fn path(&self) -> &'static str {
        "quests/Q00062_PathOfTheTrooper"
    }

    fn id(&self) -> i32 {
        62
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_GWAIN]);
        s.add_talk_id(&[MASTER_GWAIN, MASTER_SHUBAIN]);
        s.add_kill_id(&[FELIM_LIZARDMAN_WARRIOR, VENOMOUS_SPIDER, TUMRAN_BUGBEAR]);
        s.register_quest_items(&[
            FELIM_LIZARDMAN_HEAD,
            VENOMOUS_SPIDERS_LEG,
            TUMRAN_BUGBEAR_HEART,
            SHUBAINS_RECOMMENDATION,
        ]);
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
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    htmltext = Some("32197-06.htm".to_owned());
                }
            }
            "32194-02.html" => {
                if qs.is_cond(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }
        match npc.id {
            FELIM_LIZARDMAN_WARRIOR => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(killer, FELIM_LIZARDMAN_HEAD) < 5 {
                    ctx.give_items(killer, FELIM_LIZARDMAN_HEAD, 1);
                    if ctx.get_quest_items_count(killer, FELIM_LIZARDMAN_HEAD) == 5 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            VENOMOUS_SPIDER => {
                if qs.is_cond(ctx, 3) && ctx.get_quest_items_count(killer, VENOMOUS_SPIDERS_LEG) < 10 {
                    ctx.give_items(killer, VENOMOUS_SPIDERS_LEG, 1);
                    if ctx.get_quest_items_count(killer, VENOMOUS_SPIDERS_LEG) == 10 {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            TUMRAN_BUGBEAR => {
                if qs.is_cond(ctx, 5)
                    && !ctx.has_quest_items(killer, TUMRAN_BUGBEAR_HEART)
                    && ctx.get_random(1000) < 500
                {
                    ctx.give_items(killer, TUMRAN_BUGBEAR_HEART, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.state(ctx) == CREATED {
            if npc.id == MASTER_GWAIN {
                if ctx.player_race(player) == race::KAMAEL {
                    if ctx.player_class_id(player) == player_class::MALE_SOLDIER {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "32197-01.htm"
                        } else {
                            "32197-02.html"
                        }
                        .to_owned();
                    } else {
                        htmltext = "32197-03.html".to_owned();
                    }
                } else {
                    htmltext = "32197-04.html".to_owned();
                }
            }
        } else if qs.state(ctx) == STARTED {
            match npc.id {
                MASTER_GWAIN => match qs.get_cond(ctx) {
                    1..=3 => htmltext = "32197-07.html".to_owned(),
                    4 => {
                        ctx.take_items(player, SHUBAINS_RECOMMENDATION, 1);
                        qs.set_memo_state(ctx, 5);
                        qs.set_cond(ctx, 5, true);
                        htmltext = "32197-08.html".to_owned();
                    }
                    5 => {
                        if !ctx.has_quest_items(player, TUMRAN_BUGBEAR_HEART) {
                            htmltext = "32197-09.html".to_owned();
                        } else {
                            ctx.give_adena(player, 163_800, true);
                            ctx.take_items(player, TUMRAN_BUGBEAR_HEART, 1);
                            ctx.give_items(player, GWAINS_RECOMMENDATION, 1);
                            let level = ctx.player_level(player);
                            if level >= 20 {
                                ctx.add_exp_and_sp(player, 320_534, 20_848);
                            } else if level == 19 {
                                ctx.add_exp_and_sp(player, 456_128, 27_546);
                            } else {
                                ctx.add_exp_and_sp(player, 591_724, 34_244);
                            }
                            qs.exit_quest(ctx, false, true);
                            // NOTE(java-quirk): Java also sends the player `SocialAction(3)` (the
                            // bow). FOLLOW-UP(engine): no script helper sends a social action to
                            // one player yet.
                            htmltext = "32197-10.html".to_owned();
                        }
                    }
                    _ => {}
                },
                MASTER_SHUBAIN => match qs.get_cond(ctx) {
                    1 => htmltext = "32194-01.html".to_owned(),
                    2 => {
                        if ctx.get_quest_items_count(player, FELIM_LIZARDMAN_HEAD) < 5 {
                            htmltext = "32194-03.html".to_owned();
                        } else {
                            ctx.take_items(player, FELIM_LIZARDMAN_HEAD, -1);
                            qs.set_memo_state(ctx, 3);
                            qs.set_cond(ctx, 3, true);
                            htmltext = "32194-04.html".to_owned();
                        }
                    }
                    3 => {
                        if ctx.get_quest_items_count(player, VENOMOUS_SPIDERS_LEG) < 10 {
                            htmltext = "32194-05.html".to_owned();
                        } else {
                            ctx.take_items(player, VENOMOUS_SPIDERS_LEG, -1);
                            ctx.give_items(player, SHUBAINS_RECOMMENDATION, 1);
                            qs.set_memo_state(ctx, 4);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "32194-06.html".to_owned();
                        }
                    }
                    4 => htmltext = "32194-07.html".to_owned(),
                    _ => {}
                },
                _ => {}
            }
        } else if qs.state(ctx) == COMPLETED && npc.id == MASTER_GWAIN {
            htmltext = "32197-05.html".to_owned();
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

    const NAME: &str = "Q00062_PathOfTheTrooper";

    const NPCS: &str = r#"
        <npc id="32194" type="Folk" level="70" name="Master Shubain"/>
        <npc id="32197" type="Folk" level="70" name="Master Gwain"/>
        <npc id="20014" type="Monster" level="20" name="Felim Lizardman Warrior"/>
        <npc id="20038" type="Monster" level="20" name="Venomous Spider"/>
        <npc id="20062" type="Monster" level="20" name="Tumran Bugbear"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9749" type="EtcItem" name="Felim Lizardman's Head"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9750" type="EtcItem" name="Venomous Spider's Leg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9751" type="EtcItem" name="Tumran Bugbear's Heart"><set name="is_questitem" val="true"/></item>
        <item id="9752" type="EtcItem" name="Shubain's Recommendation"><set name="is_questitem" val="true"/></item>
        <item id="9753" type="EtcItem" name="Gwain's Recommendation"/>"#;

    struct Npcs {
        gwain: Oid,
        shubain: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00062PathOfTheTrooper);
        let pages = Pages::of(&Q00062PathOfTheTrooper)
            .page("32197-01.htm", &["ACCEPT"])
            .page("32194-01.html", &["32194-02.html"])
            .plain(&[
                "32194-02.html",
                "32194-03.html",
                "32194-04.html",
                "32194-05.html",
                "32194-06.html",
                "32194-07.html",
                "32197-02.html",
                "32197-03.html",
                "32197-04.html",
                "32197-05.html",
                "32197-06.htm",
                "32197-07.html",
                "32197-08.html",
                "32197-09.html",
                "32197-10.html",
            ])
            .npcs(&[MASTER_GWAIN, MASTER_SHUBAIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            gwain: t.spawn(MASTER_GWAIN, 20),
            shubain: t.spawn(MASTER_SHUBAIN, 40),
        };
        (t, npcs)
    }

    /// A Kamael male soldier of `level`.
    fn enter_soldier(t: &mut ScriptTest, level: i32) {
        t.enter_as(1, level, race::KAMAEL, player_class::MALE_SOLDIER, vec![]);
    }

    /// Calls a script event directly (for the refusals whose page is not linked yet).
    fn direct(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00062PathOfTheTrooper.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    /// Kills `npc_id` until the player has `count` of `item_id`; returns the number of kills.
    fn hunt(t: &mut ScriptTest, npc_id: i32, item_id: i32, count: i64) -> usize {
        for kills in 1..2000 {
            let mob = t.spawn(npc_id, 80);
            t.kill(1, mob);
            if t.count(1, item_id) >= count {
                return kills;
            }
        }
        panic!("no {count} of {item_id} after 2000 kills");
    }

    #[test]
    fn only_a_kamael_male_soldier_of_level_18_starts_it() {
        let (mut t, npcs) = setup();
        enter_soldier(&mut t, 17);
        assert!(t.quest_page(1, npcs.gwain).contains("32197-02.html"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::MALE_SOLDIER, vec![]);
        assert!(t.quest_page(1, npcs.gwain).contains("32197-04.html"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::KAMAEL, player_class::FEMALE_SOLDIER, vec![]);
        assert!(t.quest_page(1, npcs.gwain).contains("32197-03.html"));
    }

    #[test]
    fn the_heads_the_legs_the_heart_and_the_recommendation() {
        let (mut t, npcs) = setup();
        enter_soldier(&mut t, 18);
        assert!(t.quest_page(1, npcs.gwain).contains("32197-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("32197-06.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );

        // Shubain sends the player to the felim lizardmen: memo state 2, cond 2.
        assert!(t.quest_page(1, npcs.shubain).contains("32194-01.html"));
        assert!(t.event(1, NAME, "32194-02.html").contains("32194-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "2"),
                rows::update(NAME, "cond", "2")
            ]
        );

        // Five felim heads; then Shubain takes them: memo state 3, cond 3.
        hunt(&mut t, FELIM_LIZARDMAN_WARRIOR, FELIM_LIZARDMAN_HEAD, 5);
        assert_eq!(t.count(1, FELIM_LIZARDMAN_HEAD), 5);
        assert!(t.quest_page(1, npcs.shubain).contains("32194-04.html"));
        assert_eq!(
            (t.count(1, FELIM_LIZARDMAN_HEAD), t.count(1, VENOMOUS_SPIDERS_LEG)),
            (0, 0)
        );
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3")
            ]
        );

        // Ten spider legs; then Shubain takes them and gives the recommendation: cond 4.
        hunt(&mut t, VENOMOUS_SPIDER, VENOMOUS_SPIDERS_LEG, 10);
        assert!(t.quest_page(1, npcs.shubain).contains("32194-06.html"));
        assert_eq!(t.count(1, SHUBAINS_RECOMMENDATION), 1);
        assert_eq!(t.count(1, VENOMOUS_SPIDERS_LEG), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "4"),
                rows::update(NAME, "cond", "4")
            ]
        );

        // Gwain takes the recommendation: memo state 5, cond 5.
        assert!(t.quest_page(1, npcs.gwain).contains("32197-08.html"));
        assert_eq!(t.count(1, SHUBAINS_RECOMMENDATION), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "5"),
                rows::update(NAME, "cond", "5")
            ]
        );

        // The tumran bugbear heart (a roll of 50%), then Gwain's recommendation and the adena.
        assert!(t.quest_page(1, npcs.gwain).contains("32197-09.html"));
        hunt(&mut t, TUMRAN_BUGBEAR, TUMRAN_BUGBEAR_HEART, 1);
        assert!(t.quest_page(1, npcs.gwain).contains("32197-10.html"));
        assert_eq!(t.count(1, GWAINS_RECOMMENDATION), 1);
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, TUMRAN_BUGBEAR_HEART), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.gwain).contains("32197-05.html"));
    }

    #[test]
    fn the_shubain_pages_refuse_short_counts_and_gwain_waits_for_the_heart() {
        let (mut t, npcs) = setup();
        enter_soldier(&mut t, 18);
        t.quest_page(1, npcs.gwain);
        t.event(1, NAME, "ACCEPT");
        t.quest_page(1, npcs.shubain);
        t.event(1, NAME, "32194-02.html");
        t.take_writes();

        // Four felim heads: Shubain refuses, nothing taken.
        t.world.run_as_script(NAME, |ctx| {
            for _ in 0..4 {
                ctx.give_items(Oid(1), FELIM_LIZARDMAN_HEAD, 1);
            }
        });
        assert!(t.quest_page(1, npcs.shubain).contains("32194-03.html"));
        assert_eq!(t.count(1, FELIM_LIZARDMAN_HEAD), 4);
        assert!(t.take_writes().is_empty());

        // Gwain at cond 5 without the heart: the page for it.
        assert_eq!(direct(&mut t, "ACCEPT"), None);
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, 5, false);
            }
        });
        t.take_writes();
        assert!(t.quest_page(1, npcs.gwain).contains("32197-09.html"));
        assert_eq!(t.count(1, 57), 0);
        assert!(t.take_writes().is_empty());
    }
}
