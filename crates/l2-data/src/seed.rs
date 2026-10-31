//! The manor seeds: `data/Seeds.xml` (M7-MANOR-01).
//!
//! Java: `managers/CastleManorManager.java` (`load`, `parseDocument`, the seed queries) and
//! `mechanics/siege/manor/Seed.java`. The seeds are immutable; the castle manor state (the
//! production and the procurement of a period) is the world's (`l2_world::manor`).
//!
//! # The order of Java's `HashMap`
//! `CastleManorManager._seeds` is a `HashMap<Integer, Seed>` keyed by the seed item id, and the
//! packets (`ExShowManorDefaultInfo`, `ExShowSeedSetting`, ...) list the seeds in its iteration
//! order. [`java_hash_order`] gives that order for integer keys, so the lists the client gets have
//! Java's order.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::item::ItemTemplates;
use crate::loader::{load_file, log_loaded};

/// One `<crop>` of `Seeds.xml`: a seed and the crop it grows (Java `Seed`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Seed {
    /// `id`: the crop item id.
    pub crop_id: i32,
    /// `seedId`: the seed item id.
    pub seed_id: i32,
    /// `level`: the seed level.
    pub level: i32,
    /// `mature_Id`: the mature crop item id (what the castle gets).
    pub mature_id: i32,
    pub reward1: i32,
    pub reward2: i32,
    /// The castle (manor) the seed can be farmed in (`<castle id>`).
    pub castle_id: i32,
    /// `alternative`.
    pub alternative: bool,
    /// `limit_seed`.
    pub limit_seeds: i32,
    /// `limit_crops`.
    pub limit_crops: i32,
    /// The seed item's price (1 when the item does not exist).
    pub seed_reference_price: i32,
    /// The crop item's price (1 when the item does not exist).
    pub crop_reference_price: i32,
}

impl Seed {
    /// Java `getReward(type)`: reward 1 for type 1, else reward 2.
    pub fn reward(&self, reward_type: i32) -> i32 {
        if reward_type == 1 {
            self.reward1
        } else {
            self.reward2
        }
    }

    /// Java `getSeedLimit()`: `_limitSeeds * RATE_DROP_MANOR` (an `int` product).
    pub fn seed_limit(&self, rate_drop_manor: i32) -> i32 {
        self.limit_seeds.wrapping_mul(rate_drop_manor)
    }

    /// Java `getCropLimit()`.
    pub fn crop_limit(&self, rate_drop_manor: i32) -> i32 {
        self.limit_crops.wrapping_mul(rate_drop_manor)
    }

    /// Java `getSeedMaxPrice()`.
    pub fn seed_max_price(&self) -> i32 {
        self.seed_reference_price.wrapping_mul(10)
    }

    /// Java `getSeedMinPrice()`: `(int) (price * 0.6)`.
    pub fn seed_min_price(&self) -> i32 {
        (f64::from(self.seed_reference_price) * 0.6) as i32
    }

    /// Java `getCropMaxPrice()`.
    pub fn crop_max_price(&self) -> i32 {
        self.crop_reference_price.wrapping_mul(10)
    }

    /// Java `getCropMinPrice()`.
    pub fn crop_min_price(&self) -> i32 {
        (f64::from(self.crop_reference_price) * 0.6) as i32
    }
}

/// `data/Seeds.xml`: Java `CastleManorManager._seeds`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Seeds {
    /// By seed id.
    seeds: BTreeMap<i32, Seed>,
    /// The seed ids in the order Java's `HashMap` walks them.
    java_order: Vec<i32>,
}

impl Seeds {
    const LOADER: &'static str = "CastleManorManager";

    /// Loads `<root>/data/Seeds.xml` and logs Java's count line. The reference prices come from
    /// the item templates.
    pub fn load(root: &Path, items: &ItemTemplates) -> Self {
        let mut table = Self::default();
        let mut inserted = Vec::new();
        load_file(&root.join("data/Seeds.xml"), |doc: RawList| {
            table.add(doc, items, &mut inserted);
            Ok(())
        });
        table.java_order = java_hash_order(&inserted);
        log_loaded(Self::LOADER, table.len(), "seeds");
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str, items: &ItemTemplates) -> Result<Self, crate::DataError> {
        let doc: RawList = quick_xml::de::from_str(xml).map_err(|source| crate::DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        let mut inserted = Vec::new();
        table.add(doc, items, &mut inserted);
        table.java_order = java_hash_order(&inserted);
        Ok(table)
    }

    fn add(&mut self, doc: RawList, items: &ItemTemplates, inserted: &mut Vec<i32>) {
        for castle in doc.castles {
            let Some(castle_id) = parse_int(castle.id.as_deref()) else {
                // NOTE(java-quirk): Java's `parseInteger` throws and the rest of the file is lost.
                warn!("{}: castle without a numeric id.", Self::LOADER);
                continue;
            };
            for crop in castle.crops {
                match crop.parse(castle_id, items) {
                    Ok(seed) => {
                        // `_seeds.put(seedId, seed)`: a repeated seed id replaces the value and
                        // keeps its place.
                        if self.seeds.insert(seed.seed_id, seed.clone()).is_none() {
                            inserted.push(seed.seed_id);
                        }
                    }
                    Err(e) => warn!("{}: {e}", Self::LOADER),
                }
            }
        }
    }

    pub fn len(&self) -> usize {
        self.seeds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.seeds.is_empty()
    }

    /// Java `getSeed(seedId)`.
    pub fn get(&self, seed_id: i32) -> Option<&Seed> {
        self.seeds.get(&seed_id)
    }

    /// The seeds in the order of Java's `_seeds.values()`.
    pub fn iter(&self) -> impl Iterator<Item = &Seed> {
        self.java_order.iter().filter_map(|id| self.seeds.get(id))
    }

    /// Java `getSeedIds()`.
    pub fn seed_ids(&self) -> impl Iterator<Item = i32> + '_ {
        self.java_order.iter().copied()
    }

    /// Java `getCropIds()`: the crop ids of a `HashSet<Integer>`, in its iteration order.
    pub fn crop_ids(&self) -> Vec<i32> {
        let mut inserted = Vec::new();
        for seed in self.iter() {
            if !inserted.contains(&seed.crop_id) {
                inserted.push(seed.crop_id);
            }
        }
        java_hash_order(&inserted)
    }

    /// Java `getCrops()`: the first seed of each crop id, in the order of `_seeds.values()`.
    pub fn crops(&self) -> Vec<&Seed> {
        let mut seen = Vec::new();
        let mut crops = Vec::new();
        for seed in self.iter() {
            if !seen.contains(&seed.crop_id) {
                seen.push(seed.crop_id);
                crops.push(seed);
            }
        }
        crops
    }

    /// Java `getSeedsForCastle(castleId)`.
    // NOTE(java-quirk): Java returns a `HashSet<Seed>` of objects without `hashCode`, so the order
    // of the packets that list them is arbitrary there; here it is the order of `_seeds`.
    pub fn for_castle(&self, castle_id: i32) -> Vec<&Seed> {
        self.iter().filter(|s| s.castle_id == castle_id).collect()
    }

    /// Java `getSeedByCrop(cropId, castleId)`.
    pub fn by_crop_in_castle(&self, crop_id: i32, castle_id: i32) -> Option<&Seed> {
        self.iter()
            .find(|s| s.castle_id == castle_id && s.crop_id == crop_id)
    }

    /// Java `getSeedByCrop(cropId)`: the first seed of the crop.
    pub fn by_crop(&self, crop_id: i32) -> Option<&Seed> {
        self.iter().find(|s| s.crop_id == crop_id)
    }
}

/// The iteration order of a Java `HashMap<Integer, ?>` (or `HashSet<Integer>`) built by putting
/// `keys` in this order: by bucket (`(h ^ h >>> 16) & (capacity - 1)`), keys of one bucket in
/// insertion order. The capacity starts at 16 and doubles whenever the size passes 3/4 of it.
/// A key put twice counts once, at its first place.
pub fn java_hash_order(keys: &[i32]) -> Vec<i32> {
    let mut unique: Vec<i32> = Vec::with_capacity(keys.len());
    for key in keys {
        if !unique.contains(key) {
            unique.push(*key);
        }
    }
    let mut capacity: usize = 16;
    while unique.len() > capacity * 3 / 4 {
        capacity *= 2;
    }
    let bucket = |key: i32| {
        // `Integer.hashCode()` is the value; `HashMap.hash` spreads the high bits down.
        let h = key as u32;
        ((h ^ (h >> 16)) as usize) & (capacity - 1)
    };
    // A stable sort keeps the insertion order inside a bucket.
    unique.sort_by_key(|key| bucket(*key));
    unique
}

fn parse_int(text: Option<&str>) -> Option<i32> {
    text?.trim().parse().ok()
}

#[derive(Debug, Deserialize)]
struct RawList {
    #[serde(rename = "castle", default)]
    castles: Vec<RawCastle>,
}

#[derive(Debug, Deserialize)]
struct RawCastle {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "crop", default)]
    crops: Vec<RawCrop>,
}

#[derive(Debug, Deserialize)]
struct RawCrop {
    #[serde(rename = "@id")]
    id: Option<String>,
    #[serde(rename = "@seedId")]
    seed_id: Option<String>,
    #[serde(rename = "@mature_Id")]
    mature_id: Option<String>,
    #[serde(rename = "@reward1")]
    reward1: Option<String>,
    #[serde(rename = "@reward2")]
    reward2: Option<String>,
    #[serde(rename = "@alternative")]
    alternative: Option<String>,
    #[serde(rename = "@level")]
    level: Option<String>,
    #[serde(rename = "@limit_seed")]
    limit_seed: Option<String>,
    #[serde(rename = "@limit_crops")]
    limit_crops: Option<String>,
}

impl RawCrop {
    /// Java `new Seed(set)`: every number is required (`StatSet.getInt` throws without it).
    fn parse(self, castle_id: i32, items: &ItemTemplates) -> Result<Seed, String> {
        let int = |value: &Option<String>, name: &str| {
            parse_int(value.as_deref()).ok_or_else(|| format!("Integer value required for {name}"))
        };
        let crop_id = int(&self.id, "id")?;
        let seed_id = int(&self.seed_id, "seedId")?;
        Ok(Seed {
            crop_id,
            seed_id,
            level: int(&self.level, "level")?,
            mature_id: int(&self.mature_id, "mature_Id")?,
            reward1: int(&self.reward1, "reward1")?,
            reward2: int(&self.reward2, "reward2")?,
            castle_id,
            // `StatSet.getBoolean` accepts "true" and "false" (and numbers); anything else throws.
            alternative: match self.alternative.as_deref().map(str::trim) {
                Some(v) if v.eq_ignore_ascii_case("true") => true,
                Some(v) if v.eq_ignore_ascii_case("false") => false,
                other => return Err(format!("Boolean value required for alternative, found {other:?}")),
            },
            limit_crops: int(&self.limit_crops, "limit_crops")?,
            limit_seeds: int(&self.limit_seed, "limit_seed")?,
            crop_reference_price: items.get(crop_id).map_or(1, |t| t.price),
            seed_reference_price: items.get(seed_id).map_or(1, |t| t.price),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &str = r#"<list>
<item id="5016" type="EtcItem" name="Seed: Dark Coda"><set name="price" val="5" /></item>
<item id="5073" type="EtcItem" name="Dark Coda"><set name="price" val="9" /></item>
</list>"#;

    fn items() -> ItemTemplates {
        ItemTemplates::from_xml(ITEMS).unwrap()
    }

    const SEEDS: &str = r#"<list>
<castle id="1">
  <crop id="5073" seedId="5016" mature_Id="5103" reward1="1864" reward2="1878" alternative="false" level="10" limit_seed="8100" limit_crops="9000" />
  <crop id="5818" seedId="5650" mature_Id="5838" reward1="1864" reward2="1878" alternative="true" level="10" limit_seed="22500" limit_crops="2250" />
</castle>
<castle id="2">
  <crop id="5068" seedId="5017" mature_Id="5098" reward1="1865" reward2="1879" alternative="false" level="13" limit_seed="8100" limit_crops="9000" />
</castle>
</list>"#;

    #[test]
    fn reads_seeds_with_reference_prices() {
        let seeds = Seeds::from_xml(SEEDS, &items()).unwrap();
        assert_eq!(seeds.len(), 3);
        let coda = seeds.get(5016).unwrap();
        assert_eq!(
            (coda.crop_id, coda.castle_id, coda.level, coda.mature_id),
            (5073, 1, 10, 5103)
        );
        assert!(!coda.alternative && seeds.get(5650).unwrap().alternative);
        // The price of the item; 1 for an item the datapack does not have.
        assert_eq!((coda.seed_reference_price, coda.crop_reference_price), (5, 9));
        assert_eq!(seeds.get(5650).unwrap().seed_reference_price, 1);
    }

    #[test]
    fn prices_and_limits_follow_seed_java() {
        let coda = Seeds::from_xml(SEEDS, &items())
            .unwrap()
            .get(5016)
            .unwrap()
            .clone();
        assert_eq!((coda.seed_max_price(), coda.seed_min_price()), (50, 3));
        assert_eq!((coda.crop_max_price(), coda.crop_min_price()), (90, 5));
        assert_eq!((coda.seed_limit(2), coda.crop_limit(2)), (16_200, 18_000));
        assert_eq!(
            (coda.reward(1), coda.reward(2), coda.reward(0)),
            (1864, 1878, 1878)
        );
    }

    #[test]
    fn lookups_by_crop_and_castle() {
        let seeds = Seeds::from_xml(SEEDS, &items()).unwrap();
        assert_eq!(seeds.by_crop(5068).unwrap().seed_id, 5017);
        assert_eq!(seeds.by_crop_in_castle(5068, 2).unwrap().seed_id, 5017);
        assert!(seeds.by_crop_in_castle(5068, 1).is_none());
        let ids: Vec<i32> = seeds.for_castle(1).iter().map(|s| s.seed_id).collect();
        assert_eq!(ids.len(), 2);
        assert_eq!(seeds.crops().len(), 3);
    }

    #[test]
    fn a_hash_map_walks_by_bucket() {
        // 16 buckets: 5016 & 15 = 8, 5017 & 15 = 9, 5650 & 15 = 2; ties keep the put order.
        assert_eq!(java_hash_order(&[5016, 5017, 5650]), vec![5650, 5016, 5017]);
        assert_eq!(java_hash_order(&[17, 1, 33]), vec![17, 1, 33]);
        // The table grows past 12 entries: 32 buckets, so 1 and 17 part (the repeated 17 counts once).
        let many: Vec<i32> = (0..13).map(|i| i * 16 + 1).chain([17]).collect();
        assert_eq!(
            java_hash_order(&many),
            vec![1, 33, 65, 97, 129, 161, 193, 17, 49, 81, 113, 145, 177]
        );
        // A repeated key counts once.
        assert_eq!(java_hash_order(&[3, 3, 1]), vec![1, 3]);
    }

    #[test]
    fn the_shipped_file_loads() {
        let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game"));
        if !root.join("data/Seeds.xml").exists() {
            eprintln!("skipped: the Java reference folder is not there");
            return;
        }
        let items = ItemTemplates::load(root, false);
        let seeds = Seeds::load(root, &items);
        assert!(seeds.len() > 100);
        // Every castle has seeds; a crop id is shared by the castles that grow it.
        assert!((1..=9).all(|castle| !seeds.for_castle(castle).is_empty()));
        assert!(seeds.crops().len() < seeds.len());
        assert_eq!(seeds.crops().len(), seeds.crop_ids().len());
    }
}
