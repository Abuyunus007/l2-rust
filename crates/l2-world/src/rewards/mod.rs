//! What a kill gives: exp and SP (`exp.rs`), drops (`drop.rs`, M4-DROP-01), and the vitality and
//! Nevit bonuses on the exp (`vitality.rs`, `nevit.rs`, M4-VIT-01).
//! Design: `docs/design/m4-combat.md`.
// Java: entity/actor/Attackable.java (calculateRewards), entity/actor/templates/NpcTemplate.java
//       (calculateDrops)

pub mod drop;
pub mod exp;
pub mod nevit;
pub mod vitality;
