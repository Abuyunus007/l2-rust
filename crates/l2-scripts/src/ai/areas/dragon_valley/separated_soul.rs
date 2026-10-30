//! Separated Soul teleport AI: the souls of the Dragon Valley send characters of level 80 or more
//! across the valley and the Lair of Antharas, and synthesise the Antharas' Blood Crystal.
// Java: dist/game/data/scripts/ai/areas/DragonValley/SeparatedSoul/SeparatedSoul.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const SEPARATED_SOULS: [i32; 8] = [32864, 32865, 32866, 32867, 32868, 32869, 32870, 32891];

// Items
const WILL_OF_ANTHARAS: i32 = 17266;
const SEALED_BLOOD_CRYSTAL: i32 = 17267;
const ANTHARAS_BLOOD_CRYSTAL: i32 = 17268;

// Misc
const MIN_LEVEL: i32 = 80;

// Locations
const LOCATIONS: [(&str, (i32, i32, i32)); 8] = [
    ("HuntersVillage", (117031, 76769, -2696)),
    ("AntharasLair", (131116, 114333, -3704)),
    ("AntharasLairDeep", (148447, 110582, -3944)),
    ("AntharasLairMagicForceFieldBridge", (146129, 111232, -3568)),
    ("DragonValley", (73122, 118351, -3714)),
    ("DragonValleyCenter", (99218, 110283, -3696)),
    ("DragonValleyNorth", (116992, 113716, -3056)),
    ("DragonValleySouth", (113203, 121063, -3712)),
];

/// Java `ai.areas.DragonValley.SeparatedSoul.SeparatedSoul`.
pub struct SeparatedSoul;

impl Script for SeparatedSoul {
    fn name(&self) -> &'static str {
        "SeparatedSoul"
    }

    fn path(&self) -> &'static str {
        "ai/areas/DragonValley/SeparatedSoul"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&SEPARATED_SOULS);
        s.add_talk_id(&SEPARATED_SOULS);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let loc = LOCATIONS.iter().find(|(name, _)| *name == event).map(|(_, loc)| *loc);
        if let Some((x, y, z)) = loc {
            if ctx.player_level(player) >= MIN_LEVEL {
                ctx.teleport_player(player, x, y, z, 0, true);
            } else {
                return Some("no-level.htm".to_owned());
            }
        } else if event == "Synthesis" {
            // Request Item Synthesis
            if ctx.has_all_quest_items(player, &[WILL_OF_ANTHARAS, SEALED_BLOOD_CRYSTAL]) {
                ctx.take_items(player, WILL_OF_ANTHARAS, 1);
                ctx.take_items(player, SEALED_BLOOD_CRYSTAL, 1);
                ctx.give_items(player, ANTHARAS_BLOOD_CRYSTAL, 1);
            } else {
                return Some("no-items.htm".to_owned());
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32864" type="Folk" level="70" name="Separated Soul"/>"#;
    const ITEMS: &str = r#"
        <item id="17266" type="EtcItem" name="Will of Antharas"/>
        <item id="17267" type="EtcItem" name="Sealed Blood Crystal"/>
        <item id="17268" type="EtcItem" name="Antharas' Blood Crystal"/>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(SeparatedSoul);
        let pages = Pages::of(&SeparatedSoul)
            .page("32864.htm", &["Synthesis", "DragonValley"])
            .plain(&["no-level.htm", "no-items.htm"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let soul = t.spawn(32864, 20);
        (t, soul)
    }

    #[test]
    fn synthesis_swaps_the_two_items_for_the_blood_crystal() {
        let (mut t, soul) = setup();
        t.enter(
            1,
            80,
            vec![item(9001, WILL_OF_ANTHARAS, 1), item(9002, SEALED_BLOOD_CRYSTAL, 1)],
        );
        t.enter(2, 79, vec![]);
        t.talk(1, soul);
        t.take();
        // The pages of the NPC come from the stand-in default page; offer the links by hand.
        t.world.run_as_script("SeparatedSoul", |ctx| {
            let out = SeparatedSoul.on_event(ctx, "Synthesis", None, Some(ObjectId(1)));
            assert_eq!(out, None);
            let out = SeparatedSoul.on_event(ctx, "DragonValley", None, Some(ObjectId(2)));
            assert_eq!(out.as_deref(), Some("no-level.htm"));
            let out = SeparatedSoul.on_event(ctx, "Synthesis", None, Some(ObjectId(2)));
            assert_eq!(out.as_deref(), Some("no-items.htm"));
        });
        assert_eq!(t.count(1, ANTHARAS_BLOOD_CRYSTAL), 1);
        assert_eq!(t.count(1, WILL_OF_ANTHARAS), 0);
        assert_eq!(t.count(1, SEALED_BLOOD_CRYSTAL), 0);
    }
}
