# Runbook: parties, PvP, trade, clans, companions and the economy on the Rust GS (M6)

Setup as in `docs/runbooks/m4-combat-test.md` (Rust GS "Kain" next to the Java GS "Sieghardt" on
the same DB). M6 needs **two characters** for most steps (two clients, or one client per server).
M6 adds parties, PvP flag and karma, trade and private stores, duels, clans (core, crests,
alliances, wars, clan skills, academy), servitors, pets, cubics and transformations, NPC shops,
multisell, warehouses and freight, enchanting (items, attributes, skills), crafting, augmentation,
friends and block list, mail, and the player actions (sit, run/walk, socials).

**Side by side (D-39, D-49, D-50):** both servers use the same database, but each one only reads
what it needs when it needs it. A clan created on Sieghardt shows on Kain when its member logs into
Kain. Mails and the clan warehouse are re-read before Kain hands items out. Scheduled mass writes
(clan maintenance, mail expiry) stay with Sieghardt while both run (`config/SideBySide.ini`).

## 1. Checkpoints

### CP-M6a: party, PvP, trade, duel
1. Invite the second character (`/invite` or the party button), accept: the party window shows both
   with HP/MP bars; change the loot rule, kick, leave, give the leader to the other.
2. Hunt together: exp and SP are shared (about Java's bonus for two members), drops follow the loot
   rule; a party buff (e.g. a prophet's party skill) lands on both.
3. Attack the other player with Ctrl: your name turns purple (flag), it blinks at the end and goes
   back after the flag time. Kill an unflagged player: karma, red name, PK count; guards attack you.
4. Trade: request, both add items and adena, both confirm: the items change owner; relog both on
   Sieghardt and the items are where Kain put them.
5. Private store: sit down with "Private store - sell", set items and prices, the other buys. Then a
   buy store. Then `/duel` the other: countdown, fight, the loser kneels at 1 HP, nobody dies.

### CP-M6b: clan
1. At a village master: create a clan, invite the other character, give a title and a rank, level
   the clan up (costs as in Java), set a crest (`.bmp` 16x12) and look at it on both clients.
2. Create an alliance with a second clan, declare a clan war and check the war icons; kill a war
   enemy: clan reputation changes as in Java.
3. Learn a clan skill at the clan trainer: every member gets it. The academy: a level 40 or lower
   character joins as an academy member.
4. Log the clan leader into Sieghardt: the same clan, members, ranks, crest and skills.

### CP-M6c: companions
1. Summon a servitor (a summoner class skill): it follows, attacks your target, casts, shares exp,
   uses beast shots; unsummon.
2. A pet: use its collar, feed it, give it an item and take it back, level it, let it die and
   revive it. Relog: the pet keeps its level and items. Trade the collar to the other character:
   the pet keeps its level (D-50).
3. Cubics (e.g. a Warlord's or a Shillien Knight's cubic skill): they attack, heal or debuff on
   their timers.
4. A transformation (skill or scroll): new look, skills and action bar; it ends on death or the
   dispel skill, and comes back after a relog while its time lasts.

### CP-M6e: economy
1. NPC shop: buy, sell, use the refund tab; prices with the town tax.
2. Multisell (e.g. a blacksmith exchange): an exchange with ingredients and an enchanted item kept.
3. Warehouse: deposit and withdraw (30 adena per item), the clan warehouse with clan rights, freight
   to another character of your account.
4. Enchant a weapon with a scroll (success and a failure on a test item), add an attribute stone,
   enchant a skill at the trainer (route, cost, book).
5. Crafting: a dwarf learns a recipe, crafts an item, opens a manufacture shop and crafts for the
   other character.
6. Augmentation at the blacksmith: life stone plus gemstones, the augment shows, then remove it.
7. Friends: add the other as a friend, see the login notice, block and unblock. Mail: send a mail
   with an attachment and a cash on delivery price, receive it on the other character.

### CP-M6d: milestone
A party session on Kain with summons, a clan and a trade, then the same characters on Sieghardt:
party exp, items, clan, pets, mails and warehouse as Kain left them.

## 2. Admin commands (new in M6)

| Command | What it does |
|---|---|
| `//partyinfo`, `//recall_party_menu`, `//recall_clan_menu` | party information, recall a party or clan to you |
| `//clan_info`, `//clan_changeleader`, `//pledge create/dismiss/setlevel/rep`, `//remove_clan_penalty` | clan admin |
| `//nokarma`, `//setkarma`, `//setpk`, `//setpvp`, `//set_pvp_flag` | karma and PvP |
| `//summon <npc>`, `//summon_info`, `//unsummon`, `//summon_setlvl` | summons and pets |
| `//transform <id>`, `//untransform`, `//transform_menu` | transformations |
| `//enchant` (+ slot words) | enchant the target's equipped items |
| `//get_skills`, `//reset_skills`, `//setskill`, `//buff`, `//removereuse` | the charskills page buttons |

## 3. Known limits in M6
- **Sieges, castles, fortresses, clan halls (M7):** no castle tax in shops and multisell, no siege
  zones, no clan hall/fortress managers, clan level 11 needs a territory and fails.
- **Olympiad, Seven Signs, instances (M8):** party duels are fought where the parties stand (Java
  moves them to an arena instance).
- **Quests and scripts (later):** class change and subclasses come with the village master scripts;
  academy graduation waits for the class change.
- Not modelled yet: mounts (strider, wyvern), fishing, offline trade/craft shops, flood protectors,
  the bot report button, PC cafe points.
- Crest ids: if both servers register a crest in the same moment, one of them fails and is logged
  (rare; fixed before the switch from Java).
