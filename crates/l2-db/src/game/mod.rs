//! Game server tables, one module per table group (same pattern as [`crate::login`]).
//!
//! `characters` is the reference for the others: a repository struct holding the [`crate::Pool`],
//! the Java SQL as constants with a `// Java:` comment, runtime queries, row structs that
//! mirror the column types.

mod buylists;
mod characters;
mod clans;
mod crests;
mod fishing;
mod friends;
mod ground_items;
mod hennas;
mod ids;
mod item_reuse;
mod items;
mod macros;
mod messages;
mod pets;
mod punishments;
mod recipes;
mod recommendations;
mod shortcuts;
mod skills;
mod sql_number;
mod subclasses;
mod summons;
mod variables;

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
pub use friends::{FriendLink, FriendRepository};
pub use ground_items::{GroundItemRecord, GroundItemRepository};
pub use hennas::{HennaRepository, HennaRow};
pub use ids::{IdRepository, UsedIds};
pub use item_reuse::{ItemReuseRepository, ItemReuseRow, NewItemReuse};
pub use items::{
    ItemElementalRow, ItemLocation, ItemRepository, ItemRow, ItemUpdate, NewItem, OwnerAugmentationRow,
    OwnerElementalRow, PaperdollItemRow,
};
pub use macros::{MacroRepository, MacroRow, NewMacro};
pub use messages::{MessageRepository, MessageRow};
pub use pets::{NewPetSkillSave, PetRepository, PetRow, PetSkillSaveRow, PetValues};
pub use punishments::{NewPunishment, PunishmentRepository, PunishmentRow};
pub use recipes::{RecipeBookRow, RecipeRepository, RecipeShopRow};
pub use recommendations::{RecoBonus, RecommendationRepository};
pub use shortcuts::{NewShortcut, ShortcutRepository, ShortcutRow};
pub use skills::{NewSkillSave, SkillLevel, SkillRepository, SkillSaveRow};
pub use subclasses::{SubclassRepository, SubclassRow, SubclassValues};
pub use summons::{NewSummonSkillSave, SummonRepository, SummonSkillSaveRow};
pub use variables::VariableRepository;
