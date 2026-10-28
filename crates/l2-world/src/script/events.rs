//! The remaining `EventType`s of the scripts and the per-object listeners (S-API-04).
// Java: mechanics/events/{EventDispatcher,ListenersContainer,EventType}.java
// Java: mechanics/events/returns/TerminateReturn.java, mechanics/events/holders/**
//
// # Model
// Java's scripts register listeners in three places: on the global container (`Containers.Global()`,
// `Containers.Players()`: every object), on an NPC template (`@Id`, `setXxxId`), and on one
// object (`player.addListener(..)`, `ListenersContainer`). A [`ScriptEvent`] is one of the holders
// of `mechanics/events/holders`; [`World::script_fire`] hands it to the listeners of the object,
// of the NPC template and then the global ones, like `EventDispatcher.notifyEventImpl`.
//
// A listener is a script, not a closure (D-10: scripts hold no state). The default of
// [`Script::on_listener_event`] calls the typed hook of the event (`on_creature_attacked`, ...),
// so a global or template listener overrides the hook it wants. A per-object listener carries
// the `tag` its script gave it ([`ScriptCtx::add_listener`]), which is Java's
// `listener.getOwner()`: the script overrides `on_listener_event` when it needs the tag.
//
// # Order and return values
// Object listeners first (registration order), then the template's, then the global ones. The
// first answer counts; a later one replaces it only when it says `override`; an `abort` answer
// stops the notification, and a local `abort` also skips the global container (Java
// `notifyToListeners`).
//
// Events whose Java call is `notifyEventAsync` wait in the script queue ([`World::script_drain`]).

use l2_util::ObjectId;

use super::api::{Script, ScriptItem, ScriptNpc};
use super::ctx::ScriptCtx;
use super::notify::PendingEvent;
use super::setup::{EventKind, ScriptId};
use crate::effects::SkillKey;
use crate::world::World;

/// Java `TerminateReturn(terminate, override, abort)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TerminateReturn {
    /// Java `terminate()`: the caller stops what it was doing (the attack, the cast, the death).
    pub terminate: bool,
    /// Java `override()`: replaces the answer of an earlier listener.
    pub override_: bool,
    /// Java `abort()`: the other listeners are not told.
    pub abort: bool,
}

impl TerminateReturn {
    /// `new TerminateReturn(terminate, false, false)`, what every script in the datapack returns.
    pub const fn new(terminate: bool) -> Self {
        Self {
            terminate,
            override_: false,
            abort: false,
        }
    }
}

/// Java `TrapAction`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TrapAction {
    TrapTriggered,
    TrapDetected,
    TrapDisarmed,
}

/// The damage of `OnCreatureDamageDealt` / `OnCreatureDamageReceived`.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DamageInfo {
    pub attacker: Option<ObjectId>,
    pub target: ObjectId,
    pub damage: f64,
    pub skill: Option<SkillKey>,
    pub critical: bool,
    pub damage_over_time: bool,
}

/// A holder of `mechanics/events/holders`: what happened.
#[derive(Debug, Clone, PartialEq)]
pub enum ScriptEvent {
    /// `OnCreatureDeath` (`add_creature_death_id` or an object listener; container: the victim).
    /// Only `Playable.doDie` reads the answer.
    CreatureDeath {
        killer: Option<ObjectId>,
        victim: ObjectId,
    },
    /// `OnCreatureAttack`: `attacker` starts to attack `target` (container: the attacker).
    CreatureAttack { attacker: ObjectId, target: ObjectId },
    /// `OnCreatureAttacked`: the same, for the listeners of the target (container: the target).
    CreatureAttacked { attacker: ObjectId, target: ObjectId },
    /// `OnCreatureAttackAvoid` (container: the target that avoided).
    CreatureAttackAvoid {
        attacker: ObjectId,
        target: ObjectId,
        damage_over_time: bool,
    },
    /// `OnCreatureSkillUse` (container: the caster). `target` is the first target.
    CreatureSkillUse {
        caster: ObjectId,
        skill: SkillKey,
        simultaneously: bool,
        target: ObjectId,
    },
    /// `OnCreatureDamageDealt` (container: the attacker).
    CreatureDamageDealt(DamageInfo),
    /// `OnCreatureDamageReceived` (container: the target).
    CreatureDamageReceived(DamageInfo),
    /// `OnCreatureKilled` (container: the victim).
    CreatureKilled {
        attacker: Option<ObjectId>,
        target: ObjectId,
    },
    /// `OnPlayerSit`.
    PlayerSit { player: ObjectId },
    /// `OnPlayerLevelChanged`.
    PlayerLevelChanged {
        player: ObjectId,
        old_level: i32,
        new_level: i32,
    },
    /// `OnPlayerItemAdd`.
    PlayerItemAdd { player: ObjectId, item: ScriptItem },
    /// `OnPlayerItemPickup`.
    PlayerItemPickup { player: ObjectId, item: ScriptItem },
    /// `OnPlayerProfessionChange`: `class_id` is the id of the new template.
    PlayerProfessionChange {
        player: ObjectId,
        class_id: i32,
        is_sub_class: bool,
    },
    /// `OnPlayerSummonTalk`: a player clicked the summon.
    PlayerSummonTalk { summon: ObjectId },
    /// `OnPlayableExpChanged`.
    PlayableExpChanged {
        playable: ObjectId,
        old_exp: i64,
        new_exp: i64,
    },
    /// `OnNpcManorBypass` (container: the NPC).
    NpcManorBypass {
        player: ObjectId,
        npc: ScriptNpc,
        request: i32,
        manor_id: i32,
        next_period: bool,
    },
    /// `OnTrapAction` (container: the trap).
    TrapAction {
        trap: ScriptNpc,
        trigger: ObjectId,
        action: TrapAction,
    },
    /// `OnDailyReset`.
    DailyReset,
    /// `OnClanWarStart` (the two clan ids).
    ClanWarStart { clan1: i32, clan2: i32 },
    /// `OnClanWarFinish`.
    ClanWarFinish { clan1: i32, clan2: i32 },
    /// `OnCastleSiegeStart`.
    CastleSiegeStart { castle_id: i32 },
    /// `OnCastleSiegeFinish`.
    CastleSiegeFinish { castle_id: i32 },
    /// `OnCastleSiegeOwnerChange`.
    CastleSiegeOwnerChange { castle_id: i32 },
    /// `OnFortSiegeStart`.
    FortSiegeStart { fort_id: i32 },
    /// `OnFortSiegeFinish`.
    FortSiegeFinish { fort_id: i32 },
    /// `OnOlympiadMatchResult`: the `Participant`s as their character ids (`winner` is `None`
    /// where Java passes `null`) and the `CompetitionType` ordinal (0 = CLASSED, 1 = NON_CLASSED,
    /// 2 = TEAMS).
    OlympiadMatchResult {
        winner: Option<i32>,
        loser: i32,
        competition_type: i32,
    },
}

impl ScriptEvent {
    /// The `EventType` of the holder.
    pub fn kind(&self) -> EventKind {
        match self {
            Self::CreatureDeath { .. } => EventKind::CreatureDeath,
            Self::CreatureAttack { .. } => EventKind::CreatureAttack,
            Self::CreatureAttacked { .. } => EventKind::CreatureAttacked,
            Self::CreatureAttackAvoid { .. } => EventKind::CreatureAttackAvoid,
            Self::CreatureSkillUse { .. } => EventKind::CreatureSkillUse,
            Self::CreatureDamageDealt(_) => EventKind::CreatureDamageDealt,
            Self::CreatureDamageReceived(_) => EventKind::CreatureDamageReceived,
            Self::CreatureKilled { .. } => EventKind::CreatureKilled,
            Self::PlayerSit { .. } => EventKind::PlayerSit,
            Self::PlayerLevelChanged { .. } => EventKind::PlayerLevelChanged,
            Self::PlayerItemAdd { .. } => EventKind::PlayerItemAdd,
            Self::PlayerItemPickup { .. } => EventKind::PlayerItemPickup,
            Self::PlayerProfessionChange { .. } => EventKind::PlayerProfessionChange,
            Self::PlayerSummonTalk { .. } => EventKind::PlayerSummonTalk,
            Self::PlayableExpChanged { .. } => EventKind::PlayableExpChanged,
            Self::NpcManorBypass { .. } => EventKind::NpcManorBypass,
            Self::TrapAction { .. } => EventKind::TrapAction,
            Self::DailyReset => EventKind::DailyReset,
            Self::ClanWarStart { .. } => EventKind::ClanWarStart,
            Self::ClanWarFinish { .. } => EventKind::ClanWarFinish,
            Self::CastleSiegeStart { .. } => EventKind::CastleSiegeStart,
            Self::CastleSiegeFinish { .. } => EventKind::CastleSiegeFinish,
            Self::CastleSiegeOwnerChange { .. } => EventKind::CastleSiegeOwnerChange,
            Self::FortSiegeStart { .. } => EventKind::FortSiegeStart,
            Self::FortSiegeFinish { .. } => EventKind::FortSiegeFinish,
            Self::OlympiadMatchResult { .. } => EventKind::OlympiadMatchResult,
        }
    }

    /// Calls the typed hook of the event on `script` (what [`Script::on_listener_event`] does by
    /// default).
    pub fn call_hook(
        &self,
        script: &(impl Script + ?Sized),
        ctx: &mut ScriptCtx<'_>,
    ) -> Option<TerminateReturn> {
        match self {
            Self::CreatureDeath { killer, victim } => {
                script.on_creature_death_terminate(ctx, *killer, *victim)
            }
            Self::CreatureAttack { attacker, target } => script.on_creature_attack(ctx, *attacker, *target),
            Self::CreatureAttacked { attacker, target } => {
                script.on_creature_attacked(ctx, *attacker, *target)
            }
            Self::CreatureAttackAvoid {
                attacker,
                target,
                damage_over_time,
            } => {
                script.on_creature_attack_avoid(ctx, *attacker, *target, *damage_over_time);
                None
            }
            Self::CreatureSkillUse {
                caster,
                skill,
                simultaneously,
                target,
            } => script.on_creature_skill_use(ctx, *caster, *skill, *simultaneously, *target),
            Self::CreatureDamageDealt(info) => {
                script.on_creature_damage_dealt(ctx, info);
                None
            }
            Self::CreatureDamageReceived(info) => {
                script.on_creature_damage_received(ctx, info);
                None
            }
            Self::CreatureKilled { attacker, target } => script.on_creature_killed(ctx, *attacker, *target),
            Self::PlayerSit { player } => {
                script.on_player_sit(ctx, *player);
                None
            }
            Self::PlayerLevelChanged {
                player,
                old_level,
                new_level,
            } => {
                script.on_player_level_changed(ctx, *player, *old_level, *new_level);
                None
            }
            Self::PlayerItemAdd { player, item } => {
                script.on_player_item_add(ctx, *player, *item);
                None
            }
            Self::PlayerItemPickup { player, item } => {
                script.on_player_item_pickup(ctx, *player, *item);
                None
            }
            Self::PlayerProfessionChange {
                player,
                class_id,
                is_sub_class,
            } => {
                script.on_player_profession_change(ctx, *player, *class_id, *is_sub_class);
                None
            }
            Self::PlayerSummonTalk { summon } => {
                script.on_player_summon_talk(ctx, *summon);
                None
            }
            Self::PlayableExpChanged {
                playable,
                old_exp,
                new_exp,
            } => script.on_playable_exp_changed(ctx, *playable, *old_exp, *new_exp),
            Self::NpcManorBypass {
                player,
                npc,
                request,
                manor_id,
                next_period,
            } => {
                script.on_npc_manor_bypass(ctx, *player, *npc, *request, *manor_id, *next_period);
                None
            }
            Self::TrapAction {
                trap,
                trigger,
                action,
            } => {
                script.on_trap_action(ctx, *trap, *trigger, *action);
                None
            }
            Self::DailyReset => {
                script.on_daily_reset(ctx);
                None
            }
            Self::ClanWarStart { clan1, clan2 } => {
                script.on_clan_war_start(ctx, *clan1, *clan2);
                None
            }
            Self::ClanWarFinish { clan1, clan2 } => {
                script.on_clan_war_finish(ctx, *clan1, *clan2);
                None
            }
            Self::CastleSiegeStart { castle_id } => {
                script.on_castle_siege_start(ctx, *castle_id);
                None
            }
            Self::CastleSiegeFinish { castle_id } => {
                script.on_castle_siege_finish(ctx, *castle_id);
                None
            }
            Self::CastleSiegeOwnerChange { castle_id } => {
                script.on_castle_siege_owner_change(ctx, *castle_id);
                None
            }
            Self::FortSiegeStart { fort_id } => {
                script.on_fort_siege_start(ctx, *fort_id);
                None
            }
            Self::FortSiegeFinish { fort_id } => {
                script.on_fort_siege_finish(ctx, *fort_id);
                None
            }
            Self::OlympiadMatchResult {
                winner,
                loser,
                competition_type,
            } => {
                script.on_olympiad_match_result(ctx, *winner, *loser, *competition_type);
                None
            }
        }
    }
}

/// One listener added to one object with [`ScriptCtx::add_listener`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ObjectListener {
    pub(crate) script: ScriptId,
    /// Java `listener.getOwner()`, as the script chose it.
    pub(crate) tag: i64,
}

/// The listeners of single objects (Java `ListenersContainer` of a player or an NPC).
#[derive(Debug, Default)]
pub(crate) struct ObjectListeners {
    by_object: std::collections::HashMap<(ObjectId, EventKind), Vec<ObjectListener>>,
}

impl ObjectListeners {
    fn has(&self, object: ObjectId, kind: EventKind) -> bool {
        self.by_object.contains_key(&(object, kind))
    }

    /// The number of listeners of the object (tests).
    pub(crate) fn count(&self, object: ObjectId) -> usize {
        self.by_object
            .iter()
            .filter(|((o, _), _)| *o == object)
            .map(|(_, v)| v.len())
            .sum()
    }
}

impl<'w> ScriptCtx<'w> {
    /// Java `object.addListener(new ConsumerEventListener(object, kind, callback, owner))`: the
    /// running script hears `kind` of `object`. `tag` is Java's `owner` (what
    /// [`ScriptCtx::remove_listener_if`] matches and what `on_listener_event` receives). Java
    /// keeps a listener only once per `(container, listener)`; two calls make two listeners
    /// here as well (a different callback object in Java).
    pub fn add_listener(&mut self, object: ObjectId, kind: EventKind, tag: i64) {
        let script = self.script;
        self.world
            .scripts
            .object_listeners
            .by_object
            .entry((object, kind))
            .or_default()
            .push(ObjectListener { script, tag });
    }

    /// Java `object.removeListenerIf(kind, listener -> listener.getOwner() == tag)`, for the
    /// listeners of the running script. Returns how many were removed.
    pub fn remove_listener_if(&mut self, object: ObjectId, kind: EventKind, tag: i64) -> usize {
        let script = self.script;
        let listeners = &mut self.world.scripts.object_listeners;
        let Some(list) = listeners.by_object.get_mut(&(object, kind)) else {
            return 0;
        };
        let before = list.len();
        list.retain(|l| !(l.script == script && l.tag == tag));
        let removed = before - list.len();
        if list.is_empty() {
            listeners.by_object.remove(&(object, kind));
        }
        removed
    }

    /// Java `object.removeListeners(kind)` ... of the running script: all of its listeners of
    /// `kind` on the object.
    pub fn remove_listeners(&mut self, object: ObjectId, kind: EventKind) -> usize {
        let script = self.script;
        let listeners = &mut self.world.scripts.object_listeners;
        let Some(list) = listeners.by_object.get_mut(&(object, kind)) else {
            return 0;
        };
        let before = list.len();
        list.retain(|l| l.script != script);
        let removed = before - list.len();
        if list.is_empty() {
            listeners.by_object.remove(&(object, kind));
        }
        removed
    }

    /// Whether the running script has a listener of `kind` on the object with this tag.
    pub fn has_listener(&self, object: ObjectId, kind: EventKind, tag: i64) -> bool {
        self.world
            .scripts
            .object_listeners
            .by_object
            .get(&(object, kind))
            .is_some_and(|l| l.iter().any(|l| l.script == self.script && l.tag == tag))
    }
}

impl World {
    /// Java `EventDispatcher.hasListener(type, container)`: the object's, the NPC template's or a
    /// global listener exists.
    pub(crate) fn script_has_listener(&self, kind: EventKind, container: Option<ObjectId>) -> bool {
        let npc_id = container.and_then(|c| self.npcs.get(&c)).map(|n| n.npc_id);
        !self.scripts.listeners.global(kind).is_empty()
            || container.is_some_and(|c| self.scripts.object_listeners.has(c, kind))
            || npc_id.is_some_and(|id| self.scripts.listeners.has(kind, id))
    }

    /// Java `EventDispatcher.notifyEvent(event, container, TerminateReturn.class)`: tells the
    /// listeners at once and returns the answer. `None` without a listener or an answer.
    pub(crate) fn script_fire(
        &mut self,
        event: ScriptEvent,
        container: Option<ObjectId>,
    ) -> Option<TerminateReturn> {
        let kind = event.kind();
        if !self.script_has_listener(kind, container) {
            return None;
        }
        let npc_id = container.and_then(|c| self.npcs.get(&c)).map(|n| n.npc_id);
        let mut local: Vec<(ScriptId, Option<i64>)> = Vec::new();
        if let Some(c) = container
            && let Some(list) = self.scripts.object_listeners.by_object.get(&(c, kind))
        {
            local.extend(list.iter().map(|l| (l.script, Some(l.tag))));
        }
        if let Some(id) = npc_id {
            local.extend(self.scripts.listeners.of(kind, id).iter().map(|s| (*s, None)));
        }
        let global: Vec<(ScriptId, Option<i64>)> = self
            .scripts
            .listeners
            .global(kind)
            .iter()
            .map(|s| (*s, None))
            .collect();
        let mut callback = self.notify_listeners(&local, &event, None);
        if callback.is_none_or(|c| !c.abort) {
            callback = self.notify_listeners(&global, &event, callback);
        }
        callback
    }

    /// Java `notifyToListeners`.
    fn notify_listeners(
        &mut self,
        listeners: &[(ScriptId, Option<i64>)],
        event: &ScriptEvent,
        start: Option<TerminateReturn>,
    ) -> Option<TerminateReturn> {
        let mut callback = start;
        for &(script, tag) in listeners {
            let Some(rb) = self
                .with_script(script, |s, ctx| s.on_listener_event(ctx, tag, event))
                .flatten()
            else {
                continue;
            };
            if callback.is_none_or(|_| rb.override_) {
                callback = Some(rb);
            } else if rb.abort {
                break;
            }
        }
        callback
    }

    /// Java `EventDispatcher.notifyEventAsync(event, container)`: the listeners hear it after
    /// the command that fired it.
    pub(crate) fn script_fire_async(&mut self, event: ScriptEvent, container: Option<ObjectId>) {
        if self.script_has_listener(event.kind(), container) {
            self.scripts
                .pending
                .push_back(PendingEvent::Fire { event, container });
        }
    }

    /// The queued part of [`World::script_fire_async`].
    pub(crate) fn dispatch_fired_event(&mut self, event: ScriptEvent, container: Option<ObjectId>) {
        self.script_fire(event, container);
    }

    /// Forgets the listeners added to an object that left the world.
    pub(crate) fn script_forget_listeners(&mut self, object: ObjectId) {
        self.scripts
            .object_listeners
            .by_object
            .retain(|(o, _), _| *o != object);
    }

    /// How many listeners are added to the object (tests).
    pub fn script_object_listener_count(&self, object: ObjectId) -> usize {
        self.scripts.object_listeners.count(object)
    }
}
