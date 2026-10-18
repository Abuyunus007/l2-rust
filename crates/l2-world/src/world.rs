//! The world: all game state, changed only by [`World::handle`] and [`World::tick`].

use std::collections::HashMap;
use std::time::Duration;

use l2_net::Encode;
use l2_util::{ObjectId, WorldRng};
use tracing::{debug, info, warn};

use crate::command::{AdminQuery, ClientInput, LeaveReason, SessionId, WorldCommand};
use crate::outbox::{LsEvent, Outbox, PacketSink, PersistCommand};
use crate::player::{OnlinePlayer, Player, PlayerData};
use crate::timer::{TimerKind, TimerWheel};

/// One world tick (D-12).
pub const TICK: Duration = Duration::from_millis(50);

pub struct World {
    tick: u64,
    rng: WorldRng,
    timers: TimerWheel,
    players: HashMap<ObjectId, Player>,
    sessions: HashMap<SessionId, ObjectId>,
    /// The session of each player (the reverse of `sessions`).
    player_sessions: HashMap<ObjectId, SessionId>,
    sinks: HashMap<ObjectId, Box<dyn PacketSink>>,
    outbox: Box<dyn Outbox>,
    stopped: bool,
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
        }
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
            } => self.enter(session, *player, sink),
            WorldCommand::Input { session, input } => self.on_input(session, input),
            WorldCommand::Disconnected { session } => {
                if let Some(id) = self.sessions.get(&session).copied() {
                    self.leave(id, LeaveReason::Disconnected);
                }
            }
            WorldCommand::Kick { account } => {
                let found = self
                    .players
                    .values()
                    .find(|p| p.data.account == account)
                    .map(|p| p.core.object_id);
                if let Some(id) = found {
                    self.leave(id, LeaveReason::Kicked);
                }
            }
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

    // Java: EnterWorld.run (the world part; the packets come with M2-ENTER-01)
    fn enter(&mut self, session: SessionId, data: PlayerData, sink: Box<dyn PacketSink>) {
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
        self.players.insert(id, Player::from_data(data, self.tick));
        self.sessions.insert(session, id);
        self.player_sessions.insert(id, session);
        self.sinks.insert(id, sink);
        self.outbox.ls_event(LsEvent::PlayerInGame { account });
        self.on_entered(id);
    }

    /// Hook for the enter-world packet sequence (M2-ENTER-01).
    fn on_entered(&mut self, _id: ObjectId) {}

    /// Removes a player: final store, then `PlayerLeft`, then the session hears about it.
    fn leave(&mut self, id: ObjectId, reason: LeaveReason) {
        let Some(player) = self.players.remove(&id) else {
            return;
        };
        if let Some(session) = self.player_sessions.remove(&id) {
            self.sessions.remove(&session);
        }
        let mut snapshot = player.snapshot();
        let online_ticks = self.tick.saturating_sub(player.entered_at_tick);
        let online_secs = online_ticks * TICK.as_millis() as u64 / 1000;
        snapshot.online_time += i64::try_from(online_secs).unwrap_or(0);
        let account = snapshot.account.clone();
        info!("{} ({account}) left the world: {reason:?}.", snapshot.name);
        self.outbox
            .persist(PersistCommand::StorePlayer(Box::new(snapshot)));
        self.outbox.persist(PersistCommand::PlayerLeft {
            account,
            object_id: id,
        });
        if let Some(sink) = self.sinks.remove(&id) {
            sink.left_world(reason);
        }
    }

    fn on_input(&mut self, session: SessionId, input: ClientInput) {
        let Some(id) = self.sessions.get(&session).copied() else {
            debug!("{session:?} sent {input:?} without a character in the world");
            return;
        };
        match input {
            ClientInput::Logout => self.leave(id, LeaveReason::Logout),
            ClientInput::Restart => self.leave(id, LeaveReason::Restart),
            ClientInput::Other { opcode, ex_opcode } => {
                debug!("unhandled in-game packet 0x{opcode:02X} {ex_opcode:?} from {session:?}");
            }
        }
    }

    fn on_timer(&mut self, kind: TimerKind) {
        match kind {
            TimerKind::Autosave(id) => {
                if let Some(player) = self.players.get(&id) {
                    self.outbox
                        .persist(PersistCommand::StorePlayer(Box::new(player.snapshot())));
                }
            }
            TimerKind::Marker(n) => debug!("timer marker {n}"),
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
                    })
                    .collect();
                list.sort_by(|a, b| a.name.cmp(&b.name));
                reply(list);
            }
        }
    }
}
