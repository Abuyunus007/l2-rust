//! `data/scripts/ai/areas/DenOfEvil`: the Den of Evil scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].
//!
//! Java's `DenOfEvil` (the Kasha's Eye effect zones) is annotated `@Disabled` ("Mobius: this needs
//! to be rewritten"): the script engine never loads it, so it is not ported.

pub mod frightened_ragna_orc;
pub mod ragna_orc_commander;
pub mod ragna_orc_hero;
pub mod ragna_orc_seer;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(frightened_ragna_orc::FrightenedRagnaOrc);
    r.add(ragna_orc_commander::RagnaOrcCommander);
    r.add(ragna_orc_hero::RagnaOrcHero);
    r.add(ragna_orc_seer::RagnaOrcSeer);
}
