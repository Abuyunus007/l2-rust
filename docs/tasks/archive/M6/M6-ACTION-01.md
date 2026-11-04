# M6-ACTION-01: Player actions (sit, stand, walk, run, socials)

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | spot |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | SUMMON-01, STORE-01 |
| Size | M |

## Goal
The rest of `RequestActionUse` for players: sit/stand (action 0, with the sitting rules and `ChangeWaitType`, the sit/stand checks other code already asks for), walk/run (action 1, `ChangeMoveType`), the social actions (`RequestSocialAction` and the action-list socials, `SocialAction` broadcast, the refusals while casting/moving/fighting), couple actions (`ExAskCoupleAction`, `RequestExAskCoupleAction` answer, `isInDuel` refusal), the bot report action as a marker. STORE-01 routes ids 10/28/61 and SUMMON-01 the summon ids: keep those arms, add the player ids to the same handler.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{RequestActionUse,RequestSocialAction,RequestExAskCoupleAction}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (sitDown, standUp)`

## Files
- `crates/l2-world/src/actions.rs (new)`
- `crates/l2-protocol-game (any missing packet)`
- `crates/game-server/src/client/enter.rs`
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
- [ ] committed as `M6-ACTION-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- Merged: `summon/actions.rs` (summon branch), `trade/store.rs` (store ids), `transform/rules.rs` (`transformed_action_allowed`), `duel.rs` (`is_in_duel`).
- Up to seven M6 workers run at the same time. In shared files (`skills/targets.rs`, `enter.rs`, `player_view.rs`, `leave.rs`, `death.rs`, `world.rs`, `dispatch.rs`, `client/enter.rs`) only add your lines: no reformatting or moving code.
- Use `-j 4` for cargo clippy/test (parallel linking crashed under load) and CARGO_INCREMENTAL=0.
