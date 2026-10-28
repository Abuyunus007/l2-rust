//! Buron, the trader of Hellbound's native village who sells armor for Darion's Badges.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Buron/Buron.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

use crate::ai::areas::hellbound::engine;

const BURON: i32 = 32345;
const HELMET: i32 = 9669;
const TUNIC: i32 = 9670;
const PANTS: i32 = 9671;
const DARION_BADGE: i32 = 9674;

/// Java `ai.areas.Hellbound.AI.NPC.Buron.Buron`.
pub struct Buron;

impl Script for Buron {
    fn name(&self) -> &'static str {
        "Buron"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Buron"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[BURON]);
        s.add_start_npc(&[BURON]);
        s.add_talk_id(&[BURON]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let htmltext;
        if event.eq_ignore_ascii_case("Rumor") {
            htmltext = Some(format!("32345-{}r.htm", engine::level(ctx)));
        } else if engine::level(ctx) < 2 {
            htmltext = Some("32345-lowlvl.htm".to_owned());
        } else if ctx.get_quest_items_count(player, DARION_BADGE) >= 10 {
            ctx.take_items(player, DARION_BADGE, 10);
            // `player.addItem(QUEST, item, 1, npc, true)`.
            if event.eq_ignore_ascii_case("Tunic") {
                ctx.give_items(player, TUNIC, 1);
            } else if event.eq_ignore_ascii_case("Helmet") {
                ctx.give_items(player, HELMET, 1);
            } else if event.eq_ignore_ascii_case("Pants") {
                ctx.give_items(player, PANTS, 1);
            }

            htmltext = None;
        } else {
            htmltext = Some("32345-noitems.htm".to_owned());
        }

        htmltext
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        ctx.get_quest_state(player, true);
        Some(
            match engine::level(ctx) {
                1 => "32345-01.htm",
                2..=4 => "32345-02.htm",
                _ => "32345-01a.htm",
            }
            .to_owned(),
        )
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::ai::areas::hellbound::testing::{last_page, set_level};
    use crate::testing::{Pages, ScriptTest, item};

    const NPCS: &str = r#"<npc id="32345" type="Folk" level="70" name="Buron"/>"#;
    const ITEMS: &str = r#"
        <item id="9670" type="EtcItem" name="Tunic"/>
        <item id="9674" type="EtcItem" name="Darion's Badge"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Buron);
        let pages = Pages::of(&Buron)
            .page("32345-01a.htm", &["Tunic", "Rumor"])
            .page("32345-02.htm", &["Tunic", "Rumor"])
            .plain(&["32345-lowlvl.htm", "32345-noitems.htm", "32345-3r.htm"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let npc = t.spawn(BURON, 20);
        t.enter(1, 80, vec![item(100, DARION_BADGE, 25)]);
        (t, npc)
    }

    #[test]
    fn the_tunic_costs_ten_badges_from_level_two() {
        let (mut t, npc) = setup();
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32345-01a.htm"));
        assert!(t.event(1, "Buron", "Tunic").contains("32345-lowlvl.htm"));
        set_level(&mut t, "Buron", 3);
        t.talk(1, npc);
        assert!(last_page(&t.take(), 1).contains("32345-02.htm"));
        t.bypass(1, "Script Buron Tunic");
        assert_eq!(t.count(1, DARION_BADGE), 15);
        assert_eq!(t.count(1, 9670), 1);
        assert!(t.event(1, "Buron", "Rumor").contains("32345-3r.htm"));
    }

    #[test]
    fn too_few_badges_get_the_noitems_page() {
        let (mut t, npc) = setup();
        set_level(&mut t, "Buron", 3);
        t.talk(1, npc);
        t.take();
        t.bypass(1, "Script Buron Tunic");
        t.talk(1, npc);
        t.take();
        t.bypass(1, "Script Buron Tunic");
        assert_eq!(t.count(1, DARION_BADGE), 5);
        t.talk(1, npc);
        t.take();
        assert!(t.event(1, "Buron", "Tunic").contains("32345-noitems.htm"));
    }
}
