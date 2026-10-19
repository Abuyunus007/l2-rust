# M5-SPOIL-01: Spoil and sweep

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M5-EFX-03 |
| Size | S |

## Goal
Spoil and Sweeper effects, the spoil roll (M4-DROP-01's calculator already rolls SPOIL), `Die.sweepable`, `SpoiledCorpseExtendTime`, the sweep rewards into the inventory with Java's messages.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\{Spoil,Sweeper}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Attackable.java (spoil parts, isSpoiled, getSpoilLootItems)`

## Files
- `crates/l2-world/src/effects/handlers/spoil.rs (new)`
- `crates/l2-world/src/{rewards/drop.rs,death.rs} (spoil parts)`
- `crates/l2-world/tests/spoil.rs (new)`

## Requirements
1. Seeded RNG; messages/packets as Java.

## Out of scope
- Manor (later).

## Tests (must exist and pass)
- Spoil success/fail, sweep, corpse time extension.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-SPOIL-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
