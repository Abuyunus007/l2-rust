//! `data/scripts/ai/others`: NPC and monster scripts that belong to no area.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`], in the
//! same order (Java's load order is the folder listing).

pub mod abercrombie;
pub mod altars_of_sacrifice;
pub mod arena_manager;
pub mod asamah;
pub mod asher;
pub mod avant_garde;
pub mod ballista;
pub mod black_judge;
pub mod black_marketeer_of_mammon;
pub mod blackdagger_wing;
pub mod bleeding_fly;
pub mod clan_trader;
pub mod class_master;
pub mod crimson_hatu_otis;
pub mod delusion_teleport;
pub mod dorian;
pub mod dust_rider;
pub mod emerald_horn;
pub mod epidos;
pub mod fairy_trees;
pub mod fame_manager;
pub mod flee_monsters;
pub mod gatekeeper_spirit;
pub mod gordon;
pub mod hunting_grounds_teleport;
pub mod kier;
pub mod manor_manager;
pub mod monument_of_heroes;
pub mod muscle_bomber;
pub mod subclass_certification;
pub mod symbol_maker;

use l2_world::ScriptRegistry;

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(abercrombie::Abercrombie);
    r.add(altars_of_sacrifice::AltarsOfSacrifice);
    r.add(arena_manager::ArenaManager);
    r.add(asamah::Asamah);
    r.add(asher::Asher);
    r.add(ballista::Ballista);
    r.add(black_judge::BlackJudge);
    r.add(black_marketeer_of_mammon::BlackMarketeerOfMammon);
    r.add(blackdagger_wing::BlackdaggerWing);
    r.add(bleeding_fly::BleedingFly);
    r.add(clan_trader::ClanTrader);
    r.add(crimson_hatu_otis::CrimsonHatuOtis);
    r.add(delusion_teleport::DelusionTeleport);
    r.add(dorian::Dorian);
    r.add(dust_rider::DustRider);
    r.add(emerald_horn::EmeraldHorn);
    r.add(epidos::Epidos);
    r.add(fairy_trees::FairyTrees);
    r.add(fame_manager::FameManager);
    r.add(flee_monsters::FleeMonsters);
    r.add(gatekeeper_spirit::GatekeeperSpirit);
    r.add(gordon::Gordon);
    r.add(hunting_grounds_teleport::HuntingGroundsTeleport);
    r.add(kier::Kier);
    r.add(manor_manager::ManorManager);
    r.add(monument_of_heroes::MonumentOfHeroes);
    r.add(muscle_bomber::MuscleBomber);
    r.add(avant_garde::AvantGarde);
    r.add(class_master::ClassMaster::default());
    r.add(manor_manager::ManorManager);
    r.add(monument_of_heroes::MonumentOfHeroes);
    r.add(subclass_certification::SubclassCertification);
    r.add(symbol_maker::SymbolMaker);
}
