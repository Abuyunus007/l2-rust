# Task board

Status values: `planned`, `ready`, `in-progress`, `review`, `fix-needed`, `blocked`, `merged`.
The orchestrator keeps this file current. It's the single source of truth for progress.

## M0: Planning ✅

| Card | Title | Model | Status | Branch / notes |
|---|---|---|---|---|
| M0-01 | Git repo, `.gitignore`, `.gitattributes` | opus | merged | |
| M0-02 | Codebase analysis (`docs/analysis/`) | opus | merged | |
| M0-03 | Decisions (`docs/DECISIONS.md`) | opus | merged | |
| M0-04 | PLAN, MODULE_MAP, CLAUDE.md, agents, task workflow | opus | merged | signed off by owner |
| M0-05 | Public GitHub repo + CI skeleton | opus | merged | `Abuyunus007/l2-rust`; commits use the GitHub no-reply email |

## M1: Foundations, Rust login server, packet inspector ✅ (closed 2026-10-18)

34 cards: 5 opus (orchestrator), 21 sonnet, 8 haiku. M1 is mostly *first-of-its-kind* work (patterns others copy), so Sonnet carries most of it. The Haiku share rises from M2 on, once these reference patterns exist.

### Waves
A card can start once everything it depends on is merged. Cards in the same wave run in parallel worktrees. **o** = opus (I do it), **s** = sonnet, **h** = haiku.

| Wave | Cards | Parallel workers |
|---|---|---:|
| W0 | WS-01 (o) | 0 (me) |
| W1 | NET-01 (o), ADM-01 (o), JV-01 (s), CFG-01 (s), GEN-01 (s) | 3 |
| W2 | NET-02 (o), JV-02 (s), JV-03 (h), CRYPTO-01 (s), CFG-02 (s), ADM-02 (s), GUI-01 (s) | 6 |
| W3 | CRYPTO-02 (s), CRYPTO-03 (s), CRYPTO-04 (h), CRYPTO-05 (h), DB-01 (s), LPROTO-01 (s), GEO-01 (s), GUI-02 (h), GUI-03 (h) | 9 |
| W4 | LS-01 (s), LPROTO-02 (h), GEO-02 (h) | 3 |
| W5 | LS-02 (o), LS-04 (s), PROXY-01 (s) | 2 |
| 🎮 **CP1a** ✅ | **First client test (you):** Rust LS against your local DB (`l2jmobiush5`). Real client logs in, the password is checked, server list shown (no GS yet). Right after LS-02. | |
| 🎮 **CP1** | **Client checkpoint 1 (you):** Java LS + Java GS through the proxy CLI. Real client logs in, the capture shows decoded login packets. | |
| W6 | LS-03 (s), PROXY-02 (s) | 2 |
| W7 | LS-05 (s), GUI-04 (s) | 2 |
| 🎮 **CP2** ✅ | **Client checkpoint 2 (you):** **Rust LS** + Java GS + real client: login, server list, enter the game. | |
| W8 | LS-06 (s), ADM-03 (h) | 2 |
| 🎮 **CP3** | **Milestone test (you):** full M1 runbook from LS-06 with the GUI, packet inspector and supervisor. | |

Every milestone from M2 on also gets client checkpoints. The rule: you can see progress in the real client at least every 2–3 waves.

Critical path: WS-01 → CFG-01 → CFG-02 → DB-01 → LS-01 → LS-04 → LS-05 → LS-06.

### Cards

| Card | Title | Model | Review | Depends on | Status |
|---|---|---|---|---|---|
| [M1-WS-01](M1/M1-WS-01.md) | Cargo workspace skeleton | opus | full | none | merged (`a96b3c0`, CI green; reviewed by the orchestrator as author) |
| [M1-JV-01](M1/M1-JV-01.md) | Java vector harness + crypto vectors | sonnet | full | WS-01 | merged |
| [M1-JV-02](M1/M1-JV-02.md) | Login packet vectors | sonnet | spot | JV-01 | merged |
| [M1-JV-03](M1/M1-JV-03.md) | Geodata vectors | haiku | spot | JV-01 | merged |
| [M1-GEN-01](M1/M1-GEN-01.md) | codegen skeleton + game opcode tables | sonnet | full | WS-01 | merged |
| [M1-CRYPTO-01](M1/M1-CRYPTO-01.md) | Blowfish-LE and NewCrypt | sonnet | full | WS-01, JV-01 | merged |
| [M1-CRYPTO-02](M1/M1-CRYPTO-02.md) | RSA (no padding) and scrambled modulus | sonnet | full | CRYPTO-01 | merged |
| [M1-CRYPTO-03](M1/M1-CRYPTO-03.md) | Login client encryption | sonnet | full | CRYPTO-01 | merged |
| [M1-CRYPTO-04](M1/M1-CRYPTO-04.md) | Game XOR cipher and BF key generator | haiku | spot | CRYPTO-01, JV-01 | merged |
| [M1-CRYPTO-05](M1/M1-CRYPTO-05.md) | LS↔GS link cipher | haiku | spot | CRYPTO-01 | merged |
| [M1-NET-01](M1/M1-NET-01.md) | Packet reader/writer and packet traits | opus | full | WS-01 | merged (`2d289a5`) |
| [M1-NET-02](M1/M1-NET-02.md) | Framing codec and connection task | opus | full | NET-01 | merged |
| [M1-CFG-01](M1/M1-CFG-01.md) | `.ini` reader with ConfigReader semantics | sonnet | full | WS-01 | merged |
| [M1-CFG-02](M1/M1-CFG-02.md) | Login server config structs | sonnet | full | CFG-01 | merged |
| [M1-DB-01](M1/M1-DB-01.md) | Database pool and login repository | sonnet | full | CFG-02 | merged |
| [M1-LPROTO-01](M1/M1-LPROTO-01.md) | Client↔login server packets | sonnet | full | NET-01, JV-02 | merged |
| [M1-LPROTO-02](M1/M1-LPROTO-02.md) | Login↔game server link packets | haiku | spot | LPROTO-01 | merged |
| [M1-LS-01](M1/M1-LS-01.md) | Login server bootstrap | sonnet | full | CFG-02, DB-01, CRYPTO-02 | merged |
| [M1-LS-02](M1/M1-LS-02.md) | Client connection, GameGuard, authentication | opus | full | LS-01, NET-02, CRYPTO-03, LPROTO-01 | merged |
| [M1-LS-03](M1/M1-LS-03.md) | Server list, server login, session purge | sonnet | full | LS-02 | merged |
| [M1-LS-04](M1/M1-LS-04.md) | GS link: listener and registration | sonnet | full | LS-01, CRYPTO-05, LPROTO-02, NET-02 | merged |
| [M1-LS-05](M1/M1-LS-05.md) | GS link: authed packet handlers | sonnet | full | LS-04, LS-03 | merged |
| [M1-LS-06](M1/M1-LS-06.md) | End-to-end test kit + Java compatibility runbook | sonnet | full | LS-05, PROXY-01 | merged |
| [M1-ADM-01](M1/M1-ADM-01.md) | Admin API protocol | opus | full | WS-01 | merged (`ea43164`) |
| [M1-ADM-02](M1/M1-ADM-02.md) | Supervisor | sonnet | full | ADM-01 | merged |
| [M1-ADM-03](M1/M1-ADM-03.md) | Login server admin endpoint | haiku | spot | ADM-01, LS-05 | merged |
| [M1-PROXY-01](M1/M1-PROXY-01.md) | Login MITM proxy and capture format | sonnet | full | LPROTO-02, CRYPTO-02, CRYPTO-03, NET-02 | merged |
| [M1-PROXY-02](M1/M1-PROXY-02.md) | Game traffic pass-through with opcode names | sonnet | full | PROXY-01, GEN-01, CRYPTO-04 | merged |
| [M1-GUI-01](M1/M1-GUI-01.md) | Admin GUI shell | sonnet | full | ADM-01 | merged |
| [M1-GUI-02](M1/M1-GUI-02.md) | Servers view | haiku | spot | GUI-01, ADM-02 | merged |
| [M1-GUI-03](M1/M1-GUI-03.md) | Logs view | haiku | spot | GUI-01, ADM-02 | merged |
| [M1-GUI-04](M1/M1-GUI-04.md) | Packet inspector | sonnet | full | GUI-01, PROXY-02 | merged |
| [M1-GEO-01](M1/M1-GEO-01.md) | Geodata parser and region model | sonnet | full | WS-01, JV-03 | merged |
| M1-ADM-04 | Graceful stop via admin `shutdown` request (D-38), done together with ADM-03 | opus | full | ADM-02, ADM-03 | merged |
| [M1-GEO-02](M1/M1-GEO-02.md) | Geodata tool (stats, diff, render) | haiku | spot | GEO-01 | merged |

## M2: Game server skeleton (log in, create a character, enter the world) ✅ (closed 2026-10-19)

27 cards: 4 opus (orchestrator), 14 sonnet, 9 haiku. Design: [docs/design/world-loop.md](../design/world-loop.md). Side by side with the Java GS on the same DB (D-39).

### Waves
| Wave | Cards | Parallel workers |
|---|---|---:|
| 🎮 **CP-M2-0** | **You, starting now:** play normally on the Java GS (sessions are recorded automatically). Please also do once: create a character, delete it, enter the world, chat, log out, use *Restart* (back to character select). | |
| W0 | WORLD-01 (o), GEN-01 (s), GEN-02 (s), DATA-01 (s), GPROTO-01 (s), DB-01 (s), CAP-01 (o) | 5 |
| W1 | CFG-01 (h), DATA-02 (h), DATA-03 (h), DATA-04 (s), GPROTO-02 (s), GPROTO-03 (h), GPROTO-04 (h), DB-02 (h), IDF-01 (s) | 9 |
| W2 | GS-01 (s), PERSIST-01 (s), PROXY-01 (h) | 3 |
| W3 | GS-02 (s), ADM-01 (h) | 2 |
| W4 | GS-03 (o), GUI-01 (h) | 1 |
| 🎮 **CP-M2a** | **You:** the Rust GS appears in the server list **next to** the Java GS; the character screen of the Rust GS shows your existing characters (same DB). | ✅ passed 2026-10-18 |
| W5 | CHAR-01 (s) | 1 |
| 🎮 **CP-M2b** | **You:** create and delete a character on the Rust GS. | ✅ passed 2026-10-18 |
| W6 | ENTER-01 (s) | 1 |
| 🎮 **CP-M2c** | **You:** enter the world on the Rust GS: see yourself and your gear, chat, log out, log in again (saved). | ✅ passed 2026-10-18 (enter + restart by owner; chat/logout via tests + E2E-01) |
| W7 | E2E-01 (s) | 1 |
| W8 | RUN-01 (o) | 0 |
| 🎮 **CP-M2d** | **Milestone test (you):** cross-check — a character created on the Rust GS plays on the Java GS and vice versa. | ✅ passed 2026-10-19 |

Critical path: DB-01 → IDF-01 → GS-01 → GS-02 → GS-03 → CHAR-01 → ENTER-01 → E2E-01 → RUN-01.

### Cards
| Card | Title | Model | Review | Depends on | Status |
|---|---|---|---|---|---|
| [M2-WORLD-01](M2/M2-WORLD-01.md) | World loop skeleton | opus (orchestrator) | full | none | merged |
| [M2-GEN-01](M2/M2-GEN-01.md) | SystemMessageId and NpcStringId generator | sonnet | spot | none | merged |
| [M2-GEN-02](M2/M2-GEN-02.md) | Game config generator | sonnet | full | none | merged |
| [M2-CFG-01](M2/M2-CFG-01.md) | Hand-ported game config fields needed by M2 | haiku | spot | GEN-02 | merged |
| [M2-DATA-01](M2/M2-DATA-01.md) | l2-data crate + player templates (reference loader) | sonnet | full | none | merged |
| [M2-DATA-02](M2/M2-DATA-02.md) | Experience table, initial equipment, initial shortcuts | haiku | spot | DATA-01 | merged |
| [M2-DATA-03](M2/M2-DATA-03.md) | Skill trees (ids and levels only) | haiku | spot | DATA-01 | merged |
| [M2-DATA-04](M2/M2-DATA-04.md) | Item templates (basic fields) | sonnet | full | DATA-01 | merged |
| [M2-GPROTO-01](M2/M2-GPROTO-01.md) | Game packet pattern + handshake and character screen packets | sonnet | full | none | merged |
| [M2-GPROTO-02](M2/M2-GPROTO-02.md) | UserInfo and ItemList | sonnet | full | GPROTO-01 | merged |
| [M2-GPROTO-03](M2/M2-GPROTO-03.md) | Enter-world small packets and SystemMessage | haiku | spot | GPROTO-01, GEN-01 | merged |
| [M2-GPROTO-04](M2/M2-GPROTO-04.md) | Chat, logout and restart packets | haiku | spot | GPROTO-01 | merged |
| [M2-DB-01](M2/M2-DB-01.md) | Game repository: characters | sonnet | full | none | merged |
| [M2-DB-02](M2/M2-DB-02.md) | Game repository: items, skills, shortcuts, subclasses | haiku | spot | DB-01 | merged |
| [M2-IDF-01](M2/M2-IDF-01.md) | IdFactory (object ids) with a separate Rust range | sonnet | full | DB-01 | merged |
| [M2-GS-01](M2/M2-GS-01.md) | Game server bootstrap | sonnet | full | GEN-02, DB-01, IDF-01, DATA-01, WORLD-01 | merged |
| [M2-GS-02](M2/M2-GS-02.md) | Login server link (GS side) | sonnet | full | GS-01 | merged |
| [M2-GS-03](M2/M2-GS-03.md) | Client session layer | opus (orchestrator) | full | GS-02, GPROTO-01, DB-02 | merged |
| [M2-CHAR-01](M2/M2-CHAR-01.md) | Character screen logic | sonnet | full | GS-03, DATA-02, DATA-03, DATA-04, DB-02, CFG-01 | merged |
| [M2-ENTER-01](M2/M2-ENTER-01.md) | Enter world, chat, logout, restart | sonnet | full | CHAR-01, GPROTO-02, GPROTO-03, GPROTO-04, WORLD-01 | merged |
| [M2-PERSIST-01](M2/M2-PERSIST-01.md) | Persistence task and autosave | sonnet | full | WORLD-01, DB-02 | merged |
| [M2-ADM-01](M2/M2-ADM-01.md) | Game server admin status + supervisor entry | haiku | spot | GS-01 | merged |
| [M2-GUI-01](M2/M2-GUI-01.md) | Online players view | haiku | spot | ADM-01 | merged |
| [M2-PROXY-01](M2/M2-PROXY-01.md) | Field decoding of game packets in proxy and inspector | haiku | spot | GPROTO-01, GPROTO-02, GPROTO-03, GPROTO-04 | merged |
| [M2-CAP-01](M2/M2-CAP-01.md) | Reference captures from the owner's Java GS | opus (orchestrator) + owner | full | none | merged |
| [M2-E2E-01](M2/M2-E2E-01.md) | Fake game client and end-to-end tests | sonnet | full | ENTER-01, PERSIST-01 | merged |
| [M2-RUN-01](M2/M2-RUN-01.md) | Side-by-side run setup, runbook, cross-compat check | opus (orchestrator) + owner | full | E2E-01, ADM-01 | merged |

## M3: World and movement (approved 2026-10-18)

21 cards: 3 opus (orchestrator), 10 sonnet, 8 haiku. Design: [docs/design/m3-world.md](../design/m3-world.md). GEO-01 started early on the owner's go (2026-10-18). Workers run **at most 4-5 at a time** (CPU: every worktree compiles the workspace); the next card starts when one merges.

### Waves
| Wave | Cards | Parallel workers |
|---|---|---:|
| W0 | GEO-01 (s, running), GPROTO-05 (s), GPROTO-06 (h), DATA-05 (s), DATA-06 (h), DATA-07 (s), DATA-08 (h), DATA-09 (h) | 4-5 at a time |
| W1 | GEO-02 (s), GUI-02 (h), GUI-03 (h), WREG-01 (o, after M2-ENTER-01) | 3 |
| W2 | MOVE-01 (s), NPC-01 (s), TELE-01 (s), DOOR-01 (s) | 4 |
| 🎮 **CP-M3a** | **You:** walk around on Kain with geodata (no walking through walls, no falling), see NPCs where Java has them, doors. Plus the geodata check: compare both sets at a few spots with `/loc` (12_24, 17_10, 16_10, 14_23) so we pick the default set (D-26). | |
| W3 | TALK-01 (s), PLAYERS-01 (o) | 1 |
| 🎮 **CP-M3b** | **You:** talk to gatekeepers and teleport; two clients see each other moving and chatting. | |
| W4 | ADMIN-01 (h), E2E-01 (s) | 2 |
| W5 | RUN-01 (o) | 0 |
| 🎮 **CP-M3c** | **Milestone test (you):** walk the world with geodata, NPCs, a second player, gatekeepers, admin commands (`//spawn`, `//teleport`, `//kill`, `//heal`; `/loc` for the position). Steps: [docs/runbooks/m3-world-test.md](../runbooks/m3-world-test.md). | |

### Cards
| Card | Title | Model | Review | Depends on | Status |
|---|---|---|---|---|---|
| [M3-GEO-01](M3/M3-GEO-01.md) | Geoengine: height, line of sight, movement checks, per-region override | sonnet | full | M1 l2-geo | merged |
| [M3-GEO-02](M3/M3-GEO-02.md) | Pathfinding on worker threads | sonnet | full | GEO-01 | merged |
| [M3-GPROTO-05](M3/M3-GPROTO-05.md) | Visibility and movement packets | sonnet | full | none | merged |
| [M3-GPROTO-06](M3/M3-GPROTO-06.md) | Target, action and NPC dialog packets | haiku | spot | none | merged |
| [M3-DATA-05](M3/M3-DATA-05.md) | NPC templates | sonnet | full | none | merged |
| [M3-DATA-06](M3/M3-DATA-06.md) | Spawn data | haiku | spot | none | merged |
| [M3-DATA-07](M3/M3-DATA-07.md) | Zones and map regions | sonnet | full | none | merged |
| [M3-DATA-08](M3/M3-DATA-08.md) | Html cache and teleporter lists | haiku | spot | none | merged |
| [M3-DATA-09](M3/M3-DATA-09.md) | Doors and fences data | haiku | spot | none | merged (inside DOOR-01) |
| [M3-GUI-02](M3/M3-GUI-02.md) | GUI data browser (NPCs, items) | haiku | spot | DATA-05 | merged |
| [M3-GUI-03](M3/M3-GUI-03.md) | GUI geodata viewer | haiku | spot | GEO-01 | merged |
| [M3-WREG-01](M3/M3-WREG-01.md) | World regions, visibility, object tables | opus (orchestrator) | full | M2-ENTER-01, GPROTO-05 | merged |
| [M3-MOVE-01](M3/M3-MOVE-01.md) | Movement with geodata and pathfinding | sonnet | full | WREG-01, GEO-02, GPROTO-05 | merged |
| [M3-NPC-01](M3/M3-NPC-01.md) | NPC spawns in the world (no AI) | sonnet | full | WREG-01, DATA-05, DATA-06, GPROTO-05 | merged |
| [M3-TELE-01](M3/M3-TELE-01.md) | Teleports, zones in the world, map regions | sonnet | full | WREG-01, DATA-07, GPROTO-05 | merged |
| [M3-DOOR-01](M3/M3-DOOR-01.md) | Doors and fences in the world | sonnet | full | WREG-01, DATA-09, GEO-01, GPROTO-06 | merged |
| [M3-TALK-01](M3/M3-TALK-01.md) | Targeting, NPC dialogs, bypasses, gatekeepers | sonnet | full | NPC-01, DATA-08, TELE-01, GPROTO-06 | merged |
| [M3-PLAYERS-01](M3/M3-PLAYERS-01.md) | Other players visible (two clients) | opus (orchestrator) | full | MOVE-01 | merged |
| [M3-ADMIN-01](M3/M3-ADMIN-01.md) | Core admin commands | haiku | spot | TALK-01 | merged |
| [M3-E2E-01](M3/M3-E2E-01.md) | Two fake clients end to end | sonnet | spot | PLAYERS-01, TALK-01, M2-E2E-01 | merged |
| [M3-GEO-03](M3/M3-GEO-03.md) | Geoengine and path pool in the game server | sonnet | full | GEO-01, GEO-02 | merged |
| [M3-CFG-02](M3/M3-CFG-02.md) | Datapack options from the game config | haiku | spot | DATA-05, DATA-06, DATA-08 | merged |
| [M3-PROXY-02](M3/M3-PROXY-02.md) | Proxy field decoders for the M2/M3 server packets | haiku | spot | GPROTO-05, GPROTO-06, M2-ENTER-01 | merged |
| [M3-FIX-01](M3/M3-FIX-01.md) | Name templates (regex) and the html encoding check flag | haiku | spot | M2-CHAR-01, M3-CFG-02 | merged |
| [M3-RUN-01](M3/M3-RUN-01.md) | M3 runbook and geodata decision | opus (orchestrator) | full | E2E-01 | runbook done; geodata decision at CP-M3a |

## M4: Combat, NPC AI, progression (approved 2026-10-19)

17 cards (draft: [M4/DRAFT.md](M4/DRAFT.md)). Design: [docs/design/m4-combat.md](../design/m4-combat.md). Owner decisions: same `Rates.ini` on both servers; ground items follow `SaveDroppedItem` of `General.ini` like Java (default off); stats first. Workers 4-5 at a time.

### Waves
| Wave | Cards | Parallel workers |
|---|---|---:|
| W0 | DATA-10 (s), DATA-12 (h), JV-01 (s), GPROTO-07 (h) | 4 |
| W1 | DATA-11 (s), STAT-01 (o, after DATA-10/11/12 + JV-01) | 2 |
| 🎮 **CP-M4a** | **You:** equip/unequip gear on Kain; P.Atk/P.Def/M.Def match Sieghardt for the same character. | |
| W2 | INV-01 (s), REGEN-01 (h), COMBAT-01 (o) | 3 |
| W3 | AI-01 (s), DEATH-01 (s), AI-02 (h) | 3 |
| 🎮 **CP-M4b** | **You:** hunt a monster: it fights back, dies, gives exp/SP, drops items you pick up. | |
| W4 | EXP-01 (s), DROP-01 (s), ADMIN-02 (h) | 3 |
| 🎮 **CP-M4c** | **You:** die, go to town, monsters respawn, HP/MP regenerate. | |
| W5 | E2E-01 (s), RUN-01 (o) | 1 |
| 🎮 **CP-M4d** | **Milestone (you):** hunt on Kain, then play the character on Sieghardt with the loot and levels. | |

### Cards
| Card | Title | Model | Review | Depends on | Status |
|---|---|---|---|---|---|
| [M4-DATA-10](M4/M4-DATA-10.md) | Stat enum, function templates, conditions, item stat data | sonnet | full | M2-DATA-04 | merged |
| [M4-DATA-12](M4/M4-DATA-12.md) | Armor sets, enchant HP bonus, hit condition bonus, hennas | haiku | spot | none | merged |
| [M4-JV-01](M4/M4-JV-01.md) | Java vectors for the stat engine and Formulas | sonnet | full | none | merged |
| [M4-GPROTO-07](M4/M4-GPROTO-07.md) | Combat, death and item packets | haiku | spot | none | merged |
| [M4-DATA-11](M4/M4-DATA-11.md) | Skill data (templates per level, effect funcs, conditions) | sonnet | full | DATA-10 | merged |
| [M4-STAT-01](M4/M4-STAT-01.md) | Stat engine for players and NPCs | opus | full | DATA-10, DATA-11, DATA-12, JV-01 | merged |
| [M4-FORM-01](M4/M4-FORM-01.md) | Combat and regen formulas as pure functions (split out of COMBAT-01) | sonnet | full | JV-01 | merged |
| [M4-HENNA-01](M4/M4-HENNA-01.md) | Load the character's dyes and show them | haiku | spot | STAT-01, DATA-12 | merged |
| [M4-INV-01](M4/M4-INV-01.md) | Inventory and equipping | sonnet | full | STAT-01, GPROTO-07 | merged |
| [M4-REGEN-01](M4/M4-REGEN-01.md) | HP/MP/CP regeneration | haiku | spot | STAT-01 | merged |
| [M4-COMBAT-01](M4/M4-COMBAT-01.md) | Auto-attack (players and NPCs) | opus | full | STAT-01, GPROTO-07 | merged |
| [M4-AI-01](M4/M4-AI-01.md) | NPC AI: attackable core | sonnet | full | COMBAT-01 | merged |
| [M4-AI-02](M4/M4-AI-02.md) | NPC AI: idle behaviour | haiku | spot | AI-01 | merged |
| [M4-DEATH-01](M4/M4-DEATH-01.md) | Death, respawn, corpses | sonnet | full | COMBAT-01 | merged |
| [M4-EXP-01](M4/M4-EXP-01.md) | Exp/SP rewards and levelling | sonnet | full | DEATH-01 | merged |
| [M4-VIT-01](M4/M4-VIT-01.md) | Vitality and Nevit (Java gives 1.5x-3x kill exp with the shipped config) | sonnet | full | EXP-01 | merged |
| [M4-DROP-01](M4/M4-DROP-01.md) | Drops, items on the ground, pickup | sonnet | full | DEATH-01, INV-01 | merged |
| [M4-JV-02](M4/M4-JV-02.md) | Java vectors for drop chances | sonnet | full | JV-01, DROP-01 | merged |
| [M4-ADMIN-02](M4/M4-ADMIN-02.md) | Admin commands for combat testing | haiku | spot | DEATH-01 | merged |
| [M4-ADMIN-03](M4/M4-ADMIN-03.md) | Item, level and exp admin commands (rest of ADMIN-02) | haiku | spot | ADMIN-02, EXP-01, DROP-01 | merged |
| [M4-E2E-01](M4/M4-E2E-01.md) | End to end: hunt, loot, level, die, respawn | sonnet | spot | EXP-01, DROP-01 | merged |
| [M4-RUN-01](M4/M4-RUN-01.md) | Runbook and milestone checks | opus | full | E2E-01 | runbook done; CP-M4a..d with the owner |

## M5: Skills and effects (approved 2026-10-20)

18 cards (draft: [M5/DRAFT.md](M5/DRAFT.md)). Design: [docs/design/m5-skills.md](../design/m5-skills.md). Owner decisions: summons/pets/cubics/transformations and parties go to M6 ("party" = just me in M5); effect engine and casting first, consumables right after. Starts while M4 finishes; workers 4-5 at a time.

### Checkpoints
| Checkpoint | You test |
|---|---|
| 🎮 **CP-M5a** | Potions, herbs, scrolls; soulshots glow and hit harder; auto soulshots. |
| 🎮 **CP-M5b** | Attack, heal and buff skills; buff icons and times match Sieghardt; buffs and reuse survive a relog and a move to Sieghardt. |
| 🎮 **CP-M5c** | Monsters use their skills; spoil and sweep; learn a skill at a trainer. |
| 🎮 **CP-M5d** | Milestone: a play session with skills and consumables on Kain, then the same character on Sieghardt. |

### Cards
| Card | Title | Model | Review | Depends on | Status |
|---|---|---|---|---|---|
| [M5-GPROTO-08](M5/M5-GPROTO-08.md) | Skill, buff and item-use packets | haiku | spot | none | merged |
| [M5-JV-03](M5/M5-JV-03.md) | Java vectors for skill formulas | sonnet | full | none | merged |
| [M5-FORM-02](M5/M5-FORM-02.md) | Skill formulas as pure functions | sonnet | full | JV-03 | merged |
| [M5-EFFECT-01](M5/M5-EFFECT-01.md) | Effect engine: effect list, buff info, stacking, durations, ticks | opus | full | GPROTO-08 (icons only) | merged |
| [M5-CAST-01](M5/M5-CAST-01.md) | Casting | opus | full | EFFECT-01, FORM-02 | merged |
| [M5-TARGET-01](M5/M5-TARGET-01.md) | The 34 target handlers | sonnet | full | none (CAST-01 calls it) | merged |
| [M5-EFX-01](M5/M5-EFX-01.md) | Effect handlers: damage and healing | sonnet | full | EFFECT-01, FORM-02 | part 1 merged (heal/over-time); rest in EFX-04 |
| [M5-EFX-02](M5/M5-EFX-02.md) | Effect handlers: buffs, debuffs, control | sonnet | full | EFFECT-01 | merged |
| [M5-EFX-03](M5/M5-EFX-03.md) | Effect handlers: dispel, hate, movement, misc | haiku | spot | EFFECT-01 | 26/44 merged; rest in EFX-04/05 |
| [M5-EFX-04](M5/M5-EFX-04.md) | Effect handlers: damage, Heal, hate chances (rest of EFX-01/03) | sonnet | full | FORM-02 | merged |
| [M5-EFX-05](M5/M5-EFX-05.md) | Effect handlers needing casting or new world parts (rest of EFX-03) | sonnet | full | CAST-01 | merged |
| [M5-ITEM-01](M5/M5-ITEM-01.md) | Item handlers: potions, herbs, scrolls, elixirs, extractables | sonnet | full | CAST-01, EFX-01 | merged |
| [M5-SHOT-01](M5/M5-SHOT-01.md) | Soulshots and spiritshots | sonnet | full | CAST-01 | merged |
| [M5-SPOIL-01](M5/M5-SPOIL-01.md) | Spoil and sweep | sonnet | full | EFX-03, FORM-02 | merged |
| [M5-AI-03](M5/M5-AI-03.md) | NPC skill use | sonnet | full | CAST-01, EFX-01, EFX-02 | merged |
| [M5-LEARN-01](M5/M5-LEARN-01.md) | Learning skills at trainers | sonnet | full | GPROTO-08 | merged |
| [M5-SAVE-01](M5/M5-SAVE-01.md) | Buffs and reuse times across relog | haiku | spot | EFFECT-01, CAST-01 | merged |
| [M5-ADMIN-04](M5/M5-ADMIN-04.md) | Admin commands for skills | haiku | spot | CAST-01 | merged |
| [M5-SHORTCUT-01](M5/M5-SHORTCUT-01.md) | Shortcut bar: register and delete (skills on the bar) | sonnet | full | CAST-01 | merged |
| [M5-FIX-03](M5/M5-FIX-03.md) | Follow-ups: item reuse save, ClassBalance skill tables, old corpses, maps | sonnet | spot | ITEM-01, EFX-04, SAVE-01 | merged |
| [M5-E2E-01](M5/M5-E2E-01.md) | End to end: cast, buff, relog, potion, soulshot | sonnet | spot | ITEM-01, SHOT-01, SAVE-01 | merged |
| [M5-RUN-01](M5/M5-RUN-01.md) | Runbook and milestone checks | opus | full | E2E-01 | runbook done; CP-M5a..d with the owner |
| [M5-FU-01](M5/M5-FU-01.md) | Admin pages, confirm dialogs, GM item list | sonnet | spot | M5 | merged |
| [M5-FU-02](M5/M5-FU-02.md) | Guards and defenders, raid minion respawn, NPC fear | sonnet | full | M5 | merged |
| [M5-FU-03](M5/M5-FU-03.md) | Daily resets and recommendations | sonnet | full | M5 | merged |
| [M5-FU-04](M5/M5-FU-04.md) | Punishments and illegal actions | sonnet | full | M5 | merged |
| [M5-FU-05](M5/M5-FU-05.md) | Item elements, augments and item rows | sonnet | full | M5 | merged |

## M6: Parties, PvP, trade, clans, companions (approved 2026-10-20)

16 cards, plus 8 economy cards added 2026-10-21 from PLAN.md's M6 scope (draft: [M6/DRAFT.md](M6/DRAFT.md)). Design: [docs/design/m6-social.md](../design/m6-social.md). Owner decisions: sieges/castles/clan halls in M7, Olympiad/Seven Signs in M8, quests after that; order inside M6: party/PvP/trade, then clans, then summons/pets/cubics/transformations.

### Checkpoints
| Checkpoint | You test |
|---|---|
| 🎮 **CP-M6a** | Two players: party with shared exp/loot and party buffs, a trade, PvP flag and karma. |
| 🎮 **CP-M6b** | Clan: create, invite, ranks, crest, clan skills; the same clan on Sieghardt. |
| 🎮 **CP-M6c** | Servitor, pet, cubics, a transformation. |
| 🎮 **CP-M6e** | Economy: buy/sell/refund at an NPC shop, multisell, warehouse and freight, enchant and attribute an item, craft, friends, a mail with an attachment. |
| 🎮 **CP-M6d** | Milestone: a party session with summons and a clan on Kain, then on Sieghardt. |

### Cards
| Card | Title | Model | Review | Depends on | Status |
|---|---|---|---|---|---|
| [M6-GPROTO-09](M6/M6-GPROTO-09.md) | Party, PvP, trade, clan and summon packets | haiku | spot | none | merged |
| [M6-PVP-01](M6/M6-PVP-01.md) | PvP flag, karma, PK, relations | sonnet | full | GPROTO-09 | merged |
| [M6-PARTY-01](M6/M6-PARTY-01.md) | Parties | sonnet | full | GPROTO-09 | merged |
| [M6-TRADE-01](M6/M6-TRADE-01.md) | Private trade | sonnet | full | GPROTO-09 | merged |
| [M6-STORE-01](M6/M6-STORE-01.md) | Private stores (sell, buy, package sell) | sonnet | full | TRADE-01 | merged |
| [M6-CLAN-01](M6/M6-CLAN-01.md) | Clan core | opus | full | GPROTO-09 | merged |
| [M6-CLAN-02](M6/M6-CLAN-02.md) | Crests, alliances, clan wars | sonnet | full | CLAN-01 | merged |
| [M6-CLAN-03](M6/M6-CLAN-03.md) | Clan skills and academy | sonnet | full | CLAN-01 | merged |
| [M6-SUMMON-01](M6/M6-SUMMON-01.md) | Summons: servitors | opus | full | GPROTO-09 | merged |
| [M6-PET-01](M6/M6-PET-01.md) | Pets | sonnet | full | SUMMON-01 | merged |
| [M6-CUBIC-01](M6/M6-CUBIC-01.md) | Cubics | sonnet | full | SUMMON-01 | merged |
| [M6-TRANS-01](M6/M6-TRANS-01.md) | Transformations | sonnet | full | none (M5) | merged |
| [M6-DUEL-01](M6/M6-DUEL-01.md) | Duels | sonnet | spot | PARTY-01, PVP-01 | merged |
| [M6-ADMIN-05](M6/M6-ADMIN-05.md) | Admin commands for social and summons | haiku | spot | CLAN-01, SUMMON-01 | merged |
| [M6-E2E-01](M6/M6-E2E-01.md) | End to end: party hunt, trade, clan, summon | sonnet | spot | PARTY-01, TRADE-01, CLAN-01, SUMMON-01, SHOP-01, WH-01 | merged |
| [M6-RUN-01](M6/M6-RUN-01.md) | Runbook and milestone checks | opus | full | E2E-01 | ready (refined when deps merge) |
| [M6-SHOP-01](M6/M6-SHOP-01.md) | NPC shops (buy, sell, refund) | sonnet | full | none | merged |
| [M6-MULTISELL-01](M6/M6-MULTISELL-01.md) | Multisell | sonnet | full | none | merged |
| [M6-WH-01](M6/M6-WH-01.md) | Warehouses and freight | sonnet | full | TRADE-01, CLAN-01 | merged |
| [M6-ENCHANT-01](M6/M6-ENCHANT-01.md) | Enchanting and attributes | sonnet | full | none | merged |
| [M6-CRAFT-01](M6/M6-CRAFT-01.md) | Crafting and manufacture shops | sonnet | full | STORE-01 | merged |
| [M6-FRIEND-01](M6/M6-FRIEND-01.md) | Friends and block list | haiku | spot | none | merged |
| [M6-MAIL-01](M6/M6-MAIL-01.md) | Mail | sonnet | full | TRADE-01 | merged |
| [M6-AUGMENT-01](M6/M6-AUGMENT-01.md) | Augmentation | sonnet | full | none | merged |
| [M6-ACTION-01](M6/M6-ACTION-01.md) | Player actions (sit, stand, walk, run, socials) | sonnet | spot | SUMMON-01, STORE-01 | merged |
| [M6-SYNC-01](M6/M6-SYNC-01.md) | Fresh shared rows for mail and the clan warehouse (D-39) | sonnet | full | MAIL-01, WH-01 | merged |
| [M6-ENCHANT-02](M6/M6-ENCHANT-02.md) | Skill enchanting, enchant options, //enchant | sonnet | full | ENCHANT-01 | merged |
| [M6-FU-06](M6/M6-FU-06.md) | Flood protectors | sonnet | full | none | merged |
| [M6-FU-07](M6/M6-FU-07.md) | Mounts: strider, wyvern, wolf | sonnet | full | none | merged |
| [M6-FU-08](M6/M6-FU-08.md) | Fishing | sonnet | full | none | merged |
| [M6-FU-09](M6/M6-FU-09.md) | M6 leftovers: offline block list, pet collar rule, offline pet decay, bot report | sonnet | full | none | merged |
| [M6-FU-10](M6/M6-FU-10.md) | Fisherman skill learning | sonnet | spot | FU-08 | merged |
| [M6-FU-11](M6/M6-FU-11.md) | Stale M6 markers: duel, summon and store checks, GM pet views, SkillCoolTime, talisman mana, PC cafe points | sonnet | full | none | merged |
| [M6-FU-12](M6/M6-FU-12.md) | Skill channelizer, weapon and option trigger skills | sonnet | full | none | merged |
| [M6-FU-13](M6/M6-FU-13.md) | Command channels | sonnet | full | none | merged |
| [M6-FU-14](M6/M6-FU-14.md) | Pet evolve and offline trade | sonnet | full | none | merged |

## M7: Castles, sieges, fortresses, clan halls (approved 2026-10-23)

12 cards (draft: [M7/DRAFT.md](M7/DRAFT.md)). Design: [docs/design/m7-residences.md](../design/m7-residences.md). Owner decisions: Sieghardt runs the real sieges while both share the DB, Kain only GM-started test sieges with `SiegeDatabaseWrites` off (D-51); Territory War last in M7; order: residences, castle sieges and NPCs, fortresses and clan halls, manor, Territory War.

### Checkpoints
| Checkpoint | You test |
|---|---|
| 🎮 **CP-M7a** | A castle owned by your clan: the chamberlain (tax rate, doors, treasury), the castle tax in a town shop, the castle warehouse and teleporter. |
| 🎮 **CP-M7b** | A GM-started castle siege on Kain: register, a flag, guards and mercenaries, break the doors, engrave, your clan owns the castle (in Kain's world; nothing global written with the switch off). |
| 🎮 **CP-M7c** | A fortress siege; a clan hall auction and its functions; one conquerable clan hall siege. |
| 🎮 **CP-M7d** | The manor and Territory War; milestone: residences and their owners as Sieghardt keeps them. |

### Cards
| Card | Title | Model | Review | Depends on | Status |
|---|---|---|---|---|---|
| [M7-GPROTO-10](M7/M7-GPROTO-10.md) | Siege, castle, fortress, manor, territory packets | haiku | spot | none | merged |
| [M7-RES-01](M7/M7-RES-01.md) | Residences core: castles, fortresses, clan halls | opus | full | none | merged |
| [M7-SIEGE-01](M7/M7-SIEGE-01.md) | Castle sieges | opus | full | RES-01, GPROTO-10 | merged |
| [M7-SIEGE-02](M7/M7-SIEGE-02.md) | Door and wall damage, siege guard AIs | opus | full | SIEGE-01, FORT-01 | merged |
| [M7-NPC-01](M7/M7-NPC-01.md) | Castle NPCs | sonnet | full | RES-01, SIEGE-01 | merged |
| [M7-FORT-01](M7/M7-FORT-01.md) | Fortresses and fortress sieges | sonnet | full | RES-01, GPROTO-10 | merged |
| [M7-CH-01](M7/M7-CH-01.md) | Clan halls and auctions | sonnet | full | RES-01 | merged |
| [M7-CHSIEGE-01](M7/M7-CHSIEGE-01.md) | Conquerable clan hall sieges | sonnet | full | CH-01, SIEGE-01 | merged |
| [M7-MANOR-01](M7/M7-MANOR-01.md) | The manor | sonnet | full | RES-01, GPROTO-10 | merged |
| [M7-TW-01](M7/M7-TW-01.md) | Territory War | opus | full | SIEGE-01 | merged |
| [M7-ADMIN-06](M7/M7-ADMIN-06.md) | Admin commands for residences | haiku | spot | SIEGE-01, FORT-01, CH-01 | merged |
| [M7-FU-01](M7/M7-FU-01.md) | Hall siege guard AI, doorman hall branch | sonnet | spot | SIEGE-02, CHSIEGE-01, NPC-01 | merged |
| [M7-FU-02](M7/M7-FU-02.md) | Fixes from the M7 end-to-end tests (Flag Display, function rows) | sonnet | full | E2E-01 | merged |
| [M7-FU-03](M7/M7-FU-03.md) | Crest ids side by side, auction list re-read, residence door dialogs, siege trap zones | sonnet | full | none | merged |
| [M7-ADMIN-08](M7/M7-ADMIN-08.md) | Admin: effects, menu, teleport, skills, char tools | haiku | spot | none | merged |
| [M7-ADMIN-09](M7/M7-ADMIN-09.md) | Admin: edit char, spawn, admin page, items, doors, shops | haiku | spot | none | merged |
| [M7-ADMIN-10](M7/M7-ADMIN-10.md) | Admin: punishments, kick, server control, announcements, petitions | sonnet | full | none | merged |
| [M7-ADMIN-11](M7/M7-ADMIN-11.md) | Admin: geodata, regions, zones, zone build, path nodes | sonnet | spot | none | merged |
| [M7-ADMIN-12](M7/M7-ADMIN-12.md) | Admin: the rest of edit char, spawn and the admin page | sonnet | full | ADMIN-09 | merged |
| [M7-E2E-01](M7/M7-E2E-01.md) | End to end: castle siege, fortress siege, clan hall auction | sonnet | spot | SIEGE-01, SIEGE-02, FORT-01, CH-01 | merged |
| [M7-RUN-01](M7/M7-RUN-01.md) | Runbook and milestone checks | opus | full | E2E-01 | merged |

## Leftovers of M0..M7 (2026-10-27)

Gaps found after M7: client packets without a handler, old markers, admin words, config keys.

| Card | Title | Model | Review | Depends on | Status |
|---|---|---|---|---|---|
| [L-01](L/L-01.md) | Hennas (dyes) and the symbol maker | sonnet | full | none | merged |
| [L-02](L/L-02.md) | Client leftovers: macros, crystallize, wear, item links, key mapping, inventory order | sonnet | full | none | merged |
| [L-03](L/L-03.md) | Party matching rooms, loot modification, nicknames, GM list | sonnet | full | none | merged |
| [L-04](L/L-04.md) | Teleport bookmarks, secondary password, contact list, game guard | sonnet | full | none | ready |
| [L-05](L/L-05.md) | Item auctions | sonnet | full | none | ready |
| [L-06](L/L-06.md) | World leftovers: falling, water, teleport rules, cast abort, toggles | sonnet | full | none | merged |
| [L-07](L/L-07.md) | Runtime geodata edits | opus | full | none | merged |
| [L-08](L/L-08.md) | Admin rest: fight calculator, spawn reload, misc room, pools | sonnet | spot | none | merged |
| [L-09](L/L-09.md) | Core config keys still unported | sonnet | spot | none | ready |
| [L-10](L/L-10.md) | Logs, new-character access level, LS host names | haiku | spot | none | merged |

## M8: Olympiad, heroes, Seven Signs, instances and the big systems (approved 2026-10-27)

Owner: "start M8"; the draft's recommendations hold (D-53: Sieghardt keeps the global cycles; Kain writes them only with `CycleDatabaseWrites`). Petitions moved to M7-ADMIN-10.

| Card | Title | Model | Review | Depends on | Status |
|---|---|---|---|---|---|
| [M8-GPROTO-11](M8/M8-GPROTO-11.md) | Olympiad, hero, Seven Signs, instance, vehicle, mini game packets | haiku | spot | none | merged |
| [M8-INST-01](M8/M8-INST-01.md) | Instances | opus | full | GPROTO-11 | ready |
| [M8-OLY-01](M8/M8-OLY-01.md) | Olympiad | opus | full | GPROTO-11, INST-01 | ready |
| [M8-HERO-01](M8/M8-HERO-01.md) | Heroes | sonnet | full | OLY-01 | ready |
| [M8-SS-01](M8/M8-SS-01.md) | Seven Signs | opus | full | GPROTO-11 | ready |
| [M8-FEST-01](M8/M8-FEST-01.md) | Festival of Darkness and Dimensional Rift | sonnet | full | SS-01, INST-01 | ready |
| [M8-BOSS-01](M8/M8-BOSS-01.md) | Grand and raid boss states, raid points | sonnet | full | none | ready |
| [M8-VEHICLE-01](M8/M8-VEHICLE-01.md) | Boats and airships | sonnet | full | GPROTO-11 | ready |
| [M8-CURSED-01](M8/M8-CURSED-01.md) | Cursed weapons | sonnet | full | none | merged |
| [M8-GAMES-01](M8/M8-GAMES-01.md) | Mini games | sonnet | full | INST-01, GPROTO-11 | ready |
| [M8-MISC-01](M8/M8-MISC-01.md) | Weddings, fishing championship, event drops, walking routes | sonnet | full | none | merged |
| [M8-ADMIN-07](M8/M8-ADMIN-07.md) | Admin commands for M8 | haiku | spot | OLY-01, SS-01, INST-01, BOSS-01, CURSED-01 | ready |
| [M8-E2E-01](M8/M8-E2E-01.md) | End to end: olympiad match, Seven Signs, instance | sonnet | full | OLY-01, SS-01, INST-01 | ready |
| [M8-RUN-01](M8/M8-RUN-01.md) | Runbook and milestone checks | opus | full | E2E-01 | ready |

## Milestone test results

### M1 (2026-10-18)
| Check | Result |
|---|---|
| CP1a: real H5 client logs in through the Rust LS against the real DB (`l2jmobiush5`) | ✅ passed |
| Java GS (`C:\L2J_Mobius_CT_2.6_HighFive`) registers with the Rust LS (hexid, host list) | ✅ passed |
| CP2: login → server list (GS up) → server login → **enter the game on the Java GS** | ✅ passed |
| Supervisor + admin GUI control the Rust LS, live logs, clean stop via admin `shutdown` | ✅ running |
| CI: Windows + Linux + MariaDB integration (612 tests) | ✅ green |
| CP1: Java LS through the proxy, capture of a Java session | ⏳ optional, not done yet (needed for the Java capture fixture in LS-06) |

Findings during the tests:
- Two old accounts had a different password than the owner remembered; the hashing was proven identical to the installed Java `LoginServer.jar` (SHA-1/Base64). Failed logins are now logged (account, reason, attempt count; never the password).
- "High traffic" on server select before LS-05 = `PlayFail(SERVER_OVERLOADED)` because `ServerStatus` wasn't handled yet.

M1 follow-ups carried into M2: record a Java capture fixture (CP1); Windows CI build time (GUI/wgpu); Java GS stays outside the supervisor (D-38).

### M2 (2026-10-19)
| Check | Result |
|---|---|
| CP-M2a: the Rust GS (Kain, id 3, port 7778) appears next to the Java GS; its character list shows the existing characters (same DB) | ✅ passed |
| CP-M2b: create, delete and restore a character on the Rust GS | ✅ passed |
| CP-M2c: enter the world on the Rust GS, gear/skills/shortcuts, restart | ✅ passed |
| CP-M2d: a Rust-created character plays on the Java GS and a Java character on the Rust GS, nothing lost | ✅ passed |
| Java fixtures: re-encoded byte for byte (charscreen, enter world, chat; scrubbed copies in `fixtures/captures/`) | ✅ in CI |
| CI: Windows + Linux + MariaDB (game-server DB tests and end-to-end tests included) | ✅ green |

Findings during the tests:
- An account row stored with NUL padding could log in but never enter a game server ("Session key incorrect"); the Rust LS now trims the name read from the DB.
- CI's DB job had not run game-server's DB tests at all; added, and it found a missing test table and a wrong assertion.
- M2-E2E-01 found a restart race (character list read before the store finished); the session now waits for the store.
- The owner's Java GS runs without geodata (`data/geodata` is empty); the Rust GS uses `C:\l2-rust\geodata-converted` for now (decision at CP-M3a).

M2 follow-ups carried into M3: `CheckHtmlEncoding` gating in the html loader; regex support for name templates (`CnameTemplate`); clan data (pledge packets) and the remaining enter-world packets (quests, friends, hennas).

### M4 (in progress)
| Check | Result |
|---|---|
| CP-M4a part 1: a geared character on Kain shows the same P.Atk, P.Def, M.Atk, M.Def, accuracy, evasion, crit, speeds and max HP/MP/CP as on Sieghardt (enchants, armor sets, passive skills) | ✅ passed (2026-10-19) |
| CP-M4a part 2: equip/unequip changes the numbers like on Sieghardt | ⏳ to confirm |
| CP-M4b (first test, 2026-10-20): fighting works, drops land and adena is picked up; herbs drop and vanish on pickup without their effect (herb skills are M5) | ✅ partly (no vitality, herbs M5) |
