#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The residence admin words (M7-ADMIN-06): `//clanhall` and its `set`, `del`, doors and
//! `teleportself` words, `//manor`, and the residence names of `//clan_info`.
//! Java: scripts/handlers/chat/commands/admin/{AdminSiege,AdminManor,AdminClan}.java. The expected
//! values come from the Java code named in each test; the pages are small files in a temporary
//! datapack root that print the placeholders the Java code replaces.

use std::sync::{Arc, Mutex};

use l2_data::skill::EnchantSkillGroups;
use l2_data::{
    ArmorSets, EnchantHpBonus, ExperienceLoss, ExperienceTable, Hennas, HtmCache, ItemTemplates, KarmaLoss,
    MapRegions, NpcLoadOptions, NpcTemplates, PlayerTemplates, SkillLoadOptions, SkillTemplates, SkillTrees,
    SpawnData, SpawnLoadOptions, StaticObjectTemplates, TeleporterData, Zones,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::server::{NpcHtmlMessage, SystemMessage, SystemMessageParam, TeleportToLocation};
use l2_protocol_game::{ServerOpcode, SystemMessageId};
use l2_world::clan::{ClanDataRecord, ClanMemberRecord, PlayerClanState};
use l2_world::residence::{CastleRecord, ClanHallRecord, ResidenceLoad, SiegableHallRecord};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

#[derive(Debug, Clone)]
enum Out {
    Packet(i32, Vec<u8>),
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

struct NullOutbox;

impl Outbox for NullOutbox {
    fn persist(&mut self, _command: PersistCommand) {}
    fn ls_event(&mut self, _event: LsEvent) {}
}

/// A class with the levels 1 to 40 and a regeneration of every vital.
fn class_xml() -> String {
    let mut rows = String::new();
    for l in 1..=40 {
        rows.push_str(&format!(
            "<level val=\"{l}\"><hp>{}.5</hp><mp>{}.25</mp><cp>{l}.75</cp>\
<hpRegen>2.0</hpRegen><mpRegen>0.9</mpRegen><cpRegen>1.5</cpRegen></level>",
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
            "<list></list>",
            &EnchantSkillGroups::default(),
            &SkillLoadOptions::default(),
        )),
    }
}

/// The GM panel rights: every word of this card is Master (100) only, as `AdminCommands.xml` says.
fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (100, 0)] {
        rights.add_level(level, child);
    }
    for command in [
        "siege",
        "clanhall",
        "clanhallset",
        "clanhalldel",
        "clanhallopendoors",
        "clanhallclosedoors",
        "clanhallteleportself",
        "manor",
        "clan_info",
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
        item_id == ADENA
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.stats)
    }
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
/// Hall 36 (owned by the Wolves) has its zone and its spawn point this far east.
const HALL_36_SPAWN: (i32, i32, i32) = (X + 20_000, Y + 100, Z);
/// Lions (P1 leader) and Wolves (P3 leader, the owner of hall 36 and of castle 1).
const P1: i32 = 0x6000_0001;
const P3: i32 = 0x6000_0003;
const GM: i32 = 0x6000_0009;
const LIONS: i32 = 500;
const WOLVES: i32 = 600;
/// Random teleport offset of the default `MaxOffsetOnTeleport`.
const OFFSET: i32 = 50;

const PAGES: &[(&str, &str)] = &[
    (
        "admin/clanhall.htm",
        "<html><body>%clanhallName%|%clanhallId%|%clanhallOwner%</body></html>",
    ),
    (
        "admin/siegablehall.htm",
        "<html><body>%clanhallId%|%clanhallName%|%clanhallOwner%</body></html>",
    ),
    (
        "admin/castles.htm",
        "<html><body>%castles%|%siegableHalls%|%clanhalls%|%freeclanhalls%</body></html>",
    ),
    (
        "admin/claninfo.htm",
        "<html><body>%clan_name%|%clan_has_castle%|%clan_has_clanhall%|%clan_has_fortress%|%current_player_name%</body></html>",
    ),
    (
        "admin/manor.htm",
        "<html><body>%status%|%change%|%castleInfo%</body></html>",
    ),
];

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
    let (sx, sy, sz) = HALL_36_SPAWN;
    format!(
        r#"<?xml version="1.0"?>
<list enabled="true">
  <zone name="agit36" type="ClanHallZone" {}<stat name="clanHallId" val="36" /><spawn X="{sx}" Y="{sy}" Z="{sz}" /></zone>
  <zone name="agit21" type="ClanHallZone" {}<stat name="clanHallId" val="21" /><spawn X="{sx}" Y="{sy}" Z="{sz}" /></zone>
</list>"#,
        cuboid(sx, sy, 300),
        cuboid(sx + 5_000, sy, 300),
    )
}

struct Scene {
    world: World,
    log: Log,
    _root: tempfile::TempDir,
}

impl Scene {
    /// The castle 1 (Gludio, the Wolves' castle) and the halls 36 (the Wolves') and 21 (free) and
    /// the siegable hall 62 (free) are loaded before the players enter, as the server loads them.
    fn new() -> Self {
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
        for (page, text) in PAGES {
            write(&format!("data/html/{page}"), text);
        }
        let log = Log::default();
        let mut world = World::new(WorldRng::from_seed(7), Box::new(NullOutbox));
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
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        let npcs = Arc::new(NpcTemplates::from_xml("<list></list>", &NpcLoadOptions::default()).unwrap());
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
        world.set_residence_settings(ResidenceSettings::default());
        world.load_clans(vec![lions(), wolves()], Vec::new());
        world.load_residences(ResidenceLoad {
            castles: vec![CastleRecord {
                id: 1,
                found: true,
                name: "Gludio".to_owned(),
                owner_id: WOLVES,
                time_registration_over: true,
                ..CastleRecord::default()
            }],
            clan_halls: vec![hall(36, WOLVES), hall(21, 0)],
            siegable_halls: vec![SiegableHallRecord {
                id: 62,
                name: "Fortress of Resistance".to_owned(),
                location: "Dion".to_owned(),
                ..SiegableHallRecord::default()
            }],
            ..ResidenceLoad::default()
        });
        log.take();
        Self {
            world,
            log,
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

    /// Everybody online: the Lions leader, the Wolves leader and the GM.
    fn enter_all(&mut self) {
        self.enter(pdata(P1, LIONS, X, 0));
        self.enter(pdata(P3, WOLVES, X + 10, 0));
        self.enter(pdata(GM, 0, X + 20, 100));
        self.log.take();
    }

    /// `//command` of the GM, as the admin panel and the client send it.
    fn admin(&mut self, command: &str) {
        self.world
            .admin_command(ObjectId(GM), &format!("admin_{command}"), false);
    }

    fn take(&mut self) -> Vec<Out> {
        self.log.take()
    }
}

fn clan(id: i32, name: &str, leader: i32, members: &[(i32, i32)], castle_id: i32) -> ClanRecord {
    ClanRecord {
        clan_id: id,
        data: Some(ClanDataRecord {
            name: Some(name.to_owned()),
            level: 5,
            leader_id: leader,
            castle_id,
            ..ClanDataRecord::default()
        }),
        members: members
            .iter()
            .map(|&(object_id, power_grade)| ClanMemberRecord {
                object_id,
                name: format!("P{}", object_id - 0x6000_0000),
                level: 40,
                power_grade,
                ..ClanMemberRecord::default()
            })
            .collect(),
        ..ClanRecord::default()
    }
}

fn lions() -> ClanRecord {
    clan(LIONS, "Lions", P1, &[(P1, 1)], 0)
}

/// The Wolves hold castle 1 and hall 36.
fn wolves() -> ClanRecord {
    clan(WOLVES, "Wolves", P3, &[(P3, 1)], 1)
}

fn hall(id: i32, owner_id: i32) -> ClanHallRecord {
    ClanHallRecord {
        id,
        name: format!("Hall {id}"),
        owner_id,
        lease: 1000,
        desc: format!("Description of hall {id}"),
        location: "Gludio".to_owned(),
        paid_until: 0,
        grade: 0,
        paid: false,
        functions: Vec::new(),
    }
}

fn pdata(id: i32, clan_id: i32, x: i32, access_level: i32) -> PlayerData {
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
        items: vec![],
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

/// The plain text lines (`sendMessage`, `SystemMessageId::S1`-style text) sent to a player.
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

fn pages_to(out: &[Out], to: i32) -> Vec<NpcHtmlMessage> {
    packets_to(out, to, "NPC_HTML_MESSAGE")
        .iter()
        .map(|b| decode::<NpcHtmlMessage>(b))
        .collect()
}

fn last_page(out: &[Out], to: i32) -> String {
    pages_to(out, to).pop().map(|p| p.html).expect("a page was sent")
}

// ---------------------------------------------------------------------------------- tests

/// Java `AdminSiege.showClanHallPage`: `clanhall.htm` with the hall's name, its id and its owner
/// (`None` for a free hall).
#[test]
fn the_hall_page_shows_its_name_id_and_owner() {
    let mut s = Scene::new();
    s.enter_all();
    s.admin("clanhall 36");
    let out = s.take();
    assert_eq!(last_page(&out, GM), "<html><body>Hall 36|36|Wolves</body></html>");
    s.admin("clanhall 21");
    let out = s.take();
    assert_eq!(last_page(&out, GM), "<html><body>Hall 21|21|None</body></html>");
}

/// Java `AdminSiege.showSiegableHallPage`: `siegablehall.htm`; a conquerable hall without an
/// owner reads "No Owner".
#[test]
fn a_conquerable_hall_page_says_no_owner() {
    let mut s = Scene::new();
    s.enter_all();
    s.admin("clanhall 62");
    let out = s.take();
    assert_eq!(
        last_page(&out, GM),
        "<html><body>62|Fortress of Resistance|No Owner</body></html>"
    );
}

/// `//clanhall` without an id is `showCastleSelectPage`: the castles and the halls, owned and free.
#[test]
fn clanhall_alone_shows_the_castle_select_page() {
    let mut s = Scene::new();
    s.enter_all();
    s.admin("clanhall");
    let html = last_page(&s.take(), GM);
    assert!(html.contains("bypass -h admin_siege Gludio"), "{html}");
    assert!(html.contains("bypass -h admin_clanhall 36"), "{html}");
    assert!(html.contains("bypass -h admin_clanhall 21"), "{html}");
    assert!(html.contains("bypass -h admin_chsiege_siegablehall 62"), "{html}");
}

/// A hall id that is not in the table is Java's `NullPointerException` at the first use of the
/// hall, which the dispatcher tells the GM.
#[test]
fn an_unknown_hall_is_a_null_pointer_for_the_gm() {
    let mut s = Scene::new();
    s.enter_all();
    s.admin("clanhall 999");
    let out = s.take();
    assert!(pages_to(&out, GM).is_empty());
    let texts = texts_to(&out, GM);
    assert_eq!(texts.len(), 1, "{texts:?}");
    assert!(texts[0].ends_with("java.lang.NullPointerException"), "{texts:?}");
}

/// `admin_clanhallset`: a free hall goes to the target's clan (`ClanHallTable.setOwner`), and the
/// clan gets the hall id. A clan that has a hall already is refused, and so is an owned hall.
#[test]
fn clanhallset_gives_a_free_hall_to_the_targets_clan() {
    let mut s = Scene::new();
    s.enter_all();
    s.world.set_target(ObjectId(GM), Some(ObjectId(P1)));
    s.admin("clanhallset 21");
    assert_eq!(s.world.clan_hall(ClanHallId(21)).unwrap().owner_id, LIONS);
    assert_eq!(s.world.clan(LIONS).unwrap().hideout_id, 21);
    // The hall is taken now: "This Clan Hall is not free!" (`sendSysMessage`).
    s.admin("clanhallset 21");
    assert_eq!(texts_to(&s.take(), GM), ["This Clan Hall is not free!"]);
    // Hall 36 belongs to the Wolves.
    s.admin("clanhallset 36");
    assert_eq!(texts_to(&s.take(), GM), ["This Clan Hall is not free!"]);
    assert_eq!(s.world.clan_hall(ClanHallId(36)).unwrap().owner_id, WOLVES);
}

/// Java: a clan that has a hall already (`getHideoutId() > 0`) is refused for a free hall too.
#[test]
fn a_clan_with_a_hall_cannot_take_another() {
    let mut s = Scene::new();
    s.enter_all();
    s.world.set_target(ObjectId(GM), Some(ObjectId(P3)));
    s.admin("clanhallset 21");
    let out = s.take();
    assert_eq!(texts_to(&out, GM), ["You have already a Clan Hall!"]);
    assert_eq!(s.world.clan_hall(ClanHallId(21)).unwrap().owner_id, 0);
}

/// `admin_clanhallset` without a target: Java's `INVALID_TARGET`.
#[test]
fn clanhallset_without_a_target_is_refused() {
    let mut s = Scene::new();
    s.enter_all();
    s.admin("clanhallset 21");
    let out = s.take();
    assert_eq!(message_ids(&out, GM), [SystemMessageId::InvalidTarget]);
    assert_eq!(s.world.clan_hall(ClanHallId(21)).unwrap().owner_id, 0);
}

/// `admin_clanhalldel` of an owned auctionable hall: the hall is free, the clan loses its hall id
/// and the hall goes back to the auction (`initNPC`).
#[test]
fn clanhalldel_takes_the_hall_back_and_puts_it_up_for_auction() {
    let mut s = Scene::new();
    s.enter_all();
    s.admin("clanhalldel 36");
    let out = s.take();
    assert!(texts_to(&out, GM).is_empty(), "{:?}", texts_to(&out, GM));
    assert!(s.world.clan_hall_is_free(ClanHallId(36)));
    assert_eq!(s.world.clan(WOLVES).unwrap().hideout_id, 0);
    assert!(
        s.world.clan_hall_auctions().iter().any(|a| a.item_id == 36),
        "the free hall starts its auction"
    );
    // A free hall says so.
    s.admin("clanhalldel 21");
    assert_eq!(texts_to(&s.take(), GM), ["This Clan Hall is already free!"]);
}

/// `admin_clanhalldel` of a conquerable hall: `clanhall.free()`, and the old owner clan loses the
/// hall id. The hall is not put up for auction.
#[test]
fn clanhalldel_frees_a_conquerable_hall_without_an_auction() {
    let mut s = Scene::new();
    s.enter_all();
    s.world.clan_hall_set_owner(ClanHallId(62), WOLVES);
    assert_eq!(s.world.clan(WOLVES).unwrap().hideout_id, 62);
    s.admin("clanhalldel 62");
    s.take();
    assert_eq!(s.world.clan_hall(ClanHallId(62)).unwrap().owner_id, 0);
    assert_eq!(s.world.clan(WOLVES).unwrap().hideout_id, 0);
    assert!(!s.world.clan_hall_auctions().iter().any(|a| a.item_id == 62));
}

/// `admin_clanhallteleportself`: the GM goes to a point of the hall's zone (`getSpawnLoc`) with
/// the random offset of `teleToLocation(loc, true)`.
#[test]
fn teleport_self_takes_the_gm_to_the_hall_zone() {
    let mut s = Scene::new();
    s.enter_all();
    s.admin("clanhallteleportself 36");
    let out = s.take();
    let moved = packets_to(&out, GM, "TELEPORT_TO_LOCATION");
    assert_eq!(moved.len(), 1, "one teleport packet");
    let moved = decode::<TeleportToLocation>(&moved[0]);
    let (sx, sy, _) = HALL_36_SPAWN;
    assert!((moved.x - sx).abs() <= OFFSET, "{moved:?}");
    assert!((moved.y - sy).abs() <= OFFSET, "{moved:?}");
}

/// `admin_clanhallopendoors` and `admin_clanhallclosedoors` with a hall that has no door: nothing
/// is sent and no error comes back.
#[test]
fn the_door_words_of_a_hall_without_doors_do_nothing() {
    let mut s = Scene::new();
    s.enter_all();
    s.admin("clanhallopendoors 36");
    s.admin("clanhallclosedoors 36");
    assert!(texts_to(&s.take(), GM).is_empty());
}

/// `admin_clan_info`: the castle, the clan hall and the fortress of the clan are named (the
/// `CastleManager`, `ClanHallTable` and `FortManager` names), and "No" for what the clan lacks.
#[test]
fn clan_info_names_the_residences_of_the_clan() {
    let mut s = Scene::new();
    s.enter_all();
    s.admin("clan_info P3");
    let html = last_page(&s.take(), GM);
    assert_eq!(html, "<html><body>Wolves|Gludio|Hall 36|No|P3</body></html>");
    // The Lions have none of the three.
    s.admin("clan_info P1");
    let html = last_page(&s.take(), GM);
    assert_eq!(html, "<html><body>Lions|No|No|No|P1</body></html>");
}

/// `admin_manor`: the castle rows of `manor.htm`, the manor cost of each castle for this and the
/// next period (no seed or crop lists here, so 0 Adena).
#[test]
fn manor_lists_every_castle_with_its_costs() {
    let mut s = Scene::new();
    s.enter_all();
    s.admin("manor");
    let html = last_page(&s.take(), GM);
    assert!(
        html.contains(
            "<tr><td>Name:</td><td><font color=008000>Gludio</font></td></tr>\
<tr><td>Current period cost:</td><td><font color=FF9900>0 Adena</font></td></tr>\
<tr><td>Next period cost:</td><td><font color=FF9900>0 Adena</font></td></tr>\
<tr><td><font color=808080>--------------------------</font></td>\
<td><font color=808080>--------------------------</font></td></tr>"
        ),
        "{html}"
    );
    // The status and the change come from the manor period, which is not fixed in this test.
    assert!(html.starts_with("<html><body>"), "{html}");
}
