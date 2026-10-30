#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
//! End to end for the scripts: players log in to the game server's world with the real datapack
//! of the Java checkout (`dist/game`), the real script registry (`l2_scripts::all_with`, installed
//! the way `Server::start_with` installs it) and play what the card S-E2E-01 lists:
//!
//! 1. a new character: the tutorial, a first village quest (talk, kill, reward) and aborting a
//!    quest (CP-Sa);
//! 2. the first class change at a village master and a subclass added, switched and cancelled,
//!    skills and hennas following (CP-Sb);
//! 3. a Kamaloka entered, a kill inside, the re-enter time (CP-Sc);
//! 4. a raid boss with its AI starting a fight, and a grand boss whose row says dead staying
//!    unspawned on Kain (D-54);
//! 5. side by side (CP-Sd): a `character_quests` row in progress written by "Sieghardt" is
//!    continued after login; `global_variables` untouched with `CycleDatabaseWrites` off and
//!    written with it on (D-53, D-54).
//!
//! The world is driven directly (no sockets, no database, ticks instead of time) like the bare
//! half of `e2e_m8.rs`: every client action is the `ClientInput` the session would give the
//! world. The persistence commands the world sends are recorded, and the reads it asks for are
//! answered from a model of the rows (`e2e/scripts_db.rs`). The same recorded commands are replayed
//! into a throwaway MariaDB when `L2_TEST_DATABASE_URL` is set.
//!
//! The datapack and the Java checkout are needed: every test prints a note and returns when
//! `C:\l2-rust\l2jmobius\dist\game` (or `L2JMOBIUS_GAME_DIR`) is missing, as in CI.

#[path = "e2e/scripts_db.rs"]
mod scripts_db;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use game_server::world_data::{
    DatapackData, multisell_settings, regen_settings, skill_load_options, summon_settings, world_settings,
};
use game_server::{geo, npcs};
use l2_config::GameConfig;
use l2_data::{BuyListOptions, BuyLists, Datapack, DatapackOptions, MerchantPriceConfigs};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{NpcHtmlMessage, NpcInfo, SystemMessage, SystemMessageParam};
use l2_testkit::fake_game_client::{names_of, packet_name};
use l2_world::script::QuestInput;
use l2_world::SubclassWrite;
use l2_world::{
    BossLoad, ClientInput, IdFactory, ItemData, LeaveReason, LsEvent, ObjectId, Outbox, PacketSink,
    PersistCommand, PlayerData, PlayerProfile, SessionId, StaticData, World, WorldCommand, WorldRng,
};

use scripts_db::{Rows, Seed};

/// Far above the ids the factory hands out to the NPCs (it counts up from `RUST_FIRST_OBJECT_ID`).
const BARE_FIRST_ID: i32 = 0x7000_0001;
/// 50 ms ticks.
const TICK_MS: i64 = 50;
const ADENA_ID: i32 = 57;
/// The access level of the `Master` row of the shipped `AccessLevels.xml`.
const GM_LEVEL: i32 = 100;

pub const HUMAN_FIGHTER: i32 = 0;
pub const WARRIOR: i32 = 1;
pub const GLADIATOR: i32 = 2;

/// The Talking Island start point of a new human fighter (`classId 0` `creationPoints`).
const TALKING_ISLAND: (i32, i32, i32) = (-71_338, 258_271, -3_104);

// ------------------------------------------------------------------------------------------
// Where the datapack is
// ------------------------------------------------------------------------------------------

/// The `dist/game` folder of the Java checkout, `None` (and a printed note) without it.
fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var("L2JMOBIUS_GAME_DIR")
        .map_or_else(|_| PathBuf::from(r"C:\l2-rust\l2jmobius\dist\game"), PathBuf::from);
    if dir.join("data").join("stats").is_dir() && dir.join("config").join("AccessLevels.xml").is_file() {
        Some(dir)
    } else {
        eprintln!("skipped: no Java datapack at {}", dir.display());
        None
    }
}

/// The config folder of the test server: Java's defaults, the access levels and admin words of the
/// shipped files, flood protection off and the switch of `SideBySide.ini` (D-53). `extra` are
/// `(file, lines)` appended to a file of the folder.
fn write_game_config(config_dir: &Path, game_dir: &Path, cycle_writes: bool, extra: &[(&str, &str)]) {
    std::fs::create_dir_all(config_dir).unwrap();
    for file in ["AccessLevels.xml", "AdminCommands.xml", "ClassMaster.xml"] {
        std::fs::copy(game_dir.join("config").join(file), config_dir.join(file)).unwrap();
    }
    let flood: String = [
        "UseItem",
        "RollDice",
        "ItemPetSummon",
        "HeroVoice",
        "GlobalChat",
        "Subclass",
        "DropItem",
        "ServerBypass",
        "MultiSell",
        "Transaction",
        "Manufacture",
        "SendMail",
        "CharacterSelect",
        "ItemAuction",
        "PlayerAction",
    ]
    .iter()
    .map(|name| format!("FloodProtector{name}Interval = 0\n"))
    .collect();
    let mut files: Vec<(&str, String)> = vec![
        ("FloodProtector.ini", flood),
        (
            "SideBySide.ini",
            format!(
                "CycleDatabaseWrites = {}\n",
                if cycle_writes { "True" } else { "False" }
            ),
        ),
        (
            "Player.ini",
            "DisconnectAfterDeath = False\nEnableVitality = False\nAltSubClassWithoutQuests = True\n".to_owned(),
        ),
        ("General.ini", "MaxNpcAnimation = 0\nMinimumChatLevel = 0\n".to_owned()),
    ];
    for (file, lines) in extra {
        match files.iter_mut().find(|(name, _)| name == file) {
            Some((_, text)) => text.push_str(lines),
            None => files.push((file, (*lines).to_owned())),
        }
    }
    for (file, text) in files {
        std::fs::write(config_dir.join(file), text).unwrap();
    }
}

// ------------------------------------------------------------------------------------------
// Looking at packets
// ------------------------------------------------------------------------------------------

fn decode_any<P: Decode>(body: &[u8]) -> P {
    let skip = if body[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&body[skip..])).unwrap_or_else(|e| {
        panic!(
            "cannot decode {} as {}: {e:?}",
            packet_name(body),
            std::any::type_name::<P>()
        )
    })
}

/// Every packet of `batch` called `name`, decoded.
fn all<P: Decode>(batch: &[Vec<u8>], name: &str) -> Vec<P> {
    batch
        .iter()
        .filter(|body| packet_name(body) == name)
        .map(|body| decode_any(body))
        .collect()
}

fn names(batch: &[Vec<u8>]) -> Vec<String> {
    names_of(batch)
}

fn count(batch: &[Vec<u8>], name: &str) -> usize {
    batch.iter().filter(|b| packet_name(b) == name).count()
}

fn message_ids(batch: &[Vec<u8>]) -> Vec<SystemMessageId> {
    all::<SystemMessage>(batch, "SYSTEM_MESSAGE")
        .into_iter()
        .map(|m| m.id)
        .collect()
}

fn has_message(batch: &[Vec<u8>], id: SystemMessageId) -> bool {
    message_ids(batch).contains(&id)
}

/// The texts of the system messages with one string (what `sendMessage` sends).
fn texts(batch: &[Vec<u8>]) -> Vec<String> {
    all::<SystemMessage>(batch, "SYSTEM_MESSAGE")
        .into_iter()
        .filter_map(|m| match m.params.as_slice() {
            [SystemMessageParam::Text(text)] => Some(text.clone()),
            _ => None,
        })
        .collect()
}

/// The html of the `NpcHtmlMessage`s and `NpcQuestHtmlMessage`s of `batch`.
fn pages(batch: &[Vec<u8>]) -> Vec<String> {
    let mut pages: Vec<String> = all::<NpcHtmlMessage>(batch, "NPC_HTML_MESSAGE")
        .into_iter()
        .map(|p| p.html)
        .collect();
    pages.extend(
        all::<l2_protocol_game::server::NpcQuestHtmlMessage>(batch, "EX_NPC_QUEST_HTML_MESSAGE")
            .into_iter()
            .map(|p| p.html),
    );
    pages
}

/// The object id of the NPC called `npc_id` among the `NpcInfo` of `batch`.
fn npc_object(batch: &[Vec<u8>], npc_id: i32) -> Option<i32> {
    all::<NpcInfo>(batch, "NPC_INFO")
        .into_iter()
        .find(|n| n.display_id == npc_id)
        .map(|n| n.object_id)
}

fn now_ms() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_millis(),
    )
    .unwrap()
}

// ------------------------------------------------------------------------------------------
// Client actions
// ------------------------------------------------------------------------------------------

/// What a player does, as the input the world takes.
mod act {
    use super::*;

    pub fn click(object_id: i32) -> ClientInput {
        ClientInput::Action {
            object_id,
            action_id: 0,
        }
    }

    pub fn bypass(command: &str) -> ClientInput {
        ClientInput::Bypass {
            command: command.to_owned(),
        }
    }

    /// `//<command>`: the GM's chat command (`SendBypassBuildCmd`, Java puts `admin_` in front).
    pub fn admin(command: &str) -> ClientInput {
        ClientInput::BuildCmd {
            command: command.to_owned(),
        }
    }

    pub fn appearing() -> ClientInput {
        ClientInput::Appearing
    }
}

// ------------------------------------------------------------------------------------------
// The bare world: the real datapack and the real scripts, driven without sockets or database
// ------------------------------------------------------------------------------------------

/// `(object id of the receiver, bytes)`.
type Sent = Vec<(i32, Vec<u8>)>;

/// Packets sent to the players.
#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

impl Log {
    fn take(&self, id: i32) -> Vec<Vec<u8>> {
        let mut all = self.0.lock().unwrap();
        let (mine, others): (Vec<_>, Vec<_>) = all.drain(..).partition(|(to, _)| *to == id);
        *all = others;
        mine.into_iter().map(|(_, bytes)| bytes).collect()
    }
}

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }

    fn left_world(&self, _reason: LeaveReason) {}
}

/// Everything the world sent to the persistence task, in order.
#[derive(Clone, Default)]
struct Persisted(Arc<Mutex<Vec<PersistCommand>>>);

struct RecordingOutbox(Persisted);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        self.0.0.lock().unwrap().push(command);
    }

    fn ls_event(&mut self, _event: LsEvent) {}
}

impl Persisted {
    fn commands(&self) -> Vec<PersistCommand> {
        self.0.lock().unwrap().clone()
    }
}

/// How a scenario starts.
#[derive(Clone)]
struct Opts {
    /// `CycleDatabaseWrites` (D-53).
    cycle_writes: bool,
    /// The rows Sieghardt (the Java game server) keeps (D-39).
    seed: Seed,
    /// More lines for the config files: `(file, lines)`.
    config: Vec<(&'static str, &'static str)>,
    /// One entry per character: where it stands, its class and level, and whether it is a GM.
    players: Vec<Hero>,
}

/// A character of a scenario.
#[derive(Clone)]
struct Hero {
    name: &'static str,
    gm: bool,
    class_id: i32,
    base_class: i32,
    race: i32,
    level: i32,
    position: (i32, i32, i32),
    items: Vec<(i32, i64)>,
    /// `(skill id, level)` of the class the character plays.
    skills: Vec<(i32, i32)>,
}

impl Hero {
    /// A human fighter of level 1 on Talking Island, no items.
    fn new(name: &'static str) -> Self {
        Self {
            name,
            gm: false,
            class_id: HUMAN_FIGHTER,
            base_class: HUMAN_FIGHTER,
            race: 0,
            level: 1,
            position: TALKING_ISLAND,
            items: Vec::new(),
            skills: Vec::new(),
        }
    }

    fn gm(mut self) -> Self {
        self.gm = true;
        self
    }

    fn level(mut self, level: i32) -> Self {
        self.level = level;
        self
    }

    fn class(mut self, class_id: i32) -> Self {
        self.class_id = class_id;
        self.base_class = class_id;
        self
    }

    fn at(mut self, position: (i32, i32, i32)) -> Self {
        self.position = position;
        self
    }

    fn with_items(mut self, items: &[(i32, i64)]) -> Self {
        self.items.extend_from_slice(items);
        self
    }

    fn with_skills(mut self, skills: &[(i32, i32)]) -> Self {
        self.skills.extend_from_slice(skills);
        self
    }
}

impl Opts {
    fn new(cycle_writes: bool, players: Vec<Hero>) -> Self {
        Self {
            cycle_writes,
            seed: Seed::default(),
            config: Vec::new(),
            players,
        }
    }
}

struct Bare {
    world: World,
    log: Log,
    persisted: Persisted,
    /// The model of the rows the persistence task would read.
    rows: Rows,
    /// How many of the persistence commands the model has looked at.
    answered: usize,
    players: Vec<PlayerData>,
    datapack: Arc<Datapack>,
    /// Keeps the folder of the config (nothing reads it after the start).
    _root: tempfile::TempDir,
}

impl Bare {
    /// Loads the configuration, the datapack of the Java checkout, the zones and the html the way
    /// `Server::start_with` does, installs the real scripts, then makes the characters. `None`
    /// without the Java checkout.
    fn start(opts: &Opts) -> Option<Self> {
        let game_dir = game_dir()?;
        let root = tempfile::tempdir().unwrap();
        let config_dir = root.path().join("config");
        write_game_config(&config_dir, &game_dir, opts.cycle_writes, &opts.config);

        let config = GameConfig::load(&config_dir).unwrap();
        // The datapack options of the config (`options_from_config` of the server, which is
        // private): the defaults of Java, no spawn files (a test places what it needs).
        let options = DatapackOptions {
            skills: skill_load_options(&config, &config_dir),
            max_aggro_range: config.npc.max_aggro_range,
            default_corpse_time: config.npc.default_corpse_time,
            no_spawns: true,
            ..DatapackOptions::default()
        };
        let datapack = Arc::new(Datapack::load_with(&game_dir, &options).unwrap());
        let static_data: Arc<dyn StaticData> =
            Arc::new(DatapackData::load(Arc::clone(&datapack), &config_dir));
        let settings = world_settings(&config, &config_dir);
        let geo_settings = geo::GeoSettings::load(&config, &config_dir, &game_dir);
        let engine = geo::load_engine(&geo_settings);
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());

        let persisted = Persisted::default();
        let mut world = World::new(
            WorldRng::from_seed(7),
            Box::new(RecordingOutbox(persisted.clone())),
        );
        world.set_static_data(Box::new(static_data));
        world.set_settings(settings);
        world.set_regen_settings(regen_settings(&config));
        world.set_summon_settings(summon_settings(&config));
        world.set_geo(engine);
        world.set_zone_data(Arc::clone(&datapack.zones), Arc::clone(&datapack.map_regions));
        world.set_html_cache(Arc::clone(&datapack.html));
        world.set_teleporter_data(Arc::clone(&datapack.teleporters));
        world.set_hit_condition_bonus(Arc::clone(&datapack.hit_condition_bonus));
        world.set_exp_data(
            Arc::clone(&datapack.experience),
            Arc::clone(&datapack.experience_loss),
            Arc::clone(&datapack.karma_loss),
            Arc::clone(&datapack.skill_trees),
        );
        world.set_skill_learn_data(Arc::clone(&datapack.skill_learn));
        world.set_pet_skill_data(Arc::clone(&datapack.pet_skills));
        world.set_pet_data(Arc::clone(&datapack.pets));
        world.set_cubic_data(Arc::clone(&datapack.cubics));
        world.set_multisell_data(Arc::clone(&datapack.multisell), multisell_settings(&config));
        world.set_enchant_data(
            Arc::clone(&datapack.enchant_items),
            Arc::clone(&datapack.enchant_groups),
            Arc::clone(&datapack.elemental_attributes),
        );
        world.set_augmentation_data(datapack.augmentation_data.clone());
        world.spawn_door_data(&datapack, &ids).unwrap();
        world.set_class_list(Arc::clone(&datapack.class_list));
        world.set_instance_settings(game_server::world_data::instance_settings(&config, &config_dir));
        world.set_instance_data(Arc::clone(&datapack.instances));
        world.set_walking_routes(Arc::clone(&datapack.routes));

        let mut rows = Rows::new(&opts.seed);
        for (index, hero) in opts.players.iter().enumerate() {
            rows.seed_quests(&opts.seed, hero.name, Self::id_of(index));
        }
        // Java: RaidBossPointsManager and GrandBossManager before the "Scripts" section.
        world.set_boss_settings(game_server::world_data::boss_settings(&config, &config_dir));
        world.load_bosses(opts.seed.boss_load());
        world.set_script_settings(game_server::script_settings(&config, &config_dir));
        world.set_global_variables(opts.seed.global_variables());
        world.set_category_data(Arc::clone(&datapack.categories));
        world.set_level_up_crystal_data(Arc::new(l2_data::LevelUpCrystalData::load(
            &datapack.root,
            config.player.soul_crystal_chance_multiplier,
        )));
        // Java: the "Scripts" section, then the spawns (none here: `no_spawns`).
        let scripts = l2_scripts::all_with(l2_data::ClassMasterConfig::load(&datapack.root));
        npcs::spawn_npcs_with_scripts(&mut world, &datapack, ids, &config, &config_dir, Some(scripts));
        world.spawn_raid_bosses(Vec::new());
        let buy_options = BuyListOptions {
            custom_buylist_load: config.general.custom_buylist_load,
            correct_prices: config.general.correct_prices,
        };
        world.set_buy_lists(
            Arc::new(BuyLists::load(&datapack.root, &datapack.items, &buy_options)),
            Arc::new(MerchantPriceConfigs::load(&datapack.root)),
        );
        world.load_crests(Vec::new());

        let players: Vec<PlayerData> = opts
            .players
            .iter()
            .enumerate()
            .map(|(index, hero)| bare_player(&datapack, index, hero, &opts.seed))
            .collect();
        Some(Self {
            world,
            log: Log::default(),
            persisted,
            rows,
            answered: 0,
            players,
            datapack,
            _root: root,
        })
    }

    fn id_of(index: usize) -> i32 {
        BARE_FIRST_ID + i32::try_from(index).unwrap()
    }

    fn me(&self, index: usize) -> ObjectId {
        ObjectId(Self::id_of(index))
    }

    /// Player `index` does `input` (the packets it causes are left in the log).
    fn input_of(&mut self, index: usize, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(index + 1).unwrap()),
            input,
        });
        self.answer_reads();
    }

    fn ticks(&mut self, count: usize) {
        for _ in 0..count {
            self.world.tick();
            self.answer_reads();
        }
    }

    /// `ms` of world time.
    fn run_ms(&mut self, ms: i64) {
        self.ticks(usize::try_from(ms / TICK_MS).unwrap());
    }

    /// What player `index` got since the last call, after a quiet moment of 500 ms.
    fn quiet_of(&mut self, index: usize) -> Vec<Vec<u8>> {
        self.ticks(10);
        self.log.take(Self::id_of(index))
    }

    /// Player `index` does `input`, then the packets of a quiet moment.
    fn do_of(&mut self, index: usize, input: ClientInput) -> Vec<Vec<u8>> {
        self.input_of(index, input);
        self.quiet_of(index)
    }

    /// Enters player `index` (the character as the world stored it when it left, the first time
    /// the new one); returns the packets up to the closing `SKILL_LIST`.
    fn enter_player(&mut self, index: usize) -> Vec<Vec<u8>> {
        let id = Self::id_of(index);
        let player = self
            .persisted
            .commands()
            .into_iter()
            .rev()
            .find_map(|c| match c {
                PersistCommand::StorePlayer(data) if data.object_id == ObjectId(id) => Some(*data),
                _ => None,
            })
            .unwrap_or_else(|| self.players[index].clone());
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(index + 1).unwrap()),
            player: Box::new(player),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
        self.answer_reads();
        // The queued script events of the login (`ON_PLAYER_LOGIN`, the tutorial) run in the ticks.
        self.ticks(4);
        self.log.take(id)
    }

    /// Player `index` logs out; the world stores the character.
    fn leave(&mut self, index: usize) {
        self.input_of(index, ClientInput::Logout);
        self.ticks(2);
        self.log.take(Self::id_of(index));
    }

    /// The reads the persistence task would answer: they come from the model of the rows.
    fn answer_reads(&mut self) {
        loop {
            let commands = self.persisted.commands();
            if self.answered >= commands.len() {
                break;
            }
            let new = commands[self.answered..].to_vec();
            self.answered = commands.len();
            for command in new {
                self.rows.apply(&command);
                self.rows.answer(&mut self.world, &command);
            }
        }
    }

    /// Every persistence command so far.
    fn commands(&self) -> Vec<PersistCommand> {
        self.persisted.commands()
    }

    fn player_data(&self, index: usize) -> PlayerData {
        self.world.player(self.me(index)).unwrap().data.clone()
    }

    fn items_of(&self, index: usize, item_id: i32) -> i64 {
        self.world
            .player(self.me(index))
            .map(|p| {
                p.data
                    .items
                    .iter()
                    .filter(|i| i.item_id == item_id)
                    .map(|i| i.count)
                    .sum()
            })
            .unwrap_or(0)
    }
}

/// A character of `hero` with the experience of its level.
fn bare_player(datapack: &Datapack, index: usize, hero: &Hero, seed: &Seed) -> PlayerData {
    let id = BARE_FIRST_ID + i32::try_from(index).unwrap();
    let base = 0x7100_0000 + i32::try_from(index * 100).unwrap();
    let mut items = vec![ItemData {
        object_id: ObjectId(base + 99),
        item_id: ADENA_ID,
        count: 1_000_000,
        loc: "INVENTORY".to_owned(),
        mana_left: -1,
        time: -1,
        ..Default::default()
    }];
    for (position, (item_id, count)) in hero.items.iter().enumerate() {
        items.push(ItemData {
            object_id: ObjectId(base + 50 + i32::try_from(position).unwrap()),
            item_id: *item_id,
            count: *count,
            loc: "INVENTORY".to_owned(),
            mana_left: -1,
            time: -1,
            ..Default::default()
        });
    }
    let exp = datapack.experience.exp_for_level(hero.level).unwrap_or(0);
    let mut data = PlayerData {
        object_id: ObjectId(id),
        account: format!("e2esacc{index}"),
        name: hero.name.to_owned(),
        title: String::new(),
        class_id: hero.class_id,
        base_class: hero.base_class,
        race: hero.race,
        sex: 0,
        face: 1,
        hair_style: 1,
        hair_color: 1,
        level: hero.level,
        exp,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: if hero.gm { GM_LEVEL } else { 0 },
        x: hero.position.0 + i32::try_from(index * 20).unwrap(),
        y: hero.position.1,
        z: hero.position.2,
        heading: 0,
        cur_hp: 5_000.0,
        max_hp: 5_000.0,
        cur_mp: 1_000.0,
        max_mp: 1_000.0,
        cur_cp: 1_000.0,
        max_cp: 1_000.0,
        online_time: 0,
        items,
        skills: hero
            .skills
            .iter()
            .map(|(skill_id, skill_level)| l2_world::SkillData {
                skill_id: *skill_id,
                skill_level: *skill_level,
            })
            .collect(),
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    };
    data.profile.quests = seed.quests_of(hero.name);
    data
}

/// The commands a page offers: `<a action="bypass -h npc_1_Foo">` is the bypass `npc_1_Foo`.
fn links(html: &str) -> Vec<String> {
    let mut found = Vec::new();
    let mut rest = html;
    while let Some(at) = rest.find("action=\"") {
        rest = &rest[at + 8..];
        let Some(end) = rest.find('"') else { break };
        let action = &rest[..end];
        if let Some(command) = action.strip_prefix("bypass -h ").or_else(|| action.strip_prefix("bypass ")) {
            found.push(command.to_owned());
        }
        rest = &rest[end..];
    }
    found
}

impl Bare {
    /// The GM `index` spawns the NPC `npc_id` `dx` units east of itself (`//spawnat`); returns the
    /// object id the client got in `NpcInfo`.
    fn spawn_npc(&mut self, index: usize, npc_id: i32, dx: i32) -> i32 {
        let p = self.player_data(index);
        let batch = self.do_of(
            index,
            act::admin(&format!("spawnat {npc_id} {} {} {}", p.x + dx, p.y, p.z)),
        );
        npc_object(&batch, npc_id)
            .unwrap_or_else(|| panic!("NPC {npc_id} did not appear: {:?} {:?}", names(&batch), texts(&batch)))
    }

    /// Talks to an NPC: the first click targets it, the second opens the dialog.
    fn talk(&mut self, index: usize, npc: i32) -> Vec<Vec<u8>> {
        self.input_of(index, act::click(npc));
        self.input_of(index, act::click(npc));
        self.quiet_of(index)
    }

    /// Clicks the link of the last page of `batch` whose command contains `part`.
    fn follow(&mut self, index: usize, batch: &[Vec<u8>], part: &str) -> Vec<Vec<u8>> {
        let page = pages(batch).pop().unwrap_or_else(|| panic!("no page in {:?}", names(batch)));
        let command = links(&page)
            .into_iter()
            .find(|c| c.contains(part))
            .unwrap_or_else(|| panic!("no link {part:?} in {page}"));
        self.do_of(index, act::bypass(&command))
    }
}

// ------------------------------------------------------------------------------------------
// The tests
// ------------------------------------------------------------------------------------------

const TUTORIAL: &str = "Q00255_Tutorial";
const GUARD_IS_BUSY: &str = "Q00257_TheGuardIsBusy";
const GILBERT: i32 = 30039;
const LORDS_MARK: i32 = 1084;
const ORC_NECKLACE: i32 = 1085;
const ORC_FIGHTER: i32 = 20093;

/// The GM `index` targets the monster `mob` and kills it with `//kill` (the GM is the attacker,
/// so it is the killer the quests see). A level 8 hero with bare hands loses to an orc fighter, so
/// the kill is the admin's, as in the other e2e files. Returns what the client got up to the
/// kill delay of the scripts (2.5 s) after the death.
fn slay(w: &mut Bare, index: usize, mob: i32) -> Vec<Vec<u8>> {
    w.input_of(index, act::click(mob));
    w.input_of(index, act::admin("kill"));
    w.run_ms(3_000);
    let got = w.log.take(Bare::id_of(index));
    assert!(
        all::<l2_protocol_game::server::Die>(&got, "DIE")
            .iter()
            .any(|d| d.object_id == mob),
        "the monster did not die: {:?}",
        names(&got)
    );
    got
}

/// 1. A new character: the tutorial starts (CP-Sa).
#[test]
fn a_new_character_gets_the_tutorial() {
    let opts = Opts::new(false, vec![Hero::new("Newbie")]);
    let Some(mut w) = Bare::start(&opts) else {
        return;
    };
    let me = w.me(0).0;
    let entered = w.enter_player(0);
    // `Q00255_Tutorial.onEnterWorld`: the state row, the memo bits and the events of the client.
    assert_eq!(w.rows.quest_var(me, TUTORIAL, "<state>"), Some("Started"));
    assert_eq!(w.rows.quest_var(me, TUTORIAL, "memoStateEx1"), Some("-2"));
    assert!(w.rows.quest_var(me, TUTORIAL, "memoState").is_some());
    assert!(names(&entered).contains(&"TUTORIAL_ENABLE_CLIENT_EVENT".to_owned()), "{:?}", names(&entered));

    // 5 s later the first page of the tutorial window and the guide book.
    w.run_ms(5_000);
    let first = w.quiet_of(0);
    let shown: Vec<String> = all::<l2_protocol_game::server::TutorialShowHtml>(&first, "TUTORIAL_SHOW_HTML")
        .into_iter()
        .map(|p| p.html)
        .collect();
    assert!(
        shown.len() == 1 && shown[0].contains("Cedric's Training Hall") && shown[0].contains("tutorial_close_2"),
        "{shown:?}"
    );
    assert_eq!(w.items_of(0, 5588), 1, "the Tutorial Guide");

    // The page's link: the window closes and the next topic is announced.
    let next = w.do_of(
        0,
        ClientInput::Quest(QuestInput::TutorialLinkHtml {
            bypass: "tutorial_close_2".to_owned(),
        }),
    );
    // `tutorialEvent(-2)`: the voice, the second page of the window and the client's events.
    let second: Vec<String> = all::<l2_protocol_game::server::TutorialShowHtml>(&next, "TUTORIAL_SHOW_HTML")
        .into_iter()
        .map(|p| p.html)
        .collect();
    assert_eq!(second.len(), 1, "{:?}", names(&next));
    assert!(names(&next).contains(&"PLAY_SOUND".to_owned()));
    assert!(names(&next).contains(&"TUTORIAL_ENABLE_CLIENT_EVENT".to_owned()));
    assert_eq!(w.rows.quest_var(me, TUTORIAL, "memoStateEx1"), Some("-5"));
}

/// 1. A first village quest: talk, kill, reward, and aborting it (CP-Sa).
#[test]
fn a_first_village_quest_from_talk_to_reward_and_abort() {
    let opts = Opts::new(false, vec![Hero::new("Fighter").gm().level(8)]);
    let Some(mut w) = Bare::start(&opts) else {
        return;
    };
    let me = w.me(0).0;
    w.enter_player(0);
    let gilbert = w.spawn_npc(0, GILBERT, 30);

    // Gilbert's page has the quest link; the quest window offers the quest at level 8.
    let talked = w.talk(0, gilbert);
    let quest = w.follow(0, &talked, "Script");
    assert!(pages(&quest).iter().any(|p| p.contains("Say you will participate in the hunt")));
    // Say yes: Gilbert hands over the Lord's mark and the quest starts (cond 1).
    let started = w.follow(0, &quest, "30039-03.htm");
    assert!(pages(&started).iter().any(|p| p.contains("Go and slay orcs and werewolves")));
    assert_eq!(w.items_of(0, LORDS_MARK), 1);
    assert_eq!(w.rows.quest_var(me, GUARD_IS_BUSY, "<state>"), Some("Started"));
    assert_eq!(w.rows.quest_var(me, GUARD_IS_BUSY, "cond"), Some("1"));
    assert!(names(&started).contains(&"QUEST_LIST".to_owned()));

    // Kill orc fighters (85% for a necklace).
    for _ in 0..30 {
        if w.items_of(0, ORC_NECKLACE) >= 2 {
            break;
        }
        let orc = w.spawn_npc(0, ORC_FIGHTER, 40);
        slay(&mut w, 0, orc);
    }
    let necklaces = w.items_of(0, ORC_NECKLACE);
    assert!(necklaces >= 2, "{necklaces}");

    // Back to Gilbert: 20 adena a necklace (plus 1000 for ten), the rookie soulshots, once.
    let adena = w.items_of(0, ADENA_ID);
    let talked = w.talk(0, gilbert);
    let quest = w.follow(0, &talked, "Script");
    assert!(pages(&quest).iter().any(|p| p.contains("Gilbert")));
    assert_eq!(w.items_of(0, ADENA_ID), adena + necklaces * 20);
    assert_eq!(w.items_of(0, ORC_NECKLACE), 0);
    assert_eq!(w.items_of(0, 5789), 6000, "Soulshot: No Grade for Beginners");

    // The journal lists the quest; the abort deletes it (`exitQuest(true)`, the quest item goes).
    let list = w.do_of(0, ClientInput::Quest(QuestInput::List));
    let listed = all::<l2_protocol_game::server::QuestList>(&list, "QUEST_LIST");
    assert!(listed[0].quests.iter().any(|q| q.quest_id == 257));
    let aborted = w.do_of(0, ClientInput::Quest(QuestInput::Abort { quest_id: 257 }));
    assert_eq!(w.rows.quest_var(me, GUARD_IS_BUSY, "<state>"), None);
    assert_eq!(w.rows.quest_var(me, GUARD_IS_BUSY, "cond"), None);
    assert_eq!(w.items_of(0, LORDS_MARK), 0);
    let listed = all::<l2_protocol_game::server::QuestList>(&aborted, "QUEST_LIST");
    assert!(listed.last().unwrap().quests.iter().all(|q| q.quest_id != 257));
}

const VILLAGE_MASTER: &str = "ElfHumanFighterChange1";
const PABRIS: i32 = 30066;
const MEDALLION_OF_WARRIOR: i32 = 1145;
const SHADOW_COUPON_D: i32 = 8869;

/// 2. The first class change at a village master (CP-Sb).
#[test]
fn the_first_class_change_at_a_village_master() {
    let opts = Opts::new(
        false,
        vec![Hero::new("Fighter").gm().level(20).with_items(&[(MEDALLION_OF_WARRIOR, 1)])],
    );
    let Some(mut w) = Bare::start(&opts) else {
        return;
    };
    w.enter_player(0);
    let pabris = w.spawn_npc(0, PABRIS, 30);
    let talked = w.talk(0, pabris);
    let list = w.follow(0, &talked, "Script ElfHumanFighterChange1");
    assert!(pages(&list).last().unwrap().contains("Warrior"), "{:?}", pages(&list));
    let info = w.follow(0, &list, "30066-02.htm");
    // The page of the Warrior has the link that asks for the change.
    let changed = w.follow(0, &info, "ElfHumanFighterChange1 1");
    assert!(pages(&changed).last().unwrap().contains("Congratulations! You are now a Warrior"));
    assert!(names(&changed).contains(&"MAGIC_SKILL_USE".to_owned()), "the class change effect");
    assert!(has_message(&changed, SystemMessageId::CongratulationsYouVeCompletedAClassTransfer));
    let data = w.player_data(0);
    assert_eq!((data.class_id, data.base_class), (WARRIOR, WARRIOR));
    assert_eq!(w.items_of(0, MEDALLION_OF_WARRIOR), 0, "the proof is taken");
    assert_eq!(w.items_of(0, SHADOW_COUPON_D), 15);
    assert!(names(&changed).contains(&"USER_INFO".to_owned()));
}

const HANNAVALT: i32 = 30109;
const DYE: i32 = 37;
const DYE_ITEM: i32 = 4481;
const POWER_STRIKE: i32 = 3;

/// Hannavalt's page and its "Sub Class" link.
fn sub_class_menu(w: &mut Bare, master: i32) -> Vec<Vec<u8>> {
    let talked = w.talk(0, master);
    w.follow(0, &talked, "Subclass 0")
}

/// 2. A subclass added, switched to, replaced and the base class back (CP-Sb): the skills and the
/// dye follow the class.
#[test]
fn a_subclass_is_added_switched_and_cancelled() {
    let opts = Opts::new(
        false,
        vec![
            Hero::new("Fighter")
                .gm()
                .level(76)
                .class(GLADIATOR)
                .with_items(&[(DYE_ITEM, 10)])
                .with_skills(&[(POWER_STRIKE, 1)]),
        ],
    );
    let Some(mut w) = Bare::start(&opts) else {
        return;
    };
    let me = w.me(0).0;
    w.enter_player(0);

    // A dye on the Gladiator (a real client request): slot 1.
    let equipped = w.do_of(0, ClientInput::Henna(l2_world::HennaInput::Equip { symbol_id: DYE }));
    assert!(has_message(&equipped, SystemMessageId::TheSymbolHasBeenAdded), "{:?}", message_ids(&equipped));
    assert_eq!(w.player_data(0).profile.hennas, [DYE, 0, 0]);
    assert_eq!(w.items_of(0, DYE_ITEM), 0);

    // Hannavalt's page: the sub class menu, then "add".
    let master = w.spawn_npc(0, HANNAVALT, 30);
    let talked = w.talk(0, master);
    let menu = w.follow(0, &talked, "Subclass 0");
    let add = w.follow(0, &menu, "Subclass 1");
    let offered = links(&pages(&add).pop().unwrap());
    assert!(offered.iter().any(|c| c.ends_with("Subclass 4 3")), "{offered:?}");
    let added = w.follow(0, &add, "Subclass 4 3");
    assert!(has_message(&added, SystemMessageId::TheNewSubclassHasBeenAdded), "{:?}", message_ids(&added));
    assert!(pages(&added).last().unwrap().contains("You've added a new subclass"), "{:?}", pages(&added));
    let writes: Vec<SubclassWrite> = w
        .commands()
        .into_iter()
        .filter_map(|c| match c {
            PersistCommand::Subclass(write) => Some(write),
            _ => None,
        })
        .collect();
    let [SubclassWrite::Add { holder, skills: sub_skills, .. }] = writes.as_slice() else {
        panic!("{writes:?}");
    };
    assert_eq!((holder.class_id, holder.class_index, holder.level), (3, 1, 40));
    assert!(!sub_skills.is_empty(), "the skills of the new class up to level 40");
    // The new class is active: its level, no dye, its skills (the base class' ones are stored).
    let data = w.player_data(0);
    assert_eq!((data.class_id, data.base_class, data.profile.class_index, data.level), (3, GLADIATOR, 1, 40));
    assert_eq!(data.profile.hennas, [0, 0, 0]);
    assert_eq!(data.skills, *sub_skills);
    assert!(w.rows.has_skill(me, 0, POWER_STRIKE), "the base class' skills were stored");

    // Back to the base class: the level, the skills and the dye come back.
    let menu = sub_class_menu(&mut w, master);
    let menu = w.follow(0, &menu, "Subclass 2");
    let back = w.follow(0, &menu, "Subclass 5 0");
    assert!(has_message(&back, SystemMessageId::YouHaveSuccessfullySwitchedToYourSubclass));
    let data = w.player_data(0);
    assert_eq!((data.class_id, data.profile.class_index, data.level), (GLADIATOR, 0, 76));
    assert_eq!(data.profile.hennas, [DYE, 0, 0]);
    assert!(data.skills.iter().any(|s| s.skill_id == POWER_STRIKE));
    assert!(names(&back).contains(&"SKILL_LIST".to_owned()));
    assert!(names(&back).contains(&"HENNA_INFO".to_owned()));

    // And to the subclass again: no dye.
    let menu = sub_class_menu(&mut w, master);
    let menu = w.follow(0, &menu, "Subclass 2");
    w.follow(0, &menu, "Subclass 5 1");
    let data = w.player_data(0);
    assert_eq!((data.class_id, data.profile.class_index, data.level), (3, 1, 40));
    assert_eq!(data.profile.hennas, [0, 0, 0]);

    // Cancel it and take another class in its place (index 1 again).
    let menu = sub_class_menu(&mut w, master);
    let menu = w.follow(0, &menu, "Subclass 3");
    let choice = w.follow(0, &menu, "Subclass 6 1");
    let page = pages(&choice).pop().unwrap();
    let replace = links(&page)
        .into_iter()
        .find(|c| c.contains("Subclass 7 1 "))
        .unwrap_or_else(|| panic!("{page}"));
    let new_class: i32 = replace.rsplit(' ').next().unwrap().parse().unwrap();
    let replaced = w.do_of(0, act::bypass(&replace));
    assert!(has_message(&replaced, SystemMessageId::TheNewSubclassHasBeenAdded), "{:?}", message_ids(&replaced));
    let writes: Vec<SubclassWrite> = w
        .commands()
        .into_iter()
        .filter_map(|c| match c {
            PersistCommand::Subclass(write) => Some(write),
            _ => None,
        })
        .collect();
    assert!(
        matches!(writes[1], SubclassWrite::Delete { class_index: 1, .. }),
        "{:?}",
        writes[1]
    );
    assert!(matches!(
        writes[2],
        SubclassWrite::Add { holder, .. } if holder.class_id == new_class && holder.class_index == 1
    ));
    let data = w.player_data(0);
    assert_eq!((data.class_id, data.profile.class_index, data.level), (new_class, 1, 40));
}

const BATHIS: i32 = 30332;
/// The template of Kamaloka (Hall of the Abyss, level 23) and its boss.
const KAMALOKA_TEMPLATE: i32 = 57;
const KAMALOKA_BOSS: i32 = 18554;

/// 3. A Kamaloka entered by a party of two, the boss killed inside, the way out and the re-enter
/// time (CP-Sc).
#[test]
fn a_kamaloka_is_entered_its_boss_killed_and_the_reenter_time_holds() {
    let opts = Opts::new(
        false,
        vec![
            Hero::new("Leader").gm().level(25),
            Hero::new("Friend").level(25),
        ],
    );
    let Some(mut w) = Bare::start(&opts) else {
        return;
    };
    let (leader, friend) = (w.me(0), w.me(1));
    w.enter_player(0);
    w.enter_player(1);
    w.input_of(
        0,
        ClientInput::JoinParty {
            name: "Friend".to_owned(),
            distribution_type_id: 0,
        },
    );
    w.input_of(1, ClientInput::AnswerJoinParty { response: 1 });
    assert!(w.world.is_in_party_with(leader, friend), "the party was not made");
    w.quiet_of(0);
    w.quiet_of(1);

    // Captain Bathis: the Hall of the Abyss, level 23.
    let captain = w.spawn_npc(0, BATHIS, 30);
    let talked = w.talk(0, captain);
    let talked = w.follow(0, &talked, "Script");
    assert!(pages(&talked).last().unwrap().contains("Do you wish to enter Kamaloka"));
    let entered = w.follow(0, &talked, "Script Kamaloka 0");
    let instance = w.world.instance_id_of(leader);
    assert!(instance != 0, "the leader is in an instance");
    assert_eq!(w.world.instance_id_of(friend), instance, "the party goes in together");
    assert_eq!(
        w.world.instance(instance).map(|i| i.template_id),
        Some(KAMALOKA_TEMPLATE)
    );
    assert!(names(&entered).contains(&"TELEPORT_TO_LOCATION".to_owned()), "{:?}", names(&entered));
    let friend_got = w.quiet_of(1);
    assert!(names(&friend_got).contains(&"TELEPORT_TO_LOCATION".to_owned()));
    w.input_of(0, act::appearing());
    w.input_of(1, act::appearing());
    w.quiet_of(0);
    w.quiet_of(1);

    // The boss of the hall waits inside: the leader kills it and the instance ends.
    let boss = w
        .world
        .npc_ids()
        .into_iter()
        .find(|n| w.world.npc(*n).map(|npc| npc.npc_id) == Some(KAMALOKA_BOSS) && w.world.instance_id_of(*n) == instance)
        .expect("the boss of the hall");
    w.world.do_die(boss, Some(leader));
    w.run_ms(3_000);
    let done = w.log.take(Bare::id_of(0));
    assert!(
        has_message(
            &done,
            SystemMessageId::InstantZoneS1SEntryHasBeenRestrictedYouCanCheckTheNextPossibleEntryTimeByUsingTheCommandInstancezone
        ),
        "{:?}",
        message_ids(&done)
    );
    // The re-enter time of both players is the next 06:30 (a `character_instance_time` row each).
    let now = now_ms();
    for who in [leader, friend] {
        let time = w.rows.instance_times.get(&(who.0, KAMALOKA_TEMPLATE)).copied().unwrap();
        assert!(time > now && time <= now + 86_400_000, "{time} vs {now}");
    }

    // After the 5 minutes the instance gives the party back to the world.
    w.run_ms(5 * 60_000);
    w.quiet_of(0);
    assert_eq!(w.world.instance_id_of(leader), 0, "the leader is out");
    assert_eq!(w.world.instance_id_of(friend), 0, "the friend is out");

    // A second try the same day is refused for the member who may not re-enter yet.
    let talked = w.talk(0, captain);
    let talked = w.follow(0, &talked, "Script");
    let again = w.follow(0, &talked, "Script Kamaloka 0");
    assert!(has_message(&again, SystemMessageId::C1MayNotReEnterYet), "{:?}", message_ids(&again));
    assert_eq!(w.world.instance_id_of(leader), 0);
}

const TYPHOON: i32 = 25539;
const GUST: i32 = 5434;

/// The object ids of the NPCs called `npc_id` that are in the world.
fn npcs_of(w: &Bare, npc_id: i32) -> Vec<ObjectId> {
    w.world
        .npc_ids()
        .into_iter()
        .filter(|id| w.world.npc(*id).is_some_and(|n| n.npc_id == npc_id && !n.decayed))
        .collect()
}

/// The places `ai/others/RandomSpawn` sends Typhoon to.
const TYPHOON_PLACES: [(i32, i32, i32); 10] = [
    (-20641, 255370, -3235),
    (-16157, 250993, -3058),
    (-18269, 250721, -3151),
    (-16532, 254864, -3223),
    (-19055, 253489, -3440),
    (-9684, 254256, -3148),
    (-6209, 251924, -3189),
    (-10547, 251359, -2929),
    (-7254, 254997, -3261),
    (-4883, 253171, -3322),
];

/// 4. A raid boss and its AI: Typhoon (Hellbound) is sent to one of its places by `RandomSpawn`,
/// casts its Gust on a timer once it is there and again when a player comes into its aggro range.
#[test]
fn a_raid_boss_ai_starts_the_fight() {
    let opts = Opts::new(false, vec![Hero::new("Slayer").gm().level(80).class(GLADIATOR)]);
    let Some(mut w) = Bare::start(&opts) else {
        return;
    };
    w.enter_player(0);
    let created = w.do_of(0, act::admin(&format!("spawn {TYPHOON}")));
    assert!(texts(&created).iter().any(|t| t.starts_with("Created Typhoon")), "{:?}", texts(&created));
    let boss = npcs_of(&w, TYPHOON);
    assert_eq!(boss.len(), 1);
    let boss = boss[0];

    // `RandomSpawn.onSpawn`: 100 ms later the boss is at one of its places, away from the GM.
    let (x, y, z) = w.world.position_of(boss).unwrap();
    assert!(
        TYPHOON_PLACES.iter().any(|(px, py, _)| (px - x).abs() <= 200 && (py - y).abs() <= 200),
        "{:?}",
        (x, y, z)
    );

    // The GM goes there: the boss gets a player in its aggro range and the Gust goes off; the
    // timer of `onSpawn` repeats it every 5 seconds.
    w.log.take(Bare::id_of(0));
    w.do_of(0, act::admin(&format!("move_to {} {} {}", x + 60, y, z)));
    w.input_of(0, act::appearing());
    w.run_ms(12_000);
    let fight = w.log.take(Bare::id_of(0));
    let casts: Vec<_> = all::<l2_protocol_game::server::MagicSkillUse>(&fight, "MAGIC_SKILL_USE")
        .into_iter()
        .filter(|c| c.creature_id == boss.0)
        .collect();
    assert!(casts.len() >= 2, "{} casts of the boss in {:?}", casts.len(), names(&fight));
    assert!(casts.iter().all(|c| c.skill_id == GUST));
}

const QUEEN_ANT: i32 = 29001;
const QUEEN_NEST: (i32, i32, i32) = (-21_610, 181_594, -5_734);
const ALIVE: i32 = 0;
const DEAD: i32 = 1;

/// A world whose `grandboss_data` has Queen Ant with this `status` and `respawn_time`.
fn queen_world(status: i32, respawn_time: i64, cycle_writes: bool) -> Option<Bare> {
    let mut opts = Opts::new(cycle_writes, vec![Hero::new("Watcher").gm().level(40)]);
    opts.seed.grand_bosses = vec![scripts_db::grand_boss_row(QUEEN_ANT, status, respawn_time, QUEEN_NEST)];
    let mut w = Bare::start(&opts)?;
    w.enter_player(0);
    w.run_ms(5_000);
    Some(w)
}

fn boss_writes(w: &Bare) -> Vec<l2_world::BossWrite> {
    w.commands()
        .into_iter()
        .filter_map(|c| match c {
            PersistCommand::Boss(write) => Some(write),
            _ => None,
        })
        .collect()
}

/// 4. D-54: a grand boss whose row (Sieghardt's) says dead stays unspawned on Kain, whatever the
/// respawn time says, and Kain writes nothing to the row.
#[test]
fn a_grand_boss_whose_row_says_dead_stays_unspawned() {
    // Dead, respawn time in the future: the unlock timer waits.
    let Some(w) = queen_world(DEAD, now_ms() + 3_600_000, false) else {
        return;
    };
    assert!(npcs_of(&w, QUEEN_ANT).is_empty(), "Queen Ant is not spawned");
    assert_eq!(w.world.grand_boss_status(QUEEN_ANT), Some(DEAD));
    assert!(boss_writes(&w).is_empty(), "{:?}", boss_writes(&w));

    // Dead, respawn time passed: Kain reads the row again and the row still says dead.
    let Some(w) = queen_world(DEAD, now_ms() - 60_000, false) else {
        return;
    };
    assert!(npcs_of(&w, QUEEN_ANT).is_empty(), "Queen Ant is not spawned");
    assert_eq!(w.world.grand_boss_status(QUEEN_ANT), Some(DEAD));
    assert!(boss_writes(&w).is_empty(), "{:?}", boss_writes(&w));
}

/// 4. The same boss with an alive row runs on Kain, and with the switch on (the Rust server alone
/// on the database) a dead row whose respawn time has passed brings her back.
#[test]
fn a_grand_boss_whose_row_says_alive_is_spawned() {
    let Some(w) = queen_world(ALIVE, 0, false) else {
        return;
    };
    let queen = npcs_of(&w, QUEEN_ANT);
    assert_eq!(queen.len(), 1, "Queen Ant is spawned");
    let (x, y, _) = w.world.position_of(queen[0]).unwrap();
    assert!((x - QUEEN_NEST.0).abs() < 1_000 && (y - QUEEN_NEST.1).abs() < 1_000);

    let Some(w) = queen_world(DEAD, now_ms() - 60_000, true) else {
        return;
    };
    assert_eq!(npcs_of(&w, QUEEN_ANT).len(), 1, "Kain owns the row with the switch on");
}

const DROPH: i32 = 32711;
const HEKATON_PRIME: i32 = 25687;
const CONTROL_DEVICE: &str = "Q00307_ControlDeviceOfTheGiants";
const RESPAWN_VARIABLE: &str = "GiantsControlDeviceRespawn";
const SUPPORT_ITEMS: i32 = 14850;

fn global_writes(w: &Bare) -> Vec<l2_world::script::GlobalVariablesWrite> {
    w.commands()
        .into_iter()
        .filter_map(|c| match c {
            PersistCommand::GlobalVariables(write) => Some(write),
            _ => None,
        })
        .collect()
}

/// 5. Side by side (CP-Sd): Sieghardt wrote the rows of quest 307 in progress for "Leader" (and a
/// `global_variables` row of its own). Kain continues the quest after the login, the boss dies in
/// the party, and the `global_variables` row of the quest is written only with the switch on.
fn side_by_side(cycle_writes: bool) {
    let mut opts = Opts::new(
        cycle_writes,
        vec![
            Hero::new("Leader")
                .gm()
                .level(80)
                .with_items(&[(14_851, 1), (14_852, 1), (14_853, 1)]),
            Hero::new("Friend").level(80),
        ],
    );
    opts.seed.quests = vec![
        scripts_db::QuestRow {
            hero: "Leader",
            name: CONTROL_DEVICE,
            var: scripts_db::STATE_VAR,
            value: "Started",
        },
        scripts_db::QuestRow {
            hero: "Leader",
            name: CONTROL_DEVICE,
            var: "cond",
            value: "1",
        },
    ];
    opts.seed.globals = vec![("SieghardtOwned", "42")];
    let Some(mut w) = Bare::start(&opts) else {
        return;
    };
    let (leader, friend) = (w.me(0), w.me(1));
    let entered = w.enter_player(0);
    w.enter_player(1);

    // The login: the journal lists quest 307 at its condition 1, as Sieghardt left it.
    let list = all::<l2_protocol_game::server::QuestList>(&entered, "QUEST_LIST");
    assert!(
        list.iter().any(|l| l.quests.iter().any(|q| q.quest_id == 307 && q.status == 1)),
        "{list:?}"
    );

    // Droph knows the quest is going on: the sheets are all there, the page offers the summon.
    let droph = w.spawn_npc(0, DROPH, 30);
    let talked = w.talk(0, droph);
    let talked = w.follow(0, &talked, "Script");
    assert!(pages(&talked).last().unwrap().contains("Summon Hekaton Prime"), "{:?}", pages(&talked));
    let summoned = w.follow(0, &talked, "spawn");
    assert!(pages(&summoned).last().is_some());
    let hekaton = npcs_of(&w, HEKATON_PRIME);
    assert_eq!(hekaton.len(), 1, "Hekaton Prime is summoned");

    // The party kills it: the condition goes to 2 in the row Sieghardt wrote.
    w.input_of(
        0,
        ClientInput::JoinParty {
            name: "Friend".to_owned(),
            distribution_type_id: 0,
        },
    );
    w.input_of(1, ClientInput::AnswerJoinParty { response: 1 });
    assert!(w.world.is_in_party_with(leader, friend));
    w.world.do_die(hekaton[0], Some(leader));
    w.run_ms(3_000);
    assert_eq!(w.rows.quest_var(leader.0, CONTROL_DEVICE, "cond"), Some("2"));

    // Droph pays: the support items, and the quest rows go (`exitQuest(true)`), per-player rows
    // written whatever the switch says.
    let talked = w.talk(0, droph);
    w.follow(0, &talked, "Script");
    assert_eq!(w.items_of(0, SUPPORT_ITEMS), 1);
    assert_eq!(w.rows.quest_var(leader.0, CONTROL_DEVICE, "<state>"), None);

    // The global row is written when the server stops (`GlobalVariablesManager.storeMe`), and only
    // with the switch on; Kain knows the respawn time either way.
    let respawn = w.world.global_variables().get_long(RESPAWN_VARIABLE, 0);
    assert!(respawn > now_ms(), "{respawn}");
    w.world.handle(WorldCommand::Shutdown { done: Box::new(|| {}) });
    w.answer_reads();
    assert_eq!(w.rows.globals.get("SieghardtOwned").map(String::as_str), Some("42"));
    if cycle_writes {
        let row: i64 = w.rows.globals.get(RESPAWN_VARIABLE).unwrap().parse().unwrap();
        assert!(row > now_ms());
        assert_eq!(global_writes(&w).len(), 1, "{:?}", global_writes(&w));
    } else {
        assert!(!w.rows.globals.contains_key(RESPAWN_VARIABLE));
        assert!(global_writes(&w).is_empty(), "{:?}", global_writes(&w));
    }

    // With a throwaway MariaDB: the same statements on the real tables leave the rows the model
    // says (only under `L2_TEST_DATABASE_URL`).
    scripts_db::replay_in_database(
        "scripts_side",
        &opts.seed,
        &[("Leader", leader.0), ("Friend", friend.0)],
        &w.commands(),
        &w.rows,
    );

}

#[test]
fn side_by_side_with_the_switch_off() {
    side_by_side(false);
}

#[test]
fn side_by_side_with_the_switch_on() {
    side_by_side(true);
}
