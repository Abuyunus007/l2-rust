//! `LongTimeEvent`: the dates, event drops, spawns and messages of a seasonal event.
// Java: org/l2jmobius/gameserver/mechanics/script/LongTimeEvent.java
//
// The reference of the seasonal event scripts: copy `the_valentine_event.rs` (the smallest one).
//
// Java's `LongTimeEvent extends Script` reads `data/scripts/events/<name>/config.xml` in its
// constructor. Scripts are compiled in (D-16), so the shipped `config.xml` of each event is an
// [`EventConfig`] constant next to its script (same values, same date strings; the dates are parsed
// at start-up exactly as Java parses them). An event script
//
// 1. calls `long_time_event::load(ctx, &CONFIG)` from its `on_load` (Java: the `super()` call of
//    the constructor, which starts the event when now is inside the dates);
// 2. gives `on_event` a first line `if long_time_event::on_event(ctx, &CONFIG, event, npc) { return None; }`
//    (Java: `ScheduleStart` / `ScheduleEnd` are `Runnable`s, here quest timers on the wheel); give it
//    the `npc` of the call too;
// 3. asks `long_time_event::is_event_period(ctx)` where Java calls `isEventPeriod()`.
//
// Several events run side by side: the state is kept per script name in the shared state.

use std::collections::BTreeMap;

use l2_world::rewards::event_drop::EventDrop;
use l2_world::{ScriptCtx, ScriptNpc};
use tracing::{info, warn};

/// One `<add item=".." min=".." max=".." chance="..%" .../>` of the `<droplist>` (Java builds an
/// `EventDropHolder` from it). The chance is the percent without the `%` sign; Java reads `0` for
/// a chance that does not end with `%`.
#[derive(Debug, Clone, Copy)]
pub struct DropSpec {
    pub item: i32,
    pub min: i64,
    pub max: i64,
    pub chance: f64,
    pub min_level: i32,
    pub max_level: i32,
    pub monster_ids: &'static [i32],
}

/// One `<add npc=".." x y z heading respawnTime/>` of the `<spawnlist>`.
#[derive(Debug, Clone, Copy)]
pub struct SpawnSpec {
    pub npc: i32,
    pub x: i32,
    pub y: i32,
    pub z: i32,
    pub heading: i32,
    /// `respawnTime * 1000`, as Java stores it.
    pub respawn_time_ms: i32,
}

impl SpawnSpec {
    /// The attributes as the XML has them (`respawn_time` in seconds, 0 without the attribute).
    pub const fn at(npc: i32, x: i32, y: i32, z: i32, heading: i32, respawn_time: i32) -> Self {
        Self {
            npc,
            x,
            y,
            z,
            heading,
            respawn_time_ms: respawn_time * 1000,
        }
    }
}

/// The `config.xml` of an event.
#[derive(Debug, Clone, Copy)]
pub struct EventConfig {
    /// `<event name="..">`: the name in the log lines.
    pub name: &'static str,
    /// `active="dd MM yyyy-dd MM yyyy"` (21 characters) or `"dd MM-dd MM"` (11, this year).
    pub active: &'static str,
    /// `dropPeriod`, the same formats; the active period when the attribute is absent or empty.
    pub drop_period: Option<&'static str>,
    pub drops: &'static [DropSpec],
    pub spawns: &'static [SpawnSpec],
    /// `<messages><add type="onEnter" text=".."/>`; empty when there is none.
    pub on_enter_msg: &'static str,
    /// `<add type="onEnd" ..>`.
    pub on_end_msg: &'static str,
    /// `<destroyItemsOnEnd><item id=".."/>`.
    pub destroy_items_on_end: &'static [i32],
}

/// Quest timer names; Java's `ScheduleStart` / `ScheduleEnd` / `spawn::stopRespawn` runnables.
const SCHEDULE_START: &str = "LongTimeEvent_ScheduleStart";
const SCHEDULE_END: &str = "LongTimeEvent_ScheduleEnd";
const STOP_RESPAWN: &str = "LongTimeEvent_StopRespawn";

const DAY_MS: i64 = 86_400_000;

/// What Java keeps in the `LongTimeEvent` fields of one event.
#[derive(Debug, Clone, Default)]
struct EventState {
    start_date: Option<i64>,
    end_date: Option<i64>,
    drop_start_date: Option<i64>,
    drop_end_date: Option<i64>,
    initialized: bool,
    active: bool,
    enter_announce_id: Option<i32>,
}

/// The states of every event script by script name (Java: one object per script).
#[derive(Default)]
struct Events(BTreeMap<&'static str, EventState>);

fn state_of(ctx: &mut ScriptCtx<'_>) -> EventState {
    let name = ctx.script_name();
    ctx.shared_state::<Events>()
        .0
        .get(name)
        .cloned()
        .unwrap_or_default()
}

fn store(ctx: &mut ScriptCtx<'_>, state: EventState) {
    let name = ctx.script_name();
    ctx.shared_state::<Events>().0.insert(name, state);
}

// The clock the tests of the event scripts set (the shipped dates are all in the past), per test
// thread. Java reads `new Date()` / `System.currentTimeMillis()`.
#[cfg(test)]
thread_local! {
    pub(crate) static TEST_CLOCK: std::cell::Cell<Option<i64>> = const { std::cell::Cell::new(None) };
}

/// Java `System.currentTimeMillis()`.
fn now(ctx: &ScriptCtx<'_>) -> i64 {
    #[cfg(test)]
    if let Some(ms) = TEST_CLOCK.get() {
        return ms;
    }
    ctx.now_ms()
}

/// Java `isEventPeriod()`: the event has started and not ended.
pub fn is_event_period(ctx: &mut ScriptCtx<'_>) -> bool {
    state_of(ctx).active
}

/// Java `getStartDate()` in milliseconds.
pub fn start_date(ctx: &mut ScriptCtx<'_>) -> Option<i64> {
    state_of(ctx).start_date
}

/// Java `getEndDate()` in milliseconds.
pub fn end_date(ctx: &mut ScriptCtx<'_>) -> Option<i64> {
    state_of(ctx).end_date
}

// ------------------------------------------------------------------------------ dates

/// Days since 1970-01-01 of a civil date (Hinnant).
fn days_from_civil(year: i64, month: i64, day: i64) -> i64 {
    let y = if month <= 2 { year - 1 } else { year };
    let era = y.div_euclid(400);
    let yoe = y.rem_euclid(400);
    let mp = if month > 2 { month - 3 } else { month + 9 };
    let doy = (153 * mp + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    era * 146_097 + doe - 719_468
}

/// The year of a day count since 1970-01-01.
fn year_from_days(days: i64) -> i64 {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    yoe + era * 400 + i64::from(month <= 2)
}

fn is_leap(year: i64) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

/// Java `LocalDate.parse(text, DateTimeFormatter.ofPattern("dd MM yyyy", Locale.US))
/// .atStartOfDay(ZoneId.systemDefault())`, in milliseconds. `None` for text that does not parse.
///
/// The pattern needs two digits for the day and the month and at least four for the year; the
/// default (smart) resolver moves a day past the end of a short month to the last day of it.
fn parse_date(text: &str, offset_at: &impl Fn(i64) -> i64) -> Option<i64> {
    let mut parts = text.split(' ');
    let (day, month, year) = (parts.next()?, parts.next()?, parts.next()?);
    if parts.next().is_some() {
        return None;
    }
    let digits = |s: &str| !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit());
    if day.len() != 2 || month.len() != 2 || year.len() < 4 || !digits(day) || !digits(month) || !digits(year)
    {
        return None;
    }
    let (day, month, year): (i64, i64, i64) = (day.parse().ok()?, month.parse().ok()?, year.parse().ok()?);
    if !(1..=12).contains(&month) || !(1..=31).contains(&day) {
        return None;
    }
    let month_length = match month {
        4 | 6 | 9 | 11 => 30,
        2 if is_leap(year) => 29,
        2 => 28,
        _ => 31,
    };
    let day = day.min(month_length);
    let local = days_from_civil(year, month, day) * DAY_MS;
    let guess = local - offset_at(local);
    Some(local - offset_at(guess))
}

/// Java `parseDateRange(dateRange, format)`: `"a-b"` with two dates, `None` (and a warning) else.
fn parse_date_range(range: &str, offset_at: &impl Fn(i64) -> i64) -> Option<(i64, i64)> {
    // `String.split("-")` drops trailing empty strings.
    let mut dates: Vec<&str> = range.split('-').collect();
    while dates.len() > 1 && dates.last().is_some_and(|d| d.is_empty()) {
        dates.pop();
    }
    if dates.len() == 2 {
        let start = parse_date(dates[0].trim(), offset_at);
        let end = parse_date(dates[1].trim(), offset_at);
        match (start, end) {
            (Some(start), Some(end)) => return Some((start, end)),
            _ => warn!("Invalid Date Format: Text '{range}' could not be parsed"),
        }
    }
    None
}

/// The `active` / `dropPeriod` handling of `parseDocument`: 21 characters are two full dates, 11
/// are two day-months of the current year, anything else leaves the dates unset.
fn parse_period(period: &str, year: i64, offset_at: &impl Fn(i64) -> i64) -> Option<(i64, i64)> {
    if period.len() == 21 {
        parse_date_range(period, offset_at)
    } else if period.len() == 11 {
        let mut parts = period.split('-');
        let (start, end) = (parts.next()?, parts.next()?);
        parse_date_range(&format!("{start} {year}-{end} {year}"), offset_at)
    } else {
        None
    }
}

/// The dates of a config; the error is Java's `NullPointerException` text.
#[derive(Debug, PartialEq, Eq)]
struct Dates {
    start: i64,
    end: i64,
    drop_start: Option<i64>,
    drop_end: Option<i64>,
}

fn read_dates(cfg: &EventConfig, now_ms: i64, offset_at: &impl Fn(i64) -> i64) -> Result<Dates, String> {
    let year = year_from_days((now_ms + offset_at(now_ms)).div_euclid(DAY_MS));
    let period = cfg.active;
    // If no drop period is defined, use the active period as the drop period.
    let drop_period = cfg.drop_period.filter(|p| !p.is_empty()).unwrap_or(period);
    let active = parse_period(period, year, offset_at);
    let drop = parse_period(drop_period, year, offset_at);
    let (start, end) = (active.map(|a| a.0), active.map(|a| a.1));
    let (drop_start, drop_end) = (drop.map(|a| a.0), drop.map(|a| a.1));

    // Check that drop period is inside active period. (`before(null)` is a NullPointerException
    // in Java, which fails the load as well.)
    if let Some(d) = drop_start
        && start.is_none_or(|s| d < s)
    {
        return Err(format!(
            "{} event: drop period must be within the active period",
            cfg.name
        ));
    }
    if let Some(d) = drop_end
        && end.is_none_or(|e| d > e)
    {
        return Err(format!(
            "{} event: drop period must be within the active period",
            cfg.name
        ));
    }
    match (start, end) {
        (Some(start), Some(end)) if start < end => Ok(Dates {
            start,
            end,
            drop_start,
            drop_end,
        }),
        _ => Err(format!("{} event: illegal event period", cfg.name)),
    }
}

// ------------------------------------------------------------------------------ the event

/// Java's constructor of `LongTimeEvent`: reads the config and starts the event (now inside the
/// dates), schedules its start (dates ahead) or destroys its items (dates passed). Call it from
/// the `on_load` of the event script: Java's spawns wait for `ON_SERVER_START`, which is after
/// every script has loaded, and so does `on_load`.
pub fn load(ctx: &mut ScriptCtx<'_>, cfg: &EventConfig) {
    let now = now(ctx);
    let offset_at = |t: i64| l2_util::time::local_offset_millis(t);
    let mut state = EventState::default();
    match read_dates(cfg, now, &offset_at) {
        Ok(dates) => {
            state.start_date = Some(dates.start);
            state.end_date = Some(dates.end);
            state.drop_start_date = dates.drop_start;
            state.drop_end_date = dates.drop_end;
        }
        Err(message) => {
            // Java throws from the constructor: the script does not load.
            warn!("WARNING!!! {message}");
            return;
        }
    }
    store(ctx, state.clone());

    if let (Some(start), Some(end)) = (state.start_date, state.end_date) {
        if now >= start && now <= end {
            start_event(ctx, cfg);
            info!("Event {} active till {}", cfg.name, end);
        } else if start > now {
            ctx.start_quest_timer(SCHEDULE_START, start - now, None, None, false);
            info!("Event {} will be started at {}", cfg.name, start);
        } else {
            // Destroy items that must exist only on event period.
            destroy_items_on_end(ctx, cfg);
            info!("Event {} has passed... Ignored ", cfg.name);
        }
    }
    let mut state = state_of(ctx);
    state.initialized = true;
    store(ctx, state);
}

/// The timers of the event (`ScheduleStart`, `ScheduleEnd` and the spawn's `stopRespawn`).
/// Returns true when `event` was one of them.
pub fn on_event(ctx: &mut ScriptCtx<'_>, cfg: &EventConfig, event: &str, npc: Option<ScriptNpc>) -> bool {
    match event {
        SCHEDULE_START => start_event(ctx, cfg),
        SCHEDULE_END => stop_event(ctx, cfg),
        STOP_RESPAWN => {
            if let Some(index) = npc.and_then(|n| ctx.world().npc_spawn_index(n.object_id)) {
                ctx.world_mut().stop_spawn_respawn(index);
            }
        }
        _ => return false,
    }
    true
}

/// Java `startEvent()`.
fn start_event(ctx: &mut ScriptCtx<'_>, cfg: &EventConfig) {
    let now = now(ctx);
    let mut state = state_of(ctx);
    // Set Active.
    state.active = true;

    // Add event drops if within the drop period.
    if let (Some(drop_start), Some(drop_end)) = (state.drop_start_date, state.drop_end_date)
        && now >= drop_start
        && now <= drop_end
    {
        let name = ctx.script_name();
        let drops = event_drops(ctx, cfg);
        ctx.world_mut().add_event_drops(name, drops);
    }

    if !cfg.spawns.is_empty() {
        // Add spawns on event start / on server start (`on_load` is after the scripts loaded).
        spawn_npcs(ctx, cfg, state.end_date.unwrap_or(now));
    }

    // Event enter announcement.
    if !cfg.on_enter_msg.is_empty() {
        // Send message on begin.
        ctx.broadcast_to_all_online_players(cfg.on_enter_msg);

        // Add announce for entering players.
        if let (Some(start), Some(end)) = (state.start_date, state.end_date) {
            state.enter_announce_id = ctx
                .world_mut()
                .add_event_announcement(start, end, cfg.on_enter_msg);
        }
    }

    // Schedule event end.
    let millis_to_event_end = state.end_date.unwrap_or(now) - now;
    store(ctx, state);
    ctx.start_quest_timer(SCHEDULE_END, millis_to_event_end, None, None, false);
}

/// Java `stopEvent()`.
fn stop_event(ctx: &mut ScriptCtx<'_>, cfg: &EventConfig) {
    let mut state = state_of(ctx);
    // Set Active.
    state.active = false;

    // Stop event drops.
    let name = ctx.script_name();
    ctx.world_mut().remove_event_drops(name);

    // Destroy items that must exist only on event period.
    destroy_items_on_end(ctx, cfg);

    // Send message on end.
    if !cfg.on_end_msg.is_empty() {
        ctx.broadcast_to_all_online_players(cfg.on_end_msg);
    }

    // Remove announce for entering players.
    if let Some(id) = state.enter_announce_id.take() {
        ctx.world_mut().delete_event_announcement(id);
    }
    store(ctx, state);
}

/// The `<droplist>` as Java's loader keeps it: an unknown item, `min > max` and a chance outside
/// 0 to 100 are skipped with a warning.
fn event_drops(ctx: &mut ScriptCtx<'_>, cfg: &EventConfig) -> Vec<EventDrop> {
    let mut list = Vec::new();
    for drop in cfg.drops {
        if ctx.world().item_template_name(drop.item).is_none() {
            warn!(
                "{} event: {} is wrong item id, item was not added in droplist",
                ctx.script_name(),
                drop.item
            );
            continue;
        }
        if drop.min > drop.max {
            warn!(
                "{} event: item {} - min greater than max, item was not added in droplist",
                ctx.script_name(),
                drop.item
            );
            continue;
        }
        if drop.chance < 0.0 || drop.chance > 100.0 {
            warn!(
                "{} event: item {} - incorrect drop chance, item was not added in droplist",
                ctx.script_name(),
                drop.item
            );
            continue;
        }
        list.push(EventDrop {
            item_id: drop.item,
            min: drop.min,
            max: drop.max,
            chance: drop.chance,
            min_level: drop.min_level,
            max_level: drop.max_level,
            monster_ids: drop.monster_ids.iter().copied().collect(),
        });
    }
    list
}

/// Java `spawnNpcs()`: every NPC of the spawn list, deleted when the event ends.
fn spawn_npcs(ctx: &mut ScriptCtx<'_>, cfg: &EventConfig, end_date: i64) {
    let millis_to_event_end = end_date - now(ctx);
    for spawn in cfg.spawns {
        if !ctx.world().npc_template_exists(spawn.npc) {
            warn!(
                "{} event: {} is wrong NPC id, NPC was not added in spawnlist",
                ctx.script_name(),
                spawn.npc
            );
            continue;
        }
        let npc = ctx.add_spawn(
            spawn.npc,
            spawn.x,
            spawn.y,
            spawn.z,
            spawn.heading,
            false,
            millis_to_event_end,
            0,
        );
        if spawn.respawn_time_ms > 0
            && let Some(npc) = npc
            && let Some(index) = ctx.world().npc_spawn_index(npc.object_id)
        {
            // NOTE(java-quirk): `setRespawnDelay` takes seconds and gets `respawnTime * 1000`.
            ctx.world_mut()
                .set_spawn_respawn_delay(index, spawn.respawn_time_ms);
            ctx.world_mut().start_spawn_respawn(index);
            ctx.start_quest_timer(
                STOP_RESPAWN,
                millis_to_event_end - i64::from(spawn.respawn_time_ms),
                Some(npc),
                None,
                false,
            );
        }
    }
}

/// Java `destroyItemsOnEnd()`: the items leave every online player's inventory.
///
/// Left out: `DELETE FROM items WHERE item_id=?` (the rows of offline characters): the world has
/// no outbox request for it yet.
fn destroy_items_on_end(ctx: &mut ScriptCtx<'_>, cfg: &EventConfig) {
    for &item_id in cfg.destroy_items_on_end {
        if ctx.world().item_template_name(item_id).is_none() {
            warn!("{} event: Item {} does not exist.", ctx.script_name(), item_id);
            continue;
        }
        // Remove item from online players.
        for player in ctx.online_players() {
            ctx.world_mut().destroy_item_by_item_id(player, item_id, -1, true);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UTC: fn(i64) -> i64 = |_| 0;

    fn config(active: &'static str, drop_period: Option<&'static str>) -> EventConfig {
        EventConfig {
            name: "Test",
            active,
            drop_period,
            drops: &[],
            spawns: &[],
            on_enter_msg: "",
            on_end_msg: "",
            destroy_items_on_end: &[],
        }
    }

    #[test]
    fn dates_are_parsed_like_java() {
        // 2007-02-27 00:00 UTC.
        assert_eq!(parse_date("27 02 2007", &UTC), Some(1_172_534_400_000));
        // One digit, a bad month and a missing year do not parse.
        assert_eq!(parse_date("7 02 2007", &UTC), None);
        assert_eq!(parse_date("27 13 2007", &UTC), None);
        assert_eq!(parse_date("27 02", &UTC), None);
        // The smart resolver moves the 30th of February to the last day.
        assert_eq!(parse_date("30 02 2007", &UTC), parse_date("28 02 2007", &UTC));
        // The zone moves the start of the day: UTC+1 starts an hour earlier.
        assert_eq!(
            parse_date("27 02 2007", &|_| 3_600_000),
            Some(1_172_534_400_000 - 3_600_000)
        );
    }

    #[test]
    fn the_two_period_formats() {
        let now = 1_172_534_400_000; // 2007-02-27
        let full = read_dates(&config("27 02 2007-28 02 2007", None), now, &UTC).unwrap();
        assert_eq!(full.end - full.start, DAY_MS);
        // Eleven characters: this year. The drop period is the active period when absent.
        let short = read_dates(&config("27 02-28 02", None), now, &UTC).unwrap();
        assert_eq!(short, full_with_drop(&full));
        // Other lengths leave the dates unset: an illegal period.
        assert!(read_dates(&config("27 2 2007-28 02 2007", None), now, &UTC).is_err());
    }

    fn full_with_drop(full: &Dates) -> Dates {
        Dates {
            start: full.start,
            end: full.end,
            drop_start: Some(full.start),
            drop_end: Some(full.end),
        }
    }

    #[test]
    fn a_bad_period_fails_the_load() {
        let now = 1_172_534_400_000;
        // Start after end.
        assert!(read_dates(&config("28 02 2007-27 02 2007", None), now, &UTC).is_err());
        // The drop period starts before the event does.
        assert!(
            read_dates(
                &config("27 02 2007-28 02 2007", Some("26 02 2007-28 02 2007")),
                now,
                &UTC
            )
            .is_err()
        );
        // The drop period ends after the event does.
        assert!(
            read_dates(
                &config("27 02 2007-28 02 2007", Some("27 02 2007-01 03 2007")),
                now,
                &UTC
            )
            .is_err()
        );
        // Java quirk: a drop period that ends before it starts passes the check.
        assert!(
            read_dates(
                &config("27 02 2007-28 02 2007", Some("28 02 2007-27 02 2007")),
                now,
                &UTC
            )
            .is_ok()
        );
    }

    // ------------------------------------------------------------ a stand-in event on a world

    use l2_util::ObjectId;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use crate::testing::{ScriptTest, item, names_to};

    const NPCS: &str = r#"<npc id="31860" type="Folk" level="70" name="Event Manager"/>"#;
    const ITEMS: &str = r#"
        <item id="6391" type="EtcItem" name="Nectar"><set name="is_stackable" val="true"/></item>
        <item id="6392" type="EtcItem" name="Medal"><set name="is_stackable" val="true"/></item>
        <item id="15366" type="EtcItem" name="Event Item"><set name="is_stackable" val="true"/></item>"#;

    const fn drop(item: i32, min: i64, max: i64, chance: f64) -> DropSpec {
        DropSpec {
            item,
            min,
            max,
            chance,
            min_level: 1,
            max_level: i32::MAX,
            monster_ids: &[],
        }
    }

    /// The droplist of the stand-in event: two good rows and the three kinds Java refuses.
    const DROPS: &[DropSpec] = &[
        drop(6391, 1, 1, 10.0),
        drop(6392, 2, 3, 5.0),
        // An item the server does not know.
        drop(99_999, 1, 1, 5.0),
        // `min` above `max`.
        drop(6391, 5, 1, 5.0),
        // A chance above 100.
        drop(6391, 1, 1, 150.0),
    ];

    /// Two good places and an NPC the server does not know.
    const SPAWNS: &[SpawnSpec] = &[
        SpawnSpec::at(31860, 10_000, 10_000, -3_000, 0, 0),
        SpawnSpec::at(31860, 10_100, 10_000, -3_000, 16_000, 0),
        SpawnSpec::at(777_777, 10_200, 10_000, -3_000, 0, 0),
    ];

    const fn event(active: &'static str, drop_period: Option<&'static str>) -> EventConfig {
        EventConfig {
            name: "Test Event",
            active,
            drop_period,
            drops: DROPS,
            spawns: SPAWNS,
            on_enter_msg: "Test Event: ongoing!",
            on_end_msg: "Test Event: end!",
            destroy_items_on_end: &[15_366, 99_999],
        }
    }

    /// Now is inside, the drops run only in the second half of the event.
    const INSIDE: EventConfig = event("01 01 2000-31 12 2099", Some("01 01 2090-31 12 2099"));
    const INSIDE_DROPS: EventConfig = event("01 01 2000-31 12 2099", None);
    const AHEAD: EventConfig = event("01 01 2098-31 12 2099", None);
    const PASSED: EventConfig = event("01 01 2000-02 01 2000", None);

    /// `class TestEvent extends LongTimeEvent`.
    struct TestEvent(&'static EventConfig);

    impl Script for TestEvent {
        fn name(&self) -> &'static str {
            "TestEvent"
        }
        fn path(&self) -> &'static str {
            "events/TestEvent"
        }
        fn setup(&self, _s: &mut ScriptSetup) {}
        fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
            load(ctx, self.0);
        }
        fn on_event(
            &self,
            ctx: &mut ScriptCtx<'_>,
            event: &str,
            npc: Option<ScriptNpc>,
            _player: Option<ObjectId>,
        ) -> Option<String> {
            on_event(ctx, self.0, event, npc);
            None
        }
    }

    fn world(config: &'static EventConfig) -> ScriptTest {
        TEST_CLOCK.set(None);
        let mut registry = ScriptRegistry::new();
        registry.add(TestEvent(config));
        ScriptTest::new(registry, NPCS, ITEMS, &[])
    }

    fn period(t: &mut ScriptTest) -> bool {
        let mut active = false;
        t.world
            .run_as_script("TestEvent", |ctx| active = is_event_period(ctx));
        active
    }

    fn fire(t: &mut ScriptTest, config: &'static EventConfig, timer: &str) {
        t.world.run_as_script("TestEvent", |ctx| {
            assert!(
                on_event(ctx, config, timer, None),
                "{timer} is a timer of the event"
            );
        });
    }

    #[test]
    fn an_event_inside_its_dates_starts_with_drops_npcs_and_an_announcement() {
        let mut t = world(&INSIDE_DROPS);
        assert!(period(&mut t));
        // The good rows only.
        let drops = t.world.event_drops_of("TestEvent").unwrap();
        assert_eq!(drops.len(), 2);
        assert_eq!(
            (drops[0].item_id, drops[0].min, drops[0].max, drops[0].chance),
            (6391, 1, 1, 10.0)
        );
        assert_eq!(
            (drops[1].item_id, drops[1].min, drops[1].max, drops[1].chance),
            (6392, 2, 3, 5.0)
        );
        // Two NPCs; the unknown NPC id is refused.
        assert_eq!(t.npcs_of(31860).len(), 2);
        // The enter message is an event announcement that a player entering later is shown.
        assert_eq!(t.world.announcement_count(), 1);
        t.take();
        t.enter(1, 40, vec![]);
        let names = names_to(&t.take(), 1);
        assert!(names.contains(&"SAY2".to_owned()), "{names:?}");
        // The end is on the wheel.
        let mut end = false;
        t.world.run_as_script("TestEvent", |ctx| {
            end = ctx.has_quest_timer(SCHEDULE_END, None, None)
        });
        assert!(end);
        let mut dates = (None, None);
        t.world
            .run_as_script("TestEvent", |ctx| dates = (start_date(ctx), end_date(ctx)));
        assert!(dates.0.unwrap() < dates.1.unwrap());
    }

    #[test]
    fn the_drops_wait_for_their_own_period() {
        // The drop period (2090 to 2099) has not begun: the event runs, the drops do not.
        let mut t = world(&INSIDE);
        assert!(period(&mut t));
        assert!(t.world.event_drops_of("TestEvent").is_none());
        assert_eq!(t.npcs_of(31860).len(), 2);
    }

    #[test]
    fn the_end_of_the_event_takes_everything_back() {
        let mut t = world(&INSIDE_DROPS);
        t.enter(1, 40, vec![item(1000, 15_366, 3), item(1001, 6392, 4)]);
        t.take();
        fire(&mut t, &INSIDE_DROPS, SCHEDULE_END);
        assert!(!period(&mut t));
        assert!(t.world.event_drops_of("TestEvent").is_none());
        assert_eq!(t.world.announcement_count(), 0);
        // The end message went to the player and the event item left the bag; other items stay.
        assert!(names_to(&t.take(), 1).contains(&"SAY2".to_owned()));
        assert_eq!(t.count(1, 15_366), 0);
        assert_eq!(t.count(1, 6392), 4);
    }

    #[test]
    fn an_event_ahead_waits_for_its_start() {
        let mut t = world(&AHEAD);
        assert!(!period(&mut t));
        assert!(t.world.event_drops_of("TestEvent").is_none());
        assert!(t.npcs_of(31860).is_empty());
        assert_eq!(t.world.announcement_count(), 0);
        let mut scheduled = false;
        t.world.run_as_script("TestEvent", |ctx| {
            scheduled = ctx.has_quest_timer(SCHEDULE_START, None, None)
        });
        assert!(scheduled, "the start is on the wheel");
        // The start timer starts it (the NPCs and the message; the drops wait for 2098 as well).
        fire(&mut t, &AHEAD, SCHEDULE_START);
        assert!(period(&mut t));
        assert_eq!(t.npcs_of(31860).len(), 2);
        assert_eq!(t.world.announcement_count(), 1);
        assert!(t.world.event_drops_of("TestEvent").is_none());
    }

    #[test]
    fn a_passed_event_does_nothing_but_destroy_its_items() {
        let mut t = world(&PASSED);
        assert!(!period(&mut t));
        assert!(t.world.event_drops_of("TestEvent").is_none());
        assert!(t.npcs_of(31860).is_empty());
        assert_eq!(t.world.announcement_count(), 0);
        // `destroyItemsOnEnd` ran at the start; with a player online it takes the item.
        t.enter(1, 40, vec![item(1000, 15_366, 3)]);
        t.world.run_as_script("TestEvent", |ctx| load(ctx, &PASSED));
        assert_eq!(t.count(1, 15_366), 0);
    }

    #[test]
    fn other_events_are_not_timers_of_the_event() {
        let mut t = world(&PASSED);
        t.world.run_as_script("TestEvent", |ctx| {
            assert!(!on_event(ctx, &PASSED, "something else", None));
        });
    }
}
