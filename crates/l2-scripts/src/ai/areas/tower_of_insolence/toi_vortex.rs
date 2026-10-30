//! The Dimension Vortexes and the gatekeepers of the Tower of Insolence: dimension stones for a
//! trip to a floor.
// Java: dist/game/data/scripts/ai/areas/TowerOfInsolence/ToIVortex/ToIVortex.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const KEPLON: i32 = 30949;
const EUCLIE: i32 = 30950;
const PITHGON: i32 = 30951;
const DIMENSION_VORTEX_1: i32 = 30952;
const DIMENSION_VORTEX_2: i32 = 30953;
const DIMENSION_VORTEX_3: i32 = 30954;
// Items
const GREEN_DIMENSION_STONE: i32 = 4401;
const BLUE_DIMENSION_STONE: i32 = 4402;
const RED_DIMENSION_STONE: i32 = 4403;
/// Java `Inventory.ADENA_ID`.
const ADENA_ID: i32 = 57;
/// `TOI_FLOORS` and `TOI_FLOOR_ITEMS`: floor "1".."10" to its place and the stone it takes.
const TOI_FLOORS: [((i32, i32, i32), i32); 10] = [
    ((114356, 13423, -5096), GREEN_DIMENSION_STONE),
    ((114666, 13380, -3608), GREEN_DIMENSION_STONE),
    ((111982, 16028, -2120), GREEN_DIMENSION_STONE),
    ((114636, 13413, -640), BLUE_DIMENSION_STONE),
    ((114152, 19902, 928), BLUE_DIMENSION_STONE),
    ((117131, 16044, 1944), BLUE_DIMENSION_STONE),
    ((113026, 17687, 2952), RED_DIMENSION_STONE),
    ((115571, 13723, 3960), RED_DIMENSION_STONE),
    ((114649, 14144, 4976), RED_DIMENSION_STONE),
    ((118507, 16605, 5984), RED_DIMENSION_STONE),
];

/// Java `ai.areas.TowerOfInsolence.ToIVortex.ToIVortex`.
pub struct ToIVortex;

impl Script for ToIVortex {
    fn name(&self) -> &'static str {
        "ToIVortex"
    }

    fn path(&self) -> &'static str {
        "ai/areas/TowerOfInsolence/ToIVortex"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        let npcs = [
            KEPLON,
            EUCLIE,
            PITHGON,
            DIMENSION_VORTEX_1,
            DIMENSION_VORTEX_2,
            DIMENSION_VORTEX_3,
        ];
        s.add_start_npc(&npcs);
        s.add_talk_id(&npcs);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let npc_id = npc?.id;
        let player = player?;
        match event {
            "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" | "10" => {
                let floor = event.parse::<usize>().ok()?;
                let ((x, y, z), item_id) = TOI_FLOORS[floor - 1];
                if ctx.has_quest_items(player, item_id) {
                    ctx.take_items(player, item_id, 1);
                    ctx.teleport_player(player, x, y, z, 0, true);
                } else {
                    return Some("no-stones.htm".to_owned());
                }
            }
            "GREEN" | "BLUE" | "RED" => {
                if ctx.adena_of(player) >= 10000 {
                    ctx.take_items(player, ADENA_ID, 10000);
                    let stone = match event {
                        "GREEN" => GREEN_DIMENSION_STONE,
                        "BLUE" => BLUE_DIMENSION_STONE,
                        _ => RED_DIMENSION_STONE,
                    };
                    ctx.give_items(player, stone, 1);
                } else {
                    return Some(format!("{npc_id}no-adena.htm"));
                }
            }
            _ => {}
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="30949" type="Folk" level="70" name="Keplon"/>"#;
    const ITEMS: &str = r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="4401" type="EtcItem" name="Green"/><item id="4402" type="EtcItem" name="Blue"/>"#;

    #[test]
    fn stones_cost_adena_and_a_stone_buys_a_floor() {
        let mut registry = ScriptRegistry::new();
        registry.add(ToIVortex);
        let pages = Pages::of(&ToIVortex)
            .page("30949.htm", &["GREEN", "BLUE", "1", "4"])
            .page("no-stones.htm", &["1", "4"])
            .page("30949no-adena.htm", &["1", "4"])
            .npcs(&[KEPLON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let keplon = t.spawn(KEPLON, 20);
        t.enter(1, 70, vec![item(500, 57, 10_000)]);
        t.talk(1, keplon);
        // `Script` pages of a plain script: open the page the NPC shows by hand.
        t.world.run_as_script("ToIVortex", |ctx| {
            ctx.show_html_file(ObjectId(1), "30949.htm", ctx.script_npc(keplon));
        });
        t.take();
        assert_eq!(t.event(1, "ToIVortex", "GREEN"), "");
        assert_eq!(t.count(1, 57), 0);
        assert_eq!(t.count(1, GREEN_DIMENSION_STONE), 1);
        // No adena left.
        assert!(t.event(1, "ToIVortex", "BLUE").contains("30949no-adena.htm"));
        // Floor 4 needs a blue stone.
        assert!(t.event(1, "ToIVortex", "4").contains("no-stones.htm"));
        // The green stone opens floor 1 and is gone.
        assert_eq!(t.event(1, "ToIVortex", "1"), "");
        assert_eq!(t.count(1, GREEN_DIMENSION_STONE), 0);
        let (x, y, _) = t.world.position_of(ObjectId(1)).unwrap();
        assert!((x - 114356).abs() < 200 && (y - 13423).abs() < 200);
    }
}
