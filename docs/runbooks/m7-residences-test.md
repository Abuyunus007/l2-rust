# Runbook: castles, sieges, fortresses, clan halls, manor, Territory War on the Rust GS (M7)

Setup as in `docs/runbooks/m4-combat-test.md` (Rust GS "Kain" next to the Java GS "Sieghardt" on
the same DB). Most steps need a GM character and two clans (two clients, or one per server).

**Side by side (D-51):** Sieghardt runs the real sieges, fortress sieges, hall sieges, the manor
changes and Territory War while both servers share the DB. Kain runs them only when a GM starts
them (`//startsiege`, `//startfortsiege`, `//chsiege_*`, `//territory_war_start`), and with
`SiegeDatabaseWrites = False` in `config/SideBySide.ini` (the default) it writes no global result:
a new owner, registrations, the treasury income, rent and auction ends stay in Kain's memory. What
a player does himself (a tax rate, a bought function, a deposit) is written as usual. Kain re-reads
residence owners and registrations before acting on them (D-50).

## 1. Checkpoints

### CP-M7a: a castle
1. Give your clan a castle with a GM (`//siege`, the castle page). Talk to the chamberlain: tax
   rate, treasury, open and close the doors, door and trap upgrades, a function.
2. Buy something in a town of that castle: the price includes the castle tax.
3. Castle warehouse (vault), mass gatekeeper, blacksmith, court magician (squad skills),
   mercenary manager (buy a ticket, drop it inside the castle: a mercenary stands there), the
   castle doorman (opens the gates for your clan only).
4. Clan hall or fortress owner: click a door of your hall/fortress: "Do you want to open/close the
   gate?" (yes works only while the door is still targeted).

### CP-M7b: a castle siege (GM test siege)
1. Register an attacker clan at the siege manager, start the siege with `//startsiege <castle>`.
2. Build a headquarters (flag); die and restart at it.
3. Upgrade the castle traps at the chamberlain first: during the siege an attacker in the swamp is
   slowed and the flame zone hurts. Guards and mercenaries attack the attackers, not the defenders. Break the gate ("The castle
   gate has been destroyed."); walls only take siege weapon damage.
4. Seal of Ruler on the artifact: your clan owns the castle on Kain. `//endsiege`: the gates are
   repaired, reputation changes (+1500 / -3000).
5. Restart Sieghardt or look at its castle page: the owner is still Sieghardt's (switch off).

### CP-M7c: fortresses and clan halls
1. Register at a fortress (the suspicious merchant), `//startfortsiege <id>`, kill the commanders,
   carry the combat flag to the pole and use Flag Display: your clan owns the fortress, the
   fortress NPCs and the castle contract.
2. Clan hall auction: list, bid (adena from the clan warehouse), the hall changes hands at the end;
   the hall manager (teleport, buffs, regen, decorations), the doorman, the hall warehouse.
3. A conquerable hall siege (Fortress of Resistance, Devastated Castle or Fortress of the Dead) via
   `//chsiege_*`. Bandit Stronghold and Wild Beast Reserve never start their battle (a Java bug,
   the same on Sieghardt).

### CP-M7d: manor, Territory War, milestone
1. Manor: the lord sets seeds and crops at the chamberlain's manor page; buy seeds at the manor
   manager, sow a monster, harvest it, sell the crop.
2. Territory War: register at the mercenary captain or territory manager, `//territory_war_start`:
   wards, badges, outposts, battlefield chat; the war ends with its rewards.
3. Milestone: a clan leader whose clan owns a residence on Sieghardt logs into Kain: the same
   residence shows; a tax change on Kain shows on Sieghardt after its next start.

## 2. Admin commands (new in M7)

| Command | What it does |
|---|---|
| `//siege`, `//startsiege`, `//endsiege` | castle siege pages, start/end a test siege |
| `//fortsiege`, `//startfortsiege`, `//endfortsiege`, `//setfort`, `//removefort` | fortress sieges and owners |
| `//clanhall`, `//clanhallset`, `//clanhalldel`, `//clanhallopendoors`, `//clanhallclosedoors`, `//clanhallteleportself` | clan halls |
| `//chsiege_*` | conquerable hall sieges |
| `//manor` | manor mode, next change, costs |
| `//territory_war`, `//territory_war_start`, `//territory_war_end`, wards list | Territory War |
| `//admin` panel (M7-ADMIN-08..12) | the whole GM panel: effects, menus, edit char (name, title, colours, class, sex, noble, fame, access level), spawn index, gmshop, doors, fences, delete, search, kick, ban/jail, GM chat, snoop, announcements, petitions, server switches and `//server_shutdown`/`//server_restart` (Kain only), geodata info, regions, zones, zone builder, flags (`gmliston`, `silence`, `diet`, `tradeoff`), `//setconfig` rates, `//repair` |

## 3. Known limits in M7
- Admin commands not there yet: geodata edits (`//geoenablenorth` ... `//geosave`), `//fight_calculator`, `//respawnall`, `//spawn_reload`, `//spawn_misc`, `//list_pools`, `//reload` of anything but html (restart instead), and the M8/script ones (Olympiad, heroes, instances, grand bosses, cursed weapons, quests). They answer "The command ... does not exist!".
- Territory War quests and their points, the noblesse quests (`MakeMeNoble`): with the quests (D-05).
- Seven Signs effects on castles (seal of strife, ticket prices): M8; the castle NPCs show the
  neutral state.
- Party duels still fight where the parties stand (arena instances: M8).
