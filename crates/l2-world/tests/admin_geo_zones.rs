#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The geodata, region, zone, zone builder and path admin commands.
//! Java: scripts/handlers/chat/commands/admin/{AdminGeodata,AdminRegions,AdminZone,
//! AdminZoneBuild,AdminPathNode}.java, managers/ZoneBuildManager.java, util/GeoUtils.java.
//!
//! The pages are small files in a temporary datapack root; one test reads the real pages when the
//! reference folder exists.

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use l2_data::{
    HtmCache, ItemTemplates, MapRegions, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions, Zones,
};
use l2_geo::coords::{geo_x, geo_y};
use l2_geo::{GeoEngine, GeoEngineOptions, GeoLoadSpec, GeoSet, PathRequest, Region};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ConfirmDlg, DeleteObject, ExServerPrimitive, NpcHtmlMessage, PrimitiveColor, ShowBoard, SpawnItem,
    SystemMessage, SystemMessageParam,
};
use l2_world::*;

// ---------------------------------------------------------------------------------- harness

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

fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [(0, 0), (30, 0), (70, 30), (100, 70)] {
        rights.add_level(level, child);
    }
    // The commands of the card at the levels of the shipped `AdminCommands.xml`.
    for (command, level) in [
        ("zone_check", 30),
        ("geomap", 30),
        ("geo_pos", 30),
        ("ge", 100),
        ("geoedit", 100),
        ("zone_visual", 100),
        ("zone_visual_clear", 100),
        ("zone_build", 100),
        ("zone_build_save", 100),
        ("zone_build_toggle", 100),
        ("zone_build_clear", 100),
        ("zone_build_delete", 100),
    ] {
        rights.add_command(command, level, false);
    }
    rights
}

const ITEMS: &str =
    r#"<list><item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item></list>"#;

struct Fake {
    rights: AdminRights,
    stats: StatData,
}

impl StaticData for Fake {
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
        if level >= 70 {
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

fn stat_data() -> StatData {
    let mut levels = String::new();
    for level in 1..=40 {
        levels.push_str(&format!(
            "<level val=\"{level}\"><hp>100</hp><mp>50</mp><cp>10</cp></level>"
        ));
    }
    let xml = format!(
        r#"<list><classId>0</classId><staticData>
<collisionMale><radius>9.0</radius><height>23.0</height></collisionMale>
<collisionFemale><radius>8.0</radius><height>23.5</height></collisionFemale>
</staticData><lvlUpgainData>{levels}</lvlUpgainData></list>"#
    );
    StatData {
        player_templates: Arc::new(l2_data::PlayerTemplates::from_xml(&xml, 85).unwrap()),
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        ..StatData::default()
    }
}

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn decoded<P: Decode>(out: &Sent, to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

fn html_to(out: &Sent, to: i32) -> Vec<NpcHtmlMessage> {
    decoded(out, to, "NPC_HTML_MESSAGE")
}

fn primitives_to(out: &Sent, to: i32) -> Vec<ExServerPrimitive> {
    decoded(out, to, "EX_SERVER_PRIMITIVE")
}

fn boards_to(out: &Sent, to: i32) -> Vec<ShowBoard> {
    decoded(out, to, "SHOW_BOARD")
}

/// The text of every `sendMessage` or SYS line (system message `S1_2` with one string).
fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

fn system_ids_to(out: &Sent, to: i32) -> Vec<SystemMessageId> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter(|m| m.id != SystemMessageId::S12)
        .map(|m| m.id)
        .collect()
}

const X: i32 = -71_338;
const Y: i32 = 258_271;
const Z: i32 = -3_104;
const GM: i32 = 100;
const OTHER: i32 = 1;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="2000" type="Monster" level="3" name="Rat"><stats><vitals hp="62" mp="10"/></stats></npc>
</list>"#;

/// The map tile of `(X, Y)` is 17_25; a town region covers it.
const MAP_REGION: &str = r#"<list enabled="true">
  <region name="test_town" town="Test Town" castle="3" locId="911" bbs="7">
    <respawnPoint X="-84141" Y="244623" Z="-3730" />
    <map X="17" Y="25" />
  </region>
</list>"#;

const ZONES: &str = r#"<list enabled="true">
  <zone id="9001" name="Test Peace" type="PeaceZone" shape="Cuboid" minZ="-3500" maxZ="-2800">
    <node X="-71438" Y="258171" />
    <node X="-71238" Y="258371" />
  </zone>
  <zone id="9002" name="Test Cylinder" type="ScriptZone" shape="Cylinder" minZ="-3500" maxZ="-2800" rad="100">
    <node X="-71338" Y="258271" />
  </zone>
  <zone id="400001" type="ScriptZone" shape="Cuboid" minZ="-3500" maxZ="-2800">
    <node X="-71400" Y="258200" />
    <node X="-71300" Y="258300" />
  </zone>
</list>"#;

const ZONE_PAGE: &str = "<html><body>peace %PEACE% pvp %PVP% town %TOWN% script %SCRIPT% land %LAND% noland %NOLAND% zones: %ZLIST%\
<button action=\"bypass -h admin_zone_check\"><button action=\"bypass -h admin_zone_visual all\">\
<button action=\"bypass -h admin_zone_visual_clear\"></body></html>";

const BUILDER_PAGE: &str = "<html><body>\
<button value=\"%movement%\" action=\"bypass -h admin_zone_build_toggle\">\
<button action=\"bypass -h admin_zone_build_save\"><button action=\"bypass -h admin_zone_build_clear\">\
%paths%</body></html>";

const GEOEDIT_PAGE: &str = "<html><body>%N% %E% %S% %W%\
<button action=\"bypass admin_ge xy_0_0\" back=\"bg_0_0\" fore=\"bg_0_0\">\
<button action=\"bypass admin_ge xy_1_0\" back=\"bg_1_0\" fore=\"bg_1_0\">\
<button action=\"bypass admin_ge xy_0_-1\" back=\"bg_0_-1\" fore=\"bg_0_-1\"></body></html>";

const GEOCELL_PAGE: &str = "<html><body>%N% %E% %S% %W%\
<button action=\"bypass admin_%cmd_n%\" back=\"%bg_n%\"><button action=\"bypass admin_%cmd_e%\" back=\"%bg_e%\">\
<button action=\"bypass admin_%cmd_s%\" back=\"%bg_s%\"><button action=\"bypass admin_%cmd_w%\" back=\"%bg_w%\">\
<button action=\"bypass admin_geoedit\"></body></html>";

struct Setup {
    world: World,
    log: Log,
    root: tempfile::TempDir,
}

fn write(root: &Path, path: &str, text: &str) {
    let file = root.join(path);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(file, text).unwrap();
}

/// `geo`: the engine to install; `zones`: install the zone table and the map region.
fn setup(geo: Option<Arc<GeoEngine>>, zones: bool) -> Setup {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "data/spawns/none.xml",
        r#"<?xml version="1.0"?><list enabled="true"></list>"#,
    );
    write(root.path(), "data/html/admin/zone.htm", ZONE_PAGE);
    write(root.path(), "data/html/admin/zone_builder.htm", BUILDER_PAGE);
    write(root.path(), "data/html/admin/geoedit.htm", GEOEDIT_PAGE);
    write(root.path(), "data/html/admin/geoedit_cell.htm", GEOCELL_PAGE);
    write(root.path(), "data/zones/test.xml", ZONES);
    write(root.path(), "data/mapregion/test.xml", MAP_REGION);
    let log = Log::default();
    let mut settings = WorldSettings::default();
    settings.teleport.offset_on_teleport_enabled = false;
    settings.teleport.max_offset_on_teleport = 0;
    let mut world = World::new(WorldRng::from_seed(11), Box::new(NullOutbox));
    world.set_static_data(Box::new(Fake {
        rights: rights(),
        stats: stat_data(),
    }));
    world.set_settings(settings);
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    if let Some(geo) = geo {
        world.set_geo(geo);
    }
    if zones {
        world.set_zone_data(
            Arc::new(Zones::load(root.path())),
            Arc::new(MapRegions::load(root.path())),
        );
    }
    world.set_zone_build_dir(root.path().join("data").join("zones"));
    Setup { world, log, root }
}

fn player(id: i32, access_level: i32, dx: i32) -> PlayerData {
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
        level: 40,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level,
        x: X + dx,
        y: Y,
        z: Z,
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

impl Setup {
    fn enter(&mut self, id: i32, dx: i32) {
        let level = if id == OTHER { 0 } else { id };
        self.enter_with(player(id, level, dx));
    }

    fn enter_with(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    /// The command as the admin panel runs it, without the confirmation.
    fn run(&mut self, command: &str) {
        self.world.admin_command(ObjectId(GM), command, false);
    }

    fn bypass(&mut self, command: &str) {
        self.input(
            GM,
            ClientInput::Bypass {
                command: command.to_owned(),
            },
        );
    }

    fn click(&mut self, x: i32, y: i32, z: i32) {
        self.input(
            GM,
            ClientInput::MoveTo {
                target_x: x,
                target_y: y,
                target_z: z,
                origin_x: X,
                origin_y: Y,
                origin_z: Z,
                movement_mode: 1,
            },
        );
    }

    fn answer(&mut self, answer: i32) {
        self.input(
            GM,
            ClientInput::DlgAnswer {
                message_id: SystemMessageId::S13.id(),
                answer,
                requester_id: 0,
            },
        );
    }

    fn pos(&self, id: i32) -> (i32, i32, i32) {
        let core = &self.world.player(ObjectId(id)).unwrap().core;
        (core.x, core.y, core.z)
    }

    /// The text the GM got, in order.
    fn said(&self, out: &Sent) -> Vec<String> {
        texts_to(out, GM)
    }
}

// ---------------------------------------------------------------------------------- geodata

/// A region file: every block flat at `height`, except `complex`, which is one complex block
/// whose cells carry the cell word `word`, except cell `odd`, which carries `odd_word`.
fn region_bytes(height: i16, complex: Option<(usize, usize, u16, u16)>) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(65_536 * 3);
    for block in 0..65_536usize {
        match complex {
            Some((index, odd, word, odd_word)) if index == block => {
                bytes.push(1);
                for cell in 0..64 {
                    let w = if cell == odd { odd_word } else { word };
                    bytes.extend_from_slice(&w.to_le_bytes());
                }
            }
            _ => {
                bytes.push(0);
                bytes.extend_from_slice(&height.to_le_bytes());
            }
        }
    }
    bytes
}

/// NSWE bits of the geodata cell words.
const NORTH: u16 = 8;
const EAST: u16 = 1;
const SOUTH: u16 = 4;
const WEST: u16 = 2;

/// The geo cell of the GM and an engine whose region holds it: flat at `Z`, with the GM's own
/// block complex, all its cells open, the GM's own cell closed to the south when `closed_south`.
fn engine_with_cell(closed_south: bool, pathfinding: i32) -> ((i32, i32), Arc<GeoEngine>) {
    let (gx, gy) = (geo_x(X), geo_y(Y));
    let (rx, ry) = (gx / 2048, gy / 2048);
    let block = (((gx >> 3) & 0xFF) * 256 + ((gy >> 3) & 0xFF)) as usize;
    let height = (Z * 2) as i16 as u16 & 0xFFF0;
    let all = NORTH | EAST | SOUTH | WEST;
    let own = if closed_south { all & !SOUTH } else { all };
    // Cell index inside the block: `(geoX % 8) * 8 + (geoY % 8)`.
    let cell = ((gx & 7) * 8 + (gy & 7)) as usize;
    let region = Region::parse(&region_bytes(
        Z as i16,
        Some((block, cell, height | all, height | own)),
    ))
    .unwrap();
    let mut set = GeoSet::new();
    assert!(set.insert(rx as u32, ry as u32, region));
    let options = GeoEngineOptions {
        pathfinding,
        ..GeoEngineOptions::default()
    };
    ((gx, gy), Arc::new(GeoEngine::new(set, options)))
}

#[test]
fn every_command_of_the_handlers_is_registered_or_answers_like_java() {
    let mut s = setup(None, true);
    s.enter(GM, 0);
    s.log.take();
    let registered = [
        "admin_geo_pos",
        "admin_geo_spawn_pos",
        "admin_geo_can_move",
        "admin_geo_can_see",
        "admin_geogrid",
        "admin_geomap",
        "admin_geocell",
        "admin_geoedit",
        "admin_ge",
        "admin_region",
        "admin_regions",
        "admin_show_region",
        "admin_show_regions",
        "admin_showregion",
        "admin_showregions",
        "admin_region_clear",
        "admin_regions_clear",
        "admin_regionclear",
        "admin_regionsclear",
        "admin_zone_check",
        "admin_zone_visual all",
        "admin_zone_visual_clear",
        "admin_zone_build",
        "admin_zone_build_save",
        "admin_zone_build_delete 0",
        "admin_zone_build_toggle",
        "admin_zone_build_clear",
        "admin_path_find",
        // The geodata edits (L-07).
        "admin_geoenablenorth",
        "admin_en",
        "admin_geodisablenorth",
        "admin_dn",
        "admin_geoenablesouth",
        "admin_es",
        "admin_geodisablesouth",
        "admin_ds",
        "admin_geoenableeast",
        "admin_ee",
        "admin_geodisableeast",
        "admin_de",
        "admin_geoenablewest",
        "admin_ew",
        "admin_geodisablewest",
        "admin_dw",
        "admin_geosave",
        "admin_geosaveall",
        "admin_geomap_reload",
    ];
    for command in registered {
        s.run(command);
        let out = s.log.take();
        let missing = s.said(&out).iter().any(|t| t.contains("does not exist"));
        assert!(!missing, "{command} has no handler");
    }
}

// ---------------------------------------------------------------------------------- geodata edits (L-07)

/// The region of [`engine_with_cell`] as a file in `<root>/geodata`, loaded the way the server
/// does (`GeoEngine::load`), with `GeoEditPath` = `<root>/saves`. Returns the GM's cell, the region
/// tile name (`17_25`) and the engine.
fn loaded_engine(root: &Path, closed_south: bool) -> ((i32, i32), String, Arc<GeoEngine>) {
    let ((gx, gy), engine) = engine_with_cell(closed_south, 2);
    let (rx, ry) = (gx / 2048, gy / 2048);
    let tile = format!("{rx}_{ry}");
    let bytes = engine.set().region(rx as u32, ry as u32).unwrap().to_bytes();
    write_bytes(&root.join("geodata").join(format!("{tile}.l2j")), &bytes);
    let spec = GeoLoadSpec::new(root.join("geodata")).with_geoedit_path(root.join("saves"));
    let (engine, report) = GeoEngine::load(&spec, GeoEngineOptions::shipped());
    assert_eq!(report.loaded, 1);
    ((gx, gy), tile, Arc::new(engine))
}

fn write_bytes(path: &Path, bytes: &[u8]) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn open(world: &World, gx: i32, gy: i32, nswe: u16) -> bool {
    world.geo().unwrap().check_nearest_nswe(gx, gy, Z, nswe as u8)
}

#[test]
fn the_edit_commands_open_and_close_one_side_of_a_cell() {
    let ((gx, gy), engine) = engine_with_cell(false, 2);
    let mut s = setup(Some(Arc::clone(&engine)), false);
    s.enter(GM, 0);
    s.log.take();

    // The long forms edit the GM's own cell and send nothing.
    s.run("admin_geodisablesouth");
    assert!(s.log.take().is_empty());
    assert!(!open(&s.world, gx, gy, SOUTH));
    assert!(open(&s.world, gx, gy, NORTH));
    // The engine the world started with (what a running path search holds) is not changed.
    assert!(engine.check_nearest_nswe(gx, gy, Z, SOUTH as u8));
    assert!(!Arc::ptr_eq(s.world.geo().unwrap(), &engine));

    // The short forms take coordinates and show the cell page again (`admin_ge gx gy`).
    s.run(&format!("admin_es {gx} {gy}"));
    assert!(open(&s.world, gx, gy, SOUTH));
    let boards = boards_to(&s.log.take(), GM);
    assert_eq!(boards.len(), 3);
    assert!(
        boards[0]
            .content
            .contains(&format!("bypass admin_ds {gx} {gy}\""))
    );

    for (close, open_again, side) in [
        ("de", "ee", EAST),
        ("dw", "ew", WEST),
        ("ds", "es", SOUTH),
        ("dn", "en", NORTH),
    ] {
        s.run(&format!("admin_{close} {gx} {gy}"));
        assert!(!open(&s.world, gx, gy, side), "{close}");
        let page = &boards_to(&s.log.take(), GM)[0].content;
        assert!(
            page.contains(&format!("bypass admin_{open_again} {gx} {gy}\"")),
            "{page}"
        );
        s.run(&format!("admin_{open_again} {gx} {gy}"));
        assert!(open(&s.world, gx, gy, side), "{open_again}");
        s.log.take();
    }

    // A cell of a flat block becomes a complex block when a side is closed.
    let flat = (gx + 8, gy);
    let region = |w: &World| {
        w.geo()
            .unwrap()
            .region_at(flat.0, flat.1)
            .unwrap()
            .block_type(flat.0, flat.1)
    };
    assert_eq!(region(&s.world), l2_geo::BlockType::Flat);
    s.run(&format!("admin_geoenableeast {} {}", flat.0, flat.1));
    assert_eq!(
        region(&s.world),
        l2_geo::BlockType::Flat,
        "opening a flat cell changes nothing"
    );
    s.run(&format!("admin_geodisableeast {} {}", flat.0, flat.1));
    assert_eq!(region(&s.world), l2_geo::BlockType::Complex);
    assert!(!open(&s.world, flat.0, flat.1, EAST));
    assert!(open(&s.world, flat.0, flat.1, WEST));

    // No region there.
    s.run("admin_en 0 0");
    assert_eq!(s.said(&s.log.take()), ["There is no geodata at this position."]);
    // NOTE(java-quirk): `dn` without coordinates takes its X from `getGeoY(getX())`, a cell of
    // another region (not loaded here).
    s.run("admin_dn");
    assert_eq!(s.said(&s.log.take()), ["There is no geodata at this position."]);
    assert!(open(&s.world, gx, gy, NORTH));
    // `en` without coordinates is the GM's own cell.
    s.run("admin_geodisablenorth 1 2");
    s.log.take();
    s.run(&format!("admin_dn {gx} {gy}"));
    s.run("admin_en");
    assert!(open(&s.world, gx, gy, NORTH));
    // A coordinate that is not a number: Java's NumberFormatException.
    s.log.take();
    s.run("admin_en x");
    let said = s.said(&s.log.take());
    assert_eq!(said.len(), 1);
    assert!(said[0].contains("NumberFormatException"), "{said:?}");
}

#[test]
fn the_path_workers_get_the_edited_geodata_with_the_request() {
    #[derive(Clone, Default)]
    struct Requests(Arc<Mutex<Vec<PathRequest>>>);
    struct Recording(Requests);
    impl PathRequests for Recording {
        fn request(&mut self, request: PathRequest) -> bool {
            self.0.0.lock().unwrap().push(request);
            true
        }
        fn cancel(&mut self, _requester: u64) {}
    }

    let ((gx, gy), engine) = engine_with_cell(false, 2);
    let mut s = setup(Some(Arc::clone(&engine)), false);
    let requests = Requests::default();
    s.world.set_path_requests(Box::new(Recording(requests.clone())));
    s.enter(GM, 0);
    s.log.take();

    // Close the GM's cell to the east: a move east is blocked, so the world asks for a path.
    s.run(&format!("admin_geodisableeast {gx} {gy}"));
    s.click(X + 200, Y, Z);
    let sent = requests.0.lock().unwrap().clone();
    assert_eq!(sent.len(), 1, "the straight line is blocked by the edit");
    let geo = sent[0]
        .geo
        .as_ref()
        .expect("the request carries the world's geodata");
    assert!(Arc::ptr_eq(geo, s.world.geo().unwrap()));
    assert!(!geo.check_nearest_nswe(gx, gy, Z, EAST as u8));
    // The pool's own engine (the one loaded at startup) still has the old data.
    assert!(engine.check_nearest_nswe(gx, gy, Z, EAST as u8));
}

#[test]
fn geosave_writes_the_region_and_geomap_reload_reads_it_back() {
    let mut s = setup(None, false);
    let root = s.root.path().to_path_buf();
    let ((gx, gy), tile, engine) = loaded_engine(&root, false);
    s.world.set_geo(engine);
    s.enter(GM, 0);
    s.log.take();

    s.run("admin_geodisablesouth");
    assert!(!open(&s.world, gx, gy, SOUTH));
    s.run("admin_geosave");
    assert_eq!(
        s.said(&s.log.take()),
        [format!("Saved region {tile} at {tile}.l2j")]
    );
    let saved = std::fs::read(root.join("saves").join(format!("{tile}.l2j"))).unwrap();
    let (rx, ry) = ((gx / 2048) as u32, (gy / 2048) as u32);
    assert_eq!(
        saved,
        s.world.geo().unwrap().set().region(rx, ry).unwrap().to_bytes()
    );
    // The saved file is a valid region with the edit in it.
    assert!(
        !Region::parse(&saved)
            .unwrap()
            .check_nearest_nswe(gx, gy, Z, SOUTH as u8)
    );

    // Reload from GeoDataPath (the GM's tile): the edit is gone.
    s.run("admin_geomap_reload");
    assert_eq!(
        s.said(&s.log.take()),
        [format!("GeoMap: {tile} reloaded successfully.")]
    );
    assert!(open(&s.world, gx, gy, SOUTH));

    // Put the saved file in place and reload by name: the edit is back.
    write_bytes(&root.join("geodata").join(format!("{tile}.l2j")), &saved);
    s.run(&format!("admin_geomap_reload {tile}"));
    assert_eq!(
        s.said(&s.log.take()),
        [format!("GeoMap: {tile} reloaded successfully.")]
    );
    assert!(!open(&s.world, gx, gy, SOUTH));

    // A tile without a file, and arguments Java cannot parse.
    s.run("admin_geomap_reload 12_10");
    assert_eq!(s.said(&s.log.take()), ["Failed to reload GeoMap: 12_10"]);
    for bad in ["17", "a_b", "_25", "17_"] {
        s.run(&format!("admin_geomap_reload {bad}"));
        assert_eq!(
            s.said(&s.log.take()),
            ["Could not parse geomap_reload command parameters."],
            "{bad}"
        );
    }

    // Save everything: the one loaded region, then the count.
    std::fs::remove_dir_all(root.join("saves")).unwrap();
    s.run("admin_geosaveall");
    assert_eq!(
        s.said(&s.log.take()),
        [
            format!("Saved region {tile} at {tile}.l2j"),
            "Saved 1 regions.".to_owned()
        ]
    );
    assert_eq!(
        std::fs::read(root.join("saves").join(format!("{tile}.l2j"))).unwrap(),
        saved
    );
}

#[test]
fn geosave_without_a_region_or_a_save_folder() {
    // An engine without the GM's region but with `GeoEditPath`.
    let mut s = setup(None, false);
    let root = s.root.path().to_path_buf();
    std::fs::create_dir_all(root.join("geodata")).unwrap();
    let spec = GeoLoadSpec::new(root.join("geodata")).with_geoedit_path(root.join("saves"));
    let (engine, _) = GeoEngine::load(&spec, GeoEngineOptions::shipped());
    s.world.set_geo(Arc::new(engine));
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_geosave");
    assert_eq!(s.said(&s.log.take()), ["Could not find region: 17_25"]);
    s.run("admin_geosaveall");
    assert_eq!(s.said(&s.log.take()), ["Saved 0 regions."]);
    // A reload turns the null region into a region when the file appears.
    let (_, tile, _) = loaded_engine(&root, true);
    s.run("admin_geomap_reload");
    assert_eq!(
        s.said(&s.log.take()),
        [format!("GeoMap: {tile} reloaded successfully.")]
    );
    s.run("admin_geosave");
    assert_eq!(
        s.said(&s.log.take()),
        [format!("Saved region {tile} at {tile}.l2j")]
    );

    // No `GeoEditPath` (an engine built without a source): the folder cannot be made.
    let (_, engine) = engine_with_cell(false, 2);
    let mut s = setup(Some(engine), false);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_geosave");
    assert_eq!(s.said(&s.log.take()), ["Could not create output directory."]);
    s.run("admin_geomap_reload");
    assert_eq!(s.said(&s.log.take()), ["Failed to reload GeoMap: 17_25"]);
}

#[test]
fn geo_pos_names_the_cell_and_its_height() {
    // Without geodata: Java's null regions.
    let mut s = setup(None, false);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_geo_pos");
    assert_eq!(s.said(&s.log.take()), ["There is no geodata at this position."]);

    let ((gx, gy), engine) = engine_with_cell(false, 0);
    let mut s = setup(Some(engine), false);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_geo_pos");
    let expected = format!("WorldX: {X}, WorldY: {Y}, WorldZ: {Z}, GeoX: {gx}, GeoY: {gy}, GeoZ: {Z}");
    assert_eq!(s.said(&s.log.take()), std::slice::from_ref(&expected));
    // Java gives `geo_spawn_pos` the same body.
    s.run("admin_geo_spawn_pos");
    assert_eq!(s.said(&s.log.take()), [expected]);
}

#[test]
fn geomap_and_geocell_messages() {
    let ((gx, gy), engine) = engine_with_cell(false, 0);
    let mut s = setup(Some(engine), false);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_geomap");
    // tile 17_25: ((X + 294912) >> 15) + 11 and ((Y + 262144) >> 15) + 10.
    assert_eq!(
        s.said(&s.log.take()),
        ["GeoMap: 17_25 (-98304,229376 to -65537,262143)"]
    );
    s.run("admin_geocell");
    let (cx, cy) = (l2_geo::coords::world_x(gx), l2_geo::coords::world_y(gy));
    assert_eq!(
        s.said(&s.log.take()),
        [format!("GeoCell: {gx}, {gy}. XYZ ({cx}, {cy}, {Z})")]
    );
}

#[test]
fn geo_can_see_and_can_move_ask_for_a_target() {
    let mut s = setup(None, false);
    s.enter(GM, 0);
    s.enter(OTHER, 50);
    s.log.take();
    // No target: INVALID_TARGET, for both.
    s.run("admin_geo_can_see");
    s.run("admin_geo_can_move");
    let out = s.log.take();
    assert_eq!(
        system_ids_to(&out, GM),
        [SystemMessageId::InvalidTarget, SystemMessageId::InvalidTarget]
    );
    s.world.set_target(ObjectId(GM), Some(ObjectId(OTHER)));
    s.log.take();
    s.run("admin_geo_can_see");
    s.run("admin_geo_can_move");
    assert_eq!(s.said(&s.log.take()), ["Can see target.", "Can move beeline."]);
}

#[test]
fn geogrid_draws_41_by_41_cells_in_43_packets_and_off_wipes_them() {
    let ((gx, gy), engine) = engine_with_cell(true, 0);
    let mut s = setup(Some(engine), false);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_geogrid");
    let packets = primitives_to(&s.log.take(), GM);
    assert_eq!(packets.len(), 43);
    for (i, p) in packets.iter().enumerate() {
        assert_eq!(p.name, format!("DebugGrid_{i}"));
        assert_eq!((p.x, p.y, p.z), (X, Y, -16000));
    }
    // 40 cells of 16 lines; the last packet holds the 1681st cell.
    assert_eq!(packets[0].lines().len(), 640);
    assert_eq!(packets[42].lines().len(), 16);
    // The GM's own cell is the 21st column's 21st row: cell index 20 * 41 + 20 = 840, in packet
    // 21, slot 0. Its south arrow is closed (red), the other three open (green).
    let own = &packets[21].lines()[..16];
    assert!(own[..4].iter().all(|l| l.color == PrimitiveColor::GREEN), "north");
    assert!(own[4..8].iter().all(|l| l.color == PrimitiveColor::GREEN), "east");
    assert!(own[8..12].iter().all(|l| l.color == PrimitiveColor::RED), "south");
    assert!(own[12..].iter().all(|l| l.color == PrimitiveColor::GREEN), "west");
    let (cx, cy) = (l2_geo::coords::world_x(gx), l2_geo::coords::world_y(gy));
    assert_eq!((own[0].x, own[0].y, own[0].z), (cx - 1, cy - 7, Z));

    s.run("admin_geogrid off");
    let packets = primitives_to(&s.log.take(), GM);
    assert_eq!(packets.len(), 43);
    assert_eq!(packets[0].lines().len(), 40);
    assert!(
        packets
            .iter()
            .flat_map(|p| p.lines())
            .all(|l| l.color == PrimitiveColor::BLACK && l.z == -16000 && l.end == Some((l.x, l.y, -16000)))
    );
    // Any other word is "on".
    s.run("admin_geogrid on");
    assert_eq!(primitives_to(&s.log.take(), GM)[0].lines().len(), 640);
}

#[test]
fn geoedit_shows_the_cells_around_the_player_on_the_board() {
    let ((gx, gy), engine) = engine_with_cell(true, 0);
    let mut s = setup(Some(engine), false);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_geoedit");
    let out = s.log.take();
    let boards = boards_to(&out, GM);
    // A short page: the page, then two empty parts, which Java sends as the text "null".
    assert_eq!(boards.len(), 3);
    assert!(boards[0].show);
    assert!(boards[0].content.starts_with("101\u{8}<html>"));
    assert_eq!(boards[1].content, "102\u{8}null");
    assert_eq!(boards[2].content, "103\u{8}null");
    let page = &boards[0].content;
    // Heading 0 looks north: the letters stay.
    assert!(page.contains("N E S W"));
    // Cell (0, 0) of the page is the GM's own, closed to the south: the arrow icon.
    assert!(page.contains(&format!(
        "bypass admin_ge {gx} {gy}\" back=\"L2UI_CH3.minibar_arrow\""
    )));
    // The cell east of it (dx 1, dy 0) is open on every side.
    assert!(page.contains(&format!(
        "bypass admin_ge {} {gy}\" back=\"L2UI_CH3.minibar_food\"",
        gx + 1
    )));
    // dy -1 is the cell north of it.
    assert!(page.contains(&format!("bypass admin_ge {gx} {}\"", gy - 1)));
    assert!(!page.contains("xy_") && !page.contains("bg_"));

    // The links of the page are cached for the bypass check: `admin_ge` runs from it.
    s.bypass(&format!("admin_ge {gx} {gy}"));
    let boards = boards_to(&s.log.take(), GM);
    assert_eq!(boards.len(), 3);
    let cell = &boards[0].content;
    // North, east and west are open ("disable" commands), south is closed ("enable").
    assert!(cell.contains(&format!("bypass admin_dn {gx} {gy}\"")), "{cell}");
    assert!(cell.contains(&format!("bypass admin_de {gx} {gy}\"")));
    assert!(cell.contains(&format!("bypass admin_es {gx} {gy}\"")));
    assert!(cell.contains(&format!("bypass admin_dw {gx} {gy}\"")));
    assert!(cell.contains("back=\"L2UI_CH3.minibar_arrow\""));
    // Facing north, every percent sign is gone, the compass reads N E S W.
    assert!(!cell.contains('%'));
    assert!(cell.contains("N E S W"));
    // The Back button leads to `admin_geoedit`, which exists.
    s.bypass("admin_geoedit");
    assert_eq!(boards_to(&s.log.take(), GM).len(), 3);
}

#[test]
fn geoedit_turns_with_the_heading() {
    let (_, engine) = engine_with_cell(false, 0);
    let mut s = setup(Some(engine), false);
    let mut data = player(GM, GM, 0);
    data.heading = 16_384; // east
    s.enter_with(data);
    s.log.take();
    s.run("admin_geoedit");
    let boards = boards_to(&s.log.take(), GM);
    assert!(boards[0].content.contains("E S W N"));
    // The cell page turns the letters and the sides: north of the page is the player's east.
    s.run("admin_ge 100 200");
    let boards = boards_to(&s.log.take(), GM);
    let cell = &boards[0].content;
    assert!(cell.contains("E S W N"));
    // No geodata at (100, 200) of this world: every side is open, so the commands switch off. The
    // button that shows the player's "north" (the east side) is `de`.
    assert!(cell.contains("bypass admin_de 100 200"));
    // A broken number is Java's NumberFormatException.
    s.run("admin_ge x 2");
    assert_eq!(
        s.said(&s.log.take()),
        [
            "Exception during execution of  'admin_ge x 2': java.lang.NumberFormatException: For input string: \"x\""
        ]
    );
}

#[test]
fn geoedit_cuts_a_long_page_into_three_parts() {
    // The board packets of `HtmlUtil.sendCBHtml`: 16250 characters a part.
    let mut s = setup(None, false);
    s.enter(GM, 0);
    let long = format!("<html><body>{}</body></html>", "x".repeat(16_300));
    write(s.root.path(), "data/html/admin/geoedit.htm", &long);
    s.world
        .set_html_cache(Arc::new(HtmCache::load(s.root.path(), false)));
    s.log.take();
    s.run("admin_geoedit");
    let boards = boards_to(&s.log.take(), GM);
    assert_eq!(boards.len(), 3);
    assert_eq!(boards[0].content.chars().count(), "101\u{8}".len() + 16_250);
    assert!(boards[1].content.starts_with("102\u{8}"));
    assert_eq!(boards[2].content, "103\u{8}null");
}

// ---------------------------------------------------------------------------------- regions

#[test]
fn region_draws_the_surrounding_world_regions_and_clear_wipes_them() {
    let mut s = setup(None, false);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_region");
    let packets = primitives_to(&s.log.take(), GM);
    assert_eq!(packets.len(), 1);
    let p = &packets[0];
    assert_eq!(p.name, "DebugRegion");
    assert_eq!((p.x, p.y, p.z), (X, Y, -16000));
    // 27 boxes of 12 lines (the player is deep inside the grid); the player's own is orange, and
    // is the first of the list.
    assert_eq!(p.lines().len(), 27 * 12);
    let orange: Vec<_> = p
        .lines()
        .iter()
        .filter(|l| l.color == PrimitiveColor::ORANGE)
        .collect();
    assert_eq!(orange.len(), 12);
    assert!(p.lines()[..12].iter().all(|l| l.color == PrimitiveColor::ORANGE));
    assert!(p.lines()[12..].iter().all(|l| l.color == PrimitiveColor::GREEN));
    // The own box is 2048 wide and holds the GM.
    let first = &p.lines()[0];
    let end = first.end.unwrap();
    assert_eq!(end.0 - first.x, 2048);
    assert!(first.x <= X && X < end.0);

    for command in ["admin_regions_clear", "admin_regionclear", "admin_region_clear"] {
        s.run(command);
        let packets = primitives_to(&s.log.take(), GM);
        assert_eq!(packets.len(), 1, "{command}");
        assert_eq!(packets[0].lines().len(), 1);
        assert_eq!(packets[0].lines()[0].color, PrimitiveColor::BLACK);
        assert_eq!(packets[0].lines()[0].z, -16000);
    }
}

// ------------------------------------------------------------------------------------- path

#[test]
fn path_find_tells_the_nodes_or_why_not() {
    // No geodata: PathFinding is off.
    let mut s = setup(None, false);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_path_find");
    assert_eq!(s.said(&s.log.take()), ["PathFinding is disabled."]);

    // Geodata with PathFinding 2.
    let (_, engine) = engine_with_cell(false, 2);
    let mut s = setup(Some(engine), false);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_path_find");
    assert_eq!(s.said(&s.log.take()), ["No Target!"]);

    // A target 160 units east on the flat ground: a path with at least its end.
    s.enter(OTHER, 160);
    s.world.set_target(ObjectId(GM), Some(ObjectId(OTHER)));
    s.log.take();
    s.run("admin_path_find");
    let lines = s.said(&s.log.take());
    assert!(!lines.is_empty());
    assert!(
        lines
            .iter()
            .all(|l| l.starts_with("x:") && l.contains(" y:") && l.contains(" z:")),
        "{lines:?}"
    );

    // Only an exact word runs.
    s.run("admin_path_find now");
    assert!(s.log.take().is_empty());

    // A target outside the loaded region: no route.
    let mut out_of_geo = player(2, 0, 0);
    out_of_geo.x = -250_000;
    out_of_geo.y = -250_000;
    s.enter_with(out_of_geo);
    s.world.set_target(ObjectId(GM), Some(ObjectId(2)));
    s.log.take();
    s.run("admin_path_find");
    assert_eq!(s.said(&s.log.take()), ["No Route!"]);
}

// ------------------------------------------------------------------------------------- zones

fn yes() -> &'static str {
    "<font color=\"LEVEL\">YES</font>"
}

#[test]
fn zone_check_shows_the_flags_the_zones_and_the_restart_points() {
    let mut s = setup(None, true);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_zone_check");
    let out = s.log.take();
    let pages = html_to(&out, GM);
    assert_eq!(pages.len(), 1);
    let page = &pages[0].html;
    assert!(
        page.contains(&format!("peace {} pvp NO town NO", yes())),
        "{page}"
    );
    // The ScriptZone sets its flag; the zones are listed: a named one with its id below 300000,
    // the unnamed one by its id only.
    assert!(page.contains(&format!("script {}", yes())));
    assert!(page.contains("land NO noland NO"));
    assert!(page.contains("Test Peace (9001)<br1> "), "{page}");
    assert!(page.contains("Test Cylinder (9002)<br1> "));
    assert!(page.contains("400001 "));
    // Its buttons are cached: the page's own refresh runs.
    let texts = s.said(&out);
    assert_eq!(texts[0], "MapRegion: x:17 y:25 (911)");
    // ((X + 327680) >> 4 >> 11) + 10 and the same for Y.
    assert_eq!(texts[1], "GeoRegion: 17_25");
    assert_eq!(texts[2], "Closest Town: Test Town");
    assert_eq!(texts.len(), 7);
    assert!(texts[3].starts_with("TeleToLocation (Castle): x:"));
    assert!(texts[4].starts_with("TeleToLocation (ClanHall): x:"));
    assert!(texts[5].starts_with("TeleToLocation (SiegeFlag): x:"));
    assert!(
        texts[6].starts_with("TeleToLocation (Town): x:-84141 y:244623 z:-3730"),
        "{}",
        texts[6]
    );

    s.bypass("admin_zone_check");
    assert_eq!(html_to(&s.log.take(), GM).len(), 1);
}

#[test]
fn zone_visual_drops_markers_along_the_border_and_clear_takes_them() {
    let mut s = setup(None, true);
    s.enter(GM, 0);
    s.log.take();
    // The cuboid of id 9001 is 200 x 200: 20 steps of 10 on each axis, two sides each.
    s.run("admin_zone_visual 9001");
    let out = s.log.take();
    let spawned = decoded::<SpawnItem>(&out, GM, "SPAWN_ITEM");
    assert_eq!(spawned.len(), 80);
    // z + 5, the Adena item.
    assert!(spawned.iter().all(|i| i.z == Z + 5 && i.display_id == 57));
    assert_eq!(s.world.ground_item_ids().len(), 80);
    // The markers are no dropped items: the ground item table of the database never gets them.
    assert!(s.world.ground_item_rows().is_empty());

    // `all`: the three zones at the GM (a 200-wide cuboid, a circle of radius 100 with
    // (int)(2 pi 100 / 10) = 62 points, a 100-wide cuboid) and the page.
    s.run("admin_zone_visual all");
    let out = s.log.take();
    assert_eq!(decoded::<SpawnItem>(&out, GM, "SPAWN_ITEM").len(), 80 + 62 + 40);
    assert_eq!(html_to(&out, GM).len(), 1);

    s.run("admin_zone_visual_clear");
    let out = s.log.take();
    assert_eq!(
        decoded::<DeleteObject>(&out, GM, "DELETE_OBJECT").len(),
        80 + 80 + 62 + 40
    );
    assert!(s.world.ground_item_ids().is_empty());
    assert_eq!(html_to(&out, GM).len(), 1);

    // An unknown zone id is Java's NullPointerException; no id is NoSuchElementException.
    s.run("admin_zone_visual 12345");
    assert_eq!(
        s.said(&s.log.take()),
        ["Exception during execution of  'admin_zone_visual 12345': java.lang.NullPointerException"]
    );
    s.run("admin_zone_visual");
    assert_eq!(
        s.said(&s.log.take()),
        ["Exception during execution of  'admin_zone_visual': java.util.NoSuchElementException"]
    );
}

#[test]
fn zone_visual_works_from_the_zone_page() {
    let mut s = setup(None, true);
    s.enter(GM, 0);
    s.run("admin_zone_check");
    s.log.take();
    // The page's buttons: `-h` is stripped by the cache.
    s.bypass("admin_zone_visual all");
    let out = s.log.take();
    assert_eq!(decoded::<SpawnItem>(&out, GM, "SPAWN_ITEM").len(), 182);
    s.bypass("admin_zone_visual_clear");
    assert!(s.world.ground_item_ids().is_empty());
}

// ------------------------------------------------------------------------------ zone builder

fn page_of(out: &Sent) -> String {
    html_to(out, GM).pop().unwrap().html
}

#[test]
fn zone_build_toggle_records_clicks_and_draws_them() {
    let mut s = setup(None, false);
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_zone_build");
    let page = page_of(&s.log.take());
    assert!(page.contains("Enable Draw"));
    assert!(!page.contains("<table"));

    s.bypass("admin_zone_build_toggle");
    let out = s.log.take();
    assert_eq!(s.said(&out), ["Draw zone active. Click on point to record."]);
    assert!(page_of(&out).contains("Disable Draw"));
    assert_eq!(s.world.tele_mode(ObjectId(GM)), 3);

    // A click records instead of moving.
    s.click(X + 100, Y, Z);
    let out = s.log.take();
    assert_eq!(s.pos(GM), (X, Y, Z));
    assert!(!s.world.player(ObjectId(GM)).unwrap().movement.is_moving());
    assert!(names(&out).contains(&"ACTION_FAIL".to_owned()));
    assert_eq!(
        s.said(&out),
        [format!(
            "Point saved [Location] X: {} Y: {Y} Z: {Z} Heading: 0 InstanceId: 0",
            X + 100
        )]
    );
    let drawn = primitives_to(&out, GM);
    assert_eq!(drawn.len(), 1);
    assert_eq!(drawn[0].name, "ZoneBuilder1");
    assert_eq!((drawn[0].x, drawn[0].y, drawn[0].z), (X + 100, Y, 65535 + Z));
    assert_eq!(drawn[0].points().len(), 1);
    assert_eq!(drawn[0].points()[0].name, "0");
    assert_eq!(drawn[0].points()[0].color, PrimitiveColor::RED);
    assert!(drawn[0].points()[0].name_colored);
    // The panel lists the point with its delete button.
    let page = page_of(&out);
    assert!(page.contains(&format!("<td width=120>X: {} Y: {Y} Z: {Z}</td>", X + 100)));
    assert!(page.contains("bypass -h admin_zone_build_delete 0"));

    // The second point adds a green line.
    s.click(X + 100, Y + 100, Z);
    let drawn = primitives_to(&s.log.take(), GM);
    assert_eq!(drawn[0].points().len(), 2);
    assert_eq!(drawn[0].lines().len(), 1);
    assert_eq!(drawn[0].lines()[0].color, PrimitiveColor::GREEN);

    // Toggling off lets the GM move again.
    s.bypass("admin_zone_build_toggle");
    assert_eq!(s.said(&s.log.take()), ["Draw zone disabled. You can move."]);
    assert_eq!(s.world.tele_mode(ObjectId(GM)), 0);
    s.click(X + 100, Y, Z);
    assert!(s.world.player(ObjectId(GM)).unwrap().movement.is_moving());
}

fn names(out: &Sent) -> Vec<String> {
    out.iter().map(|(_, b)| name_of(b)).collect()
}

#[test]
fn zone_build_splits_long_drawings_delete_and_clear() {
    let mut s = setup(None, false);
    s.enter(GM, 0);
    s.run("admin_zone_build_toggle");
    for i in 0..12 {
        s.click(X + 10 + i * 10, Y + i, Z);
    }
    s.log.take();
    // Delete entry 3: 11 points are left, redrawn in two packets (10 and 1 points).
    s.bypass("admin_zone_build_delete 3");
    let out = s.log.take();
    let drawn = primitives_to(&out, GM);
    assert_eq!(drawn.len(), 2);
    assert_eq!(drawn[0].name, "ZoneBuilder1");
    assert_eq!(drawn[1].name, "ZoneBuilder2");
    assert_eq!(drawn[0].points().len(), 10);
    // The second packet starts at the last point of the first, and has the join line.
    assert_eq!(drawn[1].points().len(), 1);
    assert_eq!(drawn[1].lines().len(), 1);
    let page = page_of(&out);
    assert!(page.contains("admin_zone_build_delete 10"));
    assert!(!page.contains("admin_zone_build_delete 11"));
    // Out of range or not a number: nothing changes (Java ignores them, silently for text).
    s.run("admin_zone_build_delete 99");
    assert_eq!(html_to(&s.log.take(), GM).len(), 1);
    s.run("admin_zone_build_delete x");
    assert!(s.log.take().is_empty());

    // Clear: one wipe packet per ten points, then an empty list.
    s.bypass("admin_zone_build_clear");
    let out = s.log.take();
    let wipes = primitives_to(&out, GM);
    assert_eq!(wipes.len(), 2);
    assert_eq!((wipes[0].x, wipes[0].y, wipes[0].z), (X, Y, -16000));
    assert_eq!(wipes[0].name, "ZoneBuilder1");
    assert_eq!(wipes[1].name, "ZoneBuilder2");
    assert_eq!(wipes[0].points()[0].color, PrimitiveColor::GREEN);
    assert_eq!(
        (
            wipes[0].points()[0].x,
            wipes[0].points()[0].y,
            wipes[0].points()[0].z
        ),
        (0, 0, -32768)
    );
    assert!(!page_of(&out).contains("<table"));

    // Deleting down to one point clears the whole list.
    s.click(X + 5, Y, Z);
    s.click(X + 10, Y, Z);
    s.log.take();
    s.run("admin_zone_build_delete 0");
    let out = s.log.take();
    assert_eq!(primitives_to(&out, GM).len(), 1);
    assert!(!page_of(&out).contains("<table"));
}

#[test]
fn zone_build_save_asks_and_writes_the_zone_file() {
    let mut s = setup(None, false);
    s.enter(GM, 0);
    // Nothing recorded: the answer is "No path entries to save."
    s.bypass("zone_build");
    s.run("admin_zone_build_save");
    let out = s.log.take();
    let dialogs = decoded::<ConfirmDlg>(&out, GM, "CONFIRM_DLG");
    assert_eq!(dialogs.len(), 1);
    assert_eq!(dialogs[0].message.id, SystemMessageId::S13);
    assert_eq!(
        dialogs[0].message.params,
        [SystemMessageParam::Text(
            "Do you wish to save the zone?".to_owned()
        )]
    );
    s.answer(1);
    assert_eq!(s.said(&s.log.take()), ["No path entries to save."]);

    s.run("admin_zone_build_toggle");
    s.click(100, 200, -3000);
    s.click(300, 400, -3100);
    s.click(500, 100, -3200);
    s.log.take();

    // A no forgets the question and keeps the points.
    s.run("admin_zone_build_save");
    s.answer(0);
    let out = s.log.take();
    assert!(s.said(&out).is_empty());
    assert!(page_of_or_none(&out).is_none());

    // A yes writes `<name>-<millis>.xml` and clears.
    s.run("admin_zone_build_save");
    s.log.take();
    s.answer(1);
    let out = s.log.take();
    let said = s.said(&out);
    assert_eq!(said.len(), 1);
    assert!(
        said[0].starts_with("Zone saved at data/zones/P100-"),
        "{}",
        said[0]
    );
    let dir = s.root.path().join("data").join("zones");
    let file = std::fs::read_dir(&dir)
        .unwrap()
        .map(|e| e.unwrap().path())
        .find(|p| p.file_name().unwrap().to_string_lossy().starts_with("P100-"))
        .unwrap();
    let xml = std::fs::read_to_string(&file).unwrap();
    let stem = file.file_stem().unwrap().to_string_lossy().into_owned();
    let millis = stem.strip_prefix("P100-").unwrap();
    // The Z band is +-1000 around the first point.
    assert!(xml.starts_with("<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<list enabled=\"true\""));
    assert!(xml.contains(&format!(
        "\t<zone name=\"P100_Zone_{millis}\" type=\"ScriptZone\" shape=\"NPoly\" minZ=\"-4000\" maxZ=\"-2000\">\n"
    )));
    assert!(xml.contains("\t\t<node X=\"100\" Y=\"200\" />\n\t\t<node X=\"300\" Y=\"400\" />\n\t\t<node X=\"500\" Y=\"100\" />\n"));
    assert!(xml.ends_with("\t</zone>\n</list>"));
    // `admin_zone_build_clear` ran after the save: the wipe packets are out.
    assert_eq!(primitives_to(&out, GM).len(), 1);
    // And the xml loads as a zone.
    let zones = Zones::load(s.root.path());
    assert!(zones.iter().any(|z| z.zone_type == "ScriptZone" && z.name.as_deref() == Some(&format!("P100_Zone_{millis}"))));
}

fn page_of_or_none(out: &Sent) -> Option<NpcHtmlMessage> {
    html_to(out, GM).pop()
}

#[test]
fn zone_build_save_to_a_missing_folder_fails_like_java() {
    let mut s = setup(None, false);
    s.enter(GM, 0);
    s.world
        .set_zone_build_dir(PathBuf::from("/this/folder/does/not/exist"));
    s.run("admin_zone_build_toggle");
    s.click(1, 2, 3);
    s.run("admin_zone_build_save");
    s.log.take();
    s.answer(1);
    let said = s.said(&s.log.take());
    assert_eq!(said.len(), 1);
    assert!(said[0].starts_with("Failed to save zone: "), "{}", said[0]);
}

#[test]
fn the_builder_forgets_a_gm_who_left() {
    let mut s = setup(None, false);
    s.enter(GM, 0);
    s.run("admin_zone_build_toggle");
    s.click(1, 2, 3);
    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(GM).unwrap()),
    });
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_zone_build");
    // The new `Player` has no tele mode and no points.
    assert_eq!(s.world.tele_mode(ObjectId(GM)), 0);
    assert!(!page_of(&s.log.take()).contains("<table"));
}

// ------------------------------------------------------------------------------ real pages

#[test]
fn the_real_pages_have_no_placeholder_left() {
    let root = PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game"));
    if !root
        .join("data")
        .join("html")
        .join("admin")
        .join("geoedit.htm")
        .is_file()
    {
        eprintln!("skipped: {} has no admin pages", root.display());
        return;
    }
    let (_, engine) = engine_with_cell(true, 0);
    let mut s = setup(Some(engine), true);
    s.world.set_html_cache(Arc::new(HtmCache::load(&root, false)));
    s.enter(GM, 0);
    s.log.take();
    s.run("admin_geoedit");
    let boards = boards_to(&s.log.take(), GM);
    // The real page is longer than one board part: the parts are cut at 16250 characters.
    assert_eq!(boards.len(), 3);
    let page: String = boards
        .iter()
        .enumerate()
        .map(|(i, b)| {
            b.content
                .strip_prefix(&format!("10{}\u{8}", i + 1))
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>()
        .join("");
    let page = &page;
    assert!(!page.contains("xy_") && !page.contains("bg_") && !page.contains("%N%"));
    // Every cell button of the template is a link `admin_ge <gx> <gy>` with numbers.
    let template = std::fs::read_to_string(root.join("data/html/admin/geoedit.htm")).unwrap();
    let links = page.matches("bypass admin_ge ").count();
    assert!(links > 100);
    assert_eq!(links, template.matches("bypass admin_ge xy_").count());
    s.run("admin_ge 30000 30000");
    let cell = &boards_to(&s.log.take(), GM)[0].content;
    assert!(!cell.contains('%') && !cell.contains("cmd_") && !cell.contains("bg_"));

    s.run("admin_zone_build");
    let page = page_of(&s.log.take());
    assert!(page.contains("admin_zone_build_toggle"));
    s.run("admin_zone_check");
    let page = page_of(&s.log.take());
    assert!(!page.contains('%'), "{page}");
}
