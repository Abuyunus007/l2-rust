//! Quest 10270: Birth of the Seed (Plenos, Artius, Ginby and Lelrikia at the Keucereus Alliance
//! Base; the Seed of Infinity bosses).
// Java: dist/game/data/scripts/quests/Q10270_BirthOfTheSeed/Q10270_BirthOfTheSeed.java
//
// Pattern: a one-time story quest on the memo state. The kills of Yehan Klodekus, Yehan Klanikus
// and Cohemenes give each party member in range one proof (`executeForEachPlayer`); the way on
// depends on the player's "Light Fragment" (Q10272) state.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const ARTIUS: i32 = 32559;
const PLENOS: i32 = 32563;
const GINBY: i32 = 32566;
const LELRIKIA: i32 = 32567;

// Monsters
const COHEMENES: i32 = 25634;
const YEHAN_KLODEKUS: i32 = 25665;
const YEHAN_KLANIKUS: i32 = 25666;

// Items
const YEHAN_KLODEKUS_BADGE: i32 = 13868;
const YEHAN_KLANIKUS_BADGE: i32 = 13869;
const LICH_CRYSTAL: i32 = 13870;
const ADENA: i32 = 57;

// Misc
const MIN_LEVEL: i32 = 75;
const LIGHT_FRAGMENT: &str = "Q10272_LightFragment";

// Location
const INSTANCE_EXIT: (i32, i32, i32) = (-185057, 242821, 1576);

/// Java `Q10270_BirthOfTheSeed`.
pub struct Q10270BirthOfTheSeed;

/// `(st1 == null) || (st1.isStarted() && (st1.getMemoState() < 10))`.
fn fragment_not_yet(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.player_quest_state(player, LIGHT_FRAGMENT)
        .is_none_or(|q| q.is_started() && q.memo_state() < 10)
}

/// `st1 != null && ((st1.isStarted() && st1.getMemoState() >= 10) || st1.isCompleted())`.
fn fragment_done(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.player_quest_state(player, LIGHT_FRAGMENT)
        .is_some_and(|q| (q.is_started() && q.memo_state() >= 10) || q.is_completed())
}

impl Script for Q10270BirthOfTheSeed {
    fn name(&self) -> &'static str {
        "Q10270_BirthOfTheSeed"
    }

    fn path(&self) -> &'static str {
        "quests/Q10270_BirthOfTheSeed"
    }

    fn id(&self) -> i32 {
        10270
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PLENOS]);
        s.add_talk_id(&[PLENOS, GINBY, LELRIKIA, ARTIUS]);
        s.add_kill_id(&[COHEMENES, YEHAN_KLODEKUS, YEHAN_KLANIKUS]);
        s.register_quest_items(&[YEHAN_KLODEKUS_BADGE, YEHAN_KLANIKUS_BADGE, LICH_CRYSTAL]);
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
        if qs.is_memo_state(ctx, 2)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false)
        {
            let badge = match npc.id {
                YEHAN_KLODEKUS => YEHAN_KLODEKUS_BADGE,
                YEHAN_KLANIKUS => YEHAN_KLANIKUS_BADGE,
                COHEMENES => LICH_CRYSTAL,
                _ => return,
            };
            if !ctx.has_quest_items(player, badge) {
                ctx.give_items(player, badge, 1);
                ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
            }
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
        match event {
            "32563-02.htm" => htmltext = Some(event.to_owned()),
            "32563-03.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                htmltext = Some(event.to_owned());
            }
            "32566-02.html" => {
                if qs.is_memo_state(ctx, 4) {
                    if fragment_not_yet(ctx, player) {
                        htmltext = Some(event.to_owned());
                    } else if fragment_done(ctx, player) {
                        htmltext = Some("32566-03.html".to_owned());
                    }
                }
            }
            "32566-04.html" => {
                if qs.is_memo_state(ctx, 4) {
                    if ctx.get_quest_items_count(player, ADENA) < 10000 {
                        htmltext = Some(event.to_owned());
                    } else {
                        ctx.take_items(player, ADENA, 10000);
                        qs.set_memo_state(ctx, 5);
                        htmltext = Some("32566-05.html".to_owned());
                    }
                }
            }
            "32566-06.html" => {
                if qs.is_memo_state(ctx, 5) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32567-02.html" | "32567-03.html" => {
                if qs.is_memo_state(ctx, 10) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32567-04.html" => {
                if qs.is_memo_state(ctx, 10) {
                    qs.set_memo_state(ctx, 11);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32567-05.html" => {
                if qs.is_memo_state(ctx, 11) {
                    qs.set_memo_state(ctx, 20);
                    let (x, y, z) = INSTANCE_EXIT;
                    ctx.teleport_player(player, x, y, z, 0, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32559-02.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32559-08.html" => {
                if qs.is_memo_state(ctx, 3) && fragment_not_yet(ctx, player) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32559-10.html" => {
                if qs.is_memo_state(ctx, 3) && fragment_done(ctx, player) {
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        ctx.execute_for_each_player(killer, npc, is_summon, true, false);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            COMPLETED => {
                if npc.id == PLENOS {
                    htmltext = "32563-05.html".to_owned();
                } else if npc.id == ARTIUS {
                    htmltext = "32559-03.html".to_owned();
                }
            }
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "32563-01.htm"
                } else {
                    "32563-04.htm"
                }
                .to_owned();
            }
            STARTED => match npc.id {
                PLENOS => {
                    if qs.is_memo_state(ctx, 1) {
                        htmltext = "32563-06.html".to_owned();
                    }
                }
                GINBY => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state == 4 {
                        htmltext = "32566-01.html".to_owned();
                    } else if memo_state < 4 {
                        htmltext = "32566-07.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "32566-06.html".to_owned();
                    } else if (10..20).contains(&memo_state) {
                        htmltext = "32566-08.html".to_owned();
                    } else if memo_state == 20 {
                        htmltext = "32566-09.html".to_owned();
                    }
                }
                LELRIKIA => {
                    let memo_state = qs.get_memo_state(ctx);
                    if memo_state == 10 {
                        htmltext = "32567-01.html".to_owned();
                    } else if memo_state == 11 {
                        htmltext = "32567-06.html".to_owned();
                    }
                }
                ARTIUS => match qs.get_memo_state(ctx) {
                    1 => htmltext = "32559-01.html".to_owned(),
                    2 => {
                        let klodekus = ctx.has_quest_items(player, YEHAN_KLODEKUS_BADGE);
                        let klanikus = ctx.has_quest_items(player, YEHAN_KLANIKUS_BADGE);
                        let lich = ctx.has_quest_items(player, LICH_CRYSTAL);
                        if klodekus && klanikus && lich {
                            qs.set_memo_state(ctx, 3);
                            qs.set_cond(ctx, 3, true);
                            ctx.take_items_many(
                                player,
                                -1,
                                &[YEHAN_KLODEKUS_BADGE, YEHAN_KLANIKUS_BADGE, LICH_CRYSTAL],
                            );
                            htmltext = "32559-04.html".to_owned();
                        } else if !klodekus && !klanikus && !lich {
                            htmltext = "32559-05.html".to_owned();
                        } else {
                            htmltext = "32559-06.html".to_owned();
                        }
                    }
                    3 => {
                        if fragment_not_yet(ctx, player) {
                            htmltext = "32559-07.html".to_owned();
                        } else if fragment_done(ctx, player) {
                            htmltext = "32559-09.html".to_owned();
                        }
                    }
                    20 => {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            ctx.give_adena(player, 133590, true);
                            ctx.add_exp_and_sp(player, 625343, 48222);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "32559-11.html".to_owned();
                        }
                    }
                    _ => {}
                },
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

    const NAME: &str = "Q10270_BirthOfTheSeed";

    const NPCS: &str = r#"
        <npc id="32559" type="Folk" level="1" name="Artius"/>
        <npc id="32563" type="Folk" level="1" name="Plenos"/>
        <npc id="32566" type="Folk" level="1" name="Ginby"/>
        <npc id="32567" type="Folk" level="1" name="Lelrikia"/>
        <npc id="25665" type="RaidBoss" level="80" name="Yehan Klodekus"/>
        <npc id="25666" type="RaidBoss" level="80" name="Yehan Klanikus"/>
        <npc id="25634" type="RaidBoss" level="80" name="Cohemenes"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13868" type="EtcItem" name="Klodekus Badge"><set name="is_questitem" val="true"/></item>
        <item id="13869" type="EtcItem" name="Klanikus Badge"><set name="is_questitem" val="true"/></item>
        <item id="13870" type="EtcItem" name="Lich Crystal"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10270BirthOfTheSeed);
        let pages = Pages::of(&Q10270BirthOfTheSeed)
            .page("32563-01.htm", &["32563-02.htm"])
            .page("32563-02.htm", &["32563-03.htm"])
            .plain(&["32563-03.htm", "32563-04.htm", "32559-01.html", "32559-02.html"])
            .page("32559-01.html", &["32559-02.html"])
            .plain(&[
                "32559-04.html",
                "32559-05.html",
                "32559-06.html",
                "32559-07.html",
                "32559-08.html",
                "32559-10.html",
                "32559-11.html",
                "32566-01.html",
                "32566-02.html",
                "32566-04.html",
                "32566-05.html",
                "32566-06.html",
                "32566-07.html",
                "32567-01.html",
                "32567-04.html",
                "32567-05.html",
            ])
            .npcs(&[PLENOS, ARTIUS, GINBY, LELRIKIA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let plenos = t.spawn(PLENOS, 20);
        let artius = t.spawn(ARTIUS, 30);
        (t, plenos, artius)
    }

    fn talk(t: &mut ScriptTest, npc_id: i32, player: i32) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            let npc = ScriptNpc {
                object_id: Oid(0),
                id: npc_id,
            };
            out = Q10270BirthOfTheSeed.on_talk(ctx, npc, Oid(player));
        });
        out
    }

    fn event(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q10270BirthOfTheSeed.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    fn memo(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_memo_state(ctx, value);
        });
        t.take_writes();
    }

    #[test]
    fn the_level_gate_and_the_start() {
        let (mut t, plenos, _) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, plenos).contains("32563-04.htm"));
        let (mut t, plenos, _) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, plenos).contains("32563-01.htm"));
        assert!(t.event(1, NAME, "32563-02.htm").contains("32563-02.htm"));
        assert!(t.event(1, NAME, "32563-03.htm").contains("32563-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert_eq!(talk(&mut t, PLENOS, 1).as_deref(), Some("32563-06.html"));
        assert_eq!(talk(&mut t, ARTIUS, 1).as_deref(), Some("32559-01.html"));
    }

    #[test]
    fn the_bosses_give_one_proof_each_and_artius_takes_them() {
        let (mut t, plenos, _) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, plenos);
        t.event(1, NAME, "32563-02.htm");
        t.event(1, NAME, "32563-03.htm");
        assert_eq!(event(&mut t, "32559-02.html").as_deref(), Some("32559-02.html"));
        assert_eq!(talk(&mut t, ARTIUS, 1).as_deref(), Some("32559-05.html"));
        t.take_writes();

        for boss in [25665, 25666, 25634] {
            let npc = t.spawn(boss, 60);
            t.kill(1, npc);
            // A second kill gives no second proof.
            let npc = t.spawn(boss, 60);
            t.kill(1, npc);
        }
        for item in [YEHAN_KLODEKUS_BADGE, YEHAN_KLANIKUS_BADGE, LICH_CRYSTAL] {
            assert_eq!(t.count(1, item), 1);
        }
        assert_eq!(talk(&mut t, ARTIUS, 1).as_deref(), Some("32559-04.html"));
        for item in [YEHAN_KLODEKUS_BADGE, YEHAN_KLANIKUS_BADGE, LICH_CRYSTAL] {
            assert_eq!(t.count(1, item), 0);
        }
        // Without a Light Fragment (Q10272) the second path is offered.
        assert_eq!(talk(&mut t, ARTIUS, 1).as_deref(), Some("32559-07.html"));
        assert_eq!(event(&mut t, "32559-10.html"), None);
        assert_eq!(event(&mut t, "32559-08.html").as_deref(), Some("32559-08.html"));
    }

    #[test]
    fn ginby_takes_ten_thousand_adena() {
        let (mut t, plenos, _) = setup();
        t.enter(1, 80, vec![crate::testing::item(100, 57, 10_000)]);
        t.quest_page(1, plenos);
        t.event(1, NAME, "32563-02.htm");
        t.event(1, NAME, "32563-03.htm");
        memo(&mut t, 4);
        assert_eq!(talk(&mut t, GINBY, 1).as_deref(), Some("32566-01.html"));
        assert_eq!(event(&mut t, "32566-02.html").as_deref(), Some("32566-02.html"));
        assert_eq!(event(&mut t, "32566-04.html").as_deref(), Some("32566-05.html"));
        assert_eq!(t.count(1, 57), 0);
        assert_eq!(talk(&mut t, GINBY, 1).as_deref(), Some("32566-06.html"));
    }

    #[test]
    fn the_reward_at_the_end() {
        let (mut t, plenos, _) = setup();
        t.enter(1, 80, vec![]);
        t.quest_page(1, plenos);
        t.event(1, NAME, "32563-02.htm");
        t.event(1, NAME, "32563-03.htm");
        memo(&mut t, 10);
        assert_eq!(talk(&mut t, LELRIKIA, 1).as_deref(), Some("32567-01.html"));
        assert_eq!(event(&mut t, "32567-04.html").as_deref(), Some("32567-04.html"));
        assert_eq!(talk(&mut t, LELRIKIA, 1).as_deref(), Some("32567-06.html"));
        assert_eq!(event(&mut t, "32567-05.html").as_deref(), Some("32567-05.html"));
        t.take_writes();
        assert_eq!(talk(&mut t, GINBY, 1).as_deref(), Some("32566-09.html"));
        assert_eq!(talk(&mut t, ARTIUS, 1).as_deref(), Some("32559-11.html"));
        assert_eq!(t.count(1, 57), 133_590);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::delete(NAME, false)), "{writes:?}");
        assert_eq!(talk(&mut t, ARTIUS, 1).as_deref(), Some("32559-03.html"));
    }
}
