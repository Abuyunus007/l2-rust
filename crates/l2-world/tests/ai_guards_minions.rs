#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Guards, siege defenders and minions (M5-FU-02), through the world with a recording client:
//! - a `Guard` attacks players with karma and, with `GuardAttackAggroMob`, aggressive monsters
//!   (but not passive ones); a guard that is hit calls the guards around it; the siege classes
//!   and `QuestGuard` take the hate they take; a click on a guard attacks or talks,
//! - a minion that dies comes back after `CustomMinionsRespawnTime` / `RaidMinionRespawnTime`
//!   with the aggro list of its master; a raid boss takes its minions with it,
//! - an NPC with a fear time runs away and calms down (`AttackableAI.notifyActionAfraid`).
//!
//! Java: ai/AttackableAI.java, entity/actor/instance/{Guard,QuestGuard,Defender,FortCommander,
//! Monster}.java, entity/actor/holders/npc/MinionList.java, entity/actor/Attackable.java.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use l2_data::{
    ArmorSets, EnchantHpBonus, Hennas, ItemTemplates, NpcLoadOptions, NpcTemplates, PlayerTemplates,
    SkillTemplates, SpawnData, SpawnLoadOptions,
};
use l2_world::*;

// ------------------------------------------------------------------ harness

type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self) {
        self.0.lock().unwrap().clear();
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

fn player_data(id: i32, name: &str, (x, y, z): (i32, i32, i32), karma: i32) -> PlayerData {
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
        karma,
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

const GUARD: i32 = 9101;
const WOLF: i32 = 9102;
const HARE: i32 = 9103;
const QUEST_GUARD: i32 = 9104;
const DEFENDER: i32 = 9105;
const COMMANDER: i32 = 9106;
const CHIEF: i32 = 9110;
const MINION: i32 = 9111;
const RAID: i32 = 9112;

/// One NPC template. `aggressive` false writes `isAggressive="false"` (a monster that never
/// aggresses); `privates` is the `Privates` minion list of a master.
fn npc(id: i32, npc_type: &str, name: &str, aggressive: bool, privates: &str) -> String {
    let parameters = if privates.is_empty() {
        r#"<parameters><param name="SetAggressiveTime" value="0" /></parameters>"#.to_owned()
    } else {
        format!(r#"<parameters><minions name="Privates">{privates}</minions></parameters>"#)
    };
    let aggressive = if aggressive {
        ""
    } else {
        r#" isAggressive="false""#
    };
    format!(
        r#"<npc id="{id}" type="{npc_type}" level="1" name="{name}">
    {parameters}
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="300"{aggressive} />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>"#
    )
}

fn npc_xml() -> String {
    let minions = format!(r#"<npc id="{MINION}" count="2" respawnTime="10"/>"#);
    let list = [
        npc(GUARD, "Guard", "Town Guard", false, ""),
        npc(WOLF, "Monster", "Wolf", true, ""),
        npc(HARE, "Monster", "Hare", false, ""),
        npc(QUEST_GUARD, "QuestGuard", "Quest Guard", false, ""),
        npc(DEFENDER, "Defender", "Siege Guard", false, ""),
        npc(COMMANDER, "FortCommander", "Fort Commander", false, ""),
        npc(CHIEF, "Monster", "Boar Chief", false, &minions),
        npc(MINION, "Monster", "Boar", false, ""),
        npc(RAID, "RaidBoss", "Boar King", false, &minions),
    ]
    .join("\n");
    format!(r#"<?xml version="1.0"?><list>{list}</list>"#)
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
const PK: i32 = 0x2000_0002;

struct Scene {
    world: World,
    log: Log,
    _dir: tempfile::TempDir,
}

impl Scene {
    fn new(spawns: &str) -> Self {
        Self::with(spawns, AiSettings::default())
    }

    /// A world with the synthetic templates, the given spawns (the body of a spawn file) and AI
    /// settings.
    fn with(spawns: &str, settings: AiSettings) -> Self {
        let dir = tempfile::tempdir().unwrap();
        let spawn_dir = dir.path().join("data/spawns");
        std::fs::create_dir_all(&spawn_dir).unwrap();
        std::fs::write(
            spawn_dir.join("test.xml"),
            format!(r#"<?xml version="1.0"?><list enabled="true">{spawns}</list>"#),
        )
        .unwrap();
        let npcs = Arc::new(NpcTemplates::from_xml(&npc_xml(), &NpcLoadOptions::default()).unwrap());
        let spawn_data = Arc::new(SpawnData::load(dir.path(), &npcs, &SpawnLoadOptions::default()));
        let mut world = World::new(WorldRng::from_seed(20_261_020), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data(synthetic_stats())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_ai_settings(settings);
        world.set_npc_data(npcs, spawn_data);
        // Spawned before anybody is online: the regions are off, nobody thinks.
        world.spawn_all_npcs(false);
        Self {
            world,
            log: Log::default(),
            _dir: dir,
        }
    }

    fn enter(&mut self, id: i32, name: &str, pos: (i32, i32, i32), karma: i32) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player_data(id, name, pos, karma)),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn click(&mut self, id: i32, target: ObjectId) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input: ClientInput::Action {
                object_id: target.0,
                action_id: 0,
            },
        });
    }

    fn ticks(&mut self, n: u32) {
        for _ in 0..n {
            self.world.tick();
            self.log.take();
        }
    }

    /// Runs ticks until `done`; returns how many it took. Panics after `limit` ticks.
    fn run_until(&mut self, limit: u32, mut done: impl FnMut(&World) -> bool) -> u32 {
        for n in 1..=limit {
            self.world.tick();
            self.log.take();
            if done(&self.world) {
                return n;
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
    // A respawn far away, so a master that dies stays dead for the length of a scenario.
    format!(r#"<spawn><npc id="{npc_id}" x="{x}" y="{y}" z="{z}" heading="0" respawnTime="3600"/></spawn>"#)
}

fn dist2(a: (i32, i32, i32), b: (i32, i32, i32)) -> f64 {
    let (dx, dy) = (f64::from(a.0 - b.0), f64::from(a.1 - b.1));
    (dx * dx + dy * dy).sqrt()
}

fn me() -> ObjectId {
    ObjectId(ME)
}

fn settings_with_table(table: &[(i32, i32)]) -> AiSettings {
    AiSettings {
        minions_respawn_time: table.iter().copied().collect::<HashMap<_, _>>(),
        ..AiSettings::default()
    }
}

// ------------------------------------------------------------------ guards

/// `Guard.onSpawn` activates the AI at once, but with no region on the guard stays out of the
/// think set (the invariant of 42,000 NPCs); a player with karma near it is attacked, a player
/// without is not.
#[test]
fn a_guard_attacks_a_player_with_karma_and_ignores_the_others() {
    let mut s = Scene::new(&spawn_line(GUARD, X + 100, Y, Z));
    let guard = s.npc_of(GUARD);
    assert!(
        s.world.has_ai(guard),
        "Guard.onSpawn: getAI().setIntentionActive()"
    );
    assert_eq!(s.world.intention_of(guard), Intention::Active);
    assert!(
        !s.world.is_thinking_npc(guard),
        "nobody is near: it does not cost a think"
    );
    assert_eq!(s.world.thinking_npc_count(), 0);

    s.enter(ME, "Citizen", (X, Y, Z), 0);
    assert!(
        s.world.is_thinking_npc(guard),
        "the region turned on: the guard thinks"
    );
    s.enter(PK, "Outlaw", (X + 50, Y, Z), 100);
    s.run_until(400, |w| w.intention_of(guard) == Intention::Attack);
    assert_eq!(s.world.attack_target_of(guard), Some(ObjectId(PK)));
    assert!(s.world.aggro_contains(guard, ObjectId(PK)));
    assert!(!s.world.aggro_contains(guard, me()), "no karma: no aggression");
    assert!(s.world.is_running(guard));
    // And it hits: a guard fights like any other attackable.
    s.run_until(600, |w| w.player(ObjectId(PK)).unwrap().core.cur_hp < 5000.0);
    assert_eq!(s.world.player(me()).unwrap().core.cur_hp, 5000.0);
}

#[test]
fn a_guard_leaves_players_without_karma_alone() {
    let mut s = Scene::new(&spawn_line(GUARD, X + 100, Y, Z));
    let guard = s.npc_of(GUARD);
    s.enter(ME, "Citizen", (X, Y, Z), 0);
    s.ticks(400);
    assert_eq!(s.world.intention_of(guard), Intention::Active);
    assert!(s.world.aggro_list(guard).is_empty());
}

fn guard_scene(aggro_mob: bool) -> Scene {
    let settings = AiSettings {
        guard_attack_aggro_mob: aggro_mob,
        ..AiSettings::default()
    };
    // The monsters are 450 and 500 from the player (aggro range 300): only the guard's look at
    // monsters (it has no aggro range check for them) can start a fight.
    let spawns = format!(
        "{}{}{}",
        spawn_line(GUARD, X + 100, Y, Z),
        spawn_line(WOLF, X + 450, Y, Z),
        spawn_line(HARE, X + 500, Y + 50, Z)
    );
    let mut s = Scene::with(&spawns, settings);
    s.enter(ME, "Citizen", (X, Y, Z), 0);
    s
}

/// `GuardAttackAggroMob`: the guard fights the aggressive wolf and not the passive hare; the wolf
/// (`Monster.isAutoAttackable`) fights back.
#[test]
fn a_guard_attacks_aggressive_monsters_with_guard_attack_aggro_mob() {
    let mut s = guard_scene(true);
    let (guard, wolf, hare) = (s.npc_of(GUARD), s.npc_of(WOLF), s.npc_of(HARE));
    s.run_until(600, |w| w.aggro_contains(guard, wolf));
    assert!(
        !s.world.aggro_contains(guard, hare),
        "a passive monster is no target"
    );
    assert_eq!(s.world.intention_of(guard), Intention::Attack);
    assert_eq!(s.world.attack_target_of(guard), Some(wolf));
    // The guard's hit puts the guard on the wolf's aggro list and the wolf on the guard.
    s.run_until(2000, |w| w.aggro_contains(wolf, guard));
    assert_eq!(s.world.intention_of(wolf), Intention::Attack);
    assert_eq!(s.world.attack_target_of(wolf), Some(guard));
    assert!(
        !s.world.aggro_contains(wolf, me()),
        "the wolf is far from the player"
    );
    // Both land hits: a monster hits a guard, a guard hits a monster.
    let hp = |w: &World, id: ObjectId| w.npc(id).unwrap().core.cur_hp;
    s.run_until(2000, |w| hp(w, guard) < 3000.0 && hp(w, wolf) < 3000.0);
}

#[test]
fn a_guard_ignores_monsters_without_the_setting() {
    let mut s = guard_scene(false);
    let (guard, wolf) = (s.npc_of(GUARD), s.npc_of(WOLF));
    s.ticks(600);
    assert!(s.world.aggro_list(guard).is_empty());
    assert!(s.world.aggro_list(wolf).is_empty());
    assert_eq!(s.world.intention_of(guard), Intention::Active);
}

/// `Guard.addDamage`: the guard and every guard within 500 follow the attacker and hate it by 10;
/// a guard further away does not hear.
#[test]
fn a_hit_guard_calls_the_guards_around_it() {
    let spawns = format!(
        "{}{}{}{}",
        spawn_line(GUARD, X, Y, Z),
        spawn_line(GUARD, X + 300, Y, Z),
        spawn_line(GUARD, X + 900, Y, Z),
        spawn_line(HARE, X + 100, Y, Z)
    );
    let mut s = Scene::new(&spawns);
    s.enter(ME, "Citizen", (X, Y + 100, Z), 0);
    let ids: Vec<ObjectId> = s
        .world
        .npc_ids()
        .into_iter()
        .filter(|id| s.world.npc(*id).unwrap().npc_id == GUARD)
        .collect();
    let by_x = |id: &ObjectId| s.pos(*id).0;
    let (near, mid, far) = {
        let mut sorted = ids;
        sorted.sort_by_key(by_x);
        (sorted[0], sorted[1], sorted[2])
    };
    let hare = s.npc_of(HARE);

    s.world.reduce_current_hp(near, 1.0, Some(hare));

    assert!(s.world.aggro_hating(near, hare) >= 10);
    assert_eq!(
        s.world.follow_target_of(near),
        Some(hare),
        "startFollow(attacker)"
    );
    assert_eq!(s.world.aggro_hating(mid, hare), 10);
    assert_eq!(s.world.follow_target_of(mid), Some(hare));
    assert_eq!(
        s.world.aggro_hating(far, hare),
        0,
        "900 away: out of the 500 range"
    );
    assert_eq!(s.world.follow_target_of(far), None);
}

/// `QuestGuard.addDamageHate` takes no hate from players; `Defender` and `FortCommander` take
/// none from defenders (and, with no siege, from anybody else they do).
#[test]
fn the_guard_classes_refuse_the_hate_java_refuses() {
    let spawns = format!(
        "{}{}{}{}",
        spawn_line(QUEST_GUARD, X, Y, Z),
        spawn_line(DEFENDER, X + 200, Y, Z),
        spawn_line(COMMANDER, X + 400, Y, Z),
        spawn_line(HARE, X + 600, Y, Z)
    );
    let mut s = Scene::new(&spawns);
    s.enter(ME, "Citizen", (X, Y + 100, Z), 0);
    let (quest, defender, commander, hare) = (
        s.npc_of(QUEST_GUARD),
        s.npc_of(DEFENDER),
        s.npc_of(COMMANDER),
        s.npc_of(HARE),
    );

    s.world.add_damage_hate(quest, me(), 0, 10);
    assert!(
        s.world.aggro_list(quest).is_empty(),
        "QuestGuard: !attacker.isPlayer()"
    );
    s.world.add_damage_hate(quest, hare, 0, 10);
    assert!(s.world.aggro_contains(quest, hare));

    s.world.add_damage_hate(defender, commander, 0, 10);
    assert!(
        s.world.aggro_list(defender).is_empty(),
        "Defender: !(attacker instanceof Defender)"
    );
    s.world.add_damage_hate(commander, defender, 0, 10);
    assert!(s.world.aggro_list(commander).is_empty());
    s.world.add_damage_hate(commander, commander, 0, 10);
    assert!(s.world.aggro_list(commander).is_empty());
    s.world.add_damage_hate(defender, me(), 0, 10);
    assert!(s.world.aggro_contains(defender, me()));
    s.world.add_damage_hate(commander, hare, 0, 10);
    assert!(s.world.aggro_contains(commander, hare));
}

/// `Defender.returnHome` walks back when more than 40 from the spawn, `FortCommander.returnHome`
/// when more than 200; both forget their aggro on the way.
#[test]
fn defenders_return_home_from_forty_and_commanders_from_two_hundred() {
    let spawns = format!(
        "{}{}{}{}",
        spawn_line(DEFENDER, X, Y, Z),
        spawn_line(DEFENDER, X, Y + 1000, Z),
        spawn_line(COMMANDER, X + 2000, Y, Z),
        spawn_line(COMMANDER, X + 2000, Y + 1000, Z)
    );
    let mut s = Scene::new(&spawns);
    s.enter(ME, "Citizen", (X, Y + 500, Z), 0);
    let by_class = |s: &Scene, class: i32| -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = s
            .world
            .npc_ids()
            .into_iter()
            .filter(|id| s.world.npc(*id).unwrap().npc_id == class)
            .collect();
        ids.sort_by_key(|id| s.pos(*id).1);
        ids
    };
    let defenders = by_class(&s, DEFENDER);
    let commanders = by_class(&s, COMMANDER);
    let step = |s: &mut Scene, id: ObjectId, dx: i32| {
        let (x, y, z) = s.pos(id);
        s.world.tele_to_location(id, TeleportTarget::at(x + dx, y, z));
        s.world.set_intention_active(id);
        s.world.add_damage_hate(id, ObjectId(ME), 0, 5);
        s.world.return_home(id);
    };

    step(&mut s, defenders[0], 30);
    assert!(
        s.world.aggro_contains(defenders[0], me()),
        "30 away: home already"
    );
    assert_eq!(s.world.intention_of(defenders[0]), Intention::Active);
    step(&mut s, defenders[1], 60);
    assert!(s.world.aggro_list(defenders[1]).is_empty());
    assert_eq!(s.world.intention_of(defenders[1]), Intention::MoveTo);

    step(&mut s, commanders[0], 150);
    assert!(
        s.world.aggro_contains(commanders[0], me()),
        "150 away: home for a commander"
    );
    assert_eq!(s.world.intention_of(commanders[0]), Intention::Active);
    step(&mut s, commanders[1], 250);
    assert!(s.world.aggro_list(commanders[1]).is_empty());
    assert_eq!(s.world.intention_of(commanders[1]), Intention::MoveTo);
}

/// `Guard.onAction`: the second click attacks when the player is on the guard's aggro list,
/// otherwise it talks (or walks up to the guard).
#[test]
fn guard_click_branches() {
    let mut s = Scene::new(&spawn_line(GUARD, X + 40, Y, Z));
    let guard = s.npc_of(GUARD);
    // No karma: the guard never hates the player by itself.
    s.enter(ME, "Citizen", (X, Y, Z), 0);
    s.click(ME, guard);
    s.click(ME, guard);
    assert_ne!(s.world.intention_of(me()), Intention::Attack, "talk, not attack");

    s.world.add_damage_hate(guard, me(), 0, 5);
    s.click(ME, guard);
    assert_eq!(
        s.world.intention_of(me()),
        Intention::Attack,
        "the player is on the aggro list"
    );
}

// ------------------------------------------------------------------ fear

/// `AttackableAI.notifyActionAfraid` on a guard (an attackable since this card): with a fear time
/// it runs away from the effector, five seconds of fear are taken off every five seconds, and it
/// calms down.
#[test]
fn a_guard_with_a_fear_time_runs_away_and_calms_down() {
    let mut s = Scene::new(&spawn_line(GUARD, X + 100, Y, Z));
    let guard = s.npc_of(GUARD);
    s.enter(ME, "Citizen", (X, Y, Z), 0);
    let before = s.pos(guard);
    s.world.set_fear_time(guard, 10);
    s.world.notify_action_afraid(guard, me(), true);
    assert_eq!(s.world.fear_time(guard), 10);
    s.ticks(60);
    let after = s.pos(guard);
    // The guard stands east of the player: it runs east.
    assert!(after.0 > before.0 + 50, "{before:?} -> {after:?}");
    s.ticks(260);
    assert!(s.world.fear_time(guard) <= 0);
}

// ------------------------------------------------------------------ minions

fn chief_scene(settings: AiSettings) -> (Scene, ObjectId, Vec<ObjectId>) {
    let mut s = Scene::with(&spawn_line(CHIEF, X, Y, Z), settings);
    s.enter(ME, "Hero", (X + 100, Y + 400, Z), 0);
    let chief = s.npc_of(CHIEF);
    let minions = s.world.minions_of(chief).to_vec();
    assert_eq!(minions.len(), 2);
    (s, chief, minions)
}

/// `CustomMinionsRespawnTime`: the minion comes back with its object id after 30 seconds,
/// next to its master, and takes the master's aggro list (`MinionRespawnTask`).
#[test]
fn a_minion_of_the_table_comes_back_and_inherits_the_master_aggro() {
    let (mut s, chief, minions) = chief_scene(settings_with_table(&[(MINION, 30)]));
    let (dead, survivor) = (minions[0], minions[1]);
    assert!(s.world.do_die(dead, None));
    assert_eq!(s.world.minions_of(chief), &[survivor]);
    assert_eq!(s.world.pending_minion_respawns(), 1);
    assert_eq!(
        s.world.npc(dead).unwrap().leader,
        Some(chief),
        "it keeps its master: it comes back"
    );

    // The corpse decays; the minion waits.
    let decayed = s.run_until(300, |w| w.npc(dead).is_none());
    s.ticks(590 - decayed);
    assert!(
        s.world.npc(dead).is_none(),
        "29.5 seconds after its death: still gone"
    );
    assert_eq!(s.world.pending_minion_respawns(), 1);

    // The master has a target when the minion arrives, 30 seconds (600 ticks) after its death.
    s.world.add_damage_hate(chief, me(), 0, 50);
    let waited = s.run_until(40, |w| w.npc(dead).is_some());
    assert!((9..=11).contains(&waited), "{waited}");
    assert_eq!(s.world.pending_minion_respawns(), 0);
    assert_eq!(s.world.npc(dead).unwrap().leader, Some(chief));
    assert_eq!(s.world.minions_of(chief).len(), 2);
    assert!(!s.world.is_dead(dead));
    assert!(
        dist2(s.pos(dead), s.pos(chief)) <= 450.0,
        "{:?} {:?}",
        s.pos(dead),
        s.pos(chief)
    );
    assert!(s.world.aggro_contains(dead, me()), "the master's aggro list");
    assert_eq!(s.world.intention_of(dead), Intention::Attack);
    assert_eq!(s.world.attack_target_of(dead), Some(me()));
}

/// A table entry of 0 means "never": the master forgets the minion, which forgets its master.
#[test]
fn a_minion_with_respawn_time_zero_is_gone_for_good() {
    let (mut s, chief, minions) = chief_scene(settings_with_table(&[(MINION, 0)]));
    let dead = minions[0];
    assert!(s.world.do_die(dead, None));
    assert_eq!(s.world.npc(dead).unwrap().leader, None, "setLeader(null)");
    assert_eq!(s.world.minions_of(chief), &[minions[1]]);
    assert_eq!(s.world.pending_minion_respawns(), 0);
    s.ticks(1400);
    assert!(s.world.npc(dead).is_none());
    assert_eq!(s.world.minions_of(chief).len(), 1);
}

/// Without a table entry only a raid boss' minions come back.
#[test]
fn the_minion_of_an_ordinary_master_does_not_come_back_by_default() {
    let (mut s, chief, minions) = chief_scene(AiSettings::default());
    assert!(s.world.do_die(minions[0], None));
    assert_eq!(s.world.pending_minion_respawns(), 0);
    s.ticks(1400);
    assert!(s.world.npc(minions[0]).is_none());
    assert_eq!(s.world.minions_of(chief).len(), 1);
}

/// `MinionRespawnTask` does nothing for a master that is dead by then.
#[test]
fn a_dead_master_does_not_get_its_minion_back() {
    let (mut s, chief, minions) = chief_scene(settings_with_table(&[(MINION, 30)]));
    assert!(s.world.do_die(minions[0], None));
    assert!(s.world.do_die(chief, None));
    // An ordinary master dies alone: the other minion lives on, the respawn is still waiting.
    assert!(s.world.npc(minions[1]).is_some());
    assert_eq!(s.world.pending_minion_respawns(), 1);
    s.ticks(700);
    assert_eq!(s.world.pending_minion_respawns(), 0, "the task ran");
    assert!(s.world.npc(minions[0]).is_none(), "and did nothing");
}

/// `ForceDeleteMinions`: an ordinary master takes its minions with it too.
#[test]
fn force_delete_minions_takes_the_minions_of_an_ordinary_master() {
    let (mut s, chief, minions) = chief_scene(AiSettings {
        force_delete_minions: true,
        ..AiSettings::default()
    });
    assert!(s.world.do_die(chief, None));
    for minion in &minions {
        assert!(s.world.npc(*minion).is_none(), "deleted with its master");
    }
    assert!(s.world.minions_of(chief).is_empty());
}

fn raid_scene(settings: AiSettings) -> (Scene, ObjectId, Vec<ObjectId>) {
    // Raid bosses are not in the spawn files (`SpawnData.checkTemplate`): the boss is placed
    // the way the raid boss spawn manager would.
    let mut s = Scene::with("", settings);
    let raid = s.world.spawn_npc_at(RAID, X, Y, Z, 0).unwrap();
    assert_eq!(s.world.spawn_minions_of(raid), 2);
    s.enter(ME, "Hero", (X + 100, Y + 400, Z), 0);
    let minions = s.world.minions_of(raid).to_vec();
    assert_eq!(minions.len(), 2);
    (s, raid, minions)
}

/// A raid boss' minion comes back after `RaidMinionRespawnTime` (the shipped 300000 ms is the
/// default of the setting; here 20 seconds).
#[test]
fn a_raid_minion_comes_back_after_the_raid_minion_respawn_time() {
    assert_eq!(AiSettings::default().raid_minion_respawn_timer, 300_000);
    let (mut s, raid, minions) = raid_scene(AiSettings {
        raid_minion_respawn_timer: 20_000,
        ..AiSettings::default()
    });
    assert!(s.world.do_die(minions[0], None));
    assert_eq!(s.world.pending_minion_respawns(), 1);
    assert_eq!(s.world.npc(minions[0]).unwrap().leader, Some(raid));
    s.ticks(380);
    assert!(s.world.npc(minions[0]).is_none());
    s.run_until(40, |w| w.npc(minions[0]).is_some());
    assert_eq!(s.world.minions_of(raid).len(), 2);
}

/// A raid boss that dies takes its minions with it and cancels the respawn of the dead ones.
#[test]
fn a_raid_boss_takes_its_minions_and_their_respawns_with_it() {
    let (mut s, raid, minions) = raid_scene(AiSettings {
        raid_minion_respawn_timer: 20_000,
        ..AiSettings::default()
    });
    assert!(s.world.do_die(minions[0], None));
    assert_eq!(s.world.pending_minion_respawns(), 1);
    assert!(s.world.do_die(raid, None));
    assert!(s.world.npc(minions[1]).is_none(), "the living minion is deleted");
    assert_eq!(
        s.world.pending_minion_respawns(),
        0,
        "the respawn task is cancelled"
    );
    assert!(s.world.minions_of(raid).is_empty());
    s.ticks(600);
    for minion in &minions {
        assert!(s.world.npc(*minion).is_none(), "nobody comes back");
    }
}
