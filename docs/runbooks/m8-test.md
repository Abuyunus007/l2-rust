# Runbook: Olympiad, heroes, Seven Signs, instances and the big systems on the Rust GS (M8)

Setup as in `docs/runbooks/m4-combat-test.md` (Rust GS "Kain" next to the Java GS "Sieghardt" on
the same DB). Several steps need a GM character, a noble character and a party.

**Side by side (D-53):** Sieghardt keeps running the global cycles while both servers share the DB:
the Olympiad periods and weekly points, the hero choice, the Seven Signs cycle and festival
results, the grand boss states and raid boss respawn times, the item auction ends and the cursed
weapons. With `CycleDatabaseWrites = False` in `config/SideBySide.ini` (the default) Kain keeps
those in memory, re-reads them at start and when they are shown (D-50), and writes only the rows
Java writes per player (instance re-entry times, raid points, olympiad fight rows, Seven Signs
member rows). With the switch off Kain never drops a cursed weapon, spawns a grand boss only when
Sieghardt's row says it is alive, and shows item auctions without taking bids ("This feature is
disabled."). Offline shops keep the account on Kain (D-52).

## 1. Checkpoints

### CP-M8a: Olympiad and heroes
1. Make a character noble (`//setnoble`), register at the Olympiad manager (classed, non-classed,
   team). During the competition hours a match starts: the stadium (an instance), the buffer,
   the countdown, the fight, the result and the points.
2. A second character watches the match through the manager's observation list (spectator mode)
   and returns.
3. `/olympiadstat`, the manager's ranking pages, `//checkolympiad`. The hero list at the monument.
4. Heroes: a hero character logs in with the hero aura, skills and voice chat (`%`), the diary
   page at the monument. `//sethero` on a test character.
5. With the switch off, `//endolympiad` forms heroes in Kain's memory only; Sieghardt's heroes
   stay as they are.

### CP-M8b: Seven Signs, festival, rift
1. Register at a Dawn or Dusk priest, contribute seal stones, check the score page (`SSQStatus`,
   the Seven Signs record item). Ancient Adena exchange at the priest.
2. Catacombs and necropolises: the gatekeepers let you in by period and cabal.
3. Festival of Darkness: sign up with a party at a festival guide, the arena round (witch, monsters,
   offerings, chests), the score.
4. Dimensional Rift: enter with a party and fragments, the rooms jump, the boss room, leave.
5. Seal effects in the seal validation period (prices at castle NPCs, the Mammon NPCs:
   `//mammon_find`, `//mammon_respawn`).

### CP-M8c: instances, boats, bosses, cursed weapons, mini games
1. A party duel (`/partyduel`): both parties go into the arena instance and come back.
2. Boats: Talking Island <-> Gludin (ticket), Rune <-> Gludin, Giran <-> Talking Island, the
   Innadril tour; the Gludio-Gracia airship; a clan airship with a license (Keucereus).
3. Raid bosses: a kill, the raid points (`/raidrank` window, `RequestGetBossRecord`), the respawn
   time. Grand boss zones oust strangers. `//grandboss`.
4. Cursed weapons: `//cw_info`, `//cw_add` on a test character (switch off: in memory only),
   the transformation, kills, `//cw_remove`.
5. Mini games: Handy's Block Checker (lobby, teams, a match), Kratei's Cube (registration at
   minute 27/57), the Underground Coliseum.
6. Weddings: `.engage`, the priest, `.gotolove`, `.divorce` (with AllowWedding on).

### CP-M8d: milestone
1. A noble with points on Sieghardt logs into Kain: the same points and ranking show.
2. The Seven Signs period and seal owners on Kain match Sieghardt's.
3. A raid boss Sieghardt killed is dead on Kain until its respawn time.

## 2. Admin commands (new in M8)

| Command | What it does |
|---|---|
| `//checkolympiad`, `//saveolymp`, `//endolympiad` | Olympiad times, save, hero selection |
| `//sethero`, `//givehero` | hero status |
| `//mammon_find`, `//mammon_respawn` | the Mammon NPCs (seal validation) |
| `//instance`, `//setinstance`, `//createinstance`, `//destroyinstance`, `//listinstances`, `//instancezone(s)`, `//instancezone_clear`, `//instance_spawns` | instances |
| `//grandboss`, `//grandboss_skip`, `//grandboss_respawn`, `//grandboss_minions`, `//grandboss_abort` | grand bosses (the script events need the boss scripts) |
| `//cw_info`, `//cw_info_menu`, `//cw_add`, `//cw_remove`, `//cw_goto`, `//cw_reload` | cursed weapons |

## 3. Known limits in M8
- Scripts (D-05) come with the script milestone: the boss AIs (Antharas, Valakas, Baium, ... and
  their `//grandboss_*` events), instance quests, the event scripts, quest 635 and the
  `OracleTeleport` links of the rift pages, the block checker's `FlipBlock` skill wiring.
- Not modelled: HWID checks, champion monsters, premium, the item log call sites, the chat filter.
- `//spawn_reload` and `//respawnall` do not re-run the Seven Signs auto spawns.
- The festival creates new spawn records each round (a small leak until a restart).
