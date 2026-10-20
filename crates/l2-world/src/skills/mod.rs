//! Skills in the world: who a skill hits (M5-TARGET-01), learning skills at a trainer
//! (M5-LEARN-01), and later how it is cast (M5-CAST-01).
//!
//! `targets` holds Java's target type handlers (`dist/game/data/scripts/handlers/skill/targets`)
//! behind one function, [`World::skill_target_list`]. Casting calls it; the casting card adds
//! its own files next to `targets.rs`.

pub mod learn;
pub mod targets;

pub use targets::{
    HANDLED_TARGET_TYPES, NOT_HANDLED_TEXT, TargetContext, TargetMessage, TargetResult,
    is_target_type_handled,
};
