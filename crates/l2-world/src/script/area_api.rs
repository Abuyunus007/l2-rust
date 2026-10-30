//! Small `pub` entry points to world behaviour that the scripts of `ai/areas` (part 2) need and
//! the `pub` world API did not offer (S-AI-04). Each is a thin call into the module that owns the
//! rule.
// Java: skills/Skill.java (isMagic, getMpConsume, getHpConsume), entity/actor/Npc.java
//       (getCurrentMp), entity/teleporter/TeleportHolder.java (showTeleportList),
//       entity/spawns/Spawn.java (decreaseCount), entity/siege/Siege.java

use l2_util::ObjectId;

use crate::effects::SkillKey;
use crate::residence::CastleId;
use crate::world::World;

impl World {
    /// Java `skill.isMagic()` of a skill a script names by id and level; `false` for a skill the
    /// table does not have.
    pub fn script_skill_is_magic(&self, key: SkillKey) -> bool {
        self.skill_table()
            .and_then(|table| table.get(key.id, key.level).map(|s| s.is_magic()))
            .unwrap_or(false)
    }

    /// Java `(skill.getMpConsume(), skill.getHpConsume())`; `None` for a skill the table does not
    /// have.
    pub fn script_skill_mp_hp_consume(&self, key: SkillKey) -> Option<(i32, i32)> {
        let table = self.skill_table()?;
        let skill = table.get(key.id, key.level)?;
        Some((skill.mp_consume, skill.hp_consume))
    }

    /// Java `npc.getCurrentMp()`.
    pub fn script_npc_current_mp(&self, npc: ObjectId) -> f64 {
        self.npc(npc).map_or(0.0, |n| n.core.cur_mp)
    }

    /// Java `TeleporterData.getHolder(npcId, listName).showTeleportList(player, npc)`; `false`
    /// when the NPC has no such list.
    pub fn script_show_teleport_list(&mut self, npc: ObjectId, player: ObjectId, list_name: &str) -> bool {
        let npc_id = self.npc(npc).map_or(0, |n| n.npc_id);
        let data = self.talk.teleporters.clone();
        let Some(holder) = data.as_ref().and_then(|d| d.get_holder(npc_id, list_name)) else {
            return false;
        };
        let bypass = format!("npc_{}_teleport", npc.0);
        self.show_teleport_list_with(npc, player, holder, &bypass);
        true
    }

    /// Java `npc.getSpawn().decreaseCount(npc)` (a script calls it before `deleteMe`, which
    /// decreases again).
    pub fn script_spawn_decrease_count(&mut self, npc: ObjectId) {
        let Some((index, enchant)) = self.npc(npc).and_then(|n| n.spawn.map(|s| (s, n.enchant_effect)))
        else {
            return;
        };
        self.decrease_count(index, npc, enchant);
    }

    /// `(castle.getSiegeDate().getTimeInMillis(), System.currentTimeMillis())` for the siege
    /// clock; `None` for an unknown castle.
    pub fn script_castle_siege_clock(&self, castle: i32) -> Option<(i64, i64)> {
        let date = self.castle(CastleId(castle))?.siege_date;
        Some((date, self.siege_now_ms()))
    }

    /// Java `npc.getTemplate().canTeach(player.getPlayerClass())` (HealerTrainer; S-AI-08).
    pub fn script_npc_can_teach(&self, npc_template_id: i32, player: ObjectId) -> bool {
        self.player(player).is_some_and(|p| {
            u16::try_from(p.data.class_id)
                .is_ok_and(|class| self.can_teach(npc_template_id, l2_data::ClassId(class)))
        })
    }

    /// Java `castle.getSiege().getControlTowerCount()`.
    pub fn script_castle_control_tower_count(&self, castle: i32) -> i32 {
        self.castle_siege(CastleId(castle))
            .map_or(0, |s| s.control_tower_count)
    }

    /// Java `castle.getSiege().isInProgress()`.
    pub fn script_castle_siege_in_progress(&self, castle: i32) -> bool {
        self.is_castle_siege_in_progress(CastleId(castle))
    }

    /// Java `creature.enableSkill(skill)` of a skill a script names by id and level.
    pub fn script_enable_skill(&mut self, id: ObjectId, key: SkillKey) {
        if let Some(table) = self.skill_table()
            && let Some(skill) = table.get(key.id, key.level)
        {
            self.enable_skill(id, skill);
        }
    }

    /// Java `creature.disableSkill(skill, delay)` of a skill a script names by id and level (a
    /// delay of 0 or less disables it until it is enabled again).
    pub fn script_disable_skill(&mut self, id: ObjectId, key: SkillKey, delay_ms: i64) {
        if let Some(table) = self.skill_table()
            && let Some(skill) = table.get(key.id, key.level)
        {
            self.disable_skill(id, skill, delay_ms);
        }
    }

    /// Java `creature.getEffectList().getBuffInfoBySkillId(skillId).getSkill().getLevel()`;
    /// `None` when the creature has no such effect.
    pub fn script_buff_skill_level(&self, owner: ObjectId, skill_id: i32) -> Option<i32> {
        self.effect_list(owner)?
            .info_by_skill_id(skill_id)
            .map(|info| info.skill.level)
    }

    /// Java `npc.getTemplate().getClanHelpRange()`.
    pub fn script_npc_clan_help_range(&self, npc: ObjectId) -> i32 {
        let npc_id = self.npc(npc).map_or(0, |n| n.npc_id);
        self.npc_world.template(npc_id).map_or(0, |t| t.clan_help_range)
    }

    /// Java `creature.setHeading(heading)`.
    pub fn script_set_heading(&mut self, id: ObjectId, heading: i32) {
        self.set_heading(id, heading);
    }

    /// Java `creature.isInsideZone(ZoneId.SIEGE)`.
    pub fn script_is_inside_siege_zone(&self, creature: ObjectId) -> bool {
        self.is_inside_zone(creature, crate::zones::ZoneId::Siege)
    }
}
