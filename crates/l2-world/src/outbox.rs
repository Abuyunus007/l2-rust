//! How the world talks to the outside: per-player packet sinks and one outbox for the rest.
//!
//! Both are traits so the world runs synchronously in tests (recording implementations) and
//! the game server plugs in real connections and channels. Implementations must never block:
//! they hand the value to a queue and return.

use crate::command::LeaveReason;
use crate::player::{ItemData, PlayerData, SkillData};
use l2_util::ObjectId;

/// One player's connection.
pub trait PacketSink: Send {
    /// Queues an encoded game packet (plaintext; the connection encrypts it).
    fn send(&self, bytes: Vec<u8>);

    /// The player left the world. `Logout`/`Kicked`/`ServerShutdown` close the connection
    /// after the queued packets; `Restart` keeps it open for the character screen.
    fn left_world(&self, reason: LeaveReason);
}

/// Writes for the persistence task (D-15). Applied in order per character.
#[derive(Debug, Clone, PartialEq)]
pub enum PersistCommand {
    /// Store the character's current state (logout, restart, autosave, shutdown).
    StorePlayer(Box<PlayerData>),
    /// The player left. Sent *after* the final `StorePlayer`, so the persistence task can tell
    /// the login server `PlayerLogout` only once the data is safe.
    ///
    /// `logout` is false when the connection stays (a restart to the character screen): the
    /// character is marked offline, but the account is still on this server for the login server.
    PlayerLeft {
        account: String,
        object_id: ObjectId,
        logout: bool,
    },
    /// Java `Item.updateDatabase()`: item rows of one character that changed, written when they
    /// change (equip, unequip, destroy, drop) and not only at the next full store. `upserts` are
    /// the whole rows (an `UPDATE` when the row exists, else an `INSERT`, like Java's
    /// `_existsInDb`), `removed` are rows Java deletes (`removeFromDb`). Applied in order with
    /// the character's other commands.
    ItemChanges {
        owner: ObjectId,
        upserts: Vec<ItemData>,
        removed: Vec<ObjectId>,
    },
    /// Java `Item.updateDatabase()` for items that changed owner or container: the rows of a
    /// warehouse, freight or clan warehouse move (M6-WH-01). Unlike `ItemChanges` the rows may
    /// belong to somebody else than the character whose inventory they left (another character's
    /// freight, the clan's warehouse), so every op names its owner. `tracked` is the character
    /// whose last-written rows the persistence task keeps (its inventory and paperdoll items):
    /// after each op the task updates them, so a later full store neither deletes an item that
    /// went to a warehouse nor inserts one that came back. Applied in order with the character's
    /// other commands.
    ItemRows {
        tracked: Option<ObjectId>,
        ops: Vec<ItemRowOp>,
    },
    /// Java `ItemsOnGroundManager.run()` (only with `SaveDroppedItem`): `DELETE FROM
    /// itemsonground`, then one `INSERT` per row.
    SaveGroundItems(Vec<crate::ground_item::GroundItemRow>),
    /// Java `Player.storeSkill` / `removeSkill`: `character_skills` rows of one character that
    /// changed (a level up that teaches or takes skills), written when they change and not only
    /// at the next full store. `upserts` are an `UPDATE` of the level when the row exists, else a
    /// `REPLACE`, like Java; `removed` are skill ids whose row Java deletes. Applied in order with
    /// the character's other commands.
    SkillChanges {
        owner: ObjectId,
        upserts: Vec<SkillData>,
        removed: Vec<i32>,
    },
    /// Java `Player.storeRecommendationValues()` (a vote, `RecoGiveTask`):
    /// `REPLACE INTO character_reco_bonus (charId,rec_have,rec_left) VALUES (?,?,?)`. The
    /// `REPLACE` drops the old row, so `time_left` falls back to the column default 0 (a Java
    /// quirk the Rust side keeps). Applied in order with the character's other commands.
    RecoValues {
        owner: ObjectId,
        rec_have: i32,
        rec_left: i32,
    },
    /// Java `DailyResetManager.resetRecommends()`, the database half: the two `UPDATE`s of
    /// `character_reco_bonus` for every character (`rec_left` 20, `time_left` one hour,
    /// `rec_have` lowered by 20, not below 0). D-39: the Java game server runs the same
    /// statements on its own daily reset.
    ResetRecommends,
    /// Java `PlayerVariables.storeMe()`: the `character_variables` rows of one online character
    /// that changed, written now instead of with the next store (the daily reset does this for
    /// every player). `upserts` are `(var, value)` pairs, `removed` the names Java deletes.
    PlayerVariables {
        owner: ObjectId,
        upserts: Vec<(String, String)>,
        removed: Vec<String>,
    },
    /// Java `PunishmentTask.onStart`: `INSERT INTO punishments`. The persistence task keeps the
    /// id the database gave the row, to find it again at `EndPunishment`.
    StorePunishment(crate::punishments::StoredPunishment),
    /// Java `PunishmentTask.onEnd`: `UPDATE punishments SET expiration = <ended_at> WHERE id = ?`.
    /// `id` is the row id of a punishment loaded at startup, 0 for one this server stored (the
    /// persistence task finds the row by `key`, `affect` and `kind`).
    EndPunishment {
        id: i32,
        key: String,
        affect: crate::punishments::PunishmentAffect,
        kind: crate::punishments::PunishmentType,
        ended_at: i64,
    },
    /// Java `PunishmentManager.changeCharAccessLevel`: `UPDATE characters SET accessLevel=? WHERE
    /// charId=?`.
    CharAccessLevel { char_id: i32, level: i32 },
    /// Java `AdminRepairChar.handleRepair` (`//repair`, `//restore`): the character `name` is moved
    /// to Talking Island, its shortcuts are deleted and its items go back to the inventory.
    RepairChar { name: String },
    /// The summon tables (`character_summons`, `character_summon_skills_save`), see
    /// `summon/servitor.rs`. Applied in order with the character's other commands.
    Summon(crate::summon::servitor::SummonWrite),
    /// A clan row write with Java's statement (`clan_data`, `clan_privs`, `clan_subpledges`,
    /// `clan_notices`, the clan columns of `characters`), see `clan/`. Applied in order with the
    /// other commands.
    Clan(crate::clan::ClanWrite),
    /// Java `Product.save()`: `INSERT INTO buylists ... ON DUPLICATE KEY UPDATE` for one limited
    /// product whose stock changed, see `shop/`.
    BuyList(crate::shop::ProductRow),
    /// Java `Player.insertNewRecipeData` / `deleteRecipeData`: a `character_recipebook` row, see
    /// `craft/`.
    Recipe(crate::craft::RecipeWrite),
    /// Asks the persistence task to read the `pets` row and the `character_pet_skills_save` rows
    /// of a pet collar the world has no row for (a collar that came to the character after the
    /// login). The answer comes back as `WorldCommand::PetRowLoaded` (M6-PET-01, D-15: the world
    /// never waits for the database).
    PetRowRequest {
        owner: ObjectId,
        control_object_id: ObjectId,
    },
    /// A `character_hennas` row change with Java's statement (L-01, see `henna.rs`). Applied in
    /// order with the other commands.
    Henna(crate::henna::HennaWrite),
    /// A `character_tpbookmark` row change with Java's statement (L-04, see `bookmarks.rs`).
    Bookmark(crate::bookmarks::BookmarkWrite),
    /// A `character_contacts` row change with Java's statement (L-04, see `contacts.rs`).
    Contact(crate::contacts::ContactWrite),
    /// Asks the persistence task to read the `characters` rows (name, online, class, level) of
    /// friends who are not in the world for `FriendListExtended` (L-04). The answer comes back
    /// as `WorldCommand::FriendSummariesLoaded`.
    FriendSummariesRequest { requester: ObjectId, friends: Vec<i32> },
    /// A `character_friends` row change of the friend list or the block list (M6-FRIEND-01, see
    /// `friends.rs`). Applied in order with the other commands.
    Friend(crate::friends::FriendWrite),
    /// A write of the `messages` table with Java's statement, see `mail/`.
    Mail(crate::mail::MailWrite),
    /// Asks the persistence task to read the attachment rows of one mail (`items` with `owner_id`
    /// = the sender, `loc = 'MAIL'`, `loc_data` = the message id, Java `Mail.restore`) before
    /// `RequestPostAttachment` hands them out (M6-SYNC-01, D-39: the Java game server shares the
    /// rows). The answer comes back as `WorldCommand::MailAttachmentsLoaded`. Read in order with
    /// the other commands, so the answer includes every write sent before it.
    MailAttachmentsRequest {
        player: ObjectId,
        message_id: i32,
        sender_id: i32,
    },
    /// Asks the persistence task for the used crest ids (`SELECT crest_id FROM crests`) before a
    /// crest is created (M7-FU-03). The answer comes back as `WorldCommand::CrestIdsLoaded`.
    CrestIdsRequest { ticket: u32 },
    /// Asks the persistence task to read the `CLANWH` rows of a clan before a clan warehouse
    /// window opens (M6-SYNC-01). The answer comes back as `WorldCommand::ClanWarehouseLoaded`.
    ClanWarehouseRequest { player: ObjectId, clan_id: i32 },
    /// Java `FishingChampionshipManager.shutdown()`: `fishChampionshipEnd` (a `global_variables`
    /// row) and the `fishing_championship` rows (`DELETE`, then one `INSERT` per row), see
    /// `fishing/championship.rs`. Only sent with `FishingChampionshipDatabaseWrites` (D-49).
    FishingChampionship(Box<crate::fishing::ChampionshipData>),
    /// A residence row write with Java's statement (`castle`, `fort`, `clanhall`, the function
    /// tables, `hasCastle`), see `residence/` (M7-RES-01). Applied in order with the other commands.
    Residence(crate::residence::ResidenceWrite),
    /// A manor row write with Java's statement (`castle_manor_production`, `castle_manor_procure`),
    /// see `manor/` (M7-MANOR-01). Applied in order with the other commands.
    Manor(crate::manor::ManorWrite),
    /// A write of the cursed weapon tables with Java's statements (`cursed_weapons`, and the
    /// `items`/`characters` rows of an owner who is offline), see `cursed/` (M8-CURSED-01).
    Cursed(crate::cursed::CursedWrite),
    /// Asks the persistence task to read one residence again (D-50, D-51); the answer comes back
    /// as `WorldCommand::ResidenceReread` carrying `then` (M7-RES-01).
    ResidenceRereadRequest {
        residence_id: i32,
        kind: crate::residence::ResidenceReadKind,
        then: crate::residence::ResidenceReadThen,
    },
    /// Java `BotReportTable.saveReportedCharData`, one row at a time (M6-FU-09, see
    /// `bot_report.rs`): `INSERT INTO bot_reported_char_data VALUES (?,?,?)`, replacing the report
    /// date of a row that exists.
    BotReport(crate::bot_report::BotReportRow),
    /// Asks the persistence task to read the block list of a character that is not online
    /// (`character_friends` with `relation = 1`, Java `BlockList.loadList`) before
    /// `RequestSendPost` sends a mail to it (M6-FU-09). The answer comes back as
    /// `WorldCommand::BlockListLoaded`.
    BlockListRequest { sender: ObjectId, owner: i32 },
    /// Asks the persistence task to read the `PET`, `PET_EQUIP`, `INVENTORY` and `PAPERDOLL` rows
    /// of a character that is offline while its dead pet's corpse decays (`Pet.deleteMe`, M6-FU-09).
    /// The answer comes back as `WorldCommand::OfflinePetRowsLoaded`.
    OfflinePetRowsRequest {
        owner: ObjectId,
        control_object_id: ObjectId,
    },
    /// A write of the offline trade tables with Java's statements (`OfflineTraderTable`, M6-FU-14,
    /// see `trade/offline.rs`). Only sent with `OfflineTradeDatabaseWrites` (D-52).
    OfflineTrade(crate::trade::OfflineTradeWrite),
    /// A write of the `announcements` table with Java's statement (`//announces`), see
    /// `admin/announce.rs`. An `Insert` is answered with `WorldCommand::AnnouncementStored`.
    Announcement(crate::admin::announce::AnnouncementWrite),
    /// A `character_instance_time` row change with Java's statement (M8-INST-01, see
    /// `instance/times.rs`). Applied in order with the other commands.
    InstanceTime(crate::instance::InstanceTimeWrite),
    /// Java `RequestPetitionFeedback`: `INSERT INTO petition_feedback VALUES (?,?,?,?,?)`.
    PetitionFeedback {
        character: String,
        gm: String,
        rate: i32,
        message: String,
        /// `System.currentTimeMillis()`.
        time: i64,
    },
    /// A write of `mods_wedding` with Java's statement (`Couple`, M8-MISC-01, see `wedding.rs`).
    Wedding(crate::wedding::WeddingWrite),
}

/// One row write of [`PersistCommand::ItemRows`], with Java's statement (`Item.updateInDb`,
/// `insertIntoDb`, `removeFromDb`). The item carries `loc`, `loc_data`, count and the rest; the
/// owner column is `owner_id`.
#[derive(Debug, Clone, PartialEq)]
pub enum ItemRowOp {
    /// `Item.updateDatabase(true)` of an item that may not be in the table yet (it was created in
    /// this session and the character was not stored since): an `UPDATE` when the persistence task
    /// knows the row (the `tracked` character's last-written rows), else an `INSERT`.
    Write { owner_id: i32, item: ItemData },
    /// `updateInDb`: the row exists (it was read from a warehouse, freight or clan warehouse, or
    /// written by an earlier op).
    Update { owner_id: i32, item: ItemData },
    /// `insertIntoDb`: a new item (a new object id of the Rust range).
    Insert { owner_id: i32, item: ItemData },
    /// `removeFromDb`: `DELETE FROM items WHERE object_id = ?` (and the item's own rows).
    Delete { object_id: ObjectId },
}

/// Messages for the login server link.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LsEvent {
    /// Java `LoginServerThread.sendLogin` → `PlayerInGame`.
    PlayerInGame { account: String },
    /// Java `LoginServerThread.sendAccessLevel` → `ChangeAccessLevel` (a ban is -1, its end 0).
    AccessLevel { account: String, level: i32 },
    /// Java `LoginServerThread.sendLogout` → `PlayerLogout` without the character leaving: a
    /// player who stays in the world as an offline trader frees its account (M6-FU-14).
    PlayerLogout { account: String },
    /// Java `LoginServerThread.setServerStatus` (`AdminLogin`, `Shutdown`): the list status
    /// (one of the `STATUS_*` values) is sent as the `SERVER_LIST_STATUS` attribute and kept.
    SetServerStatus(i32),
    /// Java `LoginServerThread.setMaxPlayer`: the `MAX_PLAYERS` attribute, kept for the pages.
    SetMaxPlayer(i32),
    /// Java `LoginServerThread.sendServerStatus(id, value)`: one attribute of `ServerStatus`
    /// (`sendServerType`, the age).
    ServerStatus { id: i32, value: i32 },
}

/// A request of the world to the process around it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ControlRequest {
    /// The GM shutdown countdown is over (Java `Shutdown` calling `System.exit(0)` or
    /// `System.exit(2)`): stop the server, with exit code 2 for a restart.
    Shutdown { restart: bool },
    /// `//reload html [path]`: read the html files again. The answer comes back as
    /// `WorldCommand::HtmlReloaded` carrying the same `requester` and `path`.
    ReloadHtml {
        requester: l2_util::ObjectId,
        path: Option<String>,
    },
    /// `//loadhtml <path>`: read one html file of the datapack from the disk (Java
    /// `HtmCache.loadFile`). The answer comes back as `WorldCommand::HtmlFileLoaded`.
    LoadHtmlFile {
        requester: l2_util::ObjectId,
        path: String,
    },
    /// `//respawnall`, `//spawn_reload`: read the spawn files again (Java `SpawnData.load()`). The
    /// answer comes back as `WorldCommand::SpawnsReloaded`.
    ReloadSpawns,
    /// `SpawnData.addNewSpawn`: write the spawn of an admin command into the spawn file of its map
    /// tile (`data/spawns/Others/<x>_<y>.xml`, under the server's own folder).
    StoreSpawn { spawn: l2_data::spawn_store::NewSpawn },
}

/// Everything the world produces that is not a packet for a player.
pub trait Outbox: Send {
    fn persist(&mut self, command: PersistCommand);
    fn ls_event(&mut self, event: LsEvent);

    /// Asks the process to do something that is not the world's to do (stop the server). The
    /// default ignores it, which is what an outbox without a process (a test) does.
    fn control(&mut self, _request: ControlRequest) {}
}
