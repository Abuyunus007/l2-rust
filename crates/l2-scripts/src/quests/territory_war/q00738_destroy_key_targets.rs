//! Quest 738: Destroy Key Targets.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00738_DestroyKeyTargets.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00738_DestroyKeyTargets`.
pub struct Q00738DestroyKeyTargets;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00738_DestroyKeyTargets",
    quest_id: 738,
    class_ids: &[51, 115, 57, 118],
    npc_string: &[
        NpcStringId::YouHaveDefeatedS2OfS1WarsmithsAndOverlords,
        NpcStringId::YouDestroyedTheEnemySProfessionals,
    ],
    random_min: 3,
    random_max: 8,
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00738DestroyKeyTargets, DATA);
