//! `data/scripts/ai/bosses`: the grand and raid boss AIs.
//!
//! One module per script, sorted by name; one `add` line per script in [`register`], in the
//! same order (Java's load order is the folder listing).
//!
//! # Side by side (D-53, D-54)
//! The grand boss state rows (`grandboss_data`) belong to the Java game server while both run on
//! one database. The managers (`l2_world::boss`) write them only with `CycleDatabaseWrites`; the
//! scripts add the D-54 rule on top: with the switch off a grand boss AI only runs while the row
//! says the boss is alive (status 0). [`row_lets_boss_run`] is that test, and a respawn timer reads
//! the row again before it brings the boss back ([`World::reread_grand_boss_for_script`]).
//!
//! [`World::reread_grand_boss_for_script`]: l2_world::World::reread_grand_boss_for_script

pub mod antharas;
pub mod baium;
pub mod core;
pub mod orfen;
pub mod valakas;

use l2_world::{ScriptCtx, ScriptRegistry};

/// Registers every script of the folder.
pub(crate) fn register(r: &mut ScriptRegistry) {
    r.add(antharas::Antharas);
    r.add(baium::Baium);
    r.add(core::Core);
    r.add(orfen::Orfen);
    r.add(valakas::Valakas);
}

/// The event a boss script gets once its row was read again (D-54). Not a Java event: Java has
/// no shared database and decides on its own memory.
pub(crate) const EVENT_ROW_READ: &str = "D54_ROW_READ";

/// D-54: with `CycleDatabaseWrites` on, every state of the row counts (Java). With it off the
/// boss AI runs only while the row says the boss is alive (status 0); a boss without a row never
/// runs.
pub(crate) fn row_lets_boss_run(ctx: &ScriptCtx<'_>, boss_id: i32) -> bool {
    ctx.world().boss_writes_enabled() || ctx.world().grand_boss_may_spawn(boss_id)
}

/// Java `TimeUtil.getDateTimeString(millis)`: `dd/MM/yyyy HH:mm:ss` in the machine's zone (D-48).
pub(crate) fn date_time_string(epoch_ms: i64) -> String {
    let local = epoch_ms + l2_util::time::local_offset_millis(epoch_ms);
    let days = local.div_euclid(86_400_000);
    let ms_of_day = local.rem_euclid(86_400_000);
    let (hour, minute, second) = (
        ms_of_day / 3_600_000,
        (ms_of_day / 60_000) % 60,
        (ms_of_day / 1000) % 60,
    );
    // Days since 1970-01-01 to the civil date (H. Hinnant's algorithm).
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = yoe + era * 400 + i64::from(month <= 2);
    format!("{day:02}/{month:02}/{year:04} {hour:02}:{minute:02}:{second:02}")
}

/// Java `LocationUtil.calculateAngleFrom(origin, target)`: degrees from 0 to 360.
pub(crate) fn angle_from(origin: (i32, i32, i32), target: (i32, i32, i32)) -> f64 {
    let mut angle = f64::from(target.1 - origin.1)
        .atan2(f64::from(target.0 - origin.0))
        .to_degrees();
    if angle < 0.0 {
        angle += 360.0;
    }
    angle
}

/// Java `calculateDistance3D`.
pub(crate) fn distance_3d(a: (i32, i32, i32), b: (i32, i32, i32)) -> f64 {
    let (dx, dy, dz) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1), f64::from(a.2 - b.2));
    (dx * dx + dy * dy + dz * dz).sqrt()
}

/// What the boss tests share: the real zones of the three lairs, the boss rows and the reads.
#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, dead_code)]
pub(crate) mod test_support {
    use std::sync::Arc;

    use l2_data::{MapRegions, Zones};
    use l2_util::ObjectId;
    use l2_world::*;

    use crate::testing::{ScriptTest, player_data};

    /// The zones the scripts name, with the shapes of `data/zones/no_restart.xml` and
    /// `custom_boss.xml`.
    pub const ZONES: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list enabled="true">
	<zone name="antaras_no_restart" id="70050" type="NoRestartZone" shape="NPoly" minZ="-8380" maxZ="-4880">
		<stat name="restartTime" val="1800" />
		<stat name="restartAllowedTime" val="600" />
		<node X="173469" Y="110431" />
		<node X="188081" Y="110284" />
		<node X="188132" Y="119249" />
		<node X="173386" Y="119391" />
	</zone>
	<zone name="baium_no_restart" id="70051" type="NoRestartZone" shape="NPoly" minZ="10061" maxZ="11061">
		<stat name="restartTime" val="1800" />
		<stat name="restartAllowedTime" val="600" />
		<node X="111074" Y="16177" />
		<node X="114562" Y="12623" />
		<node X="118194" Y="15790" />
		<node X="114825" Y="19589" />
	</zone>
	<zone name="Valakas Boss" id="12010" type="BossZone" shape="NPoly" minZ="-2572" maxZ="5428">
		<stat name="InvadeTime" val="1800000" />
		<node X="196866" Y="-130845" />
		<node X="229289" Y="-130860" />
		<node X="229176" Y="-98508" />
		<node X="196880" Y="-98498" />
	</zone>
</list>
"#;

    pub fn now_ms() -> i64 {
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| i64::try_from(d.as_millis()).unwrap())
            .unwrap()
    }

    /// A `grandboss_data` row at the given place.
    pub fn row(boss_id: i32, status: i32, respawn_time: i64, (x, y, z): (i32, i32, i32)) -> GrandBossData {
        GrandBossData {
            boss_id,
            loc_x: x,
            loc_y: y,
            loc_z: z,
            heading: 0,
            respawn_time,
            current_hp: 1000.0,
            current_mp: 500.0,
            status,
        }
    }

    /// A world with the zones, the boss rows (`CycleDatabaseWrites` = `writes`) and the scripts.
    pub fn boss_test(
        registry: ScriptRegistry,
        npcs: &str,
        items: &str,
        pages: &[(&str, &str)],
        rows: Vec<GrandBossData>,
        writes: bool,
    ) -> ScriptTest {
        let mut pages: Vec<(&str, &str)> = pages.to_vec();
        pages.push(("data/zones/bosses.xml", ZONES));
        ScriptTest::new_with(registry, npcs, items, &pages, |world, root| {
            world.set_zone_data(Arc::new(Zones::load(root)), Arc::new(MapRegions::load(root)));
            world.set_boss_settings(BossSettings {
                cycle_database_writes: writes,
                ..BossSettings::default()
            });
            world.load_bosses(BossLoad {
                grand_bosses: rows,
                ..BossLoad::default()
            });
        })
    }

    /// A character standing at `at`.
    pub fn player_at(id: i32, at: (i32, i32, i32), items: Vec<ItemData>) -> PlayerData {
        let mut data = player_data(id, 80, items);
        (data.x, data.y, data.z) = at;
        data
    }

    /// Runs the world for `ms` milliseconds (and one tick more).
    pub fn run_ms(t: &mut ScriptTest, ms: u32) {
        t.ticks(ms / 50 + 1);
    }

    /// The grand boss writes among the recorded commands.
    pub fn grand_writes(commands: &[PersistCommand]) -> Vec<BossWrite> {
        commands
            .iter()
            .filter_map(|c| match c {
                PersistCommand::Boss(
                    w @ (BossWrite::GrandBossStatus { .. }
                    | BossWrite::GrandBoss(_)
                    | BossWrite::GrandBossStoreAll { .. }),
                ) => Some(w.clone()),
                _ => None,
            })
            .collect()
    }

    /// The re-read requests among the recorded commands.
    pub fn rereads(commands: &[PersistCommand]) -> Vec<(BossReadKind, BossReadThen)> {
        commands
            .iter()
            .filter_map(|c| match c {
                PersistCommand::BossRereadRequest { kind, then } => Some((*kind, *then)),
                _ => None,
            })
            .collect()
    }

    /// The persistence task answers a re-read with `row`.
    pub fn answer(t: &mut ScriptTest, then: BossReadThen, boss_id: i32, row: GrandBossData) {
        t.world.handle(WorldCommand::BossReread {
            result: Ok(BossRereadRows::GrandBoss {
                boss_id,
                row: Some(row),
            }),
            then,
        });
    }

    /// The living NPCs of a template in the world (a corpse stays until it decays).
    pub fn npcs_of(t: &ScriptTest, npc_id: i32) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = t
            .world
            .npc_ids()
            .into_iter()
            .filter(|id| t.world.npc(*id).is_some_and(|n| n.npc_id == npc_id) && !t.world.is_dead(*id))
            .collect();
        ids.sort_by_key(|id| id.0);
        ids
    }

    /// The client loaded the teleport target.
    pub fn appear(t: &mut ScriptTest, player: i32) {
        t.input(player, ClientInput::Appearing);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_date_string_is_java_s_pattern() {
        let text = date_time_string(0);
        // `dd/MM/yyyy HH:mm:ss`, whatever the zone of the machine.
        assert_eq!(text.len(), 19);
        assert_eq!(&text[2..3], "/");
        assert_eq!(&text[5..6], "/");
        let offset = l2_util::time::local_offset_millis(0);
        if offset == 0 {
            assert_eq!(text, "01/01/1970 00:00:00");
        }
        if l2_util::time::local_offset_millis(1_700_000_000_000) == 0 {
            assert_eq!(date_time_string(1_700_000_000_000), "14/11/2023 22:13:20");
        }
    }

    #[test]
    fn the_angle_runs_from_0_to_360() {
        assert_eq!(angle_from((0, 0, 0), (10, 0, 0)), 0.0);
        assert!((angle_from((0, 0, 0), (0, 10, 0)) - 90.0).abs() < 1e-9);
        assert!((angle_from((0, 0, 0), (0, -10, 0)) - 270.0).abs() < 1e-9);
    }
}
