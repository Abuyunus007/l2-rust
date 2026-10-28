//! Small `pub` entry points to NPC behaviour that the scripts of `ai/areas/Gracia` need and the
//! `pub` world API did not offer (S-AI-01). Each is a thin call into the module that owns the rule.
// Java: entity/actor/Creature.java (setRunning, disableCoreAI), entity/actor/Npc.java
//       (setRandomWalking, setTitle), entity/actor/instance/Attackable.java
//       (setCanReturnToSpawnPoint), ai/AbstractAI.java (setIntentionFollow, stopFollow)

use l2_data::stats::AbnormalType;
use l2_util::ObjectId;

use crate::effects::SkillFinishType;
use crate::interact::NORMAL_FOLLOW_RANGE;
use crate::world::World;

impl World {
    /// Java `Creature.setRunning()` / `setWalking()` of an NPC.
    pub fn script_set_running(&mut self, npc: ObjectId, value: bool) {
        self.set_running(npc, value);
    }

    /// Java `WorldObject.setHeading(heading)` of an NPC.
    pub fn script_set_heading(&mut self, npc: ObjectId, heading: i32) {
        self.set_heading(npc, heading);
    }

    /// Java `Creature.disableCoreAI(value)`.
    pub fn script_disable_core_ai(&mut self, npc: ObjectId, value: bool) {
        self.disable_core_ai(npc, value);
    }

    /// Java `Npc.setRandomWalking(value)`; a value `true` goes back to the template's rule.
    pub fn script_set_random_walking(&mut self, npc: ObjectId, value: bool) {
        if value {
            self.npc_world.script_no_random_walk.remove(&npc);
        } else {
            self.npc_world.script_no_random_walk.insert(npc);
        }
    }

    /// Java `Attackable.setCanReturnToSpawnPoint(value)`.
    pub fn script_set_can_return_to_spawn_point(&mut self, npc: ObjectId, value: bool) {
        if value {
            self.npc_world.script_no_return_home.remove(&npc);
        } else {
            self.npc_world.script_no_return_home.insert(npc);
        }
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
