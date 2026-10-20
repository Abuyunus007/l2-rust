# M6-PARTY-01: Parties

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | GPROTO-09 |
| Size | L |

## Goal
Party invite/accept/leave/kick/leader change, loot distribution types, the party window and member updates, `Party.distributeXpAndSp` (+ `RatePartyXp`/`RatePartySp`, vitality, level-difference cuts), party drops and party sweep (M4-DROP-01/M5-SPOIL-01 hooks), party buffs/heals: every `// M6:` party marker in `skills/targets.rs` and the effect handlers, `PartySpelled`, party in conditions. Ids only (`PartyId`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\Party.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{RequestJoinParty,RequestAnswerJoinParty,RequestWithDrawalParty,RequestOustPartyMember,RequestChangePartyLeader}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Attackable.java (party rewards)`

## Files
- `crates/l2-world/src/party/*.rs (new) + wire-ins (rewards/, skills/targets.rs, effects/, death.rs, leave.rs)`
- `crates/game-server/src/client/enter.rs (dispatch)`
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
- [ ] committed as `M6-PARTY-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
