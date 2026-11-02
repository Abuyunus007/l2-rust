#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The buffer customs (M9-C03): the custom community board (`CommunityBoard.ini`), the scheme
//! buffer NPC (`SchemeBuffer.ini`) and the buff shop (`SellBuffs.ini`). Each feature has a test
//! with the switch off (nothing changes) and tests with it on; the expected values come from the
//! Java code named in each test (`HomeBoard`, `SchemeBuffer`, `SchemeBufferTable`,
//! `SellBuffsManager`, `SellBuffBypassHandler`). The pages are small stand-ins in a temporary
//! datapack root with the links of the real pages.

use std::collections::{BTreeSet, HashMap};
use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates, KarmaLoss,
    NpcLoadOptions, NpcTemplates, PlayerTemplates, SchemeBufferSkills, SellBuffData, SkillTemplates,
    SkillTrees, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{NpcHtmlMessage, ShowBoard, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Debug, Clone, PartialEq)]
enum Out {
    Packet(i32, Vec<u8>),
    Persist(PersistCommand),
}

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Out>>>);

impl Log {
    fn take(&self) -> Vec<Out> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push(Out::Packet(self.0, bytes));
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

struct RecOutbox(Log);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

fn class_xml() -> String {
    let rows: String = (1..=85)
        .map(|l| {
            format!(
                "<level val=\"{l}\"><hp>{}</hp><mp>{}</mp><cp>{l}</cp></level>",
                100 + l,
                50 + l
            )
        })
        .collect();
    format!(
        r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{rows}</lvlUpgainData></list>"#
    )
}

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
</list>"#;

/// Three buffs (1040 and 1059 are listed by the buffer and the board, 1003 is a dance) and one
/// that is not listed anywhere.
const SKILLS: &str = r##"<list>
<skill id="1040" levels="1" name="Shield"><icon>icon.skill1040</icon><operateType>A2</operateType><targetType>ONE</targetType>
<mpConsume>10</mpConsume><abnormalTime>60</abnormalTime><abnormalType>PD_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pDef" val="1.1"/></effect></effects></skill>
<skill id="1059" levels="1" name="Empower"><icon>icon.skill1059</icon><operateType>A2</operateType><targetType>ONE</targetType>
<abnormalTime>60</abnormalTime><abnormalType>MA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="mAtk" val="1.1"/></effect></effects></skill>
<skill id="1003" levels="1" name="Dance of Fire"><icon>icon.skill1003</icon><operateType>A2</operateType><targetType>ONE</targetType><isMagic>3</isMagic>
<abnormalTime>60</abnormalTime><abnormalType>CRITICAL_DMG_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="critDmg" val="1.1"/></effect></effects></skill>
<skill id="1068" levels="1" name="Might"><icon>icon.skill1068</icon><operateType>A2</operateType><targetType>ONE</targetType>
<abnormalTime>60</abnormalTime><abnormalType>PA_UP</abnormalType><abnormalLevel>1</abnormalLevel>
<effects><effect name="Buff"><mul stat="pAtk" val="1.1"/></effect></effects></skill>
</list>"##;

/// `SchemeBufferSkills.xml`: 1040 costs 100, 1059 is free, 1003 is a dance; the auto-buff groups
/// are the hidden `MAGE_GROUP` / `FIGHTER_GROUP`.
const SCHEME_SKILLS: &str = r#"<list>
<category type="Buffs">
<buff id="1040" level="1" price="100" desc="Increases P. Def." />
<buff id="1059" level="1" price="0" desc="Increases M. Atk." />
</category>
<category type="Dances"><buff id="1003" level="1" price="5" desc="Dance." /></category>
<category type="MAGE_GROUP"><buff id="1059" level="1" price="0" desc="" /></category>
<category type="FIGHTER_GROUP"><buff id="1068" level="1" price="0" desc="" /></category>
</list>"#;

/// `SellBuffData.xml`: 1040 and 1059 may be sold.
const SELL_DATA: &str = r#"<list><skill id="1040" /><skill id="1059" /></list>"#;

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: i16::try_from(t.type2).unwrap(),
            body_part: t.body_part.mask(),
            quest_item: t.quest_item,
            weight: t.weight,
            shared_reuse_group: t.shared_reuse_group,
            weapon: t.is_weapon(),
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
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.0.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&class_xml(), 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const A: i32 = 0x6000_0001;
const B: i32 = 0x6000_0002;
const BUFFER: i32 = 50008;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="50008" type="SchemeBuffer" level="70" name="Thiago" usingServerSideName="true">
    <status attackable="false" />
  </npc>
</list>"#;

/// The links every stand-in page of the buffer carries: the real pages have the same links, the
/// world takes a bypass only when the last page offered it.
const BUFFER_LINKS: &str = "\
<a action=\"bypass -h npc_%objectId%_menu\">m</a><a action=\"bypass -h npc_%objectId%_cleanup\">c</a>\
<a action=\"bypass -h npc_%objectId%_heal\">h</a><a action=\"bypass -h npc_%objectId%_support\">s</a>\
<a action=\"bypass -h npc_%objectId%_autobuff\">a</a>\
<a action=\"bypass -h npc_%objectId%_autobuff;$t\">a</a>\
<a action=\"bypass -h npc_%objectId%_givebuffs;$t\">g</a>\
<a action=\"bypass -h npc_%objectId%_editschemes;$t\">e</a>\
<a action=\"bypass -h npc_%objectId%_skillselect;$t\">s</a>\
<a action=\"bypass -h npc_%objectId%_skillunselect;$t\">u</a>\
<a action=\"bypass -h npc_%objectId%_createscheme $name\">n</a>\
<a action=\"bypass -h npc_%objectId%_deletescheme;$t\">d</a>\
<a action=\"bypass -h npc_%objectId%_manual;$t\">m</a>\
<a action=\"bypass -h npc_%objectId%_castbuff;$t\">c</a>";

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
        let board = "data/html/CommunityBoard/Custom/";
        write(&format!("{board}navigation.html"), "NAV");
        write(&format!("{board}home.html"), "<html>%navigation%HOME</html>");
        write(
            &format!("{board}buffer/main.html"),
            "<html>%navigation%BUFFER</html>",
        );
        write(
            &format!("{board}merchant/main.html"),
            "<html>%navigation%MERCHANT</html>",
        );
        write(
            &format!("{board}delevel/complete.html"),
            "<html>%navigation%DELEVELED</html>",
        );
        let buffer = "data/html/mods/SchemeBuffer/";
        write(
            &format!("{buffer}50008.htm"),
            &format!("<html>MAIN {BUFFER_LINKS}</html>"),
        );
        write(
            &format!("{buffer}50008-1.htm"),
            &format!("<html>SCHEMES %schemes%|max=%max_schemes% {BUFFER_LINKS}</html>"),
        );
        write(
            &format!("{buffer}50008-2.htm"),
            &format!("<html>EDIT %schemename%|%count%|%typesframe%|%skilllistframe% {BUFFER_LINKS}</html>"),
        );
        write(
            &format!("{buffer}50008-3.htm"),
            &format!("<html>MANUAL %category%|%skills% {BUFFER_LINKS}</html>"),
        );
        let shop = "data/html/mods/SellBuffs/";
        write(
            &format!("{shop}BuffMenu.html"),
            "<html>MENU <button action=\"bypass -h sellbuffadd\"><button action=\"bypass -h sellbuffedit\">\
             <button action=\"bypass -h sellbuffstart $title\"></html>",
        );
        write(
            &format!("{shop}BuffMenu_already.html"),
            "<html>ALREADY <button action=\"bypass -h sellbuffstop\"></html>",
        );
        write(
            &format!("{shop}BuffChoice.html"),
            "<html>CHOICE %list% <button action=\"bypass -h voice .sellbuff\"></html>",
        );
        write(&format!("{shop}BuffBuyMenu.html"), "<html>BUY %list%</html>");
        Self(dir)
    }
}

struct Scene {
    world: World,
    log: Log,
    _root: Root,
}

fn adena(count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(0x1000_0001),
        item_id: 57,
        count,
        loc: "INVENTORY".to_owned(),
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

fn pdata(id: i32, level: i32, adena_count: i64, skills: &[i32]) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{}", id - 0x6000_0000),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X + if id == B { 20 } else { 0 },
        y: Y,
        z: Z,
        heading: 0,
        cur_hp: 10.0,
        max_hp: 100.0,
        cur_mp: 30.0,
        max_mp: 50.0,
        cur_cp: 5.0,
        max_cp: 20.0,
        online_time: 0,
        items: if adena_count > 0 {
            vec![adena(adena_count)]
        } else {
            vec![]
        },
        skills: skills
            .iter()
            .map(|&skill_id| SkillData {
                skill_id,
                skill_level: 1,
            })
            .collect(),
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn experience_table() -> Arc<ExperienceTable> {
    let rows: String = (1..=88)
        .map(|level| {
            format!(
                r#"<experience level="{level}" tolevel="{}" />"#,
                level * level * 1000
            )
        })
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
        ))
        .unwrap(),
    )
}

impl Scene {
    fn new() -> Self {
        let root = Root::new();
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(RecOutbox(log.clone())));
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(
            root.0.path(),
            &npcs,
            &SpawnLoadOptions::default(),
        ));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.0.path(), false)));
        world.set_exp_data(
            experience_table(),
            Arc::new(ExperienceLoss::from_xml(r#"<list><experience level="5" val="9.5" /></list>"#).unwrap()),
            Arc::new(KarmaLoss::from_xml(r#"<list><modifier level="10" val="2.0" /></list>"#).unwrap()),
            Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
        );
        Self {
            world,
            log,
            _root: root,
        }
    }

    fn enter(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.log.take();
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    fn bypass(&mut self, player: i32, command: &str) {
        self.input(
            player,
            ClientInput::Bypass {
                command: command.to_owned(),
            },
        );
    }

    fn say(&mut self, player: i32, text: &str) {
        self.input(
            player,
            ClientInput::Say {
                chat_type: 0,
                text: text.to_owned(),
                target: None,
            },
        );
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn adena_of(&self, player: i32) -> i64 {
        self.world
            .player(ObjectId(player))
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == 57)
            .map(|i| i.count)
            .sum()
    }

    fn hp_of(&self, player: i32) -> f64 {
        self.world.player(ObjectId(player)).unwrap().core.cur_hp
    }

    fn affected(&self, player: i32, skill_id: i32) -> bool {
        self.world.is_affected_by_skill(ObjectId(player), skill_id)
    }

    // ---- the board

    fn board(&mut self, custom: bool, settings: CustomBoardSettings) {
        self.world.set_community_board_settings(CommunityBoardSettings {
            custom_cb_enabled: custom,
            ..CommunityBoardSettings::default()
        });
        self.world.set_custom_board_settings(settings);
    }

    // ---- the scheme buffer

    fn buffer(&mut self, settings: SchemeBufferSettings) -> ObjectId {
        self.world.set_scheme_buffer(
            settings,
            Arc::new(SchemeBufferSkills::from_xml(SCHEME_SKILLS).unwrap()),
        );
        let npc = self.world.spawn_npc_at(BUFFER, X + 50, Y, Z, 0).unwrap();
        self.input(
            A,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
        // The click selects the NPC; a second one talks.
        self.input(
            A,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
        self.log.take();
        npc
    }

    /// Sends `npc_<id>_<command>` and returns what happened.
    fn npc(&mut self, npc: ObjectId, command: &str) -> Vec<Out> {
        self.bypass(A, &format!("npc_{}_{command}", npc.0));
        self.log.take()
    }

    // ---- the shop

    fn shop(&mut self, enabled: bool) {
        self.world.set_sell_buffs(
            SellBuffsSettings {
                enabled,
                min_price: 100,
                max_price: 1000,
                max_buffs: 2,
                ..SellBuffsSettings::default()
            },
            Arc::new(SellBuffData::from_xml(SELL_DATA).unwrap()),
        );
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

fn packets(out: &[Out], to: i32, name: &str) -> Vec<Vec<u8>> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, bytes) if *id == to && name_of(bytes) == name => Some(bytes.clone()),
            _ => None,
        })
        .collect()
}

/// The text messages (`sendMessage`) sent to a player.
fn texts(out: &[Out], to: i32) -> Vec<String> {
    packets(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|b| decode::<SystemMessage>(b))
        .filter(|m| m.id == SystemMessageId::S12)
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

/// The `ShowBoard` contents sent to a player.
fn board_packets(out: &[Out], to: i32) -> Vec<ShowBoard> {
    packets(out, to, "SHOW_BOARD")
        .iter()
        .map(|b| decode::<ShowBoard>(b))
        .collect()
}

/// The page of a board answer ("101" packet).
fn board_page(out: &[Out], to: i32) -> Option<String> {
    board_packets(out, to)
        .into_iter()
        .find_map(|p| p.content.strip_prefix("101\u{8}").map(str::to_owned))
}

fn html_of(out: &[Out], to: i32) -> Vec<String> {
    packets(out, to, "NPC_HTML_MESSAGE")
        .iter()
        .map(|b| decode::<NpcHtmlMessage>(b).html)
        .collect()
}

fn scheme_rows(out: &[Out]) -> Vec<(i32, Vec<(String, String)>)> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::BufferSchemes { object_id, schemes }) => {
                Some((object_id.0, schemes.clone()))
            }
            _ => None,
        })
        .collect()
}

fn custom_settings() -> CustomBoardSettings {
    CustomBoardSettings {
        teleport_price: 10,
        buff_price: 50,
        heal_price: 100,
        delevel_price: 25,
        available_buffs: BTreeSet::from([1040]),
        available_teleports: HashMap::from([("Giran".to_owned(), (82698, 148638, -3473))]),
        ..CustomBoardSettings::default()
    }
}

// ---------------------------------------------------------------------------------- the board

/// Off (`CustomCommunityBoard = False`): none of the custom words is a board command, so the
/// heal neither costs nor heals and no page goes out.
#[test]
fn board_switch_off_takes_no_custom_command() {
    let mut s = Scene::new();
    s.board(false, custom_settings());
    s.enter(pdata(A, 40, 1000, &[]));
    for word in [
        "_bbsheal;merchant/main",
        "_bbsteleport;Giran",
        "_bbsbuff;1040,1;buffer/main",
        "_bbsdelevel",
    ] {
        assert!(!s.world.is_community_board_command(word), "{word}");
        s.bypass(A, word);
    }
    let out = s.log.take();
    assert!(board_packets(&out, A).is_empty());
    assert_eq!(s.adena_of(A), 1000);
    assert!((s.hp_of(A) - 10.0).abs() < f64::EPSILON);
    // The plain home page keeps working.
    s.bypass(A, "_bbstop;merchant/main.html");
    assert!(s.world.is_community_board_command("_bbshome"));
}

/// `_bbsheal`: `COMMUNITYBOARD_HEAL_PRICE` in the currency, HP/MP/CP to the maximum, "You used
/// heal!", then the page of the command with `%navigation%` replaced.
#[test]
fn board_heal_charges_and_refills() {
    let mut s = Scene::new();
    s.board(true, custom_settings());
    s.enter(pdata(A, 40, 1000, &[]));
    assert!(s.world.is_community_board_command("_bbsheal;merchant/main"));
    s.bypass(A, "_bbsheal;merchant/main");
    let out = s.log.take();
    assert_eq!(s.adena_of(A), 900);
    let max_hp = f64::from(s.world.stats(ObjectId(A)).unwrap().max_hp());
    assert!(max_hp > 10.0);
    assert!((s.hp_of(A) - max_hp).abs() < f64::EPSILON);
    assert!(texts(&out, A).contains(&"You used heal!".to_owned()));
    assert_eq!(board_page(&out, A).as_deref(), Some("<html>NAVMERCHANT</html>"));
}

/// `Not enough currency!`: nothing is taken and nothing healed; the page still comes (Java builds
/// `returnHtml` after the branch).
#[test]
fn board_heal_without_enough_currency() {
    let mut s = Scene::new();
    s.board(true, custom_settings());
    s.enter(pdata(A, 40, 99, &[]));
    s.bypass(A, "_bbsheal;merchant/main");
    let out = s.log.take();
    assert!(texts(&out, A).contains(&"Not enough currency!".to_owned()));
    assert_eq!(s.adena_of(A), 99);
    assert!((s.hp_of(A) - 10.0).abs() < f64::EPSILON);
    assert_eq!(board_page(&out, A).as_deref(), Some("<html>NAVMERCHANT</html>"));
}

/// `COMBAT_CHECK`: a dead (alike dead) player gets "You can't use the Community Board right now."
/// for the custom words only.
#[test]
fn board_combat_check_refuses_custom_words() {
    let mut s = Scene::new();
    s.board(true, custom_settings());
    let mut dead = pdata(A, 40, 1000, &[]);
    dead.cur_hp = 0.0;
    s.enter(dead);
    s.bypass(A, "_bbsheal;merchant/main");
    let out = s.log.take();
    assert_eq!(texts(&out, A), ["You can't use the Community Board right now."]);
    assert_eq!(s.adena_of(A), 1000);
    // With `CommunityCombatDisabled = False` the check is off.
    let mut open = custom_settings();
    open.combat_disabled = false;
    s.world.set_custom_board_settings(open);
    s.bypass(A, "_bbsheal;merchant/main");
    assert!(
        texts(&s.log.take(), A)
            .iter()
            .all(|t| t != "You can't use the Community Board right now.")
    );
}

/// `_bbsbuff;<id>,<level>;...;<page>`: the fee is `price * buffCount` (also for a skill the list
/// does not allow, which is skipped), the allowed skills land on the player.
#[test]
fn board_buffs_charge_per_buff_and_skip_unlisted_skills() {
    let mut s = Scene::new();
    s.board(true, custom_settings());
    s.enter(pdata(A, 40, 1000, &[]));
    s.bypass(A, "_bbsbuff;1040,1;1059,1;buffer/main");
    let out = s.log.take();
    assert_eq!(s.adena_of(A), 900);
    assert!(s.affected(A, 1040));
    assert!(!s.affected(A, 1059));
    assert_eq!(board_page(&out, A).as_deref(), Some("<html>NAVBUFFER</html>"));
    // A skill the table does not know ends the command after the fee (Java's NPE).
    s.bypass(A, "_bbsbuff;7777,1;buffer/main");
    assert_eq!(s.adena_of(A), 850);
    assert!(board_page(&s.log.take(), A).is_none());
}

/// `_bbsteleport;<name>`: the fee, the board closes, the skills are disabled and come back after
/// 3 s; a name not in the list does nothing.
#[test]
fn board_teleport_pays_closes_the_board_and_locks_skills() {
    let mut s = Scene::new();
    s.board(true, custom_settings());
    s.enter(pdata(A, 40, 1000, &[]));
    s.bypass(A, "_bbsteleport;Nowhere");
    assert_eq!(s.adena_of(A), 1000);
    assert!(board_packets(&s.log.take(), A).is_empty());

    s.bypass(A, "_bbsteleport;Giran");
    let out = s.log.take();
    assert_eq!(s.adena_of(A), 990);
    let shown = board_packets(&out, A);
    assert!(shown.iter().any(|p| !p.show), "the board is hidden");
    assert!(s.world.all_skills_locked(ObjectId(A)));
    s.advance(3_100);
    assert!(!s.world.all_skills_locked(ObjectId(A)));
}

/// `_bbsdelevel` is its own switch (`CommunityEnableDelevel`, default False); on, it refuses level
/// 1 and otherwise takes one level with the exp of that level.
#[test]
fn board_delevel() {
    let mut s = Scene::new();
    s.board(true, custom_settings());
    assert!(!s.world.is_community_board_command("_bbsdelevel"));
    let mut with = custom_settings();
    with.enable_delevel = true;
    s.world.set_custom_board_settings(with);
    assert!(s.world.is_community_board_command("_bbsdelevel"));

    s.enter(pdata(A, 1, 1000, &[]));
    s.bypass(A, "_bbsdelevel");
    assert_eq!(texts(&s.log.take(), A), ["You are at minimum level!"]);
    assert_eq!(s.adena_of(A), 1000);

    s.enter(pdata(B, 40, 1000, &[]));
    s.bypass(B, "_bbsdelevel");
    let out = s.log.take();
    let player = s.world.player(ObjectId(B)).unwrap();
    assert_eq!(player.core.level, 39);
    assert_eq!(player.data.exp, exp_of_level(39));
    assert_eq!(s.adena_of(B), 975);
    assert_eq!(board_page(&out, B).as_deref(), Some("<html>NAVDELEVELED</html>"));
}

/// `experience.xml` row of a level as the test table writes it: the exp the level starts at.
fn exp_of_level(level: i32) -> i64 {
    experience_table().exp_for_level(level).unwrap_or(0)
}

/// `_bbsmultisell;<list>,<page>` shows the page and runs the multisell 100 ms later (no such list
/// here: Java warns and sends nothing).
#[test]
fn board_multisell_shows_the_page_and_runs_later() {
    let mut s = Scene::new();
    s.board(true, custom_settings());
    s.enter(pdata(A, 40, 0, &[]));
    s.bypass(A, "_bbsmultisell;100,merchant/main");
    let out = s.log.take();
    assert_eq!(board_page(&out, A).as_deref(), Some("<html>NAVMERCHANT</html>"));
    s.advance(200);
    // A malformed command ends at the exception.
    s.bypass(A, "_bbsmultisell;x,merchant/main");
    assert!(board_page(&s.log.take(), A).is_none());
    s.bypass(A, "_bbsexcmultisell;100");
    assert!(board_page(&s.log.take(), A).is_none());
}

// ---------------------------------------------------------------------------------- the buffer

/// `createscheme`, `editschemes`, `skillselect`, `givebuffs`, `skillunselect`, `deletescheme`:
/// the schemes of a player, the rows written after each change, the fee of a scheme.
#[test]
fn scheme_buffer_create_edit_give_and_delete() {
    let mut s = Scene::new();
    s.enter(pdata(A, 40, 1000, &[]));
    let npc = s.buffer(SchemeBufferSettings {
        max_schemes: 2,
        ..SchemeBufferSettings::default()
    });

    // A name with a bad character, a long one, an empty one.
    let out = s.npc(npc, "createscheme bad_name");
    assert_eq!(texts(&out, A), ["Please use plain alphanumeric characters."]);
    let out = s.npc(npc, "createscheme ThisNameIsFarTooLong");
    assert_eq!(texts(&out, A), ["Scheme's name must contain up to 14 chars."]);

    let out = s.npc(npc, "createscheme Alpha");
    assert_eq!(
        scheme_rows(&out),
        [(A, vec![("Alpha".to_owned(), String::new())])]
    );
    let page = html_of(&out, A).pop().unwrap();
    assert!(page.contains("Alpha [0 skill(s)]"), "{page}");
    assert!(
        page.contains(&format!("npc_{}_givebuffs;Alpha;0", npc.0)),
        "{page}"
    );
    assert!(page.contains("|max=2"));

    // The same name in another case, then the limit.
    let out = s.npc(npc, "createscheme ALPHA");
    assert_eq!(texts(&out, A), ["The scheme name already exists: ALPHA."]);
    s.npc(npc, "createscheme Beta");
    let out = s.npc(npc, "createscheme Gamma");
    assert_eq!(texts(&out, A), ["Maximum schemes amount is already reached."]);

    // Edit: the types frame hides the two auto-buff groups, the list offers the select button.
    let out = s.npc(npc, "editschemes;Buffs;Alpha;1");
    let page = html_of(&out, A).pop().unwrap();
    assert!(page.contains("EDIT Alpha|0 / "), "{page}");
    assert!(page.contains(&format!("npc_{}_skillselect;Buffs;Alpha;1040;1", npc.0)));
    assert!(page.contains(">Dances</a>") && !page.contains("MAGE_GROUP") && !page.contains("FIGHTER_GROUP"));

    // Select 1040 (once), the fee of the scheme is its price.
    let out = s.npc(npc, "skillselect;Buffs;Alpha;1040;1");
    assert_eq!(
        scheme_rows(&out),
        [(
            A,
            vec![
                ("Alpha".to_owned(), "1040".to_owned()),
                ("Beta".to_owned(), String::new())
            ]
        )]
    );
    let out = s.npc(npc, "skillselect;Buffs;Alpha;1040;1");
    assert!(scheme_rows(&out).is_empty(), "already in the scheme");
    let page = html_of(&s.npc(npc, "support"), A).pop().unwrap();
    assert!(page.contains("Alpha [1 skill(s)] - cost: 100"), "{page}");

    // `givebuffs`: the cost token of the bypass is never trusted, the fee is computed.
    s.npc(npc, "givebuffs;Alpha;1");
    assert_eq!(s.adena_of(A), 900);
    assert!(s.affected(A, 1040));
    // Another player's name is not a scheme of mine.
    let out = s.npc(npc, "givebuffs;Nope;0");
    assert_eq!(texts(&out, A), ["Invalid scheme name: Nope."]);
    // A broke player pays nothing and gets nothing.
    s.world.stop_all_effects(ObjectId(A));
    s.world.destroy_item_by_item_id(ObjectId(A), 57, 850, false);
    s.npc(npc, "givebuffs;Alpha;0");
    assert!(!s.affected(A, 1040));
    assert_eq!(s.adena_of(A), 50);

    // Unselect and delete.
    let out = s.npc(npc, "skillunselect;Buffs;Alpha;1040;1");
    assert_eq!(scheme_rows(&out).len(), 1);
    let out = s.npc(npc, "deletescheme;Alpha");
    assert_eq!(scheme_rows(&out), [(A, vec![("Beta".to_owned(), String::new())])]);
    let out = s.npc(npc, "deletescheme;Beta");
    assert_eq!(scheme_rows(&out), [(A, vec![])]);
    let page = html_of(&out, A).pop().unwrap();
    assert!(page.contains("You haven't defined any scheme."));
}

/// A static cost per buff (`BufferStaticCostPerBuff`) replaces the prices, and a free scheme
/// (cost 0) is given without paying.
#[test]
fn scheme_buffer_static_cost() {
    let mut s = Scene::new();
    s.enter(pdata(A, 40, 1000, &[]));
    let npc = s.buffer(SchemeBufferSettings {
        static_buff_cost: 7,
        ..SchemeBufferSettings::default()
    });
    s.npc(npc, "createscheme Pair");
    s.npc(npc, "skillselect;Buffs;Pair;1040;1");
    s.npc(npc, "skillselect;Buffs;Pair;1059;1");
    s.npc(npc, "givebuffs;Pair;0");
    assert_eq!(s.adena_of(A), 986);
    assert!(s.affected(A, 1040) && s.affected(A, 1059));
}

/// Dances have their own limit (`DancesMaxAmount`) and the pet target needs a pet.
#[test]
fn scheme_buffer_dances_pets_and_the_rest() {
    let mut s = Scene::new();
    s.enter(pdata(A, 40, 1000, &[]));
    let npc = s.buffer(SchemeBufferSettings::default());
    s.npc(npc, "createscheme Dancer");
    let out = s.npc(npc, "skillselect;Dances;Dancer;1003;1");
    assert_eq!(scheme_rows(&out).len(), 1);
    // Not a number: ignored.
    let out = s.npc(npc, "skillselect;Dances;Dancer;x;1");
    assert!(html_of(&out, A).is_empty() && scheme_rows(&out).is_empty());
    // No pet.
    let out = s.npc(npc, "givebuffs;Dancer;5;pet");
    assert_eq!(texts(&out, A), ["You don't have a pet."]);
    assert_eq!(s.adena_of(A), 1000);

    // Manual casting: the page lists the category, the cast buff lands on me.
    let out = s.npc(npc, "manual;Buffs;1;me");
    let page = html_of(&out, A).pop().unwrap();
    assert!(page.contains("MANUAL Buffs|"), "{page}");
    assert!(page.contains(&format!("npc_{}_castbuff;1040;Buffs;1;me", npc.0)));
    assert!(page.contains("Increases P. Def."));
    s.npc(npc, "castbuff;1040;Buffs;1;me");
    assert!(s.affected(A, 1040));
    let out = s.npc(npc, "castbuff;1040;Buffs;1;pet");
    assert_eq!(texts(&out, A), ["You don't have a pet."]);
    // An unknown category.
    let out = s.npc(npc, "manual;Nothing;1;me");
    assert_eq!(texts(&out, A), ["That category doesn't contain any skills."]);

    // The auto-buff of a fighter: FIGHTER_GROUP (Might).
    let out = s.npc(npc, "autobuff");
    assert_eq!(texts(&out, A), ["Auto buff applied successfully!"]);
    assert!(s.affected(A, 1068));

    // Cleanup and heal.
    s.npc(npc, "cleanup");
    assert!(!s.affected(A, 1040) && !s.affected(A, 1068));
    s.npc(npc, "heal");
    assert!(s.hp_of(A) > 10.0);
    // A word that is not the buffer's goes to `Npc.onBypassFeedback` (nothing handles it).
    let out = s.npc(npc, "nonsense");
    assert!(html_of(&out, A).is_empty());
}

/// `loadPlayerSchemes`: a skill the list does not offer is left out, and a player is limited by
/// `BufferMaxSchemesPerChar`.
#[test]
fn scheme_buffer_rows_are_loaded_and_filtered() {
    let mut s = Scene::new();
    s.enter(pdata(A, 40, 1000, &[]));
    s.world.set_scheme_buffer(
        SchemeBufferSettings {
            max_schemes: 1,
            ..SchemeBufferSettings::default()
        },
        Arc::new(SchemeBufferSkills::from_xml(SCHEME_SKILLS).unwrap()),
    );
    s.world.load_buffer_schemes(vec![
        (A, "Saved".to_owned(), "1040,9999,1059".to_owned()),
        (A, "Second".to_owned(), "1040".to_owned()),
    ]);
    let npc = s.world.spawn_npc_at(BUFFER, X + 50, Y, Z, 0).unwrap();
    s.input(
        A,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.input(
        A,
        ClientInput::Action {
            object_id: npc.0,
            action_id: 0,
        },
    );
    s.log.take();
    let page = html_of(&s.npc(npc, "support"), A).pop().unwrap();
    assert!(page.contains("Saved [2 skill(s)] - cost: 100"), "{page}");
    assert!(!page.contains("Second"));
}

// ---------------------------------------------------------------------------------- the shop

/// Off (`SellBuffEnable = False`): `.sellbuff` is plain chat and the bypass words are unknown.
#[test]
fn sell_buffs_off_registers_nothing() {
    let mut s = Scene::new();
    s.shop(false);
    s.enter(pdata(A, 40, 1000, &[1040]));
    s.say(A, ".sellbuff");
    s.bypass(A, "sellbuffadd");
    let out = s.log.take();
    assert!(board_packets(&out, A).is_empty());
    assert!(!s.world.is_selling_buffs(ObjectId(A)));
    assert!(!s.world.sell_buffs_enabled());
}

/// The whole round: `.sellbuff` shows the menu, buffs are added with a price (limits of
/// `SellBuffs.ini`), the shop starts (the player sits down in a package store with a title), a
/// customer clicks the seller, buys a buff with the payment item and the shop stops.
#[test]
fn sell_buffs_add_start_buy_and_stop() {
    let mut s = Scene::new();
    s.shop(true);
    s.enter(pdata(A, 40, 0, &[1040, 1059, 1068]));
    s.enter(pdata(B, 40, 1000, &[]));

    s.say(A, ".sellbuff");
    let out = s.log.take();
    assert!(board_page(&out, A).unwrap().starts_with("<html>MENU"));
    // Nothing to start with an empty list.
    s.bypass(A, "sellbuffstart Cheap");
    assert_eq!(
        texts(&s.log.take(), A),
        ["Your list of buffs is empty, please add some buffs first!"]
    );

    // The choice menu lists the allowed skills the player knows (1068 is not allowed).
    s.bypass(A, "sellbuffadd");
    let page = board_page(&s.log.take(), A).unwrap();
    assert!(page.contains("sellbuffaddskill 1040 $price_1040"), "{page}");
    assert!(page.contains("sellbuffaddskill 1059 $price_1059"));
    assert!(!page.contains("sellbuffaddskill 1068"));

    // Price limits, the quirky message of a number that is too big, the list limit.
    s.bypass(A, "sellbuffaddskill 1040 50");
    assert_eq!(texts(&s.log.take(), A), ["Too small price! Minimum price is 100"]);
    s.bypass(A, "sellbuffaddskill 1040 5000");
    assert_eq!(texts(&s.log.take(), A), ["Too big price! Maximum price is 1000"]);
    s.bypass(A, "sellbuffaddskill 1040 99999999999");
    assert_eq!(texts(&s.log.take(), A)[0], "Too big price! Maximum price is 100");
    // The failed number left the edit page on the screen, which has no such link: the menu of
    // `.sellbuff` and its "Add Buffs" button bring the list of choices back.
    s.say(A, ".sellbuff");
    s.bypass(A, "sellbuffadd");
    s.log.take();
    s.bypass(A, "sellbuffaddskill 1040 200");
    let out = s.log.take();
    assert_eq!(texts(&out, A), ["Shield has been added!"]);
    assert_eq!(
        s.world.selling_buffs(ObjectId(A)),
        [SellBuffHolder {
            skill_id: 1040,
            price: 200
        }]
    );
    s.bypass(A, "sellbuffaddskill 1059 300");
    s.log.take();
    s.bypass(A, "sellbuffaddskill 1068 300");
    assert!(
        s.world.selling_buffs(ObjectId(A)).len() == 2,
        "1068 is not a known allowed skill, the list is full anyway"
    );

    // Edit: back to the menu with the button of the choice page, then change a price and remove one.
    s.bypass(A, "voice .sellbuff");
    s.log.take();
    s.bypass(A, "sellbuffedit");
    let page = board_page(&s.log.take(), A).unwrap();
    assert!(
        page.contains("sellbuffchangeprice 1040 $price_1040") && page.contains("sellbuffremove 1059"),
        "{page}"
    );
    s.bypass(A, "sellbuffchangeprice 1040 250");
    assert_eq!(
        texts(&s.log.take(), A),
        ["Price of Shield has been changed to 250!"]
    );
    s.bypass(A, "sellbuffremove 1059");
    assert_eq!(texts(&s.log.take(), A), ["Skill Empower has been removed!"]);
    assert_eq!(s.world.selling_buffs(ObjectId(A)).len(), 1);

    // Start: a title over 40 characters is refused.
    s.bypass(A, "voice .sellbuff");
    s.log.take();
    s.bypass(A, "sellbuffstart a very long title for a buff shop here");
    assert_eq!(
        texts(&s.log.take(), A),
        ["Your title cannot exceed 29 characters in length. Please try again."]
    );
    s.bypass(A, "sellbuffstart cheap buffs");
    let out = s.log.take();
    assert!(s.world.is_selling_buffs(ObjectId(A)));
    assert!(s.world.is_sitting(ObjectId(A)));
    assert_eq!(
        s.world.private_store_type(ObjectId(A)),
        PrivateStoreType::PackageSell
    );
    assert!(!packets(&out, A, ServerOpcode::ExPrivateStorePackageMsg.name()).is_empty());
    assert!(board_page(&out, A).unwrap().starts_with("<html>ALREADY"));
    // While selling, the edit commands do nothing.
    s.bypass(A, "sellbuffedit");
    assert!(board_page(&s.log.take(), A).is_none());

    // A customer clicks the seller and gets the buff menu instead of the sell list.
    s.input(
        B,
        ClientInput::Action {
            object_id: A,
            action_id: 0,
        },
    );
    s.log.take();
    s.input(
        B,
        ClientInput::Action {
            object_id: A,
            action_id: 0,
        },
    );
    let out = s.log.take();
    let page = board_page(&out, B).expect("the buff menu");
    assert!(page.starts_with("<html>BUY"), "{page}");
    assert!(page.contains(&format!("sellbuffbuyskill {A} 1040 0")), "{page}");
    assert!(packets(&out, B, "PRIVATE_STORE_LIST_SELL").is_empty());

    // Buying: the payment item moves, the seller pays the MP, the buff lands on the buyer.
    let mp_before = s.world.player(ObjectId(A)).unwrap().core.cur_mp;
    s.bypass(B, &format!("sellbuffbuyskill {A} 1040 0"));
    assert_eq!(s.adena_of(B), 750);
    assert_eq!(s.adena_of(A), 250);
    assert!(s.affected(B, 1040));
    assert!((mp_before - s.world.player(ObjectId(A)).unwrap().core.cur_mp - 10.0).abs() < 0.5);
    // The pet variant needs a living pet and charges nothing without one.
    s.world.stop_all_effects(ObjectId(B));
    s.bypass(B, &format!("sellbuffbuyskillPet {A} 1040 0"));
    assert!(
        texts(&s.log.take(), B).contains(&"Your pet must be summoned and alive to receive buffs.".to_owned())
    );
    assert_eq!(s.adena_of(B), 750);
    // Too poor.
    s.world.destroy_item_by_item_id(ObjectId(B), 57, 700, false);
    s.bypass(B, &format!("sellbuffbuyskill {A} 1040 0"));
    assert!(texts(&s.log.take(), B).contains(&"Not enough Adena!".to_owned()));

    // Stop: back on his feet, the menu again.
    s.bypass(A, "sellbuffstop");
    let out = s.log.take();
    assert!(!s.world.is_selling_buffs(ObjectId(A)));
    assert_eq!(s.world.private_store_type(ObjectId(A)), PrivateStoreType::None);
    assert!(board_page(&out, A).unwrap().starts_with("<html>MENU"));
}

/// M9-L01: a buff seller who logs out stays as an offline trader (type 9, `SELL_BUFFS`) under the
/// offline trade switch, its skills go to the table as lines with count 0, and a restore brings
/// the shop back as a package sale with `isSellingBuffs()`.
#[test]
fn a_buff_seller_is_stored_and_restored_like_an_offline_trader() {
    let offline = |database_writes| OfflineSettings {
        offline_trade_enable: true,
        database_writes,
        ..OfflineSettings::default()
    };
    let mut s = Scene::new();
    s.shop(true);
    s.world.set_offline_settings(offline(true));
    s.enter(pdata(A, 40, 0, &[1040]));
    s.say(A, ".sellbuff");
    s.bypass(A, "sellbuffadd");
    s.bypass(A, "sellbuffaddskill 1040 200");
    s.bypass(A, "voice .sellbuff");
    s.bypass(A, "sellbuffstart cheap buffs");
    assert!(s.world.is_selling_buffs(ObjectId(A)), "{:?}", texts(&s.log.take(), A));
    s.log.take();
    s.input(A, ClientInput::Logout);
    let out = s.log.take();
    assert!(s.world.is_offline_trader(ObjectId(A)));
    let status = out.iter().find_map(|o| match o {
        Out::Persist(PersistCommand::OfflineTrade(OfflineTradeWrite::Transaction {
            items,
            status: Some(status),
            ..
        })) => Some((items.clone(), status.clone())),
        _ => None,
    });
    let (items, status) = status.expect("the first call writes the shop");
    assert_eq!(status.store_type, 9);
    assert_eq!(status.title.as_deref(), Some("BUFF SELL: cheap buffs "));
    assert_eq!(
        items,
        vec![OfflineTradeLine {
            item: 1040,
            count: 0,
            price: 200
        }]
    );

    // A restore (a start with `OfflineTradeDatabaseWrites`) brings the seller back.
    let mut t = Scene::new();
    t.shop(true);
    t.world.set_offline_settings(offline(true));
    t.world.restore_offline_traders(vec![(
        pdata(A, 40, 0, &[1040]),
        OfflineTradeRow {
            char_id: ObjectId(A),
            status,
            items,
        },
    )]);
    assert!(t.world.is_offline_trader(ObjectId(A)));
    assert!(t.world.is_selling_buffs(ObjectId(A)));
    assert_eq!(t.world.private_store_type(ObjectId(A)), PrivateStoreType::PackageSell);
    assert_eq!(
        t.world.selling_buffs(ObjectId(A)),
        [SellBuffHolder {
            skill_id: 1040,
            price: 200
        }]
    );
}

/// Without `OfflineTradeDatabaseWrites` the buff seller stays in memory and writes no row (D-52).
#[test]
fn a_buff_seller_writes_no_row_without_the_switch() {
    let mut s = Scene::new();
    s.shop(true);
    s.world.set_offline_settings(OfflineSettings {
        offline_trade_enable: true,
        ..OfflineSettings::default()
    });
    s.enter(pdata(A, 40, 0, &[1040]));
    s.say(A, ".sellbuff");
    s.bypass(A, "sellbuffadd");
    s.bypass(A, "sellbuffaddskill 1040 200");
    s.bypass(A, "voice .sellbuff");
    s.bypass(A, "sellbuffstart cheap buffs");
    s.log.take();
    s.input(A, ClientInput::Logout);
    let out = s.log.take();
    assert!(s.world.is_offline_trader(ObjectId(A)));
    assert!(
        !out.iter()
            .any(|o| matches!(o, Out::Persist(PersistCommand::OfflineTrade(_))))
    );
}
