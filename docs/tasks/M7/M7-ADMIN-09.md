# M7-ADMIN-09: Admin: edit char, spawn, admin page, items, doors, shops

| Field | Value |
|---|---|
| Milestone | M7 (admin completion) |
| Model | haiku |
| Review | spot |
| Reference | `crates/l2-world/src/admin/` (the merged ADMIN-01..06 handlers, their registry and rights) |
| Depends on | none |
| Size | L |

## Goal
Port the Java admin command handlers listed below completely: every command they register (`getCommandList`), with Java's messages, HTML pages (`data/html/admin/*.htm`, the same files Java sends) and side effects. The rest of edit char (character info and list pages, find character/account/IP/dualbox, set hp/mp/cp, name, title, colours, sex, class, noble, fame, recommendations, food), the rest of spawn (index pages, reload, day/night, respawn and unspawn all, NPC index), the rest of the admin page (`gmliston`/`gmlistoff`, `silence`, `diet`, `tradeoff`, `set`/`setconfig`/`config_server` pages, `set_mod`), delete, repair char, scan, search, destroy items, the GM shop (`buy`, `gmshop`), door control (`open`/`close`/`openall`/`closeall`), fences, change access level, fight calculator. `setclass` changes the class as Java's handler does (no class change quest needed). `changename` checks the name as character creation does.

Every `bypass -h admin_*` link in the admin HTML pages these handlers send must work afterwards. Commands that belong to M8 or the scripts (Olympiad, heroes, instances, grand bosses, cursed weapons, Mammon, quests, Hellbound, Gracia seeds, events, premium, game points, fake players, HWID) stay out: answer them like Java answers an unknown command and list them in your report.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminEditChar.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminSpawn.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminAdmin.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminDelete.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminRepairChar.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminScan.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminSearch.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminDestroyItems.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminShop.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminDoorControl.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminFence.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminChangeAccessLevel.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminFightCalculator.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminTest.java`
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
- [ ] committed as `M7-ADMIN-09: <summary>`
- [ ] report delivered in the agent's report format (list every command you left and why)

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS first. M0..M7 are merged.
- Three other admin workers run at the same time on other handlers: in `admin/commands.rs`, `admin/rights.rs`, `admin/mod.rs`, `lib.rs` only add your lines. Before your final checks run `git merge main`, resolve conflicts keeping both sides, watch for doubled `mod`/`use` lines.
