//! Quest 136: More Than Meets the Eye (Hardin, Errickin and Clayton; ghosts' ectoplasm, jaguar crystals).
// Java: dist/game/data/scripts/quests/Q00136_MoreThanMeetsTheEye/Q00136_MoreThanMeetsTheEye.java
//
// Pattern: kills of the killer's own quest (no party), an item stack that moves the cond at its
// limit (`give_item` below is Java's private `giveItem`), and a "talked" variable on Hardin.

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Qs, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const HARDIN: i32 = 30832;
const ERRICKIN: i32 = 30701;
const CLAYTON: i32 = 30464;

// Monsters
const GLASS_JAGUAR: i32 = 20250;
const GHOST1: i32 = 20636;
const GHOST2: i32 = 20637;
const GHOST3: i32 = 20638;
const MIRROR: i32 = 20639;

// Items
const ECTOPLASM: i32 = 9787;
const STABILIZED_ECTOPLASM: i32 = 9786;
const ORDER: i32 = 9788;
const GLASS_JAGUAR_CRYSTAL: i32 = 9789;
const BOOK_OF_SEAL: i32 = 9790;
const TRANSFORM_BOOK: i32 = 9648;

// Misc
const MIN_LEVEL: i32 = 50;
const ECTOPLASM_COUNT: i64 = 35;
const CRYSTAL_COUNT: i64 = 5;
const CHANCES: [i32; 4] = [0, 40, 90, 290];

/// Java `giveItem(qs, itemId, count, maxCount, cond)`.
fn give_item(ctx: &mut ScriptCtx<'_>, qs: Qs, item_id: i32, count: i64, max_count: i64, cond: i32) {
    let player = qs.player();
    ctx.give_items(player, item_id, count);
    if ctx.get_quest_items_count(player, item_id) >= max_count {
        qs.set_cond(ctx, cond, true);
    } else {
        ctx.play_sound(player, QuestSound::ItemsoundQuestItemget);
    }
}

/// Java `Q00136_MoreThanMeetsTheEye`.
pub struct Q00136MoreThanMeetsTheEye;

impl Script for Q00136MoreThanMeetsTheEye {
    fn name(&self) -> &'static str {
        "Q00136_MoreThanMeetsTheEye"
    }

    fn path(&self) -> &'static str {
        "quests/Q00136_MoreThanMeetsTheEye"
    }

    fn id(&self) -> i32 {
        136
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[HARDIN]);
        s.add_talk_id(&[HARDIN, ERRICKIN, CLAYTON]);
        s.add_kill_id(&[GHOST1, GHOST2, GHOST3, GLASS_JAGUAR, MIRROR]);
        s.register_quest_items(&[
            ECTOPLASM,
            STABILIZED_ECTOPLASM,
            ORDER,
            GLASS_JAGUAR_CRYSTAL,
            BOOK_OF_SEAL,
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

        let mut htmltext = Some(event.to_owned());
        match event {
            "30832-05.html" | "30832-06.html" | "30832-12.html" | "30832-13.html" | "30832-18.html" => {}
            "30832-03.htm" => qs.start_quest(ctx),
            "30832-07.html" => qs.set_cond(ctx, 2, true),
            "30832-11.html" => qs.set(ctx, "talked", "2"),
            "30832-14.html" => {
                qs.unset(ctx, "talked");
                ctx.give_items(player, ORDER, 1);
                qs.set_cond(ctx, 6, true);
            }
            "30832-17.html" => qs.set(ctx, "talked", "2"),
            "30832-19.html" => {
                ctx.give_items(player, TRANSFORM_BOOK, 1);
                ctx.give_adena(player, 67_550, true);
                qs.exit_quest(ctx, false, true);
            }
            "30701-03.html" => qs.set_cond(ctx, 3, true),
            "30464-03.html" => {
                ctx.take_items(player, ORDER, -1);
                qs.set_cond(ctx, 7, true);
            }
            _ => htmltext = None,
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };

        let npc_id = npc.id;
        if npc_id != GLASS_JAGUAR && qs.is_cond(ctx, 3) {
            let count =
                if npc_id == MIRROR && ctx.get_quest_items_count(killer, ECTOPLASM) + 2 < ECTOPLASM_COUNT {
                    2
                } else {
                    1
                };
            let index = (npc_id - GHOST1) as usize;
            // NOTE(java-quirk): a hit of the extra chance gives one more ectoplasm before the
            // regular `giveItem`, so a kill can give up to `count + 1`.
            if ctx.get_random(1000) < CHANCES[index]
                && ctx.get_quest_items_count(killer, ECTOPLASM) + count < ECTOPLASM_COUNT
            {
                ctx.give_items(killer, ECTOPLASM, 1);
            }

            give_item(ctx, qs, ECTOPLASM, count, ECTOPLASM_COUNT, 4);
        } else if npc_id == GLASS_JAGUAR && qs.is_cond(ctx, 7) {
            give_item(ctx, qs, GLASS_JAGUAR_CRYSTAL, 1, CRYSTAL_COUNT, 8);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            HARDIN => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30832-01.htm"
                    } else {
                        "30832-02.htm"
                    }
                    .to_owned();
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => htmltext = "30832-04.html".to_owned(),
                    2..=4 => htmltext = "30832-08.html".to_owned(),
                    5 => {
                        if qs.get_int(ctx, "talked") == 1 {
                            htmltext = "30832-10.html".to_owned();
                        } else if qs.get_int(ctx, "talked") == 2 {
                            htmltext = "30832-12.html".to_owned();
                        } else if ctx.has_quest_items(player, STABILIZED_ECTOPLASM) {
                            ctx.take_items(player, STABILIZED_ECTOPLASM, -1);
                            qs.set(ctx, "talked", "1");
                            htmltext = "30832-09.html".to_owned();
                        } else {
                            htmltext = "30832-08.html".to_owned();
                        }
                    }
                    6..=8 => htmltext = "30832-15.html".to_owned(),
                    9 => {
                        if qs.get_int(ctx, "talked") == 1 {
                            qs.set(ctx, "talked", "2");
                            htmltext = "30832-17.html".to_owned();
                        } else if qs.get_int(ctx, "talked") == 2 {
                            htmltext = "30832-18.html".to_owned();
                        } else {
                            ctx.take_items(player, BOOK_OF_SEAL, -1);
                            qs.set(ctx, "talked", "1");
                            htmltext = "30832-16.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            ERRICKIN => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1 => htmltext = "30701-01.html".to_owned(),
                        2 => htmltext = "30701-02.html".to_owned(),
                        3 => htmltext = "30701-04.html".to_owned(),
                        4 => {
                            if ctx.get_quest_items_count(player, ECTOPLASM) < ECTOPLASM_COUNT {
                                ctx.give_items(player, STABILIZED_ECTOPLASM, 1);
                                qs.set_cond(ctx, 5, true);
                                htmltext = "30701-06.html".to_owned();
                            } else {
                                ctx.take_items(player, ECTOPLASM, -1);
                                htmltext = "30701-05.html".to_owned();
                            }
                        }
                        _ => htmltext = "30701-07.html".to_owned(),
                    }
                }
            }
            CLAYTON => {
                if qs.is_started(ctx) {
                    match qs.get_cond(ctx) {
                        1..=5 => htmltext = "30464-01.html".to_owned(),
                        6 => htmltext = "30464-02.html".to_owned(),
                        7 => htmltext = "30464-04.html".to_owned(),
                        8 => {
                            ctx.give_items(player, BOOK_OF_SEAL, 1);
                            ctx.take_items(player, GLASS_JAGUAR_CRYSTAL, -1);
                            qs.set_cond(ctx, 9, true);
                            htmltext = "30464-05.html".to_owned();
                        }
                        _ => htmltext = "30464-06.html".to_owned(),
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
    use crate::testing::rows::{delete, insert, update};
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="30832" type="Folk" level="50" name="Hardin"/>
        <npc id="30701" type="Folk" level="50" name="Errickin"/>
        <npc id="30464" type="Folk" level="50" name="Clayton"/>
        <npc id="20639" type="Monster" level="50" name="Mirror"/>
        <npc id="20250" type="Monster" level="50" name="Glass Jaguar"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="9787" type="EtcItem" name="Ectoplasm"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9786" type="EtcItem" name="Stabilized Ectoplasm"><set name="is_questitem" val="true"/></item>
        <item id="9788" type="EtcItem" name="Order"><set name="is_questitem" val="true"/></item>
        <item id="9789" type="EtcItem" name="Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="9790" type="EtcItem" name="Book of Seal"><set name="is_questitem" val="true"/></item>
        <item id="9648" type="EtcItem" name="Transform Book"/>"#;

    const NAME: &str = "Q00136_MoreThanMeetsTheEye";

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00136MoreThanMeetsTheEye);
        let pages = Pages::of(&Q00136MoreThanMeetsTheEye)
            .page("30832-01.htm", &["30832-03.htm"])
            .page("30832-03.htm", &["30832-05.html"])
            .page("30832-05.html", &["30832-06.html"])
            .page("30832-06.html", &["30832-07.html"])
            .plain(&["30832-02.htm", "30832-04.html", "30832-07.html", "30832-08.html"])
            .page("30832-09.html", &["30832-11.html"])
            .page("30832-11.html", &["30832-12.html"])
            .page("30832-12.html", &["30832-13.html"])
            .page("30832-13.html", &["30832-14.html"])
            .plain(&["30832-14.html", "30832-15.html"])
            .page("30832-16.html", &["30832-17.html"])
            .page("30832-17.html", &["30832-18.html"])
            .page("30832-18.html", &["30832-19.html"])
            .plain(&["30832-19.html"])
            .plain(&["30701-01.html", "30701-02.html", "30701-05.html", "30701-06.html"])
            .page("30701-04.html", &["30701-03.html"])
            .plain(&["30701-03.html", "30701-07.html"])
            .plain(&["30464-01.html"])
            .page("30464-02.html", &["30464-03.html"])
            .plain(&["30464-03.html", "30464-04.html", "30464-05.html", "30464-06.html"])
            .npcs(&[HARDIN, ERRICKIN, CLAYTON])
            .file("data/html/noquest.htm", "<html><body>noquest</body></html>");
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let hardin = t.spawn(HARDIN, 20);
        let errickin = t.spawn(ERRICKIN, 30);
        let clayton = t.spawn(CLAYTON, 40);
        (t, hardin, errickin, clayton)
    }

    #[test]
    fn hardin_gates_level_50() {
        let (mut t, hardin, _, _) = setup();
        t.enter(1, 49, vec![]);
        assert!(t.quest_page(1, hardin).contains("30832-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_quest_to_the_transform_book() {
        let (mut t, hardin, errickin, clayton) = setup();
        t.enter(1, 50, vec![]);
        assert!(t.quest_page(1, hardin).contains("30832-01.htm"));
        assert!(t.event(1, NAME, "30832-03.htm").contains("30832-03.htm"));
        assert_eq!(
            t.take_writes(),
            [insert(NAME, "cond", "1"), insert(NAME, "<state>", "Started")]
        );
        assert!(t.event(1, NAME, "30832-05.html").contains("30832-05.html"));
        assert!(t.event(1, NAME, "30832-06.html").contains("30832-06.html"));
        assert!(t.event(1, NAME, "30832-07.html").contains("30832-07.html"));
        assert_eq!(t.take_writes(), [update(NAME, "cond", "2")]);

        // Errickin: cond 3.
        assert!(t.quest_page(1, errickin).contains("30701-02.html"));
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), false).unwrap();
            qs.set_cond(ctx, 3, false);
        });
        t.take_writes();

        // Mirrors (count 2 each) and the extra chance fill 35 ectoplasm: cond 4.
        for _ in 0..200 {
            if t.world.player_quests(Oid(1)).is_some() && t.count(1, ECTOPLASM) >= 35 {
                break;
            }
            let mob = t.spawn(MIRROR, 80);
            t.kill(1, mob);
        }
        assert!(t.count(1, ECTOPLASM) >= 35);
        assert!(t.take_writes().contains(&update(NAME, "cond", "4")));

        // Errickin takes them: with 35 or more he takes all and answers 05 (no stabilized one).
        assert!(t.quest_page(1, errickin).contains("30701-05.html"));
        assert_eq!(t.count(1, ECTOPLASM), 0);
        // NOTE(java-quirk): with fewer than 35 he hands out the stabilized ectoplasm instead.
        t.world
            .run_as_script(NAME, |ctx| ctx.give_items(Oid(1), ECTOPLASM, 3));
        assert!(t.quest_page(1, errickin).contains("30701-06.html"));
        assert_eq!(t.count(1, STABILIZED_ECTOPLASM), 1);
        assert_eq!(t.take_writes().last(), Some(&update(NAME, "cond", "5")));

        // Hardin: takes the ectoplasm, "talked" 1 then 2, the order (cond 6).
        assert!(t.quest_page(1, hardin).contains("30832-09.html"));
        assert_eq!(t.count(1, STABILIZED_ECTOPLASM), 0);
        for page in ["30832-11.html", "30832-12.html", "30832-13.html", "30832-14.html"] {
            assert!(t.event(1, NAME, page).contains(page), "{page}");
        }
        assert_eq!(t.count(1, ORDER), 1);
        assert!(t.take_writes().contains(&update(NAME, "cond", "6")));

        // Clayton takes the order (cond 7); five jaguars give cond 8; the book comes at cond 9.
        assert!(t.quest_page(1, clayton).contains("30464-02.html"));
        assert!(t.event(1, NAME, "30464-03.html").contains("30464-03.html"));
        assert_eq!(t.count(1, ORDER), 0);
        t.take_writes();
        for _ in 0..5 {
            let jaguar = t.spawn(GLASS_JAGUAR, 80);
            t.kill(1, jaguar);
        }
        assert_eq!(t.count(1, GLASS_JAGUAR_CRYSTAL), 5);
        assert!(t.take_writes().contains(&update(NAME, "cond", "8")));
        assert!(t.quest_page(1, clayton).contains("30464-05.html"));
        assert_eq!(
            (t.count(1, BOOK_OF_SEAL), t.count(1, GLASS_JAGUAR_CRYSTAL)),
            (1, 0)
        );
        t.take_writes();

        // Hardin's last pages and the reward.
        assert!(t.quest_page(1, hardin).contains("30832-16.html"));
        assert_eq!(t.count(1, BOOK_OF_SEAL), 0);
        for page in ["30832-17.html", "30832-18.html", "30832-19.html"] {
            assert!(t.event(1, NAME, page).contains(page), "{page}");
        }
        assert_eq!(t.count(1, TRANSFORM_BOOK), 1);
        assert_eq!(t.count(1, 57), 67_550);
        assert!(
            t.take_writes()
                .ends_with(&[delete(NAME, false), update(NAME, "<state>", "Completed")])
        );
    }
}
