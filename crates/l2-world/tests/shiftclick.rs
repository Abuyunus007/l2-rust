#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The GM shift click (S-HAND-01): `Action` with `actionId` 1 on an NPC, a door, a static object, an
//! item on the ground, a player and a summon. Java:
//! dist/game/data/scripts/handlers/actions/shiftclick/*.java. The pages are small files in a
//! temporary datapack root; the shipped `npcinfo.htm` and `doorinfo.htm` are checked for left over
//! placeholders when the reference folder exists.

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{
    DoorTemplates, HtmCache, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions, StaticObjectTemplates,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{NpcHtmlMessage, StaticObjectInfo};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) -> Sent {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

struct Data {
    rights: AdminRights,
    stats: StatData,
}

/// Class 0 at level 40 with 100 HP, and the adena item.
fn stat_data() -> StatData {
    let mut levels = String::new();
    for level in 1..=40 {
        levels.push_str(&format!(
            "<level val=\"{level}\"><hp>100</hp><mp>50</mp><cp>10</cp></level>"
        ));
    }
    let class = format!(
        "<list><classId>0</classId><staticData><collisionMale><radius>9.0</radius><height>23.0</height></collisionMale><collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale></staticData><lvlUpgainData>{levels}</lvlUpgainData></list>"
    );
    StatData {
        player_templates: Arc::new(l2_data::PlayerTemplates::from_xml(&class, 85).unwrap()),
        items: Arc::new(
            l2_data::ItemTemplates::from_xml(
                r#"<list><item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item></list>"#,
            )
            .unwrap(),
        ),
        ..StatData::default()
    }
}

impl StaticData for Data {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        Some(ItemStatic {
            display_id: 57,
            type2: 5,
            body_part: 0,
            quest_item: false,
            weight: 0,
            shared_reuse_group: -1,
            weapon: false,
        })
    }
    fn item_stackable(&self, _item_id: i32) -> bool {
        true
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        false
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        if level >= 100 {
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

fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (100, 0)] {
        rights.add_level(level, child);
    }
    for command in ["character_info", "summon_info"] {
        rights.add_command(command, 100, false);
    }
    rights
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decoded<P: Decode>(out: &Sent, to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| {
            let skip = if b[0] == 0xFE { 3 } else { 1 };
            P::decode(&mut PacketReader::new(&b[skip..])).unwrap()
        })
        .collect()
}

fn html_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|m| m.html)
        .collect()
}

fn names_to(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, _)| *t == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const GM: i32 = 0x1000_0001;
const PLAYER: i32 = 0x1000_0002;
const GHOUL: i32 = 20_001;
const VILLAGER: i32 = 1_001;

/// A monster with a clan range and a folk NPC.
const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="20001" type="Monster" level="10" name="Ghoul">
  <race>UNDEAD</race>
  <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
    <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
    <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
    <defence physical="44.44444" magical="29.5" />
    <speed><walk ground="40" /><run ground="100" /></speed>
  </stats>
  <ai aggroRange="300" clanHelpRange="400" />
  <collision><radius normal="10"/><height normal="20"/></collision>
</npc>
<npc id="1001" type="Folk" level="10" name="Villager" />
</list>"#;

/// The same placeholders as the shipped pages, one per line.
const NPCINFO: &str = "NPC %objid% %class% %race% %id% %lvl% %name% %tmplid% aggro=%aggro% hp=%hp%/%hpmax% mp=%mp%/%mpmax% \
patk=%patk% matk=%matk% pdef=%pdef% mdef=%mdef% accu=%accu% evas=%evas% crit=%crit% rspd=%rspd% aspd=%aspd% cspd=%cspd% \
type=%atkType% rng=%atkRng% str=%str% dex=%dex% con=%con% int=%int% wit=%wit% men=%men% loc=%loc% heading=%heading% \
radius=%collision_radius% height=%collision_height% d2=%loc2d% d3=%loc3d% \
ele=%ele_atk%/%ele_atk_value% f=%ele_dfire% w=%ele_dwater% wi=%ele_dwind% e=%ele_dearth% h=%ele_dholy% d=%ele_ddark% \
terr=%territory% stype=%spawntype% file=%spawnfile% spawn=%spawn% resp=%resp% \
[%ai_intention%][%ai%][%ai_type%][%ai_clan%][%ai_enemy_clan%][%route%]";

/// The `%name%` words left in a page (a `%` inside `width="100%"` is no placeholder).
fn left_over_placeholders(html: &str) -> Vec<String> {
    let bytes = html.as_bytes();
    let mut found = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%' {
            let rest = &html[i + 1..];
            let word: String = rest
                .chars()
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                .collect();
            if !word.is_empty() && rest[word.len()..].starts_with('%') {
                found.push(format!("%{word}%"));
                i += word.len() + 2;
                continue;
            }
        }
        i += 1;
    }
    found
}

const DOORINFO: &str = "DOOR %class% hp=%hp%/%hpmax% obj=%objid% id=%doorid% min=%minx% %miny% %minz% max=%maxx% %maxy% %maxz% unlock=%unlock%";

struct Scene {
    world: World,
    log: Log,
    root: tempfile::TempDir,
}

fn pdata(id: i32, name: &str, access_level: i32) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: name.to_owned(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 40,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level,
        x: X,
        y: Y,
        z: Z,
        heading: 321,
        cur_hp: 100.0,
        max_hp: 100.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 10.0,
        max_cp: 10.0,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

impl Scene {
    /// A Ghoul with a spawn (60 seconds), a Villager without one, a door, a static object and an
    /// item on the ground, all near the GM; a plain player enters too.
    fn new(npcinfo: &str, doorinfo: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write("data/html/admin/npcinfo.htm", npcinfo);
        write("data/html/admin/doorinfo.htm", doorinfo);
        write(
            "data/html/admin/charinfo.htm",
            "<html><body>CHARINFO %name%</body></html>",
        );
        write(
            "data/html/admin/petinfo.htm",
            "<html><body>PETINFO %name%</body></html>",
        );
        write(
            "data/spawns/ghouls.xml",
            &format!(
                r#"<?xml version="1.0"?><list enabled="true"><spawn><npc id="{GHOUL}" x="{}" y="{Y}" z="{Z}" heading="0" respawnDelay="60" /></spawn></list>"#,
                X + 100
            ),
        );
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(11), Box::new(NullOutbox));
        world.set_static_data(Box::new(Data {
            rights: rights(),
            stats: stat_data(),
        }));
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        world.set_class_list(Arc::new(
            l2_data::ClassList::from_xml(r#"<list><class classId="0" name="Human Fighter"/></list>"#)
                .unwrap(),
        ));
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.spawn_all_npcs(false);
        world
            .spawn_doors(
                &DoorTemplates::from_xml(&format!(
                    r#"<list><door id="101" name="d" height="150" baseHpMax="1000" pos="{},{Y},{Z}"
                    node1="{},{}" node2="{},{}" node3="{},{}" node4="{},{}" nodeZ="{}" open_method="8"/></list>"#,
                    X + 30,
                    X + 20,
                    Y - 50,
                    X + 40,
                    Y - 50,
                    X + 40,
                    Y + 50,
                    X + 20,
                    Y + 50,
                    Z - 10
                ))
                .unwrap(),
                &ids,
            )
            .unwrap();
        world
            .spawn_static_objects(
                &StaticObjectTemplates::from_xml(&format!(
                    r#"<list><object id="19210010" name="throne" type="1" x="{}" y="{Y}" z="{Z}"/></list>"#,
                    X + 50
                ))
                .unwrap(),
                &ids,
            )
            .unwrap();
        world.load_ground_items(vec![GroundItemRow {
            object_id: ObjectId(0x6100_0001),
            item_id: 57,
            count: 5,
            enchant_level: 0,
            x: X + 60,
            y: Y,
            z: Z,
            drop_time: -1,
            equipable: false,
        }]);
        let mut scene = Self { world, log, root };
        scene.enter(pdata(GM, "Gamemaster", 100));
        scene.enter(pdata(PLAYER, "Pawn", 0));
        scene.log.take();
        scene
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    /// `Action(objectId, actionId)` of a player.
    fn action(&mut self, player: i32, object: ObjectId, action_id: i8) -> Sent {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input: ClientInput::Action {
                object_id: object.0,
                action_id,
            },
        });
        self.log.take()
    }

    fn npc(&self, npc_id: i32) -> ObjectId {
        self.world
            .npc_ids()
            .into_iter()
            .find(|id| self.world.npc(*id).unwrap().npc_id == npc_id)
            .unwrap()
    }
}

// ---------------------------------------------------------------------------------- tests

/// `NpcShiftClick`: the target is set, the page has the values of the NPC, and the click ends with
/// `ActionFailed`.
#[test]
fn a_gm_shift_click_on_a_monster_opens_npcinfo() {
    let mut s = Scene::new(NPCINFO, DOORINFO);
    let ghoul = s.npc(GHOUL);
    let out = s.action(GM, ghoul, 1);
    assert_eq!(s.world.target_of(ObjectId(GM)), Some(ghoul));
    let names = names_to(&out, GM);
    assert!(names.contains(&"NPC_HTML_MESSAGE".to_owned()), "{names:?}");
    assert_eq!(names.last().map(String::as_str), Some("ACTION_FAIL"));

    let html = html_to(&out, GM).pop().unwrap();
    let view = s.world.stats(ghoul).unwrap();
    let expect = |text: &str| assert!(html.contains(text), "missing {text:?} in {html}");
    expect(&format!("NPC {} Monster UNDEAD 20001 10 Ghoul 20001", ghoul.0));
    // The monster's `aggroRange` and its life.
    expect("aggro=300 hp=3000/3000");
    expect(&format!(
        "patk={} matk={} pdef={} mdef={}",
        view.p_atk(None) as i32,
        view.m_atk(None, None) as i32,
        view.p_def(None) as i32,
        view.m_def(None, None) as i32
    ));
    expect(&format!(
        "accu={} evas={} crit={} rspd={} aspd={:?} cspd={}",
        view.accuracy(),
        view.evasion_rate(None),
        view.critical_hit(None, None),
        view.run_speed() as i32,
        view.p_atk_spd(),
        view.m_atk_spd()
    ));
    expect("type=SWORD rng=40 str=40 dex=30 con=43 int=21 wit=20 men=20");
    expect(&format!("loc={} {Y} {Z} heading=", X + 100));
    expect("radius=10.0 height=20.0");
    // 100 away in x only; the GM is at the same height.
    expect("d2=100 d3=100");
    // No element: `getAttackElement()` is -2 and `Elementals.getElementName` says "None".
    expect("ele=None/0 f=0 w=0 wi=0 e=0 h=0 d=0");
    // The spawn: no territory, a fixed place, 60 seconds, the file name.
    expect(&format!(
        "terr=None stype=Fixed file=ghouls.xml spawn={} {Y} {Z} resp=60 sec",
        X + 100
    ));
}

/// The AI rows exist once the NPC has an AI (`hasAI()`): a monster of a region with a player gets
/// one. Their text is Java's, with the intention name.
#[test]
fn the_ai_rows_follow_has_ai() {
    let mut s = Scene::new(NPCINFO, DOORINFO);
    let ghoul = s.npc(GHOUL);
    let out = s.action(GM, ghoul, 1);
    let html = html_to(&out, GM).pop().unwrap();
    if s.world.has_ai(ghoul) {
        assert!(html.contains("<font color=FFAA00>Intention:</font></td><td align=right width=170>"));
        assert!(
            html.contains("<font color=FFAA00>AI</font></td><td align=right width=170>AttackableAI</td>")
        );
        assert!(html.contains("<font color=FFAA00>AIType</font></td><td align=right width=170>FIGHTER</td>"));
        assert!(
            html.contains("<font color=FFAA00>Clan & Range:</font></td><td align=right width=170> 400</td>")
        );
        assert!(
            html.contains(
                "<font color=FFAA00>Ignore & Range:</font></td><td align=right width=170> 300</td>"
            )
        );
    } else {
        assert!(html.contains("[][][][][][]"), "no AI: six empty rows in {html}");
    }
}

/// An NPC with no spawn (`getSpawn() == null`) shows the red dashes.
#[test]
fn an_npc_without_a_spawn_shows_dashes() {
    let mut s = Scene::new(NPCINFO, DOORINFO);
    let villager = s.world.spawn_npc_at(VILLAGER, X + 10, Y, Z, 0).unwrap();
    let out = s.action(GM, villager, 1);
    let html = html_to(&out, GM).pop().unwrap();
    assert!(html.contains("NPC "), "{html}");
    assert!(html.contains(" Folk "));
    let dash = "<font color=FF0000>--</font>";
    assert!(
        html.contains(&format!(
            "terr={dash} stype={dash} file={dash} spawn=<font color=FF0000>null</font> resp={dash}"
        )),
        "{html}"
    );
    // A folk NPC is not attackable: aggro 0.
    assert!(html.contains("aggro=0 "));
}

/// The pages of the shipped datapack have no placeholder that the handler does not fill.
#[test]
fn the_shipped_pages_are_filled_completely() {
    let dist = Path::new(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../dist/game/data/html/admin"
    ));
    let (Ok(npcinfo), Ok(doorinfo)) = (
        std::fs::read_to_string(dist.join("npcinfo.htm")),
        std::fs::read_to_string(dist.join("doorinfo.htm")),
    ) else {
        eprintln!("skipped: the reference datapack is not at {}", dist.display());
        return;
    };
    let mut s = Scene::new(&npcinfo, &doorinfo);
    let ghoul = s.npc(GHOUL);
    let html = html_to(&s.action(GM, ghoul, 1), GM).pop().unwrap();
    assert!(
        left_over_placeholders(&html).is_empty(),
        "left over placeholders in {html}"
    );
    assert!(html.contains("Npc Info"));

    let door = s.world.doors().next().unwrap().object_id;
    let html = html_to(&s.action(GM, door, 1), GM).pop().unwrap();
    assert!(left_over_placeholders(&html).is_empty(), "{html}");
    assert!(html.contains("GM Door Info"));
}

/// `DoorShiftClick`: `StaticObjectInfo` (always targetable for a GM) then the page.
#[test]
fn a_gm_shift_click_on_a_door() {
    let mut s = Scene::new(NPCINFO, DOORINFO);
    let door = s.world.doors().next().unwrap().object_id;
    let out = s.action(GM, door, 1);
    assert_eq!(s.world.target_of(ObjectId(GM)), Some(door));
    let names = names_to(&out, GM);
    let info_at = names.iter().position(|n| n == "STATIC_OBJECT").unwrap();
    let html_at = names.iter().position(|n| n == "NPC_HTML_MESSAGE").unwrap();
    assert!(info_at < html_at);
    assert_eq!(names.last().map(String::as_str), Some("ACTION_FAIL"));
    let info = decoded::<StaticObjectInfo>(&out, GM, "STATIC_OBJECT")
        .pop()
        .unwrap();
    assert_eq!(info.object_id, door.0);
    assert!(info.targetable);

    // `open_method="8"` is `OPEN_BY_SKILL`: Unlockable YES. The corners are node 1 and node 3.
    let html = html_to(&out, GM).pop().unwrap();
    assert_eq!(
        html,
        format!(
            "<html><body>DOOR Door hp=1000/1000 obj={} id=101 min={} {} {} max={} {} {} unlock=<font color=00FF00>YES<font></body></html>",
            door.0,
            X + 20,
            Y - 50,
            Z - 10,
            X + 40,
            Y + 50,
            Z - 10 + 150,
        )
    );
}

/// `StaticObjectShiftClick` and `ItemShiftClick`: the small pages with Java's text.
#[test]
fn static_object_and_item_pages() {
    let mut s = Scene::new(NPCINFO, DOORINFO);
    let object = s.world.static_objects().next().unwrap().object_id;
    let out = s.action(GM, object, 1);
    assert_eq!(s.world.target_of(ObjectId(GM)), Some(object));
    assert_eq!(decoded::<StaticObjectInfo>(&out, GM, "STATIC_OBJECT").len(), 1);
    let html = html_to(&out, GM).pop().unwrap();
    assert_eq!(
        html,
        format!(
            "<html><body><center><font color=\"LEVEL\">Static Object Info</font></center><br><table border=0><tr><td>Coords X,Y,Z: </td><td>{}, {Y}, {Z}</td></tr><tr><td>Object ID: </td><td>{}</td></tr><tr><td>Static Object ID: </td><td>19210010</td></tr><tr><td>Mesh Index: </td><td>0</td></tr><tr><td><br></td></tr><tr><td>Class: </td><td>StaticObject</td></tr></table></body></html>",
            X + 50,
            object.0
        )
    );

    // The item: no target change (Java's handler does not set it), the page with the ids.
    let item = s.world.ground_item_ids().into_iter().next().unwrap();
    let before = s.world.target_of(ObjectId(GM));
    let out = s.action(GM, item, 1);
    assert_eq!(s.world.target_of(ObjectId(GM)), before);
    let html = html_to(&out, GM).pop().unwrap();
    assert!(html.starts_with("<html><head><title>"), "{html}");
    assert!(html.contains(&format!("<td>Object ID: </td><td>{}</td>", item.0)));
    assert!(html.contains("<td>Item ID: </td><td>57</td>"));
    assert!(html.contains(&format!(
        "<td>Location: </td><td>[Location] X: {} Y: {Y} Z: {Z} Heading: 0 InstanceId: 0</td>",
        X + 60
    )));
    assert!(html.ends_with("<td>Class: </td><td>Item</td></tr></table></body></html>"));
}

/// `PlayerShiftClick` and `SummonShiftClick` run `admin_character_info <name>` and
/// `admin_summon_info`, which need the GM rights and a target.
#[test]
fn player_shift_click_runs_character_info() {
    let mut s = Scene::new(NPCINFO, DOORINFO);
    let out = s.action(GM, ObjectId(PLAYER), 1);
    assert_eq!(s.world.target_of(ObjectId(GM)), Some(ObjectId(PLAYER)));
    assert!(
        html_to(&out, GM).iter().any(|h| h.contains("CHARINFO Pawn")),
        "{:?}",
        html_to(&out, GM)
    );
    assert_eq!(names_to(&out, GM).last().map(String::as_str), Some("ACTION_FAIL"));
}

/// A player who is not a GM gets the normal click (`AltGameViewNpc` is off): no GM window, and the
/// NPC is only targeted.
#[test]
fn a_player_shift_click_is_a_plain_click() {
    let mut s = Scene::new(NPCINFO, DOORINFO);
    let ghoul = s.npc(GHOUL);
    let out = s.action(PLAYER, ghoul, 1);
    assert!(html_to(&out, PLAYER).is_empty());
    assert_eq!(s.world.target_of(ObjectId(PLAYER)), Some(ghoul));
    let _ = &s.root;
}
