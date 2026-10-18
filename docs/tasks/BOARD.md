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

## M1: Foundations, Rust login server, packet inspector

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
| 🎮 **CP1a** | **First client test (you):** Rust LS against your local DB (`l2jmobiush5`). Real client logs in, the password is checked, server list shown (no GS yet). Right after LS-02. | |
| 🎮 **CP1** | **Client checkpoint 1 (you):** Java LS + Java GS through the proxy CLI. Real client logs in, the capture shows decoded login packets. | |
| W6 | LS-03 (s), PROXY-02 (s) | 2 |
| W7 | LS-05 (s), GUI-04 (s) | 2 |
| 🎮 **CP2** | **Client checkpoint 2 (you):** **Rust LS** + Java GS + real client: login, server list, enter the game. | |
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
| [M1-LS-02](M1/M1-LS-02.md) | Client connection, GameGuard, authentication | opus | full | LS-01, NET-02, CRYPTO-03, LPROTO-01 | in-progress |
| [M1-LS-03](M1/M1-LS-03.md) | Server list, server login, session purge | sonnet | full | LS-02 | planned |
| [M1-LS-04](M1/M1-LS-04.md) | GS link: listener and registration | sonnet | full | LS-01, CRYPTO-05, LPROTO-02, NET-02 | in-progress |
| [M1-LS-05](M1/M1-LS-05.md) | GS link: authed packet handlers | sonnet | full | LS-04, LS-03 | planned |
| [M1-LS-06](M1/M1-LS-06.md) | End-to-end test kit + Java compatibility runbook | sonnet | full | LS-05, PROXY-01 | planned |
| [M1-ADM-01](M1/M1-ADM-01.md) | Admin API protocol | opus | full | WS-01 | merged (`ea43164`) |
| [M1-ADM-02](M1/M1-ADM-02.md) | Supervisor | sonnet | full | ADM-01 | merged |
| [M1-ADM-03](M1/M1-ADM-03.md) | Login server admin endpoint | haiku | spot | ADM-01, LS-05 | planned |
| [M1-PROXY-01](M1/M1-PROXY-01.md) | Login MITM proxy and capture format | sonnet | full | LPROTO-02, CRYPTO-02, CRYPTO-03, NET-02 | in-progress |
| [M1-PROXY-02](M1/M1-PROXY-02.md) | Game traffic pass-through with opcode names | sonnet | full | PROXY-01, GEN-01, CRYPTO-04 | planned |
| [M1-GUI-01](M1/M1-GUI-01.md) | Admin GUI shell | sonnet | full | ADM-01 | merged |
| [M1-GUI-02](M1/M1-GUI-02.md) | Servers view | haiku | spot | GUI-01, ADM-02 | in-progress |
| [M1-GUI-03](M1/M1-GUI-03.md) | Logs view | haiku | spot | GUI-01, ADM-02 | in-progress |
| [M1-GUI-04](M1/M1-GUI-04.md) | Packet inspector | sonnet | full | GUI-01, PROXY-02 | planned |
| [M1-GEO-01](M1/M1-GEO-01.md) | Geodata parser and region model | sonnet | full | WS-01, JV-03 | merged |
| M1-ADM-04 | Graceful stop on Windows (admin `Shutdown` request and/or console Ctrl event; needs a decision on `unsafe`/`windows-sys`) | opus | full | ADM-02 | planned (follow-up from ADM-02, not needed for CP1) |
| [M1-GEO-02](M1/M1-GEO-02.md) | Geodata tool (stats, diff, render) | haiku | spot | GEO-01 | merged |

## Milestone test results
_None yet._
