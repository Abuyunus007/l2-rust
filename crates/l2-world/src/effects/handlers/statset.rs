//! The parameter reads of the handler constructors: Java `StatSet.getInt/getDouble/getBoolean/
//! getEnum(key, default)` over an effect's raw `<param>` texts.
// Java: util/StatSet.java

use l2_data::skill::EffectTemplate;
use l2_data::stats::jnum::{parse_boolean, parse_double, parse_int};

/// Java `getInt(key, default)`: `Integer.parseInt` of the text; a bad number makes the Java
/// constructor throw (the effect is then dropped).
pub(super) fn get_int(template: &EffectTemplate, key: &str, default: i32) -> Result<i32, String> {
    match template.param(key) {
        None => Ok(default),
        Some(text) => parse_int(text).ok_or_else(|| format!("Integer value required, but found: {text}")),
    }
}

/// Java `getDouble(key, default)`.
pub(super) fn get_double(template: &EffectTemplate, key: &str, default: f64) -> Result<f64, String> {
    match template.param(key) {
        None => Ok(default),
        Some(text) => parse_double(text).ok_or_else(|| format!("Double value required, but found: {text}")),
    }
}

/// Java `getBoolean(key, default)`: only `true` (any case) is true.
pub(super) fn get_boolean(template: &EffectTemplate, key: &str, default: bool) -> bool {
    template.param(key).map_or(default, parse_boolean)
}

/// Java `getEnum(key, Enum, default)` for an enum given as its constant names: an unknown name
/// throws, like `Enum.valueOf`.
pub(super) fn get_enum<T: Copy>(
    template: &EffectTemplate,
    key: &str,
    names: &[(&str, T)],
    default: T,
) -> Result<T, String> {
    match template.param(key) {
        None => Ok(default),
        Some(text) => names
            .iter()
            .find(|(name, _)| *name == text)
            .map(|(_, value)| *value)
            .ok_or_else(|| format!("Enum value required, but found: {text}")),
    }
}
