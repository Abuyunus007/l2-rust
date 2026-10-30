//! Quest 731: Protect the Military Association Leader.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00731_ProtectTheMilitaryAssociationLeader.java

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00731_ProtectTheMilitaryAssociationLeader`.
pub struct Q00731ProtectTheMilitaryAssociationLeader;

/// Java `getTerritoryIdForThisNPCId(npcId)`.
fn territory_of_npc(npc_id: i32) -> i32 {
    81 + ((npc_id - 36508) / 6)
}

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00731_ProtectTheMilitaryAssociationLeader",
    quest_id: 731,
    npc_ids: &[36508, 36514, 36520, 36526, 36532, 36538, 36544, 36550, 36556],
    territory_of_npc,
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00731ProtectTheMilitaryAssociationLeader, DATA);
