//! Quest 291: Revenge of the Redbonnet (Maryse Redbonnet in the Dark Elven area, Black Wolves).
// Java: dist/game/data/scripts/quests/Q00291_RevengeOfTheRedbonnet/Q00291_RevengeOfTheRedbonnet.java
//
// Pattern: a kill-count quest with a drop chance and a limit (`docs/design/scripts.md`, "Quest
// patterns"): Black Wolves in the party range drop pelts up to 40, the 40th sets cond 2; the NPC
// takes the pelts, gives one random grandma's treasure and ends the quest (repeatable).

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const MARYSE_REDBONNET: i32 = 30553;

// Item
const BLACK_WOLF_PELT: i32 = 1482;
const BLACK_WOLF_PELT_COUNT: i64 = 40;

// Monster
const BLACK_WOLF: i32 = 20317;

// Rewards
const SCROLL_OF_ESCAPE: i32 = 736;
const GRANDMAS_PEARL: i32 = 1502;
const GRANDMAS_MIRROR: i32 = 1503;
const GRANDMAS_NECKLACE: i32 = 1504;
const GRANDMAS_HAIRPIN: i32 = 1505;

// Misc
const MIN_LEVEL: i32 = 4;

/// Java `Q00291_RevengeOfTheRedbonnet`.
pub struct Q00291RevengeOfTheRedbonnet;

impl Script for Q00291RevengeOfTheRedbonnet {
    fn name(&self) -> &'static str {
        "Q00291_RevengeOfTheRedbonnet"
    }

    fn path(&self) -> &'static str {
        "quests/Q00291_RevengeOfTheRedbonnet"
    }

    fn id(&self) -> i32 {
        291
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MARYSE_REDBONNET]);
        s.add_talk_id(&[MARYSE_REDBONNET]);
        s.add_kill_id(&[BLACK_WOLF]);
        s.register_quest_items(&[BLACK_WOLF_PELT]);
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
        if event == "30553-03.htm" {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
            && ctx.give_item_randomly(
                killer,
                Some(npc),
                BLACK_WOLF_PELT,
                1,
                1,
                BLACK_WOLF_PELT_COUNT,
                1.0,
                true,
            )
        {
            // `setCond(2)`: no sound of its own.
            qs.set_cond(ctx, 2, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut html = ctx.get_no_quest_msg(player);
        if qs.state(ctx) == CREATED {
            html = if ctx.player_level(player) >= MIN_LEVEL {
                "30553-02.htm"
            } else {
                "30553-01.htm"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            if qs.is_cond(ctx, 2)
                && ctx.get_quest_items_count(player, BLACK_WOLF_PELT) >= BLACK_WOLF_PELT_COUNT
            {
                ctx.take_items(player, BLACK_WOLF_PELT, BLACK_WOLF_PELT_COUNT);
                let chance = ctx.get_random(100);
                if chance <= 2 {
                    ctx.give_items(player, GRANDMAS_PEARL, 1);
                } else if chance <= 20 {
                    ctx.give_items(player, GRANDMAS_MIRROR, 1);
                } else if chance <= 45 {
                    ctx.give_items(player, GRANDMAS_NECKLACE, 1);
                } else {
                    ctx.give_items(player, GRANDMAS_HAIRPIN, 1);
                    ctx.give_items(player, SCROLL_OF_ESCAPE, 1);
                }
                qs.exit_quest(ctx, true, true);
                html = "30553-05.html".to_owned();
            } else {
                html = "30553-04.html".to_owned();
            }
        }
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00291_RevengeOfTheRedbonnet";

    const NPCS: &str = r#"
        <npc id="30553" type="Folk" level="70" name="Maryse Redbonnet"/>
        <npc id="20317" type="Monster" level="20" name="Black Wolf"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1482" type="EtcItem" name="Black Wolf Pelt"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="736" type="EtcItem" name="Scroll of Escape"><set name="is_stackable" val="true"/></item>
        <item id="1502" type="EtcItem" name="Grandma's Pearl"><set name="is_stackable" val="true"/></item>
        <item id="1503" type="EtcItem" name="Grandma's Mirror"><set name="is_stackable" val="true"/></item>
        <item id="1504" type="EtcItem" name="Grandma's Necklace"><set name="is_stackable" val="true"/></item>
        <item id="1505" type="EtcItem" name="Grandma's Hairpin"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00291RevengeOfTheRedbonnet);
        let pages = Pages::of(&Q00291RevengeOfTheRedbonnet)
            .page("30553-02.htm", &["30553-03.htm"])
            .plain(&["30553-01.htm", "30553-03.htm", "30553-04.html", "30553-05.html"])
            .npcs(&[MARYSE_REDBONNET]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let maryse = t.spawn(MARYSE_REDBONNET, 20);
        (t, maryse)
    }

    /// Kills Black Wolves until `pelts` pelts are in the inventory; returns the kills.
    fn hunt(t: &mut ScriptTest, pelts: i64) -> usize {
        for kills in 1..400 {
            let wolf = t.spawn(BLACK_WOLF, 60);
            t.kill(1, wolf);
            if t.count(1, BLACK_WOLF_PELT) >= pelts {
                return kills;
            }
        }
        panic!("no {pelts} pelts after 400 kills");
    }

    #[test]
    fn a_level_three_character_is_sent_away() {
        let (mut t, maryse) = setup();
        t.enter(1, 3, vec![]);
        assert!(t.quest_page(1, maryse).contains("30553-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn forty_pelts_then_one_grandma_treasure() {
        let (mut t, maryse) = setup();
        t.enter(1, 4, vec![]);
        assert!(t.quest_page(1, maryse).contains("30553-02.htm"));
        assert!(t.event(1, NAME, "30553-03.htm").contains("30553-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // A wolf out of the killer's range gives nothing.
        let far = t
            .world
            .spawn_npc_at(
                BLACK_WOLF,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, BLACK_WOLF_PELT), 0);

        // The 40th pelt sets cond 2 (`setCond(2)`, no sound of its own).
        hunt(&mut t, BLACK_WOLF_PELT_COUNT);
        assert_eq!(t.count(1, BLACK_WOLF_PELT), BLACK_WOLF_PELT_COUNT);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Maryse takes the pelts and gives one treasure, the escape scroll only with the hairpin.
        assert!(t.quest_page(1, maryse).contains("30553-05.html"));
        assert_eq!(t.count(1, BLACK_WOLF_PELT), 0);
        let treasures: i64 = [
            GRANDMAS_PEARL,
            GRANDMAS_MIRROR,
            GRANDMAS_NECKLACE,
            GRANDMAS_HAIRPIN,
        ]
        .iter()
        .map(|&item| t.count(1, item))
        .sum();
        assert_eq!(treasures, 1);
        assert_eq!(t.count(1, SCROLL_OF_ESCAPE), t.count(1, GRANDMAS_HAIRPIN));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
