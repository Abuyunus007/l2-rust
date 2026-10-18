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

pub use error::{DbError, Result};
pub use game::{
    CalendarDate, CharacterIdName, CharacterNameAccess, CharacterRepository, CharacterRow, CharacterUpdate,
    IdRepository, NewCharacter, UsedIds,
};
pub use game::{ItemLocation, ItemRepository, ItemRow, ItemUpdate, NewItem, PaperdollItemRow};
pub use game::{NewShortcut, ShortcutRepository, ShortcutRow};
pub use game::{NewSkillSave, SkillLevel, SkillRepository, SkillSaveRow};
pub use game::{SubclassRepository, SubclassRow, SubclassValues};
pub use login::{
    AccountInfoRow, ChangePasswordResult, GameServerRow, IpAuthKind, IpAuthRow, LoginRepository, TracertHops,
    hash_password, hex_to_string, string_to_hex,
};
pub use pool::{Pool, connect, connect_options, connect_with, pool_max_size, pool_min_idle};
