//! The Delusion Chamber teleporters (Pathfinder Worker and the seal guardians).
// Java: dist/game/data/scripts/ai/others/DelusionTeleport/DelusionTeleport.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const NPCS: [i32; 7] = [
    32484, // Pathfinder Worker
    32658, // Guardian of Eastern Seal
    32659, // Guardian of Western Seal
    32660, // Guardian of Southern Seal
    32661, // Guardian of Northern Seal
    32662, // Guardian of Great Seal
    32663, // Guardian of Tower of Seal
];

// Location
const HALL_LOCATIONS: [(i32, i32, i32); 2] = [(-114597, -152501, -6750), (-114589, -154162, -6750)];

// Player Variables
const DELUSION_RETURN: &str = "DELUSION_RETURN";

/// Java `RETURN_LOCATIONS.get(townId)`.
fn return_location(town_id: i32) -> Option<(i32, i32, i32)> {
    match town_id {
        0 => Some((43835, -47749, -792)),     // Undefined origin, return to Rune
        7 => Some((-14023, 123677, -3112)),   // Gludio
        8 => Some((18101, 145936, -3088)),    // Dion
        10 => Some((80905, 56361, -1552)),    // Oren
        14 => Some((42772, -48062, -792)),    // Rune
        15 => Some((108469, 221690, -3592)),  // Heine
        17 => Some((85991, -142234, -1336)),  // Schuttgart
        _ => None,
    }
}

/// Java `ai.others.DelusionTeleport.DelusionTeleport`.
pub struct DelusionTeleport;

impl Script for DelusionTeleport {
    fn name(&self) -> &'static str {
        "DelusionTeleport"
    }

    fn path(&self) -> &'static str {
        "ai/others/DelusionTeleport"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPCS);
        s.add_talk_id(&NPCS);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if npc.id == NPCS[0] {
            // Pathfinder Worker
            let town_id = ctx
                .npc_position(npc)
                .and_then(|(x, y, z, _)| ctx.town_id_at(x, y, z))
                .unwrap_or(0);
            ctx.set_player_variable(player, DELUSION_RETURN, &town_id.to_string());
            if let Some((x, y, z)) = ctx.get_random_entry(&HALL_LOCATIONS) {
                ctx.teleport_player(player, x, y, z, 0, false);
            }
        } else {
            let town_id = ctx.player_variable_int(player, DELUSION_RETURN, 0);
            // NOTE(java-quirk): `RETURN_LOCATIONS.get(townId)` is null for a town that is not in the
            // map and `teleToLocation(null, true)` throws; the player stays and the variable stays.
            let Some((x, y, z)) = return_location(town_id) else {
                return None;
            };
            ctx.teleport_player(player, x, y, z, 0, true);
            ctx.remove_player_variable(player, DELUSION_RETURN);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, X, Y, Z};

    const NPC_DEFS: &str = r#"<npc id="32484" type="Folk" level="70" name="Pathfinder Worker"/>
        <npc id="32658" type="Folk" level="70" name="Guardian of Eastern Seal"/>"#;

    fn setup() -> (ScriptTest, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(DelusionTeleport);
        let page = r#"<html><body><a action="bypass Script DelusionTeleport">go</a></body></html>"#;
        let pages = [
            ("data/html/default/32484.htm", page),
            ("data/html/default/32658.htm", page),
        ];
        let mut t = ScriptTest::new(registry, NPC_DEFS, "", &pages);
        let worker = t.spawn(32484, 20);
        let guardian = t.spawn(32658, 40);
        t.enter(1, 80, vec![]);
        t.take();
        (t, worker, guardian)
    }

    fn go(t: &mut ScriptTest, npc: ObjectId) {
        t.talk(1, npc);
        t.bypass(1, "Script DelusionTeleport");
        t.take();
    }

    fn position(t: &ScriptTest) -> (i32, i32, i32) {
        t.world.position_of(ObjectId(1)).unwrap()
    }

    #[test]
    fn the_worker_remembers_the_town_and_sends_the_player_into_the_hall() {
        let (mut t, worker, _) = setup();
        go(&mut t, worker);
        let at = position(&t);
        assert!(HALL_LOCATIONS.iter().any(|&(x, y, _)| (x, y) == (at.0, at.1)), "{at:?}");
        // The test world has no town zone: Java's `town == null` gives 0.
        let player = t.world.player(ObjectId(1)).unwrap();
        assert_eq!(player.data.profile.variables.get(DELUSION_RETURN).map(String::as_str), Some("0"));
    }

    #[test]
    fn a_guardian_sends_the_player_back_and_forgets_the_town() {
        let (mut t, _, guardian) = setup();
        // (The player stands in the hall after the worker; the guardian is tested where he is.)
        t.world
            .player_mut(ObjectId(1))
            .unwrap()
            .data
            .profile
            .variables
            .insert(DELUSION_RETURN.to_owned(), "0".to_owned());
        go(&mut t, guardian);
        let at = position(&t);
        // Undefined origin (town 0) returns to Rune, within the random offset.
        assert!((at.0 - 43835).abs() < 500 && (at.1 + 47749).abs() < 500, "{at:?}");
        let player = t.world.player(ObjectId(1)).unwrap();
        assert!(!player.data.profile.variables.contains_key(DELUSION_RETURN));
        assert!(player.data.profile.variables_removed.contains(DELUSION_RETURN));
    }

    #[test]
    fn an_unknown_town_leaves_the_player_where_he_is() {
        let (mut t, _, guardian) = setup();
        t.world.player_mut(ObjectId(1)).unwrap().data.profile.variables.insert(DELUSION_RETURN.to_owned(), "99".to_owned());
        go(&mut t, guardian);
        assert_eq!(position(&t), (X, Y, Z));
    }

    #[test]
    fn the_return_table_is_javas() {
        assert_eq!(return_location(14), Some((42772, -48062, -792)));
        assert_eq!(return_location(17), Some((85991, -142234, -1336)));
        assert_eq!(return_location(1), None);
    }
}
