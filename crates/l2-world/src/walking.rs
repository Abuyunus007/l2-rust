//! `WalkingManager`: NPCs that walk a route of `data/Routes.xml` (M8-MISC-01).
// Java: managers/WalkingManager.java
// Java: entity/actor/holders/npc/{WalkInfo,WalkNode,WalkRoute}.java
// Java: entity/actor/tasks/npc/walker/ArrivedTask.java
// Java: data/holders/NpcRoutesHolder.java (in `l2_data::route`)
// Java: entity/actor/Npc.java (`onSpawn`, `onDecay`), ai/CreatureAI.java (`onEvtArrived`),
//       entity/actor/Attackable.java (`addDamage`)
//
// # How it fits in
// - `Routes.xml` is parsed by `l2_data::route`; [`World::set_walking_routes`] resolves the
//   `NpcStringId` words of the points and keeps the routes. Call it before the spawns: a spawn
//   asks `isTargeted` (random monster spawns and random walking leave a targeted NPC alone).
// - A spawn ([`World::walking_on_spawn`]) starts the walk of an NPC whose id and spawn location
//   are a `<target>` of a route. The walk is a `MoveTo` intention to the current node; the AI's
//   arrival event ([`World::walking_on_arrived`]) says the point's words, steps the cursor and
//   waits `100 ms + delay` before the next move (the `Arrived` timer).
// - Java's three scheduled tasks (`_startMoveTasks`, `_repeatMoveTasks`, `_arriveTasks`) are
//   [`WalkingTimer`]s on the wheel (D-12). The 10 s repeat task keeps trying to resume a walk
//   that a fight interrupted.
// - The `ON_NPC_MOVE_NODE_ARRIVED` and `ON_NPC_MOVE_ROUTE_FINISHED` script events are scripts
//   (D-05) and not sent; `startMoving` for a quest NPC (Q00021) is the script's call and works
//   through [`World::walking_start_moving`].

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use l2_data::route::{NO_REPEAT, REPEAT_GO_BACK, REPEAT_GO_FIRST, REPEAT_RANDOM, REPEAT_TELE_FIRST};
use l2_data::{RouteSay, Routes};
use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::{ChatType, NpcSay};
use l2_util::ObjectId;
use tracing::warn;

use crate::formulas::rng::Rnd;
use crate::movement::Intention;
use crate::target::{distance_2d, distance_3d};
use crate::teleport::TeleportTarget;
use crate::timer::{TimerId, TimerKind, TimerWheel};
use crate::world::World;

/// `WalkInfo.NODE_FIRST`.
const NODE_FIRST: i32 = 0;
/// `WalkInfo.NODE_SECOND`.
const NODE_SECOND: i32 = 1;
/// `WalkInfo.STEP_FORWARD`.
const STEP_FORWARD: i32 = 1;
/// The `10000` of `startMoving`'s repeat and retry tasks, in milliseconds.
const RETRY_MS: u64 = 10_000;
/// `npc.isInsideRadius3D(node, 3000)`: farther than this and the NPC is teleported to the node.
const TOO_FAR: f64 = 3000.0;
/// `npc.isInsideRadius2D(node, 10)` of `onArrived`.
const ARRIVED_RADIUS: f64 = 10.0;

/// What a node makes the NPC say, with the `NpcStringId` resolved.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Say {
    /// `getNpcString()`: the client id.
    NpcString(i32),
    /// `getChatText()`.
    Text(String),
}

/// Java `WalkNode`.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Node {
    x: i32,
    y: i32,
    z: i32,
    delay: i32,
    run: bool,
    say: Option<Say>,
}

/// Java `WalkRoute`, as the world keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Route {
    nodes: Vec<Node>,
    repeat_walk: bool,
    repeat_type: i8,
}

/// Java `WalkInfo`: how far one NPC got on its route.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WalkInfo {
    route: String,
    blocked: bool,
    suspended: bool,
    stopped_by_attack: bool,
    cursor: i32,
    step: i32,
    /// `_walkCheckTask`: the 10 s repeat task of this walk.
    check_task: Option<TimerId>,
}

impl WalkInfo {
    fn new(route: &str) -> Self {
        Self {
            route: route.to_owned(),
            blocked: false,
            suspended: false,
            stopped_by_attack: false,
            cursor: NODE_FIRST,
            step: STEP_FORWARD,
            check_task: None,
        }
    }
}

/// The scheduled tasks of Java's `WalkingManager`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WalkingTimer {
    /// `_startMoveTasks`: `startMoving(npc, routeName)` 10 s after the NPC was busy.
    Start { npc: ObjectId, route: String },
    /// `_repeatMoveTasks`: `scheduleAtFixedRate(startMoving, 10000, 10000)`.
    Repeat { npc: ObjectId },
    /// `ArrivedTask`: the wait at a node is over.
    Arrived { npc: ObjectId, route: String },
}

/// The state of the walking manager (one field of the [`World`]).
#[derive(Debug, Default)]
pub struct WalkingWorld {
    /// `_routes`, `_targetedNpcIds` and `_routesToAttach`.
    targets: Arc<Routes>,
    routes: HashMap<String, Route>,
    /// `_activeRoutes`.
    active: HashMap<ObjectId, WalkInfo>,
    /// `Npc._isWalker`: set once an NPC starts a walk and never cleared.
    walkers: HashSet<ObjectId>,
    start_tasks: HashMap<ObjectId, TimerId>,
    repeat_tasks: HashMap<ObjectId, TimerId>,
    arrive_tasks: HashMap<ObjectId, TimerId>,
}

/// `NpcStringId.getNpcStringId(name)`.
fn npc_string_by_name(name: &str) -> Option<i32> {
    NpcStringId::ALL.iter().find(|s| s.name() == name).map(|s| s.id())
}

impl World {
    /// Java `WalkingManager.load()`: installs the routes (and `reload walker` would call it
    /// again). A point whose `NpcStringId` does not exist is dropped, with Java's warning.
    pub fn set_walking_routes(&mut self, routes: Arc<Routes>) {
        let mut resolved = HashMap::new();
        for route in routes.iter() {
            let name = &route.name;
            let mut nodes = Vec::with_capacity(route.nodes.len());
            for node in &route.nodes {
                let say = match &node.say {
                    None => None,
                    Some(RouteSay::Text(text)) => Some(Say::Text(text.clone())),
                    Some(RouteSay::NpcStringName(word)) => match npc_string_by_name(word) {
                        Some(id) => Some(Say::NpcString(id)),
                        None => {
                            warn!("WalkingManager: Unknown npcString '{word}' for route '{name}'");
                            continue;
                        }
                    },
                    Some(RouteSay::NpcStringId(id)) => match NpcStringId::from_id(*id) {
                        Some(s) => Some(Say::NpcString(s.id())),
                        None => {
                            warn!("WalkingManager: Unknown npcString '{id}' for route '{name}'");
                            continue;
                        }
                    },
                };
                nodes.push(Node {
                    x: node.x,
                    y: node.y,
                    z: node.z,
                    delay: node.delay,
                    run: node.run,
                    say,
                });
            }
            if nodes.is_empty() {
                continue;
            }
            resolved.insert(
                name.clone(),
                Route {
                    nodes,
                    repeat_walk: route.repeat_walk,
                    repeat_type: route.repeat_type,
                },
            );
        }
        self.walking.targets = routes;
        self.walking.routes = resolved;
    }

    /// Java `WalkingManager.isTargeted(npc)`: some route names this NPC id as a target.
    pub fn walking_is_targeted(&self, npc_id: i32) -> bool {
        self.walking.targets.is_targeted(npc_id)
    }

    /// Java `Npc.isWalker()`: a monster follows its leader's flag.
    pub fn is_walker(&self, id: ObjectId) -> bool {
        let owner = self.leader_of(id).unwrap_or(id);
        self.walking.walkers.contains(&owner)
    }

    /// Java `getRouteName(npc)`: the empty string when the NPC is on no route.
    pub fn walking_route_name(&self, npc: ObjectId) -> &str {
        self.walking.active.get(&npc).map_or("", |w| w.route.as_str())
    }

    /// Java `isRegistered(npc)`.
    fn walking_is_registered(&self, npc: ObjectId) -> bool {
        self.walking.active.contains_key(&npc)
    }

    /// The `monster` of `isOnWalk` and `stopMoving`: a monster is its leader, or itself;
    /// anything else has none.
    fn walking_monster_of(&self, npc: ObjectId) -> Option<ObjectId> {
        if self.npcs.get(&npc).is_some_and(|n| n.class.is_monster()) {
            Some(self.leader_of(npc).unwrap_or(npc))
        } else {
            None
        }
    }

    /// Java `WalkingManager.isOnWalk(npc)`.
    pub fn walking_is_on_walk(&self, npc: ObjectId) -> bool {
        let monster = self.walking_monster_of(npc);
        // NOTE(java-quirk): a minion needs its own entry as well as its leader's, and it never has
        // one (only the leader is registered), so it is never "on walk".
        if monster.is_some_and(|m| !self.walking_is_registered(m)) || !self.walking_is_registered(npc) {
            return false;
        }
        self.walking
            .active
            .get(&monster.unwrap_or(npc))
            .is_some_and(|w| !w.stopped_by_attack && !w.suspended)
    }

    fn walking_route(&self, name: &str) -> Option<&Route> {
        self.walking.routes.get(name)
    }

    /// Java `WalkInfo.getCurrentNode()`: the cursor clamped into the route.
    fn current_node(route: &Route, walk: &WalkInfo) -> Node {
        let last = route.nodes.len() - 1;
        let cursor = usize::try_from(walk.cursor.max(NODE_FIRST))
            .unwrap_or(0)
            .min(last);
        route.nodes[cursor].clone()
    }

    fn schedule_walking(&mut self, delay_ms: u64, timer: WalkingTimer) -> TimerId {
        let due = self.tick + TimerWheel::ms_to_ticks(delay_ms);
        self.timers.schedule_at(due, TimerKind::Walking(timer))
    }

    /// Java `WalkInfo.calculateNextNode(npc)`. `true` when the walk ended for good
    /// (`cancelMoving`); the caller then drops `walk`.
    fn walk_calculate_next_node(&mut self, npc: ObjectId, walk: &mut WalkInfo, route: &Route) -> bool {
        let count = i32::try_from(route.nodes.len()).unwrap_or(i32::MAX);
        if route.repeat_type == REPEAT_RANDOM {
            if count <= NODE_SECOND {
                walk.cursor = NODE_FIRST;
                return false;
            }
            let base = walk.cursor;
            let mut selected = base;
            while selected == base {
                let candidate = self.rng().get_int(count);
                if candidate != base {
                    selected = candidate;
                }
            }
            walk.cursor = selected;
            return false;
        }

        walk.cursor += walk.step;
        if walk.cursor == count {
            // `ON_NPC_MOVE_ROUTE_FINISHED`: scripts (D-05).
            if !route.repeat_walk {
                return true;
            }
            match route.repeat_type {
                REPEAT_GO_BACK => {
                    walk.step = -STEP_FORWARD;
                    walk.cursor = count - 2;
                }
                REPEAT_GO_FIRST => {
                    walk.step = STEP_FORWARD;
                    walk.cursor = NODE_FIRST;
                }
                REPEAT_TELE_FIRST => {
                    let (x, y, z) = self.npcs.get(&npc).map_or((0, 0, 0), |n| n.spawn_point);
                    self.tele_to_location(npc, TeleportTarget::at(x, y, z));
                    walk.step = STEP_FORWARD;
                    walk.cursor = NODE_FIRST;
                }
                _ => {}
            }
            return false;
        }
        if walk.cursor == i32::from(NO_REPEAT) {
            walk.cursor = NODE_SECOND;
            walk.step = STEP_FORWARD;
        }
        false
    }

    /// `npc.setRunning()` / `setWalking()` and `getAI().setIntentionMoveTo(node)`.
    fn walk_to_node(&mut self, npc: ObjectId, node: &Node) {
        self.set_running(npc, node.run);
        self.npc_set_intention_move_to(npc, node.x, node.y, node.z);
    }

    /// Java `WalkingManager.startMoving(npc, routeName)`: starts the walk, or resumes one that
    /// stopped.
    pub fn walking_start_moving(&mut self, npc: ObjectId, route_name: &str) {
        let Some(route) = self.walking_route(route_name).cloned() else {
            return;
        };
        if !self.npcs.contains_key(&npc) || self.is_dead(npc) {
            return;
        }
        let idle = matches!(self.intention_of(npc), Intention::Active | Intention::Idle);

        if !self.walking.active.contains_key(&npc) {
            // New walk task.
            if idle {
                let mut walk = WalkInfo::new(route_name);
                let mut node = Self::current_node(&route, &walk);
                // Adjust next waypoint, if NPC spawns at first waypoint.
                let here = self.position_of(npc).unwrap_or((0, 0, 0));
                if here.0 == node.x && here.1 == node.y {
                    // (`cancelMoving` of an NPC that is not registered yet undoes nothing, and Java
                    // goes on with the clamped node.)
                    self.walk_calculate_next_node(npc, &mut walk, &route);
                    node = Self::current_node(&route, &walk);
                }
                if distance_3d(here, (node.x, node.y, node.z)) > TOO_FAR {
                    warn!(
                        "WalkingManager: Route '{route_name}': NPC (id={}, x={}, y={}, z={}) is too far from starting point (node x={}, y={}, z={}, range={}). Teleporting to proper location.",
                        self.npcs.get(&npc).map_or(0, |n| n.npc_id),
                        here.0,
                        here.1,
                        here.2,
                        node.x,
                        node.y,
                        node.z,
                        distance_3d(here, (node.x, node.y, node.z)),
                    );
                    self.tele_to_location(npc, TeleportTarget::at(node.x, node.y, node.z));
                }
                self.walk_to_node(npc, &node);

                // Start the walk check task, for resuming the walk after a fight.
                if !self.walking.repeat_tasks.contains_key(&npc) {
                    let task = self.schedule_walking(RETRY_MS, WalkingTimer::Repeat { npc });
                    self.walking.repeat_tasks.insert(npc, task);
                    walk.check_task = Some(task);
                }
                // `npc.setWalker()`, then register the route.
                self.walking.walkers.insert(npc);
                self.walking.active.insert(npc, walk);
            } else if !self.walking.start_tasks.contains_key(&npc) {
                let task = self.schedule_walking(
                    RETRY_MS,
                    WalkingTimer::Start {
                        npc,
                        route: route_name.to_owned(),
                    },
                );
                self.walking.start_tasks.insert(npc, task);
            }
        } else if idle {
            // The walk was stopped (arrived at a node, a script, a fight...): resume it.
            let Some(walk) = self.walking.active.get_mut(&npc) else {
                return;
            };
            // Prevent a call from the scheduled task and `onArrived` at the same time, or a walk
            // that was stopped on purpose.
            if walk.blocked || walk.suspended {
                return;
            }
            let node = Self::current_node(&route, walk);
            walk.blocked = true;
            self.walk_to_node(npc, &node);
            if let Some(walk) = self.walking.active.get_mut(&npc) {
                walk.blocked = false;
                walk.stopped_by_attack = false;
            }
        }
    }

    /// Java `cancelMoving(npc)`: the walk is over for good.
    pub fn walking_cancel_moving(&mut self, npc: ObjectId) {
        if let Some(walk) = self.walking.active.remove(&npc) {
            self.walking_drop_walk(npc, walk);
        }
    }

    /// Cancels the check task of a walk that left `_activeRoutes`.
    fn walking_drop_walk(&mut self, npc: ObjectId, walk: WalkInfo) {
        if let Some(task) = walk.check_task {
            self.timers.cancel(task);
            // The same task sits in `_repeatMoveTasks`; a cancelled one is replaced next time.
            if self.walking.repeat_tasks.get(&npc) == Some(&task) {
                self.walking.repeat_tasks.remove(&npc);
            }
        }
    }

    /// Java `resumeMoving(npc)`.
    pub fn walking_resume_moving(&mut self, npc: ObjectId) {
        let Some(walk) = self.walking.active.get_mut(&npc) else {
            return;
        };
        walk.suspended = false;
        walk.stopped_by_attack = false;
        let route = walk.route.clone();
        self.walking_start_moving(npc, &route);
    }

    /// Java `stopMoving(npc, suspend, stoppedByAttack)`.
    pub fn walking_stop_moving(&mut self, npc: ObjectId, suspend: bool, stopped_by_attack: bool) {
        let monster = self.walking_monster_of(npc);
        if monster.is_some_and(|m| !self.walking_is_registered(m)) || !self.walking_is_registered(npc) {
            return;
        }
        let target = monster.unwrap_or(npc);
        if let Some(walk) = self.walking.active.get_mut(&target) {
            walk.suspended = suspend;
            walk.stopped_by_attack = stopped_by_attack;
        }
        self.stop_move(target, None);
        self.set_intention_active(target);
    }

    /// Java `WalkingManager.onArrived(npc)`, called by `CreatureAI.onEvtArrived`.
    pub(crate) fn walking_on_arrived(&mut self, npc: ObjectId) {
        let Some(mut walk) = self.walking.active.remove(&npc) else {
            return;
        };
        let Some(route) = self.walking_route(&walk.route).cloned() else {
            self.walking.active.insert(npc, walk);
            return;
        };
        // ON_NPC_MOVE_NODE_ARRIVED: scripts (D-05).
        let count = i32::try_from(route.nodes.len()).unwrap_or(i32::MAX);
        // "Opposite should not happen... but happens sometime."
        if walk.cursor < 0 || walk.cursor >= count {
            self.walking.active.insert(npc, walk);
            return;
        }
        let node = route.nodes[usize::try_from(walk.cursor)
            .unwrap_or(0)
            .min(route.nodes.len() - 1)]
        .clone();
        let here = self.position_of(npc).unwrap_or((i32::MAX, i32::MAX, i32::MAX));
        if distance_2d(here, (node.x, node.y, node.z)) > ARRIVED_RADIUS {
            self.walking.active.insert(npc, walk);
            return;
        }

        let ended = self.walk_calculate_next_node(npc, &mut walk, &route);
        // Prevents the walk check task from running while there is a delay in this node.
        walk.blocked = true;
        let route_name = walk.route.clone();
        if ended {
            self.walking_drop_walk(npc, walk);
        } else {
            self.walking.active.insert(npc, walk);
        }

        if let Some(say) = &node.say {
            self.walking_say(npc, say);
        }
        if !self.walking.arrive_tasks.contains_key(&npc) {
            let delay = 100 + u64::try_from(node.delay).unwrap_or(0) * 1000;
            let task = self.schedule_walking(
                delay,
                WalkingTimer::Arrived {
                    npc,
                    route: route_name,
                },
            );
            self.walking.arrive_tasks.insert(npc, task);
        }
    }

    /// `npc.broadcastSay(ChatType.NPC_GENERAL, ...)`.
    fn walking_say(&mut self, npc: ObjectId, say: &Say) {
        let Some(npc_id) = self.npcs.get(&npc).map(|n| n.npc_id) else {
            return;
        };
        let (npc_string, text) = match say {
            Say::NpcString(id) => (*id, String::new()),
            Say::Text(text) => (-1, text.clone()),
        };
        self.broadcast_packet(
            npc,
            &NpcSay {
                object_id: npc.0,
                text_type: ChatType::NpcGeneral as i32,
                npc_id,
                npc_string,
                text,
                parameters: Vec::new(),
            },
        );
    }

    /// Java `WalkingManager.onSpawn(npc)` (`Npc.onSpawn`, not for a teleport).
    pub(crate) fn walking_on_spawn(&mut self, npc: ObjectId) {
        if self.is_teleporting(npc) {
            return;
        }
        let Some(n) = self.npcs.get(&npc) else {
            return;
        };
        let route = self
            .walking
            .targets
            .route_at(n.npc_id, n.spawn_point)
            .map(str::to_owned);
        if let Some(route) = route {
            self.walking_start_moving(npc, &route);
        }
    }

    /// The NPC left the world for good (decayed): its tasks and entries go. Java leaks the
    /// `Npc` keys of its task maps; a new object id never meets them.
    pub(crate) fn walking_forget_npc(&mut self, npc: ObjectId) {
        self.walking_cancel_moving(npc);
        self.walking.walkers.remove(&npc);
        for task in [
            self.walking.start_tasks.remove(&npc),
            self.walking.repeat_tasks.remove(&npc),
            self.walking.arrive_tasks.remove(&npc),
        ]
        .into_iter()
        .flatten()
        {
            self.timers.cancel(task);
        }
    }

    /// The timers of the walking manager.
    pub(crate) fn on_walking_timer(&mut self, timer: WalkingTimer) {
        match timer {
            WalkingTimer::Start { npc, route } => {
                self.walking.start_tasks.remove(&npc);
                self.walking_start_moving(npc, &route);
            }
            WalkingTimer::Repeat { npc } => {
                // `scheduleAtFixedRate`: the next run is already due.
                let task = self.schedule_walking(RETRY_MS, WalkingTimer::Repeat { npc });
                self.walking.repeat_tasks.insert(npc, task);
                let Some(walk) = self.walking.active.get_mut(&npc) else {
                    // Cancelled with its walk.
                    self.timers.cancel(task);
                    self.walking.repeat_tasks.remove(&npc);
                    return;
                };
                walk.check_task = Some(task);
                let route = walk.route.clone();
                self.walking_start_moving(npc, &route);
            }
            WalkingTimer::Arrived { npc, route } => {
                self.walking.arrive_tasks.remove(&npc);
                // `ArrivedTask`: `_walk.setBlocked(false)`.
                if let Some(walk) = self.walking.active.get_mut(&npc) {
                    walk.blocked = false;
                }
                // NOTE(java-quirk): the task calls `startMoving` even for a walk that ended
                // (`cancelMoving`), so a route without `repeat` starts over from its first point.
                self.walking_start_moving(npc, &route);
            }
        }
    }

    /// The number of routes loaded (for the log line and tests).
    pub fn walking_route_count(&self) -> usize {
        self.walking.routes.len()
    }
}
