# L2Rust vs. L2JMobius CT 2.6 High Five: what is different

State of 2026-11-10 (commit `1a3e86d7`). The reference is the L2JMobius CT 2.6 H5 release the
port started from. The binding decisions behind each point are in `docs/DECISIONS.md` (D-xx).

## 1. Gameplay changes (what players notice)

| What | Java original | L2Rust | Where |
|---|---|---|---|
| **Buff durations** | Songs and dances 2 min; Prophecies, Chant of Victory, Magnus' Chant, Victory of Pa'agrio 5 min | `EnableModifySkillDuration = True`; those 35 support buffs last **20 min** (1200 s). Longer buffs and combat skills are unchanged | `config/Player.ini` |
| **Class Buffer** (D-60) | none | NPC 50011 next to all 21 town gatekeepers: free buffs, songs and dances by class group, for pets and servitors too. `.buffme` opens it anywhere for level × 1,000 Adena | NPC, spawns, script `custom/ClassBuffer` |
| **Companions** (D-61) | none | Hire up to 3 virtual party members (classes of your class tier, 8 roles) at a gatekeeper or with `.companion`, for level × 5,000 Adena. They fight with their class skills, heal, buff, summon, come back at login, move up their class line with you and show in the party window. Exp and drops go to you | NPCs 50021-50029, gatekeeper pages, `L2R.ini [Companions]` |
| **Rift invasions** (D-64) | none | Random rifts in 5 zones (levels 40-84) with waves and a boss (Anakazel). Rift shards (item 4405, bound to the character, weekly cap) are traded at the Rissbeschwoerer in Giran (NPC 50030, multisell 90000001). The zone fits the players online; monster levels can adapt | `data/RiftInvasion.xml`, `L2R.ini [RiftInvasion]` |
| **Login server list** (D-62) | Shows every registered game server, the ones that are down too | `ShowDownServers = False` (shipped): only the servers that are up | `login/config/L2R.ini` |
| **Datapack fix** | `classList.xml`: Bladedancer and Elemental Master have the wrong parent class | Parents corrected (Palus Knight, Elemental Summoner), as Java's own `PlayerClass` and skill trees have them | `data/stats/players/classList.xml` |
| **Bound item** (D-64) | Item 4405 (Blue Dimensional Stone - Shard) is a free, unused item | As the rift shard it cannot be traded, dropped, sold in a private store, mailed, sent by freight or put into the clan warehouse (`RiftShardBound`) | engine, `L2R.ini` |

Everything else plays like L2JMobius: the same rates, formulas, skills, quests (all 509), AIs,
bosses, instances, sieges, Olympiad, Seven Signs and the same `.ini` keys and defaults.
The Java custom features (`config/Custom/*.ini`, the custom events) are there and, as in Java,
**off by default** (D-57).

## 2. New settings files (Java's own files are never extended, D-36)

| File | What |
|---|---|
| `game/config/L2R.ini` | Companions and rift invasions; `//reload l2r` while running (D-62) |
| `login/config/L2R.ini` | `ShowDownServers` (D-62) |
| `config/Admin.ini` (each server) | The admin endpoint of the server (D-30) |
| `bin/supervisor.ini` | The supervisor: which servers it starts, its port and token |
| `bin/Backup.ini` | Scheduled database backups (mysqldump) |

## 3. Tools that Java does not have

| Tool | What |
|---|---|
| **Supervisor** (`l2-supervisor`) | Starts, stops and restarts login and game server, collects their logs, stops them cleanly through the admin API (D-38) |
| **Admin GUI** (`l2-admin-gui`) | Server control, live logs, setup wizard, database installer, accounts, game admin (players, punishments, announcements, console, shutdown), config editor (every `.ini`), backups, **packet inspector** with a decrypting proxy (D-31..D-35, D-56, D-59) |
| **AI admin chat** (D-63) | GUI view "AI assistant", `l2-ai` (terminal) and `l2-mcp` (MCP bridge for Claude Desktop and others): a local model (Ollama, llama.cpp) runs admin commands; bans, kicks and shutdowns only after you confirm; audit log |
| **Admin API** | WebSocket + JSON, localhost by default, with a token (D-30) |
| **Database installer** (`l2-db-installer`) | Creates the L2JMobius schema (GUI and console) |
| **Geodata tool** (`l2-geotool`), **load test** (`l2-loadtest`), **proxy** (`l2-proxy`) | Geodata statistics and checks; a load test for test setups only; a recording proxy |

## 4. Technical differences (not visible in play)

- **Rust** instead of Java. Windows and Linux (D-03, D-27).
- **One world loop** owns all game state; network, database and AI talk to it through channels.
  No locks around game objects (D-10, D-11).
- **One 50 ms tick** for everything. Java uses 50 ms for players and 100 ms for NPCs (D-12).
  Java's thread pool tasks and quest timers are one timer wheel.
- **Database saves never block the game**: they go to a separate task (D-15). Same MySQL/MariaDB
  schema as Java (D-21).
- **Scripts are compiled in**: no script hot-reload; a script change needs a restart. Data,
  HTML and config still reload live (D-16).
- **New object ids** come from `0x60000000` upwards (Java: `0x10000000`). A leftover of the
  side-by-side time; harmless (D-39 → D-58).
- **Own datapack copy** in `dist/`, no L2JMobius install needed (D-55). The Java tools
  (code generators, vector programs) are gone; the tests protect the behaviour (D-58).
- **Graceful stop** through the admin API, never by killing the process (D-38).

## 5. Known gaps (from notes in the code, not checked one by one)

The code marks places where a Java detail is missing. Some of these notes are older than the
systems around them and may be outdated; a checked list needs a review pass.

- HWID punishments and `MaxPlayersPerHWID` (no hardware id from the client).
- The item audit log (`LogItems`) and the Olympiad fight log (`OlympiadLogFights`).
- `default-ipconfig.xml` auto-detection of the network interfaces (the address comes from the
  config instead).
- Seed of Infinity spawn/stop functions of the Energy Seeds script; parts of some instance and
  boss scripts (Pailaka Injured Dragon, Beleth, Last Imperial Tomb: see the notes in their files).
- A few effect handlers run as stand-ins (their stat parts work, their action does nothing; the
  server logs each one at start).
- Small admin command details: `//setclass` does not re-apply item skills, `//setsex` does not
  refresh the look at once.
- Client "next action" callbacks of skill packets, chaos monsters, transfer pain of summons.
- Script hot-reload (by design, see above).
