# M2-DATA-01: l2-data crate + player templates (reference loader)

| Field | Value |
|---|---|
| Milestone | M2 |
| Model | sonnet |
| Review | full |
| Reference | yes (pattern for every XML loader) |
| Depends on | none |
| Size | M |

## Goal
`l2-data` with the XML loading pattern (quick-xml + serde, immutable tables behind `Arc`) and the first loader: player class templates and the class list.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\PlayerTemplateData.java, ClassListData.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\commons\util\IXmlReader.java (what the loaders do: parse, validate, count log line)`
- `C:\l2-rust\l2jmobius\dist\game\data\stats\players\templates\*.xml, classList.xml`
- `C:\l2-rust\l2jmobius\dist\game\data\xsd\ (schemas)`

## Reference implementation to copy
- none (this card creates it); config pattern in crates/l2-config

## Files
- `crates/l2-data/{Cargo.toml,src/lib.rs,src/loader.rs,src/player_template.rs,src/class_list.rs}`
- `crates/l2-data/tests/player_templates.rs`

## Requirements
1. `Datapack::load(root)` style entry point that later loaders extend; each loader logs Java's count line (e.g. "Loaded 89 player templates.").
2. Templates hold what character creation and UserInfo need: base stats, collision radius/height per sex, creation points (spawn locations), hp/mp/cp per level tables, class id/race/parent.
3. Unknown elements/attributes: tolerated or rejected exactly like the Java loader.

## Out of scope
- Items, skills (other cards).

## Tests (must exist and pass)
- Loads the real dist files (skip if absent) with Java's counts; unit tests on small inline XML.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M2-DATA-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/world-loop.md` and the CLAUDE.md safety rules first (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
