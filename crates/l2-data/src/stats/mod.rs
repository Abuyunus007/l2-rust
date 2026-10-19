//! The data model of Java's stat system, parse only: the `Stat` enum, function templates and
//! condition trees. Nothing here is evaluated; `l2-world` (M4-STAT-01) turns these into
//! calculators and tests conditions against the world.
//!
//! Java: `mechanics/stats/Stat.java`, `mechanics/stats/functions/{FuncTemplate,StatFunction}.java`,
//! `mechanics/conditions/*`, and the parsing half of `util/DocumentBase.java`.
//! Design: `docs/design/m4-combat.md`.
//!
//! # Pieces
//! - [`stat`]: [`Stat`] (127 constants, XML names) and [`NUM_STATS`].
//! - [`func`]: [`FuncKind`], [`FuncTemplate`] and the function parsers (`attachFunc`,
//!   `processAlternativeFuncFormat`, the `parseTemplate` function switch).
//! - [`condition`]: [`Condition`], [`ConditionParser`] (the `parseCondition` family) and the
//!   `<conditions msg msgId addName>` wrapper [`MessagedCondition`].
//! - [`enums`]: the name-only enums conditions refer to (`AbnormalType`, `CategoryType`,
//!   `InstanceType`, `PlayerState`).
//! - [`node`]: [`XmlNode`], the ordered element tree the parsers walk (serde can fill one from
//!   any element, see its docs).
//! - [`jnum`]: Java's `Integer.decode`, `Double.parseDouble`, `String.split` semantics.
//!
//! # Reusing the parsers (skills, M4-DATA-11)
//! A loader keeps the `<stats>`/`<conditions>`/effect subtree as [`XmlNode`] fields in its raw
//! serde structs, then builds a [`ConditionParser`] with a `#table` resolver and calls
//! [`ConditionParser::parse_condition`] / [`ConditionParser::parse_conditions_block`] and
//! [`func::parse_func_element`]. Item templates do exactly this in `item/funcs.rs`.

pub mod condition;
pub mod enums;
pub mod func;
pub mod jnum;
pub mod node;
pub mod stat;

pub use condition::{
    Condition, ConditionMessage, ConditionParser, MessagedCondition, ParseError, TableResolver,
};
pub use enums::{AbnormalType, CategoryType, InstanceType, PlayerState};
pub use func::{FuncKind, FuncTemplate};
pub use node::XmlNode;
pub use stat::{NUM_STATS, Stat};
