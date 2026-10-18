//! Integers MySQL sends as text: `DECIMAL` columns and numeric `VARCHAR` columns.
//!
//! sqlx only decodes integer types from integer columns, so `DECIMAL` (`items.mana_left`,
//! `items.time`, the shortcut columns) and `VARCHAR` (`character_shortcuts.level`) need this
//! wrapper. Java reads them with `getInt`/`getLong`, which parse the text the same way.

use std::str::FromStr;

use sqlx::error::BoxDynError;
use sqlx::mysql::{MySqlTypeInfo, MySqlValueRef};
use sqlx::{Decode, MySql, Type, TypeInfo};

/// An integer decoded from the text of a `DECIMAL` or numeric `VARCHAR` column.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SqlNumber<T>(pub(crate) T);

impl<T> Type<MySql> for SqlNumber<T> {
    fn type_info() -> MySqlTypeInfo {
        <&str as Type<MySql>>::type_info()
    }

    fn compatible(ty: &MySqlTypeInfo) -> bool {
        ty.name() == "DECIMAL" || <&str as Type<MySql>>::compatible(ty)
    }
}

impl<'r, T> Decode<'r, MySql> for SqlNumber<T>
where
    T: FromStr,
    T::Err: std::error::Error + Send + Sync + 'static,
{
    fn decode(value: MySqlValueRef<'r>) -> Result<Self, BoxDynError> {
        let text = <&str as Decode<'r, MySql>>::decode(value)?;
        Ok(Self(text.trim().parse::<T>()?))
    }
}
