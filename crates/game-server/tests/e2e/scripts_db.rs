//! The rows of the scripts tests: what Sieghardt (the Java game server) keeps in the shared
//! database before Kain starts (`Seed`), a model of the tables the persistence task reads for
//! the world (`Rows`, D-50), and the replay of the world's writes into a throwaway MariaDB.
//!
//! Included by `e2e_scripts.rs` (`#[path]`).

use std::collections::BTreeMap;

use l2_world::{
    BossReadKind, BossRereadRows, BossWrite, ClassIndexLoad, GrandBossData, PersistCommand, QuestLoad,
    QuestWrite, SkillData, SubclassWrite, World, WorldCommand,
};
use l2_world::boss::GrandBossStoreRow;
use l2_world::script::GlobalVariablesWrite;

/// The state row of a quest in `character_quests` (`var = '<state>'`, Java `Quest.STATE_VAR`).
pub const STATE_VAR: &str = "<state>";

/// A row Sieghardt wrote to `character_quests`.
#[derive(Clone, Debug)]
pub struct QuestRow {
    /// The name of the character the row belongs to.
    pub hero: &'static str,
    pub name: &'static str,
    pub var: &'static str,
    pub value: &'static str,
}

/// The rows of the Java game server (D-39), before Kain starts.
#[derive(Clone, Debug, Default)]
pub struct Seed {
    /// `grandboss_data`.
    pub grand_bosses: Vec<GrandBossData>,
    /// `global_variables`.
    pub globals: Vec<(&'static str, &'static str)>,
    /// `character_quests`.
    pub quests: Vec<QuestRow>,
}

impl Seed {
    pub fn boss_load(&self) -> l2_world::BossLoad {
        l2_world::BossLoad {
            grand_bosses: self.grand_bosses.clone(),
            ..l2_world::BossLoad::default()
        }
    }

    pub fn global_variables(&self) -> Vec<(String, Option<String>)> {
        self.globals
            .iter()
            .map(|(var, value)| ((*var).to_owned(), Some((*value).to_owned())))
            .collect()
    }

    /// The `character_quests` rows of `hero` as the session reads them at `CharacterSelect`.
    pub fn quests_of(&self, hero: &str) -> QuestLoad {
        let mut load = QuestLoad::default();
        for row in self.quests.iter().filter(|r| r.hero == hero) {
            if row.var == STATE_VAR {
                load.states.push((row.name.to_owned(), Some(row.value.to_owned())));
            } else {
                load.variables
                    .push((row.name.to_owned(), row.var.to_owned(), Some(row.value.to_owned())));
            }
        }
        load
    }
}

/// A grand boss row as Sieghardt would have written it.
pub fn grand_boss_row(boss_id: i32, status: i32, respawn_time: i64, at: (i32, i32, i32)) -> GrandBossData {
    GrandBossData {
        boss_id,
        loc_x: at.0,
        loc_y: at.1,
        loc_z: at.2,
        heading: 0,
        respawn_time,
        current_hp: 0.0,
        current_mp: 0.0,
        status,
    }
}

/// The tables as the world's writes leave them, and the answers to the world's reads.
pub struct Rows {
    /// `global_variables`.
    pub globals: BTreeMap<String, String>,
    /// `grandboss_data` by boss id.
    pub grand_bosses: BTreeMap<i32, GrandBossData>,
    /// `character_quests`: `(char id, quest name, var)` to the value.
    pub quests: BTreeMap<(i32, String, String), String>,
    /// `character_instance_time`: `(char id, instance id)` to the time.
    pub instance_times: BTreeMap<(i32, i32), i64>,
    /// `character_skills` by class index (what a switch reads back).
    skills: BTreeMap<(i32, i32), Vec<SkillData>>,
    /// `character_hennas` by class index: the dye ids of slots 1 to 3.
    hennas: BTreeMap<(i32, i32), [i32; 3]>,
}

impl Rows {
    pub fn new(seed: &Seed) -> Self {
        Self {
            globals: seed
                .globals
                .iter()
                .map(|(var, value)| ((*var).to_owned(), (*value).to_owned()))
                .collect(),
            grand_bosses: seed.grand_bosses.iter().map(|b| (b.boss_id, b.clone())).collect(),
            quests: BTreeMap::new(),
            instance_times: BTreeMap::new(),
            skills: BTreeMap::new(),
            hennas: BTreeMap::new(),
        }
    }

    /// The `character_quests` rows of the seed, for the character `char_id` called `hero`.
    pub fn seed_quests(&mut self, seed: &Seed, hero: &str, char_id: i32) {
        for row in seed.quests.iter().filter(|r| r.hero == hero) {
            self.quests.insert(
                (char_id, row.name.to_owned(), row.var.to_owned()),
                row.value.to_owned(),
            );
        }
    }

    /// The effect of a command on the tables (Java's statements).
    pub fn apply(&mut self, command: &PersistCommand) {
        match command {
            PersistCommand::Quest(write) => self.apply_quest(write),
            PersistCommand::GlobalVariables(GlobalVariablesWrite::Replace(rows)) => {
                for (var, value) in rows {
                    self.globals.insert(var.clone(), value.clone());
                }
            }
            PersistCommand::GlobalVariables(GlobalVariablesWrite::Delete(vars)) => {
                for var in vars {
                    self.globals.remove(var);
                }
            }
            PersistCommand::Boss(BossWrite::GrandBossStatus { boss_id, status }) => {
                if let Some(row) = self.grand_bosses.get_mut(boss_id) {
                    row.status = *status;
                }
            }
            PersistCommand::Boss(BossWrite::GrandBoss(row)) => {
                self.grand_bosses.insert(row.boss_id, row.clone());
            }
            PersistCommand::Boss(BossWrite::GrandBossStoreAll { rows, .. }) => {
                for row in rows {
                    match row {
                        GrandBossStoreRow::StatusOnly { boss_id, status } => {
                            if let Some(row) = self.grand_bosses.get_mut(boss_id) {
                                row.status = *status;
                            }
                        }
                        GrandBossStoreRow::Full(row) => {
                            self.grand_bosses.insert(row.boss_id, row.clone());
                        }
                    }
                }
            }
            PersistCommand::InstanceTime(l2_world::InstanceTimeWrite::Set {
                char_id,
                instance_id,
                time,
            }) => {
                self.instance_times.insert((*char_id, *instance_id), *time);
            }
            PersistCommand::InstanceTime(l2_world::InstanceTimeWrite::Delete { char_id, instance_id }) => {
                self.instance_times.remove(&(*char_id, *instance_id));
            }
            PersistCommand::StorePlayer(data) => {
                // `store`: the skills and the hennas of the class the character plays.
                let key = (data.object_id.0, data.profile.class_index);
                self.skills.insert(key, data.skills.clone());
                self.hennas.insert(key, data.profile.hennas);
            }
            PersistCommand::Subclass(SubclassWrite::Add { char_id, holder, skills }) => {
                self.skills.insert((*char_id, holder.class_index), skills.clone());
                self.hennas.insert((*char_id, holder.class_index), [0; 3]);
            }
            PersistCommand::Subclass(SubclassWrite::Delete {
                char_id, class_index, ..
            }) => {
                self.skills.remove(&(*char_id, *class_index));
                self.hennas.remove(&(*char_id, *class_index));
            }
            _ => {}
        }
    }

    fn apply_quest(&mut self, write: &QuestWrite) {
        match write {
            QuestWrite::InsertVar {
                char_id,
                name,
                var,
                value,
            }
            | QuestWrite::UpdateVar {
                char_id,
                name,
                var,
                value,
            } => {
                self.quests
                    .insert((*char_id, name.clone(), var.clone()), value.clone());
            }
            QuestWrite::DeleteVar { char_id, name, var } => {
                self.quests.remove(&(*char_id, name.clone(), var.clone()));
            }
            QuestWrite::DeleteQuest {
                char_id,
                name,
                repeatable,
            } => {
                // `deleteQuestInDb(qs, repeatable)`: every row when repeatable, else every row but the state row.
                self.quests
                    .retain(|(c, n, v), _| !(c == char_id && n == name && (*repeatable || v != STATE_VAR)));
            }
        }
    }

    /// Whether `character_skills` has the skill for the class index.
    pub fn has_skill(&self, char_id: i32, class_index: i32, skill_id: i32) -> bool {
        self.skills
            .get(&(char_id, class_index))
            .is_some_and(|skills| skills.iter().any(|s| s.skill_id == skill_id))
    }

    /// The value of a `character_quests` row.
    pub fn quest_var(&self, char_id: i32, name: &str, var: &str) -> Option<&str> {
        self.quests
            .get(&(char_id, name.to_owned(), var.to_owned()))
            .map(String::as_str)
    }

    /// What the persistence task answers to a command that asks a question (D-50).
    pub fn answer(&self, world: &mut World, command: &PersistCommand) {
        match command {
            PersistCommand::GlobalVariablesRereadRequest { token, names } => {
                let rows = names
                    .iter()
                    .filter_map(|n| self.globals.get(n).map(|v| (n.clone(), Some(v.clone()))))
                    .collect();
                world.handle(WorldCommand::GlobalVariablesReread {
                    token: *token,
                    result: Ok(rows),
                });
            }
            PersistCommand::BossRereadRequest { kind, then } => {
                let rows = match kind {
                    BossReadKind::GrandBoss(boss_id) => BossRereadRows::GrandBoss {
                        boss_id: *boss_id,
                        row: self.grand_bosses.get(boss_id).cloned(),
                    },
                    BossReadKind::RaidPoints(char_id) => BossRereadRows::RaidPoints {
                        char_id: *char_id,
                        rows: Vec::new(),
                    },
                };
                world.handle(WorldCommand::BossReread {
                    result: Ok(rows),
                    then: *then,
                });
            }
            PersistCommand::CrestIdsRequest { ticket } => {
                world.handle(WorldCommand::CrestIdsLoaded {
                    ticket: *ticket,
                    result: Ok(Vec::new()),
                });
            }
            PersistCommand::ClassIndexRequest { owner, class_index } => {
                let key = (owner.0, *class_index);
                let load = ClassIndexLoad {
                    skills: self.skills.get(&key).cloned().unwrap_or_default(),
                    hennas: self.hennas.get(&key).copied().unwrap_or([0; 3]),
                    ..ClassIndexLoad::default()
                };
                world.handle(WorldCommand::ClassIndexLoaded {
                    owner: *owner,
                    class_index: *class_index,
                    load: Box::new(load),
                });
            }
            _ => {}
        }
    }
}
