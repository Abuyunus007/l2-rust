# Ideas for after the main work

A list of features to build **after** the port is finished and the switch from Java is done.
Nothing here is planned or promised. When an idea is picked, it gets a task card.

**Rules for every idea**
- Retail H5 stays the default. Each idea is a switch in its own Rust-only `.ini` (or a key in
  `Custom/`), off by default, like the Java customs (D-57).
- Side by side with Java (D-39) must keep working: anything that writes rows for many players or
  global state follows the `*DatabaseWrites` switches (D-53).
- New tables go into `dist/db_installer/sql/` through `tools/sync-dist.py`.

Effort: **S** = a few days of one worker, **M** = one card, **L** = several cards.

---

## Proposals

### Quality of life
| # | Idea | What it does | Effort |
|---|---|---|---|
| Q1 | **Player menu `.menu`** | One HTML window with the voiced commands (exp off, auto loot, auto play, language, online, ...) as buttons instead of typed commands. | S |
| Q2 | **Drop and spawn finder** | Community board page: search an item, see which monsters drop it and where they are (on the map); search a monster, see its drops with the real rates of this server. Builds on the drop search board. | M |
| Q3 | **Auto loot options** | Per player: auto loot on/off, only adena, herbs off. (Java has a global switch only.) | S |
| Q4 | **Free teleport bookmarks for low levels** | Bookmarks without the item cost up to level 40 (helps new players). | S |
| Q5 | **Buff profiles at the NPC buffer** | Save up to 3 buff profiles per character, rebuff with one click (scheme buffer exists; this adds "rebuff all + pet"). | S |

### Progression and economy
| # | Idea | What it does | Effort |
|---|---|---|---|
| E1 | **Daily and weekly missions** | A small list per character (kill N monsters in your level range, do a quest, win an Olympiad match) with rewards; reset at 06:30 like the daily reset. | M |
| E2 | **Login rewards (attendance)** | A reward for each day logged in, a bigger one every 7 days; shown in a window at enter world. | S |
| E3 | **Auction house** | A server-wide market board: list items with a price and duration, others buy offline; fee in adena; replaces long offline-shop rows in towns. | L |
| E4 | **Vote rewards** | Reward items for votes on server-list sites (polling their API through the supervisor, never from the world). | M |
| E5 | **Adena sink: item upgrade / exchange** | An NPC that trades old grade sets for upgrade materials, to keep the economy from inflating at 5x rates. | M |

### PvP and events
| # | Idea | What it does | Effort |
|---|---|---|---|
| P1 | **Arena queue 1v1 / 2v2 / 5v5** | Register anywhere, get matched by rating (ELO), fight in an Olympiad-like instance, weekly ranking with rewards. Builds on the Olympiad and the ported events. | L |
| P2 | **Event scheduler in the GUI** | Plan TvT, CTF, Deathmatch, Race and the seasonal events on a calendar in the admin GUI instead of `config.xml` cron lines. | M |
| P3 | **PvP zone of the day** | A rotating zone with bonus drops and a PvP flag for everyone inside, announced at 06:30. | M |
| P4 | **Kill streaks and titles** | Announcements and short buffs for kill streaks, titles for PvP milestones. | S |

### Community and achievements
| # | Idea | What it does | Effort |
|---|---|---|---|
| C1 | **Achievements** | Counters for kills, quests, bosses, PvP, crafting; titles and small rewards; a community board page per character. | L |
| C2 | **Discord bridge** | Shout/trade/hero chat and boss/siege announcements go to Discord channels (webhook from the supervisor, read-only by default). | M |
| C3 | **Public website data** | A read-only JSON/HTML export: online count, top PvP/PK, clans, castle owners, boss status, Olympiad heroes. The supervisor serves it; no DB access from the web. | M |

### Server operation (uses the Rust side)
| # | Idea | What it does | Effort |
|---|---|---|---|
| O1 | **Hot reload of the datapack** | Reload XML data (multisells, buylists, drops, spawns) from the GUI without a restart, applied between ticks. Java needs a restart for many of them. | L |
| O2 | **Metrics and graphs in the GUI** | Online players, tick time, memory, packets per second over the last 24 hours as charts; alerts in the GUI when the tick time goes up. | M |
| O3 | **Anti-bot heuristics** | Flags for suspicious players (same path loop, no chat, 24h online, exact timings) listed in the GUI for a GM to check; no automatic bans. | L |
| O4 | **GM session replay** | Record the packets of one player on request (the proxy already records) and replay it in the GUI for investigations. | M |
| O5 | **Scheduled restarts** | A daily restart with countdown and announcements, set in the GUI (the shutdown code exists). | S |

---

## Recommendation: where to start

1. **O5 scheduled restarts and O2 metrics.** Small, and useful from the first day after the switch.
2. **Q1 player menu and Q2 drop finder.** Players notice them at once, and the risk is low.
3. **E1 daily missions and E2 login rewards.** They keep players coming back. Keep the rewards small so the 5x economy stays sound.
4. **P1 arena queue.** The biggest PvP draw, but it's an L. Do it once the server has players.
5. **Wait with E3 auction house and O1 hot reload.** Both are large and touch many systems.

---

## Owner's ideas

_Add your ideas here, one line each. They are worked through with the proposals above once the main work is finished._

| # | Idea | Notes |
|---|---|---|
| | | |
