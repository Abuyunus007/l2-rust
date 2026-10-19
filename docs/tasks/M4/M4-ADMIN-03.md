# M4-ADMIN-03: Item, level and exp admin commands

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | haiku |
| Review | spot |
| Reference | no (copy M4-ADMIN-02's `crates/l2-world/src/admin/{combat,exp_sp}.rs`) |
| Depends on | M4-ADMIN-02, M4-EXP-01, M4-DROP-01 |
| Size | S |

## Goal
The commands M4-ADMIN-02 could not finish because they need EXP-01 and DROP-01:
`//create_item` (+ the forms of Java's AdminCreateItem: `//create_coin`, `//give_item_target`, ...
as Java registers them), `//set_level`/`//add_level` (AdminLevel), and applying the exp/sp change of
`//add_exp_sp`/`//remove_exp_sp` (the `// M4-EXP-01:` markers in `admin/exp_sp.rs`), incl. the
`MaxSp` clamp.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\admincommandhandlers\{AdminCreateItem,AdminLevel,AdminExpSp}.java`

## Files
- `crates/l2-world/src/admin/*.rs`
- `crates/l2-world/tests/admin_combat.rs` (extend)

## Requirements
1. Use DROP-01's `Inventory.addItem` port (stacking, `ADDED` update, capacity) and EXP-01's `addExpAndSp`/`removeExpAndSp`/`setLevel` ports. No second copy.
2. Command names, access checks and messages exactly as Java.

## Out of scope
- Anything else.

## Tests (must exist and pass)
- Each command's effect, DB-relevant state and messages.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-ADMIN-03: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md first (safety rules).
