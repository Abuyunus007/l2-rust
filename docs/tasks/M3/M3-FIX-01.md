# M3-FIX-01: Name templates (regex) and the html encoding check flag

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | haiku |
| Review | spot |
| Reference | no |
| Depends on | M2-CHAR-01, M3-CFG-02 |
| Size | S |

## Goal
Two M2 leftovers:
1. `CnameTemplate`, `PetNameTemplate`, `ClanNameTemplate` (Server.ini) are Java regexes. Today only `.*` works and any other pattern refuses every name (fail closed). Compile them with the `regex` crate (approved for this card, D-28) and match like Java (`Pattern.matcher(name).matches()` = whole-string match). An invalid pattern falls back to `.*` with a warning, as Java does.
2. `CheckHtmlEncoding` (General.ini, Java default true): the html loader warns about non-ASCII files unconditionally; gate it on the flag like `HtmCache.java` does, through `DatapackOptions` (M3-CFG-02) and `options_from_config`.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\config\ServerConfig.java` (the three patterns, fallback)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\CharacterCreate.java` (where CnameTemplate is used)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\cache\HtmCache.java` (CHECK_HTML_ENCODING)

## Reference implementation to copy
- crates/l2-config/src/game_manual.rs (the raw pattern strings), crates/game-server/src/client/charscreen.rs (name validation), crates/l2-data/src/{lib.rs,html.rs}

## Files
- workspace `Cargo.toml` + `crates/l2-config/Cargo.toml` (`regex`), `crates/l2-config/src/game_manual.rs`
- `crates/game-server/src/client/charscreen.rs` (use the compiled pattern)
- `crates/l2-data/src/{lib.rs,html.rs}`, `crates/game-server/src/lib.rs` (`options_from_config`)
- docs/DECISIONS.md: one line noting `regex` is approved (D-28)

## Requirements
1. Java regex features the shipped/typical patterns use must work (character classes, quantifiers, anchors). Rust `regex` lacks look-around/backreferences: such a pattern logs a warning and falls back to `.*` (document it).
2. No other behaviour change.

## Out of scope
- Using the pet/clan patterns (no pets or clans yet): compile and expose them only.

## Tests (must exist and pass)
- Pattern parsing/fallback, whole-string matching (`[A-Za-z0-9]{1,16}` accepts/refuses), CharacterCreate refusing a name that the template refuses; html check on/off.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-FIX-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read the CLAUDE.md safety rules first (never touch a local database or real ports).
