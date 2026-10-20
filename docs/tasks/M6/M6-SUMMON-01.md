# M6-SUMMON-01: Summons: servitors

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | opus |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | GPROTO-09 |
| Size | XL |

## Goal
Servitors as creatures with an owner (`docs/design/m6-social.md`): the `Summon` effect handler, summon info/status packets to owner and viewers, follow/attack/stay AI through the M4 intention machine, summon actions (`RequestActionUse` summon branch), shared exp (`ServitorShare`, exp penalty), consumption over time, unsummon/death/owner logout, summon targets and shots (`// M6:` markers in targets, effects, `inventory/handlers/shots.rs`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\{Summon,instance\Servitor}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\ai\SummonAI.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\{Summon,ServitorShare,Unsummon}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\RequestActionUse.java`

## Files
- `crates/l2-world/src/summon/*.rs (new) + wire-ins`
- `crates/l2-world/src/effects/handlers/{summon,servitor_share,unsummon}.rs`
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
- [ ] committed as `M6-SUMMON-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
