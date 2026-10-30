//! Quest 414: Path of the Orc Raider (Prefect Karukia, Prefect Kasman and Prefect Tazeer).
// Java: dist/game/data/scripts/quests/Q00414_PathOfTheOrcRaider/Q00414_PathOfTheOrcRaider.java
//
// Pattern: a kill chain where the green blood drops summon the Kuruka ratman leader (a spawn at
// the goblin's place that attacks the killer), the tooth count sets cond 2, and two ways lead to
// the reward: Kasman (betrayer heads) or Tazeer (the Timora orc head), with the memo state as
// the step counter (`docs/design/scripts.md`, "Quest patterns").

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PREFECT_KARUKIA: i32 = 30570;
const PREFRCT_KASMAN: i32 = 30501;
const PREFRCT_TAZEER: i32 = 31978;

// Items
const GREEN_BLOOD: i32 = 1578;
const GOBLIN_DWELLING_MAP: i32 = 1579;
const KURUKA_RATMAN_TOOTH: i32 = 1580;
const BETRAYER_UMBAR_REPORT: i32 = 1589;
const BETRAYER_ZAKAN_REPORT: i32 = 1590;
const HEAD_OF_BETRAYER: i32 = 1591;
const TIMORA_ORC_HEAD: i32 = 8544;

// Reward
const MARK_OF_RAIDER: i32 = 1592;

// Quest monsters
const KURUKA_RATMAN_LEADER: i32 = 27045;
const UMBAR_ORC: i32 = 27054;
const TIMORA_ORC: i32 = 27320;

// Monster
const GOBLIN_TOMB_RAIDER_LEADER: i32 = 20320;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `Q00414_PathOfTheOrcRaider`.
pub struct Q00414PathOfTheOrcRaider;

impl Script for Q00414PathOfTheOrcRaider {
    fn name(&self) -> &'static str {
        "Q00414_PathOfTheOrcRaider"
    }

    fn path(&self) -> &'static str {
        "quests/Q00414_PathOfTheOrcRaider"
    }

    fn id(&self) -> i32 {
        414
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PREFECT_KARUKIA]);
        s.add_talk_id(&[PREFECT_KARUKIA, PREFRCT_KASMAN, PREFRCT_TAZEER]);
        s.add_kill_id(&[
            KURUKA_RATMAN_LEADER,
            UMBAR_ORC,
            TIMORA_ORC,
            GOBLIN_TOMB_RAIDER_LEADER,
        ]);
        s.register_quest_items(&[
            GREEN_BLOOD,
            GOBLIN_DWELLING_MAP,
            KURUKA_RATMAN_TOOTH,
            BETRAYER_UMBAR_REPORT,
            BETRAYER_ZAKAN_REPORT,
            HEAD_OF_BETRAYER,
            TIMORA_ORC_HEAD,
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
                htmltext = Some(
                    if ctx.player_class_id(player) == player_class::ORC_FIGHTER {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            if ctx.has_quest_items(player, MARK_OF_RAIDER) {
                                "30570-04.htm"
                            } else {
                                if !ctx.has_quest_items(player, GOBLIN_DWELLING_MAP) {
                                    ctx.give_items(player, GOBLIN_DWELLING_MAP, 1);
                                }
                                qs.start_quest(ctx);
                                "30570-05.htm"
                            }
                        } else {
                            "30570-02.htm"
                        }
                    } else if ctx.player_class_id(player) == player_class::ORC_RAIDER {
                        "30570-02a.htm"
                    } else {
                        "30570-03.htm"
                    }
                    .to_owned(),
                );
            }
            "30570-07a.html" => {
                if ctx.has_quest_items(player, GOBLIN_DWELLING_MAP)
                    && ctx.get_quest_items_count(player, KURUKA_RATMAN_TOOTH) >= 10
                {
                    ctx.take_items(player, GOBLIN_DWELLING_MAP, 1);
                    ctx.take_items(player, KURUKA_RATMAN_TOOTH, -1);
                    ctx.give_items(player, BETRAYER_UMBAR_REPORT, 1);
                    ctx.give_items(player, BETRAYER_ZAKAN_REPORT, 1);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30570-07b.html" => {
                if ctx.has_quest_items(player, GOBLIN_DWELLING_MAP)
                    && ctx.get_quest_items_count(player, KURUKA_RATMAN_TOOTH) >= 10
                {
                    ctx.take_items(player, GOBLIN_DWELLING_MAP, 1);
                    ctx.take_items(player, KURUKA_RATMAN_TOOTH, -1);
                    qs.set_cond(ctx, 5, true);
                    qs.set_memo_state(ctx, 2);
                    htmltext = Some(event.to_owned());
                }
            }
            "31978-04.html" => {
                if qs.is_memo_state(ctx, 2) {
                    htmltext = Some(event.to_owned());
                }
            }
            "31978-02.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 6, true);
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
            GOBLIN_TOMB_RAIDER_LEADER => {
                if ctx.has_quest_items(killer, GOBLIN_DWELLING_MAP)
                    && ctx.get_quest_items_count(killer, KURUKA_RATMAN_TOOTH) < 10
                    && ctx.get_quest_items_count(killer, GREEN_BLOOD) <= 20
                {
                    if i64::from(ctx.get_random(100)) < ctx.get_quest_items_count(killer, GREEN_BLOOD) * 5 {
                        ctx.take_items(killer, GREEN_BLOOD, -1);
                        if let Some(leader) = ctx.add_spawn_at(KURUKA_RATMAN_LEADER, npc.object_id, true, 0) {
                            ctx.add_attack_desire(leader, killer, 999);
                        }
                    } else {
                        ctx.give_items(killer, GREEN_BLOOD, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            KURUKA_RATMAN_LEADER => {
                if ctx.has_quest_items(killer, GOBLIN_DWELLING_MAP)
                    && ctx.get_quest_items_count(killer, KURUKA_RATMAN_TOOTH) < 10
                {
                    ctx.take_items(killer, GREEN_BLOOD, -1);
                    if ctx.get_quest_items_count(killer, KURUKA_RATMAN_TOOTH) >= 9 {
                        ctx.give_items(killer, KURUKA_RATMAN_TOOTH, 1);
                        qs.set_cond(ctx, 2, true);
                    } else {
                        ctx.give_items(killer, KURUKA_RATMAN_TOOTH, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            UMBAR_ORC => {
                if ctx.has_at_least_one_quest_item(killer, &[BETRAYER_UMBAR_REPORT, BETRAYER_ZAKAN_REPORT])
                    && ctx.get_quest_items_count(killer, HEAD_OF_BETRAYER) < 2
                    && ctx.get_random(10) < 2
                {
                    ctx.give_items(killer, HEAD_OF_BETRAYER, 1);
                    if ctx.has_quest_items(killer, BETRAYER_ZAKAN_REPORT) {
                        ctx.take_items(killer, BETRAYER_ZAKAN_REPORT, 1);
                    } else if ctx.has_quest_items(killer, BETRAYER_UMBAR_REPORT) {
                        ctx.take_items(killer, BETRAYER_UMBAR_REPORT, 1);
                    }
                    if ctx.get_quest_items_count(killer, HEAD_OF_BETRAYER) == 2 {
                        qs.set_cond(ctx, 4, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            TIMORA_ORC => {
                if qs.is_memo_state(ctx, 3)
                    && !ctx.has_quest_items(killer, TIMORA_ORC_HEAD)
                    && ctx.get_random(100) < 60
                {
                    ctx.give_items(killer, TIMORA_ORC_HEAD, 1);
                    qs.set_cond(ctx, 7, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED || state == COMPLETED {
            if npc.id == PREFECT_KARUKIA {
                htmltext = "30570-01.htm".to_owned();
            }
        } else if state == STARTED {
            match npc.id {
                PREFECT_KARUKIA => {
                    if ctx.has_quest_items(player, GOBLIN_DWELLING_MAP)
                        && ctx.get_quest_items_count(player, KURUKA_RATMAN_TOOTH) < 10
                    {
                        htmltext = "30570-06.html".to_owned();
                    } else if ctx.has_quest_items(player, GOBLIN_DWELLING_MAP)
                        && ctx.get_quest_items_count(player, KURUKA_RATMAN_TOOTH) >= 10
                    {
                        if !ctx.has_at_least_one_quest_item(
                            player,
                            &[BETRAYER_UMBAR_REPORT, BETRAYER_ZAKAN_REPORT],
                        ) {
                            htmltext = "30570-07.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, HEAD_OF_BETRAYER)
                        || ctx.has_at_least_one_quest_item(
                            player,
                            &[BETRAYER_UMBAR_REPORT, BETRAYER_ZAKAN_REPORT],
                        )
                    {
                        htmltext = "30570-08.html".to_owned();
                    } else if qs.is_memo_state(ctx, 2) {
                        htmltext = "30570-07b.html".to_owned();
                    }
                }
                PREFRCT_KASMAN => {
                    if !ctx.has_quest_items(player, HEAD_OF_BETRAYER)
                        && ctx
                            .get_quest_items_count_of(player, &[BETRAYER_UMBAR_REPORT, BETRAYER_ZAKAN_REPORT])
                            >= 2
                    {
                        htmltext = "30501-01.html".to_owned();
                    } else if ctx.get_quest_items_count(player, HEAD_OF_BETRAYER) == 1 {
                        htmltext = "30501-02.html".to_owned();
                    } else if ctx.get_quest_items_count(player, HEAD_OF_BETRAYER) == 2 {
                        ctx.give_adena(player, 163_800, true);
                        ctx.give_items(player, MARK_OF_RAIDER, 1);
                        let level = ctx.player_level(player);
                        if level >= 20 {
                            ctx.add_exp_and_sp(player, 320_534, 21_312);
                        } else if level == 19 {
                            ctx.add_exp_and_sp(player, 456_128, 28_010);
                        } else {
                            ctx.add_exp_and_sp(player, 591_724, 34_708);
                        }
                        qs.exit_quest(ctx, false, true);
                        ctx.world_mut().send_social_action(player, 3);
                        htmltext = "30501-03.html".to_owned();
                    }
                }
                PREFRCT_TAZEER => {
                    if qs.is_memo_state(ctx, 2) {
                        htmltext = "31978-01.html".to_owned();
                    } else if qs.is_memo_state(ctx, 3) {
                        if !ctx.has_quest_items(player, TIMORA_ORC_HEAD) {
                            htmltext = "31978-03.html".to_owned();
                        } else {
                            ctx.give_adena(player, 81_900, true);
                            ctx.give_items(player, MARK_OF_RAIDER, 1);
                            let level = ctx.player_level(player);
                            if level >= 20 {
                                ctx.add_exp_and_sp(player, 160_267, 10_656);
                            } else if level == 19 {
                                ctx.add_exp_and_sp(player, 228_064, 14_005);
                            } else {
                                ctx.add_exp_and_sp(player, 295_862, 17_354);
                            }
                            qs.exit_quest(ctx, false, true);
                            ctx.world_mut().send_social_action(player, 3);
                            htmltext = "31978-05.html".to_owned();
                        }
                    }
                }
                _ => {}
            }
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00414_PathOfTheOrcRaider";

    const NPCS: &str = r#"
        <npc id="30570" type="Folk" level="70" name="Prefect Karukia"/>
        <npc id="30501" type="Folk" level="70" name="Prefect Kasman"/>
        <npc id="31978" type="Folk" level="70" name="Prefect Tazeer"/>
        <npc id="20320" type="Monster" level="20" name="Goblin Tomb Raider Leader"/>
        <npc id="27045" type="Monster" level="20" name="Kuruka Ratman Leader"/>
        <npc id="27054" type="Monster" level="30" name="Umbar Orc"/>
        <npc id="27320" type="Monster" level="30" name="Timora Orc"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1578" type="EtcItem" name="Green Blood"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1579" type="EtcItem" name="Goblin Dwelling Map"><set name="is_questitem" val="true"/></item>
        <item id="1580" type="EtcItem" name="Kuruka Ratman Tooth"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1589" type="EtcItem" name="Betrayer Umbar Report"><set name="is_questitem" val="true"/></item>
        <item id="1590" type="EtcItem" name="Betrayer Zakan Report"><set name="is_questitem" val="true"/></item>
        <item id="1591" type="EtcItem" name="Head of Betrayer"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="8544" type="EtcItem" name="Timora Orc Head"><set name="is_questitem" val="true"/></item>
        <item id="1592" type="EtcItem" name="Mark of Raider"/>"#;

    struct Npcs {
        karukia: Oid,
        kasman: Oid,
        tazeer: Oid,
    }

    fn pages() -> Pages {
        Pages::of(&Q00414PathOfTheOrcRaider)
            .page("30570-01.htm", &["ACCEPT"])
            .page("30570-07.html", &["30570-07a.html", "30570-07b.html"])
            .page("31978-01.html", &["31978-04.html"])
            .page("31978-04.html", &["31978-02.html"])
            .plain(&[
                "30501-01.html",
                "30501-02.html",
                "30501-03.html",
                "30570-02.htm",
                "30570-02a.htm",
                "30570-03.htm",
                "30570-04.htm",
                "30570-05.htm",
                "30570-06.html",
                "30570-07a.html",
                "30570-07b.html",
                "30570-08.html",
                "31978-02.html",
                "31978-03.html",
                "31978-05.html",
            ])
            .npcs(&[PREFECT_KARUKIA, PREFRCT_KASMAN, PREFRCT_TAZEER])
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00414PathOfTheOrcRaider);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            karukia: t.spawn(PREFECT_KARUKIA, 20),
            kasman: t.spawn(PREFRCT_KASMAN, 40),
            tazeer: t.spawn(PREFRCT_TAZEER, 60),
        };
        (t, npcs)
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

    /// Ten kuruka ratman teeth: the goblins drop green blood that calls the leaders, which the
    /// player kills. Each leader that spawned is removed once it is dead.
    fn ten_teeth(t: &mut ScriptTest) {
        for _ in 0..5000 {
            if t.count(1, KURUKA_RATMAN_TOOTH) >= 10 {
                return;
            }
            if let Some(&leader) = t.npcs_of(KURUKA_RATMAN_LEADER).first() {
                t.kill(1, leader);
                t.world.delete_npc(leader);
            } else {
                let goblin = t.spawn(GOBLIN_TOMB_RAIDER_LEADER, 80);
                t.kill(1, goblin);
            }
        }
        panic!("no ten teeth after 5000 kills");
    }

    #[test]
    fn the_accept_page_depends_on_class_level_and_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 17, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.karukia).contains("30570-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30570-02.htm"));
        assert!(t.take_writes().is_empty());

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ORC, player_class::ORC_RAIDER, vec![]);
        t.quest_page(1, npcs.karukia);
        assert!(t.event(1, NAME, "ACCEPT").contains("30570-02a.htm"));

        let (mut t, npcs) = setup();
        t.enter_as(1, 30, race::ORC, player_class::ORC_MAGE, vec![]);
        t.quest_page(1, npcs.karukia);
        assert!(t.event(1, NAME, "ACCEPT").contains("30570-03.htm"));

        // With the mark the talk already shows the page (no link to ACCEPT on it).
        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            30,
            race::ORC,
            player_class::ORC_FIGHTER,
            vec![item(5000, 1592, 1)],
        );
        assert!(t.quest_page(1, npcs.karukia).contains("30570-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30570-04.htm"));
    }

    #[test]
    fn the_reports_from_kasman_to_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ORC, player_class::ORC_FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.karukia).contains("30570-01.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30570-05.htm"));
        assert_eq!(t.count(1, GOBLIN_DWELLING_MAP), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.karukia).contains("30570-06.html"));

        // Ten teeth; the tenth sets cond 2.
        ten_teeth(&mut t);
        assert_eq!(t.count(1, KURUKA_RATMAN_TOOTH), 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, npcs.karukia).contains("30570-07.html"));

        // Karukia takes the map and the teeth and gives the two reports: cond 3.
        assert!(t.event(1, NAME, "30570-07a.html").contains("30570-07a.html"));
        assert_eq!(
            (
                t.count(1, GOBLIN_DWELLING_MAP),
                t.count(1, KURUKA_RATMAN_TOOTH),
                t.count(1, BETRAYER_ZAKAN_REPORT)
            ),
            (0, 0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Umbar orcs give betrayer heads (a 20% drop each). The first head: Kasman's page 02 is
        // not reached until there are two heads; the second head sets cond 4.
        hunt(&mut t, UMBAR_ORC, HEAD_OF_BETRAYER, 1);
        assert!(t.quest_page(1, npcs.kasman).contains("30501-02.html"));
        assert_eq!(t.take_writes(), []);
        hunt(&mut t, UMBAR_ORC, HEAD_OF_BETRAYER, 2);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Kasman's reward: adena, the mark, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.kasman).contains("30501-03.html"));
        assert_eq!(t.count(1, 57), 163_800);
        assert_eq!(t.count(1, MARK_OF_RAIDER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
        assert!(t.quest_page(1, npcs.karukia).contains("30570-01.htm"));
    }

    #[test]
    fn the_timora_orc_head_from_tazeer_to_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 18, race::ORC, player_class::ORC_FIGHTER, vec![]);
        t.quest_page(1, npcs.karukia);
        t.event(1, NAME, "ACCEPT");
        t.take_writes();
        ten_teeth(&mut t);
        assert!(t.quest_page(1, npcs.karukia).contains("30570-07.html"));
        t.take_writes();

        // Karukia's other way: cond 5 (a jump from 2 skips 3 and 4: the completed-state flags,
        // 0x80000013, as Java writes them) and memo state 2; Tazeer takes it from there.
        assert!(t.event(1, NAME, "30570-07b.html").contains("30570-07b.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "5"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483629"),
                rows::insert(NAME, "memoState", "2"),
            ]
        );
        assert!(t.quest_page(1, npcs.tazeer).contains("31978-01.html"));
        assert!(t.event(1, NAME, "31978-04.html").contains("31978-04.html"));
        assert!(t.event(1, NAME, "31978-02.html").contains("31978-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "6"),
                rows::update(NAME, "__compltdStateFlags", "-2147483597"),
            ]
        );
        assert!(t.quest_page(1, npcs.tazeer).contains("31978-03.html"));

        // The Timora orc head (a 60% drop): cond 7.
        hunt(&mut t, TIMORA_ORC, TIMORA_ORC_HEAD, 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "7"),
                rows::update(NAME, "__compltdStateFlags", "-2147483533"),
            ]
        );

        // Tazeer's reward: adena, the mark, the quest ends, the bow.
        assert!(t.quest_page(1, npcs.tazeer).contains("31978-05.html"));
        assert_eq!(t.count(1, 57), 81_900);
        assert_eq!(t.count(1, MARK_OF_RAIDER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
