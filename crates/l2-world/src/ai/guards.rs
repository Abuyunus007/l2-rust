//! Guards and siege defenders: what `Guard`, `QuestGuard`, `Defender` and `FortCommander` add to
//! an `Attackable`, and who attacks whom once NPCs can fight NPCs.
// Java: entity/actor/instance/{Guard,QuestGuard,Defender,FortCommander}.java
// Java: entity/actor/instance/Monster.java (isAutoAttackable: the NPC attacker side)
// Java: entity/actor/instance/{FriendlyMob,Block}.java (isAutoAttackable)
// Java: entity/actor/Player.java (isAutoAttackable: the `Guard` and `FriendlyMob` attackers)
//
// # Which AI runs
// A `Guard` (and `QuestGuard`) uses the `AttackableAI` of every attackable (`Attackable.initAI`);
// what is different is the `Guard` branches inside it (`attackable.rs`: `isAggressiveTowards`,
// `targetReconsider`; `npc_skills.rs`) and the overrides here. A `Defender` (and its
// `FortCommander`) would use `SiegeGuardAI`, `FortSiegeGuardAI` or `SpecialSiegeGuardAI`
// depending on the castle, fort or siegable hall it stands in.
// The siege rules of `Defender.isAutoAttackable` / `addDamageHate` and its click are the castle
// siege's (`siege/npcs.rs`, M7-SIEGE-01; the fortress and hall parts: their cards).
// FOLLOW-UP(M7-SIEGE-02): the three siege guard AIs; until then a defender has the attackable
// AI. `FortCommander`'s speech and `killedCommander`: M7-FORT-01.
//
// # Left out
// - The faction system's guards (`FactionSystemConfig.FACTION_GUARDS_ENABLED`, a Custom config
//   that is off by default).
// - Scripts: `QuestGuard.setPassive` / `setAutoAttackable` and its `ON_ATTACKABLE_*` events (D-05).
// - Fake players.

use l2_util::ObjectId;

use crate::npc::NpcClass;
use crate::target::distance_2d;
use crate::world::World;

/// `Guard.addDamage`: "World.forEachVisibleObjectInRange(this, Guard.class, 500, ...)".
const GUARD_CALL_RANGE: i32 = 500;
/// `Guard.addDamage`: the hate a guard (and every guard near it) gets from the attacker.
const GUARD_HATE: i64 = 10;
/// `Defender.returnHome`: how close to the spawn is "home".
const DEFENDER_HOME_RADIUS: f64 = 40.0;
/// `FortCommander.returnHome`.
const COMMANDER_HOME_RADIUS: f64 = 200.0;

impl World {
    fn npc_class(&self, id: ObjectId) -> Option<NpcClass> {
        self.npcs.get(&id).map(|n| n.class)
    }

    /// Java `instanceof Guard`.
    pub(crate) fn is_guard(&self, id: ObjectId) -> bool {
        self.npc_class(id).is_some_and(NpcClass::is_guard)
    }

    /// Java `Attackable.canReturnToSpawnPoint()`: `QuestGuard` turns it off.
    pub(crate) fn can_return_to_spawn_point(&self, id: ObjectId) -> bool {
        self.npc_class(id) != Some(NpcClass::QuestGuard)
    }

    /// Java `Guard.onSpawn()` after `Attackable.onSpawn()`: "getAI().setIntentionActive()". The
    /// commented-out region check of Java is not there either, so a guard thinks from its spawn
    /// whether a player is near or not (its think returns at once while its region sleeps).
    // NOTE(java-quirk): `Guard.onSpawn` calls `setRandomWalking(false)` *before* `super.onSpawn()`,
    // and `Npc.onSpawn` sets the flag from the template again, so a guard whose template walks
    // does walk.
    // NOTE(java-quirk): `World.addVisibleObject` wakes only `isMonster()` NPCs. A guard whose
    // region switched off was sent idle with its AI detached (`WorldRegion.switchAI`), and nothing
    // wakes it again until something hits it (or it respawns).
    // DEVIATION(performance): Java puts the guard in the think set at once, where its think returns
    // at the first line while no player is near. With ~42,000 NPCs only those near players may
    // cost anything (`the_real_world_thinks_only_near_players`), so a guard that spawns where no
    // neighbouring region is active stays out of the set, with its AI and its ACTIVE intention,
    // and joins when a region around it turns on ([`World::wake_guards_around`]). The behaviour
    // is the same: it thinks exactly from the moment Java's think would get past that check.
    pub(crate) fn guard_on_spawn(&mut self, id: ObjectId) {
        if !self.is_guard(id) {
            return;
        }
        self.set_intention_active(id);
        if !self.are_region_neighbors_active(id) {
            self.stop_ai_task(id);
        }
    }

    /// A region turned on: the guards around it that Java would have thinking since their spawn
    /// join the think set (see [`World::guard_on_spawn`]). A guard whose AI was detached when its
    /// region switched off (`WorldRegion.switchAI`) is not one of them.
    pub(crate) fn wake_guards_around(&mut self, region: crate::region::RegionId) {
        for r in region.surrounding() {
            let guards: Vec<ObjectId> = self
                .regions
                .objects(r)
                .filter(|id| {
                    self.is_guard(*id) && self.has_ai(*id) && !self.is_dead(*id) && !self.is_thinking_npc(*id)
                })
                .collect();
            for guard in guards {
                self.start_ai_task(guard);
            }
        }
    }

    /// The rest of Java `Guard.addDamage` after `super.addDamage`: the guard and every guard
    /// within 500 follow the attacker and hate it.
    pub(crate) fn guard_add_damage(&mut self, id: ObjectId, attacker: ObjectId) {
        if !self.is_guard(id) {
            return;
        }
        self.wake_ai(id);
        self.start_follow(id, attacker, -1);
        self.add_damage_hate(id, attacker, 0, GUARD_HATE);
        for guard in self.visible_objects_in_range(id, GUARD_CALL_RANGE) {
            if !self.is_guard(guard) {
                continue;
            }
            // `guard.getAI()` makes the AI.
            self.wake_ai(guard);
            self.start_follow(guard, attacker, -1);
            self.add_damage_hate(guard, attacker, 0, GUARD_HATE);
        }
    }

    /// The overrides of `addDamageHate` that refuse an attacker: `Defender` and `FortCommander`
    /// take no hate from defenders, `QuestGuard` none from players. (Both also have a siege rule
    /// for players; sieges are not ported.)
    pub(crate) fn npc_refuses_hate(
        &self,
        npc: ObjectId,
        attacker: ObjectId,
        damage: i64,
        aggro: i64,
    ) -> bool {
        match self.npc_class(npc) {
            Some(NpcClass::Defender) => {
                self.npc_class(attacker).is_some_and(NpcClass::is_defender)
                    || self.defender_refuses_hate(npc, attacker, damage, aggro)
            }
            Some(NpcClass::FortCommander) => self.npc_class(attacker).is_some_and(NpcClass::is_defender),
            // `!_isPassive && !attacker.isPlayer()`; `setPassive` is a script call.
            Some(NpcClass::QuestGuard) => self.is_player(attacker),
            _ => false,
        }
    }

    /// Java `target.isAutoAttackable(attacker)` for an NPC `attacker` and an NPC `target`.
    pub(crate) fn npc_auto_attackable_by_npc(&self, target: ObjectId, attacker: ObjectId) -> bool {
        let (Some(target_class), Some(attacker_class)) = (self.npc_class(target), self.npc_class(attacker))
        else {
            return false;
        };
        match target_class {
            // `Block.isAutoAttackable`: "return true" for anybody but a player.
            NpcClass::Block => true,
            // `Monster.isAutoAttackable`.
            NpcClass::Monster | NpcClass::RaidBoss | NpcClass::GrandBoss => {
                // Check if the Monster target is aggressive.
                if self.ai.attackable.settings.guard_attack_aggro_mob
                    && self.is_aggressive_npc(target)
                    && attacker_class.is_guard()
                {
                    return true;
                }
                // `attacker.isMonster()`: `attacker.isFakePlayer()`.
                if attacker_class.is_monster() {
                    return false;
                }
                // "Anything considers monsters friendly except Players, Attackables (Guards,
                // Friendly NPC), Traps and EffectPoints."
                // NOTE(java-quirk): then `Npc.isAutoAttackable` answers `_isAutoAttackable`, which
                // `Monster`'s constructor set: every attackable that is no monster may fight one.
                self.is_attackable_npc(attacker)
            }
            // `Guard.isAutoAttackable`: "attacker.isMonster() && !attacker.isFakePlayer()", then
            // the faction rule, then `Npc._isAutoAttackable`, false for a guard.
            NpcClass::Guard => attacker_class.is_monster(),
            // `_isAutoAttackable && !attacker.isPlayer()`; `setAutoAttackable` is a script call.
            NpcClass::QuestGuard => true,
            // `FriendlyMob`: only players with karma. `Defender` / `FortCommander`: only players
            // (and only in a siege).
            NpcClass::FriendlyMob
            | NpcClass::Defender
            | NpcClass::FortCommander
            | NpcClass::Npc
            | NpcClass::Summon => false,
        }
    }

    /// Java `Defender.returnHome()` / `FortCommander.returnHome()`: back to the spawn point, but
    /// only when more than 40 (200) away from it. `true` when the NPC is a defender (the caller
    /// has nothing more to do).
    pub(crate) fn defender_return_home(&mut self, id: ObjectId) -> bool {
        let Some(class) = self.npc_class(id).filter(|c| c.is_defender()) else {
            return false;
        };
        if class == NpcClass::Defender && self.stats(id).is_none_or(|s| s.walk_speed() <= 0.0) {
            return true;
        }
        let Some(npc) = self.npcs.get(&id) else {
            return true;
        };
        if npc.spawn.is_none() {
            return true;
        }
        let radius = if class == NpcClass::Defender {
            DEFENDER_HOME_RADIUS
        } else {
            COMMANDER_HOME_RADIUS
        };
        let spawn_point = npc.spawn_point;
        let here = (npc.core.x, npc.core.y, npc.core.z);
        if distance_2d(here, spawn_point) >= radius {
            self.aggro_clear(id);
            if self.has_ai(id) {
                self.npc_set_intention_move_to(id, spawn_point.0, spawn_point.1, spawn_point.2);
            }
        }
        true
    }

    /// The second click of `Guard.onAction` and `Defender.onAction`. `true` when `npc` is one of
    /// them and the click is handled.
    pub(crate) fn guard_second_click(&mut self, player: ObjectId, npc: ObjectId) -> bool {
        let Some(class) = self.npc_class(npc) else {
            return false;
        };
        if class.is_guard() {
            // Check if the Player is in the _aggroList of the GuardInstance.
            if self.aggro_contains(npc, player) {
                // Set the Player Intention to ATTACK.
                self.set_intention_attack(player, npc);
            } else if !self.can_interact(npc, player) {
                // Set the Player Intention to INTERACT.
                self.set_intention_interact(player, npc);
            } else {
                if let Some(p) = self.players.get_mut(&player) {
                    p.talk.last_folk_npc = Some(npc);
                }
                // Open a chat window on client with the text of the GuardInstance.
                self.show_chat_window_default(npc, player);
            }
            return true;
        }
        if class.is_defender() {
            let attackable = self.is_auto_attackable_by(npc, player);
            // This max heigth difference might need some tweaking.
            let dz = match (self.position_of(player), self.position_of(npc)) {
                (Some(p), Some(n)) => (p.2 - n.2).abs(),
                _ => i32::MAX,
            };
            if attackable && !self.is_alike_dead(npc) && dz < 600 {
                self.set_intention_attack(player, npc);
            }
            // Notify the Player AI with INTERACT: a defender never opens a dialog.
            if !attackable && !self.can_interact(npc, player) {
                self.set_intention_interact(player, npc);
            }
            return true;
        }
        false
    }
}
