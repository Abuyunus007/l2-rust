//! Kintaijin teleports a party into the Stakato Nest once "I'm the Only One You Can Trust" is done.
// Java: dist/game/data/scripts/ai/areas/StakatoNest/StakatoNestTeleporter/StakatoNestTeleporter.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// Locations
const LOCS: [(i32, i32, i32); 5] = [
    (80456, -52322, -5640),
    (88718, -46214, -4640),
    (87464, -54221, -5120),
    (80848, -49426, -5128),
    (87682, -43291, -4128),
];
// NPC
const KINTAIJIN: i32 = 32640;
/// Java `Q00240_ImTheOnlyOneYouCanTrust.class.getSimpleName()`.
const IM_THE_ONLY_ONE_YOU_CAN_TRUST: &str = "Q00240_ImTheOnlyOneYouCanTrust";

/// Java `ai.areas.StakatoNest.StakatoNestTeleporter.StakatoNestTeleporter`.
pub struct StakatoNestTeleporter;

impl Script for StakatoNestTeleporter {
    fn name(&self) -> &'static str {
        "StakatoNestTeleporter"
    }

    fn path(&self) -> &'static str {
        "ai/areas/StakatoNest/StakatoNestTeleporter"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[KINTAIJIN]);
        s.add_talk_id(&[KINTAIJIN]);
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
            let members = ctx.world().party_members_of(player);
            // NOTE(java-quirk): the player is a member of the party too, so it is teleported with
            // the random offset first and then, below, without it. The distance is read after
            // the members before it were teleported: once the player moved, the members listed
            // after it are too far away and stay.
            for member in members {
                if ctx.distance_3d(member, player).is_some_and(|d| d < 1000.0) {
                    ctx.teleport_player(member, x, y, z, 0, true);
                }
            }
            ctx.teleport_player(player, x, y, z, 0, false);
        }
        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let completed = ctx
            .player_quest_state(player, IM_THE_ONLY_ONE_YOU_CAN_TRUST)
            .is_some_and(|qs| qs.is_completed());
        Some(if completed { "32640.htm" } else { "32640-no.htm" }.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{ScriptTest, X};

    const NPCS: &str = r#"<npc id="32640" type="Folk" level="70" name="Kintaijin"/>"#;

    /// Java `Q00240_ImTheOnlyOneYouCanTrust`: only its name and state matter here.
    struct Q00240;

    impl Script for Q00240 {
        fn name(&self) -> &'static str {
            IM_THE_ONLY_ONE_YOU_CAN_TRUST
        }
        fn path(&self) -> &'static str {
            "quests/Q00240_ImTheOnlyOneYouCanTrust"
        }
        fn id(&self) -> i32 {
            240
        }
        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    #[test]
    fn the_talk_page_follows_the_quest_and_a_choice_teleports_the_party() {
        let mut registry = ScriptRegistry::new();
        registry.add(StakatoNestTeleporter);
        registry.add(Q00240);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let kintaijin = t.spawn(KINTAIJIN, 20);
        t.enter(1, 80, vec![]);
        t.enter(2, 80, vec![]);
        t.party(1, 2);

        t.world.run_as_script("StakatoNestTeleporter", |ctx| {
            let npc = ctx.script_npc(kintaijin).unwrap();
            assert_eq!(
                StakatoNestTeleporter.on_talk(ctx, npc, ObjectId(1)).as_deref(),
                Some("32640-no.htm")
            );
        });
        t.world.run_as_script("StakatoNestTeleporter", |ctx| {
            let qs = ctx
                .quest_state_of(IM_THE_ONLY_ONE_YOU_CAN_TRUST, ObjectId(1), true)
                .unwrap();
            qs.start_quest(ctx);
            qs.exit_quest(ctx, false, false);
            let npc = ctx.script_npc(kintaijin).unwrap();
            assert_eq!(
                StakatoNestTeleporter.on_talk(ctx, npc, ObjectId(1)).as_deref(),
                Some("32640.htm")
            );
        });

        // A bad number does nothing; "2" moves both party members to the second place (the
        // second member clicks: the leader comes before it in the list, see the quirk note).
        t.world.run_as_script("StakatoNestTeleporter", |ctx| {
            assert_eq!(
                StakatoNestTeleporter.on_event(ctx, "x", None, Some(ObjectId(1))),
                None
            );
            assert_eq!(
                StakatoNestTeleporter.on_event(ctx, "9", None, Some(ObjectId(1))),
                None
            );
        });
        assert_eq!(t.world.position_of(ObjectId(1)).unwrap().0, X);
        t.world.run_as_script("StakatoNestTeleporter", |ctx| {
            StakatoNestTeleporter.on_event(ctx, "2", None, Some(ObjectId(2)));
        });
        for player in [1, 2] {
            let (x, y, _) = t.world.position_of(ObjectId(player)).unwrap();
            assert!(
                (x - LOCS[1].0).abs() < 1000 && (y - LOCS[1].1).abs() < 1000,
                "{player}"
            );
        }
    }
}
