//! `//gracia_seeds` and its words (S-API-05): the Seed of Destruction and Seed of Infinity states
//! for a GM, and the switches of the two seed managers.
// Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminGraciaSeeds.java
//
// Java's seed managers are core singletons; here they are the scripts `SeedOfDestructionManager`
// and `SeedOfInfinityManager` (`l2-scripts`, `ai/areas/gracia/seeds.rs`), which keep their state
// in the global variables. The words hand the change to the manager as its event
// (`increaseSoDTiatKilled`, `setSoDState <n>`, `setCurrentStage <n>`); the manager re-reads its
// rows first (D-54) and decides after the command, so the menu shown at once has the values of
// before the change (Java's has the new ones). Without the manager scripts (`NoQuests`) the
// words change nothing.

use l2_util::ObjectId;

use crate::admin::{AdminHandlers, JavaException};
use crate::world::World;

type Outcome = Result<(), JavaException>;

const SOD_MANAGER: &str = "SeedOfDestructionManager";
const SOI_MANAGER: &str = "SeedOfInfinityManager";

/// Java `AdminGraciaSeeds.ADMIN_COMMANDS`.
pub(crate) fn register(handlers: &mut AdminHandlers) {
    handlers.register("admin_gracia_seeds", admin_gracia_seeds);
    handlers.register("admin_kill_tiat", admin_gracia_seeds);
    handlers.register("admin_set_sodstate", admin_gracia_seeds);
    handlers.register("admin_set_soistage", admin_gracia_seeds);
}

/// Java `AdminGraciaSeeds.onCommand`.
fn admin_gracia_seeds(world: &mut World, gm: ObjectId, command: &str) -> Outcome {
    let mut tokens = command.split(' ').filter(|t| !t.is_empty());
    let actual_command = tokens.next().unwrap_or("");
    let val = tokens.next().unwrap_or("");
    let parse = |val: &str| val.parse::<i32>().map_err(|_| JavaException::number_format(val));
    if actual_command.eq_ignore_ascii_case("admin_kill_tiat") {
        world.notify_seed_manager(SOD_MANAGER, "increaseSoDTiatKilled");
    } else if actual_command.eq_ignore_ascii_case("admin_set_sodstate") {
        let value = parse(val)?;
        world.notify_seed_manager(SOD_MANAGER, &format!("setSoDState {value}"));
    } else if actual_command.eq_ignore_ascii_case("admin_set_soistage") {
        let value = parse(val)?;
        world.notify_seed_manager(SOI_MANAGER, &format!("setCurrentStage {value}"));
    }
    show_menu(world, gm);
    Ok(())
}

/// Java `AdminGraciaSeeds.showMenu`.
fn show_menu(world: &mut World, gm: ObjectId) {
    let globals = &world.scripts.globals;
    let sod_state = globals.get_int("SoDState", 1);
    let tiat_killed = globals.get_int("SoDTiatKilled", 0);
    let last_change = globals.get_long("SoDLSCDate", 0);
    let soi_stage = globals.get_int("SoI_stage", 1);
    let cohemenes = globals.get_int("SoI_CohemenesCount", 0);
    let ekimus = globals.get_int("SoI_EkimusCount", 0);
    let now = crate::world::unix_ms_now();
    // `getSoDTimeForNextStateChange()`: only stage 2 has an end.
    let next_change = match sod_state {
        // Java's `long` arithmetic wraps.
        2 => last_change
            .wrapping_add(world.scripts.settings.gracia_seeds.sod_stage_2_length)
            .wrapping_sub(now),
        _ => -1,
    };
    let mut page = world.admin_html_file(gm, "data/html/admin/graciaseeds.htm");
    // Seed of destruction
    page.replace("%sodstage%", &sod_state.to_string());
    page.replace("%sodtiatkill%", &tiat_killed.to_string());
    if next_change > 0 {
        page.replace(
            "%sodtime%",
            &crate::boss::java_date(now.wrapping_add(next_change)),
        );
    } else {
        page.replace("%sodtime%", "-1");
    }
    // Seed of infinity
    page.replace("%soistage%", &soi_stage.to_string());
    page.replace("%twinkills%", "N/A");
    page.replace("%cohemeneskills%", &cohemenes.to_string());
    page.replace("%ekimuskills%", &ekimus.to_string());
    page.replace("%soitime%", "N/A");
    world.send_admin_html(gm, &page);
}

impl World {
    /// The event of a seed manager script (`ctx.notify_event_of(manager, event, None, None)`);
    /// nothing when it is not installed.
    fn notify_seed_manager(&mut self, manager: &str, event: &str) {
        if let Some(script) = self.scripts.by_name(manager) {
            self.script_notify_event(script, event, None, None);
            self.script_drain();
        }
    }
}
