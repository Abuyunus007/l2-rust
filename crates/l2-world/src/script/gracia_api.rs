//! Small `pub` entry points to NPC behaviour that the scripts of `ai/areas/Gracia` need and the
//! `pub` world API did not offer (S-AI-01). Each is a thin call into the module that owns the rule.
// Java: entity/actor/Npc.java (setTitle), ai/AbstractAI.java (setIntentionFollow, stopFollow),
//       mechanics/skill/BuffInfo.java, entity/instancezone/Instance.java

use l2_data::stats::AbnormalType;
use l2_util::ObjectId;

use crate::effects::SkillFinishType;
use crate::interact::NORMAL_FOLLOW_RANGE;
use crate::world::World;

impl World {
    /// Java `GeneralConfig.INSTANCE_FINISH_TIME` (`DefaultFinishTime`), in milliseconds.
    pub fn script_instance_finish_time_ms(&self) -> i32 {
        self.instances.settings.instance_finish_time_ms
    }

    /// Java `instance.getAliveNpcs(Monster.class)`: the monsters of an instance that are not dead.
    pub fn script_alive_monsters_of_instance(&self, instance_id: i32) -> Vec<ObjectId> {
        let Some(instance) = self.instance(instance_id) else {
            return Vec::new();
        };
        instance
            .npcs
            .keys()
            .copied()
            .filter(|id| self.npc(*id).is_some_and(|n| n.class.is_monster()) && !self.is_dead(*id))
            .collect()
    }

    /// Java `Player.isFlyingMounted()`.
    pub fn script_is_flying_mounted(&self, player: ObjectId) -> bool {
        self.stats(player).is_some_and(|view| view.is_flying_mounted())
    }

    /// Java `getAI().setIntentionFollow(target)` of an NPC.
    pub fn script_start_follow(&mut self, npc: ObjectId, target: ObjectId) {
        self.start_follow(npc, target, NORMAL_FOLLOW_RANGE);
    }

    /// Java `getAI().stopFollow()`.
    pub fn script_stop_follow(&mut self, npc: ObjectId) {
        self.stop_follow(npc);
    }

    /// Java `getEffectList().getBuffInfoByAbnormalType(type)` then `getSkill().getAbnormalLevel()`:
    /// the abnormal level of the buff of that abnormal type (a Java constant such as
    /// `"SOA_BUFF1"`), `None` without one (or for an unknown name).
    pub fn script_buff_abnormal_level(&self, owner: ObjectId, abnormal_type: &str) -> Option<i32> {
        let abnormal_type = AbnormalType::from_name(abnormal_type)?;
        self.effect_list(owner)?
            .info_by_abnormal_type(abnormal_type)
            .map(|info| info.facts.abnormal_level)
    }

    /// Java `getEffectList().stopSkillEffects(SkillFinishType.REMOVED, abnormalType)`.
    pub fn script_stop_buffs_by_abnormal(&mut self, owner: ObjectId, abnormal_type: &str) {
        if let Some(abnormal_type) = AbnormalType::from_name(abnormal_type) {
            self.stop_skill_effects_by_abnormal(owner, SkillFinishType::Removed, abnormal_type);
        }
    }

    /// Java `Player.isInventoryUnder90(includeQuestInv)`.
    pub fn script_is_inventory_under_90(&self, player: ObjectId, include_quest: bool) -> bool {
        self.is_inventory_under_90(player, include_quest)
    }

    /// Java `Npc.setTitle(title)`, shown to those who see the NPC again.
    pub fn script_set_npc_title(&mut self, npc: ObjectId, title: &str) {
        self.sieges.npc_titles.insert(npc, title.to_owned());
        self.broadcast_info(npc);
    }
}

// S-AI-07: the Gracia instances.
impl World {
    /// Java `skill.hasEffectType(types...)` of a skill by id and level (false for an unknown
    /// skill).
    pub fn script_skill_has_effect_type(
        &mut self,
        skill: crate::effects::SkillKey,
        types: &[crate::effects::registry::EffectType],
    ) -> bool {
        let Some(table) = self.skill_table() else {
            return false;
        };
        let Some(template) = table.get(skill.id, skill.level) else {
            return false;
        };
        self.skill_has_effect_type(template, types)
    }

    /// Java `skill.getEffectPoint()` (0 for an unknown skill).
    pub fn script_skill_effect_point(&self, skill: crate::effects::SkillKey) -> i32 {
        self.skill_table()
            .and_then(|t| t.get(skill.id, skill.level).map(|s| s.effect_point))
            .unwrap_or(0)
    }
}
