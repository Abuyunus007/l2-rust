//! Skills in the world: who a skill hits (M5-TARGET-01), learning skills at a trainer
//! (M5-LEARN-01), and how it is cast (M5-CAST-01).
//!
//! `targets` holds Java's target type handlers (`dist/game/data/scripts/handlers/skill/targets`)
//! behind one function, [`World::skill_target_list`]. `cast` is the cast itself (the one entry
//! point of players, items and NPC AI), `checks` what must hold before it, `reuse` the reuse
//! stamps and `SkillCoolTime`.

pub mod cast;
pub mod checks;
pub mod learn;
pub mod reuse;
pub mod targets;

pub use cast::{CastSettings, CastState, CastTimer, MagicUseTask, SkillUse};
pub use reuse::ReuseTimeStamp;

pub use targets::{
    HANDLED_TARGET_TYPES, NOT_HANDLED_TEXT, TargetContext, TargetMessage, TargetResult,
    is_target_type_handled,
};
