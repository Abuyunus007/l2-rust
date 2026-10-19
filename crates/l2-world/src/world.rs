//! The world: all game state, changed only by [`World::handle`] and [`World::tick`].

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use l2_net::Encode;
use l2_protocol_game::server::NetPing;
use l2_protocol_game::server::enterworld::SkillCoolTime;
use l2_util::{ObjectId, WorldRng};
use tracing::{debug, info, warn};

use crate::command::{AdminQuery, ClientInput, LeaveReason, SessionId, WorldCommand};
use crate::data::{NoData, StaticData, WorldSettings};
use crate::door::{Door, DoorIndex, DoorSnapshot, Fence, StaticObject};
use crate::objects::{Npc, ObjectEntry, ObjectKind};
use crate::outbox::{LsEvent, Outbox, PacketSink, PersistCommand};
use crate::player::{OnlinePlayer, Player, PlayerData};
use crate::region::RegionGrid;
use crate::snapshot::snapshot_for_store;
use crate::status::{CreatureStatus, RegenSettings, STATUS_SWEEP_TICKS};
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
    /// Spawn lists, NPC templates and settings (M3-NPC-01).
    pub(crate) npc_world: crate::npc::NpcWorld,
    /// Html pages, gatekeeper lists and bypass handlers (M3-TALK-01).
    pub(crate) talk: crate::dialog::TalkWorld,
    /// The admin command handlers (M3-ADMIN-01).
    pub(crate) admin: crate::admin::AdminWorld,
    pub(crate) doors: HashMap<ObjectId, Door>,
    /// Java `GameTimeTaskManager._isNight`; `None` while the game clock is off.
    pub(crate) game_clock_night: Option<bool>,
    /// The loaded geodata (M3-GEO-03); `None` in tests that need none. Immutable, shared with
    /// the path workers (D-14).
    pub(crate) geo: Option<Arc<l2_geo::GeoEngine>>,
    /// Where path searches go (D-14); installed by the game server after the pool started.
    pub(crate) path_requests: Option<Box<dyn crate::movement::PathRequests>>,
    pub(crate) next_path_request_id: u64,
    pub(crate) warned_no_path_service: bool,
    /// Creatures with a running move (Java `MovementTaskManager`'s pools).
    pub(crate) moving: Vec<ObjectId>,
    /// The ids in `moving`, for the membership test.
    pub(crate) moving_set: std::collections::HashSet<ObjectId>,
    /// Zone table, map regions and geoengine (see `zones.rs`, `teleport.rs`).
    pub(crate) zoning: crate::zones::ZoneContext,
    pub(crate) fences: HashMap<ObjectId, Fence>,
    pub(crate) static_objects: HashMap<ObjectId, StaticObject>,
    /// `DoorData`'s id and group lookups.
    pub(crate) door_index: DoorIndex,
    /// The door view handed to path workers; `None` after a door or fence changed.
    pub(crate) door_snapshot: Option<Arc<DoorSnapshot>>,
    /// Java `NPC_STD_CALCULATOR`: the calculators every NPC shares until it gets its own.
    pub(crate) std_npc_calculators: crate::stats::npc_stat::StdNpcCalculators,
    /// Attack timing, death, aggro and the combat stances (M4-COMBAT-01, `combat/`).
    pub(crate) combat: crate::combat::CombatWorld,
    /// The AI state of every creature that has one (M4-COMBAT-01, `ai/`).
    pub(crate) ai: crate::ai::AiWorld,
    /// HP, MP and CP regeneration and the status listeners, per creature (`status.rs`).
    pub(crate) statuses: HashMap<ObjectId, CreatureStatus>,
    /// The multipliers the regeneration formulas read (`status.rs`).
    pub(crate) regen: RegenSettings,
    /// Spawn counters, corpses waiting to decay, NPCs waiting to respawn, pending revives
    /// (M4-DEATH-01, `death.rs`, `respawn.rs`).
    pub(crate) death: crate::death::DeathWorld,
    /// Items on the ground, the auto destroy list and the `itemsonground` timers (M4-DROP-01).
    pub(crate) ground: crate::ground_item::GroundWorld,
    /// The experience tables and skill trees exp and levels read (M4-EXP-01, `rewards/exp.rs`).
    pub(crate) rewards: crate::rewards::exp::ExpWorld,
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
            npc_world: crate::npc::NpcWorld::default(),
            talk: crate::dialog::TalkWorld::default(),
            admin: crate::admin::AdminWorld::default(),
            doors: HashMap::new(),
            game_clock_night: None,
            geo: None,
            path_requests: None,
            next_path_request_id: 0,
            warned_no_path_service: false,
            moving: Vec::new(),
            moving_set: std::collections::HashSet::new(),
            zoning: crate::zones::ZoneContext::default(),
            fences: HashMap::new(),
            static_objects: HashMap::new(),
            door_index: DoorIndex::default(),
            door_snapshot: None,
            std_npc_calculators: crate::stats::npc_stat::StdNpcCalculators::default(),
            combat: crate::combat::CombatWorld::default(),
            ai: crate::ai::AiWorld::default(),
            statuses: HashMap::new(),
            regen: RegenSettings::default(),
            death: crate::death::DeathWorld::default(),
            ground: crate::ground_item::GroundWorld::default(),
            rewards: crate::rewards::exp::ExpWorld::default(),
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

    /// Hands the world the geodata. Call before players enter.
    pub fn set_geo(&mut self, geo: Arc<l2_geo::GeoEngine>) {
        // One engine for everything: movement, spawns and teleports (M3-TELE-01 keeps its copy in
        // the zone context).
        self.zoning.geo = Some(Arc::clone(&geo));
        self.geo = Some(geo);
    }

    /// The geodata, if the server loaded any engine at all (it can hold zero regions).
    pub fn geo(&self) -> Option<&Arc<l2_geo::GeoEngine>> {
        self.geo.as_ref()
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

    /// Runs `f` with a read-only world and the world RNG at once (the formulas read the stat
    /// engine through `&World` while they draw). The RNG is moved out for the call and put back,
    /// so the sequence of draws is the same as drawing from `self.rng` directly.
    pub(crate) fn with_rng<R>(&mut self, f: impl FnOnce(&World, &mut WorldRng) -> R) -> R {
        let mut rng = std::mem::replace(&mut self.rng, WorldRng::from_seed(0));
        let result = f(self, &mut rng);
        self.rng = rng;
        result
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
            WorldCommand::PathFound {
                requester,
                request_id,
                result,
            } => self.on_path_found(requester, request_id, result),
            WorldCommand::SetPathRequests(requests) => self.set_path_requests(requests),
            WorldCommand::Shutdown { done } => {
                let online: Vec<ObjectId> = self.players.keys().copied().collect();
                info!("World shutting down, storing {} player(s).", online.len());
                for id in online {
                    self.leave(id, LeaveReason::ServerShutdown);
                }
                // Java Shutdown: "Save items on ground before closing."
                self.save_ground_items();
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
        if self.tick % STATUS_SWEEP_TICKS == 0 {
            self.sweep_statuses();
        }
        // Java: MovementTaskManager.
        self.update_movement();
        // Java: AttackableThinkTaskManager.
        self.think_attackables();
        // Java: GameTimeTaskManager checks every 100 ms (two world ticks).
        if self.tick % 2 == 0 {
            self.update_game_clock();
        }
    }

    /// Java `GameTimeTaskManager`: from now on the world follows the game clock and switches the
    /// day and night spawns when the game hour crosses 6 or 0. Off until called, so tests stay
    /// independent of the wall clock.
    pub fn start_game_clock(&mut self) {
        self.game_clock_night = Some(Self::is_night_now());
    }

    fn update_game_clock(&mut self) {
        let Some(was_night) = self.game_clock_night else {
            return;
        };
        let night = Self::is_night_now();
        if night != was_night {
            self.game_clock_night = Some(night);
            info!("Game clock: it is now {}.", if night { "night" } else { "day" });
            // Java also adds or removes the Dark Elves' Shadow Sense skill here (with a system
            // message): skills and their effects are M5.
            self.set_night(night);
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
        // A new Java object starts with no regeneration task and no listeners.
        self.forget_status(id);
        let account = data.account.clone();
        info!("{} ({account}) entered the world.", data.name);
        let mut player = Player::from_data(data, self.tick);
        player.entered_at_unix_ms = unix_ms_now();
        player.client_ip = ip;
        self.players.insert(id, player);
        // Java `Player` constructor + `restore`: the standard functions, the equipped items'
        // functions, then the current HP/MP/CP clamped to the maxima.
        self.init_player_stats(id);
        // Java `Player.restore`'s setCurrentCp/Hp/Mp: starts the regeneration of what is not full.
        self.restore_player_vitals(id);
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
            input @ (ClientInput::MoveTo { .. }
            | ClientInput::ValidatePosition { .. }
            | ClientInput::CannotMoveAnymore { .. }) => self.on_movement_input(id, input),
            // Java: Appearing -> onTeleported (when teleporting), then updateUserInfo
            ClientInput::Appearing => self.on_appearing(id),
            // Java: Action, RequestTargetCanceld, RequestBypassToServer, RequestLinkHtml
            ClientInput::Action { object_id, action_id } => self.on_action(id, object_id, action_id),
            ClientInput::TargetCanceled { unselect } => self.on_target_canceled(id, unselect),
            ClientInput::Bypass { command } => self.on_bypass_to_server(id, &command),
            ClientInput::LinkHtml { link } => self.on_link_html(id, &link),
            ClientInput::BuildCmd { command } => self.on_build_cmd(id, &command),
            ClientInput::UserCommand { id: command } => self.on_user_command(id, command),
            // Java: AttackRequest (the origin and the shift flag are never used)
            ClientInput::AttackRequest { object_id, .. } => self.on_attack_request(id, object_id),
            // Java: UseItem, RequestUnEquipItem, RequestDestroyItem, RequestDropItem, RequestItemList
            ClientInput::UseItem {
                object_id,
                ctrl_pressed,
            } => self.on_use_item(id, object_id, ctrl_pressed),
            ClientInput::UnEquipItem { paperdoll_slot } => self.on_request_unequip_item(id, paperdoll_slot),
            ClientInput::DestroyItem { object_id, count } => {
                self.on_request_destroy_item(id, object_id, count)
            }
            ClientInput::DropItem {
                object_id,
                count,
                x,
                y,
                z,
            } => self.on_request_drop_item(id, object_id, count, x, y, z),
            ClientInput::ItemList => self.on_request_item_list(id),
            // Java: RequestRestartPoint
            ClientInput::RestartPoint { point_type } => self.on_request_restart_point(id, point_type),
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
            TimerKind::InventoryUpdate(id) => self.on_inventory_update_timer(id),
            TimerKind::Follow(id) => self.on_follow_timer(id),
            // Java: HitTask -> Creature.onHitTimer
            TimerKind::Hit(task) => self.on_hit_task(task),
            // Java: getAI().notifyActionReadyToAct() after an attack
            TimerKind::ReadyToAct(id) => {
                if self.objects.contains_key(&id) {
                    self.notify_action_ready_to_act(id);
                }
            }
            TimerKind::AttackStanceCheck => self.on_attack_stance_check(),
            // Java: UseItem's delayed useEquippableItem(item, false)
            TimerKind::EquipAfterAttack { player, item } => {
                if self.players.contains_key(&player) {
                    self.use_equippable_item(player, item, false);
                }
            }
            TimerKind::CharInfo(id) => self.on_char_info_timer(id),
            TimerKind::SkillList(id) => self.on_skill_list_timer(id),
            TimerKind::ItemList(id) => self.on_item_list_timer(id),
            TimerKind::Regeneration(id) => self.on_regeneration_timer(id),
            TimerKind::SelfStatusUpdate(id) => self.on_self_update_timer(id),
            // Java: DecayTaskManager, RespawnTaskManager, SpawnGroupTaskManager
            TimerKind::Decay(id) => self.on_decay_timer(id),
            TimerKind::Respawn(id) => self.on_respawn_timer(id),
            TimerKind::GroupSpawn { group, entry } => self.on_group_spawn_timer(group, entry),
            // Java: ItemManager.ResetOwner, DropProtection, ItemsAutoDestroyTaskManager,
            // ItemsOnGroundManager
            TimerKind::ItemOwnerReset(id) => self.on_item_owner_reset(id),
            TimerKind::ItemDropProtection(id) => self.on_item_drop_protection_end(id),
            TimerKind::ItemsAutoDestroy => self.on_items_auto_destroy(),
            TimerKind::SaveGroundItems => self.on_save_ground_items_timer(),
            // Java: RandomAnimationTaskManager.run (for one NPC)
            TimerKind::RandomAnimation(id) => self.on_random_animation_timer(id),
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
