# M8 draft: Olympiad, heroes, Seven Signs, instances and the big systems (for the owner's review — no worker starts before sign-off)

**Done when:** the Olympiad runs its periods and matches (classed, non-classed, teams), nobles
earn and spend points, heroes are chosen with their skills and diary; Seven Signs runs its cycle
(registration, stones, seals and their effects, Ancient Adena, the catacombs and necropolises),
the Festival of Darkness and the Dimensional Rift work; instances exist (instance templates,
instance worlds with their doors and spawns, re-entry times), so party duels get their arena;
grand boss and raid boss states and raid points are kept like Java; boats and airships sail;
cursed weapons (Zariche, Akamanah) appear and pass on; the mini games (Underground Coliseum,
Kratei's Cube, Handy's Block Checker), petitions, weddings and the fishing championship work.
Everything stays readable for Sieghardt on the same DB (D-39).

What M6/M7 leave for M8 (the `// M8:` markers): olympiad checks in skills, targets, duels,
friends, cubics and the bot report; cursed weapon checks in mounts and transformations; the party
duel arena (an instance); hero voice chat; `isInOlympiadMode` everywhere.

Java size, for scale: `mechanics/olympiad` + `mechanics/sevensigns` 11,700 lines (`Olympiad`,
`OlympiadGame*`, `Hero`, `SevenSigns`, `SevenSignsFestival`, `DimensionalRift`), `InstanceManager`
and the instance templates, `GrandBossManager`, `RaidBossSpawnManager`, `BoatManager`,
`AirShipManager`, `CursedWeaponsManager`, `undergroundColiseum/*`, `krateisCube/*`,
`HandysBlockCheckerManager`, `PetitionManager`, `CoupleManager`, `FishingChampionshipManager`.
The boss AIs, the instance scripts and the event scripts are **scripts** (D-05) and come with the
script milestone after M8; M8 ports the engines they run on.

## Proposed cards

| Card | Title | Model | Depends on | Notes |
|---|---|---|---|---|
| M8-GPROTO-11 | Olympiad, hero, Seven Signs, instance, vehicle, mini game packets | haiku | — | `ExOlympiad*`, `ExHeroList`, `ExReceiveOlympiad`, `SSQInfo`, `SSQStatus`, `RequestSSQStatus`, `ExShowSevenSignsMapInfo`?, `ExSendUIEvent`, `VehicleInfo`/`VehicleDeparture`/`VehicleStarted`/`MoveToLocationInVehicle`/`ExAirShip*`, `ExCubeGame*`, `ExUC*`?, `ExCursedWeapon*`, petition packets, ... |
| M8-INST-01 | Instances | **opus** | GPROTO-11 | `InstanceManager`, instance templates (`data/instances/*.xml`), instance worlds with their own doors, spawns and zones, enter/exit, re-entry times (`character_instance_time`), instance ids on objects (visibility per instance); the party duel arena (the DUEL-01 marker) |
| M8-OLY-01 | Olympiad | **opus** | GPROTO-11, INST-01 | `Olympiad` periods and validation, `olympiad_nobles` rows, registration, `OlympiadGameManager` and the classed/non-classed/team games in the stadiums, buffers and spectators, points and rewards, `isInOlympiadMode` everywhere (the `// M8:` markers) |
| M8-HERO-01 | Heroes | sonnet | OLY-01 | `Hero` (`heroes`, `heroes_diary` rows), hero skills and items, the hero list, hero voice chat, the monument NPC pages |
| M8-SS-01 | Seven Signs | **opus** | GPROTO-11 | `SevenSigns` cycle (competition, results, seal validation), registration, seal stones, the seals and their effects (avarice, gnosis, strife: spawns, shop prices, siege doors), Ancient Adena, dawn/dusk priests and the catacomb/necropolis gatekeepers built in like CLAN-02's `ClanMaster` |
| M8-FEST-01 | Festival of Darkness and Dimensional Rift | sonnet | SS-01, INST-01 | `SevenSignsFestival`, `DimensionalRift`/`DimensionalRiftManager` |
| M8-BOSS-01 | Grand and raid boss states, raid points | sonnet | — | `GrandBossManager` (`grandboss_data`), `RaidBossSpawnManager` (`raidboss_spawnlist`), `RaidBossPointsManager` (`character_raid_points`), the boss zones; the boss AIs stay scripts (D-05) |
| M8-VEHICLE-01 | Boats and airships | sonnet | GPROTO-11 | `BoatManager`, `AirShipManager`, vehicle movement and boarding, the boat routes (the boat route scripts as the engine needs them) |
| M8-CURSED-01 | Cursed weapons | sonnet | — | `CursedWeaponsManager` (`cursed_weapons` rows), Zariche and Akamanah: drop, ownership, the transformation, kills, the timer; the M6 cursed weapon markers |
| M8-GAMES-01 | Mini games | sonnet | INST-01 | Underground Coliseum, Kratei's Cube, Handy's Block Checker |
| M8-MISC-01 | Petitions, weddings, fishing championship, event drops | sonnet | — | `PetitionManager`, `CoupleManager` (`mods_wedding`), `FishingChampionshipManager`, `EventDropManager`, `WalkingManager` (NPC routes) |
| M8-ADMIN-07 | Admin commands for M8 | haiku | OLY-01, SS-01, INST-01 | `//olympiad`, `//hero`, `//sevensigns`, `//instance`, `//cw`, `//grandboss`, ... as Java registers them |
| M8-E2E-01 | End to end: an olympiad match, Seven Signs registration, an instance | sonnet | OLY-01, SS-01, INST-01 | |
| M8-RUN-01 | Runbook and milestone checks | opus | E2E-01 | |

14 cards: 3 opus (INST-01, OLY-01, SS-01) + RUN-01 and the reviews, 9 sonnet, 2 haiku.

## Checkpoints

| Checkpoint | You test |
|---|---|
| CP-M8a | Olympiad: register a noble, a match in the stadium (classed and non-classed), points; a spectator; the hero list. |
| CP-M8b | Seven Signs: register at a priest, contribute seal stones, the catacombs and necropolises, a seal effect; the Festival of Darkness; the Dimensional Rift. |
| CP-M8c | An instance (a party duel in its arena); a boat trip; a cursed weapon; a mini game. |
| CP-M8d | Milestone: Olympiad, Seven Signs and boss states as Sieghardt keeps them. |

## Open questions for the owner

1. **Side by side (like D-51):** the Olympiad period end and hero choice, the Seven Signs cycle
   changes and the grand boss states are global jobs with global rows. Recommended: **Sieghardt
   keeps running them** while both share the DB. Kain runs olympiad matches and Seven Signs
   registration/contributions for its own players (per-player rows as usual), but the period/cycle
   changes, hero calculation and grand boss state writes only with the `SiegeDatabaseWrites`-style
   switch (one new key, e.g. `CycleDatabaseWrites = False`), and it re-reads those states at
   startup and when they are shown (D-50).
2. **Grand bosses on Kain:** each server spawns its own world. Recommended: Kain spawns a grand
   boss only when its state row (written by Sieghardt) says it is alive, and does not write the
   kill/respawn state while the switch is off.
3. **Scope:** the boss AIs, instance scripts and event scripts are scripts (D-05) and come after
   M8 with the script milestone. Recommended: yes; M8 ports only the engines.
4. **Order inside M8:** instances first (the olympiad stadiums and the party duel arena need
   them), then Olympiad and heroes, then Seven Signs and festivals, then the rest. Recommended: yes.
