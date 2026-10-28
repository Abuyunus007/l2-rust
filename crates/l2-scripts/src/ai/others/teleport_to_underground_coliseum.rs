//! The Underground Coliseum helpers and managers: teleports in and out of the coliseum.
// Java: dist/game/data/scripts/ai/others/TeleportToUndergroundColiseum/TeleportToUndergroundColiseum.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const COLISEUM_HELPER: i32 = 32491;
const PADDIES: i32 = 32378;
const MANAGERS: [i32; 5] = [32377, 32513, 32514, 32515, 32516];

type Loc = (i32, i32, i32);

// Locations
const COLISEUM_LOCS: [Loc; 4] = [
    (-81896, -49589, -10352),
    (-82271, -49196, -10352),
    (-81886, -48784, -10352),
    (-81490, -49167, -10352),
];

const RETURN_LOCS: [Loc; 5] = [
    (-59161, -56954, -2036),
    (-59155, -56831, -2036),
    (-59299, -56955, -2036),
    (-59224, -56837, -2036),
    (-59134, -56899, -2036),
];

const MANAGERS_LOCS: [[Loc; 2]; 5] = [
    [(-84451, -45452, -10728), (-84580, -45587, -10728)],
    [(-86154, -50429, -10728), (-86118, -50624, -10728)],
    [(-82009, -53652, -10728), (-81802, -53665, -10728)],
    [(-77603, -50673, -10728), (-77586, -50503, -10728)],
    [(-79186, -45644, -10728), (-79309, -45561, -10728)],
];

/// Java `ai.others.TeleportToUndergroundColiseum.TeleportToUndergroundColiseum`.
pub struct TeleportToUndergroundColiseum;

impl Script for TeleportToUndergroundColiseum {
    fn name(&self) -> &'static str {
        "TeleportToUndergroundColiseum"
    }

    fn path(&self) -> &'static str {
        "ai/others/TeleportToUndergroundColiseum"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&MANAGERS);
        s.add_start_npc(&[COLISEUM_HELPER, PADDIES]);
        s.add_talk_id(&MANAGERS);
        s.add_talk_id(&[COLISEUM_HELPER, PADDIES]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event.ends_with(".htm") {
            return Some(event.to_owned());
        }
        let player = player?;
        if event == "return" {
            if let Some((x, y, z)) = ctx.get_random_entry(&RETURN_LOCS) {
                ctx.teleport_player(player, x, y, z, 0, false);
            }
        } else if !event.is_empty() && event.bytes().all(|b| b.is_ascii_digit()) {
            // Java `StringUtil.isNumeric` and `Integer.parseInt`.
            // NOTE(java-quirk): an index outside the managers throws in Java; nothing happens here.
            let val = event.parse::<i32>().ok().map(|v| v - 1);
            if let Some(locs) = val.and_then(|v| usize::try_from(v).ok()).and_then(|v| MANAGERS_LOCS.get(v)) {
                if let Some((x, y, z)) = ctx.get_random_entry(locs) {
                    ctx.teleport_player(player, x, y, z, 0, false);
                }
            }
        }

        None
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let loc = if MANAGERS.contains(&npc.id) {
            ctx.get_random_entry(&RETURN_LOCS)
        } else {
            ctx.get_random_entry(&COLISEUM_LOCS)
        };
        if let Some((x, y, z)) = loc {
            ctx.teleport_player(player, x, y, z, 0, false);
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
        <npc id="32377" type="Folk" level="70" name="Manager"/>"#;

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(TeleportToUndergroundColiseum);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.spawn(PADDIES, 20);
        t.spawn(32377, 40);
        t.enter(1, 60, vec![]);
        t
    }

    fn at(t: &ScriptTest) -> (i32, i32) {
        let p = t.world.position_of(ObjectId(1)).unwrap();
        (p.0, p.1)
    }

    #[test]
    fn talking_to_paddies_goes_in_and_to_a_manager_comes_out() {
        let mut t = setup();
        let ids = (t.npcs_of(PADDIES)[0], t.npcs_of(32377)[0]);
        t.world.run_as_script("TeleportToUndergroundColiseum", |ctx| {
            let npc = ctx.script_npc(ids.0).unwrap();
            TeleportToUndergroundColiseum.on_talk(ctx, npc, ObjectId(1));
        });
        assert!(COLISEUM_LOCS.iter().any(|&(x, y, _)| (x, y) == at(&t)));
        t.world.run_as_script("TeleportToUndergroundColiseum", |ctx| {
            let npc = ctx.script_npc(ids.1).unwrap();
            TeleportToUndergroundColiseum.on_talk(ctx, npc, ObjectId(1));
        });
        assert!(RETURN_LOCS.iter().any(|&(x, y, _)| (x, y) == at(&t)));
    }

    #[test]
    fn the_events_are_pages_return_and_manager_numbers() {
        let mut t = setup();
        let mut answers = Vec::new();
        t.world.run_as_script("TeleportToUndergroundColiseum", |ctx| {
            let p = Some(ObjectId(1));
            answers.push(TeleportToUndergroundColiseum.on_event(ctx, "32377-01.htm", None, p));
            answers.push(TeleportToUndergroundColiseum.on_event(ctx, "2", None, p));
        });
        assert_eq!(answers, [Some("32377-01.htm".to_owned()), None]);
        assert!(MANAGERS_LOCS[1].iter().any(|&(x, y, _)| (x, y) == at(&t)));
        t.world.run_as_script("TeleportToUndergroundColiseum", |ctx| {
            TeleportToUndergroundColiseum.on_event(ctx, "return", None, Some(ObjectId(1)));
        });
        assert!(RETURN_LOCS.iter().any(|&(x, y, _)| (x, y) == at(&t)));
    }
}
