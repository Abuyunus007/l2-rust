#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The Seven Signs (M8-SS-01): joining a cabal at a priest, contributing seal stones, the period
//! changes with `CycleDatabaseWrites` on and off, the seals' effects (the necropolis gatekeeper,
//! the siege golem, the Mammon auto spawn) and the D-50 re-read of the record. The expected values
//! are read off the Java sources (`SevenSigns.java`, `SignsPriest.java`, `DawnPriest.java`,
//! `DungeonGatekeeper.java`, `AutoSpawnHandler.java`, `RequestSSQStatus.java`), not off the code.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{NpcHtmlMessage, SsqInfo, SsqStatus, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::sevensigns::{
    AutoSpawnGroup, CABAL_DAWN, CABAL_DUSK, CABAL_NULL, PERIOD_COMP_RECRUITING, PERIOD_COMP_RESULTS,
    PERIOD_COMPETITION, PERIOD_SEAL_VALIDATION, SEAL_AVARICE, SEAL_GNOSIS, SEAL_STRIFE, SevenSignsLoad,
    SevenSignsSettings, SevenSignsStatus, SevenSignsThen, SevenSignsWrite, SignsPlayer,
};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Debug, Clone)]
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

struct Recorder(Log);

impl Outbox for Recorder {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(Out::Persist(command));
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

/// A Gladiator (class 2: the second class transfer, so Dawn asks for the fee).
const CLASS: &str = r#"<list><classId>2</classId><staticData>
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

const ADENA: i32 = 57;
const ANCIENT_ADENA: i32 = 5575;
const BLUE: i32 = 6360;
const RED: i32 = 6362;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="5575" type="EtcItem" name="Ancient Adena"><set name="is_stackable" val="true" /></item>
<item id="5707" type="EtcItem" name="Record of Seven Signs"></item>
<item id="6360" type="EtcItem" name="Blue Seal Stone"><set name="is_stackable" val="true" /></item>
<item id="6361" type="EtcItem" name="Green Seal Stone"><set name="is_stackable" val="true" /></item>
<item id="6362" type="EtcItem" name="Red Seal Stone"><set name="is_stackable" val="true" /></item>
<item id="6388" type="EtcItem" name="Lord's Certificate"><set name="is_stackable" val="true" /></item>
</list>"#;

const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>"#;

const DAWN_PRIEST: i32 = 31078;
const GATEKEEPER: i32 = 31095;
const MAMMON_MERCHANT: i32 = 31113;

/// The links every test page offers, so each next bypass is in the html action cache.
const LINKS: &str = r#"<a action="bypass -h npc_%objectId%_SevenSigns 4 2 1">join</a><a action="bypass -h npc_%objectId%_SevenSigns 6 4">all stones</a><a action="bypass -h npc_%objectId%_SevenSigns 9">reward</a><a action="bypass -h npc_%objectId%_necro 1">necro</a>"#;

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ALICE: i32 = 0x1000_0001;
const BOB: i32 = 0x1000_0002;

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
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

/// UTC for the tests' clock.
fn utc(_ms: i64) -> i64 {
    0
}

/// 2026-01-07 (a Wednesday) noon UTC: in the middle of a competition week.
const WEDNESDAY: i64 = 1_767_571_200_000 + 2 * 86_400_000 + 12 * 3_600_000;

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new(cycle_writes: bool, status: SevenSignsStatus, players: Vec<SignsPlayer>) -> Self {
        let root = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = root.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        for page in [
            "dawn_priest_1a",
            "dawn_priest_1b",
            "dawn_priest_2a",
            "dawn_priest_2c",
            "dawn_priest_2d",
            "dawn_priest_3a",
            "dawn_priest_4",
            "signs_4_dawn",
            "signs_6_dawn",
            "signs_6_dawn_no_stones",
            "signs_9_dawn_a",
            "signs_9_dawn_b",
            "signs_33_dawn_fee",
            "necro_no",
        ] {
            write(
                &format!("data/html/seven_signs/{page}.htm"),
                &format!("<html><body>{page} {LINKS}</body></html>"),
            );
        }
        write(
            &format!("data/html/teleporter/{GATEKEEPER}.htm"),
            &format!("<html><body>gatekeeper {LINKS}</body></html>"),
        );
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Recorder(log.clone())));
        let stats = StatData {
            items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
            player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::default()),
        };
        world.set_static_data(Box::new(Data(stats)));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(
            NpcTemplates::from_xml(
                &format!(
                    r#"<list>
<npc id="{DAWN_PRIEST}" type="DawnPriest" level="70" name="Priest of Dawn">{STATS}</npc>
<npc id="{GATEKEEPER}" type="DungeonGatekeeper" level="70" name="Gatekeeper Ziggurat">{STATS}</npc>
<npc id="{MAMMON_MERCHANT}" type="SignsPriest" level="70" name="Merchant of Mammon">{STATS}</npc>
</list>"#
                ),
                &NpcLoadOptions::default(),
            )
            .unwrap(),
        );
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_seven_signs_settings(SevenSignsSettings {
            cycle_database_writes: cycle_writes,
            ..SevenSignsSettings::default()
        });
        world.load_seven_signs_at(
            SevenSignsLoad {
                players,
                status: Some(status),
                ..SevenSignsLoad::default()
            },
            WEDNESDAY,
            utc,
        );
        let s = Self {
            world,
            log,
            _root: root,
        };
        s.log.take();
        s
    }

    fn enter(&mut self, id: i32, name: &str, items: Vec<(i32, i64)>) {
        let items = items
            .into_iter()
            .enumerate()
            .map(|(i, (item_id, count))| ItemData {
                object_id: ObjectId(id + 0x100 + i as i32),
                item_id,
                count,
                loc: "INVENTORY".into(),
                ..ItemData::default()
            })
            .collect();
        let data = PlayerData {
            object_id: ObjectId(id),
            account: format!("acc{id}"),
            name: name.into(),
            title: String::new(),
            class_id: 2,
            base_class: 2,
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
            access_level: 0,
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
            ip: None,
        });
    }

    fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(id as u64),
            input,
        });
    }

    fn bypass(&mut self, id: i32, command: &str) {
        self.input(
            id,
            ClientInput::Bypass {
                command: command.into(),
            },
        );
    }

    /// Clicks the NPC twice: the first selects it, the second talks.
    fn talk_to(&mut self, id: i32, npc: ObjectId) {
        for _ in 0..2 {
            self.input(
                id,
                ClientInput::Action {
                    object_id: npc.0,
                    action_id: 0,
                },
            );
            self.ticks(2);
        }
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn take(&self) -> Vec<Out> {
        self.log.take()
    }

    fn count(&self, id: i32, item_id: i32) -> i64 {
        self.world
            .player(ObjectId(id))
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id && i.loc == "INVENTORY")
            .map(|i| i.count)
            .sum()
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

fn of<'a>(out: &'a [Out], to: i32, name: &str) -> Vec<&'a [u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, b) if *id == to && name_of(b) == name => Some(b.as_slice()),
            _ => None,
        })
        .collect()
}

fn messages(out: &[Out], to: i32) -> Vec<SystemMessage> {
    of(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode(raw))
        .collect()
}

fn message_ids(out: &[Out], to: i32) -> Vec<SystemMessageId> {
    messages(out, to).into_iter().map(|m| m.id).collect()
}

fn pages(out: &[Out], to: i32) -> Vec<String> {
    of(out, to, "NPC_HTML_MESSAGE")
        .iter()
        .map(|raw| decode::<NpcHtmlMessage>(raw).html)
        .collect()
}

fn writes(out: &[Out]) -> Vec<&SevenSignsWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::SevenSigns(w)) => Some(w),
            _ => None,
        })
        .collect()
}

fn competition() -> SevenSignsStatus {
    SevenSignsStatus {
        current_cycle: 4,
        active_period: PERIOD_COMPETITION,
        // Saved this Wednesday: no change was missed.
        last_save: WEDNESDAY - 3_600_000,
        ..SevenSignsStatus::default()
    }
}

fn member(char_id: i32, cabal: &str, seal: i32) -> SignsPlayer {
    SignsPlayer {
        char_id,
        cabal: cabal.into(),
        seal,
        ..SignsPlayer::default()
    }
}

// ---------------------------------------------------------------------------------- the cycle at start

/// The constructor: no change was missed, so the next one is Monday 18:00 local time.
#[test]
fn the_restore_schedules_the_next_monday() {
    let s = Scene::new(false, competition(), vec![]);
    assert_eq!(s.world.ss_current_period(), PERIOD_COMPETITION);
    // Wednesday noon to Monday 18:00 is five days and six hours (the seconds of now stay: 0).
    assert_eq!(
        s.world.ss_next_period_change(),
        WEDNESDAY + 5 * 86_400_000 + 6 * 3_600_000
    );
    assert!(s.world.ss_has_period_timer());
}

// ---------------------------------------------------------------------------------- register

/// `SevenSigns 4 2 1` at a dawn priest: a Gladiator without a castle pays the 50,000 adena fee
/// (after the Lord's Certificate it does not have), joins the Lords of Dawn for the Seal of
/// Avarice, and the new row is inserted (a per-player write, so with the switch off too).
#[test]
fn joining_the_dawn_pays_the_fee_and_inserts_the_row() {
    let mut s = Scene::new(false, competition(), vec![]);
    s.enter(ALICE, "Alice", vec![(ADENA, 60_000)]);
    let priest = s.world.spawn_npc_at(DAWN_PRIEST, X + 20, Y, Z, 0).unwrap();
    s.take();
    s.talk_to(ALICE, priest);
    let out = s.take();
    // `DawnPriest.showChatWindow`: no cabal, the competition: `dawn_priest_1a.htm`.
    assert!(pages(&out, ALICE).iter().any(|p| p.contains("dawn_priest_1a")));

    s.bypass(ALICE, &format!("npc_{}_SevenSigns 4 2 1", priest.0));
    let out = s.take();
    let ids = message_ids(&out, ALICE);
    // The certificate is tried first (`destroyItemByItemId(.., true)` says so), then the adena.
    assert!(ids.contains(&SystemMessageId::IncorrectItemCount2), "{ids:?}");
    assert!(ids.contains(&SystemMessageId::S1AdenaDisappeared), "{ids:?}");
    let joined = ids
        .iter()
        .position(|m| *m == SystemMessageId::YouWillParticipateInTheSevenSignsAsAMemberOfTheLordsOfDawn)
        .unwrap();
    assert_eq!(
        ids[joined + 1],
        SystemMessageId::YouVeChosenToFightForTheSealOfAvariceDuringThisQuestEventPeriod
    );
    assert!(pages(&out, ALICE).iter().any(|p| p.contains("signs_4_dawn")));
    assert_eq!(s.count(ALICE, ADENA), 10_000);
    assert_eq!(s.world.ss_player_cabal(ALICE), CABAL_DAWN);
    assert_eq!(
        s.world.seven_signs_status().dawn_seal_totals[SEAL_AVARICE as usize],
        1
    );
    assert_eq!(
        writes(&out),
        vec![&SevenSignsWrite::InsertPlayer {
            char_id: ALICE,
            cabal: "dawn".into(),
            seal: SEAL_AVARICE,
        }]
    );

    // A second join is refused: `signs_33_dawn_member.htm` (the member page).
    s.bypass(ALICE, &format!("npc_{}_SevenSigns 4 2 1", priest.0));
    let out = s.take();
    assert!(writes(&out).is_empty());
    assert_eq!(
        s.world.seven_signs_status().dawn_seal_totals[SEAL_AVARICE as usize],
        1
    );
}

/// Joining outside the competition period only shows the priest's window again.
#[test]
fn joining_needs_the_competition_period() {
    let status = SevenSignsStatus {
        active_period: PERIOD_SEAL_VALIDATION,
        ..competition()
    };
    let mut s = Scene::new(false, status, vec![]);
    s.enter(ALICE, "Alice", vec![(ADENA, 60_000)]);
    let priest = s.world.spawn_npc_at(DAWN_PRIEST, X + 20, Y, Z, 0).unwrap();
    s.talk_to(ALICE, priest);
    s.take();
    s.bypass(ALICE, &format!("npc_{}_SevenSigns 4 2 1", priest.0));
    let out = s.take();
    assert_eq!(s.world.ss_player_cabal(ALICE), CABAL_NULL);
    assert!(writes(&out).is_empty());
    // Validation period, a tie, no cabal: `dawn_priest_2d.htm`.
    assert!(pages(&out, ALICE).iter().any(|p| p.contains("dawn_priest_2d")));
}

// ---------------------------------------------------------------------------------- contribute

/// `SevenSigns 6 4`: every stone that fits under the limit, red first. 10 red and 2 blue are
/// 10 * 10 + 2 * 3 = 106 points and 106 Ancient Adena to collect; the Dawn's stone score grows.
/// With `AltSevenSignsLazyUpdate` (the default) nothing is written now; `Player.store` writes the
/// member's row.
#[test]
fn contributing_all_stones_counts_for_the_member_and_the_cabal() {
    let mut s = Scene::new(false, competition(), vec![member(ALICE, "dawn", SEAL_AVARICE)]);
    s.enter(ALICE, "Alice", vec![(RED, 10), (BLUE, 2)]);
    let priest = s.world.spawn_npc_at(DAWN_PRIEST, X + 20, Y, Z, 0).unwrap();
    s.talk_to(ALICE, priest);
    s.take();
    s.bypass(ALICE, &format!("npc_{}_SevenSigns 6 4", priest.0));
    let out = s.take();
    let msgs = messages(&out, ALICE);
    let gone: Vec<(i32, i64)> = msgs
        .iter()
        .filter(|m| m.id == SystemMessageId::S2S1HasDisappeared)
        .map(|m| match (&m.params[0], &m.params[1]) {
            (SystemMessageParam::ItemName(id), SystemMessageParam::LongNumber(n)) => (*id, *n),
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(gone, vec![(RED, 10), (BLUE, 2)]);
    let gained = msgs
        .iter()
        .find(|m| m.id == SystemMessageId::YourContributionScoreHasIncreasedByS1)
        .unwrap();
    assert_eq!(gained.params, vec![SystemMessageParam::LongNumber(106)]);
    assert!(pages(&out, ALICE).iter().any(|p| p.contains("signs_6_dawn ")));
    let row = s.world.seven_signs_player(ALICE).unwrap();
    assert_eq!(
        (
            row.red_stones,
            row.blue_stones,
            row.contribution_score,
            row.ancient_adena_amount
        ),
        (10, 2, 106, 106)
    );
    assert_eq!(s.world.seven_signs_status().dawn_stone_score, 106.0);
    assert_eq!(s.world.ss_current_score(CABAL_DAWN), 500);
    assert_eq!((s.count(ALICE, RED), s.count(ALICE, BLUE)), (0, 0));
    assert!(writes(&out).is_empty(), "lazy update: no write yet");

    // `Player.store` carries the row (`saveSevenSignsData(objectId)`).
    s.input(ALICE, ClientInput::Logout);
    let out = s.take();
    let stored = out
        .iter()
        .find_map(|o| match o {
            Out::Persist(PersistCommand::StorePlayer(p)) => Some(p.profile.seven_signs_to_store.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(stored.map(|r| r.contribution_score), Some(106));
}

/// The contribution limit: a member at `AltMaxPlayerContrib` is refused at once.
#[test]
fn a_member_at_the_limit_may_not_contribute() {
    let full = SignsPlayer {
        contribution_score: 1_000_000,
        ..member(ALICE, "dawn", SEAL_AVARICE)
    };
    let mut s = Scene::new(false, competition(), vec![full]);
    s.enter(ALICE, "Alice", vec![(RED, 10)]);
    let priest = s.world.spawn_npc_at(DAWN_PRIEST, X + 20, Y, Z, 0).unwrap();
    s.talk_to(ALICE, priest);
    s.take();
    s.bypass(ALICE, &format!("npc_{}_SevenSigns 6 4", priest.0));
    let out = s.take();
    assert!(
        message_ids(&out, ALICE)
            .contains(&SystemMessageId::ContributionLevelHasExceededTheLimitYouMayNotContinue)
    );
    assert_eq!(s.count(ALICE, RED), 10);
}

/// `SevenSigns 9` in the seal validation period: a member of the winner collects the Ancient
/// Adena of his stones; the stones and the amount of his row go back to 0.
#[test]
fn the_winners_collect_their_ancient_adena() {
    let mut status = competition();
    status.active_period = PERIOD_SEAL_VALIDATION;
    status.dawn_stone_score = 106.0;
    let row = SignsPlayer {
        red_stones: 10,
        blue_stones: 2,
        ancient_adena_amount: 106,
        contribution_score: 106,
        ..member(ALICE, "dawn", SEAL_AVARICE)
    };
    let mut s = Scene::new(false, status, vec![row]);
    s.enter(ALICE, "Alice", vec![]);
    let priest = s.world.spawn_npc_at(DAWN_PRIEST, X + 20, Y, Z, 0).unwrap();
    s.talk_to(ALICE, priest);
    let out = s.take();
    // Dawn won, but does not own Gnosis: `dawn_priest_2c.htm`.
    assert!(pages(&out, ALICE).iter().any(|p| p.contains("dawn_priest_2c")));
    s.bypass(ALICE, &format!("npc_{}_SevenSigns 9", priest.0));
    let out = s.take();
    assert_eq!(s.count(ALICE, ANCIENT_ADENA), 106);
    let earned = messages(&out, ALICE)
        .into_iter()
        .find(|m| m.id == SystemMessageId::YouHaveEarnedS2S1S)
        .unwrap();
    assert_eq!(
        earned.params,
        vec![
            SystemMessageParam::ItemName(ANCIENT_ADENA),
            SystemMessageParam::LongNumber(106)
        ]
    );
    assert!(pages(&out, ALICE).iter().any(|p| p.contains("signs_9_dawn_a")));
    let row = s.world.seven_signs_player(ALICE).unwrap();
    assert_eq!(
        (
            row.red_stones,
            row.blue_stones,
            row.ancient_adena_amount,
            row.contribution_score
        ),
        (0, 0, 0, 106)
    );
    // A second time there is nothing left (less than 3): `signs_9_dawn_b.htm`.
    s.bypass(ALICE, &format!("npc_{}_SevenSigns 9", priest.0));
    let out = s.take();
    assert!(pages(&out, ALICE).iter().any(|p| p.contains("signs_9_dawn_b")));
    assert_eq!(s.count(ALICE, ANCIENT_ADENA), 106);
}

// ---------------------------------------------------------------------------------- the cycle

/// Two Dawn members both chose Avarice and the Dawn lead: at the end of the competition the Dawn
/// take the Seal of Avarice (100% >= 35%), Gnosis and Strife stay unclaimed. With
/// `CycleDatabaseWrites = True` the whole table and the status row are written.
fn dawn_wins() -> (SevenSignsStatus, Vec<SignsPlayer>) {
    let mut status = competition();
    status.dawn_stone_score = 100.0;
    status.dawn_seal_totals[SEAL_AVARICE as usize] = 2;
    (
        status,
        vec![
            member(ALICE, "dawn", SEAL_AVARICE),
            member(BOB, "dawn", SEAL_AVARICE),
        ],
    )
}

#[test]
fn the_cycle_runs_and_writes_with_the_switch_on() {
    let (status, players) = dawn_wins();
    let mut s = Scene::new(true, status, players);
    s.enter(ALICE, "Alice", vec![]);
    s.take();
    let monday = s.world.ss_next_period_change();

    // Competition -> results (the timer fires at Monday 18:00; the clock of the call is now).
    s.world.ss_period_change_at(WEDNESDAY, utc);
    let out = s.take();
    assert_eq!(s.world.ss_current_period(), PERIOD_COMP_RESULTS);
    assert_eq!(s.world.ss_seal_owner(SEAL_AVARICE), CABAL_DAWN);
    assert_eq!(s.world.ss_seal_owner(SEAL_GNOSIS), CABAL_NULL);
    assert_eq!(s.world.ss_seal_owner(SEAL_STRIFE), CABAL_NULL);
    assert_eq!(s.world.seven_signs_status().previous_winner, CABAL_DAWN);
    assert_eq!(
        message_ids(&out, ALICE),
        vec![
            SystemMessageId::SevenSignsTheCompetitionPeriodHasEndedTheNextQuestEventWillStartInOneWeek,
            SystemMessageId::SevenSignsTheLordsOfDawnHaveObtainedTheSealOfAvarice,
            SystemMessageId::SevenSignsTheLordsOfDawnHaveWon,
        ]
    );
    let w = writes(&out);
    assert!(matches!(w[0], SevenSignsWrite::UpdateAllPlayers(rows) if rows.len() == 2));
    match w[1] {
        SevenSignsWrite::UpdateStatus(st) => {
            assert_eq!(st.active_period, PERIOD_COMP_RESULTS);
            assert_eq!(st.seal_owners[SEAL_AVARICE as usize], CABAL_DAWN);
            assert!(st.last_save > 0);
        }
        other => panic!("{other:?}"),
    }
    // The sky does not change before the seal validation.
    let sky: SsqInfo = decode(of(&out, ALICE, "SSQ_INFO")[0]);
    assert_eq!(sky.state, 0);
    // The results period lasts 15 minutes after the change (Java adds them to `_nextPeriodChange`).
    assert_eq!(s.world.ss_next_period_change(), monday + 900_000);

    // Results -> seal validation: the Dawn's sky.
    s.world.ss_period_change_at(WEDNESDAY + 900_000, utc);
    let out = s.take();
    assert_eq!(s.world.ss_current_period(), PERIOD_SEAL_VALIDATION);
    assert_eq!(
        message_ids(&out, ALICE),
        vec![SystemMessageId::SevenSignsTheSealValidationPeriodHasBegun]
    );
    let sky: SsqInfo = decode(of(&out, ALICE, "SSQ_INFO")[0]);
    assert_eq!(sky.state, 2);

    // Seal validation -> a new cycle: everybody's cabal, seal and score go, the seals stay.
    s.world.ss_period_change_at(WEDNESDAY + 1_800_000, utc);
    let out = s.take();
    assert_eq!(s.world.ss_current_period(), PERIOD_COMP_RECRUITING);
    assert_eq!(s.world.ss_current_cycle(), 5);
    assert_eq!(s.world.ss_player_cabal(ALICE), CABAL_NULL);
    assert_eq!(s.world.seven_signs_status().dawn_seal_totals, [0; 4]);
    assert_eq!(s.world.seven_signs_status().dawn_stone_score, 0.0);
    assert_eq!(s.world.ss_seal_owner(SEAL_AVARICE), CABAL_DAWN);
    assert_eq!(
        message_ids(&out, ALICE),
        vec![SystemMessageId::SevenSignsTheSealValidationPeriodHasEnded]
    );
    assert_eq!(writes(&out).len(), 2);
}

/// The same change with the switch off (D-53): the world changes, but neither the table nor the
/// status row is written.
#[test]
fn the_cycle_writes_nothing_global_with_the_switch_off() {
    let (status, players) = dawn_wins();
    let mut s = Scene::new(false, status, players);
    s.world.ss_period_change_at(WEDNESDAY, utc);
    let out = s.take();
    assert_eq!(s.world.ss_current_period(), PERIOD_COMP_RESULTS);
    assert_eq!(s.world.ss_seal_owner(SEAL_AVARICE), CABAL_DAWN);
    assert!(writes(&out).is_empty(), "{:?}", writes(&out));
    assert!(s.world.ss_has_period_timer());
}

// ---------------------------------------------------------------------------------- the record

/// `RequestSSQStatus` with the switch off: the rows are read first (D-50); the page shows what the
/// Java game server wrote. A failed read shows nothing. Page 4 is refused in the validation period.
#[test]
fn the_record_is_read_again_with_the_switch_off() {
    let mut s = Scene::new(false, competition(), vec![]);
    s.enter(ALICE, "Alice", vec![]);
    s.take();
    s.input(ALICE, ClientInput::SsqStatus { page: 1 });
    let out = s.take();
    assert!(of(&out, ALICE, "SSQ_STATUS").is_empty());
    let then = out
        .iter()
        .find_map(|o| match o {
            Out::Persist(PersistCommand::SevenSignsReadRequest { then }) => Some(then.clone()),
            _ => None,
        })
        .unwrap();
    assert_eq!(
        then,
        SevenSignsThen::Status {
            player: ObjectId(ALICE),
            page: 1
        }
    );

    // A failed read: nothing.
    s.world.handle(WorldCommand::SevenSignsLoaded {
        result: Err(()),
        then: then.clone(),
    });
    assert!(of(&s.take(), ALICE, "SSQ_STATUS").is_empty());

    // Sieghardt moved on to the seal validation, the Dusk own the Seal of Strife and Bob joined.
    let mut sieghardt = competition();
    sieghardt.active_period = PERIOD_SEAL_VALIDATION;
    sieghardt.dusk_stone_score = 50.0;
    sieghardt.seal_owners[SEAL_STRIFE as usize] = CABAL_DUSK;
    s.world.handle(WorldCommand::SevenSignsLoaded {
        result: Ok(SevenSignsLoad {
            players: vec![member(BOB, "dusk", SEAL_STRIFE)],
            status: Some(sieghardt),
            ..SevenSignsLoad::default()
        }),
        then,
    });
    let out = s.take();
    let page: SsqStatus = decode(of(&out, ALICE, "SSQ_STATUS")[0]);
    assert_eq!((page.page, page.period), (1, PERIOD_SEAL_VALIDATION as u8));
    assert_eq!(s.world.seven_signs_strife_owner(), CABAL_DUSK);
    assert_eq!(s.world.ss_total_members(CABAL_DUSK), 1);
    // The next period change follows the new period: Monday 18:00.
    assert!(s.world.ss_has_period_timer());

    // Page 4 is not shown in the seal validation period.
    s.input(ALICE, ClientInput::SsqStatus { page: 4 });
    let out = s.take();
    assert!(
        !out.iter()
            .any(|o| matches!(o, Out::Persist(PersistCommand::SevenSignsReadRequest { .. })))
    );
}

/// With the switch on the record comes from memory at once.
#[test]
fn the_record_is_shown_at_once_with_the_switch_on() {
    let mut s = Scene::new(true, competition(), vec![member(ALICE, "dawn", SEAL_GNOSIS)]);
    s.enter(ALICE, "Alice", vec![]);
    s.take();
    s.input(ALICE, ClientInput::SsqStatus { page: 3 });
    let out = s.take();
    let page: SsqStatus = decode(of(&out, ALICE, "SSQ_STATUS")[0]);
    assert_eq!(page.page, 3);
}

// ---------------------------------------------------------------------------------- seal effects

/// The seal validation after the Dawn won and took the Seal of Avarice: only Dawn members may
/// enter the necropolises (`DungeonGatekeeper`), and they are in a Seven Signs dungeon then.
#[test]
fn the_seal_of_avarice_opens_the_necropolis_to_the_winner_only() {
    let mut status = competition();
    status.active_period = PERIOD_SEAL_VALIDATION;
    status.dawn_stone_score = 10.0;
    status.seal_owners[SEAL_AVARICE as usize] = CABAL_DAWN;
    let mut s = Scene::new(
        false,
        status,
        vec![
            member(ALICE, "dawn", SEAL_AVARICE),
            member(BOB, "dusk", SEAL_AVARICE),
        ],
    );
    s.enter(ALICE, "Alice", vec![]);
    s.enter(BOB, "Bob", vec![]);
    let gatekeeper = s.world.spawn_npc_at(GATEKEEPER, X + 20, Y, Z, 0).unwrap();
    s.talk_to(BOB, gatekeeper);
    s.take();
    s.bypass(BOB, &format!("npc_{}_necro 1", gatekeeper.0));
    let out = s.take();
    assert!(message_ids(&out, BOB).contains(&SystemMessageId::OnlyALordOfDawnMayUseThis));
    assert!(pages(&out, BOB).iter().any(|p| p.contains("necro_no")));
    assert!(!s.world.is_in_7s_dungeon(ObjectId(BOB)));

    s.talk_to(ALICE, gatekeeper);
    s.take();
    s.bypass(ALICE, &format!("npc_{}_necro 1", gatekeeper.0));
    let out = s.take();
    assert!(!message_ids(&out, ALICE).contains(&SystemMessageId::OnlyALordOfDawnMayUseThis));
    assert!(s.world.is_in_7s_dungeon(ObjectId(ALICE)));
}

/// The Seal of Strife owned by the Dawn in the seal validation period: a Dusk member may not
/// summon a siege golem (`checkSummonConditions`); and the period change moves the Dusk out of the
/// dungeons.
#[test]
fn the_seal_of_strife_stops_the_dusk_golems() {
    let mut status = competition();
    status.active_period = PERIOD_SEAL_VALIDATION;
    status.seal_owners[SEAL_STRIFE as usize] = CABAL_DAWN;
    let mut s = Scene::new(
        false,
        status,
        vec![
            member(ALICE, "dawn", SEAL_STRIFE),
            member(BOB, "dusk", SEAL_STRIFE),
        ],
    );
    s.enter(ALICE, "Alice", vec![]);
    s.enter(BOB, "Bob", vec![]);
    s.take();
    assert!(s.world.ss_check_summon_conditions(ObjectId(BOB)));
    assert!(
        message_ids(&s.take(), BOB)
            .contains(&SystemMessageId::DueToTheAffectsOfTheSealOfStrifeItIsNotPossibleToSummonAtThisTime)
    );
    assert!(!s.world.ss_check_summon_conditions(ObjectId(ALICE)));
    assert_eq!(s.world.seven_signs_strife_owner(), CABAL_DAWN);
}

/// `spawnSevenSignsNPC` with `AutoSpawnHandler`: the Merchant of Mammon of `random_spawn` comes 30
/// seconds after the seal validation begins with the Dawn owning Avarice, and leaves when the new
/// cycle starts.
#[test]
fn the_merchant_of_mammon_follows_the_seal_of_avarice() {
    let (status, players) = dawn_wins();
    let mut s = Scene::new(false, status, players);
    s.world.load_auto_spawns(vec![AutoSpawnGroup {
        npc_id: MAMMON_MERCHANT,
        count: 1,
        initial_delay: -1,
        respawn_delay: -1,
        despawn_delay: -1,
        broadcast: true,
        random_spawn: true,
        locations: vec![(X + 100, Y, Z, -1)],
    }]);
    // The competition: the merchant is not active.
    s.world.spawn_seven_signs_npc();
    assert_eq!(s.world.auto_spawn_active(MAMMON_MERCHANT), vec![false]);

    // To the results: Dawn wins Avarice; the merchant is active and comes after 30 seconds.
    s.world.ss_period_change_at(WEDNESDAY, utc);
    assert_eq!(s.world.auto_spawn_active(MAMMON_MERCHANT), vec![true]);
    assert!(s.world.auto_spawn_npcs(MAMMON_MERCHANT).is_empty());
    s.ticks(601);
    assert_eq!(s.world.auto_spawn_npcs(MAMMON_MERCHANT).len(), 1);

    // Through the validation period to the new cycle: the merchant goes.
    s.world.ss_period_change_at(WEDNESDAY + 900_000, utc);
    s.world.ss_period_change_at(WEDNESDAY + 1_800_000, utc);
    assert_eq!(s.world.auto_spawn_active(MAMMON_MERCHANT), vec![false]);
    assert!(s.world.auto_spawn_npcs(MAMMON_MERCHANT).is_empty());
}
