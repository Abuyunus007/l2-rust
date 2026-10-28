//! Quest 70: Saga of the Phoenix Knight (Paladin -> Phoenix Knight).
// Java: dist/game/data/scripts/quests/Q00070_SagaOfThePhoenixKnight/Q00070_SagaOfThePhoenixKnight.java
//
// Pattern: a saga (`docs/design/scripts.md`, "Quest patterns"): the Java class only fills the
// fields of `AbstractSagaQuest`; here that is a `SagaData` constant and one `saga_quest!` line.
// Copy this file for the other 33 sagas, the arrays in Java's order.

use super::abstract_saga_quest::{SagaData, saga_quest};

/// Java `Q00070_SagaOfThePhoenixKnight`.
pub struct Q00070SagaOfThePhoenixKnight;

const DATA: SagaData = SagaData {
    quest_id: 70,
    npc: [
        30849, 31624, 31277, 30849, 31631, 31646, 31647, 31650, 31654, 31655, 31657, 31277,
    ],
    items: [
        7080, 7534, 7081, 7485, 7268, 7299, 7330, 7361, 7392, 7423, 7093, 6482,
    ],
    mob: [27286, 27219, 27278],
    class_id: &[90],
    prev_class: &[0x05],
    npc_spawn_locations: [
        (191046, -40640, -3042),
        (46087, -36372, -1685),
        (46066, -36396, -1685),
    ],
    text: [
        "PLAYERNAME! Pursued to here! However, I jumped out of the Banshouren boundaries! You look at the giant as the sign of power!",
        "... Oh ... good! So it was ... let's begin!",
        "I do not have the patience ..! I have been a giant force ...! Cough chatter ah ah ah!",
        "Paying homage to those who disrupt the orderly will be PLAYERNAME's death!",
        "Now, my soul freed from the shackles of the millennium, Halixia, to the back side I come ...",
        "Why do you interfere others' battles?",
        "This is a waste of time.. Say goodbye...!",
        "...That is the enemy",
        "...Goodness! PLAYERNAME you are still looking?",
        "PLAYERNAME ... Not just to whom the victory. Only personnel involved in the fighting are eligible to share in the victory.",
        "Your sword is not an ornament. Don't you think, PLAYERNAME?",
        "Goodness! I no longer sense a battle there now.",
        "let...",
        "Only engaged in the battle to bar their choice. Perhaps you should regret.",
        "The human nation was foolish to try and fight a giant's strength.",
        "Must...Retreat... Too...Strong.",
        "PLAYERNAME. Defeat...by...retaining...and...Mo...Hacker",
        "....! Fight...Defeat...It...Fight...Defeat...It...",
    ],
};

saga_quest!(
    Q00070SagaOfThePhoenixKnight,
    "Q00070_SagaOfThePhoenixKnight",
    DATA
);
