#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The Festival of Darkness (M8-FEST-01): signing a party up at a guide, a whole round of the
//! manager (the arena, the witch, the monsters, the second spawn, the chests, the end), the blood
//! offerings, the score registration and the rows it writes with `CycleDatabaseWrites` on and
//! off (D-53), and the score tables read again (D-50). The expected values are read off the Java
//! sources (`SevenSignsFestival.java`, `FestivalGuide.java`, `FestivalMonster.java`,
//! `Festival.java`), not off the code.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::sevensigns::{
    CABAL_DAWN, CABAL_DUSK, FestivalData, FestivalSettings, PERIOD_COMPETITION, PERIOD_SEAL_VALIDATION, SevenSignsLoad,
    SevenSignsSettings, SevenSignsStatus, SevenSignsThen, SevenSignsWrite, SignsPlayer,
};
use l2_world::*;
use l2_world::teleport::TeleportTarget;

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

const BLUE: i32 = 6360;
const OFFERING: i32 = 5901;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="5901" type="EtcItem" name="Blood Offering"><set name="is_stackable" val="true" /></item>
<item id="6360" type="EtcItem" name="Blue Seal Stone"><set name="is_stackable" val="true" /></item>
</list>"#;

const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>"#;

/// The dawn guide of the lowest range (the oracle's chat guide too), the dusk chat guide and the
/// witch of that range.
const DAWN_GUIDE: i32 = 31127;
const DUSK_GUIDE: i32 = 31137;
const DAWN_WITCH: i32 = 31132;

/// The monsters of the primary and secondary lists of the Dawn's lowest range.
const MONSTERS: [i32; 10] = [18009, 18010, 18011, 18012, 18013, 18014, 18015, 18016, 18017, 18018];
/// The chest of that range.
const CHEST: i32 = 18109;

/// Every command a page lets the player send (the html action cache).
const LINKS: &str = r#"<a action="bypass -h npc_%objectId%_Festival 1">a</a><a action="bypass -h npc_%objectId%_Festival 2 6360">b</a><a action="bypass -h npc_%objectId%_Festival 3">c</a><a action="bypass -h npc_%objectId%_Festival 4">d</a><a action="bypass -h npc_%objectId%_Festival 5">e</a><a action="bypass -h npc_%objectId%_Festival 6">f</a><a action="bypass -h npc_%objectId%_Festival 8">g</a><a action="bypass -h npc_%objectId%_Festival 9">h</a><a action="bypass -h npc_%objectId%_Festival 0">i</a>"#;

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const NAMES: [&str; 5] = ["Alpha", "Bravo", "Charlie", "Delta", "Echo"];
const P: [i32; 5] = [0x1000_0001, 0x1000_0002, 0x1000_0003, 0x1000_0004, 0x1000_0005];

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

fn utc(_ms: i64) -> i64 {
    0
}

fn real_now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis() as i64
}

/// Short festival times (ms): a round of 100 s after the 80 s of signing up.
fn short_festival() -> FestivalSettings {
    FestivalSettings {
        min_player: 5,
        manager_start: 1_000,
        length: 100_000,
        cycle_length: 240_000,
        first_spawn: 5_000,
        first_swarm: 10_000,
        second_spawn: 20_000,
        second_swarm: 30_000,
        chest_spawn: 40_000,
        win_points: 200,
    }
}

/// The ten rows `resetFestivalData` makes for a cycle.
fn rows_of(cycle: i32) -> Vec<FestivalData> {
    let mut rows = Vec::new();
    for cabal in ["dusk", "dawn"] {
        for festival_id in 0..5 {
            rows.push(FestivalData {
                festival_id,
                cabal: cabal.into(),
                cycle,
                date: 0,
                score: 0,
                members: String::new(),
            });
        }
    }
    rows
}

struct Scene {
    world: World,
    log: Log,
    guide: ObjectId,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new(cycle_writes: bool, rows: Vec<FestivalData>) -> Self {
        let status = SevenSignsStatus {
            current_cycle: 4,
            active_period: PERIOD_COMPETITION,
            last_save: real_now() - 60_000,
            ..SevenSignsStatus::default()
        };
        Self::with(cycle_writes, rows, status, Vec::new())
    }

    fn with(
        cycle_writes: bool,
        rows: Vec<FestivalData>,
        status: SevenSignsStatus,
        players: Vec<SignsPlayer>,
    ) -> Self {
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
        write(
            "data/html/seven_signs/festival/dawn_guide.htm",
            &format!("<html><body>dawn guide %festivalMins% {LINKS}</body></html>"),
        );
        write(
            "data/html/seven_signs/festival/festival_witch.htm",
            &format!("<html><body>witch {LINKS}</body></html>"),
        );
        for page in [
            "festival_1",
            "festival_2a",
            "festival_2b",
            "festival_2c",
            "festival_2d",
            "festival_2e",
            "festival_2f",
            "festival_3a",
            "festival_3b",
            "festival_3c",
            "festival_3d",
            "festival_5",
            "festival_6",
            "festival_8a",
            "festival_8b",
            "festival_8c",
            "festival_0a",
            "festival_0b",
        ] {
            let table = match page {
                "festival_1" => "%festivalType% %cycleMins% %blueStoneNeeded%/%greenStoneNeeded%/%redStoneNeeded%",
                "festival_2b" => "min %minFestivalPartyMembers%",
                "festival_5" => "%statsTable%",
                "festival_6" => "%bonusTable%",
                _ => "",
            };
            write(
                &format!("data/html/seven_signs/festival/{page}.htm"),
                &format!("<html><body>{page} {table} {LINKS}</body></html>"),
            );
        }
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(11), Box::new(Recorder(log.clone())));
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
        let mut npcs = format!(
            r#"<list>
<npc id="{DAWN_GUIDE}" type="FestivalGuide" level="70" name="Festival Guide">{STATS}</npc>
<npc id="{DUSK_GUIDE}" type="FestivalGuide" level="70" name="Festival Guide">{STATS}</npc>
<npc id="{DAWN_WITCH}" type="FestivalGuide" level="70" name="Festival Witch">{STATS}</npc>
<npc id="{CHEST}" type="FestivalMonster" level="30" name="Treasures of the Festival">{STATS}</npc>"#
        );
        for id in MONSTERS {
            npcs.push_str(&format!(
                r#"<npc id="{id}" type="FestivalMonster" level="30" name="Offering Monster {id}">{STATS}</npc>"#
            ));
        }
        npcs.push_str("</list>");
        let npcs = Arc::new(NpcTemplates::from_xml(&npcs, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_seven_signs_settings(SevenSignsSettings {
            cycle_database_writes: cycle_writes,
            festival: short_festival(),
            ..SevenSignsSettings::default()
        });
        // The next period change is on a Monday: far from the round (the manager does not run
        // when the period is due to change within one cycle).
        world.load_seven_signs_at(
            SevenSignsLoad {
                players,
                status: Some(status),
                festival: rows,
            },
            real_now(),
            utc,
        );
        let guide = world.spawn_npc_at(DAWN_GUIDE, X, Y, Z, 0).unwrap();
        world.spawn_npc_at(DUSK_GUIDE, X + 3000, Y, Z, 0).unwrap();
        let s = Self {
            world,
            log,
            guide,
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

    /// The five players, the first one's party, the guide's page open for the leader.
    fn party_of_five(&mut self) {
        self.enter(P[0], NAMES[0], vec![(BLUE, 900)]);
        for i in 1..5 {
            self.enter(P[i], NAMES[i], Vec::new());
        }
        for i in 1..5 {
            self.input(
                P[0],
                ClientInput::JoinParty {
                    name: NAMES[i].into(),
                    distribution_type_id: 0,
                },
            );
            self.input(P[i], ClientInput::AnswerJoinParty { response: 1 });
        }
        // Two clicks: select the guide, then talk to it (its page has the links).
        for _ in 0..2 {
            self.input(
                P[0],
                ClientInput::Action {
                    object_id: self.guide.0,
                    action_id: 0,
                },
            );
            self.ticks(2);
        }
        self.log.take();
    }

    fn bypass(&mut self, id: i32, command: &str) {
        let guide = self.guide;
        self.bypass_npc(id, guide, command);
    }

    fn bypass_npc(&mut self, id: i32, npc: ObjectId, command: &str) {
        let command = format!("npc_{}_{command}", npc.0);
        self.input(id, ClientInput::Bypass { command });
    }

    /// Two clicks: select the NPC, then talk to it (its page has the links).
    fn click(&mut self, id: i32, npc: ObjectId) {
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

    /// A monster dies of `killer`'s hand (he did the damage).
    fn kill(&mut self, monster: ObjectId, killer: i32) {
        self.world.add_damage_hate(monster, ObjectId(killer), 10, 1);
        assert!(self.world.do_die(monster, Some(ObjectId(killer))));
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    /// Waits `ms` of world time.
    fn wait_ms(&mut self, ms: u32) {
        self.ticks(ms.div_ceil(50));
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

    fn position(&self, id: i32) -> (i32, i32, i32) {
        self.world.position_of(ObjectId(id)).unwrap()
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

fn pages(out: &[Out], to: i32) -> Vec<String> {
    of(out, to, "NPC_HTML_MESSAGE")
        .iter()
        .map(|raw| decode::<NpcHtmlMessage>(raw).html)
        .collect()
}

fn messages(out: &[Out], to: i32) -> Vec<SystemMessage> {
    of(out, to, "SYSTEM_MESSAGE").iter().map(|raw| decode(raw)).collect()
}

fn festival_writes(out: &[Out]) -> Vec<&Vec<FestivalData>> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::SevenSigns(SevenSignsWrite::ReplaceFestival(rows))) => Some(rows),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------- tests

/// The manager of a competition period starts at once (`startFestivalManager` at the server
/// start) and counts the cycle up; with no registration no arena is made.
#[test]
fn the_manager_starts_with_the_competition_and_an_empty_round_makes_no_arena() {
    let mut s = Scene::new(true, rows_of(4));
    // `new FestivalManager()` increments the cycle counter.
    assert_eq!(s.world.festival_cycle(), 1);
    assert!(!s.world.festival_initialized());
    // The first round starts after 1 s, signs up for 80 s and finds nobody: it waits another
    // `CYCLE - SIGNUP` and looks again.
    s.wait_ms(1_000 + 80_000 + 1_000);
    assert!(!s.world.festival_initialized());
    assert!(s.world.festival_witch(CABAL_DAWN, 0).is_none());
}

/// A page of the guide: `%festivalMins%`, the fee and the level range are filled in; a party of
/// five that pays the stones signs up (`Festival 2`), the stones go and the bonus grows.
#[test]
fn a_party_signs_up_at_the_guide() {
    let mut s = Scene::new(true, rows_of(4));
    s.party_of_five();
    // "Become a participant": the page with the fee of the range.
    s.bypass(P[0], "Festival 1");
    let out = s.take();
    let page = pages(&out, P[0]).pop().expect("the fee page");
    assert!(page.contains("Level 31 or lower"), "{page}");
    assert!(page.contains("900/540/270"), "{page}");
    assert!(!page.contains('%'), "{page}");
    // Not registered yet.
    assert!(!s.world.is_festival_participant(ObjectId(P[0])));

    // The blue stones: 900 for the lowest range.
    s.bypass(P[0], "Festival 2 6360");
    let out = s.take();
    assert_eq!(s.count(P[0], BLUE), 0);
    assert!(pages(&out, P[0]).pop().unwrap().contains("festival_2e"));
    assert!(s.world.is_festival_participant(ObjectId(P[0])));
    assert!(s.world.is_festival_participant(ObjectId(P[4])));
    assert_eq!(
        s.world.festival_participants(CABAL_DAWN, 0).map(Vec::len),
        Some(5)
    );
    // Each blue stone is worth 3 in the bonus of that festival.
    assert_eq!(s.world.festival_accumulated_bonus(0), 2_700);
    assert_eq!(s.world.festival_total_accumulated_bonus(), 2_700);
}

/// A party that is too small, or whose member is too high for the range, is turned away.
#[test]
fn a_small_party_and_a_high_level_are_refused() {
    let mut s = Scene::new(true, rows_of(4));
    s.enter(P[0], NAMES[0], Vec::new());
    for _ in 0..2 {
        s.input(
            P[0],
            ClientInput::Action {
                object_id: s.guide.0,
                action_id: 0,
            },
        );
        s.ticks(2);
    }
    s.take();
    // No party: page 2b, with the minimum number of members.
    s.bypass(P[0], "Festival 1");
    let page = pages(&s.take(), P[0]).pop().unwrap();
    assert!(page.contains("festival_2b") && page.contains("min 5"), "{page}");
}

/// A whole round with a registered party: the arena at the end of the sign up, the witch, the
/// monsters, the offerings for the party leader, the second spawn, the chests with five times the
/// offering and the end that sends everybody back.
#[test]
fn a_festival_round_runs_from_the_signup_to_the_end() {
    let mut s = Scene::new(true, rows_of(4));
    s.party_of_five();
    // A stray blood offering of a member goes when he is taken to the arena.
    s.world.add_item(ObjectId(P[1]), OFFERING, 3, false);
    s.bypass(P[0], "Festival 2 6360");
    s.take();
    assert_eq!(s.count(P[1], OFFERING), 3);

    // 1 s until the manager's round, 80 s of signing up.
    s.wait_ms(1_000 + 80_000);
    assert!(s.world.festival_initialized());
    assert!(!s.world.festival_in_progress());
    // The party stands in the arena of the lowest Dawn range: around (-79187, 113186, -4895),
    // at most 230 away, and the stray offering is gone.
    for id in P {
        let (x, y, z) = s.position(id);
        assert!((x + 79_187).abs() <= 230 + 100 && (y - 113_186).abs() <= 230 + 100, "{x} {y}");
        assert_eq!(z, -4_895 + 5, "{id}");
    }
    assert_eq!(s.count(P[1], OFFERING), 0);
    // The witch of the arena is there, the monsters are not yet.
    let witch = s.world.festival_witch(CABAL_DAWN, 0).expect("the witch");
    assert_eq!(s.world.npc(witch).unwrap().npc_id, DAWN_WITCH);
    assert!(s.world.festival_monsters(CABAL_DAWN, 0).is_empty());
    // A signed up player cannot log out meanwhile.
    s.input(P[2], ClientInput::Logout);
    assert!(s.world.player(ObjectId(P[2])).is_some());
    // The party arrives (the client loaded the new place) and talks to the witch.
    let (wx, wy, wz) = s.world.position_of(witch).unwrap();
    for id in [P[0], P[2]] {
        s.input(id, ClientInput::Appearing);
        s.world.tele_to_location(ObjectId(id), TeleportTarget::at(wx, wy + 20, wz));
        s.input(id, ClientInput::Appearing);
        s.click(id, witch);
    }

    // 5 s later the festival starts: the 16 primary monsters, a stretch where challenge can rise.
    s.wait_ms(5_000);
    assert!(s.world.festival_in_progress());
    let monsters = s.world.festival_monsters(CABAL_DAWN, 0);
    assert_eq!(monsters.len(), 16);
    // Only the leader can ask for more challenge.
    s.bypass_npc(P[2], witch, "Festival 8");
    assert_eq!(s.world.festival_monsters(CABAL_DAWN, 0).len(), 16);
    s.bypass_npc(P[0], witch, "Festival 8");
    // The twist: the 13 monsters of the primary list that are no archers (the ids ending in 4 or
    // 9: 18009 twice and 18014), with twice the offering.
    let after = s.world.festival_monsters(CABAL_DAWN, 0);
    assert_eq!(after.len(), 16 + 13);
    // ... and only once.
    s.bypass_npc(P[0], witch, "Festival 8");
    assert_eq!(s.world.festival_monsters(CABAL_DAWN, 0).len(), 29);

    // A member kills a plain monster: the leader gets one offering, nobody else.
    s.kill(monsters[0], P[3]);
    assert_eq!(s.count(P[0], OFFERING), 1);
    assert_eq!(s.count(P[3], OFFERING), 0);
    // A monster of the increased challenge gives two.
    s.kill(after[16], P[3]);
    assert_eq!(s.count(P[0], OFFERING), 3);

    // 15 s on (20 s after the start): the second spawn, 10 archers.
    s.wait_ms(15_000);
    assert_eq!(s.world.festival_monsters(CABAL_DAWN, 0).len(), 29 + 10);
    // 20 s on (40 s after the start): the 12 chests.
    s.wait_ms(20_000);
    let all = s.world.festival_monsters(CABAL_DAWN, 0);
    assert_eq!(all.len(), 29 + 10 + 12);
    // A chest is worth five offerings.
    let chest = *all.last().unwrap();
    assert_eq!(s.world.npc(chest).unwrap().npc_id, CHEST);
    s.kill(chest, P[1]);
    assert_eq!(s.count(P[0], OFFERING), 8);

    // The end, 100 s after the start of the round: everybody is back where they were, the
    // arena is empty and the sign up is open again.
    s.wait_ms(60_000 + 100);
    assert!(!s.world.festival_in_progress());
    assert!(!s.world.festival_initialized());
    assert!(s.world.festival_monsters(CABAL_DAWN, 0).is_empty());
    assert!(!s.world.is_festival_participant(ObjectId(P[0])));
    for id in P {
        let (x, y, _) = s.position(id);
        // `teleToLocation(origin, true)`: the random offset of a teleport.
        assert!((x - X).abs() <= 100 && (y - Y).abs() <= 100, "{id}: {x} {y}");
    }
    // The leader and his party are the previous participants who may register the score.
    let previous = s.world.festival_previous_participants(CABAL_DAWN, 0).cloned().unwrap();
    assert_eq!(previous.len(), 5);
    assert_eq!(previous[0], ObjectId(P[0]));
}

/// The score is registered by the leader of the last party: the offerings go, the score is the
/// best of the cycle, the cabal gets the festival points of the range and (with the switch on)
/// the rows are written.
#[test]
fn the_leader_registers_the_score_and_the_rows_are_written_with_the_switch_on() {
    let mut s = Scene::new(true, rows_of(4));
    s.party_of_five();
    s.bypass(P[0], "Festival 2 6360");
    s.wait_ms(1_000 + 80_000 + 5_000);
    let monsters = s.world.festival_monsters(CABAL_DAWN, 0);
    s.kill(monsters[0], P[1]);
    s.kill(monsters[1], P[1]);
    assert_eq!(s.count(P[0], OFFERING), 2);
    s.wait_ms(100_000 - 5_000 + 100);
    // Back at the guide (the clients load the place), both talk to it again.
    for id in [P[0], P[1]] {
        s.input(id, ClientInput::Appearing);
        s.click(id, s.guide);
    }
    s.take();

    // Not the leader.
    s.bypass(P[1], "Festival 3");
    assert!(pages(&s.take(), P[1]).pop().unwrap().contains("festival_3b"));
    // The leader: 2 offerings of 5 points.
    s.bypass(P[0], "Festival 3");
    let out = s.take();
    assert_eq!(s.count(P[0], OFFERING), 0);
    let gained = messages(&out, P[0])
        .into_iter()
        .find(|m| m.id == SystemMessageId::YourContributionScoreHasIncreasedByS1)
        .expect("the contribution message");
    assert_eq!(gained.params, vec![SystemMessageParam::LongNumber(10)]);
    // The best score of this cycle, "highest" page 3c.
    assert!(pages(&out, P[0]).pop().unwrap().contains("festival_3c"));
    assert_eq!(s.world.festival_highest_score(CABAL_DAWN, 0), 10);
    assert_eq!(s.world.festival_highest_score(CABAL_DUSK, 0), 0);
    // The cabal gets the 60 points of the lowest range.
    assert_eq!(s.world.seven_signs_status().dawn_festival_score, 60);
    // The data is written: the whole table once, with the new row.
    let writes = festival_writes(&out);
    assert_eq!(writes.len(), 1);
    let row = writes[0]
        .iter()
        .find(|r| r.cabal == "dawn" && r.festival_id == 0 && r.cycle == 4)
        .unwrap();
    assert_eq!(row.score, 10);
    assert_eq!(row.members, "Alpha,Bravo,Charlie,Delta,Echo");
    assert!(row.date > 0);
    assert_eq!(writes[0].len(), 10);
    // The status row goes with it (`saveFestivalData(true)`).
    assert!(out.iter().any(|o| matches!(
        o,
        Out::Persist(PersistCommand::SevenSigns(SevenSignsWrite::UpdateStatus(st))) if st.dawn_festival_score == 60
    )));

    // Dusk cannot beat what already counts: a better score of the leading cabal is refused.
    // NOTE(java-quirk): the leader of the second party of the same cabal gets the same answer.
    assert!(!s.world.festival_set_final_score(CABAL_DAWN, 0, 500));
}

/// With `CycleDatabaseWrites = False` (D-53) the score stays in memory and no festival row is
/// written; the tables are read again when shown (D-50) and show what the Java game server has.
#[test]
fn with_the_switch_off_nothing_global_is_written_and_the_tables_are_read_again() {
    let mut s = Scene::new(false, rows_of(4));
    s.party_of_five();
    s.bypass(P[0], "Festival 2 6360");
    s.wait_ms(1_000 + 80_000 + 5_000);
    let monsters = s.world.festival_monsters(CABAL_DAWN, 0);
    s.kill(monsters[0], P[1]);
    s.wait_ms(100_000 - 5_000 + 100);
    s.take();

    s.bypass(P[0], "Festival 3");
    let out = s.take();
    // The memory has the score, the database gets nothing.
    assert_eq!(s.world.festival_highest_score(CABAL_DAWN, 0), 5);
    assert!(festival_writes(&out).is_empty());
    assert!(
        !out.iter().any(|o| matches!(
            o,
            Out::Persist(PersistCommand::SevenSigns(SevenSignsWrite::UpdateStatus(_)))
        )),
        "the status row is global too"
    );

    // The score table (page 5) asks for the rows first.
    s.bypass(P[0], "Festival 5");
    let out = s.take();
    assert!(pages(&out, P[0]).is_empty(), "nothing is shown before the answer");
    let then = out
        .iter()
        .find_map(|o| match o {
            Out::Persist(PersistCommand::SevenSignsReadRequest { then }) => Some(then.clone()),
            _ => None,
        })
        .expect("a read request");
    assert!(matches!(then, SevenSignsThen::FestivalPage { page: 5, .. }));
    // The Java game server has a Dusk record of 77 in the lowest range and no Dawn record.
    let mut sieghardt = rows_of(4);
    sieghardt[0].score = 77;
    sieghardt[0].members = "Sieg".into();
    s.world.handle(WorldCommand::SevenSignsLoaded {
        result: Ok(SevenSignsLoad {
            festival: sieghardt,
            ..SevenSignsLoad::default()
        }),
        then,
    });
    let page = pages(&s.take(), P[0]).pop().expect("the table once it is read");
    // `getStatsTable`: Dusk 77, Dawn 0 (Kain's own 5 was replaced), "Children of Dusk" win.
    assert!(page.contains("Level 31 or lower"), "{page}");
    assert!(page.contains(">77<") && page.contains("Children of Dusk"), "{page}");
    assert_eq!(s.world.festival_highest_score(CABAL_DAWN, 0), 0);
    assert_eq!(s.world.festival_highest_score(CABAL_DUSK, 0), 77);
}

/// The accumulated bonus: the share of a winner's party, off the festival's bonus, in the seal
/// validation period (Java pays nothing out; only the page and the bonus change).
#[test]
fn the_winning_cabal_collects_its_share_of_the_bonus_in_the_validation_period() {
    let mut rows = rows_of(4);
    // Dawn's best party of the lowest range has two members, one of them Alpha.
    let dawn0 = rows.iter_mut().find(|r| r.cabal == "dawn" && r.festival_id == 0).unwrap();
    dawn0.score = 50;
    dawn0.members = "Alpha,Bravo".into();
    let status = SevenSignsStatus {
        current_cycle: 4,
        active_period: PERIOD_SEAL_VALIDATION,
        dawn_stone_score: 100.0,
        accumulated_bonus: [2_700, 0, 0, 0, 0],
        last_save: real_now() - 60_000,
        ..SevenSignsStatus::default()
    };
    let alpha = SignsPlayer {
        char_id: P[0],
        cabal: "dawn".into(),
        ..SignsPlayer::default()
    };
    let mut s = Scene::with(true, rows, status, vec![alpha]);
    s.enter(P[0], NAMES[0], Vec::new());
    s.enter(P[1], NAMES[1], Vec::new());
    for id in [P[0], P[1]] {
        for _ in 0..2 {
            s.input(
                id,
                ClientInput::Action {
                    object_id: s.guide.0,
                    action_id: 0,
                },
            );
            s.ticks(2);
        }
    }
    s.take();
    // The festival is closed in this period, so the guide's own pages say so.
    s.bypass(P[0], "Festival 1");
    assert!(pages(&s.take(), P[0]).pop().unwrap().contains("festival_2a"));
    // Alpha's name is in the best party of range 0 and his cabal leads: half of 2700.
    s.bypass(P[0], "Festival 0");
    assert!(pages(&s.take(), P[0]).pop().unwrap().contains("festival_0a"));
    assert_eq!(s.world.festival_accumulated_bonus(0), 1_350);
    // NOTE(java-quirk): nothing stops him asking again; each time takes half of the rest.
    s.bypass(P[0], "Festival 0");
    assert_eq!(s.world.festival_accumulated_bonus(0), 675);
    // Bravo is in the same party but belongs to no cabal (the winner is Dawn): nothing.
    s.bypass(P[1], "Festival 0");
    assert!(pages(&s.take(), P[1]).pop().unwrap().contains("festival_0b"));
    assert_eq!(s.world.festival_accumulated_bonus(0), 675);
}

/// The shout and the countdown of the NPC pages (`%festivalMins%`).
#[test]
fn the_pages_of_the_npcs_show_the_minutes_to_the_next_festival() {
    let mut s = Scene::new(true, rows_of(4));
    s.enter(P[0], NAMES[0], Vec::new());
    for _ in 0..2 {
        s.input(
            P[0],
            ClientInput::Action {
                object_id: s.guide.0,
                action_id: 0,
            },
        );
        s.ticks(2);
    }
    let page = pages(&s.take(), P[0]).pop().unwrap();
    assert!(page.contains("The next festival will begin in"), "{page}");
    assert!(page.contains("minute(s)"), "{page}");
    assert!(!page.contains("%festivalMins%"), "{page}");
}
