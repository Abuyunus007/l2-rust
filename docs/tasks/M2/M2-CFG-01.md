# M2-CFG-01: Hand-ported game config fields needed by M2

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M2-GEN-02 |
| Size | S |

## Goal
Port the non-simple config fields that M2 code reads (lists, maps, patterns, derived values) by hand, matching Java exactly.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\config\ServerConfig.java (AllowedProtocolRevisions, CnameTemplate, … ), PlayerConfig.java, GeneralConfig.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\config\custom\{AllowedPlayerRacesConfig,StartingLocationConfig,StartingTitleConfig,FactionSystemConfig}.java`

## Reference implementation to copy
- the generated fields in crates/l2-config/src/generated/game/ and docs/analysis/config-manual-fields.md

## Files
- `crates/l2-config/src/game_manual.rs (or per-class manual modules)`
- `crates/l2-config/src/game.rs (wire them in)`

## Requirements
1. Only fields used by M2 cards: protocol revisions list, name templates (regex), forbidden names, starting adena/level/sp/vitality, max characters per account, allowed races, starting location, starting title, faction starting locations, character deletion days, and whatever CHAR-01/ENTER-01 report missing.
2. Each field: Java line in a comment, same parsing behaviour (split rules, defaults).

## Out of scope
- Fields no M2 card uses (left as TODO for later milestones).

## Tests (must exist and pass)
- Each ported field parses the dist value and the empty default.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-CFG-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
