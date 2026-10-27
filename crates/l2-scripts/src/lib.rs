//! The compiled-in scripts of `data/scripts` (D-05, D-16: no hot reload) on the script engine
//! of `l2_world::script`. Porting guide: `docs/design/scripts.md`.
//!
//! The folders follow Java's `data/scripts`; each has a `register` function and the game server
//! calls [`register_all`] once at startup. The order is Java's load order (the folder listing:
//! `ai` before `quests`), which is the order listeners of the same NPC hear an event.

pub mod ai;
pub mod quests;

#[cfg(test)]
pub(crate) mod testing;

use l2_world::ScriptRegistry;

/// Every script, in Java's load order.
pub fn register_all(r: &mut ScriptRegistry) {
    ai::register(r);
    quests::register(r);
}

/// A registry with every script.
pub fn all() -> ScriptRegistry {
    let mut r = ScriptRegistry::new();
    register_all(&mut r);
    r
}
