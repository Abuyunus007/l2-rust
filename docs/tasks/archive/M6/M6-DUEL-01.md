# M6-DUEL-01: Duels

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | spot |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | PARTY-01, PVP-01 |
| Size | S |

## Goal
Duel request/answer, 1:1 and party duels, the duel countdown and end conditions, duel HP restore, messages and packets as Java.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\DuelManager.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\Duel.java (grep)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\RequestDuel*.java`

## Files
- `crates/l2-world/src/duel.rs (new)`
- `tests`

## Requirements
1. Java behaviour, messages, packets and DB rows exactly (D-39: the Java GS shares the DB).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data.
3. Resolve the `// M6:` markers in your area; list the ones you leave.

- From M6-PVP-01: wire `FlagPvPZone` in `zones.rs` (Java `FlagZone.onEnter/onExit`: `updatePvPFlag(1)` / `updatePvPFlag(0)`); fill the `M6-DUEL` markers in `pvp.rs` (`updatePvPStatus`, `isAutoAttackable`, `onKillUpdatePvPKarma`). `NicknameChanged` exists now (`server::visibility`).

## Out of scope
- Sieges/castles/fortresses/clan halls (M7), Olympiad/Seven Signs (M8), quests (later).

## Tests (must exist and pass)
- Each part with Java's numbers/messages/packets; DB tests skip without `L2_TEST_DATABASE_URL`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M6-DUEL-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- GPROTO-09 did not port the duel packets: add them in `crates/l2-protocol-game/src/{client,server}/duel.rs` with round-trip tests, plus dispatch arms (`RequestDuelStart`, `RequestDuelAnswerStart`, `RequestDuelSurrender`; `ExDuelAskStart`, `ExDuelReady`, `ExDuelStart`, `ExDuelEnd`, `ExDuelUpdateUserInfo`). Check opcodes in `generated/opcodes.rs` and PACKET_DOCS.
- Parties are merged (`party/`, `World::party_of`); PvP is merged (`pvp.rs`, `M6-DUEL` markers in `update_pvp_status`, `is_auto_attackable`, `on_kill_update_pvp_karma`); trade has `World::cancel_active_trade` (Java calls it on duel start); the request state is `trade/request.rs` (`active_requester`, `on_transaction_request`, ...).
- Duel countdown and checks run on the timer wheel, not threads.
- Other M6 workers run at the same time (CLAN-01, SUMMON-01). In shared files change only your lines; no reformatting or moving code.
- Use `-j 4` for cargo clippy/test (parallel linking crashed under load) and CARGO_INCREMENTAL=0.
