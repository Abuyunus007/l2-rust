#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Residences (M7-RES-01): loading castles, fortresses and clan halls like Java's managers, the
//! owners on the clans, the residential skills, the circlets, the castle tax and treasury, the
//! function fees, the clan hall zone's `AgitDecoInfo`, the `SiegeDatabaseWrites` switch (D-51) and
//! the re-reads at `CharacterSelect` and on demand (D-50).
//! Java: managers/{CastleManager,FortManager}.java, mechanics/siege/{Castle,Fort}.java,
//! entity/residences/{AbstractResidence,ClanHall,AuctionableHall}.java, data/sql/ClanHallTable.java,
//! entity/zone/type/ClanHallZone.java, network/serverpackets/AgitDecoInfo.java,
//! network/clientpackets/{EnterWorld,RequestAnswerJoinPledge}.java, entity/clan/Clan.java
//! (removeClanMember). The expected values come from the Java code named in each test.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates, KarmaLoss,
    MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates, SkillTrees,
    SpawnData, SpawnLoadOptions, StaticObjectTemplates, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{AgitDecoInfo, PlaySound, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::residence::{
    CastleRecord, ClanHallKind, ClanHallRecord, FortRecord, FunctionRecord, ResidenceReadThen,
    ResidenceRereadRows, SiegableHallRecord, Writes,
};
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

/// Residence Body (590, Gludio's line of `pledgeSkillTree.xml`) and a fortress skill (591).
const SKILLS: &str = r#"<list>
<skill id="590" levels="1" name="Residence Body"><operateType>P</operateType></skill>
<skill id="591" levels="1" name="Fortress Skill"><operateType>P</operateType></skill>
</list>"#;

const TREES: &str = r#"<list>
<skillTree type="pledgeSkillTree">
  <skill skillName="Residence Body" skillId="590" skillLevel="1" getLevel="4" residenceSkill="true">
    <socialClass>APPRENTICE</socialClass><residenceId>1</residenceId><residenceId>8</residenceId></skill>
  <skill skillName="Fortress Skill" skillId="591" skillLevel="1" getLevel="4" residenceSkill="true">
    <socialClass>APPRENTICE</socialClass><residenceId>101</residenceId></skill>
</skillTree>
</list>"#;

const ADENA: i32 = 57;
/// `CastleManager._castleCirclets[1]`: the Gludio circlet.
const GLUDIO_CIRCLET: i32 = 6838;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="6838" type="Armor" name="Gludio Circlet"><set name="bodypart" val="hair" /><set name="weight" val="10" /></item>
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

struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
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
        true
    }
    fn exp_percent(&self, _exp: i64, _level: i32) -> f64 {
        0.0
    }
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        item_id == ADENA
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
/// Lions: P1 (leader) and P2; Wolves: P3 (leader) and P4.
const P1: i32 = 0x6000_0001;
const P2: i32 = 0x6000_0002;
const P3: i32 = 0x6000_0003;
const P4: i32 = 0x6000_0004;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
const HALL_X: i32 = X + 20_000;

/// Castle 1's siege zone holds the players' spot; castle 2 and fortress 101 are elsewhere; hall 36
/// is far east (two zones, the second registers last), hall 37 has no zone.
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
  <zone name="gludio_castle" type="CastleZone" {}<stat name="castleId" val="1" /><spawn X="{}" Y="{}" Z="{}" /></zone>
  <zone name="dion_castle_battlefield_territory" type="SiegeZone" {}<stat name="castleId" val="2" /></zone>
  <zone name="shanty_fortress_siege" type="SiegeZone" {}<stat name="fortId" val="101" /></zone>
  <zone name="shanty_fortress" type="FortZone" {}<stat name="fortId" val="101" /></zone>
  <zone name="agit_old" type="ClanHallZone" {}<stat name="clanHallId" val="36" /></zone>
  <zone name="agit" type="ClanHallZone" {}<stat name="clanHallId" val="36" /></zone>
</list>"#,
        cuboid(X, Y, 2000),
        cuboid(X + 500, Y, 200),
        X + 500,
        Y,
        Z,
        cuboid(X + 40_000, Y, 2000),
        cuboid(X - 40_000, Y, 2000),
        cuboid(X - 40_000, Y, 200),
        cuboid(HALL_X + 5000, Y, 200),
        cuboid(HALL_X, Y, 300),
    )
}

/// The flag poles: Shanty's, but none for fortress 102 "Hive".
const STATIC_OBJECTS: &str = r#"<list>
  <object id="18220500" name="Shanty_flagpole" type="3" x="-111338" y="258271" z="-3104" />
  <object id="18220501" name="Shanty_sign" type="0" x="-111338" y="258271" z="-3104" />
</list>"#;

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

impl Scene {
    fn new(siege_database_writes: bool) -> Self {
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
        write("data/zones/residences.xml", &zones_xml());
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Recorder(log.clone())));
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_exp_data(
            Arc::new(ExperienceTable::from_xml(r#"<table maxLevel="85" maxPetLevel="85"></table>"#).unwrap()),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml(TREES).unwrap()),
        );
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        let npcs = Arc::new(NpcTemplates::from_xml("<list></list>", &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
        world
            .spawn_static_objects(&StaticObjectTemplates::from_xml(STATIC_OBJECTS).unwrap(), &ids)
            .unwrap();
        world.set_residence_settings(ResidenceSettings {
            siege_database_writes,
            ..ResidenceSettings::default()
        });
        world.load_clans(vec![lions(1), wolves()], Vec::new());
        Self {
            world,
            log,
            _root: root,
        }
    }

    fn load(&mut self, load: ResidenceLoad) {
        self.world.load_residences(load);
        self.log.take();
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

    fn take(&mut self) -> Vec<Out> {
        self.log.take()
    }

    fn run_ms(&mut self, ms: u64) {
        for _ in 0..ms.div_ceil(50) {
            self.world.tick();
        }
    }

    fn clan(&self, id: i32) -> &Clan {
        self.world.clan(id).unwrap()
    }

    fn skills_of(&self, id: i32) -> Vec<i32> {
        let mut skills: Vec<i32> = self
            .world
            .player(ObjectId(id))
            .unwrap()
            .core
            .skills
            .iter()
            .map(|s| s.0)
            .collect();
        skills.sort_unstable();
        skills
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

/// Lions (clan 500) with `hasCastle = castle_id` and a clan warehouse of 10,000 adena.
fn lions(castle_id: i32) -> ClanRecord {
    ClanRecord {
        clan_id: LIONS,
        data: Some(ClanDataRecord {
            name: Some("Lions".to_owned()),
            level: 5,
            castle_id,
            leader_id: P1,
            ..ClanDataRecord::default()
        }),
        members: vec![member(P1, 1), member(P2, 5)],
        warehouse: vec![ItemData {
            object_id: ObjectId(0x6000_1000),
            item_id: ADENA,
            count: 10_000,
            loc: "CLANWH".to_owned(),
            ..Default::default()
        }],
        ..ClanRecord::default()
    }
}

fn wolves() -> ClanRecord {
    ClanRecord {
        clan_id: WOLVES,
        data: Some(ClanDataRecord {
            name: Some("Wolves".to_owned()),
            level: 5,
            leader_id: P3,
            ..ClanDataRecord::default()
        }),
        members: vec![member(P3, 1), member(P4, 5)],
        ..ClanRecord::default()
    }
}

fn castle(id: i32, name: &str, owner_id: i32, tax_percent: i32) -> CastleRecord {
    CastleRecord {
        id,
        found: true,
        name: name.to_owned(),
        tax_percent,
        time_registration_over: true,
        owner_id,
        ..CastleRecord::default()
    }
}

fn hall(id: i32, owner_id: i32, grade: i32, functions: Vec<FunctionRecord>) -> ClanHallRecord {
    ClanHallRecord {
        id,
        name: format!("Hall {id}"),
        owner_id,
        lease: 500_000,
        desc: String::new(),
        location: "Gludio".to_owned(),
        paid_until: 1_700_000_000_000,
        grade,
        paid: true,
        functions,
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

fn residence_writes(out: &[Out]) -> Vec<ResidenceWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Residence(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn item_row_ops(out: &[Out]) -> Vec<ItemRowOp> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ItemRows { ops, .. }) => Some(ops.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

/// Gludio (owned by the Lions), Dion (free), Shanty (owned by the Lions), Hive (no flag pole),
/// hall 36 (the Wolves', grade 2) and hall 37 (owned by a clan that does not exist).
fn standard_load() -> ResidenceLoad {
    ResidenceLoad {
        castles: vec![castle(1, "Gludio", LIONS, 10), castle(2, "Dion", 0, 15)],
        forts: vec![
            FortRecord {
                id: 101,
                name: "Shanty".to_owned(),
                owner_id: LIONS,
                last_owned_time: 1_700_000_000_000,
                ..FortRecord::default()
            },
            FortRecord {
                id: 102,
                name: "Hive".to_owned(),
                ..FortRecord::default()
            },
        ],
        clan_halls: vec![
            hall(
                36,
                WOLVES,
                2,
                vec![
                    FunctionRecord {
                        function_type: 3,
                        lvl: 260,
                        lease: 1000,
                        rate: 86_400_000,
                        end_time: i64::MAX / 2,
                    },
                    FunctionRecord {
                        function_type: 1,
                        lvl: 1,
                        lease: 1000,
                        rate: 86_400_000,
                        end_time: i64::MAX / 2,
                    },
                ],
            ),
            hall(37, 999, 1, vec![]),
        ],
        siegable_halls: vec![SiegableHallRecord {
            id: 21,
            name: "Fortress of Resistance".to_owned(),
            owner_id: 0,
            next_siege: 0,
            siege_length: 3_600_000,
            schedule_config: Some("14;0;0;12;00".to_owned()),
            ..SiegableHallRecord::default()
        }],
    }
}

// ---------------------------------------------------------------------------------- loading

/// `CastleManager.loadInstances`, `FortManager.loadInstances`, `ClanHallTable`: the owners reach
/// the clans (`hasCastle`, `setFortId`, `setHideoutId`), the zones and flag poles are found, a hall
/// whose owner is gone is freed, and the forts stop at the first one without a flag pole.
#[test]
fn loading_installs_owners_zones_and_flag_poles_like_java() {
    let mut s = Scene::new(true);
    s.world.load_residences(standard_load());
    let out = s.take();

    let gludio = s.world.castle(CastleId(1)).unwrap();
    assert_eq!((gludio.owner_id, gludio.tax_percent), (LIONS, 10));
    assert!((gludio.tax_rate - 0.1).abs() < f64::EPSILON);
    assert!(gludio.siege_zone.is_some() && gludio.zone.is_some());
    assert_eq!(gludio.residential_skills, [(590, 1)]);
    assert_eq!(s.world.castle_by_owner(LIONS), Some(CastleId(1)));
    assert_eq!(s.world.castle_by_name(" gLuDiO "), Some(CastleId(1)));
    assert_eq!(s.world.castle_at(X, Y, Z), Some(CastleId(1)));
    assert_eq!(s.world.castle_at(X + 40_000, Y, Z), Some(CastleId(2)));
    assert_eq!(s.world.castle_at(X + 10_000, Y, Z), None);
    // `findNearestCastleIndex`: outside every siege zone the nearest one.
    assert_eq!(
        s.world.nearest_castle(X + 30_000, Y, Z, i64::MAX),
        Some(CastleId(2))
    );
    assert_eq!(s.world.nearest_castle(X + 30_000, Y, Z, 100), None);
    assert!(s.world.has_owned_castle());

    // Shanty is owned and shows its flag; Hive has no flag pole, so it is not loaded.
    assert_eq!(s.world.forts().len(), 1);
    let shanty = s.world.fort(FortId(101)).unwrap();
    assert_eq!(shanty.owner_id, LIONS);
    assert_eq!(shanty.residential_skills, [(591, 1)]);
    let pole = shanty.flag_pole.unwrap();
    assert_eq!(s.world.static_object(pole).unwrap().mesh_index, 1);
    assert_eq!(s.clan(LIONS).fort_id, 101);
    assert_eq!(s.world.fort_by_owner(LIONS), Some(FortId(101)));
    assert_eq!(s.world.fort_at(X - 40_000, Y, Z), Some(FortId(101)));

    // Hall 36: the Wolves' hideout, its zone is the second one that names it.
    assert_eq!(s.clan(WOLVES).hideout_id, 36);
    let h36 = s.world.clan_hall(ClanHallId(36)).unwrap();
    assert!(!h36.is_free);
    assert_eq!(s.world.clan_hall_at(HALL_X, Y, Z), Some(ClanHallId(36)));
    assert_eq!(s.world.clan_hall_at(HALL_X + 5000, Y, Z), None);
    assert_eq!(s.world.auctionable_hall_by_owner(WOLVES), Some(ClanHallId(36)));
    assert_eq!(s.world.clan_hall_by_owner(WOLVES), Some(ClanHallId(36)));
    assert_eq!(
        s.world
            .residence_functions(ResidenceRef::ClanHall(ClanHallId(36)))
            .len(),
        2
    );
    // Hall 37: its clan is gone, so `free()` from the constructor. NOTE(java-quirk): the row gets
    // `paidUntil` 0 and `paid` 0, the hall keeps the row's values.
    let h37 = s.world.clan_hall(ClanHallId(37)).unwrap();
    assert_eq!(h37.owner_id, 0);
    assert!(h37.is_free && h37.paid());
    assert!(s.world.clan_hall_is_free(ClanHallId(37)));
    let writes = residence_writes(&out);
    assert_eq!(
        writes
            .iter()
            .filter(|w| !matches!(w, ResidenceWrite::SiegableHall { .. }))
            .cloned()
            .collect::<Vec<_>>(),
        [ResidenceWrite::ClanHall {
            hall_id: 37,
            owner_id: 0,
            paid_until: 0,
            paid: false
        }]
    );
    // The conquerable hall's siege was due within the hour: `updateNextSiege()` moved it and wrote
    // it (M7-CHSIEGE-01).
    assert!(
        writes.iter().any(|w| matches!(
            w,
            ResidenceWrite::SiegableHall {
                hall_id: 21,
                owner_id: 0,
                ..
            }
        )),
        "{writes:?}"
    );
    // The conquerable hall is a hall too.
    let h21 = s.world.clan_hall(ClanHallId(21)).unwrap();
    assert!(h21.is_siegable_hall() && h21.is_free);
    assert_eq!(h21.grade(), 0);
}

/// D-51: the start-up `free()` of a hall whose clan is gone is a siege-like job.
#[test]
fn loading_without_the_switch_writes_nothing() {
    let mut s = Scene::new(false);
    s.world.load_residences(standard_load());
    assert!(residence_writes(&s.take()).is_empty());
    assert_eq!(s.world.clan_hall(ClanHallId(37)).unwrap().owner_id, 0);
}

// ---------------------------------------------------------------------------------- skills

/// `EnterWorld`'s "Residential skills support": a member of the castle's and the fortress's owner
/// gets their skills (not stored) and a second `SkillList`.
#[test]
fn members_get_the_residential_skills_at_enter_world() {
    let mut s = Scene::new(false);
    s.load(standard_load());
    s.enter(pdata(P2, LIONS, X));
    assert_eq!(s.skills_of(P2), [590, 591]);
    s.enter(pdata(P3, WOLVES, X));
    assert!(s.skills_of(P3).is_empty());
    s.take();
    s.run_ms(500);
    assert_eq!(packets_to(&s.take(), P2, "SKILL_LIST").len(), 1);
}

/// `Castle.setOwner(clan)`: the old owner loses the castle, the circlets (online: taken off and
/// destroyed; offline: the row) and the skills; the new owner gets the castle id,
/// `PledgeShowInfoUpdate`, `Siege_Victory` and the skills. Only with the switch are the rows
/// written.
#[test]
fn a_new_castle_owner_takes_the_skills_and_the_circlets_go() {
    for switch in [false, true] {
        let mut s = Scene::new(switch);
        s.load(standard_load());
        let mut p2 = pdata(P2, LIONS, X);
        p2.items.push(ItemData {
            object_id: ObjectId(0x6000_2000),
            item_id: GLUDIO_CIRCLET,
            count: 1,
            loc: "PAPERDOLL".to_owned(),
            loc_data: 2,
            ..Default::default()
        });
        s.enter(p2);
        s.enter(pdata(P3, WOLVES, X));
        s.run_ms(500);
        s.take();

        s.world.castle_set_owner(CastleId(1), Some(WOLVES));
        let out = s.take();
        assert_eq!(s.world.castle(CastleId(1)).unwrap().owner_id, WOLVES);
        assert_eq!(s.world.castle(CastleId(1)).unwrap().former_owner, Some(LIONS));
        assert_eq!((s.clan(LIONS).castle_id, s.clan(WOLVES).castle_id), (0, 1));
        assert_eq!(s.skills_of(P2), [591]);
        assert_eq!(s.skills_of(P3), [590]);
        // The online member's circlet "has disappeared".
        assert!(
            !s.world
                .player(ObjectId(P2))
                .unwrap()
                .data
                .items
                .iter()
                .any(|i| i.item_id == GLUDIO_CIRCLET)
        );
        assert!(
            messages_to(&out, P2)
                .iter()
                .any(|m| m.id == SystemMessageId::S1HasDisappeared
                    && m.params == [SystemMessageParam::ItemName(GLUDIO_CIRCLET)])
        );
        let sound = packets_to(&out, P3, "PLAY_SOUND");
        assert_eq!(
            decode::<PlaySound>(&sound[0]),
            PlaySound::with_unknown1(1, "Siege_Victory")
        );
        assert!(!packets_to(&out, P3, "PLEDGE_SHOW_INFO_UPDATE").is_empty());
        // The offline Lions member P1's circlet row, the owner rows: only with the switch.
        let writes = residence_writes(&out);
        if switch {
            assert_eq!(
                writes,
                [
                    ResidenceWrite::DeleteCirclets {
                        owner_id: P1,
                        item_id: GLUDIO_CIRCLET
                    },
                    ResidenceWrite::CastleOwner {
                        castle_id: 1,
                        clan_id: WOLVES
                    },
                ]
            );
        } else {
            assert!(writes.is_empty());
        }
    }
}

/// `Castle.removeOwner(clan)`: nobody owns the castle, the clan loses it and its functions go.
#[test]
fn removing_the_castle_owner_drops_the_functions() {
    let mut s = Scene::new(true);
    let mut load = standard_load();
    load.castles[0].functions = vec![FunctionRecord {
        function_type: 2,
        lvl: 40,
        lease: 1000,
        rate: 86_400_000,
        end_time: i64::MAX / 2,
    }];
    s.load(load);
    assert!(
        s.world
            .residence_function(ResidenceRef::Castle(CastleId(1)), 2)
            .is_some()
    );
    s.world.castle_remove_owner(CastleId(1), Some(LIONS));
    let out = s.take();
    assert_eq!(s.world.castle(CastleId(1)).unwrap().owner_id, 0);
    assert_eq!(s.clan(LIONS).castle_id, 0);
    assert!(
        s.world
            .residence_function(ResidenceRef::Castle(CastleId(1)), 2)
            .is_none()
    );
    let writes = residence_writes(&out);
    assert!(writes.contains(&ResidenceWrite::CastleOwner {
        castle_id: 1,
        clan_id: 0
    }));
    assert!(writes.contains(&ResidenceWrite::FunctionDelete {
        residence: ResidenceRef::Castle(CastleId(1)),
        function_type: 2
    }));
}

/// `Clan.removeClanMember`: the member who leaves loses the residential skills (and the castle's
/// circlet, `RemoveCastleCirclets`).
#[test]
fn a_member_who_leaves_loses_the_residential_skills() {
    let mut s = Scene::new(false);
    s.load(standard_load());
    s.enter(pdata(P1, LIONS, X));
    s.enter(pdata(P2, LIONS, X));
    assert_eq!(s.skills_of(P2), [590, 591]);
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(P2).unwrap()),
        input: ClientInput::Clan(ClanInput::WithdrawalPledge),
    });
    assert!(s.skills_of(P2).is_empty());
}

// ---------------------------------------------------------------------------------- tax and treasury

/// `Castle.addToTreasury`: Schuttgart and Goddard give Rune its tax rate of the income, the castles
/// other than Aden, Rune, Schuttgart and Goddard give Aden its rate; `addToTreasuryNoTax` stops at
/// `MaxAdena` and refuses to go below 0.
#[test]
fn the_treasury_shares_the_income_with_rune_and_aden() {
    let mut s = Scene::new(true);
    s.world.load_clans(
        vec![ClanRecord {
            clan_id: 700,
            data: Some(ClanDataRecord {
                name: Some("Owls".to_owned()),
                ..ClanDataRecord::default()
            }),
            ..ClanRecord::default()
        }],
        Vec::new(),
    );
    s.load(ResidenceLoad {
        castles: vec![
            castle(1, "Gludio", LIONS, 10),
            castle(5, "Aden", WOLVES, 20),
            castle(8, "Rune", 700, 10),
            castle(9, "Schuttgart", LIONS, 0),
        ],
        ..ResidenceLoad::default()
    });
    let treasury = |s: &Scene, id: i32| s.world.castle(CastleId(id)).unwrap().treasury;

    s.world.castle_add_to_treasury(CastleId(9), 1000);
    // `(long) (1000 * 0.1)` to Rune, the rest stays; Schuttgart pays no Aden tax.
    assert_eq!((treasury(&s, 9), treasury(&s, 8), treasury(&s, 5)), (900, 100, 0));
    s.world.castle_add_to_treasury(CastleId(1), 1001);
    // `(long) (1001 * 0.2)` = 200 to Aden.
    assert_eq!((treasury(&s, 1), treasury(&s, 5)), (801, 200));
    let writes = residence_writes(&s.take());
    assert_eq!(
        writes,
        [
            ResidenceWrite::CastleTreasury {
                castle_id: 8,
                treasury: 100
            },
            ResidenceWrite::CastleTreasury {
                castle_id: 9,
                treasury: 900
            },
            ResidenceWrite::CastleTreasury {
                castle_id: 5,
                treasury: 200
            },
            ResidenceWrite::CastleTreasury {
                castle_id: 1,
                treasury: 801
            },
        ]
    );
    // Withdrawals: not more than there is.
    assert!(
        !s.world
            .castle_add_to_treasury_no_tax(CastleId(1), -802, Writes::Player)
    );
    assert!(
        s.world
            .castle_add_to_treasury_no_tax(CastleId(1), -800, Writes::Player)
    );
    assert_eq!(treasury(&s, 1), 1);
    // `MaxAdena` (99,900,000,000 by default).
    s.world
        .castle_add_to_treasury_no_tax(CastleId(1), i64::MAX - 10, Writes::Player);
    assert_eq!(treasury(&s, 1), 99_900_000_000);
    // A castle without an owner takes nothing.
    s.world.castle_set_owner(CastleId(9), None);
    assert!(
        !s.world
            .castle_add_to_treasury_no_tax(CastleId(9), 5, Writes::Player)
    );
}

/// D-51: without the switch the tax income changes the treasury in this world only.
#[test]
fn the_tax_income_is_written_only_with_the_switch() {
    let mut s = Scene::new(false);
    s.load(standard_load());
    s.world.castle_add_to_treasury(CastleId(1), 500);
    assert_eq!(s.world.castle(CastleId(1)).unwrap().treasury, 500);
    assert!(residence_writes(&s.take()).is_empty());
    // The castle lord's own deposit and the tax rate are player actions.
    s.world
        .castle_add_to_treasury_no_tax(CastleId(1), 100, Writes::Player);
    s.world.castle_set_tax_percent(CastleId(1), 15);
    assert!((s.world.castle(CastleId(1)).unwrap().tax_rate - 0.15).abs() < f64::EPSILON);
    assert_eq!(
        residence_writes(&s.take()),
        [
            ResidenceWrite::CastleTreasury {
                castle_id: 1,
                treasury: 600
            },
            ResidenceWrite::CastleTaxPercent {
                castle_id: 1,
                tax_percent: 15
            },
        ]
    );
}

// ---------------------------------------------------------------------------------- functions

/// `FunctionTask`: a fee that is due is paid from the clan warehouse, the row is written and the
/// next fee comes after `rate`; a warehouse that cannot pay loses the function.
#[test]
fn a_due_fee_is_paid_from_the_clan_warehouse_or_the_function_goes() {
    let mut s = Scene::new(true);
    let mut load = standard_load();
    load.castles[0].functions = vec![
        FunctionRecord {
            function_type: 2,
            lvl: 40,
            lease: 4000,
            rate: 1000,
            end_time: 0,
        },
        FunctionRecord {
            function_type: 3,
            lvl: 40,
            lease: 7000,
            rate: 1000,
            end_time: 0,
        },
    ];
    // The Wolves' rent is paid for a long time (M7-CH-01: it runs on the same ticks).
    load.clan_halls[0].paid_until = i64::MAX / 2;
    s.load(load);
    // Both are due at once: type 2 pays 4000 (10,000 left: 6000), type 3 cannot pay 7000.
    s.run_ms(50);
    let out = s.take();
    let castle = ResidenceRef::Castle(CastleId(1));
    assert!(s.world.residence_function(castle, 2).is_some());
    assert!(s.world.residence_function(castle, 3).is_none());
    assert_eq!(s.clan(LIONS).warehouse[0].count, 6000);
    let writes = residence_writes(&out);
    assert!(matches!(
        writes[0],
        ResidenceWrite::FunctionSave {
            function_type: 2,
            lvl: 40,
            lease: 4000,
            rate: 1000,
            ..
        }
    ));
    assert_eq!(
        writes[1],
        ResidenceWrite::FunctionDelete {
            residence: castle,
            function_type: 3
        }
    );
    assert!(
        matches!(&item_row_ops(&out)[..], [ItemRowOp::Update { owner_id: LIONS, item }] if item.count == 6000)
    );
    // One second later the next fee: 2000 left; then 4000 cannot be paid again.
    s.run_ms(1000);
    assert_eq!(s.clan(LIONS).warehouse[0].count, 2000);
    s.run_ms(1000);
    assert!(s.world.residence_function(castle, 2).is_none());
}

/// D-51: without the switch the warehouse fee is Sieghardt's; the task only moves the time on.
#[test]
fn without_the_switch_a_fee_charges_nothing() {
    let mut s = Scene::new(false);
    let mut load = standard_load();
    load.castles[0].functions = vec![FunctionRecord {
        function_type: 2,
        lvl: 40,
        lease: 70_000,
        rate: 1000,
        end_time: 0,
    }];
    s.load(load);
    s.run_ms(50);
    let castle = ResidenceRef::Castle(CastleId(1));
    let function = s.world.residence_function(castle, 2).unwrap();
    assert!(function.end_time > 0);
    assert_eq!(s.clan(LIONS).warehouse[0].count, 10_000);
    let out = s.take();
    assert!(residence_writes(&out).is_empty());
    assert!(item_row_ops(&out).is_empty());
}

/// `updateFunctions`: the player pays the lease; the first run (paid by the player, `cwh` false)
/// writes the row without charging the warehouse, like any player action, even without the
/// switch. A lower lease changes the function in place; level 0 and lease 0 stops it.
#[test]
fn buying_a_function_writes_its_row() {
    let mut s = Scene::new(false);
    s.load(standard_load());
    let mut p1 = pdata(P1, LIONS, X);
    p1.items.push(ItemData {
        object_id: ObjectId(0x6000_3000),
        item_id: ADENA,
        count: 100_000,
        loc: "INVENTORY".to_owned(),
        ..Default::default()
    });
    s.enter(p1);
    s.take();
    let castle = ResidenceRef::Castle(CastleId(1));
    // Not enough adena: nothing ("You do not have enough adena").
    assert!(
        !s.world
            .residence_update_functions(ObjectId(P1), castle, 4, 1, 200_000, 1000, true)
    );
    assert!(s.world.residence_function(castle, 4).is_none());
    assert!(
        s.world
            .residence_update_functions(ObjectId(P1), castle, 4, 1, 1000, 86_400_000, true)
    );
    assert_eq!(s.world.adena_of(ObjectId(P1)), 99_000);
    s.run_ms(50);
    let writes = residence_writes(&s.take());
    assert!(matches!(
        writes[..],
        [ResidenceWrite::FunctionSave {
            function_type: 4,
            lvl: 1,
            lease: 1000,
            rate: 86_400_000,
            ..
        }]
    ));
    assert!(s.world.residence_function(castle, 4).unwrap().cwh);
    assert_eq!(s.clan(LIONS).warehouse[0].count, 10_000);
    // A lower lease changes the object and writes its row at once.
    assert!(
        s.world
            .residence_update_functions(ObjectId(P1), castle, 4, 2, 500, 86_400_000, false)
    );
    assert_eq!(s.world.residence_function(castle, 4).unwrap().lvl, 2);
    assert!(matches!(
        residence_writes(&s.take())[..],
        [ResidenceWrite::FunctionSave {
            lvl: 2,
            lease: 500,
            ..
        }]
    ));
    // Level 0 and lease 0 stops it (a player's removal is written).
    assert!(
        s.world
            .residence_update_functions(ObjectId(P1), castle, 4, 0, 0, 0, false)
    );
    assert!(s.world.residence_function(castle, 4).is_none());
    assert_eq!(
        residence_writes(&s.take()),
        [ResidenceWrite::FunctionDelete {
            residence: castle,
            function_type: 4
        }]
    );
}

/// NOTE(java-quirk): a higher lease makes a new function object (end time -1); the old object
/// keeps its fee task, charges the clan warehouse and writes its row over the new one.
#[test]
fn a_replaced_function_keeps_charging_like_java() {
    let mut s = Scene::new(true);
    s.load(standard_load());
    let mut p1 = pdata(P1, LIONS, X);
    p1.items.push(ItemData {
        object_id: ObjectId(0x6000_3000),
        item_id: ADENA,
        count: 100_000,
        loc: "INVENTORY".to_owned(),
        ..Default::default()
    });
    s.enter(p1);
    let castle = ResidenceRef::Castle(CastleId(1));
    assert!(
        s.world
            .residence_update_functions(ObjectId(P1), castle, 4, 1, 1000, 1000, true)
    );
    s.run_ms(50);
    assert!(
        s.world
            .residence_update_functions(ObjectId(P1), castle, 4, 3, 3000, 1000, false)
    );
    s.run_ms(50);
    s.take();
    assert_eq!(s.clan(LIONS).warehouse[0].count, 10_000);
    // One second later both objects pay: the old one 1000 (and writes lease 1000), the new one 3000.
    s.run_ms(1000);
    assert_eq!(s.clan(LIONS).warehouse[0].count, 6000);
    let leases: Vec<i32> = residence_writes(&s.take())
        .into_iter()
        .filter_map(|w| match w {
            ResidenceWrite::FunctionSave { lease, .. } => Some(lease),
            _ => None,
        })
        .collect();
    assert_eq!(leases, [1000, 3000]);
    assert_eq!(s.world.residence_function(castle, 4).unwrap().lease, 3000);
}

// ---------------------------------------------------------------------------------- clan halls

/// `ClanHallZone.onEnter`: the `CLAN_HALL` flag and `AgitDecoInfo` of the hall (grade 2: HP 260 is
/// high, teleport 1 is low).
#[test]
fn entering_a_clan_hall_shows_its_decorations() {
    let mut s = Scene::new(false);
    s.load(standard_load());
    s.enter(pdata(P3, WOLVES, HALL_X));
    let out = s.take();
    assert!(s.world.is_inside_zone(ObjectId(P3), ZoneId::ClanHall));
    let deco = packets_to(&out, P3, "AGIT_DECO_INFO");
    assert_eq!(
        decode::<AgitDecoInfo>(&deco[0]),
        AgitDecoInfo {
            hall_id: 36,
            fireplace: 2,
            mirror: 1,
            ..AgitDecoInfo::default()
        }
    );
    assert_eq!(s.world.agit_deco_info(ClanHallId(21)), None);
}

/// `ClanHallTable.setOwner` / `setFree`: the clan's hideout id follows, the rows only with the
/// switch; `AuctionableHall.free()` keeps the old paid values in the row (Java's order).
#[test]
fn a_hall_changes_hands_with_its_hideout_id() {
    let mut s = Scene::new(true);
    s.load(standard_load());
    s.world.clan_hall_table_set_free(ClanHallId(36));
    assert_eq!(s.clan(WOLVES).hideout_id, 0);
    assert!(
        s.world
            .residence_functions(ResidenceRef::ClanHall(ClanHallId(36)))
            .is_empty()
    );
    let writes = residence_writes(&s.take());
    assert_eq!(
        writes.last(),
        Some(&ResidenceWrite::ClanHall {
            hall_id: 36,
            owner_id: 0,
            paid_until: 1_700_000_000_000,
            paid: true
        })
    );
    let hall = s.world.clan_hall(ClanHallId(36)).unwrap();
    assert!(matches!(
        hall.kind,
        ClanHallKind::Auctionable {
            paid_until: 0,
            paid: false,
            ..
        }
    ));
    s.world.clan_hall_table_set_owner(ClanHallId(36), LIONS);
    assert_eq!(s.clan(LIONS).hideout_id, 36);
    assert_eq!(s.world.clan_hall(ClanHallId(36)).unwrap().owner_id, LIONS);
    assert!(matches!(
        residence_writes(&s.take())[..],
        [ResidenceWrite::ClanHall {
            hall_id: 36,
            owner_id: LIONS,
            ..
        }]
    ));
}

/// `EnterWorld`: an unpaid hall reminds the member of the rent (the lease as the number).
#[test]
fn an_unpaid_hall_reminds_the_members() {
    let mut s = Scene::new(false);
    let mut load = standard_load();
    load.clan_halls[0].paid = false;
    s.load(load);
    s.enter(pdata(P3, WOLVES, X));
    let out = s.take();
    assert!(messages_to(&out, P3).iter().any(|m| m.id
        == SystemMessageId::PaymentForYourClanHallHasNotBeenMadePleaseMakePaymentToYourClanWarehouseByS1Tomorrow
        && m.params == [SystemMessageParam::IntNumber(500_000)]));
}

// ---------------------------------------------------------------------------------- fortresses

/// `Fort.setOwner(clan, true)`: the old owner loses it (and `LooseFortPoints`), the new one gets
/// `TakeFortPoints`, the fortress id, the flag stays up, everybody hears who won.
#[test]
fn a_fortress_changes_hands() {
    let mut s = Scene::new(true);
    s.load(standard_load());
    s.enter(pdata(P3, WOLVES, X));
    s.take();
    let before = s.clan(WOLVES).reputation_score;
    assert!(s.world.fort_set_owner(FortId(101), WOLVES, true));
    let out = s.take();
    assert_eq!(s.world.fort(FortId(101)).unwrap().owner_id, WOLVES);
    assert_eq!((s.clan(LIONS).fort_id, s.clan(WOLVES).fort_id), (0, 101));
    assert_eq!(s.clan(WOLVES).reputation_score, before + 200);
    assert_eq!(s.skills_of(P3), [591]);
    assert!(messages_to(&out, P3).iter().any(|m| m.id
        == SystemMessageId::S1IsVictoriousInTheFortressBattleOfS2
        && m.params
            == [
                SystemMessageParam::Text("Wolves".to_owned()),
                SystemMessageParam::CastleName(101)
            ]));
    let writes = residence_writes(&out);
    assert!(writes.contains(&ResidenceWrite::FortState {
        fort_id: 101,
        state: 0,
        castle_id: 0
    }));
    assert!(writes.iter().any(|w| matches!(
        w,
        ResidenceWrite::FortOwner {
            fort_id: 101,
            owner: WOLVES,
            ..
        }
    )));
    // A clan with a castle does not keep a fortress (the rebel army).
    assert!(!s.world.fort_set_owner(FortId(101), LIONS, true));
}

// ---------------------------------------------------------------------------------- re-reads

/// D-50, D-51: at `CharacterSelect` the session read the clan's residences; the world follows the
/// database (another castle, a lost fortress, a new hall).
#[test]
fn the_character_select_read_moves_the_clan_to_what_the_database_says() {
    let mut s = Scene::new(false);
    s.load(standard_load());
    let mut p1 = pdata(P1, LIONS, X);
    p1.profile.residence_refresh = Some(Box::new(ResidenceRefresh {
        clan_id: LIONS,
        castle_id: 2,
        castle: Some(castle(2, "Dion", LIONS, 25)),
        forts: Vec::new(),
        clan_halls: vec![hall(37, LIONS, 1, vec![])],
        siegable_halls: Vec::new(),
    }));
    s.enter(p1);
    assert_eq!(s.world.castle(CastleId(1)).unwrap().owner_id, 0);
    let dion = s.world.castle(CastleId(2)).unwrap();
    assert_eq!((dion.owner_id, dion.tax_percent), (LIONS, 25));
    assert_eq!(s.clan(LIONS).castle_id, 2);
    assert_eq!(s.world.fort(FortId(101)).unwrap().owner_id, 0);
    assert_eq!(s.clan(LIONS).fort_id, 0);
    assert_eq!(s.clan(LIONS).hideout_id, 37);
    // No rows: the database already says so.
    assert!(residence_writes(&s.take()).is_empty());
    // Dion gives no residence skill here; Gludio's and Shanty's are gone with them.
    assert!(s.skills_of(P1).is_empty());
}

/// `request_residence_reread`: the request goes out, the answer replaces the castle row; a failed
/// read changes nothing.
#[test]
fn a_reread_answer_replaces_the_rows() {
    let mut s = Scene::new(false);
    s.load(standard_load());
    s.world
        .request_residence_reread(ResidenceRef::Castle(CastleId(1)), ResidenceReadThen::Nothing);
    let out = s.take();
    assert!(out.iter().any(|o| matches!(
        o,
        Out::Persist(PersistCommand::ResidenceRereadRequest { residence_id: 1, .. })
    )));
    s.world.handle(WorldCommand::ResidenceReread {
        result: Err(()),
        then: ResidenceReadThen::Nothing,
    });
    assert_eq!(s.world.castle(CastleId(1)).unwrap().tax_percent, 10);
    let mut row = castle(1, "Gludio", WOLVES, 5);
    row.treasury = 777;
    s.world.handle(WorldCommand::ResidenceReread {
        result: Ok(ResidenceRereadRows::Castle(row)),
        then: ResidenceReadThen::Nothing,
    });
    let gludio = s.world.castle(CastleId(1)).unwrap();
    assert_eq!(
        (gludio.owner_id, gludio.tax_percent, gludio.treasury),
        (WOLVES, 5, 777)
    );
    assert_eq!((s.clan(LIONS).castle_id, s.clan(WOLVES).castle_id), (0, 1));
    assert!(residence_writes(&s.take()).is_empty());
}

/// `ClanTable.destroyClan`: the fortress of a dissolved clan is free again (written with the clan's
/// rows).
#[test]
fn a_destroyed_clan_frees_its_fortress() {
    let mut s = Scene::new(false);
    s.load(standard_load());
    s.world.destroy_clan(LIONS, true);
    assert_eq!(s.world.fort(FortId(101)).unwrap().owner_id, 0);
    let writes = residence_writes(&s.take());
    assert!(writes.iter().any(|w| matches!(
        w,
        ResidenceWrite::FortOwner {
            fort_id: 101,
            owner: 0,
            ..
        }
    )));
}

/// M7-FU-02 (D-51): a castle that has its owner in this world only (a GM test siege without
/// `SiegeDatabaseWrites`) gets no function row: the database thinks it is free. The purchase itself
/// works in this world; with the switch the owner row went out and the function row follows.
#[test]
fn a_function_of_an_owner_that_exists_only_in_this_world_writes_no_row() {
    for switch in [false, true] {
        let mut s = Scene::new(switch);
        s.load(standard_load());
        let mut p1 = pdata(P1, LIONS, X);
        p1.items.push(ItemData {
            object_id: ObjectId(0x6000_3000),
            item_id: ADENA,
            count: 100_000,
            loc: "INVENTORY".to_owned(),
            ..Default::default()
        });
        s.enter(p1);
        let dion = ResidenceRef::Castle(CastleId(2));
        s.world.castle_set_owner(CastleId(2), Some(LIONS));
        assert_eq!(s.world.residence_owner_world_only(dion), !switch);
        // The startup owner of Gludio came from the database.
        assert!(
            !s.world
                .residence_owner_world_only(ResidenceRef::Castle(CastleId(1)))
        );
        s.take();
        assert!(
            s.world
                .residence_update_functions(ObjectId(P1), dion, 4, 1, 1000, 86_400_000, true)
        );
        s.run_ms(50);
        assert!(s.world.residence_function(dion, 4).is_some());
        let writes = residence_writes(&s.take());
        let saves = writes
            .iter()
            .filter(|w| matches!(w, ResidenceWrite::FunctionSave { .. }))
            .count();
        assert_eq!(saves, usize::from(switch), "switch {switch}: {writes:?}");
        // A removal by the player is not written either.
        assert!(
            s.world
                .residence_update_functions(ObjectId(P1), dion, 4, 0, 0, 0, false)
        );
        let deletes = residence_writes(&s.take())
            .iter()
            .filter(|w| matches!(w, ResidenceWrite::FunctionDelete { .. }))
            .count();
        assert_eq!(deletes, usize::from(switch), "switch {switch}");
    }
}
