//! Pagan Temple teleport AI: the mirrors of Triol and the gate keepers of the temple doors.
// Java: dist/game/data/scripts/ai/areas/PaganTemple/PaganTeleporters/PaganTeleporters.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const TRIOLS_MIRROR_1: i32 = 32039;
const TRIOLS_MIRROR_2: i32 = 32040;

// Locations
const TRIOLS_MIRROR_1_LOC: (i32, i32, i32) = (-12766, -35840, -10856);
const TRIOLS_MIRROR_2_LOC: (i32, i32, i32) = (36640, -51218, 718);
const NPCS: [i32; 6] = [32034, 32035, 32036, 32037, 32039, 32040];

// Items
const VISITORS_MARK: i32 = 8064;
const FADED_VISITORS_MARK: i32 = 8065;
const PAGANS_MARK: i32 = 8067;

// Doors
const DOOR_1: i32 = 19160001;
const DOOR_2A: i32 = 19160010;
const DOOR_2B: i32 = 19160011;

/// Java `ai.areas.PaganTemple.PaganTeleporters.PaganTeleporters`.
pub struct PaganTeleporters;

impl Script for PaganTeleporters {
    fn name(&self) -> &'static str {
        "PaganTeleporters"
    }

    fn path(&self) -> &'static str {
        "ai/areas/PaganTemple/PaganTeleporters"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&NPCS);
        s.add_talk_id(&NPCS);
        s.add_first_talk_id(&[TRIOLS_MIRROR_1, TRIOLS_MIRROR_2]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "CLOSE_DOOR_1" => {
                ctx.close_door(DOOR_1, 0);
            }
            "CLOSE_DOOR_2" => {
                ctx.close_door(DOOR_2A, 0);
                ctx.close_door(DOOR_2B, 0);
            }
            _ => {}
        }

        Some(String::new())
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let loc = match npc.id {
            TRIOLS_MIRROR_1 => Some(TRIOLS_MIRROR_1_LOC),
            TRIOLS_MIRROR_2 => Some(TRIOLS_MIRROR_2_LOC),
            _ => None,
        };
        if let Some((x, y, z)) = loc {
            ctx.teleport_player(player, x, y, z, 0, false);
        }

        Some(String::new())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        match npc.id {
            32034 => {
                if !ctx
                    .has_at_least_one_quest_item(player, &[VISITORS_MARK, FADED_VISITORS_MARK, PAGANS_MARK])
                {
                    return Some("noItem.htm".to_owned());
                }

                ctx.open_door(DOOR_1, 0);
                ctx.start_quest_timer("CLOSE_DOOR_1", 10_000, None, None, false);
                return Some("FadedMark.htm".to_owned());
            }
            32035 => {
                ctx.open_door(DOOR_1, 0);
                ctx.start_quest_timer("CLOSE_DOOR_1", 10_000, None, None, false);
                return Some("FadedMark.htm".to_owned());
            }
            32036 => {
                if !ctx.has_quest_items(player, PAGANS_MARK) {
                    return Some("noMark.htm".to_owned());
                }

                ctx.open_door(DOOR_2A, 0);
                ctx.open_door(DOOR_2B, 0);
                ctx.start_quest_timer("CLOSE_DOOR_2", 10_000, None, None, false);
                return Some("openDoor.htm".to_owned());
            }
            32037 => {
                ctx.open_door(DOOR_2A, 0);
                ctx.open_door(DOOR_2B, 0);
                ctx.start_quest_timer("CLOSE_DOOR_2", 10_000, None, None, false);
                return Some("FadedMark.htm".to_owned());
            }
            _ => {}
        }

        // `super.onTalk(npc, player)`: the default of `Script` shows nothing.
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, X, pages_to};

    const NPCS_XML: &str = r#"
        <npc id="32034" type="Folk" level="70" name="Gatekeeper"/>
        <npc id="32039" type="Folk" level="70" name="Triol's Mirror"/>"#;
    const ITEMS: &str = r#"<item id="8067" type="EtcItem" name="Pagan's Mark"/>"#;

    #[test]
    fn the_gatekeeper_shows_no_item_without_a_mark_and_the_mirror_teleports() {
        let mut registry = ScriptRegistry::new();
        registry.add(PaganTeleporters);
        let pages = Pages::of(&PaganTeleporters)
            .plain(&["noItem.htm", "FadedMark.htm"])
            .file(
                "data/html/default/32034.htm",
                r#"<html><body><a action="bypass Script PaganTeleporters">x</a></body></html>"#,
            );
        let mut t = ScriptTest::with_pages(registry, NPCS_XML, ITEMS, pages);
        let gate = t.spawn(32034, 20);
        let mirror = t.spawn(TRIOLS_MIRROR_1, 30);
        t.enter(1, 75, vec![]);
        t.take();
        t.talk(1, gate);
        t.bypass(1, "Script PaganTeleporters");
        let page = pages_to(&t.take(), 1).pop().unwrap_or_default();
        assert!(page.contains("noItem.htm"), "{page}");

        // The mirror's `onFirstTalk` teleports and shows nothing.
        t.talk(1, mirror);
        t.ticks(5);
        let (px, _py, _pz) = t.world.position_of(ObjectId(1)).unwrap();
        assert_ne!(px, X);
    }
}
