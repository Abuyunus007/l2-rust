#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Doors and walls as creatures and the siege guard AIs (M7-SIEGE-02): a gate broken by the
//! attackers of a GM test siege (the damage, `doDie`'s message, the door packets, the geodata), a
//! wall that only a siege weapon hurts, the doors repaired at the end of the siege, the guards
//! that attack the attackers and leave the defenders alone, and the guards a hit gate calls.
//! Java: entity/actor/instance/Door.java (reduceCurrentHp, doDie, broadcastStatusUpdate,
//! isAutoAttackable), entity/actor/stat/DoorStat.java, ai/DoorAI.java, ai/SiegeGuardAI.java,
//! entity/actor/instance/Defender.java, mechanics/siege/{Siege,Castle}.java (startSiege,
//! endSiege, spawnDoor). The expected values come from the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, DoorTemplates, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache,
    ItemTemplates, KarmaLoss, MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SiegeSchedule,
    SkillLoadOptions, SkillTemplates, SkillTrees, SpawnData, SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{DoorStatusUpdate, Revive, StaticObjectInfo, SystemMessage};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::ai::siege_guard::SiegeGuardAiKind;
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::effects::SkillKey;
use l2_world::movement::Intention;
use l2_world::residence::CastleRecord;
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

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
</staticData><lvlUpgainData><level val="1"><hp>100</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#;

/// A siege golem and a cat, summoned the way the `Summon` effect does it.
const SUMMON_GOLEM: i32 = 9201;
const SUMMON_CAT: i32 = 9202;
const GOLEM: i32 = 14_768;
const CAT: i32 = 14_111;

const SKILLS: &str = r#"<list>
<skill id="9201" levels="1" name="Summon Siege Golem"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="Summon"><npcId>14768</npcId></effect></effects></skill>
<skill id="9202" levels="1" name="Summon Cat"><operateType>A1</operateType><targetType>SELF</targetType>
<effects><effect name="Summon"><npcId>14111</npcId></effect></effects></skill>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml("<list></list>").unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(CLASS, 85).unwrap()),
        armor_sets: Arc::new(ArmorSets::default()),
        enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
        hennas: Arc::new(Hennas::default()),
        base_stat_bonus: Arc::new(BaseStatBonus::default()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (100, 0)] {
        rights.add_level(level, child);
    }
    for command in ["siege", "startsiege", "endsiege"] {
        rights.add_command(command, 100, false);
    }
    rights
}

struct Data {
    stats: StatData,
    rights: AdminRights,
}

impl StaticData for Data {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
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
                name_color: 0,
                title_color: 0,
                is_gm: true,
            }
        } else {
            AccessLevelStatic::USER
        }
    }
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.rights)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
}

const X: i32 = -18_000;
const Y: i32 = 110_000;
const Z: i32 = -2_500;
/// Lions (the owner): P1; Wolves (the attacker): P3 and P4; Bears: P5 outside; the GM: P9 far
/// away.
const P1: i32 = 0x6000_0001;
const P3: i32 = 0x6000_0003;
const P4: i32 = 0x6000_0004;
const P5: i32 = 0x6000_0005;
const P9: i32 = 0x6000_0009;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
const BEARS: i32 = 700;
const OUTSIDE: i32 = X + 20_000;

/// 2026-10-21 12:34:56.789 UTC, the siege clock at tick 0, and the next siege date.
const NOW: i64 = 1_792_586_096_789;
const NEXT_SIEGE: i64 = 1_793_548_800_789;

const GUARD: i32 = 35_085;
const GATE: i32 = 19_210_001;
const WALL: i32 = 19_210_003;
/// Where the guard stands: 141 from P1, 197 from P3, within the gate's clan help range.
const GUARD_X: i32 = X + 300;
const GUARD_Y: i32 = Y + 100;

fn npc_xml() -> String {
    format!(
        r#"<?xml version="1.0"?><list>
  <npc id="{GUARD}" type="Defender" level="70" name="Archer Guard">
    <race>HUMAN</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="500" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai type="ARCHER" aggroRange="0" clanHelpRange="1200" isAggressive="false" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>
  <npc id="{GOLEM}" type="SiegeSummon" level="70" name="Siege Golem">
    <race>SIEGE_WEAPON</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="5000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="300" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="60" /></speed>
    </stats>
    <collision><radius normal="20"/><height normal="40"/></collision>
  </npc>
  <npc id="{CAT}" type="Servitor" level="22" name="Kat the Cat">
    <race>BEAST</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="25">
      <vitals hp="387" hpRegen="3.5" mp="218" mpRegen="1.5" />
      <attack physical="51" magical="35" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="92" magical="67" />
      <speed><walk ground="80" /><run ground="120" /></speed>
    </stats>
    <collision><radius normal="9"/><height normal="16"/></collision>
  </npc>
</list>"#
    )
}

/// Gludio's siege zone around (X, Y) and its castle zone.
fn zones_xml() -> String {
    let cuboid = |x: i32, y: i32, w: i32| {
        format!(
            r#"shape="Cuboid" minZ="-5000" maxZ="5000"><node X="{}" Y="{}" /><node X="{}" Y="{}" />"#,
            x - w,
            y - w,
            x + w,
            y + w
        )
    };
    format!(
        r#"<?xml version="1.0"?>
<list enabled="true">
  <zone name="gludio_castle_battlefield_territory" type="SiegeZone" {}<stat name="castleId" val="1" /></zone>
  <zone name="gludio_castle" type="CastleZone" {}<stat name="castleId" val="1" />
    <spawn X="{}" Y="{}" Z="{}" /><spawn X="{}" Y="{}" Z="{}" type="chaotic" /><spawn X="{}" Y="{}" Z="{}" type="other" /></zone>
</list>"#,
        cuboid(X, Y, 3000),
        cuboid(X + 2000, Y, 300),
        X + 2000,
        Y,
        Z,
        X + 2100,
        Y,
        Z,
        X + 2200,
        Y,
        Z,
    )
}

/// A door of `Doors.xml` in the shape of a castle gate (weak defences, so a level 40 fist hurts it): 20
/// wide in x, 100 in y, around (x, y).
fn door_xml(id: i32, name: &str, x: i32, y: i32, extra: &str) -> String {
    format!(
        r#"<door id="{id}" name="{name}" height="150" baseHpMax="1000" basePDef="20" baseMDef="20" pos="{x},{y},{Z}" {extra}
        node1="{},{}" node2="{},{}" node3="{},{}" node4="{},{}" nodeZ="{}" />"#,
        x - 10,
        y - 50,
        x + 10,
        y - 50,
        x + 10,
        y + 50,
        x - 10,
        y + 50,
        Z - 10
    )
}

const SCHEDULE: &str = r#"<list>
  <schedule castleId="1" castleName="Gludio" siegeEnabled="true" day="SUNDAY" hour="16" maxConcurrent="5" />
</list>"#;

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

impl Scene {
    /// Gludio owned by the Lions, the Wolves registered as attackers, the guards of `guards`.
    fn new(guards: Vec<SiegeGuardRecord>) -> Self {
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
        write("data/zones/castle.xml", &zones_xml());
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Recorder(log.clone())));
        world.set_static_data(Box::new(Data {
            stats: stat_data(),
            rights: rights(),
        }));
        world.set_exp_data(
            Arc::new(ExperienceTable::from_xml(r#"<table maxLevel="85" maxPetLevel="85"></table>"#).unwrap()),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
        );
        let ids = Arc::new(IdFactory::new(vec![P1, P3, P4, P5, P9]).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        let npcs = Arc::new(NpcTemplates::from_xml(&npc_xml(), &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
        let doors = DoorTemplates::from_xml(&format!(
            "<list>{}{}</list>",
            door_xml(GATE, "gludio_castle_gate", X + 500, Y, ""),
            door_xml(WALL, "gludio_castle_wall_001", X + 500, Y + 600, r#"is_wall="true""#),
        ))
        .unwrap();
        world.spawn_doors(&doors, &ids).unwrap();
        world.set_residence_settings(ResidenceSettings::default());
        world.set_siege_schedule(Arc::new(SiegeSchedule::from_xml(SCHEDULE).unwrap()));
        world.load_clans(
            vec![
                clan(LIONS, "Lions", 1, P1),
                clan(WOLVES, "Wolves", 0, P3),
                clan(BEARS, "Bears", 0, P5),
            ],
            Vec::new(),
        );
        world.load_residences(ResidenceLoad {
            castles: vec![CastleRecord {
                id: 1,
                found: true,
                name: "Gludio".to_owned(),
                siege_date: NEXT_SIEGE,
                time_registration_over: true,
                owner_id: LIONS,
                ..CastleRecord::default()
            }],
            ..ResidenceLoad::default()
        });
        world.set_siege_settings(SiegeSettings {
            siege_hour_list: vec![16, 20],
            clock_override_unix_ms: Some(NOW),
            zone_offset_override_ms: Some(0),
            ..SiegeSettings::default()
        });
        world.load_sieges(SiegeLoad {
            siege_clans: vec![SiegeClanRecord {
                castle_id: 1,
                clan_id: WOLVES,
                kind: 1,
            }],
            siege_guards: guards,
            ..SiegeLoad::default()
        });
        log.take();
        let mut scene = Self {
            world,
            log,
            _root: root,
        };
        scene.enter(pdata(P1, LIONS, X + 400, Y));
        scene.enter(pdata(P3, WOLVES, X + 470, Y));
        scene.enter(pdata(P4, WOLVES, X + 470, Y + 600));
        scene.enter(pdata(P5, BEARS, OUTSIDE, Y));
        let mut gm = pdata(P9, 0, OUTSIDE, Y);
        gm.access_level = 100;
        gm.profile.clan = None;
        scene.enter(gm);
        scene.admin("admin_startsiege Gludio");
        scene.take();
        scene
    }

    fn enter(&mut self, data: PlayerData) {
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

    /// Two clicks on an object: the target, then the attack (`Door.onAction`).
    fn attack(&mut self, id: i32, object: ObjectId) {
        for _ in 0..2 {
            self.input(
                id,
                ClientInput::Action {
                    object_id: object.0,
                    action_id: 0,
                },
            );
        }
    }

    fn admin(&mut self, command: &str) {
        self.world.admin_command(ObjectId(P9), command, false);
    }

    fn take(&mut self) -> Vec<Out> {
        self.log.take()
    }

    fn run_ms(&mut self, ms: u64) {
        for _ in 0..ms.div_ceil(50) {
            self.world.tick();
        }
    }

    fn door(&self, door_id: i32) -> ObjectId {
        self.world.door_by_door_id(door_id).unwrap().object_id
    }

    fn hp(&self, door: ObjectId) -> f64 {
        self.world.door(door).unwrap().current_hp()
    }

    fn guard(&self) -> ObjectId {
        let ids: Vec<ObjectId> = self
            .world
            .npc_ids()
            .into_iter()
            .filter(|id| self.world.npc(*id).is_some_and(|n| n.npc_id == GUARD))
            .collect();
        assert_eq!(ids.len(), 1);
        ids[0]
    }
}

fn clan(clan_id: i32, name: &str, castle_id: i32, leader: i32) -> ClanRecord {
    ClanRecord {
        clan_id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level: 5,
            castle_id,
            leader_id: leader,
            ..ClanDataRecord::default()
        }),
        members: vec![ClanMemberRecord {
            object_id: leader,
            name: format!("P{}", leader - 0x6000_0000),
            level: 40,
            power_grade: 1,
            ..ClanMemberRecord::default()
        }],
        ..ClanRecord::default()
    }
}

fn pdata(id: i32, clan_id: i32, x: i32, y: i32) -> PlayerData {
    let mut data = PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{}", id - 0x6000_0000),
        title: String::new(),
        class_id: 0,
        base_class: 0,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        // The class template has the hit points of level 1 only.
        level: 1,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x,
        y,
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
    };
    data.profile.clan = Some(PlayerClanState {
        clan_id,
        ..PlayerClanState::default()
    });
    data
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn packets_to(out: &[Out], to: i32, name: &str) -> Vec<Vec<u8>> {
    out.iter()
        .filter_map(|o| match o {
            Out::Packet(t, b) if *t == to && name_of(b) == name => Some(b.clone()),
            _ => None,
        })
        .collect()
}

fn message_ids(out: &[Out], to: i32) -> Vec<SystemMessageId> {
    packets_to(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|b| decode::<SystemMessage>(b).id)
        .collect()
}

fn door_updates(out: &[Out], to: i32, door: ObjectId) -> Vec<DoorStatusUpdate> {
    packets_to(out, to, "DOOR_STATUS_UPDATE")
        .iter()
        .map(|b| decode::<DoorStatusUpdate>(b))
        .filter(|u| u.object_id == door.0)
        .collect()
}

fn residence_writes(out: &[Out]) -> usize {
    out.iter()
        .filter(|o| matches!(o, Out::Persist(PersistCommand::Residence(_))))
        .count()
}

// ---------------------------------------------------------------------------------- doors

/// The attackers of a GM test siege break the gate: `Door` is a creature, so the auto attack
/// hurts it (`YOU_HIT_FOR_S1_DAMAGE` to the attacker, `DoorStatusUpdate` and `StaticObjectInfo`
/// to everybody who sees it, sent targetable because it is a castle door); the last hit kills it
/// (`Creature.doDie`): "The castle gate has been destroyed." to everybody who sees it, no `Die`
/// (`DoorAI.notifyActionDeath` does nothing), and the dead gate stops blocking. The siege's end
/// (`spawnDoor`) brings it back with full HP (`doRevive`: `Revive`). D-51: nothing is written.
#[test]
fn attackers_break_the_gate_and_the_siege_end_repairs_it() {
    let mut s = Scene::new(Vec::new());
    let gate = s.door(GATE);
    // The players around saw the door: its `DoorAI` exists.
    assert!(s.world.has_ai(gate));
    assert!(s.world.door(gate).unwrap().is_enemy());
    // Attackable by the attacker, not by the owner (`Door.isAutoAttackable`).
    assert!(s.world.is_auto_attackable_by(gate, ObjectId(P3)));
    assert!(!s.world.is_auto_attackable_by(gate, ObjectId(P1)));
    // The gate's stats: `DoorStat` over the door template.
    let view = s.world.stats(gate).unwrap();
    assert_eq!(view.max_hp(), 1000);
    assert_eq!(view.level(), 0);
    let blocking_before = s.world.door_snapshot().door_count();
    assert_eq!(blocking_before, 2);

    s.attack(P3, gate);
    s.run_ms(5000);
    let out = s.take();
    let hp = s.hp(gate);
    assert!(hp < 1000.0, "the auto attack hurts the gate: {hp}");
    assert!(message_ids(&out, P3).contains(&SystemMessageId::YouHitForS1Damage));
    let updates = door_updates(&out, P1, gate);
    let last = updates.last().expect("DoorStatusUpdate to the viewers");
    assert_eq!((last.current_hp, last.max_hp), (hp as i32, 1000));
    assert!(last.is_enemy && last.closed);
    // `broadcastStatusUpdate`: a castle door goes out targetable to everybody.
    let info: Vec<StaticObjectInfo> = packets_to(&out, P1, "STATIC_OBJECT")
        .iter()
        .map(|b| decode::<StaticObjectInfo>(b))
        .filter(|i| i.object_id == gate.0)
        .collect();
    assert!(info.last().is_some_and(|i| i.targetable));

    // The last hit.
    s.world.reduce_current_hp(gate, 5000.0, Some(ObjectId(P3)));
    let out = s.take();
    assert!(s.world.is_dead(gate));
    assert!(s.world.door(gate).unwrap().dead);
    assert_eq!(s.hp(gate), 0.0);
    for viewer in [P1, P3] {
        assert!(
            message_ids(&out, viewer).contains(&SystemMessageId::TheCastleGateHasBeenDestroyed),
            "{viewer:x}"
        );
    }
    // Far away, nobody sees it.
    assert!(!message_ids(&out, P5).contains(&SystemMessageId::TheCastleGateHasBeenDestroyed));
    let last = *door_updates(&out, P1, gate).last().unwrap();
    assert_eq!((last.current_hp, last.damage), (0, 6));
    assert!(packets_to(&out, P1, "DIE").is_empty(), "a door gets no Die");
    // A dead door no longer blocks.
    assert_eq!(s.world.door_snapshot().door_count(), blocking_before - 1);
    // A dead door is no target any more.
    s.run_ms(2000);
    assert_eq!(s.hp(gate), 0.0);

    // `endSiege` -> `spawnDoor()`.
    s.admin("admin_endsiege Gludio");
    let out = s.take();
    assert!(!s.world.is_dead(gate));
    assert_eq!(s.hp(gate), 1000.0);
    let revives: Vec<Revive> = packets_to(&out, P1, "REVIVE")
        .iter()
        .map(|b| decode::<Revive>(b))
        .collect();
    assert!(revives.iter().any(|r| r.object_id == gate.0));
    // NOTE(java-quirk): `endSiege` repairs the doors before the zone goes off, so the last update
    // still says "enemy"; nothing sends the door again when the zone is off.
    let last = *door_updates(&out, P1, gate).last().unwrap();
    assert_eq!((last.current_hp, last.damage, last.is_enemy), (1000, 0, true));
    assert!(!s.world.door(gate).unwrap().is_enemy());
    assert_eq!(s.world.door_snapshot().door_count(), blocking_before);
    assert_eq!(residence_writes(&out), 0, "D-51");
}

/// `Door.reduceCurrentHp`: a wall outside an instance takes damage only from a servitor of race
/// `SIEGE_WEAPON`; a player, another servitor or a hit without an attacker leave it whole. A gate
/// takes anybody's damage.
#[test]
fn a_wall_takes_only_siege_weapon_damage() {
    let mut s = Scene::new(Vec::new());
    let wall = s.door(WALL);
    let gate = s.door(GATE);
    assert!(s.world.door(wall).unwrap().is_wall());
    s.world.reduce_current_hp(wall, 300.0, Some(ObjectId(P4)));
    s.world.reduce_current_hp(wall, 300.0, None);
    assert_eq!(s.hp(wall), 1000.0);
    // A cat is a servitor, but no siege weapon.
    s.world
        .apply_effects(ObjectId(P3), ObjectId(P3), SkillKey::new(SUMMON_CAT, 1));
    let cat = s.world.summon_of(ObjectId(P3)).expect("a cat");
    assert!(s.world.is_servitor(cat));
    s.world.reduce_current_hp(wall, 300.0, Some(cat));
    assert_eq!(s.hp(wall), 1000.0);
    // The golem is.
    s.world
        .apply_effects(ObjectId(P4), ObjectId(P4), SkillKey::new(SUMMON_GOLEM, 1));
    let golem = s.world.summon_of(ObjectId(P4)).expect("a golem");
    s.world.reduce_current_hp(wall, 300.0, Some(golem));
    assert_eq!(s.hp(wall), 700.0);
    // A gate is no wall.
    s.world.reduce_current_hp(gate, 100.0, Some(ObjectId(P3)));
    assert_eq!(s.hp(gate), 900.0);
}

// ---------------------------------------------------------------------------------- guards

fn guard_row() -> SiegeGuardRecord {
    SiegeGuardRecord {
        castle_id: 1,
        npc_id: GUARD,
        x: GUARD_X,
        y: GUARD_Y,
        z: Z,
        heading: 0,
        respawn_delay: 0,
        is_hired: true,
    }
}

/// `SiegeGuardAI` (a castle within 10000: `Defender.initAI`): once it thinks, it puts every
/// enemy of the siege within its attack range on its aggro list (`autoAttackCondition`:
/// `Defender.isAutoAttackable` says the attacker P3 is one, the defender P1 is not) after the
/// 10 seconds of `_globalAggro = -10`, then attacks the attacker and leaves the defender alone.
#[test]
fn guards_attack_attackers_and_ignore_defenders() {
    let mut s = Scene::new(vec![guard_row()]);
    let guard = s.guard();
    // The players saw it come: `getAI()` made its `SiegeGuardAI`.
    let state = s.world.siege_guard_state(guard).expect("a siege guard AI");
    assert_eq!(state.kind, SiegeGuardAiKind::Castle);
    assert_eq!(state.attack_range, 500);
    assert!(state.analysis.is_archer);
    // `spawnMe`: `World.addVisibleObject` makes the AI for the players who see the guard (the
    // constructor's `_globalAggro = -10`), then `Attackable.onSpawn` sets it active, and the
    // first `thinkActive` counts once.
    assert_eq!(s.world.intention_of(guard), Intention::Active);
    assert_eq!(s.world.ai_state(guard).unwrap().global_aggro, -9);
    s.run_ms(8000);
    assert!(s.world.aggro_list(guard).is_empty(), "no aggro during the first 10 s");
    // Then the attacker gets the hate and the attack (a level 70 guard kills him quickly).
    let mut attacked = Vec::new();
    for _ in 0..(5000 / 50) {
        s.world.tick();
        if let Some(target) = s.world.attack_target_of(guard)
            && !attacked.contains(&target)
        {
            attacked.push(target);
        }
    }
    assert_eq!(attacked, [ObjectId(P3)]);
    assert!(!s.world.aggro_contains(guard, ObjectId(P1)));
    let p3 = s.world.player(ObjectId(P3)).map_or(0.0, |p| p.core.cur_hp);
    assert!(p3 < 100.0 || s.world.is_dead(ObjectId(P3)), "the attacker is hit");
    s.run_ms(5000);
    let p1 = s.world.player(ObjectId(P1)).unwrap().core.cur_hp;
    assert_eq!(p1, 100.0, "the defender is left alone");
    assert!(!s.world.aggro_contains(guard, ObjectId(P1)));
}

/// `DoorAI.notifyActionAttacked`: an attacker who hits the gate calls the defenders within their
/// clan help range (`notifyActionAggression(attacker, 15)`), which run to attack him at once
/// (`SiegeGuardAI.notifyActionAggression`), long before their own aggro would start.
#[test]
fn a_hit_gate_calls_the_guards() {
    let mut s = Scene::new(vec![guard_row()]);
    let guard = s.guard();
    let gate = s.door(GATE);
    assert_eq!(s.world.intention_of(guard), Intention::Active);
    s.attack(P3, gate);
    // The first hit on the gate calls the guard, long before its own aggro (10 s) would start.
    let mut called = None;
    for _ in 0..(3000 / 50) {
        s.world.tick();
        if called.is_none() && s.world.attack_target_of(guard) == Some(ObjectId(P3)) {
            let hate = s
                .world
                .aggro_list(guard)
                .iter()
                .find(|a| a.attacker == ObjectId(P3))
                .map_or(0, |a| a.hate);
            called = Some((s.hp(gate), s.world.intention_of(guard), hate));
        }
    }
    let (gate_hp, intention, hate) = called.expect("the guard came");
    assert!(gate_hp < 1000.0, "called by the hit");
    assert_eq!(intention, Intention::Attack);
    assert!(hate >= 15, "hate {hate}");
    assert!(!s.world.aggro_contains(guard, ObjectId(P1)));
}
