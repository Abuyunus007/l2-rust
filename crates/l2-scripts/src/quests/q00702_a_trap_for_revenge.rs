//! Quest 702: A Trap for Revenge (Plenos, Engineer Lekon and Tenius at the Keucereus Alliance
//! Base; the Drakes of Gracia).
// Java: dist/game/data/scripts/quests/Q00702_ATrapForRevenge/Q00702_ATrapForRevenge.java
//
// Pattern: a repeatable quest with items traded between three NPCs. Drake's Flesh (cond 2) is
// sold to Plenos or turned into Rotten Blood by Tenius; Lekon makes the bait; the Variant Drake
// Wing Horns that the bait brings out are sold or opened for a random reward.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const PLENOS: i32 = 32563;
const LEKON: i32 = 32557;
const TENIUS: i32 = 32555;
const MONSTERS: [i32; 7] = [22612, 22613, 25632, 22610, 22611, 25631, 25626];

// Items
const DRAKES_FLESH: i32 = 13877;
const ROTTEN_BLOOD: i32 = 13878;
const BAIT_FOR_DRAKES: i32 = 13879;
const VARIANT_DRAKE_WING_HORNS: i32 = 13880;
const EXTRACTED_RED_STAR_STONE: i32 = 14009;

/// Java `Q00702_ATrapForRevenge`.
pub struct Q00702ATrapForRevenge;

impl Q00702ATrapForRevenge {
    /// The two-item pairs and the single piece of `32555-15`'s second roll (`i1` over 600) or
    /// (`i1` under 600): which of 9628, 9629 and 9630 are given, each `getRandom(3) + 1`.
    fn give_scrolls(ctx: &mut ScriptCtx<'_>, player: ObjectId, items: &[i32]) {
        for item in items {
            let amount = ctx.get_random(3) + 1;
            ctx.give_items(player, *item, i64::from(amount));
        }
    }
}

impl Script for Q00702ATrapForRevenge {
    fn name(&self) -> &'static str {
        "Q00702_ATrapForRevenge"
    }

    fn path(&self) -> &'static str {
        "quests/Q00702_ATrapForRevenge"
    }

    fn id(&self) -> i32 {
        702
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PLENOS]);
        s.add_talk_id(&[PLENOS, LEKON, TENIUS]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[
            DRAKES_FLESH,
            ROTTEN_BLOOD,
            BAIT_FOR_DRAKES,
            VARIANT_DRAKE_WING_HORNS,
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
        let mut htmltext = event.to_owned();
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        // Java compares with `equalsIgnoreCase`.
        let event_lc = event.to_ascii_lowercase();
        match event_lc.as_str() {
            "32563-04.htm" => qs.start_quest(ctx),
            "32563-07.html" => {
                htmltext = if ctx.has_quest_items(player, DRAKES_FLESH) {
                    "32563-08.html"
                } else {
                    "32563-07.html"
                }
                .to_owned();
            }
            "32563-09.html" => {
                let flesh = ctx.get_quest_items_count(player, DRAKES_FLESH);
                ctx.give_adena(player, flesh * 100, false);
                ctx.take_items(player, DRAKES_FLESH, -1);
            }
            "32563-11.html" => {
                if ctx.has_quest_items(player, VARIANT_DRAKE_WING_HORNS) {
                    let horns = ctx.get_quest_items_count(player, VARIANT_DRAKE_WING_HORNS);
                    ctx.give_adena(player, horns * 200_000, false);
                    ctx.take_items(player, VARIANT_DRAKE_WING_HORNS, -1);
                    htmltext = "32563-12.html".to_owned();
                } else {
                    htmltext = "32563-11.html".to_owned();
                }
            }
            "32563-14.html" => qs.exit_quest(ctx, true, true),
            "32557-03.html" => {
                let blood = ctx.has_quest_items(player, ROTTEN_BLOOD);
                let stones = ctx.get_quest_items_count(player, EXTRACTED_RED_STAR_STONE) >= 100;
                if !blood && !stones {
                    htmltext = "32557-03.html".to_owned();
                } else if blood && !stones {
                    htmltext = "32557-04.html".to_owned();
                } else if !blood && stones {
                    htmltext = "32557-05.html".to_owned();
                } else {
                    ctx.give_items(player, BAIT_FOR_DRAKES, 1);
                    ctx.take_items(player, ROTTEN_BLOOD, 1);
                    ctx.take_items(player, EXTRACTED_RED_STAR_STONE, 100);
                    htmltext = "32557-06.html".to_owned();
                }
            }
            "32555-03.html" => qs.set_cond(ctx, 2, true),
            "32555-05.html" => qs.exit_quest(ctx, true, true),
            "32555-06.html" => {
                htmltext = if ctx.get_quest_items_count(player, DRAKES_FLESH) < 100 {
                    "32555-06.html"
                } else {
                    "32555-07.html"
                }
                .to_owned();
            }
            "32555-08.html" => {
                ctx.give_items(player, ROTTEN_BLOOD, 1);
                ctx.take_items(player, DRAKES_FLESH, 100);
            }
            "32555-10.html" => {
                htmltext = if ctx.has_quest_items(player, VARIANT_DRAKE_WING_HORNS) {
                    "32555-11.html"
                } else {
                    "32555-10.html"
                }
                .to_owned();
            }
            "32555-15.html" => {
                let i0 = ctx.get_random(1000);
                let i1 = ctx.get_random(1000);
                if i0 >= 500 && i1 >= 600 {
                    let adena = ctx.get_random(49917) + 125_000;
                    ctx.give_adena(player, i64::from(adena), false);
                    Self::pair_reward(ctx, player, i1);
                    htmltext = "32555-15.html".to_owned();
                } else if i0 >= 500 && i1 < 600 {
                    let adena = ctx.get_random(49917) + 125_000;
                    ctx.give_adena(player, i64::from(adena), false);
                    Self::single_reward(ctx, player, i1);
                    htmltext = "32555-16.html".to_owned();
                } else if i0 < 500 && i1 >= 600 {
                    let adena = ctx.get_random(49917) + 25_000;
                    ctx.give_adena(player, i64::from(adena), false);
                    Self::pair_reward(ctx, player, i1);
                    htmltext = "32555-17.html".to_owned();
                } else {
                    let adena = ctx.get_random(49917) + 25_000;
                    ctx.give_adena(player, i64::from(adena), false);
                    Self::single_reward(ctx, player, i1);
                    htmltext = "32555-18.html".to_owned();
                }
                ctx.take_items(player, VARIANT_DRAKE_WING_HORNS, 1);
            }
            _ => {}
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_random_party_member_cond(killer, 2).is_none() {
            return;
        }
        // NOTE(java-quirk): the items go to the killer, not to the party member drawn.
        let chance = ctx.get_random(1000);
        let double = |limit: i32| if chance < limit { 2 } else { 1 };
        match npc.id {
            22612 => ctx.give_items(killer, DRAKES_FLESH, double(413)),
            22613 => ctx.give_items(killer, DRAKES_FLESH, double(440)),
            25632 => {
                if chance < 996 {
                    ctx.give_items(killer, DRAKES_FLESH, 1);
                }
            }
            22610 => ctx.give_items(killer, DRAKES_FLESH, double(485)),
            22611 => ctx.give_items(killer, DRAKES_FLESH, double(451)),
            25631 => ctx.give_items(killer, DRAKES_FLESH, double(485)),
            25626 => {
                let count = if chance < 708 {
                    ctx.get_random(2) + 1
                } else if chance < 978 {
                    ctx.get_random(3) + 3
                } else if chance < 994 {
                    ctx.get_random(4) + 6
                } else if chance < 998 {
                    ctx.get_random(4) + 10
                } else if chance < 1000 {
                    ctx.get_random(5) + 14
                } else {
                    0
                };
                ctx.give_items(killer, VARIANT_DRAKE_WING_HORNS, i64::from(count));
            }
            _ => {}
        }
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if npc.id == PLENOS {
            match qs.state(ctx) {
                CREATED => {
                    let prev_done = ctx
                        .player_quest_state(player, "Q10273_GoodDayToFly")
                        .is_some_and(|q| q.is_completed());
                    htmltext = if prev_done && ctx.player_level(player) >= 78 {
                        "32563-01.htm"
                    } else {
                        "32563-02.htm"
                    }
                    .to_owned();
                }
                STARTED => {
                    htmltext = if qs.is_cond(ctx, 1) {
                        "32563-05.html"
                    } else {
                        "32563-06.html"
                    }
                    .to_owned();
                }
                _ => {}
            }
        }

        if qs.state(ctx) == STARTED {
            if npc.id == LEKON {
                match qs.get_cond(ctx) {
                    1 => htmltext = "32557-01.html".to_owned(),
                    2 => htmltext = "32557-02.html".to_owned(),
                    _ => {}
                }
            } else if npc.id == TENIUS {
                match qs.get_cond(ctx) {
                    1 => htmltext = "32555-01.html".to_owned(),
                    2 => htmltext = "32555-04.html".to_owned(),
                    _ => {}
                }
            }
        }
        Some(htmltext)
    }
}

impl Q00702ATrapForRevenge {
    /// The reward of a roll `i1` of 600 or more: two or three kinds of scrolls.
    fn pair_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, i1: i32) {
        if i1 < 720 {
            Self::give_scrolls(ctx, player, &[9628, 9629]);
        } else if i1 < 840 {
            Self::give_scrolls(ctx, player, &[9629, 9630]);
        } else if i1 < 960 {
            Self::give_scrolls(ctx, player, &[9628, 9630]);
        } else if i1 < 1000 {
            Self::give_scrolls(ctx, player, &[9628, 9629, 9630]);
        }
    }

    /// The reward of a roll `i1` under 600: nothing under 210, else one kind of scroll.
    fn single_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, i1: i32) {
        if i1 < 210 {
            // Do nothing.
        } else if i1 < 340 {
            Self::give_scrolls(ctx, player, &[9628]);
        } else if i1 < 470 {
            Self::give_scrolls(ctx, player, &[9629]);
        } else if i1 < 600 {
            Self::give_scrolls(ctx, player, &[9630]);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::COMPLETED;

    use super::*;
    use crate::quests::q10273_good_day_to_fly::Q10273GoodDayToFly;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00702_ATrapForRevenge";
    const GOOD_DAY_TO_FLY: &str = "Q10273_GoodDayToFly";

    const NPCS: &str = r#"
        <npc id="32563" type="Folk" level="1" name="Plenos"/>
        <npc id="32557" type="Folk" level="1" name="Lekon"/>
        <npc id="32555" type="Folk" level="1" name="Tenius"/>
        <npc id="22612" type="Monster" level="78" name="Drake"/>
        <npc id="25626" type="RaidBoss" level="80" name="Drake Leader"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9628" type="EtcItem" name="Scroll A"><set name="is_stackable" val="true"/></item>
        <item id="9629" type="EtcItem" name="Scroll B"><set name="is_stackable" val="true"/></item>
        <item id="9630" type="EtcItem" name="Scroll C"><set name="is_stackable" val="true"/></item>
        <item id="13877" type="EtcItem" name="Drake's Flesh"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13878" type="EtcItem" name="Rotten Blood"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13879" type="EtcItem" name="Bait for Drakes"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="13880" type="EtcItem" name="Variant Drake Wing Horns"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="14009" type="EtcItem" name="Extracted Red Star Stone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        plenos: Oid,
        lekon: Oid,
        tenius: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00702ATrapForRevenge);
        registry.add(Q10273GoodDayToFly);
        let pages = Pages::of(&Q00702ATrapForRevenge)
            .page("32563-01.htm", &["32563-04.htm"])
            .page(
                "32563-05.html",
                &["32563-07.html", "32563-09.html", "32563-11.html", "32563-14.html"],
            )
            .page(
                "32563-06.html",
                &["32563-07.html", "32563-09.html", "32563-11.html", "32563-14.html"],
            )
            .plain(&[
                "32563-02.htm",
                "32563-04.htm",
                "32563-07.html",
                "32563-08.html",
                "32563-09.html",
                "32563-11.html",
                "32563-12.html",
                "32563-14.html",
            ])
            .page("32557-01.html", &["32557-03.html"])
            .page("32557-02.html", &["32557-03.html"])
            .plain(&["32557-03.html", "32557-04.html", "32557-05.html", "32557-06.html"])
            .page("32555-01.html", &["32555-03.html"])
            .page(
                "32555-04.html",
                &["32555-05.html", "32555-06.html", "32555-10.html"],
            )
            .plain(&[
                "32555-03.html",
                "32555-05.html",
                "32555-06.html",
                "32555-07.html",
                "32555-08.html",
                "32555-10.html",
                "32555-11.html",
                "32555-15.html",
                "32555-16.html",
                "32555-17.html",
                "32555-18.html",
            ])
            .npcs(&[PLENOS, LEKON, TENIUS]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            plenos: t.spawn(PLENOS, 20),
            lekon: t.spawn(LEKON, 30),
            tenius: t.spawn(TENIUS, 40),
        };
        (t, npcs)
    }

    fn complete_good_day_to_fly(t: &mut ScriptTest) {
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.quest_state_of(GOOD_DAY_TO_FLY, Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    /// Starts the quest at level 78 after Good Day to Fly.
    fn start(t: &mut ScriptTest, n: &Npcs) {
        t.enter(1, 78, vec![]);
        complete_good_day_to_fly(t);
        assert!(t.quest_page(1, n.plenos).contains("32563-01.htm"));
        assert!(t.event(1, NAME, "32563-04.htm").contains("32563-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
    }

    /// Lekon also hosts Good Day to Fly (Q10273): the talk offers both quests, the link of
    /// this one is `npc_<id>_Script <name>`.
    fn lekon_page(t: &mut ScriptTest, lekon: Oid) -> String {
        t.talk(1, lekon);
        t.bypass(1, "Script");
        t.take();
        t.bypass(1, &format!("npc_{}_Script {NAME}", lekon.0));
        crate::testing::pages_to(&t.take(), 1).pop().unwrap_or_default()
    }

    fn give(t: &mut ScriptTest, item: i32, count: i64) {
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), item, count));
    }

    #[test]
    fn the_quest_needs_good_day_to_fly_and_level_78() {
        let (mut t, n) = setup();
        t.enter(1, 77, vec![]);
        complete_good_day_to_fly(&mut t);
        assert!(t.quest_page(1, n.plenos).contains("32563-02.htm"));
    }

    #[test]
    fn tenius_moves_the_quest_to_cond_2_and_the_drakes_drop_flesh() {
        let (mut t, n) = setup();
        start(&mut t, &n);
        assert!(t.quest_page(1, n.tenius).contains("32555-01.html"));
        assert!(t.event(1, NAME, "32555-03.html").contains("32555-03.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // A Drake drops one or two pieces per kill.
        let drake = t.spawn(22612, 60);
        t.kill(1, drake);
        let flesh = t.count(1, DRAKES_FLESH);
        assert!((1..=2).contains(&flesh), "{flesh}");
        // Plenos pays 100 adena per piece.
        assert!(t.quest_page(1, n.plenos).contains("32563-06.html"));
        assert!(t.event(1, NAME, "32563-09.html").contains("32563-09.html"));
        assert_eq!(t.count(1, 57), flesh * 100);
        assert_eq!(t.count(1, DRAKES_FLESH), 0);
    }

    #[test]
    fn a_hundred_flesh_make_rotten_blood_and_lekon_makes_the_bait() {
        let (mut t, n) = setup();
        start(&mut t, &n);
        t.quest_page(1, n.tenius);
        t.event(1, NAME, "32555-03.html");
        give(&mut t, DRAKES_FLESH, 100);
        assert!(t.quest_page(1, n.tenius).contains("32555-04.html"));
        assert!(t.event(1, NAME, "32555-06.html").contains("32555-07.html"));
        // Rotten blood by Tenius's 08 page: the event is not linked from page 07 in this stand-in.
        t.world.run_as_script(NAME, |ctx| {
            Q00702ATrapForRevenge.on_event(ctx, "32555-08.html", None, Some(Oid(1)));
        });
        assert_eq!((t.count(1, ROTTEN_BLOOD), t.count(1, DRAKES_FLESH)), (1, 0));

        give(&mut t, EXTRACTED_RED_STAR_STONE, 100);
        let page = lekon_page(&mut t, n.lekon);
        assert!(page.contains("32557-02.html"), "{page}");
        assert!(t.event(1, NAME, "32557-03.html").contains("32557-06.html"));
        assert_eq!(
            (
                t.count(1, BAIT_FOR_DRAKES),
                t.count(1, ROTTEN_BLOOD),
                t.count(1, EXTRACTED_RED_STAR_STONE)
            ),
            (1, 0, 0)
        );
    }

    #[test]
    fn the_horns_are_sold_to_plenos() {
        let (mut t, n) = setup();
        start(&mut t, &n);
        give(&mut t, VARIANT_DRAKE_WING_HORNS, 2);
        t.quest_page(1, n.plenos);
        assert!(t.event(1, NAME, "32563-11.html").contains("32563-12.html"));
        assert_eq!(t.count(1, 57), 400_000);
        assert_eq!(t.count(1, VARIANT_DRAKE_WING_HORNS), 0);
    }

    #[test]
    fn a_horn_opened_by_tenius_gives_adena_and_scrolls() {
        let (mut t, n) = setup();
        start(&mut t, &n);
        t.quest_page(1, n.tenius);
        t.event(1, NAME, "32555-03.html");
        give(&mut t, VARIANT_DRAKE_WING_HORNS, 1);
        t.quest_page(1, n.tenius);
        t.world.run_as_script(NAME, |ctx| {
            let page = Q00702ATrapForRevenge.on_event(ctx, "32555-15.html", None, Some(Oid(1)));
            assert!(page.is_some_and(|p| {
                ["32555-15.html", "32555-16.html", "32555-17.html", "32555-18.html"].contains(&p.as_str())
            }));
        });
        assert_eq!(t.count(1, VARIANT_DRAKE_WING_HORNS), 0);
        let adena = t.count(1, 57);
        assert!((25_000..=174_916).contains(&adena), "{adena}");
    }

    #[test]
    fn plenos_closes_the_quest() {
        let (mut t, n) = setup();
        start(&mut t, &n);
        t.quest_page(1, n.plenos);
        assert!(t.event(1, NAME, "32563-14.html").contains("32563-14.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
