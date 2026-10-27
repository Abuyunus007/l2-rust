#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Boats and air ships (M8-VEHICLE-01): boarding a ferry, a leg of its route, getting off, the
//! busy dock quirk, an air ship boarding, and the route tables against the Java scripts.
//! Java: managers/BoatManager.java, entity/actor/Vehicle.java, the scripts in
//! `dist/game/data/scripts/vehicles/` and `ai/areas/Gracia/vehicles/`.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{ExGetOnAirShip, GetOffVehicle, GetOnVehicle, VehicleDeparture, VehicleInfo};
use l2_util::ObjectId;
use l2_world::vehicle::boat_routes::BOAT_ROUTES;
use l2_world::*;

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
<item id="1074" type="EtcItem" name="Gludio Ticket"><set name="weight" val="1" /></item>
</list>"#;

const STATS: &str = r#"<stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>"#;

const TICKET: i32 = 1074;
const ALICE: i32 = 0x1000_0001;
const BOB: i32 = 0x1000_0002;
/// `BoatTalkingGludin`'s dock at Talking Island, the boat's spawn.
const TALKING_DOCK: (i32, i32, i32) = (-96_622, 261_660, -3610);

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
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new(pages: &[(&str, &str)]) -> Self {
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
        for (path, text) in pages {
            write(path, text);
        }
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(5), Box::new(Recorder(log.clone())));
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
                    r#"<list><npc id="32609" type="Folk" level="70" name="Controller">{STATS}</npc></list>"#
                ),
                &NpcLoadOptions::default(),
            )
            .unwrap(),
        );
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        Self {
            world,
            log,
            _root: root,
        }
    }

    fn enter(&mut self, id: i32, name: &str, pos: (i32, i32, i32), tickets: i64) {
        let mut items = vec![ItemData {
            object_id: ObjectId(id + 0x100),
            item_id: 57,
            count: 1000,
            loc: "INVENTORY".into(),
            ..ItemData::default()
        }];
        if tickets > 0 {
            items.push(ItemData {
                object_id: ObjectId(id + 0x200),
                item_id: TICKET,
                count: tickets,
                loc: "INVENTORY".into(),
                ..ItemData::default()
            });
        }
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
            access_level: 0,
            x: pos.0,
            y: pos.1,
            z: pos.2,
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

    fn vehicle_input(&mut self, id: i32, input: VehicleInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(id as u64),
            input: ClientInput::Vehicle(input),
        });
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    fn pos(&self, id: i32) -> (i32, i32, i32) {
        self.world.position_of(ObjectId(id)).unwrap()
    }

    fn boat(&self, index: usize) -> ObjectId {
        self.world.vehicles().boats()[index]
    }

    fn boat_pos(&self, boat: ObjectId) -> (i32, i32, i32) {
        let v = self.world.vehicles().vehicle(boat).unwrap();
        (v.x, v.y, v.z)
    }

    fn tickets(&self, id: i32) -> i64 {
        self.world
            .player(ObjectId(id))
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == TICKET)
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

fn decode_ex<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[3..])).unwrap()
}

fn of<'a>(out: &'a [Out], to: i32, name: &str) -> Vec<&'a [u8]> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(id, b) if *id == to && name_of(b) == name => Some(b.as_slice()),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------- boats

/// The five boats are made; the Rune harbour can never be busy (`_docksBusy` has three flags).
#[test]
fn the_boats_are_made_and_the_rune_harbour_cannot_be_busy() {
    let mut s = Scene::new(&[]);
    s.world.spawn_boats();
    assert_eq!(s.world.vehicles().boats().len(), 5);
    // `main` of BoatTalkingGludin: `dockShip(TALKING_ISLAND, true)`.
    assert!(s.world.dock_busy(1));
    // `main` of BoatGludinRune: `dockShip(GLUDIN_HARBOR, true)`.
    assert!(s.world.dock_busy(2));
    // `main` of BoatRunePrimeval locks dock 3, which Java's three-entry array cannot hold.
    assert!(!s.world.dock_busy(3));
    assert!(!s.world.dock_busy(7));
    assert!(!s.world.dock_busy(-1));
    let first = s.boat(0);
    let v = s.world.vehicles().vehicle(first).unwrap();
    assert_eq!(
        ((v.x, v.y, v.z, v.heading), v.engine.is_some()),
        (
            (BOAT_ROUTES[0].x, BOAT_ROUTES[0].y, BOAT_ROUTES[0].z, 32768),
            true
        )
    );
}

/// `AllowBoat = False`: no boat at all.
#[test]
fn no_boats_without_allow_boat() {
    let mut s = Scene::new(&[]);
    s.world.set_vehicle_settings(VehicleSettings {
        allow_boat: false,
        ..VehicleSettings::default()
    });
    s.world.spawn_boats();
    assert!(s.world.vehicles().boats().is_empty());
}

/// Board at the dock, get off again before the boat leaves.
#[test]
fn board_a_boat_and_get_off_at_the_dock() {
    let mut s = Scene::new(&[]);
    s.world.spawn_boats();
    let boat = s.boat(0);
    s.enter(
        ALICE,
        "Alice",
        (TALKING_DOCK.0 + 50, TALKING_DOCK.1, TALKING_DOCK.2),
        1,
    );
    s.enter(
        BOB,
        "Bob",
        (TALKING_DOCK.0 + 60, TALKING_DOCK.1, TALKING_DOCK.2),
        0,
    );
    s.log.take();
    s.vehicle_input(
        ALICE,
        VehicleInput::GetOn {
            boat_id: boat.0,
            x: 10,
            y: 20,
            z: 30,
        },
    );
    assert_eq!(s.world.vehicle_of(ObjectId(ALICE)), Some(boat));
    assert_eq!(s.world.in_vehicle_position(ObjectId(ALICE)), Some((10, 20, 30)));
    assert!(s.world.is_in_boat(ObjectId(ALICE)));
    // Self and everybody around get `GetOnVehicle`.
    let out = s.log.take();
    for to in [ALICE, BOB] {
        let packets = of(&out, to, "GET_ON_VEHICLE");
        assert_eq!(packets.len(), 1, "{to:x}");
        let p: GetOnVehicle = decode(packets[0]);
        assert_eq!(
            (p.object_id, p.boat_object_id, p.x, p.y, p.z),
            (ALICE, boat.0, 10, 20, 30)
        );
    }
    // The player is put where the boat is.
    assert_eq!(s.pos(ALICE), s.boat_pos(boat));

    // Off again, next to the boat.
    s.vehicle_input(
        ALICE,
        VehicleInput::GetOff {
            boat_id: boat.0,
            x: TALKING_DOCK.0 + 100,
            y: TALKING_DOCK.1,
            z: TALKING_DOCK.2,
        },
    );
    assert_eq!(s.world.vehicle_of(ObjectId(ALICE)), None);
    assert_eq!(s.world.in_vehicle_position(ObjectId(ALICE)), None);
    let out = s.log.take();
    let off: GetOffVehicle = decode(of(&out, BOB, "GET_OFF_VEHICLE")[0]);
    assert_eq!(
        (off.object_id, off.boat_object_id, off.x),
        (ALICE, boat.0, TALKING_DOCK.0 + 100)
    );
    assert_eq!(
        s.pos(ALICE),
        (TALKING_DOCK.0 + 100, TALKING_DOCK.1, TALKING_DOCK.2)
    );
}

/// A player far from the boat cannot board it (`isInsideRadius3D(boat, 1000)`).
#[test]
fn boarding_needs_the_boat_in_reach() {
    let mut s = Scene::new(&[]);
    s.world.spawn_boats();
    let boat = s.boat(0);
    s.enter(
        ALICE,
        "Alice",
        (TALKING_DOCK.0 + 1500, TALKING_DOCK.1, TALKING_DOCK.2),
        1,
    );
    s.vehicle_input(
        ALICE,
        VehicleInput::GetOn {
            boat_id: boat.0,
            x: 0,
            y: 0,
            z: 0,
        },
    );
    assert_eq!(s.world.vehicle_of(ObjectId(ALICE)), None);
}

/// Sail the whole Talking Island to Gludin leg: the ticket is taken at the start, the passenger
/// goes with the boat, the boat docks, and the passenger gets off at Gludin.
#[test]
fn sail_a_route_leg_and_get_off_at_the_other_dock() {
    let mut s = Scene::new(&[]);
    s.world.spawn_boats();
    let boat = s.boat(0);
    s.enter(
        ALICE,
        "Alice",
        (TALKING_DOCK.0 + 50, TALKING_DOCK.1, TALKING_DOCK.2),
        1,
    );
    // Bob rides without a ticket.
    s.enter(
        BOB,
        "Bob",
        (TALKING_DOCK.0 + 60, TALKING_DOCK.1, TALKING_DOCK.2),
        0,
    );
    for id in [ALICE, BOB] {
        s.vehicle_input(
            id,
            VehicleInput::GetOn {
                boat_id: boat.0,
                x: 1,
                y: 2,
                z: 3,
            },
        );
    }
    s.log.take();
    // 3 minutes, then 5 + 40 s + 20 s of announcements: the boat leaves after 480 s (9600 ticks).
    s.ticks(9590);
    assert_eq!(s.tickets(ALICE), 1);
    assert!(s.world.vehicles().vehicle(boat).unwrap().passengers.is_empty());
    s.ticks(30);
    // `payForRide(1074, 1, ...)`: Alice paid and rides, Bob is put ashore.
    assert_eq!(s.tickets(ALICE), 0);
    assert_eq!(
        s.world.vehicles().vehicle(boat).unwrap().passengers,
        vec![ObjectId(ALICE)]
    );
    assert_eq!(s.world.vehicle_of(ObjectId(BOB)), None);
    let out = s.log.take();
    // `VehicleStarted(1)`, `VehicleDeparture` towards the first point of the route.
    let departure: VehicleDeparture = decode(of(&out, ALICE, "VEHICLE_DEPARTURE")[0]);
    assert_eq!(
        (
            departure.x,
            departure.y,
            departure.z,
            departure.move_speed,
            departure.rotation_speed
        ),
        (-121_385, 261_660, -3610, 180, 800)
    );
    assert!(!of(&out, ALICE, "VEHICLE_START").is_empty());
    assert!(!of(&out, ALICE, "PLAY_SOUND").is_empty());
    assert!(!of(&out, BOB, "CREATURE_SAY").is_empty() || !of(&out, BOB, "SAY2").is_empty());

    // The first leg is 24763 units at 180 per second, 138 s: the boat is on its way and the
    // passenger is where the boat is.
    s.ticks(1000);
    let (x, y, _) = s.boat_pos(boat);
    assert!(x < TALKING_DOCK.0 - 5000 && y == 261_660, "{x} {y}");
    assert_eq!(s.pos(ALICE), s.boat_pos(boat));
    s.ticks(1900);
    // The boat sails on, the passenger with it.
    s.ticks(3000);
    assert_ne!(s.boat_pos(boat), (-121_385, 261_660, -3610));
    assert_eq!(s.pos(ALICE), s.boat_pos(boat));

    // The trip takes 919 s; the Gludin dock is free, the boat comes to rest there.
    s.ticks(16_000);
    let dock = (-95_686, 150_514, -3610);
    assert_eq!(s.boat_pos(boat), dock);
    assert!(!s.world.vehicles().vehicle(boat).unwrap().is_moving());
    assert!(s.world.dock_busy(2));
    // Get off at the quay.
    s.log.take();
    s.vehicle_input(
        ALICE,
        VehicleInput::GetOff {
            boat_id: boat.0,
            x: -95_686,
            y: 150_900,
            z: -3610,
        },
    );
    assert_eq!(s.world.vehicle_of(ObjectId(ALICE)), None);
    assert_eq!(s.pos(ALICE), (-95_686, 150_900, -3610));
}

/// A player who comes into view of a moving boat is told where it goes.
#[test]
fn a_new_player_sees_the_boat_and_where_it_goes() {
    let mut s = Scene::new(&[]);
    s.world.spawn_boats();
    let boat = s.boat(0);
    s.enter(
        ALICE,
        "Alice",
        (TALKING_DOCK.0 + 50, TALKING_DOCK.1, TALKING_DOCK.2),
        1,
    );
    s.vehicle_input(
        ALICE,
        VehicleInput::GetOn {
            boat_id: boat.0,
            x: 1,
            y: 2,
            z: 3,
        },
    );
    s.ticks(9700);
    s.log.take();
    s.enter(BOB, "Bob", (-98_000, 261_660, -3610), 0);
    let out = s.log.take();
    let info: VehicleInfo = decode(of(&out, BOB, "VEHICLE_INFO")[0]);
    assert_eq!(info.object_id, boat.0);
    assert!(!of(&out, BOB, "VEHICLE_DEPARTURE").is_empty());
    // Alice's `CharInfo` shows her inside the boat.
    let char_info = s.world.char_info(ObjectId(ALICE), ObjectId(BOB)).unwrap();
    assert_eq!(
        (char_info.vehicle_id, char_info.x, char_info.y, char_info.z),
        (boat.0, 1, 2, 3)
    );
}

// ---------------------------------------------------------------------------------- air ships

const GLUDIO_PAGE: &str = "data/scripts/ai/areas/Gracia/vehicles/AirShipGludioGracia/32609.htm";

/// The scheduled ship waits in the Gludio dock; the controller NPC boards a player; the ship
/// leaves a minute after the start and the passenger goes with it.
#[test]
fn board_the_gludio_gracia_air_ship() {
    let mut s = Scene::new(&[(
        GLUDIO_PAGE,
        "<html><body><a action=\"bypass -h Script AirShipGludioGracia board\">Board</a></body></html>",
    )]);
    s.world.start_air_ships();
    let ship = s.world.vehicles().gludio_gracia_ship().unwrap();
    let v = s.world.vehicles().vehicle(ship).unwrap();
    assert_eq!((v.x, v.y, v.z, v.dock_id), (-149_378, 252_552, 198, 10));
    let npc = s
        .world
        .spawn_npc_at(32609, -149_360, 252_936, -136, 20_771)
        .unwrap();
    s.enter(ALICE, "Alice", (-149_340, 252_900, -136), 0);
    // Click the controller twice: the first selects, the second talks.
    for _ in 0..2 {
        s.world.handle(WorldCommand::Input {
            session: SessionId(ALICE as u64),
            input: ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        });
        s.ticks(2);
    }
    s.log.take();
    s.world.handle(WorldCommand::Input {
        session: SessionId(ALICE as u64),
        input: ClientInput::Bypass {
            command: "Script AirShipGludioGracia board".into(),
        },
    });
    assert_eq!(s.world.vehicle_of(ObjectId(ALICE)), Some(ship));
    assert!(s.world.is_in_air_ship(ObjectId(ALICE)));
    assert_eq!(s.world.in_vehicle_position(ObjectId(ALICE)), Some((0, 0, 0)));
    let out = s.log.take();
    let on: ExGetOnAirShip = decode_ex(of(&out, ALICE, "EX_GET_ON_AIRSHIP")[0]);
    assert_eq!((on.player_id, on.air_ship_id), (ALICE, ship.0));
    assert_eq!(s.pos(ALICE), (-149_378, 252_552, 198));

    // After 60 s the ship leaves the dock for the warp gate; the passenger moves with it.
    s.ticks(1250);
    let v = s.world.vehicles().vehicle(ship).unwrap();
    assert_eq!(v.dock_id, 0);
    assert!(v.is_moving());
    s.ticks(400);
    let v = s.world.vehicles().vehicle(ship).unwrap();
    assert_ne!((v.x, v.y), (-149_378, 252_552));
    assert_eq!(s.pos(ALICE), (v.x, v.y, v.z));
    // Control actions of a passenger who is no captain do nothing.
    s.world.handle(WorldCommand::Input {
        session: SessionId(ALICE as u64),
        input: ClientInput::Vehicle(VehicleInput::AirShipControl {
            command: 1,
            param1: 0,
            param2: 0,
        }),
    });
}

// ---------------------------------------------------------------------------------- the data

/// Every path of the boat scripts, as the Java sources have it.
#[test]
fn the_boat_paths_are_the_ones_of_the_java_scripts() {
    use l2_world::vehicle::boat_routes as r;
    let root = std::path::Path::new(r"C:\l2-rust\l2jmobius\dist\game\data\scripts\vehicles");
    if !root.is_dir() {
        eprintln!("the Java scripts are not here; skipping");
        return;
    }
    let java = |name: &str| std::fs::read_to_string(root.join(format!("{name}.java"))).unwrap();
    let points = |src: &str, array: &str| -> Vec<(i32, i32, i32, i32, i32)> {
        let start = src.find(&format!("VehiclePathPoint[] {array} =")).unwrap();
        let end = start + src[start..].find("};").unwrap();
        src[start..end]
            .split("new VehiclePathPoint(")
            .skip(1)
            .map(|p| {
                let n: Vec<i32> = p[..p.find(')').unwrap()]
                    .split(',')
                    .map(|v| v.trim().parse().unwrap())
                    .collect();
                if n.len() == 3 {
                    (n[0], n[1], n[2], 350, 4000)
                } else {
                    (n[0], n[1], n[2], n[3], n[4])
                }
            })
            .collect()
    };
    let rust = |path: &[l2_world::PathPoint]| -> Vec<(i32, i32, i32, i32, i32)> {
        path.iter()
            .map(|p| (p.x, p.y, p.z, p.move_speed, p.rotation_speed))
            .collect()
    };
    let cases: [(&str, &str, &[l2_world::PathPoint]); 10] = [
        (
            "BoatTalkingGludin",
            "TALKING_TO_GLUDIN",
            &r::TALKING_GLUDIN_TALKING_TO_GLUDIN,
        ),
        (
            "BoatTalkingGludin",
            "GLUDIN_TO_TALKING",
            &r::TALKING_GLUDIN_GLUDIN_TO_TALKING,
        ),
        (
            "BoatGiranTalking",
            "GIRAN_TO_TALKING",
            &r::GIRAN_TALKING_GIRAN_TO_TALKING,
        ),
        (
            "BoatGiranTalking",
            "TALKING_TO_GIRAN",
            &r::GIRAN_TALKING_TALKING_TO_GIRAN,
        ),
        ("BoatGludinRune", "GLUDIN_TO_RUNE", &r::GLUDIN_RUNE_GLUDIN_TO_RUNE),
        ("BoatGludinRune", "RUNE_TO_GLUDIN", &r::GLUDIN_RUNE_RUNE_TO_GLUDIN),
        (
            "BoatRunePrimeval",
            "RUNE_TO_PRIMEVAL",
            &r::RUNE_PRIMEVAL_RUNE_TO_PRIMEVAL,
        ),
        (
            "BoatRunePrimeval",
            "PRIMEVAL_TO_RUNE",
            &r::RUNE_PRIMEVAL_PRIMEVAL_TO_RUNE,
        ),
        ("BoatInnadrilTour", "TOUR", &r::INNADRIL_TOUR_TOUR),
        ("BoatGludinRune", "RUNE_DOCK", &r::GLUDIN_RUNE_RUNE_DOCK),
    ];
    for (file, array, path) in cases {
        assert_eq!(rust(path), points(&java(file), array), "{file} {array}");
    }
}

/// The docks and the Gludio-Gracia paths against the Java sources.
#[test]
fn the_air_ship_paths_are_the_ones_of_the_java_scripts() {
    use l2_world::vehicle::controller as c;
    let root = std::path::Path::new(r"C:\l2-rust\l2jmobius\dist\game\data\scripts\ai\areas\Gracia\vehicles");
    if !root.is_dir() {
        eprintln!("the Java scripts are not here; skipping");
        return;
    }
    let numbers = |text: &str| -> Vec<(i32, i32, i32, i32, i32)> {
        text.split("new VehiclePathPoint(")
            .skip(1)
            .map(|p| {
                let n: Vec<i32> = p[..p.find(')').unwrap()]
                    .split(',')
                    .map(|v| v.trim().parse().unwrap())
                    .collect();
                if n.len() == 3 {
                    (n[0], n[1], n[2], 350, 4000)
                } else {
                    (n[0], n[1], n[2], n[3], n[4])
                }
            })
            .collect()
    };
    let rust = |path: &[l2_world::PathPoint]| -> Vec<(i32, i32, i32, i32, i32)> {
        path.iter()
            .map(|p| (p.x, p.y, p.z, p.move_speed, p.rotation_speed))
            .collect()
    };
    for def in &c::CONTROLLERS {
        let src = std::fs::read_to_string(root.join(def.name).join(format!("{}.java", def.name))).unwrap();
        let section = |name: &str| {
            let start = src.find(&format!("{name} =")).unwrap();
            let end = start + src[start..].find("};").unwrap();
            src[start..end].to_owned()
        };
        assert_eq!(
            rust(def.arrival),
            numbers(&section("ARRIVAL")),
            "{} arrival",
            def.name
        );
        assert_eq!(
            rust(def.depart),
            numbers(&section("DEPART")),
            "{} depart",
            def.name
        );
        let teleports: Vec<_> = def.teleports.iter().map(|t| rust(t)).collect();
        // The rows are the groups at depth 2 of the braces.
        let java_teleports = {
            let all = section("TELEPORTS");
            let (mut depth, mut row, mut out) = (0, String::new(), Vec::new());
            for ch in all.chars() {
                match ch {
                    '{' => depth += 1,
                    '}' => {
                        if depth == 2 {
                            out.push(numbers(&row));
                            row.clear();
                        }
                        depth -= 1;
                    }
                    _ if depth == 2 => row.push(ch),
                    _ => {}
                }
            }
            out
        };
        assert_eq!(teleports, java_teleports, "{} teleports", def.name);
        assert!(src.contains(&format!("DOCK_ZONE = {}", def.dock_zone)));
        assert!(src.contains(&format!("CONTROLLER_ID = {}", def.npc_id)));
        assert!(src.contains(&format!("LOCATION = {}", def.location_id)));
    }
    let src =
        std::fs::read_to_string(root.join("AirShipGludioGracia").join("AirShipGludioGracia.java")).unwrap();
    for (array, path) in [
        ("GLUDIO_TO_WARPGATE", &c::GLUDIO_TO_WARPGATE[..]),
        ("WARPGATE_TO_GRACIA", &c::WARPGATE_TO_GRACIA[..]),
        ("GRACIA_TO_WARPGATE", &c::GRACIA_TO_WARPGATE[..]),
        ("WARPGATE_TO_GLUDIO", &c::WARPGATE_TO_GLUDIO[..]),
    ] {
        let start = src.find(&format!("{array} =")).unwrap();
        let end = start + src[start..].find("};").unwrap();
        assert_eq!(rust(path), numbers(&src[start..end]), "{array}");
    }
}

/// `getTeleportDestination` / `registerAirShipTeleportList` ignore a list of the wrong length.
#[test]
fn a_dock_without_zones_is_disabled() {
    let mut s = Scene::new(&[]);
    s.world.start_air_ships();
    // No zone table: `validityCheck` finds no ScriptZone, "controller disabled".
    assert_eq!(s.world.air_ship_controllers().len(), 4);
    assert!(s.world.air_ship_controllers().iter().all(|c| c.is_busy()));
}

/// A license is a new row; a second registration does nothing.
#[test]
fn a_license_is_a_row() {
    let mut s = Scene::new(&[]);
    s.world.load_airships(VehicleLoad {
        airships: vec![AirshipRow {
            owner_id: 7,
            fuel: 123,
        }],
    });
    assert!(s.world.has_air_ship_license(7));
    assert!(!s.world.has_air_ship_license(8));
    s.world.register_air_ship_license(8);
    s.world.register_air_ship_license(8);
    assert!(s.world.has_air_ship_license(8));
    let writes: Vec<_> = s
        .log
        .take()
        .into_iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Vehicle(w)) => Some(w),
            _ => None,
        })
        .collect();
    assert_eq!(
        writes,
        vec![VehicleWrite::AddLicense {
            owner_id: 8,
            fuel: 600
        }]
    );
}
