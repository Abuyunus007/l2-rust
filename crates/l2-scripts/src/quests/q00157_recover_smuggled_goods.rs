//! Quest 157: Recover Smuggled Goods (Wilford sends the player after Giant Toads for adamantite ore).
// Java: dist/game/data/scripts/quests/Q00157_RecoverSmuggledGoods/Q00157_RecoverSmuggledGoods.java
//
// Pattern: a kill count with a drop chance up to a limit (`docs/design/scripts.md`, "Kills,
// drops and parties"): every Giant Toad drops the ore with a 4-in-10 roll until 20 are in hand,
// then cond 2 is set. The start NPC gives the buckler and ends the quest (one-time).

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const WILFORD: i32 = 30005;

// Monster
const GIANT_TOAD: i32 = 20121;

// Items
const BUCKLER: i32 = 20;
const ADAMANTITE_ORE: i32 = 1024;

// Misc
const MIN_LEVEL: i32 = 5;
const ORE_NEEDED: i64 = 20;

/// Java `Q00157_RecoverSmuggledGoods`.
pub struct Q00157RecoverSmuggledGoods;

impl Script for Q00157RecoverSmuggledGoods {
    fn name(&self) -> &'static str {
        "Q00157_RecoverSmuggledGoods"
    }

    fn path(&self) -> &'static str {
        "quests/Q00157_RecoverSmuggledGoods"
    }

    fn id(&self) -> i32 {
        157
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WILFORD]);
        s.add_talk_id(&[WILFORD]);
        s.add_kill_id(&[GIANT_TOAD]);
        s.register_quest_items(&[ADAMANTITE_ORE]);
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
        match event {
            "30005-03.htm" => Some(event.to_owned()),
            "30005-04.htm" => {
                qs.start_quest(ctx);
                Some(event.to_owned())
            }
            _ => None,
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.get_random(10) < 4
            && ctx.get_quest_items_count(killer, ADAMANTITE_ORE) < ORE_NEEDED
        {
            ctx.give_items(killer, ADAMANTITE_ORE, 1);
            if ctx.get_quest_items_count(killer, ADAMANTITE_ORE) >= ORE_NEEDED {
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
                    "30005-02.htm".to_owned()
                } else {
                    "30005-01.htm".to_owned()
                };
            }
            STARTED => {
                if qs.is_cond(ctx, 2) && ctx.get_quest_items_count(player, ADAMANTITE_ORE) >= ORE_NEEDED {
                    ctx.give_items(player, BUCKLER, 1);
                    qs.exit_quest(ctx, false, true);
                    htmltext = "30005-06.html".to_owned();
                } else {
                    htmltext = "30005-05.html".to_owned();
                }
            }
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
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

    const NAME: &str = "Q00157_RecoverSmuggledGoods";

    const NPCS: &str = r#"
        <npc id="30005" type="Folk" level="1" name="Wilford"/>
        <npc id="20121" type="Monster" level="2" name="Giant Toad"/>"#;

    const ITEMS: &str = r#"
        <item id="20" type="Weapon" name="Wooden Buckler"/>
        <item id="1024" type="EtcItem" name="Adamantite Ore"><set name="is_questitem" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00157RecoverSmuggledGoods);
        let pages = Pages::of(&Q00157RecoverSmuggledGoods)
            .page("30005-02.htm", &["30005-03.htm"])
            .page("30005-03.htm", &["30005-04.htm"])
            .plain(&[
                "30005-01.htm",
                "30005-04.htm",
                "30005-05.html",
                "30005-06.html",
            ])
            .npcs(&[WILFORD]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let wilford = t.spawn(WILFORD, 20);
        (t, wilford)
    }

    #[test]
    fn a_character_below_level_5_is_sent_away() {
        let (mut t, wilford) = setup();
        t.enter(1, 4, vec![]);
        assert!(t.quest_page(1, wilford).contains("30005-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_ore_from_the_toads_and_the_buckler() {
        let (mut t, wilford) = setup();
        t.enter(1, 5, vec![]);
        assert!(t.quest_page(1, wilford).contains("30005-02.htm"));
        assert!(t.event(1, NAME, "30005-03.htm").contains("30005-03.htm"));
        assert!(t.event(1, NAME, "30005-04.htm").contains("30005-04.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Not enough ore yet: cond 1 answers.
        assert!(t.quest_page(1, wilford).contains("30005-05.html"));
        assert!(t.take_writes().is_empty());

        // Toads until 20 ores: the last one sets cond 2 (no pick-up sound for it).
        let mut toads = 0;
        while t.count(1, ADAMANTITE_ORE) < ORE_NEEDED {
            let toad = t.spawn(GIANT_TOAD, 40);
            t.kill(1, toad);
            toads += 1;
            assert!(toads < 500, "the toads never dropped 20 ore");
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Wilford: the buckler, `exitQuest(false, true)` with the finish sound.
        assert!(t.quest_page(1, wilford).contains("30005-06.html"));
        assert_eq!(t.count(1, BUCKLER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(
            t.quest_page(1, wilford)
                .contains("This quest has already been completed.")
        );
    }
}
