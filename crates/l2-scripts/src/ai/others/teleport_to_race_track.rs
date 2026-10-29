//! The Race Manager sends the player to the Monster Race Track and back, from the town teleporters.
// Java: dist/game/data/scripts/ai/others/TeleportToRaceTrack/TeleportToRaceTrack.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const RACE_MANAGER: i32 = 30995;

// Locations
const RACE_TRACK_TELEPORT: (i32, i32, i32) = (12661, 181687, -3540);

/// Java `TELEPORTER_LOCATIONS.get(id)`.
fn teleporter_location(npc_id: i32) -> Option<(i32, i32, i32)> {
    match npc_id {
        30320 => Some((-80826, 149775, -3043)), // Richlin
        30256 => Some((-12672, 122776, -3116)), // Bella
        30059 => Some((15670, 142983, -2705)),  // Trisha
        30080 => Some((83400, 147943, -3404)),  // Clarissa
        30899 => Some((111409, 219364, -3545)), // Flauen
        30177 => Some((82956, 53162, -1495)),   // Valentina
        30848 => Some((146331, 25762, -2018)),  // Elisa
        30233 => Some((116819, 76994, -2714)),  // Esmeralda
        31320 => Some((43835, -47749, -792)),   // Ilyana
        31275 => Some((147930, -55281, -2728)), // Tatiana
        31964 => Some((87386, -143246, -1293)), // Bilia
        31210 => Some((12882, 181053, -3560)),  // Race Track Gatekeeper
        _ => None,
    }
}

const TELEPORTERS: [i32; 12] = [
    30320, 30256, 30059, 30080, 30899, 30177, 30848, 30233, 31320, 31275, 31964, 31210,
];

// Other
const MONSTER_RETURN: &str = "MONSTER_RETURN";

/// Java `ai.others.TeleportToRaceTrack.TeleportToRaceTrack`.
pub struct TeleportToRaceTrack;

impl Script for TeleportToRaceTrack {
    fn name(&self) -> &'static str {
        "TeleportToRaceTrack"
    }

    fn path(&self) -> &'static str {
        "ai/others/TeleportToRaceTrack"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RACE_MANAGER]);
        s.add_start_npc(&TELEPORTERS);
        s.add_talk_id(&[RACE_MANAGER]);
        s.add_talk_id(&TELEPORTERS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("TELEPORT") {
            let npc = npc?;
            let player = player?;
            if npc.id == RACE_MANAGER {
                let return_id = ctx.player_variable_int(player, MONSTER_RETURN, -1);
                if return_id > 30000 {
                    // NOTE(java-quirk): a `returnId` that is not a teleporter gives a null
                    // location and `teleToLocation(null)` throws; the player stays.
                    let (x, y, z) = teleporter_location(return_id)?;
                    ctx.teleport_player(player, x, y, z, 0, false);
                    ctx.remove_player_variable(player, MONSTER_RETURN);
                } else {
                    let (x, y, z) = teleporter_location(30059)?; // Default to Dion
                    ctx.teleport_player(player, x, y, z, 0, false);
                }
            } else {
                let (x, y, z) = RACE_TRACK_TELEPORT;
                ctx.teleport_player(player, x, y, z, 0, false);
                ctx.set_player_variable(player, MONSTER_RETURN, &npc.id.to_string());
            }

            return None;
        }

        // Java `super.onEvent`.
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="30995" type="Folk" level="70" name="Race Manager"/>
        <npc id="30080" type="Folk" level="70" name="Clarissa"/>"#;

    fn setup() -> (ScriptTest, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(TeleportToRaceTrack);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let manager = t.spawn(RACE_MANAGER, 20);
        let clarissa = t.spawn(30080, 40);
        t.enter(1, 60, vec![]);
        (t, manager, clarissa)
    }

    fn go(t: &mut ScriptTest, npc: ObjectId) {
        t.world.run_as_script("TeleportToRaceTrack", |ctx| {
            let npc = ctx.script_npc(npc).unwrap();
            TeleportToRaceTrack.on_event(ctx, "TELEPORT", Some(npc), Some(ObjectId(1)));
        });
    }

    fn var(t: &ScriptTest, name: &str) -> Option<String> {
        t.world
            .player(ObjectId(1))
            .unwrap()
            .data
            .profile
            .variables
            .get(name)
            .cloned()
    }

    fn at(t: &ScriptTest) -> (i32, i32) {
        let p = t.world.position_of(ObjectId(1)).unwrap();
        (p.0, p.1)
    }

    #[test]
    fn to_the_track_and_back_to_the_town() {
        let (mut t, manager, clarissa) = setup();
        go(&mut t, clarissa);
        assert_eq!(at(&t), (12661, 181687));
        assert_eq!(var(&t, MONSTER_RETURN).as_deref(), Some("30080"));
        go(&mut t, manager);
        assert_eq!(at(&t), (83400, 147943));
        assert!(var(&t, MONSTER_RETURN).is_none());
    }

    #[test]
    fn without_a_return_town_the_manager_sends_to_dion() {
        let (mut t, manager, _) = setup();
        go(&mut t, manager);
        assert_eq!(at(&t), (15670, 142983));
    }
}
