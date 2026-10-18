# M3-TALK-01: Targeting, NPC dialogs, bypasses, gatekeepers

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | sonnet |
| Review | full |
| Reference | no |
| Depends on | M3-NPC-01, M3-DATA-08, M3-TELE-01, M3-GPROTO-06 |
| Size | M |

## Goal
Action (click) -> target select (MyTargetSelected/TargetSelected), second click -> talk (NpcHtmlMessage from the html cache with Java's file choice and replacements), RequestBypassToServer/RequestLinkHtml handler registry with the core bypasses (npc chat/link, Teleporter teleport lists with costs), RequestTargetCanceld.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\clientpackets\{Action,RequestBypassToServer,RequestLinkHtml,RequestTargetCanceld}.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\Npc.java (showChatWindow, getHtmlPath)`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\instance\Teleporter.java`
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\actions, handlers\bypass (only the ones needed: Link, Chat, teleporter path)`

## Reference implementation to copy
- crates/l2-world (M3-WREG-01, M3-NPC-01)

## Files
- `crates/l2-world/src/{target.rs,dialog.rs,bypass.rs}`
- `crates/l2-world/tests/dialogs.rs`

## Requirements
1. Java's distance checks (interaction range) and ActionFailed answers.
2. Teleport costs (adena) deducted as Java and stored through persistence.
3. Unknown bypasses are logged and ignored like Java.

## Out of scope
- Quests, shops, other NPC types (later).

## Tests (must exist and pass)
- Click/talk/teleport flow with a fake player against a gatekeeper template.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-TALK-01: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read `docs/design/m3-world.md` (M3 rules) and `docs/design/world-loop.md` first. Then the CLAUDE.md safety rules (never touch a local database or real ports).
- The owner's real Java GS and DB run side by side with our servers (D-39): anything written to the DB must look exactly like Java writes it.
