//! Quest 729: Protect the Territory Catapult!
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00729_ProtectTheTerritoryCatapult.java

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00729_ProtectTheTerritoryCatapult`.
pub struct Q00729ProtectTheTerritoryCatapult;

/// Java `getTerritoryIdForThisNPCId(npcId)`.
fn territory_of_npc(npc_id: i32) -> i32 {
    npc_id - 36418
}

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00729_ProtectTheTerritoryCatapult",
    quest_id: 729,
    npc_ids: &[36499, 36500, 36501, 36502, 36503, 36504, 36505, 36506, 36507],
    territory_of_npc,
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00729ProtectTheTerritoryCatapult, DATA);
