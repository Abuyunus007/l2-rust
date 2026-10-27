//! What a kill gives: exp and SP (`exp.rs`), drops (`drop.rs`, M4-DROP-01), and the vitality and
//! Nevit bonuses on the exp (`vitality.rs`, `nevit.rs`, M4-VIT-01), the recommendations and the
//! daily reset (`recommendations.rs`, M5-FU-03).
//! Design: `docs/design/m4-combat.md`.
// Java: entity/actor/Attackable.java (calculateRewards), entity/actor/templates/NpcTemplate.java
//       (calculateDrops)

pub mod drop;
pub mod event_drop;
pub mod exp;
pub mod nevit;
pub mod recommendations;
pub mod vitality;
