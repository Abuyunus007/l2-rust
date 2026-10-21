#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)] // test helpers may panic

//! The combat admin commands (M4-ADMIN-02): `//res`, `//res_monster`, `//kill`, `//kill_monster`
//! and the exp/sp handlers, and the item and level commands (M4-ADMIN-03), with a fake GM, a fake
//! player and a fake monster.
//! Java: scripts/handlers/chat/commands/admin/{AdminRes,AdminKill,AdminExpSp,AdminCreateItem,
//! AdminLevel}.java.
//!
//! The harness is the one of `admin.rs` (the framework and the core commands).

use std::path::Path;
use std::sync::{Arc, Mutex};

use l2_data::{
    ClassList, ExperienceLoss, ExperienceTable, HtmCache, ItemTemplates, KarmaLoss, NpcLoadOptions,
    NpcTemplates, SkillTemplates, SkillTrees, SpawnData, SpawnLoadOptions,
};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ConfirmDlg, Die, GmViewItemList, NpcHtmlMessage, SystemMessage, SystemMessageParam,
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
        ("invul", 30, false),
        ("setinvul", 100, false),
        ("heal", 100, false),
        ("itemcreate", 100, false),
        ("create_item", 100, false),
        ("create_coin", 100, false),
        ("give_item_target", 100, true),
        ("give_item_to_all", 100, true),
        ("add_level", 100, false),
        ("set_level", 100, false),
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
/// The item templates of the item commands: stackable money and etc items, a bracelet (equippable,
/// not enchantable) and a blade that can be enchanted.
const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="5575" type="EtcItem" name="Ancient Adena"><set name="is_stackable" val="true" /></item>
<item id="23" type="EtcItem" name="Pile"><set name="is_stackable" val="true" /><set name="weight" val="2" /></item>
<item id="12" type="Armor" name="Bracelet"><set name="bodypart" val="rbracelet" /><set name="weight" val="1" /></item>
<item id="24" type="EtcItem" name="Blade"><set name="enchant_enabled" val="true" /><set name="weight" val="1" /></item>
</list>"#;

/// A passive skill with a reflect effect (`reflectDam`, 50 percent): the target of the reflect test
/// learns it, so the stat engine gives it reflect.
const SKILLS: &str = r##"<list>
<skill id="3100" levels="1" name="Reflect"><operateType>P</operateType><targetType>SELF</targetType>
<effects><effect name="Buff"><add stat="reflectDam" val="50"/></effect></effects></skill>
</list>"##;

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
        items: Arc::new(ItemTemplates::from_xml(ITEMS).unwrap()),
        skills: Arc::new(SkillTemplates::from_xml(
            SKILLS,
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        )),
        ..StatData::default()
    }
}

impl StaticData for Fake {
    fn item(&self, _item_id: i32) -> Option<ItemStatic> {
        None
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
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.stats.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.stats.items.get(item_id).is_some_and(|t| t.stackable)
    }
    fn skill_is_passive(&self, skill_id: i32) -> bool {
        self.stats
            .skills
            .iter()
            .any(|s| s.id == skill_id && s.is_passive())
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

/// The admin pages (`NPC_HTML_MESSAGE`) sent to `to`.
fn html_to(out: &Sent, to: i32) -> Vec<NpcHtmlMessage> {
    decoded::<NpcHtmlMessage>(out, to, "NPC_HTML_MESSAGE")
}

/// An item row of a character (`items` table): one item in the given location.
fn item_row(object_id: i32, item_id: i32, count: i64, loc: &str, loc_data: i32) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        enchant_level: 0,
        loc: loc.into(),
        loc_data,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

/// The item rows of a character as `(item id, count, location)`, in the order the list has them.
fn items_of(s: &Setup, id: i32) -> Vec<(i32, i64, String)> {
    s.world
        .player(ObjectId(id))
        .unwrap()
        .data
        .items
        .iter()
        .map(|i| (i.item_id, i.count, i.loc.clone()))
        .collect()
}

/// `experience.xml` for levels 1 to 88 (`maxLevel` 85: Java's `getMaxLevel()` is 86, and the exp of
/// that cap is the row of level 86, as in the shipped table): a level needs more exp than the one
/// before it.
fn experience_table() -> Arc<ExperienceTable> {
    let rows: String = (1..=88)
        .map(|level| {
            format!(
                r#"<experience level="{level}" tolevel="{}" />"#,
                level * level * 1000
            )
        })
        .collect();
    Arc::new(
        ExperienceTable::from_xml(&format!(
            r#"<table maxLevel="85" maxPetLevel="85">{rows}</table>"#
        ))
        .unwrap(),
    )
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
        write(
            "data/html/admin/itemcreation.htm",
            "<html><body>Item creation</body></html>",
        );
        write(
            "data/html/admin/expsp.htm",
            "<html><body>%name% Lv %level% %class% Exp %xp% SP %sp% for %name%</body></html>",
        );
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
    world.set_class_list(Arc::new(
        ClassList::from_xml(r#"<list><class classId="0" name="Human Fighter" /></list>"#).unwrap(),
    ));
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

    /// The NPCs of a template, in object id order.
    fn npcs_of(&self, npc_id: i32) -> Vec<ObjectId> {
        self.world
            .npc_ids()
            .into_iter()
            .filter(|id| self.world.npc(*id).unwrap().npc_id == npc_id)
            .collect()
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

    // Dead already: no second death is sent to him. The `stopAllEffects()` of the kill (M5/M6,
    // Java `Player.stopAllEffects` -> `updateAndBroadcastStatus`) still refreshes his status.
    s.cmd(GM, "kill P1");
    let again = names_to(&s.log.take(), USER);
    assert!(!again.contains(&"DIE".to_owned()));
    assert_eq!(again, ["USER_INFO", "EX_BR_EXTRA_USER_INFO"]);
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

    // Zero and zero: nothing is said, and the menu of the target (`expsp.htm`) follows every
    // command, with the exp and sp the target has by now and the class name of `ClassListData`.
    s.cmd(GM, "add_exp_sp 0 0");
    let out = s.log.take();
    assert!(texts_to(&out, GM).is_empty() && texts_to(&out, USER).is_empty());
    let pages = html_to(&out, GM);
    assert_eq!(pages.len(), 1);
    let exp = s.world.player(ObjectId(USER)).unwrap().data.exp;
    assert_eq!(
        pages[0].html,
        format!("<html><body>P1 Lv 40 Human Fighter Exp {exp} SP 5 for P1</body></html>")
    );

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

/// Runs ticks until `done` holds; the number of ticks it took, `None` when it never did.
fn run_until(s: &mut Setup, limit: u32, done: impl Fn(&World) -> bool) -> Option<u32> {
    for n in 1..=limit {
        s.world.tick();
        if done(&s.world) {
            return Some(n);
        }
    }
    None
}

/// Java `Spawn.setRespawnDelay` / `init()`: 50 ms ticks for a number of seconds.
fn ticks_of_seconds(seconds: u32) -> u32 {
    seconds * 20
}

// ------------------------------------------------------------------------------ //invul

#[test]
fn invul_makes_the_gm_immune_and_toggles_with_java_messages() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    // Java: the GM is told, and nobody else (the admin is the target).
    s.cmd(GM, "invul");
    assert_eq!(texts_to(&s.log.take(), GM), ["You are now invulnerable."]);
    assert!(s.world.is_invul(ObjectId(GM)));

    // Damage does not reach an invulnerable creature (`reduceHp` returns early).
    s.world
        .reduce_current_hp(ObjectId(GM), 1000.0, Some(ObjectId(USER)));
    assert!(!s.world.is_dead(ObjectId(GM)));
    assert_eq!(s.world.player(ObjectId(GM)).unwrap().core.cur_hp, 100.0);

    // And the toggle back.
    s.cmd(GM, "invul");
    assert_eq!(texts_to(&s.log.take(), GM), ["You are now mortal."]);
    assert!(!s.world.is_invul(ObjectId(GM)));
    s.world
        .reduce_current_hp(ObjectId(GM), 1000.0, Some(ObjectId(USER)));
    assert!(s.world.is_dead(ObjectId(GM)));

    // Unknown words are not the command: nothing happens (Java's `switch` on the whole text).
    s.log.take();
    s.cmd(GM, "invul x");
    let out = s.log.take();
    assert!(texts_to(&out, GM).is_empty());
    assert!(out.is_empty(), "{:?}", names_to(&out, GM));
}

#[test]
fn setinvul_targets_a_player_and_tells_both_of_them() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "setinvul");
    let out = s.log.take();
    assert_eq!(texts_to(&out, USER), ["You are now invulnerable."]);
    assert_eq!(texts_to(&out, GM), ["You made P1 invulnerable."]);
    assert!(s.world.is_invul(ObjectId(USER)));

    // A kill does nothing to him now: `PlayerStatus.reduceHp` returns early.
    s.cmd(GM, "kill P1");
    assert!(!s.world.is_dead(ObjectId(USER)));
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().core.cur_hp, 100.0);

    s.log.take();
    s.cmd(GM, "setinvul");
    let out = s.log.take();
    assert_eq!(texts_to(&out, USER), ["You are now mortal."]);
    assert_eq!(texts_to(&out, GM), ["You made P1 mortal."]);
    s.cmd(GM, "kill P1");
    assert!(s.world.is_dead(ObjectId(USER)));
}

#[test]
fn setinvul_needs_a_player_target() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    let rat = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    s.log.take();

    s.cmd(GM, "setinvul");
    assert_eq!(texts_to(&s.log.take(), GM), ["You must target a player."]);
    s.world.set_target(ObjectId(GM), Some(rat));
    s.cmd(GM, "setinvul");
    assert_eq!(texts_to(&s.log.take(), GM), ["You must target a player."]);
    assert!(!s.world.is_invul(rat));
}

#[test]
fn an_invulnerable_npc_is_made_mortal_for_the_kill_and_stays_invulnerable() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    let rat = s.world.spawn_npc_at(2000, X + 50, Y, Z, 0).unwrap();
    s.world.set_invul(rat, true);
    s.world.set_target(ObjectId(GM), Some(rat));

    // Java `AdminKill.kill`: `setInvul(false)`, the hit, `setInvul(true)`.
    s.cmd(GM, "kill");
    assert!(s.world.is_dead(rat));
    assert!(s.world.is_invul(rat));
}

#[test]
fn a_teleporting_player_cannot_be_hurt() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 300);
    s.cmd(GM, "move_to -80000 150000 -3000");
    // Java `isInvul()`: `_isTeleporting` counts, until the client says it arrived.
    assert!(s.world.is_teleporting(ObjectId(GM)));
    assert!(s.world.is_invul(ObjectId(GM)));
    s.world
        .reduce_current_hp(ObjectId(GM), 1000.0, Some(ObjectId(USER)));
    assert!(!s.world.is_dead(ObjectId(GM)));
}

#[test]
fn invul_is_gone_when_the_character_leaves_the_world() {
    let mut s = setup();
    s.enter(GM, 0);
    s.cmd(GM, "invul");
    assert!(s.world.is_invul(ObjectId(GM)));

    s.world.handle(WorldCommand::Disconnected {
        session: SessionId(u64::try_from(GM).unwrap()),
    });
    s.enter(GM, 0);
    // A new login is a new Java object: `_isInvul` starts false.
    assert!(!s.world.is_invul(ObjectId(GM)));
}

// ------------------------------------------------------------------------------ spawns

#[test]
fn spawn_with_a_respawn_time_brings_the_npc_back_at_its_spawn() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    // Java: `setAmount(2)`, `setRespawnDelay(30)`, `init()` leaves `_doRespawn` on.
    s.cmd(GM, "spawn 2000 2 30");
    let rats = s.npcs_of(2000);
    assert_eq!(rats.len(), 2);
    let rat = rats[0];
    let (x, y, z) = {
        let core = &s.world.npc(rat).unwrap().core;
        (core.x, core.y, core.z)
    };

    s.world.reduce_current_hp(rat, 1000.0, None);
    assert!(s.world.is_dead(rat));
    // The corpse decays first (its template's corpse time), then the spawn counts one NPC less.
    assert!(run_until(&mut s, 2000, |w| w.npc(rat).is_none()).is_some());
    assert_eq!(s.npcs_of(2000).len(), 1);
    assert!(s.world.has_pending_respawn(rat));

    // The respawn is 30 seconds later, with the same object id, at the spawn place, alive.
    let waited = run_until(&mut s, 2000, |w| w.npc(rat).is_some());
    assert_eq!(waited, Some(ticks_of_seconds(30)));
    let back = s.world.npc(rat).unwrap();
    assert_eq!((back.core.x, back.core.y, back.core.z), (x, y, z));
    assert!(!s.world.is_dead(rat));
    assert_eq!(s.npcs_of(2000).len(), 2);
}

#[test]
fn spawn_once_and_a_zero_delay_do_not_respawn() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    // `spawn_once` is not permanent: `stopRespawn()`.
    s.cmd(GM, "spawn_once 2000");
    let once = s.npcs_of(2000)[0];
    // A zero delay: `init()` leaves `_doRespawn = false`.
    s.cmd(GM, "spawn 2001 1 0");
    let zero = s.npcs_of(2001)[0];

    for npc in [once, zero] {
        s.world.reduce_current_hp(npc, 1000.0, None);
        assert!(run_until(&mut s, 2000, |w| w.npc(npc).is_none()).is_some());
        assert!(!s.world.has_pending_respawn(npc));
    }
    // Well past any respawn delay the tests use: nothing comes back.
    run_ticks(&mut s, ticks_of_seconds(300));
    assert!(s.npcs_of(2000).is_empty());
    assert!(s.npcs_of(2001).is_empty());
}

#[test]
fn spawnat_comes_back_after_sixty_seconds() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    // Java `spawnMonster(id, x, y, z, h)`: `setRespawnDelay(60)` and `init()`.
    s.cmd(GM, &format!("spawnat 2000 {} {Y} {Z} 0", X + 50));
    let rat = s.npcs_of(2000)[0];
    s.world.reduce_current_hp(rat, 1000.0, None);
    assert!(run_until(&mut s, 2000, |w| w.npc(rat).is_none()).is_some());
    let waited = run_until(&mut s, 5000, |w| w.npc(rat).is_some());
    assert_eq!(waited, Some(ticks_of_seconds(60)));
}

// ------------------------------------------------------------------- item and level (ADMIN-03)

#[test]
fn create_item_gives_the_gm_the_item_and_tells_them_so() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    s.cmd(GM, "create_item 12");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        ["You have spawned 1 Bracelet(12) in P100 inventory."]
    );
    // The item page is shown after the command, as Java does.
    assert_eq!(html_to(&out, GM).len(), 1);
    assert_eq!(items_of(&s, GM), [(12, 1, "INVENTORY".to_owned())]);
}

#[test]
fn create_item_stacks_and_refuses_a_count_over_ten_for_a_non_stackable_item() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    s.cmd(GM, "create_item 23 5");
    s.cmd(GM, "create_item 23 3");
    // One row: the second creation joined the first stack.
    assert_eq!(items_of(&s, GM), [(23, 8, "INVENTORY".to_owned())]);

    s.log.take();
    s.cmd(GM, "create_item 12 11");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["This item does not stack - Creation aborted."]
    );
    assert_eq!(items_of(&s, GM).len(), 1);

    s.cmd(GM, "create_item 999");
    assert_eq!(texts_to(&s.log.take(), GM), ["This item doesn't exist."]);
}

#[test]
fn create_item_with_bad_numbers_says_so_and_still_shows_the_page() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    for command in ["create_item x", "create_item 12 y", "create_item 12 1 z"] {
        s.cmd(GM, command);
        let out = s.log.take();
        assert_eq!(texts_to(&out, GM), ["Specify a valid number."], "{command}");
        assert_eq!(html_to(&out, GM).len(), 1, "{command}");
    }
    assert!(items_of(&s, GM).is_empty());
}

#[test]
fn create_item_with_an_enchant_applies_no_enchant_to_a_plain_item_and_refuses_an_enchantable_one() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    // The bracelet is not enchantable: the item is made and Java's message says so.
    s.cmd(GM, "create_item 12 1 5");
    let out = s.log.take();
    assert_eq!(
        texts_to(&out, GM),
        [
            "This item is not enchantable, no enchant applied.",
            "You have spawned 1 Bracelet(12) in P100 inventory.",
        ]
    );
    assert_eq!(items_of(&s, GM), [(12, 1, "INVENTORY".to_owned())]);

    // The blade is enchantable, which needs `EnchantItemGroupsData` (not ported): nothing is made
    // and, like Java's other unsupported paths, nothing is said to the GM.
    s.cmd(GM, "create_item 24 1 5");
    assert!(texts_to(&s.log.take(), GM).is_empty());
    assert_eq!(items_of(&s, GM).len(), 1);
}

#[test]
fn create_coin_knows_the_coin_names_and_only_those() {
    let mut s = setup();
    s.enter(GM, 0);
    s.log.take();

    s.cmd(GM, "create_coin adena 500");
    assert_eq!(items_of(&s, GM), [(57, 500, "INVENTORY".to_owned())]);

    s.cmd(GM, "create_coin ANCIENTADENA");
    assert!(items_of(&s, GM).contains(&(5575, 1, "INVENTORY".to_owned())));

    // An unknown coin with an amount makes nothing and says nothing, but the page is shown.
    s.log.take();
    s.cmd(GM, "create_coin nothing 5");
    let out = s.log.take();
    assert!(texts_to(&out, GM).is_empty());
    assert_eq!(html_to(&out, GM).len(), 1);
    assert_eq!(items_of(&s, GM).len(), 2);

    // An unknown coin alone is an item id 0: "This item doesn't exist.".
    s.cmd(GM, "create_coin nothing");
    assert_eq!(texts_to(&s.log.take(), GM), ["This item doesn't exist."]);
}

#[test]
fn give_item_target_needs_a_player_target_and_gives_the_item_to_it() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    // `confirmDlg="true"`: the command asks first and runs after the answer.
    s.cmd(GM, "give_item_target 23 2");
    let out = s.log.take();
    assert!(texts_to(&out, GM).is_empty());
    let dialogs = decoded::<ConfirmDlg>(&out, GM, "CONFIRM_DLG");
    assert_eq!(dialogs.len(), 1);
    assert_eq!(dialogs[0].message.id, SystemMessageId::S13);
    assert_eq!(
        dialogs[0].message.params,
        [SystemMessageParam::Text(
            "Are you sure you want execute command 'give_item_target' ?".to_owned()
        )]
    );

    // The command as it runs after the dialog said yes (`onCommand(.., false)`).
    // No target: "Invalid target." and no page (Java returns first).
    s.world
        .admin_command(ObjectId(GM), "admin_give_item_target 23 2", false);
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Invalid target."]);
    assert!(html_to(&out, GM).is_empty());

    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();
    s.world
        .admin_command(ObjectId(GM), "admin_give_item_target 23 2", false);
    let out = s.log.take();
    assert_eq!(items_of(&s, USER), [(23, 2, "INVENTORY".to_owned())]);
    assert!(items_of(&s, GM).is_empty());
    assert_eq!(texts_to(&out, USER), ["Admin spawned 2 Pile in your inventory."]);
    assert_eq!(
        texts_to(&out, GM),
        ["You have spawned 2 Pile(23) in P1 inventory."]
    );
    assert_eq!(html_to(&out, GM).len(), 1);
}

#[test]
fn give_item_to_all_rewards_every_online_player_but_the_gm() {
    let mut s = setup();
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.enter(2, 200);
    s.log.take();

    s.world
        .admin_command(ObjectId(GM), "admin_give_item_to_all 23 3", false);
    let out = s.log.take();
    assert_eq!(items_of(&s, USER), [(23, 3, "INVENTORY".to_owned())]);
    assert_eq!(items_of(&s, 2), [(23, 3, "INVENTORY".to_owned())]);
    assert!(items_of(&s, GM).is_empty());
    assert_eq!(texts_to(&out, USER), ["Admin spawned 3 Pile in your inventory."]);
    assert_eq!(texts_to(&out, GM), ["2 players rewarded with Pile"]);

    s.log.take();
    s.world
        .admin_command(ObjectId(GM), "admin_give_item_to_all 12 11", false);
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["This item does not stack - Creation aborted."]
    );
    s.world
        .admin_command(ObjectId(GM), "admin_give_item_to_all 999", false);
    assert_eq!(texts_to(&s.log.take(), GM), ["This item doesn't exist."]);
    // A bad number is not caught by the handler: the dispatcher reports the exception.
    s.world
        .admin_command(ObjectId(GM), "admin_give_item_to_all x", false);
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Exception during execution of  'admin_give_item_to_all x': java.lang.NumberFormatException: For input string: \"x\""
        ]
    );
}

#[test]
fn delete_item_destroys_the_count_or_the_whole_item_of_its_owner() {
    let mut s = setup();
    s.enter(GM, 0);
    let mut data = player(USER, 0, 100);
    data.items = vec![item_row(500, 23, 5, "INVENTORY", 0)];
    s.enter_with(data);
    // The bypasses need the page that offers them (`validateHtmlAction`).
    s.log.take();

    // Two of the five; one token means one.
    s.world
        .admin_command(ObjectId(GM), "admin_delete_item 500 2", false);
    assert_eq!(items_of(&s, USER), [(23, 3, "INVENTORY".to_owned())]);
    let out = s.log.take();
    assert_eq!(texts_to(&out, GM), ["Item deleted."]);
    // `GMViewItemList` of the owner goes to the GM, before the message, with what is left.
    let lists = decoded::<GmViewItemList>(&out, GM, "GM_VIEW_ITEM_LIST");
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].player_name, "P1");
    assert_eq!(
        lists[0].limit,
        WorldSettings::default().inventory_maximum_no_dwarf
    );
    assert_eq!(lists[0].items.len(), 1);
    assert_eq!((lists[0].items[0].object_id, lists[0].items[0].count), (500, 3));
    let names: Vec<String> = out
        .iter()
        .filter(|(t, _)| *t == GM)
        .map(|(_, b)| name_of(b))
        .collect();
    let at = |name: &str| names.iter().position(|n| n == name).unwrap();
    assert!(at("GM_VIEW_ITEM_LIST") < at("SYSTEM_MESSAGE"));
    s.world
        .admin_command(ObjectId(GM), "admin_delete_item 500", false);
    assert_eq!(items_of(&s, USER), [(23, 2, "INVENTORY".to_owned())]);

    // An amount of 0 takes the whole item.
    s.world
        .admin_command(ObjectId(GM), "admin_delete_item 500 0", false);
    assert!(items_of(&s, USER).is_empty());

    // No such object: Java's NullPointerException, reported by the dispatcher.
    s.log.take();
    s.world
        .admin_command(ObjectId(GM), "admin_delete_item 999", false);
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Exception during execution of  'admin_delete_item 999': java.lang.NullPointerException"]
    );
}

#[test]
fn use_item_equips_an_equippable_item_of_its_owner() {
    let mut s = setup();
    s.enter(GM, 0);
    let mut data = player(USER, 0, 100);
    data.items = vec![item_row(600, 12, 1, "INVENTORY", 0)];
    s.enter_with(data);
    s.log.take();

    s.world.admin_command(ObjectId(GM), "admin_use_item 600", false);
    assert_eq!(items_of(&s, USER), [(12, 1, "PAPERDOLL".to_owned())]);
    // The GM's item window shows the owner's inventory after the change.
    let lists = decoded::<GmViewItemList>(&s.log.take(), GM, "GM_VIEW_ITEM_LIST");
    assert_eq!(lists.len(), 1);
    assert_eq!(lists[0].items.len(), 1);
    assert_eq!(lists[0].items[0].equipped, 1);

    // Java's `substring(15)` of the bare command, and the number of the command.
    s.log.take();
    s.world.admin_command(ObjectId(GM), "admin_use_item", false);
    assert_eq!(
        texts_to(&s.log.take(), GM),
        [
            "Exception during execution of  'admin_use_item': java.lang.StringIndexOutOfBoundsException: begin 15, end 14, length 14"
        ]
    );
    s.world.admin_command(ObjectId(GM), "admin_use_item 999", false);
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Exception during execution of  'admin_use_item 999': java.lang.NullPointerException"]
    );
}

#[test]
fn add_level_raises_the_targeted_player_and_reads_a_byte() {
    let mut s = setup();
    // `add_level` needs the experience table (`ExperienceData`); the other tests do not.
    s.world.set_exp_data(
        experience_table(),
        Arc::new(ExperienceLoss::from_xml(r#"<list><experience level="5" val="9.5" /></list>"#).unwrap()),
        Arc::new(KarmaLoss::from_xml(r#"<list><modifier level="10" val="2.0" /></list>"#).unwrap()),
        Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
    );
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.log.take();

    // No target: nothing, not even the number is looked at.
    s.cmd(GM, "add_level 300");
    assert!(texts_to(&s.log.take(), GM).is_empty());

    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.cmd(GM, "add_level 1");
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().core.level, 41);

    // `Byte.parseByte`: a number out of a byte is "Wrong Number Format".
    s.log.take();
    s.cmd(GM, "add_level 300");
    s.cmd(GM, "add_level x");
    assert_eq!(
        texts_to(&s.log.take(), GM),
        ["Wrong Number Format", "Wrong Number Format"]
    );
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().core.level, 41);
}

#[test]
fn add_exp_sp_caps_the_sp_at_max_sp() {
    let mut settings = WorldSettings::default();
    settings.exp.max_sp = 100;
    let mut s = setup_with(settings);
    s.enter(GM, 0);
    s.enter(USER, 100);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    // Java: `Math.min(sp, MAX_SP)`, and the messages carry the capped number.
    s.cmd(GM, "add_exp_sp 0 500");
    let out = s.log.take();
    assert_eq!(texts_to(&out, USER), ["Admin is adding you 0 xp and 100 sp."]);
    assert_eq!(texts_to(&out, GM), ["Added 0 xp and 100 sp to P1."]);
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().data.sp, 100);

    // At the cap nothing more is added, and the message still says what the admin asked for.
    s.cmd(GM, "add_exp_sp 0 50");
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().data.sp, 100);
}

// ----------------------------------------- set_level, remove_exp_sp, startup invul, blocked hit

/// The experience table of `experience_table()`, for the tests that read exp.
fn set_experience(s: &mut Setup) {
    s.world.set_exp_data(
        experience_table(),
        Arc::new(ExperienceLoss::from_xml(r#"<list><experience level="5" val="9.5" /></list>"#).unwrap()),
        Arc::new(KarmaLoss::from_xml(r#"<list><modifier level="10" val="2.0" /></list>"#).unwrap()),
        Arc::new(SkillTrees::from_xml("<list></list>").unwrap()),
    );
}

fn exp_of(s: &Setup, id: i32) -> i64 {
    s.world.player(ObjectId(id)).unwrap().data.exp
}

fn level_of(s: &Setup, id: i32) -> i32 {
    s.world.player(ObjectId(id)).unwrap().core.level
}

fn hp_of(s: &Setup, id: i32) -> f64 {
    s.world.player(ObjectId(id)).unwrap().core.cur_hp
}

#[test]
fn set_level_moves_the_exp_to_that_levels_exp_and_tells_both_sides() {
    let mut s = setup();
    set_experience(&mut s);
    s.enter(GM, 0);
    let mut data = player(USER, 0, 100);
    data.exp = 1_000_000;
    s.enter_with(data);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    // Down: level 10 needs 100000 exp; the difference is removed, and the target is told so.
    s.cmd(GM, "set_level 10");
    let out = s.log.take();
    assert_eq!(level_of(&s, USER), 10);
    assert_eq!(exp_of(&s, USER), 100_000);
    assert_eq!(texts_to(&out, GM), ["Removed 900000 exp."]);
    assert_eq!(
        system_ids_to(&out, USER),
        [
            SystemMessageId::YourExperienceHasDecreasedByS1,
            SystemMessageId::YourSpHasDecreasedByS1,
        ]
    );

    // Up: level 20 needs 400000 exp; the exp is added (the level follows it).
    s.log.take();
    s.cmd(GM, "set_level 20");
    let out = s.log.take();
    assert_eq!(level_of(&s, USER), 20);
    assert_eq!(exp_of(&s, USER), 400_000);
    assert_eq!(texts_to(&out, GM), ["Added 300000 exp."]);

    // Java's `maxLevel` is the table's last level plus one.
    for command in ["set_level 0", "set_level 87", "set_level x"] {
        s.log.take();
        s.cmd(GM, command);
        assert_eq!(
            texts_to(&s.log.take(), GM),
            ["You must specify level between 1 and 86."],
            "{command}"
        );
    }
    assert_eq!(level_of(&s, USER), 20);

    // No target: `THAT_IS_AN_INCORRECT_TARGET`.
    s.world.set_target(ObjectId(GM), None);
    s.log.take();
    s.cmd(GM, "set_level 5");
    assert_eq!(
        system_ids_to(&s.log.take(), GM),
        [SystemMessageId::ThatIsAnIncorrectTarget]
    );
}

#[test]
fn remove_exp_sp_takes_the_exp_and_the_sp_and_tells_the_target_twice() {
    let mut s = setup();
    set_experience(&mut s);
    s.enter(GM, 0);
    let mut data = player(USER, 0, 100);
    data.exp = 1_000_000;
    data.sp = 10;
    s.enter_with(data);
    s.world.set_target(ObjectId(GM), Some(ObjectId(USER)));
    s.log.take();

    s.cmd(GM, "remove_exp_sp 500 4");
    let out = s.log.take();
    assert_eq!(exp_of(&s, USER), 999_500);
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().data.sp, 6);
    assert_eq!(texts_to(&out, USER), ["Admin is removing you 500 xp and 4 sp."]);
    assert_eq!(
        system_ids_to(&out, USER),
        [
            SystemMessageId::YourExperienceHasDecreasedByS1,
            SystemMessageId::YourSpHasDecreasedByS1,
        ]
    );
    assert_eq!(texts_to(&out, GM), ["Removed 500 xp and 4 sp from P1."]);

    // More sp than the target has: the sp goes to 0.
    s.cmd(GM, "remove_exp_sp 0 100");
    assert_eq!(s.world.player(ObjectId(USER)).unwrap().data.sp, 0);

    // Nothing asked: nothing said.
    s.log.take();
    s.cmd(GM, "remove_exp_sp 0 0");
    let out = s.log.take();
    assert!(texts_to(&out, USER).is_empty());
    assert!(system_ids_to(&out, USER).is_empty());
}

#[test]
fn gm_startup_invulnerable_makes_a_gm_with_the_admin_invul_right_invulnerable_on_enter() {
    let settings = WorldSettings {
        gm_startup_invulnerable: true,
        ..WorldSettings::default()
    };
    let mut s = setup_with(settings);
    s.enter(GM, 0);
    s.enter(USER, 100);
    assert!(s.world.is_invul(ObjectId(GM)));
    assert!(!s.world.is_invul(ObjectId(USER)));
}

#[test]
fn gm_startup_invulnerable_is_off_by_default() {
    let mut s = setup();
    s.enter(GM, 0);
    assert!(!s.world.is_invul(ObjectId(GM)));
}

/// A hit of `attacker` on `target` as `Creature.onHitTimer` gets it (no crit, no miss).
fn hit(attacker: i32, target: i32, damage: i32) -> HitTask {
    HitTask {
        attacker: ObjectId(attacker),
        target: ObjectId(target),
        damage,
        crit: false,
        miss: false,
        shld: 0,
        soulshot: false,
        recharge_shots: false,
    }
}

/// A GM that learned the reflect passive (`reflectDam` 50 percent).
fn reflecting_gm() -> PlayerData {
    let mut data = player(GM, 100, 0);
    data.skills = vec![SkillData {
        skill_id: 3100,
        skill_level: 1,
    }];
    data
}

#[test]
fn an_invulnerable_target_takes_no_hit_reflects_nothing_and_the_attacker_is_told() {
    let mut s = setup();
    s.enter_with(reflecting_gm());
    s.enter(USER, 100);
    s.world.set_invul(ObjectId(GM), true);
    s.log.take();

    s.world.on_hit_timer(hit(USER, GM, 40));
    let out = s.log.take();
    assert_eq!(
        system_ids_to(&out, USER),
        [SystemMessageId::TheAttackHasBeenBlocked]
    );
    assert_eq!(hp_of(&s, GM), 100.0);
    assert_eq!(hp_of(&s, USER), 100.0);
}

#[test]
fn a_mortal_target_takes_the_hit_and_reflects_part_of_it() {
    let mut s = setup();
    s.enter_with(reflecting_gm());
    s.enter(USER, 100);
    s.log.take();

    s.world.on_hit_timer(hit(USER, GM, 40));
    let out = s.log.take();
    assert!(!system_ids_to(&out, USER).contains(&SystemMessageId::TheAttackHasBeenBlocked));
    assert!(hp_of(&s, GM) < 100.0);
    assert!(hp_of(&s, USER) < 100.0, "the reflected part");
}
