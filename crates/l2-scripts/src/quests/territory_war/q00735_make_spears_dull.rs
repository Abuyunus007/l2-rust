//! Quest 735: Make Spears Dull!
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00735_MakeSpearsDull.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00735_MakeSpearsDull`.
pub struct Q00735MakeSpearsDull;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00735_MakeSpearsDull",
    quest_id: 735,
    class_ids: &[
        23, 101, 36, 108, 8, 93, 2, 88, 3, 89, 48, 114, 46, 113, 55, 117, 9, 92, 24, 102, 37, 109, 34, 107,
        21, 100, 127, 131, 128, 132, 129, 133, 130, 134, 135, 136,
    ],
    npc_string: &[
        NpcStringId::YouHaveDefeatedS2OfS1WarriorsAndRogues,
        NpcStringId::YouWeakenedTheEnemySAttack,
    ],
    random_min: 15,
    random_max: 20,
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00735MakeSpearsDull, DATA);
