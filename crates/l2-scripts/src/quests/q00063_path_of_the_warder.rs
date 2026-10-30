//! Quest 63: Path of the Warder (Master Sione, Master Gobie, Master Tobias, Captain Bathis, the Kamael female soldiers' masters).
// Java: dist/game/data/scripts/quests/Q00063_PathOfTheWarder/Q00063_PathOfTheWarder.java
//
// Pattern: a delivery and kill chain with the memo state as the step counter
// (`docs/design/scripts.md`, "Quest patterns"), sibling of `q00062_path_of_the_trooper.rs`. A
// quest monster (Ol Mahum Officer Tak) appears after five lizardmen and is sent at the killer.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MASTER_SIONE: i32 = 32195;
const MASTER_GOBIE: i32 = 32198;
const MASTER_TOBIAS: i32 = 30297;
const CAPTAIN_BATHIS: i32 = 30332;

// Items
const ORDERS: i32 = 9762;
const ORGANIZATION_CHART: i32 = 9763;
const GOBIES_ORDERS: i32 = 9764;
const LETTER_TO_HUMANS: i32 = 9765;
const HUMANS_REOLY: i32 = 9766;
const LETTER_TO_THE_DARKELVES: i32 = 9767;
const DARK_ELVES_REPLY: i32 = 9768;
const REPORT_TO_SIONE: i32 = 9769;
const EMPTY_SOUL_CRYSTAL: i32 = 9770;
const TAKS_CAPTURED_SOUL: i32 = 9771;

// Reward
const STEELRAZOR_EVALUTION: i32 = 9772;

// Monster
const OL_MAHUM_PATROL: i32 = 20053;
const OL_MAHUM_NOVICE: i32 = 20782;
const MAILLE_LIZARDMAN: i32 = 20919;
const MAILLE_LIZARDMAN_SCOUT: i32 = 20920;
const MAILLE_LIZARDMAN_GUARD: i32 = 20921;

// Quest Monster
const OL_MAHUM_OFFICER_TAK: i32 = 27337;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00063_PathOfTheWarder`.
pub struct Q00063PathOfTheWarder;

impl Script for Q00063PathOfTheWarder {
    fn name(&self) -> &'static str {
        "Q00063_PathOfTheWarder"
    }

    fn path(&self) -> &'static str {
        "quests/Q00063_PathOfTheWarder"
    }

    fn id(&self) -> i32 {
        63
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_SIONE]);
        s.add_talk_id(&[MASTER_SIONE, MASTER_GOBIE, MASTER_TOBIAS, CAPTAIN_BATHIS]);
        s.add_kill_id(&[
            OL_MAHUM_PATROL,
            OL_MAHUM_NOVICE,
            MAILLE_LIZARDMAN,
            MAILLE_LIZARDMAN_SCOUT,
            MAILLE_LIZARDMAN_GUARD,
            OL_MAHUM_OFFICER_TAK,
        ]);
        s.register_quest_items(&[
            ORDERS,
            ORGANIZATION_CHART,
            GOBIES_ORDERS,
            LETTER_TO_HUMANS,
            HUMANS_REOLY,
            LETTER_TO_THE_DARKELVES,
            DARK_ELVES_REPLY,
            REPORT_TO_SIONE,
            EMPTY_SOUL_CRYSTAL,
            TAKS_CAPTURED_SOUL,
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
                    htmltext = Some("32195-05.htm".to_owned());
                }
            }
            "32195-06.html" => {
                if qs.is_memo_state(ctx, 1) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32195-08.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32198-03.html" => {
                if qs.is_memo_state(ctx, 3) {
                    ctx.take_items(player, GOBIES_ORDERS, 1);
                    ctx.give_items(player, LETTER_TO_HUMANS, 1);
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32198-07.html" => {
                if qs.is_memo_state(ctx, 7) {
                    htmltext = Some(event.to_owned());
                }
            }
            "32198-08.html" => {
                if qs.is_memo_state(ctx, 7) {
                    ctx.give_items(player, LETTER_TO_THE_DARKELVES, 1);
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32198-12.html" => {
                if qs.is_memo_state(ctx, 12) {
                    ctx.give_items(player, REPORT_TO_SIONE, 1);
                    qs.set_memo_state(ctx, 13);
                    qs.set_cond(ctx, 9, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "32198-15.html" => {
                if qs.is_memo_state(ctx, 14) {
                    qs.set_memo_state(ctx, 15);
                    htmltext = Some(event.to_owned());
                }
            }
            "32198-16.html" => {
                if qs.is_memo_state(ctx, 15) {
                    ctx.give_items(player, EMPTY_SOUL_CRYSTAL, 1);
                    qs.set_memo_state(ctx, 16);
                    qs.set_int(ctx, "ex", 0);
                    qs.set_cond(ctx, 11, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30332-03.html" => {
                if qs.is_memo_state(ctx, 4) {
                    ctx.take_items(player, LETTER_TO_HUMANS, 1);
                    ctx.give_items(player, HUMANS_REOLY, 1);
                    qs.set_memo_state(ctx, 5);
                    htmltext = Some(event.to_owned());
                }
            }
            "30332-05.html" => {
                if qs.is_memo_state(ctx, 5) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30332-06.html" => {
                if qs.is_memo_state(ctx, 5) {
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 6, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30297-03.html" => {
                if qs.is_memo_state(ctx, 9) {
                    htmltext = Some(event.to_owned());
                }
            }
            "30297-04.html" => {
                if qs.is_memo_state(ctx, 9) {
                    qs.set_memo_state(ctx, 10);
                    htmltext = Some(event.to_owned());
                }
            }
            "30297-06.html" => {
                if qs.is_memo_state(ctx, 10) {
                    ctx.give_items(player, DARK_ELVES_REPLY, 1);
                    qs.set_memo_state(ctx, 11);
                    qs.set_cond(ctx, 8, true);
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
            OL_MAHUM_PATROL => {
                if qs.is_memo_state(ctx, 2) && ctx.get_quest_items_count(killer, ORGANIZATION_CHART) < 5 {
                    if ctx.get_quest_items_count(killer, ORDERS) >= 10
                        && ctx.get_quest_items_count(killer, ORGANIZATION_CHART) >= 4
                    {
                        qs.set_cond(ctx, 3, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                    ctx.give_items(killer, ORGANIZATION_CHART, 1);
                }
            }
            OL_MAHUM_NOVICE => {
                if qs.is_memo_state(ctx, 2) && ctx.get_quest_items_count(killer, ORDERS) < 10 {
                    if ctx.get_quest_items_count(killer, ORDERS) >= 9
                        && ctx.get_quest_items_count(killer, ORGANIZATION_CHART) >= 5
                    {
                        qs.set_cond(ctx, 3, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                    ctx.give_items(killer, ORDERS, 1);
                }
            }
            MAILLE_LIZARDMAN | MAILLE_LIZARDMAN_SCOUT | MAILLE_LIZARDMAN_GUARD => {
                if qs.is_memo_state(ctx, 16) && !ctx.has_quest_items(killer, TAKS_CAPTURED_SOUL) {
                    let i4 = qs.get_int(ctx, "ex");
                    if i4 < 4 {
                        qs.set_int(ctx, "ex", i4 + 1);
                    } else {
                        qs.set_int(ctx, "ex", 0);
                        if let Some(monster) = ctx.add_spawn_at(OL_MAHUM_OFFICER_TAK, npc.object_id, true, 0)
                        {
                            ctx.add_attack_desire(monster, killer, 999);
                        }
                    }
                }
            }
            OL_MAHUM_OFFICER_TAK => {
                if qs.is_memo_state(ctx, 16) && !ctx.has_quest_items(killer, TAKS_CAPTURED_SOUL) {
                    ctx.take_items(killer, EMPTY_SOUL_CRYSTAL, 1);
                    ctx.give_items(killer, TAKS_CAPTURED_SOUL, 1);
                    qs.set_cond(ctx, 12, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                if npc.id == MASTER_SIONE {
                    if ctx.player_class_id(player) == player_class::FEMALE_SOLDIER
                        && !ctx.has_quest_items(player, STEELRAZOR_EVALUTION)
                    {
                        htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                            "32195-01.htm"
                        } else {
                            "32195-02.html"
                        }
                        .to_owned();
                    } else if ctx.has_quest_items(player, STEELRAZOR_EVALUTION) {
                        htmltext = "32195-03.html".to_owned();
                    } else {
                        htmltext = "32195-04.html".to_owned();
                    }
                }
            }
            STARTED => match npc.id {
                MASTER_SIONE => {
                    if memo_state == 1 {
                        htmltext = "32195-07.html".to_owned();
                    } else if memo_state == 2 {
                        if ctx.get_quest_items_count(player, ORDERS) < 10
                            || ctx.get_quest_items_count(player, ORGANIZATION_CHART) < 5
                        {
                            htmltext = "32195-09.html".to_owned();
                        } else {
                            ctx.take_items(player, ORDERS, -1);
                            ctx.take_items(player, ORGANIZATION_CHART, -1);
                            ctx.give_items(player, GOBIES_ORDERS, 1);
                            qs.set_memo_state(ctx, 3);
                            qs.set_cond(ctx, 4, true);
                            htmltext = "32195-10.html".to_owned();
                        }
                    } else if memo_state == 3 {
                        htmltext = "32195-11.html".to_owned();
                    } else if memo_state > 3 && memo_state != 13 {
                        htmltext = "32195-12.html".to_owned();
                    } else if memo_state == 13 {
                        ctx.take_items(player, REPORT_TO_SIONE, 1);
                        qs.set_memo_state(ctx, 14);
                        qs.set_cond(ctx, 10, true);
                        htmltext = "32195-13.html".to_owned();
                    }
                }
                MASTER_GOBIE => {
                    if memo_state < 3 {
                        htmltext = "32198-01.html".to_owned();
                    } else if memo_state == 3 {
                        htmltext = "32198-02.html".to_owned();
                    } else if memo_state == 4 || memo_state == 5 {
                        htmltext = "32198-04.html".to_owned();
                    } else if memo_state == 6 {
                        ctx.take_items(player, HUMANS_REOLY, 1);
                        qs.set_memo_state(ctx, 7);
                        htmltext = "32198-05.html".to_owned();
                    } else if memo_state == 7 {
                        htmltext = "32198-06.html".to_owned();
                    } else if memo_state == 8 {
                        htmltext = "32198-09.html".to_owned();
                    } else if memo_state == 11 {
                        ctx.take_items(player, DARK_ELVES_REPLY, 1);
                        qs.set_memo_state(ctx, 12);
                        htmltext = "32198-10.html".to_owned();
                    } else if memo_state == 12 {
                        ctx.give_items(player, REPORT_TO_SIONE, 1);
                        qs.set_memo_state(ctx, 13);
                        htmltext = "32198-11.html".to_owned();
                    } else if memo_state == 13 {
                        htmltext = "32198-13.html".to_owned();
                    } else if memo_state == 14 {
                        htmltext = "32198-14.html".to_owned();
                    } else if memo_state == 15 {
                        ctx.give_items(player, EMPTY_SOUL_CRYSTAL, 1);
                        qs.set_memo_state(ctx, 16);
                        qs.set_int(ctx, "ex", 0);
                        qs.set_cond(ctx, 11, true);
                        htmltext = "32198-17.html".to_owned();
                    } else if memo_state == 16 {
                        if !ctx.has_quest_items(player, TAKS_CAPTURED_SOUL) {
                            qs.set_int(ctx, "ex", 0);
                            htmltext = "32198-18.html".to_owned();
                        } else {
                            ctx.give_adena(player, 163_800, true);
                            ctx.take_items(player, TAKS_CAPTURED_SOUL, 1);
                            ctx.give_items(player, STEELRAZOR_EVALUTION, 1);
                            let level = ctx.player_level(player);
                            if level >= 20 {
                                ctx.add_exp_and_sp(player, 320_534, 22_046);
                            } else if level == 19 {
                                ctx.add_exp_and_sp(player, 456_128, 28_744);
                            } else {
                                ctx.add_exp_and_sp(player, 591_724, 35_442);
                            }
                            qs.exit_quest(ctx, false, true);
                            // `SocialAction(3)` goes to this player only.
                            ctx.send_social_action(player, 3);
                            htmltext = "32198-19.html".to_owned();
                        }
                    }
                }
                CAPTAIN_BATHIS => {
                    if memo_state == 4 {
                        htmltext = "30332-01.html".to_owned();
                    } else if memo_state < 4 {
                        htmltext = "30332-02.html".to_owned();
                    } else if memo_state == 5 {
                        htmltext = "30332-04.html".to_owned();
                    } else if memo_state == 6 {
                        htmltext = "30332-07.html".to_owned();
                    } else if memo_state > 6 {
                        htmltext = "30332-08.html".to_owned();
                    }
                }
                MASTER_TOBIAS => {
                    if memo_state == 8 {
                        ctx.take_items(player, LETTER_TO_THE_DARKELVES, 1);
                        qs.set_memo_state(ctx, 9);
                        htmltext = "30297-01.html".to_owned();
                    } else if memo_state == 9 {
                        htmltext = "30297-02.html".to_owned();
                    } else if memo_state == 10 {
                        ctx.give_items(player, DARK_ELVES_REPLY, 1);
                        qs.set_memo_state(ctx, 11);
                        qs.set_cond(ctx, 8, true);
                        htmltext = "30297-05.html".to_owned();
                    } else if memo_state >= 11 {
                        htmltext = "30297-07.html".to_owned();
                    }
                }
                _ => {}
            },
            COMPLETED => {
                if npc.id == MASTER_GOBIE && ctx.has_quest_items(player, STEELRAZOR_EVALUTION) {
                    htmltext = "32198-20.html".to_owned();
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
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00063_PathOfTheWarder";

    const NPCS: &str = r#"
        <npc id="32195" type="Folk" level="70" name="Master Sione"/>
        <npc id="32198" type="Folk" level="70" name="Master Gobie"/>
        <npc id="30297" type="Folk" level="70" name="Master Tobias"/>
        <npc id="30332" type="Folk" level="70" name="Captain Bathis"/>
        <npc id="20053" type="Monster" level="20" name="Ol Mahum Patrol"/>
        <npc id="20782" type="Monster" level="20" name="Ol Mahum Novice"/>
        <npc id="20919" type="Monster" level="20" name="Maille Lizardman"/>
        <npc id="27337" type="Monster" level="20" name="Ol Mahum Officer Tak"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9762" type="EtcItem" name="Orders"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9763" type="EtcItem" name="Organization Chart"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9764" type="EtcItem" name="Gobie's Orders"><set name="is_questitem" val="true"/></item>
        <item id="9765" type="EtcItem" name="Letter to Humans"><set name="is_questitem" val="true"/></item>
        <item id="9769" type="EtcItem" name="Report to Sione"><set name="is_questitem" val="true"/></item>
        <item id="9770" type="EtcItem" name="Empty Soul Crystal"><set name="is_questitem" val="true"/></item>
        <item id="9771" type="EtcItem" name="Tak's Captured Soul"><set name="is_questitem" val="true"/></item>
        <item id="9772" type="EtcItem" name="Steelrazor Evalution"/>"#;

    struct Npcs {
        sione: Oid,
        gobie: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00063PathOfTheWarder);
        let pages = Pages::of(&Q00063PathOfTheWarder)
            .page("32195-01.htm", &["ACCEPT"])
            .page("32195-05.htm", &["32195-06.html"])
            .page("32195-06.html", &["32195-08.html"])
            .plain(&[
                "32195-02.html",
                "32195-03.html",
                "32195-04.html",
                "32195-07.html",
                "32195-08.html",
                "32195-09.html",
                "32195-10.html",
                "32198-17.html",
                "32198-18.html",
                "32198-19.html",
            ])
            .npcs(&[MASTER_SIONE, MASTER_GOBIE, MASTER_TOBIAS, CAPTAIN_BATHIS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            sione: t.spawn(MASTER_SIONE, 20),
            gobie: t.spawn(MASTER_GOBIE, 40),
        };
        (t, npcs)
    }

    fn enter_soldier(t: &mut ScriptTest, level: i32) {
        t.enter_as(1, level, race::KAMAEL, player_class::FEMALE_SOLDIER, vec![]);
    }

    fn ev(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00063PathOfTheWarder.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00063PathOfTheWarder.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn memo(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, value);
            }
        });
        t.take_writes();
    }

    #[test]
    fn only_a_female_soldier_of_level_18_starts_it() {
        let (mut t, npcs) = setup();
        enter_soldier(&mut t, 17);
        assert!(t.quest_page(1, npcs.sione).contains("32195-02.html"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::KAMAEL, player_class::MALE_SOLDIER, vec![]);
        assert!(t.quest_page(1, npcs.sione).contains("32195-04.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_orders_and_the_charts_then_gobie() {
        let (mut t, npcs) = setup();
        enter_soldier(&mut t, 18);
        assert!(t.quest_page(1, npcs.sione).contains("32195-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("32195-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert!(t.event(1, NAME, "32195-06.html").contains("32195-06.html"));
        assert!(t.event(1, NAME, "32195-08.html").contains("32195-08.html"));
        t.take_writes();

        // Ten orders from the novices and five charts from the patrols: cond 3.
        for _ in 0..10 {
            let mob = t.spawn(OL_MAHUM_NOVICE, 80);
            t.kill(1, mob);
        }
        for _ in 0..5 {
            let mob = t.spawn(OL_MAHUM_PATROL, 80);
            t.kill(1, mob);
        }
        assert_eq!((t.count(1, ORDERS), t.count(1, ORGANIZATION_CHART)), (10, 5));
        assert!(t.quest_page(1, npcs.sione).contains("32195-10.html"));
        assert_eq!(t.count(1, GOBIES_ORDERS), 1);
        assert_eq!(t.count(1, ORDERS), 0);
        assert_eq!(ev(&mut t, "32198-03.html").as_deref(), Some("32198-03.html"));
        assert_eq!(t.count(1, LETTER_TO_HUMANS), 1);
    }

    #[test]
    fn the_fifth_lizardman_brings_tak_and_gobie_pays() {
        let (mut t, npcs) = setup();
        enter_soldier(&mut t, 19);
        t.quest_page(1, npcs.sione);
        t.event(1, NAME, "ACCEPT");
        memo(&mut t, 15);
        assert_eq!(ev(&mut t, "32198-16.html").as_deref(), Some("32198-16.html"));
        assert_eq!(t.count(1, EMPTY_SOUL_CRYSTAL), 1);

        // Five lizardmen: the fifth call makes Tak appear next to it.
        for _ in 0..5 {
            let mob = t.spawn(MAILLE_LIZARDMAN, 80);
            t.kill(1, mob);
        }
        let tak = t.npcs_of(OL_MAHUM_OFFICER_TAK);
        assert_eq!(tak.len(), 1);
        t.kill(1, tak[0]);
        assert_eq!(t.count(1, TAKS_CAPTURED_SOUL), 1);
        assert_eq!(t.count(1, EMPTY_SOUL_CRYSTAL), 0);

        t.take_writes();
        assert_eq!(talk(&mut t, npcs.gobie).as_deref(), Some("32198-19.html"));
        assert_eq!(t.count(1, STEELRAZOR_EVALUTION), 1);
        assert_eq!(t.count(1, 57), 163_800);
        let w: Vec<_> = t
            .take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect();
        assert_eq!(
            w,
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert_eq!(talk(&mut t, npcs.gobie).as_deref(), Some("32198-20.html"));
    }
}
