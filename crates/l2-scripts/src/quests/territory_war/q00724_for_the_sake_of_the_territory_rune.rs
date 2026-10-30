//! Quest 724: For the Sake of the Territory - Rune.
// Java: dist/game/data/scripts/quests/TerritoryWarScripts/Q00724_ForTheSakeOfTheTerritoryRune.java

use l2_protocol_game::NpcStringId;

use super::territory_war_super_class::{TerritoryWarData, territory_war_quest};

/// Java `Q00724_ForTheSakeOfTheTerritoryRune`.
pub struct Q00724ForTheSakeOfTheTerritoryRune;

pub static DATA: TerritoryWarData = TerritoryWarData {
    name: "Q00724_ForTheSakeOfTheTerritoryRune",
    quest_id: 724,
    catapult_id: 36506,
    territory_id: 88,
    leader_ids: &[36550, 36552, 36555, 36598],
    guard_ids: &[36551, 36553, 36554],
    npc_string: &[NpcStringId::TheCatapultOfRuneHasBeenDestroyed],
    ..TerritoryWarData::NONE
};

territory_war_quest!(Q00724ForTheSakeOfTheTerritoryRune, DATA);
