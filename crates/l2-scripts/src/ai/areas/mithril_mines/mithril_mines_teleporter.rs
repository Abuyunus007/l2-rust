//! The Teleport Crystals of the Mithril Mines.
// Java: dist/game/data/scripts/ai/areas/MithrilMines/MithrilMinesTeleporter/MithrilMinesTeleporter.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TELEPORT_CRYSTAL: i32 = 32652;
// Location
const LOCS: [(i32, i32, i32); 6] = [
    (171946, -173352, 3440),
    (175499, -181586, -904),
    (173462, -174011, 3480),
    (179299, -182831, -224),
    (178591, -184615, -360),
    (175499, -181586, -904),
];
/// Java `Npc.INTERACTION_DISTANCE`.
const INTERACTION_DISTANCE: f64 = 150.0;

/// Java `ai.areas.MithrilMines.MithrilMinesTeleporter.MithrilMinesTeleporter`.
pub struct MithrilMinesTeleporter;

/// Java `npc.isInsideRadius2D(x, y, 0, INTERACTION_DISTANCE)`.
fn near(ctx: &ScriptCtx<'_>, npc: ScriptNpc, x: i32, y: i32) -> bool {
    ctx.world().position_of(npc.object_id).is_some_and(|(nx, ny, _)| {
        let (dx, dy) = (f64::from(nx - x), f64::from(ny - y));
        (dx * dx + dy * dy).sqrt() < INTERACTION_DISTANCE
    })
}

impl Script for MithrilMinesTeleporter {
    fn name(&self) -> &'static str {
        "MithrilMinesTeleporter"
    }

    fn path(&self) -> &'static str {
        "ai/areas/MithrilMines/MithrilMinesTeleporter"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TELEPORT_CRYSTAL]);
        s.add_first_talk_id(&[TELEPORT_CRYSTAL]);
        s.add_talk_id(&[TELEPORT_CRYSTAL]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // `Integer.parseInt(event) - 1`; Java throws for text and indexes outside the array.
        let index = usize::try_from(event.trim().parse::<i32>().ok()? - 1).ok()?;
        let player = player?;
        if let Some(&(x, y, z)) = LOCS.get(index) {
            ctx.teleport_player(player, x, y, z, 0, false);
        }
        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        if near(ctx, npc, 173147, -173762) {
            return Some("32652-01.htm".to_owned());
        }
        if near(ctx, npc, 181941, -174614) {
            return Some("32652-02.htm".to_owned());
        }
        if near(ctx, npc, 179560, -182956) {
            return Some("32652-03.htm".to_owned());
        }
        // `super.onFirstTalk(npc, player)` is null: no page.
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, X};

    const NPCS: &str = r#"<npc id="32652" type="Folk" level="70" name="Teleport Crystal"/>"#;

    #[test]
    fn the_page_follows_the_place_and_a_number_teleports() {
        let mut registry = ScriptRegistry::new();
        registry.add(MithrilMinesTeleporter);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let crystal = t.spawn(TELEPORT_CRYSTAL, 20);
        t.enter(1, 70, vec![]);
        let npc = ScriptNpc {
            object_id: crystal,
            id: TELEPORT_CRYSTAL,
        };
        t.world.run_as_script("MithrilMinesTeleporter", |ctx| {
            // Far from the three places in the test world.
            assert_eq!(MithrilMinesTeleporter.on_first_talk(ctx, npc, ObjectId(1)), None);
        });
        // Move the crystal next to the second place.
        t.world
            .tele_to_location(crystal, l2_world::TeleportTarget::at(181941, -174614, 0));
        t.world.run_as_script("MithrilMinesTeleporter", |ctx| {
            assert_eq!(
                MithrilMinesTeleporter
                    .on_first_talk(ctx, npc, ObjectId(1))
                    .as_deref(),
                Some("32652-02.htm")
            );
        });
        t.world.run_as_script("MithrilMinesTeleporter", |ctx| {
            assert_eq!(
                MithrilMinesTeleporter.on_event(ctx, "x", None, Some(ObjectId(1))),
                None
            );
            assert_eq!(
                MithrilMinesTeleporter.on_event(ctx, "7", None, Some(ObjectId(1))),
                None
            );
        });
        assert_eq!(t.world.position_of(ObjectId(1)).unwrap().0, X);
        t.world.run_as_script("MithrilMinesTeleporter", |ctx| {
            MithrilMinesTeleporter.on_event(ctx, "5", None, Some(ObjectId(1)));
        });
        let (x, y, _) = t.world.position_of(ObjectId(1)).unwrap();
        assert_eq!((x, y), (LOCS[4].0, LOCS[4].1));
    }
}
