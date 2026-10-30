//! Quest 357: Warehouse Keeper's Ambition (Warehouse Keeper Silva, Jades in the Enchanted Valley).
// Java: dist/game/data/scripts/quests/Q00357_WarehouseKeepersAmbition/Q00357_WarehouseKeepersAmbition.java
//
// Pattern: a repeatable kill-count quest with a drop chance (`docs/design/scripts.md`, "Quest
// patterns"): the jades drop with no limit to a random started party member, the NPC pays for
// them (a bonus from 100 jades) and the player chooses to go on or to quit.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const SILVA: i32 = 30686;

// Item
const JADE_CRYSTAL: i32 = 5867;

// Monsters (drop chance)
const DROP_DATA: [(i32, f64); 4] = [
    (20594, 0.577), // Forest Runner
    (20595, 0.6),   // Fline Elder
    (20596, 0.638), // Liele Elder
    (20597, 0.062), // Valley Treant Elder
];

// Misc
const MIN_LEVEL: i32 = 47;

/// Java `Q00357_WarehouseKeepersAmbition`.
pub struct Q00357WarehouseKeepersAmbition;

impl Script for Q00357WarehouseKeepersAmbition {
    fn name(&self) -> &'static str {
        "Q00357_WarehouseKeepersAmbition"
    }

    fn path(&self) -> &'static str {
        "quests/Q00357_WarehouseKeepersAmbition"
    }

    fn id(&self) -> i32 {
        357
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SILVA]);
        s.add_talk_id(&[SILVA]);
        let mobs: Vec<i32> = DROP_DATA.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[JADE_CRYSTAL]);
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
        let mut htmltext = None;
        if let Some(qs) = qs {
            match event {
                "30686-01.htm" | "30686-03.htm" | "30686-04.htm" | "30686-10.html" => {
                    htmltext = Some(event.to_owned());
                }
                "30686-05.htm" => {
                    if qs.is_created(ctx) {
                        qs.start_quest(ctx);
                        htmltext = Some(event.to_owned());
                    }
                }
                "30686-09.html" => {
                    let crystal_count = ctx.get_quest_items_count(player, JADE_CRYSTAL);
                    if crystal_count > 0 {
                        let mut adena_reward = crystal_count * 425;
                        if crystal_count < 100 {
                            adena_reward += 13500;
                            htmltext = Some("30686-08.html".to_owned());
                        } else {
                            adena_reward += 40500;
                            htmltext = Some(event.to_owned());
                        }
                        ctx.give_adena(player, adena_reward, true);
                        ctx.take_items(player, JADE_CRYSTAL, -1);
                    }
                }
                "30686-11.html" => {
                    let crystal_count = ctx.get_quest_items_count(player, JADE_CRYSTAL);
                    if crystal_count > 0 {
                        // NOTE(java-quirk): quitting pays the 40500 bonus from 100 jades, but not
                        // the 13500 that "Receive reward" adds below 100.
                        let bonus = if crystal_count >= 100 { 40500 } else { 0 };
                        ctx.give_adena(player, crystal_count * 425 + bonus, true);
                        ctx.take_items(player, JADE_CRYSTAL, -1);
                    }
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
                _ => {}
            }
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut htmltext = ctx.get_no_quest_msg(talker);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(talker) < MIN_LEVEL {
                "30686-01.html"
            } else {
                "30686-02.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            htmltext = if ctx.has_quest_items(talker, JADE_CRYSTAL) {
                "30686-07.html"
            } else {
                "30686-06.html"
            }
            .to_owned();
        }
        Some(htmltext)
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(killer, -1, 3, Some(npc));
        if let Some(qs) = qs {
            let chance = DROP_DATA
                .iter()
                .find(|(id, _)| *id == npc.id)
                .map_or(0.0, |(_, c)| *c);
            ctx.give_item_randomly(qs.player(), Some(npc), JADE_CRYSTAL, 1, 1, 0, chance, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00357_WarehouseKeepersAmbition";

    const NPCS: &str = r#"
        <npc id="30686" type="Folk" level="70" name="Warehouse Keeper Silva"/>
        <npc id="20594" type="Monster" level="50" name="Forest Runner"/>
        <npc id="20595" type="Monster" level="50" name="Fline Elder"/>
        <npc id="20596" type="Monster" level="50" name="Liele Elder"/>
        <npc id="20597" type="Monster" level="50" name="Valley Treant Elder"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5867" type="EtcItem" name="Jade Crystal"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00357WarehouseKeepersAmbition);
        let pages = Pages::of(&Q00357WarehouseKeepersAmbition)
            .plain(&["30686-01.html"])
            .page("30686-02.htm", &["30686-03.htm"])
            .page("30686-03.htm", &["30686-04.htm"])
            .page("30686-04.htm", &["30686-05.htm"])
            .plain(&["30686-05.htm"])
            .page("30686-06.html", &["30686-10.html", "30686-11.html"])
            .page(
                "30686-07.html",
                &["30686-09.html", "30686-10.html", "30686-11.html"],
            )
            .page("30686-08.html", &["30686-10.html", "30686-11.html"])
            .page("30686-09.html", &["30686-10.html", "30686-11.html"])
            .plain(&["30686-10.html"])
            .page("30686-11.html", &[])
            .npcs(&[SILVA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let silva = t.spawn(SILVA, 20);
        (t, silva)
    }

    /// Kills the valley monsters (the three common ones) until the player has `jades` jades.
    fn hunt(t: &mut ScriptTest, jades: i64) {
        for kills in 0..800 {
            if t.count(1, JADE_CRYSTAL) >= jades {
                return;
            }
            let mob = t.spawn(DROP_DATA[kills % 3].0, 60);
            t.kill(1, mob);
        }
        panic!("no {jades} jades after 800 kills");
    }

    #[test]
    fn a_level_46_character_is_sent_away() {
        let (mut t, silva) = setup();
        t.enter(1, 46, vec![]);
        assert!(t.quest_page(1, silva).contains("30686-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_few_jades_get_the_small_bonus_and_quitting_ends_the_quest() {
        let (mut t, silva) = setup();
        t.enter(1, 47, vec![]);
        assert!(t.quest_page(1, silva).contains("30686-02.htm"));
        assert!(t.event(1, NAME, "30686-03.htm").contains("30686-03.htm"));
        assert!(t.event(1, NAME, "30686-04.htm").contains("30686-04.htm"));
        assert!(t.event(1, NAME, "30686-05.htm").contains("30686-05.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, silva).contains("30686-06.html"));

        hunt(&mut t, 3);
        let jades = t.count(1, JADE_CRYSTAL);
        assert!(jades < 100);
        assert!(t.quest_page(1, silva).contains("30686-07.html"));
        let adena = t.count(1, 57);
        assert!(t.event(1, NAME, "30686-09.html").contains("30686-08.html"));
        assert_eq!(t.count(1, 57), adena + jades * 425 + 13500);
        assert_eq!(t.count(1, JADE_CRYSTAL), 0);
        assert!(t.take_writes().is_empty());

        // "Say you want to quit": nothing left to pay, the quest ends repeatable.
        assert!(t.quest_page(1, silva).contains("30686-06.html"));
        assert!(t.event(1, NAME, "30686-11.html").contains("30686-11.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        // Level 47 again: the offer is back.
        assert!(t.quest_page(1, silva).contains("30686-02.htm"));
    }

    #[test]
    fn a_hundred_jades_get_the_big_bonus() {
        let (mut t, silva) = setup();
        t.enter(1, 47, vec![]);
        assert!(t.quest_page(1, silva).contains("30686-02.htm"));
        t.event(1, NAME, "30686-03.htm");
        t.event(1, NAME, "30686-04.htm");
        t.event(1, NAME, "30686-05.htm");
        t.take_writes();

        hunt(&mut t, 100);
        let jades = t.count(1, JADE_CRYSTAL);
        assert!(jades >= 100);
        assert!(t.quest_page(1, silva).contains("30686-07.html"));
        let adena = t.count(1, 57);
        assert!(t.event(1, NAME, "30686-09.html").contains("30686-09.html"));
        assert_eq!(t.count(1, 57), adena + jades * 425 + 40500);
        assert_eq!(t.count(1, JADE_CRYSTAL), 0);
        assert!(t.take_writes().is_empty());
    }
}
