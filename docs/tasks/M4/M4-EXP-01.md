# M4-EXP-01: Exp/SP rewards and levelling

| Field | Value |
|---|---|
| Milestone | M4 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M4-DEATH-01 |
| Size | M |

## Goal
`Attackable.calculateRewards` exp/SP part: damage share, level-difference penalty, `Rates.ini` (`RateXp`, `RateSp`), `addExpAndSp`, level up (stats refresh, HP/MP/CP rules, level-up `SocialAction`, system messages, `UserInfo`), auto-learn skills on level up (`AutoLearnSkills`), death exp loss.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Attackable.java (`calculateRewards`, `calculateExpAndSp`)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\stat\{PlayableStat,PlayerStat}.java (`addExp`, `addExpAndSp`, `addLevel`)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (`rewardSkills`, `calculateDeathExpPenalty`)`
- config: RatesConfig, PlayerConfig keys

## Files
- `crates/l2-world/src/rewards/exp.rs (new)`
- `crates/l2-world/src/{player.rs,lib.rs} (wire-in)`
- `crates/game-server/src/persist.rs (exp/sp/level columns; skills on level up)`
- `crates/l2-world/tests/exp.rs (new)`

## Requirements
1. Numbers exactly as Java; the `characters` row after a level up is identical to Java's (D-39).

## Out of scope
- Party exp share, vitality (later).

## Tests (must exist and pass)
- Kill rewards for level diffs; level-up 1 to 2 to 5 with skills learned; death penalty.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M4-EXP-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m4-combat.md`, `docs/design/world-loop.md`, `docs/design/m3-world.md` first, then CLAUDE.md safety rules.
- Merged APIs: death/respawn (`crates/l2-world/src/death.rs`, `respawn.rs`), combat (`combat/`: aggro list `AggroInfo` with damage per attacker), status (`status.rs`), inventory (`inventory/`), stats (`stats/`). DEATH-01 left `// M4-EXP-01:` / `// M4-DROP-01:` markers in `death.rs` (`calculate_rewards`, ...): fill yours there.
- M4-DROP-01 runs at the same time and fills the drop half of `calculate_rewards`: keep your edits to that function to the exp/sp part, additive.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
