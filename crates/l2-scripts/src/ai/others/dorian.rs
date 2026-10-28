//! Dorian, the Forest of the Dead's keeper of the Silver Cross: reacts to a player in sight.
// Java: dist/game/data/scripts/ai/others/Dorian/Dorian.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const DORIAN: i32 = 25332;

// Items
const SILVER_CROSS: i32 = 7153;
const BROKEN_SILVER_CROSS: i32 = 7154;
/// Java `Q00024_InhabitantsOfTheForestOfTheDead.class.getSimpleName()`.
const Q00024_INHABITANTS_OF_THE_FOREST_OF_THE_DEAD: &str = "Q00024_InhabitantsOfTheForestOfTheDead";

/// Java `ai.others.Dorian.Dorian`.
pub struct Dorian;

impl Script for Dorian {
    fn name(&self) -> &'static str {
        "Dorian"
    }

    fn path(&self) -> &'static str {
        "ai/others/Dorian"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_creature_see_id(&[DORIAN]);
    }

    // The engine does not fire `on_creature_see` yet (`CreatureSeeTaskManager`, see
    // `docs/design/scripts.md`), so this hook waits for it.
    fn on_creature_see(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        if ctx.world().player(creature).is_some() {
            let qs = ctx.quest_state_of(Q00024_INHABITANTS_OF_THE_FOREST_OF_THE_DEAD, creature, false);
            if let Some(qs) = qs {
                if qs.is_cond(ctx, 3) {
                    ctx.take_items(creature, SILVER_CROSS, -1);
                    ctx.give_items(creature, BROKEN_SILVER_CROSS, 1);
                    qs.set_cond(ctx, 4, true);
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::ThatSign, &[]);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::{QuestLoad, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{ScriptTest, item, names_to, player_data};

    const NPCS: &str = r#"<npc id="25332" type="Monster" level="60" name="Dorian"/>"#;
    const ITEMS: &str = r#"<item id="7153" type="EtcItem" name="Silver Cross"/><item id="7154" type="EtcItem" name="Broken Silver Cross"/>"#;

    /// A stand-in for quest 24 (it only has to exist by name).
    struct Q24;

    impl Script for Q24 {
        fn name(&self) -> &'static str {
            Q00024_INHABITANTS_OF_THE_FOREST_OF_THE_DEAD
        }
        fn path(&self) -> &'static str {
            "quests/Q00024_InhabitantsOfTheForestOfTheDead"
        }
        fn id(&self) -> i32 {
            24
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    #[test]
    fn the_cross_breaks_for_a_player_at_cond_three() {
        let mut registry = ScriptRegistry::new();
        registry.add(Dorian);
        registry.add(Q24);
        let mut t = ScriptTest::new(registry, NPCS, ITEMS, &[]);
        let dorian = t.spawn(DORIAN, 20);
        let mut data = player_data(1, 60, vec![item(900, SILVER_CROSS, 1)]);
        data.profile.quests = QuestLoad {
            states: vec![(
                Q00024_INHABITANTS_OF_THE_FOREST_OF_THE_DEAD.to_owned(),
                Some("Started".to_owned()),
            )],
            variables: vec![(
                Q00024_INHABITANTS_OF_THE_FOREST_OF_THE_DEAD.to_owned(),
                "cond".to_owned(),
                Some("3".to_owned()),
            )],
        };
        t.enter_with(data);
        t.take();
        t.take_writes();

        t.world.script_on_creature_see(dorian, ObjectId(1));
        t.world.script_drain();
        assert_eq!(t.count(1, BROKEN_SILVER_CROSS), 1);
        assert_eq!(t.count(1, SILVER_CROSS), 0);
        assert!(names_to(&t.take(), 1).contains(&"NPC_SAY".to_owned()));
        assert!(!t.take_writes().is_empty());
    }
}
