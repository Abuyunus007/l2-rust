//! Game server tables, one module per table group (same pattern as [`crate::login`]).
//!
//! `characters` is the reference for the others: a repository struct holding the [`crate::Pool`],
//! the Java SQL as constants with a `// Java:` comment, runtime queries, row structs that
//! mirror the column types.

mod characters;
mod ids;
mod items;
mod shortcuts;
mod skills;
mod sql_number;
mod subclasses;

pub use characters::{
    CalendarDate, CharacterIdName, CharacterNameAccess, CharacterRepository, CharacterRow, CharacterUpdate,
    NewCharacter,
};
pub use ids::{IdRepository, UsedIds};
pub use items::{ItemLocation, ItemRepository, ItemRow, ItemUpdate, NewItem, PaperdollItemRow};
pub use shortcuts::{NewShortcut, ShortcutRepository, ShortcutRow};
pub use skills::{NewSkillSave, SkillLevel, SkillRepository, SkillSaveRow};
pub use subclasses::{SubclassRepository, SubclassRow, SubclassValues};
