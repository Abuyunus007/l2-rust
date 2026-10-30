//! Making a summon and putting it into the world: the `Summon` constructor, `Servitor`'s, the
//! `Summon` effect's set up, `spawnMe` and `onSpawn`.
// Java: entity/actor/Summon.java (constructor, onSpawn, setFollowStatus, setRunning via
//       Creature.setRunning)
// Java: entity/actor/instance/Servitor.java (constructor, onSpawn)
// Java: dist/game/data/scripts/handlers/skill/effects/Summon.java (onStart)

use l2_protocol_game::server::{RelationChanged, RelationEntry};
use l2_util::ObjectId;
use tracing::warn;

use super::{ServitorState, Summon, SummonAi, SummonKind, SummonTimer};
use crate::npc::NpcClass;
use crate::objects::{Npc, ObjectKind};
use crate::player::CreatureCore;
use crate::timer::TimerKind;
use crate::world::World;

/// `World.forEachVisibleObjectInRange(getOwner(), Player.class, 800, ...)` of `onSpawn`.
const RELATION_RANGE: i32 = 800;

/// What the `Summon` effect hands over (its parameters, Java's `Servitor` setters).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ServitorParams {
    /// `npcId`.
    pub npc_id: i32,
    /// `expMultiplier` (default 1).
    pub exp_multiplier: f32,
    /// `consumeItemId` (default 0) and `consumeItemCount` (default 1).
    pub consume_item: (i32, i32),
    /// `lifeTime` (seconds, default 3600) * 1000.
    pub life_time_ms: i32,
    /// The consume interval in ms after the effect's default (`consumeItemInterval`, else 240 s,
    /// 60 s for siege weapons).
    pub consume_interval_ms: i32,
    /// `skill.getId()`: the reference skill.
    pub reference_skill: i32,
}

impl World {
    /// The `Summon` effect's `onStart` from `new Servitor(template, player)` on: makes the
    /// servitor of `owner` and spawns it next to the owner. `None` when the template is unknown
    /// (Java's constructor throws on a null template) or no object id is left.
    pub fn summon_servitor(&mut self, owner: ObjectId, params: ServitorParams) -> Option<ObjectId> {
        let templates = self.npc_world.templates_arc()?;
        let Some(template) = templates.get(params.npc_id) else {
            warn!("Summon: no NPC template {} for {owner}.", params.npc_id);
            return None;
        };
        let (owner_name, owner_heading) = {
            let player = self.players.get(&owner)?;
            (player.data.name.clone(), player.core.heading)
        };
        let ids = self.npc_world.ids.clone()?;
        let id = match ids.allocate() {
            Ok(id) => id,
            Err(e) => {
                warn!("Summon: cannot make the servitor of {owner}: {e}");
                return None;
            }
        };

        // Summon constructor: "Make sure summon does not spawn in a wall."
        let (x, y, z) = self.position_of(owner)?;
        let instance = self.objects.get(&owner).map_or(0, |e| e.instance_id);
        let dx = self.rng().get(-100, 100);
        let dy = self.rng().get(-100, 100);
        let location = self.geo_valid_location(
            l2_geo::Location::new(x, y, z),
            l2_geo::Location::new(x.wrapping_add(dx), y.wrapping_add(dy), z),
            instance,
        );
        let npc = Npc {
            core: CreatureCore {
                object_id: id,
                x: location.x,
                y: location.y,
                z: location.z,
                // `summon.setHeading(player.getHeading())` before `spawnMe()`.
                heading: owner_heading,
                level: i32::from(template.level),
                cur_hp: 0.0,
                max_hp: 0.0,
                cur_mp: 0.0,
                max_mp: 0.0,
                cur_cp: 0.0,
                max_cp: 0.0,
                // `Formulas.addFuncsToNewCharacter` gives a summon the functions of the standard
                // NPC set, so it starts with that set (copied on its first own function).
                calculators: self.npc_calculators(),
                skills: Vec::new(),
            },
            npc_id: template.id,
            class: NpcClass::Summon,
            spawn: None,
            leader: None,
            decayed: false,
            raid_minion: false,
            in_water: self.npc_world.is_water_at(location.x, location.y, location.z),
            right_hand: 0,
            left_hand: 0,
            enchant_effect: template.weapon_enchant,
            display_effect: 0,
            collision_radius: f64::from(template.collision_radius),
            must_reward_exp_sp: false,
            special_drop: false,
            movement: crate::movement::NpcMovement::default(),
            spawn_point: (0, 0, 0),
            spawn_time_ms: 0,
        };
        self.npcs.insert(id, npc);
        self.forget_effects(id);
        self.forget_status(id);
        // The constructor copies the template's skills.
        self.add_npc_template_skills(id);

        // `Summon` effect: the consume interval, name, title, reference skill, exp multiplier, life
        // time and consumed item.
        let summon = Summon {
            owner,
            owner_offline: false,
            kind: SummonKind::Servitor,
            name: template.name.clone(),
            title: owner_name,
            follow: true,
            previous_follow: true,
            restore_summon: true,
            attack_range: 36,
            show_summon_animation: true,
            exp: 0,
            sp: 0,
            ai: SummonAi {
                // `_startFollow = getFollowStatus()` when the AI is made: true.
                start_follow: true,
                ..SummonAi::default()
            },
            abnormal_effect_timer: None,
            pet: None,
            servitor: Some(ServitorState {
                exp_multiplier: params.exp_multiplier,
                consume_item: params.consume_item,
                life_time: params.life_time_ms,
                life_time_remaining: params.life_time_ms,
                consume_interval: params.consume_interval_ms,
                consume_interval_remaining: params.consume_interval_ms,
                life_timer: None,
                reference_skill: params.reference_skill,
            }),
        };
        self.summons.summons.insert(id, summon);
        // The AI is made in the constructor (`getAI()`).
        self.ai.creatures.insert(id, crate::ai::AiState::default());

        // The exp of its level: `getExpForLevel(maxPetLevel - 1)` at or above the highest pet
        // level, else `getExpForLevel(level % maxPetLevel)`.
        let level = i32::from(template.level);
        if let Some(max_pet_level) = self.exp_table_max_pet_level() {
            let exp_level = if level >= max_pet_level {
                max_pet_level - 1
            } else {
                level % max_pet_level
            };
            let exp = self.exp_needed_for_level(exp_level).unwrap_or(0);
            if let Some(state) = self.summon_mut(id) {
                state.exp = exp;
            }
        }

        // `setXYZInvisible` + the instance of the owner, then the object joins the world table.
        self.register_object(id, ObjectKind::Summon);
        if let Some(entry) = self.objects.get_mut(&id) {
            entry.instance_id = instance;
        }

        // `summon.fullRestore()`: HP and MP to the maximum (the regeneration starts if needed).
        if let Some((hp, mp)) = self
            .stats(id)
            .map(|v| (f64::from(v.max_hp()), f64::from(v.max_mp())))
        {
            if let Some(npc) = self.npcs.get_mut(&id) {
                npc.core.max_hp = hp;
                npc.core.max_mp = mp;
            }
            self.set_current_hp(id, hp, true);
            self.set_current_mp(id, mp, true);
        }
        // `player.setPet(summon)`, then `summon.spawnMe()`.
        self.set_pet(owner, Some(id));
        self.spawn(id);
        if !self.npcs.contains_key(&id) {
            // Out of the world: `spawn` disposed of it.
            self.set_pet(owner, None);
            self.summons.summons.remove(&id);
            ids.release(id);
            return None;
        }
        // `summon.setRunning()`.
        self.summon_set_running(id, true);
        Some(id)
    }

    /// Java `Summon.onSpawn()` then `Servitor.onSpawn()`; called by `spawn` for every object.
    pub(crate) fn summon_on_spawn(&mut self, id: ObjectId) {
        let Some(owner) = self.summon_owner(id) else {
            return;
        };
        // `restoreEffects()` unless the summon comes back from a teleport.
        if self.summons.settings.summon_store_skill_cooltime && !self.is_teleporting(id) {
            if self.is_pet(id) {
                self.pet_restore_effects(id);
            } else {
                self.servitor_restore_effects(id);
            }
        }
        self.set_follow_status(id, true);
        self.update_and_broadcast_summon_status(id, 0);
        // `sendPacket(new RelationChanged(this, _owner.getRelation(_owner), false))` to the
        // owner, then the players within 800 of the owner.
        self.send_summon_relation(id, owner, Some(false));
        let viewers: Vec<ObjectId> = self
            .visible_objects_in_range(owner, RELATION_RANGE)
            .into_iter()
            .filter(|o| self.objects.get(o).is_some_and(|e| e.kind == ObjectKind::Player))
            .collect();
        for viewer in viewers {
            self.send_summon_relation(id, viewer, None);
        }
        self.summon_party_window_add(id);

        // "AddVisibleObject created the info packets with summon animation."
        // "If someone comes into range now, the animation shouldn't show any more."
        if let Some(state) = self.summon_mut(id) {
            state.show_summon_animation = false;
            state.restore_summon = false;
        }
        self.summon_recharge_shots(id, true, true);
        // "Notify to scripts." (`ON_PLAYER_SUMMON_SPAWN`, S-API-05)
        self.script_on_summon_spawn(id);

        // `BabyPet.onSpawn`: the baby pet sorts its skills and starts to heal.
        self.baby_on_spawn(id);

        // `Servitor.onSpawn`: the life task, at a fixed rate of 5 s from now.
        // NOTE: Java's first run has a delay of 0 and runs at once on the thread pool; here it runs
        // on the next tick.
        let has_task = self
            .summon(id)
            .and_then(|s| s.servitor.as_ref())
            .is_none_or(|s| s.life_timer.is_some());
        if !has_task {
            let timer = self
                .timers
                .schedule_at(self.tick + 1, TimerKind::Summon(SummonTimer::Life(id)));
            if let Some(servitor) = self.summon_mut(id).and_then(|s| s.servitor.as_mut()) {
                servitor.life_timer = Some(timer);
            }
        }
    }

    /// `new RelationChanged(summon, owner.getRelation(viewer), autoAttackable)` to `viewer`.
    /// `auto_attackable` `None` = `isAutoAttackable(viewer)` of the summon.
    fn send_summon_relation(&self, summon: ObjectId, viewer: ObjectId, auto_attackable: Option<bool>) {
        let entry = self.summon_relation_entry(summon, viewer, auto_attackable);
        self.send(viewer, &RelationChanged::single(entry));
    }

    /// `new RelationChanged(summon, owner.getRelation(viewer), autoAttackable)`: the owner's
    /// relation, karma and PvP flag. `auto_attackable` `None` = `owner.isAutoAttackable(viewer)`.
    pub(crate) fn summon_relation_entry(
        &self,
        summon: ObjectId,
        viewer: ObjectId,
        auto_attackable: Option<bool>,
    ) -> RelationEntry {
        let owner = self.summon_owner(summon).unwrap_or(summon);
        RelationEntry {
            object_id: summon.0,
            relation: self.relation(owner, viewer),
            auto_attackable: auto_attackable
                .unwrap_or_else(|| self.summon_auto_attackable_by(summon, viewer)),
            karma: self.players.get(&owner).map_or(0, |p| p.data.karma),
            pvp_flag: self.pvp_flag_of(owner),
        }
    }

    /// Java `Creature.setRunning(true/false)` for a summon: `ChangeMoveType`, then
    /// `broadcastStatusUpdate()` (which for a summon is `updateAndBroadcastStatus(1)` too).
    pub(crate) fn summon_set_running(&mut self, id: ObjectId, value: bool) {
        let Some(npc) = self.npcs.get_mut(&id) else {
            return;
        };
        if npc.movement.running == value {
            return;
        }
        npc.movement.running = value;
        if self.stats(id).is_some_and(|s| s.run_speed() != 0.0) {
            self.broadcast_packet(
                id,
                &l2_protocol_game::server::ChangeMoveType {
                    object_id: id.0,
                    running: value,
                },
            );
        }
        self.broadcast_status_update(id);
    }
}
