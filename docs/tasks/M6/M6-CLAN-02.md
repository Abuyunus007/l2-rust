# M6-CLAN-02: Crests, alliances, clan wars

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | CLAN-01 |
| Size | M |

## Goal
Clan/ally crests (upload, `crests` table, `PledgeCrest`/`AllyCrest` packets, the crest cache), alliances (create, invite, leave, dismiss, ally info), clan wars (declare, accept, stop, war relation masks with M6-PVP-01).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\sql\CrestTable.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\clan\Crest.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{RequestSetPledgeCrest*,RequestSetAllyCrest,RequestJoinAlly,RequestStartPledgeWar,...}.java`

## Files
- `crates/l2-world/src/clan/{crest,alliance,war}.rs`
- `crates/l2-db`
- `tests`

## Requirements
1. Java behaviour, messages, packets and DB rows exactly (D-39: the Java GS shares the DB).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data.
3. Resolve the `// M6:` markers in your area; list the ones you leave.

- From CLAN-01: the `ClanMaster` village master script (`bypass Script ClanMaster ...` plus a `ScriptLink` handler) so players reach the clan commands through the NPC (CP-M6b needs it); `create_ally`/`dissolve_ally` stubs in `clan/village_master.rs`; the `ClanInput::NotPorted` packets (crests, ally, wars); war reputation on death (`death.rs`) and drops (`rewards/drop.rs`).

## Out of scope
- Sieges/castles/fortresses/clan halls (M7), Olympiad/Seven Signs (M8), quests (later).

## Tests (must exist and pass)
- Each part with Java's numbers/messages/packets; DB tests skip without `L2_TEST_DATABASE_URL`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M6-CLAN-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- CLAN-01 is merged: `crates/l2-world/src/clan/` (`model.rs`, `table.rs`, `members.rs`, `handlers.rs`, `village_master.rs`), crest/ally/war packets currently go to `ClanInput::NotPorted`: route them to your code. War relation inputs plug into `pvp.rs::relation_input` (CLAN-01 filled the war bits from `clan_wars`; you add declare/accept/stop and the live updates).
- The ClanMaster village master script: grep `ClanMaster` under `C:\l2-rust\l2jmobius\dist\game\data\scripts` (an AI/village master script) and port what the clan pages need, using the existing bypass/html pattern in `bypass.rs`/`dialog.rs`.
- Up to seven M6 workers run at the same time. In shared files (`skills/targets.rs`, `enter.rs`, `player_view.rs`, `leave.rs`, `death.rs`, `world.rs`, `dispatch.rs`, `client/enter.rs`) only add your lines: no reformatting or moving code.
- Use `-j 4` for cargo clippy/test (parallel linking crashed under load) and CARGO_INCREMENTAL=0.
- Merged APIs to reuse: `trade/` (`TradeList`, `World::transfer_item` in `inventory/transfer.rs`, `check_item_manipulation`, the request state in `trade/request.rs`), `party/` (`World::party_of`), `clan/` (`ClanTable`, privileges), `pvp.rs`, `transform/`. Item rows go through the persistence outbox like TRADE-01 does.
