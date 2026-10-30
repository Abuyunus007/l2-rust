//! `ScriptCtx` helpers of the quest and AI scripts that ask a player about its clan links, send it
//! a social action, or touch its private warehouse (S-DATA-01): small `pub` doors named after
//! the Java calls they stand for.
// Java: entity/actor/Player.java (getSponsor, getApprentice, getPledgeType, getWarehouse),
//       network/serverpackets/SocialAction.java,
//       entity/itemcontainer/ItemContainer.java (destroyItem, getItemByObjectId)

use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;

use super::ctx::ScriptCtx;
use crate::outbox::ItemRowOp;
use crate::warehouse::ContainerRef;
use crate::world::World;

impl World {
    /// Java `player.getWarehouse().getItemByObjectId(objectId)` of the private warehouse: the
    /// template id and the count, `None` when the player is gone or the item is not in it.
    pub fn private_warehouse_item(&self, player: ObjectId, object_id: ObjectId) -> Option<(i32, i64)> {
        self.container_item(ContainerRef::Private(player), object_id)
            .map(|item| (item.item_id, item.count))
    }

    /// Java `player.getWarehouse().destroyItem(DESTROY, item, count, player, null)` for an item of
    /// the private warehouse: a smaller count only lowers the stack, the whole stack deletes the
    /// row and releases the object id. `false` where Java returns `null` (the item is not there,
    /// or it holds fewer than `count`).
    pub fn destroy_private_warehouse_item(
        &mut self,
        player: ObjectId,
        object_id: ObjectId,
        count: i64,
    ) -> bool {
        let container = ContainerRef::Private(player);
        let Some(item) = self.container_item(container, object_id).cloned() else {
            return false;
        };
        let op = if item.count > count {
            // `item.changeCount(process, -count, ...)`.
            let Some(stack) = self
                .container_items_mut(container)
                .and_then(|items| items.iter_mut().find(|i| i.object_id == object_id))
            else {
                return false;
            };
            stack.count -= count;
            ItemRowOp::Update {
                owner_id: container.owner_id(),
                item: stack.clone(),
            }
        } else if item.count < count {
            return false;
        } else {
            // `removeItem`, `ItemManager.destroyItem` (the id is released) and `updateDatabase`.
            let Some(items) = self.container_items_mut(container) else {
                return false;
            };
            let Some(index) = items.iter().position(|i| i.object_id == object_id) else {
                return false;
            };
            items.remove(index);
            self.release_item_id(object_id);
            ItemRowOp::Delete { object_id }
        };
        self.flush_item_rows(player, vec![op]);
        true
    }
}

impl ScriptCtx<'_> {
    /// Java `player.getSponsor()`: the object id of the academy member's sponsor, 0 for none.
    pub fn player_sponsor(&self, player: ObjectId) -> i32 {
        self.world().player_clan_state(player).sponsor
    }

    /// Java `player.getApprentice()`: the object id of the sponsor's apprentice, 0 for none.
    pub fn player_apprentice(&self, player: ObjectId) -> i32 {
        self.world().player_clan_state(player).apprentice
    }

    /// Java `player.getPledgeType()`: 0 for the main pledge and for a player without a clan,
    /// -1 for the academy.
    pub fn player_pledge_type(&self, player: ObjectId) -> i32 {
        self.world().player_clan_state(player).pledge_type
    }

    /// Java `player.sendPacket(new SocialAction(player.getObjectId(), actionId))`: the animation
    /// goes to this player only (`broadcastSocialAction` is the one that goes to everybody).
    pub fn send_social_action(&mut self, player: ObjectId, action_id: i32) {
        self.world().send(
            player,
            &SocialAction {
                object_id: player.0,
                action_id,
            },
        );
    }

    /// Java `player.setNoble(value)`: the noble skill tree is given or taken, the flag is set and
    /// the skill list sent.
    pub fn set_noble(&mut self, player: ObjectId, value: bool) {
        self.world_mut().set_noble(player, value);
    }

    /// Java `player.getWarehouse().getItemByObjectId(objectId)`: `(item id, count)`.
    pub fn private_warehouse_item(&self, player: ObjectId, object_id: ObjectId) -> Option<(i32, i64)> {
        self.world().private_warehouse_item(player, object_id)
    }

    /// Java `player.getWarehouse().destroyItem(DESTROY, item, count, player, false)`.
    pub fn destroy_private_warehouse_item(
        &mut self,
        player: ObjectId,
        object_id: ObjectId,
        count: i64,
    ) -> bool {
        self.world_mut()
            .destroy_private_warehouse_item(player, object_id, count)
    }
}
