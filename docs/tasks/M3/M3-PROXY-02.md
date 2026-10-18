# M3-PROXY-02: Proxy field decoders for the M2/M3 server packets

| Field | Value |
|---|---|
| Milestone | M3 |
| Model | haiku |
| Review | spot |
| Reference | no (copy M2-PROXY-01) |
| Depends on | M3-GPROTO-05, M3-GPROTO-06, M2-ENTER-01 |
| Size | S |

## Goal
The proxy fills `fields` for every server packet that now has a `Decode` impl in `l2-protocol-game` but still shows `null` in captures: CharInfo, NpcInfo, DeleteObject, RelationChanged, StatusUpdate, ExSetCompassZoneCode, MoveToLocation, StopMove, MoveToPawn, TeleportToLocation, MyTargetSelected, TargetSelected, TargetUnselected, NpcHtmlMessage, DoorInfo, DoorStatusUpdate, StaticObjectInfo, MacroList, ExQuestItemList, ExRotation, ExBrExtraUserInfo, the Nevit packets, ActionFailed.

## Java source (the spec)
- none

## Reference implementation to copy
- crates/l2-proxy/src/game_fields.rs (how the existing server packets are described)

## Files
- `crates/l2-proxy/src/game_fields.rs`

## Requirements
1. Decoding never blocks or breaks forwarding; a decode error gives `{"decode_error": ...}` like today.
2. Personal data stays out as today (names are game data, not account data; session keys stay redacted).

## Out of scope
- New packet structs.

## Tests (must exist and pass)
- One test per packet family; a sweep over the committed fixtures (`fixtures/captures/java-gs-*.jsonl`) that every server packet with a decoder now gets fields without `decode_error`.

## Done when
- [ ] requirements met; fmt, clippy -D warnings, test pass
- [ ] committed as `M3-PROXY-02: <summary>`
- [ ] report delivered in the agent's report format

## Notes from the orchestrator
- Read the CLAUDE.md safety rules first (never touch a local database or real ports).
