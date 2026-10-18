# Captures used as test fixtures

JSON-lines files recorded with `l2-proxy` (format: `crates/l2-proxy/src/capture.rs`, D-34).
The repo is public: a capture is only committed after it has been checked (see below).

## `java-ls-login.jsonl` (to be recorded by the owner)

A login session with the **Java** login server. The test
`the_rust_login_server_answers_like_the_java_capture` in `crates/login-server/tests/e2e.rs`
replays every connection of it against the **Rust** login server (behind an in-process proxy)
and compares the packet types and fields. Random values (session id, keys, login/play pairs),
addresses and player counts are ignored. Until the file exists the test prints "skipped".

How to record it: `docs/runbooks/m1-login-test.md`, part 6. In short: Java login server on
port 2107, `l2-proxy --listen 127.0.0.1:2106 --upstream 127.0.0.1:2107 --no-rewrite --capture ...`,
Java game server stopped, then in the client: one login with a wrong password for an existing
account, one login with the right password, look at the server list, close the client.

What the replay needs from the recording:
- Only the login channel is used. Record with `--no-rewrite` (no `proxy_ip` fields).
- Passwords are never in a capture (`RequestAuthLogin` is stored as `<redacted>`), and account
  names are only recorded with `--log-accounts`, which you must not use for a committed file.
  The replay decides by itself which password to send: when the Java login server answered
  `LoginOk` it sends the right one, otherwise a wrong one (account `compat`, created by the test).

## Before committing a capture

1. Open it in a text editor. Every line is one packet.
2. Search for your public IP address and for account names. The `ServerList` packet holds the
   address the game server announced: `127.0.0.1` or a `192.168.x.x` address is fine, a public
   address is not. If it is public, do not commit; ask the orchestrator for a scrub step.
3. The file must be LF text (the repo converts line endings).

## `java-gs-*.jsonl` (recorded from the owner's Java GS, scrubbed)

Game-channel sessions with the **Java** game server, recorded by the proxy in front of the LS
(D-39) and made safe with the scrubber (M2-CAP-01):

| File | Contents |
|---|---|
| `java-gs-session.jsonl` | character select, enter world, moving around, chat, restart, logout |
| `java-gs-charscreen.jsonl` | character screen: NewCharacter, two CharacterCreate, select, delete (marked), restore |

Used by the capture tests of `l2-protocol-game` (`tests/charscreen.rs`, `chat.rs`,
`enterworld_big.rs`, `enterworld_small.rs`): every packet that has a struct is decoded and
re-encoded and must be byte-identical. They read `L2_CAPTURE_DIR` when it is set (your own,
unscrubbed recordings in `run/captures/`), else this folder.

### Making a new one

```
cargo run --release -p l2-proxy --bin scrub-capture -- run/captures/capture-<ms>.jsonl fixtures/captures/java-gs-<topic>.jsonl --secret <account> --secret <clan name>
```

The scrubber (`crates/l2-proxy/src/scrub.rs`) keeps only game packets; replaces account names,
character names, titles and chat lines (found automatically, plus every `--secret`) with
placeholders of the same length in the fields and in the raw bytes; zeroes session ids and the
game key; and refuses to write anything if a secret is still found afterwards. Pass every
account name of the session and any clan name with `--secret`. Then still read the file once.
