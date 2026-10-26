//! The world thread and the seam between it and the async side (persistence, LS link).
// Java: org/l2jmobius/gameserver/Shutdown.java (disconnectAllCharacters saves everyone)
// Design: docs/design/world-loop.md ("Processes and tasks", "Shutdown")
//
// The world only knows the `Outbox` trait. Here it is implemented with a channel
// ([`ChannelOutbox`]): the world thread pushes [`OutboxMessage`]s, a tokio task consumes them.
// Which task that is, is decided by the caller of [`WorldRuntime::start`]:
// - tests: [`spawn_stub_consumer`] (logs and drops),
// - the server: the persistence task (crates/game-server/src/persist.rs), which stores
//   `StorePlayer` in order and answers `PlayerLeft` with a `PlayerLogout` notice for the login
//   server link (M2-GS-02) once the data is safe,
// - M2-GS-03 / M2-ENTER-01: client sessions send `WorldCommand`s through [`WorldHandle`] and
//   hand the world a `PacketSink` backed by their `ConnectionHandle`.

use std::sync::mpsc;
use std::thread::JoinHandle;

use l2_util::WorldRng;
use l2_world::{
    ControlRequest, DEFAULT_AUTOSAVE_INTERVAL, LsEvent, Outbox, PersistCommand, World, WorldCommand,
};
use tokio::sync::{mpsc::UnboundedReceiver, mpsc::UnboundedSender, oneshot};
use tokio::task;
use tracing::{debug, error, info, warn};

use crate::state::WorldHandle;

/// What the world hands to the async side.
#[derive(Debug, Clone, PartialEq)]
pub enum OutboxMessage {
    Persist(PersistCommand),
    Ls(LsEvent),
    /// A request to the process around the world (the GM shutdown countdown ended, the html
    /// files are to be read again).
    Control(ControlRequest),
}

/// [`Outbox`] over a channel. Never blocks (the world must not wait, D-15).
pub struct ChannelOutbox {
    sender: UnboundedSender<OutboxMessage>,
}

impl ChannelOutbox {
    pub fn new(sender: UnboundedSender<OutboxMessage>) -> Self {
        Self { sender }
    }

    fn push(&self, message: OutboxMessage) {
        if let Err(lost) = self.sender.send(message) {
            // The consumer is gone: a store may be lost, so this is an error, not a debug line.
            error!("Outbox consumer is gone, dropping {:?}.", lost.0);
        }
    }
}

impl Outbox for ChannelOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.push(OutboxMessage::Persist(command));
    }

    fn ls_event(&mut self, event: LsEvent) {
        self.push(OutboxMessage::Ls(event));
    }

    fn control(&mut self, request: ControlRequest) {
        self.push(OutboxMessage::Control(request));
    }
}

/// Consumer for tests that do not need a database. If a store arrives anyway it is logged
/// loudly, never silently dropped.
pub fn spawn_stub_consumer(mut messages: UnboundedReceiver<OutboxMessage>) -> task::JoinHandle<()> {
    task::spawn(async move {
        while let Some(message) = messages.recv().await {
            match message {
                OutboxMessage::Persist(PersistCommand::StorePlayer(player)) => {
                    warn!(
                        "The stub consumer does not persist: the state of {} is NOT stored.",
                        player.name
                    );
                }
                other => debug!("Outbox message ignored by the stub consumer: {other:?}"),
            }
        }
        debug!("Outbox closed.");
    })
}

/// The running world thread, its command channel and the outbox consumer.
pub struct WorldRuntime {
    handle: WorldHandle,
    thread: JoinHandle<World>,
    consumer: task::JoinHandle<()>,
}

impl WorldRuntime {
    /// Starts the world thread (D-10) with a seeded RNG (D-13) and the consumer made by
    /// `spawn_consumer` from the receiving end of the outbox channel.
    ///
    /// Must run inside a tokio runtime (the consumer is a task).
    pub fn start<F>(rng: WorldRng, spawn_consumer: F) -> std::io::Result<Self>
    where
        F: FnOnce(UnboundedReceiver<OutboxMessage>) -> task::JoinHandle<()>,
    {
        Self::start_with_autosave(rng, DEFAULT_AUTOSAVE_INTERVAL, spawn_consumer)
    }

    /// Like [`start`](Self::start), with the time between two autosaves of a player
    /// (Java `CharacterDataStoreInterval`, see [`crate::persist::autosave_interval`]).
    pub fn start_with_autosave<F>(
        rng: WorldRng,
        autosave: std::time::Duration,
        spawn_consumer: F,
    ) -> std::io::Result<Self>
    where
        F: FnOnce(UnboundedReceiver<OutboxMessage>) -> task::JoinHandle<()>,
    {
        Self::start_configured(rng, autosave, |_| {}, spawn_consumer)
    }

    /// Like [`start_with_autosave`](Self::start_with_autosave), and `setup` can configure the
    /// world (the datapack view, the settings) before its thread starts.
    pub fn start_configured<S, F>(
        rng: WorldRng,
        autosave: std::time::Duration,
        setup: S,
        spawn_consumer: F,
    ) -> std::io::Result<Self>
    where
        S: FnOnce(&mut World),
        F: FnOnce(UnboundedReceiver<OutboxMessage>) -> task::JoinHandle<()>,
    {
        let (outbox_tx, outbox_rx) = tokio::sync::mpsc::unbounded_channel();
        let consumer = spawn_consumer(outbox_rx);
        let mut world = World::new(rng, Box::new(ChannelOutbox::new(outbox_tx)));
        world.set_autosave_interval(autosave);
        setup(&mut world);
        let (commands_tx, commands_rx) = mpsc::channel();
        let thread = l2_world::runner::spawn(world, commands_rx)?;
        Ok(Self {
            handle: WorldHandle::new(commands_tx),
            thread,
            consumer,
        })
    }

    pub fn handle(&self) -> WorldHandle {
        self.handle.clone()
    }

    /// Stores and disconnects every player, stops the world thread, then waits until the
    /// consumer has handled everything the world produced (D-15, "Shutdown" in the design).
    // Java: Shutdown.disconnectAllCharacters
    pub async fn shutdown(self) {
        let (done_tx, done_rx) = oneshot::channel();
        let sent = self.handle.send(WorldCommand::Shutdown {
            done: Box::new(move || {
                let _ = done_tx.send(());
            }),
        });
        if sent {
            // Also resolves (with an error) if the world thread died before calling `done`.
            let _ = done_rx.await;
        } else {
            warn!("The world thread had already stopped.");
        }
        let thread = self.thread;
        // The world is dropped inside the blocking task, which drops its outbox sender and so
        // closes the channel the consumer waits on.
        let joined = task::spawn_blocking(move || thread.join().map(drop)).await;
        match joined {
            Ok(Ok(())) => {}
            Ok(Err(_)) => error!("The world thread panicked."),
            Err(e) => error!("Could not wait for the world thread: {e}"),
        }
        if let Err(e) = self.consumer.await {
            error!("The outbox consumer failed: {e}");
        }
        info!("World stopped.");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};
    use std::time::Duration;

    use l2_world::{LeaveReason, ObjectId, PacketSink, PlayerData, SessionId};

    use super::*;

    struct Sink(Arc<Mutex<Vec<LeaveReason>>>);

    impl PacketSink for Sink {
        fn send(&self, _bytes: Vec<u8>) {}
        fn left_world(&self, reason: LeaveReason) {
            self.0.lock().unwrap().push(reason);
        }
    }

    fn player(id: i32, name: &str) -> PlayerData {
        PlayerData {
            object_id: ObjectId(id),
            account: format!("acc{id}"),
            name: name.into(),
            title: String::new(),
            class_id: 0,
            base_class: 0,
            race: 0,
            sex: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            level: 1,
            exp: 0,
            sp: 0,
            karma: 0,
            pvp_kills: 0,
            pk_kills: 0,
            access_level: 0,
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
            cur_hp: 10.0,
            max_hp: 10.0,
            cur_mp: 5.0,
            max_mp: 5.0,
            cur_cp: 0.0,
            max_cp: 5.0,
            online_time: 0,
            items: vec![],
            skills: vec![],
            shortcuts: vec![],
            profile: Default::default(),
        }
    }

    /// A consumer that keeps what it receives, for assertions.
    fn collecting_consumer(
        store: Arc<Mutex<Vec<OutboxMessage>>>,
    ) -> impl FnOnce(UnboundedReceiver<OutboxMessage>) -> task::JoinHandle<()> {
        move |mut rx| {
            task::spawn(async move {
                while let Some(message) = rx.recv().await {
                    store.lock().unwrap().push(message);
                }
            })
        }
    }

    #[tokio::test]
    async fn shutdown_stores_every_online_player_before_returning() {
        let received = Arc::new(Mutex::new(Vec::new()));
        let runtime =
            WorldRuntime::start(WorldRng::from_seed(3), collecting_consumer(Arc::clone(&received))).unwrap();
        let handle = runtime.handle();
        let left = Arc::new(Mutex::new(Vec::new()));
        for (session, id, name) in [(1, 0x6000_0001, "Alpha"), (2, 0x6000_0002, "Beta")] {
            assert!(handle.send(WorldCommand::EnterWorld {
                session: SessionId(session),
                player: Box::new(player(id, name)),
                sink: Box::new(Sink(Arc::clone(&left))),
                ip: None,
            }));
        }

        tokio::time::timeout(Duration::from_secs(10), runtime.shutdown())
            .await
            .unwrap();

        let messages = received.lock().unwrap().clone();
        let stored: Vec<&str> = messages
            .iter()
            .filter_map(|m| match m {
                OutboxMessage::Persist(PersistCommand::StorePlayer(p)) => Some(p.name.as_str()),
                _ => None,
            })
            .collect();
        assert_eq!(stored.len(), 2, "{messages:?}");
        assert!(stored.contains(&"Alpha") && stored.contains(&"Beta"));
        let left_count = messages
            .iter()
            .filter(|m| matches!(m, OutboxMessage::Persist(PersistCommand::PlayerLeft { .. })))
            .count();
        assert_eq!(left_count, 2);
        // The login server hears about both players entering.
        let entered = messages
            .iter()
            .filter(|m| matches!(m, OutboxMessage::Ls(LsEvent::PlayerInGame { .. })))
            .count();
        assert_eq!(entered, 2);
        assert_eq!(
            *left.lock().unwrap(),
            vec![LeaveReason::ServerShutdown, LeaveReason::ServerShutdown]
        );
        // The world is gone: later commands are refused instead of queued forever.
        assert!(!handle.send(WorldCommand::Disconnected {
            session: SessionId(1)
        }));
    }

    #[tokio::test]
    async fn shutdown_without_players_returns_promptly() {
        let runtime = WorldRuntime::start(WorldRng::from_seed(1), spawn_stub_consumer).unwrap();
        tokio::time::timeout(Duration::from_secs(10), runtime.shutdown())
            .await
            .unwrap();
    }

    #[test]
    fn the_outbox_forwards_in_order() {
        let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
        let mut outbox = ChannelOutbox::new(tx);
        outbox.ls_event(LsEvent::PlayerInGame { account: "a".into() });
        outbox.persist(PersistCommand::PlayerLeft {
            account: "a".into(),
            object_id: ObjectId(1),
            logout: true,
        });
        assert!(matches!(rx.try_recv(), Ok(OutboxMessage::Ls(_))));
        assert!(matches!(rx.try_recv(), Ok(OutboxMessage::Persist(_))));
    }
}
