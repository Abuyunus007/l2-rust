//! `data/scripts/ai/areas/FantasyIsle`: the Fantasy Isle scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod handys_block_checker_event;
pub mod krateis_cube;
pub mod parade;
pub mod talent_show;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(handys_block_checker_event::HandysBlockCheckerEvent);
    r.add(krateis_cube::KrateisCube);
    r.add(parade::Parade);
    r.add(talent_show::TalentShow);
}
