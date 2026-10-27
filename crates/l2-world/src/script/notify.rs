//! Event dispatch: what the world hooks call, and the queue of Java's asynchronous events.
// Java: mechanics/events/EventDispatcher.java (notifyEvent, notifyEventAsync, notifyToListeners)
// Java: mechanics/script/Quest.java (notifyEvent, notifyTalk, notifyFirstTalk, notifyItemTalk,
//       notifyEventReceived, notifyMoveFinished, notifyNodeArrived, notifyRouteFinished,
//       notifyTeleport, notifyOnCanSeeMe, playerEnter)
// Java: the firing sites: handlers/actions/click/NpcClick, handlers/bypass/npc/ChatLink,
//       Npc.onSpawn, Attackable.addDamage / addDamageHate / doDie, Playable.doDie,
//       ZoneType.revalidateInZone / removeCharacter, Player.onPlayerEnter, Item.onBypassFeedback,
//       AttackableAI (ON_NPC_HATE, ON_ATTACKABLE_FACTION_CALL), WalkingManager, WalkInfo
//
// # Order
// Java's `notifyEventAsync` hands the event to a thread pool, so it runs after the code that
// fired it. Here it waits in a queue that [`World::script_drain`] empties at the end of the
// command or tick that queued it (an event queued while the queue runs runs in the same loop).
// Synchronous Java events (`notifyEvent`: `OnCreatureDeath`, `OnAttackableHate`, the quest
// window, the script bypass, timers) run at once. Listeners run in registration order
// (`setup.rs`).

use l2_util::ObjectId;
use tracing::warn;

use super::api::{ScriptItem, ScriptNpc};
use super::ctx::{ScriptCtx, show_result};
use super::qs::Qs;
use super::setup::{EventKind, ScriptId};
use super::timers::{DEFAULT_ON_KILL_DELAY_MS, ScriptTimer};
use crate::effects::SkillKey;
use crate::timer::{TimerKind, TimerWheel};
use crate::world::World;

/// A safety net against scripts that queue events forever: Java would just keep a pool busy.
const MAX_EVENTS_PER_DRAIN: usize = 100_000;

/// An event Java sends with `notifyEventAsync`, waiting for the end of the command.
#[derive(Debug, Clone)]
pub(crate) enum PendingEvent {
    FirstTalk {
        npc: ScriptNpc,
        player: ObjectId,
    },
    Attack {
        npc: ScriptNpc,
        attacker: ObjectId,
        damage: i32,
        is_summon: bool,
        skill: Option<SkillKey>,
    },
    AggroRangeEnter {
        npc: ScriptNpc,
        player: ObjectId,
        is_summon: bool,
    },
    Spawn(ScriptNpc),
    Teleport(ScriptNpc),
    ZoneEnter {
        creature: ObjectId,
        zone_id: i32,
    },
    ZoneExit {
        creature: ObjectId,
        zone_id: i32,
    },
    Login(ObjectId),
    ItemTalk {
        item: ScriptItem,
        player: ObjectId,
    },
    ItemEvent {
        item: ScriptItem,
        player: ObjectId,
        event: String,
    },
    SkillSee {
        npc: ScriptNpc,
        caster: ObjectId,
        skill: SkillKey,
        targets: Vec<ObjectId>,
        is_summon: bool,
    },
    SpellFinished {
        npc: ScriptNpc,
        player: Option<ObjectId>,
        skill: SkillKey,
    },
    FactionCall {
        npc: ScriptNpc,
        caller: ScriptNpc,
        attacker: ObjectId,
        is_summon: bool,
    },
    CreatureSee {
        npc: ScriptNpc,
        creature: ObjectId,
    },
    EventReceived {
        event: String,
        sender: ScriptNpc,
        receiver: ScriptNpc,
        reference: Option<ObjectId>,
    },
    MoveFinished(ScriptNpc),
    NodeArrived(ScriptNpc),
    RouteFinished(ScriptNpc),
}

impl World {
    fn script_npc_of(&self, npc: ObjectId) -> Option<ScriptNpc> {
        self.npcs.get(&npc).map(|n| ScriptNpc {
            object_id: npc,
            id: n.npc_id,
        })
    }

    fn has_npc_listener(&self, kind: EventKind, npc: ObjectId) -> Option<ScriptNpc> {
        let npc = self.script_npc_of(npc)?;
        self.scripts.listeners.has(kind, npc.id).then_some(npc)
    }

    /// `attacker.asPlayer()` and `attacker.isSummon()`.
    fn player_and_summon(&self, attacker: ObjectId) -> Option<(ObjectId, bool)> {
        let player = self.acting_player(attacker)?;
        Some((player, self.summons.summons.contains_key(&attacker)))
    }

    fn queue_script_event(&mut self, event: PendingEvent) {
        self.scripts.pending.push_back(event);
    }

    /// Runs the queued asynchronous events. The world calls it at the end of every command and
    /// tick; tests that call world functions directly can call it too.
    pub fn script_drain(&mut self) {
        if self.scripts.draining {
            return;
        }
        self.scripts.draining = true;
        let mut handled = 0;
        while let Some(event) = self.scripts.pending.pop_front() {
            handled += 1;
            if handled > MAX_EVENTS_PER_DRAIN {
                warn!("Script events: more than {MAX_EVENTS_PER_DRAIN} in one go; the rest is dropped.");
                self.scripts.pending.clear();
                break;
            }
            self.dispatch_script_event(event);
        }
        self.scripts.draining = false;
    }

    fn listeners_of(&self, kind: EventKind, id: i32) -> Vec<ScriptId> {
        self.scripts.listeners.of(kind, id).to_vec()
    }

    fn dispatch_script_event(&mut self, event: PendingEvent) {
        match event {
            PendingEvent::FirstTalk { npc, player } => {
                for script in self.listeners_of(EventKind::NpcFirstTalk, npc.id) {
                    self.script_notify_first_talk(script, npc, player);
                }
            }
            PendingEvent::Attack {
                npc,
                attacker,
                damage,
                is_summon,
                skill,
            } => {
                for script in self.listeners_of(EventKind::AttackableAttack, npc.id) {
                    self.with_script(script, |s, ctx| s.on_attack(ctx, npc, attacker, damage, is_summon, skill));
                }
            }
            PendingEvent::AggroRangeEnter { npc, player, is_summon } => {
                for script in self.listeners_of(EventKind::AttackableAggroRangeEnter, npc.id) {
                    self.with_script(script, |s, ctx| s.on_aggro_range_enter(ctx, npc, player, is_summon));
                }
            }
            PendingEvent::Spawn(npc) => {
                for script in self.listeners_of(EventKind::NpcSpawn, npc.id) {
                    self.with_script(script, |s, ctx| s.on_spawn(ctx, npc));
                }
            }
            PendingEvent::Teleport(npc) => {
                for script in self.listeners_of(EventKind::NpcTeleport, npc.id) {
                    self.with_script(script, |s, ctx| s.on_teleport(ctx, npc));
                }
            }
            PendingEvent::ZoneEnter { creature, zone_id } => {
                for script in self.listeners_of(EventKind::CreatureZoneEnter, zone_id) {
                    self.with_script(script, |s, ctx| s.on_enter_zone(ctx, creature, zone_id));
                }
            }
            PendingEvent::ZoneExit { creature, zone_id } => {
                for script in self.listeners_of(EventKind::CreatureZoneExit, zone_id) {
                    self.with_script(script, |s, ctx| s.on_exit_zone(ctx, creature, zone_id));
                }
            }
            PendingEvent::Login(player) => {
                for script in self.scripts.listeners.global(EventKind::PlayerLogin).to_vec() {
                    self.with_script(script, |s, ctx| s.on_enter_world(ctx, player));
                }
            }
            PendingEvent::ItemTalk { item, player } => {
                for script in self.listeners_of(EventKind::ItemTalk, item.id) {
                    // Java `notifyItemTalk`: `showResult(player, res)` without an NPC.
                    let res = self.with_script(script, |s, ctx| s.on_item_talk(ctx, item, player));
                    show_result(self, script, player, res.flatten().as_deref(), None);
                }
            }
            PendingEvent::ItemEvent { item, player, event } => {
                for script in self.listeners_of(EventKind::ItemBypassEvent, item.id) {
                    self.with_script(script, |s, ctx| s.on_item_event(ctx, item, player, &event));
                }
            }
            PendingEvent::SkillSee {
                npc,
                caster,
                skill,
                targets,
                is_summon,
            } => {
                for script in self.listeners_of(EventKind::NpcSkillSee, npc.id) {
                    self.with_script(script, |s, ctx| s.on_skill_see(ctx, npc, caster, skill, &targets, is_summon));
                }
            }
            PendingEvent::SpellFinished { npc, player, skill } => {
                for script in self.listeners_of(EventKind::NpcSkillFinished, npc.id) {
                    self.with_script(script, |s, ctx| s.on_spell_finished(ctx, npc, player, skill));
                }
            }
            PendingEvent::FactionCall {
                npc,
                caller,
                attacker,
                is_summon,
            } => {
                for script in self.listeners_of(EventKind::AttackableFactionCall, npc.id) {
                    self.with_script(script, |s, ctx| s.on_faction_call(ctx, npc, caller, attacker, is_summon));
                }
            }
            PendingEvent::CreatureSee { npc, creature } => {
                for script in self.listeners_of(EventKind::CreatureSee, npc.id) {
                    self.with_script(script, |s, ctx| s.on_creature_see(ctx, npc, creature));
                }
            }
            PendingEvent::EventReceived {
                event,
                sender,
                receiver,
                reference,
            } => {
                for script in self.listeners_of(EventKind::NpcEventReceived, receiver.id) {
                    // Java `notifyEventReceived` ignores the returned text.
                    self.with_script(script, |s, ctx| s.on_event_received(ctx, &event, sender, receiver, reference));
                }
            }
            PendingEvent::MoveFinished(npc) => {
                for script in self.listeners_of(EventKind::NpcMoveFinished, npc.id) {
                    self.with_script(script, |s, ctx| s.on_move_finished(ctx, npc));
                }
            }
            PendingEvent::NodeArrived(npc) => {
                for script in self.listeners_of(EventKind::NpcMoveNodeArrived, npc.id) {
                    self.with_script(script, |s, ctx| s.on_node_arrived(ctx, npc));
                }
            }
            PendingEvent::RouteFinished(npc) => {
                for script in self.listeners_of(EventKind::NpcMoveRouteFinished, npc.id) {
                    self.with_script(script, |s, ctx| s.on_route_finished(ctx, npc));
                }
            }
        }
    }

    /// Runs `f` with a script and a context on it. `None` for an unknown script.
    pub(crate) fn with_script<R>(
        &mut self,
        script: ScriptId,
        f: impl FnOnce(&dyn super::Script, &mut ScriptCtx<'_>) -> R,
    ) -> Option<R> {
        let s = self.scripts.script(script)?;
        let mut ctx = ScriptCtx::new(self, script);
        Some(f(&*s, &mut ctx))
    }

    // ------------------------------------------------------------------ synchronous notifies

    /// Java `Quest.notifyEvent(event, npc, player)`: `onEvent`, then `showResult`.
    pub(crate) fn script_notify_event(
        &mut self,
        script: ScriptId,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) {
        let res = self.with_script(script, |s, ctx| s.on_event(ctx, event, npc, player));
        if let Some(player) = player {
            show_result(self, script, player, res.flatten().as_deref(), npc);
        }
    }

    /// Java `Quest.notifyTalk(npc, player)`: the start condition page for a player without a
    /// state of the quest, else `onTalk`; then the NPC is the player's quest NPC and the result
    /// is shown.
    pub(crate) fn script_notify_talk(&mut self, script: ScriptId, npc: ScriptNpc, player: ObjectId) {
        let condition_html = self.start_condition_html(script, player);
        let name = self.scripts.name_of(script);
        let has_state = self
            .scripts
            .players
            .get(&player)
            .is_some_and(|q| q.contains(name));
        let res = match condition_html {
            Some(html) if !has_state => Some(html),
            _ => self
                .with_script(script, |s, ctx| s.on_talk(ctx, npc, player))
                .flatten(),
        };
        if let Some(p) = self.players.get_mut(&player) {
            p.talk.last_quest_npc_object = npc.object_id.0;
        }
        show_result(self, script, player, res.as_deref(), Some(npc));
    }

    /// Java `Quest.notifyFirstTalk(npc, player)`.
    fn script_notify_first_talk(&mut self, script: ScriptId, npc: ScriptNpc, player: ObjectId) {
        let res = self
            .with_script(script, |s, ctx| s.on_first_talk(ctx, npc, player))
            .flatten();
        show_result(self, script, player, res.as_deref(), Some(npc));
    }

    /// Java `getStartConditionHtml(player)`: the page of the first condition the player fails.
    pub(crate) fn start_condition_html(&self, script: ScriptId, player: ObjectId) -> Option<String> {
        self.scripts
            .start_conditions(script)
            .iter()
            .find(|(condition, _)| !condition(self, player))
            .map(|(_, html)| html.clone())
    }

    /// Java `canStartQuest(player)`.
    pub(crate) fn can_start_quest(&self, script: ScriptId, player: ObjectId) -> bool {
        self.scripts
            .start_conditions(script)
            .iter()
            .all(|(condition, _)| condition(self, player))
    }

    // ------------------------------------------------------------------ world hooks

    /// `NpcClick`: `ON_NPC_QUEST_START` makes the NPC the player's quest NPC, and an
    /// `ON_NPC_FIRST_TALK` listener takes the talk. `true` when a script took it.
    pub(crate) fn script_on_npc_click(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        let Some(n) = self.script_npc_of(npc) else {
            return false;
        };
        if self.scripts.listeners.has(EventKind::NpcQuestStart, n.id)
            && let Some(p) = self.players.get_mut(&player)
        {
            p.talk.last_quest_npc_object = npc.0;
        }
        self.script_first_talk(n, player)
    }

    /// `ChatLink` with `Chat 0`: an `ON_NPC_FIRST_TALK` listener takes the talk.
    pub(crate) fn script_on_chat_first_talk(&mut self, npc: ObjectId, player: ObjectId) -> bool {
        match self.script_npc_of(npc) {
            Some(n) => self.script_first_talk(n, player),
            None => false,
        }
    }

    fn script_first_talk(&mut self, npc: ScriptNpc, player: ObjectId) -> bool {
        if !self.scripts.listeners.has(EventKind::NpcFirstTalk, npc.id) {
            return false;
        }
        self.queue_script_event(PendingEvent::FirstTalk { npc, player });
        true
    }

    /// Java `Npc.onSpawn`: `ON_NPC_SPAWN`.
    pub(crate) fn script_on_npc_spawn(&mut self, npc: ObjectId) {
        if let Some(n) = self.has_npc_listener(EventKind::NpcSpawn, npc) {
            self.queue_script_event(PendingEvent::Spawn(n));
        }
    }

    /// Java `Npc.onTeleported`: `ON_NPC_TELEPORT`.
    pub(crate) fn script_on_npc_teleport(&mut self, npc: ObjectId) {
        if let Some(n) = self.has_npc_listener(EventKind::NpcTeleport, npc) {
            self.queue_script_event(PendingEvent::Teleport(n));
        }
    }

    /// Java `Attackable.addDamage`: `ON_ATTACKABLE_ATTACK` for an attack by a player or a
    /// summon.
    pub(crate) fn script_on_attackable_attack(
        &mut self,
        npc: ObjectId,
        attacker: ObjectId,
        damage: i32,
        skill: Option<SkillKey>,
    ) {
        let Some((player, is_summon)) = self.player_and_summon(attacker) else {
            return;
        };
        if let Some(n) = self.has_npc_listener(EventKind::AttackableAttack, npc) {
            self.queue_script_event(PendingEvent::Attack {
                npc: n,
                attacker: player,
                damage,
                is_summon,
                skill,
            });
        }
    }

    /// Java `Attackable.addDamageHate`: `ON_ATTACKABLE_AGGRO_RANGE_ENTER` for a player or summon
    /// that entered the aggro range (hate 0).
    pub(crate) fn script_on_aggro_range_enter(&mut self, npc: ObjectId, attacker: ObjectId) {
        let Some((player, is_summon)) = self.player_and_summon(attacker) else {
            return;
        };
        if let Some(n) = self.has_npc_listener(EventKind::AttackableAggroRangeEnter, npc) {
            self.queue_script_event(PendingEvent::AggroRangeEnter {
                npc: n,
                player,
                is_summon,
            });
        }
    }

    /// Java `Attackable.doDie`: `OnAttackableKill` `onKillDelay` after the death, for a killer
    /// that is a player or a summon.
    pub(crate) fn script_on_attackable_kill(&mut self, npc: ObjectId, killer: Option<ObjectId>) {
        let Some((player, is_summon)) = killer.and_then(|k| self.player_and_summon(k)) else {
            return;
        };
        let Some(n) = self.has_npc_listener(EventKind::AttackableKill, npc) else {
            return;
        };
        let delay = self
            .scripts
            .kill_delays
            .get(&npc)
            .copied()
            .unwrap_or(DEFAULT_ON_KILL_DELAY_MS);
        let ticks = TimerWheel::ms_to_ticks(u64::try_from(delay).unwrap_or(0));
        self.timers.schedule_at(
            self.tick + ticks,
            TimerKind::Script(ScriptTimer::Kill {
                npc: n,
                killer: player,
                is_summon,
            }),
        );
    }

    /// The delayed `OnAttackableKill`: every kill listener of the NPC's template.
    pub(crate) fn script_deliver_kill(&mut self, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        for script in self.listeners_of(EventKind::AttackableKill, npc.id) {
            self.with_script(script, |s, ctx| s.on_kill(ctx, npc, killer, is_summon));
        }
    }

    /// Java `Attackable.setOnKillDelay(delay)`.
    pub fn set_on_kill_delay(&mut self, npc: ObjectId, delay_ms: i32) {
        self.scripts.kill_delays.insert(npc, delay_ms);
    }

    /// Java `Playable.doDie`: "Notify Quest of Playable's death." `victim` is the dead player
    /// (or summon), `player` its owner.
    pub(crate) fn script_on_player_death(&mut self, victim: ObjectId, killer: Option<ObjectId>) {
        let Some(player) = self.acting_player(victim) else {
            return;
        };
        let names = self
            .scripts
            .players
            .get(&player)
            .map(|q| q.notify_of_death.clone())
            .unwrap_or_default();
        let killer = killer.unwrap_or(victim);
        for name in names {
            let Some(script) = self.scripts.by_name(&name) else {
                continue;
            };
            let qs = Qs { player, script };
            self.with_script(script, |s, ctx| s.on_death(ctx, killer, victim, qs));
        }
    }

    /// Java `ZoneType.revalidateInZone`: `ON_CREATURE_ZONE_ENTER`.
    pub(crate) fn script_on_zone_enter(&mut self, creature: ObjectId, zone_id: i32) {
        if self.scripts.listeners.has(EventKind::CreatureZoneEnter, zone_id) {
            self.queue_script_event(PendingEvent::ZoneEnter { creature, zone_id });
        }
    }

    /// Java `ZoneType.removeCharacter`: `ON_CREATURE_ZONE_EXIT`.
    pub(crate) fn script_on_zone_exit(&mut self, creature: ObjectId, zone_id: i32) {
        if self.scripts.listeners.has(EventKind::CreatureZoneExit, zone_id) {
            self.queue_script_event(PendingEvent::ZoneExit { creature, zone_id });
        }
    }

    /// Java `Player.onPlayerEnter`: `ON_PLAYER_LOGIN`.
    pub(crate) fn script_on_player_login(&mut self, player: ObjectId) {
        if !self.scripts.listeners.global(EventKind::PlayerLogin).is_empty() {
            self.queue_script_event(PendingEvent::Login(player));
        }
    }

    /// Java `AttackableAI` (`ON_NPC_HATE`, a synchronous `TerminateReturn`): `false` when a
    /// script forbids the monster to hate the player. Every listener runs; the first answer
    /// counts, because a later `TerminateReturn` without `override` does not replace it.
    pub(crate) fn script_npc_hate(&mut self, npc: ObjectId, target: ObjectId) -> bool {
        let Some(n) = self.has_npc_listener(EventKind::NpcHate, npc) else {
            return true;
        };
        let Some((player, is_summon)) = self.player_and_summon(target) else {
            return true;
        };
        let mut first: Option<bool> = None;
        for script in self.listeners_of(EventKind::NpcHate, n.id) {
            let via_can_see_me = self.scripts.listeners.is_can_see_me(script, n.id);
            let allowed = self.with_script(script, |s, ctx| {
                if via_can_see_me {
                    s.on_can_see_me(ctx, n, player)
                } else {
                    s.on_npc_hate(ctx, n, player, is_summon)
                }
            });
            if first.is_none() {
                first = allowed;
            }
        }
        first.unwrap_or(true)
    }

    /// Java `AttackableAI` / `Creature.doDie` (`ON_ATTACKABLE_FACTION_CALL`): `npc` is asked for
    /// help by `caller` against a player or summon.
    pub(crate) fn script_on_faction_call(&mut self, npc: ObjectId, caller: ObjectId, attacker: ObjectId) {
        let (Some(n), Some(c)) = (
            self.has_npc_listener(EventKind::AttackableFactionCall, npc),
            self.script_npc_of(caller),
        ) else {
            return;
        };
        let Some((player, is_summon)) = self.player_and_summon(attacker) else {
            return;
        };
        self.queue_script_event(PendingEvent::FactionCall {
            npc: n,
            caller: c,
            attacker: player,
            is_summon,
        });
    }

    /// Java `Creature.onMagicFinalizer` / `Weapon.castOnMagicSkill` (`ON_NPC_SKILL_SEE`): the
    /// NPC saw a player (or its summon) cast.
    pub(crate) fn script_on_skill_see(
        &mut self,
        npc: ObjectId,
        caster: ObjectId,
        skill: SkillKey,
        targets: Vec<ObjectId>,
    ) {
        let Some(n) = self.has_npc_listener(EventKind::NpcSkillSee, npc) else {
            return;
        };
        let Some((player, is_summon)) = self.player_and_summon(caster) else {
            return;
        };
        self.queue_script_event(PendingEvent::SkillSee {
            npc: n,
            caster: player,
            skill,
            targets,
            is_summon,
        });
    }

    /// Java `Npc.notifyQuestEventSkillFinished` (`ON_NPC_SKILL_FINISHED`).
    pub(crate) fn script_on_spell_finished(&mut self, npc: ObjectId, target: Option<ObjectId>, skill: SkillKey) {
        if let Some(n) = self.has_npc_listener(EventKind::NpcSkillFinished, npc) {
            let player = target.filter(|t| self.players.contains_key(t));
            self.queue_script_event(PendingEvent::SpellFinished { npc: n, player, skill });
        }
    }

    /// Java `Creature.updateSeenCreatures` (`ON_CREATURE_SEE`): an NPC with the listener sees a
    /// creature. Nothing calls it yet: `CreatureSeeTaskManager` is not ported (a follow-up of
    /// the script cards that use `addCreatureSeeId`).
    #[allow(dead_code)]
    pub(crate) fn script_on_creature_see(&mut self, npc: ObjectId, creature: ObjectId) {
        if let Some(n) = self.has_npc_listener(EventKind::CreatureSee, npc) {
            self.queue_script_event(PendingEvent::CreatureSee { npc: n, creature });
        }
    }

    /// Java `Npc.broadcastEvent` / `sendScriptEvent` (`ON_NPC_EVENT_RECEIVED`).
    pub fn script_send_event(&mut self, event: &str, sender: ObjectId, receiver: ObjectId, reference: Option<ObjectId>) {
        let (Some(s), Some(r)) = (
            self.script_npc_of(sender),
            self.has_npc_listener(EventKind::NpcEventReceived, receiver),
        ) else {
            return;
        };
        self.queue_script_event(PendingEvent::EventReceived {
            event: event.to_owned(),
            sender: s,
            receiver: r,
            reference,
        });
    }

    /// Java `CreatureAI` (`ON_NPC_MOVE_FINISHED`).
    pub(crate) fn script_on_move_finished(&mut self, npc: ObjectId) {
        if let Some(n) = self.has_npc_listener(EventKind::NpcMoveFinished, npc) {
            self.queue_script_event(PendingEvent::MoveFinished(n));
        }
    }

    /// Java `WalkingManager.onArrived` (`ON_NPC_MOVE_NODE_ARRIVED`).
    pub(crate) fn script_on_node_arrived(&mut self, npc: ObjectId) {
        if let Some(n) = self.has_npc_listener(EventKind::NpcMoveNodeArrived, npc) {
            self.queue_script_event(PendingEvent::NodeArrived(n));
        }
    }

    /// Java `WalkInfo.calculateNextNode` (`ON_NPC_MOVE_ROUTE_FINISHED`).
    pub(crate) fn script_on_route_finished(&mut self, npc: ObjectId) {
        if let Some(n) = self.has_npc_listener(EventKind::NpcMoveRouteFinished, npc) {
            self.queue_script_event(PendingEvent::RouteFinished(n));
        }
    }

    /// Java `Item.onBypassFeedback(player, command)`: `Script ...` of an item page.
    pub(crate) fn script_on_item_bypass(&mut self, player: ObjectId, item: ScriptItem, command: &str) {
        if !command.starts_with("Script") {
            return;
        }
        let quest_name = command.get(6..).unwrap_or("");
        // NOTE(java-quirk): `command.substring(6)` keeps the space after `Script`, so the
        // `indexOf(' ') > 0` test fails for `Script <name> <event>` and every such link is an
        // item talk; only `Script<name> <event>` reaches the item event.
        let event = match quest_name.find(' ') {
            Some(index) if index > 0 => Some(quest_name[index..].trim_matches(|c: char| c <= ' ').to_owned()),
            _ => None,
        };
        match event {
            Some(event) => {
                if self.scripts.listeners.has(EventKind::ItemBypassEvent, item.id) {
                    self.queue_script_event(PendingEvent::ItemEvent { item, player, event });
                }
            }
            None => {
                if self.scripts.listeners.has(EventKind::ItemTalk, item.id) {
                    self.queue_script_event(PendingEvent::ItemTalk { item, player });
                }
            }
        }
    }

    /// The script part of a player leaving: `Player.stopAllTasks` stops its quest timers; the
    /// quest states and radar markers go with the object.
    pub(crate) fn script_on_player_leave(&mut self, player: ObjectId) {
        self.script_cancel_timers_of(player);
        self.scripts.players.remove(&player);
        self.scripts.radar.remove(&player);
    }

    /// `Npc.onDecay`: `stopQuestTimers()`.
    pub(crate) fn script_on_npc_decay(&mut self, npc: ObjectId) {
        self.script_cancel_timers_of(npc);
        self.scripts.kill_delays.remove(&npc);
    }
}
