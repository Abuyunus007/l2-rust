//! Kief, the native who turns badges and life forces into Hellbound's trust.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Kief/Kief.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const KIEF: i32 = 32354;

// Items
const BOTTLE: i32 = 9672; // Magic Bottle
const DARION_BADGE: i32 = 9674; // Darion's Badge
const DIM_LIFE_FORCE: i32 = 9680; // Dim Life Force
const LIFE_FORCE: i32 = 9681; // Life Force
const CONTAINED_LIFE_FORCE: i32 = 9682; // Contained Life Force
const STINGER: i32 = 10012; // Scorpion Poison Stinger

/// Java `ai.areas.Hellbound.AI.NPC.Kief.Kief`.
pub struct Kief;

/// The three life force cases of `onEvent`: all of one item go for `points` each.
fn hand_in(ctx: &mut ScriptCtx<'_>, player: ObjectId, item: i32, points: i64, yes: &str, no: &str) -> Option<String> {
    if ctx.has_quest_items(player, item) {
        engine::update_trust(ctx, (ctx.get_quest_items_count(player, item) * points) as i32, true);
        ctx.take_items(player, item, -1);
        Some(yes.to_owned())
    } else {
        Some(no.to_owned())
    }
}

impl Script for Kief {
    fn name(&self) -> &'static str {
        "Kief"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Kief"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[KIEF]);
        s.add_start_npc(&[KIEF]);
        s.add_talk_id(&[KIEF]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = None;
        match event {
            "32354-11g.htm" => {
                htmltext = Some(event.to_owned());
            }
            "Badges" => match engine::level(ctx) {
                2 | 3 => {
                    if ctx.has_quest_items(player, DARION_BADGE) {
                        // `(int) getQuestItemsCount(...) * 10`: the cast binds first.
                        let points = (ctx.get_quest_items_count(player, DARION_BADGE) as i32).wrapping_mul(10);
                        engine::update_trust(ctx, points, true);
                        ctx.take_items(player, DARION_BADGE, -1);
                        return Some("32354-10.htm".to_owned());
                    }
                }
                _ => {
                    htmltext = Some("32354-10a.htm".to_owned());
                }
            },
            "Bottle" => {
                if engine::level(ctx) >= 7 {
                    if ctx.get_quest_items_count(player, STINGER) >= 20 {
                        ctx.take_items(player, STINGER, 20);
                        ctx.give_items(player, BOTTLE, 1);
                        htmltext = Some("32354-11h.htm".to_owned());
                    } else {
                        htmltext = Some("32354-11i.htm".to_owned());
                    }
                }
            }
            "dlf" => {
                if engine::level(ctx) == 7 {
                    htmltext = hand_in(ctx, player, DIM_LIFE_FORCE, 20, "32354-11a.htm", "32354-11b.htm");
                }
            }
            "lf" => {
                if engine::level(ctx) == 7 {
                    htmltext = hand_in(ctx, player, LIFE_FORCE, 80, "32354-11c.htm", "32354-11d.htm");
                }
            }
            "clf" => {
                if engine::level(ctx) == 7 {
                    htmltext = hand_in(ctx, player, CONTAINED_LIFE_FORCE, 200, "32354-11e.htm", "32354-11f.htm");
                }
            }
            _ => {}
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(
            match engine::level(ctx) {
                1 => "32354-01.htm",
                2 | 3 => "32354-01a.htm",
                4 => "32354-01e.htm",
                5 => "32354-01d.htm",
                6 => "32354-01b.htm",
                7 => "32354-01c.htm",
                _ => "32354-01f.htm",
            }
            .to_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::engine::HellboundEngine;
    use crate::ai::areas::hellbound::testing::{last_page, set_level};
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32354" type="Folk" level="70" name="Kief"/>"#;
    const ITEMS: &str = r#"
        <item id="9672" type="EtcItem" name="Magic Bottle"/>
        <item id="9674" type="EtcItem" name="Darion's Badge"><set name="is_stackable" val="true"/></item>
        <item id="9681" type="EtcItem" name="Life Force"><set name="is_stackable" val="true"/></item>
        <item id="10012" type="EtcItem" name="Stinger"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(HellboundEngine);
        registry.add(Kief);
        let pages = Pages::of(&Kief)
            .page("32354-01a.htm", &["Badges"])
            .page("32354-01c.htm", &["Bottle", "dlf", "lf", "clf"])
            .plain(&[
                "32354-01.htm",
                "32354-10.htm",
                "32354-10a.htm",
                "32354-11c.htm",
                "32354-11d.htm",
                "32354-11f.htm",
                "32354-11h.htm",
                "32354-11i.htm",
            ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npc = t.spawn(KIEF, 20);
        t.enter(
            1,
            80,
            vec![
                item(100, DARION_BADGE, 4),
                item(101, LIFE_FORCE, 3),
                item(102, STINGER, 20),
            ],
        );
        (t, npc)
    }

    fn trust(t: &ScriptTest) -> i32 {
        t.world.global_variables().get_int("HBTrust", 0)
    }

    #[test]
    fn badges_become_trust_at_levels_two_and_three() {
        let (mut t, npc) = setup();
        set_level(&mut t, "Kief", 2);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32354-01a.htm"));
        assert!(t.event(1, "Kief", "Badges").contains("32354-10.htm"));
        assert_eq!(t.count(1, DARION_BADGE), 0);
        assert_eq!(trust(&t), 40);
    }

    #[test]
    fn the_wrong_level_for_badges_gets_the_other_page() {
        let (mut t, npc) = setup();
        set_level(&mut t, "Kief", 2);
        t.talk(1, npc);
        t.take();
        set_level(&mut t, "Kief", 5);
        assert!(t.event(1, "Kief", "Badges").contains("32354-10a.htm"));
        assert_eq!(t.count(1, DARION_BADGE), 4);
    }

    #[test]
    fn life_force_and_the_bottle_work_at_level_seven() {
        let (mut t, npc) = setup();
        set_level(&mut t, "Kief", 7);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32354-01c.htm"));
        assert!(t.event(1, "Kief", "lf").contains("32354-11c.htm"));
        assert_eq!(trust(&t), 240);
        assert_eq!(t.count(1, LIFE_FORCE), 0);
        t.talk(1, npc);
        t.take();
        assert!(t.event(1, "Kief", "clf").contains("32354-11f.htm"));
        t.talk(1, npc);
        t.take();
        assert!(t.event(1, "Kief", "Bottle").contains("32354-11h.htm"));
        assert_eq!(t.count(1, BOTTLE), 1);
        t.talk(1, npc);
        t.take();
        assert!(t.event(1, "Kief", "Bottle").contains("32354-11i.htm"));
    }
}
