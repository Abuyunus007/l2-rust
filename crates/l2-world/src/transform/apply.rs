//! Turning a player into a form and back: `Player.transform`, `untransform`,
//! `Creature.stopTransformation`, `Transform.onTransform` / `onUntransform` / `onLevelUp` and the
//! transformation part of `Player.sendSkillList`.
// Java: entity/actor/Player.java (transform, untransform, sendSkillList, addTransformSkill,
//       removeAllTransformSkills), entity/actor/Creature.java (stopTransformation),
//       entity/actor/transform/Transform.java (onTransform, onUntransform, onLevelUp),
//       data/xml/TransformData.java (transformPlayer)

use std::sync::Arc;

use l2_data::stats::AbnormalType;
use l2_data::{Transform, Transforms};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ExBasicActionList, SystemMessage};
use l2_util::ObjectId;

use crate::effects::SkillFinishType;
use crate::world::World;
use crate::zones::ZoneId;

/// Java `World.GRACIA_MAX_X`: a flying form does not leave the Gracia area.
pub const GRACIA_MAX_X: i32 = -166_168;

impl World {
    /// The transformations of the datapack (a cheap clone of the shared table, so the caller can
    /// change the world while it holds a form).
    pub(crate) fn transform_table(&self) -> Option<Arc<Transforms>> {
        self.data.transforms().map(Arc::clone)
    }

    /// Java `TransformData.transformPlayer(id, player)`: `false` for an id without a form.
    pub fn transform_player(&mut self, transform_id: i32, player: ObjectId) -> bool {
        let Some(table) = self.transform_table() else {
            return false;
        };
        let Some(form) = table.get(transform_id) else {
            return false;
        };
        self.transform(player, form);
        true
    }

    /// Java `Player.transform(transformation)`.
    fn transform(&mut self, id: ObjectId, form: &Transform) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        if form.is_flying() && player.core.x > GRACIA_MAX_X {
            return;
        }
        // `FeatureConfig.ALLOW_MOUNTS_DURING_SIEGE` (`AllowRideMountsDuringSiege`, M7-SIEGE-01).
        let allow_mounts = self.siege_settings().allow_mounts_during_siege;
        if !allow_mounts && form.is_riding() && player.zones.is_inside(ZoneId::Siege) {
            return;
        }
        if player.transform.transformation.is_some() {
            // You already polymorphed and cannot polymorph again.
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAlreadyPolymorphedAndCannotPolymorphAgain),
            );
            return;
        }

        self.set_queued_skill(id, None);
        if self.is_mounted(id) {
            // Get off the strider or something else if character is mounted.
            self.dismount(id);
        }
        if let Some(player) = self.players.get_mut(&id) {
            player.transform.transformation = Some(form.id);
        }
        self.stop_all_toggles(id, true);
        self.on_transform(id, form);
        self.send_skill_list(id);
        self.send(id, &self.skill_cool_time(id));
        self.broadcast_user_info(id);
        // The ON_PLAYER_TRANSFORM script event is D-05.
    }

    /// Java `Player.untransform()`.
    pub fn untransform(&mut self, id: ObjectId) {
        let Some(form_id) = self.players.get(&id).and_then(|p| p.transform.transformation) else {
            return;
        };
        self.set_queued_skill(id, None);
        if let Some(table) = self.transform_table()
            && let Some(form) = table.get(form_id)
        {
            self.on_untransform(id, form);
        }
        if let Some(player) = self.players.get_mut(&id) {
            player.transform.transformation = None;
        }
        self.stop_skill_effects_by_abnormal(id, SkillFinishType::Normal, AbnormalType::Transform);
        self.send_skill_list(id);
        self.send(id, &self.skill_cool_time(id));
        self.broadcast_user_info(id);
    }

    /// Java `Creature.stopTransformation(removeEffects)`: with `remove_effects` the transformation
    /// buff goes first (its `onExit` comes back here with `false`).
    pub fn stop_transformation(&mut self, id: ObjectId, remove_effects: bool) {
        if remove_effects {
            self.stop_skill_effects_by_abnormal(id, SkillFinishType::Normal, AbnormalType::Transform);
        }
        // If this is a player instance, then untransform.
        if self
            .players
            .get(&id)
            .is_some_and(|p| p.transform.transformation.is_some())
        {
            self.untransform(id);
        }
        // `getAI().notifyActionThink()` is for NPCs: they do not transform.
        self.update_abnormal_effect(id);
    }

    /// Java `Transform.onTransform(player)`.
    fn on_transform(&mut self, id: ObjectId, form: &Transform) {
        let Some(female) = self.players.get(&id).map(|p| p.data.sex != 0) else {
            return;
        };
        let Some(template) = form.template(female) else {
            return;
        };
        if let Some(player) = self.players.get_mut(&id) {
            // Start flying.
            if form.is_flying() {
                player.transform.flying = true;
            }
            if let Some(name) = &form.name {
                player.transform.visible_name = Some(name.clone());
            }
            if let Some(title) = &form.title {
                player.transform.visible_title = Some(title.clone());
            }
        }

        // Set inventory blocks if needed.
        if !template.additional_items.is_empty() {
            let allowed: Vec<i32> = template
                .additional_items
                .iter()
                .filter(|i| i.allowed)
                .map(|i| i.id)
                .collect();
            let not_allowed: Vec<i32> = template
                .additional_items
                .iter()
                .filter(|i| !i.allowed)
                .map(|i| i.id)
                .collect();
            if !allowed.is_empty() {
                self.set_inventory_block(id, allowed, 1);
            }
            // NOTE(java-quirk): a form with both lists keeps the second block only.
            if !not_allowed.is_empty() {
                self.set_inventory_block(id, not_allowed, 2);
            }
        }

        // Send basic action list.
        if let Some(actions) = &template.actions {
            self.send(
                id,
                &ExBasicActionList {
                    action_ids: actions.clone(),
                },
            );
        }
    }

    /// Java `Transform.onUntransform(player)`.
    fn on_untransform(&mut self, id: ObjectId, form: &Transform) {
        let Some(female) = self.players.get(&id).map(|p| p.data.sex != 0) else {
            return;
        };
        let Some(template) = form.template(female) else {
            return;
        };
        let Some(level) = self.core_of(id).map(|c| c.level) else {
            return;
        };
        if let Some(player) = self.players.get_mut(&id) {
            // Stop flying.
            if form.is_flying() {
                player.transform.flying = false;
            }
            if form.name.is_some() {
                player.transform.visible_name = None;
            }
            if form.title.is_some() {
                player.transform.visible_title = None;
            }
        }

        // Remove common skills.
        for holder in &template.skills {
            self.remove_transform_skill_if_not_allowed(id, holder.id, holder.level);
        }
        // Remove skills depending on level.
        for holder in &template.additional_skills {
            if level >= holder.min_level {
                self.remove_transform_skill_if_not_allowed(id, holder.id, holder.level);
            }
        }
        // Remove transformation skills.
        if let Some(player) = self.players.get_mut(&id) {
            player.transform.skills.clear();
        }

        // Remove inventory blocks if needed.
        if !template.additional_items.is_empty() {
            self.unblock_inventory(id);
        }
        self.send(id, &ExBasicActionList::default_list());
    }

    /// `if (!SkillTreeData.isSkillAllowed(player, skill)) player.removeSkill(skill, false,
    /// skill.isPassive())`.
    fn remove_transform_skill_if_not_allowed(&mut self, id: ObjectId, skill_id: i32, level: i32) {
        // `holder.getSkill()` is null for a skill the datapack lacks, which Java would trip over.
        let Some(passive) = self
            .data
            .stat_data()
            .and_then(|d| d.skills.get(skill_id, level))
            .map(l2_data::SkillTemplate::is_passive)
        else {
            return;
        };
        if !self.is_skill_allowed(id, skill_id, level) {
            self.remove_skill(id, skill_id, passive);
        }
    }

    /// Java `PlayerStat.addLevel`: `if (isTransformed() || isInStance()) transformation.onLevelUp(player)`,
    /// which is `Transform.onLevelUp`: the additional skills the new level brings.
    pub(crate) fn transformation_level_up(&mut self, id: ObjectId) {
        let Some(form_id) = self.players.get(&id).and_then(|p| p.transform.transformation) else {
            return;
        };
        let Some(table) = self.transform_table() else {
            return;
        };
        let (Some(form), Some(female), Some(level)) = (
            table.get(form_id),
            self.players.get(&id).map(|p| p.data.sex != 0),
            self.core_of(id).map(|c| c.level),
        ) else {
            return;
        };
        let Some(template) = form.template(female) else {
            return;
        };
        // Add skills depending on level.
        for holder in &template.additional_skills {
            let known = self
                .core_of(id)
                .and_then(|c| c.skills.iter().find(|(s, _)| *s == holder.id))
                .map_or(0, |&(_, l)| l);
            if level >= holder.min_level
                && known < holder.level
                && self.add_skill(id, holder.id, holder.level)
            {
                if let Some(player) = self.players.get_mut(&id) {
                    player.transform.skills.insert(holder.id, holder.level);
                }
            }
        }
    }

    /// The transformation part of `Player.sendSkillList`'s task: the skills the form gives, by
    /// skill id, as `(id, level)`. They become the player's transformation skills
    /// (`addTransformSkill`). Empty when the player is not transformed.
    pub(crate) fn transformation_skill_list(&mut self, id: ObjectId) -> Vec<(i32, i32)> {
        let Some(form_id) = self.players.get(&id).and_then(|p| p.transform.transformation) else {
            return Vec::new();
        };
        let Some(table) = self.transform_table() else {
            return Vec::new();
        };
        let (Some(form), Some(female), Some(level)) = (
            table.get(form_id),
            self.players.get(&id).map(|p| p.data.sex != 0),
            self.core_of(id).map(|c| c.level),
        ) else {
            return Vec::new();
        };
        let Some(template) = form.template(female) else {
            return Vec::new();
        };
        let mut skills: std::collections::BTreeMap<i32, i32> = std::collections::BTreeMap::new();
        let mut put = |skill_id: i32, skill_level: i32| {
            let entry = skills.entry(skill_id).or_insert(skill_level);
            if *entry < skill_level {
                *entry = skill_level;
            }
        };
        for holder in &template.skills {
            put(holder.id, holder.level);
        }
        for holder in &template.additional_skills {
            if level >= holder.min_level {
                put(holder.id, holder.level);
            }
        }
        // FOLLOW-UP(flying): a flying form adds the skills of the collect skill tree the player
        // knows (`SkillTreeData.getCollectSkillTree`); that tree is not loaded.
        let Some(data) = self.data.stat_data() else {
            return Vec::new();
        };
        // `SkillData.getSkill(id, level)`: a skill the datapack lacks would stop Java's task with a
        // null pointer; it is left out here.
        let entries: Vec<(i32, i32)> = skills
            .into_iter()
            .filter(|&(skill_id, skill_level)| data.skills.get(skill_id, skill_level).is_some())
            .collect();
        if let Some(player) = self.players.get_mut(&id) {
            for &(skill_id, skill_level) in &entries {
                player.transform.skills.insert(skill_id, skill_level);
            }
        }
        entries
    }

    /// Java `Player.getTransformSkill(id)` as a skill key.
    pub fn transform_skill(&self, id: ObjectId, skill_id: i32) -> Option<crate::effects::SkillKey> {
        self.players
            .get(&id)?
            .transform
            .skills
            .get(&skill_id)
            .map(|&level| crate::effects::SkillKey::new(skill_id, level))
    }

    /// Java `Player.hasTransformSkill(id)`.
    pub fn has_transform_skill(&self, id: ObjectId, skill_id: i32) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| p.transform.skills.contains_key(&skill_id))
    }

    /// Java `WaterZone.onEnter` for a transformed player: a form that cannot swim ends, any other
    /// player gets its user info (the speeds change in water).
    // FOLLOW-UP(water): `WaterZone` is inert in the zone code (no player is "in water" yet); the
    // water card calls this from its `onEnter`.
    pub fn transformed_player_entered_water(&mut self, id: ObjectId) {
        let can_swim = self
            .stats(id)
            .and_then(|v| v.transformation().map(|t| t.can_swim));
        match can_swim {
            Some(false) if self.is_transformed(id) => self.stop_transformation(id, true),
            _ => self.broadcast_user_info(id),
        }
    }
}
