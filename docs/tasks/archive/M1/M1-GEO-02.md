# M1-GEO-02: Geodata tool (stats, diff, render)

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M1-GEO-01 |
| Size | S |

## Goal
`l2-geotool` CLI to compare your two geodata sets visually and numerically, so we can decide which set (or which mix) to use (D-26).

## Java source (the spec)
- none (uses `l2-geo`). The analysis prototype: `tools/analysis/geocmp.py`, `geolayers.py`

## Reference implementation to copy
- `tools/analysis/geocmp.py` (same metrics and CSV columns)

## Files
Create:
- `crates/l2-geotool/{Cargo.toml,src/main.rs,src/stats.rs,src/diff.rs,src/render.rs}`

## Requirements
1. `l2-geotool stats <dir>`: per-region block counts + average layers (CSV to stdout).
2. `l2-geotool diff <dirA> <dirB> [regions…]`: the same columns as `geocmp.py`, plus the "A surface found in B / B in A" percentages from `geolayers.py`. Parallel over regions (rayon is not approved; use `std::thread::scope`).
3. `l2-geotool render <dir> <region> --out file.png [--mode height|nswe|layers]`: 2048×2048 PNG. Height = a colour ramp over the region's min..max of the top layer; nswe = blocked directions in colour; layers = layer count heatmap. `--side-by-side <dirB>` puts A | B | difference next to each other.
4. Clear `--help` texts (you're going to run this yourself).

## Out of scope
- GUI integration (geo viewer: later milestone).

## Tests
- Stats/diff numbers equal the Python CSV for 3 regions (copy expected values into the test).
- Render produces a PNG of the right size for a synthetic region.

## Done when
- [ ] tests pass, renders for 12_24, 17_10, 16_10, 14_23 attached to the report
- [ ] committed as `M1-GEO-02: <summary>`
