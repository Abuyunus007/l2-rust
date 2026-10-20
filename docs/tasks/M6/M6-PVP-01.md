# M6-PVP-01: PvP flag, karma, PK, relations

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | GPROTO-09 |
| Size | M |

## Goal
`Player.updatePvPStatus` (flag on attacking/casting at a player outside zones), `PvpFlagTaskManager` (flag end, blinking), karma gain/loss (`Formulas.calculateKarmaGain/Lost`, `onKillUpdatePvPKarma`, karma drop through M4-DROP-01's `onDieDropItem`), PK count, name colors (PvP/PK name color configs), `RelationChanged` with Java's real relation masks and `isAutoAttackable` for players (replace the M3 stubs in `player_view.rs` and the `pvp_flag_of` placeholder), karma in the target handlers and conditions. `characters` karma/pvpkills/pkkills columns exactly as Java (D-39).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (updatePvPStatus, onKillUpdatePvPKarma, getRelation, isAutoAttackable, setKarma)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\taskmanagers\PvpFlagTaskManager.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\mechanics\stats\Formulas.java (karma)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\RelationChanged.java`

## Files
- `crates/l2-world/src/pvp.rs (new) + wire-ins (combat/, skills/, player_view.rs, effects/control.rs, death.rs)`
- `crates/game-server/src/{persist.rs,world_data.rs}`
- `tests`

## Requirements
1. Java behaviour, messages, packets and DB rows exactly (D-39: the Java GS shares the DB).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data.
3. Resolve the `// M6:` markers in your area; list the ones you leave.

## Out of scope
- Sieges/castles/fortresses/clan halls (M7), Olympiad/Seven Signs (M8), quests (later).

## Tests (must exist and pass)
- Each part with Java's numbers/messages/packets; DB tests skip without `L2_TEST_DATABASE_URL`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M6-PVP-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
