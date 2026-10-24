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

## The residence API (M7-RES-01, merged)
All on `World`; types in `l2_world::residence`.
- Ids/refs: `CastleId`, `FortId`, `ClanHallId`, `ResidenceRef::{Castle,Fort,ClanHall}`; public fields on `Castle`, `Fort`, `ClanHall` (`ClanHallKind::{Auctionable,Siegable}`): `owner_id`, `siege_date`, `time_registration_over(_date)`, `doors`, zone ids, `former_owner`, `first_mid_victory`, `ticket_buy_count`, `state`, `castle_id`, `supply_level`, `last_owned_time`.
- Lookups: `castle/castles/castle_by_owner/castle_by_name/castle_index/castle_at/castle_contains/castle_distance/nearest_castle/has_owned_castle`, `npc_castle/npc_is_in_town/npc_castle_and_town`; `fort/forts/fort_by_owner/fort_by_name/fort_at/fort_contains/nearest_fort/npc_fort`; `clan_hall/clan_halls/clan_hall_is_free/auctionable_hall_by_owner/clan_hall_by_owner/clan_hall_at/nearby_auctionable_hall/nearby_clan_hall`; `residence_name`, `residence_owner_id`, `residence_doors`, `residence_door`.
- D-51 writes: `siege_database_writes()`, `siege_write(ResidenceWrite) -> bool` for every global siege-like write, `write_residence(Writes::{Player,Job,Never}, ResidenceWrite)`; `ResidenceWrite` carries Java's statements (incl. `CastleTicketBuyCount`, `FortState`, `FortSupplyLevel`, `SiegableHall`). Add new variants there for new tables.
- Owners: `castle_set_owner`, `castle_remove_owner`, `castle_update_clans_reputation`; `fort_set_owner(id, clan, update_rep) -> bool`, `fort_remove_owner`, `fort_set_state`, `fort_raise_supply_level`, `fort_set_supply_level`, `fort_save_variables`, `fort_update_clans_reputation`, `fort_set_visible_flag`; `clan_hall_table_set_owner`, `clan_hall_table_set_free`, `clan_hall_set_owner`, `clan_hall_free`.
- Treasury/tax: `castle_add_to_treasury` (Job), `castle_add_to_treasury_no_tax(id, amount, Writes)`, `castle_set_tax_percent`, `castle_set_show_npc_crest`, `castle_set_ticket_buy_count`.
- Functions: `residence_function(res, type)`, `residence_functions`, `residence_update_functions(..)`, `residence_remove_function(res, type, Writes)`, `CASTLE_FUNC_*`, `HALL_FUNC_*`.
- Doors/zones: `residence_open_close_door`, `clan_hall_open_close_doors`, `residence_banish_foreigners`, `castle_oust_all_players`, `agit_deco_info`.
- Skills: `give_residential_skills` / `remove_residential_skills(res, player)`.
- Re-read before acting (D-50): `request_residence_reread(res, ResidenceReadThen)` → `WorldCommand::ResidenceReread` → `on_residence_reread`; add your own `ResidenceReadThen` variants; a failed read refuses the follow-up.
- Siege schedule: `siege_schedule()` (`for_castle(id)`), `set_siege_schedule`. Startup: `load_residences(ResidenceLoad)` (game-server `persist::load_residences`, `persist::load_residence_refresh`, `residence_settings()`).
- Left markers name their card: SIEGE-01 (`actions.rs`, `pvp.rs`, clan siege skills/states, `summon/cubic.rs`, `mount.rs`, residence `midVictory`/`loadDoorUpgrade`, restart points castle/hall/fort in `death.rs`/`teleport.rs`), FORT-01 (FortUpdater, fort spawns, siege announcements), CH-01 (rent, auction initNPC), CHSIEGE-01 (`updateNextSiege`), MANOR-01 (`resetManorData`), TW-01 (ward skills, `setOwnerClan`), ADMIN-06 (`admin/clan.rs` names).

## The fortress siege API (M7-FORT-01)
In `l2_world::fort` (`World::fort_*`); state in `FortWorld` (`world.fort`), timers `TimerKind::Fort(FortTimer)`, client input `ClientInput::Fort(FortInput)`.
- Siege: `fort_add_attacker/fort_remove_attacker`, `fort_start_siege/fort_end_siege`, `fort_siege_in_progress`, `fort_player_in_siege`, `fort_clan_is_registered`, `fort_sieges_remove_attacker` (clan destroyed), `fort_killed_commander`, `fort_siege_add_flag/fort_siege_killed_flag/fort_siege_flags` (for castle-side `SiegeFlag`/headquarters), `fort_reset_doors`, `fort_player_left`.
- Zone: `fort_siege_zone_on_enter/on_exit` are called from `zones.rs` for `ZoneKind::Siege` zones with a `fortId`; castle zones stay with SIEGE-01.
- Updater: `fort_start_updaters_on_load`, `fort_restart_updaters`, `fort_time_till_next_update`; the money-moving part runs only with `SiegeDatabaseWrites` (D-51), blood oath counts go up in memory regardless.
- Writes: `ResidenceWrite::FortSiege(FortSiegeWrite)` (siege date, attacker rows, clear, blood oath count). Re-reads: `ResidenceReadKind::FortSiege`, `ResidenceReadThen::Fort(FortThen)`; the automatic start chain does nothing without the switch, `//startfortsiege` always works.
- Combat flag: `CombatFlag` items are tracked in `FortState`; hooks in pickup, equip, death, logout and the `TakeFort`/`TakeFortStart` effects.
- NPCs: FortManager/FortDoorman/FortLogistics/FortCommander are dispatched from `fort_npc_bypass_feedback` and `fort_show_chat_window`; the scripts FortressSiegeManager and FortressArcherCaptain are built in (`fort_script_first_talk/fort_script_event`).
- Left: `FortSiegeGuardAI`, Territory War checks (`M7-TW-01`, in `fort_add_attacker`), door damage and repair, the fort restart point (SIEGE-01), the `CastleAmbassador` dialog (NPC-01; `fort_castle_id_by_ambassador` and `fort_contracted_castle` are ready).
