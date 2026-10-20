# M5-FIX-03: M5 follow-ups (item reuse save, ClassBalance skill tables, old corpses, maps)

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | spot |
| Reference | no |
| Depends on | M5-ITEM-01, M5-EFX-04, M5-SAVE-01 |
| Size | M |

## Goal
Small gaps the M5 workers reported:
1. Item reuse across relog: Java `character_item_reuse_save` (`storeItemReuseDelay` / `restoreItemReuse`), from `ReuseState.item_stamps` (M5-ITEM-01), next to M5-SAVE-01's skill save. Exact Java SQL (D-39).
2. ClassBalance skill tables: the `ClassBalance.ini` tables that M5-FORM-02/EFX-04 read as 1.0 (`M5-CFG` markers: skill crit chance/damage, blow, energy, PvE magical skill damage, healing, skill mastery chance, ...) into `StatSettings`, read in `game-server/src/world_data.rs` like the existing physical tables.
3. `skills/targets.rs::corpse_mob`: Java `isOldCorpse(player, CorpseConsumeSkillAllowedTimeBeforeDecay, true)` for HP_DRAIN skills (`World::is_old_corpse` / `decay_remaining_ms` exist since M5-SPOIL-01).
4. `ShowMiniMap` and `ShowCalculator` packets + the Maps and Calculator item handlers (M5-ITEM-01's registry).

## Java source (the spec)
- `Player.java` (`storeItemReuseDelay`, `restoreItemReuse`), `config/ClassBalanceConfig.java`, `entity/actor/Attackable.java` (`isOldCorpse`), `handlers/skill/targets/CorpseMob.java`, `handlers/items/{Maps,Calculator}.java`, the two server packets.

## Files
- `crates/l2-db`, `crates/game-server/src/{persist.rs,world_data.rs,client/enter.rs}`, `crates/l2-world/src/{stats/mod.rs,skills/targets.rs,inventory/handlers/*}`, `crates/l2-protocol-game` (2 packets), tests.

## Requirements
1. Java behaviour, SQL and config keys exactly; each part tested.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-FIX-03: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules) first.
