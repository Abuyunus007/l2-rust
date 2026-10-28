//! The compiled-in scripts of `data/scripts` (D-05, D-16: no hot reload) on the script engine
//! of `l2_world::script`. Porting guide: `docs/design/scripts.md`.
//!
//! The folders follow Java's `data/scripts`; each has a `register` function and the game server
//! calls [`register_all`] once at startup. The order is Java's load order (the folder listing:
//! `ai` before `quests`), which is the order listeners of the same NPC hear an event.

// The ports keep Java's `switch` / `if` shape line by line, so a reviewer can lay them next to the
// Java file; folding an `if` into a match guard would hide that.
#![allow(
    clippy::collapsible_match,
    clippy::collapsible_if,
    clippy::collapsible_else_if
)]

pub mod ai;
pub mod custom;
pub mod quests;
pub mod village_master;

#[cfg(test)]
pub(crate) mod testing;

use l2_world::ScriptRegistry;

/// Every script, in Java's load order.
pub fn register_all(r: &mut ScriptRegistry) {
    ai::register(r);
    custom::register(r);
    quests::register(r);
    village_master::register(r);
}

/// A registry with every script.
pub fn all() -> ScriptRegistry {
    let mut r = ScriptRegistry::new();
    register_all(&mut r);
    r
}
