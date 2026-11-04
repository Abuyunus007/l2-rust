# M5-SHOT-01: Soulshots and spiritshots

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M5-CAST-01 |
| Size | M |

## Goal
SoulShots, SpiritShot, BlessedSpiritShot handlers, the charge state on the weapon (`isChargedShot`/`setChargedShot`), auto shots (`RequestAutoSoulShot`, `ExAutoSoulShot`, `rechargeShots` after each attack/cast), the shot animation (`MagicSkillUse` of the shot skill), the effect on damage (M4-COMBAT-01's `ss` flag and Attack hit flags, magic sps/bss through FORM-02), grade checks and messages.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\items\{SoulShots,SpiritShot,BlessedSpiritShot}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (rechargeShots, addAutoSoulShot, removeAutoSoulShot, disableAutoShot*)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\item\instance\Item.java (charged shots)`

## Files
- `crates/l2-world/src/inventory/handlers/shots.rs (new)`
- `crates/l2-world/src/combat/*.rs, skills/cast.rs (charge consumption)`
- `crates/game-server/src/client/* (dispatch)`
- `crates/l2-world/tests/shots.rs (new)`

## Requirements
1. Damage with/without shots matches the vectors; consumption counts and messages as Java.

## Out of scope
- Beast/fish shots (M6).

## Tests (must exist and pass)
- Auto shots on/off, consumption, grade mismatch, damage doubling.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-SHOT-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- Casting is merged (M5-CAST-01, `crates/l2-world/src/skills/cast.rs`): players and items cast through `World::use_magic(player, SkillKey, force_use, dont_move)`; potions/herbs through `do_simultaneous_cast`; NPCs through `set_npc_target(npc, Some(t))` + `do_cast(npc, key)`; triggered skills `do_cast_at`; reuse in `skills/reuse.rs`; `// M5-SHOT-01:` markers (spiritshot 0.6x cast time, `rechargeShots` at the hit and finalizer, the shot flags in `effects/apply.rs` and EFX handlers) are the shots card's to fill.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
