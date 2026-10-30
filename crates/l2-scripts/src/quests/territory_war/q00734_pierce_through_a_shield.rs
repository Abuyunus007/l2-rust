//! Quest 734: Pierce through a Shield!
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00734_PierceThroughAShield.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00734_PierceThroughAShield`.
pub struct Q00734PierceThroughAShield;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00734_PierceThroughAShield",
    quest_id: 734,
    class_ids: &[6, 91, 5, 90, 20, 99, 33, 106],
    npc_string: &[
        NpcStringId::YouHaveDefeatedS2OfS1Knights,
        NpcStringId::YouWeakenedTheEnemySDefense,
    ],
    random_min: 10,
    random_max: 15,
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00734PierceThroughAShield, DATA);
