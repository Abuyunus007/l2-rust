# Design: castles, sieges, fortresses, clan halls, manor, Territory War (M7)

Builds on `docs/design/m6-social.md`. Owner decisions (2026-10-23): Sieghardt runs the real sieges
while both servers share the DB (D-51); Kain runs GM-started test sieges and writes their results
only with `SiegeDatabaseWrites = True`. Order inside M7: residences core, castle sieges and the
castle NPCs, fortresses and clan halls, the manor, Territory War last.

## Where it lives
| Piece | Crate / module | Notes |
|---|---|---|
| Residence data | `l2-data` `residence.rs` | the static residence definitions (castle/fort/hall ids, zones, doors, functions, `SiegeSchedule.xml`, fort spawn lists) |
| Residences | `l2-world` `residence/` | `CastleId`/`FortId`/`ClanHallId` ids (D-11), tables in the world; owner clan id, tax, treasury, functions, doors; loaded at startup from `castle`/`fort`/`clanhall` rows; re-read at CharacterSelect for the clan's residences (D-50/D-51) |
| Castle sieges | `l2-world` `siege/` | one `Siege` state per castle with its timers on the wheel; attackers/defenders by clan id; siege flags and guards are NPCs in the world's tables |
| Fortresses | `l2-world` `fort/` | `Fort`, `FortSiege`, commanders, the combat flag |
| Clan halls | `l2-world` `clanhall/` | halls, functions, auctions; conquerable halls in `clanhall/siege/` |
| Manor | `l2-world` `manor/` | seed/crop production and procurement per castle, the daily manor period on the wheel |
| Territory War | `l2-world` `territory/` | wards, badges, registration, the war period |
| Residence NPCs | `l2-world` `residence/npc/` | chamberlain, siege manager, mercenary manager, warehouse/teleporter/blacksmith/court magician, doormen, envoys: built in like CLAN-02's `ClanMaster` (bypass + html pages) |
| DB | `l2-db` residence repositories | `castle`, `castle_functions`, `castle_doorupgrade`, `castle_trapupgrade`, `castle_manor_*`, `castle_siege_guards`, `siege_clans`, `fort`, `fort_functions`, `fortsiege_clans`, `fort_doorupgrade`, `clanhall`, `clanhall_functions`, `clanhall_siege_*`, `auction`, `auction_bid`, `territories`, `territory_registrations`, `merchant_lease` ... exactly as Java |

## Rules
- Residence, siege and clan references are ids; no back pointers. The `// M7:` markers in M5/M6
  code are the integration points: each card lists the ones it resolves.
- **Side by side (D-49, D-51):** every scheduled siege/fort/hall/manor/territory job that writes
  global rows runs its DB part only with its `SideBySide.ini` switch on (`SiegeDatabaseWrites`,
  default `False`; one switch for all siege-like results is enough). Without it the job still runs
  in this world (a GM test siege works end to end) but writes nothing global. Per-player writes
  (a player's items, a clan's own reputation from a kill) follow the normal rules.
- **Shared rows (D-50):** residence owners and siege registrations change on Sieghardt while Kain
  runs; Kain re-reads them at startup and before acting on them (registration windows, chamberlain
  pages, tax collection), through the outbox and a world command back, never inside a tick.
- Castle tax: `getTotalTaxRate` and the treasury share in shops and multisell (`// M7:` markers of
  SHOP-01 and MULTISELL-01). The treasury add is a per-castle write: only with the switch on while
  both run (Sieghardt collects the real tax).
