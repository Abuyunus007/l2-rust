//! `ItemSkills`, `ItemSkillsTemplate` and `Elixir`: potions, scrolls, elixirs and herbs cast the
//! skills the item carries.
// Java: dist/game/data/scripts/handlers/items/ItemSkillsTemplate.java
// Java: dist/game/data/scripts/handlers/items/ItemSkills.java
// Java: dist/game/data/scripts/handlers/items/Elixir.java
//
// # What happens
// For every `item_skill` of the item: the skill's own conditions, its reuse, the casting state;
// then the skill is cast. A static skill of an item with an immediate effect (potions, herbs) is
// cast *simultaneously*, so a potion works while a spell is being cast; anything else (a scroll
// of escape) goes through `useMagic` like a skill the player picked, with its cast time and its
// own item cost (`itemConsumeId`). An item with an immediate effect and no consuming skill is
// used up here.

use l2_data::SkillTemplate;
use l2_data::item::ActionType;
use l2_data::item::ItemTemplate;
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::SystemMessage;
use l2_util::ObjectId;
use tracing::info;

use super::reuse::reuse_message;
use crate::effects::SkillKey;
use crate::effects::buff_info::skill_name_param;
use crate::inventory::item_name_param;
use crate::player::ItemData;
use crate::world::World;

impl World {
    /// Java `ItemSkillsTemplate.onItemUse`, which `ItemSkills` and `Elixir` extend. `item` is not
    /// necessarily in the inventory: a picked-up herb is used from the ground. `true` when the
    /// item was used.
    pub(crate) fn item_skills_on_use(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
        force_use: bool,
    ) -> bool {
        // `ItemSkills` (and so `Elixir`) refuse in a Grand Olympiad match: the handler registry
        // (`registry.rs`, M8-OLY-01). Events (`isOnEvent()`) are not modelled.
        if !self.is_player(id) && !self.is_pet(id) {
            return false;
        }
        // Pets can use items only when they are tradable.
        if self.is_pet(id) && !template.tradeable {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YourPetCannotCarryThisItem),
            );
            return false;
        }

        // Verify that item is not under reuse.
        if !self.check_item_skill_reuse(id, None, item) {
            return false;
        }

        let Some(skills) = template.skills.item_skills.clone() else {
            info!(
                "Item {} does not have registered any skill for handler.",
                template.label()
            );
            return false;
        };
        let Some(table) = self.skill_table() else {
            return false;
        };

        let mut has_consume_skill = false;
        for skill_ref in skills {
            // A skill that does not exist is skipped (`SkillHolder.getSkill()` is null).
            let Some(item_skill) = table.get(skill_ref.id, skill_ref.level) else {
                continue;
            };
            let key = SkillKey::of(item_skill);
            if item_skill.item_consume_id > 0 {
                has_consume_skill = true;
            }

            let target = self.target_of(id).unwrap_or(ObjectId(0));
            if !self.check_skill_condition(id, target, key) {
                return false;
            }

            // Verify that skill is not under reuse.
            if !self.check_item_skill_reuse(id, Some(item_skill), item) {
                return false;
            }

            if self.is_skill_disabled(id, item_skill) {
                return false;
            }

            if !template.is_potion()
                && !template.is_elixir()
                && !template.is_scroll()
                && self.is_casting_now(id)
            {
                return false;
            }

            // Send message to the master.
            if self.is_pet(id) {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::YourPetUsesS1).with(skill_name_param(item_skill)),
                );
            }

            if item_skill.simultaneous_cast
                || ((template.immediate_effect || template.ex_immediate_effect) && item_skill.is_static())
            {
                self.do_simultaneous_cast(id, key);
            } else {
                self.set_intention_idle(id);
                if !self.use_magic(id, key, force_use, false) {
                    return false;
                }
            }
        }

        if check_consume(template, has_consume_skill)
            && !self.destroy_item_by_object_id(id, item.object_id, 1, false)
        {
            self.send(id, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            return false;
        }

        true
    }

    /// Java `ItemSkillsTemplate.checkReuse(playable, skill, item)`: `true` when the skill (or,
    /// without one, the item) is not in reuse. The player hears how long is left; a static skill
    /// is named by its item.
    fn check_item_skill_reuse(
        &mut self,
        id: ObjectId,
        skill: Option<&SkillTemplate>,
        item: &ItemData,
    ) -> bool {
        let remaining = match skill {
            Some(skill) => self.skill_remaining_reuse_time(id, skill.hash_code()),
            None => self.item_remaining_reuse_time(id, item.object_id),
        };
        let available = remaining <= 0;
        // `playable.isPlayer() && !isAvailable`: a pet is not told.
        if !available && self.is_player(id) {
            let name = match skill {
                Some(skill) if !skill.is_static() => skill_name_param(skill),
                _ => {
                    let templates = self.item_templates();
                    item_name_param(templates.as_deref(), item.item_id)
                }
            };
            self.send(id, &reuse_message(name, remaining));
        }
        available
    }
}

/// Java `ItemSkillsTemplate.checkConsume(item, hasConsumeSkill)`: the item is used up here when
/// it is a capsule or a reducing item with an immediate effect and no skill takes it.
fn check_consume(template: &ItemTemplate, has_consume_skill: bool) -> bool {
    match template.default_action {
        ActionType::Capsule | ActionType::SkillReduce => !has_consume_skill && template.immediate_effect,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use l2_data::ItemTemplates;

    use super::*;

    fn template(xml: &str) -> ItemTemplate {
        let templates = ItemTemplates::from_xml(&format!("<list>{xml}</list>")).unwrap();
        templates.iter().next().unwrap().clone()
    }

    #[test]
    fn only_capsules_and_reducing_items_with_an_immediate_effect_are_used_up() {
        let potion = template(
            r#"<item id="1" type="EtcItem" name="P"><set name="default_action" val="SKILL_REDUCE" />
            <set name="immediate_effect" val="true" /></item>"#,
        );
        assert!(check_consume(&potion, false));
        // A skill that takes the item itself (a scroll of escape) leaves it to the cast.
        assert!(!check_consume(&potion, true));
        let scroll = template(
            r#"<item id="2" type="EtcItem" name="S"><set name="default_action" val="SKILL_REDUCE" /></item>"#,
        );
        assert!(!check_consume(&scroll, false));
        let herb = template(
            r#"<item id="3" type="EtcItem" name="H"><set name="immediate_effect" val="true" /></item>"#,
        );
        assert!(!check_consume(&herb, false));
        let capsule = template(
            r#"<item id="4" type="EtcItem" name="C"><set name="default_action" val="CAPSULE" />
            <set name="immediate_effect" val="true" /></item>"#,
        );
        assert!(check_consume(&capsule, false));
    }
}
