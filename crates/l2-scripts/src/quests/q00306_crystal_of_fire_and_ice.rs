//! Quest 306: Crystals of Fire and Ice (Katerina in the Town of Dion sends level 17 players after the elemental shards).
// Java: dist/game/data/scripts/quests/Q00306_CrystalOfFireAndIce/Q00306_CrystalOfFireAndIce.java
//
// Pattern: a party drop quest without a condition (`docs/design/scripts.md`, "Quest patterns"):
// the salamanders drop flame shards and the undines ice shards, a random started party member
// gets them in the party range. Katerina pays 40 adena per shard, with a bonus from 10 shards, and
// takes them all; the quest repeats. Undine Nobles give their ice shards to the killer only.

use l2_util::ObjectId;
use l2_world::script::{CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const KATERINA: i32 = 30004;

// Items
const FLAME_SHARD: i32 = 1020;
const ICE_SHARD: i32 = 1021;

// Misc
const MIN_LEVEL: i32 = 17;

// Monsters
const UNDINE_NOBLE: i32 = 20115;

/// Java `MONSTER_DROPS`: the shard and the threshold of each monster; the drop chance is
/// `1000 / threshold` percent-of-a-thousand (so the chance is above one for every monster).
fn monster_drop(npc_id: i32) -> Option<(i32, i64)> {
    match npc_id {
        20109 => Some((FLAME_SHARD, 925)), // Salamander
        20110 => Some((ICE_SHARD, 900)),   // Undine
        20112 => Some((FLAME_SHARD, 900)), // Salamander Elder
        20113 => Some((ICE_SHARD, 925)),   // Undine Elder
        20114 => Some((FLAME_SHARD, 925)), // Salamander Noble
        UNDINE_NOBLE => Some((ICE_SHARD, 950)),
        _ => None,
    }
}

/// Java `Q00306_CrystalOfFireAndIce`.
pub struct Q00306CrystalOfFireAndIce;

impl Script for Q00306CrystalOfFireAndIce {
    fn name(&self) -> &'static str {
        "Q00306_CrystalOfFireAndIce"
    }

    fn path(&self) -> &'static str {
        "quests/Q00306_CrystalOfFireAndIce"
    }

    fn id(&self) -> i32 {
        306
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KATERINA]);
        s.add_talk_id(&[KATERINA]);
        s.add_kill_id(&[20109, 20110, 20112, 20113, 20114, UNDINE_NOBLE]);
        s.register_quest_items(&[FLAME_SHARD, ICE_SHARD]);
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
            "30004-04.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "30004-08.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30004-09.html" => htmltext = Some(event.to_owned()),
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if npc.id == UNDINE_NOBLE {
            // Undine Noble gives quest drops only for the killer.
            let qs = ctx.get_quest_state(killer, false);
            if qs.is_some_and(|qs| qs.is_started(ctx)) {
                give_kill_reward(ctx, killer, npc);
            }
        } else if let Some(member) = ctx.random_party_member_state(killer, -1, 3, Some(npc)) {
            give_kill_reward(ctx, member.player(), npc);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30004-03.htm"
                } else {
                    "30004-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                let shards = [FLAME_SHARD, ICE_SHARD];
                if ctx.has_at_least_one_quest_item(player, &shards) {
                    let flame = ctx.get_quest_items_count(player, FLAME_SHARD);
                    let ice = ctx.get_quest_items_count(player, ICE_SHARD);
                    let bonus = if flame + ice >= 10 { 5000 } else { 0 };
                    ctx.give_adena(player, flame * 40 + ice * 40 + bonus, true);
                    ctx.take_items_many(player, -1, &shards);
                    htmltext = "30004-07.html".to_owned();
                } else {
                    htmltext = "30004-05.html".to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

/// Java `giveKillReward`: the shard of the monster, in the party range without the height check.
fn give_kill_reward(ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: ScriptNpc) {
    if ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, player, false) {
        if let Some((item, threshold)) = monster_drop(npc.id) {
            // Java's 7-argument overload: one shard, no limit, chance 1000 / threshold.
            ctx.give_item_randomly(player, Some(npc), item, 1, 1, 0, 1000.0 / threshold as f64, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00306_CrystalOfFireAndIce";

    const NPCS: &str = r#"
        <npc id="30004" type="Folk" level="70" name="Katerina"/>
        <npc id="20109" type="Monster" level="20" name="Salamander"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1020" type="EtcItem" name="Flame Shard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1021" type="EtcItem" name="Ice Shard"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00306CrystalOfFireAndIce);
        let pages = Pages::of(&Q00306CrystalOfFireAndIce)
            .page("30004-03.htm", &["30004-04.htm"])
            .plain(&[
                "30004-02.htm",
                "30004-04.htm",
                "30004-05.html",
                "30004-09.html",
                "30004-08.html",
            ])
            .page("30004-07.html", &["30004-08.html", "30004-09.html"])
            .npcs(&[KATERINA]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let katerina = t.spawn(KATERINA, 20);
        (t, katerina)
    }

    #[test]
    fn a_level_sixteen_character_is_sent_away() {
        let (mut t, katerina) = setup();
        t.enter_as(1, 16, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, katerina).contains("30004-02.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn shards_for_forty_adena_each_with_the_bonus_from_ten() {
        let (mut t, katerina) = setup();
        t.enter_as(1, 17, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, katerina).contains("30004-03.htm"));
        assert!(t.event(1, NAME, "30004-04.htm").contains("30004-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, katerina).contains("30004-05.html"));

        // Salamanders drop flame shards; no condition is set.
        for _ in 0..12 {
            let salamander = t.spawn(20109, 60);
            t.kill(1, salamander);
        }
        let flame = t.count(1, FLAME_SHARD);
        assert_eq!(flame, 12);
        assert!(t.take_writes().is_empty());

        // 40 adena a shard and 5000 more from 10 shards on; the shards go.
        assert!(t.quest_page(1, katerina).contains("30004-07.html"));
        assert_eq!(t.count(1, 57), flame * 40 + 5000);
        assert_eq!(t.count(1, FLAME_SHARD), 0);

        // `exitQuest(true, true)` from the last page.
        assert!(t.event(1, NAME, "30004-08.html").contains("30004-08.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
