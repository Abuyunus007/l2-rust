//! Quest 714: Path to Becoming a Lord - Schuttgart (August sends the castle's lord after the golems).
// Java: dist/game/data/scripts/quests/Q00714_PathToBecomingALordSchuttgart/Q00714_PathToBecomingALordSchuttgart.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{CastleId, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const AUGUST: i32 = 35555;
const NEWYEAR: i32 = 31961;
const YASHENI: i32 = 31958;

// Item
const GOLEM_SHARD_PIECE: i32 = 17162;

// Monsters: the golems.
const MOBS: [i32; 11] = [
    22801, 22802, 22803, 22804, 22805, 22806, 22807, 22808, 22809, 22810, 22811,
];

/// Java `SCHUTTGART_CASTLE`.
const SCHUTTGART_CASTLE: CastleId = CastleId(9);

/// Java `Q00114_ResurrectionOfAnOldManager`, `Q00120_PavelsLastResearch`, `Q00121_PavelTheGiant`.
const Q00114: &str = "Q00114_ResurrectionOfAnOldManager";
const Q00120: &str = "Q00120_PavelsLastResearch";
const Q00121: &str = "Q00121_PavelTheGiant";

/// Java `castle.getOwner()`: the leader's character id, `None` when the castle has no lord.
fn castle_lord(ctx: &ScriptCtx<'_>) -> Option<i32> {
    let owner = ctx.world().castle(SCHUTTGART_CASTLE)?.owner_id;
    Some(ctx.world().clan(owner)?.leader_id())
}

/// Java `castle.getOwner().getLeader().getPlayer()`: the lord, when he is online.
fn castle_lord_player(ctx: &ScriptCtx<'_>) -> Option<ObjectId> {
    castle_lord(ctx)
        .map(ObjectId)
        .filter(|lord| ctx.world().player(*lord).is_some())
}

/// Java `hasFort()`: a fortress is contracted to the Schuttgart castle.
fn has_fort(ctx: &ScriptCtx<'_>) -> bool {
    ctx.world()
        .forts()
        .iter()
        .any(|fort| ctx.world().fort_contracted_castle(fort.id) == Some(SCHUTTGART_CASTLE))
}

/// Java `Q00714_PathToBecomingALordSchuttgart`.
pub struct Q00714PathToBecomingALordSchuttgart;

impl Script for Q00714PathToBecomingALordSchuttgart {
    fn name(&self) -> &'static str {
        "Q00714_PathToBecomingALordSchuttgart"
    }

    fn path(&self) -> &'static str {
        "quests/Q00714_PathToBecomingALordSchuttgart"
    }

    fn id(&self) -> i32 {
        714
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[AUGUST]);
        s.add_talk_id(&[AUGUST, NEWYEAR, YASHENI]);
        s.add_kill_id(&MOBS);
        s.register_quest_items(&[GOLEM_SHARD_PIECE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        if castle_lord(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(event.to_owned());
        };

        match event {
            "35555-03.html" => qs.start_quest(ctx),
            "35555-05.html" => qs.set_cond(ctx, 2, false),
            "31961-03.html" => qs.set_cond(ctx, 3, false),
            "31958-02.html" => qs.set_cond(ctx, 5, false),
            "35555-08.html" => {
                if castle_lord_player(ctx).is_some() {
                    if let Some(npc) = npc {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcShout,
                            NpcStringId::S1HasBecomeTheLordOfTheTownOfSchuttgartMayThereBeGloryInTheTerritoryOfSchuttgart,
                            &[&name],
                        );
                    }
                    qs.exit_quest(ctx, true, true);
                }
            }
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_cond(ctx, 5) {
            if ctx.get_quest_items_count(killer, GOLEM_SHARD_PIECE) < 300 {
                ctx.give_items(killer, GOLEM_SHARD_PIECE, 1);
            }
            if ctx.get_quest_items_count(killer, GOLEM_SHARD_PIECE) >= 300 {
                qs.set_cond(ctx, 6, false);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if castle_lord(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);

        match npc.id {
            AUGUST => {
                if qs.is_cond(ctx, 0) {
                    if castle_lord_player(ctx) == Some(player) {
                        if has_fort(ctx) {
                            htmltext = "35555-00.html".to_owned();
                            qs.exit_quest(ctx, true, false);
                        } else {
                            htmltext = "35555-01.html".to_owned();
                        }
                    } else {
                        htmltext = "35555-00a.html".to_owned();
                        qs.exit_quest(ctx, true, false);
                    }
                } else if qs.is_cond(ctx, 1) {
                    htmltext = "35555-04.html".to_owned();
                } else if qs.is_cond(ctx, 2) {
                    htmltext = "35555-06.html".to_owned();
                } else if qs.is_cond(ctx, 7) {
                    htmltext = "35555-07.html".to_owned();
                }
            }
            NEWYEAR => {
                if qs.is_cond(ctx, 2) {
                    htmltext = "31961-01.html".to_owned();
                } else if qs.is_cond(ctx, 3) {
                    // Java null-checks each state before `isCompleted()`: a missing one is not done.
                    let completed = |name: &str| {
                        ctx.player_quest_state(player, name)
                            .is_some_and(|s| s.is_completed())
                    };
                    htmltext = if completed(Q00121) {
                        if completed(Q00114) {
                            if completed(Q00120) {
                                qs.set_cond(ctx, 4, false);
                                "31961-04.html"
                            } else {
                                "31961-04a.html"
                            }
                        } else {
                            "31961-04b.html"
                        }
                    } else {
                        "31961-04c.html"
                    }
                    .to_owned();
                }
            }
            YASHENI => {
                if qs.is_cond(ctx, 4) {
                    htmltext = "31958-01.html".to_owned();
                } else if qs.is_cond(ctx, 5) {
                    htmltext = "31958-03.html".to_owned();
                } else if qs.is_cond(ctx, 6) {
                    ctx.take_items(player, GOLEM_SHARD_PIECE, -1);
                    qs.set_cond(ctx, 7, false);
                    htmltext = "31958-04.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"
        <npc id="35555" type="Folk" level="80" name="August"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00714PathToBecomingALordSchuttgart);
        let pages = Pages::of(&Q00714PathToBecomingALordSchuttgart).npcs(&[AUGUST]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let august = t.spawn(AUGUST, 20);
        (t, august)
    }

    /// No castle lord exists in the test world (no clan owns Schuttgart): the quest says so
    /// (`Castle has no lord.`, a chat line, not a page) and nothing is written.
    #[test]
    fn without_a_castle_lord_the_quest_says_so() {
        let (mut t, august) = setup();
        t.enter(1, 40, vec![]);
        assert_eq!(t.quest_page(1, august), "");
        assert!(t.take_writes().is_empty());
    }
}
