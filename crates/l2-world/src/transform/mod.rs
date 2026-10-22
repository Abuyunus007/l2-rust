//! Transformations (M6-TRANS-01): the `Transformation` effect turns a player into a form of the
//! datapack (`l2_data::transform`), with the form's stats, skills, action list and item rules.
//! Design: `docs/design/m6-social.md`.
// Java: entity/actor/Player.java (transform, untransform, sendSkillList, getTransformation...),
//       entity/actor/Creature.java (stopTransformation), entity/actor/transform/Transform.java
//       (onTransform, onUntransform, onLevelUp), data/xml/TransformData.java,
//       entity/itemcontainer/PlayerInventory.java (the inventory block),
//       network/serverpackets/ExBasicActionList.java (ACTIONS_ON_TRANSFORM)
//
// # How it fits together
// - The state of a transformed player is [`TransformState`] in `Player::transform`: the id of the
//   transformation (Java `_transformation`), the transformation skills the skill list made
//   (`_transformSkills`), the visible name and title, the flying flag and the inventory block.
//   The transformation itself is data (`StaticData::transforms`), found by id.
// - `apply.rs`: [`World::transform_player`] (`TransformData.transformPlayer`), `untransform`,
//   `stop_transformation`, the level-up hook and the transformation skill list.
// - `stats.rs`: the answers the stat engine needs (`StatView::transformation`, the
//   transformation's stat, level modifier, defence per slot). The calls sit in `stats/`: the
//   transformation's value replaces the start value of a stat (`calc_stat`), the attack range,
//   the base speeds, the level modifier, the attack type and the collision come from the
//   template, `FuncPDefMod`/`FuncMDefMod` use the template's defence.
// - `rules.rs`: the action list a form allows, the inventory block (the items a form may or may
//   not use) and the conditions `canTransform` / `canUntransform`.
// - `effect.rs`: the `Transformation` effect handler. A form ends with its buff: death, the
//   buff running out, `TransformDispel` (skill 619, a `DispelBySlot` of `TRANSFORM,-1`) all end
//   the effect, whose `onExit` untransforms. The buff is saved with the others
//   (`character_skills_save`) and comes back after a relog, which transforms the player again.
//
// Not here: the flying movement rules (geodata, falling) wait for the mounts card; the collect
// skill tree a flying form adds to the skill list is not loaded; the cursed weapon forms (301, 302)
// work as forms, equipping the weapon is the cursed weapons' (M8); the admin `//transform`
// commands are the admin cards'.

mod apply;
pub mod effect;
mod rules;
mod stats;

use std::collections::BTreeMap;

pub(crate) use apply::ALLOW_MOUNTS_DURING_SIEGE;
pub use apply::GRACIA_MAX_X;
pub use rules::ACTIONS_ON_TRANSFORM;
pub(crate) use rules::{can_transform, can_untransform};

/// Java `PlayerInventory._blockMode` / `_blockItems`: which items the client may use.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryBlock {
    /// `_blockMode`: 0 blocks the listed items, 1 allows only the listed items, 2 is what a
    /// form's "not allowed" list sets (the client greys them out; the server does not check).
    pub mode: i32,
    /// `_blockItems`: item template ids.
    pub items: Vec<i32>,
}

impl InventoryBlock {
    /// Java `PlayerInventory.hasInventoryBlock()`.
    pub fn is_active(&self) -> bool {
        self.mode > -1 && !self.items.is_empty()
    }

    /// Java `PlayerInventory.canManipulateWithItemId(itemId)`.
    pub fn can_manipulate(&self, item_id: i32) -> bool {
        let listed = self.items.contains(&item_id);
        (self.mode != 0 || !listed) && (self.mode != 1 || listed)
    }
}

/// The transformation of one player.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct TransformState {
    /// Java `_transformation`: the id of the form the player is in (a stance included).
    pub transformation: Option<i32>,
    /// Java `_transformSkills`: skill id to level, filled by the skill list.
    pub skills: BTreeMap<i32, i32>,
    /// Java `PlayerAppearance._visibleName` (`None` shows the real name).
    pub visible_name: Option<String>,
    /// Java `PlayerAppearance._visibleTitle` (`None` shows the real title).
    pub visible_title: Option<String>,
    /// Java `Creature._isFlying`, as a flying form sets it.
    pub flying: bool,
    /// Java `PlayerInventory` block (`None`: `_blockMode == -1`).
    pub block: Option<InventoryBlock>,
}
