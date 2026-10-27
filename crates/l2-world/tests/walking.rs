#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! NPC walking routes (M8-MISC-01): an NPC walks the nodes of a `Routes.xml` route in Java's
//! order for each repeat style, says the words of a node, waits `100 ms + delay` there, comes
//! back to its route after a fight stopped it, and a route's target NPC never wanders by itself.
//! Also the event drops (`EventDropManager`).
//! Java: managers/WalkingManager.java, entity/actor/holders/npc/WalkInfo.java,
//! managers/EventDropManager.java.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates, Routes,
    SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{MoveToLocation, NpcSay};
use l2_world::rewards::event_drop::EventDrop;
use l2_world::*;

// ------------------------------------------------------------------ harness

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

struct NoOutbox;

impl Outbox for NoOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
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
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

fn player_data(id: i32, (x, y, z): (i32, i32, i32)) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: "Hero".into(),
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
        x,
        y,
        z,
        heading: 0,
        cur_hp: 5000.0,
        max_hp: 5000.0,
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

const CLASS: &str = r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>5000</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="9900" type="EtcItem" name="Event Coin"><set name="is_stackable" val="true" /></item>
</list>"#;

fn npc(id: i32, kind: &str, level: i32, body: &str) -> String {
    format!(
        r#"<npc id="{id}" type="{kind}" level="{level}" name="Test {id}">
    {body}
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai isAggressive="false" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>"#
    )
}

/// 9020 a folk NPC that walks routes, 9021 a monster that walks one, 9022 a plain monster.
fn npcs_xml() -> String {
    format!(
        r#"<?xml version="1.0"?><list>{}{}{}{}</list>"#,
        npc(9020, "Folk", 1, ""),
        npc(9021, "Monster", 1, ""),
        npc(9022, "Monster", 5, ""),
        npc(9023, "Folk", 1, ""),
    )
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: i32 = 0x2000_0001;

/// P0 is the spawn point.
const ROUTES: &str = r#"<routes>
  <route name="line" repeat="true" repeatStyle="back">
    <target id="9020" spawnX="-71338" spawnY="258271" spawnZ="-3104" />
    <point X="-71338" Y="258271" Z="-3104" delay="0" run="true" />
    <point X="-71138" Y="258271" Z="-3104" delay="1" run="true" string="Halt!" />
    <point X="-71138" Y="258471" Z="-3104" delay="0" run="true" />
  </route>
  <route name="loop" repeat="true" repeatStyle="cycle">
    <target id="9023" spawnX="-71338" spawnY="258271" spawnZ="-3104" />
    <point X="-71338" Y="258271" Z="-3104" delay="0" run="true" />
    <point X="-71238" Y="258271" Z="-3104" delay="0" run="true" />
    <point X="-71238" Y="258371" Z="-3104" delay="0" run="true" />
  </route>
</routes>"#;

struct Scene {
    world: World,
    log: Log,
    _dir: tempfile::TempDir,
}

impl Scene {
    /// The spawn is `spawns` (the body of a spawn file); the routes are installed before it.
    fn new(spawns: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let spawn_dir = dir.path().join("data/spawns");
        std::fs::create_dir_all(&spawn_dir).unwrap();
        std::fs::write(
            spawn_dir.join("test.xml"),
            format!(r#"<?xml version="1.0"?><list enabled="true">{spawns}</list>"#),
        )
        .unwrap();
        let npcs = Arc::new(NpcTemplates::from_xml(&npcs_xml(), &NpcLoadOptions::default()).unwrap());
        let spawn_data = Arc::new(SpawnData::load(dir.path(), &npcs, &SpawnLoadOptions::default()));
        let routes = Arc::new(Routes::from_xml(ROUTES, &|id| npcs.get(id).is_some()).unwrap());
        let mut world = World::new(WorldRng::from_seed(20_261_019), Box::new(NoOutbox));
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
        world.set_npc_settings(NpcSettings::default());
        world.set_npc_data(npcs, spawn_data);
        world.set_walking_routes(routes);
        let mut scene = Self {
            world,
            log: Log::default(),
            _dir: dir,
        };
        // A player close by: the region is on, the NPCs think and the walkers move.
        scene.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(ME).unwrap()),
            player: Box::new(player_data(ME, (X + 50, Y, Z))),
            sink: Box::new(Sink(ME, scene.log.clone())),
            ip: None,
        });
        scene.world.spawn_all_npcs(false);
        scene.log.take();
        scene
    }

    fn npc_of(&self, npc_id: i32) -> ObjectId {
        let found: Vec<ObjectId> = self
            .world
            .npc_ids()
            .into_iter()
            .filter(|id| self.world.npc(*id).unwrap().npc_id == npc_id)
            .collect();
        assert_eq!(found.len(), 1, "one NPC {npc_id}: {found:?}");
        found[0]
    }

    fn pos(&self, id: ObjectId) -> (i32, i32, i32) {
        self.world.position_of(id).unwrap()
    }
}

fn spawn_line(npc_id: i32, x: i32, y: i32, z: i32) -> String {
    format!(r#"<spawn><npc id="{npc_id}" x="{x}" y="{y}" z="{z}" heading="0"/></spawn>"#)
}

fn near(a: (i32, i32, i32), x: i32, y: i32) -> bool {
    (a.0 - x).abs() <= 10 && (a.1 - y).abs() <= 10
}

// ------------------------------------------------------------------ the walk

/// A `back` route: the NPC spawns on the first node, so it starts toward the second, says the
/// words there, waits, goes on to the third, comes back through the second to the first, and
/// after the first goes forward again (`WalkInfo.calculateNextNode`: the cursor -1 becomes 1).
#[test]
fn a_back_route_visits_its_nodes_in_javas_order() {
    let mut s = Scene::new(&spawn_line(9020, X, Y, Z));
    let walker = s.npc_of(9020);
    assert!(s.world.is_walker(walker));
    assert_eq!(s.world.walking_route_name(walker), "line");

    let nodes = [(X, Y), (X + 200, Y), (X + 200, Y + 200)];
    let mut visited: Vec<usize> = Vec::new();
    let mut said = Vec::new();
    let mut said_at_tick = 0;
    let mut arrived_at_tick = 0;
    let mut first_move = None;
    for tick in 1..=3000_u32 {
        s.world.tick();
        for (to, raw) in s.log.take() {
            if to != ME {
                continue;
            }
            match name_of(&raw).as_str() {
                "MOVE_TO_LOCATION" => {
                    let m: MoveToLocation = decode(&raw);
                    if m.object_id == walker.0 && first_move.is_none() {
                        first_move = Some((m.dest_x, m.dest_y));
                    }
                }
                "NPC_SAY" => {
                    let say: NpcSay = decode(&raw);
                    if say.object_id == walker.0 {
                        said.push(say.text);
                        said_at_tick = tick;
                    }
                }
                _ => {}
            }
        }
        let here = s.pos(walker);
        for (index, (x, y)) in nodes.iter().enumerate() {
            if near(here, *x, *y) && visited.last() != Some(&index) {
                visited.push(index);
                if index == 1 && arrived_at_tick == 0 {
                    arrived_at_tick = tick;
                }
            }
        }
        if visited.len() >= 7 {
            break;
        }
    }
    // The NPC stood on node 0 at the start, so the walk is [1, 2, 1, 0, 1, 2, 1, ...].
    assert_eq!(first_move, Some((X + 200, Y)), "starts toward the second node");
    assert_eq!(visited[..7], [0, 1, 2, 1, 0, 1, 2], "{visited:?}");
    // It said the node's words at the second node, each time it arrived there.
    assert!(said.iter().all(|t| t == "Halt!") && said.len() >= 2, "{said:?}");
    // The wait at the second node is 100 ms + 1 s: the NPC does not leave at once.
    assert!(said_at_tick >= arrived_at_tick);
}

/// A `cycle` route goes from the last node straight to the first.
#[test]
fn a_cycle_route_goes_back_to_the_first_node() {
    let mut s = Scene::new(&spawn_line(9023, X, Y, Z));
    let walker = s.npc_of(9023);
    let nodes = [(X, Y), (X + 100, Y), (X + 100, Y + 100)];
    let mut visited: Vec<usize> = Vec::new();
    for _ in 0..3000 {
        s.world.tick();
        s.log.take();
        let here = s.pos(walker);
        for (index, (x, y)) in nodes.iter().enumerate() {
            if near(here, *x, *y) && visited.last() != Some(&index) {
                visited.push(index);
            }
        }
        if visited.len() >= 7 {
            break;
        }
    }
    assert_eq!(visited[..7], [0, 1, 2, 0, 1, 2, 0], "{visited:?}");
}

/// A walker that a fight stopped (`stopMoving(npc, false, true)`) is not "on walk" any more, and
/// the 10 s check task of its walk sets it going again.
#[test]
fn a_stopped_walker_resumes_after_the_check_task() {
    let mut s = Scene::new(&spawn_line(9020, X, Y, Z));
    let walker = s.npc_of(9020);
    for _ in 0..10 {
        s.world.tick();
    }
    s.log.take();
    assert!(s.world.walking_is_on_walk(walker));

    s.world.walking_stop_moving(walker, false, true);
    assert!(!s.world.walking_is_on_walk(walker));
    let stopped_at = s.pos(walker);
    s.log.take();
    // Nine seconds: the walker stands still.
    for _ in 0..180 {
        s.world.tick();
    }
    assert_eq!(s.pos(walker), stopped_at);
    // The check task (every 10 s) resumes the walk.
    let mut moved = false;
    for _ in 0..60 {
        s.world.tick();
        moved |= s.pos(walker) != stopped_at;
    }
    assert!(moved, "the walker is still where the fight stopped it");
    assert!(s.world.walking_is_on_walk(walker));
}

/// Only the NPC ids a route targets are walkers' targets; a spawn at another location is not
/// attached, and a monster of a targeted id never random-walks.
#[test]
fn only_the_targeted_spawn_walks() {
    let s = Scene::new(&format!(
        "{}{}",
        spawn_line(9020, X, Y, Z),
        spawn_line(9022, X + 500, Y, Z)
    ));
    assert!(s.world.walking_is_targeted(9020));
    assert!(!s.world.walking_is_targeted(9022));
    assert!(s.world.is_walker(s.npc_of(9020)));
    assert!(!s.world.is_walker(s.npc_of(9022)));
    assert_eq!(s.world.walking_route_name(s.npc_of(9022)), "");
    // 9020 spawned somewhere the route does not name: no walk.
    let mut elsewhere = Scene::new(&spawn_line(9020, X + 1, Y, Z));
    assert!(!elsewhere.world.is_walker(elsewhere.npc_of(9020)));
    elsewhere.world.tick();
}

// ------------------------------------------------------------------ event drops

fn coin_drop(chance: f64, min_level: i32, max_level: i32, monsters: &[i32]) -> EventDrop {
    EventDrop {
        item_id: 9900,
        min: 2,
        max: 4,
        chance,
        min_level,
        max_level,
        monster_ids: monsters.iter().copied().collect(),
    }
}

/// How many event coins the player holds.
fn coins(s: &Scene) -> i64 {
    s.world
        .player(ObjectId(ME))
        .unwrap()
        .data
        .items
        .iter()
        .filter(|i| i.item_id == 9900)
        .map(|i| i.count)
        .sum()
}

/// A scene with a monster of level 5 (9022) and a monster of level 1 (9021), and a killer who
/// auto loots.
fn drop_scene() -> Scene {
    let mut s = Scene::new(&format!(
        "{}{}",
        spawn_line(9022, X + 300, Y, Z),
        spawn_line(9021, X + 400, Y, Z)
    ));
    s.world.set_settings(loot_settings(9));
    s
}

/// An auto looting world with the given `EventItemMaxLevelDifference`.
fn loot_settings(difference: i32) -> WorldSettings {
    WorldSettings {
        ground: GroundItemSettings {
            auto_loot: true,
            ..GroundItemSettings::default()
        },
        drops: DropSettings {
            event_item_max_level_difference: difference,
            ..DropSettings::default()
        },
        ..WorldSettings::default()
    }
}

/// `EventDropManager.doEventDrop`: an event's list is rolled for the monster; the monster id
/// list, the level range, the chance and the killer's level difference all have to allow it.
#[test]
fn an_event_drop_is_rolled_for_a_kill() {
    let mut s = drop_scene();
    let (five, one) = (s.npc_of(9022), s.npc_of(9021));
    let killer = Some(ObjectId(ME));

    // No event, no drop.
    s.world.do_event_drop(killer, five);
    assert_eq!(coins(&s), 0);

    s.world
        .add_event_drops("Halloween", vec![coin_drop(100.0, 1, 99, &[])]);
    s.world.do_event_drop(killer, five);
    let first = coins(&s);
    assert!((2..=4).contains(&first), "{first}");

    // The list of the monster ids: 9021 is not on it.
    s.world
        .add_event_drops("Halloween", vec![coin_drop(100.0, 1, 99, &[9021])]);
    s.world.do_event_drop(killer, five);
    assert_eq!(coins(&s), first);
    s.world.do_event_drop(killer, one);
    assert!(coins(&s) > first);

    // The level range of the monster (level 5): 6 to 99 does not take it.
    let before = coins(&s);
    s.world
        .add_event_drops("Halloween", vec![coin_drop(100.0, 6, 99, &[])]);
    s.world.do_event_drop(killer, five);
    // A chance of 0 never drops.
    s.world
        .add_event_drops("Halloween", vec![coin_drop(0.0, 1, 99, &[])]);
    for _ in 0..50 {
        s.world.do_event_drop(killer, five);
    }
    // Nobody killed it (a monster that died to nothing, or to an NPC).
    s.world
        .add_event_drops("Halloween", vec![coin_drop(100.0, 1, 99, &[])]);
    s.world.do_event_drop(None, five);
    assert_eq!(coins(&s), before);

    // The event ends: its list goes.
    s.world.remove_event_drops("Halloween");
    s.world.do_event_drop(killer, five);
    assert_eq!(coins(&s), before);
}

/// The killer may be at most `EventItemMaxLevelDifference` levels above the monster.
#[test]
fn a_killer_far_above_the_monster_gets_no_event_drop() {
    let mut s = drop_scene();
    let monster = s.npc_of(9022);
    s.world
        .add_event_drops("Halloween", vec![coin_drop(100.0, 1, 99, &[])]);
    // Killer level 1, monster level 5: a difference of -4. With a limit of -5 it is over.
    s.world.set_settings(loot_settings(-5));
    s.world.do_event_drop(Some(ObjectId(ME)), monster);
    assert_eq!(coins(&s), 0);
    s.world.set_settings(loot_settings(-4));
    s.world.do_event_drop(Some(ObjectId(ME)), monster);
    assert!(coins(&s) >= 2);
}

/// Without auto loot the coins land on the ground, with the killer as the owner.
#[test]
fn an_event_drop_without_auto_loot_lands_on_the_ground() {
    let mut s = Scene::new(&spawn_line(9022, X + 300, Y, Z));
    let monster = s.npc_of(9022);
    s.world
        .add_event_drops("Halloween", vec![coin_drop(100.0, 1, 99, &[])]);
    s.world.do_event_drop(Some(ObjectId(ME)), monster);
    assert_eq!(coins(&s), 0);
    let ground = s.world.ground_item_ids();
    assert_eq!(ground.len(), 1);
    let item = s.world.ground_item(ground[0]).unwrap();
    assert_eq!(item.data.item_id, 9900);
    assert!((2..=4).contains(&item.data.count));
}
