//! Bernarde, the native of Hellbound who trades Darion's Badges for Holy Water.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Bernarde/Bernarde.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const BERNARDE: i32 = 32300;

// Misc
const NATIVE_TRANSFORM: i32 = 101;

// Items
const HOLY_WATER: i32 = 9673;
const DARION_BADGE: i32 = 9674;
const TREASURE: i32 = 9684;

/// Java `ai.areas.Hellbound.AI.NPC.Bernarde.Bernarde`.
pub struct Bernarde;

/// Java `isTransformed(player)`.
fn is_transformed(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.world().is_transformed(player) && ctx.world().transformation_id(player) == NATIVE_TRANSFORM
}

impl Script for Bernarde {
    fn name(&self) -> &'static str {
        "Bernarde"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Bernarde"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[BERNARDE]);
        s.add_start_npc(&[BERNARDE]);
        s.add_talk_id(&[BERNARDE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        match event {
            "HolyWater" => {
                // `exchangeItemsById(QUEST, npc, DARION_BADGE, 5, HOLY_WATER, 1, true)`: the five
                // badges go, then the water comes.
                if engine::level(ctx) == 2
                    && ctx.get_quest_items_count(player, DARION_BADGE) >= 5
                    && ctx.take_items(player, DARION_BADGE, 5)
                {
                    ctx.give_items(player, HOLY_WATER, 1);
                    return Some("32300-02b.htm".to_owned());
                }

                return Some("32300-02c.htm".to_owned());
            }
            "Treasure" => {
                if engine::level(ctx) == 3 && ctx.has_quest_items(player, TREASURE) {
                    let points = (ctx.get_quest_items_count(player, TREASURE) * 1000) as i32;
                    engine::update_trust(ctx, points, true);
                    ctx.take_items(player, TREASURE, -1);
                    return Some("32300-02d.htm".to_owned());
                }

                return Some("32300-02e.htm".to_owned());
            }
            "rumors" => {
                return Some(format!("32300-{}r.htm", engine::level(ctx)));
            }
            _ => {}
        }

        Some(event.to_owned())
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let transformed = is_transformed(ctx, player);
        let pick = |a: &'static str, b: &'static str| if transformed { a } else { b };
        let page = match engine::level(ctx) {
            0 | 1 => pick("32300-01a.htm", "32300-01.htm"),
            2 => pick("32300-02.htm", "32300-03.htm"),
            3 => pick("32300-01c.htm", "32300-03.htm"),
            4 => pick("32300-01d.htm", "32300-03.htm"),
            _ => pick("32300-01f.htm", "32300-03.htm"),
        };
        Some(page.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::engine::HellboundEngine;
    use crate::ai::areas::hellbound::testing::{last_page, set_level};
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32300" type="Folk" level="70" name="Bernarde"/>"#;
    const ITEMS: &str = r#"
        <item id="9673" type="EtcItem" name="Holy Water"/>
        <item id="9674" type="EtcItem" name="Darion's Badge"><set name="is_stackable" val="true"/></item>
        <item id="9684" type="EtcItem" name="Treasure"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(Bernarde);
        let pages = Pages::of(&Bernarde)
            .page("32300-03.htm", &["HolyWater", "Treasure", "rumors"])
            .plain(&[
                "32300-01.htm",
                "32300-02b.htm",
                "32300-02c.htm",
                "32300-02d.htm",
                "32300-02e.htm",
                "32300-2r.htm",
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let bernarde = t.spawn(BERNARDE, 20);
        (t, bernarde)
    }

    #[test]
    fn the_first_page_depends_on_the_level() {
        let (mut t, bernarde) = setup();
        t.enter(1, 80, vec![]);
        t.talk(1, bernarde);
        assert!(last_page(&t.take(), 1).contains("32300-01.htm"));
        set_level(&mut t, "Bernarde", 3);
        t.talk(1, bernarde);
        assert!(last_page(&t.take(), 1).contains("32300-03.htm"));
    }

    #[test]
    fn the_holy_water_costs_five_badges_at_level_two() {
        let (mut t, bernarde) = setup();
        t.enter(1, 80, vec![item(100, DARION_BADGE, 7)]);
        set_level(&mut t, "Bernarde", 3);
        t.talk(1, bernarde);
        t.take();
        // Wrong level.
        assert!(t.event(1, "Bernarde", "HolyWater").contains("32300-02c.htm"));
        assert_eq!(t.count(1, DARION_BADGE), 7);
        set_level(&mut t, "Bernarde", 2);
        t.talk(1, bernarde);
        t.take();
        assert!(t.event(1, "Bernarde", "HolyWater").contains("32300-02b.htm"));
        assert_eq!(t.count(1, DARION_BADGE), 2);
        assert_eq!(t.count(1, HOLY_WATER), 1);
        // Two badges are too few (the page of an answer has no links: ask again).
        t.talk(1, bernarde);
        t.take();
        assert!(t.event(1, "Bernarde", "HolyWater").contains("32300-02c.htm"));
        assert_eq!(t.count(1, HOLY_WATER), 1);
    }

    #[test]
    fn the_treasure_gives_trust_at_level_three() {
        let (mut t, bernarde) = setup();
        t.enter(1, 80, vec![item(100, TREASURE, 2)]);
        set_level(&mut t, "Bernarde", 3);
        t.world
            .run_as_script("Bernarde", |ctx| ctx.set_global_variable("HBTrust", "600000"));
        t.talk(1, bernarde);
        t.take();
        assert!(t.event(1, "Bernarde", "Treasure").contains("32300-02d.htm"));
        assert_eq!(t.count(1, TREASURE), 0);
        assert_eq!(t.world.global_variables().get_int("HBTrust", 0), 602_000);
        // Nothing left: the other page.
        t.talk(1, bernarde);
        t.take();
        assert!(t.event(1, "Bernarde", "Treasure").contains("32300-02e.htm"));
    }

    #[test]
    fn the_rumors_page_is_the_level_s() {
        let (mut t, bernarde) = setup();
        t.enter(1, 80, vec![]);
        set_level(&mut t, "Bernarde", 2);
        t.talk(1, bernarde);
        t.take();
        assert!(t.event(1, "Bernarde", "rumors").contains("32300-2r.htm"));
    }
}
