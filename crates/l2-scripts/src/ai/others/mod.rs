//! `data/scripts/ai/others`: NPC and monster scripts that belong to no area.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`], in the
//! same order (Java's load order is the folder listing).

pub mod asamah;
pub mod manor_manager;
pub mod monument_of_heroes;
pub mod symbol_maker;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(asamah::Asamah);
    r.add(manor_manager::ManorManager);
    r.add(monument_of_heroes::MonumentOfHeroes);
    r.add(symbol_maker::SymbolMaker);
}
