# M7-ADMIN-11: Admin: geodata, regions, zones, zone build, path nodes

| Field | Value |
|---|---|
| Milestone | M7 (admin completion) |
| Model | sonnet |
| Review | spot |
| Reference | `crates/l2-world/src/admin/` (the merged ADMIN-01..06 handlers, their registry and rights) |
| Depends on | none |
| Size | L |

## Goal
Port the Java admin command handlers listed below completely: every command they register (`getCommandList`), with Java's messages, HTML pages (`data/html/admin/*.htm`, the same files Java sends) and side effects. Geodata commands (`geo_z`, `geo_type`, `geo_nswe`, `geo_los`, `geo_position`, `geo_bug`, the geodata info pages) on the existing `l2-geo` crate, region info, zone check and zone visual (`ExServerPrimitive` drawing of zones), zone build (draw, toggle, clear, save to a local file as Java does: never into the repo), path nodes.

Every `bypass -h admin_*` link in the admin HTML pages these handlers send must work afterwards. Commands that belong to M8 or the scripts (Olympiad, heroes, instances, grand bosses, cursed weapons, Mammon, quests, Hellbound, Gracia seeds, events, premium, game points, fake players, HWID) stay out: answer them like Java answers an unknown command and list them in your report.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminGeodata.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminRegions.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminZone.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminZoneBuild.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminPathNode.java`
- The pages: `C:\l2-rust\l2jmobius\dist\game\data\html\admin` (grep the commands)

## Files
- `crates/l2-world/src/admin/<area>.rs` (new files per handler group) + the registry in `admin/commands.rs` and `admin/rights.rs` (add your lines only)
- wire-ins where a command needs world state that exists already
- `crates/game-server/src/*` only where a command needs the persistence task or the login server link
- tests

## Requirements
1. Java behaviour, messages, packets, HTML and DB rows exactly (D-39: the Java GS shares the DB; D-49..D-52 for global writes).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data; no DB call or await in a tick (D-15: request/answer through the outbox).
3. Port the packets you need yourself in `crates/l2-protocol-game/src/server/<area>.rs` with round-trip tests and proxy decoders.
4. Access levels from `AdminCommandAccessRights.xml` as the existing rights code reads them.

## Tests (must exist and pass)
- One test per command group at least: the command, its message or page, its effect. A test that every command of your handlers is registered.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `M7-ADMIN-11: <summary>`
- [ ] report delivered in the agent's report format (list every command you left and why)

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS first. M0..M7 are merged.
- Three other admin workers run at the same time on other handlers: in `admin/commands.rs`, `admin/rights.rs`, `admin/mod.rs`, `lib.rs` only add your lines. Before your final checks run `git merge main`, resolve conflicts keeping both sides, watch for doubled `mod`/`use` lines.
