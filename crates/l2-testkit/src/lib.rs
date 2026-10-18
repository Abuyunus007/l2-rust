//! Test support shared by all crates. Use it as a **dev-dependency only**.
//!
//! - [`vectors`]: load the JSON test vectors the Java harness writes to `java-vectors/out/` (D-41).
//! - [`bytes`]: hex conversion and byte-slice assertions with readable diffs.
//!
//! Failures here panic on purpose: this crate only runs inside tests, and a panic with a clear
//! message is how a test reports a failure.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod bytes;
pub mod vectors;

pub use bytes::{assert_bytes_eq, from_hex, to_hex};
