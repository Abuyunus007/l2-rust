//! The `EventItem` item handler: Handy's Block Checker Bond and Land Mine.
// Java: dist/game/data/scripts/handlers/items/EventItem.java

use l2_data::item::ItemTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use crate::effects::SkillKey;
use crate::player::ItemData;
use crate::world::World;

/// Handy's Block Checker Bond.
const BOND: i32 = 13787;
/// Handy's Block Checker Land Mine.
const LAND_MINE: i32 = 13788;

impl World {
    /// Java `EventItem.onItemUse`.
    pub(crate) fn event_item_on_use(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
    ) -> bool {
        if !self.players.contains_key(&id) {
            if let Some(owner) = self.acting_player(id) {
                self.send(
                    owner,
                    &SystemMessage::new(SystemMessageId::YourPetCannotCarryThisItem),
                );
            }
            return false;
        }
        match item.item_id {
            BOND | LAND_MINE => self.use_block_checker_item(id, item, template),
            other => {
                warn!("EventItemHandler: Item with id: {other} is not handled");
                false
            }
        }
    }

    /// Java `EventItem.useBlockCheckerItem`.
    fn use_block_checker_item(&mut self, caster: ObjectId, item: &ItemData, template: &ItemTemplate) -> bool {
        let arena = self.block_checker_arena_of(caster);
        if arena == -1 {
            self.send(
                caster,
                &SystemMessage::new(SystemMessageId::S1CannotBeUsedDueToUnsuitableTerms)
                    .with(SystemMessageParam::ItemName(item.item_id)),
            );
            return false;
        }

        let Some(skill_ref) = template.skills.item_skills.as_ref().and_then(|s| s.first()) else {
            warn!("EventItem: item {} has no skill", item.item_id);
            return false;
        };
        let Some(table) = self.skill_table() else {
            return false;
        };
        let Some(skill) = table.get(skill_ref.id, skill_ref.level) else {
            return false;
        };
        let key = SkillKey::of(skill);
        let effect_range = skill.effect_range;

        if !self.destroy_item_by_object_id(caster, item.object_id, 1, true) {
            return false;
        }

        // NOTE(java-quirk): `(Block) caster.getTarget()` throws when the target is not a block;
        // the item is gone by then.
        let Some(block) = self.target_of(caster).filter(|t| self.is_block_checker_block(*t)) else {
            warn!("EventItem: {caster} used a block checker item without a block as target");
            return false;
        };
        let Some(team) = self.block_checker_team_of(arena, caster) else {
            warn!("Char: {caster}[{}] has unknown block checker arena", caster.0);
            return false;
        };
        for pc in self.visible_objects_in_range(block, effect_range) {
            if !self.players.contains_key(&pc) {
                continue;
            }
            let enemy_team = self.block_checker_team_of(arena, pc).unwrap_or(-1);
            if enemy_team != -1 && enemy_team != team {
                self.apply_effects(caster, pc, key);
            }
        }
        true
    }
}
