# M5-SPOIL-01: Spoil and sweep

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
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
- `crates/l2-world/src/effects/handlers/{spoil.rs,sweeper.rs}` (new)
- `crates/l2-world/src/stats/conditions.rs` (`PlayerCanSweep`, Java `ConditionPlayerCanSweep`)
- `crates/l2-world/src/{respawn.rs,npc.rs}` (`SpoiledCorpseExtendTime` in `add_decay_task`; reset spoiler/sweep items on respawn, Java `Attackable.onSpawn`)
- `crates/l2-world/src/combat/mod.rs`, `crates/game-server/src/world_data.rs` (settings: `CalculateMagicSuccessBySkillMagicLevel`, `MinNPCLevelForMagicPenalty`, `SkillChancePenaltyForLvLDifferences` hand-ported like `npc_dmg_penalty`, `SpoiledCorpseExtendTime`)
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
- Merged APIs: the effect handler pattern (`crates/l2-world/src/effects/handlers/`, register in `handlers/mod.rs`, EFX-03's `params.rs` for parameters), M4-DROP-01's `rewards/drop.rs` (`calculate_drops` already rolls the SPOIL list), `death.rs` (`die_packet` sweepable, `add_decay_task` with the `// M4-DROP-01:` SpoiledCorpseExtendTime marker), `inventory/add.rs` (`inventory_add_item_by_id` / Player.addItem for the sweep rewards). Spoil's success uses `effects/probability.rs` / `formulas/effect.rs` like the other chance handlers. M5-CAST-01 and M5-EFX-04 run at the same time: keep `handlers/mod.rs` changes to added lines.
- Findings of the first worker (verified on main): Spoil's success is Java `Formulas.calcMagicSuccess` → `formulas::magic::calc_magic_success` (M5-FORM-02), with a World wrapper building its inputs; Sweeper needs the `canSweep` condition (target dead, spoiled, `checkSpoilOwner`, `isOldCorpse` with `CorpseConsumeSkillAllowedTimeBeforeDecay`, `checkInventorySlotsAndWeight`); the SPOIL roll in `rewards/drop.rs::do_item_drop` goes after the cursed-weapon check and before the DROP roll (RNG order); sweep rewards send YOU_HAVE_EARNED_S1 / YOU_HAVE_EARNED_S2_S1_S like Java's `addItem(SWEEP, ..)`; `Die.sweepable` from the sweep state; respawn reuses the object id, so the spoil state must be reset on respawn.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
