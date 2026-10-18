#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Spawn data: the rules of Java's `SpawnData` on small files, and the real Java spawn files.
//!
//! The real files are skipped with a printed message when the folder does not exist (CI). The
//! folder defaults to `C:\l2-rust\l2jmobius\dist\game`; set `L2JMOBIUS_GAME_DIR` to use another.

use std::fs;
use std::path::PathBuf;
use std::sync::OnceLock;

use l2_data::spawn::{PeriodOfDay, SpawnSelection, SpawnTerritory, TerritoryShape};
use l2_data::{NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_util::WorldRng;

/// NPC templates the small fixtures spawn. 2000 is a raid boss and 2001 a siege guard: both
/// are never spawned from data (Java `checkTemplate`).
fn npc_table() -> NpcTemplates {
    NpcTemplates::from_xml(
        r#"<?xml version="1.0"?><list>
            <npc id="1000"/><npc id="1001"/><npc id="1002"/>
            <npc id="2000" type="RaidBoss"/><npc id="2001" type="SiegeGuard"/>
        </list>"#,
        &NpcLoadOptions::default(),
    )
    .unwrap()
}

/// Wraps spawn blocks in the `<list>` root every spawn file has.
fn list(body: &str) -> String {
    format!(r#"<?xml version="1.0" encoding="UTF-8"?><list enabled="true">{body}</list>"#)
}

/// Writes the given files (relative name, text) into a fresh datapack and loads them.
fn load_files(files: &[(&str, &str)]) -> SpawnData {
    let dir = tempfile::tempdir().unwrap();
    for (name, text) in files {
        let path = dir.path().join("data/spawns").join(name);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }
    SpawnData::load(dir.path(), &npc_table(), &SpawnLoadOptions::default())
}

fn load_one(body: &str) -> SpawnData {
    load_files(&[("a.xml", &list(body))])
}

const SQUARE: &str =
    r#"<node x="0" y="0"/><node x="100" y="0"/><node x="100" y="100"/><node x="0" y="100"/>"#;

fn territory(name: &str, nodes: &str) -> String {
    format!(r#"<territory name="{name}" minZ="-100" maxZ="100">{nodes}</territory>"#)
}

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
    if dir.join("data").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: {} does not exist", dir.display());
        None
    }
}

// ---------------------------------------------------------------------------------------------
// Fixed spawns
// ---------------------------------------------------------------------------------------------

#[test]
fn each_count_is_its_own_spawn_with_the_attributes_as_written() {
    let data = load_one(
        r#"<spawn name="guards">
             <npc id="1000" count="3" x="10" y="20" z="30" heading="5" respawnDelay="60"
                  respawnRandom="20" chaseRange="900"/>
           </spawn>"#,
    );
    assert_eq!(data.parsed_files(), 1);
    assert_eq!(data.len(), 3);
    for spawn in data.spawns() {
        assert_eq!(spawn.npc_id, 1000);
        assert_eq!(
            spawn.amount, 1,
            "Java creates one Spawn per count, each of amount 1"
        );
        assert_eq!(
            (spawn.location.x, spawn.location.y, spawn.location.z),
            (10, 20, 30)
        );
        assert_eq!((spawn.heading, spawn.chase_range), (5, 900));
        assert_eq!((spawn.respawn_min_ms, spawn.respawn_max_ms), (40_000, 80_000));
        assert!(spawn.respawn_enabled);
        assert_eq!(spawn.name.as_deref(), Some("guards"));
        assert_eq!(spawn.territory, None);
        assert_eq!(spawn.group, None);
        assert_eq!(spawn.source, "data/spawns/a.xml");
    }
    assert_eq!(data.planned_fixed_spawns(), 3);
}

#[test]
fn defaults_are_java_defaults_and_numbers_use_integer_decode() {
    let data = load_one(r#"<spawn><npc id="0x3E8" x="0x10" y="-0x10" z="010"/></spawn>"#);
    let spawn = &data.spawns()[0];
    assert_eq!(spawn.npc_id, 1000);
    assert_eq!(
        (spawn.location.x, spawn.location.y, spawn.location.z),
        (16, -16, 8)
    );
    assert_eq!(spawn.heading, -1, "heading defaults to random (-1)");
    assert_eq!((spawn.respawn_min_ms, spawn.respawn_max_ms), (0, 0));
    assert!(!spawn.respawn_enabled, "no delay, no respawn");
    assert_eq!((spawn.chase_range, spawn.name.as_deref()), (0, None));
}

#[test]
fn disabled_list_is_parsed_but_loads_nothing() {
    let data = load_files(&[(
        "a.xml",
        r#"<list enabled="false"><spawn><npc id="1000" x="1" y="1"/></spawn></list>"#,
    )]);
    assert_eq!(data.parsed_files(), 1, "Java counts the file as parsed");
    assert!(data.is_empty());
}

#[test]
fn a_list_without_enabled_aborts_its_file() {
    let data = load_files(&[
        (
            "a.xml",
            r#"<list><spawn><npc id="1000" x="1" y="1"/></spawn></list>"#,
        ),
        ("b.xml", &list(r#"<spawn><npc id="1001" x="1" y="1"/></spawn>"#)),
    ]);
    assert_eq!(data.parsed_files(), 2);
    assert_eq!(data.len(), 1, "only the good file loads");
    assert_eq!(data.spawns()[0].npc_id, 1001);
}

#[test]
fn npc_without_coordinates_or_zone_is_skipped() {
    let data = load_one(r#"<spawn><npc id="1000"/><npc id="1001" x="7" y="8"/></spawn>"#);
    assert_eq!(data.len(), 1);
    assert_eq!(data.spawns()[0].npc_id, 1001);
}

#[test]
fn raid_bosses_siege_guards_and_unknown_templates_never_spawn() {
    let data = load_one(
        r#"<spawn><npc id="2000" x="1" y="1"/><npc id="2001" x="1" y="1"/><npc id="9999" x="1" y="1"/>
           <npc id="1002" x="1" y="1"/></spawn>"#,
    );
    assert_eq!(data.len(), 1);
    assert_eq!(data.spawns()[0].npc_id, 1002);
}

#[test]
fn unknown_children_are_ignored_like_java_does() {
    let data = load_one(r#"<spawn><foo bar="1"><baz/></foo><npc id="1000" x="1" y="2"/></spawn>"#);
    assert_eq!(data.len(), 1);
}

#[test]
fn ai_data_applies_only_to_npcs_after_it() {
    let data = load_one(
        r#"<spawn>
             <npc id="1000" x="1" y="1"/>
             <AIData><fleeX>5</fleeX><disableRandomWalk>true</disableRandomWalk>
                     <disableRandomAnimation>no</disableRandomAnimation></AIData>
             <npc id="1001" x="2" y="2"/>
           </spawn>"#,
    );
    let first = &data.spawns()[0];
    let second = &data.spawns()[1];
    assert_eq!(first.ai_data, None);
    let ai = second.ai_data.as_ref().unwrap();
    assert_eq!(ai.get("fleeX"), Some(&5));
    assert_eq!(ai.get("disableRandomWalk"), Some(&1));
    assert_eq!(ai.get("disableRandomAnimation"), Some(&0));
}

#[test]
fn a_bad_number_aborts_the_rest_of_the_file_and_keeps_what_came_before() {
    let data = load_files(&[(
        "a.xml",
        &list(
            r#"<spawn><npc id="1000" x="1" y="1"/></spawn>
               <spawn><npc id="1001" x="1" y="zz"/></spawn>
               <spawn><npc id="1002" x="1" y="1"/></spawn>"#,
        ),
    )]);
    assert_eq!(data.len(), 1);
    assert_eq!(data.spawns()[0].npc_id, 1000);
}

#[test]
fn a_missing_y_keeps_x_and_zeroes_the_rest_like_java() {
    // Java reads x, then y throws (NPE, caught): y and z stay 0 and x keeps its value.
    let data = load_one(r#"<spawn><npc id="1000" x="5" z="9"/></spawn>"#);
    let spawn = &data.spawns()[0];
    assert_eq!((spawn.location.x, spawn.location.y, spawn.location.z), (5, 0, 0));
}

#[test]
fn day_and_night_spawns_keep_their_flag() {
    let data = load_one(
        r#"<spawn><npc id="1000" count="2" x="1" y="1" periodOfDay="NIGHT"/>
                  <npc id="1001" x="1" y="1" periodOfDay="noon"/></spawn>"#,
    );
    assert_eq!(data.len(), 3);
    assert_eq!(data.spawns()[0].period_of_day, Some(PeriodOfDay::Night));
    assert_eq!(data.spawns()[2].period_of_day, None, "unknown values are ignored");
    assert!(
        data.spawns()[0].respawn_enabled,
        "Java keeps the default for day/night spawns"
    );
}

// ---------------------------------------------------------------------------------------------
// Territories
// ---------------------------------------------------------------------------------------------

#[test]
fn territory_is_defined_before_its_npcs_and_random_points_stay_in_it() {
    let data = load_one(&format!(
        r#"<spawn zone="t1">{}
             <banned_territory minZ="-100" maxZ="100"><node x="0" y="0"/><node x="50" y="0"/><node x="50" y="50"/></banned_territory>
             <npc id="1000" count="2"/></spawn>"#,
        territory("t1", SQUARE)
    ));
    assert_eq!(data.len(), 2);
    for spawn in data.spawns() {
        assert_eq!(spawn.territory.as_deref(), Some("t1"));
    }
    let t = data.territory("t1").unwrap();
    assert_eq!(t.banned().len(), 1);
    let mut rng = WorldRng::from_seed(42);
    for _ in 0..300 {
        let point = t.random_point(&mut rng, &|_, _, z| z).unwrap();
        assert!(t.shape().unwrap().contains(point.x, point.y, point.z));
        assert!(!matches!(&t.banned()[0], Some(s) if s.contains(point.x, point.y, point.z)));
    }
}

#[test]
fn an_npc_before_its_territory_gets_no_territory() {
    let data = load_one(&format!(
        r#"<spawn zone="t2"><npc id="1000" x="5" y="5"/>{}</spawn>"#,
        territory("t2", SQUARE)
    ));
    assert_eq!(data.spawns()[0].territory, None);
    assert!(
        data.territory("t2").is_some(),
        "the territory itself is still defined"
    );
}

#[test]
fn banned_territory_before_its_territory_is_skipped() {
    let data = load_one(
        r#"<spawn zone="t3"><banned_territory minZ="0" maxZ="1"><node x="0" y="0"/><node x="1" y="0"/><node x="1" y="1"/></banned_territory>
           </spawn>"#,
    );
    assert!(data.territory("t3").is_none());
}

#[test]
fn banned_territory_without_a_territory_aborts_the_file() {
    let data = load_files(&[(
        "a.xml",
        &list(
            r#"<spawn><npc id="1000" x="1" y="1"/></spawn>
                   <spawn><banned_territory minZ="0" maxZ="1"><node x="0" y="0"/><node x="1" y="0"/><node x="1" y="1"/></banned_territory></spawn>
                   <spawn><npc id="1001" x="1" y="1"/></spawn>"#,
        ),
    )]);
    assert_eq!(data.len(), 1, "the block after the abort is not read");
}

#[test]
fn territory_without_minz_aborts_the_file() {
    let data = load_one(
        r#"<spawn zone="t4"><territory><node x="0" y="0"/><node x="1" y="0"/><node x="1" y="1"/></territory>
           <npc id="1000" count="1"/></spawn>"#,
    );
    assert!(data.is_empty());
    assert!(data.territory("t4").is_none());
}

#[test]
fn first_territory_definition_wins() {
    let data = load_files(&[
        (
            "a.xml",
            &list(&format!(r#"<spawn>{}</spawn>"#, territory("dup", SQUARE))),
        ),
        (
            "b.xml",
            &list(&format!(
                r#"<spawn>{}</spawn>"#,
                territory(
                    "dup",
                    r#"<node x="500" y="500"/><node x="600" y="500"/><node x="600" y="600"/>"#
                )
            )),
        ),
    ]);
    match data.territory("dup").unwrap().shape() {
        Some(TerritoryShape::NPoly { xs, .. }) => assert_eq!(xs, &vec![0, 100, 100, 0]),
        other => panic!("unexpected shape {other:?}"),
    }
}

#[test]
fn territory_with_broken_coordinates_is_registered_without_a_shape() {
    let data = load_one(&format!(
        r#"{}<spawn zone="bad"/><spawn zone="bad"><npc id="1000" count="1"/></spawn>"#,
        r#"<spawn><territory name="bad" minZ="0" maxZ="1"><node x="zz" y="0"/></territory></spawn>"#
    ));
    let t = data.territory("bad").unwrap();
    assert!(t.shape().is_none());
    assert_eq!(data.spawns()[0].territory.as_deref(), Some("bad"));
    let mut rng = WorldRng::from_seed(1);
    assert!(
        t.random_point(&mut rng, &|_, _, z| z).is_none(),
        "Java throws here"
    );
}

#[test]
fn polygon_with_two_points_is_rejected_and_not_registered() {
    let data = load_one(
        r#"<spawn><territory name="thin" minZ="0" maxZ="1"><node x="0" y="0"/><node x="1" y="1"/></territory></spawn>"#,
    );
    assert!(data.territory("thin").is_none());
}

#[test]
fn cuboid_and_cylinder_shapes() {
    let data = load_one(
        r#"<spawn>
             <territory name="box" shape="Cuboid" minZ="50" maxZ="-50"><node x="10" y="20"/><node x="0" y="0"/></territory>
             <territory name="disc" shape="Cylinder" minZ="-5" maxZ="5"><node x="100" y="100" rad="30"/></territory>
           </spawn>"#,
    );
    assert_eq!(
        data.territory("box").unwrap().shape(),
        Some(&TerritoryShape::Cuboid {
            x: 0,
            y: 0,
            width: 10,
            height: 20,
            z1: -50,
            z2: 50,
        })
    );
    assert_eq!(
        data.territory("disc").unwrap().shape(),
        Some(&TerritoryShape::Cylinder {
            x: 100,
            y: 100,
            z1: -5,
            z2: 5,
            rad: 30,
        })
    );
}

#[test]
fn banned_area_with_broken_coordinates_is_kept_without_a_shape() {
    let data = load_one(&format!(
        r#"<spawn zone="t5">{}<banned_territory minZ="0" maxZ="1"><node x="zz" y="0"/></banned_territory>
           <npc id="1000" count="1"/></spawn>"#,
        territory("t5", SQUARE)
    ));
    let t: &SpawnTerritory = data.territory("t5").unwrap();
    assert_eq!(
        t.banned().len(),
        1,
        "the broken banned area is kept, as Java's null"
    );
    assert!(t.banned()[0].is_none());
    let mut rng = WorldRng::from_seed(3);
    assert!(
        t.random_point(&mut rng, &|_, _, z| z).is_none(),
        "Java throws on the null area"
    );
}

#[test]
fn banned_area_with_one_point_is_rejected_and_not_kept() {
    let data = load_one(&format!(
        r#"<spawn zone="t6">{}<banned_territory minZ="0" maxZ="1"><node x="0" y="0"/></banned_territory></spawn>"#,
        territory("t6", SQUARE)
    ));
    assert!(data.territory("t6").unwrap().banned().is_empty());
}

// ---------------------------------------------------------------------------------------------
// Groups
// ---------------------------------------------------------------------------------------------

#[test]
fn random_fill_group_has_one_slot_per_territory() {
    let data = load_one(&format!(
        r#"<spawn zones="a;b;c" selection="randomFill" maximumNpc="4">
             {}{}{}
             <npc id="1000" count="2" respawnDelay="30"/>
             <npc id="1001" count="1"/>
           </spawn>"#,
        territory("a", SQUARE),
        territory("b", SQUARE),
        territory("c", SQUARE)
    ));
    let group = &data.groups()[0];
    assert_eq!(
        group.name, "a",
        "no spawn name: the first territory names the group"
    );
    assert_eq!(group.selection, SpawnSelection::RandomFill);
    assert_eq!(group.maximum_npc, 4);
    assert_eq!(group.entries.len(), 2);
    assert_eq!((group.entries[0].total, group.entries[0].respawn_delay), (2, 30));
    assert_eq!(group.entries[0].slots.len(), 3);
    assert_eq!(data.spawns().len(), 6);
    for &index in &group.entries[0].slots {
        let spawn = &data.spawns()[index];
        assert_eq!(spawn.group, Some(0));
        assert_eq!(spawn.amount, 2);
        assert!(!spawn.respawn_enabled, "rolling groups replace their own units");
        assert!(spawn.territory.is_some());
    }
    assert_eq!(data.planned_fixed_spawns(), 0);
    assert_eq!(data.random_groups(), 1);
}

#[test]
fn all_group_stops_at_maximum_npc_for_the_fixed_count() {
    let data = load_one(&format!(
        r#"<spawn zone="g" selection="all" maximumNpc="3">{}
             <npc id="1000" count="2"/><npc id="1001" count="2" respawnDelay="10"/></spawn>"#,
        territory("g", SQUARE)
    ));
    assert_eq!(data.groups()[0].entries.len(), 2);
    assert_eq!(
        data.planned_fixed_spawns(),
        2,
        "the second entry does not fit the cap of 3"
    );
    // One slot per entry (a single territory): the second entry has the delay.
    assert!(
        data.spawns()[1].respawn_enabled,
        "all group slots respawn when they have a delay"
    );
    assert!(!data.spawns()[0].respawn_enabled);
}

#[test]
fn unknown_selection_means_all_and_random_ignores_the_cap() {
    let data = load_one(&format!(
        r#"<spawn zone="g" selection="sometimes" maximumNpc="5">{}<npc id="1000" count="1"/></spawn>
           <spawn zone="g" selection="random" maximumNpc="5">{}<npc id="1000" count="1"/></spawn>"#,
        territory("g", SQUARE),
        ""
    ));
    assert_eq!(data.groups()[0].selection, SpawnSelection::All);
    assert_eq!(data.groups()[1].selection, SpawnSelection::Random);
    assert_eq!(data.groups()[1].maximum_npc, 0);
}

#[test]
fn group_entry_with_day_or_night_is_skipped() {
    let data = load_one(&format!(
        r#"<spawn zone="g" selection="random">{}<npc id="1000" x="1" y="1" periodOfDay="day"/></spawn>"#,
        territory("g", SQUARE)
    ));
    assert_eq!(data.groups()[0].entries.len(), 0);
    assert!(data.is_empty());
    assert_eq!(data.random_groups(), 0);
}

#[test]
fn zones_with_zone_is_exclusive_and_zones_without_selection_is_ignored() {
    let data = load_one(&format!(
        r#"<spawn zone="first" zones="x;y">{}<npc id="1000" x="1" y="1"/></spawn>
           <spawn zones="p;q"><npc id="1000" x="1" y="1"/></spawn>"#,
        territory("first", SQUARE)
    ));
    assert_eq!(data.groups().len(), 0, "no selection, no group");
    assert_eq!(data.spawns()[0].territory.as_deref(), Some("first"));
    assert_eq!(data.spawns()[1].territory, None, "p is not defined");
}

// ---------------------------------------------------------------------------------------------
// The real Java spawn files
// ---------------------------------------------------------------------------------------------

fn real_data() -> Option<&'static SpawnData> {
    static DATA: OnceLock<Option<(NpcTemplates, SpawnData)>> = OnceLock::new();
    DATA.get_or_init(|| {
        let root = game_dir()?;
        let npcs = NpcTemplates::load(&root, &NpcLoadOptions::default());
        let spawns = SpawnData::load(&root, &npcs, &SpawnLoadOptions::default());
        Some((npcs, spawns))
    })
    .as_ref()
    .map(|(_, spawns)| spawns)
}

#[test]
fn real_datapack_counts() {
    let Some(data) = real_data() else {
        return;
    };
    // The folder holds 187 XML files, all enabled, and 429 blocks with a selection attribute.
    assert_eq!(data.parsed_files(), 187);
    assert_eq!(data.groups().len(), 429);
    eprintln!(
        "real spawns: {} table entries, {} planned fixed, {} random groups, {} territories",
        data.len(),
        data.planned_fixed_spawns(),
        data.random_groups(),
        data.territories().count()
    );
}

#[test]
fn real_datapack_spawns_point_at_existing_slots_and_territories() {
    let Some(data) = real_data() else {
        return;
    };
    for (index, group) in data.groups().iter().enumerate() {
        for entry in &group.entries {
            for &slot in &entry.slots {
                assert_eq!(data.spawns()[slot].group, Some(index));
            }
        }
    }
    for spawn in data.spawns() {
        if let Some(name) = &spawn.territory {
            assert!(data.territory(name).is_some(), "{name} is registered");
        }
    }
    let mut rng = WorldRng::from_seed(2026);
    let mut checked = 0;
    for territory in data.territories() {
        if territory.shape().is_none() {
            continue;
        }
        let point = territory.random_point(&mut rng, &|_, _, z| z);
        if let Some(point) = point {
            let shape = territory.shape().unwrap();
            assert!(
                shape.contains(point.x, point.y, point.z) || matches!(shape, TerritoryShape::Cuboid { .. })
            );
            checked += 1;
        }
    }
    assert!(checked > 0);
    eprintln!("random points checked for {checked} territories");
}

#[test]
fn real_datapack_sample_entries() {
    let Some(data) = real_data() else {
        return;
    };
    // Gremlins.xml has seven `<npc id="18342">` lines: 10 + 10 + 15 * 4 + 20 = 100 (grep agrees).
    let gremlins = data.spawns_of(18342).count();
    assert_eq!(gremlins, 100, "Gremlin count from Others/Gremlins.xml");
    // Turek orc spawns carry AIData (fleeX, fleeY, fleeZ).
    let with_ai = data
        .spawns()
        .iter()
        .filter(|spawn| spawn.ai_data.is_some())
        .count();
    assert!(with_ai > 0);
    // The 'random' groups of Wasteland.xml are named after their first territory.
    assert!(data.groups().iter().any(|group| group.name == "gludio05_1923_03"));
}
