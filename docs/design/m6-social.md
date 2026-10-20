# Design: parties, PvP, trade, clans and companions (M6)

Builds on `docs/design/m5-skills.md`. Owner decisions (2026-10-20): sieges/castles/fortresses/clan
halls are M7, Olympiad/Seven Signs/festivals M8, quests and scripts after that. Order inside M6:
party, PvP and trade first (CP-M6a), then clans, then summons, pets, cubics and transformations.

## Where it lives
| Piece | Crate / module | Notes |
|---|---|---|
| Parties | `l2-world` `party/` | `PartyId` (D-11 ids), a party table in the world; members by `ObjectId` |
| PvP flag, karma, relations | `l2-world` `pvp.rs` | replaces the M3 relation stubs in `player_view.rs` |
| Trade, private stores | `l2-world` `trade/` | item moves through the inventory module; rows through the persistence outbox |
| Clans | `l2-world` `clan/` + `l2-db` clan repositories | clan state is loaded at startup like Java's `ClanTable` (read-only scan is fine), changed in the world, written through the outbox exactly as Java writes it (D-39: the Java GS loads the same clans at its next start; live changes on Sieghardt are not seen until then, same as Java between two servers) |
| Summons, pets, cubics | `l2-world` `summon/` | a summon is a creature in the world's tables with an owner id; its AI uses the M4 intention machine |
| Transformations | `l2-world` `transform/` | the stat engine's transformation hooks (M4-STAT-01 left them as `None`) |

## Rules
- Party/clan/summon references are ids; no back pointers. The `// M6:` markers in the target
  handlers, rewards, effects and combat are the integration points: each card lists the ones it
  resolves.
- Shared DB (D-39): clans, pets, items moved by trade, karma/pk columns are written exactly like
  Java. A character can switch servers between sessions; both servers must read what the other wrote.
