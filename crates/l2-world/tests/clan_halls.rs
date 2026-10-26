#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! Clan halls (M7-CH-01): the weekly rent, the auctions and the auction board, the hall manager,
//! the hall doorman and what the hall functions do.
//! Java: entity/residences/{ClanHall,AuctionableHall,ClanHallAuction}.java,
//! managers/ClanHallAuctionManager.java, entity/actor/instance/{Auctioneer,ClanHallManager,
//! ClanHallDoorman,Doorman}.java, mechanics/stats/Formulas.java (calcHpRegen, calcMpRegen). The
//! expected values come from the Java code named in each test.
//!
//! The pages are small files in a temporary datapack root: each prints the placeholders the Java
//! code replaces (`NAME=value;`) and carries a link to every bypass command, so the client-side
//! validation of the bypasses (`validateHtmlAction`) lets the tests send them.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates, KarmaLoss,
    MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates, SkillTrees,
    SpawnData, SpawnLoadOptions, StaticObjectTemplates, TeleporterData, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{AgitDecoInfo, NpcHtmlMessage, SystemMessage, SystemMessageParam};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord};
use l2_world::clanhall::{AuctionBidRecord, AuctionRecord, AuctionWrite, ClanHallSettings};
use l2_world::residence::{
    CastleRecord, ClanHallKind, ClanHallRecord, FunctionRecord, ResidenceReadKind, ResidenceReadThen,
    ResidenceRereadRows, ResidenceWrite,
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

/// A class with the levels 1 to 40 and a regeneration of every vital.
fn class_xml() -> String {
    let mut rows = String::new();
    for l in 1..=40 {
        rows.push_str(&format!(
            "<level val=\"{l}\"><hp>{}.5</hp><mp>{}.25</mp><cp>{l}.75</cp>             <hpRegen>2.0</hpRegen><mpRegen>0.9</mpRegen><cpRegen>1.5</cpRegen></level>",
            l * 10,
            l * 5
        ));
    }
    format!(
        r#"<list><classId>0</classId><staticData>
<baseINT>21</baseINT><baseSTR>40</baseSTR><baseCON>43</baseCON><baseMEN>25</baseMEN><baseDEX>30</baseDEX><baseWIT>11</baseWIT>
<basePAtk>4</basePAtk><baseCritRate>4</baseCritRate><basePAtkSpd>300</basePAtkSpd>
<basePDef><chest>31</chest><legs>18</legs><head>12</head><feet>7</feet><gloves>8</gloves><underwear>3</underwear><cloak>1</cloak></basePDef>
<baseMAtk>6</baseMAtk>
<baseMDef><rear>9</rear><lear>8</lear><rfinger>5</rfinger><lfinger>4</lfinger><neck>13</neck></baseMDef>
<baseMoveSpd><walk>80</walk><run>120</run><slowSwim>50</slowSwim><fastSwim>60</fastSwim></baseMoveSpd>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{rows}</lvlUpgainData></list>"#
    )
}

/// Skill 5000 is a hall buff that costs 50 mana (plus 10 to start); 5001 is a free one.
const SKILLS: &str = r#"<list>
<skill id="5000" levels="1" name="Hall Buff"><operateType>A2</operateType><mpConsume>50</mpConsume><mpInitialConsume>10</mpInitialConsume></skill>
</list>"#;

const ADENA: i32 = 57;

const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
</list>"#;

fn stat_data() -> StatData {
    StatData {
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        player_templates: Arc::new(PlayerTemplates::from_xml(&class_xml(), 85).unwrap()),
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
/// Hall 36's zone is around this x.
const HALL_X: i32 = X + 20_000;
/// The door of hall 36 in `hall_scene`.
const HALL_DOOR: i32 = 22_000_036;
/// Lions (P1 leader, P2) and Wolves (P3 leader, P4); P5 is the leader of the level 1 Bears.
const P1: i32 = 0x6000_0001;
const P2: i32 = 0x6000_0002;
const P3: i32 = 0x6000_0003;
const P4: i32 = 0x6000_0004;
const P5: i32 = 0x6000_0005;
const GM: i32 = 0x6000_0006;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
const BEARS: i32 = 700;
/// The auctioneer (in Gludio's town), the hall manager and the hall doorman (at hall 36).
const AUCTIONEER: i32 = 30_001;
const MANAGER: i32 = 35_001;
const DOORMAN: i32 = 35_002;

const WEEK: i64 = 604_800_000;
const FAR_FUTURE: i64 = i64::MAX / 2;

fn now_ms() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

/// The placeholders the pages print, so a test sees what the code replaced.
const PLACEHOLDERS: &[&str] = &[
    "AGIT_NAME",
    "OWNER_PLEDGE_NAME",
    "AGIT_OWNER_PLEDGE_NAME",
    "OWNER_PLEDGE_MASTER",
    "AGIT_SIZE",
    "AGIT_LEASE",
    "AGIT_LOCATION",
    "AGIT_AUCTION_END",
    "AGIT_AUCTION_REMAIN",
    "AGIT_AUCTION_MINBID",
    "AGIT_AUCTION_COUNT",
    "AGIT_AUCTION_BIDCOUNT",
    "AGIT_AUCTION_MYBID",
    "AGIT_AUCTION_MIN",
    "AGIT_AUCTION_DESC",
    "AGIT_AUCTION_BID",
    "AGIT_LINK_BACK",
    "AGIT_LINK_BIDLIST",
    "AGIT_LINK_RE",
    "itemsField",
    "AGIT_LIST",
    "PLEDGE_ADENA",
    "AGIT_PLEDGE_ADENA",
    "AGIT_DEPOSIT",
    "AGIT_LAST_PRICE",
    "AGIT_BID",
    "AGIT_BID_REMAIN",
    "x",
    "id",
    "location",
    "LOCATION",
    "name",
    "cost",
    "use",
    "apply",
    "val",
    "rent",
    "date",
    "mp",
    "xp_regen",
    "hp_regen",
    "mp_regen",
    "hp_recovery",
    "hp_period",
    "change_hp",
    "mp_recovery",
    "mp_period",
    "change_mp",
    "exp_recovery",
    "exp_period",
    "change_exp",
    "tele",
    "tele_period",
    "change_tele",
    "support",
    "support_period",
    "change_support",
    "item",
    "item_period",
    "change_item",
    "curtain",
    "curtain_period",
    "change_curtain",
    "fixture",
    "fixture_period",
    "change_fixture",
    "npcId",
    "npcname",
    "clanname",
    "leadername",
    "hallname",
    "objectId",
];

/// Every bypass command the NPCs of this card take, as a link with a variable part (`$p`): the
/// validation accepts any command that starts with the link.
const COMMANDS: &[&str] = &[
    "auction",
    "confirmAuction",
    "bidding",
    "bid",
    "bid1",
    "list",
    "bidlist",
    "selectedItems",
    "cancelBid",
    "doCancelBid",
    "cancelAuction",
    "doCancelAuction",
    "sale2",
    "sale",
    "rebid",
    "location",
    "start",
    "banish_foreigner",
    "manage_vault",
    "door",
    "functions",
    "manage",
    "support",
    "list_back",
    "support_back",
    "goto",
    "open_doors",
    "close_doors",
    "tele",
    "evolve",
    "Chat",
    "DepositC",
];

fn page_text(file: &str) -> String {
    let mut text = format!("<html><body>PAGE={file};");
    for name in PLACEHOLDERS {
        text.push_str(&format!(" {name}=%{name}%;"));
    }
    for command in COMMANDS {
        text.push_str(&format!(
            "<a action=\"bypass -h npc_%objectId%_{command} $p\">{command}</a>"
        ));
    }
    text.push_str("</body></html>");
    text
}

const PAGES: &[&str] = &[
    "auction/auction-busy.htm",
    "auction/auction.htm",
    "auction/AgitAuctionInfo.htm",
    "auction/AgitAuctionList.htm",
    "auction/AgitBid1.htm",
    "auction/AgitBid2.htm",
    "auction/AgitBidCancel.htm",
    "auction/AgitBidInfo.htm",
    "auction/AgitBidderList.htm",
    "auction/AgitInfo.htm",
    "auction/AgitSale1.htm",
    "auction/AgitSale2.htm",
    "auction/AgitSale3.htm",
    "auction/AgitSaleCancel.htm",
    "auction/AgitSaleInfo.htm",
    "auction/location.htm",
    "auction/not_authorized.htm",
    "clanHallManager/banish-list.htm",
    "clanHallManager/banish.htm",
    "clanHallManager/chamberlain-no.htm",
    "clanHallManager/chamberlain-of.htm",
    "clanHallManager/chamberlain.htm",
    "clanHallManager/chamberlain-nac.htm",
    "clanHallManager/deco.htm",
    "clanHallManager/door-close.htm",
    "clanHallManager/door-open.htm",
    "clanHallManager/door.htm",
    "clanHallManager/edit_other.htm",
    "clanHallManager/edit_recovery.htm",
    "clanHallManager/functions-apply.htm",
    "clanHallManager/functions-apply_confirmed.htm",
    "clanHallManager/functions-cancel.htm",
    "clanHallManager/functions-cancel_confirmed.htm",
    "clanHallManager/functions-used.htm",
    "clanHallManager/functions.htm",
    "clanHallManager/low_adena.htm",
    "clanHallManager/manage.htm",
    "clanHallManager/not_authorized.htm",
    "clanHallManager/support-done.htm",
    "clanHallManager/support-no_mana.htm",
    "clanHallManager/support1.htm",
    "clanHallManager/teleGludio1.htm",
    "clanHallManager/vault.htm",
    "clanHallManager/vault-chs.htm",
    "clanHallDoorman/doorman-closed.htm",
    "clanHallDoorman/doorman-no.htm",
    "clanHallDoorman/doorman-opened.htm",
    "clanHallDoorman/doorman1.htm",
    "clanHallDoorman/doorman2.htm",
    "clanHallDoorman/emptyowner.htm",
    "clanHallDoorman/evolve-no.htm",
    "clanHallDoorman/evolve-ok.htm",
];

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="30001" type="Auctioneer" level="70" name="Auctioneer" usingServerSideName="true"><status attackable="false" /></npc>
  <npc id="35001" type="ClanHallManager" level="70" name="Chamberlain" usingServerSideName="true"><status attackable="false" /><stats><vitals hp="1000" mp="30" /></stats></npc>
  <npc id="35002" type="ClanHallDoorman" level="70" name="Doorman" usingServerSideName="true"><status attackable="false" /></npc>
</list>"#;

/// The manager's mirror: stage 1 leads to one place.
const TELEPORTERS: &str = r#"<?xml version="1.0"?><list>
  <npc id="35001">
    <teleport name="tel1" type="OTHER"><location x="-80684" y="149770" z="-3040" /></teleport>
  </npc>
  <npc id="35002">
    <teleport type="OTHER"><location x="-80000" y="149000" z="-3040" /></teleport>
  </npc>
</list>"#;

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
  <zone id="11025" name="gludio_town" type="TownZone" {}<stat name="taxById" val="1" /></zone>
  <zone name="gludio_castle_battlefield_territory" type="SiegeZone" {}<stat name="castleId" val="1" /></zone>
  <zone name="agit36" type="ClanHallZone" {}<stat name="clanHallId" val="36" /></zone>
</list>"#,
        cuboid(X, Y, 3000),
        cuboid(X, Y, 4000),
        cuboid(HALL_X, Y, 300),
    )
}

struct Scene {
    world: World,
    log: Log,
    /// A test that answers the auction list reads itself sets this (M7-FU-03); else `bypass`
    /// answers with what the world has.
    hold_list_reads: bool,
    ids: Arc<IdFactory>,
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
        write("data/zones/halls.xml", &zones_xml());
        write("data/teleporters/test.xml", TELEPORTERS);
        for page in PAGES {
            write(&format!("data/html/{page}"), &page_text(page));
        }
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(Recorder(log.clone())));
        world.set_static_data(Box::new(Data(stat_data())));
        world.set_exp_data(
            Arc::new(ExperienceTable::from_xml(r#"<table maxLevel="85" maxPetLevel="85"></table>"#).unwrap()),
            Arc::new(ExperienceLoss::from_xml("<list></list>").unwrap()),
            Arc::new(KarmaLoss::from_xml("<list></list>").unwrap()),
            Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
        );
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        world.set_teleporter_data(Arc::new(TeleporterData::load(root.path())));
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
        world
            .spawn_static_objects(&StaticObjectTemplates::from_xml("<list></list>").unwrap(), &ids)
            .unwrap();
        world.set_residence_settings(ResidenceSettings {
            siege_database_writes,
            ..ResidenceSettings::default()
        });
        world.load_clans(vec![lions(), wolves(), bears()], Vec::new());
        Self {
            world,
            log,
            hold_list_reads: false,
            ids,
            _root: root,
        }
    }

    /// Loads the halls and then the auctions like the game server does.
    fn load(&mut self, halls: Vec<ClanHallRecord>, auctions: Vec<AuctionRecord>) {
        self.load_halls(halls);
        self.world.load_clan_hall_auctions(auctions);
        self.log.take();
    }

    /// Loads the halls (`ClanHallTable.load`) without the auctions.
    fn load_halls(&mut self, halls: Vec<ClanHallRecord>) {
        self.world.load_residences(ResidenceLoad {
            castles: vec![CastleRecord {
                id: 1,
                found: true,
                name: "Gludio".to_owned(),
                time_registration_over: true,
                ..CastleRecord::default()
            }],
            clan_halls: halls,
            ..ResidenceLoad::default()
        });
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

    fn spawn(&mut self, npc_id: i32, x: i32) -> ObjectId {
        self.world.spawn_npc_at(npc_id, x, Y, Z, 0).unwrap()
    }

    /// The client's click on the NPC: the chat window.
    fn talk(&mut self, player: i32, npc: ObjectId) {
        self.world.show_chat_window_default(npc, ObjectId(player));
    }

    fn bypass(&mut self, player: i32, npc: ObjectId, command: &str) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input: ClientInput::Bypass {
                command: format!("npc_{}_{command}", npc.0),
            },
        });
        if !self.hold_list_reads {
            // The persistence task reads the `auction` rows: here they are what the world has.
            let requests = self.take_list_requests();
            for then in requests {
                let rows = self
                    .world
                    .clan_hall_auctions()
                    .iter()
                    .map(|a| a.to_record())
                    .collect();
                self.world.handle(WorldCommand::ResidenceReread {
                    result: Ok(ResidenceRereadRows::Auctions(rows)),
                    then,
                });
            }
        }
    }

    /// The auction list reads in the log, taken out of it.
    fn take_list_requests(&mut self) -> Vec<ResidenceReadThen> {
        let mut log = self.log.0.lock().unwrap();
        let mut thens = Vec::new();
        log.retain(|o| match o {
            Out::Persist(PersistCommand::ResidenceRereadRequest {
                kind: ResidenceReadKind::Auctions,
                then,
                ..
            }) => {
                thens.push(then.clone());
                false
            }
            _ => true,
        });
        thens
    }

    /// The answer of the persistence task to the auction list read in `out`.
    fn answer_list_read(&mut self, out: &[Out], rows: Result<Vec<AuctionRecord>, ()>) {
        let then = out
            .iter()
            .rev()
            .find_map(|o| match o {
                Out::Persist(PersistCommand::ResidenceRereadRequest {
                    kind: ResidenceReadKind::Auctions,
                    then,
                    ..
                }) => Some(then.clone()),
                _ => None,
            })
            .expect("an auction list read was requested");
        self.world.handle(WorldCommand::ResidenceReread {
            result: rows.map(ResidenceRereadRows::Auctions),
            then,
        });
    }

    /// The answer of the persistence task to the last read of an auction.
    fn answer_auction_read(&mut self, out: &[Out], record: Option<AuctionRecord>) {
        let (id, then) = out
            .iter()
            .rev()
            .find_map(|o| match o {
                Out::Persist(PersistCommand::ResidenceRereadRequest {
                    residence_id,
                    kind: ResidenceReadKind::Auction,
                    then,
                }) => Some((*residence_id, then.clone())),
                _ => None,
            })
            .expect("an auction read was requested");
        self.world.handle(WorldCommand::ResidenceReread {
            result: Ok(ResidenceRereadRows::Auction { id, record }),
            then,
        });
    }

    /// The answer of the persistence task to the last clan warehouse read.
    fn answer_warehouse_read(&mut self, out: &[Out], items: Vec<ItemData>) {
        let (player, clan_id) = out
            .iter()
            .rev()
            .find_map(|o| match o {
                Out::Persist(PersistCommand::ClanWarehouseRequest { player, clan_id }) => {
                    Some((*player, *clan_id))
                }
                _ => None,
            })
            .expect("a clan warehouse read was requested");
        self.world.handle(WorldCommand::ClanWarehouseLoaded {
            player,
            clan_id,
            result: Ok(items),
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

fn adena_item(object_id: i32, count: i64, loc: &str) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id: ADENA,
        count,
        loc: loc.to_owned(),
        ..Default::default()
    }
}

/// Lions (level 5): a warehouse of 100,000 adena.
fn lions() -> ClanRecord {
    ClanRecord {
        clan_id: LIONS,
        data: Some(ClanDataRecord {
            name: Some("Lions".to_owned()),
            level: 5,
            leader_id: P1,
            ..ClanDataRecord::default()
        }),
        members: vec![member(P1, 1), member(P2, 5)],
        warehouse: vec![adena_item(0x6000_1000, 100_000, "CLANWH")],
        ..ClanRecord::default()
    }
}

/// Wolves (level 5): own hall 36; a warehouse of 2,000,000 adena.
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
        warehouse: vec![adena_item(0x6000_1001, 2_000_000, "CLANWH")],
        ..ClanRecord::default()
    }
}

/// Bears (level 1): too small to bid.
fn bears() -> ClanRecord {
    ClanRecord {
        clan_id: BEARS,
        data: Some(ClanDataRecord {
            name: Some("Bears".to_owned()),
            level: 1,
            leader_id: P5,
            ..ClanDataRecord::default()
        }),
        members: vec![member(P5, 1)],
        ..ClanRecord::default()
    }
}

fn hall(id: i32, owner_id: i32, lease: i32, grade: i32, paid_until: i64, paid: bool) -> ClanHallRecord {
    ClanHallRecord {
        id,
        name: format!("Hall {id}"),
        owner_id,
        lease,
        desc: format!("Description of hall {id}"),
        location: "Gludio".to_owned(),
        paid_until,
        grade,
        paid,
        functions: Vec::new(),
    }
}

fn auction_record(id: i32, starting_bid: i64, end_date: i64) -> AuctionRecord {
    AuctionRecord {
        id,
        seller_id: 0,
        seller_name: "NPC".to_owned(),
        seller_clan_name: "NPC Clan".to_owned(),
        item_type: "ClanHall".to_owned(),
        item_id: id,
        item_object_id: 0,
        item_name: format!("Auction {id}"),
        starting_bid,
        current_bid: 0,
        end_date,
        bidders: Vec::new(),
    }
}

fn bid_record(clan_id: i32, leader: &str, clan: &str, max_bid: i64) -> AuctionBidRecord {
    AuctionBidRecord {
        bidder_id: clan_id,
        bidder_name: leader.to_owned(),
        max_bid,
        clan_name: clan.to_owned(),
        time_bid: 1_700_000_000_000,
    }
}

fn pdata(id: i32, clan_id: i32, x: i32, adena: i64, access_level: i32) -> PlayerData {
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
        access_level,
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
        items: if adena > 0 {
            vec![adena_item(0x7000_0000 + id % 100, adena, "INVENTORY")]
        } else {
            vec![]
        },
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

/// The ids of the system messages sent to a player.
fn message_ids(out: &[Out], to: i32) -> Vec<SystemMessageId> {
    messages_to(out, to).iter().map(|m| m.id).collect()
}

/// The plain text lines (`sendMessage`) sent to a player.
fn texts_to(out: &[Out], to: i32) -> Vec<String> {
    messages_to(out, to)
        .into_iter()
        .filter(|m| m.id == SystemMessageId::S12)
        .filter_map(|m| match m.params.first() {
            Some(SystemMessageParam::Text(t)) => Some(t.clone()),
            _ => None,
        })
        .collect()
}

/// The pages sent to a player.
fn pages_to(out: &[Out], to: i32) -> Vec<NpcHtmlMessage> {
    packets_to(out, to, "NPC_HTML_MESSAGE")
        .iter()
        .map(|b| decode::<NpcHtmlMessage>(b))
        .collect()
}

fn last_page(out: &[Out], to: i32) -> String {
    pages_to(out, to).pop().map(|p| p.html).expect("a page was sent")
}

/// `NAME=value;` of a page printed by `page_text`.
fn field(page: &str, name: &str) -> String {
    let marker = format!(" {name}=");
    let start = page
        .find(&marker)
        .unwrap_or_else(|| panic!("no {name} in {page}"))
        + marker.len();
    let end = page[start..].find(';').unwrap() + start;
    page[start..end].to_owned()
}

fn residence_writes(out: &[Out]) -> Vec<ResidenceWrite> {
    out.iter()
        .filter_map(|o| match o {
            Out::Persist(PersistCommand::Residence(w)) => Some(w.clone()),
            _ => None,
        })
        .collect()
}

fn auction_writes(out: &[Out]) -> Vec<AuctionWrite> {
    residence_writes(out)
        .into_iter()
        .filter_map(|w| match w {
            ResidenceWrite::ClanHallAuction(a) => Some(a),
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

fn warehouse_adena(s: &Scene, clan: i32) -> i64 {
    s.clan(clan)
        .warehouse
        .iter()
        .find(|i| i.item_id == ADENA)
        .map_or(0, |i| i.count)
}

fn lease_of(s: &Scene, hall: i32) -> (i64, bool) {
    match s.world.clan_hall(ClanHallId(hall)).unwrap().kind {
        ClanHallKind::Auctionable { paid_until, paid, .. } => (paid_until, paid),
        ClanHallKind::Siegable { .. } => panic!("not an auctionable hall"),
    }
}

// ---------------------------------------------------------------------------------- the rent

/// `AuctionableHall.FeeTask`: a due rent is taken from the clan warehouse, `paidUntil` moves one
/// week on (`CH_RATE`) from where it was, the row is written.
#[test]
fn a_due_rent_is_taken_from_the_clan_warehouse() {
    let mut s = Scene::new(true);
    let before = now_ms();
    s.load(vec![hall(36, WOLVES, 500_000, 2, before - 1000, true)], vec![]);
    s.enter(pdata(P3, WOLVES, HALL_X, 0, 0));
    s.take();
    s.run_ms(100);
    let out = s.take();
    let (paid_until, paid) = lease_of(&s, 36);
    assert!(paid);
    // `while (_paidUntil <= _time) _paidUntil += CH_RATE`: exactly one week from the old value.
    assert_eq!(paid_until, before - 1000 + WEEK);
    assert_eq!(warehouse_adena(&s, WOLVES), 1_500_000);
    assert_eq!(
        residence_writes(&out),
        [ResidenceWrite::ClanHall {
            hall_id: 36,
            owner_id: WOLVES,
            paid_until,
            paid: true
        }]
    );
    assert!(matches!(
        item_row_ops(&out)[..],
        [ItemRowOp::Update { owner_id: WOLVES, .. }]
    ));
    // Nothing is due for a week: no second charge.
    s.run_ms(5000);
    assert_eq!(warehouse_adena(&s, WOLVES), 1_500_000);
}

/// A clan that cannot pay hears about it (the lease as the number) and keeps the hall; the row says
/// `paid = 0`. NOTE(java-quirk): the next try is scheduled with the absolute time as the delay,
/// so it never comes.
#[test]
fn a_clan_that_cannot_pay_is_reminded_and_keeps_the_hall() {
    let mut s = Scene::new(true);
    let before = now_ms();
    let mut poor = wolves();
    poor.warehouse.clear();
    s.world.load_clans(vec![lions(), poor, bears()], Vec::new());
    s.load(vec![hall(36, WOLVES, 500_000, 2, before - 1000, true)], vec![]);
    s.enter(pdata(P3, WOLVES, HALL_X, 0, 0));
    s.take();
    s.run_ms(100);
    let out = s.take();
    let messages = messages_to(&out, P3);
    assert_eq!(
        messages.last().map(|m| (m.id, m.params.clone())),
        Some((
            SystemMessageId::PaymentForYourClanHallHasNotBeenMadePleaseMakePaymentToYourClanWarehouseByS1Tomorrow,
            vec![SystemMessageParam::IntNumber(500_000)]
        ))
    );
    let (paid_until, paid) = lease_of(&s, 36);
    assert!(!paid);
    assert_eq!(paid_until, before - 1000);
    assert_eq!(s.world.clan_hall(ClanHallId(36)).unwrap().owner_id, WOLVES);
    assert_eq!(
        residence_writes(&out),
        [ResidenceWrite::ClanHall {
            hall_id: 36,
            owner_id: WOLVES,
            paid_until,
            paid: false
        }]
    );
    s.run_ms(10_000);
    assert!(messages_to(&s.take(), P3).is_empty());
}

/// A rent a week late takes the hall away: `initNPC`, `setFree`, the message to the clan.
#[test]
fn a_rent_a_week_late_takes_the_hall_and_puts_it_up_for_auction() {
    let mut s = Scene::new(true);
    let before = now_ms();
    let mut poor = wolves();
    poor.warehouse.clear();
    s.world.load_clans(vec![lions(), poor, bears()], Vec::new());
    let mut owned = hall(36, WOLVES, 500_000, 2, before - WEEK - 10_000, false);
    owned.functions = vec![FunctionRecord {
        function_type: 1,
        lvl: 1,
        lease: 1000,
        rate: 86_400_000,
        end_time: FAR_FUTURE,
    }];
    s.load(vec![owned], vec![]);
    s.enter(pdata(P3, WOLVES, HALL_X, 0, 0));
    s.take();
    s.run_ms(100);
    let out = s.take();
    assert!(s.world.clan_hall(ClanHallId(36)).unwrap().is_free);
    assert_eq!(s.clan(WOLVES).hideout_id, 0);
    assert!(message_ids(&out, P3).contains(
        &SystemMessageId::TheClanHallFeeIsOneWeekOverdueThereforeTheClanHallOwnershipHasBeenRevoked
    ));
    // `initNPC(36)`: the NPC auction of "The Golden Chamber" starts a week from now.
    let auction = s.world.clan_hall_auction(36).unwrap();
    assert_eq!(auction.item_name, "The Golden Chamber");
    assert_eq!((auction.seller_id, auction.starting_bid), (0, 50_000_000));
    let writes = residence_writes(&out);
    assert!(writes.iter().any(|w| matches!(
        w,
        ResidenceWrite::ClanHallAuction(AuctionWrite::NpcAuctionInsert {
            hall_id: 36,
            starting_bid: 50_000_000,
            end_date: 1_106_827_200_000,
            ..
        })
    )));
    assert!(writes.iter().any(|w| matches!(
        w,
        ResidenceWrite::ClanHallAuction(AuctionWrite::EndDate { id: 36, .. })
    )));
    // The functions went with the owner, the hall row says owner 0.
    assert!(
        writes
            .iter()
            .any(|w| matches!(w, ResidenceWrite::FunctionDelete { function_type: 1, .. }))
    );
    assert!(writes.iter().any(|w| matches!(
        w,
        ResidenceWrite::ClanHall {
            hall_id: 36,
            owner_id: 0,
            ..
        }
    )));
}

/// D-51: without `SiegeDatabaseWrites` the rent only moves the paid time on: nothing is taken,
/// nothing is written, a hall that cannot pay stays (the Java game server decides).
#[test]
fn without_the_switch_the_rent_charges_and_writes_nothing() {
    let mut s = Scene::new(false);
    let before = now_ms();
    s.load(
        vec![
            hall(36, WOLVES, 500_000, 2, before - 1000, true),
            hall(37, LIONS, 500_000, 2, before - WEEK - 10_000, false),
        ],
        vec![],
    );
    s.run_ms(100);
    let out = s.take();
    assert!(residence_writes(&out).is_empty());
    assert!(item_row_ops(&out).is_empty());
    assert_eq!(warehouse_adena(&s, WOLVES), 2_000_000);
    assert_eq!(lease_of(&s, 36), (before - 1000 + WEEK, true));
    // The late hall is not taken away either.
    assert!(!s.world.clan_hall(ClanHallId(37)).unwrap().is_free);
    assert_eq!(s.clan(LIONS).hideout_id, 37);
}

/// A hall that has just been won starts its rent at once (`initialyzeTask(true)`): the first week
/// is charged right away and `paidUntil` is a week from then.
#[test]
fn a_new_owner_pays_the_first_week_at_once() {
    let mut s = Scene::new(true);
    s.load(vec![hall(38, 0, 100_000, 1, 0, false)], vec![]);
    s.world.clan_hall_table_set_owner(ClanHallId(38), LIONS);
    s.take();
    let before = now_ms();
    s.run_ms(100);
    let out = s.take();
    let (paid_until, paid) = lease_of(&s, 38);
    assert!(paid);
    assert!(paid_until >= before + WEEK - 200 && paid_until <= now_ms() + WEEK);
    assert_eq!(warehouse_adena(&s, LIONS), 0);
    assert!(residence_writes(&out).iter().any(|w| matches!(
        w,
        ResidenceWrite::ClanHall {
            hall_id: 38,
            paid: true,
            ..
        }
    )));
}

// ---------------------------------------------------------------------------------- loading

/// `ClanHallTable.load`: a free hall with a lease and no auction gets its NPC auction
/// (`ITEM_INIT_DATA`); one with an auction row, one without a lease and an owned one do not.
#[test]
fn loading_starts_the_npc_auctions_of_free_halls() {
    let mut s = Scene::new(true);
    let before = now_ms();
    s.world.load_residences(ResidenceLoad {
        clan_halls: vec![
            hall(36, WOLVES, 500_000, 2, FAR_FUTURE, true),
            hall(38, 0, 100_000, 1, 0, false),
            hall(39, 0, 100_000, 1, 0, false),
            hall(40, 0, 0, 1, 0, false),
        ],
        ..ResidenceLoad::default()
    });
    s.take();
    s.world
        .load_clan_hall_auctions(vec![auction_record(39, 1000, before + 3_600_000)]);
    let out = s.take();
    let ids: Vec<i32> = s.world.clan_hall_auctions().iter().map(|a| a.id).collect();
    assert_eq!(ids, [39, 38]);
    let npc = s.world.clan_hall_auction(38).unwrap();
    assert_eq!(npc.item_name, "The Mithril Chamber");
    assert_eq!(npc.starting_bid, 50_000_000);
    assert_eq!(
        (npc.seller_name.as_str(), npc.seller_clan_name.as_str()),
        ("NPC", "NPC Clan")
    );
    // The old end date moved a week on (`startAutoTask`); the loaded auction keeps its own.
    assert!(npc.end_date >= before + WEEK && npc.end_date <= now_ms() + WEEK);
    assert_eq!(
        s.world.clan_hall_auction(39).unwrap().end_date,
        before + 3_600_000
    );
    let writes = auction_writes(&out);
    assert_eq!(writes.len(), 2);
    assert!(matches!(
        &writes[0],
        AuctionWrite::NpcAuctionInsert { hall_id: 38, name, starting_bid: 50_000_000, end_date: 1_106_827_200_000 }
            if name == "The Mithril Chamber"
    ));
    assert!(matches!(&writes[1], AuctionWrite::EndDate { id: 38, .. }));
}

/// `loadBid`: the highest bid is the first row; the bidders are all of them.
#[test]
fn the_loaded_bids_name_the_highest_bidder() {
    let mut s = Scene::new(true);
    s.load_halls(vec![hall(39, 0, 100_000, 1, 0, false)]);
    let mut record = auction_record(39, 1000, now_ms() + 3_600_000);
    record.bidders = vec![
        bid_record(WOLVES, "P3", "Wolves", 5000),
        bid_record(LIONS, "P1", "Lions", 3000),
    ];
    s.world.load_clan_hall_auctions(vec![record]);
    let auction = s.world.clan_hall_auction(39).unwrap();
    assert_eq!(auction.highest_bidder_id, WOLVES);
    assert_eq!(auction.highest_bidder_name, "P3");
    assert_eq!(auction.highest_bidder_max_bid, 5000);
    assert_eq!(auction.bidders.len(), 2);
}

#[test]
fn the_clan_hall_settings_default_to_javas_fees() {
    let s = ClanHallSettings::default();
    assert_eq!(s.hp.fee(1), 700);
    assert_eq!(s.hp.fee(13), 5166);
    assert_eq!(s.hp.days(), 1);
    assert_eq!(s.tele.days(), 7);
    assert_eq!(s.front.days(), 3);
    assert_eq!(s.support.fee(8), 52_000);
    assert!(!s.buff_free);
    // `default:` is the highest slot.
    assert_eq!(s.tele.fee(9), 14_000);
}

// ---------------------------------------------------------------------------------- the board

/// A scene with the auctioneer at the town: hall 36 is the Wolves', hall 39 is for sale (an NPC
/// auction from 1,000 adena that ends in an hour). Everybody stands next to the auctioneer.
fn auction_scene(switch: bool) -> (Scene, ObjectId) {
    let mut s = Scene::new(switch);
    s.load(
        vec![
            hall(36, WOLVES, 500_000, 2, FAR_FUTURE, true),
            hall(39, 0, 100_000, 1, 0, false),
        ],
        vec![auction_record(39, 1000, now_ms() + 3_600_000)],
    );
    let npc = s.spawn(AUCTIONEER, X + 50);
    for (id, clan) in [(P1, LIONS), (P2, LIONS), (P3, WOLVES), (P4, WOLVES), (P5, BEARS)] {
        s.enter(pdata(id, clan, X, 0, 0));
    }
    s.enter(pdata(GM, 0, X, 0, 0));
    s.take();
    (s, npc)
}

/// The auction as the database has it while a read is answered.
fn db_auction(id: i32, bidders: Vec<AuctionBidRecord>) -> AuctionRecord {
    AuctionRecord {
        bidders,
        ..auction_record(id, 1000, now_ms() + 3_600_000)
    }
}

/// `bid <auction> <adena>` of a player, with the two reads answered (the auction as `db`, the clan
/// warehouse as `items`). Returns what came out after the last answer.
fn place_bid(
    s: &mut Scene,
    npc: ObjectId,
    player: i32,
    auction: i32,
    bid: i64,
    db: AuctionRecord,
    items: Vec<ItemData>,
) -> Vec<Out> {
    s.talk(player, npc);
    s.take();
    s.bypass(player, npc, &format!("bid {auction} {bid}"));
    let out = s.take();
    assert!(
        out.iter().any(|o| matches!(
            o,
            Out::Persist(PersistCommand::ResidenceRereadRequest {
                residence_id,
                kind: ResidenceReadKind::Auction,
                then: ResidenceReadThen::AuctionBid { auction_id, bid: b, .. },
            }) if *residence_id == auction && *auction_id == auction && *b == bid
        )),
        "the auction is read before the bid"
    );
    s.answer_auction_read(&out, Some(db));
    let out = s.take();
    if !out
        .iter()
        .any(|o| matches!(o, Out::Persist(PersistCommand::ClanWarehouseRequest { .. })))
    {
        return out;
    }
    s.answer_warehouse_read(&out, items);
    s.take()
}

/// `ClanHallAuction.setBid` and `updateInDB`: the adena comes out of the clan warehouse, the bid
/// row is inserted, the clan remembers the auction, the highest bidder is the clan's leader.
#[test]
fn a_bid_takes_the_adena_from_the_clan_warehouse() {
    let (mut s, npc) = auction_scene(true);
    let out = place_bid(
        &mut s,
        npc,
        P2,
        39,
        1500,
        db_auction(39, vec![]),
        vec![adena_item(0x6000_1000, 100_000, "CLANWH")],
    );
    assert!(message_ids(&out, P2).contains(&SystemMessageId::YourBidHasBeenSuccessfullyPlaced));
    assert_eq!(warehouse_adena(&s, LIONS), 98_500);
    assert!(matches!(
        item_row_ops(&out)[..],
        [ItemRowOp::Update { owner_id: LIONS, .. }]
    ));
    let writes = auction_writes(&out);
    assert_eq!(writes.len(), 2);
    assert!(matches!(
        &writes[0],
        AuctionWrite::BidInsert { auction_id: 39, bidder_id: LIONS, bidder_name, max_bid: 1500, clan_name, .. }
            if bidder_name == "P2" && clan_name == "Lions"
    ));
    assert_eq!(
        writes[1],
        AuctionWrite::ClanBidAt {
            clan_id: LIONS,
            auction_id: 39
        }
    );
    assert_eq!(s.clan(LIONS).auction_bid_at, 39);
    let auction = s.world.clan_hall_auction(39).unwrap();
    assert_eq!(auction.highest_bidder_id, LIONS);
    assert_eq!(auction.highest_bidder_max_bid, 1500);
    // NOTE(java-quirk): the row holds the player's name, the memory the leader's.
    assert_eq!(auction.highest_bidder_name, "P1");
    assert_eq!(auction.bidders[&LIONS].name, "P1");
}

/// Below the starting bid, and below the highest bid: "price must be higher", nothing is taken.
#[test]
fn a_low_bid_is_refused_with_the_message() {
    let (mut s, npc) = auction_scene(true);
    let out = place_bid(
        &mut s,
        npc,
        P2,
        39,
        500,
        db_auction(39, vec![]),
        vec![adena_item(0x6000_1000, 100_000, "CLANWH")],
    );
    assert_eq!(
        message_ids(&out, P2),
        [SystemMessageId::YourBidPriceMustBeHigherThanTheMinimumPriceCurrentlyBeingBid]
    );
    assert!(auction_writes(&out).is_empty() && item_row_ops(&out).is_empty());
    assert_eq!(warehouse_adena(&s, LIONS), 100_000);
    assert_eq!(s.clan(LIONS).auction_bid_at, 0);
}

/// D-50: the bid of another clan on the Java game server counts: this world's copy had no bid.
#[test]
fn the_auction_is_read_again_so_a_bid_of_the_other_server_counts() {
    let (mut s, npc) = auction_scene(true);
    let out = place_bid(
        &mut s,
        npc,
        P2,
        39,
        2000,
        db_auction(39, vec![bid_record(WOLVES, "P3", "Wolves", 5000)]),
        vec![adena_item(0x6000_1000, 100_000, "CLANWH")],
    );
    assert_eq!(
        message_ids(&out, P2),
        [SystemMessageId::YourBidPriceMustBeHigherThanTheMinimumPriceCurrentlyBeingBid]
    );
    assert_eq!(
        s.world.clan_hall_auction(39).unwrap().highest_bidder_max_bid,
        5000
    );
    assert_eq!(warehouse_adena(&s, LIONS), 100_000);
}

/// More than the clan warehouse holds: "not enough adena in the clan hall warehouse".
#[test]
fn a_bid_the_warehouse_cannot_pay_is_refused() {
    let (mut s, npc) = auction_scene(true);
    let out = place_bid(
        &mut s,
        npc,
        P2,
        39,
        200_000,
        db_auction(39, vec![]),
        vec![adena_item(0x6000_1000, 100_000, "CLANWH")],
    );
    assert_eq!(
        message_ids(&out, P2),
        [SystemMessageId::ThereIsNotEnoughAdenaInTheClanHallWarehouse]
    );
    assert!(auction_writes(&out).is_empty());
    assert_eq!(warehouse_adena(&s, LIONS), 100_000);
}

/// A clan whose row names another name than its leader pays the whole bid again (Java compares
/// the highest bidder's name with the leader's), and its row is updated with the leader's name.
#[test]
fn a_row_with_the_players_name_makes_the_clan_pay_the_whole_bid_again() {
    let (mut s, npc) = auction_scene(true);
    let out = place_bid(
        &mut s,
        npc,
        P2,
        39,
        2000,
        db_auction(39, vec![bid_record(LIONS, "P2", "Lions", 1500)]),
        vec![adena_item(0x6000_1000, 98_500, "CLANWH")],
    );
    assert_eq!(warehouse_adena(&s, LIONS), 96_500);
    assert!(matches!(
        &auction_writes(&out)[0],
        AuctionWrite::BidUpdate { auction_id: 39, bidder_id: LIONS, bidder_name, max_bid: 2000, .. }
            if bidder_name == "P1"
    ));
}

/// The row names the leader (a bid raised before): only the difference is taken.
#[test]
fn the_leaders_name_makes_the_clan_pay_only_the_difference() {
    let (mut s, npc) = auction_scene(true);
    let out = place_bid(
        &mut s,
        npc,
        P2,
        39,
        2000,
        db_auction(39, vec![bid_record(LIONS, "P1", "Lions", 1500)]),
        vec![adena_item(0x6000_1000, 100_000, "CLANWH")],
    );
    assert_eq!(warehouse_adena(&s, LIONS), 99_500);
    assert!(matches!(
        &auction_writes(&out)[0],
        AuctionWrite::BidUpdate { max_bid: 2000, .. }
    ));
    assert_eq!(s.world.clan_hall_auction(39).unwrap().bidders.len(), 1);
}

/// A new clan outbids: the old highest bidder's leader hears it, the new row names the player.
#[test]
fn another_clan_outbids_and_the_old_leader_hears_it() {
    let (mut s, npc) = auction_scene(true);
    let out = place_bid(
        &mut s,
        npc,
        P4,
        39,
        3000,
        db_auction(39, vec![bid_record(LIONS, "P1", "Lions", 1500)]),
        vec![adena_item(0x6000_1001, 2_000_000, "CLANWH")],
    );
    assert_eq!(warehouse_adena(&s, WOLVES), 1_997_000);
    assert_eq!(texts_to(&out, P1), ["You have been out bidded"]);
    assert!(matches!(
        &auction_writes(&out)[0],
        AuctionWrite::BidInsert { bidder_id: WOLVES, bidder_name, max_bid: 3000, .. } if bidder_name == "P4"
    ));
    let auction = s.world.clan_hall_auction(39).unwrap();
    assert_eq!(auction.highest_bidder_id, WOLVES);
    assert_eq!(auction.bidders.len(), 2);
}

/// A failed read refuses the bid (D-50): nothing is taken from the memory's copy.
#[test]
fn a_failed_read_refuses_the_bid() {
    let (mut s, npc) = auction_scene(true);
    s.talk(P2, npc);
    s.take();
    s.bypass(P2, npc, "bid 39 1500");
    let out = s.take();
    let then = out
        .iter()
        .find_map(|o| match o {
            Out::Persist(PersistCommand::ResidenceRereadRequest { then, .. }) => Some(then.clone()),
            _ => None,
        })
        .unwrap();
    s.world.handle(WorldCommand::ResidenceReread {
        result: Err(()),
        then,
    });
    assert!(s.take().is_empty());
    assert_eq!(warehouse_adena(&s, LIONS), 100_000);
}

/// The auction is gone from the database ("Invalid bid!"), and from this world.
#[test]
fn a_bid_on_an_auction_the_other_server_ended_is_invalid() {
    let (mut s, npc) = auction_scene(true);
    s.talk(P2, npc);
    s.take();
    s.bypass(P2, npc, "bid 39 1500");
    let out = s.take();
    s.answer_auction_read(&out, None);
    let out = s.take();
    assert_eq!(texts_to(&out, P2), ["Invalid bid!"]);
    assert!(s.world.clan_hall_auction(39).is_none());
}

/// Bad numbers and unknown auctions end in the messages Java catches them with.
#[test]
fn the_bid_command_answers_bad_input_with_javas_messages() {
    let (mut s, npc) = auction_scene(true);
    s.talk(P2, npc);
    s.take();
    s.bypass(P2, npc, "bid x 100");
    s.bypass(P2, npc, "bid 39 abc");
    s.bypass(P2, npc, "bid 77 100");
    let out = s.take();
    assert_eq!(
        texts_to(&out, P2),
        ["Invalid auction!", "Invalid bid!", "Invalid bid!"]
    );
    assert!(
        !out.iter()
            .any(|o| matches!(o, Out::Persist(PersistCommand::ResidenceRereadRequest { .. })))
    );
}

/// `doCancelBid`: the bid row goes, 90% of the bid comes back to the clan warehouse, the clan
/// forgets the auction.
#[test]
fn cancelling_a_bid_gives_back_ninety_percent() {
    let (mut s, npc) = auction_scene(true);
    place_bid(
        &mut s,
        npc,
        P2,
        39,
        1500,
        db_auction(39, vec![]),
        vec![adena_item(0x6000_1000, 100_000, "CLANWH")],
    );
    s.talk(P2, npc);
    s.take();
    s.bypass(P2, npc, "doCancelBid");
    let out = s.take();
    assert!(out.iter().any(|o| matches!(
        o,
        Out::Persist(PersistCommand::ResidenceRereadRequest {
            then: ResidenceReadThen::AuctionCancelBid { auction_id: 39, .. },
            ..
        })
    )));
    s.answer_auction_read(
        &out,
        Some(db_auction(39, vec![bid_record(LIONS, "P2", "Lions", 1500)])),
    );
    let out = s.take();
    s.answer_warehouse_read(&out, vec![adena_item(0x6000_1000, 98_500, "CLANWH")]);
    let out = s.take();
    // `(long) (1500 * 0.9)`.
    assert_eq!(warehouse_adena(&s, LIONS), 98_500 + 1350);
    assert_eq!(
        auction_writes(&out),
        [
            AuctionWrite::BidDelete {
                auction_id: 39,
                bidder_id: LIONS
            },
            AuctionWrite::ClanBidAt {
                clan_id: LIONS,
                auction_id: 0
            }
        ]
    );
    assert!(message_ids(&out, P2).contains(&SystemMessageId::YouHaveCanceledYourBid));
    assert_eq!(s.clan(LIONS).auction_bid_at, 0);
    let auction = s.world.clan_hall_auction(39).unwrap();
    assert_eq!((auction.highest_bidder_id, auction.bidders.len()), (0, 0));
}

/// `ClanTable.destroyClan`: the bid of a dissolved clan is cancelled.
#[test]
fn a_dissolved_clan_loses_its_bid() {
    for write_db in [true, false] {
        let (mut s, npc) = auction_scene(true);
        place_bid(
            &mut s,
            npc,
            P2,
            39,
            1500,
            db_auction(39, vec![]),
            vec![adena_item(0x6000_1000, 100_000, "CLANWH")],
        );
        s.take();
        s.world.destroy_clan(LIONS, write_db);
        let out = s.take();
        // The row goes only with the clan maintenance switch (D-49); the bid always.
        assert_eq!(
            auction_writes(&out).contains(&AuctionWrite::BidDelete {
                auction_id: 39,
                bidder_id: LIONS
            }),
            write_db
        );
        assert!(s.world.clan_hall_auction(39).unwrap().bidders.is_empty());
    }
}

/// `bid1`: only clans of level 2 and up bid, and only one auction, and not with a hall.
#[test]
fn the_bid_page_checks_the_clan() {
    let (mut s, npc) = auction_scene(true);
    for player in [P5, P3, GM, P2] {
        s.talk(player, npc);
    }
    s.take();
    s.bypass(P5, npc, "bid1 39");
    s.bypass(P3, npc, "bid1 39");
    s.bypass(GM, npc, "bid1 39");
    let out = s.take();
    assert_eq!(
        message_ids(&out, P5),
        [SystemMessageId::OnlyAClanLeaderWhoseClanIsOfLevel2OrHigherIsAllowedToParticipateInAClanHallAuction]
    );
    assert_eq!(
        message_ids(&out, P3),
        [SystemMessageId::SinceYouHaveAlreadySubmittedABidYouAreNotAllowedToParticipateInAnotherAuctionAtThisTime]
    );
    assert_eq!(
        message_ids(&out, GM),
        [SystemMessageId::OnlyAClanLeaderWhoseClanIsOfLevel2OrHigherIsAllowedToParticipateInAClanHallAuction]
    );
    // Lions may: the page shows the warehouse's adena and the minimum (the starting bid).
    s.bypass(P2, npc, "bid1 39");
    let page = last_page(&s.take(), P2);
    assert_eq!(field(&page, "PLEDGE_ADENA"), "100000");
    assert_eq!(field(&page, "AGIT_AUCTION_MINBID"), "1000");
    assert_eq!(
        field(&page, "AGIT_LINK_BACK"),
        format!("bypass -h npc_{}_bidding 39", npc.0)
    );
}

/// `bidding`: the page of one auction with the hall's values.
#[test]
fn the_auction_page_shows_the_hall() {
    let (mut s, npc) = auction_scene(true);
    s.talk(P2, npc);
    s.take();
    s.bypass(P2, npc, "bidding 39");
    let page = last_page(&s.take(), P2);
    assert_eq!(field(&page, "AGIT_NAME"), "Auction 39");
    assert_eq!(field(&page, "OWNER_PLEDGE_NAME"), "NPC Clan");
    assert_eq!(field(&page, "OWNER_PLEDGE_MASTER"), "NPC");
    // grade 1 * 10, the lease and the hall's location and description.
    assert_eq!(field(&page, "AGIT_SIZE"), "10");
    assert_eq!(field(&page, "AGIT_LEASE"), "100000");
    assert_eq!(field(&page, "AGIT_LOCATION"), "Gludio");
    assert_eq!(field(&page, "AGIT_AUCTION_DESC"), "Description of hall 39");
    assert_eq!(field(&page, "AGIT_AUCTION_MINBID"), "1000");
    assert_eq!(field(&page, "AGIT_AUCTION_COUNT"), "0");
    let remain = field(&page, "AGIT_AUCTION_REMAIN");
    assert!(
        remain.starts_with("0 hours 59") || remain.starts_with("1 hours 0"),
        "{remain}"
    );
    assert_eq!(
        field(&page, "AGIT_LINK_BIDLIST"),
        format!("bypass -h npc_{}_bidlist 39", npc.0)
    );
    assert_eq!(
        field(&page, "AGIT_LINK_RE"),
        format!("bypass -h npc_{}_bid1 39", npc.0)
    );
    assert_eq!(
        field(&page, "AGIT_LINK_BACK"),
        format!("bypass -h npc_{}_list", npc.0)
    );
    // An unknown id sends the page unchanged (Java only warns); a bad one answers with a message.
    // (The real page has no `%objectId%`; the test pages link everything, so look at the NPC again.)
    s.talk(P2, npc);
    s.take();
    s.bypass(P2, npc, "bidding 77");
    assert_eq!(field(&last_page(&s.take(), P2), "AGIT_NAME"), "%AGIT_NAME%");
    s.talk(P2, npc);
    s.take();
    s.bypass(P2, npc, "bidding x");
    assert_eq!(texts_to(&s.take(), P2), ["Invalid auction!"]);
}

/// `list`: 15 auctions to a page and a link per page.
#[test]
fn the_list_has_fifteen_auctions_to_a_page() {
    let mut s = Scene::new(true);
    let halls: Vec<ClanHallRecord> = (40..=55).map(|id| hall(id, 0, 0, 1, 0, false)).collect();
    let auctions: Vec<AuctionRecord> = (40..=55)
        .map(|id| auction_record(id, 1000 + i64::from(id), now_ms() + 3_600_000))
        .collect();
    s.load(halls, auctions);
    let npc = s.spawn(AUCTIONEER, X + 50);
    s.enter(pdata(P2, LIONS, X, 0, 0));
    s.take();
    s.talk(P2, npc);
    s.take();
    s.bypass(P2, npc, "list");
    let first = field(&last_page(&s.take(), P2), "itemsField");
    assert!(first.contains(&format!("npc_{}_list 1\"> Page 1 </a>", npc.0)));
    assert!(first.contains(&format!("npc_{}_list 2\"> Page 2 </a>", npc.0)));
    assert!(!first.contains(&format!("npc_{}_list 3", npc.0)));
    assert!(first.contains(&format!(
        "<a action=\"bypass -h npc_{}_bidding 40\">Auction 40</a>",
        npc.0
    )));
    assert!(first.contains("bidding 54\">Auction 54</a>"));
    assert!(!first.contains("Auction 55"));
    assert!(first.contains("<td>Gludio</td>"));
    assert!(first.contains("<td>1040</td>"));
    s.bypass(P2, npc, "list 2");
    let second = field(&last_page(&s.take(), P2), "itemsField");
    assert!(second.contains("Auction 55") && !second.contains("Auction 54"));
}

/// M7-FU-03 (D-50): the list is built from `auction` rows read just now, not from memory.
#[test]
fn the_list_is_built_from_rows_read_again() {
    let mut s = Scene::new(true);
    let halls: Vec<ClanHallRecord> = (40..=43).map(|id| hall(id, 0, 0, 1, 0, false)).collect();
    let auctions: Vec<AuctionRecord> = (40..=42)
        .map(|id| auction_record(id, 1000 + i64::from(id), now_ms() + 3_600_000))
        .collect();
    s.load(halls, auctions);
    let npc = s.spawn(AUCTIONEER, X + 50);
    s.enter(pdata(P2, LIONS, X, 0, 0));
    s.take();
    s.talk(P2, npc);
    s.take();
    // The Java game server changed the starting bid of 41, ended 40 (the row is gone) and added 43
    // (a hall the world has no auction for).
    s.hold_list_reads = true;
    s.bypass(P2, npc, "list");
    let out = s.take();
    // Nothing is shown before the rows are in.
    assert!(
        out.iter()
            .all(|o| !matches!(o, Out::Packet(id, body) if *id == P2 && name_of(body) == "NPC_HTML_MESSAGE")),
        "the page waited for the rows"
    );
    assert!(out.iter().any(|o| matches!(
        o,
        Out::Persist(PersistCommand::ResidenceRereadRequest {
            kind: ResidenceReadKind::Auctions,
            then: ResidenceReadThen::AuctionList { .. },
            ..
        })
    )));
    let mut changed = auction_record(41, 7777, now_ms() + 3_600_000);
    changed.item_name = "Changed 41".to_owned();
    let added = auction_record(43, 4343, now_ms() + 3_600_000);
    s.answer_list_read(
        &out,
        Ok(vec![
            auction_record(42, 1042, now_ms() + 3_600_000),
            changed,
            added,
        ]),
    );
    let page = field(&last_page(&s.take(), P2), "itemsField");
    assert!(!page.contains("Auction 40"), "{page}");
    assert!(
        page.contains("Changed 41") && page.contains("<td>7777</td>"),
        "{page}"
    );
    assert!(
        page.contains("Auction 42") && page.contains("Auction 43"),
        "{page}"
    );
    // The world's own copy follows the rows.
    let ids: Vec<i32> = s.world.clan_hall_auctions().iter().map(|a| a.id).collect();
    assert!(!ids.contains(&40) && ids.contains(&43), "{ids:?}");
}

/// M7-FU-03: a read that fails shows no list (never the copy in memory).
#[test]
fn a_failed_read_shows_no_auction_list() {
    let mut s = Scene::new(true);
    s.load(
        vec![hall(40, 0, 0, 1, 0, false)],
        vec![auction_record(40, 1040, now_ms() + 3_600_000)],
    );
    let npc = s.spawn(AUCTIONEER, X + 50);
    s.enter(pdata(P2, LIONS, X, 0, 0));
    s.take();
    s.talk(P2, npc);
    s.take();
    s.hold_list_reads = true;
    s.bypass(P2, npc, "list");
    let out = s.take();
    s.answer_list_read(&out, Err(()));
    assert!(
        s.take()
            .iter()
            .all(|o| !matches!(o, Out::Packet(id, body) if *id == P2 && name_of(body) == "NPC_HTML_MESSAGE"))
    );
    assert!(s.world.clan_hall_auction(40).is_some());
}

/// `bidlist`: the clans that bid with the date of the bid (no padding).
#[test]
fn the_bidder_list_shows_the_bids() {
    let (mut s, npc) = auction_scene(true);
    let mut record = db_auction(39, vec![bid_record(LIONS, "P1", "Lions", 3000)]);
    // 2023-11-15T12:00:00Z: the 15th (or at the edge of the zones, the 16th).
    record.bidders[0].time_bid = 1_700_049_600_000;
    s.talk(P2, npc);
    s.take();
    s.bypass(P2, npc, "bid 39 3000");
    let out = s.take();
    // The bid is refused ("price must be higher"? no: 3000 > 1000) and the read replaced the rows.
    s.answer_auction_read(&out, Some(record));
    s.take();
    s.bypass(P2, npc, "bidlist 39");
    let page = last_page(&s.take(), P2);
    let list = field(&page, "AGIT_LIST");
    assert!(
        list.starts_with("<tr><td>Lions</td><td>P1</td><td>2023/11/1"),
        "{list}"
    );
    assert!(list.ends_with("</td><td>3000</td></tr>"));
    assert_eq!(field(&page, "x"), "39");
}

/// `selectedItems`: what the clan has to do with the board.
#[test]
fn the_overview_page_follows_the_clan() {
    let (mut s, npc) = auction_scene(true);
    for player in [P2, P3, GM] {
        s.talk(player, npc);
    }
    s.take();
    // A clan with nothing, and a player without a clan.
    s.bypass(P2, npc, "selectedItems");
    s.bypass(GM, npc, "selectedItems");
    let out = s.take();
    assert_eq!(
        message_ids(&out, P2),
        [SystemMessageId::ThereAreNoOfferingsIOwnOrIMadeABidFor]
    );
    assert_eq!(
        message_ids(&out, GM),
        [SystemMessageId::YouDoNotMeetTheRequirementsToParticipateInAnAuction]
    );
    // The Wolves own hall 36: the page of the hall.
    s.bypass(P3, npc, "selectedItems");
    let page = last_page(&s.take(), P3);
    assert_eq!(field(&page, "AGIT_NAME"), "Hall 36");
    assert_eq!(field(&page, "AGIT_OWNER_PLEDGE_NAME"), "Wolves");
    assert_eq!(field(&page, "OWNER_PLEDGE_MASTER"), "P3");
    assert_eq!(field(&page, "AGIT_SIZE"), "20");
    assert_eq!(field(&page, "AGIT_LEASE"), "500000");
    // Lions with a bid: the page of the bid.
    place_bid(
        &mut s,
        npc,
        P2,
        39,
        1500,
        db_auction(39, vec![]),
        vec![adena_item(0x6000_1000, 100_000, "CLANWH")],
    );
    s.bypass(P2, npc, "selectedItems");
    let page = last_page(&s.take(), P2);
    assert_eq!(field(&page, "AGIT_NAME"), "Auction 39");
    assert_eq!(field(&page, "AGIT_AUCTION_MYBID"), "1500");
    s.bypass(P2, npc, "cancelBid");
    let page = last_page(&s.take(), P2);
    assert_eq!(field(&page, "AGIT_BID"), "1500");
    assert_eq!(field(&page, "AGIT_BID_REMAIN"), "1350");
    // The member has no privilege for the board: the "not authorized" page.
    s.bypass(P2, npc, "rebid");
    assert!(last_page(&s.take(), P2).starts_with("<html><body>PAGE=auction/not_authorized.htm"));
}

/// The Wolves put hall 36 up for sale: `auction <days> <bid>` prepares it, `confirmAuction` writes
/// it. Java's `days * 86400000` is an int, so 30 days wrap.
#[test]
fn a_clan_puts_its_hall_up_for_sale() {
    let (mut s, npc) = auction_scene(true);
    s.talk(P3, npc);
    s.take();
    let before = now_ms();
    s.bypass(P3, npc, "auction 3 5000");
    let page = last_page(&s.take(), P3);
    assert_eq!(field(&page, "x"), "3");
    assert_eq!(field(&page, "AGIT_AUCTION_MINBID"), "5000");
    assert_eq!(field(&page, "AGIT_AUCTION_MIN"), "5000");
    assert_eq!(field(&page, "AGIT_AUCTION_DESC"), "Description of hall 36");
    assert_eq!(
        field(&page, "AGIT_LINK_BACK"),
        format!("bypass -h npc_{}_sale2", npc.0)
    );
    // dd/MM/yyyy HH:mm
    assert_eq!(field(&page, "AGIT_AUCTION_END").len(), 16);
    // Nothing is on the list yet.
    assert!(s.world.clan_hall_auction(36).is_none());
    s.bypass(P3, npc, "confirmAuction");
    let out = s.take();
    let after = now_ms();
    let writes = auction_writes(&out);
    let [AuctionWrite::AuctionInsert(record)] = &writes[..] else {
        panic!("one auction row: {writes:?}");
    };
    assert_eq!((record.id, record.item_id, record.seller_id), (36, 36, P3));
    assert_eq!(
        (
            record.seller_name.as_str(),
            record.seller_clan_name.as_str(),
            record.item_name.as_str()
        ),
        ("P3", "Wolves", "Hall 36")
    );
    assert_eq!(
        (record.item_type.as_str(), record.starting_bid, record.current_bid),
        ("ClanHall", 5000, 0)
    );
    assert!(record.end_date >= before + 3 * 86_400_000 && record.end_date <= after + 3 * 86_400_000);
    assert_eq!(s.world.clan_hall_auction(36).unwrap().seller_clan_name, "Wolves");
    // A second confirm finds nothing pending.
    s.bypass(P3, npc, "confirmAuction");
    assert_eq!(texts_to(&s.take(), P3), ["Invalid auction"]);
    // Bad input.
    s.bypass(P3, npc, "auction x");
    s.bypass(P3, npc, "auction 3 abc");
    assert_eq!(
        texts_to(&s.take(), P3),
        ["Invalid auction duration!", "Invalid bid!"]
    );
    s.talk(P2, npc);
    s.take();
    s.bypass(P2, npc, "auction 3 5");
    assert_eq!(texts_to(&s.take(), P2), ["Invalid bid!"]);
    // 30 days: `30 * 86400000` wraps in an int.
    s.bypass(P3, npc, "auction 30 100");
    s.take();
    s.bypass(P3, npc, "confirmAuction");
    let writes = auction_writes(&s.take());
    let [AuctionWrite::AuctionInsert(record)] = &writes[..] else {
        panic!("one auction row");
    };
    assert!(record.end_date < now_ms());
}

/// The sale pages need the privilege for the hall auction: the leader has it, a member does not.
#[test]
fn the_sale_pages_need_the_privilege() {
    let (mut s, npc) = auction_scene(true);
    for player in [P3, P4] {
        s.talk(player, npc);
    }
    s.take();
    s.bypass(P4, npc, "sale");
    s.bypass(P4, npc, "cancelAuction");
    let pages = pages_to(&s.take(), P4);
    assert_eq!(pages.len(), 2);
    for page in pages {
        assert!(
            page.html
                .starts_with("<html><body>PAGE=auction/not_authorized.htm")
        );
    }
    s.bypass(P3, npc, "sale");
    let page = last_page(&s.take(), P3);
    assert_eq!(field(&page, "AGIT_DEPOSIT"), "500000");
    assert_eq!(field(&page, "AGIT_PLEDGE_ADENA"), "2000000");
    s.bypass(P3, npc, "sale2");
    let page = last_page(&s.take(), P3);
    assert_eq!(field(&page, "AGIT_LAST_PRICE"), "500000");
    assert_eq!(
        field(&page, "AGIT_LINK_BACK"),
        format!("bypass -h npc_{}_sale", npc.0)
    );
    s.bypass(P3, npc, "cancelAuction");
    assert_eq!(field(&last_page(&s.take(), P3), "AGIT_DEPOSIT"), "500000");
}

/// `doCancelAuction`: the hall owner takes its auction back; the bids are deleted and the bidders
/// without a hall get 90% back.
#[test]
fn the_owner_can_cancel_its_auction() {
    let (mut s, npc) = auction_scene(true);
    s.talk(P3, npc);
    s.take();
    s.bypass(P3, npc, "auction 3 5000");
    s.bypass(P3, npc, "confirmAuction");
    s.take();
    s.bypass(P3, npc, "doCancelAuction");
    let out = s.take();
    let mut record = auction_record(36, 5000, now_ms() + 3_600_000);
    record.seller_id = P3;
    record.seller_clan_name = "Wolves".to_owned();
    record.bidders = vec![bid_record(LIONS, "P1", "Lions", 10_000)];
    s.answer_auction_read(&out, Some(record));
    let out = s.take();
    assert!(s.world.clan_hall_auction(36).is_none());
    assert_eq!(
        auction_writes(&out),
        [
            AuctionWrite::AuctionDelete { item_id: 36 },
            AuctionWrite::BidsDelete { auction_id: 36 },
            AuctionWrite::ClanBidAt {
                clan_id: LIONS,
                auction_id: 0
            },
        ]
    );
    // Lions have no hall: 90% of 10,000 back.
    assert_eq!(warehouse_adena(&s, LIONS), 109_000);
    assert_eq!(texts_to(&out, P3), ["Your auction has been canceled"]);
}

// ---------------------------------------------------------------------------------- the end

/// The auction record for an auction that ends now (a 60 ms task).
fn ending(id: i32, seller: i32, seller_clan: &str, bidders: Vec<AuctionBidRecord>) -> AuctionRecord {
    AuctionRecord {
        seller_id: seller,
        seller_name: if seller == 0 {
            "NPC".to_owned()
        } else {
            "P3".to_owned()
        },
        seller_clan_name: seller_clan.to_owned(),
        bidders,
        ..auction_record(id, 1000, now_ms() + 60)
    }
}

/// A scene whose hall-39 auction is due in 60 ms.
fn ending_scene(switch: bool, record: AuctionRecord) -> Scene {
    let mut s = Scene::new(switch);
    s.load(
        vec![
            hall(36, WOLVES, 500_000, 2, FAR_FUTURE, true),
            hall(39, 0, 100_000, 1, 0, false),
        ],
        vec![record],
    );
    s.enter(pdata(P1, LIONS, X, 0, 0));
    s.enter(pdata(P3, WOLVES, X, 0, 0));
    s.take();
    s
}

/// Runs the end task, answers its read with the same rows (now past due) and returns the output.
fn end_it(s: &mut Scene, record: AuctionRecord) -> Vec<Out> {
    s.run_ms(200);
    let out = s.take();
    assert!(out.iter().any(|o| matches!(
        o,
        Out::Persist(PersistCommand::ResidenceRereadRequest {
            kind: ResidenceReadKind::Auction,
            then: ResidenceReadThen::AuctionEnd(_),
            ..
        })
    )));
    s.answer_auction_read(
        &out,
        Some(AuctionRecord {
            end_date: now_ms() - 1,
            ..record
        }),
    );
    s.take()
}

/// `endAuction`: the highest bidder gets the hall; the losers without a hall get 90% back, a loser
/// with one is only told "Congratulation you have won ClanHall!" (Java's message); the auction and
/// the bids are deleted.
#[test]
fn the_highest_bidder_wins_the_hall() {
    let bidders = vec![
        bid_record(LIONS, "P1", "Lions", 5000),
        bid_record(WOLVES, "P3", "Wolves", 4000),
        bid_record(BEARS, "P5", "Bears", 3000),
    ];
    let record = ending(39, 0, "NPC Clan", bidders);
    let mut s = ending_scene(true, record.clone());
    let out = end_it(&mut s, record);
    assert!(s.world.clan_hall_auction(39).is_none());
    let hall = s.world.clan_hall(ClanHallId(39)).unwrap();
    assert_eq!(hall.owner_id, LIONS);
    assert_eq!(s.clan(LIONS).hideout_id, 39);
    assert_eq!(s.clan(LIONS).auction_bid_at, 0);
    assert_eq!(
        auction_writes(&out),
        [
            AuctionWrite::AuctionDelete { item_id: 39 },
            AuctionWrite::ClanBidAt {
                clan_id: LIONS,
                auction_id: 0
            },
            AuctionWrite::BidsDelete { auction_id: 39 },
            AuctionWrite::ClanBidAt {
                clan_id: WOLVES,
                auction_id: 0
            },
            AuctionWrite::ClanBidAt {
                clan_id: BEARS,
                auction_id: 0
            },
        ]
    );
    // The hall row (owner and paid values) is written by `setOwner`.
    assert!(residence_writes(&out).iter().any(|w| matches!(
        w,
        ResidenceWrite::ClanHall {
            hall_id: 39,
            owner_id: LIONS,
            ..
        }
    )));
    // Bears: no hall, 90% of 3,000 into a new adena stack. Wolves own hall 36: no refund.
    assert_eq!(warehouse_adena(&s, BEARS), 2700);
    assert_eq!(warehouse_adena(&s, WOLVES), 2_000_000);
    assert!(
        item_row_ops(&out)
            .iter()
            .any(|op| matches!(op, ItemRowOp::Insert { owner_id: BEARS, .. }))
    );
    assert_eq!(texts_to(&out, P3), ["Congratulation you have won ClanHall!"]);
}

/// A player's auction pays the seller: 90% of the highest bid and the lease (deposit) back.
#[test]
fn the_seller_gets_the_price_and_the_deposit() {
    let record = ending(39, P3, "Wolves", vec![bid_record(LIONS, "P1", "Lions", 5000)]);
    let mut s = ending_scene(true, record.clone());
    let out = end_it(&mut s, record);
    // 4,500 (90% of 5,000) and the lease of hall 39 (100,000) in full.
    assert_eq!(warehouse_adena(&s, WOLVES), 2_104_500);
    assert_eq!(s.world.clan_hall(ClanHallId(39)).unwrap().owner_id, LIONS);
    assert_eq!(item_row_ops(&out).len(), 2);
}

/// D-51: without `SiegeDatabaseWrites` the end of an auction changes this world only.
#[test]
fn without_the_switch_an_auction_ends_in_this_world_only() {
    let record = ending(
        39,
        P3,
        "Wolves",
        vec![
            bid_record(LIONS, "P1", "Lions", 5000),
            bid_record(BEARS, "P5", "Bears", 3000),
        ],
    );
    let mut s = ending_scene(false, record.clone());
    let out = end_it(&mut s, record);
    assert_eq!(s.world.clan_hall(ClanHallId(39)).unwrap().owner_id, LIONS);
    assert!(s.world.clan_hall_auction(39).is_none());
    assert!(residence_writes(&out).is_empty());
    assert!(item_row_ops(&out).is_empty());
}

/// An NPC auction without a bid goes on for a week (`startAutoTask`, the end date row).
#[test]
fn an_auction_without_bids_is_extended_a_week() {
    let record = ending(39, 0, "NPC Clan", vec![]);
    let mut s = ending_scene(true, record.clone());
    let out = end_it(&mut s, record);
    let auction = s.world.clan_hall_auction(39).unwrap();
    assert!(auction.end_date > now_ms() + WEEK - 5000);
    assert_eq!(
        auction_writes(&out),
        [AuctionWrite::EndDate {
            id: 39,
            end_date: auction.end_date
        }]
    );
}

/// A player's auction without a bid leaves the list; NOTE(java-quirk): the row stays.
#[test]
fn an_unsold_auction_leaves_only_the_list() {
    let record = ending(39, P3, "Wolves", vec![]);
    let mut s = ending_scene(true, record.clone());
    let out = end_it(&mut s, record);
    assert!(s.world.clan_hall_auction(39).is_none());
    assert!(auction_writes(&out).is_empty());
    assert!(s.world.clan_hall(ClanHallId(39)).unwrap().is_free);
}

/// D-50: the Java game server moved the end on, or ended the auction: this world follows.
#[test]
fn the_end_reads_the_auction_again() {
    let record = ending(39, 0, "NPC Clan", vec![bid_record(LIONS, "P1", "Lions", 5000)]);
    // Moved on: the auction stays, nothing is decided.
    let mut s = ending_scene(true, record.clone());
    s.run_ms(200);
    let out = s.take();
    s.answer_auction_read(
        &out,
        Some(AuctionRecord {
            end_date: now_ms() + 3_600_000,
            ..record.clone()
        }),
    );
    assert!(s.take().is_empty());
    assert!(s.world.clan_hall_auction(39).is_some());
    assert!(s.world.clan_hall(ClanHallId(39)).unwrap().is_free);
    // Gone: the auction leaves the list.
    let mut s = ending_scene(true, record);
    s.run_ms(200);
    let out = s.take();
    s.answer_auction_read(&out, None);
    assert!(s.world.clan_hall_auction(39).is_none());
    assert!(s.take().is_empty());
}

/// A read that fails: the end is tried again a minute later, never decided on the memory's copy.
#[test]
fn a_failed_read_delays_the_end() {
    let record = ending(39, 0, "NPC Clan", vec![bid_record(LIONS, "P1", "Lions", 5000)]);
    let mut s = ending_scene(true, record);
    s.run_ms(200);
    let out = s.take();
    let then = out
        .iter()
        .find_map(|o| match o {
            Out::Persist(PersistCommand::ResidenceRereadRequest { then, .. }) => Some(then.clone()),
            _ => None,
        })
        .unwrap();
    s.world.handle(WorldCommand::ResidenceReread {
        result: Err(()),
        then,
    });
    s.run_ms(30_000);
    let is_read = |o: &Out| matches!(o, Out::Persist(PersistCommand::ResidenceRereadRequest { .. }));
    assert!(!s.take().iter().any(is_read));
    s.run_ms(31_000);
    assert!(
        s.take()
            .iter()
            .any(|o| matches!(o, Out::Persist(PersistCommand::ResidenceRereadRequest { .. })))
    );
    assert!(s.world.clan_hall(ClanHallId(39)).unwrap().is_free);
}

// ---------------------------------------------------------------------------------- the manager

/// Hall 36 (grade 2, the Wolves') with the manager and the doorman beside it; P3 (leader, with
/// 10,000 adena), P4 (member without privileges), P1 (another clan), the GM and a clanless player
/// stand in the hall.
fn hall_scene(switch: bool, functions: Vec<FunctionRecord>) -> (Scene, ObjectId, ObjectId) {
    let mut s = Scene::new(switch);
    let mut owned = hall(36, WOLVES, 500_000, 2, FAR_FUTURE, true);
    owned.functions = functions;
    s.load(vec![owned], vec![]);
    let manager = s.spawn(MANAGER, HALL_X + 50);
    let doorman = s.spawn(DOORMAN, HALL_X - 50);
    s.enter(pdata(P3, WOLVES, HALL_X, 10_000, 0));
    s.enter(pdata(P4, WOLVES, HALL_X, 0, 0));
    s.enter(pdata(P1, LIONS, HALL_X, 0, 0));
    s.enter(pdata(GM, 0, HALL_X, 0, 100));
    s.enter(pdata(P5, 0, HALL_X, 0, 0));
    s.take();
    (s, manager, doorman)
}

/// `DoorClick.onAction` and `DlgAnswer` (M7-FU-03): the owner clan of a clan hall is asked whether
/// to open or close the hall's door.
#[test]
fn the_owner_of_a_hall_is_asked_to_open_and_close_its_door() {
    use l2_protocol_game::server::ConfirmDlg;

    let (mut s, _manager, _doorman) = hall_scene(true, vec![]);
    // The door of hall 36 (`clanhall_id`), next to the people in the hall (not in `hall_scene`:
    // another spawn would shift the other tests' random numbers).
    let doors = l2_data::DoorTemplates::from_xml(&format!(
        r#"<list><door id="{HALL_DOOR}" name="hall_door" height="150" baseHpMax="1000" clanhall_id="36"
            pos="{},{Y},{Z}" node1="{},{}" node2="{},{}" node3="{},{}" node4="{},{}" nodeZ="{}"/></list>"#,
        HALL_X + 100,
        HALL_X + 90,
        Y - 50,
        HALL_X + 110,
        Y - 50,
        HALL_X + 110,
        Y + 50,
        HALL_X + 90,
        Y + 50,
        Z - 10
    ))
    .unwrap();
    s.world.spawn_doors(&doors, &s.ids).unwrap();
    let door = s.world.door_by_door_id(HALL_DOOR).unwrap().object_id;
    let click = |s: &mut Scene, who: i32| {
        s.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who).unwrap()),
            input: ClientInput::Action {
                object_id: door.0,
                action_id: 0,
            },
        });
    };
    let dialogs = |out: &[Out], who: i32| -> Vec<SystemMessageId> {
        out.iter()
            .filter_map(|o| match o {
                Out::Packet(to, body) if *to == who && name_of(body) == "CONFIRM_DLG" => {
                    let mut r = PacketReader::new(&body[1..]);
                    Some(ConfirmDlg::decode(&mut r).unwrap().message.id)
                }
                _ => None,
            })
            .collect()
    };
    let answer = |s: &mut Scene, who: i32, id: SystemMessageId, answer: i32| {
        s.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(who).unwrap()),
            input: ClientInput::DlgAnswer {
                message_id: id.id(),
                answer,
                requester_id: 0,
            },
        });
    };

    // Another clan, a clan without the hall and a player without a clan are not asked.
    for who in [P1, P5, GM] {
        click(&mut s, who);
        click(&mut s, who);
    }
    assert!(s.take().iter().all(|o| match o {
        Out::Packet(_, body) => name_of(body) != "CONFIRM_DLG",
        Out::Persist(_) => true,
    }));

    // The first click targets, the second asks (the leader stands next to the door).
    click(&mut s, P3);
    assert!(dialogs(&s.take(), P3).is_empty());
    click(&mut s, P3);
    assert_eq!(
        dialogs(&s.take(), P3),
        [SystemMessageId::WouldYouLikeToOpenTheGate]
    );
    answer(&mut s, P3, SystemMessageId::WouldYouLikeToOpenTheGate, 1);
    assert!(s.world.door(door).unwrap().open);

    click(&mut s, P3);
    assert_eq!(
        dialogs(&s.take(), P3),
        [SystemMessageId::WouldYouLikeToCloseTheGate]
    );
    answer(&mut s, P3, SystemMessageId::WouldYouLikeToCloseTheGate, 0);
    assert!(s.world.door(door).unwrap().open);
    click(&mut s, P3);
    answer(&mut s, P3, SystemMessageId::WouldYouLikeToCloseTheGate, 1);
    assert!(!s.world.door(door).unwrap().open);

    // Too far from the door: the player walks to it instead (no dialog yet).
    s.world
        .tele_to_location(ObjectId(P4), TeleportTarget::at(HALL_X + 1_000, Y, Z));
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(P4).unwrap()),
        input: ClientInput::Appearing,
    });
    s.take();
    click(&mut s, P4);
    click(&mut s, P4);
    assert!(dialogs(&s.take(), P4).is_empty());
}

fn function(function_type: i32, lvl: i32, lease: i32, rate: i64) -> FunctionRecord {
    FunctionRecord {
        function_type,
        lvl,
        lease,
        rate,
        end_time: FAR_FUTURE,
    }
}

fn is_page(html: &str, file: &str) -> bool {
    html.starts_with(&format!("<html><body>PAGE={file};"))
}

/// `showChatWindow`: the owner gets the chamberlain, another clan "of", a stranger "no".
#[test]
fn the_chamberlain_greets_the_owner_the_others_and_strangers() {
    let (mut s, manager, _) = hall_scene(true, vec![]);
    for (player, page) in [
        (P3, "clanHallManager/chamberlain.htm"),
        (GM, "clanHallManager/chamberlain.htm"),
        (P1, "clanHallManager/chamberlain-of.htm"),
        (P5, "clanHallManager/chamberlain-no.htm"),
    ] {
        s.talk(player, manager);
        let out = s.take();
        // ActionFailed first, then the page with the object and npc ids.
        let Some(Out::Packet(_, first)) = out.first() else {
            panic!("no packet");
        };
        assert_eq!(name_of(first), "ACTION_FAIL");
        let html = last_page(&out, player);
        assert!(is_page(&html, page), "{html}");
        assert_eq!(field(&html, "objectId"), manager.0.to_string());
        assert_eq!(field(&html, "npcId"), "35001");
    }
}

/// Only the owner (or a GM) is answered; everybody else gets nothing, not even the generic commands.
#[test]
fn only_the_owner_uses_the_manager() {
    let (mut s, manager, _) = hall_scene(true, vec![]);
    s.talk(P3, manager);
    s.talk(P1, manager);
    s.talk(P5, manager);
    s.take();
    s.bypass(P1, manager, "functions");
    s.bypass(P5, manager, "manage");
    assert!(pages_to(&s.take(), P1).is_empty());
    s.bypass(P3, manager, "functions");
    let page = last_page(&s.take(), P3);
    assert_eq!(field(&page, "xp_regen"), "0");
    assert_eq!(field(&page, "hp_regen"), "0");
    assert_eq!(field(&page, "mp_regen"), "0");
}

/// `functions`: the levels of the regeneration functions, the mirror's page by location and level,
/// the support page with the manager's mana.
#[test]
fn the_functions_pages_show_the_hall() {
    let (mut s, manager, _) = hall_scene(
        true,
        vec![
            function(3, 260, 1000, 86_400_000),
            function(1, 1, 7000, WEEK),
            function(6, 1, 2500, 86_400_000),
        ],
    );
    s.talk(P3, manager);
    s.take();
    s.bypass(P3, manager, "functions");
    let page = last_page(&s.take(), P3);
    assert_eq!(field(&page, "hp_regen"), "260");
    s.bypass(P3, manager, "functions tele");
    // `tele` + location + level.
    assert!(is_page(
        &last_page(&s.take(), P3),
        "clanHallManager/teleGludio1.htm"
    ));
    s.bypass(P3, manager, "functions support");
    let page = last_page(&s.take(), P3);
    assert!(is_page(&page, "clanHallManager/support1.htm"));
    assert_eq!(field(&page, "mp"), "30");
    // No item creation: the "not active" page.
    s.bypass(P3, manager, "functions item_creation 5");
    assert!(is_page(
        &last_page(&s.take(), P3),
        "clanHallManager/chamberlain-nac.htm"
    ));
    s.bypass(P3, manager, "functions back");
    assert!(is_page(
        &last_page(&s.take(), P3),
        "clanHallManager/chamberlain.htm"
    ));
}

/// `door`, `manage_vault` and `banish_foreigner` need their privileges.
#[test]
fn the_hall_commands_need_their_privileges() {
    let (mut s, manager, _) = hall_scene(true, vec![]);
    s.talk(P3, manager);
    s.talk(P4, manager);
    s.take();
    for command in ["door open", "manage_vault", "banish_foreigner list", "manage"] {
        s.bypass(P4, manager, command);
        let page = last_page(&s.take(), P4);
        assert!(
            is_page(&page, "clanHallManager/not_authorized.htm"),
            "{command}: {page}"
        );
    }
    s.bypass(P3, manager, "door open");
    assert!(is_page(
        &last_page(&s.take(), P3),
        "clanHallManager/door-open.htm"
    ));
    s.bypass(P3, manager, "door close");
    assert!(is_page(
        &last_page(&s.take(), P3),
        "clanHallManager/door-close.htm"
    ));
    s.bypass(P3, manager, "door");
    assert!(is_page(&last_page(&s.take(), P3), "clanHallManager/door.htm"));
    s.bypass(P3, manager, "banish_foreigner banish");
    assert!(is_page(&last_page(&s.take(), P3), "clanHallManager/banish.htm"));
    s.bypass(P3, manager, "manage_vault");
    let page = last_page(&s.take(), P3);
    assert!(is_page(&page, "clanHallManager/vault.htm"));
    assert_eq!(field(&page, "rent"), "500000");
    s.bypass(P3, manager, "manage");
    assert!(is_page(&last_page(&s.take(), P3), "clanHallManager/manage.htm"));
}

/// `manage recovery`: the overview of the functions by hall grade, with the current one.
#[test]
fn the_recovery_overview_lists_the_levels_of_the_grade() {
    let (mut s, manager, _) = hall_scene(true, vec![function(3, 260, 1000, 86_400_000)]);
    s.talk(P3, manager);
    s.take();
    s.bypass(P3, manager, "manage recovery");
    let page = last_page(&s.take(), P3);
    let oid = manager.0;
    assert_eq!(
        field(&page, "hp_recovery"),
        "260%</font> (<font color=\"FFAABB\">1000</font>Adena /1 Day)"
    );
    assert!(field(&page, "hp_period").starts_with("Withdraw the fee for the next time at "));
    // Grade 2: 80, 140, 200, 260 with the deactivate link in front.
    let link = |word: &str, level: i32| {
        format!("[<a action=\"bypass -h npc_{oid}_manage recovery edit_{word} {level}\">{level}%</a>]")
    };
    let expected_hp = format!(
        "[<a action=\"bypass -h npc_{oid}_manage recovery hp_cancel\">Deactivate</a>]{}{}{}{}",
        link("hp", 80),
        link("hp", 140),
        link("hp", 200),
        link("hp", 260)
    );
    assert_eq!(field(&page, "change_hp"), expected_hp);
    // No MP or exp function: "none" and the grade 2 links.
    assert_eq!(field(&page, "mp_recovery"), "none");
    assert_eq!(field(&page, "mp_period"), "none");
    assert_eq!(
        field(&page, "change_mp"),
        format!("{}{}{}", link("mp", 5), link("mp", 15), link("mp", 30))
    );
    assert_eq!(
        field(&page, "change_exp"),
        format!(
            "{}{}{}{}",
            link("exp", 5),
            link("exp", 15),
            link("exp", 25),
            link("exp", 40)
        )
    );
}

/// `edit_hp 80`: the page with the cost of the level (Feature.ini) and what to send to buy it.
#[test]
fn the_edit_page_shows_the_cost() {
    let (mut s, manager, _) = hall_scene(true, vec![]);
    s.talk(P3, manager);
    s.take();
    s.bypass(P3, manager, "manage recovery edit_hp 80");
    let page = last_page(&s.take(), P3);
    assert_eq!(field(&page, "name"), "Fireplace (HP Recovery Device)");
    // CH_HPREG3_FEE = 1000 for 80%, every day.
    assert_eq!(field(&page, "cost"), "1000</font>Adena /1 Day</font>)");
    assert_eq!(
        field(&page, "use"),
        "Provides additional HP recovery for clan members in the clan hall.<font color=\"00FFFF\">80%</font>"
    );
    assert_eq!(field(&page, "apply"), "recovery hp 80");
    s.bypass(P3, manager, "manage other edit_tele 2");
    let page = last_page(&s.take(), P3);
    // CH_TELE2_FEE = 14000 every 7 days.
    assert_eq!(field(&page, "cost"), "14000</font>Adena /7 Day</font>)");
    assert_eq!(
        field(&page, "use"),
        "Teleports clan members in a clan hall to the target <font color=\"00FFFF\">Stage 2</font> staging area"
    );
    assert_eq!(field(&page, "apply"), "other tele 2");
    s.bypass(P3, manager, "manage deco edit_fixtures 2");
    let page = last_page(&s.take(), P3);
    assert_eq!(field(&page, "cost"), "4000</font>Adena /3 Day</font>)");
    assert_eq!(field(&page, "apply"), "deco fixtures 2");
    s.bypass(P3, manager, "manage recovery hp_cancel");
    assert_eq!(field(&last_page(&s.take(), P3), "apply"), "recovery hp 0");
}

/// Buying a function: the lease comes from the player's adena, the function exists, the client
/// gets the hall's decorations again, the row is written by the first fee task; buying the same
/// level again says so; level 0 cancels.
#[test]
fn a_function_is_bought_changed_and_cancelled() {
    let (mut s, manager, _) = hall_scene(true, vec![]);
    s.talk(P3, manager);
    s.take();
    s.bypass(P3, manager, "manage recovery hp 80");
    let out = s.take();
    assert!(is_page(
        &last_page(&out, P3),
        "clanHallManager/functions-apply_confirmed.htm"
    ));
    // revalidateDeco: AgitDecoInfo of the hall.
    let deco: AgitDecoInfo = decode(&packets_to(&out, P3, "AGIT_DECO_INFO")[0]);
    assert_eq!(deco.hall_id, 36);
    let function = s
        .world
        .residence_function(ResidenceRef::ClanHall(ClanHallId(36)), 3)
        .unwrap();
    assert_eq!(
        (function.lvl, function.lease, function.rate),
        (80, 1000, 86_400_000)
    );
    // The first fee task runs at once; the player paid, so only the row is written.
    s.run_ms(100);
    let out = s.take();
    assert!(matches!(
        residence_writes(&out)[..],
        [ResidenceWrite::FunctionSave {
            function_type: 3,
            lvl: 80,
            lease: 1000,
            rate: 86_400_000,
            ..
        }]
    ));
    assert_eq!(warehouse_adena(&s, WOLVES), 2_000_000);
    // The same level again.
    s.bypass(P3, manager, "manage recovery hp 80");
    let page = last_page(&s.take(), P3);
    assert!(is_page(&page, "clanHallManager/functions-used.htm"));
    assert_eq!(field(&page, "val"), "80%");
    // Level 0 stops it.
    s.bypass(P3, manager, "manage recovery hp 0");
    let out = s.take();
    assert!(is_page(
        &last_page(&out, P3),
        "clanHallManager/functions-cancel_confirmed.htm"
    ));
    assert!(
        s.world
            .residence_function(ResidenceRef::ClanHall(ClanHallId(36)), 3)
            .is_none()
    );
    assert_eq!(
        residence_writes(&out),
        [ResidenceWrite::FunctionDelete {
            residence: ResidenceRef::ClanHall(ClanHallId(36)),
            function_type: 3
        }]
    );
}

/// A player without the adena gets the "low adena" page, twice (Java sends it twice).
#[test]
fn a_function_without_adena_shows_the_low_adena_page_twice() {
    let (mut s, manager, _) = hall_scene(true, vec![]);
    s.talk(P3, manager);
    s.take();
    // The leader has 10,000 adena; the support level 8 costs 52,000.
    s.bypass(P3, manager, "manage other support 8");
    let out = s.take();
    let pages = pages_to(&out, P3);
    assert_eq!(pages.len(), 2);
    for page in pages {
        assert!(is_page(&page.html, "clanHallManager/low_adena.htm"));
    }
    assert!(
        s.world
            .residence_function(ResidenceRef::ClanHall(ClanHallId(36)), 6)
            .is_none()
    );
    assert!(packets_to(&out, P3, "AGIT_DECO_INFO").is_empty());
}

/// A member without the privilege to manage functions cannot buy.
#[test]
fn the_functions_need_the_manage_privilege() {
    let (mut s, manager, _) = hall_scene(true, vec![]);
    s.talk(P4, manager);
    s.take();
    s.bypass(P4, manager, "manage recovery hp 80");
    assert!(is_page(
        &last_page(&s.take(), P4),
        "clanHallManager/not_authorized.htm"
    ));
    assert!(
        s.world
            .residence_function(ResidenceRef::ClanHall(ClanHallId(36)), 3)
            .is_none()
    );
}

/// The support buff costs the manager's mana unless `AltClanHallMpBuffFree`; an unknown skill is
/// answered with Java's message.
#[test]
fn the_support_buff_needs_mana_or_the_free_setting() {
    let (mut s, manager, _) = hall_scene(true, vec![function(6, 1, 2500, 86_400_000)]);
    s.talk(P3, manager);
    s.take();
    // Skill 5000 costs 50 + 10 mana; the manager has 30.
    s.bypass(P3, manager, "support 5000 1");
    let page = last_page(&s.take(), P3);
    assert!(is_page(&page, "clanHallManager/support-no_mana.htm"));
    assert_eq!(field(&page, "mp"), "30");
    s.bypass(P3, manager, "support 4999 1");
    s.bypass(P3, manager, "support abc");
    assert_eq!(
        texts_to(&s.take(), P3),
        [
            "Invalid skill level, contact your admin!",
            "Invalid skill level, contact your admin!"
        ]
    );
    // Free buffs: the manager casts (no "no mana" page) and the player sees "done".
    s.world.set_clan_hall_settings(ClanHallSettings {
        buff_free: true,
        ..ClanHallSettings::default()
    });
    s.bypass(P3, manager, "support 5000 1");
    let page = last_page(&s.take(), P3);
    assert!(is_page(&page, "clanHallManager/support-done.htm"));
}

/// The mirror teleports a clan member to the place of its stage.
#[test]
fn the_mirror_teleports_the_clan() {
    let (mut s, manager, _) = hall_scene(true, vec![function(1, 1, 7000, WEEK)]);
    s.talk(P3, manager);
    s.take();
    s.bypass(P3, manager, "goto 0");
    let out = s.take();
    assert_eq!(packets_to(&out, P3, "TELEPORT_TO_LOCATION").len(), 1);
    // Without the function nothing happens.
    let (mut s, manager, _) = hall_scene(true, vec![]);
    s.talk(P3, manager);
    s.take();
    s.bypass(P3, manager, "goto 0");
    assert!(packets_to(&s.take(), P3, "TELEPORT_TO_LOCATION").is_empty());
}

// ---------------------------------------------------------------------------------- the doorman

/// `ClanHallDoorman.showChatWindow`: the owner's page (hall 36 has the evolve function), the page
/// for the others with the leader's name, the empty hall's page.
#[test]
fn the_doorman_greets_by_clan() {
    let (mut s, _, doorman) = hall_scene(true, vec![]);
    s.talk(P3, doorman);
    let page = last_page(&s.take(), P3);
    assert!(is_page(&page, "clanHallDoorman/doorman2.htm"));
    assert_eq!(field(&page, "clanname"), "Wolves");
    assert_eq!(field(&page, "objectId"), doorman.0.to_string());
    s.talk(P1, doorman);
    let page = last_page(&s.take(), P1);
    assert!(is_page(&page, "clanHallDoorman/doorman-no.htm"));
    assert_eq!(field(&page, "leadername"), "P3");
    assert_eq!(field(&page, "clanname"), "Wolves");
    s.world.clan_hall_table_set_free(ClanHallId(36));
    s.take();
    s.talk(P1, doorman);
    let page = last_page(&s.take(), P1);
    assert!(is_page(&page, "clanHallDoorman/emptyowner.htm"));
    assert_eq!(field(&page, "hallname"), "Hall 36");
}

/// The doors open and close for the owner only; `tele` and `evolve` too.
#[test]
fn the_doorman_serves_only_the_owner() {
    let (mut s, _, doorman) = hall_scene(true, vec![]);
    s.talk(P3, doorman);
    s.talk(P1, doorman);
    s.take();
    s.bypass(P1, doorman, "open_doors");
    s.bypass(P1, doorman, "tele 0");
    assert!(pages_to(&s.take(), P1).is_empty());
    s.bypass(P3, doorman, "open_doors");
    assert!(is_page(
        &last_page(&s.take(), P3),
        "clanHallDoorman/doorman-opened.htm"
    ));
    s.bypass(P3, doorman, "close_doors");
    assert!(is_page(
        &last_page(&s.take(), P3),
        "clanHallDoorman/doorman-closed.htm"
    ));
    // `PetManager.doEvolve` is not ported: the "no" page.
    s.bypass(P3, doorman, "evolve 1");
    assert!(is_page(
        &last_page(&s.take(), P3),
        "clanHallDoorman/evolve-no.htm"
    ));
    // (The real "no" page has no `%objectId%`; look at the doorman again for the test links.)
    s.talk(P3, doorman);
    s.take();
    s.bypass(P3, doorman, "tele 0");
    assert_eq!(packets_to(&s.take(), P3, "TELEPORT_TO_LOCATION").len(), 1);
}

// ---------------------------------------------------------------------------------- the effects

/// `Formulas.calcHpRegen` / `calcMpRegen`: a member of the owning clan in the hall regenerates
/// `1 + level / 100` times as fast as anybody else.
#[test]
fn the_hall_functions_speed_up_the_regeneration_of_the_owners() {
    let (mut s, _, _) = hall_scene(
        true,
        vec![
            function(3, 100, 1000, 86_400_000),
            function(4, 50, 1000, 86_400_000),
        ],
    );
    for id in [P3, P1] {
        s.world.set_current_hp_mp(ObjectId(id), 10.0, 5.0);
    }
    s.run_ms(3100);
    let gain = |id: i32| {
        let p = s.world.player(ObjectId(id)).unwrap();
        (p.core.cur_hp - 10.0, p.core.cur_mp - 5.0)
    };
    let (owner_hp, owner_mp) = gain(P3);
    let (other_hp, other_mp) = gain(P1);
    assert!(other_hp > 0.0 && other_mp > 0.0, "{other_hp} {other_mp}");
    // Level 100: twice the HP; level 50: one and a half times the MP.
    assert!((owner_hp / other_hp - 2.0).abs() < 1e-9, "{owner_hp} {other_hp}");
    assert!((owner_mp / other_mp - 1.5).abs() < 1e-9, "{owner_mp} {other_mp}");
}

/// `RequestRestartPoint`: the exp-recovery function of the clan's hall.
#[test]
fn the_exp_function_level_is_what_a_resurrection_gives_back() {
    let (s, _, _) = hall_scene(true, vec![function(5, 25, 1000, 86_400_000)]);
    assert_eq!(s.world.clan_hall_restore_exp_level(WOLVES), Some(25));
    assert_eq!(s.world.clan_hall_restore_exp_level(LIONS), None);
}

/// The hall manager is a warehouse NPC: the owner opens the clan warehouse at it (the window
/// waits for fresh rows, M6-SYNC-01); another clan gets nothing.
#[test]
fn the_manager_keeps_the_clan_warehouse() {
    let (mut s, manager, _) = hall_scene(true, vec![]);
    s.talk(P3, manager);
    s.talk(P1, manager);
    s.take();
    s.bypass(P1, manager, "DepositC");
    assert!(
        !s.take()
            .iter()
            .any(|o| matches!(o, Out::Persist(PersistCommand::ClanWarehouseRequest { .. })))
    );
    s.bypass(P3, manager, "DepositC");
    let out = s.take();
    assert!(out.iter().any(|o| matches!(
        o,
        Out::Persist(PersistCommand::ClanWarehouseRequest { clan_id: WOLVES, .. })
    )));
}

/// A hall the Java game server gave to a clan (read again, D-50) starts its rent here too, as
/// `new AuctionableHall(set)` does; a hall that went back to nobody stops paying.
#[test]
fn a_hall_read_again_with_an_owner_starts_its_rent() {
    let mut s = Scene::new(true);
    s.load(vec![hall(39, 0, 100_000, 1, 0, false)], vec![]);
    s.run_ms(100);
    s.take();
    let record = ClanHallRecord {
        owner_id: LIONS,
        paid_until: now_ms() - 10,
        paid: true,
        ..hall(39, 0, 100_000, 1, 0, true)
    };
    s.world.handle(WorldCommand::ResidenceReread {
        result: Ok(ResidenceRereadRows::ClanHall(record)),
        then: ResidenceReadThen::Nothing,
    });
    s.take();
    s.run_ms(100);
    let out = s.take();
    assert_eq!(s.world.clan_hall(ClanHallId(39)).unwrap().owner_id, LIONS);
    assert_eq!(warehouse_adena(&s, LIONS), 0);
    assert!(residence_writes(&out).iter().any(|w| matches!(
        w,
        ResidenceWrite::ClanHall {
            hall_id: 39,
            paid: true,
            ..
        }
    )));
    // Back to nobody: the task finds a free hall and ends without charging.
    s.world.handle(WorldCommand::ResidenceReread {
        result: Ok(ResidenceRereadRows::ClanHall(hall(39, 0, 100_000, 1, 0, false))),
        then: ResidenceReadThen::Nothing,
    });
    s.take();
    s.run_ms(1000);
    assert!(residence_writes(&s.take()).is_empty());
}

/// `RequestAllAgitInfo`: every auctionable hall by id with its owner clan and the clan's leader;
/// the flag is `grade < 1`.
#[test]
fn the_clan_hall_list_names_the_owners() {
    use l2_protocol_game::server::ExShowAgitInfo;
    let (mut s, _) = auction_scene(true);
    s.world.handle(WorldCommand::Input {
        session: SessionId(u64::try_from(P2).unwrap()),
        input: ClientInput::AllAgitInfo,
    });
    let out = s.take();
    let packets = packets_to(&out, P2, "EX_SHOW_AGIT_INFO");
    assert_eq!(packets.len(), 1);
    let list: ExShowAgitInfo = decode(&packets[0]);
    let halls: Vec<(i32, &str, &str, bool)> = list
        .halls
        .iter()
        .map(|h| {
            (
                h.hall_id,
                h.owner_clan_name.as_str(),
                h.leader_name.as_str(),
                h.grade_below_one,
            )
        })
        .collect();
    // Hall 36 (grade 2) is the Wolves'; hall 39 (grade 1) is for sale.
    assert_eq!(halls, [(36, "Wolves", "P3", false), (39, "", "", false)]);
}

// ---------------------------------------------------------------------------------- real pages

/// The datapack's pages (skipped without it): the links the code builds and the ones it replaces
/// in the pages are the ones the client sends, so the bypass validation lets the commands through.
const DIST_GAME: &str = r"C:\l2-rust\l2jmobius\dist\game";

/// Installs the datapack's html; `false` (with a printed note) when it is not there.
fn real_pages(s: &mut Scene) -> bool {
    let root = std::path::Path::new(DIST_GAME);
    if !root.join("data/html/auction/AgitBid1.htm").is_file() {
        eprintln!("skipped: the datapack pages are not at {DIST_GAME}");
        return false;
    }
    s.world.set_html_cache(Arc::new(HtmCache::load(root, false)));
    true
}

#[test]
fn the_real_auction_pages_lead_to_a_bid() {
    let (mut s, npc) = auction_scene(true);
    if !real_pages(&mut s) {
        return;
    }
    s.talk(P2, npc);
    assert!(last_page(&s.take(), P2).contains("selectedItems"));
    s.bypass(P2, npc, "list");
    assert!(last_page(&s.take(), P2).contains("Auction 39"));
    s.bypass(P2, npc, "bidding 39");
    let page = last_page(&s.take(), P2);
    assert!(page.contains("Description of hall 39"), "{page}");
    s.bypass(P2, npc, "bid1 39");
    let page = last_page(&s.take(), P2);
    assert!(page.contains("100000") && page.contains("1000"), "{page}");
    // The bid button: `npc_%objectId%_bid $bidprice` became `npc_<id>_bid 39 $bidprice`.
    s.bypass(P2, npc, "bid 39 1500");
    assert!(s.take().iter().any(|o| matches!(
        o,
        Out::Persist(PersistCommand::ResidenceRereadRequest {
            then: ResidenceReadThen::AuctionBid { bid: 1500, .. },
            ..
        })
    )));
}

#[test]
fn the_real_chamberlain_pages_lead_to_a_purchase() {
    let (mut s, manager, _) = hall_scene(true, vec![]);
    if !real_pages(&mut s) {
        return;
    }
    s.talk(P3, manager);
    assert!(last_page(&s.take(), P3).contains("Clan hall managment"));
    s.bypass(P3, manager, "manage");
    assert!(last_page(&s.take(), P3).contains("Manage Clan Hall"));
    s.bypass(P3, manager, "manage recovery");
    let page = last_page(&s.take(), P3);
    assert!(
        page.contains(&format!("npc_{}_manage recovery edit_hp 80", manager.0)),
        "{page}"
    );
    s.bypass(P3, manager, "manage recovery edit_hp 80");
    let page = last_page(&s.take(), P3);
    // The OK button of the page: `manage %apply%`.
    assert!(
        page.contains(&format!("npc_{}_manage recovery hp 80", manager.0)),
        "{page}"
    );
    s.bypass(P3, manager, "manage recovery hp 80");
    assert!(
        s.world
            .residence_function(ResidenceRef::ClanHall(ClanHallId(36)), 3)
            .is_some()
    );
}
