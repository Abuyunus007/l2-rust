//! `ScriptCtx` helpers of the `ai/others` scripts (S-AI-05): small `pub` doors on world state the
//! scripts read or change, named after the Java calls they stand for.
// Java: entity/actor/Player.java (getFame, setFame, getPkKills, setPkKills, getExpertiseLevel,
//       getDeathPenaltyBuffLevel, setDeathPenaltyBuffLevel, hasAccess), entity/actor/Npc.java
//       (getCurrentHp, getMaxHp, getSpawn), mechanics/script/Quest.java (addSkillCastDesire)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::ctx::ScriptCtx;
use crate::clan::model::ClanAccess;
use crate::effects::SkillKey;
use crate::script::ScriptNpc;

/// Java `Player.getExpertiseLevel()` reads this skill.
const EXPERTISE_SKILL: i32 = 239;

impl ScriptCtx<'_> {
    /// The `Feature.ini` points the scripts read (`FeatureConfig.BLOODOATH_POINTS`, ...).
    pub fn script_settings(&self) -> &super::ScriptSettings {
        &self.world().scripts.settings
    }

    /// `player.sendPacket(SystemMessageId)`.
    pub fn send_system_message(&mut self, player: ObjectId, id: SystemMessageId) {
        self.world().send(player, &SystemMessage::new(id));
    }

    /// `new SystemMessage(id)` + `addInt(value)` + `player.sendPacket(sm)`.
    pub fn send_system_message_int(&mut self, player: ObjectId, id: SystemMessageId, value: i32) {
        self.world()
            .send(player, &SystemMessage::new(id).with(SystemMessageParam::IntNumber(value)));
    }

    /// Java `player.getAdena()`.
    pub fn adena_of(&self, player: ObjectId) -> i64 {
        self.world().adena_of(player)
    }

    /// Java `player.getExpertiseLevel()`: the level of skill 239.
    pub fn expertise_level(&self, player: ObjectId) -> i32 {
        self.world()
            .stats(player)
            .map_or(0, |s| s.skill_level(EXPERTISE_SKILL))
    }

    /// Java `player.getFame()`.
    pub fn fame_of(&self, player: ObjectId) -> i32 {
        self.world().player(player).map_or(0, |p| p.data.profile.fame)
    }

    /// Java `player.setFame(fame)`.
    pub fn set_fame(&mut self, player: ObjectId, fame: i32) {
        self.world_mut().set_fame(player, fame);
    }

    /// Java `player.getPkKills()`.
    pub fn pk_kills_of(&self, player: ObjectId) -> i32 {
        self.world().player(player).map_or(0, |p| p.data.pk_kills)
    }

    /// Java `player.setPkKills(value)`.
    pub fn set_pk_kills(&mut self, player: ObjectId, value: i32) {
        if let Some(p) = self.world_mut().player_mut(player) {
            p.data.pk_kills = value;
        }
    }

    /// Java `player.getPlayerClass().level()`: 0 first class, 1, 2, 3.
    pub fn class_level_of(&self, player: ObjectId) -> i32 {
        self.world()
            .player(player)
            .and_then(|p| u16::try_from(p.data.class_id).ok())
            .map_or(0, |c| l2_data::ClassId(c).class_level())
    }

    /// Java `player.updateUserInfo()`.
    pub fn update_user_info(&mut self, player: ObjectId) {
        self.world().send_user_info(player);
    }

    /// Java `player.getDeathPenaltyBuffLevel()`.
    pub fn death_penalty_buff_level(&self, player: ObjectId) -> i32 {
        self.world()
            .player(player)
            .map_or(0, |p| p.data.profile.death_penalty_level)
    }

    /// Java `player.setDeathPenaltyBuffLevel(level)`: only the field (no skill change).
    pub fn set_death_penalty_buff_level(&mut self, player: ObjectId, level: i32) {
        if let Some(p) = self.world_mut().player_mut(player) {
            p.data.profile.death_penalty_level = level;
        }
    }

    /// `player.sendPacket(new EtcStatusUpdate(player))`.
    pub fn send_etc_status_update(&mut self, player: ObjectId) {
        self.world().send_etc_status_update(player);
    }

    /// Java `player.getClanId()` (0 without a clan).
    pub fn clan_id_of(&self, player: ObjectId) -> i32 {
        self.world().clan_id_of(player)
    }

    /// Java `player.getClan().getLevel()`; `None` without a clan.
    pub fn clan_level_of(&self, player: ObjectId) -> Option<i32> {
        self.world().clan_of(player).map(|c| c.level)
    }

    /// Java `player.isClanLeader()`.
    pub fn is_clan_leader(&self, player: ObjectId) -> bool {
        self.world().is_clan_leader(player)
    }

    /// Java `player.hasAccess(ClanAccess.MEMBER_FAME)`.
    pub fn has_member_fame_access(&self, player: ObjectId) -> bool {
        self.world().has_clan_access(player, ClanAccess::MemberFame)
    }

    /// Java `player.getClan().addReputationScore(value)`.
    pub fn add_clan_reputation_score(&mut self, player: ObjectId, value: i32) {
        let clan_id = self.world().clan_id_of(player);
        if clan_id != 0 {
            self.world_mut().add_reputation_score(clan_id, value);
        }
    }

    /// Java `npc.getCurrentHp()`.
    pub fn npc_current_hp(&self, npc: ScriptNpc) -> f64 {
        self.world().npc(npc.object_id).map_or(0.0, |n| n.core.cur_hp)
    }

    /// Java `npc.getMaxHp()`.
    pub fn npc_max_hp(&self, npc: ScriptNpc) -> i32 {
        self.world().stats(npc.object_id).map_or(0, |s| s.max_hp())
    }

    /// Java `npc.getSpawn().getX/Y/Z()`: where the NPC spawned.
    pub fn npc_spawn_point(&self, npc: ScriptNpc) -> Option<(i32, i32, i32)> {
        self.world().npc(npc.object_id).map(|n| n.spawn_point)
    }

    /// Java `npc.getX()/getY()/getZ()` and `getHeading()`.
    pub fn npc_position(&self, npc: ScriptNpc) -> Option<(i32, i32, i32, i32)> {
        let (x, y, z) = self.world().position_of(npc.object_id)?;
        Some((x, y, z, self.world().heading_of(npc.object_id)))
    }

    /// Java `npc.isDead()`.
    pub fn is_dead(&self, creature: ObjectId) -> bool {
        self.world().is_dead(creature)
    }

    /// Java `npc.teleToLocation(x, y, z)`.
    pub fn tele_npc_to(&mut self, npc: ScriptNpc, x: i32, y: i32, z: i32) {
        self.world_mut()
            .tele_to_location(npc.object_id, crate::teleport::TeleportTarget::at(x, y, z));
    }

    /// Java `npc.deleteMe()`.
    pub fn delete_npc(&mut self, npc: ScriptNpc) {
        self.world_mut().delete_npc(npc.object_id);
    }

    /// Java `Quest.addSkillCastDesire(npc, target, skill, desire)`.
    pub fn add_skill_cast_desire(&mut self, npc: ScriptNpc, target: ObjectId, skill: SkillKey, desire: i64) {
        if self.world().is_attackable_npc(npc.object_id) {
            self.world_mut().add_damage_hate(npc.object_id, target, 0, desire);
        }
        self.world_mut().set_npc_target(npc.object_id, Some(target));
        self.world_mut()
            .set_intention_cast(npc.object_id, skill, Some(target));
    }

    /// Java `player.getVariables().getString(name)`: a `character_variables` value.
    pub fn player_variable(&self, player: ObjectId, name: &str) -> Option<String> {
        self.world()
            .player(player)
            .and_then(|p| p.data.profile.variables.get(name).cloned())
    }

    /// Java `player.getVariables().getInt(name, default)`: the default for a missing or bad value.
    pub fn player_variable_int(&self, player: ObjectId, name: &str, default: i32) -> i32 {
        self.player_variable(player, name)
            .and_then(|v| v.trim().parse().ok())
            .unwrap_or(default)
    }

    /// Java `player.getVariables().set(name, value)`: stored with the next store of the character.
    pub fn set_player_variable(&mut self, player: ObjectId, name: &str, value: &str) {
        self.world_mut()
            .set_player_variable_text(player, name, value.to_owned());
    }

    /// Java `player.getVariables().remove(name)`.
    pub fn remove_player_variable(&mut self, player: ObjectId, name: &str) {
        if let Some(p) = self.world_mut().player_mut(player) {
            let profile = &mut p.data.profile;
            if profile.variables.remove(name).is_some() {
                profile.variables_changed.remove(name);
                profile.variables_removed.insert(name.to_owned());
            }
        }
    }

    /// Java `TownManager.getTown(x, y, z).getTownId()`; `None` outside every `TownZone`.
    pub fn town_id_at(&self, x: i32, y: i32, z: i32) -> Option<i32> {
        self.world().zoning.zones.as_ref().and_then(|zones| {
            zones
                .zones_at(x, y, z)
                .into_iter()
                .find(|zone| zone.zone_type == "TownZone")
                .map(|zone| zone.int_param("townId").unwrap_or(0))
        })
    }

    /// Java `GeoEngine.getInstance().getHeight(x, y, z)`.
    pub fn geo_height(&self, x: i32, y: i32, z: i32) -> i32 {
        self.world().geo_height(x, y, z)
    }

    /// Java `creature.isInCombat()`.
    pub fn is_in_combat(&self, creature: ObjectId) -> bool {
        self.world().is_in_combat(creature)
    }

    /// Java `SevenSigns.getPlayerCabal(player.getObjectId())`.
    pub fn ss_player_cabal(&self, player: ObjectId) -> i32 {
        self.world().ss_player_cabal(player.0)
    }

    /// Java `SevenSigns.getPlayerSeal(player.getObjectId())`.
    pub fn ss_player_seal(&self, player: ObjectId) -> i32 {
        self.world().ss_player_seal(player.0)
    }

    /// Java `SevenSigns.getSealOwner(seal)`.
    pub fn ss_seal_owner(&self, seal: i32) -> i32 {
        self.world().ss_seal_owner(seal)
    }

    /// Java `SevenSigns.getCabalHighestScore()`.
    pub fn ss_cabal_highest_score(&self) -> i32 {
        self.world().ss_cabal_highest_score()
    }

    /// Java `SevenSigns.isSealValidationPeriod()`.
    pub fn ss_is_seal_validation_period(&self) -> bool {
        self.world().ss_is_seal_validation_period()
    }

    /// Java `npc.isAffectedBySkill(skillId)`.
    pub fn is_affected_by_skill(&self, creature: ObjectId, skill_id: i32) -> bool {
        self.world().is_affected_by_skill(creature, skill_id)
    }

    /// Java `creature.isInsideZone(ZoneId.PVP)`.
    pub fn is_inside_pvp_zone(&self, creature: ObjectId) -> bool {
        self.world().is_inside_zone(creature, crate::zones::ZoneId::Pvp)
    }
}
