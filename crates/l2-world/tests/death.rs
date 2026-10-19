#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Death, corpses, respawn and the restart point (M4-DEATH-01).
//!
//! The scenarios use small synthetic templates and spawn files, so they run in CI. They check the
//! packets and their order (against Java's `Creature.doDie`, `Playable.doDie`, `Player.doDie`,
//! `Npc.onDecay`, `Spawn.decreaseCount`, `RequestRestartPoint`) and the timing of the corpse
//! and the respawn on the world's 50 ms ticks.
//! Java: entity/actor/{Creature,Playable,Player,Npc,Attackable}.java, entity/spawns/Spawn.java,
//! taskmanagers/{DecayTaskManager,RespawnTaskManager}.java, network/clientpackets/RequestRestartPoint.java

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, MapRegions, NpcLoadOptions, NpcTemplates,
    PlayerTemplates, SkillTemplates, SpawnData, SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{DeleteObject, Die, NpcInfo, Revive, StatusUpdate, TeleportToLocation};
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

/// The item that lets its owner stand up in place (Java `allow_self_resurrection`).
const FEATHER: i32 = 10649;

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
    fn access_level(&self, level: i32) -> AccessLevelStatic {
        if level >= 100 {
            AccessLevelStatic {
                is_gm: true,
                ..AccessLevelStatic::USER
            }
        } else {
            AccessLevelStatic::USER
        }
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.0.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_allows_self_resurrection(&self, item_id: i32) -> bool {
        item_id == FEATHER
    }
    fn access_level_allows_fixed_res(&self, level: i32) -> bool {
        level == 20
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    P::decode(&mut PacketReader::new(&raw[1..])).unwrap()
}

/// The packet names one client got, in order.
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

/// `want` appears in `names` in this order, other packets in between allowed.
fn has_in_order(names: &[String], want: &[&str]) -> bool {
    let mut it = names.iter();
    want.iter().all(|w| it.any(|n| n == w))
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
        cur_hp: 100.0,
        max_hp: 100.0,
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

// ------------------------------------------------------------------ synthetic data

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

const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />
<stats><stat type="pAtk">50</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat><stat type="pAtkRange">40</stat></stats></item>
<item id="10649" type="EtcItem" name="Scroll of Self-Resurrection"><set name="etcitem_type" val="SCROLL" /></item>
</list>"#;

/// A weak monster (300 HP) with a 5 second corpse, a folk NPC and a monster with another
/// corpse time (the default of 7 seconds).
const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="9001" type="Monster" level="1" name="Weak Monster">
    <race>FAIRY</race>
    <corpseTime>5</corpseTime>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9002" type="Monster" level="1" name="Other Monster">
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="9004" type="Monster" level="30" name="Master"><parameters>
    <minions name="Privates"><npc id="9002" count="2" respawnTime="10"/></minions></parameters></npc>
  <npc id="9003" type="Folk" level="10" name="Villager">
    <corpseTime>5</corpseTime>
    <collision><radius normal="8"/><height normal="23"/></collision>
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
const OTHER: i32 = 0x2000_0002;
const T: &str = r#"<territory minZ="-3300" maxZ="-2900"><node x="-71500" y="258100"/><node x="-71200" y="258100"/>
  <node x="-71200" y="258500"/><node x="-71500" y="258500"/></territory>"#;

/// A temporary datapack root with spawn files, one empty zone file and one map region (the
/// default restart region of `MapRegionData`).
struct Root(PathBuf);

impl Root {
    fn new(tag: &str, spawn_files: &[(&str, String)]) -> Self {
        let dir = std::env::temp_dir().join(format!("l2-world-death-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        for sub in ["spawns", "zones", "mapregion"] {
            std::fs::create_dir_all(dir.join("data").join(sub)).unwrap();
        }
        for (name, body) in spawn_files {
            std::fs::write(dir.join("data/spawns").join(name), body).unwrap();
        }
        std::fs::write(
            dir.join("data/zones/test.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\"></list>\n",
        )
        .unwrap();
        std::fs::write(
            dir.join("data/mapregion/talking_island_town.xml"),
            "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\">\n\
             <region name=\"talking_island_town\" town=\"Talking Island Village\" castle=\"0\" locId=\"911\" bbs=\"2\">\n\
             <respawnPoint X=\"-84318\" Y=\"244579\" Z=\"-3730\" />\n\
             </region>\n</list>\n",
        )
        .unwrap();
        Self(dir)
    }
}

impl Drop for Root {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn spawn_file(body: &str) -> String {
    format!(r#"<?xml version="1.0"?><list enabled="true">{body}</list>"#)
}

struct Scene {
    world: World,
    log: Log,
    _root: Root,
}

impl Scene {
    fn new(tag: &str, seed: u64, spawns: &str) -> Self {
        Self::with_settings(tag, seed, spawns, DeathSettings::default())
    }

    fn with_settings(tag: &str, seed: u64, spawns: &str, death: DeathSettings) -> Self {
        let root = Root::new(tag, &[("a.xml", spawn_file(spawns))]);
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(seed), Box::new(NoOutbox));
        world.set_settings(WorldSettings {
            death: DeathSettings {
                disconnect_after_death: false,
                ..death
            },
            ..WorldSettings::default()
        });
        world.set_static_data(Box::new(Data(synthetic_stats())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_zone_data(
            Arc::new(Zones::load(&root.0)),
            Arc::new(MapRegions::load(&root.0)),
        );
        let npcs = Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(&root.0, &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.spawn_all_npcs(false);
        Self {
            world,
            log,
            _root: root,
        }
    }

    fn enter(&mut self, id: i32, name: &str, pos: (i32, i32, i32), items: Vec<ItemData>) {
        self.enter_data(player_data(id, name, pos, items));
    }

    fn enter_data(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input,
        });
    }

    fn click(&mut self, id: i32, target: ObjectId) {
        self.input(
            id,
            ClientInput::Action {
                object_id: target.0,
                action_id: 0,
            },
        );
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
        }
    }

    /// Ticks until `done`; the tick count and the packets of the run.
    fn run_until(&mut self, limit: u32, mut done: impl FnMut(&World) -> bool) -> (u32, Sent) {
        let mut sent = Vec::new();
        for n in 1..=limit {
            self.world.tick();
            sent.extend(self.log.take());
            if done(&self.world) {
                return (n, sent);
            }
        }
        panic!("not done after {limit} ticks");
    }

    /// The first NPC of template `npc_id`.
    fn npc_of(&self, npc_id: i32) -> Option<ObjectId> {
        self.world
            .npc_ids()
            .into_iter()
            .find(|id| self.world.npc(*id).unwrap().npc_id == npc_id)
    }

    /// The monster dies of a hit by `killer`.
    fn kill(&mut self, victim: ObjectId, killer: Option<ObjectId>) {
        self.world.reduce_current_hp(victim, 1_000_000.0, killer);
    }
}

/// Ticks for `ms` milliseconds (Java delay -> world ticks).
fn ticks_of(ms: u32) -> u32 {
    ms.div_ceil(50).max(1)
}

const ONE_MONSTER: &str = r#"<spawn><npc id="9001" x="-71338" y="258271" z="-3104" heading="77" respawnDelay="20" respawnRandom="4"/></spawn>"#;

// ------------------------------------------------------------------ NPC death

/// The monster dies: its listeners and the players around it get Java's packets in Java's order
/// (HP bar, `StopMove`, HP bar again, `Die`), the corpse stays, and it is seen as dead.
#[test]
fn an_npc_dies_with_java_packets_and_stays_as_a_corpse() {
    let mut s = Scene::new("npc-die", 5, ONE_MONSTER);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let npc = s.npc_of(9001).unwrap();
    s.click(ME, npc); // target: the player listens to its HP
    s.log.take();

    s.kill(npc, Some(ObjectId(ME)));
    let sent = s.log.take();
    let me = names(&sent, ME);
    assert!(
        has_in_order(&me, &["STATUS_UPDATE", "STOP_MOVE", "STATUS_UPDATE", "DIE"]),
        "{me:?}"
    );
    let die: Die = decode(of(&sent, ME, "DIE")[0]);
    assert_eq!(
        die,
        Die {
            object_id: npc.0,
            can_teleport: true,
            to_clan_hall: false,
            to_castle: false,
            to_siege_hq: false,
            sweepable: false,
            fixed_res: false,
            to_fortress: false,
        }
    );
    // The two HP bars: the hit and `doDie`'s own broadcast.
    let bars: Vec<StatusUpdate> = of(&sent, ME, "STATUS_UPDATE")
        .iter()
        .map(|raw| decode::<StatusUpdate>(raw))
        .filter(|u| u.object_id == npc.0)
        .collect();
    assert_eq!(bars.len(), 2);
    assert!(bars.iter().all(|u| u.attributes.contains(&(status::CUR_HP, 0))));

    assert!(s.world.is_dead(npc));
    assert!(s.world.has_decay_task(npc));
    assert!(!s.world.is_hp_mp_regenerating(npc), "the regeneration task stops");
    assert!(s.world.npc(npc).is_some(), "the corpse is still in the world");
    // The aggro list is kept until the corpse decays: the rewards read it.
    assert_eq!(s.world.aggro_list(npc).len(), 1);
    // Killing is only possible one time.
    assert!(!s.world.do_die(npc, Some(ObjectId(ME))));
    assert!(s.log.take().is_empty());

    // A player who comes by now sees the corpse lying there.
    s.enter(OTHER, "Passer", (X + 10, Y, Z), vec![]);
    let seen = s.log.take();
    let info: NpcInfo = decode(of(&seen, OTHER, "NPC_INFO")[0]);
    assert!(info.alike_dead);
}

/// The corpse stays `corpseTime` seconds, then every player gets `DeleteObject`; after the
/// spawn's respawn delay (`respawnDelay` +- `respawnRandom`) the same object comes back at the
/// spawn point, alive and at full health.
#[test]
fn the_corpse_decays_and_the_npc_respawns_at_its_spawn() {
    let mut s = Scene::new("npc-respawn", 5, ONE_MONSTER);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let npc = s.npc_of(9001).unwrap();
    s.log.take();
    assert_eq!(
        (s.world.spawn_current_count(0), s.world.spawn_scheduled_count(0)),
        (1, 0)
    );

    s.kill(npc, Some(ObjectId(ME)));
    s.log.take();

    // `corpseTime` is 5 seconds: the decay timer fires 100 ticks after the death.
    let (n, sent) = s.run_until(1000, |w| w.npc(npc).is_none());
    assert_eq!(n, ticks_of(5_000));
    let deletes: Vec<i32> = of(&sent, ME, "DELETE_OBJECT")
        .iter()
        .map(|raw| decode::<DeleteObject>(raw).object_id)
        .collect();
    assert_eq!(deletes, vec![npc.0]);
    // `Spawn.decreaseCount`: one less in the world, a respawn in progress.
    assert_eq!(
        (s.world.spawn_current_count(0), s.world.spawn_scheduled_count(0)),
        (0, 1)
    );
    assert!(s.world.has_pending_respawn(npc));
    assert!(!s.world.has_decay_task(npc));
    assert!(s.world.object(npc).is_none());

    // 20 s +- 4 s: `Rnd.get(16000, 24000)` ms.
    let (n, sent) = s.run_until(2000, |w| w.npc(npc).is_some());
    assert!(
        (ticks_of(16_000)..=ticks_of(24_000)).contains(&n),
        "respawned after {n} ticks"
    );
    // The same object (Java re-initialises the same `Npc`), at the spawn point, alive, full HP.
    let back = s.world.npc(npc).unwrap();
    assert_eq!(
        (back.core.x, back.core.y, back.core.z, back.core.heading),
        (X, Y, Z, 77)
    );
    assert_eq!(back.core.cur_hp, back.core.max_hp);
    assert!(back.core.cur_hp > 0.0);
    assert!(!s.world.is_dead(npc));
    assert_eq!(
        (s.world.spawn_current_count(0), s.world.spawn_scheduled_count(0)),
        (1, 0)
    );
    assert!(!s.world.has_pending_respawn(npc));
    assert_eq!(s.world.last_spawn(0), Some(npc));
    // The players around see it come.
    let infos: Vec<NpcInfo> = of(&sent, ME, "NPC_INFO")
        .iter()
        .map(|raw| decode::<NpcInfo>(raw))
        .collect();
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].object_id, npc.0);
    assert!(!infos[0].alike_dead);

    // And it can die and come back again.
    s.kill(npc, Some(ObjectId(ME)));
    assert!(s.world.is_dead(npc));
    s.run_until(2000, |w| w.npc(npc).is_none());
    s.run_until(2000, |w| w.npc(npc).is_some());
    assert!(!s.world.is_dead(npc));
}

/// Same seed, same respawn tick; the respawn time is drawn from the world's RNG.
#[test]
fn the_respawn_delay_comes_from_the_world_rng() {
    let respawn_tick = |seed: u64| {
        let mut s = Scene::new("npc-seed", seed, ONE_MONSTER);
        let npc = s.npc_of(9001).unwrap();
        s.kill(npc, None);
        s.run_until(1000, |w| w.npc(npc).is_none());
        s.run_until(2000, |w| w.npc(npc).is_some()).0
    };
    assert_eq!(respawn_tick(11), respawn_tick(11));
    let ticks: std::collections::HashSet<u32> = (0..20).map(respawn_tick).collect();
    assert!(ticks.len() > 3, "the delay is random: {ticks:?}");
}

/// Without `respawnRandom` the delay is exactly `respawnDelay`.
#[test]
fn a_fixed_respawn_delay_is_exact() {
    let mut s = Scene::new(
        "npc-fixed",
        5,
        r#"<spawn><npc id="9002" x="-71338" y="258271" z="-3104" respawnDelay="30"/></spawn>"#,
    );
    let npc = s.npc_of(9002).unwrap();
    s.kill(npc, None);
    // The default corpse time of 7 seconds.
    let (n, _) = s.run_until(1000, |w| w.npc(npc).is_none());
    assert_eq!(n, ticks_of(7_000));
    let (n, _) = s.run_until(2000, |w| w.npc(npc).is_some());
    assert_eq!(n, ticks_of(30_000));
}

/// A spawn without a delay does not respawn (`_doRespawn = _respawnMinDelay > 0`).
#[test]
fn a_spawn_without_a_respawn_delay_does_not_come_back() {
    let mut s = Scene::new(
        "npc-none",
        5,
        r#"<spawn><npc id="9001" x="-71338" y="258271" z="-3104"/></spawn>"#,
    );
    let npc = s.npc_of(9001).unwrap();
    s.kill(npc, None);
    s.run_until(1000, |w| w.npc(npc).is_none());
    assert!(!s.world.has_pending_respawn(npc));
    assert_eq!(s.world.spawn_current_count(0), 0);
    s.ticks(5000);
    assert_eq!(s.world.npc_count(), 0);
}

/// `Npc.deleteMe()` is `onDecay()`: the spawn counts it out and schedules the respawn, like a
/// corpse that decayed. The day/night manager stops the respawn first, so a day NPC does not come
/// back at night.
#[test]
fn deleting_an_npc_respawns_it_unless_the_spawn_was_stopped() {
    let mut s = Scene::new("npc-delete", 5, ONE_MONSTER);
    let npc = s.npc_of(9001).unwrap();
    assert!(s.world.delete_npc(npc));
    assert!(s.world.npc(npc).is_none());
    assert!(s.world.has_pending_respawn(npc));
    s.run_until(2000, |w| w.npc(npc).is_some());

    let mut s = Scene::new(
        "npc-daynight",
        5,
        r#"<spawn><npc id="9001" x="-71338" y="258271" z="-3104" respawnDelay="10" periodOfDay="day"/></spawn>"#,
    );
    // The scene starts by day: the day creature stands.
    assert_eq!(s.world.npc_count(), 1);
    // Night falls: `stopRespawn()`, then `deleteMe()`: it goes and stays away.
    s.world.set_night(true);
    assert_eq!(s.world.npc_count(), 0);
    s.ticks(2000);
    assert_eq!(s.world.npc_count(), 0, "nothing comes back at night");
    // Day again: `startRespawn()` and a new one.
    assert_eq!(s.world.set_night(false), 1);
    assert_eq!(s.world.npc_count(), 1);
}

/// A folk NPC killed by a GM command dies without the AI that sends `Die`: Java only sends the
/// packet through `notifyActionDeath`, which needs an AI (`hasAI()`).
#[test]
fn an_npc_without_an_ai_dies_without_the_die_packet() {
    let mut s = Scene::new(
        "npc-folk",
        5,
        r#"<spawn><npc id="9003" x="-71338" y="258271" z="-3104" respawnDelay="5"/></spawn>"#,
    );
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let npc = s.npc_of(9003).unwrap();
    s.log.take();
    s.kill(npc, Some(ObjectId(ME)));
    let me = names(&s.log.take(), ME);
    assert!(me.contains(&"STOP_MOVE".to_owned()), "{me:?}");
    assert!(!me.contains(&"DIE".to_owned()), "{me:?}");
    assert!(s.world.is_dead(npc));
    s.run_until(1000, |w| w.npc(npc).is_none());
    s.run_until(1000, |w| w.npc(npc).is_some());
}

/// A master monster that comes back finds its minions still around (they follow `leader`, and the
/// master keeps its object id), so only the missing ones are made (`MinionList.spawnMinions`).
#[test]
fn a_respawned_master_keeps_its_minions_and_makes_only_the_missing_ones() {
    let mut s = Scene::new(
        "master",
        5,
        r#"<spawn><npc id="9004" x="-71338" y="258271" z="-3104" respawnDelay="30"/></spawn>"#,
    );
    let master = s.npc_of(9004).unwrap();
    assert_eq!(s.world.npc_count(), 3, "the master and two minions");
    s.kill(master, None);
    s.run_until(1000, |w| w.npc(master).is_none());
    assert_eq!(s.world.npc_count(), 2, "a plain monster's minions outlive it");
    // One minion dies meanwhile and decays: it does not respawn on its own.
    let minion = s.npc_of(9002).unwrap();
    s.kill(minion, None);
    s.run_until(1000, |w| w.npc(minion).is_none());
    assert_eq!(s.world.npc_count(), 1);
    s.run_until(2000, |w| w.npc(master).is_some());
    assert_eq!(s.world.npc_count(), 3, "one minion was missing, one made");
    let leaders = s
        .world
        .npc_ids()
        .into_iter()
        .filter(|id| s.world.npc(*id).unwrap().leader == Some(master))
        .count();
    assert_eq!(leaders, 2);
}

// ------------------------------------------------------------------ spawn groups

/// A `random` group replaces the unit that died with a freshly rolled one, after the rolled
/// entry's respawn delay (counted from the decay, when `decreaseCount` runs).
#[test]
fn a_random_group_replaces_a_dead_unit_after_the_entry_delay() {
    let group = format!(
        r#"<spawn zone="g1" selection="random" maximumNpc="3">{T}
             <npc id="9001" count="3" respawnDelay="10"/><npc id="9002" count="3" respawnDelay="10"/></spawn>"#
    );
    let mut s = Scene::new("group-random", 9, &group);
    assert_eq!(s.world.npc_count(), 3);
    let before = s.world.npc_ids();
    let victim = before[0];
    s.kill(victim, None);
    s.run_until(1000, |w| w.npc(victim).is_none());
    assert_eq!(s.world.npc_count(), 2);
    // The delay is 10 s from the decay.
    let (n, _) = s.run_until(1000, |w| w.npc_count() == 3);
    assert_eq!(n, ticks_of(10_000));
    // A new object, not the old one: the group makes a new unit (`slot.doSpawn(false)`).
    assert!(s.world.npc(victim).is_none());
    assert_eq!(s.world.npc_ids().len(), 3);
    // The slot itself does not respawn (`_doRespawn` is off in a rolling group).
    s.ticks(2000);
    assert_eq!(s.world.npc_count(), 3);
}

/// An `all` group slot with a delay respawns itself, like a fixed spawn.
#[test]
fn an_all_group_slot_respawns_itself() {
    let group = format!(
        r#"<spawn zone="g1" selection="all" maximumNpc="2">{T}
             <npc id="9001" count="2" respawnDelay="10"/></spawn>"#
    );
    let mut s = Scene::new("group-all", 9, &group);
    assert_eq!(s.world.npc_count(), 2);
    let victim = s.world.npc_ids()[0];
    s.kill(victim, None);
    s.run_until(1000, |w| w.npc(victim).is_none());
    let (n, _) = s.run_until(1000, |w| w.npc(victim).is_some());
    assert_eq!(n, ticks_of(10_000));
    assert_eq!(s.world.npc_count(), 2);
    // A territory spawn respawns at a fresh place inside its territory.
    let back = s.world.npc(victim).unwrap();
    assert!((-71_500..=-71_200).contains(&back.core.x) && (258_100..=258_500).contains(&back.core.y));
}

// ------------------------------------------------------------------ player death

fn sword_items() -> Vec<ItemData> {
    vec![item(0x3000_0001, 2, "PAPERDOLL", paperdoll::RHAND, 1)]
}

/// A player dies: target forgotten, movement stopped, `Die` to the player first and to those who
/// see him, the HP bar of the player 50 ms later; no regeneration, no decay without
/// `DisconnectAfterDeath`.
#[test]
fn a_player_dies_with_java_packets_in_java_order() {
    let mut s = Scene::new("player-die", 5, ONE_MONSTER);
    s.enter(ME, "Hero", (X, Y, Z), sword_items());
    s.enter(OTHER, "Friend", (X + 20, Y, Z), vec![]);
    let monster = s.npc_of(9001).unwrap();
    s.click(ME, monster); // a target to lose
    s.log.take();

    s.kill(ObjectId(ME), Some(monster));
    let sent = s.log.take();
    let me = names(&sent, ME);
    // Playable.doDie: setTarget(null), stopMove, broadcastStatusUpdate (a timer), notify the AI.
    assert!(
        has_in_order(&me, &["TARGET_UNSELECTED", "STOP_MOVE", "DIE"]),
        "{me:?}"
    );
    let friend = names(&sent, OTHER);
    assert!(
        has_in_order(&friend, &["TARGET_UNSELECTED", "STOP_MOVE", "DIE"]),
        "{friend:?}"
    );
    let die: Die = decode(of(&sent, ME, "DIE")[0]);
    assert_eq!(die.object_id, ME);
    assert!(die.can_teleport);
    assert!(!die.fixed_res && !die.sweepable);
    let die: Die = decode(of(&sent, OTHER, "DIE")[0]);
    assert_eq!(die.object_id, ME, "the same packet to the players around");

    assert!(s.world.is_dead(ObjectId(ME)));
    assert!(!s.world.is_hp_mp_regenerating(ObjectId(ME)));
    assert!(!s.world.has_decay_task(ObjectId(ME)));
    assert_eq!(s.world.player(ObjectId(ME)).unwrap().core.cur_hp, 0.0);
    assert_eq!(s.world.intention_of(ObjectId(ME)), Intention::Idle);
    assert!(!s.world.do_die(ObjectId(ME), Some(monster)), "only one death");

    // The status update of the player's own bars follows on the next tick.
    s.ticks(2);
    let later = s.log.take();
    let me = names(&later, ME);
    assert!(me.contains(&"STATUS_UPDATE".to_owned()), "{me:?}");
}

/// A dead player cannot attack, walk or use items (Java's `isAlikeDead()` and
/// `isMovementDisabled()` checks); a monster does not hit him either.
#[test]
fn a_dead_player_cannot_act() {
    let mut s = Scene::new("player-dead-acts", 5, ONE_MONSTER);
    s.enter(ME, "Hero", (X, Y, Z), sword_items());
    let monster = s.npc_of(9001).unwrap();
    s.kill(ObjectId(ME), Some(monster));
    s.ticks(5);
    s.log.take();

    // Attack: ActionFailed, no Attack packet.
    s.click(ME, monster);
    s.click(ME, monster);
    s.ticks(40);
    let sent = s.log.take();
    let me = names(&sent, ME);
    assert!(!me.contains(&"ATTACK".to_owned()), "{me:?}");
    assert_eq!(s.world.aggro_list(monster).len(), 0);
    // Walk: refused.
    s.input(
        ME,
        ClientInput::MoveTo {
            target_x: X + 100,
            target_y: Y,
            target_z: Z,
            origin_x: X,
            origin_y: Y,
            origin_z: Z,
            movement_mode: 1,
        },
    );
    s.ticks(5);
    let sent = s.log.take();
    let me = names(&sent, ME);
    assert!(!me.contains(&"MOVE_TO_LOCATION".to_owned()), "{me:?}");
    assert!(!s.world.is_moving(ObjectId(ME)));
    // Use the sword: refused.
    s.input(
        ME,
        ClientInput::UseItem {
            object_id: 0x3000_0001,
            ctrl_pressed: false,
        },
    );
    s.ticks(5);
    let me = names(&s.log.take(), ME);
    assert!(!me.contains(&"INVENTORY_UPDATE".to_owned()), "{me:?}");
}

/// A character stored dead enters the world dead and is told with `Die`.
#[test]
fn a_character_stored_dead_gets_die_when_it_enters() {
    let mut s = Scene::new("player-login-dead", 5, ONE_MONSTER);
    let mut data = player_data(ME, "Hero", (X, Y, Z), vec![]);
    data.cur_hp = 0.0;
    s.enter_data(data);
    let sent = s.log.take();
    assert!(s.world.is_dead(ObjectId(ME)));
    let me = names(&sent, ME);
    let die_at = me.iter().position(|n| n == "DIE").expect("Die");
    // After the welcome message, before the skill cool times, like EnterWorld.
    let welcome_at = me.iter().position(|n| n == "SYSTEM_MESSAGE").unwrap();
    let cool_at = me.iter().rposition(|n| n == "SKILL_COOL_TIME").unwrap();
    assert!(welcome_at < die_at && die_at < cool_at, "{me:?}");
    assert_eq!(me.iter().filter(|n| *n == "DIE").count(), 1);
    let die: Die = decode(of(&sent, ME, "DIE")[0]);
    assert!(die.can_teleport);
}

/// With `DisconnectAfterDeath` the dead player is disconnected an hour later; reviving cancels it.
#[test]
fn a_dead_player_is_disconnected_after_an_hour_when_configured() {
    let death = DeathSettings {
        disconnect_after_death: true,
        ..DeathSettings::default()
    };
    let make = |tag: &str| {
        let mut s = Scene::new(tag, 5, ONE_MONSTER);
        // `Scene::with_settings` switches the option off; turn it on again.
        s.world.set_settings(WorldSettings {
            death: death.clone(),
            ..WorldSettings::default()
        });
        s.enter(ME, "Hero", (X, Y, Z), vec![]);
        s
    };
    let mut s = make("player-hour");
    let monster = s.npc_of(9001).unwrap();
    s.kill(ObjectId(ME), Some(monster));
    assert!(s.world.has_decay_task(ObjectId(ME)));
    s.ticks(ticks_of(3_599_000));
    assert!(s.world.player(ObjectId(ME)).is_some());
    s.log.take();
    s.ticks(40);
    assert!(
        s.world.player(ObjectId(ME)).is_none(),
        "disconnected after 3600 s"
    );

    let mut s = make("player-hour-revived");
    let monster = s.npc_of(9001).unwrap();
    s.kill(ObjectId(ME), Some(monster));
    s.world.do_revive(ObjectId(ME));
    assert!(!s.world.has_decay_task(ObjectId(ME)));
    s.ticks(ticks_of(3_700_000));
    assert!(s.world.player(ObjectId(ME)).is_some());
}

/// `Player.doDie`: a player with karma loses a quarter of it (all of it below 200).
#[test]
fn death_lowers_karma() {
    for (before, after) in [(500, 375), (199, 0), (200, 150), (0, 0)] {
        let mut s = Scene::new("player-karma", 5, ONE_MONSTER);
        let mut data = player_data(ME, "Hero", (X, Y, Z), vec![]);
        data.karma = before;
        s.enter_data(data);
        let monster = s.npc_of(9001).unwrap();
        s.log.take();
        s.kill(ObjectId(ME), Some(monster));
        assert_eq!(
            s.world.player(ObjectId(ME)).unwrap().data.karma,
            after,
            "karma {before}"
        );
        if before > 0 {
            let sent = s.log.take();
            let updates: Vec<StatusUpdate> = of(&sent, ME, "STATUS_UPDATE")
                .iter()
                .map(|raw| decode::<StatusUpdate>(raw))
                .collect();
            assert!(
                updates
                    .iter()
                    .any(|u| u.attributes == vec![(status::KARMA, after)]),
                "{updates:?}"
            );
        }
    }
}

// ------------------------------------------------------------------ revive and restart points

/// `RequestRestartPoint` (to the village): `Revive`, the stop of the move, the teleport to the
/// village's restart point; the vitals come back to `RespawnRestoreHP` (65%) when the client has
/// loaded the target and answers `Appearing`.
#[test]
fn restart_to_the_village_revives_and_teleports() {
    let mut s = Scene::new("restart-town", 5, ONE_MONSTER);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    s.enter(OTHER, "Friend", (X + 20, Y, Z), vec![]);
    let monster = s.npc_of(9001).unwrap();
    s.kill(ObjectId(ME), Some(monster));
    s.ticks(5);
    s.log.take();

    s.input(ME, ClientInput::RestartPoint { point_type: 0 });
    let sent = s.log.take();
    let me = names(&sent, ME);
    // Player.doRevive: Revive, EtcStatusUpdate; then teleToLocation: StopMove, TeleportToLocation.
    assert!(
        has_in_order(
            &me,
            &["REVIVE", "ETC_STATUS_UPDATE", "STOP_MOVE", "TELEPORT_TO_LOCATION"]
        ),
        "{me:?}"
    );
    let friend = names(&sent, OTHER);
    assert!(
        has_in_order(
            &friend,
            &["REVIVE", "STOP_MOVE", "DELETE_OBJECT", "TELEPORT_TO_LOCATION"]
        ),
        "{friend:?}"
    );
    let revive: Revive = decode(of(&sent, ME, "REVIVE")[0]);
    assert_eq!(revive.object_id, ME);

    let player = s.world.player(ObjectId(ME)).unwrap();
    assert!(!s.world.is_dead(ObjectId(ME)));
    assert!(!s.world.is_pending_revive(ObjectId(ME)));
    // 65% of 100 HP; MP and CP stay (`RespawnRestoreMP/CP` are 0).
    assert_eq!(player.core.cur_hp, 65.0);
    assert_eq!(player.core.cur_cp, 20.0);
    assert!(s.world.is_teleporting(ObjectId(ME)));
    // The restart point of the default region, with the random offset of a teleport.
    let tele: TeleportToLocation = decode(of(&sent, ME, "TELEPORT_TO_LOCATION")[0]);
    assert!(
        (tele.x + 84_318).abs() <= 50 && (tele.y - 244_579).abs() <= 50,
        "{tele:?}"
    );
    assert_eq!(tele.z, -3_730 + 5);
    assert!(
        s.world.is_hp_mp_regenerating(ObjectId(ME)),
        "alive and hurt: regenerating"
    );

    // The client arrives.
    s.input(ME, ClientInput::Appearing);
    assert!(!s.world.is_teleporting(ObjectId(ME)));
    let (x, y, _) = s.world.position_of(ObjectId(ME)).unwrap();
    assert!((x + 84_318).abs() <= 50 && (y - 244_579).abs() <= 50);
}

/// The buttons that need clans, castles, fortresses or sieges are refused (Java warns and
/// returns), and a living player's request does nothing.
#[test]
fn restart_points_that_cannot_apply_do_nothing() {
    let mut s = Scene::new("restart-refused", 5, ONE_MONSTER);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    // Alive: nothing.
    s.input(ME, ClientInput::RestartPoint { point_type: 0 });
    assert!(
        s.log
            .take()
            .iter()
            .all(|(_, b)| name_of(b) != "TELEPORT_TO_LOCATION")
    );
    assert!(!s.world.is_teleporting(ObjectId(ME)));

    let monster = s.npc_of(9001).unwrap();
    s.kill(ObjectId(ME), Some(monster));
    s.ticks(5);
    s.log.take();
    // Clan hall, castle, fortress, siege headquarters, fixed (no item), jail (not jailed), the
    // agathion button.
    for point_type in [1, 2, 3, 4, 5, 6, 27] {
        s.input(ME, ClientInput::RestartPoint { point_type });
        assert!(s.world.is_dead(ObjectId(ME)), "type {point_type}");
        assert!(!s.world.is_teleporting(ObjectId(ME)), "type {point_type}");
        assert!(s.log.take().is_empty(), "type {point_type}");
    }
}

/// The fixed resurrection (`type 5`) uses a self-resurrection item and stands the player up where
/// he lies; the `Die` packet offers the button only when he has one.
#[test]
fn a_self_resurrection_item_revives_in_place() {
    let mut s = Scene::new("restart-fixed", 5, ONE_MONSTER);
    s.enter(
        ME,
        "Hero",
        (X, Y, Z),
        vec![item(0x3000_0002, FEATHER, "INVENTORY", 0, 1)],
    );
    let monster = s.npc_of(9001).unwrap();
    s.log.take();
    s.kill(ObjectId(ME), Some(monster));
    let sent = s.log.take();
    let die: Die = decode(of(&sent, ME, "DIE")[0]);
    assert!(die.fixed_res, "the player owns a self-resurrection item");

    s.input(ME, ClientInput::RestartPoint { point_type: 5 });
    let sent = s.log.take();
    let me = names(&sent, ME);
    assert!(me.contains(&"REVIVE".to_owned()), "{me:?}");
    assert!(!me.contains(&"TELEPORT_TO_LOCATION".to_owned()), "{me:?}");
    assert!(!s.world.is_dead(ObjectId(ME)));
    assert_eq!(s.world.position_of(ObjectId(ME)), Some((X, Y, Z)));
    // The item is used up.
    assert!(
        s.world
            .player(ObjectId(ME))
            .unwrap()
            .data
            .items
            .iter()
            .all(|i| i.item_id != FEATHER)
    );
}

/// A player without the item may not use the fixed button, a GM may without one; an access level
/// with `allowFixedRes` only sees the button.
#[test]
fn the_fixed_button_needs_an_item_or_a_gm() {
    let mut s = Scene::new("restart-fixed-gm", 5, ONE_MONSTER);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let mut gm = player_data(OTHER, "Admin", (X + 30, Y, Z), vec![]);
    gm.access_level = 100;
    s.enter_data(gm);
    let mut test_gm = player_data(0x2000_0003, "TestGm", (X + 40, Y, Z), vec![]);
    test_gm.access_level = 20;
    s.enter_data(test_gm);
    let monster = s.npc_of(9001).unwrap();
    s.log.take();
    for id in [ME, OTHER, 0x2000_0003] {
        s.kill(ObjectId(id), Some(monster));
    }
    let sent = s.log.take();
    // Everybody around also gets the others' `Die`: pick the one about oneself.
    let fixed = |id: i32| {
        of(&sent, id, "DIE")
            .iter()
            .map(|raw| decode::<Die>(raw))
            .find(|d| d.object_id == id)
            .unwrap()
            .fixed_res
    };
    assert!(!fixed(ME));
    assert!(!fixed(OTHER), "isGM is not allowFixedRes");
    assert!(fixed(0x2000_0003), "allowFixedRes of the access level");

    s.input(ME, ClientInput::RestartPoint { point_type: 5 });
    assert!(s.world.is_dead(ObjectId(ME)));
    s.input(0x2000_0003, ClientInput::RestartPoint { point_type: 5 });
    assert!(s.world.is_dead(ObjectId(0x2000_0003)), "not a GM and no item");
    s.input(OTHER, ClientInput::RestartPoint { point_type: 5 });
    assert!(
        !s.world.is_dead(ObjectId(OTHER)),
        "a GM stands up without an item"
    );
}

/// `doRevive` restores the share of the maximum that `RespawnRestoreCP/HP/MP` ask for, only if
/// the creature has less.
#[test]
fn revive_restores_the_configured_share() {
    let death = DeathSettings {
        respawn_restore_cp: 0.5,
        respawn_restore_hp: 1.0,
        respawn_restore_mp: 0.1,
        ..DeathSettings::default()
    };
    let mut s = Scene::with_settings("revive-config", 5, ONE_MONSTER, death);
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let monster = s.npc_of(9001).unwrap();
    s.kill(ObjectId(ME), Some(monster));
    s.log.take();
    s.world.do_revive(ObjectId(ME));
    let core = &s.world.player(ObjectId(ME)).unwrap().core;
    // CP 20 -> 10 of 20 only if it had less (it stays at 20), HP 0 -> 100, MP 50 stays.
    assert_eq!((core.cur_cp, core.cur_hp, core.cur_mp), (20.0, 100.0, 50.0));
    let sent = s.log.take();
    assert!(names(&sent, ME).contains(&"REVIVE".to_owned()));
    // Reviving a creature that is not dead does nothing visible except the player's own extras.
    s.world.do_revive(ObjectId(ME));
    let me = names(&s.log.take(), ME);
    assert!(!me.contains(&"REVIVE".to_owned()));
}

// ------------------------------------------------------------------ the whole way

/// A player kills a monster with his sword (the `HitTask` path), the monster respawns at its spawn
/// after its corpse time and respawn delay.
#[test]
fn a_monster_killed_in_melee_comes_back() {
    let mut s = Scene::new("e2e", 20_261_019, ONE_MONSTER);
    s.enter(ME, "Hero", (X, Y, Z), sword_items());
    let monster = s.npc_of(9001).unwrap();
    s.click(ME, monster);
    s.click(ME, monster);
    s.run_until(4000, |w| w.is_dead(monster));
    assert!(s.world.has_decay_task(monster));
    // The player stops attacking.
    s.run_until(200, |w| w.intention_of(ObjectId(ME)) == Intention::Idle);
    s.run_until(1000, |w| w.npc(monster).is_none());
    s.run_until(2000, |w| w.npc(monster).is_some());
    let back = s.world.npc(monster).unwrap();
    assert_eq!((back.core.x, back.core.y, back.core.z), (X, Y, Z));
    assert!(!s.world.is_dead(monster));
    // The aggro list of the old life is gone.
    assert!(s.world.aggro_list(monster).is_empty());
}
