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
// flags and, since S-AI-07, the skills it applies: `ApplySkill`, `addSkill`, `removeSkill`,
// `clearSkills`, `removeEffectsOnExit`; see "EffectZone" below). Every other type is loaded and found by lookups, but
// entering it does nothing (`SiegeZone` is the siege's, `siege/zone.rs`, M7-SIEGE-01),
// `OlympiadStadiumZone`, `BossZone`, `FishingZone`, `RespawnZone` (restart points
// only). `WaterZone` is `water.rs`. `SwampZone` and `DamageZone`, the castle traps, are in `zone_trap.rs` (M7-FU-03). `FlagPvPZone` flags the player (M6-DUEL-01); its
// `onDieInside` / `onReviveInside` hooks need zone calls from the death code, which does not
// exist.
//
// What Java does in these zones that is left out, with the system that brings it:
// - `SiegeZone`: the dismount of a wyvern and the mounts it refuses (M7, `mount.rs` has the
//   `enteredNoLanding` half)
// - `NoRestartZone.onPlayerLoginInside`: the town teleport of a long absence: needs
//   `characters.lastAccess`; the login hook is not called
// - `isAffected`: the `affectedClassType` check (needs the class list)
// - `updatePvPStatus` after a siege zone
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
    /// `AllowWater`: the water zones start the breath task (`Player.revalidateZone`).
    pub allow_water: bool,
    /// `EnableFallingDamage` (`Formulas.calcFallDam`).
    pub enable_falling_damage: bool,
}

/// Datapack tables and the geoengine, as handed to the world at startup.
#[derive(Default)]
pub(crate) struct ZoneContext {
    pub zones: Option<Arc<Zones>>,
    pub map_regions: Option<Arc<MapRegions>>,
    pub geo: Option<Arc<GeoEngine>>,
    /// The running `DamageZone` tasks by zone id (`zone_trap.rs`).
    pub damage_tasks: std::collections::HashMap<i32, crate::timer::TimerId>,
    /// The running `WaterTask` by player (`water.rs`).
    pub water_tasks: std::collections::HashMap<ObjectId, crate::timer::TimerId>,
    /// The pending falling damage by player (`water.rs`).
    pub(crate) falling: std::collections::HashMap<ObjectId, crate::water::FallingDamage>,
    /// `EffectZone._skills` of the zones whose skills a script changed (`addSkill`,
    /// `removeSkill`, `clearSkills`), by zone id; the others have their `skillIdLvl` parameter.
    pub(crate) effect_skills: std::collections::HashMap<i32, Vec<(i32, i32)>>,
    /// The running `EffectZone.ApplySkill` tasks (`_task`) by zone id.
    pub(crate) effect_tasks: std::collections::HashMap<i32, crate::timer::TimerId>,
    /// `ZoneType._enabled` of the effect zones a script switched (`setEnabled`), by zone id; the
    /// others have their `default_enabled` parameter.
    pub(crate) effect_enabled: std::collections::HashMap<i32, bool>,
    /// The zones an NPC is in, for the scripts' `ON_CREATURE_ZONE_ENTER` / `EXIT` (S-API-05).
    pub(crate) npc_zones: std::collections::HashMap<ObjectId, NpcZones>,
}

/// An NPC's part of the zones' `_characterList`s and its `_lastZoneValidateLocation`. Only the
/// zones a script listens to are kept: the zone effects themselves (damage, peace, ...) are the
/// players' (S-API-05).
#[derive(Debug, Clone, Default)]
pub(crate) struct NpcZones {
    last_validate: Option<(i32, i32, i32)>,
    members: Vec<ZoneKey>,
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
    /// `instanceId` (`_instanceTemplateId`, default -1): the zone only takes creatures whose
    /// instance world is of this template (M8-INST-01).
    instance_template_id: i32,
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
            instance_template_id: -1,
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
                "instanceId" => rules.instance_template_id = int().unwrap_or(rules.instance_template_id),
                _ => {}
            }
        }
        rules
    }

    /// Java `ZoneType.isAffected(creature)` for an NPC: the instance, the switch, the level and
    /// `targetClass` (a player-only target class leaves NPCs out; races and classes are the
    /// players').
    fn affects_npc(&self, zone: &Zone, level: i32, world_template: Option<i32>) -> bool {
        if self.instance_template_id > 0
            && world_template.is_some_and(|template| template != self.instance_template_id)
        {
            return false;
        }
        if !self.enabled {
            return false;
        }
        if level < self.min_level || level > self.max_level {
            return false;
        }
        // `creature.isInstanceTypes(_target)`: `Playable` and `Player` are not NPCs.
        !zone
            .params
            .iter()
            .rev()
            .find(|(name, _)| name == "targetClass")
            .is_some_and(|(_, value)| matches!(value.as_str(), "Playable" | "Player"))
    }

    /// Java `ZoneType.isAffected(creature)` for a player. `world_template` is the template of the
    /// player's instance world (`InstanceManager.getWorld(creature)`), `None` without one.
    // NOTE(unported): `affectedClassType` (needs the class list) is not checked; no shipped zone
    // uses it.
    fn affects(&self, player: &Player, world_template: Option<i32>) -> bool {
        // Check instance.
        if self.instance_template_id > 0
            && world_template.is_some_and(|template| template != self.instance_template_id)
        {
            return false;
        }
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
    /// `SwampZone` (M7-FU-03, `zone_trap.rs`).
    Swamp,
    /// `DamageZone` (M7-FU-03, `zone_trap.rs`).
    Damage,
    /// `BossZone` (M8-BOSS-01, `boss/zone.rs`).
    Boss,
    /// `WaterZone` (`water.rs`).
    Water,
    /// `OlympiadStadiumZone` (M8-OLY-01, `olympiad/stadium.rs`).
    OlympiadStadium,
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
            "SwampZone" => Self::Swamp,
            "DamageZone" => Self::Damage,
            "BossZone" => Self::Boss,
            "WaterZone" => Self::Water,
            "OlympiadStadiumZone" => Self::OlympiadStadium,
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
        // `Creature.isInsideZone`: a PvP instance is a PvP zone and no peace zone, everywhere in
        // it (M8-INST-01).
        match zone {
            ZoneId::Pvp if self.is_in_pvp_instance(id) => return true,
            ZoneId::Peace if self.is_in_pvp_instance(id) => return false,
            _ => {}
        }
        self.players.get(&id).is_some_and(|p| p.zones.is_inside(zone))
    }

    // ------------------------------------------------------------------ one zone (S-BOSS-01)
    //
    // Java's `ZoneManager.getZoneById(id, Class)` and the `ZoneType` methods the boss scripts use.
    // Only players keep zone state in the world, so a zone holds a player by its `_characterList`
    // (the membership of `revalidateZone`) and an NPC by its position.

    /// Java `ZoneManager.getZoneById(id, Class)`.
    pub fn zone_by_id(&self, zone_id: i32, zone_type: &str) -> Option<&Zone> {
        self.zoning.zones.as_ref()?.get_of_type(zone_id, zone_type)
    }

    /// Java `ZoneType.getPlayersInside()`.
    pub fn zone_players_inside(&self, zone_id: i32, zone_type: &str) -> Vec<ObjectId> {
        let Some(zone) = self.zone_by_id(zone_id, zone_type) else {
            return Vec::new();
        };
        self.players
            .values()
            .filter(|p| p.zones.is_member(zone))
            .map(|p| p.core.object_id)
            .collect()
    }

    /// Java `ZoneType.getCharactersInside()`: the players inside, then the NPCs whose position is
    /// in the zone.
    // NOTE: Java's NPCs join a zone's `_characterList` when they move (`revalidateZone`); an NPC
    // here is in the zone where it stands, which is the same once it stopped.
    pub fn zone_characters_inside(&self, zone_id: i32, zone_type: &str) -> Vec<ObjectId> {
        let Some(zone) = self.zone_by_id(zone_id, zone_type) else {
            return Vec::new();
        };
        let mut inside = self.zone_players_inside(zone_id, zone_type);
        let mut npcs: Vec<ObjectId> = self
            .npcs
            .keys()
            .copied()
            .filter(|id| {
                self.objects.get(id).is_some_and(|e| e.spawned)
                    && self
                        .position_of(*id)
                        .is_some_and(|(x, y, z)| zone.is_inside_zone(x, y, z))
            })
            .collect();
        npcs.sort_by_key(|id| id.0);
        inside.extend(npcs);
        inside
    }

    /// Java `ZoneType.isCharacterInZone(creature)`.
    pub fn is_character_in_zone(&self, zone_id: i32, zone_type: &str, id: ObjectId) -> bool {
        let Some(zone) = self.zone_by_id(zone_id, zone_type) else {
            return false;
        };
        match self.players.get(&id) {
            Some(player) => player.zones.is_member(zone),
            None => self.is_npc(id) && self.is_inside_zone_by_position(zone_id, zone_type, id),
        }
    }

    /// Java `ZoneType.isInsideZone(object)`: by the object's position.
    pub fn is_inside_zone_by_position(&self, zone_id: i32, zone_type: &str, id: ObjectId) -> bool {
        let Some(zone) = self.zone_by_id(zone_id, zone_type) else {
            return false;
        };
        self.position_of(id)
            .is_some_and(|(x, y, z)| zone.is_inside_zone(x, y, z))
    }

    /// Java `ZoneType.broadcastPacket(packet)`: to every player inside.
    pub fn broadcast_to_zone(&self, zone_id: i32, zone_type: &str, packet: &impl l2_net::Encode) {
        for player in self.zone_players_inside(zone_id, zone_type) {
            self.send(player, packet);
        }
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
        if self.npcs.contains_key(&id) {
            self.revalidate_npc_script_zones(id, force);
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
        if self.settings.zones.allow_water {
            self.check_water_state(id);
        }
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
        let trap_override = self.trap_zone_override(zone);
        let world_template = self.instance_world_of(id).map(|w| w.template_id);
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
        let mut rules = ZoneRules::of(zone);
        // The flame towers switch the castle trap zones (`setEnabled`, M7-FU-03).
        if let Some(enabled) = trap_override {
            rules.enabled = enabled;
        }
        // "If the character can't be affected by this zone return."
        if !rules.affects(player, world_template) {
            return;
        }
        // `OlympiadStadiumZone.isAffected`: only the creatures of the stadium's instance.
        if matches!(ZoneKind::of(&zone.zone_type), ZoneKind::OlympiadStadium)
            && !self.stadium_takes(zone.id, id)
        {
            return;
        }
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        player.zones.members.push((zone.id, zone.zone_type.clone()));
        // "Notify to scripts." (`ON_CREATURE_ZONE_ENTER`, S-API-01), then the zone itself.
        self.script_on_zone_enter(id, zone.id);
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
        // "Notify to scripts." (`ON_CREATURE_ZONE_EXIT`, S-API-01), then the zone itself.
        self.script_on_zone_exit(id, zone.id);
        let Some(player) = self.players.get_mut(&id) else {
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
                // "if (_skills != null)": the `ApplySkill` task starts with the first one in.
                self.effect_zone_start_task(zone);
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
            ZoneKind::Swamp => self.swamp_zone_enter(id, zone),
            ZoneKind::Damage => self.damage_zone_enter(id, zone),
            ZoneKind::Boss => self.boss_zone_enter(id, zone.id),
            ZoneKind::Water => self.water_zone_enter(id),
            ZoneKind::OlympiadStadium => self.stadium_on_enter(id, zone),
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
                self.effect_zone_exit(id, zone);
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
            ZoneKind::Swamp => self.swamp_zone_exit(id, zone),
            ZoneKind::Damage => self.damage_zone_exit(zone),
            ZoneKind::Boss => self.boss_zone_exit(id, zone.id, true),
            ZoneKind::Water => self.water_zone_exit(id),
            ZoneKind::OlympiadStadium => self.stadium_on_exit(id, zone),
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
        let code = if zones.is_inside(ZoneId::Altered) {
            compass_zone::ALTEREDZONE
        } else if zones.is_inside(ZoneId::Siege) {
            compass_zone::SIEGEWARZONE2
        } else if zones.is_inside(ZoneId::Pvp) {
            compass_zone::PVPZONE
        } else if player.data.profile.in_7s_dungeon {
            // `_isIn7sDungeon` (M8-SS-01).
            compass_zone::SEVENSIGNSZONE
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

    /// Java `MapRegionData.getNearestTownRespawn(creature)` for a creature that is no player (a
    /// boat): the restart point of the `RespawnZone` or the map region at the place.
    pub(crate) fn nearest_town_respawn_at(&mut self, x: i32, y: i32, z: i32) -> Option<(i32, i32, i32)> {
        let (zones, regions) = (self.zoning.zones.clone()?, self.zoning.map_regions.clone()?);
        let mut pick = self.restart_point_picker();
        let loc = regions.nearest_town_respawn(&zones, l2_data::Location { x, y, z }, None, &mut pick)?;
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

// ---------------------------------------------------------------------------------- EffectZone
//
// Java `EffectZone`: while players are inside, a task (`ApplySkill`, `initialDelay` then every
// `reuse` milliseconds) gives each living player, with `chance` percent, every skill of the zone
// he is not affected by yet. The skills come from `skillIdLvl` ("id-level;id-level") or are added
// by scripts (`addSkill`, the Seed of Annihilation buffs); `maxDynamicSkillCount` makes an empty
// list scripts fill.
//
// NOTE: Java keeps `_skills` in a `ConcurrentHashMap`; the skills are given here in the order
// they were added (an added id that is there already keeps its place).
// NOTE(java-quirk): `ThreadPool.scheduleAtFixedRate(.., initialDelay, reuse)`: an initial delay
// of 0 runs the first pass on the next tick.

/// `EffectZone` defaults: `_chance`, `_initialDelay`, `_reuse`.
const EFFECT_DEFAULT_CHANCE: i32 = 100;
const EFFECT_DEFAULT_INITIAL_DELAY_MS: i64 = 0;
const EFFECT_DEFAULT_REUSE_MS: i64 = 30_000;

/// The parameters `EffectZone.setParameter` keeps, read from the zone's raw `<stat>` list in file
/// order (a later `skillIdLvl` or `maxDynamicSkillCount` replaces the list).
#[derive(Debug, Clone, PartialEq, Eq)]
struct EffectParams {
    chance: i32,
    initial_delay_ms: i64,
    reuse_ms: i64,
    bypass_conditions: bool,
    remove_effects_on_exit: bool,
    /// `_skills`: `None` is Java's `null` (no skill list, no task).
    skills: Option<Vec<(i32, i32)>>,
}

impl EffectParams {
    fn of(zone: &Zone) -> Self {
        let mut p = Self {
            chance: EFFECT_DEFAULT_CHANCE,
            initial_delay_ms: EFFECT_DEFAULT_INITIAL_DELAY_MS,
            reuse_ms: EFFECT_DEFAULT_REUSE_MS,
            bypass_conditions: false,
            remove_effects_on_exit: false,
            skills: None,
        };
        // A number `Integer.parseInt` rejects is skipped (Java's loader throws).
        let int = |value: &str| value.trim().parse::<i32>().ok();
        for (name, value) in &zone.params {
            match name.as_str() {
                "chance" => p.chance = int(value).unwrap_or(p.chance),
                "initialDelay" => p.initial_delay_ms = int(value).map_or(p.initial_delay_ms, i64::from),
                "reuse" => p.reuse_ms = int(value).map_or(p.reuse_ms, i64::from),
                "bypassSkillConditions" => p.bypass_conditions = parse_bool(value),
                "maxDynamicSkillCount" => p.skills = Some(Vec::new()),
                "removeEffectsOnExit" => p.remove_effects_on_exit = parse_bool(value),
                "skillIdLvl" => {
                    let mut skills: Vec<(i32, i32)> = Vec::new();
                    for skill in value.split(';') {
                        let split: Vec<&str> = skill.split('-').collect();
                        if split.len() != 2 {
                            debug!(
                                "EffectZone {}: invalid config property -> skillsIdLvl \"{skill}\"",
                                zone.id
                            );
                            continue;
                        }
                        if let (Some(id), Some(level)) = (int(split[0]), int(split[1])) {
                            put_skill(&mut skills, id, level);
                        }
                    }
                    p.skills = Some(skills);
                }
                _ => {}
            }
        }
        p
    }
}

/// `_skills.put(id, level)`.
fn put_skill(skills: &mut Vec<(i32, i32)>, id: i32, level: i32) {
    match skills.iter_mut().find(|(skill, _)| *skill == id) {
        Some(entry) => entry.1 = level,
        None => skills.push((id, level)),
    }
}

impl World {
    /// The effect zone of that id (Java `ZoneManager.getZoneById(id, EffectZone.class)`).
    fn effect_zone(&self, zone_id: i32) -> Option<Zone> {
        self.zoning
            .zones
            .as_ref()
            .and_then(|z| z.get_of_type(zone_id, "EffectZone"))
            .cloned()
    }

    /// `EffectZone._skills` of a zone: what scripts made of it, else its parameters.
    fn effect_zone_skills(&self, zone: &Zone) -> Option<Vec<(i32, i32)>> {
        match self.zoning.effect_skills.get(&zone.id) {
            Some(skills) => Some(skills.clone()),
            None => EffectParams::of(zone).skills,
        }
    }

    /// Java `ZoneManager.getZoneByName(name)`: the id of the first zone with that name.
    pub fn zone_id_by_name(&self, name: &str) -> Option<i32> {
        self.zoning
            .zones
            .as_ref()?
            .iter()
            .find(|z| z.name.as_deref() == Some(name))
            .map(|z| z.id)
    }

    /// Java `ZoneType.isEnabled()` of an effect zone: what a script set, else `default_enabled`.
    pub fn is_effect_zone_enabled(&self, zone_id: i32) -> bool {
        if let Some(enabled) = self.zoning.effect_enabled.get(&zone_id) {
            return *enabled;
        }
        self.effect_zone(zone_id)
            .is_some_and(|zone| zone.param("default_enabled").is_none_or(parse_bool))
    }

    /// Java `ZoneType.setEnabled(value)` of an effect zone.
    pub fn set_effect_zone_enabled(&mut self, zone_id: i32, enabled: bool) {
        if self.effect_zone(zone_id).is_some() {
            self.zoning.effect_enabled.insert(zone_id, enabled);
        }
    }

    /// Java `EffectZone.addSkill(skillId, skillLevel)`: a level below 1 removes the skill.
    pub fn effect_zone_add_skill(&mut self, zone_id: i32, skill_id: i32, skill_level: i32) {
        if skill_level < 1 {
            self.effect_zone_remove_skill(zone_id, skill_id);
            return;
        }
        let Some(zone) = self.effect_zone(zone_id) else {
            return;
        };
        // "if (_skills == null) _skills = new ConcurrentHashMap<>(3)".
        let mut skills = self.effect_zone_skills(&zone).unwrap_or_default();
        put_skill(&mut skills, skill_id, skill_level);
        self.zoning.effect_skills.insert(zone_id, skills);
    }

    /// Java `EffectZone.removeSkill(skillId)`.
    pub fn effect_zone_remove_skill(&mut self, zone_id: i32, skill_id: i32) {
        let Some(zone) = self.effect_zone(zone_id) else {
            return;
        };
        if let Some(mut skills) = self.effect_zone_skills(&zone) {
            skills.retain(|(id, _)| *id != skill_id);
            self.zoning.effect_skills.insert(zone_id, skills);
        }
    }

    /// Java `EffectZone.clearSkills()`.
    pub fn effect_zone_clear_skills(&mut self, zone_id: i32) {
        let Some(zone) = self.effect_zone(zone_id) else {
            return;
        };
        if self.effect_zone_skills(&zone).is_some() {
            self.zoning.effect_skills.insert(zone_id, Vec::new());
        }
    }

    /// Java `EffectZone.getSkillLevel(skillId)`: 0 without that skill.
    pub fn effect_zone_skill_level(&self, zone_id: i32, skill_id: i32) -> i32 {
        self.effect_zone(zone_id)
            .and_then(|zone| self.effect_zone_skills(&zone))
            .and_then(|skills| {
                skills
                    .iter()
                    .find(|(id, _)| *id == skill_id)
                    .map(|(_, level)| *level)
            })
            .unwrap_or(0)
    }

    /// The `_task == null` block of `EffectZone.onEnter`.
    fn effect_zone_start_task(&mut self, zone: &Zone) {
        if self.zoning.effect_tasks.contains_key(&zone.id) || self.effect_zone_skills(zone).is_none() {
            return;
        }
        let delay = EffectParams::of(zone).initial_delay_ms;
        let timer = self.schedule_zone_timer(crate::zone_trap::ZoneTimer::EffectZone(zone.id), delay);
        self.zoning.effect_tasks.insert(zone.id, timer);
    }

    /// The rest of `EffectZone.onExit(creature)` for a player: the skills of a
    /// `removeEffectsOnExit` zone go, and the task stops with the last one out.
    fn effect_zone_exit(&mut self, id: ObjectId, zone: &Zone) {
        let params = EffectParams::of(zone);
        if params.remove_effects_on_exit
            && let Some(skills) = self.effect_zone_skills(zone)
        {
            let table = self.skill_table();
            for (skill_id, level) in skills {
                let exists = table.as_ref().is_some_and(|t| t.get(skill_id, level).is_some());
                if exists && self.is_affected_by_skill(id, skill_id) {
                    self.stop_skill_effects(id, crate::effects::SkillFinishType::Removed, skill_id);
                }
            }
        }
        if self.zone_players_of(zone).is_empty()
            && let Some(timer) = self.zoning.effect_tasks.remove(&zone.id)
        {
            self.timers.cancel(timer);
        }
    }

    /// The players a zone holds (Java `getCharactersInside()`, players only), by object id.
    fn zone_players_of(&self, zone: &Zone) -> Vec<ObjectId> {
        let mut inside: Vec<ObjectId> = self
            .players
            .iter()
            .filter(|(_, p)| p.zones.is_member(zone))
            .map(|(id, _)| *id)
            .collect();
        inside.sort();
        inside
    }

    /// `EffectZone.ApplySkill.run()`.
    pub(crate) fn on_effect_zone_run(&mut self, zone_id: i32) {
        // The timer that fired is spent; the task goes on only by scheduling the next pass.
        self.zoning.effect_tasks.remove(&zone_id);
        let Some(zone) = self.effect_zone(zone_id) else {
            return;
        };
        let inside = self.zone_players_of(&zone);
        if inside.is_empty() {
            return;
        }
        let params = EffectParams::of(&zone);
        // A period of 0 would be an `IllegalArgumentException` in Java; one tick is the floor.
        let timer = self.schedule_zone_timer(
            crate::zone_trap::ZoneTimer::EffectZone(zone_id),
            params.reuse_ms.max(1),
        );
        self.zoning.effect_tasks.insert(zone_id, timer);
        if !self.is_effect_zone_enabled(zone_id) {
            return;
        }
        let skills = self.effect_zone_skills(&zone).unwrap_or_default();
        let table = self.skill_table();
        for player in inside {
            if self.is_dead(player) || self.rng().below(100) >= params.chance {
                continue;
            }
            for &(skill_id, level) in &skills {
                if !table.as_ref().is_some_and(|t| t.get(skill_id, level).is_some()) {
                    continue;
                }
                let key = crate::effects::SkillKey::new(skill_id, level);
                if (params.bypass_conditions || self.check_skill_condition(player, player, key))
                    && !self.is_affected_by_skill(player, skill_id)
                {
                    self.apply_effects(player, player, key);
                }
            }
        }
    }
}

impl World {
    /// `Creature.revalidateZone(force)` of an NPC, for the zones scripts listen to
    /// (`addEnterZoneId` / `addExitZoneId`): Java's `ON_CREATURE_ZONE_ENTER` / `EXIT` of NPCs
    /// (S-API-05). Without `force` it runs once the NPC is `MaxDriftRange` away from the last
    /// check (100 while in combat), like Java.
    fn revalidate_npc_script_zones(&mut self, id: ObjectId, force: bool) {
        let known = self.zoning.npc_zones.contains_key(&id);
        if self.scripts.listeners.listened_zone_ids().is_empty() && !known {
            return;
        }
        if self.objects.get(&id).and_then(|e| e.region).is_none() {
            return;
        }
        let Some((x, y, z)) = self.position_of(id) else {
            return;
        };
        let drift = if self.is_in_combat(id) {
            100.0
        } else {
            f64::from(self.ai.attackable.settings.max_drift_range)
        };
        let last = self.zoning.npc_zones.get(&id).and_then(|s| s.last_validate);
        if !force && let Some((lx, ly, lz)) = last {
            let (dx, dy, dz) = (f64::from(x - lx), f64::from(y - ly), f64::from(z - lz));
            if (dx * dx + dy * dy + dz * dz).sqrt() < drift {
                return;
            }
        }
        self.zoning.npc_zones.entry(id).or_default().last_validate = Some((x, y, z));
        let listened = self.scripts.listeners.listened_zone_ids().to_vec();
        let Some(zones) = self.zoning.zones.clone() else {
            return;
        };
        let level = self.npcs.get(&id).map_or(0, |n| n.core.level);
        let world_template = self.instance_world_of(id).map(|w| w.template_id);
        for zone in zones.region_zones(x, y) {
            if listened.binary_search(&zone.id).is_err() {
                continue;
            }
            let key: ZoneKey = (zone.id, zone.zone_type.clone());
            let member = self
                .zoning
                .npc_zones
                .get(&id)
                .is_some_and(|s| s.members.contains(&key));
            if zone.is_inside_zone(x, y, z) {
                if member {
                    continue;
                }
                let mut rules = ZoneRules::of(zone);
                if let Some(enabled) = self.trap_zone_override(zone) {
                    rules.enabled = enabled;
                }
                if matches!(ZoneKind::of(&zone.zone_type), ZoneKind::Effect) {
                    rules.enabled = self.is_effect_zone_enabled(zone.id);
                }
                if !rules.affects_npc(zone, level, world_template) {
                    continue;
                }
                self.zoning.npc_zones.entry(id).or_default().members.push(key);
                self.script_on_zone_enter(id, zone.id);
            } else if member {
                if let Some(state) = self.zoning.npc_zones.get_mut(&id) {
                    state.members.retain(|k| *k != key);
                }
                self.script_on_zone_exit(id, zone.id);
            }
        }
        // `Creature.setXYZ`: a new zone cell drops the zones of the old one
        // (`oldZoneRegion.removeFromZones(this)`).
        let members = self
            .zoning
            .npc_zones
            .get(&id)
            .map(|s| s.members.clone())
            .unwrap_or_default();
        for key in members {
            let inside = zones
                .get_of_type(key.0, &key.1)
                .is_some_and(|zone| zone.is_inside_zone(x, y, z));
            if !inside {
                if let Some(state) = self.zoning.npc_zones.get_mut(&id) {
                    state.members.retain(|k| *k != key);
                }
                self.script_on_zone_exit(id, key.0);
            }
        }
    }

    /// `ZoneRegion.removeFromZones(npc)` of `Npc.deleteMe` / `Creature.onDecay`: the NPC leaves
    /// every zone it was in (the scripts hear the exits).
    pub(crate) fn npc_leave_script_zones(&mut self, id: ObjectId) {
        let Some(state) = self.zoning.npc_zones.remove(&id) else {
            return;
        };
        for (zone_id, _) in state.members {
            self.script_on_zone_exit(id, zone_id);
        }
    }

    /// The zones (id, type) a script listens to that the NPC is in.
    pub fn npc_script_zones(&self, id: ObjectId) -> Vec<(i32, String)> {
        self.zoning
            .npc_zones
            .get(&id)
            .map(|s| s.members.clone())
            .unwrap_or_default()
    }
}
