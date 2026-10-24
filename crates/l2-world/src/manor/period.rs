//! The manor periods: the mode at start, the daily change on the timer wheel and what each change
//! does (`CastleManorManager` constructor, `scheduleModeChange`, `changeMode`).
// Java: managers/CastleManorManager.java

use std::sync::Arc;

use l2_data::Seeds;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::{info, warn};

use super::{CropProcure, ManorLoad, ManorMode, ManorSettings, ManorTimer, SeedProduction};
use crate::outbox::{ItemRowOp, PersistCommand};
use crate::residence::{CastleId, Writes};
use crate::timer::{TimerKind, TimerWheel};
use crate::warehouse::ContainerRef;
use crate::world::{World, unix_ms_now};

const SECOND_MS: i64 = 1000;
const MINUTE_MS: i64 = 60 * SECOND_MS;
const HOUR_MS: i64 = 60 * MINUTE_MS;
const DAY_MS: i64 = 24 * HOUR_MS;

/// The mode the manor starts in at this hour and minute (local time): the first lines of the
/// `CastleManorManager` constructor.
// NOTE(java-quirk): `(hour >= REFRESH_TIME) && (min >= maintenanceMin)` tests the minute of the
// hour, not the time of the day: from 21:00 to 21:05 the manor starts APPROVED, not MODIFIABLE.
pub fn manor_initial_mode(settings: &ManorSettings, hour: i32, min: i32) -> ManorMode {
    let maintenance_min = settings.refresh_min + settings.maintenance_min;
    if (hour >= settings.refresh_time && min >= maintenance_min)
        || hour < settings.approve_time
        || (hour == settings.approve_time && min <= settings.approve_min)
    {
        ManorMode::Modifiable
    } else if hour == settings.refresh_time && min >= settings.refresh_min && min < maintenance_min {
        ManorMode::Maintenance
    } else {
        ManorMode::Approved
    }
}

/// `Calendar.set(SECOND, 0)` on "now", then the hour and minute set: the wall clock time
/// `hour:min:00` of today with the milliseconds of now, as UTC milliseconds. `offset_at(utc_ms)`
/// is the zone's offset from UTC (D-48).
// NOTE(java-quirk): `Calendar.set(SECOND, 0)` leaves the millisecond field alone.
fn today_at(now_ms: i64, offset_at: &impl Fn(i64) -> i64, hour: i32, min: i32) -> (i64, i64) {
    let local_now = now_ms + offset_at(now_ms);
    let day_start = local_now - local_now.rem_euclid(DAY_MS);
    let millis = local_now.rem_euclid(SECOND_MS);
    let local = day_start + i64::from(hour) * HOUR_MS + i64::from(min) * MINUTE_MS + millis;
    (local, day_start)
}

fn local_to_utc(local: i64, now_ms: i64, offset_at: &impl Fn(i64) -> i64) -> i64 {
    let guess = local - offset_at(now_ms);
    local - offset_at(guess)
}

/// When the next mode change is due for `mode`: Java's `scheduleModeChange` before the one-second
/// floor. `None` for a disabled manor.
pub fn manor_next_mode_change_ms(
    mode: ManorMode,
    settings: &ManorSettings,
    now_ms: i64,
    offset_at: impl Fn(i64) -> i64,
) -> Option<i64> {
    let (hour, min) = match mode {
        ManorMode::Modifiable => (settings.approve_time, settings.approve_min),
        ManorMode::Maintenance => (
            settings.refresh_time,
            settings.refresh_min + settings.maintenance_min,
        ),
        ManorMode::Approved => (settings.refresh_time, settings.refresh_min),
        ManorMode::Disabled => return None,
    };
    let (local, _) = today_at(now_ms, &offset_at, hour, min);
    let today = local_to_utc(local, now_ms, &offset_at);
    // "If the time has already passed today, schedule for tomorrow."
    Some(if today < now_ms {
        local_to_utc(local + DAY_MS, now_ms, &offset_at)
    } else {
        today
    })
}

/// Java `DATE_FORMAT` (`dd/MM HH:mm:ss`) of a local wall clock time in epoch milliseconds.
fn format_local(local_ms: i64) -> String {
    let days = local_ms.div_euclid(DAY_MS);
    let ms = local_ms.rem_euclid(DAY_MS);
    // Days since 1970-01-01 to a civil date (Howard Hinnant's algorithm).
    let z = days + 719_468;
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    format!(
        "{day:02}/{month:02} {:02}:{:02}:{:02}",
        ms / HOUR_MS,
        (ms % HOUR_MS) / MINUTE_MS,
        (ms % MINUTE_MS) / SECOND_MS
    )
}

impl World {
    /// Installs the manor configuration. Call before [`Self::load_manor`].
    pub fn set_manor_settings(&mut self, settings: ManorSettings) {
        self.manor.settings = settings;
    }

    /// Installs `data/Seeds.xml`. Call before [`Self::load_manor`].
    pub fn set_manor_seeds(&mut self, seeds: Arc<Seeds>) {
        self.manor.seeds = seeds;
    }

    /// The manor configuration.
    pub fn manor_settings(&self) -> &ManorSettings {
        &self.manor.settings
    }

    /// The seeds (Java `CastleManorManager._seeds`).
    pub fn manor_seeds(&self) -> &Arc<Seeds> {
        &self.manor.seeds
    }

    /// Java `isUnderMaintenance()`.
    pub fn manor_is_under_maintenance(&self) -> bool {
        self.manor.mode == ManorMode::Maintenance
    }

    /// Java `isManorApproved()`.
    pub fn manor_is_approved(&self) -> bool {
        self.manor.mode == ManorMode::Approved
    }

    /// Java `isModifiablePeriod()`.
    pub fn manor_is_modifiable(&self) -> bool {
        self.manor.mode == ManorMode::Modifiable
    }

    /// Java `getCurrentModeName()`.
    pub fn manor_current_mode_name(&self) -> &'static str {
        self.manor.mode.name()
    }

    /// Java `getNextModeChange()`: `dd/MM HH:mm:ss` in the server's zone, or "Disabled".
    pub fn manor_next_mode_change_text(&self) -> String {
        self.manor_next_mode_change_text_at(l2_util::time::local_offset_millis)
    }

    /// [`Self::manor_next_mode_change_text`] for a given zone (`offset_at` as in
    /// `next_daily_reset_ms`).
    pub fn manor_next_mode_change_text_at(&self, offset_at: impl Fn(i64) -> i64) -> String {
        match self.manor.next_mode_change {
            None => "Disabled".to_owned(),
            Some(at) => format_local(at + offset_at(at)),
        }
    }

    /// Java `CastleManorManager`'s constructor, the part after `Seeds.xml`: the rows of every
    /// castle, the mode for the time of day, the change on the wheel and the automatic save.
    /// Call once at startup, after the residences are loaded. A disabled manor loads nothing.
    pub fn load_manor(&mut self, load: ManorLoad) {
        self.load_manor_at(load, unix_ms_now(), l2_util::time::local_offset_millis);
    }

    /// [`Self::load_manor`] for a given clock (`offset_at` as in `next_daily_reset_ms`).
    pub fn load_manor_at(&mut self, load: ManorLoad, now_ms: i64, offset_at: impl Fn(i64) -> i64 + Copy) {
        if !self.manor.settings.allow_manor {
            self.manor.mode = ManorMode::Disabled;
            self.manor.next_mode_change = None;
            info!("CastleManorManager: Manor system is deactivated.");
            return;
        }
        self.manor_load_db(load);

        // Set the initial manor mode based on the current time.
        let local = now_ms + offset_at(now_ms);
        let ms_of_day = local.rem_euclid(DAY_MS);
        let hour = i32::try_from(ms_of_day / HOUR_MS).unwrap_or(0);
        let min = i32::try_from((ms_of_day % HOUR_MS) / MINUTE_MS).unwrap_or(0);
        self.manor.mode = manor_initial_mode(&self.manor.settings, hour, min);

        // Schedule the mode change task.
        self.manor_schedule_mode_change(now_ms, offset_at);

        // Schedule auto-save if not saving all actions.
        if !self.manor.settings.save_all_actions {
            self.manor_schedule_auto_save();
        }
    }

    /// Java `loadDb`: for every castle the seeds it sells and the crops it buys, both periods.
    /// A row of a seed or crop `Seeds.xml` does not know is skipped with Java's warning.
    fn manor_load_db(&mut self, load: ManorLoad) {
        let castle_ids: Vec<i32> = self.castles().iter().map(|c| c.id.0).collect();
        let crop_ids = self.manor.seeds.crop_ids();
        let manor = &mut self.manor;
        for castle_id in castle_ids {
            let mut current: Vec<SeedProduction> = Vec::new();
            let mut next: Vec<SeedProduction> = Vec::new();
            for row in load.production.iter().filter(|r| r.castle_id == castle_id) {
                if manor.seeds.get(row.seed_id).is_none() {
                    warn!("CastleManorManager: Unknown seed ID: {}", row.seed_id);
                    continue;
                }
                let seed = SeedProduction {
                    id: row.seed_id,
                    amount: row.amount,
                    price: row.price,
                    start_amount: row.start_amount,
                };
                if row.next_period {
                    next.push(seed);
                } else {
                    current.push(seed);
                }
            }
            manor.production.insert(castle_id, current);
            manor.production_next.insert(castle_id, next);

            let mut current: Vec<CropProcure> = Vec::new();
            let mut next: Vec<CropProcure> = Vec::new();
            for row in load.procure.iter().filter(|r| r.castle_id == castle_id) {
                if !crop_ids.contains(&row.crop_id) {
                    warn!("CastleManorManager: Unknown crop ID: {}", row.crop_id);
                    continue;
                }
                let crop = CropProcure {
                    id: row.crop_id,
                    amount: row.amount,
                    reward: row.reward_type,
                    start_amount: row.start_amount,
                    price: row.price,
                };
                if row.next_period {
                    next.push(crop);
                } else {
                    current.push(crop);
                }
            }
            manor.procure.insert(castle_id, current);
            manor.procure_next.insert(castle_id, next);
        }
        info!("CastleManorManager: Manor data loaded.");
    }

    /// Java `scheduleModeChange()`: the next change for the current mode, at least a second away.
    fn manor_schedule_mode_change(&mut self, now_ms: i64, offset_at: impl Fn(i64) -> i64) {
        let Some(mut at) =
            manor_next_mode_change_ms(self.manor.mode, &self.manor.settings, now_ms, offset_at)
        else {
            // A disabled manor does not schedule.
            return;
        };
        // Ensure delay is not negative (minimum 1 second).
        let mut delay = at - now_ms;
        if delay < SECOND_MS {
            delay = SECOND_MS;
            at = now_ms + delay;
        }
        self.manor.next_mode_change = Some(at);
        let due = self.tick + TimerWheel::ms_to_ticks(u64::try_from(delay).unwrap_or(0));
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Manor(ManorTimer::ModeChange));
        if let Some(old) = self.manor.mode_timer.replace(timer) {
            self.timers.cancel(old);
        }
    }

    /// `ThreadPool.scheduleAtFixedRate(this::storeMe, rate * 3600 * 1000, rate * 3600 * 1000)`.
    fn manor_schedule_auto_save(&mut self) {
        // NOTE: Java's executor refuses a period of 0 or less (the server would not start); one
        // hour is the shortest period here.
        let hours = u64::try_from(self.manor.settings.save_period_rate)
            .unwrap_or(1)
            .max(1);
        let due = self.tick + TimerWheel::ms_to_ticks(hours * 3_600_000);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Manor(ManorTimer::AutoSave));
        if let Some(old) = self.manor.save_timer.replace(timer) {
            self.timers.cancel(old);
        }
    }

    /// The manor timers.
    pub(crate) fn on_manor_timer(&mut self, timer: ManorTimer) {
        match timer {
            ManorTimer::ModeChange => {
                self.manor.mode_timer = None;
                self.manor_change_mode_at(unix_ms_now(), l2_util::time::local_offset_millis);
            }
            ManorTimer::AutoSave => {
                self.manor.save_timer = None;
                self.manor_store_me();
                self.manor_schedule_auto_save();
            }
        }
    }

    /// Java `changeMode()`: the manor goes on to its next mode (APPROVED to MAINTENANCE at 20:00,
    /// MAINTENANCE to MODIFIABLE six minutes later, MODIFIABLE to APPROVED at 4:30) and schedules
    /// the change after it. `now_ms` and `offset_at` are the clock of the schedule.
    pub fn manor_change_mode_at(&mut self, now_ms: i64, offset_at: impl Fn(i64) -> i64) {
        match self.manor.mode {
            ManorMode::Approved => {
                // Transition to maintenance mode.
                self.manor.mode = ManorMode::Maintenance;
                self.manor_update_periods();
                // Save the updated manor data.
                self.manor_store_me();
            }
            ManorMode::Maintenance => {
                // Notify clan leaders of the manor mode change.
                for (_, owner) in self.manor_castle_owners() {
                    self.manor_tell_leader(owner, SystemMessageId::TheManorInformationHasBeenUpdated);
                }
                // Transition to modifiable mode.
                self.manor.mode = ManorMode::Modifiable;
            }
            ManorMode::Modifiable => {
                // Transition to approved mode.
                self.manor.mode = ManorMode::Approved;
                self.manor_validate_next_periods();
                // Store changes if configured to save all actions.
                if self.manor.settings.save_all_actions {
                    self.manor_store_me();
                }
            }
            ManorMode::Disabled => return,
        }
        // Schedule the next mode change.
        self.manor_schedule_mode_change(now_ms, offset_at);
    }

    /// The castles that have an owner clan this world knows: `(castle id, clan id)`.
    fn manor_castle_owners(&self) -> Vec<(i32, i32)> {
        self.castles()
            .iter()
            .filter(|c| self.clan(c.owner_id).is_some())
            .map(|c| (c.id.0, c.owner_id))
            .collect()
    }

    /// `clanLeader != null && clanLeader.isOnline()` then `sendPacket(message)`.
    fn manor_tell_leader(&self, clan_id: i32, message: SystemMessageId) {
        let leader = self.clan(clan_id).and_then(|c| c.leader);
        if let Some(leader) = leader.filter(|l| self.players.contains_key(l)) {
            self.send(leader, &SystemMessage::new(message));
        }
    }

    /// The `APPROVED` branch of `changeMode`: the crops the castles bought go to the clan
    /// warehouses, what they did not spend goes back to the treasuries, the next period becomes
    /// the current one, and it is emptied when the treasury cannot pay for it.
    fn manor_update_periods(&mut self) {
        for (castle_id, owner) in self.manor_castle_owners() {
            // Process crop procurement and treasury updates.
            let crops = self.manor.procure.get(&castle_id).cloned().unwrap_or_default();
            for crop in crops {
                if crop.start_amount <= 0 {
                    continue;
                }
                // Adding bought crops to clan warehouse.
                if crop.start_amount != crop.amount {
                    // `(long) ((startAmount - amount) * 0.9)`
                    let mut harvested = ((crop.start_amount - crop.amount) as f64 * 0.9) as i64;
                    if harvested < 1 && self.rng().below(99) < 90 {
                        harvested = 1;
                    }
                    // Add harvested crops to the clan warehouse.
                    if harvested > 0 {
                        match self.manor.seeds.by_crop(crop.id).map(|s| s.mature_id) {
                            Some(mature_id) => self.manor_reward_clan_warehouse(owner, mature_id, harvested),
                            None => warn!("CastleManorManager: no seed for the crop {}", crop.id),
                        }
                    }
                }
                // Add unspent amount to castle treasury.
                if crop.amount > 0 {
                    self.castle_add_to_treasury_no_tax(
                        CastleId(castle_id),
                        crop.amount.wrapping_mul(crop.price),
                        Writes::Job,
                    );
                }
            }

            // Update current production and procurements to the next period's data.
            let next_production = self
                .manor
                .production_next
                .get(&castle_id)
                .cloned()
                .unwrap_or_default();
            let next_procure = self
                .manor
                .procure_next
                .get(&castle_id)
                .cloned()
                .unwrap_or_default();
            self.manor.production.insert(castle_id, next_production);
            self.manor.procure.insert(castle_id, next_procure);

            // Prepare data for the upcoming period, depending on treasury funds.
            let treasury = self.castle(CastleId(castle_id)).map_or(0, |c| c.treasury);
            if treasury < self.manor_cost(castle_id, false) {
                self.manor.production_next.insert(castle_id, Vec::new());
                self.manor.procure_next.insert(castle_id, Vec::new());
            } else {
                // Reset production and procurement amounts for the next period. In Java the
                // current period holds the very same objects, so both lists are reset.
                for list in [
                    self.manor.production_next.get_mut(&castle_id),
                    self.manor.production.get_mut(&castle_id),
                ]
                .into_iter()
                .flatten()
                {
                    for seed in list {
                        seed.amount = seed.start_amount;
                    }
                }
                for list in [
                    self.manor.procure_next.get_mut(&castle_id),
                    self.manor.procure.get_mut(&castle_id),
                ]
                .into_iter()
                .flatten()
                {
                    for crop in list {
                        crop.amount = crop.start_amount;
                    }
                }
            }
        }
    }

    /// The `MODIFIABLE` branch of `changeMode`: the lord's next period is checked against the
    /// funds and the clan warehouse, then paid for.
    fn manor_validate_next_periods(&mut self) {
        for (castle_id, owner) in self.manor_castle_owners() {
            // Calculate the necessary slots in the warehouse for new crops.
            // NOTE(java-quirk): Java counts a slot when `cwh.getAllItemsByItemId(matureId) ==
            // null`, but that method returns an empty collection, never null: no slot is ever
            // required.
            let required_slots: i64 = 0;
            let manor_cost = self.manor_cost(castle_id, true);
            let treasury = self.castle(CastleId(castle_id)).map_or(0, |c| c.treasury);

            // Check if there's enough capacity and funds.
            // NOTE(java-quirk): `&&`, not `||`: the next period is only cleared when the clan
            // warehouse is over its limit and the treasury is short.
            if !self.container_validate_capacity(ContainerRef::Clan(owner), required_slots)
                && treasury < manor_cost
            {
                // Clear next period data if insufficient resources.
                if let Some(list) = self.manor.production_next.get_mut(&castle_id) {
                    list.clear();
                }
                if let Some(list) = self.manor.procure_next.get_mut(&castle_id) {
                    list.clear();
                }
                // Notify clan leader.
                self.manor_tell_leader(
                    owner,
                    SystemMessageId::YouDoNotHaveEnoughFundsInTheClanWarehouseForTheManorToOperate,
                );
            } else {
                // Deduct manor cost from treasury.
                self.castle_add_to_treasury_no_tax(
                    CastleId(castle_id),
                    manor_cost.wrapping_neg(),
                    Writes::Job,
                );
            }
        }
    }

    /// Java `ClanWarehouse.addItem(REWARD, itemId, count, null, null)`: the harvested crops join
    /// the clan warehouse. Only with `SiegeDatabaseWrites` (D-49, D-51): the clan warehouse is read
    /// from the database at every window (M6-SYNC-01) and the Java game server hands the same crops
    /// out at the same hour, so a copy in memory would only be lost at the next window.
    fn manor_reward_clan_warehouse(&mut self, clan_id: i32, item_id: i32, count: i64) {
        if !self.siege_database_writes() {
            tracing::debug!(
                "SideBySide: {count} of item {item_id} not added to the warehouse of clan {clan_id} (SiegeDatabaseWrites = False)"
            );
            return;
        }
        let Some(templates) = self.item_templates() else {
            return;
        };
        let Some(template) = templates.get(item_id) else {
            warn!("Invalid ItemId ({item_id}) requested by REWARD");
            return;
        };
        let container = ContainerRef::Clan(clan_id);
        let mut ops: Vec<ItemRowOp> = Vec::new();
        let existing = self.container_item_by_item_id(container, item_id).cloned();
        match existing {
            // If stackable item is found in inventory just add to current quantity.
            Some(old) if template.stackable => {
                let Some(items) = self.container_items_mut(container) else {
                    return;
                };
                if let Some(item) = items.iter_mut().find(|i| i.object_id == old.object_id) {
                    item.count = item.count.saturating_add(count);
                    ops.push(ItemRowOp::Update {
                        owner_id: clan_id,
                        item: item.clone(),
                    });
                }
            }
            // If item hasn't be found in inventory, create new one.
            _ => {
                let multiple = self.settings.ground.multiple_item_drop;
                let mut i: i64 = 0;
                while i < count {
                    let Some(id) = self.allocate_item_id() else {
                        break;
                    };
                    let mut data =
                        self.new_item_data(template, id, if template.stackable { count } else { 1 });
                    data.loc = container.base_location().to_string();
                    data.loc_data = container.loc_data();
                    data.enchant_level = template.default_enchant_level;
                    let Some(items) = self.container_items_mut(container) else {
                        break;
                    };
                    items.push(data.clone());
                    ops.push(ItemRowOp::Insert {
                        owner_id: clan_id,
                        item: data,
                    });
                    i += 1;
                    // If stackable, end loop as entire count is included in 1 instance of item.
                    if template.stackable || !multiple {
                        break;
                    }
                }
            }
        }
        if !ops.is_empty() {
            self.outbox
                .persist(PersistCommand::ItemRows { tracked: None, ops });
        }
    }

    /// Whether the mode change is on the wheel (tests).
    pub fn manor_has_mode_timer(&self) -> bool {
        self.manor.mode_timer.is_some()
    }

    /// The seeded state of the monster is gone with it (`Attackable.onSpawn` clears it).
    pub(crate) fn manor_forget_monster(&mut self, id: ObjectId) {
        self.manor.monsters.remove(&id);
    }

    /// Java `Shutdown`: "Save all manor data." (`storeMe`, only without `AltManorSaveAllActions`).
    // NOTE(java-quirk): Java stores even when the manor is disabled, which empties both tables.
    pub(crate) fn manor_shutdown(&mut self) {
        if !self.manor.settings.save_all_actions {
            self.manor_store_me();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn defaults() -> ManorSettings {
        ManorSettings::default()
    }

    /// The offset of the zone for the tests: UTC.
    fn utc(_ms: i64) -> i64 {
        0
    }

    fn at(hour: i64, min: i64) -> i64 {
        // 2026-01-05 (a Monday) hour:min:00.250
        1_767_571_200_000 + hour * HOUR_MS + min * MINUTE_MS + 250
    }

    #[test]
    fn the_mode_at_start_follows_javas_conditions() {
        let s = defaults();
        // Before 4:30 and at 4:30 sharp the lords may still set the period.
        assert_eq!(manor_initial_mode(&s, 3, 59), ManorMode::Modifiable);
        assert_eq!(manor_initial_mode(&s, 4, 30), ManorMode::Modifiable);
        assert_eq!(manor_initial_mode(&s, 4, 31), ManorMode::Approved);
        assert_eq!(manor_initial_mode(&s, 12, 0), ManorMode::Approved);
        assert_eq!(manor_initial_mode(&s, 19, 59), ManorMode::Approved);
        // 20:00 to 20:05 is the maintenance.
        assert_eq!(manor_initial_mode(&s, 20, 0), ManorMode::Maintenance);
        assert_eq!(manor_initial_mode(&s, 20, 5), ManorMode::Maintenance);
        assert_eq!(manor_initial_mode(&s, 20, 6), ManorMode::Modifiable);
        assert_eq!(manor_initial_mode(&s, 23, 30), ManorMode::Modifiable);
        // The quirk: the minute is compared on its own.
        assert_eq!(manor_initial_mode(&s, 21, 3), ManorMode::Approved);
    }

    #[test]
    fn the_next_change_is_the_next_time_of_the_mode() {
        let s = defaults();
        // Approved at noon: 20:00 today (seconds 0, the milliseconds of now are kept).
        assert_eq!(
            manor_next_mode_change_ms(ManorMode::Approved, &s, at(12, 0), utc),
            Some(at(20, 0))
        );
        // Maintenance at 20:01: 20:06.
        assert_eq!(
            manor_next_mode_change_ms(ManorMode::Maintenance, &s, at(20, 1), utc),
            Some(at(20, 6))
        );
        // Modifiable at 22:00: 4:30 tomorrow.
        assert_eq!(
            manor_next_mode_change_ms(ManorMode::Modifiable, &s, at(22, 0), utc),
            Some(at(4, 30) + DAY_MS)
        );
        // Modifiable at 3:00: 4:30 today.
        assert_eq!(
            manor_next_mode_change_ms(ManorMode::Modifiable, &s, at(3, 0), utc),
            Some(at(4, 30))
        );
        assert_eq!(
            manor_next_mode_change_ms(ManorMode::Disabled, &s, at(3, 0), utc),
            None
        );
    }

    #[test]
    fn a_zone_offset_moves_the_wall_clock() {
        let s = defaults();
        // UTC+2: noon UTC is 14:00 local, so the 20:00 local change is at 18:00 UTC.
        let plus_two = |_ms: i64| 2 * HOUR_MS;
        assert_eq!(
            manor_next_mode_change_ms(ManorMode::Approved, &s, at(12, 0), plus_two),
            Some(at(18, 0))
        );
    }

    #[test]
    fn the_next_change_text_is_javas_date_format() {
        // 2026-01-05 20:06:00 local.
        assert_eq!(format_local(at(20, 6) - 250), "05/01 20:06:00");
        // The leap day.
        assert_eq!(format_local(1_709_209_200_000), "29/02 12:20:00");
    }
}
