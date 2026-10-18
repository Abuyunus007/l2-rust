//! Shared basics used by every other crate: object ids, the seedable world RNG and time helpers.

pub mod ids;
pub mod rng;
pub mod time;

pub use ids::ObjectId;
pub use rng::WorldRng;
