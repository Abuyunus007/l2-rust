# M4-ADMIN-02: Admin commands for combat testing

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | haiku |
| Review | spot |
| Reference | no (copy M3-ADMIN-01 handlers) |
| Depends on | M4-DEATH-01 |
| Size | S |

## Goal
Resurrect, set level, create item, invulnerability, spawn with respawn, kill with real death, add exp/sp: command names and behaviour exactly as Java's admin command handlers (look them up in the handler files).

## Java source (the spec)
- (dist) game\data\scripts\handlers\admincommandhandlers\* (AdminRes, AdminLevel, AdminCreateItem, AdminKill, AdminSpawn, AdminExpSp, the invul command: grep)

## Files
- `crates/l2-world/src/admin/*.rs (new handlers)`
- `crates/l2-world/tests/admin_combat.rs (new)`

## Requirements
1. Command names and access checks from `AdminCommands.xml` like M3-ADMIN-01.

## Out of scope
- Everything else.

## Tests (must exist and pass)
- Each command's effect and messages.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-ADMIN-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md`, `docs/design/world-loop.md`, `docs/design/m3-world.md` first, then CLAUDE.md safety rules.
- Merged APIs: death/respawn (`crates/l2-world/src/death.rs`, `respawn.rs`), combat (`combat/`: aggro list `AggroInfo` with damage per attacker), status (`status.rs`), inventory (`inventory/`), stats (`stats/`). DEATH-01 left `// M4-EXP-01:` / `// M4-DROP-01:` markers in `death.rs` (`calculate_rewards`, ...): fill yours there.
- DEATH-01 notes: `//kill` (admin/commands.rs) still deletes the NPC and only logs for a player (FOLLOW-UP(M4)): make it Java's kill (`reduceCurrentHp(maxHp + 1)` style, see AdminKill) and update its test to Java's behaviour. `//spawn` with a respawn time needs a real spawn (Java `AdminSpawn` creates a `Spawn` with the delay).
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
