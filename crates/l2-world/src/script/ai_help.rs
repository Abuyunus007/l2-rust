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

/// Java `Npc.getVariables()` (`NpcVariables`) and `Npc.getScriptValue()` for the scripts that
/// keep a few flags and counters on a monster: a script's [`ScriptCtx::state`] holds one of
/// these, keyed by the NPC's object id (a script holds no state of its own, D-10).
// NOTE(java-quirk): Java's variables live in the `Npc` object, which a respawn of the same spawn
// reuses, so they survive a respawn there; here they belong to the object id, which a respawn
// keeps too (`respawn.rs`). Entries of NPCs that are gone for good (minions, summoned bosses) are
// dropped by [`ScriptCtx::clear_npc_vars`] where a script knows it.
#[derive(Debug, Default)]
pub struct NpcVars(std::collections::HashMap<ObjectId, std::collections::HashMap<&'static str, i64>>);

impl NpcVars {
    fn get(&self, npc: ScriptNpc, key: &'static str) -> i64 {
        self.0
            .get(&npc.object_id)
            .and_then(|v| v.get(key))
            .copied()
            .unwrap_or(0)
    }

    fn set(&mut self, npc: ScriptNpc, key: &'static str, value: i64) {
        self.0.entry(npc.object_id).or_default().insert(key, value);
    }
}

/// The key of Java `Npc.getScriptValue()` in [`NpcVars`].
const SCRIPT_VALUE: &str = "@scriptValue";

impl crate::world::World {
    /// `true` once a script called `setCanReturnToSpawnPoint(false)` on the NPC (tests).
    pub fn npc_cannot_return_to_spawn_point(&self, npc: ObjectId) -> bool {
        self.npc_world.no_return_to_spawn.contains(&npc)
    }

    /// `true` once a script called `setRandomWalking(false)` on the NPC (tests).
    pub fn npc_random_walking_forbidden(&self, npc: ObjectId) -> bool {
        self.npc_world.random_walk_off.contains(&npc)
    }

    /// The value of an NPC variable of a script whose state is an [`NpcVars`] (0 when unset):
    /// for tests and admin views. Flags read 0 or 1; the script value has the key
    /// `"@scriptValue"`.
    pub fn script_npc_var(&self, script: &str, npc: ObjectId, key: &str) -> i64 {
        self.scripts
            .by_name(script)
            .and_then(|id| self.scripts.script_state.get(&id))
            .and_then(|state| state.downcast_ref::<NpcVars>())
            .and_then(|vars| vars.0.get(&npc))
            .and_then(|vars| vars.get(key))
            .copied()
            .unwrap_or(0)
    }
}

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

    /// Java `npc.getVariables().getBoolean(key, false)` (the state of the script must be an
    /// [`NpcVars`]).
    pub fn npc_flag(&mut self, npc: ScriptNpc, key: &'static str) -> bool {
        self.state::<NpcVars>().get(npc, key) != 0
    }

    /// Java `npc.getVariables().set(key, value)` for a boolean.
    pub fn set_npc_flag(&mut self, npc: ScriptNpc, key: &'static str, value: bool) {
        self.state::<NpcVars>().set(npc, key, i64::from(value));
    }

    /// Java `npc.getVariables().getInt(key)`.
    pub fn npc_int(&mut self, npc: ScriptNpc, key: &'static str) -> i32 {
        i32::try_from(self.state::<NpcVars>().get(npc, key)).unwrap_or(0)
    }

    /// Java `npc.getVariables().set(key, value)` for an int.
    pub fn set_npc_int(&mut self, npc: ScriptNpc, key: &'static str, value: i32) {
        self.state::<NpcVars>().set(npc, key, i64::from(value));
    }

    /// Java `npc.isScriptValue(value)`.
    pub fn is_script_value(&mut self, npc: ScriptNpc, value: i32) -> bool {
        self.state::<NpcVars>().get(npc, SCRIPT_VALUE) == i64::from(value)
    }

    /// Java `npc.setScriptValue(value)`.
    pub fn set_script_value(&mut self, npc: ScriptNpc, value: i32) {
        self.state::<NpcVars>().set(npc, SCRIPT_VALUE, i64::from(value));
    }

    /// Forgets the variables of an NPC that is gone.
    pub fn clear_npc_vars(&mut self, npc: ScriptNpc) {
        self.state::<NpcVars>().0.remove(&npc.object_id);
    }

    /// Java `LocationUtil.calculateDistance(npc, npc.getSpawn(), false, false)`: the flat distance
    /// to the spawn point; 0 for an NPC without a spawn (Java throws).
    pub fn npc_distance_from_spawn(&self, npc: ScriptNpc) -> f64 {
        // An NPC made by `addSpawn` or an admin command has no entry in the spawn list here.
        if self.world().npc(npc.object_id).is_none_or(|n| n.spawn.is_none()) {
            return 0.0;
        }
        let (Some((sx, sy, _)), Some((x, y, _))) = (
            self.npc_spawn_point(npc),
            self.world().position_of(npc.object_id),
        ) else {
            return 0.0;
        };
        let (dx, dy) = (f64::from(x - sx), f64::from(y - sy));
        (dx * dx + dy * dy).sqrt()
    }

    /// Java `npc.asAttackable().getMostHated()`.
    pub fn most_hated(&mut self, npc: ScriptNpc) -> Option<ObjectId> {
        self.world_mut().aggro_most_hated(npc.object_id)
    }

    /// Java `npc.asAttackable().stopHating(target)`: the hate goes to 0.
    pub fn stop_hating(&mut self, npc: ScriptNpc, target: ObjectId) {
        if let Some(state) = self.world_mut().combat.creatures.get_mut(&npc.object_id) {
            for info in &mut state.aggro {
                if info.attacker == target {
                    info.hate = 0;
                }
            }
        }
    }

    /// Java `npc.getAI().setIntentionAttack(target)`.
    pub fn set_intention_attack(&mut self, npc: ScriptNpc, target: ObjectId) {
        self.world_mut().set_intention_attack(npc.object_id, target);
    }

    /// Java `npc.setTarget(target)` + `npc.doCast(skill)`: [`ScriptCtx::cast_skill`] for a target
    /// that may be none.
    pub fn cast_skill_at(&mut self, npc: ScriptNpc, target: Option<ObjectId>, skill: SkillKey) {
        self.world_mut().set_npc_target(npc.object_id, target);
        self.world_mut().do_cast(npc.object_id, skill);
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

    /// Java `npc.setRandomWalking(value)`: only `false` is kept (the template decides again at
    /// the next spawn, as `Npc.onSpawn` does, which the script then sets again).
    pub fn set_random_walking(&mut self, npc: ScriptNpc, value: bool) {
        let off = &mut self.world_mut().npc_world.random_walk_off;
        if value {
            off.remove(&npc.object_id);
        } else {
            off.insert(npc.object_id);
        }
    }

    /// Java `npc.asAttackable().setCanReturnToSpawnPoint(value)`.
    pub fn set_can_return_to_spawn_point(&mut self, npc: ScriptNpc, value: bool) {
        let set = &mut self.world_mut().npc_world.no_return_to_spawn;
        if value {
            set.remove(&npc.object_id);
        } else {
            set.insert(npc.object_id);
        }
    }

    /// Java `npc.setImmobilized(value)`.
    pub fn set_immobilized(&mut self, npc: ScriptNpc, value: bool) {
        self.world_mut().set_immobilized(npc.object_id, value);
    }

    /// Java `creature.isCursedWeaponEquipped()`.
    pub fn is_cursed_weapon_equipped(&self, player: ObjectId) -> bool {
        self.world().is_cursed_weapon_equipped(player)
    }

    /// Java `owner.getSummon()`: the servitor or pet of a player.
    pub fn summon_of(&self, owner: ObjectId) -> Option<ObjectId> {
        self.world().summon_of(owner)
    }

    /// Java `a.calculateDistance3D(b)`.
    pub fn distance_3d(&self, a: ObjectId, b: ObjectId) -> Option<f64> {
        let (ax, ay, az) = self.world().position_of(a)?;
        let (bx, by, bz) = self.world().position_of(b)?;
        let (dx, dy, dz) = (f64::from(ax - bx), f64::from(ay - by), f64::from(az - bz));
        Some((dx * dx + dy * dy + dz * dz).sqrt())
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

    /// Java `Rnd.nextDouble()`: `[0, 1)`.
    pub fn next_double(&mut self) -> f64 {
        crate::formulas::rng::Rnd::next_double(self.world_mut().rng())
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
