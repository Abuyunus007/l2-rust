#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The social and summon admin commands (M6-ADMIN-05) with a fake GM, a fake player and a fake
//! clan member: karma, pk and pvp, the party and clan recall menus, `//partyinfo`, the clan
//! commands (`//clan_info`, `//clan_changeleader`, `//clan_show_pending`, `//clan_force_pending`,
//! `//pledge`, `//remove_clan_penalty`), the summon words, the transformation words, and the
//! skill buttons that were left over from M5-FU-01 (`//get_skills`, `//reset_skills`,
//! `//give_all_skills_fs`, `//setskill`, `//removereuse`, `//buff`).
//! Java: scripts/handlers/chat/commands/admin/{AdminEditChar,AdminClan,AdminPledge,AdminMenu,
//! AdminSummon,AdminTransform,AdminRide,AdminSkill,AdminBuffs,AdminKill}.java.
//!
//! The harness is the one of `admin_combat.rs`; the clan fixtures are loaded before the players
//! enter, as the clan tests do.

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{
    ClassList, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates, SkillTemplates, SpawnData,
    SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{GmViewPledgeInfo, NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord, PlayerClanState};
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

/// The levels of the shipped `AccessLevels.xml` (only 70 and 100 are GMs), and the rights of the
/// commands these tests use. The tests call `admin_command(.., false)`, so the confirmations of
/// `AdminCommands.xml` do not matter here.
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
    }
    for command in [
        "nokarma",
        "setkarma",
        "setpk",
        "setpvp",
        "set_pvp_flag",
        "partyinfo",
        "recall_party_menu",
        "recall_clan_menu",
        "clan_info",
        "clan_changeleader",
        "clan_show_pending",
        "clan_force_pending",
        "pledge",
        "remove_clan_penalty",
        "summon",
        "summon_info",
        "unsummon",
        "summon_setlvl",
        "show_pet_inv",
        "transform",
        "untransform",
        "transform_menu",
        "ride_horse",
        "ride_bike",
        "unride",
        "unride_wyvern",
        "unride_strider",
        "unride_wolf",
        "get_skills",
        "reset_skills",
        "give_all_skills_fs",
        "setskill",
        "removereuse",
        "buff",
        "spawn_once",
        "create_item",
    ] {
        rights.add_command(command, 100, false);
    }
    rights
}

/// The class template of the test characters (class 0, level 40), and the one passive skill the
/// skill tests use: `Reflect` (3100, one level).
const SKILLS: &str = r##"<list>
<skill id="3100" levels="1" name="Reflect"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="reflectDam" val="50"/></effect></effects></skill>
</list>"##;

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
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
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
    fn item_name(&self, _item_id: i32) -> Option<String> {
        None
    }
    fn item_stackable(&self, _item_id: i32) -> bool {
        false
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.stats
            .skills
            .iter()
            .any(|s| s.id == skill_id && s.is_passive())
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

fn decoded<P: Decode>(out: &Sent, to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

/// The text of every `sendMessage` / `sendSysMessage` (a system message with one string) to `to`.
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

/// A system message with the given id and parameters sent to `to`.
fn has_system(out: &Sent, to: i32, id: SystemMessageId, params: &[SystemMessageParam]) -> bool {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .iter()
        .any(|m| m.id == id && m.params == params)
}

/// The admin pages (`NPC_HTML_MESSAGE`) sent to `to`.
fn html_to(out: &Sent, to: i32) -> Vec<NpcHtmlMessage> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
}

/// The text of an admin page sent to `to` (the first one).
fn page_html(out: &Sent, to: i32) -> String {
    html_to(out, to)
        .first()
        .map(|m| m.html.clone())
        .unwrap_or_default()
}

/// The admin html pages the tests need: the ones the handlers show, with recognisable content.
const CLAN_INFO_PAGE: &str =
    "<html><body>%clan_name%|%clan_leader%|%clan_level%|%clan_players_count%|%clan_ally%</body></html>";
const CLAN_CHANGES_PAGE: &str = "<html><body>%data%</body></html>";
const PARTY_INFO_PAGE: &str = "<html><body>%party_loot%%player%%party%%recall_party%</body></html>";

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
        write(
            "data/html/admin/game_menu.htm",
            "<html><body>Game menu</body></html>",
        );
        write(
            "data/html/admin/charmanage.htm",
            "<html><body>Char manage</body></html>",
        );
        write(
            "data/html/admin/transform.htm",
            "<html><body>Transform</body></html>",
        );
        write("data/html/admin/claninfo.htm", CLAN_INFO_PAGE);
        write("data/html/admin/clanchanges.htm", CLAN_CHANGES_PAGE);
        write("data/html/admin/partyinfo.htm", PARTY_INFO_PAGE);
        write(
            "data/html/admin/petinfo.htm",
            "<html><body>%name% %owner% %inv%</body></html>",
        );
        write(
            "data/html/admin/charskills.htm",
            "<html><body>%name%</body></html>",
        );
        Self(dir)
    }

    fn path(&self) -> &Path {
        self.0.path()
    }
}

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="1001" type="Folk" level="10" name="Villager" usingServerSideName="true"/>
</list>"#;

struct Setup {
    world: World,
    log: Log,
    _root: Root,
}

fn setup() -> Setup {
    let mut settings = WorldSettings::default();
    // Exact places: the random offset of a teleport has its own tests.
    settings.teleport.offset_on_teleport_enabled = false;
    settings.teleport.max_offset_on_teleport = 0;
    let root = Root::new();
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(13), Box::new(NullOutbox));
    world.set_static_data(Box::new(Fake {
        rights: rights(),
        stats: class_data(),
    }));
    world.set_settings(settings);
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    world.set_class_list(Arc::new(
        ClassList::from_xml(r#"<list><class classId="0" name="Human Fighter" /></list>"#).unwrap(),
    ));
    Setup {
        world,
        log,
        _root: root,
    }
}

const GM: i32 = 100;
const USER: i32 = 1;
const MEMBER: i32 = 2;
const OTHER: i32 = 3;
const CLAN: i32 = 500;

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

fn player(id: i32, access_level: i32, dx: i32) -> PlayerData {
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
        level: 40,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level,
        x: X + dx,
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
    /// A player whose access level is its object id (the GM is 100, the others are not GMs).
    fn enter(&mut self, id: i32, dx: i32) {
        let level = if id == GM { 100 } else { 0 };
        self.enter_with(player(id, level, dx));
    }

    /// A player of the clan `CLAN` with the given pledge grade.
    fn enter_member(&mut self, id: i32, power_grade: i32) {
        let mut data = player(id, 0, 300);
        data.profile.clan = Some(PlayerClanState {
            clan_id: CLAN,
            power_grade,
            ..PlayerClanState::default()
        });
        self.enter_with(data);
    }

    fn enter_with(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn target(&mut self, gm: i32, target: Option<i32>) {
        self.world.set_target(ObjectId(gm), target.map(ObjectId));
    }

    fn karma(&self, id: i32) -> i32 {
        self.world.player(ObjectId(id)).unwrap().data.karma
    }

    fn skills(&self, id: i32) -> Vec<(i32, i32)> {
        self.world.player(ObjectId(id)).unwrap().core.skills.clone()
    }

    fn position(&self, id: i32) -> Option<(i32, i32, i32)> {
        self.world.position_of(ObjectId(id))
    }
}

/// `//command` as the admin panels run it: the confirmation is already answered (`false`).
fn run(s: &mut Setup, gm: i32, command: &str) {
    s.world.admin_command(ObjectId(gm), command, false);
}

/// Lions (`CLAN`): `leader` is the leader, the members of the list are its members, level 5.
fn lions(leader: i32, members: &[(i32, i32)]) -> ClanRecord {
    let member = |id: i32, power_grade: i32| ClanMemberRecord {
        object_id: id,
        name: format!("P{id}"),
        level: 40,
        class_id: 0,
        title: String::new(),
        power_grade,
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
        female: false,
        race: 0,
    };
    ClanRecord {
        clan_id: CLAN,
        data: Some(ClanDataRecord {
            name: Some("Lions".to_owned()),
            level: 5,
            leader_id: leader,
            ..ClanDataRecord::default()
        }),
        members: members.iter().map(|(id, grade)| member(*id, *grade)).collect(),
        ..ClanRecord::default()
    }
}

// ----------------------------------------------------------------------- karma, pk and pvp

#[test]
fn setkarma_sets_the_target_karma_and_tells_the_target_and_the_gm() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_setkarma 250");
    let out = s.log.take();
    assert_eq!(s.karma(USER), 250);
    assert!(has_system(
        &out,
        USER,
        SystemMessageId::YourKarmaHasBeenChangedToS1,
        &[SystemMessageParam::IntNumber(250)]
    ));
    assert_eq!(
        texts_to(&out, GM),
        ["Successfully Changed karma for P1 from (0) to (250)."]
    );
}

#[test]
fn setkarma_refuses_a_negative_value_and_a_missing_number_is_the_usage() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_setkarma -5");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["You must enter a value for karma greater than or equal to 0."]
    );
    assert_eq!(s.karma(USER), 0);

    run(&mut s, GM, "admin_setkarma");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //setkarma <new_karma_value>"]
    );
}

#[test]
fn nokarma_sets_the_karma_to_zero_and_is_silent_without_a_player_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.target(GM, Some(USER));
    run(&mut s, GM, "admin_setkarma 40");
    s.log.take();

    run(&mut s, GM, "admin_nokarma");
    let out = s.log.take();
    assert_eq!(s.karma(USER), 0);
    assert_eq!(
        texts_to(&out, GM),
        ["Successfully Changed karma for P1 from (40) to (0)."]
    );

    // Without a target Java returns before anything is sent.
    s.target(GM, None);
    s.log.take();
    run(&mut s, GM, "admin_nokarma");
    assert!(s.log.take().is_empty());
}

#[test]
fn setpk_and_setpvp_set_the_counts_and_tell_both_sides() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_setpk 7");
    let out = s.log.take();
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().data.pk_kills, 7);
    assert_eq!(texts_to(&out, USER), ["A GM changed your PK count to 7"]);
    assert_eq!(texts_to(&out, GM), ["P1's PK count changed to 7"]);

    run(&mut s, GM, "admin_setpvp 3");
    let out = s.log.take();
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().data.pvp_kills, 3);
    assert_eq!(texts_to(&out, USER), ["A GM changed your PVP count to 3"]);
    assert_eq!(texts_to(&out, GM), ["P1's PVP count changed to 3"]);
}

#[test]
fn setpk_needs_a_number_and_a_player_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.log.take();

    run(&mut s, GM, "admin_setpk x");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usage: //setpk <pk_count>"]);

    run(&mut s, GM, "admin_setpk 2");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);
}

#[test]
fn set_pvp_flag_toggles_the_flag_of_the_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.target(GM, Some(USER));
    s.log.take();

    assert_eq!(s.world.pvp_flag_of(ObjectId(USER)), 0);
    run(&mut s, GM, "admin_set_pvp_flag");
    assert_eq!(s.world.pvp_flag_of(ObjectId(USER)), 1);
    run(&mut s, GM, "admin_set_pvp_flag");
    assert_eq!(s.world.pvp_flag_of(ObjectId(USER)), 0);
}

// ---------------------------------------------------------------------------- party and recall

#[test]
fn partyinfo_of_a_character_without_a_party_says_so() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_partyinfo");
    assert_eq!(texts_to(&s.log.take(), GM), ["P1 is not in a party."]);

    // By name, from anywhere.
    s.target(GM, None);
    run(&mut s, GM, "admin_partyinfo P1");
    assert_eq!(texts_to(&s.log.take(), GM), ["P1 is not in a party."]);

    // No target and no name: Java's INVALID_TARGET.
    run(&mut s, GM, "admin_partyinfo");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);
}

#[test]
fn recall_party_menu_without_a_name_is_the_usage() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();
    run(&mut s, GM, "admin_recall_party_menu");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //recall_party_menu <playerName>"]
    );
}

#[test]
fn recall_party_menu_brings_a_character_outside_a_party_to_the_gm() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    let gm_place = s.position(GM).unwrap();
    s.log.take();

    run(&mut s, GM, "admin_recall_party_menu P1");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Player is not in party."]);
    assert_eq!(texts_to(&out, USER), ["Admin is teleporting you."]);
    // The ground height of the teleport may change Z; the place is the GM's X and Y.
    assert_eq!(
        s.position(USER).map(|(x, y, _)| (x, y)),
        Some((gm_place.0, gm_place.1))
    );
    assert!(!html_to(&out, GM).is_empty());
}

#[test]
fn recall_party_menu_of_an_unknown_name_is_an_invalid_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();
    run(&mut s, GM, "admin_recall_party_menu Nobody");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);
}

#[test]
fn recall_clan_menu_brings_a_character_without_a_clan_to_the_gm() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    let gm_place = s.position(GM).unwrap();
    s.log.take();

    run(&mut s, GM, "admin_recall_clan_menu P1");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Player is not in a clan."]);
    assert_eq!(
        s.position(USER).map(|(x, y, _)| (x, y)),
        Some((gm_place.0, gm_place.1))
    );

    run(&mut s, GM, "admin_recall_clan_menu");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //recall_clan_menu <playerName>"]
    );
}

// ------------------------------------------------------------------------------------- clans

#[test]
fn clan_info_shows_the_clan_of_the_target_in_claninfo() {
    let mut s = setup();
    s.world
        .load_clans(vec![lions(USER, &[(USER, 1), (MEMBER, 5)])], Vec::new());
    s.enter(GM, 0);
    s.enter_member(USER, 1);
    s.enter_member(MEMBER, 5);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_clan_info");
    let out = s.log.take();
    assert_eq!(
        page_html(&out, GM),
        "<html><body>Lions|P1|5|2|Not in ally</body></html>"
    );
}

#[test]
fn clan_info_of_a_character_without_a_clan_is_refused() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(OTHER, 300);
    s.target(GM, Some(OTHER));
    s.log.take();

    run(&mut s, GM, "admin_clan_info");
    let out = s.log.take();
    assert_eq!(
        system_ids_to(&out, GM),
        [SystemMessageId::TheTargetMustBeAClanMember]
    );
    assert!(html_to(&out, GM).is_empty());
}

#[test]
fn clan_changeleader_makes_the_target_the_leader() {
    let mut s = setup();
    s.world
        .load_clans(vec![lions(USER, &[(USER, 1), (MEMBER, 5)])], Vec::new());
    s.enter(GM, 0);
    s.enter_member(USER, 1);
    s.enter_member(MEMBER, 5);
    s.target(GM, Some(MEMBER));
    s.log.take();

    run(&mut s, GM, "admin_clan_changeleader");
    assert_eq!(s.world.clan(CLAN).unwrap().leader_id(), MEMBER);
}

#[test]
fn clan_show_pending_lists_the_pending_leader_change_and_force_runs_it() {
    let mut s = setup();
    let mut record = lions(USER, &[(USER, 1), (MEMBER, 5)]);
    if let Some(data) = record.data.as_mut() {
        data.new_leader_id = MEMBER;
    }
    s.world.load_clans(vec![record], Vec::new());
    s.enter(GM, 0);
    s.log.take();

    run(&mut s, GM, "admin_clan_show_pending");
    let out = s.log.take();
    let page = page_html(&out, GM);
    assert!(page.contains("<td>Lions</td><td>P2</td>"), "{page}");
    assert!(page.contains("admin_clan_force_pending 500"), "{page}");

    run(&mut s, GM, "admin_clan_force_pending 500");
    let out = s.log.take();
    assert_eq!(s.world.clan(CLAN).unwrap().leader_id(), MEMBER);
    assert_eq!(texts_to(&out, GM), ["Task have been forcely executed."]);

    // A clan id that is not numeric, or unknown, does nothing.
    run(&mut s, GM, "admin_clan_force_pending nope");
    run(&mut s, GM, "admin_clan_force_pending 999");
    assert!(s.log.take().is_empty());
}

#[test]
fn pledge_without_an_action_is_refused_and_a_non_leader_is_told() {
    let mut s = setup();
    s.world
        .load_clans(vec![lions(USER, &[(USER, 1), (MEMBER, 5)])], Vec::new());
    s.enter(GM, 0);
    s.enter_member(USER, 1);
    s.enter_member(MEMBER, 5);
    s.log.take();

    run(&mut s, GM, "admin_pledge");
    assert_eq!(texts_to(&s.log.take(), GM), ["Not allowed Action on Clan"]);

    s.target(GM, Some(MEMBER));
    run(&mut s, GM, "admin_pledge info");
    let out = s.log.take();
    assert!(has_system(
        &out,
        GM,
        SystemMessageId::S1IsNotAClanLeader,
        &[SystemMessageParam::Text("P2".to_owned())]
    ));
    assert!(!html_to(&out, GM).is_empty());
}

#[test]
fn pledge_info_sends_the_clan_to_the_gm_with_every_member() {
    let mut s = setup();
    s.world
        .load_clans(vec![lions(USER, &[(USER, 1), (MEMBER, 5)])], Vec::new());
    s.enter(GM, 0);
    s.enter_member(USER, 1);
    s.enter_member(MEMBER, 5);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_pledge info");
    let out = s.log.take();
    let info = decoded::<GmViewPledgeInfo>(&out, GM, "GM_VIEW_PLEDGE_INFO");
    assert_eq!(info.len(), 1);
    assert_eq!(info[0].player_name, "P1");
    assert_eq!(info[0].clan_name, "Lions");
    assert_eq!(info[0].leader_name, "P1");
    assert_eq!(info[0].members.len(), 2);
}

#[test]
fn pledge_info_of_an_unknown_clan_is_java_s_null_pointer() {
    let mut s = setup();
    s.world.load_clans(vec![lions(USER, &[(USER, 1)])], Vec::new());
    s.enter(GM, 0);
    s.enter_member(USER, 1);
    // The leader is the target: the GM is not a leader, so the check before `info` refuses him.
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_pledge info Nowhere");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Exception during execution of  'admin_pledge info Nowhere': java.lang.NullPointerException"]
    );
}

#[test]
fn pledge_rep_needs_a_clan_of_level_five_and_moves_the_score() {
    let mut s = setup();
    s.world.load_clans(vec![lions(USER, &[(USER, 1)])], Vec::new());
    s.enter(GM, 0);
    s.enter_member(USER, 1);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_pledge rep 10");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        ["You add 10 points to Lions's reputation. Their current score is 10"]
    );
    assert_eq!(s.world.clan(CLAN).unwrap().reputation_score, 10);

    run(&mut s, GM, "admin_pledge rep many");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Points incorrect.", "Usage: //pledge <setlevel|rep> <number>"]
    );
}

#[test]
fn pledge_setlevel_checks_the_range_and_changes_the_level() {
    let mut s = setup();
    s.world.load_clans(vec![lions(USER, &[(USER, 1)])], Vec::new());
    s.enter(GM, 0);
    s.enter_member(USER, 1);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_pledge setlevel 12");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Level incorrect.", "Usage: //pledge <setlevel|rep> <number>"]
    );

    run(&mut s, GM, "admin_pledge setlevel 6");
    assert_eq!(texts_to(&s.log.take(), GM), ["You set level 6 for clan Lions"]);
    assert_eq!(s.world.clan(CLAN).unwrap().level, 6);
}

#[test]
fn pledge_create_needs_a_name_and_a_clanless_target_makes_one() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(OTHER, 300);
    s.target(GM, Some(OTHER));
    s.log.take();

    run(&mut s, GM, "admin_pledge create");
    assert_eq!(texts_to(&s.log.take(), GM), ["Please, enter clan name."]);

    run(&mut s, GM, "admin_pledge create Tigers");
    assert_eq!(texts_to(&s.log.take(), GM), ["Clan Tigers created. Leader: P3"]);
    assert!(s.world.clan_by_name("Tigers").is_some());
}

#[test]
fn pledge_dismiss_removes_the_clan_of_the_target() {
    let mut s = setup();
    s.world.load_clans(vec![lions(USER, &[(USER, 1)])], Vec::new());
    s.enter(GM, 0);
    s.enter_member(USER, 1);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_pledge dismiss");
    assert_eq!(texts_to(&s.log.take(), GM), ["Clan disbanded."]);
    assert_eq!(s.world.clan_id_of(ObjectId(USER)), 0);
}

#[test]
fn remove_clan_penalty_clears_an_online_characters_penalty() {
    let mut s = setup();
    s.enter(GM, 0);
    let mut data = player(OTHER, 0, 300);
    data.profile.clan = Some(PlayerClanState {
        clan_join_expiry_time: 123_456,
        ..PlayerClanState::default()
    });
    s.enter_with(data);
    s.log.take();

    run(&mut s, GM, "admin_remove_clan_penalty join");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //remove_clan_penalty join|create charname"]
    );

    run(&mut s, GM, "admin_remove_clan_penalty join P3");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Clan penalty successfully removed to character: P3"]
    );
    assert_eq!(
        s.world.player_clan_state(ObjectId(OTHER)).clan_join_expiry_time,
        0
    );
}

#[test]
fn remove_clan_penalty_of_an_offline_character_sends_nothing() {
    // Java's statement has no `= value` and the database refuses it: only the log has a line.
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    run(&mut s, GM, "admin_remove_clan_penalty create Nobody");
    assert!(texts_to(&s.log.take(), GM).is_empty());
}

// ----------------------------------------------------------------------------------- summons

#[test]
fn summon_words_are_checked_like_java() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    run(&mut s, GM, "admin_summon");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usage: //summon <id> [count]"]);

    run(&mut s, GM, "admin_summon abc");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Incorrect format for command 'summon'"]
    );
}

#[test]
fn summon_from_1000000_spawns_the_npc_once_with_java_s_warning() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    run(&mut s, GM, "admin_summon 1001001");
    let out = s.log.take();
    // The warning comes first; then `admin_spawn_once` reports the NPC it made.
    let texts = texts_to(&out, GM);
    assert_eq!(
        texts.first().map(String::as_str),
        Some("This is only a temporary spawn.  The mob(s) will NOT respawn.")
    );
    assert_eq!(texts.get(1).map(String::as_str), Some("Created Villager on 100"));
    assert_eq!(s.world.npc_ids().len(), 1);
}

#[test]
fn summon_info_unsummon_and_the_pet_words_need_their_targets() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_summon_info");
    assert_eq!(texts_to(&s.log.take(), GM), ["Invalid target."]);

    run(&mut s, GM, "admin_unsummon");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usable only with Pets/Summons"]);

    run(&mut s, GM, "admin_summon_setlvl 5");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usable only with Pets"]);

    run(&mut s, GM, "admin_show_pet_inv");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usable only with Pets"]);
}

// ----------------------------------------------------------------------------- transformations

#[test]
fn transform_menu_shows_the_transformation_page() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    run(&mut s, GM, "admin_transform_menu");
    assert!(page_html(&s.log.take(), GM).contains("Transform"));
}

#[test]
fn transform_needs_a_player_target_and_an_id() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.log.take();

    run(&mut s, GM, "admin_transform 5");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);

    s.target(GM, Some(USER));
    run(&mut s, GM, "admin_transform");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usage: //transform <id>"]);

    run(&mut s, GM, "admin_transform 999999");
    assert_eq!(
        texts_to(&s.log.take(), USER),
        ["Unknown transformation ID: 999999"]
    );
}

#[test]
fn untransform_of_a_normal_character_is_an_invalid_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_untransform");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);
}

#[test]
fn unride_without_a_mount_or_transformation_does_nothing() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_unride");
    assert!(s.log.take().is_empty());
    assert_eq!(s.world.transformation_id(ObjectId(USER)), 0);
}

// ------------------------------------------------------------------------------------ skills

#[test]
fn setskill_without_arguments_is_java_s_index_exception() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    run(&mut s, GM, "admin_setskill");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Exception during execution of  'admin_setskill': java.lang.ArrayIndexOutOfBoundsException: Index 1 out of bounds for length 1"
        ]
    );
}

#[test]
fn setskill_gives_the_gm_the_skill_and_its_list() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    run(&mut s, GM, "admin_setskill 3100 1");
    let out = s.log.take();
    assert_eq!(s.skills(GM), [(3100, 1)]);
    assert_eq!(
        texts_to(&out, GM),
        ["You added yourself skill Reflect(3100) level 1"]
    );
}

#[test]
fn get_skills_on_yourself_is_refused_and_reset_needs_a_stash() {
    let mut s = setup();
    s.enter(GM, 0);
    s.target(GM, Some(GM));
    s.log.take();

    run(&mut s, GM, "admin_get_skills");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::YouCannotUseThisOnYourself]
    );

    s.target(GM, Some(GM));
    run(&mut s, GM, "admin_reset_skills");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["You must get the skills of someone in order to do this."]
    );
}

#[test]
fn get_skills_takes_the_targets_skills_and_reset_gives_the_gms_back() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    run(&mut s, GM, "admin_setskill 3100 1");
    s.target(GM, Some(USER));
    s.log.take();
    assert_eq!(s.skills(GM), [(3100, 1)]);

    // The target knows nothing: the GM's own skill goes into the stash and out of the GM.
    run(&mut s, GM, "admin_get_skills");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["You now have all the skills of P1."]
    );
    assert!(s.skills(GM).is_empty());

    run(&mut s, GM, "admin_reset_skills");
    let out = s.log.take();
    assert_eq!(s.skills(GM), [(3100, 1)]);
    assert_eq!(texts_to(&out, GM), ["You now have all your skills back."]);
    assert_eq!(texts_to(&out, USER), ["[GM]P100 updated your skills."]);
}

#[test]
fn give_all_skills_fs_without_skill_trees_gives_none() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.target(GM, Some(USER));
    s.log.take();

    run(&mut s, GM, "admin_give_all_skills_fs");
    assert_eq!(texts_to(&s.log.take(), GM), ["You gave 0 skills to P1"]);
}

#[test]
fn removereuse_needs_a_target_or_an_online_name() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.log.take();

    run(&mut s, GM, "admin_removereuse");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::ThatIsAnIncorrectTarget]
    );

    run(&mut s, GM, "admin_removereuse Nobody");
    assert_eq!(texts_to(&s.log.take(), GM), ["The player Nobody is not online."]);

    s.target(GM, Some(USER));
    run(&mut s, GM, "admin_removereuse");
    assert_eq!(texts_to(&s.log.take(), GM), ["Skill reuse was removed from P1."]);
}

#[test]
fn buff_checks_its_arguments_and_applies_a_known_skill() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.log.take();

    run(&mut s, GM, "admin_buff");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::ThatIsAnIncorrectTarget]
    );

    s.target(GM, Some(USER));
    run(&mut s, GM, "admin_buff");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Skill Id and level are not specified.",
            "Usage: //buff <skillId> <skillLevel>"
        ]
    );

    run(&mut s, GM, "admin_buff abc");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Failed buffing: For input string: \"abc\"",
            "Usage: //buff <skillId> <skillLevel>"
        ]
    );

    run(&mut s, GM, "admin_buff 9999 1");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Skill with id: 9999, level: 1 not found."]
    );

    // Without a level the highest one of the skill is used.
    run(&mut s, GM, "admin_buff 3100");
    assert_eq!(texts_to(&s.log.take(), GM), ["Admin buffing Reflect (3100,1)"]);
}
