//! Quest 635: Into the Dimensional Rift (a dummy quest, shown in the journal inside the rift).
// Java: dist/game/data/scripts/quests/Q00635_IntoTheDimensionalRift/Q00635_IntoTheDimensionalRift.java
//
// Java's constructor is `super(635)` and nothing else: no NPC, no item, no hook. The quest only
// exists so that its name and id are known; the rift code is what puts it in the journal.

use l2_world::{Script, ScriptSetup};

/// Java `Q00635_IntoTheDimensionalRift`.
pub struct Q00635IntoTheDimensionalRift;

impl Script for Q00635IntoTheDimensionalRift {
    fn name(&self) -> &'static str {
        "Q00635_IntoTheDimensionalRift"
    }

    fn path(&self) -> &'static str {
        "quests/Q00635_IntoTheDimensionalRift"
    }

    fn id(&self) -> i32 {
        635
    }

    fn setup(&self, _setup: &mut ScriptSetup) {}
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    #[test]
    fn the_dummy_quest_writes_nothing_and_offers_no_page() {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00635IntoTheDimensionalRift);
        let mut t = ScriptTest::new(registry, "", "", &[]);
        t.enter(1, 40, vec![]);
        assert!(t.take_writes().is_empty());
        assert!(t.world.quest_list(l2_util::ObjectId(1)).quests.is_empty());
    }
}
