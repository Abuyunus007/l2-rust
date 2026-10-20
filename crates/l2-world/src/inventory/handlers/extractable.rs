//! `ExtractableItems`: opening a box or pack (`<capsuled_items>` of the item).
// Java: dist/game/data/scripts/handlers/items/ExtractableItems.java
// Java: entity/item/holders/ExtractableProduct.java
//
// # How the roll works
// Every product has a chance in thousandths of a percent (`(int) (chance * 1000)`), compared with
// `Rnd.get(100000)`. The box gives the products that hit, at most `extractableCountMax` different
// items; with `extractableCountMin` above 0 the list is walked again until that many different
// items came out. The amounts are multiplied by `RateExtractable`.

use l2_data::item::{ExtractableProduct, ItemTemplate};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::{info, warn};

use crate::inventory::item_name_param;
use crate::player::ItemData;
use crate::world::World;

/// A guard Java does not have: with `extractableCountMin` above what the products can give (or
/// above `extractableCountMax`) Java's `while` never ends. No shipped box does that.
const MAX_ROUNDS: usize = 10_000;

/// What the roll made so far: each distinct item once, with the count it received.
#[derive(Default)]
struct Extracted {
    /// Java `extractedItems` (a `HashMap<Item, Long>`; here in the order the items came).
    items: Vec<(ObjectId, i64)>,
    /// Java `enchantedItems`.
    enchanted: Vec<ObjectId>,
}

impl Extracted {
    /// Java `addItem(extractedItems, newItem, count)`: `merge(newItem, count, Long::sum)`.
    fn add(&mut self, item: ObjectId, count: i64) {
        match self.items.iter_mut().find(|(o, _)| *o == item) {
            Some((_, total)) => *total = total.wrapping_add(count),
            None => self.items.push((item, count)),
        }
    }
}

impl World {
    /// Java `ExtractableItems.onItemUse`.
    pub(crate) fn extractable_items_on_use(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
    ) -> bool {
        // Pets cannot open boxes: `Playable` is always a player here.
        let Some(etc) = template.etc() else {
            return false;
        };
        let products = &etc.extractable_items;
        if products.is_empty() {
            info!("No extractable data defined for {}", template.label());
            return false;
        }

        // `isInventoryUnder80(false)`.
        let under_80 = self
            .stats(id)
            .is_some_and(|v| f64::from(v.non_quest_size()) <= f64::from(v.inventory_limit()) * 0.8);
        if !under_80 {
            self.send_message(
                id,
                "You've exceeded the limit and cannot retrieve the item. Please check your limit in the inventory.",
            );
            return false;
        }

        // destroy item
        if !self.destroy_item_by_object_id(id, item.object_id, 1, true) {
            return false;
        }

        let mut extracted = Extracted::default();
        let count_min = etc.extractable_count_min;
        let count_max = etc.extractable_count_max;
        if count_min > 0 {
            let mut rounds = 0;
            while extracted.items.len() < usize::try_from(count_min).unwrap_or(0) {
                rounds += 1;
                if rounds > MAX_ROUNDS {
                    warn!("{} cannot give {count_min} different items.", template.label());
                    break;
                }
                for product in products {
                    if count_max > 0 && extracted.items.len() == usize::try_from(count_max).unwrap_or(0) {
                        break;
                    }
                    self.extract_product(id, product, &mut extracted, Some((products.len(), count_max)));
                }
            }
        } else {
            for product in products {
                if count_max > 0 && extracted.items.len() == usize::try_from(count_max).unwrap_or(0) {
                    break;
                }
                self.extract_product(id, product, &mut extracted, None);
            }
        }

        if extracted.items.is_empty() {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::ThereWasNothingFoundInside),
            );
        }

        if !extracted.enchanted.is_empty() {
            for object_id in &extracted.enchanted {
                if let Some(entry) = self.modified_entry(id, *object_id) {
                    self.send_inventory_update(id, entry);
                }
            }
        }

        let templates = self.item_templates();
        for (object_id, count) in &extracted.items {
            let Some(created) = self.inventory_item(id, *object_id).cloned() else {
                continue;
            };
            let name = item_name_param(templates.as_deref(), created.item_id);
            let message = if *count > 1 {
                SystemMessage::new(SystemMessageId::YouHaveObtainedS2S1)
                    .with(name)
                    .with(SystemMessageParam::LongNumber(*count))
            } else if created.enchant_level > 0 {
                SystemMessage::new(SystemMessageId::YouHaveObtainedAS1S2)
                    .with(SystemMessageParam::IntNumber(created.enchant_level))
                    .with(name)
            } else {
                SystemMessage::new(SystemMessageId::YouHaveObtainedS1).with(name)
            };
            self.send(id, &message);
        }

        true
    }

    /// One product of the roll: the chance, the amount, and the items. `duplicate_check` is the
    /// `extractableCountMin > 0` loop's `(products, extractableCountMax)`.
    fn extract_product(
        &mut self,
        id: ObjectId,
        product: &ExtractableProduct,
        extracted: &mut Extracted,
        duplicate_check: Option<(usize, i32)>,
    ) {
        if self.rng().below(100_000) > product.chance {
            return;
        }
        let rate = self.settings.inventory.rate_extractable;
        // `(long) (expi.getMin() * RatesConfig.RATE_EXTRACTABLE)`: a `long * float` is a float.
        let min = (product.min as f32 * rate) as i64;
        let max = (product.max as f32 * rate) as i64;
        let mut amount = if max == min {
            min
        } else {
            self.rng()
                .below_i64(max.wrapping_sub(min).wrapping_add(1))
                .wrapping_add(min)
        };
        if amount == 0 {
            return;
        }

        let templates = self.item_templates();
        if let Some((product_count, count_max)) = duplicate_check {
            // Do not extract the same item.
            let already_extracted = extracted.items.iter().any(|(object_id, _)| {
                self.inventory_item(id, *object_id)
                    .is_some_and(|created| created.item_id == product.id)
            });
            // NOTE(java-quirk): with `extractableCountMax` 0 this is always true, so the second
            // product of the same item is always refused.
            if already_extracted && i64::try_from(product_count).unwrap_or(i64::MAX) >= i64::from(count_max) {
                return;
            }
        }
        let Some(created_template) = templates.as_deref().and_then(|t| t.get(product.id)) else {
            warn!(
                "ExtractableItems: Could not find product template with id {}!",
                product.id
            );
            return;
        };

        if created_template.stackable || amount == 1 {
            self.create_extracted(id, product, amount, extracted);
        } else {
            while amount > 0 {
                self.create_extracted(id, product, 1, extracted);
                amount -= 1;
            }
        }
    }

    /// `player.addItem(REWARD, id, count, player, false)` and the enchant level of the product.
    fn create_extracted(
        &mut self,
        id: ObjectId,
        product: &ExtractableProduct,
        count: i64,
        extracted: &mut Extracted,
    ) {
        // A herb never enters the inventory (it casts its skills): Java would fail on the null.
        let Some(created) = self.add_item(id, product.id, count, false) else {
            return;
        };
        if product.max_enchant > 0 {
            let level = self.rng().get(product.min_enchant, product.max_enchant);
            if let Some(item) = self
                .players
                .get_mut(&id)
                .and_then(|p| p.data.items.iter_mut().find(|i| i.object_id == created))
            {
                item.enchant_level = level;
            }
            extracted.enchanted.push(created);
        }
        extracted.add(created, count);
    }
}
