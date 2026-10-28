//! Deltuva, who sends the players who finished Matras' Curiosity to the Tower of Naia.
// Java: dist/game/data/scripts/ai/areas/Hellbound/AI/NPC/Deltuva/Deltuva.java

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const DELTUVA: i32 = 32313;

// Location
const TELEPORT: (i32, i32, i32) = (17934, 283189, -9701);

/// Java `Q00132_MatrasCuriosity.class.getSimpleName()`.
const Q00132_MATRAS_CURIOSITY: &str = "Q00132_MatrasCuriosity";

/// Java `ai.areas.Hellbound.AI.NPC.Deltuva.Deltuva`.
pub struct Deltuva;

impl Script for Deltuva {
    fn name(&self) -> &'static str {
        "Deltuva"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Hellbound/AI/NPC/Deltuva"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DELTUVA]);
        s.add_talk_id(&[DELTUVA]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event.eq_ignore_ascii_case("teleport") {
            let player = player?;
            let completed = ctx
                .player_quest_state(player, Q00132_MATRAS_CURIOSITY)
                .is_some_and(|qs| qs.is_completed());
            if !completed {
                return Some("32313-02.htm".to_owned());
            }

            ctx.teleport_player(player, TELEPORT.0, TELEPORT.1, TELEPORT.2, 0, false);
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{QuestLoad, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, player_data};

    const NPCS: &str = r#"<npc id="32313" type="Folk" level="70" name="Deltuva"/>"#;

    /// Java `Q00132_MatrasCuriosity` as far as Deltuva needs it: a quest of that name.
    struct MatrasCuriosity;

    impl Script for MatrasCuriosity {
        fn name(&self) -> &'static str {
            Q00132_MATRAS_CURIOSITY
        }
        fn path(&self) -> &'static str {
            "quests/Q00132_MatrasCuriosity"
        }
        fn id(&self) -> i32 {
            132
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    #[test]
    fn only_the_completed_curiosity_may_teleport() {
        let mut registry = ScriptRegistry::new();
        registry.add(Deltuva);
        registry.add(MatrasCuriosity);
        let pages = Pages::of(&Deltuva)
            .page("32313-01.htm", &["teleport"])
            .plain(&["32313-02.htm"]);
        let mut t = ScriptTest::with_pages(registry, NPCS, "", pages);
        t.enter(1, 80, vec![]);
        // The quest list entry is not completed: the refusal page.
        t.world.run_as_script("Deltuva", |ctx| {
            let r = Deltuva.on_event(ctx, "teleport", None, Some(ObjectId(1)));
            assert_eq!(r.as_deref(), Some("32313-02.htm"));
        });

        let mut data = player_data(2, 80, vec![]);
        data.profile.quests = QuestLoad {
            states: vec![(Q00132_MATRAS_CURIOSITY.to_owned(), Some("Completed".to_owned()))],
            variables: vec![],
        };
        t.enter_with(data);
        t.world.run_as_script("Deltuva", |ctx| {
            let r = Deltuva.on_event(ctx, "teleport", None, Some(ObjectId(2)));
            assert_eq!(r, None);
        });
        t.ticks(100);
        let (x, y, _) = t.world.position_of(ObjectId(2)).unwrap();
        assert_eq!((x, y), (TELEPORT.0, TELEPORT.1));
    }
}
