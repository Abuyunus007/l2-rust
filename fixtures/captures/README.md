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
