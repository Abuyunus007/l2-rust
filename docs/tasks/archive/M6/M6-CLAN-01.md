# M6-CLAN-01: Clan core

| Field | Value |
|---|---|
| Milestone | M6 |
| Model | opus |
| Review | full |
| Reference | no (copy the merged M4/M5 code around it) |
| Depends on | GPROTO-09 |
| Size | XL |

## Goal
Clans per `docs/design/m6-social.md`: load all clans at startup (`ClanTable`), create/disband (with Java's delay), invite/join/leave/kick, sub-pledges and ranks, privileges, clan level up (costs), reputation, clan notice, the clan window packets, clan in UserInfo/CharInfo, `clan_data`/`clan_privs`/`clan_subpledges`/`clan_skills`(read)/`characters.clanid` rows exactly as Java (D-39). Clan targets (`// M6:` markers in `skills/targets.rs`) and clan conditions.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\clan\*.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\sql\ClanTable.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\RequestPledge*.java, RequestJoinPledge, RequestAnswerJoinPledge, RequestWithdrawalPledge, RequestOustPledgeMember`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\serverpackets\Pledge*.java`

## Files
- `crates/l2-world/src/clan/*.rs (new) + wire-ins`
- `crates/l2-db/src/game/clans.rs (new)`
- `crates/game-server/src/{lib.rs,persist.rs,client/enter.rs}`
- `tests`

## Requirements
1. Java behaviour, messages, packets and DB rows exactly (D-39: the Java GS shares the DB).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data.
3. Resolve the `// M6:` markers in your area; list the ones you leave.

## Out of scope
- Sieges/castles/fortresses/clan halls (M7), Olympiad/Seven Signs (M8), quests (later).

## Tests (must exist and pass)
- Each part with Java's numbers/messages/packets; DB tests skip without `L2_TEST_DATABASE_URL`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M6-CLAN-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), `docs/design/m4-combat.md`, `docs/design/m5-skills.md`, `docs/design/m6-social.md` first.
- Packets are merged (GPROTO-09): client `l2_protocol_game::client::pledge` (RequestJoinPledge ... RequestPledgeReorganizeMember, ally and war packets), server `server::pledge`. Dispatch arms exist; route them in `enter.rs`. Ally/war handlers belong to CLAN-02/03: route them to a `not ported` stub there, but load the ally/war columns.
- Clan markers: `skills/targets.rs` 313, 363, 1032, 1185 (clan halves; party halves are PARTY-01's).
- Repositories in `l2-db` with sqlx like the existing `l2-db/src/game/*.rs`; DB tests skip without `L2_TEST_DATABASE_URL`.
- Five other M6 workers run at the same time (PVP-01, PARTY-01, TRADE-01 and two of CLAN-01/SUMMON-01/TRANS-01). In shared files (`skills/targets.rs`, `skills/cast.rs`, `death.rs`, `leave.rs`, `player_view.rs`, `enter.rs`) change only the lines of your markers and add new calls; no reformatting or moving code.
- Use `-j 4` for cargo clippy/test (parallel linking crashed under load).
