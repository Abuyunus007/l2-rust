#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! NPC AI (M4-AI-01): `AttackableAI` and the NPC movement it needs, through the world with a
//! recording client. The scenarios use small synthetic templates (they run in CI):
//! - an aggressive monster attacks a player who comes into its aggro range, chasing it with
//!   `MoveToPawn`; a passive one only fights back when it is hit,
//! - the target leaves, or the monster strays too far from its spawn: it goes home,
//! - a monster that is hit calls the members of its clan,
//! - regions that switch off take their monsters out of the think set,
//! - an NPC's path search goes through the path service like a player's.
//!
//! Java: ai/AttackableAI.java, ai/CreatureAI.java, ai/AbstractAI.java,
//! entity/actor/Attackable.java, entity/WorldRegion.java (switchAI), Creature.moveToLocation.

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_geo::block::BLOCK_CELLS;
use l2_geo::cell::NSWE_ALL;
use l2_geo::coords::{world_x, world_y};
use l2_geo::region::REGION_BLOCKS;
use l2_geo::{
    GeoEngine, GeoEngineOptions, GeoSet, NoObstacles, PathFinder, PathOptions, PathRequest, PathResult,
    Region,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::{Attack, ChangeMoveType, MoveToLocation, MoveToPawn, NpcInfo};
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

fn names(sent: &Sent, to: i32) -> Vec<String> {
    sent.iter()
        .filter(|(id, _)| *id == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

fn of<'a>(sent: &'a Sent, to: i32, name: &str) -> Vec<&'a [u8]> {
    sent.iter()
        .filter(|(id, b)| *id == to && name_of(b) == name)
        .map(|(_, b)| b.as_slice())
        .collect()
}

fn player_data(id: i32, name: &str, (x, y, z): (i32, i32, i32), items: Vec<ItemData>) -> PlayerData {
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
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

fn item(object_id: i32, item_id: i32, loc: &str, slot: usize, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        enchant_level: 0,
        loc: loc.into(),
        loc_data: i32::try_from(slot).unwrap(),
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
    }
}

fn sword() -> Vec<ItemData> {
    vec![item(0x3000_0001, 2, "PAPERDOLL", paperdoll::RHAND, 1)]
}

// ------------------------------------------------------------------ synthetic data

/// Class 0 at level 1 with plenty of HP, so a fight does not end the scenario.
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
<item id="2" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />
<stats><stat type="pAtk">50</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat><stat type="pAtkRange">40</stat></stats></item>
</list>"#;

/// 9001: an aggressive monster that does not wait to see a player (`SetAggressiveTime` 0).
/// 9002: a monster that never aggresses. 9003 / 9004: two members of the GOBLIN clan (9004 is
/// called by 9003). 9005: a monster of no clan.
const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="9001" type="Monster" level="1" name="Wolf">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="300" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9002" type="Monster" level="1" name="Hare">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="300" isAggressive="false" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9003" type="Monster" level="1" name="Goblin Scout">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="300" clanHelpRange="500" isAggressive="false"><clanList><clan>GOBLIN</clan></clanList></ai>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9004" type="Monster" level="1" name="Goblin Guard">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="300" clanHelpRange="500" isAggressive="false"><clanList><clan>GOBLIN</clan></clanList></ai>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9005" type="Monster" level="1" name="Boar">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="300" isAggressive="false" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9006" type="Monster" level="1" name="Boar Chief">
    <parameters><minions name="Privates"><npc id="9005" count="2" respawnTime="10"/></minions></parameters>
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="300" isAggressive="false" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
</list>"#;

fn synthetic_stats() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::default()),
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const ME: i32 = 0x2000_0001;

struct Scene {
    world: World,
    log: Log,
    _dir: tempfile::TempDir,
}

impl Scene {
    /// A world with the synthetic templates and the given spawns (the body of a spawn file).
    fn new(spawns: &str) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let spawn_dir = dir.path().join("data/spawns");
        std::fs::create_dir_all(&spawn_dir).unwrap();
        std::fs::write(
            spawn_dir.join("test.xml"),
            format!(r#"<?xml version="1.0"?><list enabled="true">{spawns}</list>"#),
        )
        .unwrap();
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        let spawn_data = Arc::new(SpawnData::load(dir.path(), &npcs, &SpawnLoadOptions::default()));
        let mut world = World::new(WorldRng::from_seed(20_261_019), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data(synthetic_stats())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(npcs, spawn_data);
        // Spawned before anybody is online: the regions are off, nobody thinks.
        world.spawn_all_npcs(false);
        Self {
            world,
            log: Log::default(),
            _dir: dir,
        }
    }

    fn enter(&mut self, id: i32, name: &str, pos: (i32, i32, i32), items: Vec<ItemData>) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player_data(id, name, pos, items)),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn leave(&mut self, id: i32) {
        self.world.handle(WorldCommand::Disconnected {
            session: SessionId(u64::try_from(id).unwrap()),
        });
    }

    fn ticks(&mut self, n: u32) -> Sent {
        let mut sent = Vec::new();
        for _ in 0..n {
            self.world.tick();
            sent.extend(self.log.take());
        }
        sent
    }

    /// Runs ticks until `done` or `limit` ticks; the packets of the whole run.
    fn run_until(&mut self, limit: u32, mut done: impl FnMut(&World) -> bool) -> Sent {
        let mut sent = Vec::new();
        for _ in 0..limit {
            self.world.tick();
            sent.extend(self.log.take());
            if done(&self.world) {
                return sent;
            }
        }
        panic!("not done after {limit} ticks");
    }

    /// The only NPC of a template.
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

fn dist2(a: (i32, i32, i32), b: (i32, i32, i32)) -> f64 {
    let (dx, dy) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1));
    (dx * dx + dy * dy).sqrt()
}

// ------------------------------------------------------------------ scenarios

/// A monster nobody is near does nothing, and nobody counts it in the AI: 42,000 NPCs are
/// spawned, only those in view of a player think.
#[test]
fn npcs_do_not_think_until_a_player_is_near() {
    let mut s = Scene::new(&format!(
        "{}{}",
        spawn_line(9001, X, Y, Z),
        // Far away from the player, in other regions.
        spawn_line(9001, X + 40_000, Y, Z)
    ));
    assert_eq!(s.world.thinking_npc_count(), 0);
    assert_eq!(
        s.world.timers().len(),
        0,
        "no timer per NPC while the regions are off"
    );
    s.ticks(100);
    assert_eq!(s.world.thinking_npc_count(), 0);

    s.enter(ME, "Hero", (X + 1000, Y, Z), vec![]);
    let near = s.world.npc_ids()[0];
    let far = s.world.npc_ids()[1];
    assert!(s.world.has_ai(near), "the monster in view got an AI");
    assert!(!s.world.has_ai(far), "the one far away did not");
    assert_eq!(s.world.thinking_npc_count(), 1, "and only the near one thinks");
    assert_eq!(s.world.intention_of(near), Intention::Active);
    assert_eq!(s.world.intention_of(far), Intention::Idle);
}

/// An aggressive monster attacks a player who is in its aggro range: it runs (`ChangeMoveType`),
/// chases with `MoveToPawn` at the attack range, stops (`StopMove`) and attacks.
#[test]
fn aggressive_monster_attacks_an_approaching_player() {
    let mut s = Scene::new(&spawn_line(9001, X + 250, Y, Z));
    let wolf = s.npc_of(9001);
    // The player is 250 units away, inside the 300 of the aggro range; the monster is aggressive
    // from the start (`SetAggressiveTime` 0), so it notices the player at once.
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let entering = s.log.take();
    assert!(
        names(&entering, ME).contains(&"CHANGE_MOVE_TYPE".to_owned()),
        "the monster starts running: {:?}",
        names(&entering, ME)
    );
    let run = decode::<ChangeMoveType>(of(&entering, ME, "CHANGE_MOVE_TYPE")[0]);
    assert_eq!((run.object_id, run.running), (wolf.0, true));
    assert_eq!(s.world.intention_of(wolf), Intention::Attack);
    assert_eq!(s.world.attack_target_of(wolf), Some(ObjectId(ME)));
    assert_eq!(s.world.aggro_list(wolf).len(), 1);
    assert_eq!(s.world.aggro_list(wolf)[0].attacker, ObjectId(ME));
    assert!(s.world.is_running(wolf));

    // The next think (within a second) starts the chase.
    let sent = s.run_until(60, |w| w.is_moving(wolf));
    let pawn = decode::<MoveToPawn>(of(&sent, ME, "MOVE_TO_PAWN")[0]);
    assert_eq!((pawn.object_id, pawn.target_id), (wolf.0, ME));
    // The physical attack range 40 plus both collision radii (10 and 9).
    assert_eq!(pawn.distance, 59);
    assert_eq!((pawn.x, pawn.y), (X + 250, Y));
    assert_eq!((pawn.target_x, pawn.target_y), (X, Y));

    // Java asks again every second while the monster walks (the range doubles while it moves, so
    // the first leg ends further away); it arrives (`MoveToLocation` of the arrival, `StopMove`
    // of the pawn move) and attacks.
    let sent = s.run_until(300, |w| w.is_attacking_now(wolf));
    let order = names(&sent, ME);
    assert!(order.contains(&"STOP_MOVE".to_owned()), "{order:?}");
    let attack = decode::<Attack>(of(&sent, ME, "ATTACK")[0]);
    assert_eq!(attack.attacker_id, wolf.0);
    assert_eq!(attack.first_hit.target_id, ME);
    // It stopped within its reach: 40 + 10 + 9 from the player (the walk ends a little closer
    // than the range because of Java's "move 5 units closer" rounding).
    assert!(dist2(s.pos(wolf), (X, Y, Z)) <= 59.0, "{:?}", s.pos(wolf));

    // The first hit puts it in combat (`AutoAttackStart`), and it keeps attacking.
    let more = s.ticks(200);
    assert!(!of(&more, ME, "ATTACK").is_empty());
    let starts: Vec<i32> = of(&more, ME, "AUTO_ATTACK_START")
        .iter()
        .map(|raw| decode::<l2_protocol_game::server::AutoAttackStart>(raw).target_id)
        .collect();
    assert!(starts.contains(&wolf.0), "{starts:?}");
    assert!(s.world.is_auto_attacking(wolf));
}

/// A passive monster stands next to a player for as long as it likes; when it is hit it runs
/// at the attacker.
#[test]
fn passive_monster_fights_back_only_when_hit() {
    let mut s = Scene::new(&spawn_line(9002, X + 50, Y, Z));
    let hare = s.npc_of(9002);
    s.enter(ME, "Hero", (X, Y, Z), sword());
    // 20 seconds: no aggression, no movement.
    let sent = s.ticks(400);
    assert!(of(&sent, ME, "ATTACK").is_empty(), "{:?}", names(&sent, ME));
    assert!(s.world.aggro_list(hare).is_empty());
    assert_eq!(s.world.intention_of(hare), Intention::Active);
    assert!(!s.world.is_running(hare));
    assert!(!s.world.is_moving(hare));

    // The player hits it (and keeps hitting).
    s.world.set_intention_attack(ObjectId(ME), hare);
    let mut sent = s.run_until(300, |w| w.intention_of(hare) == Intention::Attack);
    assert!(
        names(&sent, ME).contains(&"ATTACK".to_owned()),
        "the player's own attack"
    );
    assert!(s.world.is_running(hare), "setRunning on the first hit");
    assert_eq!(s.world.attack_target_of(hare), Some(ObjectId(ME)));
    assert_eq!(s.world.aggro_list(hare)[0].attacker, ObjectId(ME));

    // And hits back within a few seconds.
    sent.extend(s.ticks(300));
    let hits: Vec<Attack> = of(&sent, ME, "ATTACK")
        .iter()
        .map(|raw| decode::<Attack>(raw))
        .filter(|a| a.attacker_id == hare.0)
        .collect();
    assert!(!hits.is_empty());
    assert_eq!(hits[0].first_hit.target_id, ME);
}

/// The target is gone (it left the game): the monster, far from its spawn, walks home and
/// forgets the fight.
#[test]
fn monster_walks_home_when_its_target_leaves() {
    let spawn = (X, Y, Z);
    let mut s = Scene::new(&spawn_line(9001, spawn.0, spawn.1, spawn.2));
    let wolf = s.npc_of(9001);
    // 700 units away: out of its aggro range of 300, so the hate is put there by hand.
    s.enter(ME, "Hero", (X + 700, Y, Z), vec![]);
    s.ticks(5);
    assert!(s.world.aggro_list(wolf).is_empty());
    s.world.add_damage_hate(wolf, ObjectId(ME), 0, 100);
    // The think after the hate: attack. It chases (running) until it is 300 units from home.
    s.run_until(600, |w| {
        w.position_of(wolf).is_some_and(|p| dist2(p, (X, Y, Z)) > 350.0)
    });
    assert_eq!(s.world.intention_of(wolf), Intention::Attack);

    s.leave(ME);
    // The player left its view: the attack target is forgotten and the monster is active again.
    assert_eq!(s.world.attack_target_of(wolf), None);
    s.log.take();
    s.run_until(40, |w| w.intention_of(wolf) == Intention::MoveTo);
    assert!(!s.world.is_running(wolf), "it walks home");
    assert!(s.world.aggro_list(wolf).is_empty());
    // It arrives exactly at its spawn point and stays there.
    s.run_until(1200, |w| w.position_of(wolf) == Some(spawn) && !w.is_moving(wolf));
    s.ticks(40);
    assert_eq!(s.pos(wolf), spawn);
    assert_eq!(s.world.intention_of(wolf), Intention::Active);
}

/// `AggroDistanceCheck`: a monster that chased too far from its spawn gives up, heals and
/// returns.
#[test]
fn monster_that_strays_too_far_gives_up_and_returns_home() {
    let spawn = (X, Y, Z);
    let mut s = Scene::new(&spawn_line(9001, spawn.0, spawn.1, spawn.2));
    s.world.set_ai_settings(AiSettings {
        aggro_distance_check_enabled: true,
        aggro_distance_check_range: 400,
        ..AiSettings::default()
    });
    let wolf = s.npc_of(9001);
    s.enter(ME, "Hero", (X + 1000, Y, Z), vec![]);
    s.ticks(5);
    s.world.add_damage_hate(wolf, ObjectId(ME), 0, 100);
    s.world.set_current_hp(wolf, 100.0, true);
    // It chases past 400 units from its spawn, then the next think sends it back.
    s.run_until(900, |w| {
        w.position_of(wolf).is_some_and(|p| dist2(p, (X, Y, Z)) > 400.0)
    });
    s.run_until(80, |w| w.intention_of(wolf) == Intention::MoveTo);
    assert!(s.world.aggro_list(wolf).is_empty(), "the fight is forgotten");
    assert!(
        s.world.npc(wolf).unwrap().core.cur_hp > 100.0,
        "AggroDistanceCheckRestoreLife"
    );
    s.run_until(1500, |w| w.position_of(wolf) == Some(spawn) && !w.is_moving(wolf));
    // The player is still there and the monster does not hate it any more.
    s.ticks(100);
    assert!(s.world.aggro_list(wolf).is_empty());
    assert_eq!(s.pos(wolf), spawn);
}

/// A monster that is hit calls the members of its clan in range ("faction call"); a monster of
/// another clan does not come.
#[test]
fn a_hit_monster_calls_its_clan() {
    let body = format!(
        "{}{}{}",
        spawn_line(9003, X + 60, Y, Z),
        spawn_line(9004, X + 120, Y + 60, Z),
        spawn_line(9005, X + 120, Y - 60, Z)
    );
    let mut s = Scene::new(&body);
    let scout = s.npc_of(9003);
    let guard = s.npc_of(9004);
    let boar = s.npc_of(9005);
    s.enter(ME, "Hero", (X, Y, Z), sword());
    // Monsters spawned less than 7 seconds ago are not called.
    s.ticks(200);
    assert_eq!(s.world.intention_of(guard), Intention::Active);

    s.world.set_intention_attack(ObjectId(ME), scout);
    // The first hit makes the scout attack; the clan is called from the second hit on, because
    // the attacker is only in the scout's "attacked by" list after the first.
    s.run_until(600, |w| w.intention_of(guard) == Intention::Attack);
    assert_eq!(s.world.attack_target_of(guard), Some(ObjectId(ME)));
    assert!(
        s.world
            .aggro_list(guard)
            .iter()
            .any(|a| a.attacker == ObjectId(ME))
    );
    assert!(s.world.is_running(guard));
    assert_eq!(
        s.world.intention_of(boar),
        Intention::Active,
        "another clan stays out"
    );
    assert!(s.world.aggro_list(boar).is_empty());
}

/// Regions that switch off take their monsters out of the think set and drop their AI.
#[test]
fn a_region_that_switches_off_stops_its_monsters() {
    let mut s = Scene::new(&spawn_line(9001, X + 600, Y, Z));
    let wolf = s.npc_of(9001);
    s.world.set_settings(WorldSettings {
        grid_neighbor_turnoff_secs: 2,
        ..WorldSettings::default()
    });
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    s.ticks(60);
    assert!(s.world.has_ai(wolf));
    assert_eq!(s.world.thinking_npc_count(), 1);

    s.leave(ME);
    // 2 s after the last player left, the regions switch off.
    s.ticks(60);
    assert_eq!(s.world.thinking_npc_count(), 0);
    assert!(!s.world.has_ai(wolf));
    assert_eq!(s.world.intention_of(wolf), Intention::Idle);
    // Nothing runs for it any more: no timer, nothing to think. (The HP regeneration of a full
    // monster has stopped on its own.)
    s.ticks(100);
    assert_eq!(s.world.timers().len(), 0);
    assert_eq!(s.world.thinking_npc_count(), 0);
}

/// A monster far from its spawn when its region switches off is put back at its spawn.
#[test]
fn a_region_that_switches_off_sends_a_straying_monster_home() {
    let spawn = (X, Y, Z);
    let mut s = Scene::new(&spawn_line(9001, spawn.0, spawn.1, spawn.2));
    let wolf = s.npc_of(9001);
    s.world.set_settings(WorldSettings {
        grid_neighbor_turnoff_secs: 2,
        ..WorldSettings::default()
    });
    s.enter(ME, "Hero", (X + 1000, Y, Z), vec![]);
    s.ticks(5);
    s.world.add_damage_hate(wolf, ObjectId(ME), 0, 100);
    s.run_until(600, |w| {
        w.position_of(wolf).is_some_and(|p| dist2(p, (X, Y, Z)) > 400.0)
    });
    s.leave(ME);
    s.ticks(100);
    // (`teleToLocation` puts the creature 5 units above the ground.)
    let (x, y, _) = s.pos(wolf);
    assert_eq!((x, y), (spawn.0, spawn.1), "teleported to its spawn by switchAI");
    assert!(!s.world.is_thinking_npc(wolf));
    assert!(s.world.aggro_list(wolf).is_empty());
}

// ------------------------------------------------------------------ NPC movement and paths

/// World coordinates of region 20_18 start at (0, 0).
const RX: i32 = 20 * 2048;
const RY: i32 = 18 * 2048;
/// A wall one cell thick at geo x = RX + 1000, y = RY + 960 .. RY + 1040 (flat ground at z = 0).
const WALL_CELL_X: i32 = RX + 1000;

fn wall_engine(pathfinding: i32) -> Arc<GeoEngine> {
    let blocks: Vec<(i32, i32)> = (960 / 8..=1040 / 8).map(|by| (1000 / 8, by)).collect();
    let wall = |x: i32, y: i32| x == WALL_CELL_X && (RY + 960..RY + 1040).contains(&y);
    let mut bytes = Vec::new();
    for block in 0..REGION_BLOCKS {
        let bx = (block / 256) as i32;
        let by = (block % 256) as i32;
        if blocks.contains(&(bx, by)) {
            bytes.push(1);
            for cell in 0..(BLOCK_CELLS as i32) {
                let cx = RX + bx * 8 + cell / 8;
                let cy = RY + by * 8 + cell % 8;
                let nswe = if wall(cx, cy) { 0 } else { NSWE_ALL };
                bytes.extend_from_slice(&u16::from(nswe).to_le_bytes());
            }
        } else {
            bytes.push(0);
            bytes.extend_from_slice(&0i16.to_le_bytes());
        }
    }
    let mut set = GeoSet::new();
    assert!(set.insert(20, 18, Region::parse(&bytes).unwrap()));
    Arc::new(GeoEngine::new(
        set,
        GeoEngineOptions {
            pathfinding,
            avoid_obstructed_path_nodes: true,
        },
    ))
}

fn cell(geo_x: i32, geo_y: i32) -> (i32, i32, i32) {
    (world_x(geo_x), world_y(geo_y), 0)
}

#[derive(Clone, Default)]
struct Requests(Arc<Mutex<Vec<PathRequest>>>);

struct RecordingRequests(Requests);

impl PathRequests for RecordingRequests {
    fn request(&mut self, request: PathRequest) -> bool {
        self.0.0.lock().unwrap().push(request);
        true
    }

    fn cancel(&mut self, _requester: u64) {}
}

/// Runs a request the way a pool worker does.
fn search(engine: &GeoEngine, request: &PathRequest) -> PathResult {
    let mut finder = PathFinder::new(PathOptions::shipped()).unwrap();
    let path = finder.find_path_detailed(
        engine,
        &NoObstacles,
        request.from,
        request.to,
        request.instance_id,
        request.playable,
    );
    PathResult {
        requester: request.requester,
        id: request.id,
        path,
        elapsed: std::time::Duration::ZERO,
    }
}

fn path_found(world: &mut World, result: PathResult) {
    world.handle(WorldCommand::PathFound {
        requester: result.requester,
        request_id: result.id,
        result,
    });
}

/// A world with the wall, the recording path service, a player and one monster (no spawn).
fn wall_scene(
    player_at: (i32, i32, i32),
    wolf_at: (i32, i32, i32),
) -> (Scene, ObjectId, Arc<GeoEngine>, Requests) {
    let mut s = Scene::new("");
    let engine = wall_engine(2);
    s.world.set_geo(Arc::clone(&engine));
    let requests = Requests::default();
    s.world
        .set_path_requests(Box::new(RecordingRequests(requests.clone())));
    let wolf = s
        .world
        .spawn_npc_at(9001, wolf_at.0, wolf_at.1, wolf_at.2, 0)
        .unwrap();
    s.enter(ME, "Hero", player_at, vec![]);
    s.log.take();
    (s, wolf, engine, requests)
}

/// A monster whose line to its target is blocked asks the path service (as a non-player) and
/// walks the detour; its `MoveToLocation` goes to the players who see it.
#[test]
fn a_monster_walks_around_a_wall_by_a_found_path() {
    let me = cell(WALL_CELL_X + 10, RY + 1000);
    let wolf_at = cell(WALL_CELL_X - 10, RY + 1000);
    let (mut s, wolf, engine, requests) = wall_scene(me, wolf_at);
    // The monster is aggressive and sees the player: the first think already hates it.
    s.world.add_damage_hate(wolf, ObjectId(ME), 0, 100);
    s.ticks(50);
    let sent = requests.0.lock().unwrap().clone();
    assert!(!sent.is_empty(), "the blocked line asks for a path");
    assert_eq!(sent[0].requester, u64::try_from(wolf.0).unwrap());
    assert!(
        !sent[0].playable,
        "only players get the multi-pass line of sight filter"
    );
    assert!(!s.world.is_moving(wolf), "nothing moves until the answer is in");
    assert!(s.world.npc(wolf).unwrap().movement.is_waiting_for_path());
    s.log.take();

    let result = search(&engine, &sent[0]);
    let nodes = result.path.clone().unwrap();
    assert!(nodes.len() > 1, "there is a detour");
    path_found(&mut s.world, result);
    assert!(s.world.is_moving(wolf));
    let out = s.log.take();
    // The monster is on a calculated path: `MoveToLocation`, not `MoveToPawn`.
    let moves: Vec<MoveToLocation> = of(&out, ME, "MOVE_TO_LOCATION")
        .iter()
        .map(|raw| decode::<MoveToLocation>(raw))
        .collect();
    assert_eq!(moves.len(), 1, "{:?}", names(&out, ME));
    assert_eq!(moves[0].object_id, wolf.0);
    assert!(of(&out, ME, "MOVE_TO_PAWN").is_empty());

    // It walks around the wall's end and ends up at the player. Its think asks again every two
    // seconds (Java recalculates the route then); the pool answers each request.
    let wall_x = world_x(WALL_CELL_X);
    let mut answered = 1;
    let mut sent = Vec::new();
    for _ in 0..2000 {
        s.world.tick();
        sent.extend(s.log.take());
        let all = requests.0.lock().unwrap().clone();
        while answered < all.len() {
            path_found(&mut s.world, search(&engine, &all[answered]));
            answered += 1;
        }
        if s.world.is_attacking_now(wolf) {
            break;
        }
        // Never inside the wall's cell.
        let (x, y, _) = s.pos(wolf);
        assert!(!((x - wall_x).abs() < 8 && (world_y(RY + 960)..world_y(RY + 1040)).contains(&y)));
    }
    assert!(
        !of(&sent, ME, "ATTACK").is_empty(),
        "it got to the player and attacked"
    );
    let (px, _, _) = s.pos(wolf);
    assert!(px > wall_x, "on the player's side of the wall");
}

/// When the path to a monster's target is not found, the monster looks for the closest point it
/// can reach (a search for each candidate, one answer after the other), then walks there.
#[test]
fn a_monster_without_a_path_tries_the_closest_reachable_points() {
    let me = cell(WALL_CELL_X + 10, RY + 1000);
    let wolf_at = cell(WALL_CELL_X - 10, RY + 1000);
    let (mut s, wolf, engine, requests) = wall_scene(me, wolf_at);
    s.world.add_damage_hate(wolf, ObjectId(ME), 0, 100);
    s.ticks(50);
    let first = requests.0.lock().unwrap().clone();
    assert_eq!(first.len(), 1);
    // The service finds nothing for the player's place.
    let mut none = search(&engine, &first[0]);
    none.path = Err(l2_geo::PathFailure::NoGeo);
    path_found(&mut s.world, none);
    // A second request is out at once: the closest-point loop, for a candidate point.
    let all = requests.0.lock().unwrap().clone();
    assert_eq!(all.len(), 2, "the loop asks for its first candidate");
    assert_ne!(all[1].to, all[0].to);
    assert!(!s.world.is_moving(wolf));
    // Answer the candidates with real searches until the loop ends.
    let mut answered = 1;
    loop {
        let all = requests.0.lock().unwrap().clone();
        if all.len() <= answered {
            break;
        }
        let request = all[answered].clone();
        answered += 1;
        path_found(&mut s.world, search(&engine, &request));
        assert!(answered < 40, "the loop is bounded");
    }
    assert!(s.world.is_moving(wolf), "it walks the best candidate's path");
}

/// An answer for a request that is not the live one is dropped; an NPC that stopped does not
/// start walking from an old answer.
#[test]
fn a_stale_path_answer_for_an_npc_is_dropped() {
    let me = cell(WALL_CELL_X + 10, RY + 1000);
    let wolf_at = cell(WALL_CELL_X - 10, RY + 1000);
    let (mut s, wolf, engine, requests) = wall_scene(me, wolf_at);
    s.world.add_damage_hate(wolf, ObjectId(ME), 0, 100);
    s.ticks(50);
    let sent = requests.0.lock().unwrap().clone();
    let mut stale = search(&engine, &sent[0]);
    stale.id += 100;
    path_found(&mut s.world, stale);
    assert!(!s.world.is_moving(wolf));
    // The player leaves: the monster forgets it, and the live answer finds nobody waiting.
    s.leave(ME);
    path_found(&mut s.world, search(&engine, &sent[0]));
    assert!(!s.world.is_moving(wolf));
}

/// The packets an NPC move sends to a player who comes into view of a monster that is moving.
#[test]
fn a_player_who_arrives_is_told_where_a_moving_monster_goes() {
    let mut s = Scene::new("");
    let wolf = s.world.spawn_npc_at(9001, X + 300, Y, Z, 0).unwrap();
    // A player far away keeps the region on and makes the monster walk (hate by hand).
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    s.world.add_damage_hate(wolf, ObjectId(ME), 0, 100);
    s.run_until(80, |w| w.is_moving(wolf));
    s.log.take();
    // A second player enters the monster's view.
    s.enter(0x2000_0002, "Late", (X + 100, Y + 50, Z), vec![]);
    let out = s.log.take();
    let n = names(&out, 0x2000_0002);
    let info = n.iter().position(|n| n == "NPC_INFO").unwrap();
    let go = n.iter().position(|n| n == "MOVE_TO_LOCATION").unwrap();
    assert!(info < go, "NpcInfo first, then where it goes: {n:?}");
    let raw = of(&out, 0x2000_0002, "NPC_INFO");
    let npc_info = decode::<NpcInfo>(
        raw.iter()
            .find(|r| decode::<NpcInfo>(r).object_id == wolf.0)
            .unwrap(),
    );
    assert!(npc_info.running, "NpcInfo shows the monster running");
}

/// Minions walk to their leader when it is more than 200 units away, at its speed type.
#[test]
fn minions_follow_their_leader() {
    let mut s = Scene::new(&spawn_line(9006, X + 400, Y, Z));
    let chief = s.npc_of(9006);
    let minions = s.world.minions_of(chief).to_vec();
    assert_eq!(minions.len(), 2);
    assert!(
        minions
            .iter()
            .all(|m| s.world.npc(*m).unwrap().leader == Some(chief))
    );
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    s.ticks(100);
    // The leader goes 280 units away (as a teleport would take it; that is still inside its
    // drift range, so it does not walk home).
    let start: Vec<(i32, i32, i32)> = minions.iter().map(|m| s.pos(*m)).collect();
    s.world.set_position(chief, X + 680, Y, Z);
    let sent = s.run_until(100, |w| minions.iter().any(|m| w.is_moving(*m)));
    assert!(!of(&sent, ME, "MOVE_TO_LOCATION").is_empty());
    s.run_until(1500, |w| {
        let leader = w.position_of(chief).unwrap();
        minions
            .iter()
            .all(|m| !w.is_moving(*m) && dist2(w.position_of(*m).unwrap(), leader) <= 200.0)
    });
    let end: Vec<(i32, i32, i32)> = minions.iter().map(|m| s.pos(*m)).collect();
    assert_ne!(start, end, "they walked");
    for m in &minions {
        assert!(!s.world.is_running(*m), "the leader walks, so they do");
    }
}

/// A hit leader calls its minions: they get the attacker's hate, ten times a minion's own.
#[test]
fn a_hit_leader_calls_its_minions() {
    let mut s = Scene::new(&spawn_line(9006, X + 50, Y, Z));
    let chief = s.npc_of(9006);
    let minions = s.world.minions_of(chief).to_vec();
    s.enter(ME, "Hero", (X, Y, Z), sword());
    s.ticks(200);
    s.world.do_attack(ObjectId(ME), chief);
    s.run_until(200, |w| w.intention_of(chief) == Intention::Attack);
    for m in &minions {
        let aggro = s.world.aggro_list(*m);
        assert_eq!(aggro.len(), 1, "{m:?}");
        assert_eq!(aggro[0].attacker, ObjectId(ME));
        assert!(aggro[0].hate >= 10, "MASTER_AGGRO_AMOUNT: {}", aggro[0].hate);
    }
    // And they come: each minion goes to attack.
    s.run_until(400, |w| {
        minions.iter().all(|m| w.intention_of(*m) == Intention::Attack)
    });
}

// ------------------------------------------------------------------ the real world

const REFERENCE: &str = r"C:\l2-rust\l2jmobius\dist\game";

/// With the real datapack (skipped without it): about 42,000 NPCs spawn and none of them thinks;
/// when a player stands in a hunting ground, only the monsters around it do, and a tick stays
/// cheap. Prints the numbers.
#[test]
fn the_real_world_thinks_only_near_players() {
    let root = std::path::PathBuf::from(REFERENCE);
    if !root.join("data/stats/npcs").is_dir() || !root.join("data/spawns").is_dir() {
        eprintln!("skipped: {REFERENCE} is not available");
        return;
    }
    let npcs = Arc::new(NpcTemplates::load(&root, &NpcLoadOptions::default()));
    let spawns = Arc::new(SpawnData::load(&root, &npcs, &SpawnLoadOptions::default()));
    let mut world = World::new(WorldRng::from_seed(5), Box::new(NoOutbox));
    world.set_static_data(Box::new(Data(synthetic_stats())));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_npc_data(Arc::clone(&npcs), Arc::clone(&spawns));
    let report = world.spawn_all_npcs(false);
    assert!(report.npcs > 10_000, "{report:?}");
    assert_eq!(world.thinking_npc_count(), 0, "nobody is online");
    let timers_without_players = world.timers().len();
    assert_eq!(
        timers_without_players, 0,
        "no timer per NPC while the regions are off"
    );

    // A player in the middle of the biggest crowd of aggressive monsters of a 2000 x 2000 area.
    let mut best = (0usize, (0, 0, 0));
    let monsters: Vec<(i32, i32, i32)> = spawns
        .spawns()
        .iter()
        .filter(|s| s.territory.is_none() && s.location.x != 0)
        .filter(|s| {
            npcs.get(s.npc_id)
                .is_some_and(|t| t.npc_type == "Monster" && t.is_aggressive)
        })
        .map(|s| (s.location.x, s.location.y, s.location.z))
        .collect();
    for probe in monsters.iter().step_by(25) {
        let near = monsters
            .iter()
            .filter(|m| (m.0 - probe.0).abs() < 1000 && (m.1 - probe.1).abs() < 1000)
            .count();
        if near > best.0 {
            best = (near, *probe);
        }
    }
    let (_, at) = best;
    let log = Log::default();
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(1),
        player: Box::new(player_data(ME, "Hero", at, vec![])),
        sink: Box::new(Sink(ME, log.clone())),
        ip: None,
    });
    let thinking = world.thinking_npc_count();
    let started = std::time::Instant::now();
    let ticks = 400u32;
    let mut worst = std::time::Duration::ZERO;
    for _ in 0..ticks {
        let t = std::time::Instant::now();
        world.tick();
        worst = worst.max(t.elapsed());
        log.take();
    }
    let mean = started.elapsed() / ticks;
    eprintln!(
        "{} NPCs in the world, {thinking} think after the player entered at {at:?}; {} timers; {ticks} ticks: mean {mean:?}, worst {worst:?} (debug build)",
        report.npcs,
        world.timers().len()
    );
    assert!(thinking > 0 && thinking < 3000, "{thinking}");
    assert!(thinking < report.npcs / 10);
}
