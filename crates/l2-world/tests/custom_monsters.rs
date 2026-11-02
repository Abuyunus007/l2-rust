#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The custom monster features (M9-C01, D-57): champion monsters and the boss announcements,
//! each with its switch off (nothing changes) and on (the Java behaviour).
//! Java: config/custom/{ChampionMonstersConfig,BossAnnouncementsConfig}.java,
//! entity/spawns/Spawn.java, entity/actor/Creature.java, entity/actor/templates/NpcTemplate.java,
//! network/serverpackets/AbstractNpcInfo.java.

use std::collections::{BTreeSet, VecDeque};
use std::sync::{Arc, Mutex};

use l2_data::npc::DropType;
use l2_data::{ItemTemplates, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::NpcInfo;
use l2_world::custom::{BossAnnouncementSettings, ChampionSettings, CustomSettings};
use l2_world::formulas::Rnd;
use l2_world::rewards::drop::{
    ChampionDrops, DropKiller, DropSettings, DropVictim, ItemHolder, calculate_drops,
};
use l2_world::*;

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

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn player(id: i32, x: i32, y: i32, z: i32) -> PlayerData {
    PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{id}"),
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
        cur_cp: 10.0,
        max_cp: 10.0,
        online_time: 0,
        items: vec![],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;

const TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="1000" type="Folk" level="20" name="Gatekeeper"/>
  <npc id="2000" type="Monster" level="20" name="Mob"><stats><vitals hp="700" mp="10"/></stats></npc>
  <npc id="2001" type="Monster" level="70" name="High Mob"><stats><vitals hp="700" mp="10"/></stats></npc>
  <npc id="2002" type="Monster" level="20" name="Quest Mob" title="Quest Monster"/>
  <npc id="2003" type="Monster" level="20" name="Undying Mob"><status undying="true"/></npc>
  <npc id="2004" type="Monster" level="20" name="Regen Mob"><stats><vitals hp="1000" hpRegen="5" mp="10"/></stats></npc>
  <npc id="2005" type="Monster" level="20" name="Wolf">
    <parameters><param name="SetAggressiveTime" value="0" /></parameters>
    <stats><vitals hp="3000" mp="40"/></stats>
    <ai aggroRange="300" />
  </npc>
  <npc id="3000" type="RaidBoss" level="50" name="Raid Alpha"/>
  <npc id="3001" type="GrandBoss" level="80" name="Grand Beta"/>
  <npc id="3002" type="RaidBoss" level="50" name="Raid Gamma"/>
  <npc id="3003" type="RaidBoss" level="50" name="Raid Master"><parameters>
    <minions name="Privates"><npc id="3002" count="1" respawnTime="10"/></minions></parameters></npc>
</list>"#;

fn world_with(custom: CustomSettings, npc_settings: NpcSettings) -> (World, Log, tempfile::TempDir) {
    world_spawning("", custom, npc_settings)
}

/// A world whose spawn list is `spawns` (the inside of a `<list>` of a spawn file).
fn world_spawning(
    spawns: &str,
    custom: CustomSettings,
    npc_settings: NpcSettings,
) -> (World, Log, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join("data/spawns")).unwrap();
    std::fs::write(
        dir.path().join("data/spawns/a.xml"),
        format!(r#"<?xml version="1.0"?><list enabled="true">{spawns}</list>"#),
    )
    .unwrap();
    let npcs = Arc::new(NpcTemplates::from_xml(TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(dir.path(), &npcs, &SpawnLoadOptions::default()));
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(7), Box::new(NullOutbox));
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    world.set_npc_data(npcs, spawns);
    world.set_npc_settings(npc_settings);
    world.set_custom_settings(custom);
    (world, log, dir)
}

fn enter(world: &mut World, log: &Log, id: i32) {
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(id).unwrap()),
        player: Box::new(player(id, X, Y, Z)),
        sink: Box::new(Sink(id, log.clone())),
        ip: None,
    });
    log.take();
}

fn champions_on(frequency: i32) -> CustomSettings {
    CustomSettings {
        champion: ChampionSettings {
            enable: true,
            frequency,
            ..ChampionSettings::default()
        },
        ..CustomSettings::default()
    }
}

fn info_of(world: &World, npc: ObjectId) -> NpcInfo {
    let bytes = world.npc_info_bytes(npc, ObjectId(10)).unwrap();
    NpcInfo::decode(&mut PacketReader::new(&bytes[1..])).unwrap()
}

// ------------------------------------------------------------------ champions: the roll

/// `ChampionEnable = False` (the default): nobody is rolled, whatever the frequency, and the
/// `NpcInfo` is the plain one.
#[test]
fn champions_off_nothing_changes() {
    let custom = CustomSettings {
        champion: ChampionSettings {
            enable: false,
            frequency: 100,
            ..ChampionSettings::default()
        },
        ..CustomSettings::default()
    };
    let (mut world, log, _dir) = world_with(custom, NpcSettings::default());
    enter(&mut world, &log, 10);
    let mob = world.spawn_npc_at(2000, X, Y, Z, 5).unwrap();
    assert!(!world.is_champion(mob));
    let info = info_of(&world, mob);
    assert_eq!((info.title.as_str(), info.team), ("", 0));
}

/// A monster inside the level range is a champion at 100 %; the rest are not: `Folk`, a quest
/// monster, an undying one, a raid boss, and a level outside `ChampionMinLevel..ChampionMaxLevel`.
#[test]
fn champions_on_roll_only_eligible_monsters() {
    let (mut world, log, _dir) = world_with(champions_on(100), NpcSettings::default());
    enter(&mut world, &log, 10);
    let at = |world: &mut World, id: i32| world.spawn_npc_at(id, X, Y, Z, 5).unwrap();
    let mob = at(&mut world, 2000);
    assert!(world.is_champion(mob));
    for not_champion in [1000, 2001, 2002, 2003, 3000, 3001] {
        let npc = at(&mut world, not_champion);
        assert!(!world.is_champion(npc), "npc {not_champion}");
    }
    // The aura and the title reach the client: the red team and `ChampionTitle`.
    let info = info_of(&world, mob);
    assert_eq!((info.title.as_str(), info.team), ("Champion", 2));
}

/// `ChampionFrequency = 0` rolls nobody.
#[test]
fn champion_frequency_zero_rolls_nobody() {
    let (mut world, log, _dir) = world_with(champions_on(0), NpcSettings::default());
    enter(&mut world, &log, 10);
    let mob = world.spawn_npc_at(2000, X, Y, Z, 5).unwrap();
    assert!(!world.is_champion(mob));
}

/// `ChampionAura = False` leaves the team alone; `ShowNpcLevel` puts `ChampionTitle` in front of
/// the level title.
#[test]
fn champion_aura_and_level_title() {
    let mut custom = champions_on(100);
    custom.champion.show_aura = false;
    custom.champion.title = "Elite".to_owned();
    let npc_settings = NpcSettings {
        show_npc_level: true,
        ..NpcSettings::default()
    };
    let (mut world, log, _dir) = world_with(custom, npc_settings);
    enter(&mut world, &log, 10);
    let mob = world.spawn_npc_at(2000, X, Y, Z, 5).unwrap();
    let info = info_of(&world, mob);
    assert_eq!((info.title.as_str(), info.team), ("Elite Lv 20", 0));
}

/// `ChampionHp`: a champion takes the damage divided by it, a normal monster the whole damage.
#[test]
fn a_champion_takes_a_share_of_the_damage() {
    let (mut world, log, _dir) = world_with(champions_on(100), NpcSettings::default());
    enter(&mut world, &log, 10);
    let champion = world.spawn_npc_at(2000, X, Y, Z, 5).unwrap();
    let plain = world.spawn_npc_at(2001, X, Y, Z, 5).unwrap();
    let before = world.npc(champion).unwrap().core.cur_hp;
    world.reduce_current_hp(champion, 70.0, Some(ObjectId(10)));
    // ChampionHp = 7 (the default): 70 / 7 = 10.
    assert_eq!(before - world.npc(champion).unwrap().core.cur_hp, 10.0);
    let before = world.npc(plain).unwrap().core.cur_hp;
    world.reduce_current_hp(plain, 30.0, Some(ObjectId(10)));
    assert_eq!(before - world.npc(plain).unwrap().core.cur_hp, 30.0);
}

/// `ChampionHpRegen`: the regeneration of a champion is the template value times it (`Formulas.
/// calcHpRegen`), the one of a plain monster the template value.
#[test]
fn champion_hp_regen_multiplies_the_template_regeneration() {
    let mut custom = champions_on(100);
    custom.champion.hp_regen = 3.0;
    let (mut world, log, _dir) = world_with(custom, NpcSettings::default());
    enter(&mut world, &log, 10);
    let champion = world.spawn_npc_at(2004, X, Y, Z, 5).unwrap();
    let plain = world.spawn_npc_at(2004, X, Y, Z, 5).unwrap();
    assert!(world.is_champion(champion));
    world.set_champion(plain, false);
    for npc in [champion, plain] {
        world.set_current_hp(npc, 500.0, false);
    }
    // One regeneration period is 3 s (60 ticks): 5 for the plain monster, 5 * 3 for the champion.
    for _ in 0..60 {
        world.tick();
    }
    assert_eq!(world.npc(plain).unwrap().core.cur_hp, 505.0);
    assert_eq!(world.npc(champion).unwrap().core.cur_hp, 515.0);
}

/// A world with an aggressive monster 250 units from where the player will stand, inside its
/// aggro range of 300; `passive` is `ChampionPassive`, `champion` whether the monster stays a
/// champion. Returns whether the monster took the player as a target when he entered.
fn aggressive_monster_notices_player(passive: bool, champion: bool) -> bool {
    let mut custom = champions_on(100);
    custom.champion.passive = passive;
    let spawns = format!(
        r#"<spawn><npc id="2005" x="{}" y="{Y}" z="{Z}" heading="0"/></spawn>"#,
        X + 250
    );
    let (mut world, log, _dir) = world_spawning(&spawns, custom, NpcSettings::default());
    // Spawned before anybody is online: the regions are off, nobody thinks.
    world.spawn_all_npcs(false);
    let wolf = world.npc_ids()[0];
    assert!(world.is_champion(wolf));
    world.set_champion(wolf, champion);
    enter(&mut world, &log, 10);
    for _ in 0..40 {
        world.tick();
    }
    !world.aggro_list(wolf).is_empty()
}

/// `ChampionPassive`: a champion does not start a fight (`AttackableAI.autoAttackCondition`), a
/// plain aggressive monster does, and a champion does too when the switch is off.
#[test]
fn a_passive_champion_does_not_attack_a_player_in_its_aggro_range() {
    assert!(aggressive_monster_notices_player(false, false), "plain monster");
    assert!(
        aggressive_monster_notices_player(false, true),
        "champion, ChampionPassive off"
    );
    assert!(
        !aggressive_monster_notices_player(true, true),
        "champion, ChampionPassive on"
    );
    assert!(
        aggressive_monster_notices_player(true, false),
        "plain monster, ChampionPassive on"
    );
}

/// `ChampionHp = 0` turns the division off.
#[test]
fn champion_hp_zero_turns_the_division_off() {
    let mut custom = champions_on(100);
    custom.champion.hp = 0;
    let (mut world, log, _dir) = world_with(custom, NpcSettings::default());
    enter(&mut world, &log, 10);
    let champion = world.spawn_npc_at(2000, X, Y, Z, 5).unwrap();
    let before = world.npc(champion).unwrap().core.cur_hp;
    world.reduce_current_hp(champion, 70.0, Some(ObjectId(10)));
    assert_eq!(before - world.npc(champion).unwrap().core.cur_hp, 70.0);
}

/// `Quest.giveItemRandomly`: a champion raises the chance and the amounts, adena has its own.
#[test]
fn quest_drops_of_a_champion_use_the_champion_factors() {
    let mut custom = champions_on(100);
    custom.champion.rewards_chance = 2.0;
    custom.champion.rewards_amount = 3.0;
    custom.champion.adenas_rewards_chance = 4.0;
    custom.champion.adenas_rewards_amount = 5.0;
    let (mut world, log, _dir) = world_with(custom, NpcSettings::default());
    enter(&mut world, &log, 10);
    let champion = world.spawn_npc_at(2000, X, Y, Z, 5).unwrap();
    let plain = world.spawn_npc_at(2001, X, Y, Z, 5).unwrap();
    assert_eq!(
        world.champion_quest_drop_factors(champion, 1234),
        Some((2.0, 3.0))
    );
    assert_eq!(world.champion_quest_drop_factors(champion, 57), Some((4.0, 5.0)));
    assert_eq!(
        world.champion_quest_drop_factors(champion, 5575),
        Some((4.0, 5.0))
    );
    assert_eq!(world.champion_quest_drop_factors(plain, 1234), None);
}

// ------------------------------------------------------------------ champions: the drops

/// The random source of the drop tests: doubles for the chances, longs for the amounts, ints for
/// `Rnd.get(100)`.
#[derive(Default)]
struct Scripted {
    doubles: VecDeque<f64>,
    ints: VecDeque<i32>,
}

impl Rnd for Scripted {
    fn next_double(&mut self) -> f64 {
        self.doubles.pop_front().expect("out of doubles")
    }
    fn get_int(&mut self, bound: i32) -> i32 {
        let value = self.ints.pop_front().expect("out of ints");
        assert!((0..bound).contains(&value));
        value
    }
    fn get_long_range(&mut self, origin: i64, bound: i64) -> i64 {
        assert_eq!(origin, bound, "the tests use fixed counts");
        origin
    }
    fn next_boolean(&mut self) -> bool {
        unreachable!()
    }
    fn next_bytes(&mut self, _bytes: &mut [u8]) {
        unreachable!()
    }
    fn get_int_range(&mut self, _origin: i32, _bound: i32) -> i32 {
        unreachable!()
    }
    fn next_int(&mut self) -> i32 {
        unreachable!()
    }
    fn get_long(&mut self, _bound: i64) -> i64 {
        unreachable!()
    }
    fn next_long(&mut self) -> i64 {
        unreachable!()
    }
    fn next_float(&mut self) -> f32 {
        unreachable!()
    }
    fn get_double(&mut self, _bound: f64) -> f64 {
        unreachable!()
    }
    fn get_double_range(&mut self, _origin: f64, _bound: f64) -> f64 {
        unreachable!()
    }
    fn next_gaussian(&mut self) -> f64 {
        unreachable!()
    }
}

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="1060" type="EtcItem" name="Potion"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
<item id="4356" type="EtcItem" name="Bonus"><set name="is_stackable" val="true" /><set name="weight" val="1" /></item>
</list>"#;

fn monster_with(drops: &str) -> NpcTemplates {
    NpcTemplates::from_xml(
        &format!(r#"<list><npc id="1" type="Monster" level="10" name="M"><dropLists><drop>{drops}</drop></dropLists></npc></list>"#),
        &NpcLoadOptions::default(),
    )
    .unwrap()
}

fn roll(settings: &DropSettings, npcs: &NpcTemplates, champion: bool, rng: &mut Scripted) -> Vec<(i32, i64)> {
    let items = ItemTemplates::from_xml(ITEMS).unwrap();
    let victim = DropVictim {
        level: 10,
        raid: false,
        champion,
        seeded: false,
    };
    calculate_drops(
        settings,
        &items,
        npcs.get(1).unwrap(),
        DropType::Drop,
        &victim,
        &DropKiller::plain(10),
        rng,
    )
    .into_iter()
    .map(|h| (h.id, h.count))
    .collect()
}

fn champion_drops() -> ChampionDrops {
    ChampionDrops {
        rewards_chance: 2.0,
        rewards_amount: 3.0,
        adenas_rewards_chance: 1.5,
        adenas_rewards_amount: 4.0,
        lower_level_item_chance: 0,
        higher_level_item_chance: 100,
        reward_items: vec![ItemHolder { id: 4356, count: 10 }],
    }
}

/// Without the champion values (the feature is off), a champion victim drops like any monster and
/// draws nothing extra.
#[test]
fn champion_drops_off_nothing_changes() {
    let npcs = monster_with(r#"<item id="1060" min="2" max="2" chance="30"/>"#);
    let settings = DropSettings::default();
    // gap, chance (0.5 * 100 = 50 is not below 30).
    let mut rng = Scripted {
        doubles: [0.5, 0.5].into(),
        ..Scripted::default()
    };
    assert!(roll(&settings, &npcs, true, &mut rng).is_empty());
    // A drop that rolls is the plain amount.
    let mut rng = Scripted {
        doubles: [0.5, 0.1].into(),
        ..Scripted::default()
    };
    assert_eq!(roll(&settings, &npcs, true, &mut rng), [(1060, 2)]);
}

/// A champion: the chance and the amount are multiplied, and the reward items follow when
/// `Rnd.get(100)` is below the chance of its level case.
#[test]
fn champion_drops_on_multiply_and_add_the_reward_items() {
    let npcs = monster_with(r#"<item id="1060" min="2" max="2" chance="30"/>"#);
    let settings = DropSettings {
        champion: Some(champion_drops()),
        ..DropSettings::default()
    };
    // 30 * 2 = 60 > 50: the potion drops, 2 * 3 = 6 of them; equal levels use the higher-level
    // chance (100), the draw 99 is below it: the reward items are added.
    let mut rng = Scripted {
        doubles: [0.5, 0.5].into(),
        ints: [99].into(),
    };
    assert_eq!(roll(&settings, &npcs, true, &mut rng), [(1060, 6), (4356, 10)]);
    // The same monster, not a champion: 50 is not below 30 and no extra draw.
    let mut rng = Scripted {
        doubles: [0.5, 0.5].into(),
        ..Scripted::default()
    };
    assert!(roll(&settings, &npcs, false, &mut rng).is_empty());
    // The reward roll fails at 100 (not below 100): nothing is added.
    let mut rng = Scripted {
        doubles: [0.5, 0.9].into(),
        ints: [0].into(),
    };
    assert_eq!(roll(&settings, &npcs, true, &mut rng), [(4356, 10)]);
}

/// `RATE_DROP_CHANCE_BY_ID` / `RATE_DROP_AMOUNT_BY_ID` of adena: the champion adena factors come
/// on top of the by-id rate, and the chance is capped at 100 afterwards.
#[test]
fn champion_adena_factors_follow_the_by_id_rates() {
    let npcs = monster_with(r#"<item id="57" min="10" max="10" chance="20"/>"#);
    let mut settings = DropSettings {
        champion: Some(champion_drops()),
        ..DropSettings::default()
    };
    settings.rate_drop_chance_by_id.insert(57, 2.0);
    settings.rate_drop_amount_by_id.insert(57, 2.0);
    // 20 * (2 * 1.5) = 60 > 55.
    let mut rng = Scripted {
        doubles: [0.5, 0.55].into(),
        ints: [100 - 1].into(),
    };
    // Amount: 10 * (2 * 4) = 80; the reward items follow.
    assert_eq!(roll(&settings, &npcs, true, &mut rng), [(57, 80), (4356, 10)]);
}

// ------------------------------------------------------------------ boss announcements

fn announcements(f: impl FnOnce(&mut BossAnnouncementSettings)) -> CustomSettings {
    let mut custom = CustomSettings::default();
    f(&mut custom.boss_announcements);
    custom
}

/// What the player received, as `(opcode name, contains the text)` of the two announcement
/// packets.
fn announced(log: &Log, text: &str) -> (usize, usize) {
    let wide: Vec<u8> = text.encode_utf16().flat_map(u16::to_le_bytes).collect();
    let has_text = |b: &[u8]| b.windows(wide.len()).any(|w| w == wide.as_slice());
    let sent = log.take();
    let count = |name: &str| {
        sent.iter()
            .filter(|(_, b)| name_of(b) == name && has_text(b))
            .count()
    };
    (count("SAY2"), count("EX_SHOW_SCREEN_MESSAGE"))
}

#[test]
fn boss_announcements_off_say_nothing() {
    let (mut world, log, _dir) = world_with(CustomSettings::default(), NpcSettings::default());
    enter(&mut world, &log, 10);
    let boss = world.spawn_npc_at(3000, X, Y, Z, 0).unwrap();
    world.do_die(boss, None);
    let sent = log.take();
    assert!(
        !sent.iter().any(|(_, b)| name_of(b) == "EX_SHOW_SCREEN_MESSAGE"),
        "no announcement packets"
    );
}

#[test]
fn raid_boss_spawn_and_defeat_are_announced_to_everybody() {
    let custom = announcements(|a| {
        a.raidboss_spawn_announcements = true;
        a.raidboss_defeat_announcements = true;
    });
    let (mut world, log, _dir) = world_with(custom, NpcSettings::default());
    enter(&mut world, &log, 10);
    let boss = world.spawn_npc_at(3000, X, Y, Z, 0).unwrap();
    // The chat line and the 10 s on-screen message, both with the template name.
    assert_eq!(announced(&log, "Raid Alpha has spawned!"), (1, 1));
    world.do_die(boss, None);
    assert_eq!(announced(&log, "Raid Alpha has been defeated!"), (1, 1));
}

/// A grand boss follows its own switches; the raid boss switches do not apply to it, and the
/// exclusion lists name bosses by npc id.
#[test]
fn grand_boss_switches_and_exclusions() {
    let custom = announcements(|a| {
        a.raidboss_spawn_announcements = true;
        a.grandboss_defeat_announcements = true;
        a.raidbosses_excluded_from_spawn_announcements = BTreeSet::from([3002]);
        a.raidbosses_excluded_from_defeat_announcements = BTreeSet::from([3001]);
    });
    let (mut world, log, _dir) = world_with(custom, NpcSettings::default());
    enter(&mut world, &log, 10);
    // The grand boss spawn switch is off: the raid boss one does not apply.
    let grand = world.spawn_npc_at(3001, X, Y, Z, 0).unwrap();
    assert_eq!(announced(&log, "Grand Beta has spawned!"), (0, 0));
    // The raid boss in the exclusion list is silent, the other one is not.
    world.spawn_npc_at(3002, X, Y, Z, 0).unwrap();
    assert_eq!(announced(&log, "Raid Gamma has spawned!"), (0, 0));
    world.spawn_npc_at(3000, X, Y, Z, 0).unwrap();
    assert_eq!(announced(&log, "Raid Alpha has spawned!"), (1, 1));
    // The grand boss is excluded from the defeat announcements too (it shares the lists).
    world.do_die(grand, None);
    assert_eq!(announced(&log, "Grand Beta has been defeated!"), (0, 0));
}

/// A minion is never announced (`!isMinion()`), even when it is a raid boss template itself.
#[test]
fn minions_are_not_announced() {
    let custom = announcements(|a| a.raidboss_spawn_announcements = true);
    let (mut world, log, _dir) = world_with(custom, NpcSettings::default());
    enter(&mut world, &log, 10);
    let master = world.spawn_npc_at(3003, X, Y, Z, 0).unwrap();
    assert_eq!(announced(&log, "Raid Master has spawned!"), (1, 1));
    assert_eq!(world.spawn_minions_of(master), 1);
    assert_eq!(announced(&log, "Raid Gamma has spawned!"), (0, 0));
}

// ------------------------------------------------------------------ random spawns

fn random_spawn_world(enabled: bool) -> World {
    let spawns = format!(
        r#"<spawn><npc id="2000" x="{X}" y="{Y}" z="{Z}" heading="5" count="8"/></spawn>
           <spawn><npc id="2001" x="{X}" y="{Y}" z="{Z}" heading="5"/></spawn>
           <spawn><npc id="1000" x="{X}" y="{Y}" z="{Z}" heading="5"/></spawn>"#
    );
    let npc_settings = NpcSettings {
        random_monster_spawns: enabled,
        mob_max_spawn_range: 150,
        mobs_not_random: BTreeSet::from([2001]).into_iter().collect(),
        ..NpcSettings::default()
    };
    let (mut world, _log, dir) = world_spawning(&spawns, CustomSettings::default(), npc_settings);
    // The temporary datapack folder is no longer needed once the spawn list is loaded.
    drop(dir);
    world.spawn_all_npcs(false);
    world
}

fn places_of(world: &World, npc_id: i32) -> Vec<(i32, i32, i32)> {
    world
        .npc_ids()
        .into_iter()
        .filter_map(|id| world.npc(id))
        .filter(|n| n.npc_id == npc_id)
        .map(|n| (n.core.x - X, n.core.y - Y, n.core.heading))
        .collect()
}

/// `EnableRandomMonsterSpawns = False` (the default): every NPC stands where the XML puts it.
#[test]
fn random_spawns_off_keep_the_xml_places() {
    let world = random_spawn_world(false);
    assert_eq!(places_of(&world, 2000), vec![(0, 0, 5); 8]);
}

/// On: a monster is moved by up to `MaxSpawnMobRange` in x and y and gets a random heading; the
/// ids of `MobsSpawnNotRandom`, raid bosses and NPCs that are no monsters stay where they are.
#[test]
fn random_spawns_on_move_monsters_but_not_the_excluded() {
    let world = random_spawn_world(true);
    let moved = places_of(&world, 2000);
    assert_eq!(moved.len(), 8);
    assert!(
        moved
            .iter()
            .all(|&(dx, dy, _)| dx.abs() <= 150 && dy.abs() <= 150),
        "{moved:?}"
    );
    assert!(
        moved.iter().any(|&(dx, dy, h)| (dx, dy) != (0, 0) && h != 5),
        "{moved:?}"
    );
    for fixed in [2001, 1000] {
        assert_eq!(places_of(&world, fixed), vec![(0, 0, 5)], "npc {fixed}");
    }
}

/// `Quest.addSpawn` without a random offset keeps the place it was given, even with random spawns
/// on ("Retain monster original position"); the heading is the one asked for.
#[test]
fn script_spawns_keep_their_place_with_random_spawns_on() {
    let npc_settings = NpcSettings {
        random_monster_spawns: true,
        ..NpcSettings::default()
    };
    let (mut world, _log, _dir) = world_with(CustomSettings::default(), npc_settings);
    for _ in 0..8 {
        let npc = world.script_add_spawn(2000, (X, Y, Z), 77, false, 0, 0).unwrap();
        let n = world.npc(npc).unwrap();
        assert_eq!((n.core.x, n.core.y, n.core.heading), (X, Y, 77));
    }
}
