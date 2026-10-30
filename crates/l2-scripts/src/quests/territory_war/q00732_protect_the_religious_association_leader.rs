//! Quest 732: Protect the Religious Association Leader.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00732_ProtectTheReligiousAssociationLeader.java

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00732_ProtectTheReligiousAssociationLeader`.
pub struct Q00732ProtectTheReligiousAssociationLeader;

/// Java `getTerritoryIdForThisNPCId(npcId)`.
fn territory_of_npc(npc_id: i32) -> i32 {
    81 + ((npc_id - 36510) / 6)
}

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00732_ProtectTheReligiousAssociationLeader",
    quest_id: 732,
    npc_ids: &[36510, 36516, 36522, 36528, 36534, 36540, 36546, 36552, 36558],
    territory_of_npc,
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00732ProtectTheReligiousAssociationLeader, DATA);
