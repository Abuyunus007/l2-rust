#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! The custom features of M9-C02: the premium system, the auto potions, the auto play, the offline
//! play and the `.offline` command. Each has one test with its switch off (nothing changes) and
//! tests with it on (the Java behaviour named in each test).
//! AutoUseTaskManager}.java, dist/game/data/scripts/handlers/chat/commands/voiced/*.java.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{ConfirmDlg, NpcHtmlMessage, SystemMessage};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::custom::dualbox::OFFLINE_PLAY;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Vec<Vec<u8>>>>);

impl Log {
    fn take(&self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct Sink(Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.0.0.lock().unwrap().push(bytes);
    }
    fn left_world(&self, _reason: LeaveReason) {}
}

#[derive(Clone, Default)]
struct Commands(Arc<Mutex<Vec<PersistCommand>>>);

impl Commands {
    fn take(&self) -> Vec<PersistCommand> {
        std::mem::take(&mut self.0.lock().unwrap())
    }
}

struct RecordingOutbox(Commands);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

const TEMPLATE: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="1060" type="EtcItem" name="Healing Potion"><set name="is_stackable" val="true" /></item>
<item id="1542" type="EtcItem" name="Junk" />
<item id="1539" type="EtcItem" name="Greater Healing Potion"><set name="is_stackable" val="true" /></item>
<item id="1540" type="EtcItem" name="Buff Scroll"><set name="handler" val="ItemSkills" /><skills><skill id="9001" level="1" /></skills></item>
<item id="1541" type="EtcItem" name="Herb of Test"><set name="ex_immediate_effect" val="true" /><skills><skill id="9001" level="1" /></skills></item>
</list>"#;

/// 9001 is the skill of the Buff Scroll, 9002 a buff the player knows.
fn skills_xml() -> String {
    let mut xml = String::from("<list>");
    for (id, abnormal) in [(9001, "PD_UP"), (9002, "MD_UP")] {
        xml.push_str(&format!(
            "<skill id=\"{id}\" levels=\"1\" name=\"Test Buff {id}\"><operateType>A2</operateType><targetType>SELF</targetType>             <abnormalLevel>1</abnormalLevel><abnormalTime>600</abnormalTime><abnormalType>{abnormal}</abnormalType>             <castRange>400</castRange><effectRange>900</effectRange><isMagic>1</isMagic>             <effects><effect name=\"Buff\"><mul stat=\"pDef\" val=\"1.01\"/></effect></effects></skill>"
        ));
    }
    // Sweeper (42) and Sweeper Festival (444), as the datapack has them (no enchant routes).
    xml.push_str(
        r#"<skill id="42" levels="1" name="Sweeper"><operateType>A1</operateType><targetType>CORPSE_MOB</targetType>
        <castRange>20</castRange><effectRange>400</effectRange><hitTime>500</hitTime><isMagic>2</isMagic>
        <magicLevel>10</magicLevel><mpConsume>3</mpConsume><reuseDelay>500</reuseDelay>
        <conditions><player canSweep="true" /></conditions>
        <effects><effect name="Sweeper" /><effect name="ConsumeBody" /></effects></skill>
        <skill id="444" levels="1" name="Sweeper Festival"><operateType>A1</operateType><targetType>AURA_CORPSE_MOB</targetType>
        <affectRange>200</affectRange><hitTime>500</hitTime><isMagic>2</isMagic><magicLevel>10</magicLevel>
        <mpConsume>12</mpConsume><reuseDelay>3000</reuseDelay>
        <effects><effect name="Sweeper" /><effect name="ConsumeBody" /></effects></skill>"#,
    );
    xml.push_str("</list>");
    xml
}

const NPCS: &str = r#"<?xml version="1.0"?><list>
<npc id="20001" type="Monster" level="5" name="Rat" usingServerSideName="true">
  <race>BEAST</race>
  <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
    <vitals hp="300" hpRegen="2" mp="400.0" mpRegen="0.9" />
    <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
    <defence physical="44.44444" magical="29.5" />
    <speed><walk ground="20" /><run ground="50" /></speed>
  </stats>
  <collision><radius normal="10"/><height normal="20"/></collision>
  <dropLists><drop><item id="1541" min="1" max="1" chance="100"/></drop></dropLists>
</npc>
<npc id="20002" type="Monster" level="5" name="Fakey" usingServerSideName="true">
  <race>HUMAN</race>
  <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
    <vitals hp="300" hpRegen="2" mp="400.0" mpRegen="0.9" />
    <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
    <defence physical="44.44444" magical="29.5" />
    <speed><walk ground="20" /><run ground="50" /></speed>
  </stats>
  <collision><radius normal="10"/><height normal="20"/></collision>
  <ai aggroRange="300" isAggressive="true"/>
  <fakePlayer classId="1" fakePlayerTalkable="true"/>
</npc>
<npc id="20003" type="Monster" level="5" name="Silent Fakey" usingServerSideName="true">
  <race>HUMAN</race>
  <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
    <vitals hp="300" hpRegen="2" mp="400.0" mpRegen="0.9" />
    <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
    <defence physical="44.44444" magical="29.5" />
    <speed><walk ground="20" /><run ground="50" /></speed>
  </stats>
  <collision><radius normal="10"/><height normal="20"/></collision>
  <ai aggroRange="300" isAggressive="false"/>
  <fakePlayer classId="1" fakePlayerTalkable="false"/>
</npc>
<npc id="20005" type="Monster" level="5" name="Spoil Rat" usingServerSideName="true">
  <race>BEAST</race>
  <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
    <vitals hp="300" hpRegen="2" mp="400.0" mpRegen="0.9" />
    <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
    <defence physical="44.44444" magical="29.5" />
    <speed><walk ground="20" /><run ground="50" /></speed>
  </stats>
  <collision><radius normal="10"/><height normal="20"/></collision>
  <dropLists><spoil><item id="1060" min="1" max="1" chance="100"/></spoil></dropLists>
</npc>
<npc id="20004" type="Monster" level="5" name="Chaos Rat" usingServerSideName="true">
  <race>BEAST</race>
  <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
    <vitals hp="300" hpRegen="2" mp="400.0" mpRegen="0.9" />
    <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
    <defence physical="44.44444" magical="29.5" />
    <speed><walk ground="20" /><run ground="50" /></speed>
  </stats>
  <collision><radius normal="10"/><height normal="20"/></collision>
  <ai aggroRange="400" isChaos="true" isAggressive="true"/>
</npc>
</list>"#;

struct Data {
    items: Arc<ItemTemplates>,
    stats: StatData,
    /// `OptionData`: one augmentation option (M9-L01).
    option: Option<l2_data::AugmentationOption>,
}

impl StaticData for Data {
    fn augmentation_option(&self, id: i32) -> Option<&l2_data::AugmentationOption> {
        self.option.as_ref().filter(|o| o.id == id)
    }
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.items.get(item_id)?;
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
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        AccessLevelStatic {
            is_gm: level >= 100,
            ..AccessLevelStatic::USER
        }
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

fn data() -> Data {
    let items = Arc::new(ItemTemplates::from_xml(ITEMS).unwrap());
    let stats = StatData {
        items: Arc::clone(&items),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::from_xml("<list></list>").unwrap()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            &skills_xml(),
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
    };
    Data {
        items,
        stats,
        option: None,
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: ObjectId = ObjectId(0x1000_0001);
const ACCOUNT: &str = "acc268435457";

fn item(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        loc: "INVENTORY".to_owned(),
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

fn player_data(items: Vec<ItemData>, variables: &[(&str, &str)]) -> PlayerData {
    let mut profile = PlayerProfile::default();
    for (name, value) in variables {
        profile.variables.insert((*name).to_owned(), (*value).to_owned());
    }
    PlayerData {
        object_id: ME,
        account: ACCOUNT.to_owned(),
        name: "Hunter".to_owned(),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level: 1,
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
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items,
        skills: vec![SkillData {
            skill_id: 9002,
            skill_level: 1,
        }],
        shortcuts: vec![],
        profile,
    }
}

/// What a scene turns on before the player enters.
#[derive(Default)]
struct Switches {
    premium: Option<PremiumSettings>,
    auto_potions: Option<AutoPotionSettings>,
    auto_play: Option<AutoPlayRules>,
    offline_play: Option<OfflinePlaySettings>,
    offline: Option<OfflineSettings>,
    /// M9-L01: the dual-box limits and the client address of the player.
    dualbox: Option<DualboxSettings>,
    captcha: Option<CaptchaSettings>,
    fake: Option<FakePlayerSettings>,
    auto_loot_herbs: bool,
    bot_report: bool,
    option: Option<l2_data::AugmentationOption>,
    ip: Option<std::net::IpAddr>,
}

struct Scene {
    world: World,
    me: Log,
    out: Commands,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new(switches: Switches) -> Self {
        Self::with(switches, vec![], &[])
    }

    fn with(switches: Switches, items: Vec<ItemData>, variables: &[(&str, &str)]) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/html/mods/AutoPlay/Main.htm",
            "<html><body>%attack% %loot% %mode1% %percent% %status_button% \
             <button action=\"bypass voice .play attack\" value=\" \"></body></html>",
        );
        write(
            "data/html/mods/AutoPlay/Skills.htm",
            "<html><body>%skills%</body></html>",
        );
        write(
            "data/html/mods/AutoPlay/Items.htm",
            "<html><body>%items%</body></html>",
        );
        write(
            "data/html/mods/AutoPlay/Potion.htm",
            "<html><body>%items%</body></html>",
        );
        let out = Commands::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(RecordingOutbox(out.clone())));
        let mut data = data();
        data.option = switches.option.clone();
        world.set_static_data(Box::new(data));
        world.set_regen_settings(RegenSettings {
            mp_regen_multiplier: 0.0,
            ..RegenSettings::default()
        });
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(
                NpcTemplates::from_xml(
                    NPCS,
                    &NpcLoadOptions {
                        fake_players_enabled: true,
                        ..NpcLoadOptions::default()
                    },
                )
                .unwrap(),
            ),
            Arc::new(SpawnData::default()),
        );
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        let rows: String = (1..=85)
            .map(|level| format!(r#"<experience level="{level}" tolevel="{}" />"#, level * 100_000))
            .collect();
        world.set_exp_data(
            Arc::new(
                l2_data::ExperienceTable::from_xml(&format!(
                    r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
                ))
                .unwrap(),
            ),
            Arc::new(l2_data::ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(l2_data::KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(l2_data::SkillTrees::from_xml("<list></list>").unwrap()),
        );
        world.set_flood_settings(FloodSettings::java_defaults());
        if let Some(s) = switches.premium {
            world.set_premium_settings(s);
        }
        if let Some(s) = switches.auto_potions {
            world.set_auto_potion_settings(s);
        }
        if let Some(s) = switches.auto_play {
            world.set_auto_play_rules(s);
        }
        if let Some(s) = switches.offline_play {
            world.set_offline_play_settings(s);
        }
        if let Some(s) = switches.offline {
            world.set_offline_settings(s);
        }
        if switches.dualbox.is_some() || switches.captcha.is_some() || switches.fake.is_some() {
            let mut settings = WorldSettings::default();
            if let Some(d) = switches.dualbox {
                settings.custom_misc.dualbox = d;
            }
            if let Some(c) = switches.captcha {
                settings.custom_misc.captcha = c;
            }
            if let Some(f) = switches.fake {
                settings.fake_players = f;
            }
            settings.ground.auto_loot_herbs = switches.auto_loot_herbs;
            world.set_settings(settings);
        }
        if switches.bot_report {
            world.set_bot_report(BotReportSettings {
                enable: true,
                ..BotReportSettings::default()
            });
        }
        let me = Log::default();
        world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(ME.0).unwrap()),
            player: Box::new(player_data(items, variables)),
            sink: Box::new(Sink(me.clone())),
            ip: switches.ip,
        });
        let mut scene = Self {
            world,
            me,
            out,
            _root: root,
        };
        scene.advance(1000);
        scene.me.take();
        scene
    }

    fn advance(&mut self, ms: u64) {
        for _ in 0..(ms / 50) {
            self.world.tick();
        }
    }

    fn say(&mut self, text: &str) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME.0).unwrap()),
            input: ClientInput::Say {
                chat_type: 0,
                text: text.to_owned(),
                target: None,
            },
        });
    }

    fn dlg_answer(&mut self, answer: i32) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME.0).unwrap()),
            input: ClientInput::DlgAnswer {
                message_id: SystemMessageId::S13.id(),
                answer,
                requester_id: 0,
            },
        });
    }

    /// Runs the bypass handler of `Java BypassHandler.getHandler(command)`.
    fn bypass(&mut self, command: &str) -> Option<bool> {
        let handler = BypassHandlers::default().get(command)?;
        Some(handler(&mut self.world, ME, command, None))
    }

    fn variables_stored_on_leave(&mut self) -> std::collections::HashMap<String, String> {
        self.out.take();
        self.world.handle(WorldCommand::Disconnected {
            session: SessionId(u64::try_from(ME.0).unwrap()),
        });
        self.out
            .take()
            .into_iter()
            .find_map(|c| match c {
                PersistCommand::StorePlayer(data) => {
                    Some(data.profile.variables.clone().into_iter().collect())
                }
                _ => None,
            })
            .expect("the player was stored")
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

fn of<P: Decode>(out: &[Vec<u8>], opcode: ServerOpcode) -> Vec<P> {
    out.iter()
        .filter(|b| name_of(b) == opcode.name())
        .map(|b| decode::<P>(b))
        .collect()
}

fn html(out: &[Vec<u8>]) -> Vec<NpcHtmlMessage> {
    of(out, ServerOpcode::NpcHtmlMessage)
}

/// Whether any packet carries `needle` as a UTF-16 string (`sendMessage` is a `SystemMessage` with
/// a text, chat is a `Say2`).
fn said_contains(out: &[Vec<u8>], needle: &str) -> bool {
    let bytes: Vec<u8> = needle.encode_utf16().flat_map(u16::to_le_bytes).collect();
    out.iter().any(|b| b.windows(bytes.len()).any(|w| w == bytes))
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

fn premium_on() -> PremiumSettings {
    PremiumSettings {
        enabled: true,
        ..PremiumSettings::default()
    }
}

// ---------------------------------------------------------------------------------- premium

/// Off (the Java default): nothing asks for a row, nobody is premium and `.premium` is chat.
#[test]
fn premium_off_changes_nothing() {
    let mut s = Scene::new(Switches::default());
    assert!(!s.world.has_premium_status(ME));
    assert!(
        !s.out
            .take()
            .iter()
            .any(|c| matches!(c, PersistCommand::PremiumRequest { .. }))
    );
    s.say(".premium");
    assert!(html(&s.me.take()).is_empty());
}

/// `PremiumManager`'s `OnPlayerLogin` listener: the row is read, an end date in the future is
/// the status, and the expire task ends it.
#[test]
fn premium_on_a_row_in_the_future_is_the_status() {
    let mut s = Scene::new(Switches {
        premium: Some(premium_on()),
        ..Switches::default()
    });
    // The enter-world asked for the row of the account (lower case).
    let out = s.out.take();
    assert!(out.contains(&PersistCommand::PremiumRequest {
        player: ME,
        key: ACCOUNT.to_owned(),
    }));
    assert!(!s.world.has_premium_status(ME));
    s.world.handle(WorldCommand::PremiumLoaded {
        player: ME,
        key: ACCOUNT.to_owned(),
        result: Ok(Some(now_ms() + 60_000)),
    });
    assert!(s.world.has_premium_status(ME));
    // `.premium` shows the premium page, with the Java item id 5.
    s.me.take();
    s.say(".premium");
    let pages = html(&s.me.take());
    assert_eq!(pages.len(), 1);
    assert!(pages[0].html.contains("Status: <font color=\"LEVEL\">Premium"));
    assert!(pages[0].html.contains("Rate XP: <font color=\"LEVEL\">x2.0"));
    assert_eq!(pages[0].item_id, 5);
    // The expire task: the status ends 60 s later, the row stays.
    s.advance(61_000);
    assert!(!s.world.has_premium_status(ME));
    assert!(
        !s.out
            .take()
            .iter()
            .any(|c| matches!(c, PersistCommand::Premium(_)))
    );
}

#[test]
fn premium_on_an_expired_row_is_removed_only_with_cycle_writes() {
    let mut s = Scene::new(Switches {
        premium: Some(PremiumSettings { ..premium_on() }),
        ..Switches::default()
    });
    s.out.take();
    s.world.handle(WorldCommand::PremiumLoaded {
        player: ME,
        key: ACCOUNT.to_owned(),
        result: Ok(Some(1_000)),
    });
    assert!(!s.world.has_premium_status(ME));
    let deleted = s.out.take().into_iter().any(|c| {
        c == PersistCommand::Premium(PremiumWrite::Delete {
            account: ACCOUNT.to_owned(),
        })
    });
    assert!(deleted);
}

/// `addPremiumTime`: at least from now, the row is replaced, and the first online player of the
/// account becomes premium. `//premium_info` and `//premium_remove` read the cache.
#[test]
fn premium_add_and_remove_time() {
    let mut s = Scene::new(Switches {
        premium: Some(premium_on()),
        ..Switches::default()
    });
    s.out.take();
    s.world.add_premium_time(&ACCOUNT.to_uppercase(), 1);
    assert!(s.world.has_premium_status(ME));
    let out = s.out.take();
    let end = out
        .iter()
        .find_map(|c| match c {
            PersistCommand::Premium(PremiumWrite::Replace { account, end_date }) => {
                assert_eq!(account, ACCOUNT);
                Some(*end_date)
            }
            _ => None,
        })
        .expect("the row is replaced");
    assert!(end >= now_ms() + 86_400_000 - 5_000);
    assert_eq!(s.world.premium_expiration(ACCOUNT), end);
    // An admin removal always writes.
    s.world.remove_premium_status(ACCOUNT, true);
    assert!(!s.world.has_premium_status(ME));
    assert_eq!(s.world.premium_expiration(ACCOUNT), 0);
    assert!(
        s.out
            .take()
            .contains(&PersistCommand::Premium(PremiumWrite::Delete {
                account: ACCOUNT.to_owned(),
            }))
    );
}

/// The admin command `//premium_add1 <account>` of a GM, with the system on and off.
#[test]
fn premium_admin_command_follows_the_switch() {
    let mut off = Scene::new(Switches::default());
    off.world.add_premium_time(ACCOUNT, 1);
    off.me.take();
    off.world.premium_admin_command(ME, "admin_premium_add1 someone");
    // Off: "Premium system is disabled." and the menu page.
    assert!(said_contains(&off.me.take(), "Premium system is disabled."));

    let mut on = Scene::new(Switches {
        premium: Some(premium_on()),
        ..Switches::default()
    });
    on.out.take();
    on.me.take();
    on.world.premium_admin_command(ME, "admin_premium_add2 someone");
    assert!(said_contains(&on.me.take(), "will now have premium status until"));
    assert!(on.out.take().iter().any(|c| matches!(
        c,
        PersistCommand::Premium(PremiumWrite::Replace { account, .. }) if account == "someone"
    )));
    // No account name: Java's `StringIndexOutOfBoundsException` answer.
    on.me.take();
    on.world.premium_admin_command(ME, "admin_premium_add1");
    assert!(said_contains(&on.me.take(), "Please enter a valid account name."));
}

/// The premium drop rates: the amount of a death drop is doubled for a premium killer, the chance
/// of a herb is not (Java: "TODO: Premium herb chance? :)").
#[test]
fn premium_rates_apply_to_the_drop_of_a_premium_killer() {
    use l2_data::npc::DropType;
    use l2_world::rewards::drop::{DropKiller, DropVictim, calculate_drops};

    let items = ItemTemplates::from_xml(ITEMS).unwrap();
    let npcs = NpcTemplates::from_xml(
        r#"<list><npc id="1" type="Monster" level="10" name="M"><dropLists><drop>
            <item id="1060" min="10" max="10" chance="100"/></drop></dropLists></npc></list>"#,
        &NpcLoadOptions::default(),
    )
    .unwrap();
    let victim = DropVictim {
        level: 10,
        raid: false,
        champion: false,
        seeded: false,
    };
    let rates = PremiumRates {
        drop_amount: 2.0,
        drop_chance: 3.0,
        ..PremiumRates::default()
    };
    let mut rng = WorldRng::from_seed(1);
    let mut roll = |killer: &DropKiller| {
        calculate_drops(
            &DropSettings::default(),
            &items,
            npcs.get(1).unwrap(),
            DropType::Drop,
            &victim,
            killer,
            &mut rng,
        )
        .into_iter()
        .map(|h| (h.id, h.count))
        .collect::<Vec<_>>()
    };
    assert_eq!(roll(&DropKiller::plain(10)), [(1060, 10)]);
    let mut premium = DropKiller::plain(10);
    premium.premium = Some(Arc::new(rates));
    assert_eq!(roll(&premium), [(1060, 20)]);
}

// ---------------------------------------------------------------------------------- auto potions

fn potions_on() -> AutoPotionSettings {
    AutoPotionSettings {
        enabled: true,
        hp_item_ids: vec![1060, 1539],
        ..AutoPotionSettings::default()
    }
}

/// Off: `.apon` is plain chat and nobody gets potions.
#[test]
fn auto_potions_off_changes_nothing() {
    let mut s = Scene::new(Switches::default());
    s.say(".apon");
    assert!(!s.world.has_auto_potion(ME));
    assert!(!said_contains(&s.me.take(), "Auto potions"));
}

/// `AutoPotion.onCommand`: on and off with the words of Java, and the level check.
#[test]
fn auto_potions_commands() {
    let mut s = Scene::new(Switches {
        auto_potions: Some(potions_on()),
        ..Switches::default()
    });
    s.say(".apon");
    assert!(s.world.has_auto_potion(ME));
    assert!(said_contains(&s.me.take(), "Auto potions is enabled."));
    s.say(".potionoff");
    assert!(!s.world.has_auto_potion(ME));
    assert!(said_contains(&s.me.take(), "Auto potions is disabled."));

    let mut low = Scene::new(Switches {
        auto_potions: Some(AutoPotionSettings {
            min_level: 40,
            ..potions_on()
        }),
        ..Switches::default()
    });
    low.say(".apon");
    assert!(!low.world.has_auto_potion(ME));
    assert!(said_contains(
        &low.me.take(),
        "You need to be at least 40 to use auto potions."
    ));
}

/// `AutoPotionTaskManager.run()`: a player under the percentage with a potion of the list uses it
/// ("Restored HP."), one without any is told so. The list is walked in Java's hash order.
#[test]
fn auto_potions_pass() {
    let mut s = Scene::with(
        Switches {
            auto_potions: Some(potions_on()),
            ..Switches::default()
        },
        vec![item(0x1000_0100, 1060, 5)],
        &[],
    );
    s.world.add_auto_potion(ME);
    // 100/100 HP: not under 70 %, the potion is only counted. MP and CP have no potions.
    s.advance(1100);
    assert!(!said_contains(&s.me.take(), "Restored HP."));
    // Hurt: 30 of 100.
    s.world.player_mut(ME).unwrap().core.cur_hp = 30.0;
    s.advance(1100);
    assert!(said_contains(&s.me.take(), "Auto potion: Restored HP."));

    // Without a potion of any list: the reminder, every second.
    let mut empty = Scene::new(Switches {
        auto_potions: Some(potions_on()),
        ..Switches::default()
    });
    empty.world.add_auto_potion(ME);
    empty.advance(1100);
    assert!(said_contains(
        &empty.me.take(),
        "Auto potion: You are out of potions!"
    ));
}

// ---------------------------------------------------------------------------------- auto play

fn auto_play_on() -> AutoPlayRules {
    AutoPlayRules {
        enable_auto_play: true,
        ..AutoPlayRules::default()
    }
}

/// Off: `.play` is plain chat and `start_auto_play` never makes a player play.
#[test]
fn auto_play_off_changes_nothing() {
    let mut s = Scene::new(Switches::default());
    s.say(".play");
    assert!(html(&s.me.take()).is_empty());
    assert!(!s.world.is_auto_playing(ME));
    // A forced start is stopped by the first pass (`!ENABLE_AUTO_PLAY`).
    s.world.start_auto_play(ME);
    assert!(s.world.is_auto_playing(ME));
    s.advance(800);
    assert!(!s.world.is_auto_playing(ME));
}

/// `AutoPlay.onCommand`: the page, the toggles and start/stop through the `voice` bypass.
#[test]
fn auto_play_commands() {
    let mut s = Scene::new(Switches {
        auto_play: Some(auto_play_on()),
        ..Switches::default()
    });
    s.say(".play");
    let pages = html(&s.me.take());
    assert_eq!(pages.len(), 1);
    // Defaults of the holder: no auto attack, loot on, monster mode, 50 percent, "Start" button.
    assert!(pages[0].html.contains("L2UI.CheckBox "));
    assert!(pages[0].html.contains("L2UI.CheckBox_checked"));
    assert!(pages[0].html.contains("L2UI_CH3.radiobutton2"));
    assert!(pages[0].html.contains("bypass voice .play start"));

    s.say(".play attack");
    s.say(".play loot");
    s.say(".play mode2");
    s.say(".play percent 120");
    let page = html(&s.me.take()).pop().unwrap();
    assert!(
        page.html.contains("100"),
        "the percent is clamped to 100: {}",
        page.html
    );
    s.bypass("voice .play start").unwrap();
    assert!(s.world.is_auto_playing(ME));
    s.bypass("voice .play stop").unwrap();
    assert!(!s.world.is_auto_playing(ME));
}

/// `AutoPlayTaskManager`: the player picks the nearest monster and attacks it.
#[test]
fn auto_play_picks_a_monster() {
    let mut s = Scene::new(Switches {
        auto_play: Some(auto_play_on()),
        ..Switches::default()
    });
    let rat = s.world.spawn_npc_at(20001, X + 100, Y, Z, 0).unwrap();
    s.say(".play start");
    assert!(s.world.is_auto_playing(ME));
    s.advance(800);
    assert_eq!(s.world.target_of(ME), Some(rat));
}

/// `AutoUseTaskManager`: a supply item chosen on `.playitems` is used while hunting, a buff chosen
/// on `.playskills` is cast on the player. Both lists only offer what Java's filters let through.
#[test]
fn auto_use_items_and_buffs() {
    let mut s = Scene::with(
        Switches {
            auto_play: Some(auto_play_on()),
            ..Switches::default()
        },
        vec![item(0x1000_0100, 1540, 1), item(0x1000_0101, 1060, 3)],
        &[],
    );
    // The list holds the scroll (a continuous item skill), not the plain potion.
    s.say(".playitems");
    let page = html(&s.me.take()).pop().unwrap();
    assert!(page.html.contains("Buff Scroll"), "{}", page.html);
    assert!(!page.html.contains("Healing Potion"));
    // A toggle adds it, the second one takes it away again.
    s.say(".playitems 1 1540");
    let page = html(&s.me.take()).pop().unwrap();
    assert!(page.html.contains("mapbutton_zoomout2"), "{}", page.html);
    // The buff list shows the known skill; choosing it makes it an auto buff.
    s.say(".playskills 1 9002");
    let page = html(&s.me.take()).pop().unwrap();
    assert!(page.html.contains("Test Buff 9002"));
    assert!(page.html.contains("mapbutton_zoomout2"));

    s.say(".play start");
    s.advance(3000);
    assert!(s.world.is_affected_by_skill(ME, 9001), "the supply item was used");
    assert!(s.world.is_affected_by_skill(ME, 9002), "the auto buff was cast");
}

/// With `EnableAutoItem` and `EnableAutoSkill` off the lists do nothing.
#[test]
fn auto_play_item_and_skill_switches() {
    let mut s = Scene::with(
        Switches {
            auto_play: Some(AutoPlayRules {
                enable_auto_item: false,
                enable_auto_skill: false,
                ..auto_play_on()
            }),
            ..Switches::default()
        },
        vec![item(0x1000_0100, 1540, 1)],
        &[],
    );
    s.say(".playitems 1 1540");
    s.say(".playskills 1 9002");
    s.say(".play start");
    s.advance(3000);
    assert!(!s.world.is_affected_by_skill(ME, 9001));
    assert!(!s.world.is_affected_by_skill(ME, 9002));
}

/// The settings go to the variables at logout (`ON_PLAYER_LOGOUT`) and come back at login, and
/// `ResumeAutoPlay` starts the play again.
#[test]
fn auto_play_settings_survive_a_session() {
    let mut s = Scene::new(Switches {
        auto_play: Some(auto_play_on()),
        ..Switches::default()
    });
    s.say(".play attack");
    s.say(".play mode3");
    s.say(".play start");
    let variables = s.variables_stored_on_leave();
    assert_eq!(variables.get("AUTO_USE_ACTIONS").map(String::as_str), Some("2"));
    assert_eq!(
        variables.get("AUTO_USE_SETTINGS").map(String::as_str),
        Some("0,1,1,3,0,50,1")
    );

    let pairs: Vec<(&str, &str)> = variables.iter().map(|(k, v)| (k.as_str(), v.as_str())).collect();
    let resumed = Scene::with(
        Switches {
            auto_play: Some(AutoPlayRules {
                resume_auto_play: true,
                ..auto_play_on()
            }),
            ..Switches::default()
        },
        vec![],
        &pairs,
    );
    assert!(resumed.world.is_auto_playing(ME));
    let plain = Scene::with(
        Switches {
            auto_play: Some(auto_play_on()),
            ..Switches::default()
        },
        vec![],
        &pairs,
    );
    assert!(!plain.world.is_auto_playing(ME));
}

// ---------------------------------------------------------------------------------- offline play

fn offline_play_on() -> OfflinePlaySettings {
    OfflinePlaySettings {
        enable_offline_play_command: true,
        ..OfflinePlaySettings::default()
    }
}

/// Off: `.offlineplay` is plain chat.
#[test]
fn offline_play_off_changes_nothing() {
    let mut s = Scene::new(Switches::default());
    s.say(".offlineplay");
    let out = s.me.take();
    assert!(of::<ConfirmDlg>(&out, ServerOpcode::ConfirmDlg).is_empty());
    assert!(!s.world.is_offline_play(ME));
}

/// `OfflinePlay.onCommand`, `DlgAnswer` and `startOfflinePlay`: without auto play the command is
/// refused, with it the question is asked and a yes leaves the player in the world without a
/// client; the settings of the file decide the colour.
#[test]
fn offline_play_command_and_dialog() {
    let mut s = Scene::new(Switches {
        auto_play: Some(auto_play_on()),
        offline_play: Some(OfflinePlaySettings {
            offline_play_set_name_color: true,
            ..offline_play_on()
        }),
        ..Switches::default()
    });
    s.say(".offlineplay");
    assert!(said_contains(
        &s.me.take(),
        "You need to enable auto play before exiting."
    ));

    s.say(".play start");
    s.me.take();
    s.say(".offlineplay");
    let out = s.me.take();
    let dialogs = of::<ConfirmDlg>(&out, ServerOpcode::ConfirmDlg);
    assert_eq!(dialogs.len(), 1);
    assert_eq!(dialogs[0].message.id, SystemMessageId::S13);

    // A no does nothing.
    s.dlg_answer(0);
    assert!(!s.world.is_offline_play(ME));
    // The question is asked again, a yes starts the offline play.
    s.say(".offlineplay");
    s.dlg_answer(1);
    assert!(s.world.is_offline_play(ME));
    assert!(s.world.player(ME).is_some(), "the character stays in the world");
    // The auto play goes on without the client.
    let rat = s.world.spawn_npc_at(20001, X + 100, Y, Z, 0).unwrap();
    s.advance(800);
    assert_eq!(s.world.target_of(ME), Some(rat));
    // A forced answer without a question is nothing.
    s.dlg_answer(1);
}

/// `OfflinePlayPremium`: the command needs a premium status.
#[test]
fn offline_play_premium_is_required_with_the_key() {
    let mut s = Scene::new(Switches {
        auto_play: Some(auto_play_on()),
        offline_play: Some(OfflinePlaySettings {
            offline_play_premium: true,
            ..offline_play_on()
        }),
        ..Switches::default()
    });
    s.say(".play start");
    s.me.take();
    s.say(".offlineplay");
    let out = s.me.take();
    assert!(of::<ConfirmDlg>(&out, ServerOpcode::ConfirmDlg).is_empty());
    assert!(said_contains(
        &out,
        "This command is only available to premium players."
    ));
}

// ---------------------------------------------------------------------------------- .offline

/// `.offline` needs `EnableOfflineCommand` and a trade or craft key; the shipped file has them
/// on, the Java default has `OfflineTradeEnable` off.
#[test]
fn offline_command_follows_the_trade_keys() {
    // Defaults: no trade, no craft: not a command.
    let mut off = Scene::new(Switches::default());
    off.say(".offline");
    let out = off.me.take();
    // No handler took the text: no question, no store message.
    assert!(of::<ConfirmDlg>(&out, ServerOpcode::ConfirmDlg).is_empty());
    let messages: Vec<SystemMessage> = of(&out, ServerOpcode::SystemMessage);
    assert!(
        !messages
            .iter()
            .any(|m| m.id == SystemMessageId::PrivateStoreAlreadyClosed)
    );
    assert!(!off.world.is_offline_trader(ME));

    let mut on = Scene::new(Switches {
        offline: Some(OfflineSettings {
            offline_trade_enable: true,
            ..OfflineSettings::default()
        }),
        ..Switches::default()
    });
    on.me.take();
    on.say(".offline");
    // Not in store mode: "PRIVATE_STORE_ALREADY_CLOSED" and no question.
    let out = on.me.take();
    assert!(of::<ConfirmDlg>(&out, ServerOpcode::ConfirmDlg).is_empty());
    let messages: Vec<SystemMessage> = of(&out, ServerOpcode::SystemMessage);
    assert!(
        messages
            .iter()
            .any(|m| m.id == SystemMessageId::PrivateStoreAlreadyClosed)
    );
}

// ------------------------------------------------------------------ M9-L01: the offline play rows

fn offline_writes_on() -> OfflineSettings {
    OfflineSettings {
        ..OfflineSettings::default()
    }
}

fn offline_play_commands(out: &Commands) -> Vec<OfflinePlayWrite> {
    out.take()
        .into_iter()
        .filter_map(|c| match c {
            PersistCommand::OfflinePlay(w) => Some(w),
            _ => None,
        })
        .collect()
}

/// A scene whose player auto plays with one supply item and one buff and goes offline.
fn offline_player_scene(offline: Option<OfflineSettings>, op: OfflinePlaySettings) -> Scene {
    let mut s = Scene::with(
        Switches {
            auto_play: Some(auto_play_on()),
            offline_play: Some(op),
            offline,
            ..Switches::default()
        },
        vec![item(0x1000_0100, 1540, 1)],
        &[],
    );
    s.say(".playitems 1 1540");
    s.say(".playskills 1 9002");
    s.say(".play start");
    s.out.take();
    s.say(".offlineplay");
    s.dlg_answer(1);
    s
}

#[test]
fn offline_play_rows_follow_the_database_switch() {
    let mut on = offline_player_scene(Some(offline_writes_on()), offline_play_on());
    assert!(on.world.is_offline_play(ME));
    assert_eq!(
        offline_play_commands(&on.out),
        vec![OfflinePlayWrite::Store {
            char_id: ME,
            rows: vec![(1, 1540), (2, 9002)],
        }]
    );
    on.world.stop_auto_play(ME);
    assert_eq!(
        offline_play_commands(&on.out),
        vec![OfflinePlayWrite::Remove { char_id: ME }]
    );

    // `RestoreAutoPlayOffliners = False`.
    let keyless = offline_player_scene(
        Some(offline_writes_on()),
        OfflinePlaySettings {
            restore_auto_play_offliners: false,
            ..offline_play_on()
        },
    );
    assert!(keyless.world.is_offline_play(ME));
    assert!(offline_play_commands(&keyless.out).is_empty());
}

/// `restoreOfflinePlayers`: the character comes back without a client, plays and uses its
/// settings, a group is put together, and the group table is emptied.
#[test]
fn offline_players_are_restored_with_their_settings_and_groups() {
    let mut s = Scene::new(Switches {
        auto_play: Some(AutoPlayRules {
            enable_auto_assist: true,
            ..auto_play_on()
        }),
        offline_play: Some(offline_play_on()),
        offline: Some(offline_writes_on()),
        ..Switches::default()
    });
    s.out.take();
    let restored = |id: i32, rows: Vec<(i32, i32)>| {
        let mut data = player_data(vec![], &[]);
        data.object_id = ObjectId(id);
        data.name = format!("Bot{id}");
        OfflinePlayRestore { data, rows }
    };
    s.world.restore_offline_players(
        vec![restored(2, vec![(2, 9002)]), restored(3, vec![])],
        vec![OfflinePlayGroup {
            leader: ObjectId(2),
            members: vec![(ObjectId(3), 1), (ObjectId(99), 1)],
        }],
    );
    for id in [2, 3] {
        assert!(s.world.is_offline_play(ObjectId(id)));
        assert!(s.world.is_auto_playing(ObjectId(id)));
    }
    s.advance(3000);
    assert!(
        s.world.is_affected_by_skill(ObjectId(2), 9002),
        "the auto buff was cast"
    );
    assert!(!s.world.is_affected_by_skill(ObjectId(3), 9002));
    assert!(s.world.party_of(ObjectId(2)).is_some());
    assert_eq!(s.world.party_of(ObjectId(2)), s.world.party_of(ObjectId(3)));
    assert_eq!(offline_play_commands(&s.out), vec![OfflinePlayWrite::ClearGroups]);

    // `Shutdown`: `storeOfflineGroups` writes the member whose leader auto plays too.
    s.world.handle(WorldCommand::Shutdown {
        done: Box::new(|| {}),
    });
    assert_eq!(
        offline_play_commands(&s.out),
        vec![OfflinePlayWrite::StoreGroups(vec![(ObjectId(2), ObjectId(3), 1)])]
    );
}

/// `.playskills` leaves out the activation skills of the options of a worn augmented item. Java
/// looks the augmentation up with its packed id, so only an augmentation whose high option is 0
/// (the packed id is the option id) is found.
#[test]
fn playskills_leaves_out_augmentation_option_skills() {
    let option = l2_data::AugmentationOption {
        id: 77,
        activation_skills: vec![l2_data::augmentation::OptionSkillHolder {
            skill: l2_data::armor_set::SkillHolder { id: 9002, level: 1 },
            chance: 10.0,
            kind: l2_data::augmentation::OptionSkillType::Attack,
        }],
        ..Default::default()
    };
    let page = |augmentation: Option<i32>, option: Option<l2_data::AugmentationOption>| {
        let worn = ItemData {
            loc: "PAPERDOLL".to_owned(),
            augmentation,
            ..item(0x1000_0200, 1539, 1)
        };
        let mut s = Scene::with(
            Switches {
                auto_play: Some(auto_play_on()),
                option,
                ..Switches::default()
            },
            vec![worn],
            &[],
        );
        s.say(".playskills");
        html(&s.me.take()).pop().unwrap().html
    };
    assert!(page(None, Some(option.clone())).contains("Test Buff 9002"));
    assert!(page(Some(77), None).contains("Test Buff 9002"));
    assert!(!page(Some(77), Some(option.clone())).contains("Test Buff 9002"));
    assert!(page(Some(77 + (5 << 16)), Some(option)).contains("Test Buff 9002"));
}

// ------------------------------------------------------------------ M9-L01: the fake players

fn fakes_on() -> Switches {
    Switches {
        fake: Some(FakePlayerSettings {
            enabled: true,
            can_drop_items: true,
            can_pickup: true,
            ..FakePlayerSettings::default()
        }),
        ip: Some("10.0.0.5".parse().unwrap()),
        ..Switches::default()
    }
}

/// `BotReportTable.reportBot`: a talkable fake player can be reported, counted and named, and is
/// never punished; one that is not talkable cannot be reported.
#[test]
fn a_bot_report_on_a_fake_player_follows_java() {
    let mut s = Scene::new(Switches {
        bot_report: true,
        ..fakes_on()
    });
    let talkable = s.world.spawn_npc_at(20002, X + 50, Y, Z, 0).unwrap();
    let silent = s.world.spawn_npc_at(20003, X + 60, Y, Z, 0).unwrap();
    let rat = s.world.spawn_npc_at(20001, X + 70, Y, Z, 0).unwrap();
    s.me.take();
    assert!(!s.world.report_bot(ME, Some(silent)), "not talkable");
    assert!(
        !s.world.report_bot(ME, Some(rat)),
        "not a player, not a fake player"
    );
    assert!(s.me.take().is_empty());
    assert!(s.world.report_bot(ME, Some(talkable)));
    let out = s.me.take();
    let messages: Vec<SystemMessage> = of(&out, ServerOpcode::SystemMessage);
    assert_eq!(messages.len(), 2);
    assert_eq!(messages[0].id, SystemMessageId::C1WasReportedAsABot);
    assert_eq!(
        messages[1].id,
        SystemMessageId::YouHaveUsedAReportPointOnC1YouHaveS2PointsRemainingOnThisAccount
    );
    assert!(
        s.out
            .take()
            .iter()
            .any(|c| matches!(c, PersistCommand::BotReport(row) if row.bot_id == talkable.0))
    );
}

/// `NpcClick`: the second click on a fake player that stands close and can be seen sets the
/// player to follow it.
#[test]
fn clicking_a_fake_player_follows_it() {
    let mut s = Scene::new(fakes_on());
    let fake = s.world.spawn_npc_at(20002, X + 50, Y, Z, 0).unwrap();
    let click = |s: &mut Scene, object: ObjectId| {
        s.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(ME.0).unwrap()),
            input: ClientInput::Action {
                object_id: object.0,
                action_id: 0,
            },
        });
    };
    click(&mut s, fake);
    assert_eq!(s.world.follow_target_of(ME), None, "the first click selects");
    s.advance(1000);
    click(&mut s, fake);
    assert_eq!(s.world.follow_target_of(ME), Some(fake));
    assert_eq!(s.world.intention_of(ME), Intention::Follow);
    // Any other NPC is not followed (a monster is attacked instead).
    let mut t = Scene::new(fakes_on());
    let rat = t.world.spawn_npc_at(20001, X + 60, Y, Z, 0).unwrap();
    click(&mut t, rat);
    t.advance(1000);
    click(&mut t, rat);
    assert_eq!(t.world.intention_of(ME), Intention::Attack);
}

/// `Attackable.doItemDrop` and `AttackableAI.thinkActive`: the skills of a herb are cast by the
/// fake player, with the auto loot of herbs and when it picks the herb up from the ground.
#[test]
fn a_herb_dropped_for_a_fake_player_casts_its_skill() {
    // Auto loot of herbs: cast at once.
    let mut s = Scene::new(Switches {
        auto_loot_herbs: true,
        ..fakes_on()
    });
    let fake = s.world.spawn_npc_at(20002, X + 50, Y, Z, 0).unwrap();
    let rat = s.world.spawn_npc_at(20001, X + 60, Y, Z, 0).unwrap();
    s.world.do_item_drop(rat, Some(fake));
    s.advance(500);
    assert!(s.world.is_affected_by_skill(fake, 9001), "cast on the spot");

    // On the ground: the fake player walks to it, takes it and casts it.
    let mut s = Scene::new(fakes_on());
    let fake = s.world.spawn_npc_at(20002, X + 50, Y, Z, 0).unwrap();
    let rat = s.world.spawn_npc_at(20001, X + 55, Y, Z, 0).unwrap();
    s.world.do_item_drop(rat, Some(fake));
    assert!(!s.world.is_affected_by_skill(fake, 9001));
    s.advance(15_000);
    assert!(s.world.is_affected_by_skill(fake, 9001), "cast when picked up");
}

/// `AttackableAI.thinkActive`: a chaos monster hates the fake players in range
/// (`isAggressiveTowards` of an attackable target), an ordinary monster does not.
#[test]
fn monsters_aggro_fake_players_like_javas_scan() {
    let setup = |chaos_id: i32| {
        let mut s = Scene::new(fakes_on());
        // Far from the player (it would be aggressed otherwise), in the view of its region.
        let fake = s.world.spawn_npc_at(20002, X + 900, Y, Z, 0).unwrap();
        let mob = s.world.spawn_npc_at(chaos_id, X + 1000, Y, Z, 0).unwrap();
        s.advance(15_000);
        (s, fake, mob)
    };
    let (s, fake, mob) = setup(20004);
    assert_eq!(s.world.attack_target_of(mob), Some(fake));
    // A plain aggressive monster (the rat has the default AI) leaves it alone.
    let (s, fake, mob) = setup(20001);
    assert_ne!(s.world.attack_target_of(mob), Some(fake));
}

/// `CaptchaManager.updateCounter`: the kill of a player in auto play is not counted (with
/// `EnableAutoPlay`), the kill of any other player is.
#[test]
fn the_captcha_counter_skips_a_player_in_auto_play() {
    let kill_one = |auto_play: bool| {
        let mut s = Scene::new(Switches {
            auto_play: Some(auto_play_on()),
            captcha: Some(CaptchaSettings {
                enable: true,
                kill_counter: 0,
                kill_counter_randomization: 1,
                ..CaptchaSettings::default()
            }),
            ..Switches::default()
        });
        if auto_play {
            s.say(".play start");
        }
        let rat = s.world.spawn_npc_at(20001, X + 100, Y, Z, 0).unwrap();
        s.world.do_die(rat, Some(ME));
        s.world.captcha_is_in_report_mode(ME)
    };
    assert!(
        kill_one(false),
        "the first kill asks for the code (the counter is 0)"
    );
    assert!(!kill_one(true), "a kill in auto play is not counted");
}

/// `OfflinePlayDisconnectSameAccount`: the character list releases the offline player, and only
/// with the key.
#[test]
fn the_character_list_releases_an_offline_player_with_the_key() {
    let released = |disconnect: bool| {
        let mut s = offline_player_scene(
            None,
            OfflinePlaySettings {
                offline_play_disconnect_same_account: disconnect,
                ..offline_play_on()
            },
        );
        let done = Arc::new(Mutex::new(None));
        let slot = Arc::clone(&done);
        s.world.handle(WorldCommand::ReleaseOfflineSameAccount {
            object_id: ME,
            trader: false,
            play: disconnect,
            done: Box::new(move |released| *slot.lock().unwrap() = Some(released)),
        });
        let answer = done.lock().unwrap().unwrap();
        assert_eq!(answer, s.world.player(ME).is_none());
        answer
    };
    assert!(released(true));
    assert!(!released(false));
}

/// `DualboxCheckMaxOfflinePlayPerIP`: a character over the limit of its address stays online, with
/// Java's message; below the limit it leaves and is counted.
#[test]
fn offline_play_counts_the_connections_of_an_address() {
    let ip: std::net::IpAddr = "10.1.2.3".parse().unwrap();
    let mut s = Scene::new(Switches {
        auto_play: Some(auto_play_on()),
        offline_play: Some(offline_play_on()),
        dualbox: Some(DualboxSettings {
            max_offlineplay_per_ip: 1,
            ..DualboxSettings::default()
        }),
        ip: Some(ip),
        ..Switches::default()
    });
    // Another character of the address already plays offline.
    assert!(s.world.anti_feed_try_add_client(OFFLINE_PLAY, Some(ip), 1));
    s.say(".play start");
    s.me.take();
    s.say(".offlineplay");
    s.dlg_answer(1);
    assert!(!s.world.is_offline_play(ME));
    assert!(said_contains(
        &s.me.take(),
        "Only 1 offline players allowed per IP."
    ));
    // The place is free again: the character goes.
    s.world.anti_feed_remove_client(OFFLINE_PLAY, Some(ip));
    s.say(".offlineplay");
    s.dlg_answer(1);
    assert!(s.world.is_offline_play(ME));
}

/// `CharacterSelect`: a premium offline character that was just released is counted against
/// `DualboxCheckMaxOfflinePlayPremiumPerIP`.
#[test]
fn character_select_counts_a_released_premium_offline_player() {
    let ip: std::net::IpAddr = "10.1.2.3".parse().unwrap();
    let mut s = Scene::new(Switches {
        auto_play: Some(auto_play_on()),
        offline_play: Some(offline_play_on()),
        premium: Some(premium_on()),
        dualbox: Some(DualboxSettings {
            max_offlineplay_premium_per_ip: 1,
            ..DualboxSettings::default()
        }),
        ip: Some(ip),
        ..Switches::default()
    });
    s.world.handle(WorldCommand::PremiumLoaded {
        player: ME,
        key: ACCOUNT.to_owned(),
        result: Ok(Some(now_ms() + 600_000)),
    });
    s.say(".play start");
    s.say(".offlineplay");
    s.dlg_answer(1);
    assert!(s.world.is_offline_play(ME));
    s.world.handle(WorldCommand::ReleaseOfflineTrader {
        object_id: ME,
        done: Box::new(|_| {}),
    });
    // Another connection of the address holds the only place before the check.
    assert!(s.world.anti_feed_try_add_client(OFFLINE_PLAY, Some(ip), 1));
    let page = Arc::new(Mutex::new(None));
    let slot = Arc::clone(&page);
    s.world.handle(WorldCommand::OfflinePlaySelectCheck {
        object_id: ME,
        ip: Some(ip),
        access_level: 0,
        done: Box::new(move |p| *slot.lock().unwrap() = Some(p)),
    });
    let page = page.lock().unwrap().clone().unwrap();
    // No IPRestriction page in this scene: the fallback text names the page.
    assert!(page.unwrap().contains("IPRestriction.htm"));
    // A second check without a release has nothing to count.
    let again = Arc::new(Mutex::new(None));
    let slot = Arc::clone(&again);
    s.world.handle(WorldCommand::OfflinePlaySelectCheck {
        object_id: ME,
        ip: Some(ip),
        access_level: 0,
        done: Box::new(move |p| *slot.lock().unwrap() = Some(p)),
    });
    assert_eq!(again.lock().unwrap().clone(), Some(None));
}

// ------------------------------------------------------------------ auto play: sweeping

const SPOIL_RAT: i32 = 20005;
const SPOIL_ITEM: i32 = 1060;

/// A spoiler on `.play` (Sweeper, and Sweeper Festival with `festival`), without Spoil in its
/// auto skills.
fn spoiler(festival: bool) -> Scene {
    spoiler_with(festival, vec![])
}

fn spoiler_with(festival: bool, items: Vec<ItemData>) -> Scene {
    let mut s = Scene::with(
        Switches {
            auto_play: Some(auto_play_on()),
            ..Switches::default()
        },
        items,
        &[],
    );
    assert!(s.world.add_skill(ME, 42, 1));
    if festival {
        assert!(s.world.add_skill(ME, 444, 1));
    }
    s
}

/// A Spoil Rat at `(dx, dy)` from the player, spoiled by `spoiler` and killed by the player: a
/// corpse with its spoil.
fn spoiled_corpse(s: &mut Scene, dx: i32, dy: i32, spoiler: ObjectId) -> ObjectId {
    let rat = s.world.spawn_npc_at(SPOIL_RAT, X + dx, Y + dy, Z, 0).unwrap();
    s.world.test_set_spoiler(rat, Some(spoiler));
    s.world.add_damage_hate(rat, ME, 100, 1);
    assert!(s.world.do_die(rat, Some(ME)));
    rat
}

fn spoil_items(s: &Scene) -> i64 {
    s.world
        .player(ME)
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == SPOIL_ITEM)
        .map(|i| i.count)
        .sum()
}

fn skills_cast(out: &[Vec<u8>]) -> Vec<i32> {
    of::<l2_protocol_game::server::MagicSkillUse>(out, ServerOpcode::MagicSkillUse)
        .into_iter()
        .filter(|m| m.creature_id == ME.0)
        .map(|m| m.skill_id)
        .collect()
}

/// A spoiled corpse that is not the target is swept, without Spoil in the auto skills.
#[test]
fn auto_play_sweeps_a_spoiled_corpse() {
    let mut s = spoiler(false);
    let rat = spoiled_corpse(&mut s, 150, 0, ME);
    assert!(s.world.is_sweep_active(rat));
    assert_eq!(s.world.target_of(ME), None);
    s.say(".play start");
    s.advance(5_000);
    assert!(!s.world.is_sweep_active(rat), "the corpse was swept");
    assert_eq!(spoil_items(&s), 1);
    assert!(skills_cast(&s.me.take()).contains(&42));
}

/// Two corpses: Sweeper on each, no festival (it is for more than two).
#[test]
fn two_corpses_are_swept_one_by_one() {
    let mut s = spoiler(true);
    let a = spoiled_corpse(&mut s, 150, 0, ME);
    let b = spoiled_corpse(&mut s, 170, 60, ME);
    s.say(".play start");
    s.advance(6_000);
    assert!(!s.world.is_sweep_active(a) && !s.world.is_sweep_active(b));
    assert_eq!(spoil_items(&s), 2);
    let cast = skills_cast(&s.me.take());
    assert!(cast.contains(&42) && !cast.contains(&444), "{cast:?}");
}

/// Three corpses close together: one Sweeper Festival sweeps them all.
#[test]
fn three_corpses_bring_sweeper_festival() {
    let mut s = spoiler(true);
    let corpses = [
        spoiled_corpse(&mut s, 250, 0, ME),
        spoiled_corpse(&mut s, 300, 60, ME),
        spoiled_corpse(&mut s, 280, -60, ME),
    ];
    s.say(".play start");
    s.advance(6_000);
    assert!(corpses.iter().all(|c| !s.world.is_sweep_active(*c)));
    assert_eq!(spoil_items(&s), 3);
    let cast = skills_cast(&s.me.take());
    assert_eq!(cast.iter().filter(|id| **id == 444).count(), 1, "{cast:?}");
    assert!(!cast.contains(&42), "{cast:?}");
}

/// Three corpses without Sweeper Festival: Sweeper three times.
#[test]
fn without_the_festival_three_corpses_are_swept_one_by_one() {
    let mut s = spoiler(false);
    let corpses = [
        spoiled_corpse(&mut s, 150, 0, ME),
        spoiled_corpse(&mut s, 170, 60, ME),
        spoiled_corpse(&mut s, 160, -60, ME),
    ];
    s.say(".play start");
    s.advance(8_000);
    assert!(corpses.iter().all(|c| !s.world.is_sweep_active(*c)));
    assert_eq!(spoil_items(&s), 3);
    let cast = skills_cast(&s.me.take());
    assert_eq!(cast.iter().filter(|id| **id == 42).count(), 3, "{cast:?}");
}

/// A corpse somebody else spoiled is not the player's to sweep; a character without Sweeper
/// never tries.
#[test]
fn other_spoils_and_non_spoilers_are_left_alone() {
    let mut s = spoiler(true);
    let rat = spoiled_corpse(&mut s, 150, 0, ObjectId(0x1000_0099));
    s.say(".play start");
    s.advance(3_000);
    assert!(s.world.is_sweep_active(rat));
    assert!(skills_cast(&s.me.take()).is_empty());

    let mut s = Scene::new(Switches {
        auto_play: Some(auto_play_on()),
        ..Switches::default()
    });
    let rat = spoiled_corpse(&mut s, 150, 0, ME);
    s.say(".play start");
    s.advance(3_000);
    assert!(s.world.is_sweep_active(rat));
}

const PLAIN_RAT: i32 = 20001;

fn is_corpse_lying(s: &Scene, corpse: ObjectId) -> bool {
    s.world.npc(corpse).is_some_and(|n| !n.decayed)
}

/// The owner's complaint: a live monster near does not draw the spoiler away from its corpse.
#[test]
fn sweeping_comes_before_hunting_on() {
    let mut s = spoiler(false);
    let corpse = spoiled_corpse(&mut s, 150, 0, ME);
    let live = s.world.spawn_npc_at(PLAIN_RAT, X + 300, Y, Z, 0).unwrap();
    let full = s.world.npc(live).unwrap().core.cur_hp;
    s.say(".play start");
    s.advance(1_400);
    assert_eq!(s.world.target_of(ME), Some(corpse));
    assert_eq!(s.world.npc(live).unwrap().core.cur_hp, full);
    s.advance(4_000);
    assert_eq!(spoil_items(&s), 1);
    assert_eq!(s.world.target_of(ME), Some(live), "then it hunts on");
}

/// A sweep that fails (no MP) is tried again the next passes, until it works.
#[test]
fn a_failed_sweep_is_tried_again() {
    let mut s = spoiler(false);
    s.world.set_current_mp(ME, 1.0, false);
    let corpse = spoiled_corpse(&mut s, 150, 0, ME);
    s.say(".play start");
    s.advance(2_000);
    assert!(s.world.is_sweep_active(corpse));
    s.world.set_current_mp(ME, 50.0, false);
    s.advance(2_000);
    assert!(!s.world.is_sweep_active(corpse));
    assert_eq!(spoil_items(&s), 1);
}

/// A corpse that cannot be swept is left after its passes (about 10 s), and the hunt goes on.
#[test]
fn an_unsweepable_corpse_is_given_up() {
    let mut s = spoiler(false);
    s.world.set_current_mp(ME, 0.0, false);
    let corpse = spoiled_corpse(&mut s, 150, 0, ME);
    let live = s.world.spawn_npc_at(PLAIN_RAT, X + 300, Y, Z, 0).unwrap();
    s.say(".play start");
    s.advance(3_000);
    assert_eq!(s.world.target_of(ME), Some(corpse));
    s.advance(9_000);
    assert!(s.world.is_sweep_active(corpse), "never swept");
    assert_eq!(s.world.target_of(ME), Some(live));
}

/// While the festival is on reuse, more corpses are swept with Sweeper.
#[test]
fn sweeper_while_the_festival_is_on_reuse() {
    let mut s = spoiler(true);
    let first = [
        spoiled_corpse(&mut s, 250, 0, ME),
        spoiled_corpse(&mut s, 300, 60, ME),
        spoiled_corpse(&mut s, 280, -60, ME),
    ];
    s.say(".play start");
    s.advance(4_000);
    assert!(first.iter().all(|c| !s.world.is_sweep_active(*c)));
    let cast = skills_cast(&s.me.take());
    assert_eq!(cast.iter().filter(|id| **id == 444).count(), 1, "{cast:?}");
    // Three more right next to the player, within the festival's 3 s reuse.
    let (px, py, _) = s.world.position_of(ME).unwrap();
    let second = [
        spoiled_corpse(&mut s, px - X + 20, py - Y, ME),
        spoiled_corpse(&mut s, px - X - 20, py - Y + 20, ME),
        spoiled_corpse(&mut s, px - X, py - Y - 25, ME),
    ];
    s.advance(2_500);
    assert!(second.iter().all(|c| !s.world.is_sweep_active(*c)));
    assert_eq!(spoil_items(&s), 6);
    let cast = skills_cast(&s.me.take());
    assert_eq!(cast.iter().filter(|id| **id == 42).count(), 3, "{cast:?}");
    assert!(!cast.contains(&444), "{cast:?}");
}

/// A full inventory: Sweeper would consume the corpse without its loot, so the corpse is left
/// lying (for the player to make room and sweep it by hand).
#[test]
fn a_full_inventory_leaves_the_corpse_lying() {
    let junk: Vec<ItemData> = (0..150).map(|i| item(0x1000_1000 + i, 1542, 1)).collect();
    let mut s = spoiler_with(true, junk);
    let corpses = [
        spoiled_corpse(&mut s, 150, 0, ME),
        spoiled_corpse(&mut s, 170, 60, ME),
        spoiled_corpse(&mut s, 160, -60, ME),
    ];
    s.say(".play start");
    s.advance(3_000);
    assert!(
        corpses
            .iter()
            .all(|c| s.world.is_sweep_active(*c) && is_corpse_lying(&s, *c))
    );
    assert!(skills_cast(&s.me.take()).is_empty());
}

/// Another spoiler's corpse among them: no festival (it would consume that corpse), Sweeper
/// on each own one, and the other's spoil stays.
#[test]
fn no_festival_next_to_somebody_elses_spoil() {
    let mut s = spoiler(true);
    let own = [
        spoiled_corpse(&mut s, 150, 0, ME),
        spoiled_corpse(&mut s, 170, 60, ME),
        spoiled_corpse(&mut s, 160, -60, ME),
    ];
    let other = spoiled_corpse(&mut s, 190, 20, ObjectId(0x1000_0099));
    s.say(".play start");
    s.advance(8_000);
    assert!(own.iter().all(|c| !s.world.is_sweep_active(*c)));
    assert!(s.world.is_sweep_active(other) && is_corpse_lying(&s, other));
    let cast = skills_cast(&s.me.take());
    assert!(!cast.contains(&444), "{cast:?}");
}

/// While a monster attacks the player, it fights back instead of walking to a corpse; the corpse
/// is swept afterwards.
#[test]
fn fighting_back_comes_before_walking_to_a_corpse() {
    let mut s = spoiler(false);
    let corpse = spoiled_corpse(&mut s, 300, 0, ME);
    let attacker = s.world.spawn_npc_at(PLAIN_RAT, X + 40, Y, Z, 0).unwrap();
    s.world.add_damage_hate(attacker, ME, 1, 100);
    s.world.set_intention_attack(attacker, ME);
    s.say(".play start");
    s.advance(1_400);
    assert_eq!(s.world.target_of(ME), Some(attacker));
    assert!(s.world.is_sweep_active(corpse));
    // The attacker dies: the corpse is swept.
    s.world.do_die(attacker, Some(ME));
    s.advance(4_000);
    assert!(!s.world.is_sweep_active(corpse));
}

/// A corpse with less than `CorpseConsumeSkillAllowedTimeBeforeDecay` (2 s) left is too old for
/// Sweeper: it is not tried (no "too old" message each pass, no lost hunting time).
#[test]
fn a_corpse_too_old_is_not_tried() {
    let mut s = spoiler(false);
    let corpse = spoiled_corpse(&mut s, 20, 0, ME);
    while s.world.decay_remaining_ms(corpse) > 1_800 {
        s.advance(100);
    }
    s.me.take();
    s.say(".play start");
    s.advance(1_000);
    assert!(is_corpse_lying(&s, corpse) && s.world.is_sweep_active(corpse));
    let out = s.me.take();
    assert!(!skills_cast(&out).contains(&42));
    let messages: Vec<SystemMessage> = of(&out, ServerOpcode::SystemMessage);
    assert!(
        messages
            .iter()
            .all(|m| m.id != SystemMessageId::TheCorpseIsTooOldTheSkillCannotBeUsed)
    );
}

/// A dead spoiler on auto play does not sweep.
#[test]
fn a_dead_spoiler_does_not_sweep() {
    let mut s = spoiler(false);
    let corpse = spoiled_corpse(&mut s, 20, 0, ME);
    assert!(s.world.do_die(ME, None));
    s.world.start_auto_play(ME);
    s.me.take();
    s.advance(1_500);
    assert!(s.world.is_sweep_active(corpse));
    assert!(!skills_cast(&s.me.take()).contains(&42));
}
