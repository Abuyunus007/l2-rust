//! Quest 269: Invention Ambition (Inventor Maru in the Gludin area buys the energy ores).
// Java: dist/game/data/scripts/quests/Q00269_InventionAmbition/Q00269_InventionAmbition.java
//
// Pattern: a repeatable collect quest with a party drop (`docs/design/scripts.md`, "Quest
// patterns"): a random started party member gets an energy ore from the northern and kerope
// monsters with a drop chance; Maru pays per ore, with a bonus from 10 ores, and takes them all.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const INVENTOR_MARU: i32 = 32486;

// Item
const ENERGY_ORE: i32 = 10866;

// Misc
const MIN_LEVEL: i32 = 18;

/// Java `MONSTERS`: the monsters and their drop chance.
const MONSTERS: [(i32, f64); 8] = [
    (21124, 0.46), // Red Eye Barbed Bat
    (21125, 0.48), // Northern Trimden
    (21126, 0.5),  // Kerope Werewolf
    (21127, 0.64), // Northern Goblin
    (21128, 0.66), // Spine Golem
    (21129, 0.68), // Kerope Werewolf Chief
    (21130, 0.76), // Northern Goblin Leader
    (21131, 0.78), // Enchanted Spine Golem
];

/// Java `Q00269_InventionAmbition`.
pub struct Q00269InventionAmbition;

impl Script for Q00269InventionAmbition {
    fn name(&self) -> &'static str {
        "Q00269_InventionAmbition"
    }

    fn path(&self) -> &'static str {
        "quests/Q00269_InventionAmbition"
    }

    fn id(&self) -> i32 {
        269
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[INVENTOR_MARU]);
        s.add_talk_id(&[INVENTOR_MARU]);
        let ids: Vec<i32> = MONSTERS.iter().map(|&(id, _)| id).collect();
        s.add_kill_id(&ids);
        s.register_quest_items(&[ENERGY_ORE]);
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
            "32486-03.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    htmltext = Some(event.to_owned());
                }
            }
            "32486-04.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "32486-07.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "32486-08.html" => htmltext = Some(event.to_owned()),
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(member) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) {
            let chance = MONSTERS
                .iter()
                .find(|&&(id, _)| id == npc.id)
                .map_or(0.0, |&(_, chance)| chance);
            // Limit 0: no limit on the ores.
            ctx.give_item_randomly(member.player(), Some(npc), ENERGY_ORE, 1, 1, 0, chance, true);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.state(ctx) == CREATED {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "32486-01.htm"
            } else {
                "32486-02.html"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            if ctx.has_quest_items(player, ENERGY_ORE) {
                let count = ctx.get_quest_items_count(player, ENERGY_ORE);
                let bonus = if count >= 10 { 2044 } else { 0 };
                ctx.give_adena(player, count * 50 + bonus, true);
                ctx.take_items(player, ENERGY_ORE, -1);
                htmltext = "32486-06.html".to_owned();
            } else {
                htmltext = "32486-05.html".to_owned();
            }
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00269_InventionAmbition";

    const NPCS: &str = r#"
        <npc id="32486" type="Folk" level="70" name="Inventor Maru"/>
        <npc id="21124" type="Monster" level="30" name="Red Eye Barbed Bat"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10866" type="EtcItem" name="Energy Ore"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00269InventionAmbition);
        let pages = Pages::of(&Q00269InventionAmbition)
            .page("32486-01.htm", &["32486-03.htm"])
            .page("32486-03.htm", &["32486-04.htm"])
            .page("32486-05.html", &["32486-07.html", "32486-08.html"])
            .page("32486-06.html", &["32486-07.html", "32486-08.html"])
            .plain(&["32486-02.html", "32486-04.htm", "32486-07.html", "32486-08.html"])
            .npcs(&[INVENTOR_MARU]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let maru = t.spawn(INVENTOR_MARU, 20);
        (t, maru)
    }

    #[test]
    fn a_level_seventeen_character_is_sent_away() {
        let (mut t, maru) = setup();
        t.enter_as(1, 17, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, maru).contains("32486-02.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn ores_for_adena_with_the_bonus_from_ten_ores() {
        let (mut t, maru) = setup();
        t.enter_as(1, 18, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, maru).contains("32486-01.htm"));
        assert!(t.event(1, NAME, "32486-03.htm").contains("32486-03.htm"));
        assert!(t.event(1, NAME, "32486-04.htm").contains("32486-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // No ore yet.
        assert!(t.quest_page(1, maru).contains("32486-05.html"));

        // Ores come with a chance per kill; no condition is set.
        for _ in 0..300 {
            if t.count(1, ENERGY_ORE) >= 10 {
                break;
            }
            let bat = t.spawn(21124, 60);
            t.kill(1, bat);
        }
        let ores = t.count(1, ENERGY_ORE);
        assert!(ores >= 10, "{ores}");
        assert!(t.take_writes().is_empty());

        // Maru pays 50 per ore and 2044 more from 10 ores; the ores go.
        assert!(t.quest_page(1, maru).contains("32486-06.html"));
        assert_eq!(t.count(1, 57), ores * 50 + 2044);
        assert_eq!(t.count(1, ENERGY_ORE), 0);
        assert!(t.take_writes().is_empty());

        // `exitQuest(true, true)` from the last page.
        assert!(t.event(1, NAME, "32486-07.html").contains("32486-07.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
