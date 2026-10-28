#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Territory War (M7-TW-01): the territories at startup, the next war date, the countdown and
//! the war, the registrations and their re-read (D-50), the `SiegeDatabaseWrites` switch and the
//! side by side re-read (D-51), the wards and their carriers, the outposts and flags, the
//! rewards, the Territory Managers, clan level 11, `//territory_war*`, the disguise scroll and the
//! battlefield channel.
//! Java: managers/TerritoryWarManager.java, mechanics/siege/TerritoryWard.java,
//! entity/actor/instance/TerritoryWard.java, network/clientpackets/{RequestJoinDominionWar,
//! RequestDominionInfo}.java, network/serverpackets/{ExShowDominionRegistry,ExReplyDominionInfo,
//! ExShowOwnthingPos}.java, dist/game/data/scripts/ai/others/TerritoryManagers,
//! quests/TerritoryWarScripts/TerritoryWarSuperClass.java (the super class instance),
//! handlers/skill/effects/{OutpostCreate,OutpostDestroy,TakeTerritoryFlag}.java,
//! handlers/chat/commands/admin/AdminTerritoryWar.java, handlers/items/Disguise.java,
//! handlers/chat/channels/ChatBattlefield.java, handlers/bypass/npc/TerritoryStatus.java,
//! entity/clan/Clan.java (levelUpClan). The expected values come from the Java code named in each
//! test.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates, KarmaLoss,
    MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates, SkillTrees,
    SpawnData, SpawnLoadOptions, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::territory::{ExReplyDominionInfo, ExShowDominionRegistry, ExShowOwnthingPos};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord, ClanSettings};
use l2_world::residence::{CastleRecord, ResidenceReadKind, ResidenceReadThen, ResidenceRereadRows};
use l2_world::territory::{
    TerritoryRecord, TerritoryRegistrationRecord, TerritoryRows, TerritorySpawnRecord, TerritoryThen,
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

/// A player template of `class_id`.
fn class_xml(class_id: i32) -> String {
    format!(
        r#"<list><classId>{class_id}</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData><level val="1"><hp>5000</hp><mp>50</mp><cp>20</cp></level></lvlUpgainData></list>"#
    )
}

/// The ward skills of Gludio and Dion (`Territory Benefaction`).
const SKILLS: &str = r#"<list>
<skill id="848" levels="1" name="Gludio Territory Benefaction"><operateType>P</operateType></skill>
<skill id="849" levels="1" name="Dion Territory Benefaction"><operateType>P</operateType></skill>
</list>"#;

const TREES: &str = r#"<list>
<skillTree type="pledgeSkillTree">
  <skill skillName="Gludio Territory Benefaction" skillId="848" skillLevel="1" getLevel="5" residenceSkill="true">
    <socialClass>APPRENTICE</socialClass><residenceId>81</residenceId></skill>
  <skill skillName="Dion Territory Benefaction" skillId="849" skillLevel="1" getLevel="5" residenceSkill="true">
    <socialClass>APPRENTICE</socialClass><residenceId>82</residenceId></skill>
</skillTree>
</list>"#;

const ADENA: i32 = 57;
const GLUDIO_WARD_ITEM: i32 = 13560;
const DION_WARD_ITEM: i32 = 13561;
const GLUDIO_BADGE: i32 = 13757;
const GLUDIO_DISGUISE: i32 = 13677;
const DION_DISGUISE: i32 = 13678;

fn items_xml() -> String {
    let ward = |id: i32, name: &str| {
        format!(
            r#"<item id="{id}" type="Weapon" name="{name}"><set name="weapon_type" val="OWNTHING" /><set name="bodypart" val="lrhand" />
  <set name="weight" val="500" /><set name="is_dropable" val="false" /><set name="is_destroyable" val="false" />
  <set name="isForceEquip" val="true" /></item>"#
        )
    };
    let disguise = |id: i32, name: &str| {
        format!(
            r#"<item id="{id}" type="EtcItem" name="{name}"><set name="is_stackable" val="true" /><set name="handler" val="Disguise" /></item>"#
        )
    };
    format!(
        r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="13757" type="EtcItem" name="Gludio Territory Badge"><set name="is_stackable" val="true" /></item>
{}{}{}{}
</list>"#,
        ward(GLUDIO_WARD_ITEM, "Gludio Ward"),
        ward(DION_WARD_ITEM, "Dion Ward"),
        disguise(GLUDIO_DISGUISE, "Gludio Disguise Scroll"),
        disguise(DION_DISGUISE, "Dion Disguise Scroll"),
    )
}

fn stackable(item_id: i32) -> bool {
    matches!(item_id, ADENA | GLUDIO_BADGE | GLUDIO_DISGUISE | DION_DISGUISE)
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
            weapon: t.is_weapon(),
        })
    }
    fn skill_is_passive(&self, _skill_id: i32) -> bool {
        true
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
    fn admin_rights(&self) -> Option<&AdminRights> {
        Some(&self.rights)
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        stackable(item_id)
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
}

/// Gludio's siege zone is around (X, Y), Dion's around (DX, Y); a peace zone lies between.
const X: i32 = -18_000;
const Y: i32 = 110_000;
const Z: i32 = -2_500;
const DX: i32 = X + 60_000;
const PEACE_X: i32 = X + 30_000;
/// Lions own Gludio: P1 (leader) and P2; Wolves own Dion: P3 (leader); Bears registered for
/// Gludio: P5 (leader) and P6; P7 a mercenary registered for Dion; P8 a level 30 player; P9 a GM.
const P1: i32 = 0x6000_0001;
const P2: i32 = 0x6000_0002;
const P3: i32 = 0x6000_0003;
const P5: i32 = 0x6000_0005;
const P6: i32 = 0x6000_0006;
const P7: i32 = 0x6000_0007;
const P8: i32 = 0x6000_0008;
const GM: i32 = 0x6000_0009;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
const BEARS: i32 = 700;
/// Gladiator: a second class (`getPlayerClass().level() == 2`).
const SECOND_CLASS: i32 = 2;

/// 2026-10-21 (a Wednesday) 12:34:56.789 UTC, the clock at tick 0.
const NOW: i64 = 1_792_586_096_789;
/// 2026-10-24 (the Saturday of this week) 20:00:00.789: the next war with an owned castle.
const SATURDAY_20: i64 = 1_792_872_000_789;
/// 2026-11-01 22:00:00.789: the Sunday of this week (22:00) is over, two weeks on.
const SUNDAY_22: i64 = 1_793_570_400_789;

const CAPTAIN: i32 = 36_481;
const MANAGER: i32 = 36_490;
const DION_MANAGER: i32 = 36_491;
const CATAPULT: i32 = 36_520;
const GLUDIO_WARD: i32 = 36_572;
const DION_WARD: i32 = 36_573;
const OUTPOST: i32 = 36_590;
const HQ: i32 = 35_062;

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
        npc(CAPTAIN, "Folk", "Mercenary Captain"),
        npc(MANAGER, "Folk", "Territory Manager"),
        npc(DION_MANAGER, "Folk", "Territory Manager"),
        npc(CATAPULT, "Folk", "Catapult"),
        npc(GLUDIO_WARD, "TerritoryWard", "Gludio Ward"),
        npc(DION_WARD, "TerritoryWard", "Dion Ward"),
        npc(OUTPOST, "Folk", "Outpost"),
        npc(HQ, "Folk", "Headquarters"),
    ]
    .join("\n");
    format!(r#"<?xml version="1.0"?><list>{list}</list>"#)
}

fn zones_xml() -> String {
    let cuboid = |x: i32, w: i32| {
        format!(
            r#"shape="Cuboid" minZ="-5000" maxZ="5000"><node X="{}" Y="{}" /><node X="{}" Y="{}" />"#,
            x - w,
            Y - w,
            x + w,
            Y + w
        )
    };
    format!(
        r#"<?xml version="1.0"?>
<list enabled="true">
  <zone name="gludio_castle_battlefield_territory" type="SiegeZone" {}<stat name="castleId" val="1" /></zone>
  <zone name="gludio_castle_headquarter_territory1" type="HqZone" {}<stat name="castleId" val="1" /></zone>
  <zone name="dion_castle_battlefield_territory" type="SiegeZone" {}<stat name="castleId" val="2" /></zone>
  <zone name="peace" type="PeaceZone" {}</zone>
</list>"#,
        cuboid(X, 3000),
        cuboid(X - 1000, 500),
        cuboid(DX, 3000),
        cuboid(PEACE_X, 1000),
    )
}

/// The pages of the Territory Managers; each carries a link that takes any event of the script
/// (`$e`), so the bypass validation accepts the events.
fn manager_pages() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "36490.html",
            r#"MANAGER %objectId%<a action="bypass npc_%objectId%_TerritoryStatus">status</a>"#,
        ),
        ("36491.html", "DION MANAGER"),
        ("36490-02.html", "NO BADGES"),
        ("36490-03.html", "BADGES"),
        ("36490-04.html", "NOBLE %badge%"),
        ("36490-05.html", "ALREADY NOBLE"),
        ("36490-06.html", "LEVEL 75"),
        ("36490-08.html", "MORE TRAINING"),
        ("reward-0a.html", "NOTHING"),
        ("reward-0b.html", "OTHER %castle%"),
        (
            "reward-1.html",
            "REWARD %castle% %badge% %adena% %territoryId% %objectId%",
        ),
        ("reward-2.html", "HERE %objectId%"),
        ("reward-3.html", "LATER"),
    ]
}

struct Options {
    siege_database_writes: bool,
    gludio_owner: i32,
    dion_owner: i32,
    next_tw_start_date: i64,
    settings: TerritorySettings,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            siege_database_writes: true,
            gludio_owner: LIONS,
            dion_owner: WOLVES,
            next_tw_start_date: 0,
            settings: TerritorySettings::default(),
        }
    }
}

struct Scene {
    world: World,
    log: Log,
    /// What `load_territories` sent (the startup's writes).
    startup: Vec<Out>,
    _root: tempfile::TempDir,
}

fn spawn(id: i32, castle_id: i32, npc_id: i32, x: i32, spawn_type: i32) -> TerritorySpawnRecord {
    TerritorySpawnRecord {
        id,
        castle_id,
        npc_id,
        x,
        y: Y,
        z: Z,
        heading: 0,
        spawn_type,
    }
}

fn territory_load(next_tw_start_date: i64) -> TerritoryLoad {
    TerritoryLoad {
        spawns: Some(vec![
            spawn(1, 1, CAPTAIN, X + 66, 0),
            spawn(2, 1, MANAGER, X + 65, 0),
            spawn(3, 1, CATAPULT, X + 400, 2),
            spawn(4, 1, 36_572, X + 500, 3),
            spawn(5, 1, 36_573, X + 520, 3),
            spawn(6, 2, DION_MANAGER, DX + 310, 0),
            spawn(7, 2, 36_573, DX + 500, 3),
            spawn(8, 2, 36_572, DX + 520, 3),
            // A type Java does not know.
            spawn(9, 2, CAPTAIN, DX, 7),
        ]),
        territories: Some(vec![
            TerritoryRecord {
                territory_id: 81,
                castle_id: 1,
                fort_id: 101,
                owned_ward_ids: "81;".into(),
            },
            TerritoryRecord {
                territory_id: 82,
                castle_id: 2,
                fort_id: 103,
                owned_ward_ids: "82;".into(),
            },
        ]),
        registrations: Some(vec![
            TerritoryRegistrationRecord {
                castle_id: 1,
                registered_id: BEARS,
            },
            TerritoryRegistrationRecord {
                castle_id: 2,
                registered_id: P7,
            },
        ]),
        next_tw_start_date,
    }
}

impl Scene {
    fn new(siege_database_writes: bool) -> Self {
        Self::with(Options {
            siege_database_writes,
            ..Options::default()
        })
    }

    fn with(options: Options) -> Self {
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
        write("data/zones/territory.xml", &zones_xml());
        write("data/stats/players/templates/0.xml", &class_xml(0));
        write("data/stats/players/templates/2.xml", &class_xml(SECOND_CLASS));
        let link = r#"<a action="bypass -h Script TerritoryManagers $e">x</a>"#;
        for (file, body) in manager_pages() {
            write(
                &format!("data/scripts/ai/others/TerritoryManagers/{file}"),
                &format!("<html><body>{body}{link}</body></html>"),
            );
        }
        write(
            "data/html/territorystatus.htm",
            "<html><body>STATUS %clanname% %clanleadername% %castlename% %taxpercent% %territory%</body></html>",
        );
        write(
            "data/html/territorynoclan.htm",
            "<html><body>NOCLAN %castlename%</body></html>",
        );
        write(
            "data/html/admin/territorywar.htm",
            "<html><body>TW MENU</body></html>",
        );
        write(
            "data/html/admin/territorywartime.htm",
            "<html><body>TIME %time%</body></html>",
        );
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Recorder(log.clone())));
        let mut rights = AdminRights::default();
        for (level, child) in [(0, 0), (100, 0)] {
            rights.add_level(level, child);
        }
        for command in [
            "territory_war",
            "territory_war_time",
            "territory_war_start",
            "territory_war_end",
            "territory_wards_list",
        ] {
            rights.add_command(command, 100, false);
        }
        let stats = StatData {
            items: Arc::new(ItemTemplates::from_xml(&items_xml()).unwrap()),
            player_templates: Arc::new(PlayerTemplates::load(root.path(), 85)),
            armor_sets: Arc::new(ArmorSets::default()),
            enchant_hp_bonus: Arc::new(EnchantHpBonus::default()),
            hennas: Arc::new(Hennas::default()),
            base_stat_bonus: Arc::new(BaseStatBonus::default()),
            skills: Arc::new(SkillTemplates::from_xml(
                SKILLS,
                &EnchantSkillGroups::default(),
                &SkillLoadOptions::default(),
            )),
        };
        world.set_static_data(Box::new(Data { stats, rights }));
        world.set_exp_data(
            Arc::new(ExperienceTable::from_xml(r#"<table maxLevel="85" maxPetLevel="85"></table>"#).unwrap()),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml(TREES).unwrap()),
        );
        let ids = Arc::new(IdFactory::new(vec![P1, P2, P3, P5, P6, P7, P8, GM]).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        let npcs = Arc::new(NpcTemplates::from_xml(&npc_xml(), &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
        world.set_residence_settings(ResidenceSettings {
            siege_database_writes: options.siege_database_writes,
            ..ResidenceSettings::default()
        });
        world.set_clan_settings(ClanSettings {
            clan_level_requirements: [1; 6],
            ..ClanSettings::default()
        });
        world.load_clans(
            vec![
                clan(LIONS, "Lions", 10, 1, P1, &[P2]),
                clan(WOLVES, "Wolves", 5, 2, P3, &[]),
                clan(BEARS, "Bears", 10, 0, P5, &[P6]),
            ],
            Vec::new(),
        );
        world.load_residences(ResidenceLoad {
            castles: vec![
                castle(1, "Gludio", options.gludio_owner),
                castle(2, "Dion", options.dion_owner),
            ],
            ..ResidenceLoad::default()
        });
        world.set_siege_settings(SiegeSettings {
            clock_override_unix_ms: Some(NOW),
            zone_offset_override_ms: Some(0),
            ..SiegeSettings::default()
        });
        world.load_sieges(SiegeLoad::default());
        world.set_territory_settings(options.settings);
        log.take();
        world.load_territories(territory_load(options.next_tw_start_date));
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
        self.world.tick();
        self.input(id, ClientInput::Appearing);
        self.run_ms(100);
    }

    /// Everybody of the scene online: the Lions, the Wolves' leader and the Bears at Gludio, the
    /// mercenary and the low player at Dion, the GM at Gludio.
    fn enter_all(&mut self) {
        self.enter(pdata(P1, LIONS, X + 50, 76, SECOND_CLASS));
        self.enter(pdata(P2, LIONS, X + 60, 40, SECOND_CLASS));
        self.enter(pdata(P3, WOLVES, X + 70, 76, SECOND_CLASS));
        self.enter(pdata(P5, BEARS, X + 80, 76, SECOND_CLASS));
        self.enter(pdata(P6, BEARS, X + 90, 40, 0));
        self.enter(pdata(P7, 0, DX + 50, 40, SECOND_CLASS));
        self.enter(pdata(P8, 0, DX + 60, 30, SECOND_CLASS));
        let mut gm = pdata(GM, 0, X + 100, 76, SECOND_CLASS);
        gm.access_level = 100;
        self.enter(gm);
        self.take();
    }

    fn input(&mut self, id: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(id).unwrap()),
            input,
        });
    }

    fn admin(&mut self, command: &str) {
        self.world.admin_command(ObjectId(GM), command, false);
    }

    fn take(&mut self) -> Vec<Out> {
        self.log.take()
    }

    fn run_ms(&mut self, ms: u64) {
        for _ in 0..ms.div_ceil(50) {
            self.world.tick();
        }
    }

    /// The siege (and war) clock now.
    fn now(&self) -> i64 {
        NOW + self.world.world_ms()
    }

    /// The NPCs of a template that stand in the world.
    fn npcs_of(&self, npc_id: i32) -> Vec<ObjectId> {
        self.world
            .npc_ids()
            .into_iter()
            .filter(|id| self.world.npc(*id).is_some_and(|n| n.npc_id == npc_id))
            .collect()
    }

    fn npc(&self, npc_id: i32) -> ObjectId {
        *self.npcs_of(npc_id).first().expect("the NPC stands")
    }

    /// A GM starts the war (`//territory_war_start`), which runs a second later.
    fn start_war(&mut self) {
        self.admin("admin_territory_war_start");
        self.run_ms(1100);
        assert!(self.world.is_tw_in_progress());
    }

    /// Answers the last re-read of the war with `rows`.
    fn answer_reread(&mut self, out: &[Out], rows: TerritoryRows) {
        let then = territory_rereads(out).pop().expect("a re-read request");
        self.world.handle(WorldCommand::ResidenceReread {
            result: Ok(ResidenceRereadRows::Territory(Box::new(rows))),
            then: ResidenceReadThen::Territory(then),
        });
    }

    /// The rows as this world has them (Sieghardt changed nothing).
    fn rows_now(&self) -> TerritoryRows {
        let mut registrations = Vec::new();
        for castle in [1, 2] {
            for id in self
                .world
                .territory_registered_clans(castle)
                .into_iter()
                .flatten()
                .chain(
                    self.world
                        .territory_registered_mercenaries(castle)
                        .into_iter()
                        .flatten(),
                )
            {
                registrations.push(TerritoryRegistrationRecord {
                    castle_id: castle,
                    registered_id: id,
                });
            }
        }
        TerritoryRows {
            territories: self
                .world
                .territories()
                .map(|t| TerritoryRecord {
                    territory_id: t.territory_id,
                    castle_id: t.castle_id,
                    fort_id: t.fort_id,
                    owned_ward_ids: t.owned_ward_ids().iter().map(|w| format!("{w};")).collect(),
                })
                .collect(),
            registrations,
            next_tw_start_date: self.world.tw_start_time_ms(),
        }
    }

    /// `RequestJoinDominionWar` and the re-read it asks for, answered with `rows`.
    fn join(
        &mut self,
        player: i32,
        territory_id: i32,
        is_clan: i32,
        is_joining: i32,
        rows: TerritoryRows,
    ) -> Vec<Out> {
        self.take();
        self.input(
            player,
            ClientInput::Territory(TerritoryInput::JoinDominionWar {
                territory_id,
                is_clan,
                is_joining,
            }),
        );
        let asked = self.take();
        assert_eq!(
            territory_rereads(&asked).len(),
            1,
            "the registrations are read again first"
        );
        self.answer_reread(&asked, rows);
        let mut out = asked;
        out.extend(self.take());
        out
    }

    /// The click that opens the NPC's window.
    fn talk(&mut self, player: i32, npc: ObjectId) {
        self.world.set_target(ObjectId(player), Some(npc));
        self.input(
            player,
            ClientInput::Action {
                object_id: npc.0,
                action_id: 0,
            },
        );
        self.run_ms(100);
    }

    fn script(&mut self, player: i32, event: &str) {
        self.input(
            player,
            ClientInput::Bypass {
                command: format!("Script TerritoryManagers {event}"),
            },
        );
    }

    fn skills(&self, player: i32) -> Vec<i32> {
        self.world
            .player(ObjectId(player))
            .unwrap()
            .core
            .skills
            .iter()
            .map(|k| k.0)
            .collect()
    }

    fn item_count(&self, player: i32, item_id: i32) -> i64 {
        self.world
            .player(ObjectId(player))
            .unwrap()
            .data
            .items
            .iter()
            .filter(|i| i.item_id == item_id)
            .map(|i| i.count)
            .sum()
    }

    fn worn(&self, player: i32, item_id: i32) -> bool {
        self.world
            .player(ObjectId(player))
            .unwrap()
            .data
            .items
            .iter()
            .any(|i| i.item_id == item_id && i.loc == "PAPERDOLL")
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

fn clan(clan_id: i32, name: &str, level: i32, castle_id: i32, leader: i32, others: &[i32]) -> ClanRecord {
    let mut members = vec![member(leader, 1)];
    members.extend(others.iter().map(|m| member(*m, 5)));
    ClanRecord {
        clan_id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level,
            reputation_score: 100_000,
            castle_id,
            leader_id: leader,
            ..ClanDataRecord::default()
        }),
        members,
        ..ClanRecord::default()
    }
}

fn castle(id: i32, name: &str, owner_id: i32) -> CastleRecord {
    CastleRecord {
        id,
        found: true,
        name: name.to_owned(),
        tax_percent: 7,
        time_registration_over: true,
        owner_id,
        ..CastleRecord::default()
    }
}

fn pdata(id: i32, clan_id: i32, x: i32, level: i32, class_id: i32) -> PlayerData {
    let mut data = PlayerData {
        object_id: ObjectId(id),
        account: format!("acc{id}"),
        name: format!("P{}", id - 0x6000_0000),
        title: String::new(),
        class_id,
        base_class: class_id,
        race: 0,
        sex: 0,
        face: 0,
        hair_style: 0,
        hair_color: 0,
        level,
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
        cur_hp: 5000.0,
        max_hp: 5000.0,
        cur_mp: 50.0,
        max_mp: 50.0,
        cur_cp: 20.0,
        max_cp: 20.0,
        online_time: 0,
        items: vec![ItemData {
            object_id: ObjectId(id + 0x0100_0000),
            item_id: ADENA,
            count: 1000,
            loc: "INVENTORY".to_owned(),
            ..Default::default()
        }],
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    };
    if clan_id != 0 {
        data.profile.clan = Some(PlayerClanState {
            clan_id,
            ..PlayerClanState::default()
        });
    }
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

/// The text of the last page, without the fixture link.
#[track_caller]
fn last_page(out: &[Out], to: i32) -> String {
    let Some(raw) = packets_to(out, to, "NPC_HTML_MESSAGE").pop() else {
        panic!("no page to {to:#x}");
    };
    let page = decode::<NpcHtmlMessage>(&raw).html;
    let text = page
        .trim_start_matches("<html><body>")
        .trim_end_matches("</body></html>");
    text.split("<a action").next().unwrap().to_owned()
}

fn territory_writes(out: &[Out]) -> Vec<TerritoryWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Residence(ResidenceWrite::Territory(w))) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn territory_rereads(out: &[Out]) -> Vec<TerritoryThen> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::ResidenceRereadRequest {
                residence_id: 0,
                kind: ResidenceReadKind::Territory,
                then: ResidenceReadThen::Territory(then),
            }) => Some(*then),
            _ => None,
        })
        .collect()
}

// ---------------------------------------------------------------------------------- startup

/// `load()`: the town NPCs of the owned territories stand, the ward places hold their wards (not
/// in the world outside the war), the registrations are clans or mercenaries; the
/// `TerritoryWarSuperClass` constructor sets the next date (Saturday 20:00 with an owned castle)
/// and keeps it in the global variable (D-51: only with `SiegeDatabaseWrites`).
#[test]
fn startup_loads_the_territories_and_sets_the_next_war() {
    let s = Scene::new(true);
    // Type 0 NPCs of the owned territories; the castle NPCs (type 2) only in the war.
    assert_eq!(s.npcs_of(CAPTAIN).len(), 1);
    assert_eq!(s.npcs_of(MANAGER).len(), 1);
    assert_eq!(s.npcs_of(DION_MANAGER).len(), 1);
    assert!(s.npcs_of(CATAPULT).is_empty());
    // The wards are kept on their places but not spawned (`decayMe`).
    assert!(s.npcs_of(GLUDIO_WARD).is_empty() && s.npcs_of(DION_WARD).is_empty());
    let gludio = s.world.territory(1).unwrap();
    assert_eq!(gludio.territory_id, 81);
    assert_eq!(gludio.fort_id, 101);
    assert_eq!(gludio.owner_clan, Some(LIONS));
    assert_eq!(gludio.owned_ward_ids(), [81]);
    assert_eq!(gludio.ward_places.len(), 2);
    assert_eq!(s.world.territory(2).unwrap().owned_ward_ids(), [82]);
    // A known clan id is a clan registration, anything else a mercenary.
    assert_eq!(s.world.territory_registered_clans(1), Some(vec![BEARS]));
    assert_eq!(s.world.territory_registered_mercenaries(2), Some(vec![P7]));
    assert_eq!(s.world.territory_registered_clans(2), None);
    // The next war and its global variable.
    assert_eq!(s.world.tw_start_time_ms(), SATURDAY_20);
    assert_eq!(
        territory_writes(&s.startup),
        [TerritoryWrite::NextTwStartDate(SATURDAY_20)]
    );
    assert!(s.world.owned_territories().len() == 2);
    assert!(!s.world.is_tw_in_progress() && !s.world.is_tw_channel_open());

    // D-51: side by side the date is the same, but the variable stays Sieghardt's.
    let s = Scene::new(false);
    assert_eq!(s.world.tw_start_time_ms(), SATURDAY_20);
    assert!(territory_writes(&s.startup).is_empty());
}

/// `setNextTWDate`: a variable that is still ahead is the date; without an owned castle the war is
/// on Sunday 22:00, two weeks on when this week's is over.
#[test]
fn the_next_war_date_follows_the_variable_and_the_castles() {
    let ahead = SATURDAY_20 + 7 * 86_400_000;
    let s = Scene::with(Options {
        next_tw_start_date: ahead,
        ..Options::default()
    });
    assert_eq!(s.world.tw_start_time_ms(), ahead);
    assert!(territory_writes(&s.startup).is_empty());
    // A variable in the past is replaced.
    let s = Scene::with(Options {
        next_tw_start_date: NOW - 1,
        ..Options::default()
    });
    assert_eq!(s.world.tw_start_time_ms(), SATURDAY_20);
    // No castle has an owner: Sunday 22:00, and no town NPC stands.
    let s = Scene::with(Options {
        gludio_owner: 0,
        dion_owner: 0,
        ..Options::default()
    });
    assert_eq!(s.world.tw_start_time_ms(), SUNDAY_22);
    assert!(s.npcs_of(CAPTAIN).is_empty());
    assert_eq!(s.world.territory(1).unwrap().owner_clan, None);
}

// ---------------------------------------------------------------------------------- the war

/// `ScheduleStartTWTask` from 21 minutes before the war: the end of the registration, the
/// channel and the countdown messages (to everybody), the players' sides, then the war: the
/// castle NPCs and the wards stand, the siege zones are active, the participants attack.
#[test]
fn the_countdown_opens_the_channel_and_starts_the_war() {
    let mut s = Scene::new(true);
    s.enter_all();
    let start = s.now() + 21 * 60_000;
    s.world.tw_set_start_time(start);
    s.run_ms(1000);
    let out = s.take();
    assert!(message_ids(&out, P8).contains(&SystemMessageId::TheTerritoryWarRequestPeriodHasEnded));
    assert!(s.world.is_tw_registration_over());
    assert!(!s.world.is_tw_channel_open());
    assert_eq!(s.world.registered_territory_id(ObjectId(P1)), 0);

    // 20 minutes before: the channel opens and the participants get their side.
    s.run_ms(60_000);
    let out = s.take();
    assert!(message_ids(&out, P8).contains(
        &SystemMessageId::TheTerritoryWarWillBeginIn20MinutesTerritoryRelatedFunctionsIEBattlefieldChannelDisguiseScrollsTransformationsEtcCanNowBeUsed
    ));
    assert!(s.world.is_tw_channel_open());
    for (player, side) in [(P1, 81), (P2, 81), (P3, 82), (P5, 81), (P7, 82)] {
        assert_eq!(
            s.world.registered_territory_id(ObjectId(player)),
            side,
            "{player:#x}"
        );
        assert_eq!(s.world.siege_side_of(ObjectId(player)), side, "{player:#x}");
        assert_eq!(s.world.siege_state_of(ObjectId(player)), 0);
    }
    // A first class member of a registered clan gets no side; a level 30 player is no one's.
    assert_eq!(s.world.siege_side_of(ObjectId(P6)), 0);
    assert_eq!(s.world.registered_territory_id(ObjectId(P8)), 0);

    // 10, 5 and 1 minutes before, then the war.
    s.run_ms(10 * 60_000);
    assert!(message_ids(&s.take(), P8).contains(&SystemMessageId::TheTerritoryWarBeginsIn10Minutes));
    s.run_ms(5 * 60_000);
    assert!(message_ids(&s.take(), P8).contains(&SystemMessageId::TheTerritoryWarBeginsIn5Minutes));
    s.run_ms(4 * 60_000);
    assert!(message_ids(&s.take(), P8).contains(&SystemMessageId::TheTerritoryWarBeginsIn1Minute));
    assert!(!s.world.is_tw_in_progress());
    s.run_ms(60_000);
    let out = s.take();
    assert!(s.world.is_tw_in_progress());
    assert!(message_ids(&out, P8).contains(&SystemMessageId::TerritoryWarHasBegun));
    // The attackers of the war: the relation of a registered player is the territory bit.
    assert_eq!(s.world.siege_state_of(ObjectId(P1)), 1);
    assert_eq!(s.world.user_info_relation(ObjectId(P1)), 0x40 | 0x1000);
    // The castle NPCs of the war and the wards on their places.
    assert_eq!(s.npcs_of(CATAPULT).len(), 1);
    assert_eq!(s.npcs_of(GLUDIO_WARD).len(), 1);
    assert_eq!(s.npcs_of(DION_WARD).len(), 1);
    assert_eq!(s.world.territory_wards().len(), 2);
    assert_eq!(s.world.territory_wards()[0].owner_castle_id, 1);
    // The siege zone of Gludio is active with the war as its siege.
    let zone = s.world.castle(CastleId(1)).unwrap().siege_zone.unwrap();
    assert!(s.world.is_siege_zone_active(zone));
    assert!(s.world.is_in_siege(ObjectId(P1)));
}

/// `ScheduleEndTWTask` and `endTerritoryWar`: the countdown to the participants, the end (the
/// wards and castle NPCs go, the zones close, the registration rows go, the next date), then the
/// channel closes ten minutes later.
#[test]
fn the_war_ends_and_the_channel_closes() {
    let mut s = Scene::with(Options {
        settings: TerritorySettings {
            war_length_ms: 15 * 60_000,
            ..TerritorySettings::default()
        },
        ..Options::default()
    });
    s.enter_all();
    s.start_war();
    // The first end task (a second after the start): 14 minutes and some seconds are left.
    s.run_ms(1000);
    let out = s.take();
    let ends: Vec<SystemMessage> = messages_to(&out, P1)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::TheTerritoryWarWillEndInS1MinuteS)
        .collect();
    assert_eq!(ends.len(), 1);
    assert_eq!(ends[0].params, [SystemMessageParam::IntNumber(14)]);
    // Not a participant: P8 hears nothing of it.
    assert!(!message_ids(&out, P8).contains(&SystemMessageId::TheTerritoryWarWillEndInS1MinuteS));
    s.run_ms(15 * 60_000);
    let out = s.take();
    assert!(message_ids(&out, P1).contains(&SystemMessageId::S1SecondSToTheEndOfTerritoryWar));
    assert!(message_ids(&out, P8).contains(&SystemMessageId::TerritoryWarHasEnded));
    assert!(!s.world.is_tw_in_progress());
    assert!(s.world.territory_wards().is_empty());
    assert!(s.npcs_of(GLUDIO_WARD).is_empty() && s.npcs_of(CATAPULT).is_empty());
    let zone = s.world.castle(CastleId(1)).unwrap().siege_zone.unwrap();
    assert!(!s.world.is_siege_zone_active(zone));
    // The registration rows go (Java keeps its lists in memory).
    let writes = territory_writes(&out);
    assert!(writes.contains(&TerritoryWrite::RegistrationDelete {
        castle_id: 1,
        registered_id: BEARS,
    }));
    assert!(writes.contains(&TerritoryWrite::RegistrationDelete {
        castle_id: 2,
        registered_id: P7,
    }));
    assert_eq!(s.world.territory_registered_clans(1), Some(vec![BEARS]));
    // The next war: the variable is ahead (Saturday 20:00 of this week).
    assert_eq!(s.world.tw_start_time_ms(), SATURDAY_20);
    // The attackers are no attackers any more, their side stays with the channel.
    assert_eq!(s.world.siege_state_of(ObjectId(P1)), 0);
    assert_eq!(s.world.siege_side_of(ObjectId(P1)), 81);
    assert!(s.world.is_tw_channel_open());
    s.run_ms(10 * 60_000);
    assert!(!s.world.is_tw_channel_open());
    assert_eq!(s.world.siege_side_of(ObjectId(P1)), 0);
}

/// D-51: side by side the schedule runs (the messages, the channel) but the war is Sieghardt's;
/// once its channel closed the rows are read again and the schedule starts from the date read.
#[test]
fn side_by_side_the_war_is_read_again_afterwards() {
    let mut s = Scene::with(Options {
        siege_database_writes: false,
        settings: TerritorySettings {
            war_length_ms: 60_000,
            ..TerritorySettings::default()
        },
        ..Options::default()
    });
    s.enter_all();
    let start = s.now() + 61_000;
    s.world.tw_set_start_time(start);
    s.run_ms(1000);
    assert!(message_ids(&s.take(), P8).contains(&SystemMessageId::TheTerritoryWarBeginsIn1Minute));
    assert!(s.world.is_tw_channel_open());
    s.run_ms(61_000);
    let out = s.take();
    assert!(!s.world.is_tw_in_progress());
    assert!(!message_ids(&out, P8).contains(&SystemMessageId::TerritoryWarHasBegun));
    assert!(s.npcs_of(GLUDIO_WARD).is_empty());
    assert!(territory_rereads(&out).is_empty());
    // The war (a minute) and its channel (ten more) are over: the channel closes, the rows are
    // read again.
    s.run_ms(11 * 60_000);
    let out = s.take();
    assert!(!s.world.is_tw_channel_open());
    assert_eq!(s.world.siege_side_of(ObjectId(P1)), 0);
    assert_eq!(territory_rereads(&out), [TerritoryThen::RestartSchedule]);
    // Sieghardt's war: the Wolves took Gludio's ward, the registrations are gone, a new date.
    let next = SATURDAY_20 + 14 * 86_400_000;
    let rows = TerritoryRows {
        territories: vec![
            TerritoryRecord {
                territory_id: 81,
                castle_id: 1,
                fort_id: 101,
                owned_ward_ids: String::new(),
            },
            TerritoryRecord {
                territory_id: 82,
                castle_id: 2,
                fort_id: 103,
                owned_ward_ids: "82;81;".into(),
            },
        ],
        registrations: vec![],
        next_tw_start_date: next,
    };
    // The Lions lose their ward skill, the Wolves get Gludio's.
    assert!(s.skills(P1).contains(&848));
    assert!(s.skills(P3).contains(&849) && !s.skills(P3).contains(&848));
    s.answer_reread(&out, rows);
    assert!(s.world.territory(1).unwrap().owned_ward_ids().is_empty());
    assert_eq!(s.world.territory(2).unwrap().owned_ward_ids(), [82, 81]);
    assert_eq!(s.world.territory_registered_clans(1), None);
    assert_eq!(s.world.territory_registered_mercenaries(2), None);
    assert_eq!(s.world.tw_start_time_ms(), next);
    assert!(!s.skills(P1).contains(&848));
    assert!(s.skills(P3).contains(&848) && s.skills(P3).contains(&849));
    // Nothing of it was written.
    assert!(territory_writes(&s.take()).is_empty());
}

/// D-51: a GM runs a test war whatever the switch says; nothing of it is written without it.
#[test]
fn a_gm_runs_the_war_without_the_switch() {
    let mut s = Scene::new(false);
    s.enter_all();
    s.start_war();
    assert!(message_ids(&s.take(), P8).contains(&SystemMessageId::TerritoryWarHasBegun));
    assert_eq!(s.npcs_of(GLUDIO_WARD).len(), 1);
    // `//territory_war_end`: the start moves `WarLength` back, the end task ends the war.
    s.admin("admin_territory_war_end");
    s.run_ms(1100);
    let out = s.take();
    assert!(!s.world.is_tw_in_progress());
    assert!(message_ids(&out, P8).contains(&SystemMessageId::TerritoryWarHasEnded));
    assert!(territory_writes(&out).is_empty());
    // A war needs two owned territories.
    let mut s = Scene::with(Options {
        siege_database_writes: false,
        dion_owner: 0,
        ..Options::default()
    });
    s.enter_all();
    s.admin("admin_territory_war_start");
    s.run_ms(1100);
    assert!(!s.world.is_tw_in_progress());
}

// ---------------------------------------------------------------------------------- registration

/// `RequestJoinDominionWar` after the re-read (D-50): the registration period, the owner clan,
/// the clan right, the registration elsewhere, the mercenary's level and class, and the window it
/// answers with; the rows go out with the switch.
#[test]
fn registrations_follow_java_after_a_reread() {
    let mut s = Scene::new(true);
    s.enter_all();
    // More than two hours before the war the registration is open.
    s.run_ms(1100);
    assert!(!s.world.is_tw_registration_over());
    // The owner clan of the territory.
    let out = s.join(P1, 81, 1, 1, s.rows_now());
    assert_eq!(
        message_ids(&out, P1),
        [SystemMessageId::TheClanWhoOwnsTheTerritoryCannotParticipateInTheTerritoryWarAsMercenaries]
    );
    // A member without the siege right.
    let out = s.join(P6, 82, 1, 1, s.rows_now());
    assert_eq!(
        message_ids(&out, P6),
        [SystemMessageId::YouAreNotAuthorizedToDoThat]
    );
    // The Bears are registered for Gludio already.
    let out = s.join(P5, 82, 1, 1, s.rows_now());
    assert_eq!(
        message_ids(&out, P5),
        [SystemMessageId::YouVeAlreadyRequestedATerritoryWarInAnotherTerritoryElsewhere]
    );
    // The Bears take Gludio back and register for Dion.
    let out = s.join(P5, 81, 1, 0, s.rows_now());
    assert_eq!(
        territory_writes(&out),
        [TerritoryWrite::RegistrationDelete {
            castle_id: 1,
            registered_id: BEARS,
        }]
    );
    let window: ExShowDominionRegistry = decode(&packets_to(&out, P5, "EX_SHOW_DOMINION_REGISTRY")[0]);
    assert_eq!(window.castle_id, 1);
    assert_eq!((window.clan_requests, window.clan_registered), (0, false));
    assert_eq!(
        (
            window.owner_clan_name.as_str(),
            window.owner_leader_name.as_str(),
            window.owner_ally_name.as_str()
        ),
        ("Lions", "P1", "")
    );
    assert_eq!(i64::from(window.war_time), SATURDAY_20 / 1000);
    assert_eq!(window.territories.len(), 2);
    let out = s.join(P5, 82, 1, 1, s.rows_now());
    assert_eq!(
        territory_writes(&out),
        [TerritoryWrite::RegistrationInsert {
            castle_id: 2,
            registered_id: BEARS,
        }]
    );
    let window: ExShowDominionRegistry = decode(&packets_to(&out, P5, "EX_SHOW_DOMINION_REGISTRY")[0]);
    assert_eq!(
        (window.castle_id, window.clan_requests, window.clan_registered),
        (2, 1, true)
    );
    assert_eq!((window.merc_requests, window.merc_registered), (1, false));
    // A mercenary below level 40 is refused without a word (Java's TODO: punish player).
    let out = s.join(P8, 81, 0, 1, s.rows_now());
    assert!(message_ids(&out, P8).is_empty());
    assert!(packets_to(&out, P8, "EX_SHOW_DOMINION_REGISTRY").is_empty());
    // A mercenary registers once; a member of a registered clan cannot be one.
    let out = s.join(P7, 81, 0, 1, s.rows_now());
    assert_eq!(
        message_ids(&out, P7),
        [SystemMessageId::YouVeAlreadyRequestedATerritoryWarInAnotherTerritoryElsewhere]
    );
    let out = s.join(P7, 82, 0, 0, s.rows_now());
    assert_eq!(
        territory_writes(&out),
        [TerritoryWrite::RegistrationDelete {
            castle_id: 2,
            registered_id: P7,
        }]
    );
    let out = s.join(P7, 81, 0, 1, s.rows_now());
    assert_eq!(
        territory_writes(&out),
        [TerritoryWrite::RegistrationInsert {
            castle_id: 1,
            registered_id: P7,
        }]
    );
    // A failed read refuses the request.
    s.take();
    s.input(
        P7,
        ClientInput::Territory(TerritoryInput::JoinDominionWar {
            territory_id: 81,
            is_clan: 0,
            is_joining: 0,
        }),
    );
    let asked = s.take();
    let then = territory_rereads(&asked).pop().unwrap();
    s.world.handle(WorldCommand::ResidenceReread {
        result: Err(()),
        then: ResidenceReadThen::Territory(then),
    });
    assert!(s.take().is_empty());
    assert_eq!(s.world.territory_registered_mercenaries(1), Some(vec![P7]));
    // Two hours before the war the registration is over.
    let start = s.now() + 3_600_000;
    s.world.tw_set_start_time(start);
    s.run_ms(1100);
    assert!(s.world.is_tw_registration_over());
    let out = s.join(P5, 82, 1, 0, s.rows_now());
    assert_eq!(
        message_ids(&out, P5),
        [SystemMessageId::ItIsNotATerritoryWarRegistrationPeriodSoARequestCannotBeMadeAtThisTime]
    );
    assert!(territory_writes(&out).is_empty());
}

/// D-50/D-51: the re-read brings Sieghardt's rows; a registration this world made without a row
/// (the switch off) stays.
#[test]
fn unsaved_registrations_survive_a_reread() {
    let mut s = Scene::new(false);
    s.enter_all();
    s.run_ms(1100);
    let rows = s.rows_now();
    // The GM registers as a mercenary of Gludio and the Bears take their registration back:
    // neither statement goes out.
    let out = s.join(GM, 81, 0, 1, rows.clone());
    assert!(territory_writes(&out).is_empty());
    let out = s.join(P5, 81, 1, 0, rows.clone());
    assert!(territory_writes(&out).is_empty());
    assert_eq!(s.world.territory_registered_clans(1), Some(vec![]));
    // Sieghardt's rows still have the Bears at Gludio, and a new mercenary of Dion.
    let mut sieghardt = rows;
    sieghardt.registrations.push(TerritoryRegistrationRecord {
        castle_id: 2,
        registered_id: P8,
    });
    let out = s.join(P6, 82, 1, 1, sieghardt);
    assert_eq!(
        message_ids(&out, P6),
        [SystemMessageId::YouAreNotAuthorizedToDoThat]
    );
    // The rows are the database's: the Bears are back; the GM's registration has no row and
    // stays.
    assert_eq!(s.world.territory_registered_clans(1), Some(vec![BEARS]));
    assert_eq!(s.world.territory_registered_mercenaries(1), Some(vec![GM]));
    assert_eq!(s.world.territory_registered_mercenaries(2), Some(vec![P7, P8]));
}

/// `RequestDominionInfo`: the territory list (the owned territories) and the ward positions (only
/// during the war).
#[test]
fn dominion_info_lists_the_territories_and_the_wards() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.input(P8, ClientInput::Territory(TerritoryInput::DominionInfo));
    let out = s.take();
    let info: ExReplyDominionInfo = decode(&packets_to(&out, P8, "EX_REPLY_DOMINION_INFO")[0]);
    assert_eq!(info.territories.len(), 2);
    assert_eq!(info.territories[0].territory_id, 81);
    assert_eq!(info.territories[0].name, "gludio_dominion");
    assert_eq!(info.territories[0].owner_clan_name, "Lions");
    assert_eq!(info.territories[0].ward_ids, [81]);
    assert_eq!(i64::from(info.territories[0].war_start_secs), SATURDAY_20 / 1000);
    let pos: ExShowOwnthingPos = decode(&packets_to(&out, P8, "EX_SHOW_OWNTHING_POS")[0]);
    assert!(pos.wards.is_empty());
    s.start_war();
    s.input(P8, ClientInput::Territory(TerritoryInput::DominionInfo));
    let out = s.take();
    let pos: ExShowOwnthingPos = decode(&packets_to(&out, P8, "EX_SHOW_OWNTHING_POS")[0]);
    assert_eq!(pos.wards.len(), 2);
    assert_eq!((pos.wards[0].territory_id, pos.wards[0].x), (81, X + 500));
    assert_eq!((pos.wards[1].territory_id, pos.wards[1].x), (82, DX + 500));
}

/// `isAllyField`: the player's territory's castle, or its fortress.
#[test]
fn the_ally_field_is_the_castle_and_the_fortress() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.start_war();
    let p1 = ObjectId(P1);
    assert!(s.world.is_ally_field(p1, 1));
    assert!(s.world.is_ally_field(p1, 101));
    assert!(!s.world.is_ally_field(p1, 2));
    assert!(!s.world.is_ally_field(p1, 103));
    assert!(!s.world.is_ally_field(ObjectId(P8), 1));
}

// ---------------------------------------------------------------------------------- wards

/// The ward of Gludio: only the enemies of the field may hit it, without skills; the killer on a
/// side takes the ward item and wears it; brought to his outpost with Ward Transport it becomes his
/// territory's (the rows, the message with exp to the participants, the ward skills).
#[test]
fn a_ward_is_taken_and_transported_to_the_outpost() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.start_war();
    let ward = s.npc(GLUDIO_WARD);
    // The Lions defend it, the Wolves attack it; outside of the sides nobody.
    assert!(!s.world.is_auto_attackable_by(ward, ObjectId(P1)));
    assert!(s.world.is_auto_attackable_by(ward, ObjectId(P3)));
    assert!(!s.world.is_auto_attackable_by(ward, ObjectId(P8)));
    // A defender's hit does nothing.
    s.world.reduce_current_hp(ward, 10.0, Some(ObjectId(P1)));
    assert!(!s.world.is_dead(ward));
    s.take();
    // The Wolves' leader destroys it and carries the ward.
    s.world.reduce_current_hp(ward, 1e9, Some(ObjectId(P3)));
    let out = s.take();
    assert!(s.npcs_of(GLUDIO_WARD).is_empty());
    assert!(s.worn(P3, GLUDIO_WARD_ITEM));
    assert!(s.world.is_ward_carrier(ObjectId(P3)));
    assert!(s.world.is_combat_flag_equipped(ObjectId(P3)));
    assert!(
        message_ids(&out, P3).contains(&SystemMessageId::YouVeAcquiredTheWardMoveQuicklyToYourForcesOutpost)
    );
    let destroyed: Vec<SystemMessage> = messages_to(&out, P1)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::TheS1WardHasBeenDestroyedC2NowHasTheTerritoryWard)
        .collect();
    assert_eq!(
        destroyed[0].params,
        [
            SystemMessageParam::Text("Gludio".into()),
            SystemMessageParam::PlayerName("P3".into())
        ]
    );
    // The ward point (`giveTWPoint(player, 81, 5)`).
    assert_eq!(s.world.territory_points(ObjectId(P3)).unwrap()[5], 1);
    // The ward stands in no place, its carrier is on the map.
    s.input(P8, ClientInput::Territory(TerritoryInput::DominionInfo));
    let pos: ExShowOwnthingPos = decode(&packets_to(&s.take(), P8, "EX_SHOW_OWNTHING_POS")[0]);
    assert_eq!((pos.wards[0].territory_id, pos.wards[0].x), (81, X + 70));
    // The outpost of the Wolves' territory, then Ward Transport on it.
    s.world.territory_create_outpost(ObjectId(P3));
    let outpost = s.npc(OUTPOST);
    assert_eq!(s.world.territory_hq_for_clan(WOLVES), Some(outpost));
    assert_eq!(s.world.territory_hq(82), Some(outpost));
    assert!(!s.world.is_auto_attackable_by(outpost, ObjectId(P1)));
    s.take();
    s.world
        .territory_on_skill_see(outpost, ObjectId(P3), 847, &[outpost]);
    let out = s.take();
    assert!(!s.world.is_ward_carrier(ObjectId(P3)));
    assert_eq!(s.item_count(P3, GLUDIO_WARD_ITEM), 0);
    assert!(s.world.territory(1).unwrap().owned_ward_ids().is_empty());
    assert_eq!(s.world.territory(2).unwrap().owned_ward_ids(), [82, 81]);
    assert_eq!(s.world.territory(2).unwrap().quest_done, [0, 1]);
    assert_eq!(s.world.territory_wards()[0].owner_castle_id, 2);
    // The ward stands on Dion's second place.
    let new_ward = s.npc(GLUDIO_WARD);
    assert_eq!(s.world.position_of(new_ward).unwrap().0, DX + 520);
    assert_eq!(
        territory_writes(&out),
        [
            TerritoryWrite::OwnedWardIds {
                territory_id: 81,
                ward_ids: String::new(),
            },
            TerritoryWrite::OwnedWardIds {
                territory_id: 82,
                ward_ids: "82;81;".into(),
            },
        ]
    );
    let captured: Vec<SystemMessage> = messages_to(&out, P1)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::ClanS1HasSucceededInCapturingS2STerritoryWard)
        .collect();
    assert_eq!(
        captured[0].params,
        [
            SystemMessageParam::Text("Wolves".into()),
            SystemMessageParam::CastleName(81)
        ]
    );
    // The ward skills follow the wards.
    assert!(s.skills(P3).contains(&848) && s.skills(P3).contains(&849));
    assert!(!s.skills(P1).contains(&848));
    // Outpost Demolition by somebody else's clan does nothing; by the owner it goes.
    s.world
        .territory_on_skill_see(outpost, ObjectId(P1), 845, &[outpost]);
    assert!(s.world.npc(outpost).is_some());
    s.world
        .territory_on_skill_see(outpost, ObjectId(P3), 845, &[outpost]);
    assert!(s.world.npc(outpost).is_none());
    assert_eq!(s.world.territory_hq_for_clan(WOLVES), None);
}

/// The carrier's death drops the ward where he died (the message to the participants); entering
/// a peace zone sends it back where it was picked up; a mounted or unregistered killer cannot
/// carry it.
#[test]
fn a_ward_carrier_dies_or_flees_into_a_peace_zone() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.start_war();
    let ward = s.npc(DION_WARD);
    // The Bears' leader (Gludio's side) destroys Dion's ward from where he stands.
    s.world.reduce_current_hp(ward, 1e9, Some(ObjectId(P5)));
    assert!(s.worn(P5, DION_WARD_ITEM));
    s.take();
    s.world.reduce_current_hp(ObjectId(P5), 1e9, Some(ObjectId(P3)));
    let out = s.take();
    assert!(s.world.is_dead(ObjectId(P5)));
    assert_eq!(s.item_count(P5, DION_WARD_ITEM), 0);
    let dropped = s.npc(DION_WARD);
    assert_eq!(s.world.position_of(dropped).unwrap().0, X + 80);
    let killed: Vec<SystemMessage> = messages_to(&out, P1)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::TheCharacterThatAcquiredS1SWardHasBeenKilled)
        .collect();
    assert_eq!(killed[0].params, [SystemMessageParam::Text("Dion".into())]);
    // At Gludio's field the Lions defend the ward that lies there.
    s.world.reduce_current_hp(dropped, 1e9, Some(ObjectId(P2)));
    assert!(!s.world.is_dead(dropped));
    // The Wolves' leader takes it back and walks into a peace zone: it goes back where it lay.
    s.world.reduce_current_hp(dropped, 1e9, Some(ObjectId(P3)));
    assert!(s.worn(P3, DION_WARD_ITEM));
    s.world
        .tele_to_location(ObjectId(P3), TeleportTarget::at(PEACE_X, Y, Z));
    s.input(P3, ClientInput::Appearing);
    s.run_ms(100);
    assert!(!s.world.is_ward_carrier(ObjectId(P3)));
    assert_eq!(s.item_count(P3, DION_WARD_ITEM), 0);
    let back = s.npc(DION_WARD);
    assert_eq!(s.world.position_of(back).unwrap().0, X + 80);
    // A player on no side does not hurt it.
    s.world.reduce_current_hp(back, 1e9, Some(ObjectId(P8)));
    assert!(!s.world.is_dead(back));
}

// ---------------------------------------------------------------------------------- outposts

/// `canCreateOutpost` and the flags of the war: the messages of each refusal, the outpost of a
/// castle owner only, the flag that replaces the clan's first, and the flag's death.
#[test]
fn outposts_and_flags_of_the_war() {
    let mut s = Scene::new(true);
    s.enter_all();
    // Outside the war the siege field is not active.
    assert!(!s.world.cond_can_create_outpost(ObjectId(P1)));
    assert_eq!(
        s.take()
            .iter()
            .filter(|o| matches!(o, Out::Packet(t, _) if *t == P1))
            .count(),
        1
    );
    s.start_war();
    // Not a clan leader.
    assert!(!s.world.cond_can_create_outpost(ObjectId(P2)));
    // Not in a headquarters zone.
    s.take();
    assert!(!s.world.cond_can_create_outpost(ObjectId(P1)));
    assert_eq!(
        message_ids(&s.take(), P1),
        [SystemMessageId::YouCannotSetUpABaseHere]
    );
    // In one.
    s.world
        .tele_to_location(ObjectId(P5), TeleportTarget::at(X - 1000, Y, Z));
    s.input(P5, ClientInput::Appearing);
    s.run_ms(100);
    assert!(s.world.cond_can_create_outpost(ObjectId(P5)));
    // The Bears own no castle: their outpost stands but is nobody's headquarters.
    s.world.territory_create_outpost(ObjectId(P5));
    assert_eq!(s.npcs_of(OUTPOST).len(), 1);
    assert_eq!(s.world.territory_hq_for_clan(BEARS), None);
    // The flag of the war, twice: the second replaces the first in the list.
    s.world.territory_take_flag(ObjectId(P5));
    let first = s.world.territory_flag_for_clan(BEARS).unwrap();
    s.take();
    assert!(!s.world.cond_can_create_outpost(ObjectId(P5)));
    assert_eq!(
        message_ids(&s.take(), P5),
        [SystemMessageId::AFlagIsAlreadyBeingDisplayedAnotherFlagCannotBeDisplayed]
    );
    s.world.territory_take_flag(ObjectId(P5));
    let second = s.world.territory_flag_for_clan(BEARS).unwrap();
    assert_ne!(first, second);
    // A flag of the war can be hit; its death takes the clan's flag off the list.
    assert!(s.world.is_auto_attackable_by(first, ObjectId(P1)));
    s.world.reduce_current_hp(first, 1e9, Some(ObjectId(P1)));
    assert_eq!(s.world.territory_flag_for_clan(BEARS), None);
    // The Lions' outpost is their territory's headquarters; a second is refused.
    s.world
        .tele_to_location(ObjectId(P1), TeleportTarget::at(X - 1000, Y, Z));
    s.input(P1, ClientInput::Appearing);
    s.run_ms(100);
    s.world.territory_create_outpost(ObjectId(P1));
    assert!(s.world.territory_hq_for_clan(LIONS).is_some());
    s.take();
    assert!(!s.world.cond_can_create_outpost(ObjectId(P1)));
    assert_eq!(
        message_ids(&s.take(), P1),
        [SystemMessageId::AnOutpostOrHeadquartersCannotBeBuiltBecauseOneAlreadyExists]
    );
    // `OutpostDestroy`.
    s.world.territory_destroy_outpost(ObjectId(P1));
    assert_eq!(s.world.territory_hq_for_clan(LIONS), None);
}

// ---------------------------------------------------------------------------------- rewards

/// `calcReward` with Java's numbers, and the Territory Managers' reward pages: nothing during the
/// war, the other territory's manager, the badges and adena, nothing twice.
#[test]
fn the_rewards_of_the_last_war() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.start_war();
    let p1 = ObjectId(P1);
    // 75 minutes online, two quests, 49 kills, three NPCs, a catapult and a ward.
    for _ in 0..75 {
        s.world.territory_give_point(p1, 1000, 6);
    }
    for _ in 0..2 {
        s.world.territory_give_quest_point(p1);
    }
    for _ in 0..49 {
        s.world.territory_give_point(p1, 82, 1);
    }
    for _ in 0..3 {
        s.world.territory_give_point(p1, 82, 3);
    }
    s.world.territory_give_point(p1, 82, 4);
    s.world.territory_give_point(p1, 82, 5);
    // No point for a kill of the own side.
    s.world.territory_give_point(p1, 81, 1);
    assert_eq!(s.world.territory_points(p1).unwrap(), [81, 49, 2, 3, 1, 1, 75]);
    // 7 + 14, then `(int) (21 + 4.9)` = 25, + 3 + 2 + 5, + the territory's one ward.
    assert_eq!(s.world.territory_calc_reward(p1), [81, 36]);
    // During the war the manager has nothing.
    let manager = s.npc(MANAGER);
    s.talk(P1, manager);
    assert_eq!(last_page(&s.take(), P1), format!("MANAGER {}", manager.0));
    s.script(P1, "CalcRewards");
    assert_eq!(last_page(&s.take(), P1), "NOTHING");
    s.admin("admin_territory_war_end");
    s.run_ms(1100);
    assert!(!s.world.is_tw_in_progress());
    s.take();
    s.script(P1, "CalcRewards");
    assert_eq!(
        last_page(&s.take(), P1),
        format!("REWARD Gludio 36 180000 81 {}", manager.0)
    );
    s.script(P1, "ReceiveRewards");
    assert_eq!(last_page(&s.take(), P1), format!("HERE {}", manager.0));
    assert_eq!(s.item_count(P1, GLUDIO_BADGE), 36);
    assert_eq!(s.item_count(P1, ADENA), 1000 + 180_000);
    // `resetReward`: the online minutes are gone, so is the reward.
    assert_eq!(s.world.territory_calc_reward(p1), [81, 0]);
    s.script(P1, "ReceiveRewards");
    assert_eq!(last_page(&s.take(), P1), "NOTHING");
    // Dion's manager sends a Gludio player to Gludio's.
    for _ in 0..10 {
        s.world.territory_give_point(ObjectId(P2), 1000, 6);
    }
    let dion = s.npc(DION_MANAGER);
    s.world
        .tele_to_location(ObjectId(P2), TeleportTarget::at(DX + 320, Y, Z));
    s.input(P2, ClientInput::Appearing);
    s.run_ms(100);
    s.talk(P2, dion);
    assert_eq!(last_page(&s.take(), P2), "DION MANAGER");
    s.script(P2, "CalcRewards");
    assert_eq!(last_page(&s.take(), P2), "OTHER Gludio");
}

/// The Territory Managers' other pages: the training, the badge products, the noblesse checks.
#[test]
fn the_territory_managers_answer_like_java() {
    let mut s = Scene::new(true);
    s.enter_all();
    let manager = s.npc(MANAGER);
    // A first class player.
    s.talk(P6, manager);
    assert_eq!(last_page(&s.take(), P6), "MORE TRAINING");
    s.talk(P2, manager);
    s.take();
    s.script(P2, "36490-03.html");
    assert_eq!(last_page(&s.take(), P2), "BADGES");
    s.script(P2, "36490-04.html");
    assert_eq!(last_page(&s.take(), P2), "NOBLE 100");
    s.script(P2, "BuyProducts");
    assert_eq!(last_page(&s.take(), P2), "NO BADGES");
    s.script(P2, "MakeMeNoble");
    assert_eq!(last_page(&s.take(), P2), "NO BADGES");
    s.world.add_item(ObjectId(P2), GLUDIO_BADGE, 100, false);
    s.script(P2, "MakeMeNoble");
    assert_eq!(last_page(&s.take(), P2), "LEVEL 75");
    // The noblesse (S-VM-01): the badges go, the noblesse is set; no page follows (the
    // noblesse quests are not installed in this world, so none is completed).
    s.talk(P1, manager);
    s.world.add_item(ObjectId(P1), GLUDIO_BADGE, 100, false);
    s.take();
    assert!(!s.world.is_noble(ObjectId(P1)));
    s.script(P1, "MakeMeNoble");
    assert!(packets_to(&s.take(), P1, "NPC_HTML_MESSAGE").is_empty());
    assert_eq!(s.item_count(P1, GLUDIO_BADGE), 0);
    assert!(s.world.is_noble(ObjectId(P1)));
    // `TerritoryStatus`: the castle's owner and its kingdom.
    s.talk(P1, manager);
    s.take();
    s.input(
        P1,
        ClientInput::Bypass {
            command: format!("npc_{}_TerritoryStatus", manager.0),
        },
    );
    assert_eq!(
        last_page(&s.take(), P1),
        "STATUS Lions P1 Gludio 7 The Kingdom of Aden"
    );
}

// ---------------------------------------------------------------------------------- the rest

/// `levelUpClan` to 11: only a clan that owns a territory.
#[test]
fn clan_level_11_needs_a_territory() {
    let mut s = Scene::new(true);
    s.enter_all();
    assert!(s.world.level_up_clan(ObjectId(P1)));
    assert_eq!(s.world.clan(LIONS).unwrap().level, 11);
    s.take();
    assert!(!s.world.level_up_clan(ObjectId(P5)));
    assert_eq!(
        message_ids(&s.take(), P5),
        [SystemMessageId::TheConditionsNecessaryToIncreaseTheClanSLevelHaveNotBeenMet]
    );
}

/// `//territory_war_time`: the page with the start date, a new day (kept in the variable with the
/// switch), a date in the past and a day the month has not; `//territory_wards_list` outside and
/// during the war.
#[test]
fn the_gm_commands_of_the_war() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.admin("admin_territory_war");
    assert_eq!(last_page(&s.take(), GM), "TW MENU");
    s.admin("admin_territory_war_time");
    assert_eq!(
        last_page(&s.take(), GM),
        "TIME Sat Oct 24 20:00:00 GMT+00:00 2026"
    );
    s.admin("admin_territory_war_time day 25");
    let out = s.take();
    let sunday = SATURDAY_20 + 86_400_000;
    assert_eq!(s.world.tw_start_time_ms(), sunday);
    assert_eq!(territory_writes(&out), [TerritoryWrite::NextTwStartDate(sunday)]);
    assert_eq!(last_page(&out, GM), "TIME Sun Oct 25 20:00:00 GMT+00:00 2026");
    s.admin("admin_territory_war_time day 1");
    let out = s.take();
    assert_eq!(s.world.tw_start_time_ms(), sunday);
    let texts: Vec<String> = messages_to(&out, GM)
        .into_iter()
        .flat_map(|m| m.params)
        .filter_map(|p| match p {
            SystemMessageParam::Text(t) => Some(t),
            _ => None,
        })
        .collect();
    assert!(
        texts.contains(&"Unable to change TW Date!".to_owned()),
        "{texts:?}"
    );
    s.admin("admin_territory_war_time day 40");
    let out = s.take();
    let texts: Vec<String> = messages_to(&out, GM)
        .into_iter()
        .flat_map(|m| m.params)
        .filter_map(|p| match p {
            SystemMessageParam::Text(t) => Some(t),
            _ => None,
        })
        .collect();
    assert!(
        texts
            .contains(&"Unable to change Siege Date - Incorrect day value only 1-31 is accepted!".to_owned()),
        "{texts:?}"
    );
    // The page is not shown after a wrong value.
    assert!(packets_to(&out, GM, "NPC_HTML_MESSAGE").is_empty());
    s.admin("admin_territory_wards_list");
    assert!(last_page(&s.take(), GM).contains("The Ward List is empty!"));
    s.start_war();
    s.admin("admin_territory_wards_list");
    let page = decode::<NpcHtmlMessage>(&packets_to(&s.take(), GM, "NPC_HTML_MESSAGE")[0]);
    assert_eq!(page.item_id, 1);
    assert!(page.html.contains("Gludio Ward"));
    assert!(page.html.contains(&format!("admin_move_to {} {Y} {Z}", X + 500)));
}

/// `Disguise`: the scroll of the player's own territory while the channel is open, another
/// territory's, or none at all.
#[test]
fn the_disguise_scroll_needs_the_own_territory() {
    let mut s = Scene::new(true);
    s.enter_all();
    for (player, item) in [
        (P7, DION_DISGUISE),
        (P7, GLUDIO_DISGUISE),
        (P8, DION_DISGUISE),
        (P1, GLUDIO_DISGUISE),
    ] {
        s.world.add_item(ObjectId(player), item, 1, false);
    }
    let use_item = |s: &mut Scene, player: i32, item_id: i32| {
        let object = s
            .world
            .player(ObjectId(player))
            .unwrap()
            .data
            .items
            .iter()
            .find(|i| i.item_id == item_id)
            .unwrap()
            .object_id;
        s.take();
        s.input(
            player,
            ClientInput::UseItem {
                object_id: object.0,
                ctrl_pressed: false,
            },
        );
        s.take()
    };
    // Before the channel opens nobody is registered.
    let out = use_item(&mut s, P7, DION_DISGUISE);
    assert!(message_ids(&out, P7).contains(
        &SystemMessageId::TheTerritoryWarExclusiveDisguiseAndTransformationCanBeUsed20MinutesBeforeTheStartOfTheTerritoryWarTo10MinutesAfterItsEnd
    ));
    s.start_war();
    let out = use_item(&mut s, P7, GLUDIO_DISGUISE);
    assert!(message_ids(&out, P7).contains(
        &SystemMessageId::TheDisguiseScrollCannotBeUsedBecauseItIsMeantForUseInADifferentTerritory
    ));
    let out = use_item(&mut s, P1, GLUDIO_DISGUISE);
    assert!(
        message_ids(&out, P1).contains(&SystemMessageId::ATerritoryOwningClanMemberCannotUseADisguiseScroll)
    );
    assert!(!s.world.is_disguised(ObjectId(P7)));
    use_item(&mut s, P7, DION_DISGUISE);
    assert!(s.world.is_disguised(ObjectId(P7)));
    assert_eq!(s.item_count(P7, DION_DISGUISE), 0);
}

/// `ChatBattlefield`: while the channel is open, the speaker's side hears it.
#[test]
fn the_battlefield_channel_is_the_side_s() {
    let mut s = Scene::new(true);
    s.enter_all();
    let say = |s: &mut Scene| {
        s.take();
        s.input(
            P1,
            ClientInput::Say {
                chat_type: 20,
                text: "charge".into(),
                target: None,
            },
        );
        s.take()
    };
    assert!(packets_to(&say(&mut s), P2, "SAY2").is_empty());
    s.start_war();
    let out = say(&mut s);
    for player in [P1, P2, P5] {
        assert_eq!(packets_to(&out, player, "SAY2").len(), 1, "{player:#x}");
    }
    assert!(packets_to(&out, P3, "SAY2").is_empty());
    assert!(packets_to(&out, P8, "SAY2").is_empty());
}

/// `isSiegeFriend` looks for the castle of the player's side, which a territory id (81 to 89) is
/// not: two players of one territory are no siege friends in Java (so the territory's own refusal
/// message never shows), but they are on one siege side.
#[test]
fn players_of_one_territory_are_on_one_side() {
    let mut s = Scene::new(true);
    s.enter_all();
    s.start_war();
    // Both Lions stand in Gludio's active siege zone on side 81, both attackers.
    assert!(!s.world.is_siege_friend(ObjectId(P1), ObjectId(P2)));
    assert!(s.world.is_on_same_siege_side_with(ObjectId(P1), ObjectId(P2)));
    assert!(!s.world.is_on_same_siege_side_with(ObjectId(P1), ObjectId(P3)));
}
