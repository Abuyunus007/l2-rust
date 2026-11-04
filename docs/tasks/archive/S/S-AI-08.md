# S-AI-08: The AI scripts the AI batches left

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | full |
| Reference | `docs/design/scripts.md`, the ported AIs in `crates/l2-scripts/src/ai/` |
| Depends on | AI batches |
| Size | L |

## Goal
Port the AI scripts that S-AI-03, S-AI-05 and S-AI-06 left, each as one file under the right `crates/l2-scripts/src/ai/` folder, with tests:
- `ai/areas/FourSepulchers` (the XML-driven rooms; its quest Q00620 FourGoblets is in S-QR-xx: coordinate by name), `ai/areas/SelMahumTrainingGrounds/SelMahumSquad`, `TarBeetle` and `TarBeetleSpawn` (`tar_beetle.xml` polygons, random point in a polygon).
- `ai/others/WyvernManager`, `SiegeGuards` (check what the world's native siege guard AI already does; port only what is missing), `NpcBuffers` (NpcBuffersData.xml), `DivineBeast`, `Minigame`, `Katenar` (with Q00065's npc0/player0 variables), `HealerTrainer` (the transfer skills: tree, list, learn, remove with the refund).
- Ballista: use `set_mortal` for Java's `setMortal(false)`.
If a hook is missing that S-API-05 adds at the same time, merge main and use it; otherwise mark `// FOLLOW-UP(engine):` and list it.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\{FourSepulchers,SelMahumTrainingGrounds,TarBeetle}\**`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\others\{WyvernManager,SiegeGuards,NpcBuffers,DivineBeast,Minigame,Katenar,HealerTrainer}\**`

## Files
- `crates/l2-scripts/src/ai/**`, small pub world helpers where missing (add your lines only); tests

## Requirements
1. Java behaviour, messages, html and rows exactly (D-39, D-54); scripts hold no state; timers on the wheel; world RNG.

## Tests (must exist and pass)
- One test per script.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-AI-08: <summary>`
- [ ] report delivered

## Notes from the orchestrator
- Disk space is tight: set CARGO_PROFILE_DEV_DEBUG=0 and CARGO_PROFILE_TEST_DEBUG=0. Reuse existing helpers; never add a helper whose name already exists.
