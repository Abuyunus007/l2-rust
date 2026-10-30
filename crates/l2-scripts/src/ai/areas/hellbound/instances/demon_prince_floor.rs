//! The Demon Prince's floor of the Tower of Infinitum (the 6th floor, instance 142).
// Java: dist/game/data/scripts/ai/areas/Hellbound/Instances/DemonPrinceFloor/DemonPrinceFloor.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use super::{boss_killed, gatekeeper_talk};

// NPCs
const GK_4: i32 = 32748;
const CUBE: i32 = 32375;
const DEMON_PRINCE: i32 = 25540;

// Item
const SEAL_BREAKER_5: i32 = 15515;

// Locations
const ENTRY_POINT: (i32, i32, i32) = (-22208, 277056, -8239);
const EXIT_POINT: (i32, i32, i32) = (-19024, 277122, -8256);

// Misc
const TEMPLATE_ID: i32 = 142;
const MIN_LV: i32 = 78;

/// Java `ai.areas.Hellbound.Instances.DemonPrinceFloor.DemonPrinceFloor`.
pub struct DemonPrinceFloor;

impl Script for DemonPrinceFloor {
    fn name(&self) -> &'static str {
        "DemonPrinceFloor"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/Instances/DemonPrinceFloor"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GK_4, CUBE]);
        s.add_talk_id(&[GK_4, CUBE]);
        s.add_kill_id(&[DEMON_PRINCE]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = None;
        if npc.id == GK_4 {
            htmltext = gatekeeper_talk(ctx, player, TEMPLATE_ID, MIN_LV, SEAL_BREAKER_5, ENTRY_POINT);
        } else if npc.id == CUBE {
            let instance_id = ctx.world().instance_id_of(npc.object_id);
            if let Some(world) = ctx.world_mut().instance_world_mut(instance_id) {
                world.remove_allowed(player);
                ctx.teleport_player(player, EXIT_POINT.0, EXIT_POINT.1, EXIT_POINT.2, 0, false);
            }
        }

        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _killer: ObjectId, _is_summon: bool) {
        boss_killed(ctx, npc, EXIT_POINT, (-22144, 278744, -8239, 0), CUBE);
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::last_page;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32748" type="Folk" level="80" name="Gatekeeper 4"/>
        <npc id="32375" type="Folk" level="80" name="Cube"/>
        <npc id="25540" type="Monster" level="80" name="Demon Prince"/>"#;
    const ITEMS: &str = r#"<item id="15515" type="EtcItem" name="Seal Breaker"/>"#;

    fn setup(level: i32, items: Vec<l2_world::ItemData>) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(DemonPrinceFloor);
        let pages = Pages::of(&DemonPrinceFloor).plain(&["gk-noparty.htm", "gk-noleader.htm"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let gk = t.spawn(GK_4, 20);
        t.enter(1, level, items);
        (t, gk)
    }

    fn talk(t: &mut ScriptTest, gk: ObjectId, player: i32) {
        t.world.run_as_script("DemonPrinceFloor", |ctx| {
            let npc = ctx.script_npc(gk).unwrap();
            let page = DemonPrinceFloor.on_talk(ctx, npc, ObjectId(player));
            ctx.show_result(ObjectId(player), page.as_deref(), Some(npc));
        });
    }

    #[test]
    fn a_lone_player_is_not_a_party() {
        let (mut t, gk) = setup(80, vec![item(100, SEAL_BREAKER_5, 1)]);
        talk(&mut t, gk, 1);
        assert!(last_page(&t.take(), 1).contains("gk-noparty.htm"));
    }

    #[test]
    fn only_the_leader_asks() {
        let (mut t, gk) = setup(80, vec![item(100, SEAL_BREAKER_5, 1)]);
        t.enter(2, 80, vec![item(101, SEAL_BREAKER_5, 1)]);
        t.party(1, 2);
        talk(&mut t, gk, 2);
        assert!(last_page(&t.take(), 2).contains("gk-noleader.htm"));
    }

    #[test]
    fn a_party_with_a_seal_breaker_enters_and_pays_one() {
        let (mut t, gk) = setup(80, vec![item(100, SEAL_BREAKER_5, 1)]);
        t.enter(2, 80, vec![item(101, SEAL_BREAKER_5, 2)]);
        t.party(1, 2);
        talk(&mut t, gk, 1);
        t.ticks(60);
        let world = t.world.player_instance_world(ObjectId(1)).unwrap();
        assert!(world.is_allowed(ObjectId(1)) && world.is_allowed(ObjectId(2)));
        // (The test world has no instance template data, so the world's template is -1.)
        assert_eq!(t.count(1, SEAL_BREAKER_5), 0);
        assert_eq!(t.count(2, SEAL_BREAKER_5), 1);
    }

    #[test]
    fn a_member_without_a_seal_breaker_stops_the_party() {
        let (mut t, gk) = setup(80, vec![item(100, SEAL_BREAKER_5, 1)]);
        t.enter(2, 80, vec![]);
        t.party(1, 2);
        talk(&mut t, gk, 1);
        assert!(t.world.player_instance_world(ObjectId(1)).is_none());
        assert_eq!(t.count(1, SEAL_BREAKER_5), 1);
    }

    #[test]
    fn a_low_level_member_stops_the_party() {
        let (mut t, gk) = setup(80, vec![item(100, SEAL_BREAKER_5, 1)]);
        t.enter(2, 70, vec![item(101, SEAL_BREAKER_5, 1)]);
        t.party(1, 2);
        talk(&mut t, gk, 1);
        assert!(t.world.player_instance_world(ObjectId(1)).is_none());
    }
}
