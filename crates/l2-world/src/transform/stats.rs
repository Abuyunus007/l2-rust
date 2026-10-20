//! What the stat engine asks a transformed player: the transformation, its template, the
//! base values it replaces.
// Java: entity/actor/transform/Transform.java (getStat, getBaseDefBySlot, getLevelMod,
//       getBaseAttackRange, getCollisionRadius/Height), entity/actor/transform/TransformTemplate.java
// Java: entity/actor/Player.java (isTransformed, isInStance, getTransformationId,
//       getTransformationDisplayId, isFlying, isFlyingMounted)
//
// The calls sit where Java calls them (`creature_stat.rs`, `std_funcs.rs`, `conditions.rs`); this
// file only answers.

use l2_data::{Stat, Transform, TransformTemplate};

use crate::stats::StatView;
use crate::stats::creature_stat::MoveType;

impl<'w> StatView<'w> {
    /// Java `getTransformation()`: the player's transformation, a stance included. NPCs have none.
    pub fn transformation(&self) -> Option<&'w Transform> {
        let id = self.player()?.transform.transformation?;
        self.world().data.transforms()?.get(id)
    }

    /// Java `Transform.getTemplate(player)`: the template of the player's sex.
    pub fn transform_template(&self) -> Option<&'w TransformTemplate> {
        let female = self.player()?.data.sex != 0;
        self.transformation()?.template(female)
    }

    /// Java `isTransformed()`: transformed, and not into a stance.
    pub fn is_transformed(&self) -> bool {
        self.transformation().is_some_and(|t| !t.is_stance())
    }

    /// Java `isInStance()`.
    pub fn is_in_stance(&self) -> bool {
        self.transformation().is_some_and(Transform::is_stance)
    }

    /// Java `getTransformationId()`: 0 unless transformed (a stance counts as none).
    pub fn transformation_id(&self) -> i32 {
        self.transformation()
            .filter(|t| !t.is_stance())
            .map_or(0, |t| t.id)
    }

    /// Java `getTransformationDisplayId()`.
    pub fn transformation_display_id(&self) -> i32 {
        self.transformation()
            .filter(|t| !t.is_stance())
            .map_or(0, |t| t.display_id)
    }

    /// Java `isFlying()` (`_isFlying`): set while a flying transformation lasts. Wyverns and
    /// airships set it too, once they exist.
    pub fn is_flying(&self) -> bool {
        self.player().is_some_and(|p| p.transform.flying)
    }

    /// Java `Player.isFlyingMounted()`: `isTransformed() && _transformation.isFlying()`.
    pub fn is_flying_mounted(&self) -> bool {
        self.is_transformed() && self.transformation().is_some_and(Transform::is_flying)
    }

    /// Java `Transform.getStat(player, stat)`: the template's value, replaced by the level
    /// data's when there is one for the player's level (0 for a stat the level data lacks).
    pub(crate) fn transform_stat(&self, stat: Stat) -> f64 {
        let Some(template) = self.transform_template() else {
            return 0.0;
        };
        match template.level_data(self.level()) {
            Some(data) => data.stat(stat),
            None => template.stat(stat),
        }
    }

    /// Java `Transform.getLevelMod(player)`: `-1` without level data.
    pub(crate) fn transform_level_mod(&self) -> f64 {
        self.transform_template()
            .and_then(|t| t.level_data(self.level()))
            .map_or(-1.0, |d| d.level_mod)
    }

    /// Java `TransformTemplate.getBaseMoveSpeed(type)`.
    pub(crate) fn transform_move_speed(template: &TransformTemplate, kind: MoveType) -> f64 {
        let speeds = &template.speeds;
        match kind {
            MoveType::Run => speeds.run,
            MoveType::Walk => speeds.walk,
            MoveType::FastSwim => speeds.fast_swim,
            MoveType::SlowSwim => speeds.slow_swim,
            MoveType::FastFly => speeds.fast_fly,
            MoveType::SlowFly => speeds.slow_fly,
        }
    }

    /// Java `Transform.getBaseDefBySlot(player, slot)` for a transformed player: the template's
    /// defence of the slot, else the class template's.
    pub(crate) fn transform_def_by_slot(&self, slot: usize) -> i32 {
        match self.transform_template() {
            Some(template) => template.defense(slot),
            None => self.player_template().map_or(0, |t| t.base_def_by_slot(slot)),
        }
    }

    /// The key `FuncPDefMod` / `FuncMDefMod` hand to `getBaseDefBySlot`:
    /// `p.getTemplate().getBaseDefBySlot(p.isTransformed() ? transform.getBaseDefBySlot(p, slot) : slot)`.
    // NOTE(java-quirk): while transformed, the *transformation's defence value* is used as the
    // slot number of the class template (a value of 25 or more finds nothing and subtracts 0).
    // Only the chest is subtracted as a value (see `p_def_mod`).
    pub(crate) fn def_mod_slot_key(&self, slot: usize) -> usize {
        if self.is_transformed() {
            usize::try_from(self.transform_def_by_slot(slot)).unwrap_or(usize::MAX)
        } else {
            slot
        }
    }
}
