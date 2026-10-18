//! The world: all game state, changed only by [`World::handle`] and [`World::tick`].

use std::collections::HashMap;
use std::net::IpAddr;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use l2_net::Encode;
use l2_protocol_game::server::NetPing;
use l2_protocol_game::server::enterworld::SkillCoolTime;
use l2_util::{ObjectId, WorldRng};
use tracing::{debug, info, warn};

use crate::command::{AdminQuery, ClientInput, LeaveReason, SessionId, WorldCommand};
use crate::data::{NoData, StaticData, WorldSettings};
use crate::objects::{Door, Npc, ObjectEntry, ObjectKind};
use crate::outbox::{LsEvent, Outbox, PacketSink, PersistCommand};
use crate::player::{OnlinePlayer, Player, PlayerData};
use crate::region::RegionGrid;
use crate::snapshot::snapshot_for_store;
use crate::timer::{TimerId, TimerKind, TimerWheel};

/// One world tick (D-12).
pub const TICK: Duration = Duration::from_millis(50);

/// Java `GeneralConfig.CHAR_DATA_STORE_INTERVAL` default: `CharacterDataStoreInterval = 15` minutes.
pub const DEFAULT_AUTOSAVE_INTERVAL: Duration = Duration::from_secs(15 * 60);

pub struct World {
    pub(crate) tick: u64,
    rng: WorldRng,
    pub(crate) timers: TimerWheel,
    pub(crate) players: HashMap<ObjectId, Player>,
    pub(crate) sessions: HashMap<SessionId, ObjectId>,
    /// The session of each player (the reverse of `sessions`).
    pub(crate) player_sessions: HashMap<ObjectId, SessionId>,
    pub(crate) sinks: HashMap<ObjectId, Box<dyn PacketSink>>,
    pub(crate) outbox: Box<dyn Outbox>,
    stopped: bool,
    /// Ticks between two autosaves of a player (Java `CHAR_DATA_STORE_INTERVAL`).
    autosave_ticks: u64,
    /// The pending autosave timer of each online player.
    pub(crate) autosave_timers: HashMap<ObjectId, TimerId>,
    /// Item templates, skills, class templates (see [`StaticData`]).
    pub(crate) data: Box<dyn StaticData>,
    /// The configuration values the world reads.
    pub(crate) settings: WorldSettings,
    /// Every object in the world by id: its kind and grid state (Java `World._allObjects`).
    pub(crate) objects: HashMap<ObjectId, ObjectEntry>,
    /// The region grid (Java `World._worldRegions`).
    pub(crate) regions: RegionGrid,
    pub(crate) npcs: HashMap<ObjectId, Npc>,
    pub(crate) doors: HashMap<ObjectId, Door>,
    /// Zone table, map regions and geoengine (see `zones.rs`, `teleport.rs`).
    pub(crate) zoning: crate::zones::ZoneContext,
}

impl World {
    pub fn new(rng: WorldRng, outbox: Box<dyn Outbox>) -> Self {
        Self {
            tick: 0,
            rng,
            timers: TimerWheel::default(),
            players: HashMap::new(),
            sessions: HashMap::new(),
            player_sessions: HashMap::new(),
            sinks: HashMap::new(),
            outbox,
            stopped: false,
            autosave_ticks: TimerWheel::duration_to_ticks(DEFAULT_AUTOSAVE_INTERVAL),
            autosave_timers: HashMap::new(),
            data: Box::new(NoData),
            settings: WorldSettings::default(),
            objects: HashMap::new(),
            regions: RegionGrid::new(false),
            npcs: HashMap::new(),
            doors: HashMap::new(),
            zoning: crate::zones::ZoneContext::default(),
        }
    }

    /// Installs the datapack view the packets are built from. Call before players enter.
    pub fn set_static_data(&mut self, data: Box<dyn StaticData>) {
        self.data = data;
    }

    /// Installs the configuration values the world reads. Call before players enter.
    pub fn set_settings(&mut self, settings: WorldSettings) {
        // Java: `WorldRegion._active` starts as `GRIDS_ALWAYS_ON`. Settings are installed before
        // anything spawns, so the grid is still empty here.
        if self.regions.len() == 0 {
            self.regions = RegionGrid::new(settings.grids_always_on);
        }
        self.settings = settings;
    }

    /// Sets the time between two autosaves of a player; applies from the next (re)schedule.
    // Java: PlayerAutoSaveTaskManager polls once a second, so shorter intervals are not honoured
    // there either; callers clamp to one second.
    pub fn set_autosave_interval(&mut self, interval: Duration) {
        self.autosave_ticks = TimerWheel::duration_to_ticks(interval);
    }

    /// Ticks since the world started.
    pub fn tick_count(&self) -> u64 {
        self.tick
    }

    pub fn rng(&mut self) -> &mut WorldRng {
        &mut self.rng
    }

    pub fn timers(&mut self) -> &mut TimerWheel {
        &mut self.timers
    }

    pub fn player(&self, id: ObjectId) -> Option<&Player> {
        self.players.get(&id)
    }

    pub fn player_mut(&mut self, id: ObjectId) -> Option<&mut Player> {
        self.players.get_mut(&id)
    }

    pub fn player_of_session(&self, session: SessionId) -> Option<ObjectId> {
        self.sessions.get(&session).copied()
    }

    pub fn players(&self) -> impl Iterator<Item = &Player> {
        self.players.values()
    }

    pub fn online_count(&self) -> usize {
        self.players.len()
    }

    /// True after `Shutdown`: the runner stops.
    pub fn is_stopped(&self) -> bool {
        self.stopped
    }

    /// Encodes and queues a packet for one player. `false` if the player isn't online.
    pub fn send(&self, to: ObjectId, packet: &impl Encode) -> bool {
        self.send_bytes(to, packet.to_bytes())
    }

    pub fn send_bytes(&self, to: ObjectId, bytes: Vec<u8>) -> bool {
        match self.sinks.get(&to) {
            Some(sink) => {
                sink.send(bytes);
                true
            }
            None => false,
        }
    }

    /// Applies one command.
    pub fn handle(&mut self, command: WorldCommand) {
        if self.stopped {
            debug!("world stopped, dropping {command:?}");
            return;
        }
        match command {
            WorldCommand::EnterWorld {
                session,
                player,
                sink,
                ip,
            } => self.enter(session, *player, sink, ip),
            WorldCommand::Input { session, input } => self.on_input(session, input),
            WorldCommand::Disconnected { session } => {
                if let Some(id) = self.sessions.get(&session).copied() {
                    self.leave(id, LeaveReason::Disconnected);
                }
            }
            WorldCommand::Kick { account } => self.kick_account(&account),
            WorldCommand::Admin(query) => self.on_admin(query),
            WorldCommand::Shutdown { done } => {
                let online: Vec<ObjectId> = self.players.keys().copied().collect();
                info!("World shutting down, storing {} player(s).", online.len());
                for id in online {
                    self.leave(id, LeaveReason::ServerShutdown);
                }
                self.stopped = true;
                done();
            }
        }
    }

    /// Advances one tick: fires due timers. Movement, AI and regeneration join in later
    /// milestones.
    pub fn tick(&mut self) {
        self.tick += 1;
        for (_, kind) in self.timers.take_due(self.tick) {
            self.on_timer(kind);
        }
    }

    // Java: EnterWorld.runImpl (the world part; the packets are in `enter.rs`)
    fn enter(&mut self, session: SessionId, data: PlayerData, sink: Box<dyn PacketSink>, ip: Option<IpAddr>) {
        if self.sessions.contains_key(&session) {
            warn!("{session:?} tried to enter the world twice; ignored.");
            return;
        }
        let id = data.object_id;
        if self.players.contains_key(&id) {
            // Java: the old instance is disconnected (e.g. a stale session after a crash).
            warn!(
                "Character {} is already in the world; removing the old instance.",
                data.name
            );
            self.leave(id, LeaveReason::Kicked);
        }
        let account = data.account.clone();
        info!("{} ({account}) entered the world.", data.name);
        let mut player = Player::from_data(data, self.tick);
        player.stats = self.data.derive_stats(&player.data);
        player.entered_at_unix_ms = unix_ms_now();
        player.client_ip = ip;
        self.players.insert(id, player);
        // Spawned in `on_entered`, at the point of Java's EnterWorld where `spawnMe` runs.
        self.register_object(id, ObjectKind::Player);
        self.sessions.insert(session, id);
        self.player_sessions.insert(id, session);
        self.sinks.insert(id, sink);
        // Java: Player.load -> PlayerAutoSaveTaskManager.add
        self.schedule_autosave(id);
        self.outbox.ls_event(LsEvent::PlayerInGame { account });
        self.on_entered(id);
    }

    /// Arms the next autosave of `id`, replacing a pending one so there is only ever one.
    fn schedule_autosave(&mut self, id: ObjectId) {
        let due = self.tick + self.autosave_ticks;
        let timer = self.timers.schedule_at(due, TimerKind::Autosave(id));
        if let Some(old) = self.autosave_timers.insert(id, timer) {
            self.timers.cancel(old);
        }
    }

    fn on_input(&mut self, session: SessionId, input: ClientInput) {
        let Some(id) = self.sessions.get(&session).copied() else {
            debug!("{session:?} sent {input:?} without a character in the world");
            return;
        };
        match input {
            ClientInput::Logout => self.on_logout(id),
            ClientInput::Restart => self.on_restart(id),
            ClientInput::Say {
                chat_type,
                text,
                target,
            } => self.on_say(id, chat_type, text, target),
            // Java: RequestNetPing -> NetPing(GameTimeTaskManager.getGameTime())
            ClientInput::NetPing => {
                let game_time = game_time_minutes(unix_ms_now());
                self.send(id, &NetPing { game_time });
            }
            // Java: RequestSkillCoolTime -> SkillCoolTime(player). The reuse timers are not
            // modelled yet, so the list is empty.
            ClientInput::SkillCoolTime => {
                self.send(id, &SkillCoolTime::default());
            }
            // Java: RequestKeyMapping -> ExUISetting(player) when StoreCharUiSettings is on.
            ClientInput::KeyMapping => {
                if let Some(player) = self.players.get(&id)
                    && let Some(packet) = crate::enter::ui_setting(&player.data, &self.settings)
                {
                    self.send(id, &packet);
                }
            }
            // Java: Appearing -> onTeleported (when teleporting), then updateUserInfo
            ClientInput::Appearing => self.on_appearing(id),
            ClientInput::Other { opcode, ex_opcode } => {
                debug!("unhandled in-game packet 0x{opcode:02X} {ex_opcode:?} from {session:?}");
            }
        }
    }

    fn on_timer(&mut self, kind: TimerKind) {
        match kind {
            // Java: PlayerAutoSaveTaskManager.run -> Player.autoSave, then due again after
            // CHAR_DATA_STORE_INTERVAL. A player who left has no timer any more.
            TimerKind::Autosave(id) => {
                if let Some(player) = self.players.get(&id) {
                    let snapshot = snapshot_for_store(player, self.tick);
                    self.outbox
                        .persist(PersistCommand::StorePlayer(Box::new(snapshot)));
                    self.schedule_autosave(id);
                }
            }
            TimerKind::Marker(n) => debug!("timer marker {n}"),
            TimerKind::RegionNeighborsOn(_) | TimerKind::RegionNeighborsOff(_) => self.on_region_timer(kind),
            TimerKind::TeleportWatchdog(id) => self.on_teleport_watchdog(id),
        }
    }

    fn on_admin(&mut self, query: AdminQuery) {
        match query {
            AdminQuery::OnlinePlayers(reply) => {
                let tick_ms = TICK.as_millis() as u64;
                let mut list: Vec<OnlinePlayer> = self
                    .players
                    .values()
                    .map(|p| OnlinePlayer {
                        object_id: p.core.object_id,
                        account: p.data.account.clone(),
                        name: p.data.name.clone(),
                        level: p.core.level,
                        class_id: p.data.class_id,
                        online_secs: self.tick.saturating_sub(p.entered_at_tick) * tick_ms / 1000,
                        online_since_unix_ms: p.entered_at_unix_ms,
                        ip: p.client_ip,
                    })
                    .collect();
                list.sort_by(|a, b| a.name.cmp(&b.name));
                reply(list);
            }
        }
    }
}

/// Unix time in milliseconds. A clock before 1970 reads as 0.
pub(crate) fn unix_ms_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX))
}

/// Java `GameTimeTaskManager.getGameTime()`: the in-game minute of the day, 0 to 1439.
///
/// One game minute lasts ten real seconds, so a game day is four real hours.
// NOTE: Java counts from local midnight; this counts from UTC midnight (std has no time zone).
pub fn game_time_minutes(now_ms: i64) -> i32 {
    const TICKS_PER_IG_DAY: i64 = 144_000;
    let ticks = now_ms.rem_euclid(86_400_000) / 100;
    i32::try_from(((ticks % TICKS_PER_IG_DAY) / 100) % (24 * 60)).unwrap_or(0)
}
