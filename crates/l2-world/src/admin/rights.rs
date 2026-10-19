//! Who may run which admin command.
// Java: data/xml/AdminData.java (hasAccess, requireConfirm, getAccessLevel)
// Java: data/holders/{AccessLevel,AccessRight}.java
//
// The game server fills this from `config/AccessLevels.xml` and `config/AdminCommands.xml`
// (`add_level`, `add_command`); the world only asks.

use std::collections::{BTreeMap, HashMap};

/// One `<access>` row, as far as command rights need it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LevelRow {
    /// `AccessLevel.getLevel()`.
    level: i32,
    /// `childAccess`: the next level whose commands this level may also use.
    child: i32,
}

/// One `<admin>` row (Java `AccessRight`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CommandRight {
    level: i32,
    confirm: bool,
}

/// Java `AdminData`'s access levels and command rights.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AdminRights {
    levels: BTreeMap<i32, LevelRow>,
    /// Keyed by the full command, `admin_` prefix included (Java `AccessRight.getCommand`).
    commands: HashMap<String, CommandRight>,
    /// Java `_highestLevel` (starts at 0).
    highest: i32,
}

/// Cap for walking a `childAccess` chain (a cycle in the file would make Java overflow its
/// stack; here the walk just ends).
const MAX_CHILD_DEPTH: usize = 64;

impl AdminRights {
    /// Java `AdminData.parseDocument`, `<access>`: `level` and `childAccess` (default 0).
    pub fn add_level(&mut self, level: i32, child: i32) {
        if level > self.highest {
            self.highest = level;
        }
        self.levels.insert(level, LevelRow { level, child });
    }

    /// Java `AdminData.parseDocument`, `<admin>`: `command` is the name without `admin_`,
    /// `level` is `accessLevel` (default 100), `confirm` is `confirmDlg` (default false).
    pub fn add_command(&mut self, command: &str, level: i32, confirm: bool) {
        self.commands
            .insert(format!("admin_{command}"), CommandRight { level, confirm });
    }

    /// Number of access levels (Java: "Loaded N access levels.").
    pub fn level_count(&self) -> usize {
        self.levels.len()
    }

    /// Number of command rights (Java: "Loaded N access commands.").
    pub fn command_count(&self) -> usize {
        self.commands.len()
    }

    /// Java `getAccessLevel(level)`: below 0 is the banned row, a level the file does not list is
    /// `new AccessLevel()` (level 0, no child), whatever number was asked for.
    fn row(&self, level: i32) -> LevelRow {
        if level < 0 {
            // Java would hand out `null` when the file has no -1 row.
            return self
                .levels
                .get(&-1)
                .copied()
                .unwrap_or(LevelRow { level: -1, child: 0 });
        }
        self.levels
            .get(&level)
            .copied()
            .unwrap_or(LevelRow { level: 0, child: 0 })
    }

    /// `getAccessLevel(level).getLevel()`: the level a character with this `accesslevel` column
    /// really has. NOTE(java-quirk): an unlisted number counts as level 0.
    pub fn effective_level(&self, level: i32) -> i32 {
        self.row(level).level
    }

    /// Java `AccessLevel.hasChildAccess`: is `wanted` found walking down the `childAccess` chain?
    fn has_child_access(&self, from: LevelRow, wanted: i32) -> bool {
        let mut row = from;
        for _ in 0..MAX_CHILD_DEPTH {
            if row.child <= 0 {
                return false;
            }
            row = self.row(row.child);
            if row.level == wanted {
                return true;
            }
        }
        false
    }

    /// Java `AdminData.hasAccess(adminCommand, accessLevel)` for a character with this
    /// `accesslevel` column. `command` includes the `admin_` prefix.
    pub fn has_access(&self, command: &str, player_level: i32) -> bool {
        let player = self.row(player_level);
        match self.commands.get(command) {
            Some(right) => {
                let required = self.row(right.level).level;
                required == player.level || self.has_child_access(player, required)
            }
            // "No rights defined for admin command": the highest level gets the command on the
            // spot, everybody else is refused.
            None => player.level > 0 && player.level == self.highest,
        }
    }

    /// Java `AdminData.requireConfirm(command)`, called right after `hasAccess`.
    ///
    /// NOTE(java-quirk): a command the file does not list is registered by `hasAccess` for the
    /// highest level with `confirmDlg` true, so its first use asks for a confirmation.
    pub fn requires_confirm(&self, command: &str, player_level: i32) -> bool {
        match self.commands.get(command) {
            Some(right) => right.confirm,
            None => {
                let player = self.row(player_level);
                player.level > 0 && player.level == self.highest
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn shipped() -> AdminRights {
        let mut rights = AdminRights::default();
        for (level, child) in [
            (-1, 0),
            (0, 0),
            (10, 0),
            (20, 10),
            (30, 20),
            (40, 30),
            (50, 40),
            (60, 50),
            (70, 60),
            (100, 70),
        ] {
            rights.add_level(level, child);
        }
        rights.add_command("admin", 30, false);
        rights.add_command("kill", 100, false);
        rights.add_command("recall_char_menu", 100, true);
        rights
    }

    #[test]
    fn a_level_may_use_the_commands_of_its_children() {
        let rights = shipped();
        // 100 -> 70 -> 60 -> 50 -> 40 -> 30.
        assert!(rights.has_access("admin_admin", 100));
        assert!(rights.has_access("admin_admin", 70));
        assert!(rights.has_access("admin_admin", 30));
        assert!(!rights.has_access("admin_admin", 20));
        assert!(!rights.has_access("admin_admin", 0));
        assert!(rights.has_access("admin_kill", 100));
        assert!(!rights.has_access("admin_kill", 70));
    }

    #[test]
    fn an_unlisted_level_counts_as_level_zero() {
        let rights = shipped();
        assert_eq!(rights.effective_level(55), 0);
        assert!(!rights.has_access("admin_admin", 55));
        assert_eq!(rights.effective_level(-7), -1);
    }

    #[test]
    fn an_unlisted_command_is_for_the_highest_level_only() {
        let rights = shipped();
        assert!(rights.has_access("admin_nothing", 100));
        assert!(!rights.has_access("admin_nothing", 70));
        assert!(!rights.has_access("admin_nothing", 0));
        // The quirk: its first use wants a confirmation.
        assert!(rights.requires_confirm("admin_nothing", 100));
        assert!(!rights.requires_confirm("admin_kill", 100));
        assert!(rights.requires_confirm("admin_recall_char_menu", 100));
    }

    #[test]
    fn a_child_cycle_ends() {
        let mut rights = AdminRights::default();
        rights.add_level(5, 6);
        rights.add_level(6, 5);
        rights.add_command("x", 9, false);
        assert!(!rights.has_access("admin_x", 5));
    }
}
