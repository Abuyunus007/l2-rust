# M7 draft: castles, sieges, fortresses, clan halls (for the owner's review — no worker starts before sign-off)

**Done when:** clans own castles, fortresses and clan halls on Kain as in Java: the chamberlain
sets the tax and opens doors, shops and multisell pay castle tax into the treasury, castle sieges
run on their schedule (registration, flags and headquarters, siege guards and mercenaries, doors
and walls, engraving the artifact, the new owner), fortress sieges (commanders, the combat flag,
contracts with a castle), clan halls (auctions, rent, functions such as teleport, buffs, regen),
the six conquerable clan hall sieges, the manor (seeds, crops, procurement) and Territory War.
Everything stays readable for Sieghardt on the same DB (D-39).

What M6 leaves for M7 (the `// M7:` markers): castle tax in NPC shops and multisell, siege zones
and the siege checks in PvP, duels, alliances and clan wars, clan level 11 (needs a territory),
clan hall and fortress warehouse managers, squad skills that need a castle or fortress, the
mercenary ticket and fort checks in pet pickup, the siege kill/death counts and siege relation bits.

Java size, for scale: `mechanics/siege/*` 6,400 lines (`Siege`, `Castle`, `Fort`, `FortSiege`, ...),
`TerritoryWarManager` 1,887, `CastleManorManager` 936, `MercTicketManager` 438, `FortSiegeManager`
357, `CastleManager` 374, `SiegeManager` 322, plus the residence NPC scripts (`CastleChamberlain`,
`CastleSiegeManager`, `CastleMercenaryManager`, `CastleWarehouse`, `CastleTeleporter`,
`CastleBlacksmith`, `CastleCourtMagician`, `FortressSiegeManager`, `MercenaryCaptain`,
`TerritoryManagers`, `ManorManager`, ...) and six conquerable hall scripts.

## Proposed cards

| Card | Title | Model | Depends on | Notes |
|---|---|---|---|---|
| M7-GPROTO-10 | Siege, castle, fortress, manor, territory packets | haiku | — | `SiegeInfo`, `SiegeAttackerList`/`DefenderList`, `RequestJoinSiege`, `RequestConfirmSiegeWaitingList`, `ExShowCastleInfo`, `ExShowFortressInfo`/`SiegeInfo`/`MapInfo`, the manor `ExShow*` and `RequestSet*`/`RequestBuySeed`/`RequestProcureCropList`, `ExDominionWar*`, `ExShowDominionRegistry`, `RequestJoinDominionWar`, ... |
| M7-RES-01 | Residences core: castles, fortresses, clan halls | **opus** | — | load `castle`/`fort`/`clanhall` rows and functions as Java, residence zones (castle, siege, fort, clan hall), owner clan id on the clan (`castle_id`, `fort_id`, `hall_id`), residence skills, circlets, castle tax and treasury (shops, multisell), doors of residences, the `// M7:` markers |
| M7-SIEGE-01 | Castle sieges | **opus** | RES-01, GPROTO-10 | `SiegeManager`/`Siege`: schedule (`SiegeSchedule.xml`), registration, siege zones and the siege relation bits, `SiegeFlag` and headquarters, respawn at flag/castle, siege guards (`SiegeGuardManager`), mercenary tickets (`MercTicketManager`), attackable/repairable doors and walls, engraving the artifact, owner change, kill counts |
| M7-FORT-01 | Fortresses and fortress sieges | sonnet | RES-01, GPROTO-10 | `FortManager`/`Fort`/`FortSiege`/`FortSiegeManager`/`FortSiegeGuardManager`, commanders, `CombatFlag`, fort functions and envoys (castle contracts), `FortUpdater` |
| M7-CH-01 | Clan halls and auctions | sonnet | RES-01 | `ClanHall`, `AuctionableHall`, `ClanHallAuctionManager` (bids, rent, the auction board), `ClanHallManager` functions (teleport, buffs, HP/MP/exp regen, item create, decorations), `ClanHallDoorman` |
| M7-CHSIEGE-01 | Conquerable clan hall sieges | sonnet | CH-01, SIEGE-01 | `CHSiegeManager` + the six halls (Bandit Stronghold, Devastated Castle, Fortress of Resistance, Fortress of the Dead, Rainbow Springs Chateau, Wild Beast Reserve) |
| M7-NPC-01 | Residence NPCs | sonnet | RES-01, SIEGE-01 | the castle/fortress scripts built in like CLAN-02's ClanMaster: chamberlain (tax, doors, functions, treasury), siege manager, mercenary manager, castle warehouse/teleporter/blacksmith/court magician (squad skills), fortress siege manager and envoys, doormen, mercenary captain |
| M7-MANOR-01 | The manor | sonnet | RES-01, GPROTO-10 | `CastleManorManager` (seed/crop production, procurement, the daily manor period), the `ManorManager` NPC, `Seed` and `Harvester` item handlers, crop sale |
| M7-TW-01 | Territory War | **opus** | SIEGE-01 | `TerritoryWarManager` (registration, wards, badges, the war period, territory quests' hooks as markers), `TerritoryWard`, the `TerritoryManagers` NPCs, clan level 11 |
| M7-ADMIN-06 | Admin commands for residences | haiku | SIEGE-01, FORT-01, CH-01 | `//siege`, `//fortsiege`, `//clanhall`, `//territory_war`, `//manor` pages as Java registers them |
| M7-E2E-01 | End to end: a castle siege, a fortress siege, a clan hall auction | sonnet | SIEGE-01, FORT-01, CH-01 | bare-world and DB variants like M6 |
| M7-RUN-01 | Runbook and milestone checks | opus | E2E-01 | |

12 cards: 3 opus (RES-01, SIEGE-01, TW-01) + RUN-01 and the reviews, 7 sonnet, 2 haiku.

## Checkpoints

| Checkpoint | You test |
|---|---|
| CP-M7a | A castle owned by your clan: the chamberlain (tax rate, doors, treasury), the castle tax in a town shop, the castle warehouse and teleporter. |
| CP-M7b | A castle siege on Kain (a GM can start one early with `//siege`): register, place a flag, fight guards and mercenaries, break the doors, engrave the artifact, your clan owns the castle. |
| CP-M7c | A fortress siege; a clan hall auction and its functions; one conquerable clan hall siege. |
| CP-M7d | The manor and Territory War; milestone: residences and their owners as Kain left them, read by Sieghardt after its next start. |

## Open questions for the owner

1. **Sieges side by side (D-39/D-49): who runs the real sieges while both servers share the DB?**
   Both servers keep their own world, so a siege on Kain and the same siege on Sieghardt are two
   different fights with their own players, and both would write the castle owner. Recommended:
   **Sieghardt keeps running the real sieges** while both run. Kain runs sieges only when a GM
   starts one (`//siege`), and writes the result to the DB only with a new switch
   `SiegeDatabaseWrites = False` in `config/SideBySide.ini` (D-49 pattern). Kain re-reads castle,
   fortress and hall owners at startup and when a clan member logs in (D-50 pattern), so it shows
   what Sieghardt decided. Turned on the day Kain runs alone.
2. **Scope:** Territory War is the biggest single piece (and its quests come with the quest
   milestone). Recommended: keep it in M7 but last; or move it to M8 with the Olympiad if you want
   M7 sooner.
3. **Order inside M7:** residences core → castle sieges and the castle NPCs → fortresses and clan
   halls → manor → Territory War. Recommended: yes.
