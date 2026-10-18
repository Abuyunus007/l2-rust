# Geodata: format and comparison of the two sets

## `.l2j` format (from `gameserver/geoengine/geodata/`)

- One file per region `X_Y.l2j`, with X in 11..26 and Y in 10..26. The world origin is at region (20, 18).
- A region is **256 × 256 blocks**, a block is **8 × 8 cells**, a cell is **16 × 16 world units**. So a region is 2048 × 2048 cells = 32768 world units.
- Blocks are stored in order `bx * 256 + by`, cells inside a block in order `cx * 8 + cy`. Little-endian.
- Each block starts with a type byte:
  - `0` **flat**: `i16 height` (no NSWE, every direction passable). 3 bytes.
  - `1` **complex**: 64 × `i16`. NSWE = `v & 0x0F`, height = `(i16)(v & 0xFFF0) >> 1` (signed!). 129 bytes.
  - `2` **multilayer**: per cell `u8 n` (1..125), then n × `i16` with the same encoding as complex. Variable size.
- NSWE bits: E=1, W=2, S=4, N=8.
- An all-flat region is exactly 196,608 bytes (65536 × 3), meaning "no real data".
- Java memory-maps each file and keeps a region array of 32 × 32. Doors and fences change NSWE at runtime (`setNearestNswe`/`unsetNearestNswe`; a flat block is converted to complex when needed).
- Pathfinding: `PathFinding.java` + `NodeBuffer` (A*-style with buffers sized by `PathFindBuffers` in `GeoEngine.ini`). `PathFinding = 2` in your config means pathfinding is on and uses the geodata cells.

The geo engine is ~3.3k lines and depends on very little else (config, doors/fences, `World` constants). That makes it a good **early, isolated** porting target. It can be tested in isolation: same `(x, y, z)` query gives the same answer in Java and Rust.

## Comparison of your two sets

Scripts: `tools/analysis/geocmp.py` and `geolayers.py` (Python + numpy, run with `python -I geocmp.py <dirA> <dirB>`). Full per-region results: [05-geodata-comparison.csv](05-geodata-comparison.csv). Both sets have the **same 203 region files**, and **every file parses cleanly** (no trailing bytes, no invalid block types) in both sets.

| | Downloaded (A) | Converted from client (B) |
|---|---:|---:|
| Empty (all-flat) regions | 25 | 8 |
| Flat blocks | 7.13 M | 7.63 M |
| Complex blocks | 5.46 M | 3.44 M |
| Multilayer blocks | 0.71 M | **2.24 M** |
| Average layers per cell | 1.04 | 1.25 |

- **17 regions have data only in the converted set**: 15_20, 15_21, 15_24, 16_17, 16_22, 16_23, 17_18, 18_13, 18_17, 18_18, 21_12, 21_13, 22_12, 25_22, 25_24, 26_15, 26_16. The downloaded set leaves them flat. No region has data only in the downloaded set.
- Over all regions, the top-layer height differs by more than 16 in a **median of 0.8%** of cells (90th percentile 9%). NSWE differs in a median of 1.1% of cells.
- **The converted set has many more layers** in towns and dungeons (14_23: 4.9 vs 1.2 layers per cell). That fits building floors, bridges and interiors taken from the client geometry. More layers is more accurate if they're real, and wrong if they're noise (roofs, tree canopies).

Outliers, using "is A's surface present at any layer of B?":

| Region | A surface found in B | B surface found in A | Reading |
|---|---:|---:|---|
| 22_22 (typical) | 98% | 98% | basically the same |
| 14_23, 14_22, 23_12 | ~80% | ~80% | B has extra structure, same base terrain |
| 12_25 | 88% | 90% | mostly the same |
| 16_10 | 78% | **0%** | B has an extra layer far above (~+10k) in every cell, probably a ceiling or sky mesh from the client |
| 12_24 | **8%** | **8%** | completely different terrain |
| 17_10 | **0%** | **0%** | completely different terrain |

## What this means

- The converted set covers more of the world and has more detail. It's probably the better base, but it has at least one suspicious region (16_10), and 12_24 and 17_10 disagree completely between the sets. One of the two is wrong there (an offset, a different map version, or broken conversion).
- This can't be settled by numbers alone. It needs an in-game check at a few spots (Java server, `//loc` and walking around in 12_24, 17_10, 16_10, 14_23).

**Proposed for the plan** (to decide together):
1. Milestone-1 tooling adds a `geotool` (Rust): stats, a diff, and a **heightmap/NSWE PNG render per region** for both sets, side by side.
2. Allow a **per-region override** (`GeoDataPath` plus an optional override list) so we can mix sets where one is clearly better.
3. Run the Java↔Rust geo comparison on both sets.
