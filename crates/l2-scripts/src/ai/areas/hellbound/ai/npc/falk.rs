//! Falk, the caravan trader who hands out the basic certificate for Darion's Badges.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Falk/Falk.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const FALK: i32 = 32297;

// Items
const DARION_BADGE: i32 = 9674;
const BASIC_CERT: i32 = 9850; // Basic Caravan Certificate
const STANDART_CERT: i32 = 9851; // Standard Caravan Certificate
const PREMIUM_CERT: i32 = 9852; // Premium Caravan Certificate

/// Java `ai.areas.Hellbound.AI.NPC.Falk.Falk`.
pub struct Falk;

impl Script for Falk {
    fn name(&self) -> &'static str {
        "Falk"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Falk"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[FALK]);
        s.add_start_npc(&[FALK]);
        s.add_talk_id(&[FALK]);
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if ctx.has_at_least_one_quest_item(player, &[BASIC_CERT, STANDART_CERT, PREMIUM_CERT]) {
            return Some("32297-01a.htm".to_owned());
        }

        Some("32297-01.htm".to_owned())
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        if ctx.has_at_least_one_quest_item(player, &[BASIC_CERT, STANDART_CERT, PREMIUM_CERT]) {
            return Some("32297-01a.htm".to_owned());
        }

        Some("32297-02.htm".to_owned())
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        if event.eq_ignore_ascii_case("badges")
            && !ctx.has_at_least_one_quest_item(player, &[BASIC_CERT, STANDART_CERT, PREMIUM_CERT])
        {
            if ctx.get_quest_items_count(player, DARION_BADGE) >= 20 {
                ctx.take_items(player, DARION_BADGE, 20);
                ctx.give_items(player, BASIC_CERT, 1);
                return Some("32297-02a.htm".to_owned());
            }

            return Some("32297-02b.htm".to_owned());
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::last_page;
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32297" type="Folk" level="70" name="Falk"/>"#;
    const ITEMS: &str = r#"
        <item id="9850" type="EtcItem" name="Basic Certificate"/>
        <item id="9674" type="EtcItem" name="Darion's Badge"><set name="is_stackable" val="true"/></item>"#;

    fn setup(items: Vec<l2_world::ItemData>) -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Falk);
        let pages = Pages::of(&Falk)
            .page("32297-01.htm", &["badges"])
            .page("32297-02.htm", &["badges"])
            .plain(&["32297-01a.htm", "32297-02a.htm", "32297-02b.htm"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npc = t.spawn(FALK, 20);
        t.enter(1, 80, items);
        (t, npc)
    }

    #[test]
    fn twenty_badges_buy_the_basic_certificate() {
        let (mut t, npc) = setup(vec![item(100, DARION_BADGE, 20)]);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32297-01.htm"));
        assert!(t.event(1, "Falk", "badges").contains("32297-02a.htm"));
        assert_eq!(t.count(1, DARION_BADGE), 0);
        assert_eq!(t.count(1, BASIC_CERT), 1);
        // With a certificate the first page is the other one.
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32297-01a.htm"));
    }

    #[test]
    fn too_few_badges_are_refused() {
        let (mut t, npc) = setup(vec![item(100, DARION_BADGE, 19)]);
        t.talk(1, npc);
        t.take();
        assert!(t.event(1, "Falk", "badges").contains("32297-02b.htm"));
        assert_eq!(t.count(1, DARION_BADGE), 19);
    }
}
