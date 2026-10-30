//! The test harness of the script ports (unit tests only): a world with a few NPC and item
//! templates, html pages in a temporary datapack, fake players driven through world commands,
//! and the packets and `character_quests` writes they cause.
//!
//! A script's tests live at the end of its own file and use [`ScriptTest`], so adding a script
//! touches no shared test file (`docs/design/scripts.md`).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic, dead_code)]

use std::sync::{Arc, Mutex};

use l2_data::{HtmCache, ItemTemplates, NpcLoadOptions, NpcTemplates, SpawnData, SpawnLoadOptions};
use l2_net::{Decode, PacketReader};
use l2_protocol_game::ServerOpcode;
use l2_protocol_game::server::{NpcHtmlMessage, NpcQuestHtmlMessage};
use l2_util::ObjectId;
use l2_world::*;

/// `(player object id, packet bytes)` in the order the world sent them.
pub type Sent = Vec<(i32, Vec<u8>)>;

#[derive(Clone, Default)]
struct Log(Arc<Mutex<Sent>>);

struct Sink(i32, Log);

impl PacketSink for Sink {
    fn send(&self, bytes: Vec<u8>) {
        self.1.0.lock().unwrap().push((self.0, bytes));
    }

    fn left_world(&self, _reason: LeaveReason) {}
}

#[derive(Clone, Default)]
struct Writes(Arc<Mutex<Vec<QuestWrite>>>);

/// The grand boss writes and re-read requests (S-BOSS-01).
#[derive(Clone, Default)]
struct BossCommands(Arc<Mutex<Vec<PersistCommand>>>);

struct RecordingOutbox(Writes, BossCommands);

impl Outbox for RecordingOutbox {
    fn persist(&mut self, command: PersistCommand) {
        if let PersistCommand::Quest(write) = command {
            self.0.0.lock().unwrap().push(write);
        } else if matches!(
            command,
            PersistCommand::Boss(_) | PersistCommand::BossRereadRequest { .. }
        ) {
            self.1.0.lock().unwrap().push(command);
        }
    }

    fn ls_event(&mut self, _event: LsEvent) {}
}

/// Item names, stackability and the item templates (for `give_items`).
struct Data(StatData);

impl StaticData for Data {
    fn item(&self, item_id: i32) -> Option<ItemStatic> {
        let t = self.0.items.get(item_id)?;
        Some(ItemStatic {
            display_id: t.display_id,
            type2: if item_id == 57 { 4 } else { 5 },
            body_part: 0,
            quest_item: t.quest_item,
            weight: 0,
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
    fn access_level(&self, _level: i32) -> AccessLevelStatic {
        AccessLevelStatic::USER
    }
    fn stat_data(&self) -> Option<&StatData> {
        Some(&self.0)
    }
    fn item_name(&self, item_id: i32) -> Option<String> {
        self.0.items.get(item_id).map(|t| t.name.clone())
    }
    fn item_stackable(&self, item_id: i32) -> bool {
        self.0.items.get(item_id).is_some_and(|t| t.stackable)
    }
}

/// Where the test players stand; NPCs spawn a few steps away, inside the talk range.
pub const X: i32 = -71_338;
pub const Y: i32 = 258_271;
pub const Z: i32 = -3_104;

/// One world tick in milliseconds (`l2_world::world::TICK`).
const TICK_MS: u32 = 50;
/// Java `Attackable._onKillDelay` default, plus a tick.
const KILL_DELAY_MS: u32 = 2_550;

/// A world with the scripts installed.
pub struct ScriptTest {
    pub world: World,
    log: Log,
    writes: Writes,
    boss: BossCommands,
    /// The world's id factory (for objects a helper spawns besides NPCs).
    ids: Arc<IdFactory>,
    _root: tempfile::TempDir,
}

impl ScriptTest {
    /// `npcs` and `items` are `<npc>` and `<item>` elements in the datapack's XML format;
    /// `pages` are `(datapack path, text)` html files.
    pub fn new(scripts: ScriptRegistry, npcs: &str, items: &str, pages: &[(&str, &str)]) -> Self {
        Self::new_with(scripts, npcs, items, pages, |_, _| {})
    }

    /// [`ScriptTest::new`] with `prepare` run on the world (and the datapack folder) before the
    /// scripts are installed: zones, boss rows, settings the scripts read when they load.
    pub fn new_with(
        scripts: ScriptRegistry,
        npcs: &str,
        items: &str,
        pages: &[(&str, &str)],
        prepare: impl FnOnce(&mut World, &std::path::Path),
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
        for (path, text) in pages {
            write(path, text);
        }
        let log = Log::default();
        let writes = Writes::default();
        let boss = BossCommands::default();
        let mut world = World::new(
            WorldRng::from_seed(7),
            Box::new(RecordingOutbox(writes.clone(), boss.clone())),
        );
        let items = Arc::new(ItemTemplates::from_xml(&format!("<list>{items}</list>")).unwrap());
        world.set_static_data(Box::new(Data(StatData {
            items,
            ..StatData::default()
        })));
        let ids = Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap());
        world.set_id_factory(Arc::clone(&ids));
        let npcs = Arc::new(
            NpcTemplates::from_xml(&format!("<list>{npcs}</list>"), &NpcLoadOptions::default()).unwrap(),
        );
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        // The class groups of `CategoryData.xml` that the class change scripts read.
        world.set_category_data(Arc::new(l2_world::subclass::categories::fixture::class_groups()));
        prepare(&mut world, root.path());
        world.install_scripts(scripts);
        Self {
            world,
            log,
            writes,
            boss,
            ids,
            _root: root,
        }
    }

    /// A character of `level` with `items` enters the world.
    pub fn enter(&mut self, id: i32, level: i32, items: Vec<ItemData>) {
        self.enter_with(player_data(id, level, items));
    }

    pub fn enter_with(&mut self, data: PlayerData) {
        let id = data.object_id.0;
        self.world.handle(WorldCommand::EnterWorld {
            session: SessionId(u64::try_from(id).unwrap()),
            player: Box::new(data),
            sink: Box::new(Sink(id, self.log.clone())),
            ip: None,
        });
    }

    /// An NPC `dx` steps east of the players.
    pub fn spawn(&mut self, npc_id: i32, dx: i32) -> ObjectId {
        self.world.spawn_npc_at(npc_id, X + dx, Y, Z, 0).unwrap()
    }

    pub fn input(&mut self, player: i32, input: ClientInput) {
        self.world.handle(WorldCommand::Input {
            session: SessionId(u64::try_from(player).unwrap()),
            input,
        });
    }

    /// Clicks the NPC until the player talks to it: the first click targets, the next talks
    /// (one click when it is the target already, so the NPC talks once).
    pub fn talk(&mut self, player: i32, npc: ObjectId) {
        let clicks = if self.world.target_of(ObjectId(player)) == Some(npc) {
            1
        } else {
            2
        };
        for _ in 0..clicks {
            self.input(
                player,
                ClientInput::Action {
                    object_id: npc.0,
                    action_id: 0,
                },
            );
        }
    }

    pub fn bypass(&mut self, player: i32, command: &str) {
        self.input(
            player,
            ClientInput::Bypass {
                command: command.to_owned(),
            },
        );
    }

    pub fn ticks(&mut self, ticks: u32) {
        for _ in 0..ticks {
            self.world.tick();
        }
    }

    /// Lets `ms` milliseconds of world time pass (quest timers, despawns).
    pub fn wait_ms(&mut self, ms: u32) {
        self.ticks(ms.div_ceil(TICK_MS));
    }

    /// A character of `level` with Java's `race` (`Race.ordinal()`) and class
    /// (`PlayerClass.getId()`), which is also the base class, enters the world.
    pub fn enter_as(&mut self, id: i32, level: i32, race: i32, class_id: i32, items: Vec<ItemData>) {
        let mut data = player_data(id, level, items);
        data.race = race;
        data.class_id = class_id;
        data.base_class = class_id;
        self.enter_with(data);
    }

    /// `killer` kills `npc` (Java `doDie(killer)`), then the kill delay passes: `on_kill` runs
    /// `onKillDelay` (2.5 s) after the death, like Java.
    pub fn kill(&mut self, killer: i32, npc: ObjectId) {
        self.world.do_die(npc, Some(ObjectId(killer)));
        self.wait_ms(KILL_DELAY_MS);
    }

    /// `leader` invites `member` (both entered) into a party and `member` accepts.
    pub fn party(&mut self, leader: i32, member: i32) {
        let name = self.world.player(ObjectId(member)).unwrap().data.name.clone();
        self.input(
            leader,
            ClientInput::JoinParty {
                name,
                distribution_type_id: 0,
            },
        );
        self.input(member, ClientInput::AnswerJoinParty { response: 1 });
        assert!(
            self.world.is_in_party_with(ObjectId(leader), ObjectId(member)),
            "the party was not made"
        );
    }

    /// The NPCs of `npc_id` in the world (a script's `add_spawn`s), not deleted.
    pub fn npcs_of(&self, npc_id: i32) -> Vec<ObjectId> {
        self.world
            .npc_ids()
            .into_iter()
            .filter(|id| self.in_world(*id) && self.world.npc(*id).is_some_and(|n| n.npc_id == npc_id))
            .collect()
    }

    /// The NPC is in the world: spawned and not deleted (`deleteMe`) or decayed. A deleted NPC
    /// keeps its entry (and object id) for its next life.
    pub fn in_world(&self, npc: ObjectId) -> bool {
        self.world.npc(npc).is_some_and(|n| !n.decayed)
    }

    /// Everything sent since the last call.
    pub fn take(&self) -> Sent {
        std::mem::take(&mut self.log.0.lock().unwrap())
    }

    /// The `character_quests` writes since the last call.
    pub fn take_writes(&self) -> Vec<QuestWrite> {
        std::mem::take(&mut self.writes.0.lock().unwrap())
    }

    /// The grand boss writes and re-read requests since the last call.
    pub fn take_boss(&self) -> Vec<PersistCommand> {
        std::mem::take(&mut self.boss.0.lock().unwrap())
    }

    /// How many of `item_id` the player has (Java `getQuestItemsCount`).
    pub fn count(&self, player: i32, item_id: i32) -> i64 {
        self.world.player(ObjectId(player)).map_or(0, |p| {
            p.data
                .items
                .iter()
                .filter(|i| i.item_id == item_id && (i.loc == "INVENTORY" || i.loc == "PAPERDOLL"))
                .map(|i| i.count)
                .sum()
        })
    }
}

/// Stand-in html pages for a script's test: each page holds its file name (so a test can see
/// which one came) and the `bypass Script <name> <event>` links the real page has (the world
/// only takes a bypass that the last page offered, like Java's html action cache).
pub struct Pages {
    name: &'static str,
    dir: String,
    pages: Vec<(String, String)>,
}

impl Pages {
    /// The pages of `script` (its `name()` and `path()`).
    pub fn of(script: &dyn Script) -> Self {
        Self {
            name: script.name(),
            dir: format!("data/scripts/{}", script.path()),
            pages: Vec::new(),
        }
    }

    /// A page of the script folder with links to `events` (`bypass Script <name> <event>`).
    pub fn page(mut self, file: &str, events: &[&str]) -> Self {
        let links: String = events
            .iter()
            .map(|e| format!(r#"<a action="bypass Script {} {e}">{e}</a>"#, self.name))
            .collect();
        self.pages.push((
            format!("{}/{file}", self.dir),
            format!("<html><body>{file}{links}</body></html>"),
        ));
        self
    }

    /// Pages without links.
    pub fn plain(mut self, files: &[&str]) -> Self {
        for file in files {
            self = self.page(file, &[]);
        }
        self
    }

    /// The default page of each NPC (`data/html/default/<id>.htm`) with the quest link
    /// (`bypass Script`), and the "already completed" page.
    pub fn npcs(mut self, npc_ids: &[i32]) -> Self {
        for npc in npc_ids {
            self.pages.push((
                format!("data/html/default/{npc}.htm"),
                format!(r#"<html><body>npc {npc} <a action="bypass Script">Quest</a></body></html>"#),
            ));
        }
        self.pages.push((
            "data/html/alreadycompleted.htm".to_owned(),
            "<html><body>This quest has already been completed.</body></html>".to_owned(),
        ));
        self
    }

    /// Any other file of the datapack.
    pub fn file(mut self, path: &str, text: &str) -> Self {
        self.pages.push((path.to_owned(), text.to_owned()));
        self
    }
}

impl ScriptTest {
    /// [`ScriptTest::new`] with [`Pages`].
    pub fn with_pages(scripts: ScriptRegistry, npcs: &str, items: &str, pages: Pages) -> Self {
        let pages: Vec<(&str, &str)> = pages
            .pages
            .iter()
            .map(|(a, b)| (a.as_str(), b.as_str()))
            .collect();
        Self::new(scripts, npcs, items, &pages)
    }

    /// Talks to the NPC (two clicks), opens its quest link (`bypass Script`) and returns the
    /// last page it shows (empty when none came).
    pub fn quest_page(&mut self, player: i32, npc: ObjectId) -> String {
        self.talk(player, npc);
        self.bypass(player, "Script");
        pages_to(&self.take(), player).pop().unwrap_or_default()
    }

    /// Clicks a link of the script's last page (`bypass Script <name> <event>`) and returns
    /// the last page it shows (empty when none came).
    pub fn event(&mut self, player: i32, script_name: &str, event: &str) -> String {
        self.bypass(player, &format!("Script {script_name} {event}"));
        pages_to(&self.take(), player).pop().unwrap_or_default()
    }
}

/// Clans with residences, summons and skills (S-API-05): the branches of clan, castle and summon
/// scripts that the plain test world cannot reach.
impl ScriptTest {
    /// Player `leader` (entered) founds the clan `name`; it owns the castle `castle` and/or the
    /// fortress `fort` (residence ids, `None` for none; Java's clans never hold both). The
    /// residences are (re)loaded, so call it once per test. Returns the clan id; the packets and writes it causes are dropped.
    pub fn clan_with_residence(
        &mut self,
        leader: i32,
        name: &str,
        castle: Option<i32>,
        fort: Option<i32>,
    ) -> i32 {
        let clan = self.world.create_clan(ObjectId(leader), name).unwrap();
        // A fortress loads only with its flag pole (a static object of type 3 named after it).
        if let Some(id) = fort {
            let pole = format!(
                r#"<list><object id="{}" name="Fort{id}_flagpole" type="3" x="{X}" y="{Y}" z="{Z}" texture="" map_x="0" map_y="0" /></list>"#,
                23_000_000 + id
            );
            let poles = l2_data::StaticObjectTemplates::from_xml(&pole).unwrap();
            self.world.spawn_static_objects(&poles, &self.ids).unwrap();
        }
        self.world.load_residences(ResidenceLoad {
            castles: castle
                .map(|id| l2_world::residence::CastleRecord {
                    id,
                    found: true,
                    name: format!("Castle{id}"),
                    ..Default::default()
                })
                .into_iter()
                .collect(),
            forts: fort
                .map(|id| l2_world::residence::FortRecord {
                    id,
                    name: format!("Fort{id}"),
                    owner_id: clan,
                    ..Default::default()
                })
                .into_iter()
                .collect(),
            ..ResidenceLoad::default()
        });
        if let Some(id) = castle {
            self.world.castle_set_owner(CastleId(id), Some(clan));
        }
        self.take();
        self.take_writes();
        clan
    }

    /// A servitor of `npc_id` (a `Servitor` NPC template) for player `owner`, next to him (the
    /// `Summon` effect's `onStart`). The packets are dropped.
    pub fn summon(&mut self, owner: i32, npc_id: i32) -> ObjectId {
        let summon = self
            .world
            .summon_servitor(
                ObjectId(owner),
                l2_world::summon::spawn::ServitorParams {
                    npc_id,
                    exp_multiplier: 1.0,
                    consume_item: (0, 1),
                    life_time_ms: 3_600_000,
                    consume_interval_ms: 240_000,
                    reference_skill: 0,
                },
            )
            .unwrap();
        self.take();
        summon
    }

    /// Installs skill templates (`<skill>` elements of the datapack format) next to the item
    /// templates `items` (the same text as the test's `ITEMS`), for `cast_skill` and skill checks.
    pub fn set_skills(&mut self, items: &str, skills: &str) {
        let items = Arc::new(ItemTemplates::from_xml(&format!("<list>{items}</list>")).unwrap());
        let skills = Arc::new(l2_data::SkillTemplates::from_xml(
            &format!("<list>{skills}</list>"),
            &l2_data::skill::EnchantSkillGroups::default(),
            &l2_data::SkillLoadOptions::default(),
        ));
        self.world.set_static_data(Box::new(Data(StatData {
            items,
            skills,
            ..StatData::default()
        })));
    }
}

/// `character_quests` rows as the outbox gets them.
pub mod rows {
    use l2_world::QuestWrite;

    pub fn insert(name: &str, var: &str, value: &str) -> QuestWrite {
        QuestWrite::InsertVar {
            char_id: 1,
            name: name.to_owned(),
            var: var.to_owned(),
            value: value.to_owned(),
        }
    }

    pub fn update(name: &str, var: &str, value: &str) -> QuestWrite {
        QuestWrite::UpdateVar {
            char_id: 1,
            name: name.to_owned(),
            var: var.to_owned(),
            value: value.to_owned(),
        }
    }

    pub fn delete_var(name: &str, var: &str) -> QuestWrite {
        QuestWrite::DeleteVar {
            char_id: 1,
            name: name.to_owned(),
            var: var.to_owned(),
        }
    }

    /// `exitQuest(repeatable)`: `deleteQuestInDb`.
    pub fn delete(name: &str, repeatable: bool) -> QuestWrite {
        QuestWrite::DeleteQuest {
            char_id: 1,
            name: name.to_owned(),
            repeatable,
        }
    }

    /// The same rows of another character.
    pub fn of(char_id: i32, mut write: QuestWrite) -> QuestWrite {
        match &mut write {
            QuestWrite::InsertVar { char_id: c, .. }
            | QuestWrite::UpdateVar { char_id: c, .. }
            | QuestWrite::DeleteVar { char_id: c, .. }
            | QuestWrite::DeleteQuest { char_id: c, .. } => *c = char_id,
        }
        write
    }
}

/// The name of a server packet.
pub fn name_of(body: &[u8]) -> String {
    let ex = (body[0] == 0xFE).then(|| u16::from_le_bytes([body[1], body[2]]));
    ServerOpcode::from_id(body[0], ex).map_or_else(|| format!("?{:02X}", body[0]), |o| o.name().to_owned())
}

/// Decodes a server packet.
pub fn decode<P: Decode>(raw: &[u8]) -> P {
    let skip = if raw[0] == 0xFE { 3 } else { 1 };
    P::decode(&mut PacketReader::new(&raw[skip..])).unwrap()
}

/// The names of the packets sent to one player.
pub fn names_to(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, _)| *t == to)
        .map(|(_, b)| name_of(b))
        .collect()
}

/// The pages (NPC and quest windows) sent to one player, in order.
pub fn pages_to(out: &Sent, to: i32) -> Vec<String> {
    out.iter()
        .filter(|(t, _)| *t == to)
        .filter_map(|(_, b)| match name_of(b).as_str() {
            "NPC_HTML_MESSAGE" => Some(decode::<NpcHtmlMessage>(b).html),
            "EX_NPC_QUEST_HTML_MESSAGE" => Some(decode::<NpcQuestHtmlMessage>(b).html),
            _ => None,
        })
        .collect()
}

/// A plain item row.
pub fn item(object_id: i32, item_id: i32, count: i64) -> ItemData {
    ItemData {
        object_id: ObjectId(object_id),
        item_id,
        count,
        enchant_level: 0,
        loc: "INVENTORY".to_owned(),
        loc_data: 0,
        custom_type1: 0,
        custom_type2: 0,
        mana_left: -1,
        time: -1,
        ..Default::default()
    }
}

/// A test character at ([`X`], [`Y`], [`Z`]).
pub fn player_data(id: i32, level: i32, items: Vec<ItemData>) -> PlayerData {
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
        level,
        exp: 0,
        sp: 0,
        karma: 0,
        pvp_kills: 0,
        pk_kills: 0,
        access_level: 0,
        x: X,
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
        items,
        skills: vec![],
        shortcuts: vec![],
        profile: PlayerProfile::default(),
    }
}
