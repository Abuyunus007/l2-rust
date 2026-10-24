//! The castle and fortress doors and walls in a siege: their upgrades (`castle_doorupgrade`), the
//! respawn at the start and the end of a siege (weaker after an owner change), who may attack
//! them, the damage they take (walls only from siege weapons), their destruction, the defenders a
//! hit door calls, and the upgrades a castle loses with its owner during a siege.
// Java: mechanics/siege/Castle.java (spawnDoor, setDoorUpgrade, loadDoorUpgrade,
//       removeDoorUpgrade, removeUpgrade)
// Java: entity/actor/instance/Door.java (isAutoAttackable, isEnemy, getCastle, getFort,
//       reduceCurrentHp, reduceCurrentHpByDOT, doDie)
// Java: entity/actor/stat/DoorStat.java (getMaxHp, setUpgradeHpRatio)
// Java: ai/DoorAI.java (notifyActionAttacked)
//
// A door is a creature of the stat engine and of `status.rs` (see `door.rs`); the generic attack,
// skill and death code reaches the door parts here: `reduce_current_hp` hands a door to
// [`World::door_reduce_current_hp`], `do_die` to [`World::door_do_die`], `notify_action_attacked`
// to [`World::door_notify_attacked`].
//
// M7-CHSIEGE-01: the doors of a siegable clan hall (`_clanHall.isSiegableHall()`): their
// `isEnemy`, `isAutoAttackable` (`doorIsAutoAttackable`, `checkIsAttacker`) and the "gate
// destroyed" message while the hall is in siege.

use l2_data::npc::NpcRace;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;

use crate::residence::{CastleId, FortId, ResidenceRef, ResidenceWrite, Writes};
use crate::world::World;

/// `DoorAI.notifyActionAttacked`: the hate a defender near a hit door gets for the attacker.
const DOOR_CALL_AGGRO: i64 = 15;

impl World {
    /// Java `Castle.setDoorUpgrade(doorId, ratio, save)`: the door's maximum HP is multiplied by
    /// `ratio` and the door is healed to it; `save` writes the row (the chamberlain's upgrade, a
    /// player's purchase).
    pub fn castle_set_door_upgrade(&mut self, id: CastleId, door_id: i32, ratio: i32, save: bool) {
        // `getDoors().isEmpty() ? DoorData.getDoor(doorId) : getDoor(doorId)`.
        let door = if self.castle(id).is_some_and(|c| c.doors.is_empty()) {
            self.door_by_door_id(door_id).map(|d| d.object_id)
        } else {
            self.residence_door(ResidenceRef::Castle(id), door_id)
        };
        let Some(door) = door else {
            return;
        };
        self.set_door_upgrade_hp_ratio(door, ratio);
        let max = self.door(door).map_or(0, |d| d.max_hp());
        self.set_door_current_hp(door, f64::from(max));
        if save {
            self.write_residence(
                Writes::Player,
                ResidenceWrite::CastleDoorUpgrade {
                    door_id,
                    ratio,
                    castle_id: id.0,
                },
            );
        }
    }

    /// Java `Castle.removeDoorUpgrade()`: every door back to its own HP, the rows go (a siege
    /// job).
    fn castle_remove_door_upgrade(&mut self, id: CastleId) {
        for door in self.residence_doors(ResidenceRef::Castle(id)) {
            self.set_door_upgrade_hp_ratio(door, 1);
            let hp = self.door(door).map_or(0.0, |d| d.current_hp());
            self.set_door_current_hp(door, hp);
        }
        self.siege_job_write(ResidenceWrite::CastleDoorUpgradesDelete { castle_id: id.0 });
    }

    /// Java `Castle.removeUpgrade()`: the door and trap upgrades and every function go.
    pub(crate) fn castle_remove_upgrade(&mut self, id: CastleId) {
        self.castle_remove_door_upgrade(id);
        self.castle_remove_trap_upgrade(id);
        self.residence_remove_all_functions(ResidenceRef::Castle(id));
    }

    /// Java `Castle.spawnDoor(isDoorWeak)`: a destroyed door comes back (`doRevive`) with all
    /// its HP (half after an owner change), an open one closes.
    pub fn castle_spawn_door(&mut self, id: CastleId, weak: bool) {
        for door in self.residence_doors(ResidenceRef::Castle(id)) {
            if self.is_dead(door) {
                self.do_revive(door);
                let max = self.stats(door).map_or(0, |v| v.max_hp());
                let hp = if weak { max / 2 } else { max };
                self.set_door_current_hp(door, f64::from(hp));
            }
            if self.door(door).is_some_and(|d| d.open) {
                self.close_door(door);
            }
        }
    }

    /// The siege zone of a castle went on or off: its doors with an HP bar are enemies
    /// (`Door.isEnemy()`) while it is on.
    pub(crate) fn castle_doors_set_enemy(&mut self, id: CastleId, active: bool) {
        for door in self.residence_doors(ResidenceRef::Castle(id)) {
            if let Some(d) = self.doors.get_mut(&door) {
                d.siege_enemy = active && d.template.show_hp;
            }
        }
    }

    /// The same for a fortress (`getFort().getZone().isActive() && isShowHp()`).
    pub(crate) fn fort_doors_set_enemy(&mut self, id: FortId, active: bool) {
        for door in self.residence_doors(ResidenceRef::Fort(id)) {
            if let Some(d) = self.doors.get_mut(&door) {
                d.siege_enemy = active && d.template.show_hp;
            }
        }
    }

    // ------------------------------------------------------------------ damage and death

    /// Java `Door.reduceCurrentHp(damage, attacker, awake, isDOT, skill)`: a wall (outside an
    /// instance) takes damage only from a siege weapon servitor; then `Creature.reduceCurrentHp`
    /// goes to `CreatureStatus.reduceHp` (a door is no player and no champion).
    pub(crate) fn door_reduce_current_hp(&mut self, door: ObjectId, amount: f64, attacker: Option<ObjectId>) {
        let Some(is_wall) = self.door(door).map(|d| d.is_wall()) else {
            return;
        };
        let in_instance = self.objects.get(&door).is_some_and(|e| e.instance_id != 0);
        if is_wall && !in_instance {
            // NOTE(java-quirk): Java calls `attacker.isServitor()` without a null check, so a hit
            // without an attacker throws there; here it does nothing either.
            let Some(attacker) = attacker else {
                return;
            };
            if !self.is_servitor(attacker) {
                return;
            }
            if self
                .npc_template(attacker)
                .is_none_or(|t| t.race != NpcRace::SiegeWeapon)
            {
                return;
            }
        }
        self.reduce_hp(door, amount, attacker);
    }

    /// The rest of Java `Door.doDie(killer)` after `Creature.doDie`: everybody who sees the door
    /// is told "The castle gate has been destroyed." when it falls in a castle or fortress siege.
    pub(crate) fn door_do_die(&mut self, door: ObjectId) {
        let Some((x, y, z)) = self.door(door).map(|d| (d.x, d.y, d.z)) else {
            return;
        };
        // `getFort() != null && getFort().getResidenceId() > 0 && getFort().getSiege().isInProgress()`.
        let is_fort = self
            .fort_at(x, y, z)
            .is_some_and(|f| f.0 > 0 && self.fort_siege_in_progress(f));
        let is_castle = self
            .castle_at(x, y, z)
            .is_some_and(|c| c.0 > 0 && self.is_castle_siege_in_progress(c));
        // M7-CHSIEGE-01: `isHall` (a siegable hall in siege).
        if is_fort || is_castle {
            self.broadcast_packet(
                door,
                &SystemMessage::new(SystemMessageId::TheCastleGateHasBeenDestroyed),
            );
        }
    }

    /// Java `DoorAI.notifyActionAttacked(attacker)`: every defender that sees the door and stands
    /// within its clan help range of it is told about the attacker
    /// (`notifyActionAggression(attacker, 15)`).
    // NOTE: Java runs the loop on the thread pool (`ThreadPool.execute`); here it runs at once,
    // in the same tick.
    pub(crate) fn door_notify_attacked(&mut self, door: ObjectId, attacker: ObjectId) {
        if !self.objects.get(&attacker).is_some_and(|e| e.kind.is_creature()) {
            return;
        }
        // `World.forEachVisibleObject(door, Defender.class, ...)`: `FortCommander` extends
        // `Defender`.
        let guards: Vec<ObjectId> = self
            .visible_objects(door)
            .into_iter()
            .filter(|id| self.is_siege_guard(*id))
            .collect();
        for guard in guards {
            let range = self.npc_template(guard).map_or(0, |t| t.clan_help_range);
            if self.dist_3d(door, guard) < f64::from(range) {
                // `guard.getAI()` makes the AI.
                self.wake_ai(guard);
                self.notify_action_aggression(guard, attacker, DOOR_CALL_AGGRO);
            }
        }
    }

    /// Java `Door.isAutoAttackable(attacker)` for a playable attacker: an attackable door always;
    /// a door with an HP bar of a castle (fortress) whose siege zone is active, for everybody but
    /// the owning clan.
    pub(crate) fn door_auto_attackable(&self, door: ObjectId, attacker: ObjectId) -> bool {
        // Doors can't be attacked by NPCs.
        if !self.is_playable(attacker) {
            return false;
        }
        let Some(d) = self.doors.get(&door) else {
            return false;
        };
        if d.attackable_door {
            return true;
        }
        if !d.template.show_hp {
            return false;
        }
        let Some(player) = self.acting_player(attacker) else {
            return false;
        };
        // The doors of a siegable hall (`doorIsAutoAttackable`, M7-CHSIEGE-01).
        if d.template.clanhall_id > 0 && self.is_siegable_hall_id(d.template.clanhall_id) {
            let hall = crate::residence::ClanHallId(d.template.clanhall_id);
            return self.hall_is_in_siege(hall)
                && self.hall_door_is_auto_attackable(hall)
                && self.hall_siege_is_attacker(hall, self.clan_id_of(player));
        }
        // M7-TW-01: during Territory War, everybody but the ally field.
        // Attackable only during siege by everyone (not owner).
        let castle = self.castle_at(d.x, d.y, d.z).filter(|c| c.0 > 0).filter(|c| {
            self.castle(*c)
                .and_then(|c| c.siege_zone)
                .is_some_and(|z| self.is_siege_zone_active(z))
        });
        // `isFort`: the fortress whose zone is active (M7-FORT-01).
        let fort = self
            .fort_at(d.x, d.y, d.z)
            .filter(|f| f.0 > 0)
            .filter(|f| self.fort_zone_is_active(*f));
        let clan = self.clan_id_of(player);
        if let Some(fort) = fort {
            if clan != 0 && self.fort(fort).is_some_and(|f| f.owner_id == clan) {
                return false;
            }
        } else if let Some(castle) = castle
            && clan != 0
            && self.castle(castle).is_some_and(|c| c.owner_id == clan)
        {
            return false;
        }
        castle.is_some() || fort.is_some()
    }
}
