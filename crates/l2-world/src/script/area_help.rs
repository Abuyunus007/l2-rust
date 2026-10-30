//! Small world reads and writes the area AI scripts of S-AI-03 need (Beast Farm, Pagan Temple,
//! Dragon Valley, Monastery of Silence, Primeval Isle, ...).
// Java: config/PlayerConfig.java (AUTO_LOOT), entity/actor/Npc.java (broadcastSocialAction),
//       entity/actor/Creature.java (getTemplate().getParameters())

use l2_data::npc::NpcParam;
use l2_protocol_game::server::{SocialAction, ValidateLocation};
use l2_util::ObjectId;

use super::ScriptNpc;
use super::ctx::ScriptCtx;
use crate::effects::SkillKey;

impl ScriptCtx<'_> {
    /// Java `PlayerConfig.AUTO_LOOT` (`AutoLoot` of `Player.ini`).
    pub fn auto_loot(&self) -> bool {
        self.world().settings.ground.auto_loot
    }

    /// Java `npc.broadcastSocialAction(id)`: the animation goes to every player who sees the NPC.
    pub fn broadcast_npc_social_action(&mut self, npc: ScriptNpc, action_id: i32) {
        self.world().broadcast_packet(
            npc.object_id,
            &SocialAction {
                object_id: npc.object_id.0,
                action_id,
            },
        );
    }

    /// Java `npc.setHeading(heading); npc.broadcastPacket(new ValidateLocation(npc))`.
    pub fn set_heading_and_validate(&mut self, npc: ScriptNpc, heading: i32) {
        self.world_mut().set_heading(npc.object_id, heading);
        if let Some((x, y, z)) = self.world().position_of(npc.object_id) {
            self.world().broadcast_packet(
                npc.object_id,
                &ValidateLocation {
                    object_id: npc.object_id.0,
                    x,
                    y,
                    z,
                    heading,
                },
            );
        }
    }

    /// Java `npc.asAttackable().returnHome()`.
    pub fn return_home(&mut self, npc: ScriptNpc) {
        self.world_mut().return_home(npc.object_id);
    }

    /// Java `npc.getTemplate().getParameters().getObject(name, SkillHolder.class)`: a
    /// `<skill name=".." id=".." level=".."/>` parameter of the template.
    pub fn npc_template_skill(&self, npc: ScriptNpc, name: &str) -> Option<SkillKey> {
        match self.world().npc_template(npc.object_id)?.parameters.get(name)? {
            NpcParam::Skill { id, level } => Some(SkillKey::new(*id, *level)),
            _ => None,
        }
    }

    /// Java `npc.getTemplate().getParameters().getInt(name, default)`.
    pub fn npc_template_int(&self, npc: ScriptNpc, name: &str, default: i32) -> i32 {
        self.world()
            .npc_template_param_int(npc.object_id, name)
            .unwrap_or(default)
    }

    /// Java `player.isInventoryUnder80(includeQuestInv)`: the inventory holds at most 80% of
    /// its limit.
    pub fn is_inventory_under_80(&self, player: ObjectId, include_quest: bool) -> bool {
        let Some(view) = self.world().stats(player) else {
            return false;
        };
        let size = if include_quest {
            view.non_quest_size() + self.world().quest_size(player)
        } else {
            view.non_quest_size()
        };
        f64::from(size) <= f64::from(view.inventory_limit()) * 0.8
    }

    /// Java `ItemHandler.getInstance().getHandler(item.getEtcItem()).onItemUse(player, item, true)`
    /// for the first inventory item `item_id` (a summon item): does nothing without the item.
    pub fn use_summon_item_forced(&mut self, player: ObjectId, item_id: i32) {
        let Some(item) = self.world().inventory_item_by_item_id(player, item_id).cloned() else {
            return;
        };
        let Some(templates) = self.world().item_templates() else {
            return;
        };
        let Some(template) = templates.get(item_id) else {
            return;
        };
        self.world_mut()
            .summon_items_on_use(player, &item, template, true);
    }

    /// Java `npc.isInMySpawnGroup(other)`: both NPCs have a spawn and the spawns have the same
    /// (non-null) name.
    pub fn is_in_my_spawn_group(&self, npc: ScriptNpc, other: ScriptNpc) -> bool {
        let name_of = |id: ObjectId| {
            let index = self.world().npc_spawn_index(id)?;
            self.world().spawn_def_name(index)
        };
        match (name_of(npc.object_id), name_of(other.object_id)) {
            (Some(a), Some(b)) => a == b,
            _ => false,
        }
    }

    /// Java `SpawnTable.getInstance().getSpawns(npcId)` read as `(spawn.getLastSpawn(), name
    /// starts with prefix, x, y, z, heading)`, for the spawns whose name starts with `prefix`.
    pub fn spawns_named(&self, npc_id: i32, prefix: &str) -> Vec<super::area_spawns::NamedSpawn> {
        self.world().named_spawns_of(npc_id, prefix)
    }

    /// Java `npc.getAI().getIntention() == Intention.IDLE`.
    pub fn is_intention_idle(&self, npc: ScriptNpc) -> bool {
        self.world().intention_of(npc.object_id) == crate::movement::Intention::Idle
    }

    /// Java `npc.isAttackable()`.
    pub fn is_attackable(&self, npc: ScriptNpc) -> bool {
        self.world().is_attackable_npc(npc.object_id)
    }

    /// Java `player.hasPet()`.
    pub fn has_pet(&self, owner: ObjectId) -> bool {
        self.world().has_pet(owner)
    }

    /// Java `player.sendMessage(text)`: a system message with a plain text.
    pub fn send_message(&mut self, player: ObjectId, text: &str) {
        self.world().send_message(player, text);
    }
}
