#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The character admin commands of `AdminEditChar` (`admin/edit_char.rs`), with a fake GM and
//! fake players.
//! Java: dist/game/data/scripts/handlers/chat/commands/admin/AdminEditChar.java
//!
//! The admin pages are small files in a temporary datapack root.

use std::net::IpAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{ClassList, HtmCache, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
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

/// Keeps every persistence command, so a test can see the stores and the recommendation rows.
#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

struct RecordingOutbox(Persisted);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }

    fn ls_event(&mut self, _event: LsEvent) {}
}

/// The levels of the shipped `AccessLevels.xml` (only 70 and 100 are GMs) and the rights of the
/// commands these tests use, with the levels of the shipped `AdminCommands.xml`.
fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [
        (-1, 0),
        (0, 0),
        (10, 0),
        (20, 10),
        (30, 20),
        (40, 30),
        (50, 40),
        (60, 50),
        (70, 60),
        (100, 70),
    ] {
        rights.add_level(level, child);
        rights.set_level_name(level, &format!("Lvl{level}"));
    }
    for (command, level, confirm) in [
        ("current_player", 30, false),
        ("character_info", 30, false),
        ("character_list", 30, false),
        ("show_characters", 30, false),
        ("find_character", 30, false),
        ("find_ip", 30, false),
        ("find_account", 30, false),
        ("find_dualbox", 30, false),
        ("edit_character", 100, false),
        ("setfame", 100, false),
        ("rec", 100, false),
        ("setclass", 100, false),
        ("settitle", 100, false),
        ("changename", 100, false),
        ("setsex", 100, false),
        ("settcolor", 100, false),
        ("fullfood", 100, false),
        ("setnoble", 100, true),
        ("set_hp", 100, false),
        ("set_mp", 100, false),
        ("set_cp", 100, false),
        ("setcolor", 100, false),
        ("tracert", 100, false),
        ("strict_find_dualbox", 100, false),
        ("changelvl", 100, false),
        ("repair", 100, false),
        ("restore", 100, false),
        ("gmliston", 100, false),
        ("gmlistoff", 100, false),
        ("silence", 100, false),
        ("diet", 100, false),
        ("tradeoff", 100, false),
        ("setconfig", 100, false),
        ("config_server", 100, false),
    ] {
        rights.add_command(command, level, confirm);
    }
    rights
}

struct Fake {
    rights: AdminRights,
    stats: StatData,
}

impl Fake {
    fn new(rights: AdminRights) -> Self {
        Self {
            rights,
            stats: class_data(),
        }
    }
}

/// The class templates of the test characters: class 0 and the Kamael classes 123 and 124, level 40
/// with 100 HP, 50 MP and 10 CP. No `statBonus.xml`: every bonus is 1.
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
    let template = |id: i32| {
        format!(
            "<list><classId>{id}</classId><staticData>\
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>\
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>\
</staticData><lvlUpgainData>{levels}</lvlUpgainData></list>"
        )
    };
    let xml = template(0);
    StatData {
        player_templates: Arc::new(l2_data::PlayerTemplates::from_xml(&xml, 85).unwrap()),
        ..StatData::default()
    }
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

/// `classList.xml` of the test classes: Human Fighter, Warrior, and the Kamael soldiers.
const CLASS_LIST: &str = r#"<list>
  <class classId="0" name="Human Fighter"/>
  <class classId="1" name="Warrior" parentClassId="0"/>
  <class classId="123" name="Kamael Soldier"/>
  <class classId="124" name="Kamael Soldier Female"/>
</list>"#;

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn decoded<P: Decode>(out: &Sent, to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

fn html_to(out: &Sent, to: i32) -> Vec<NpcHtmlMessage> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
}

/// The text of every `sendMessage` (system message `S1_2` with one string) sent to `to`.
fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The ids of the system messages (those with no string) sent to `to`.
fn system_ids_to(out: &Sent, to: i32) -> Vec<SystemMessageId> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter(|m| m.id != SystemMessageId::S12)
        .map(|m| m.id)
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

const GM: i32 = 100;
const P1: i32 = 1;
const P2: i32 = 2;
const P3: i32 = 3;

const ACCESS_REFUSED: &str = "You don't have the access rights to use this command!";

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="1001" type="Folk" level="10" name="Villager" usingServerSideName="true"/>
</list>"#;

/// The pages the tests read. They carry the placeholders the handlers replace.
const CHARINFO: &str = "<html><body>INFO %name% %level% %classid% %class% %ip% %currenthp% \
%maxhp% %noblesse% %access% %percent% %inst%</body></html>";
const CHARFIND: &str = "<html><body>FIND %results% %number% %end%</body></html>";
const CHARLIST: &str = "<html><body>LIST %pages% %players%</body></html>";
const IPFIND: &str = "<html><body>IP %ip% %results% %number% %end%</body></html>";
const DUALBOX: &str = "<html><body>DUAL %multibox% %results% %strict%</body></html>";

struct Root(tempfile::TempDir);

impl Root {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = dir.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        write("data/html/admin/charinfo.htm", CHARINFO);
        write("data/html/admin/charedit.htm", CHARINFO);
        write("data/html/admin/charfind.htm", CHARFIND);
        write("data/html/admin/charlist.htm", CHARLIST);
        write("data/html/admin/ipfind.htm", IPFIND);
        write("data/html/admin/dualbox.htm", DUALBOX);
        write("data/html/admin/gm_menu.htm", "<html><body>GM MENU</body></html>");
        write(
            "data/html/admin/accountinfo.htm",
            "<html><body>ACC %account% %player% %characters%</body></html>",
        );
        write(
            "data/html/admin/setclass/human_fighter.htm",
            "<html><body>Class menu</body></html>",
        );
        Self(dir)
    }

    fn path(&self) -> &Path {
        self.0.path()
    }
}

struct Setup {
    world: World,
    log: Log,
    persisted: Persisted,
    _root: Root,
}

fn setup() -> Setup {
    let root = Root::new();
    let log = Log::default();
    let persisted = Persisted::default();
    let mut world = World::new(
        WorldRng::from_seed(11),
        Box::new(RecordingOutbox(persisted.clone())),
    );
    world.set_static_data(Box::new(Fake::new(rights())));
    let mut settings = WorldSettings::default();
    settings.teleport.offset_on_teleport_enabled = false;
    settings.teleport.max_offset_on_teleport = 0;
    world.set_settings(settings);
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_class_list(Arc::new(ClassList::from_xml(CLASS_LIST).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    Setup {
        world,
        log,
        persisted,
        _root: root,
    }
}

fn player(id: i32, access_level: i32, name: &str) -> PlayerData {
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

impl Setup {
    /// The GM (access level 100) is `GM`; the players are `P1`, `P2`, `P3`.
    fn enter_gm(&mut self) {
        self.enter_with(player(GM, 100, "Gamemaster"), None);
    }

    fn enter_player(&mut self, id: i32, name: &str, ip: Option<&str>) {
        let ip = ip.map(|text| text.parse::<IpAddr>().unwrap());
        self.enter_with(player(id, 0, name), ip);
    }

    fn enter_with(&mut self, data: PlayerData, ip: Option<IpAddr>) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip,
        });
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    /// `//command`.
    fn cmd(&mut self, player: i32, command: &str) {
        self.input(
            player,
            ClientInput::BuildCmd {
                command: command.to_owned(),
            },
        );
    }

    /// The target of a player: `RequestTargetCanceld`-free `Target` input of the object.
    fn target(&mut self, player: i32, target: i32) {
        self.world.set_target(ObjectId(player), Some(ObjectId(target)));
    }

    fn p(&self, id: i32) -> &Player {
        self.world.player(ObjectId(id)).unwrap()
    }

    fn take(&self) -> Sent {
        self.log.take()
    }

    fn persist_count(&self) -> usize {
        self.persisted.0.lock().unwrap().len()
    }
}

// ------------------------------------------------------------------------------ registration

/// Every word of the card that this module registers is a known command: it never answers "does
/// not exist". The words left out (`tracert`, `strict_find_dualbox`, `setcolor`) do.
#[test]
fn registered_words_exist_and_left_out_ones_do_not() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    for word in [
        "current_player",
        "character_info",
        "character_list",
        "show_characters",
        "find_character",
        "find_ip",
        "find_account",
        "edit_character",
        "find_dualbox",
        "setfame",
        "rec",
        "setclass",
        "settitle",
        "changename",
        "setsex",
        "settcolor",
        "fullfood",
        "setnoble",
        "set_hp",
        "set_mp",
        "set_cp",
        "setcolor",
        "tracert",
        "strict_find_dualbox",
    ] {
        s.cmd(GM, word);
        let texts = texts_to(&s.take(), GM);
        assert!(
            texts.iter().all(|t| !t.starts_with("The command")),
            "{word} must be registered, got {texts:?}"
        );
    }
}

// ------------------------------------------------------------------------------- the edits

#[test]
fn setfame_sets_the_fame_and_tells_both() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.cmd(GM, "setfame 1234");
    assert_eq!(s.p(P1).data.profile.fame, 1234);
    let out = s.take();
    assert_eq!(
        texts_to(&out, P1),
        vec!["A GM changed your Reputation points to 1234".to_owned()]
    );
    assert_eq!(texts_to(&out, GM), vec!["P1's Fame changed to 1234".to_owned()]);
}

#[test]
fn setfame_without_a_number_is_the_usage_line() {
    let mut s = setup();
    s.enter_gm();
    s.cmd(GM, "setfame");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["Usage: //setfame <new_fame_value>".to_owned()]
    );
}

#[test]
fn rec_sets_the_recommendations_and_stores_them() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.cmd(GM, "rec 7");
    assert_eq!(s.p(P1).recom_have, 7);
    let out = s.take();
    assert_eq!(
        texts_to(&out, P1),
        vec!["A GM changed your Recommend points to 7".to_owned()]
    );
    assert_eq!(texts_to(&out, GM), vec!["P1's Recommend changed to 7".to_owned()]);
    let stored = s.persisted.0.lock().unwrap();
    assert!(
        stored.iter().any(|c| matches!(
            c,
            PersistCommand::RecoValues { owner, rec_have: 7, .. } if *owner == ObjectId(P1)
        )),
        "the counters are written"
    );
}

#[test]
fn settitle_cuts_a_long_title_like_java() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    // Longer than 21 characters is cut to 20 (`Creature.setTitle`, `length() > 21`); 21 stays.
    s.cmd(GM, "settitle 1234567890123456789012");
    assert_eq!(s.p(P1).data.title, "12345678901234567890");
    s.cmd(GM, "settitle 123456789012345678901");
    assert_eq!(s.p(P1).data.title, "123456789012345678901");
    s.cmd(GM, "settitle 12345678901234567890");
    assert_eq!(s.p(P1).data.title, "12345678901234567890");
    s.cmd(GM, "settitle Hero");
    assert_eq!(s.p(P1).data.title, "Hero");
    assert!(texts_to(&s.take(), P1).contains(&"Your title has been changed by a GM".to_owned()));
}

#[test]
fn settitle_without_text_asks_for_it() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.cmd(GM, "settitle");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["You need to specify the new title.".to_owned()]
    );
}

#[test]
fn settcolor_takes_a_hex_colour() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.cmd(GM, "settcolor FF00FF");
    assert_eq!(s.p(P1).data.profile.title_color, 0xFF00FF);
    s.cmd(GM, "settcolor -5");
    assert_eq!(s.p(P1).data.profile.title_color, 0xFF00FF);
    let out = s.take();
    assert_eq!(
        texts_to(&out, GM),
        vec!["You need to specify a valid new color.".to_owned()]
    );
    assert!(texts_to(&out, P1).contains(&"Your title color has been changed by a GM".to_owned()));
}

#[test]
fn set_hp_sets_the_vital_of_the_target() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.cmd(GM, "set_hp 55");
    assert_eq!(s.world.player(ObjectId(P1)).unwrap().core.cur_hp as i32, 55);
    s.cmd(GM, "set_mp 20");
    assert_eq!(s.world.player(ObjectId(P1)).unwrap().core.cur_mp as i32, 20);
    s.cmd(GM, "set_cp 3");
    assert_eq!(s.world.player(ObjectId(P1)).unwrap().core.cur_cp as i32, 3);
    s.cmd(GM, "set_hp abc");
    assert_eq!(texts_to(&s.take(), GM), vec!["Usage: //set_hp 1000".to_owned()]);
}

#[test]
fn set_hp_without_a_creature_target_is_the_invalid_target() {
    let mut s = setup();
    s.enter_gm();
    s.take();
    s.cmd(GM, "set_hp 10");
    assert_eq!(system_ids_to(&s.take(), GM), vec![SystemMessageId::InvalidTarget]);
}

#[test]
fn setsex_flips_the_sex_of_the_target() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.cmd(GM, "setsex");
    assert_eq!(s.p(P1).data.sex, 1);
    s.cmd(GM, "setsex");
    assert_eq!(s.p(P1).data.sex, 0);
    assert!(texts_to(&s.take(), P1).contains(&"Your gender has been changed by a GM".to_owned()));
}

#[test]
fn setnoble_with_no_target_is_the_gm_and_asks_first() {
    let mut s = setup();
    s.enter_gm();
    // `setnoble` has `confirmDlg="true"`: the command waits for the answer.
    s.cmd(GM, "setnoble");
    assert!(!s.p(GM).data.profile.noble, "not changed before the answer");
    s.world.admin_command(ObjectId(GM), "admin_setnoble", false);
    assert!(s.p(GM).data.profile.noble);
    assert!(texts_to(&s.take(), GM).contains(&"GM changed your nobless status!".to_owned()));
}

/// The framework runs an admin command only for a GM: a plain player gets nothing and the command
/// does not run (Java `AdminCommandHandler` is not reached for a non-GM).
#[test]
fn settitle_is_not_run_for_a_plain_player() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(P1, P2);
    s.enter_player(P2, "P2", None);
    s.take();
    s.cmd(P1, "settitle nope");
    assert!(s.take().iter().all(|(to, _)| *to != P1));
    assert_eq!(s.p(P2).data.title, "");
}

#[test]
fn changename_refuses_a_taken_name_and_stores_the_new_one() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.enter_player(P2, "P2", None);
    s.target(GM, P1);
    s.cmd(GM, "changename P2");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["Warning, player P2 already exists".to_owned()]
    );
    assert_eq!(s.p(P1).data.name, "P1");

    let before = s.persist_count();
    s.cmd(GM, "changename Renamed");
    assert_eq!(s.p(P1).data.name, "Renamed");
    assert!(s.persist_count() > before, "the character is stored");
    let out = s.take();
    assert_eq!(texts_to(&out, GM), vec!["Changed name to Renamed".to_owned()]);
    assert!(texts_to(&out, P1).contains(&"Your name has been changed by a GM.".to_owned()));
}

#[test]
fn setclass_changes_the_class_and_the_base_class() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.cmd(GM, "setclass 1");
    assert_eq!(s.p(P1).data.class_id, 1);
    assert_eq!(s.p(P1).data.base_class, 1);
    let out = s.take();
    assert!(texts_to(&out, P1).contains(&"A GM changed your class to Warrior.".to_owned()));
    assert_eq!(texts_to(&out, GM), vec!["P1 is a Warrior.".to_owned()]);
}

#[test]
fn setclass_refuses_an_unknown_or_the_same_class() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.cmd(GM, "setclass 99999");
    s.cmd(GM, "setclass 0");
    s.cmd(GM, "setclass abc");
    assert_eq!(s.p(P1).data.class_id, 0);
    assert_eq!(
        texts_to(&s.take(), GM),
        vec![
            "Usage: //setclass <valid_new_classid>".to_owned(),
            "Usage: //setclass <valid_new_classid>".to_owned(),
            "Usage: //setclass <valid_new_classid>".to_owned(),
        ]
    );
}

#[test]
fn setclass_of_a_kamael_follows_the_class_sex() {
    let mut s = setup();
    s.enter_gm();
    let mut kamael = player(P1, 0, "P1");
    kamael.race = 5;
    kamael.sex = 1;
    s.enter_with(kamael, None);
    s.target(GM, P1);
    s.cmd(GM, "setclass 123");
    assert_eq!(s.p(P1).data.class_id, 123);
    assert_eq!(
        s.p(P1).data.sex,
        0,
        "a male Kamael class makes the character male"
    );
}

#[test]
fn setclass_without_a_player_target_does_nothing() {
    let mut s = setup();
    s.enter_gm();
    s.take();
    s.cmd(GM, "setclass 1");
    assert!(s.take().iter().all(|(to, _)| *to != GM));
}

#[test]
fn set_hp_is_not_run_for_a_plain_player() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(P1, GM);
    s.take();
    s.cmd(P1, "set_hp 1");
    assert!(s.take().iter().all(|(to, _)| *to != P1));
    assert_eq!(s.world.player(ObjectId(GM)).unwrap().core.cur_hp as i32, 100);
}

/// A GM without the right for the command gets Java's refusal (level 70 may not `//set_hp`).
#[test]
fn a_gm_without_the_right_is_refused() {
    let mut s = setup();
    s.enter_player(P1, "P1", None);
    s.enter_with(player(P2, 70, "P2"), None);
    s.take();
    s.cmd(P2, "set_hp 1");
    assert_eq!(texts_to(&s.take(), P2), vec![ACCESS_REFUSED.to_owned()]);
}

// ------------------------------------------------------------------------------ the pages

#[test]
fn current_player_shows_the_gm_and_makes_it_the_target() {
    let mut s = setup();
    s.enter_gm();
    s.cmd(GM, "current_player");
    let out = s.take();
    let html = html_to(&out, GM);
    assert_eq!(html.len(), 1, "one page to the GM");
    let text = &html[0].html;
    assert!(text.contains("INFO Gamemaster 40"), "name and level: {text}");
    assert!(text.contains("100 100"), "current and max hp: {text}");
    assert!(text.contains(" No "), "the noble flag: {text}");
    assert!(!text.contains("%percent%"), "every placeholder is replaced");
    assert_eq!(s.world.target_of(ObjectId(GM)), Some(ObjectId(GM)));
}

#[test]
fn character_info_names_a_player_or_uses_the_target() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", Some("10.0.0.5"));
    s.cmd(GM, "character_info P1");
    let out = s.take();
    let html = html_to(&out, GM);
    assert_eq!(html.len(), 1);
    assert!(html[0].html.contains("INFO P1 40"), "{}", html[0].html);
    assert!(
        html[0].html.contains("10.0.0.5"),
        "the client address: {}",
        html[0].html
    );
    assert_eq!(s.world.target_of(ObjectId(GM)), Some(ObjectId(P1)));

    // Without a name and without a target: the invalid target.
    s.world.set_target(ObjectId(GM), None);
    s.cmd(GM, "character_info");
    assert_eq!(system_ids_to(&s.take(), GM), vec![SystemMessageId::InvalidTarget]);
}

#[test]
fn edit_character_of_an_offline_name_shows_nothing() {
    let mut s = setup();
    s.enter_gm();
    s.cmd(GM, "edit_character Nobody");
    assert!(html_to(&s.take(), GM).is_empty(), "no page for an unknown name");
}

#[test]
fn find_character_lists_the_matching_names() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "Alpha", None);
    s.enter_player(P2, "Beta", None);
    s.cmd(GM, "find_character alp");
    let out = s.take();
    let html = html_to(&out, GM);
    assert_eq!(html.len(), 1);
    let text = &html[0].html;
    assert!(text.contains("admin_character_info Alpha"), "{text}");
    assert!(!text.contains("Beta"), "{text}");
    assert!(text.contains("FIND "), "{text}");
    assert!(text.contains(" 1 ") && text.contains("."), "one result: {text}");
}

#[test]
fn find_character_without_a_name_is_the_usage_and_the_list() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "Alpha", None);
    s.cmd(GM, "find_character");
    let out = s.take();
    assert!(texts_to(&out, GM).contains(&"Usage: //find_character <character_name>".to_owned()));
    assert_eq!(html_to(&out, GM).len(), 1, "the list follows");
}

#[test]
fn character_list_shows_the_players_with_their_names() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "Alpha", None);
    s.cmd(GM, "character_list");
    let out = s.take();
    let html = html_to(&out, GM);
    assert_eq!(html.len(), 1);
    assert!(
        html[0].html.contains("admin_character_info Alpha"),
        "{}",
        html[0].html
    );
    assert!(
        html[0].html.contains("Human Fighter"),
        "the class name: {}",
        html[0].html
    );
}

#[test]
fn show_characters_needs_a_page_number() {
    let mut s = setup();
    s.enter_gm();
    s.cmd(GM, "show_characters");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["Usage: //show_characters <page_number>".to_owned()]
    );
}

#[test]
fn find_ip_lists_the_players_on_an_address() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "Alpha", Some("10.0.0.5"));
    s.enter_player(P2, "Beta", Some("10.0.0.5"));
    s.enter_player(P3, "Gamma", Some("10.0.0.9"));
    s.cmd(GM, "find_ip 10.0.0.5");
    let html = html_to(&s.take(), GM);
    assert_eq!(html.len(), 1);
    let text = &html[0].html;
    assert!(text.contains("IP 10.0.0.5"), "{text}");
    assert!(text.contains("admin_character_info Alpha") && text.contains("admin_character_info Beta"));
    assert!(!text.contains("Gamma"), "{text}");
    assert!(text.contains(" 2 "), "two results: {text}");
}

#[test]
fn find_ip_refuses_a_malformed_address() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "Alpha", None);
    s.cmd(GM, "find_ip 300.1.1.1");
    let out = s.take();
    assert!(texts_to(&out, GM).contains(&"Usage: //find_ip <www.xxx.yyy.zzz>".to_owned()));
    assert_eq!(html_to(&out, GM).len(), 1, "the list follows");
}

#[test]
fn find_dualbox_lists_the_addresses_with_two_players() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "Alpha", Some("10.0.0.5"));
    s.enter_player(P2, "Beta", Some("10.0.0.5"));
    s.enter_player(P3, "Gamma", Some("10.0.0.9"));
    s.cmd(GM, "find_dualbox");
    let html = html_to(&s.take(), GM);
    assert_eq!(html.len(), 1);
    let text = &html[0].html;
    assert!(text.contains("DUAL 2 "), "{text}");
    assert!(text.contains("admin_find_ip 10.0.0.5\">10.0.0.5 (2)"), "{text}");
    assert!(!text.contains("10.0.0.9"), "one player is no dualbox: {text}");
}

#[test]
fn find_dualbox_refuses_a_count_below_one() {
    let mut s = setup();
    s.enter_gm();
    s.cmd(GM, "find_dualbox 0");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["Usage: //find_dualbox [number > 0]".to_owned()]
    );
}

#[test]
fn find_account_needs_an_online_player() {
    let mut s = setup();
    s.enter_gm();
    s.cmd(GM, "find_account Nobody");
    let out = s.take();
    assert!(texts_to(&out, GM).contains(&"Usage: //find_account <player_name>".to_owned()));
}

#[test]
fn find_account_shows_the_account_of_the_player() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "Alpha", None);
    s.cmd(GM, "find_account Alpha");
    let html = html_to(&s.take(), GM);
    assert_eq!(html.len(), 1);
    assert!(html[0].html.contains("ACC acc1 Alpha"), "{}", html[0].html);
}

// --------------------------------------------------------------------------- M7-ADMIN-12

fn html_texts(out: &Sent, to: i32) -> Vec<String> {
    html_to(out, to).into_iter().map(|m| m.html).collect()
}

#[test]
fn setcolor_sets_the_name_colour_of_the_target() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.take();
    s.cmd(GM, "setcolor FF00FF");
    assert_eq!(s.p(P1).gm.name_color, Some(0x00FF_00FF));
    assert!(texts_to(&s.take(), P1).contains(&"Your name color has been changed by a GM".to_owned()));
    s.cmd(GM, "setcolor zz");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["You need to specify a valid new color.".to_owned()]
    );
    assert_eq!(s.p(P1).gm.name_color, Some(0x00FF_00FF));
}

#[test]
fn tracert_prints_the_five_hops() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", Some("10.0.0.1"));
    s.input(
        P1,
        ClientInput::Tracert {
            hops: [[1, 2, 3, 4], [5, 6, 7, 8], [0; 4], [0; 4], [9, 9, 9, 9]],
        },
    );
    s.take();
    s.cmd(GM, "tracert P1");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec![
            "Hop0: 1.2.3.4".to_owned(),
            "Hop1: 5.6.7.8".to_owned(),
            "Hop2: 0.0.0.0".to_owned(),
            "Hop3: 0.0.0.0".to_owned(),
            "Hop4: 9.9.9.9".to_owned(),
        ]
    );
    // No name and no player target: the invalid target message.
    s.cmd(GM, "tracert");
    assert_eq!(system_ids_to(&s.take(), GM), vec![SystemMessageId::InvalidTarget]);
}

#[test]
fn strict_find_dualbox_needs_the_same_hops_as_well() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", Some("10.0.0.1"));
    s.enter_player(P2, "P2", Some("10.0.0.1"));
    s.enter_player(P3, "P3", Some("10.0.0.1"));
    let hops = [[1, 1, 1, 1]; 5];
    s.input(P1, ClientInput::Tracert { hops });
    s.input(P2, ClientInput::Tracert { hops });
    s.input(
        P3,
        ClientInput::Tracert {
            hops: [[2, 2, 2, 2]; 5],
        },
    );
    s.take();
    s.cmd(GM, "strict_find_dualbox");
    let pages = html_texts(&s.take(), GM);
    assert_eq!(pages.len(), 1);
    // P1 and P2 share the pack, P3 stands alone: one entry with a count of 2.
    assert!(pages[0].contains("10.0.0.1 (2)"), "{}", pages[0]);
    assert!(pages[0].contains("strict_"), "{}", pages[0]);
    s.cmd(GM, "strict_find_dualbox 0");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["Usage: //strict_find_dualbox [number > 0]".to_owned()]
    );
}

#[test]
fn gmliston_and_gmlistoff_change_the_list() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.take();
    // A GM starts hidden: the public list is empty for a player.
    s.input(P1, ClientInput::GmList);
    assert_eq!(
        system_ids_to(&s.take(), P1),
        vec![SystemMessageId::ThereAreNoGmsCurrentlyVisibleInThePublicListAsTheyMayBePerformingOtherFunctionsAtTheMoment]
    );
    s.cmd(GM, "gmliston");
    let out = s.take();
    assert_eq!(texts_to(&out, GM), vec!["Registered into GM list.".to_owned()]);
    assert_eq!(html_texts(&out, GM).len(), 1);
    s.input(P1, ClientInput::GmList);
    let out = s.take();
    assert_eq!(
        system_ids_to(&out, P1),
        vec![SystemMessageId::GmList, SystemMessageId::GmC1]
    );
    s.cmd(GM, "gmlistoff");
    assert_eq!(texts_to(&s.take(), GM), vec!["Removed from GM list.".to_owned()]);
}

#[test]
fn silence_toggles_and_blocks_whispers() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.take();
    s.cmd(GM, "silence");
    assert!(s.p(GM).gm.silence);
    assert!(system_ids_to(&s.take(), GM).contains(&SystemMessageId::MessageRefusalMode));
    // A whisper to the silent GM is answered as if he was offline.
    s.input(
        P1,
        ClientInput::Say {
            chat_type: 2,
            text: "hi".into(),
            target: Some("Gamemaster".into()),
        },
    );
    let out = s.take();
    assert_eq!(
        system_ids_to(&out, P1),
        vec![SystemMessageId::ThatPlayerIsNotOnline]
    );
    assert!(out.iter().all(|(to, _)| *to != GM));
    s.cmd(GM, "silence");
    assert!(!s.p(GM).gm.silence);
    assert!(system_ids_to(&s.take(), GM).contains(&SystemMessageId::MessageAcceptanceMode));
}

#[test]
fn diet_follows_the_java_word_quirk() {
    let mut s = setup();
    s.enter_gm();
    s.take();
    s.cmd(GM, "diet on");
    assert!(s.p(GM).gm.diet);
    assert!(texts_to(&s.take(), GM).contains(&"Diet mode on".to_owned()));
    // `//diet off` has no third word, so it toggles (here: off).
    s.cmd(GM, "diet off");
    assert!(!s.p(GM).gm.diet);
    assert!(texts_to(&s.take(), GM).contains(&"Diet mode off".to_owned()));
    // And once more `//diet off` toggles back on.
    s.cmd(GM, "diet off");
    assert!(s.p(GM).gm.diet);
    s.take();
    // A second word that is not "on" with a third that is not "off": nothing changes.
    s.cmd(GM, "diet x y");
    assert!(s.p(GM).gm.diet);
    assert!(texts_to(&s.take(), GM).is_empty());
    s.cmd(GM, "diet x off");
    assert!(!s.p(GM).gm.diet);
}

#[test]
fn tradeoff_sets_and_toggles_the_refusal() {
    let mut s = setup();
    s.enter_gm();
    s.take();
    s.cmd(GM, "tradeoff on");
    assert!(s.p(GM).trade.refusal);
    assert!(texts_to(&s.take(), GM).contains(&"Trade refusal enabled".to_owned()));
    s.cmd(GM, "tradeoff off");
    assert!(!s.p(GM).trade.refusal);
    assert!(texts_to(&s.take(), GM).contains(&"Trade refusal disabled".to_owned()));
    // No mode: toggles.
    s.cmd(GM, "tradeoff");
    assert!(s.p(GM).trade.refusal);
    // An unknown mode: nothing.
    s.take();
    s.cmd(GM, "tradeoff maybe");
    assert!(s.p(GM).trade.refusal);
    assert!(texts_to(&s.take(), GM).is_empty());
}

#[test]
fn changelvl_changes_the_target_or_a_named_character() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.take();
    s.cmd(GM, "changelvl 70");
    assert_eq!(s.p(P1).data.access_level, 70);
    let out = s.take();
    assert_eq!(
        texts_to(&out, P1),
        vec!["Your access level has been changed to Lvl70 (70).".to_owned()]
    );
    assert_eq!(
        texts_to(&out, GM),
        vec!["P1's access level has been changed to Lvl70 (70).".to_owned()]
    );
    s.cmd(GM, "changelvl 55");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec![
            "You are trying to set unexisting access level: 55 please try again with a valid one!".to_owned()
        ]
    );
    // A bad number with a target: the usage line.
    s.cmd(GM, "changelvl x");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["Usage: //changelvl <target_new_level> | <player_name> <new_level>".to_owned()]
    );
    // By name, an online player.
    s.cmd(GM, "changelvl P1 0");
    assert_eq!(s.p(P1).data.access_level, 0);
    // Unknown name.
    s.take();
    s.cmd(GM, "changelvl Nobody 10");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["Character not found or access level unaltered.".to_owned()]
    );
    // A ban: the player is told and leaves the world.
    s.cmd(GM, "changelvl -1");
    let out = s.take();
    assert_eq!(
        texts_to(&out, P1),
        vec!["Your character has been banned. Bye.".to_owned()]
    );
    assert!(s.world.player(ObjectId(P1)).is_none());
}

#[test]
fn repair_and_restore_queue_the_statements() {
    let mut s = setup();
    s.enter_gm();
    let before = s.persist_count();
    s.cmd(GM, "repair Stuck");
    s.cmd(GM, "restore Other");
    // Wrong number of words: nothing.
    s.cmd(GM, "repair");
    s.cmd(GM, "repair a b");
    let cmds = s.persisted.0.lock().unwrap();
    let names: Vec<&str> = cmds[before..]
        .iter()
        .filter_map(|c| match c {
            PersistCommand::RepairChar { name } => Some(name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(names, vec!["Stuck", "Other"]);
}

#[test]
fn setconfig_changes_a_rate_and_shows_the_page() {
    let mut s = setup();
    s.enter_gm();
    s.take();
    s.cmd(GM, "setconfig RateXp 5");
    let out = s.take();
    assert_eq!(
        texts_to(&out, GM),
        vec!["Config parameter RateXp set to 5".to_owned()]
    );
    let pages = html_texts(&out, GM);
    assert_eq!(pages.len(), 1);
    assert!(pages[0].contains("Rate EXP</font> = 5.0"), "{}", pages[0]);
    s.cmd(GM, "setconfig RateXp");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["Usage: //setconfig <parameter> <value>".to_owned()]
    );
    s.cmd(GM, "setconfig RateXp abc");
    assert_eq!(
        texts_to(&s.take(), GM),
        vec!["Usage: //setconfig <parameter> <value>".to_owned()]
    );
    s.cmd(GM, "config_server");
    let pages = html_texts(&s.take(), GM);
    assert!(pages[0].contains("Config Server Panel"));
    assert!(pages[0].contains("Rate EXP</font> = 5.0"));
}

#[test]
fn setclass_tells_the_player_about_the_transfer() {
    let mut s = setup();
    s.enter_gm();
    s.enter_player(P1, "P1", None);
    s.target(GM, P1);
    s.take();
    s.cmd(GM, "setclass 1");
    let out = s.take();
    assert!(
        system_ids_to(&out, P1).contains(&SystemMessageId::CongratulationsYouVeCompletedAClassTransfer),
        "{:?}",
        system_ids_to(&out, P1)
    );
    assert_eq!(s.p(P1).data.class_id, 1);
}
