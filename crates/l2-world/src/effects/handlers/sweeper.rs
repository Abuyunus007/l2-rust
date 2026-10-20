//! The `Sweeper` effect: the spoiler takes the SPOIL items of a dead monster.
// Java: dist/game/data/scripts/handlers/skill/effects/Sweeper.java
// Java: entity/actor/Player.java (addItem with `ItemProcessType.SWEEP`)

use l2_data::skill::EffectTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;

use crate::effects::registry::{EffectCx, EffectHandler};
use crate::inventory::item_name_param;
use crate::rewards::drop::ItemHolder;
use crate::world::World;

/// Java `Sweeper`. It reads no parameters.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct Sweeper;

impl Sweeper {
    /// Java `Sweeper(attachCond, applyCond, set, params)`.
    pub(crate) fn create(_template: &EffectTemplate) -> Result<Box<dyn EffectHandler>, String> {
        Ok(Box::new(Sweeper))
    }
}

/// Java `Player.addItem(ItemProcessType.SWEEP, itemId, count, reference, true)`: the same as
/// [`World::add_item`], but the message is "You have earned" (`SWEEP` and `QUEST` say so, every
/// other process says "You have obtained").
fn add_sweep_item(world: &mut World, player: ObjectId, item: ItemHolder) {
    if item.count <= 0 {
        return;
    }
    let Some(templates) = world.item_templates() else {
        return;
    };
    let Some(template) = templates.get(item.id) else {
        tracing::error!("Item doesn't exist so cannot be added. Item ID: {}", item.id);
        return;
    };
    // Sends message to client: `(!isCastingNow() && hasExImmediateEffect()) || !hasExImmediateEffect()`.
    if !template.ex_immediate_effect || !world.is_casting_now(player) {
        let name = item_name_param(Some(&templates), item.id);
        let message = if item.count > 1 {
            SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                .with(name)
                .with(SystemMessageParam::LongNumber(item.count))
        } else {
            SystemMessage::new(SystemMessageId::YouHaveEarnedS1).with(name)
        };
        world.send(player, &message);
    }
    world.add_item(player, item.id, item.count, false);
}

impl EffectHandler for Sweeper {
    fn is_instant(&self) -> bool {
        true
    }

    fn on_start(&self, world: &mut World, cx: &EffectCx<'_>) {
        if !world.is_player(cx.effector) || !world.is_attackable_npc(cx.effected) {
            return;
        }
        if !world.check_spoil_owner(cx.effected, cx.effector) {
            return;
        }
        let loot = world.spoil_loot_items(cx.effected);
        if !world.check_inventory_slots_and_weight(cx.effector, &loot) {
            return;
        }
        if let Some(items) = world.take_sweep(cx.effected) {
            for item in items {
                // `player.isInParty()` -> `distributeItem(player, item, true, monster)`: parties
                // are M6, so the sweeper always keeps what it sweeps.
                add_sweep_item(world, cx.effector, item);
            }
        }
    }
}
