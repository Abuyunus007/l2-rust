# L2JMobius High Five 2.6: codebase analysis

Phase 0, step 2. Read-only analysis of the Java reference at `C:\l2-rust\l2jmobius`.
Nothing here is a decision yet. Decisions go in `docs/DECISIONS.md` once we agree on them.

| Doc | Topic |
|---|---|
| [01-login-server.md](01-login-server.md) | Login server: client protocol, LS↔GS protocol, DB, behaviour (porting spec for milestone 1) |
| [02-network-crypto.md](02-network-crypto.md) | Framing, Blowfish/RSA/XOR crypto, packet dispatch, Java threading model |
| [03-config-db-data.md](03-config-db-data.md) | `.ini` configs, MySQL schema, XML datapack, HTML |
| [04-world-and-threading.md](04-world-and-threading.md) | Entity hierarchy, world regions, task managers, scripts API (main architecture risk) |
| [05-geodata.md](05-geodata.md) | `.l2j` format and comparison of the two geodata sets |

## Size

| Area | Files | Lines |
|---|---:|---:|
| `commons` (network, crypt, db, threads, config) | 38 | 9,620 |
| `log` | 27 | 1,356 |
| `loginserver` | 56 | 7,447 |
| `tools` | 4 | 4,655 |
| `gameserver` (core) | 1,778 | 341,566 |
| Scripts (`dist/game/data/scripts`) | 1,291 | ~302,000 |
| **Total Java** | **~3,200** | **~663,000** |

Data: ~2,900 XML files (85 MB), 8 MB of HTML dialogs, 73 `.ini` files with ~1,576 keys, 112 game SQL tables + 4 login tables.

### Game server core by package (lines)

| Package | Lines | Notes |
|---|---:|---|
| `entity` | 105,809 | `Player.java` 15.4k, `Creature.java` 7.6k, 73 NPC "instance" subclasses |
| `network` | 94,055 | 280 client packets (31k), 389 server packets (28k), `NpcStringId` 18k + `SystemMessageId` 10k lines (generated-style enums) |
| `mechanics` | 58,064 | skills, stats/formulas, conditions (88 classes), olympiad, siege, seven signs, events/listeners |
| `managers` | 22,961 | 53 singleton managers |
| `data` | 20,992 | 49 XML loaders + 10 SQL loaders |
| `ai` | 8,824 | `AttackableAI` 2.8k, `CreatureAI` 1.9k |
| `util` | 9,027 | `DocumentSkill`/`DocumentBase` = skill/item XML parsing |
| `config` | 6,814 | 61 config classes (static fields loaded from `.ini`) |
| `geoengine` | 3,344 | small, self-contained |
| `taskmanagers` | 3,215 | 20 periodic tickers (movement, AI think, decay, respawn...) |

### Scripts by folder (lines)

| Folder | Files | Lines |
|---|---:|---:|
| `quests` | 543 | 158,760 |
| `ai` | 239 | 58,150 |
| `handlers` (chat/admin, skill effects, bypass, items, actions) | 419 | 56,083 |
| `instances` | 31 | 13,418 |
| `custom`, `events`, `village_master`, `conquerablehalls`, `vehicles`, `features` | 59 | 19,824 |

## Main findings

1. **The login server is small and self-contained.** About 7.4k lines, 4 DB tables, 6 client packets, 11 GS packets. It's the right first milestone, and the docs here are detailed enough to serve as its spec.
2. **Your config runs the game server with `PacketEncryption = False`.** The server tells the client to turn off game-traffic encryption. Game traffic is then plaintext with simple framing, which makes the packet tool and the reference recordings easy for the game server. Login traffic is always Blowfish + RSA.
3. **Blowfish here is the little-endian variant.** L2 reads 32-bit words little-endian. A standard big-endian Blowfish gives wrong output. This is a classic trap. Rust's `blowfish` crate has `BlowfishLE`.
4. **The Java core is "everything shared, everything locked".** Each incoming packet runs on a pool thread, so two packets from the same client can run at the same time. 20 task managers and ~540 `ThreadPool.schedule` calls (core + scripts) touch shared objects. There are 288 `synchronized` blocks and ~500 concurrent collections. Objects point at each other directly (target, summon, party, clan, AI). **This is the borrow-checker trap from your earlier attempts.** A one-to-one translation will fail. The Rust core needs its own ownership model (see doc 04).
5. **Scripts depend on a fairly small API.** 581 distinct core classes are imported, but `Player`, `Npc`, `Quest`/`QuestState`, `Creature` and `Skill` dominate. 962 `startQuestTimer` calls show that timers are part of the script API. Designing that API is what makes the 300k lines of scripts portable by cheap agents later.
6. **Large generated-style files** (`NpcStringId`, `SystemMessageId`, the packet enums) should be converted by a code generator, not by agents.
7. **The two geodata sets differ a lot.** The client-converted set has far more multi-layer cells. See doc 05.

## Reference paths (absolute, used by every agent)

- Java core: `C:\l2-rust\l2jmobius\java\org\l2jmobius\`
- Datapack/config/scripts: `C:\l2-rust\l2jmobius\dist\`
- SQL schema: `C:\l2-rust\l2jmobius\dist\db_installer\sql\{login,game}\`
- Geodata: `C:\l2-rust\geodata-download\`, `C:\l2-rust\geodata-converted\`

These folders are git-ignored and must be treated as read-only.
