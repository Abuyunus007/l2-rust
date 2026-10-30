//! Quest 717: For the Sake of the Territory - Gludio.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00717_ForTheSakeOfTheTerritoryGludio.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00717_ForTheSakeOfTheTerritoryGludio`.
pub struct Q00717ForTheSakeOfTheTerritoryGludio;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00717_ForTheSakeOfTheTerritoryGludio",
    quest_id: 717,
    catapult_id: 36499,
    territory_id: 81,
    leader_ids: &[36508, 36510, 36513, 36591],
    guard_ids: &[36509, 36511, 36512],
    npc_string: &[NpcStringId::TheCatapultOfGludioHasBeenDestroyed],
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00717ForTheSakeOfTheTerritoryGludio, DATA);
