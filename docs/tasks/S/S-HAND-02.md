# S-HAND-02: The handlers S-HAND-01 left

| Field | Value |
|---|---|
| Milestone | Scripts |
| Model | sonnet |
| Review | full |
| Reference | `crates/l2-world/src/handler_registry.rs` (S-HAND-01's audit list) |
| Depends on | S-HAND-01 |
| Size | L |

## Goal
Port the handlers S-HAND-01 listed as left in `handler_registry.rs`, and update that list:
- NPC bypasses: `SupportMagic` and `SupportBlessing` (the newbie buffs the NPCs cast), `RentPet`,
  `Loto` (lottery, `games` table, `LotteryManager`), `BuyShadowItem`, `Wear` (shop preview lists;
  L-02 already has `RequestPreviewItem`), `FindPvP`, `VoiceCommand`, `ReceivePremium` only as
  Java answers without premium.
- User commands: `Unstuck` (the long escape cast with Java's times and checks), `MyBirthday`
  (load `characters.createDate`).
- Voiced commands: `ExperienceGain` (`.expon`/`.expoff`, the `EXPOFF` variable restored at login);
  the config-gated others when their config is off answer as Java does.
- Items: `CharmOfCourage`, `RollingDice` (with its flood protector), `EventItem` (Bond / Land Mine
  only where the block checker needs them; see M8-GAMES-01's follow-up).
- Creatures: `Decoy` and `Trap` (the skill effects that summon them, their AI and `DecoyClick` /
  `TrapClick`).
- `NpcViewMod` and `DropSearchBoard`: the shared drop-rate code with player bonuses
  (`AltGameViewNpc`), the `Custom/dropsearch` pages only when the config enables them.
- Fix `admin/summon.rs` `intention_name` (Java's `Intention.name()` is `IDLE`, not
  `AI_INTENTION_IDLE`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\bypass\npc\*.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\{user,voiced}\*.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\items\*.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\actions\click\{DecoyClick,TrapClick}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\{Decoy,Trap}.java`

## Files
- `crates/l2-world/src/*` (new files per handler group) + the registries and `handler_registry.rs`
- crates/game-server, crates/l2-db where a row is needed; tests

## Requirements
1. Java behaviour, messages, packets, HTML and DB rows exactly (D-39, D-49..D-54).
2. Single-owner world, ids not references, timers on the wheel, world RNG; no unwrap/expect on external data; no DB call in a tick (D-15).

## Tests (must exist and pass)
- One test per handler with its page or message and effect; the registry audit updated.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `S-HAND-02: <summary>`
- [ ] report delivered (every handler left and why, every guess)

## Notes from the orchestrator
- Read CLAUDE.md (safety rules) and DECISIONS first. The script engine (S-API-01) and subclasses (S-SUB-01) are written at the same time: in shared files only add your lines; run `git merge main` every hour or so.
