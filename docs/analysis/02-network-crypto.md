# Network layer, crypto and the Java threading model

Source: `commons/network/`, `commons/crypt/`, `gameserver/network/`.

## Shared network core (`commons/network`, ~2.5k lines)

- `ConnectionManager`: NIO2 `AsynchronousServerSocketChannel` on its own thread pool (`Network.ini` → `ThreadPoolSize`, `<1` means CPUs × 4).
- Framing: `u16 LE length` including the 2 header bytes. `ReadHandler` reads the header, then the payload (max 65533). It decrypts in place, gets a packet object from the `PacketHandler` (opcode → packet), calls `read()` (parse), and on success submits `run()` to the **`PacketExecutor` thread pool**.
- ⚠️ **Packets from the same client can run on different threads at the same time.** Ordering is not guaranteed once a packet is in the executor. The game code therefore locks almost everything (`GameClient.getPlayerLock()`, `synchronized` in `Player`/`Inventory`, concurrent maps everywhere).
- Writes: `Client.writePacket` → `WritablePacket.write` into pooled buffers (`ResourcePool`/`BufferPool`, configured in `Network.ini`) → `encrypt` → `WriteHandler`.
- `Network.ini` also has `DropPackets`/`DropPacketThreshold` (flood control on the incoming queue).

The Rust replacement is tokio: one task per connection with a read loop and a write channel. The buffer pool is unnecessary at first (`bytes::BytesMut`). Behaviour to keep: framing, max size, drop threshold, and the state checks.

## Packet primitives
- Integers are little-endian: `writeByte/Short/Int/Long/Double`.
- Strings: UTF-16LE, terminated by `0x0000`.
- `readString` on LS↔GS packets reads `u16` units until 0.
- The client "ex" packets use opcode `0xD0` + `u16` sub-opcode. Ex `0x51` has a further `i32` sub-sub-id (bookmark slots).

## Crypto

| Use | Algorithm | Details |
|---|---|---|
| Login client traffic | Blowfish **LE word order**, ECB | static key for the first server packet + XOR pass. Per-client random 16-byte key after that; checksums on every packet. See 01. |
| Login credentials | RSA-1024, no padding | scrambled modulus in `Init` |
| LS↔GS link | Blowfish LE + checksum, RSA-512 key exchange | default key `"_;v.]05-31!\|+-%xT!^[$\0"` |
| Game client traffic | rolling XOR stream cipher (`gameserver/network/Encryption.java`) | 16-byte key: 8 random bytes + fixed tail `C8 27 93 01 A1 6C 31 97`. `out[i] = in[i] ^ key[i&15] ^ out[i-1]`. After each packet, bytes 8..11 of the key (u32 LE) += packet size. The first outgoing packet (`KeyPacket`) is sent unencrypted. Separate in/out key state. |
| Game client traffic, your config | **off** | `Server.ini PacketEncryption = False` → `KeyPacket` tells the client "no encryption". The XOR cipher still has to be implemented (config option), but your normal traffic is plaintext. |

`BlowfishEngine.java` (1,468 lines) is mostly S-box tables. Rust uses the `blowfish` crate (`BlowfishLE`) and checks it against vectors made with the Java class.

## Game server packets

| | Count | Lines |
|---|---:|---:|
| Client packets (incl. ex) | 170 + 144 opcodes / 280 classes | 31,312 |
| Server packets | 485 enum ids / 389 classes | 28,045 |
| `SystemMessageId` | 3,236 entries (`@ClientString(id, message)`) | 10,041 |
| `NpcStringId` | 5,915 entries | 18,098 |

- `ClientPackets.java` / `ExClientPackets.java` are tables of opcode → class → allowed `ConnectionState`s. `ServerPackets.java` holds opcodes for writing.
- Game connection states: `CONNECTED → AUTHENTICATED → ENTERING → IN_GAME` (plus `DISCONNECTED`, `CLOSING`).
- Handshake: `ProtocolVersion(0x0E)` → `KeyPacket` (allowed revisions in `Server.ini`: `267;268;271;273`) → `AuthLogin` (account + session key, checked with the LS via `PlayerAuthRequest`) → character list → select → `EnterWorld(0x11)`.
- **Code generation opportunity:** the opcode tables, the `SystemMessageId`/`NpcStringId` enums, and most server packet *layouts* (sequences of `writeX` calls) can be extracted mechanically from the Java source. A generator (Rust or Python) is cheaper and more reliable than agents for these, and the generated Rust code becomes the pattern Haiku follows for the handler logic.

## Packet tool implications (milestone 1)

- **Login traffic:** a man-in-the-middle proxy can't use the server's RSA key, but it doesn't need to. It sits between client and LS and acts as the LS toward the client with its own RSA/Blowfish keys, and as a client toward the real LS (decrypting the real `Init` with the static key). It can then log both sides decoded. It must rewrite the GS IP/port in `ServerList` so the client connects to the proxy's game port next.
- **Game traffic:** with `PacketEncryption = False` it's plaintext. The proxy forwards it and decodes opcodes and fields using the shared packet definitions. With encryption on, the proxy sees the key in `KeyPacket`, so it can decrypt passively too.
- Output: a structured capture file (JSON lines: time, direction, opcode, name, decoded fields, raw hex). These captures become **reference fixtures** for Rust server tests.
