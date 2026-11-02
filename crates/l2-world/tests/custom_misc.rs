#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The misc custom features of M9-C06 (`config/Custom/*.ini`): each one with its switch off (the
//! default: nothing changes) and on (the Java behaviour). The expected texts are read off the Java
//! sources (`voiced/{Online,Banking,ChatAdmin,ChangePassword,Lang}.java`, `CaptchaManager.java`,
//! `CustomMailManager.java`, `AntiFeedManager.java`, `Say2.java`, `EnterWorld.java`).

#[path = "admin_common/mod.rs"]
mod admin_common;

use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use admin_common::{html_to, name_of, says_to, texts_to};
use l2_data::{ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, PlayerTemplates, SkillTemplates};
use l2_protocol_game::server::ChatType;
use l2_world::custom::dualbox::{GAME_ID, OLYMPIAD_ID, java_string_hash};
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

#[derive(Clone, Default)]
struct Outs(Arc<Mutex<Vec<PersistCommand>>>, Arc<Mutex<Vec<LsEvent>>>);

struct Recorder(Outs);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, event: LsEvent) {
        self.0.1.lock().unwrap().push(event);
    }
}

const CLASS: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level>
<level val="20"><hp>300</hp><mp>150</mp><cp>60</cp></level></lvlUpgainData></list>"#;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="3470" type="EtcItem" name="Gold Bar"><set name="is_stackable" val="true" /></item>
</list>"#;

const ADENA: i32 = 57;
const GOLD_BAR: i32 = 3470;
const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ALICE: i32 = 0x1000_0001;
const BOB: i32 = 0x1000_0002;
const MODERATOR: i32 = 0x1000_0009;

struct Data {
    stats: StatData,
    rights: AdminRights,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.stats.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap_or(5),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: -1,
            weapon: t.is_weapon(),
        })
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
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.stats.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.stats.items.get(item_id).is_some_and(|t| t.stackable)
    }
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.rights)
    }
}

struct Scene {
    world: World,
    log: Log,
    outs: Outs,
    root: tempfile::TempDir,
}

const PAGES: &[(&str, &str)] = &[
    (
        "data/html/mods/IPRestriction.htm",
        "<html><body>Maximum %max% connection(s) per IP address allowed.</body></html>",
    ),
    (
        "data/html/mods/ChangePassword.htm",
        "<html><body><center><td><tr>Current Password:</tr><tr><edit type=\"password\" var=\"oldpass\" width=150></tr><br>\
         <button value=\"Change Password\" action=\"bypass -h voice .changepassword $oldpass $newpass $repeatnewpass\" width=160 height=25 back=\"L2UI_ct1.button_df\" fore=\"L2UI_ct1.button_df\"></center></body></html>",
    ),
    (
        "data/html/mods/Lang/LanguageSelect.htm",
        "<html><body>languages %list%</body></html>",
    ),
    ("data/html/mods/Lang/Ok.htm", "<html><body>lang ok</body></html>"),
    (
        "data/html/mods/Lang/Error.htm",
        "<html><body>lang error</body></html>",
    ),
    (
        "data/lang/el/data/html/mods/Lang/Ok.htm",
        "<html><body>lang ok el</body></html>",
    ),
];

impl Scene {
    fn new(custom: impl FnOnce(&mut CustomMiscSettings)) -> Self {
        let root = tempfile::tempdir().unwrap();
        for (path, text) in PAGES {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        }
        let outs = Outs::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(Recorder(outs.clone())));
        let stats = StatData {
            items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
            player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::default()),
        };
        let mut rights = AdminRights::default();
        for (level, child) in [(0, 0), (70, 0), (100, 70)] {
            rights.add_level(level, child);
        }
        for command in ["banchat", "chatban", "unbanchat", "chatunban"] {
            rights.add_command(command, 100, false);
        }
        world.set_static_data(Box::new(Data { stats, rights }));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let mut settings = WorldSettings::default();
        custom(&mut settings.custom_misc);
        world.set_settings(settings);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        Self {
            world,
            log: Log::default(),
            outs,
            root,
        }
    }

    fn enter(&mut self, id: i32, name: &str, adena: i64, ip: Option<IpAddr>) {
        self.enter_as(id, name, adena, 0, ip);
    }

    fn enter_as(&mut self, id: i32, name: &str, adena: i64, access_level: i32, ip: Option<IpAddr>) {
        self.enter_raw(id, name, adena, access_level, ip);
        // The login server hears `PlayerInGame` at every login: not what the tests look at.
        self.ls_events();
    }

    fn enter_raw(&mut self, id: i32, name: &str, adena: i64, access_level: i32, ip: Option<IpAddr>) {
        let items = vec![ItemData {
            object_id: ObjectId(id + 0x100),
            item_id: ADENA,
            count: adena,
            loc: "INVENTORY".into(),
            ..ItemData::default()
        }];
        let data = PlayerData {
            object_id: ObjectId(id),
            account: format!("acc{id}"),
            name: name.into(),
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
            access_level,
            x: X,
            y: Y,
            z: Z,
            heading: 0,
            cur_hp: 300.0,
            max_hp: 300.0,
            cur_mp: 50.0,
            max_mp: 50.0,
            cur_cp: 0.0,
            max_cp: 50.0,
            online_time: 10,
            items,
            skills: vec![],
            shortcuts: vec![],
            profile: Default::default(),
        };
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(id as u64),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip,
        });
    }

    fn say(&mut self, id: i32, text: &str) {
        self.say_as(id, ChatType::General, text, None);
    }

    fn say_as(&mut self, id: i32, chat_type: ChatType, text: &str, target: Option<&str>) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(id as u64),
            input: ClientInput::Say {
                chat_type: chat_type.client_id(),
                text: text.into(),
                target: target.map(str::to_owned),
            },
        });
    }

    fn bypass(&mut self, id: i32, command: &str) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(id as u64),
            input: ClientInput::Bypass {
                command: command.into(),
            },
        });
    }

    /// What `to` was sent since the last call.
    fn sent(&self) -> Sent {
        self.log.take()
    }

    fn persists(&self) -> Vec<PersistCommand> {
        std::mem::take(&mut self.outs.0.lock().unwrap())
    }

    fn ls_events(&self) -> Vec<LsEvent> {
        std::mem::take(&mut self.outs.1.lock().unwrap())
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn adena(&self, id: i32) -> i64 {
        self.world.adena_of(ObjectId(id))
    }

    fn bars(&self, id: i32) -> i64 {
        self.world.inventory_item_count(ObjectId(id), GOLD_BAR)
    }
}

fn texts(sent: &Sent, to: i32) -> Vec<String> {
    texts_to(sent, to)
}

fn has(sent: &Sent, to: i32, opcode: &str) -> bool {
    sent.iter().any(|(t, b)| *t == to && name_of(b) == opcode)
}

fn ip(text: &str) -> Option<IpAddr> {
    Some(text.parse().unwrap())
}

// ------------------------------------------------------------------------ ScreenWelcomeMessage

#[test]
fn welcome_message_is_off_by_default() {
    let mut s = Scene::new(|_| {});
    s.enter(ALICE, "Alice", 0, None);
    assert!(!has(&s.sent(), ALICE, "EX_SHOW_SCREEN_MESSAGE"));
}

#[test]
fn welcome_message_on_shows_the_text_at_login() {
    let mut s = Scene::new(|c| {
        c.screen_welcome.enabled = true;
        c.screen_welcome.text = "Hello there".into();
        c.screen_welcome.time_ms = 4000;
    });
    s.enter(ALICE, "Alice", 0, None);
    let sent = s.sent();
    let packet: Vec<_> = sent
        .iter()
        .filter(|(t, b)| *t == ALICE && name_of(b) == "EX_SHOW_SCREEN_MESSAGE")
        .collect();
    assert_eq!(packet.len(), 1);
    let utf16: Vec<u8> = "Hello there".encode_utf16().flat_map(u16::to_le_bytes).collect();
    assert!(packet[0].1.windows(utf16.len()).any(|w| w == utf16.as_slice()));
    // 4000 ms as the time field.
    assert!(packet[0].1.windows(4).any(|w| w == 4000i32.to_le_bytes()));
}

// ------------------------------------------------------------------------------- OnlineInfo

#[test]
fn online_command_off_is_plain_chat() {
    let mut s = Scene::new(|_| {});
    s.enter(ALICE, "Alice", 0, None);
    s.sent();
    s.say(ALICE, ".online");
    let sent = s.sent();
    assert!(texts(&sent, ALICE).is_empty());
    assert_eq!(says_to(&sent, ALICE).len(), 1);
}

#[test]
fn online_command_on_counts_the_players() {
    let mut s = Scene::new(|c| c.online_command = true);
    s.enter(ALICE, "Alice", 0, None);
    s.sent();
    s.say(ALICE, ".online");
    let sent = s.sent();
    assert_eq!(texts(&sent, ALICE), vec!["There is 1 player online!".to_owned()]);
    // The handler took the text: it is not said.
    assert!(says_to(&sent, ALICE).is_empty());
    s.enter(BOB, "Bob", 0, None);
    s.sent();
    s.say(BOB, ".online");
    assert_eq!(
        texts(&s.sent(), BOB),
        vec!["There are 2 players online!".to_owned()]
    );
    // NOTE(java-quirk): the command is compared whole, so a trailing space says nothing.
    s.say(BOB, ".online ");
    let sent = s.sent();
    assert!(texts(&sent, BOB).is_empty() && says_to(&sent, BOB).is_empty());
}

// --------------------------------------------------------------------------------- Banking

#[test]
fn banking_off_is_plain_chat() {
    let mut s = Scene::new(|_| {});
    s.enter(ALICE, "Alice", 2_000_000_000, None);
    s.sent();
    s.say(ALICE, ".deposit");
    assert_eq!(s.adena(ALICE), 2_000_000_000);
    assert_eq!(s.bars(ALICE), 0);
    assert_eq!(says_to(&s.sent(), ALICE).len(), 1);
}

#[test]
fn banking_on_converts_adena_and_gold_bars() {
    let mut s = Scene::new(|c| {
        c.banking.enabled = true;
        c.banking.goldbar_count = 2;
        c.banking.adena_count = 1_000;
    });
    s.enter(ALICE, "Alice", 2_500, None);
    s.sent();
    s.say(ALICE, ".bank");
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec![
            "Use .deposit to convert 1000 Adena to 2 Gold Bars.".to_owned(),
            "Use .withdraw to convert 2 Gold Bars to 1000 Adena.".to_owned(),
        ]
    );
    s.say(ALICE, ".deposit");
    assert_eq!((s.adena(ALICE), s.bars(ALICE)), (1_500, 2));
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec!["Successfully converted 2 Gold Bars into 1000 Adena.".to_owned()]
    );
    s.say(ALICE, ".deposit");
    assert_eq!((s.adena(ALICE), s.bars(ALICE)), (500, 4));
    s.sent();
    // Not enough Adena any more.
    s.say(ALICE, ".deposit");
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec![
            "You don't have enough Adena to convert to Gold Bars.".to_owned(),
            "You need 1000 Adena but you only have 500. You need 500 more.".to_owned(),
        ]
    );
    s.say(ALICE, ".withdraw");
    assert_eq!((s.adena(ALICE), s.bars(ALICE)), (1_500, 2));
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec!["Successfully converted 1000 Adena to 2 Gold Bars.".to_owned()]
    );
    s.say(ALICE, ".withdraw");
    s.sent();
    s.say(ALICE, ".withdraw");
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec![
            "You don't have enough Gold Bars to convert to Adena.".to_owned(),
            "You need 2 Gold Bars but you only have 0. You need 2 more.".to_owned(),
        ]
    );
}

#[test]
fn banking_auto_convert_off_does_nothing_and_on_converts_above_the_limit() {
    let mut off = Scene::new(|c| c.banking.adena_count = 1_000);
    off.enter(ALICE, "Alice", 10_000, None);
    off.world.banking_auto_convert(ObjectId(ALICE));
    assert_eq!((off.adena(ALICE), off.bars(ALICE)), (10_000, 0));

    let mut on = Scene::new(|c| {
        c.banking.auto_convert_enabled = true;
        c.banking.auto_convert_adena_limit = 5_000;
        c.banking.adena_count = 1_000;
        c.banking.goldbar_count = 1;
    });
    on.enter(ALICE, "Alice", 5_000, None);
    on.sent();
    // Not over the limit (the limit itself is not over).
    on.world.banking_auto_convert(ObjectId(ALICE));
    assert_eq!((on.adena(ALICE), on.bars(ALICE)), (5_000, 0));
    on.enter(BOB, "Bob", 5_001, None);
    on.sent();
    on.world.banking_auto_convert(ObjectId(BOB));
    assert_eq!((on.adena(BOB), on.bars(BOB)), (4_001, 1));
    assert_eq!(
        texts(&on.sent(), BOB),
        vec!["Automatically converted 1000 Adena to 1 Gold Bar.".to_owned()]
    );
}

// ----------------------------------------------------------------------------- ChatModeration

#[test]
fn chat_admin_off_is_plain_chat() {
    let mut s = Scene::new(|c| c.chat_admin = false);
    s.enter_as(MODERATOR, "Mod", 0, 100, None);
    s.sent();
    s.say(MODERATOR, ".banchat Alice");
    let sent = s.sent();
    assert!(texts(&sent, MODERATOR).is_empty());
    assert_eq!(says_to(&sent, MODERATOR).len(), 1);
}

#[test]
fn chat_admin_is_on_by_default_but_needs_the_right() {
    let mut s = Scene::new(|_| {});
    s.enter(ALICE, "Alice", 0, None);
    s.enter(BOB, "Bob", 0, None);
    s.sent();
    // A player without the right: the handler takes the text and says nothing.
    s.say(ALICE, ".banchat Bob 5");
    let sent = s.sent();
    assert!(texts(&sent, ALICE).is_empty() && says_to(&sent, ALICE).is_empty());
    assert!(!s.world.is_chat_banned(ObjectId(BOB)));
}

#[test]
fn chat_admin_bans_and_unbans() {
    let mut s = Scene::new(|_| {});
    s.enter_as(MODERATOR, "Mod", 0, 100, None);
    s.enter(ALICE, "Alice", 0, None);
    s.sent();
    s.say(MODERATOR, ".banchat");
    assert_eq!(
        texts(&s.sent(), MODERATOR),
        vec!["Usage: .banchat name [minutes]".to_owned()]
    );
    s.say(MODERATOR, ".banchat Nobody 5");
    assert_eq!(texts(&s.sent(), MODERATOR), vec!["Player not found!".to_owned()]);
    s.say(MODERATOR, ".banchat Mod 5");
    assert_eq!(
        texts(&s.sent(), MODERATOR),
        vec!["You can't ban yourself!".to_owned()]
    );

    s.say(MODERATOR, ".banchat Alice 5");
    let sent = s.sent();
    assert_eq!(
        texts(&sent, MODERATOR),
        vec!["Player Alice chat banned for 5 minutes.".to_owned()]
    );
    // The punishment tells the player first, then the moderator's message follows.
    assert_eq!(
        texts(&sent, ALICE).last().map(String::as_str),
        Some("Chat banned by moderator Mod")
    );
    assert!(s.world.is_chat_banned(ObjectId(ALICE)));
    s.persists();

    s.say(MODERATOR, ".chatban Alice");
    assert_eq!(
        texts(&s.sent(), MODERATOR),
        vec!["Player is already punished!".to_owned()]
    );

    s.say(MODERATOR, ".unbanchat Alice");
    let sent = s.sent();
    assert_eq!(
        texts(&sent, MODERATOR),
        vec!["Player Alice chat unbanned.".to_owned()]
    );
    assert_eq!(
        texts(&sent, ALICE).last().map(String::as_str),
        Some("Chat unbanned by moderator Mod")
    );
    assert!(!s.world.is_chat_banned(ObjectId(ALICE)));
    s.say(MODERATOR, ".unbanchat Alice");
    assert_eq!(
        texts(&s.sent(), MODERATOR),
        vec!["Player is not chat banned!".to_owned()]
    );
}

// ----------------------------------------------------------------------- WalkerBotProtection

#[test]
fn walker_protection_off_lets_the_whisper_through() {
    let mut s = Scene::new(|_| {});
    s.enter(ALICE, "Alice", 0, None);
    s.enter(BOB, "Bob", 0, None);
    s.sent();
    s.say_as(ALICE, ChatType::Whisper, "USESKILL 1", Some("Bob"));
    let sent = s.sent();
    assert!(texts(&sent, ALICE).is_empty());
    assert_eq!(says_to(&sent, BOB).len(), 1);
}

#[test]
fn walker_protection_on_punishes_a_walker_command_in_a_whisper() {
    let mut s = Scene::new(|c| c.l2walker_protection = true);
    s.enter(ALICE, "Alice", 0, None);
    s.enter(BOB, "Bob", 0, None);
    s.sent();
    // Only a whisper counts.
    s.say(ALICE, "USESKILL 1");
    assert!(texts(&s.sent(), ALICE).is_empty());
    s.say_as(ALICE, ChatType::Whisper, "hello", Some("Bob"));
    assert_eq!(says_to(&s.sent(), BOB).len(), 1);
    // A walker command: an illegal action (DEFAULT_PUNISH is KICK) and not sent.
    s.say_as(ALICE, ChatType::Whisper, "MOVETO 1 2 3", Some("Bob"));
    let sent = s.sent();
    assert_eq!(
        texts(&sent, ALICE),
        vec!["You will be kicked for illegal action, GM informed.".to_owned()]
    );
    assert!(says_to(&sent, BOB).is_empty());
}

// ------------------------------------------------------------------------------ PasswordChange

#[test]
fn change_password_off_is_plain_chat() {
    let mut s = Scene::new(|_| {});
    s.enter(ALICE, "Alice", 0, None);
    s.sent();
    s.say(ALICE, ".changepassword a b b");
    assert_eq!(says_to(&s.sent(), ALICE).len(), 1);
    assert!(s.ls_events().is_empty());
}

#[test]
fn change_password_on_asks_the_login_server() {
    let mut s = Scene::new(|c| c.allow_change_password = true);
    s.enter(ALICE, "Alice", 0, None);
    s.sent();
    s.say(ALICE, ".changepassword");
    assert_eq!(html_to(&s.sent(), ALICE).len(), 1);

    s.say(ALICE, ".changepassword old new");
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec!["Invalid password data! You have to fill all boxes.".to_owned()]
    );
    s.say(ALICE, ".changepassword old new other");
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec!["The new password doesn't match with the repeated one!".to_owned()]
    );
    s.say(ALICE, ".changepassword old ab ab");
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec!["The new password is shorter than 3 chars! Please try with a longer one.".to_owned()]
    );
    let long = "x".repeat(31);
    s.say(ALICE, &format!(".changepassword old {long} {long}"));
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec!["The new password is longer than 30 chars! Please try with a shorter one.".to_owned()]
    );
    assert!(s.ls_events().is_empty());

    s.say(ALICE, ".changepassword old newpass newpass");
    assert_eq!(
        s.ls_events(),
        vec![LsEvent::ChangePassword {
            account: format!("acc{ALICE}"),
            character: "Alice".to_owned(),
            old_pass: "old".to_owned(),
            new_pass: "newpass".to_owned(),
        }]
    );
    // The bypass of the page takes the same way.
    s.say(ALICE, ".changepassword");
    s.sent();
    s.bypass(ALICE, "voice .changepassword old newpass newpass");
    assert_eq!(s.ls_events().len(), 1);

    // The login server's answer reaches the character.
    s.world.handle(WorldCommand::PasswordChangeResponse {
        character: "Alice".to_owned(),
        text: "Password changed.".to_owned(),
    });
    assert_eq!(texts(&s.sent(), ALICE), vec!["Password changed.".to_owned()]);
}

// -------------------------------------------------------------------------- MultilingualSupport

#[test]
fn lang_command_off_is_plain_chat_and_the_prefix_is_empty() {
    let mut s = Scene::new(|_| {});
    s.enter(ALICE, "Alice", 0, None);
    s.sent();
    s.say(ALICE, ".lang el");
    assert_eq!(says_to(&s.sent(), ALICE).len(), 1);
    assert_eq!(s.world.player_lang(ObjectId(ALICE)), None);
}

#[test]
fn lang_command_on_chooses_a_language() {
    let mut s = Scene::new(|c| {
        c.multilang.enable = true;
        c.multilang.default_lang = "en".into();
        c.multilang.allowed = vec!["en".into(), "el".into()];
    });
    s.enter(ALICE, "Alice", 0, None);
    s.sent();
    s.say(ALICE, ".lang");
    let pages = html_to(&s.sent(), ALICE);
    assert_eq!(pages.len(), 1);
    assert!(pages[0].contains("<button value=\"EN\""), "{}", pages[0]);
    assert!(pages[0].contains("bypass -h voice .lang el"), "{}", pages[0]);

    s.say(ALICE, ".lang xx");
    assert!(html_to(&s.sent(), ALICE)[0].contains("lang error"));
    // A language that is not allowed sets the default one.
    assert_eq!(s.world.player_lang(ObjectId(ALICE)), Some("en"));

    s.say(ALICE, ".lang el");
    assert!(html_to(&s.sent(), ALICE)[0].contains("lang ok"));
    assert_eq!(s.world.player_lang(ObjectId(ALICE)), Some("el"));
    // The page is now read from `data/lang/el/`.
    s.say(ALICE, ".lang el");
    assert!(html_to(&s.sent(), ALICE)[0].contains("lang ok el"));
    // The bypass of the page takes the same way.
    s.say(ALICE, ".lang");
    s.sent();
    s.bypass(ALICE, "voice .lang en");
    assert!(html_to(&s.sent(), ALICE)[0].contains("lang ok"));
    assert_eq!(s.world.player_lang(ObjectId(ALICE)), Some("en"));
}

#[test]
fn lang_command_needs_the_voice_switch() {
    let mut s = Scene::new(|c| {
        c.multilang.enable = true;
        c.multilang.voiced_allow = false;
    });
    s.enter(ALICE, "Alice", 0, None);
    s.sent();
    s.say(ALICE, ".lang");
    assert_eq!(says_to(&s.sent(), ALICE).len(), 1);
}

// --------------------------------------------------------------------------- CustomMailManager

fn mail_entry(receiver: i32, items: &str) -> CustomMailEntry {
    CustomMailEntry {
        date: "2026-01-02 03:04:05".into(),
        receiver,
        subject: "Gift".into(),
        message: "For you".into(),
        items: items.into(),
    }
}

#[test]
fn custom_mail_off_never_reads() {
    let mut s = Scene::new(|c| c.custom_mail_delay_ms = 1000);
    s.world.start_custom_misc();
    s.ticks(100);
    assert!(s.persists().is_empty());
}

#[test]
fn custom_mail_on_reads_every_period_and_sends_to_online_players() {
    let mut s = Scene::new(|c| {
        c.custom_mail_enabled = true;
        c.custom_mail_delay_ms = 1000;
    });
    s.enter(ALICE, "Alice", 0, None);
    s.world.start_custom_misc();
    s.persists();
    s.ticks(19);
    assert!(s.persists().is_empty());
    s.ticks(2);
    assert_eq!(s.persists(), vec![PersistCommand::CustomMailRequest]);
    // The answer is pending: the next period does not ask again.
    s.ticks(30);
    assert!(s.persists().is_empty());

    s.sent();
    s.world.handle(WorldCommand::CustomMailLoaded {
        result: Ok(vec![mail_entry(ALICE, "57 5000 2;3470"), mail_entry(BOB, "57 1")]),
    });
    let persists = s.persists();
    // Alice's row is deleted, Bob (not online) keeps his.
    assert!(persists.contains(&PersistCommand::CustomMailDelete {
        date: "2026-01-02 03:04:05".into(),
        receiver: ALICE,
    }));
    assert!(!persists.iter().any(|p| matches!(
        p,
        PersistCommand::CustomMailDelete { receiver, .. } if *receiver == BOB
    )));
    // The mail and its two attachments were stored.
    let mail = persists
        .iter()
        .find_map(|p| match p {
            PersistCommand::Mail(_) => Some(()),
            _ => None,
        })
        .is_some();
    assert!(mail);
    assert!(
        persists
            .iter()
            .any(|p| matches!(p, PersistCommand::ItemRows { .. }))
    );
    assert!(has(&s.sent(), ALICE, "EX_NOTICE_POST_ARRIVED"));
    assert!(s.world.mail_has_unread_post(ObjectId(ALICE)));
    assert_eq!(s.world.mail_inbox_size(ALICE), 1);

    // A failed read is only logged and the next period asks again.
    s.world.handle(WorldCommand::CustomMailLoaded { result: Err(()) });
    s.ticks(21);
    assert_eq!(s.persists(), vec![PersistCommand::CustomMailRequest]);
}

// ------------------------------------------------------------------------------- DualboxCheck

#[test]
fn string_hash_matches_java() {
    // "127.0.0.1".hashCode() in Java.
    assert_eq!(java_string_hash("127.0.0.1"), 1_505_998_205);
}

#[test]
fn dualbox_off_lets_everybody_select() {
    let mut s = Scene::new(|_| {});
    for _ in 0..5 {
        assert_eq!(s.world.dualbox_check_select(ip("10.0.0.1"), 0), None);
    }
}

#[test]
fn dualbox_on_limits_the_connections_of_one_address() {
    let mut s = Scene::new(|c| c.dualbox.max_players_per_ip = 2);
    let a = ip("10.0.0.1");
    assert_eq!(s.world.dualbox_check_select(a, 0), None);
    assert_eq!(s.world.dualbox_check_select(a, 0), None);
    let page = s.world.dualbox_check_select(a, 0).unwrap();
    assert_eq!(
        page,
        "<html><body>Maximum 2 connection(s) per IP address allowed.</body></html>"
    );
    // Another address, and a high access level, are not limited.
    assert_eq!(s.world.dualbox_check_select(ip("10.0.0.2"), 0), None);
    assert_eq!(s.world.dualbox_check_select(a, 100), None);
    // A client that left before entering frees its place.
    s.world.handle(WorldCommand::DualboxRelease { ip: a });
    assert_eq!(s.world.dualbox_check_select(a, 0), None);
    // A player who leaves the world frees it too.
    s.enter(ALICE, "Alice", 0, a);
    assert!(s.world.dualbox_check_select(a, 0).is_some());
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(ALICE as u64),
    });
    assert_eq!(s.world.dualbox_check_select(a, 0), None);
}

#[test]
fn dualbox_whitelist_never_matches_an_address_like_java() {
    // The counters use `String.hashCode()` of the address, the whitelist `InetAddress.hashCode()`.
    let mut s = Scene::new(|c| {
        c.dualbox.max_players_per_ip = 1;
        c.dualbox.whitelist.insert(0x0a00_0001, 5);
    });
    let a = ip("10.0.0.1");
    assert_eq!(s.world.dualbox_check_select(a, 0), None);
    assert!(s.world.dualbox_check_select(a, 0).is_some());
    assert_eq!(s.world.anti_feed_limit(a, 1), 1);
    // A whitelist entry under the string hash does apply; 0 or less is unlimited.
    let hash = java_string_hash("10.0.0.1");
    let mut t = Scene::new(|c| {
        c.dualbox.max_players_per_ip = 1;
        c.dualbox.whitelist.insert(hash, 2);
    });
    assert_eq!(t.world.anti_feed_limit(a, 1), 3);
    assert_eq!(t.world.dualbox_check_select(a, 0), None);
    assert_eq!(t.world.dualbox_check_select(a, 0), None);
    assert_eq!(t.world.dualbox_check_select(a, 0), None);
    assert!(t.world.dualbox_check_select(a, 0).is_some());
    let u = Scene::new(|c| {
        c.dualbox.whitelist.insert(hash, -1);
    });
    assert_eq!(u.world.anti_feed_limit(a, 1), i32::MAX);
    assert_eq!(u.world.anti_feed_limit(None, 7), 7);
}

#[test]
fn anti_feed_events_count_separately() {
    let mut s = Scene::new(|_| {});
    let a = ip("10.0.0.1");
    // An event that is not registered refuses.
    assert!(!s.world.anti_feed_try_add_client(OLYMPIAD_ID, a, 1));
    s.world.anti_feed_register_event(OLYMPIAD_ID);
    s.world.anti_feed_register_event(GAME_ID);
    assert!(s.world.anti_feed_try_add_client(OLYMPIAD_ID, a, 1));
    assert!(!s.world.anti_feed_try_add_client(OLYMPIAD_ID, a, 1));
    assert!(s.world.anti_feed_try_add_client(GAME_ID, a, 1));
    // No client address: refused.
    assert!(!s.world.anti_feed_try_add_client(GAME_ID, None, 1));
    // `removeClient` answers whether the address still has connections.
    assert!(!s.world.anti_feed_remove_client(OLYMPIAD_ID, a));
    assert!(s.world.anti_feed_try_add_client(OLYMPIAD_ID, a, 1));
    s.world.anti_feed_clear(OLYMPIAD_ID);
    assert!(s.world.anti_feed_try_add_client(OLYMPIAD_ID, a, 1));
}

// ------------------------------------------------------------------------------------- Captcha

#[test]
fn captcha_challenge_asks_for_the_code_and_accepts_it() {
    let mut s = Scene::new(|c| {
        c.captcha.enable = true;
        c.captcha.validation_time = 60;
    });
    s.enter(ALICE, "Alice", 0, None);
    s.sent();
    s.world.captcha_validation_tasks(ObjectId(ALICE));
    let sent = s.sent();
    assert!(has(&sent, ALICE, "PLEDGE_CREST"));
    assert!(s.world.captcha_is_in_report_mode(ObjectId(ALICE)));
    let code = s.world.captcha_code_of(ObjectId(ALICE)).unwrap();
    assert_eq!(code.len(), 5);
    assert!(code.chars().all(|c| c.is_ascii_uppercase()));

    // The first window comes with the countdown; the other windows only after `continue`.
    let sent = {
        s.ticks(2);
        s.sent()
    };
    assert!(html_to(&sent, ALICE)[0].contains("report_continue"));
    // The first second of the countdown: one minute is left.
    assert_eq!(
        texts(&sent, ALICE),
        vec!["1 minute(s) to enter the code.".to_owned()]
    );
    s.bypass(ALICE, "report_continue");
    let page = &html_to(&s.sent(), ALICE)[0];
    assert!(page.contains("Crest.crest_1_"), "{page}");

    // A wrong code: one more attempt.
    s.bypass(ALICE, "report_ NOPE");
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec!["Incorrect code. You have 1 more attempt(s) left.".to_owned()]
    );
    // The right one, in lower case and with spaces around it.
    s.bypass(ALICE, &format!("report_  {} ", code.to_lowercase()));
    assert_eq!(
        texts(&s.sent(), ALICE),
        vec!["Congratulations, code matches!".to_owned()]
    );
    assert!(!s.world.captcha_is_in_report_mode(ObjectId(ALICE)));
    // Nothing is checked any more.
    s.bypass(ALICE, "report_ NOPE");
    assert!(texts(&s.sent(), ALICE).is_empty());
}

#[test]
fn captcha_counts_down_and_punishes_in_the_time_limit() {
    let mut s = Scene::new(|c| {
        c.captcha.enable = true;
        c.captcha.validation_time = 5;
        c.captcha.punishment = 1;
    });
    s.enter(ALICE, "Alice", 0, None);
    s.world.captcha_validation_tasks(ObjectId(ALICE));
    s.sent();
    let sent = {
        s.ticks(5 * 20 + 2);
        s.sent()
    };
    let said = texts(&sent, ALICE);
    for second in ["5", "4", "3", "2", "1"] {
        assert!(
            said.contains(&format!("{second} second(s) to enter the code!")),
            "{said:?}"
        );
    }
    // The punishment 1 kicks the player: no "code doesn't match" message follows.
    assert!(!said.iter().any(|t| t.contains("doesn't match")));
    assert!(s.world.player(ObjectId(ALICE)).is_none());
}

#[test]
fn captcha_wrong_answers_end_with_the_punishment_of_the_config() {
    // Moved to the town (0): the message comes last.
    let mut s = Scene::new(|c| {
        c.captcha.enable = true;
        c.captcha.captcha_attempts = 2;
        c.captcha.punishment = 0;
    });
    s.enter(ALICE, "Alice", 0, None);
    s.world.captcha_validation_tasks(ObjectId(ALICE));
    s.sent();
    s.bypass(ALICE, "report_ WRONG");
    s.bypass(ALICE, "report_ WRONG");
    let said = texts(&s.sent(), ALICE);
    assert_eq!(
        said.last().map(String::as_str),
        Some("Unfortunately, code doesn't match.")
    );
    assert!(!s.world.captcha_is_in_report_mode(ObjectId(ALICE)));

    // Jailed (2): a jail punishment is stored.
    let mut j = Scene::new(|c| {
        c.captcha.enable = true;
        c.captcha.captcha_attempts = 1;
        c.captcha.punishment = 2;
        c.captcha.jail_time = 3;
    });
    j.enter(ALICE, "Alice", 0, None);
    j.world.captcha_validation_tasks(ObjectId(ALICE));
    j.persists();
    j.bypass(ALICE, "report_ WRONG");
    assert!(j.world.is_jailed(ObjectId(ALICE)));

    // Banned (3).
    let mut b = Scene::new(|c| {
        c.captcha.enable = true;
        c.captcha.captcha_attempts = 1;
        c.captcha.punishment = 3;
    });
    b.enter(ALICE, "Alice", 0, None);
    b.world.captcha_validation_tasks(ObjectId(ALICE));
    b.bypass(ALICE, "report_ WRONG");
    assert!(b.world.has_punishment(
        &ALICE.to_string(),
        PunishmentAffect::Character,
        PunishmentType::Ban
    ));
}

#[test]
fn captcha_bypass_without_a_challenge_does_nothing() {
    let mut s = Scene::new(|c| c.captcha.enable = true);
    s.enter(ALICE, "Alice", 0, None);
    s.sent();
    s.bypass(ALICE, "report_ ABCDE");
    s.bypass(ALICE, "report_continue");
    assert!(s.sent().is_empty());
}

#[allow(dead_code)]
fn unused(s: &Scene) -> &std::path::Path {
    s.root.path()
}
