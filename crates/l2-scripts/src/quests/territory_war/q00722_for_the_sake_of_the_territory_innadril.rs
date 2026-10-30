//! Quest 722: For the Sake of the Territory - Innadril.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00722_ForTheSakeOfTheTerritoryInnadril.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00722_ForTheSakeOfTheTerritoryInnadril`.
pub struct Q00722ForTheSakeOfTheTerritoryInnadril;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00722_ForTheSakeOfTheTerritoryInnadril",
    quest_id: 722,
    catapult_id: 36504,
    territory_id: 86,
    leader_ids: &[36538, 36540, 36543, 36596],
    guard_ids: &[36539, 36541, 36542],
    npc_string: &[NpcStringId::TheCatapultOfInnadrilHasBeenDestroyed],
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00722ForTheSakeOfTheTerritoryInnadril, DATA);
