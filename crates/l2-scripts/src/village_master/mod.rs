//! `data/scripts/village_master`: the village masters' scripts.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`].

pub mod alliance_master;
pub mod clan_master;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(alliance_master::AllianceMaster);
    r.add(clan_master::ClanMaster);
}
