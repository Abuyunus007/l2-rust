# M5-GPROTO-08: Skill, buff and item-use packets

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | haiku |
| Review | spot |
| Reference | no (copy M4-GPROTO-07: `server/combat.rs`, `client/combat.rs`, dispatch, proxy field decoders) |
| Depends on | none |
| Size | M |

## Goal
- Server: `MagicSkillUse`, `MagicSkillLaunched`, `MagicSkillCanceled`, `AbnormalStatusUpdate`, `ShortBuffStatusUpdate`, `ExAutoSoulShot`, `AcquireSkillList`, `AcquireSkillInfo`, `AcquireSkillDone`, `PartySpelled` (structure only; sent in M6).
- Client: `RequestMagicSkillUse`, `RequestAutoSoulShot`, `RequestAcquireSkill`, `RequestAcquireSkillInfo`, `RequestDispel`.
- Wire the client packets into `dispatch.rs`, add field decoders in `crates/l2-proxy/src/game_fields.rs` (every exhaustive `ClientPacket` match in the workspace gets its arms).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\{MagicSkillUse,MagicSkillLaunched,MagicSkillCanceled,AbnormalStatusUpdate,ShortBuffStatusUpdate,ExAutoSoulShot,AcquireSkillList,AcquireSkillInfo,AcquireSkillDone,PartySpelled}.java`
- `...\network\clientpackets\{RequestMagicSkillUse,RequestAutoSoulShot,RequestAcquireSkill,RequestAcquireSkillInfo,RequestDispel}.java` (read part only)
- `C:\l2-rust\PACKET_DOCS\` (quick map)

## Files
- `crates/l2-protocol-game/src/server/skills.rs`, `client/skills.rs` (new), `server/mod.rs`, `client/mod.rs`, `dispatch.rs`
- `crates/l2-proxy/src/game_fields.rs`
- `crates/l2-protocol-game/tests/skills.rs` (new)
- any other exhaustive `ClientPacket` match that breaks (add arms only)

## Requirements
1. Byte layout exactly as Java; Java's flag/enum constants provided as consts.
2. Byte checks against `fixtures/captures/java-gs-*.jsonl` for every packet the fixtures contain; local recordings through `L2_CAPTURE_DIR` (never commit them).

## Out of scope
- Handlers (M5-CAST-01, M5-SHOT-01, M5-LEARN-01, ...).

## Tests (must exist and pass)
- Round trip + layout per packet, fixture comparison where recorded, dispatch tests.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-GPROTO-08: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m5-skills.md` first, then CLAUDE.md safety rules.
