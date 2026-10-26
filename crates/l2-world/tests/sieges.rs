#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Castle sieges (M7-SIEGE-01): the schedule, the registrations and their re-read (D-50), a GM
//! test siege end to end (D-51), the owner change in the middle, the headquarters, the siege
//! kills, the siege zone, the restart points, the mercenary tickets, the guards, the door
//! upgrades and the `SiegeDatabaseWrites` switch.
//! Java: mechanics/siege/{Siege,Castle}.java, managers/{SiegeManager,SiegeGuardManager,
//! MercTicketManager}.java, entity/actor/instance/{SiegeFlag,ControlTower,FlameTower,Artefact,
//! Defender}.java, entity/zone/type/SiegeZone.java, network/clientpackets/{RequestJoinSiege,
//! RequestConfirmSiegeWaitingList,RequestSetCastleSiegeTime,RequestRestartPoint}.java,
//! network/serverpackets/{SiegeInfo,SiegeDefenderList,Die}.java,
//! handlers/items/MercTicket.java, handlers/chat/commands/admin/AdminSiege.java. The expected
//! values come from the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, DoorTemplates, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache,
    ItemTemplates, KarmaLoss, MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SiegeSchedule,
    SkillLoadOptions, SkillTemplates, SkillTrees, SpawnData, SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::siege::{DefenderState, SiegeDefenderList, SiegeInfo, SiegeTime};
use l2_protocol_game::server::{NpcHtmlMessage, RelationChanged, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::residence::{CastleRecord, ResidenceReadKind, ResidenceReadThen, ResidenceRereadRows};
use l2_world::siege::{DoorUpgradeRecord, SiegeClanType, SiegeTeleportWhoType};
use l2_world::zones::ZoneId;
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

/// The siege skills (`SkillData.getSiegeSkills`): Seal of Ruler, Build Headquarters, the advanced
/// one and the two of a castle owner.
const SKILLS: &str = r#"<list>
<skill id="246" levels="1" name="Seal of Ruler"><operateType>A1</operateType></skill>
<skill id="247" levels="1" name="Build Headquarters"><operateType>A1</operateType></skill>
<skill id="326" levels="1" name="Build Advanced Headquarters"><operateType>A1</operateType></skill>
<skill id="844" levels="1" name="Outpost Construction"><operateType>A1</operateType></skill>
<skill id="845" levels="1" name="Outpost Demolition"><operateType>A1</operateType></skill>
</list>"#;

const ADENA: i32 = 57;
/// Mercenary Posting Ticket (Sword/Stationary), Gludio.
const TICKET: i32 = 3960;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="3960" type="EtcItem" name="Mercenary Posting Ticket (Sword/Stationary)">
  <set name="etcitem_type" val="CASTLE_GUARD" /><set name="immediate_effect" val="true" />
  <set name="is_stackable" val="true" /><set name="handler" val="MercTicket" /></item>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
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

/// The admin levels (100 is a GM) and the siege words of `AdminCommands.xml`.
fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (100, 0)] {
        rights.add_level(level, child);
    }
    for command in [
        "siege",
        "add_attacker",
        "add_defender",
        "add_guard",
        "list_siege_clans",
        "clear_siege_list",
        "move_defenders",
        "spawn_doors",
        "endsiege",
        "startsiege",
        "setsiegetime",
        "setcastle",
        "removecastle",
    ] {
        rights.add_command(command, 100, false);
    }
    rights
}

struct Data {
    stats: StatData,
    rights: AdminRights,
}

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.stats.items.get(item_id)?;
        Some(ItemStatic {
            display_id: item_id,
            type2: i16::try_from(t.type2).unwrap(),
            body_part: t.body_part.mask(),
            quest_item: false,
            weight: t.weight,
            shared_reuse_group: -1,
            weapon: false,
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
    fn item_stackable(&self, item_id: i32) -> bool {
        item_id == ADENA || item_id == TICKET
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
}

const X: i32 = -18_000;
const Y: i32 = 110_000;
const Z: i32 = -2_500;
/// Lions: P1 (leader) and P2; Wolves: P3 (leader) and P4; Bears (level 4): P5; the GM: P9.
const P1: i32 = 0x6000_0001;
const P2: i32 = 0x6000_0002;
const P3: i32 = 0x6000_0003;
const P4: i32 = 0x6000_0004;
const P5: i32 = 0x6000_0005;
const P9: i32 = 0x6000_0009;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
const BEARS: i32 = 700;
const OUTSIDE: i32 = X + 20_000;

/// 2026-10-21 (a Wednesday) 12:34:56.789 UTC, the siege clock at tick 0.
const NOW: i64 = 1_792_586_096_789;
/// 2026-11-01 (a Sunday) 16:00:00.789: the next siege of the schedule (`SUNDAY`, 16, two weeks
/// after the Sunday of this week, which has passed).
const NEXT_SIEGE: i64 = 1_793_548_800_789;

const CONTROL_TOWER: i32 = 13_002;
const FLAME_TOWER: i32 = 13_004;
const HQ: i32 = 35_062;
const ARTEFACT: i32 = 35_063;
const GUARD: i32 = 35_085;
const MERC: i32 = 35_010;
const DOOR: i32 = 19_210_001;

fn npc(id: i32, npc_type: &str, name: &str) -> String {
    format!(
        r#"<npc id="{id}" type="{npc_type}" level="70" name="{name}">
    <race>CONSTRUCT</race>
    <stats str="40" int="21" dex="30" wit="20" con="43" men="20">
      <vitals hp="3000" hpRegen="2" mp="40.0" mpRegen="0.9" />
      <attack physical="8" magical="5" random="30" critical="4" accuracy="4.75" attackSpeed="253" type="SWORD" range="40" distance="80" width="120" />
      <defence physical="44.44444" magical="29.5" />
      <speed><walk ground="40" /><run ground="100" /></speed>
    </stats>
    <ai aggroRange="0" isAggressive="false" />
    <collision><radius normal="10"/><height normal="20"/></collision>
  </npc>"#
    )
}

fn npc_xml() -> String {
    let list = [
        npc(CONTROL_TOWER, "ControlTower", "Life Control Tower"),
        npc(FLAME_TOWER, "FlameTower", "Flame Control Tower"),
        npc(HQ, "Folk", "Headquarters"),
        npc(ARTEFACT, "Artefact", "Holy Artifact"),
        npc(GUARD, "Defender", "Guard"),
        npc(MERC, "Defender", "Mercenary"),
    ]
    .join("\n");
    format!(r#"<?xml version="1.0"?><list>{list}</list>"#)
}

/// Gludio's siege zone around (X, Y) with its castle zone (restart points), the headquarters zone
/// and the teleport zone; Dion's siege zone far away.
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
  <zone name="gludio_castle_headquarter_territory1" type="HqZone" {}<stat name="castleId" val="1" /></zone>
  <zone name="dion_castle_battlefield_territory" type="SiegeZone" {}<stat name="castleId" val="2" /></zone>
  <zone name="gludio_slow_1" id="70001" type="SwampZone" {}<stat name="default_enabled" val="false" />
    <stat name="move_bonus" val="0.2" /><stat name="castleId" val="1" /><stat name="eventId" val="2418002" /></zone>
  <zone name="gludio_damage_1" id="70002" type="DamageZone" {}<stat name="default_enabled" val="false" />
    <stat name="dmgHPSec" val="10" /><stat name="castleId" val="1" /></zone>
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
        cuboid(X - 1000, Y, 500),
        cuboid(X + 60_000, Y, 2000),
        cuboid(X - 2200, Y, 300),
        cuboid(X - 2800, Y, 150),
    )
}

const SCHEDULE: &str = r#"<list>
  <schedule castleId="1" castleName="Gludio" siegeEnabled="true" day="SUNDAY" hour="16" maxConcurrent="5" />
</list>"#;

const CASTLES_HTM: &str = "<html><body>%castles%|%siegableHalls%|%clanhalls%|%freeclanhalls%</body></html>";
const CASTLE_HTM: &str = "<html><body>Castle: %castleName%</body></html>";
const TIME_HTM: &str =
    "<html><body>%castleName% %time% %sundaylink% %sunday% %saturdaylink% %saturday%</body></html>";

struct Scene {
    world: World,
    log: Log,
    /// What `load_sieges` sent (the startup's writes).
    startup: Vec<Out>,
    _root: tempfile::TempDir,
}

impl Scene {
    /// Gludio owned by `owner` (0 for none), its next siege at `siege_date`, the switch as given.
    fn new(siege_database_writes: bool, owner: i32, siege_date: i64) -> Self {
        Self::with(
            siege_database_writes,
            owner,
            siege_date,
            SiegeLoad::default(),
            |_| {},
        )
    }

    fn with(
        siege_database_writes: bool,
        owner: i32,
        siege_date: i64,
        load: SiegeLoad,
        settings: impl FnOnce(&mut SiegeSettings),
    ) -> Self {
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
        write("data/html/admin/castles.htm", CASTLES_HTM);
        write("data/html/admin/castle.htm", CASTLE_HTM);
        write("data/html/admin/castlesiegetime.htm", TIME_HTM);
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
        // The characters' ids are taken (they are in the factory's range).
        let ids = Arc::new(IdFactory::new(vec![P1, P2, P3, P4, P5, P9, 0x6000_2000]).unwrap());
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
            r#"<list><door id="{DOOR}" name="gludio_castle_gate" height="150" baseHpMax="1000" pos="{},{},{Z}"
            node1="{},{}" node2="{},{}" node3="{},{}" node4="{},{}" nodeZ="{}" /></list>"#,
            X + 500,
            Y,
            X + 490,
            Y - 50,
            X + 510,
            Y - 50,
            X + 510,
            Y + 50,
            X + 490,
            Y + 50,
            Z - 10
        ))
        .unwrap();
        world.spawn_doors(&doors, &ids).unwrap();
        world.set_residence_settings(ResidenceSettings {
            siege_database_writes,
            ..ResidenceSettings::default()
        });
        world.set_siege_schedule(Arc::new(SiegeSchedule::from_xml(SCHEDULE).unwrap()));
        world.load_clans(vec![lions(), wolves(), bears()], Vec::new());
        world.load_residences(ResidenceLoad {
            castles: vec![
                castle(1, "Gludio", owner, siege_date),
                castle(2, "Dion", 0, NEXT_SIEGE),
            ],
            ..ResidenceLoad::default()
        });
        let mut siege = SiegeSettings {
            siege_hour_list: vec![16, 20],
            clock_override_unix_ms: Some(NOW),
            zone_offset_override_ms: Some(0),
            ..SiegeSettings::default()
        };
        siege.castles.insert(
            1,
            CastleSiegeConfig {
                control_towers: vec![TowerSpawn {
                    npc_id: CONTROL_TOWER,
                    x: X + 100,
                    y: Y,
                    z: Z,
                    zone_list: vec![],
                    upgrade_level: 0,
                }],
                flame_towers: vec![TowerSpawn {
                    npc_id: FLAME_TOWER,
                    x: X + 200,
                    y: Y,
                    z: Z,
                    zone_list: vec![70001, 70002],
                    upgrade_level: 0,
                }],
                max_mercenaries: 100,
            },
        );
        settings(&mut siege);
        world.set_siege_settings(siege);
        log.take();
        world.load_sieges(load);
        let startup = log.take();
        Self {
            world,
            log,
            startup,
            _root: root,
        }
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

    /// `teleToLocation`, then the client's `Appearing`.
    fn teleport(&mut self, id: i32, x: i32, y: i32, z: i32) {
        self.world
            .tele_to_location(ObjectId(id), TeleportTarget::at(x, y, z));
        self.input(id, ClientInput::Appearing);
    }

    fn admin(&mut self, gm: i32, command: &str) {
        self.world.admin_command(ObjectId(gm), command, false);
    }

    fn take(&mut self) -> Vec<Out> {
        self.log.take()
    }

    fn run_ms(&mut self, ms: u64) {
        for _ in 0..ms.div_ceil(50) {
            self.world.tick();
        }
    }

    fn siege(&self) -> &Siege {
        self.world.castle_siege(CastleId(1)).unwrap()
    }

    /// Everybody of the scene online: the Lions and the Wolves in the siege zone, the Bears
    /// outside, the GM in the zone.
    fn enter_all(&mut self) {
        self.enter(pdata(P1, LIONS, X));
        self.enter(pdata(P2, LIONS, X + 10));
        self.enter(pdata(P3, WOLVES, X + 20));
        self.enter(pdata(P4, WOLVES, X + 30));
        self.enter(pdata(P5, BEARS, OUTSIDE));
        let mut gm = pdata(P9, 0, X + 40);
        gm.access_level = 100;
        gm.profile.clan = None;
        self.enter(gm);
        self.take();
    }

    /// Answers the last registration re-read the world asked for (D-50).
    fn answer_reread(&mut self, out: &[Out], rows: Vec<SiegeClanRecord>) {
        let then = reread_requests(out).pop().expect("a re-read request").1;
        self.world.handle(WorldCommand::ResidenceReread {
            result: Ok(ResidenceRereadRows::SiegeClans { castle: None, rows }),
            then,
        });
    }
}

fn member(id: i32, power_grade: i32) -> ClanMemberRecord {
    ClanMemberRecord {
        object_id: id,
        name: format!("P{}", id - 0x6000_0000),
        level: 40,
        power_grade,
        ..ClanMemberRecord::default()
    }
}

fn clan(clan_id: i32, name: &str, level: i32, castle_id: i32, leader: i32, members: &[i32]) -> ClanRecord {
    ClanRecord {
        clan_id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level,
            castle_id,
            leader_id: leader,
            ..ClanDataRecord::default()
        }),
        members: members
            .iter()
            .map(|m| member(*m, if *m == leader { 1 } else { 5 }))
            .collect(),
        ..ClanRecord::default()
    }
}

fn lions() -> ClanRecord {
    clan(LIONS, "Lions", 5, 1, P1, &[P1, P2])
}

fn wolves() -> ClanRecord {
    clan(WOLVES, "Wolves", 5, 0, P3, &[P3, P4])
}

fn bears() -> ClanRecord {
    clan(BEARS, "Bears", 4, 0, P5, &[P5])
}

fn castle(id: i32, name: &str, owner_id: i32, siege_date: i64) -> CastleRecord {
    CastleRecord {
        id,
        found: true,
        name: name.to_owned(),
        siege_date,
        time_registration_over: true,
        owner_id,
        ..CastleRecord::default()
    }
}

fn pdata(id: i32, clan_id: i32, x: i32) -> PlayerData {
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
        level: 40,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x,
        y: Y,
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

fn messages_to(out: &[Out], to: i32) -> Vec<SystemMessage> {
    packets_to(out, to, "SYSTEM_MESSAGE")
        .iter()
        .map(|b| decode::<SystemMessage>(b))
        .collect()
}

fn message_ids(out: &[Out], to: i32) -> Vec<SystemMessageId> {
    messages_to(out, to).into_iter().map(|m| m.id).collect()
}

fn residence_writes(out: &[Out]) -> Vec<ResidenceWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Residence(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn reread_requests(out: &[Out]) -> Vec<(i32, ResidenceReadThen)> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ResidenceRereadRequest {
                residence_id,
                kind: ResidenceReadKind::SiegeClans,
                then,
            }) => Some((*residence_id, then.clone())),
            _ => None,
        })
        .collect()
}

fn npcs_of(world: &World, npc_id: i32) -> Vec<ObjectId> {
    let mut ids: Vec<ObjectId> = world
        .npc_ids()
        .into_iter()
        .filter(|id| world.npc(*id).is_some_and(|n| n.npc_id == npc_id))
        .collect();
    ids.sort_unstable();
    ids
}

fn wolves_row() -> SiegeClanRecord {
    SiegeClanRecord {
        castle_id: 1,
        clan_id: WOLVES,
        kind: 1,
    }
}

// ---------------------------------------------------------------------------------- schedule

/// `Siege.startAutoTask` -> `correctSiegeDateTime`: a siege date in the past moves to the Sunday
/// of the schedule (16:00, minute and second 0, the milliseconds kept) two weeks on, the
/// registration is open, "Gludio has announced the next castle siege time." goes out, and
/// `saveSiegeDate` writes the dates only with `SiegeDatabaseWrites` (D-51).
#[test]
fn a_past_siege_date_moves_to_the_next_sunday_and_is_saved_only_with_the_switch() {
    for writes in [true, false] {
        let s = Scene::new(writes, LIONS, 0);
        let gludio = s.world.castle(CastleId(1)).unwrap();
        assert_eq!(gludio.siege_date, NEXT_SIEGE);
        let saved = residence_writes(&s.startup).contains(&ResidenceWrite::CastleSiegeDate {
            castle_id: 1,
            siege_date: NEXT_SIEGE,
            reg_time_end: 0,
            reg_time_over: true,
        });
        assert_eq!(saved, writes, "D-51");
        assert!(!s.siege().registration_over);
        // Dion has no schedule: its date stays.
        assert_eq!(s.world.castle(CastleId(2)).unwrap().siege_date, NEXT_SIEGE);
    }
    let mut s = Scene::new(true, LIONS, 0);
    // A new date by the GM is saved again (`saveSiegeDate`): with the switch it is written.
    s.enter_all();
    s.admin(P9, "admin_setsiegetime Gludio hour 20");
    let out = s.take();
    assert_eq!(
        s.world.castle(CastleId(1)).unwrap().siege_date,
        NEXT_SIEGE + 4 * 3_600_000
    );
    assert!(residence_writes(&out).contains(&ResidenceWrite::CastleSiegeDate {
        castle_id: 1,
        siege_date: NEXT_SIEGE + 4 * 3_600_000,
        reg_time_end: 0,
        reg_time_over: true,
    }));
    let told = messages_to(&out, P9);
    assert_eq!(
        told[0].params,
        [SystemMessageParam::Text(
            "Castle siege time for castle Gludio has been changed.".to_owned()
        )]
    );
    let page = packets_to(&out, P9, "NPC_HTML_MESSAGE");
    let html = decode::<NpcHtmlMessage>(page.last().unwrap()).html;
    // `Date.toString()` of the new date; today is Wednesday, so Saturday is day 297 (10/24, the
    // month counted from 0) and Sunday day 298.
    assert_eq!(
        html,
        "<html><body>Gludio Sun Nov 01 20:00:00 GMT+00:00 2026 298 9/25 297 9/24</body></html>"
    );

    let mut off = Scene::new(false, LIONS, 0);
    off.enter_all();
    off.admin(P9, "admin_setsiegetime Gludio hour 20");
    let out = off.take();
    assert!(
        !residence_writes(&out)
            .iter()
            .any(|w| matches!(w, ResidenceWrite::CastleSiegeDate { .. })),
        "D-51: no siege date row without the switch"
    );
}

/// `ScheduleStartSiegeTask`: the task walks down to the siege date and starts the siege; with
/// `SiegeDatabaseWrites` off the Java GS runs it, and this world reads the castle and the
/// registrations again once it is over (D-51).
#[test]
fn the_start_task_starts_the_siege_only_with_the_switch() {
    let load = SiegeLoad {
        siege_clans: vec![wolves_row()],
        ..SiegeLoad::default()
    };
    let mut on = Scene::with(true, LIONS, NOW + 5_000, load.clone(), |_| {});
    on.enter_all();
    assert_eq!(on.siege().attacker_clans().len(), 1);
    on.run_ms(6_100);
    let out = on.take();
    assert!(on.siege().in_progress);
    assert!(message_ids(&out, P5).contains(&SystemMessageId::TheS1SiegeHasStarted));

    let mut off = Scene::with(false, LIONS, NOW + 5_000, load, |s| s.siege_length = 1);
    off.enter_all();
    off.run_ms(6_100);
    let out = off.take();
    assert!(!off.siege().in_progress, "D-51: Sieghardt runs the real siege");
    assert!(!message_ids(&out, P5).contains(&SystemMessageId::TheS1SiegeHasStarted));
    assert!(reread_requests(&out).is_empty());
    // The siege length (1 minute) and ten more: the castle and its registrations again.
    off.run_ms(11 * 60_000);
    let out = off.take();
    let requests = reread_requests(&out);
    assert_eq!(requests.len(), 1);
    assert_eq!(
        requests[0],
        (1, ResidenceReadThen::SiegeRestartSchedule { castle_id: 1 })
    );
    // Sieghardt moved the date two weeks on and cleared the registrations.
    let then = requests[0].1.clone();
    off.world.handle(WorldCommand::ResidenceReread {
        result: Ok(ResidenceRereadRows::SiegeClans {
            castle: Some(castle(1, "Gludio", LIONS, NEXT_SIEGE)),
            rows: vec![],
        }),
        then,
    });
    assert_eq!(off.world.castle(CastleId(1)).unwrap().siege_date, NEXT_SIEGE);
    assert!(off.siege().attacker_clans().is_empty());
}

// ---------------------------------------------------------------------------------- registration

/// `RequestJoinSiege`: the registrations are read again first (D-50); the attacker is saved
/// (`INSERT INTO siege_clans`, only with the switch) and the player gets `SiegeInfo`. A failed
/// read refuses the request.
#[test]
fn joining_reads_the_registrations_again_and_saves_the_attacker() {
    let mut s = Scene::new(true, LIONS, NEXT_SIEGE);
    s.enter_all();
    s.input(
        P3,
        ClientInput::Siege(SiegeInput::JoinSiege {
            castle_id: 1,
            is_attacker: 1,
            is_joining: 1,
        }),
    );
    let out = s.take();
    assert!(s.siege().attacker_clans().is_empty(), "nothing before the read");
    s.answer_reread(&out, vec![]);
    let out = s.take();
    assert_eq!(
        s.siege()
            .attacker_clans()
            .iter()
            .map(|c| c.clan_id)
            .collect::<Vec<_>>(),
        [WOLVES]
    );
    assert_eq!(
        residence_writes(&out),
        vec![ResidenceWrite::SiegeClanInsert {
            clan_id: WOLVES,
            castle_id: 1,
            kind: 1
        }]
    );
    let info = decode::<SiegeInfo>(&packets_to(&out, P3, "CASTLE_SIEGE_INFO")[0]);
    assert_eq!((info.residence_id, info.owner_id), (1, LIONS));
    assert!(!info.viewer_is_owner_leader);
    assert_eq!(info.now_secs, i32::try_from(NOW / 1000).unwrap());
    assert_eq!(
        info.time,
        SiegeTime::Date(i32::try_from(NEXT_SIEGE / 1000).unwrap())
    );

    // A failed read changes nothing and refuses the follow-up.
    s.input(
        P3,
        ClientInput::Siege(SiegeInput::JoinSiege {
            castle_id: 1,
            is_attacker: 1,
            is_joining: 0,
        }),
    );
    let out = s.take();
    let then = reread_requests(&out).pop().unwrap().1;
    s.world.handle(WorldCommand::ResidenceReread {
        result: Err(()),
        then,
    });
    assert_eq!(s.siege().attacker_clans().len(), 1);
    assert!(s.take().is_empty());

    // Taking the registration back: `DELETE FROM siege_clans WHERE castle_id=? and clan_id=?`.
    s.input(
        P3,
        ClientInput::Siege(SiegeInput::JoinSiege {
            castle_id: 1,
            is_attacker: 1,
            is_joining: 0,
        }),
    );
    let out = s.take();
    s.answer_reread(&out, vec![wolves_row()]);
    let out = s.take();
    assert!(s.siege().attacker_clans().is_empty());
    assert_eq!(
        residence_writes(&out),
        vec![ResidenceWrite::SiegeClanDelete {
            castle_id: 1,
            clan_id: WOLVES
        }]
    );
}

/// `checkIfCanRegister` and `registerDefender`: the first rule that fails sends its message.
#[test]
fn the_registration_rules_send_javas_messages() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    let join = |s: &mut Scene, player: i32, attacker: i32| {
        s.input(
            player,
            ClientInput::Siege(SiegeInput::JoinSiege {
                castle_id: 1,
                is_attacker: attacker,
                is_joining: 1,
            }),
        );
        let out = s.take();
        s.answer_reread(&out, vec![]);
        s.take()
    };
    // Bears are level 4.
    let out = join(&mut s, P5, 1);
    assert_eq!(
        message_ids(&out, P5),
        [SystemMessageId::OnlyClansOfLevel5OrHigherMayRegisterForACastleSiege]
    );
    // The owner registers as a defender automatically.
    let out = join(&mut s, P1, 1);
    assert_eq!(
        message_ids(&out, P1),
        [SystemMessageId::CastleOwningClansAreAutomaticallyRegisteredOnTheDefendingSide]
    );
    // A second request.
    join(&mut s, P3, 1);
    let out = join(&mut s, P3, 1);
    assert_eq!(
        message_ids(&out, P3),
        [SystemMessageId::YouHaveAlreadyRequestedACastleSiege]
    );
    // A member without the clan privilege (`CASTLE_SIEGE`).
    s.input(
        P4,
        ClientInput::Siege(SiegeInput::JoinSiege {
            castle_id: 1,
            is_attacker: 1,
            is_joining: 1,
        }),
    );
    let out = s.take();
    assert_eq!(
        message_ids(&out, P4),
        [SystemMessageId::YouAreNotAuthorizedToDoThat]
    );
    assert!(reread_requests(&out).is_empty());

    // A castle without an owner has no defenders.
    let mut free = Scene::new(false, 0, NEXT_SIEGE);
    free.enter_all();
    let out = join(&mut free, P3, 0);
    let text = packets_to(&out, P3, "SYSTEM_MESSAGE");
    let message = decode::<SystemMessage>(&text[0]);
    assert_eq!(
        message.params,
        [SystemMessageParam::Text(
            "You cannot register as a defender because Gludio is owned by NPC.".to_owned()
        )]
    );
}

/// A defender waits for the lord (`DEFENDER_NOT_APPROVED`), the lord approves it
/// (`UPDATE siege_clans SET type = 0`), and `SiegeDefenderList` shows the owner (1) and the
/// approved defender (3), Java's mapping of `SiegeClanType`.
#[test]
fn the_lord_approves_a_waiting_defender() {
    let mut s = Scene::new(true, LIONS, NEXT_SIEGE);
    s.enter_all();
    s.input(
        P3,
        ClientInput::Siege(SiegeInput::JoinSiege {
            castle_id: 1,
            is_attacker: 0,
            is_joining: 1,
        }),
    );
    let out = s.take();
    s.answer_reread(&out, vec![]);
    let out = s.take();
    assert_eq!(
        residence_writes(&out),
        vec![ResidenceWrite::SiegeClanInsert {
            clan_id: WOLVES,
            castle_id: 1,
            kind: 2
        }]
    );
    assert_eq!(s.siege().defender_waiting_clans().len(), 1);
    // Somebody who is not the lord is ignored.
    s.input(
        P3,
        ClientInput::Siege(SiegeInput::ConfirmSiegeWaitingList {
            castle_id: 1,
            clan_id: WOLVES,
            approved: 1,
        }),
    );
    let out = s.take();
    s.answer_reread(
        &out,
        vec![SiegeClanRecord {
            castle_id: 1,
            clan_id: WOLVES,
            kind: 2,
        }],
    );
    assert!(s.take().is_empty());
    // The lord approves.
    s.input(
        P1,
        ClientInput::Siege(SiegeInput::ConfirmSiegeWaitingList {
            castle_id: 1,
            clan_id: WOLVES,
            approved: 1,
        }),
    );
    let out = s.take();
    s.answer_reread(
        &out,
        vec![SiegeClanRecord {
            castle_id: 1,
            clan_id: WOLVES,
            kind: 2,
        }],
    );
    let out = s.take();
    assert_eq!(
        residence_writes(&out),
        vec![ResidenceWrite::SiegeClanUpdate {
            castle_id: 1,
            clan_id: WOLVES,
            kind: 0
        }]
    );
    let list = decode::<SiegeDefenderList>(&packets_to(&out, P1, "CASTLE_SIEGE_DEFENDER_LIST")[0]);
    let states: Vec<(i32, DefenderState)> = list
        .clans
        .iter()
        .flatten()
        .map(|d| (d.clan.clan_id, d.state))
        .collect();
    assert_eq!(
        states,
        [(LIONS, DefenderState::Owner), (WOLVES, DefenderState::Defender)]
    );
    assert!(s.siege().defender_waiting_clans().is_empty());
}

/// D-51: registrations this world made without the switch stay over a re-read until the siege
/// (a GM test siege), while the rows of the Java GS replace the rest.
#[test]
fn unsaved_registrations_survive_a_reread() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(P9).unwrap()),
        input: ClientInput::Action {
            object_id: P3,
            action_id: 0,
        },
    });
    s.admin(P9, "admin_add_attacker Gludio");
    let out = s.take();
    assert_eq!(s.siege().attacker_clans().len(), 1);
    assert!(
        residence_writes(&out).is_empty(),
        "D-51: the registration row is not written"
    );
    s.input(P5, ClientInput::Siege(SiegeInput::AttackerList { castle_id: 1 }));
    let out = s.take();
    // The Java GS has Bears as attackers of Gludio.
    s.answer_reread(
        &out,
        vec![SiegeClanRecord {
            castle_id: 1,
            clan_id: BEARS,
            kind: 1,
        }],
    );
    let mut ids: Vec<i32> = s.siege().attacker_clans().iter().map(|c| c.clan_id).collect();
    ids.sort_unstable();
    assert_eq!(ids, [WOLVES, BEARS]);
}

// ---------------------------------------------------------------------------------- the siege

/// A GM test siege without `SiegeDatabaseWrites` (D-51): `//add_attacker`, `//startsiege`, the
/// participants' siege state and the siege zone, the towers, then `//endsiege` with the
/// defender's victory. No global row is written.
#[test]
fn a_gm_test_siege_runs_end_to_end_without_global_writes() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    s.input(
        P9,
        ClientInput::Action {
            object_id: P3,
            action_id: 0,
        },
    );
    s.admin(P9, "admin_add_attacker Gludio");
    s.admin(P9, "admin_startsiege Gludio");
    let out = s.take();
    assert!(s.siege().in_progress);
    assert_eq!(s.siege().first_owner_clan_id, LIONS);
    let started = messages_to(&out, P5)
        .into_iter()
        .find(|m| m.id == SystemMessageId::TheS1SiegeHasStarted)
        .unwrap();
    assert_eq!(started.params, [SystemMessageParam::CastleName(1)]);
    // The owner first in the defenders (`loadSiegeClan`), the Wolves attack.
    let defenders: Vec<(i32, SiegeClanType)> = s
        .siege()
        .defender_clans()
        .iter()
        .map(|c| (c.clan_id, c.kind))
        .collect();
    assert_eq!(defenders, [(LIONS, SiegeClanType::Owner)]);
    // `updatePlayerSiegeStateFlags`: attacker 1, defender 2, in the siege inside the zone.
    assert_eq!(s.world.siege_state_of(ObjectId(P3)), 1);
    assert_eq!(s.world.siege_state_of(ObjectId(P1)), 2);
    assert_eq!(s.world.siege_side_of(ObjectId(P3)), 1);
    assert!(s.world.is_in_siege(ObjectId(P3)));
    assert!(!s.world.is_in_siege(ObjectId(P5)));
    // `UserInfo`'s relation: an attacker 0x180, a defender leader 0x40 | 0x80.
    assert_eq!(s.world.user_info_relation(ObjectId(P3)), 0x40 | 0x180);
    assert_eq!(s.world.user_info_relation(ObjectId(P2)), 0x80);
    // `RelationChanged` of the attacker for the defender who sees him: in siege, enemy,
    // attacker.
    let relations: Vec<RelationChanged> = packets_to(&out, P1, "RELATION_CHANGED")
        .iter()
        .map(|b| decode::<RelationChanged>(b))
        .collect();
    let of_p3 = relations
        .iter()
        .flat_map(|r| r.relations.iter())
        .rfind(|e| e.object_id == P3)
        .unwrap();
    assert_eq!(of_p3.relation & 0x1E00, 0x200 | 0x1000 | 0x400);
    // The zone is active: everybody inside gets the combat zone message and its flags.
    assert!(message_ids(&out, P2).contains(&SystemMessageId::YouHaveEnteredACombatZone));
    assert!(s.world.is_inside_zone(ObjectId(P2), ZoneId::Siege));
    assert!(s.world.is_inside_zone(ObjectId(P2), ZoneId::Pvp));
    assert!(!s.world.is_inside_zone(ObjectId(P5), ZoneId::Siege));
    assert_eq!(s.world.siege_at(X, Y, Z), Some(CastleId(1)));
    // The towers.
    assert_eq!(npcs_of(&s.world, CONTROL_TOWER).len(), 1);
    assert_eq!(npcs_of(&s.world, FLAME_TOWER).len(), 1);
    assert_eq!(s.siege().control_tower_count, 1);
    // The castle gate is an enemy now.
    let gate = s.world.door_by_door_id(DOOR).unwrap();
    assert!(gate.is_enemy());
    // Only the attackers may hit a tower.
    let tower = npcs_of(&s.world, CONTROL_TOWER)[0];
    assert!(s.world.is_auto_attackable_by(tower, ObjectId(P3)));
    assert!(!s.world.is_auto_attackable_by(tower, ObjectId(P1)));
    assert!(
        residence_writes(&out).is_empty(),
        "D-51: {:?}",
        residence_writes(&out)
    );

    // The end: the defender holds the castle.
    s.admin(P9, "admin_endsiege Gludio");
    let out = s.take();
    assert!(!s.siege().in_progress);
    let ids = message_ids(&out, P5);
    assert!(ids.contains(&SystemMessageId::TheS1SiegeHasFinished));
    let victory = messages_to(&out, P5)
        .into_iter()
        .find(|m| m.id == SystemMessageId::ClanS1IsVictoriousOverS2SCastleSiege)
        .unwrap();
    assert_eq!(
        victory.params,
        [
            SystemMessageParam::Text("Lions".to_owned()),
            SystemMessageParam::Text("Gludio".to_owned())
        ]
    );
    // `increaseBloodAllianceCount`: `BloodAllianceReward` (1), in this world only.
    assert_eq!(s.world.clan(LIONS).unwrap().blood_alliance_count, 1);
    // NOTE(java-quirk): `_formerOwner` is only set by an owner change, so a castle that kept
    // its owner since the start gives `TakeCastlePoints` (1500), not `CastleDefendedPoints`.
    assert_eq!(s.world.clan(LIONS).unwrap().reputation_score, 1500);
    // Everybody leaves the siege; the towers go; the lists are empty; the zone is off.
    assert_eq!(s.world.siege_state_of(ObjectId(P3)), 0);
    assert!(!s.world.is_in_siege(ObjectId(P3)));
    assert!(npcs_of(&s.world, CONTROL_TOWER).is_empty());
    assert!(npcs_of(&s.world, FLAME_TOWER).is_empty());
    assert!(s.siege().attacker_clans().is_empty());
    assert!(message_ids(&out, P2).contains(&SystemMessageId::YouHaveLeftACombatZone));
    assert!(!s.world.door_by_door_id(DOOR).unwrap().is_enemy());
    // `saveCastleSiege`: the next siege two weeks on, the registration open for a day.
    let gludio = s.world.castle(CastleId(1)).unwrap();
    assert_eq!(gludio.siege_date, NEXT_SIEGE);
    assert!(!gludio.time_registration_over);
    assert_eq!(gludio.time_registration_over_date, NOW + 86_400_000);
    assert!(
        !residence_writes(&out)
            .iter()
            .any(|w| !matches!(w, ResidenceWrite::ClanBloodAllianceCount { .. })),
        "D-51: {:?}",
        residence_writes(&out)
    );
    assert!(residence_writes(&out).is_empty());
}

/// `startSiege` without attackers: called off ("canceled because there were no clans that
/// participated" for an owned castle, which gets the blood alliance reward).
#[test]
fn a_siege_without_attackers_is_called_off() {
    let mut s = Scene::new(true, LIONS, NEXT_SIEGE);
    s.enter_all();
    s.admin(P9, "admin_startsiege Gludio");
    let out = s.take();
    assert!(!s.siege().in_progress);
    assert!(
        message_ids(&out, P5)
            .contains(&SystemMessageId::S1SSiegeWasCanceledBecauseThereWereNoClansThatParticipated)
    );
    assert!(
        residence_writes(&out).contains(&ResidenceWrite::ClanBloodAllianceCount {
            clan_id: LIONS,
            count: 1
        })
    );
    let mut free = Scene::new(true, 0, NEXT_SIEGE);
    free.enter_all();
    free.admin(P9, "admin_startsiege Gludio");
    let out = free.take();
    assert!(message_ids(&out, P5).contains(&SystemMessageId::TheSiegeOfS1HasBeenCanceledDueToLackOfInterest));
}

/// `ScheduleEndSiegeTask`: the countdown to both sides ("10 minute(s) until castle siege
/// conclusion.", ..., "This castle siege will end in 10 second(s)!"), then the end.
#[test]
fn the_end_task_counts_down_to_both_sides() {
    let mut s = Scene::with(true, LIONS, NEXT_SIEGE, SiegeLoad::default(), |s| {
        s.siege_length = 11
    });
    s.enter_all();
    s.input(
        P9,
        ClientInput::Action {
            object_id: P3,
            action_id: 0,
        },
    );
    s.admin(P9, "admin_add_attacker Gludio");
    s.admin(P9, "admin_startsiege Gludio");
    s.take();
    s.run_ms(1_000);
    let out = s.take();
    let minutes = messages_to(&out, P3)
        .into_iter()
        .find(|m| m.id == SystemMessageId::S1MinuteSUntilCastleSiegeConclusion)
        .unwrap();
    // 11 minutes minus the second the first run waited.
    assert_eq!(minutes.params, [SystemMessageParam::IntNumber(10)]);
    assert!(messages_to(&out, P5).is_empty(), "only the two sides hear it");
    s.run_ms(10 * 60_000 + 50_000);
    let out = s.take();
    let seconds = messages_to(&out, P1)
        .into_iter()
        .find(|m| m.id == SystemMessageId::ThisCastleSiegeWillEndInS1SecondS)
        .unwrap();
    assert_eq!(seconds.params, [SystemMessageParam::IntNumber(10)]);
    s.run_ms(10_000);
    let out = s.take();
    assert!(!s.siege().in_progress);
    assert!(message_ids(&out, P5).contains(&SystemMessageId::TheS1SiegeHasFinished));
}

/// Seal of Ruler on the artefact (`TakeCastle` -> `Castle.engrave`): the Wolves own the castle,
/// "Clan Wolves has successfully engraved the holy artifact!", and `midVictory` turns the sides:
/// the new owner defends, the old defenders attack, the doors stand at half HP. At the end the
/// new owner takes the reputation of the old one.
#[test]
fn engraving_the_artefact_changes_the_owner_in_the_middle() {
    let mut s = Scene::new(true, LIONS, NEXT_SIEGE);
    s.enter_all();
    let artefact = s.world.spawn_npc_at(ARTEFACT, X + 300, Y, Z, 0).unwrap();
    s.input(
        P9,
        ClientInput::Action {
            object_id: P3,
            action_id: 0,
        },
    );
    s.admin(P9, "admin_add_attacker Gludio");
    s.admin(P9, "admin_startsiege Gludio");
    s.take();
    s.world.castle_engrave(ObjectId(P3), artefact);
    let out = s.take();
    assert_eq!(s.world.castle(CastleId(1)).unwrap().owner_id, WOLVES);
    let engraved = messages_to(&out, P5)
        .into_iter()
        .find(|m| m.id == SystemMessageId::ClanS1HasSuccessfullyEngravedTheHolyArtifact)
        .unwrap();
    assert_eq!(engraved.params, [SystemMessageParam::Text("Wolves".to_owned())]);
    assert!(s.siege().in_progress);
    assert!(s.world.castle(CastleId(1)).unwrap().first_mid_victory);
    let defenders: Vec<(i32, SiegeClanType)> = s
        .siege()
        .defender_clans()
        .iter()
        .map(|c| (c.clan_id, c.kind))
        .collect();
    assert_eq!(defenders, [(WOLVES, SiegeClanType::Owner)]);
    let attackers: Vec<(i32, SiegeClanType)> = s
        .siege()
        .attacker_clans()
        .iter()
        .map(|c| (c.clan_id, c.kind))
        .collect();
    assert_eq!(attackers, [(LIONS, SiegeClanType::Attacker)]);
    assert_eq!(s.world.siege_state_of(ObjectId(P3)), 2);
    assert_eq!(s.world.siege_state_of(ObjectId(P1)), 1);
    // The towers came back (`_controlTowerCount = 0` and a new spawn).
    assert_eq!(npcs_of(&s.world, CONTROL_TOWER).len(), 1);
    // `removeMercs` of the new owner (a siege job).
    assert!(residence_writes(&out).contains(&ResidenceWrite::SiegeGuardsDeleteHired { castle_id: 1 }));

    s.admin(P9, "admin_endsiege Gludio");
    let out = s.take();
    let victory = messages_to(&out, P5)
        .into_iter()
        .find(|m| m.id == SystemMessageId::ClanS1IsVictoriousOverS2SCastleSiege)
        .unwrap();
    assert_eq!(victory.params[0], SystemMessageParam::Text("Wolves".to_owned()));
    // A new owner: the ticket count starts again (a siege job write), no blood alliance.
    assert!(
        residence_writes(&out).contains(&ResidenceWrite::CastleTicketBuyCount {
            castle_id: 1,
            count: 0
        })
    );
    assert_eq!(s.world.clan(WOLVES).unwrap().blood_alliance_count, 0);
    // `updateClansReputation`: the old owner loses `LooseCastlePoints` (3000, below 0 is
    // allowed), the new owner takes at most what the old one had (0 here).
    assert_eq!(s.world.clan(LIONS).unwrap().reputation_score, -3000);
    assert_eq!(s.world.clan(WOLVES).unwrap().reputation_score, 0);
}

/// `HeadquarterCreate` and `SiegeFlag`: the clan leader's headquarters with the clan name as its
/// title, the `canCreateBase` checks, "Your base is being attacked." once every 20 seconds, the
/// "to siege HQ" button of `Die` and the restart at the headquarters.
#[test]
fn headquarters_stand_warn_their_clan_and_take_the_dead_back() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    // Outside a siege the condition refuses ("cannot be used due to unsuitable terms").
    assert!(
        !s.world
            .siege_create_headquarters_allowed(ObjectId(P3), Some((247, 1)))
    );
    let out = s.take();
    assert_eq!(
        message_ids(&out, P3),
        [SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms]
    );

    s.input(
        P9,
        ClientInput::Action {
            object_id: P3,
            action_id: 0,
        },
    );
    s.admin(P9, "admin_add_attacker Gludio");
    s.admin(P9, "admin_startsiege Gludio");
    s.take();
    // The leader stands outside the headquarters zone.
    assert!(
        !s.world
            .siege_create_headquarters_allowed(ObjectId(P3), Some((247, 1)))
    );
    let out = s.take();
    assert_eq!(message_ids(&out, P3), [SystemMessageId::YouCannotSetUpABaseHere]);
    // A member is no leader.
    s.teleport(P4, X - 1000, Y, Z);
    s.teleport(P3, X - 1000, Y, Z);
    assert!(
        !s.world
            .siege_create_headquarters_allowed(ObjectId(P4), Some((247, 1)))
    );
    assert!(
        s.world
            .siege_create_headquarters_allowed(ObjectId(P3), Some((247, 1)))
    );
    s.take();
    s.world.siege_create_headquarters(ObjectId(P3), false);
    let flag = npcs_of(&s.world, HQ)[0];
    // 50 above the leader.
    let (lx, ly, lz) = s.world.position_of(ObjectId(P3)).unwrap();
    assert_eq!(s.world.position_of(flag), Some((lx, ly, lz + 50)));
    assert_eq!(s.world.npc_custom_title(flag), Some("Wolves"));
    assert_eq!(s.siege().flags_of(WOLVES), [flag]);
    // `MaxFlags` (1): no second one.
    assert!(
        !s.world
            .siege_create_headquarters_allowed(ObjectId(P3), Some((247, 1)))
    );
    s.take();

    // Damage warns the clan, then not again for 20 seconds.
    s.world.reduce_current_hp(flag, 10.0, Some(ObjectId(P1)));
    let out = s.take();
    assert!(message_ids(&out, P4).contains(&SystemMessageId::YourBaseIsBeingAttacked));
    assert!(!message_ids(&out, P1).contains(&SystemMessageId::YourBaseIsBeingAttacked));
    s.world.reduce_current_hp(flag, 10.0, Some(ObjectId(P1)));
    assert!(!message_ids(&s.take(), P4).contains(&SystemMessageId::YourBaseIsBeingAttacked));
    s.run_ms(20_050);
    s.world.reduce_current_hp(flag, 10.0, Some(ObjectId(P1)));
    assert!(message_ids(&s.take(), P4).contains(&SystemMessageId::YourBaseIsBeingAttacked));

    // A dead attacker may restart at the headquarters.
    s.world.do_die(ObjectId(P4), Some(ObjectId(P1)));
    let die = s.world.die_packet(ObjectId(P4));
    assert!(die.to_siege_hq);
    assert!(!die.to_castle);
    // `AttackerRespawn` is 0: the restart runs at the next tick (`DeathTask`).
    s.take();
    s.input(P4, ClientInput::RestartPoint { point_type: 4 });
    s.run_ms(50);
    let out = s.take();
    assert!(!packets_to(&out, P4, "TELEPORT_TO_LOCATION").is_empty());
    let dead_defender_die = {
        s.world.do_die(ObjectId(P2), Some(ObjectId(P3)));
        s.world.die_packet(ObjectId(P2))
    };
    assert!(dead_defender_die.to_castle && !dead_defender_die.to_siege_hq);

    // The headquarters dies: it leaves its clan and vanishes.
    s.world.do_die(flag, Some(ObjectId(P1)));
    assert!(s.siege().flags_of(WOLVES).is_empty());
    assert!(npcs_of(&s.world, HQ).is_empty());
}

/// `Player.onKillUpdatePvPKarma` in a siege zone: a kill of the other side counts for both clans
/// (`addSiegeKill`, `addSiegeDeath`), a kill of a siege friend does not, and no PvP kill or karma
/// comes of it.
#[test]
fn kills_in_the_siege_count_for_the_clans() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    s.input(
        P9,
        ClientInput::Action {
            object_id: P3,
            action_id: 0,
        },
    );
    s.admin(P9, "admin_add_attacker Gludio");
    s.admin(P9, "admin_startsiege Gludio");
    s.take();
    s.world.do_die(ObjectId(P1), Some(ObjectId(P3)));
    assert_eq!(s.world.clan_siege_kills(WOLVES), 1);
    assert_eq!(s.world.clan_siege_deaths(LIONS), 1);
    assert_eq!(s.world.player(ObjectId(P3)).unwrap().data.karma, 0);
    assert_eq!(s.world.player(ObjectId(P3)).unwrap().data.pvp_kills, 0);
    // Two attackers before the first owner change are siege friends.
    assert!(s.world.is_siege_friend(ObjectId(P3), ObjectId(P4)));
    s.world.do_die(ObjectId(P4), Some(ObjectId(P3)));
    assert_eq!(s.world.clan_siege_kills(WOLVES), 1);
    // The end clears the counts.
    s.admin(P9, "admin_endsiege Gludio");
    assert_eq!(s.world.clan_siege_kills(WOLVES), 0);
}

/// The restart point "to castle" (`MapRegionData.getTeleToLocation(CASTLE)`): the castle zone's
/// spawn for the owner's members, its chaotic spawn for a member with karma.
#[test]
fn the_castle_restart_point_is_the_castle_zones() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    assert_eq!(
        s.world
            .get_tele_to_location(ObjectId(P2), TeleportWhereType::Castle),
        Some((X + 2000, Y, Z))
    );
    s.world.player_mut(ObjectId(P2)).unwrap().data.karma = 100;
    assert_eq!(
        s.world
            .get_tele_to_location(ObjectId(P2), TeleportWhereType::Castle),
        Some((X + 2100, Y, Z))
    );
    // A clan without a castle has no castle restart point (the town: no regions here).
    assert_eq!(
        s.world
            .get_tele_to_location(ObjectId(P3), TeleportWhereType::Castle),
        None
    );
    let die = {
        s.world.do_die(ObjectId(P1), None);
        s.world.die_packet(ObjectId(P1))
    };
    assert!(die.to_castle && !die.to_siege_hq && !die.to_clan_hall && !die.to_fortress);
}

// ---------------------------------------------------------------------------------- mercenaries

/// `MercTicket.onItemUse`: the castle lord places a ticket in his castle; the mercenary is hired
/// (`Insert Into castle_siege_guards`, his own action: written without the switch too) and the
/// ticket lies there. Somebody else cannot pick it up; the lord can, which undoes the hire; the
/// start of the siege takes the tickets away.
#[test]
fn mercenary_tickets_hire_and_can_be_taken_back_by_the_lord() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    let mut lord = pdata(P1, LIONS, X);
    lord.items.push(ItemData {
        object_id: ObjectId(0x6000_2000),
        item_id: TICKET,
        count: 2,
        loc: "INVENTORY".to_owned(),
        ..Default::default()
    });
    s.enter(lord);
    s.enter(pdata(P3, WOLVES, X + 100));
    let mut gm = pdata(P9, 0, X + 40);
    gm.access_level = 100;
    gm.profile.clan = None;
    s.enter(gm);
    s.take();
    s.input(
        P1,
        ClientInput::UseItem {
            object_id: 0x6000_2000,
            ctrl_pressed: false,
        },
    );
    let out = s.take();
    let placed = messages_to(&out, P1)
        .into_iter()
        .find(|m| m.id == SystemMessageId::PlaceS1InTheCurrentLocationAndDirectionDoYouWishToContinue)
        .unwrap();
    assert_eq!(placed.params, [SystemMessageParam::ItemName(TICKET)]);
    assert_eq!(
        residence_writes(&out),
        vec![ResidenceWrite::SiegeGuardInsert(SiegeGuardRecord {
            castle_id: 1,
            npc_id: MERC,
            x: X,
            y: Y,
            z: Z,
            heading: 0,
            respawn_delay: 0,
            is_hired: true,
        })]
    );
    let ticket = s.world.dropped_merc_tickets()[0];
    assert_eq!(s.world.ground_item(ticket).unwrap().data.item_id, TICKET);
    // The mercenary shows for 3 seconds.
    assert_eq!(npcs_of(&s.world, MERC).len(), 1);
    s.run_ms(3_050);
    assert!(npcs_of(&s.world, MERC).is_empty());
    // A second ticket right there is too close.
    s.input(
        P1,
        ClientInput::UseItem {
            object_id: 0x6000_2000,
            ctrl_pressed: false,
        },
    );
    let out = s.take();
    assert_eq!(
        message_ids(&out, P1),
        [SystemMessageId::PositioningCannotBeDoneHereBecauseTheDistanceBetweenMercenariesIsTooShort]
    );
    // The Wolves' leader may not position mercenaries, nor pick the ticket up.
    s.input(
        P3,
        ClientInput::Action {
            object_id: ticket.0,
            action_id: 0,
        },
    );
    let out = s.take();
    let text = decode::<SystemMessage>(&packets_to(&out, P3, "SYSTEM_MESSAGE")[0]);
    assert_eq!(
        text.params,
        [SystemMessageParam::Text(
            "Only the castle lord can pickup mercenaries.".to_owned()
        )]
    );
    assert!(s.world.ground_item(ticket).is_some());
    // The lord picks it up: the hire goes (`removeMerc`).
    s.world.do_pickup_item(ObjectId(P1), ticket);
    let out = s.take();
    assert!(s.world.ground_item(ticket).is_none());
    assert!(s.world.dropped_merc_tickets().is_empty());
    assert!(
        residence_writes(&out).contains(&ResidenceWrite::SiegeGuardDeleteHired {
            npc_id: MERC,
            x: X,
            y: Y,
            z: Z
        })
    );
}

/// `MercTicketManager.load`: a ticket on the ground for every hired mercenary at startup; the
/// siege start takes them away and the hired mercenaries of an owned castle stand as guards
/// without respawn (`SiegeGuardManager.spawnSiegeGuard`); the end removes them.
#[test]
fn hired_mercenaries_lie_as_tickets_and_stand_in_the_siege() {
    let merc = SiegeGuardRecord {
        castle_id: 1,
        npc_id: MERC,
        x: X + 50,
        y: Y,
        z: Z,
        heading: 0,
        respawn_delay: 0,
        is_hired: true,
    };
    let guard = SiegeGuardRecord {
        npc_id: GUARD,
        respawn_delay: 600,
        is_hired: false,
        ..merc
    };
    let load = SiegeLoad {
        siege_guards: vec![guard, merc],
        siege_clans: vec![wolves_row()],
        ..SiegeLoad::default()
    };
    let mut s = Scene::with(false, LIONS, NEXT_SIEGE, load, |_| {});
    s.enter_all();
    assert_eq!(s.world.dropped_merc_tickets().len(), 1);
    s.admin(P9, "admin_startsiege Gludio");
    s.take();
    assert!(s.world.dropped_merc_tickets().is_empty());
    // An owned castle has its hired mercenaries, not the datapack's guards.
    assert_eq!(npcs_of(&s.world, MERC).len(), 1);
    assert!(npcs_of(&s.world, GUARD).is_empty());
    let mercenary = npcs_of(&s.world, MERC)[0];
    // A defender is attackable by the attackers in the siege, not by the defenders.
    assert!(s.world.is_auto_attackable_by(mercenary, ObjectId(P3)));
    assert!(!s.world.is_auto_attackable_by(mercenary, ObjectId(P1)));
    s.admin(P9, "admin_endsiege Gludio");
    assert!(npcs_of(&s.world, MERC).is_empty());
}

/// A castle without an owner has the datapack's guards; they keep respawning while their
/// control tower stands, and stop once it falls (`ControlTower.doDie`).
#[test]
fn the_guards_of_a_free_castle_stop_respawning_with_their_tower() {
    let guard = SiegeGuardRecord {
        castle_id: 1,
        npc_id: GUARD,
        x: X + 150,
        y: Y,
        z: Z,
        heading: 0,
        respawn_delay: 600,
        is_hired: false,
    };
    let load = SiegeLoad {
        siege_guards: vec![guard],
        siege_clans: vec![wolves_row()],
        ..SiegeLoad::default()
    };
    let mut s = Scene::with(false, 0, NEXT_SIEGE, load, |_| {});
    s.enter_all();
    s.admin(P9, "admin_startsiege Gludio");
    s.take();
    let guards = npcs_of(&s.world, GUARD);
    assert_eq!(guards.len(), 1);
    let spawn = s.world.npc(guards[0]).unwrap().spawn.unwrap();
    assert!(s.world.is_respawn_enabled(spawn));
    let tower = npcs_of(&s.world, CONTROL_TOWER)[0];
    s.world.do_die(tower, Some(ObjectId(P3)));
    assert_eq!(s.siege().control_tower_count, 0);
    assert!(!s.world.is_respawn_enabled(spawn));
}

// ---------------------------------------------------------------------------------- doors

/// `Castle.loadDoorUpgrade` (an owned castle) and `spawnDoor`: the gate's HP times the ratio, a
/// destroyed gate stands again at the start of the siege and is closed.
#[test]
fn door_upgrades_multiply_the_gate_and_the_siege_raises_it_again() {
    let load = SiegeLoad {
        door_upgrades: vec![DoorUpgradeRecord {
            door_id: DOOR,
            ratio: 3,
            castle_id: 1,
        }],
        siege_clans: vec![wolves_row()],
        ..SiegeLoad::default()
    };
    let mut s = Scene::with(false, LIONS, NEXT_SIEGE, load, |_| {});
    s.enter_all();
    let gate = s.world.door_by_door_id(DOOR).unwrap().object_id;
    assert_eq!(s.world.door(gate).unwrap().max_hp(), 3000);
    assert!((s.world.door(gate).unwrap().current_hp() - 3000.0).abs() < f64::EPSILON);
    s.world.set_door_dead(gate, true);
    s.world.open_door(gate);
    s.admin(P9, "admin_startsiege Gludio");
    let door = s.world.door(gate).unwrap();
    assert!(!door.dead && !door.open);
    assert!((door.current_hp() - 3000.0).abs() < f64::EPSILON);
    // Attackable in the siege by everybody but the owner.
    assert!(s.world.is_auto_attackable_by(gate, ObjectId(P3)));
    assert!(!s.world.is_auto_attackable_by(gate, ObjectId(P1)));
}

// ---------------------------------------------------------------------------------- the rest

/// The siege skills (`SiegeManager.addSiegeSkills`) of a leader of a clan above level 3 at
/// `EnterWorld`: 246 and 247, with 844 and 845 for a castle owner.
#[test]
fn clan_leaders_get_the_siege_skills() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    let skills = |s: &Scene, id: i32| {
        let mut skills: Vec<i32> = s
            .world
            .player(ObjectId(id))
            .unwrap()
            .core
            .skills
            .iter()
            .map(|k| k.0)
            .collect();
        skills.sort_unstable();
        skills
    };
    assert_eq!(skills(&s, P1), [246, 247, 844, 845]);
    assert_eq!(skills(&s, P3), [246, 247]);
    assert!(skills(&s, P2).is_empty());
    // Bears are level 4: above 3.
    assert_eq!(skills(&s, P5), [246, 247]);
}

/// `//siege` without a castle: the castle list page; with one: `castle.htm`.
#[test]
fn the_admin_pages_list_the_castles() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    s.admin(P9, "admin_siege");
    let out = s.take();
    let html = decode::<NpcHtmlMessage>(&packets_to(&out, P9, "NPC_HTML_MESSAGE")[0]).html;
    assert_eq!(
        html,
        "<html><body><td fixwidth=90><a action=\"bypass -h admin_siege Gludio\">Gludio</a></td>\
<td fixwidth=90><a action=\"bypass -h admin_siege Dion\">Dion</a></td>|||</body></html>"
    );
    s.admin(P9, "admin_siege gludio");
    let out = s.take();
    let html = decode::<NpcHtmlMessage>(&packets_to(&out, P9, "NPC_HTML_MESSAGE")[0]).html;
    assert_eq!(html, "<html><body>Castle: Gludio</body></html>");
    // An unknown castle: Java's NullPointerException, told to the GM.
    s.admin(P9, "admin_startsiege Nowhere");
    assert!(!s.take().is_empty());
}

/// `RequestSetCastleSiegeTime`: the lord picks one of `SiegeHourList` on the siege day while the
/// registration is open ("Gludio has announced the next castle siege time."); `SiegeInfo` offers
/// the hours to him.
#[test]
fn the_lord_picks_the_siege_hour() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    s.world.handle(WorldCommand::ResidenceReread {
        result: Ok(ResidenceRereadRows::Castle(CastleRecord {
            time_registration_over: false,
            ..castle(1, "Gludio", LIONS, NEXT_SIEGE)
        })),
        then: ResidenceReadThen::Nothing,
    });
    let info = s.world.siege_info_packet(ObjectId(P1), CastleId(1)).unwrap();
    assert!(info.viewer_is_owner_leader);
    let sixteen = i32::try_from(NEXT_SIEGE / 1000).unwrap();
    assert_eq!(info.time, SiegeTime::Hours(vec![sixteen, sixteen + 4 * 3600]));
    // A member who is no leader is refused (only a log line).
    s.input(
        P2,
        ClientInput::Siege(SiegeInput::SetCastleSiegeTime {
            castle_id: 1,
            time: sixteen + 4 * 3600,
        }),
    );
    assert!(s.take().is_empty());
    s.input(
        P1,
        ClientInput::Siege(SiegeInput::SetCastleSiegeTime {
            castle_id: 1,
            time: sixteen + 4 * 3600,
        }),
    );
    let out = s.take();
    let gludio = s.world.castle(CastleId(1)).unwrap();
    // `setTimeInMillis(_time)`: the chosen second, no milliseconds.
    assert_eq!(gludio.siege_date, i64::from(sixteen + 4 * 3600) * 1000);
    assert!(gludio.time_registration_over);
    assert!(message_ids(&out, P5).contains(&SystemMessageId::S1HasAnnouncedTheNextCastleSiegeTime));
    assert_eq!(packets_to(&out, P1, "CASTLE_SIEGE_INFO").len(), 1);
}

/// The teleport of `teleportPlayer(NotOwner, TOWN)` leaves the owner's members and GMs where
/// they are; the spectators of a siege are the players in the zone who are not in it.
#[test]
fn the_zone_sorts_owners_attackers_and_spectators() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    s.input(
        P9,
        ClientInput::Action {
            object_id: P3,
            action_id: 0,
        },
    );
    s.admin(P9, "admin_add_attacker Gludio");
    s.admin(P9, "admin_startsiege Gludio");
    s.take();
    let mut attackers = s.world.siege_attackers_in_zone(CastleId(1));
    attackers.sort_unstable();
    assert_eq!(attackers, [ObjectId(P3), ObjectId(P4)]);
    let mut owners = s.world.siege_owners_in_zone(CastleId(1));
    owners.sort_unstable();
    assert_eq!(owners, [ObjectId(P1), ObjectId(P2)]);
    assert_eq!(s.world.siege_spectators_in_zone(CastleId(1)), [ObjectId(P9)]);
    s.world.siege_teleport_player(
        CastleId(1),
        SiegeTeleportWhoType::Spectator,
        TeleportWhereType::Town,
    );
    // The GM stays (and there is no town in this scene).
    assert_eq!(s.world.position_of(ObjectId(P9)), Some((X + 40, Y, Z)));
}

/// `SiegeManager.loadTrapUpgrade` of an owned castle and `FlameTower.setZoneList`: an upgraded
/// flame tower switches the first `2 * level` trap zones on, its death switches them off.
#[test]
fn an_upgraded_flame_tower_switches_its_trap_zones() {
    let load = SiegeLoad {
        trap_upgrades: vec![l2_world::siege::TrapUpgradeRecord {
            castle_id: 1,
            tower_index: 0,
            level: 1,
        }],
        siege_clans: vec![wolves_row()],
        ..SiegeLoad::default()
    };
    let mut s = Scene::with(false, LIONS, NEXT_SIEGE, load, |_| {});
    s.enter_all();
    assert_eq!(s.world.castle_trap_upgrade_level(CastleId(1), 0), 1);
    assert_eq!(s.world.trap_zone_enabled(70001), None);
    s.admin(P9, "admin_startsiege Gludio");
    assert_eq!(s.world.trap_zone_enabled(70001), Some(true));
    assert_eq!(s.world.trap_zone_enabled(70002), Some(true));
    let tower = npcs_of(&s.world, FLAME_TOWER)[0];
    s.world.do_die(tower, Some(ObjectId(P3)));
    assert_eq!(s.world.trap_zone_enabled(70001), Some(false));
}

/// `SwampZone` and `DamageZone` of `castle_trap.xml` (M7-FU-03): off until a flame tower of the
/// castle switches them on, then only during the siege and not for the defenders. A swamp slows
/// by its `move_bonus` and sends the zone's event; a damage zone hurts every `reuse` ms.
#[test]
fn the_castle_traps_slow_and_hurt_the_attackers_during_the_siege() {
    use l2_world::zones::ZoneId;

    let load = SiegeLoad {
        trap_upgrades: vec![l2_world::siege::TrapUpgradeRecord {
            castle_id: 1,
            tower_index: 0,
            level: 1,
        }],
        siege_clans: vec![wolves_row()],
        ..SiegeLoad::default()
    };
    let mut s = Scene::with(false, LIONS, NEXT_SIEGE, load, |_| {});
    // Level 1: the class of the scene has hit points for that level only (100).
    for (id, clan) in [(P1, LIONS), (P3, WOLVES), (P9, 0)] {
        let mut data = pdata(id, clan, X + 20);
        data.level = 1;
        if id == P9 {
            data.access_level = 100;
            data.profile.clan = None;
        }
        s.enter(data);
    }
    s.take();
    let swamp = (X - 2200, Y, Z);
    let damage = (X - 2800, Y, Z);
    let hp = |s: &Scene, who: i32| s.world.player(ObjectId(who)).unwrap().core.cur_hp;
    let run_speed = |s: &Scene, who: i32| s.world.stats(ObjectId(who)).unwrap().run_speed();
    let swamped = |s: &Scene, who: i32| s.world.is_inside_zone(ObjectId(who), ZoneId::Swamp);

    // Off before the siege: the tower has not switched them on, and the siege is not running.
    let normal = run_speed(&s, P3);
    s.teleport(P3, swamp.0, swamp.1, swamp.2);
    assert!(!swamped(&s, P3));
    assert_eq!(run_speed(&s, P3), normal);
    s.teleport(P3, damage.0, damage.1, damage.2);
    s.run_ms(6000);
    assert_eq!(hp(&s, P3), 100.0);
    s.teleport(P3, X + 20, Y, Z);

    s.admin(P9, "admin_startsiege Gludio");
    s.take();
    assert_eq!(s.world.trap_zone_enabled(70001), Some(true));
    // The defenders are not slowed.
    s.teleport(P1, swamp.0, swamp.1, swamp.2);
    assert!(!swamped(&s, P1));

    // An attacker is, and gets the zone's event and a `UserInfo`.
    s.take();
    s.teleport(P3, swamp.0, swamp.1, swamp.2);
    let out = s.take();
    assert!(swamped(&s, P3));
    let triggers = packets_to(&out, P3, "EVENT_TRIGGER");
    assert_eq!(triggers.len(), 1);
    let trigger = decode::<l2_protocol_game::server::OnEventTrigger>(&triggers[0]);
    assert_eq!((trigger.emitter_id, trigger.enabled), (2_418_002, true));
    assert!(!packets_to(&out, P3, "USER_INFO").is_empty());
    // `move_bonus` 0.2 on both the run and the walk speed.
    assert!(
        (run_speed(&s, P3) - normal * 0.2).abs() < 1e-9,
        "{}",
        run_speed(&s, P3)
    );
    let walk = s.world.stats(ObjectId(P3)).unwrap().walk_speed();
    s.teleport(P3, X + 20, Y, Z);
    assert!(!swamped(&s, P3));
    assert!((s.world.stats(ObjectId(P3)).unwrap().walk_speed() - walk / 0.2).abs() < 1e-6);
    let out = s.take();
    let triggers = packets_to(&out, P3, "EVENT_TRIGGER");
    assert_eq!(triggers.len(), 1);
    assert!(!decode::<l2_protocol_game::server::OnEventTrigger>(&triggers[0]).enabled);

    // The damage zone: the first pass comes at once, then every `reuse` (5 s); a defender is spared.
    s.teleport(P1, damage.0, damage.1, damage.2);
    s.teleport(P3, damage.0, damage.1, damage.2);
    s.run_ms(100);
    assert_eq!(hp(&s, P3), 90.0);
    assert_eq!(hp(&s, P1), 100.0);
    // The hit points regenerate a little between the passes, so the numbers are ranges.
    s.run_ms(2000);
    assert!(
        (90.0..93.0).contains(&hp(&s, P3)),
        "not before the 5 s are over: {}",
        hp(&s, P3)
    );
    s.run_ms(3000);
    assert!((78.0..86.0).contains(&hp(&s, P3)), "{}", hp(&s, P3));
    // Leaving stops it (the last one out cancels the task): twelve seconds, no more damage.
    s.teleport(P3, X + 20, Y, Z);
    s.teleport(P1, X, Y, Z);
    let before = hp(&s, P3);
    s.run_ms(12_000);
    assert!(hp(&s, P3) >= before, "{} < {before}", hp(&s, P3));

    // The siege is over: a castle zone hurts nobody any more.
    s.admin(P9, "admin_endsiege Gludio");
    s.teleport(P3, damage.0, damage.1, damage.2);
    let before = hp(&s, P3);
    s.run_ms(6000);
    assert!(hp(&s, P3) >= before, "{} < {before}", hp(&s, P3));
}

/// `RequestAllCastleInfo`: `ExShowCastleInfo` with each castle's owner, tax and siege date.
#[test]
fn the_castle_window_lists_owners_taxes_and_siege_dates() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    s.input(P5, ClientInput::Siege(SiegeInput::AllCastleInfo));
    let out = s.take();
    let info = decode::<l2_protocol_game::server::siege::ExShowCastleInfo>(
        &packets_to(&out, P5, "EX_SHOW_CASTLE_INFO")[0],
    );
    let lines: Vec<(i32, String, i32, i32)> = info
        .castles
        .iter()
        .map(|c| {
            (
                c.residence_id,
                c.owner_name.clone(),
                c.tax_percent,
                c.siege_date_secs,
            )
        })
        .collect();
    let date = i32::try_from(NEXT_SIEGE / 1000).unwrap();
    assert_eq!(
        lines,
        [(1, "Lions".to_owned(), 0, date), (2, String::new(), 0, date)]
    );
}

/// `ConditionPlayerCanResurrect`: a living target is refused ("cannot be used due to unsuitable
/// terms"); in a castle siege in progress every branch refuses (Java's quirk), a defender of a
/// castle without towers with "The guardian tower has been destroyed...", an attacker without
/// headquarters with "If a base camp does not exist...", and only the Blessed Scroll of
/// Battlefield Resurrection (2393) passes.
#[test]
fn resurrection_follows_the_siege_rules() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    assert!(
        !s.world
            .cond_can_resurrect(ObjectId(P2), Some(ObjectId(P1)), Some((1016, 1)))
    );
    let out = s.take();
    assert_eq!(
        message_ids(&out, P2),
        [SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms]
    );
    // Dead outside a siege: yes.
    s.world.do_die(ObjectId(P1), None);
    assert!(
        s.world
            .cond_can_resurrect(ObjectId(P2), Some(ObjectId(P1)), Some((1016, 1)))
    );
    s.world.do_revive(ObjectId(P1));

    s.input(
        P9,
        ClientInput::Action {
            object_id: P3,
            action_id: 0,
        },
    );
    s.admin(P9, "admin_add_attacker Gludio");
    s.admin(P9, "admin_startsiege Gludio");
    s.take();
    s.world.do_die(ObjectId(P4), None);
    assert!(
        !s.world
            .cond_can_resurrect(ObjectId(P3), Some(ObjectId(P4)), Some((1016, 1)))
    );
    let out = s.take();
    assert_eq!(
        message_ids(&out, P3),
        [SystemMessageId::IfABaseCampDoesNotExistResurrectionIsNotPossible]
    );
    assert!(
        s.world
            .cond_can_resurrect(ObjectId(P3), Some(ObjectId(P4)), Some((2393, 1)))
    );
    // The defender's tower falls.
    let tower = npcs_of(&s.world, CONTROL_TOWER)[0];
    s.world.do_die(tower, Some(ObjectId(P3)));
    s.world.do_die(ObjectId(P2), None);
    s.take();
    assert!(
        !s.world
            .cond_can_resurrect(ObjectId(P1), Some(ObjectId(P2)), Some((1016, 1)))
    );
    let out = s.take();
    assert_eq!(
        message_ids(&out, P1),
        [SystemMessageId::TheGuardianTowerHasBeenDestroyedAndResurrectionIsNotPossible]
    );
}

/// `ConditionPlayerCanTakeCastle`: the attacker's leader next to his castle's artefact may cast
/// Seal of Ruler, and the defenders hear "The opposing clan has started to engrave the holy
/// artifact."; too far away the casting stops.
#[test]
fn seal_of_ruler_needs_the_leader_at_the_artefact() {
    let mut s = Scene::new(false, LIONS, NEXT_SIEGE);
    s.enter_all();
    let artefact = s.world.spawn_npc_at(ARTEFACT, X + 300, Y, Z, 0).unwrap();
    s.input(
        P9,
        ClientInput::Action {
            object_id: P3,
            action_id: 0,
        },
    );
    s.admin(P9, "admin_add_attacker Gludio");
    s.admin(P9, "admin_startsiege Gludio");
    s.take();
    // Too far (the leader stands 280 away).
    assert!(
        !s.world
            .cond_can_take_castle(ObjectId(P3), Some(artefact), Some((246, 1)))
    );
    let out = s.take();
    assert_eq!(
        message_ids(&out, P3),
        [SystemMessageId::TheDistanceIsTooFarAndSoTheCastingHasBeenStopped]
    );
    s.teleport(P3, X + 250, Y, Z);
    s.take();
    assert!(
        s.world
            .cond_can_take_castle(ObjectId(P3), Some(artefact), Some((246, 1)))
    );
    let out = s.take();
    assert!(
        message_ids(&out, P1).contains(&SystemMessageId::TheOpposingClanHasStartedToEngraveTheHolyArtifact)
    );
    assert!(
        !message_ids(&out, P3).contains(&SystemMessageId::TheOpposingClanHasStartedToEngraveTheHolyArtifact)
    );
    // A member is no leader.
    assert!(
        !s.world
            .cond_can_take_castle(ObjectId(P4), Some(artefact), Some((246, 1)))
    );
}
