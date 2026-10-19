#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! NPC idle behaviour (M4-AI-02) through the world with a recording client:
//! - a monster's random walk stays inside `MaxDriftRange` of its spawn, and only when the
//!   template allows it (`randomWalk` and `MoveArounding`, Java `NpcTemplate.isRandomWalkEnabled`),
//! - random animation: a `SocialAction` at the configured interval (`MinNpcAnimation` and
//!   `MaxNpcAnimation`, `MinMonsterAnimation` and `MaxMonsterAnimation`), for the NPCs that have
//!   one, while their region is on, and no timer while it is off,
//! - `takeAggro="false"` of `AccessLevels.xml`: a GM level that takes no aggro is not attacked.
//!
//! The scenarios use small synthetic templates (they run in CI).
//! Java: ai/AttackableAI.java (thinkActive), taskmanagers/RandomAnimationTaskManager.java,
//! entity/actor/Npc.java (onRandomAnimation, hasRandomAnimation), entity/WorldRegion.java
//! (switchAI), data/holders/AccessLevel.java (canTakeAggro).

use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{MoveToLocation, SocialAction};
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

/// The static data of the scenes: the synthetic stat tables, and the access levels that take no
/// aggro (the `takeAggro="false"` rows of `AccessLevels.xml`).
struct Data {
    stats: StatData,
    no_aggro: Vec<i32>,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.stats.items.get(item_id)?;
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
    fn can_take_aggro(&self, level: i32) -> bool {
        !self.no_aggro.contains(&level)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
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

fn player_data(id: i32, name: &str, (x, y, z): (i32, i32, i32), level: i32) -> PlayerData {
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
        access_level: level,
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

/// One synthetic NPC. `body` holds extra children (`<status>`, `<parameters>`), `ai` the
/// attributes of `<ai>`.
fn npc(id: i32, kind: &str, body: &str, ai: &str) -> String {
    format!(
        r#"<npc id="{id}" type="{kind}" level="1" name="Test {id}">
    {body}
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai {ai} />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>"#
    )
}

/// 9001: an aggressive monster (`SetAggressiveTime` 0). 9002: a passive monster that walks.
/// 9010: a folk NPC. 9011: a chest, a monster type that never animates. 9012: a monster whose
/// template says `randomWalk="false"`. 9013: a passive monster with `MoveArounding` 0 (it never
/// walks, so its animation timing is exact). 9014: a folk NPC with `randomAnimation="false"`.
/// 9015: a passive monster with `randomAnimation="false"`.
fn npcs_xml() -> String {
    let move_arounding_off = r#"<parameters><param name="MoveArounding" value="0" /></parameters>"#;
    let set_aggressive = r#"<parameters><param name="SetAggressiveTime" value="0" /></parameters>"#;
    format!(
        r#"<?xml version="1.0"?><list>{}{}{}{}{}{}{}{}</list>"#,
        npc(9001, "Monster", set_aggressive, r#"aggroRange="300""#),
        npc(9002, "Monster", "", r#"aggroRange="300" isAggressive="false""#),
        npc(9010, "Folk", "", ""),
        npc(9011, "Chest", "", r#"isAggressive="false""#),
        npc(
            9012,
            "Monster",
            r#"<status randomWalk="false" />"#,
            r#"isAggressive="false""#
        ),
        npc(9013, "Monster", move_arounding_off, r#"isAggressive="false""#),
        npc(9014, "Folk", r#"<status randomAnimation="false" />"#, ""),
        npc(
            9015,
            "Monster",
            r#"<status randomAnimation="false" />"#,
            r#"isAggressive="false""#
        ),
    )
}

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
    /// A world with the synthetic templates, the NPC settings and the spawns (the body of a spawn
    /// file). `no_aggro` are the access levels that take no aggro.
    fn new(spawns: &str, settings: NpcSettings, no_aggro: &[i32]) -> Self {
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
        let mut world = World::new(WorldRng::from_seed(20_261_019), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data {
            stats: synthetic_stats(),
            no_aggro: no_aggro.to_vec(),
        }));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_settings(settings);
        world.set_npc_data(npcs, spawn_data);
        // Spawned before anybody is online: the regions are off, nobody thinks.
        world.spawn_all_npcs(false);
        Self {
            world,
            log: Log::default(),
            _dir: dir,
        }
    }

    fn enter(&mut self, id: i32, name: &str, pos: (i32, i32, i32)) {
        self.enter_at_level(id, name, pos, 0);
    }

    fn enter_at_level(&mut self, id: i32, name: &str, pos: (i32, i32, i32), level: i32) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player_data(id, name, pos, level)),
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

    /// Runs `ticks` ticks and returns the social actions the player `ME` got, as (object id of
    /// the NPC, action id, tick number counted from here).
    fn socials(&mut self, ticks: u64) -> Vec<(i32, i32, u64)> {
        let mut out = Vec::new();
        for t in 1..=ticks {
            self.world.tick();
            for (to, raw) in self.log.take() {
                if to == ME && name_of(&raw) == "SOCIAL_ACTION" {
                    let social = decode::<SocialAction>(&raw);
                    out.push((social.object_id, social.action_id, t));
                }
            }
        }
        out
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

/// The gaps between the social actions of one NPC, in ticks.
fn gaps_of(socials: &[(i32, i32, u64)], npc: i32) -> Vec<u64> {
    let ticks: Vec<u64> = socials
        .iter()
        .filter(|(id, _, _)| *id == npc)
        .map(|(_, _, t)| *t)
        .collect();
    ticks.windows(2).map(|w| w[1] - w[0]).collect()
}

// ------------------------------------------------------------------ scenarios

/// A passive monster walks around its spawn point and never leaves `MaxDriftRange` of it: every
/// destination is checked against the range, and the positions in between lie on the way there.
#[test]
fn a_monster_random_walk_stays_inside_the_drift_range() {
    let mut s = Scene::new(&spawn_line(9002, X, Y, Z), NpcSettings::default(), &[]);
    // 150 units from the spawn point: the region is on, the monster thinks.
    s.enter(ME, "Hero", (X + 150, Y, Z));
    let hare = s.npc_of(9002);
    let drift = f64::from(s.world.ai_settings().max_drift_range);
    let mut walks = 0;
    for _ in 0..12_000 {
        s.world.tick();
        for raw in of(&s.log.take(), ME, "MOVE_TO_LOCATION") {
            if decode::<MoveToLocation>(raw).object_id == hare.0 {
                walks += 1;
            }
        }
        let position = s.pos(hare);
        assert!(
            dist2(position, (X, Y, Z)) <= drift + 2.0,
            "the monster is {} from its spawn point, the drift range is {drift}",
            dist2(position, (X, Y, Z))
        );
    }
    // About one think in 30 walks (`RANDOM_WALK_RATE`), over 600 thinks.
    assert!(walks >= 5, "the monster walked {walks} times in 600 s");
}

/// The random walk needs the template's `randomWalk` flag and `MoveArounding` not 0 (Java
/// `NpcTemplate.isRandomWalkEnabled`). The passive monster 9002 is the control.
#[test]
fn the_random_walk_needs_the_template_flag_and_move_arounding() {
    let mut s = Scene::new(
        &format!(
            "{}{}{}",
            spawn_line(9002, X - 30, Y, Z),
            spawn_line(9012, X - 40, Y, Z),
            spawn_line(9013, X - 50, Y, Z)
        ),
        NpcSettings::default(),
        &[],
    );
    s.enter(ME, "Hero", (X + 150, Y, Z));
    let control = s.npc_of(9002).0;
    let (flag_off, arounding_off) = (s.npc_of(9012).0, s.npc_of(9013).0);
    let mut walks = std::collections::BTreeMap::<i32, usize>::new();
    for _ in 0..12_000 {
        s.world.tick();
        for raw in of(&s.log.take(), ME, "MOVE_TO_LOCATION") {
            *walks.entry(decode::<MoveToLocation>(raw).object_id).or_default() += 1;
        }
    }
    assert!(
        walks.get(&control).copied().unwrap_or(0) >= 5,
        "the control walks: {walks:?}"
    );
    assert_eq!(
        walks.get(&flag_off).copied().unwrap_or(0),
        0,
        "randomWalk=false walks"
    );
    assert_eq!(
        walks.get(&arounding_off).copied().unwrap_or(0),
        0,
        "MoveArounding 0 walks"
    );
}

/// A folk NPC plays a social action every `MinNpcAnimation` = `MaxNpcAnimation` seconds (10 here),
/// and a passive monster every `MinMonsterAnimation` = `MaxMonsterAnimation` seconds (7 here). The
/// first check is one interval after the region came on.
#[test]
fn the_animation_comes_at_the_configured_interval() {
    let settings = NpcSettings {
        min_npc_animation: 10,
        max_npc_animation: 10,
        min_monster_animation: 7,
        max_monster_animation: 7,
        ..NpcSettings::default()
    };
    let mut s = Scene::new(
        &format!("{}{}", spawn_line(9010, X, Y, Z), spawn_line(9013, X, Y - 60, Z)),
        settings,
        &[],
    );
    s.enter(ME, "Hero", (X + 150, Y, Z));
    let folk = s.npc_of(9010).0;
    let monster = s.npc_of(9013).0;
    let socials = s.socials(3000);
    // 10 s = 200 ticks, 7 s = 140 ticks. Both are longer than the 6 s social interval.
    let folk_ticks: Vec<u64> = socials
        .iter()
        .filter(|(id, _, _)| *id == folk)
        .map(|x| x.2)
        .collect();
    let monster_ticks: Vec<u64> = socials
        .iter()
        .filter(|(id, _, _)| *id == monster)
        .map(|x| x.2)
        .collect();
    assert_eq!(folk_ticks.first(), Some(&200));
    assert_eq!(monster_ticks.first(), Some(&140));
    assert!(
        gaps_of(&socials, folk).iter().all(|g| *g == 200),
        "folk gaps {:?}",
        gaps_of(&socials, folk)
    );
    assert!(
        gaps_of(&socials, monster).iter().all(|g| *g == 140),
        "monster gaps {:?}",
        gaps_of(&socials, monster)
    );
    // The action ids are the two Java draws `Rnd.get(2, 3)`.
    assert!(socials.iter().all(|(_, action, _)| *action == 2 || *action == 3));
}

/// Without a configured interval the wait is drawn between 5 and 60 seconds (`Rnd.get(min, max)`
/// with the defaults of `General.ini`). A social action is only sent when more than 6 seconds
/// have passed since the last one, so the gap between two sent actions is at most 66 seconds.
#[test]
fn the_default_interval_is_drawn_between_5_and_60_seconds() {
    let mut s = Scene::new(&spawn_line(9010, X, Y, Z), NpcSettings::default(), &[]);
    s.enter(ME, "Hero", (X + 150, Y, Z));
    let folk = s.npc_of(9010).0;
    let socials = s.socials(7200); // 360 s
    let first = socials.iter().find(|(id, _, _)| *id == folk).map(|x| x.2);
    // The first check: 5 s = 100 ticks at the least, 60 s = 1200 ticks at the most.
    assert!(
        first.is_some_and(|t| (100..=1200).contains(&t)),
        "first at {first:?}"
    );
    let gaps = gaps_of(&socials, folk);
    assert!(gaps.len() >= 4, "{} gaps in 360 s", gaps.len());
    for gap in gaps {
        assert!((121..=1320).contains(&gap), "gap of {gap} ticks");
    }
}

/// Only the NPCs whose template and spawn allow it play an animation: a folk NPC and a passive
/// monster do; a chest (a monster type that never does), and the two templates with
/// `randomAnimation="false"`, do not.
#[test]
fn only_the_npcs_that_have_an_animation_play_one() {
    let mut s = Scene::new(
        &format!(
            "{}{}{}{}{}",
            spawn_line(9010, X, Y, Z),
            spawn_line(9002, X, Y - 60, Z),
            spawn_line(9011, X, Y - 120, Z),
            spawn_line(9014, X, Y - 180, Z),
            spawn_line(9015, X, Y - 240, Z)
        ),
        NpcSettings::default(),
        &[],
    );
    s.enter(ME, "Hero", (X + 150, Y, Z));
    let socials = s.socials(4800); // 240 s
    let played = |s: &Scene, npc: i32| socials.iter().filter(|(id, _, _)| *id == s.npc_of(npc).0).count();
    assert!(played(&s, 9010) >= 1, "the folk NPC plays");
    assert!(played(&s, 9002) >= 1, "the passive monster plays");
    assert_eq!(played(&s, 9011), 0, "a chest never plays");
    assert_eq!(played(&s, 9014), 0, "randomAnimation=false (NPC) does not play");
    assert_eq!(
        played(&s, 9015),
        0,
        "randomAnimation=false (monster) does not play"
    );
}

/// A region that is off has no animation timer at all: nothing is polled for 42,000 NPCs, only
/// those near a player have one.
#[test]
fn no_animation_timer_while_the_region_is_off() {
    let mut s = Scene::new(
        &format!(
            "{}{}",
            spawn_line(9010, X + 40_000, Y, Z),
            spawn_line(9013, X + 40_000, Y - 60, Z)
        ),
        NpcSettings::default(),
        &[],
    );
    s.ticks(2400);
    assert_eq!(s.world.timers().len(), 0, "no timer for NPCs nobody is near");
}

/// When the player leaves and the regions switch off, the animation checks are cancelled (Java
/// `WorldRegion.switchAI(false)` removes the NPCs from the animation manager).
#[test]
fn leaving_cancels_the_animation_checks() {
    let mut s = Scene::new(
        &format!("{}{}", spawn_line(9010, X, Y, Z), spawn_line(9013, X, Y - 60, Z)),
        NpcSettings::default(),
        &[],
    );
    s.enter(ME, "Hero", (X + 150, Y, Z));
    s.ticks(600);
    assert!(
        s.world.timers().len() >= 2,
        "one check per animated NPC is scheduled"
    );
    s.leave(ME);
    // Every timer goes: the animation checks, and the regions' own switch-off timers after them.
    s.run_until_timers_empty(20_000);
}

/// A monster whose level takes no aggro (`takeAggro="false"`, here access level 30) is not
/// attacked by an aggressive monster. The control, a user level, is attacked at once.
#[test]
fn a_gm_level_that_takes_no_aggro_is_not_attacked() {
    for (level, attacked) in [(0, true), (30, false)] {
        let mut s = Scene::new(&spawn_line(9001, X + 250, Y, Z), NpcSettings::default(), &[30]);
        let wolf = s.npc_of(9001);
        s.enter_at_level(ME, "Hero", (X, Y, Z), level);
        let _ = s.log.take();
        let sent = s.ticks(200);
        assert_eq!(
            s.world.intention_of(wolf) == Intention::Attack,
            attacked,
            "level {level}: {:?}",
            names(&sent, ME)
        );
        assert_eq!(
            !s.world.aggro_list(wolf).is_empty(),
            attacked,
            "level {level} aggro list"
        );
        assert_eq!(
            !of(&sent, ME, "MOVE_TO_PAWN").is_empty() || !of(&sent, ME, "ATTACK").is_empty(),
            attacked,
            "level {level}: {:?}",
            names(&sent, ME)
        );
    }
}

impl Scene {
    /// Runs ticks until no timer is left, panics after `limit` ticks.
    fn run_until_timers_empty(&mut self, limit: u32) {
        for _ in 0..limit {
            if self.world.timers().is_empty() {
                return;
            }
            self.world.tick();
            self.log.take();
        }
        panic!("timers left after {limit} ticks: {}", self.world.timers().len());
    }
}
