//! `data/scripts/quests`: the quests, one module per quest, named like the Java folder
//! (`Q00001_LettersOfLove` -> `q00001_letters_of_love`).
//!
//! Adding a quest: its file, one `pub mod` line and one `add` line below, both sorted by id
//! (`docs/design/scripts.md`).

pub mod q00001_letters_of_love;

use l2_world::ScriptRegistry;

/// Registers every quest, by id.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(q00001_letters_of_love::Q00001LettersOfLove);
}
