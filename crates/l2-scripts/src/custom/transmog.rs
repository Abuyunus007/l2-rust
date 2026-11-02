//! Transmog (placeholder).
use l2_world::{Script, ScriptSetup};

/// Java `custom.Transmog.Transmog`.
pub struct Transmog;

impl Script for Transmog {
    fn name(&self) -> &'static str {
        "Transmog"
    }

    fn path(&self) -> &'static str {
        "custom/Transmog"
    }

    fn setup(&self, _s: &mut ScriptSetup) {}
}
