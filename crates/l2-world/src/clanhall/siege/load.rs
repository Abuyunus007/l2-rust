//! Loading the hall sieges at startup (the engine of each hall with its script) and the rows the
//! sieges write and read again.
// Java: mechanics/siege/clanhalls/ClanHallSiegeEngine.java (the constructor, loadAttackers,
//       loadGuards)
// Java: mechanics/siege/clanhalls/SiegableHall.java (the constructor: `_scheduleConfig`)
// Java: dist/game/data/scripts/conquerablehalls/* (the constructors and `loadAttackers`)

use l2_data::residence::siege_zone_of;
use tracing::info;

use super::{
    HallScript, HallSiege, ScriptState, SiegeStatus, devastated, flag_war, rainbow, resistance, schedule,
};
use crate::residence::{ClanHallId, ClanHallKind, SiegableHallRecord};
use crate::siege::{SiegeClan, SiegeClanType};
use crate::world::World;

/// A `clanhall_siege_guards` row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HallGuardRecord {
    pub npc_id: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// Seconds.
    pub respawn_delay: i32,
}

/// A registered clan of a flag war: its `siegable_hall_flagwar_attackers` row and its members.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FlagWarRecord {
    pub clan_id: i32,
    pub flag: i32,
    pub npc: i32,
    /// `siegable_hall_flagwar_attackers_members`: the object ids of the fighters.
    pub members: Vec<i32>,
}

/// What the game server read for one hall.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HallSiegeHallLoad {
    pub hall_id: i32,
    /// `clanhall_siege_attackers`.
    pub attackers: Vec<i32>,
    /// `clanhall_siege_guards`.
    pub guards: Vec<HallGuardRecord>,
    /// The flag war tables (Bandit Stronghold, Wild Beast Reserve).
    pub flag_war: Vec<FlagWarRecord>,
    /// `rainbowsprings_attacker_list` (Rainbow Springs Chateau): clan id and war decrees.
    pub rainbow: Vec<(i32, i64)>,
}

/// Everything the game server read for the hall sieges at startup (D-15: only reads, D-39).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct HallSiegeLoad {
    pub halls: Vec<HallSiegeHallLoad>,
}

/// The rows of a hall read again on demand (`ResidenceReadKind::HallSiege`, D-50).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HallSiegeRows {
    pub hall_id: i32,
    /// The `siegable_clanhall` row with its functions, `None` when it is not there.
    pub hall: Option<SiegableHallRecord>,
    pub flag_war: Vec<FlagWarRecord>,
    pub rainbow: Vec<(i32, i64)>,
}

/// A hall siege row write with Java's statement (inside `ResidenceWrite::HallSiege`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HallSiegeWrite {
    /// `ClanHallSiegeEngine.saveAttackers`: `DELETE FROM clanhall_siege_attackers WHERE
    /// clanhall_id = ?`, then `INSERT INTO clanhall_siege_attackers VALUES (?,?)` per clan.
    SaveAttackers { hall_id: i32, clans: Vec<i32> },
    /// `saveClan`: `INSERT INTO siegable_hall_flagwar_attackers VALUES(hall, flag, 0, clan)`.
    FlagWarClan { hall_id: i32, flag: i32, clan_id: i32 },
    /// `saveNpc`: `UPDATE siegable_hall_flagwar_attackers SET npc = ? WHERE clan_id = ?`.
    FlagWarNpc { npc: i32, clan_id: i32 },
    /// `saveMember`: `INSERT INTO siegable_hall_flagwar_attackers_members VALUES (?,?,?)`.
    FlagWarMember {
        hall_id: i32,
        clan_id: i32,
        object_id: i32,
    },
    /// `clearTables`: both flag war tables of the hall.
    FlagWarClear { hall_id: i32 },
    /// `updateAttacker`: `INSERT INTO rainbowsprings_attacker_list VALUES (?,?)`.
    RainbowAdd { clan_id: i32, count: i64 },
    /// `updateAttacker` (remove): `DELETE FROM rainbowsprings_attacker_list WHERE clanId = ?`.
    RainbowRemove { clan_id: i32 },
    /// `clearTables`: `DELETE FROM rainbowsprings_attacker_list`.
    RainbowClear,
}

impl World {
    /// Java's scripts at startup, after the residences: for each conquerable hall that has a
    /// script, the engine (`ClanHallSiegeEngine`'s constructor and the script's own). Call after
    /// [`World::load_residences`], the NPC data and the zones.
    pub fn load_hall_sieges(&mut self, load: HallSiegeLoad) {
        if self.clanhall.siege.settings.no_quests {
            info!("CHSiegeManager: NoQuests is on, the clan hall sieges are not loaded.");
            return;
        }
        for script in HallScript::ALL {
            let id = ClanHallId(script.hall_id());
            if !self.clan_hall(id).is_some_and(|h| h.is_siegable_hall()) {
                continue;
            }
            let rows = load
                .halls
                .iter()
                .find(|h| h.hall_id == id.0)
                .cloned()
                .unwrap_or_default();
            self.new_hall_siege(script, id, rows);
        }
        info!(
            "CHSiegeManager: {} clan hall sieges.",
            self.clanhall.siege.halls.len()
        );
    }

    /// The constructor of a script (`ClanHallSiegeEngine(hallId)` and the subclass).
    fn new_hall_siege(&mut self, script: HallScript, id: ClanHallId, rows: HallSiegeHallLoad) {
        let (name, config) = match self.clan_hall(id) {
            Some(hall) => {
                let raw = match &hall.kind {
                    ClanHallKind::Siegable { schedule_config, .. } => schedule_config.clone(),
                    ClanHallKind::Auctionable { .. } => None,
                };
                (
                    hall.name.clone(),
                    schedule::parse_schedule_config(&hall.name, raw.as_deref()),
                )
            }
            None => return,
        };
        let siege_zone = self
            .zoning
            .zones
            .as_ref()
            .and_then(|z| siege_zone_of(z, id.0))
            .map(|z| z.id);
        // `loadAttackers()` and the script's own state.
        let mut attackers: Vec<i32> = Vec::new();
        let state = match script {
            HallScript::FortressOfResistance => {
                attackers = rows.attackers.clone();
                let nurka_spawn = self.resistance_new_nurka_spawn();
                ScriptState::Resistance(resistance::Resistance {
                    nurka_spawn,
                    damage: Vec::new(),
                    messenger_page: None,
                })
            }
            HallScript::DevastatedCastle => {
                attackers = rows.attackers.clone();
                ScriptState::Devastated(devastated::Devastated::default())
            }
            HallScript::FortressOfTheDead => {
                attackers = rows.attackers.clone();
                ScriptState::Dead(super::dead::Dead::default())
            }
            HallScript::BanditStronghold | HallScript::WildBeastReserve => {
                let (state, clans) = flag_war::new_state(self, script, &rows.flag_war);
                attackers = clans;
                ScriptState::FlagWar(state)
            }
            HallScript::RainbowSpringsChateau => {
                ScriptState::Rainbow(Box::new(rainbow::new_state(self, &rows.rainbow)))
            }
        };
        let mut siege = HallSiege {
            script,
            status: SiegeStatus::Registering,
            schedule_config: config,
            attackers: Vec::new(),
            attackers_peak: 0,
            task: None,
            guard_rows: rows.guards,
            guards: None,
            mission_accomplished: false,
            siege_zone,
            state,
        };
        // The base engine's `SiegeClan`s; a clan that is gone is skipped by the scripts' loaders.
        for clan_id in attackers {
            siege
                .attackers
                .push(SiegeClan::new(clan_id, SiegeClanType::Attacker));
        }
        siege.attackers_peak = siege.attackers.len();
        self.clanhall.siege.halls.insert(id, siege);
        self.hall_link_siege_zone(id);
        self.hall_arm_prepare_owner(id);
        info!("{name} siege scheduled for {}.", self.hall_next_siege_time(id));
        if script == HallScript::FortressOfResistance {
            self.resistance_build_messenger(id);
        }
    }
}
