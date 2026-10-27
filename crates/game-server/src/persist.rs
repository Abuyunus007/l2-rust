//! The persistence task: stores player snapshots in order and tells the login server link when
//! a player's data is safe.
// Java: org/l2jmobius/gameserver/entity/actor/Player.java (store, storeCharBase)
// Java: org/l2jmobius/gameserver/taskmanagers/PlayerAutoSaveTaskManager.java (autosave interval)
// Java: org/l2jmobius/gameserver/Shutdown.java (everybody is stored before the DB is closed)
// Design: docs/design/world-loop.md ("Persistence")
//
// The world never waits for the database (D-15): it pushes `PersistCommand`s into the outbox
// channel and this task applies them one after the other. One task, one queue, so the writes of
// a character are applied in the order the world produced them, and `PlayerLeft` (which the
// world sends after the final `StorePlayer`) is only handled once that store finished. That is
// requirement 1: the login server hears `PlayerLogout` strictly after the data is written, so
// a quick re-login on the Java game server can never read stale rows.
//
// Side by side with the Java GS (D-39, requirement 2): the Java game server runs on the same
// database. This task only ever writes
// - `characters` through Java's own `UPDATE_CHARACTER` and `UPDATE_ONLINE_STATUS`,
// - `items`, `character_skills`, `character_shortcuts` of the character it stores,
// - `character_skills_save` and `character_item_reuse_save` (the buffs and reuse timers),
// - `character_reco_bonus` (the Nevit hourglass time) and the `character_variables` rows the
//   world changed (Nevit's points; M4-VIT-01),
// - `character_reco_bonus` again for a vote or `RecoGiveTask` (`storeRecommendationValues`) and
//   for Java's daily reset (`resetRecommends`: the two `UPDATE`s over every row, M5-FU-03; only
//   with `DailyResetDatabaseWrites = True` in `SideBySide.ini`),
// - the clan tables (`clan_data`, `clan_privs`, `clan_subpledges`, `clan_notices`) and the clan
//   columns of offline members' `characters` rows, with Java's clan statements
//   (`PersistCommand::Clan`, M6-CLAN-01),
// - `buylists` through Java's `Product.save` statement (`PersistCommand::BuyList`, M6-SHOP-01),
// - `character_friends` rows of the friend list and the block list (`PersistCommand::Friend`,
//   M6-FRIEND-01),
// - `character_recipebook` (a recipe learned or forgotten, `PersistCommand::Recipe`) and
//   `character_recipeshoplist` (with the store, `StoreRecipeShopList`; M6-CRAFT-01),
// - the residence tables (`castle`, `fort`, `clanhall`, `siegable_clanhall`, the function tables,
//   `clan_data.hasCastle`) and the circlet rows of offline clan members, with Java's residence
//   statements (`PersistCommand::Residence`, M7-RES-01; the world sends the siege-like ones only
//   with `SiegeDatabaseWrites = True` in `SideBySide.ini`, D-51),
// with the statements the repositories copied from Java. It never runs bulk updates or cleanup
// on its own; the daily reset `UPDATE`s are the one bulk write, because the Java game server
// runs the same ones every day.
//
// Item rows reach the database two ways, like in Java: `PersistCommand::ItemChanges` carries the
// rows of one `Item.updateDatabase()` (an equip, an unequip, a destroyed item) and is written at
// once; the full store (`StorePlayer`) writes what differs from the last write, which covers
// everything Java only writes later (a lower count, new items). The baseline of a character
// follows both, so neither writes a row twice or inserts a row that exists.
// `UPDATE_CHARACTER` writes every column, including ones M2 does not model (nobless...). The clan
// columns come from the snapshot when it carries them (`PlayerProfile::clan`, M6-CLAN-01).
// (`vitality_points` is modelled since M4-VIT-01 and comes from the snapshot.) Those are read from the row first and written back unchanged, so a store never
// changes what the Java side owns. The account may be on one game server at a time (the login
// server enforces it), so nobody else changes the row between that read and the write.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use l2_config::IniFile;
use l2_config::game::GeneralConfig;
use l2_db::CrestRepository;
use l2_db::FortSiegeRepository;
use l2_db::HallSiegeRepository;
use l2_db::InstanceTimeRepository;
use l2_db::ItemAuctionRepository;
use l2_db::OfflineTradeRepository;
use l2_db::SiegeRepository;
use l2_db::TerritoryRepository;
use l2_db::UndergroundColiseumRepository;
use l2_db::{AirshipRepository, CursedWeaponRepository, CursedWeaponRow};
use l2_db::{AnnouncementRepository, AutoColumns, HennaRepository, PetitionFeedbackRepository};
use l2_db::{AuctionRepository, FunctionTable, ResidenceRepository};
use l2_db::{BbsFavoriteRepository, BookmarkRepository, BookmarkRow, ContactRepository};
use l2_db::{BossRepository, GrandBossListRow, GrandBossStore, GrandBossUpdate, RaidBossRow};
use l2_db::{
    BotReportRepository, BuyListRepository, CharacterRepository, CharacterRow, CharacterUpdate,
    ClanRepository, ClanUpdate, FriendRepository, GroundItemRecord, GroundItemRepository, ItemLocation,
    ItemRepository, ItemReuseRepository, ItemUpdate, MacroRepository, MessageRepository, NewItem,
    NewItemReuse, NewMacro, NewPetSkillSave, NewPunishment, NewShortcut, NewSkillSave, NewSummonSkillSave,
    PetRepository, PetValues, Pool, PunishmentRepository, RecipeRepository, RecipeShopRow,
    RecommendationRepository, ShortcutRepository, SkillLevel, SkillRepository, SubclassRepository,
    SubclassValues, SummonRepository, VariableRepository,
};
use l2_db::{FishingChampionshipRepository, FishingChampionshipRow};
use l2_db::{ManorRepository, ProcureRow, ProductionRow};
use l2_world::InstanceTimeWrite;
use l2_world::clan::{
    ClanDataRecord, ClanMemberRecord, ClanRow, ClanUpdateRow, CrestRecord, CrestWrite, WarWrite,
};
use l2_world::fort::{
    FortSiegeFortLoad, FortSiegeGuardRecord, FortSiegeLoad, FortSiegeRows, FortSiegeWrite, FortSpawnRecord,
};
use l2_world::residence::{
    CastleRecord, ClanHallRecord, FortRecord, FunctionRecord, ResidenceLoad, ResidenceReadKind, ResidenceRef,
    ResidenceRefresh, ResidenceRereadRows, ResidenceWrite, SiegableHallRecord,
};
use l2_world::territory::{
    TerritoryLoad, TerritoryRecord, TerritoryRegistrationRecord, TerritoryRows, TerritorySpawnRecord,
    TerritoryWrite,
};
use l2_world::{AirshipRow, CursedLoad, CursedRow, CursedWrite, VehicleLoad, VehicleWrite};
use l2_world::{AnnouncementRecord, AnnouncementWrite};
use l2_world::{AuctionBidRecord, AuctionRecord, AuctionWrite};
use l2_world::{BookmarkWrite, ContactWrite, FriendSummary};
use l2_world::{
    BossLoad, BossReadKind, BossRereadRows, BossWrite, GrandBossData, GrandBossStoreRow, RaidBossData,
    RaidPointsRecord,
};
use l2_world::{
    BotReportRow, ChampionshipData, ChampionshipRow, ClanRecord, ClanWrite, FriendWrite, GroundItemRow,
    HennaWrite, ItemData, ItemRowOp, LsEvent, MacroData, MailCharacter, MailLoad, MailMessage, MailRefresh,
    MailWrite, ManorLoad, ManorProcureRow, ManorProductionRow, ManorWrite, ObjectId, OfflinePetRows,
    PersistCommand, PetRowData, PlayerData, ProductRow, PunishmentAffect, PunishmentRecord, PunishmentType,
    RecipeWrite, SavedEffect, SavedItemReuse, SavedPet, SavedPetEffect, ShortcutData, SkillData,
    StoredPunishment, SummonWrite,
};
use l2_world::{FavoriteAnswer, FavoriteEntry, FavoriteRead, FavoriteWrite};
use l2_world::{
    FlagWarRecord, HallGuardRecord, HallScript, HallSiegeHallLoad, HallSiegeLoad, HallSiegeRows,
    HallSiegeWrite,
};
use l2_world::{ItemAuctionRecord, ItemAuctionWrite};
use l2_world::{MinigameLoad, MinigameWrite};
use l2_world::{
    NobleRow, NobleStats, OlympiadData, OlympiadLoad, OlympiadRereadRequest, OlympiadRereadRows,
    OlympiadWrite,
};
use l2_world::{OfflineSettings, OfflineTradeLine, OfflineTradeRow, OfflineTradeStatus, OfflineTradeWrite};
use tokio::sync::mpsc::{UnboundedReceiver, UnboundedSender};
use tokio::sync::oneshot;
use tokio::task;
use tracing::{debug, error, info, warn};

use crate::runtime::OutboxMessage;

/// What the login server link has to send, in the order the world produced it.
///
/// The persistence task is the only consumer of the world's outbox, so it also forwards the
/// login server events. Keeping both in one queue preserves their order (a player who enters
/// and leaves quickly is announced before it is logged out).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LsNotice {
    /// Java `LoginServerThread.sendLogin` -> `PlayerInGame`.
    PlayerInGame { account: String },
    /// Java `LoginServerThread.sendLogout` -> `PlayerLogout`. Only sent after the character's
    /// final store finished (or failed for good, which is logged).
    PlayerLogout { account: String },
    /// Java `LoginServerThread.sendAccessLevel` -> `ChangeAccessLevel` (a ban is -1, its end 0).
    AccessLevel { account: String, level: i32 },
    /// Java `LoginServerThread.setServerStatus`: the list status.
    SetServerStatus(i32),
    /// Java `LoginServerThread.setMaxPlayer`.
    SetMaxPlayer(i32),
    /// Java `LoginServerThread.sendServerStatus(id, value)`.
    ServerStatus { id: i32, value: i32 },
}

/// Why a store failed.
#[derive(Debug, thiserror::Error)]
pub enum PersistError {
    #[error("database error: {0}")]
    Db(#[from] l2_db::DbError),
    /// There is no row to update. The Rust game server never creates characters here.
    #[error("character {0} does not exist in the database")]
    NoSuchCharacter(i32),
    /// The row belongs to another account than the snapshot says: something is badly wrong, so
    /// nothing is written.
    #[error("character {0} belongs to another account than {1}")]
    WrongAccount(i32, String),
}

/// Where snapshots go. The real one is [`DbStore`]; tests record the calls.
pub trait PlayerStore: Send + 'static {
    /// Writes the character's current state.
    fn store_player(&mut self, player: &PlayerData) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// The character left the world: marks it offline (Java `setOnlineStatus(false, true)`).
    fn player_offline(
        &mut self,
        object_id: ObjectId,
    ) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// Item rows that changed (Java `Item.updateDatabase()`): written at once, not with the next
    /// full store. `upserts` become an `UPDATE` when the row is known, else an `INSERT`.
    fn store_item_changes(
        &mut self,
        owner: ObjectId,
        upserts: &[ItemData],
        removed: &[ObjectId],
    ) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// Java `Item.updateDatabase()` for items that changed owner or container (a warehouse, the
    /// freight, the clan warehouse; M6-WH-01): one statement per op, in order. `tracked` is the
    /// character whose inventory rows the store keeps a baseline of.
    fn store_item_rows(
        &mut self,
        _tracked: Option<ObjectId>,
        _ops: &[ItemRowOp],
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// Java `ItemsOnGroundManager.run()`: the table `itemsonground` is emptied and `rows` are
    /// inserted. Only sent with `SaveDroppedItem` on.
    fn store_ground_items(
        &mut self,
        rows: &[GroundItemRow],
    ) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// `character_skills` rows that changed (Java `Player.storeSkill` / `removeSkill`): written at
    /// once, not with the next full store. An upsert becomes an `UPDATE` of the level when the row
    /// is known, else a `REPLACE`; `removed` skill ids are deleted.
    fn store_skill_changes(
        &mut self,
        owner: ObjectId,
        upserts: &[SkillData],
        removed: &[i32],
    ) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// Java `Player.storeRecommendationValues`: `REPLACE INTO character_reco_bonus
    /// (charId,rec_have,rec_left)`; `time_left` falls back to the column default (0).
    fn store_reco_values(
        &mut self,
        owner: ObjectId,
        rec_have: i32,
        rec_left: i32,
    ) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// Java `DailyResetManager.resetRecommends`, the database half: the two `UPDATE`s of every
    /// `character_reco_bonus` row.
    fn reset_recommends(&mut self) -> impl Future<Output = Result<(), PersistError>> + Send;

    /// Java `PlayerVariables.storeMe`: the `character_variables` rows one character changed
    /// (deletes first, then upserts).
    fn store_player_variables(
        &mut self,
        owner: ObjectId,
        upserts: &[(String, String)],
        removed: &[String],
    ) -> impl Future<Output = Result<(), PersistError>> + Send;
    /// Java `PunishmentTask.onStart`: `INSERT INTO punishments`. The store keeps the id the
    /// database gave the row to find it again in [`end_punishment`](Self::end_punishment).
    fn store_punishment(
        &mut self,
        _punishment: &StoredPunishment,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// Java `PunishmentTask.onEnd`: the row's `expiration` becomes `ended_at`. `id` is 0 for a
    /// punishment this store inserted itself (found again by key, affect and type).
    fn end_punishment(
        &mut self,
        _id: i32,
        _key: &str,
        _affect: PunishmentAffect,
        _kind: PunishmentType,
        _ended_at: i64,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// Java `PunishmentManager.changeCharAccessLevel`.
    fn change_char_access_level(
        &mut self,
        _char_id: i32,
        _level: i32,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// Java `AdminRepairChar.handleRepair`.
    fn repair_char(&mut self, _name: &str) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// Java `Product.save`: the `buylists` row of a limited product whose stock changed.
    fn store_buy_list_product(
        &mut self,
        _row: &ProductRow,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// The servitor tables (`CharSummonTable`, `Servitor.storeEffect` / `restoreEffects`).
    fn store_summon(
        &mut self,
        _write: &SummonWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A write of the `messages` table with Java's statement (`MailManager`).
    fn store_mail_write(
        &mut self,
        _write: &MailWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A clan row write with Java's statement (`Clan.store`, `updateClanInDB`, ...).
    fn store_clan_write(
        &mut self,
        _write: &ClanWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A `character_recipebook` row (`Player.insertNewRecipeData`, `deleteRecipeData`).
    fn store_recipe_write(
        &mut self,
        _write: &RecipeWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// Java `FishingChampionshipManager.shutdown()`: `fishChampionshipEnd` and the rows of
    /// `fishing_championship` (M6-FU-08).
    fn store_fishing_championship(
        &mut self,
        _data: &ChampionshipData,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A residence row write with Java's statement (`Castle`, `Fort`, `ClanHall`, M7-RES-01).
    fn store_residence_write(
        &mut self,
        _write: &ResidenceWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A manor row write with Java's statement (`castle_manor_production`, `castle_manor_procure`,
    /// M7-MANOR-01).
    fn store_manor_write(
        &mut self,
        _write: &ManorWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// An `airships` write with Java's statement (M8-VEHICLE-01).
    fn store_vehicle_write(
        &mut self,
        _write: &VehicleWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A cursed weapon write with Java's statement (`cursed_weapons`, and the `items` and
    /// `characters` rows of an owner who is offline, M8-CURSED-01).
    fn store_cursed_write(
        &mut self,
        _write: &CursedWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// An Olympiad write with Java's statements (M8-OLY-01).
    fn store_olympiad_write(
        &mut self,
        _write: &OlympiadWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// An Olympiad read (the rankings, the last points, the unclaimed passes; M8-OLY-01). A store
    /// without a database has no rows.
    fn load_olympiad_reread(
        &mut self,
        request: &OlympiadRereadRequest,
    ) -> impl Future<Output = Result<OlympiadRereadRows, PersistError>> + Send {
        let rows = match request {
            OlympiadRereadRequest::ClassLeaderBoard { .. } => OlympiadRereadRows::Names(Vec::new()),
            OlympiadRereadRequest::LastPoints { .. } => OlympiadRereadRows::Points(0),
            OlympiadRereadRequest::UnclaimedPasses { .. } => OlympiadRereadRows::Passes(None),
            OlympiadRereadRequest::Heroes { .. } => OlympiadRereadRows::Heroes(Box::default()),
        };
        async move { Ok(rows) }
    }

    /// One residence read again (D-50, D-51, M7-RES-01). A store without a database has no rows.
    fn load_residence(
        &mut self,
        _kind: ResidenceReadKind,
        _residence_id: i32,
    ) -> impl Future<Output = Result<ResidenceRereadRows, PersistError>> + Send {
        async { Ok(ResidenceRereadRows::Missing) }
    }

    /// Java `Pet.restore` and `Pet.restoreEffects` for one collar: its `pets` row and its saved
    /// buffs (M6-PET-01). A store without a database has no rows.
    fn load_pet_row(
        &mut self,
        _control: ObjectId,
    ) -> impl Future<Output = Result<PetRowData, PersistError>> + Send {
        async {
            Ok(PetRowData {
                row: None,
                effects: Vec::new(),
            })
        }
    }

    /// Java `Mail.restore` for one message (M6-SYNC-01): the items still in the mail. The Java game
    /// server shares the rows (D-39), so the world asks before it hands attachments out.
    fn load_mail_attachments(
        &mut self,
        _sender_id: i32,
        _message_id: i32,
    ) -> impl Future<Output = Result<Vec<ItemData>, PersistError>> + Send {
        async { Ok(Vec::new()) }
    }

    /// Java `ClanWarehouse.restore` (M6-SYNC-01): the `CLANWH` items of a clan, read when a clan
    /// warehouse window opens.
    fn load_clan_warehouse(
        &mut self,
        _clan_id: i32,
    ) -> impl Future<Output = Result<Vec<ItemData>, PersistError>> + Send {
        async { Ok(Vec::new()) }
    }

    /// Java `PetInventory.restore` and `PlayerInventory.restore` of a character that is offline
    /// while its dead pet decays (M6-FU-09): the `PET`, `PET_EQUIP`, `INVENTORY` and `PAPERDOLL`
    /// rows. Read in order with the other commands, so the answer includes every write sent
    /// before it.
    fn load_offline_pet_rows(
        &mut self,
        _owner: ObjectId,
    ) -> impl Future<Output = Result<OfflinePetRows, PersistError>> + Send {
        async { Ok(OfflinePetRows::default()) }
    }

    /// Java `BlockList.loadList` for a character that is not online (M6-FU-09): the ids of its
    /// block list. The Java game server shares the rows (D-39), so the world asks before a mail
    /// goes to an offline receiver.
    fn load_block_list(
        &mut self,
        _owner: i32,
    ) -> impl Future<Output = Result<Vec<i32>, PersistError>> + Send {
        async { Ok(Vec::new()) }
    }

    /// A write of the offline trade tables (Java `OfflineTraderTable`, M6-FU-14).
    fn store_offline_trade(
        &mut self,
        _write: &OfflineTradeWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// The ids of every `crests` row (M7-FU-03): a new crest takes the lowest free one.
    fn load_crest_ids(&mut self) -> impl Future<Output = Result<Vec<i32>, PersistError>> + Send {
        async { Ok(Vec::new()) }
    }

    /// One row of `bot_reported_char_data` (Java `BotReportTable.saveReportedCharData`, M6-FU-09).
    fn store_bot_report(
        &mut self,
        _row: &BotReportRow,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A `character_friends` row change (Java's friend and block list statements, M6-FRIEND-01).
    fn store_friend_write(
        &mut self,
        _write: &FriendWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A write of the `announcements` table (`//announces`, M7-ADMIN-10). An insert returns the
    /// id the database gave the row; the other writes return 0.
    fn store_announcement(
        &mut self,
        _write: &AnnouncementWrite,
    ) -> impl Future<Output = Result<i32, PersistError>> + Send {
        async { Ok(0) }
    }

    /// A boss row write with Java's statement (`GrandBossManager`, `RaidBossSpawnManager`,
    /// `RaidBossPointsManager`, M8-BOSS-01).
    fn store_boss_write(
        &mut self,
        _write: &BossWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A `character_tpbookmark` row change (L-04): Java `Player.teleportBookmarkAdd` / `Modify` /
    /// `Delete`.
    fn store_bookmark_write(
        &mut self,
        _write: &BookmarkWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// Boss rows read again (D-50, D-53, M8-BOSS-01). A store without a database has no rows.
    fn load_boss_rows(
        &mut self,
        kind: BossReadKind,
    ) -> impl Future<Output = Result<BossRereadRows, PersistError>> + Send {
        async move {
            Ok(match kind {
                BossReadKind::GrandBoss(boss_id) => BossRereadRows::GrandBoss { boss_id, row: None },
                BossReadKind::RaidPoints(char_id) => BossRereadRows::RaidPoints {
                    char_id,
                    rows: Vec::new(),
                },
            })
        }
    }

    /// A `character_contacts` row change (L-04): Java `ContactList.add` / `remove`.
    fn store_contact_write(
        &mut self,
        _write: &ContactWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// The `characters` row of a friend who is not in the world (Java `FriendListExtended`):
    /// `None` when no row has the id. An `Err` leaves the friend out, like Java's empty `catch`.
    fn load_friend_summary(
        &mut self,
        _char_id: i32,
    ) -> impl Future<Output = Result<Option<FriendSummary>, PersistError>> + Send {
        async { Ok(None) }
    }

    /// A write of the subclass tables (S-SUB-01): Java `Player.addSubClass` / `modifySubClass`.
    fn store_subclass_write(
        &mut self,
        _write: &l2_world::SubclassWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// The rows of one class index for `setActiveClass` (S-SUB-01): every part Java reads, a part
    /// whose read fails is logged and left empty (Java's catch blocks). The saved buffs are
    /// deleted after the read (`restoreEffects`).
    fn load_class_index(
        &mut self,
        _owner: ObjectId,
        _class_index: i32,
    ) -> impl Future<Output = l2_world::ClassIndexLoad> + Send {
        async { l2_world::ClassIndexLoad::default() }
    }

    /// A `character_hennas` row change (L-01): Java `Player.addHenna` / `removeHenna`.
    fn store_henna_write(
        &mut self,
        _write: &HennaWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A write of `seven_signs` / `seven_signs_status` (Java's `SevenSigns` statements, M8-SS-01).
    fn store_seven_signs(
        &mut self,
        _write: &l2_world::sevensigns::SevenSignsWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A row of the Underground Coliseum (`underground_coliseum`, the `UC_*_TIME` rows of
    /// `global_variables`, M8-GAMES-01).
    fn store_minigame_write(
        &mut self,
        _write: &MinigameWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A `character_instance_time` row change (M8-INST-01): Java `InstanceManager.setInstanceTime`
    /// / `deleteInstanceTime`.
    fn store_instance_time(
        &mut self,
        _write: &InstanceTimeWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// Every `seven_signs` row and the status row read again (D-50, M8-SS-01). A store without a
    /// database has none.
    fn load_seven_signs(
        &mut self,
    ) -> impl Future<Output = Result<l2_world::sevensigns::SevenSignsLoad, PersistError>> + Send {
        async { Ok(l2_world::sevensigns::SevenSignsLoad::default()) }
    }

    /// A write of `mods_wedding` (Java's `Couple` statements, M8-MISC-01).
    fn store_wedding(
        &mut self,
        _write: &l2_world::wedding::WeddingWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A row write of the item auctions (Java `ItemAuction.storeMe`, `updatePlayerBidInternal`,
    /// `ItemAuctionManager.deleteAuction`, L-05).
    fn store_item_auction_write(
        &mut self,
        _write: &ItemAuctionWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// The `item_auction` rows and bids of one instance (Java `ItemAuctionInstance.loadAuction`),
    /// read again before an auction is shown or bid on while the Java game server writes the
    /// states (L-05, D-53). Read in order with the other commands, so the rows include every bid
    /// written before the request.
    fn load_item_auction_records(
        &mut self,
        _instance_id: i32,
    ) -> impl Future<Output = Result<Vec<ItemAuctionRecord>, PersistError>> + Send {
        async { Ok(Vec::new()) }
    }

    /// Java `RequestPetitionFeedback`: one `petition_feedback` row (M7-ADMIN-10).
    fn store_petition_feedback(
        &mut self,
        _character: &str,
        _gm: &str,
        _rate: i32,
        _message: &str,
        _time: i64,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A `bbs_favorites` write of the community board (S-HAND-01).
    fn store_bbs_favorite(
        &mut self,
        _write: &FavoriteWrite,
    ) -> impl Future<Output = Result<(), PersistError>> + Send {
        async { Ok(()) }
    }

    /// A `bbs_favorites` read of the community board (S-HAND-01). A store without a database has no
    /// favorites.
    fn load_bbs_favorites(
        &mut self,
        _player: ObjectId,
        kind: FavoriteRead,
    ) -> impl Future<Output = Result<FavoriteAnswer, PersistError>> + Send {
        async move {
            Ok(match kind {
                FavoriteRead::List => FavoriteAnswer::List(Vec::new()),
                FavoriteRead::Count => FavoriteAnswer::Count(0),
            })
        }
    }
}

/// How often a failed store is tried again before it is given up (and logged as lost).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Total tries, at least 1.
    pub attempts: u32,
    pub delay: Duration,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempts: 2,
            delay: Duration::from_millis(500),
        }
    }
}

/// Who waits for a character's last store. The session of a client that goes back to the
/// character screen (`Restart`) waits here before it reads the character list, because Java
/// stores synchronously before it answers `RestartResponse` (found by M2-E2E-01).
///
/// Network-side bookkeeping, not game state (D-10): the lock is held only to add or take a
/// waiter.
#[derive(Clone, Default)]
pub struct LeftAcks {
    waiters: Arc<Mutex<HashMap<ObjectId, Vec<oneshot::Sender<()>>>>>,
    /// The way back to the world, for the answers of reads the world asked for (the pet rows of
    /// a collar, M6-PET-01). Set once the world thread runs.
    world: Arc<std::sync::OnceLock<crate::state::WorldHandle>>,
    /// Who carries out the requests of the world that are not the world's to do: the GM
    /// shutdown and the html reload (M7-ADMIN-10). Set once the server runs.
    control: Arc<std::sync::OnceLock<ControlHandler>>,
}

/// Carries out a [`l2_world::ControlRequest`].
pub type ControlHandler = Arc<dyn Fn(l2_world::ControlRequest) + Send + Sync>;

impl LeftAcks {
    /// Installs the handler of the world's control requests (once; later calls are ignored).
    pub fn set_control(&self, handler: ControlHandler) {
        let _ = self.control.set(handler);
    }

    /// Hands a control request on. Without a handler (a test) it is logged and dropped.
    pub fn control(&self, request: l2_world::ControlRequest) {
        match self.control.get() {
            Some(handler) => handler(request),
            None => debug!("No control handler: {request:?} ignored."),
        }
    }

    /// Installs the world's handle (once; later calls are ignored).
    pub fn set_world(&self, handle: crate::state::WorldHandle) {
        let _ = self.world.set(handle);
    }

    /// Sends an answer to the world. `false` when there is no world (yet) or it is gone.
    pub fn send_to_world(&self, command: l2_world::WorldCommand) -> bool {
        self.world.get().is_some_and(|world| world.send(command))
    }

    /// A receiver that completes once `PlayerLeft` of `id` was handled: the final store is done
    /// (or failed for good, which is logged) and the character is marked offline.
    pub fn wait_for(&self, id: ObjectId) -> oneshot::Receiver<()> {
        let (tx, rx) = oneshot::channel();
        self.waiters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .entry(id)
            .or_default()
            .push(tx);
        rx
    }

    /// Forgets the waiters of `id` (the wait was not needed after all).
    pub fn cancel_wait(&self, id: ObjectId) {
        self.waiters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id);
    }

    /// Wakes everyone waiting for `id`. The persistence task calls it after `PlayerLeft`.
    pub fn ack(&self, id: ObjectId) {
        let waiters = self
            .waiters
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id);
        for tx in waiters.into_iter().flatten() {
            let _ = tx.send(());
        }
    }
}

/// Starts the persistence task on the receiving end of the world's outbox.
///
/// The task ends when the channel is closed (the world thread is gone), after handling
/// everything that was queued: that is the shutdown drain (D-15).
pub fn spawn<S: PlayerStore>(
    store: S,
    messages: UnboundedReceiver<OutboxMessage>,
    notices: UnboundedSender<LsNotice>,
    retry: RetryPolicy,
    acks: LeftAcks,
) -> task::JoinHandle<()> {
    task::spawn(run(store, messages, notices, retry, acks))
}

async fn run<S: PlayerStore>(
    mut store: S,
    mut messages: UnboundedReceiver<OutboxMessage>,
    notices: UnboundedSender<LsNotice>,
    retry: RetryPolicy,
    acks: LeftAcks,
) {
    // Characters whose last store failed for good.
    let mut unsaved: HashSet<ObjectId> = HashSet::new();
    let attempts = retry.attempts.max(1);
    while let Some(message) = messages.recv().await {
        match message {
            OutboxMessage::Persist(PersistCommand::StorePlayer(player)) => {
                let mut result = store.store_player(&player).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store {} (try {} of {attempts}): {e}",
                        player.name,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_player(&player).await;
                }
                match result {
                    Ok(()) => {
                        unsaved.remove(&player.object_id);
                        debug!("Stored {}.", player.name);
                    }
                    Err(e) => {
                        error!("The state of {} could NOT be stored: {e}", player.name);
                        unsaved.insert(player.object_id);
                    }
                }
            }
            OutboxMessage::Persist(PersistCommand::PlayerLeft {
                account,
                object_id,
                logout,
            }) => {
                if unsaved.remove(&object_id) {
                    // Java logs a failed store and carries on with the logout, so does this.
                    error!("Character {} left with data that is NOT stored.", object_id.0);
                }
                if let Err(e) = store.player_offline(object_id).await {
                    error!("Could not mark character {} offline: {e}", object_id.0);
                }
                // A restart keeps the connection: the account is still on this server.
                if logout {
                    let _ = notices.send(LsNotice::PlayerLogout { account });
                }
                acks.ack(object_id);
            }
            OutboxMessage::Persist(PersistCommand::ItemChanges {
                owner,
                upserts,
                removed,
            }) => {
                let mut result = store.store_item_changes(owner, &upserts, &removed).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store item changes of {} (try {} of {attempts}): {e}",
                        owner.0,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_item_changes(owner, &upserts, &removed).await;
                }
                // A lost item write is caught by the next full store, which compares the whole
                // item list against what was last written.
                if let Err(e) = result {
                    error!("Item changes of {} could NOT be stored: {e}", owner.0);
                }
            }
            OutboxMessage::Persist(PersistCommand::ItemRows { tracked, ops }) => {
                let mut result = store.store_item_rows(tracked, &ops).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store item rows (try {} of {attempts}): {e}",
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_item_rows(tracked, &ops).await;
                }
                if let Err(e) = result {
                    error!("Item rows could NOT be stored: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::SaveGroundItems(rows)) => {
                let mut result = store.store_ground_items(&rows).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store the items on the ground (try {} of {attempts}): {e}",
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_ground_items(&rows).await;
                }
                // The next periodic save (or the one at shutdown) writes the whole table again.
                if let Err(e) = result {
                    error!("The items on the ground could NOT be stored: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::SkillChanges {
                owner,
                upserts,
                removed,
            }) => {
                let mut result = store.store_skill_changes(owner, &upserts, &removed).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store skill changes of {} (try {} of {attempts}): {e}",
                        owner.0,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_skill_changes(owner, &upserts, &removed).await;
                }
                // A lost skill write is caught by the next full store, which compares the whole
                // skill list against what was last written.
                if let Err(e) = result {
                    error!("Skill changes of {} could NOT be stored: {e}", owner.0);
                }
            }
            OutboxMessage::Persist(PersistCommand::RecoValues {
                owner,
                rec_have,
                rec_left,
            }) => {
                let mut result = store.store_reco_values(owner, rec_have, rec_left).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store the recommendations of {} (try {} of {attempts}): {e}",
                        owner.0,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_reco_values(owner, rec_have, rec_left).await;
                }
                // Java logs it and goes on; the next full store writes the counters again.
                if let Err(e) = result {
                    error!("The recommendations of {} could NOT be stored: {e}", owner.0);
                }
            }
            OutboxMessage::Persist(PersistCommand::ResetRecommends) => {
                // Java: "Could not reset Recommendations System", logged, no retry.
                if let Err(e) = store.reset_recommends().await {
                    error!("Could not reset Recommendations System: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::PlayerVariables {
                owner,
                upserts,
                removed,
            }) => {
                let mut result = store.store_player_variables(owner, &upserts, &removed).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store the variables of {} (try {} of {attempts}): {e}",
                        owner.0,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_player_variables(owner, &upserts, &removed).await;
                }
                // The variables stay marked as changed in the world: the next full store writes
                // them again.
                if let Err(e) = result {
                    error!("The variables of {} could NOT be stored: {e}", owner.0);
                }
            }

            OutboxMessage::Persist(PersistCommand::StorePunishment(punishment)) => {
                let mut result = store.store_punishment(&punishment).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store the punishment of {} (try {} of {attempts}): {e}",
                        punishment.key,
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_punishment(&punishment).await;
                }
                // Java logs it and goes on: the punishment is active, it is only not stored.
                if let Err(e) = result {
                    error!("The punishment of {} could NOT be stored: {e}", punishment.key);
                }
            }
            OutboxMessage::Persist(PersistCommand::EndPunishment {
                id,
                key,
                affect,
                kind,
                ended_at,
            }) => {
                let mut result = store.end_punishment(id, &key, affect, kind, ended_at).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not end the punishment of {key} (try {} of {attempts}): {e}",
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.end_punishment(id, &key, affect, kind, ended_at).await;
                }
                if let Err(e) = result {
                    error!("The end of the punishment of {key} could NOT be stored: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::CharAccessLevel { char_id, level }) => {
                let mut result = store.change_char_access_level(char_id, level).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not set the access level of character {char_id} (try {} of {attempts}): {e}",
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.change_char_access_level(char_id, level).await;
                }
                if let Err(e) = result {
                    error!("The access level of character {char_id} could NOT be stored: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::RepairChar { name }) => {
                let mut result = store.repair_char(&name).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not repair the character {name} (try {} of {attempts}): {e}",
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.repair_char(&name).await;
                }
                if let Err(e) = result {
                    error!("The character {name} could NOT be repaired: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::Summon(write)) => {
                let mut result = store.store_summon(&write).await;
                for attempt in 2..=attempts {
                    let Err(e) = &result else { break };
                    warn!(
                        "Could not store the summon tables ({write:?}, try {} of {attempts}): {e}",
                        attempt - 1
                    );
                    tokio::time::sleep(retry.delay).await;
                    result = store.store_summon(&write).await;
                }
                // Java logs it and goes on ("Summon cannot be removed", "Could not store summon
                // effect data").
                if let Err(e) = result {
                    error!("The summon tables could NOT be written ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::PetRowRequest {
                owner,
                control_object_id,
            }) => {
                // Not a write: the answer goes back to the world (D-15). A failed read is
                // reported as a failure, and the world summons nothing.
                let result = match store.load_pet_row(control_object_id).await {
                    Ok(data) => Ok(data),
                    Err(e) => {
                        warn!(
                            "Could not read the pet row of collar {}: {e}",
                            control_object_id.0
                        );
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::PetRowLoaded {
                    owner,
                    control_object_id,
                    result,
                };
                if !acks.send_to_world(answer) {
                    warn!(
                        "The pet row of collar {} could not be handed to the world.",
                        control_object_id.0
                    );
                }
            }
            OutboxMessage::Persist(PersistCommand::MailAttachmentsRequest {
                player,
                message_id,
                sender_id,
            }) => {
                // Not a write: the answer goes back to the world (D-15). The queue is in order, so
                // the rows include every write sent before the request. A failed read is reported
                // as a failure and the world hands nothing out.
                let result = match store.load_mail_attachments(sender_id, message_id).await {
                    Ok(items) => Ok(items),
                    Err(e) => {
                        warn!("Could not read the attachments of mail {message_id}: {e}");
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::MailAttachmentsLoaded {
                    player,
                    message_id,
                    result,
                };
                if !acks.send_to_world(answer) {
                    warn!("The attachments of mail {message_id} could not be handed to the world.");
                }
            }
            OutboxMessage::Persist(PersistCommand::OfflinePetRowsRequest {
                owner,
                control_object_id,
            }) => {
                // Not a write either: see above.
                let result = match store.load_offline_pet_rows(owner).await {
                    Ok(rows) => Ok(rows),
                    Err(e) => {
                        warn!(
                            "Could not read the pet and inventory rows of character {}: {e}",
                            owner.0
                        );
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::OfflinePetRowsLoaded {
                    owner,
                    control_object_id,
                    result,
                };
                if !acks.send_to_world(answer) {
                    warn!(
                        "The pet rows of character {} could not be handed to the world.",
                        owner.0
                    );
                }
            }
            OutboxMessage::Persist(PersistCommand::BlockListRequest { sender, owner }) => {
                // Not a write either: see above.
                let result = match store.load_block_list(owner).await {
                    Ok(list) => Ok(list),
                    Err(e) => {
                        warn!("Could not read the block list of character {owner}: {e}");
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::BlockListLoaded {
                    sender,
                    owner,
                    result,
                };
                if !acks.send_to_world(answer) {
                    warn!("The block list of character {owner} could not be handed to the world.");
                }
            }
            OutboxMessage::Persist(PersistCommand::CrestIdsRequest { ticket }) => {
                // Not a write either: see above. Read in order, so the ids include every crest
                // inserted before the request.
                let result = match store.load_crest_ids().await {
                    Ok(ids) => Ok(ids),
                    Err(e) => {
                        warn!("Could not read the used crest ids: {e}");
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::CrestIdsLoaded { ticket, result };
                if !acks.send_to_world(answer) {
                    warn!("The used crest ids could not be handed to the world.");
                }
            }
            OutboxMessage::Persist(PersistCommand::ClanWarehouseRequest { player, clan_id }) => {
                // Not a write either: see above.
                let result = match store.load_clan_warehouse(clan_id).await {
                    Ok(items) => Ok(items),
                    Err(e) => {
                        warn!("Could not read the warehouse of clan {clan_id}: {e}");
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::ClanWarehouseLoaded {
                    player,
                    clan_id,
                    result,
                };
                if !acks.send_to_world(answer) {
                    warn!("The warehouse of clan {clan_id} could not be handed to the world.");
                }
            }
            OutboxMessage::Persist(PersistCommand::Clan(write)) => {
                // Java logs a failed clan statement and goes on (no retry).
                if let Err(e) = store.store_clan_write(&write).await {
                    error!("Clan data could NOT be stored ({}): {e}", clan_write_name(&write));
                    // The other server may have taken the crest id (M7-FU-03): the world retries
                    // once under a fresh read.
                    if let ClanWrite::Crest(CrestWrite::Insert { crest_id, .. }) = &write
                        && !acks
                            .send_to_world(l2_world::WorldCommand::CrestInsertFailed { crest_id: *crest_id })
                    {
                        warn!("The failed insert of crest {crest_id} could not be handed to the world.");
                    }
                }
            }
            OutboxMessage::Persist(PersistCommand::SevenSigns(write)) => {
                // Java logs a failed statement and goes on ("SevenSigns: Unable to save data").
                if let Err(e) = store.store_seven_signs(&write).await {
                    error!("SevenSigns: Unable to save data to database ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::SevenSignsReadRequest { then }) => {
                // Not a write: the answer goes back to the world (D-15, D-50). A failed read is
                // reported as a failure; the world then changes nothing.
                let result = match store.load_seven_signs().await {
                    Ok(load) => Ok(load),
                    Err(e) => {
                        warn!("SevenSigns: Unable to load Seven Signs data from database: {e}");
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::SevenSignsLoaded { result, then };
                if !acks.send_to_world(answer) {
                    warn!("The Seven Signs rows could not be handed to the world.");
                }
            }
            OutboxMessage::Persist(PersistCommand::Wedding(write)) => {
                // Java logs a failed statement and goes on (no retry).
                if let Err(e) = store.store_wedding(&write).await {
                    error!("A couple could NOT be stored ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::BotReport(row)) => {
                // Java logs a failed statement and goes on (no retry).
                if let Err(e) = store.store_bot_report(&row).await {
                    error!("A bot report could NOT be stored ({row:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::OfflineTrade(write)) => {
                // Java logs a failed statement and goes on (no retry).
                if let Err(e) = store.store_offline_trade(&write).await {
                    error!("Offline trade data could NOT be stored: {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::Bookmark(write)) => {
                // Java logs a failed bookmark statement and goes on (no retry).
                if let Err(e) = store.store_bookmark_write(&write).await {
                    error!("Teleport bookmark data could NOT be stored ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::BbsFavorite(write)) => {
                // Java logs a failed favorites statement and goes on (no retry).
                if let Err(e) = store.store_bbs_favorite(&write).await {
                    error!("Community board favorite could NOT be stored ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::BbsFavoriteRequest { player, kind }) => {
                // Not a write: a read the world asked for, answered in order with the writes
                // before it.
                let result = match store.load_bbs_favorites(player, kind).await {
                    Ok(answer) => Ok(answer),
                    Err(e) => {
                        warn!("Could not read the favorites of character {}: {e}", player.0);
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::BbsFavoritesLoaded { player, kind, result };
                if !acks.send_to_world(answer) {
                    warn!(
                        "The favorites of character {} could not be handed to the world.",
                        player.0
                    );
                }
            }
            OutboxMessage::Persist(PersistCommand::Contact(write)) => {
                // Java logs a failed contact statement and goes on (no retry).
                if let Err(e) = store.store_contact_write(&write).await {
                    error!("Contact list data could NOT be stored ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::FriendSummariesRequest { requester, friends }) => {
                // Not a write: a read the world asked for, answered in order with the writes
                // before it. A friend whose read fails (or has no row) is left out.
                let mut rows = Vec::new();
                for friend in friends {
                    match store.load_friend_summary(friend).await {
                        Ok(Some(row)) => rows.push(row),
                        Ok(None) => {}
                        Err(e) => warn!("Could not read the character row of friend {friend}: {e}"),
                    }
                }
                let answer = l2_world::WorldCommand::FriendSummariesLoaded { requester, rows };
                if !acks.send_to_world(answer) {
                    warn!(
                        "The friend rows for {} could not be handed to the world.",
                        requester.0
                    );
                }
            }
            OutboxMessage::Persist(PersistCommand::Henna(write)) => {
                // Java logs a failed henna statement and goes on (no retry).
                if let Err(e) = store.store_henna_write(&write).await {
                    error!("Henna data could NOT be stored ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::Minigame(write)) => {
                // Java logs a failed statement and goes on (no retry).
                if let Err(e) = store.store_minigame_write(&write).await {
                    error!("UndergroundColiseumManager: could not save ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::InstanceTime(write)) => {
                // Java logs a failed instance time statement and goes on (no retry).
                if let Err(e) = store.store_instance_time(&write).await {
                    error!("Could not store character instance time data ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::ItemAuction(write)) => {
                // Java logs a failed statement and goes on (no retry).
                if let Err(e) = store.store_item_auction_write(&write).await {
                    error!("Item auction data could NOT be stored ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::ItemAuctionRequest { instance_id, then }) => {
                // Not a write either: the answer goes back to the world (D-15).
                let result = match store.load_item_auction_records(instance_id).await {
                    Ok(rows) => Ok(rows),
                    Err(e) => {
                        warn!("Could not read the auctions of item auction instance {instance_id}: {e}");
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::ItemAuctionReread {
                    instance_id,
                    result,
                    then,
                };
                if !acks.send_to_world(answer) {
                    warn!(
                        "The auctions of item auction instance {instance_id} could not be handed to the world."
                    );
                }
            }
            OutboxMessage::Persist(PersistCommand::Friend(write)) => {
                // Java logs a failed friend statement and goes on (no retry).
                if let Err(e) = store.store_friend_write(&write).await {
                    error!("Friend data could NOT be stored ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::Mail(write)) => {
                // Java logs a failed statement and goes on (no retry).
                if let Err(e) = store.store_mail_write(&write).await {
                    error!("Mail data could NOT be stored ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::BuyList(row)) => {
                // Java logs a failed save and goes on (no retry).
                if let Err(e) = store.store_buy_list_product(&row).await {
                    error!(
                        "Failed to save Product buylist_id:{} item_id:{}: {e}",
                        row.list_id, row.item_id
                    );
                }
            }
            OutboxMessage::Persist(PersistCommand::Residence(write)) => {
                // Java logs a failed residence statement and goes on (no retry).
                if let Err(e) = store.store_residence_write(&write).await {
                    error!("Residence data could NOT be stored ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::Manor(write)) => {
                // Java logs a failed statement and goes on ("Unable to store manor data!").
                if let Err(e) = store.store_manor_write(&write).await {
                    error!("CastleManorManager: Unable to store manor data ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::Vehicle(write)) => {
                // Java logs a failed statement and goes on.
                if let Err(e) = store.store_vehicle_write(&write).await {
                    error!("AirShipManager: Could not write the airships table ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::Cursed(write)) => {
                // Java logs a failed statement and goes on ("Failed to save data.").
                if let Err(e) = store.store_cursed_write(&write).await {
                    error!("CursedWeapon: Failed to save data ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::Olympiad(write)) => {
                // Java logs a failed statement and goes on ("Olympiad System: Failed to save ...").
                if let Err(e) = store.store_olympiad_write(&write).await {
                    error!("Olympiad System: Failed to save data ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::OlympiadRereadRequest { request, then }) => {
                // Not a write: the answer goes back to the world (D-15). A failed read is reported
                // as a failure; the world then shows and hands out nothing (D-50).
                let result = match store.load_olympiad_reread(&request).await {
                    Ok(rows) => Ok(rows),
                    Err(e) => {
                        warn!("Olympiad System: Couldn't read {request:?}: {e}");
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::OlympiadReread { result, then };
                if !acks.send_to_world(answer) {
                    warn!("The Olympiad read {request:?} could not be handed to the world.");
                }
            }
            OutboxMessage::Persist(PersistCommand::ResidenceRereadRequest {
                residence_id,
                kind,
                then,
            }) => {
                // Not a write: the answer goes back to the world (D-15). A failed read is reported
                // as a failure; the world then changes nothing (D-50).
                let result = match store.load_residence(kind, residence_id).await {
                    Ok(rows) => Ok(rows),
                    Err(e) => {
                        warn!("Could not read residence {residence_id} ({kind:?}) again: {e}");
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::ResidenceReread { result, then };
                if !acks.send_to_world(answer) {
                    warn!("Residence {residence_id} could not be handed to the world.");
                }
            }
            OutboxMessage::Persist(PersistCommand::Boss(write)) => {
                // Java logs a failed statement and goes on (no retry).
                if let Err(e) = store.store_boss_write(&write).await {
                    error!("Boss data could NOT be stored ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::BossRereadRequest { kind, then }) => {
                // Not a write: the answer goes back to the world (D-15). A failed read is reported
                // as a failure; the world then changes nothing and refuses the action (D-50).
                let result = match store.load_boss_rows(kind).await {
                    Ok(rows) => Ok(rows),
                    Err(e) => {
                        warn!("Could not read the boss rows {kind:?} again: {e}");
                        Err(())
                    }
                };
                let answer = l2_world::WorldCommand::BossReread { result, then };
                if !acks.send_to_world(answer) {
                    warn!("The boss rows {kind:?} could not be handed to the world.");
                }
            }
            OutboxMessage::Persist(PersistCommand::Subclass(write)) => {
                // Java logs a failed statement and goes on (no retry, S-SUB-01).
                if let Err(e) = store.store_subclass_write(&write).await {
                    error!("The subclass rows could NOT be written ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::ClassIndexRequest { owner, class_index }) => {
                // Not a write: the answer goes back to the world (D-15). Read in order, so the
                // rows include the store `setActiveClass` sent before it.
                let load = store.load_class_index(owner, class_index).await;
                let answer = l2_world::WorldCommand::ClassIndexLoaded {
                    owner,
                    class_index,
                    load: Box::new(load),
                };
                if !acks.send_to_world(answer) {
                    warn!(
                        "The rows of class index {class_index} of character {} could not be handed to the world.",
                        owner.0
                    );
                }
            }
            OutboxMessage::Persist(PersistCommand::Recipe(write)) => {
                // Java logs a failed statement and goes on ("SQL exception while inserting
                // recipe", no retry).
                if let Err(e) = store.store_recipe_write(&write).await {
                    error!("The recipe book row could NOT be written ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::FishingChampionship(data)) => {
                // Java logs a failed save and goes on (no retry).
                if let Err(e) = store.store_fishing_championship(&data).await {
                    error!("Fishing Championship Manager: can't update infos: {e}");
                }
            }
            OutboxMessage::Ls(LsEvent::PlayerInGame { account }) => {
                let _ = notices.send(LsNotice::PlayerInGame { account });
            }
            OutboxMessage::Ls(LsEvent::AccessLevel { account, level }) => {
                let _ = notices.send(LsNotice::AccessLevel { account, level });
            }
            OutboxMessage::Ls(LsEvent::PlayerLogout { account }) => {
                let _ = notices.send(LsNotice::PlayerLogout { account });
            }
            OutboxMessage::Ls(LsEvent::SetServerStatus(status)) => {
                let _ = notices.send(LsNotice::SetServerStatus(status));
            }
            OutboxMessage::Ls(LsEvent::SetMaxPlayer(max_players)) => {
                let _ = notices.send(LsNotice::SetMaxPlayer(max_players));
            }
            OutboxMessage::Ls(LsEvent::ServerStatus { id, value }) => {
                let _ = notices.send(LsNotice::ServerStatus { id, value });
            }
            OutboxMessage::Control(request) => acks.control(request),
            OutboxMessage::Persist(PersistCommand::Announcement(write)) => {
                // Java logs a failed statement and goes on (no retry). An insert is answered with
                // the id the database gave the row, or with a failure.
                let result = store.store_announcement(&write).await;
                if let AnnouncementWrite::Insert { ticket, .. } = &write {
                    let answer = l2_world::WorldCommand::AnnouncementStored {
                        ticket: *ticket,
                        result: match &result {
                            Ok(id) => Ok(*id),
                            Err(_) => Err(()),
                        },
                    };
                    if !acks.send_to_world(answer) {
                        warn!("The id of a new announcement could not be handed to the world.");
                    }
                }
                if let Err(e) = result {
                    warn!("Announcement: Couldn't store announcement ({write:?}): {e}");
                }
            }
            OutboxMessage::Persist(PersistCommand::PetitionFeedback {
                character,
                gm,
                rate,
                message,
                time,
            }) => {
                // Java logs a failed statement and goes on (no retry).
                if let Err(e) = store
                    .store_petition_feedback(&character, &gm, rate, &message, time)
                    .await
                {
                    warn!("RequestPetitionFeedback: Error while saving petition feedback: {e}");
                }
            }
        }
    }
    debug!("Outbox closed, persistence task done.");
}

/// Java `FishingChampionshipManager.restoreData()`: the rows of `fishing_championship` and the
/// global variable `fishChampionshipEnd`. Only reads (D-39). A database error is logged and the
/// championship starts empty, like Java.
pub async fn load_fishing_championship(pool: &Pool) -> ChampionshipData {
    let repo = FishingChampionshipRepository::new(pool.clone());
    let mut data = ChampionshipData::default();
    match repo.end_date().await {
        Ok(end_date) => data.end_date = end_date,
        Err(e) => warn!("FishingChampionshipManager: can't restore the end of the period: {e}"),
    }
    match repo.load_rows().await {
        Ok(rows) => {
            data.rows = rows
                .into_iter()
                .map(|r| ChampionshipRow {
                    name: r.player_name,
                    length: r.fish_length,
                    rewarded: r.rewarded,
                })
                .collect();
        }
        Err(e) => warn!("FishingChampionshipManager: can't restore fishing championship info: {e}"),
    }
    data
}

fn grand_boss_update(row: &GrandBossData) -> GrandBossUpdate {
    GrandBossUpdate {
        boss_id: row.boss_id,
        loc_x: row.loc_x,
        loc_y: row.loc_y,
        loc_z: row.loc_z,
        heading: row.heading,
        respawn_time: row.respawn_time,
        current_hp: row.current_hp,
        current_mp: row.current_mp,
        status: row.status,
    }
}

fn grand_boss_data(row: l2_db::GrandBossRow) -> GrandBossData {
    GrandBossData {
        boss_id: row.boss_id,
        loc_x: row.loc_x,
        loc_y: row.loc_y,
        loc_z: row.loc_z,
        heading: row.heading,
        respawn_time: row.respawn_time,
        current_hp: row.current_hp,
        current_mp: row.current_mp,
        status: row.status,
    }
}

fn raid_points_record(row: l2_db::RaidPointsRow) -> RaidPointsRecord {
    RaidPointsRecord {
        char_id: row.char_id,
        boss_id: row.boss_id,
        points: row.points,
    }
}

/// Java `GrandBossManager.init`, `initZones` and `RaidBossPointsManager.init`: the grand boss
/// states, the players of the boss zones and the raid points, read only (M8-BOSS-01). A database
/// error is logged and the table counts as empty, like Java.
pub async fn load_bosses(pool: &Pool) -> BossLoad {
    let repo = BossRepository::new(pool.clone());
    let mut load = BossLoad::default();
    match repo.grand_bosses().await {
        Ok(rows) => load.grand_bosses = rows.into_iter().map(grand_boss_data).collect(),
        Err(e) => warn!("GrandBossManager: Could not load grandboss_data table: {e}"),
    }
    match repo.grand_boss_list().await {
        Ok(rows) => load.grand_boss_list = rows.into_iter().map(|r| (r.player_id, r.zone)).collect(),
        Err(e) => warn!("GrandBossManager: Could not load grandboss_list table: {e}"),
    }
    match repo.raid_points().await {
        Ok(rows) => load.raid_points = rows.into_iter().map(raid_points_record).collect(),
        Err(e) => warn!("RaidBossPointsManager: Could not load raid points {e}"),
    }
    load
}

/// Java `RaidBossSpawnManager.load()`, the database half: the rows of `raidboss_spawnlist`, read
/// only. A database error is logged and the list counts as empty, like Java.
pub async fn load_raid_boss_rows(pool: &Pool) -> Vec<RaidBossData> {
    match BossRepository::new(pool.clone()).raid_bosses().await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| RaidBossData {
                boss_id: r.boss_id,
                amount: r.amount,
                loc_x: r.loc_x,
                loc_y: r.loc_y,
                loc_z: r.loc_z,
                heading: r.heading,
                respawn_delay: r.respawn_delay,
                respawn_random: r.respawn_random,
                respawn_time: r.respawn_time,
                current_hp: r.current_hp,
                current_mp: r.current_mp,
            })
            .collect(),
        Err(e) => {
            warn!("RaidBossSpawnManager: Could not load raidboss_spawnlist table");
            debug!("{e}");
            Vec::new()
        }
    }
}

/// Java `BuyListData.load()`, the database half: the rows of `buylists`, which hold the stock and
/// the restock times the last run left. A database error is logged and the lists start full, like
/// Java.
pub async fn load_buy_list_stock(pool: &Pool) -> Vec<ProductRow> {
    match BuyListRepository::new(pool.clone()).load_all().await {
        Ok(rows) => rows
            .into_iter()
            .map(|row| ProductRow {
                list_id: row.buylist_id,
                item_id: row.item_id,
                count: row.count,
                next_restock_time: row.next_restock_time,
            })
            .collect(),
        Err(e) => {
            warn!("Failed to load buyList data from database. {e}");
            Vec::new()
        }
    }
}

/// Java `MailManager`'s constructor and `CharInfoTable`'s, the database half: every mail with the
/// items of its `Mail` container, and the id, name and access level of every character. Only reads
/// (D-39). A database error is logged and the post box starts empty, like Java.
pub async fn load_mail(pool: &Pool) -> MailLoad {
    let mut load = MailLoad::default();
    match CharacterRepository::new(pool.clone()).all_names().await {
        Ok(rows) => {
            load.characters = rows
                .into_iter()
                .map(|row| MailCharacter {
                    id: i32::try_from(row.char_id).unwrap_or(0),
                    name: row.char_name,
                    access_level: row.accesslevel.unwrap_or(0),
                })
                .collect();
        }
        Err(e) => warn!("CharInfoTable: Couldn't retrieve all char id/name/access: {e}"),
    }
    let rows = match MessageRepository::new(pool.clone()).load_all().await {
        Ok(rows) => rows,
        Err(e) => {
            warn!("MailManager: Error loading from database: {e}");
            return load;
        }
    };
    // Java `Mail.restore()`: `WHERE owner_id=? AND loc='MAIL' AND loc_data=?` per mail; here one
    // read per sender, split by message.
    let items = ItemRepository::new(pool.clone());
    let mut by_sender: std::collections::HashMap<i32, Vec<ItemData>> = std::collections::HashMap::new();
    for row in rows.iter().filter(|r| r.has_attachments) {
        if by_sender.contains_key(&row.sender_id) {
            continue;
        }
        let list = match load_container_items(&items, row.sender_id, ItemLocation::Mail).await {
            Ok(list) => list,
            Err(e) => {
                warn!("Could not restore container: {e}");
                Vec::new()
            }
        };
        by_sender.insert(row.sender_id, list);
    }
    load.messages = mail_messages_of(rows, &by_sender);
    load
}

/// The mails with the items of their `Mail` containers: `by_sender` holds the `MAIL` rows of each
/// sender, the items of a message are the ones whose `loc_data` is its id.
fn mail_messages_of(
    rows: Vec<l2_db::MessageRow>,
    by_sender: &std::collections::HashMap<i32, Vec<ItemData>>,
) -> Vec<MailMessage> {
    rows.into_iter()
        .map(|row| {
            let attachments = if row.has_attachments {
                by_sender
                    .get(&row.sender_id)
                    .map(|list| {
                        list.iter()
                            .filter(|i| i.loc_data == row.message_id)
                            .cloned()
                            .collect()
                    })
                    .unwrap_or_default()
            } else {
                Vec::new()
            };
            MailMessage {
                id: row.message_id,
                sender_id: row.sender_id,
                receiver_id: row.receiver_id,
                subject: row.subject,
                content: row.content,
                expiration: row.expiration,
                req_adena: row.req_adena,
                has_attachments: row.has_attachments,
                unread: row.is_unread,
                returned: row.is_returned,
                send_by_system: row.send_by_system,
                deleted_by_sender: row.is_deleted_by_sender,
                deleted_by_receiver: row.is_deleted_by_receiver,
                attachments,
            }
        })
        .collect()
}

/// The mails of one character as sender or receiver with their attachments, read at
/// `CharacterSelect` (M6-SYNC-01): the Java game server shares the tables (D-39), so what
/// `load_mail` read at startup may be out of date. Only reads. A failed read is an error, never
/// an empty list: the world then keeps what it has.
pub async fn load_mail_of_character(
    messages: &MessageRepository,
    items: &ItemRepository,
    char_id: i32,
) -> Result<MailRefresh, l2_db::DbError> {
    let rows = messages.messages_of_character(char_id).await?;
    let mut by_sender: std::collections::HashMap<i32, Vec<ItemData>> = std::collections::HashMap::new();
    for row in rows.iter().filter(|r| r.has_attachments) {
        if let std::collections::hash_map::Entry::Vacant(entry) = by_sender.entry(row.sender_id) {
            entry.insert(load_container_items(items, row.sender_id, ItemLocation::Mail).await?);
        }
    }
    Ok(MailRefresh {
        messages: mail_messages_of(rows, &by_sender),
    })
}

/// The attachment rows of one mail (Java `Mail.restore`: `owner_id` = the sender, `loc = 'MAIL'`,
/// `loc_data` = the message id), read before `RequestPostAttachment` hands them out (M6-SYNC-01).
pub async fn load_mail_attachments(
    items: &ItemRepository,
    sender_id: i32,
    message_id: i32,
) -> Result<Vec<ItemData>, l2_db::DbError> {
    let mut list = load_container_items(items, sender_id, ItemLocation::Mail).await?;
    list.retain(|i| i.loc_data == message_id);
    Ok(list)
}

/// One shop of `storeOffliners`: its lines, then its row.
async fn write_offline_shop(
    repo: &OfflineTradeRepository,
    row: &OfflineTradeRow,
) -> Result<(), PersistError> {
    for line in &row.items {
        repo.insert_item(row.char_id.0, line.item, line.count, line.price)
            .await?;
    }
    repo.insert_status(
        row.char_id.0,
        row.status.time,
        row.status.store_type,
        row.status.title.as_deref(),
    )
    .await?;
    Ok(())
}

/// Java `OfflineTraderTable.restoreOfflineTraders`, the database half: the shops of the tables
/// that are young enough and of a known type, each with its character loaded like at
/// `CharacterSelect`. Only reads (D-39). A shop whose character cannot be read is skipped with
/// Java's warning; the world sits them down (`World::restore_offline_traders`).
pub async fn load_offline_traders(
    pool: &Pool,
    settings: &OfflineSettings,
    now_ms: i64,
) -> Vec<(PlayerData, OfflineTradeRow)> {
    use crate::client::enter::{DbPlayerLoader, PlayerLoader};
    let repo = OfflineTradeRepository::new(pool.clone());
    let characters = CharacterRepository::new(pool.clone());
    let loader = DbPlayerLoader::new(pool.clone());
    let statuses = match repo.load_status().await {
        Ok(rows) => rows,
        Err(e) => {
            warn!("OfflineTraderTable: Error while loading offline traders: {e}");
            return Vec::new();
        }
    };
    let mut traders = Vec::new();
    for status in statuses {
        let Ok(char_id) = i32::try_from(status.char_id) else {
            continue;
        };
        let time = i64::try_from(status.time).unwrap_or(i64::MAX);
        if l2_world::restorable_store_type(settings, time, i32::from(status.store_type), now_ms).is_none() {
            continue;
        }
        let account = match characters.restore_character(char_id).await {
            Ok(Some(row)) => row.account_name.unwrap_or_default(),
            _ => {
                warn!("OfflineTraderTable: Error loading trader: the character {char_id} is not there.");
                continue;
            }
        };
        let player = match loader.load_player(&account, char_id).await {
            Ok(player) => player,
            Err(e) => {
                warn!("OfflineTraderTable: Error loading trader: {char_id} {e}");
                continue;
            }
        };
        let items = match repo.load_items(char_id).await {
            Ok(rows) => rows
                .into_iter()
                .map(|r| OfflineTradeLine {
                    item: i32::try_from(r.item).unwrap_or(0),
                    count: i64::try_from(r.count).unwrap_or(i64::MAX),
                    price: i64::try_from(r.price).unwrap_or(i64::MAX),
                })
                .collect(),
            Err(e) => {
                warn!("OfflineTraderTable: Error loading trader: {char_id} {e}");
                continue;
            }
        };
        traders.push((
            player,
            OfflineTradeRow {
                char_id: ObjectId(char_id),
                status: OfflineTradeStatus {
                    time,
                    store_type: i32::from(status.store_type),
                    title: status.title,
                },
                items,
            },
        ));
    }
    traders
}

/// Java `BotReportTable.loadReportedCharData`, the database half: the rows of
/// `bot_reported_char_data`. A database error is logged and the server goes on without reports,
/// like Java. Only with `EnableBotReportButton`, like Java's constructor.
pub async fn load_bot_reports(pool: &Pool) -> Vec<BotReportRow> {
    match BotReportRepository::new(pool.clone()).load_all().await {
        Ok(rows) => rows
            .into_iter()
            .filter_map(|row| {
                Some(BotReportRow {
                    bot_id: i32::try_from(row.bot_id).ok()?,
                    reporter_id: i32::try_from(row.reporter_id).ok()?,
                    report_date: i64::try_from(row.report_date).unwrap_or(i64::MAX),
                })
            })
            .collect(),
        Err(e) => {
            warn!("BotReportTable: Could not load reported char data! {e}");
            Vec::new()
        }
    }
}

/// Java `PunishmentManager.load()`, the database half: the rows of `punishments`. A database
/// error is logged and the server goes on without punishments, like Java. The world sorts out
/// the unknown and the expired ones (`World::load_punishments`).
pub async fn load_punishments(pool: &Pool) -> Vec<PunishmentRecord> {
    match PunishmentRepository::new(pool.clone()).load_all().await {
        Ok(rows) => rows
            .into_iter()
            .map(|row| PunishmentRecord {
                id: i32::try_from(row.id).unwrap_or(0),
                key: row.key,
                affect: row.affect,
                kind: row.kind,
                expiration: row.expiration,
                reason: row.reason,
                punished_by: row.punished_by,
            })
            .collect(),
        Err(e) => {
            warn!("PunishmentManager: Error while loading punishments: {e}");
            Vec::new()
        }
    }
}

/// A world member row as the `seven_signs` row Java writes (`ps.setInt` narrows the stones,
/// `ps.setDouble` the two `DECIMAL` columns).
pub fn seven_signs_row(p: &l2_world::sevensigns::SignsPlayer) -> l2_db::SevenSignsRow {
    l2_db::SevenSignsRow {
        char_id: p.char_id,
        cabal: p.cabal.clone(),
        seal: p.seal,
        red_stones: p.red_stones as i32,
        green_stones: p.green_stones as i32,
        blue_stones: p.blue_stones as i32,
        ancient_adena_amount: p.ancient_adena_amount as f64,
        contribution_score: p.contribution_score as f64,
    }
}

/// A `seven_signs` row as the world keeps it (`rs.getInt` / `rs.getDouble`).
pub fn signs_player_of(r: l2_db::SevenSignsRow) -> l2_world::sevensigns::SignsPlayer {
    l2_world::sevensigns::SignsPlayer {
        char_id: r.char_id,
        cabal: r.cabal,
        seal: r.seal,
        red_stones: i64::from(r.red_stones),
        green_stones: i64::from(r.green_stones),
        blue_stones: i64::from(r.blue_stones),
        ancient_adena_amount: r.ancient_adena_amount as i64,
        contribution_score: r.contribution_score as i64,
    }
}

/// The world's cycle state as the `seven_signs_status` row (`UPDATE_STATUS`).
pub fn seven_signs_status_row(s: &l2_world::sevensigns::SevenSignsStatus) -> l2_db::SevenSignsStatusRow {
    l2_db::SevenSignsStatusRow {
        current_cycle: s.current_cycle,
        festival_cycle: s.festival_cycle,
        active_period: s.active_period,
        date: s.last_save,
        previous_winner: s.previous_winner,
        dawn_stone_score: s.dawn_stone_score,
        dawn_festival_score: s.dawn_festival_score,
        dusk_stone_score: s.dusk_stone_score,
        dusk_festival_score: s.dusk_festival_score,
        avarice_owner: s.seal_owners[1],
        gnosis_owner: s.seal_owners[2],
        strife_owner: s.seal_owners[3],
        avarice_dawn_score: s.dawn_seal_totals[1],
        gnosis_dawn_score: s.dawn_seal_totals[2],
        strife_dawn_score: s.dawn_seal_totals[3],
        avarice_dusk_score: s.dusk_seal_totals[1],
        gnosis_dusk_score: s.dusk_seal_totals[2],
        strife_dusk_score: s.dusk_seal_totals[3],
        accumulated_bonus: s.accumulated_bonus,
    }
}

/// The `seven_signs_status` row as the world keeps it (`restoreSevenSignsData`).
pub fn seven_signs_status_of(r: l2_db::SevenSignsStatusRow) -> l2_world::sevensigns::SevenSignsStatus {
    l2_world::sevensigns::SevenSignsStatus {
        current_cycle: r.current_cycle,
        active_period: r.active_period,
        previous_winner: r.previous_winner,
        dawn_stone_score: r.dawn_stone_score,
        dawn_festival_score: r.dawn_festival_score,
        dusk_stone_score: r.dusk_stone_score,
        dusk_festival_score: r.dusk_festival_score,
        seal_owners: [0, r.avarice_owner, r.gnosis_owner, r.strife_owner],
        dawn_seal_totals: [0, r.avarice_dawn_score, r.gnosis_dawn_score, r.strife_dawn_score],
        dusk_seal_totals: [0, r.avarice_dusk_score, r.gnosis_dusk_score, r.strife_dusk_score],
        festival_cycle: r.festival_cycle,
        accumulated_bonus: r.accumulated_bonus,
        last_save: r.date,
    }
}

/// Every `seven_signs` row and the status row.
pub async fn read_seven_signs(
    repo: &l2_db::SevenSignsRepository,
) -> Result<l2_world::sevensigns::SevenSignsLoad, PersistError> {
    let players = repo.load_all().await?.into_iter().map(signs_player_of).collect();
    let status = repo.load_status().await?.map(seven_signs_status_of);
    let festival = repo
        .load_festival()
        .await?
        .into_iter()
        .map(festival_data_of)
        .collect();
    Ok(l2_world::sevensigns::SevenSignsLoad {
        players,
        status,
        festival,
    })
}

/// A `seven_signs_festival` row as the world keeps it (`restoreFestivalData`).
pub fn festival_data_of(r: l2_db::FestivalRow) -> l2_world::sevensigns::FestivalData {
    l2_world::sevensigns::FestivalData {
        festival_id: r.festival_id,
        cabal: r.cabal,
        cycle: r.cycle,
        date: i64::try_from(r.date).unwrap_or(i64::MAX),
        score: r.score,
        members: r.members,
    }
}

/// A world festival set as the row `saveFestivalData` writes (`Long.parseLong(date)`).
pub fn festival_row(d: &l2_world::sevensigns::FestivalData) -> l2_db::FestivalRow {
    l2_db::FestivalRow {
        festival_id: d.festival_id,
        cabal: d.cabal.clone(),
        cycle: d.cycle,
        date: u64::try_from(d.date).unwrap_or(0),
        score: d.score,
        members: d.members.clone(),
    }
}

/// Java `DimensionalRiftManager.loadRooms()` (read only). A database that cannot be read gives no
/// rooms, like Java ("Can't load Dimension Rift zones").
pub async fn load_dimensional_rift_rooms(pool: &Pool) -> Vec<l2_world::sevensigns::RiftRoomDef> {
    match l2_db::DimensionalRiftRepository::new(pool.clone())
        .load_all()
        .await
    {
        Ok(rows) => rows
            .into_iter()
            .map(|r| l2_world::sevensigns::RiftRoomDef {
                kind: i8::try_from(r.kind).unwrap_or(i8::MAX),
                room: i8::try_from(r.room_id).unwrap_or(i8::MAX),
                x_min: r.x_min,
                x_max: r.x_max,
                y_min: r.y_min,
                y_max: r.y_max,
                z_min: r.z_min,
                z_max: r.z_max,
                x_t: r.x_t,
                y_t: r.y_t,
                z_t: r.z_t,
                boss: r.boss > 0,
            })
            .collect(),
        Err(e) => {
            warn!("Can't load Dimension Rift zones. {e}");
            Vec::new()
        }
    }
}

/// Java `SevenSigns.restoreSevenSignsData()` (read only at start, D-53). A database that cannot be
/// read gives the defaults, like Java ("Unable to load Seven Signs data from database").
pub async fn load_seven_signs(pool: &Pool) -> l2_world::sevensigns::SevenSignsLoad {
    match read_seven_signs(&l2_db::SevenSignsRepository::new(pool.clone())).await {
        Ok(load) => load,
        Err(e) => {
            error!("SevenSigns: Unable to load Seven Signs data from database: {e}");
            l2_world::sevensigns::SevenSignsLoad::default()
        }
    }
}

/// Java `AutoSpawnHandler.restoreSpawnData()`: the groups of `random_spawn` (read only). A failed
/// read gives none, like Java ("Could not restore spawn data").
pub async fn load_auto_spawns(pool: &Pool) -> Vec<l2_world::sevensigns::AutoSpawnGroup> {
    match l2_db::RandomSpawnRepository::new(pool.clone()).load_all().await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| l2_world::sevensigns::AutoSpawnGroup {
                npc_id: r.npc_id,
                count: r.count,
                initial_delay: r.initial_delay,
                respawn_delay: r.respawn_delay,
                despawn_delay: r.despawn_delay,
                broadcast: r.broadcast_spawn,
                random_spawn: r.random_spawn,
                locations: r.locations,
            })
            .collect(),
        Err(e) => {
            warn!("AutoSpawnHandler: Could not restore spawn data: {e}");
            Vec::new()
        }
    }
}

/// Java `CoupleManager.load()`, the database half (only with `AllowWedding`): the couples of
/// `mods_wedding`. A database that cannot be read gives none, like Java.
pub async fn load_couples(pool: &Pool) -> Vec<l2_world::wedding::Couple> {
    match l2_db::WeddingRepository::new(pool.clone()).load_all().await {
        Ok(rows) => rows
            .into_iter()
            .map(|row| l2_world::wedding::Couple {
                id: row.id,
                player1_id: row.player1_id,
                player2_id: row.player2_id,
                married: row.married,
                affiance_date: row.affiance_date,
                wedding_date: row.wedding_date,
            })
            .collect(),
        Err(e) => {
            warn!("Exception: CoupleManager.load(): {e}");
            Vec::new()
        }
    }
}

/// Java `AnnouncementsTable.load()`, the database half: the rows of `announcements`. A database
/// error is logged and the server goes on without announcements, like Java. The world sorts out
/// the types (`World::load_announcements`).
pub async fn load_announcements(pool: &Pool) -> Vec<AnnouncementRecord> {
    match AnnouncementRepository::new(pool.clone()).load_all().await {
        Ok(rows) => rows
            .into_iter()
            .map(|row| AnnouncementRecord {
                id: i32::try_from(row.id).unwrap_or(0),
                kind: row.kind,
                initial: row.initial,
                delay: row.delay,
                repeat: row.repeat,
                author: row.author,
                content: row.content,
            })
            .collect(),
        Err(e) => {
            warn!("AnnouncementsTable: Failed loading announcements: {e}");
            Vec::new()
        }
    }
}

/// A `pets` row as the world keeps it (`Pet.restore`).
pub fn saved_pet_of(r: l2_db::PetRow) -> SavedPet {
    SavedPet {
        control_object_id: ObjectId(i32::try_from(r.item_obj_id).unwrap_or(0)),
        name: r.name.filter(|n| !n.is_empty()),
        level: i32::from(r.level),
        cur_hp: i32::try_from(r.cur_hp.unwrap_or(0)).unwrap_or(i32::MAX),
        cur_mp: i32::try_from(r.cur_mp.unwrap_or(0)).unwrap_or(i32::MAX),
        exp: i64::try_from(r.exp.unwrap_or(0)).unwrap_or(i64::MAX),
        sp: i64::from(r.sp.unwrap_or(0)),
        fed: i32::try_from(r.fed.unwrap_or(0)).unwrap_or(i32::MAX),
        restore: r.restore == "true",
    }
}

/// A `character_pet_skills_save` row as the world keeps it (`Pet.restoreEffects`).
pub fn saved_pet_effect_of(r: l2_db::PetSkillSaveRow) -> SavedPetEffect {
    SavedPetEffect {
        control_object_id: ObjectId(r.pet_obj_item_id),
        skill_id: r.skill_id,
        skill_level: r.skill_level,
        remaining_time: r.remaining_time,
    }
}

/// Java `PetNameTable.doesPetNameExist`, read once: the name and the collar item id of every
/// named pet. Only reads (D-39); a failed read leaves the table empty and is logged.
pub async fn load_pet_names(pool: &Pool) -> Vec<(String, i32)> {
    match PetRepository::new(pool.clone()).all_pet_names().await {
        Ok(names) => names,
        Err(e) => {
            warn!("PetNameTable: Could not load the pet names: {e}");
            Vec::new()
        }
    }
}

/// Java `ClanTable`'s constructor, the database half: every clan (`new Clan(clanId)`'s queries)
/// and the `clan_wars` rows. Only reads (D-39). A clan whose rows cannot be read is logged and
/// left out, like Java's caught `restore` (which keeps a half-restored clan).
pub async fn load_clans(pool: &Pool) -> (Vec<ClanRecord>, Vec<(String, String)>) {
    let repo = ClanRepository::new(pool.clone());
    let ids = match repo.clan_ids().await {
        Ok(ids) => ids,
        Err(e) => {
            error!("ClanTable: Error restoring ClanTable. {e}");
            return (Vec::new(), Vec::new());
        }
    };
    let items = ItemRepository::new(pool.clone());
    let mut clans = Vec::with_capacity(ids.len());
    for clan_id in ids {
        match load_clan(&repo, clan_id).await {
            Ok(mut record) => {
                // `_warehouse.restore()`: Java catches a failed read and goes on with an empty one.
                record.warehouse = load_clan_warehouse(&items, clan_id).await;
                clans.push(record);
            }
            Err(e) => error!("Clan: Error restoring clan data of clan {clan_id}: {e}"),
        }
    }
    let wars = match repo.clan_wars().await {
        Ok(rows) => rows.into_iter().map(|r| (r.clan1, r.clan2)).collect(),
        Err(e) => {
            error!("ClanTable: Error restoring clan wars data. {e}");
            Vec::new()
        }
    };
    (clans, wars)
}

/// Java `new ClanWarehouse(clan)` + `restore()`: the `CLANWH` items of the clan. A failed read is
/// logged and gives an empty warehouse ("Could not restore container").
pub async fn load_clan_warehouse(items: &ItemRepository, clan_id: i32) -> Vec<ItemData> {
    match load_container_items(items, clan_id, ItemLocation::ClanWh).await {
        Ok(items) => items,
        Err(e) => {
            warn!("Could not restore container: {e}");
            Vec::new()
        }
    }
}

/// Java `CrestTable.load()`, the database half: the rows of `crests`, only a read (D-39). A
/// database error is logged and the server goes on without crests, like Java.
pub async fn load_crests(pool: &Pool) -> Vec<CrestRecord> {
    match CrestRepository::new(pool.clone()).all().await {
        Ok(rows) => rows
            .into_iter()
            .map(|row| CrestRecord {
                crest_id: row.crest_id,
                data: row.data,
                crest_type: i32::from(row.crest_type),
            })
            .collect(),
        Err(e) => {
            warn!("CrestTable: There was an error while loading crests from database: {e}");
            Vec::new()
        }
    }
}

/// Java's residence managers at startup, the database half (M7-RES-01): the conquerable halls
/// (`CHSiegeManager`), the auctionable halls (`ClanHallTable`), the castles
/// (`CastleManager.loadInstances`, `new Castle(id)`) and the fortresses (`FortManager`), each with
/// the functions of an owned residence. Only reads (D-39). A failed read is logged like Java's and
/// the rest goes on.
pub async fn load_residences(pool: &Pool) -> ResidenceLoad {
    let repo = ResidenceRepository::new(pool.clone());
    let mut load = ResidenceLoad::default();
    match repo.siegable_halls().await {
        Ok(rows) => {
            for row in rows {
                let mut record = siegable_hall_record(row);
                if record.owner_id != 0 {
                    record.functions = load_functions(&repo, FunctionTable::ClanHall, record.id).await;
                }
                load.siegable_halls.push(record);
            }
        }
        Err(e) => warn!("CHSiegeManager: Could not load siegable clan halls!:{e}"),
    }
    match repo.clan_halls().await {
        Ok(rows) => {
            for row in rows {
                let mut record = clan_hall_record(row);
                if record.owner_id != 0 {
                    record.functions = load_functions(&repo, FunctionTable::ClanHall, record.id).await;
                }
                load.clan_halls.push(record);
            }
        }
        Err(e) => warn!("Exception: ClanHallManager.load(): {e}"),
    }
    match repo.castle_ids().await {
        Ok(ids) => {
            for id in ids {
                load.castles.push(load_castle_record(&repo, id).await);
            }
        }
        Err(e) => warn!("Exception: loadCastleData(): {e}"),
    }
    match repo.fort_ids().await {
        Ok(ids) => {
            for id in ids {
                match repo.fort(id).await {
                    Ok(Some(row)) => load.forts.push(load_fort_record(&repo, row).await),
                    Ok(None) => load.forts.push(FortRecord {
                        id,
                        ..FortRecord::default()
                    }),
                    Err(e) => {
                        warn!("Exception: loadFortData(): {e}");
                        load.forts.push(FortRecord {
                            id,
                            ..FortRecord::default()
                        });
                    }
                }
            }
        }
        Err(e) => warn!("Exception: loadFortData(): {e}"),
    }
    load
}

/// Java's siege managers at startup, the database half (M7-SIEGE-01): the registrations
/// (`Siege.loadSiegeClan`), the guards and hired mercenaries (`SiegeGuardManager`,
/// `MercTicketManager`) and the door and trap upgrades (`Castle.loadDoorUpgrade`,
/// `SiegeManager.loadTrapUpgrade`). Only reads (D-39). A failed read is logged like Java's and the
/// rest goes on.
pub async fn load_sieges(pool: &Pool) -> l2_world::SiegeLoad {
    let repo = SiegeRepository::new(pool.clone());
    let mut load = l2_world::SiegeLoad::default();
    match repo.siege_clans().await {
        Ok(rows) => load.siege_clans = rows.into_iter().map(siege_clan_record).collect(),
        Err(e) => warn!("Exception: loadSiegeClan(): {e}"),
    }
    match repo.siege_guards().await {
        Ok(rows) => {
            load.siege_guards = rows
                .into_iter()
                .map(|r| l2_world::SiegeGuardRecord {
                    castle_id: r.castle_id,
                    npc_id: r.npc_id,
                    x: r.x,
                    y: r.y,
                    z: r.z,
                    heading: r.heading,
                    respawn_delay: r.respawn_delay,
                    is_hired: r.is_hired == 1,
                })
                .collect();
        }
        Err(e) => warn!("Exception: loadMercenaryData(): {e}"),
    }
    match repo.door_upgrades().await {
        Ok(rows) => {
            load.door_upgrades = rows
                .into_iter()
                .map(|r| l2_world::siege::DoorUpgradeRecord {
                    door_id: r.door_id,
                    ratio: r.ratio,
                    castle_id: r.castle_id,
                })
                .collect();
        }
        Err(e) => warn!("Exception: loadCastleDoorUpgrade(): {e}"),
    }
    match repo.trap_upgrades().await {
        Ok(rows) => {
            load.trap_upgrades = rows
                .into_iter()
                .map(|r| l2_world::siege::TrapUpgradeRecord {
                    castle_id: r.castle_id,
                    tower_index: r.tower_index,
                    level: r.level,
                })
                .collect();
        }
        Err(e) => warn!("Exception: loadTrapUpgrade(): {e}"),
    }
    load
}

/// Java's fortress constructors and `FortSiege` constructors at startup, the database half
/// (M7-FORT-01): `fort_spawnlist`, `fort_siege_guards` and `fortsiege_clans` of every fortress.
/// Only reads (D-39). A failed read is logged like Java's and that fortress goes without.
pub async fn load_fort_sieges(pool: &Pool) -> FortSiegeLoad {
    let residences = ResidenceRepository::new(pool.clone());
    let repo = FortSiegeRepository::new(pool.clone());
    let mut load = FortSiegeLoad::default();
    let ids = match residences.fort_ids().await {
        Ok(ids) => ids,
        Err(e) => {
            warn!("Exception: loadFortData(): {e}");
            return load;
        }
    };
    for id in ids {
        let mut fort = FortSiegeFortLoad {
            fort_id: id,
            ..FortSiegeFortLoad::default()
        };
        match repo.spawns(id).await {
            Ok(rows) => {
                fort.spawns = rows
                    .into_iter()
                    .map(|r| FortSpawnRecord {
                        id: r.id,
                        npc_id: r.npc_id,
                        x: r.x,
                        y: r.y,
                        z: r.z,
                        heading: r.heading,
                        spawn_type: r.spawn_type,
                        castle_id: r.castle_id,
                    })
                    .collect();
            }
            Err(e) => warn!("Fort {id} initNpcs: Spawn could not be initialized: {e}"),
        }
        match repo.siege_guards(id).await {
            Ok(rows) => {
                fort.siege_guards = rows
                    .into_iter()
                    .map(|r| FortSiegeGuardRecord {
                        npc_id: r.npc_id,
                        x: r.x,
                        y: r.y,
                        z: r.z,
                        heading: r.heading,
                        respawn_delay: r.respawn_delay,
                    })
                    .collect();
            }
            Err(e) => warn!("Error loading siege guard for fort {id}: {e}"),
        }
        match repo.siege_clans(id).await {
            Ok(clans) => fort.siege_clans = clans,
            Err(e) => warn!("Exception: loadSiegeClan(): {e}"),
        }
        load.forts.push(fort);
    }
    load
}

/// A `territories` row for the world.
fn territory_record(row: l2_db::TerritoryRow) -> TerritoryRecord {
    TerritoryRecord {
        territory_id: row.territory_id,
        castle_id: row.castle_id,
        fort_id: row.fort_id,
        owned_ward_ids: row.owned_ward_ids,
    }
}

/// `GlobalVariablesManager.getLong(name, 0)`: no row, an empty value or one that is not a number
/// is 0 (Java throws for the last; the war then uses its own date).
fn parse_global_long(value: Option<&str>) -> i64 {
    match value {
        None => 0,
        Some(text) => text.trim().parse::<i64>().unwrap_or_else(|_| {
            warn!("GlobalVariables: nextTWStartDate is not a number: {text}");
            0
        }),
    }
}

/// Java `TerritoryWarManager.load()` and `GlobalVariablesManager` (`nextTWStartDate`), the
/// database half (M7-TW-01): the spawn list, the territories, the registrations. Only reads
/// (D-39); a failed read is logged like Java's and that part is empty.
pub async fn load_territories(pool: &Pool) -> TerritoryLoad {
    let repo = TerritoryRepository::new(pool.clone());
    let spawns = match repo.spawns().await {
        Ok(rows) => Some(
            rows.into_iter()
                .map(|r| TerritorySpawnRecord {
                    id: r.id,
                    castle_id: r.castle_id,
                    npc_id: r.npc_id,
                    x: r.x,
                    y: r.y,
                    z: r.z,
                    heading: r.heading,
                    spawn_type: r.spawn_type,
                })
                .collect(),
        ),
        Err(e) => {
            warn!("TerritoryWarManager: SpawnList error: {e}");
            None
        }
    };
    let territories = match repo.territories().await {
        Ok(rows) => Some(rows.into_iter().map(territory_record).collect()),
        Err(e) => {
            warn!("TerritoryWarManager: territory list error(): {e}");
            None
        }
    };
    let registrations = match repo.registrations().await {
        Ok(rows) => Some(
            rows.into_iter()
                .map(|r| TerritoryRegistrationRecord {
                    castle_id: r.castle_id,
                    registered_id: r.registered_id,
                })
                .collect(),
        ),
        Err(e) => {
            warn!("TerritoryWarManager: registration list error: {e}");
            None
        }
    };
    let next_tw_start_date = match repo.next_tw_start_date().await {
        Ok(value) => parse_global_long(value.as_deref()),
        Err(e) => {
            warn!("GlobalVariablesManager: could not read nextTWStartDate: {e}");
            0
        }
    };
    TerritoryLoad {
        spawns,
        territories,
        registrations,
        next_tw_start_date,
    }
}

/// Java's conquerable hall scripts at startup, the database half (M7-CHSIEGE-01):
/// `clanhall_siege_attackers`, `clanhall_siege_guards` and, for the scripts that have them, the
/// flag war tables and `rainbowsprings_attacker_list`. Only reads (D-39). A failed read is logged
/// like Java's and that part goes without. Java reads the guards at the first siege; they are read
/// here because the world never waits for the database (D-15).
pub async fn load_hall_sieges(pool: &Pool) -> HallSiegeLoad {
    let repo = HallSiegeRepository::new(pool.clone());
    let mut load = HallSiegeLoad::default();
    for script in HallScript::ALL {
        let id = script.hall_id();
        let mut hall = HallSiegeHallLoad {
            hall_id: id,
            ..HallSiegeHallLoad::default()
        };
        let flag_war = matches!(
            script,
            HallScript::BanditStronghold | HallScript::WildBeastReserve
        );
        let rainbow = script == HallScript::RainbowSpringsChateau;
        if flag_war {
            match repo.flag_war_clans(id).await {
                Ok(rows) => {
                    for row in rows {
                        let members = match repo.flag_war_members(row.clan_id).await {
                            Ok(members) => members,
                            Err(e) => {
                                warn!("{}.loadAttackerMembers()->{e}", script.name());
                                Vec::new()
                            }
                        };
                        hall.flag_war.push(FlagWarRecord {
                            clan_id: row.clan_id,
                            flag: row.flag,
                            npc: row.npc,
                            members,
                        });
                    }
                }
                Err(e) => warn!("{}.loadAttackers()->{e}", script.name()),
            }
        } else if rainbow {
            match repo.rainbow_attackers().await {
                Ok(rows) => {
                    hall.rainbow = rows
                        .into_iter()
                        .map(|r| (r.clan_id, r.war_decrees_count))
                        .collect();
                }
                Err(e) => warn!("{}.loadAttackers()->{e}", script.name()),
            }
        } else {
            match repo.attackers(id).await {
                Ok(attackers) => hall.attackers = attackers,
                Err(e) => warn!("{}: Could not load siege attackers! {e}", script.name()),
            }
        }
        match repo.guards(id).await {
            Ok(rows) => {
                hall.guards = rows
                    .into_iter()
                    .map(|r| HallGuardRecord {
                        npc_id: r.npc_id,
                        x: r.x,
                        y: r.y,
                        z: r.z,
                        heading: r.heading,
                        respawn_delay: r.respawn_delay,
                    })
                    .collect();
            }
            Err(e) => warn!("{}: Could not load siege guards! {e}", script.name()),
        }
        load.halls.push(hall);
    }
    load
}

fn olympiad_noble_row(char_id: i32, noble: &NobleStats) -> l2_db::OlympiadNobleRow {
    l2_db::OlympiadNobleRow {
        char_id,
        class_id: noble.class_id,
        char_name: noble.char_name.clone(),
        olympiad_points: noble.points,
        competitions_done: noble.comp_done,
        competitions_won: noble.comp_won,
        competitions_lost: noble.comp_lost,
        competitions_drawn: noble.comp_drawn,
        competitions_done_week: noble.comp_done_week,
        competitions_done_week_classed: noble.comp_done_week_classed,
        competitions_done_week_non_classed: noble.comp_done_week_non_classed,
        competitions_done_week_team: noble.comp_done_week_team,
    }
}

/// Java `Olympiad.load()`, the database half (M8-OLY-01): the `olympiad_data` row, the nobles
/// and the last period's ranking (`GET_ALL_CLASSIFIED_NOBLESS`, which `loadNoblesRank` reads when
/// the server starts in the validation period). Read in both modes of `CycleDatabaseWrites`
/// (D-53: the state is read again at start). A failed read is logged like Java's and that part
/// starts empty.
pub async fn load_olympiad(pool: &Pool, min_matches: i32) -> OlympiadLoad {
    let repo = l2_db::OlympiadRepository::new(pool.clone());
    let data = match repo.load_data().await {
        Ok(row) => row.map(|r| OlympiadData {
            current_cycle: r.current_cycle,
            period: r.period,
            olympiad_end: r.olympiad_end,
            validation_end: r.validation_end,
            next_weekly_change: r.next_weekly_change,
        }),
        Err(e) => {
            warn!("Olympiad System: Error loading olympiad data from database: {e}");
            None
        }
    };
    let nobles = match repo.load_nobles().await {
        Ok(rows) => rows
            .into_iter()
            .map(|r| NobleRow {
                char_id: r.char_id,
                stats: NobleStats {
                    class_id: r.class_id,
                    char_name: r.char_name,
                    points: r.olympiad_points,
                    comp_done: r.competitions_done,
                    comp_won: r.competitions_won,
                    comp_lost: r.competitions_lost,
                    comp_drawn: r.competitions_drawn,
                    comp_done_week: r.competitions_done_week,
                    comp_done_week_classed: r.competitions_done_week_classed,
                    comp_done_week_non_classed: r.competitions_done_week_non_classed,
                    comp_done_week_team: r.competitions_done_week_team,
                    to_save: false,
                },
            })
            .collect(),
        Err(e) => {
            warn!("Olympiad System: Error loading noblesse data from database: {e}");
            Vec::new()
        }
    };
    let classified = match repo.classified_nobles(min_matches).await {
        Ok(ids) => ids,
        Err(e) => {
            warn!("Olympiad System: Error loading noblesse data from database for Ranking: {e}");
            Vec::new()
        }
    };
    OlympiadLoad {
        data,
        nobles,
        classified,
    }
}

fn cursed_db_row(row: &CursedRow) -> CursedWeaponRow {
    CursedWeaponRow {
        item_id: row.item_id,
        char_id: row.char_id,
        player_karma: row.player_karma,
        player_pk_kills: row.player_pk_kills,
        nb_kills: row.nb_kills,
        end_time: row.end_time,
    }
}

/// Java `Hero.init()`, the database half (M8-HERO-01): every `heroes` row with the clan of the
/// character, and the message, the diary and the match history of the heroes of the last period
/// (`before` is [`l2_world::hero_fights_cutoff_ms`]). Read in both modes of `CycleDatabaseWrites`
/// (D-53: the state is read again at start). A failed read is logged like Java's and starts empty.
pub async fn load_heroes(pool: &Pool, before: i64) -> l2_world::HeroLoad {
    match read_heroes(&l2_db::HeroRepository::new(pool.clone()), before, None).await {
        Ok(load) => load,
        Err(e) => {
            warn!("Hero System: Could not load Heroes: {e}");
            l2_world::HeroLoad::default()
        }
    }
}

/// The reads of `Hero.init` (`only` `None`) or of `loadFights` and `loadDiary` of one hero.
async fn read_heroes(
    repo: &l2_db::HeroRepository,
    before: i64,
    only: Option<i32>,
) -> Result<l2_world::HeroLoad, l2_db::DbError> {
    let mut rows = Vec::new();
    let mut ids = Vec::new();
    match only {
        Some(char_id) => ids.push(char_id),
        None => {
            for row in repo.all_heroes().await? {
                if row.played == 1 {
                    ids.push(row.char_id);
                }
                rows.push(l2_world::HeroRow {
                    char_id: row.char_id,
                    char_name: row.char_name,
                    class_id: row.class_id,
                    count: row.count,
                    played: row.played,
                    claimed: row.claimed,
                    clan_id: row.clan_id,
                    ally_id: row.ally_id,
                });
            }
        }
    }
    let mut names: std::collections::HashMap<i32, Option<String>> = std::collections::HashMap::new();
    let mut details = Vec::with_capacity(ids.len());
    for char_id in ids {
        let message = repo.message(char_id).await?;
        let diary = repo
            .diary(char_id)
            .await?
            .into_iter()
            .map(|d| l2_world::HeroDiaryRow {
                time: d.time,
                action: d.action,
                param: d.param,
            })
            .collect();
        let mut fights = Vec::new();
        for f in repo.fights(char_id, before).await? {
            // `CharInfoTable.getNameById` of both fighters.
            for id in [f.char_one_id, f.char_two_id] {
                if let std::collections::hash_map::Entry::Vacant(e) = names.entry(id) {
                    let name = repo.char_name(id).await?;
                    e.insert(name);
                }
            }
            fights.push(l2_world::HeroFightRow {
                char_one_id: f.char_one_id,
                char_one_class: f.char_one_class,
                char_one_name: names.get(&f.char_one_id).cloned().flatten(),
                char_two_id: f.char_two_id,
                char_two_class: f.char_two_class,
                char_two_name: names.get(&f.char_two_id).cloned().flatten(),
                winner: f.winner,
                start: f.start,
                time: f.time,
                classed: f.classed,
            });
        }
        details.push(l2_world::HeroDetails {
            char_id,
            message,
            diary,
            fights,
        });
    }
    Ok(l2_world::HeroLoad { rows, details })
}

/// Java `AirShipManager.load()`: the rows of `airships` (M8-VEHICLE-01).
pub async fn load_airships(pool: &Pool) -> VehicleLoad {
    match AirshipRepository::new(pool.clone()).all().await {
        Ok(rows) => VehicleLoad {
            airships: rows
                .into_iter()
                .map(|r| AirshipRow {
                    owner_id: r.owner_id,
                    fuel: r.fuel,
                })
                .collect(),
        },
        Err(e) => {
            warn!("AirShipManager: Could not load airships table: {e}");
            VehicleLoad::default()
        }
    }
}

/// Java `UndergroundColiseumManager.load()` and `restoreBestTeams()`, the database half
/// (M8-GAMES-01): `UC_START_TIME` and `UC_STOP_TIME` of `global_variables`, and the rows of
/// `underground_coliseum`. A failed read is logged like Java's and reads as nothing.
pub async fn load_minigames(pool: &Pool) -> MinigameLoad {
    let repo = UndergroundColiseumRepository::new(pool.clone());
    let uc_start_ms = repo.variable_long("UC_START_TIME").await.unwrap_or_else(|e| {
        warn!("Could not read UC_START_TIME: {e}");
        0
    });
    let uc_stop_ms = repo.variable_long("UC_STOP_TIME").await.unwrap_or_else(|e| {
        warn!("Could not read UC_STOP_TIME: {e}");
        0
    });
    let best_teams = match repo.best_teams().await {
        Ok(rows) => rows.into_iter().map(|r| (r.arena_id, r.leader, r.wins)).collect(),
        Err(e) => {
            warn!("UndergroundColiseumManager: Could not load underground_coliseum table ({e})");
            Vec::new()
        }
    };
    MinigameLoad {
        uc_start_ms,
        uc_stop_ms,
        best_teams,
    }
}

/// Java `CursedWeaponsManager.restore()` and `controlPlayers()`, the database half (M8-CURSED-01):
/// the rows of `cursed_weapons`, and the cleanup of a cursed weapon item a character holds
/// without a row. Called only with `CycleDatabaseWrites` (D-53): without it the table is the
/// Java game server's and nothing is read or written.
///
/// A failed read is logged like Java's and the weapons start as not in the world.
pub async fn load_cursed(pool: &Pool, defs: &l2_data::CursedWeaponDefs) -> CursedLoad {
    let repo = CursedWeaponRepository::new(pool.clone());
    let rows = match repo.all().await {
        Ok(rows) => rows,
        Err(e) => {
            warn!("Could not restore CursedWeapons data: {e}");
            return CursedLoad::default();
        }
    };
    // `controlPlayers()`: a weapon no row holds must not be in anybody's inventory. (A weapon with
    // a row is the world's: it re-activates it, or ends it when its time is up.)
    for def in defs.iter() {
        if rows.iter().any(|row| row.item_id == def.item_id) {
            continue;
        }
        let item_id = def.item_id;
        let result: Result<(), l2_db::DbError> = async {
            let Some(owner) = repo.item_owner(item_id).await? else {
                return Ok(());
            };
            // A player has the cursed weapon in his inventory ...
            info!("PROBLEM : Player {owner} owns the cursed weapon {item_id} but he shouldn't.");
            // Delete the item.
            if repo.delete_owner_item(owner, item_id).await? != 1 {
                warn!("Error while deleting cursed weapon {item_id} from userId {owner}");
            }
            // Restore the player's old karma and pk count.
            // NOTE(java-quirk): the weapon has no row, so its old karma and pk count are 0.
            if repo.restore_karma(owner, 0, 0).await? != 1 {
                warn!("Error while updating karma & pkkills for userId 0");
            }
            // Clean up the cursed weapons table.
            repo.remove(item_id).await
        }
        .await;
        if let Err(e) = result {
            warn!("Could not check CursedWeapons data: {e}");
            break;
        }
    }
    CursedLoad {
        rows: rows
            .into_iter()
            .map(|row| CursedRow {
                item_id: row.item_id,
                char_id: row.char_id,
                player_karma: row.player_karma,
                player_pk_kills: row.player_pk_kills,
                nb_kills: row.nb_kills,
                end_time: row.end_time,
            })
            .collect(),
    }
}

/// Java `CastleManorManager.loadDb`, the database half (M7-MANOR-01): the seeds the castles sell
/// and the crops they buy, both periods, for every castle of the `castle` table. Only reads
/// (D-39). A failed read is logged like Java's ("Unable to load manor data!") and the manor starts
/// with empty lists.
pub async fn load_manor(pool: &Pool) -> ManorLoad {
    let residences = ResidenceRepository::new(pool.clone());
    let repo = ManorRepository::new(pool.clone());
    let mut load = ManorLoad::default();
    let result: Result<(), l2_db::DbError> = async {
        for castle_id in residences.castle_ids().await? {
            for row in repo.production(castle_id).await? {
                load.production.push(ManorProductionRow {
                    castle_id: row.castle_id,
                    seed_id: row.seed_id,
                    amount: row.amount,
                    start_amount: row.start_amount,
                    price: row.price,
                    next_period: row.next_period,
                });
            }
            for row in repo.procure(castle_id).await? {
                load.procure.push(ManorProcureRow {
                    castle_id: row.castle_id,
                    crop_id: row.crop_id,
                    amount: row.amount,
                    start_amount: row.start_amount,
                    price: row.price,
                    reward_type: row.reward_type,
                    next_period: row.next_period,
                });
            }
        }
        Ok(())
    }
    .await;
    if let Err(e) = result {
        warn!("CastleManorManager: Unable to load manor data!: {e}");
        return ManorLoad::default();
    }
    load
}

fn siege_clan_record(row: l2_db::SiegeClanRow) -> l2_world::SiegeClanRecord {
    l2_world::SiegeClanRecord {
        castle_id: row.castle_id,
        clan_id: row.clan_id,
        kind: row.kind,
    }
}

/// The siege statements of `PersistCommand::Residence` (M7-SIEGE-01).
async fn store_siege_write(repo: &SiegeRepository, write: &ResidenceWrite) -> Result<(), l2_db::DbError> {
    match write {
        ResidenceWrite::CastleSiegeDate {
            castle_id,
            siege_date,
            reg_time_end,
            reg_time_over,
        } => {
            repo.update_castle_siege_date(*castle_id, *siege_date, *reg_time_end, *reg_time_over)
                .await
        }
        ResidenceWrite::SiegeClanInsert {
            clan_id,
            castle_id,
            kind,
        } => repo.insert_siege_clan(*clan_id, *castle_id, *kind).await,
        ResidenceWrite::SiegeClanUpdate {
            castle_id,
            clan_id,
            kind,
        } => repo.update_siege_clan(*castle_id, *clan_id, *kind).await,
        ResidenceWrite::SiegeClanDelete { castle_id, clan_id } => {
            repo.delete_siege_clan(*castle_id, *clan_id).await
        }
        ResidenceWrite::SiegeClansDeleteCastle { castle_id } => {
            repo.delete_siege_clans_of_castle(*castle_id).await
        }
        ResidenceWrite::SiegeClansDeleteClan { clan_id } => repo.delete_siege_clans_of_clan(*clan_id).await,
        ResidenceWrite::SiegeClansDeleteWaiting { castle_id } => {
            repo.delete_waiting_siege_clans(*castle_id).await
        }
        ResidenceWrite::SiegeGuardInsert(guard) => {
            let new = l2_db::NewSiegeGuard {
                castle_id: guard.castle_id,
                npc_id: guard.npc_id,
                x: guard.x,
                y: guard.y,
                z: guard.z,
                heading: guard.heading,
                respawn_delay: guard.respawn_delay,
                is_hired: i32::from(guard.is_hired),
            };
            repo.insert_siege_guard(&new).await
        }
        ResidenceWrite::SiegeGuardDeleteHired { npc_id, x, y, z } => {
            repo.delete_hired_guard(*npc_id, *x, *y, *z).await
        }
        ResidenceWrite::SiegeGuardsDeleteHired { castle_id } => repo.delete_hired_guards(*castle_id).await,
        ResidenceWrite::CastleDoorUpgrade {
            door_id,
            ratio,
            castle_id,
        } => repo.replace_door_upgrade(*door_id, *ratio, *castle_id).await,
        ResidenceWrite::CastleDoorUpgradesDelete { castle_id } => repo.delete_door_upgrades(*castle_id).await,
        ResidenceWrite::CastleTrapUpgrade {
            castle_id,
            tower_index,
            level,
        } => repo.replace_trap_upgrade(*castle_id, *tower_index, *level).await,
        ResidenceWrite::CastleTrapUpgradesDelete { castle_id } => repo.delete_trap_upgrades(*castle_id).await,
        ResidenceWrite::ClanBloodAllianceCount { clan_id, count } => {
            repo.update_blood_alliance_count(*clan_id, *count).await
        }
        // The residence statements are `store_residence_write`'s.
        _ => Ok(()),
    }
}

fn production_row(row: &ManorProductionRow) -> ProductionRow {
    ProductionRow {
        castle_id: row.castle_id,
        seed_id: row.seed_id,
        amount: row.amount,
        start_amount: row.start_amount,
        price: row.price,
        next_period: row.next_period,
    }
}

fn procure_row(row: &ManorProcureRow) -> ProcureRow {
    ProcureRow {
        castle_id: row.castle_id,
        crop_id: row.crop_id,
        amount: row.amount,
        start_amount: row.start_amount,
        price: row.price,
        reward_type: row.reward_type,
        next_period: row.next_period,
    }
}

/// The residences of one clan as the database has them now (D-50, D-51): the castle its
/// `hasCastle` names (`castle_id`, read at `CharacterSelect` with the clan) and the fortresses and
/// halls whose owner it is, each with its functions.
pub async fn load_residence_refresh(
    repo: &ResidenceRepository,
    clan_id: i32,
    castle_id: i32,
) -> Result<ResidenceRefresh, l2_db::DbError> {
    let mut refresh = ResidenceRefresh {
        clan_id,
        castle_id,
        ..ResidenceRefresh::default()
    };
    if castle_id > 0 {
        refresh.castle = Some(read_castle_record(repo, castle_id).await?);
    }
    for row in repo.forts_of_owner(clan_id).await? {
        let mut record = fort_record(row);
        record.functions = function_records(repo.functions(FunctionTable::Fort, record.id).await?);
        refresh.forts.push(record);
    }
    for row in repo.clan_halls_of_owner(clan_id).await? {
        let mut record = clan_hall_record(row);
        record.functions = function_records(repo.functions(FunctionTable::ClanHall, record.id).await?);
        refresh.clan_halls.push(record);
    }
    for row in repo.siegable_halls_of_owner(clan_id).await? {
        let mut record = siegable_hall_record(row);
        record.functions = function_records(repo.functions(FunctionTable::ClanHall, record.id).await?);
        refresh.siegable_halls.push(record);
    }
    Ok(refresh)
}

/// Java `new Castle(id)`'s reads with Java's error handling: a failed row read is logged and the
/// castle keeps its defaults, failed functions are logged and the castle has none.
async fn load_castle_record(repo: &ResidenceRepository, id: i32) -> CastleRecord {
    let mut record = CastleRecord {
        id,
        time_registration_over: true,
        ..CastleRecord::default()
    };
    match repo.castle(id).await {
        Ok(Some(row)) => apply_castle_row(&mut record, row),
        Ok(None) => {}
        Err(e) => warn!("Exception: loadCastleData(): {e}"),
    }
    match repo.castle_owner_ids(id).await {
        Ok(owners) => record.owner_id = owners.last().copied().unwrap_or(0),
        Err(e) => warn!("Exception: loadCastleData(): {e}"),
    }
    if record.owner_id != 0 {
        record.functions = load_functions(repo, FunctionTable::Castle, id).await;
    }
    record
}

/// A castle read again: any failure fails the read (D-50).
async fn read_castle_record(repo: &ResidenceRepository, id: i32) -> Result<CastleRecord, l2_db::DbError> {
    let mut record = CastleRecord {
        id,
        time_registration_over: true,
        ..CastleRecord::default()
    };
    if let Some(row) = repo.castle(id).await? {
        apply_castle_row(&mut record, row);
    }
    record.owner_id = repo.castle_owner_ids(id).await?.last().copied().unwrap_or(0);
    if record.owner_id != 0 {
        record.functions = function_records(repo.functions(FunctionTable::Castle, id).await?);
        // D-50: the owner's proofs of alliance, which the vault keeper hands out.
        record.owner_blood_alliance_count = repo.blood_alliance_count(record.owner_id).await?;
    }
    Ok(record)
}

fn apply_castle_row(record: &mut CastleRecord, row: l2_db::CastleRow) {
    record.found = true;
    record.name = row.name;
    record.tax_percent = row.tax_percent;
    record.treasury = row.treasury;
    record.siege_date = row.siege_date;
    // `Boolean.parseBoolean(rs.getString(...))`.
    record.time_registration_over = row.reg_time_over.eq_ignore_ascii_case("true");
    record.time_registration_over_date = row.reg_time_end;
    record.show_npc_crest = row.show_npc_crest.eq_ignore_ascii_case("true");
    record.ticket_buy_count = row.ticket_buy_count;
}

/// Java `new Fort(id)`'s reads: the functions of an owned fortress.
async fn load_fort_record(repo: &ResidenceRepository, row: l2_db::FortRow) -> FortRecord {
    let mut record = fort_record(row);
    if record.owner_id > 0 {
        record.functions = load_functions(repo, FunctionTable::Fort, record.id).await;
    }
    record
}

fn fort_record(row: l2_db::FortRow) -> FortRecord {
    FortRecord {
        id: row.id,
        name: row.name,
        siege_date: row.siege_date,
        last_owned_time: row.last_owned_time,
        owner_id: row.owner,
        fort_type: row.fort_type,
        state: row.state,
        castle_id: row.castle_id,
        supply_level: row.supply_level,
        functions: Vec::new(),
    }
}

fn clan_hall_record(row: l2_db::ClanHallRow) -> ClanHallRecord {
    ClanHallRecord {
        id: row.id,
        name: row.name,
        owner_id: row.owner_id,
        lease: row.lease,
        desc: row.desc,
        location: row.location,
        paid_until: row.paid_until,
        grade: row.grade,
        paid: row.paid,
        functions: Vec::new(),
    }
}

/// `getInt` / `getLong` of NULL is 0; `getString` of NULL is `null` (an empty name here).
fn siegable_hall_record(row: l2_db::SiegableHallRow) -> SiegableHallRecord {
    SiegableHallRecord {
        id: row.clan_hall_id,
        name: row.name.unwrap_or_default(),
        owner_id: row.owner_id.unwrap_or(0),
        desc: row.desc.unwrap_or_default(),
        location: row.location.unwrap_or_default(),
        next_siege: row.next_siege.unwrap_or(0),
        siege_length: i64::from(row.siege_length.unwrap_or(0)),
        schedule_config: row.schedule_config,
        functions: Vec::new(),
    }
}

fn function_records(rows: Vec<l2_db::FunctionRow>) -> Vec<FunctionRecord> {
    rows.into_iter()
        .map(|r| FunctionRecord {
            function_type: r.function_type,
            lvl: r.lvl,
            lease: r.lease,
            rate: r.rate,
            end_time: r.end_time,
        })
        .collect()
}

/// Java `loadFunctions()`: a failed read is logged ("Exception: Castle.loadFunctions(): ...") and
/// the residence has no functions.
async fn load_functions(repo: &ResidenceRepository, table: FunctionTable, id: i32) -> Vec<FunctionRecord> {
    match repo.functions(table, id).await {
        Ok(rows) => function_records(rows),
        Err(e) => {
            error!("Exception: {table:?}.loadFunctions(): {e}");
            Vec::new()
        }
    }
}

/// A write of the clan hall auction tables (M7-CH-01).
async fn store_auction_write(repo: &AuctionRepository, write: &AuctionWrite) -> Result<(), PersistError> {
    match write {
        AuctionWrite::EndDate { id, end_date } => repo.update_end_date(*id, *end_date).await?,
        AuctionWrite::BidUpdate {
            auction_id,
            bidder_id,
            bidder_name,
            max_bid,
            time_bid,
        } => {
            repo.update_bid(*auction_id, *bidder_id, bidder_name, *max_bid, *time_bid)
                .await?
        }
        AuctionWrite::BidInsert {
            id,
            auction_id,
            bidder_id,
            bidder_name,
            max_bid,
            clan_name,
            time_bid,
        } => {
            repo.insert_bid(
                *id,
                *auction_id,
                *bidder_id,
                bidder_name,
                *max_bid,
                clan_name,
                *time_bid,
            )
            .await?
        }
        AuctionWrite::BidsDelete { auction_id } => repo.delete_bids(*auction_id).await?,
        AuctionWrite::AuctionDelete { item_id } => repo.delete_auction(*item_id).await?,
        AuctionWrite::BidDelete {
            auction_id,
            bidder_id,
        } => repo.delete_bid(*auction_id, *bidder_id).await?,
        AuctionWrite::AuctionInsert(record) => {
            repo.insert_auction(&l2_db::NewAuction {
                id: record.id,
                seller_id: record.seller_id,
                seller_name: record.seller_name.clone(),
                seller_clan_name: record.seller_clan_name.clone(),
                item_type: record.item_type.clone(),
                item_id: record.item_id,
                item_object_id: record.item_object_id,
                item_name: record.item_name.clone(),
                // `_itemQuantity` is a constant 0 in Java.
                item_quantity: 0,
                starting_bid: record.starting_bid,
                current_bid: record.current_bid,
                end_date: record.end_date,
            })
            .await?
        }
        AuctionWrite::NpcAuctionInsert {
            hall_id,
            name,
            starting_bid,
            end_date,
        } => {
            repo.insert_npc_auction(*hall_id, name, *starting_bid, *end_date)
                .await?
        }
        AuctionWrite::ClanBidAt { clan_id, auction_id } => {
            repo.update_clan_auction_bid_at(*clan_id, *auction_id).await?
        }
    }
    Ok(())
}

/// The clan hall auction rows (`ClanHallAuctionManager.load()`: every `auction` id, each read like
/// `new ClanHallAuction(id)` with its bids). Only reads (D-39). A failed read is logged like
/// Java's.
pub async fn load_clan_hall_auctions(pool: &Pool) -> Vec<AuctionRecord> {
    let repo = AuctionRepository::new(pool.clone());
    let mut records = Vec::new();
    match repo.auction_ids().await {
        Ok(ids) => {
            for id in ids {
                match read_auction_record(&repo, id).await {
                    Ok(Some(record)) => records.push(record),
                    // `load()` finds no row: the auction keeps its start values.
                    Ok(None) => records.push(AuctionRecord {
                        id,
                        ..AuctionRecord::default()
                    }),
                    Err(e) => warn!("Exception: Auction.load(): {e}"),
                }
            }
        }
        Err(e) => warn!("ClanHallAuctionManager: Exception: AuctionManager.load(): {e}"),
    }
    records
}

/// One `auction` row with its bids (`ClanHallAuction.load` and `loadBid`).
async fn read_auction_record(
    repo: &AuctionRepository,
    id: i32,
) -> Result<Option<AuctionRecord>, l2_db::DbError> {
    let Some(row) = repo.auction(id).await? else {
        return Ok(None);
    };
    let bidders = repo
        .bids(id)
        .await?
        .into_iter()
        .map(|b| AuctionBidRecord {
            bidder_id: b.bidder_id,
            bidder_name: b.bidder_name,
            max_bid: b.max_bid,
            clan_name: b.clan_name,
            time_bid: b.time_bid,
        })
        .collect();
    Ok(Some(AuctionRecord {
        id,
        seller_id: row.seller_id,
        seller_name: row.seller_name,
        seller_clan_name: row.seller_clan_name,
        item_type: row.item_type,
        item_id: row.item_id,
        item_object_id: row.item_object_id,
        item_name: row.item_name,
        starting_bid: row.starting_bid,
        current_bid: row.current_bid,
        end_date: row.end_date,
        bidders,
    }))
}

/// The function table of a residence.
fn function_table(residence: ResidenceRef) -> FunctionTable {
    match residence {
        ResidenceRef::Castle(_) => FunctionTable::Castle,
        ResidenceRef::Fort(_) => FunctionTable::Fort,
        ResidenceRef::ClanHall(_) => FunctionTable::ClanHall,
    }
}

/// The rows of one clan (`Clan.restore`, `restoreSubPledges`, `restoreRankPrivs`,
/// `restoreSkills`, `restoreNotice`).
pub async fn load_clan(repo: &ClanRepository, clan_id: i32) -> Result<ClanRecord, l2_db::DbError> {
    let mut record = ClanRecord {
        clan_id,
        ..ClanRecord::default()
    };
    if let Some(row) = repo.clan_data(clan_id).await? {
        let int = |v: Option<i32>| v.unwrap_or(0);
        record.data = Some(ClanDataRecord {
            name: row.clan_name,
            level: int(row.clan_level),
            reputation_score: row.reputation_score,
            castle_id: int(row.has_castle),
            blood_alliance_count: i32::from(row.blood_alliance_count),
            blood_oath_count: i32::from(row.blood_oath_count),
            ally_id: int(row.ally_id),
            ally_name: row.ally_name,
            leader_id: int(row.leader_id),
            crest_id: int(row.crest_id),
            crest_large_id: int(row.crest_large_id),
            ally_crest_id: int(row.ally_crest_id),
            auction_bid_at: row.auction_bid_at,
            ally_penalty_expiry_time: clamp_i64(row.ally_penalty_expiry_time),
            ally_penalty_type: i32::from(row.ally_penalty_type),
            char_penalty_expiry_time: clamp_i64(row.char_penalty_expiry_time),
            dissolving_expiry_time: clamp_i64(row.dissolving_expiry_time),
            // `getInt` of an unsigned `INT(10)`: the low 32 bits.
            new_leader_id: row.new_leader_id as i32,
        });
        record.members = repo
            .clan_members(clan_id)
            .await?
            .into_iter()
            .map(|m| ClanMemberRecord {
                object_id: clamp_i32(u64::from(m.char_id)),
                name: m.char_name,
                level: m.level.map_or(0, i32::from),
                class_id: m.classid.map_or(0, i32::from),
                // `getString("title")` may be null; it is written as an empty string.
                title: m.title.unwrap_or_default(),
                power_grade: m.power_grade.map_or(0, i32::from),
                pledge_type: i32::from(m.subpledge),
                apprentice: clamp_i32(u64::from(m.apprentice)),
                sponsor: clamp_i32(u64::from(m.sponsor)),
                female: m.sex.is_some_and(|s| s != 0),
                race: m.race.map_or(0, i32::from),
            })
            .collect();
    }
    record.sub_pledges = repo
        .sub_pledges(clan_id)
        .await?
        .into_iter()
        .map(|r| (r.sub_pledge_id, r.name.unwrap_or_default(), r.leader_id))
        .collect();
    record.rank_privs = repo
        .rank_privs(clan_id)
        .await?
        .into_iter()
        .map(|r| (r.rank, r.party, r.privs))
        .collect();
    record.skills = repo
        .clan_skills(clan_id)
        .await?
        .into_iter()
        .map(|r| (r.skill_id, r.skill_level, r.sub_pledge_id))
        .collect();
    record.notices = repo
        .clan_notices(clan_id)
        .await?
        .into_iter()
        .map(|r| (r.enabled, r.notice))
        .collect();
    Ok(record)
}

/// The Java method a clan write comes from, for the log.
fn clan_write_name(write: &ClanWrite) -> &'static str {
    match write {
        ClanWrite::Insert(_) => "Clan.store",
        ClanWrite::Update(_) => "Clan.updateClanInDB",
        ClanWrite::Level { .. } => "Clan.changeLevel",
        ClanWrite::AllyCrestOfClan { .. } => "Clan.changeAllyCrest",
        ClanWrite::Crest(CrestWrite::Insert { .. }) => "CrestTable.createCrest",
        ClanWrite::Crest(CrestWrite::Delete { .. }) => "CrestTable.removeCrest",
        ClanWrite::Crest(CrestWrite::ClanCrest { .. }) => "Clan.changeClanCrest",
        ClanWrite::Crest(CrestWrite::ClanLargeCrest { .. }) => "Clan.changeLargeCrest",
        ClanWrite::Crest(CrestWrite::AllyCrestOfClan { .. } | CrestWrite::AllyCrestOfAlly { .. }) => {
            "Clan.changeAllyCrest"
        }
        ClanWrite::War(WarWrite::Store { .. }) => "ClanTable.storeClanWars",
        ClanWrite::War(WarWrite::Delete { .. }) => "ClanTable.deleteClanWars",
        ClanWrite::RemoveMember { .. } => "Clan.removeMemberInDatabase",
        ClanWrite::CharClanPrivs { .. } => "Clan.setNewLeader",
        ClanWrite::CharSubPledge { .. } => "ClanMember.updatePledgeType",
        ClanWrite::CharPowerGrade { .. } => "ClanMember.updatePowerGrade",
        ClanWrite::CharApprenticeSponsor { .. } => "ClanMember.saveApprenticeAndSponsor",
        ClanWrite::InsertSubPledge { .. } => "Clan.createSubPledge",
        ClanWrite::UpdateSubPledge { .. } => "Clan.updateSubPledgeInDB",
        ClanWrite::RankPrivs { .. } => "Clan.setRankPrivs",
        ClanWrite::Notice { .. } => "Clan.storeNotice",
        ClanWrite::Delete { .. } => "ClanTable.destroyClan",
        ClanWrite::UpdateSkill { .. } | ClanWrite::InsertSkill { .. } => "Clan.addNewSkill",
    }
}

/// `INSERT_CLAN_DATA`'s values.
fn new_clan_of(row: &ClanRow) -> l2_db::NewClan {
    l2_db::NewClan {
        clan_id: row.clan_id,
        clan_name: row.name.clone(),
        clan_level: row.level,
        castle_id: row.castle_id,
        blood_alliance_count: row.blood_alliance_count,
        blood_oath_count: row.blood_oath_count,
        ally_id: row.ally_id,
        ally_name: row.ally_name.clone(),
        leader_id: row.leader_id,
        crest_id: row.crest_id,
        crest_large_id: row.crest_large_id,
        ally_crest_id: row.ally_crest_id,
        new_leader_id: row.new_leader_id,
    }
}

/// `updateClanInDB`'s values.
fn clan_update_of(row: &ClanUpdateRow) -> ClanUpdate {
    ClanUpdate {
        clan_id: row.clan_id,
        leader_id: row.leader_id,
        ally_id: row.ally_id,
        ally_name: row.ally_name.clone(),
        reputation_score: row.reputation_score,
        ally_penalty_expiry_time: row.ally_penalty_expiry_time,
        ally_penalty_type: row.ally_penalty_type,
        char_penalty_expiry_time: row.char_penalty_expiry_time,
        dissolving_expiry_time: row.dissolving_expiry_time,
        new_leader_id: row.new_leader_id,
    }
}

/// Java `CharInfoTable`'s constructor, the database half: the id and name of every character.
pub async fn load_char_names(pool: &Pool) -> Vec<(i32, String)> {
    match CharacterRepository::new(pool.clone()).all_names().await {
        Ok(rows) => rows
            .into_iter()
            .map(|row| (i32::try_from(row.char_id).unwrap_or(0), row.char_name))
            .collect(),
        Err(e) => {
            warn!("CharInfoTable: Failed to load character names: {e}");
            Vec::new()
        }
    }
}

/// Java `ItemsOnGroundManager`'s constructor, the database half (`load()`): what is in
/// `itemsonground` when the server starts. The world puts the rows back
/// (`World::load_ground_items`).
///
/// D-39: exactly as Java, nothing touches the table unless the owner turned `SaveDroppedItem`
/// (read, and rewritten later by the world's periodic save) or `ClearDroppedItemTable` (emptied
/// when `SaveDroppedItem` is off) on. A database error is logged and the server goes on without
/// the items, like Java.
pub async fn restore_ground_items(pool: &Pool, general: &GeneralConfig) -> Vec<GroundItemRow> {
    let repo = GroundItemRepository::new(pool.clone());
    // If SaveDroppedItem is false, may want to delete all items previously stored to avoid add old
    // items on reactivate.
    if !general.save_dropped_item && general.clear_dropped_item_table {
        if let Err(e) = repo.empty_table().await {
            error!("ItemsOnGroundManager: Error while cleaning table ItemsOnGround {e}");
        }
    }
    if !general.save_dropped_item {
        return Vec::new();
    }

    // If DestroyPlayerDroppedItem was previously false, items currently protected will be added to
    // ItemsAutoDestroy.
    if general.destroy_dropped_player_item {
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        if let Err(e) = repo
            .recycle_protected(now_ms, general.destroy_equipable_player_item)
            .await
        {
            error!("ItemsOnGroundManager: Error while updating table ItemsOnGround {e}");
        }
    }

    // Add items to world.
    let rows = match repo.load_all().await {
        Ok(records) => records
            .into_iter()
            .map(|r| GroundItemRow {
                object_id: ObjectId(r.object_id),
                item_id: r.item_id,
                count: r.count,
                enchant_level: r.enchant_level,
                x: r.x,
                y: r.y,
                z: r.z,
                drop_time: r.drop_time,
                equipable: r.equipable,
            })
            .collect(),
        Err(e) => {
            error!("ItemsOnGroundManager: Error while loading ItemsOnGround {e}");
            Vec::new()
        }
    };

    if general.empty_dropped_item_table_after_load {
        if let Err(e) = repo.empty_table().await {
            error!("ItemsOnGroundManager: Error while cleaning table ItemsOnGround {e}");
        }
    }
    rows
}

/// Java `GeneralConfig.CHAR_DATA_STORE_INTERVAL`: `CharacterDataStoreInterval` (minutes,
/// default 15) in `General.ini`. The generated config keeps this key as a placeholder, so it
/// is read here, with Java's default when the file or key is missing.
///
/// Java checks the players once a second, so anything below a second behaves like a second.
pub fn autosave_interval(config_dir: &std::path::Path) -> Duration {
    let minutes = match IniFile::load(config_dir.join(GeneralConfig::FILE)) {
        Ok(ini) => ini.get_int("CharacterDataStoreInterval", 15),
        Err(e) => {
            warn!(
                "Cannot read {}: {e}; autosave uses the default of 15 minutes.",
                GeneralConfig::FILE
            );
            15
        }
    };
    let millis = i64::from(minutes).saturating_mul(60_000).max(1000);
    Duration::from_millis(u64::try_from(millis).unwrap_or(1000))
}

/// [`autosave_interval`] from the loaded configuration (`manual.core.general.char_data_store_interval`,
/// Java's `CHAR_DATA_STORE_INTERVAL` in milliseconds).
pub fn autosave_interval_from_config(config: &l2_config::GameConfig) -> Duration {
    let millis = i64::from(config.manual.core.general.char_data_store_interval).max(1000);
    Duration::from_millis(u64::try_from(millis).unwrap_or(1000))
}

/// Java `Item.restoreAttributes` for the items of one owner: the augmentation and the element
/// attributes the `item_attributes` and `item_elementals` rows give them. Java reads the two
/// tables one item at a time; this reads them once for the owner (same rows, same order).
// Java: Item.restoreAttributes
pub async fn restore_item_attributes<'a>(
    repo: &ItemRepository,
    owner_id: i32,
    items: impl IntoIterator<Item = &'a mut ItemData>,
) -> Result<(), l2_db::DbError> {
    let mut items: Vec<&mut ItemData> = items.into_iter().collect();
    if items.is_empty() {
        return Ok(());
    }
    let augmentations = repo.augmentations_of_owner(owner_id).await?;
    let elementals = repo.elementals_of_owner(owner_id).await?;
    apply_item_attribute_rows(&mut items, &augmentations, &elementals);
    Ok(())
}

/// Java `ItemContainer.restore()`: the items one owner has in one location (`SELECT ... FROM items
/// WHERE owner_id=? AND loc=?`), with their augmentation and elements. The private warehouse
/// (`WAREHOUSE`), the freight (`FREIGHT`) and the clan warehouse (`CLANWH`, the owner is the clan)
/// read their items this way (M6-WH-01).
// Java: ItemContainer.restore, Item.restoreFromDb
pub async fn load_container_items(
    repo: &ItemRepository,
    owner_id: i32,
    location: ItemLocation,
) -> Result<Vec<ItemData>, l2_db::DbError> {
    let mut items: Vec<ItemData> = repo
        .items_of_location(owner_id, location)
        .await?
        .into_iter()
        .map(|row| ItemData {
            object_id: ObjectId(row.object_id),
            item_id: row.item_id.unwrap_or(0),
            count: i64::try_from(row.count).unwrap_or(i64::MAX),
            enchant_level: row.enchant_level.unwrap_or(0),
            loc: row.loc.clone().unwrap_or_default(),
            loc_data: row.loc_data.unwrap_or(0),
            custom_type1: row.custom_type1.unwrap_or(0),
            custom_type2: row.custom_type2.unwrap_or(0),
            mana_left: row.mana_left,
            time: row.time,
            ..Default::default()
        })
        .collect();
    // Java catches a failed read of the attributes and logs it; the items then have none.
    if let Err(e) = restore_item_attributes(repo, owner_id, items.iter_mut()).await {
        warn!("Could not restore augmentation and elemental data for the items of {owner_id}: {e}");
    }
    Ok(items)
}

/// The rule part of [`restore_item_attributes`]: the rows of the augmentation `-1` and of an
/// element with type or value `-1` are skipped (the table's "nothing" default).
fn apply_item_attribute_rows(
    items: &mut [&mut ItemData],
    augmentations: &[l2_db::OwnerAugmentationRow],
    elementals: &[l2_db::OwnerElementalRow],
) {
    for item in items.iter_mut() {
        let id = item.object_id.0;
        for row in augmentations.iter().filter(|r| r.item_id == id) {
            if row.aug_attributes != -1 {
                item.augmentation = Some(row.aug_attributes);
            }
        }
        for row in elementals.iter().filter(|r| r.item_id == id) {
            if row.elem_type != -1 && row.elem_value != -1 {
                item.apply_attribute(i32::from(row.elem_type), row.elem_value);
            }
        }
    }
}

/// The element rows an item writes (`Item.updateItemElements`), ordered by element so two lists
/// with the same elements compare equal.
fn element_rows(item: &ItemData) -> Vec<l2_db::ItemElementalRow> {
    let mut rows: Vec<l2_db::ItemElementalRow> = item
        .elementals
        .iter()
        .map(|e| l2_db::ItemElementalRow {
            // The column is a `tinyint`; `Elementals.getElement()` is a Java `byte`.
            elem_type: e.element as i8,
            elem_value: e.value,
        })
        .collect();
    rows.sort_by_key(|r| r.elem_type);
    rows
}

// ---------------------------------------------------------------------------------------
// The database store
// ---------------------------------------------------------------------------------------

/// What is known to be in the database for one character, so a store only writes the
/// difference (Java writes items, skills and shortcuts when they change, not on every save).
#[derive(Debug, Default)]
struct Baseline {
    items: HashMap<i32, ItemData>,
    skills: HashMap<i32, i32>,
    shortcuts: HashMap<(i32, i32), ShortcutData>,
    /// `character_macroses` by macro id (L-02).
    macros: HashMap<i32, MacroData>,
    /// True while the maps come from the database instead of an earlier snapshot. Rows that are
    /// only in the database are not ours to delete: the snapshot may simply not carry them.
    from_database: bool,
    /// The class index `skills` and `shortcuts` belong to (S-SUB-01); `None` when they were not
    /// read (a subclass the old model did not know).
    class_index: Option<i32>,
}

impl Baseline {
    /// The state after a successful store: the snapshot itself.
    fn of_snapshot(player: &PlayerData) -> Self {
        Self {
            items: player.items.iter().map(|i| (i.object_id.0, i.clone())).collect(),
            skills: player
                .skills
                .iter()
                .map(|s| (s.skill_id, s.skill_level))
                .collect(),
            shortcuts: player.shortcuts.iter().map(|s| ((s.page, s.slot), *s)).collect(),
            macros: player.profile.macros.iter().map(|m| (m.id, m.clone())).collect(),
            from_database: false,
            class_index: class_index_of(player),
        }
    }
}

/// Stores into the real database through the repositories (Java's SQL, D-39).
pub struct DbStore {
    characters: CharacterRepository,
    items: ItemRepository,
    ground_items: GroundItemRepository,
    skills: SkillRepository,
    item_reuse: ItemReuseRepository,
    shortcuts: ShortcutRepository,
    macros: MacroRepository,
    hennas: HennaRepository,
    item_auctions: ItemAuctionRepository,
    instance_times: InstanceTimeRepository,
    coliseum: UndergroundColiseumRepository,
    bookmarks: BookmarkRepository,
    contacts: ContactRepository,
    reco: RecommendationRepository,
    variables: VariableRepository,
    punishments: PunishmentRepository,
    summons: SummonRepository,
    pets: PetRepository,
    clans: ClanRepository,
    buylists: BuyListRepository,
    messages: MessageRepository,
    recipes: RecipeRepository,
    crests: CrestRepository,
    friends: FriendRepository,
    championship: FishingChampionshipRepository,
    residences: ResidenceRepository,
    sieges: SiegeRepository,
    fort_sieges: FortSiegeRepository,
    territories: TerritoryRepository,
    hall_sieges: HallSiegeRepository,
    manor: ManorRepository,
    bosses: BossRepository,
    cursed: CursedWeaponRepository,
    olympiad: l2_db::OlympiadRepository,
    heroes: l2_db::HeroRepository,
    airships: AirshipRepository,
    auctions: AuctionRepository,
    bot_reports: BotReportRepository,
    offline_trades: OfflineTradeRepository,
    announcements: AnnouncementRepository,
    wedding: l2_db::WeddingRepository,
    seven_signs: l2_db::SevenSignsRepository,
    petition_feedback: PetitionFeedbackRepository,
    /// `character_subclasses` (S-SUB-01).
    subclasses: SubclassRepository,
    bbs_favorites: BbsFavoriteRepository,
    /// The ids the database gave the punishments this store inserted, by (key, affect, type).
    punishment_ids: HashMap<(String, &'static str, &'static str), u32>,
    baselines: HashMap<ObjectId, Baseline>,
    /// Item ids a `PersistCommand::ItemRows` wrote, so that trying the same ops again (a retry)
    /// updates the row instead of inserting it twice (M6-WH-01).
    rows_written: HashSet<i32>,
}

impl DbStore {
    /// A write of the hero tables with Java's statements (M8-HERO-01).
    async fn store_hero_write(&self, write: &l2_world::HeroWrite) -> Result<(), PersistError> {
        use l2_world::HeroWrite;
        let repo = &self.heroes;
        match write {
            HeroWrite::ResetPlayed => repo.reset_played().await?,
            HeroWrite::DeleteItems => repo.delete_hero_items().await?,
            HeroWrite::Insert {
                char_id,
                class_id,
                count,
                played,
                claimed,
            } => {
                repo.insert_hero(*char_id, *class_id, *count, *played, *claimed)
                    .await?
            }
            HeroWrite::Update {
                char_id,
                class_id,
                count,
                played,
                claimed,
            } => {
                repo.update_hero(*char_id, *class_id, *count, *played, *claimed)
                    .await?
            }
            HeroWrite::Diary {
                char_id,
                time,
                action,
                param,
            } => repo.insert_diary(*char_id, *time, *action, *param).await?,
            HeroWrite::Message { char_id, message } => repo.save_message(*char_id, message).await?,
        }
        Ok(())
    }

    pub fn new(pool: Pool) -> Self {
        Self {
            characters: CharacterRepository::new(pool.clone()),
            items: ItemRepository::new(pool.clone()),
            ground_items: GroundItemRepository::new(pool.clone()),
            skills: SkillRepository::new(pool.clone()),
            item_reuse: ItemReuseRepository::new(pool.clone()),
            shortcuts: ShortcutRepository::new(pool.clone()),
            macros: MacroRepository::new(pool.clone()),
            hennas: HennaRepository::new(pool.clone()),
            item_auctions: ItemAuctionRepository::new(pool.clone()),
            instance_times: InstanceTimeRepository::new(pool.clone()),
            coliseum: UndergroundColiseumRepository::new(pool.clone()),
            bookmarks: BookmarkRepository::new(pool.clone()),
            contacts: ContactRepository::new(pool.clone()),
            reco: RecommendationRepository::new(pool.clone()),
            variables: VariableRepository::new(pool.clone()),
            summons: SummonRepository::new(pool.clone()),
            pets: PetRepository::new(pool.clone()),
            punishments: PunishmentRepository::new(pool.clone()),
            buylists: BuyListRepository::new(pool.clone()),
            messages: MessageRepository::new(pool.clone()),
            recipes: RecipeRepository::new(pool.clone()),
            clans: ClanRepository::new(pool.clone()),
            crests: CrestRepository::new(pool.clone()),
            residences: ResidenceRepository::new(pool.clone()),
            sieges: SiegeRepository::new(pool.clone()),
            fort_sieges: FortSiegeRepository::new(pool.clone()),
            territories: TerritoryRepository::new(pool.clone()),
            hall_sieges: HallSiegeRepository::new(pool.clone()),
            manor: ManorRepository::new(pool.clone()),
            bosses: BossRepository::new(pool.clone()),
            cursed: CursedWeaponRepository::new(pool.clone()),
            olympiad: l2_db::OlympiadRepository::new(pool.clone()),
            heroes: l2_db::HeroRepository::new(pool.clone()),
            airships: AirshipRepository::new(pool.clone()),
            auctions: AuctionRepository::new(pool.clone()),
            friends: FriendRepository::new(pool.clone()),
            championship: FishingChampionshipRepository::new(pool.clone()),
            bot_reports: BotReportRepository::new(pool.clone()),
            announcements: AnnouncementRepository::new(pool.clone()),
            wedding: l2_db::WeddingRepository::new(pool.clone()),
            seven_signs: l2_db::SevenSignsRepository::new(pool.clone()),
            petition_feedback: PetitionFeedbackRepository::new(pool.clone()),
            subclasses: SubclassRepository::new(pool.clone()),
            bbs_favorites: BbsFavoriteRepository::new(pool.clone()),
            offline_trades: OfflineTradeRepository::new(pool),
            punishment_ids: HashMap::new(),
            baselines: HashMap::new(),
            rows_written: HashSet::new(),
        }
    }

    /// Java `Player.storeRecommendations` and `PlayerVariables.saveNowSync`: the Nevit rows the
    /// world changed (M4-VIT-01). Only what the snapshot says it changed is written.
    async fn store_nevit_rows(&self, player: &PlayerData) -> Result<(), PersistError> {
        let char_id = player.object_id.0;
        let profile = &player.profile;
        // `REPLACE INTO character_reco_bonus (charId,rec_have,rec_left,time_left)`.
        if profile.reco_store {
            self.reco
                .replace_reco_bonus(
                    char_id,
                    profile.recom_have,
                    profile.recom_left,
                    profile.recom_time_left_ms,
                )
                .await?;
        }
        // The deletes first, then the upserts, both by name (Java sorts them).
        for name in &profile.variables_removed {
            self.variables.delete_variable(char_id, name).await?;
        }
        for name in &profile.variables_changed {
            // Java: `if (value != null)`.
            if let Some(value) = profile.variables.get(name) {
                self.variables.upsert_variable(char_id, name, value).await?;
            }
        }
        Ok(())
    }

    /// Java `Player.storeEffect`: the character's `character_skills_save` rows are deleted, then
    /// one row per saved buff or reuse timer, `buff_index` counting from 1 in the order the world
    /// gave them (`restoreEffects` reads them back in that order). Java runs the batch on one
    /// connection without a transaction, so does this.
    // Java: Player.storeEffect (DELETE_SKILL_SAVE, ADD_SKILL_SAVE)
    async fn store_effects(
        &self,
        char_id: i32,
        class_index: i32,
        effects: &[SavedEffect],
    ) -> Result<(), PersistError> {
        let rows: Vec<NewSkillSave> = effects
            .iter()
            .enumerate()
            .map(|(index, effect)| NewSkillSave {
                skill_id: effect.skill_id,
                skill_level: effect.skill_level,
                remaining_time: effect.remaining_time,
                reuse_delay: effect.reuse_delay,
                systime: effect.systime,
                restore_type: effect.restore_type,
                buff_index: i32::try_from(index + 1).unwrap_or(i32::MAX),
            })
            .collect();
        self.skills.delete_skill_effects(char_id, class_index).await?;
        self.skills.add_skill_effects(char_id, class_index, &rows).await?;
        Ok(())
    }

    /// M5-FIX-03: Java `Player.storeItemReuseDelay` replaces the character's
    /// `character_item_reuse_save` rows with the timers that still run. Java catches a failure and
    /// logs it ("Could not store char item reuse data"), so a failure here does not fail the store.
    // Java: Player.storeItemReuseDelay (DELETE_ITEM_REUSE_SAVE, ADD_ITEM_REUSE_SAVE)
    async fn store_item_reuse(&self, char_id: i32, rows: &[SavedItemReuse]) {
        let rows: Vec<NewItemReuse> = rows
            .iter()
            .map(|r| NewItemReuse {
                item_id: r.item_id,
                item_obj_id: r.item_obj_id,
                reuse_delay: r.reuse_delay,
                systime: r.systime,
            })
            .collect();
        let result = async {
            self.item_reuse.delete_item_reuse(char_id).await?;
            self.item_reuse.add_item_reuse(char_id, &rows).await
        }
        .await;
        if let Err(e) = result {
            warn!("Could not store char item reuse data: {e}");
        }
    }

    /// The class index of a `characters` row (Java `Player.restore`): 0 when it plays its base
    /// class, else the index of the `character_subclasses` row of its class; `None` when there is
    /// no such row (Java reverts such a character to its base class at its next login).
    async fn stored_class_index(&self, row: &CharacterRow) -> Result<Option<i32>, PersistError> {
        let class_id = row.classid.map(i32::from);
        if class_id == Some(i32::from(row.base_class)) {
            return Ok(Some(0));
        }
        let Some(class_id) = class_id else {
            return Ok(None);
        };
        let subs = self.subclasses.subclasses_of(row.object_id()).await?;
        Ok(subs
            .iter()
            .find(|s| s.class_id == class_id)
            .map(|s| s.class_index))
    }

    /// The baseline of a character that was not stored since the server started: its rows as
    /// they are in the database now.
    async fn load_baseline(&self, char_id: i32, class_index: Option<i32>) -> Result<Baseline, PersistError> {
        let mut baseline = Baseline {
            from_database: true,
            class_index,
            ..Baseline::default()
        };
        for location in ALL_LOCATIONS {
            for row in self.items.items_of_location(char_id, location).await? {
                let item = ItemData {
                    object_id: ObjectId(row.object_id),
                    item_id: row.item_id.unwrap_or(0),
                    count: i64::try_from(row.count).unwrap_or(i64::MAX),
                    enchant_level: row.enchant_level.unwrap_or(0),
                    loc: row.loc.clone().unwrap_or_default(),
                    loc_data: row.loc_data.unwrap_or(0),
                    custom_type1: row.custom_type1.unwrap_or(0),
                    custom_type2: row.custom_type2.unwrap_or(0),
                    mana_left: row.mana_left,
                    time: row.time,
                    ..Default::default()
                };
                baseline.items.insert(row.object_id, item);
            }
        }
        // Java `Item.restoreAttributes`: the augmentation and elements the rows hold now.
        restore_item_attributes(&self.items, char_id, baseline.items.values_mut()).await?;
        // Java: MacroList.restoreMe (the same rows and the same reading as at the login).
        for row in self.macros.macros_of(char_id).await? {
            let macro_data = MacroData {
                id: row.id,
                icon: row.icon.unwrap_or(0),
                name: row.name.unwrap_or_default(),
                descr: row.descr.unwrap_or_default(),
                acronym: row.acronym.unwrap_or_default(),
                commands: crate::client::enter::parse_macro_commands(row.commands.as_deref().unwrap_or("")),
            };
            baseline.macros.insert(macro_data.id, macro_data);
        }
        if let Some(class_index) = class_index {
            for skill in self.skills.skills_of(char_id, class_index).await? {
                baseline.skills.insert(skill.skill_id, skill.skill_level);
            }
            for row in self.shortcuts.shortcuts_of(char_id, class_index).await? {
                let shortcut = ShortcutData {
                    slot: row.slot,
                    page: row.page,
                    kind: row.kind.unwrap_or(0),
                    shortcut_id: row.shortcut_id.and_then(|v| i32::try_from(v).ok()).unwrap_or(0),
                    level: row.level.unwrap_or(0),
                };
                baseline
                    .shortcuts
                    .insert((shortcut.page, shortcut.slot), shortcut);
            }
        }
        Ok(baseline)
    }

    /// Java `Item.updateInDb` (the row exists, `old` is what was written before) or
    /// `Item.insertIntoDb` (`old` is `None`), then the augmentation and element rows if they
    /// differ from `old` (Java writes them when they are set, and with an insert). `Ok(false)`
    /// when the item is in a location the schema does not know (nothing is written).
    async fn write_item(
        &self,
        char_id: i32,
        item: &ItemData,
        old: Option<&ItemData>,
    ) -> Result<bool, PersistError> {
        self.write_item_row(char_id, item, old.is_some(), old).await
    }

    /// [`write_item`](Self::write_item) with the owner column and the existence of the row given
    /// (`PersistCommand::ItemRows` knows both: the owner may be a clan or another character).
    async fn write_item_row(
        &self,
        char_id: i32,
        item: &ItemData,
        exists: bool,
        old: Option<&ItemData>,
    ) -> Result<bool, PersistError> {
        let id = item.object_id.0;
        let Some(loc) = ItemLocation::from_db_str(&item.loc) else {
            warn!(
                "Item {id} of character {char_id} has the unknown location {:?}; not stored.",
                item.loc
            );
            return Ok(false);
        };
        if exists {
            self.items
                .update_item(
                    id,
                    &ItemUpdate {
                        owner_id: char_id,
                        count: item.count,
                        loc,
                        loc_data: item.loc_data,
                        enchant_level: item.enchant_level,
                        custom_type1: item.custom_type1,
                        custom_type2: item.custom_type2,
                        mana_left: item.mana_left,
                        time: item.time,
                    },
                )
                .await?;
        } else {
            self.items
                .insert_item(&NewItem {
                    owner_id: char_id,
                    item_id: item.item_id,
                    count: item.count,
                    loc,
                    loc_data: item.loc_data,
                    enchant_level: item.enchant_level,
                    object_id: id,
                    custom_type1: item.custom_type1,
                    custom_type2: item.custom_type2,
                    mana_left: item.mana_left,
                    time: item.time,
                })
                .await?;
        }
        // Java: updateItemAttributes (REPLACE) / removeAugmentation (DELETE), updateItemElements
        // (DELETE all, INSERT each).
        let old_augmentation = old.and_then(|o| o.augmentation);
        if item.augmentation != old_augmentation {
            match item.augmentation {
                Some(augmentation) => self.items.set_augmentation(id, augmentation).await?,
                None => self.items.delete_augmentation(id).await?,
            }
        }
        let rows = element_rows(item);
        if rows != old.map(element_rows).unwrap_or_default() {
            self.items.replace_elementals(id, &rows).await?;
        }
        Ok(true)
    }

    /// The writes of `store_item_changes`; the baseline follows each successful statement.
    async fn apply_item_changes(
        &self,
        char_id: i32,
        upserts: &[ItemData],
        removed: &[ObjectId],
        baseline: &mut Baseline,
    ) -> Result<(), PersistError> {
        for item in upserts {
            let id = item.object_id.0;
            if !self.write_item(char_id, item, baseline.items.get(&id)).await? {
                continue;
            }
            baseline.items.insert(id, item.clone());
        }
        for object_id in removed {
            // Java `removeFromDb` only runs for a row that exists.
            if baseline.items.contains_key(&object_id.0) {
                self.items.delete_item(object_id.0).await?;
                baseline.items.remove(&object_id.0);
            }
        }
        Ok(())
    }

    /// The writes of `store_item_rows`: each op is Java's statement for an item that changed
    /// owner or container. The `tracked` character's baseline follows each successful statement:
    /// a row now in its inventory or paperdoll is a row the next full store compares, any other
    /// row (a warehouse, the freight, somebody else's) is not, so the store neither deletes it
    /// as "gone" nor inserts it again.
    async fn apply_item_rows(
        &mut self,
        tracked: Option<ObjectId>,
        ops: &[ItemRowOp],
        mut baseline: Option<&mut Baseline>,
    ) -> Result<(), PersistError> {
        for op in ops {
            match op {
                ItemRowOp::Write { owner_id, item }
                | ItemRowOp::Update { owner_id, item }
                | ItemRowOp::Insert { owner_id, item } => {
                    let id = item.object_id.0;
                    let old = baseline.as_ref().and_then(|b| b.items.get(&id)).cloned();
                    let exists = match op {
                        ItemRowOp::Update { .. } => true,
                        _ => old.is_some() || self.rows_written.contains(&id),
                    };
                    if !self.write_item_row(*owner_id, item, exists, old.as_ref()).await? {
                        continue;
                    }
                    if self.rows_written.len() >= ROWS_WRITTEN_LIMIT {
                        self.rows_written.clear();
                    }
                    self.rows_written.insert(id);
                    if let Some(baseline) = baseline.as_deref_mut() {
                        let mine = tracked.is_some_and(|t| t.0 == *owner_id)
                            && (item.loc == "INVENTORY" || item.loc == "PAPERDOLL");
                        if mine {
                            baseline.items.insert(id, item.clone());
                        } else {
                            baseline.items.remove(&id);
                        }
                    }
                }
                ItemRowOp::Delete { object_id } => {
                    self.items.delete_item(object_id.0).await?;
                    self.rows_written.remove(&object_id.0);
                    if let Some(baseline) = baseline.as_deref_mut() {
                        baseline.items.remove(&object_id.0);
                    }
                }
            }
        }
        Ok(())
    }

    /// The writes of `store_skill_changes`; the baseline follows each successful statement.
    // Java: Player.storeSkill (UPDATE_CHARACTER_SKILL_LEVEL when the skill was known, else
    // ADD_NEW_SKILLS) and Player.removeSkill (DELETE_SKILL_FROM_CHAR).
    async fn apply_skill_changes(
        &self,
        char_id: i32,
        class_index: i32,
        upserts: &[SkillData],
        removed: &[i32],
        baseline: &mut Baseline,
    ) -> Result<(), PersistError> {
        for SkillData {
            skill_id,
            skill_level,
        } in upserts
        {
            if baseline.skills.contains_key(skill_id) {
                self.skills
                    .update_skill_level(char_id, class_index, *skill_id, *skill_level)
                    .await?;
            } else {
                self.skills
                    .replace_skills(
                        char_id,
                        class_index,
                        &[SkillLevel {
                            skill_id: *skill_id,
                            skill_level: *skill_level,
                        }],
                    )
                    .await?;
            }
            baseline.skills.insert(*skill_id, *skill_level);
        }
        for skill_id in removed {
            self.skills.delete_skill(char_id, class_index, *skill_id).await?;
            baseline.skills.remove(skill_id);
        }
        Ok(())
    }

    /// Writes what differs between `player` and the baseline, and updates the baseline after
    /// every successful statement, so a failure half way leaves it accurate for the next try.
    async fn sync_children(
        &self,
        player: &PlayerData,
        class_index: Option<i32>,
        baseline: &mut Baseline,
    ) -> Result<(), PersistError> {
        let char_id = player.object_id.0;
        // Rows we saw in an earlier snapshot and that are gone now were destroyed in the world.
        let may_delete = !baseline.from_database;

        // Items. Java: Item.insertIntoDb / updateInDb / removeFromDb.
        let now_items: HashSet<i32> = player.items.iter().map(|i| i.object_id.0).collect();
        for item in &player.items {
            let id = item.object_id.0;
            if baseline.items.get(&id) == Some(item) {
                continue;
            }
            if !self.write_item(char_id, item, baseline.items.get(&id)).await? {
                continue;
            }
            baseline.items.insert(id, item.clone());
        }
        if may_delete {
            let gone: Vec<i32> = baseline
                .items
                .keys()
                .copied()
                .filter(|id| !now_items.contains(id))
                .collect();
            for id in gone {
                self.items.delete_item(id).await?;
                baseline.items.remove(&id);
            }
        }

        // Java: MacroList.registerMacroInDb / deleteMacroFromDb (by difference, L-02). The macros
        // belong to the character, not to a class index, and a snapshot always carries the whole
        // list (the profile is loaded at the login), so a macro that is gone is deleted even while
        // the baseline still comes from the database.
        let now_macros: HashSet<i32> = player.profile.macros.iter().map(|m| m.id).collect();
        for macro_data in &player.profile.macros {
            let old = baseline.macros.get(&macro_data.id);
            if old == Some(macro_data) {
                continue;
            }
            // `registerMacro` deletes the row of the macro it replaces, then inserts.
            if old.is_some() {
                self.macros.delete_macro(char_id, macro_data.id).await?;
            }
            self.macros
                .insert_macro(
                    char_id,
                    &NewMacro {
                        id: macro_data.id,
                        icon: macro_data.icon,
                        name: macro_data.name.clone(),
                        descr: Some(macro_data.descr.clone()),
                        acronym: macro_data.acronym.clone(),
                        commands: macro_commands_text(macro_data),
                    },
                )
                .await?;
            baseline.macros.insert(macro_data.id, macro_data.clone());
        }
        let gone: Vec<i32> = baseline
            .macros
            .keys()
            .copied()
            .filter(|id| !now_macros.contains(id))
            .collect();
        for macro_id in gone {
            self.macros.delete_macro(char_id, macro_id).await?;
            baseline.macros.remove(&macro_id);
        }

        // Skills and shortcuts belong to the active class index; without one (a subclass is
        // active and M2 does not model subclasses) they are left alone.
        let Some(class_index) = class_index else {
            return Ok(());
        };

        // Java: Player.storeSkill
        let now_skills: HashSet<i32> = player.skills.iter().map(|s| s.skill_id).collect();
        for SkillData {
            skill_id,
            skill_level,
        } in &player.skills
        {
            match baseline.skills.get(skill_id) {
                Some(level) if level == skill_level => continue,
                Some(_) => {
                    self.skills
                        .update_skill_level(char_id, class_index, *skill_id, *skill_level)
                        .await?;
                }
                None => {
                    self.skills
                        .replace_skills(
                            char_id,
                            class_index,
                            &[SkillLevel {
                                skill_id: *skill_id,
                                skill_level: *skill_level,
                            }],
                        )
                        .await?;
                }
            }
            baseline.skills.insert(*skill_id, *skill_level);
        }
        if may_delete {
            let gone: Vec<i32> = baseline
                .skills
                .keys()
                .copied()
                .filter(|id| !now_skills.contains(id))
                .collect();
            for id in gone {
                self.skills.delete_skill(char_id, class_index, id).await?;
                baseline.skills.remove(&id);
            }
        }

        // Java: Shortcuts.registerShortcutInDb / deleteShortcutFromDb
        let now_shortcuts: HashSet<(i32, i32)> = player.shortcuts.iter().map(|s| (s.page, s.slot)).collect();
        for shortcut in &player.shortcuts {
            let key = (shortcut.page, shortcut.slot);
            if baseline.shortcuts.get(&key) == Some(shortcut) {
                continue;
            }
            self.shortcuts
                .replace_shortcut(
                    char_id,
                    class_index,
                    &NewShortcut {
                        slot: shortcut.slot,
                        page: shortcut.page,
                        kind: shortcut.kind,
                        shortcut_id: shortcut.shortcut_id,
                        level: shortcut.level,
                    },
                )
                .await?;
            baseline.shortcuts.insert(key, *shortcut);
        }
        if may_delete {
            let gone: Vec<(i32, i32)> = baseline
                .shortcuts
                .keys()
                .copied()
                .filter(|key| !now_shortcuts.contains(key))
                .collect();
            for (page, slot) in gone {
                self.shortcuts
                    .delete_shortcut(char_id, class_index, slot, page)
                    .await?;
                baseline.shortcuts.remove(&(page, slot));
            }
        }
        Ok(())
    }
}

/// The `commands` column of a macro (Java `MacroList.registerMacroInDb`): `type,d1,d2[,cmd];` per
/// command, cut to 255 characters (UTF-16 units, like `StringBuilder.setLength(255)`).
pub(crate) fn macro_commands_text(macro_data: &MacroData) -> String {
    let mut text = String::new();
    for cmd in &macro_data.commands {
        text.push_str(&format!("{},{},{}", cmd.kind, cmd.d1, cmd.d2));
        if !cmd.cmd.is_empty() {
            text.push(',');
            text.push_str(&cmd.cmd);
        }
        text.push(';');
    }
    let units: Vec<u16> = text.encode_utf16().collect();
    if units.len() > 255 {
        return String::from_utf16_lossy(&units[..255]);
    }
    text
}

/// How many ids `DbStore::rows_written` remembers before it starts over.
const ROWS_WRITTEN_LIMIT: usize = 65_536;

/// Every place an item of a character can be (the baseline reads them all, so an item that
/// moved between inventory and warehouse is updated, not inserted twice).
const ALL_LOCATIONS: [ItemLocation; 11] = [
    ItemLocation::Void,
    ItemLocation::Inventory,
    ItemLocation::Paperdoll,
    ItemLocation::Warehouse,
    ItemLocation::ClanWh,
    ItemLocation::Pet,
    ItemLocation::PetEquip,
    ItemLocation::Lease,
    ItemLocation::Refund,
    ItemLocation::Mail,
    ItemLocation::Freight,
];

/// The class index of the data being stored (S-SUB-01): the snapshot's. A snapshot that plays a
/// class other than its base class under index 0 does not know its subclass (the old model), so
/// its skills and shortcuts are skipped.
fn class_index_of(player: &PlayerData) -> Option<i32> {
    let index = player.profile.class_index;
    (index > 0 || player.class_id == player.base_class).then_some(index)
}

fn clamp_i32(value: u64) -> i32 {
    i32::try_from(value).unwrap_or(i32::MAX)
}

fn clamp_i64(value: u64) -> i64 {
    i64::try_from(value).unwrap_or(i64::MAX)
}

/// The values of Java's `Player.storeCharBase`: what the snapshot models comes from the
/// snapshot, everything else is the row's current value, written back unchanged.
// Java: Player.storeCharBase (UPDATE_CHARACTER)
pub fn build_character_update(player: &PlayerData, row: &CharacterRow) -> CharacterUpdate {
    // Java stores the base class level, exp and sp in `characters` (`getBaseLevel` ...); the
    // active subclass has its own row in `character_subclasses` (S-SUB-01). A snapshot without the
    // base values keeps the row's.
    let base = l2_world::base_class_stats(player).filter(|_| class_index_of(player).is_some());
    // M6-CLAN-01: the clan columns the world keeps (Java `storeCharBase`); a snapshot without
    // them keeps the row's values.
    let clan = player.profile.clan.filter(|c| !c.keep_stored);
    let title = if player.title.is_empty() && row.title.is_none() {
        None
    } else {
        Some(player.title.clone())
    };
    CharacterUpdate {
        level: base.map_or_else(|| i32::from(row.level.unwrap_or(1)), |b| b.level),
        // Java getMaxHp() etc. are ints; HP/MP/CP current values are bound as doubles.
        max_hp: player.max_hp as i32,
        cur_hp: player.cur_hp,
        max_cp: player.max_cp as i32,
        cur_cp: player.cur_cp,
        max_mp: player.max_mp as i32,
        cur_mp: player.cur_mp,
        face: player.face,
        hair_style: player.hair_style,
        hair_color: player.hair_color,
        sex: i32::from(player.sex == 1),
        heading: player.heading,
        x: player.x,
        y: player.y,
        z: player.z,
        exp: base.map_or_else(|| row.exp.map_or(0, clamp_i64), |b| b.exp),
        // Java writes `_expBeforeDeath`, which it never reads back (0 after a login).
        exp_before_death: player.profile.exp_before_death,
        sp: base.map_or_else(|| clamp_i64(row.sp), |b| b.sp),
        karma: player.karma,
        // Java writes `getFame()`: multisell fame ingredients and products change it (M6-MULTISELL-01).
        fame: player.profile.fame,
        pvp_kills: player.pvp_kills,
        pk_kills: player.pk_kills,
        clan_id: clan.map_or_else(
            || row.clanid.map_or(0, |v| clamp_i32(u64::from(v))),
            |c| c.clan_id,
        ),
        race: player.race,
        class_id: player.class_id,
        delete_time: clamp_i64(row.deletetime),
        title,
        // Java writes `getAppearance().getTitleColor()`: the PvP title colours change it.
        title_color: player.profile.title_color,
        access_level: player.access_level,
        // The snapshot is of a player in the world; `PlayerLeft` marks it offline afterwards.
        // 2 for a player without a client (an offline trader, Java `isOnlineInt()`).
        online: player.profile.online_status,
        // Java `_isIn7sDungeon` (M8-SS-01).
        is_in_7s_dungeon: player.profile.in_7s_dungeon,
        clan_privs: clan.map_or_else(
            || row.clan_privs.map_or(0, |v| clamp_i32(u64::from(v))),
            |c| c.stored_privileges(),
        ),
        wants_peace: clan.map_or_else(|| row.wantspeace.map_or(0, i32::from), |c| c.wants_peace),
        base_class: player.base_class,
        online_time: player.online_time,
        newbie: row.newbie.map_or(0, |v| clamp_i32(u64::from(v))),
        noble: row.nobless != 0,
        power_grade: clan.map_or_else(|| row.power_grade.map_or(0, i32::from), |c| c.power_grade),
        subpledge: clan.map_or_else(|| i32::from(row.subpledge), |c| c.pledge_type),
        lvl_joined_academy: clan.map_or_else(|| i32::from(row.lvl_joined_academy), |c| c.lvl_joined_academy),
        apprentice: clan.map_or_else(|| i64::from(row.apprentice), |c| i64::from(c.apprentice)),
        sponsor: clan.map_or_else(|| i64::from(row.sponsor), |c| i64::from(c.sponsor)),
        clan_join_expiry_time: clan.map_or_else(
            || clamp_i64(row.clan_join_expiry_time),
            |c| c.clan_join_expiry_time,
        ),
        clan_create_expiry_time: clan.map_or_else(
            || clamp_i64(row.clan_create_expiry_time),
            |c| c.clan_create_expiry_time,
        ),
        char_name: player.name.clone(),
        death_penalty_level: i64::from(row.death_penalty_level),
        bookmark_slot: i32::from(row.bookmarkslot),
        // Java stores `(int) _vitalityPoints`. The world keeps the number in the profile; 0 (never
        // a real value, the minimum is 1) means a snapshot that does not carry it.
        vitality_points: if player.profile.vitality_points >= 1 {
            player.profile.vitality_points
        } else {
            i32::from(row.vitality_points)
        },
        language: row.language.clone(),
        faction: i32::from(row.faction),
        // Java writes `getPcCafePoints()`: hunting, the retail timer and multisell change it
        // (M6-FU-11).
        pccafe_points: player.profile.pccafe_points,
    }
}

impl PlayerStore for DbStore {
    async fn store_player(&mut self, player: &PlayerData) -> Result<(), PersistError> {
        let char_id = player.object_id.0;
        let row = self
            .characters
            .restore_character(char_id)
            .await?
            .ok_or(PersistError::NoSuchCharacter(char_id))?;
        if row.account_name.as_deref() != Some(player.account.as_str()) {
            return Err(PersistError::WrongAccount(char_id, player.account.clone()));
        }
        // Java: Player.store -> storeCharBase
        self.characters
            .update_character(char_id, &build_character_update(player, &row))
            .await?;
        // Java: Player.store -> storeCharSub (S-SUB-01). Java logs a failure and goes on.
        for sub in l2_world::stored_subclasses(player) {
            let values = SubclassValues {
                class_id: sub.class_id,
                exp: sub.exp,
                sp: sub.sp,
                level: sub.level,
            };
            if let Err(e) = self
                .subclasses
                .update_subclass(char_id, sub.class_index, &values)
                .await
            {
                warn!("Could not store sub class data for {}: {e}", player.name);
            }
        }
        // Java: Player.store -> getVariables().storeMe(); autoSave / deleteMe -> storeRecommendations.
        self.store_nevit_rows(player).await?;
        // Java: Player.store -> storeEffect (the buffs of the character, see `store_effects`).
        if let Some(effects) = &player.profile.effects_to_store {
            self.store_effects(char_id, player.profile.class_index, effects)
                .await?;
        }

        // Java: Player.store -> storeItemReuseDelay.
        if let Some(rows) = &player.profile.item_reuse_to_store {
            self.store_item_reuse(char_id, rows).await;
        }

        // Java: Player.store -> storeRecipeShopList (only with `StoreRecipeShopList`, and only a
        // character that has a shop; Java logs a failure and goes on).
        if let Some(shop) = &player.profile.recipe_shop_to_store {
            let rows: Vec<RecipeShopRow> = shop
                .iter()
                .map(|&(recipe_id, price)| RecipeShopRow { recipe_id, price })
                .collect();
            if let Err(e) = self.recipes.store_shop(char_id, &rows).await {
                error!("Could not store recipe shop for playerId {char_id}: {e}");
            }
        }

        // Java: Player.store -> SevenSigns.saveSevenSignsData(getObjectId()) (M8-SS-01; Java logs a
        // failure and goes on).
        if let Some(row) = &player.profile.seven_signs_to_store
            && let Err(e) = self.seven_signs.update_player(&seven_signs_row(row)).await
        {
            error!("SevenSigns: Unable to save data to database: {e}");
        }

        // Java: Inventory.updateDatabase, Shortcuts / storeSkill (by difference).
        let class_index = class_index_of(player);
        // A baseline of another class index (the class changed since it was made, S-SUB-01) says
        // nothing about this one's rows: they are read again, and nothing is deleted this time.
        let mut baseline = match self.baselines.remove(&player.object_id) {
            Some(baseline) if baseline.class_index == class_index => baseline,
            _ => self.load_baseline(char_id, class_index).await?,
        };
        let result = self.sync_children(player, class_index, &mut baseline).await;
        if result.is_ok() {
            // From now on the baseline is exactly what we manage: rows that were only in the
            // database (not in the snapshot) drop out and are never deleted.
            baseline = Baseline::of_snapshot(player);
        }
        self.baselines.insert(player.object_id, baseline);
        result
    }

    async fn store_item_changes(
        &mut self,
        owner: ObjectId,
        upserts: &[ItemData],
        removed: &[ObjectId],
    ) -> Result<(), PersistError> {
        let char_id = owner.0;
        // The rows this character is known to have: from its last store, else from the database
        // (once; the baseline then follows every write).
        let mut baseline = match self.baselines.remove(&owner) {
            Some(baseline) => baseline,
            None => {
                let row = self
                    .characters
                    .restore_character(char_id)
                    .await?
                    .ok_or(PersistError::NoSuchCharacter(char_id))?;
                let class_index = self.stored_class_index(&row).await?;
                self.load_baseline(char_id, class_index).await?
            }
        };
        let result = self
            .apply_item_changes(char_id, upserts, removed, &mut baseline)
            .await;
        self.baselines.insert(owner, baseline);
        result
    }

    async fn store_item_rows(
        &mut self,
        tracked: Option<ObjectId>,
        ops: &[ItemRowOp],
    ) -> Result<(), PersistError> {
        // The rows the tracked character is known to have: from its last store, else from the
        // database (once; the baseline then follows every write).
        let mut baseline = match tracked {
            None => None,
            Some(owner) => Some(match self.baselines.remove(&owner) {
                Some(baseline) => baseline,
                None => {
                    let row = self
                        .characters
                        .restore_character(owner.0)
                        .await?
                        .ok_or(PersistError::NoSuchCharacter(owner.0))?;
                    let class_index = self.stored_class_index(&row).await?;
                    self.load_baseline(owner.0, class_index).await?
                }
            }),
        };
        let result = self.apply_item_rows(tracked, ops, baseline.as_mut()).await;
        if let (Some(owner), Some(baseline)) = (tracked, baseline) {
            self.baselines.insert(owner, baseline);
        }
        result
    }

    async fn store_ground_items(&mut self, rows: &[GroundItemRow]) -> Result<(), PersistError> {
        let records: Vec<GroundItemRecord> = rows
            .iter()
            .map(|row| GroundItemRecord {
                object_id: row.object_id.0,
                item_id: row.item_id,
                count: row.count,
                enchant_level: row.enchant_level,
                x: row.x,
                y: row.y,
                z: row.z,
                drop_time: row.drop_time,
                equipable: row.equipable,
            })
            .collect();
        self.ground_items.replace_all(&records).await?;
        Ok(())
    }

    async fn store_skill_changes(
        &mut self,
        owner: ObjectId,
        upserts: &[SkillData],
        removed: &[i32],
    ) -> Result<(), PersistError> {
        let char_id = owner.0;
        // Skills belong to the active class index: the one of the baseline (the last store or the
        // last class switch, S-SUB-01), else the one the database row names. The skills this
        // character is known to have come from its last store, else from the database (once; the
        // baseline then follows every write).
        let mut baseline = match self.baselines.remove(&owner) {
            Some(baseline) => baseline,
            None => {
                let row = self
                    .characters
                    .restore_character(char_id)
                    .await?
                    .ok_or(PersistError::NoSuchCharacter(char_id))?;
                let class_index = self.stored_class_index(&row).await?;
                self.load_baseline(char_id, class_index).await?
            }
        };
        let Some(class_index) = baseline.class_index else {
            self.baselines.insert(owner, baseline);
            return Ok(());
        };
        let result = self
            .apply_skill_changes(char_id, class_index, upserts, removed, &mut baseline)
            .await;
        self.baselines.insert(owner, baseline);
        result
    }

    async fn store_reco_values(
        &mut self,
        owner: ObjectId,
        rec_have: i32,
        rec_left: i32,
    ) -> Result<(), PersistError> {
        self.reco.replace_reco_values(owner.0, rec_have, rec_left).await?;
        Ok(())
    }

    async fn reset_recommends(&mut self) -> Result<(), PersistError> {
        self.reco.reset_recommends().await?;
        Ok(())
    }

    async fn store_player_variables(
        &mut self,
        owner: ObjectId,
        upserts: &[(String, String)],
        removed: &[String],
    ) -> Result<(), PersistError> {
        // The deletes first, then the upserts (`PlayerVariables.saveNowSync`).
        for name in removed {
            self.variables.delete_variable(owner.0, name).await?;
        }
        for (name, value) in upserts {
            self.variables.upsert_variable(owner.0, name, value).await?;
        }
        Ok(())
    }

    async fn store_punishment(&mut self, punishment: &StoredPunishment) -> Result<(), PersistError> {
        let id = self
            .punishments
            .insert(&NewPunishment {
                key: punishment.key.clone(),
                affect: punishment.affect.name().to_owned(),
                kind: punishment.kind.name().to_owned(),
                expiration: punishment.expiration,
                reason: punishment.reason.clone(),
                punished_by: punishment.punished_by.clone(),
            })
            .await?;
        if let Ok(id) = u32::try_from(id) {
            self.punishment_ids.insert(
                (
                    punishment.key.clone(),
                    punishment.affect.name(),
                    punishment.kind.name(),
                ),
                id,
            );
        }
        Ok(())
    }

    async fn end_punishment(
        &mut self,
        id: i32,
        key: &str,
        affect: PunishmentAffect,
        kind: PunishmentType,
        ended_at: i64,
    ) -> Result<(), PersistError> {
        let known = self
            .punishment_ids
            .remove(&(key.to_owned(), affect.name(), kind.name()));
        let Some(id) = u32::try_from(id).ok().filter(|id| *id > 0).or(known) else {
            // The insert failed (it was logged): there is no row to update.
            warn!(
                "No stored row for the punishment {} of {key}; nothing to end.",
                kind.name()
            );
            return Ok(());
        };
        self.punishments.update_expiration(id, ended_at).await?;
        Ok(())
    }

    async fn change_char_access_level(&mut self, char_id: i32, level: i32) -> Result<(), PersistError> {
        self.characters.update_access_level(char_id, level).await?;
        Ok(())
    }

    async fn repair_char(&mut self, name: &str) -> Result<(), PersistError> {
        self.characters.repair_character(name).await?;
        Ok(())
    }

    async fn store_mail_write(&mut self, write: &MailWrite) -> Result<(), PersistError> {
        match write {
            // Java: MailManager.sendMessage (MailMessage.getStatement)
            MailWrite::Insert(msg) => {
                self.messages
                    .insert(&l2_db::MessageRow {
                        message_id: msg.id,
                        sender_id: msg.sender_id,
                        receiver_id: msg.receiver_id,
                        subject: msg.subject.clone(),
                        content: msg.content.clone(),
                        expiration: msg.expiration,
                        req_adena: msg.req_adena,
                        has_attachments: msg.has_attachments,
                        is_unread: msg.unread,
                        is_deleted_by_sender: msg.deleted_by_sender,
                        is_deleted_by_receiver: msg.deleted_by_receiver,
                        send_by_system: msg.send_by_system,
                        is_returned: msg.returned,
                    })
                    .await?;
            }
            MailWrite::MarkRead(id) => self.messages.mark_read(*id).await?,
            MailWrite::MarkDeletedBySender(id) => self.messages.mark_deleted_by_sender(*id).await?,
            MailWrite::MarkDeletedByReceiver(id) => self.messages.mark_deleted_by_receiver(*id).await?,
            MailWrite::RemoveAttachments(id) => self.messages.remove_attachments(*id).await?,
            MailWrite::Delete(id) => self.messages.delete(*id).await?,
        }
        Ok(())
    }

    async fn store_buy_list_product(&mut self, row: &ProductRow) -> Result<(), PersistError> {
        self.buylists
            .save_product(&l2_db::BuyListRow {
                buylist_id: row.list_id,
                item_id: row.item_id,
                count: row.count,
                next_restock_time: row.next_restock_time,
            })
            .await?;
        Ok(())
    }

    async fn store_summon(&mut self, write: &SummonWrite) -> Result<(), PersistError> {
        match write {
            // Java: CharSummonTable.saveSummon
            SummonWrite::SaveServitor {
                owner,
                summon_skill_id,
                cur_hp,
                cur_mp,
                time,
            } => {
                self.summons
                    .save_summon(owner.0, *summon_skill_id, *cur_hp, *cur_mp, *time)
                    .await?;
            }
            // Java: CharSummonTable.removeServitor
            SummonWrite::RemoveServitor { owner } => self.summons.remove_summon(owner.0).await?,
            // Java: Servitor.storeEffect / restoreEffects
            SummonWrite::ServitorEffects {
                owner,
                class_index,
                summon_skill_id,
                effects,
            } => {
                let rows: Vec<NewSummonSkillSave> = effects
                    .iter()
                    .map(|e| NewSummonSkillSave {
                        skill_id: e.skill_id,
                        skill_level: e.skill_level,
                        remaining_time: e.remaining_time,
                    })
                    .collect();
                self.summons
                    .store_servitor_effects(owner.0, *class_index, *summon_skill_id, &rows)
                    .await?;
            }
            // Java: Pet.storeMe
            SummonWrite::StorePet { insert, pet } => {
                let values = PetValues {
                    name: pet.name.clone().unwrap_or_default(),
                    level: pet.level,
                    cur_hp: pet.cur_hp,
                    cur_mp: pet.cur_mp,
                    exp: pet.exp,
                    sp: pet.sp,
                    fed: pet.fed,
                    owner_id: pet.owner.0,
                    restore: pet.restore,
                    item_obj_id: pet.control_object_id.0,
                };
                if *insert {
                    // A collar whose row exists already (it came to this character after the
                    // login, so the world did not know the row) is updated instead.
                    if let Err(e) = self.pets.insert_pet(&values).await {
                        if e.is_duplicate_key() {
                            self.pets.update_pet(&values).await?;
                        } else {
                            return Err(e.into());
                        }
                    }
                } else {
                    self.pets.update_pet(&values).await?;
                }
            }
            // Java: Player.storePetFood
            SummonWrite::PetFed {
                control_object_id,
                fed,
            } => {
                self.pets.update_fed(control_object_id.0, *fed).await?;
            }
            // Java: Pet.destroyControlItem, ItemManager.destroyItem
            SummonWrite::DeletePet { control_object_id } => self.pets.delete_pet(control_object_id.0).await?,
            // Java: Pet.storeEffect / restoreEffects
            SummonWrite::PetEffects {
                control_object_id,
                effects,
            } => {
                let rows: Vec<NewPetSkillSave> = effects
                    .iter()
                    .map(|e| NewPetSkillSave {
                        skill_id: e.skill_id,
                        skill_level: e.skill_level,
                        remaining_time: e.remaining_time,
                    })
                    .collect();
                self.pets.store_effects(control_object_id.0, &rows).await?;
            }
        }
        Ok(())
    }

    async fn store_recipe_write(&mut self, write: &RecipeWrite) -> Result<(), PersistError> {
        match write {
            RecipeWrite::Insert {
                char_id,
                recipe_id,
                class_index,
                dwarven,
            } => {
                self.recipes
                    .insert_recipe(*char_id, *recipe_id, *class_index, *dwarven)
                    .await?;
            }
            RecipeWrite::Delete {
                char_id,
                recipe_id,
                class_index,
            } => {
                self.recipes
                    .delete_recipe(*char_id, *recipe_id, *class_index)
                    .await?
            }
        }
        Ok(())
    }

    async fn store_fishing_championship(&mut self, data: &ChampionshipData) -> Result<(), PersistError> {
        let rows: Vec<FishingChampionshipRow> = data
            .rows
            .iter()
            .map(|r| FishingChampionshipRow {
                player_name: r.name.clone(),
                fish_length: r.length,
                rewarded: r.rewarded,
            })
            .collect();
        self.championship.save(data.end_date, &rows).await?;
        Ok(())
    }

    async fn load_pet_row(&mut self, control: ObjectId) -> Result<PetRowData, PersistError> {
        let row = self.pets.pet_of_control_item(control.0).await?;
        let effects = self.pets.effects_of(control.0).await?;
        Ok(PetRowData {
            row: row.map(saved_pet_of),
            effects: effects.into_iter().map(saved_pet_effect_of).collect(),
        })
    }

    async fn load_mail_attachments(
        &mut self,
        sender_id: i32,
        message_id: i32,
    ) -> Result<Vec<ItemData>, PersistError> {
        Ok(load_mail_attachments(&self.items, sender_id, message_id).await?)
    }

    async fn load_clan_warehouse(&mut self, clan_id: i32) -> Result<Vec<ItemData>, PersistError> {
        Ok(load_container_items(&self.items, clan_id, ItemLocation::ClanWh).await?)
    }

    async fn load_offline_pet_rows(&mut self, owner: ObjectId) -> Result<OfflinePetRows, PersistError> {
        let items = &self.items;
        let mut pet_items = load_container_items(items, owner.0, ItemLocation::Pet).await?;
        pet_items.extend(load_container_items(items, owner.0, ItemLocation::PetEquip).await?);
        let mut inventory = load_container_items(items, owner.0, ItemLocation::Inventory).await?;
        inventory.extend(load_container_items(items, owner.0, ItemLocation::Paperdoll).await?);
        Ok(OfflinePetRows { pet_items, inventory })
    }

    async fn store_offline_trade(&mut self, write: &OfflineTradeWrite) -> Result<(), PersistError> {
        let repo = &self.offline_trades;
        match write {
            // Java `storeOffliners`: both tables are emptied, then each shop is written; a shop that
            // fails is logged and the others go on.
            OfflineTradeWrite::StoreAll(rows) => {
                repo.clear_all().await?;
                for row in rows {
                    if let Err(e) = write_offline_shop(repo, row).await {
                        warn!(
                            "OfflineTraderTable: Error while saving offline trader: {} {e}",
                            row.char_id.0
                        );
                    }
                }
            }
            // Java `onTransaction`: the lines of the trader go; then its row (finished) or the new
            // lines and, on the first call, its row.
            OfflineTradeWrite::Transaction {
                char_id,
                finished,
                items,
                status,
            } => {
                repo.clear_player_items(char_id.0).await?;
                if *finished {
                    repo.clear_player_status(char_id.0).await?;
                } else {
                    for line in items {
                        repo.insert_item(char_id.0, line.item, line.count, line.price)
                            .await?;
                    }
                    if let Some(status) = status {
                        repo.insert_status(
                            char_id.0,
                            status.time,
                            status.store_type,
                            status.title.as_deref(),
                        )
                        .await?;
                    }
                }
            }
            // Java `removeTrader`.
            OfflineTradeWrite::Remove { char_id } => {
                repo.clear_player_items(char_id.0).await?;
                repo.clear_player_status(char_id.0).await?;
            }
        }
        Ok(())
    }

    async fn store_bot_report(&mut self, row: &BotReportRow) -> Result<(), PersistError> {
        self.bot_reports
            .insert_report(row.bot_id, row.reporter_id, row.report_date)
            .await?;
        Ok(())
    }

    async fn load_crest_ids(&mut self) -> Result<Vec<i32>, PersistError> {
        Ok(self.crests.ids().await?)
    }

    async fn load_block_list(&mut self, owner: i32) -> Result<Vec<i32>, PersistError> {
        Ok(self
            .friends
            .block_list_of(owner)
            .await?
            .into_iter()
            .map(|link| link.friend_id)
            .collect())
    }

    async fn store_announcement(&mut self, write: &AnnouncementWrite) -> Result<i32, PersistError> {
        let auto = |auto: &Option<(i64, i64, i32)>| {
            auto.map(|(initial, delay, repeat)| AutoColumns {
                initial,
                delay,
                repeat,
            })
        };
        match write {
            AnnouncementWrite::Insert {
                kind,
                content,
                author,
                auto: columns,
                ..
            } => {
                let id = self
                    .announcements
                    .insert(*kind, content, author, auto(columns))
                    .await?;
                Ok(i32::try_from(id).unwrap_or(0))
            }
            AnnouncementWrite::Update {
                id,
                kind,
                content,
                author,
                auto: columns,
            } => {
                self.announcements
                    .update(
                        u32::try_from(*id).unwrap_or(0),
                        *kind,
                        content,
                        author,
                        auto(columns),
                    )
                    .await?;
                Ok(0)
            }
            AnnouncementWrite::Delete { id } => {
                self.announcements.delete(u32::try_from(*id).unwrap_or(0)).await?;
                Ok(0)
            }
        }
    }

    async fn store_boss_write(&mut self, write: &BossWrite) -> Result<(), PersistError> {
        let repo = &self.bosses;
        match write {
            BossWrite::GrandBossStatus { boss_id, status } => {
                repo.update_grand_boss_status(*boss_id, *status).await?
            }
            BossWrite::GrandBoss(row) => repo.update_grand_boss(&grand_boss_update(row)).await?,
            BossWrite::GrandBossStoreAll { list, rows } => {
                let list: Vec<GrandBossListRow> = list
                    .iter()
                    .map(|(player_id, zone)| GrandBossListRow {
                        player_id: *player_id,
                        zone: *zone,
                    })
                    .collect();
                let rows: Vec<GrandBossStore> = rows
                    .iter()
                    .map(|row| match row {
                        GrandBossStoreRow::StatusOnly { boss_id, status } => GrandBossStore::StatusOnly {
                            boss_id: *boss_id,
                            status: *status,
                        },
                        GrandBossStoreRow::Full(row) => GrandBossStore::Full(grand_boss_update(row)),
                    })
                    .collect();
                repo.store_all(&list, &rows).await?
            }
            BossWrite::RaidBossInsert(row) => {
                repo.insert_raid_boss(&RaidBossRow {
                    boss_id: row.boss_id,
                    amount: row.amount,
                    loc_x: row.loc_x,
                    loc_y: row.loc_y,
                    loc_z: row.loc_z,
                    heading: row.heading,
                    respawn_delay: row.respawn_delay,
                    respawn_random: row.respawn_random,
                    respawn_time: row.respawn_time,
                    current_hp: row.current_hp,
                    current_mp: row.current_mp,
                })
                .await?
            }
            BossWrite::RaidBossDelete { boss_id } => repo.delete_raid_boss(*boss_id).await?,
            BossWrite::RaidBossUpdate(rows) => {
                // Java logs a failed row and goes on with the next.
                for row in rows {
                    if let Err(e) = repo
                        .update_raid_boss(row.boss_id, row.respawn_time, row.current_hp, row.current_mp)
                        .await
                    {
                        warn!("RaidBossSpawnManager: Could not update raidboss_spawnlist table {e}");
                    }
                }
            }
            BossWrite::RaidPoints {
                char_id,
                boss_id,
                points,
            } => repo.replace_raid_points(*char_id, *boss_id, *points).await?,
            BossWrite::RaidPointsDeleteAll => repo.delete_all_raid_points().await?,
        }
        Ok(())
    }

    async fn store_bookmark_write(&mut self, write: &BookmarkWrite) -> Result<(), PersistError> {
        match write {
            BookmarkWrite::Add { char_id, bookmark } => {
                let row = BookmarkRow {
                    id: bookmark.id,
                    x: bookmark.x,
                    y: bookmark.y,
                    z: bookmark.z,
                    icon: bookmark.icon,
                    tag: Some(bookmark.tag.clone()),
                    name: bookmark.name.clone(),
                };
                self.bookmarks.add_bookmark(*char_id, &row).await?;
            }
            BookmarkWrite::Update {
                char_id,
                id,
                icon,
                tag,
                name,
            } => {
                self.bookmarks
                    .update_bookmark(*char_id, *id, *icon, Some(tag), name)
                    .await?;
            }
            BookmarkWrite::Delete { char_id, id } => self.bookmarks.delete_bookmark(*char_id, *id).await?,
        }
        Ok(())
    }

    async fn store_contact_write(&mut self, write: &ContactWrite) -> Result<(), PersistError> {
        match *write {
            ContactWrite::Add { char_id, contact_id } => {
                self.contacts.add_contact(char_id, contact_id).await?
            }
            ContactWrite::Remove { char_id, contact_id } => {
                self.contacts.remove_contact(char_id, contact_id).await?;
            }
        }
        Ok(())
    }

    async fn load_boss_rows(&mut self, kind: BossReadKind) -> Result<BossRereadRows, PersistError> {
        Ok(match kind {
            BossReadKind::GrandBoss(boss_id) => BossRereadRows::GrandBoss {
                boss_id,
                row: self.bosses.grand_boss(boss_id).await?.map(grand_boss_data),
            },
            BossReadKind::RaidPoints(char_id) => BossRereadRows::RaidPoints {
                char_id,
                rows: self
                    .bosses
                    .raid_points_of(char_id)
                    .await?
                    .into_iter()
                    .map(raid_points_record)
                    .collect(),
            },
        })
    }

    async fn load_friend_summary(&mut self, char_id: i32) -> Result<Option<FriendSummary>, PersistError> {
        Ok(self
            .friends
            .character_summary(char_id)
            .await?
            .map(|row| FriendSummary {
                object_id: char_id,
                name: row.name,
                // Java: `rset.getInt(2) == 1`
                online: row.online == 1,
                class_id: row.class_id,
                level: row.level,
            }))
    }

    async fn store_subclass_write(&mut self, write: &l2_world::SubclassWrite) -> Result<(), PersistError> {
        use l2_world::SubclassWrite;
        match write {
            // Java: Player.addSubClass (ADD_CHAR_SUBCLASS, then storeSkill for each skill). A
            // failed insert stores no skill.
            SubclassWrite::Add {
                char_id,
                holder,
                skills,
            } => {
                let values = SubclassValues {
                    class_id: holder.class_id,
                    exp: holder.exp,
                    sp: holder.sp,
                    level: holder.level,
                };
                self.subclasses
                    .insert_subclass(*char_id, holder.class_index, &values)
                    .await?;
                let levels: Vec<SkillLevel> = skills
                    .iter()
                    .map(|s| SkillLevel {
                        skill_id: s.skill_id,
                        skill_level: s.skill_level,
                    })
                    .collect();
                if !levels.is_empty() {
                    self.skills
                        .replace_skills(*char_id, holder.class_index, &levels)
                        .await?;
                }
            }
            // Java: Player.modifySubClass (DELETE_CHAR_HENNA per slot, DELETE_CHAR_SHORTCUTS,
            // DELETE_SKILL_SAVE, DELETE_CHAR_SKILLS, DELETE_CHAR_SUBCLASS; one try block).
            SubclassWrite::Delete {
                char_id,
                class_index,
                henna_slots,
            } => {
                for slot in henna_slots {
                    self.hennas.delete_henna(*char_id, *slot, *class_index).await?;
                }
                self.shortcuts
                    .delete_class_shortcuts(*char_id, *class_index)
                    .await?;
                self.skills.delete_skill_effects(*char_id, *class_index).await?;
                self.skills.delete_class_skills(*char_id, *class_index).await?;
                self.subclasses.delete_subclass(*char_id, *class_index).await?;
            }
        }
        Ok(())
    }

    async fn load_class_index(&mut self, owner: ObjectId, class_index: i32) -> l2_world::ClassIndexLoad {
        let char_id = owner.0;
        let mut load = l2_world::ClassIndexLoad::default();
        // Java: restoreRecipeBook(false)
        match self.recipes.dwarven_recipes_of_class(char_id, class_index).await {
            Ok(ids) => load.dwarven_recipes = ids,
            Err(e) => error!("Could not restore recipe book data:{e}"),
        }
        // Java: restoreSkills (sorted like the login's, `enter.rs`)
        match self.skills.skills_of(char_id, class_index).await {
            Ok(mut rows) => {
                rows.sort_by_key(|s| s.skill_id);
                load.skills = rows
                    .into_iter()
                    .map(|s| SkillData {
                        skill_id: s.skill_id,
                        skill_level: s.skill_level,
                    })
                    .collect();
            }
            Err(e) => warn!("Could not restore character {char_id} skills: {e}"),
        }
        // Java: restoreEffects, then "Remove previously restored skills." (the rows are deleted).
        match self.skills.skill_effects_of(char_id, class_index).await {
            Ok(rows) => {
                load.effects = rows.iter().map(crate::client::enter::saved_effect_of).collect();
                if let Err(e) = self.skills.delete_skill_effects(char_id, class_index).await {
                    warn!("Could not restore {char_id} active effect data: {e}");
                }
            }
            Err(e) => warn!("Could not restore {char_id} active effect data: {e}"),
        }
        // Java: restoreHenna
        match self.hennas.hennas_of(char_id, class_index).await {
            Ok(rows) => load.hennas = crate::client::enter::henna_slots(&rows),
            Err(e) => error!("Failed restoing character {char_id} hennas. {e}"),
        }
        // Java: Shortcuts.restoreMe
        match self.shortcuts.shortcuts_of(char_id, class_index).await {
            Ok(rows) => {
                load.shortcuts = rows
                    .into_iter()
                    .map(|s| ShortcutData {
                        slot: s.slot,
                        page: s.page,
                        kind: s.kind.unwrap_or(0),
                        shortcut_id: s.shortcut_id.and_then(|v| i32::try_from(v).ok()).unwrap_or(0),
                        level: s.level.unwrap_or(0),
                    })
                    .collect();
            }
            Err(e) => warn!("Could not restore character shortcuts: {e}"),
        }
        // Java: Servitor.restoreEffects reads these when a servitor spawns; with the rest here.
        match self.summons.servitor_effects_of(char_id, class_index).await {
            Ok(rows) => {
                load.summon_effects = rows
                    .into_iter()
                    .map(|r| l2_world::SavedSummonEffect {
                        summon_skill_id: r.summon_skill_id,
                        skill_id: r.skill_id,
                        skill_level: r.skill_level,
                        remaining_time: r.remaining_time,
                    })
                    .collect();
            }
            Err(e) => warn!("Could not restore summon effect data of {char_id}: {e}"),
        }
        // From now on the character's skills and shortcuts are those of the new index: the
        // baseline follows (the store before the request wrote the old index's).
        match self.baselines.get_mut(&owner) {
            Some(baseline) => {
                baseline.skills = load.skills.iter().map(|s| (s.skill_id, s.skill_level)).collect();
                baseline.shortcuts = load.shortcuts.iter().map(|s| ((s.page, s.slot), *s)).collect();
                baseline.class_index = Some(class_index);
            }
            None => match self.load_baseline(char_id, Some(class_index)).await {
                Ok(baseline) => {
                    self.baselines.insert(owner, baseline);
                }
                Err(e) => warn!("Could not read the rows of character {char_id}: {e}"),
            },
        }
        load
    }

    async fn store_henna_write(&mut self, write: &HennaWrite) -> Result<(), PersistError> {
        match *write {
            HennaWrite::Add {
                char_id,
                symbol_id,
                slot,
                class_index,
            } => {
                self.hennas
                    .add_henna(char_id, symbol_id, slot, class_index)
                    .await?
            }
            HennaWrite::Delete {
                char_id,
                slot,
                class_index,
            } => self.hennas.delete_henna(char_id, slot, class_index).await?,
        }
        Ok(())
    }

    async fn store_seven_signs(
        &mut self,
        write: &l2_world::sevensigns::SevenSignsWrite,
    ) -> Result<(), PersistError> {
        use l2_world::sevensigns::SevenSignsWrite;
        match write {
            SevenSignsWrite::InsertPlayer { char_id, cabal, seal } => {
                self.seven_signs.insert_player(*char_id, cabal, *seal).await?
            }
            SevenSignsWrite::UpdatePlayer(row) => {
                self.seven_signs.update_player(&seven_signs_row(row)).await?
            }
            SevenSignsWrite::UpdateAllPlayers(rows) => {
                let rows: Vec<l2_db::SevenSignsRow> = rows.iter().map(seven_signs_row).collect();
                self.seven_signs.update_all_players(&rows).await?
            }
            SevenSignsWrite::UpdateStatus(status) => {
                self.seven_signs
                    .update_status(&seven_signs_status_row(status))
                    .await?
            }
            SevenSignsWrite::ReplaceFestival(rows) => {
                let rows: Vec<l2_db::FestivalRow> = rows.iter().map(festival_row).collect();
                self.seven_signs.replace_festival(&rows).await?
            }
        }
        Ok(())
    }

    async fn store_minigame_write(&mut self, write: &MinigameWrite) -> Result<(), PersistError> {
        match write {
            MinigameWrite::BestTeamInsert {
                arena_id,
                leader,
                wins,
            } => self.coliseum.insert_best_team(*arena_id, leader, *wins).await?,
            MinigameWrite::BestTeamUpdate { arena_id, wins } => {
                self.coliseum.update_best_team(*arena_id, *wins).await?
            }
            MinigameWrite::GlobalVariable { name, value } => {
                self.coliseum.set_variable_long(name, *value).await?
            }
        }
        Ok(())
    }

    async fn store_instance_time(&mut self, write: &InstanceTimeWrite) -> Result<(), PersistError> {
        match *write {
            InstanceTimeWrite::Set {
                char_id,
                instance_id,
                time,
            } => {
                self.instance_times
                    .set_instance_time(char_id, instance_id, time)
                    .await?
            }
            InstanceTimeWrite::Delete { char_id, instance_id } => {
                self.instance_times
                    .delete_instance_time(char_id, instance_id)
                    .await?
            }
        }
        Ok(())
    }

    async fn load_seven_signs(&mut self) -> Result<l2_world::sevensigns::SevenSignsLoad, PersistError> {
        read_seven_signs(&self.seven_signs).await
    }

    async fn store_wedding(&mut self, write: &l2_world::wedding::WeddingWrite) -> Result<(), PersistError> {
        use l2_world::wedding::WeddingWrite;
        match write {
            WeddingWrite::Insert(couple) => {
                self.wedding
                    .insert(&l2_db::CoupleRow {
                        id: couple.id,
                        player1_id: couple.player1_id,
                        player2_id: couple.player2_id,
                        married: couple.married,
                        affiance_date: couple.affiance_date,
                        wedding_date: couple.wedding_date,
                    })
                    .await?;
            }
            WeddingWrite::Marry { id, wedding_date } => self.wedding.marry(*id, *wedding_date).await?,
            WeddingWrite::Divorce { id } => self.wedding.delete(*id).await?,
        }
        Ok(())
    }

    async fn store_item_auction_write(&mut self, write: &ItemAuctionWrite) -> Result<(), PersistError> {
        match write {
            ItemAuctionWrite::Store {
                auction_id,
                instance_id,
                auction_item_id,
                starting_time,
                ending_time,
                state_id,
            } => {
                self.item_auctions
                    .store_auction(&l2_db::ItemAuctionRow {
                        auction_id: *auction_id,
                        instance_id: *instance_id,
                        auction_item_id: *auction_item_id,
                        starting_time: *starting_time,
                        ending_time: *ending_time,
                        auction_state_id: *state_id,
                    })
                    .await?
            }
            ItemAuctionWrite::Bid {
                auction_id,
                player_obj_id,
                bid,
            } => {
                self.item_auctions
                    .store_bid(*auction_id, *player_obj_id, *bid)
                    .await?
            }
            ItemAuctionWrite::DeleteBid {
                auction_id,
                player_obj_id,
            } => self.item_auctions.delete_bid(*auction_id, *player_obj_id).await?,
            ItemAuctionWrite::DeleteAuction { auction_id } => {
                self.item_auctions.delete_auction(*auction_id).await?
            }
        }
        Ok(())
    }

    async fn load_item_auction_records(
        &mut self,
        instance_id: i32,
    ) -> Result<Vec<ItemAuctionRecord>, PersistError> {
        Ok(crate::item_auctions::read_instance_records(&self.item_auctions, instance_id).await?)
    }

    async fn store_petition_feedback(
        &mut self,
        character: &str,
        gm: &str,
        rate: i32,
        message: &str,
        time: i64,
    ) -> Result<(), PersistError> {
        self.petition_feedback
            .insert(character, gm, rate, message, time)
            .await?;
        Ok(())
    }

    async fn store_bbs_favorite(&mut self, write: &FavoriteWrite) -> Result<(), PersistError> {
        match write {
            FavoriteWrite::Add {
                player_id,
                title,
                bypass,
            } => self.bbs_favorites.add_favorite(*player_id, title, bypass).await?,
            FavoriteWrite::Delete { player_id, fav_id } => {
                self.bbs_favorites.delete_favorite(*player_id, *fav_id).await?
            }
        }
        Ok(())
    }

    async fn load_bbs_favorites(
        &mut self,
        player: ObjectId,
        kind: FavoriteRead,
    ) -> Result<FavoriteAnswer, PersistError> {
        Ok(match kind {
            FavoriteRead::List => FavoriteAnswer::List(
                self.bbs_favorites
                    .favorites_of(player.0)
                    .await?
                    .into_iter()
                    .map(|row| FavoriteEntry {
                        fav_id: i64::from(row.fav_id),
                        title: row.title,
                        bypass: row.bypass,
                        added_ms: row.added_epoch_seconds.saturating_mul(1000),
                    })
                    .collect(),
            ),
            FavoriteRead::Count => FavoriteAnswer::Count(self.bbs_favorites.count_favorites(player.0).await?),
        })
    }

    async fn store_friend_write(&mut self, write: &FriendWrite) -> Result<(), PersistError> {
        let friends = &self.friends;
        match write {
            FriendWrite::AddFriends { first, second } => friends.add_friends(first.0, second.0).await?,
            FriendWrite::RemoveFriends { first, second } => friends.remove_friends(first.0, second.0).await?,
            FriendWrite::AddBlock { owner, target } => friends.add_block(owner.0, target.0).await?,
            FriendWrite::RemoveBlock { owner, target } => friends.remove_block(owner.0, target.0).await?,
        }
        Ok(())
    }

    async fn store_olympiad_write(&mut self, write: &OlympiadWrite) -> Result<(), PersistError> {
        let repo = &self.olympiad;
        match write {
            OlympiadWrite::SaveNobles(nobles) => {
                // Java writes one statement per noble and logs the failed ones.
                for (char_id, noble) in nobles {
                    if let Err(e) = repo
                        .save_noble(&olympiad_noble_row(*char_id, noble), noble.to_save)
                        .await
                    {
                        error!("Olympiad System: Failed to save noble data for charId {char_id}: {e}");
                    }
                }
            }
            OlympiadWrite::SaveData(data) => {
                repo.save_data(&l2_db::OlympiadDataRow {
                    current_cycle: data.current_cycle,
                    period: data.period,
                    olympiad_end: data.olympiad_end,
                    validation_end: data.validation_end,
                    next_weekly_change: data.next_weekly_change,
                })
                .await?
            }
            OlympiadWrite::DeleteNobles => repo.delete_nobles().await?,
            OlympiadWrite::UpdateMonthlyData => repo.update_monthly_data().await?,
            OlympiadWrite::UnclaimedPoints { char_id, points } => {
                repo.store_unclaimed_points(
                    *char_id,
                    l2_world::olympiad::UNCLAIMED_OLYMPIAD_PASSES_VAR,
                    *points,
                )
                .await?
            }
            OlympiadWrite::Hero(write) => self.store_hero_write(write).await?,
            OlympiadWrite::Fight(fight) => {
                repo.insert_fight(&l2_db::OlympiadFightRow {
                    char_one_id: fight.char_one_id,
                    char_two_id: fight.char_two_id,
                    char_one_class: fight.char_one_class,
                    char_two_class: fight.char_two_class,
                    winner: fight.winner,
                    start: fight.start,
                    time: fight.time,
                    classed: fight.classed,
                })
                .await?
            }
        }
        Ok(())
    }

    async fn store_vehicle_write(&mut self, write: &VehicleWrite) -> Result<(), PersistError> {
        let repo = &self.airships;
        match write {
            VehicleWrite::AddLicense { owner_id, fuel } => repo.insert(*owner_id, *fuel).await?,
            VehicleWrite::StoreFuel { owner_id, fuel } => repo.update_fuel(*owner_id, *fuel).await?,
        }
        Ok(())
    }

    async fn load_olympiad_reread(
        &mut self,
        request: &OlympiadRereadRequest,
    ) -> Result<OlympiadRereadRows, PersistError> {
        let repo = &self.olympiad;
        Ok(match request {
            OlympiadRereadRequest::ClassLeaderBoard {
                class_id,
                monthly,
                min_matches,
            } => OlympiadRereadRows::Names(repo.class_leader_board(*class_id, *monthly, *min_matches).await?),
            OlympiadRereadRequest::LastPoints { char_id } => {
                OlympiadRereadRows::Points(repo.last_noble_points(*char_id).await?)
            }
            OlympiadRereadRequest::UnclaimedPasses { char_id } => OlympiadRereadRows::Passes(
                repo.variable(*char_id, l2_world::olympiad::UNCLAIMED_OLYMPIAD_PASSES_VAR)
                    .await?,
            ),
            OlympiadRereadRequest::Heroes { before, only } => {
                OlympiadRereadRows::Heroes(Box::new(read_heroes(&self.heroes, *before, *only).await?))
            }
        })
    }

    async fn store_cursed_write(&mut self, write: &CursedWrite) -> Result<(), PersistError> {
        let repo = &self.cursed;
        match write {
            CursedWrite::Save { item_id, row } => {
                let row = row.as_ref().map(cursed_db_row);
                repo.save(*item_id, row.as_ref()).await?
            }
            CursedWrite::Remove { item_id } => repo.remove(*item_id).await?,
            CursedWrite::OfflineEnd {
                player_id,
                item_id,
                karma,
                pk_kills,
            } => {
                // Delete the item, then restore the karma. Java warns when a statement does not
                // change exactly one row, and goes on.
                if repo.delete_owner_item(*player_id, *item_id).await? != 1 {
                    warn!("Error while deleting itemId {item_id} from userId {player_id}");
                }
                if repo.restore_karma(*player_id, *karma, *pk_kills).await? != 1 {
                    warn!("Error while updating karma & pkkills for userId {player_id}");
                }
            }
        }
        Ok(())
    }

    async fn store_manor_write(&mut self, write: &ManorWrite) -> Result<(), PersistError> {
        let repo = &self.manor;
        match write {
            ManorWrite::StoreAll { production, procure } => {
                let production: Vec<ProductionRow> = production.iter().map(production_row).collect();
                let procure: Vec<ProcureRow> = procure.iter().map(procure_row).collect();
                repo.store_all(&production, &procure).await?
            }
            ManorWrite::ReplaceNextProduction { castle_id, rows } => {
                let rows: Vec<ProductionRow> = rows.iter().map(production_row).collect();
                repo.replace_next_production(*castle_id, &rows).await?
            }
            ManorWrite::ReplaceNextProcure { castle_id, rows } => {
                let rows: Vec<ProcureRow> = rows.iter().map(procure_row).collect();
                repo.replace_next_procure(*castle_id, &rows).await?
            }
            ManorWrite::UpdateCurrentProduction { castle_id, amounts } => {
                repo.update_current_production(*castle_id, amounts).await?
            }
            ManorWrite::UpdateCurrentProcure { castle_id, amounts } => {
                repo.update_current_procure(*castle_id, amounts).await?
            }
            ManorWrite::DeleteCastle { castle_id } => repo.delete_castle(*castle_id).await?,
        }
        Ok(())
    }

    async fn store_residence_write(&mut self, write: &ResidenceWrite) -> Result<(), PersistError> {
        let repo = &self.residences;
        match write {
            ResidenceWrite::CastleTreasury { castle_id, treasury } => {
                repo.update_castle_treasury(*castle_id, *treasury).await?
            }
            ResidenceWrite::CastleTaxPercent {
                castle_id,
                tax_percent,
            } => repo.update_castle_tax_percent(*castle_id, *tax_percent).await?,
            ResidenceWrite::CastleShowNpcCrest { castle_id, show } => {
                repo.update_castle_show_npc_crest(*castle_id, *show).await?
            }
            ResidenceWrite::CastleTicketBuyCount { castle_id, count } => {
                repo.update_castle_ticket_buy_count(*castle_id, *count).await?
            }
            ResidenceWrite::CastleOwner { castle_id, clan_id } => {
                repo.update_castle_owner(*castle_id, *clan_id).await?
            }
            ResidenceWrite::FortOwner {
                fort_id,
                owner,
                last_owned_time,
            } => repo.update_fort_owner(*fort_id, *owner, *last_owned_time).await?,
            ResidenceWrite::FortState {
                fort_id,
                state,
                castle_id,
            } => repo.update_fort_state(*fort_id, *state, *castle_id).await?,
            ResidenceWrite::FortSupplyLevel {
                fort_id,
                supply_level,
            } => repo.update_fort_supply_level(*fort_id, *supply_level).await?,
            ResidenceWrite::ClanHall {
                hall_id,
                owner_id,
                paid_until,
                paid,
            } => {
                repo.update_clan_hall(*hall_id, *owner_id, *paid_until, *paid)
                    .await?
            }
            ResidenceWrite::SiegableHall {
                hall_id,
                owner_id,
                next_siege,
            } => {
                repo.update_siegable_hall(*hall_id, *owner_id, *next_siege)
                    .await?
            }
            ResidenceWrite::FunctionSave {
                residence,
                function_type,
                lvl,
                lease,
                rate,
                end_time,
            } => {
                let save = l2_db::FunctionSave {
                    table: function_table(*residence),
                    residence_id: residence.residence_id(),
                    function_type: *function_type,
                    lvl: *lvl,
                    lease: *lease,
                    rate: *rate,
                    end_time: *end_time,
                };
                repo.save_function(&save).await?
            }
            ResidenceWrite::FunctionDelete {
                residence,
                function_type,
            } => {
                repo.delete_function(
                    function_table(*residence),
                    residence.residence_id(),
                    *function_type,
                )
                .await?
            }
            ResidenceWrite::DeleteCirclets { owner_id, item_id } => {
                repo.delete_circlets(*owner_id, *item_id).await?
            }
            ResidenceWrite::FortSiege(write) => {
                let repo = &self.fort_sieges;
                match write {
                    FortSiegeWrite::SiegeDate { fort_id, siege_date } => {
                        repo.update_siege_date(*fort_id, *siege_date).await?
                    }
                    FortSiegeWrite::AddAttacker { fort_id, clan_id } => {
                        repo.insert_siege_clan(*clan_id, *fort_id).await?
                    }
                    FortSiegeWrite::RemoveAttackers { fort_id, clan_id } => {
                        repo.delete_siege_clan(*fort_id, *clan_id).await?
                    }
                    FortSiegeWrite::ClearSiegeClans {
                        fort_id,
                        owner_clan_id,
                    } => repo.clear_siege_clans(*fort_id, *owner_clan_id).await?,
                    FortSiegeWrite::BloodOathCount { clan_id, count } => {
                        repo.update_blood_oath_count(*clan_id, *count).await?
                    }
                }
            }
            ResidenceWrite::HallSiege(write) => {
                let repo = &self.hall_sieges;
                match write {
                    HallSiegeWrite::SaveAttackers { hall_id, clans } => {
                        repo.save_attackers(*hall_id, clans).await?
                    }
                    HallSiegeWrite::FlagWarClan {
                        hall_id,
                        flag,
                        clan_id,
                    } => repo.insert_flag_war_clan(*hall_id, *flag, 0, *clan_id).await?,
                    HallSiegeWrite::FlagWarNpc { npc, clan_id } => {
                        repo.update_flag_war_npc(*npc, *clan_id).await?
                    }
                    HallSiegeWrite::FlagWarMember {
                        hall_id,
                        clan_id,
                        object_id,
                    } => {
                        repo.insert_flag_war_member(*hall_id, *clan_id, *object_id)
                            .await?
                    }
                    HallSiegeWrite::FlagWarClear { hall_id } => repo.clear_flag_war(*hall_id).await?,
                    HallSiegeWrite::RainbowAdd { clan_id, count } => {
                        repo.insert_rainbow_attacker(*clan_id, *count).await?
                    }
                    HallSiegeWrite::RainbowRemove { clan_id } => {
                        repo.delete_rainbow_attacker(*clan_id).await?
                    }
                    HallSiegeWrite::RainbowClear => repo.clear_rainbow_attackers().await?,
                }
            }
            ResidenceWrite::ClanHallAuction(write) => store_auction_write(&self.auctions, write).await?,
            // The Territory War tables (M7-TW-01).
            ResidenceWrite::Territory(write) => {
                let repo = &self.territories;
                match write {
                    TerritoryWrite::RegistrationInsert {
                        castle_id,
                        registered_id,
                    } => repo.insert_registration(*castle_id, *registered_id).await?,
                    TerritoryWrite::RegistrationDelete {
                        castle_id,
                        registered_id,
                    } => repo.delete_registration(*castle_id, *registered_id).await?,
                    TerritoryWrite::OwnedWardIds {
                        territory_id,
                        ward_ids,
                    } => repo.update_owned_ward_ids(*territory_id, ward_ids).await?,
                    TerritoryWrite::NextTwStartDate(start_ms) => {
                        repo.store_next_tw_start_date(*start_ms).await?
                    }
                }
            }
            // The siege statements (M7-SIEGE-01).
            write => store_siege_write(&self.sieges, write).await?,
        }
        Ok(())
    }

    async fn load_residence(
        &mut self,
        kind: ResidenceReadKind,
        residence_id: i32,
    ) -> Result<ResidenceRereadRows, PersistError> {
        let repo = &self.residences;
        let rows = match kind {
            ResidenceReadKind::Castle => {
                ResidenceRereadRows::Castle(read_castle_record(repo, residence_id).await?)
            }
            ResidenceReadKind::Fort => match repo.fort(residence_id).await? {
                Some(row) => {
                    let mut record = fort_record(row);
                    if record.owner_id > 0 {
                        record.functions =
                            function_records(repo.functions(FunctionTable::Fort, residence_id).await?);
                    }
                    ResidenceRereadRows::Fort(record)
                }
                None => ResidenceRereadRows::Missing,
            },
            ResidenceReadKind::FortSiege => match repo.fort(residence_id).await? {
                Some(row) => {
                    let mut fort = fort_record(row);
                    if fort.owner_id > 0 {
                        fort.functions =
                            function_records(repo.functions(FunctionTable::Fort, residence_id).await?);
                    }
                    let sieges = &self.fort_sieges;
                    let clans = sieges.siege_clans(residence_id).await?;
                    let owner_blood_oath_count = if fort.owner_id > 0 {
                        sieges.blood_oath_count(fort.owner_id).await?
                    } else {
                        None
                    };
                    ResidenceRereadRows::FortSiege(Box::new(FortSiegeRows {
                        fort,
                        clans,
                        owner_blood_oath_count,
                    }))
                }
                None => ResidenceRereadRows::Missing,
            },
            ResidenceReadKind::ClanHall => {
                match repo
                    .clan_halls()
                    .await?
                    .into_iter()
                    .find(|r| r.id == residence_id)
                {
                    Some(row) => {
                        let mut record = clan_hall_record(row);
                        if record.owner_id != 0 {
                            record.functions = function_records(
                                repo.functions(FunctionTable::ClanHall, residence_id).await?,
                            );
                        }
                        ResidenceRereadRows::ClanHall(record)
                    }
                    None => ResidenceRereadRows::Missing,
                }
            }
            ResidenceReadKind::Auction => ResidenceRereadRows::Auction {
                id: residence_id,
                record: read_auction_record(&self.auctions, residence_id).await?,
            },
            ResidenceReadKind::Auctions => {
                // Like `ClanHallAuctionManager.load()`: every `auction` id with its bids.
                let mut records = Vec::new();
                for id in self.auctions.auction_ids().await? {
                    if let Some(record) = read_auction_record(&self.auctions, id).await? {
                        records.push(record);
                    }
                }
                ResidenceRereadRows::Auctions(records)
            }
            ResidenceReadKind::SiegableHall => {
                match repo
                    .siegable_halls()
                    .await?
                    .into_iter()
                    .find(|r| r.clan_hall_id == residence_id)
                {
                    Some(row) => {
                        let mut record = siegable_hall_record(row);
                        if record.owner_id != 0 {
                            record.functions = function_records(
                                repo.functions(FunctionTable::ClanHall, residence_id).await?,
                            );
                        }
                        ResidenceRereadRows::SiegableHall(record)
                    }
                    None => ResidenceRereadRows::Missing,
                }
            }
            // M7-TW-01: the territories, the registrations and the next war date (D-50, D-51).
            ResidenceReadKind::Territory => {
                let repo = &self.territories;
                let territories = repo.territories().await?;
                let registrations = repo.registrations().await?;
                let next = repo.next_tw_start_date().await?;
                ResidenceRereadRows::Territory(Box::new(TerritoryRows {
                    territories: territories.into_iter().map(territory_record).collect(),
                    registrations: registrations
                        .into_iter()
                        .map(|r| TerritoryRegistrationRecord {
                            castle_id: r.castle_id,
                            registered_id: r.registered_id,
                        })
                        .collect(),
                    next_tw_start_date: parse_global_long(next.as_deref()),
                }))
            }
            // M7-CHSIEGE-01: a conquerable hall and its registration tables (D-50).
            ResidenceReadKind::HallSiege => {
                let hall = match repo
                    .siegable_halls()
                    .await?
                    .into_iter()
                    .find(|r| r.clan_hall_id == residence_id)
                {
                    Some(row) => {
                        let mut record = siegable_hall_record(row);
                        if record.owner_id != 0 {
                            record.functions = function_records(
                                repo.functions(FunctionTable::ClanHall, residence_id).await?,
                            );
                        }
                        Some(record)
                    }
                    None => None,
                };
                let halls = &self.hall_sieges;
                let mut flag_war = Vec::new();
                for row in halls.flag_war_clans(residence_id).await? {
                    flag_war.push(FlagWarRecord {
                        clan_id: row.clan_id,
                        flag: row.flag,
                        npc: row.npc,
                        members: halls.flag_war_members(row.clan_id).await?,
                    });
                }
                let rainbow = if residence_id == l2_world::clanhall::siege::RAINBOW_SPRINGS {
                    halls
                        .rainbow_attackers()
                        .await?
                        .into_iter()
                        .map(|r| (r.clan_id, r.war_decrees_count))
                        .collect()
                } else {
                    Vec::new()
                };
                ResidenceRereadRows::HallSiege(Box::new(HallSiegeRows {
                    hall_id: residence_id,
                    hall,
                    flag_war,
                    rainbow,
                }))
            }
            // M7-SIEGE-01: the castle and every registration (D-50).
            ResidenceReadKind::SiegeClans => {
                let castle = read_castle_record(repo, residence_id).await?;
                let rows = self.sieges.siege_clans().await?;
                ResidenceRereadRows::SiegeClans {
                    castle: castle.found.then_some(castle),
                    rows: rows.into_iter().map(siege_clan_record).collect(),
                }
            }
        };
        Ok(rows)
    }

    async fn store_clan_write(&mut self, write: &ClanWrite) -> Result<(), PersistError> {
        let clans = &self.clans;
        match write {
            ClanWrite::Insert(row) => clans.insert_clan(&new_clan_of(row)).await?,
            ClanWrite::Update(row) => clans.update_clan(&clan_update_of(row)).await?,
            ClanWrite::Level { clan_id, level } => clans.update_clan_level(*clan_id, *level).await?,
            ClanWrite::AllyCrestOfClan { clan_id, crest_id } => {
                clans.update_ally_crest_of_clan(*clan_id, *crest_id).await?
            }
            ClanWrite::Crest(write) => match write {
                CrestWrite::Insert {
                    crest_id,
                    data,
                    crest_type,
                } => self.crests.insert(*crest_id, data, *crest_type).await?,
                CrestWrite::Delete { crest_id } => self.crests.delete(*crest_id).await?,
                CrestWrite::ClanCrest { clan_id, crest_id } => {
                    clans.update_clan_crest(*clan_id, *crest_id).await?
                }
                CrestWrite::ClanLargeCrest { clan_id, crest_id } => {
                    clans.update_clan_large_crest(*clan_id, *crest_id).await?
                }
                CrestWrite::AllyCrestOfClan { clan_id, crest_id } => {
                    clans.update_ally_crest_of_clan(*clan_id, *crest_id).await?
                }
                CrestWrite::AllyCrestOfAlly { ally_id, crest_id } => {
                    clans.update_ally_crest_of_ally(*ally_id, *crest_id).await?
                }
            },
            ClanWrite::War(write) => match write {
                WarWrite::Store { clan1, clan2 } => clans.store_clan_war(*clan1, *clan2).await?,
                WarWrite::Delete { clan1, clan2 } => clans.delete_clan_war(*clan1, *clan2).await?,
            },
            ClanWrite::RemoveMember {
                char_id,
                clan_join_expiry_time,
                clan_create_expiry_time,
            } => {
                clans
                    .remove_member(*char_id, *clan_join_expiry_time, *clan_create_expiry_time)
                    .await?
            }
            ClanWrite::CharClanPrivs { char_id, privs } => {
                clans.update_char_clan_privs(*char_id, *privs).await?
            }
            ClanWrite::CharSubPledge { char_id, pledge_type } => {
                clans.update_char_subpledge(*char_id, *pledge_type).await?
            }
            ClanWrite::CharPowerGrade { char_id, power_grade } => {
                clans.update_char_power_grade(*char_id, *power_grade).await?
            }
            ClanWrite::CharApprenticeSponsor {
                char_id,
                apprentice,
                sponsor,
            } => {
                clans
                    .update_char_apprentice_sponsor(*char_id, *apprentice, *sponsor)
                    .await?
            }
            ClanWrite::InsertSubPledge {
                clan_id,
                sub_pledge_id,
                name,
                leader_id,
            } => {
                clans
                    .insert_sub_pledge(*clan_id, *sub_pledge_id, name, *leader_id)
                    .await?
            }
            ClanWrite::UpdateSubPledge {
                clan_id,
                sub_pledge_id,
                name,
                leader_id,
            } => {
                clans
                    .update_sub_pledge(*clan_id, *sub_pledge_id, name, *leader_id)
                    .await?
            }
            ClanWrite::RankPrivs { clan_id, rank, privs } => {
                clans.replace_rank_privs(*clan_id, *rank, *privs).await?
            }
            ClanWrite::Notice {
                clan_id,
                notice,
                enabled,
            } => clans.store_notice(*clan_id, notice, *enabled).await?,
            ClanWrite::Delete { clan_id, castle_id } => clans.delete_clan(*clan_id, *castle_id).await?,
            ClanWrite::UpdateSkill {
                clan_id,
                skill_id,
                skill_level,
            } => clans.update_clan_skill(*clan_id, *skill_id, *skill_level).await?,
            ClanWrite::InsertSkill {
                clan_id,
                skill_id,
                skill_level,
                skill_name,
                sub_pledge_id,
            } => {
                clans
                    .insert_clan_skill(*clan_id, *skill_id, *skill_level, skill_name, *sub_pledge_id)
                    .await?
            }
        }
        Ok(())
    }

    async fn player_offline(&mut self, object_id: ObjectId) -> Result<(), PersistError> {
        self.baselines.remove(&object_id);
        let now_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| i64::try_from(d.as_millis()).unwrap_or(i64::MAX));
        // Java: Player.updateOnlineStatus (UPDATE_ONLINE_STATUS: online and lastAccess)
        self.characters
            .update_online_status(object_id.0, 0, now_ms)
            .await?;
        info!("Character {} is offline.", object_id.0);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::*;

    /// S-SUB-01: the skills and shortcuts of a snapshot go to its class index; one that plays a
    /// class other than its base class under index 0 (the old model) writes none.
    #[test]
    fn the_class_index_of_a_snapshot() {
        let mut data = player(1, "a");
        assert_eq!(class_index_of(&data), Some(0));
        data.class_id = 16;
        assert_eq!(class_index_of(&data), None);
        data.profile.class_index = 2;
        assert_eq!(class_index_of(&data), Some(2));
    }

    /// Java `MacroList.registerMacroInDb`: `type,d1,d2[,cmd];` per command, no command text when it
    /// is empty, cut to 255 characters.
    #[test]
    fn macro_commands_are_written_like_java() {
        use l2_world::MacroCmdData;
        let cmd = |kind, d1, d2, text: &str| MacroCmdData {
            kind,
            d1,
            d2,
            cmd: text.into(),
        };
        let mut macro_data = MacroData {
            id: 1000,
            icon: 0,
            name: "n".into(),
            descr: String::new(),
            acronym: String::new(),
            commands: vec![cmd(1, 1011, 0, ""), cmd(3, 0, 0, "/sit")],
        };
        assert_eq!(macro_commands_text(&macro_data), "1,1011,0;3,0,0,/sit;");
        macro_data.commands = vec![cmd(3, 0, 0, &"y".repeat(400))];
        let text = macro_commands_text(&macro_data);
        assert_eq!(text.encode_utf16().count(), 255);
        assert!(text.starts_with("3,0,0,yy"));
        macro_data.commands.clear();
        assert_eq!(macro_commands_text(&macro_data), "");
    }

    #[derive(Debug, Clone, PartialEq)]
    enum Call {
        Store(String),
        Offline(i32),
        /// Item changes: owner, object ids written, object ids deleted.
        Items(i32, Vec<i32>, Vec<i32>),
        /// The rows of one `itemsonground` save (object ids).
        GroundItems(Vec<i32>),
        /// `ItemRows` (M6-WH-01): the tracked character and each op as (statement, owner, object id).
        ItemRows(Option<i32>, Vec<(&'static str, i32, i32)>),
        /// Skill changes: owner, (skill id, level) written, skill ids deleted.
        Skills(i32, Vec<(i32, i32)>, Vec<i32>),
        /// Recommendation counters: owner, rec_have, rec_left.
        RecoValues(i32, i32, i32),
        /// The daily reset of `character_reco_bonus`.
        ResetRecommends,
        /// Variables: owner, (name, value) written, names deleted.
        Variables(i32, Vec<(String, String)>, Vec<String>),
        /// A stored punishment: key, affect and type names.
        Punishment(String, &'static str, &'static str),
        /// An ended punishment: row id, key, affect and type names, time.
        EndPunishment(i32, String, &'static str, &'static str, i64),
        /// `changeCharAccessLevel`: character, level.
        AccessLevel(i32, i32),
        /// `AdminRepairChar`: the name.
        Repair(String),
        /// `Product.save`: list id, item id, count, next restock time.
        BuyList(i32, i32, i64, i64),
        /// `Mail` (M6-MAIL-01): the write.
        Mail(MailWrite),
        /// The read of the attachments of a mail: sender, message (M6-SYNC-01).
        MailAttachments(i32, i32),
        /// The read of the warehouse of a clan (M6-SYNC-01).
        ClanWarehouse(i32),
        /// The read of an offline receiver's block list (M6-FU-09).
        BlockList(i32),
        /// The read of the used crest ids (M7-FU-03).
        CrestIds,
        /// A crest insert (M7-FU-03): the id.
        CrestInsert(i32),
        /// One bot report row: bot, reporter, date (M6-FU-09).
        BotReport(i32, i32, i64),
        OfflineTrade(OfflineTradeWrite),
        /// The read of an offline owner's pet and inventory rows (M6-FU-09).
        OfflinePetRows(i32),
        /// A recipe book row.
        Recipe(RecipeWrite),
        /// The fishing championship: the end of the period and the number of rows.
        Championship(i64, usize),
        /// An `announcements` write (M7-ADMIN-10).
        Announcement(AnnouncementWrite),
        /// A `character_hennas` write (L-01).
        Henna(HennaWrite),
        /// An item auction row write (L-05).
        ItemAuction(ItemAuctionWrite),
        /// The re-read of an item auction instance (L-05).
        ItemAuctionRecords(i32),
        /// A boss row write (M8-BOSS-01).
        Boss(BossWrite),
        /// A read of boss rows (M8-BOSS-01).
        BossRead(BossReadKind),
        /// A `character_instance_time` write (M8-INST-01).
        InstanceTime(InstanceTimeWrite),
        /// An Underground Coliseum write (M8-GAMES-01).
        Minigame(MinigameWrite),
        /// A `character_tpbookmark` write (L-04).
        Bookmark(BookmarkWrite),
        /// A `character_contacts` write (L-04).
        Contact(ContactWrite),
        /// The `characters` row read of a friend (L-04).
        FriendSummary(i32),
        /// A cursed weapon write (M8-CURSED-01).
        Cursed(CursedWrite),
        /// An `airships` write (M8-VEHICLE-01).
        Vehicle(VehicleWrite),
        /// A `petition_feedback` row: character, GM, rate, message, time (M7-ADMIN-10).
        PetitionFeedback(String, String, i32, String, i64),
        /// A `bbs_favorites` write (S-HAND-01).
        BbsFavorite(FavoriteWrite),
        /// A `bbs_favorites` read (S-HAND-01).
        BbsFavoriteRead(i32, FavoriteRead),
    }

    /// Records the calls; stores can be made to fail, and are slow so ordering bugs show.
    #[derive(Clone, Default)]
    struct Recording {
        calls: Arc<Mutex<Vec<Call>>>,
        /// Names whose stores fail this many more times.
        failures: Arc<Mutex<HashMap<String, u32>>>,
    }

    impl PlayerStore for Recording {
        async fn store_player(&mut self, player: &PlayerData) -> Result<(), PersistError> {
            tokio::time::sleep(Duration::from_millis(20)).await;
            let mut failures = self.failures.lock().unwrap();
            if let Some(left) = failures.get_mut(&player.name).filter(|left| **left > 0) {
                *left -= 1;
                return Err(PersistError::NoSuchCharacter(player.object_id.0));
            }
            self.calls.lock().unwrap().push(Call::Store(player.name.clone()));
            Ok(())
        }

        async fn player_offline(&mut self, object_id: ObjectId) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Offline(object_id.0));
            Ok(())
        }

        async fn store_ground_items(&mut self, rows: &[GroundItemRow]) -> Result<(), PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::GroundItems(rows.iter().map(|r| r.object_id.0).collect()));
            Ok(())
        }

        async fn store_item_changes(
            &mut self,
            owner: ObjectId,
            upserts: &[ItemData],
            removed: &[ObjectId],
        ) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Items(
                owner.0,
                upserts.iter().map(|i| i.object_id.0).collect(),
                removed.iter().map(|i| i.0).collect(),
            ));
            Ok(())
        }

        async fn store_item_rows(
            &mut self,
            tracked: Option<ObjectId>,
            ops: &[ItemRowOp],
        ) -> Result<(), PersistError> {
            let ops = ops
                .iter()
                .map(|op| match op {
                    ItemRowOp::Write { owner_id, item } => ("write", *owner_id, item.object_id.0),
                    ItemRowOp::Update { owner_id, item } => ("update", *owner_id, item.object_id.0),
                    ItemRowOp::Insert { owner_id, item } => ("insert", *owner_id, item.object_id.0),
                    ItemRowOp::Delete { object_id } => ("delete", 0, object_id.0),
                })
                .collect();
            self.calls
                .lock()
                .unwrap()
                .push(Call::ItemRows(tracked.map(|t| t.0), ops));
            Ok(())
        }

        async fn store_skill_changes(
            &mut self,
            owner: ObjectId,
            upserts: &[SkillData],
            removed: &[i32],
        ) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Skills(
                owner.0,
                upserts.iter().map(|s| (s.skill_id, s.skill_level)).collect(),
                removed.to_vec(),
            ));
            Ok(())
        }

        async fn store_reco_values(
            &mut self,
            owner: ObjectId,
            rec_have: i32,
            rec_left: i32,
        ) -> Result<(), PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::RecoValues(owner.0, rec_have, rec_left));
            Ok(())
        }

        async fn reset_recommends(&mut self) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::ResetRecommends);
            Ok(())
        }

        async fn store_player_variables(
            &mut self,
            owner: ObjectId,
            upserts: &[(String, String)],
            removed: &[String],
        ) -> Result<(), PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Variables(owner.0, upserts.to_vec(), removed.to_vec()));
            Ok(())
        }

        async fn store_punishment(&mut self, punishment: &StoredPunishment) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Punishment(
                punishment.key.clone(),
                punishment.affect.name(),
                punishment.kind.name(),
            ));
            Ok(())
        }

        async fn end_punishment(
            &mut self,
            id: i32,
            key: &str,
            affect: PunishmentAffect,
            kind: PunishmentType,
            ended_at: i64,
        ) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::EndPunishment(
                id,
                key.to_owned(),
                affect.name(),
                kind.name(),
                ended_at,
            ));
            Ok(())
        }

        async fn change_char_access_level(&mut self, char_id: i32, level: i32) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::AccessLevel(char_id, level));
            Ok(())
        }

        async fn repair_char(&mut self, name: &str) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Repair(name.to_owned()));
            Ok(())
        }

        async fn store_mail_write(&mut self, write: &MailWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Mail(write.clone()));
            Ok(())
        }

        /// A mail 9 has one item row left; a mail below 0 cannot be read.
        async fn load_mail_attachments(
            &mut self,
            sender_id: i32,
            message_id: i32,
        ) -> Result<Vec<ItemData>, PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::MailAttachments(sender_id, message_id));
            if message_id < 0 {
                return Err(PersistError::NoSuchCharacter(sender_id));
            }
            Ok(if message_id == 9 {
                vec![ItemData {
                    object_id: ObjectId(7001),
                    item_id: 57,
                    count: 3,
                    loc: "MAIL".into(),
                    loc_data: 9,
                    ..Default::default()
                }]
            } else {
                Vec::new()
            })
        }

        async fn store_offline_trade(&mut self, write: &OfflineTradeWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::OfflineTrade(write.clone()));
            Ok(())
        }

        /// An insert gets the id 42; an announcement with the content "fail" cannot be stored.
        async fn store_announcement(&mut self, write: &AnnouncementWrite) -> Result<i32, PersistError> {
            self.calls.lock().unwrap().push(Call::Announcement(write.clone()));
            match write {
                AnnouncementWrite::Insert { content, .. } if content == "fail" => {
                    Err(PersistError::NoSuchCharacter(0))
                }
                AnnouncementWrite::Insert { .. } => Ok(42),
                _ => Ok(0),
            }
        }

        async fn store_henna_write(&mut self, write: &HennaWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Henna(*write));
            Ok(())
        }

        async fn store_boss_write(&mut self, write: &BossWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Boss(write.clone()));
            Ok(())
        }

        async fn load_boss_rows(&mut self, kind: BossReadKind) -> Result<BossRereadRows, PersistError> {
            self.calls.lock().unwrap().push(Call::BossRead(kind));
            Ok(match kind {
                BossReadKind::GrandBoss(boss_id) => BossRereadRows::GrandBoss { boss_id, row: None },
                BossReadKind::RaidPoints(char_id) => BossRereadRows::RaidPoints {
                    char_id,
                    rows: Vec::new(),
                },
            })
        }

        async fn store_instance_time(&mut self, write: &InstanceTimeWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::InstanceTime(*write));
            Ok(())
        }

        async fn store_minigame_write(&mut self, write: &MinigameWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Minigame(write.clone()));
            Ok(())
        }

        async fn store_bookmark_write(&mut self, write: &BookmarkWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Bookmark(write.clone()));
            Ok(())
        }

        async fn store_contact_write(&mut self, write: &ContactWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Contact(*write));
            Ok(())
        }

        /// Friend 7 has a row, friend 8 has none, a friend below 0 cannot be read.
        async fn load_friend_summary(&mut self, char_id: i32) -> Result<Option<FriendSummary>, PersistError> {
            self.calls.lock().unwrap().push(Call::FriendSummary(char_id));
            if char_id < 0 {
                return Err(PersistError::NoSuchCharacter(char_id));
            }
            Ok((char_id == 7).then(|| FriendSummary {
                object_id: 7,
                name: "Seven".into(),
                online: false,
                class_id: 18,
                level: 61,
            }))
        }

        async fn store_cursed_write(&mut self, write: &CursedWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Cursed(write.clone()));
            Ok(())
        }

        async fn store_vehicle_write(&mut self, write: &VehicleWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Vehicle(write.clone()));
            Ok(())
        }

        async fn store_item_auction_write(&mut self, write: &ItemAuctionWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::ItemAuction(write.clone()));
            Ok(())
        }

        /// Instance 7 has one auction with one bid; below 0 cannot be read.
        async fn load_item_auction_records(
            &mut self,
            instance_id: i32,
        ) -> Result<Vec<ItemAuctionRecord>, PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::ItemAuctionRecords(instance_id));
            if instance_id < 0 {
                return Err(PersistError::NoSuchCharacter(instance_id));
            }
            Ok(if instance_id == 7 {
                vec![ItemAuctionRecord {
                    auction_id: 3,
                    instance_id: 7,
                    auction_item_id: 1,
                    starting_time: 10,
                    ending_time: 20,
                    state_id: 1,
                    bids: vec![(1001, 500)],
                }]
            } else {
                Vec::new()
            })
        }

        async fn store_petition_feedback(
            &mut self,
            character: &str,
            gm: &str,
            rate: i32,
            message: &str,
            time: i64,
        ) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::PetitionFeedback(
                character.to_owned(),
                gm.to_owned(),
                rate,
                message.to_owned(),
                time,
            ));
            Ok(())
        }

        async fn store_bbs_favorite(&mut self, write: &FavoriteWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::BbsFavorite(write.clone()));
            Ok(())
        }

        /// Player 5 has one favorite; a player below 0 cannot be read.
        async fn load_bbs_favorites(
            &mut self,
            player: ObjectId,
            kind: FavoriteRead,
        ) -> Result<FavoriteAnswer, PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::BbsFavoriteRead(player.0, kind));
            if player.0 < 0 {
                return Err(PersistError::NoSuchCharacter(player.0));
            }
            Ok(match kind {
                FavoriteRead::List => FavoriteAnswer::List(if player.0 == 5 {
                    vec![FavoriteEntry {
                        fav_id: 1,
                        title: "Home".to_owned(),
                        bypass: "_bbshome".to_owned(),
                        added_ms: 1_000,
                    }]
                } else {
                    Vec::new()
                }),
                FavoriteRead::Count => FavoriteAnswer::Count(i64::from(player.0 == 5)),
            })
        }

        async fn store_bot_report(&mut self, row: &BotReportRow) -> Result<(), PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::BotReport(row.bot_id, row.reporter_id, row.report_date));
            Ok(())
        }

        /// A character 6 has one item on its pet and one in its inventory; a character below 0 cannot
        /// be read.
        async fn load_offline_pet_rows(&mut self, owner: ObjectId) -> Result<OfflinePetRows, PersistError> {
            self.calls.lock().unwrap().push(Call::OfflinePetRows(owner.0));
            if owner.0 < 0 {
                return Err(PersistError::NoSuchCharacter(owner.0));
            }
            let row = |object_id: i32, loc: &str| ItemData {
                object_id: ObjectId(object_id),
                item_id: 57,
                count: 1,
                loc: loc.into(),
                ..Default::default()
            };
            Ok(if owner.0 == 6 {
                OfflinePetRows {
                    pet_items: vec![row(7001, "PET")],
                    inventory: vec![row(7002, "INVENTORY")],
                }
            } else {
                OfflinePetRows::default()
            })
        }

        /// Crests 1 and 2 are used.
        async fn load_crest_ids(&mut self) -> Result<Vec<i32>, PersistError> {
            self.calls.lock().unwrap().push(Call::CrestIds);
            Ok(vec![1, 2])
        }

        /// The insert of crest 1 fails (the primary key is taken).
        async fn store_clan_write(&mut self, write: &ClanWrite) -> Result<(), PersistError> {
            if let ClanWrite::Crest(CrestWrite::Insert { crest_id, .. }) = write {
                self.calls.lock().unwrap().push(Call::CrestInsert(*crest_id));
                if *crest_id == 1 {
                    return Err(PersistError::NoSuchCharacter(*crest_id));
                }
            }
            Ok(())
        }

        /// A character 6 has character 4 on its block list; a character below 0 cannot be read.
        async fn load_block_list(&mut self, owner: i32) -> Result<Vec<i32>, PersistError> {
            self.calls.lock().unwrap().push(Call::BlockList(owner));
            if owner < 0 {
                return Err(PersistError::NoSuchCharacter(owner));
            }
            Ok(if owner == 6 { vec![4] } else { Vec::new() })
        }

        /// A clan 500 has one item; a clan below 0 cannot be read.
        async fn load_clan_warehouse(&mut self, clan_id: i32) -> Result<Vec<ItemData>, PersistError> {
            self.calls.lock().unwrap().push(Call::ClanWarehouse(clan_id));
            if clan_id < 0 {
                return Err(PersistError::NoSuchCharacter(clan_id));
            }
            Ok(if clan_id == 500 {
                vec![ItemData {
                    object_id: ObjectId(401),
                    item_id: 57,
                    count: 50,
                    loc: "CLANWH".into(),
                    ..Default::default()
                }]
            } else {
                Vec::new()
            })
        }

        async fn store_recipe_write(&mut self, write: &RecipeWrite) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::Recipe(write.clone()));
            Ok(())
        }

        async fn store_fishing_championship(&mut self, data: &ChampionshipData) -> Result<(), PersistError> {
            self.calls
                .lock()
                .unwrap()
                .push(Call::Championship(data.end_date, data.rows.len()));
            Ok(())
        }

        async fn store_buy_list_product(&mut self, row: &ProductRow) -> Result<(), PersistError> {
            self.calls.lock().unwrap().push(Call::BuyList(
                row.list_id,
                row.item_id,
                row.count,
                row.next_restock_time,
            ));
            Ok(())
        }
    }

    fn player(id: i32, name: &str) -> PlayerData {
        PlayerData {
            object_id: ObjectId(id),
            account: format!("acc{id}"),
            name: name.into(),
            title: String::new(),
            class_id: 0,
            base_class: 0,
            race: 0,
            sex: 0,
            face: 0,
            hair_style: 0,
            hair_color: 0,
            level: 1,
            exp: 0,
            sp: 0,
            karma: 0,
            pvp_kills: 0,
            pk_kills: 0,
            access_level: 0,
            x: 0,
            y: 0,
            z: 0,
            heading: 0,
            cur_hp: 10.0,
            max_hp: 10.0,
            cur_mp: 5.0,
            max_mp: 5.0,
            cur_cp: 0.0,
            max_cp: 5.0,
            online_time: 0,
            items: vec![],
            skills: vec![],
            shortcuts: vec![],
            profile: Default::default(),
        }
    }

    fn store_msg(id: i32, name: &str) -> OutboxMessage {
        OutboxMessage::Persist(PersistCommand::StorePlayer(Box::new(player(id, name))))
    }

    fn left_msg(id: i32) -> OutboxMessage {
        OutboxMessage::Persist(PersistCommand::PlayerLeft {
            account: format!("acc{id}"),
            object_id: ObjectId(id),
            logout: true,
        })
    }

    const NO_WAIT: RetryPolicy = RetryPolicy {
        attempts: 3,
        delay: Duration::ZERO,
    };

    /// Runs `messages` through the task and returns the store calls and the notices.
    async fn run_all(store: Recording, messages: Vec<OutboxMessage>) -> (Vec<Call>, Vec<LsNotice>) {
        let calls = Arc::clone(&store.calls);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, mut notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, LeftAcks::default());
        for message in messages {
            tx.send(message).unwrap();
        }
        drop(tx);
        // The drain: the task ends only after every queued message was handled.
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        let mut notices = Vec::new();
        while let Ok(n) = notice_rx.try_recv() {
            notices.push(n);
        }
        let calls = calls.lock().unwrap().clone();
        (calls, notices)
    }

    /// A pet row request is not a write: the store is asked and the answer goes back to the world
    /// as `WorldCommand::PetRowLoaded` (M6-PET-01).
    #[tokio::test]
    async fn a_pet_row_request_is_answered_to_the_world() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(Recording::default(), rx, notice_tx, NO_WAIT, acks);
        tx.send(OutboxMessage::Persist(PersistCommand::PetRowRequest {
            owner: ObjectId(1),
            control_object_id: ObjectId(77),
        }))
        .unwrap();
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        match world_rx.try_recv().unwrap() {
            l2_world::WorldCommand::PetRowLoaded {
                owner,
                control_object_id,
                result,
            } => {
                assert_eq!((owner, control_object_id), (ObjectId(1), ObjectId(77)));
                assert_eq!(
                    result,
                    Ok(PetRowData {
                        row: None,
                        effects: vec![]
                    })
                );
            }
            other => panic!("{other:?}"),
        }
    }

    /// A residence re-read (M7-RES-01, D-50) is not a write either: the store is asked and the
    /// rows go back to the world as `WorldCommand::ResidenceReread` with the follow-up the world
    /// named.
    #[tokio::test]
    async fn a_residence_reread_is_answered_to_the_world() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(Recording::default(), rx, notice_tx, NO_WAIT, acks);
        tx.send(OutboxMessage::Persist(PersistCommand::ResidenceRereadRequest {
            residence_id: 1,
            kind: ResidenceReadKind::Castle,
            then: l2_world::residence::ResidenceReadThen::Nothing,
        }))
        .unwrap();
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        match world_rx.try_recv().unwrap() {
            l2_world::WorldCommand::ResidenceReread { result, then } => {
                assert_eq!(result, Ok(ResidenceRereadRows::Missing));
                assert_eq!(then, l2_world::residence::ResidenceReadThen::Nothing);
            }
            other => panic!("{other:?}"),
        }
    }

    /// The boss writes reach the store in order, and a boss read (M8-BOSS-01, D-50) comes after
    /// the writes sent before it and goes back to the world as `WorldCommand::BossReread`.
    #[tokio::test]
    async fn the_boss_writes_and_reads_are_in_order_and_answered_to_the_world() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, acks);
        let send = |command: PersistCommand| tx.send(OutboxMessage::Persist(command)).unwrap();
        send(PersistCommand::Boss(BossWrite::GrandBossStatus {
            boss_id: 29028,
            status: 3,
        }));
        send(PersistCommand::BossRereadRequest {
            kind: BossReadKind::GrandBoss(29028),
            then: l2_world::BossReadThen::Nothing,
        });
        send(PersistCommand::Boss(BossWrite::RaidPointsDeleteAll));
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            *calls.lock().unwrap(),
            vec![
                Call::Boss(BossWrite::GrandBossStatus {
                    boss_id: 29028,
                    status: 3
                }),
                Call::BossRead(BossReadKind::GrandBoss(29028)),
                Call::Boss(BossWrite::RaidPointsDeleteAll),
            ]
        );
        match world_rx.try_recv().unwrap() {
            l2_world::WorldCommand::BossReread { result, then } => {
                assert_eq!(
                    result,
                    Ok(BossRereadRows::GrandBoss {
                        boss_id: 29028,
                        row: None
                    })
                );
                assert_eq!(then, l2_world::BossReadThen::Nothing);
            }
            other => panic!("{other:?}"),
        }
    }

    /// The two reads of M6-SYNC-01 are not writes: they come after the writes sent before them (the
    /// answer includes them) and go back to the world as commands; a failed read is `Err`.
    #[tokio::test]
    async fn the_mail_and_clan_warehouse_reads_are_answered_to_the_world_in_order() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, acks);
        let send = |command: PersistCommand| tx.send(OutboxMessage::Persist(command)).unwrap();
        send(PersistCommand::Mail(MailWrite::RemoveAttachments(8)));
        send(PersistCommand::MailAttachmentsRequest {
            player: ObjectId(1),
            message_id: 9,
            sender_id: 4,
        });
        send(PersistCommand::MailAttachmentsRequest {
            player: ObjectId(1),
            message_id: -1,
            sender_id: 4,
        });
        send(PersistCommand::ClanWarehouseRequest {
            player: ObjectId(2),
            clan_id: 500,
        });
        send(PersistCommand::ClanWarehouseRequest {
            player: ObjectId(2),
            clan_id: -1,
        });
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();

        assert_eq!(
            calls.lock().unwrap().clone(),
            vec![
                Call::Mail(MailWrite::RemoveAttachments(8)),
                Call::MailAttachments(4, 9),
                Call::MailAttachments(4, -1),
                Call::ClanWarehouse(500),
                Call::ClanWarehouse(-1),
            ]
        );
        let mut answers = Vec::new();
        while let Ok(command) = world_rx.try_recv() {
            answers.push(command);
        }
        assert_eq!(answers.len(), 4);
        match &answers[0] {
            l2_world::WorldCommand::MailAttachmentsLoaded {
                player,
                message_id,
                result,
            } => {
                assert_eq!((*player, *message_id), (ObjectId(1), 9));
                let items = result.as_ref().unwrap();
                assert_eq!(items.len(), 1);
                assert_eq!((items[0].object_id, items[0].loc_data), (ObjectId(7001), 9));
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            &answers[1],
            l2_world::WorldCommand::MailAttachmentsLoaded {
                message_id: -1,
                result: Err(()),
                ..
            }
        ));
        match &answers[2] {
            l2_world::WorldCommand::ClanWarehouseLoaded {
                player,
                clan_id,
                result,
            } => {
                assert_eq!((*player, *clan_id), (ObjectId(2), 500));
                assert_eq!(result.as_ref().unwrap().len(), 1);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            &answers[3],
            l2_world::WorldCommand::ClanWarehouseLoaded {
                clan_id: -1,
                result: Err(()),
                ..
            }
        ));
    }

    /// M6-FU-14: the offline trade writes go to the store in order with the other commands, and a
    /// player who goes offline frees its account (`PlayerLogout`) without a `PlayerLeft`.
    #[tokio::test]
    async fn offline_trade_writes_are_applied_in_order_and_the_account_is_freed() {
        let write = OfflineTradeWrite::Transaction {
            char_id: ObjectId(7),
            finished: true,
            items: Vec::new(),
            status: None,
        };
        let (calls, notices) = run_all(
            Recording::default(),
            vec![
                OutboxMessage::Persist(PersistCommand::OfflineTrade(write.clone())),
                OutboxMessage::Ls(LsEvent::PlayerLogout {
                    account: "acc".into(),
                }),
                OutboxMessage::Persist(PersistCommand::OfflineTrade(OfflineTradeWrite::Remove {
                    char_id: ObjectId(7),
                })),
            ],
        )
        .await;
        assert_eq!(
            calls,
            vec![
                Call::OfflineTrade(write),
                Call::OfflineTrade(OfflineTradeWrite::Remove { char_id: ObjectId(7) }),
            ]
        );
        assert_eq!(
            notices,
            vec![LsNotice::PlayerLogout {
                account: "acc".into()
            }]
        );
    }

    /// M6-FU-09: a bot report is one row, written in order with the other commands.
    #[tokio::test]
    async fn a_bot_report_is_one_row() {
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, LeftAcks::default());
        tx.send(OutboxMessage::Persist(PersistCommand::BotReport(BotReportRow {
            bot_id: 3,
            reporter_id: 4,
            report_date: 5_000_000_000,
        })))
        .unwrap();
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            calls.lock().unwrap().clone(),
            vec![Call::BotReport(3, 4, 5_000_000_000)]
        );
    }

    /// M7-FU-03: the used crest ids go back to the world, and an insert that fails is reported so
    /// the world retries once under a fresh read.
    #[tokio::test]
    async fn the_crest_ids_are_read_and_a_failed_insert_is_reported() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, acks);
        tx.send(OutboxMessage::Persist(PersistCommand::CrestIdsRequest {
            ticket: 7,
        }))
        .unwrap();
        for crest_id in [3, 1] {
            tx.send(OutboxMessage::Persist(PersistCommand::Clan(ClanWrite::Crest(
                CrestWrite::Insert {
                    crest_id,
                    data: vec![1],
                    crest_type: 1,
                },
            ))))
            .unwrap();
        }
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            calls.lock().unwrap().clone(),
            vec![Call::CrestIds, Call::CrestInsert(3), Call::CrestInsert(1)]
        );
        let answers: Vec<_> = world_rx.try_iter().collect();
        assert_eq!(answers.len(), 2);
        match &answers[0] {
            l2_world::WorldCommand::CrestIdsLoaded { ticket, result } => {
                assert_eq!(*ticket, 7);
                assert_eq!(result.as_ref().unwrap(), &vec![1, 2]);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            &answers[1],
            l2_world::WorldCommand::CrestInsertFailed { crest_id: 1 }
        ));
    }

    /// M6-FU-09: the block list read goes back to the world as a command, `Err` when it failed.
    #[tokio::test]
    async fn the_block_list_read_is_answered_to_the_world() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, acks);
        for owner in [6, -1] {
            tx.send(OutboxMessage::Persist(PersistCommand::BlockListRequest {
                sender: ObjectId(4),
                owner,
            }))
            .unwrap();
        }
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            calls.lock().unwrap().clone(),
            vec![Call::BlockList(6), Call::BlockList(-1)]
        );
        let answers: Vec<_> = world_rx.try_iter().collect();
        assert_eq!(answers.len(), 2);
        match &answers[0] {
            l2_world::WorldCommand::BlockListLoaded {
                sender,
                owner,
                result,
            } => {
                assert_eq!((*sender, *owner), (ObjectId(4), 6));
                assert_eq!(result.as_ref().unwrap(), &vec![4]);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            &answers[1],
            l2_world::WorldCommand::BlockListLoaded {
                owner: -1,
                result: Err(()),
                ..
            }
        ));
    }

    /// M6-FU-09: the rows of an offline owner whose pet decayed go back to the world; `Err` when the
    /// read failed.
    #[tokio::test]
    async fn the_offline_pet_read_is_answered_to_the_world() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, acks);
        for owner in [6, -1] {
            tx.send(OutboxMessage::Persist(PersistCommand::OfflinePetRowsRequest {
                owner: ObjectId(owner),
                control_object_id: ObjectId(9),
            }))
            .unwrap();
        }
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            calls.lock().unwrap().clone(),
            vec![Call::OfflinePetRows(6), Call::OfflinePetRows(-1)]
        );
        let answers: Vec<_> = world_rx.try_iter().collect();
        assert_eq!(answers.len(), 2);
        match &answers[0] {
            l2_world::WorldCommand::OfflinePetRowsLoaded {
                owner,
                control_object_id,
                result,
            } => {
                assert_eq!((*owner, *control_object_id), (ObjectId(6), ObjectId(9)));
                let rows = result.as_ref().unwrap();
                assert_eq!(rows.pet_items.len(), 1);
                assert_eq!(rows.inventory.len(), 1);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            &answers[1],
            l2_world::WorldCommand::OfflinePetRowsLoaded { result: Err(()), .. }
        ));
    }

    /// `Product.save` rows reach the store in the order the world sent them.
    #[tokio::test]
    async fn buy_list_rows_are_stored_in_order() {
        let row = |count: i64, next: i64| {
            OutboxMessage::Persist(PersistCommand::BuyList(ProductRow {
                list_id: 382,
                item_id: 1835,
                count,
                next_restock_time: next,
            }))
        };
        let (calls, _) = run_all(Recording::default(), vec![row(4, 1_700_000_000_000), row(5, 0)]).await;
        assert_eq!(
            calls,
            vec![
                Call::BuyList(382, 1835, 4, 1_700_000_000_000),
                Call::BuyList(382, 1835, 5, 0)
            ]
        );
    }

    /// M6-MAIL-01: the `messages` writes reach the store in the order the world sent them, the
    /// rows of the attachments in between.
    #[tokio::test]
    async fn mail_writes_are_stored_in_order() {
        let msgs = vec![
            OutboxMessage::Persist(PersistCommand::Mail(MailWrite::RemoveAttachments(5))),
            OutboxMessage::Persist(PersistCommand::Mail(MailWrite::MarkRead(6))),
            OutboxMessage::Persist(PersistCommand::Mail(MailWrite::Delete(5))),
        ];
        let (calls, _) = run_all(Recording::default(), msgs).await;
        assert_eq!(
            calls,
            vec![
                Call::Mail(MailWrite::RemoveAttachments(5)),
                Call::Mail(MailWrite::MarkRead(6)),
                Call::Mail(MailWrite::Delete(5)),
            ]
        );
    }

    /// A recipe learned and forgotten reaches the store in the order the world sent it.
    #[tokio::test]
    async fn recipe_book_rows_are_stored_in_order() {
        let insert = RecipeWrite::Insert {
            char_id: 7,
            recipe_id: 20,
            class_index: 1,
            dwarven: true,
        };
        let delete = RecipeWrite::Delete {
            char_id: 7,
            recipe_id: 20,
            class_index: 1,
        };
        let (calls, _) = run_all(
            Recording::default(),
            vec![
                OutboxMessage::Persist(PersistCommand::Recipe(insert.clone())),
                OutboxMessage::Persist(PersistCommand::Recipe(delete.clone())),
            ],
        )
        .await;
        assert_eq!(calls, vec![Call::Recipe(insert), Call::Recipe(delete)]);
    }

    /// The championship goes to the store as one command with the end of the period and every row.
    #[tokio::test]
    async fn the_fishing_championship_reaches_the_store() {
        let data = ChampionshipData {
            end_date: 1_792_000_000_000,
            rows: vec![
                ChampionshipRow {
                    name: "Alpha".into(),
                    length: 80.5,
                    rewarded: 1,
                },
                ChampionshipRow {
                    name: "Bravo".into(),
                    length: 70.25,
                    rewarded: 0,
                },
            ],
        };
        let (calls, _) = run_all(
            Recording::default(),
            vec![OutboxMessage::Persist(PersistCommand::FishingChampionship(
                Box::new(data),
            ))],
        )
        .await;
        assert_eq!(calls, vec![Call::Championship(1_792_000_000_000, 2)]);
    }

    #[tokio::test]
    async fn logout_is_announced_only_after_the_store_finished() {
        let (calls, notices) = run_all(Recording::default(), vec![store_msg(1, "Alpha"), left_msg(1)]).await;
        assert_eq!(calls, vec![Call::Store("Alpha".into()), Call::Offline(1)]);
        assert_eq!(
            notices,
            vec![LsNotice::PlayerLogout {
                account: "acc1".into()
            }]
        );
    }

    #[tokio::test]
    async fn the_notice_is_not_sent_while_the_store_is_still_running() {
        let store = Recording::default();
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, mut notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let calls = Arc::clone(&store.calls);
        let task = spawn(store, rx, notice_tx, NO_WAIT, LeftAcks::default());
        tx.send(store_msg(1, "Alpha")).unwrap();
        tx.send(left_msg(1)).unwrap();
        // The store sleeps 20 ms: right now nothing may have been announced.
        assert!(notice_rx.try_recv().is_err());
        let notice = tokio::time::timeout(Duration::from_secs(10), notice_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(notice, LsNotice::PlayerLogout { .. }));
        // When the notice arrives the store is already in the log.
        assert_eq!(calls.lock().unwrap().first(), Some(&Call::Store("Alpha".into())));
        drop(tx);
        task.await.unwrap();
    }

    fn item_msg(owner: i32, upserts: &[i32], removed: &[i32]) -> OutboxMessage {
        let item = |object_id: i32| ItemData {
            object_id: ObjectId(object_id),
            item_id: 57,
            count: 1,
            enchant_level: 0,
            loc: "INVENTORY".into(),
            loc_data: 0,
            custom_type1: 0,
            custom_type2: 0,
            mana_left: -1,
            time: -1,
            ..Default::default()
        };
        OutboxMessage::Persist(PersistCommand::ItemChanges {
            owner: ObjectId(owner),
            upserts: upserts.iter().map(|o| item(*o)).collect(),
            removed: removed.iter().map(|o| ObjectId(*o)).collect(),
        })
    }

    /// `Item.restoreAttributes`: the rows with -1 are the table's "nothing"; elements keep the
    /// order of the rows; an item without rows stays plain.
    #[test]
    fn restored_attribute_rows_follow_javas_rules() {
        use l2_db::{OwnerAugmentationRow, OwnerElementalRow};
        use l2_world::effects::item_extras::ItemElemental;

        let plain = |id: i32| ItemData {
            object_id: ObjectId(id),
            item_id: 57,
            ..ItemData::default()
        };
        let (mut a, mut b, mut c) = (plain(1), plain(2), plain(3));
        let augmentations = [
            OwnerAugmentationRow {
                item_id: 1,
                aug_attributes: 0x0002_0001,
            },
            OwnerAugmentationRow {
                item_id: 2,
                aug_attributes: -1,
            },
        ];
        let elementals = [
            OwnerElementalRow {
                item_id: 1,
                elem_type: 2,
                elem_value: 30,
            },
            OwnerElementalRow {
                item_id: 1,
                elem_type: 4,
                elem_value: 10,
            },
            OwnerElementalRow {
                item_id: 2,
                elem_type: -1,
                elem_value: 5,
            },
            OwnerElementalRow {
                item_id: 2,
                elem_type: 3,
                elem_value: -1,
            },
        ];
        apply_item_attribute_rows(&mut [&mut a, &mut b, &mut c], &augmentations, &elementals);
        assert_eq!(a.augmentation, Some(0x0002_0001));
        assert_eq!(
            a.elementals,
            [
                ItemElemental {
                    element: 2,
                    value: 30
                },
                ItemElemental {
                    element: 4,
                    value: 10
                }
            ]
        );
        assert_eq!((b.augmentation, b.elementals.len()), (None, 0));
        assert_eq!((c.augmentation, c.elementals.len()), (None, 0));
    }

    /// The element rows a store writes do not depend on the order the item holds them in, so a
    /// reordered list alone writes nothing.
    #[test]
    fn element_rows_are_ordered_by_element() {
        let mut item = ItemData::default();
        item.apply_attribute(4, 10);
        item.apply_attribute(1, 20);
        let rows: Vec<_> = element_rows(&item)
            .iter()
            .map(|r| (r.elem_type, r.elem_value))
            .collect();
        assert_eq!(rows, [(1, 20), (4, 10)]);
    }

    #[tokio::test]
    async fn item_changes_are_applied_in_order_between_the_stores() {
        let messages = vec![
            item_msg(1, &[10], &[]),
            store_msg(1, "A1"),
            item_msg(1, &[11, 12], &[10]),
            left_msg(1),
        ];
        let (calls, _) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Items(1, vec![10], vec![]),
                Call::Store("A1".into()),
                Call::Items(1, vec![11, 12], vec![10]),
                Call::Offline(1),
            ]
        );
    }

    /// M6-WH-01: the rows of a warehouse move reach the store in order with the other writes of
    /// the character, with the tracked character and the owner of each row.
    #[tokio::test]
    async fn item_rows_are_applied_in_order_between_the_stores() {
        let row = |id: i32| ItemData {
            object_id: ObjectId(id),
            ..ItemData::default()
        };
        let messages = vec![
            store_msg(1, "A1"),
            OutboxMessage::Persist(PersistCommand::ItemRows {
                tracked: Some(ObjectId(1)),
                ops: vec![
                    ItemRowOp::Write {
                        owner_id: 1,
                        item: row(10),
                    },
                    ItemRowOp::Insert {
                        owner_id: 500,
                        item: row(11),
                    },
                ],
            }),
            OutboxMessage::Persist(PersistCommand::ItemRows {
                tracked: None,
                ops: vec![ItemRowOp::Delete {
                    object_id: ObjectId(11),
                }],
            }),
            left_msg(1),
        ];
        let (calls, _) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Store("A1".into()),
                Call::ItemRows(Some(1), vec![("write", 1, 10), ("insert", 500, 11)]),
                Call::ItemRows(None, vec![("delete", 0, 11)]),
                Call::Offline(1),
            ]
        );
    }

    #[tokio::test]
    async fn a_repair_reaches_the_store_with_the_name() {
        let messages = vec![OutboxMessage::Persist(PersistCommand::RepairChar {
            name: "Stuck".into(),
        })];
        let (calls, _) = run_all(Recording::default(), messages).await;
        assert_eq!(calls, vec![Call::Repair("Stuck".into())]);
    }

    #[tokio::test]
    async fn punishment_writes_keep_their_order_and_the_login_server_hears_the_ban() {
        let messages = vec![
            OutboxMessage::Persist(PersistCommand::StorePunishment(StoredPunishment {
                key: "7".into(),
                affect: PunishmentAffect::Character,
                kind: PunishmentType::Ban,
                expiration: 0,
                reason: "r".into(),
                punished_by: "GM".into(),
            })),
            store_msg(7, "Seven"),
            left_msg(7),
            OutboxMessage::Persist(PersistCommand::CharAccessLevel {
                char_id: 7,
                level: -1,
            }),
            OutboxMessage::Ls(LsEvent::AccessLevel {
                account: "acc7".into(),
                level: -1,
            }),
            OutboxMessage::Persist(PersistCommand::EndPunishment {
                id: 0,
                key: "7".into(),
                affect: PunishmentAffect::Character,
                kind: PunishmentType::Ban,
                ended_at: 123,
            }),
        ];
        let (calls, notices) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Punishment("7".into(), "CHARACTER", "BAN"),
                Call::Store("Seven".into()),
                Call::Offline(7),
                Call::AccessLevel(7, -1),
                Call::EndPunishment(0, "7".into(), "CHARACTER", "BAN", 123),
            ]
        );
        assert_eq!(
            notices,
            vec![
                LsNotice::PlayerLogout {
                    account: "acc7".into()
                },
                LsNotice::AccessLevel {
                    account: "acc7".into(),
                    level: -1
                },
            ]
        );
    }

    #[tokio::test]
    async fn skill_changes_are_applied_in_order_between_the_stores() {
        let skills = |owner: i32, upserts: &[(i32, i32)], removed: &[i32]| {
            OutboxMessage::Persist(PersistCommand::SkillChanges {
                owner: ObjectId(owner),
                upserts: upserts
                    .iter()
                    .map(|&(skill_id, skill_level)| SkillData {
                        skill_id,
                        skill_level,
                    })
                    .collect(),
                removed: removed.to_vec(),
            })
        };
        let messages = vec![
            skills(1, &[(239, 1)], &[]),
            store_msg(1, "A1"),
            skills(1, &[(239, 2), (1216, 1)], &[3]),
            left_msg(1),
        ];
        let (calls, _) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Skills(1, vec![(239, 1)], vec![]),
                Call::Store("A1".into()),
                Call::Skills(1, vec![(239, 2), (1216, 1)], vec![3]),
                Call::Offline(1),
            ]
        );
    }

    #[tokio::test]
    async fn recommendation_writes_are_applied_in_order_with_the_stores() {
        let messages = vec![
            OutboxMessage::Persist(PersistCommand::RecoValues {
                owner: ObjectId(1),
                rec_have: 4,
                rec_left: 19,
            }),
            store_msg(1, "A1"),
            OutboxMessage::Persist(PersistCommand::ResetRecommends),
            OutboxMessage::Persist(PersistCommand::PlayerVariables {
                owner: ObjectId(1),
                upserts: vec![("hunting_time".into(), "0".into())],
                removed: vec!["nevit_b".into()],
            }),
            left_msg(1),
        ];
        let (calls, _) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::RecoValues(1, 4, 19),
                Call::Store("A1".into()),
                Call::ResetRecommends,
                Call::Variables(
                    1,
                    vec![("hunting_time".into(), "0".into())],
                    vec!["nevit_b".into()]
                ),
                Call::Offline(1),
            ]
        );
    }

    #[tokio::test]
    async fn ground_item_saves_are_applied_in_order_with_the_other_writes() {
        let ground = |ids: &[i32]| {
            OutboxMessage::Persist(PersistCommand::SaveGroundItems(
                ids.iter()
                    .map(|id| GroundItemRow {
                        object_id: ObjectId(*id),
                        item_id: 57,
                        count: 1,
                        enchant_level: 0,
                        x: 0,
                        y: 0,
                        z: 0,
                        drop_time: -1,
                        equipable: false,
                    })
                    .collect(),
            ))
        };
        let messages = vec![item_msg(1, &[10], &[]), ground(&[7, 8]), ground(&[]), left_msg(1)];
        let (calls, _) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Items(1, vec![10], vec![]),
                Call::GroundItems(vec![7, 8]),
                Call::GroundItems(vec![]),
                Call::Offline(1),
            ]
        );
    }

    #[tokio::test]
    async fn stores_of_one_character_keep_their_order() {
        let messages = vec![
            store_msg(1, "A1"),
            store_msg(2, "B1"),
            store_msg(1, "A2"),
            left_msg(2),
            store_msg(1, "A3"),
            left_msg(1),
        ];
        let (calls, notices) = run_all(Recording::default(), messages).await;
        assert_eq!(
            calls,
            vec![
                Call::Store("A1".into()),
                Call::Store("B1".into()),
                Call::Store("A2".into()),
                Call::Offline(2),
                Call::Store("A3".into()),
                Call::Offline(1),
            ]
        );
        assert_eq!(notices.len(), 2);
    }

    #[tokio::test]
    async fn login_server_events_keep_their_order_with_the_logouts() {
        let (_, notices) = run_all(
            Recording::default(),
            vec![
                OutboxMessage::Ls(LsEvent::PlayerInGame {
                    account: "acc1".into(),
                }),
                store_msg(1, "Alpha"),
                left_msg(1),
                OutboxMessage::Ls(LsEvent::PlayerInGame {
                    account: "acc1".into(),
                }),
            ],
        )
        .await;
        assert_eq!(
            notices,
            vec![
                LsNotice::PlayerInGame {
                    account: "acc1".into()
                },
                LsNotice::PlayerLogout {
                    account: "acc1".into()
                },
                LsNotice::PlayerInGame {
                    account: "acc1".into()
                },
            ]
        );
    }

    #[tokio::test]
    async fn a_failed_store_is_retried_and_the_logout_waits_for_it() {
        let store = Recording::default();
        store.failures.lock().unwrap().insert("Alpha".into(), 2);
        let (calls, notices) = run_all(store, vec![store_msg(1, "Alpha"), left_msg(1)]).await;
        // Two failures, the third try works (3 attempts).
        assert_eq!(calls, vec![Call::Store("Alpha".into()), Call::Offline(1)]);
        assert_eq!(notices.len(), 1);
    }

    #[tokio::test]
    async fn a_store_that_never_works_does_not_block_the_logout_forever() {
        let store = Recording::default();
        store.failures.lock().unwrap().insert("Alpha".into(), 100);
        let (calls, notices) = run_all(
            store,
            vec![
                store_msg(1, "Alpha"),
                left_msg(1),
                store_msg(2, "Beta"),
                left_msg(2),
            ],
        )
        .await;
        // Alpha is lost (logged as an error); Beta is unaffected and the LS hears both logouts.
        assert_eq!(
            calls,
            vec![Call::Offline(1), Call::Store("Beta".into()), Call::Offline(2)]
        );
        assert_eq!(notices.len(), 2);
    }

    #[tokio::test]
    async fn the_task_drains_everything_before_it_ends() {
        let mut messages = Vec::new();
        for id in 1..=20 {
            messages.push(store_msg(id, &format!("P{id}")));
            messages.push(left_msg(id));
        }
        let (calls, notices) = run_all(Recording::default(), messages).await;
        assert_eq!(calls.len(), 40);
        assert_eq!(notices.len(), 20);
    }

    struct NullSink;

    impl l2_world::PacketSink for NullSink {
        fn send(&self, _bytes: Vec<u8>) {}
        fn left_world(&self, _reason: l2_world::LeaveReason) {}
    }

    fn enter(handle: &crate::state::WorldHandle, session: u64, id: i32, name: &str) {
        assert!(handle.send(l2_world::WorldCommand::EnterWorld {
            session: l2_world::SessionId(session),
            player: Box::new(player(id, name)),
            sink: Box::new(NullSink),
            ip: None,
        }));
    }

    fn runtime_with(store: Recording) -> (crate::runtime::WorldRuntime, UnboundedReceiver<LsNotice>) {
        let (notice_tx, notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let runtime = crate::runtime::WorldRuntime::start_with_autosave(
            l2_util::WorldRng::from_seed(5),
            Duration::from_millis(200),
            move |outbox| spawn(store, outbox, notice_tx, NO_WAIT, LeftAcks::default()),
        )
        .unwrap();
        (runtime, notice_rx)
    }

    #[tokio::test]
    async fn a_logout_in_the_world_is_announced_after_its_store() {
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (runtime, mut notices) = runtime_with(store);
        let handle = runtime.handle();
        enter(&handle, 1, 0x6000_0001, "Alpha");
        assert!(handle.send(l2_world::WorldCommand::Input {
            session: l2_world::SessionId(1),
            input: l2_world::ClientInput::Logout,
        }));
        let first = tokio::time::timeout(Duration::from_secs(10), notices.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            first,
            LsNotice::PlayerInGame {
                account: "acc1610612737".into()
            }
        );
        let second = tokio::time::timeout(Duration::from_secs(10), notices.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            second,
            LsNotice::PlayerLogout {
                account: "acc1610612737".into()
            }
        );
        // The store is in the log by the time the logout is announced.
        assert_eq!(
            calls.lock().unwrap().clone(),
            vec![Call::Store("Alpha".into()), Call::Offline(0x6000_0001)]
        );
        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn autosave_goes_through_the_world_timer_to_the_store() {
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (runtime, _notices) = runtime_with(store);
        let handle = runtime.handle();
        enter(&handle, 1, 0x6000_0001, "Alpha");
        // The interval is 200 ms; wait for at least one autosave without a logout.
        let deadline = tokio::time::Instant::now() + Duration::from_secs(10);
        while calls.lock().unwrap().is_empty() {
            assert!(tokio::time::Instant::now() < deadline, "no autosave happened");
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
        assert_eq!(calls.lock().unwrap().first(), Some(&Call::Store("Alpha".into())));
        assert!(
            !calls
                .lock()
                .unwrap()
                .iter()
                .any(|c| matches!(c, Call::Offline(_)))
        );
        runtime.shutdown().await;
    }

    #[tokio::test]
    async fn shutdown_returns_only_after_every_player_is_stored_and_announced() {
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (runtime, mut notices) = runtime_with(store);
        let handle = runtime.handle();
        for (session, id, name) in [
            (1, 0x6000_0001, "Alpha"),
            (2, 0x6000_0002, "Beta"),
            (3, 0x6000_0003, "Gamma"),
        ] {
            enter(&handle, session, id, name);
        }
        // Not waiting for anything: the shutdown itself must drain the slow stores.
        tokio::time::timeout(Duration::from_secs(10), runtime.shutdown())
            .await
            .unwrap();
        let calls = calls.lock().unwrap().clone();
        let stores = calls.iter().filter(|c| matches!(c, Call::Store(_))).count();
        let offline = calls.iter().filter(|c| matches!(c, Call::Offline(_))).count();
        assert_eq!((stores, offline), (3, 3));
        let mut logouts = 0;
        while let Ok(notice) = notices.try_recv() {
            if matches!(notice, LsNotice::PlayerLogout { .. }) {
                logouts += 1;
            }
        }
        assert_eq!(logouts, 3);
    }

    #[test]
    fn autosave_interval_follows_general_ini() {
        let dir = tempfile::tempdir().unwrap();
        // Java default without the file or the key: 15 minutes.
        assert_eq!(autosave_interval(dir.path()), Duration::from_secs(900));
        std::fs::write(dir.path().join("General.ini"), "CharacterDataStoreInterval = 5\n").unwrap();
        assert_eq!(autosave_interval(dir.path()), Duration::from_secs(300));
        std::fs::write(dir.path().join("General.ini"), "CharacterDataStoreInterval = 0\n").unwrap();
        assert_eq!(autosave_interval(dir.path()), Duration::from_secs(1));
    }

    /// L-09: the same interval from the loaded configuration (`manual.core.general`).
    #[test]
    fn autosave_interval_from_the_loaded_config() {
        let dir = tempfile::tempdir().unwrap();
        let config = l2_config::GameConfig::load(dir.path()).unwrap();
        assert_eq!(autosave_interval_from_config(&config), Duration::from_secs(900));
        std::fs::write(dir.path().join("General.ini"), "CharacterDataStoreInterval = 3\n").unwrap();
        let config = l2_config::GameConfig::load(dir.path()).unwrap();
        assert_eq!(autosave_interval_from_config(&config), Duration::from_secs(180));
        assert_eq!(
            autosave_interval_from_config(&config),
            autosave_interval(dir.path())
        );
        std::fs::write(dir.path().join("General.ini"), "CharacterDataStoreInterval = 0\n").unwrap();
        let config = l2_config::GameConfig::load(dir.path()).unwrap();
        assert_eq!(autosave_interval_from_config(&config), Duration::from_secs(1));
    }

    /// M7-ADMIN-10: the login server switches of the admin commands reach the link in order.
    #[tokio::test]
    async fn the_login_server_switches_become_notices() {
        let (calls, notices) = run_all(
            Recording::default(),
            vec![
                OutboxMessage::Ls(LsEvent::SetServerStatus(5)),
                OutboxMessage::Ls(LsEvent::SetMaxPlayer(150)),
                OutboxMessage::Ls(LsEvent::ServerStatus { id: 2, value: 6 }),
                OutboxMessage::Ls(LsEvent::SetServerStatus(0)),
            ],
        )
        .await;
        assert!(calls.is_empty());
        assert_eq!(
            notices,
            vec![
                LsNotice::SetServerStatus(5),
                LsNotice::SetMaxPlayer(150),
                LsNotice::ServerStatus { id: 2, value: 6 },
                LsNotice::SetServerStatus(0),
            ]
        );
    }

    /// M7-ADMIN-10: the id of a new announcement comes back to the world; a failure too.
    #[tokio::test]
    async fn an_announcement_insert_is_answered_to_the_world() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let task = spawn(store, rx, notice_tx, NO_WAIT, acks);
        let insert = |ticket: u32, content: &str| AnnouncementWrite::Insert {
            ticket,
            kind: 0,
            content: content.to_owned(),
            author: "GM".to_owned(),
            auto: None,
        };
        for write in [
            insert(1, "hello"),
            insert(2, "fail"),
            AnnouncementWrite::Update {
                id: 42,
                kind: 0,
                content: "x".to_owned(),
                author: "GM".to_owned(),
                auto: None,
            },
            AnnouncementWrite::Delete { id: 42 },
        ] {
            tx.send(OutboxMessage::Persist(PersistCommand::Announcement(write)))
                .unwrap();
        }
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        let answers: Vec<_> = world_rx.try_iter().collect();
        assert!(matches!(
            answers.as_slice(),
            [
                l2_world::WorldCommand::AnnouncementStored {
                    ticket: 1,
                    result: Ok(42)
                },
                l2_world::WorldCommand::AnnouncementStored {
                    ticket: 2,
                    result: Err(())
                },
            ]
        ));
        // The update and the delete are writes without an answer.
        assert_eq!(calls.lock().unwrap().len(), 4);
    }

    /// M8-CURSED-01: the cursed weapon writes reach the store in order.
    #[tokio::test]
    async fn cursed_weapon_writes_are_stored_in_order() {
        let save = CursedWrite::Save {
            item_id: 8190,
            row: Some(CursedRow {
                item_id: 8190,
                char_id: 5,
                player_karma: 1,
                player_pk_kills: 2,
                nb_kills: 3,
                end_time: 4,
            }),
        };
        let offline = CursedWrite::OfflineEnd {
            player_id: 5,
            item_id: 8190,
            karma: 1,
            pk_kills: 2,
        };
        let remove = CursedWrite::Remove { item_id: 8190 };
        let (calls, _) = run_all(
            Recording::default(),
            vec![
                OutboxMessage::Persist(PersistCommand::Cursed(save.clone())),
                OutboxMessage::Persist(PersistCommand::Cursed(offline.clone())),
                OutboxMessage::Persist(PersistCommand::Cursed(remove.clone())),
            ],
        )
        .await;
        assert_eq!(
            calls.as_slice(),
            [Call::Cursed(save), Call::Cursed(offline), Call::Cursed(remove)]
        );
    }

    /// M8-GAMES-01: the Underground Coliseum writes reach the store in order.
    #[tokio::test]
    async fn minigame_writes_are_stored_in_order() {
        let insert = MinigameWrite::BestTeamInsert {
            arena_id: 5,
            leader: "Winner".to_owned(),
            wins: 2,
        };
        let update = MinigameWrite::BestTeamUpdate { arena_id: 5, wins: 3 };
        let variable = MinigameWrite::GlobalVariable {
            name: "UC_START_TIME".to_owned(),
            value: 1_800_000_000_000,
        };
        let (calls, _) = run_all(
            Recording::default(),
            vec![
                OutboxMessage::Persist(PersistCommand::Minigame(insert.clone())),
                OutboxMessage::Persist(PersistCommand::Minigame(update.clone())),
                OutboxMessage::Persist(PersistCommand::Minigame(variable.clone())),
            ],
        )
        .await;
        assert_eq!(
            calls.as_slice(),
            [
                Call::Minigame(insert),
                Call::Minigame(update),
                Call::Minigame(variable)
            ]
        );
    }

    /// L-01: a drawn and a removed dye are writes of their own, in order.
    #[tokio::test]
    async fn henna_writes_are_stored_in_order() {
        let add = HennaWrite::Add {
            char_id: 5,
            symbol_id: 9,
            slot: 2,
            class_index: 0,
        };
        let delete = HennaWrite::Delete {
            char_id: 5,
            slot: 2,
            class_index: 0,
        };
        let (calls, _) = run_all(
            Recording::default(),
            vec![
                OutboxMessage::Persist(PersistCommand::Henna(add)),
                OutboxMessage::Persist(PersistCommand::Henna(delete)),
            ],
        )
        .await;
        assert_eq!(calls, vec![Call::Henna(add), Call::Henna(delete)]);
    }

    /// M8-INST-01: a re-entry time set and one deleted are writes of their own, in order.
    #[tokio::test]
    async fn instance_time_writes_are_stored_in_order() {
        let set = InstanceTimeWrite::Set {
            char_id: 5,
            instance_id: 10,
            time: 1_800_000_000_000,
        };
        let delete = InstanceTimeWrite::Delete {
            char_id: 5,
            instance_id: 11,
        };
        let (calls, _) = run_all(
            Recording::default(),
            vec![
                OutboxMessage::Persist(PersistCommand::InstanceTime(set)),
                OutboxMessage::Persist(PersistCommand::InstanceTime(delete)),
            ],
        )
        .await;
        assert_eq!(calls, vec![Call::InstanceTime(set), Call::InstanceTime(delete)]);
    }

    /// L-04: bookmark and contact writes are writes of their own, in order.
    #[tokio::test]
    async fn bookmark_and_contact_writes_are_stored_in_order() {
        let bookmark = l2_world::BookmarkData {
            id: 1,
            x: 10,
            y: 20,
            z: 30,
            icon: 2,
            tag: "t".into(),
            name: "Home".into(),
        };
        let add = BookmarkWrite::Add { char_id: 5, bookmark };
        let update = BookmarkWrite::Update {
            char_id: 5,
            id: 1,
            icon: 3,
            tag: "u".into(),
            name: "Castle".into(),
        };
        let delete = BookmarkWrite::Delete { char_id: 5, id: 1 };
        let contact_add = ContactWrite::Add {
            char_id: 5,
            contact_id: 9,
        };
        let contact_remove = ContactWrite::Remove {
            char_id: 5,
            contact_id: 9,
        };
        let (calls, _) = run_all(
            Recording::default(),
            vec![
                OutboxMessage::Persist(PersistCommand::Bookmark(add.clone())),
                OutboxMessage::Persist(PersistCommand::Bookmark(update.clone())),
                OutboxMessage::Persist(PersistCommand::Contact(contact_add)),
                OutboxMessage::Persist(PersistCommand::Bookmark(delete.clone())),
                OutboxMessage::Persist(PersistCommand::Contact(contact_remove)),
            ],
        )
        .await;
        assert_eq!(
            calls,
            vec![
                Call::Bookmark(add),
                Call::Bookmark(update),
                Call::Contact(contact_add),
                Call::Bookmark(delete),
                Call::Contact(contact_remove),
            ]
        );
    }

    /// L-04: the friend rows are read one by one and handed to the world; a friend without a row
    /// or whose read failed is left out.
    #[tokio::test]
    async fn the_friend_summaries_are_answered_to_the_world() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, acks);
        tx.send(OutboxMessage::Persist(PersistCommand::FriendSummariesRequest {
            requester: ObjectId(4),
            friends: vec![7, 8, -1],
        }))
        .unwrap();
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            calls.lock().unwrap().clone(),
            vec![
                Call::FriendSummary(7),
                Call::FriendSummary(8),
                Call::FriendSummary(-1)
            ]
        );
        let answers: Vec<_> = world_rx.try_iter().collect();
        assert_eq!(answers.len(), 1);
        match &answers[0] {
            l2_world::WorldCommand::FriendSummariesLoaded { requester, rows } => {
                assert_eq!(*requester, ObjectId(4));
                assert_eq!(rows.len(), 1);
                assert_eq!((rows[0].object_id, rows[0].name.as_str()), (7, "Seven"));
            }
            other => panic!("{other:?}"),
        }
    }

    /// L-05: the item auction rows are writes in order, and the re-read of an instance is an
    /// answer to the world, in order with the writes before it (a failed read is `Err`).
    #[tokio::test]
    async fn item_auction_rows_and_the_reread_are_in_order() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let bid = ItemAuctionWrite::Bid {
            auction_id: 3,
            player_obj_id: 1001,
            bid: 500,
        };
        let then = l2_world::ItemAuctionThen::Info {
            player: ObjectId(1001),
        };
        let calls = Arc::new(Mutex::new(Vec::new()));
        let store = Recording {
            calls: Arc::clone(&calls),
            ..Recording::default()
        };
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, acks);
        for message in [
            OutboxMessage::Persist(PersistCommand::ItemAuction(bid.clone())),
            OutboxMessage::Persist(PersistCommand::ItemAuctionRequest { instance_id: 7, then }),
            OutboxMessage::Persist(PersistCommand::ItemAuctionRequest {
                instance_id: -1,
                then,
            }),
        ] {
            tx.send(message).unwrap();
        }
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            calls.lock().unwrap().clone(),
            vec![
                Call::ItemAuction(bid),
                Call::ItemAuctionRecords(7),
                Call::ItemAuctionRecords(-1),
            ]
        );
        match world_rx.try_recv().unwrap() {
            l2_world::WorldCommand::ItemAuctionReread {
                instance_id,
                result,
                then: answered,
            } => {
                assert_eq!((instance_id, answered), (7, then));
                let rows = result.unwrap();
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0].bids, vec![(1001, 500)]);
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            world_rx.try_recv().unwrap(),
            l2_world::WorldCommand::ItemAuctionReread {
                instance_id: -1,
                result: Err(()),
                ..
            }
        ));
    }

    /// M7-ADMIN-10: a petition feedback is a write of its own.
    #[tokio::test]
    async fn petition_feedback_is_stored() {
        let (calls, _) = run_all(
            Recording::default(),
            vec![OutboxMessage::Persist(PersistCommand::PetitionFeedback {
                character: "Ann".to_owned(),
                gm: "Bob".to_owned(),
                rate: 4,
                message: "thanks".to_owned(),
                time: 99,
            })],
        )
        .await;
        assert_eq!(
            calls,
            vec![Call::PetitionFeedback(
                "Ann".to_owned(),
                "Bob".to_owned(),
                4,
                "thanks".to_owned(),
                99
            )]
        );
    }

    /// S-HAND-01: a favorite write and a read after it go in order, and the read is answered to the
    /// world (`Err` when it failed).
    #[tokio::test]
    async fn favorite_writes_and_reads_are_in_order_and_the_read_is_answered() {
        let (world_tx, world_rx) = std::sync::mpsc::channel();
        let acks = LeftAcks::default();
        acks.set_world(crate::state::WorldHandle::new(world_tx));
        let store = Recording::default();
        let calls = Arc::clone(&store.calls);
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(store, rx, notice_tx, NO_WAIT, acks);
        let add = FavoriteWrite::Add {
            player_id: 5,
            title: "Home".to_owned(),
            bypass: "_bbshome".to_owned(),
        };
        tx.send(OutboxMessage::Persist(PersistCommand::BbsFavorite(add.clone())))
            .unwrap();
        for (player, kind) in [
            (5, FavoriteRead::List),
            (5, FavoriteRead::Count),
            (-1, FavoriteRead::List),
        ] {
            tx.send(OutboxMessage::Persist(PersistCommand::BbsFavoriteRequest {
                player: ObjectId(player),
                kind,
            }))
            .unwrap();
        }
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            calls.lock().unwrap().clone(),
            vec![
                Call::BbsFavorite(add),
                Call::BbsFavoriteRead(5, FavoriteRead::List),
                Call::BbsFavoriteRead(5, FavoriteRead::Count),
                Call::BbsFavoriteRead(-1, FavoriteRead::List),
            ]
        );
        let answers: Vec<_> = world_rx.try_iter().collect();
        assert_eq!(answers.len(), 3);
        match &answers[0] {
            l2_world::WorldCommand::BbsFavoritesLoaded { player, kind, result } => {
                assert_eq!((*player, *kind), (ObjectId(5), FavoriteRead::List));
                match result.as_ref().unwrap() {
                    FavoriteAnswer::List(rows) => assert_eq!(rows[0].bypass, "_bbshome"),
                    other => panic!("{other:?}"),
                }
            }
            other => panic!("{other:?}"),
        }
        assert!(matches!(
            &answers[1],
            l2_world::WorldCommand::BbsFavoritesLoaded {
                result: Ok(FavoriteAnswer::Count(1)),
                ..
            }
        ));
        assert!(matches!(
            &answers[2],
            l2_world::WorldCommand::BbsFavoritesLoaded { result: Err(()), .. }
        ));
    }

    /// M7-ADMIN-10: the requests of the world that are not its to do go to the control handler.
    #[tokio::test]
    async fn control_requests_reach_the_handler() {
        let seen = Arc::new(Mutex::new(Vec::new()));
        let acks = LeftAcks::default();
        // Without a handler a request is dropped.
        acks.control(l2_world::ControlRequest::Shutdown { restart: false });
        let sink = Arc::clone(&seen);
        acks.set_control(Arc::new(move |request| sink.lock().unwrap().push(request)));
        let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
        let (notice_tx, _notice_rx) = tokio::sync::mpsc::unbounded_channel();
        let task = spawn(Recording::default(), rx, notice_tx, NO_WAIT, acks);
        tx.send(OutboxMessage::Control(l2_world::ControlRequest::Shutdown {
            restart: true,
        }))
        .unwrap();
        tx.send(OutboxMessage::Control(l2_world::ControlRequest::ReloadHtml {
            requester: ObjectId(7),
            path: Some("admin/x.htm".to_owned()),
        }))
        .unwrap();
        drop(tx);
        tokio::time::timeout(Duration::from_secs(10), task)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            seen.lock().unwrap().as_slice(),
            [
                l2_world::ControlRequest::Shutdown { restart: true },
                l2_world::ControlRequest::ReloadHtml {
                    requester: ObjectId(7),
                    path: Some("admin/x.htm".to_owned())
                },
            ]
        );
    }
}
