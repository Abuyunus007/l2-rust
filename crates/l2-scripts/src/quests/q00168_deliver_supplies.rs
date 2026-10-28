//! Quest 168: Deliver Supplies (Jenna sends a Dark Elf to Harant for three sentry blades).
// Java: dist/game/data/scripts/quests/Q00168_DeliverSupplies/Q00168_DeliverSupplies.java
//
// Pattern: a delivery chain over several NPCs. Harant swaps Jenna's letter for three sentry
// blades; Jenna takes one blade (cond 3); Kristin and Roselyn each take their blade for an old
// bronze sword (cond 4 with two swords); Jenna pays and ends the quest (one-time). Dark Elves
// only (race gate).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const JENNA: i32 = 30349;
const ROSELYN: i32 = 30355;
const KRISTIN: i32 = 30357;
const HARANT: i32 = 30360;

// Items
const JENNAS_LETTER: i32 = 1153;
const SENTRY_BLADE1: i32 = 1154;
const SENTRY_BLADE2: i32 = 1155;
const SENTRY_BLADE3: i32 = 1156;
const OLD_BRONZE_SWORD: i32 = 1157;

// Misc
const MIN_LEVEL: i32 = 3;

/// Java `SENTRIES`: the blade each sentry takes.
fn sentry_blade_of(npc_id: i32) -> Option<i32> {
    match npc_id {
        KRISTIN => Some(SENTRY_BLADE3),
        ROSELYN => Some(SENTRY_BLADE2),
        _ => None,
    }
}

/// Java `Q00168_DeliverSupplies`.
pub struct Q00168DeliverSupplies;

impl Script for Q00168DeliverSupplies {
    fn name(&self) -> &'static str {
        "Q00168_DeliverSupplies"
    }

    fn path(&self) -> &'static str {
        "quests/Q00168_DeliverSupplies"
    }

    fn id(&self) -> i32 {
        168
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JENNA]);
        s.add_talk_id(&[JENNA, ROSELYN, KRISTIN, HARANT]);
        s.register_quest_items(&[
            JENNAS_LETTER,
            SENTRY_BLADE1,
            SENTRY_BLADE2,
            SENTRY_BLADE3,
            OLD_BRONZE_SWORD,
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
        let qs = ctx.get_quest_state(player, false);
        if let Some(qs) = qs
            && event == "30349-03.htm"
        {
            qs.start_quest(ctx);
            ctx.give_items(player, JENNAS_LETTER, 1);
            return Some(event.to_owned());
        }
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            JENNA => match qs.state(ctx) {
                CREATED => {
                    htmltext = if ctx.player_race(player) == race::DARK_ELF {
                        if ctx.player_level(player) >= MIN_LEVEL {
                            "30349-02.htm".to_owned()
                        } else {
                            "30349-01.htm".to_owned()
                        }
                    } else {
                        "30349-00.htm".to_owned()
                    };
                }
                STARTED => match qs.get_cond(ctx) {
                    1 => {
                        if ctx.has_quest_items(player, JENNAS_LETTER) {
                            htmltext = "30349-04.html".to_owned();
                        }
                    }
                    2 => {
                        if ctx.has_all_quest_items(player, &[SENTRY_BLADE1, SENTRY_BLADE2, SENTRY_BLADE3]) {
                            ctx.take_items(player, SENTRY_BLADE1, -1);
                            qs.set_cond(ctx, 3, true);
                            htmltext = "30349-05.html".to_owned();
                        }
                    }
                    3 => {
                        if ctx.has_at_least_one_quest_item(player, &[SENTRY_BLADE2, SENTRY_BLADE3]) {
                            htmltext = "30349-07.html".to_owned();
                        }
                    }
                    4 => {
                        if ctx.get_quest_items_count(player, OLD_BRONZE_SWORD) >= 2 {
                            ctx.give_adena(player, 820, true);
                            qs.exit_quest(ctx, false, true);
                            htmltext = "30349-07.html".to_owned();
                        }
                    }
                    _ => {}
                },
                COMPLETED => htmltext = ctx.get_already_completed_msg(player),
                _ => {}
            },
            HARANT => {
                if qs.is_cond(ctx, 1) && ctx.has_quest_items(player, JENNAS_LETTER) {
                    ctx.take_items(player, JENNAS_LETTER, -1);
                    ctx.give_items(player, SENTRY_BLADE1, 1);
                    ctx.give_items(player, SENTRY_BLADE2, 1);
                    ctx.give_items(player, SENTRY_BLADE3, 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = "30360-01.html".to_owned();
                } else if qs.is_cond(ctx, 2) {
                    htmltext = "30360-02.html".to_owned();
                }
            }
            ROSELYN | KRISTIN => {
                if let Some(blade) = sentry_blade_of(npc.id) {
                    if qs.is_cond(ctx, 3) && ctx.has_quest_items(player, blade) {
                        ctx.take_items(player, blade, -1);
                        ctx.give_items(player, OLD_BRONZE_SWORD, 1);
                        if ctx.get_quest_items_count(player, OLD_BRONZE_SWORD) >= 2 {
                            qs.set_cond(ctx, 4, true);
                        }
                        htmltext = format!("{}-01.html", npc.id);
                    } else if !ctx.has_quest_items(player, blade) && ctx.has_quest_items(player, OLD_BRONZE_SWORD) {
                        htmltext = format!("{}-02.html", npc.id);
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

    const NAME: &str = "Q00168_DeliverSupplies";
    const ADENA: i32 = 57;

    const NPCS: &str = r#"
        <npc id="30349" type="Folk" level="1" name="Jenna"/>
        <npc id="30355" type="Folk" level="1" name="Roselyn"/>
        <npc id="30357" type="Folk" level="1" name="Kristin"/>
        <npc id="30360" type="Folk" level="1" name="Harant"/>"#;

    const ITEMS: &str = r#"
        <item id="1153" type="EtcItem" name="Jenna's Letter"><set name="is_questitem" val="true"/></item>
        <item id="1154" type="EtcItem" name="Sentry Blade 1"><set name="is_questitem" val="true"/></item>
        <item id="1155" type="EtcItem" name="Sentry Blade 2"><set name="is_questitem" val="true"/></item>
        <item id="1156" type="EtcItem" name="Sentry Blade 3"><set name="is_questitem" val="true"/></item>
        <item id="1157" type="EtcItem" name="Old Bronze Sword"><set name="is_questitem" val="true"/></item>
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    struct Npcs {
        jenna: Oid,
        roselyn: Oid,
        kristin: Oid,
        harant: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00168DeliverSupplies);
        let pages = Pages::of(&Q00168DeliverSupplies)
            .page("30349-02.htm", &["30349-03.htm"])
            .plain(&[
                "30349-00.htm",
                "30349-01.htm",
                "30349-03.htm",
                "30349-04.html",
                "30349-05.html",
                "30349-07.html",
                "30355-01.html",
                "30355-02.html",
                "30357-01.html",
                "30357-02.html",
                "30360-01.html",
                "30360-02.html",
            ])
            .npcs(&[JENNA, ROSELYN, KRISTIN, HARANT]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npcs = Npcs {
            jenna: t.spawn(JENNA, 20),
            roselyn: t.spawn(ROSELYN, 40),
            kristin: t.spawn(KRISTIN, 60),
            harant: t.spawn(HARANT, 80),
        };
        (t, npcs)
    }

    #[test]
    fn only_a_dark_elf_of_level_3_starts_the_delivery() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(1, npcs.jenna).contains("30349-00.htm"));
        t.enter_as(2, 2, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(2, npcs.jenna).contains("30349-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_blades_the_swords_and_the_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 3, race::DARK_ELF, 0, vec![]);
        assert!(t.quest_page(1, npcs.jenna).contains("30349-02.htm"));
        assert!(t.event(1, NAME, "30349-03.htm").contains("30349-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, npcs.jenna).contains("30349-04.html"));

        // Harant swaps the letter for three blades: cond 2.
        assert!(t.quest_page(1, npcs.harant).contains("30360-01.html"));
        assert_eq!(t.count(1, JENNAS_LETTER), 0);
        assert_eq!(t.count(1, SENTRY_BLADE3), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Jenna takes one blade: cond 3.
        assert!(t.quest_page(1, npcs.jenna).contains("30349-05.html"));
        assert_eq!(t.count(1, SENTRY_BLADE1), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // Kristin takes blade 3 for a sword; the second talk has nothing to take.
        assert!(t.quest_page(1, npcs.kristin).contains("30357-01.html"));
        assert_eq!(t.count(1, OLD_BRONZE_SWORD), 1);
        assert!(t.quest_page(1, npcs.kristin).contains("30357-02.html"));
        assert!(t.take_writes().is_empty());

        // Roselyn takes blade 2: two swords, cond 4.
        assert!(t.quest_page(1, npcs.roselyn).contains("30355-01.html"));
        assert_eq!(t.count(1, OLD_BRONZE_SWORD), 2);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // Jenna pays 820 adena and ends the quest (`exitQuest(false, true)`).
        assert!(t.quest_page(1, npcs.jenna).contains("30349-07.html"));
        assert_eq!(t.count(1, ADENA), 820);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }
}
