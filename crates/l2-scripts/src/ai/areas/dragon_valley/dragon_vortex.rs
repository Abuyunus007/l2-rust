//! Dragon Vortex AI: a Large Dragon Bone calls one of the raid bosses of the Dragon Valley.
// Java: dist/game/data/scripts/ai/areas/DragonValley/DragonVortex/DragonVortex.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const DRAGON_VORTEX: i32 = 32871;

// Raids
const EMERALD_HORN: i32 = 25718;
const DUST_RIDER: i32 = 25719;
const BLEEDING_FLY: i32 = 25720;
const BLACKDAGGER_WING: i32 = 25721;
const SHADOW_SUMMONER: i32 = 25722;
const SPIKE_SLASHER: i32 = 25723;
const MUSCLE_BOMBER: i32 = 25724;

// Item
const LARGE_DRAGON_BONE: i32 = 17248;

// Variables
const I_QUEST0: &str = "I_QUEST0";

// Locations
const SPOT_1: (i32, i32, i32) = (92744, 114045, -3072);
const SPOT_2: (i32, i32, i32) = (110112, 124976, -3624);
const SPOT_3: (i32, i32, i32) = (121637, 113657, -3792);
const SPOT_4: (i32, i32, i32) = (109346, 111849, -3040);

/// Java `ai.areas.DragonValley.DragonVortex.DragonVortex`.
pub struct DragonVortex;

impl Script for DragonVortex {
    fn name(&self) -> &'static str {
        "DragonVortex"
    }

    fn path(&self) -> &'static str {
        "ai/areas/DragonValley/DragonVortex"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DRAGON_VORTEX]);
        s.add_first_talk_id(&[DRAGON_VORTEX]);
        s.add_talk_id(&[DRAGON_VORTEX]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "RAIDBOSS" => {
                let (npc, player) = (npc?, player?);
                if ctx.has_quest_items(player, LARGE_DRAGON_BONE) {
                    let busy = ctx.world_mut().npc_variables_mut(npc.object_id).get_bool(I_QUEST0, false);
                    if !busy {
                        ctx.take_items(player, LARGE_DRAGON_BONE, 1);
                        let random = ctx.get_random(100);
                        let raid = if random < 3 {
                            MUSCLE_BOMBER
                        } else if random < 8 {
                            SHADOW_SUMMONER
                        } else if random < 15 {
                            SPIKE_SLASHER
                        } else if random < 25 {
                            BLACKDAGGER_WING
                        } else if random < 45 {
                            BLEEDING_FLY
                        } else if random < 67 {
                            DUST_RIDER
                        } else {
                            EMERALD_HORN
                        };

                        let x = ctx.world().position_of(npc.object_id).map(|(x, _, _)| x);
                        let loc = match x {
                            Some(92225) => Some(SPOT_1),
                            Some(110116) => Some(SPOT_2),
                            Some(121172) => Some(SPOT_3),
                            Some(108924) => Some(SPOT_4),
                            _ => None,
                        };

                        ctx.world_mut()
                            .npc_variables_mut(npc.object_id)
                            .set_bool(I_QUEST0, true);
                        // NOTE(java-quirk): a Vortex at none of the four places has no `loc`;
                        // Java's `addSpawn(raid, null, ..)` throws and the timer below is never
                        // started, which `?` keeps.
                        let (lx, ly, lz) = loc?;
                        ctx.add_spawn(raid, lx, ly, lz, 0, false, 0, 0);
                        ctx.start_quest_timer("CANSPAWN", 60_000, Some(npc), None, false);
                    } else {
                        return Some("32871-02.html".to_owned());
                    }
                } else {
                    return Some("32871-01.html".to_owned());
                }
            }
            "CANSPAWN" => {
                let npc = npc?;
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_bool(I_QUEST0, false);
            }
            _ => {}
        }

        None
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32871" type="Folk" level="70" name="Dragon Vortex"/>"#;
    const ITEMS: &str = r#"<item id="17248" type="EtcItem" name="Large Dragon Bone"/>"#;

    #[test]
    fn the_bone_is_taken_and_the_vortex_is_busy_for_a_minute() {
        let mut registry = ScriptRegistry::new();
        registry.add(DragonVortex);
        let pages = Pages::of(&DragonVortex)
            .page("32871.html", &["RAIDBOSS"])
            .plain(&["32871-01.html", "32871-02.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let vortex = t.spawn(DRAGON_VORTEX, 20);
        t.enter(1, 80, vec![]);
        t.talk(1, vortex);
        t.take();
        // No bone: 32871-01.
        assert!(t.event(1, "DragonVortex", "RAIDBOSS").contains("32871-01"));

        t.enter(2, 80, vec![item(9001, LARGE_DRAGON_BONE, 2)]);
        t.talk(2, vortex);
        t.take();
        // The test Vortex stands at none of the four places: the bone is taken and the
        // vortex stays busy (the quirk above), then a second bone is refused with 32871-02.
        t.event(2, "DragonVortex", "RAIDBOSS");
        assert_eq!(t.count(2, LARGE_DRAGON_BONE), 1);
        assert!(t.event(2, "DragonVortex", "RAIDBOSS").contains("32871-02"));
        assert_eq!(t.count(2, LARGE_DRAGON_BONE), 1);
    }
}
