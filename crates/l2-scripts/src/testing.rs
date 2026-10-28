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

/// A world with the scripts installed.
pub struct ScriptTest {
    pub world: World,
    log: Log,
    writes: Writes,
    boss: BossCommands,
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
        world.set_id_factory(Arc::new(IdFactory::new(Vec::<i32>::new()).unwrap()));
        let npcs = Arc::new(
            NpcTemplates::from_xml(&format!("<list>{npcs}</list>"), &NpcLoadOptions::default()).unwrap(),
        );
        let spawns = Arc::new(SpawnData::load(root.path(), &npcs, &SpawnLoadOptions::default()));
        world.set_npc_data(npcs, spawns);
        world.set_html_cache(Arc::new(HtmCache::load(root.path(), false)));
        prepare(&mut world, root.path());
        world.install_scripts(scripts);
        Self {
            world,
            log,
            writes,
            boss,
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

    /// The first click on an object (target), then the second (talk).
    pub fn talk(&mut self, player: i32, npc: ObjectId) {
        for _ in 0..2 {
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
