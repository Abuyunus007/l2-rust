#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The admin words of the script systems (S-ADMIN-08): `//quest_info`, `//show_quests`,
//! `//quest_reload`, `//script_load`, `//script_unload`, `//charquestmenu`, `//setcharquest` and
//! `//reload quest`, with a fake GM and a fake player.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminQuest,AdminShowQuests,AdminReload}.java

use std::sync::{Arc, Mutex};

use l2_data::HtmCache;
use l2_world::*;

mod admin_common;
use admin_common::{GM, Out, P1, Scene, html_to, texts_to};

type Lines = Arc<Mutex<Vec<String>>>;

fn lines(log: &Lines) -> Vec<String> {
    std::mem::take(&mut log.lock().unwrap())
}

const QUEST: &str = "Q00005_Quester";

/// A journal quest that listens to players entering the world and writes down what runs.
struct Quester(Lines);

impl Script for Quester {
    fn name(&self) -> &'static str {
        QUEST
    }
    fn path(&self) -> &'static str {
        "quests/Q00005_Quester"
    }
    fn id(&self) -> i32 {
        5
    }
    fn setup(&self, s: &mut ScriptSetup) {
        s.set_on_enter_world();
        s.add_talk_id(&[1001]);
        s.register_quest_items(&[9999]);
        self.0.lock().unwrap().push("setup".to_owned());
    }
    fn on_load(&self, _ctx: &mut ScriptCtx<'_>) {
        self.0.lock().unwrap().push("load".to_owned());
    }
    fn on_save(&self, _ctx: &mut ScriptCtx<'_>) {
        self.0.lock().unwrap().push("save".to_owned());
    }
    fn on_enter_world(&self, _ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        self.0.lock().unwrap().push(format!("enter {}", player.0));
    }
    fn on_event(
        &self,
        _ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        _player: Option<ObjectId>,
    ) -> Option<String> {
        self.0.lock().unwrap().push(format!("event {event}"));
        None
    }
}

/// A script that is not a quest.
struct Helper;

impl Script for Helper {
    fn name(&self) -> &'static str {
        "Helper"
    }
    fn path(&self) -> &'static str {
        "ai/others/Helper"
    }
    fn setup(&self, _s: &mut ScriptSetup) {}
}

struct Fake {
    rights: AdminRights,
}

impl StaticData for Fake {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        if level >= 70 {
            AccessLevelStatic {
                name_color: 0,
                title_color: 0,
                is_gm: true,
            }
        } else {
            AccessLevelStatic::USER
        }
    }
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.rights)
    }
}

struct Setup {
    scene: Scene,
    log: Lines,
}

impl Setup {
    fn new() -> Self {
        let mut scene = Scene::new();
        let mut rights = AdminRights::default();
        for (level, child) in [(0, 0), (70, 0), (100, 70)] {
            rights.add_level(level, child);
        }
        for word in [
            "show_quests",
            "quest_info",
            "quest_reload",
            "script_load",
            "script_unload",
            "charquestmenu",
            "setcharquest",
            "reload",
        ] {
            rights.add_command(word, 100, false);
        }
        scene.world.set_static_data(Box::new(Fake { rights }));
        let page = scene.root.path().join("data/html/admin/npc-quests.htm");
        std::fs::write(&page, "<html>%quests%|%objid%|%questName%</html>").unwrap();
        scene
            .world
            .set_html_cache(Arc::new(HtmCache::load(scene.root.path(), false)));
        let log = Lines::default();
        let mut registry = ScriptRegistry::new();
        registry.add(Quester(Arc::clone(&log)));
        registry.add(Helper);
        scene.world.install_scripts(registry);
        scene.enter(GM);
        scene.enter(P1);
        scene.log.take();
        scene.outs.take();
        lines(&log);
        Self { scene, log }
    }

    /// Runs a command of the GM and returns the texts and the pages he got.
    fn say(&mut self, command: &str) -> (Vec<String>, Vec<String>) {
        self.scene.cmd(GM, command);
        let out = self.scene.log.take();
        (texts_to(&out, GM), html_to(&out, GM))
    }

    fn start_quest(&mut self) {
        self.scene.world.run_as_script(QUEST, |c| {
            let qs = c.get_quest_state(ObjectId(P1), true).unwrap();
            qs.start_quest(c);
            qs.set_cond(c, 2, false);
            qs.set_int(c, "kills", 3);
        });
        self.scene.log.take();
        self.scene.outs.take();
    }

    fn quest_writes(&self) -> Vec<QuestWrite> {
        self.scene
            .outs
            .take()
            .into_iter()
            .filter_map(|o| match o {
                Out::Persist(PersistCommand::Quest(w)) => Some(w),
                _ => None,
            })
            .collect()
    }

    fn state_of_p1(&self) -> Option<(u8, Option<String>)> {
        let quests = self.scene.world.player_quests(ObjectId(P1))?;
        let qs = quests.get(QUEST)?;
        Some((qs.state(), qs.get("kills").map(str::to_owned)))
    }
}

// ---------------------------------------------------------------------------- show / info

#[test]
fn show_quests_needs_a_target_and_lists_the_scripts_of_it() {
    let mut s = Setup::new();
    let (texts, _) = s.say("show_quests");
    assert_eq!(texts, ["Get a target first."]);
    s.scene.target(GM, P1);
    s.scene.log.take();
    let (_, pages) = s.say("show_quests");
    // A player has no listeners of its own: the page is the empty list.
    assert_eq!(pages, [format!("<html>|{P1}|</html>")]);
}

#[test]
fn quest_info_shows_the_script_and_answers_a_bad_name() {
    let mut s = Setup::new();
    let (texts, pages) = s.say("quest_info Nope");
    assert_eq!(texts, ["Couldn't find quest or script with name Nope !"]);
    assert!(pages.is_empty());
    let (_, pages) = s.say(&format!("quest_info {QUEST}"));
    let page = &pages[0];
    assert!(page.contains("<font color=\"LEVEL\">ID:</font> <font color=00FF00>5</font>"));
    assert!(page.contains(&format!("admin_quest_reload {QUEST}")));
    assert!(page.contains(&format!("admin_script_unload {QUEST}")));
    // The NPC ids and the quest item of the setup.
    assert!(page.contains("NPCs:</font> <font color=00FF00>1: 1001"));
    assert!(page.contains("Items:</font> <font color=00FF00>1:9999"));
}

// ---------------------------------------------------------------------- unload and reload

#[test]
fn unload_stops_the_events_of_a_script_and_reload_brings_them_back() {
    let mut s = Setup::new();
    s.scene
        .world
        .run_as_script(QUEST, |c| c.start_quest_timer("tick", 60_000, None, None, false));
    assert_eq!(s.scene.world.quest_timer_count(), 1);
    // Heard while loaded.
    s.scene.enter(2);
    assert_eq!(lines(&s.log), ["enter 2"]);

    let (texts, _) = s.say(&format!("script_unload {QUEST}"));
    assert_eq!(texts, ["Script successfully unloaded!"]);
    assert_eq!(lines(&s.log), ["save"]);
    assert_eq!(s.scene.world.quest_timer_count(), 0);
    assert!(!s.scene.world.has_script(QUEST));
    s.scene.enter(3);
    assert!(lines(&s.log).is_empty());
    // Gone from the lists: the name and the id are unknown now.
    let (texts, _) = s.say(&format!("quest_info {QUEST}"));
    assert_eq!(
        texts,
        [format!("Couldn't find quest or script with name {QUEST} !")]
    );
    let (texts, _) = s.say("quest_reload 5");
    assert_eq!(texts, ["The script 5 couldn't be found!"]);

    // `script_load` brings the unloaded script back (here: the same compiled code).
    let (texts, _) = s.say(&format!("script_load scripts/quests/{QUEST}.java"));
    assert_eq!(texts, ["Script loaded successfully!"]);
    assert_eq!(lines(&s.log), ["setup", "load"]);
    s.scene.enter(4);
    assert_eq!(lines(&s.log), ["enter 4"]);
    assert!(s.scene.world.has_script(QUEST));
}

#[test]
fn quest_reload_unloads_and_loads_by_name_or_id() {
    let mut s = Setup::new();
    s.scene
        .world
        .run_as_script(QUEST, |c| c.start_quest_timer("tick", 60_000, None, None, false));
    let (texts, _) = s.say(&format!("quest_reload {QUEST}"));
    assert_eq!(texts, ["Script successfully reloaded."]);
    assert_eq!(lines(&s.log), ["save", "setup", "load"]);
    // The timer of the old instance is gone.
    assert_eq!(s.scene.world.quest_timer_count(), 0);
    // Still listening, once.
    s.scene.enter(2);
    assert_eq!(lines(&s.log), ["enter 2"]);
    let (texts, _) = s.say("quest_reload 5");
    assert_eq!(texts, ["Script successfully reloaded."]);
    assert_eq!(lines(&s.log), ["save", "setup", "load"]);
    s.scene.enter(3);
    assert_eq!(lines(&s.log), ["enter 3"]);
}

#[test]
fn the_reload_and_load_words_answer_bad_arguments() {
    let mut s = Setup::new();
    let (texts, _) = s.say("quest_reload");
    assert_eq!(texts, ["Usage: //quest_reload <questName> or <questId>"]);
    let (texts, _) = s.say("quest_reload Nope");
    assert_eq!(texts, ["The script Nope couldn't be found!"]);
    let (texts, _) = s.say("script_load");
    assert_eq!(texts, ["Usage: //script_load path/to/script.java"]);
    let (texts, _) = s.say("script_load scripts/Nope.java");
    assert_eq!(texts, ["Failed to load script!"]);
    // NOTE(java-quirk): the unload usage names script_load.
    let (texts, _) = s.say("script_unload");
    assert_eq!(texts, ["Usage: //script_load path/to/script.java"]);
    let (texts, _) = s.say("script_unload Nope");
    assert_eq!(texts, ["The script Nope couldn't be found!"]);
    // A number that does not fit an int is the Java exception.
    let (texts, _) = s.say("quest_reload 99999999999");
    assert!(texts[0].contains("NumberFormatException"), "{texts:?}");
    assert!(lines(&s.log).is_empty());
}

#[test]
fn reload_quest_by_name_by_id_and_all() {
    let mut s = Setup::new();
    let (texts, _) = s.say(&format!("reload quest {QUEST}"));
    assert_eq!(lines(&s.log), ["save", "setup", "load"]);
    assert!(
        texts.contains(&format!("P{GM}: Reloaded Quest Name:{QUEST}.")),
        "{texts:?}"
    );
    assert!(
        texts
            .last()
            .unwrap()
            .starts_with("WARNING: There are several known issues")
    );

    let (texts, _) = s.say("reload quest 5");
    assert_eq!(lines(&s.log), ["save", "setup", "load"]);
    assert!(
        texts.contains(&format!("P{GM}: Reloaded Quest ID:5.")),
        "{texts:?}"
    );

    // NOTE(java-quirk): an unknown name is announced too.
    let (texts, _) = s.say("reload quest Nope");
    assert!(lines(&s.log).is_empty());
    assert!(
        texts.contains(&format!("P{GM}: Reloaded Quest Name:Nope.")),
        "{texts:?}"
    );

    let (texts, _) = s.say("reload quest");
    assert_eq!(texts[0], "All scripts have been reloaded.");
    assert_eq!(texts[1], format!("P{GM}: Reloaded Quests."));
    assert_eq!(lines(&s.log), ["save", "setup", "load"]);
    s.scene.enter(2);
    assert_eq!(lines(&s.log), ["enter 2"]);
}

// ---------------------------------------------------------------------- //charquestmenu

#[test]
fn charquestmenu_shows_the_first_page_and_the_lists() {
    let mut s = Setup::new();
    let (_, pages) = s.say("charquestmenu P1");
    assert!(pages[0].contains("Quest Menu for <font color=\"LEVEL\">P1</font> (ID:1)"));
    assert!(pages[0].contains("admin_charquestmenu P1 3"));

    s.start_quest();
    let (_, pages) = s.say("charquestmenu P1 3");
    assert!(pages[0].contains("Full Quest List for"));
    assert!(pages[0].contains(&format!("admin_charquestmenu P1 {QUEST}\">{QUEST}</a>")));
    let (_, pages) = s.say("charquestmenu P1 1");
    assert!(pages[0].contains("Quests with state: <font color=\"LEVEL\">Started</font>"));
    assert!(pages[0].contains(&format!(">{QUEST}</a>")));
    let (_, pages) = s.say("charquestmenu P1 2");
    assert!(!pages[0].contains(QUEST));

    // The page of one quest: state and variables with their buttons.
    let (_, pages) = s.say(&format!("charquestmenu P1 {QUEST}"));
    assert!(pages[0].contains("State: <font color=\"LEVEL\">STARTED</font>"));
    assert!(pages[0].contains("<td>kills</td><td>3</td>"));
    assert!(pages[0].contains(&format!("admin_setcharquest P1 {QUEST} kills $varkills")));
    assert!(pages[0].contains(&format!("admin_setcharquest P1 {QUEST} state DELETE")));

    // By number: a quest the player has, one he has not, one that does not exist.
    let (_, pages) = s.say("charquestmenu P1 5 custom");
    assert!(pages[0].contains(&format!("Quest: <font color=\"LEVEL\">{QUEST}</font>")));
    assert!(pages[0].contains("<td>kills</td>"));
    s.scene.world.run_as_script(QUEST, |c| {
        let qs = c.get_quest_state(ObjectId(P1), false).unwrap();
        qs.exit_quest(c, true, false);
    });
    s.scene.log.take();
    let (_, pages) = s.say("charquestmenu P1 5 custom");
    assert!(pages[0].contains("State: <font color=\"LEVEL\">N/A</font>"));
    assert!(pages[0].contains("admin_setcharquest P1 5 state CREATE"));
    let (_, pages) = s.say("charquestmenu P1 77 custom");
    assert!(pages[0].contains("Quest with number </font><font color=\"LEVEL\">77</font>"));
}

#[test]
fn charquestmenu_bad_arguments() {
    let mut s = Setup::new();
    // No such player, and no target: INVALID_TARGET.
    for command in [
        "charquestmenu Nobody",
        "charquestmenu",
        "setcharquest Nobody a b c",
    ] {
        s.scene.cmd(GM, command);
        let out = s.scene.log.take();
        assert_eq!(admin_common::system_to(&out, GM).len(), 1, "{command}");
    }
    // A quest number that is not a number: "There was an error.".
    let (texts, pages) = s.say("charquestmenu P1 abc custom");
    assert_eq!(texts, ["There was an error."]);
    assert!(pages.is_empty());
    // A target works without the name.
    s.scene.target(GM, P1);
    s.scene.log.take();
    let (_, pages) = s.say("charquestmenu");
    assert!(pages[0].contains("Quest Menu for"));
}

// ---------------------------------------------------------------------- //setcharquest

#[test]
fn setcharquest_sets_and_deletes_a_variable_and_shows_the_page() {
    let mut s = Setup::new();
    s.start_quest();
    let (texts, pages) = s.say(&format!("setcharquest P1 {QUEST} kills 9"));
    assert_eq!(s.state_of_p1().unwrap().1.as_deref(), Some("9"));
    assert_eq!(
        s.quest_writes(),
        [QuestWrite::UpdateVar {
            char_id: P1,
            name: QUEST.to_owned(),
            var: "kills".to_owned(),
            value: "9".to_owned()
        }]
    );
    // `sendMessage("")`, then the page of the quest.
    assert_eq!(texts, [""]);
    assert!(pages[0].contains("<td>kills</td><td>9</td>"));

    s.say(&format!("setcharquest P1 {QUEST} extra 1"));
    assert!(matches!(s.quest_writes()[..], [QuestWrite::InsertVar { .. }]));
    s.say(&format!("setcharquest P1 {QUEST} extra delete"));
    assert!(matches!(s.quest_writes()[..], [QuestWrite::DeleteVar { .. }]));
}

#[test]
fn setcharquest_state_words() {
    let mut s = Setup::new();
    // CREATE by number: a started quest with cond 1.
    s.say("setcharquest P1 5 state CREATE");
    let writes = s.quest_writes();
    assert!(writes.contains(&QuestWrite::InsertVar {
        char_id: P1,
        name: QUEST.to_owned(),
        var: "<state>".to_owned(),
        value: "Started".to_owned()
    }));
    assert!(writes.contains(&QuestWrite::InsertVar {
        char_id: P1,
        name: QUEST.to_owned(),
        var: "cond".to_owned(),
        value: "1".to_owned()
    }));
    assert_eq!(s.state_of_p1().unwrap().0, STARTED_STATE);

    // COMPLETED 0: an unrepeatable quest keeps the completed state row.
    s.say(&format!("setcharquest P1 {QUEST} state COMPLETED 0"));
    assert_eq!(s.state_of_p1().unwrap().0, 2);
    let writes = s.quest_writes();
    assert!(writes.contains(&QuestWrite::DeleteQuest {
        char_id: P1,
        name: QUEST.to_owned(),
        repeatable: false
    }));

    // DELETE: the rows go (it is not started any more, so only the first delete runs: the state
    // stays in memory, like Java).
    s.say(&format!("setcharquest P1 {QUEST} state DELETE"));
    assert_eq!(
        s.quest_writes(),
        [QuestWrite::DeleteQuest {
            char_id: P1,
            name: QUEST.to_owned(),
            repeatable: true
        }]
    );

    // CC creates the state only (java-quirk), CREATE replaces it.
    s.say("setcharquest P1 5 state CC");
    assert_eq!(s.state_of_p1().unwrap().0, 0);
    s.say("setcharquest P1 5 state CREATE");
    s.quest_writes();
    // COMPLETED 1: repeatable, the state is gone.
    s.say(&format!("setcharquest P1 {QUEST} state COMPLETED 1"));
    assert!(s.state_of_p1().is_none());
}

const STARTED_STATE: u8 = 1;

#[test]
fn setcharquest_bad_arguments() {
    let mut s = Setup::new();
    // Fewer than five words: nothing happens.
    let (texts, pages) = s.say(&format!("setcharquest P1 {QUEST} kills"));
    assert!(texts.is_empty() && pages.is_empty());
    assert!(s.quest_writes().is_empty());
    // A quest the player has no state of: Java's NullPointerException reaches the GM.
    let (texts, _) = s.say(&format!("setcharquest P1 {QUEST} kills 1"));
    assert!(
        texts.iter().any(|t| t.contains("NullPointerException")),
        "{texts:?}"
    );
    // CREATE with a name instead of a number: NumberFormatException; with an unknown number: NPE.
    let (texts, _) = s.say(&format!("setcharquest P1 {QUEST} state CREATE"));
    assert!(
        texts.iter().any(|t| t.contains("NumberFormatException")),
        "{texts:?}"
    );
    let (texts, _) = s.say("setcharquest P1 77 state CREATE");
    assert!(
        texts.iter().any(|t| t.contains("NullPointerException")),
        "{texts:?}"
    );
    // COMPLETED without the repeatable flag: NPE (the flag is missing).
    s.start_quest();
    let (texts, _) = s.say(&format!("setcharquest P1 {QUEST} state COMPLETED"));
    assert!(
        texts.iter().any(|t| t.contains("NullPointerException")),
        "{texts:?}"
    );
}
