# Login server: protocol and behaviour

Source: `java/org/l2jmobius/loginserver/` (56 files, ~7.4k lines). This is the spec for milestone 1.
Anything marked **[verify]** must be confirmed against a packet capture of the Java server before the Rust version is accepted.

## Processes and ports (from `dist/login/config/Server.ini`)

| What | Default | Config key |
|---|---|---|
| Client listener (H5 client → LS) | `0.0.0.0:2106` | `LoginserverHostname`, `LoginserverPort` |
| Game server listener (GS → LS) | `127.0.0.1:9014` | `LoginHostname`, `LoginPort` |
| Failed logins before IP ban / ban seconds | 5 / 900 | `LoginTryBeforeBan`, `LoginBlockAfterBan` |
| Show licence (sends `LoginOk` before server list) | `True` | `ShowLicence` |
| Auto-create accounts on first login | `True` | `AutoCreateAccounts` |
| Accept unknown game servers | `True` | `AcceptNewGameServer` |
| Flood protection | on, 15 fast / 700 ms normal / 350 ms fast / 50 per IP | `EnableFloodProtection`, `FastConnectionLimit`, ... |
| Scheduled restart | off / 24 h | `LoginRestartSchedule`, `LoginRestartTime` |

Other login files: `Database.ini`, `Network.ini`, `Threads.ini`, `Interface.ini`, `banned_ip.cfg` (format: `ip [durationMillis]  # comment`), `data/servername.xml` (id → name), `log.cfg`.
Exit codes: 0 = shutdown, 2 = restart requested (the start scripts loop on 2), 1 = fatal.

## Startup

1. Load config, connect to the DB (HikariCP).
2. Generate **10 RSA-1024 key pairs** (e = 65537) for clients and precompute each one's *scrambled modulus*.
3. Generate **20 random 16-byte Blowfish keys**. Each client gets a random one.
4. `GameServerTable`: load `servername.xml`, load registered servers from the `gameservers` table, and generate **10 RSA-512 key pairs** (these are used for game server links).
5. Load `banned_ip.cfg`.
6. A purge task runs every 5 min. It kicks clients that haven't joined a GS within `LOGIN_TIMEOUT` = 5 min, and disconnected clients.
7. Open both listeners.

## Client ↔ LS protocol

### Framing
`u16 LE length` (includes these 2 bytes), then the encrypted body. All integers are little-endian. Strings in LS↔GS packets are UTF-16LE with a 0x0000 terminator.

### Encryption (`LoginEncryption`, `NewCrypt`, `BlowfishEngine`)
- **Blowfish, little-endian word order** (`bytesTo32bits` reads bytes [3,2,1,0]). ECB over 8-byte blocks.
- **First server packet (`Init`)** uses the static key `6B 60 CB 5B 82 CE 90 B1 CC 2B 6C 55 6C 6C 6C 6C`:
  - body size = payload + 8 header bytes, padded up to a multiple of 8 (a full extra block is added when it's already aligned), plus 8;
  - `encXORPass(raw, 0, size, randomInt)` (rolling XOR over 32-bit words starting at offset 4; the final key is written into the last 8 bytes), then static Blowfish over the whole body.
- **All later server packets:** body = payload + 4 header bytes, padded as above, plus 8. Then `appendChecksum` (XOR of all 32-bit words, stored in the last word before the end), then Blowfish with the client's session key.
- **All client packets** are decrypted with the session key, and the checksum must verify. Otherwise the connection closes. **[verify]** the exact padding sizes against a capture. The Java padding logic is easy to get off by one block.

### RSA and the scrambled modulus
The 128-byte modulus (leading 0x00 stripped) is scrambled before it's sent:
1. swap bytes `0x00..0x03` with `0x4D..0x50`
2. `b[0x00..0x3F] ^= b[0x40..0x7F]`
3. `b[0x0D..0x10] ^= b[0x34..0x37]`
4. `b[0x40..0x7F] ^= b[0x00..0x3F]`

The client encrypts its credentials with raw RSA (no padding). The server decrypts with `RSA/ECB/NoPadding`.

### Connection states
`CONNECTED → AUTHED_GG → AUTHED_LOGIN`. A packet that arrives in the wrong state is silently ignored.

### Client → LS packets

| Op | Name | State | Payload | Handling |
|---|---|---|---|---|
| 0x07 | AuthGameGuard | CONNECTED | `i32 sessionId`, 4× `i32` (ignored) | sessionId must match, then → AUTHED_GG, reply `GGAuth` |
| 0x00 | RequestAuthLogin | AUTHED_GG | 128 bytes (old) or 256 bytes (new auth) of RSA blocks | see below |
| 0x05 | RequestServerList | AUTHED_LOGIN | `i32 loginOk1, i32 loginOk2` | key pair must match, reply `ServerList`, otherwise `LoginFail(ACCESS_FAILED)` |
| 0x02 | RequestServerLogin | AUTHED_LOGIN | `i32 loginOk1, i32 loginOk2, u8 serverId` | see below |
| 0x0E / 0x0F | PI agreement | AUTHED_LOGIN | not implemented (ignored) | |

**RequestAuthLogin decoding:**
- New method (256 bytes, H5): user = `trim(dec[0x4E..+50]) + trim(dec[0xCE..+14])`, password = `trim(dec[0xDC..+16])`
- Old method (128 bytes): user = `dec[0x5E..+14]`, password = `dec[0x6C..+16]`

**Login flow:**
1. Hash: `Base64(SHA-1(UTF-8 password))`, compared with `accounts.password`.
2. The account query also folds in temp bans: `account_data` row `var='ban_temp'`. If its value (ms timestamp) is still in the future, accessLevel becomes -1.
3. Unknown account: auto-create if enabled (`accessLevel 0`, `lastIP`), otherwise fail. Each failure is counted per IP, and reaching `LoginTryBeforeBan` adds a temporary IP ban.
4. `accessLevel < 0` → `AccountKicked(PERMANENTLY_BANNED)`.
5. IP allow/deny lists from `accounts_ipauth`. A failure counts as a wrong password.
6. On success, update `lastactive` and `lastIP`.
7. Already on a GS → `LoginFail(ACCOUNT_IN_USE)`, and the LS sends `KickPlayer` to that GS. Already logged in on the LS → kick both.
8. Otherwise: generate a `SessionKey` (4 random i32s: loginOk1/2, playOk1/2), send `RequestCharacters` to every authed GS, then send `LoginOk` (if ShowLicence) or `ServerList`.

**RequestServerLogin:** the loginOk pair is checked only when ShowLicence is on. If the LS status is DOWN, or GM-only and the account isn't a GM → `LoginFail(ACCESS_FAILED)`. If the GS is authed and has room → update `accounts.lastServer` and send `PlayOk(playOk1, playOk2)`. Otherwise `PlayFail(SERVER_OVERLOADED)`.

### LS → client packets

| Op | Name | Payload |
|---|---|---|
| 0x00 | Init | `i32 sessionId, i32 0x0000C621 (protocol), 128B scrambled modulus, i32 0x29DD954E, 0x77C39CFC, 0x97ADB620, 0x07BDE0F7 (GG), 16B blowfish key, u8 0` |
| 0x01 | LoginFail | `u8 reason` (see `LoginFailReason`: 0x02 wrong user/pass, 0x07 in use, 0x15 access failed, ...) |
| 0x02 | AccountKicked | `i32 reason` (0x01 data stealer, 0x08 generic, 0x10 7-day, 0x20 permanent) |
| 0x03 | LoginOk | `i32 loginOk1, i32 loginOk2, i32 0, i32 0, i32 0x3EA, i32 0, i32 0, i32 0, 16B zero` |
| 0x04 | ServerList | `u8 count, u8 lastServer`, per server: `u8 id, 4B ip, i32 port, u8 ageLimit, u8 pvp, u16 cur, u16 max, u8 up, i32 type, u8 brackets`; then `u16 0`, `u8 n`, per entry `u8 serverId, u8 chars, u8 nDel, nDel × i32 secondsUntilDelete` |
| 0x06 | PlayFail | `u8 reason` |
| 0x07 | PlayOk | `i32 playOk1, i32 playOk2` |
| 0x0B | GGAuth | `i32 sessionId, 4× i32 0` |

Notes for ServerList:
- Servers with status GM_ONLY are hidden from non-GM accounts. For accessLevel < 0, every server shows as down.
- The IP sent depends on the client's address: `GameServerAddress` subnet matching (LAN vs. WAN hosts from the GS's host list).
- The Java code **busy-waits up to 500 ms** for `ReplyCharacters` from the game servers before it builds the list. In Rust this becomes an async wait with a timeout, not a sleep.

## LS ↔ GS protocol (port 9014)

### Framing and crypto
`u16 LE length` (includes the header), then a Blowfish (little-endian) encrypted body. The body is padded with zeros to a multiple of 8, and the last 4 bytes of the plaintext are the XOR checksum.
- Initial key: the 22 bytes of `"_;v.]05-31!|+-%xT!^[$"` plus a trailing NUL.
- After `BlowFishKey`: the 40-byte random key chosen by the GS.
- One thread per GS in Java with blocking IO. In Rust: one tokio task per GS link.

### Sequence
```
GS connects
LS → InitLS(0x00)        i32 protocolRev=0x0106, i32 len, modulus bytes (RSA-512, Java BigInteger.toByteArray: may have a leading 0x00)
GS → BlowFishKey(0x00)   i32 len, RSA-NoPadding(40-byte key)   [LS strips leading zero bytes after decrypt]
     both sides switch to the new Blowfish key → state BF_CONNECTED
GS → GameServerAuth(0x01) u8 desiredId, u8 acceptAlternate, u8 reserved, u16 port, i32 maxPlayers,
                          i32 hexIdLen(≤64), hexId bytes, i32 hostPairs(≤32), 2×hostPairs strings (subnet, host)
LS → AuthResponse(0x02)  u8 serverId, string serverName      → state AUTHED
     (or LoginServerFail(0x01) u8 reason: 1 IP banned, 2 IP reserved, 3 wrong hexid, 4 id reserved, 5 no free id, 6 not authed, 7 already logged in)
```

Registration rules: a known id with a matching hexId gets attached (rejected if already authed). An unknown id is accepted if `AcceptNewGameServer` is on, and gets stored in `gameservers` (hexid is stored as a hex string, e.g. `-2ad66b3f...`, which is Java `BigInteger.toString(16)`; **[verify]** the encoding). With a wrong hexId, if both `AcceptNewGameServer` and `acceptAlternate` are set, the GS gets the first free id.

### GS → LS (state AUTHED)

| Op | Name | Payload | Effect |
|---|---|---|---|
| 0x02 | PlayerInGame | `u16 n`, n × string account | add accounts to the GS's online set, drop them from the LS authed list |
| 0x03 | PlayerLogout | string account | remove the account from the online set |
| 0x04 | ChangeAccessLevel | `i32 level`, string account | update `accounts.accessLevel` |
| 0x05 | PlayerAuthRequest | string account, `i32 playOk1, playOk2, loginOk1, loginOk2` | compare with the stored SessionKey (loginOk is compared only when ShowLicence is on), reply PlayerAuthResponse |
| 0x06 | ServerStatus | `i32 n`, n × (`i32 type, i32 value`) | types: 1 status, 2 server type, 3 brackets, 4 max players, 5 test, 6 age |
| 0x07 | PlayerTracert | 6 strings: account, pcIp, hop1-4 | update the `accounts` pcIp/hop columns |
| 0x08 | ReplyCharacters | string account, `u8 chars`, `u8 nDel`, nDel × `i64` delete time ms | stored on the waiting client, used by ServerList |
| 0x09 | SendMail | ignored | |
| 0x0A | RequestTempBan | string account, string ip, `i64 banTime`, `u8 hasReason`, [string reason] | temp ban |
| 0x0B | ChangePassword | 4 strings: login, character, current, new | verify and update the hash, reply ChangePasswordResponse |

### LS → GS

| Op | Name | Payload |
|---|---|---|
| 0x00 | InitLS | see above |
| 0x01 | LoginServerFail | `u8 reason` |
| 0x02 | AuthResponse | `u8 id`, string name |
| 0x03 | PlayerAuthResponse | string account, `u8 ok` |
| 0x04 | KickPlayer | string account |
| 0x05 | RequestCharacters | string account |
| 0x06 | ChangePasswordResponse | string character, string message |

## Database (login)

```sql
accounts(login PK varchar45, password varchar45 /*base64 sha1*/, email, created_time, lastactive bigint ms,
         accessLevel tinyint, lastIP char15, lastServer tinyint default 1, pcIp, hop1..hop4)
account_data(account_name, var, value, PK(account_name,var))   -- var='ban_temp' value=ms timestamp
accounts_ipauth(login, ip, type enum('deny','allow'))
gameservers(server_id PK, hexid varchar50, host varchar50)
```
The Rust LS must use the **same schema**, so the Java GS and the Java LS tools (AccountManager, GameServerRegister) keep working against it.

## Out of scope for the Rust LS v1
- The Swing GUI (`ui/`), the splash screen and the `Interface.ini` GUI flag. Rust is headless and logs to console + file.
- `DatabaseBackup` on shutdown (optional, later).

## Acceptance for milestone 1 (draft)
1. Unit tests: Blowfish-LE test vectors, checksum, XOR pass, scrambled modulus, packet encode/decode round-trips. Byte-exact against vectors produced by the Java classes.
2. The **Java GS** registers with the **Rust LS** (hexid flow, AuthResponse).
3. The real H5 client logs in, sees the server list with character counts, picks the server and enters the game on the Java GS (PlayerAuthRequest flow).
4. Wrong password, IP ban after 5 failures, account in use (kick), auto-create all behave like Java.
5. Same logs/decisions as Java for the same inputs (compared through the packet tool).
