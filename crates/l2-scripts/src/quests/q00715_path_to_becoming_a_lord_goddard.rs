//! Quest 715: Path to Becoming a Lord - Goddard (Alfred sends the castle's lord to two spirits).
// Java: dist/game/data/scripts/quests/Q00715_PathToBecomingALordGoddard/Q00715_PathToBecomingALordGoddard.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{CastleId, Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ALFRED: i32 = 35363;

// Monsters
const WATER_SPIRIT_ASHUTAR: i32 = 25316;
const FIRE_SPIRIT_NASTRON: i32 = 25306;

/// Java `GODDARD_CASTLE`.
const GODDARD_CASTLE: CastleId = CastleId(7);

/// Java `castle.getOwner().getLeader()`: the leader's character id of the castle's clan, `None`
/// when the castle has no lord (Java `castle.getOwner() == null`).
fn castle_lord(ctx: &ScriptCtx<'_>) -> Option<i32> {
    let owner = ctx.world().castle(GODDARD_CASTLE)?.owner_id;
    Some(ctx.world().clan(owner)?.leader_id())
}

/// Java `castle.getOwner().getLeader().getPlayer()`: the lord, when he is online.
fn castle_lord_player(ctx: &ScriptCtx<'_>) -> Option<ObjectId> {
    castle_lord(ctx)
        .map(ObjectId)
        .filter(|lord| ctx.world().player(*lord).is_some())
}

/// Java `hasFort()`: a fortress is contracted to the Goddard castle.
fn has_fort(ctx: &ScriptCtx<'_>) -> bool {
    ctx.world()
        .forts()
        .iter()
        .any(|fort| ctx.world().fort_contracted_castle(fort.id) == Some(GODDARD_CASTLE))
}

/// Java `Q00715_PathToBecomingALordGoddard`.
pub struct Q00715PathToBecomingALordGoddard;

impl Script for Q00715PathToBecomingALordGoddard {
    fn name(&self) -> &'static str {
        "Q00715_PathToBecomingALordGoddard"
    }

    fn path(&self) -> &'static str {
        "quests/Q00715_PathToBecomingALordGoddard"
    }

    fn id(&self) -> i32 {
        715
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ALFRED]);
        s.add_kill_id(&[WATER_SPIRIT_ASHUTAR, FIRE_SPIRIT_NASTRON]);
        s.add_talk_id(&[ALFRED]);
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
            "35363-03.html" => qs.start_quest(ctx),
            "35363-04a.html" => qs.set_cond(ctx, 3, false),
            "35363-04b.html" => qs.set_cond(ctx, 2, false),
            "35363-08.html" => {
                if castle_lord_player(ctx).is_some() {
                    if let Some(npc) = npc {
                        let name = ctx.player_name(player);
                        ctx.broadcast_npc_say(
                            npc,
                            ChatType::NpcShout,
                            NpcStringId::S1HasBecomeTheLordOfTheTownOfGoddardMayThereBeGloryInTheTerritoryOfGoddard,
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

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };

        if qs.is_cond(ctx, 2) && npc.id == FIRE_SPIRIT_NASTRON {
            qs.set_cond(ctx, 4, false);
        } else if qs.is_cond(ctx, 3) && npc.id == WATER_SPIRIT_ASHUTAR {
            qs.set_cond(ctx, 5, false);
        }
        if qs.is_cond(ctx, 6) && npc.id == WATER_SPIRIT_ASHUTAR {
            qs.set_cond(ctx, 9, false);
        } else if qs.is_cond(ctx, 7) && npc.id == FIRE_SPIRIT_NASTRON {
            qs.set_cond(ctx, 8, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if castle_lord(ctx).is_none() {
            return Some("Castle has no lord.".to_owned());
        }
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);

        if qs.is_cond(ctx, 0) {
            if castle_lord_player(ctx) == Some(player) {
                if has_fort(ctx) {
                    htmltext = "35363-00.html".to_owned();
                    qs.exit_quest(ctx, true, false);
                } else {
                    htmltext = "35363-01.html".to_owned();
                }
            } else {
                htmltext = "35363-00a.html".to_owned();
                qs.exit_quest(ctx, true, false);
            }
        } else if qs.is_cond(ctx, 1) {
            htmltext = "35363-03.html".to_owned();
        } else if qs.is_cond(ctx, 2) {
            htmltext = "35363-05b.html".to_owned();
        } else if qs.is_cond(ctx, 3) {
            htmltext = "35363-05a.html".to_owned();
        } else if qs.is_cond(ctx, 4) {
            qs.set_cond(ctx, 6, false);
            htmltext = "35363-06b.html".to_owned();
        } else if qs.is_cond(ctx, 5) {
            qs.set_cond(ctx, 7, false);
            htmltext = "35363-06a.html".to_owned();
        } else if qs.is_cond(ctx, 6) {
            htmltext = "35363-06b.html".to_owned();
        } else if qs.is_cond(ctx, 7) {
            htmltext = "35363-06a.html".to_owned();
        } else if qs.is_cond(ctx, 8) || qs.is_cond(ctx, 9) {
            htmltext = "35363-07.html".to_owned();
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
        <npc id="35363" type="Folk" level="80" name="Alfred"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00715PathToBecomingALordGoddard);
        let pages = Pages::of(&Q00715PathToBecomingALordGoddard)
            .plain(&[
                "35363-00.html",
                "35363-00a.html",
                "35363-01.html",
                "35363-03.html",
            ])
            .npcs(&[ALFRED]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let alfred = t.spawn(ALFRED, 20);
        (t, alfred)
    }

    /// No castle lord exists in the test world (no clan owns Goddard): the quest says so
    /// (`Castle has no lord.`, a chat line, not a page) and nothing is written.
    #[test]
    fn without_a_castle_lord_the_quest_says_so() {
        let (mut t, alfred) = setup();
        t.enter(1, 40, vec![]);
        assert_eq!(t.quest_page(1, alfred), "");
        assert!(t.take_writes().is_empty());
    }
}
