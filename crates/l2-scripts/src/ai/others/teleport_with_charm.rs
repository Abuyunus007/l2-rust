//! Whirpy and Tamil teleport a player to Talking Island for a gatekeeper charm or token.
// Java: dist/game/data/scripts/ai/others/TeleportWithCharm/TeleportWithCharm.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const WHIRPY: i32 = 30540;
const TAMIL: i32 = 30576;

// Items
const ORC_GATEKEEPER_CHARM: i32 = 1658;
const DWARF_GATEKEEPER_TOKEN: i32 = 1659;

// Locations
const ORC_TELEPORT: (i32, i32, i32) = (-80826, 149775, -3043);
const DWARF_TELEPORT: (i32, i32, i32) = (-80826, 149775, -3043);

/// Java `ai.others.TeleportWithCharm.TeleportWithCharm`.
pub struct TeleportWithCharm;

impl Script for TeleportWithCharm {
    fn name(&self) -> &'static str {
        "TeleportWithCharm"
    }

    fn path(&self) -> &'static str {
        "ai/others/TeleportWithCharm"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WHIRPY, TAMIL]);
        s.add_talk_id(&[WHIRPY, TAMIL]);
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        match npc.id {
            WHIRPY => {
                if ctx.has_quest_items(player, DWARF_GATEKEEPER_TOKEN) {
                    ctx.take_items(player, DWARF_GATEKEEPER_TOKEN, 1);
                    let (x, y, z) = DWARF_TELEPORT;
                    ctx.teleport_player(player, x, y, z, 0, false);
                } else {
                    return Some("30540-01.htm".to_owned());
                }
            }
            TAMIL => {
                if ctx.has_quest_items(player, ORC_GATEKEEPER_CHARM) {
                    ctx.take_items(player, ORC_GATEKEEPER_CHARM, 1);
                    let (x, y, z) = ORC_TELEPORT;
                    ctx.teleport_player(player, x, y, z, 0, false);
                } else {
                    return Some("30576-01.htm".to_owned());
                }
            }
            _ => {}
        }

        // Java `super.onTalk`.
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, item};

    const NPCS: &str = r#"<npc id="30540" type="Folk" level="70" name="Whirpy"/>
        <npc id="30576" type="Folk" level="70" name="Tamil"/>"#;
    const ITEMS: &str = r#"<item id="1658" type="EtcItem" name="Orc Gatekeeper Charm"><set name="is_questitem" val="true"/></item>
        <item id="1659" type="EtcItem" name="Dwarf Gatekeeper Token"><set name="is_questitem" val="true"/></item>"#;

    fn talk(t: &mut ScriptTest, npc: ObjectId) -> Option<String> {
        let mut out = None;
        t.world.run_as_script("TeleportWithCharm", |ctx| {
            let npc = ctx.script_npc(npc).unwrap();
            out = TeleportWithCharm.on_talk(ctx, npc, ObjectId(1));
        });
        out
    }

    #[test]
    fn without_the_item_a_page_with_it_a_teleport_and_the_item_is_taken() {
        let mut registry = ScriptRegistry::new();
        registry.add(TeleportWithCharm);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        let whirpy = t.spawn(WHIRPY, 20);
        let tamil = t.spawn(TAMIL, 40);
        t.enter(1, 20, vec![item(5000, DWARF_GATEKEEPER_TOKEN, 1)]);
        assert_eq!(talk(&mut t, tamil).as_deref(), Some("30576-01.htm"));
        assert_eq!(talk(&mut t, whirpy), None);
        assert_eq!(t.count(1, DWARF_GATEKEEPER_TOKEN), 0);
        let at = t.world.position_of(ObjectId(1)).unwrap();
        assert_eq!((at.0, at.1), (-80826, 149775));
        assert_eq!(talk(&mut t, whirpy).as_deref(), Some("30540-01.htm"));
    }
}
