# S-HAND-01: Shift-click GM windows and the community board

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M0..M7 code around it) |
| Depends on | none |
| Size | M |

## Goal
`handlers/actions/shiftclick/*` (NpcShiftClick with npcinfo pages and the `%route%` row, PlayerShiftClick, DoorShiftClick, SummonShiftClick, ItemShiftClick, StaticObjectShiftClick: the `FOLLOW-UP(admin)` in `target.rs`), `handlers/actions/click/*` if not yet ported, the community board (`RequestShowBoard`, `RequestBBSwrite`, `handlers/bypass/communityboard/*`: HomeBoard, HomepageBoard, RegionBoard, FavoriteBoard, ClanBoard, MailBoard, MemoBoard, FriendsBoard, DropSearchBoard, with their tables `bbs_favorites` etc.), and every handler under `handlers/bypass`, `handlers/chat` (user and voiced commands), `handlers/items`, `handlers/punishments` that is still not registered in Rust (check the registries first and list what you found).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\handlers\actions\**`
- `C:\l2-rust\l2jmobius\dist\game\data\handlers\bypass\communityboard\*.java`
- `C:\l2-rust\l2jmobius\dist\game\data\handlers\bypass\npc\*.java`
- `C:\l2-rust\l2jmobius\dist\game\data\handlers\chat\commands\ (user, voiced)`

## Files
- `crates/l2-world/src/{shiftclick,community_board}/*` (new) + the registries
- crates/game-server, crates/l2-db (board tables)
- tests

## Requirements
1. Java behaviour, messages, packets, HTML and DB rows exactly (D-39: the Java GS shares the DB; D-49..D-54 for global writes and fresh reads).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data; no DB call or await in a tick (D-15: request/answer through the outbox).
3. Port the packets you need yourself in `crates/l2-protocol-game/src/{client,server}/<area>.rs` with round-trip tests, dispatch arms and proxy decoders (unless M8-GPROTO-11 already has them: check main first).
4. Small wire-ins in shared files are allowed: add your lines only. List every marker you resolve and every one you leave.

## Out of scope
- The customs in `scripts/custom/`, premium, item mall (M9).

## Tests (must exist and pass)
- Each window/board page with Java's HTML; a registry test listing every Java handler as ported or left with its reason.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-HAND-01: <summary>`
- [ ] report delivered in the agent's report format (what you left and why, every guess)

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS (D-15, D-39, D-49..D-54), the `docs/design/*.md` of the area you touch first.
- Up to seven workers run at the same time: in shared files (`lib.rs`, `world.rs`, `command.rs`, `outbox.rs`, `timer.rs`, `dispatch.rs`, `persist.rs`, `world_data.rs`) only add your lines; after `git merge main` watch for doubled `mod`/`use` lines.
- DB tests use `l2_db::GAME_SCHEMA`; add a schema copy to `crates/l2-db/sql/game/` for every new table.
