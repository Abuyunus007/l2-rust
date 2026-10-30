//! Tunatun, the beast herder of the Beast Farm: hands out the Beast Handler's Whip.
// Java: dist/game/data/scripts/ai/areas/BeastFarm/Tunatun/Tunatun.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const TUNATUN: i32 = 31537;

// Item
const BEAST_HANDLERS_WHIP: i32 = 15473;

// Misc
const MIN_LEVEL: i32 = 82;
/// Java `Q00020_BringUpWithLove.class.getSimpleName()`.
const Q00020_BRING_UP_WITH_LOVE: &str = "Q00020_BringUpWithLove";

/// Java `ai.areas.BeastFarm.Tunatun.Tunatun`.
pub struct Tunatun;

impl Script for Tunatun {
    fn name(&self) -> &'static str {
        "Tunatun"
    }

    fn path(&self) -> &'static str {
        "ai/areas/BeastFarm/Tunatun"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TUNATUN]);
        s.add_first_talk_id(&[TUNATUN]);
        s.add_talk_id(&[TUNATUN]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        if event == "Whip" {
            if ctx.has_quest_items(player, BEAST_HANDLERS_WHIP) {
                return Some("31537-01.html".to_owned());
            }

            let qs_exists = ctx.player_quest_state(player, Q00020_BRING_UP_WITH_LOVE).is_some();
            if !qs_exists && ctx.player_level(player) < MIN_LEVEL {
                return Some("31537-02.html".to_owned());
            } else if qs_exists || ctx.player_level(player) >= MIN_LEVEL {
                ctx.give_items(player, BEAST_HANDLERS_WHIP, 1);
                return Some("31537-03.html".to_owned());
            }
        }

        Some(event.to_owned())
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some(format!("{}.html", npc.id))
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{Pages, ScriptTest};

    const NPCS: &str = r#"<npc id="31537" type="Folk" level="70" name="Tunatun"/>"#;
    const ITEMS: &str = r#"<item id="15473" type="EtcItem" name="Beast Handler's Whip"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Tunatun);
        let pages = Pages::of(&Tunatun)
            .page("31537.html", &["Whip"])
            .page("31537-03.html", &["Whip"])
            .plain(&["31537-01.html", "31537-02.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let tunatun = t.spawn(TUNATUN, 20);
        (t, tunatun)
    }

    #[test]
    fn the_whip_is_given_once_and_only_from_level_82_without_the_quest() {
        let (mut t, tunatun) = setup();
        t.enter(1, 81, vec![]);
        t.talk(1, tunatun);
        t.take();
        // Level 81 and no Q00020: "31537-02.html", no whip.
        assert!(t.event(1, "Tunatun", "Whip").contains("31537-02"));
        assert_eq!(t.count(1, BEAST_HANDLERS_WHIP), 0);

        t.enter(2, 82, vec![]);
        t.talk(2, tunatun);
        t.take();
        assert!(t.event(2, "Tunatun", "Whip").contains("31537-03"));
        assert_eq!(t.count(2, BEAST_HANDLERS_WHIP), 1);
        assert!(t.event(2, "Tunatun", "Whip").contains("31537-01"));
        assert_eq!(t.count(2, BEAST_HANDLERS_WHIP), 1);
    }
}
