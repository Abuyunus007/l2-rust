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

    /// Java `skill.getReuseHashCode()`; `None` for a skill the table does not have (S-QR-10).
    pub fn script_skill_reuse_hash_code(&self, key: SkillKey) -> Option<i32> {
        let table = self.skill_table()?;
        Some(table.get(key.id, key.level)?.reuse_hash_code)
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

    /// Java `npc.getAI().getIntention() == Intention.ATTACK` (SelMahumSquad; S-AI-08).
    pub fn script_intention_is_attack(&self, npc: ObjectId) -> bool {
        self.intention_of(npc) == crate::movement::Intention::Attack
    }

    /// Java `npc.getAI().getIntention() == Intention.CAST`.
    pub fn script_intention_is_cast(&self, npc: ObjectId) -> bool {
        self.intention_of(npc) == crate::movement::Intention::Cast
    }

    /// Java `npc.getRightHandItem()`: the item id in the right hand, 0 for none.
    pub fn script_npc_right_hand(&self, npc: ObjectId) -> i32 {
        self.npc(npc).map_or(0, |n| n.right_hand)
    }

    /// Java `npc.isInSurroundingRegion(other)`.
    pub fn script_is_in_surrounding_region(&self, npc: ObjectId, other: ObjectId) -> bool {
        self.is_in_surrounding_region(npc, other)
    }

    /// Java `skill.getAffectRange()` of a skill by id and level (0 for an unknown skill).
    pub fn script_skill_affect_range(&self, skill: SkillKey) -> i32 {
        self.skill_table()
            .and_then(|t| t.get(skill.id, skill.level).map(|s| s.affect_range))
            .unwrap_or(0)
    }

    /// Java `player.getKarma()`.
    pub fn script_player_karma(&self, player: ObjectId) -> i32 {
        self.karma_of_player(player)
    }

    /// Java `World.forEachVisibleObjectInRange(npc, Creature.class, range, ..)`: the players, NPCs
    /// and summons the NPC sees within `range`.
    pub fn script_visible_creatures_in_range(&self, npc: ObjectId, range: i32) -> Vec<ObjectId> {
        self.visible_objects_in_range(npc, range)
            .into_iter()
            .filter(|o| self.player(*o).is_some() || self.npc(*o).is_some() || self.is_summon(*o))
            .collect()
    }

    /// Java `player.updatePvPStatus(target)`.
    pub fn script_update_pvp_status(&mut self, player: ObjectId, target: ObjectId) {
        self.update_pvp_status_target(player, target);
    }

    /// Java `npc.getAggroRange()`.
    pub fn script_npc_aggro_range(&self, npc: ObjectId) -> i32 {
        self.npc_template(npc).map_or(0, |t| t.aggro_range)
    }

    /// Java `npc.getTemplate().getBaseAttackType()` is `SWORD` or `POLE`.
    pub fn script_npc_attacks_with_sword_or_pole(&self, npc: ObjectId) -> bool {
        use l2_data::item::WeaponType;
        self.npc_template(npc)
            .is_some_and(|t| matches!(t.base_attack_type, WeaponType::Sword | WeaponType::Pole))
    }

    /// Java `creature.isRaid()`.
    pub fn script_is_raid(&self, creature: ObjectId) -> bool {
        self.is_raid(creature)
    }

    /// Java `creature.isInvisible()`.
    pub fn script_is_invisible(&self, creature: ObjectId) -> bool {
        self.objects.get(&creature).is_some_and(|e| e.invisible)
    }

    /// Java `npc.getSpawn().stopRespawn()` (nothing for an NPC without a spawn).
    pub fn script_stop_npc_respawn(&mut self, npc: ObjectId) {
        if let Some(index) = self.npcs.get(&npc).and_then(|n| n.spawn) {
            self.stop_respawn(index);
        }
    }

    /// Java `ZoneManager.getZones(x, y, z)` filtered to `EffectZone`: the ids.
    pub fn script_effect_zones_at(&self, x: i32, y: i32, z: i32) -> Vec<i32> {
        self.zoning.zones.as_ref().map_or_else(Vec::new, |zones| {
            zones
                .zones_at(x, y, z)
                .into_iter()
                .filter(|zone| zone.zone_type == "EffectZone")
                .map(|zone| zone.id)
                .collect()
        })
    }

    /// Java `ZoneType.oustAllPlayers()` of a zone: everybody inside goes to the nearest town.
    pub fn script_oust_all_players(&mut self, zone_id: i32, zone_type: &str) {
        for player in self.zone_players_inside(zone_id, zone_type) {
            self.tele_to_location_where(player, crate::teleport::TeleportWhereType::Town);
        }
    }

    /// Java `GameTimeTaskManager.isNight()`.
    pub fn script_is_night(&self) -> bool {
        Self::is_night_now()
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
