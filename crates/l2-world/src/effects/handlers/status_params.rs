//! Reading the parameters of a status effect the way Java's `StatSet` getters do (the constructor
//! of a handler throws on a bad number, which `createEffect` logs and turns into "no effect").
// Java: util/StatSet.java (getInt, getDouble, getString)

use l2_data::skill::EffectTemplate;
use l2_data::stats::jnum::{parse_double, parse_int};

/// `params.getInt(key, default)`.
pub(super) fn int_or(template: &EffectTemplate, key: &str, default: i32) -> Result<i32, String> {
    match template.param(key) {
        None => Ok(default),
        Some(text) => parse_int(text).ok_or_else(|| format!("Integer value required, but found: {text}")),
    }
}

/// `params.getDouble(key, default)`.
pub(super) fn double_or(template: &EffectTemplate, key: &str, default: f64) -> Result<f64, String> {
    match template.param(key) {
        None => Ok(default),
        Some(text) => {
            parse_double(text).ok_or_else(|| format!("Double value required, but found: {text}"))
        }
    }
}

/// `params.getString(key)`: required.
pub(super) fn required_str<'t>(template: &'t EffectTemplate, key: &str) -> Result<&'t str, String> {
    template
        .param(key)
        .ok_or_else(|| format!("String value required, but not specified: {key}!"))
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use l2_data::skill::{EffectScope, ParamValue};

    use super::*;

    fn template(params: &[(&str, &str)]) -> EffectTemplate {
        EffectTemplate {
            name: "Test".into(),
            scope: EffectScope::General,
            ticks: 0,
            attrs: BTreeMap::new(),
            params: params
                .iter()
                .map(|(k, v)| ((*k).to_owned(), ParamValue::Text((*v).to_owned())))
                .collect(),
            funcs: Vec::new(),
            attach_cond: None,
            apply_cond: None,
        }
    }

    #[test]
    fn missing_keys_take_the_default() {
        let t = template(&[]);
        assert_eq!(int_or(&t, "chance", 100), Ok(100));
        assert_eq!(double_or(&t, "power", 7.5), Ok(7.5));
        assert!(required_str(&t, "slot").is_err());
    }

    #[test]
    fn numbers_parse_like_java() {
        let t = template(&[("chance", "35"), ("power", "1.5"), ("bad", "x")]);
        assert_eq!(int_or(&t, "chance", 100), Ok(35));
        assert_eq!(double_or(&t, "power", 0.0), Ok(1.5));
        assert!(int_or(&t, "bad", 0).is_err());
        assert!(double_or(&t, "bad", 0.0).is_err());
    }
}
