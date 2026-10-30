//! Quest 464: Oath (a Solina strongbox starts a daily errand through the villages).
// Java: dist/game/data/scripts/quests/Q00464_Oath/Q00464_Oath.java
//
// Pattern: a quest started by using an item (`on_item_talk`), a monster drop (`dropItem`) and a
// daily exit. The NPC to visit is chosen at random (cond 2..9) and kept in the quest variable
// "npc".

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestType, STARTED};
use l2_world::{Script, ScriptCtx, ScriptItem, ScriptNpc, ScriptSetup};

// NPC, EXP, SP, Adena
const NPC: [(i32, i64, i32, i64); 9] = [
    (32596, 0, 0, 0),
    (30657, 15449, 17696, 42910),
    (30839, 189377, 21692, 52599),
    (30899, 249180, 28542, 69210),
    (31350, 249180, 28542, 69210),
    (30539, 19408, 47062, 169442),
    (30297, 24146, 58551, 210806),
    (31960, 15449, 17696, 42910),
    (31588, 15449, 17696, 42910),
];

// Items
const STRONGBOX: i32 = 15537;
const BOOK: i32 = 15538;
const BOOK2: i32 = 15539;

// Misc
const MIN_LEVEL: i32 = 82;

// Monsters: id and the drop chance of the strongbox, out of 1000.
const MOBS: [(i32, i32); 12] = [
    (22799, 9),
    (22794, 6),
    (22800, 10),
    (22796, 9),
    (22798, 9),
    (22795, 8),
    (22797, 7),
    (22789, 5),
    (22791, 4),
    (22790, 5),
    (22792, 4),
    (22793, 5),
];

/// Java `Q00464_Oath`.
pub struct Q00464Oath;

impl Script for Q00464Oath {
    fn name(&self) -> &'static str {
        "Q00464_Oath"
    }

    fn path(&self) -> &'static str {
        "quests/Q00464_Oath"
    }

    fn id(&self) -> i32 {
        464
    }

    fn setup(&self, s: &mut ScriptSetup) {
        for npc in NPC {
            s.add_talk_id(&[npc.0]);
        }

        let mobs: Vec<i32> = MOBS.iter().map(|m| m.0).collect();
        s.add_kill_id(&mobs);
        s.add_item_talk_id(&[STRONGBOX]);
        s.register_quest_items(&[BOOK, BOOK2]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;
        let mut htmltext = Some(event.to_owned());
        match event {
            "32596-04.html" => {
                if !ctx.has_quest_items(player, BOOK) {
                    return Some(ctx.get_no_quest_msg(player));
                }

                let cond = ctx.get_random_range(2, 9);
                qs.set(ctx, "npc", &NPC[(cond - 1) as usize].0.to_string());
                qs.set_cond(ctx, cond, true);
                ctx.take_items(player, BOOK, 1);
                ctx.give_items(player, BOOK2, 1);
                htmltext = Some(
                    match cond {
                        2 => "32596-04.html",
                        3 => "32596-04a.html",
                        4 => "32596-04b.html",
                        5 => "32596-04c.html",
                        6 => "32596-04d.html",
                        7 => "32596-04e.html",
                        8 => "32596-04f.html",
                        _ => "32596-04g.html",
                    }
                    .to_owned(),
                );
            }
            "end_quest" => {
                if !ctx.has_quest_items(player, BOOK2) {
                    return Some(ctx.get_no_quest_msg(player));
                }

                let i = (qs.get_cond(ctx) - 1) as usize;
                // Java indexes the table with the cond; an out of range cond is a Java exception.
                let (_, exp, sp, adena) = *NPC.get(i)?;
                ctx.add_exp_and_sp(player, exp, sp);
                ctx.give_adena(player, adena, true);
                qs.exit_quest_type(ctx, QuestType::Daily, true);
                htmltext = Some(format!("{}-02.html", npc?.id));
            }
            "32596-02.html" | "32596-03.html" => {}
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_item_talk(&self, ctx: &mut ScriptCtx<'_>, _item: ScriptItem, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let mut start_quest = false;
        match qs.state(ctx) {
            CREATED => start_quest = true,
            STARTED => htmltext = "strongbox-02.html".to_owned(),
            COMPLETED => {
                if qs.is_now_available(ctx) {
                    qs.set_state(ctx, CREATED);
                    start_quest = true;
                } else {
                    htmltext = "strongbox-03.html".to_owned();
                }
            }
            _ => {}
        }

        if start_quest {
            if ctx.player_level(player) >= MIN_LEVEL {
                qs.start_quest(ctx);
                ctx.take_items(player, STRONGBOX, 1);
                ctx.give_items(player, BOOK, 1);
                htmltext = "strongbox-01.htm".to_owned();
            } else {
                htmltext = "strongbox-00.htm".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let chance = MOBS.iter().find(|m| m.0 == npc.id).map_or(0, |m| m.1);
        if ctx.get_random(1000) < chance {
            ctx.drop_item(npc, Some(killer), STRONGBOX, 1);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_started(ctx) {
            let npc_id = npc.id;
            if npc_id == NPC[0].0 {
                let page = match qs.get_cond(ctx) {
                    1 => Some("32596-01.html"),
                    2 => Some("32596-05.html"),
                    3 => Some("32596-05a.html"),
                    4 => Some("32596-05b.html"),
                    5 => Some("32596-05c.html"),
                    6 => Some("32596-05d.html"),
                    7 => Some("32596-05e.html"),
                    8 => Some("32596-05f.html"),
                    9 => Some("32596-05g.html"),
                    _ => None,
                };
                if let Some(page) = page {
                    htmltext = page.to_owned();
                }
            } else if qs.get_cond(ctx) > 1 && qs.get_int(ctx, "npc") == npc_id {
                htmltext = format!("{npc_id}-01.html");
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

    const NAME: &str = "Q00464_Oath";

    const NPCS: &str = r#"
        <npc id="32596" type="Folk" level="70" name="Wizard"/>
        <npc id="30657" type="Folk" level="70" name="Kristin"/>
        <npc id="22789" type="Monster" level="80" name="Guide Solina"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15537" type="EtcItem" name="Strongbox"/>
        <item id="15538" type="EtcItem" name="Book"><set name="is_questitem" val="true"/></item>
        <item id="15539" type="EtcItem" name="Book 2"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00464Oath);
        let pages = Pages::of(&Q00464Oath)
            .page("32596-01.html", &["32596-02.html"])
            .page("32596-02.html", &["32596-03.html"])
            .page("32596-03.html", &["32596-04.html"])
            .plain(&["32596-04.html", "32596-04a.html", "32596-04b.html", "32596-04c.html"])
            .plain(&["32596-04d.html", "32596-04e.html", "32596-04f.html", "32596-04g.html"])
            .page("30657-01.html", &["end_quest"])
            .plain(&["30657-02.html"])
            .npcs(&[32596, 30657]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let wizard = t.spawn(32596, 20);
        (t, wizard)
    }

    fn use_strongbox(t: &mut ScriptTest) -> Option<String> {
        let mut text = None;
        t.world.run_as_script(NAME, |ctx| {
            text = Q00464Oath.on_item_talk(
                ctx,
                ScriptItem {
                    object_id: Oid(5000),
                    id: STRONGBOX,
                },
                Oid(1),
            );
        });
        text
    }

    #[test]
    fn a_level_81_character_cannot_open_the_strongbox() {
        let (mut t, _) = setup();
        t.enter(1, 81, vec![]);
        assert_eq!(use_strongbox(&mut t), Some("strongbox-00.htm".to_owned()));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_strongbox_starts_the_quest_and_the_errand_ends_daily() {
        let (mut t, wizard) = setup();
        t.enter(1, 82, vec![crate::testing::item(5000, STRONGBOX, 1)]);
        assert_eq!(use_strongbox(&mut t), Some("strongbox-01.htm".to_owned()));
        assert_eq!((t.count(1, STRONGBOX), t.count(1, BOOK)), (0, 1));
        assert_eq!(
            t.take_writes(),
            [rows::insert(NAME, "cond", "1"), rows::insert(NAME, "<state>", "Started")]
        );
        // A second use only tells the quest runs.
        assert_eq!(use_strongbox(&mut t), Some("strongbox-02.html".to_owned()));

        // The wizard: the first page, the questions, the errand.
        assert!(t.quest_page(1, wizard).contains("32596-01.html"));
        t.event(1, NAME, "32596-02.html");
        t.event(1, NAME, "32596-03.html");
        let page = t.event(1, NAME, "32596-04.html");
        assert!(page.contains("32596-04"), "{page}");
        assert_eq!((t.count(1, BOOK), t.count(1, BOOK2)), (0, 1));
        let writes = t.take_writes();
        // The chosen NPC is kept in "npc"; the cond moves to 2..9.
        assert!(
            writes
                .iter()
                .any(|w| matches!(w, l2_world::QuestWrite::InsertVar { var, .. } if var == "npc")),
            "{writes:?}"
        );
        let cond = {
            let mut c = 0;
            t.world.run_as_script(NAME, |ctx| {
                c = ctx.get_quest_state(Oid(1), false).unwrap().get_cond(ctx);
            });
            c
        };
        assert!((2..=9).contains(&cond), "{cond}");

        // Hand-in at the chosen NPC only (here Kristin's id is forced).
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set(ctx, "npc", "30657");
            qs.set_cond(ctx, 2, false);
        });
        let kristin = t.spawn(30657, 30);
        assert!(t.quest_page(1, kristin).contains("30657-01.html"));
        t.take_writes();
        // NPC[1] is Kristin: 15449 exp, 17696 sp, 42910 adena (rates 1 in the test world).
        assert!(t.event(1, NAME, "end_quest").contains("30657-02.html"));
        assert!(t.count(1, 57) >= 42910);
        // The daily exit removes the registered quest items.
        assert_eq!(t.count(1, BOOK2), 0);
        let writes = t.take_writes();
        assert!(
            writes
                .iter()
                .any(|w| matches!(w, l2_world::QuestWrite::InsertVar { var, .. } if var == "restartTime")),
            "{writes:?}"
        );
    }

    #[test]
    fn a_solina_sometimes_drops_the_strongbox() {
        let (mut t, _) = setup();
        t.enter(1, 82, vec![]);
        // 4 to 10 in 1000 per kill: a few hundred kills find one on the ground.
        for _ in 0..2000 {
            let solina = t.spawn(22789, 60);
            t.kill(1, solina);
            let dropped = t
                .world
                .ground_item_ids()
                .into_iter()
                .any(|id| t.world.ground_item(id).is_some_and(|g| g.data.item_id == STRONGBOX));
            if dropped {
                // Dropped on the ground, not into the bag.
                assert_eq!(t.count(1, STRONGBOX), 0);
                return;
            }
        }
        panic!("no strongbox in 2000 kills");
    }
}
