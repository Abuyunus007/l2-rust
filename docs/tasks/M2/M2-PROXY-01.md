# M2-PROXY-01: Field decoding of game packets in proxy and inspector

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M2-GPROTO-01, M2-GPROTO-02, M2-GPROTO-03, M2-GPROTO-04 |
| Size | S |

## Goal
The proxy fills `fields` for every game packet that has a struct in `l2-protocol-game` (decode a copy; never block forwarding). The GUI shows them.

## Java source (the spec)
- none

## Reference implementation to copy
- crates/l2-proxy/src/game.rs, crates/l2-proxy/src/login.rs (how login fields are filled)

## Files
- `crates/l2-proxy/src/game_fields.rs`
- `crates/l2-proxy/src/game.rs (hook)`

## Requirements
1. Credentials/session keys stay redacted (AuthLogin).

## Out of scope
- nothing beyond the files above

## Tests (must exist and pass)
- Unit tests per decoded packet.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-PROXY-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
