#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The combat admin commands (M4-ADMIN-02): `//res`, `//res_monster`, `//kill`, `//kill_monster`
//! and the exp/sp handlers, with a fake GM, a fake player and a fake monster.
//! Java: scripts/handlers/chat/commands/admin/{AdminRes,AdminKill,AdminExpSp}.java.
//!
//! The harness is the one of `admin.rs` (the framework and the core commands).

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{HtmCache, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{Die, SystemMessage, SystemMessageParam};
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

/// The levels of the shipped `AccessLevels.xml` (only 70 and 100 are GMs) and the rights of the
/// commands these tests use, with the levels of the shipped `AdminCommands.xml`.
fn rights() -> AdminRights {
    let mut rights = AdminRights::default();
    for (level, child) in [
        (-1, 0),
        (0, 0),
        (10, 0),
        (20, 10),
        (30, 20),
        (40, 30),
        (50, 40),
        (60, 50),
        (70, 60),
        (100, 70),
    ] {
        rights.add_level(level, child);
    }
    for (command, level, confirm) in [
        ("admin", 30, false),
        ("admin1", 30, false),
        ("admin2", 30, false),
        ("admin3", 30, false),
        ("admin6", 30, false),
        ("show_spawns", 100, false),
        ("spawn", 100, false),
        ("spawnat", 100, false),
        ("spawn_monster", 100, false),
        ("spawn_once", 100, false),
        ("list_positions", 100, false),
        ("list_spawns", 100, false),
        ("show_moves", 100, false),
        ("show_teleport", 30, false),
        ("teleport_to_character", 30, false),
        ("teleportto", 30, false),
        ("move_to", 100, false),
        ("teleport_character", 100, false),
        ("recall", 100, false),
        ("tele", 30, false),
        ("gonorth", 30, false),
        ("goup", 30, false),
        ("goto", 30, false),
        ("kill", 100, false),
        ("kill_monster", 100, false),
        ("res", 100, false),
        ("res_monster", 100, false),
        ("add_exp_sp_to_character", 100, false),
        ("add_exp_sp", 100, false),
        ("remove_exp_sp", 100, false),
        ("heal", 100, false),
    ] {
        rights.add_command(command, level, confirm);
    }
    rights
}

struct Fake {
    rights: AdminRights,
    stats: StatData,
}

impl Fake {
    fn new(rights: AdminRights) -> Self {
        Self {
            rights,
            stats: class_data(),
        }
    }
}

/// The class template of the test characters (class 0, level 40): 100 HP, 50 MP and 10 CP, the
/// values their character rows store. No `statBonus.xml`: every bonus is 1.
fn class_data() -> StatData {
    let mut levels = String::new();
    for level in 1..=40 {
        let (hp, mp, cp) = if level == 40 {
            (100, 50, 10)
        } else {
            (level, level, level)
        };
        levels.push_str(&format!(
            "<level val=\"{level}\"><hp>{hp}</hp><mp>{mp}</mp><cp>{cp}</cp></level>"
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
        ..StatData::default()
    }
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

fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

fn names_to(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, _)| *t == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

fn decoded<P: Decode>(out: &Sent, to: i32, name: &str) -> Vec<P> {
    out.iter()
        .filter(|(t, b)| *t == to && name_of(b) == name)
        .map(|(_, b)| decode::<P>(b))
        .collect()
}

/// The text of every `sendMessage` (system message `S1_2` with one string) sent to `to`.
fn texts_to(out: &Sent, to: i32) -> Vec<String> {
    decoded::<SystemMessage>(out, to, "SYSTEM_MESSAGE")
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] if m.id == SystemMessageId::S12 => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The ids of the system messages (those with no string) sent to `to`.
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
const USER: i32 = 1;

const NPC_TEMPLATES: &str = r#"<?xml version="1.0"?><list>
  <npc id="1001" type="Folk" level="10" name="Villager" usingServerSideName="true"/>
  <npc id="2000" type="Monster" level="3" name="Rat">
    <stats><vitals hp="62" mp="10"/></stats>
  </npc>
  <npc id="2001" type="Monster" level="5" name="Wolf Pup">
    <stats><vitals hp="100" mp="20"/></stats>
  </npc>
</list>"#;

struct Root(tempfile::TempDir);

impl Root {
    fn new() -> Self {
        let dir = tempfile::tempdir().unwrap();
        let write = |path: &str, text: &str| {
            let file = dir.path().join(path);
            std::fs::create_dir_all(file.parent().unwrap()).unwrap();
            std::fs::write(file, text).unwrap();
        };
        write(
            "data/spawns/none.xml",
            r#"<?xml version="1.0"?><list enabled="true"></list>"#,
        );
        write(
            "data/html/admin/main_menu.htm",
            "<html><title>Main Admin Menu</title><body>\
             <button value=\"Char\" action=\"bypass admin_admin6\">\
             <button value=\"Heal\" action=\"bypass -h admin_heal\"></body></html>",
        );
        write(
            "data/html/admin/char_menu.htm",
            "<html><body>Character menu</body></html>",
        );
        write(
            "data/html/admin/game_menu.htm",
            "<html><body>Game menu</body></html>",
        );
        write(
            "data/html/admin/spawns.htm",
            "<html><body>Spawn panel</body></html>",
        );
        write(
            "data/html/admin/teleports.htm",
            "<html><body>Teleport panel</body></html>",
        );
        write("data/html/admin/move.htm", "<html><body>Move panel</body></html>");
        Self(dir)
    }

    fn path(&self) -> &Path {
        self.0.path()
    }
}

struct Setup {
    world: World,
    log: Log,
    _root: Root,
}

fn setup() -> Setup {
    setup_with(WorldSettings::default())
}

fn setup_with(mut settings: WorldSettings) -> Setup {
    // Exact places: the random offset of a teleport has its own tests.
    settings.teleport.offset_on_teleport_enabled = false;
    settings.teleport.max_offset_on_teleport = 0;
    let root = Root::new();
    let log = Log::default();
    let mut world = World::new(WorldRng::from_seed(11), Box::new(NullOutbox));
    world.set_static_data(Box::new(Fake::new(rights())));
    world.set_settings(settings);
    world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
    let npcs = Arc::new(NpcTemplates::from_xml(NPC_TEMPLATES, &NpcLoadOptions::default()).unwrap());
    let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
    world.set_npc_data(npcs, spawns);
    world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
    Setup {
        world,
        log,
        _root: root,
    }
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
        heading: 321,
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
    /// A player whose object id is its access level (or 1 for the plain user).
    fn enter(&mut self, id: i32, dx: i32) {
        let level = if id == USER { 0 } else { id };
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

    /// `//command`.
    fn cmd(&mut self, player: i32, command: &str) {
        self.input(
            player,
            ClientInput::BuildCmd {
                command: command.to_owned(),
            },
        );
    }
}

/// Ticks of 50 ms.
fn run_ticks(s: &mut Setup, ticks: u32) {
    for _ in 0..ticks {
        s.world.tick();
    }
}

// -------------------------------------------------------------------------------- //kill

#[test]
fn kill_on_an_npc_is_a_real_death_and_the_corpse_stays_until_it_decays() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    let rat = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    s.world.set_target(ObjectId(GM), Some(rat));
    s.log.take();

    // Java: reduceCurrentHp(maxHp + 1): the rat is dead, its corpse is in the world and decays later.
    s.cmd(GM, "kill");
    assert!(s.world.is_dead(rat));
    assert_eq!(s.world.npc(rat).unwrap().core.cur_hp, 0.0);
    assert!(s.world.has_decay_task(rat));
    // The rat has its AI, so the `Die` packet reaches the GM who sees it (`notifyActionDeath`).
    assert!(names_to(&s.log.take(), GM).contains(&"DIE".to_owned()));

    // Killing a corpse changes nothing: `reduceHp` returns for a dead creature.
    s.cmd(GM, "kill");
    assert!(s.world.has_decay_task(rat));

    // The corpse decays and the rat is gone (no spawn, so nothing comes back).
    run_ticks(&mut s, 60 * 20 * 3);
    assert!(s.world.npc(rat).is_none());
}

#[test]
fn kill_by_radius_spares_the_gm_and_hits_every_creature_in_range() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    let near = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    let far = s.world.spawn_npc_at(2000, X + 1500, Y, Z, 0).unwrap();
    s.log.take();

    s.cmd(GM, "kill 100");
    assert!(s.world.is_dead(near));
    assert!(!s.world.is_dead(far));
    assert!(!s.world.is_dead(ObjectId(GM)));
    // 300 units away: outside the 100 unit radius.
    assert!(!s.world.is_dead(ObjectId(USER)));
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Killed all characters within a 100 unit radius."]
    );

    // Around another player: the player is the centre and is not in his own radius; the GM is
    // never killed.
    s.cmd(GM, "kill_monster p1 2000");
    assert!(s.world.is_dead(far));
    assert!(!s.world.is_dead(ObjectId(USER)));
    assert!(!s.world.is_dead(ObjectId(GM)));
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Killed all characters within a 2000 unit radius."]
    );
}

#[test]
fn kill_a_player_is_a_real_death_with_the_die_packet() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    s.cmd(GM, "kill P1");
    let out = s.log.take();
    // Java: reduceCurrentHp(maxHp + maxCp + 1): the CP and then the HP go to zero.
    let core = s.world.player(ObjectId(USER)).unwrap().core.clone();
    assert_eq!((core.cur_hp, core.cur_cp), (0.0, 0.0));
    assert!(s.world.is_dead(ObjectId(USER)));
    for to in [USER, GM] {
        assert!(names_to(&out, to).contains(&"DIE".to_owned()), "to {to}");
    }
    assert_eq!(decoded::<Die>(&out, USER, "DIE")[0].object_id, USER);

    // Dead already: nothing more is sent to him.
    s.cmd(GM, "kill P1");
    assert!(names_to(&s.log.take(), USER).is_empty());
}

// -------------------------------------------------------------------------------- //res

#[test]
fn res_brings_a_dead_player_back_with_the_restored_share() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.cmd(GM, "kill");
    assert!(s.world.is_dead(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "res");
    let out = s.log.take();
    assert!(!s.world.is_dead(ObjectId(USER)));
    // Java: doRevive restores RespawnRestoreHP (65 %) of the maximum HP.
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().core.cur_hp, 65.0);
    assert!(names_to(&out, USER).contains(&"REVIVE".to_owned()));

    // Alive already: `doResurrect` does nothing.
    s.cmd(GM, "res");
    assert!(names_to(&s.log.take(), USER).is_empty());
}

#[test]
fn res_by_name_by_radius_and_bad_parameters() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.cmd(GM, "kill P1");
    assert!(s.world.is_dead(ObjectId(USER)));
    s.log.take();

    // By name, the target is not used.
    s.cmd(GM, "res p1");
    assert!(!s.world.is_dead(ObjectId(USER)));

    // A radius: every player in range, not the GM; the player 300 units away is in 500.
    s.cmd(GM, "kill P1");
    s.log.take();
    s.cmd(GM, "res 500");
    assert!(!s.world.is_dead(ObjectId(USER)));
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Resurrected all players within a 500 unit radius."]
    );

    // Neither a name nor a number.
    s.cmd(GM, "res nobody_here");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Enter a valid player name or radius."]
    );
}

#[test]
fn res_monster_brings_a_dead_npc_back_and_refuses_a_player() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    let rat = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    s.world.set_target(ObjectId(GM), Some(rat));
    s.cmd(GM, "kill");
    assert!(s.world.is_dead(rat));
    assert!(s.world.has_decay_task(rat));
    s.log.take();

    // Java: `DecayTaskManager.cancel` and `doRevive`: the corpse does not decay.
    s.cmd(GM, "res_monster");
    assert!(!s.world.is_dead(rat));
    assert!(!s.world.has_decay_task(rat));
    assert!(s.world.npc(rat).unwrap().core.cur_hp > 0.0);
    assert!(system_ids_to(&s.log.take(), GM).is_empty());

    // A player is not a monster: the answer is INCORRECT_ITEM.
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.cmd(GM, "res_monster");
    assert_eq!(system_ids_to(&s.log.take(), GM), [SystemMessageId::IncorrectItem]);
}

#[test]
fn res_monster_radius_revives_the_dead_non_players_in_range() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    let near = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    let far = s.world.spawn_npc_at(2000, X + 1500, Y, Z, 0).unwrap();
    s.world.reduce_current_hp(near, 1000.0, None);
    s.world.reduce_current_hp(far, 1000.0, None);
    s.log.take();

    // No target: the radius is answered, then INCORRECT_ITEM for the missing target.
    s.cmd(GM, "res_monster 300");
    assert!(!s.world.is_dead(near));
    assert!(s.world.is_dead(far));
    assert!(!s.world.is_dead(ObjectId(USER)));
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        ["Resurrected all non-players within a 300 unit radius."]
    );
    assert_eq!(system_ids_to(&out, GM), [SystemMessageId::IncorrectItem]);

    s.cmd(GM, "res_monster x");
    assert_eq!(texts_to(&s.log.take(), GM), ["Enter a valid radius."]);
}

// ---------------------------------------------------------------------------- //add_exp_sp

#[test]
fn add_exp_sp_tells_the_player_and_the_gm() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "add_exp_sp 1000 5");
    let out = s.log.take();
    assert_eq!(texts_to(&out, USER), ["Admin is adding you 1000 xp and 5 sp."]);
    assert_eq!(texts_to(&out, GM), ["Added 1000 xp and 5 sp to P1."]);

    // Zero and zero: nothing is said.
    s.cmd(GM, "add_exp_sp 0 0");
    assert!(s.log.take().is_empty());

    // A sign is a number, and `-0` is zero.
    s.cmd(GM, "add_exp_sp +7 -0");
    assert_eq!(
        texts_to(&s.log.take(), USER),
        ["Admin is adding you 7 xp and 0 sp."]
    );
}

#[test]
fn add_exp_sp_without_two_numbers_is_a_usage_line() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    // `add_exp_sp_to_character` starts with `admin_add_exp_sp` too, so it is the same branch.
    for command in [
        "add_exp_sp",
        "add_exp_sp 1000",
        "add_exp_sp a 5",
        "add_exp_sp 1 2 3",
        "add_exp_sp 99999999999999999999 1",
        "add_exp_sp_to_character",
    ] {
        s.cmd(GM, command);
        let out = s.log.take();
        assert_eq!(texts_to(&out, GM), ["Usage: //add_exp_sp exp sp"], "{command}");
        assert!(texts_to(&out, USER).is_empty(), "{command}");
    }
}

#[test]
fn add_exp_sp_without_a_player_target_says_invalid_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    // No target: `INVALID_TARGET`, the usage line, and `INVALID_TARGET` again from the menu.
    s.cmd(GM, "add_exp_sp 1 1");
    let out = s.log.take();
    assert_eq!(
        system_ids_to(&out, GM),
        [SystemMessageId::InvalidTarget, SystemMessageId::InvalidTarget]
    );
    assert_eq!(texts_to(&out, GM), ["Usage: //add_exp_sp exp sp"]);
}

#[test]
fn remove_exp_sp_mirrors_add_and_takes_no_cap() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "remove_exp_sp 50 -2");
    let out = s.log.take();
    assert_eq!(texts_to(&out, USER), ["Admin is removing you 50 xp and -2 sp."]);
    assert_eq!(texts_to(&out, GM), ["Removed 50 xp and -2 sp from P1."]);

    for command in [
        "remove_exp_sp",
        "remove_exp_sp 1",
        "remove_exp_sp 1 99999999999999999999999",
    ] {
        s.cmd(GM, command);
        assert_eq!(
            texts_to(&s.log.take(), GM),
            ["Usage: //remove_exp_sp exp sp"],
            "{command}"
        );
    }
}
