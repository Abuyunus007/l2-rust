//! The Java custom features of `config/Custom/*.ini` (D-57), one file per feature. Each is off by
//! default, as in Java, and switched by the keys of its own ini file.
//!
//! M9-C07: the switches of the custom scripts (DelevelManager, NoblessMaster, Transmog, the
//! custom events), `//event_start` and friends, the transmog appearance collection.

pub mod events;
pub mod script_features;
pub mod transmog;

pub use script_features::{
    CustomScriptSettings, CustomScriptWorld, DelevelManagerSettings, NoblessMasterSettings, TransmogSettings,
};
