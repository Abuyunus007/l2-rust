//! Ranku's floor of the Tower of Infinitum (the 10th floor, instance 143).
// Java: dist/game/data/scripts/ai/areas/Hellbound/Instances/RankuFloor/RankuFloor.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::{boss_killed, gatekeeper_talk};

// NPCs
const GK_9: i32 = 32752;
const CUBE: i32 = 32374;
const RANKU: i32 = 25542;

// Item
const SEAL_BREAKER_10: i32 = 15516;

// Locations
const ENTRY_POINT: (i32, i32, i32) = (-19008, 277024, -15000);
const EXIT_POINT: (i32, i32, i32) = (-19008, 277122, -13376);

// Misc
const TEMPLATE_ID: i32 = 143;
const MIN_LV: i32 = 78;

/// Java `ai.areas.Hellbound.Instances.RankuFloor.RankuFloor`.
pub struct RankuFloor;

impl Script for RankuFloor {
    fn name(&self) -> &'static str {
        "RankuFloor"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/Instances/RankuFloor"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GK_9, CUBE]);
        s.add_talk_id(&[GK_9, CUBE]);
        s.add_kill_id(&[RANKU]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = None;
        if npc.id == GK_9 {
            htmltext = gatekeeper_talk(ctx, player, TEMPLATE_ID, MIN_LV, SEAL_BREAKER_10, ENTRY_POINT);
        } else if npc.id == CUBE {
            let instance_id = ctx.world().instance_id_of(npc.object_id);
            if ctx.world().instance_world(instance_id).is_some() {
                ctx.teleport_player(player, EXIT_POINT.0, EXIT_POINT.1, EXIT_POINT.2, 0, false);
            }
        }

        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        boss_killed(ctx, npc, EXIT_POINT, (-19056, 278732, -15000, 0), CUBE);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::last_page;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32752" type="Folk" level="80" name="Gatekeeper 9"/>
        <npc id="32374" type="Folk" level="80" name="Cube"/>
        <npc id="25542" type="Monster" level="80" name="Ranku"/>"#;
    const ITEMS: &str = r#"<item id="15516" type="EtcItem" name="Seal Breaker"/>"#;

    fn talk(t: &mut ScriptTest, gk: ObjectId, player: i32) {
        t.world.run_as_script("RankuFloor", |ctx| {
            let npc = ctx.script_npc(gk).unwrap();
            let page = RankuFloor.on_talk(ctx, npc, ObjectId(player));
            ctx.show_result(ObjectId(player), page.as_deref(), Some(npc));
        });
    }

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(RankuFloor);
        let pages = Pages::of(&RankuFloor).plain(&["gk-noparty.htm", "gk-noleader.htm"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gk = t.spawn(GK_9, 20);
        t.enter(1, 80, vec![item(100, SEAL_BREAKER_10, 1)]);
        (t, gk)
    }

    #[test]
    fn the_gatekeeper_asks_for_a_party_and_its_leader() {
        let (mut t, gk) = setup();
        talk(&mut t, gk, 1);
        assert!(last_page(&t.take(), 1).contains("gk-noparty.htm"));
        t.enter(2, 80, vec![item(101, SEAL_BREAKER_10, 1)]);
        t.party(1, 2);
        talk(&mut t, gk, 2);
        assert!(last_page(&t.take(), 2).contains("gk-noleader.htm"));
    }

    #[test]
    fn the_party_enters_instance_143_and_pays_a_seal_breaker_each() {
        let (mut t, gk) = setup();
        t.enter(2, 80, vec![item(101, SEAL_BREAKER_10, 1)]);
        t.party(1, 2);
        talk(&mut t, gk, 1);
        t.ticks(60);
        assert!(t.world.player_instance_world(ObjectId(2)).is_some());
        // (The test world has no instance template data, so the world's template is -1.)
        assert_eq!(t.count(1, SEAL_BREAKER_10) + t.count(2, SEAL_BREAKER_10), 0);
    }

    #[test]
    fn ranku_s_death_outside_an_instance_does_nothing() {
        let (mut t, _gk) = setup();
        let ranku = t.spawn(RANKU, 40);
        t.kill(1, ranku);
        assert!(t.npcs_of(CUBE).is_empty());
    }
}
