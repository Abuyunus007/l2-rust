//! The `<schedule pattern>` loop every custom event script has (`loadConfig` and the
//! `Schedule*` branch of `onTimerEvent`).
// Java: dist/game/data/scripts/custom/events/Elpies/Elpies.java (`loadConfig`, `onTimerEvent`)
//
// The patterns come from the `config.xml` of the event folder (the game server reads them into
// `CustomScriptSettings::event_schedules`, keyed by the event's name). The shipped files have
// every `<schedule>` commented out, so nothing is scheduled by default.

use l2_world::{ScriptCtx, TimerParams};
use tracing::info;

/// Java `loadConfig()`: one `Schedule<n>` timer per `<schedule>` of the event, `delay + 5000` ms
/// ("Added 5 seconds to prevent overlapping"). `key` is the event's key in the settings, `name`
/// the `name` attribute of its `<event>` element.
pub(super) fn load_config(ctx: &mut ScriptCtx<'_>, key: &str, name: &str) {
    let patterns = ctx
        .world()
        .custom_script_settings()
        .event_schedules
        .get(key)
        .cloned()
        .unwrap_or_default();
    for (index, pattern) in patterns.iter().enumerate() {
        let Some(delay) = ctx.world().scheduling_delay_to_next(pattern) else {
            continue;
        };
        let params = TimerParams::new()
            .set("Name", name)
            .set("SchedulingPattern", pattern);
        ctx.add_timer(
            &format!("Schedule{}", index + 1),
            Some(params),
            delay + 5000,
            None,
            None,
        );
        info!("Event {name} scheduled in {} s", delay / 1000);
    }
}

/// Java `event.startsWith("Schedule")`.
pub(super) fn is_schedule(event: &str) -> bool {
    event.starts_with("Schedule")
}

/// The part of Java's `onTimerEvent` after `eventStart(null)`: the same timer again for the next
/// match of the pattern.
pub(super) fn reschedule(ctx: &mut ScriptCtx<'_>, event: &str, params: Option<&TimerParams>) {
    let Some(params) = params else {
        return;
    };
    let pattern = params.get_string("SchedulingPattern");
    let Some(delay) = ctx.world().scheduling_delay_to_next(&pattern) else {
        return;
    };
    ctx.add_timer(event, Some(params.clone()), delay + 5000, None, None);
    info!(
        "Event {} scheduled in {} s",
        params.get_string("Name"),
        delay / 1000
    );
}
