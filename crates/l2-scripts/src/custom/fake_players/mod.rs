//! `data/scripts/custom/FakePlayers`: the pvp flag of the fake players.

pub mod pvp_flagging_stop_task;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(pvp_flagging_stop_task::PvpFlaggingStopTask);
}
