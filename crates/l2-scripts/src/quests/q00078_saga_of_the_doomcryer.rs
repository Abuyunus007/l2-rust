//! Quest 78: Saga of the Doomcryer.
// Java: dist/game/data/scripts/quests/Q00078_SagaOfTheDoomcryer/Q00078_SagaOfTheDoomcryer.java
//
// Pattern: a saga (`docs/design/scripts.md`, "Quest patterns"): the Java class only fills the
// fields of `AbstractSagaQuest`; here that is a `SagaData` constant and one `saga_quest!` line.
// The hooks are the shared ones of `abstract_saga_quest.rs`.

use super::abstract_saga_quest::{SagaData, saga_quest};

/// Java `Q00078SagaOfTheDoomcryer`.
pub struct Q00078SagaOfTheDoomcryer;

const DATA: SagaData = SagaData {
    quest_id: 78,
    npc: [
        31336, 31624, 31589, 31290, 31642, 31646, 31649, 31650, 31654, 31655, 31657, 31290,
    ],
    items: [
        7080, 7539, 7081, 7493, 7276, 7307, 7338, 7369, 7400, 7431, 7101, 0,
    ],
    mob: [27295, 27227, 27285],
    class_id: &[116],
    prev_class: &[0x34],
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

saga_quest!(Q00078SagaOfTheDoomcryer, "Q00078_SagaOfTheDoomcryer", DATA);

#[cfg(test)]
mod tests {
    use l2_util::ObjectId;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::{DATA, Q00078SagaOfTheDoomcryer as Saga};
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00078_SagaOfTheDoomcryer";

    fn npcs() -> String {
        format!(
            r#"<npc id="{}" type="Folk" level="70" name="Start"/>
            <npc id="{}" type="Folk" level="70" name="Step 1"/>"#,
            DATA.npc[0], DATA.npc[2]
        )
    }

    fn items() -> String {
        format!(
            r#"<item id="{}" type="EtcItem" name="Saga item"><set name="is_questitem" val="true"/></item>"#,
            DATA.items[10]
        )
    }

    fn setup() -> (ScriptTest, ObjectId, ObjectId) {
        let mut registry = ScriptRegistry::new();
        registry.add(Saga);
        let pages = Pages::of(&Saga)
            .page("0-01.htm", &["0-1"])
            .page("0-05.htm", &["accept"])
            .page("2-01.htm", &["2-1"])
            .plain(&["0-02.htm", "0-03.htm", "2-05.htm"])
            .npcs(&[DATA.npc[0], DATA.npc[2]]);
        let mut t = ScriptTest::with_pages(registry, &npcs(), &items(), pages);
        let start = t.spawn(DATA.npc[0], 20);
        let step1 = t.spawn(DATA.npc[2], 40);
        (t, start, step1)
    }

    #[test]
    fn a_lower_level_or_another_class_gets_no_start() {
        let (mut t, start, _) = setup();
        t.enter_as(1, 75, race::HUMAN, DATA.prev_class[0], vec![]);
        assert!(t.quest_page(1, start).contains("0-01.htm"));
        assert!(t.event(1, NAME, "0-1").contains("0-02.htm"));
        assert!(t.take_writes().is_empty());

        t.enter_as(2, 76, race::HUMAN, 0, vec![]);
        assert!(!t.quest_page(2, start).contains("0-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_writes_the_cond_and_gives_the_item_then_step_1() {
        let (mut t, start, step1) = setup();
        t.enter_as(1, 76, race::HUMAN, DATA.prev_class[0], vec![]);
        assert!(t.quest_page(1, start).contains("0-01.htm"));
        assert!(t.event(1, NAME, "0-1").contains("0-05.htm"));
        assert!(t.event(1, NAME, "accept").contains("0-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert_eq!(t.count(1, DATA.items[10]), 1);

        assert!(t.quest_page(1, step1).contains("2-01.htm"));
        assert!(t.event(1, NAME, "2-1").contains("2-05.htm"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
    }
}
