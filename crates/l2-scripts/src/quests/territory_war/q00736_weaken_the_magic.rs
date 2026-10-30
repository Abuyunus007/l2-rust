//! Quest 736: Weaken the magic!
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00736_WeakenTheMagic.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00736_WeakenTheMagic`.
pub struct Q00736WeakenTheMagic;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00736_WeakenTheMagic",
    quest_id: 736,
    class_ids: &[40, 110, 27, 103, 13, 95, 12, 94, 41, 111, 28, 104, 14, 96],
    npc_string: &[
        NpcStringId::YouHaveDefeatedS2OfS1Enemies,
        NpcStringId::YouWeakenedTheEnemySMagic,
    ],
    random_min: 10,
    random_max: 15,
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00736WeakenTheMagic, DATA);
