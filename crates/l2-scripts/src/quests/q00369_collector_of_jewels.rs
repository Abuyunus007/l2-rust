//! Quest 369: Collector of Jewels (Nell in the Town of Talking Island, the shards of the Forgotten Temple).
// Java: dist/game/data/scripts/quests/Q00369_CollectorOfJewels/Q00369_CollectorOfJewels.java
//
// Pattern: a party quest with a memo state per stage (`docs/design/scripts.md`, "Quest
// patterns"): memo 1 = collecting 100 shards, memo 2 = paid, memo 3 = collecting 400. A kill
// gives the shard to a random party member who is at memo 1 or 3 (`random_party_member_for_npc`,
// `check_party_member_state` = Java `checkPartyMember`), up to a limit per shard kind.

use l2_util::ObjectId;
use l2_world::script::Qs;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const NELL: i32 = 30376;

// Items
const FLARE_SHARD: i32 = 5882;
const FREEZING_SHARD: i32 = 5883;

// Misc
const MIN_LEVEL: i32 = 25;

/// Java `MOBS_DROP_CHANCES`: monster, shard, chance in percent, shards per drop.
const MOBS_DROP_CHANCES: [(i32, i32, i32, i64); 6] = [
    (20609, FLARE_SHARD, 75, 1),     // salamander_lakin
    (20612, FLARE_SHARD, 91, 1),     // salamander_rowin
    (20749, FLARE_SHARD, 100, 2),    // death_fire
    (20616, FREEZING_SHARD, 81, 1),  // undine_lakin
    (20619, FREEZING_SHARD, 87, 1),  // undine_rowin
    (20747, FREEZING_SHARD, 100, 2), // roxide
];

/// Java `Q00369_CollectorOfJewels`.
pub struct Q00369CollectorOfJewels;

impl Script for Q00369CollectorOfJewels {
    fn name(&self) -> &'static str {
        "Q00369_CollectorOfJewels"
    }

    fn path(&self) -> &'static str {
        "quests/Q00369_CollectorOfJewels"
    }

    fn id(&self) -> i32 {
        369
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[NELL]);
        s.add_talk_id(&[NELL]);
        let mobs: Vec<i32> = MOBS_DROP_CHANCES.iter().map(|(id, ..)| *id).collect();
        s.add_kill_id(&mobs);
        s.register_quest_items(&[FLARE_SHARD, FREEZING_SHARD]);
    }

    fn check_party_member_state(&self, ctx: &mut ScriptCtx<'_>, qs: Qs, _npc: Option<ScriptNpc>) -> bool {
        qs.is_memo_state(ctx, 1) || qs.is_memo_state(ctx, 3)
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
            "30376-02.htm" => {
                qs.start_quest(ctx);
                qs.set_memo_state(ctx, 1);
                htmltext = Some(event.to_owned());
            }
            "30376-05.html" => {
                htmltext = Some(event.to_owned());
            }
            "30376-06.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30376-07.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(&(_, item_id, chance, count)) = MOBS_DROP_CHANCES.iter().find(|(id, ..)| *id == npc.id)
        else {
            return;
        };
        if ctx.get_random(100) < chance
            && let Some(lucky) = ctx.random_party_member_for_npc(killer, Some(npc))
            && let Some(qs) = ctx.get_quest_state(lucky, false)
        {
            let memo1 = qs.is_memo_state(ctx, 1);
            let item_count: i64 = if memo1 { 50 } else { 200 };
            let cond = if memo1 { 2 } else { 4 };
            if ctx.give_item_randomly(lucky, Some(npc), item_id, count, count, item_count, 1.0, true)
                && ctx.get_quest_items_count_of(lucky, &[FLARE_SHARD, FREEZING_SHARD]) >= item_count * 2
            {
                qs.set_cond(ctx, cond, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                "30376-01.htm"
            } else {
                "30376-03.html"
            }
            .to_owned();
        } else if qs.is_started(ctx) {
            match qs.get_memo_state(ctx) {
                1 => {
                    if ctx.get_quest_items_count_of(player, &[FLARE_SHARD, FREEZING_SHARD]) >= 100 {
                        ctx.give_adena(player, 31810, true);
                        ctx.take_items_many(player, -1, &[FLARE_SHARD, FREEZING_SHARD]);
                        qs.set_memo_state(ctx, 2);
                        htmltext = "30376-04.html".to_owned();
                    } else {
                        htmltext = "30376-08.html".to_owned();
                    }
                }
                2 => {
                    htmltext = "30376-09.html".to_owned();
                }
                3 => {
                    if ctx.get_quest_items_count_of(player, &[FLARE_SHARD, FREEZING_SHARD]) >= 400 {
                        ctx.give_adena(player, 84415, true);
                        ctx.take_items_many(player, -1, &[FLARE_SHARD, FREEZING_SHARD]);
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30376-10.html".to_owned();
                    } else {
                        htmltext = "30376-11.html".to_owned();
                    }
                }
                _ => {}
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

    const NAME: &str = "Q00369_CollectorOfJewels";

    const NPCS: &str = r#"
        <npc id="30376" type="Folk" level="70" name="Nell"/>
        <npc id="20609" type="Monster" level="30" name="Salamander Lakin"/>
        <npc id="20616" type="Monster" level="30" name="Undine Lakin"/>
        <npc id="20749" type="Monster" level="40" name="Death Fire"/>
        <npc id="20747" type="Monster" level="40" name="Roxide"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="5882" type="EtcItem" name="Flare Shard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="5883" type="EtcItem" name="Freezing Shard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00369CollectorOfJewels);
        let pages = Pages::of(&Q00369CollectorOfJewels)
            .page("30376-01.htm", &["30376-02.htm"])
            .plain(&[
                "30376-02.htm",
                "30376-03.html",
                "30376-06.html",
                "30376-07.html",
                "30376-08.html",
                "30376-10.html",
                "30376-11.html",
            ])
            .page(
                "30376-04.html",
                &["30376-05.html", "30376-06.html", "30376-07.html"],
            )
            .page("30376-05.html", &["30376-06.html", "30376-07.html"])
            .page(
                "30376-09.html",
                &["30376-05.html", "30376-06.html", "30376-07.html"],
            )
            .npcs(&[NELL]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let nell = t.spawn(NELL, 20);
        (t, nell)
    }

    /// Kills the monsters of `mobs` in turns until the player has `shards` shards of both kinds.
    fn hunt(t: &mut ScriptTest, mobs: &[i32], shards: i64) {
        for kills in 0..1000 {
            if t.count(1, FLARE_SHARD) + t.count(1, FREEZING_SHARD) >= shards {
                return;
            }
            let mob = t.spawn(mobs[kills % mobs.len()], 60);
            t.kill(1, mob);
        }
        panic!("no {shards} shards after 1000 kills");
    }

    #[test]
    fn a_level_24_character_is_sent_away() {
        let (mut t, nell) = setup();
        t.enter(1, 24, vec![]);
        assert!(t.quest_page(1, nell).contains("30376-03.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_hundred_shards_then_the_four_hundred_shards() {
        let (mut t, nell) = setup();
        t.enter(1, 25, vec![]);
        assert!(t.quest_page(1, nell).contains("30376-01.htm"));
        assert!(t.event(1, NAME, "30376-02.htm").contains("30376-02.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert!(t.quest_page(1, nell).contains("30376-08.html"));

        // Stage 1: 50 flare and 50 freezing shards; the set that completes both sets cond 2.
        hunt(&mut t, &[20609, 20616], 100);
        assert_eq!(t.count(1, FLARE_SHARD), 50);
        assert_eq!(t.count(1, FREEZING_SHARD), 50);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The NPC takes the shards and pays; memo state 2.
        let adena = t.count(1, 57);
        assert!(t.quest_page(1, nell).contains("30376-04.html"));
        assert_eq!(t.count(1, 57), adena + 31810);
        assert_eq!((t.count(1, FLARE_SHARD), t.count(1, FREEZING_SHARD)), (0, 0));
        assert_eq!(t.take_writes(), [rows::update(NAME, "memoState", "2")]);
        assert!(t.quest_page(1, nell).contains("30376-09.html"));

        // Stage 2: "Would you be interested in working some more?" (06): memo 3, cond 3.
        assert!(t.event(1, NAME, "30376-06.html").contains("30376-06.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "memoState", "3"),
                rows::update(NAME, "cond", "3"),
            ]
        );
        assert!(t.quest_page(1, nell).contains("30376-11.html"));

        // Stage 3: 200 of each kind; the set that completes both sets cond 4.
        hunt(&mut t, &[20749, 20747], 400);
        assert_eq!(t.count(1, FLARE_SHARD), 200);
        assert_eq!(t.count(1, FREEZING_SHARD), 200);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);

        // The reward, the shards go, the quest ends repeatable.
        let adena = t.count(1, 57);
        assert!(t.quest_page(1, nell).contains("30376-10.html"));
        assert_eq!(t.count(1, 57), adena + 84415);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
