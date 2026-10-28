//! Paddies sends the player to the Fantasy Isle and back, from the town teleporters.
// Java: dist/game/data/scripts/ai/others/TeleportToFantasy/TeleportToFantasy.java

use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const PADDIES: i32 = 32378;

// Locations
const ISLE_LOCATIONS: [(i32, i32, i32); 4] = [
    (-58752, -56898, -2032),
    (-59716, -57868, -2032),
    (-60691, -56893, -2032),
    (-59720, -55921, -2032),
];

/// Java `TELEPORTER_LOCATIONS.get(id)`.
fn teleporter_location(npc_id: i32) -> Option<(i32, i32, i32)> {
    match npc_id {
        30320 => Some((-80826, 149775, -3043)),  // Richlin
        30256 => Some((-12672, 122776, -3116)),  // Bella
        30059 => Some((15670, 142983, -2705)),   // Trisha
        30080 => Some((83400, 147943, -3404)),   // Clarissa
        30899 => Some((111409, 219364, -3545)),  // Flauen
        30177 => Some((82956, 53162, -1495)),    // Valentina
        30848 => Some((146331, 25762, -2018)),   // Elisa
        30233 => Some((116819, 76994, -2714)),   // Esmeralda
        31320 => Some((43835, -47749, -792)),    // Ilyana
        31275 => Some((147930, -55281, -2728)),  // Tatiana
        31964 => Some((87386, -143246, -1293)),  // Bilia
        _ => None,
    }
}

const TELEPORTERS: [i32; 11] = [
    30320, 30256, 30059, 30080, 30899, 30177, 30848, 30233, 31320, 31275, 31964,
];

// Other
const FANTASY_RETURN: &str = "FANTASY_RETURN";

/// Java `ai.others.TeleportToFantasy.TeleportToFantasy`.
pub struct TeleportToFantasy;

impl Script for TeleportToFantasy {
    fn name(&self) -> &'static str {
        "TeleportToFantasy"
    }

    fn path(&self) -> &'static str {
        "ai/others/TeleportToFantasy"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PADDIES]);
        s.add_start_npc(&TELEPORTERS);
        s.add_talk_id(&[PADDIES]);
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
            if npc.id == PADDIES {
                let return_id = ctx.player_variable_int(player, FANTASY_RETURN, -1);
                if return_id > 30000 {
                    // NOTE(java-quirk): `TELEPORTER_LOCATIONS.get(returnId)` is null for an id that
                    // is not a teleporter and `teleToLocation(null)` throws; the player stays.
                    let (x, y, z) = teleporter_location(return_id)?;
                    ctx.teleport_player(player, x, y, z, 0, false);
                    ctx.remove_player_variable(player, FANTASY_RETURN);
                } else {
                    ctx.broadcast_npc_say_text(
                        npc,
                        ChatType::General,
                        "If your means of arrival was a bit unconventional, then I'll be sending you back to Rune Township, which is the nearest town.",
                    );
                    let (x, y, z) = teleporter_location(31320)?; // Rune
                    ctx.teleport_player(player, x, y, z, 0, false);
                }
            } else {
                if let Some((x, y, z)) = ctx.get_random_entry(&ISLE_LOCATIONS) {
                    ctx.teleport_player(player, x, y, z, 0, false);
                }
                ctx.set_player_variable(player, FANTASY_RETURN, &npc.id.to_string());
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="32378" type="Folk" level="70" name="Paddies"/>
        <npc id="31320" type="Folk" level="70" name="Ilyana"/>"#;

    fn setup() -> (ScriptTest, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(TeleportToFantasy);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        let paddies = t.spawn(PADDIES, 20);
        let ilyana = t.spawn(31320, 40);
        t.enter(1, 60, vec![]);
        t.take();
        (t, paddies, ilyana)
    }

    fn go(t: &mut ScriptTest, npc: ObjectId) {
        t.world.run_as_script("TeleportToFantasy", |ctx| {
            let npc = ctx.script_npc(npc).unwrap();
            TeleportToFantasy.on_event(ctx, "teleport", Some(npc), Some(ObjectId(1)));
        });
    }

    fn var(t: &ScriptTest, name: &str) -> Option<String> {
        t.world.player(ObjectId(1)).unwrap().data.profile.variables.get(name).cloned()
    }

    fn at(t: &ScriptTest) -> (i32, i32) {
        let p = t.world.position_of(ObjectId(1)).unwrap();
        (p.0, p.1)
    }

    #[test]
    fn a_town_teleporter_sends_the_player_to_the_isle_and_remembers_it() {
        let (mut t, _, ilyana) = setup();
        go(&mut t, ilyana);
        assert!(ISLE_LOCATIONS.iter().any(|&(x, y, _)| (x, y) == at(&t)));
        assert_eq!(var(&t, FANTASY_RETURN).as_deref(), Some("31320"));
    }

    #[test]
    fn paddies_sends_the_player_back_to_where_he_came_from() {
        let (mut t, paddies, ilyana) = setup();
        go(&mut t, ilyana);
        go(&mut t, paddies);
        assert_eq!(at(&t), (43835, -47749));
        assert!(var(&t, FANTASY_RETURN).is_none());
    }

    #[test]
    fn paddies_without_a_return_town_sends_to_rune_with_a_line() {
        let (mut t, paddies, _) = setup();
        go(&mut t, paddies);
        assert_eq!(at(&t), (43835, -47749));
        assert_eq!(teleporter_location(31320), Some((43835, -47749, -792)));
    }
}
