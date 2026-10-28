//! `data/scripts/ai/areas/Hellbound`: the Hellbound engine (level and trust), its NPCs, monsters,
//! zones and instances. Java's `HellboundLoader` loads the engine first, then the scripts.

pub mod ai;
pub mod data;
pub mod engine;

use l2_world::ScriptRegistry;

/// Registers every script of the folder, the engine first (Java's `HellboundLoader` order).
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(engine::HellboundEngine);
    ai::register(r);
}

/// Helpers of the Hellbound script tests.
#[cfg(test)]
pub(crate) mod testing {
    use crate::testing::ScriptTest;

    /// Sets the `HBLevel` variable the scripts read (what `//hellbound_setlevel` writes).
    pub(crate) fn set_level(t: &mut ScriptTest, script: &str, level: i32) {
        t.world.run_as_script(script, |ctx| {
            ctx.set_global_variable("HBLevel", &level.to_string());
        });
    }

    /// The last page `player` was sent in `out`.
    pub(crate) fn last_page(out: &crate::testing::Sent, player: i32) -> String {
        crate::testing::pages_to(out, player).pop().unwrap_or_default()
    }
}
