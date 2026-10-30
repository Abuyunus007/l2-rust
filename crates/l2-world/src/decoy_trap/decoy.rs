//! The decoy: a copy of its owner that draws the hate of monsters around it.
// Java: entity/actor/instance/Decoy.java, dist/game/data/scripts/handlers/skill/effects/SummonNpc.java
//       (the "Decoy" branch), handlers/actions/click/DecoyClick.java,
//       network/serverpackets/CharInfo.java (the `Decoy` constructor)

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{CharInfo, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use super::{DecoyState, DecoyTrapTimer};
use crate::effects::SkillKey;
use crate::npc::NpcClass;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// `Decoy`'s `hateSpamSkillId`.
pub const DECOY_HATE_SKILL_ID: i32 = 5272;

impl World {
    /// `SummonNpc.onStart`'s "Decoy" case from `new Decoy(npcTemplate, player, despawnDelay)` on:
    /// the decoy of `owner`, spawned at the owner. `None` for an unknown template or when no object
    /// id is left. A decoy the owner had before is not touched: `setDecoy` replaces the reference
    /// and the old one lives out its life time.
    pub fn summon_decoy(
        &mut self,
        owner: ObjectId,
        npc_id: i32,
        total_life_time_ms: i32,
    ) -> Option<ObjectId> {
        let templates = self.npc_world.templates_arc()?;
        let template = templates.get(npc_id)?;
        let (x, y, z) = self.position_of(owner)?;
        let heading = self.players.get(&owner)?.core.heading;
        let instance = self.objects.get(&owner).map_or(0, |e| e.instance_id);
        // `setXYZInvisible(owner)`, then `decoy.spawnMe(player.getX(), ...)` at the same place.
        let id = self.make_special_npc(template, NpcClass::Decoy, (x, y, z), heading, instance)?;

        // The constructor: the life task, and the hate spam (`aggressive`, always true here) every
        // 5 s after 2 s.
        let life = self.timers.schedule_at(
            self.tick + TimerWheel::ms_to_ticks(u64::try_from(total_life_time_ms).unwrap_or(0)),
            TimerKind::DecoyTrap(DecoyTrapTimer::DecoyLife(id)),
        );
        let hate = self.timers.schedule_at(
            self.tick + TimerWheel::ms_to_ticks(2000),
            TimerKind::DecoyTrap(DecoyTrapTimer::DecoyHate(id)),
        );
        self.decoy_trap.decoys.insert(
            id,
            DecoyState {
                owner: Some(owner),
                life_timer: Some(life),
                hate_timer: Some(hate),
            },
        );
        self.spawn_at(id, x, y, z);
        if !self.npcs.contains_key(&id) {
            return None;
        }
        // `decoy.setSummoner(player)`: after the spawn here, because the world's `Npc.onSpawn`
        // resets the links, which a `Creature` does not.
        self.set_npc_summoner(id, owner);
        // `Decoy.onSpawn`: `sendPacket(new CharInfo(this, false))` reaches the owner.
        if let Some(info) = self.decoy_char_info(id, owner) {
            self.send(owner, &info);
        }
        // `player.setDecoy(decoy)`.
        self.decoy_trap.decoy_of.insert(owner, id);
        Some(id)
    }

    /// `new CharInfo(decoy, false)` for `viewer`: the owner's look at the decoy's place.
    pub(crate) fn decoy_char_info(&self, decoy: ObjectId, viewer: ObjectId) -> Option<CharInfo> {
        let owner = self.decoy_owner(decoy)?;
        let mut info = self.char_info(owner, viewer)?;
        let npc = self.npcs.get(&decoy)?;
        info.object_id = decoy.0;
        info.x = npc.core.x;
        info.y = npc.core.y;
        info.z = npc.core.z;
        info.heading = npc.core.heading;
        Some(info)
    }

    /// `Decoy.sendInfo(player)`.
    pub(crate) fn send_decoy_info(&mut self, decoy: ObjectId, viewer: ObjectId) {
        if !self.is_visible_for(decoy, viewer) {
            return;
        }
        if let Some(info) = self.decoy_char_info(decoy, viewer) {
            self.send(viewer, &info);
        }
    }

    /// `Decoy.isAutoAttackable(attacker)`: the owner's answer.
    pub(crate) fn decoy_auto_attackable(&self, decoy: ObjectId, attacker: ObjectId) -> bool {
        self.decoy_owner(decoy)
            .is_some_and(|owner| self.is_auto_attackable(owner, attacker))
    }

    /// `Decoy.unSummon(owner)`: the hate task stops; a living decoy leaves the world and the owner
    /// forgets it; the life task is cancelled.
    pub(crate) fn decoy_unsummon(&mut self, decoy: ObjectId) {
        let Some(state) = self.decoy_trap.decoys.get_mut(&decoy) else {
            return;
        };
        let (hate, life) = (state.hate_timer.take(), state.life_timer.take());
        if let Some(timer) = hate {
            self.timers.cancel(timer);
        }
        let spawned = self.objects.get(&decoy).is_some_and(|e| e.spawned);
        if spawned && !self.is_dead(decoy) {
            // `owner.setDecoy(null)` and `decayMe()`.
            self.on_npc_decay(decoy);
        }
        if let Some(timer) = life {
            self.timers.cancel(timer);
        }
    }

    /// The life task.
    pub(crate) fn decoy_life_task(&mut self, decoy: ObjectId) {
        if let Some(state) = self.decoy_trap.decoys.get_mut(&decoy) {
            state.life_timer = None;
        }
        self.decoy_unsummon(decoy);
    }

    /// `Decoy.HateSpam.run()`: the decoy targets itself and casts "Decoy Provocation". Reschedules
    /// itself (`scheduleAtFixedRate`).
    pub(crate) fn decoy_hate_spam(&mut self, decoy: ObjectId) {
        let Some(state) = self.decoy_trap.decoys.get_mut(&decoy) else {
            return;
        };
        let due = self.tick + TimerWheel::ms_to_ticks(5000);
        state.hate_timer = Some(
            self.timers
                .schedule_at(due, TimerKind::DecoyTrap(DecoyTrapTimer::DecoyHate(decoy))),
        );
        let Some(display_id) = self
            .npcs
            .get(&decoy)
            .and_then(|n| self.npc_world.template(n.npc_id))
            .map(|t| t.display_id)
        else {
            return;
        };
        let Some(table) = self.skill_table() else {
            return;
        };
        // `Math.min(getDisplayId() - 13070, SkillData.getMaxLevel(5272))`.
        let level = (display_id - 13070).min(table.max_level(DECOY_HATE_SKILL_ID));
        if level < 1 {
            // Java: `doCast(null)` throws and the task logs it.
            warn!("Decoy Error: no skill {DECOY_HATE_SKILL_ID} level {level}");
            return;
        }
        self.set_npc_target(decoy, Some(decoy));
        self.do_cast(decoy, SkillKey::new(DECOY_HATE_SKILL_ID, level));
    }

    /// `Decoy.doDie` after `Creature.doDie`: the hate task stops, `unSummon`, then the corpse
    /// decays like any other (`DecayTaskManager.add`).
    pub(crate) fn decoy_do_die(&mut self, decoy: ObjectId) {
        self.decoy_unsummon(decoy);
        self.add_decay_task(decoy);
    }

    /// `DecoyClick.onAction`: the click only targets; a locked target (`TargetMe`) refuses.
    pub(crate) fn decoy_trap_click(&mut self, player: ObjectId, obj: ObjectId) {
        // Aggression target lock effect.
        if self.is_locked_target(player) && self.locked_target(player) != Some(obj) {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::FailedToChangeAttackTarget),
            );
            return;
        }
        self.set_target(player, Some(obj));
    }
}
