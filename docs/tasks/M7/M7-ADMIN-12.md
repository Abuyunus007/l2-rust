# M7-ADMIN-12: Admin: the rest of edit char, spawn and the admin page

| Field | Value |
|---|---|
| Milestone | M7 (admin completion) |
| Model | sonnet |
| Review | full |
| Reference | `crates/l2-world/src/admin/` (ADMIN-08..11 merged) |
| Depends on | M7-ADMIN-09 |
| Size | L |

## Goal
Finish what M7-ADMIN-09 left (its module headers list it). Small wire-ins outside `admin/` are allowed (add lines; keep them minimal):
- `changelvl` and the `%access%` name: keep the access level names of `AccessLevels.xml` in the rights data.
- `repair`/`restore` (AdminRepairChar): a persistence command for its SQL (D-15).
- `fight_calculator`, `fight_calculator_show`, `fcs`: build the stat inputs of two off-world monsters from their templates for the existing pure formulas.
- `tracert`, `strict_find_dualbox`: hand the EnterWorld trace hops to the world.
- `setcolor`: a player name colour (`characters.name_color` as Java stores it).
- GM flags `gmliston`/`gmlistoff` (the GM list, `/gmlist`), `silence` (refuse private messages), `diet` (no weight penalty), `tradeoff` (refuse trades), each with the code that reads it.
- `setconfig`, `config_server`: the runtime rate/config changes Java's page allows, on world settings that become mutable for those keys.
- `respawnall`, `spawn_reload`, `spawn_misc`/`spawnmisc`, `list_pools`.
- The partials: `setclass` (class template, skills, hennas, MagicSkillUse, party/clan class icons, the texture refresh as a timer: transform 105, untransform 200 ms later), `setsex` (texture refresh), `changename` (the creation name format check), `setnoble` (noble skills), `setfame` (`EnableFameSystem`).

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\chat\commands\admin\{AdminEditChar,AdminAdmin,AdminSpawn,AdminRepairChar,AdminFightCalculator,AdminChangeAccessLevel}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Player.java` (setClassId, setSilenceMode, setDietMode, setTradeRefusal, setAppearance name colour)

## Files
- `crates/l2-world/src/admin/*` + the wire-ins named above; `crates/game-server/src/{persist.rs,world_data.rs,client/enter.rs}` where needed; tests

## Requirements
1. Java behaviour, messages, HTML and DB rows exactly (D-39, D-49..D-52).
2. Single-owner world, ids not references, timers on the wheel; no unwrap/expect on external data; no DB call in a tick (D-15).
3. Olympiad/hero/instance words stay out (M8).

## Tests (must exist and pass)
- Each command or flag above with its message/page and its effect.

## Done when
- [ ] fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`, `-j 4`); `git merge main` before the final checks
- [ ] committed as `M7-ADMIN-12: <summary>`
- [ ] report delivered (every command left and why)

## Notes from the orchestrator
- Read CLAUDE.md (safety rules), DECISIONS first. Other workers may run: in shared files only add your lines; watch for doubled `mod`/`use` lines after merging main.
