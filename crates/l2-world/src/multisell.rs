//! Multisell: the exchange windows of NPCs. Opening a list for a player (`PreparedListContainer`,
//! `PreparedEntry`, the `MultiSellList` pages), the `multisell` / `exc_multisell` bypasses and
//! `MultiSellChoose`.
// Java: data/xml/MultisellData.java (separateAndSend, hasSpecialIngredient, takeSpecialIngredient,
//       giveSpecialProduct)
// Java: mechanics/multisell/{PreparedListContainer,PreparedEntry,ItemInfo}.java
// Java: network/clientpackets/MultiSellChoose.java, network/serverpackets/MultiSellList.java
// Java: dist/game/data/scripts/handlers/bypass/npc/Multisell.java
//
// # Model
// The lists are the datapack's ([`l2_data::MultisellData`], handed to the world with
// [`World::set_multisell_data`]). Opening one builds a [`PreparedList`] for the player, stored on
// the [`Player`](crate::Player) (Java `Player._multiSell`): every `MultiSellChoose` is checked
// against that copy, not against the template, so the client can only pick what it was shown.
// Entry ids of the copy are `template id * 100000` (+ the enchant level for a list that keeps
// enchantments).
//
// # Items
// Ingredients are taken and products added through the inventory module
// (`destroy_item_by_object_id`, `inventory_add_item_by_id`), which sends the same
// `InventoryUpdate`s as Java. An enchanted, augmented or elemental product is written to the
// database with `update_item_in_db`, like Java's `product.updateDatabase()`.
//
// # Left out on purpose (each has a comment where it belongs)
// - (Castle taxes are in since M7-RES-01: an `applyTaxes` list adds the tax of the castle that
//   owns the NPC's town to the adena ingredient and pays it into that castle's treasury.)
// - `ItemProcessType` logging and the GM audit.
// - PC cafe points (`-100`): the world does not model them yet, so a player has 0.
// - Fame is changed in the world but the persistence task does not write it back yet.

use std::collections::BTreeSet;
use std::sync::Arc;

use l2_data::item::{ADENA_ID, ANCIENT_ADENA_ID, ItemTemplate};
use l2_data::multisell::{
    CLAN_REPUTATION, Entry, FAME, Ingredient, IngredientTemplate, ItemInfo, MultisellData, PAGE_SIZE,
    PC_CAFE_POINTS,
};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    MultiSellEntry, MultiSellIngredient, MultiSellItemDetail, MultiSellList, MultiSellProduct, SystemMessage,
    SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::warn;

use crate::dialog::INTERACTION_DISTANCE;
use crate::flood::FloodKind;
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL, item_name_param};
use crate::player::ItemData;
use crate::target::distance_3d;
use crate::world::World;

/// The config values multisell reads. [`Default`] is the Java default of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct MultisellSettings {
    /// `General.ini` `MultisellAmountLimit` (default 10000): the most a `MultiSellChoose` may ask for.
    pub amount_limit: i64,
    /// `Player.ini` `AltBlacksmithUseRecipes` (default true): `maintainIngredient` items are used
    /// up like any other ingredient.
    pub alt_blacksmith_use_recipes: bool,
    /// `Player.ini` `MaxPersonalFamePoints` (default 100000): `Player.setFame` stops there.
    pub max_personal_fame_points: i32,
}

impl Default for MultisellSettings {
    fn default() -> Self {
        Self {
            amount_limit: 10_000,
            alt_blacksmith_use_recipes: true,
            max_personal_fame_points: 100_000,
        }
    }
}

/// The lists and settings, owned by the world.
#[derive(Debug, Default)]
pub(crate) struct MultisellWorld {
    data: Option<Arc<MultisellData>>,
    settings: MultisellSettings,
}

/// Java `PreparedListContainer`: a list as one player was shown it.
#[derive(Debug, Clone, PartialEq)]
pub struct PreparedList {
    /// Java `getListId()`.
    pub list_id: i32,
    /// Java `getMaintainEnchantment()`.
    pub maintain_enchantment: bool,
    /// Java `getApplyTaxes()`: the castle tax is in the adena of the entries.
    pub apply_taxes: bool,
    /// Java `_entries`: prepared entries (ids multiplied, item infos set).
    pub entries: Vec<Entry>,
    /// Java `_npcObjectId`: the NPC that opened it, 0 for none.
    pub npc_object_id: i32,
    /// Java `_npcsAllowed` of the template.
    pub npcs_allowed: BTreeSet<i32>,
}

impl PreparedList {
    /// Java `isNpcAllowed(npcId)`.
    pub fn is_npc_allowed(&self, npc_id: i32) -> bool {
        self.npcs_allowed.contains(&npc_id)
    }

    /// Java `checkNpcObjectId(npcObjectId)`.
    pub fn check_npc_object_id(&self, npc_object_id: i32) -> bool {
        self.npc_object_id == 0 || self.npc_object_id == npc_object_id
    }
}

/// Java `new ItemInfo(item)`: what a list shows of an item the player owns.
pub fn item_info_of(item: &ItemData, template: Option<&ItemTemplate>) -> ItemInfo {
    ItemInfo {
        enchant_level: item.enchant_level,
        augment_id: item.augmentation.unwrap_or(0),
        element_id: item.attack_element_type(template),
        element_power: item.attack_element_power(template),
        elementals: std::array::from_fn(|element| item.element_def_attr(element as i32, template)),
    }
}

/// Java `new PreparedEntry(template, item, applyTaxes, maintainEnchantment, taxRate)`; `item` is
/// the info of the inventory item the entry is made for (`None` for a list shown whole).
pub fn prepared_entry(
    template: &Entry,
    item: Option<&ItemInfo>,
    apply_taxes: bool,
    maintain_enchantment: bool,
    tax_rate: f64,
) -> Entry {
    let mut entry = Entry::new(template.entry_id.wrapping_mul(100_000));
    if maintain_enchantment && let Some(item) = item {
        entry.entry_id = entry.entry_id.wrapping_add(item.enchant_level);
    }

    let mut info: Option<ItemInfo> = None;
    let mut adena_amount: i64 = 0;
    let mut tax_amount: i64 = 0;
    let mut adena_template: Option<IngredientTemplate> = None;
    for ingredient in &template.ingredients {
        if ingredient.item_id == ADENA_ID {
            adena_template = ingredient.template;
            // Tax ingredients are added only if taxes are enabled.
            if ingredient.is_tax_ingredient {
                if apply_taxes {
                    // `Math.round`: half up.
                    tax_amount += (ingredient.item_count as f64 * tax_rate + 0.5).floor() as i64;
                }
            } else {
                adena_amount += ingredient.item_count;
            }
            // The adena is added after the loop: non tax adena may come later in the list.
            continue;
        }

        if maintain_enchantment
            && let Some(item) = item
            && ingredient.is_armor_or_weapon()
        {
            info = Some(item.clone());
            let mut copy = ingredient.copy();
            copy.item_info = info.clone();
            entry.ingredients.push(copy);
        } else {
            entry.ingredients.push(ingredient.copy());
        }
    }

    // Now add the adena, if any (the tax with it).
    adena_amount += tax_amount;
    if adena_amount > 0 {
        entry.ingredients.push(Ingredient {
            item_id: ADENA_ID,
            item_count: adena_amount,
            enchantment_level: 0,
            is_tax_ingredient: false,
            maintain_ingredient: false,
            template: adena_template,
            item_info: None,
        });
    }
    entry.tax_amount = tax_amount;

    // Now copy the products.
    for product in &template.products {
        if !product.is_stackable() {
            entry.stackable = false;
        }
        let mut copy = product.copy();
        if maintain_enchantment && product.is_armor_or_weapon() {
            // NOTE(java-quirk): `info` is whatever the last armor or weapon ingredient (or an
            // earlier product with a default enchant level) left; it is null when there was none.
            copy.item_info = info.clone();
        } else if product.is_armor_or_weapon()
            && let Some(default_level) = product.template.map(|t| t.default_enchant_level)
            && default_level > 0
        {
            info = Some(ItemInfo::with_enchant(default_level));
            copy.item_info = info.clone();
        }
        entry.products.push(copy);
    }
    entry
}

/// `(long) Math.max(count * multiplier, 1)`.
fn scaled(count: i64, multiplier: f64) -> i64 {
    (count as f64 * multiplier).max(1.0) as i64
}

/// The `MultiSellList` fields of the item info an ingredient carries.
fn detail_of(ingredient: &Ingredient) -> MultiSellItemDetail {
    match &ingredient.item_info {
        Some(info) => MultiSellItemDetail {
            enchant: info.enchant_level as i16,
            augment_id: info.augment_id,
            attack_element: info.element_id as i16,
            attack_element_power: info.element_power as i16,
            element_defense: info.elementals.map(|v| v as i16),
        },
        None => MultiSellItemDetail {
            enchant: ingredient.enchantment_level as i16,
            ..MultiSellItemDetail::default()
        },
    }
}

/// One entry as `MultiSellList.writeImpl` shows it.
pub fn entry_packet(entry: &Entry) -> MultiSellEntry {
    MultiSellEntry {
        entry_id: entry.entry_id,
        stackable: entry.stackable,
        products: entry
            .products
            .iter()
            .map(|product| match product.template {
                Some(t) => MultiSellProduct {
                    item_id: t.display_id,
                    body_part: t.body_part_mask,
                    type2: t.type2 as u16,
                    count: product.item_count,
                    detail: detail_of(product),
                },
                None => MultiSellProduct {
                    item_id: product.item_id,
                    body_part: 0,
                    type2: 65535,
                    count: product.item_count,
                    detail: detail_of(product),
                },
            })
            .collect(),
        ingredients: entry
            .ingredients
            .iter()
            .map(|ingredient| MultiSellIngredient {
                item_id: ingredient.template.map_or(ingredient.item_id, |t| t.display_id),
                type2: ingredient.template.map_or(65535, |t| t.type2 as u16),
                count: ingredient.item_count,
                detail: detail_of(ingredient),
            })
            .collect(),
    }
}

fn in_inventory(item: &ItemData) -> bool {
    item.loc == LOC_INVENTORY || item.loc == LOC_PAPERDOLL
}

fn is_equipped(item: &ItemData) -> bool {
    item.loc == LOC_PAPERDOLL
}

/// Java `Multisell.onCommand`: `multisell <list>` shows the whole list, `exc_multisell <list>`
/// only what the player's inventory can pay for.
pub(crate) fn multisell_bypass_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    target: Option<ObjectId>,
) -> bool {
    // `if (!target.isNpc()) return false;`
    let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) else {
        return false;
    };
    let lower = command.to_lowercase();
    let (skip, inventory_only) = if lower.starts_with("multisell") {
        (9, false)
    } else if lower.starts_with("exc_multisell") {
        (13, true)
    } else {
        return false;
    };
    // `Integer.parseInt(command.substring(n).trim())`; `trim()` strips every char up to a space.
    let text = command.get(skip..).unwrap_or("").trim_matches(|c: char| c <= ' ');
    match text.parse::<i32>() {
        Ok(list_id) => {
            world.multisell_separate_and_send(list_id, player, Some(npc), inventory_only, 1.0, 1.0);
            true
        }
        Err(_) => {
            warn!("Exception in Multisell: bad list id \"{text}\" in bypass [{command}]");
            false
        }
    }
}

impl World {
    /// Installs the multisell lists and settings. Call before players enter. Calling it again
    /// replaces the lists (Java `AdminReload multisell`).
    pub fn set_multisell_data(&mut self, data: Arc<MultisellData>, settings: MultisellSettings) {
        self.multisell.data = Some(data);
        self.multisell.settings = settings;
    }

    /// Java `MultisellData.separateAndSend(listId, player, npc, inventoryOnly)`: what scripts call.
    pub fn open_multisell(
        &mut self,
        list_id: i32,
        player: ObjectId,
        npc: Option<ObjectId>,
        inventory_only: bool,
    ) {
        self.multisell_separate_and_send(list_id, player, npc, inventory_only, 1.0, 1.0);
    }

    /// Java `Player.setMultiSell(null)`.
    pub fn clear_multisell(&mut self, player: ObjectId) {
        if let Some(p) = self.players.get_mut(&player) {
            p.multisell = None;
        }
    }

    /// Java `PreparedListContainer`'s tax rate: `npc.isInTown() && npc.getCastle().getOwnerId() >
    /// 0` gives `npc.getCastle().getTaxRate()` (the castle of the town the NPC stands in,
    /// M7-RES-01), `None` otherwise.
    fn multisell_tax_rate(&self, npc: ObjectId) -> Option<f64> {
        let (castle, in_town) = self.npc_castle_and_town(npc)?;
        let castle = self.castle(castle).filter(|_| in_town)?;
        (castle.owner_id > 0).then_some(castle.tax_rate)
    }

    /// Java `MultisellData.separateAndSend(listId, player, npc, inventoryOnly, productMultiplier,
    /// ingredientMultiplier)`.
    pub fn multisell_separate_and_send(
        &mut self,
        list_id: i32,
        player: ObjectId,
        npc: Option<ObjectId>,
        inventory_only: bool,
        product_multiplier: f64,
        ingredient_multiplier: f64,
    ) {
        let Some(data) = self.multisell.data.clone() else {
            return;
        };
        let Some(me) = self.players.get(&player) else {
            return;
        };
        let name = me.data.name.clone();
        let npc_template_id = npc.and_then(|n| self.npcs.get(&n)).map(|n| n.npc_id);
        let Some(template) = data.get(list_id) else {
            warn!(
                "MultisellData: can't find list id: {list_id} requested by player: {name}, npcId:{}",
                npc_template_id.unwrap_or(0)
            );
            return;
        };

        if !template.is_npc_allowed(-1) && !npc_template_id.is_some_and(|id| template.is_npc_allowed(id)) {
            if self.is_gm(player) {
                self.send_message(
                    player,
                    &format!(
                        "Multisell {list_id} is restricted. Under current conditions cannot be used. Only GMs are allowed to use it."
                    ),
                );
            } else {
                warn!(
                    "MultisellData: {name} attempted to open multisell {list_id} from npc {} which is not allowed!",
                    npc_template_id.map_or_else(|| "null".to_owned(), |id| id.to_string())
                );
                return;
            }
        }

        // `new PreparedListContainer(template, inventoryOnly, player, npc)`.
        let tax_rate = npc.and_then(|n| self.multisell_tax_rate(n));
        let apply_taxes = template.apply_taxes && tax_rate.is_some();
        let tax_rate = tax_rate.unwrap_or(0.0);
        let maintain = template.maintain_enchantment;
        let mut entries: Vec<Entry> = Vec::new();
        if inventory_only {
            let templates = self.item_templates();
            let owned: Vec<ItemData> = self
                .players
                .get(&player)
                .map(|p| p.data.items.iter().filter(|i| in_inventory(i)).cloned().collect())
                .unwrap_or_default();
            // `getUniqueItemsByEnchantLevel(false, false, false)` /
            // `getUniqueItems(false, false, false, true)`: the first item of each id (and enchant
            // level). The order of Java's item set is arbitrary; the inventory order stands in.
            let mut unique: Vec<&ItemData> = Vec::new();
            for item in &owned {
                if item.item_id == ADENA_ID || item.item_id == ANCIENT_ADENA_ID {
                    continue;
                }
                let duplicate = unique.iter().any(|added| {
                    added.item_id == item.item_id && (!maintain || added.enchant_level == item.enchant_level)
                });
                if !duplicate {
                    unique.push(item);
                }
            }
            for entry in &template.entries {
                let Some(first) = entry.ingredients.first() else {
                    continue;
                };
                // NOTE(java-quirk): the comment of Java says a list that keeps enchantments shows
                // an entry per enchant level, but the loop stops at the first item that matches.
                if let Some(item) = unique
                    .iter()
                    .find(|i| !is_equipped(i) && i.item_id == first.item_id)
                {
                    let info = item_info_of(item, templates.as_deref().and_then(|t| t.get(item.item_id)));
                    entries.push(prepared_entry(
                        entry,
                        Some(&info),
                        apply_taxes,
                        maintain,
                        tax_rate,
                    ));
                }
            }
        } else {
            for entry in &template.entries {
                entries.push(prepared_entry(entry, None, apply_taxes, false, tax_rate));
            }
        }

        // Pass through this only when multipliers are different from 1.
        if product_multiplier != 1.0 || ingredient_multiplier != 1.0 {
            for entry in &mut entries {
                // `Math.max` is used here to avoid dropping the count to 0.
                for product in &mut entry.products {
                    product.item_count = scaled(product.item_count, product_multiplier);
                }
                for ingredient in &mut entry.ingredients {
                    ingredient.item_count = scaled(ingredient.item_count, ingredient_multiplier);
                }
            }
        }

        let list = PreparedList {
            list_id: template.list_id,
            maintain_enchantment: maintain,
            apply_taxes,
            entries,
            npc_object_id: npc.map_or(0, |n| n.0),
            npcs_allowed: template.npcs_allowed.clone(),
        };

        // Send the list at least once, even if it has no entries.
        let mut index = 0;
        loop {
            let (len, finished) = MultiSellList::page_len(list.entries.len(), index);
            let packet = MultiSellList {
                list_id: list.list_id,
                page: MultiSellList::page_number(index),
                finished,
                entries: list.entries[index..index + len]
                    .iter()
                    .map(entry_packet)
                    .collect(),
            };
            self.send(player, &packet);
            index += PAGE_SIZE;
            if index >= list.entries.len() {
                break;
            }
        }
        if let Some(p) = self.players.get_mut(&player) {
            p.multisell = Some(list);
        }
    }

    // ------------------------------------------------------------------ inventory reads

    /// Java `getInventoryItemCount(itemId, enchantLevel, false)`: -1 matches any enchant level;
    /// a stackable item is its count.
    fn multisell_item_count(&self, player: ObjectId, item_id: i32, enchant_level: i32) -> i64 {
        let Some(p) = self.players.get(&player) else {
            return 0;
        };
        let stackable = self.data.item_stackable(item_id);
        let mut count = 0;
        for item in p.data.items.iter().filter(|i| in_inventory(i) && !is_equipped(i)) {
            if item.item_id == item_id && (item.enchant_level == enchant_level || enchant_level < 0) {
                if stackable {
                    return item.count;
                }
                count += 1;
            }
        }
        count
    }

    /// The items of an id that are not worn, in inventory order (Java `getAllItemsByItemId`).
    fn multisell_unequipped(&self, player: ObjectId, item_id: i32, enchant: Option<i32>) -> Vec<ItemData> {
        self.players
            .get(&player)
            .map(|p| {
                p.data
                    .items
                    .iter()
                    .filter(|i| in_inventory(i) && !is_equipped(i) && i.item_id == item_id)
                    .filter(|i| enchant.is_none_or(|e| i.enchant_level == e))
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    // ------------------------------------------------------------------ special ingredients

    /// Java `Player.getPcCafePoints()`.
    // FOLLOW-UP(pccafe): PC cafe points (`characters.pccafe_points`, `ExPCCafePointInfo`) are not
    // modelled; every player has 0, so a list that asks for them can never be paid.
    fn pc_cafe_points(&self, _player: ObjectId) -> i64 {
        0
    }

    /// Java `Player.setFame(fame)`: within 0 and `MaxPersonalFamePoints`.
    // FOLLOW-UP(fame): the persistence task still writes the fame it loaded (`build_character_update`
    // in `game-server/src/persist.rs` reads the row), so a change here is lost at the next login.
    fn set_fame(&mut self, player: ObjectId, fame: i32) {
        let max = self.multisell.settings.max_personal_fame_points;
        if let Some(p) = self.players.get_mut(&player) {
            p.data.profile.fame = fame.min(max).max(0);
        }
    }

    fn fame_of(&self, player: ObjectId) -> i32 {
        self.players.get(&player).map_or(0, |p| p.data.profile.fame)
    }

    /// Java `MultisellData.hasSpecialIngredient(id, amount, player)`.
    fn has_special_ingredient(&mut self, item_id: i32, amount: i64, player: ObjectId) -> bool {
        match item_id {
            PC_CAFE_POINTS => {
                if self.pc_cafe_points(player) < amount {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YouAreShortOfAccumulatedPoints),
                    );
                    return false;
                }
                true
            }
            CLAN_REPUTATION => {
                let Some(score) = self.clan_of(player).map(|c| c.reputation_score) else {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YouAreNotAClanMemberAndCannotPerformThisAction),
                    );
                    return false;
                };
                if !self.is_clan_leader(player) {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::OnlyTheClanLeaderIsEnabled),
                    );
                    return false;
                }
                if i64::from(score) < amount {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::TheClanReputationScoreIsTooLow),
                    );
                    return false;
                }
                true
            }
            FAME => {
                if i64::from(self.fame_of(player)) < amount {
                    self.send(
                        player,
                        &SystemMessage::new(SystemMessageId::YouDonTHaveEnoughReputationToDoThat),
                    );
                    return false;
                }
                true
            }
            _ => false,
        }
    }

    /// Java `MultisellData.takeSpecialIngredient(id, amount, player)`.
    fn take_special_ingredient(&mut self, item_id: i32, amount: i64, player: ObjectId) -> bool {
        match item_id {
            PC_CAFE_POINTS => {
                // FOLLOW-UP(pccafe): `setPcCafePoints(points - amount)` and `ExPCCafePointInfo`.
                true
            }
            CLAN_REPUTATION => {
                // `player.getClan().takeReputationScore((int) amount)`; the clan is known here
                // because `hasSpecialIngredient` passed.
                let Some(clan_id) = self.clan_of(player).map(|c| c.id) else {
                    return false;
                };
                self.take_reputation_score(clan_id, amount as i32);
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::S1PointsHaveBeenDeductedFromTheClanSReputationScore)
                        .with(SystemMessageParam::LongNumber(amount)),
                );
                true
            }
            FAME => {
                let fame = self.fame_of(player).wrapping_sub(amount as i32);
                self.set_fame(player, fame);
                self.send_user_info(player);
                true
            }
            _ => false,
        }
    }

    /// Java `MultisellData.giveSpecialProduct(id, amount, player)`.
    fn give_special_product(&mut self, item_id: i32, amount: i64, player: ObjectId) {
        match item_id {
            PC_CAFE_POINTS => {
                // FOLLOW-UP(pccafe): `setPcCafePoints(points + amount)` and `ExPCCafePointInfo`.
            }
            CLAN_REPUTATION => match self.clan_of(player).map(|c| c.id) {
                Some(clan_id) => self.add_reputation_score(clan_id, amount as i32),
                // NOTE(java-quirk): `player.getClan()` is null here and Java throws a
                // NullPointerException, which ends the packet with the ingredients already taken.
                None => warn!("Multisell: {player} has no clan to receive the reputation"),
            },
            FAME => {
                let fame = self.fame_of(player).wrapping_add(amount as i32);
                self.set_fame(player, fame);
                self.send_user_info(player);
            }
            _ => {}
        }
    }

    // ------------------------------------------------------------------ MultiSellChoose

    /// Whether the player may still use the list from where it stands (the NPC checks at the
    /// top of `MultiSellChoose.runImpl`).
    fn multisell_npc_in_reach(&self, player: ObjectId, list: &PreparedList) -> bool {
        let Some(npc) = self
            .players
            .get(&player)
            .and_then(|p| p.talk.last_folk_npc)
            .filter(|n| self.npcs.contains_key(n))
        else {
            return false;
        };
        let npc_id = self.npcs.get(&npc).map_or(0, |n| n.npc_id);
        let instance = |id: ObjectId| self.objects.get(&id).map_or(0, |e| e.instance_id);
        let (Some(me), Some(there)) = (self.position_of(player), self.position_of(npc)) else {
            return false;
        };
        list.is_npc_allowed(npc_id)
            && list.check_npc_object_id(npc.0)
            && instance(player) == instance(npc)
            && distance_3d(me, there) < f64::from(INTERACTION_DISTANCE)
    }

    /// Java `MultiSellChoose.runImpl`.
    pub(crate) fn on_multisell_choose(&mut self, id: ObjectId, list_id: i32, entry_id: i32, amount: i64) {
        if !self.players.contains_key(&id) {
            return;
        }
        // Ignore the flood click only, the open list must stay valid for the next exchange.
        if !self.flood_allows(id, FloodKind::MultiSell) {
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let settings = self.multisell.settings.clone();
        if amount < 1 || amount > settings.amount_limit {
            self.clear_multisell(id);
            return;
        }
        let Some(list) = player.multisell.as_ref().filter(|l| l.list_id == list_id) else {
            self.clear_multisell(id);
            return;
        };
        let maintain = list.maintain_enchantment;
        let npc = player.talk.last_folk_npc.filter(|n| self.npcs.contains_key(n));
        let mut npc_ok = true;
        if !list.is_npc_allowed(-1) {
            npc_ok = self.multisell_npc_in_reach(id, list);
        }
        let entry = list.entries.iter().find(|e| e.entry_id == entry_id).cloned();
        if !npc_ok {
            if self.is_gm(id) {
                self.send_message(
                    id,
                    &format!(
                        "Multisell {list_id} is restricted. Under current conditions cannot be used. Only GMs are allowed to use it."
                    ),
                );
            } else {
                self.clear_multisell(id);
                return;
            }
        }
        let Some(entry) = entry else {
            return;
        };

        if !entry.stackable && amount > 1 {
            warn!(
                "Character: {} is trying to set amount > 1 on non-stackable multisell, id:{list_id}:{entry_id}",
                self.multisell_player_name(id)
            );
            self.clear_multisell(id);
            return;
        }

        // The slots and the weight the products need.
        // NOTE(java-quirk): both are `int` variables that `+=` a `long`, so they wrap.
        let mut slots: i32 = 0;
        let mut weight: i32 = 0;
        for product in &entry.products {
            if product.item_id < 0 {
                continue;
            }
            let total = product.item_count.wrapping_mul(amount);
            if !product.is_stackable() {
                slots = i64::from(slots).wrapping_add(total) as i32;
            } else if self.inventory_item_by_item_id(id, product.item_id).is_none() {
                slots = slots.wrapping_add(1);
            }
            weight = i64::from(weight).wrapping_add(total.wrapping_mul(i64::from(product.weight()))) as i32;
        }
        if !self.validate_weight(id, i64::from(weight)) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveExceededTheWeightLimit),
            );
            return;
        }
        if !self.validate_capacity(id, i64::from(slots), false) {
            self.send(id, &SystemMessage::new(SystemMessageId::YourInventoryIsFull));
            return;
        }

        // A list of distinct ingredients and counts, to check the player has enough of each.
        let mut ingredients: Vec<Ingredient> = Vec::with_capacity(entry.ingredients.len());
        for e in &entry.ingredients {
            let mut new_ingredient = true;
            // At this point the template has already been modified so that enchantments are
            // properly included: uniqueness is judged by item id AND enchantment level.
            for i in (0..ingredients.len()).rev() {
                let existing = &ingredients[i];
                // If the item was already added, merely increment the count (one entry may have
                // the same ingredient twice, two swords for a dual).
                if existing.item_id == e.item_id && existing.enchant_level() == e.enchant_level() {
                    if existing.item_count.wrapping_add(e.item_count) > i64::from(i32::MAX) {
                        self.send(
                            id,
                            &SystemMessage::new(SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted),
                        );
                        return;
                    }
                    // Two same ingredients, merge into one and replace the old one.
                    // NOTE(java-quirk): `getCopy()` drops the item info, so the merged ingredient
                    // has the enchant level of the XML again.
                    let mut merged = existing.copy();
                    merged.item_count = existing.item_count + e.item_count;
                    ingredients[i] = merged;
                    new_ingredient = false;
                    break;
                }
            }
            if new_ingredient {
                ingredients.push(e.clone());
            }
        }

        // Check that the player has enough of everything.
        let templates = self.item_templates();
        for e in &ingredients {
            let total = e.item_count.wrapping_mul(amount);
            if total > i64::from(i32::MAX) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YouHaveExceededTheQuantityThatCanBeInputted),
                );
                return;
            }
            if e.item_id < 0 {
                if !self.has_special_ingredient(e.item_id, total, id) {
                    return;
                }
            } else {
                // A list that does not keep enchantments counts every item of the id, otherwise
                // only those with exactly the needed enchant level.
                let required = if settings.alt_blacksmith_use_recipes || !e.maintain_ingredient {
                    total
                } else {
                    e.item_count
                };
                let level = if maintain || e.enchant_level() > 0 {
                    e.enchant_level()
                } else {
                    -1
                };
                if self.multisell_item_count(id, e.item_id, level) < required {
                    self.send(
                        id,
                        &SystemMessage::new(SystemMessageId::S2UnitSOfTheItemS1IsAreRequired)
                            .with(item_name_param(templates.as_deref(), e.item_id))
                            .with(SystemMessageParam::LongNumber(required)),
                    );
                    return;
                }
            }
        }

        // All ok: remove the items.
        let mut augmentations: Vec<i32> = Vec::new();
        let mut elemental: Option<Vec<crate::effects::item_extras::ItemElemental>> = None;
        for e in &entry.ingredients {
            let total = e.item_count.wrapping_mul(amount);
            if e.item_id < 0 {
                if !self.take_special_ingredient(e.item_id, total, id) {
                    return;
                }
                continue;
            }
            // The first item of the id is the first guess for the item to take.
            let Some(item_to_take) = self.inventory_item_by_item_id(id, e.item_id).cloned() else {
                // This is a cheat, the transaction is aborted and the items already taken are not
                // returned.
                warn!(
                    "Character: {} is trying to cheat in multisell, id:{list_id}:{entry_id}",
                    self.multisell_player_name(id)
                );
                self.clear_multisell(id);
                return;
            };
            if !(settings.alt_blacksmith_use_recipes || !e.maintain_ingredient) {
                continue;
            }
            if self.data.item_stackable(item_to_take.item_id) {
                // A stackable item: reduce the amount of the first (only) instance.
                if !self.destroy_item_by_object_id(id, item_to_take.object_id, total, true) {
                    self.clear_multisell(id);
                    return;
                }
            } else if maintain || e.enchant_level() > 0 {
                // The list keeps enchantments: take the instances with exactly that level, one by
                // one, until the amount is taken.
                let candidates = self.multisell_unequipped(id, e.item_id, Some(e.enchant_level()));
                for (taken, current) in candidates.into_iter().enumerate() {
                    if taken as i64 >= total {
                        break;
                    }
                    if let Some(augmentation) = current.augmentation {
                        augmentations.push(augmentation);
                    }
                    if !current.elementals.is_empty() {
                        elemental = Some(current.elementals.clone());
                    }
                    if !self.destroy_item_by_object_id(id, current.object_id, 1, true) {
                        self.clear_multisell(id);
                        return;
                    }
                }
            } else {
                // The enchantment is not kept: take the instances with the LOWEST enchant level.
                for _ in 0..total {
                    let contents = self.multisell_unequipped(id, e.item_id, None);
                    // NOTE(java-quirk): `iterator().next()` on nothing throws, which ends the
                    // packet where it is (the list stays open).
                    let Some(mut lowest) = contents.first().cloned() else {
                        warn!("Multisell: {id} has no {} left to take", e.item_id);
                        return;
                    };
                    if lowest.enchant_level > 0 {
                        for item in &contents {
                            if item.enchant_level < lowest.enchant_level {
                                lowest = item.clone();
                                // Nothing has less than +0: take a +0 at once.
                                if lowest.enchant_level == 0 {
                                    break;
                                }
                            }
                        }
                    }
                    if !self.destroy_item_by_object_id(id, lowest.object_id, 1, true) {
                        self.clear_multisell(id);
                        return;
                    }
                }
            }
        }

        // Generate the products.
        for e in &entry.products {
            let total = e.item_count.wrapping_mul(amount);
            if e.item_id < 0 {
                self.give_special_product(e.item_id, total, id);
                continue;
            }
            if e.is_stackable() {
                self.inventory_add_item_by_id(id, e.item_id, total);
            } else {
                let keeps_enchant = maintain || e.enchant_level() > 0;
                for i in 0..total {
                    let Some(added) = self.inventory_add_item_by_id(id, e.item_id, 1) else {
                        continue;
                    };
                    if !keeps_enchant {
                        continue;
                    }
                    let object_id = added.item.object_id;
                    if let Some(item) = self
                        .players
                        .get_mut(&id)
                        .and_then(|p| p.data.items.iter_mut().find(|it| it.object_id == object_id))
                    {
                        if let Some(augmentation) = usize::try_from(i).ok().and_then(|i| augmentations.get(i))
                        {
                            item.augmentation = Some(*augmentation);
                        }
                        if let Some(elementals) = &elemental {
                            for element in elementals {
                                item.apply_attribute(element.element, element.value);
                            }
                        }
                        item.enchant_level = e.enchant_level();
                    }
                    self.update_item_in_db(id, object_id);
                    // The `InventoryUpdate` of the add was built before the enchant level was
                    // set; Java builds it when it is sent. The newer entry replaces the waiting one.
                    if let Some(item) = self.inventory_item(id, object_id).cloned() {
                        let entry = self.update_entry(&item, added.change);
                        self.send_inventory_update(id, entry);
                    }
                }
            }

            // The message.
            let name = item_name_param(templates.as_deref(), e.item_id);
            let message = if total > 1 {
                SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                    .with(name)
                    .with(SystemMessageParam::LongNumber(total))
            } else if maintain && e.enchant_level() > 0 {
                SystemMessage::new(SystemMessageId::AcquiredS1S2)
                    .with(SystemMessageParam::LongNumber(i64::from(e.enchant_level())))
                    .with(name)
            } else {
                SystemMessage::new(SystemMessageId::YouHaveEarnedS1).with(name)
            };
            self.send(id, &message);
        }

        self.send_item_list(id, false);
        self.send_cur_load(id);

        // Finally, give the tax to the castle (M7-RES-01; D-51: the treasury row only with
        // `SiegeDatabaseWrites`).
        if entry.tax_amount > 0
            && let Some(castle) = npc.and_then(|n| self.npc_castle(n))
        {
            self.castle_add_to_treasury(castle, entry.tax_amount.wrapping_mul(amount));
        }
    }

    fn multisell_player_name(&self, id: ObjectId) -> String {
        self.players
            .get(&id)
            .map_or_else(|| id.to_string(), |p| p.data.name.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_data::ItemTemplates;
    use l2_data::multisell::MultisellLoadOptions;

    const ITEMS: &str = r#"<list>
<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
<item id="1" type="Weapon" name="Sword"><set name="bodypart" val="rhand"/><set name="weight" val="50"/></item>
<item id="2" type="Weapon" name="Enchanted Sword"><set name="bodypart" val="rhand"/><set name="enchanted" val="4"/></item>
<item id="30" type="EtcItem" name="Stuff"><set name="is_stackable" val="true"/></item>
</list>"#;

    fn items() -> ItemTemplates {
        ItemTemplates::from_xml(ITEMS).unwrap()
    }

    fn template_entry(xml: &str) -> Entry {
        let data = MultisellData::from_xml(
            &[(1, xml)],
            &items(),
            &MultisellLoadOptions {
                correct_prices: false,
                ..Default::default()
            },
        );
        data.get(1).unwrap().entries[0].clone()
    }

    fn info(level: i32) -> ItemInfo {
        ItemInfo {
            enchant_level: level,
            augment_id: 7,
            element_id: 1,
            element_power: 30,
            elementals: [0; 6],
        }
    }

    #[test]
    fn the_adena_of_an_entry_is_merged_and_moved_last() {
        let template = template_entry(
            r#"<list><item>
              <ingredient count="100" id="57"/>
              <ingredient count="3" id="30"/>
              <ingredient count="50" id="57"/>
              <production count="1" id="30"/>
            </item></list>"#,
        );
        let entry = prepared_entry(&template, None, false, false, 0.0);
        assert_eq!(entry.entry_id, 100_000);
        let ids: Vec<(i32, i64)> = entry
            .ingredients
            .iter()
            .map(|i| (i.item_id, i.item_count))
            .collect();
        assert_eq!(ids, [(30, 3), (57, 150)]);
        assert!(entry.stackable);
        assert_eq!(entry.tax_amount, 0);
    }

    #[test]
    fn a_tax_ingredient_counts_only_with_taxes_and_rounds_half_up() {
        let template = template_entry(
            r#"<list><item>
              <ingredient count="1000" id="57"/>
              <ingredient count="25" id="57" isTaxIngredient="true"/>
              <production count="1" id="30"/>
            </item></list>"#,
        );
        let without = prepared_entry(&template, None, false, false, 0.15);
        assert_eq!(without.ingredients[0].item_count, 1000);
        assert_eq!(without.tax_amount, 0);
        // 25 * 0.15 = 3.75 -> 4.
        let with = prepared_entry(&template, None, true, false, 0.15);
        assert_eq!(with.ingredients[0].item_count, 1004);
        assert_eq!(with.tax_amount, 4);
        // 25 * 0.1 = 2.5 -> 3 (`Math.round` goes up at .5).
        assert_eq!(prepared_entry(&template, None, true, false, 0.1).tax_amount, 3);
        // Only tax adena: the sum is still added.
        let only_tax = template_entry(
            r#"<list><item><ingredient count="40" id="57" isTaxIngredient="true"/><production count="1" id="30"/></item></list>"#,
        );
        let entry = prepared_entry(&only_tax, None, true, false, 0.5);
        assert_eq!(entry.ingredients.len(), 1);
        assert_eq!(entry.ingredients[0].item_count, 20);
        assert!(
            !prepared_entry(&only_tax, None, false, false, 0.5)
                .ingredients
                .iter()
                .any(|i| i.item_id == ADENA_ID)
        );
    }

    #[test]
    fn a_list_that_keeps_enchantments_copies_the_item_info_to_weapons() {
        let template = template_entry(
            r#"<list><item>
              <ingredient count="1" id="1"/>
              <ingredient count="10" id="57"/>
              <production count="1" id="1"/>
              <production count="5" id="30"/>
            </item></list>"#,
        );
        let item = info(5);
        let entry = prepared_entry(&template, Some(&item), false, true, 0.0);
        // The entry id carries the enchant level.
        assert_eq!(entry.entry_id, 100_005);
        assert_eq!(entry.ingredients[0].item_info.as_ref().unwrap().enchant_level, 5);
        assert_eq!(entry.ingredients[0].enchant_level(), 5);
        // The product weapon gets the same info, the stackable one none.
        assert_eq!(entry.products[0].item_info, Some(item));
        assert!(entry.products[1].item_info.is_none());
        // Without `maintainEnchantment` nothing changes, whatever the item.
        let plain = prepared_entry(&template, Some(&info(5)), false, false, 0.0);
        assert_eq!(plain.entry_id, 100_000);
        assert!(plain.ingredients[0].item_info.is_none());
        assert!(plain.products[0].item_info.is_none());
    }

    #[test]
    fn a_product_with_a_default_enchant_shows_it() {
        let template = template_entry(
            r#"<list><item><ingredient count="10" id="57"/><production count="1" id="2"/></item></list>"#,
        );
        let entry = prepared_entry(&template, None, false, false, 0.0);
        let shown = entry.products[0].item_info.as_ref().unwrap();
        assert_eq!(shown.enchant_level, 4);
        // `new ItemInfo(int)`: element NONE, no augmentation.
        assert_eq!((shown.element_id, shown.augment_id), (-1, 0));
    }

    #[test]
    fn a_non_stackable_product_makes_the_prepared_entry_non_stackable() {
        let template = template_entry(
            r#"<list><item><ingredient count="10" id="57"/><production count="1" id="1"/></item></list>"#,
        );
        assert!(!prepared_entry(&template, None, false, false, 0.0).stackable);
    }

    #[test]
    fn multipliers_never_drop_a_count_to_zero() {
        assert_eq!(scaled(10, 0.5), 5);
        assert_eq!(scaled(1, 0.1), 1);
        assert_eq!(scaled(7, 1.5), 10);
    }

    #[test]
    fn the_packet_entry_follows_the_templates_and_infos() {
        let template = template_entry(
            r#"<list><item>
              <ingredient count="1" id="1" enchantmentLevel="2"/>
              <ingredient count="5" id="-300"/>
              <production count="1" id="2"/>
            </item></list>"#,
        );
        let entry = prepared_entry(&template, None, false, false, 0.0);
        let packet = entry_packet(&entry);
        assert_eq!(packet.entry_id, 100_000);
        assert!(!packet.stackable);
        // A weapon ingredient without an info shows its XML enchant level.
        assert_eq!(
            (
                packet.ingredients[0].item_id,
                packet.ingredients[0].detail.enchant
            ),
            (1, 2)
        );
        // The fame id has no template: the raw id and type2 65535.
        assert_eq!(
            (packet.ingredients[1].item_id, packet.ingredients[1].type2),
            (-300, 65535)
        );
        // The enchanted product shows its default level and `ItemInfo.NONE` element.
        assert_eq!(packet.products[0].detail.enchant, 4);
        assert_eq!(packet.products[0].detail.attack_element, -1);
        assert_eq!(packet.products[0].body_part, 0x80);
    }
}
