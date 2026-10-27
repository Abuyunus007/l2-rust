//! Game server tables, one module per table group (same pattern as [`crate::login`]).
//!
//! `characters` is the reference for the others: a repository struct holding the [`crate::Pool`],
//! the Java SQL as constants with a `// Java:` comment, runtime queries, row structs that
//! mirror the column types.

mod announcements;
mod auctions;
mod bot_reports;
mod buylists;
mod characters;
mod clans;
mod crests;
mod fishing;
mod fort_sieges;
mod friends;
mod ground_items;
mod hall_sieges;
mod hennas;
mod ids;
mod item_reuse;
mod items;
mod macros;
mod manor;
mod messages;
mod offline_trades;
mod petition_feedback;
mod pets;
mod punishments;
mod recipes;
mod recommendations;
mod residences;
mod shortcuts;
mod sieges;
mod skills;
mod sql_number;
mod subclasses;
mod summons;
mod territories;
mod variables;
mod wedding;

pub use announcements::{AnnouncementRepository, AnnouncementRow, AutoColumns};
pub use auctions::{AuctionBidRow, AuctionRepository, AuctionRow, NewAuction};
pub use bot_reports::{BotReportDbRow, BotReportRepository};
pub use buylists::{BuyListRepository, BuyListRow};
pub use characters::{
    CalendarDate, CharacterIdName, CharacterNameAccess, CharacterRepository, CharacterRow, CharacterUpdate,
    NewCharacter,
};
pub use clans::{
    ClanDataRow, ClanMemberRow, ClanNoticeRow, ClanPrivsRow, ClanRepository, ClanSkillRow, ClanSubPledgeRow,
    ClanUpdate, ClanWarRow, NewClan,
};
pub use crests::{CrestRepository, CrestRow};
pub use fishing::{FISH_CHAMPIONSHIP_END, FishingChampionshipRepository, FishingChampionshipRow};
pub use fort_sieges::{FortSiegeGuardRow, FortSiegeRepository, FortSpawnRow};
pub use friends::{FriendLink, FriendRepository};
pub use ground_items::{GroundItemRecord, GroundItemRepository};
pub use hall_sieges::{FlagWarClanRow, HallSiegeGuardRow, HallSiegeRepository, RainbowAttackerRow};
pub use hennas::{HennaRepository, HennaRow};
pub use ids::{IdRepository, UsedIds};
pub use item_reuse::{ItemReuseRepository, ItemReuseRow, NewItemReuse};
pub use items::{
    ItemElementalRow, ItemLocation, ItemRepository, ItemRow, ItemUpdate, NewItem, OwnerAugmentationRow,
    OwnerElementalRow, PaperdollItemRow,
};
pub use macros::{MacroRepository, MacroRow, NewMacro};
pub use manor::{ManorRepository, ProcureRow, ProductionRow};
pub use messages::{MessageRepository, MessageRow};
pub use offline_trades::{OfflineTradeItemRow, OfflineTradeRepository, OfflineTradeStatusRow};
pub use petition_feedback::PetitionFeedbackRepository;
pub use pets::{NewPetSkillSave, PetRepository, PetRow, PetSkillSaveRow, PetValues};
pub use punishments::{NewPunishment, PunishmentRepository, PunishmentRow};
pub use recipes::{RecipeBookRow, RecipeRepository, RecipeShopRow};
pub use recommendations::{RecoBonus, RecommendationRepository};
pub use residences::{
    CastleRow, ClanHallRow, FortRow, FunctionRow, FunctionSave, FunctionTable, ResidenceRepository,
    SiegableHallRow,
};
pub use shortcuts::{NewShortcut, ShortcutRepository, ShortcutRow};
pub use sieges::{
    DoorUpgradeRow, NewSiegeGuard, SiegeClanRow, SiegeGuardRow, SiegeRepository, TrapUpgradeRow,
};
pub use skills::{NewSkillSave, SkillLevel, SkillRepository, SkillSaveRow};
pub use subclasses::{SubclassRepository, SubclassRow, SubclassValues};
pub use summons::{NewSummonSkillSave, SummonRepository, SummonSkillSaveRow};
pub use territories::{
    NEXT_TW_START_DATE, TerritoryRegistrationRow, TerritoryRepository, TerritoryRow, TerritorySpawnRow,
};
pub use variables::VariableRepository;
pub use wedding::{CoupleRow, WeddingRepository, married_text};
