//! Java `GlobalVariablesManager` for the scripts, with the side by side switch (D-54).
// Java: managers/GlobalVariablesManager.java, mechanics/variables/AbstractVariables.java,
//       util/StatSet.java (getInt, getLong, getBoolean, getString)
//
// # Side by side (D-54)
// The rows are global: the Java GS reads and rewrites the same table. The world loads every
// row at startup ([`World::set_global_variables`]); scripts read and change the copy in memory.
// The changes reach the database only with `CycleDatabaseWrites = True`, at shutdown (Java's
// `Shutdown` -> `storeMe`).
//
// NOTE: Java's `storeMe` deletes the whole table and writes its map back. Here other systems of
// this server write their own rows of the table directly (`nextTWStartDate`, the coliseum
// times, the fishing championship), so a whole-table rewrite would put back stale copies of
// them. The store writes only the variables the scripts changed or removed, with Java's
// `REPLACE INTO` and a `DELETE` of the removed names; the rows end up as Java's would.

use std::collections::{BTreeSet, HashMap};

use tracing::info;

use crate::outbox::PersistCommand;
use crate::world::World;

/// A write of `global_variables` (Java `GlobalVariablesManager.storeMe`, see the module notes).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GlobalVariablesWrite {
    /// `REPLACE INTO global_variables (var, value) VALUES (?, ?)` for every changed variable.
    Replace(Vec<(String, String)>),
    /// `DELETE FROM global_variables WHERE var = ?` for every removed variable.
    Delete(Vec<String>),
}

/// The global variables as this server knows them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GlobalVariables {
    /// Java `AbstractVariables`' map (a `ConcurrentHashMap`: no order).
    vars: HashMap<String, String>,
    changed: BTreeSet<String>,
    removed: BTreeSet<String>,
}

impl GlobalVariables {
    /// Java `hasVariable(name)`.
    pub fn has_variable(&self, name: &str) -> bool {
        self.vars.contains_key(name)
    }

    /// Java `getString(name, default)`.
    pub fn get_string(&self, name: &str, default: &str) -> String {
        self.vars.get(name).cloned().unwrap_or_else(|| default.to_owned())
    }

    /// Java `getInt(name, default)`: a value that is no number is the default (Java throws).
    pub fn get_int(&self, name: &str, default: i32) -> i32 {
        self.vars
            .get(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Java `getLong(name, default)`.
    pub fn get_long(&self, name: &str, default: i64) -> i64 {
        self.vars
            .get(name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Java `getBoolean(name, default)`: `Boolean.parseBoolean`, so anything but `true` (any
    /// case) is false.
    pub fn get_bool(&self, name: &str, default: bool) -> bool {
        self.vars
            .get(name)
            .map_or(default, |v| v.eq_ignore_ascii_case("true"))
    }

    /// Java `set(name, value)`.
    pub fn set(&mut self, name: &str, value: &str) {
        self.vars.insert(name.to_owned(), value.to_owned());
        self.changed.insert(name.to_owned());
        self.removed.remove(name);
    }

    /// Java `remove(name)`.
    pub fn remove(&mut self, name: &str) {
        if self.vars.remove(name).is_some() {
            self.changed.remove(name);
            self.removed.insert(name.to_owned());
        }
    }

    /// The writes of the changes since the load or the last store, and the change marks cleared.
    pub(crate) fn take_writes(&mut self) -> Vec<GlobalVariablesWrite> {
        let mut writes = Vec::new();
        let changed: Vec<(String, String)> = std::mem::take(&mut self.changed)
            .into_iter()
            .filter_map(|name| self.vars.get(&name).map(|v| (name, v.clone())))
            .collect();
        if !changed.is_empty() {
            writes.push(GlobalVariablesWrite::Replace(changed));
        }
        let removed: Vec<String> = std::mem::take(&mut self.removed).into_iter().collect();
        if !removed.is_empty() {
            writes.push(GlobalVariablesWrite::Delete(removed));
        }
        writes
    }

    /// The number of variables.
    pub fn len(&self) -> usize {
        self.vars.len()
    }

    pub fn is_empty(&self) -> bool {
        self.vars.is_empty()
    }
}

impl World {
    /// Java `GlobalVariablesManager.restoreMe()`: the rows read at startup. A NULL value is
    /// skipped (`StatSet.set(name, null)` ignores it).
    pub fn set_global_variables(&mut self, rows: Vec<(String, Option<String>)>) {
        let mut globals = GlobalVariables::default();
        for (var, value) in rows {
            if let Some(value) = value {
                globals.vars.insert(var, value);
            }
        }
        info!("GlobalVariablesManager: Loaded {} variables.", globals.len());
        self.scripts.globals = globals;
    }

    /// Java `GlobalVariablesManager.getInstance()`, read only.
    pub fn global_variables(&self) -> &GlobalVariables {
        &self.scripts.globals
    }

    /// Java `GlobalVariablesManager.storeMe()` (D-54: only with `CycleDatabaseWrites`).
    pub(crate) fn store_global_variables(&mut self) {
        if !self.scripts.settings.cycle_database_writes {
            if !self.scripts.globals.changed.is_empty() || !self.scripts.globals.removed.is_empty() {
                info!(
                    "GlobalVariablesManager: CycleDatabaseWrites is off, the script variables stay in memory (D-54)."
                );
            }
            return;
        }
        let writes = self.scripts.globals.take_writes();
        let count = self.scripts.globals.len();
        for write in writes {
            self.outbox.persist(PersistCommand::GlobalVariables(write));
        }
        info!("GlobalVariablesManager: Stored {count} variables.");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_read_like_statset() {
        let mut g = GlobalVariables::default();
        g.set("a", "12");
        g.set("b", "TRUE");
        g.set("c", "x");
        assert_eq!(g.get_int("a", 0), 12);
        assert_eq!(g.get_long("a", 0), 12);
        assert!(g.get_bool("b", false));
        assert!(!g.get_bool("c", true));
        assert_eq!(g.get_int("c", 7), 7);
        assert_eq!(g.get_string("missing", "d"), "d");
    }

    #[test]
    fn writes_name_the_changes_once() {
        let mut g = GlobalVariables::default();
        g.set("a", "1");
        g.set("b", "2");
        g.remove("b");
        g.remove("never");
        assert_eq!(
            g.take_writes(),
            [
                GlobalVariablesWrite::Replace(vec![("a".to_owned(), "1".to_owned())]),
                GlobalVariablesWrite::Delete(vec!["b".to_owned()]),
            ]
        );
        assert!(g.take_writes().is_empty());
    }
}
