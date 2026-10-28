//! `data/scripts/ai/others`: NPC and monster scripts that belong to no area.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`], in the
//! same order (Java's load order is the folder listing).

pub mod abercrombie;
pub mod arena_manager;
pub mod asamah;
pub mod asher;
pub mod black_judge;
pub mod black_marketeer_of_mammon;
pub mod clan_trader;
pub mod delusion_teleport;
pub mod dorian;
pub mod fame_manager;
pub mod gatekeeper_spirit;
pub mod hunting_grounds_teleport;
pub mod kier;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(abercrombie::Abercrombie);
    r.add(arena_manager::ArenaManager);
    r.add(asamah::Asamah);
    r.add(asher::Asher);
    r.add(black_judge::BlackJudge);
    r.add(black_marketeer_of_mammon::BlackMarketeerOfMammon);
    r.add(clan_trader::ClanTrader);
    r.add(delusion_teleport::DelusionTeleport);
    r.add(dorian::Dorian);
    r.add(fame_manager::FameManager);
    r.add(gatekeeper_spirit::GatekeeperSpirit);
    r.add(hunting_grounds_teleport::HuntingGroundsTeleport);
    r.add(kier::Kier);
}
