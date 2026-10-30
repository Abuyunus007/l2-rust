//! What `LongTimeEvent` and the seasonal event scripts need from the world (S-EVENT-01).
// Java: mechanics/script/LongTimeEvent.java, entity/World.java (broadcastToAllOnlinePlayers,
// Java: getPlayers), mechanics/announce/EventAnnouncement.java

use l2_util::ObjectId;

use crate::script::ctx::ScriptCtx;
use crate::warehouse::ContainerRef;
use crate::world::World;

impl World {
    /// Java `NpcData.getInstance().getTemplate(npcId) != null`.
    pub fn npc_template_exists(&self, npc_id: i32) -> bool {
        self.npc_world.template(npc_id).is_some()
    }

    /// Java `World.broadcastToAllOnlinePlayers(text)`: a non-critical announcement to everyone.
    pub fn broadcast_announcement_text(&self, text: &str) {
        self.broadcast_announcement(text, false);
    }

    /// Java `creature.getEffectList().getBuffInfoBySkillId(skillId).getSkill().getLevel()`: the
    /// level of the skill of the effect, `None` while the creature is not affected by it.
    pub fn effect_skill_level(&self, creature: ObjectId, skill_id: i32) -> Option<i32> {
        self.effect_list(creature)?
            .info_by_skill_id(skill_id)
            .map(|info| info.skill.level)
    }

    /// Java `World.getPlayers()`: the object ids of the online players, in ascending order.
    pub fn online_player_ids(&self) -> Vec<ObjectId> {
        let mut ids: Vec<ObjectId> = self.players.keys().copied().collect();
        ids.sort_unstable_by_key(|id| id.0);
        ids
    }
}

impl ScriptCtx<'_> {
    /// Java `World.broadcastToAllOnlinePlayers(text)`.
    pub fn broadcast_to_all_online_players(&self, text: &str) {
        self.world().broadcast_announcement_text(text);
    }

    /// Java `World.getPlayers()`.
    pub fn online_players(&self) -> Vec<ObjectId> {
        self.world().online_player_ids()
    }

    /// Java `Quest.ownsAtLeastOneItem(player, itemIds)`: the inventory and the private
    /// warehouse. Left out: the summon's inventory and the mail attachments (inbox, outbox).
    pub fn owns_at_least_one_item(&self, player: ObjectId, item_ids: &[i32]) -> bool {
        // Inventory.
        if item_ids.iter().any(|&id| self.has_quest_items(player, id)) {
            return true;
        }
        // Warehouse.
        self.world()
            .container_items(ContainerRef::Private(player))
            .is_some_and(|items| items.iter().any(|i| item_ids.contains(&i.item_id)))
    }
}
