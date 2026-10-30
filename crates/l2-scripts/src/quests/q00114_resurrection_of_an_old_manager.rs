//! Quest 114: Resurrection of an Old Manager (Yumi, Wendy, the box, the stones, Newyear).
// Java: dist/game/data/scripts/quests/Q00114_ResurrectionOfAnOldManager/Q00114_ResurrectionOfAnOldManager.java
//
// Pattern: a long cond chain with quest variables (`talk`, `talk1`, `choice`, `spawned`), an
// `on_event` that echoes the event, a summoned guardian kept in the script's state (Java's
// `static Attackable golem`), and `on_creature_see` of the pile of stones.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NEWYEAR: i32 = 31961;
const YUMI: i32 = 32041;
const STONES: i32 = 32046;
const WENDY: i32 = 32047;
const BOX: i32 = 32050;

// Items
const STARSTONE: i32 = 8287;
const LETTER: i32 = 8288;
const STARSTONE2: i32 = 8289;
const DETCTOR: i32 = 8090;
const DETCTOR2: i32 = 8091;
const ADENA: i32 = 57;

// Monster
const GUARDIAN: i32 = 27318;

/// Java `Q00121_PavelTheGiant.class.getSimpleName()`.
const PAVEL_THE_GIANT: &str = "Q00121_PavelTheGiant";

/// Java `private static Attackable golem`: the guardian of the quest, shared by all players.
#[derive(Default)]
struct Golem(Option<ObjectId>);

/// Java `Q00114_ResurrectionOfAnOldManager`.
pub struct Q00114ResurrectionOfAnOldManager;

impl Script for Q00114ResurrectionOfAnOldManager {
    fn name(&self) -> &'static str {
        "Q00114_ResurrectionOfAnOldManager"
    }

    fn path(&self) -> &'static str {
        "quests/Q00114_ResurrectionOfAnOldManager"
    }

    fn id(&self) -> i32 {
        114
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[YUMI]);
        s.add_talk_id(&[YUMI, WENDY, BOX, STONES, NEWYEAR]);
        s.add_kill_id(&[GUARDIAN]);
        s.add_creature_see_id(&[STONES]);
        s.register_quest_items(&[STARSTONE, STARSTONE2, DETCTOR, DETCTOR2, LETTER]);
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

        let mut htmltext = Some(event.to_owned());
        match event {
            "32041-04.htm" => qs.start_quest(ctx),
            "32041-08.html" => qs.set(ctx, "talk", "1"),
            "32041-09.html" => {
                qs.set_cond(ctx, 2, true);
                qs.unset(ctx, "talk");
            }
            "32041-12.html" => match qs.get_cond(ctx) {
                3 => htmltext = Some("32041-12.html".to_owned()),
                4 => htmltext = Some("32041-13.html".to_owned()),
                5 => htmltext = Some("32041-14.html".to_owned()),
                _ => {}
            },
            "32041-15.html" => qs.set(ctx, "talk", "1"),
            "32041-23.html" => qs.set(ctx, "talk", "2"),
            "32041-26.html" => {
                qs.set_cond(ctx, 6, true);
                qs.unset(ctx, "talk");
            }
            "32041-31.html" => {
                ctx.give_items(player, DETCTOR, 1);
                qs.set_cond(ctx, 17, true);
            }
            "32041-34.html" => {
                qs.set(ctx, "talk", "1");
                ctx.take_items(player, DETCTOR2, 1);
            }
            "32041-38.html" => {
                if qs.get_int(ctx, "choice") == 2 {
                    htmltext = Some("32041-37.html".to_owned());
                }
            }
            "32041-39.html" => {
                qs.unset(ctx, "talk");
                qs.set_cond(ctx, 20, true);
            }
            "32041-40.html" => {
                qs.set_cond(ctx, 21, true);
                qs.unset(ctx, "talk");
                ctx.give_items(player, LETTER, 1);
            }
            "32046-03.html" => qs.set_cond(ctx, 19, true),
            "32046-07.html" => {
                ctx.add_exp_and_sp(player, 1_846_611, 144_270);
                qs.exit_quest(ctx, false, true);
            }
            "32047-02.html" => {
                if qs.get_int(ctx, "talk") == 0 {
                    qs.set(ctx, "talk", "1");
                }
            }
            "32047-03.html" => {
                if qs.get_int(ctx, "talk1") == 0 {
                    qs.set(ctx, "talk1", "1");
                }
            }
            "32047-05.html" => {
                if qs.get_int(ctx, "talk") == 0 || qs.get_int(ctx, "talk1") == 0 {
                    htmltext = Some("32047-04.html".to_owned());
                }
            }
            "32047-06.html" => {
                qs.set(ctx, "choice", "1");
                qs.set_cond(ctx, 3, true);
                qs.unset(ctx, "talk1");
                qs.unset(ctx, "talk");
            }
            "32047-07.html" => {
                qs.set(ctx, "choice", "2");
                qs.set_cond(ctx, 4, true);
                qs.unset(ctx, "talk1");
                qs.unset(ctx, "talk");
            }
            "32047-09.html" => {
                qs.set(ctx, "choice", "3");
                qs.set_cond(ctx, 5, true);
                qs.unset(ctx, "talk1");
                qs.unset(ctx, "talk");
            }
            "32047-14ab.html" => {
                qs.set(ctx, "choice", "3");
                qs.set_cond(ctx, 7, true);
            }
            "32047-14b.html" => qs.set_cond(ctx, 10, true),
            "32047-15b.html" => {
                let golem = ctx.state::<Golem>().0;
                if golem.is_none_or(|g| ctx.is_dead(g)) {
                    if let Some(g) = ctx.add_spawn(GUARDIAN, 96977, -110_625, -3280, 0, false, 0, 0) {
                        ctx.state::<Golem>().0 = Some(g.object_id);
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            g,
                            ChatType::NpcGeneral,
                            NpcStringId::YouS1YouAttackedWendyPrepareToDie,
                            &[&name],
                        );
                        ctx.add_attack_desire(g, player, 999);
                    }
                    qs.set(ctx, "spawned", "1");
                    ctx.start_quest_timer("golem_despawn", 300_000, None, Some(player), false);
                } else if qs.get_int(ctx, "spawned") == 1 {
                    htmltext = Some("32047-17b.html".to_owned());
                } else {
                    htmltext = Some("32047-16b.html".to_owned());
                }
            }
            "32047-20a.html" => qs.set_cond(ctx, 8, true),
            "32047-20b.html" => qs.set_cond(ctx, 12, true),
            "32047-20c.html" => qs.set_cond(ctx, 13, true),
            "32047-21a.html" => qs.set_cond(ctx, 9, true),
            "32047-23a.html" => qs.set_cond(ctx, 23, true),
            "32047-23c.html" => {
                ctx.take_items(player, STARSTONE, 1);
                qs.set_cond(ctx, 15, true);
            }
            "32047-29c.html" => {
                if ctx.get_quest_items_count(player, ADENA) >= 3000 {
                    ctx.give_items(player, STARSTONE2, 1);
                    ctx.take_items(player, ADENA, 3000);
                    qs.unset(ctx, "talk");
                    qs.set_cond(ctx, 26, true);
                } else {
                    htmltext = Some("32047-29ca.html".to_owned());
                }
            }
            "32047-30c.html" => qs.set(ctx, "talk", "1"),
            "32050-01r.html" => qs.set(ctx, "talk", "1"),
            "32050-03.html" => {
                ctx.give_items(player, STARSTONE, 1);
                qs.set_cond(ctx, 14, true);
                qs.unset(ctx, "talk");
            }
            "32050-05.html" => {
                qs.set_cond(ctx, 24, true);
                ctx.give_items(player, STARSTONE2, 1);
            }
            "31961-02.html" => {
                ctx.take_items(player, LETTER, 1);
                ctx.give_items(player, STARSTONE2, 1);
                qs.set_cond(ctx, 22, true);
            }
            "golem_despawn" => {
                qs.unset(ctx, "spawned");
                // NOTE(java-quirk): `golem` is dereferenced without a check; no golem throws.
                if let Some(golem) = ctx.state::<Golem>().0.take()
                    && let Some(golem) = ctx.script_npc(golem)
                {
                    let name = ctx.player_name(player);
                    ctx.broadcast_npc_say(
                        golem,
                        ChatType::NpcGeneral,
                        NpcStringId::S1YourEnemyWasDrivenOutIWillNowWithdrawAndAwaitYourNextCommand,
                        &[&name],
                    );
                    ctx.delete_npc(golem);
                }
                htmltext = None;
            }
            "32041-05.html" | "32041-06.html" | "32041-07.html" | "32041-17.html" | "32041-18.html"
            | "32041-19.html" | "32041-20.html" | "32041-21.html" | "32041-22.html" | "32041-25.html"
            | "32041-29.html" | "32041-30.html" | "32041-35.html" | "32041-36.html" | "32046-05.html"
            | "32046-06.html" | "32047-06a.html" | "32047-12a.html" | "32047-12b.html"
            | "32047-12c.html" | "32047-13a.html" | "32047-14a.html" | "32047-13b.html"
            | "32047-13c.html" | "32047-14c.html" | "32047-15c.html" | "32047-17c.html"
            | "32047-13ab.html" | "32047-15a.html" | "32047-16a.html" | "32047-16c.html"
            | "32047-18a.html" | "32047-19a.html" | "32047-18ab.html" | "32047-19ab.html"
            | "32047-18c.html" | "32047-17a.html" | "32047-19c.html" | "32047-21b.html"
            | "32047-27c.html" | "32047-28c.html" => {}
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_cond(ctx, 10) && qs.get_int(ctx, "spawned") == 1 {
            ctx.broadcast_npc_say(
                npc,
                ChatType::NpcGeneral,
                NpcStringId::ThisEnemyIsFarTooPowerfulForMeToFightIMustWithdraw,
                &[],
            );
            qs.set_cond(ctx, 11, true);
            qs.unset(ctx, "spawned");
            ctx.cancel_quest_timers("golem_despawn");
        }
    }

    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, creature: ObjectId) {
        if ctx.world().player(creature).is_none() {
            return;
        }
        let player = creature;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return;
        };
        if qs.is_cond(ctx, 17) {
            ctx.take_items(player, DETCTOR, 1);
            ctx.give_items(player, DETCTOR2, 1);
            qs.set_cond(ctx, 18, true);
            ctx.show_on_screen_npc_string(
                player,
                NpcStringId::TheRadioSignalDetectorIsRespondingASuspiciousPileOfStonesCatchesYourEye,
                2,
                4500,
                &[],
            );
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let talk = qs.get_int(ctx, "talk");

        match npc.id {
            YUMI => match qs.state(ctx) {
                CREATED => {
                    let prev_completed = ctx
                        .player_quest_state(player, PAVEL_THE_GIANT)
                        .is_some_and(|prev| prev.is_completed());
                    if prev_completed {
                        htmltext = if ctx.player_level(player) >= 70 {
                            "32041-02.htm"
                        } else {
                            "32041-03.htm"
                        }
                        .to_owned();
                    } else {
                        htmltext = "32041-01.htm".to_owned();
                    }
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => {
                        htmltext = if talk == 1 { "32041-08.html" } else { "32041-04.htm" }.to_owned();
                    }
                    2 => htmltext = "32041-10.html".to_owned(),
                    3..=5 => match talk {
                        0 => htmltext = "32041-11.html".to_owned(),
                        1 => htmltext = "32041-16.html".to_owned(),
                        2 => htmltext = "32041-24.html".to_owned(),
                        _ => {}
                    },
                    6 | 7 | 8 | 10 | 11 | 13 | 14 | 15 => htmltext = "32041-27.html".to_owned(),
                    9 | 12 | 16 => htmltext = "32041-28.html".to_owned(),
                    17 | 18 => htmltext = "32041-32.html".to_owned(),
                    19 => {
                        htmltext = if talk == 1 { "32041-34z.html" } else { "32041-33.html" }.to_owned();
                    }
                    20 => htmltext = "32041-39z.html".to_owned(),
                    21 => htmltext = "32041-40z.html".to_owned(),
                    22 | 25 | 26 => {
                        qs.set_cond(ctx, 27, true);
                        htmltext = "32041-41.html".to_owned();
                    }
                    27 => htmltext = "32041-42.html".to_owned(),
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            WENDY => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        2 => {
                            htmltext = if talk == 1 && qs.get_int(ctx, "talk1") == 1 {
                                "32047-05.html"
                            } else {
                                "32047-01.html"
                            }
                            .to_owned();
                        }
                        3 => htmltext = "32047-06b.html".to_owned(),
                        4 => htmltext = "32047-08.html".to_owned(),
                        5 => htmltext = "32047-10.html".to_owned(),
                        6 => match qs.get_int(ctx, "choice") {
                            1 => htmltext = "32047-11a.html".to_owned(),
                            2 => htmltext = "32047-11b.html".to_owned(),
                            3 => htmltext = "32047-11c.html".to_owned(),
                            _ => {}
                        },
                        7 => htmltext = "32047-11c.html".to_owned(),
                        8 => htmltext = "32047-17a.html".to_owned(),
                        9 | 12 | 16 => htmltext = "32047-25c.html".to_owned(),
                        10 => htmltext = "32047-18b.html".to_owned(),
                        11 => htmltext = "32047-19b.html".to_owned(),
                        13 => htmltext = "32047-21c.html".to_owned(),
                        14 => htmltext = "32047-22c.html".to_owned(),
                        15 => {
                            qs.set_cond(ctx, 16, true);
                            htmltext = "32047-24c.html".to_owned();
                        }
                        20 => {
                            if qs.get_int(ctx, "choice") == 1 {
                                htmltext = "32047-22a.html".to_owned();
                            } else {
                                htmltext = if talk == 1 { "32047-31c.html" } else { "32047-26c.html" }
                                    .to_owned();
                            }
                        }
                        23 => htmltext = "32047-23z.html".to_owned(),
                        24 => {
                            qs.set_cond(ctx, 25, true);
                            htmltext = "32047-24a.html".to_owned();
                        }
                        25 => htmltext = "32047-24a.html".to_owned(),
                        26 => htmltext = "32047-32c.html".to_owned(),
                        _ => {}
                    }
                }
            }
            NEWYEAR => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        21 => htmltext = "31961-01.html".to_owned(),
                        22 => htmltext = "31961-03.html".to_owned(),
                        _ => {}
                    }
                }
            }
            BOX => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        13 => {
                            htmltext = if talk == 1 { "32050-02.html" } else { "32050-01.html" }.to_owned();
                        }
                        14 => htmltext = "32050-04.html".to_owned(),
                        23 => htmltext = "32050-04b.html".to_owned(),
                        24 => htmltext = "32050-05z.html".to_owned(),
                        _ => {}
                    }
                }
            }
            STONES => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        18 => htmltext = "32046-02.html".to_owned(),
                        19 => htmltext = "32046-03.html".to_owned(),
                        27 => htmltext = "32046-04.html".to_owned(),
                        _ => {}
                    }
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00114_ResurrectionOfAnOldManager";

    const NPCS: &str = r#"
        <npc id="31961" type="Folk" level="70" name="Newyear"/>
        <npc id="32041" type="Folk" level="70" name="Yumi"/>
        <npc id="32046" type="Folk" level="70" name="Stones"/>
        <npc id="32047" type="Folk" level="70" name="Wendy"/>
        <npc id="27318" type="Monster" level="40" name="Guardian"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="8090" type="EtcItem" name="Detector"><set name="is_questitem" val="true"/></item>
        <item id="8091" type="EtcItem" name="Detector 2"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00121_PavelTheGiant` as far as Yumi reads it: a quest of that name.
    struct PavelTheGiant;

    impl Script for PavelTheGiant {
        fn name(&self) -> &'static str {
            PAVEL_THE_GIANT
        }
        fn path(&self) -> &'static str {
            "quests/Q00121_PavelTheGiant"
        }
        fn id(&self) -> i32 {
            121
        }
        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00114ResurrectionOfAnOldManager);
        registry.add(PavelTheGiant);
        let pages = Pages::of(&Q00114ResurrectionOfAnOldManager)
            .page("32041-02.htm", &["32041-04.htm"])
            .plain(&["32041-01.htm", "32041-03.htm", "32041-04.htm", "32041-08.html"])
            .page("32047-01.html", &["32047-02.html", "32047-03.html", "32047-05.html"])
            .plain(&["32047-02.html", "32047-03.html", "32047-04.html", "32047-05.html"])
            .page("32047-10.html", &["32047-06.html"])
            .plain(&["32047-06.html", "32046-07.html"])
            .npcs(&[YUMI, WENDY, STONES]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let yumi = t.spawn(YUMI, 20);
        let wendy = t.spawn(WENDY, 40);
        let stones = t.spawn(STONES, 60);
        (t, yumi, wendy, stones)
    }

    fn ev(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00114ResurrectionOfAnOldManager.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00114ResurrectionOfAnOldManager.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn cond(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_cond(ctx, value, false);
            }
        });
        t.take_writes();
    }

    #[test]
    fn it_needs_pavel_the_giant_done_and_level_70() {
        let (mut t, yumi, _, _) = setup();
        t.enter(1, 70, vec![]);
        assert!(t.quest_page(1, yumi).contains("32041-01.htm"));

        let (mut t, yumi, _, _) = setup();
        t.enter(1, 69, vec![]);
        t.world.run_as_script(PAVEL_THE_GIANT, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        assert!(t.quest_page(1, yumi).contains("32041-03.htm"));
        t.take_writes();
    }

    #[test]
    fn the_start_and_the_first_talks_write_the_rows() {
        let (mut t, yumi, wendy, _) = setup();
        t.enter(1, 70, vec![]);
        t.world.run_as_script(PAVEL_THE_GIANT, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
        assert!(t.quest_page(1, yumi).contains("32041-02.htm"));
        assert!(t.event(1, NAME, "32041-04.htm").contains("32041-04.htm"));
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "cond", "1"), rows::insert(NAME, "<state>", "Started")]
        );
        assert_eq!(ev(&mut t, "32041-08.html").as_deref(), Some("32041-08.html"));
        assert_eq!(ev(&mut t, "32041-09.html").as_deref(), Some("32041-09.html"));
        t.take_writes();

        // Wendy: both talks needed before the choice; an unknown event answers nothing.
        assert_eq!(talk(&mut t, wendy).as_deref(), Some("32047-01.html"));
        assert_eq!(ev(&mut t, "32047-05.html").as_deref(), Some("32047-04.html"));
        ev(&mut t, "32047-02.html");
        ev(&mut t, "32047-03.html");
        assert_eq!(talk(&mut t, wendy).as_deref(), Some("32047-05.html"));
        assert_eq!(ev(&mut t, "32047-06.html").as_deref(), Some("32047-06.html"));
        assert_eq!(ev(&mut t, "bogus"), None);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "3")));
    }

    #[test]
    fn the_stones_swap_the_detector_and_the_end_pays_the_exp() {
        let (mut t, yumi, _, stones) = setup();
        t.enter(1, 70, vec![]);
        t.world.run_as_script(PAVEL_THE_GIANT, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.quest_page(1, yumi);
        t.event(1, NAME, "32041-04.htm");
        cond(&mut t, 17);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), DETCTOR, 1);
            Q00114ResurrectionOfAnOldManager.on_creature_see(
                ctx,
                ctx.script_npc(stones).unwrap(),
                Oid(1),
            );
        });
        assert_eq!(t.count(1, DETCTOR), 0);
        assert_eq!(t.count(1, DETCTOR2), 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "18")));

        assert_eq!(talk(&mut t, stones).as_deref(), Some("32046-02.html"));
        assert_eq!(ev(&mut t, "32046-07.html").as_deref(), Some("32046-07.html"));
        let w: Vec<_> = t
            .take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect();
        assert!(w.ends_with(&[
            rows::delete(NAME, false),
            rows::update(NAME, "<state>", "Completed")
        ]));
    }

    #[test]
    fn the_guardian_is_spawned_once_and_its_death_moves_the_cond() {
        let (mut t, yumi, _, _) = setup();
        t.enter(1, 70, vec![]);
        t.world.run_as_script(PAVEL_THE_GIANT, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.quest_page(1, yumi);
        t.event(1, NAME, "32041-04.htm");
        cond(&mut t, 10);

        assert_eq!(ev(&mut t, "32047-15b.html").as_deref(), Some("32047-15b.html"));
        let golems = t.npcs_of(GUARDIAN);
        assert_eq!(golems.len(), 1);
        assert!(t.take_writes().contains(&rows::insert(NAME, "spawned", "1")));
        // A second request while the guardian lives shows the "already there" page.
        assert_eq!(ev(&mut t, "32047-15b.html").as_deref(), Some("32047-17b.html"));

        t.kill(1, golems[0]);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "11")));
        // The kill cancelled the despawn timer: nothing is written five minutes later.
        t.wait_ms(301_000);
        assert!(t.take_writes().is_empty());
    }
}
