//! Rignos, the host of the Grand Isle of Prayer Race.
// Java: dist/game/data/scripts/ai/areas/IsleOfPrayer/Rignos/Rignos.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const RIGNOS: i32 = 32349; // Rignos
// Item
const STAMP: i32 = 10013; // Race Stamp
const KEY: i32 = 9694; // Secret Key
// Skill
const TIMER: SkillKey = SkillKey { id: 5239, level: 5 }; // Event Timer
// Misc
const MIN_LV: i32 = 78;

/// Java `ai.areas.IsleOfPrayer.Rignos.Rignos`.
pub struct Rignos;

impl Script for Rignos {
    fn name(&self) -> &'static str {
        "Rignos"
    }

    fn path(&self) -> &'static str {
        "ai/areas/IsleOfPrayer/Rignos"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[RIGNOS]);
        s.add_talk_id(&[RIGNOS]);
        s.add_first_talk_id(&[RIGNOS]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "32349-03.html" => {
                return Some(event.to_owned());
            }
            "startRace" => {
                let (npc, player) = (npc?, player?);
                if ctx.is_script_value(npc, 0) {
                    ctx.set_script_value(npc, 1);
                    ctx.start_quest_timer("TIME_OUT", 1800000, Some(npc), None, false);
                    ctx.world_mut().apply_effects(player, player, TIMER);
                    if let Some(summon) = ctx.summon_of(player) {
                        ctx.world_mut().apply_effects(summon, summon, TIMER);
                    }
                    if ctx.has_quest_items(player, STAMP) {
                        ctx.take_items(player, STAMP, -1);
                    }
                }
            }
            "exchange" => {
                let player = player?;
                if ctx.get_quest_items_count(player, STAMP) >= 4 {
                    ctx.give_items(player, KEY, 3);
                    ctx.take_items(player, STAMP, -1);
                }
            }
            "TIME_OUT" => {
                ctx.set_script_value(npc?, 0);
            }
            _ => {}
        }
        None
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let mut htmltext = if ctx.is_script_value(npc, 0) && ctx.player_level(player) >= MIN_LV {
            "32349.html"
        } else {
            "32349-02.html"
        };
        if ctx.get_quest_items_count(player, STAMP) >= 4 {
            htmltext = "32349-01.html";
        }
        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, pages_to};

    const NPCS: &str = r#"<npc id="32349" type="Folk" level="70" name="Rignos"/>"#;
    const ITEMS: &str = r#"<item id="10013" type="EtcItem" name="Race Stamp"><set name="is_stackable" val="true"/></item>
        <item id="9694" type="EtcItem" name="Secret Key"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Rignos);
        let pages = Pages::of(&Rignos)
            .page("32349.html", &["32349-03.html", "startRace"])
            .page("32349-01.html", &["exchange"])
            .plain(&["32349-02.html"])
            .plain(&["32349-03.html"]);
        let t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let mut t = t;
        let rignos = t.spawn(RIGNOS, 20);
        (t, rignos)
    }

    #[test]
    fn the_race_starts_once_and_ends_after_thirty_minutes() {
        let (mut t, rignos) = setup();
        t.enter(1, 78, vec![item(500, STAMP, 2)]);
        t.enter(2, 70, vec![]);
        t.talk(1, rignos);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32349.html"));
        t.talk(2, rignos);
        assert!(pages_to(&t.take(), 2).pop().unwrap().contains("32349-02.html"));

        t.bypass(1, "Script Rignos startRace");
        assert!(t.world.is_npc_script_value(rignos, 1));
        // The stamps of an earlier run are taken.
        assert_eq!(t.count(1, STAMP), 0);
        // Somebody else sees the race running.
        t.talk(1, rignos);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32349-02.html"));
        t.wait_ms(1_800_100);
        assert!(t.world.is_npc_script_value(rignos, 0));
    }

    #[test]
    fn four_stamps_buy_three_keys() {
        let (mut t, rignos) = setup();
        t.enter(1, 78, vec![item(500, STAMP, 4)]);
        t.talk(1, rignos);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32349-01.html"));
        t.bypass(1, "Script Rignos exchange");
        assert_eq!(t.count(1, KEY), 3);
        assert_eq!(t.count(1, STAMP), 0);
    }
}
