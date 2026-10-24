#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The bot report button (M6-FU-09): `RequestActionUse` 65, the checks of `reportBot`, the report
//! points, the punishments by report count and the daily reset. The expected values come from the
//! Java code named in each test.
//! Java: data/BotReportTable.java, network/clientpackets/RequestActionUse.java (case 65).

use std::collections::BTreeMap;
use std::net::IpAddr;
use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, MapRegions, PlayerTemplates, SkillTemplates, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- a recording world -------------------------------------------------------------------------

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

// ---- the datapack -------------------------------------------------------------------------------

const TEMPLATE: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>1000</hp><mp>500</mp><cp>200</cp></level></lvlUpgainData></list>"#;

/// 6038 and 6039 of the datapack ("Report Status": the chat ban and the party ban), as shipped.
const SKILLS: &str = r##"<list>
<skill id="6038" levels="1" name="Report Status">
	<operateType>A2</operateType><targetType>ONE</targetType>
	<abnormalLevel>1</abnormalLevel><abnormalTime>600</abnormalTime><abnormalType>BOT_PENALTY</abnormalType>
	<isMagic>2</isMagic><magicLevel>-1</magicLevel><stayAfterDeath>True</stayAfterDeath>
	<effects><effect name="BlockChat" /></effects>
</skill>
<skill id="6039" levels="1" name="Report Status">
	<operateType>A2</operateType><targetType>ONE</targetType>
	<abnormalLevel>1</abnormalLevel><abnormalTime>3600</abnormalTime><abnormalType>BOT_PENALTY</abnormalType>
	<isMagic>2</isMagic><magicLevel>-1</magicLevel><stayAfterDeath>True</stayAfterDeath>
	<effects><effect name="BlockParty" /></effects>
</skill>
</list>"##;

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
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
}

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(TEMPLATE, 85).unwrap()),
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

// ---- the scene ----------------------------------------------------------------------------------

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const R1: ObjectId = ObjectId(0x2000_0001);
const R2: ObjectId = ObjectId(0x2000_0002);
const R3: ObjectId = ObjectId(0x2000_0003);
const BOT: ObjectId = ObjectId(0x2000_0010);
const BOT2: ObjectId = ObjectId(0x2000_0011);
const LIONS: i32 = 500;
const TIGERS: i32 = 501;
/// 2023-11-14, some time of the day.
const NOW: i64 = 1_700_000_000_000;
const MINUTE: i64 = 60_000;
const DELAY: i64 = 30 * MINUTE;

fn punishments() -> BTreeMap<i32, BotPunishment> {
    BTreeMap::from([
        // The 2nd report: a chat ban and its message (2473, "Chatting will be blocked").
        (
            2,
            BotPunishment {
                skill_id: 6038,
                skill_level: 1,
                system_message: 2473,
            },
        ),
        // From the 3rd report on (`neededReportCount` -3): a party ban, no message.
        (
            -3,
            BotPunishment {
                skill_id: 6039,
                skill_level: 1,
                system_message: -1,
            },
        ),
        // A skill the datapack does not have is dropped.
        (
            9,
            BotPunishment {
                skill_id: 1,
                skill_level: 1,
                system_message: -1,
            },
        ),
    ])
}

fn enabled() -> BotReportSettings {
    BotReportSettings {
        enable: true,
        reset_hour_minute: Some((0, 0)),
        report_delay_ms: DELAY,
        allow_reports_from_same_clan_members: false,
        punishments: punishments(),
    }
}

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

/// A zone of `zone_type` around the scene.
fn zone_files(root: &std::path::Path, zone_type: &str) {
    std::fs::create_dir_all(root.join("data/zones")).unwrap();
    std::fs::create_dir_all(root.join("data/mapregion")).unwrap();
    std::fs::write(
        root.join("data/zones/test.xml"),
        format!(
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\
             <zone id=\"1\" name=\"z1\" type=\"{zone_type}\" shape=\"Cuboid\" minZ=\"-5000\" maxZ=\"5000\">\
             <node X=\"{}\" Y=\"{}\"/><node X=\"{}\" Y=\"{}\"/></zone></list>\n",
            X - 500,
            Y - 500,
            X + 500,
            Y + 500
        ),
    )
    .unwrap();
    std::fs::write(
        root.join("data/mapregion/talking_island_town.xml"),
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
         <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
         <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n\
         </region>\n</list>\n",
    )
    .unwrap();
}

fn member(id: ObjectId) -> ClanMemberRecord {
    ClanMemberRecord {
        object_id: id.0,
        name: format!("P{}", id.0 - 0x2000_0000),
        level: 40,
        class_id: 0,
        title: String::new(),
        power_grade: 5,
        pledge_type: 0,
        apprentice: 0,
        sponsor: 0,
        female: false,
        race: 0,
    }
}

fn clan(id: i32, name: &str, members: &[ObjectId]) -> ClanRecord {
    ClanRecord {
        clan_id: id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level: 3,
            leader_id: members[0].0,
            ..ClanDataRecord::default()
        }),
        members: members.iter().map(|m| member(*m)).collect(),
        ..ClanRecord::default()
    }
}

impl Scene {
    fn new(settings: BotReportSettings) -> Self {
        Self::build(settings, None)
    }

    fn with_zone(settings: BotReportSettings, zone_type: &str) -> Self {
        Self::build(settings, Some(zone_type))
    }

    fn build(settings: BotReportSettings, zone_type: Option<&str>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(RecOutbox(log.clone())));
        world.set_static_data(Box::new(Data(stat_data())));
        if let Some(zone_type) = zone_type {
            zone_files(root.path(), zone_type);
            world.set_zone_data(
                Arc::new(Zones::load(root.path())),
                Arc::new(MapRegions::load(root.path())),
            );
        }
        // Lions (500): R1 and R3; Tigers (501): BOT2, at war with the Lions.
        world.load_clans(
            vec![clan(LIONS, "Lions", &[R1, R3]), clan(TIGERS, "Tigers", &[BOT2])],
            vec![(TIGERS.to_string(), LIONS.to_string())],
        );
        world.set_bot_report(settings);
        Self {
            world,
            log,
            _root: root,
        }
    }

    /// A level 40 character with exp, from `10.0.0.<ip>`, in the clan `clan_id` (0 for none).
    fn enter(&mut self, id: ObjectId, ip: u8, exp: i64, clan_id: i32) {
        let name = format!("P{}", id.0 - 0x2000_0000);
        let mut data = PlayerData {
            object_id: id,
            account: format!("acc{}", id.0),
            name,
            title: String::new(),
            class_id: 0,
            base_class: 0,
            race: 0,
            sex: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            level: 40,
            exp,
            sp: 0,
            karma: 0,
            pvp_kills: 0,
            pk_kills: 0,
            access_level: 0,
            x: X,
            y: Y,
            z: Z,
            heading: 0,
            cur_hp: 1000.0,
            max_hp: 1000.0,
            cur_mp: 500.0,
            max_mp: 500.0,
            cur_cp: 200.0,
            max_cp: 200.0,
            online_time: 0,
            items: vec![],
            skills: vec![],
            shortcuts: vec![],
            profile: PlayerProfile::default(),
        };
        if clan_id != 0 {
            data.profile.clan = Some(PlayerClanState {
                clan_id,
                power_grade: 5,
                ..PlayerClanState::default()
            });
        }
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id.0).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id.0, self.log.clone())),
            ip: Some(IpAddr::from([10, 0, 0, ip])),
        });
        // The zones are entered a tick later.
        self.world.tick();
    }

    /// The button: the target is the player's, then `RequestActionUse` 65.
    fn press(&mut self, reporter: ObjectId, target: Option<ObjectId>) {
        self.world.set_target(reporter, target);
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(reporter.0).unwrap()),
            input: ClientInput::ActionUse {
                action_id: 65,
                ctrl_pressed: false,
                shift_pressed: false,
            },
        });
    }

    /// `reportBot` at `now`; the system messages `reporter` got (the log is emptied).
    fn report(&mut self, reporter: ObjectId, bot: ObjectId, now: i64) -> (bool, Vec<SystemMessage>) {
        self.world.set_target(reporter, Some(bot));
        self.log.take();
        let result = self.world.report_bot_at(reporter, Some(bot), now);
        (result, messages(&self.log.take(), reporter))
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode_checked<P: Decode>(raw: &[u8]) -> Option<P> {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).ok()
}

fn messages(out: &[Out], to: ObjectId) -> Vec<SystemMessage> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, bytes) if *id == to.0 && name_of(bytes) == "SYSTEM_MESSAGE" => {
                // A message sent without the parameters its id declares (C1_PURCHASED_S3_S2_S, a
                // Java quirk) cannot be decoded; its id is all there is to see.
                Some(
                    decode_checked::<SystemMessage>(bytes).unwrap_or_else(|| SystemMessage {
                        id: SystemMessageId::from_id(i32::from_le_bytes(bytes[1..5].try_into().unwrap()))
                            .unwrap(),
                        params: Vec::new(),
                    }),
                )
            }
            _ => None,
        })
        .collect()
}

fn ids(messages: &[SystemMessage]) -> Vec<SystemMessageId> {
    messages.iter().map(|m| m.id).collect()
}

fn rows(out: &[Out]) -> Vec<BotReportRow> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::BotReport(row)) => Some(*row),
            _ => None,
        })
        .collect()
}

fn text(name: &str) -> SystemMessageParam {
    SystemMessageParam::Text(name.to_owned())
}

/// The reporter's second message: "$c1 was reported" then "you have S2 points remaining".
fn points_left(messages: &[SystemMessage]) -> i32 {
    match messages[1].params.as_slice() {
        [_, SystemMessageParam::IntNumber(points)] => *points,
        other => panic!("{other:?}"),
    }
}

/// `addPunishment`: a punishment whose skill does not exist is dropped.
#[test]
fn a_punishment_without_its_skill_is_dropped() {
    let s = Scene::new(enabled());
    let kept: Vec<i32> = s
        .world
        .bot_report_settings()
        .punishments
        .keys()
        .copied()
        .collect();
    assert_eq!(kept, [-3, 2]);
}

// ---- RequestActionUse 65 ---------------------------------------------------------------------------

/// `RequestActionUse` case 65 without `EnableBotReportButton`: `sendMessage("This feature is
/// disabled.")` and nothing else.
#[test]
fn the_button_is_disabled_by_default() {
    let mut s = Scene::new(BotReportSettings::default());
    s.enter(R1, 1, 1000, 0);
    s.enter(BOT, 2, 1000, 0);
    s.log.take();
    s.press(R1, Some(BOT));
    let out = s.log.take();
    let sent = messages(&out, R1);
    assert_eq!(sent.len(), 1);
    assert_eq!(sent[0], SystemMessage::text("This feature is disabled."));
    assert!(rows(&out).is_empty());
}

/// A report through the packet: "$s1 was reported as a bot." and "You have used a report point on
/// $s1. You have $s2 points remaining on this account." (7 points, one used), and the row of
/// `bot_reported_char_data` (bot, reporter, time).
#[test]
fn a_report_costs_a_point_and_writes_the_row() {
    let mut s = Scene::new(enabled());
    s.enter(R1, 1, 1000, 0);
    s.enter(BOT, 2, 1000, 0);
    s.log.take();
    s.press(R1, Some(BOT));
    let out = s.log.take();
    let sent = messages(&out, R1);
    assert_eq!(
        ids(&sent),
        [
            SystemMessageId::C1WasReportedAsABot,
            SystemMessageId::YouHaveUsedAReportPointOnC1YouHaveS2PointsRemainingOnThisAccount
        ]
    );
    assert_eq!(sent[0].params, [text("P16")]);
    assert_eq!(sent[1].params, [text("P16"), SystemMessageParam::IntNumber(6)]);
    let written = rows(&out);
    assert_eq!(written.len(), 1);
    assert_eq!((written[0].bot_id, written[0].reporter_id), (BOT.0, R1.0));
    assert!(written[0].report_date > 1_700_000_000_000);
    // No punishment for one report.
    assert!(s.world.effect_icons(BOT).is_empty());
    // Nothing for the reported character itself.
    assert!(messages(&out, BOT).is_empty());
}

// ---- the checks of reportBot -----------------------------------------------------------------------

#[test]
fn nothing_happens_without_a_player_target() {
    let mut s = Scene::new(enabled());
    s.enter(R1, 1, 1000, 0);
    s.log.take();
    // `getTarget() == null`, and the reporter itself.
    assert!(!s.world.report_bot_at(R1, None, NOW));
    assert!(!s.world.report_bot_at(R1, Some(R1), NOW));
    // An object that is no player.
    assert!(!s.world.report_bot_at(R1, Some(ObjectId(0x2FFF_FFFF)), NOW));
    assert!(s.log.take().is_empty());
}

/// `bot.isInsideZone(PEACE) || bot.isInsideZone(PVP)`.
#[test]
fn nobody_is_reported_in_a_peace_zone_or_a_battlefield() {
    for zone_type in ["PeaceZone", "ArenaZone"] {
        let mut s = Scene::with_zone(enabled(), zone_type);
        s.enter(R1, 1, 1000, 0);
        s.enter(BOT, 2, 1000, 0);
        let (reported, sent) = s.report(R1, BOT, NOW);
        assert!(!reported, "{zone_type}");
        assert_eq!(
            ids(&sent).last(),
            Some(&SystemMessageId::YouCannotReportACharacterWhoIsInAPeaceZoneOrABattlefield),
            "{zone_type}"
        );
        assert!(rows(&s.log.take()).is_empty());
    }
}

#[test]
fn clans_at_war_cannot_report_each_other() {
    let mut s = Scene::new(enabled());
    s.enter(R1, 1, 1000, LIONS);
    s.enter(BOT2, 2, 1000, TIGERS);
    // Tigers are at war with the Lions; `bot.getClan().isAtWarWith(reporter.getClan())`.
    let (reported, sent) = s.report(R1, BOT2, NOW);
    assert!(!reported);
    assert_eq!(
        ids(&sent),
        [SystemMessageId::YouCannotReportWhenAClanWarHasBeenDeclared]
    );
    // The other way round the Tigers' side is not "at war with" the Lions' attacker list.
    let (reported, _) = s.report(BOT2, R1, NOW);
    assert!(reported);
}

/// NOTE(java-quirk): the starting exp is never set, so only exp 0 counts as "not acquired any exp".
#[test]
fn a_character_without_exp_is_not_reported() {
    let mut s = Scene::new(enabled());
    s.enter(R1, 1, 1000, 0);
    s.enter(BOT, 2, 0, 0);
    let (reported, sent) = s.report(R1, BOT, NOW);
    assert!(!reported);
    assert_eq!(
        ids(&sent),
        [SystemMessageId::YouCannotReportACharacterWhoHasNotAcquiredAnyExpAfterConnecting]
    );
}

#[test]
fn a_reported_character_cannot_report() {
    let mut s = Scene::new(enabled());
    s.enter(R1, 1, 1000, 0);
    s.enter(R2, 2, 1000, 0);
    s.enter(BOT, 3, 1000, 0);
    // R2 reports R1 ...
    assert!(s.report(R2, R1, NOW).0);
    // ... so R1 is a key of `_reports` and may not report anybody.
    let (reported, sent) = s.report(R1, BOT, NOW);
    assert!(!reported);
    assert_eq!(
        ids(&sent),
        [SystemMessageId::YouHaveBeenReportedAsAnIllegalProgramUserAndCannotReportOtherUsers]
    );
}

/// `timeHasPassed(_ipRegistry, ip)`: an address reports once per `BotReportDelay`; the same
/// reporter cannot report the same character twice ("C1_PURCHASED_S3_S2_S", sic).
#[test]
fn one_report_per_address_and_delay_and_one_per_bot() {
    let mut s = Scene::new(enabled());
    s.enter(R1, 1, 1000, 0);
    s.enter(R2, 1, 1000, 0);
    s.enter(BOT, 3, 1000, 0);
    s.enter(BOT2, 4, 1000, 0);
    assert!(s.report(R1, BOT, NOW).0);
    // The same address within the delay.
    let (reported, sent) = s.report(R2, BOT2, NOW + DELAY);
    assert!(!reported);
    assert_eq!(
        ids(&sent),
        [SystemMessageId::ThisCharacterCannotMakeAReportTheTargetHasAlreadyBeenReportedByEitherYourClanOrAllianceOrHasAlreadyBeenReportedFromYourCurrentIp]
    );
    // `(now - time) > delay`: one millisecond later it passes.
    assert!(s.report(R2, BOT2, NOW + DELAY + 1).0);
    // R1 on BOT again: its own address is the first check, later it is the pair.
    let (reported, sent) = s.report(R1, BOT, NOW + 10 * DELAY);
    assert!(!reported);
    assert_eq!(ids(&sent), [SystemMessageId::C1PurchasedS3S2S]);
}

/// `!BOTREPORT_ALLOW_REPORTS_FROM_SAME_CLAN_MEMBERS && rcd.reportedBySameClan(reporter.getClan())`.
#[test]
fn clan_members_do_not_report_the_same_bot_unless_allowed() {
    let mut s = Scene::new(enabled());
    s.enter(R1, 1, 1000, LIONS);
    s.enter(R3, 2, 1000, LIONS);
    s.enter(BOT, 3, 1000, 0);
    assert!(s.report(R1, BOT, NOW).0);
    let (reported, sent) = s.report(R3, BOT, NOW + 2 * DELAY);
    assert!(!reported);
    assert_eq!(
        ids(&sent),
        [SystemMessageId::ThisCharacterCannotMakeAReportTheTargetHasAlreadyBeenReportedByEitherYourClanOrAllianceOrHasAlreadyBeenReportedFromYourCurrentIp]
    );

    let mut allowed = enabled();
    allowed.allow_reports_from_same_clan_members = true;
    let mut s = Scene::new(allowed);
    s.enter(R1, 1, 1000, LIONS);
    s.enter(R3, 2, 1000, LIONS);
    s.enter(BOT, 3, 1000, 0);
    assert!(s.report(R1, BOT, NOW).0);
    assert!(s.report(R3, BOT, NOW + 2 * DELAY).0);
}

/// 7 points a day, one per report, and `BotReportDelay` between two reports of one character.
#[test]
fn the_points_and_the_delay_of_a_reporter() {
    let mut s = Scene::new(enabled());
    s.enter(R1, 1, 1000, 0);
    // Eight characters to report, each from its own address.
    let bots: Vec<ObjectId> = (0..8).map(|i| ObjectId(0x2000_0100 + i)).collect();
    for (i, bot) in bots.iter().enumerate() {
        s.enter(*bot, 10 + i as u8, 1000, 0);
    }
    let (reported, sent) = s.report(R1, bots[0], NOW);
    assert!(reported);
    assert_eq!(points_left(&sent), 6);

    // The same address is held back by `_ipRegistry` first, so the character comes back from
    // another one. 10 minutes later: "$s1 minutes since the last report, $s2 points left" (sic,
    // the text id is FIVE_YEARS_HAVE_PASSED_SINCE_THIS_CHARACTER_S_CREATION).
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(R1.0).unwrap()),
    });
    s.enter(R1, 99, 1000, 0);
    let (reported, sent) = s.report(R1, bots[1], NOW + 10 * MINUTE);
    assert!(!reported);
    assert_eq!(
        ids(&sent),
        [SystemMessageId::FiveYearsHavePassedSinceThisCharacterSCreation]
    );
    assert_eq!(
        sent[0].params,
        [
            SystemMessageParam::IntNumber(10),
            SystemMessageParam::IntNumber(6)
        ]
    );

    // `reuse < delay` is the check: the delay is enough.
    for (n, bot) in bots.iter().enumerate().skip(1).take(6) {
        let at = NOW + (DELAY + 1) * n as i64;
        let (reported, sent) = s.report(R1, *bot, at);
        assert!(reported, "report {n}");
        assert_eq!(points_left(&sent), 6 - n as i32);
    }
    // 7 points are used.
    let (reported, sent) = s.report(R1, bots[7], NOW + (DELAY + 1) * 7);
    assert!(!reported);
    assert_eq!(
        ids(&sent),
        [SystemMessageId::YouHaveUsedAllAvailablePointsPointsAreResetEverydayAtNoon]
    );
}

// ---- the punishments ------------------------------------------------------------------------------

/// `handleReport`: the punishment of exactly this count (the 2nd report: the chat ban, with its
/// message), then every range punishment (negative key) whose count is reached (from the 3rd on:
/// the party ban).
#[test]
fn the_punishments_come_with_the_report_count() {
    let mut s = Scene::new(enabled());
    s.enter(BOT, 1, 1000, 0);
    for (i, reporter) in [R1, R2, R3].into_iter().enumerate() {
        s.enter(reporter, 10 + i as u8, 1000, 0);
    }
    assert!(s.report(R1, BOT, NOW).0);
    assert!(s.world.effect_icons(BOT).is_empty());

    // The 2nd report: the chat ban (6038) and its message.
    s.log.take();
    assert!(s.world.report_bot_at(R2, Some(BOT), NOW));
    let to_bot = messages(&s.log.take(), BOT);
    // (The effect itself tells "$s1's effect can be felt" first.)
    assert!(ids(&to_bot).contains(
        &SystemMessageId::YouHaveBeenReportedAsAnIllegalProgramUserSoYourChattingWillBeBlockedFor10Minutes
    ));
    let skills: Vec<i32> = s.world.effect_icons(BOT).iter().map(|i| i.skill_id).collect();
    assert_eq!(skills, [6038]);

    // The 3rd: the range punishment of -3 (the party ban), no message.
    s.log.take();
    assert!(s.world.report_bot_at(R3, Some(BOT), NOW));
    let to_bot = messages(&s.log.take(), BOT);
    assert!(
        to_bot.iter().all(|m| !(2473..=2492).contains(&m.id.id())),
        "{to_bot:?}"
    );
    let mut skills: Vec<i32> = s.world.effect_icons(BOT).iter().map(|i| i.skill_id).collect();
    skills.sort_unstable();
    assert!(skills.contains(&6039), "{skills:?}");
}

// ---- the table ------------------------------------------------------------------------------------

/// `loadReportedCharData`: the rows are the reports; a reporter with a row newer than the last
/// reset has one point less (a new entry starts at 6 and counts that row), and a character that
/// is reported cannot report.
#[test]
fn the_rows_of_the_table_come_back() {
    let mut s = Scene::new(enabled());
    s.enter(R1, 1, 1000, 0);
    s.enter(R2, 2, 1000, 0);
    s.enter(R3, 3, 1000, 0);
    s.enter(BOT, 4, 1000, 0);
    s.enter(BOT2, 5, 1000, 0);
    let bot3 = ObjectId(0x2000_0012);
    s.enter(bot3, 6, 1000, 0);
    // Reset at 00:00 UTC, the clock is in the evening: today's reset is at `day_start` (and the
    // seconds of the clock).
    let day_start = NOW - NOW.rem_euclid(86_400_000);
    let hour = 3_600_000;
    let row = |bot: ObjectId, reporter: ObjectId, date: i64| BotReportRow {
        bot_id: bot.0,
        reporter_id: reporter.0,
        report_date: date,
    };
    s.world.load_bot_reports_at(
        vec![
            // R2 reported twice today: its entry starts at 6 and loses one more.
            row(BOT, R2, day_start + hour),
            row(BOT2, R2, day_start + 2 * hour),
            // R3 reported before the reset: no entry, all its points.
            row(BOT, R3, day_start - 1000),
            // R1 is itself reported.
            row(R1, R3, day_start - 5000),
        ],
        NOW,
        |_| 0,
    );
    // The reported R1 cannot report.
    let (reported, sent) = s.report(R1, bot3, NOW + 5 * DELAY);
    assert!(!reported);
    assert_eq!(
        ids(&sent),
        [SystemMessageId::YouHaveBeenReportedAsAnIllegalProgramUserAndCannotReportOtherUsers]
    );
    // R2 is on BOT already (the row).
    let (reported, sent) = s.report(R2, BOT, NOW + 5 * DELAY);
    assert!(!reported);
    assert_eq!(ids(&sent), [SystemMessageId::C1PurchasedS3S2S]);
    // R2 had 5 points, R3 has 7.
    let (reported, sent) = s.report(R2, bot3, NOW + 5 * DELAY);
    assert!(reported);
    assert_eq!(points_left(&sent), 4);
    let (reported, sent) = s.report(R3, bot3, NOW + 5 * DELAY);
    assert!(reported);
    assert_eq!(points_left(&sent), 6);
}

/// `ResetPointTask`: every reporter has 7 points again, and the task is scheduled again.
#[test]
fn the_points_come_back_at_the_reset_hour() {
    let mut s = Scene::new(enabled());
    s.enter(R1, 1, 1000, 0);
    s.enter(BOT, 2, 1000, 0);
    s.enter(BOT2, 3, 1000, 0);
    assert!(!s.world.has_bot_report_reset_task());
    // 10 minutes to the reset hour (12:10 UTC, the task keeps the seconds of the clock).
    let day_start = NOW - NOW.rem_euclid(86_400_000);
    let hour = (NOW - day_start) / 3_600_000;
    let minute = (NOW - day_start) % 3_600_000 / MINUTE + 10;
    s.world.set_bot_report(BotReportSettings {
        reset_hour_minute: Some((hour, minute)),
        ..enabled()
    });
    s.world.schedule_bot_report_reset(NOW, |_| 0);
    assert!(s.world.has_bot_report_reset_task());

    let (reported, sent) = s.report(R1, BOT, NOW);
    assert!(reported);
    assert_eq!(points_left(&sent), 6);
    // Before the reset the points stay.
    for _ in 0..(9 * MINUTE / 50) {
        s.world.tick();
    }
    let (reported, sent) = s.report(R1, BOT2, NOW + DELAY + 1);
    assert!(reported);
    assert_eq!(points_left(&sent), 5);
    // Over the reset: 7 again (and the next run is scheduled).
    for _ in 0..(2 * MINUTE / 50) {
        s.world.tick();
    }
    assert!(s.world.has_bot_report_reset_task());
    s.enter(R2, 4, 1000, 0);
    let (reported, sent) = s.report(R1, R2, NOW + 2 * (DELAY + 1));
    assert!(reported);
    assert_eq!(points_left(&sent), 6);
}
