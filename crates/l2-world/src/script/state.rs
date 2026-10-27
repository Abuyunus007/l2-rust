//! Quest states as data: the state of one quest of one player, the quests of a player, and the
//! `character_quests` writes they cause.
// Java: mechanics/script/{QuestState,State,QuestType}.java
// Java: entity/actor/Player.java (_quests, getQuestState, setQuestState, delQuestState,
//       _notifyQuestOfDeathList)
//
// The methods that change a state and talk to the client or the database are on the handle
// [`super::Qs`]; this file only keeps the values, so it has no world access.

use std::collections::{BTreeMap, HashMap};

/// Java `State.CREATED`.
pub const CREATED: u8 = 0;
/// Java `State.STARTED`.
pub const STARTED: u8 = 1;
/// Java `State.COMPLETED`.
pub const COMPLETED: u8 = 2;

/// Java `State.getStateName(state)`: the text of the `<state>` row.
pub fn state_name(state: u8) -> &'static str {
    match state {
        STARTED => "Started",
        COMPLETED => "Completed",
        _ => "Start",
    }
}

/// Java `State.getStateId(statename)`: anything unknown is `CREATED`.
pub fn state_id(name: &str) -> u8 {
    match name {
        "Started" => STARTED,
        "Completed" => COMPLETED,
        _ => CREATED,
    }
}

/// Java `QuestType`: how `exitQuest(QuestType)` ends a quest.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuestType {
    /// The rows go, the quest can be taken again.
    Repeatable,
    /// The state row stays `Completed`.
    OneTime,
    /// Like `OneTime`, plus the `restartTime` variable (the next 6:30).
    Daily,
}

/// Java `QuestState`: the state, the condition and the variables of one quest of one player.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct QuestState {
    /// Java `_questName`: the script's name as written in `character_quests.name`.
    pub(crate) quest_name: String,
    pub(crate) state: u8,
    /// Java `_cond`: kept apart from the `cond` variable, parsed when it is set.
    pub(crate) cond: i32,
    /// Java `_vars`. `exitQuest` sets it to `null`; an empty map behaves the same.
    pub(crate) vars: HashMap<String, String>,
    /// Java `_isExitQuestOnCleanUp`.
    pub(crate) exit_quest_on_clean_up: bool,
}

impl QuestState {
    /// Java `new QuestState(quest, player, state)` without the registration in the player.
    pub(crate) fn new(quest_name: &str, state: u8) -> Self {
        Self {
            quest_name: quest_name.to_owned(),
            state,
            cond: 0,
            vars: HashMap::new(),
            exit_quest_on_clean_up: false,
        }
    }

    /// Java `getQuestName()`.
    pub fn quest_name(&self) -> &str {
        &self.quest_name
    }

    /// Java `getState()`.
    pub fn state(&self) -> u8 {
        self.state
    }

    /// Java `isCreated()`.
    pub fn is_created(&self) -> bool {
        self.state == CREATED
    }

    /// Java `isStarted()`.
    pub fn is_started(&self) -> bool {
        self.state == STARTED
    }

    /// Java `isCompleted()`.
    pub fn is_completed(&self) -> bool {
        self.state == COMPLETED
    }

    /// Java `get(variable)`.
    pub fn get(&self, variable: &str) -> Option<&str> {
        self.vars.get(variable).map(String::as_str)
    }

    /// Java `isSet(variable)`.
    pub fn is_set(&self, variable: &str) -> bool {
        self.vars.contains_key(variable)
    }

    /// Java `getInt(variable)`: 0 for a missing, empty or non-numeric value (Java logs the
    /// last).
    pub fn get_int(&self, variable: &str) -> i32 {
        match self.vars.get(variable) {
            None => 0,
            Some(text) if text.is_empty() => 0,
            Some(text) => match text.parse::<i32>() {
                Ok(value) => value,
                Err(_) => {
                    tracing::info!(
                        "Quest {}, method getInt({variable}), tried to parse a non-integer value ({text}).",
                        self.quest_name
                    );
                    0
                }
            },
        }
    }

    /// Java `getCond()`: the condition, only while the quest is started.
    pub fn cond(&self) -> i32 {
        if self.is_started() { self.cond } else { 0 }
    }

    /// Java `isCond(condition)`: compares the stored condition whatever the state.
    pub fn is_cond(&self, condition: i32) -> bool {
        self.cond == condition
    }

    /// Java `getMemoState()`.
    pub fn memo_state(&self) -> i32 {
        if self.is_started() { self.get_int(MEMO_VAR) } else { 0 }
    }

    /// Java `isMemoState(memoState)`: reads the variable whatever the state.
    pub fn is_memo_state(&self, memo_state: i32) -> bool {
        self.get_int(MEMO_VAR) == memo_state
    }

    /// Java `getMemoStateEx(slot)`.
    pub fn memo_state_ex(&self, slot: i32) -> i32 {
        if self.is_started() {
            self.get_int(&format!("{MEMO_EX_VAR}{slot}"))
        } else {
            0
        }
    }

    /// Java `isExitQuestOnCleanUp()`.
    pub fn is_exit_quest_on_clean_up(&self) -> bool {
        self.exit_quest_on_clean_up
    }

    /// Java `setInternal(variable, value)`: a value read from the database, without a write.
    pub(crate) fn set_internal(&mut self, variable: &str, value: Option<&str>) {
        let Some(value) = value else {
            self.vars.insert(variable.to_owned(), String::new());
            return;
        };
        if variable == COND_VAR
            && let Ok(cond) = value.parse::<i32>()
        {
            self.cond = cond;
        }
        self.vars.insert(variable.to_owned(), value.to_owned());
    }
}

/// Java `QuestState.COND_VAR`.
pub const COND_VAR: &str = "cond";
/// Java `QuestState.RESTART_VAR`.
pub const RESTART_VAR: &str = "restartTime";
/// Java `QuestState.MEMO_VAR`.
pub const MEMO_VAR: &str = "memoState";
/// Java `QuestState.MEMO_EX_VAR`.
pub const MEMO_EX_VAR: &str = "memoStateEx";
/// The step flags `setCond` keeps when steps were skipped.
pub const COMPLETED_STATE_FLAGS_VAR: &str = "__compltdStateFlags";
/// The `var` of the state row.
pub const STATE_VAR: &str = "<state>";

/// The quests of one online player (Java `Player._quests`, a `ConcurrentSkipListMap` with
/// `String.CASE_INSENSITIVE_ORDER`: the names match whatever their case and iterate sorted that
/// way), plus the quests that want to hear about the player's death.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerQuests {
    /// Keyed by the lower-case name; for the ASCII script names that sorts like Java's
    /// case-insensitive order.
    pub(crate) states: BTreeMap<String, QuestState>,
    /// Java `_notifyQuestOfDeathList`: the names of the quests, in the order they asked.
    pub(crate) notify_of_death: Vec<String>,
}

impl PlayerQuests {
    /// Java `getQuestState(name)`.
    pub fn get(&self, name: &str) -> Option<&QuestState> {
        self.states.get(&name.to_ascii_lowercase())
    }

    pub(crate) fn get_mut(&mut self, name: &str) -> Option<&mut QuestState> {
        self.states.get_mut(&name.to_ascii_lowercase())
    }

    /// Java `setQuestState(qs)`.
    pub(crate) fn insert(&mut self, qs: QuestState) {
        self.states.insert(qs.quest_name.to_ascii_lowercase(), qs);
    }

    /// Java `delQuestState(name)`.
    pub(crate) fn remove(&mut self, name: &str) {
        self.states.remove(&name.to_ascii_lowercase());
    }

    /// Java `hasQuestState(name)`.
    pub fn contains(&self, name: &str) -> bool {
        self.states.contains_key(&name.to_ascii_lowercase())
    }

    /// Every state, in Java's iteration order.
    pub fn iter(&self) -> impl Iterator<Item = &QuestState> {
        self.states.values()
    }
}

/// One `character_quests` statement of Java's `Quest` (`createQuestVarInDb`, ...), sent to the
/// persistence task in the order Java runs them (D-15). Per-player rows: written side by side
/// as Java writes them (D-54).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum QuestWrite {
    /// Java `createQuestVarInDb`: `INSERT ... ON DUPLICATE KEY UPDATE value=?` (also the state
    /// row of `createQuestInDb`).
    InsertVar {
        char_id: i32,
        name: String,
        var: String,
        value: String,
    },
    /// Java `updateQuestVarInDb` (also the state row of `updateQuestInDb`).
    UpdateVar {
        char_id: i32,
        name: String,
        var: String,
        value: String,
    },
    /// Java `deleteQuestVarInDb`.
    DeleteVar { char_id: i32, name: String, var: String },
    /// Java `deleteQuestInDb(qs, repeatable)`: every row, or every row but the state row.
    DeleteQuest {
        char_id: i32,
        name: String,
        repeatable: bool,
    },
}

/// The `character_quests` rows of a character as the session read them at `CharacterSelect`
/// (D-15): the state rows (`var = '<state>'`) and the other rows, each in the database's order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct QuestLoad {
    /// `(name, value)` of the state rows.
    pub states: Vec<(String, Option<String>)>,
    /// `(name, var, value)` of the other rows.
    pub variables: Vec<(String, String, Option<String>)>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_names_are_javas() {
        assert_eq!(state_name(CREATED), "Start");
        assert_eq!(state_name(STARTED), "Started");
        assert_eq!(state_name(COMPLETED), "Completed");
        assert_eq!(state_id("Started"), STARTED);
        assert_eq!(state_id("Completed"), COMPLETED);
        assert_eq!(state_id("Start"), CREATED);
        assert_eq!(state_id("anything"), CREATED);
    }

    #[test]
    fn set_internal_parses_cond_and_keeps_bad_values() {
        let mut qs = QuestState::new("Q", STARTED);
        qs.set_internal("cond", Some("3"));
        qs.set_internal("x", None);
        qs.set_internal("y", Some("abc"));
        assert_eq!(qs.cond(), 3);
        assert_eq!(qs.get("x"), Some(""));
        assert_eq!(qs.get_int("y"), 0);
        // A cond that is not a number leaves the old one (Java ignores the exception).
        qs.set_internal("cond", Some("z"));
        assert_eq!(qs.cond(), 3);
        assert_eq!(qs.get("cond"), Some("z"));
    }

    #[test]
    fn cond_and_memo_read_zero_unless_started() {
        let mut qs = QuestState::new("Q", CREATED);
        qs.set_internal("cond", Some("2"));
        qs.set_internal("memoState", Some("5"));
        assert_eq!(qs.cond(), 0);
        assert!(qs.is_cond(2));
        assert_eq!(qs.memo_state(), 0);
        assert!(qs.is_memo_state(5));
    }

    #[test]
    fn names_match_case_insensitively_and_iterate_sorted() {
        let mut quests = PlayerQuests::default();
        quests.insert(QuestState::new("Q00002_B", STARTED));
        quests.insert(QuestState::new("q00001_a", STARTED));
        quests.insert(QuestState::new("NewbieGuide", CREATED));
        assert!(quests.contains("Q00001_A"));
        assert!(quests.get("newbieguide").is_some());
        let names: Vec<&str> = quests.iter().map(QuestState::quest_name).collect();
        assert_eq!(names, ["NewbieGuide", "q00001_a", "Q00002_B"]);
        quests.remove("Q00002_b");
        assert!(!quests.contains("Q00002_B"));
    }
}
