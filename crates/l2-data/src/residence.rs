//! Residence definitions: what the datapack says about castles, fortresses and clan halls. The
//! residences themselves (owners, tax, treasury, functions) are rows of the database and live in
//! the world (`l2_world::residence`); this module only holds the immutable parts.
//!
//! Java: `managers/CastleManager.java` (`_castleCirclets`), `data/xml/SiegeScheduleData.java`,
//! `mechanics/siege/SiegeScheduleDate.java`, and the zone lookups of `mechanics/siege/Castle.java`
//! (`getZone`, `initResidenceZone`, `getTeleZone`), `mechanics/siege/Fort.java` (`getZone`,
//! `initResidenceZone`) and `entity/zone/type/ClanHallZone.java` (`setParameter`, which registers
//! the zone with its hall). Data: `config/SiegeSchedule.xml` (`xsd/SiegeSchedule.xsd`) and the
//! residence zones of `data/zones/*.xml`.
//!
//! # Zones
//! Java finds a residence's zones by their `<stat>` parameters. The lookups here return the zone
//! the Java lookup returns:
//! - the siege zone: the first `SiegeZone` of the class map whose siege object (`castleId`,
//!   `fortId` or `clanHallId`, whichever the zone names first) is the residence id;
//! - the castle or fortress zone: the first `CastleZone` (`castleId`) / `FortZone` (`fortId`);
//! - the castle's teleport zone: the first `ResidenceTeleportZone` (exact class) with
//!   `residenceId`;
//! - the clan hall zone: the *last created* `ClanHallZone` or `SiegableHallZone` with
//!   `clanHallId`, because each such zone registers itself with the hall while it is parsed (the
//!   shipped `clan_hall.xml` has two zones for the halls 26 to 30; the second one wins).

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::loader::{DataError, load_file, log_loaded};
use crate::zone::{Zone, Zones};

/// `CastleManager._castleCirclets`: the circlet item of each castle id (index 0 is no castle).
pub const CASTLE_CIRCLETS: [i32; 10] = [0, 6838, 6835, 6839, 6837, 6840, 6834, 6836, 8182, 8183];

/// Java `CastleManager.getCircletByCastleId(castleId)`: 0 outside 1 to 9.
pub fn circlet_by_castle_id(castle_id: i32) -> i32 {
    match usize::try_from(castle_id) {
        Ok(index @ 1..=9) => CASTLE_CIRCLETS[index],
        _ => 0,
    }
}

/// Java `CastleManager.getCirclet()`: the circlet of castle 1.
pub fn circlet() -> i32 {
    circlet_by_castle_id(1)
}

// ------------------------------------------------------------------------------------- zones

/// Java `SiegeZone.getSiegeObjectId()`: the first of `castleId`, `fortId` and `clanHallId` the
/// zone names (Java throws "Siege object already defined!" for a second one), -1 for none.
pub fn siege_object_id(zone: &Zone) -> i32 {
    zone.params
        .iter()
        .find(|(name, _)| matches!(name.as_str(), "castleId" | "fortId" | "clanHallId"))
        .and_then(|(_, value)| value.trim().parse().ok())
        .unwrap_or(-1)
}

/// Java `Castle.getZone()` / `Fort.getZone()`: the siege zone of a castle or fortress.
pub fn siege_zone_of(zones: &Zones, residence_id: i32) -> Option<&Zone> {
    zones
        .all_of_type("SiegeZone")
        .into_iter()
        .find(|zone| siege_object_id(zone) == residence_id)
}

/// Java `Castle.initResidenceZone()`: the `CastleZone` of a castle.
pub fn castle_zone_of(zones: &Zones, castle_id: i32) -> Option<&Zone> {
    first_with(zones, "CastleZone", "castleId", castle_id)
}

/// Java `Fort.initResidenceZone()`: the `FortZone` of a fortress.
pub fn fort_zone_of(zones: &Zones, fort_id: i32) -> Option<&Zone> {
    first_with(zones, "FortZone", "fortId", fort_id)
}

/// Java `Castle.getTeleZone()`: the `ResidenceTeleportZone` of a residence (exact class; the
/// clan hall teleport zones are `ResidenceHallTeleportZone`s and not found here, like in Java).
pub fn residence_teleport_zone_of(zones: &Zones, residence_id: i32) -> Option<&Zone> {
    first_with(zones, "ResidenceTeleportZone", "residenceId", residence_id)
}

/// Java `ClanHallZone.setParameter("clanHallId")`: the zone a clan hall ends up with, the last
/// created `ClanHallZone` (or `SiegableHallZone`) that names it.
pub fn clan_hall_zone_of(zones: &Zones, hall_id: i32) -> Option<&Zone> {
    zones
        .created()
        .filter(|zone| zone.is_a("ClanHallZone"))
        .filter(|zone| zone.int_param("clanHallId") == Some(hall_id))
        .last()
}

/// The first zone of exactly `zone_type` (class map order) whose `param` is `value`.
fn first_with<'a>(zones: &'a Zones, zone_type: &str, param: &str, value: i32) -> Option<&'a Zone> {
    zones
        .all_of_type(zone_type)
        .into_iter()
        .find(|zone| zone.int_param(param) == Some(value))
}

// ----------------------------------------------------------------------------- siege schedule

/// `java.util.Calendar.SUNDAY`.
pub const CALENDAR_SUNDAY: i32 = 1;

/// One `<schedule>`: Java `SiegeScheduleDate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SiegeScheduleDate {
    /// `Calendar.DAY_OF_WEEK` (1 Sunday .. 7 Saturday; a name Java cannot resolve is -1).
    pub day: i32,
    pub hour: i32,
    pub max_concurrent: i32,
    pub siege_enabled: bool,
}

impl Default for SiegeScheduleDate {
    /// Java's `StatSet` defaults (`day` Sunday, `hour` 16, `maxConcurrent` 5, disabled).
    fn default() -> Self {
        Self {
            day: CALENDAR_SUNDAY,
            hour: 16,
            max_concurrent: 5,
            siege_enabled: false,
        }
    }
}

/// `config/SiegeSchedule.xml`: Java `SiegeScheduleData`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SiegeSchedule {
    /// `_scheduleData` by castle id (a later entry for the same castle replaces the earlier one).
    schedules: BTreeMap<i32, SiegeScheduleDate>,
}

impl SiegeSchedule {
    const LOADER: &'static str = "SiegeScheduleData";

    /// Loads `<root>/config/SiegeSchedule.xml` (Java `parseDatapackFile`, relative to the
    /// datapack root) and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let mut table = Self::default();
        load_file(&root.join("config/SiegeSchedule.xml"), |doc: RawList| {
            table.add(doc);
            Ok(())
        });
        log_loaded(Self::LOADER, table.len(), "siege schedulers");
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add(doc);
        Ok(table)
    }

    fn add(&mut self, doc: RawList) {
        for raw in doc.schedules {
            match raw.parse() {
                Ok((castle_id, date)) => {
                    self.schedules.insert(castle_id, date);
                }
                // NOTE(java-quirk): Java's `StatSet.getInt` throws on a value that is not a number
                // and the rest of the file is lost; here only that entry is skipped.
                Err(e) => warn!("{}: {e}", Self::LOADER),
            }
        }
    }

    /// Java `getScheduleDateForCastleId(castleId)`.
    pub fn for_castle(&self, castle_id: i32) -> Option<&SiegeScheduleDate> {
        self.schedules.get(&castle_id)
    }

    pub fn len(&self) -> usize {
        self.schedules.len()
    }

    pub fn is_empty(&self) -> bool {
        self.schedules.is_empty()
    }
}

/// Java `Calendar.class.getField(name).getInt(...)` for the day names, -1 (with Java's warning)
/// for anything else.
fn calendar_day(name: &str) -> i32 {
    match name {
        "SUNDAY" => 1,
        "MONDAY" => 2,
        "TUESDAY" => 3,
        "WEDNESDAY" => 4,
        "THURSDAY" => 5,
        "FRIDAY" => 6,
        "SATURDAY" => 7,
        other => {
            warn!("SiegeScheduleData: Could not get value for field {other}.");
            -1
        }
    }
}

#[derive(Debug, Deserialize)]
struct RawList {
    #[serde(rename = "schedule", default)]
    schedules: Vec<RawSchedule>,
}

#[derive(Debug, Deserialize)]
struct RawSchedule {
    #[serde(rename = "@castleId")]
    castle_id: Option<String>,
    #[serde(rename = "@siegeEnabled")]
    siege_enabled: Option<String>,
    #[serde(rename = "@day")]
    day: Option<String>,
    #[serde(rename = "@hour")]
    hour: Option<String>,
    #[serde(rename = "@maxConcurrent")]
    max_concurrent: Option<String>,
}

impl RawSchedule {
    fn parse(self) -> Result<(i32, SiegeScheduleDate), String> {
        let int = |value: &Option<String>, default: Option<i32>, name: &str| match value {
            Some(text) => text
                .trim()
                .parse::<i32>()
                .map_err(|_| format!("Integer value required, but found: {text} ({name})")),
            None => default.ok_or_else(|| format!("Integer value required, but not specified ({name})")),
        };
        let defaults = SiegeScheduleDate::default();
        // "day" that is not numeric is a `Calendar` field name.
        let day = match &self.day {
            Some(text) if !is_numeric(text) => calendar_day(text),
            other => int(other, Some(defaults.day), "day")?,
        };
        let date = SiegeScheduleDate {
            day,
            hour: int(&self.hour, Some(defaults.hour), "hour")?,
            max_concurrent: int(
                &self.max_concurrent,
                Some(defaults.max_concurrent),
                "maxConcurrent",
            )?,
            // `Boolean.parseBoolean`.
            siege_enabled: self
                .siege_enabled
                .as_deref()
                .map_or(defaults.siege_enabled, |v| v.eq_ignore_ascii_case("true")),
        };
        Ok((int(&self.castle_id, None, "castleId")?, date))
    }
}

/// Java `StringUtil.isNumeric`: only ASCII digits, and at least one.
fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The shipped `config/SiegeSchedule.xml`, first two lines.
    const SCHEDULE: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="../data/xsd/SiegeSchedule.xsd">
	<schedule castleId="1" castleName="Gludio" siegeEnabled="true" day="SUNDAY" hour="16" maxConcurrent="5" />
	<schedule castleId="2" castleName="Dion" siegeEnabled="true" day="SUNDAY" hour="20" maxConcurrent="5" />
	<schedule castleId="3" day="4" />
	<schedule castleId="4" day="Funday" siegeEnabled="yes" />
	<schedule castleId="x" />
</list>"#;

    #[test]
    fn circlets_are_the_castle_table() {
        assert_eq!(circlet_by_castle_id(1), 6838);
        assert_eq!(circlet_by_castle_id(8), 8182);
        assert_eq!(circlet_by_castle_id(9), 8183);
        assert_eq!(circlet_by_castle_id(0), 0);
        assert_eq!(circlet_by_castle_id(10), 0);
        assert_eq!(circlet_by_castle_id(-1), 0);
        assert_eq!(circlet(), 6838);
    }

    #[test]
    fn siege_schedule_reads_days_defaults_and_skips_broken_entries() {
        let schedule = SiegeSchedule::from_xml(SCHEDULE).unwrap();
        assert_eq!(schedule.len(), 4);
        assert_eq!(
            schedule.for_castle(2),
            Some(&SiegeScheduleDate {
                day: 1,
                hour: 20,
                max_concurrent: 5,
                siege_enabled: true
            })
        );
        // A numeric day stays, the others take Java's StatSet defaults (siege disabled).
        assert_eq!(
            schedule.for_castle(3),
            Some(&SiegeScheduleDate {
                day: 4,
                ..SiegeScheduleDate::default()
            })
        );
        // An unknown day name is -1; `Boolean.parseBoolean("yes")` is false.
        let odd = schedule.for_castle(4).unwrap();
        assert_eq!((odd.day, odd.siege_enabled), (-1, false));
        assert!(schedule.for_castle(9).is_none());
    }

    fn zones(files: &[(&str, &str)]) -> (tempfile::TempDir, Zones) {
        let dir = tempfile::tempdir().unwrap();
        let zone_dir = dir.path().join("data/zones");
        std::fs::create_dir_all(&zone_dir).unwrap();
        for (name, text) in files {
            std::fs::write(zone_dir.join(name), text).unwrap();
        }
        let zones = Zones::load(dir.path());
        (dir, zones)
    }

    const RESIDENCE_ZONES: &str = r#"<list enabled="true">
  <zone name="gludio_castle_battlefield_territory" type="SiegeZone" shape="Cuboid" minZ="-100" maxZ="100">
    <stat name="castleId" val="1" /><node X="0" Y="0" /><node X="1000" Y="1000" /></zone>
  <zone name="shanty_fortress_siege" type="SiegeZone" shape="Cuboid" minZ="-100" maxZ="100">
    <stat name="fortId" val="101" /><node X="5000" Y="0" /><node X="6000" Y="1000" /></zone>
  <zone name="gludio_castle" type="CastleZone" shape="Cuboid" minZ="-100" maxZ="100">
    <stat name="castleId" val="1" /><node X="100" Y="100" /><node X="200" Y="200" /></zone>
  <zone name="shanty_fortress" type="FortZone" shape="Cuboid" minZ="-100" maxZ="100">
    <stat name="fortId" val="101" /><node X="5100" Y="100" /><node X="5200" Y="200" /></zone>
  <zone name="gludio_castle_teleport" type="ResidenceTeleportZone" shape="Cuboid" minZ="-100" maxZ="100">
    <stat name="residenceId" val="1" /><node X="150" Y="150" /><node X="160" Y="160" /></zone>
  <zone name="hall_teleport" type="ResidenceHallTeleportZone" shape="Cuboid" minZ="-100" maxZ="100">
    <stat name="residenceId" val="1" /><node X="150" Y="150" /><node X="160" Y="160" /></zone>
  <zone name="gludin_agit_001" type="ClanHallZone" shape="Cuboid" minZ="-100" maxZ="100">
    <stat name="clanHallId" val="26" /><node X="9000" Y="0" /><node X="9100" Y="100" /></zone>
  <zone name="gludin_agit_001" type="ClanHallZone" shape="Cuboid" minZ="-100" maxZ="100">
    <stat name="clanHallId" val="26" /><node X="9200" Y="0" /><node X="9300" Y="100" /></zone>
  <zone name="bandit" type="SiegableHallZone" shape="Cuboid" minZ="-100" maxZ="100">
    <stat name="clanHallId" val="35" /><node X="9500" Y="0" /><node X="9600" Y="100" /></zone>
</list>"#;

    #[test]
    fn residence_zones_are_found_like_java() {
        let (_dir, zones) = zones(&[("residence.xml", RESIDENCE_ZONES)]);
        let siege = siege_zone_of(&zones, 1).unwrap();
        assert_eq!(siege.name.as_deref(), Some("gludio_castle_battlefield_territory"));
        assert_eq!(siege_object_id(siege), 1);
        assert_eq!(
            siege_zone_of(&zones, 101).unwrap().name.as_deref(),
            Some("shanty_fortress_siege")
        );
        assert!(siege_zone_of(&zones, 2).is_none());
        assert_eq!(
            castle_zone_of(&zones, 1).unwrap().name.as_deref(),
            Some("gludio_castle")
        );
        assert!(castle_zone_of(&zones, 101).is_none());
        assert_eq!(
            fort_zone_of(&zones, 101).unwrap().name.as_deref(),
            Some("shanty_fortress")
        );
        // The exact class: the hall teleport zone with the same residenceId is not taken.
        let tele = residence_teleport_zone_of(&zones, 1).unwrap();
        assert_eq!(tele.zone_type, "ResidenceTeleportZone");
        // The second zone of hall 26 is the one the hall keeps (it registered last).
        let hall = clan_hall_zone_of(&zones, 26).unwrap();
        assert!(hall.is_inside_zone(9250, 50, 0));
        assert!(!hall.is_inside_zone(9050, 50, 0));
        // A siegable hall's zone is a ClanHallZone too.
        assert_eq!(
            clan_hall_zone_of(&zones, 35).unwrap().zone_type,
            "SiegableHallZone"
        );
        assert!(clan_hall_zone_of(&zones, 27).is_none());
    }
}
