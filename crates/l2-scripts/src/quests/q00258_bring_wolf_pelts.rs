//! Quest 258: Bring Wolf Pelts (Lector in the Talking Island village gives the quest to the
//! characters of level 3 and up; 40 wolf pelts earn a random piece of cheap armour).
// Java: dist/game/data/scripts/quests/Q00258_BringWolfPelts/Q00258_BringWolfPelts.java
//
// Pattern: a kill count with a random reward and a repeatable quest (`docs/design/scripts.md`,
// "Quest patterns"): `exit_quest(ctx, true, true)` at the reward, so the quest can be taken again.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const LECTOR: i32 = 30001;

// Item
const WOLF_PELT: i32 = 702;

// Monsters: Wolf, Elder Wolf
const MONSTERS: [i32; 2] = [20120, 20442];

/// Java `REWARDS` (item, the chance roll below which it is given) in the order Java's `HashMap`
/// iterates it: buckets 6 (390, 22), 10 (426), 13 (29), 15 (1119) of 16.
// NOTE(java-quirk): the loop stops at the first entry whose limit is above the roll, so Tunic
// (limit 16) takes every roll from 9 up and Leather Pants (29) and Short Leather Gloves (1119)
// are never given.
const REWARDS: [(i32, i32); 5] = [
    (390, 1),   // Cotton Shirt
    (22, 9),    // Leather Shirt
    (426, 16),  // Tunic
    (29, 6),    // Leather Pants
    (1119, 13), // Short Leather Gloves
];

// Misc
const MIN_LEVEL: i32 = 3;
const WOLF_PELT_COUNT: i64 = 40;

/// The reward of Java's loop over `REWARDS` for a roll of `chance` (0..16).
fn reward_item(chance: i32) -> Option<i32> {
    REWARDS
        .iter()
        .find(|(_, limit)| chance < *limit)
        .map(|(item, _)| *item)
}

/// Java `Q00258_BringWolfPelts`.
pub struct Q00258BringWolfPelts;

impl Script for Q00258BringWolfPelts {
    fn name(&self) -> &'static str {
        "Q00258_BringWolfPelts"
    }

    fn path(&self) -> &'static str {
        "quests/Q00258_BringWolfPelts"
    }

    fn id(&self) -> i32 {
        258
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LECTOR]);
        s.add_talk_id(&[LECTOR]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[WOLF_PELT]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        if let Some(qs) = ctx.get_quest_state(player, false)
            && event.eq_ignore_ascii_case("30001-03.html")
        {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_cond(ctx, 1) {
            ctx.give_items(killer, WOLF_PELT, 1);
            if ctx.get_quest_items_count(killer, WOLF_PELT) >= WOLF_PELT_COUNT {
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30001-02.htm"
                } else {
                    "30001-01.html"
                }
                .to_owned();
            }
            STARTED => {
                if qs.is_cond(ctx, 1) {
                    htmltext = "30001-04.html".to_owned();
                } else if qs.is_cond(ctx, 2)
                    && ctx.get_quest_items_count(player, WOLF_PELT) >= WOLF_PELT_COUNT
                {
                    let chance = ctx.get_random(16);
                    if let Some(item) = reward_item(chance) {
                        ctx.give_items(player, item, 1);
                    }
                    qs.exit_quest(ctx, true, true);
                    htmltext = "30001-05.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00258_BringWolfPelts";

    const NPCS: &str = r#"
        <npc id="30001" type="Folk" level="70" name="Lector"/>
        <npc id="20120" type="Monster" level="6" name="Wolf"/>
        <npc id="20442" type="Monster" level="10" name="Elder Wolf"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="702" type="EtcItem" name="Wolf Pelt"><set name="is_questitem" val="true"/></item>
        <item id="390" type="Armor" name="Cotton Shirt"><set name="bodypart" val="chest"/></item>
        <item id="22" type="Armor" name="Leather Shirt"><set name="bodypart" val="chest"/></item>
        <item id="426" type="Armor" name="Tunic"><set name="bodypart" val="chest"/></item>
        <item id="29" type="Armor" name="Leather Pants"><set name="bodypart" val="legs"/></item>
        <item id="1119" type="Armor" name="Short Leather Gloves"><set name="bodypart" val="gloves"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00258BringWolfPelts);
        let pages = Pages::of(&Q00258BringWolfPelts)
            .page("30001-02.htm", &["30001-03.html"])
            .plain(&["30001-01.html", "30001-03.html", "30001-04.html", "30001-05.html"])
            .npcs(&[LECTOR]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let lector = t.spawn(LECTOR, 20);
        (t, lector)
    }

    #[test]
    fn a_character_below_level_3_is_sent_away() {
        let (mut t, lector) = setup();
        t.enter(1, 2, vec![]);
        assert!(t.quest_page(1, lector).contains("30001-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_wolves_bring_the_pelts_and_the_reward_repeats() {
        let (mut t, lector) = setup();
        t.enter(1, 3, vec![]);
        assert!(t.quest_page(1, lector).contains("30001-02.htm"));
        assert!(t.event(1, NAME, "30001-03.html").contains("30001-03.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, lector).contains("30001-04.html"));

        // Each wolf gives a pelt; the 40th sets cond 2.
        for _ in 0..39 {
            let wolf = t.spawn(20120, 20);
            t.kill(1, wolf);
        }
        assert_eq!(t.count(1, WOLF_PELT), 39);
        assert!(t.take_writes().is_empty());
        let elder = t.spawn(20442, 20);
        t.kill(1, elder);
        assert_eq!(t.count(1, WOLF_PELT), 40);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Lector pays one piece of armour and the quest ends (`exitQuest(true, true)`).
        assert!(t.quest_page(1, lector).contains("30001-05.html"));
        assert_eq!(t.count(1, WOLF_PELT), 0);
        let rewards = [390, 22, 426].iter().map(|i| t.count(1, *i)).sum::<i64>();
        assert_eq!(rewards, 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());

        // Repeatable: Lector offers the quest again.
        assert!(t.quest_page(1, lector).contains("30001-02.htm"));
    }

    #[test]
    fn the_reward_roll_follows_java_hash_map_order() {
        assert_eq!(reward_item(0), Some(390));
        for chance in 1..=8 {
            assert_eq!(reward_item(chance), Some(22), "{chance}");
        }
        // NOTE(java-quirk): Tunic takes every roll from 9, so 29 and 1119 never come out.
        for chance in 9..16 {
            assert_eq!(reward_item(chance), Some(426), "{chance}");
        }
        assert_eq!(reward_item(16), None);
    }

    #[test]
    fn a_pelt_count_below_40_is_still_collecting() {
        let (mut t, lector) = setup();
        t.enter(1, 3, vec![]);
        t.quest_page(1, lector);
        assert!(t.event(1, NAME, "30001-03.html").contains("30001-03.html"));
        t.take_writes();
        let wolf = t.spawn(20120, 20);
        t.kill(1, wolf);
        // One pelt of 40: cond stays 1, so Lector only asks for more.
        assert!(t.quest_page(1, lector).contains("30001-04.html"));
        assert_eq!(t.count(1, WOLF_PELT), 1);
        assert!(t.take_writes().is_empty());
        assert!(names_to(&t.take(), 1).is_empty());
    }
}
