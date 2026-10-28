#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic
#![allow(dead_code)] // the harness is a trimmed copy of tests/script.rs

//! The engine follow-ups (S-API-03): `TimerExecutor`, `CreatureSeeTaskManager`, the player and
//! creature listeners, the skill of a hit, the movie holder, `//show_quests`, spawn slots.
//! Java: mechanics/script/timers/*, taskmanagers/CreatureSeeTaskManager, mechanics/events/*.

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{NpcHtmlMessage, NpcQuestHtmlMessage, QuestList};
use l2_world::script::TimerParams;
use l2_world::script::{GlobalVariablesWrite, QuestInput};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

/// The quest and global variable writes the persistence task would get.
#[derive(Clone, Default)]
struct Writes {
    quests: Arc<Mutex<Vec<QuestWrite>>>,
    globals: Arc<Mutex<Vec<GlobalVariablesWrite>>>,
}

struct RecordingOutbox(Writes);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        match command {
            PersistCommand::Quest(write) => self.0.quests.lock().unwrap().push(write),
            PersistCommand::GlobalVariables(write) => self.0.globals.lock().unwrap().push(write),
            _ => {}
        }
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: 5,
            body_part: 0,
            quest_item: t.quest_item,
            weight: 0,
            shared_reuse_group: -1,
            weapon: false,
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

const FOLK: i32 = 1001;
const OTHER_FOLK: i32 = 1002;
const RAT: i32 = 2000;
const TOKEN: i32 = 9999;

const NPCS: &str = r#"<list>
  <npc id="1001" type="Folk" level="10" name="Villager"/>
  <npc id="1002" type="Folk" level="10" name="Elder"/>
  <npc id="2000" type="Monster" level="3" name="Rat"><stats><vitals hp="62" mp="10"/></stats></npc>
</list>"#;

const ITEMS: &str = r#"<list>
  <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
  <item id="9999" type="EtcItem" name="Token"><set name="is_stackable" val="true"/><set name="is_questitem" val="true"/></item>
</list>"#;

/// What the recording scripts heard, as `"<script>:<event>"` lines.
#[derive(Clone, Default)]
struct Heard(Arc<Mutex<Vec<String>>>);

impl Heard {
    fn push(&self, line: String) {
        self.0.lock().unwrap().push(line);
    }
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

/// The script under test: writes down every hook of this card.
struct Probe {
    heard: Heard,
}

impl Script for Probe {
    fn name(&self) -> &'static str {
        "Probe"
    }
    fn path(&self) -> &'static str {
        "ai/others/Probe"
    }
    fn setup(&self, s: &mut ScriptSetup) {
        s.add_attack_id(&[RAT]);
        s.add_creature_see_id(&[RAT]);
        s.add_creature_death_id(&[RAT]);
        s.add_talk_id(&[FOLK]);
        s.set_on_player_bypass();
        s.set_on_player_press_tutorial_mark();
        s.set_on_player_logout();
        s.set_on_day_night_change();
    }
    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        match event {
            "add" => {
                let params = TimerParams::new().set("Name", "x").set("N", 7);
                ctx.add_timer("T", Some(params), 100, npc, player);
            }
            "add_other" => {
                let params = TimerParams::new().set("Name", "y");
                ctx.add_timer("T", Some(params), 100, npc, player);
            }
            "repeat" => {
                ctx.add_repeating_timer("R", 100, None, player);
            }
            "stop" => {
                self.heard.push(format!("stopped {}", ctx.cancel_timers("R")));
            }
            "has" => {
                self.heard
                    .push(format!("has {}", ctx.has_timer("T", npc, player)));
            }
            _ => {}
        }
        None
    }
    fn on_timer_event(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        event: &str,
        params: Option<&TimerParams>,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) {
        let name = params.map(|p| p.get_string("Name")).unwrap_or_default();
        let n = params.map_or(0, |p| p.get_int("N", 0));
        self.heard.push(format!("event {event} {name} {n}"));
    }
    fn on_timer_cancel(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        event: &str,
        params: Option<&TimerParams>,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) {
        let name = params.map(|p| p.get_string("Name")).unwrap_or_default();
        self.heard.push(format!("cancel {event} {name}"));
    }
    fn on_attack(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        _npc: ScriptNpc,
        _attacker: ObjectId,
        damage: i32,
        _is_summon: bool,
        skill: Option<SkillKey>,
    ) {
        self.heard.push(format!(
            "attack {damage} {}",
            skill.map_or_else(|| "none".to_owned(), |s| format!("{}-{}", s.id, s.level))
        ));
    }
    fn on_creature_see(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, creature: ObjectId) {
        self.heard.push(format!("see {} {}", npc.id, creature.0));
    }
    fn on_creature_death(&self, _ctx: &mut ScriptCtx<'_>, killer: Option<ObjectId>, victim: ObjectId) {
        self.heard
            .push(format!("death {} {}", killer.map_or(0, |k| k.0), victim.0 != 0));
    }
    fn on_player_bypass(&self, _ctx: &mut ScriptCtx<'_>, player: ObjectId, command: &str) {
        self.heard.push(format!("bypass {} {command}", player.0));
    }
    fn on_player_press_tutorial_mark(&self, _ctx: &mut ScriptCtx<'_>, player: ObjectId, mark_id: i32) {
        self.heard.push(format!("mark {} {mark_id}", player.0));
    }
    fn on_player_logout(&self, _ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        self.heard.push(format!("logout {}", player.0));
    }
}

fn probe(heard: &Heard) -> ScriptRegistry {
    let mut r = ScriptRegistry::new();
    r.add(Probe { heard: heard.clone() });
    r
}

fn setup() -> Setup {
    setup_with(ScriptSettings::default(), probe)
}

struct Setup {
    world: World,
    log: Log,
    writes: Writes,
    heard: Heard,
    _root: tempfile::TempDir,
}

fn write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

/// A world with the scripts `scripts` makes.
fn setup_with(settings: ScriptSettings, scripts: impl FnOnce(&Heard) -> ScriptRegistry) -> Setup {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "data/spawns/none.xml",
        r#"<?xml version="1.0"?><list enabled="true"></list>"#,
    );
    write(
        root.path(),
        "data/html/default/1001.htm",
        r#"<html><body>Villager <a action="bypass Script">Quest</a></body></html>"#,
    );
    write(
        root.path(),
        "data/html/noquest.htm",
        "<html><body>no quest %objectId%</body></html>",
    );
    write(
        root.path(),
        "data/scripts/quests/Recorder/page.htm",
        r#"<html><body>quest page <a action="bypass Script QA x">x</a></body></html>"#,
    );
    write(
        root.path(),
        "data/scripts/quests/Recorder/page.html",
        "<html><body>plain page</body></html>",
    );
    let log = Log::default();
    let writes = Writes::default();
    let heard = Heard::default();
    let mut world = World::new(WorldRng::from_seed(5), Box::new(RecordingOutbox(writes.clone())));
    world.set_static_data(Box::new(Data(StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        ..StatData::default()
    })));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    world.set_script_settings(settings);
    world.install_scripts(scripts(&heard));
    Setup {
        world,
        log,
        writes,
        heard,
        _root: root,
    }
}

fn player(id: i32, items: Vec<ItemData>) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{id}"),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 20,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X,
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 10.0,
        max_cp: 10.0,
        online_time: 0,
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn token(count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(0x1000_0002),
        item_id: TOKEN,
        count,
        enchant_level: 0,
        loc: "INVENTORY".to_owned(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

impl Setup {
    fn enter_with(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn enter(&mut self, id: i32) {
        self.enter_with(player(id, vec![]));
    }

    fn spawn(&mut self, npc_id: i32, dx: i32) -> ObjectId {
        self.world.spawn_npc_at(npc_id, X + dx, Y, Z, 0).unwrap()
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    fn talk(&mut self, player: i32, npc: ObjectId) {
        for _ in 0..2 {
            self.input(
                player,
                ClientInput::Action {
                    object_id: npc.0,
                    action_id: 0,
                },
            );
        }
    }

    fn bypass(&mut self, player: i32, command: &str) {
        self.input(
            player,
            ClientInput::Bypass {
                command: command.to_owned(),
            },
        );
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn take(&self) -> Sent {
        std::mem::take(&mut self.log.0.lock().unwrap())
    }

    fn quest_writes(&self) -> Vec<QuestWrite> {
        std::mem::take(&mut self.writes.quests.lock().unwrap())
    }

    /// Runs `f` as the script `name` (a hook of it, for the tests of the helpers).
    fn as_script(&mut self, name: &str, f: impl FnOnce(&mut ScriptCtx<'_>)) {
        self.world.run_as_script(name, f);
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn names_to(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, _)| *t == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

fn pages_to(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, _)| *t == to)
        .filter_map(|(_, b)| match name_of(b).as_str() {
            "NPC_HTML_MESSAGE" => Some(decode::<NpcHtmlMessage>(b).html),
            "EX_NPC_QUEST_HTML_MESSAGE" => Some(decode::<NpcQuestHtmlMessage>(b).html),
            _ => None,
        })
        .collect()
}

fn quest_lists_to(out: &Sent, to: i32) -> Vec<QuestList> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == "QUEST_LIST")
        .map(|(_, b)| decode::<QuestList>(b))
        .collect()
}

// ---------------------------------------------------------------------------------- TimerExecutor

#[test]
fn timer_executor_one_shot_cancels_then_fires_with_params() {
    let mut s = setup();
    s.enter(1);
    s.world.run_as_script("Probe", |c| {
        c.add_timer(
            "T",
            Some(TimerParams::new().set("Name", "x").set("N", 7)),
            100,
            None,
            Some(ObjectId(1)),
        );
    });
    assert_eq!(s.world.exec_timer_count(), 1);
    s.ticks(1);
    assert!(s.heard.take().is_empty());
    s.ticks(1);
    // NOTE(java-quirk): the running one-shot holder is cancelled by `onTimerPostExecute` first.
    assert_eq!(s.heard.take(), ["cancel T x", "event T x 7"]);
    assert_eq!(s.world.exec_timer_count(), 0);
    s.ticks(10);
    assert!(s.heard.take().is_empty());
}

#[test]
fn timer_executor_replaces_an_equal_timer() {
    let mut s = setup();
    s.enter(1);
    s.world.run_as_script("Probe", |c| {
        c.add_timer(
            "T",
            Some(TimerParams::new().set("Name", "x")),
            100,
            None,
            Some(ObjectId(1)),
        );
        // Equal: same event, npc, player. The first one is cancelled (with its params).
        c.add_timer(
            "T",
            Some(TimerParams::new().set("Name", "y")),
            100,
            None,
            Some(ObjectId(1)),
        );
        // Another player: kept.
        c.add_timer("T", Some(TimerParams::new().set("Name", "z")), 100, None, None);
    });
    assert_eq!(s.heard.take(), ["cancel T x"]);
    assert_eq!(s.world.exec_timer_count(), 2);
    s.ticks(2);
    assert_eq!(
        s.heard.take(),
        ["cancel T y", "event T y 0", "cancel T z", "event T z 0"]
    );
}

#[test]
fn timer_executor_repeating_runs_until_cancelled() {
    let mut s = setup();
    s.enter(1);
    s.world.run_as_script("Probe", |c| {
        c.add_repeating_timer("R", 100, None, Some(ObjectId(1)));
    });
    s.ticks(6);
    assert_eq!(s.heard.take(), ["event R  0", "event R  0", "event R  0"]);
    assert_eq!(s.world.exec_timer_count(), 1);
    s.world.run_as_script("Probe", |c| {
        assert!(c.cancel_timers("R"));
        assert!(!c.cancel_timers("R"));
    });
    assert_eq!(s.heard.take(), ["cancel R "]);
    s.ticks(6);
    assert!(s.heard.take().is_empty());
}

#[test]
fn timer_executor_npc_decay_stops_the_task_but_keeps_the_holder() {
    let mut s = setup();
    s.enter(1);
    let folk = s.spawn(FOLK, 30);
    let npc = ScriptNpc {
        object_id: folk,
        id: FOLK,
    };
    s.world.run_as_script("Probe", |c| {
        c.add_timer(
            "T",
            Some(TimerParams::new().set("Name", "x")),
            100,
            Some(npc),
            None,
        );
    });
    assert!(s.world.delete_npc(folk));
    s.ticks(4);
    // NOTE(java-quirk): `stopTimerHolders` only cancels the task: no event, no cancel hook, and
    // `hasTimer` still says yes.
    assert!(s.heard.take().is_empty());
    assert_eq!(s.world.exec_timer_count(), 1);
    s.world
        .run_as_script("Probe", |c| assert!(c.has_timer("T", Some(npc), None)));
    // An equal timer removes the stale holder without a cancel hook.
    s.world.run_as_script("Probe", |c| {
        c.add_timer(
            "T",
            Some(TimerParams::new().set("Name", "y")),
            100,
            Some(npc),
            None,
        );
    });
    assert!(s.heard.take().is_empty());
    assert_eq!(s.world.exec_timer_count(), 1);
}

#[test]
fn timer_executor_player_logout_stops_the_task() {
    let mut s = setup();
    s.enter(1);
    s.world.run_as_script("Probe", |c| {
        c.add_timer(
            "T",
            Some(TimerParams::new().set("Name", "x")),
            100,
            None,
            Some(ObjectId(1)),
        );
    });
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    s.ticks(4);
    assert_eq!(s.heard.take(), ["logout 1"]);
}

// ---------------------------------------------------------------------------------- on_attack

#[test]
fn attack_event_carries_the_skill_of_the_hit() {
    let mut s = setup();
    s.enter(1);
    let rat = s.spawn(RAT, 30);
    s.world.reduce_current_hp(rat, 10.0, Some(ObjectId(1)));
    s.world.script_drain();
    assert_eq!(s.heard.take(), ["attack 10 none"]);
    // What `reduce_current_hp_by_skill` does around the hit.
    s.world.set_script_hit_skill(Some(SkillKey::new(1177, 4)));
    s.world.reduce_current_hp(rat, 5.0, Some(ObjectId(1)));
    s.world.set_script_hit_skill(None);
    s.world.script_drain();
    assert_eq!(s.heard.take(), ["attack 5 1177-4"]);
}

// ---------------------------------------------------------------------------------- creature see

#[test]
fn creature_see_fires_once_per_creature_each_second() {
    let mut s = setup();
    s.enter(1);
    let rat = s.spawn(RAT, 30);
    assert_eq!(s.world.creature_see_count(), 1);
    s.ticks(19);
    assert!(s.heard.take().is_empty());
    s.ticks(1);
    s.world.script_drain();
    assert_eq!(s.heard.take(), [format!("see {RAT} 1")]);
    s.ticks(60);
    assert!(s.heard.take().is_empty());
    // A creature that enters later is new.
    s.enter(2);
    s.ticks(20);
    assert_eq!(s.heard.take(), [format!("see {RAT} 2")]);
    // `deleteMe` forgets everything.
    s.world.delete_npc(rat);
    assert_eq!(s.world.creature_see_count(), 0);
}

// ---------------------------------------------------------------------------------- listeners

#[test]
fn creature_death_is_heard_at_once_by_the_npc_listeners() {
    let mut s = setup();
    s.enter(1);
    let rat = s.spawn(RAT, 30);
    s.heard.take();
    assert!(s.world.do_die(rat, Some(ObjectId(1))));
    assert_eq!(s.heard.take(), ["death 1 true"]);
}

#[test]
fn player_bypass_tutorial_mark_and_logout_reach_the_global_listeners() {
    let mut s = setup();
    s.enter(1);
    // `item_` is one of the commands that need no page validation.
    s.bypass(1, "item_123_nothing");
    assert_eq!(s.heard.take(), ["bypass 1 item_123_nothing"]);
    s.input(
        1,
        ClientInput::Quest(QuestInput::TutorialPassCmd {
            bypass: "tutor".to_owned(),
        }),
    );
    assert_eq!(s.heard.take(), ["bypass 1 tutor"]);
    s.input(
        1,
        ClientInput::Quest(QuestInput::TutorialQuestionMark { number: 9 }),
    );
    assert_eq!(s.heard.take(), ["mark 1 9"]);
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(1),
    });
    assert_eq!(s.heard.take(), ["logout 1"]);
}

// ---------------------------------------------------------------------------------- movie

#[test]
fn movie_holder_waits_for_the_end_scene_packet() {
    let mut s = setup();
    s.enter(1);
    s.take();
    s.world.run_as_script("Probe", |c| c.play_movie(ObjectId(1), 7));
    assert!(s.world.is_playing_movie(ObjectId(1)));
    assert!(names_to(&s.take(), 1).contains(&"EX_START_SCENE_PLAYER".to_owned()));
    // A second movie is ignored while one plays.
    s.world.run_as_script("Probe", |c| c.play_movie(ObjectId(1), 8));
    assert!(!names_to(&s.take(), 1).contains(&"EX_START_SCENE_PLAYER".to_owned()));
    // The wrong id and id 0 change nothing.
    s.input(1, ClientInput::Quest(QuestInput::EndScene { movie_id: 8 }));
    s.input(1, ClientInput::Quest(QuestInput::EndScene { movie_id: 0 }));
    assert!(s.world.is_playing_movie(ObjectId(1)));
    s.input(1, ClientInput::Quest(QuestInput::EndScene { movie_id: 7 }));
    assert!(!s.world.is_playing_movie(ObjectId(1)));
    // Free again.
    s.world.run_as_script("Probe", |c| c.play_movie(ObjectId(1), 8));
    assert!(s.world.is_playing_movie(ObjectId(1)));
}

// ---------------------------------------------------------------------------------- admin views

#[test]
fn gm_view_quest_info_lists_the_active_quests_with_their_cond() {
    let mut s = setup_with(ScriptSettings::default(), |heard| {
        let mut r = ScriptRegistry::new();
        r.add(Quester { heard: heard.clone() });
        r
    });
    s.enter(1);
    s.world.run_as_script("Q00005_Quester", |c| {
        let qs = c.get_quest_state(ObjectId(1), true).unwrap();
        qs.start_quest(c);
        qs.set_cond(c, 3, false);
    });
    assert_eq!(s.world.gm_view_quests(ObjectId(1)), [(5, 3)]);
}

/// A journal quest, for the GM window.
struct Quester {
    heard: Heard,
}

impl Script for Quester {
    fn name(&self) -> &'static str {
        "Q00005_Quester"
    }
    fn path(&self) -> &'static str {
        "quests/Q00005_Quester"
    }
    fn id(&self) -> i32 {
        5
    }
    fn setup(&self, s: &mut ScriptSetup) {
        s.add_talk_id(&[FOLK]);
        let _ = &self.heard;
    }
}

// ---------------------------------------------------------------------------------- spawn slots

#[test]
fn script_spawn_slots_are_reused_after_the_npc_is_gone() {
    let mut s = setup();
    let mut ids = Vec::new();
    s.world.run_as_script("Probe", |c| {
        for _ in 0..3 {
            let npc = c.add_spawn(FOLK, X, Y, Z, 0, false, 0, 0).unwrap().object_id;
            ids.push(npc);
        }
    });
    let slots = s.world.runtime_spawn_slots();
    for id in ids {
        assert!(s.world.delete_npc(id));
    }
    s.world.run_as_script("Probe", |c| {
        for _ in 0..3 {
            c.add_spawn(FOLK, X, Y, Z, 0, false, 0, 0).unwrap();
        }
    });
    assert_eq!(s.world.runtime_spawn_slots(), slots);
}
