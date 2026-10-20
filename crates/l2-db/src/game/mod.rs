//! Game server tables, one module per table group (same pattern as [`crate::login`]).
//!
//! `characters` is the reference for the others: a repository struct holding the [`crate::Pool`],
//! the Java SQL as constants with a `// Java:` comment, runtime queries, row structs that
//! mirror the column types.

mod characters;
mod clans;
mod ground_items;
mod hennas;
mod ids;
mod item_reuse;
mod items;
mod macros;
mod recommendations;
mod shortcuts;
mod skills;
mod sql_number;
mod subclasses;
mod variables;

pub use characters::{
    CalendarDate, CharacterIdName, CharacterNameAccess, CharacterRepository, CharacterRow, CharacterUpdate,
    NewCharacter,
};
pub use clans::ClanRepository;
pub use ground_items::{GroundItemRecord, GroundItemRepository};
pub use hennas::{HennaRepository, HennaRow};
pub use ids::{IdRepository, UsedIds};
pub use item_reuse::{ItemReuseRepository, ItemReuseRow, NewItemReuse};
pub use items::{
    ItemElementalRow, ItemLocation, ItemRepository, ItemRow, ItemUpdate, NewItem, OwnerAugmentationRow,
    OwnerElementalRow, PaperdollItemRow,
};
pub use macros::{MacroRepository, MacroRow, NewMacro};
pub use recommendations::{RecoBonus, RecommendationRepository};
pub use shortcuts::{NewShortcut, ShortcutRepository, ShortcutRow};
pub use skills::{NewSkillSave, SkillLevel, SkillRepository, SkillSaveRow};
pub use subclasses::{SubclassRepository, SubclassRow, SubclassValues};
pub use variables::VariableRepository;
