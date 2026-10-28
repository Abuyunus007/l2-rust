//! Quest 295: Dreaming of the Skies (Arin in the Dwarven village, Magical Weavers).
// Java: dist/game/data/scripts/quests/Q00295_DreamingOfTheSkies/Q00295_DreamingOfTheSkies.java
//
// Pattern: a kill-count quest with a drop chance (`docs/design/scripts.md`, "Quest patterns"):
// `on_kill` gives the quest item with `give_item_randomly` up to a limit and moves to cond 2 when
// the limit is reached; the NPC rewards and ends the quest (repeatable).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ARIN: i32 = 30536;

// Monster
const MAGICAL_WEAVER: i32 = 20153;

// Item
const FLOATING_STONE: i32 = 1492;

// Reward
const RING_OF_FIREFLY: i32 = 1509;

// Misc
const MIN_LEVEL: i32 = 11;

/// Java `Q00295_DreamingOfTheSkies`.
pub struct Q00295DreamingOfTheSkies;

impl Script for Q00295DreamingOfTheSkies {
    fn name(&self) -> &'static str {
        "Q00295_DreamingOfTheSkies"
    }

    fn path(&self) -> &'static str {
        "quests/Q00295_DreamingOfTheSkies"
    }

    fn id(&self) -> i32 {
        295
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ARIN]);
        s.add_talk_id(&[ARIN]);
        s.add_kill_id(&[MAGICAL_WEAVER]);
        s.register_quest_items(&[FLOATING_STONE]);
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
            && qs.is_created(ctx)
            && event == "30536-03.htm"
        {
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
        {
            // Java evaluates the amount (`getRandom(100) > 25 ? 1 : 2`) before the call.
            let amount = if ctx.get_random(100) > 25 { 1 } else { 2 };
            if ctx.give_item_randomly(killer, Some(npc), FLOATING_STONE, amount, amount, 50, 1.0, true) {
                qs.set_cond(ctx, 2, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut html = ctx.get_no_quest_msg(talker);
        if qs.is_created(ctx) {
            html = if ctx.player_level(talker) >= MIN_LEVEL {
                "30536-02.htm"
            } else {
                "30536-01.htm"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            if qs.is_cond(ctx, 2) {
                if ctx.has_quest_items(talker, RING_OF_FIREFLY) {
                    ctx.give_adena(talker, 2400, true);
                    html = "30536-06.html".to_owned();
                } else {
                    ctx.give_items(talker, RING_OF_FIREFLY, 1);
                    html = "30536-05.html".to_owned();
                }
                ctx.take_items(talker, FLOATING_STONE, -1);
                qs.exit_quest(ctx, true, true);
            } else {
                html = "30536-04.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00295_DreamingOfTheSkies";

    const NPCS: &str = r#"
        <npc id="30536" type="Folk" level="70" name="Arin"/>
        <npc id="20153" type="Monster" level="12" name="Magical Weaver"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1492" type="EtcItem" name="Floating Stone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1509" type="Armor" name="Ring of Firefly"><set name="bodypart" val="rfinger;lfinger"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00295DreamingOfTheSkies);
        let pages = Pages::of(&Q00295DreamingOfTheSkies)
            .page("30536-02.htm", &["30536-03.htm"])
            .plain(&[
                "30536-01.htm",
                "30536-03.htm",
                "30536-04.html",
                "30536-05.html",
                "30536-06.html",
            ])
            .npcs(&[ARIN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let arin = t.spawn(ARIN, 20);
        (t, arin)
    }

    /// Kills Magical Weavers until the quest is at cond 2; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..200 {
            let weaver = t.spawn(MAGICAL_WEAVER, 60);
            t.kill(1, weaver);
            if t.count(1, FLOATING_STONE) >= 50 {
                return kills;
            }
        }
        panic!("no 50 stones after 200 kills");
    }

    #[test]
    fn a_low_level_character_is_sent_away() {
        let (mut t, arin) = setup();
        t.enter(1, 10, vec![]);
        assert!(t.quest_page(1, arin).contains("30536-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn fifty_stones_then_the_ring_then_adena() {
        let (mut t, arin) = setup();
        t.enter(1, 11, vec![]);
        assert!(t.quest_page(1, arin).contains("30536-02.htm"));
        assert!(t.event(1, NAME, "30536-03.htm").contains("30536-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, arin).contains("30536-04.html"));

        // A kill out of range of the killer gives nothing.
        let far = t
            .world
            .spawn_npc_at(
                MAGICAL_WEAVER,
                crate::testing::X + 3000,
                crate::testing::Y,
                crate::testing::Z,
                0,
            )
            .unwrap();
        t.kill(1, far);
        assert_eq!(t.count(1, FLOATING_STONE), 0);

        // The stones come one or two at a time, never more than 50; the 50th sets cond 2
        // (`setCond(2)`: no sound of its own, `give_item_randomly` plays the middle sound).
        let kills = hunt(&mut t);
        assert!(kills >= 25, "{kills}");
        assert_eq!(t.count(1, FLOATING_STONE), 50);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        let weaver = t.spawn(MAGICAL_WEAVER, 60);
        t.kill(1, weaver);
        assert_eq!(t.count(1, FLOATING_STONE), 50);

        // First time: the ring; the stones go, the quest ends repeatable.
        t.take();
        assert!(t.quest_page(1, arin).contains("30536-05.html"));
        assert_eq!((t.count(1, RING_OF_FIREFLY), t.count(1, FLOATING_STONE)), (1, 0));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);

        // Again with the ring: 2400 adena instead.
        assert!(t.quest_page(1, arin).contains("30536-02.htm"));
        t.event(1, NAME, "30536-03.htm");
        hunt(&mut t);
        t.take_writes();
        t.take();
        t.talk(1, arin);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, 57), 2400);
        assert_eq!(t.count(1, RING_OF_FIREFLY), 1);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
