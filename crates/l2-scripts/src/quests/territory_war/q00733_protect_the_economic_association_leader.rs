//! Quest 733: Protect the Economic Association Leader.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00733_ProtectTheEconomicAssociationLeader.java

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00733_ProtectTheEconomicAssociationLeader`.
pub struct Q00733ProtectTheEconomicAssociationLeader;

/// Java `getTerritoryIdForThisNPCId(npcId)`.
fn territory_of_npc(npc_id: i32) -> i32 {
    81 + ((npc_id - 36513) / 6)
}

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00733_ProtectTheEconomicAssociationLeader",
    quest_id: 733,
    npc_ids: &[36513, 36519, 36525, 36531, 36537, 36543, 36549, 36555, 36561],
    territory_of_npc,
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00733ProtectTheEconomicAssociationLeader, DATA);
