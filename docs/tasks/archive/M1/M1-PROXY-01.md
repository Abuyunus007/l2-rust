# M1-PROXY-01: Login MITM proxy and capture format

| Field | Value |
|---|---|
| Milestone | M1 |
| Model | sonnet |
| Review | full |
| Reference | yes (capture format used by tests and the GUI) |
| Depends on | M1-LPROTO-02, M1-CRYPTO-02, M1-CRYPTO-03, M1-NET-02 |
| Size | M |

## Goal
`l2-proxy` (library + small CLI): sits between the H5 client and a login server (Java or Rust), decrypts both directions, logs every packet decoded into a JSONL capture, and rewrites the server list so the client's game connection also goes through the proxy.

## Java source (the spec)
- Protocol: `docs/analysis/01-login-server.md`, `docs/analysis/02-network-crypto.md` ("Packet tool implications")
- `C:\l2-rust\l2jmobius\java\org\l2jmobius\gameserver\network\PacketLogger.java` (small reference for what Java logs)

## Reference implementation to copy
- `crates/l2-testkit/src/*` fake client code if it exists; otherwise `crates/login-server/src/client/` for the server-side handshake

## Files
Create:
- `crates/l2-proxy/{Cargo.toml,src/lib.rs,src/capture.rs,src/login.rs,src/rewrite.rs,src/bin/l2-proxy.rs}`
- `crates/l2-proxy/tests/login_mitm.rs`

## Requirements
1. Toward the **client**, the proxy is a login server with its **own** RSA-1024 pair and BF key: it sends its own `Init` (the real session id copied from the real server's Init so GG auth matches).
2. Toward the **real LS**, it's a client: it decrypts the real `Init` (static key + XOR), learns the real BF key and the real modulus (`unscramble_modulus`).
3. `RequestAuthLogin`: decrypt the client's RSA block(s) with the proxy key, then **re-encrypt** with the real modulus and forward. Credentials never appear in the capture: replace them with `"<redacted>"` (and log only the account name if `--log-accounts` is given).
4. `ServerList`: replace each server's IP/port with the proxy's game listener (one listen port per server id) and remember the real address for M1-PROXY-02.
5. Capture format (JSONL, one record per packet): `{ "ts_ms", "conn", "dir": "c2s"|"s2c", "channel": "login"|"game"|"link", "opcode", "name", "fields": {…} | null, "raw": "<hex of plaintext payload>" }`. Version header line first. `capture.rs` provides the writer + reader (used by the GUI and tests).
6. `l2-proxy` CLI: `--listen 127.0.0.1:2106 --upstream <ip:port> --capture file.jsonl`. The library API lets the GUI run it in-process with a live record channel (D-35).

## Out of scope
- Game traffic (M1-PROXY-02).

## Tests
- In-process: fake client ↔ proxy ↔ Rust LS (or a fake LS from `l2-protocol-login`): login succeeds through the proxy. The capture contains Init, GGAuth, AuthLogin (redacted), LoginOk, ServerList (original and rewritten addresses), PlayOk, with decoded fields.

## Done when
- [ ] tests pass
- [ ] committed as `M1-PROXY-01: <summary>`

## Notes from the orchestrator
The real H5 client connects to whatever `l2.ini`/hosts points at. The runbook (M1-LS-06) explains pointing it at the proxy. Never ship credentials in captures: fixtures are committed to a public repo.
