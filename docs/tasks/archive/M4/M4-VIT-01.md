# M4-VIT-01: Vitality and Nevit's blessing

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | sonnet |
| Review | full |
| Reference | no (extends M4-EXP-01's `crates/l2-world/src/rewards/exp.rs`) |
| Depends on | M4-EXP-01 |
| Size | M |

## Goal
The shipped config has `EnableVitality=True`, so Java multiplies kill exp by the vitality level's
rate (1.5x to 3x) and Kain gives less until this exists. Port vitality points and levels
(`PlayerStat.getVitalityLevel`, `updateVitalityPoints` after a kill, `getVitalityMultiplier`,
the recovery while offline/at peace as Java does it, `ExVitalityPointInfo`, the `vitality_points`
DB column / variable exactly as Java stores it) and Nevit (`Nevit.java`: points per kill,
blessing, hourglass/advent tasks, `ExNevitAdventPointInfo`/`ExNevitAdventTimeChange`/
`ExNevitAdventEffect`, `ExVoteSystemInfo`) as far as kill exp and the enter-world packets need.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\stat\PlayerStat.java` (vitality parts)
- `...\entity\actor\Attackable.java` (vitality in `calculateRewards`), `...\entity\actor\Player.java` (Nevit, vitality store/restore)
- `...\entity\Nevit.java` (or wherever it lives: grep), `...\config\` keys `EnableVitality`, `RateVitality*`, `NevitEnabled`, ...
- Network packets named above.

## Files
- `crates/l2-world/src/rewards/{exp.rs,vitality.rs,nevit.rs}`
- `crates/l2-world/src/{enter.rs,world.rs,lib.rs,timer.rs}` (wire-in, additive)
- `crates/l2-protocol-game/src/server/` (missing packets, with round trips)
- `crates/game-server/src/{persist.rs,world_data.rs}` and `crates/l2-db` (the stored values, exactly Java's columns)
- tests next to them

## Requirements
1. Same numbers as Java for exp with vitality at each level; same DB values (D-39: the character moves between Kain and Sieghardt).
2. Timers on the world's timer wheel; config keys and defaults as Java.

## Out of scope
- Vitality items/skills effects (M5).

## Tests (must exist and pass)
- Exp per vitality level, points after kills, level thresholds, store/restore, packets.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-VIT-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md, `docs/design/m4-combat.md` first. M4-DROP-01 and M4-AI-02 may run at the same time: keep shared-file changes additive.
