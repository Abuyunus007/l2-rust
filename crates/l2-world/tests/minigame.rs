#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The minigames (M8-GAMES-01): a Handy's Block Checker match, the Kratei's Cube registration and
//! match, and an Underground Coliseum match.
//! Java: managers/HandysBlockCheckerManager.java, managers/games/{BlockCheckerManager,
//! KrateisCubeManager,UndergroundColiseumManager}.java, mechanics/krateisCube/*,
//! mechanics/undergroundColiseum/*, entity/actor/instance/{Block,KrateisCubeManager,
//! KrateisMatchManager,UCManager,UCTower}.java.
//!
//! The scenes run on the real world with small synthetic data: the NPCs of the games, one arena of
//! each game in the XML shape of the shipped files, and the html pages the NPCs answer with. The
//! expected numbers are read off the Java sources.

use std::fs;
use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, HtmCache, Hennas, ItemTemplates, MinigameData, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{
    ExCubeGameEnd, ExPvpMatchCcMyRecord, ExPvpMatchCcRecord, ExPvpMatchRecord, ExShowScreenMessage,
    NpcHtmlMessage, SystemMessage, SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_util::WorldRng;
use l2_world::stats::{BaseStatBonus, StatData};
use l2_world::*;

// ---- the data ----------------------------------------------------------------------------------

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="13067" type="EtcItem" name="Coupon"><set name="is_stackable" val="true" /></item>
<item id="13787" type="EtcItem" name="Bond"><set name="is_stackable" val="true" /></item>
<item id="13788" type="EtcItem" name="Land Mine"><set name="is_stackable" val="true" /></item>
</list>"#;

const CLASS: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

fn npc_xml(id: i32, kind: &str, level: i32, hp: i32) -> String {
    format!(
        r#"<npc id="{id}" type="{kind}" level="{level}" name="N{id}">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="{hp}" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>"#
    )
}

fn npcs() -> String {
    let mut out = String::from("<list>");
    for (id, kind, level, hp) in [
        (18672, "Block", 1, 300),
        (18676, "Monster", 73, 300),
        (18601, "Monster", 70, 300),
        (18602, "Monster", 70, 300),
        (18539, "UCTower", 45, 3000),
        (32503, "KrateisCubeManager", 70, 3000),
        (32504, "KrateisMatchManager", 70, 3000),
        (32513, "UCManager", 70, 3000),
    ] {
        out.push_str(&npc_xml(id, kind, level, hp));
    }
    out.push_str("</list>");
    out
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

/// The coliseum arena of the scene (the shape of `UndergroundColiseum.xml`).
const COLISEUM_XML: &str = r#"<list>
<arena id="1" minLvl="40" maxLvl="49" curator="32513">
  <tower id="18539" x="-71000" y="258000" z="-3104" />
  <tower id="18539" x="-71600" y="258600" z="-3104" />
  <spawn x="-71100" y="258100" z="-3104" />
  <spawn x="-71200" y="258200" z="-3104" />
  <spawn x="-71300" y="258300" z="-3104" />
  <spawn x="-71400" y="258400" z="-3104" />
  <rewards><item id="-300" amount="20" useModifers="true" /></rewards>
</arena>
</list>"#;

/// The Kratei's Cube arena of the scene (the shape of `KrateisCube.xml`).
const KRATEIS_XML: &str = r#"<list>
<arena id="1" manager="32504" minLevel="70" maxLevel="75" minPlayers="2" maxPlayers="25">
  <waitLocations><point x="-70000" y="258000" z="-3104" /><point x="-70100" y="258000" z="-3104" /></waitLocations>
  <battleLocations><point x="-69000" y="259000" z="-3104" /><point x="-69100" y="259000" z="-3104" /></battleLocations>
  <watcherLocations><point x="-69500" y="259500" z="-3104" /></watcherLocations>
  <rewards><item id="13067" amount="1" useModifers="true" /></rewards>
  <add_parameters>
    <set name="minPoints" value="10" /><set name="pointPerPlayer" value="3" /><set name="pointPerMob" value="1" />
    <set name="expModifier" value="1800" /><set name="spModifier" value="180" />
    <set name="doorRotationTime" value="25" /><set name="watcherRotationTime" value="30" /><set name="respawnTime" value="10" />
  </add_parameters>
</arena>
</list>"#;

fn minigame_data() -> Arc<MinigameData> {
    // Both games in one value: the loader reads one document per game.
    let mut data = MinigameData::coliseum_from_xml(COLISEUM_XML).unwrap();
    let krateis = MinigameData::krateis_from_xml(KRATEIS_XML).unwrap();
    data.merge(krateis);
    Arc::new(data)
}

// ---- a recording world -------------------------------------------------------------------------

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
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

impl Persisted {
    fn writes(&self) -> Vec<MinigameWrite> {
        self.0
            .lock()
            .unwrap()
            .iter()
            .filter_map(|c| match c {
                PersistCommand::Minigame(w) => Some(w.clone()),
                _ => None,
            })
            .collect()
    }
}

struct RecOutbox(Persisted);

impl Outbox for RecOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }
    fn ls_event(&mut self, _event: LsEvent) {}
}

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
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.0.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
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

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn of(sent: &Sent, to: i32, opcode: ServerOpcode) -> Vec<&[u8]> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == opcode.name())
        .map(|(_, b)| b.as_slice())
        .collect()
}

fn messages(sent: &Sent, to: i32) -> Vec<SystemMessage> {
    of(sent, to, ServerOpcode::SystemMessage)
        .into_iter()
        .map(decode::<SystemMessage>)
        .collect()
}

fn message_ids(sent: &Sent, to: i32) -> Vec<SystemMessageId> {
    messages(sent, to).into_iter().map(|m| m.id).collect()
}

fn pages(sent: &Sent, to: i32) -> Vec<String> {
    of(sent, to, ServerOpcode::NpcHtmlMessage)
        .into_iter()
        .map(|b| {
            decode::<NpcHtmlMessage>(b)
                .html
                .replace("<html><body>", "")
                .replace("</body></html>", "")
        })
        .collect()
}

/// The sub ids of the `EX_BLOCK_UP_SET_LIST` packets one player got: 1 add, 2 remove, 3 time, 4
/// ready question, 5 team change, 0 the list, -1 close.
fn block_list_subs(sent: &Sent, to: i32) -> Vec<i32> {
    of(sent, to, ServerOpcode::ExBlockUpSetList)
        .into_iter()
        .map(|b| i32::from_le_bytes([b[3], b[4], b[5], b[6]]))
        .collect()
}

const CLOCK: i64 = 1_800_000_000_000;

fn player_data(id: i32, name: &str, level: i32) -> PlayerData {
    PlayerData {
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
        level,
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
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

struct Scene {
    world: World,
    log: Log,
    persisted: Persisted,
    _html: tempfile::TempDir,
}

/// What a scene is made of.
struct Config {
    cycle: bool,
    clock: i64,
    uc_party_size: i32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            cycle: false,
            clock: CLOCK,
            uc_party_size: 2,
        }
    }
}

fn write_page(root: &std::path::Path, folder: &str, file: &str, text: &str) {
    let dir = root.join("data/html").join(folder);
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join(file), text).unwrap();
}

impl Scene {
    fn new(config: Config) -> Self {
        let log = Log::default();
        let persisted = Persisted::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(RecOutbox(persisted.clone())));
        let mut settings = WorldSettings::default();
        // These tests are about the games, not about the penalty of dying or the vitality.
        settings.death.disconnect_after_death = false;
        settings.vitality.enable_vitality = false;
        // No client answers the teleports: the watchdog completes them after a second.
        settings.teleport.teleport_watchdog_timeout = 1;
        world.set_settings(settings);
        let stats = StatData {
            items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
            player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::from_xml(
                "<list></list>",
                &l2_data::skill::EnchantSkillGroups::default(),
                &l2_data::SkillLoadOptions::default(),
            )),
        };
        world.set_static_data(Box::new(Data(stats)));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(&npcs(), &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        let html = tempfile::tempdir().unwrap();
        for (file, text) in [
            ("32503-03.htm", "REGISTERED"),
            ("32503-04.htm", "ALREADY"),
            ("32503-05.htm", "CANCELED"),
            ("32503-06.htm", "WRONGLEVEL"),
            ("32503-07.htm", "CLOSED"),
        ] {
            write_page(html.path(), "krateisCube", file, text);
        }
        for (file, text) in [
            ("registered.htm", "UCREGISTERED"),
            ("noTeam.htm", "NOTEAM"),
            ("notStarted.htm", "NOTSTARTED"),
            ("notPartyLeader.htm", "NOTLEADER"),
            ("alreadyRegistered.htm", "UCALREADY"),
            ("wrongLevel.htm", "UCWRONGLEVEL %name%"),
            ("view-participating-teams.htm", "LIST %list%"),
        ] {
            write_page(html.path(), "undergroundColiseum", file, text);
        }
        world.set_html_cache(Arc::new(HtmCache::load(html.path(), false)));
        world.set_minigame_settings(MinigameSettings {
            enable_block_checker_event: true,
            min_block_checker_team_members: 2,
            uc_party_size: config.uc_party_size,
            cycle_database_writes: config.cycle,
            clock_override_unix_ms: Some(config.clock),
            ..MinigameSettings::default()
        });
        world.set_minigame_data(minigame_data());
        Self {
            world,
            log,
            persisted,
            _html: html,
        }
    }

    fn enter(&mut self, id: i32, name: &str, level: i32) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player_data(id, name, level)),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.log.take();
    }

    fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input,
        });
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn seconds(&mut self, s: u32) {
        self.ticks(s * 20);
    }

    fn player(&self, id: i32) -> &Player {
        self.world.player(ObjectId(id)).unwrap()
    }

    fn count(&self, id: i32, item_id: i32) -> i64 {
        self.player(id)
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id && i.loc == "INVENTORY")
            .map(|i| i.count)
            .sum()
    }

    /// `player` clicks a link of `npc`'s page: the page is shown first so the link is allowed.
    fn bypass(&mut self, player: i32, npc: ObjectId, command: &str) {
        let page = format!(r#"<html><body><a action="bypass -h npc_{}_{command}">x</a></body></html>"#, npc.0);
        self.world
            .send_html(ObjectId(player), &NpcHtmlMessage::new(npc.0, &page).unwrap());
        // The page that allows the link is not the answer.
        self.log.take();
        self.input(
            player,
            ClientInput::Bypass {
                command: format!("npc_{}_{command}", npc.0),
            },
        );
    }

    fn at(&self, id: i32) -> (i32, i32, i32) {
        self.world.position_of(ObjectId(id)).unwrap()
    }

    fn form_party(&mut self, leader: i32, leader_name: &str, members: &[(i32, &str)]) {
        let _ = leader_name;
        for (id, name) in members {
            self.input(
                leader,
                ClientInput::JoinParty {
                    name: (*name).to_owned(),
                    distribution_type_id: 0,
                },
            );
            self.input(*id, ClientInput::AnswerJoinParty { response: 1 });
        }
        self.log.take();
    }
}

// ---- Handy's Block Checker ---------------------------------------------------------------------

const P1: i32 = 0x2000_0001;
const P2: i32 = 0x2000_0002;
const P3: i32 = 0x2000_0003;
const P4: i32 = 0x2000_0004;
const P5: i32 = 0x2000_0005;
const P6: i32 = 0x2000_0006;

fn id(raw: i32) -> ObjectId {
    ObjectId(raw)
}

/// Four players join an arena: Java fills red first, then blue when it has fewer, so the order of
/// joining is red, blue, red, blue; with two in each team the ready question goes out.
#[test]
fn block_checker_lobby_fills_the_teams_and_asks_when_both_have_enough() {
    let mut s = Scene::new(Config::default());
    s.world.start_minigames(MinigameLoad::default());
    for (player, name) in [(P1, "A"), (P2, "B"), (P3, "C"), (P4, "D")] {
        s.enter(player, name, 20);
    }
    for player in [P1, P2, P3] {
        s.world.block_checker_first_talk(id(player), 0);
    }
    let view = s.world.block_arena(0).unwrap();
    assert_eq!(view.red, vec![id(P1), id(P3)]);
    assert_eq!(view.blue, vec![id(P2)]);
    // Three players: no question yet. P1 got the add packet of everybody who joined after him.
    let sent = s.log.take();
    assert!(!block_list_subs(&sent, P1).contains(&4));
    assert_eq!(
        block_list_subs(&sent, P1)[..2],
        [1, 0],
        "his own add packet, then the team list"
    );

    s.world.block_checker_first_talk(id(P4), 0);
    let sent = s.log.take();
    for player in [P1, P2, P3, P4] {
        let subs = block_list_subs(&sent, player);
        assert!(subs.contains(&4), "{player:x} is asked if he is ready");
        assert!(subs.contains(&3), "{player:x} hears the seconds to the start");
    }
    // Registered twice: "C1 is already registered".
    s.world.block_checker_first_talk(id(P1), 0);
    let sent = s.log.take();
    assert_eq!(
        message_ids(&sent, P1),
        vec![SystemMessageId::C1IsAlreadyRegisteredOnTheMatchWaitingList]
    );
    // An arena of another number does not know the player.
    assert!(s.world.block_arena(1).unwrap().red.is_empty());
}

/// The last player of a team leaves the waiting room: the event ends, the others are sent back to
/// Handy, and the one who left must wait 10 seconds before he registers again.
#[test]
fn block_checker_an_empty_team_ends_the_event_and_the_leaver_waits_ten_seconds() {
    let mut s = Scene::new(Config::default());
    s.world.start_minigames(MinigameLoad::default());
    s.enter(P1, "A", 20);
    s.enter(P2, "B", 20);
    s.world.block_checker_first_talk(id(P1), 0);
    s.world.block_checker_first_talk(id(P2), 0);
    s.log.take();

    // "RequestExCubeGameChangeTeam" with team -1 removes the player (the wire arena is -1).
    s.input(
        P2,
        ClientInput::Minigame(MinigameInput::CubeChangeTeam { arena: 0, team: -1 }),
    );
    let sent = s.log.take();
    assert!(block_list_subs(&sent, P1).contains(&2), "the team mate sees him go");
    // The blue team is empty, so `EndEvent` runs (one tick later): P1 is told the game ended.
    s.ticks(2);
    let sent = s.log.take();
    let end: ExCubeGameEnd = decode(of(&sent, P1, ServerOpcode::ExBlockUpSetState)[0]);
    assert!(!end.red_team_win, "Java keeps the old winner flag (false at first)");
    assert!(s.world.block_arena(0).unwrap().red.is_empty(), "the arena is emptied");
    assert!(!s.world.block_arena_is_being_used(0));

    // The penalty: 10 seconds.
    s.world.block_checker_first_talk(id(P2), 0);
    assert_eq!(
        message_ids(&s.log.take(), P2),
        vec![SystemMessageId::YouMustWait10SecondsBeforeAttemptingToRegisterAgain]
    );
    s.seconds(10);
    s.world.block_checker_first_talk(id(P2), 0);
    assert_eq!(s.world.block_arena(0).unwrap().red, vec![id(P2)]);
}

/// A whole match: the votes start it, 16 blocks come, players flip them for points, the rounds
/// add blocks, and at the end the red team (more points) is paid.
#[test]
fn block_checker_a_match_from_the_votes_to_the_rewards() {
    let mut s = Scene::new(Config::default());
    s.world.start_minigames(MinigameLoad::default());
    for (player, name) in [(P1, "A"), (P2, "B"), (P3, "C"), (P4, "D")] {
        s.enter(player, name, 20);
        s.world.block_checker_first_talk(id(player), 0);
    }
    s.log.take();

    // Two votes are not enough (the vote must be above half of the four players), the third
    // starts the event, one tick later.
    for (voter, expected_started) in [(P1, false), (P2, false), (P3, false)] {
        s.input(
            voter,
            ClientInput::Minigame(MinigameInput::CubeReadyAnswer { arena: 0, answer: 1 }),
        );
        assert_eq!(s.world.block_arena(0).unwrap().started, expected_started);
    }
    s.ticks(1);
    let view = s.world.block_arena(0).unwrap();
    assert!(view.started);
    assert!(s.world.block_arena_is_being_used(0));
    // The players are in their teams (2 red, 1 blue), in the arena, in the PvP zone.
    assert_eq!(s.world.team_of(id(P1)), 2);
    assert_eq!(s.world.team_of(id(P2)), 1);
    assert_eq!(s.world.block_checker_arena_of(id(P1)), 0);
    assert_ne!(s.at(P1), s.at(P2), "each team starts at its own corner");
    let sent = s.log.take();
    // ExCubeGameChangePoints: time 300, blue 0, red 0 (no block yet), sent to all four.
    assert!(!of(&sent, P1, ServerOpcode::ExBlockUpSetState).is_empty());
    assert!(block_list_subs(&sent, P1).contains(&-1), "the lobby window closes");

    // The first round: 16 blocks, 8 more points for each team; the girl comes too.
    s.ticks(1);
    let view = s.world.block_arena(0).unwrap();
    assert_eq!(view.blocks.len(), 16);
    assert_eq!((view.red_points, view.blue_points), (8, 8));
    // Blocks alternate red, blue, red, ...
    let colors: Vec<i32> = view.blocks.iter().map(|b| s.world.block_color_of(*b)).collect();
    assert_eq!(&colors[..4], &[0x53, 0, 0x53, 0]);

    // P1 (red) turns the blue blocks red: each flip is a point for him and the team.
    let blue_blocks: Vec<ObjectId> = view
        .blocks
        .iter()
        .copied()
        .filter(|b| s.world.block_color_of(*b) == 0)
        .collect();
    for block in &blue_blocks[..3] {
        s.world.block_checker_flip_block(id(P1), *block);
        assert_eq!(s.world.block_color_of(*block), 0x53);
    }
    let view = s.world.block_arena(0).unwrap();
    assert_eq!((view.red_points, view.blue_points), (11, 5));
    assert_eq!(s.world.block_player_points(0, id(P1), true), 3);
    // A red player cannot flip a red block (the effect checks the colour).
    s.world.block_checker_flip_block(id(P1), blue_blocks[0]);
    assert_eq!(s.world.block_arena(0).unwrap().red_points, 11);
    // Blocks may be hit by players of the arena only.
    s.enter(P5, "Outsider", 20);
    assert!(s.world.is_auto_attackable_by(blue_blocks[1], id(P1)));
    assert!(!s.world.is_auto_attackable_by(blue_blocks[1], id(P5)));

    // The second round, a minute after the first: 20 more blocks (the points get 10 each).
    s.seconds(60);
    let view = s.world.block_arena(0).unwrap();
    assert_eq!(view.blocks.len(), 36);
    assert_eq!((view.red_points, view.blue_points), (21, 15));
    // P1 gets to 10 points: 7 more flips of blue blocks.
    let more: Vec<ObjectId> = view
        .blocks
        .iter()
        .copied()
        .filter(|b| s.world.block_color_of(*b) == 0)
        .take(7)
        .collect();
    for block in more {
        s.world.block_checker_flip_block(id(P1), block);
    }
    assert_eq!(s.world.block_player_points(0, id(P1), true), 10);
    // P3 (red) gets 1 point only.
    let one = s
        .world
        .block_arena(0)
        .unwrap()
        .blocks
        .into_iter()
        .find(|b| s.world.block_color_of(*b) == 0)
        .unwrap();
    s.world.block_checker_flip_block(id(P3), one);
    s.log.take();

    // The third round (14 blocks), then three minutes to the end.
    s.seconds(60);
    assert_eq!(s.world.block_arena(0).unwrap().blocks.len(), 50);
    s.seconds(180);
    s.ticks(2);
    let sent = s.log.take();

    // Red won: P1 has 10 points (2 coupons, and the 8 for the best), P3 has 1 (nothing).
    assert_eq!(s.count(P1, 13067), 10);
    assert_eq!(s.count(P3, 13067), 0);
    assert_eq!(s.count(P2, 13067), 0);
    let end: ExCubeGameEnd = decode(of(&sent, P2, ServerOpcode::ExBlockUpSetState).last().unwrap());
    assert!(end.red_team_win);
    let won = messages(&sent, P2)
        .into_iter()
        .find(|m| m.id == SystemMessageId::TheC1TeamHasWon)
        .unwrap();
    assert_eq!(won.params, vec![SystemMessageParam::Text("Red Team".to_owned())]);
    // Everything is cleaned: blocks gone, players back, the arena free again.
    let view = s.world.block_arena(0).unwrap();
    assert!(view.blocks.is_empty() && view.red.is_empty() && view.blue.is_empty());
    assert!(!view.started);
    assert!(!s.world.block_arena_is_being_used(0));
    assert_eq!(s.world.team_of(id(P1)), 0);
    assert_eq!(s.world.block_checker_arena_of(id(P1)), -1);
    assert_eq!(s.at(P1).0, -57_478, "everybody is sent back to Handy");
    assert!(s.world.ground_item_ids().is_empty(), "the dropped event items go");
}

/// A tie pays nothing (Java returns before the rewards).
#[test]
fn block_checker_a_tie_pays_nothing() {
    let mut s = Scene::new(Config::default());
    s.world.start_minigames(MinigameLoad::default());
    for (player, name) in [(P1, "A"), (P2, "B")] {
        s.enter(player, name, 20);
        s.world.block_checker_first_talk(id(player), 0);
    }
    s.input(
        P1,
        ClientInput::Minigame(MinigameInput::CubeReadyAnswer { arena: 0, answer: 1 }),
    );
    s.input(
        P2,
        ClientInput::Minigame(MinigameInput::CubeReadyAnswer { arena: 0, answer: 1 }),
    );
    // One vote is above half of two players: the second vote is already late.
    s.seconds(10);
    assert!(s.world.block_arena(0).unwrap().started);
    s.seconds(300);
    assert_eq!(s.count(P1, 13067) + s.count(P2, 13067), 0);
    assert!(!s.world.block_arena_is_being_used(0));
}

// ---- Kratei's Cube ------------------------------------------------------------------------------

/// The registration, the cycle (close, prepare, start), the points, the respawn and the rewards.
#[test]
fn krateis_cube_registration_and_a_match() {
    // 10 minutes past the hour: the next registration closes at minute 27.
    let clock = CLOCK + 10 * 60_000;
    let mut s = Scene::new(Config {
        clock,
        ..Config::default()
    });
    let entrance = s.world.spawn_npc_at(32503, X, Y, Z, 0).unwrap();
    let _match_manager = s.world.spawn_npc_at(32504, X, Y, Z, 0).unwrap();
    s.enter(P1, "A", 72);
    s.enter(P2, "B", 72);
    s.enter(P3, "Low", 60);
    s.world.start_minigames(MinigameLoad::default());
    assert!(s.world.krateis_is_register_time());
    assert_eq!(s.world.krateis_next_match_time(), clock + 17 * 60_000);
    s.log.take();

    // A level 60 player is refused by the arena of levels 70 to 75; a level 72 one registers.
    s.bypass(P3, entrance, "Register 1");
    assert_eq!(pages(&s.log.take(), P3), vec!["WRONGLEVEL"]);
    s.bypass(P1, entrance, "Register 1");
    assert_eq!(pages(&s.log.take(), P1), vec!["REGISTERED"]);
    assert!(s.world.is_registered_on_event(id(P1)));
    assert_eq!(s.world.krateis_arena_of(id(P1)), Some(1));
    s.bypass(P1, entrance, "Register 1");
    assert_eq!(pages(&s.log.take(), P1), vec!["ALREADY"]);
    // Cancel and register again.
    s.bypass(P1, entrance, "Cancel");
    assert_eq!(pages(&s.log.take(), P1), vec!["CANCELED"]);
    assert!(!s.world.is_registered_on_event(id(P1)));
    s.bypass(P1, entrance, "Register 1");
    s.bypass(P2, entrance, "Register 1");
    s.log.take();
    // A registered player may not log out ("A superior power doesn't allow you to leave.").
    s.input(P1, ClientInput::Logout);
    assert!(s.world.player(id(P1)).is_some());

    // 17 minutes later registration closes; a new registration says it is closed.
    s.seconds(17 * 60);
    assert!(!s.world.krateis_is_register_time());
    assert_eq!(s.world.krateis_state(), KrateisState::Preparing);
    s.bypass(P3, entrance, "Register 1");
    // (P3's level is wrong, which is answered first.)
    assert_eq!(pages(&s.log.take(), P3), vec!["WRONGLEVEL"]);

    // Three minutes later the players near the entrance are put into the waiting room.
    s.seconds(3 * 60);
    assert_eq!(s.world.krateis_arena_flags(1), (true, false));
    let wait = [(-70000, 258000), (-70100, 258000)];
    for player in [P1, P2] {
        let (x, y, _) = s.at(player);
        assert!(
            wait.iter().any(|(wx, wy)| (x - wx).abs() <= 100 && (y - wy).abs() <= 100),
            "{player:x} is in the waiting room at {x},{y}"
        );
    }
    // The countdown: 10 messages, one a second.
    s.seconds(1);
    assert!(!message_ids(&s.log.take(), P1).is_empty());

    // 11 seconds after that the match starts: the players are in the arena with full vitals.
    s.seconds(11);
    assert_eq!(s.world.krateis_arena_flags(1), (true, true));
    assert_eq!(s.world.krateis_state(), KrateisState::Started);
    let (x, y, _) = s.at(P1);
    assert!((x + 69_000).abs() <= 200 && (y - 259_000).abs() <= 200, "P1 is in the arena");
    assert!(!s.world.can_revive(id(P1)), "nobody can ask for a resurrection in the match");
    let sent = s.log.take();
    let record: ExPvpMatchCcRecord = decode(of(&sent, P1, ServerOpcode::ExPvpMatchCcRecord)[0]);
    assert_eq!(record.players.len(), 2);
    let mine: ExPvpMatchCcMyRecord = decode(of(&sent, P1, ServerOpcode::ExPvpMatchCcMyRecord)[0]);
    assert_eq!(mine.points, 0);

    // The teleport of the start is over (a player who is still teleporting takes no damage).
    s.seconds(5);
    // P2 kills P1: 3 points for the killer; P1 is back in the waiting room after 10 seconds.
    s.world.reduce_current_hp(id(P1), 1_000_000.0, Some(id(P2)));
    assert!(s.world.is_dead(id(P1)));
    assert_eq!(s.world.krateis_points_of(1, id(P2)), 3);
    s.seconds(10);
    s.ticks(2);
    assert!(!s.world.is_dead(id(P1)), "revived in the waiting room");
    let (x, _, _) = s.at(P1);
    assert!((x + 70_000).abs() <= 200 || (x + 70_100).abs() <= 200);

    // P2 earns enough points: 14 kills of 3 are 42 points.
    for _ in 0..13 {
        s.world.krateis_add_points(id(P2), true);
    }
    assert_eq!(s.world.krateis_points_of(1, id(P2)), 42);
    s.log.take();

    // 20 minutes after the start the match ends: ranks pay by points (P2 first, with 42 >= 10).
    s.seconds(20 * 60);
    assert_eq!(s.world.krateis_arena_flags(1), (false, false));
    assert_eq!(s.world.krateis_state(), KrateisState::Registration);
    // count = (int) (42 * 0.05 * (1 + (2 / 1) * 0.04)) = 2: the reward item "useModifers" is 2.
    assert_eq!(s.count(P2, 13067), 2);
    assert_eq!(s.count(P1, 13067), 0, "P1 has no points");
    assert!(!s.world.is_registered_on_event(id(P2)));
    assert_eq!(s.world.krateis_arena_of(id(P2)), None);
    assert!(s.world.can_revive(id(P2)));
    let (x, y, _) = s.at(P2);
    assert!((x + 70_381).abs() <= 200 && (y + 70_937).abs() <= 200, "back at Fantasy Isle");
    let sent = s.log.take();
    assert!(!of(&sent, P2, ServerOpcode::ExPvpMatchCcRetire).is_empty());
    // The watchers of the match are gone.
    assert!(s.world.npc_ids().into_iter().all(|n| {
        let npc = s.world.npc(n).unwrap();
        npc.decayed || (npc.npc_id != 18601 && npc.npc_id != 18602)
    }));
}

// ---- Underground Coliseum -------------------------------------------------------------------------

/// A coliseum scene: the period is on, the curator stands next to the players.
fn coliseum_scene() -> (Scene, ObjectId) {
    let mut s = Scene::new(Config::default());
    let curator = s.world.spawn_npc_at(32513, X, Y, Z, 0).unwrap();
    for (player, name) in [(P1, "A1"), (P2, "A2"), (P3, "B1"), (P4, "B2")] {
        s.enter(player, name, 45);
        // A third class: the coliseum asks for it.
        s.world.player_mut(id(player)).unwrap().data.class_id = 88;
    }
    s.form_party(P1, "A1", &[(P2, "A2")]);
    s.form_party(P3, "B1", &[(P4, "B2")]);
    s.world.start_minigames(MinigameLoad {
        uc_start_ms: CLOCK - 1000,
        uc_stop_ms: CLOCK + 2 * 3_600_000,
        best_teams: vec![],
    });
    (s, curator)
}

#[test]
fn coliseum_registration_answers() {
    // Before the period: "not started".
    let mut s = Scene::new(Config::default());
    let curator = s.world.spawn_npc_at(32513, X, Y, Z, 0).unwrap();
    s.enter(P1, "A1", 45);
    s.enter(P2, "A2", 45);
    s.world.start_minigames(MinigameLoad::default());
    assert!(!s.world.coliseum_is_started());
    s.bypass(P1, curator, "register 1");
    assert_eq!(pages(&s.log.take(), P1), vec!["NOTEAM"]);

    let (mut s, curator) = coliseum_scene();
    assert!(s.world.coliseum_is_started());
    // A member is not a leader.
    s.bypass(P2, curator, "register 1");
    assert_eq!(pages(&s.log.take(), P2), vec!["NOTLEADER"]);
    // A member of the wrong class level is named.
    s.world.player_mut(id(P2)).unwrap().data.class_id = 0;
    s.bypass(P1, curator, "register 1");
    assert_eq!(pages(&s.log.take(), P1), vec!["UCWRONGLEVEL A2"]);
    s.world.player_mut(id(P2)).unwrap().data.class_id = 88;
    // Registered; again is "already registered"; the list shows the party.
    s.bypass(P1, curator, "register 1");
    assert_eq!(pages(&s.log.take(), P1), vec!["UCREGISTERED"]);
    assert!(s.world.is_registered_on_event(id(P1)) || s.world.coliseum_waiting(1).len() == 1);
    s.bypass(P1, curator, "register 1");
    assert_eq!(pages(&s.log.take(), P1), vec!["UCALREADY"]);
    s.bypass(P1, curator, "listTeams 1");
    let page = pages(&s.log.take(), P1).remove(0);
    assert!(page.contains("1. (Participating Team: <font color=00ffff>A1;A2;</font>)<br>"), "{page}");
    assert!(page.contains("2. (Participating Team: Team)<br>"), "{page}");
    // Cancel takes the party out of the list.
    s.bypass(P1, curator, "cancel");
    assert!(s.world.coliseum_waiting(1).is_empty());
    // A registered party cannot leave the party in the middle of a match (see the match test).
}

#[test]
fn coliseum_a_match_from_the_registration_to_the_reward() {
    let (mut s, curator) = coliseum_scene();
    s.bypass(P1, curator, "register 1");
    s.bypass(P3, curator, "register 1");
    s.log.take();
    // The second party starts the match task at once (one tick), the countdown takes a minute.
    s.ticks(2);
    assert!(s.world.coliseum_is_battle_now(1));
    let team_a = s.world.coliseum_team(1, 0).unwrap();
    let team_b = s.world.coliseum_team(1, 1).unwrap();
    assert_eq!(team_a.party, s.world.party_of(id(P1)));
    assert_eq!(team_b.party, s.world.party_of(id(P3)));
    let sent = s.log.take();
    let first: ExShowScreenMessage = decode(of(&sent, P1, ServerOpcode::ExShowScreenMessage)[0]);
    assert_eq!(first.params, vec!["1".to_owned()]);
    s.seconds(30);
    s.seconds(31);
    let sent = s.log.take();
    // 8 countdown messages in all (the first one was taken above): 7 more.
    assert_eq!(of(&sent, P1, ServerOpcode::ExShowScreenMessage).len(), 7);

    // The teams are split over the start points; towers stand; the colours are set.
    assert_eq!(s.world.uc_state_of(id(P1)), UC_STATE_POINT);
    assert!((s.at(P1).0 + 71_100).abs() <= 100, "the leader of team 1 is at the first point");
    assert!((s.at(P2).0 + 71_200).abs() <= 100, "the next member at the second");
    assert_eq!(s.world.team_of(id(P1)), 2, "team 0 is red");
    assert_eq!(s.world.team_of(id(P3)), 1, "team 1 is blue");
    let tower_a = s.world.coliseum_team(1, 0).unwrap().tower.unwrap();
    let tower_b = s.world.coliseum_team(1, 1).unwrap().tower.unwrap();
    assert!(s.world.is_auto_attackable_by(tower_b, id(P1)));
    assert!(s.world.is_registered_on_event(id(P1)));
    // `getTeleToLocation` has no answer in the coliseum.
    assert_eq!(s.world.get_tele_to_location(id(P1), TeleportWhereType::Town), None);
    // The record at the start: both lists.
    let record = of(&sent, P1, ServerOpcode::ExPvpMatchRecord)
        .into_iter()
        .map(decode::<ExPvpMatchRecord>)
        .collect::<Vec<_>>();
    assert_eq!(record[0].record_type, 0);
    assert_eq!(record[0].blue.len(), 2);

    // The teleport of the start is over.
    s.seconds(3);
    // Nobody can leave the party in the middle of the match.
    s.input(P3, ClientInput::WithdrawParty);
    assert!(s.world.party_of(id(P3)).is_some());

    // P1 kills P3: one kill for team 0, the point's doors open (none here), P3 comes back after
    // the resurrection time (10 s) at his tower.
    s.world.reduce_current_hp(id(P3), 1_000_000.0, Some(id(P1)));
    assert_eq!(s.world.coliseum_team(1, 0).unwrap().kill_count, 1);
    assert_eq!(s.world.uc_kills_deaths_of(id(P1)), (1, 0));
    assert_eq!(s.world.uc_kills_deaths_of(id(P3)), (0, 1));
    assert_eq!(s.world.uc_state_of(id(P3)), UC_STATE_ARENA);
    s.seconds(10);
    s.ticks(2);
    assert!(!s.world.is_dead(id(P3)));
    assert!((s.at(P3).0 + 71_600).abs() <= 200, "next to his own tower");
    // His teleport back is over (a teleporting player takes no damage).
    s.seconds(2);

    // A hit of the tower's own colour does nothing; the other team destroys it.
    s.world.reduce_current_hp(tower_b, 1.0e9, Some(id(P4)));
    assert!(s.world.coliseum_team(1, 1).unwrap().tower.is_some(), "own team: no damage");
    s.world.reduce_current_hp(tower_b, 1.0e9, Some(id(P1)));
    assert!(s.world.coliseum_team(1, 1).unwrap().tower.is_none(), "the tower is down");
    assert!(s.world.npc(tower_b).is_none() || s.world.npc(tower_b).unwrap().decayed);

    // Without a tower the dead do not come back; when all of team 1 is dead team 0 wins at once.
    s.world.reduce_current_hp(id(P3), 1_000_000.0, Some(id(P1)));
    assert_eq!(s.world.coliseum_team(1, 1).unwrap().status, 0);
    s.world.reduce_current_hp(id(P4), 1_000_000.0, Some(id(P1)));
    let sent = s.log.take();
    assert!(message_ids(&sent, P1).contains(&SystemMessageId::TheBlueTeamIsVictorious));
    // The winner is paid 20 fame (the modifier of a first win is 1.0), with Java's "reputation"
    // message.
    assert_eq!(s.player(P1).data.profile.fame, 20);
    assert_eq!(s.player(P2).data.profile.fame, 20);
    assert_eq!(s.player(P3).data.profile.fame, 0);
    assert!(message_ids(&sent, P1).contains(&SystemMessageId::YouHaveAcquiredS1Reputation));
    let _ = tower_a;

    // The task runs at once: teams removed, everybody back and alive, the arena free.
    s.ticks(3);
    assert!(!s.world.coliseum_is_battle_now(1));
    for player in [P1, P2, P3, P4] {
        assert_eq!(s.world.uc_state_of(id(player)), UC_STATE_NONE);
        assert_eq!(s.world.team_of(id(player)), 0);
        assert!(!s.world.is_registered_on_event(id(player)));
        assert!(!s.world.is_dead(id(player)));
    }
    assert!(s.world.coliseum_team(1, 0).unwrap().party.is_none());
    assert!(s.world.coliseum_team(1, 1).unwrap().party.is_none());
    assert_eq!(s.world.uc_party_state(s.world.party_of(id(P1)).unwrap()), None);
    // The record of the end names team 1 (the red... blue index 0 + 1) as the winner.
    let sent = s.log.take();
    let finish = of(&sent, P1, ServerOpcode::ExPvpMatchRecord)
        .into_iter()
        .map(decode::<ExPvpMatchRecord>)
        .find(|r| r.record_type == 2)
        .unwrap();
    assert_eq!(finish.winner_team, 1);
    // The kills of team 0: the three kills of P1 (the counts go with `cleanUp`, after the record).
    assert_eq!((finish.blue_kills, finish.red_kills), (3, 0));
}

#[test]
fn coliseum_period_rows_are_written_only_with_the_cycle_switch() {
    // No period in the database: Java makes the next one from the cron pattern and stores it.
    for cycle in [false, true] {
        let mut s = Scene::new(Config {
            cycle,
            ..Config::default()
        });
        s.world.start_minigames(MinigameLoad::default());
        let writes = s.persisted.writes();
        if cycle {
            let names: Vec<&str> = writes
                .iter()
                .filter_map(|w| match w {
                    MinigameWrite::GlobalVariable { name, .. } => Some(name.as_str()),
                    _ => None,
                })
                .collect();
            assert_eq!(names, vec!["UC_START_TIME", "UC_STOP_TIME"]);
            let (start, end) = s.world.coliseum_period();
            assert!(start > CLOCK);
            assert_eq!(end - start, 2 * 3_600_000, "BattlePeriod is 2 hours");
        } else {
            assert!(writes.is_empty(), "side by side: nothing is written ({writes:?})");
            // The period is still kept in memory.
            assert!(s.world.coliseum_period().0 > CLOCK);
        }
    }
}

#[test]
fn coliseum_the_period_starts_and_ends_on_the_wheel() {
    let mut s = Scene::new(Config::default());
    s.world.start_minigames(MinigameLoad {
        uc_start_ms: CLOCK + 60_000,
        uc_stop_ms: CLOCK + 120_000,
        best_teams: vec![(1, "Leader".to_owned(), 4)],
    });
    assert!(!s.world.coliseum_is_started());
    assert_eq!(s.world.coliseum_best_team(1), Some(("Leader".to_owned(), 4)));
    s.seconds(61);
    assert!(s.world.coliseum_is_started());
    s.seconds(60);
    assert!(!s.world.coliseum_is_started());
    // The next period is generated from the pattern (a new start in the future).
    assert!(s.world.coliseum_period().0 > CLOCK + 120_000);
}

// A free-standing check of the id helper used above.
#[test]
fn ids_are_object_ids() {
    assert_eq!(id(P6).0, 0x2000_0006);
}
