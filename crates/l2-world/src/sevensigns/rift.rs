//! The Dimensional Rift (M8-FEST-01): the rooms and their monsters, a party entering the rift, the
//! jumps from room to room, the boss room, and the leader's NPC commands.
// Java: managers/DimensionalRiftManager.java
// Java: mechanics/sevensigns/{DimensionalRift,DimensionalRiftRoom}.java
// Java: entity/actor/instance/RiftInvader.java
// Java: dist/game/data/scripts/handlers/bypass/npc/Rift.java
//
// # Shape
// - The rooms are the rows of `dimensional_rift` (read at start, never written) and the monsters
//   are `data/DimensionalRift.xml`; each monster is a runtime spawn of its room.
// - A running rift is a [`Rift`] in the world's table (`RiftId`), and `Party._dr` is the map from
//   a party to its rift (D-11: ids, no references). Java's two `java.util.Timer`s and the
//   `ThreadPool` task of the earthquake are three timers of the wheel ([`RiftTimer`]).
// - The room a party stands in is chosen with the world's RNG.
//
// # Left out, with the card that brings it
// - Quest 635 (`Q00635_IntoTheDimensionalRift`): a script (D-05). Java starts it when a party
//   enters and exits it in the waiting room; the quest script has to do both from the rift events.
// - `OracleTeleport` (the Script bypass that brings players to the waiting room) is a script too.
// - The `Player.usedTeleport` caller (`DimensionalRift.usedTeleport`) is a script's; the method is
//   here.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use l2_data::RiftSpawns;
use l2_protocol_game::server::Earthquake;
use l2_util::ObjectId;
use tracing::{info, warn};

use super::SevenSignsTimer;
use crate::party::PartyId;
use crate::teleport::TeleportTarget;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `DIMENSIONAL_FRAGMENT_ITEM_ID`: what a party pays to enter.
pub const DIMENSIONAL_FRAGMENT_ITEM_ID: i32 = 7079;
/// `FIVE_SECONDS`.
const FIVE_SECONDS: i64 = 5000;
const RIFT_HTML: &str = "data/html/seven_signs/rift/";

/// The values of `General.ini` the rift reads (`GeneralConfig.RIFT_*`), `Default` the Java
/// defaults.
#[derive(Debug, Clone, PartialEq)]
pub struct RiftSettings {
    /// `RiftMinPartySize`.
    pub min_party_size: i32,
    /// `MaxRiftJumps`: 1 to 8, else 4.
    pub max_jumps: i32,
    /// `RiftSpawnDelay`: ms after arriving until the room's monsters spawn.
    pub spawn_delay: i32,
    /// `AutoJumpsDelayMin` / `AutoJumpsDelayMax`: seconds until the rift jumps by itself.
    pub auto_jumps_time_min: i32,
    pub auto_jumps_time_max: i32,
    /// `BossRoomTimeMultiply`.
    pub boss_room_time_multiply: f32,
    /// `RecruitCost` ... `HeroCost`: dimensional fragments of areas 1 to 6.
    pub enter_cost: [i32; 6],
}

impl Default for RiftSettings {
    fn default() -> Self {
        Self {
            min_party_size: 5,
            max_jumps: 4,
            spawn_delay: 10_000,
            auto_jumps_time_min: 480,
            auto_jumps_time_max: 600,
            boss_room_time_multiply: 1.5,
            enter_cost: [18, 21, 24, 27, 30, 33],
        }
    }
}

/// A row of `dimensional_rift`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RiftRoomDef {
    /// `type`: 0 the waiting room and the rift zone, 1 to 6 the areas.
    pub kind: i8,
    pub room: i8,
    pub x_min: i32,
    pub x_max: i32,
    pub y_min: i32,
    pub y_max: i32,
    pub z_min: i32,
    pub z_max: i32,
    /// Where the party arrives.
    pub x_t: i32,
    pub y_t: i32,
    pub z_t: i32,
    pub boss: bool,
}

/// The id of a running rift.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct RiftId(pub u32);

/// The rift timers (inside `SevenSignsTimer::Rift`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RiftTimer {
    /// `teleporterTimerTask`: the jump, or (`reason_tp = false`) the removal of a wrong member.
    Teleport { rift: RiftId, reason_tp: bool },
    /// `spawnTimerTask`: the monsters of `room` appear.
    Spawn { rift: RiftId, room: i8 },
    /// `earthQuakeTask`: the warning seven seconds before the jump.
    Earthquake(RiftId),
}

/// Java `DimensionalRiftRoom`.
#[derive(Debug, Clone)]
struct Room {
    def: RiftRoomDef,
    /// `_xMin` ... `_yMax`: the area monsters spawn in is 128 smaller on every side.
    spawn_x: (i32, i32),
    spawn_y: (i32, i32),
    /// `_roomSpawns` (runtime spawn indices).
    spawns: Vec<usize>,
    party_inside: bool,
}

impl Room {
    /// `checkIfInZone(x, y, z)`.
    // `java.awt.Polygon.contains` of the four corners: the left and top edges are inside, the right
    // and bottom ones are not.
    fn contains(&self, x: i32, y: i32, z: i32) -> bool {
        let d = &self.def;
        x >= d.x_min && x < d.x_max && y >= d.y_min && y < d.y_max && z >= d.z_min && z <= d.z_max
    }
}

/// Java `DimensionalRift`: one party's run.
#[derive(Debug)]
pub(crate) struct Rift {
    kind: i8,
    /// `_party` (`None` after `killRift`).
    party: Option<PartyId>,
    completed_rooms: Vec<i8>,
    /// `currentJump`: starts at 1 to count the first room.
    current_jump: i32,
    /// `_choosenRoom`.
    chosen_room: i8,
    dead: BTreeSet<ObjectId>,
    revived_in_waiting_room: BTreeSet<ObjectId>,
    boss_room: bool,
    teleport_timer: Option<TimerId>,
    spawn_timer: Option<TimerId>,
    earthquake_timer: Option<TimerId>,
}

/// The rift state of the world.
#[derive(Debug, Default)]
pub(crate) struct RiftWorld {
    settings: RiftSettings,
    /// `_rooms`: by type, then by room.
    rooms: BTreeMap<i8, BTreeMap<i8, Room>>,
    rifts: BTreeMap<RiftId, Rift>,
    /// `Party._dr`.
    by_party: HashMap<PartyId, RiftId>,
    next_id: u32,
}

impl World {
    // ------------------------------------------------------------------ the manager

    /// Installs the rift configuration. Call before [`Self::load_dimensional_rift`].
    pub fn set_rift_settings(&mut self, settings: RiftSettings) {
        self.seven_signs.rift.settings = settings;
    }

    /// The rift configuration.
    pub fn rift_settings(&self) -> &RiftSettings {
        &self.seven_signs.rift.settings
    }

    /// Java `DimensionalRiftManager`'s constructor: `loadRooms()` then `loadSpawns()`. A room
    /// whose type or number the table does not have makes a warning and a counted error.
    pub fn load_dimensional_rift(&mut self, rows: Vec<RiftRoomDef>, spawns: &RiftSpawns) {
        self.seven_signs.rift.rooms.clear();
        for def in rows {
            self.seven_signs.rift.rooms.entry(def.kind).or_default().insert(
                def.room,
                Room {
                    def,
                    spawn_x: (def.x_min + 128, def.x_max - 128),
                    spawn_y: (def.y_min + 128, def.y_max - 128),
                    spawns: Vec::new(),
                    party_inside: false,
                },
            );
        }
        let types = self.seven_signs.rift.rooms.len();
        let rooms: usize = self.seven_signs.rift.rooms.values().map(BTreeMap::len).sum();
        info!("DimensionalRiftManager: Loaded {types} room types with {rooms} rooms.");
        let (mut good, mut bad) = (0, 0);
        for def in spawns.iter() {
            if !self.seven_signs.rift.rooms.contains_key(&def.area) {
                warn!("Type {} not found!", def.area);
            } else if !self
                .seven_signs
                .rift
                .rooms
                .get(&def.area)
                .is_some_and(|r| r.contains_key(&def.room))
            {
                warn!("Room {} in Type {} not found!", def.room, def.area);
            }
            for _ in 0..def.count {
                let Some(room) = self
                    .seven_signs
                    .rift
                    .rooms
                    .get(&def.area)
                    .and_then(|r| r.get(&def.room))
                else {
                    bad += 1;
                    continue;
                };
                let (x_range, y_range, z) = (room.spawn_x, room.spawn_y, room.def.z_t);
                let x = self.rng().get(x_range.0, x_range.1);
                let y = self.rng().get(y_range.0, y_range.1);
                // `new Spawn(mobId)` throws for an unknown template or a class Java cannot build.
                let constructible = self
                    .npc_world
                    .template(def.mob_id)
                    .is_some_and(|t| crate::npc::NpcClass::from_type(&t.npc_type).is_some());
                if !constructible {
                    bad += 1;
                    continue;
                }
                // `setRespawnDelay(delay)`: `Math.max(1, delay) * 1000`.
                let delay_ms = def.delay.max(1).saturating_mul(1000);
                let index = self.add_runtime_spawn(l2_data::spawn::NpcSpawn {
                    npc_id: def.mob_id,
                    amount: 1,
                    location: l2_data::Location { x, y, z },
                    // `setHeading(-1)`: a random one when it spawns.
                    heading: -1,
                    respawn_min_ms: delay_ms,
                    respawn_max_ms: delay_ms,
                    respawn_enabled: true,
                    chase_range: 0,
                    name: None,
                    territory: None,
                    period_of_day: None,
                    ai_data: None,
                    group: None,
                    source: "dimensional_rift".to_owned(),
                });
                if let Some(room) = self
                    .seven_signs
                    .rift
                    .rooms
                    .get_mut(&def.area)
                    .and_then(|r| r.get_mut(&def.room))
                {
                    room.spawns.push(index);
                    good += 1;
                }
            }
        }
        info!("DimensionalRiftManager: Loaded {good} dimensional rift spawns, {bad} errors.");
    }

    fn rift_room(&self, kind: i8, room: i8) -> Option<&Room> {
        self.seven_signs.rift.rooms.get(&kind)?.get(&room)
    }

    fn rift_room_mut(&mut self, kind: i8, room: i8) -> Option<&mut Room> {
        self.seven_signs.rift.rooms.get_mut(&kind)?.get_mut(&room)
    }

    /// `getRoom(type, room).getSpawns()`'s size (tests).
    pub fn rift_room_spawn_count(&self, kind: i8, room: i8) -> usize {
        self.rift_room(kind, room).map_or(0, |r| r.spawns.len())
    }

    /// `getRoom(type, room).isPartyInside()` (tests).
    pub fn rift_room_party_inside(&self, kind: i8, room: i8) -> bool {
        self.rift_room(kind, room).is_some_and(|r| r.party_inside)
    }

    /// The monsters of a room now in the world (tests).
    pub fn rift_room_monsters(&self, kind: i8, room: i8) -> Vec<ObjectId> {
        self.rift_room(kind, room)
            .map(|r| r.spawns.clone())
            .unwrap_or_default()
            .into_iter()
            .filter_map(|index| self.last_spawn(index))
            .collect()
    }

    /// `checkIfInRiftZone(x, y, z, ignorePeaceZone)`.
    pub fn rift_check_in_rift_zone(&self, x: i32, y: i32, z: i32, ignore_peace_zone: bool) -> bool {
        let in_rift = self.rift_room(0, 1).is_some_and(|r| r.contains(x, y, z));
        if ignore_peace_zone {
            return in_rift;
        }
        in_rift && !self.rift_room(0, 0).is_some_and(|r| r.contains(x, y, z))
    }

    /// `checkIfInPeaceZone(x, y, z)`: the waiting room.
    pub fn rift_check_in_peace_zone(&self, x: i32, y: i32, z: i32) -> bool {
        self.rift_room(0, 0).is_some_and(|r| r.contains(x, y, z))
    }

    /// `teleportToWaitingRoom(player)`.
    pub fn rift_teleport_to_waiting_room(&mut self, player: ObjectId) {
        let Some(d) = self.rift_room(0, 0).map(|r| r.def) else {
            return;
        };
        self.tele_to_location(player, TeleportTarget::at(d.x_t, d.y_t, d.z_t).with_instance(0));
    }

    /// `Room.spawn()`: every spawn of the room creates its monster and starts respawning.
    fn rift_room_spawn(&mut self, kind: i8, room: i8) {
        let spawns = self
            .rift_room(kind, room)
            .map(|r| r.spawns.clone())
            .unwrap_or_default();
        for index in spawns {
            let mut report = crate::npc::SpawnReport::default();
            self.initialize_npc(index, None, &mut report);
            self.start_respawn(index);
        }
    }

    /// `Room.unspawn()`: no more respawns, the monsters leave.
    fn rift_room_unspawn(&mut self, kind: i8, room: i8) {
        let spawns = self
            .rift_room(kind, room)
            .map(|r| r.spawns.clone())
            .unwrap_or_default();
        for index in spawns {
            self.stop_respawn(index);
            if let Some(npc) = self.last_spawn(index) {
                self.delete_npc(npc);
            }
        }
    }

    /// `isAllowedEnter(type)`: at most (rooms - 1) parties are inside.
    fn rift_is_allowed_enter(&self, kind: i8) -> bool {
        let Some(rooms) = self.seven_signs.rift.rooms.get(&kind) else {
            return false;
        };
        let count = rooms.values().filter(|r| r.party_inside).count();
        count < rooms.len().saturating_sub(1)
    }

    /// `getFreeRooms(type)`: the numbers of the rooms without a party.
    fn rift_free_rooms(&self, kind: i8) -> Vec<i8> {
        self.seven_signs
            .rift
            .rooms
            .get(&kind)
            .map(|rooms| {
                rooms
                    .values()
                    .filter(|r| !r.party_inside)
                    .map(|r| r.def.room)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// `getNeededItems(type)`; `None` where Java throws.
    fn rift_needed_items(&self, kind: i8) -> Option<i32> {
        let index = usize::try_from(kind).ok()?.checked_sub(1)?;
        self.seven_signs.rift.settings.enter_cost.get(index).copied()
    }

    /// `showHtmlFile(player, file, npc)`.
    fn rift_show_html_file(&mut self, player: ObjectId, file: &str, npc: ObjectId) {
        let name = self.creature_name(npc);
        if let Some(mut html) = self.html_from_file(player, npc.0, file) {
            html.html = html.html.replace("%npc_name%", &name);
            self.send_html(player, &html);
        }
    }

    /// A page with `%npc_name%` and `%count%`.
    fn rift_show_html_count(&mut self, player: ObjectId, file: &str, npc: ObjectId, count: i32) {
        let name = self.creature_name(npc);
        if let Some(mut html) = self.html_from_file(player, npc.0, file) {
            html.html = html
                .html
                .replace("%npc_name%", &name)
                .replace("%count%", &count.to_string());
            self.send_html(player, &html);
        }
    }

    /// `handleCheat(player, npc)`.
    pub(crate) fn rift_handle_cheat(&mut self, player: ObjectId, npc: ObjectId) {
        self.rift_show_html_file(player, &format!("{RIFT_HTML}Cheater.htm"), npc);
        if !self.is_gm(player) {
            let name = self
                .players
                .get(&player)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            warn!("{name} was cheating in dimension rift area!");
            let punishment = self.default_punish();
            self.handle_illegal_player_action(
                player,
                &format!("Warning!! Character {name} tried to cheat in dimensional rift."),
                punishment,
            );
        }
    }

    /// `DimensionalRiftManager.start(player, type, npc)`: a party leader asks to enter.
    pub(crate) fn rift_start(&mut self, player: ObjectId, kind: i8, npc: ObjectId) {
        let Some(party) = self.party_of(player) else {
            self.rift_show_html_file(player, &format!("{RIFT_HTML}NoParty.htm"), npc);
            return;
        };
        if self.party_leader(party) != Some(player) {
            self.rift_show_html_file(player, &format!("{RIFT_HTML}NotPartyLeader.htm"), npc);
            return;
        }
        if self.seven_signs.rift.by_party.contains_key(&party) {
            self.rift_handle_cheat(player, npc);
            return;
        }
        let min = self.seven_signs.rift.settings.min_party_size;
        let members = self.party_members(party);
        if i32::try_from(members.len()).unwrap_or(i32::MAX) < min {
            self.rift_show_html_count(player, &format!("{RIFT_HTML}SmallParty.htm"), npc, min);
            return;
        }
        // Max parties inside is rooms count - 1.
        if !self.rift_is_allowed_enter(kind) {
            self.send_message(player, "Rift is full. Try later.");
            return;
        }
        let all_in_waiting_room = members.iter().all(|m| {
            self.position_of(*m)
                .is_some_and(|(x, y, z)| self.rift_check_in_peace_zone(x, y, z))
        });
        if !all_in_waiting_room {
            self.rift_show_html_file(player, &format!("{RIFT_HTML}NotInWaitingRoom.htm"), npc);
            return;
        }
        // NOTE(java-quirk): `getNeededItems` throws for a type outside 1..6; the bypass is cut.
        let Some(count) = self.rift_needed_items(kind) else {
            warn!("Rift: bad area type {kind}");
            return;
        };
        let has_fragments = members
            .iter()
            .all(|m| self.inventory_item_count(*m, DIMENSIONAL_FRAGMENT_ITEM_ID) >= i64::from(count.max(1)));
        if !has_fragments {
            self.rift_show_html_count(player, &format!("{RIFT_HTML}NoFragments.htm"), npc, count);
            return;
        }
        for member in &members {
            if !self.destroy_item_by_item_id(*member, DIMENSIONAL_FRAGMENT_ITEM_ID, i64::from(count), false) {
                // NOTE(java-quirk): the members before this one have paid already.
                self.rift_show_html_count(player, &format!("{RIFT_HTML}NoFragments.htm"), npc, count);
                return;
            }
        }
        // Find an empty room.
        let room = loop {
            let free = self.rift_free_rooms(kind);
            if free.is_empty() {
                return;
            }
            let pick = self.rng().get(1, i32::try_from(free.len()).unwrap_or(1)) - 1;
            let room = free[usize::try_from(pick).unwrap_or(0).min(free.len() - 1)];
            if !self.rift_room(kind, room).is_some_and(|r| r.party_inside) {
                break room;
            }
        };
        self.rift_new(party, kind, room);
    }

    // ------------------------------------------------------------------ the rift

    fn rift_timer(&mut self, delay_ms: i64, timer: RiftTimer) -> TimerId {
        let ms = u64::try_from(delay_ms).unwrap_or(0);
        let due = self.tick + TimerWheel::ms_to_ticks(ms);
        self.timers
            .schedule_at(due, TimerKind::SevenSigns(SevenSignsTimer::Rift(timer)))
    }

    /// `new DimensionalRift(party, type, room)`.
    fn rift_new(&mut self, party: PartyId, kind: i8, room: i8) -> Option<RiftId> {
        let coords = self.rift_room(kind, room)?.def;
        self.rift_room_mut(kind, room)?.party_inside = true;
        let id = RiftId(self.seven_signs.rift.next_id);
        self.seven_signs.rift.next_id += 1;
        self.seven_signs.rift.rifts.insert(
            id,
            Rift {
                kind,
                party: Some(party),
                completed_rooms: Vec::new(),
                current_jump: 1,
                chosen_room: room,
                dead: BTreeSet::new(),
                revived_in_waiting_room: BTreeSet::new(),
                boss_room: false,
                teleport_timer: None,
                spawn_timer: None,
                earthquake_timer: None,
            },
        );
        self.seven_signs.rift.by_party.insert(party, id);
        // The quest 635 is started for every member here in Java (a script, D-05).
        for member in self.party_members(party) {
            self.tele_to_location(
                member,
                TeleportTarget::at(coords.x_t, coords.y_t, coords.z_t).with_instance(0),
            );
        }
        self.rift_create_spawn_timer(id, room);
        self.rift_create_teleporter_timer(id, true);
        Some(id)
    }

    /// The rift of a party (`Party.getDimensionalRift()`).
    pub fn party_rift(&self, party: PartyId) -> Option<RiftId> {
        self.seven_signs.rift.by_party.get(&party).copied()
    }

    /// `Party.isInDimensionalRift()` of the party of a player.
    pub fn is_in_dimensional_rift(&self, player: ObjectId) -> bool {
        self.party_of(player)
            .is_some_and(|p| self.party_rift(p).is_some())
    }

    /// The type and the room of the rift of a player's party (`getType()`, `getCurrentRoom()`).
    pub fn rift_type_and_room(&self, player: ObjectId) -> Option<(i8, i8)> {
        let rift = self.party_rift(self.party_of(player)?)?;
        let r = self.seven_signs.rift.rifts.get(&rift)?;
        Some((r.kind, r.chosen_room))
    }

    /// `getRoom(type, room).checkIfInZone(x, y, z)` (the RiftInvader check of the AI): `false`
    /// where Java's lookup fails.
    pub fn rift_room_contains(&self, kind: i8, room: i8, x: i32, y: i32, z: i32) -> bool {
        self.rift_room(kind, room).is_some_and(|r| r.contains(x, y, z))
    }

    /// The "Dimensional Rift check" of `AttackableAI`: a `RiftInvader` does not go for a player
    /// whose party is in the rift while the invader stands outside the room of that rift.
    pub(crate) fn rift_invader_out_of_room(&self, invader: ObjectId, target: ObjectId) -> bool {
        if !self
            .npc_template(invader)
            .is_some_and(|t| t.npc_type == "RiftInvader")
        {
            return false;
        }
        let Some(player) = self.acting_player(target) else {
            return false;
        };
        let Some((kind, room)) = self.rift_type_and_room(player) else {
            return false;
        };
        let Some((x, y, z)) = self.position_of(invader) else {
            return false;
        };
        // `getRoom(riftType, riftRoom)` is null for a room not chosen yet; Java throws then.
        self.rift_room(kind, room).is_some_and(|r| !r.contains(x, y, z))
    }

    /// `calcTimeToNextJump()`: milliseconds.
    fn rift_calc_time_to_next_jump(&mut self, id: RiftId) -> i64 {
        let (min, max, multiply) = {
            let s = &self.seven_signs.rift.settings;
            (
                s.auto_jumps_time_min,
                s.auto_jumps_time_max,
                s.boss_room_time_multiply,
            )
        };
        let time = i64::from(self.rng().get(min, max)) * 1000;
        if self.seven_signs.rift.rifts.get(&id).is_some_and(|r| r.boss_room) {
            return (time as f32 * multiply) as i64;
        }
        time
    }

    /// `createTeleporterTimer(reasonTP)`.
    fn rift_create_teleporter_timer(&mut self, id: RiftId, reason_tp: bool) {
        let Some(rift) = self.seven_signs.rift.rifts.get_mut(&id) else {
            return;
        };
        if rift.party.is_none() {
            return;
        }
        let old = (rift.teleport_timer.take(), rift.earthquake_timer.take());
        for timer in [old.0, old.1].into_iter().flatten() {
            self.timers.cancel(timer);
        }
        if reason_tp {
            let jump_time = self.rift_calc_time_to_next_jump(id);
            let teleport = self.rift_timer(jump_time, RiftTimer::Teleport { rift: id, reason_tp });
            let quake = self.rift_timer(jump_time - 7000, RiftTimer::Earthquake(id));
            if let Some(rift) = self.seven_signs.rift.rifts.get_mut(&id) {
                rift.teleport_timer = Some(teleport);
                rift.earthquake_timer = Some(quake);
            }
        } else {
            // Incorrect party member invited.
            let teleport = self.rift_timer(FIVE_SECONDS, RiftTimer::Teleport { rift: id, reason_tp });
            if let Some(rift) = self.seven_signs.rift.rifts.get_mut(&id) {
                rift.teleport_timer = Some(teleport);
            }
        }
    }

    /// `createSpawnTimer(room)`.
    fn rift_create_spawn_timer(&mut self, id: RiftId, room: i8) {
        if let Some(old) = self
            .seven_signs
            .rift
            .rifts
            .get_mut(&id)
            .and_then(|r| r.spawn_timer.take())
        {
            self.timers.cancel(old);
        }
        let delay = i64::from(self.seven_signs.rift.settings.spawn_delay);
        let timer = self.rift_timer(delay, RiftTimer::Spawn { rift: id, room });
        if let Some(rift) = self.seven_signs.rift.rifts.get_mut(&id) {
            rift.spawn_timer = Some(timer);
        }
    }

    /// The rift timers.
    pub(crate) fn on_rift_timer(&mut self, timer: RiftTimer) {
        match timer {
            RiftTimer::Teleport { rift, reason_tp } => {
                if let Some(r) = self.seven_signs.rift.rifts.get_mut(&rift) {
                    r.teleport_timer = None;
                }
                self.rift_teleporter_task(rift, reason_tp);
            }
            RiftTimer::Spawn { rift, room } => {
                let kind = match self.seven_signs.rift.rifts.get_mut(&rift) {
                    Some(r) => {
                        r.spawn_timer = None;
                        r.kind
                    }
                    None => return,
                };
                self.rift_room_spawn(kind, room);
            }
            RiftTimer::Earthquake(rift) => {
                let Some(r) = self.seven_signs.rift.rifts.get_mut(&rift) else {
                    return;
                };
                r.earthquake_timer = None;
                let (party, revived) = (r.party, r.revived_in_waiting_room.clone());
                let Some(party) = party else {
                    return;
                };
                for member in self.party_members(party) {
                    if revived.contains(&member) {
                        continue;
                    }
                    if let Some((x, y, z)) = self.position_of(member) {
                        self.send(
                            member,
                            &Earthquake {
                                x,
                                y,
                                z,
                                intensity: 65,
                                duration: 9,
                            },
                        );
                    }
                }
            }
        }
    }

    /// The body of `teleporterTimerTask.run()`.
    fn rift_teleporter_task(&mut self, id: RiftId, reason_tp: bool) {
        let Some(r) = self.seven_signs.rift.rifts.get(&id) else {
            return;
        };
        let (kind, chosen, party) = (r.kind, r.chosen_room, r.party);
        let Some(party) = party else {
            return;
        };
        if chosen > -1 {
            self.rift_room_unspawn(kind, chosen);
            if let Some(room) = self.rift_room_mut(kind, chosen) {
                room.party_inside = false;
            }
        }
        let members = self.party_members(party);
        let (current_jump, dead, revived) = {
            let r = &self.seven_signs.rift.rifts[&id];
            (r.current_jump, r.dead.len(), r.revived_in_waiting_room.clone())
        };
        let max_jumps = self.rift_max_jumps();
        // Check if there are still available jumps.
        if reason_tp && current_jump < max_jumps && members.len() > dead {
            if let Some(r) = self.seven_signs.rift.rifts.get_mut(&id) {
                r.current_jump += 1;
                r.completed_rooms.push(chosen);
                r.chosen_room = -1;
            }
            for member in &members {
                if !revived.contains(member) {
                    self.rift_teleport_to_next_room(id, *member);
                }
            }
            self.rift_create_teleporter_timer(id, true);
            let next = self.seven_signs.rift.rifts.get(&id).map_or(-1, |r| r.chosen_room);
            self.rift_create_spawn_timer(id, next);
        } else {
            for member in &members {
                if !revived.contains(member) {
                    self.rift_teleport_to_waiting_room(*member);
                }
            }
            self.rift_kill(id);
        }
    }

    /// `getMaxJumps()`: `MaxRiftJumps` when it is 1 to 8, else 4.
    pub fn rift_max_jumps(&self) -> i32 {
        let jumps = self.seven_signs.rift.settings.max_jumps;
        if (1..=8).contains(&jumps) { jumps } else { 4 }
    }

    /// `teleportToNextRoom(player)`: picks the next room for the party (once), then moves the
    /// player there.
    fn rift_teleport_to_next_room(&mut self, id: RiftId, player: ObjectId) {
        let Some(r) = self.seven_signs.rift.rifts.get(&id) else {
            return;
        };
        let (kind, mut chosen) = (r.kind, r.chosen_room);
        if chosen == -1 {
            loop {
                let mut empty = self.rift_free_rooms(kind);
                // Do not tp in the same room a second time.
                if let Some(r) = self.seven_signs.rift.rifts.get(&id) {
                    empty.retain(|room| !r.completed_rooms.contains(room));
                }
                // If no room left, find any empty.
                if empty.is_empty() {
                    empty = self.rift_free_rooms(kind);
                }
                if empty.is_empty() {
                    return;
                }
                let pick = self.rng().get(1, i32::try_from(empty.len()).unwrap_or(1)) - 1;
                chosen = empty[usize::try_from(pick).unwrap_or(0).min(empty.len() - 1)];
                if !self.rift_room(kind, chosen).is_some_and(|r| r.party_inside) {
                    break;
                }
            }
            if let Some(r) = self.seven_signs.rift.rifts.get_mut(&id) {
                r.chosen_room = chosen;
            }
        }
        let boss = self.rift_room(kind, chosen).map(|r| (r.def, r.def.boss));
        if let Some(room) = self.rift_room_mut(kind, chosen) {
            room.party_inside = true;
        }
        if let Some((def, boss)) = boss {
            // `checkBossRoom(room)`.
            if let Some(r) = self.seven_signs.rift.rifts.get_mut(&id) {
                r.boss_room = boss;
            }
            self.tele_to_location(
                player,
                TeleportTarget::at(def.x_t, def.y_t, def.z_t).with_instance(0),
            );
        }
    }

    /// `killRift()` and `DimensionalRiftManager.killRift(d)`: the timers stop, the room is empty
    /// again, the party leaves the rift.
    pub(crate) fn rift_kill(&mut self, id: RiftId) {
        let Some(mut r) = self.seven_signs.rift.rifts.remove(&id) else {
            return;
        };
        if let Some(party) = r.party.take() {
            self.seven_signs.rift.by_party.remove(&party);
        }
        for timer in [
            r.teleport_timer.take(),
            r.spawn_timer.take(),
            r.earthquake_timer.take(),
        ]
        .into_iter()
        .flatten()
        {
            self.timers.cancel(timer);
        }
        if r.chosen_room > -1 {
            self.rift_room_unspawn(r.kind, r.chosen_room);
            if let Some(room) = self.rift_room_mut(r.kind, r.chosen_room) {
                room.party_inside = false;
            }
        }
    }

    /// `partyMemberInvited()`: somebody joined the party in the rift; in five seconds everybody
    /// is out.
    pub(crate) fn rift_party_member_invited(&mut self, party: PartyId) {
        if let Some(id) = self.party_rift(party) {
            self.rift_create_teleporter_timer(id, false);
        }
    }

    /// `partyMemberExited(player)`: with too few members left the rift ends.
    pub(crate) fn rift_party_member_exited(&mut self, party: PartyId, player: ObjectId) {
        let Some(id) = self.party_rift(party) else {
            return;
        };
        if let Some(r) = self.seven_signs.rift.rifts.get_mut(&id) {
            r.dead.remove(&player);
            r.revived_in_waiting_room.remove(&player);
        }
        let members = self.party_members(party);
        let min = self.seven_signs.rift.settings.min_party_size;
        let count = i32::try_from(members.len()).unwrap_or(i32::MAX);
        if count < min || count == 1 {
            for member in members {
                self.rift_teleport_to_waiting_room(member);
            }
            self.rift_kill(id);
        }
    }

    /// `manualTeleport(player, npc)`: the leader uses the chance card for the next room.
    pub(crate) fn rift_manual_teleport(&mut self, player: ObjectId, npc: ObjectId) {
        let Some(party) = self.party_of(player) else {
            return;
        };
        let Some(id) = self.party_rift(party) else {
            return;
        };
        if self.party_leader(party) != Some(player) {
            self.rift_show_html_file(player, &format!("{RIFT_HTML}NotPartyLeader.htm"), npc);
            return;
        }
        // Check if the maximum jump limit has been reached.
        let max_jumps = self.rift_max_jumps();
        let Some(r) = self.seven_signs.rift.rifts.get(&id) else {
            return;
        };
        if r.current_jump >= max_jumps {
            self.rift_show_html_file(player, &format!("{RIFT_HTML}AlreadyTeleported.htm"), npc);
            return;
        }
        let (kind, chosen) = (r.kind, r.chosen_room);
        if let Some(r) = self.seven_signs.rift.rifts.get_mut(&id) {
            // Increase the jump counter.
            r.current_jump += 1;
        }
        self.rift_room_unspawn(kind, chosen);
        if let Some(room) = self.rift_room_mut(kind, chosen) {
            room.party_inside = false;
        }
        if let Some(r) = self.seven_signs.rift.rifts.get_mut(&id) {
            r.completed_rooms.push(chosen);
            r.chosen_room = -1;
        }
        for member in self.party_members(party) {
            self.rift_teleport_to_next_room(id, member);
        }
        let next = self.seven_signs.rift.rifts.get(&id).map_or(-1, |r| r.chosen_room);
        if let Some(room) = self.rift_room_mut(kind, next) {
            room.party_inside = true;
        }
        self.rift_create_spawn_timer(id, next);
        self.rift_create_teleporter_timer(id, true);
    }

    /// `manualExitRift(player, npc)`: the leader takes the party out.
    pub(crate) fn rift_manual_exit(&mut self, player: ObjectId, npc: ObjectId) {
        let Some(party) = self.party_of(player) else {
            return;
        };
        let Some(id) = self.party_rift(party) else {
            return;
        };
        if self.party_leader(party) != Some(player) {
            self.rift_show_html_file(player, &format!("{RIFT_HTML}NotPartyLeader.htm"), npc);
            return;
        }
        for member in self.party_members(party) {
            self.rift_teleport_to_waiting_room(member);
        }
        self.rift_kill(id);
    }

    /// `memberDead(player)` (`Player.doDie`).
    pub(crate) fn rift_member_dead(&mut self, player: ObjectId) {
        if let Some(id) = self.party_of(player).and_then(|p| self.party_rift(p))
            && let Some(r) = self.seven_signs.rift.rifts.get_mut(&id)
        {
            r.dead.insert(player);
        }
    }

    /// `memberRessurected(player)` (`Player.onTeleported` outside the waiting room).
    pub(crate) fn rift_member_resurrected(&mut self, player: ObjectId) {
        let Some(party) = self.party_of(player) else {
            return;
        };
        let Some(id) = self.party_rift(party) else {
            return;
        };
        let Some((x, y, z)) = self.position_of(player) else {
            return;
        };
        if self.rift_check_in_peace_zone(x, y, z) {
            return;
        }
        if let Some(r) = self.seven_signs.rift.rifts.get_mut(&id) {
            r.dead.remove(&player);
        }
    }

    /// `usedTeleport(player)`: a dead member took the way out; with too few left the rift ends.
    pub fn rift_used_teleport(&mut self, player: ObjectId) {
        let Some(party) = self.party_of(player) else {
            return;
        };
        let Some(id) = self.party_rift(party) else {
            return;
        };
        let Some(r) = self.seven_signs.rift.rifts.get_mut(&id) else {
            return;
        };
        r.revived_in_waiting_room.insert(player);
        r.dead.insert(player);
        let revived = r.revived_in_waiting_room.clone();
        let members = self.party_members(party);
        let min = self.seven_signs.rift.settings.min_party_size;
        let present = i32::try_from(members.len().saturating_sub(revived.len())).unwrap_or(0);
        if present < min {
            for member in members {
                if !revived.contains(&member) {
                    self.rift_teleport_to_waiting_room(member);
                }
            }
            self.rift_kill(id);
        }
    }

    /// Whether `player` is in `getRevivedAtWaitingRoom()` of his party's rift.
    pub fn rift_revived_at_waiting_room(&self, player: ObjectId) -> bool {
        self.party_of(player)
            .and_then(|p| self.party_rift(p))
            .and_then(|id| self.seven_signs.rift.rifts.get(&id))
            .is_some_and(|r| r.revived_in_waiting_room.contains(&player))
    }

    /// `getDeadMemberList()` (tests).
    pub fn rift_dead_members(&self, party: PartyId) -> Vec<ObjectId> {
        self.party_rift(party)
            .and_then(|id| self.seven_signs.rift.rifts.get(&id))
            .map(|r| r.dead.iter().copied().collect())
            .unwrap_or_default()
    }

    /// The current jump of a party's rift (tests).
    pub fn rift_current_jump(&self, party: PartyId) -> Option<i32> {
        self.party_rift(party)
            .and_then(|id| self.seven_signs.rift.rifts.get(&id))
            .map(|r| r.current_jump)
    }

    /// The room a party's rift is in now (tests).
    pub fn rift_current_room(&self, party: PartyId) -> Option<i8> {
        self.party_rift(party)
            .and_then(|id| self.seven_signs.rift.rifts.get(&id))
            .map(|r| r.chosen_room)
    }

    /// `Rift.java`: the commands `EnterRift`, `ChangeRiftRoom` and `ExitRift`.
    pub(crate) fn rift_bypass(&mut self, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
        let Some(npc) = target.filter(|t| self.npcs.contains_key(t)) else {
            return false;
        };
        let lower = command.to_lowercase();
        if lower.starts_with("enterrift") {
            // `Byte.parseByte(command.substring(10))`.
            return match command.get(10..).and_then(|v| v.parse::<i8>().ok()) {
                Some(kind) => {
                    self.rift_start(player, kind, npc);
                    true
                }
                None => {
                    warn!("Exception in Rift: bad command {command}");
                    false
                }
            };
        }
        let in_rift = self.is_in_dimensional_rift(player);
        if lower.starts_with("changeriftroom") {
            if in_rift {
                self.rift_manual_teleport(player, npc);
            } else {
                self.rift_handle_cheat(player, npc);
            }
            return true;
        } else if lower.starts_with("exitrift") {
            if in_rift {
                self.rift_manual_exit(player, npc);
            } else {
                self.rift_handle_cheat(player, npc);
            }
        }
        true
    }
}

/// The handler of `bypass EnterRift ...`, `ChangeRiftRoom` and `ExitRift`.
pub fn rift_bypass_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    target: Option<ObjectId>,
) -> bool {
    world.rift_bypass(player, command, target)
}
