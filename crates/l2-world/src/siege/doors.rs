//! The castle doors and walls in a siege: their upgrades (`castle_doorupgrade`), the respawn at
//! the start and the end of a siege (weaker after an owner change), who may attack them, and the
//! upgrades a castle loses with its owner during a siege.
// Java: mechanics/siege/Castle.java (spawnDoor, setDoorUpgrade, loadDoorUpgrade,
//       removeDoorUpgrade, removeUpgrade)
// Java: entity/actor/instance/Door.java (isAutoAttackable, isEnemy, getCastle)
// Java: entity/actor/stat/DoorStat.java (getMaxHp, setUpgradeHpRatio)
//
// FOLLOW-UP(M7-SIEGE-02): a door is no creature of the stat engine yet, so the attack and skill
// code cannot hurt it (`Door.reduceCurrentHp`, `doDie` with "The castle gate has been destroyed.").
// The rules of who may attack a door are here already.

use l2_util::ObjectId;

use crate::residence::{CastleId, ResidenceRef, ResidenceWrite, Writes};
use crate::world::World;

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
        let max = self.door(door).map_or(0, |d| d.max_hp);
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
            let hp = self.door(door).map_or(0.0, |d| d.current_hp);
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

    /// Java `Castle.spawnDoor(isDoorWeak)`: a destroyed door comes back with all its HP (half
    /// after an owner change), an open one closes.
    pub fn castle_spawn_door(&mut self, id: CastleId, weak: bool) {
        for door in self.residence_doors(ResidenceRef::Castle(id)) {
            let Some((dead, open, max)) = self.door(door).map(|d| (d.dead, d.open, d.max_hp)) else {
                continue;
            };
            if dead {
                self.revive_door(door);
                let hp = if weak { max / 2 } else { max };
                self.set_door_current_hp(door, f64::from(hp));
            }
            if open {
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
        // M7-CHSIEGE-01: the doors of a siegable hall (`doorIsAutoAttackable`).
        // M7-TW-01: during Territory War, everybody but the ally field.
        // M7-FORT-01: the doors of a fortress whose zone is active (`isFort`).
        let castle = self.castle_at(d.x, d.y, d.z).filter(|c| c.0 > 0).filter(|c| {
            self.castle(*c)
                .and_then(|c| c.siege_zone)
                .is_some_and(|z| self.is_siege_zone_active(z))
        });
        if let Some(castle) = castle {
            let clan = self.clan_id_of(player);
            if clan != 0 && self.castle(castle).is_some_and(|c| c.owner_id == clan) {
                return false;
            }
            return true;
        }
        false
    }
}
