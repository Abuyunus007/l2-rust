#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! L-08: the fight calculator, the spawn reload, the GM misc room, the spawn pools, `//loadhtml`
//! from the disk, `//msgx` with display ids, `//hide` with silence and the spawn commands that write
//! the spawn file.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/{AdminFightCalculator,AdminSpawn,
//! AdminHtml,AdminMessages,AdminHide}.java, gameserver/data/xml/SpawnData.java (addNewSpawn).

mod admin_common;

use std::sync::Arc;

use admin_common::*;
use l2_data::{ItemTemplates, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessageParam;
use l2_world::*;

const RAT: i32 = 2000;

const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="2000" type="Monster" level="3" name="Rat" title="Pest">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="620" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="30" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.4" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="2001" type="Monster" level="3" name="Big Rat">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="1500" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="60" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="80" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="2002" type="Monster" level="3" name="Shown As Rat" displayId="2000">
    <stats><vitals hp="10" mp="10"/></stats>
  </npc>
  <npc id="2003" type="GrandBoss" level="80" name="Boss">
    <stats><vitals hp="10" mp="10"/></stats>
  </npc>
  <npc id="2004" type="Monster" level="9" name="Lone Rat">
    <stats><vitals hp="10" mp="10"/></stats>
  </npc>
</list>"#;

const ITEMS: &str = r#"<?xml version="1.0"?><list>
  <item id="57" type="EtcItem" name="Adena" />
  <item id="1000" type="EtcItem" name="Shown As Adena"><set name="displayId" val="57" /></item>
</list>"#;

/// Two spawn files: a plain spawn of two rats and a group.
fn write_spawns(root: &std::path::Path, plain: &str) {
    let dir = root.join("data/spawns");
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(
        dir.join("a.xml"),
        format!(r#"<?xml version="1.0"?><list enabled="true">{plain}</list>"#),
    )
    .unwrap();
}

const TERRITORY: &str = r#"<territory minZ="-3300" maxZ="-2900"><node x="-71500" y="258100"/><node x="-71200" y="258100"/>
  <node x="-71200" y="258500"/><node x="-71500" y="258500"/></territory>"#;

fn class_data() -> StatData {
    let mut levels = String::new();
    for level in 1..=40 {
        let (hp, mp, cp) = if level == 40 {
            (100, 50, 10)
        } else {
            (level, level, level)
        };
        levels.push_str(&format!(
            "<level val=\"{level}\"><hp>{hp}</hp><mp>{mp}</mp><cp>{cp}</cp></level>"
        ));
    }
    let xml = format!(
        r#"<list><classId>0</classId><staticData>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{levels}</lvlUpgainData></list>"#
    );
    StatData {
        player_templates: Arc::new(l2_data::PlayerTemplates::from_xml(&xml, 85).unwrap()),
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        ..StatData::default()
    }
}

struct Fake {
    rights: AdminRights,
    stats: StatData,
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
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
}

fn load_spawns(root: &std::path::Path, npcs: &NpcTemplates) -> Arc<SpawnData> {
    Arc::new(SpawnData::load(root, npcs, &SpawnLoadOptions::default()))
}

/// A recording outbox for a world made inside the test.
struct NullOutboxRecorder(Outs);

impl NullOutboxRecorder {
    fn new(outs: &Outs) -> Self {
        Self(outs.clone())
    }
}

impl Outbox for NullOutboxRecorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }

    fn ls_event(&mut self, event: LsEvent) {
        self.0.0.lock().unwrap().push(Out::Ls(event));
    }

    fn control(&mut self, request: ControlRequest) {
        self.0.0.lock().unwrap().push(Out::Control(request));
    }
}

fn controls(outs: &[Out]) -> Vec<ControlRequest> {
    outs.iter()
        .filter_map(|o| match o {
            Out::Control(c) => Some(c.clone()),
            _ => None,
        })
        .collect()
}

/// Builds the world of the tests: the GM is online, the templates and spawns are installed, the
/// spawns are started.
fn start(plain: &str) -> Scene {
    start_seeded(7, plain)
}

fn start_seeded(seed: u64, plain: &str) -> Scene {
    let mut s = Scene::new();
    // A world of its own with the seed, the static data and the datapack of the scene.
    let mut world = World::new(
        WorldRng::from_seed(seed),
        Box::new(NullOutboxRecorder::new(&s.outs)),
    );
    let mut rights = rights();
    for word in [
        "fight_calculator",
        "fight_calculator_show",
        "fcs",
        "respawnall",
        "spawn_reload",
        "spawn_misc",
        "spawnmisc",
        "list_pools",
        "loadhtml",
        "msgx",
        "hide",
        "spawn",
        "spawn_once",
        "spawnat",
        "circlespawn",
        "kill_menu",
    ] {
        rights.add_command(word, 100, false);
    }
    world.set_static_data(Box::new(Fake {
        rights,
        stats: class_data(),
    }));
    let mut settings = WorldSettings::default();
    settings.teleport.offset_on_teleport_enabled = false;
    settings.teleport.max_offset_on_teleport = 0;
    world.set_settings(settings);
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    write_spawns(s.root.path(), plain);
    let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
    let spawns = load_spawns(s.root.path(), &npcs);
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(l2_data::HtmCache::load(s.root.path(), false)));
    s.world = world;
    s.world.spawn_all_npcs(false);
    s.enter(GM);
    s.log.take();
    s.outs.take();
    s
}

const TWO_RATS: &str =
    r#"<spawn><npc id="2000" count="2" x="-71338" y="258271" z="-3104" respawnDelay="60"/></spawn>"#;

fn rats(s: &Scene) -> Vec<ObjectId> {
    s.world
        .npc_ids()
        .into_iter()
        .filter(|id| s.world.npc(*id).is_some_and(|n| n.npc_id == RAT))
        .collect()
}

// -------------------------------------------------------------------- fight calculator

#[test]
fn fight_calculator_walks_through_the_choice_pages() {
    let mut s = start("");
    s.cmd(GM, "fight_calculator");
    let page = html_to(&s.log.take(), GM).remove(0);
    assert_eq!(
        page,
        "<html><title>Select mobs to fight</title><body><table><tr><td>First</td><td>Second</td></tr><tr><td><edit var=\"lvl1\" width=80></td><td><edit var=\"lvl2\" width=80></td></tr></table><center><br><br><br><button value=\"OK\" action=\"bypass -h admin_fight_calculator lvl1 $lvl1 lvl2 $lvl2\"  width=100 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>"
    );

    // A level for the first mob lists the templates of the level (Rat and Big Rat, and the
    // one shown as a rat).
    s.cmd(GM, "fight_calculator lvl1 3 lvl2 9");
    let page = html_to(&s.log.take(), GM).remove(0);
    assert!(page.starts_with("<html><title>Select first mob to fight</title><body><table>"));
    assert!(page.contains(
        "<tr><td><a action=\"bypass -h admin_fight_calculator lvl1 3 lvl2 9 mid1 2000 mid2 0\">Rat</a></td></tr>"
    ));
    assert!(page.contains("mid1 2001 mid2 0\">Big Rat</a>"));
    assert!(!page.contains("Lone Rat"));

    // With the first chosen the second level is next.
    s.cmd(GM, "fight_calculator lvl1 3 lvl2 9 mid1 2000 mid2 0");
    let page = html_to(&s.log.take(), GM).remove(0);
    assert!(page.starts_with("<html><title>Select second mob to fight</title>"));
    assert!(page.contains("mid1 2000 mid2 2004\">Lone Rat</a>"));

    // Both chosen: the summary with the button to the calculation.
    s.cmd(GM, "fight_calculator lvl1 3 lvl2 3 mid1 2000 mid2 2001");
    let page = html_to(&s.log.take(), GM).remove(0);
    assert!(page.starts_with("<html><title>Selected mobs to fight</title>"));
    assert!(page.contains("<td>level 3</td><td>level 3</td>"));
    assert!(
        page.contains("<td>id 2000</td><td>id 2001</td><td>Rat</td><td>Big Rat</td>")
            || page.contains("<tr><td>Rat</td><td>Big Rat</td>")
    );
    assert!(page.contains("action=\"bypass -h admin_fight_calculator_show 2000 2001\""));

    // A value that is not a number, or missing, is the exception of the framework.
    s.cmd(GM, "fight_calculator lvl1 x");
    let texts = texts_to(&s.log.take(), GM);
    assert!(
        texts[0].contains("NumberFormatException: For input string: \"x\""),
        "{texts:?}"
    );
    s.cmd(GM, "fight_calculator lvl1");
    let texts = texts_to(&s.log.take(), GM);
    assert!(texts[0].contains("java.util.NoSuchElementException"), "{texts:?}");
}

#[test]
fn fight_calculator_show_compares_two_templates_and_deletes_them_again() {
    let mut s = start("");
    let before = s.world.npc_count();
    s.cmd(GM, "fight_calculator_show 2000 2001");
    let out = s.log.take();
    let page = html_to(&out, GM).remove(0);
    assert!(page.starts_with("<html><title>Selected mobs to fight</title><body><table>"));
    assert!(
        page.contains("<tr><td width=140>Parameter</td><td width=70>Rat</td><td width=70>Big Rat</td></tr>")
    );
    for row in [
        "miss",
        "shld",
        "crit",
        "pAtk / pDef",
        "made hits",
        "dmg per hit",
        "got dmg",
        "got regen",
        "had HP",
        "die",
    ] {
        assert!(page.contains(&format!("<tr><td>{row}</td>")), "{row} in {page}");
    }
    assert!(
        page.contains("<tr><td>had HP</td><td>620</td><td>1500</td></tr>"),
        "{page}"
    );
    assert!(page.ends_with(
        "</tr></table><center><br><button value=\"Retry\" action=\"bypass -h admin_fight_calculator_show 2000 2001\"  width=100 height=15 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>"
    ));
    // The two monsters are gone again.
    assert_eq!(s.world.npc_count(), before);

    // The same seed gives the same page; `//fcs` is the same command.
    let mut again = start("");
    again.cmd(GM, "fcs 2000 2001");
    assert_eq!(html_to(&again.log.take(), GM).remove(0), page);
}

#[test]
fn fight_calculator_show_without_ids_compares_the_gm_with_the_target() {
    let mut s = start(TWO_RATS);
    // No target: the invalid target message.
    s.cmd(GM, "fcs");
    let out = s.log.take();
    assert_eq!(system_to(&out, GM)[0].id, SystemMessageId::InvalidTarget);

    let rat = rats(&s)[0];
    s.target(GM, rat.0);
    s.log.take();
    s.cmd(GM, "fight_calculator_show");
    let page = html_to(&s.log.take(), GM).remove(0);
    assert!(
        page.contains("<tr><td width=140>Parameter</td><td width=70>me</td><td width=70>target</td></tr>")
    );
    assert!(page.contains("action=\"bypass -h admin_fight_calculator_show\"  width=100"));
    // The GM has 100 hit points, the rat 620; nothing was deleted.
    assert!(
        page.contains("<tr><td>had HP</td><td>100</td><td>620</td></tr>"),
        "{page}"
    );
    assert_eq!(rats(&s).len(), 2);
}

#[test]
fn fight_calculator_show_with_an_unknown_template_is_a_null_pointer_exception() {
    let mut s = start("");
    s.cmd(GM, "fight_calculator_show 2000 7777");
    let texts = texts_to(&s.log.take(), GM);
    assert!(texts[0].contains("java.lang.NullPointerException"), "{texts:?}");
    s.cmd(GM, "fight_calculator_show 2000");
    let texts = texts_to(&s.log.take(), GM);
    assert!(texts[0].contains("java.util.NoSuchElementException"), "{texts:?}");
    assert_eq!(s.world.npc_count(), 0);
}

// -------------------------------------------------------------------- spawn reload

#[test]
fn respawnall_deletes_everything_and_the_new_spawn_list_comes_back_from_the_server_task() {
    let mut s = start(TWO_RATS);
    assert_eq!(rats(&s).len(), 2);
    s.cmd(GM, "respawnall");
    // Everything is gone at once and the server task is asked to read the files.
    assert_eq!(s.world.npc_count(), 0);
    assert_eq!(controls(&s.outs.take()), [ControlRequest::ReloadSpawns]);
    // Nothing comes back by itself.
    s.run(120_000);
    assert_eq!(s.world.npc_count(), 0);

    // The files changed: three big rats now.
    write_spawns(
        s.root.path(),
        r#"<spawn><npc id="2001" count="3" x="-71338" y="258271" z="-3104" respawnDelay="60"/></spawn>"#,
    );
    let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
    let spawns = load_spawns(s.root.path(), &npcs);
    s.log.take();
    s.world.handle(WorldCommand::SpawnsReloaded { spawns });
    assert_eq!(s.world.npc_count(), 3);
    assert!(
        s.world
            .npc_ids()
            .iter()
            .all(|id| s.world.npc(*id).unwrap().npc_id == 2001)
    );
    assert_eq!(texts_to(&s.log.take(), GM), ["NPC respawn completed!"]);

    // The new spawns respawn like the old ones: kill one, it comes back.
    let victim = s.world.npc_ids()[0];
    s.world.delete_npc(victim);
    assert_eq!(s.world.npc_count(), 2);
    s.run(61_000);
    assert_eq!(s.world.npc_count(), 3);
}

#[test]
fn spawn_reload_is_the_same_command_and_drops_the_pending_respawns() {
    let mut s = start(TWO_RATS);
    // One rat is dead and waits for its respawn.
    let victim = rats(&s)[0];
    s.world.delete_npc(victim);
    assert!(s.world.has_pending_respawn(victim));
    s.cmd(GM, "spawn_reload");
    assert!(!s.world.has_pending_respawn(victim));
    assert_eq!(controls(&s.outs.take()), [ControlRequest::ReloadSpawns]);
    // The old respawn timer does not fire into the new list.
    let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
    let spawns = load_spawns(s.root.path(), &npcs);
    s.world.handle(WorldCommand::SpawnsReloaded { spawns });
    assert_eq!(s.world.npc_count(), 2);
    s.run(200_000);
    assert_eq!(s.world.npc_count(), 2);
}

// -------------------------------------------------------------------- GM misc room

#[test]
fn spawn_misc_spawns_one_immobile_npc_of_each_template_and_takes_the_gm_there() {
    let mut s = start("");
    s.cmd(GM, "spawn_misc");
    let texts = texts_to(&s.log.take(), GM);
    assert!(texts[0].starts_with("WARNING: There are several known issues regarding this feature."));
    // Rat, Big Rat and Lone Rat; the template shown as another id and the grand boss are skipped.
    let mut ids: Vec<i32> = s
        .world
        .npc_ids()
        .into_iter()
        .map(|id| s.world.npc(id).unwrap().npc_id)
        .collect();
    ids.sort_unstable();
    assert_eq!(ids, [2000, 2001, 2004]);
    // 200 apart on the first row, no respawn, immobilised.
    let first = s
        .world
        .npc_ids()
        .into_iter()
        .find(|id| s.world.npc(*id).unwrap().npc_id == 2000)
        .unwrap();
    assert_eq!(s.world.position_of(first), Some((-128_670, -230_115, -15_536)));
    assert!(s.world.is_immobilized(first));
    let second = s
        .world
        .npc_ids()
        .into_iter()
        .find(|id| s.world.npc(*id).unwrap().npc_id == 2001)
        .unwrap();
    assert_eq!(s.world.position_of(second), Some((-128_470, -230_115, -15_536)));
    // Killed, a misc room NPC does not come back.
    s.world.delete_npc(first);
    s.run(120_000);
    assert_eq!(s.world.npc_count(), 2);
    // `spawnmisc` is the same word; only the exact words count.
    s.cmd(GM, "spawnmisc");
    assert_eq!(s.world.npc_count(), 5);
    s.cmd(GM, "spawn_misc now");
    assert_eq!(s.world.npc_count(), 5);
}

// -------------------------------------------------------------------- spawn pools

#[test]
fn list_pools_prints_the_groups_with_their_counts() {
    let mut s = start("");
    s.cmd(GM, "list_pools");
    assert_eq!(texts_to(&s.log.take(), GM), ["There are no spawn groups."]);

    let group = format!(
        r#"<spawn zone="pool1" selection="all" maximumNpc="5">{TERRITORY}
             <npc id="2000" count="3" respawnDelay="10"/><npc id="2001" count="2" respawnDelay="10"/></spawn>"#
    );
    let mut s = start(&group);
    assert_eq!(s.world.npc_count(), 5);
    s.cmd(GM, "list_pools");
    let texts = texts_to(&s.log.take(), GM);
    assert_eq!(texts.len(), 3, "{texts:?}");
    assert!(texts[0].ends_with("[ALL cap 5] alive 5"), "{texts:?}");
    assert_eq!(texts[1], "- Rat (2000) 3/3 in 1 territories");
    assert_eq!(texts[2], "- Big Rat (2001) 2/2 in 1 territories");
}

// -------------------------------------------------------------------- loadhtml

#[test]
fn loadhtml_asks_the_server_task_for_the_file_and_shows_what_comes_back() {
    let mut s = start("");
    s.cmd(GM, "loadhtml");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usage: //loadhtml path"]);
    assert!(s.outs.take().is_empty());

    s.cmd(GM, "loadhtml data/html/admin/gm_menu.htm");
    assert_eq!(
        controls(&s.outs.take()),
        [ControlRequest::LoadHtmlFile {
            requester: ObjectId(GM),
            path: "data/html/admin/gm_menu.htm".to_owned()
        }]
    );
    // Nothing is shown until the file is read.
    assert!(html_to(&s.log.take(), GM).is_empty());
    s.world.handle(WorldCommand::HtmlFileLoaded {
        requester: ObjectId(GM),
        path: "data/html/admin/gm_menu.htm".to_owned(),
        content: Some("<html><body>FROM DISK</body></html>".to_owned()),
    });
    assert_eq!(
        html_to(&s.log.take(), GM),
        ["<html><body>FROM DISK</body></html>"]
    );

    s.world.handle(WorldCommand::HtmlFileLoaded {
        requester: ObjectId(GM),
        path: "data/html/nope.htm".to_owned(),
        content: None,
    });
    assert_eq!(
        html_to(&s.log.take(), GM),
        ["<html><body>My text is missing:<br>data/html/nope.htm</body></html>"]
    );
}

// -------------------------------------------------------------------- msgx

#[test]
fn msgx_adds_an_item_shown_as_another_item_by_its_name() {
    let mut s = start("");
    s.cmd(GM, "msgx 61 item:57 item:1000");
    let messages = system_to(&s.log.take(), GM);
    assert_eq!(messages.len(), 1);
    assert_eq!(
        messages[0].params,
        [
            SystemMessageParam::ItemName(57),
            // Item 1000 is shown to the client as 57, so Java adds its name as text.
            SystemMessageParam::Text("Shown As Adena".to_owned()),
        ]
    );
    // An item the table does not have is Java's NullPointerException in the parameter's catch:
    // the exception line first, the message without the parameter after it.
    s.cmd(GM, "msgx 61 item:424242");
    let out = s.log.take();
    assert!(texts_to(&out, GM)[0].starts_with("Exception: Cannot invoke"));
    assert!(system_to(&out, GM)[0].params.is_empty());
}

// -------------------------------------------------------------------- hide

#[test]
fn hide_also_switches_silence_mode() {
    let mut s = start("");
    s.enter(P1);
    s.log.take();
    s.cmd(GM, "hide on");
    assert!(s.world.player(ObjectId(GM)).unwrap().gm.silence);
    assert_eq!(texts_to(&s.log.take(), GM), ["Now, you cannot be seen."]);
    // Hiding again changes nothing.
    s.cmd(GM, "hide on");
    assert!(s.world.player(ObjectId(GM)).unwrap().gm.silence);
    s.cmd(GM, "hide off");
    assert!(!s.world.player(ObjectId(GM)).unwrap().gm.silence);
}

// -------------------------------------------------------------------- permanent spawns

fn stored(outs: &[Out]) -> Vec<l2_data::spawn_store::NewSpawn> {
    controls(outs)
        .into_iter()
        .filter_map(|c| match c {
            ControlRequest::StoreSpawn { spawn } => Some(spawn),
            _ => None,
        })
        .collect()
}

#[test]
fn permanent_spawns_are_handed_to_the_server_task_to_be_written() {
    let mut s = start("");
    // `//spawn` is permanent: the GM's place, the amount and the delay as typed.
    s.cmd(GM, "spawn 2000 2 30");
    let spawns = stored(&s.outs.take());
    assert_eq!(
        spawns,
        [l2_data::spawn_store::NewSpawn {
            npc_id: 2000,
            amount: 2,
            x: X + GM,
            y: Y,
            z: Z,
            heading: 0,
            respawn_delay_secs: 30,
            label: "Rat - Pest".to_owned(),
        }]
    );

    // `//spawn_once` is not permanent, nothing is written.
    s.cmd(GM, "spawn_once 2001");
    assert!(stored(&s.outs.take()).is_empty());

    // `//spawnat` writes with the fixed delay of 60.
    s.cmd(GM, "spawnat 2001 10 20 30");
    let spawns = stored(&s.outs.take());
    assert_eq!(spawns.len(), 1);
    assert_eq!((spawns[0].x, spawns[0].y, spawns[0].z), (10, 20, 30));
    assert_eq!(
        (spawns[0].respawn_delay_secs, spawns[0].label.as_str()),
        (60, "Big Rat")
    );

    // `//circlespawn` writes each of its spawns unless the last word says false.
    s.cmd(GM, "circlespawn 2001 3 100 60");
    assert_eq!(stored(&s.outs.take()).len(), 3);
    s.cmd(GM, "circlespawn 2001 3 100 60 false");
    assert!(stored(&s.outs.take()).is_empty());
}
