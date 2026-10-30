//! The tamed beasts (S-API-05): the NPC a player gets at the last growth step of the Beast Farm
//! and the Feedable Beasts. It follows its owner, buffs him, heals him when he is hit and eats
//! the owner's food to stay.
// Java: entity/actor/instance/TamedBeast.java, FeedableBeast.java,
//       entity/actor/Player.java (getTrainedBeasts, addTrainedBeast, the teleport and
//       reduceCurrentHp parts)
//
// A `TamedBeast` is a `FeedableBeast`, a `Monster`: here an NPC of class `Monster` with the
// state below kept by the script engine. Java's three `ThreadPool` tasks (`CheckDuration` every
// minute, `CheckOwnerBuffs` every 5 s, the `buffCast`s of `castBeastSkills`) are timers on the
// wheel. Java's `Player._tamedBeast` is `owners`.
//
// NOTE: Java's `setOwner` broadcasts `NpcInfo` with the summon animation; here the title change
// sends the plain `NpcInfo` (`script_set_npc_title`).

use std::collections::HashMap;

use l2_protocol_game::server::{SocialAction, StopMove};
use l2_util::ObjectId;

use super::timers::ScriptTimer;
use crate::effects::SkillKey;
use crate::effects::registry::EffectType;
use crate::npc::NpcClass;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

const MAX_DISTANCE_FROM_OWNER: f64 = 2000.0;
const MAX_DURATION: i32 = 1_200_000; // 20 minutes
const DURATION_CHECK_INTERVAL: i32 = 60_000; // 1 minute
const DURATION_INCREASE_INTERVAL: i32 = 20_000; // 20 secs (gained upon feeding)
const BUFF_INTERVAL: u64 = 5_000; // 5 seconds

/// The tasks of a tamed beast.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TamedBeastTask {
    /// `CheckDuration`.
    Duration,
    /// `CheckOwnerBuffs`.
    OwnerBuffs,
    /// `buffCast(skill)` of `castBeastSkills` (`None`: the last one, back to following).
    BuffCast(Option<SkillKey>),
}

/// The fields of one Java `TamedBeast`.
#[derive(Debug, Clone)]
struct Beast {
    food_skill_id: i32,
    remaining_time: i32,
    home: (i32, i32, i32),
    owner: Option<ObjectId>,
    is_freya_beast: bool,
    beast_skills: Vec<SkillKey>,
    /// `CheckOwnerBuffs._numBuffs`.
    num_buffs: i32,
    buff_task: Option<TimerId>,
    duration_task: Option<TimerId>,
}

/// Every tamed beast and the beasts of each player.
#[derive(Debug, Default)]
pub(crate) struct TamedBeasts {
    beasts: HashMap<ObjectId, Beast>,
    /// Java `Player._tamedBeast`, in the order they came.
    owners: HashMap<ObjectId, Vec<ObjectId>>,
}

impl World {
    /// Java `new TamedBeast(npcTemplateId, owner, food, x, y, z, isFreyaBeast)` (the Beast Farm
    /// passes `true`) and `new TamedBeast(npcTemplateId, owner, foodSkillId, x, y, z)` (the
    /// Feedable Beasts, `false`): the beast spawns at the place, full HP and MP, with its owner
    /// and food. `None` for an unknown template or an unknown owner.
    pub fn spawn_tamed_beast(
        &mut self,
        npc_id: i32,
        owner: ObjectId,
        food: i32,
        (x, y, z): (i32, i32, i32),
        is_freya_beast: bool,
    ) -> Option<ObjectId> {
        if !self.players.contains_key(&owner) {
            return None;
        }
        let beast = self.spawn_npc_at_as(npc_id, NpcClass::Monster, (x, y, z), 0)?;
        // `fullRestore()`.
        if let Some((hp, mp)) = self
            .stats(beast)
            .map(|v| (f64::from(v.max_hp()), f64::from(v.max_mp())))
        {
            self.set_current_hp(beast, hp, true);
            self.set_current_mp(beast, mp, true);
        }
        self.scripts.tamed.beasts.insert(
            beast,
            Beast {
                food_skill_id: 0,
                remaining_time: MAX_DURATION,
                home: (x, y, z),
                owner: None,
                is_freya_beast,
                beast_skills: Vec::new(),
                num_buffs: 0,
                buff_task: None,
                duration_task: None,
            },
        );
        // NOTE(java-quirk): the Feedable Beasts' constructor sets the owner before `spawnMe`, the
        // Beast Farm's after it; both end with an owner who is followed.
        self.tamed_beast_set_food_type(beast, food);
        self.tamed_beast_set_owner(beast, owner);
        if is_freya_beast {
            // `getAI().setIntentionFollow(_owner)`.
            self.script_start_follow(beast, owner);
        }
        Some(beast)
    }

    /// Java `npc.broadcastPacket(new NpcInfo(npc, ..))`: everybody who sees the NPC gets its
    /// info again (a name or title a script changed).
    pub fn broadcast_npc_info(&mut self, npc: ObjectId) {
        self.broadcast_info(npc);
    }

    /// Java `beast instanceof TamedBeast`.
    pub fn is_tamed_beast(&self, npc: ObjectId) -> bool {
        self.scripts.tamed.beasts.contains_key(&npc)
    }

    /// Java `TamedBeast.getOwner()`.
    pub fn tamed_beast_owner(&self, beast: ObjectId) -> Option<ObjectId> {
        self.scripts.tamed.beasts.get(&beast)?.owner
    }

    /// Java `TamedBeast.getFoodType()`.
    pub fn tamed_beast_food_type(&self, beast: ObjectId) -> i32 {
        self.scripts
            .tamed
            .beasts
            .get(&beast)
            .map_or(0, |b| b.food_skill_id)
    }

    /// Java `TamedBeast.getRemainingTime()`.
    pub fn tamed_beast_remaining_time(&self, beast: ObjectId) -> i32 {
        self.scripts
            .tamed
            .beasts
            .get(&beast)
            .map_or(0, |b| b.remaining_time)
    }

    /// Java `TamedBeast.isFreyaBeast()`.
    pub fn is_freya_beast(&self, beast: ObjectId) -> bool {
        self.scripts
            .tamed
            .beasts
            .get(&beast)
            .is_some_and(|b| b.is_freya_beast)
    }

    /// Java `TamedBeast.getHome()`.
    pub fn tamed_beast_home(&self, beast: ObjectId) -> Option<(i32, i32, i32)> {
        self.scripts.tamed.beasts.get(&beast).map(|b| b.home)
    }

    /// Java `player.getTrainedBeasts()`: empty without any.
    pub fn trained_beasts(&self, owner: ObjectId) -> Vec<ObjectId> {
        self.scripts.tamed.owners.get(&owner).cloned().unwrap_or_default()
    }

    /// Java `TamedBeast.onReceiveFood()`: 20 s more, at most 20 minutes.
    pub fn tamed_beast_on_receive_food(&mut self, beast: ObjectId) {
        if let Some(b) = self.scripts.tamed.beasts.get_mut(&beast) {
            b.remaining_time = (b.remaining_time + DURATION_INCREASE_INTERVAL).min(MAX_DURATION);
        }
    }

    /// Java `TamedBeast.addBeastSkill(skill)`.
    pub fn tamed_beast_add_skill(&mut self, beast: ObjectId, skill: SkillKey) {
        if let Some(b) = self.scripts.tamed.beasts.get_mut(&beast)
            && !b.beast_skills.contains(&skill)
        {
            b.beast_skills.push(skill);
        }
    }

    /// Java `TamedBeast.castBeastSkills()`: each beast skill on the owner, 100 ms plus the hit
    /// time of the one before apart, then back to following.
    pub fn tamed_beast_cast_skills(&mut self, beast: ObjectId) {
        let Some(b) = self.scripts.tamed.beasts.get(&beast) else {
            return;
        };
        if b.owner.is_none() || b.beast_skills.is_empty() {
            return;
        }
        let skills = b.beast_skills.clone();
        let table = self.skill_table();
        let mut delay: i64 = 100;
        for skill in skills {
            self.tamed_beast_schedule(beast, TamedBeastTask::BuffCast(Some(skill)), delay);
            let hit_time = table
                .as_ref()
                .and_then(|t| t.get(skill.id, skill.level))
                .map_or(0, |s| s.hit_time);
            delay += 100 + i64::from(hit_time);
        }
        self.tamed_beast_schedule(beast, TamedBeastTask::BuffCast(None), delay);
    }

    /// Java `TamedBeast.isAutoAttackable(attacker)`: `!_isFreyaBeast`; `None` for another NPC.
    pub(crate) fn tamed_beast_auto_attackable(&self, npc: ObjectId) -> Option<bool> {
        self.scripts.tamed.beasts.get(&npc).map(|b| !b.is_freya_beast)
    }

    // ------------------------------------------------------------------ the Java setters

    /// Java `setFoodType(foodItemId)`: the duration check starts (again) for a food.
    fn tamed_beast_set_food_type(&mut self, beast: ObjectId, food: i32) {
        if food <= 0 {
            return;
        }
        let old = self.scripts.tamed.beasts.get_mut(&beast).and_then(|b| {
            b.food_skill_id = food;
            b.duration_task.take()
        });
        if let Some(old) = old {
            self.timers.cancel(old);
        }
        let task = self.tamed_beast_schedule(
            beast,
            TamedBeastTask::Duration,
            i64::from(DURATION_CHECK_INTERVAL),
        );
        if let Some(b) = self.scripts.tamed.beasts.get_mut(&beast) {
            b.duration_task = Some(task);
        }
    }

    /// Java `setOwner(owner)`: the title, the trained beast list, following at 100 and (not for
    /// a Freya beast) the buff check every 5 s.
    fn tamed_beast_set_owner(&mut self, beast: ObjectId, owner: ObjectId) {
        let Some(name) = self.players.get(&owner).map(|p| p.data.name.clone()) else {
            // "despawn if no owner"
            self.delete_npc(beast);
            return;
        };
        if let Some(b) = self.scripts.tamed.beasts.get_mut(&beast) {
            b.owner = Some(owner);
        }
        self.script_set_npc_title(beast, &name);
        let list = self.scripts.tamed.owners.entry(owner).or_default();
        if !list.contains(&beast) {
            list.push(beast);
        }
        // "Always and automatically follow the owner."
        self.start_follow(beast, owner, 100);
        if self.is_freya_beast(beast) {
            return;
        }
        // "Instead of calculating this value each time, let's get this now and pass it on."
        let total_buffs_available = self
            .tamed_beast_template_skills(beast)
            .iter()
            .filter(|(_, continuous, debuff)| *continuous && !*debuff)
            .count();
        let old = self.scripts.tamed.beasts.get_mut(&beast).and_then(|b| {
            b.num_buffs = i32::try_from(total_buffs_available).unwrap_or(i32::MAX);
            b.buff_task.take()
        });
        if let Some(old) = old {
            self.timers.cancel(old);
        }
        let task = self.tamed_beast_schedule(beast, TamedBeastTask::OwnerBuffs, BUFF_INTERVAL as i64);
        if let Some(b) = self.scripts.tamed.beasts.get_mut(&beast) {
            b.buff_task = Some(task);
        }
    }

    /// `getTemplate().getSkills().values()` in Java's `HashMap` order: the skill, whether it is
    /// continuous and whether it is a debuff.
    fn tamed_beast_template_skills(&self, beast: ObjectId) -> Vec<(SkillKey, bool, bool)> {
        let Some(template) = self
            .npcs
            .get(&beast)
            .and_then(|n| self.npc_world.template(n.npc_id))
        else {
            return Vec::new();
        };
        let Some(table) = self.skill_table() else {
            return Vec::new();
        };
        // `skills.put(skill.getId(), skill)`: a later level of an id replaces the earlier one.
        let mut order: Vec<i32> = Vec::new();
        let mut levels: HashMap<i32, i32> = HashMap::new();
        for s in &template.skills {
            if table.get(s.id, s.level).is_none() {
                continue;
            }
            if levels.insert(s.id, s.level).is_none() {
                order.push(s.id);
            }
        }
        crate::stats::skills::java_hash_map_order(&order)
            .into_iter()
            .filter_map(|id| {
                let level = *levels.get(&id)?;
                let skill = table.get(id, level)?;
                Some((
                    SkillKey::new(id, level),
                    skill.operate_type.is_continuous(),
                    skill.is_debuff,
                ))
            })
            .collect()
    }

    fn tamed_beast_schedule(&mut self, beast: ObjectId, task: TamedBeastTask, delay_ms: i64) -> TimerId {
        let ticks = TimerWheel::ms_to_ticks(u64::try_from(delay_ms).unwrap_or(0));
        self.timers.schedule_at(
            self.tick + ticks,
            TimerKind::Script(ScriptTimer::TamedBeast { beast, task }),
        )
    }

    // ------------------------------------------------------------------ the tasks

    /// The timer of a task of a tamed beast.
    pub(crate) fn on_tamed_beast_timer(&mut self, beast: ObjectId, task: TamedBeastTask) {
        if !self.scripts.tamed.beasts.contains_key(&beast) {
            return;
        }
        match task {
            TamedBeastTask::Duration => {
                // `scheduleAtFixedRate`: the next run first, the check may cancel it.
                let next = self.tamed_beast_schedule(
                    beast,
                    TamedBeastTask::Duration,
                    i64::from(DURATION_CHECK_INTERVAL),
                );
                if let Some(b) = self.scripts.tamed.beasts.get_mut(&beast) {
                    b.duration_task = Some(next);
                }
                self.tamed_beast_check_duration(beast);
            }
            TamedBeastTask::OwnerBuffs => {
                let next = self.tamed_beast_schedule(beast, TamedBeastTask::OwnerBuffs, BUFF_INTERVAL as i64);
                if let Some(b) = self.scripts.tamed.beasts.get_mut(&beast) {
                    b.buff_task = Some(next);
                }
                self.tamed_beast_check_owner_buffs(beast);
            }
            TamedBeastTask::BuffCast(skill) => {
                let Some(owner) = self.tamed_beast_owner(beast) else {
                    return;
                };
                match skill {
                    // `getAI().setIntentionFollow(_owner)`.
                    None => self.script_start_follow(beast, owner),
                    Some(skill) => self.tamed_beast_sit_cast_and_follow(beast, skill, owner),
                }
            }
        }
    }

    /// Java `CheckDuration.run()`.
    fn tamed_beast_check_duration(&mut self, beast: ObjectId) {
        let Some(b) = self.scripts.tamed.beasts.get(&beast).cloned() else {
            return;
        };
        // NOTE(java-quirk): Java reads `owner.getInventory()` without a null check; a beast
        // without an owner throws there and the task goes on. Here it does nothing.
        let Some(owner) = b.owner.filter(|o| self.players.contains_key(o)) else {
            return;
        };
        let food = b.food_skill_id;
        if b.is_freya_beast {
            if self
                .inventory_item_by_item_id(owner, food)
                .is_some_and(|i| i.count >= 1)
            {
                self.destroy_item_by_item_id(owner, food, 1, true);
                self.broadcast_packet(
                    beast,
                    &SocialAction {
                        object_id: beast.0,
                        action_id: 3,
                    },
                );
            } else {
                self.delete_npc(beast);
            }
            return;
        }
        let remaining = b.remaining_time - DURATION_CHECK_INTERVAL;
        if let Some(b) = self.scripts.tamed.beasts.get_mut(&beast) {
            b.remaining_time = remaining;
        }
        // "Find which food item needs to be consumed."
        let item_id = match food {
            2188 => Some(6643),
            2189 => Some(6644),
            _ => None,
        };
        let has_food = item_id.is_some_and(|id| {
            self.inventory_item_by_item_id(owner, id)
                .is_some_and(|i| i.count >= 1)
        });
        if has_food {
            // "Emulate a call to the owner using food, but bypass all checks for range, etc."
            let old_target = self.target_of(owner);
            self.set_target(owner, Some(beast));
            if let Some(skill) = self.skill_table().and_then(|t| t.get(food, 1).cloned()) {
                self.call_skill(owner, &skill, vec![beast]);
            }
            self.set_target(owner, old_target);
        } else {
            // "if the owner has no food, the beast immediately despawns, except when it was only
            // newly spawned. Newly spawned beasts can last up to 5 minutes"
            let now = self.tamed_beast_remaining_time(beast);
            if now < MAX_DURATION - 300_000
                && let Some(b) = self.scripts.tamed.beasts.get_mut(&beast)
            {
                b.remaining_time = -1;
            }
        }
        if self.tamed_beast_remaining_time(beast) <= 0 && self.is_tamed_beast(beast) {
            self.delete_npc(beast);
        }
    }

    /// Java `CheckOwnerBuffs.run()`.
    fn tamed_beast_check_owner_buffs(&mut self, beast: ObjectId) {
        let Some(b) = self.scripts.tamed.beasts.get(&beast).cloned() else {
            return;
        };
        // "Check if the owner is no longer around...if so, despawn."
        let Some(owner) = b.owner.filter(|o| self.players.contains_key(o)) else {
            self.delete_npc(beast);
            return;
        };
        // "If the owner is too far away, stop anything else and immediately run towards the
        // owner."
        let near = self
            .distance_3d_between(beast, owner)
            .is_some_and(|d| d < MAX_DISTANCE_FROM_OWNER);
        if !near {
            self.script_start_follow(beast, owner);
            return;
        }
        // "If the owner is dead, do nothing..."
        if self.is_dead(owner) {
            return;
        }
        // "If the tamed beast is currently casting a spell, do not interfere."
        if self.is_casting_now(beast) {
            return;
        }
        let mut total_buffs_on_owner = 0;
        let mut i = 0;
        let rand = self.rng().below(b.num_buffs);
        let mut buff_to_give = None;
        for (skill, continuous, debuff) in self.tamed_beast_template_skills(beast) {
            if continuous && !debuff {
                if i == rand {
                    buff_to_give = Some(skill);
                }
                i += 1;
                if self.is_affected_by_skill(owner, skill.id) {
                    total_buffs_on_owner += 1;
                }
            }
        }
        // "If the owner has less than 60% of this beast's available buff, cast a random buff."
        if (b.num_buffs * 2) / 3 > total_buffs_on_owner
            && let Some(skill) = buff_to_give
        {
            self.tamed_beast_sit_cast_and_follow(beast, skill, owner);
        }
        self.script_start_follow(beast, owner);
    }

    /// Java `onOwnerGotAttacked(attacker)` of every beast of `owner` (`Player.reduceCurrentHp`).
    pub(crate) fn tamed_beasts_on_owner_attacked(&mut self, owner: ObjectId, attacker: Option<ObjectId>) {
        let beasts = self.trained_beasts(owner);
        for beast in beasts {
            self.tamed_beast_on_owner_attacked(beast, attacker);
        }
    }

    fn tamed_beast_on_owner_attacked(&mut self, beast: ObjectId, attacker: Option<ObjectId>) {
        let Some(b) = self.scripts.tamed.beasts.get(&beast).cloned() else {
            return;
        };
        // "Check if the owner is no longer around...if so, despawn."
        let Some(owner) = b.owner.filter(|o| self.players.contains_key(o)) else {
            self.delete_npc(beast);
            return;
        };
        // "If the owner is too far away, stop anything else and immediately run towards the
        // owner."
        let near = self
            .distance_3d_between(owner, beast)
            .is_some_and(|d| d < MAX_DISTANCE_FROM_OWNER);
        if !near {
            self.script_start_follow(beast, owner);
            return;
        }
        if self.is_dead(owner) || b.is_freya_beast {
            return;
        }
        if self.is_casting_now(beast) {
            return;
        }
        let (cur, max) = (
            self.core_of(owner).map_or(0.0, |c| c.cur_hp),
            self.stats(owner).map_or(0, |s| s.max_hp()),
        );
        // `(float) getCurrentHp() / getMaxHp()`.
        let hp_ratio = (cur as f32) / (max as f32);
        let skills = self.tamed_beast_template_skills(beast);
        if hp_ratio >= 0.8 {
            for (skill, _, debuff) in skills {
                // NOTE(java-quirk): the attacker must already have the debuff (the comment says
                // "check if the attacker has it already", the code casts only then).
                if debuff
                    && self.rng().below(3) < 1
                    && attacker.is_some_and(|a| self.is_affected_by_skill(a, skill.id))
                    && let Some(attacker) = attacker
                {
                    self.tamed_beast_sit_cast_and_follow(beast, skill, attacker);
                }
            }
        } else if hp_ratio < 0.5 {
            let chance = if hp_ratio < 0.25 { 2 } else { 1 };
            for (skill, _, _) in skills {
                if self.rng().below(5) < chance
                    && self.script_skill_has_effect_type(
                        skill,
                        &[
                            EffectType::Cpheal,
                            EffectType::Heal,
                            EffectType::ManahealByLevel,
                            EffectType::ManahealPercent,
                        ],
                    )
                {
                    self.tamed_beast_sit_cast_and_follow(beast, skill, owner);
                }
            }
        }
    }

    /// Java `sitCastAndFollow(skill, target)`.
    fn tamed_beast_sit_cast_and_follow(&mut self, beast: ObjectId, skill: SkillKey, target: ObjectId) {
        self.stop_move(beast, None);
        if let Some((x, y, z)) = self.position_of(beast) {
            let heading = self.heading_of(beast);
            self.broadcast_packet(
                beast,
                &StopMove {
                    object_id: beast.0,
                    x,
                    y,
                    z,
                    heading,
                },
            );
        }
        self.set_intention_idle(beast);
        self.set_npc_target(beast, Some(target));
        self.do_cast(beast, skill);
        if let Some(owner) = self.tamed_beast_owner(beast) {
            self.script_start_follow(beast, owner);
        }
    }

    // ------------------------------------------------------------------ the ends

    /// Java `TamedBeast.doDie` / `deleteMe` (and the NPC's decay): the tasks stop, the beast
    /// leaves its owner's list.
    pub(crate) fn tamed_beast_forget(&mut self, beast: ObjectId) {
        let Some(b) = self.scripts.tamed.beasts.remove(&beast) else {
            return;
        };
        for task in [b.buff_task, b.duration_task].into_iter().flatten() {
            self.timers.cancel(task);
        }
        if let Some(owner) = b.owner {
            let empty = self.scripts.tamed.owners.get_mut(&owner).is_some_and(|list| {
                list.retain(|id| *id != beast);
                list.is_empty()
            });
            if empty {
                self.scripts.tamed.owners.remove(&owner);
            }
        }
        // The `buffCast` tasks of `castBeastSkills` find no beast any more.
        self.timers.cancel_where(
            |kind| matches!(kind, TimerKind::Script(ScriptTimer::TamedBeast { beast: b, .. }) if *b == beast),
        );
    }

    /// Java `TamedBeast.doDie`: `getAI().stopFollow()` and the clean up.
    pub(crate) fn tamed_beast_on_death(&mut self, beast: ObjectId) {
        if !self.is_tamed_beast(beast) {
            return;
        }
        self.stop_follow(beast);
        self.tamed_beast_forget(beast);
    }

    /// Java `Player.onTeleported`: "Trained beast is lost after teleport."
    pub(crate) fn tamed_beasts_on_owner_teleport(&mut self, owner: ObjectId) {
        for beast in self.trained_beasts(owner) {
            self.delete_npc(beast);
        }
        self.scripts.tamed.owners.remove(&owner);
    }

    /// Java `TamedBeast.onAction(player, interact)`: `true` when the NPC is a tamed beast (the
    /// click is handled).
    pub(crate) fn tamed_beast_click(&mut self, player: ObjectId, beast: ObjectId, interact: bool) -> bool {
        let Some(auto_attackable) = self.tamed_beast_auto_attackable(beast) else {
            return false;
        };
        if self.target_of(player) != Some(beast) {
            self.set_target(player, Some(beast));
        } else if interact {
            let dz = match (self.position_of(player), self.position_of(beast)) {
                (Some(p), Some(b)) => (p.2 - b.2).abs(),
                _ => i32::MAX,
            };
            if auto_attackable && dz < 100 {
                self.set_intention_attack(player, beast);
            } else {
                self.send(player, &l2_protocol_game::server::ActionFailed);
            }
        }
        true
    }

    fn distance_3d_between(&self, a: ObjectId, b: ObjectId) -> Option<f64> {
        let (ax, ay, az) = self.position_of(a)?;
        let (bx, by, bz) = self.position_of(b)?;
        let (dx, dy, dz) = (f64::from(ax - bx), f64::from(ay - by), f64::from(az - bz));
        Some((dx * dx + dy * dy + dz * dz).sqrt())
    }
}
