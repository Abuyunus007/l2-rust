//! Target `java-facts`: facts the tests compare the port with, read from the Java source text.
//!
//! Java: dist/game/data/scripts/{handlers,vehicles,ai/areas/Gracia/vehicles}/**/*.java and
//! gameserver/entity/actor/enums/player/PlayerClass.java.
//! Output: `java-vectors/out/java_facts/{handlers,boat_routes,air_ship_routes,player_classes}.json`.
//!
//! The tests used to open these `.java` files themselves and so skipped on every machine without
//! the L2JMobius checkout. Now they read the committed JSON through `l2_testkit::java_facts`; only
//! regenerating needs the checkout. The extraction below is the code that used to sit in the tests,
//! moved here unchanged in behaviour. The JSON is sorted (maps by key, files and classes by name),
//! so a regenerate gives no diff.

use super::Output;
use anyhow::{Context, Result, bail};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

const OUT_DIR: &str = "java-vectors/out/java_facts";
const SCRIPTS: &str = "dist/game/data/scripts";
const PLAYER_CLASS: &str = "java/org/l2jmobius/gameserver/entity/actor/enums/player/PlayerClass.java";

// ---------------------------------------------------------------------------------- JSON

/// The few JSON shapes the facts need. Maps are sorted (`BTreeMap`), so the output is stable.
enum Json {
    Str(String),
    Num(i64),
    Arr(Vec<Json>),
    Obj(BTreeMap<String, Json>),
}

fn quote(text: &str) -> String {
    let mut out = String::from("\"");
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl Json {
    fn is_scalar_list(items: &[Json]) -> bool {
        items.iter().all(|i| matches!(i, Json::Str(_) | Json::Num(_)))
    }

    fn write(&self, out: &mut String, indent: usize) {
        let pad = "  ".repeat(indent);
        match self {
            Json::Str(s) => out.push_str(&quote(s)),
            Json::Num(n) => out.push_str(&n.to_string()),
            Json::Arr(items) if items.is_empty() => out.push_str("[]"),
            Json::Arr(items) if Self::is_scalar_list(items) => {
                out.push('[');
                for (i, item) in items.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    item.write(out, indent);
                }
                out.push(']');
            }
            Json::Arr(items) => {
                out.push_str("[\n");
                for (i, item) in items.iter().enumerate() {
                    out.push_str(&pad);
                    out.push_str("  ");
                    item.write(out, indent + 1);
                    out.push_str(if i + 1 < items.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad);
                out.push(']');
            }
            Json::Obj(map) if map.is_empty() => out.push_str("{}"),
            Json::Obj(map) => {
                out.push_str("{\n");
                for (i, (key, value)) in map.iter().enumerate() {
                    out.push_str(&pad);
                    out.push_str("  ");
                    out.push_str(&quote(key));
                    out.push_str(": ");
                    value.write(out, indent + 1);
                    out.push_str(if i + 1 < map.len() { ",\n" } else { "\n" });
                }
                out.push_str(&pad);
                out.push('}');
            }
        }
    }
}

fn strings(items: impl IntoIterator<Item = impl Into<String>>) -> Json {
    Json::Arr(items.into_iter().map(|s| Json::Str(s.into())).collect())
}

fn obj(entries: Vec<(&str, Json)>) -> Json {
    Json::Obj(entries.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn output(name: &str, source: &str, mut fields: Vec<(&str, Json)>) -> Output {
    fields.push(("generated_by", Json::Str("codegen java-facts".into())));
    fields.push(("source", Json::Str(source.into())));
    let mut text = String::new();
    obj(fields).write(&mut text, 0);
    text.push('\n');
    Output {
        path: PathBuf::from(format!("{OUT_DIR}/{name}.json")),
        contents: text,
    }
}

// ---------------------------------------------------------------------------------- sources

/// Every Java file the facts need, read once.
pub struct Sources {
    handlers: HandlerSources,
    /// File stem to text, for `scripts/vehicles/*.java`.
    boats: BTreeMap<String, String>,
    /// Dock controller folder to text, for `ai/areas/Gracia/vehicles/<name>/<name>.java`.
    air_ships: BTreeMap<String, String>,
    player_class: String,
}

struct HandlerSources {
    /// Folder (relative to `scripts/handlers`) to class name to text.
    folders: BTreeMap<&'static str, BTreeMap<String, String>>,
}

/// The handler folders the audit looks at, relative to `scripts/handlers`.
const HANDLER_FOLDERS: &[&str] = &[
    "bypass/npc",
    "bypass/communityboard",
    "items",
    "chat/commands/user",
    "chat/commands/voiced",
    "actions/click",
    "actions/shiftclick",
    "punishments",
];

/// All `.java` files under `dir` (recursive), file stem to text.
fn java_files(dir: &Path) -> Result<BTreeMap<String, String>> {
    let mut found = BTreeMap::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(path) = stack.pop() {
        for entry in std::fs::read_dir(&path).with_context(|| format!("reading {}", path.display()))? {
            let p = entry?.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().is_some_and(|e| e == "java")
                && let Some(stem) = p.file_stem()
            {
                let text = std::fs::read_to_string(&p).with_context(|| format!("reading {}", p.display()))?;
                found.insert(stem.to_string_lossy().into_owned(), text);
            }
        }
    }
    Ok(found)
}

pub fn read_sources(l2j_root: &Path) -> Result<Sources> {
    let scripts = l2j_root.join(SCRIPTS);
    let mut folders = BTreeMap::new();
    for folder in HANDLER_FOLDERS {
        folders.insert(*folder, java_files(&scripts.join("handlers").join(folder))?);
    }
    let boats = java_files(&scripts.join("vehicles"))?;

    let gracia = scripts.join("ai/areas/Gracia/vehicles");
    let mut air_ships = BTreeMap::new();
    for entry in std::fs::read_dir(&gracia).with_context(|| format!("reading {}", gracia.display()))? {
        let dir = entry?.path();
        if !dir.is_dir() {
            continue;
        }
        let name = dir
            .file_name()
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        let file = dir.join(format!("{name}.java"));
        let text = std::fs::read_to_string(&file).with_context(|| format!("reading {}", file.display()))?;
        air_ships.insert(name, text);
    }
    let path = l2j_root.join(PLAYER_CLASS);
    let player_class =
        std::fs::read_to_string(&path).with_context(|| format!("reading {}", path.display()))?;
    Ok(Sources {
        handlers: HandlerSources { folders },
        boats,
        air_ships,
        player_class,
    })
}

// ---------------------------------------------------------------------------------- handlers

/// The quoted words of the first `{ ... }` block after `marker` in a Java file.
fn words_after(source: &str, marker: &str) -> Vec<String> {
    let Some(start) = source.find(marker) else {
        return Vec::new();
    };
    let rest = &source[start..];
    let Some(end) = rest.find("};") else {
        return Vec::new();
    };
    rest[..end]
        .split('"')
        .enumerate()
        .filter(|(i, _)| i % 2 == 1)
        .map(|(_, w)| w.to_owned())
        .collect()
}

/// Lines that hold nothing but an integer: the `COMMAND_IDS` of the user commands.
fn integer_lines(source: &str) -> Vec<i64> {
    source
        .lines()
        .filter_map(|l| l.trim().parse::<i64>().ok())
        .collect()
}

fn handlers(src: &HandlerSources) -> Output {
    let folder = |name: &str| &src.folders[name];
    let by_class = |name: &str, marker: &str| {
        Json::Obj(
            folder(name)
                .iter()
                .map(|(class, text)| (class.clone(), strings(words_after(text, marker))))
                .collect(),
        )
    };
    let names = |name: &str| strings(folder(name).keys().cloned());
    let user = Json::Obj(
        folder("chat/commands/user")
            .iter()
            .map(|(class, text)| {
                (
                    class.clone(),
                    Json::Arr(integer_lines(text).into_iter().map(Json::Num).collect()),
                )
            })
            .collect(),
    );
    output(
        "handlers",
        "dist/game/data/scripts/handlers",
        vec![
            // Class name to the quoted words of its `COMMANDS` / `COMMAND` array.
            ("bypass_npc", by_class("bypass/npc", "COMMANDS")),
            ("communityboard", by_class("bypass/communityboard", "COMMAND")),
            ("items", names("items")),
            // Class name to the integer-only lines of the file (the command ids).
            ("user", user),
            ("voiced", names("chat/commands/voiced")),
            ("click", names("actions/click")),
            ("shiftclick", names("actions/shiftclick")),
            ("punishments", names("punishments")),
        ],
    )
}

// ---------------------------------------------------------------------------------- routes

type Point = [i64; 5];

fn point_json(p: &Point) -> Json {
    Json::Arr(p.iter().copied().map(Json::Num).collect())
}

fn path_json(path: &[Point]) -> Json {
    Json::Arr(path.iter().map(point_json).collect())
}

/// The `new VehiclePathPoint(...)` calls of a Java snippet as `[x, y, z, move_speed, rotation_speed]`.
/// The three argument form uses the defaults of the constructor (350 and 4000).
fn points(text: &str) -> Result<Vec<Point>> {
    let mut out = Vec::new();
    for part in text.split("new VehiclePathPoint(").skip(1) {
        let Some(end) = part.find(')') else {
            bail!("unterminated VehiclePathPoint in {part:?}");
        };
        let n = part[..end]
            .split(',')
            .map(|v| {
                v.trim()
                    .parse::<i64>()
                    .with_context(|| format!("point value {v:?}"))
            })
            .collect::<Result<Vec<_>>>()?;
        out.push(match n.as_slice() {
            [x, y, z] => [*x, *y, *z, 350, 4000],
            [x, y, z, s, r] => [*x, *y, *z, *s, *r],
            other => bail!("VehiclePathPoint with {} arguments", other.len()),
        });
    }
    Ok(out)
}

/// The text from `<marker> =` to the closing `};` (the first one after it).
fn section<'a>(src: &'a str, marker: &str) -> Result<&'a str> {
    let start = src
        .find(&format!("{marker} ="))
        .with_context(|| format!("no {marker} in the source"))?;
    let end = start
        + src[start..]
            .find("};")
            .with_context(|| format!("no end of {marker}"))?;
    Ok(&src[start..end])
}

/// Every `VehiclePathPoint[] NAME =` array of a file.
fn path_arrays(src: &str) -> Result<BTreeMap<String, Vec<Point>>> {
    const DECL: &str = "VehiclePathPoint[] ";
    let mut out = BTreeMap::new();
    let mut from = 0;
    while let Some(at) = src[from..].find(DECL) {
        let name_start = from + at + DECL.len();
        from = name_start;
        let rest = &src[name_start..];
        let name: String = rest
            .chars()
            .take_while(|c| c.is_alphanumeric() || *c == '_')
            .collect();
        if name.is_empty() || !rest[name.len()..].trim_start().starts_with('=') {
            continue;
        }
        out.insert(name.clone(), points(section(rest, &name)?)?);
    }
    Ok(out)
}

fn boat_routes(boats: &BTreeMap<String, String>) -> Result<Output> {
    let mut files = BTreeMap::new();
    for (file, text) in boats {
        let arrays = path_arrays(text)?;
        if arrays.is_empty() {
            continue;
        }
        files.insert(
            file.clone(),
            Json::Obj(arrays.iter().map(|(n, p)| (n.clone(), path_json(p))).collect()),
        );
    }
    Ok(output(
        "boat_routes",
        "dist/game/data/scripts/vehicles/*.java",
        vec![("files", Json::Obj(files))],
    ))
}

/// The value of `private static final int NAME = <digits>;`.
fn constant(src: &str, name: &str) -> Result<i64> {
    let at = src
        .find(&format!("{name} = "))
        .with_context(|| format!("no constant {name}"))?;
    let rest = &src[at + name.len() + 3..];
    let digits: String = rest
        .chars()
        .take_while(|c| c.is_ascii_digit() || *c == '-')
        .collect();
    digits.parse().with_context(|| format!("constant {name}"))
}

/// The rows of `TELEPORTS`: the groups at depth 2 of the braces.
fn teleport_rows(src: &str) -> Result<Vec<Vec<Point>>> {
    let all = section(src, "TELEPORTS")?;
    let (mut depth, mut row, mut out) = (0, String::new(), Vec::new());
    for ch in all.chars() {
        match ch {
            '{' => depth += 1,
            '}' => {
                if depth == 2 {
                    out.push(points(&row)?);
                    row.clear();
                }
                depth -= 1;
            }
            _ if depth == 2 => row.push(ch),
            _ => {}
        }
    }
    Ok(out)
}

fn air_ship_routes(air_ships: &BTreeMap<String, String>) -> Result<Output> {
    let mut controllers = BTreeMap::new();
    let mut gludio_gracia = BTreeMap::new();
    for (name, src) in air_ships {
        if src.contains("DOCK_ZONE = ") {
            let arrival = points(section(src, "ARRIVAL")?)?;
            let depart = points(section(src, "DEPART")?)?;
            let teleports = teleport_rows(src)?;
            controllers.insert(
                name.clone(),
                obj(vec![
                    ("arrival", path_json(&arrival)),
                    ("depart", path_json(&depart)),
                    (
                        "teleports",
                        Json::Arr(teleports.iter().map(|r| path_json(r)).collect()),
                    ),
                    ("dock_zone", Json::Num(constant(src, "DOCK_ZONE")?)),
                    ("controller_id", Json::Num(constant(src, "CONTROLLER_ID")?)),
                    ("location", Json::Num(constant(src, "LOCATION")?)),
                ]),
            );
        } else if name == "AirShipGludioGracia" {
            for (array, path) in path_arrays(src)? {
                gludio_gracia.insert(array, path_json(&path));
            }
        }
    }
    if controllers.is_empty() || gludio_gracia.is_empty() {
        bail!("no air ship controllers found");
    }
    Ok(output(
        "air_ship_routes",
        "dist/game/data/scripts/ai/areas/Gracia/vehicles/*/*.java",
        vec![
            ("controllers", Json::Obj(controllers)),
            ("gludio_gracia", Json::Obj(gludio_gracia)),
        ],
    ))
}

// ---------------------------------------------------------------------------------- classes

/// `FIGHTER(0, false, Race.HUMAN, null),` gives `(0, "HUMAN")`.
fn player_classes(source: &str) -> Result<Output> {
    let mut races: BTreeMap<u32, String> = BTreeMap::new();
    for line in source.lines() {
        let Some((_, rest)) = line.trim().split_once('(') else {
            continue;
        };
        let Some((id, rest)) = rest.split_once(',') else {
            continue;
        };
        let Ok(id) = id.trim().parse::<u32>() else {
            continue;
        };
        let Some((_, race)) = rest.split_once("Race.") else {
            continue;
        };
        let race = race.split(',').next().unwrap_or_default();
        races.insert(id, race.to_owned());
    }
    if races.is_empty() {
        bail!("no classes found in PlayerClass.java");
    }
    // Sorted by id; an array of `[id, race]` keeps the numeric order (JSON keys would sort as text).
    let rows: Vec<Json> = races
        .into_iter()
        .map(|(id, race)| Json::Arr(vec![Json::Num(i64::from(id)), Json::Str(race)]))
        .collect();
    Ok(output(
        "player_classes",
        "gameserver/entity/actor/enums/player/PlayerClass.java",
        vec![("races", Json::Arr(rows))],
    ))
}

pub fn generate(src: &Sources) -> Result<Vec<Output>> {
    Ok(vec![
        handlers(&src.handlers),
        boat_routes(&src.boats)?,
        air_ship_routes(&src.air_ships)?,
        player_classes(&src.player_class)?,
    ])
}
