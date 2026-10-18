# M1-CFG-01: `.ini` reader with ConfigReader semantics

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | yes (the M2 config generator builds on it) |
| Depends on | M1-WS-01 |
| Size | S |

## Goal
`l2-config` can read every L2JMobius `.ini` file exactly the way Java's `ConfigReader` does (D-02).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\commons\util\ConfigReader.java`: the whole class (file loading, encoding, `#` comments, key/value splitting, trimming, every typed getter and its parse rules: booleans, numbers like `1.` for floats, enum matching, `Duration` format, int arrays with a delimiter, behaviour on a missing/invalid value → default + a log line)
- Java `Properties` semantics used underneath, if any (line continuations, `=` vs `:` separators, escapes). Check what ConfigReader really uses.
- Sample inputs: `C:\l2-rust\l2jmobius\dist\game\config\*.ini`, `dist\login\config\*.ini`

## Reference implementation to copy
- `crates/l2-util` (layout)

## Files
Create:
- `crates/l2-config/{Cargo.toml,src/lib.rs,src/reader.rs,src/error.rs}`
- `crates/l2-config/tests/reader.rs`
- `crates/l2-config/tests/all_dist_files.rs`

## Requirements
1. `IniFile::load(path)`, `get_bool/byte/short/int/long/float/double/string/enum/duration/int_array(key, default)` with Java's parsing and fallback rules. Log a `tracing::warn!` wherever Java logs.
2. `keys()` returns all keys (for pattern keys like `BufferPool.*.Size`).
3. A missing file: same behaviour as Java (check whether it logs + uses defaults or fails).
4. Document every parsing quirk found in Java in the module docs (`// NOTE(java-quirk):` at the code).

## Out of scope
- Concrete config structs (M1-CFG-02).

## Tests
- Unit tests for each getter, including invalid values → default.
- `all_dist_files`: parses every `.ini` under `C:\l2-rust\l2jmobius\dist` without errors when that folder exists (skipped with a message otherwise, e.g. in CI). Counts keys: expect ~1,576 in total (report the exact number).

## Done when
- [ ] tests pass
- [ ] committed as `M1-CFG-01: <summary>`
