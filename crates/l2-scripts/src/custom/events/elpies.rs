//! Elpies event: a hundred Elpies appear near a town for two minutes and drop items.
// Java: dist/game/data/scripts/custom/events/Elpies/Elpies.java
//
// Off by default: the shipped `config.xml` schedules nothing, so the event only runs when a GM
// starts it (`//event_start Elpies`).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, TimerParams};

use super::schedule;

// NPC
const ELPY: i32 = 900_100;

// Amount of Elpies to spawn when the event starts.
const ELPY_AMOUNT: usize = 100;

// Event duration in minutes
const EVENT_DURATION_MINUTES: i64 = 2;

// itemId, chance, min amount, max amount
const DROPLIST_CONSUMABLES: [[i32; 4]; 9] = [
    [1540, 80, 10, 15],  // Quick Healing Potion
    [1538, 60, 5, 10],   // Blessed Scroll of Escape
    [3936, 40, 5, 10],   // Blessed Scroll of Ressurection
    [6387, 25, 5, 10],   // Blessed Scroll of Ressurection Pets
    [22025, 15, 5, 10],  // Powerful Healing Potion
    [6622, 10, 1, 1],    // Giant's Codex
    [20034, 5, 1, 1],    // Revita Pop
    [20004, 1, 1, 1],    // Energy Ginseng
    [20004, 0, 1, 1],    // Energy Ginseng
];

const DROPLIST_CRYSTALS: [[i32; 4]; 5] = [
    [1458, 80, 50, 100], // Crystal D-Grade
    [1459, 60, 40, 80],  // Crystal C-Grade
    [1460, 40, 30, 60],  // Crystal B-Grade
    [1461, 20, 20, 30],  // Crystal A-Grade
    [1462, 0, 10, 20],   // Crystal S-Grade
];

/// Java `EventLocation`: name, min x, max x, min y, max y, z.
struct EventLocation(&'static str, i32, i32, i32, i32, i32);

const LOCATIONS: [EventLocation; 5] = [
    EventLocation("Aden", 146_558, 148_341, 26_622, 28_560, -2200),
    EventLocation("Dion", 18_564, 19_200, 144_377, 145_782, -3081),
    EventLocation("Gludin", -84_040, -81_420, 150_257, 151_175, -3125),
    EventLocation("Hunters Village", 116_094, 117_141, 75_776, 77_072, -2700),
    EventLocation("Oren", 82_048, 82_940, 53_240, 54_126, -1490),
];

/// The Java static and instance fields: `EVENT_ACTIVE`, `_elpies`. (`_eventTask` is the quest
/// timer [`TIME_UP`].)
#[derive(Default)]
struct ElpiesState {
    event_active: bool,
    elpies: Vec<ObjectId>,
}

/// The timer Java's `_eventTask` (`ThreadPool.schedule`) is.
const TIME_UP: &str = "ELPIES_TIME_UP";

/// Java `custom.events.Elpies.Elpies`.
pub struct Elpies;

impl Script for Elpies {
    fn name(&self) -> &'static str {
        "Elpies"
    }

    fn path(&self) -> &'static str {
        "custom/events/Elpies"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_spawn_id(&[ELPY]);
        s.add_kill_id(&[ELPY]);
    }

    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        schedule::load_config(ctx, "Elpies", "Elpies");
    }

    fn is_event(&self) -> bool {
        true
    }

    fn on_timer_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        params: Option<&TimerParams>,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) {
        if schedule::is_schedule(event) {
            self.event_start(ctx, None);
            schedule::reschedule(ctx, event, params);
        }
    }

    fn event_start(&self, ctx: &mut ScriptCtx<'_>, event_maker: Option<ObjectId>) -> bool {
        if ctx.state::<ElpiesState>().event_active {
            return false;
        }

        // Check Custom Table - we use custom NPCs.
        if !ctx.world().custom_script_settings().custom_npc_data {
            tracing::info!("Elpies: Event can't be started because custom NPC table is disabled!");
            // NOTE(java-quirk): Java calls `eventMaker.sendMessage` here, which is a
            // NullPointerException for the scheduled start (no maker); it ends the timer task.
            if let Some(maker) = event_maker {
                ctx.world()
                    .send_message(maker, "Event Elpies can't be started because custom NPC table is disabled!");
            }
            return false;
        }

        ctx.state::<ElpiesState>().event_active = true;

        // `getRandomEntry(EventLocation.values())`.
        let loc = &LOCATIONS[ctx.get_random(LOCATIONS.len() as i32) as usize];
        let despawn_delay = EVENT_DURATION_MINUTES * 60_000;
        for _ in 0..ELPY_AMOUNT {
            let x = ctx.get_random_range(loc.1, loc.2);
            let y = ctx.get_random_range(loc.3, loc.4);
            if let Some(npc) = ctx.add_spawn(ELPY, x, y, loc.5, 0, true, despawn_delay, 0) {
                ctx.state::<ElpiesState>().elpies.push(npc.object_id);
            }
        }

        ctx.broadcast_to_all_online_players("*Squeak Squeak*");
        ctx.broadcast_to_all_online_players(&format!("Elpy invasion in {}", loc.0));
        ctx.broadcast_to_all_online_players("Help us exterminate them!");
        ctx.broadcast_to_all_online_players(&format!("You have {EVENT_DURATION_MINUTES} minutes!"));
        ctx.start_quest_timer(TIME_UP, despawn_delay, None, None, false);
        true
    }

    fn event_stop(&self, ctx: &mut ScriptCtx<'_>) -> bool {
        if !ctx.state::<ElpiesState>().event_active {
            return false;
        }

        ctx.state::<ElpiesState>().event_active = false;
        ctx.cancel_quest_timers(TIME_UP);

        let elpies = std::mem::take(&mut ctx.state::<ElpiesState>().elpies);
        for elpy in elpies {
            if let Some(npc) = ctx.script_npc(elpy) {
                ctx.delete_npc(npc);
            }
        }

        ctx.broadcast_to_all_online_players("*Squeak Squeak*");
        ctx.broadcast_to_all_online_players("Elpy Event finished!");
        true
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        if event == TIME_UP {
            ctx.broadcast_to_all_online_players("Time is up!");
            self.event_stop(ctx);
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if ctx.state::<ElpiesState>().event_active {
            ctx.state::<ElpiesState>().elpies.retain(|e| *e != npc.object_id);

            drop_item(ctx, npc, killer, &DROPLIST_CONSUMABLES);
            drop_item(ctx, npc, killer, &DROPLIST_CRYSTALS);
            if ctx.state::<ElpiesState>().elpies.is_empty() {
                ctx.broadcast_to_all_online_players("All elpies have been killed!");
                self.event_stop(ctx);
            }
        }
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        ctx.world_mut().event_monster_set_drop_on_ground(npc.object_id, true);
        ctx.world_mut().event_monster_set_block_offensive_skills(npc.object_id, true);
    }
}

/// Java `dropItem(mob, player, droplist)`.
fn drop_item(ctx: &mut ScriptCtx<'_>, mob: ScriptNpc, player: ObjectId, droplist: &[[i32; 4]]) {
    let chance = ctx.get_random(100);
    for drop in droplist {
        if chance >= drop[1] {
            let count = ctx.get_random_range(drop[2], drop[3]);
            ctx.drop_item(mob, Some(player), drop[0], i64::from(count));
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;
    use l2_world::custom::CustomScriptSettings;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="900100" type="EventMonster" level="1" name="Elpy"/>"#;

    fn scene(custom_npc_data: bool) -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(Elpies);
        let mut t = ScriptTest::new_with(registry, NPCS, "", &[], |world, _| {
            world.set_custom_script_settings(CustomScriptSettings {
                custom_npc_data,
                ..CustomScriptSettings::default()
            });
        });
        t.enter(1, 40, vec![]);
        t.take();
        t
    }

    fn start(t: &mut ScriptTest) -> bool {
        let mut started = false;
        t.world.run_as_script("Elpies", |ctx| {
            started = Elpies.event_start(ctx, Some(ObjectId(1)));
        });
        started
    }

    #[test]
    fn nothing_starts_by_itself_and_custom_npcs_are_needed() {
        let mut t = scene(false);
        t.ticks(5);
        assert!(t.npcs_of(ELPY).is_empty());
        // The GM start is refused without the custom NPC table.
        assert!(!start(&mut t));
        assert!(t.npcs_of(ELPY).is_empty());
        assert!(!crate::testing::names_to(&t.take(), 1).is_empty());
    }

    #[test]
    fn the_event_spawns_a_hundred_elpies_and_a_kill_drops_loot() {
        let mut t = scene(true);
        assert!(start(&mut t));
        let elpies = t.npcs_of(ELPY);
        assert_eq!(elpies.len(), ELPY_AMOUNT);
        // The second start is refused while it runs.
        assert!(!start(&mut t));
        // The spawn flags are set (`onSpawn`).
        assert!(t.world.event_monster_blocks_skills(elpies[0]));

        // Kills drop on the ground; the last kill ends the event.
        for (i, elpy) in elpies.iter().enumerate() {
            t.kill(1, *elpy);
            if i + 1 < elpies.len() {
                assert!(t.world.has_script("Elpies"));
            }
        }
        // All gone: the event stopped itself and a new start works.
        assert!(start(&mut t));
        let mut stopped = false;
        t.world.run_as_script("Elpies", |ctx| stopped = Elpies.event_stop(ctx));
        assert!(stopped);
        assert!(t.npcs_of(ELPY).iter().all(|n| !t.in_world(*n)));
    }

    #[test]
    fn the_event_ends_after_two_minutes() {
        let mut t = scene(true);
        assert!(start(&mut t));
        t.wait_ms(121_000);
        assert!(t.npcs_of(ELPY).iter().all(|n| !t.in_world(*n)));
        // Stopped: it can start again.
        assert!(start(&mut t));
    }
}
