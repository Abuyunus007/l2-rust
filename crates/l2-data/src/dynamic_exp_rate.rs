//! `config/DynamicExpRates.xml`: an exp and an sp rate per player level, used instead of `RateXp`
//! and `RateSp` as soon as one level has a rate other than 1.
//!
//! `Npc.getSpReward(level)` (`l2_world::rewards::exp`), which `Attackable.calculateExpAndSp` and
//! `Attackable.getVitalityPoints` call with the level of the player.
//!
//! The file lives in the config folder, not in `data/`, so it is not a [`crate::Datapack`] table:
//! the game server loads it next to the other `config/*.xml` files and hands it to the world with
//! the exp settings.

use std::path::Path;

use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use quick_xml::reader::Reader;
use tracing::{info, warn};

/// File name inside the config folder (Java: `parseDatapackFile("config/DynamicExpRates.xml")`).
pub const FILE: &str = "DynamicExpRates.xml";

/// Java `DynamicExpRateData`: the rate tables and the `_enabled` flag.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct DynamicExpRates {
    exp_rates: Vec<f32>,
    sp_rates: Vec<f32>,
    enabled: bool,
    /// How many levels had a rate other than 1 (Java's `count`, for the log line).
    levels: usize,
}

impl DynamicExpRates {
    /// Java `isEnabled()`: at least one `<dynamic>` had an exp or sp rate other than 1.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Java `getDynamicExpRate(level)`. A level outside the table is 1 here; Java throws an
    /// `ArrayIndexOutOfBoundsException`, which a player level never reaches.
    pub fn dynamic_exp_rate(&self, level: i32) -> f32 {
        Self::rate(&self.exp_rates, level)
    }

    /// Java `getDynamicSpRate(level)`. See [`Self::dynamic_exp_rate`] for a level outside the table.
    pub fn dynamic_sp_rate(&self, level: i32) -> f32 {
        Self::rate(&self.sp_rates, level)
    }

    fn rate(table: &[f32], level: i32) -> f32 {
        usize::try_from(level)
            .ok()
            .and_then(|i| table.get(i))
            .copied()
            .unwrap_or(1.0)
    }

    /// Reads `<dir>/DynamicExpRates.xml`. A missing or broken file leaves the rates disabled
    /// (Java logs the problem and `_enabled` stays false).
    ///
    /// `maximum_level` is `PlayerConfig.PLAYER_MAXIMUM_LEVEL` after its `++`
    /// (`MaximumPlayerLevel + 1`); the Java arrays have one more slot than that.
    pub fn load(config_dir: &Path, maximum_level: i32) -> Self {
        let path = config_dir.join(FILE);
        let xml = match std::fs::read(&path) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(e) => {
                warn!("Cannot parse {FILE}: file does not exist or is not valid: {e}");
                return Self::default();
            }
        };
        match Self::from_xml(&xml, maximum_level) {
            Ok(rates) => {
                if rates.enabled {
                    info!(
                        "DynamicExpRateData: Loaded dynamic rates for {} levels.",
                        rates.levels
                    );
                }
                rates
            }
            Err(e) => {
                warn!("Error parsing {FILE}: {e}");
                Self::default()
            }
        }
    }

    /// Java `parseDocument`: every `<dynamic level exp sp>` below `<list>` whose exp or sp is not 1
    /// sets the rates of that level.
    ///
    /// An `Err` is where Java throws (a missing or bad attribute, a level outside the arrays) and
    /// `parseFile` logs it: `_enabled` is then never set, so the rates stay disabled.
    pub fn from_xml(xml: &str, maximum_level: i32) -> Result<Self, String> {
        // `new float[PLAYER_MAXIMUM_LEVEL + 1]` filled with 1.
        let size = usize::try_from(maximum_level.saturating_add(1)).unwrap_or(0);
        let mut exp_rates = vec![1.0_f32; size];
        let mut sp_rates = vec![1.0_f32; size];
        let mut count = 0;
        let mut reader = Reader::from_str(xml);
        let mut path: Vec<String> = Vec::new();
        loop {
            let event = reader.read_event().map_err(|e| e.to_string())?;
            match &event {
                Event::Start(e) | Event::Empty(e) => {
                    let name = e.name().as_ref().to_owned();
                    // Java walks `list` at the top and its `dynamic` children; nothing deeper.
                    if name == "dynamic" && path.len() == 1 && path[0] == "list" {
                        let level = decode_int(&required(e, "level")?)?;
                        let exp = parse_float(&required(e, "exp")?)?;
                        let sp = parse_float(&required(e, "sp")?)?;
                        if exp != 1.0 || sp != 1.0 {
                            let slot = usize::try_from(level)
                                .ok()
                                .filter(|i| *i < size)
                                .ok_or_else(|| format!("Index {level} out of bounds for length {size}"))?;
                            exp_rates[slot] = exp;
                            sp_rates[slot] = sp;
                            count += 1;
                        }
                    }
                    if matches!(event, Event::Start(_)) {
                        path.push(name);
                    }
                }
                Event::End(_) => {
                    path.pop();
                }
                Event::Eof => break,
                _ => {}
            }
        }
        Ok(Self {
            exp_rates,
            sp_rates,
            enabled: count > 0,
            levels: count,
        })
    }
}

/// The attribute `key`; Java's `parseInteger`/`parseFloat` on a missing attribute end in a
/// `NullPointerException`.
fn required(element: &BytesStart<'_>, key: &str) -> Result<String, String> {
    for attribute in element.attributes() {
        let attribute = attribute.map_err(|e| e.to_string())?;
        if attribute.key.as_ref() == key {
            return attribute
                .normalized_value(XmlVersion::Implicit1_0)
                .map(std::borrow::Cow::into_owned)
                .map_err(|e| e.to_string());
        }
    }
    Err(format!("missing attribute \"{key}\""))
}

/// Java `Float.valueOf(text)` for the plain decimal forms (whitespace around is trimmed).
fn parse_float(text: &str) -> Result<f32, String> {
    text.trim()
        .parse::<f32>()
        .map_err(|_| format!("For input string: \"{text}\""))
}

/// Java `Integer.decode(text)`: optional sign, then decimal, `0x`/`0X`/`#` hex or `0` octal.
fn decode_int(text: &str) -> Result<i32, String> {
    let bad = || format!("For input string: \"{text}\"");
    let (negative, rest) = match text.strip_prefix('-') {
        Some(rest) => (true, rest),
        None => (false, text.strip_prefix('+').unwrap_or(text)),
    };
    let (radix, digits) = if let Some(hex) = rest.strip_prefix("0x").or_else(|| rest.strip_prefix("0X")) {
        (16, hex)
    } else if let Some(hex) = rest.strip_prefix('#') {
        (16, hex)
    } else if rest.len() > 1 && rest.starts_with('0') {
        (8, &rest[1..])
    } else {
        (10, rest)
    };
    if digits.is_empty() || digits.starts_with(['-', '+']) {
        return Err(bad());
    }
    let magnitude = i64::from_str_radix(digits, radix).map_err(|_| bad())?;
    i32::try_from(if negative { -magnitude } else { magnitude }).map_err(|_| bad())
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX: i32 = 86; // MaximumPlayerLevel = 85, after Java's `++`

    fn shipped() -> String {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../dist/game/config")
            .join(FILE);
        std::fs::read_to_string(path).unwrap()
    }

    #[test]
    fn the_shipped_file_is_disabled() {
        // Every shipped rate is 1, so Java reports the dynamic rates as off and `RateXp` applies.
        let rates = DynamicExpRates::from_xml(&shipped(), MAX).unwrap();
        assert!(!rates.is_enabled());
        assert_eq!(rates.dynamic_exp_rate(40), 1.0);
        assert_eq!(rates.dynamic_sp_rate(87), 1.0);
    }

    #[test]
    fn one_level_of_the_shipped_table_enabled() {
        // The shipped table with level 40 changed to exp 2.5 / sp 1.5.
        let xml = shipped().replace(
            r#"<dynamic level="40" exp="1" sp="1" />"#,
            r#"<dynamic level="40" exp="2.5" sp="1.5" />"#,
        );
        assert_ne!(xml, shipped());
        let rates = DynamicExpRates::from_xml(&xml, MAX).unwrap();
        assert!(rates.is_enabled());
        assert_eq!(rates.dynamic_exp_rate(40), 2.5);
        assert_eq!(rates.dynamic_sp_rate(40), 1.5);
        // Its neighbours keep the fill value 1.
        assert_eq!(rates.dynamic_exp_rate(39), 1.0);
        assert_eq!(rates.dynamic_sp_rate(41), 1.0);
    }

    #[test]
    fn exp_or_sp_alone_enables_the_level() {
        let xml = r#"<list><dynamic level="5" exp="1" sp="3"/><dynamic level="6" exp="0.5" sp="1"/></list>"#;
        let rates = DynamicExpRates::from_xml(xml, MAX).unwrap();
        assert!(rates.is_enabled());
        assert_eq!((rates.dynamic_exp_rate(5), rates.dynamic_sp_rate(5)), (1.0, 3.0));
        assert_eq!((rates.dynamic_exp_rate(6), rates.dynamic_sp_rate(6)), (0.5, 1.0));
    }

    #[test]
    fn a_level_with_both_rates_one_is_not_stored_even_beyond_the_table() {
        // Level 87 of the shipped file is one past the Java array; only a rate other than 1 would
        // index it.
        let xml = r#"<list><dynamic level="87" exp="1" sp="1"/></list>"#;
        assert!(DynamicExpRates::from_xml(xml, MAX).is_ok());
        let xml = r#"<list><dynamic level="87" exp="2" sp="1"/></list>"#;
        assert!(DynamicExpRates::from_xml(xml, MAX).is_err());
    }

    #[test]
    fn nothing_but_dynamic_elements_below_list_count() {
        let xml = r#"<!-- c --><list><x><dynamic level="5" exp="2" sp="2"/></x></list>"#;
        assert!(!DynamicExpRates::from_xml(xml, MAX).unwrap().is_enabled());
    }

    #[test]
    fn a_missing_attribute_or_bad_number_fails_the_file() {
        assert!(DynamicExpRates::from_xml(r#"<list><dynamic level="5" exp="2"/></list>"#, MAX).is_err());
        assert!(
            DynamicExpRates::from_xml(r#"<list><dynamic level="x" exp="2" sp="2"/></list>"#, MAX).is_err()
        );
        assert!(
            DynamicExpRates::from_xml(r#"<list><dynamic level="5" exp="a" sp="2"/></list>"#, MAX).is_err()
        );
    }

    #[test]
    fn integer_decode_forms() {
        assert_eq!(decode_int("12"), Ok(12));
        assert_eq!(decode_int("0x10"), Ok(16));
        assert_eq!(decode_int("010"), Ok(8));
        assert_eq!(decode_int("-7"), Ok(-7));
        assert!(decode_int("").is_err());
        assert!(decode_int(" 1").is_err());
    }

    #[test]
    fn load_reads_the_file_and_a_missing_file_is_disabled() {
        let dir = tempfile::tempdir().unwrap();
        assert!(!DynamicExpRates::load(dir.path(), MAX).is_enabled());
        std::fs::write(
            dir.path().join(FILE),
            r#"<list><dynamic level="10" exp="3" sp="4"/></list>"#,
        )
        .unwrap();
        let rates = DynamicExpRates::load(dir.path(), MAX);
        assert!(rates.is_enabled());
        assert_eq!(rates.dynamic_exp_rate(10), 3.0);
        assert_eq!(rates.dynamic_sp_rate(10), 4.0);
    }
}
