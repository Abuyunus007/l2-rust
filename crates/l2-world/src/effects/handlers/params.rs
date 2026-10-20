//! The parameter readers of the effect constructors: Java `StatSet.getInt(key, default)` and its
//! siblings. A value that does not parse makes the constructor fail, as Java's `getInt` throws.
// Java: gameserver/util/StatSet.java (getInt, getLong, getFloat, getDouble, getBoolean, getString)

use l2_data::skill::{EffectTemplate, ParamValue};

/// The text of a parameter: `None` when it is missing, an error when it is a list.
fn text<'a>(template: &'a EffectTemplate, key: &str) -> Result<Option<&'a str>, String> {
    match template.params.get(key) {
        None => Ok(None),
        Some(ParamValue::Text(text)) => Ok(Some(text)),
        Some(ParamValue::Items(_)) => Err(format!("parameter {key} is a list, not a value")),
    }
}

/// Java `StatSet.getString(key, null)`.
pub(crate) fn string(template: &EffectTemplate, key: &str) -> Result<Option<String>, String> {
    Ok(text(template, key)?.map(str::to_string))
}

/// Java `StatSet.getInt(key, defaultValue)`.
pub(crate) fn int(template: &EffectTemplate, key: &str, default: i32) -> Result<i32, String> {
    match text(template, key)? {
        None => Ok(default),
        Some(value) => value
            .parse()
            .map_err(|_| format!("Integer value required, but found: {value}")),
    }
}

/// Java `StatSet.getLong(key, defaultValue)`.
pub(crate) fn long(template: &EffectTemplate, key: &str, default: i64) -> Result<i64, String> {
    match text(template, key)? {
        None => Ok(default),
        Some(value) => value
            .parse()
            .map_err(|_| format!("Long value required, but found: {value}")),
    }
}

/// Java `StatSet.getFloat(key, defaultValue)`.
pub(crate) fn float(template: &EffectTemplate, key: &str, default: f32) -> Result<f32, String> {
    match text(template, key)? {
        None => Ok(default),
        Some(value) => value
            .parse()
            .map_err(|_| format!("Float value required, but found: {value}")),
    }
}

/// Java `StatSet.getDouble(key, defaultValue)`.
pub(crate) fn double(template: &EffectTemplate, key: &str, default: f64) -> Result<f64, String> {
    match text(template, key)? {
        None => Ok(default),
        Some(value) => value
            .parse()
            .map_err(|_| format!("Double value required, but found: {value}")),
    }
}

/// Java `StatSet.getBoolean(key, defaultValue)`: `Boolean.parseBoolean`, so anything but "true"
/// (in any case) is false.
pub(crate) fn boolean(template: &EffectTemplate, key: &str, default: bool) -> Result<bool, String> {
    Ok(match text(template, key)? {
        None => default,
        Some(value) => value.eq_ignore_ascii_case("true"),
    })
}
