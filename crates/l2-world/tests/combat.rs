#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Auto attack (M4-COMBAT-01): the hit pipeline against the Java vectors of M4-JV-01, and
//! scenarios through the world with a recording client: a player kills a weak monster, the attack
//! stance starts and ends, a target out of range is chased, a monster hits a player, bows need
//! arrows, a move asked during an attack waits for its end.
//!
//! The scenarios use small synthetic templates (they run in CI). The vector replay builds the
//! vector creatures from the real datapack (skipped when `C:\l2-rust\l2jmobius\dist\game` or
//! `L2JMOBIUS_GAME_DIR` is missing) and runs `combat::hit::calc_hit_simple`, the very function the
//! attack uses, with the recorded `Rnd` draws.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, OnceLock};

use l2_data::item::BodyPart;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceTable, Hennas, HitConditionBonus, ItemTemplates, NpcLoadOptions,
    NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates, SpawnData,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::charscreen::paperdoll;
use l2_protocol_game::server::visibility::status;
use l2_protocol_game::server::{
    Attack, AutoAttackStart, AutoAttackStop, MoveToPawn, SetupGauge, StatusUpdate, SystemMessage,
    SystemMessageParam,
};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::combat::hit::calc_hit_simple;
use l2_world::formulas::Rnd;
use l2_world::*;
use serde_json::Value;

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

fn message_id(raw: &[u8]) -> SystemMessageId {
    decode::<SystemMessage>(raw).id
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

/// Class 0 at level 1: P. Atk. 4, 300 attack speed, 100 HP, collision radius 9.
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

/// A sword without random damage (so every hit that lands without a critical does the same
/// damage), a bow and arrows of no grade.
const ITEMS: &str = r#"<list>
<item id="2" type="Weapon" name="Sword"><set name="weapon_type" val="SWORD" /><set name="bodypart" val="rhand" />
<stats><stat type="pAtk">50</stat><stat type="mAtk">40</stat><stat type="pAtkSpd">379</stat><stat type="pAtkRange">40</stat></stats></item>
<item id="13" type="Weapon" name="Bow"><set name="weapon_type" val="BOW" /><set name="bodypart" val="lrhand" />
<stats><stat type="pAtk">40</stat><stat type="mAtk">10</stat><stat type="pAtkSpd">293</stat><stat type="pAtkRange">500</stat></stats></item>
<item id="17" type="EtcItem" name="Wooden Arrow"><set name="etcitem_type" val="ARROW" /><set name="bodypart" val="lhand" /><set name="is_stackable" val="true" /></item>
</list>"#;

/// A weak monster (300 HP, so a few sword hits) and a stronger one that hits players.
const NPCS: &str = r#"<?xml version="1.0"?><list>
  <npc id="9001" type="Monster" level="1" name="Weak Monster">
    <race>FAIRY</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="300" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="20" /><run ground="50" /></speed>
    </stats>
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
const OTHER: i32 = 0x2000_0002;

struct Scene {
    world: World,
    log: Log,
}

impl Scene {
    fn new() -> Self {
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(20_261_019), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data(synthetic_stats())));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(
            Arc::new(NpcTemplates::from_xml(NPCS, &NpcLoadOptions::default()).unwrap()),
            Arc::new(SpawnData::default()),
        );
        Self { world, log }
    }

    fn enter(&mut self, id: i32, name: &str, pos: (i32, i32, i32), items: Vec<ItemData>) {
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(player_data(id, name, pos, items)),
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
}

fn sword() -> Vec<ItemData> {
    vec![item(0x3000_0001, 2, "PAPERDOLL", paperdoll::RHAND, 1)]
}

// ------------------------------------------------------------------ scenarios

/// A player with a sword clicks a weak monster twice and kills it. Every landed hit does the same
/// damage (the sword has no random damage); the monster dies on the first hit that brings the
/// total to its 300 HP, its HP bar follows every hit, and the player stops attacking afterwards.
#[test]
fn player_kills_a_weak_monster() {
    let mut s = Scene::new();
    s.enter(ME, "Hero", (X, Y, Z), sword());
    let npc = s.world.spawn_npc_at(9001, X + 40, Y, Z, 0).unwrap();
    s.log.take();

    // First click: target. Second click: attack.
    s.click(ME, npc);
    s.log.take();
    s.click(ME, npc);
    let first = s.log.take();
    let me = names(&first, ME);
    assert!(me.contains(&"ATTACK".to_owned()), "{me:?}");
    assert_eq!(
        me.last().map(String::as_str),
        Some("ACTION_FAIL"),
        "onAction ends with ActionFailed"
    );
    assert_eq!(s.world.intention_of(ObjectId(ME)), Intention::Attack);
    assert_eq!(s.world.attack_target_of(ObjectId(ME)), Some(npc));

    let sent = s.run_until(2000, |w| w.is_dead(npc));
    let mut all = first;
    all.extend(sent);
    // Every landed hit of the attacks, in order, from the Attack packets (the run stops at the
    // death, so every attack sent so far has landed).
    // (The monster hits back since M4-AI-01: its own `Attack` packets are not counted here.)
    let attacks: Vec<Attack> = of(&all, ME, "ATTACK")
        .iter()
        .map(|raw| decode::<Attack>(raw))
        .filter(|a| a.attacker_id == ME)
        .collect();
    let mut landed = Vec::new();
    for a in &attacks {
        assert!(a.other_hits.is_empty(), "a sword hits once");
        let hit = a.first_hit;
        assert_eq!(hit.target_id, npc.0);
        if hit.flags & 0x80 != 0 {
            assert_eq!(hit.damage, 0, "a miss does no damage");
        } else {
            landed.push(hit.damage);
        }
    }
    assert!(landed.len() >= 2, "300 HP takes more than one hit: {landed:?}");

    // The HP bar (StatusUpdate to the targeting player, a status listener) goes down by each
    // landed hit and up by the monster's regeneration in between (it starts with the first
    // damage); the last landed hit takes it to 0.
    let bars: Vec<(i32, i32)> = of(&all, ME, "STATUS_UPDATE")
        .iter()
        .map(|raw| decode::<StatusUpdate>(raw))
        .filter(|u| u.object_id == npc.0)
        .map(|u| (u.attributes[0].1, u.attributes[1].1))
        .collect();
    let mut hp = 300;
    let mut hits = landed.iter();
    for (max, value) in &bars {
        assert_eq!(*max, 300);
        if *value == 0 && hp == 0 {
            // `doDie` broadcasts the status once more (Java `Creature.doDie`).
            continue;
        }
        if *value > hp {
            assert!(value - hp <= 10, "regeneration, not a jump: {bars:?}");
        } else {
            let d = hits
                .next()
                .unwrap_or_else(|| panic!("a drop without a hit: {bars:?}"));
            assert_eq!(*value, (hp - d).max(0), "{bars:?}");
        }
        hp = *value;
    }
    assert!(hits.next().is_none(), "every landed hit moved the bar: {bars:?}");
    assert_eq!(hp, 0);

    // Damage messages: one per landed hit, "went astray" per miss.
    let messages: Vec<SystemMessage> = of(&all, ME, "SYSTEM_MESSAGE")
        .iter()
        .map(|raw| decode::<SystemMessage>(raw))
        .collect();
    let done: Vec<&SystemMessage> = messages
        .iter()
        .filter(|m| m.id == SystemMessageId::C1HasDoneS3PointsOfDamageToC2)
        .collect();
    assert_eq!(done.len(), landed.len());
    assert_eq!(
        done[0].params,
        vec![
            SystemMessageParam::PlayerName("Hero".into()),
            SystemMessageParam::Text("Weak Monster".into()),
            SystemMessageParam::IntNumber(landed[0]),
        ]
    );
    let aggro = s.world.aggro_list(npc);
    assert_eq!(aggro.len(), 1);
    assert_eq!(aggro[0].attacker, ObjectId(ME));
    assert_eq!(aggro[0].damage, i64::from(landed.iter().sum::<i32>()));

    // The next "ready to act" finds the target dead: the player goes idle, no more attacks.
    let after = s.run_until(100, |w| w.intention_of(ObjectId(ME)) == Intention::Idle);
    s.ticks(100);
    let mut rest = after;
    rest.extend(s.log.take());
    assert!(of(&rest, ME, "ATTACK").is_empty(), "{:?}", names(&rest, ME));
    assert_eq!(s.world.attack_target_of(ObjectId(ME)), None);
}

/// The first landed hit puts both into combat (`AutoAttackStart`); 15 to 16 seconds after the
/// last refresh the stance manager ends it (`AutoAttackStop`).
#[test]
fn attack_stance_starts_on_the_hit_and_ends_after_15_seconds() {
    let mut s = Scene::new();
    s.enter(ME, "Hero", (X, Y, Z), sword());
    let npc = s.world.spawn_npc_at(9001, X + 40, Y, Z, 0).unwrap();
    s.click(ME, npc);
    s.click(ME, npc);
    s.log.take();
    // Run until the first landed hit.
    let sent = s.run_until(400, |w| w.is_auto_attacking(ObjectId(ME)));
    let starts: Vec<i32> = of(&sent, ME, "AUTO_ATTACK_START")
        .iter()
        .map(|raw| decode::<AutoAttackStart>(raw).target_id)
        .collect();
    // The monster enters combat when it is hit, the player right after.
    assert_eq!(starts, vec![npc.0, ME]);
    assert!(s.world.has_attack_stance_task(ObjectId(ME)));

    // The monster fights back since M4-AI-01 and would keep the stance alive: it goes away.
    s.world.delete_npc(npc);
    // Stop attacking: walk away (a move during the attack animation waits for its end).
    s.input(
        ME,
        ClientInput::MoveTo {
            target_x: X - 400,
            target_y: Y,
            target_z: Z,
            origin_x: X,
            origin_y: Y,
            origin_z: Z,
            movement_mode: 1,
        },
    );
    let stop_at_most = 20 * 17 + 40;
    let mut ticks = 0;
    let mut stops = Vec::new();
    while ticks < stop_at_most {
        s.world.tick();
        ticks += 1;
        let sent = s.log.take();
        stops.extend(
            of(&sent, ME, "AUTO_ATTACK_STOP")
                .iter()
                .map(|raw| decode::<AutoAttackStop>(raw).target_id),
        );
        if stops.contains(&ME) {
            break;
        }
    }
    assert!(stops.contains(&ME), "the player's stance ended");
    assert!(!s.world.has_attack_stance_task(ObjectId(ME)));
    assert!(!s.world.is_auto_attacking(ObjectId(ME)));
    assert!(
        ticks >= 15 * 20 - 40,
        "not before 15 s after the last refresh ({ticks} ticks)"
    );
}

/// A target out of reach is chased: `MoveToPawn` first, the attack once in range.
#[test]
fn out_of_range_target_is_chased_then_attacked() {
    let mut s = Scene::new();
    s.enter(ME, "Hero", (X, Y, Z), sword());
    let npc = s.world.spawn_npc_at(9001, X + 400, Y, Z, 0).unwrap();
    s.click(ME, npc);
    s.log.take();
    s.click(ME, npc);
    let first = s.log.take();
    let me = names(&first, ME);
    assert!(me.contains(&"MOVE_TO_PAWN".to_owned()), "{me:?}");
    assert!(!me.contains(&"ATTACK".to_owned()), "{me:?}");
    let pawn = decode::<MoveToPawn>(of(&first, ME, "MOVE_TO_PAWN")[0]);
    assert_eq!(pawn.target_id, npc.0);
    assert_eq!(pawn.distance, 40, "the physical attack range");

    let sent = s.run_until(400, |w| w.is_attacking_now(ObjectId(ME)));
    let names = names(&sent, ME);
    let attack = names.iter().position(|n| n == "ATTACK").unwrap();
    assert!(
        names[..attack].contains(&"STOP_MOVE".to_owned()),
        "arrived before attacking: {names:?}"
    );
    let (px, _, _) = s.world.position_of(ObjectId(ME)).unwrap();
    // Within reach: 40 + both collision radii (9 + 10) from the monster.
    assert!((X + 400) - px < 40 + 19, "at {px}");
}

/// A monster that attacks a player: `Attack` from the monster, then on the hit the player enters
/// combat, reads the damage message and gets its HP 50 ms later.
#[test]
fn monster_hits_a_player() {
    let mut s = Scene::new();
    s.enter(ME, "Hero", (X, Y, Z), vec![]);
    let npc = s.world.spawn_npc_at(9001, X + 30, Y, Z, 0).unwrap();
    s.log.take();
    s.world.do_attack(npc, ObjectId(ME));
    let first = s.log.take();
    let attack = decode::<Attack>(of(&first, ME, "ATTACK")[0]);
    assert_eq!(attack.attacker_id, npc.0);
    assert_eq!(attack.first_hit.target_id, ME);
    let missed = attack.first_hit.flags & 0x80 != 0;

    let sent = s.run_until(100, |w| !w.is_attacking_now(npc));
    let me = names(&sent, ME);
    if missed {
        let msgs: Vec<SystemMessageId> = of(&sent, ME, "SYSTEM_MESSAGE")
            .iter()
            .map(|r| message_id(r))
            .collect();
        assert_eq!(msgs, vec![SystemMessageId::C1HasEvadedC2SAttack]);
    } else {
        // AutoAttackStart (the player, then the monster), the damage message, then (50 ms
        // later) the player's StatusUpdate.
        let start = me.iter().position(|n| n == "AUTO_ATTACK_START").unwrap();
        let message = me.iter().position(|n| n == "SYSTEM_MESSAGE").unwrap();
        let update = me.iter().position(|n| n == "STATUS_UPDATE").unwrap();
        assert!(start < message && message < update, "{me:?}");
        let msg = decode::<SystemMessage>(of(&sent, ME, "SYSTEM_MESSAGE")[0]);
        assert_eq!(msg.id, SystemMessageId::C1HasReceivedS3DamageFromC2);
        assert_eq!(
            msg.params[..2],
            [
                SystemMessageParam::Text("Hero".into()),
                SystemMessageParam::Text("Weak Monster".into())
            ]
        );
        let damage = attack.first_hit.damage;
        let update = decode::<StatusUpdate>(of(&sent, ME, "STATUS_UPDATE")[0]);
        let cur_hp = update
            .attributes
            .iter()
            .find(|(id, _)| *id == status::CUR_HP)
            .unwrap()
            .1;
        // A monster's hit does not go through CP.
        assert_eq!(cur_hp, 100 - damage);
        assert!(s.world.has_attack_stance_task(ObjectId(ME)));
    }
}

/// A bow needs arrows: without any the attack is refused (`YouHaveRunOutOfArrows`); with arrows
/// in the bag they go to the left hand (item lists), the bow shows its gauge and shoots.
#[test]
fn bows_need_arrows() {
    let bow = item(0x3000_0010, 13, "PAPERDOLL", paperdoll::RHAND, 1);
    let mut s = Scene::new();
    s.enter(ME, "Archer", (X, Y, Z), vec![bow.clone()]);
    let npc = s.world.spawn_npc_at(9001, X + 200, Y, Z, 0).unwrap();
    s.click(ME, npc);
    s.log.take();
    s.click(ME, npc);
    let sent = s.log.take();
    let msgs: Vec<SystemMessageId> = of(&sent, ME, "SYSTEM_MESSAGE")
        .iter()
        .map(|r| message_id(r))
        .collect();
    assert_eq!(msgs, vec![SystemMessageId::YouHaveRunOutOfArrows]);
    assert!(of(&sent, ME, "ATTACK").is_empty());
    assert_eq!(s.world.intention_of(ObjectId(ME)), Intention::Idle);

    let mut s = Scene::new();
    let arrows = item(0x3000_0011, 17, "INVENTORY", 0, 100);
    s.enter(ME, "Archer", (X, Y, Z), vec![bow, arrows]);
    let npc = s.world.spawn_npc_at(9001, X + 200, Y, Z, 0).unwrap();
    s.click(ME, npc);
    s.log.take();
    s.click(ME, npc);
    let sent = s.log.take();
    let me = names(&sent, ME);
    assert_eq!(me, ["SETUP_GAUGE", "ATTACK", "ACTION_FAIL"]);
    let g = decode::<SetupGauge>(of(&sent, ME, "SETUP_GAUGE")[0]);
    assert_eq!(g.color, SetupGauge::RED);
    // The arrows went to the left hand (`setPaperdollItem`) and one was shot (`reduceArrowCount`).
    let left = |s: &Scene| {
        s.world
            .player(ObjectId(ME))
            .unwrap()
            .data
            .items
            .iter()
            .find(|i| i.loc == "PAPERDOLL" && i.loc_data == paperdoll::LHAND as i32)
            .cloned()
    };
    let arrows = left(&s).unwrap();
    assert_eq!((arrows.item_id, arrows.count), (17, 99));
    assert!(s.world.is_range_attacking_now(ObjectId(ME)));
    // The count goes out in an `InventoryUpdate` (100 ms), the item list after 250 ms
    // (`sendItemList(false)`).
    s.ticks(5);
    let later = names(&s.log.take(), ME);
    let update = later.iter().position(|n| n == "INVENTORY_UPDATE").unwrap();
    let list = later.iter().position(|n| n == "ITEM_LIST").unwrap();
    assert!(update < list, "{later:?}");
}

/// The last arrow is destroyed and leaves the left hand; the next shot is refused.
#[test]
fn the_last_arrow_is_used_up() {
    let bow = item(0x3000_0010, 13, "PAPERDOLL", paperdoll::RHAND, 1);
    let arrow = item(0x3000_0011, 17, "PAPERDOLL", paperdoll::LHAND, 1);
    let mut s = Scene::new();
    s.enter(ME, "Archer", (X, Y, Z), vec![bow, arrow]);
    let npc = s.world.spawn_npc_at(9001, X + 200, Y, Z, 0).unwrap();
    s.click(ME, npc);
    s.click(ME, npc);
    let sent = s.log.take();
    assert!(!of(&sent, ME, "ATTACK").is_empty());
    let p = s.world.player(ObjectId(ME)).unwrap();
    assert!(
        p.data.items.iter().all(|i| i.item_id != 17),
        "the last arrow is gone"
    );
    // The bow can shoot again after its reuse: then there are no arrows left.
    let later = s.run_until(200, |w| w.intention_of(ObjectId(ME)) == Intention::Idle);
    let msgs: Vec<SystemMessageId> = of(&later, ME, "SYSTEM_MESSAGE")
        .iter()
        .map(|r| message_id(r))
        .collect();
    assert!(msgs.contains(&SystemMessageId::YouHaveRunOutOfArrows), "{msgs:?}");
}

/// An item used during the attack animation is equipped when the attack ends (Java `UseItem`),
/// and taking an item off during the animation is refused (`RequestUnEquipItem`).
#[test]
fn equipping_waits_for_the_attack_end() {
    let mut s = Scene::new();
    let spare = item(0x3000_0002, 2, "INVENTORY", 0, 1);
    let mut items = sword();
    items.push(spare);
    s.enter(ME, "Hero", (X, Y, Z), items);
    let npc = s.world.spawn_npc_at(9001, X + 40, Y, Z, 0).unwrap();
    s.click(ME, npc);
    s.click(ME, npc);
    assert!(s.world.is_attacking_now(ObjectId(ME)));
    s.log.take();
    s.input(
        ME,
        ClientInput::UnEquipItem {
            paperdoll_slot: paperdoll::RHAND as i32,
        },
    );
    let msgs: Vec<SystemMessageId> = of(&s.log.take(), ME, "SYSTEM_MESSAGE")
        .iter()
        .map(|r| message_id(r))
        .collect();
    assert_eq!(msgs, vec![SystemMessageId::YouCannotChangeWeaponsDuringAnAttack]);

    s.input(
        ME,
        ClientInput::UseItem {
            object_id: 0x3000_0002,
            ctrl_pressed: false,
        },
    );
    let worn = |s: &Scene| s.world.paperdoll_object(ObjectId(ME), paperdoll::RHAND);
    assert_eq!(
        worn(&s),
        Some(ObjectId(0x3000_0001)),
        "not before the attack ends"
    );
    // The player keeps attacking (the next attack starts on the tick the last one ends), so the
    // time is the end of the attack that was running, not a pause.
    let end_ms = s.world.combat_state(ObjectId(ME)).unwrap().attack_end_ms;
    while s.world.world_ms() < end_ms - 50 {
        s.world.tick();
        assert_eq!(
            worn(&s),
            Some(ObjectId(0x3000_0001)),
            "not before the attack ends"
        );
    }
    s.ticks(1);
    assert_eq!(
        worn(&s),
        Some(ObjectId(0x3000_0002)),
        "equipped when the attack ended"
    );
}

/// A move asked while the attack animation runs is refused (`ActionFailed`) and replayed when
/// the attack is over (Java `PlayerAI._savedIntention`).
#[test]
fn move_during_attack_waits_for_the_attack_end() {
    let mut s = Scene::new();
    s.enter(ME, "Hero", (X, Y, Z), sword());
    let npc = s.world.spawn_npc_at(9001, X + 40, Y, Z, 0).unwrap();
    s.click(ME, npc);
    s.click(ME, npc);
    assert!(s.world.is_attacking_now(ObjectId(ME)));
    s.log.take();
    s.input(
        ME,
        ClientInput::MoveTo {
            target_x: X - 300,
            target_y: Y,
            target_z: Z,
            origin_x: X,
            origin_y: Y,
            origin_z: Z,
            movement_mode: 1,
        },
    );
    assert_eq!(names(&s.log.take(), ME), vec!["ACTION_FAIL"]);
    assert!(!s.world.is_moving(ObjectId(ME)));
    let sent = s.run_until(200, |w| w.is_moving(ObjectId(ME)));
    assert!(names(&sent, ME).contains(&"MOVE_TO_LOCATION".to_owned()));
    assert_eq!(s.world.intention_of(ObjectId(ME)), Intention::MoveTo);
    // Java's `setIntentionMoveTo` keeps `_attackTarget`; only the intention changes, so no more
    // attacks follow.
    s.ticks(40);
    assert!(of(&s.log.take(), ME, "ATTACK").is_empty());
}

/// Another player standing by sees the attack and the stance of the attacker.
#[test]
fn bystanders_see_the_attack() {
    let mut s = Scene::new();
    s.enter(ME, "Hero", (X, Y, Z), sword());
    s.enter(OTHER, "Watcher", (X - 100, Y, Z), vec![]);
    let npc = s.world.spawn_npc_at(9001, X + 40, Y, Z, 0).unwrap();
    s.click(ME, npc);
    s.click(ME, npc);
    let sent = s.log.take();
    let attack = decode::<Attack>(of(&sent, OTHER, "ATTACK")[0]);
    assert_eq!(attack.attacker_id, ME);
    // The attacker gets its own Attack first (`Player.broadcastPacket`).
    let first_attack = sent.iter().position(|(_, b)| name_of(b) == "ATTACK").unwrap();
    assert_eq!(sent[first_attack].0, ME);
}

// ------------------------------------------------------------------ Java vectors

fn game_root() -> Option<PathBuf> {
    let root = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"));
    if root.join("data/stats/skills").is_dir() {
        Some(root)
    } else {
        eprintln!("skipped: {} has no datapack", root.display());
        None
    }
}

fn vector(name: &str) -> Option<Value> {
    let path = l2_testkit::vectors::vector_path(name);
    if !path.is_file() {
        eprintln!("skipped: {} is missing", path.display());
        return None;
    }
    Some(l2_testkit::vectors::load(name))
}

fn s(v: &Value) -> &str {
    v.as_str().unwrap_or_else(|| panic!("not a string: {v}"))
}

fn i(v: &Value) -> i32 {
    i32::try_from(v.as_i64().unwrap_or_else(|| panic!("not an int: {v}"))).unwrap()
}

fn bonus_from(vectors: &Value) -> BaseStatBonus {
    let mut bonus = BaseStatBonus::loaded();
    for (key, stat) in [
        ("str", BaseStat::Str),
        ("int", BaseStat::Int),
        ("dex", BaseStat::Dex),
        ("wit", BaseStat::Wit),
        ("con", BaseStat::Con),
        ("men", BaseStat::Men),
    ] {
        for (value, b) in vectors[key].as_array().unwrap().iter().enumerate() {
            assert!(bonus.set(stat, value, b.as_f64().unwrap()));
        }
    }
    bonus
}

struct Tables {
    stats: StatData,
    npcs: Arc<NpcTemplates>,
}

fn tables(root: &Path) -> &'static Tables {
    static TABLES: OnceLock<Tables> = OnceLock::new();
    TABLES.get_or_init(|| {
        let max_level = ExperienceTable::load(root)
            .max_level()
            .unwrap_or(l2_data::DEFAULT_MAX_PLAYER_LEVEL);
        let skills = SkillTemplates::load(
            root,
            &SkillLoadOptions {
                custom_skills_load: true,
                ..SkillLoadOptions::default()
            },
        );
        Tables {
            stats: StatData {
                items: Arc::new(ItemTemplates::load(root, true)),
                player_templates: Arc::new(PlayerTemplates::load(root, max_level)),
                armor_sets: Arc::new(ArmorSets::load(root)),
                enchant_hp_bonus: Arc::new(EnchantHpBonus::load(root)),
                hennas: Arc::new(Hennas::load(root)),
                base_stat_bonus: Arc::new(bonus_from(&vector("stats_base_bonus").unwrap())),
                skills: Arc::new(skills),
            },
            npcs: Arc::new(NpcTemplates::load(
                root,
                &NpcLoadOptions {
                    custom_npc_data: true,
                    ..NpcLoadOptions::default()
                },
            )),
        }
    })
}

/// `Inventory.equipItem`'s slot for the vector gear (as `tests/stats_vectors.rs`).
fn equip(items: &mut Vec<ItemData>, t: &Tables, object_id: i32, item_id: i32, enchant: i32) {
    let template = t.stats.items.get(item_id).unwrap();
    let occupied = |items: &Vec<ItemData>, slot: usize| items.iter().any(|i| i.loc_data == slot as i32);
    let mut clear = Vec::new();
    let slot = match template.body_part {
        BodyPart::BothHands => {
            clear.push(paperdoll::LHAND);
            paperdoll::RHAND
        }
        BodyPart::RightHand => paperdoll::RHAND,
        BodyPart::LeftHand => {
            let rh_two_handed = items
                .iter()
                .find(|i| i.loc_data == paperdoll::RHAND as i32)
                .and_then(|i| t.stats.items.get(i.item_id))
                .is_some_and(|w| w.body_part == BodyPart::BothHands);
            if rh_two_handed {
                clear.push(paperdoll::RHAND);
            }
            paperdoll::LHAND
        }
        BodyPart::LeftEar | BodyPart::RightEar | BodyPart::BothEars => {
            if !occupied(items, paperdoll::LEAR) {
                paperdoll::LEAR
            } else if !occupied(items, paperdoll::REAR) {
                paperdoll::REAR
            } else {
                paperdoll::LEAR
            }
        }
        BodyPart::LeftFinger | BodyPart::RightFinger | BodyPart::BothFingers => {
            if !occupied(items, paperdoll::LFINGER) {
                paperdoll::LFINGER
            } else if !occupied(items, paperdoll::RFINGER) {
                paperdoll::RFINGER
            } else {
                paperdoll::LFINGER
            }
        }
        BodyPart::Neck => paperdoll::NECK,
        BodyPart::FullArmor => {
            clear.push(paperdoll::LEGS);
            paperdoll::CHEST
        }
        BodyPart::Chest => paperdoll::CHEST,
        BodyPart::Legs => paperdoll::LEGS,
        BodyPart::Feet => paperdoll::FEET,
        BodyPart::Gloves => paperdoll::GLOVES,
        BodyPart::Head => paperdoll::HEAD,
        BodyPart::Back => paperdoll::CLOAK,
        BodyPart::Underwear => paperdoll::UNDER,
        other => panic!("no slot for {other:?} in the vector gear"),
    };
    clear.push(slot);
    items.retain(|i| !clear.contains(&usize::try_from(i.loc_data).unwrap()));
    let mut it = item(object_id, item_id, "PAPERDOLL", slot, 1);
    it.enchant_level = enchant;
    items.push(it);
}

/// Puts a vector creature into the world at a position and heading.
fn place(
    world: &mut World,
    t: &Tables,
    input: &Value,
    id: i32,
    pos: (i32, i32, i32),
    heading: i32,
) -> ObjectId {
    if input["kind"] == "npc" {
        return world
            .spawn_npc_at(i(&input["npc_id"]), pos.0, pos.1, pos.2, heading)
            .unwrap();
    }
    let mut items = Vec::new();
    for (n, it) in input["items"].as_array().unwrap().iter().enumerate() {
        equip(
            &mut items,
            t,
            id + 0x100 + i32::try_from(n).unwrap(),
            i(&it["item_id"]),
            i(&it["enchant"]),
        );
    }
    let class_id = i(&input["class_id"]);
    let mut data = player_data(id, "Vector", pos, items);
    data.class_id = class_id;
    data.base_class = class_id;
    data.race = i32::from(l2_data::ClassId(u16::try_from(class_id).unwrap()).race().unwrap() as u8);
    data.sex = i32::from(input["female"].as_bool().unwrap());
    data.level = i(&input["level"]);
    data.heading = heading;
    data.skills = input["skills"]
        .as_array()
        .unwrap()
        .iter()
        .map(|sk| SkillData {
            skill_id: i(&sk["id"]),
            skill_level: i(&sk["level"]),
        })
        .collect();
    world.handle(WorldCommand::EnterWorld {
        session: SessionId(u64::try_from(id).unwrap()),
        player: Box::new(data),
        sink: Box::new(Sink(id, Log::default())),
        ip: None,
    });
    // The harness never makes its players run (see `tests/stats_vectors.rs`).
    world.player_mut(ObjectId(id)).unwrap().movement.running = false;
    ObjectId(id)
}

/// Answers the draws from the recorded `rnd` list and checks method and bound.
struct Scripted {
    expected: Vec<(String, String, i64)>,
    calls: Vec<(String, String)>,
}

impl Scripted {
    fn from_case(case: &Value) -> Self {
        let expected = case["rnd"]
            .as_array()
            .unwrap()
            .iter()
            .map(|d| {
                (
                    s(&d["method"]).to_string(),
                    s(&d["bound"]).to_string(),
                    s(&d["returned"]).parse().unwrap(),
                )
            })
            .collect();
        Self {
            expected,
            calls: Vec::new(),
        }
    }

    fn answer(&mut self, method: &str, bound: String) -> i64 {
        let value = self.expected.get(self.calls.len()).map_or(0, |d| d.2);
        self.calls.push((method.to_string(), bound));
        value
    }

    fn finish(self, id: &str) {
        let wanted: Vec<(String, String)> =
            self.expected.iter().map(|d| (d.0.clone(), d.1.clone())).collect();
        assert_eq!(self.calls, wanted, "{id}: the sequence of draws");
    }
}

impl Rnd for Scripted {
    fn next_boolean(&mut self) -> bool {
        self.answer("next_boolean", String::new()) != 0
    }
    fn next_bytes(&mut self, _bytes: &mut [u8]) {
        panic!("next_bytes");
    }
    fn get_int(&mut self, bound: i32) -> i32 {
        if bound <= 0 {
            return 0;
        }
        i32::try_from(self.answer("get_int", bound.to_string())).unwrap()
    }
    fn get_int_range(&mut self, origin: i32, bound: i32) -> i32 {
        if origin >= bound {
            return origin;
        }
        i32::try_from(self.answer("get_int_range", format!("{origin},{bound}"))).unwrap()
    }
    fn next_int(&mut self) -> i32 {
        panic!("next_int");
    }
    fn get_long(&mut self, _bound: i64) -> i64 {
        panic!("get_long");
    }
    fn get_long_range(&mut self, _origin: i64, _bound: i64) -> i64 {
        panic!("get_long_range");
    }
    fn next_long(&mut self) -> i64 {
        panic!("next_long");
    }
    fn next_float(&mut self) -> f32 {
        panic!("next_float");
    }
    fn get_double(&mut self, _bound: f64) -> f64 {
        panic!("get_double");
    }
    fn get_double_range(&mut self, _origin: f64, _bound: f64) -> f64 {
        panic!("get_double_range");
    }
    fn next_double(&mut self) -> f64 {
        panic!("next_double");
    }
    fn next_gaussian(&mut self) -> f64 {
        panic!("next_gaussian");
    }
}

/// `formulas_attack_hit.json`: the 240 cases of `doAttackHitSimple` (hit/miss, shield, critical,
/// damage) through the world's own pipeline: real creatures from the datapack, the formula inputs
/// read from the stat engine, the draws replayed in Java's order.
#[test]
fn attack_hit_vectors_replay_through_the_world() {
    let (Some(root), Some(file), Some(creatures)) = (
        game_root(),
        vector("formulas_attack_hit"),
        vector("formulas_creatures"),
    ) else {
        return;
    };
    let t = tables(&root);
    let config = &creatures["config"];
    assert_eq!(
        config["ClassBalance_arrays_not_all_1"].as_array().unwrap().len(),
        0,
        "the vectors are only valid with the default ClassBalance"
    );
    let floats = |v: &Value| -> Vec<f32> {
        v.as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_f64().unwrap() as f32)
            .collect()
    };
    let settings = WorldSettings {
        combat: CombatSettings {
            alt_game_shield_blocks: config["AltShieldBlocks"].as_bool().unwrap(),
            alt_perfect_shld_block: i(&config["AltPerfectShieldBlockRate"]),
            randomize_auto_attack_damage: config["RandomizeAutoAttackDamage"].as_bool().unwrap(),
            alt_game_cancel_cast: config["AltGameCancelByHit_cast"].as_bool().unwrap(),
            alt_game_cancel_bow: config["AltGameCancelByHit_bow"].as_bool().unwrap(),
            min_npc_level_dmg_penalty: i(&config["MinNPCLevelForDmgPenalty"]),
            npc_dmg_penalty: floats(&config["NpcDmgPenalty"]),
            npc_crit_dmg_penalty: floats(&config["NpcCritDmgPenalty"]),
            ..CombatSettings::default()
        },
        ..WorldSettings::default()
    };
    let hit_bonus = HitConditionBonus {
        front: i(&config["hitConditionBonus_frontBonus"]),
        side: i(&config["hitConditionBonus_sideBonus"]),
        back: i(&config["hitConditionBonus_backBonus"]),
        high: i(&config["hitConditionBonus_highBonus"]),
        low: i(&config["hitConditionBonus_lowBonus"]),
        dark: i(&config["hitConditionBonus_darkBonus"]),
        rain: 0,
    };
    let inputs: std::collections::HashMap<String, Value> = creatures["creatures"]
        .as_array()
        .unwrap()
        .iter()
        .map(|c| (s(&c["name"]).to_string(), c["input"].clone()))
        .collect();
    let g = &file["geometry"];
    let target_pos = (i(&g["target"]["x"]), i(&g["target"]["y"]), i(&g["target"]["z"]));
    let target_heading = i(&g["target"]["heading"]);

    let cases = file["cases"].as_array().unwrap();
    assert_eq!(cases.len(), 240);
    for case in cases {
        let id = s(&case["id"]);
        let placement = &case["placement"];
        let (ax, ay) = s(&g["attacker_xy_by_relation"][s(&placement["relation"])])
            .split_once(',')
            .map(|(x, y)| (x.parse::<i32>().unwrap(), y.parse::<i32>().unwrap()))
            .unwrap();
        let mut world = World::new(WorldRng::from_seed(1), Box::new(NoOutbox));
        world.set_static_data(Box::new(Data(t.stats.clone())));
        world.set_settings(settings.clone());
        world.set_hit_condition_bonus(Arc::new(hit_bonus));
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        world.set_npc_data(Arc::clone(&t.npcs), Arc::new(SpawnData::default()));
        // The target first (Java built both before placing them).
        let target = place(
            &mut world,
            t,
            &inputs[s(&case["target"])],
            0x2000_0002,
            target_pos,
            target_heading,
        );
        let attacker = place(
            &mut world,
            t,
            &inputs[s(&case["attacker"])],
            0x2000_0001,
            (ax, ay, target_pos.2 + i(&placement["z_diff"])),
            0,
        );
        let mut rng = Scripted::from_case(case);
        let outcome = calc_hit_simple(
            &world,
            attacker,
            target,
            100.0,
            case["ss"].as_bool().unwrap(),
            &mut rng,
        );
        let result = &case["result"];
        assert_eq!(outcome.miss, result["miss"].as_bool().unwrap(), "{id}: miss");
        assert_eq!(i32::from(outcome.shld), i(&result["shld"]), "{id}: shield");
        assert_eq!(outcome.crit, result["crit"].as_bool().unwrap(), "{id}: crit");
        assert_eq!(outcome.damage, i(&result["damage"]), "{id}: damage");
        rng.finish(id);
    }
    eprintln!("attack_hit (world pipeline): {} cases", cases.len());
}
