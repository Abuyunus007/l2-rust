//! Quest 354: Conquest of Alligator Island (Warehouse Freightman Kluck, the monsters of Alligator Island).
// Java: dist/game/data/scripts/quests/Q00354_ConquestOfAlligatorIsland/Q00354_ConquestOfAlligatorIsland.java
//
// Pattern: a repeatable kill-count quest with drop chances (`docs/design/scripts.md`, "Quest
// patterns"): the teeth are paid with a bonus from 100 teeth (`ADENA` bypass), a mysterious map
// piece drops on every kill with a 10% chance and ten pieces become the pirate's treasure map
// (`REWARD` bypass). The player chooses to go on or to quit.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KLUCK: i32 = 30895;

// Items
const ALLIGATOR_TOOTH: i32 = 5863;
const MYSTERIOUS_MAP_PIECE: i32 = 5864;
const PIRATES_TREASURE_MAP: i32 = 5915;

// Misc
const MIN_LEVEL: i32 = 38;

// Mobs (drop chance of one tooth)
const MOB1: [(i32, f64); 4] = [
    (20804, 0.84), // crokian_lad
    (20805, 0.91), // dailaon_lad
    (20806, 0.88), // crokian_lad_warrior
    (20807, 0.92), // farhite_lad
];

// Mobs (chance in percent of two teeth instead of one)
const MOB2: [(i32, i32); 2] = [
    (20808, 14), // nos_lad
    (20991, 69), // tribe_of_swamp
];

/// Java `Q00354_ConquestOfAlligatorIsland`.
pub struct Q00354ConquestOfAlligatorIsland;

impl Script for Q00354ConquestOfAlligatorIsland {
    fn name(&self) -> &'static str {
        "Q00354_ConquestOfAlligatorIsland"
    }

    fn path(&self) -> &'static str {
        "quests/Q00354_ConquestOfAlligatorIsland"
    }

    fn id(&self) -> i32 {
        354
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KLUCK]);
        s.add_talk_id(&[KLUCK]);
        let mob1: Vec<i32> = MOB1.iter().map(|(id, _)| *id).collect();
        let mob2: Vec<i32> = MOB2.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&mob1);
        s.add_kill_id(&mob2);
        s.register_quest_items(&[ALLIGATOR_TOOTH, MYSTERIOUS_MAP_PIECE]);
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
            "30895-04.html" | "30895-05.html" | "30895-09.html" => {
                htmltext = Some(event.to_owned());
            }
            "30895-02.html" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "ADENA" => {
                let count = ctx.get_quest_items_count(player, ALLIGATOR_TOOTH);
                if count >= 100 {
                    ctx.give_adena(player, count * 220 + 10700, true);
                    ctx.take_items(player, ALLIGATOR_TOOTH, -1);
                    htmltext = Some("30895-06.html".to_owned());
                } else if count > 0 {
                    ctx.give_adena(player, count * 220 + 3100, true);
                    ctx.take_items(player, ALLIGATOR_TOOTH, -1);
                    htmltext = Some("30895-07.html".to_owned());
                } else {
                    htmltext = Some("30895-08.html".to_owned());
                }
            }
            "30895-10.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "REWARD" => {
                let count = ctx.get_quest_items_count(player, MYSTERIOUS_MAP_PIECE);
                if count >= 10 {
                    ctx.give_items(player, PIRATES_TREASURE_MAP, 1);
                    ctx.take_items(player, MYSTERIOUS_MAP_PIECE, 10);
                    htmltext = Some("30895-13.html".to_owned());
                } else if count > 0 {
                    htmltext = Some("30895-12.html".to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.random_party_member_state(killer, -1, 3, Some(npc));
        if qs.is_some() {
            // NOTE(java-quirk): the teeth and the map piece go to the killer (`giveItemRandomly(player,
            // ..)`), not to the party member the roll above picked.
            if let Some((_, chance)) = MOB1.iter().find(|(id, _)| *id == npc.id) {
                ctx.give_item_randomly(killer, Some(npc), ALLIGATOR_TOOTH, 1, 1, 0, *chance, true);
            } else {
                let chance = MOB2.iter().find(|(id, _)| *id == npc.id).map_or(0, |(_, c)| *c);
                let item_count = if ctx.get_random(100) < chance { 2 } else { 1 };
                ctx.give_item_randomly(
                    killer,
                    Some(npc),
                    ALLIGATOR_TOOTH,
                    item_count,
                    item_count,
                    0,
                    1.0,
                    true,
                );
            }
            ctx.give_item_randomly(killer, Some(npc), MYSTERIOUS_MAP_PIECE, 1, 1, 0, 0.1, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30895-01.htm"
            } else {
                "30895-03.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            htmltext = if ctx.has_quest_items(player, MYSTERIOUS_MAP_PIECE) {
                "30895-11.html"
            } else {
                "30895-04.html"
            }
            .to_owned();
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

    const NAME: &str = "Q00354_ConquestOfAlligatorIsland";

    const NPCS: &str = r#"
        <npc id="30895" type="Folk" level="70" name="Warehouse Freightman Kluck"/>
        <npc id="20804" type="Monster" level="36" name="Crokian Lad"/>
        <npc id="20805" type="Monster" level="36" name="Dailaon Lad"/>
        <npc id="20806" type="Monster" level="36" name="Crokian Lad Warrior"/>
        <npc id="20807" type="Monster" level="36" name="Farhite Lad"/>
        <npc id="20808" type="Monster" level="36" name="Nos Lad"/>
        <npc id="20991" type="Monster" level="36" name="Tribe of Swamp"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5863" type="EtcItem" name="Alligator Tooth"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5864" type="EtcItem" name="Mysterious Map Piece"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5915" type="EtcItem" name="Pirate's Treasure Map"/>"#;

    const MOBS: [i32; 4] = [20804, 20805, 20806, 20807];

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00354ConquestOfAlligatorIsland);
        let pages = Pages::of(&Q00354ConquestOfAlligatorIsland)
            .page("30895-01.htm", &["30895-02.html"])
            .plain(&["30895-02.html", "30895-03.html"])
            .page(
                "30895-04.html",
                &[
                    "30895-05.html",
                    "ADENA",
                    "30895-09.html",
                    "30895-10.html",
                    "REWARD",
                ],
            )
            .page("30895-05.html", &["30895-04.html"])
            .page("30895-06.html", &["30895-09.html", "30895-10.html"])
            .page("30895-07.html", &["30895-09.html", "30895-10.html"])
            .plain(&[
                "30895-08.html",
                "30895-09.html",
                "30895-10.html",
                "30895-12.html",
                "30895-13.html",
            ])
            .page(
                "30895-11.html",
                &[
                    "30895-05.html",
                    "ADENA",
                    "30895-09.html",
                    "30895-10.html",
                    "REWARD",
                ],
            )
            .npcs(&[KLUCK]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let kluck = t.spawn(KLUCK, 20);
        (t, kluck)
    }

    /// The NPC's page once the player has some map pieces too (04 without, 11 with). Both
    /// offer the `ADENA` and the quit links.
    fn hub(t: &mut ScriptTest, kluck: Oid) -> String {
        let page = t.quest_page(1, kluck);
        assert!(
            page.contains("30895-04.html") || page.contains("30895-11.html"),
            "{page}"
        );
        page
    }

    /// Kills the Crokian and Dailaon lads in turns until the player has `count` of `item_id`.
    fn hunt(t: &mut ScriptTest, item_id: i32, count: i64) {
        for kills in 0..1000 {
            if t.count(1, item_id) >= count {
                return;
            }
            let mob = t.spawn(MOBS[kills % 4], 60);
            t.kill(1, mob);
        }
        panic!("no {count} of {item_id} after 1000 kills");
    }

    #[test]
    fn a_level_37_character_is_sent_away() {
        let (mut t, kluck) = setup();
        t.enter(1, 37, vec![]);
        assert!(t.quest_page(1, kluck).contains("30895-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn teeth_for_adena_then_quit() {
        let (mut t, kluck) = setup();
        t.enter(1, 38, vec![]);
        assert!(t.quest_page(1, kluck).contains("30895-01.htm"));
        assert!(t.event(1, NAME, "30895-02.html").contains("30895-02.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, kluck).contains("30895-04.html"));

        // No teeth: "Here are the teeth" pays nothing.
        assert!(t.event(1, NAME, "ADENA").contains("30895-08.html"));
        assert_eq!(t.count(1, 57), 0);

        // Three teeth: 220 each and 3100 on top; the quest goes on.
        hunt(&mut t, ALLIGATOR_TOOTH, 3);
        let teeth = t.count(1, ALLIGATOR_TOOTH);
        assert!(teeth < 100);
        hub(&mut t, kluck);
        assert!(t.event(1, NAME, "ADENA").contains("30895-07.html"));
        assert_eq!(t.count(1, 57), teeth * 220 + 3100);
        assert_eq!(t.count(1, ALLIGATOR_TOOTH), 0);
        assert!(t.take_writes().is_empty());

        // "I want out of the Contract."
        hub(&mut t, kluck);
        assert!(t.event(1, NAME, "30895-10.html").contains("30895-10.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.quest_page(1, kluck).contains("30895-01.htm"));
    }

    #[test]
    fn ten_map_pieces_make_the_pirates_map() {
        let (mut t, kluck) = setup();
        t.enter(1, 38, vec![]);
        t.quest_page(1, kluck);
        t.event(1, NAME, "30895-02.html");
        t.take_writes();

        // The map pieces drop on every kill with a 10% chance; the NPC wants ten of them.
        hunt(&mut t, MYSTERIOUS_MAP_PIECE, 10);
        let pieces = t.count(1, MYSTERIOUS_MAP_PIECE);
        assert!(t.quest_page(1, kluck).contains("30895-11.html"));
        assert!(t.event(1, NAME, "REWARD").contains("30895-13.html"));
        assert_eq!(t.count(1, PIRATES_TREASURE_MAP), 1);
        assert_eq!(t.count(1, MYSTERIOUS_MAP_PIECE), pieces - 10);
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_hundred_teeth_get_the_bonus() {
        let (mut t, kluck) = setup();
        t.enter(1, 38, vec![]);
        t.quest_page(1, kluck);
        t.event(1, NAME, "30895-02.html");
        t.take_writes();

        hunt(&mut t, ALLIGATOR_TOOTH, 100);
        let teeth = t.count(1, ALLIGATOR_TOOTH);
        assert!(teeth >= 100);
        hub(&mut t, kluck);
        assert!(t.event(1, NAME, "ADENA").contains("30895-06.html"));
        assert_eq!(t.count(1, 57), teeth * 220 + 10700);
        assert_eq!(t.count(1, ALLIGATOR_TOOTH), 0);
    }
}
