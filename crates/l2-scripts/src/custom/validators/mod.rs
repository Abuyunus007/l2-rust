//! `data/scripts/custom/Validators`: the login checks of skills and items.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod sub_class_skills;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(sub_class_skills::SubClassSkills);
}
