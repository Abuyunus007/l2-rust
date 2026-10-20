//! The client's attack click (`AttackRequest`), the forced attack and the peace zone rules.
// Java: network/clientpackets/AttackRequest.java
// Java: entity/WorldObject.java (onForcedAttack, canBeAttacked), entity/actor/Creature.java
//       (onForcedAttack, isInsidePeaceZone), entity/actor/{Npc,Attackable,Playable}.java
//       (canBeAttacked)
// Java: entity/actor/Player.java, entity/actor/instance/Monster.java (isAutoAttackable: the NPC
//       attacker side)
//
// Left out: flood protection, boats, spawn protection
// (`onActionRequest`), private stores and pending requests, olympiad, confusion, the block
// checker arena. Each refuses an attack in Java; none of them exists yet.

use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, SystemMessage};
use l2_util::ObjectId;

use crate::effects::control::ATTACK_ACTION_BLOCK_ID;
use crate::objects::ObjectKind;
use crate::world::World;
use crate::zones::ZoneId;

impl World {
    /// Java `AttackRequest.runImpl`.
    pub(crate) fn on_attack_request(&mut self, id: ObjectId, object_id: i32) {
        if !self.players.contains_key(&id) {
            return;
        }
        // `BOT_PENALTY` effects (`BlockAction`) refuse the attack (`checkCondition(-1)`).
        if !self.check_bot_penalty(id, ATTACK_ACTION_BLOCK_ID) {
            return;
        }
        let wanted = ObjectId(object_id);
        // Avoid using expensive operations if not needed.
        let target = if self.target_of(id) == Some(wanted) {
            Some(wanted)
        } else {
            self.find_object(wanted)
        };
        let Some(target) = target else {
            self.send(id, &ActionFailed);
            return;
        };
        if !self.is_targetable(target) && !self.is_gm(id) {
            self.send(id, &ActionFailed);
            return;
        }
        // Players can't attack objects in the other instances, except from multiverse.
        let target_instance = self.objects.get(&target).map_or(0, |e| e.instance_id);
        let instance = self.objects.get(&id).map_or(0, |e| e.instance_id);
        if target_instance != instance && instance != -1 {
            self.send(id, &ActionFailed);
            return;
        }
        // Only GMs can directly attack invisible characters.
        if !self.is_visible_for(target, id) {
            self.send(id, &ActionFailed);
            return;
        }
        // `player.onActionRequest()`: spawn protection is not modelled.
        if self.target_of(id) != Some(target) {
            // `target.onAction(player)`.
            self.on_action_object(id, target, true);
        } else if target != id {
            // Private stores and pending requests (`getActiveRequester`) would refuse: M5.
            self.on_forced_attack(target, id);
        } else {
            self.send(id, &ActionFailed);
        }
    }

    /// Java `onForcedAttack(player)`: `Creature`'s for creatures, `WorldObject`'s (only
    /// `ActionFailed`) for the rest. Siege objects override it in Java (M5).
    pub(crate) fn on_forced_attack(&mut self, target: ObjectId, player: ObjectId) {
        if !self.objects.get(&target).is_some_and(|e| e.kind.is_creature()) {
            self.send(player, &ActionFailed);
            return;
        }
        if self.target_is_inside_peace_zone(target, player) {
            // If Creature or target is in a peace zone, send a system message TARGET_IN_PEACEZONE
            // a Server->Client packet ActionFailed.
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouMayNotAttackThisTargetInAPeacefulZone),
            );
            self.send(player, &ActionFailed);
            return;
        }
        // Olympiad: M5.
        // `allowPeaceAttack` of the access level is not loaded (no access level has it).
        if let Some(selected) = self.target_of(player)
            && !self.can_be_attacked(selected)
        {
            // If target is not attackable, send a Server->Client packet ActionFailed.
            self.send(player, &ActionFailed);
            return;
        }
        if self.is_confused(player) {
            // If target is confused, send a Server->Client packet ActionFailed.
            self.send(player, &ActionFailed);
            return;
        }
        // The block checker arena is not modelled.
        // Notify AI with ATTACK.
        self.set_intention_attack(player, target);
    }

    /// Java `WorldObject.canBeAttacked()` and its overrides.
    pub(crate) fn can_be_attacked(&self, id: ObjectId) -> bool {
        match self.objects.get(&id).map(|e| e.kind) {
            // `Playable.canBeAttacked()`.
            Some(ObjectKind::Player) => true,
            // `Attackable.canBeAttacked()`, else `Npc.canBeAttacked()` (`AltAttackableNpcs`).
            Some(ObjectKind::Npc) => self.is_attackable_npc(id) || self.settings.combat.alt_attackable_npcs,
            // Doors, static objects and fences keep `WorldObject`'s false (siege doors: M5).
            _ => false,
        }
    }

    /// Java `target.isInsidePeaceZone(Player attacker)`: `isInsidePeaceZone(attacker, target)`
    /// asked on the target.
    pub(crate) fn target_is_inside_peace_zone(&self, target: ObjectId, attacker: ObjectId) -> bool {
        // `!attacker.getAccessLevel().allowPeaceAttack()`: the flag is not loaded (false).
        self.is_inside_peace_zone(target, attacker, target)
    }

    /// Java `Creature.isInsidePeaceZone(WorldObject attacker, WorldObject target)` asked on
    /// `this`: only fights between playables are stopped by peace and no-PvP zones.
    pub(crate) fn is_inside_peace_zone(&self, this: ObjectId, attacker: ObjectId, target: ObjectId) -> bool {
        // `(target.isPlayable() || target.isFakePlayer()) && attacker.isPlayable()`.
        if !(self.is_player(target) && self.is_player(attacker)) {
            return false;
        }
        // Instances (`isPvP`) and territory wards: M5.
        if self.settings.combat.karma_player_can_be_killed_in_peacezone {
            // Allows red to be attacked and red to attack flagged players.
            // NOTE(java-quirk): Java asks `asPlayer()` of `this` (the creature the method runs on),
            // which is the attacker for `isInsidePeaceZone(this, target)`.
            let player_karma = self.players.get(&this).map(|p| p.data.karma);
            if player_karma.is_some_and(|k| k > 0) {
                return false;
            }
            let attacker_karma = self.players.get(&attacker).map_or(0, |p| p.data.karma);
            // `player.getPvpFlag() > 0`.
            let pvp_flag = self.pvp_flag_of(this);
            if attacker_karma > 0 && player_karma.is_some() && pvp_flag > 0 {
                return false;
            }
        }
        self.is_inside_zone(target, ZoneId::Peace)
            || self.is_inside_zone(attacker, ZoneId::Peace)
            || self.is_inside_zone(target, ZoneId::NoPvp)
            || self.is_inside_zone(attacker, ZoneId::NoPvp)
    }

    /// Java `target.isAutoAttackable(attacker)` for any attacker: a player's view is
    /// `World::is_auto_attackable_by`; an NPC attacker sees players as enemies when it is a
    /// monster and other NPCs never (`Player.isAutoAttackable`, `Monster.isAutoAttackable`).
    pub(crate) fn is_auto_attackable_for(&self, target: ObjectId, attacker: ObjectId) -> bool {
        if self.is_player(attacker) {
            return self.is_auto_attackable_by(target, attacker);
        }
        if target == attacker {
            return false;
        }
        match self.objects.get(&target).map(|e| e.kind) {
            // Friendly mobs do not attack players; monsters do. M4-AI-01: guards against players
            // with karma, the faction rules.
            Some(ObjectKind::Player) => self.is_monster(attacker),
            // A monster is not an enemy of a monster; guards and siege NPCs: M4-AI-01 / M5.
            _ => false,
        }
    }
}
