# M7-ADMIN-10: Admin: punishments, kick, server control, announcements, petitions

| Field | Value |
|---|---|
| Milestone | M7 (admin completion) |
| Model | sonnet |
| Review | full |
| Reference | `crates/l2-world/src/admin/` (the merged ADMIN-01..06 handlers, their registry and rights) |
| Depends on | none |
| Size | L |

## Goal
Port the Java admin command handlers listed below completely: every command they register (`getCommandList`), with Java's messages, HTML pages (`data/html/admin/*.htm`, the same files Java sends) and side effects. Ban/unban account and character, chat ban, jail/unjail (`PunishmentManager`, `punishments` rows, the jail zone, the menu links `ban_acc`, `ban_char`, `ban_chat`, `jail`, ...), kick and kick_non_gm, disconnect, the login server switches (`server_gm_only`, `server_all`, `server_max_player`, `server_list_clock`, `server_login`: through the existing GS-LS link, `ServerStatus`), shutdown/restart/abort with Java's countdown messages (Kain only stops itself; it never touches the Java GS), announcements (`announcements` table, normal/critical/auto, the list pages), GM chat and gm, online, server info, petitions (`PetitionManager` with its packets; take it out of the M8 MISC card), reload (only what Rust can reload; say which), packet forge, debug. **Side by side:** punishments are rows Sieghardt also reads; write them as Java does (they belong to the player, D-39), and read the active ones at start like Java. The account ban goes through the login server as in Java.

Every `bypass -h admin_*` link in the admin HTML pages these handlers send must work afterwards. Commands that belong to M8 or the scripts (Olympiad, heroes, instances, grand bosses, cursed weapons, Mammon, quests, Hellbound, Gracia seeds, events, premium, game points, fake players, HWID) stay out: answer them like Java answers an unknown command and list them in your report.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminPunishment.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminKick.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminDisconnect.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminLogin.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminShutdown.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminAnnouncements.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminGmChat.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminGm.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminOnline.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminServerInfo.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminPetition.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminReload.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminPForge.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminDebug.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\AdminHwid.java`
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
- [ ] committed as `M7-ADMIN-10: <summary>`
- [ ] report delivered in the agent's report format (list every command you left and why)

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS first. M0..M7 are merged.
- Three other admin workers run at the same time on other handlers: in `admin/commands.rs`, `admin/rights.rs`, `admin/mod.rs`, `lib.rs` only add your lines. Before your final checks run `git merge main`, resolve conflicts keeping both sides, watch for doubled `mod`/`use` lines.
