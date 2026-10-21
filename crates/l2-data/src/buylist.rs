//! NPC buy lists (`data/buylists/*.xml`) and the merchant price configs
//! (`data/MerchantPriceConfig.xml`): what a merchant sells, for how much, and which base tax its
//! town adds.
//!
//! Java: `gameserver/data/xml/BuyListData.java`, `gameserver/mechanics/buylist/{BuyListHolder,
//! Product}.java`, `gameserver/data/MerchantPriceConfigTable.java`. Data: `data/buylists/*.xml`
//! (`xsd/buylist.xsd`), `data/MerchantPriceConfig.xml`.
//!
//! These tables are the *definitions*: immutable, like everything in this crate. The mutable part
//! of a limited product (how many are left, when it restocks) is world state, see
//! `l2_world::shop`.
//!
//! The list id is the file name (`0000382.xml` is list 382); only files named `<digits>.xml` are
//! read (`BuyListData.isValidXmlFile`). The `custom` folder is read after the main one when
//! `CustomBuyListLoad` is on, so a custom list with the same id replaces the shipped one.
//!
//! NOTE(java-quirk): Java checks the `CorrectPrices` rule while it reads the document, so it only
//! sees the `<npcs>` that stand *before* the `<item>` in the file. All shipped lists put `<npcs>`
//! first; a list without `<npcs>` (or with them after the items) never gets its prices corrected.

use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::Deserialize;
use tracing::{error, warn};

use crate::item::ItemTemplates;
use crate::loader::{DataError, log_loaded, parse_file};

/// One product of a buy list. Java `Product` without the stock, which the world keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductDef {
    pub item_id: i32,
    /// Java `_price`: -1 when the file has no price (then the item's reference price is used).
    pub price: i64,
    /// Java `_restockDelay`: `restock_delay * 60000`, so milliseconds. A product without
    /// `restock_delay` has -60000 (Java multiplies the default -1).
    pub restock_delay_ms: i64,
    /// Java `_maxCount`: -1 when the file has no `count` (unlimited stock).
    pub max_count: i64,
}

impl ProductDef {
    /// Java `hasLimitedStock()`.
    pub fn has_limited_stock(&self) -> bool {
        self.max_count > -1
    }

    /// Java `getPrice()`: the file's price, or the item's reference price when there is none.
    pub fn price_or(&self, reference_price: i32) -> i64 {
        if self.price < 0 {
            i64::from(reference_price)
        } else {
            self.price
        }
    }
}

/// One buy list. Java `BuyListHolder`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuyListDef {
    pub id: i32,
    /// Java `_products`, a `LinkedHashMap`: in file order; a second `<item>` with the same id
    /// replaces the first one in place.
    products: Vec<ProductDef>,
    /// Java `_allowedNpcs`: `None` until the first `<npc>` (Java keeps a `null` set).
    allowed_npcs: Option<HashSet<i32>>,
}

impl BuyListDef {
    pub fn new(id: i32) -> Self {
        Self {
            id,
            products: Vec::new(),
            allowed_npcs: None,
        }
    }

    /// Java `addProduct`.
    pub fn add_product(&mut self, product: ProductDef) {
        match self.products.iter_mut().find(|p| p.item_id == product.item_id) {
            Some(existing) => *existing = product,
            None => self.products.push(product),
        }
    }

    /// Java `addAllowedNpc`.
    pub fn add_allowed_npc(&mut self, npc_id: i32) {
        self.allowed_npcs.get_or_insert_with(HashSet::new).insert(npc_id);
    }

    /// Java `getProducts()`.
    pub fn products(&self) -> &[ProductDef] {
        &self.products
    }

    /// Java `getProductByItemId(itemId)`.
    pub fn product(&self, item_id: i32) -> Option<&ProductDef> {
        self.products.iter().find(|p| p.item_id == item_id)
    }

    /// Position of the product in [`Self::products`] (the world keeps the stock in the same
    /// order).
    pub fn product_index(&self, item_id: i32) -> Option<usize> {
        self.products.iter().position(|p| p.item_id == item_id)
    }

    /// Java `isNpcAllowed(npcId)`: false for a list without `<npcs>`.
    pub fn is_npc_allowed(&self, npc_id: i32) -> bool {
        self.allowed_npcs.as_ref().is_some_and(|s| s.contains(&npc_id))
    }

    /// Java `getNpcsAllowed()`.
    pub fn npcs_allowed(&self) -> Option<&HashSet<i32>> {
        self.allowed_npcs.as_ref()
    }
}

/// What `BuyListData` reads from the config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BuyListOptions {
    /// `General.ini` `CustomBuyListLoad` (default false): also read `data/buylists/custom`.
    pub custom_buylist_load: bool,
    /// `General.ini` `CorrectPrices` (default true): a price below the sell price is raised.
    pub correct_prices: bool,
}

impl Default for BuyListOptions {
    /// Java's code defaults.
    fn default() -> Self {
        Self {
            custom_buylist_load: false,
            correct_prices: true,
        }
    }
}

/// All buy lists by id. Java `BuyListData`.
#[derive(Debug, Default, Clone)]
pub struct BuyLists {
    lists: BTreeMap<i32, BuyListDef>,
}

impl BuyLists {
    const LOADER: &'static str = "BuyListData";

    /// Loads `<root>/data/buylists` (and `custom`, see [`BuyListOptions`]) and logs Java's count
    /// line. `items` is `ItemData`: a product whose item does not exist is skipped with a warning.
    pub fn load(root: &Path, items: &ItemTemplates, options: &BuyListOptions) -> Self {
        let mut table = Self::default();
        table.load_dir(&root.join("data/buylists"), items, options);
        if options.custom_buylist_load {
            table.load_dir(&root.join("data/buylists/custom"), items, options);
        }
        log_loaded(Self::LOADER, table.lists.len(), "buyLists");
        table
    }

    /// Builds the table from `(file name, text)` pairs. For tests and tools.
    pub fn from_xml(files: &[(&str, &str)], items: &ItemTemplates, options: &BuyListOptions) -> Self {
        let mut table = Self::default();
        for (name, text) in files {
            let parsed = quick_xml::de::from_str::<RawList>(text).map_err(|source| DataError::Xml {
                path: name.into(),
                source,
            });
            table.add_file(name, parsed, items, options);
        }
        table
    }

    fn load_dir(&mut self, dir: &Path, items: &ItemTemplates, options: &BuyListOptions) {
        if !dir.is_dir() {
            warn!("Directory not found: {}", dir.display());
            return;
        }
        let mut files: Vec<_> = match std::fs::read_dir(dir) {
            Ok(entries) => entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.is_file())
                .collect(),
            Err(e) => {
                warn!("Cannot list {}: {e}", dir.display());
                return;
            }
        };
        files.sort();
        for path in files {
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_default();
            // Java `isValidXmlFile`: `\d+\.xml` on the lower case name.
            if !is_buylist_file_name(&name) {
                continue;
            }
            let parsed = parse_file::<RawList>(&path);
            self.add_file(&name, parsed, items, options);
        }
    }

    /// Java `parseDocument`.
    fn add_file(
        &mut self,
        name: &str,
        parsed: Result<RawList, DataError>,
        items: &ItemTemplates,
        options: &BuyListOptions,
    ) {
        let result = parsed.and_then(|doc| build_list(name, doc, items, options));
        match result {
            Ok(list) => {
                self.lists.insert(list.id, list);
            }
            // Java: the exception aborts the document, so the list is not stored at all.
            Err(e) => warn!("Failed to load buyList data from xml File:{name}: {e}"),
        }
    }

    /// Java `getBuyList(listId)`.
    pub fn get(&self, list_id: i32) -> Option<&BuyListDef> {
        self.lists.get(&list_id)
    }

    /// Every list, ordered by id.
    pub fn iter(&self) -> impl Iterator<Item = &BuyListDef> {
        self.lists.values()
    }

    pub fn len(&self) -> usize {
        self.lists.len()
    }

    pub fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }
}

/// `XML_FILE_PATTERN` (`\d+\.xml`) on the lower case file name.
fn is_buylist_file_name(name: &str) -> bool {
    let lower = name.to_lowercase();
    lower
        .strip_suffix(".xml")
        .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_digit()))
}

fn parse_int<T: std::str::FromStr>(key: &str, text: &str) -> Result<T, DataError> {
    text.parse::<T>()
        .map_err(|_| DataError::Invalid(format!("number required for {key}, found: {text}")))
}

fn build_list(
    file: &str,
    doc: RawList,
    items: &ItemTemplates,
    options: &BuyListOptions,
) -> Result<BuyListDef, DataError> {
    // `Integer.parseInt(file.getName().replace(".xml", ""))`
    let list_id: i32 = parse_int("the file name", &file.replace(".xml", ""))?;
    let mut list = BuyListDef::new(list_id);
    for child in doc.children {
        match child {
            RawChild::Item(raw) => {
                let id_text = raw
                    .id
                    .as_deref()
                    .ok_or_else(|| DataError::Invalid("item without id".into()))?;
                let item_id: i32 = parse_int("id", id_text)?;
                let price: i64 = match raw.price.as_deref() {
                    Some(text) => parse_int("price", text)?,
                    None => -1,
                };
                let restock_delay: i64 = match raw.restock_delay.as_deref() {
                    Some(text) => parse_int("restock_delay", text)?,
                    None => -1,
                };
                let count: i64 = match raw.count.as_deref() {
                    Some(text) => parse_int("count", text)?,
                    None => -1,
                };
                let Some(item) = items.get(item_id) else {
                    warn!("Item not found. BuyList:{list_id} ItemID:{item_id} File:{file}");
                    continue;
                };
                let sell_price = i64::from(item.price / 2);
                let price = if options.correct_prices
                    && price > -1
                    && sell_price > price
                    && list.npcs_allowed().is_some()
                {
                    warn!(
                        "Buy price {price} is less than sell price {sell_price} for ItemID:{item_id} of buylist {list_id}."
                    );
                    sell_price
                } else {
                    price
                };
                list.add_product(ProductDef {
                    item_id,
                    price,
                    restock_delay_ms: restock_delay.wrapping_mul(60_000),
                    max_count: count,
                });
            }
            RawChild::Npcs(npcs) => {
                for npc in npcs.npcs {
                    // `Integer.parseInt(npcs_node.getTextContent())`: no trimming in Java.
                    let npc_id: i32 = parse_int("npc", &npc.text)?;
                    list.add_allowed_npc(npc_id);
                }
            }
            RawChild::Other => {}
        }
    }
    Ok(list)
}

#[derive(Deserialize)]
struct RawList {
    #[serde(rename = "$value", default)]
    children: Vec<RawChild>,
}

#[derive(Deserialize)]
enum RawChild {
    #[serde(rename = "item")]
    Item(RawItem),
    #[serde(rename = "npcs")]
    Npcs(RawNpcs),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawItem {
    #[serde(rename = "@id", default)]
    id: Option<String>,
    #[serde(rename = "@price", default)]
    price: Option<String>,
    #[serde(rename = "@restock_delay", default)]
    restock_delay: Option<String>,
    #[serde(rename = "@count", default)]
    count: Option<String>,
}

#[derive(Deserialize)]
struct RawNpcs {
    #[serde(rename = "npc", default)]
    npcs: Vec<RawNpc>,
}

#[derive(Deserialize)]
struct RawNpc {
    #[serde(rename = "$text", default)]
    text: String,
}

// ---------------------------------------------------------------------------------------
// MerchantPriceConfig
// ---------------------------------------------------------------------------------------

/// One `<priceConfig>`. Java `MerchantPriceConfig`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MerchantPriceConfig {
    pub id: i32,
    pub name: String,
    /// The percent every merchant of the area adds to its prices.
    pub base_tax: i32,
    /// -1 when no castle controls the area.
    pub castle_id: i32,
    /// The town zone the config belongs to, -1 for none.
    pub zone_id: i32,
}

impl MerchantPriceConfig {
    /// Java `getBaseTaxRate()`.
    pub fn base_tax_rate(&self) -> f64 {
        f64::from(self.base_tax) / 100.0
    }
}

/// The price configs and the default one. Java `MerchantPriceConfigTable`.
#[derive(Debug, Default, Clone)]
pub struct MerchantPriceConfigs {
    /// Java `_mpcs`, a `HashMap<Integer, _>`: for these small ids the iteration order is by id.
    configs: BTreeMap<i32, MerchantPriceConfig>,
    default: Option<i32>,
}

impl MerchantPriceConfigs {
    const FILE: &'static str = "data/MerchantPriceConfig.xml";

    /// Java `loadInstances()`: reads `data/MerchantPriceConfig.xml`. A file that is missing loads
    /// nothing; an invalid file is logged and keeps what was read before the error (Java fills
    /// the map while it parses).
    pub fn load(root: &Path) -> Self {
        let path = root.join(Self::FILE);
        let mut table = Self::default();
        if !path.exists() {
            log_loaded("MerchantPriceConfigTable", 0, "merchant price configs");
            return table;
        }
        match parse_file::<RawPriceFile>(&path) {
            Ok(doc) => match table.fill(doc) {
                Ok(()) => log_loaded(
                    "MerchantPriceConfigTable",
                    table.configs.len(),
                    "merchant price configs",
                ),
                Err(e) => {
                    error!("MerchantPriceConfigTable: Failed loading MerchantPriceConfigTable. Reason: {e}")
                }
            },
            Err(e) => {
                error!("MerchantPriceConfigTable: Failed loading MerchantPriceConfigTable. Reason: {e}")
            }
        }
        table
    }

    /// Builds the table from XML text. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc: RawPriceFile = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.fill(doc)?;
        Ok(table)
    }

    fn fill(&mut self, doc: RawPriceFile) -> Result<(), DataError> {
        let default_text = doc.default_price_config.ok_or_else(|| {
            DataError::Invalid("merchantPriceConfig must define an 'defaultPriceConfig'".into())
        })?;
        let default_id: i32 = parse_int("defaultPriceConfig", &default_text)?;
        for raw in doc.configs {
            let id_text = raw
                .id
                .ok_or_else(|| DataError::Invalid("Must define the priceConfig 'id'".into()))?;
            let name = raw
                .name
                .ok_or_else(|| DataError::Invalid("Must define the priceConfig 'name'".into()))?;
            let base_tax_text = raw
                .base_tax
                .ok_or_else(|| DataError::Invalid("Must define the priceConfig 'baseTax'".into()))?;
            let id: i32 = parse_int("id", &id_text)?;
            let base_tax: i32 = parse_int("baseTax", &base_tax_text)?;
            let castle_id: i32 = match raw.castle_id.as_deref() {
                Some(text) => parse_int("castleId", text)?,
                None => -1,
            };
            let zone_id: i32 = match raw.zone_id.as_deref() {
                Some(text) => parse_int("zoneId", text)?,
                None => -1,
            };
            self.configs.insert(
                id,
                MerchantPriceConfig {
                    id,
                    name,
                    base_tax,
                    castle_id,
                    zone_id,
                },
            );
        }
        if !self.configs.contains_key(&default_id) {
            return Err(DataError::Invalid(
                "'defaultPriceConfig' points to an non-loaded priceConfig".into(),
            ));
        }
        self.default = Some(default_id);
        Ok(())
    }

    /// Java `getMerchantPriceConfig(id)`.
    pub fn get(&self, id: i32) -> Option<&MerchantPriceConfig> {
        self.configs.get(&id)
    }

    /// Java `getMerchantPriceConfig(npc)`: the first config (by id) whose zone is in `zone_ids`
    /// (the zones of the NPC's zone region), else the default one.
    pub fn for_zones(&self, zone_ids: &HashSet<i32>) -> Option<&MerchantPriceConfig> {
        self.configs
            .values()
            .find(|c| zone_ids.contains(&c.zone_id))
            .or_else(|| self.default.and_then(|id| self.configs.get(&id)))
    }

    pub fn len(&self) -> usize {
        self.configs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.configs.is_empty()
    }
}

#[derive(Deserialize)]
struct RawPriceFile {
    #[serde(rename = "@defaultPriceConfig", default)]
    default_price_config: Option<String>,
    #[serde(rename = "priceConfig", default)]
    configs: Vec<RawPriceConfig>,
}

#[derive(Deserialize)]
struct RawPriceConfig {
    #[serde(rename = "@id", default)]
    id: Option<String>,
    #[serde(rename = "@name", default)]
    name: Option<String>,
    #[serde(rename = "@baseTax", default)]
    base_tax: Option<String>,
    #[serde(rename = "@castleId", default)]
    castle_id: Option<String>,
    #[serde(rename = "@zoneId", default)]
    zone_id: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true" /></item>
<item id="1835" type="EtcItem" name="Soulshot"><set name="price" val="20" /></item>
<item id="2509" type="EtcItem" name="Spiritshot"><set name="price" val="100" /></item>
</list>"#;

    fn items() -> ItemTemplates {
        ItemTemplates::from_xml(ITEMS).unwrap()
    }

    #[test]
    fn file_names_are_digits_and_xml() {
        assert!(is_buylist_file_name("0000382.xml"));
        assert!(is_buylist_file_name("7.XML"));
        assert!(!is_buylist_file_name("abc.xml"));
        assert!(!is_buylist_file_name(".xml"));
        assert!(!is_buylist_file_name("12.xml.bak"));
        assert!(!is_buylist_file_name("12"));
    }

    #[test]
    fn a_list_is_read_in_file_order_with_java_defaults() {
        let xml = r#"<list>
            <npcs><npc>31380</npc><npc>31381</npc></npcs>
            <item id="2509" price="90" count="5" restock_delay="3" />
            <item id="1835" />
            <item id="999999" price="1" />
        </list>"#;
        let lists = BuyLists::from_xml(&[("0000382.xml", xml)], &items(), &BuyListOptions::default());
        let list = lists.get(382).unwrap();
        assert!(list.is_npc_allowed(31380) && list.is_npc_allowed(31381) && !list.is_npc_allowed(1));
        // The unknown item is skipped; the rest keeps the file's order.
        let ids: Vec<i32> = list.products().iter().map(|p| p.item_id).collect();
        assert_eq!(ids, [2509, 1835]);
        // price 90 is above the sell price 50: kept. A limited product has its delay in ms.
        let limited = list.product(2509).unwrap();
        assert_eq!(
            (limited.price, limited.max_count, limited.restock_delay_ms),
            (90, 5, 180_000)
        );
        assert!(limited.has_limited_stock());
        // No attributes: reference price, unlimited, Java's -1 * 60000.
        let plain = list.product(1835).unwrap();
        assert_eq!(
            (plain.price, plain.max_count, plain.restock_delay_ms),
            (-1, -1, -60_000)
        );
        assert!(!plain.has_limited_stock());
        assert_eq!(plain.price_or(20), 20);
    }

    #[test]
    fn correct_prices_raises_a_price_below_the_sell_price_only_after_npcs() {
        // Java: sell price = reference price / 2 = 50; the rule needs the `<npcs>` to be read.
        let after = r#"<list><item id="2509" price="10" /><npcs><npc>1</npc></npcs></list>"#;
        let before = r#"<list><npcs><npc>1</npc></npcs><item id="2509" price="10" /></list>"#;
        let lists = BuyLists::from_xml(
            &[("1.xml", after), ("2.xml", before)],
            &items(),
            &BuyListOptions::default(),
        );
        assert_eq!(lists.get(1).unwrap().product(2509).unwrap().price, 10);
        assert_eq!(lists.get(2).unwrap().product(2509).unwrap().price, 50);
        // CorrectPrices = False: never.
        let off = BuyListOptions {
            correct_prices: false,
            ..BuyListOptions::default()
        };
        let lists = BuyLists::from_xml(&[("2.xml", before)], &items(), &off);
        assert_eq!(lists.get(2).unwrap().product(2509).unwrap().price, 10);
    }

    #[test]
    fn a_bad_document_loads_no_list_at_all() {
        let bad = r#"<list><npcs><npc>1</npc></npcs><item id="1835" price="x" /></list>"#;
        let lists = BuyLists::from_xml(&[("5.xml", bad)], &items(), &BuyListOptions::default());
        assert!(lists.get(5).is_none());
    }

    #[test]
    fn a_duplicate_item_replaces_in_place_and_a_list_without_npcs_allows_nobody() {
        let xml = r#"<list><item id="1835" price="1" /><item id="2509" price="2" /><item id="1835" price="3" /></list>"#;
        let lists = BuyLists::from_xml(&[("9.xml", xml)], &items(), &BuyListOptions::default());
        let list = lists.get(9).unwrap();
        let prices: Vec<(i32, i64)> = list.products().iter().map(|p| (p.item_id, p.price)).collect();
        assert_eq!(prices, [(1835, 3), (2509, 2)]);
        assert!(!list.is_npc_allowed(1) && list.npcs_allowed().is_none());
        assert_eq!(list.product_index(2509), Some(1));
    }

    #[test]
    fn price_configs_pick_by_zone_then_default() {
        let xml = r#"<merchantPriceConfig defaultPriceConfig="18">
            <priceConfig id="0" name="Giran" baseTax="10" castleId="3" zoneId="11020" />
            <priceConfig id="5" name="Gludin" baseTax="20" castleId="1" zoneId="11025" />
            <priceConfig id="18" name="Neutral" baseTax="50" />
        </merchantPriceConfig>"#;
        let table = MerchantPriceConfigs::from_xml(xml).unwrap();
        let zones: HashSet<i32> = [11025, 11020].into_iter().collect();
        // Lowest config id first, like the HashMap walk.
        assert_eq!(table.for_zones(&zones).unwrap().name, "Giran");
        let none: HashSet<i32> = HashSet::new();
        let default = table.for_zones(&none).unwrap();
        assert_eq!(
            (default.id, default.base_tax, default.castle_id, default.zone_id),
            (18, 50, -1, -1)
        );
        assert!((default.base_tax_rate() - 0.5).abs() < f64::EPSILON);
    }

    #[test]
    fn a_missing_default_is_an_error() {
        let xml = r#"<merchantPriceConfig defaultPriceConfig="7"><priceConfig id="0" name="a" baseTax="1" /></merchantPriceConfig>"#;
        assert!(MerchantPriceConfigs::from_xml(xml).is_err());
        let no_attr =
            r#"<merchantPriceConfig><priceConfig id="0" name="a" baseTax="1" /></merchantPriceConfig>"#;
        assert!(MerchantPriceConfigs::from_xml(no_attr).is_err());
    }
}
