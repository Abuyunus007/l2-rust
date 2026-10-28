//! The Tower of Infinitum: Jerian lets a party with the Pass skill in, the gatekeepers move it
//! up and down.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/Zones/TowerOfInfinitum/TowerOfInfinitum.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const JERIAN: i32 = 32302;
const GK_FIRST: i32 = 32745;
const GK_LAST: i32 = 32752;

// Skills
const PASS_SKILL: i32 = 2357;

type Location = (i32, i32, i32);

/// Java `TELE_COORDS`: by gatekeeper, `[up, down]`.
fn tele_coords(npc_id: i32) -> Option<[Option<Location>; 2]> {
    Some(match npc_id {
        32745 => [Some((-22208, 277122, -13376)), None],
        32746 => [Some((-22208, 277106, -11648)), Some((-22208, 277074, -15040))],
        32747 => [Some((-22208, 277120, -9920)), Some((-22208, 277120, -13376))],
        32748 => [Some((-19024, 277126, -8256)), Some((-22208, 277106, -11648))],
        32749 => [Some((-19024, 277106, -9920)), Some((-22208, 277122, -9920))],
        32750 => [Some((-19008, 277100, -11648)), Some((-19024, 277122, -8256))],
        32751 => [Some((-19008, 277100, -13376)), Some((-19008, 277106, -9920))],
        32752 => [Some((14602, 283179, -7500)), Some((-19008, 277100, -11648))],
        _ => return None,
    })
}

const ENTER_LOCATION: Location = (-22204, 277056, -15023);

/// Java `ai.areas.Hellbound.AI.Zones.TowerOfInfinitum.TowerOfInfinitum`.
pub struct TowerOfInfinitum;

impl Script for TowerOfInfinitum {
    fn name(&self) -> &'static str {
        "TowerOfInfinitum"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/Zones/TowerOfInfinitum"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JERIAN]);
        s.add_talk_id(&[JERIAN]);

        let gatekeepers: Vec<i32> = (GK_FIRST..=GK_LAST).collect();
        s.add_start_npc(&gatekeepers);
        s.add_talk_id(&gatekeepers);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let mut htmltext = Some(event.to_owned());
        let npc = npc?;
        let player = player?;
        let npc_id = npc.id;
        if event.eq_ignore_ascii_case("enter") && npc_id == JERIAN {
            if engine::level(ctx) >= 11 {
                if ctx.world().is_party_leader(player) {
                    let members = ctx.world().party_members_of(player);
                    for &member in &members {
                        if !ctx.check_if_in_range(300, member, npc.object_id, true)
                            || !ctx.is_affected_by_skill(member, PASS_SKILL)
                        {
                            return Some("32302-02.htm".to_owned());
                        }
                    }

                    for member in members {
                        ctx.teleport_player(
                            member,
                            ENTER_LOCATION.0,
                            ENTER_LOCATION.1,
                            ENTER_LOCATION.2,
                            0,
                            true,
                        );
                    }

                    htmltext = None;
                } else {
                    htmltext = Some("32302-02a.htm".to_owned());
                }
            } else {
                htmltext = Some("32302-02b.htm".to_owned());
            }
        } else if (event.eq_ignore_ascii_case("up") || event.eq_ignore_ascii_case("down"))
            && (GK_FIRST..=GK_LAST).contains(&npc_id)
        {
            let direction = if event.eq_ignore_ascii_case("up") { 0 } else { 1 };
            if ctx.world().party_of(player).is_none() {
                htmltext = Some("gk-noparty.htm".to_owned());
            } else if !ctx.world().is_party_leader(player) {
                htmltext = Some("gk-noreq.htm".to_owned());
            } else {
                let members = ctx.world().party_members_of(player);
                for &member in &members {
                    let dz = ctx
                        .world()
                        .position_of(member)
                        .zip(ctx.world().position_of(npc.object_id))
                        .map_or(i32::MAX, |(m, n)| (m.2 - n.2).abs());
                    if !ctx.check_if_in_range(1000, member, npc.object_id, false) || dz > 100 {
                        return Some("gk-noreq.htm".to_owned());
                    }
                }

                if let Some(Some((x, y, z))) = tele_coords(npc_id).map(|t| t[direction]) {
                    for member in members {
                        ctx.teleport_player(member, x, y, z, 0, true);
                    }
                }

                htmltext = None;
            }
        }

        htmltext
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::set_level;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="32302" type="Folk" level="80" name="Jerian"/>
        <npc id="32746" type="Folk" level="80" name="Gatekeeper"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(crate::ai::areas::hellbound::engine::HellboundEngine);
        registry.add(TowerOfInfinitum);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        t
    }

    fn event(t: &mut ScriptTest, event: &str, npc: ObjectId, player: i32) -> Option<String> {
        let mut result = None;
        t.world.run_as_script("TowerOfInfinitum", |ctx| {
            let npc = ctx.script_npc(npc);
            result = TowerOfInfinitum.on_event(ctx, event, npc, Some(ObjectId(player)));
        });
        result
    }

    #[test]
    fn jerian_wants_level_eleven_and_a_party_leader() {
        let mut t = setup();
        let jerian = t.spawn(JERIAN, 20);
        assert_eq!(
            event(&mut t, "enter", jerian, 1).as_deref(),
            Some("32302-02b.htm")
        );
        set_level(&mut t, "TowerOfInfinitum", 11);
        // Level eleven, but no party: the player is not a leader.
        assert_eq!(
            event(&mut t, "enter", jerian, 1).as_deref(),
            Some("32302-02a.htm")
        );
        // A party whose members lack the Pass skill.
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        assert_eq!(event(&mut t, "enter", jerian, 1).as_deref(), Some("32302-02.htm"));
    }

    #[test]
    fn the_gatekeepers_answer_the_party_state() {
        let mut t = setup();
        let gk = t.spawn(32746, 20);
        assert_eq!(event(&mut t, "up", gk, 1).as_deref(), Some("gk-noparty.htm"));
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        assert_eq!(event(&mut t, "up", gk, 2).as_deref(), Some("gk-noreq.htm"));
        // The leader and the member are near: both go up.
        assert_eq!(event(&mut t, "up", gk, 1), None);
        t.ticks(100);
        let (x, y, _) = t.world.position_of(ObjectId(2)).unwrap();
        assert!((x + 22208).abs() < 200 && (y - 277106).abs() < 200, "{x} {y}");
    }

    #[test]
    fn the_table_has_the_eight_gatekeepers() {
        assert!((GK_FIRST..=GK_LAST).all(|id| tele_coords(id).is_some()));
        assert_eq!(tele_coords(32745).unwrap()[1], None);
        assert_eq!(tele_coords(32752).unwrap()[0], Some((14602, 283179, -7500)));
    }
}
