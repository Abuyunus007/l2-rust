# <CARD-ID>: <short title>

<!-- Card ID format: M<milestone>-<track>-<nn>, e.g. M1-CRYPTO-01. File name: docs/tasks/M1/M1-CRYPTO-01.md -->

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | haiku / sonnet / opus |
| Review | full / spot |
| Reference | yes (this card creates a pattern others copy) / no |
| Depends on | <card ids that must be merged first, or "none"> |
| Size | S (< 300 lines changed) / M (< 800). Split anything bigger. |

## Goal
One or two sentences: what exists after this card that didn't before.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\...\Foo.java`: what to take from it (whole class / method `bar()` / lines N–M)

## Reference implementation to copy
- `crates/<crate>/src/<file>.rs`: follow its structure, naming and test style. ("none" only on Reference: yes cards)

## Files
Create:
- `crates/<crate>/src/<file>.rs`
Modify:
- `crates/<crate>/src/lib.rs` (add `mod` line only)
Nothing else may change.

## Requirements
1. Concrete, checkable statements. E.g. "`encrypt()` output matches `java-vectors/out/login_encrypt.json` for all cases."
2. Edge cases from the Java code that must be kept (signedness, padding, defaults).
3. Error handling: which inputs fail and how.

## Out of scope
- Things the worker must NOT do here, even if they seem related.

## Tests (must exist and pass)
- `<test name>`: what it proves
- Round-trip / vector / capture tests as required by `CLAUDE.md`

## Done when
- [ ] all requirements met
- [ ] `cargo fmt --all`, `cargo clippy --workspace --all-targets -- -D warnings`, `cargo test --workspace` pass
- [ ] committed on the card branch as `<CARD-ID>: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
Anything learned from earlier cards, pitfalls, links to analysis docs.
