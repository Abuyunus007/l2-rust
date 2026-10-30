//! Quest 730: Protect the Supplies Safe.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00730_ProtectTheSuppliesSafe.java

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00730_ProtectTheSuppliesSafe`.
pub struct Q00730ProtectTheSuppliesSafe;

/// Java `getTerritoryIdForThisNPCId(npcId)`.
fn territory_of_npc(npc_id: i32) -> i32 {
    npc_id - 36510
}

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00730_ProtectTheSuppliesSafe",
    quest_id: 730,
    npc_ids: &[36591, 36592, 36593, 36594, 36595, 36596, 36597, 36598, 36599],
    territory_of_npc,
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00730ProtectTheSuppliesSafe, DATA);
