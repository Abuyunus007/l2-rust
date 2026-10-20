//! The `RestorationRandom` effect: the player gets one of several item lists, picked by chance
//! (item skills that "extract" items). The amounts follow `RateExtractable`.
// Java: dist/game/data/scripts/handlers/skill/effects/RestorationRandom.java

use l2_data::skill::{EffectTemplate, ItemGroup, ParamValue};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use super::restoration::set_enchant_level;
use crate::effects::registry::{EffectCx, EffectHandler};
use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL, item_name_param};
use crate::world::World;

/// Java `RestorationRandom`: its `ExtractableProductItem`s.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RestorationRandom {
    products: Vec<ItemGroup>,
}

impl RestorationRandom {
    /// Java `RestorationRandom(attachCond, applyCond, set, params)`: the `items` list.
    pub(crate) fn create(template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        // `params.getList("items", StatSet.class)`.
        let products = match template.params.get("items") {
            Some(ParamValue::Items(groups)) => groups.clone(),
            _ => return Err("RestorationRandom: no items list".to_owned()),
        };
        Ok(Box::new(Self { products }))
    }
}

impl EffectHandler for RestorationRandom {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        let rnd_num = 100.0 * world.rng().next_f64();
        let mut chance;
        let mut chance_from = 0.0_f64;
        let mut creation_list = Vec::new();

        // Explanation for future changes:
        // You get one chance for the current skill, then you can fall into
        // one of the "areas" like in a roulette.
        // Example: for an item like Id1,A1,30;Id2,A2,50;Id3,A3,20;
        // #---#-----#--#
        // 0--30----80-100
        // If you get chance equal 45% you fall into the second zone 30-80.
        // Meaning you get the second production list.
        // Calculate extraction
        for product in &self.products {
            chance = f64::from(product.chance);
            if rnd_num >= chance_from && rnd_num <= (chance + chance_from) {
                creation_list.extend(product.items.iter().cloned());
                break;
            }
            chance_from += chance;
        }

        // `effected.asPlayer()` is null for an NPC and Java then fails: nothing happens.
        let player = cx.effected;
        if !world.is_player(player) {
            return;
        }
        if creation_list.is_empty() {
            world.send(
                player,
                &SystemMessage::new(SystemMessageId::ThereWasNothingFoundInside),
            );
            return;
        }

        let rate = world.settings.inventory.rate_extractable;
        // Java: a `HashMap<Item, Long>` (the same item merges); the order here is the order the
        // items were given.
        let mut extracted: Vec<(ObjectId, i64)> = Vec::new();
        for created in creation_list {
            if created.id <= 0 || created.count <= 0 {
                continue;
            }
            // `(long) (createdItem.getCount() * RatesConfig.RATE_EXTRACTABLE)`: a long times a
            // float is a float.
            let item_count = ((created.count as f32) * rate) as i64;
            // `player.addItem(REWARD, id, count, effector, false)`.
            let Some(item) = world.add_item(player, created.id, item_count, false) else {
                continue;
            };
            if created.max_enchant > 0 {
                let level = world.rng().get(created.min_enchant, created.max_enchant);
                set_enchant_level(world, player, item, level);
            }
            match extracted.iter_mut().find(|(who, _)| *who == item) {
                Some((_, total)) => *total = total.wrapping_add(item_count),
                None => extracted.push((item, item_count)),
            }
        }

        for (item, count) in extracted {
            let Some(data) = world.inventory_item(player, item) else {
                continue;
            };
            let (item_id, enchant_level) = (data.item_id, data.enchant_level);
            let stackable = world.data.item_stackable(item_id);
            // The inventory update: the stack, or every item of that id
            // (`getAllItemsByItemId`).
            let entries: Vec<ObjectId> = if stackable {
                vec![item]
            } else {
                world.players.get(&player).map_or_else(Vec::new, |p| {
                    p.data
                        .items
                        .iter()
                        .filter(|i| {
                            i.item_id == item_id && (i.loc == LOC_INVENTORY || i.loc == LOC_PAPERDOLL)
                        })
                        .map(|i| i.object_id)
                        .collect()
                })
            };
            send_message(world, player, item_id, enchant_level, count);
            for object_id in entries {
                if let Some(entry) = world.modified_entry(player, object_id) {
                    world.send_inventory_update(player, entry);
                }
            }
        }
    }
}

/// Java `sendMessage(player, item, count)`.
fn send_message(world: &World, player: ObjectId, item_id: i32, enchant_level: i32, count: i64) {
    let templates = world.item_templates();
    let name = item_name_param(templates.as_deref(), item_id);
    let message = if count > 1 {
        SystemMessage::new(SystemMessageId::YouHaveObtainedS2S1)
            .with(name)
            .with(SystemMessageParam::LongNumber(count))
    } else if enchant_level > 0 {
        SystemMessage::new(SystemMessageId::YouHaveObtainedAS1S2)
            .with(SystemMessageParam::IntNumber(enchant_level))
            .with(name)
    } else {
        SystemMessage::new(SystemMessageId::YouHaveObtainedS1).with(name)
    };
    world.send(player, &message);
}
