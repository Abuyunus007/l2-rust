//! Residence functions: the paid services of a castle, fortress or clan hall (teleport, HP / MP /
//! exp regeneration, support magic, item creation, decorations) and their fee task.
// Java: mechanics/siege/Castle.java (CastleFunction, FunctionTask, loadFunctions, removeFunction,
//       updateFunctions)
// Java: mechanics/siege/Fort.java (FortFunction, the same)
// Java: entity/residences/ClanHall.java (ClanHallFunction, the same)
//
// The three Java classes are the same code with other constants and tables, so one
// [`ResidenceFunction`] serves all of them.
//
// # Function objects and their tasks
// In Java a function object owns its fee task (`FunctionTask`, an inner class), and nothing ever
// cancels such a task. A function that `updateFunctions` replaces (a higher lease) or removes
// keeps its task: it goes on charging the clan warehouse and writing its row (`dbSave` is a
// `REPLACE`), and when the warehouse is short it removes *the current* function of its type.
// NOTE(java-quirk): this is ported as is. The objects live in `ResidenceWorld::function_objects`
// by key; a residence's map (`Castle::functions`, ...) holds the keys of its current functions,
// and a timer names the object it belongs to. An object goes when neither the map nor a pending
// task holds it.
//
// # Side by side (D-51)
// A fee the clan warehouse pays is a scheduled job of both servers: without
// `SiegeDatabaseWrites` the task only moves the paid time on (no charge, no row, no removal); the
// Java game server charges the real warehouse. The first run after a player bought a function
// (`cwh` false, nothing is charged) writes its row like any player action.

use l2_data::item::ADENA_ID;
use tracing::{error, warn};

use super::{ResidenceRef, ResidenceTimer, ResidenceWrite, Writes};
use crate::outbox::{ItemRowOp, PersistCommand};
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::{World, unix_ms_now};

/// `Castle.FUNC_TELEPORT` (also `Fort.FUNC_TELEPORT`).
pub const CASTLE_FUNC_TELEPORT: i32 = 1;
/// `Castle.FUNC_RESTORE_HP`.
pub const CASTLE_FUNC_RESTORE_HP: i32 = 2;
/// `Castle.FUNC_RESTORE_MP`.
pub const CASTLE_FUNC_RESTORE_MP: i32 = 3;
/// `Castle.FUNC_RESTORE_EXP`.
pub const CASTLE_FUNC_RESTORE_EXP: i32 = 4;
/// `Castle.FUNC_SUPPORT`.
pub const CASTLE_FUNC_SUPPORT: i32 = 5;

/// `ClanHall.FUNC_TELEPORT`.
pub const HALL_FUNC_TELEPORT: i32 = 1;
/// `ClanHall.FUNC_ITEM_CREATE`.
pub const HALL_FUNC_ITEM_CREATE: i32 = 2;
/// `ClanHall.FUNC_RESTORE_HP`.
pub const HALL_FUNC_RESTORE_HP: i32 = 3;
/// `ClanHall.FUNC_RESTORE_MP`.
pub const HALL_FUNC_RESTORE_MP: i32 = 4;
/// `ClanHall.FUNC_RESTORE_EXP`.
pub const HALL_FUNC_RESTORE_EXP: i32 = 5;
/// `ClanHall.FUNC_SUPPORT`.
pub const HALL_FUNC_SUPPORT: i32 = 6;
/// `ClanHall.FUNC_DECO_FRONTPLATEFORM` ("Only Auctionable Halls").
pub const HALL_FUNC_DECO_FRONTPLATEFORM: i32 = 7;
/// `ClanHall.FUNC_DECO_CURTAINS` ("Only Auctionable Halls").
pub const HALL_FUNC_DECO_CURTAINS: i32 = 8;

/// One function (Java `CastleFunction`, `FortFunction`, `ClanHallFunction`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResidenceFunction {
    /// `_type`.
    pub function_type: i32,
    /// `_lvl` / `_level`.
    pub lvl: i32,
    /// `_fee` (the `lease` column).
    pub lease: i32,
    /// `_tempFee`: what the first run charges after a lease increase (`_endDate == -1`); every
    /// Java caller passes 0.
    pub temp_lease: i32,
    /// `_rate`: milliseconds between two fees.
    pub rate: i64,
    /// `_endDate`: when the paid time ends (epoch milliseconds; -1 right after a lease increase).
    pub end_time: i64,
    /// `_cwh`: the next fee comes from the clan warehouse (the first one after a purchase does not;
    /// the player paid it).
    pub cwh: bool,
}

/// A function object with its fee task (see the module comment).
#[derive(Debug, Clone)]
pub(crate) struct FunctionTask {
    pub residence: ResidenceRef,
    pub function: ResidenceFunction,
    /// The pending `FunctionTask`, `None` once it ran without scheduling another one.
    pub timer: Option<TimerId>,
}

impl World {
    // ------------------------------------------------------------------ lookups

    /// The function keys of a residence (Java `_function` / `_functions`, by type).
    fn function_keys(&self, residence: ResidenceRef) -> Option<&std::collections::BTreeMap<i32, u64>> {
        match residence {
            ResidenceRef::Castle(id) => self.castle(id).map(|c| &c.functions),
            ResidenceRef::Fort(id) => self.fort(id).map(|f| &f.functions),
            ResidenceRef::ClanHall(id) => self.clan_hall(id).map(|h| &h.functions),
        }
    }

    fn function_keys_mut(
        &mut self,
        residence: ResidenceRef,
    ) -> Option<&mut std::collections::BTreeMap<i32, u64>> {
        match residence {
            ResidenceRef::Castle(id) => self.castle_mut(id).map(|c| &mut c.functions),
            ResidenceRef::Fort(id) => self.fort_mut(id).map(|f| &mut f.functions),
            ResidenceRef::ClanHall(id) => self.clan_hall_mut(id).map(|h| &mut h.functions),
        }
    }

    /// Java `getFunction(type)`.
    pub fn residence_function(
        &self,
        residence: ResidenceRef,
        function_type: i32,
    ) -> Option<&ResidenceFunction> {
        let key = self.function_keys(residence)?.get(&function_type)?;
        self.residences.function_objects.get(key).map(|t| &t.function)
    }

    /// Every current function of a residence, by type.
    pub fn residence_functions(&self, residence: ResidenceRef) -> Vec<&ResidenceFunction> {
        self.function_keys(residence)
            .map(|keys| {
                keys.values()
                    .filter_map(|key| self.residences.function_objects.get(key))
                    .map(|t| &t.function)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Whether a residence's function tasks run (Java: `_ownerId > 0` of a castle, `_fortOwner !=
    /// null`, `!_isFree` of a clan hall).
    fn functions_active(&self, residence: ResidenceRef) -> bool {
        match residence {
            ResidenceRef::Castle(id) => self.castle(id).is_some_and(|c| c.owner_id > 0),
            ResidenceRef::Fort(id) => self.fort(id).is_some_and(|f| f.owner_id != 0),
            ResidenceRef::ClanHall(id) => self.clan_hall(id).is_some_and(|h| !h.is_free),
        }
    }

    /// The clan that pays a residence's fees (`ClanTable.getClan(getOwnerId())` / `_fortOwner`).
    fn function_payer(&self, residence: ResidenceRef) -> Option<i32> {
        let owner = self.residence_owner_id(residence);
        self.clan(owner).map(|c| c.id)
    }

    // ------------------------------------------------------------------ objects and tasks

    /// Java `new CastleFunction(type, lvl, lease, tempLease, rate, time, cwh)`: the object and,
    /// for an owned residence, its first fee task (`initializeTask`). Returns the object's key;
    /// [`Self::put_function`] makes it the residence's current function of its type.
    pub(crate) fn new_function(
        &mut self,
        residence: ResidenceRef,
        function: ResidenceFunction,
        cwh: bool,
    ) -> u64 {
        self.residences.next_function_task += 1;
        let key = self.residences.next_function_task;
        let end_time = function.end_time;
        self.residences.function_objects.insert(
            key,
            FunctionTask {
                residence,
                function,
                timer: None,
            },
        );
        // `initializeTask(cwh)`.
        if self.functions_active(residence) {
            let now = unix_ms_now();
            let delay = if end_time > now { end_time - now } else { 0 };
            self.schedule_function_fee(key, delay, cwh);
        }
        key
    }

    /// `ThreadPool.schedule(new FunctionTask(cwh), delay)`: the constructor sets the object's
    /// `_cwh`.
    fn schedule_function_fee(&mut self, key: u64, delay_ms: i64, cwh: bool) {
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Residence(ResidenceTimer::FunctionFee(key)));
        if let Some(task) = self.residences.function_objects.get_mut(&key) {
            task.function.cwh = cwh;
            if let Some(old) = task.timer.replace(timer) {
                self.timers.cancel(old);
            }
        }
    }

    /// `_function.put(type, function)`: the object becomes the residence's current function of
    /// its type. The object it replaces lives on while its task does.
    pub(crate) fn put_function(&mut self, residence: ResidenceRef, key: u64) {
        let Some(function_type) = self
            .residences
            .function_objects
            .get(&key)
            .map(|t| t.function.function_type)
        else {
            return;
        };
        let old = self
            .function_keys_mut(residence)
            .and_then(|keys| keys.insert(function_type, key));
        if let Some(old) = old.filter(|old| *old != key) {
            self.forget_function_object(old);
        }
    }

    /// Drops a function object that neither a residence map nor a pending task holds.
    fn forget_function_object(&mut self, key: u64) {
        let Some(task) = self.residences.function_objects.get(&key) else {
            return;
        };
        if task.timer.is_some() {
            return;
        }
        let residence = task.residence;
        let current = self
            .function_keys(residence)
            .is_some_and(|keys| keys.values().any(|k| *k == key));
        if !current {
            self.residences.function_objects.remove(&key);
        }
    }

    /// Java `loadFunctions()`: the rows of an owned residence become function objects (`cwh` true:
    /// their next fee comes from the clan warehouse).
    pub(crate) fn load_residence_functions(
        &mut self,
        residence: ResidenceRef,
        rows: &[super::FunctionRecord],
    ) {
        for row in rows {
            let function = ResidenceFunction {
                function_type: row.function_type,
                lvl: row.lvl,
                lease: row.lease,
                temp_lease: 0,
                rate: row.rate,
                end_time: row.end_time,
                cwh: true,
            };
            let key = self.new_function(residence, function, true);
            self.put_function(residence, key);
        }
    }

    /// Drops every current function of a residence from this world without a row (a re-read
    /// replaces them). Their tasks stop.
    pub(crate) fn clear_residence_functions(&mut self, residence: ResidenceRef) {
        let keys: Vec<u64> = self
            .function_keys_mut(residence)
            .map(|keys| std::mem::take(keys).into_values().collect())
            .unwrap_or_default();
        for key in keys {
            if let Some(timer) = self
                .residences
                .function_objects
                .get_mut(&key)
                .and_then(|t| t.timer.take())
            {
                self.timers.cancel(timer);
            }
            self.residences.function_objects.remove(&key);
        }
    }

    /// Java `removeFunction(functionType)`: out of the map and its row deleted as `writes` says
    /// (an owner change or an unpaid fee is a [`Writes::Job`], a player's own removal a
    /// [`Writes::Player`]).
    pub fn residence_remove_function(&mut self, residence: ResidenceRef, function_type: i32, writes: Writes) {
        let old = self
            .function_keys_mut(residence)
            .and_then(|keys| keys.remove(&function_type));
        if let Some(old) = old {
            self.forget_function_object(old);
        }
        let write = ResidenceWrite::FunctionDelete {
            residence,
            function_type,
        };
        self.write_residence(writes, write);
    }

    /// Java `for (Integer fc : _function.keySet()) removeFunction(fc); _function.clear();` of an
    /// owner change.
    pub(crate) fn residence_remove_all_functions(&mut self, residence: ResidenceRef) {
        self.residence_remove_all_functions_with(residence, Writes::Job);
    }

    /// [`Self::residence_remove_all_functions`] with the rows written as `writes` says.
    pub(crate) fn residence_remove_all_functions_with(&mut self, residence: ResidenceRef, writes: Writes) {
        let types: Vec<i32> = self
            .function_keys(residence)
            .map(|keys| keys.keys().copied().collect())
            .unwrap_or_default();
        for function_type in types {
            self.residence_remove_function(residence, function_type, writes);
        }
    }

    /// Java `CastleFunction.dbSave()` of a function object.
    fn function_db_save(&mut self, key: u64) {
        let Some(task) = self.residences.function_objects.get(&key) else {
            return;
        };
        let f = &task.function;
        let write = ResidenceWrite::FunctionSave {
            residence: task.residence,
            function_type: f.function_type,
            lvl: f.lvl,
            lease: f.lease,
            rate: f.rate,
            end_time: f.end_time,
        };
        self.residence_write(write);
    }

    /// Java `FunctionTask.run()`.
    pub(crate) fn on_function_fee(&mut self, key: u64) {
        let Some(task) = self.residences.function_objects.get_mut(&key) else {
            return;
        };
        task.timer = None;
        let residence = task.residence;
        let function = task.function.clone();
        if !self.functions_active(residence) {
            self.forget_function_object(key);
            return;
        }
        let now = unix_ms_now();
        if function.cwh && !self.residences.settings.siege_database_writes {
            // D-51: the Java game server charges the real clan warehouse and writes the row; this
            // world only moves the paid time on.
            if let Some(task) = self.residences.function_objects.get_mut(&key) {
                task.function.end_time = now.saturating_add(function.rate);
            }
            self.schedule_function_fee(key, function.rate, true);
            return;
        }
        let Some(payer) = self.function_payer(residence) else {
            // `ClanTable.getClan(getOwnerId()).getWarehouse()` throws: logged, the task is over.
            error!(
                "{residence}: the fee of function {} has no clan to pay it.",
                function.function_type
            );
            self.forget_function_object(key);
            return;
        };
        let adena = self.clan_warehouse_adena(payer);
        if adena >= i64::from(function.lease) || !function.cwh {
            let fee = if function.end_time == -1 {
                function.temp_lease
            } else {
                function.lease
            };
            if let Some(task) = self.residences.function_objects.get_mut(&key) {
                task.function.end_time = now.saturating_add(function.rate);
            }
            self.function_db_save(key);
            if function.cwh {
                self.clan_warehouse_destroy_adena(payer, i64::from(fee));
            }
            self.schedule_function_fee(key, function.rate, true);
        } else {
            self.residence_remove_function(residence, function.function_type, Writes::Job);
            self.forget_function_object(key);
        }
    }

    /// Java `ClanWarehouse.getAdena()`: the count of the first adena item.
    pub(crate) fn clan_warehouse_adena(&self, clan_id: i32) -> i64 {
        self.clan(clan_id)
            .and_then(|c| c.warehouse.iter().find(|i| i.item_id == ADENA_ID))
            .map_or(0, |i| i.count)
    }

    /// Java `ClanWarehouse.destroyItemByItemId(FEE, ADENA_ID, count, null, null)`: the first
    /// adena item of the clan warehouse loses `count`, or goes when that is all of it. `false`
    /// when it has less (Java returns null and the caller ignores it).
    // NOTE: Java writes a partly used adena row only with the item's next `updateDatabase`; here
    // the row is written at once, because this server reads the clan warehouse again at every
    // window (D-50) and would otherwise give the fee back.
    pub(crate) fn clan_warehouse_destroy_adena(&mut self, clan_id: i32, count: i64) -> bool {
        let Some(clan) = self.clan_mut(clan_id) else {
            return false;
        };
        let Some(index) = clan.warehouse.iter().position(|i| i.item_id == ADENA_ID) else {
            return false;
        };
        let have = clan.warehouse[index].count;
        let op = if have > count {
            let item = &mut clan.warehouse[index];
            item.count -= count;
            ItemRowOp::Update {
                owner_id: clan_id,
                item: item.clone(),
            }
        } else if have == count {
            let item = clan.warehouse.remove(index);
            self.release_item_id(item.object_id);
            ItemRowOp::Delete {
                object_id: item.object_id,
            }
        } else {
            return false;
        };
        self.outbox.persist(PersistCommand::ItemRows {
            tracked: None,
            ops: vec![op],
        });
        true
    }

    // ------------------------------------------------------------------ updateFunctions

    /// Java `updateFunctions(player, type, lvl, lease, rate, addNew)` of a castle, fortress or clan
    /// hall: the chamberlain or hall manager buys, changes or stops a function for the player's
    /// adena. `false` when the player cannot pay (Java's message: "You do not have enough adena").
    #[allow(clippy::too_many_arguments)] // Java's `updateFunctions` signature.
    pub fn residence_update_functions(
        &mut self,
        player: l2_util::ObjectId,
        residence: ResidenceRef,
        function_type: i32,
        lvl: i32,
        lease: i32,
        rate: i64,
        add_new: bool,
    ) -> bool {
        if !self.players.contains_key(&player) {
            return false;
        }
        if lease > 0 && !self.destroy_item_by_item_id(player, ADENA_ID, i64::from(lease), true) {
            return false;
        }
        let function = |end_time: i64| ResidenceFunction {
            function_type,
            lvl,
            lease,
            temp_lease: 0,
            rate,
            end_time,
            cwh: false,
        };
        if add_new {
            let key = self.new_function(residence, function(0), false);
            self.put_function(residence, key);
        } else if lvl == 0 && lease == 0 {
            self.residence_remove_function(residence, function_type, Writes::Player);
        } else {
            let Some(key) = self
                .function_keys(residence)
                .and_then(|keys| keys.get(&function_type))
                .copied()
            else {
                // NOTE(java-quirk): `_function.get(type).getLease()` throws for a type the
                // residence does not have; the adena is gone and nothing else happens.
                warn!("{residence}: updateFunctions of function {function_type} that does not exist.");
                return false;
            };
            let old_lease = self
                .residences
                .function_objects
                .get(&key)
                .map_or(0, |t| t.function.lease);
            if lease - old_lease > 0 {
                // The old object keeps its task (see the module comment).
                let key = self.new_function(residence, function(-1), false);
                self.put_function(residence, key);
            } else {
                if let Some(task) = self.residences.function_objects.get_mut(&key) {
                    task.function.lease = lease;
                    task.function.lvl = lvl;
                }
                self.function_db_save(key);
            }
        }
        true
    }
}
