//! Database access: the sqlx pool and one repository per table group, on the unchanged
//! Java schema (D-21).
//!
//! Pattern for repositories (copy it for the game tables): a struct holding a [`Pool`], one
//! `async` method per Java SQL statement, the SQL text as a constant next to a
//! `// Java: Class.CONSTANT` comment, runtime queries (`query_as` + `FromRow`), and row
//! structs whose field types match the column types.

mod error;
mod game;
mod login;
mod pool;
mod schema;

pub use error::{DbError, Result};
pub use game::PetitionFeedbackRepository;
pub use game::{
    AccountDataRepository, AccountDataRow, VAR_PWD, VAR_WTE, crypt_secondary_password, sha1_digest,
};
pub use game::{AnnouncementRepository, AnnouncementRow, AutoColumns};
pub use game::{AuctionBidRow, AuctionRepository, AuctionRow, NewAuction};
pub use game::{BookmarkRepository, BookmarkRow, ContactLink, ContactRepository};
pub use game::{BotReportDbRow, BotReportRepository};
pub use game::{BuyListRepository, BuyListRow};
pub use game::{
    CalendarDate, CharacterIdName, CharacterNameAccess, CharacterRepository, CharacterRow, CharacterUpdate,
    IdRepository, NewCharacter, UsedIds,
};
pub use game::{
    CastleRow, ClanHallRow, FortRow, FunctionRow, FunctionSave, FunctionTable, ResidenceRepository,
    SiegableHallRow,
};
pub use game::{CharacterSummary, FriendLink, FriendRepository};
pub use game::{
    ClanDataRow, ClanMemberRow, ClanNoticeRow, ClanPrivsRow, ClanRepository, ClanSkillRow, ClanSubPledgeRow,
    ClanUpdate, ClanWarRow, MacroRepository, MacroRow, NewClan, NewMacro, RecoBonus,
    RecommendationRepository, VariableRepository,
};
pub use game::{CoupleRow, WeddingRepository, married_text};
pub use game::{CrestRepository, CrestRow};
pub use game::{CursedWeaponRepository, CursedWeaponRow};
pub use game::{DoorUpgradeRow, NewSiegeGuard, SiegeClanRow, SiegeGuardRow, SiegeRepository, TrapUpgradeRow};
pub use game::{FISH_CHAMPIONSHIP_END, FishingChampionshipRepository, FishingChampionshipRow};
pub use game::{FlagWarClanRow, HallSiegeGuardRow, HallSiegeRepository, RainbowAttackerRow};
pub use game::{FortSiegeGuardRow, FortSiegeRepository, FortSpawnRow};
pub use game::{GroundItemRecord, GroundItemRepository};
pub use game::{HennaRepository, HennaRow};
pub use game::{InstanceTimeRepository, InstanceTimeRow};
pub use game::{
    ItemElementalRow, ItemLocation, ItemRepository, ItemRow, ItemUpdate, NewItem, OwnerAugmentationRow,
    OwnerElementalRow, PaperdollItemRow,
};
pub use game::{ItemReuseRepository, ItemReuseRow, NewItemReuse};
pub use game::{ManorRepository, ProcureRow, ProductionRow};
pub use game::{MessageRepository, MessageRow};
pub use game::{
    NEXT_TW_START_DATE, TerritoryRegistrationRow, TerritoryRepository, TerritoryRow, TerritorySpawnRow,
};
pub use game::{NewPetSkillSave, PetRepository, PetRow, PetSkillSaveRow, PetValues};
pub use game::{NewPunishment, PunishmentRepository, PunishmentRow};
pub use game::{NewShortcut, ShortcutRepository, ShortcutRow};
pub use game::{NewSkillSave, SkillLevel, SkillRepository, SkillSaveRow};
pub use game::{NewSummonSkillSave, SummonRepository, SummonSkillSaveRow};
pub use game::{OfflineTradeItemRow, OfflineTradeRepository, OfflineTradeStatusRow};
pub use game::{RecipeBookRow, RecipeRepository, RecipeShopRow};
pub use game::{SubclassRepository, SubclassRow, SubclassValues};
pub use login::{
    AccountInfoRow, ChangePasswordResult, GameServerRow, IpAuthKind, IpAuthRow, LoginRepository, TracertHops,
    hash_password, hex_to_string, string_to_hex,
};
pub use pool::{Pool, connect, connect_options, connect_with, pool_max_size, pool_min_idle};
pub use schema::GAME_SCHEMA;
