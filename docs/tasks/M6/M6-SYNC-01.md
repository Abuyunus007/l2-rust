# M6-SYNC-01: Fresh shared rows for mail and the clan warehouse

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the async load pattern of CLAN-01's clan record and PET-01's `PetRowRequest`) |
| Depends on | MAIL-01, WH-01 |
| Size | M |

## Goal
D-39 runs the Java GS and this server on one database. MAIL-01 loads every mail and its attachments once at startup, and WH-01 keeps each clan warehouse in memory from the clan load. A mail received or a clan warehouse item withdrawn on the Java GS after this server started is still in this server's memory, so a player could take the same items again (duplicate rows, duplicate object ids). Close that gap without waiting on the database inside a tick:
1. **Mail:** at CharacterSelect (the async loader, like the clan record of CLAN-01) read the character's mails as receiver and sender with their attachments, and replace this server's in-memory copies of exactly those messages (add new ones, drop ones the DB no longer has, refresh attachments). Before `RequestPostAttachment` moves items, re-read that message's attachment rows through the outbox and a world command back (PET-01's `PetRowRequest` / `WorldCommand::PetRowLoaded` pattern); only rows still `loc=MAIL` with that `loc_data` are handed out.
2. **Clan warehouse:** when a clan warehouse window is opened (`WithdrawC`, `WithdrawSortedC`, `DepositC`), re-read the clan's `CLANWH` rows the same way and replace the in-memory container before the list is sent. A withdraw only moves items that are still in the refreshed container.
3. **Mail expiry:** the expiry task of MAIL-01 deletes rows and returns attachments on its own schedule. Gate those writes behind the D-49 switch in `config/SideBySide.ini` (a new key `MailExpiryDatabaseWrites = False`, documented in the example file next to `ClanMaintenanceDatabaseWrites`), so only the Java GS does them while both run; the in-memory expiry still happens.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\managers\MailManager.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\item\container\{Mail,ClanWarehouse,ItemContainer}.java`
- `docs/DECISIONS.md` D-39, D-49

## Files
- `crates/l2-world/src/mail/*.rs`, `crates/l2-world/src/warehouse/*.rs`
- `crates/l2-world/src/{outbox.rs,command.rs,world.rs}` (new request/answer variants)
- `crates/game-server/src/{persist.rs,client/enter.rs,lib.rs}`, `crates/game-server/config-examples/SideBySide.ini`
- `crates/l2-db` (any missing per-character / per-message / per-clan read)
- `tests`

## Requirements
1. No database waits inside a tick (D-15): requests go out through the outbox, answers come back as world commands; the window or the attachment move continues on the answer.
2. A failed read refuses the action with Java's generic failure message for that window and logs a warning; it never falls back to stale memory.
3. Single-owner world, ids not references; no unwrap/expect on external data.

## Out of scope
- Live push between the two servers (not possible without a channel; the Java GS has none).

## Tests (must exist and pass)
- World tests: a mail removed in the DB after startup is gone after CharacterSelect; an attachment row moved away is not handed out; a clan warehouse item withdrawn elsewhere is not in the refreshed list; expiry writes respect the switch.
- DB-gated tests for any new repository read; they skip without `L2_TEST_DATABASE_URL`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`)
- [ ] committed as `M6-SYNC-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS D-15/D-39/D-49 first. Other workers (crafting, augmentation) run at the same time: in shared files only add your lines.
