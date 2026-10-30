//! Jinia hands out the Frozen Core (or the Black one) of the Freya instance.
// Java: dist/game/data/scripts/ai/areas/FrozenLabyrinth/Jinia/Jinia.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const JINIA: i32 = 32781;
// Items
const FROZEN_CORE: i32 = 15469;
const BLACK_FROZEN_CORE: i32 = 15470;
// Misc
const MIN_LEVEL: i32 = 82;
/// Java `Q10286_ReunionWithSirra.class.getSimpleName()`.
const REUNION_WITH_SIRRA: &str = "Q10286_ReunionWithSirra";

/// Java `ai.areas.FrozenLabyrinth.Jinia.Jinia`.
pub struct Jinia;

impl Script for Jinia {
    fn name(&self) -> &'static str {
        "Jinia"
    }

    fn path(&self) -> &'static str {
        "ai/areas/FrozenLabyrinth/Jinia"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[JINIA]);
        s.add_first_talk_id(&[JINIA]);
        s.add_talk_id(&[JINIA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        // NOTE(java-quirk): every other event is echoed as the page to show.
        let mut htmltext = event.to_owned();
        if event == "check" {
            let player = player?;
            if ctx.has_at_least_one_quest_item(player, &[FROZEN_CORE, BLACK_FROZEN_CORE]) {
                htmltext = "32781-03.html".to_owned();
            } else {
                let completed = ctx
                    .player_quest_state(player, REUNION_WITH_SIRRA)
                    .is_some_and(|qs| qs.is_completed());
                if completed {
                    ctx.give_items(player, FROZEN_CORE, 1);
                } else {
                    ctx.give_items(player, BLACK_FROZEN_CORE, 1);
                }
                htmltext = "32781-04.html".to_owned();
            }
        }
        Some(htmltext)
    }

    fn on_first_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let level = ctx.player_level(player);
        if let Some(qs) = ctx.player_quest_state(player, REUNION_WITH_SIRRA)
            && level >= MIN_LEVEL
        {
            if qs.is_cond(5) || qs.is_cond(6) {
                return Some("32781-09.html".to_owned());
            } else if qs.is_cond(7) {
                return Some("32781-01.html".to_owned());
            }
        }
        Some("32781-02.html".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, pages_to};

    const NPCS: &str = r#"<npc id="32781" type="Folk" level="70" name="Jinia"/>"#;
    const ITEMS: &str = r#"<item id="15469" type="EtcItem" name="Frozen Core"/>
        <item id="15470" type="EtcItem" name="Black Frozen Core"/>"#;

    /// Java `Q10286_ReunionWithSirra`: only its name and state matter here.
    struct Q10286;

    impl Script for Q10286 {
        fn name(&self) -> &'static str {
            REUNION_WITH_SIRRA
        }
        fn path(&self) -> &'static str {
            "quests/Q10286_ReunionWithSirra"
        }
        fn id(&self) -> i32 {
            10286
        }
        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    #[test]
    fn jinia_gives_the_black_core_until_the_quest_is_done() {
        let mut registry = ScriptRegistry::new();
        registry.add(Jinia);
        registry.add(Q10286);
        let pages = Pages::of(&Jinia)
            .page("32781-02.html", &["check"])
            .page("32781-01.html", &["check"])
            .page("32781-09.html", &[])
            .page("32781-04.html", &["check"])
            .plain(&["32781-03.html"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let jinia = t.spawn(JINIA, 20);
        t.enter(1, 85, vec![]);
        t.talk(1, jinia);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32781-02.html"));
        assert!(t.event(1, "Jinia", "check").contains("32781-04.html"));
        assert_eq!(t.count(1, BLACK_FROZEN_CORE), 1);
        assert_eq!(t.count(1, FROZEN_CORE), 0);
        // Already has one: the other page.
        assert!(t.event(1, "Jinia", "check").contains("32781-03.html"));
        assert_eq!(t.count(1, BLACK_FROZEN_CORE), 1);

        // A finished "Reunion with Sirra" gets the real core.
        t.enter(2, 85, vec![]);
        t.world.run_as_script("Jinia", |ctx| {
            let qs = ctx.quest_state_of(REUNION_WITH_SIRRA, ObjectId(2), true).unwrap();
            qs.start_quest(ctx);
            qs.set_cond(ctx, 7, false);
        });
        t.talk(2, jinia);
        assert!(pages_to(&t.take(), 2).pop().unwrap().contains("32781-01.html"));
        t.world.run_as_script("Jinia", |ctx| {
            let qs = ctx
                .quest_state_of(REUNION_WITH_SIRRA, ObjectId(2), false)
                .unwrap();
            qs.exit_quest(ctx, false, false);
        });
        assert!(t.event(2, "Jinia", "check").contains("32781-04.html"));
        assert_eq!(t.count(2, FROZEN_CORE), 1);
    }
}
