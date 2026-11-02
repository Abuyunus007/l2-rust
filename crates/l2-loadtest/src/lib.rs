//! `l2-loadtest`: logs in N bot clients through the real login server and game server, lets them
//! play a simple loop and measures the server (M9-H02).
//!
//! - [`options`]  the command line, and the refusal to hit the real login port.
//! - [`stats`]    counters and timings the bots fill in.
//! - [`bot`]      one bot: login, character, enter world, the play loop.
//! - [`sampler`]  every few seconds: our counters plus the game server's admin `GameInfo`.
//! - [`report`]   the summary table and the CSV of the samples.
//! - [`run`]      ramps the bots up and collects the result.
//!
//! The bots use `l2_testkit`'s fake clients, which panic on an unexpected packet (they were made
//! for tests). Each bot runs in its own task, so a panic ends that bot only and is counted as an
//! error with its message.

pub mod bot;
pub mod options;
pub mod report;
pub mod run;
pub mod sampler;
pub mod stats;
