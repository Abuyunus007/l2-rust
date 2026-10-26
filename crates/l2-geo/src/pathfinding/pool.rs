//! The path worker pool (D-14, `docs/design/m3-world.md`).
//!
//! Rust-only; Java pathfinds on the caller's thread (`Creature.moveToLocation`). Plain `std`
//! threads, no tokio. The world sends a [`PathRequest`] with the door / fence view of the moment,
//! a worker runs [`PathFinder::find_path_detailed`] on the shared immutable [`GeoEngine`] and the
//! [`PathResult`] goes to the caller-provided [`PathSink`] (in the server: a channel that turns
//! results into `WorldCommand::PathFound`).
//!
//! * **Bounded queue.** [`PathPool::request`] fails with [`PathPoolError::QueueFull`] instead of
//!   growing without limit; superseded requests are purged before that is decided.
//! * **Supersede / cancel by id.** A requester (an object id) has at most one live request: a
//!   request with a higher `id` replaces the older one (queued or running); [`PathPool::cancel`]
//!   drops it without a replacement. A dropped request never reaches the sink, even if a worker
//!   already computed it.
//! * **Order.** One queue, first in first out; with several threads results can overtake each
//!   other (they carry `requester` and `id` for that reason).
//! * **Shutdown.** [`PathPool::shutdown`] (also on drop) stops taking requests, discards the queue,
//!   lets running searches finish (their results are dropped) and joins the threads.
//! * **Edited geodata (L-07).** The engine given to the pool is only the default. A request can
//!   carry the world's current engine ([`PathRequest::with_geo`]), like it carries the doors: the
//!   world edits its engine copy-on-write, so a request searches the geodata as it was when the
//!   request was made and the pool never needs a lock or a "new engine" message.

use std::collections::{HashMap, VecDeque};
use std::panic::{AssertUnwindSafe, catch_unwind};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Condvar, Mutex, MutexGuard, PoisonError};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use thiserror::Error;
use tracing::{debug, warn};

use super::{GeoLocation, PathError, PathFailure, PathFinder, PathOptions};
use crate::coords::Location;
use crate::engine::GeoEngine;
use crate::obstacles::DynamicObstacles;

/// One path search.
#[derive(Clone)]
pub struct PathRequest {
    /// Who asked (an object id). A newer `id` of the same requester supersedes an older one.
    pub requester: u64,
    /// The requester's own serial number; higher is newer. Echoed in the [`PathResult`].
    pub id: u64,
    pub from: Location,
    pub to: Location,
    pub instance_id: i32,
    /// Java `playable`: players get the multi-pass line of sight postfilter.
    pub playable: bool,
    /// Door / fence view for the postfilter, taken when the request was made.
    pub obstacles: Arc<dyn DynamicObstacles>,
    /// The geodata to search, taken when the request was made (the world's engine after runtime
    /// edits). `None`: the engine the pool was started with.
    pub geo: Option<Arc<GeoEngine>>,
}

impl PathRequest {
    /// Instance 0, not playable.
    pub fn new(
        requester: u64,
        id: u64,
        from: Location,
        to: Location,
        obstacles: Arc<dyn DynamicObstacles>,
    ) -> PathRequest {
        PathRequest {
            requester,
            id,
            from,
            to,
            instance_id: 0,
            playable: false,
            obstacles,
            geo: None,
        }
    }

    /// Searches this engine instead of the pool's (a snapshot of the world's edited geodata).
    pub fn with_geo(mut self, geo: Arc<GeoEngine>) -> PathRequest {
        self.geo = Some(geo);
        self
    }

    pub fn with_instance(mut self, instance_id: i32) -> PathRequest {
        self.instance_id = instance_id;
        self
    }

    pub fn with_playable(mut self, playable: bool) -> PathRequest {
        self.playable = playable;
        self
    }
}

impl std::fmt::Debug for PathRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PathRequest")
            .field("requester", &self.requester)
            .field("id", &self.id)
            .field("from", &self.from)
            .field("to", &self.to)
            .field("instance_id", &self.instance_id)
            .field("playable", &self.playable)
            .field("own_geo", &self.geo.is_some())
            .finish_non_exhaustive()
    }
}

/// The answer to a [`PathRequest`].
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PathResult {
    pub requester: u64,
    pub id: u64,
    /// Java's `null` is an `Err` here, with the reason.
    pub path: Result<Vec<GeoLocation>, PathFailure>,
    /// Time the search itself took on the worker (not the time in the queue).
    pub elapsed: Duration,
}

impl PathResult {
    /// The path as Java sees it: `None` for `null`.
    pub fn into_path(self) -> Option<Vec<GeoLocation>> {
        self.path.ok()
    }
}

/// Where finished paths go. Called on a worker thread, outside any pool lock; keep it short.
pub trait PathSink: Send + Sync {
    fn deliver(&self, result: PathResult);
}

impl<F> PathSink for F
where
    F: Fn(PathResult) + Send + Sync,
{
    fn deliver(&self, result: PathResult) {
        self(result);
    }
}

/// A channel as sink. A closed receiver is ignored (the world is shutting down).
impl PathSink for Sender<PathResult> {
    fn deliver(&self, result: PathResult) {
        let _ = self.send(result);
    }
}

/// Why [`PathPool::request`] refused a request.
#[derive(Debug, Error)]
pub enum PathPoolError {
    /// The queue holds `capacity` live requests.
    #[error("path queue is full ({capacity} requests)")]
    QueueFull { capacity: usize },
    /// A newer request of the same requester is already known.
    #[error("request {id} of requester {requester} is older than request {newest}")]
    Stale { requester: u64, id: u64, newest: u64 },
    /// The pool is shut down.
    #[error("path pool is shut down")]
    ShutDown,
    /// The pool could not start its threads.
    #[error("cannot start path worker: {0}")]
    Spawn(#[source] std::io::Error),
    /// The buffer configuration is invalid.
    #[error(transparent)]
    Config(#[from] PathError),
}

/// Pool settings.
#[derive(Clone, Debug)]
pub struct PathPoolConfig {
    /// Worker threads (at least 1).
    pub threads: usize,
    /// Maximum number of queued requests (at least 1).
    pub queue_capacity: usize,
    /// Buffers and weights for the searches.
    pub options: PathOptions,
}

impl PathPoolConfig {
    pub fn new(threads: usize) -> PathPoolConfig {
        PathPoolConfig {
            threads,
            queue_capacity: 1024,
            options: PathOptions::shipped(),
        }
    }
}

/// Counters since the pool started.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PoolStats {
    /// Requests accepted into the queue.
    pub submitted: u64,
    /// Results handed to the sink.
    pub delivered: u64,
    /// Requests dropped because a newer one or a cancel replaced them (queued, running or finished).
    pub superseded: u64,
    /// Requests refused with `QueueFull`.
    pub rejected: u64,
    /// Searches that panicked (reported to the sink as `NoPath`).
    pub panicked: u64,
}

#[derive(Default)]
struct Counters {
    submitted: AtomicU64,
    delivered: AtomicU64,
    superseded: AtomicU64,
    rejected: AtomicU64,
    panicked: AtomicU64,
}

#[derive(Default)]
struct State {
    queue: VecDeque<PathRequest>,
    /// The newest live request id per requester. Removed when its result is delivered.
    latest: HashMap<u64, u64>,
    shutdown: bool,
}

impl State {
    fn is_current(&self, requester: u64, id: u64) -> bool {
        self.latest.get(&requester) == Some(&id)
    }
}

struct Shared {
    engine: Arc<GeoEngine>,
    sink: Arc<dyn PathSink>,
    capacity: usize,
    state: Mutex<State>,
    wake: Condvar,
    counters: Counters,
}

impl Shared {
    fn lock(&self) -> MutexGuard<'_, State> {
        // A panic while holding the lock cannot leave the queue half updated (plain collection
        // operations), so a poisoned lock is still usable.
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }
}

/// Worker threads that answer path requests.
pub struct PathPool {
    shared: Arc<Shared>,
    workers: Vec<JoinHandle<()>>,
}

impl std::fmt::Debug for PathPool {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PathPool")
            .field("threads", &self.workers.len())
            .field("capacity", &self.shared.capacity)
            .finish_non_exhaustive()
    }
}

impl PathPool {
    /// `threads` workers, the shipped path options and a queue of 1024.
    pub fn new(
        threads: usize,
        engine: Arc<GeoEngine>,
        sink: Arc<dyn PathSink>,
    ) -> Result<PathPool, PathPoolError> {
        PathPool::with_config(PathPoolConfig::new(threads), engine, sink)
    }

    pub fn with_config(
        config: PathPoolConfig,
        engine: Arc<GeoEngine>,
        sink: Arc<dyn PathSink>,
    ) -> Result<PathPool, PathPoolError> {
        let threads = config.threads.max(1);
        // Validate the buffer definition once, before any thread starts.
        let first = PathFinder::new(config.options.clone())?;
        let shared = Arc::new(Shared {
            engine,
            sink,
            capacity: config.queue_capacity.max(1),
            state: Mutex::new(State::default()),
            wake: Condvar::new(),
            counters: Counters::default(),
        });

        let mut pool = PathPool {
            shared: Arc::clone(&shared),
            workers: Vec::with_capacity(threads),
        };
        let mut finder = Some(first);
        for n in 0..threads {
            let finder = match finder.take() {
                Some(f) => f,
                None => PathFinder::new(config.options.clone())?,
            };
            let worker_shared = Arc::clone(&shared);
            let options = config.options.clone();
            let handle = std::thread::Builder::new()
                .name(format!("path-worker-{n}"))
                .spawn(move || worker_loop(&worker_shared, finder, &options))
                .map_err(PathPoolError::Spawn);
            match handle {
                Ok(h) => pool.workers.push(h),
                Err(e) => {
                    pool.shutdown();
                    return Err(e);
                }
            }
        }
        Ok(pool)
    }

    /// Queues a search. A request with a higher `id` than the requester's previous one replaces
    /// it; a lower one is refused as [`PathPoolError::Stale`].
    pub fn request(&self, request: PathRequest) -> Result<(), PathPoolError> {
        let shared = &self.shared;
        let mut state = shared.lock();
        if state.shutdown {
            return Err(PathPoolError::ShutDown);
        }
        if let Some(&newest) = state.latest.get(&request.requester) {
            if newest > request.id {
                shared.counters.superseded.fetch_add(1, Ordering::Relaxed);
                return Err(PathPoolError::Stale {
                    requester: request.requester,
                    id: request.id,
                    newest,
                });
            }
        }

        let previous = state.latest.insert(request.requester, request.id);
        if state.queue.len() >= shared.capacity {
            // Make room by dropping what no longer matters before refusing anybody.
            let State { queue, latest, .. } = &mut *state;
            let before = queue.len();
            queue.retain(|r| latest.get(&r.requester) == Some(&r.id));
            shared
                .counters
                .superseded
                .fetch_add((before - queue.len()) as u64, Ordering::Relaxed);
        }
        if state.queue.len() >= shared.capacity {
            match previous {
                Some(p) => state.latest.insert(request.requester, p),
                None => state.latest.remove(&request.requester),
            };
            shared.counters.rejected.fetch_add(1, Ordering::Relaxed);
            return Err(PathPoolError::QueueFull {
                capacity: shared.capacity,
            });
        }

        state.queue.push_back(request);
        shared.counters.submitted.fetch_add(1, Ordering::Relaxed);
        drop(state);
        shared.wake.notify_one();
        Ok(())
    }

    /// Drops the live request of `requester`, queued or running (no result reaches the sink).
    /// Returns whether there was one.
    pub fn cancel(&self, requester: u64) -> bool {
        let mut state = self.shared.lock();
        let had = state.latest.remove(&requester).is_some();
        let before = state.queue.len();
        state.queue.retain(|r| r.requester != requester);
        let removed = before - state.queue.len();
        drop(state);
        self.shared
            .counters
            .superseded
            .fetch_add(removed as u64, Ordering::Relaxed);
        had
    }

    /// Requests waiting for a worker.
    pub fn queued(&self) -> usize {
        self.shared.lock().queue.len()
    }

    pub fn threads(&self) -> usize {
        self.workers.len()
    }

    pub fn stats(&self) -> PoolStats {
        let c = &self.shared.counters;
        PoolStats {
            submitted: c.submitted.load(Ordering::Relaxed),
            delivered: c.delivered.load(Ordering::Relaxed),
            superseded: c.superseded.load(Ordering::Relaxed),
            rejected: c.rejected.load(Ordering::Relaxed),
            panicked: c.panicked.load(Ordering::Relaxed),
        }
    }

    /// Stops the pool: no new requests, queued ones are discarded, running searches finish (their
    /// results are dropped) and the threads are joined. Idempotent. Do not call it from the sink.
    pub fn shutdown(&mut self) {
        {
            let mut state = self.shared.lock();
            state.shutdown = true;
            let dropped = state.queue.len();
            state.queue.clear();
            state.latest.clear();
            self.shared
                .counters
                .superseded
                .fetch_add(dropped as u64, Ordering::Relaxed);
        }
        self.shared.wake.notify_all();
        for handle in self.workers.drain(..) {
            if handle.join().is_err() {
                warn!("path worker panicked outside a search");
            }
        }
    }
}

impl Drop for PathPool {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn worker_loop(shared: &Shared, mut finder: PathFinder, options: &PathOptions) {
    loop {
        // Wait for a live request.
        let request = {
            let mut state = shared.lock();
            loop {
                if state.shutdown {
                    return;
                }
                match state.queue.pop_front() {
                    Some(r) if state.is_current(r.requester, r.id) => break r,
                    Some(_) => {
                        shared.counters.superseded.fetch_add(1, Ordering::Relaxed);
                    }
                    None => {
                        state = shared.wake.wait(state).unwrap_or_else(PoisonError::into_inner);
                    }
                }
            }
        };

        let started = Instant::now();
        let engine = request.geo.as_ref().unwrap_or(&shared.engine);
        let searched = catch_unwind(AssertUnwindSafe(|| {
            finder.find_path_detailed(
                engine,
                request.obstacles.as_ref(),
                request.from,
                request.to,
                request.instance_id,
                request.playable,
            )
        }));
        let path = match searched {
            Ok(path) => path,
            Err(_) => {
                // Java catches the exception, logs it and answers null.
                warn!(
                    "path search panicked (requester {}, request {})",
                    request.requester, request.id
                );
                shared.counters.panicked.fetch_add(1, Ordering::Relaxed);
                // The buffers may be half used: start clean.
                if let Ok(fresh) = PathFinder::new(options.clone()) {
                    finder = fresh;
                }
                Err(PathFailure::NoPath)
            }
        };
        let elapsed = started.elapsed();

        // Still wanted? Decided under the lock, so a request that arrives right now either
        // supersedes this one before the check or is queued after it.
        let wanted = {
            let mut state = shared.lock();
            if state.shutdown {
                return;
            }
            let wanted = state.is_current(request.requester, request.id);
            if wanted {
                state.latest.remove(&request.requester);
            }
            wanted
        };
        if !wanted {
            shared.counters.superseded.fetch_add(1, Ordering::Relaxed);
            debug!(
                "dropped superseded path result (requester {}, request {})",
                request.requester, request.id
            );
            continue;
        }

        let result = PathResult {
            requester: request.requester,
            id: request.id,
            path,
            elapsed,
        };
        // Counted before the hand-over, so whoever receives the result already sees it counted.
        shared.counters.delivered.fetch_add(1, Ordering::Relaxed);
        if catch_unwind(AssertUnwindSafe(|| shared.sink.deliver(result))).is_err() {
            warn!("path sink panicked");
        }
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

    use std::sync::mpsc::{self, Receiver};

    use super::super::test_support::{RX, RY, arc_engine, at};
    use super::*;
    use crate::obstacles::NoObstacles;

    const WAIT: Duration = Duration::from_secs(20);

    fn obstacles() -> Arc<dyn DynamicObstacles> {
        Arc::new(NoObstacles)
    }

    /// A request whose path walks around the wall of the test engine.
    fn request(requester: u64, id: u64) -> PathRequest {
        PathRequest::new(
            requester,
            id,
            at(RX + 990, RY + 1000),
            at(RX + 1010, RY + 1000),
            obstacles(),
        )
        .with_playable(true)
    }

    fn channel_pool(threads: usize) -> (PathPool, Receiver<PathResult>) {
        let (tx, rx) = mpsc::channel();
        let pool = PathPool::new(threads, arc_engine(), Arc::new(tx)).unwrap();
        (pool, rx)
    }

    /// A sink that blocks the worker in `deliver` until released, to keep it busy on purpose.
    struct Gate {
        entered: Sender<PathResult>,
        release: Mutex<Receiver<()>>,
    }

    impl PathSink for Gate {
        fn deliver(&self, result: PathResult) {
            let _ = self.entered.send(result);
            let _ = self.release.lock().unwrap().recv();
        }
    }

    #[test]
    fn answers_a_request_with_the_same_path_as_the_finder() {
        let (pool, rx) = channel_pool(2);
        pool.request(request(7, 1)).unwrap();
        let result = rx.recv_timeout(WAIT).unwrap();
        assert_eq!((result.requester, result.id), (7, 1));
        let path = result.path.unwrap();

        let engine = arc_engine();
        let mut finder = PathFinder::new(PathOptions::shipped()).unwrap();
        let req = request(7, 1);
        assert_eq!(
            finder.find_path(&engine, &NoObstacles, req.from, req.to, 0, true),
            Some(path)
        );
        assert_eq!(pool.stats().delivered, 1);
    }

    /// L-07: the world edits its engine copy-on-write and hands the new one along with the
    /// request; the pool's own engine (and any older snapshot) keeps the old geodata.
    #[test]
    fn a_request_searches_the_edited_geodata_it_carries() {
        let started_with = arc_engine();
        let (tx, rx) = mpsc::channel();
        let pool = PathPool::new(1, Arc::clone(&started_with), Arc::new(tx)).unwrap();

        // The world's handle on the same engine; opening the wall copies the engine (the pool
        // still holds it) and, inside it, only the edited region.
        let mut world_geo = Arc::clone(&started_with);
        for y in RY + 960..RY + 1040 {
            Arc::make_mut(&mut world_geo).set_nearest_nswe(RX + 1000, y, 0, crate::cell::NSWE_ALL);
        }
        assert!(!Arc::ptr_eq(&world_geo, &started_with));
        assert!(!Arc::ptr_eq(
            world_geo.set().region_arc(20, 18).unwrap(),
            started_with.set().region_arc(20, 18).unwrap()
        ));
        assert!(started_with.is_completely_blocked(RX + 1000, RY + 1000, 0));
        assert!(!world_geo.is_completely_blocked(RX + 1000, RY + 1000, 0));

        // Without its own engine the request walks around the wall; with the edited one it goes
        // straight through.
        pool.request(request(1, 1)).unwrap();
        let around = rx.recv_timeout(WAIT).unwrap().path.unwrap();
        pool.request(request(2, 1).with_geo(Arc::clone(&world_geo)))
            .unwrap();
        let through = rx.recv_timeout(WAIT).unwrap().path.unwrap();
        assert_ne!(around, through);
        let mut finder = PathFinder::new(PathOptions::shipped()).unwrap();
        let req = request(2, 1);
        assert_eq!(
            finder.find_path(&world_geo, &NoObstacles, req.from, req.to, 0, true),
            Some(through)
        );

        // A second edit while nobody else holds the edited engine changes it in place.
        drop(pool);
        let before = Arc::as_ptr(&world_geo);
        Arc::make_mut(&mut world_geo).unset_nearest_nswe(RX + 1000, RY + 1000, 0, crate::cell::NSWE_ALL);
        assert_eq!(Arc::as_ptr(&world_geo), before);
        assert!(world_geo.is_completely_blocked(RX + 1000, RY + 1000, 0));
    }

    #[test]
    fn single_worker_answers_in_request_order() {
        let (pool, rx) = channel_pool(1);
        for requester in 0..20 {
            pool.request(request(requester, 1)).unwrap();
        }
        let order: Vec<u64> = (0..20)
            .map(|_| rx.recv_timeout(WAIT).unwrap().requester)
            .collect();
        assert_eq!(order, (0..20).collect::<Vec<_>>());
    }

    #[test]
    fn many_workers_answer_everything_once() {
        let (pool, rx) = channel_pool(4);
        for requester in 0..64 {
            pool.request(request(requester, 5)).unwrap();
        }
        let mut seen: Vec<u64> = (0..64)
            .map(|_| rx.recv_timeout(WAIT).unwrap().requester)
            .collect();
        seen.sort_unstable();
        assert_eq!(seen, (0..64).collect::<Vec<_>>());
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn newer_request_supersedes_an_older_one_in_the_queue() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let sink = Arc::new(Gate {
            entered: entered_tx,
            release: Mutex::new(release_rx),
        });
        let pool = PathPool::new(1, arc_engine(), sink).unwrap();

        // Keep the only worker busy inside the sink.
        pool.request(request(1, 1)).unwrap();
        assert_eq!(entered_rx.recv_timeout(WAIT).unwrap().requester, 1);

        pool.request(request(2, 1)).unwrap();
        pool.request(request(2, 2)).unwrap();
        pool.request(request(2, 3)).unwrap();
        pool.request(request(3, 1)).unwrap();
        // An older id than the live one is refused.
        assert!(matches!(
            pool.request(request(2, 2)),
            Err(PathPoolError::Stale { newest: 3, .. })
        ));

        release_tx.send(()).unwrap();
        let second = entered_rx.recv_timeout(WAIT).unwrap();
        assert_eq!((second.requester, second.id), (2, 3));
        release_tx.send(()).unwrap();
        let third = entered_rx.recv_timeout(WAIT).unwrap();
        assert_eq!((third.requester, third.id), (3, 1));
        release_tx.send(()).unwrap();
        assert!(entered_rx.recv_timeout(Duration::from_millis(100)).is_err());

        let stats = pool.stats();
        assert_eq!(stats.delivered, 3);
        assert_eq!(
            stats.superseded, 3,
            "two replaced in the queue plus one stale refusal"
        );
    }

    #[test]
    fn cancel_drops_queued_and_running_requests() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let sink = Arc::new(Gate {
            entered: entered_tx,
            release: Mutex::new(release_rx),
        });
        let pool = PathPool::new(1, arc_engine(), sink).unwrap();

        pool.request(request(1, 1)).unwrap();
        assert_eq!(entered_rx.recv_timeout(WAIT).unwrap().requester, 1);

        pool.request(request(2, 1)).unwrap();
        pool.request(request(3, 1)).unwrap();
        assert!(pool.cancel(2));
        assert!(!pool.cancel(2));
        assert!(!pool.cancel(99));

        release_tx.send(()).unwrap();
        let next = entered_rx.recv_timeout(WAIT).unwrap();
        assert_eq!(next.requester, 3);
        release_tx.send(()).unwrap();
        assert!(entered_rx.recv_timeout(Duration::from_millis(100)).is_err());
    }

    #[test]
    fn a_finished_search_of_a_superseded_request_is_not_delivered() {
        // Hold the worker inside a search by making the door check block.
        struct Blocking {
            entered: Mutex<Option<Sender<()>>>,
            release: Mutex<Receiver<()>>,
        }
        impl DynamicObstacles for Blocking {
            fn doors_between(&self, _: Location, _: Location, _: i32, _: bool) -> bool {
                if let Some(tx) = self.entered.lock().unwrap().take() {
                    let _ = tx.send(());
                    let _ = self.release.lock().unwrap().recv();
                }
                false
            }
            fn fence_between(&self, _: Location, _: Location, _: i32) -> bool {
                false
            }
        }

        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let blocking: Arc<dyn DynamicObstacles> = Arc::new(Blocking {
            entered: Mutex::new(Some(entered_tx)),
            release: Mutex::new(release_rx),
        });
        let (pool, rx) = channel_pool(1);

        let mut first = request(1, 1);
        first.obstacles = blocking;
        pool.request(first).unwrap();
        entered_rx.recv_timeout(WAIT).unwrap();

        // The search of request 1 is running; request 2 replaces it.
        pool.request(request(1, 2)).unwrap();
        release_tx.send(()).unwrap();

        let result = rx.recv_timeout(WAIT).unwrap();
        assert_eq!((result.requester, result.id), (1, 2));
        assert!(rx.recv_timeout(Duration::from_millis(100)).is_err());
        assert_eq!(pool.stats().delivered, 1);
    }

    #[test]
    fn full_queue_refuses_and_keeps_the_old_request() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let sink = Arc::new(Gate {
            entered: entered_tx,
            release: Mutex::new(release_rx),
        });
        let mut config = PathPoolConfig::new(1);
        config.queue_capacity = 2;
        let pool = PathPool::with_config(config, arc_engine(), sink).unwrap();

        pool.request(request(1, 1)).unwrap();
        entered_rx.recv_timeout(WAIT).unwrap();

        pool.request(request(2, 1)).unwrap();
        pool.request(request(3, 1)).unwrap();
        assert_eq!(pool.queued(), 2);
        assert!(matches!(
            pool.request(request(4, 1)),
            Err(PathPoolError::QueueFull { capacity: 2 })
        ));
        // The refused request must not have left a live entry behind: requester 4 can retry later.
        // A newer request of a queued requester replaces it in place (the old entry is purged).
        pool.request(request(2, 2)).unwrap();
        assert_eq!(pool.queued(), 2);

        release_tx.send(()).unwrap();
        let a = entered_rx.recv_timeout(WAIT).unwrap();
        release_tx.send(()).unwrap();
        let b = entered_rx.recv_timeout(WAIT).unwrap();
        release_tx.send(()).unwrap();
        let mut got = vec![(a.requester, a.id), (b.requester, b.id)];
        got.sort_unstable();
        assert_eq!(got, vec![(2, 2), (3, 1)]);
        assert_eq!(pool.stats().rejected, 1);
    }

    #[test]
    fn shutdown_discards_the_queue_joins_and_refuses_new_requests() {
        let (entered_tx, entered_rx) = mpsc::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let sink = Arc::new(Gate {
            entered: entered_tx,
            release: Mutex::new(release_rx),
        });
        let mut pool = PathPool::new(1, arc_engine(), sink).unwrap();
        pool.request(request(1, 1)).unwrap();
        entered_rx.recv_timeout(WAIT).unwrap();
        pool.request(request(2, 1)).unwrap();

        // Let the worker go as soon as shutdown has started waiting for it.
        let releaser = std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(100));
            let _ = release_tx.send(());
        });
        pool.shutdown();
        releaser.join().unwrap();

        assert_eq!(pool.threads(), 0);
        assert!(matches!(
            pool.request(request(3, 1)),
            Err(PathPoolError::ShutDown)
        ));
        // The queued request 2 was discarded and never answered.
        assert!(entered_rx.recv_timeout(Duration::from_millis(100)).is_err());
        // Idempotent.
        pool.shutdown();
    }

    #[test]
    fn dropping_an_idle_pool_stops_its_threads() {
        let (pool, rx) = channel_pool(3);
        drop(pool);
        // The sink (and its sender) went away with the workers.
        assert!(matches!(
            rx.recv_timeout(WAIT),
            Err(mpsc::RecvTimeoutError::Disconnected)
        ));
    }

    #[test]
    fn invalid_buffer_configuration_is_refused() {
        let (tx, _rx) = mpsc::channel();
        let mut config = PathPoolConfig::new(2);
        config.options.buffer_sizes = Vec::new();
        assert!(matches!(
            PathPool::with_config(config, arc_engine(), Arc::new(tx)),
            Err(PathPoolError::Config(_))
        ));
    }

    #[test]
    fn no_path_results_carry_the_reason() {
        let (pool, rx) = channel_pool(1);
        let mut req = request(1, 1);
        req.to = Location::new(-600_000, -500_000, 0);
        pool.request(req).unwrap();
        let result = rx.recv_timeout(WAIT).unwrap();
        assert_eq!(result.path, Err(PathFailure::NoGeo));
        assert_eq!(result.into_path(), None);
    }
}
