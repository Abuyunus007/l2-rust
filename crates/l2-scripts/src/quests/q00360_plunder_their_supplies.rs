//! Quest 360: Plunder Their Supplies (Guard Coleman in the Cemetery, Taik Orc Seekers and Supply Leaders).
// Java: dist/game/data/scripts/quests/Q00360_PlunderTheirSupplies/Q00360_PlunderTheirSupplies.java
//
// Pattern: a one-time kill-count quest with drops (`docs/design/scripts.md`, "Quest patterns"):
// the supplies drop with a chance, a suspicious document piece drops on every kill with a 10%
// chance and four pieces become a recipe. The NPC pays for both (`exit_quest(false, true)`).

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Npc
const COLEMAN: i32 = 30873;

// Misc
const MIN_LEVEL: i32 = 52;

// Monsters (drop chance of the supplies, in percent)
const MONSTER_DROP_CHANCES: [(i32, i32); 2] = [
    (20666, 50), // Taik Orc Seeker
    (20669, 75), // Taik Orc Supply Leader
];

// Items
const RECIPE_OF_SUPPLY: i32 = 5870;
const SUPPLY_ITEMS: i32 = 5872;
const SUSPICIOUS_DOCUMENT_PIECE: i32 = 5871;

/// Java `Q00360_PlunderTheirSupplies`.
pub struct Q00360PlunderTheirSupplies;

impl Script for Q00360PlunderTheirSupplies {
    fn name(&self) -> &'static str {
        "Q00360_PlunderTheirSupplies"
    }

    fn path(&self) -> &'static str {
        "quests/Q00360_PlunderTheirSupplies"
    }

    fn id(&self) -> i32 {
        360
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[COLEMAN]);
        s.add_talk_id(&[COLEMAN]);
        let mobs: Vec<i32> = MONSTER_DROP_CHANCES.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[SUPPLY_ITEMS, SUSPICIOUS_DOCUMENT_PIECE, RECIPE_OF_SUPPLY]);
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
            "30873-03.htm" | "30873-09.html" => {
                htmltext = Some(event.to_owned());
            }
            "30873-04.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30873-10.html" => {
                qs.exit_quest(ctx, false, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.get_quest_state(killer, false).is_none()
            || !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, false)
        {
            return;
        }
        let chance = MONSTER_DROP_CHANCES
            .iter()
            .find(|(id, _)| *id == npc.id)
            .map_or(0, |(_, c)| *c);
        if ctx.get_random(100) < chance {
            ctx.give_items(killer, SUPPLY_ITEMS, 1);
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
        if ctx.get_random(100) < 10 {
            if ctx.get_quest_items_count(killer, SUSPICIOUS_DOCUMENT_PIECE) < 4 {
                ctx.give_items(killer, SUSPICIOUS_DOCUMENT_PIECE, 1);
            } else {
                ctx.give_items(killer, RECIPE_OF_SUPPLY, 1);
                ctx.take_items(killer, SUSPICIOUS_DOCUMENT_PIECE, -1);
            }
            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30873-02.htm"
            } else {
                "30873-01.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            let supply_count = ctx.get_quest_items_count(player, SUPPLY_ITEMS);
            let recipe_count = ctx.get_quest_items_count(player, RECIPE_OF_SUPPLY);
            if supply_count == 0 {
                if recipe_count == 0 {
                    htmltext = "30873-05.html".to_owned();
                } else {
                    ctx.give_adena(player, recipe_count * 6000, true);
                    ctx.take_items(player, RECIPE_OF_SUPPLY, -1);
                    htmltext = "30873-08.html".to_owned();
                }
            } else if recipe_count == 0 {
                ctx.give_adena(player, supply_count * 100 + 6000, true);
                ctx.take_items(player, SUPPLY_ITEMS, -1);
                htmltext = "30873-06.html".to_owned();
            } else {
                ctx.give_adena(player, (supply_count * 100 + 6000) + (recipe_count * 6000), true);
                ctx.take_items(player, SUPPLY_ITEMS, -1);
                ctx.take_items(player, RECIPE_OF_SUPPLY, -1);
                htmltext = "30873-07.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00360_PlunderTheirSupplies";

    const NPCS: &str = r#"
        <npc id="30873" type="Folk" level="70" name="Guard Coleman"/>
        <npc id="20666" type="Monster" level="50" name="Taik Orc Seeker"/>
        <npc id="20669" type="Monster" level="50" name="Taik Orc Supply Leader"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5870" type="EtcItem" name="Recipe of Supply"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5871" type="EtcItem" name="Suspicious Document Piece"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5872" type="EtcItem" name="Supply Items"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00360PlunderTheirSupplies);
        let pages = Pages::of(&Q00360PlunderTheirSupplies)
            .page("30873-02.htm", &["30873-03.htm"])
            .page("30873-03.htm", &["30873-04.htm"])
            .plain(&["30873-01.html", "30873-04.htm", "30873-05.html"])
            .page("30873-06.html", &["30873-09.html", "30873-10.html"])
            .page("30873-07.html", &["30873-09.html", "30873-10.html"])
            .page("30873-08.html", &["30873-09.html", "30873-10.html"])
            .plain(&["30873-09.html", "30873-10.html"])
            .npcs(&[COLEMAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let coleman = t.spawn(COLEMAN, 20);
        (t, coleman)
    }

    /// Accepts the quest: 02 -> 03 -> 04 (cond 1).
    fn accept(t: &mut ScriptTest, coleman: Oid) {
        assert!(t.quest_page(1, coleman).contains("30873-02.htm"));
        assert!(t.event(1, NAME, "30873-03.htm").contains("30873-03.htm"));
        assert!(t.event(1, NAME, "30873-04.htm").contains("30873-04.htm"));
        t.take_writes();
    }

    #[test]
    fn a_level_51_character_is_sent_away() {
        let (mut t, coleman) = setup();
        t.enter(1, 51, vec![]);
        assert!(t.quest_page(1, coleman).contains("30873-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn supplies_are_paid_at_100_each_and_the_quest_ends_once() {
        let (mut t, coleman) = setup();
        t.enter(1, 52, vec![item(100, SUPPLY_ITEMS, 3)]);
        assert!(t.quest_page(1, coleman).contains("30873-02.htm"));
        assert!(t.event(1, NAME, "30873-03.htm").contains("30873-03.htm"));
        assert!(t.event(1, NAME, "30873-04.htm").contains("30873-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Three supplies: 3 * 100 + 6000; the NPC's page offers "I want to quit".
        assert!(t.quest_page(1, coleman).contains("30873-06.html"));
        assert_eq!(t.count(1, 57), 6300);
        assert_eq!(t.count(1, SUPPLY_ITEMS), 0);
        assert!(t.take_writes().is_empty());

        assert!(t.event(1, NAME, "30873-10.html").contains("30873-10.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
    }

    #[test]
    fn a_recipe_alone_is_paid_at_6000() {
        let (mut t, coleman) = setup();
        t.enter(1, 52, vec![item(100, RECIPE_OF_SUPPLY, 1)]);
        accept(&mut t, coleman);
        assert!(t.quest_page(1, coleman).contains("30873-08.html"));
        assert_eq!(t.count(1, 57), 6000);
        assert_eq!(t.count(1, RECIPE_OF_SUPPLY), 0);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn supplies_and_a_recipe_are_paid_together() {
        let (mut t, coleman) = setup();
        t.enter(
            1,
            52,
            vec![item(100, SUPPLY_ITEMS, 2), item(101, RECIPE_OF_SUPPLY, 1)],
        );
        accept(&mut t, coleman);
        assert!(t.quest_page(1, coleman).contains("30873-07.html"));
        assert_eq!(t.count(1, 57), 2 * 100 + 6000 + 6000);
        assert_eq!((t.count(1, SUPPLY_ITEMS), t.count(1, RECIPE_OF_SUPPLY)), (0, 0));
        assert!(t.event(1, NAME, "30873-09.html").contains("30873-09.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_monsters_drop_supplies_and_documents() {
        let (mut t, coleman) = setup();
        t.enter(1, 52, vec![]);
        accept(&mut t, coleman);
        // Supplies drop at 50% and 75%; documents at 10% (four make a recipe).
        for kills in 0..300 {
            let mob = t.spawn(if kills % 2 == 0 { 20666 } else { 20669 }, 60);
            t.kill(1, mob);
            if t.count(1, SUPPLY_ITEMS) > 0 && t.count(1, SUSPICIOUS_DOCUMENT_PIECE) > 0 {
                break;
            }
        }
        assert!(t.count(1, SUPPLY_ITEMS) > 0);
        assert!(t.count(1, SUSPICIOUS_DOCUMENT_PIECE) > 0);
        // A kill never writes a quest row.
        assert!(t.take_writes().is_empty());
    }
}
