# M5 draft: skills and effects (for the owner's review — no worker starts before sign-off)

**Done when:** you cast attack, heal and buff skills on Kain; potions, herbs, scrolls and
soulshots work; buffs show their icons and times like on Sieghardt and survive a relog; monsters
use their skills; you learn new skills at a trainer. Formulas are checked against Java vectors
(D-41).

What M4 leaves for M5 (from the M4 reports): skill effects (only passive stat functions run today),
casting, item handlers (herbs vanish without their effect, potions and soulshots do nothing),
spoil/sweep, NPC skill use in `AttackableAI` (`// M5:` markers), invulnerability/stun/etc. as
effects, skill learning at trainers, the stat effects of buffs.

Java size, for scale: `Skill.java` 1,771 lines, `EffectList.java` 1,954, `BuffInfo.java` 487,
**165 effect handlers** and **34 target handlers** (`dist/game/data/scripts/handlers/skill/`),
**30 item handlers** (`handlers/items/`), the magic part of `Formulas.java`.

## Proposed cards

| Card | Title | Model | Depends on | Notes |
|---|---|---|---|---|
| M5-GPROTO-08 | Skill, buff and item-use packets | haiku | — | MagicSkillUse, MagicSkillLaunched, MagicSkillCanceled, AbnormalStatusUpdate, ShortBuffStatusUpdate, ExAutoSoulShot, AcquireSkillList/Info/Done; client RequestMagicSkillUse, RequestAutoSoulShot, RequestAcquireSkill(Info), RequestDispel |
| M5-JV-03 | Java vectors for skill formulas | sonnet | — | same harness as M4-JV-01: magic damage, m.crit, skill physical damage, blow/backstab, effect success and resist, abnormal time, cast/reuse time, heal amounts |
| M5-FORM-02 | Skill formulas as pure functions | sonnet | JV-03 | like M4-FORM-01, checked against the vectors |
| M5-EFFECT-01 | Effect engine: effect list, buff info, stacking, durations, ticks | **opus** | GPROTO-08 | `EffectList`/`BuffInfo`: buffs, debuffs, dances/songs, toggles, triggered; abnormal type/level stacking; timers and ticks on the timer wheel; stat funcs through the stat engine; icons (AbnormalStatusUpdate); an effect-handler registry (trait + table, like the bypass/admin registries) |
| M5-CAST-01 | Casting | **opus** | EFFECT-01, FORM-02 | RequestMagicSkillUse, all use checks (MP/HP/item consume, reuse, conditions, range, line of sight), cast timing (hit/cool time), interruption, reuse and SkillCoolTime, the CAST intention in the AI, skill shortcuts |
| M5-TARGET-01 | The 34 target handlers | sonnet | CAST-01 | One, Self, Aura, Area, FrontArea, BehindAura, Corpse*, Party* (party = self until parties exist), ... |
| M5-EFX-01 | Effect handlers: damage and healing | sonnet | EFFECT-01, FORM-02 | PhysicalDamage, MagicalDamage(*), Heal(*), ManaHeal(*), CpHeal(*), HealOverTime, DamOverTime(*), Lethal, FatalBlow, Backstab, HpDrain, EnergyDamage, ... (~30) |
| M5-EFX-02 | Effect handlers: buffs, debuffs, control | sonnet | EFFECT-01 | Buff, Debuff, Stun, Sleep, Root, Paralyze, Fear, Mute, PhysicalMute, Invincible, Hide, SilentMove, Disarm, Petrification, ... (~40) |
| M5-EFX-03 | Effect handlers: dispel, hate, movement, misc | haiku | EFFECT-01 | Dispel*, AddHate, DeleteHate, TargetMe, Teleport, Escape, Blink, Resurrection, Recovery, Restoration(*), GiveSp, Lucky, ... (~40) |
| M5-ITEM-01 | Item handlers: potions, herbs, scrolls, elixirs, extractables | sonnet | CAST-01, EFX-01 | ItemSkills/ItemSkillsTemplate (herbs get their effect), Elixir, ExtractableItems (`<capsuled_items>` data), shared item reuse groups |
| M5-SHOT-01 | Soulshots and spiritshots | sonnet | CAST-01 | SoulShots, SpiritShot, BlessedSpiritShot, auto shots (ExAutoSoulShot), the charge state in normal attacks (`Attack` flags, damage ×2) and magic |
| M5-SPOIL-01 | Spoil and sweep | haiku | EFX-03 | Spoil/Sweeper effects, the spoil list (already rolled by M4-DROP-01's calculator), `Die.sweepable` |
| M5-AI-03 | NPC skill use | sonnet | CAST-01, EFX-01, EFX-02 | AttackableAI's skill branches (`AISkillScope`: buff, heal, debuff, attack, ranged, suicide), the `// M5:` markers from AI-01 |
| M5-LEARN-01 | Learning skills at trainers | sonnet | GPROTO-08 | AcquireSkillList / RequestAcquireSkill (SP, items, tree checks), `AutoLearnSkills=True`, the skill-tree iteration order noted in M4-EXP-01 |
| M5-SAVE-01 | Buffs and reuse times across relog | haiku | EFFECT-01, CAST-01 | `character_skills_save` like Java (`storeEffect`/`restoreEffects`), reuse timestamps; D-39: the Java GS reads the same rows |
| M5-ADMIN-04 | Admin commands for skills | haiku | CAST-01 | //give_skill, //remove_skill, //skill_list, //cast, //buff-style commands as Java registers them |
| M5-E2E-01 | End to end: cast, buff, relog, potion, soulshot | sonnet | ITEM-01, SHOT-01, SAVE-01 | two fake clients, CI MariaDB |
| M5-RUN-01 | Runbook and milestone checks | opus | E2E-01 | |

18 cards: 3 opus (EFFECT-01, CAST-01, RUN-01 + reviews), 9 sonnet, 6 haiku.

## Checkpoints

| Checkpoint | You test |
|---|---|
| CP-M5a | Potions, herbs and scrolls work (HP potion heals over time, herbs give their buff); soulshots glow and hit harder; auto soulshots switch on and off. |
| CP-M5b | Cast attack, heal and buff skills; buff icons and remaining times match Sieghardt; buffs and skill reuse survive a relog and a move to Sieghardt. |
| CP-M5c | Monsters use their skills (healers heal, mages cast); spoil and sweep; learn a new skill at a trainer. |
| CP-M5d | Milestone: a normal play session with skills and consumables on Kain, then the same character on Sieghardt with its buffs and reuse times intact. |

## Open questions for the owner

1. **Summons, pets, cubics, transformations:** their skills are part of the skill system, but each brings a lot of its own (pet data, pet inventory, servitor AI). Recommended: **M6**, together with parties.
2. **Parties:** party buffs and heals target the party. Without parties those skills hit only yourself. Recommended: parties (and clans) in **M6 "social"**; M5 treats "party" as "just me", like Java does for a player without a party.
3. **Order inside M5:** consumables first (CP-M5a, quick win: herbs, potions, soulshots) or casting first (CP-M5b)? Recommended: engine and casting first, because potions and herbs *are* skills; CP-M5a comes right after.
