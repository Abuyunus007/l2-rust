//! Lindvior's visit above Gracia on Tuesday and Friday evenings: the shouts of Tomaris and Artius
//! and the movie.
// Java: dist/game/data/scripts/ai/areas/Gracia/AI/Lindvior.java

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_util::time::local_now_millis;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const LINDVIOR_CAMERA: i32 = 18669;
const TOMARIS: i32 = 32552;
const ARTIUS: i32 = 32559;

const RESET_HOUR: i64 = 18;
const RESET_MIN: i64 = 58;
/// `Calendar.TUESDAY` and `Calendar.FRIDAY` (Sunday is 1).
const RESET_DAY_1: i64 = 3;
const RESET_DAY_2: i64 = 6;

const ALT_MODE: bool = false;
/// Schedule delay in minutes if `ALT_MODE` is enabled.
const ALT_MODE_MIN: i64 = 60;

/// `Movie.SC_LINDVIOR`.
const SC_LINDVIOR: i32 = 1;

const DAY_MS: i64 = 86_400_000;

/// Java `Calendar.get(DAY_OF_WEEK)` (Sunday 1) of a local wall-clock time; the epoch was a Thursday.
fn day_of_week(local_ms: i64) -> i64 {
    (local_ms.div_euclid(DAY_MS) + 4).rem_euclid(7) + 1
}

/// Java `scheduleNextLindviorDate()`, on the local wall clock of this machine (D-48).
// NOTE(java-quirk): Java sets the hour and minute only, so the seconds and milliseconds of "now"
// stay in the date; and a Saturday gets `1 + RESET_DAY_1` days (a Wednesday, not the Tuesday).
// The wait is the wall-clock difference, so a daylight saving change in between is not counted.
fn schedule_next_lindvior_date(local_now_ms: i64) -> i64 {
    let day_start = local_now_ms.div_euclid(DAY_MS) * DAY_MS;
    let mut date = day_start + RESET_HOUR * 3_600_000 + RESET_MIN * 60_000 + local_now_ms.rem_euclid(60_000);
    if local_now_ms >= date {
        date += DAY_MS;
    }

    let day_of_week = day_of_week(date);
    if day_of_week <= RESET_DAY_1 {
        date += (RESET_DAY_1 - day_of_week) * DAY_MS;
    } else if day_of_week <= RESET_DAY_2 {
        date += (RESET_DAY_2 - day_of_week) * DAY_MS;
    } else {
        date += (1 + RESET_DAY_1) * DAY_MS;
    }

    date - local_now_ms
}

/// Java `ai.areas.Gracia.AI.Lindvior`.
pub struct Lindvior;

impl Lindvior {
    /// Java `scheduleNextLindviorVisit()`.
    fn schedule_next_lindvior_visit(ctx: &mut ScriptCtx<'_>) {
        let delay = if ALT_MODE {
            ALT_MODE_MIN * 60000
        } else {
            schedule_next_lindvior_date(local_now_millis())
        };
        ctx.start_quest_timer("start", delay, None, None, false);
    }

    /// Java `SpawnTable.getInstance().getAnySpawn(npcId).getLastSpawn()`: a live NPC of that id.
    fn last_spawn(ctx: &ScriptCtx<'_>, npc_id: i32) -> Option<ScriptNpc> {
        let world = ctx.world();
        world
            .npc_ids()
            .into_iter()
            .find(|id| world.npc(*id).is_some_and(|n| n.npc_id == npc_id && !n.decayed))
            .and_then(|id| ctx.script_npc(id))
    }
}

impl Script for Lindvior {
    fn name(&self) -> &'static str {
        "Lindvior"
    }

    fn path(&self) -> &'static str {
        "ai/areas/Gracia/AI"
    }

    fn setup(&self, _s: &mut ScriptSetup) {}

    // The Java constructor schedules the first visit.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        Self::schedule_next_lindvior_visit(ctx);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "tomaris_shout1" => {
                ctx.broadcast_npc_say(
                    npc?,
                    ChatType::NpcShout,
                    NpcStringId::HuhTheSkyLooksFunnyWhatSThat,
                    &[],
                );
            }
            "artius_shout" => {
                ctx.broadcast_npc_say(
                    npc?,
                    ChatType::NpcShout,
                    NpcStringId::APowerfulSubordinateIsBeingHeldByTheBarrierOrbThisReactionMeans,
                    &[],
                );
            }
            "tomaris_shout2" => {
                ctx.broadcast_npc_say(
                    npc?,
                    ChatType::NpcShout,
                    NpcStringId::BeCarefulSomethingSComing,
                    &[],
                );
            }
            "lindvior_scene" => {
                if let Some(npc) = npc {
                    let players: Vec<ObjectId> = ctx
                        .world()
                        .visible_objects_in_range(npc.object_id, 4000)
                        .into_iter()
                        .filter(|id| ctx.world().player(*id).is_some())
                        .filter(|id| {
                            ctx.world()
                                .position_of(*id)
                                .is_some_and(|(_, _, z)| (1100..=3100).contains(&z))
                        })
                        .collect();
                    for player in players {
                        ctx.play_movie(player, SC_LINDVIOR);
                    }
                }
            }
            "start" => {
                let camera = Self::last_spawn(ctx, LINDVIOR_CAMERA);
                let tomaris = Self::last_spawn(ctx, TOMARIS);
                let artius = Self::last_spawn(ctx, ARTIUS);
                let (Some(camera), Some(tomaris), Some(artius)) = (camera, tomaris, artius) else {
                    // Java `break`s: no visit this time, and none is scheduled either.
                    return None;
                };

                ctx.start_quest_timer("tomaris_shout1", 1000, Some(tomaris), None, false);
                ctx.start_quest_timer("artius_shout", 60000, Some(artius), None, false);
                ctx.start_quest_timer("tomaris_shout2", 90000, Some(tomaris), None, false);
                ctx.start_quest_timer("lindvior_scene", 120000, Some(camera), None, false);
                Self::schedule_next_lindvior_visit(ctx);
            }
            _ => {}
        }

        // `super.onEvent(event, npc, player)`
        None
    }
}

#[cfg(test)]
mod tests {
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::{ScriptTest, names_to};

    const NPCS: &str = r#"<npc id="18669" type="Folk" level="70" name="Camera"/>
        <npc id="32552" type="Folk" level="70" name="Tomaris"/>
        <npc id="32559" type="Folk" level="70" name="Artius"/>"#;

    /// A local wall-clock time `2024-01-01 12:00` (a Monday) plus `days`.
    fn monday_noon(days: i64) -> i64 {
        // 2024-01-01 is day 19723 of the epoch.
        (19723 + days) * DAY_MS + 12 * 3_600_000
    }

    #[test]
    fn the_next_visit_is_the_coming_tuesday_or_friday_at_18_58() {
        let hours = 3_600_000;
        // Monday noon: Tuesday 18:58 is 1 day 6 h 58 min away.
        assert_eq!(
            schedule_next_lindvior_date(monday_noon(0)),
            DAY_MS + 6 * hours + 58 * 60_000
        );
        // Tuesday noon: the same day, 6 h 58 min.
        assert_eq!(
            schedule_next_lindvior_date(monday_noon(1)),
            6 * hours + 58 * 60_000
        );
        // Tuesday 19:00 (after the time): the Friday.
        let tuesday_evening = monday_noon(1) + 7 * hours;
        assert_eq!(
            schedule_next_lindvior_date(tuesday_evening),
            3 * DAY_MS - 60 * 60_000 + 58 * 60_000
        );
        // Wednesday and Thursday: the Friday.
        assert_eq!(
            schedule_next_lindvior_date(monday_noon(2)),
            2 * DAY_MS + 6 * hours + 58 * 60_000
        );
        // Friday after 18:58: Saturday is past `RESET_DAY_2`, so +1 day makes the dow 7 and the
        // quirk adds 4 days (a Wednesday).
        let friday_evening = monday_noon(4) + 7 * hours;
        assert_eq!(
            schedule_next_lindvior_date(friday_evening),
            // Saturday 18:58 + 4 days, less the 19:00 now.
            DAY_MS + 4 * DAY_MS - 2 * 60_000
        );
        // The seconds of now stay in the date (Java only sets hour and minute).
        assert_eq!(
            schedule_next_lindvior_date(monday_noon(0) + 17_000),
            DAY_MS + 6 * hours + 58 * 60_000
        );
    }

    #[test]
    fn the_visit_shouts_and_schedules_the_next_one() {
        let mut registry = ScriptRegistry::new();
        registry.add(Lindvior);
        let mut t = ScriptTest::new(registry, NPCS, "", &[]);
        t.enter(1, 80, vec![]);
        // The load scheduled the first visit.
        t.world.run_as_script("Lindvior", |ctx| {
            assert!(ctx.has_quest_timer("start", None, None));
        });
        // Without the three NPCs nothing happens.
        t.world
            .run_as_script("Lindvior", |ctx| ctx.notify_event("start", None, None));
        t.ticks(10);
        assert!(names_to(&t.take(), 1).iter().all(|n| n != "NPC_SAY"));

        t.spawn(LINDVIOR_CAMERA, 10);
        t.spawn(TOMARIS, 20);
        t.spawn(ARTIUS, 30);
        t.take();
        t.world
            .run_as_script("Lindvior", |ctx| ctx.notify_event("start", None, None));
        // Tomaris after a second, Artius after a minute, Tomaris again after 90 s.
        t.ticks(25);
        assert_eq!(
            names_to(&t.take(), 1).iter().filter(|n| *n == "NPC_SAY").count(),
            1
        );
        t.ticks(1200);
        assert_eq!(
            names_to(&t.take(), 1).iter().filter(|n| *n == "NPC_SAY").count(),
            1
        );
        t.ticks(600);
        assert_eq!(
            names_to(&t.take(), 1).iter().filter(|n| *n == "NPC_SAY").count(),
            1
        );
    }
}
