//! The war at server start (Java `TerritoryWarManager.load`, the `TerritoryWarSuperClass`
//! constructor's `setNextTWDate` and `MercenaryCaptain`'s shouting timers) and the rows read
//! again side by side (D-50, D-51).

use tracing::{info, warn};

use super::{Registration, Territory, TerritoryNpcSpawn, TerritoryThen, WARD_PLACES};
use crate::world::World;

/// One `territory_spawnlist` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerritorySpawnRecord {
    pub id: i32,
    pub castle_id: i32,
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// 0 town NPCs, 1 fortress NPCs, 2 castle NPCs, 3 a ward place.
    pub spawn_type: i32,
}

/// One `territories` row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TerritoryRecord {
    pub territory_id: i32,
    pub castle_id: i32,
    pub fort_id: i32,
    /// `ownedWardIds`: `"81;85;"`.
    pub owned_ward_ids: String,
}

/// One `territory_registrations` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerritoryRegistrationRecord {
    pub castle_id: i32,
    /// A clan id, or a character id when no clan has it.
    pub registered_id: i32,
}

/// What the game server read for the war at startup (D-15: before the world runs; only reads).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerritoryLoad {
    /// `SELECT * FROM territory_spawnlist`, in row order. `None`: the read failed (Java logs
    /// "SpawnList error" and goes on without any territory).
    pub spawns: Option<Vec<TerritorySpawnRecord>>,
    /// `SELECT * FROM territories`. `None`: the read failed.
    pub territories: Option<Vec<TerritoryRecord>>,
    /// `SELECT * FROM territory_registrations`. `None`: the read failed.
    pub registrations: Option<Vec<TerritoryRegistrationRecord>>,
    /// The `nextTWStartDate` global variable (0 without a row, like `getLong(.., 0)`).
    pub next_tw_start_date: i64,
}

/// The war's rows read again (side by side, D-50/D-51).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct TerritoryRows {
    pub territories: Vec<TerritoryRecord>,
    pub registrations: Vec<TerritoryRegistrationRecord>,
    pub next_tw_start_date: i64,
}

/// Java `Integer.parseInt` of every `;`-separated part of `ownedWardIds` (`split(";")` drops the
/// trailing empty part). `Err` where Java throws.
fn parse_ward_ids(text: &str) -> Result<Vec<i32>, String> {
    if text.is_empty() {
        return Ok(Vec::new());
    }
    let mut parts: Vec<&str> = text.split(';').collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
        .into_iter()
        .map(|p| p.parse::<i32>().map_err(|_| format!("For input string: \"{p}\"")))
        .collect()
}

impl World {
    /// Java's Territory War at server start: `TerritoryWarManager.load()` (the spawn list, the
    /// territories with their owners, town NPCs and wards, the registrations), then the
    /// `TerritoryWarSuperClass` constructor (`setNextTWDate`) and `MercenaryCaptain`'s hourly
    /// shout of every captain that stands. Call after [`Self::load_residences`] and the clans.
    pub fn load_territories(&mut self, load: TerritoryLoad) {
        self.territory.next_tw_start_date = load.next_tw_start_date;
        // The spawn list.
        match load.spawns {
            Some(rows) => {
                for row in rows {
                    let territory = self
                        .territory
                        .territories
                        .entry(row.castle_id)
                        .or_insert_with(|| Territory::new(row.castle_id));
                    let spawn = TerritoryNpcSpawn {
                        castle_id: row.castle_id,
                        x: row.x,
                        y: row.y,
                        z: row.z,
                        heading: row.heading,
                        npc_id: row.npc_id,
                        spawn_type: row.spawn_type,
                        npc: None,
                    };
                    match row.spawn_type {
                        // Town, fortress and castle NPCs.
                        0..=2 => territory.spawn_list.push(spawn),
                        // Ward spawns (`addWardSpawnPlace`: a tenth place is dropped).
                        3 => {
                            if territory.ward_places.len() < WARD_PLACES {
                                territory.ward_places.push(TerritoryNpcSpawn {
                                    npc_id: 0,
                                    spawn_type: 4,
                                    ..spawn
                                });
                            }
                        }
                        _ => warn!("TerritoryWarManager: Unknown npc type for {}", row.id),
                    }
                }
            }
            None => warn!("TerritoryWarManager: SpawnList error"),
        }
        // The territories.
        match load.territories {
            Some(rows) => {
                if let Err(e) = self.territory_load_rows(&rows) {
                    warn!("TerritoryWarManager: territory list error(): {e}");
                }
            }
            None => warn!("TerritoryWarManager: territory list error()"),
        }
        // The registrations.
        match load.registrations {
            Some(rows) => {
                for row in rows {
                    self.territory_add_registration_row(row.castle_id, row.registered_id, false);
                }
            }
            None => warn!("TerritoryWarManager: registration list error"),
        }
        // TerritoryWarSuperClass (questId < 0): "Calculate next TW date."
        self.tw_set_next_tw_date();
        info!(
            "TerritoryWarSuperClass: Siege date: {} (epoch ms)",
            self.territory.start_tw_date
        );
        // MercenaryCaptain: `startQuestTimer("say", DELAY, spawn.getNpc(), null, true)` for the
        // captains of the owned territories.
        self.territory_start_captain_timers();
    }

    /// The `territories` loop of `load()`. `Err` where Java throws (a castle that does not exist,
    /// a ward id that is not a number), which ends the loop.
    fn territory_load_rows(&mut self, rows: &[super::TerritoryRecord]) -> Result<(), String> {
        for row in rows {
            if !self.territory.territories.contains_key(&row.castle_id) {
                continue;
            }
            if let Some(t) = self.territory.territories.get_mut(&row.castle_id) {
                t.fort_id = row.fort_id;
            }
            let Some(owner) = self
                .castle(crate::residence::CastleId(row.castle_id))
                .map(|c| c.owner_id)
            else {
                return Err(format!("NullPointerException: no castle {}", row.castle_id));
            };
            if owner > 0 {
                // `ClanTable.getClan(ownerId)`: null for a clan the table does not know.
                let clan = self.clan(owner).map(|c| c.id);
                if let Some(t) = self.territory.territories.get_mut(&row.castle_id) {
                    t.owner_clan = clan;
                }
                self.territory_change_npcs_spawn(row.castle_id, 0, true);
            }
            for ward in parse_ward_ids(&row.owned_ward_ids)? {
                if ward > 0 {
                    self.territory_add_ward(ward, row.castle_id, 0, false);
                }
            }
        }
        Ok(())
    }

    /// One registration row: a clan the table knows is a clan registration, anything else a
    /// mercenary.
    fn territory_add_registration_row(&mut self, castle_id: i32, registered_id: i32, unsaved: bool) {
        let entry = Registration {
            id: registered_id,
            unsaved,
        };
        if self.clan(registered_id).is_some() {
            self.territory
                .registered_clans
                .entry(castle_id)
                .or_default()
                .push(entry);
        } else {
            self.territory
                .registered_mercenaries
                .entry(castle_id)
                .or_default()
                .push(entry);
        }
    }

    // ------------------------------------------------------------------ read again (D-50)

    /// Asks the persistence task for the war's rows; the answer comes back as
    /// `WorldCommand::ResidenceReread` and goes on with `then`.
    pub(crate) fn territory_request_reread(&mut self, then: TerritoryThen) {
        self.outbox
            .persist(crate::outbox::PersistCommand::ResidenceRereadRequest {
                residence_id: 0,
                kind: crate::residence::ResidenceReadKind::Territory,
                then: crate::residence::ResidenceReadThen::Territory(then),
            });
    }

    /// The war's rows as the database has them now: the registrations (this world's unsaved ones
    /// stay), the wards each territory owns and the next war date. Nothing changes while the war
    /// runs in this world.
    pub(crate) fn apply_territory_rows(&mut self, rows: &TerritoryRows) {
        if self.territory.in_progress {
            return;
        }
        self.territory.next_tw_start_date = rows.next_tw_start_date;
        // The registrations.
        let keep = |list: &std::collections::BTreeMap<i32, Vec<Registration>>| -> Vec<(i32, i32)> {
            list.iter()
                .flat_map(|(castle, regs)| regs.iter().filter(|r| r.unsaved).map(move |r| (*castle, r.id)))
                .filter(|(castle, id)| {
                    !rows
                        .registrations
                        .iter()
                        .any(|r| r.castle_id == *castle && r.registered_id == *id)
                })
                .collect()
        };
        let mut unsaved = keep(&self.territory.registered_clans);
        unsaved.extend(keep(&self.territory.registered_mercenaries));
        self.territory.registered_clans.clear();
        self.territory.registered_mercenaries.clear();
        for row in &rows.registrations {
            self.territory_add_registration_row(row.castle_id, row.registered_id, false);
        }
        for (castle, id) in unsaved {
            self.territory_add_registration_row(castle, id, true);
        }
        // The wards.
        for row in &rows.territories {
            let Ok(wards) = parse_ward_ids(&row.owned_ward_ids) else {
                warn!(
                    "Territory {}: bad ownedWardIds \"{}\"",
                    row.territory_id, row.owned_ward_ids
                );
                continue;
            };
            let wards: Vec<i32> = wards.into_iter().filter(|w| *w > 0).collect();
            self.territory_apply_owned_wards(row.castle_id, &wards);
        }
    }

    /// The answer of [`Self::territory_request_reread`].
    pub(crate) fn on_territory_reread(&mut self, then: TerritoryThen) {
        match then {
            TerritoryThen::Join {
                player,
                territory_id,
                is_clan,
                is_joining,
            } => self.territory_join_after_reread(player, territory_id, is_clan, is_joining),
            TerritoryThen::RestartSchedule => {
                self.tw_set_next_tw_date();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ward_ids_are_split_like_java() {
        assert_eq!(parse_ward_ids(""), Ok(vec![]));
        assert_eq!(parse_ward_ids("81;"), Ok(vec![81]));
        assert_eq!(parse_ward_ids("81;85;"), Ok(vec![81, 85]));
        assert_eq!(parse_ward_ids("81;85"), Ok(vec![81, 85]));
        // An empty part in the middle is `Integer.parseInt("")`: Java throws.
        assert!(parse_ward_ids("81;;85;").is_err());
        assert!(parse_ward_ids("x;").is_err());
    }
}
