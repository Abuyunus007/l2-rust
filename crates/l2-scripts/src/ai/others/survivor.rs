//! The Survivor of the Seed of Annihilation / Hall: a teleport for 150000 adena.
// Java: dist/game/data/scripts/ai/others/Survivor/Survivor.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const SURVIVOR: i32 = 32632;

// Misc
const MIN_LEVEL: i32 = 75;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;

// Location
const TELEPORT: (i32, i32, i32) = (-149406, 255247, -80);

/// Java `ai.others.Survivor.Survivor`.
pub struct Survivor;

impl Script for Survivor {
    fn name(&self) -> &'static str {
        "Survivor"
    }

    fn path(&self) -> &'static str {
        "ai/others/Survivor"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SURVIVOR]);
        s.add_talk_id(&[SURVIVOR]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        if event == "32632-2.htm" {
            if ctx.player_level(player) < MIN_LEVEL {
                return Some("32632-3.htm".to_owned());
            } else if ctx.adena_of(player) < 150000 {
                return Some(event.to_owned());
            } else {
                ctx.take_items(player, ADENA_ID, 150000);
                ctx.teleport_player(player, TELEPORT.0, TELEPORT.1, TELEPORT.2, 0, false);
                return None;
            }
        }

        Some(event.to_owned())
    }

    fn on_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("32632-1.htm".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item};

    const NPCS: &str = r#"<npc id="32632" type="Folk" level="70" name="Survivor"/>"#;
    const ITEMS: &str =
        r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>"#;

    fn event(t: &mut ScriptTest, player: i32, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script("Survivor", |ctx| {
            out = Survivor.on_event(ctx, event, None, Some(ObjectId(player)));
        });
        out
    }

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Survivor);
        ScriptTest::new(registry, NPCS, ITEMS, &[])
    }

    #[test]
    fn a_low_level_is_refused_and_a_poor_one_gets_the_page_again() {
        let mut t = setup();
        t.enter(1, 74, vec![item(5000, 57, 200000)]);
        assert_eq!(event(&mut t, 1, "32632-2.htm").as_deref(), Some("32632-3.htm"));
        t.enter(2, 80, vec![item(5001, 57, 100)]);
        assert_eq!(event(&mut t, 2, "32632-2.htm").as_deref(), Some("32632-2.htm"));
        assert_eq!(event(&mut t, 2, "anything").as_deref(), Some("anything"));
    }

    #[test]
    fn the_fee_is_taken_and_the_player_goes() {
        let mut t = setup();
        t.enter(1, 80, vec![item(5000, 57, 200000)]);
        assert_eq!(event(&mut t, 1, "32632-2.htm"), None);
        assert_eq!(t.count(1, 57), 50000);
        let at = t.world.position_of(ObjectId(1)).unwrap();
        assert_eq!((at.0, at.1), (TELEPORT.0, TELEPORT.1));
    }
}
