//! The Altars of Sacrifice: every four hours a boss appears at some altars and is gone four hours
//! later (once it is out of combat).
// Java: dist/game/data/scripts/ai/others/AltarsOfSacrifice.java

use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

const EVT_SPAWN_BOSS_PRE: &str = "spawnboss";
const EVT_DESPAWN_BOSS_PRE: &str = "despawnboss";
const BOSS_MIN_SPAWN_RADIUS: i32 = 250;
const BOSS_MAX_SPAWN_RADIUS: i32 = 500;

// every 240 minutes/4 hours, altars change
const ALTAR_STATE_CHANGE_DELAY: i64 = 240 * 60 * 1000;

/// Java `AltarsOfSacrifice.Altar`: the middle point and the bosses that can appear.
struct Altar {
    middle_point: (i32, i32, i32),
    boss_npc_ids: &'static [i32],
}

const ALTARS: [Altar; 18] = [
    // TalkingIsland
    Altar {
        middle_point: (-92481, 244812, -3505),
        boss_npc_ids: &[],
    },
    // Elven
    Altar {
        middle_point: (40241, 53974, -3262),
        boss_npc_ids: &[],
    },
    // DarkElven
    Altar {
        middle_point: (1851, 21697, -3305),
        boss_npc_ids: &[25750],
    },
    // Dwarven
    Altar {
        middle_point: (130133, -180968, -3271),
        boss_npc_ids: &[25800, 25782],
    },
    // Orc
    Altar {
        middle_point: (-45329, -118327, -166),
        boss_npc_ids: &[25779],
    },
    // Kamael
    Altar {
        middle_point: (-104031, 45059, -1417),
        boss_npc_ids: &[],
    },
    // Oren
    Altar {
        middle_point: (80188, 47037, -3109),
        boss_npc_ids: &[25767, 25770],
    },
    // Gludin
    Altar {
        middle_point: (-86620, 151536, -3018),
        boss_npc_ids: &[25735, 25738, 25741],
    },
    // Gludio
    Altar {
        middle_point: (-14152, 120674, -2935),
        boss_npc_ids: &[25744, 25747],
    },
    // Dion
    Altar {
        middle_point: (16715, 148320, -3210),
        boss_npc_ids: &[25753, 25754, 25757],
    },
    // Heine
    Altar {
        middle_point: (120123, 219164, -3319),
        boss_npc_ids: &[25773, 25776],
    },
    // Giran
    Altar {
        middle_point: (80712, 142538, -3487),
        boss_npc_ids: &[25760, 25763, 25766],
    },
    // Aden
    Altar {
        middle_point: (152720, 24714, -2083),
        boss_npc_ids: &[25793, 25794, 25797],
    },
    // Rune
    Altar {
        middle_point: (28010, -49175, -1278),
        boss_npc_ids: &[],
    },
    // Goddard
    Altar {
        middle_point: (152274, -57706, -3383),
        boss_npc_ids: &[25787, 25790],
    },
    // Schutgart
    Altar {
        middle_point: (82066, -139418, -2220),
        boss_npc_ids: &[25784],
    },
    // Primeval
    Altar {
        middle_point: (10998, -24068, -3603),
        boss_npc_ids: &[],
    },
    // Dragon Valley
    Altar {
        middle_point: (69592, 118694, -3417),
        boss_npc_ids: &[],
    },
];

fn altar(index: usize) -> Option<&'static Altar> {
    ALTARS.get(index)
}

const ALTAR_COUNT: usize = ALTARS.len();

/// The `_spawnedBoss` of each altar (Java `Altar._spawnedBoss`).
#[derive(Default)]
struct Bosses([Option<ScriptNpc>; ALTAR_COUNT]);

impl Altar {
    fn has_bosses(&self) -> bool {
        !self.boss_npc_ids.is_empty()
    }
}

/// Java `Altar.spawnBoss()`: `Err` where Java throws `IllegalStateException`.
fn spawn_boss(ctx: &mut ScriptCtx<'_>, index: usize, altar: &Altar) -> Result<(), ()> {
    if !altar.has_bosses() || ctx.state::<Bosses>().0[index].is_some() {
        return Err(());
    }

    let npc_id = ctx.get_random_entry(altar.boss_npc_ids).ok_or(())?;
    let heading = ctx.get_random(65536);
    let radius = ctx.get_random_range(BOSS_MIN_SPAWN_RADIUS, BOSS_MAX_SPAWN_RADIUS);
    let angle_radians = ctx.next_double() * 2.0 * std::f64::consts::PI;
    let spawn_x = (f64::from(radius) * angle_radians.cos()) as i32 + altar.middle_point.0;
    let spawn_y = (f64::from(radius) * angle_radians.sin()) as i32 + altar.middle_point.1;
    let spawn_z = ctx.geo_height(spawn_x, spawn_y, altar.middle_point.2);
    // NOTE(java-quirk): `spawn.doSpawn(false)` may return null (a bad template); Java then keeps
    // `_spawnedBoss` null and the next spawn event tries again.
    let boss = ctx.add_spawn(npc_id, spawn_x, spawn_y, spawn_z, heading, false, 0, 0);
    ctx.state::<Bosses>().0[index] = boss;
    Ok(())
}

/// Java `Altar.despawnBoss()`.
fn despawn_boss(ctx: &mut ScriptCtx<'_>, index: usize) {
    if let Some(boss) = ctx.state::<Bosses>().0[index].take() {
        ctx.delete_npc(boss);
    }
}

/// Java `Altar.isBossFighting()`.
fn is_boss_fighting(ctx: &mut ScriptCtx<'_>, index: usize) -> bool {
    match ctx.state::<Bosses>().0[index] {
        Some(boss) => ctx.is_in_combat(boss.object_id),
        None => false,
    }
}

/// Java `getSpawnBossIndex(event)` / `getDespawnBossIndex(event)`.
fn index_of(event: &str, prefix: &str) -> Option<usize> {
    // NOTE(java-quirk): `Integer.parseInt` throws on a bad number; the event is dropped.
    event.strip_prefix(prefix)?.parse().ok()
}

/// Java `ai.others.AltarsOfSacrifice`.
pub struct AltarsOfSacrifice;

impl Script for AltarsOfSacrifice {
    fn name(&self) -> &'static str {
        "AltarsOfSacrifice"
    }

    fn path(&self) -> &'static str {
        "ai/others"
    }

    fn setup(&self, _s: &mut ScriptSetup) {}

    // The Java constructor starts the first spawn timers; they need the running world.
    fn on_load(&self, ctx: &mut ScriptCtx<'_>) {
        for index in 0..ALTAR_COUNT {
            if altar(index).is_some_and(Altar::has_bosses) {
                ctx.start_quest_timer(
                    &format!("{EVT_SPAWN_BOSS_PRE}{index}"),
                    ALTAR_STATE_CHANGE_DELAY,
                    None,
                    None,
                    false,
                );
            }
        }
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<l2_util::ObjectId>,
    ) -> Option<String> {
        if event.starts_with(EVT_SPAWN_BOSS_PRE) {
            let index = index_of(event, EVT_SPAWN_BOSS_PRE)?;
            let altar = altar(index)?;
            match spawn_boss(ctx, index, altar) {
                Ok(()) => {
                    ctx.start_quest_timer(
                        &format!("{EVT_DESPAWN_BOSS_PRE}{index}"),
                        ALTAR_STATE_CHANGE_DELAY,
                        None,
                        None,
                        false,
                    );
                }
                Err(()) => {
                    // let's try again to spawn it in 5 seconds
                    ctx.start_quest_timer(event, 5000, None, None, false);
                }
            }
        } else if event.starts_with(EVT_DESPAWN_BOSS_PRE) {
            let index = index_of(event, EVT_DESPAWN_BOSS_PRE)?;
            altar(index)?;
            if is_boss_fighting(ctx, index) {
                // periodically check if the altar boss is fighting, only despawn when not fighting anymore
                ctx.start_quest_timer(event, 5000, None, None, false);
            } else {
                despawn_boss(ctx, index);
                ctx.start_quest_timer(
                    &format!("{EVT_SPAWN_BOSS_PRE}{index}"),
                    ALTAR_STATE_CHANGE_DELAY,
                    None,
                    None,
                    false,
                );
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId;
    use l2_world::ScriptRegistry;

    use super::*;
    use crate::testing::ScriptTest;

    const NPCS: &str = r#"<npc id="25750" type="RaidBoss" level="75" name="Altar Boss"><stats><vitals hp="1000" mp="100"/></stats></npc>"#;
    const FOUR_HOURS_TICKS: u32 = 4 * 3600 * 20;

    fn bosses(t: &ScriptTest) -> Vec<ObjectId> {
        t.world
            .npc_ids()
            .into_iter()
            .filter(|id| t.world.npc(*id).unwrap().npc_id == 25750)
            .collect()
    }

    fn setup() -> ScriptTest {
        let mut registry = ScriptRegistry::new();
        registry.add(AltarsOfSacrifice);
        ScriptTest::new(registry, NPCS, "", &[])
    }

    #[test]
    fn the_altar_table_is_javas() {
        assert_eq!(ALTARS.len(), 18);
        assert_eq!(altar(2).unwrap().boss_npc_ids, &[25750]);
        assert_eq!(altar(17).unwrap().middle_point, (69592, 118694, -3417));
        assert!(altar(18).is_none());
        // Twelve altars have bosses and start a timer.
        assert_eq!(
            (0..ALTAR_COUNT)
                .filter(|i| altar(*i).unwrap().has_bosses())
                .count(),
            12
        );
        assert_eq!(index_of("spawnboss12", EVT_SPAWN_BOSS_PRE), Some(12));
        assert_eq!(index_of("spawnbossx", EVT_SPAWN_BOSS_PRE), None);
    }

    #[test]
    fn a_boss_appears_after_four_hours_near_the_altar_and_leaves_four_hours_later() {
        let mut t = setup();
        // One start timer per altar with bosses (`on_load` runs when the scripts are installed).
        assert_eq!(t.world.quest_timer_count(), 12);
        assert!(bosses(&t).is_empty());

        t.ticks(FOUR_HOURS_TICKS + 2);
        let spawned = bosses(&t);
        assert_eq!(spawned.len(), 1, "only the DarkElven boss has a template here");
        let (x, y, _) = t.world.position_of(spawned[0]).unwrap();
        let (dx, dy) = (f64::from(x - 1851), f64::from(y - 21697));
        let distance = (dx * dx + dy * dy).sqrt();
        assert!((249.0..=501.0).contains(&distance), "{distance}");

        t.ticks(FOUR_HOURS_TICKS);
        assert!(bosses(&t).is_empty(), "gone after the next four hours");
    }
}
