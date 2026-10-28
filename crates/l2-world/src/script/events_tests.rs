//! Unit tests of the S-API-04 engine parts that need no world data: the new events, the
//! per-object listeners, the global variable re-read, `Script::enabled` and the NPC extras.

use std::sync::{Arc, Mutex};

use l2_util::{ObjectId, WorldRng};

use super::events::{DamageInfo, TerminateReturn, TrapAction};
use super::{EventKind, Script, ScriptCtx, ScriptEvent, ScriptItem, ScriptNpc, ScriptRegistry, ScriptSetup};
use crate::effects::SkillKey;
use crate::outbox::{LsEvent, Outbox, PersistCommand};
use crate::world::World;

type Log = Arc<Mutex<Vec<String>>>;

fn lines(log: &Log) -> Vec<String> {
    log.lock().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Keeps the global variable re-read requests.
#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<(u64, Vec<String>)>>>);

impl Outbox for Capture {
    fn persist(&mut self, command: PersistCommand) {
        if let PersistCommand::GlobalVariablesRereadRequest { token, names } = command {
            self.0
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .push((token, names));
        }
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

fn world(capture: &Capture) -> World {
    World::new(WorldRng::from_seed(1), Box::new(capture.clone()))
}

fn push(log: &Log, text: String) {
    log.lock().unwrap_or_else(|e| e.into_inner()).push(text);
}

/// Listens to every new global event and logs the hook that ran.
struct Recorder {
    log: Log,
    terminate: bool,
}

impl Script for Recorder {
    fn name(&self) -> &'static str {
        "Recorder"
    }
    fn path(&self) -> &'static str {
        "ai/others/Recorder"
    }
    fn setup(&self, s: &mut ScriptSetup) {
        s.set_on_creature_attack();
        s.set_on_creature_attacked();
        s.set_on_creature_attack_avoid();
        s.set_on_creature_skill_use();
        s.set_on_creature_damage_dealt();
        s.set_on_creature_damage_received();
        s.set_on_creature_killed();
        s.set_on_player_sit();
        s.set_on_player_level_changed();
        s.set_on_player_item_add();
        s.set_on_player_item_pickup();
        s.set_on_player_profession_change();
        s.set_on_player_summon_talk();
        s.set_on_playable_exp_changed();
        s.set_on_trap_action();
        s.set_on_daily_reset();
        s.set_on_clan_war_start();
        s.set_on_clan_war_finish();
        s.set_on_castle_siege_start();
        s.set_on_castle_siege_finish();
        s.set_on_castle_siege_owner_change();
        s.set_on_fort_siege_start();
        s.set_on_fort_siege_finish();
        s.set_on_olympiad_match_result();
        s.add_npc_manor_bypass_id(&[35644]);
    }
    fn on_creature_attack(&self, _: &mut ScriptCtx<'_>, a: ObjectId, t: ObjectId) -> Option<TerminateReturn> {
        push(&self.log, format!("attack {} {}", a.0, t.0));
        Some(TerminateReturn::new(self.terminate))
    }
    fn on_creature_attacked(
        &self,
        _: &mut ScriptCtx<'_>,
        a: ObjectId,
        t: ObjectId,
    ) -> Option<TerminateReturn> {
        push(&self.log, format!("attacked {} {}", a.0, t.0));
        Some(TerminateReturn::new(self.terminate))
    }
    fn on_creature_attack_avoid(&self, _: &mut ScriptCtx<'_>, a: ObjectId, t: ObjectId, dot: bool) {
        push(&self.log, format!("avoid {} {} {dot}", a.0, t.0));
    }
    fn on_creature_skill_use(
        &self,
        _: &mut ScriptCtx<'_>,
        caster: ObjectId,
        skill: SkillKey,
        simultaneously: bool,
        target: ObjectId,
    ) -> Option<TerminateReturn> {
        push(
            &self.log,
            format!("skill {} {} {simultaneously} {}", caster.0, skill.id, target.0),
        );
        Some(TerminateReturn::new(self.terminate))
    }
    fn on_creature_damage_dealt(&self, _: &mut ScriptCtx<'_>, info: &DamageInfo) {
        push(&self.log, format!("dealt {} {}", info.target.0, info.damage));
    }
    fn on_creature_damage_received(&self, _: &mut ScriptCtx<'_>, info: &DamageInfo) {
        push(&self.log, format!("received {} {}", info.target.0, info.critical));
    }
    fn on_creature_killed(
        &self,
        _: &mut ScriptCtx<'_>,
        a: Option<ObjectId>,
        t: ObjectId,
    ) -> Option<TerminateReturn> {
        push(&self.log, format!("killed {:?} {}", a.map(|o| o.0), t.0));
        Some(TerminateReturn::new(self.terminate))
    }
    fn on_player_sit(&self, _: &mut ScriptCtx<'_>, p: ObjectId) {
        push(&self.log, format!("sit {}", p.0));
    }
    fn on_player_level_changed(&self, _: &mut ScriptCtx<'_>, p: ObjectId, old: i32, new: i32) {
        push(&self.log, format!("level {} {old} {new}", p.0));
    }
    fn on_player_item_add(&self, _: &mut ScriptCtx<'_>, p: ObjectId, item: ScriptItem) {
        push(&self.log, format!("itemadd {} {}", p.0, item.id));
    }
    fn on_player_item_pickup(&self, _: &mut ScriptCtx<'_>, p: ObjectId, item: ScriptItem) {
        push(&self.log, format!("pickup {} {}", p.0, item.id));
    }
    fn on_player_profession_change(&self, _: &mut ScriptCtx<'_>, p: ObjectId, class_id: i32, sub: bool) {
        push(&self.log, format!("profession {} {class_id} {sub}", p.0));
    }
    fn on_player_summon_talk(&self, _: &mut ScriptCtx<'_>, s: ObjectId) {
        push(&self.log, format!("summontalk {}", s.0));
    }
    fn on_playable_exp_changed(
        &self,
        _: &mut ScriptCtx<'_>,
        p: ObjectId,
        old: i64,
        new: i64,
    ) -> Option<TerminateReturn> {
        push(&self.log, format!("exp {} {old} {new}", p.0));
        Some(TerminateReturn::new(self.terminate))
    }
    fn on_npc_manor_bypass(
        &self,
        _: &mut ScriptCtx<'_>,
        p: ObjectId,
        npc: ScriptNpc,
        request: i32,
        manor: i32,
        next: bool,
    ) {
        push(
            &self.log,
            format!("manor {} {} {request} {manor} {next}", p.0, npc.id),
        );
    }
    fn on_trap_action(&self, _: &mut ScriptCtx<'_>, trap: ScriptNpc, trigger: ObjectId, action: TrapAction) {
        push(&self.log, format!("trap {} {} {action:?}", trap.id, trigger.0));
    }
    fn on_daily_reset(&self, _: &mut ScriptCtx<'_>) {
        push(&self.log, "daily".to_owned());
    }
    fn on_clan_war_start(&self, _: &mut ScriptCtx<'_>, a: i32, b: i32) {
        push(&self.log, format!("warstart {a} {b}"));
    }
    fn on_clan_war_finish(&self, _: &mut ScriptCtx<'_>, a: i32, b: i32) {
        push(&self.log, format!("warfinish {a} {b}"));
    }
    fn on_castle_siege_start(&self, _: &mut ScriptCtx<'_>, c: i32) {
        push(&self.log, format!("castlestart {c}"));
    }
    fn on_castle_siege_finish(&self, _: &mut ScriptCtx<'_>, c: i32) {
        push(&self.log, format!("castlefinish {c}"));
    }
    fn on_castle_siege_owner_change(&self, _: &mut ScriptCtx<'_>, c: i32) {
        push(&self.log, format!("castleowner {c}"));
    }
    fn on_fort_siege_start(&self, _: &mut ScriptCtx<'_>, f: i32) {
        push(&self.log, format!("fortstart {f}"));
    }
    fn on_fort_siege_finish(&self, _: &mut ScriptCtx<'_>, f: i32) {
        push(&self.log, format!("fortfinish {f}"));
    }
    fn on_olympiad_match_result(&self, _: &mut ScriptCtx<'_>, w: Option<i32>, l: i32, t: i32) {
        push(&self.log, format!("olympiad {w:?} {l} {t}"));
    }
}

fn recorder_world(terminate: bool) -> (World, Log) {
    let log = Log::default();
    let mut w = world(&Capture::default());
    let mut registry = ScriptRegistry::new();
    registry.add(Recorder {
        log: Arc::clone(&log),
        terminate,
    });
    w.install_scripts(registry);
    (w, log)
}

const A: ObjectId = ObjectId(268_480_001);
const B: ObjectId = ObjectId(268_480_002);

fn damage() -> DamageInfo {
    DamageInfo {
        attacker: Some(A),
        target: B,
        damage: 12.0,
        skill: None,
        critical: true,
        damage_over_time: false,
    }
}

/// Every new event reaches the typed hook of the global listener with its payload.
#[test]
fn every_event_type_reaches_its_hook() {
    let (mut w, log) = recorder_world(false);
    let skill = SkillKey::new(1177, 3);
    let item = ScriptItem {
        object_id: ObjectId(5),
        id: 57,
    };
    let npc = ScriptNpc {
        object_id: B,
        id: 35644,
    };
    let events = [
        ScriptEvent::CreatureAttack {
            attacker: A,
            target: B,
        },
        ScriptEvent::CreatureAttacked {
            attacker: A,
            target: B,
        },
        ScriptEvent::CreatureAttackAvoid {
            attacker: A,
            target: B,
            damage_over_time: true,
        },
        ScriptEvent::CreatureSkillUse {
            caster: A,
            skill,
            simultaneously: false,
            target: B,
        },
        ScriptEvent::CreatureDamageDealt(damage()),
        ScriptEvent::CreatureDamageReceived(damage()),
        ScriptEvent::CreatureKilled {
            attacker: Some(A),
            target: B,
        },
        ScriptEvent::PlayerSit { player: A },
        ScriptEvent::PlayerLevelChanged {
            player: A,
            old_level: 10,
            new_level: 11,
        },
        ScriptEvent::PlayerItemAdd { player: A, item },
        ScriptEvent::PlayerItemPickup { player: A, item },
        ScriptEvent::PlayerProfessionChange {
            player: A,
            class_id: 90,
            is_sub_class: false,
        },
        ScriptEvent::PlayerSummonTalk { summon: B },
        ScriptEvent::PlayableExpChanged {
            playable: A,
            old_exp: 1,
            new_exp: 5,
        },
        ScriptEvent::TrapAction {
            trap: npc,
            trigger: A,
            action: TrapAction::TrapDetected,
        },
        ScriptEvent::DailyReset,
        ScriptEvent::ClanWarStart { clan1: 1, clan2: 2 },
        ScriptEvent::ClanWarFinish { clan1: 1, clan2: 2 },
        ScriptEvent::CastleSiegeStart { castle_id: 3 },
        ScriptEvent::CastleSiegeFinish { castle_id: 3 },
        ScriptEvent::CastleSiegeOwnerChange { castle_id: 3 },
        ScriptEvent::FortSiegeStart { fort_id: 101 },
        ScriptEvent::FortSiegeFinish { fort_id: 101 },
        ScriptEvent::OlympiadMatchResult {
            winner: None,
            loser: 7,
            competition_type: 1,
        },
    ];
    for event in events {
        let answer = w.script_fire(event, None);
        // Only the events whose Java call reads a `TerminateReturn` hand the answer back.
        assert!(answer.is_none_or(|a| !a.terminate));
    }
    assert_eq!(
        lines(&log),
        [
            "attack 268480001 268480002",
            "attacked 268480001 268480002",
            "avoid 268480001 268480002 true",
            "skill 268480001 1177 false 268480002",
            "dealt 268480002 12",
            "received 268480002 true",
            "killed Some(268480001) 268480002",
            "sit 268480001",
            "level 268480001 10 11",
            "itemadd 268480001 57",
            "pickup 268480001 57",
            "profession 268480001 90 false",
            "summontalk 268480002",
            "exp 268480001 1 5",
            "trap 35644 268480001 TrapDetected",
            "daily",
            "warstart 1 2",
            "warfinish 1 2",
            "castlestart 3",
            "castlefinish 3",
            "castleowner 3",
            "fortstart 101",
            "fortfinish 101",
            "olympiad None 7 1",
        ]
    );
}

/// The queued events (`notifyEventAsync`) wait for the drain.
#[test]
fn async_events_run_after_the_drain() {
    let (mut w, log) = recorder_world(false);
    w.script_fire_async(ScriptEvent::PlayerSit { player: A }, Some(A));
    w.script_fire_async(ScriptEvent::DailyReset, None);
    assert!(lines(&log).is_empty());
    w.script_drain();
    assert_eq!(lines(&log), ["sit 268480001", "daily"]);
}

/// The `TerminateReturn` of the first listener comes back to the code that fired the event.
#[test]
fn a_terminate_answer_reaches_the_caller() {
    let (mut w, _log) = recorder_world(true);
    for event in [
        ScriptEvent::CreatureAttack {
            attacker: A,
            target: B,
        },
        ScriptEvent::CreatureAttacked {
            attacker: A,
            target: B,
        },
        ScriptEvent::CreatureSkillUse {
            caster: A,
            skill: SkillKey::new(1, 1),
            simultaneously: false,
            target: B,
        },
        ScriptEvent::CreatureKilled {
            attacker: None,
            target: B,
        },
        ScriptEvent::PlayableExpChanged {
            playable: A,
            old_exp: 0,
            new_exp: 1,
        },
    ] {
        assert!(w.script_fire(event, Some(A)).is_some_and(|r| r.terminate));
    }
}

/// An object with no listener of the kind and no global one costs nothing and answers nothing.
#[test]
fn no_listener_no_answer() {
    let mut w = world(&Capture::default());
    assert_eq!(w.script_fire(ScriptEvent::DailyReset, None), None);
    assert!(!w.script_has_listener(EventKind::PlayerSit, Some(A)));
}

/// Java `ON_NPC_MANOR_BYPASS` is registered by NPC id: only that NPC's bypass is heard.
#[test]
fn the_manor_bypass_is_heard_by_npc_id_only() {
    let log = Log::default();
    let mut w = world(&Capture::default());
    let mut registry = ScriptRegistry::new();
    registry.add(Recorder {
        log: Arc::clone(&log),
        terminate: false,
    });
    w.install_scripts(registry);
    // The NPC objects: `has_templates` is false without NPC data, so every id is registered.
    assert!(w.scripts.listeners.has(EventKind::NpcManorBypass, 35644));
    assert!(!w.scripts.listeners.has(EventKind::NpcManorBypass, 1));
}

/// A listener of one object hears only that object, carries its tag, and can be removed by tag.
struct Watcher {
    log: Log,
}

impl Script for Watcher {
    fn name(&self) -> &'static str {
        "Watcher"
    }
    fn path(&self) -> &'static str {
        "ai/others/Watcher"
    }
    fn setup(&self, _s: &mut ScriptSetup) {}
    fn on_listener_event(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        tag: Option<i64>,
        event: &ScriptEvent,
    ) -> Option<TerminateReturn> {
        push(&self.log, format!("{tag:?} {:?}", event.kind()));
        // Java's `Tutorial` answers `TerminateReturn(true, false, false)` to stop the attack.
        Some(TerminateReturn::new(tag == Some(7)))
    }
}

#[test]
fn object_listeners_hear_only_their_object_and_can_be_removed_by_tag() {
    let log = Log::default();
    let mut w = world(&Capture::default());
    let mut registry = ScriptRegistry::new();
    registry.add(Watcher {
        log: Arc::clone(&log),
    });
    w.install_scripts(registry);
    w.run_as_script("Watcher", |ctx| {
        ctx.add_listener(A, EventKind::PlayerSit, 7);
        ctx.add_listener(A, EventKind::CreatureAttacked, 8);
        assert!(ctx.has_listener(A, EventKind::PlayerSit, 7));
    });
    assert_eq!(w.script_object_listener_count(A), 2);
    // The other object hears nothing.
    assert_eq!(w.script_fire(ScriptEvent::PlayerSit { player: B }, Some(B)), None);
    assert!(lines(&log).is_empty());
    // The tagged one answers terminate.
    assert!(
        w.script_fire(ScriptEvent::PlayerSit { player: A }, Some(A))
            .is_some_and(|r| r.terminate)
    );
    assert_eq!(lines(&log), ["Some(7) PlayerSit"]);
    // `removeListenerIf(owner == tag)`: a wrong tag removes nothing.
    w.run_as_script("Watcher", |ctx| {
        assert_eq!(ctx.remove_listener_if(A, EventKind::PlayerSit, 9), 0);
        assert_eq!(ctx.remove_listener_if(A, EventKind::PlayerSit, 7), 1);
    });
    assert_eq!(w.script_fire(ScriptEvent::PlayerSit { player: A }, Some(A)), None);
    // The object leaving the world takes the rest.
    assert_eq!(w.script_object_listener_count(A), 1);
    w.script_forget_listeners(A);
    assert_eq!(w.script_object_listener_count(A), 0);
}

/// Java `notifyToListeners`: the first answer counts, a later one only with `override`, and an
/// `abort` answer stops the rest.
struct Answers {
    name: &'static str,
    answer: TerminateReturn,
    log: Log,
}

impl Script for Answers {
    fn name(&self) -> &'static str {
        self.name
    }
    fn path(&self) -> &'static str {
        "ai/others/Answers"
    }
    fn setup(&self, s: &mut ScriptSetup) {
        s.set_on_creature_attacked();
    }
    fn on_creature_attacked(
        &self,
        _: &mut ScriptCtx<'_>,
        _: ObjectId,
        _: ObjectId,
    ) -> Option<TerminateReturn> {
        push(&self.log, self.name.to_owned());
        Some(self.answer)
    }
}

#[test]
fn answers_follow_java_override_and_abort() {
    let log = Log::default();
    let mut w = world(&Capture::default());
    let mut registry = ScriptRegistry::new();
    let answer = |terminate, override_, abort| TerminateReturn {
        terminate,
        override_,
        abort,
    };
    for (name, a) in [
        ("first", answer(false, false, false)),
        // Does not override the first answer, aborts the rest.
        ("second", answer(true, false, true)),
        ("third", answer(true, true, false)),
    ] {
        registry.add(Answers {
            name,
            answer: a,
            log: Arc::clone(&log),
        });
    }
    w.install_scripts(registry);
    let r = w.script_fire(
        ScriptEvent::CreatureAttacked {
            attacker: A,
            target: B,
        },
        Some(B),
    );
    assert_eq!(lines(&log), ["first", "second"]);
    assert_eq!(r, Some(answer(false, false, false)));
}

/// D-50: with the switch off a re-read goes out, the answer replaces the copy (a row the Java
/// GS changed, one it deleted) and only then the continuation runs.
struct Decider {
    log: Log,
}

impl Script for Decider {
    fn name(&self) -> &'static str {
        "Decider"
    }
    fn path(&self) -> &'static str {
        "ai/others/Decider"
    }
    fn setup(&self, _s: &mut ScriptSetup) {}
    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        push(
            &self.log,
            format!(
                "{event} {} {}",
                ctx.global_variables().get_long("Respawn", 0),
                ctx.global_variables().has_variable("Gone")
            ),
        );
        None
    }
    fn on_global_variables_reread_failed(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) {
        push(&self.log, format!("failed {event}"));
    }
}

fn decider_world(cycle: bool) -> (World, Log, Capture) {
    let log = Log::default();
    let capture = Capture::default();
    let mut w = world(&capture);
    w.set_script_settings(super::ScriptSettings {
        cycle_database_writes: cycle,
        ..super::ScriptSettings::default()
    });
    w.set_global_variables(vec![
        ("Respawn".to_owned(), Some("100".to_owned())),
        ("Gone".to_owned(), Some("1".to_owned())),
    ]);
    let mut registry = ScriptRegistry::new();
    registry.add(Decider {
        log: Arc::clone(&log),
    });
    w.install_scripts(registry);
    (w, log, capture)
}

#[test]
fn the_reread_replaces_the_copy_before_the_script_decides() {
    let (mut w, log, capture) = decider_world(false);
    w.run_as_script("Decider", |ctx| {
        ctx.reread_global_variables(&["Respawn", "Gone"], "decide", None, None);
    });
    // The request is out, the script waits.
    let requests = capture.0.lock().unwrap_or_else(|e| e.into_inner()).clone();
    assert_eq!(requests, [(1, vec!["Respawn".to_owned(), "Gone".to_owned()])]);
    assert_eq!(w.script_rereads_waiting(), 1);
    assert!(lines(&log).is_empty());
    // The other server changed `Respawn` and deleted `Gone`.
    w.on_global_variables_reread(1, Ok(vec![("Respawn".to_owned(), Some("999".to_owned()))]));
    w.script_drain();
    assert_eq!(lines(&log), ["decide 999 false"]);
    assert_eq!(w.script_rereads_waiting(), 0);
    // Nothing of it is written back at the next store (the marks are clear).
    assert!(w.scripts.globals.take_writes().is_empty());
}

#[test]
fn a_failed_reread_refuses_the_action() {
    let (mut w, log, _capture) = decider_world(false);
    w.run_as_script("Decider", |ctx| {
        ctx.reread_global_variables(&["Respawn"], "decide", None, None);
    });
    w.on_global_variables_reread(1, Err(()));
    w.script_drain();
    // The continuation did not run; the copy is what it was.
    assert_eq!(lines(&log), ["failed decide"]);
    assert_eq!(w.global_variables().get_long("Respawn", 0), 100);
}

#[test]
fn with_the_switch_on_the_copy_is_the_truth_and_nothing_is_read() {
    let (mut w, log, capture) = decider_world(true);
    w.run_as_script("Decider", |ctx| {
        ctx.reread_global_variables(&["Respawn"], "decide", None, None);
    });
    assert!(capture.0.lock().unwrap_or_else(|e| e.into_inner()).is_empty());
    assert_eq!(lines(&log), ["decide 100 true"]);
}

/// `Script::enabled`: a script whose Java `main` is skipped is not installed.
struct Gated(bool);

impl Script for Gated {
    fn name(&self) -> &'static str {
        "Gated"
    }
    fn path(&self) -> &'static str {
        "ai/others/Gated"
    }
    fn enabled(&self, _world: &World) -> bool {
        self.0
    }
    fn setup(&self, s: &mut ScriptSetup) {
        s.add_first_talk_id(&[1]);
    }
}

#[test]
fn a_disabled_script_is_not_installed() {
    for enabled in [false, true] {
        let mut w = world(&Capture::default());
        let mut registry = ScriptRegistry::new();
        registry.add(Gated(enabled));
        w.install_scripts(registry);
        assert_eq!(w.has_script("Gated"), enabled);
        assert_eq!(w.scripts.listeners.has(EventKind::NpcFirstTalk, 1), enabled);
    }
}

/// `Npc.getVariables()`, the script value, `setLethalable` and the over-hit values.
#[test]
fn npc_extras_are_kept_per_npc_and_go_with_it() {
    let mut w = world(&Capture::default());
    assert!(!w.npc_has_variables(A));
    w.npc_variables_mut(A).set_int("count", 3);
    assert!(w.npc_has_variables(A));
    assert_eq!(w.npc_variables(A).map(|v| v.get_int("count", 0)), Some(3));
    assert!(!w.npc_has_variables(B));
    assert!(w.is_npc_script_value(A, 0));
    w.set_npc_script_value(A, 2);
    assert!(w.is_npc_script_value(A, 2));
    assert!(w.is_lethalable_by_script(A));
    w.set_lethalable(A, false);
    assert!(!w.is_lethalable_by_script(A));
    assert!(w.is_lethalable_by_script(B));
    w.overhit_enabled(A, true);
    assert!(w.is_overhit(A));
    w.clear_overhit(A);
    assert!(!w.is_overhit(A));
    // `Npc.deleteMe`.
    w.script_on_npc_decay(A);
    assert!(!w.npc_has_variables(A));
    assert_eq!(w.npc_script_value(A), 0);
    assert!(w.is_lethalable_by_script(A));
}
