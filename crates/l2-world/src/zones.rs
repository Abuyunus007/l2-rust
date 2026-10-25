//! Zone membership of players: which zones a player is inside, the `ZoneId` flags they set, the
//! messages and compass icon the client gets, and the nearest-town lookups for restart points.
// Java: entity/zone/{ZoneType,ZoneRegion,ZoneId}.java (revalidateInZone, removeCharacter,
//       revalidateZones, isAffected, setParameter)
// Java: entity/zone/type/*.java (onEnter / onExit of each type)
// Java: entity/actor/Creature.java (revalidateZone, setXYZ, setInsideZone, isInsideZone)
// Java: entity/actor/Player.java (revalidateZone with the compass icon)
// Java: managers/ZoneManager.java (getRegion), data/xml/MapRegionData.java (getTeleToLocation)
//
// # What is modelled
// Zone shapes and the zone table come from `l2-data` (`Zones`, `MapRegions`); the world only
// keeps, per player, the zones whose `_characterList` holds them (Java keeps the same list on the
// zone), the `ZoneId` counters and the last compass icon sent.
//
// Types with a player-visible effect that does not need a system M3 lacks are ported: `TownZone`,
// `PeaceZone`, `NoPvPZone`, `ArenaZone`, `MotherTreeZone`, `JailZone` (flags and messages),
// `NoStoreZone`, `NoRestartZone`, `NoSummonFriendZone`, `ResidenceTeleportZone`,
// `ResidenceHallTeleportZone`, `LandingZone`, `NoLandingZone`, `HqZone`, `ConditionZone`,
// `CastleZone`, `FortZone`, `ScriptZone`, `DerbyTrackZone`, `EffectZone` (the altered/danger
// flags; the skills it applies are M4). Every other type is loaded and found by lookups, but
// entering it does nothing (`SiegeZone` is the siege's, `siege/zone.rs`, M7-SIEGE-01),
// `OlympiadStadiumZone`, `WaterZone`, `SwampZone`, `BossZone`, `DamageZone`, `FishingZone`,
// `RespawnZone` (restart points only). `FlagPvPZone` flags the player (M6-DUEL-01); its
// `onDieInside` / `onReviveInside` hooks need zone calls from the death code, which does not
// exist.
//
// What Java does in these zones that is left out, with the system that brings it:
// - `SiegeZone`: the dismount of a wyvern and the mounts it refuses (M7, `mount.rs` has the
//   `enteredNoLanding` half)
// - `NoRestartZone.onPlayerLoginInside`: the town teleport of a long absence: needs
//   `characters.lastAccess`; the login hook is not called
// - `isAffected`: the `affectedClassType` check (needs the class list), the instance template
//   check (no instance worlds yet)
// - Seven Signs dungeon compass icon, `updatePvPStatus` after a siege zone, `checkWaterState`
//
// `ClanHallZone` and `SiegableHallZone` set the `CLAN_HALL` flag and show the hall's decorations
// (`AgitDecoInfo`, M7-RES-01, `residence/clan_hall.rs`).
//
// NPCs and other creatures have no zone state in M3: their zones are inert.

use std::sync::Arc;

use l2_data::zone::{GRID_HEIGHT, GRID_WIDTH, OFFSET_X, OFFSET_Y, SHIFT_BY};
use l2_data::{MapRegions, Zone, Zones};
use l2_geo::GeoEngine;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::visibility::compass_zone;
use l2_protocol_game::server::{ExSetCompassZoneCode, SystemMessage};
use l2_util::ObjectId;
use tracing::debug;

use crate::objects::ObjectKind;
use crate::player::Player;
use crate::world::World;

/// Java `ZoneId`, in Java's order (the counters are indexed by it).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ZoneId {
    Pvp,
    Peace,
    Siege,
    MotherTree,
    ClanHall,
    Landing,
    NoLanding,
    Water,
    Jail,
    MonsterTrack,
    Castle,
    Swamp,
    NoSummonFriend,
    Fort,
    NoStore,
    NoPvp,
    Town,
    Script,
    Hq,
    DangerArea,
    Altered,
    NoBookmark,
    NoItemDrop,
    NoRestart,
}

/// `ZoneId.getZoneCount()`.
const ZONE_ID_COUNT: usize = 24;

/// `GeneralConfig` values the zone types read.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ZoneSettings {
    /// `PeaceZoneMode`: 2 makes peace zones and no-PvP zones set no flag.
    pub peace_zone_mode: i32,
    /// `JailIsPvp`.
    pub jail_is_pvp: bool,
    /// `JailDisableTransaction`.
    pub jail_disable_transaction: bool,
}

/// Datapack tables and the geoengine, as handed to the world at startup.
#[derive(Default)]
pub(crate) struct ZoneContext {
    pub zones: Option<Arc<Zones>>,
    pub map_regions: Option<Arc<MapRegions>>,
    pub geo: Option<Arc<GeoEngine>>,
}

/// The zones a zone holds a creature in, as Java's `_characterList` identifies them: a zone id
/// is unique only within its type.
type ZoneKey = (i32, String);

/// A player's zone state (Java: `Creature._zones`, the zones' `_characterList`s,
/// `_lastZoneValidateLocation`, `Player._lastCompassZone`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PlayerZones {
    /// `Creature._zones`: how many entered zones set each [`ZoneId`].
    counters: [i32; ZONE_ID_COUNT],
    /// The zones that hold this player.
    members: Vec<ZoneKey>,
    /// `_lastZoneValidateLocation`.
    last_validate: (i32, i32, i32),
    /// `_lastCompassZone` (0 = nothing sent yet).
    last_compass: i32,
}

impl PlayerZones {
    /// Java `Creature.isInsideZone(ZoneId)`. Instances that are PvP (`Instance.isPvP`) are not
    /// modelled, so only the rule that peace and no-PvP zones override PvP applies.
    pub fn is_inside(&self, zone: ZoneId) -> bool {
        let count = |z: ZoneId| self.counters[z as usize];
        match zone {
            ZoneId::Pvp => count(ZoneId::Pvp) > 0 && count(ZoneId::Peace) == 0 && count(ZoneId::NoPvp) == 0,
            other => count(other) > 0,
        }
    }

    /// Java `Creature.setInsideZone`: a counter, so overlapping zones of one kind add up.
    fn set_inside(&mut self, zone: ZoneId, state: bool) {
        let counter = &mut self.counters[zone as usize];
        if state {
            *counter += 1;
        } else if *counter > 0 {
            *counter -= 1;
        }
    }

    /// The zones that currently hold the player, as `(zone id, type name)`.
    pub fn members(&self) -> &[(i32, String)] {
        &self.members
    }

    fn is_member(&self, zone: &Zone) -> bool {
        self.members
            .iter()
            .any(|(id, ty)| *id == zone.id && *ty == zone.zone_type)
    }
}

/// `ZoneManager.getRegion(x, y)`: the 32768-unit zone cell, `None` where Java's array access
/// fails.
fn zone_cell(x: i32, y: i32) -> Option<(i32, i32)> {
    let cx = (x >> SHIFT_BY) + OFFSET_X;
    let cy = (y >> SHIFT_BY) + OFFSET_Y;
    let inside = (0..GRID_WIDTH as i32).contains(&cx) && (0..GRID_HEIGHT as i32).contains(&cy);
    inside.then_some((cx, cy))
}

/// Java `Boolean.parseBoolean`.
fn parse_bool(text: &str) -> bool {
    text.eq_ignore_ascii_case("true")
}

/// The parameters `ZoneType.setParameter` keeps for `isAffected` (and `allowStore`), read from
/// the zone's raw `<stat>` list the way Java does (later values replace earlier ones, race and
/// class lists accumulate). A value Java would fail to parse is skipped here; Java's loader
/// throws.
#[derive(Debug, Clone, PartialEq, Eq)]
struct ZoneRules {
    enabled: bool,
    min_level: i32,
    max_level: i32,
    races: Vec<i32>,
    classes: Vec<i32>,
    /// `targetClass` admits a player (`Creature`, `Playable`, `Player`, `WorldObject`).
    target_admits_player: bool,
    allow_store: bool,
}

impl ZoneRules {
    fn of(zone: &Zone) -> Self {
        let mut rules = Self {
            enabled: true,
            min_level: 0,
            max_level: 0xFF,
            races: Vec::new(),
            classes: Vec::new(),
            target_admits_player: true,
            allow_store: true,
        };
        for (name, value) in &zone.params {
            let int = || value.trim().parse::<i32>().ok();
            match name.as_str() {
                "affectedLvlMin" => rules.min_level = int().unwrap_or(rules.min_level),
                "affectedLvlMax" => rules.max_level = int().unwrap_or(rules.max_level),
                "affectedRace" => rules.races.extend(int()),
                "affectedClassId" => rules.classes.extend(int()),
                "targetClass" => {
                    rules.target_admits_player =
                        matches!(value.as_str(), "WorldObject" | "Creature" | "Playable" | "Player");
                }
                "allowStore" => rules.allow_store = parse_bool(value),
                "default_enabled" => rules.enabled = parse_bool(value),
                _ => {}
            }
        }
        rules
    }

    /// Java `ZoneType.isAffected(creature)` for a player.
    // NOTE(unported): `affectedClassType` (needs the class list) and the instance template check
    // (`InstanceManager.getWorld`, no instance worlds yet) are not checked; no shipped zone
    // uses either.
    fn affects(&self, player: &Player) -> bool {
        if !self.enabled {
            return false;
        }
        if player.core.level < self.min_level || player.core.level > self.max_level {
            return false;
        }
        if !self.target_admits_player {
            return false;
        }
        if !self.races.is_empty() && !self.races.contains(&player.data.race) {
            return false;
        }
        if !self.classes.is_empty() && !self.classes.contains(&player.data.class_id) {
            return false;
        }
        true
    }
}

/// What a zone type does when a player enters or leaves.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ZoneKind {
    Town,
    Peace,
    NoPvp,
    Arena,
    MotherTree,
    Jail,
    NoStore,
    NoRestart,
    NoSummonFriend,
    Landing,
    NoLanding,
    Hq,
    Condition,
    Castle,
    Fort,
    Script,
    DerbyTrack,
    Effect,
    /// `FlagPvPZone`: the PvP flag follows the player in and out.
    FlagPvp,
    /// `ClanHallZone` and its `SiegableHallZone` (M7-RES-01).
    ClanHall,
    /// `SiegeZone` (the castle ones: M7-SIEGE-01, `siege/zone.rs`; the fortress ones: M7-FORT-01,
    /// `fort/zone.rs`).
    Siege,
    /// Loaded and found by lookups; entering does nothing in M3.
    Inert,
}

impl ZoneKind {
    fn of(zone_type: &str) -> Self {
        match zone_type {
            "TownZone" => Self::Town,
            "PeaceZone" => Self::Peace,
            "NoPvPZone" => Self::NoPvp,
            "ArenaZone" => Self::Arena,
            "MotherTreeZone" => Self::MotherTree,
            "JailZone" => Self::Jail,
            "NoStoreZone" => Self::NoStore,
            "NoRestartZone" => Self::NoRestart,
            "NoSummonFriendZone" | "ResidenceTeleportZone" | "ResidenceHallTeleportZone" => {
                Self::NoSummonFriend
            }
            "LandingZone" => Self::Landing,
            "NoLandingZone" => Self::NoLanding,
            "HqZone" => Self::Hq,
            "ConditionZone" => Self::Condition,
            "CastleZone" => Self::Castle,
            "FortZone" => Self::Fort,
            "ScriptZone" => Self::Script,
            "DerbyTrackZone" => Self::DerbyTrack,
            "EffectZone" => Self::Effect,
            "FlagPvPZone" => Self::FlagPvp,
            "ClanHallZone" | "SiegableHallZone" => Self::ClanHall,
            "SiegeZone" => Self::Siege,
            _ => Self::Inert,
        }
    }
}

impl World {
    /// Installs the zone table and the map regions (`ZoneManager`, `MapRegionData`). Without
    /// them no player has zones: no flags, no zone messages and no compass icon.
    pub fn set_zone_data(&mut self, zones: Arc<Zones>, map_regions: Arc<MapRegions>) {
        self.zoning.zones = Some(zones);
        self.zoning.map_regions = Some(map_regions);
    }

    /// Installs the geoengine a teleport asks for the ground height and the line checks of its
    /// random offset. Without it the ground is where the caller said (Java without geodata).
    /// [`World::set_geo`] installs it too; this one is for tests that only teleport.
    pub fn set_teleport_geo(&mut self, geo: Arc<GeoEngine>) {
        self.zoning.geo = Some(geo);
    }

    /// Java `isInsideZone(ZoneId)` of a player; `false` for anything else.
    pub fn is_inside_zone(&self, id: ObjectId, zone: ZoneId) -> bool {
        self.players.get(&id).is_some_and(|p| p.zones.is_inside(zone))
    }

    /// The first half of `Creature.setXYZ`, before the position changes. `false` means Java
    /// returns without moving the creature: "(0, 0) is not a valid location", and a position
    /// outside the zone grid ("Prevent moving to nonexistent regions"). When the creature
    /// crosses into another zone cell, the zones of the cell it leaves drop it
    /// (`removeFromZones`) and the new cell's zones are revalidated.
    pub(crate) fn zone_gate_set_xyz(&mut self, id: ObjectId, x: i32, y: i32) -> bool {
        let Some(kind) = self.objects.get(&id).map(|e| e.kind) else {
            return true;
        };
        if x == 0 && y == 0 {
            return false;
        }
        let Some(new_cell) = zone_cell(x, y) else {
            return false;
        };
        if kind == ObjectKind::Player
            && let Some((ox, oy, _)) = self.position_of(id)
            && zone_cell(ox, oy) != Some(new_cell)
        {
            self.remove_from_zones(id, ox, oy);
            // NOTE(java-quirk): Java revalidates the new cell's zones before it moves the
            // creature, so they are tested against the old position.
            self.revalidate_zones_in_cell(id, x, y);
        }
        true
    }

    /// Java `Creature.onSpawn()`: `revalidateZone(true)`.
    pub(crate) fn on_spawn(&mut self, id: ObjectId) {
        self.revalidate_zone(id, true);
    }

    /// Java `Player.revalidateZone(force)`: re-checks the zones around the player and updates the
    /// compass icon. Without `force` it only runs once the player is 100 units away from the
    /// place of the last check.
    pub fn revalidate_zone(&mut self, id: ObjectId, force: bool) {
        if self.zoning.zones.is_none() {
            return;
        }
        // "Cannot validate if not in a world region (happens during teleport)."
        if self.objects.get(&id).and_then(|e| e.region).is_none() {
            return;
        }
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let (x, y, z) = (player.core.x, player.core.y, player.core.z);
        if !force {
            let (lx, ly, lz) = player.zones.last_validate;
            let (dx, dy, dz) = (f64::from(x - lx), f64::from(y - ly), f64::from(z - lz));
            if (dx * dx + dy * dy + dz * dz).sqrt() < 100.0 {
                return;
            }
        }
        player.zones.last_validate = (x, y, z);
        self.revalidate_zones_in_cell(id, x, y);
        self.update_compass(id);
    }

    /// Java `ZoneRegion.revalidateZones(creature)` for the cell that holds `(cell_x, cell_y)`.
    fn revalidate_zones_in_cell(&mut self, id: ObjectId, cell_x: i32, cell_y: i32) {
        let Some(zones) = self.zoning.zones.clone() else {
            return;
        };
        // "Do NOT update the world region while the character is still in the process of
        // teleporting." The revalidation happens once the teleport is completed.
        if self.is_teleporting(id) {
            return;
        }
        for zone in zones.region_zones(cell_x, cell_y) {
            self.revalidate_in_zone(id, zone);
        }
    }

    /// Java `ZoneRegion.removeFromZones(creature)` for the cell that holds `(x, y)`.
    fn remove_from_zones(&mut self, id: ObjectId, x: i32, y: i32) {
        let Some(zones) = self.zoning.zones.clone() else {
            return;
        };
        for zone in zones.region_zones(x, y) {
            self.remove_character(id, zone);
        }
    }

    /// Java `ZoneType.revalidateInZone(creature)`.
    fn revalidate_in_zone(&mut self, id: ObjectId, zone: &Zone) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let (x, y, z) = (player.core.x, player.core.y, player.core.z);
        if !zone.is_inside_zone(x, y, z) {
            self.remove_character(id, zone);
            return;
        }
        // Java asks `isAffected` first and `putIfAbsent` second; both end in "return" without
        // side effects, so the cheaper test goes first.
        if player.zones.is_member(zone) {
            return;
        }
        let rules = ZoneRules::of(zone);
        // "If the character can't be affected by this zone return."
        if !rules.affects(player) {
            return;
        }
        player.zones.members.push((zone.id, zone.zone_type.clone()));
        self.on_zone_enter(id, zone, &rules);
    }

    /// Java `ZoneType.removeCharacter(creature)`.
    fn remove_character(&mut self, id: ObjectId, zone: &Zone) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let Some(index) = player
            .zones
            .members
            .iter()
            .position(|(zid, ty)| *zid == zone.id && *ty == zone.zone_type)
        else {
            return;
        };
        player.zones.members.remove(index);
        self.on_zone_exit(id, zone);
    }

    pub(crate) fn set_inside(&mut self, id: ObjectId, zone: ZoneId, state: bool) {
        if let Some(player) = self.players.get_mut(&id) {
            player.zones.set_inside(zone, state);
        }
    }

    /// The zone's `onEnter(creature)` for a player.
    fn on_zone_enter(&mut self, id: ObjectId, zone: &Zone, rules: &ZoneRules) {
        use SystemMessageId::YouHaveEnteredACombatZone;
        let peace_mode = self.settings.zones.peace_zone_mode;
        // `PeaceZone.onEnter`: a ward carrier loses the ward to where he picked it up while the
        // Territory War runs (M7-TW-01).
        if matches!(ZoneKind::of(&zone.zone_type), ZoneKind::Peace)
            && self.is_combat_flag_equipped(id)
            && self.is_tw_in_progress()
        {
            self.territory_drop_combat_flag(id, false, true);
        }
        match ZoneKind::of(&zone.zone_type) {
            ZoneKind::Town => self.set_inside(id, ZoneId::Town, true),
            // "PVP possible during siege, now for siege participants only." (M7-SIEGE-01)
            ZoneKind::Peace | ZoneKind::NoPvp if peace_mode == 1 && self.siege_state_of(id) != 0 => {}
            ZoneKind::Peace => {
                // "Nevit Turn Off on Peace Zone" (before the flag, like Java).
                self.nevit_on_peace_zone_enter(id);
                if peace_mode != 2 {
                    self.set_inside(id, ZoneId::Peace, true);
                }
                if !rules.allow_store {
                    self.set_inside(id, ZoneId::NoStore, true);
                }
            }
            ZoneKind::NoPvp => {
                if peace_mode != 2 {
                    self.set_inside(id, ZoneId::NoPvp, true);
                }
                // "Send player info to nearby players."
                self.broadcast_info(id);
            }
            ZoneKind::Arena => {
                if !self.is_inside_zone(id, ZoneId::Pvp) {
                    self.send(id, &SystemMessage::new(YouHaveEnteredACombatZone));
                }
                self.set_inside(id, ZoneId::Pvp, true);
            }
            ZoneKind::MotherTree => {
                self.set_inside(id, ZoneId::MotherTree, true);
                self.send_zone_message(id, zone, "enterMsgId");
            }
            ZoneKind::Jail => {
                self.set_inside(id, ZoneId::Jail, true);
                self.set_inside(id, ZoneId::NoSummonFriend, true);
                if self.settings.zones.jail_is_pvp {
                    self.set_inside(id, ZoneId::Pvp, true);
                    self.send(id, &SystemMessage::new(YouHaveEnteredACombatZone));
                }
                if self.settings.zones.jail_disable_transaction {
                    self.set_inside(id, ZoneId::NoStore, true);
                }
            }
            ZoneKind::NoStore => self.set_inside(id, ZoneId::NoStore, true),
            ZoneKind::NoRestart => self.set_inside(id, ZoneId::NoRestart, true),
            ZoneKind::NoSummonFriend => self.set_inside(id, ZoneId::NoSummonFriend, true),
            ZoneKind::Landing => self.set_inside(id, ZoneId::Landing, true),
            ZoneKind::NoLanding => {
                self.set_inside(id, ZoneId::NoLanding, true);
                self.no_landing_zone_entered(id, zone.param("dismountDelay"));
            }
            ZoneKind::Hq => self.set_inside(id, ZoneId::Hq, true),
            ZoneKind::Condition => {
                let (no_bookmark, no_item_drop) = condition_flags(zone);
                if no_bookmark {
                    self.set_inside(id, ZoneId::NoBookmark, true);
                }
                if no_item_drop {
                    self.set_inside(id, ZoneId::NoItemDrop, true);
                }
            }
            ZoneKind::Castle => self.set_inside(id, ZoneId::Castle, true),
            ZoneKind::Fort => self.set_inside(id, ZoneId::Fort, true),
            ZoneKind::Script => self.set_inside(id, ZoneId::Script, true),
            ZoneKind::DerbyTrack => self.set_inside(id, ZoneId::MonsterTrack, true),
            ZoneKind::Effect => {
                self.set_inside(id, ZoneId::Altered, true);
                if show_danger_icon(zone) {
                    self.set_inside(id, ZoneId::DangerArea, true);
                    self.send_etc_status_update(id);
                }
            }
            ZoneKind::FlagPvp => {
                self.update_pvp_flag(id, 1);
                self.send_message(id, "You entered a PvP Flag zone.");
                self.broadcast_user_info(id);
            }
            ZoneKind::ClanHall => {
                // Set as in clan hall, then the decorations of an auctionable hall.
                self.set_inside(id, ZoneId::ClanHall, true);
                let hall_id = zone.int_param("clanHallId").unwrap_or(0);
                self.on_clan_hall_zone_enter(id, hall_id);
            }
            ZoneKind::Siege => {
                // A fortress's zone (M7-FORT-01), else a castle's (M7-SIEGE-01); a hall's siege
                // zone is the castle code's with the hall's id (`clanhall/siege/zone.rs`).
                if self.fort_siege_zone_on_enter(id, zone).is_none() {
                    self.on_siege_zone_enter(id, zone);
                }
            }
            ZoneKind::Inert => debug!("zone {} ({}): nothing to do on enter", zone.id, zone.zone_type),
        }
    }

    /// The zone's `onExit(creature)` for a player.
    fn on_zone_exit(&mut self, id: ObjectId, zone: &Zone) {
        use SystemMessageId::YouHaveLeftACombatZone;
        let peace_mode = self.settings.zones.peace_zone_mode;
        match ZoneKind::of(&zone.zone_type) {
            ZoneKind::Town => self.set_inside(id, ZoneId::Town, false),
            ZoneKind::Peace => {
                if peace_mode != 2 {
                    self.set_inside(id, ZoneId::Peace, false);
                }
                // `getAllowStore()` is read again on exit.
                if !ZoneRules::of(zone).allow_store {
                    self.set_inside(id, ZoneId::NoStore, false);
                }
            }
            ZoneKind::NoPvp => {
                if peace_mode != 2 {
                    self.set_inside(id, ZoneId::NoPvp, false);
                }
                if !self.is_teleporting(id) {
                    self.broadcast_info(id);
                }
            }
            ZoneKind::Arena => {
                self.set_inside(id, ZoneId::Pvp, false);
                if !self.is_inside_zone(id, ZoneId::Pvp) {
                    self.send(id, &SystemMessage::new(YouHaveLeftACombatZone));
                }
            }
            ZoneKind::MotherTree => {
                self.set_inside(id, ZoneId::MotherTree, false);
                self.send_zone_message(id, zone, "leaveMsgId");
            }
            ZoneKind::Jail => {
                self.set_inside(id, ZoneId::Jail, false);
                self.set_inside(id, ZoneId::NoSummonFriend, false);
                if self.settings.zones.jail_is_pvp {
                    self.set_inside(id, ZoneId::Pvp, false);
                    self.send(id, &SystemMessage::new(YouHaveLeftACombatZone));
                }
                // When a player wants to exit jail even if he is still jailed, teleport him back.
                self.jail_zone_exit(id);
                if self.settings.zones.jail_disable_transaction {
                    self.set_inside(id, ZoneId::NoStore, false);
                }
            }
            ZoneKind::NoStore => self.set_inside(id, ZoneId::NoStore, false),
            ZoneKind::NoRestart => self.set_inside(id, ZoneId::NoRestart, false),
            ZoneKind::NoSummonFriend => self.set_inside(id, ZoneId::NoSummonFriend, false),
            ZoneKind::Landing => self.set_inside(id, ZoneId::Landing, false),
            ZoneKind::NoLanding => {
                self.set_inside(id, ZoneId::NoLanding, false);
                self.no_landing_zone_exited(id);
            }
            ZoneKind::Hq => self.set_inside(id, ZoneId::Hq, false),
            ZoneKind::Condition => {
                let (no_bookmark, no_item_drop) = condition_flags(zone);
                if no_bookmark {
                    self.set_inside(id, ZoneId::NoBookmark, false);
                }
                if no_item_drop {
                    self.set_inside(id, ZoneId::NoItemDrop, false);
                }
            }
            ZoneKind::Castle => self.set_inside(id, ZoneId::Castle, false),
            ZoneKind::Fort => self.set_inside(id, ZoneId::Fort, false),
            ZoneKind::Script => self.set_inside(id, ZoneId::Script, false),
            ZoneKind::DerbyTrack => self.set_inside(id, ZoneId::MonsterTrack, false),
            ZoneKind::Effect => {
                self.set_inside(id, ZoneId::Altered, false);
                if show_danger_icon(zone) {
                    self.set_inside(id, ZoneId::DangerArea, false);
                    if !self.is_inside_zone(id, ZoneId::DangerArea) {
                        self.send_etc_status_update(id);
                    }
                }
            }
            ZoneKind::FlagPvp => {
                if self.pvp_flag_of(id) > 0 {
                    self.update_pvp_flag(id, 0);
                }
                self.send_message(id, "You left the PvP Flag zone.");
                self.broadcast_user_info(id);
            }
            ZoneKind::ClanHall => self.set_inside(id, ZoneId::ClanHall, false),
            ZoneKind::Siege => {
                // A fortress's zone (M7-FORT-01), else a castle's (M7-SIEGE-01); a hall's siege
                // zone is the castle code's with the hall's id (`clanhall/siege/zone.rs`).
                if self.fort_siege_zone_on_exit(id, zone).is_none() {
                    self.on_siege_zone_exit(id, zone);
                }
            }
            ZoneKind::Inert => debug!("zone {} ({}): nothing to do on exit", zone.id, zone.zone_type),
        }
    }

    /// `new SystemMessage(_enterMsg)` of a `MotherTreeZone`: the id is a zone parameter, 0 sends
    /// nothing.
    fn send_zone_message(&self, id: ObjectId, zone: &Zone, param: &str) {
        let Some(raw) = zone.int_param(param).filter(|raw| *raw != 0) else {
            return;
        };
        match SystemMessageId::from_id(raw) {
            Some(message) => {
                self.send(id, &SystemMessage::new(message));
            }
            None => debug!("zone {}: unknown system message id {raw}", zone.id),
        }
    }

    /// The compass icon block of `Player.revalidateZone`.
    fn update_compass(&mut self, id: ObjectId) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let zones = &player.zones;
        // NOTE(unported): the Seven Signs dungeon icon (`_isIn7sDungeon`) comes between PvP and
        // peace; the flag is not loaded.
        let code = if zones.is_inside(ZoneId::Altered) {
            compass_zone::ALTEREDZONE
        } else if zones.is_inside(ZoneId::Siege) {
            compass_zone::SIEGEWARZONE2
        } else if zones.is_inside(ZoneId::Pvp) {
            compass_zone::PVPZONE
        } else if zones.is_inside(ZoneId::Peace) || zones.is_inside(ZoneId::NoPvp) {
            compass_zone::PEACEZONE
        } else {
            // NOTE(unported): leaving a siege zone calls `updatePvPStatus()` here.
            compass_zone::GENERALZONE
        };
        if player.zones.last_compass == code {
            return;
        }
        player.zones.last_compass = code;
        self.send(id, &ExSetCompassZoneCode { zone_type: code });
    }

    /// The `RandomRespawnInTownEnabled` switch of `MapRegionData`: a random restart point of
    /// the region, or always the first.
    fn restart_point_picker(&mut self) -> impl FnMut(usize) -> usize + '_ {
        let random = self.settings.teleport.random_respawn_in_town_enabled;
        let rng = self.rng();
        move |len: usize| {
            if random {
                usize::try_from(rng.below(i32::try_from(len).unwrap_or(i32::MAX))).unwrap_or(0)
            } else {
                0
            }
        }
    }

    /// Java `MapRegionData.getNearestTownRespawn(player)`: the restart point of the
    /// `RespawnZone` the player stands in, else of the map region, else of the default region.
    /// `None` is Java's `null`.
    pub fn nearest_town_respawn(&mut self, id: ObjectId) -> Option<(i32, i32, i32)> {
        let (zones, regions) = (self.zoning.zones.clone()?, self.zoning.map_regions.clone()?);
        let player = self.players.get(&id)?;
        let pos = l2_data::Location {
            x: player.core.x,
            y: player.core.y,
            z: player.core.z,
        };
        let race = race_name(player.data.race);
        let mut pick = self.restart_point_picker();
        let loc = regions.nearest_town_respawn(&zones, pos, race, &mut pick)?;
        Some((loc.x, loc.y, loc.z))
    }

    /// Java `MapRegionData.getNearestKarmaRespawn(player)`: like [`Self::nearest_town_respawn`]
    /// with the chaotic restart points.
    pub fn nearest_karma_respawn(&mut self, id: ObjectId) -> Option<(i32, i32, i32)> {
        let (zones, regions) = (self.zoning.zones.clone()?, self.zoning.map_regions.clone()?);
        let player = self.players.get(&id)?;
        let pos = l2_data::Location {
            x: player.core.x,
            y: player.core.y,
            z: player.core.z,
        };
        let race = race_name(player.data.race)?;
        // `player.isFlyingMounted()`: the flying fallback region.
        let flying_mounted = self.stats(id).is_some_and(|v| v.is_flying_mounted());
        let mut pick = self.restart_point_picker();
        let loc = regions.nearest_karma_respawn(&zones, pos, race, flying_mounted, &mut pick)?;
        Some((loc.x, loc.y, loc.z))
    }
}

/// Java `Race.name()` of a player race ordinal (`Race.java`: HUMAN, ELF, DARK_ELF, ORC, DWARF,
/// KAMAEL). `None` for an ordinal that is no player race; Java would fail the lookup too.
pub(crate) fn race_name(race: i32) -> Option<&'static str> {
    match race {
        0 => Some("HUMAN"),
        1 => Some("ELF"),
        2 => Some("DARK_ELF"),
        3 => Some("ORC"),
        4 => Some("DWARF"),
        5 => Some("KAMAEL"),
        _ => None,
    }
}

/// `ConditionZone`: the `NoBookmark` and `NoItemDrop` switches (names compared case-insensitively
/// like `equalsIgnoreCase`).
fn condition_flags(zone: &Zone) -> (bool, bool) {
    let (mut no_bookmark, mut no_item_drop) = (false, false);
    for (name, value) in &zone.params {
        if name.eq_ignore_ascii_case("NoBookmark") {
            no_bookmark = parse_bool(value);
        } else if name.eq_ignore_ascii_case("NoItemDrop") {
            no_item_drop = parse_bool(value);
        }
    }
    (no_bookmark, no_item_drop)
}

/// `EffectZone._isShowDangerIcon` (default true).
fn show_danger_icon(zone: &Zone) -> bool {
    zone.param("showDangerIcon").is_none_or(parse_bool)
}
