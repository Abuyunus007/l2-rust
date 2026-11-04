# M5-EFX-05: Effect handlers that need casting or new world parts (rest of EFX-03)

| Field | Value |
|---|---|
| Milestone | M5 |
| Model | sonnet |
| Review | full |
| Reference | no (copy the merged handlers) |
| Depends on | M5-CAST-01 |
| Size | M |

## Goal
CallSkill (`makeTriggerCast`), TriggerSkillByAvoid / ByDamageDealt / ByDamageReceived / BySkill (creature events from combat and casting: attack avoided, damage dealt/received, skill used), TargetMe (`Playable.setLockedTarget`), Resurrection/ResurrectionSpecial player branch (`Player.reviveRequest` + `ConfirmDlg`/`DlgAnswer` for resurrection), NevitsHourglass (a pause setter in `rewards/nevit.rs`), OpenChest (special drop, `mustRewardExpSp`), ConvertItem (item elementals/augmented flag), Grow (per-NPC collision radius), RestorationRandom (`RateExtractable` into the settings), the `FlyToLocation` packet for Blink/ThrowUp/TeleportToTarget, and the `CancelReturn` config for the dispel handlers.

## Java source (the spec)
- `C:\l2-rust\l2jmobius\dist\game\data\scripts\handlers\skill\effects\<each name>.java`
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\entity\actor\{Creature,Player,Playable}.java` (the parts named above), `network\serverpackets\{FlyToLocation,ConfirmDlg}.java`, `network\clientpackets\DlgAnswer.java`

## Files
- `crates/l2-world/src/effects/handlers/*.rs`, `handlers/mod.rs`; small additive wire-ins in `combat/`, `skills/cast.rs`, `rewards/nevit.rs`, `npc.rs`, `data.rs`; packets in `crates/l2-protocol-game` (+ proxy decoders); tests.

## Requirements
1. Java behaviour and messages; the event hooks are plain world calls at the places Java fires the listeners.

## Out of scope
- Summons/parties (M6).

## Tests (must exist and pass)
- Per handler.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass (`CARGO_INCREMENTAL=0`)
- [ ] committed as `M5-EFX-05: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read CLAUDE.md, `docs/design/m5-skills.md` first.
