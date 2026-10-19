# M4-REGEN-01: HP/MP/CP regeneration

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | haiku |
| Review | spot |
| Reference | no (uses M4-STAT-01's engine) |
| Depends on | M4-STAT-01 |
| Size | S |

## Goal
Java's regeneration: `CreatureStatus.startHpMpRegeneration`/`doRegeneration` with `Formulas.getRegeneratePeriod` and `calcHpRegen`/`calcMpRegen`/`calcCpRegen`, `StatusUpdate` to the player and to whoever Java sends it to (status listeners), started/stopped exactly where Java does (damage, death, full).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\status\{CreatureStatus,PlayableStatus,PlayerStatus,NpcStatus,AttackableStatus}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\stats\Formulas.java (regen part)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\StatusUpdate.java`

## Files
- `crates/l2-world/src/status.rs (new)`
- `crates/l2-world/src/{world.rs,lib.rs} (timer wiring)`
- `crates/l2-world/tests/regen.rs (new)`

## Requirements
1. Timer-wheel task per creature like Java's regen future (no thread).
2. Numbers match M4-JV-01 regen vectors.
3. Status listeners: Java's `addStatusListener` semantics (targeting players see HP bars).

## Out of scope
- Regen from effects/skills (M5).

## Tests (must exist and pass)
- Vector tests, timer start/stop cases.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-REGEN-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md`, `docs/design/world-loop.md`, `docs/design/m3-world.md` first, then CLAUDE.md safety rules.
- The regen formulas are already ported and vector-checked (M4-FORM-01, `crates/l2-world/src/formulas/regen.rs`): fill `HpRegenInput`/`MpRegenInput`/`CpRegenInput` from the stat engine and the world (posture, festival/siege/clan hall factors, mother tree bonus) and call `calc_*_regen` / `get_regenerate_period`. Do not port them again.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
