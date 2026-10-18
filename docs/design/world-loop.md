# Design: the world loop (M2 core)

Implements D-10..D-15. Binding for every M2+ card that touches game state. Built by M2-WORLD-01 (orchestrator).

## Processes and tasks inside `game-server`

```
 tokio runtime (async, many threads)                         world thread (one std::thread)
 ┌─────────────────────────────────────────┐                 ┌──────────────────────────────────┐
 │ client listener → one task per client   │──WorldCommand──►│ World::handle(cmd)               │
 │   (pre-game: AuthLogin, char screen;    │                 │ every 50 ms: World::tick()       │
 │    may await DB and the LS link)        │◄─bytes via──────│   timer wheel, movement (M3), AI │
 │ LS link task (l2-protocol-login link)   │ ConnectionHandle│ never awaits, never touches DB   │
 │ persistence task (sqlx, ordered writes) │◄─PersistCommand─│                                  │
 │ admin endpoint                          │◄─AdminReply─────│                                  │
 └─────────────────────────────────────────┘                 └──────────────────────────────────┘
```

- **Pre-game lives in the client task**, not in the world. ProtocolVersion, AuthLogin (asks the LS link), the character screen (list/create/delete/restore/select) are request/response flows with DB access. They don't share state with other players, so they don't need the world.
- **The world starts at EnterWorld.** On `CharacterSelect` the client task loads the full character from the DB (async), then sends `WorldCommand::EnterWorld { session, player: PlayerData, out: ConnectionHandle }`. From then on the client task decodes every in-game packet and forwards it as `WorldCommand::Packet { session, packet }`.
- **The world thread owns all game state** (D-10). It receives commands from one `std::sync::mpsc` (or `crossbeam`-free `tokio::sync::mpsc` with `blocking_recv`/`try_recv`) channel, drains it, then runs the tick. Nothing else ever holds a reference into the world.
- **Outbound packets** are encoded in the world (`Encode::to_bytes`) and pushed to the player's `ConnectionHandle::send` (non-blocking, the connection task encrypts and writes). The world never waits on a socket.
- **Persistence** (D-15): the world sends `PersistCommand::StorePlayer(PlayerSnapshot)` etc. to the persistence task. Writes for one character are applied in order. Logout = world removes the player, sends the snapshot, the persistence task stores it and *then* tells the LS link `PlayerLogout` (so a quick re-login can't read stale data).

## Crates
- `l2-world`: `World`, `WorldCommand`, `WorldEvent`, entities, timer wheel, the `Outbox` trait. **No tokio, no sqlx, no sockets** (MODULE_MAP rule). Packets come in as decoded structs from `l2-protocol-game`.
- `game-server` (bin): network, LS link, persistence, admin; owns the world thread and the channels.

## Core types (sketch)

```rust
pub struct World {
    tick: u64,                       // 50 ms ticks since start (D-12)
    rng: WorldRng,                   // seedable (D-13)
    timers: TimerWheel,              // replaces ThreadPool.schedule / task managers
    players: HashMap<ObjectId, Player>,
    sessions: HashMap<SessionId, ObjectId>,
    out: Box<dyn Outbox>,            // packets + persist + LS link + admin replies
}

pub enum WorldCommand {
    EnterWorld { session: SessionId, player: PlayerData, conn: Box<dyn PacketSink> },
    Packet { session: SessionId, packet: InGamePacket },
    Disconnected { session: SessionId },
    Admin(AdminQuery),               // e.g. online players, reply through Outbox
    Shutdown,
}
```
- **Ids, not references** (D-11): a player's target, party, summon etc. are `ObjectId`s. Lookups go through `World`.
- **`Player`** = `CreatureCore` (position, heading, hp/mp/cp, level, stats later) + player data (account, name, class, race, appearance, exp/sp, inventory, skills, shortcuts, title…). No inheritance (D-17).
- **`Outbox` / `PacketSink` are traits**, so tests run the world synchronously: `world.handle(cmd); world.tick();` and inspect the collected output. Production implements them with `ConnectionHandle` and channel senders.
- **Timer wheel:** `schedule(at_tick, TimerKind, target)` returns a `TimerId`; cancel by id. `TimerKind` is an enum (autosave, revive, regen… added by later cards). Java's `ThreadPool.schedule(…, ms)` → `ticks = ceil(ms / 50)`.

## Tick (50 ms, D-12)
1. Drain all queued commands (bounded per tick so a flood can't starve the tick; the rest waits).
2. Fire due timers.
3. Movement/AI/regen (M3/M4; empty in M2).
4. Flush: nothing to do, sends are immediate.
A tick that takes longer than 50 ms logs a warning with its duration; the next tick starts immediately (no catch-up storm).

## Shutdown
`Shutdown` → world stores every online player (persist commands), sends `LeaveWorld`/kick to clients, returns. The game server waits for the persistence task to drain before exiting (Java `Shutdown` saves everyone).

## Compatibility with the Java GS on the same DB
The owner switches between Java GS and Rust GS on the same `l2jmobiush5`. Therefore:
- Rust writes rows exactly like Java (same columns, same defaults, same encodings). Every DB card copies the Java SQL statements verbatim, like M1-DB-01.
- Rust never deletes rows of tables it doesn't understand yet.
- Object ids come from an IdFactory port that scans the same tables Java scans (no id collisions with Java-created items/characters).
- **Side by side (D-39):** Java GS and Rust GS run at the same time with different server ids/ports. The LS guarantees an account is online on only one of them, so a character row is only ever written by one server at a time.
- **Object ids:** the Rust IdFactory scans the same tables as Java at startup and allocates new ids only from a separate high range (decided in M2-IDF-01), so the two allocators can never hand out the same id while both run.

## Testing
- Unit: drive `World` with commands, assert outbound packets (byte-exact against captures where they exist).
- Integration (DB-gated, CI): LS + GS in-process with fake clients from `l2-testkit`.
- Reference captures of the Java GS (recorded by the owner through the proxy, scrubbed) under `fixtures/captures/` are the ground truth for packet layouts.
