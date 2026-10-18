# Plan: L2JMobius H5 2.6 → Rust

Milestones in order. Each one ends with something **you can test in-game or in the GUI**.
Rules: `docs/DECISIONS.md`. Crates: `docs/MODULE_MAP.md`. Work items: `docs/tasks/`.

Legend for who builds what (D-40):
- **O** = Opus (design, core, review)
- **S** = Sonnet (harder ports)
- **H** = Haiku (templated work after a reference exists)
- **G** = generator (code produced by `codegen`, reviewed once)

A milestone's work is split into **task cards** only when the milestone starts. The lists below are scope, not cards.

---

## M0: Planning ✅
Analysis, decisions, plan, module map, `CLAUDE.md`, agent definitions, task card template, GitHub repo + CI.
**Done when:** you sign off on these docs.

## M1: Foundations, Rust login server, packet inspector ✅
The first Rust code. Everything here works with the **Java game server**.

| Track | Content | Who |
|---|---|---|
| Workspace | Cargo workspace, lints, `rustfmt.toml`, CI (Windows + Linux), `l2-testkit` | O |
| Java oracle | `java-vectors/`: Java programs that call L2JMobius classes (BlowfishEngine, NewCrypt, LoginEncryption, ScrambledKeyPair, Encryption, packet writers) and write JSON vectors | O → S |
| Crypto | Blowfish-LE, checksum, XOR pass, scrambled modulus, raw RSA, game XOR cipher, all matching the vectors | S |
| Net | framing codec, packet reader/writer (LE ints, UTF-16 strings), connection task, flood/drop limits | O |
| Config | `.ini` reader (ConfigReader semantics), login config structs; generator pattern for configs | O → G |
| DB | sqlx pool from `Database.ini`, login repository (accounts, account_data, accounts_ipauth, gameservers) | S |
| Login protocol | client packets + LS↔GS packets as structs (encode/decode) | O sets the pattern → H |
| Login server | client flow, GS link, server list, bans, auto-create, purge, `banned_ip.cfg`, restart exit code 2 | O + S |
| Admin | `l2-admin-api` (protocol types), `l2-supervisor` (start/stop/restart servers, collect logs), status endpoint in the LS | O + S |
| Proxy | `l2-proxy`: login MITM (own RSA/BF keys toward the client, rewrites the ServerList IP), game pass-through with opcode decoding, JSONL capture | O + S |
| GUI | `l2-admin-gui` (egui): connect to the supervisor, server control, live logs, **packet inspector** (list, filter, decoded fields, hex, save/load) | S → H |
| Geo tool | `l2-geo` file parser (format from doc 05, checked against Java vectors) + `l2-geotool` CLI: stats, diff, PNG heightmap/NSWE renders for both sets (resolves the doc 05 question) | S (parser) → H (tool) |

**Done when** (milestone test, done by you):
1. The Java GS registers with the Rust LS. You log in with the H5 client, see the server list with your character count, and play on the Java GS.
2. Wrong password, IP ban after 5 failures, account in use, and auto-create behave like Java.
3. The GUI starts and stops the Rust LS (and the Java GS), shows logs, and shows a live decoded capture of a full login + enter-world session.
4. CI is green on Windows and Linux. All Java-vector tests pass.

## M2: Game server skeleton (log in, create a character, enter the world)
| Content | Who |
|---|---|
| `codegen`: SystemMessageId, NpcStringId, client/server opcode tables | G |
| All game `.ini` config structs (61 classes, 1,576 keys) | G + H |
| Game protocol crate: packet structs for the handshake + char screen + enter world | O pattern → H |
| GS networking, ProtocolVersion/KeyPacket, optional XOR cipher, AuthLogin via LS link (Rust GS ↔ Rust LS) | O |
| Data loaders: player templates, exp table, initial equipment, items (basic) | O pattern → H |
| World loop skeleton (D-10..D-13): arenas, command channel, 50 ms tick, timer wheel, seeded RNG | **O** |
| Character list/create/delete/restore/select, persistence task, EnterWorld with a static world | S |
| GUI: online players view | H |

**Done when:** you create a character, enter the world, see yourself and your gear, chat, log out, and log back in (the character is persisted). Java GS captures of the same flow match where deterministic.

## M3: World and movement
| Content | Who |
|---|---|
| Regions, visibility, spawn/despawn packets | O |
| `l2-geo`: geoengine port (getHeight, canSeeTarget, canMoveToTarget, door/fence NSWE), Java-vector checks on both geodata sets | S |
| Pathfinding on worker threads (D-14) | S |
| Movement, ValidatePosition, teleports, map regions, zones (basic) | S |
| NPC spawns (no AI), NPC templates, HTML cache, first talk/bypass dialogs, gatekeepers | S + H |
| Core admin commands for testing (`//admin`, `//spawn`, `//teleport`, `//loc`, `//kill`, `//heal`) | H |
| Other players visible (two clients) | O |
| GUI: data browser (NPCs, items), geodata viewer | H |

**Done when:** you walk around the world with geodata, see NPCs and a second player, talk to gatekeepers and teleport.

## M4: Combat, NPC AI, progression
Stats/formulas, auto-attack, NPC AI (attackable aggro, return home, random walk), death/respawn, exp/sp/levels, drops, items on the ground, pickup, inventory/equip, HP/MP regen. **Done when:** you hunt mobs, level up, loot, die and respawn. Mostly O + S, with the formulas checked against Java vectors.

## M5: Skills and effects
Skill XML parsing (tables, conditions), effect framework + the 199 effect handlers (pattern → H in waves), buffs, casting, reuse, shortcuts, skill learning, class change, subclass basics. **Done when:** every class plays its basic rotation and buffs work.

## M6: Economy and social
Shops/buylists, multisell, warehouse, trade, private stores, party (+ exp share), clan basics, friends, mail, chat channels, enchant, crafting, augmentation. Mostly S + H.

## M7: Script API and mass porting of scripts
1. **O:** Script trait, event registry (89 event types), `Ctx` API, QuestState persistence, quest timers on the timer wheel.
2. **O + S:** 10–20 hand-ported reference scripts covering each pattern (simple quest, multi-step quest, AI, instance, bypass handler, item handler).
3. **H in waves, parallel worktrees:** 543 quests, 239 AI, 419 handlers, 31 instances, events, village masters. Each wave is gated by script tests + a GUI/in-game spot check.

## M8: Big systems
Sieges, fortresses, clan halls, territory war, olympiad, seven signs, grand bosses, instances, vehicles, festival, coliseum, fishing, manor.

## M9: Customs, hardening, switch from Java
The `Custom/*.ini` features, performance/load tests, crash recovery, backups, full GUI admin tasks, then **switch from Java to Rust** for your server.

---

## Parallelism
- Inside a milestone: tracks with no shared files run as parallel worktrees (D-42).
- Across milestones: pure-data work (config structs, data loaders, packet structs, geotool) can run ahead of the milestone that needs it once its pattern exists.
- The world loop (M2) and the script API (M7.1) are **serial Opus work**. Nothing that depends on them starts before they're reviewed.

## Risks
| Risk | Mitigation |
|---|---|
| Haiku output inconsistent | tiny cards, generator-produced scaffolding, reference implementation per pattern, mandatory tests, Opus spot checks |
| Behaviour drift from Java | Java vector harness, capture replays, in-game milestone tests |
| World loop design wrong | built and reviewed by Opus first, exercised with movement/combat before scripts depend on it |
| Context loss between sessions | all state in `docs/` + task board, `CLAUDE.md` read by every agent |
| Cost explodes | generators for bulk code, Haiku default, risk-based reviews (D-44) |
