# Runbook: skills, effects and consumables on the Rust GS (M5)

Setup as in `docs/runbooks/m4-combat-test.md` (Rust GS "Kain" next to the Java GS "Sieghardt" on
the same DB). M5 adds casting, almost all of Java's 165 skill effects (only summon, pet, siege and
transformation effects wait for M6), potions/herbs/scrolls/boxes, soulshots and spiritshots,
monsters using their skills, spoil and sweep, learning skills at trainers, the shortcut bar, and
buffs, skill reuse and item reuse surviving a relog. "Party" skills hit only yourself until parties
exist (M6).

## 1. Checkpoints

### CP-M5a: consumables and shots
1. Drink an HP potion: HP comes back over time (green bar); a Mana Potion fills MP; the potion's
   reuse time shows and other potions of the same group wait too.
2. Pick up a herb from a killed monster: you get its buff icon at once (it never enters the bag).
3. Scroll of Escape: 20 s cast, then you are in town. Open a box (extractable item): its items
   appear with Java's messages.
4. Soulshots: use one with a weapon of the right grade, the weapon glows; your next hit does about
   double damage. Right-click the shot icon for auto use (on/off messages); a wrong grade says so.
   Spiritshots make magic casts faster and stronger.

### CP-M5b: casting and buffs
1. Put skills on the shortcut bar (drag from the skill window), cast an attack spell, a heal and a
   buff: casting bar, animation, MP cost, reuse icon, damage/heal numbers like on Sieghardt.
2. Buff icons and times match Sieghardt; buffs of the same kind replace each other as on Java;
   debuffs (stun, sleep, root, fear, silence) stop you the way Java does.
3. Log out with buffs on and skills on reuse; log in on Kain: buffs, remaining times and reuse are
   back. Then log the same character into Sieghardt: the same buffs and reuse times.

### CP-M5c: monsters, spoil, trainers
1. Fight a mage monster (it casts), a healer group (they heal each other), a buffer.
2. A Dwarf: Spoil a monster ("The spoil condition has been activated"), kill it (blue glow), Sweep:
   the spoil items are yours.
3. Talk to your class trainer: the skill list, learn a skill with SP (and its book if needed).

### CP-M5d: milestone
A normal session on Kain with skills and consumables, then the same character on Sieghardt: skills,
shortcuts, buffs and reuse, inventory and SP as Kain left them.

## 2. Admin commands (new in M5)

| Command | What it does |
|---|---|
| `//add_skill <id> [level]` | give the target a skill |
| `//remove_skill <id>` | take a skill away (its shortcuts go too) |
| `//give_all_skills` | all skills the target's class can learn now |
| `//remove_all_skills` | remove every skill of the target |
| `//stopbuff <objectId> <skillId>`, `//stopallbuffs <objectId>` | remove buffs (`//stopallbuffs` waits for the confirmation dialog, not ported yet) |
| `//areacancel <radius>` | remove the buffs of everyone around |

The admin *pages* (`//skill_list`, the buff list page) are not ported yet: the commands work, the
GM gets no page.

## 3. Known limits in M5
- No summons, pets, cubics, transformations, parties, clans (M6): their skills do nothing or hit
  only you.
- Enchanting skills, enchant scrolls, crafting, fishing, olympiad: later milestones.
- Item elements/augments are not loaded yet, so `ConvertItem` and element damage from items do not
  apply.
- Admin pages and confirmation dialogs for admin commands are not ported.
