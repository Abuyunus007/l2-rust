# M1-NET-01: Packet reader/writer and packet traits

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | opus (orchestrator) |
| Review | full |
| Reference | yes (every packet struct in the project uses these traits) |
| Depends on | M1-WS-01 |
| Size | S |

## Goal
`l2-net` with the primitives every packet uses: a bounds-checked little-endian reader/writer, UTF-16LE strings, and `Encode`/`Decode` traits.

## Java source (the spec)
Base: `C:\l2-rust\l2jmobius\java\org\l2jmobius\commons\network\`
- `packet\ReadablePacket.java`, `packet\SimpleReadablePacket.java` (readString = u16 units until 0; behaviour on truncated data)
- `packet\WritablePacket.java`, `packet\SimpleWritablePacket.java`, `buffer\WriteBuffer.java`, `buffer\ReadBuffer.java` (writeString writes UTF-16LE + 0x0000; null string handling)

## Reference implementation to copy
none (this card creates it)

## Files
Create:
- `crates/l2-net/{Cargo.toml,src/lib.rs,src/reader.rs,src/writer.rs,src/packet.rs,src/error.rs}`

## Requirements
1. `PacketReader<'a>` over `&[u8]`: `u8/i8/u16/i16/u32/i32/i64/f64/bytes(n)/string()/remaining()`. Every read returns `Result<_, PacketError>` (truncated data is an error, never a panic). `string()` decodes UTF-16LE until 0x0000, and a missing terminator is an error.
2. `PacketWriter` (`Vec<u8>` backed): matching writes. `string(&str)` writes UTF-16LE + terminator. `Option<&str>` None writes just the terminator (check what Java does for null).
3. Traits: `trait Encode { fn encode(&self, w: &mut PacketWriter); }`, `trait Decode: Sized { fn decode(r: &mut PacketReader) -> Result<Self, PacketError>; }`. The opcode is part of the struct's encoding (as in Java `writeImpl`). Decoders for client packets are called **after** the opcode byte(s) were consumed by the dispatcher (as in Java `readImpl`).
4. A doc-comment example on `Encode` showing a minimal packet. M1-LPROTO-01 copies this style.

## Out of scope
- Framing, sockets, encryption (M1-NET-02).

## Tests
- Round trip for each primitive, including negative ints, non-BMP characters (surrogate pairs), empty strings.
- Truncation tests: every read on short input returns an error.

## Done when
- [ ] tests pass
- [ ] committed as `M1-NET-01: <summary>`
