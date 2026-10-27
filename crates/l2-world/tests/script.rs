#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The script engine (S-API-01): registration and dispatch order, quest state rows, quest
//! timers, the quest window and chooser, the journal packets and the global variables.
//! Java: mechanics/script/{Quest,QuestState,QuestTimer}.java, mechanics/events/EventDispatcher,
//! handlers/bypass/npc/ScriptLink, network/clientpackets/{RequestQuestAbort,RequestTutorial*}.

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{NpcHtmlMessage, NpcQuestHtmlMessage, QuestList};
use l2_world::script::{GlobalVariablesWrite, QuestInput, STARTED};
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

/// A quest (or script) that listens to the test NPCs and writes down every hook call.
struct Recorder {
    name: &'static str,
    id: i32,
    heard: Heard,
}

impl Script for Recorder {
    fn name(&self) -> &'static str {
        self.name
    }
    fn path(&self) -> &'static str {
        "quests/Recorder"
    }
    fn id(&self) -> i32 {
        self.id
    }
    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[FOLK]);
        s.add_talk_id(&[FOLK]);
        s.add_first_talk_id(&[OTHER_FOLK]);
        s.add_kill_id(&[RAT]);
        s.add_attack_id(&[RAT]);
        s.add_spawn_id(&[RAT]);
        s.set_on_enter_world();
        s.register_quest_items(&[TOKEN]);
    }
    fn on_event(&self, ctx: &mut ScriptCtx<'_>, event: &str, npc: Option<ScriptNpc>, player: Option<ObjectId>) -> Option<String> {
        self.heard.push(format!("{}:event {event} {}", self.name, npc.is_some()));
        match event {
            "start" => {
                let qs = ctx.get_quest_state(player?, true)?;
                qs.start_quest(ctx);
                None
            }
            "timer" => {
                ctx.start_quest_timer("tick", 1000, npc, player, false);
                None
            }
            "repeat" => {
                ctx.start_quest_timer("rep", 100, None, player, true);
                ctx.start_quest_timer("rep", 100, None, player, true);
                None
            }
            "stop" => {
                ctx.cancel_quest_timer("rep", None, player);
                None
            }
            _ => None,
        }
    }
    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        self.heard.push(format!("{}:talk", self.name));
        let qs = ctx.get_quest_state(player, true)?;
        Some(format!("<html><body>{} state {}</body></html>", self.name, qs.state(ctx)))
    }
    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        self.heard.push(format!("{}:first_talk", self.name));
        Some(format!("<html><body>first {}</body></html>", self.name))
    }
    fn on_kill(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, is_summon: bool) {
        self.heard
            .push(format!("{}:kill {} {} {is_summon}", self.name, npc.id, killer.0));
    }
    fn on_attack(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        self.heard
            .push(format!("{}:attack {} {} {damage}", self.name, npc.id, attacker.0));
    }
    fn on_spawn(&self, _ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        self.heard.push(format!("{}:spawn {}", self.name, npc.id));
    }
    fn on_enter_world(&self, _ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        self.heard.push(format!("{}:enter {}", self.name, player.0));
    }
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

/// A world with two recording quests (`QA` 1, `QB` 2) unless `scripts` says otherwise.
fn setup_with(settings: ScriptSettings, scripts: impl FnOnce(&Heard) -> ScriptRegistry) -> Setup {
    let root = tempfile::tempdir().unwrap();
    write(root.path(), "data/spawns/none.xml", r#"<?xml version="1.0"?><list enabled="true"></list>"#);
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

fn two_quests(heard: &Heard) -> ScriptRegistry {
    let mut r = ScriptRegistry::new();
    for (name, id) in [("QA", 1), ("QB", 2)] {
        r.add(Recorder {
            name,
            id,
            heard: heard.clone(),
        });
    }
    r
}

fn setup() -> Setup {
    setup_with(ScriptSettings::default(), two_quests)
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

fn insert(char_id: i32, name: &str, var: &str, value: &str) -> QuestWrite {
    QuestWrite::InsertVar {
        char_id,
        name: name.to_owned(),
        var: var.to_owned(),
        value: value.to_owned(),
    }
}

fn update(char_id: i32, name: &str, var: &str, value: &str) -> QuestWrite {
    QuestWrite::UpdateVar {
        char_id,
        name: name.to_owned(),
        var: var.to_owned(),
        value: value.to_owned(),
    }
}

// ---------------------------------------------------------------------------------- dispatch

#[test]
fn listeners_hear_events_in_registration_order() {
    let mut s = setup();
    // `setOnEnterWorld`: both, in order, after the enter world packets.
    s.enter(1);
    assert_eq!(s.heard.take(), ["QA:enter 1", "QB:enter 1"]);

    // `ON_NPC_SPAWN` (async: after the spawn).
    let rat = s.spawn(RAT, 30);
    s.world.script_drain();
    assert_eq!(s.heard.take(), ["QA:spawn 2000", "QB:spawn 2000"]);

    // `ON_ATTACKABLE_ATTACK`.
    s.world.reduce_current_hp(rat, 10.0, Some(ObjectId(1)));
    s.world.script_drain();
    assert_eq!(s.heard.take(), ["QA:attack 2000 1 10", "QB:attack 2000 1 10"]);

    // `ON_ATTACKABLE_KILL` comes `onKillDelay` (2.5 s = 50 ticks) after the death.
    assert!(s.world.do_die(rat, Some(ObjectId(1))));
    s.ticks(49);
    assert!(s.heard.take().is_empty());
    s.ticks(1);
    assert_eq!(s.heard.take(), ["QA:kill 2000 1 false", "QB:kill 2000 1 false"]);
}

#[test]
fn first_talk_listeners_replace_the_npc_page() {
    let mut s = setup();
    s.enter(1);
    let elder = s.spawn(OTHER_FOLK, 30);
    s.take();
    s.heard.take();
    s.talk(1, elder);
    // Java's `notifyEventAsync`: every listener answers, each with its own page.
    assert_eq!(s.heard.take(), ["QA:first_talk", "QB:first_talk"]);
    let pages = pages_to(&s.take(), 1);
    assert_eq!(pages.len(), 2);
    assert!(pages[0].contains("first QA") && pages[1].contains("first QB"));
}

// ---------------------------------------------------------------------------------- quest window

#[test]
fn the_quest_link_shows_a_chooser_for_two_quests() {
    let mut s = setup();
    s.enter(1);
    let villager = s.spawn(FOLK, 30);
    s.talk(1, villager);
    s.take();
    s.heard.take();
    s.bypass(1, "Script");
    let pages = pages_to(&s.take(), 1);
    let chooser = pages.last().unwrap();
    // `showQuestChooseWindow`: a link per quest with the client string of its name + state 01.
    let expected = format!(
        "<html><body><a action=\"bypass npc_{0}_Script QA\">[<fstring>101</fstring>]</a><br>\
         <a action=\"bypass npc_{0}_Script QB\">[<fstring>201</fstring>]</a><br></body></html>",
        villager.0
    );
    assert_eq!(chooser, &expected);

    // The link of a quest: its `onTalk` page.
    s.bypass(1, &format!("npc_{}_Script QB", villager.0));
    assert_eq!(s.heard.take(), ["QB:talk"]);
    assert!(pages_to(&s.take(), 1).last().unwrap().contains("QB state 0"));
}

#[test]
fn the_quest_link_of_an_unknown_script_is_the_no_quest_page() {
    let mut s = setup_with(ScriptSettings::default(), |_| ScriptRegistry::new());
    s.enter(1);
    let villager = s.spawn(FOLK, 30);
    s.talk(1, villager);
    s.take();
    s.bypass(1, "Script");
    let page = pages_to(&s.take(), 1).pop().unwrap();
    assert_eq!(page, format!("<html><body>no quest {}</body></html>", villager.0));
}

// ---------------------------------------------------------------------------------- quest state

#[test]
fn start_quest_writes_cond_and_state_and_sends_the_journal() {
    let mut s = setup();
    s.enter(1);
    s.take();
    s.as_script("QA", |ctx| {
        let qs = ctx.get_quest_state(ObjectId(1), true).unwrap();
        qs.start_quest(ctx);
    });
    assert_eq!(
        s.quest_writes(),
        [insert(1, "QA", "cond", "1"), insert(1, "QA", "<state>", "Started")]
    );
    let out = s.take();
    assert_eq!(
        names_to(&out, 1),
        ["QUEST_LIST", "EX_SHOW_QUEST_MARK", "QUEST_LIST", "PLAY_SOUND"]
    );
    // The first `QuestList` comes before the state changes: the quest is not in it yet.
    let lists = quest_lists_to(&out, 1);
    assert!(lists[0].quests.is_empty());
    assert_eq!(lists[1].quests.len(), 1);
}

#[test]
fn a_skipped_step_writes_the_step_flags() {
    let mut s = setup();
    s.enter(1);
    s.as_script("QA", |ctx| {
        let qs = ctx.get_quest_state(ObjectId(1), true).unwrap();
        qs.start_quest(ctx);
    });
    s.quest_writes();
    s.as_script("QA", |ctx| {
        let qs = ctx.get_quest_state(ObjectId(1), false).unwrap();
        // 1 -> 3 skips step 2: `0x80000001 | ((1 << 1) - 1) | (1 << 2)`.
        qs.set_cond(ctx, 3, true);
        assert_eq!(qs.get_int(ctx, "__compltdStateFlags"), 0x8000_0005_u32 as i32);
        // Forward again: the bit of step 4 joins.
        qs.set_cond(ctx, 4, false);
        assert_eq!(qs.get_int(ctx, "__compltdStateFlags"), 0x8000_000D_u32 as i32);
        // Back to 2: the flags of the steps ahead go, and with them the flags.
        qs.set_cond(ctx, 2, false);
        assert!(!qs.is_set(ctx, "__compltdStateFlags"));
    });
    let flags = (0x8000_0005_u32 as i32).to_string();
    let flags_4 = (0x8000_000D_u32 as i32).to_string();
    assert_eq!(
        s.quest_writes(),
        [
            update(1, "QA", "cond", "3"),
            insert(1, "QA", "__compltdStateFlags", &flags),
            update(1, "QA", "cond", "4"),
            update(1, "QA", "__compltdStateFlags", &flags_4),
            update(1, "QA", "cond", "2"),
            QuestWrite::DeleteVar {
                char_id: 1,
                name: "QA".to_owned(),
                var: "__compltdStateFlags".to_owned(),
            },
        ]
    );
    // NOTE(java-quirk): the journal shows the condition, as the flags are negative.
    let journal = s.world.quest_list(ObjectId(1));
    assert_eq!(journal.quests[0].status, 2);
}

#[test]
fn exit_quest_takes_the_quest_items_and_ends_the_rows() {
    let mut s = setup();
    s.enter_with(player(1, vec![token(5)]));
    s.as_script("QA", |ctx| {
        let qs = ctx.get_quest_state(ObjectId(1), true).unwrap();
        qs.start_quest(ctx);
        qs.set(ctx, "memo", "x");
    });
    s.quest_writes();
    // One-time: the state row stays `Completed`.
    s.as_script("QA", |ctx| {
        let qs = ctx.get_quest_state(ObjectId(1), false).unwrap();
        qs.exit_quest(ctx, false, true);
        assert!(qs.is_completed(ctx));
        assert_eq!(qs.get(ctx, "memo"), None);
    });
    assert_eq!(
        s.quest_writes(),
        [
            QuestWrite::DeleteQuest {
                char_id: 1,
                name: "QA".to_owned(),
                repeatable: false,
            },
            update(1, "QA", "<state>", "Completed"),
        ]
    );
    let tokens = s
        .world
        .player(ObjectId(1))
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == TOKEN)
        .count();
    assert_eq!(tokens, 0);

    // Repeatable: everything goes, the state too.
    s.as_script("QB", |ctx| {
        let qs = ctx.get_quest_state(ObjectId(1), true).unwrap();
        qs.start_quest(ctx);
        qs.exit_quest(ctx, true, false);
    });
    let writes = s.quest_writes();
    assert_eq!(
        writes.last().unwrap(),
        &QuestWrite::DeleteQuest {
            char_id: 1,
            name: "QB".to_owned(),
            repeatable: true,
        }
    );
    assert!(s.world.player_quests(ObjectId(1)).unwrap().get("QB").is_none());
}

#[test]
fn quest_rows_of_character_select_become_the_states() {
    let mut s = setup();
    let mut data = player(1, vec![]);
    data.profile.quests = QuestLoad {
        states: vec![
            ("QA".to_owned(), Some("Started".to_owned())),
            // A script this server does not have: kept in the database, ignored here (D-39).
            ("Q09999_NotPorted".to_owned(), Some("Started".to_owned())),
        ],
        variables: vec![
            ("QA".to_owned(), "cond".to_owned(), Some("3".to_owned())),
            // A variable without a state row makes a created state (`getQuestState(player, true)`).
            ("QB".to_owned(), "memo".to_owned(), None),
        ],
    };
    s.enter_with(data);
    let out = s.take();
    // `EnterWorld` sends the journal with the loaded quest.
    let lists = quest_lists_to(&out, 1);
    assert_eq!(lists[0].quests.len(), 1);
    assert_eq!((lists[0].quests[0].quest_id, lists[0].quests[0].status), (1, 3));
    let quests = s.world.player_quests(ObjectId(1)).unwrap();
    assert_eq!(quests.get("QA").unwrap().state(), STARTED);
    assert!(quests.get("QB").unwrap().is_created());
    assert_eq!(quests.get("QB").unwrap().get("memo"), Some(""));
    assert!(quests.get("Q09999_NotPorted").is_none());
    // No row is deleted.
    assert!(s.quest_writes().is_empty());
}

#[test]
fn request_quest_abort_ends_the_quest_as_repeatable() {
    let mut s = setup();
    s.enter(1);
    s.as_script("QA", |ctx| {
        let qs = ctx.get_quest_state(ObjectId(1), true).unwrap();
        qs.start_quest(ctx);
    });
    s.quest_writes();
    s.take();
    s.input(1, ClientInput::Quest(QuestInput::Abort { quest_id: 1 }));
    assert_eq!(
        s.quest_writes(),
        [QuestWrite::DeleteQuest {
            char_id: 1,
            name: "QA".to_owned(),
            repeatable: true,
        }]
    );
    let lists = quest_lists_to(&s.take(), 1);
    assert_eq!(lists.len(), 2);
    assert!(lists.iter().all(|l| l.quests.is_empty()));
    // `RequestQuestList` answers with the journal.
    s.input(1, ClientInput::Quest(QuestInput::List));
    assert_eq!(names_to(&s.take(), 1), ["QUEST_LIST"]);
}

// ---------------------------------------------------------------------------------- html

#[test]
fn an_htm_of_a_quest_is_the_quest_window_and_an_html_the_npc_window() {
    let mut s = setup();
    s.enter(1);
    let villager = s.spawn(FOLK, 30);
    // `processScriptEvent` needs the NPC the player talked to last.
    s.talk(1, villager);
    s.take();
    s.heard.take();
    s.as_script("QA", |ctx| {
        let npc = ctx.script_npc(villager);
        ctx.show_result(ObjectId(1), Some("page.htm"), npc);
        ctx.show_result(ObjectId(1), Some("page.html"), npc);
        ctx.show_result(ObjectId(1), Some("a chat line"), npc);
    });
    let out = s.take();
    assert_eq!(
        names_to(&out, 1),
        [
            "EX_NPC_QUEST_HTML_MESSAGE",
            "ACTION_FAIL",
            "NPC_HTML_MESSAGE",
            "ACTION_FAIL",
            "SYSTEM_MESSAGE"
        ]
    );
    let quest_page: NpcQuestHtmlMessage = decode(&out[0].1);
    assert_eq!((quest_page.npc_obj_id, quest_page.quest_id), (villager.0, 1));
    // The links of a quest window are allowed like those of an NPC page.
    s.bypass(1, "Script QA x");
    assert_eq!(s.heard.take(), ["QA:event x true"]);
}

// ---------------------------------------------------------------------------------- timers

#[test]
fn quest_timers_fire_once_repeat_and_stop() {
    let mut s = setup();
    s.enter(1);
    s.heard.take();
    s.as_script("QA", |ctx| {
        ctx.start_quest_timer("once", 1000, None, Some(ObjectId(1)), false);
        // The same name, NPC and player again is dropped.
        ctx.start_quest_timer("once", 50, None, Some(ObjectId(1)), false);
    });
    assert_eq!(s.world.quest_timer_count(), 1);
    s.ticks(19);
    assert!(s.heard.take().is_empty());
    s.ticks(1);
    assert_eq!(s.heard.take(), ["QA:event once false"]);
    assert_eq!(s.world.quest_timer_count(), 0);

    // Repeating every 100 ms until cancelled.
    s.as_script("QA", |ctx| {
        ctx.start_quest_timer("rep", 100, None, Some(ObjectId(1)), true);
    });
    s.ticks(6);
    assert_eq!(s.heard.take().len(), 3);
    s.as_script("QA", |ctx| ctx.cancel_quest_timer("rep", None, Some(ObjectId(1))));
    s.ticks(6);
    assert!(s.heard.take().is_empty());
}

#[test]
fn the_timers_of_a_player_stop_when_it_leaves() {
    let mut s = setup();
    s.enter(1);
    s.as_script("QA", |ctx| {
        ctx.start_quest_timer("later", 1000, None, Some(ObjectId(1)), false);
        ctx.start_quest_timer("global", 1000, None, None, false);
    });
    s.input(1, ClientInput::Logout);
    assert_eq!(s.world.quest_timer_count(), 1);
    s.heard.take();
    s.ticks(20);
    assert_eq!(s.heard.take(), ["QA:event global false"]);
}

// ---------------------------------------------------------------------------------- tutorial

#[test]
fn tutorial_packets_reach_the_tutorial_quest() {
    let mut s = setup_with(ScriptSettings::default(), |heard| {
        let mut r = ScriptRegistry::new();
        r.add(Recorder {
            name: "Q00255_Tutorial",
            id: 255,
            heard: heard.clone(),
        });
        r
    });
    s.enter(1);
    // Without a state of the tutorial quest nothing happens.
    s.input(1, ClientInput::Quest(QuestInput::TutorialQuestionMark { number: 3 }));
    s.heard.take();
    s.as_script("Q00255_Tutorial", |ctx| {
        ctx.get_quest_state(ObjectId(1), true);
    });
    s.input(1, ClientInput::Quest(QuestInput::TutorialQuestionMark { number: 3 }));
    s.input(1, ClientInput::Quest(QuestInput::TutorialClientEvent { event_id: 8 }));
    s.input(
        1,
        ClientInput::Quest(QuestInput::TutorialLinkHtml {
            bypass: "close".to_owned(),
        }),
    );
    s.input(
        1,
        ClientInput::Quest(QuestInput::TutorialPassCmd {
            bypass: "ok".to_owned(),
        }),
    );
    assert_eq!(
        s.heard.take(),
        [
            "Q00255_Tutorial:event QM:3 false",
            "Q00255_Tutorial:event CE:8 false",
            "Q00255_Tutorial:event TL:close false",
            "Q00255_Tutorial:event TC:ok false"
        ]
    );
}

// ---------------------------------------------------------------------------------- globals

#[test]
fn global_variables_are_written_at_shutdown_only_with_the_switch() {
    for switch in [false, true] {
        let settings = ScriptSettings {
            cycle_database_writes: switch,
            ..ScriptSettings::default()
        };
        let mut s = setup_with(settings, two_quests);
        s.world
            .set_global_variables(vec![("old".to_owned(), Some("1".to_owned())), ("null".to_owned(), None)]);
        assert_eq!(s.world.global_variables().len(), 1);
        s.as_script("QA", |ctx| {
            assert_eq!(ctx.global_variables().get_int("old", 0), 1);
            ctx.set_global_variable("boss", "123");
            ctx.remove_global_variable("old");
        });
        s.world.handle(WorldCommand::Shutdown { done: Box::new(|| {}) });
        let globals = s.writes.globals.lock().unwrap().clone();
        if switch {
            assert_eq!(
                globals,
                [
                    GlobalVariablesWrite::Replace(vec![("boss".to_owned(), "123".to_owned())]),
                    GlobalVariablesWrite::Delete(vec!["old".to_owned()]),
                ]
            );
        } else {
            // D-54: Sieghardt owns the rows; Kain keeps them in memory.
            assert!(globals.is_empty());
        }
    }
}
