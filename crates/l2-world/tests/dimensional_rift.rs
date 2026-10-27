#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The Dimensional Rift (M8-FEST-01): a party pays the dimensional fragments at the gatekeeper,
//! is taken to a room, jumps by the chance card and by the clock, and leaves. The expected values
//! are read off the Java sources (`DimensionalRiftManager.java`, `DimensionalRift.java`,
//! `DimensionalRiftRoom.java`, `Rift.java`, `RequestWithDrawalParty.java`), not off the code.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, RiftSpawns, SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::NpcHtmlMessage;
use l2_world::sevensigns::{RiftRoomDef, RiftSettings, rift_bypass_handler};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

/// The packets sent so far: the receiving player and the bytes.
type Sent = Vec<(i32, Vec<u8>)>;

impl Log {
    fn take(&self) -> Vec<(i32, Vec<u8>)> {
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

struct Recorder;

impl Outbox for Recorder {
    fn persist(&mut self, _command: PersistCommand) {}
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

const FRAGMENT: i32 = 7079;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="7079" type="EtcItem" name="Dimensional Fragment"><set name="is_stackable" val="true" /></item>
</list>"#;

const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>"#;

const GATEKEEPER: i32 = 31488;
const INVADER: i32 = 21659;

const LINKS: &str = r#"<a action="bypass -h npc_%objectId%_EnterRift 1">a</a><a action="bypass -h npc_%objectId%_ChangeRiftRoom">b</a><a action="bypass -h npc_%objectId%_ExitRift">c</a>"#;

/// The waiting room's middle (its teleport target).
const WAIT: (i32, i32, i32) = (-114_790, -180_576, -6_752);
const ALICE: i32 = 0x1000_0001;
const BOB: i32 = 0x1000_0002;
const CARL: i32 = 0x1000_0003;

fn room(
    kind: i8,
    room: i8,
    x: (i32, i32),
    y: (i32, i32),
    target: (i32, i32, i32),
    boss: bool,
) -> RiftRoomDef {
    RiftRoomDef {
        kind,
        room,
        x_min: x.0,
        x_max: x.1,
        y_min: y.0,
        y_max: y.1,
        z_min: -6816,
        z_max: -6240,
        x_t: target.0,
        y_t: target.1,
        z_t: target.2,
        boss,
    }
}

/// The waiting room and the rift zone of the table, and four rooms of the recruit area (the
/// numbers and sizes of the real rows 1 to 4).
fn rooms() -> Vec<RiftRoomDef> {
    vec![
        room(0, 0, (-115_440, -114_160), (-182_160, -178_048), WAIT, false),
        room(0, 1, (-120_576, -109_024), (-187_328, -172_880), (0, 0, 0), false),
        room(
            1,
            1,
            (-112_368, -111_584),
            (-182_832, -182_064),
            (-111_976, -182_448, -6_752),
            false,
        ),
        room(
            1,
            2,
            (-111_472, -110_704),
            (-182_832, -182_064),
            (-111_088, -182_448, -6_752),
            false,
        ),
        room(
            1,
            3,
            (-110_688, -109_904),
            (-180_944, -180_176),
            (-110_296, -180_560, -6_752),
            true,
        ),
        room(
            1,
            4,
            (-109_808, -109_024),
            (-180_944, -180_176),
            (-109_416, -180_560, -6_752),
            false,
        ),
    ]
}

const SPAWNS: &str = r#"<rift>
<area type="1">
<room id="1"><spawn mobId="21659" count="2" delay="90" /></room>
<room id="2"><spawn mobId="21659" count="1" delay="80" /></room>
<room id="3"><spawn mobId="21659" count="3" delay="86" /></room>
<room id="4"><spawn mobId="21659" count="1" delay="101" /></room>
</area>
</rift>"#;

fn settings() -> RiftSettings {
    RiftSettings {
        min_party_size: 2,
        max_jumps: 3,
        spawn_delay: 2_000,
        auto_jumps_time_min: 10,
        auto_jumps_time_max: 10,
        boss_room_time_multiply: 2.0,
        enter_cost: [18, 21, 24, 27, 30, 33],
    }
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

struct Scene {
    world: World,
    log: Log,
    gatekeeper: ObjectId,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new() -> Self {
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
            &format!("data/html/default/{GATEKEEPER}.htm"),
            &format!("<html><body>gatekeeper {LINKS}</body></html>"),
        );
        for page in [
            "NoParty",
            "NotPartyLeader",
            "SmallParty",
            "NotInWaitingRoom",
            "NoFragments",
            "AlreadyTeleported",
            "Cheater",
        ] {
            write(
                &format!("data/html/seven_signs/rift/{page}.htm"),
                &format!("<html><body>{page} %npc_name% %count%</body></html>"),
            );
        }
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(Recorder));
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
<npc id="{GATEKEEPER}" type="Folk" level="70" name="Rift Gatekeeper">{STATS}</npc>
<npc id="{INVADER}" type="RiftInvader" level="30" name="Otherworldly Invader Soldier">{STATS}</npc>
</list>"#
                ),
                &NpcLoadOptions::default(),
            )
            .unwrap(),
        );
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_rift_settings(settings());
        world.load_dimensional_rift(rooms(), &RiftSpawns::from_xml(SPAWNS).unwrap());
        let gatekeeper = world.spawn_npc_at(GATEKEEPER, WAIT.0, WAIT.1, WAIT.2, 0).unwrap();
        let s = Self {
            world,
            log,
            gatekeeper,
            _root: root,
        };
        s.log.take();
        s
    }

    fn enter(&mut self, id: i32, name: &str, items: Vec<(i32, i64)>) {
        self.enter_at(id, name, items, WAIT);
    }

    fn enter_at(&mut self, id: i32, name: &str, items: Vec<(i32, i64)>, at: (i32, i32, i32)) {
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
            x: at.0,
            y: at.1,
            z: at.2,
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

    /// Alice (the leader) and Bob in one party, Alice at the gatekeeper's page.
    fn party(&mut self, fragments: i64) {
        self.enter(ALICE, "Alice", vec![(FRAGMENT, fragments)]);
        self.enter(BOB, "Bob", vec![(FRAGMENT, fragments)]);
        self.input(
            ALICE,
            ClientInput::JoinParty {
                name: "Bob".into(),
                distribution_type_id: 0,
            },
        );
        self.input(BOB, ClientInput::AnswerJoinParty { response: 1 });
        for _ in 0..2 {
            self.input(
                ALICE,
                ClientInput::Action {
                    object_id: self.gatekeeper.0,
                    action_id: 0,
                },
            );
            self.ticks(2);
        }
        self.log.take();
    }

    /// Talks to the gatekeeper (two clicks), which shows the page with the links.
    fn talk(&mut self, id: i32) {
        for _ in 0..2 {
            self.input(
                id,
                ClientInput::Action {
                    object_id: self.gatekeeper.0,
                    action_id: 0,
                },
            );
            self.ticks(2);
        }
    }

    /// A link of the gatekeeper's page. The answer pages have no links, so every command comes
    /// after a fresh `talk`.
    fn bypass(&mut self, id: i32, command: &str) {
        self.talk(id);
        self.log.take();
        let command = format!("npc_{}_{command}", self.gatekeeper.0);
        self.input(id, ClientInput::Bypass { command });
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn wait_ms(&mut self, ms: u32) {
        self.ticks(ms.div_ceil(50));
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

    fn party_id(&self) -> PartyId {
        self.world.party_of(ObjectId(ALICE)).unwrap()
    }

    /// The room a party stands in.
    fn current_room(&self) -> i8 {
        self.world.rift_current_room(self.party_id()).unwrap()
    }

    /// The rooms of the recruit area that have a party.
    fn rooms_inside(&self) -> Vec<i8> {
        (1..=4)
            .filter(|r| self.world.rift_room_party_inside(1, *r))
            .collect()
    }

    /// The clients load the new place.
    fn appear(&mut self, ids: &[i32]) {
        for id in ids {
            self.input(*id, ClientInput::Appearing);
        }
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

fn pages(out: &[(i32, Vec<u8>)], to: i32) -> Vec<String> {
    out.iter()
        .filter(|(id, b)| *id == to && name_of(b) == "NPC_HTML_MESSAGE")
        .map(|(_, b)| decode::<NpcHtmlMessage>(b).html)
        .collect()
}

// ---------------------------------------------------------------------------------- tests

/// The rooms of the table and the spawns of the file: `count` monsters per `<spawn>`, in the
/// room's inner area (128 less on every side), at the teleport height.
#[test]
fn the_rooms_and_their_monsters_are_loaded() {
    let s = Scene::new();
    assert_eq!(s.world.rift_room_spawn_count(1, 1), 2);
    assert_eq!(s.world.rift_room_spawn_count(1, 3), 3);
    assert_eq!(s.world.rift_room_spawn_count(0, 0), 0);
    // Nobody is inside yet and no monster stands in a room.
    assert!(s.rooms_inside().is_empty());
    assert!(s.world.rift_room_monsters(1, 1).is_empty());
    // The waiting room is the peace zone, the rest of the rift zone is not.
    assert!(s.world.rift_check_in_peace_zone(WAIT.0, WAIT.1, WAIT.2));
    assert!(!s.world.rift_check_in_peace_zone(-111_976, -182_448, -6_752));
    assert!(s.world.rift_check_in_rift_zone(-111_976, -182_448, -6_752, false));
    assert!(!s.world.rift_check_in_rift_zone(WAIT.0, WAIT.1, WAIT.2, false));
    assert!(s.world.rift_check_in_rift_zone(WAIT.0, WAIT.1, WAIT.2, true));
    // The right and the bottom edge of a room are outside (`Polygon.contains`).
    assert!(s.world.rift_room_contains(1, 1, -112_368, -182_832, -6_752));
    assert!(!s.world.rift_room_contains(1, 1, -111_584, -182_448, -6_752));
    assert!(!s.world.rift_room_contains(1, 1, -112_000, -182_064, -6_752));
}

/// The refusals of `DimensionalRiftManager.start`, in Java's order.
#[test]
fn a_party_is_turned_away_until_it_can_pay() {
    let mut s = Scene::new();
    s.enter(ALICE, "Alice", vec![(FRAGMENT, 100)]);
    // No party.
    s.bypass(ALICE, "EnterRift 1");
    let page = pages(&s.log.take(), ALICE).pop().unwrap();
    assert!(
        page.contains("NoParty") && page.contains("Rift Gatekeeper"),
        "{page}"
    );

    s.enter(BOB, "Bob", Vec::new());
    s.input(
        ALICE,
        ClientInput::JoinParty {
            name: "Bob".into(),
            distribution_type_id: 0,
        },
    );
    s.input(BOB, ClientInput::AnswerJoinParty { response: 1 });
    s.log.take();
    // Bob has no fragments: the page names the price, nobody pays and nobody moves.
    s.bypass(ALICE, "EnterRift 1");
    let page = pages(&s.log.take(), ALICE).pop().unwrap();
    assert!(page.contains("NoFragments") && page.contains("18"), "{page}");
    assert_eq!(s.count(ALICE, FRAGMENT), 100);
    assert!(s.rooms_inside().is_empty());
    // Not the leader: Bob opens the page and asks.
    s.bypass(BOB, "EnterRift 1");
    assert!(
        pages(&s.log.take(), BOB)
            .pop()
            .unwrap()
            .contains("NotPartyLeader")
    );
}

/// The whole run: in, the chance card, the clock, out.
#[test]
fn a_party_runs_the_rift() {
    let mut s = Scene::new();
    s.party(100);
    s.bypass(ALICE, "EnterRift 1");
    // The recruit price is 18 fragments each.
    assert_eq!(s.count(ALICE, FRAGMENT), 82);
    assert_eq!(s.count(BOB, FRAGMENT), 82);
    // Alice and Bob are in one of the four rooms; only that one has a party.
    let first = s.current_room();
    assert_eq!(s.rooms_inside(), vec![first]);
    assert!(s.world.party_rift(s.party_id()).is_some());
    assert!(s.world.is_in_dimensional_rift(ObjectId(ALICE)));
    assert_eq!(s.world.rift_current_jump(s.party_id()), Some(1));
    s.appear(&[ALICE, BOB]);
    let target = rooms()
        .into_iter()
        .find(|r| r.kind == 1 && r.room == first)
        .unwrap();
    // `teleToLocation(coords)`: the target, 5 higher.
    assert_eq!(s.position(ALICE), (target.x_t, target.y_t, target.z_t + 5));
    assert!(
        s.world
            .rift_check_in_rift_zone(target.x_t, target.y_t, target.z_t, false)
    );
    // The monsters wait `RiftSpawnDelay` (2 s) before they appear.
    assert!(s.world.rift_room_monsters(1, first).is_empty());
    s.wait_ms(2_000);
    let monsters = s.world.rift_room_monsters(1, first);
    assert_eq!(monsters.len(), s.world.rift_room_spawn_count(1, first));
    assert!(
        monsters
            .iter()
            .all(|m| s.world.npc(*m).unwrap().npc_id == INVADER)
    );

    // The party cannot leave by itself, nor expel.
    s.input(BOB, ClientInput::WithdrawParty);
    assert!(s.world.is_in_dimensional_rift(ObjectId(BOB)));
    assert!(s.world.party_of(ObjectId(BOB)).is_some());

    // The leader uses the chance card: another room, the old one is empty and free.
    assert!(rift_bypass_handler(
        &mut s.world,
        ObjectId(ALICE),
        "ChangeRiftRoom",
        Some(s.gatekeeper)
    ));
    let second = s.current_room();
    assert_ne!(second, first);
    assert_eq!(s.world.rift_current_jump(s.party_id()), Some(2));
    assert_eq!(s.rooms_inside(), vec![second]);
    assert!(
        s.world
            .rift_room_monsters(1, first)
            .iter()
            .all(|m| s.world.npc(*m).is_none())
    );
    s.wait_ms(2_000);
    assert!(!s.world.rift_room_monsters(1, second).is_empty());
    // A member who is not the leader gets the page.
    s.log.take();
    rift_bypass_handler(&mut s.world, ObjectId(BOB), "ChangeRiftRoom", Some(s.gatekeeper));
    assert!(
        pages(&s.log.take(), BOB)
            .pop()
            .unwrap()
            .contains("NotPartyLeader")
    );
    // The third room by the clock: 10 s after the arrival (20 s in the boss room) the rift
    // jumps by itself (a room that was not visited yet when there is one).
    let is_boss = |room: i8| rooms().iter().any(|d| d.kind == 1 && d.room == room && d.boss);
    s.wait_ms(if is_boss(second) { 20_000 } else { 10_000 } - 2_000 - 100);
    assert_eq!(
        s.world.rift_current_jump(s.party_id()),
        Some(2),
        "not before the time"
    );
    s.wait_ms(200);
    assert_eq!(s.world.rift_current_jump(s.party_id()), Some(3));
    let third = s.current_room();
    assert_ne!(third, second);
    assert_ne!(third, first);
    assert_eq!(s.rooms_inside(), vec![third]);
    // The jumps are used up.
    s.log.take();
    rift_bypass_handler(
        &mut s.world,
        ObjectId(ALICE),
        "ChangeRiftRoom",
        Some(s.gatekeeper),
    );
    assert!(
        pages(&s.log.take(), ALICE)
            .pop()
            .unwrap()
            .contains("AlreadyTeleported")
    );
    assert_eq!(s.world.rift_current_jump(s.party_id()), Some(3));

    // The next jump is the way out: everybody in the waiting room, the rift is over.
    s.wait_ms(if is_boss(third) { 20_000 } else { 10_000 });
    assert!(s.world.party_rift(s.party_id()).is_none());
    assert!(!s.world.is_in_dimensional_rift(ObjectId(ALICE)));
    assert!(s.rooms_inside().is_empty());
    assert!(
        s.world
            .rift_room_monsters(1, third)
            .iter()
            .all(|m| s.world.npc(*m).is_none())
    );
    assert_eq!(s.position(ALICE), (WAIT.0, WAIT.1, WAIT.2 + 5));
    assert_eq!(s.position(BOB), (WAIT.0, WAIT.1, WAIT.2 + 5));
    // The party may withdraw now.
    s.input(BOB, ClientInput::WithdrawParty);
    assert!(s.world.party_of(ObjectId(BOB)).is_none());
}

/// The leader takes the party out at once.
#[test]
fn the_leader_can_leave_the_rift_at_once() {
    let mut s = Scene::new();
    s.party(100);
    s.bypass(ALICE, "EnterRift 1");
    let room = s.current_room();
    s.wait_ms(2_000);
    assert!(!s.world.rift_room_monsters(1, room).is_empty());
    // (Java has a border guard inside the rooms; here the command goes to the handler.)
    let party = s.party_id();
    assert!(rift_bypass_handler(
        &mut s.world,
        ObjectId(ALICE),
        "ExitRift",
        Some(s.gatekeeper)
    ));
    assert!(s.world.party_rift(party).is_none());
    assert!(s.rooms_inside().is_empty());
    assert_eq!(s.position(ALICE), (WAIT.0, WAIT.1, WAIT.2 + 5));
    assert_eq!(s.position(BOB), (WAIT.0, WAIT.1, WAIT.2 + 5));
    assert!(
        s.world
            .rift_room_monsters(1, room)
            .iter()
            .all(|m| s.world.npc(*m).is_none())
    );
}

/// A commands of the rift from a party that is not in it is an attempt to cheat.
#[test]
fn a_command_from_outside_the_rift_is_a_cheat() {
    let mut s = Scene::new();
    s.party(100);
    s.log.take();
    s.bypass(ALICE, "ChangeRiftRoom");
    let out = s.log.take();
    assert!(pages(&out, ALICE).pop().unwrap().contains("Cheater"));
    s.bypass(ALICE, "ExitRift");
    assert!(pages(&s.log.take(), ALICE).pop().unwrap().contains("Cheater"));
}

/// A dead member counts: with as many dead as party members the jump is the way out; a player
/// who logs in inside the rift zone is taken to the waiting room.
#[test]
fn the_dead_end_the_rift_at_the_next_jump_and_a_login_in_the_rift_goes_to_the_waiting_room() {
    let mut s = Scene::new();
    s.party(100);
    s.bypass(ALICE, "EnterRift 1");
    let party = s.party_id();
    s.appear(&[ALICE, BOB]);
    // Both die (`Player.doDie` adds a member of a rift party to the dead list).
    assert!(s.world.do_die(ObjectId(ALICE), None));
    assert!(s.world.do_die(ObjectId(BOB), None));
    assert_eq!(s.world.rift_dead_members(party).len(), 2);
    // The clock jump finds nobody alive: out (the entry room is no boss room, 10 s).
    s.wait_ms(10_100);
    assert!(s.world.party_rift(party).is_none());
    assert!(s.rooms_inside().is_empty());
    assert_eq!(s.position(ALICE), (WAIT.0, WAIT.1, WAIT.2 + 5));
    // A player whose character was saved in a room of the rift.
    s.enter_at(CARL, "Carl", Vec::new(), (-111_976, -182_448, -6_752));
    assert_eq!(s.position(CARL), (WAIT.0, WAIT.1, WAIT.2 + 5));
}

/// Too few members left (a member's connection is gone) ends the rift for the rest.
#[test]
fn losing_a_member_below_the_minimum_ends_the_rift() {
    let mut s = Scene::new();
    s.party(100);
    s.bypass(ALICE, "EnterRift 1");
    assert!(s.world.party_rift(s.party_id()).is_some());
    let party = s.party_id();
    // Bob logs out in the rift: a party of two is disbanded, and the first member to go (the
    // leader) leaves one member, which is under the minimum of two.
    s.input(BOB, ClientInput::Logout);
    assert!(s.world.party_rift(party).is_none());
    assert!(s.rooms_inside().is_empty());
    // NOTE(java-quirk): `partyMemberExited` only brings the members that are left in the party
    // back, and the leader was the first to be removed: he stays in the room.
    assert!(
        !s.world
            .rift_check_in_peace_zone(s.position(ALICE).0, s.position(ALICE).1, s.position(ALICE).2)
    );
}
