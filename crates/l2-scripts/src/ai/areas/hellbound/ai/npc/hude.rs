//! Hude, the caravan trader who upgrades the certificates.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Hude/Hude.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

// NPCs
const HUDE: i32 = 32298;

// Items
const BASIC_CERT: i32 = 9850;
const STANDART_CERT: i32 = 9851;
const PREMIUM_CERT: i32 = 9852;
const MARK_OF_BETRAYAL: i32 = 9676;
const LIFE_FORCE: i32 = 9681;
const CONTAINED_LIFE_FORCE: i32 = 9682;
const MAP: i32 = 9994;
const STINGER: i32 = 10012;

/// Java `ai.areas.Hellbound.AI.NPC.Hude.Hude`.
pub struct Hude;

impl Script for Hude {
    fn name(&self) -> &'static str {
        "Hude"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Hude"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[HUDE]);
        s.add_start_npc(&[HUDE]);
        s.add_talk_id(&[HUDE]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        match event {
            "scertif" => {
                if engine::level(ctx) > 3
                    && ctx.has_quest_items(player, BASIC_CERT)
                    && ctx.get_quest_items_count(player, MARK_OF_BETRAYAL) >= 30
                    && ctx.get_quest_items_count(player, STINGER) >= 60
                {
                    ctx.take_items(player, MARK_OF_BETRAYAL, 30);
                    ctx.take_items(player, STINGER, 60);
                    ctx.take_items(player, BASIC_CERT, 1);
                    ctx.give_items(player, STANDART_CERT, 1);
                    return Some("32298-04a.htm".to_owned());
                }

                return Some("32298-04b.htm".to_owned());
            }
            "pcertif" => {
                if engine::level(ctx) > 6
                    && ctx.has_quest_items(player, STANDART_CERT)
                    && ctx.get_quest_items_count(player, LIFE_FORCE) >= 56
                    && ctx.get_quest_items_count(player, CONTAINED_LIFE_FORCE) >= 14
                {
                    ctx.take_items(player, LIFE_FORCE, 56);
                    ctx.take_items(player, CONTAINED_LIFE_FORCE, 14);
                    ctx.take_items(player, STANDART_CERT, 1);
                    ctx.give_items(player, PREMIUM_CERT, 1);
                    ctx.give_items(player, MAP, 1);
                    return Some("32298-06a.htm".to_owned());
                }

                return Some("32298-06b.htm".to_owned());
            }
            "multisell1" => {
                if ctx.has_quest_items(player, STANDART_CERT) || ctx.has_quest_items(player, PREMIUM_CERT) {
                    ctx.world_mut().multisell_separate_and_send(
                        322_980_001,
                        player,
                        npc.map(|n| n.object_id),
                        false,
                        1.0,
                        1.0,
                    );
                }
            }
            "multisell2" => {
                if ctx.has_quest_items(player, PREMIUM_CERT) {
                    ctx.world_mut().multisell_separate_and_send(
                        322_980_002,
                        player,
                        npc.map(|n| n.object_id),
                        false,
                        1.0,
                        1.0,
                    );
                }
            }
            _ => {}
        }

        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = None;
        if !ctx.has_at_least_one_quest_item(player, &[BASIC_CERT, STANDART_CERT, PREMIUM_CERT]) {
            htmltext = Some("32298-01.htm");
        } else if ctx.has_quest_items(player, BASIC_CERT)
            && !ctx.has_at_least_one_quest_item(player, &[STANDART_CERT, PREMIUM_CERT])
        {
            htmltext = Some("32298-03.htm");
        } else if ctx.has_quest_items(player, STANDART_CERT) && !ctx.has_quest_items(player, PREMIUM_CERT) {
            htmltext = Some("32298-05.htm");
        } else if ctx.has_quest_items(player, PREMIUM_CERT) {
            htmltext = Some("32298-07.htm");
        }

        htmltext.map(str::to_owned)
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::{last_page, set_level};
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32298" type="Folk" level="70" name="Hude"/>"#;
    const ITEMS: &str = r#"
        <item id="9850" type="EtcItem" name="Basic Certificate"/>
        <item id="9851" type="EtcItem" name="Standard Certificate"/>
        <item id="9852" type="EtcItem" name="Premium Certificate"/>
        <item id="9994" type="EtcItem" name="Map"/>
        <item id="9676" type="EtcItem" name="Mark of Betrayal"><set name="is_stackable" val="true"/></item>
        <item id="10012" type="EtcItem" name="Stinger"><set name="is_stackable" val="true"/></item>
        <item id="9681" type="EtcItem" name="Life Force"><set name="is_stackable" val="true"/></item>
        <item id="9682" type="EtcItem" name="Contained Life Force"><set name="is_stackable" val="true"/></item>"#;

    fn setup(items: Vec<l2_world::ItemData>) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Hude);
        let pages = Pages::of(&Hude)
            .page("32298-03.htm", &["scertif"])
            .page("32298-05.htm", &["pcertif", "multisell1"])
            .plain(&["32298-01.htm", "32298-04a.htm", "32298-04b.htm", "32298-06a.htm", "32298-06b.htm", "32298-07.htm"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npc = t.spawn(HUDE, 20);
        t.enter(1, 80, items);
        (t, npc)
    }

    #[test]
    fn the_standard_certificate_needs_level_four_and_the_goods() {
        let items = vec![
            item(100, BASIC_CERT, 1),
            item(101, MARK_OF_BETRAYAL, 30),
            item(102, STINGER, 60),
        ];
        let (mut t, npc) = setup(items);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32298-03.htm"));
        // Level 0 is too low.
        assert!(t.event(1, "Hude", "scertif").contains("32298-04b.htm"));
        assert_eq!(t.count(1, BASIC_CERT), 1);
        set_level(&mut t, "Hude", 4);
        t.talk(1, npc);
        t.take();
        assert!(t.event(1, "Hude", "scertif").contains("32298-04a.htm"));
        assert_eq!(t.count(1, BASIC_CERT), 0);
        assert_eq!(t.count(1, STANDART_CERT), 1);
        assert_eq!(t.count(1, STINGER), 0);
    }

    #[test]
    fn the_premium_certificate_comes_with_the_map() {
        let items = vec![
            item(100, STANDART_CERT, 1),
            item(101, LIFE_FORCE, 56),
            item(102, CONTAINED_LIFE_FORCE, 14),
        ];
        let (mut t, npc) = setup(items);
        set_level(&mut t, "Hude", 7);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32298-05.htm"));
        assert!(t.event(1, "Hude", "pcertif").contains("32298-06a.htm"));
        assert_eq!(t.count(1, PREMIUM_CERT), 1);
        assert_eq!(t.count(1, MAP), 1);
        assert_eq!(t.count(1, STANDART_CERT), 0);
    }
}
