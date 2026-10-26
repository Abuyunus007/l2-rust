#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The effect, menu, teleport, GM-tool, camera, page, message, vitality, augment, element and clan
//! skill admin commands of M7-ADMIN-08, with a fake GM and two fake players in view of each other.
//! Java: scripts/handlers/chat/commands/admin/{AdminEffects,AdminMenu,AdminTeleport,AdminSkill,
//! AdminBuffs,AdminSuperHaste,AdminGmSpeed,AdminHide,AdminTarget,AdminTargetSay,AdminCamera,
//! AdminHtml,AdminHelp,AdminMessages,AdminElement,AdminVitality,AdminAugment}.java.
//!
//! The harness is the one of `admin_social.rs`. The commands that are left out (`//teleto`,
//! `//gmspeed`, ...) answer "does not exist", and the test of the registry checks that too.

use std::sync::{Arc, Mutex};

use l2_data::skill::AbnormalVisualEffect;
use l2_data::{
    ClassList, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates, SkillTemplates, SpawnData,
    SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    Earthquake, ExRedSky, MagicSkillUse, NpcHtmlMessage, OnEventTrigger, PlaySound, SocialAction,
    SpecialCamera, SystemMessage, SystemMessageParam,
};
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

/// Every command word of M7-ADMIN-08 that is registered, at the GM level (100), without a
/// confirmation (the tests call `admin_command(.., false)`).
const REGISTERED: &[&str] = &[
    "admin_invis",
    "admin_invisible",
    "admin_setinvis",
    "admin_vis",
    "admin_visible",
    "admin_invis_menu",
    "admin_earthquake",
    "admin_earthquake_menu",
    "admin_bighead",
    "admin_shrinkhead",
    "admin_unpara_all",
    "admin_para_all",
    "admin_unpara",
    "admin_para",
    "admin_unpara_all_menu",
    "admin_para_all_menu",
    "admin_unpara_menu",
    "admin_para_menu",
    "admin_social",
    "admin_effect",
    "admin_effect_menu",
    "admin_ave_abnormal",
    "admin_ave_special",
    "admin_ave_event",
    "admin_social_menu",
    "admin_play_sounds",
    "admin_play_sound",
    "admin_atmosphere",
    "admin_atmosphere_menu",
    "admin_set_displayeffect",
    "admin_set_displayeffect_menu",
    "admin_event_trigger",
    "admin_char_manage",
    "admin_teleport_character_to_menu",
    "admin_goto_char_menu",
    "admin_kick_menu",
    "admin_ban_menu",
    "admin_unban_menu",
    "admin_kill_menu",
    "admin_walk",
    "admin_recall_npc",
    "admin_sendhome",
    "admin_add_clan_skill",
    "admin_give_clan_skills",
    "admin_give_all_clan_skills",
    "admin_superhaste",
    "admin_superhaste_menu",
    "admin_speed",
    "admin_speed_menu",
    "admin_hide",
    "admin_target",
    "admin_targetsay",
    "admin_cam",
    "admin_camex",
    "admin_cam3",
    "admin_html",
    "admin_loadhtml",
    "admin_help",
    "admin_msg",
    "admin_msgx",
    "admin_setlh",
    "admin_setlc",
    "admin_setlg",
    "admin_setlb",
    "admin_setll",
    "admin_setlw",
    "admin_setls",
    "admin_set_vitality",
    "admin_set_vitality_level",
    "admin_full_vitality",
    "admin_empty_vitality",
    "admin_get_vitality",
    "admin_augment",
];

/// The words of the card that are left out, each answered like an unknown command.
const LEFT_OUT: &[&str] = &[
    "admin_clearteams",
    "admin_setteam",
    "admin_setteam_close",
    "admin_gmspeed",
    "admin_teleto",
    "admin_instant_move",
    "admin_pccafepoints",
    "admin_commands",
    "admin_instancezones",
    "admin_switch_gm_buffs",
];

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
    for command in REGISTERED.iter().chain(LEFT_OUT) {
        rights.add_command(command.trim_start_matches("admin_"), 100, false);
    }
    // The commands the tests run through the other registered handlers.
    for command in ["spawn_once", "ban_char", "unban_char"] {
        rights.add_command(command, 100, false);
    }
    rights
}

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

/// The packets of one opcode sent to `to`, decoded.
fn of<P: Decode>(out: &Sent, to: i32, op: ServerOpcode) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == op.name())
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

/// Whether a packet of this opcode was sent to `to`.
fn sent(out: &Sent, to: i32, op: ServerOpcode) -> bool {
    out.iter().any(|(t, b)| *t == to && name_of(b) == op.name())
}

/// The text of every plain message (`sendMessage` and `sendSysMessage`) to `to`.
fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    of::<SystemMessage>(out, to, ServerOpcode::SystemMessage)
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The ids of the system messages without a text sent to `to`.
fn system_ids_to(out: &Sent, to: i32) -> Vec<SystemMessageId> {
    of::<SystemMessage>(out, to, ServerOpcode::SystemMessage)
        .into_iter()
        .filter(|m| m.id != SystemMessageId::S12)
        .map(|m| m.id)
        .collect()
}

/// The admin pages sent to `to`.
fn html_to(out: &Sent, to: i32) -> Vec<NpcHtmlMessage> {
    of::<NpcHtmlMessage>(out, to, ServerOpcode::NpcHtmlMessage)
}

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
        write("data/html/admin/gm_menu.htm", "<html><body>GMMENU</body></html>");
        write(
            "data/html/admin/effects_menu.htm",
            "<html><body>EFFECTSMENU</body></html>",
        );
        write(
            "data/html/admin/ave_abnormal.htm",
            "<html><body>AVEABNORMAL</body></html>",
        );
        write(
            "data/html/admin/social.htm",
            "<html><body>SOCIALMENU</body></html>",
        );
        write(
            "data/html/admin/charmanage.htm",
            "<html><body>CHARMANAGE</body></html>",
        );
        write(
            "data/html/admin/main_menu.htm",
            "<html><body>MAINMENU</body></html>",
        );
        write("data/html/admin/help.htm", "<html><body>HELPPAGE</body></html>");
        write(
            "data/html/admin/songs/songs.htm",
            "<html><body>SONGS</body></html>",
        );
        write(
            "data/html/admin/teleports.htm",
            "<html><body>TELEPORTS</body></html>",
        );
        write(
            "data/html/admin/pccafe.htm",
            "<html><body>%points% %targetName%</body></html>",
        );
        write(
            "data/html/admin/charskills.htm",
            "<html><body>%name%</body></html>",
        );
        Self(dir)
    }

    fn path(&self) -> &std::path::Path {
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
    settings.vitality.enable_vitality = true;
    let root = Root::new();
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(21), Box::new(NullOutbox));
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
const OTHER: i32 = 2;

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
    fn enter(&mut self, id: i32, dx: i32) {
        let level = if id == GM { 100 } else { 0 };
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player(id, level, dx)),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn target(&mut self, gm: i32, target: Option<i32>) {
        self.world.set_target(ObjectId(gm), target.map(ObjectId));
    }

    fn position(&self, id: i32) -> Option<(i32, i32, i32)> {
        self.world.position_of(ObjectId(id))
    }
}

/// `//command` as the admin panels run it: the confirmation is already answered (`false`).
fn run(s: &mut Setup, gm: i32, command: &str) {
    s.world.admin_command(ObjectId(gm), command, false);
}

fn gm_and_user(s: &mut Setup) {
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();
}

// ----------------------------------------------------------------------- the registry

#[test]
fn every_registered_word_runs_and_the_left_out_ones_are_unknown() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));
    s.log.take();
    for command in REGISTERED {
        run(&mut s, GM, command);
        let out = s.log.take();
        let unknown = texts_to(&out, GM)
            .into_iter()
            .any(|t| t.contains("does not exist"));
        assert!(!unknown, "{command} is not registered");
    }
    for command in LEFT_OUT {
        run(&mut s, GM, command);
        let word = command.trim_start_matches("admin_");
        assert_eq!(
            texts_to(&s.log.take(), GM),
            [format!("The command '{word}' does not exist!")],
            "{command}"
        );
    }
}

// ------------------------------------------------------------------------- effects

#[test]
fn invis_and_vis_hide_the_gm_from_the_players_around() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_invis");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Now, you cannot be seen."]);
    let deleted = of::<l2_protocol_game::server::DeleteObject>(&out, USER, ServerOpcode::DeleteObject);
    assert!(deleted.iter().any(|d| d.object_id == GM));

    run(&mut s, GM, "admin_vis");
    assert_eq!(texts_to(&s.log.take(), GM), ["Now, you can be seen."]);
}

#[test]
fn invis_menu_toggles_and_shows_the_gm_menu() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_invis_menu");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Now, you cannot be seen."]);
    assert!(html_to(&out, GM)[0].html.contains("GMMENU"));

    run(&mut s, GM, "admin_invis_menu");
    assert_eq!(texts_to(&s.log.take(), GM), ["Now, you can be seen."]);
}

#[test]
fn setinvis_toggles_the_target_and_refuses_a_non_creature() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));

    run(&mut s, GM, "admin_setinvis");
    assert_eq!(texts_to(&s.log.take(), GM), ["You've made P1 invisible."]);

    run(&mut s, GM, "admin_setinvis");
    assert_eq!(texts_to(&s.log.take(), GM), ["You've made P1 visible."]);

    s.target(GM, None);
    run(&mut s, GM, "admin_setinvis");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);
}

#[test]
fn para_and_unpara_set_the_paralysis_and_its_flag() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));

    run(&mut s, GM, "admin_para");
    s.log.take();
    assert!(s.world.is_paralyzed(ObjectId(USER)));

    run(&mut s, GM, "admin_unpara");
    s.log.take();
    assert!(!s.world.is_paralyzed(ObjectId(USER)));

    // `admin_para_menu` has no type, so it is the paralysis, and the menu page follows.
    run(&mut s, GM, "admin_para_menu");
    let out = s.log.take();
    assert!(s.world.is_paralyzed(ObjectId(USER)));
    assert!(html_to(&out, GM)[0].html.contains("EFFECTSMENU"));
}

#[test]
fn bighead_and_shrinkhead_toggle_the_big_head_effect() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));
    let big_head = AbnormalVisualEffect::BigHead.mask();

    run(&mut s, GM, "admin_bighead");
    s.log.take();
    assert_eq!(
        s.world.abnormal_visual_effects(ObjectId(USER)).0 & big_head,
        big_head
    );

    run(&mut s, GM, "admin_shrinkhead");
    s.log.take();
    assert_eq!(s.world.abnormal_visual_effects(ObjectId(USER)).0 & big_head, 0);
}

#[test]
fn social_sends_the_action_to_the_target_and_refuses_the_wrong_kind() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));

    run(&mut s, GM, "admin_social 2");
    let out = s.log.take();
    let actions = of::<SocialAction>(&out, OTHER, ServerOpcode::SocialAction);
    assert!(actions.is_empty(), "no one else is in view");
    assert_eq!(texts_to(&out, GM), ["P1 was affected by your request."]);

    // A player takes only the social ids from 2 to 18 (and the level up one).
    run(&mut s, GM, "admin_social 40");
    // Java sends NOTHING_HAPPENED from `performSocial` and again from the `else` branch.
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::NothingHappened, SystemMessageId::NothingHappened]
    );

    run(&mut s, GM, "admin_social");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //social <social_id> [player_name|radius]"]
    );

    run(&mut s, GM, "admin_social 2 1 2");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //social <social_id> [player_name|radius]"]
    );
}

#[test]
fn ave_names_its_effect_and_the_usage_comes_without_one() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));

    run(&mut s, GM, "admin_ave_abnormal");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        ["Usage: //ave_abnormal <AbnormalVisualEffect> [radius]"]
    );
    assert!(html_to(&out, GM)[0].html.contains("AVEABNORMAL"));

    run(&mut s, GM, "admin_ave_abnormal BIG_HEAD");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["P1 affected by BIG_HEAD abnormal visual effect."]
    );
    let big_head = AbnormalVisualEffect::BigHead.mask();
    assert_eq!(
        s.world.abnormal_visual_effects(ObjectId(USER)).0 & big_head,
        big_head
    );

    // An unknown name is `return false`: no page follows.
    run(&mut s, GM, "admin_ave_event NOT_A_NAME");
    assert!(html_to(&s.log.take(), GM).is_empty());
}

#[test]
fn effect_sends_the_magic_skill_use_of_the_target() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));

    run(&mut s, GM, "admin_effect 3100 1 2");
    let out = s.log.take();
    let msu = of::<MagicSkillUse>(&out, GM, ServerOpcode::MagicSkillUse);
    assert_eq!(msu.len(), 1);
    assert_eq!(msu[0].creature_id, USER);
    assert_eq!(msu[0].target_id, GM);
    assert_eq!(msu[0].skill_id, 3100);
    assert_eq!(msu[0].skill_level, 1);
    assert_eq!(msu[0].hit_time, 2);
    assert_eq!(texts_to(&out, GM), ["P1 performs MSU 3100/1 by your request."]);

    run(&mut s, GM, "admin_effect nothing");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //effect skill [level | level hittime]"]
    );
}

#[test]
fn event_trigger_reaches_the_gm_and_the_players_in_view() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_event_trigger 5 true");
    let out = s.log.take();
    let triggers = of::<OnEventTrigger>(&out, USER, ServerOpcode::EventTrigger);
    assert_eq!(triggers.len(), 1);
    assert_eq!((triggers[0].emitter_id, triggers[0].enabled), (5, true));
    assert!(sent(&out, GM, ServerOpcode::EventTrigger));

    run(&mut s, GM, "admin_event_trigger 5");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //event_trigger id [true | false]"]
    );
}

#[test]
fn earthquake_shakes_around_the_gm_and_needs_two_numbers() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_earthquake 5 10");
    let out = s.log.take();
    let quakes = of::<Earthquake>(&out, USER, ServerOpcode::Earthquake);
    assert_eq!(quakes.len(), 1);
    assert_eq!(
        (quakes[0].x, quakes[0].y, quakes[0].z),
        (X, Y, Z),
        "the GM's place"
    );
    assert_eq!((quakes[0].intensity, quakes[0].duration), (5, 10));

    run(&mut s, GM, "admin_earthquake 5");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //earthquake <intensity> <duration>"]
    );
}

#[test]
fn play_sound_and_atmosphere_reach_every_online_player() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_play_sound town_theme");
    let out = s.log.take();
    // Java sends the GM its own copy twice (`sendPacket` and `broadcastPacket`).
    assert_eq!(of::<PlaySound>(&out, GM, ServerOpcode::PlaySound).len(), 2);
    assert_eq!(of::<PlaySound>(&out, USER, ServerOpcode::PlaySound).len(), 1);
    assert_eq!(texts_to(&out, GM), ["Playing town_theme."]);

    run(&mut s, GM, "admin_atmosphere sky night 0");
    let out = s.log.take();
    assert!(sent(&out, USER, ServerOpcode::Sunset));
    assert!(sent(&out, GM, ServerOpcode::Sunset));

    run(&mut s, GM, "admin_atmosphere sky red 0");
    let out = s.log.take();
    assert_eq!(
        of::<ExRedSky>(&out, USER, ServerOpcode::ExRedSky),
        [ExRedSky { duration: 10 }]
    );

    run(&mut s, GM, "admin_atmosphere sky red 30");
    let out = s.log.take();
    assert_eq!(
        of::<ExRedSky>(&out, USER, ServerOpcode::ExRedSky),
        [ExRedSky { duration: 30 }]
    );

    run(&mut s, GM, "admin_atmosphere_menu");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        ["Usage: //atmosphere <signsky dawn|dusk>|<sky day|night|red> <duration>"]
    );
    assert!(html_to(&out, GM)[0].html.contains("EFFECTSMENU"));
}

#[test]
fn play_sounds_shows_the_song_pages_by_number() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_play_sounds");
    assert!(html_to(&s.log.take(), GM)[0].html.contains("SONGS"));

    run(&mut s, GM, "admin_play_sounds 7");
    let text = html_to(&s.log.take(), GM)[0].html.clone();
    assert!(text.contains("songs/songs7.htm"), "{text}");

    run(&mut s, GM, "admin_play_sound");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usage: //play_sound <soundname>"]);
}

#[test]
fn set_displayeffect_changes_the_npc_and_tells_the_players_around() {
    let mut s = setup();
    gm_and_user(&mut s);
    run(&mut s, GM, "admin_spawn_once 1001");
    s.log.take();
    let npc = s.world.npc_ids()[0];
    s.target(GM, Some(npc.0));

    run(&mut s, GM, "admin_set_displayeffect 3");
    let out = s.log.take();
    let state = of::<l2_protocol_game::server::ExChangeNpcState>(&out, USER, ServerOpcode::ExChangeNpcState);
    assert_eq!(
        state,
        [l2_protocol_game::server::ExChangeNpcState {
            object_id: npc.0,
            state: 3
        }]
    );

    s.target(GM, Some(USER));
    run(&mut s, GM, "admin_set_displayeffect 3");
    let out = s.log.take();
    assert_eq!(system_ids_to(&out, GM), [SystemMessageId::InvalidTarget]);
    assert!(
        html_to(&out, GM).is_empty(),
        "no page for a target that is no NPC"
    );
}

// --------------------------------------------------------------------------- menus

#[test]
fn char_manage_and_the_teleport_menu_move_a_character() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_char_manage");
    assert!(html_to(&s.log.take(), GM)[0].html.contains("CHARMANAGE"));

    run(&mut s, GM, "admin_teleport_character_to_menu P1 10 20 30");
    let out = s.log.take();
    assert_eq!(texts_to(&out, USER), ["Admin is teleporting you."]);
    // The place of the teleport gets the height correction of the world, so x and y are checked.
    assert_eq!(s.position(USER).map(|p| (p.0, p.1)), Some((10, 20)));
    assert!(html_to(&out, GM)[0].html.contains("CHARMANAGE"));
}

#[test]
fn goto_char_menu_moves_the_gm_to_the_player() {
    let mut s = setup();
    gm_and_user(&mut s);
    let place = s.position(USER);

    run(&mut s, GM, "admin_goto_char_menu P1");
    let out = s.log.take();
    assert_eq!(s.position(GM).map(|p| (p.0, p.1)), place.map(|p| (p.0, p.1)));
    assert_eq!(
        texts_to(&out, GM),
        ["You're teleporting yourself to character P1"]
    );
}

#[test]
fn kick_menu_kicks_the_named_player_or_says_it_is_not_there() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_kick_menu P1");
    let out = s.log.take();
    assert!(sent(&out, USER, ServerOpcode::LogOutOk));
    assert!(s.world.player(ObjectId(USER)).is_none());
    assert_eq!(texts_to(&out, GM), ["You kicked P1 from the game."]);

    run(&mut s, GM, "admin_kick_menu nobody");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Player nobody was not found in the game."]
    );
}

#[test]
fn kill_menu_takes_all_the_hit_points_of_the_target() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));

    run(&mut s, GM, "admin_kill_menu");
    let out = s.log.take();
    assert!(html_to(&out, GM)[0].html.contains("CHARMANAGE"));
}

// ------------------------------------------------------------------------- teleport

#[test]
fn recall_npc_needs_a_monster_and_sendhome_needs_a_player_online() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, None);

    run(&mut s, GM, "admin_recall_npc");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::InvalidTarget]);

    run(&mut s, GM, "admin_sendhome nobody");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::ThatPlayerIsNotOnline]
    );

    run(&mut s, GM, "admin_sendhome a b");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usage: //sendhome <playername>"]);
}

#[test]
fn walk_with_bad_numbers_does_nothing_and_says_nothing() {
    let mut s = setup();
    gm_and_user(&mut s);
    let before = s.position(GM);

    run(&mut s, GM, "admin_walk 1 two 3");
    run(&mut s, GM, "admin_walk");
    let out = s.log.take();
    assert!(out.iter().all(|(t, _)| *t != GM) || texts_to(&out, GM).is_empty());
    assert_eq!(s.position(GM), before);
}

#[test]
fn recall_npc_moves_a_spawned_monster_to_the_gm() {
    let mut s = setup();
    gm_and_user(&mut s);
    run(&mut s, GM, "admin_spawn_once 1001");
    s.log.take();
    let npc = s.world.npc_ids()[0];
    s.target(GM, Some(npc.0));

    run(&mut s, GM, "admin_recall_npc");
    let out = s.log.take();
    // The monster of `admin_spawn_once` has no spawn respawn: the message names the old id.
    let text = texts_to(&out, GM);
    assert_eq!(text.len(), 1, "{text:?}");
    assert!(text[0].starts_with("Created Villager on "), "{text:?}");
}

// ----------------------------------------------------------------------------- GM tools

#[test]
fn superhaste_takes_a_level_and_says_the_usage_for_a_bad_one() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_superhaste abc");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //superhaste <Effect level (0-4)> <Time in seconds>"]
    );

    // Level 0 stops it (nothing was there); a level without a skill template is the usage too.
    run(&mut s, GM, "admin_speed 0");
    assert!(texts_to(&s.log.take(), GM).is_empty());
    run(&mut s, GM, "admin_superhaste 2");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //superhaste <Effect level (0-4)> <Time in seconds>"]
    );

    run(&mut s, GM, "admin_speed_menu");
    assert!(html_to(&s.log.take(), GM)[0].html.contains("GMMENU"));
}

#[test]
fn hide_on_and_off_make_the_gm_invisible_and_invulnerable() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_hide on");
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Now, you cannot be seen."]);
    assert!(s.world.is_invul(ObjectId(GM)));
    let deleted = of::<l2_protocol_game::server::DeleteObject>(&out, USER, ServerOpcode::DeleteObject);
    assert!(deleted.iter().any(|d| d.object_id == GM));

    run(&mut s, GM, "admin_hide on");
    assert_eq!(texts_to(&s.log.take(), GM), ["Currently, you cannot be seen."]);

    run(&mut s, GM, "admin_hide off");
    s.log.take();
    assert!(!s.world.is_invul(ObjectId(GM)));

    run(&mut s, GM, "admin_hide");
    assert_eq!(texts_to(&s.log.take(), GM), ["//hide [on|off]"]);
}

#[test]
fn target_clicks_the_named_player_and_says_when_it_is_missing() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_target P1");
    s.log.take();
    assert_eq!(s.world.target_of(ObjectId(GM)), Some(ObjectId(USER)));

    run(&mut s, GM, "admin_target nobody");
    assert_eq!(texts_to(&s.log.take(), GM), ["Player nobody not found"]);

    run(&mut s, GM, "admin_target");
    assert_eq!(texts_to(&s.log.take(), GM), ["Please specify correct name."]);
}

#[test]
fn targetsay_speaks_as_the_target() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));

    run(&mut s, GM, "admin_targetsay hello there");
    let out = s.log.take();
    assert!(sent(&out, GM, ServerOpcode::Say2));
    let says = out
        .iter()
        .filter(|(t, b)| *t == GM && name_of(b) == ServerOpcode::Say2.name())
        .count();
    assert_eq!(says, 1);

    run(&mut s, GM, "admin_targetsay");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usage: //targetsay <text>"]);
}

// ----------------------------------------------------------------------------- camera

#[test]
fn cam_needs_a_creature_and_the_usage_count() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_cam 1 2 3 4 5 6 7 8 9 10 11");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::YourTargetCannotBeFound]
    );

    s.target(GM, Some(USER));
    run(&mut s, GM, "admin_cam 1 2 3 4 5 6 7 8 9 10");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //cam force angle1 angle2 time range duration relYaw relPitch isWide relAngle"]
    );
}

#[test]
fn cam_sends_the_packet_with_the_range_as_its_duration() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));

    run(&mut s, GM, "admin_cam 1 2 3 4 5 6 7 8 9 10 11");
    let out = s.log.take();
    let cams = of::<SpecialCamera>(&out, GM, ServerOpcode::SpecialCamera);
    assert_eq!(
        cams,
        [SpecialCamera {
            id: USER,
            force: 1,
            angle1: 2,
            angle2: 3,
            time: 4,
            duration: 5,
            rel_yaw: 7,
            rel_pitch: 8,
            is_wide: 9,
            rel_angle: 10,
            unk: 0,
        }]
    );
}

#[test]
fn camex_writes_a_zero_duration_and_cam3_keeps_its_unknown_word() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));

    run(&mut s, GM, "admin_camex 1 2 3 4 5 6 7 8 9");
    let camex = of::<SpecialCamera>(&s.log.take(), GM, ServerOpcode::SpecialCamera);
    assert_eq!(camex.len(), 1);
    assert_eq!(
        (camex[0].duration, camex[0].rel_yaw, camex[0].rel_angle),
        (0, 6, 9)
    );

    run(&mut s, GM, "admin_cam3 1 2 3 4 5 6 7 8 9 10 11");
    let cam3 = of::<SpecialCamera>(&s.log.take(), GM, ServerOpcode::SpecialCamera);
    assert_eq!((cam3[0].duration, cam3[0].unk), (6, 11));
}

// ------------------------------------------------------------------------------- pages

#[test]
fn html_and_loadhtml_show_pages_and_help_shows_the_help_page() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_html gm_menu.htm");
    assert!(html_to(&s.log.take(), GM)[0].html.contains("GMMENU"));

    run(&mut s, GM, "admin_loadhtml data/html/admin/gm_menu.htm");
    assert!(html_to(&s.log.take(), GM)[0].html.contains("GMMENU"));

    run(&mut s, GM, "admin_html");
    assert_eq!(texts_to(&s.log.take(), GM), ["Usage: //html path"]);

    run(&mut s, GM, "admin_help");
    assert!(html_to(&s.log.take(), GM)[0].html.contains("HELPPAGE"));
}

// --------------------------------------------------------------------------- messages

#[test]
fn msg_sends_the_system_message_by_id() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_msg 61");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::NothingHappened]
    );

    run(&mut s, GM, "admin_msg abc");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Command format: //msg <SYSTEM_MSG_ID>"]
    );
}

#[test]
fn msgx_adds_the_parameters_in_order() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(
        &mut s,
        GM,
        "admin_msgx 61 item:57 skill:3100 zone:1,2,3 castle:4 str:'hi there'",
    );
    let out = s.log.take();
    let message = of::<SystemMessage>(&out, GM, ServerOpcode::SystemMessage)
        .into_iter()
        .find(|m| m.id == SystemMessageId::NothingHappened)
        .unwrap();
    assert_eq!(
        message.params,
        [
            SystemMessageParam::ItemName(57),
            SystemMessageParam::SkillName { id: 3100, level: 1 },
            SystemMessageParam::ZoneName { x: 1, y: 2, z: 3 },
            SystemMessageParam::CastleName(4),
            SystemMessageParam::Text("hi there".to_owned()),
        ]
    );

    run(&mut s, GM, "admin_msgx 61");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Command format: //msgx <SYSTEM_MSG_ID> [item:Id] [skill:Id] [npc:Id] [zone:x,y,x] [castle:Id] [str:'text']"
        ]
    );
}

// --------------------------------------------------------------------------- vitality

#[test]
fn vitality_words_set_the_points_of_the_target() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, Some(USER));

    run(&mut s, GM, "admin_set_vitality 500");
    assert_eq!(s.world.vitality_points(ObjectId(USER)), 500);
    assert_eq!(
        texts_to(&s.log.take(), USER),
        ["Admin set your Vitality points to 500"]
    );

    run(&mut s, GM, "admin_set_vitality_level 2");
    assert_eq!(s.world.vitality_points(ObjectId(USER)), 2000);
    s.log.take();

    run(&mut s, GM, "admin_full_vitality");
    assert_eq!(s.world.vitality_points(ObjectId(USER)), 20000);
    s.log.take();

    run(&mut s, GM, "admin_empty_vitality");
    assert_eq!(s.world.vitality_points(ObjectId(USER)), 1);
    s.log.take();

    run(&mut s, GM, "admin_get_vitality");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Player vitality level: 0", "Player vitality points: 1"]
    );
}

#[test]
fn vitality_needs_a_player_target() {
    let mut s = setup();
    gm_and_user(&mut s);
    s.target(GM, None);

    run(&mut s, GM, "admin_get_vitality");
    assert_eq!(texts_to(&s.log.take(), GM), ["Target not found or not a player"]);
}

// ---------------------------------------------------------------- augment and element

#[test]
fn augment_and_element_say_why_they_change_nothing() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_augment 57");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //augment <item id> <option id>"]
    );

    run(&mut s, GM, "admin_augment 57 10");
    assert_eq!(texts_to(&s.log.take(), GM), ["No equipped item with id 57"]);

    run(&mut s, GM, "admin_setlh fire 999");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //setlh/setlc/setlg/setlb/setll/setlw/setls <element> <value>[0-450]"]
    );

    run(&mut s, GM, "admin_setlh fire");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //setlh/setlc/setlg/setlb/setll/setlw/setls <element>[0-5] <value>[0-450]"]
    );
}

// ---------------------------------------------------------------------------- clan skills

#[test]
fn clan_skill_commands_check_the_target_and_the_clan() {
    let mut s = setup();
    gm_and_user(&mut s);

    run(&mut s, GM, "admin_add_clan_skill 380 1");
    // The page check of Java `showMainPage` sends the invalid target a second time.
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::InvalidTarget, SystemMessageId::InvalidTarget]
    );

    run(&mut s, GM, "admin_add_clan_skill 380");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Usage: //add_clan_skill <skill_id> <level>"]
    );

    s.target(GM, Some(USER));
    run(&mut s, GM, "admin_give_clan_skills");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::TheTargetMustBeAClanMember]
    );
}
