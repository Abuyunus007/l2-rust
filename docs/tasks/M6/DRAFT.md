# M6 draft: social and companions (for the owner's review — no worker starts before sign-off)

**Done when:** you play together on Kain: form a party (shared exp and loot, party buffs and heals
reach the party), trade with another player, create or join a clan (clan window, crest, ranks,
clan skills, clan warehouse), summon a servitor or pet that fights and follows you, use cubics and
transformations; PvP flags, karma and PK work like Java. Everything stays readable for Sieghardt on
the same DB (D-39).

What M4/M5 leave for M6 (the `// M6:` markers): party targets and party exp/drops (`Party.distributeXpAndSp`,
loot rules, `RatePartyXp`), summons/pets/servitors (summon effects, pet data, pet inventory, beast/spirit
shots for summons), cubics, transformations, PvP flagging and karma (`updatePvPStatus`,
`onKillUpdatePvPKarma`, `RelationChanged` relations), clans (clan targets, clan skills, crests),
private trade, duels.

Java size, for scale: `Clan.java` 3,068 lines (+ ClanMember 817), `Summon.java` 1,272, `Pet.java` 1,449,
`Servitor.java` 566, `Party.java`, ~51 client packets for pledge/clan/ally/party.

## Proposed cards

| Card | Title | Model | Depends on | Notes |
|---|---|---|---|---|
| M6-GPROTO-09 | Party, trade, clan, summon packets | haiku | — | the ~60 client/server packets (pledge*, party*, trade*, pet*/summon* info and status, ExPartyPetWindow*) |
| M6-PVP-01 | PvP flag, karma, PK, relations | sonnet | GPROTO-09 | `updatePvPStatus`, `PvpFlagTaskManager`, karma gain/loss, PK count, `RelationChanged` with the real masks, name colors, karma drops (M4-DROP-01 hook) |
| M6-PARTY-01 | Parties | sonnet | GPROTO-09 | invite/accept/leave/kick, loot rules, party window, `distributeXpAndSp` (+ vitality), party drops/sweep, party targets in the skill target handlers (`// M6:` markers) |
| M6-TRADE-01 | Private trade | sonnet | GPROTO-09 | TradeRequest/AddTradeItem/TradeDone, item transfer rows (D-39) |
| M6-STORE-01 | Private stores (sell/buy) | sonnet | TRADE-01 | sit-down shops, store messages, offline trade only if trivial |
| M6-CLAN-01 | Clan core | **opus** | GPROTO-09 | create/disband/join/leave/kick, ranks and privileges, clan level, reputation, clan window packets, `clan_data`/`clan_privs`/`clan_subpledges` rows as Java (D-39), the Java GS sees the same clan |
| M6-CLAN-02 | Crests, alliances, clan wars | sonnet | CLAN-01 | crest upload/cache, alliance create/join, war declare/accept, relation masks |
| M6-CLAN-03 | Clan skills, clan warehouse, academy | sonnet | CLAN-01 | pledge skills (`AcquireSkillType.PLEDGE`), clan warehouse, academy graduation |
| M6-SUMMON-01 | Summon core: servitors | **opus** | GPROTO-09 | `Summon`/`Servitor`: summon skills (`Summon` effect), follow/attack AI, summon info packets, shared exp, unsummon, summon items |
| M6-PET-01 | Pets | sonnet | SUMMON-01 | pet data XML, pet inventory, feeding, levels, `pets` rows (D-39), pet items/collars |
| M6-CUBIC-01 | Cubics | sonnet | SUMMON-01 | cubic data, `SummonCubic` effect, cubic skills (`calcMagicDam` cubic branch) |
| M6-TRANS-01 | Transformations | sonnet | — | transformation data, `Transformation` effect, transformed stats/skills/shortcut rules |
| M6-DUEL-01 | Duels | haiku | PARTY-01, PVP-01 | duel request/start/end, party duels |
| M6-ADMIN-05 | Admin commands for social/summons | haiku | CLAN-01, SUMMON-01 | //clan*, //party*, //summon, //karma, //pk |
| M6-E2E-01 | End to end: party hunt, trade, clan, summon | sonnet | PARTY-01, TRADE-01, CLAN-01, SUMMON-01 | |
| M6-RUN-01 | Runbook and milestone checks | opus | E2E-01 | |

16 cards: 3 opus (CLAN-01, SUMMON-01, RUN-01 + reviews), 10 sonnet, 3 haiku.

## Checkpoints

| Checkpoint | You test |
|---|---|
| CP-M6a | Two players on Kain: party, shared exp and loot, a party buff reaches both; a trade between them; PvP flag and karma after a PK. |
| CP-M6b | Clan: create, invite, ranks, crest, clan skills; the same clan shows on Sieghardt. |
| CP-M6c | Summon a servitor and a pet; they follow, fight and level; cubics; a transformation. |
| CP-M6d | Milestone: a party session with summons and a clan on Kain, then the same characters on Sieghardt. |

## Open questions for the owner

1. **Sieges, castles, fortresses, clan halls, Olympiad, Seven Signs** are big systems of their own. Recommended: **M7 "world events"** (sieges + castles + clan halls) and **M8** (Olympiad, Seven Signs, festivals), after M6.
2. **Quests and scripts** (D-05: scripts are ported last): recommended as their own milestone after M7/M8, starting with the quest engine and the most used quests.
3. **Order inside M6:** party/PvP/trade first (CP-M6a, quick multiplayer win), then clans, then summons? Recommended: yes, in that order.
