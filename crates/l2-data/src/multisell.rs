//! Multisell lists: the exchange windows of NPCs (`data/multisell/*.xml`).
//!
//! Java: `gameserver/data/xml/MultisellData.java` (the loader) and
//! `gameserver/mechanics/multisell/{Entry,Ingredient,ItemInfo,ListContainer}.java`.
//! Data: `data/multisell/<listId>.xml` (`xsd/multisell.xsd`), and `data/multisell/custom` when
//! `General.ini` `CustomMultisellLoad` is on.
//!
//! The types here are the *templates* of the lists. `l2-world` builds a per player copy from them
//! (Java `PreparedListContainer` / `PreparedEntry`) when a list is opened; the copy uses the same
//! [`Entry`] and [`Ingredient`] types, as in Java.
//!
//! # Ingredients carry their item template data
//! Java's `Ingredient` caches the `ItemTemplate` of an item id above 0 when it is created. The
//! cache is [`IngredientTemplate`] here: the few template values the list code reads (display id,
//! body part mask, `type2`, weight, stackable, armor or weapon, default enchant level). A list
//! therefore needs no item table after loading.
//!
//! # Parsing
//! The files are read with an event reader, not serde: Java walks the DOM in document order
//! (`item` and `npcs` elements may come in any order, names are matched ignoring case) and a
//! single bad attribute drops the whole file. Both are awkward to express with serde.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use quick_xml::Reader;
use quick_xml::events::{BytesStart, Event};
use tracing::{info, warn};

use crate::item::{ADENA_ID, ItemTemplate, ItemTemplates};

/// Java `MultisellData.PAGE_SIZE`: entries per `MultiSellList` packet.
pub const PAGE_SIZE: usize = 40;
/// Java `MultisellData.PC_CAFE_POINTS`: a special ingredient or product id.
pub const PC_CAFE_POINTS: i32 = -100;
/// Java `MultisellData.CLAN_REPUTATION`.
pub const CLAN_REPUTATION: i32 = -200;
/// Java `MultisellData.FAME`.
pub const FAME: i32 = -300;

/// The config values the loader reads. [`Default`] is Java's code default of each key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MultisellLoadOptions {
    /// `General.ini` `CustomMultisellLoad` (default false): also load `data/multisell/custom`.
    pub custom_multisell_load: bool,
    /// `General.ini` `CorrectPrices` (default true): a one adena for one item entry never pays
    /// less than half the item's reference price.
    pub correct_prices: bool,
}

impl Default for MultisellLoadOptions {
    fn default() -> Self {
        Self {
            custom_multisell_load: false,
            correct_prices: true,
        }
    }
}

/// Java `ItemInfo`: the enchantment, augmentation and elements an ingredient or product shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ItemInfo {
    /// Java `_enchantLevel`.
    pub enchant_level: i32,
    /// Java `_augmentId`: the augmentation id, 0 for none.
    pub augment_id: i32,
    /// Java `_elementId` (a `byte`): `Elementals.NONE` is -1; an item that is not a weapon
    /// reports -2.
    pub element_id: i32,
    /// Java `_elementPower`.
    pub element_power: i32,
    /// Java `_elementals`: fire, water, wind, earth, holy, dark defense.
    pub elementals: [i32; 6],
}

impl ItemInfo {
    /// Java `new ItemInfo(int enchantLevel)`: only an enchant level, no element (`NONE`).
    pub fn with_enchant(enchant_level: i32) -> Self {
        Self {
            enchant_level,
            augment_id: 0,
            element_id: -1,
            element_power: 0,
            elementals: [0; 6],
        }
    }
}

/// What Java's `Ingredient` reads from its cached `ItemTemplate`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IngredientTemplate {
    /// `getDisplayId()`.
    pub display_id: i32,
    /// `getBodyPart().getMask()`.
    pub body_part_mask: i32,
    /// `getType2()`.
    pub type2: i32,
    /// `getWeight()`.
    pub weight: i32,
    /// `isStackable()`.
    pub stackable: bool,
    /// `(template instanceof Armor) || (template instanceof Weapon)`.
    pub armor_or_weapon: bool,
    /// `getDefaultEnchantLevel()`.
    pub default_enchant_level: i32,
}

impl IngredientTemplate {
    fn of(template: &ItemTemplate) -> Self {
        Self {
            display_id: template.display_id,
            body_part_mask: template.body_part.mask(),
            type2: template.type2,
            weight: template.weight,
            stackable: template.stackable,
            armor_or_weapon: template.is_armor() || template.is_weapon(),
            default_enchant_level: template.default_enchant_level,
        }
    }
}

/// Java `Ingredient`: an item (or a special id below 0) with a count; used for both sides of an
/// entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ingredient {
    /// Java `_itemId`: an item id, or one of [`PC_CAFE_POINTS`], [`CLAN_REPUTATION`], [`FAME`].
    pub item_id: i32,
    /// Java `_itemCount`.
    pub item_count: i64,
    /// Java `_enchantmentLevel`: the level the XML asks for.
    pub enchantment_level: i32,
    /// Java `_isTaxIngredient`: the adena of this ingredient is the castle tax base.
    pub is_tax_ingredient: bool,
    /// Java `_maintainIngredient`: the item is checked but not taken.
    pub maintain_ingredient: bool,
    /// Java `_template`: set for an item id above 0 that exists.
    pub template: Option<IngredientTemplate>,
    /// Java `_itemInfo`: set when a list is prepared for a player.
    pub item_info: Option<ItemInfo>,
}

impl Ingredient {
    /// Java `new Ingredient(itemId, itemCount, enchantmentLevel, isTaxIngredient,
    /// maintainIngredient)`; the template comes from `items` for an id above 0.
    pub fn new(
        item_id: i32,
        item_count: i64,
        enchantment_level: i32,
        is_tax_ingredient: bool,
        maintain_ingredient: bool,
        items: &ItemTemplates,
    ) -> Self {
        let template = if item_id > 0 {
            items.get(item_id).map(IngredientTemplate::of)
        } else {
            None
        };
        Self {
            item_id,
            item_count,
            enchantment_level,
            is_tax_ingredient,
            maintain_ingredient,
            template,
            item_info: None,
        }
    }

    /// Java `getCopy()`. NOTE(java-quirk): the copy has no `ItemInfo`, even when this one has.
    pub fn copy(&self) -> Self {
        Self {
            item_info: None,
            ..self.clone()
        }
    }

    /// Java `getEnchantLevel()`: the level of the item info when there is one.
    pub fn enchant_level(&self) -> i32 {
        self.item_info
            .as_ref()
            .map_or(self.enchantment_level, |info| info.enchant_level)
    }

    /// Java `isStackable()`: true without a template.
    pub fn is_stackable(&self) -> bool {
        self.template.is_none_or(|t| t.stackable)
    }

    /// Java `isArmorOrWeapon()`.
    pub fn is_armor_or_weapon(&self) -> bool {
        self.template.is_some_and(|t| t.armor_or_weapon)
    }

    /// Java `getWeight()`: 0 without a template.
    pub fn weight(&self) -> i32 {
        self.template.map_or(0, |t| t.weight)
    }
}

/// Java `Entry`: one line of a list. Also Java `PreparedEntry` (with a tax amount).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    /// Java `_entryId`: 1, 2, 3 ... in file order; a prepared entry has `id * 100000` (+ the
    /// enchant level when the list maintains it).
    pub entry_id: i32,
    /// Java `_stackable`: false as soon as one product is not stackable.
    pub stackable: bool,
    /// Java `_products`.
    pub products: Vec<Ingredient>,
    /// Java `_ingredients`.
    pub ingredients: Vec<Ingredient>,
    /// Java `PreparedEntry._taxAmount`; `Entry.getTaxAmount()` is 0.
    pub tax_amount: i64,
}

impl Entry {
    /// Java `new Entry(entryId)`.
    pub fn new(entry_id: i32) -> Self {
        Self {
            entry_id,
            stackable: true,
            products: Vec::new(),
            ingredients: Vec::new(),
            tax_amount: 0,
        }
    }

    /// Java `addProduct`.
    pub fn add_product(&mut self, product: Ingredient) {
        if !product.is_stackable() {
            self.stackable = false;
        }
        self.products.push(product);
    }
}

/// Java `ListContainer`: one multisell file.
#[derive(Debug, Clone, PartialEq)]
pub struct ListContainer {
    /// Java `_listId`: the file name.
    pub list_id: i32,
    /// Java `_applyTaxes` (`applyTaxes="true"`).
    pub apply_taxes: bool,
    /// Java `_maintainEnchantment` (`maintainEnchantment="true"`).
    pub maintain_enchantment: bool,
    /// Java `_useRate` (`useRate`); Java never reads it again.
    pub use_rate: f64,
    /// Java `_entries`.
    pub entries: Vec<Entry>,
    /// Java `_npcsAllowed`: empty is Java's `null` (nobody listed). The id -1 allows every NPC
    /// (and no NPC at all).
    pub npcs_allowed: BTreeSet<i32>,
}

impl ListContainer {
    /// Java `new ListContainer(listId)`.
    pub fn new(list_id: i32) -> Self {
        Self {
            list_id,
            apply_taxes: false,
            maintain_enchantment: false,
            use_rate: 1.0,
            entries: Vec::new(),
            npcs_allowed: BTreeSet::new(),
        }
    }

    /// Java `isNpcAllowed(npcId)`.
    pub fn is_npc_allowed(&self, npc_id: i32) -> bool {
        self.npcs_allowed.contains(&npc_id)
    }
}

/// Java `MultisellData`: every list by id. Immutable after loading.
#[derive(Debug, Clone, Default)]
pub struct MultisellData {
    lists: BTreeMap<i32, ListContainer>,
}

impl MultisellData {
    const LOADER: &'static str = "MultisellData";

    /// Java `load()`: `data/multisell`, then `data/multisell/custom` with `CustomMultisellLoad`
    /// (a custom list with the same id replaces the shipped one), then `verify()`.
    pub fn load(root: &Path, items: &ItemTemplates, options: &MultisellLoadOptions) -> Self {
        let mut table = Self::default();
        table.load_dir(&root.join("data/multisell"), items, options);
        if options.custom_multisell_load {
            table.load_dir(&root.join("data/multisell/custom"), items, options);
        }
        table.verify();
        info!("{}: Loaded {} multisell lists.", Self::LOADER, table.lists.len());
        table
    }

    /// Builds a table from `(list id, xml text)` pairs. For tests and tools.
    pub fn from_xml(files: &[(i32, &str)], items: &ItemTemplates, options: &MultisellLoadOptions) -> Self {
        let mut table = Self::default();
        for (id, xml) in files {
            table.add_file(*id, xml, "<inline>", items, options);
        }
        table.verify();
        table
    }

    /// Java `parseDatapackDirectory(dir, false)` with `isValidXmlFile`: only `<digits>.xml`.
    fn load_dir(&mut self, dir: &Path, items: &ItemTemplates, options: &MultisellLoadOptions) {
        let Ok(read) = std::fs::read_dir(dir) else {
            warn!("Directory not found: {}", dir.display());
            return;
        };
        let mut files: Vec<(i32, String, std::path::PathBuf)> = Vec::new();
        for entry in read.flatten() {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_lowercase();
            if !path.is_file() || !is_valid_file_name(&name) {
                continue;
            }
            // Java `Integer.parseInt(file.getName().replace(".xml", ""))`: too many digits throw,
            // and the file is reported.
            let Ok(id) = name.trim_end_matches(".xml").parse::<i32>() else {
                warn!("{}: Error in file {}", Self::LOADER, path.display());
                continue;
            };
            files.push((id, name, path));
        }
        // Sorted, so the result does not depend on the OS listing order.
        files.sort();
        for (id, name, path) in files {
            match std::fs::read(&path) {
                Ok(bytes) => {
                    let text = String::from_utf8_lossy(&bytes);
                    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
                    self.add_file(id, text, &name, items, options);
                }
                Err(e) => warn!("{}: Error in file {}: {e}", Self::LOADER, path.display()),
            }
        }
    }

    fn add_file(
        &mut self,
        id: i32,
        xml: &str,
        file: &str,
        items: &ItemTemplates,
        options: &MultisellLoadOptions,
    ) {
        match parse_list(id, xml, items, options) {
            Ok(list) => {
                self.lists.insert(id, list);
            }
            Err(error) => warn!("{}: Error in file {file}: {error}", Self::LOADER),
        }
    }

    /// Java `verify()`: every item of every entry must exist.
    fn verify(&self) {
        for list in self.lists.values() {
            for entry in &list.entries {
                for ingredient in &entry.ingredients {
                    if !verify_ingredient(ingredient) {
                        warn!(
                            "{}: can't find ingredient with itemId: {} in list: {}",
                            Self::LOADER,
                            ingredient.item_id,
                            list.list_id
                        );
                    }
                }
                for product in &entry.products {
                    if !verify_ingredient(product) {
                        warn!(
                            "{}: can't find product with itemId: {} in list: {}",
                            Self::LOADER,
                            product.item_id,
                            list.list_id
                        );
                    }
                }
            }
        }
    }

    /// Java `_entries.get(listId)`.
    pub fn get(&self, list_id: i32) -> Option<&ListContainer> {
        self.lists.get(&list_id)
    }

    /// How many lists were loaded.
    pub fn len(&self) -> usize {
        self.lists.len()
    }

    /// No list at all.
    pub fn is_empty(&self) -> bool {
        self.lists.is_empty()
    }
}

/// Java `verifyIngredient`: the special ids are fine, an item needs its template.
fn verify_ingredient(ingredient: &Ingredient) -> bool {
    match ingredient.item_id {
        PC_CAFE_POINTS | CLAN_REPUTATION | FAME => true,
        _ => ingredient.template.is_some(),
    }
}

/// Java `XML_FILE_PATTERN` `\d+\.xml` on the lower cased file name.
fn is_valid_file_name(name: &str) -> bool {
    name.strip_suffix(".xml")
        .is_some_and(|stem| !stem.is_empty() && stem.bytes().all(|b| b.is_ascii_digit()))
}

/// Java `Boolean.parseBoolean`.
fn parse_boolean(value: &str) -> bool {
    value.eq_ignore_ascii_case("true")
}

/// The attributes of an element as `(name, value)` (Java copies them into a `StatSet`).
fn attributes(element: &BytesStart<'_>) -> Result<Vec<(String, String)>, String> {
    let mut out = Vec::new();
    for attr in element.attributes() {
        let attr = attr.map_err(|e| e.to_string())?;
        let value = attr
            .normalized_value(quick_xml::XmlVersion::Implicit1_0)
            .map_err(|e| e.to_string())?
            .into_owned();
        out.push((attr.key.as_ref().to_owned(), value));
    }
    Ok(out)
}

fn attr<'a>(attrs: &'a [(String, String)], name: &str) -> Option<&'a str> {
    attrs.iter().find(|(k, _)| k == name).map(|(_, v)| v.as_str())
}

/// Java `new Ingredient(StatSet)`: `id` and `count` are required, the rest has defaults. A value
/// that does not parse throws in Java (`StatSet`), which drops the whole file.
fn parse_ingredient(attrs: &[(String, String)], items: &ItemTemplates) -> Result<Ingredient, String> {
    let id = attr(attrs, "id").ok_or("Integer value required, but not specified: id!")?;
    let id: i32 = id
        .parse()
        .map_err(|_| format!("Integer value required, but found: {id}!"))?;
    let count = attr(attrs, "count").ok_or("Long value required, but not specified")?;
    let count: i64 = count
        .parse()
        .map_err(|_| format!("Long value required, but found: {count}"))?;
    let enchantment_level = match attr(attrs, "enchantmentLevel") {
        None => 0,
        Some(v) => v
            .parse()
            .map_err(|_| format!("Integer value required, but found: {v}"))?,
    };
    let is_tax_ingredient = attr(attrs, "isTaxIngredient").is_some_and(parse_boolean);
    let maintain_ingredient = attr(attrs, "maintainIngredient").is_some_and(parse_boolean);
    Ok(Ingredient::new(
        id,
        count,
        enchantment_level,
        is_tax_ingredient,
        maintain_ingredient,
        items,
    ))
}

/// Java `parseDocument` + `parseEntry` for one file.
fn parse_list(
    id: i32,
    xml: &str,
    items: &ItemTemplates,
    options: &MultisellLoadOptions,
) -> Result<ListContainer, String> {
    let mut list = ListContainer::new(id);
    let mut reader = Reader::from_str(xml);
    // Lower cased names of the open elements.
    let mut path: Vec<String> = Vec::new();
    let mut entry_id = 1;
    let mut entry: Option<Entry> = None;
    let mut npc_text = String::new();
    loop {
        let event = reader.read_event().map_err(|e| e.to_string())?;
        let (element, empty) = match &event {
            Event::Start(e) => (Some(e), false),
            Event::Empty(e) => (Some(e), true),
            _ => (None, false),
        };
        if let Some(e) = element {
            let name = e.name().as_ref().to_lowercase();
            let parent: Vec<&str> = path.iter().map(String::as_str).collect();
            match (parent.as_slice(), name.as_str()) {
                ([], "list") => {
                    let attrs = attributes(e)?;
                    list.apply_taxes = attr(&attrs, "applyTaxes").is_some_and(parse_boolean);
                    if let Some(value) = attr(&attrs, "useRate") {
                        list.use_rate = parse_use_rate(value);
                    }
                    list.maintain_enchantment =
                        attr(&attrs, "maintainEnchantment").is_some_and(parse_boolean);
                }
                (["list"], "item") => entry = Some(Entry::new(entry_id)),
                (["list", "item"], "ingredient") => {
                    let ingredient = parse_ingredient(&attributes(e)?, items)?;
                    if let Some(entry) = entry.as_mut() {
                        entry.ingredients.push(ingredient);
                    }
                }
                (["list", "item"], "production") => {
                    let product = parse_ingredient(&attributes(e)?, items)?;
                    if let Some(entry) = entry.as_mut() {
                        entry.add_product(product);
                    }
                }
                (["list", "npcs"], "npc") => npc_text.clear(),
                _ => {}
            }
            if empty {
                // An empty element closes at once.
                path.push(name);
                close_element(
                    &mut path,
                    &mut list,
                    &mut entry,
                    &mut entry_id,
                    &npc_text,
                    items,
                    options,
                )?;
            } else {
                path.push(name);
            }
            continue;
        }
        match event {
            Event::End(_) => {
                close_element(
                    &mut path,
                    &mut list,
                    &mut entry,
                    &mut entry_id,
                    &npc_text,
                    items,
                    options,
                )?;
            }
            Event::Text(t) => {
                if path.len() == 3 && path[2] == "npc" {
                    npc_text.push_str(t.as_ref());
                }
            }
            Event::CData(t) => {
                if path.len() == 3 && path[2] == "npc" {
                    npc_text.push_str(t.as_ref());
                }
            }
            Event::Eof => {
                // The DOM parser rejects a document that ends inside an element.
                if !path.is_empty() {
                    return Err("XML document structures must start and end within the same entity.".into());
                }
                break;
            }
            _ => {}
        }
    }
    Ok(list)
}

/// The end of the element on top of `path`.
fn close_element(
    path: &mut Vec<String>,
    list: &mut ListContainer,
    entry: &mut Option<Entry>,
    entry_id: &mut i32,
    npc_text: &str,
    items: &ItemTemplates,
    options: &MultisellLoadOptions,
) -> Result<(), String> {
    let closed: Vec<&str> = path.iter().map(String::as_str).collect();
    match closed.as_slice() {
        ["list", "item"] => {
            if let Some(mut finished) = entry.take() {
                correct_price(&mut finished, list.list_id, items, options);
                list.entries.push(finished);
                *entry_id += 1;
            }
        }
        ["list", "npcs", "npc"] => {
            // `Integer.parseInt(b.getTextContent())`: no trimming, a failure drops the file.
            let npc: i32 = npc_text
                .parse()
                .map_err(|_| format!("For input string: \"{npc_text}\""))?;
            list.npcs_allowed.insert(npc);
        }
        _ => {}
    }
    path.pop();
    Ok(())
}

/// Java `Double.parseDouble(useRate)`; a value of 0 or less, or one that is not a number, would
/// look the name up as a `ConfigLoader` field. NOTE(java-quirk): that lookup needs reflection on
/// the config class and is not ported; its failure path (a warning and the rate 1.0) is what
/// every such value gets. Java never reads the rate afterwards.
fn parse_use_rate(value: &str) -> f64 {
    match value.trim().parse::<f64>() {
        Ok(rate) if rate > 1e-6 => rate,
        _ => {
            warn!("MultisellData: useRate \"{value}\" is not a number above 0; using 1.0");
            1.0
        }
    }
}

/// The `CORRECT_PRICES` part of Java `parseEntry`: with one adena ingredient and one product, the
/// adena is raised to half the product's reference price times its count.
fn correct_price(entry: &mut Entry, list_id: i32, items: &ItemTemplates, options: &MultisellLoadOptions) {
    if !options.correct_prices || entry.ingredients.len() != 1 || entry.products.len() != 1 {
        return;
    }
    let ingredient = &mut entry.ingredients[0];
    if ingredient.item_id != ADENA_ID {
        return;
    }
    let mut total_price: i64 = 0;
    for product in &entry.products {
        let Some(template) = items.get(product.item_id) else {
            continue;
        };
        total_price =
            total_price.wrapping_add(product.item_count.wrapping_mul(i64::from(template.price / 2)));
    }
    let adena_count = ingredient.item_count;
    if total_price > adena_count {
        ingredient.item_count = total_price;
        warn!(
            "Buy price {adena_count} is less than sell price {total_price} at entry {} of multisell {list_id}.",
            entry.entry_id
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ITEMS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
  <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
  <item id="22" type="Armor" name="Leather Shirt"><set name="bodypart" val="underwear"/><set name="price" val="1000"/></item>
  <item id="21" type="Armor" name="Shirt"><set name="bodypart" val="underwear"/><set name="price" val="2000"/></item>
  <item id="1" type="Weapon" name="Short Sword"><set name="bodypart" val="rhand"/><set name="enchanted" val="3"/></item>
</list>"#;

    fn items() -> ItemTemplates {
        ItemTemplates::from_xml(ITEMS).unwrap()
    }

    fn one(xml: &str, options: &MultisellLoadOptions) -> ListContainer {
        let data = MultisellData::from_xml(&[(7, xml)], &items(), options);
        data.get(7).cloned().unwrap()
    }

    const LIST: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list applyTaxes="true" maintainEnchantment="TRUE" useRate="2.5">
  <!-- comment -->
  <item>
    <ingredient count="2100" id="57" />
    <ingredient count="1" id="21" enchantmentLevel="2" maintainIngredient="true" />
    <production count="1" id="22" />
  </item>
  <npcs><npc>31688</npc><npc>32024</npc></npcs>
  <item>
    <ingredient count="10" id="57" isTaxIngredient="true" />
    <production count="3" id="57" />
    <production count="1" id="-300" />
  </item>
</list>"#;

    #[test]
    fn a_list_reads_its_attributes_entries_and_npcs() {
        let list = one(
            LIST,
            &MultisellLoadOptions {
                correct_prices: false,
                ..Default::default()
            },
        );
        assert_eq!(list.list_id, 7);
        assert!(list.apply_taxes);
        assert!(list.maintain_enchantment);
        assert!((list.use_rate - 2.5).abs() < f64::EPSILON);
        assert_eq!(list.npcs_allowed, BTreeSet::from([31688, 32024]));
        assert_eq!(
            list.entries.iter().map(|e| e.entry_id).collect::<Vec<_>>(),
            vec![1, 2]
        );
        let first = &list.entries[0];
        assert_eq!(first.ingredients.len(), 2);
        assert_eq!(first.ingredients[1].enchantment_level, 2);
        assert!(first.ingredients[1].maintain_ingredient);
        assert!(first.ingredients[1].is_armor_or_weapon());
        assert_eq!(first.products[0].item_id, 22);
        assert!(first.stackable == !first.products[0].template.is_some_and(|t| !t.stackable));
        let second = &list.entries[1];
        assert!(second.ingredients[0].is_tax_ingredient);
        // The special product has no template and counts as stackable.
        assert!(second.products[1].template.is_none());
        assert!(second.products[1].is_stackable());
    }

    #[test]
    fn a_non_stackable_product_makes_the_entry_non_stackable() {
        let list = one(LIST, &MultisellLoadOptions::default());
        // Leather Shirt is not stackable.
        assert!(!list.entries[0].stackable);
        // Adena and the fame id are.
        assert!(list.entries[1].stackable);
    }

    #[test]
    fn a_list_without_npcs_allows_nobody() {
        let list = one(
            r#"<list><item><ingredient count="1" id="57"/><production count="1" id="57"/></item></list>"#,
            &MultisellLoadOptions::default(),
        );
        assert!(list.npcs_allowed.is_empty());
        assert!(!list.is_npc_allowed(-1));
    }

    #[test]
    fn element_names_are_matched_ignoring_case() {
        let list = one(
            r#"<LIST><ITEM><Ingredient count="1" id="57"/><PRODUCTION count="1" id="57"/></ITEM><NPCS><NPC>-1</NPC></NPCS></LIST>"#,
            &MultisellLoadOptions::default(),
        );
        assert_eq!(list.entries.len(), 1);
        assert!(list.is_npc_allowed(-1));
    }

    #[test]
    fn correct_prices_raises_a_too_cheap_adena_ingredient() {
        // Product Shirt: price 2000, so half is 1000; three of them are 3000.
        let xml = r#"<list>
  <item><ingredient count="100" id="57"/><production count="3" id="21"/></item>
  <item><ingredient count="5000" id="57"/><production count="3" id="21"/></item>
  <item><ingredient count="100" id="57"/><ingredient count="1" id="22"/><production count="3" id="21"/></item>
</list>"#;
        let on = one(xml, &MultisellLoadOptions::default());
        assert_eq!(on.entries[0].ingredients[0].item_count, 3000);
        assert_eq!(on.entries[1].ingredients[0].item_count, 5000);
        // Only a single ingredient is checked.
        assert_eq!(on.entries[2].ingredients[0].item_count, 100);
        let off = one(
            xml,
            &MultisellLoadOptions {
                correct_prices: false,
                ..Default::default()
            },
        );
        assert_eq!(off.entries[0].ingredients[0].item_count, 100);
    }

    #[test]
    fn a_bad_value_drops_the_whole_file() {
        let items = items();
        let options = MultisellLoadOptions::default();
        for xml in [
            r#"<list><item><ingredient count="x" id="57"/></item></list>"#,
            r#"<list><item><ingredient id="57"/></item></list>"#,
            r#"<list><item><ingredient count="1"/></item></list>"#,
            r#"<list><item><ingredient count="1" id="57" enchantmentLevel="a"/></item></list>"#,
            r#"<list><npcs><npc> 5 </npc></npcs></list>"#,
            "<list><item>",
        ] {
            let data = MultisellData::from_xml(&[(1, xml)], &items, &options);
            assert!(data.is_empty(), "{xml}");
        }
        // A boolean that is not "true" is false, not an error.
        let data = MultisellData::from_xml(
            &[(
                1,
                r#"<list applyTaxes="yes"><item><ingredient count="1" id="57" maintainIngredient="1"/></item></list>"#,
            )],
            &items,
            &options,
        );
        let list = data.get(1).unwrap();
        assert!(!list.apply_taxes);
        assert!(!list.entries[0].ingredients[0].maintain_ingredient);
    }

    #[test]
    fn a_bad_use_rate_falls_back_to_one() {
        for value in ["0", "-3", "abc"] {
            let xml = format!(r#"<list useRate="{value}"/>"#);
            assert!((one(&xml, &MultisellLoadOptions::default()).use_rate - 1.0).abs() < f64::EPSILON);
        }
    }

    #[test]
    fn ingredient_copy_drops_the_item_info() {
        let mut ingredient = Ingredient::new(1, 1, 5, false, false, &items());
        ingredient.item_info = Some(ItemInfo::with_enchant(9));
        assert_eq!(ingredient.enchant_level(), 9);
        let copy = ingredient.copy();
        assert_eq!(copy.enchant_level(), 5);
        assert_eq!(copy.item_id, 1);
        assert_eq!(ItemInfo::with_enchant(2).element_id, -1);
    }

    #[test]
    fn file_names_must_be_digits_and_xml() {
        assert!(is_valid_file_name("1235.xml"));
        assert!(!is_valid_file_name("custom"));
        assert!(!is_valid_file_name("12a.xml"));
        assert!(!is_valid_file_name(".xml"));
    }

    #[test]
    fn the_folders_load_and_custom_replaces_a_shipped_list() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("data/multisell");
        std::fs::create_dir_all(root.join("custom")).unwrap();
        let one_entry =
            r#"<list><item><ingredient count="1" id="57"/><production count="1" id="57"/></item></list>"#;
        let two_entries = r#"<list><item><ingredient count="1" id="57"/><production count="1" id="57"/></item><item><ingredient count="2" id="57"/><production count="1" id="57"/></item></list>"#;
        std::fs::write(root.join("5.xml"), one_entry).unwrap();
        std::fs::write(root.join("notes.txt"), "x").unwrap();
        std::fs::write(root.join("abc.xml"), one_entry).unwrap();
        std::fs::write(root.join("custom/5.xml"), two_entries).unwrap();
        std::fs::write(root.join("custom/6.xml"), one_entry).unwrap();

        let plain = MultisellData::load(dir.path(), &items(), &MultisellLoadOptions::default());
        assert_eq!(plain.len(), 1);
        assert_eq!(plain.get(5).unwrap().entries.len(), 1);

        let custom = MultisellData::load(
            dir.path(),
            &items(),
            &MultisellLoadOptions {
                custom_multisell_load: true,
                ..Default::default()
            },
        );
        assert_eq!(custom.len(), 2);
        assert_eq!(custom.get(5).unwrap().entries.len(), 2);
    }

    /// The shipped lists load, when the reference folder is there (CI has none).
    #[test]
    fn the_shipped_lists_load() {
        let root = Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game"));
        if !root.join("data/multisell").is_dir() {
            eprintln!("skipped: no reference datapack");
            return;
        }
        let items = ItemTemplates::load(root, false);
        let data = MultisellData::load(root, &items, &MultisellLoadOptions::default());
        // Java logs "Loaded 200 multisell lists." for the shipped folder.
        assert_eq!(data.len(), 200);
        let clan_trader = data.get(1235).unwrap();
        assert!(clan_trader.is_npc_allowed(32024));
        assert_eq!(clan_trader.entries[0].ingredients[0].item_id, CLAN_REPUTATION);
        assert_eq!(clan_trader.entries[0].ingredients[0].item_count, 696);
        let fame = data.get(103).unwrap();
        assert_eq!(fame.entries[0].products[0].item_id, FAME);
        assert_eq!(fame.entries[0].products[0].item_count, 100);
    }
}
