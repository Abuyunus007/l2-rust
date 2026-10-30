//! Quest 725: For the Sake of the Territory - Schuttgart.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00725_ForTheSakeOfTheTerritorySchuttgart.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00725_ForTheSakeOfTheTerritorySchuttgart`.
pub struct Q00725ForTheSakeOfTheTerritorySchuttgart;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00725_ForTheSakeOfTheTerritorySchuttgart",
    quest_id: 725,
    catapult_id: 36507,
    territory_id: 89,
    leader_ids: &[36556, 36558, 36561, 36599],
    guard_ids: &[36557, 36559, 36560],
    npc_string: &[NpcStringId::TheCatapultOfSchuttgartHasBeenDestroyed],
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00725ForTheSakeOfTheTerritorySchuttgart, DATA);
