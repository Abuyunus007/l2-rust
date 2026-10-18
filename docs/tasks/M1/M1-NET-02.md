# M1-NET-02: Framing codec and connection task

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | opus (orchestrator) |
| Review | full |
| Reference | yes (all servers and the proxy use it) |
| Depends on | M1-NET-01 |
| Size | M |

## Goal
A tokio-based connection layer: u16-LE-length framing, a pluggable cipher, a read loop that hands decoded frames to a handler, and a write queue. It replaces Java's NIO2 `ConnectionManager`/`ReadHandler`/`WriteHandler`.

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\commons\network\`
- `ConnectionManager.java`, `Connection.java`, `Client.java` (encrypt/decrypt hooks, close-after-send)
- `handler\ReadHandler.java` (header = u16 incl. itself, `dataSize <= 0` → keep reading, `MAX_PACKET_SIZE = 65533`, decrypt → handler → read → execute)
- `handler\WriteHandler.java`, `ConnectionConfig.java` (`DropPackets`, `DropPacketThreshold`, `TCP_NODELAY`)

## Reference implementation to copy
- `crates/l2-net/src/reader.rs` (style)

## Files
Create:
- `crates/l2-net/src/codec.rs` (`tokio_util::codec` Decoder/Encoder for frames)
- `crates/l2-net/src/cipher.rs` (`trait FrameCipher { fn encrypt(&mut self, payload: &[u8]) -> Vec<u8>; fn decrypt(&mut self, body: &mut Vec<u8>) -> Result<(), CipherError>; }`, `NoCipher`)
- `crates/l2-net/src/connection.rs` (spawned read + write tasks, an `mpsc` outbox, a close-after-flush command, a `ConnectionHandle`)
- `crates/l2-net/tests/connection.rs`
Modify:
- `crates/l2-net/src/lib.rs`, `crates/l2-net/Cargo.toml`

## Requirements
1. Frames: `u16 LE` total length incl. the 2 header bytes. Frames > 65535 total are rejected, and so are payloads > 65533 (disconnect).
2. The cipher is applied per frame body. Login/game/link ciphers from `l2-crypto` are wrapped in this trait by their users, so `l2-net` doesn't depend on `l2-crypto`.
3. Incoming frames are delivered **in order** to a `FrameHandler` (a trait or `mpsc::Sender<Frame>`). There's no parallel execution per connection (unlike Java, intentionally: D-10).
4. Outbox: `ConnectionHandle::send(bytes)`, `send_and_close(bytes)` (Java `close(packet)`), `close()`. The drop threshold: if `DropPackets` is on and the inbound queue exceeds `DropPacketThreshold`, drop frames and log a warning (Java semantics).
5. Set `TCP_NODELAY` (Java default `useNagle = false`).
6. A listener helper: `listen(addr, factory)` accepting connections and spawning a connection per socket. Graceful shutdown via a `CancellationToken`.

## Out of scope
- Any login/game-specific logic.

## Tests
- Codec: partial header, partial body, multiple frames in one read, zero/oversized length.
- Loopback test over 127.0.0.1: echo with a toy XOR cipher, ordering of 1,000 frames, send_and_close delivers the last frame before closing.

## Done when
- [ ] tests pass on Windows and Linux
- [ ] committed as `M1-NET-02: <summary>`
