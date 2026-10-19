# Design: skills, effects and consumables (M5)

Builds on `docs/design/m4-combat.md`. Owner decisions (2026-10-20): summons, pets, cubics,
transformations and parties are M6; "party" targets mean "just me" in M5, as Java does for a
player without a party; the effect engine and casting come first, consumables right after.

## Where it lives
| Piece | Crate / module | Notes |
|---|---|---|
| Skill templates, effect templates | `l2-data` `skill/` (M4-DATA-11) | already loaded; effect `params` are raw maps the handlers read |
| Effect list per creature (`EffectList`, `BuffInfo`) | `l2-world` `effects/` | a side table keyed by object id, like `status.rs` / `combat/` |
| Effect handlers | `l2-world` `effects/handlers/` | one registry: name → constructor of a small `EffectHandler` trait object or enum variant, built once; unknown names log once and do nothing (Java `createEffect` returns null) |
| Target handlers | `l2-world` `skills/targets.rs` | Java `TargetType` → function |
| Casting | `l2-world` `skills/cast.rs` | `doCast`, timers (`MagicUseTask` phases) on the timer wheel |
| Skill formulas | `l2-world` `formulas/` | extend M4-FORM-01, vector-checked |
| Item handlers | `l2-world` `inventory/handlers/` | registry like the effect handlers |
| Buff/reuse persistence | `game-server` `persist.rs` + `l2-db` | `character_skills_save`, Java's columns (D-39) |

## Rules
- Durations, ticks (`ticks` of over-time effects), cast phases, reuse: timer wheel only. An
  effect's timers belong to its `BuffInfo` id, so removing the effect cancels them.
- Stat functions of effects go through the stat engine with `FuncOwner::Effect { .. }` (M4-STAT-01
  owners), added/removed as Java's `BuffInfo.addStats`/`removeStats` do.
- Stacking (abnormal type/level, `EffectList` replace rules, buff/dance/debuff/trigger limits)
  exactly as Java; the client sees the same icons in the same order (`AbnormalStatusUpdate`).
- Randomness from the world RNG; formulas take the RNG trait (vectors replay Java's draws).
- Every handler names its Java class in a `// Java:` comment; quirks are kept with `NOTE(java-quirk)`.
