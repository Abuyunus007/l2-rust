//! Test support shared by all crates. Use it as a **dev-dependency only**.
//!
//! - [`vectors`]: load the JSON test vectors the Java harness writes to `java-vectors/out/` (D-41).
//! - [`bytes`]: hex conversion and byte-slice assertions with readable diffs.
//! - [`fake_login_client`], [`fake_game_server`]: a fake H5 login client and a fake game server
//!   that speak the real wire protocols with the real ciphers (login server integration tests).
//! - [`fake_game_client`]: a fake H5 game client (character screen, enter world, chat, logout)
//!   for end-to-end tests against the Rust game server.
//! - [`test_db`]: a throwaway database with the login schema, gated on `L2_TEST_DATABASE_URL`.
//! - [`capture_compare`]: compare two JSONL captures by packet type and fields, replay a
//!   recorded client (the "same decisions as Java" check).
//! - [`wait`]: polling helper for state that changes on another task.
//!
//! Failures here panic on purpose: this crate only runs inside tests, and a panic with a clear
//! message is how a test reports a failure.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod bytes;
pub mod capture_compare;
pub mod dist;
pub mod fake_game_client;
pub mod fake_game_server;
pub mod fake_login_client;
pub mod test_db;
pub mod vectors;
pub mod wait;

pub use bytes::{assert_bytes_eq, from_hex, to_hex};
pub use dist::{dist_dir, game_dir, login_dir, sql_dir};
