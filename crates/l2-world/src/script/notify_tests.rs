//! Unit tests of hooks that need no world data (S-API-03).

use std::sync::{Arc, Mutex};

use l2_util::WorldRng;

use super::{Script, ScriptCtx, ScriptRegistry, ScriptSetup};
use crate::outbox::{LsEvent, Outbox, PersistCommand};
use crate::world::World;

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Clock(Arc<Mutex<Vec<bool>>>);

impl Script for Clock {
    fn name(&self) -> &'static str {
        "Clock"
    }
    fn path(&self) -> &'static str {
        "ai/others/Clock"
    }
    fn setup(&self, s: &mut ScriptSetup) {
        s.set_on_day_night_change();
    }
    fn on_day_night_change(&self, _ctx: &mut ScriptCtx<'_>, is_night: bool) {
        self.0.lock().unwrap_or_else(|e| e.into_inner()).push(is_night);
    }
}

/// Java `GameTimeTaskManager`: `ON_DAY_NIGHT_CHANGE` is queued (async) and carries the new state.
#[test]
fn day_night_change_reaches_the_listeners_after_the_drain() {
    let heard = Arc::new(Mutex::new(Vec::new()));
    let mut world = World::new(WorldRng::from_seed(1), Box::new(NoOutbox));
    let mut registry = ScriptRegistry::new();
    registry.add(Clock(Arc::clone(&heard)));
    world.install_scripts(registry);
    world.script_on_day_night_change(true);
    assert!(heard.lock().unwrap_or_else(|e| e.into_inner()).is_empty());
    world.script_drain();
    world.script_on_day_night_change(false);
    world.script_drain();
    assert_eq!(*heard.lock().unwrap_or_else(|e| e.into_inner()), [true, false]);
}
