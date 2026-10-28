//! Java's seed managers: the stage of the Seed of Destruction and of the Seed of Infinity.
// Java: gameserver/managers/SeedOfInfinityManager.java, SeedOfDestructionManager.java
//
// The managers keep their state in `global_variables` (`SoDState`, `SoI_stage`, ...), which the
// scripts read through the context (D-54: in memory, written only with `CycleDatabaseWrites`).
//
// Java's managers are singletons of the core; here they are two scripts without NPCs,
// [`SeedOfDestructionManager`] and [`SeedOfInfinityManager`], so their timers (the end of the
// Seed of Destruction's stage 2, the closing of the Seed of Infinity) are quest timers on the
// wheel. Other scripts call them with `ctx.notify_event_of(<manager>, <event>, None, None)`:
//
// | Java | event |
// |---|---|
// | `SeedOfDestructionManager.increaseSoDTiatKilled()` | [`SOD_INCREASE_TIAT_KILLED`] |
// | `SeedOfDestructionManager.setSoDState(value, true)` | `"setSoDState <value>"` |
// | `SeedOfInfinityManager.notifyCohemenesKill()` | [`SOI_NOTIFY_COHEMENES_KILL`] |
// | `SeedOfInfinityManager.notifyEkimusKill()` | [`SOI_NOTIFY_EKIMUS_KILL`] |
// | `SeedOfInfinityManager.notifyHoEDefSuccess()` | [`SOI_NOTIFY_HOE_DEF_SUCCESS`] |
// | `SeedOfInfinityManager.setCurrentStage(stage)` | `"setCurrentStage <stage>"` |
//
// # Side by side (D-54)
// Every event that changes a stage first re-reads the manager's rows
// (`ctx.reread_global_variables`) and decides on the rows the Java GS wrote; with
// `CycleDatabaseWrites` on the copy is the truth and the decision runs at once (queued).
//
// NOTE: a stage the Java GS changed is only seen at the next decision; this server does not open
// the Seed of Infinity's door or start its closing timer for a stage 3 it re-read (Java's
// constructor does it only at startup, which `on_load` ports).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};
use tracing::{info, warn};

use crate::ai::areas::gracia::util::players_in_zone;

/// The script name of [`SeedOfDestructionManager`].
pub const SOD_MANAGER: &str = "SeedOfDestructionManager";
/// The script name of [`SeedOfInfinityManager`].
pub const SOI_MANAGER: &str = "SeedOfInfinityManager";
/// `SeedOfDestructionManager.increaseSoDTiatKilled()`.
pub const SOD_INCREASE_TIAT_KILLED: &str = "increaseSoDTiatKilled";
/// `SeedOfInfinityManager.notifyCohemenesKill()`.
pub const SOI_NOTIFY_COHEMENES_KILL: &str = "notifyCohemenesKill";
/// `SeedOfInfinityManager.notifyEkimusKill()`.
pub const SOI_NOTIFY_EKIMUS_KILL: &str = "notifyEkimusKill";
/// `SeedOfInfinityManager.notifyHoEDefSuccess()`.
pub const SOI_NOTIFY_HOE_DEF_SUCCESS: &str = "notifyHoEDefSuccess";

/// The prefix of the events that run once the rows are re-read.
const REREAD: &str = "reread ";

// ---------------------------------------------------------------------------------- readers

/// Java `SeedOfInfinityManager.getCurrentStage()`.
pub fn soi_current_stage(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.global_variables().get_int("SoI_stage", 1)
}

/// Java `SeedOfDestructionManager.getInstance().getSoDState()`: 1 until a state is stored.
pub fn sod_state(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.global_variables().get_int("SoDState", 1)
}

/// Java `SeedOfDestructionManager.getSoDTiatKilled()`.
pub fn sod_tiat_killed(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.global_variables().get_int("SoDTiatKilled", 0)
}

/// Java `getSoDLastStateChangeDate().getTimeInMillis()`.
pub fn sod_last_state_change(ctx: &ScriptCtx<'_>) -> i64 {
    ctx.global_variables().get_long("SoDLSCDate", 0)
}

/// Java `SeedOfDestructionManager.getSoDTimeForNextStateChange()` (`now` is
/// `System.currentTimeMillis()`): -1 outside stage 2.
pub fn sod_time_for_next_state_change(ctx: &ScriptCtx<'_>, now: i64) -> i64 {
    match sod_state(ctx) {
        2 => (sod_last_state_change(ctx) + ctx.script_settings().gracia_seeds.sod_stage_2_length) - now,
        // 1, "3: not implemented yet", default: "this should not happen!"
        _ => -1,
    }
}

/// Java `SeedOfInfinityManager.getOpenedTime()`, in milliseconds (`now` is `System.currentTimeMillis()`).
pub fn soi_opened_time(ctx: &ScriptCtx<'_>, now: i64) -> i64 {
    if soi_current_stage(ctx) != 3 {
        return 0;
    }
    (ctx.global_variables().get_long("SoI_opened", 0) * 1000) - now
}

/// Java `SeedOfInfinityManager.isSeedOpen()`.
pub fn soi_is_seed_open(ctx: &ScriptCtx<'_>, now: i64) -> bool {
    soi_opened_time(ctx, now) > 0
}

/// Java `SeedOfInfinityManager.getCohemenesCount()`.
pub fn soi_cohemenes_count(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.global_variables().get_int("SoI_CohemenesCount", 0)
}

/// Java `SeedOfInfinityManager.getEkimusCount()`.
pub fn soi_ekimus_count(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.global_variables().get_int("SoI_EkimusCount", 0)
}

/// Java `SeedOfInfinityManager.getHoEDefCount()`.
pub fn soi_hoe_def_count(ctx: &ScriptCtx<'_>) -> i32 {
    ctx.global_variables().get_int("SoI_hoedefkillcount", 0)
}

/// Java `SeedOfInfinityManager.teleportInSeed(player)`: one of the open seed's places, at random.
pub fn soi_teleport_in_seed(ctx: &mut ScriptCtx<'_>, player: ObjectId) {
    let index = ctx.get_random(OPEN_SEED_TELEPORT_LOCS.len() as i32);
    let (x, y, z) = OPEN_SEED_TELEPORT_LOCS[usize::try_from(index).unwrap_or(0)];
    ctx.world_mut()
        .tele_to_location(player, l2_world::teleport::TeleportTarget::at(x, y, z));
}

// ---------------------------------------------------------------------- Seed of Destruction

const SOD_VARIABLES: [&str; 3] = ["SoDState", "SoDTiatKilled", "SoDLSCDate"];
const EDRIC: i32 = 32527;
const ENERGY_SEEDS: &str = "EnergySeeds";

/// The numbers of the stage 2 timers (Java schedules a new task at every `handleSodStages`).
#[derive(Default)]
struct SodState {
    next_timer: u64,
}

/// Java `managers.SeedOfDestructionManager`.
pub struct SeedOfDestructionManager;

impl SeedOfDestructionManager {
    /// Java `saveData(SODTYPE)`.
    fn save_data(ctx: &mut ScriptCtx<'_>, state: i32, tiat_killed: i32, last_change: i64) {
        ctx.set_global_variable("SoDState", &state.to_string());
        ctx.set_global_variable("SoDTiatKilled", &tiat_killed.to_string());
        ctx.set_global_variable("SoDLSCDate", &last_change.to_string());
    }

    /// Java `loadData()`: the first start stores the initial values.
    fn load_data(ctx: &mut ScriptCtx<'_>) {
        if !ctx.global_variables().has_variable("SoDState") {
            let now = ctx.now_ms();
            Self::save_data(ctx, 1, 0, now);
        }
    }

    /// Java `handleSodStages()`.
    fn handle_sod_stages(ctx: &mut ScriptCtx<'_>) {
        match sod_state(ctx) {
            1 => {
                // Do nothing players should kill Tiat a few times
            }
            2 => {
                // Conquest Complete state, if too much time is passed than change to defense state
                let length = ctx.script_settings().gracia_seeds.sod_stage_2_length;
                let time_past = ctx.now_ms() - sod_last_state_change(ctx);
                if time_past >= length {
                    // change to Attack state because Defend statet is not implemented
                    Self::set_sod_state(ctx, 1, true);
                } else {
                    let state = ctx.state::<SodState>();
                    state.next_timer += 1;
                    let name = format!("stage2End {}", state.next_timer);
                    ctx.start_quest_timer(&name, length - time_past, None, None, false);
                }
            }
            3 => {
                // not implemented
                Self::set_sod_state(ctx, 1, true);
            }
            other => warn!("SeedOfDestructionManager: Unknown Seed of Destruction state({other})! "),
        }

        Self::manage_edric_spawn(ctx);
    }

    /// Java `manageEdricSpawn()`: `World.getNpc(32527)` is the first Edric in the world.
    fn manage_edric_spawn(ctx: &mut ScriptCtx<'_>) {
        let edric = ctx.world().npc_ids().into_iter().find(|id| {
            ctx.world()
                .npc(*id)
                .is_some_and(|n| n.npc_id == EDRIC && !n.decayed)
        });
        if sod_state(ctx) == 2 {
            if edric.is_none() {
                ctx.add_spawn(EDRIC, -248525, 250048, 4307, 24576, false, 0, 0);
            }
        } else if let Some(edric) = edric {
            ctx.world_mut().delete_npc(edric);
        }
    }

    /// Java `updateSodState()`.
    fn update_sod_state(ctx: &mut ScriptCtx<'_>) {
        if !ctx.world().has_script(ENERGY_SEEDS) {
            warn!("SeedOfDestructionManager: missing EnergySeeds Quest!");
            return;
        }
        ctx.notify_event_of(ENERGY_SEEDS, "StopSoDAi", None, None);
    }

    /// Java `increaseSoDTiatKilled()`.
    fn increase_sod_tiat_killed(ctx: &mut ScriptCtx<'_>) {
        if sod_state(ctx) == 1 {
            let killed = sod_tiat_killed(ctx) + 1;
            ctx.set_global_variable("SoDTiatKilled", &killed.to_string());
            if killed >= ctx.script_settings().gracia_seeds.sod_tiat_kill_count {
                Self::set_sod_state(ctx, 2, false);
            }

            let (state, killed, last) = (sod_state(ctx), sod_tiat_killed(ctx), sod_last_state_change(ctx));
            Self::save_data(ctx, state, killed, last);
            if !ctx.world().has_script(ENERGY_SEEDS) {
                warn!("SeedOfDestructionManager: missing EnergySeeds Quest!");
            } else {
                ctx.notify_event_of(ENERGY_SEEDS, "StartSoDAi", None, None);
            }
        }
    }

    /// Java `setSoDState(value, doSave)`.
    // NOTE: Java keeps the state in fields and writes the variables in `saveData`; here the
    // variables are the fields, so `doSave` false changes them in memory as well (they are
    // written with the other global variables, D-54).
    fn set_sod_state(ctx: &mut ScriptCtx<'_>, value: i32, _do_save: bool) {
        info!("SeedOfDestructionManager: New Seed of Destruction state -> {value}.");
        let now = ctx.now_ms();
        ctx.set_global_variable("SoDLSCDate", &now.to_string());
        ctx.set_global_variable("SoDState", &value.to_string());

        // Reset number of Tiat kills.
        if value == 1 {
            ctx.set_global_variable("SoDTiatKilled", "0");
        }

        Self::handle_sod_stages(ctx);
    }
}

impl Script for SeedOfDestructionManager {
    fn name(&self) -> &'static str {
        SOD_MANAGER
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia"
    }

    fn setup(&self, _s: &mut ScriptSetup) {}

    // The constructor: `loadData(); handleSodStages();`.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        Self::load_data(ctx);
        Self::handle_sod_stages(ctx);
        info!(
            "Seed of Destruction Manager: Loaded. Current stage is: {}",
            sod_state(ctx)
        );
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(decided) = event.strip_prefix(REREAD) {
            if decided == SOD_INCREASE_TIAT_KILLED {
                Self::increase_sod_tiat_killed(ctx);
            } else if decided.starts_with("stage2End") {
                Self::set_sod_state(ctx, 1, true);
                Self::update_sod_state(ctx);
            } else if let Some(value) = decided.strip_prefix("setSoDState ") {
                if let Ok(value) = value.trim().parse::<i32>() {
                    Self::set_sod_state(ctx, value, true);
                }
            }
            return None;
        }
        // Every decision first sees the rows of the other server (D-54).
        if event == SOD_INCREASE_TIAT_KILLED
            || event.starts_with("stage2End")
            || event.starts_with("setSoDState ")
        {
            ctx.reread_global_variables(&SOD_VARIABLES, &format!("{REREAD}{event}"), None, None);
        }
        None
    }
}

// ------------------------------------------------------------------------ Seed of Infinity

const SOI_VARIABLES: [&str; 5] = [
    "SoI_stage",
    "SoI_opened",
    "SoI_CohemenesCount",
    "SoI_EkimusCount",
    "SoI_hoedefkillcount",
];
const SOI_OPEN_TIME: i64 = 24 * 60 * 60 * 1000;
const SOI_DOOR: i32 = 14240102;
const SOI_ZONE: i32 = 60010;

const OPEN_SEED_TELEPORT_LOCS: [(i32, i32, i32); 18] = [
    (-179537, 209551, -15504),
    (-179779, 212540, -15520),
    (-177028, 211135, -15520),
    (-176355, 208043, -15520),
    (-179284, 205990, -15520),
    (-182268, 208218, -15520),
    (-182069, 211140, -15520),
    (-176036, 210002, -11948),
    (-176039, 208203, -11949),
    (-183288, 208205, -11939),
    (-183290, 210004, -11939),
    (-187776, 205696, -9536),
    (-186327, 208286, -9536),
    (-184429, 211155, -9536),
    (-182811, 213871, -9504),
    (-180921, 216789, -9536),
    (-177264, 217760, -9536),
    (-173727, 218169, -9536),
];

/// The numbers of the closing timers (Java schedules a new task at every `openSeed`).
#[derive(Default)]
struct SoiState {
    next_timer: u64,
}

/// Java `managers.SeedOfInfinityManager`.
pub struct SeedOfInfinityManager;

impl SeedOfInfinityManager {
    /// Java `setCurrentStage(stage)`.
    fn set_current_stage(ctx: &mut ScriptCtx<'_>, stage: i32) {
        if soi_current_stage(ctx) == stage {
            return;
        }

        if stage == 3 {
            Self::open_seed(ctx, SOI_OPEN_TIME);
        } else if soi_is_seed_open(ctx, ctx.now_ms()) {
            Self::close_seed(ctx);
        }

        ctx.set_global_variable("SoI_stage", &stage.to_string());
        Self::set_count(ctx, "SoI_CohemenesCount", 0);
        Self::set_count(ctx, "SoI_EkimusCount", 0);
        Self::set_count(ctx, "SoI_hoedefkillcount", 0);
        // `checkStageAndSpawn()`: every spawn of it is commented out in Java.
        info!("Seed of Infinity Manager: Set to stage {stage}");
    }

    fn set_count(ctx: &mut ScriptCtx<'_>, name: &str, value: i32) {
        ctx.set_global_variable(name, &value.to_string());
    }

    /// Java `openSeed(time)`.
    fn open_seed(ctx: &mut ScriptCtx<'_>, time: i64) {
        if time <= 0 {
            return;
        }

        let close_time = ctx.now_ms() + time;
        ctx.set_global_variable("SoI_opened", &(close_time / 1000).to_string());
        info!("Seed of Infinity Manager: Opening the seed for {time} ms");
        // `spawnOpenedSeed()`: its spawn is commented out in Java.
        ctx.open_door(SOI_DOOR, 0);

        let state = ctx.state::<SoiState>();
        state.next_timer += 1;
        let name = format!("closeSeed {}", state.next_timer);
        ctx.start_quest_timer(&name, time, None, None, false);
    }

    /// Java `closeSeed()`.
    fn close_seed(ctx: &mut ScriptCtx<'_>) {
        info!("Seed of Infinity Manager: Closing the seed.");
        ctx.remove_global_variable("SoI_opened");

        // EnergySeeds.SoiSeedStop();
        ctx.close_door(SOI_DOOR, 0);
        for ch in players_in_zone(ctx, SOI_ZONE) {
            ctx.world_mut().tele_to_location(
                ch,
                l2_world::teleport::TeleportTarget::at(-183285, 205996, -12896),
            );
        }
    }

    /// Java `notifyCohemenesKill()`.
    fn notify_cohemenes_kill(ctx: &mut ScriptCtx<'_>) {
        if soi_current_stage(ctx) == 1 {
            if soi_cohemenes_count(ctx) < 9 {
                let count = soi_cohemenes_count(ctx) + 1;
                Self::set_count(ctx, "SoI_CohemenesCount", count);
            } else {
                Self::set_current_stage(ctx, 2);
            }
        }
    }

    /// Java `notifyEkimusKill()`.
    fn notify_ekimus_kill(ctx: &mut ScriptCtx<'_>) {
        if soi_current_stage(ctx) == 2 {
            if soi_ekimus_count(ctx) < ctx.script_settings().gracia_seeds.soi_ekimus_kill_count {
                let count = soi_ekimus_count(ctx) + 1;
                Self::set_count(ctx, "SoI_EkimusCount", count);
            } else {
                Self::set_current_stage(ctx, 3);
            }
        }
    }

    /// Java `notifyHoEDefSuccess()`.
    fn notify_hoe_def_success(ctx: &mut ScriptCtx<'_>) {
        if soi_current_stage(ctx) == 4 {
            if soi_hoe_def_count(ctx) < 9 {
                let count = soi_hoe_def_count(ctx) + 1;
                Self::set_count(ctx, "SoI_hoedefkillcount", count);
            } else {
                Self::set_current_stage(ctx, 5);
            }
        }
    }
}

impl Script for SeedOfInfinityManager {
    fn name(&self) -> &'static str {
        SOI_MANAGER
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia"
    }

    fn setup(&self, _s: &mut ScriptSetup) {}

    // The constructor: `checkStageAndSpawn(); if (isSeedOpen()) openSeed(getOpenedTime());`.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        let now = ctx.now_ms();
        if soi_is_seed_open(ctx, now) {
            let time = soi_opened_time(ctx, now);
            Self::open_seed(ctx, time);
        }
        info!(
            "Seed of Infinity Manager: Loaded. Current stage is: {}",
            soi_current_stage(ctx)
        );
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if let Some(decided) = event.strip_prefix(REREAD) {
            match decided {
                SOI_NOTIFY_COHEMENES_KILL => Self::notify_cohemenes_kill(ctx),
                SOI_NOTIFY_EKIMUS_KILL => Self::notify_ekimus_kill(ctx),
                SOI_NOTIFY_HOE_DEF_SUCCESS => Self::notify_hoe_def_success(ctx),
                _ if decided.starts_with("closeSeed") => {
                    Self::close_seed(ctx);
                    Self::set_current_stage(ctx, 4);
                }
                _ => {
                    if let Some(stage) = decided
                        .strip_prefix("setCurrentStage ")
                        .and_then(|v| v.trim().parse::<i32>().ok())
                    {
                        Self::set_current_stage(ctx, stage);
                    }
                }
            }
            return None;
        }
        // Every decision first sees the rows of the other server (D-54).
        let decides = matches!(
            event,
            SOI_NOTIFY_COHEMENES_KILL | SOI_NOTIFY_EKIMUS_KILL | SOI_NOTIFY_HOE_DEF_SUCCESS
        ) || event.starts_with("closeSeed")
            || event.starts_with("setCurrentStage ");
        if decides {
            ctx.reread_global_variables(&SOI_VARIABLES, &format!("{REREAD}{event}"), None, None);
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::script::GraciaSeedsSettings;
    use l2_world::{ScriptRegistry, ScriptSettings, WorldCommand};

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="32527" type="Folk" level="70" name="Edric"/>"#;

    fn world(writes: bool, rows: Vec<(&str, &str)>) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(SeedOfDestructionManager);
        registry.add(SeedOfInfinityManager);
        let rows: Vec<(String, Option<String>)> = rows
            .into_iter()
            .map(|(k, v)| (k.to_owned(), Some(v.to_owned())))
            .collect();
        ScriptTest::new_with(registry, NPCS, "", &[], move |world, _| {
            world.set_script_settings(ScriptSettings {
                cycle_database_writes: writes,
                gracia_seeds: GraciaSeedsSettings {
                    sod_tiat_kill_count: 2,
                    soi_ekimus_kill_count: 1,
                    ..GraciaSeedsSettings::default()
                },
                ..ScriptSettings::default()
            });
            world.set_global_variables(rows);
        })
    }

    fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap())
            .unwrap()
    }

    fn var(t: &ScriptTest, name: &str) -> Option<String> {
        let globals = t.world.global_variables();
        globals.has_variable(name).then(|| globals.get_string(name, ""))
    }

    fn notify(t: &mut ScriptTest, manager: &str, event: &str) {
        let event = event.to_owned();
        t.world
            .run_as_script(manager, |ctx| ctx.notify_event(&event, None, None));
    }

    #[test]
    fn the_first_start_stores_the_initial_seed_of_destruction_values() {
        let t = world(true, vec![]);
        assert_eq!(var(&t, "SoDState").as_deref(), Some("1"));
        assert_eq!(var(&t, "SoDTiatKilled").as_deref(), Some("0"));
        assert!(var(&t, "SoDLSCDate").is_some());
        assert!(t.npcs_of(EDRIC).is_empty());
    }

    #[test]
    fn with_the_switch_on_tiat_kills_open_stage_2_and_spawn_edric() {
        let mut t = world(true, vec![]);
        notify(&mut t, SOD_MANAGER, SOD_INCREASE_TIAT_KILLED);
        assert_eq!(var(&t, "SoDTiatKilled").as_deref(), Some("1"));
        assert_eq!(var(&t, "SoDState").as_deref(), Some("1"));
        notify(&mut t, SOD_MANAGER, SOD_INCREASE_TIAT_KILLED);
        assert_eq!(var(&t, "SoDState").as_deref(), Some("2"));
        assert_eq!(t.npcs_of(EDRIC).len(), 1);
        // A kill in stage 2 counts nothing.
        notify(&mut t, SOD_MANAGER, SOD_INCREASE_TIAT_KILLED);
        assert_eq!(var(&t, "SoDTiatKilled").as_deref(), Some("2"));
        // Nothing waited for a read.
        assert_eq!(t.world.script_rereads_waiting(), 0);
    }

    #[test]
    fn with_the_switch_off_the_tiat_kill_decides_on_the_rows_read_again() {
        let mut t = world(
            false,
            vec![("SoDState", "1"), ("SoDTiatKilled", "0"), ("SoDLSCDate", "0")],
        );
        notify(&mut t, SOD_MANAGER, SOD_INCREASE_TIAT_KILLED);
        // The decision waits for the rows.
        assert_eq!(t.world.script_rereads_waiting(), 1);
        assert_eq!(var(&t, "SoDTiatKilled").as_deref(), Some("0"));
        // The Java GS counted a kill meanwhile: this one opens stage 2.
        t.world.handle(WorldCommand::GlobalVariablesReread {
            token: 1,
            result: Ok(vec![
                ("SoDState".to_owned(), Some("1".to_owned())),
                ("SoDTiatKilled".to_owned(), Some("1".to_owned())),
                ("SoDLSCDate".to_owned(), Some("0".to_owned())),
            ]),
        });
        assert_eq!(var(&t, "SoDState").as_deref(), Some("2"));
        assert_eq!(t.npcs_of(EDRIC).len(), 1);
        // A failed read decides nothing.
        notify(&mut t, SOD_MANAGER, "setSoDState 1");
        t.world.handle(WorldCommand::GlobalVariablesReread {
            token: 2,
            result: Err(()),
        });
        assert_eq!(var(&t, "SoDState").as_deref(), Some("2"));
    }

    #[test]
    fn stage_2_ends_after_its_length() {
        let mut t = ScriptTest::new_with(
            {
                let mut r = ScriptRegistry::new();
                r.add(SeedOfDestructionManager);
                r
            },
            NPCS,
            "",
            &[],
            |world, _| {
                world.set_script_settings(ScriptSettings {
                    cycle_database_writes: true,
                    gracia_seeds: GraciaSeedsSettings {
                        sod_stage_2_length: 1_000,
                        ..GraciaSeedsSettings::default()
                    },
                    ..ScriptSettings::default()
                });
                let now = now_ms().to_string();
                world.set_global_variables(vec![
                    ("SoDState".to_owned(), Some("2".to_owned())),
                    ("SoDTiatKilled".to_owned(), Some("10".to_owned())),
                    ("SoDLSCDate".to_owned(), Some(now)),
                ]);
            },
        );
        assert_eq!(t.npcs_of(EDRIC).len(), 1);
        t.wait_ms(1_200);
        assert_eq!(var(&t, "SoDState").as_deref(), Some("1"));
        assert_eq!(var(&t, "SoDTiatKilled").as_deref(), Some("0"));
        assert!(t.npcs_of(EDRIC).is_empty());
    }

    #[test]
    fn seed_of_infinity_stages_follow_the_kills() {
        let mut t = world(true, vec![]);
        for count in 1..=9 {
            notify(&mut t, SOI_MANAGER, SOI_NOTIFY_COHEMENES_KILL);
            assert_eq!(var(&t, "SoI_CohemenesCount"), Some(count.to_string()));
        }
        // The tenth kill moves the seed to stage 2 and resets the counts.
        notify(&mut t, SOI_MANAGER, SOI_NOTIFY_COHEMENES_KILL);
        assert_eq!(var(&t, "SoI_stage").as_deref(), Some("2"));
        assert_eq!(var(&t, "SoI_CohemenesCount").as_deref(), Some("0"));
        // `EkimusKillCount` 1: one kill counts, the next opens the seed.
        notify(&mut t, SOI_MANAGER, SOI_NOTIFY_EKIMUS_KILL);
        assert_eq!(var(&t, "SoI_EkimusCount").as_deref(), Some("1"));
        notify(&mut t, SOI_MANAGER, SOI_NOTIFY_EKIMUS_KILL);
        assert_eq!(var(&t, "SoI_stage").as_deref(), Some("3"));
        let opened: i64 = var(&t, "SoI_opened").unwrap().parse().unwrap();
        assert!(opened * 1000 > now_ms() + SOI_OPEN_TIME - 5_000);
        // An admin's stage 4 closes it again.
        notify(&mut t, SOI_MANAGER, "setCurrentStage 4");
        assert_eq!(var(&t, "SoI_stage").as_deref(), Some("4"));
        assert_eq!(var(&t, "SoI_opened"), None);
    }

    #[test]
    fn with_the_switch_off_the_seed_of_infinity_waits_for_the_rows() {
        let mut t = world(false, vec![("SoI_stage", "4")]);
        notify(&mut t, SOI_MANAGER, SOI_NOTIFY_HOE_DEF_SUCCESS);
        assert_eq!(t.world.script_rereads_waiting(), 1);
        assert_eq!(var(&t, "SoI_hoedefkillcount"), None);
        // The Java GS is at nine successes: this one moves the seed to stage 5.
        t.world.handle(WorldCommand::GlobalVariablesReread {
            token: 1,
            result: Ok(vec![
                ("SoI_stage".to_owned(), Some("4".to_owned())),
                ("SoI_hoedefkillcount".to_owned(), Some("9".to_owned())),
            ]),
        });
        assert_eq!(var(&t, "SoI_stage").as_deref(), Some("5"));
        assert_eq!(var(&t, "SoI_hoedefkillcount").as_deref(), Some("0"));
    }

    #[test]
    fn the_time_to_the_next_state_change_is_known_in_stage_2_only() {
        let mut t = world(
            true,
            vec![("SoDState", "2"), ("SoDTiatKilled", "0"), ("SoDLSCDate", "1000")],
        );
        t.world.run_as_script(SOD_MANAGER, |ctx| {
            // The stage ended long ago: the start moved it back to 1.
            assert_eq!(sod_state(ctx), 1);
            assert_eq!(sod_time_for_next_state_change(ctx, 0), -1);
        });
    }
}
