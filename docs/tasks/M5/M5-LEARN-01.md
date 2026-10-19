# M5-LEARN-01: Learning skills at trainers

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M5-GPROTO-08 |
| Size | M |

## Goal
`RequestAcquireSkillInfo`, `RequestAcquireSkill`, `AcquireSkillList` at a class trainer (the bypass from the trainer's html), SP and item costs, the tree checks, `AutoLearnSkills=True` (the M4-EXP-01 fallback), `SkillList` after learning, DB rows (`character_skills`) as Java.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{RequestAcquireSkill,RequestAcquireSkillInfo}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\data\xml\SkillTreeData.java (getAvailableSkills, isSkillAllowed)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\Trainer.java / Folk.java (showSkillList)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java (rewardSkills AutoLearnSkills branch)`

## Files
- `crates/l2-world/src/skills/learn.rs (new)`
- `crates/l2-world/src/{bypass.rs,rewards/exp.rs} (wire-in)`
- `crates/game-server/src/client/* (dispatch)`
- `crates/l2-world/tests/learn.rs (new)`

## Requirements
1. Same lists, costs, messages and DB rows as Java (D-39).

## Out of scope
- Enchanting skills, clan/pledge/fishing/transform skill trees (later).

## Tests (must exist and pass)
- Learn a skill, not enough SP, missing book, AutoLearnSkills.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-LEARN-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md (safety rules: no local DB, no real ports, never kill processes by name), `docs/design/m4-combat.md`, `docs/design/m5-skills.md` first.
- The orchestrator refines this card when its dependencies are merged (exact module names and APIs to call).
