# java-vectors

Small Java programs that call the **real L2JMobius classes** and write JSON test vectors (D-41).
The Rust crates must match these files byte for byte. The JSON in `out/` is committed, so
`cargo test` never needs Java. You only run the generator when a vector set is added or changed.

## Build and run

Needs a JDK (tested with Java 25) on `PATH`. Sources are compiled straight from the L2JMobius
Java tree with `-sourcepath` (never from `l2jmobius/bin`, which may be stale). Nothing in the
Java tree is modified.

Windows (PowerShell):

```powershell
.\java-vectors\build.ps1            # build, then run every generator into java-vectors\out
.\java-vectors\build.ps1 crypto     # one generator
.\java-vectors\build.ps1 -NoRun     # compile only
```

Linux / macOS / Git Bash:

```sh
export L2J_SRC=/path/to/l2jmobius/java   # the folder that contains org/
./java-vectors/build.sh                  # build, then run every generator
./java-vectors/build.sh crypto           # one generator
NO_RUN=1 ./java-vectors/build.sh         # compile only
```

| Variable | Meaning | Default |
|---|---|---|
| `L2J_SRC` | Java source root of L2JMobius (contains `org/`) | `C:\l2-rust\l2jmobius\java` on Windows |
| `L2J_LIBS` | folder with the third-party jars (`dist/libs`) | `<L2J_SRC>/../dist/libs` |

Compiled classes go to `java-vectors/build/` (git-ignored). Running the script twice gives
byte-identical JSON: there is no clock and no unseeded randomness anywhere.

## Layout

```
java-vectors/
  build.ps1, build.sh        build + run
  src/l2rust/vectors/
    Main.java                dispatches: crypto, login-packets, geo, geo-engine, geo-path
    Json.java                tiny JSON writer (no dependencies)
    CryptoVectors.java       one generator = one class
    LoginPacketVectors.java  login / LS<->GS packet payloads
    GeoVectors.java          region-level geodata queries (geo_<set>_<region>.json)
    GeoEngineVectors.java    GeoEngine: coordinates, line iterators, height, LOS, movement (geo_engine_*.json)
    GeoPathVectors.java      PathFinding.findPath on the whole-world engine (geo_path_*.json)
  out/*.json                 committed output
```

## Vector file format

- Every file is a JSON object starting with `source` (Java class/method) and `generated_by`
  (always `"java-vectors"`), usually followed by a `note` that explains the shape.
- Bytes are **lowercase hex strings** (`"0102ff"`, empty = `""`). Load them in Rust with
  `l2_testkit::vectors::load` and `l2_testkit::from_hex`.
- Java `int` values are plain JSON numbers (signed, e.g. the XOR keys).
- Files are LF-terminated, 2-space indented, keys in a fixed order.

## Files (crypto, M1-JV-01)

| File | Java source | Contents |
|---|---|---|
| `crypto_blowfish.json` | `BlowfishEngine` | 16/16/22/40-byte keys, `encryptBlock` and `decryptBlock` on 8-byte blocks |
| `crypto_newcrypt.json` | `NewCrypt` | `appendChecksum`, `verifyChecksum` (valid, corrupted, bad sizes), `encXORPass`, multi-block `crypt`/`decrypt` |
| `crypto_login_encryption.json` | `LoginEncryption` | first packet with the static key (payload 1..40), session packets (payload 1..40) with decrypt and corrupted-packet cases |
| `crypto_scrambled_modulus.json` | `ScrambledKeyPair` | modulus bytes to scrambled modulus (with and without the leading `0x00`) |
| `crypto_rsa_nopadding.json` | `Cipher RSA/ECB/nopadding` | fixed RSA-1024 and RSA-512 test keys (n, e, d), encrypt and decrypt |
| `crypto_game_xor.json` | `Encryption`, `BlowFishKeygen` | game XOR cipher: enable-on-first-encrypt, key evolution over 5+ packets, offset wrap |

## Files (login packets, M1-JV-02)

`login_packets.json` holds the plaintext payload (opcode included, no length header, no encryption)
of 27 packet classes, 2+ variants each: login server to client (`Init`, `LoginFail`, `AccountKicked`,
`LoginOk`, `ServerList`, `PlayFail`, `PlayOk`, `GGAuth`), login server to game server (`InitLS`,
`LoginServerFail`, `AuthResponse`, `PlayerAuthResponse`, `KickPlayer`, `RequestCharacters`,
`ChangePasswordResponse`) and game server to login server (`BlowFishKey`, `AuthRequest`,
`PlayerInGame`, `PlayerLogout`, `ChangeAccessLevel`, `PlayerAuthRequest`, `ServerStatus`,
`PlayerTracert`, `ReplyCharacters`, `SendMail`, `TempBan`, `ChangePassword`).
Shape: `packets[] { name, java_class, direction, opcode, variants[] { what, fields, payload } }`
and `skipped[] { name, reason }`. Byte-array fields end in `_hex`.

How some payloads are produced (the real Java class is used unless stated):

- **ServerList.** The constructor needs `GameServerTable` and a `LoginClient`. The instance is
  allocated without its constructor, the fields are injected by reflection and the real
  `writeImpl` runs. The constructor logic (status computation, GM-only filtering) and the
  pending-delete timers (they use the clock) are not covered.
- **AuthResponse.** Calls `GameServerTable.getInstance()` (needs a database). The variants are a
  replay of its three write calls (`"produced_by": "replay"`).
- **TempBan.** Writes `System.currentTimeMillis()`. The real class runs, its expiry is range-checked
  against the clock, then the 8 bytes are replaced with `1700000000000 + minutes * 60000`
  (`"clock_pinned": true`).
- **BlowFishKey.** RSA-512 key with the same seed as `crypto_rsa_nopadding.json` (`rsa_n_hex` is in
  the fields). The encrypted output is deterministic because the padding is `nopadding`.

### Values not taken straight from one Java call

- **LoginEncryption first packet.** Java picks the XOR key with `Rnd.nextInt()` (thread-local
  random), which can't be injected. The generator replays the exact same steps with a fixed
  key (zero-padded buffer, `NewCrypt.encXORPass`, static-key `NewCrypt.crypt`). For every case it
  also runs the real `LoginEncryption.encrypt`, recovers the random key it used from the
  output, replays with that key, and aborts if the bytes differ. So the replay is proven equal
  to the real class.
- **RSA test keys.** Generated from fixed seeds with `BigInteger.probablePrime`. They are
  throwaway keys, not used by any server. The encrypt/decrypt calls go through the real JDK
  `Cipher`.
- **Scrambled modulus, extra cases.** Only the first modulus is a valid RSA modulus; the others
  are arbitrary odd numbers of the right size. `scrambleModulus` only shuffles bytes, so they
  still pin down the byte logic.

## Files (geo engine, M3-GEO-01)

The two geo generators need the geodata folders (git-ignored, never copied into the repo), so they
are not part of `all`. `build.ps1 -NoRun` compiles them; run them by hand from a folder that has no
`data/Doors.xml` (see below), with a JVM that can hold the world (`-Xmx6g` is plenty):

```powershell
java -Xmx6g -cp "java-vectors\build;C:\l2-rust\l2jmobius\dist\libs\*" l2rust.vectors.Main geo-engine java-vectors\out C:\l2-rust\geodata-download C:\l2-rust\geodata-converted
```

| File | Java source | Contents |
|---|---|---|
| `geo_engine_coords.json` | `GeoEngine.getGeoX/getGeoY/getWorldX/getWorldY`, `GridLineIterator2D/3D` | 346 world to geo and 329 geo to world conversions per axis (edges, negatives, int overflow), 170 paths per line iterator. Needs no geodata, so CI runs it. |
| `geo_engine_download.json`, `geo_engine_converted.json` | `GeoEngine` (the real constructor, whole world of the set) | per set: 960 `getHeight`/`hasGeo`/`getSpawnHeight`/Z lookups, 640 anti-corner-cut NSWE + `isCompletelyBlocked`, 1760 `canSeeTarget`, 1200 `canMoveToTarget`, 1200 `getValidLocation`, plus 2 x 400 of the last three with one config flag flipped. Regions 12_24, 13_21, 14_23, 16_10, 22_22, 17_10 and two tile positions without a file. |

Cases are one string per query, `"<inputs> > <outputs>"`, to keep the files small (see the `note` in each file).

How the engine is driven: `GeoEngine`'s constructor is protected and reads `GeoEngineConfig.GEODATA_PATH`
(a public static), so the generator sets the statics (`PATHFINDING = 2`, `AVOID_OBSTRUCTED_PATH_NODES = true`,
the shipped ini values) and calls the constructor by reflection, once per set. **Doors and fences are not
stubbed:** the real `DoorData` / `FenceData` singletons load with no `./data/Doors.xml` / `FenceData.xml`
(warning only) and `World.init()` is never called, so `checkIfDoorsBetween` / `checkIfFenceBetween` return false.
The generator aborts if those files exist in the working directory. Only instance id 0 is used.

## Files (pathfinding, M3-GEO-02)

Same arguments and the same rules as the geo engine generator (run by hand from a folder without
`data/Doors.xml`, `-Xmx6g`, not part of `all`):

```powershell
java -Xmx6g -cp "java-vectors\build;C:\l2-rust\l2jmobius\dist\libs\*" l2rust.vectors.Main geo-path java-vectors\out C:\l2-rust\geodata-download C:\l2-rust\geodata-converted
```

| File | Java source | Contents |
|---|---|---|
| `geo_path_download.json`, `geo_path_converted.json` | `PathFinding.findPath` (real `NodeBuffer` A*, `GeoLocation`, LOS postfilter) on the whole-world `GeoEngine` | per set 4 config variants (`shipped`, `advanced_diagonal`, `odd_weights`, `no_postfilter`; ~450 + 3 x 175 cases): start/target pairs 1 to 350 cells apart in the six analysis regions, targets on completely blocked cells, same cell and neighbour cells, starts without geodata. Expected results are the node lists (`nodeX,nodeY,geoHeight,nswe`) or `null` (no path, iteration limit, no geodata, no buffer large enough). |

How it is driven: the first `GeoEngine.getInstance()` loads the first set; the second set is loaded by calling
the protected constructor by reflection (the region table is static, so the singleton sees it). `PathFinding`
reads the weights in its constructor and `NodeBuffer` when a thread-local buffer is allocated, so every config
variant gets a fresh `PathFinding` by reflection after the `GeoEngineConfig` statics are set.

## Writing a new generator

Copy the pattern of `CryptoVectors.java`:

1. One class per vector family with `static void generate(Path outDir)`, registered in `Main`.
2. All inputs are explicit constants or `new Random(<fixed seed>)`. Never `Rnd`, `SecureRandom`,
   `System.currentTimeMillis()` or `HashMap` iteration order.
3. Call the real Java class for the expected output. If it can't be driven deterministically,
   replay its steps and **check the replay against the real class** (see the login first packet).
4. Build files with `Json.header(...)`, `Json.obj()`, lists and `byte[]` values; write with
   `Json.write`.
5. Add a shape test to `crates/l2-testkit/tests/` and a short row in the table above.
6. Run the script twice and check that `git status` shows no change in `out/`.
