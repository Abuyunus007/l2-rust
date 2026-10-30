//! Quest 737: Deny Blessings.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00737_DenyBlessings.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00737_DenyBlessings`.
pub struct Q00737DenyBlessings;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00737_DenyBlessings",
    quest_id: 737,
    class_ids: &[43, 112, 30, 105, 16, 97, 17, 98, 52, 116],
    npc_string: &[
        NpcStringId::YouHaveDefeatedS2OfS1HealersAndBuffers,
        NpcStringId::YouWeakenedTheEnemySAttack,
    ],
    random_min: 3,
    random_max: 8,
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00737DenyBlessings, DATA);
