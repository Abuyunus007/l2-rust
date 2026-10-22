//! The item handler table and the handler branch of `UseItem`.
// Java: handler/ItemHandler.java (getHandler)
// Java: dist/game/data/scripts/handlers/MasterHandler.java (the names registered)
// Java: network/clientpackets/UseItem.java (the `else` branch of `if (item.isEquipable())`)

use l2_data::item::ItemTemplate;
use l2_util::ObjectId;
use tracing::warn;

use super::shots::ShotHandler;
use crate::player::ItemData;
use crate::world::World;

/// The handlers this server has. Java registers a class per name; the name is the `handler` set
/// of the item (`<set name="handler" val="ItemSkills" />`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ItemHandlerKind {
    /// `ItemSkills`: the item casts its skills (not in the olympiad).
    ItemSkills,
    /// `ItemSkillsTemplate`: the same without the olympiad rule.
    ItemSkillsTemplate,
    /// `Elixir`: `ItemSkills` for players only.
    Elixir,
    /// `ExtractableItems`: opens a box.
    ExtractableItems,
    /// `Book`: shows the help page of the item.
    Book,
    /// `Maps`: opens the mini map (M5-FIX-03).
    Maps,
    /// `Calculator`: opens the calculator (M5-FIX-03).
    Calculator,
    /// `Recipes`: a recipe item joins the recipe book (M6-CRAFT-01).
    Recipes,
    /// `SummonItems`: a pet collar (M6-PET-01).
    SummonItems,
    /// `PetFood` (M6-PET-01).
    PetFood,
    /// `EnchantScrolls`: opens the enchant scroll window (M6-ENCHANT-01).
    EnchantScrolls,
    /// `EnchantAttribute`: opens the element stone window (M6-ENCHANT-01).
    EnchantAttribute,
}

impl ItemHandlerKind {
    /// Java `ItemHandler._datatable.get(name)`: the names are case sensitive.
    pub(crate) fn from_name(name: &str) -> Option<Self> {
        Some(match name {
            "ItemSkills" => Self::ItemSkills,
            "ItemSkillsTemplate" => Self::ItemSkillsTemplate,
            "Elixir" => Self::Elixir,
            "ExtractableItems" => Self::ExtractableItems,
            "Book" => Self::Book,
            "Maps" => Self::Maps,
            "Calculator" => Self::Calculator,
            "Recipes" => Self::Recipes,
            "SummonItems" => Self::SummonItems,
            "PetFood" => Self::PetFood,
            "EnchantScrolls" => Self::EnchantScrolls,
            "EnchantAttribute" => Self::EnchantAttribute,
            _ => return None,
        })
    }

    /// Java `ItemHandler.getHandler(etcItem)`: `None` for weapons and armor, for an etc item
    /// without a handler name and for a name nothing is registered under.
    pub(crate) fn of_template(template: &ItemTemplate) -> Option<Self> {
        Self::from_name(template.etc()?.handler.as_deref()?)
    }
}

impl World {
    /// Java `IItemHandler.onItemUse(player, item, forceUse)`. `item` does not have to be in the
    /// inventory: a herb is used from the ground. `true` when the item was used.
    pub(crate) fn run_item_handler(
        &mut self,
        kind: ItemHandlerKind,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
        force_use: bool,
    ) -> bool {
        match kind {
            // `ItemSkills` only adds the olympiad rule (not modelled), `Elixir` the player-only
            // rule (a pet's item skills go through `ItemSkillsTemplate`'s own pet branches).
            ItemHandlerKind::ItemSkills | ItemHandlerKind::ItemSkillsTemplate | ItemHandlerKind::Elixir => {
                self.item_skills_on_use(id, item, template, force_use)
            }
            ItemHandlerKind::ExtractableItems => self.extractable_items_on_use(id, item, template),
            ItemHandlerKind::Book => self.book_on_use(id, item),
            ItemHandlerKind::Maps => self.maps_on_use(id, item),
            ItemHandlerKind::Calculator => self.calculator_on_use(id, item),
            ItemHandlerKind::Recipes => self.recipes_on_use(id, item),
            ItemHandlerKind::SummonItems => self.summon_items_on_use(id, item, template, force_use),
            ItemHandlerKind::PetFood => self.pet_food_on_use(id, item, template),
            ItemHandlerKind::EnchantScrolls => self.enchant_scrolls_on_use(id, item),
            ItemHandlerKind::EnchantAttribute => self.enchant_attribute_on_use(id, item),
        }
    }

    /// The end of `UseItem.runImpl`: the item is not equipable, so its handler runs, and when it
    /// used the item the reuse delay starts.
    pub(crate) fn use_item_with_handler(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
        ctrl_pressed: bool,
    ) {
        // M6: the fishing rod branch (a lure goes to `PAPERDOLL_LHAND` while a rod is held)
        // needs fishing, which is not modelled.

        // The shot handlers (M5-SHOT-01) have no item reuse delay, so the stamp below is not
        // needed for them.
        if let Some(shot) = ShotHandler::of_template(template) {
            self.use_shot_item(id, item, shot);
            return;
        }

        let Some(kind) = ItemHandlerKind::of_template(template) else {
            if let Some(name) = template.etc().and_then(|e| e.handler.as_deref()) {
                warn!("Unmanaged Item handler: {name} for Item Id: {}!", item.item_id);
            }
            return;
        };

        // Item reuse time should be added if the item is successfully used. Skill reuse delay is
        // added by `beginCast` when the cast actually starts.
        if self.run_item_handler(kind, id, item, template, ctrl_pressed) && template.reuse_delay > 0 {
            let reuse_delay = template.reuse_delay;
            self.add_time_stamp_item(id, item, template, i64::from(reuse_delay));
            self.send_shared_group_update(
                id,
                item.item_id,
                template.shared_reuse_group,
                i64::from(reuse_delay),
                reuse_delay,
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_exact() {
        assert_eq!(
            ItemHandlerKind::from_name("ItemSkills"),
            Some(ItemHandlerKind::ItemSkills)
        );
        assert_eq!(
            ItemHandlerKind::from_name("Elixir"),
            Some(ItemHandlerKind::Elixir)
        );
        assert_eq!(ItemHandlerKind::from_name("Maps"), Some(ItemHandlerKind::Maps));
        assert_eq!(
            ItemHandlerKind::from_name("Calculator"),
            Some(ItemHandlerKind::Calculator)
        );
        assert_eq!(ItemHandlerKind::from_name("itemskills"), None);
        assert_eq!(
            ItemHandlerKind::from_name("Recipes"),
            Some(ItemHandlerKind::Recipes)
        );
    }
}
