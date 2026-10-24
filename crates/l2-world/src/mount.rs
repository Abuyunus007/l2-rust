//! Mounts (M6-FU-07): a player rides a strider, a wyvern or a great wolf (`Player.mount`,
//! `dismount`, `mountPlayer`), the mount's food, the no-landing zones and the rented pet.
// Java: entity/actor/Player.java (mount, mountPlayer, dismount, setMount, isMounted, getMountType,
//       startFeed, stopFeed, setCurrentFeed, isHungry, getFeedConsume, storePetFood,
//       enteredNoLanding, exitedNoLanding, checkLandingState, startRentPet, stopRentPet,
//       disarmWeapons, disarmShield)
// Java: entity/actor/enums/player/MountType.java
// Java: entity/actor/tasks/player/{PetFeedTask,DismountTask,RentPetTask}.java
// Java: network/serverpackets/Ride.java
// Java: network/clientpackets/RequestActionUse.java (case 38)
// Java: dist/game/data/scripts/handlers/chat/commands/user/{Mount,Dismount}.java
// Java: dist/game/data/scripts/handlers/skill/effects/Ride.java
// Java: dist/game/data/scripts/handlers/items/PetFood.java (the player branch)
// Java: entity/zone/type/NoLandingZone.java
//
// # How it fits together
// - The state of a mounted player is [`MountState`] in `Player::mount` (Java `_mountType`,
//   `_mountNpcId`, `_mountLevel`, `_mountObjectID`, `_canFeed`, `_curFeed`, `_controlItemId` and
//   the three tasks). The stat engine reads it through `StatView` (speeds, collision, hunger),
//   `UserInfo` / `CharInfo` through `enter.rs`.
// - A wyvern sets the creature's flying flag, which is the one a flying transformation uses
//   (`TransformState::flying`, Java `Creature._isFlying`), and gives the skill Wyvern Breath.
// - The timers are `TimerKind::Mount`: the feed task (every 10 s), the dismount task of a
//   no-landing zone, the rent task and the 1.5 s user info after leaving a water zone.
// - The food of a pet kept in `pets.fed` is written with `SummonWrite::PetFed` (Java
//   `storePetFood`, the only statement the mount adds).
//
// # Left out, with the system that brings it
// - M6-FISHING: `isFishing()` (the fishing card): the mounting check never refuses for it.
// - M8: `isCursedWeaponEquipped()` (cursed weapons): disarming is never refused for it.
// - M7: the siege parts. Mounting in a siege zone is refused for everybody (Java lets the castle
//   lord of the sieged castle ride a strider); `checkLandingState`'s siege branch is left out;
//   the siege zone's wyvern and mount rules are the siege card's.
// - The fall damage a wyvern avoids and the flying movement checks of `Creature` are not
//   modelled by the world yet (see `movement.rs`).

use l2_data::pet::PetLevelData;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{ActionFailed, MagicSkillUse, Ride, SetupGauge, SkillPosition, SystemMessage};
use l2_util::ObjectId;
use tracing::warn;

use crate::inventory::item_name_param;
use crate::outbox::PersistCommand;
use crate::stats::StatView;
use crate::summon::servitor::SummonWrite;
use crate::teleport::TeleportWhereType;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;
use crate::zones::ZoneId;

/// Java `CommonSkill.WYVERN_BREATH` (4289, level 1).
pub const WYVERN_BREATH_ID: i32 = 4289;
const WYVERN_BREATH_LEVEL: i32 = 1;
/// `ThreadPool.scheduleAtFixedRate(new PetFeedTask(this), 10000, 10000)`.
const FEED_INTERVAL_MS: u64 = 10_000;
/// The colour the food bar of a mount uses in `SetupGauge` ("3-green").
const FEED_GAUGE_COLOR: i32 = 3;
/// `ThreadPool.schedule(..., 1500)` after a dismount in water.
const WATER_USER_INFO_DELAY_MS: u64 = 1_500;
/// The `Item` id of the combat flag (`_inventory.getItemByItemId(9819)`).
const COMBAT_FLAG_ID: i32 = 9819;
/// `NoLandingZone.dismountDelay`.
const DEFAULT_DISMOUNT_DELAY: i32 = 5;
/// `LocationUtil.checkIfInRange(200, this, pet, true)` of `mountPlayer`.
const MOUNT_RANGE: i32 = 200;
/// `getZ() > 10000` of `dismount`.
const MAX_DISMOUNT_Z: i32 = 10_000;
/// `getHeight(...) + 300 < getZ()` of `dismount`.
const MAX_DISMOUNT_HEIGHT: i32 = 300;
/// `Player.teleToLocation`: a flying mount does not go below this.
pub const FLYING_MIN_Z: i32 = -1005;

// Java: data/categoryData.xml, the categories `STRIDER`, `WYVERN_GROUP` and `WOLF_GROUP`.
const STRIDER: [i32; 7] = [12526, 12527, 12528, 16038, 16039, 16040, 16068];
const WYVERN_GROUP: [i32; 1] = [12621];
const WOLF_GROUP: [i32; 4] = [16030, 16037, 16041, 16042];

/// The skills `FreeMountsConfig` gives (`Player.rewardSkills`): Twilight Dragon rides a strider,
/// Wind Dragon a wyvern (`Custom/mount_skills.xml`, the `Ride` effect).
pub const FREE_STRIDER_SKILL_ID: i32 = 49991;
pub const FREE_WYVERN_SKILL_ID: i32 = 49992;

/// The NPC ids `//ride_wyvern`, `//ride_strider` and `//ride_wolf` mount (`AdminRide`).
pub const ADMIN_RIDE_WYVERN: i32 = 12621;
pub const ADMIN_RIDE_STRIDER: i32 = 12526;
pub const ADMIN_RIDE_WOLF: i32 = 16041;

/// Java `MountType`, in Java's order (the ordinal goes into `UserInfo` and `Ride`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MountType {
    #[default]
    None,
    Strider,
    Wyvern,
    Wolf,
}

impl MountType {
    /// Java `MountType.findByNpcId(npcId)`.
    pub fn find_by_npc_id(npc_id: i32) -> Self {
        if STRIDER.contains(&npc_id) {
            Self::Strider
        } else if WYVERN_GROUP.contains(&npc_id) {
            Self::Wyvern
        } else if WOLF_GROUP.contains(&npc_id) {
            Self::Wolf
        } else {
            Self::None
        }
    }

    /// Java `ordinal()`: 0 none, 1 strider, 2 wyvern, 3 wolf.
    pub fn ordinal(self) -> i32 {
        match self {
            Self::None => 0,
            Self::Strider => 1,
            Self::Wyvern => 2,
            Self::Wolf => 3,
        }
    }
}

/// What a timer of this module does.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MountTimer {
    /// Java `PetFeedTask`.
    Feed(ObjectId),
    /// Java `DismountTask`.
    Dismount(ObjectId),
    /// Java `RentPetTask`.
    RentPet(ObjectId),
    /// `if (isInWater()) broadcastUserInfo()` 1.5 s after a dismount in water.
    WaterUserInfo(ObjectId),
}

/// The mount of one player (Java's `_mount*` fields of `Player`).
#[derive(Debug, Clone, PartialEq)]
pub struct MountState {
    /// Java `_mountType`.
    pub mount_type: MountType,
    /// Java `_mountNpcId`.
    pub npc_id: i32,
    /// Java `_mountLevel`.
    pub level: i32,
    /// Java `_mountObjectID`: the pet collar the mount was summoned with (0 for none).
    pub object_id: ObjectId,
    /// Java `_canFeed`. Never reset by a dismount.
    pub can_feed: bool,
    /// Java `_curFeed`.
    pub cur_feed: i32,
    /// Java `_controlItemId`: the collar whose `pets.fed` `storePetFood` writes.
    pub control_item_id: ObjectId,
    /// Java `_leveldata`: the `(npc id, level)` of the first level data `getPetLevelData` read.
    // NOTE(java-quirk): Java caches it and never clears it, so the food bar of every later mount
    // of the same session uses the numbers of the first mount that fed.
    pub feed_level: Option<(i32, i32)>,
    /// Java `_mountFeedTask`.
    pub feed_timer: Option<TimerId>,
    /// Java `_dismountTask`.
    pub dismount_timer: Option<TimerId>,
    /// Java `_taskRentPet`.
    pub rent_timer: Option<TimerId>,
    /// The period of the rent task (`scheduleAtFixedRate(.., seconds * 1000, seconds * 1000)`).
    pub rent_period_ms: u64,
}

impl Default for MountState {
    /// Not mounted, no collar, no tasks.
    fn default() -> Self {
        Self {
            mount_type: MountType::None,
            npc_id: 0,
            level: 0,
            object_id: ObjectId(0),
            can_feed: false,
            cur_feed: 0,
            control_item_id: ObjectId(0),
            feed_level: None,
            feed_timer: None,
            dismount_timer: None,
            rent_timer: None,
            rent_period_ms: 0,
        }
    }
}

impl<'w> StatView<'w> {
    /// Java `getMountType()` of a player; none for an NPC.
    pub fn mount_type(&self) -> MountType {
        self.player().map_or(MountType::None, |p| p.mount.mount_type)
    }

    /// Java `isMounted()`.
    pub fn is_mounted(&self) -> bool {
        self.mount_type() != MountType::None
    }

    /// Java `getMountNpcId()`.
    pub fn mount_npc_id(&self) -> i32 {
        self.player().map_or(0, |p| p.mount.npc_id)
    }

    /// Java `getMountLevel()`.
    pub fn mount_level(&self) -> i32 {
        self.player().map_or(0, |p| p.mount.level)
    }

    /// Java `getPetLevelData(npcId)` of the player: the cached level data, else the one of the
    /// mount.
    pub(crate) fn mount_feed_data(&self) -> Option<&'w PetLevelData> {
        let mount = &self.player()?.mount;
        let (npc_id, level) = mount.feed_level.unwrap_or((mount.npc_id, mount.level));
        self.world().pet_table()?.pet_level_data(npc_id, level)
    }

    /// Java `PetDataTable.getPetLevelData(mountNpcId, mountLevel)`: the speeds of the ride.
    pub(crate) fn mount_ride_data(&self) -> Option<&'w PetLevelData> {
        let mount = &self.player()?.mount;
        self.world()
            .pet_table()?
            .pet_level_data(mount.npc_id, mount.level)
    }

    /// The template of the mount (`isMounted() && _mountNpcId > 0`): a mounted player takes its
    /// collision from it (`Player.getCollisionRadius` / `getCollisionHeight`).
    pub(crate) fn mount_template(&self) -> Option<&'w l2_data::NpcTemplate> {
        let mount = &self.player()?.mount;
        if mount.mount_type == MountType::None || mount.npc_id <= 0 {
            return None;
        }
        self.world().npc_world.template(mount.npc_id)
    }

    /// Java `Player.getCurrentFeed()`.
    pub fn current_feed(&self) -> i32 {
        self.player().map_or(0, |p| p.mount.cur_feed)
    }

    /// Java `Player.getFeedConsume()`: what the mount eats every 10 s (more while attacking).
    pub fn feed_consume(&self, attacking: bool) -> i32 {
        self.mount_feed_data()
            .map_or(0, |d| if attacking { d.feed_battle } else { d.feed_normal })
    }

    /// Java `Player.getMaxFeed()`.
    pub fn max_feed(&self) -> i32 {
        self.mount_feed_data().map_or(0, |d| d.max_feed)
    }

    /// Java `Player.isHungry()`: `_canFeed && _curFeed < (hungryLimit / 100f) * maxFeed`.
    pub fn is_hungry(&self) -> bool {
        let Some(player) = self.player() else {
            return false;
        };
        if !player.mount.can_feed {
            return false;
        }
        let Some(data) = self
            .world()
            .pet_table()
            .and_then(|t| t.pet_data(player.mount.npc_id))
        else {
            return false;
        };
        let max_feed = self.max_feed();
        let limit = (data.hungry_limit as f32 / 100.0_f32) * max_feed as f32;
        (player.mount.cur_feed as f32) < limit
    }

    /// The `speed_on_ride` of a mounted player (`PlayerStat.getBaseMoveSpeed`): `None` when the
    /// pet table has no row for the mount.
    pub(crate) fn mount_move_speed(&self, kind: crate::stats::creature_stat::MoveType) -> Option<f64> {
        use crate::stats::creature_stat::MoveType;
        let data = self.mount_ride_data()?;
        Some(match kind {
            MoveType::Walk => data.walk_speed_on_ride,
            MoveType::Run => data.run_speed_on_ride,
            MoveType::SlowSwim => data.slow_swim_speed_on_ride,
            MoveType::FastSwim => data.fast_swim_speed_on_ride,
            MoveType::SlowFly => data.slow_fly_speed_on_ride,
            MoveType::FastFly => data.fast_fly_speed_on_ride,
        })
    }

    /// The speed penalties of a mounted player (`PlayerStat.getRunSpeed` / `getWalkSpeed`): half
    /// when the mount is 10 levels above the rider, half again when it is hungry.
    pub(crate) fn mount_speed_penalty(&self, mut value: f64) -> f64 {
        if !self.is_mounted() {
            return value;
        }
        // If level diff with mount >= 10, it decreases move speed by 50%.
        if self.mount_level() - self.level() >= 10 {
            value /= 2.0;
        }
        // If mount is hungry, it decreases move speed by 50%.
        if self.is_hungry() {
            value /= 2.0;
        }
        value
    }
}

impl World {
    fn mount_state(&self, id: ObjectId) -> Option<&MountState> {
        self.players.get(&id).map(|p| &p.mount)
    }

    fn mount_state_mut(&mut self, id: ObjectId) -> Option<&mut MountState> {
        self.players.get_mut(&id).map(|p| &mut p.mount)
    }

    /// Java `Player.getMountType()`.
    pub fn mount_type_of(&self, id: ObjectId) -> MountType {
        self.mount_state(id).map_or(MountType::None, |m| m.mount_type)
    }

    /// Java `Player.isMounted()`.
    pub fn is_mounted(&self, id: ObjectId) -> bool {
        self.mount_type_of(id) != MountType::None
    }

    /// Java `Player.getMountNpcId()`.
    pub fn mount_npc_id_of(&self, id: ObjectId) -> i32 {
        self.mount_state(id).map_or(0, |m| m.npc_id)
    }

    /// Java `Player.getMountObjectID()`.
    pub fn mount_object_id_of(&self, id: ObjectId) -> ObjectId {
        self.mount_state(id).map_or(ObjectId(0), |m| m.object_id)
    }

    /// Java `Creature.isFlying()`.
    pub fn is_flying(&self, id: ObjectId) -> bool {
        self.stats(id).is_some_and(|v| v.is_flying())
    }

    /// Java `Player.isHungry()`.
    pub fn is_mount_hungry(&self, id: ObjectId) -> bool {
        self.stats(id).is_some_and(|v| v.is_hungry())
    }

    /// Java `Player.getCurrentFeed()`.
    pub fn current_feed_of(&self, id: ObjectId) -> i32 {
        self.mount_state(id).map_or(0, |m| m.cur_feed)
    }

    // ------------------------------------------------------------------ setMount

    /// Java `Player.setMount(npcId, npcLevel)`: the type follows the NPC; a wyvern flies and gets
    /// Wyvern Breath, the end of a ride takes both away.
    pub fn set_mount(&mut self, id: ObjectId, npc_id: i32, npc_level: i32) {
        let kind = MountType::find_by_npc_id(npc_id);
        match kind {
            MountType::None => {
                if self.is_flying(id) {
                    self.remove_skill(id, WYVERN_BREATH_ID, false);
                    self.set_flying(id, false);
                    self.send_skill_list(id);
                }
            }
            MountType::Wyvern => {
                self.set_flying(id, true);
                self.add_skill(id, WYVERN_BREATH_ID, WYVERN_BREATH_LEVEL);
                self.send_skill_list(id);
            }
            MountType::Strider | MountType::Wolf => {}
        }
        if let Some(mount) = self.mount_state_mut(id) {
            mount.mount_type = kind;
            mount.npc_id = npc_id;
            mount.level = npc_level;
        }
    }

    /// Java `Creature.setFlying(boolean)` of a player.
    fn set_flying(&mut self, id: ObjectId, flying: bool) {
        if let Some(player) = self.players.get_mut(&id) {
            player.transform.flying = flying;
        }
    }

    // ------------------------------------------------------------------ mounting

    /// `new Ride(this)` to everybody who sees the player and the player itself.
    fn broadcast_ride(&mut self, id: ObjectId) {
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let packet = Ride {
            object_id: id.0,
            mounted: player.mount.mount_type != MountType::None,
            ride_type: player.mount.mount_type.ordinal(),
            ride_npc_id: player.mount.npc_id,
            x: player.core.x,
            y: player.core.y,
            z: player.core.z,
        };
        self.broadcast_creature_packet(id, &packet);
    }

    /// Java `Player.mount(Summon pet)`: the player gets on the pet, which goes away.
    pub fn mount_pet(&mut self, id: ObjectId, pet: ObjectId) -> bool {
        // If mounts (excluding wyvern) are disallowed during siege, check if player is in a siege
        // zone.
        let in_siege = self
            .players
            .get(&id)
            .is_some_and(|p| p.zones.is_inside(ZoneId::Siege));
        if !crate::transform::ALLOW_MOUNTS_DURING_SIEGE && in_siege {
            // M7: only the castle lord of the sieged castle may ride a strider there.
            self.send_message(id, "You cannot mount during a siege.");
            return false;
        }

        if !self.disarm_weapons(id) || !self.disarm_shield(id) || self.is_transformed(id) {
            return false;
        }

        self.stop_all_toggles(id, true);
        let (npc_id, level) = (
            self.summon_npc_id(pet).unwrap_or(0),
            self.core_of(pet).map_or(0, |c| c.level),
        );
        self.set_mount(id, npc_id, level);
        let control = self.pet_control_object_id(pet).unwrap_or(ObjectId(0));
        if let Some(mount) = self.mount_state_mut(id) {
            mount.object_id = control;
        }
        self.start_feed(id, npc_id);
        self.broadcast_ride(id);

        // Notify self and others about speed change.
        self.broadcast_user_info(id);

        self.pet_unsummon(pet, Some(id));
        true
    }

    /// Java `Player.mount(npcId, controlItemObjId, useFood)`: a ride without a pet (the admin
    /// commands, the `Ride` effect, the rented pets).
    pub fn mount_npc(&mut self, id: ObjectId, npc_id: i32, control_item: ObjectId, use_food: bool) -> bool {
        if !self.disarm_weapons(id) || !self.disarm_shield(id) || self.is_transformed(id) {
            return false;
        }

        self.stop_all_toggles(id, true);
        let level = self.core_of(id).map_or(0, |c| c.level);
        self.set_mount(id, npc_id, level);
        if let Some(mount) = self.mount_state_mut(id) {
            mount.object_id = control_item;
        }
        self.broadcast_ride(id);

        // Notify self and others about speed change.
        self.broadcast_user_info(id);
        if use_food {
            self.start_feed(id, npc_id);
        }
        true
    }

    /// Java `Player.mountPlayer(pet)`: the checks of action 38 and the user command `/mount`;
    /// with a pet that cannot be ridden the player gets off what it rides. `false` when a check
    /// refused (Java returns `true` otherwise, also when nothing happened).
    pub fn mount_player(&mut self, id: ObjectId, pet: Option<ObjectId>) -> bool {
        let mountable = pet
            .is_some_and(|p| self.is_pet(p) && self.summon_npc_id(p).is_some_and(l2_data::pet::is_mountable));
        if let Some(pet) = pet.filter(|_| mountable && !self.is_mounted(id) && !self.is_betrayed(id)) {
            let refusal = if self.is_dead(id) {
                Some(Some(SystemMessageId::AStriderCannotBeRiddenWhenDead))
            } else if self.is_dead(pet) {
                Some(Some(SystemMessageId::ADeadStriderCannotBeRidden))
            } else if self.summon_in_combat(pet) || self.is_rooted(pet) {
                Some(Some(SystemMessageId::AStriderInBattleCannotBeRidden))
            } else if self.is_in_combat(id) {
                Some(Some(SystemMessageId::AStriderCannotBeRiddenWhileInBattle))
            } else if self.is_sitting(id) {
                Some(Some(SystemMessageId::AStriderCanBeRiddenOnlyWhenStanding))
            // M6-FISHING: `_fishing` sends YOU_CANNOT_DO_THAT_WHILE_FISHING_2 here.
            } else if self.is_transformed(id) {
                // No message needed, player while transformed doesn't have mount action.
                Some(None)
            // M8: `isCursedWeaponEquipped()` joins the transformed check.
            } else if self.inventory_item_by_item_id(id, COMBAT_FLAG_ID).is_some() {
                self.send(id, &ActionFailed);
                // FIXME in Java: "Wrong Message".
                self.send_message(id, "You cannot mount a steed while holding a flag.");
                return false;
            } else if self.pet_is_hungry(pet) {
                Some(Some(SystemMessageId::AHungryStriderCannotBeMountedOrDismounted))
            } else if !self.check_if_in_range(MOUNT_RANGE, id, pet, true) {
                Some(Some(SystemMessageId::YouAreTooFarAwayFromYourMountToRide))
            } else {
                None
            };
            if let Some(message) = refusal {
                self.send(id, &ActionFailed);
                if let Some(message) = message {
                    self.send(id, &SystemMessage::new(message));
                }
                return false;
            }
            if !self.is_dead(pet) && !self.is_mounted(id) {
                self.mount_pet(id, pet);
            }
        } else if self.is_rented_pet(id) {
            self.stop_rent_pet(id);
        } else if self.is_mounted(id) {
            if self.mount_type_of(id) == MountType::Wyvern
                && self.is_inside_zone(id, ZoneId::NoLanding)
                && !self.is_gm(id)
            {
                self.send(id, &ActionFailed);
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouAreNotAllowedToDismountInThisLocation),
                );
                return false;
            } else if self.is_mount_hungry(id) {
                self.send(id, &ActionFailed);
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::AHungryStriderCannotBeMountedOrDismounted),
                );
                return false;
            }
            self.dismount(id);
        }
        true
    }

    /// Java `Player.dismount()`. `false` when the place does not allow it.
    pub fn dismount(&mut self, id: ObjectId) -> bool {
        let Some((x, y, z)) = self.position_of(id) else {
            return false;
        };
        let water = self
            .zoning
            .zones
            .as_ref()
            .is_some_and(|zones| zones.zone_at(x, y, z - 300, "WaterZone").is_some());
        if !water {
            let in_water = self.stats(id).is_some_and(|v| v.in_water());
            if !self.is_gm(id) && !in_water && z > MAX_DISMOUNT_Z {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouAreNotAllowedToDismountInThisLocation),
                );
                self.send(id, &ActionFailed);
                return false;
            }

            // Skip elevation check only in NoLandingZone.
            let ground = self.zoning.geo.as_ref().map_or(z, |geo| geo.height(x, y, z));
            if !self.is_inside_zone(id, ZoneId::NoLanding) && ground + MAX_DISMOUNT_HEIGHT < z {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouCannotDismountFromThisElevation),
                );
                self.send(id, &ActionFailed);
                return false;
            }
        } else {
            let due = self.tick + TimerWheel::ms_to_ticks(WATER_USER_INFO_DELAY_MS);
            self.timers
                .schedule_at(due, TimerKind::Mount(MountTimer::WaterUserInfo(id)));
        }

        let was_flying = self.is_flying(id);
        self.send(id, &SetupGauge::new(id.0, FEED_GAUGE_COLOR, 0));
        // `SetupGauge(objectId, 3, 0, 0)`: both times are 0, which `new` gives too.
        let pet_id = self.mount_npc_id_of(id);
        self.set_mount(id, 0, 0);
        self.stop_feed(id);
        if was_flying {
            // `removeSkill(CommonSkill.WYVERN_BREATH.getSkill())`: the skill is gone since
            // `setMount(0, 0)`; what is left is the shortcuts of the skill (not while transformed).
            if !self.is_transformed(id) {
                self.delete_skill_shortcuts(id, WYVERN_BREATH_ID);
            }
        }

        self.broadcast_ride(id);
        if let Some(mount) = self.mount_state_mut(id) {
            mount.object_id = ObjectId(0);
        }
        self.store_pet_food(id, pet_id);

        // Notify self and others about speed change.
        self.broadcast_user_info(id);
        true
    }

    // ------------------------------------------------------------------ disarming

    /// Java `Player.disarmWeapons()`: `true` when the player was disarmed or had no weapon.
    pub(crate) fn disarm_weapons(&mut self, id: ObjectId) -> bool {
        use l2_protocol_game::server::charscreen::paperdoll;
        // If there is no weapon to disarm then return true.
        let Some(weapon) = self
            .paperdoll_object(id, paperdoll::RHAND)
            .and_then(|w| self.inventory_item(id, w).cloned())
        else {
            return true;
        };
        // M8: "Don't allow disarming a cursed weapon." M7: "... a Combat Flag or Territory Ward."
        let templates = self.item_templates();
        let Some(template) = templates.as_deref().and_then(|t| t.get(weapon.item_id)) else {
            return true;
        };
        // Don't allow disarming if the weapon is force equip.
        if template.weapon().is_some_and(|w| w.force_equip) {
            return false;
        }
        let unequipped = self.unequip_item_in_body_slot_and_record(id, template.body_part);
        self.finish_disarm(id, &unequipped);
        true
    }

    /// Java `Player.disarmShield()`: always `true`.
    pub(crate) fn disarm_shield(&mut self, id: ObjectId) -> bool {
        use l2_protocol_game::server::charscreen::paperdoll;
        let Some(shield) = self
            .paperdoll_object(id, paperdoll::LHAND)
            .and_then(|s| self.inventory_item(id, s).cloned())
        else {
            return true;
        };
        let templates = self.item_templates();
        let Some(template) = templates.as_deref().and_then(|t| t.get(shield.item_id)) else {
            return true;
        };
        let unequipped = self.unequip_item_in_body_slot_and_record(id, template.body_part);
        self.finish_disarm(id, &unequipped);
        true
    }

    /// The end both disarm methods share: the inventory update, `abortAttack`, the user info and
    /// the message about the first item that came off.
    fn finish_disarm(&mut self, id: ObjectId, unequipped: &[ObjectId]) {
        self.send_changed_items(id, unequipped);
        self.abort_attack(id);
        self.broadcast_user_info(id);

        // This can be 0 if the user pressed the right mousebutton twice very fast.
        if let Some(first) = unequipped.first().copied()
            && let Some(item) = self.inventory_item(id, first).cloned()
        {
            let templates = self.item_templates();
            let message = self.disarmed_message(item.enchant_level, item.item_id, templates.as_deref());
            self.send(id, &message);
        }
    }

    // ------------------------------------------------------------------ the food

    /// Java `Player.startFeed(npcId)`: the food bar of the mount and the task that empties it.
    pub(crate) fn start_feed(&mut self, id: ObjectId, npc_id: i32) {
        if let Some(mount) = self.mount_state_mut(id) {
            mount.can_feed = npc_id > 0;
        }
        if !self.is_mounted(id) {
            return;
        }

        if let Some(summon) = self.summon_of(id) {
            self.cache_feed_level(id);
            let fed = self.pet_current_fed(summon);
            let control = self.pet_control_object_id(summon).unwrap_or(ObjectId(0));
            self.set_current_feed(id, fed);
            if let Some(mount) = self.mount_state_mut(id) {
                mount.control_item_id = control;
            }
            self.send_feed_gauge(id);
            if !self.is_dead(id) {
                self.schedule_feed(id);
            }
        } else if self.mount_state(id).is_some_and(|m| m.can_feed) {
            self.cache_feed_level(id);
            let max = self.stats(id).map_or(0, |v| v.max_feed());
            self.set_current_feed(id, max);
            self.send_feed_gauge(id);
            if !self.is_dead(id) {
                self.schedule_feed(id);
            }
        }
    }

    /// Java `getPetLevelData(npcId)` keeps the first level data it finds for the whole session.
    fn cache_feed_level(&mut self, id: ObjectId) {
        let Some(mount) = self.mount_state(id) else {
            return;
        };
        if mount.feed_level.is_some() {
            return;
        }
        let key = (mount.npc_id, mount.level);
        let found = self
            .pet_table()
            .and_then(|t| t.pet_level_data(key.0, key.1))
            .is_some();
        if found && let Some(mount) = self.mount_state_mut(id) {
            mount.feed_level = Some(key);
        }
    }

    fn schedule_feed(&mut self, id: ObjectId) {
        let due = self.tick + TimerWheel::ms_to_ticks(FEED_INTERVAL_MS);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Mount(MountTimer::Feed(id)));
        let old = self.mount_state_mut(id).and_then(|m| m.feed_timer.replace(timer));
        if let Some(old) = old {
            self.timers.cancel(old);
        }
    }

    /// Java `Player.stopFeed()`.
    pub fn stop_feed(&mut self, id: ObjectId) {
        let timer = self.mount_state_mut(id).and_then(|m| m.feed_timer.take());
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    /// `new SetupGauge(getObjectId(), 3, (_curFeed * 10000) / getFeedConsume(), (getMaxFeed() *
    /// 10000) / getFeedConsume())`.
    fn send_feed_gauge(&self, id: ObjectId) {
        let attacking = self.is_attacking_now(id);
        let Some(view) = self.stats(id) else {
            return;
        };
        let consume = view.feed_consume(attacking);
        if consume == 0 {
            // Java divides by zero here; the mount has no food data.
            warn!("{id}: mount {} has no feed data", view.mount_npc_id());
            return;
        }
        let packet = SetupGauge {
            object_id: id.0,
            color: FEED_GAUGE_COLOR,
            time: view.current_feed().wrapping_mul(10_000) / consume,
            max_time: view.max_feed().wrapping_mul(10_000) / consume,
        };
        self.send(id, &packet);
    }

    /// Java `Player.setCurrentFeed(num)`: the bar, and everybody hears about the speed when the
    /// mount becomes hungry or full.
    pub fn set_current_feed(&mut self, id: ObjectId, num: i32) {
        let last_hungry = self.is_mount_hungry(id);
        let max = self.stats(id).map_or(0, |v| v.max_feed());
        if let Some(mount) = self.mount_state_mut(id) {
            mount.cur_feed = if num > max { max } else { num };
        }
        self.send_feed_gauge(id);

        // Broadcast move speed change when strider becomes hungry / full.
        if last_hungry != self.is_mount_hungry(id) {
            self.broadcast_user_info(id);
        }
    }

    /// Java `Player.storePetFood(petId)`: `UPDATE pets SET fed=? WHERE item_obj_id = ?` for the
    /// collar the mount was summoned with.
    pub fn store_pet_food(&mut self, id: ObjectId, pet_id: i32) {
        let Some(mount) = self.mount_state_mut(id) else {
            return;
        };
        if mount.control_item_id.0 == 0 || pet_id == 0 {
            return;
        }
        let (control, fed) = (mount.control_item_id, mount.cur_feed);
        mount.control_item_id = ObjectId(0);
        self.outbox.persist(PersistCommand::Summon(SummonWrite::PetFed {
            control_object_id: control,
            fed,
        }));
        if let Some(row) = self.summons.pet_world.rows.get_mut(&control) {
            row.fed = fed;
        }
    }

    /// Java `PetFeedTask.run()`: the mount eats into its bar, the player's food feeds a hungry
    /// mount, and an empty bar ends the ride.
    fn on_feed_timer(&mut self, id: ObjectId) {
        if let Some(mount) = self.mount_state_mut(id) {
            mount.feed_timer = None;
        } else {
            return;
        }
        let has_data = self
            .pet_table()
            .is_some_and(|t| t.pet_data(self.mount_npc_id_of(id)).is_some());
        if !self.is_mounted(id) || self.mount_npc_id_of(id) == 0 || !has_data {
            self.stop_feed(id);
            return;
        }
        // The fixed rate: the next run first.
        self.schedule_feed(id);

        let attacking = self.is_attacking_now(id);
        let (cur, consume) = self
            .stats(id)
            .map_or((0, 0), |v| (v.current_feed(), v.feed_consume(attacking)));
        if cur > consume {
            // Eat.
            self.set_current_feed(id, cur - consume);
        } else {
            self.set_current_feed(id, 0);
            self.stop_feed(id);

            if self.is_flying(id) && self.mount_type_of(id) == MountType::Wyvern {
                self.dismount(id);
                // Teleport to nearest village.
                self.tele_to_location_where(id, TeleportWhereType::Town);
            } else {
                self.dismount(id);
            }

            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouAreOutOfFeedMountStatusCanceled),
            );
            return;
        }

        let food_ids = self
            .pet_table()
            .and_then(|t| t.pet_data(self.mount_npc_id_of(id)))
            .map(l2_data::pet::PetData::food)
            .unwrap_or_default();
        if food_ids.is_empty() {
            return;
        }

        // TODO in Java: "possibly pet inv?"
        let food = food_ids
            .iter()
            .find_map(|food_id| self.inventory_item_by_item_id(id, *food_id).cloned());
        if let Some(food) = food
            && self.is_mount_hungry(id)
            && self.item_has_handler(food.item_id)
        {
            self.player_item_handler_use(id, &food);
            let templates = self.item_templates();
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourPetWasHungrySoItAteS1)
                    .with(item_name_param(templates.as_deref(), food.item_id)),
            );
        }
    }

    /// `handler.onItemUse(player, item, false)` for an item the player holds.
    fn player_item_handler_use(&mut self, id: ObjectId, item: &crate::player::ItemData) -> bool {
        let Some(templates) = self.item_templates() else {
            return false;
        };
        let Some(template) = templates.get(item.item_id) else {
            return false;
        };
        match crate::inventory::handlers::ItemHandlerKind::of_template(template) {
            Some(kind) => self.run_item_handler(kind, id, item, template, false),
            None => false,
        }
    }

    /// The mount branch of `PetFood.useFood` for a player: a mounted player feeds its mount with
    /// the food the mount eats. `false` (and the "unsuitable terms" message by the caller) when
    /// the player is not mounted or the mount does not eat it.
    pub(crate) fn mount_use_food(
        &mut self,
        id: ObjectId,
        skill_id: i32,
        skill_level: i32,
        feed: i32,
        item: &crate::player::ItemData,
    ) -> bool {
        if !self.is_mounted(id) {
            return false;
        }
        let food_ids = self
            .pet_table()
            .and_then(|t| t.pet_data(self.mount_npc_id_of(id)))
            .map(l2_data::pet::PetData::food)
            .unwrap_or_default();
        if !food_ids.contains(&item.item_id) || !self.destroy_item_by_object_id(id, item.object_id, 1, false)
        {
            return false;
        }
        if let Some((x, y, z)) = self.position_of(id) {
            let here = SkillPosition { x, y, z };
            self.broadcast_creature_packet(
                id,
                &MagicSkillUse {
                    creature_id: id.0,
                    target_id: id.0,
                    skill_id,
                    skill_level,
                    hit_time: 0,
                    reuse_delay: 0,
                    creature: here,
                    ground_location: None,
                    target: here,
                },
            );
        }
        let cur = self.current_feed_of(id);
        self.set_current_feed(id, cur.wrapping_add(feed));
        true
    }

    // ------------------------------------------------------------------ no-landing zones

    /// Java `Player.enteredNoLanding(delay)`: the wyvern is dismounted after `delay` seconds.
    pub fn entered_no_landing(&mut self, id: ObjectId, delay_secs: i32) {
        let ms = u64::try_from(delay_secs).unwrap_or(0).saturating_mul(1000);
        let due = self.tick + TimerWheel::ms_to_ticks(ms);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Mount(MountTimer::Dismount(id)));
        // Java overwrites `_dismountTask` without cancelling the old one.
        if let Some(mount) = self.mount_state_mut(id) {
            mount.dismount_timer = Some(timer);
        }
    }

    /// Java `Player.exitedNoLanding()`.
    pub fn exited_no_landing(&mut self, id: ObjectId) {
        let timer = self.mount_state_mut(id).and_then(|m| m.dismount_timer.take());
        if let Some(timer) = timer {
            self.timers.cancel(timer);
        }
    }

    /// `NoLandingZone.onEnter(creature)` after the flag is set.
    pub(crate) fn no_landing_zone_entered(&mut self, id: ObjectId, dismount_delay: Option<&str>) {
        if self.is_gm(id) {
            self.send_message(
                id,
                "You have entered a no-landing zone. Dismount restrictions are ignored for GMs.",
            );
            return;
        }
        if self.mount_type_of(id) == MountType::Wyvern {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::ThisAreaCannotBeEnteredWhileMountedAtopOfAWyvernYouWillBeDismountedFromYourWyvernIfYouDoNotLeave,
                ),
            );
            // `Integer.parseInt(value)`; the zone files only hold numbers.
            let delay = dismount_delay
                .and_then(|v| v.trim().parse::<i32>().ok())
                .unwrap_or(DEFAULT_DISMOUNT_DELAY);
            self.entered_no_landing(id, delay);
        }
    }

    /// `NoLandingZone.onExit(creature)` after the flag is cleared.
    pub(crate) fn no_landing_zone_exited(&mut self, id: ObjectId) {
        if self.mount_type_of(id) == MountType::Wyvern {
            self.exited_no_landing(id);
        }
    }

    /// Java `Player.checkLandingState()`: a wyvern cannot land here.
    // M7: the second branch of Java (a sieged castle, unless the rider is its lord).
    pub fn check_landing_state(&self, id: ObjectId) -> bool {
        self.is_inside_zone(id, ZoneId::NoLanding)
    }

    // ------------------------------------------------------------------ rented pets

    /// Java `Player.startRentPet(seconds)`.
    pub fn start_rent_pet(&mut self, id: ObjectId, seconds: i32) {
        if self.is_rented_pet(id) {
            return;
        }
        let ms = u64::try_from(seconds).unwrap_or(0).saturating_mul(1000);
        let due = self.tick + TimerWheel::ms_to_ticks(ms);
        let timer = self
            .timers
            .schedule_at(due, TimerKind::Mount(MountTimer::RentPet(id)));
        if let Some(mount) = self.mount_state_mut(id) {
            mount.rent_timer = Some(timer);
            mount.rent_period_ms = ms;
        }
    }

    /// Java `Player.isRentedPet()`.
    pub fn is_rented_pet(&self, id: ObjectId) -> bool {
        self.mount_state(id).is_some_and(|m| m.rent_timer.is_some())
    }

    /// Java `Player.stopRentPet()`: the rent is over; a wyvern over a no-landing place is
    /// teleported to the town first.
    pub fn stop_rent_pet(&mut self, id: ObjectId) {
        if !self.is_rented_pet(id) {
            return;
        }
        // If the rent of a wyvern expires while over a flying zone, tp to down before unmounting.
        if self.check_landing_state(id) && self.mount_type_of(id) == MountType::Wyvern {
            self.tele_to_location_where(id, TeleportWhereType::Town);
        }

        // This should always be true now, since we teleported already.
        if self.dismount(id) {
            let timer = self.mount_state_mut(id).and_then(|m| m.rent_timer.take());
            if let Some(timer) = timer {
                self.timers.cancel(timer);
            }
        }
    }

    // ------------------------------------------------------------------ timers

    /// The dispatch of `TimerKind::Mount`.
    pub(crate) fn on_mount_timer(&mut self, timer: MountTimer) {
        match timer {
            MountTimer::Feed(id) => self.on_feed_timer(id),
            MountTimer::Dismount(id) => {
                if let Some(mount) = self.mount_state_mut(id) {
                    mount.dismount_timer = None;
                    // Java `DismountTask.run()`.
                    self.dismount(id);
                }
            }
            MountTimer::RentPet(id) => {
                // The fixed rate: the next run first (`stopRentPet` cancels it when it succeeds).
                let Some(ms) = self.mount_state(id).map(|m| m.rent_period_ms) else {
                    return;
                };
                let due = self.tick + TimerWheel::ms_to_ticks(ms);
                let next = self
                    .timers
                    .schedule_at(due, TimerKind::Mount(MountTimer::RentPet(id)));
                if let Some(mount) = self.mount_state_mut(id) {
                    mount.rent_timer = Some(next);
                }
                self.stop_rent_pet(id);
            }
            MountTimer::WaterUserInfo(id) => {
                if self.stats(id).is_some_and(|v| v.in_water()) {
                    self.broadcast_user_info(id);
                }
            }
        }
    }

    // ------------------------------------------------------------------ the world around it

    /// Java `Player.doDie`: `if (isMounted()) stopFeed()`.
    pub(crate) fn mount_on_die(&mut self, id: ObjectId) {
        if self.is_mounted(id) {
            self.stop_feed(id);
        }
    }

    /// Java `Player.doRevive`: `if (isMounted()) startFeed(_mountNpcId)`.
    pub(crate) fn mount_on_revive(&mut self, id: ObjectId) {
        if self.is_mounted(id) {
            let npc_id = self.mount_npc_id_of(id);
            self.start_feed(id, npc_id);
        }
    }

    /// Java `Player.deleteMe`: a flying player loses Wyvern Breath, `stopAllTimers` ends the
    /// feed, stores the food and ends the rent, `stopAllTasks` cancels the dismount task.
    pub(crate) fn mount_on_leave(&mut self, id: ObjectId) {
        // `removeSkill(WYVERN_BREATH)`: the skill was never stored, so nothing is left to remove
        // from `character_skills` (the world forgets the player's skill list with it).
        self.stop_feed(id);
        let npc_id = self.mount_npc_id_of(id);
        self.store_pet_food(id, npc_id);
        let rent = self.mount_state_mut(id).and_then(|m| m.rent_timer.take());
        if let Some(timer) = rent {
            self.timers.cancel(timer);
        }
        self.exited_no_landing(id);
    }

    /// The `FreeMountsConfig` part of `Player.rewardSkills()`: Twilight Dragon (a strider) and
    /// Wind Dragon (a wyvern), `addSkill(skill, false)`. A skill the datapack lacks is not added
    /// (`if (skill != null)`).
    pub(crate) fn give_free_mount_skills(&mut self, id: ObjectId) {
        if self.settings.exp.enable_free_strider {
            self.add_skill(id, FREE_STRIDER_SKILL_ID, 1);
        }
        if self.settings.exp.enable_free_wyvern {
            self.add_skill(id, FREE_WYVERN_SKILL_ID, 1);
        }
    }

    /// The `Ride` skill effect (`handlers/skill/effects/Ride.java`): `mount(npcId, 0, false)`.
    pub(crate) fn ride_effect(&mut self, effected: ObjectId, npc_id: i32) {
        if !self.is_player(effected) {
            return;
        }
        self.mount_npc(effected, npc_id, ObjectId(0), false);
    }

    /// The user commands `/mount` (61) and `/dismount` (62).
    pub(crate) fn user_command_mount(&mut self, id: ObjectId) {
        let summon = self.summon_of(id);
        self.mount_player(id, summon);
    }

    /// Java `Dismount.onCommand`.
    pub(crate) fn user_command_dismount(&mut self, id: ObjectId) {
        if self.is_rented_pet(id) {
            self.stop_rent_pet(id);
        } else if self.is_mounted(id) {
            self.dismount(id);
        }
    }
}
