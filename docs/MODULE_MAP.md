# Module map: crates and their Java sources

Java paths are relative to `C:\l2-rust\l2jmobius\java\org\l2jmobius\` unless they start with `dist/` (= `C:\l2-rust\l2jmobius\dist\`).
Crates live in `crates/<name>/`. Binaries are marked **(bin)**. Milestone = when the crate is first created.

## Layers (dependencies point down only, no cycles)

```
 bins:      login-server   game-server   l2-supervisor   l2-proxy   l2-admin-gui   l2-geotool   codegen
              │               │              │              │            │             │
 logic:       │          l2-scripts → l2-world             │            │             │
              │               │          │                  │            │             │
 domain:  l2-protocol-login  l2-protocol-game  l2-data  l2-geo  l2-db  l2-admin-api   │
              │               │          │        │       │      │                    │
 base:      l2-net        l2-crypto    l2-config      l2-util                         │
 test:      l2-testkit (dev-dependency only)                                     (codegen writes into crates)
```
Rules:
- `l2-world` does **not** use tokio, sqlx or any socket type (it depends on `l2-net` for the `Encode`/`Decode` packet traits, on `l2-protocol-game` for the packet structs it sends, and on `l2-geo` (domain, no I/O after loading) for the shared `GeoEngine` and `PathResult`, M3-GEO-03; the datapack and the config reach it through the `StaticData` trait and `WorldSettings`, M2-ENTER-01). It receives commands and emits events/packets through plain channels and traits. That keeps it testable and synchronous (D-10, D-15).
- Protocol crates hold packet **structs + encode/decode only**, no game logic. They're shared by the servers and the proxy/GUI.
- Only the binaries choose runtimes, ports and file paths.

## Crates

| Crate | M | Purpose | Java sources |
|---|---|---|---|
| `l2-util` | 1 | ObjectId/newtypes, seedable RNG (D-13), time helpers, common errors | `commons/util/Rnd`, `commons/time/*`, `commons/util/StringUtil` |
| `l2-crypto` | 1 | Blowfish-LE, NewCrypt (checksum, XOR pass), scrambled RSA modulus, raw RSA, login encryption, game XOR cipher, BF key gen | `commons/crypt/*`, `loginserver/network/LoginEncryption`, `loginserver/controller/ScrambledKeyPair`, `gameserver/network/Encryption`, `gameserver/network/BlowFishKeygen` |
| `l2-net` | 1 | framing codec (u16 LE length), packet reader/writer (LE ints, UTF-16LE strings), connection task, packet size/drop limits | `commons/network/**` |
| `l2-config` | 1 | `.ini` reader with ConfigReader semantics; generated config structs (login M1, game M2) | `commons/util/ConfigReader`, `commons/config/*`, `loginserver/config/*`, `gameserver/config/**`, `dist/*/config/*.ini` |
| `l2-db` | 1 | sqlx pool from `Database.ini`; repositories per table group (login first, game tables later); identical schema (D-21) | `commons/database/*`, SQL in `loginserver/controller/LoginController`, `loginserver/data/GameServerTable`, later all SQL users; schema `dist/db_installer/sql/**` |
| `l2-protocol-login` | 1 | client↔LS packets, LS↔GS packets, enums (LoginFailReason, ...) | `loginserver/network/**`, `loginserver/enums/*`, `gameserver/network/loginserverpackets/**` |
| `l2-admin-api` | 1 | admin protocol types (WebSocket + JSON): status, logs, commands, capture stream; auth | new (replaces the Swing `ui/` packages) |
| `l2-testkit` | 1 | loading Java vectors and JSONL captures, test helpers, fake clients | new |
| `login-server` (bin) | 1 | the login server | `loginserver/**` minus `ui/` |
| `l2-supervisor` (bin) | 1 | starts/stops/restarts servers (exit code 2 = restart), collects logs, serves the admin API to the GUI | Java `*.sh/*.vbs` start scripts, `LoginServerTask.sh` |
| `l2-proxy` (lib+bin) | 1 | decrypting MITM proxy, JSONL captures (D-34); used by the GUI | new (Java `PacketLogger` is a small reference) |
| `l2-admin-gui` (bin) | 1 | egui app (D-31..D-33) | new (`loginserver/ui`, `gameserver/ui` as feature references) |
| `l2-geotool` (bin) | 1 | geodata stats/diff/render, region override checks | `tools/analysis/*.py` (prototype) |
| `l2-geo` | 1 / 3 | M1: `.l2j` file parsing + region model (needed by `l2-geotool`). M3: per-region override (D-26), height/NSWE queries, LOS, move checks, pathfinding | `gameserver/geoengine/**` |
| `l2-protocol-game` | 2 | game client/server packet structs, opcode tables (generated), SystemMessageId/NpcStringId (generated) | `gameserver/network/{clientpackets,serverpackets,enums,holders}/**`, `ClientPackets`, `ExClientPackets`, `ServerPackets`, `SystemMessageId`, `NpcStringId` |
| `l2-data` | 2 | immutable datapack: XML loaders (serde/quick-xml, structs from the XSDs), templates, HTML cache | `gameserver/data/**`, `gameserver/util/Document*`, `gameserver/cache/*`, `dist/game/data/**` |
| `l2-world` | 2 | world loop, arenas, regions, timer wheel, entities, AI, combat, skills, items, managers; split into more crates when it grows (e.g. `l2-mechanics`) | `gameserver/entity/**`, `ai/**`, `mechanics/**`, `managers/**`, `taskmanagers/**`, `handler/**` |
| `l2-scripts` | 7 | Script trait implementations; may split into `l2-scripts-{quests,ai,handlers,instances}` for compile times | `dist/game/data/scripts/**`, API side: `gameserver/mechanics/script/*`, `mechanics/events/**` |
| `game-server` (bin) | 2 | network ↔ world glue, LS link client, persistence task, admin endpoint | `gameserver/GameServer`, `LoginServerThread`, `network/GameClient`, `GamePacketHandler`, `Shutdown` |
| `codegen` (bin, dev) | 2 | generators (D-23); writes into `crates/*/src/generated/`, never edited by hand | input: Java sources, `.ini` files, XSDs |

## Non-crate folders

| Folder | Purpose |
|---|---|
| `java-vectors/` | Java harness (D-41): compiled against `C:\l2-rust\l2jmobius` (classpath), writes `java-vectors/out/*.json`. The vectors are committed; tests read them through `l2-testkit`. |
| `fixtures/captures/` | Selected JSONL captures used as tests (personal data scrubbed). |
| `tools/analysis/` | Phase 0 Python scripts. |
| `docs/` | Plan, decisions, analysis, task cards. |
| `.claude/agents/` | Agent definitions. |

## Generated code rules
- Lives in `src/generated/`, starts with a `// @generated by codegen — do not edit` header, and is reproducible with `cargo run -p codegen -- <target>`.
- `cargo run -p codegen -- all --check` compares the generator output in memory with the committed files. It needs the Java source, so it runs locally and **skips with a notice in CI** (no Java checkout there). Run it before committing generator changes.
- Generated modules are declared `#[rustfmt::skip] pub mod x;` (prettyplease and rustfmt disagree on layout). Register new modules in `GAME_GENERATED_MODS` in `crates/codegen/src/targets/mod.rs`.
