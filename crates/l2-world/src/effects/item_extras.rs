//! What an item instance carries besides its template: an augmentation and element attributes
//! (`item_attributes`, `item_elementals`). They live on the item itself
//! (`ItemData::augmentation`, `ItemData::elementals`), loaded with the item at login and written
//! back by the persistence task.
// Java: entity/item/instance/Item.java (isAugmented, getAugmentation, setAugmentation,
//       removeAugmentation, getElementals, getElemental, applyAttribute, setElementAttr,
//       clearElementAttr, getAttackElementType, getAttackElementPower, getElementDefAttr),
//       entity/item/holders/Elementals.java
//
// The stat side (the bonuses of the augmentation options and of the elements) is applied when
// the item is equipped: `World::equip_item_skills` in `stats/skills.rs`.

use l2_data::item::ItemTemplate;
use l2_util::ObjectId;

use crate::player::ItemData;
use crate::world::World;

/// Java `Elementals`: `getElement()` (`Elementals.FIRE` 0 to `DARK` 5) and `getValue()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemElemental {
    pub element: i32,
    pub value: i32,
}

impl ItemData {
    /// Java `Item.isAugmented()`.
    pub fn is_augmented(&self) -> bool {
        self.augmentation.is_some()
    }

    /// Java `Item.getElemental(attribute)`.
    pub fn elemental(&self, element: i32) -> Option<ItemElemental> {
        self.elementals.iter().find(|e| e.element == element).copied()
    }

    /// Java `Item.applyAttribute(element, value)`: the value of an element the item already has
    /// is replaced, a new element goes to the end.
    pub fn apply_attribute(&mut self, element: i32, value: i32) {
        match self.elementals.iter_mut().find(|e| e.element == element) {
            Some(existing) => existing.value = value,
            None => self.elementals.push(ItemElemental { element, value }),
        }
    }

    /// Java `Item.clearElementAttr(element)`: one element, or all of them for -1.
    pub fn clear_element_attr(&mut self, element: i32) {
        if element == -1 {
            self.elementals.clear();
        } else {
            self.elementals.retain(|e| e.element != element);
        }
    }

    /// Java `Item.getAttackElementType()`: -2 for anything but a weapon; the template's first
    /// element wins over the instance's first.
    pub fn attack_element_type(&self, template: Option<&ItemTemplate>) -> i32 {
        if !template.is_some_and(ItemTemplate::is_weapon) {
            return -2;
        }
        if let Some(first) = template.and_then(|t| t.elementals.first()) {
            return first.element as i32;
        }
        self.elementals.first().map_or(-2, |e| e.element)
    }

    /// Java `Item.getAttackElementPower()`: 0 for anything but a weapon.
    pub fn attack_element_power(&self, template: Option<&ItemTemplate>) -> i32 {
        if !template.is_some_and(ItemTemplate::is_weapon) {
            return 0;
        }
        if let Some(first) = template.and_then(|t| t.elementals.first()) {
            return first.value;
        }
        self.elementals.first().map_or(0, |e| e.value)
    }

    /// Java `Item.getElementDefAttr(element)`: 0 for anything but an armor.
    // NOTE(java-quirk): once the template has any element, the instance's elements are not
    // looked at (`else if`), so an armor with a template element and no entry for `element`
    // reports 0 even when the instance has one.
    pub fn element_def_attr(&self, element: i32, template: Option<&ItemTemplate>) -> i32 {
        let Some(template) = template.filter(|t| t.is_armor()) else {
            return 0;
        };
        if !template.elementals.is_empty() {
            return template
                .elementals
                .iter()
                .find(|e| e.element as i32 == element)
                .map_or(0, |e| e.value);
        }
        self.elemental(element).map_or(0, |e| e.value)
    }
}

impl World {
    fn item_of(&self, item: ObjectId) -> Option<&ItemData> {
        self.players
            .values()
            .find_map(|p| p.data.items.iter().find(|i| i.object_id == item))
    }

    fn item_mut(&mut self, item: ObjectId) -> Option<(ObjectId, &mut ItemData)> {
        self.players.iter_mut().find_map(|(id, p)| {
            p.data
                .items
                .iter_mut()
                .find(|i| i.object_id == item)
                .map(|i| (*id, i))
        })
    }

    /// Java `Item.isAugmented()`.
    pub fn is_item_augmented(&self, item: ObjectId) -> bool {
        self.item_of(item).is_some_and(ItemData::is_augmented)
    }

    /// Java `Item.getAugmentation().getAugmentationId()`, `None` for an item that is not
    /// augmented.
    pub fn item_augmentation(&self, item: ObjectId) -> Option<i32> {
        self.item_of(item).and_then(|i| i.augmentation)
    }

    /// Java `Item.setAugmentation(augmentation)` for `Some(id)` and `Item.removeAugmentation()`
    /// for `None`. The row is written with the next item write (`item_attributes`). Like Java,
    /// an item that already has an augmentation keeps it and the call returns `false`. The
    /// bonuses are not touched: Java applies them with the equip listener, and its callers
    /// (`RequestRefine`) unequip the item first.
    pub fn set_item_augmentation(&mut self, item: ObjectId, augmentation: Option<i32>) -> bool {
        let Some((owner, data)) = self.item_mut(item) else {
            return false;
        };
        if augmentation.is_some() && data.augmentation.is_some() {
            tracing::info!(
                "Warning: Augment set for ({}) item {} owner: {}",
                item.0,
                data.item_id,
                owner.0
            );
            return false;
        }
        data.augmentation = augmentation;
        self.update_item_in_db(owner, item);
        true
    }

    /// Java `Item.getElementals()[0]`.
    pub fn item_elemental(&self, item: ObjectId) -> Option<ItemElemental> {
        self.item_of(item).and_then(|i| i.elementals.first().copied())
    }

    /// Java `Item.setElementAttr(element, value)`. The rows are written with the next item write
    /// (`item_elementals`). The bonuses of an equipped item are not touched: Java's callers run
    /// `updateElementAttrBonus` after it.
    pub fn set_item_elemental(&mut self, item: ObjectId, element: i32, value: i32) {
        let Some((owner, data)) = self.item_mut(item) else {
            return;
        };
        data.apply_attribute(element, value);
        self.update_item_in_db(owner, item);
    }

    /// Java `Item.clearElementAttr(element)`: one element, or all of them for -1.
    pub fn clear_item_elemental(&mut self, item: ObjectId, element: i32) {
        let Some((owner, data)) = self.item_mut(item) else {
            return;
        };
        // "if ((getElemental(element) == null) && (element != -1)) return;"
        if element != -1 && data.elemental(element).is_none() {
            return;
        }
        data.clear_element_attr(element);
        self.update_item_in_db(owner, item);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn item() -> ItemData {
        ItemData {
            object_id: ObjectId(1),
            item_id: 1,
            count: 1,
            ..ItemData::default()
        }
    }

    #[test]
    fn apply_attribute_replaces_or_appends() {
        let mut i = item();
        i.apply_attribute(2, 30);
        i.apply_attribute(4, 20);
        i.apply_attribute(2, 40);
        assert_eq!(
            i.elementals,
            [
                ItemElemental {
                    element: 2,
                    value: 40
                },
                ItemElemental {
                    element: 4,
                    value: 20
                }
            ]
        );
        assert_eq!(
            i.elemental(4),
            Some(ItemElemental {
                element: 4,
                value: 20
            })
        );
        assert_eq!(i.elemental(0), None);
    }

    #[test]
    fn clear_element_attr_removes_one_or_all() {
        let mut i = item();
        i.apply_attribute(2, 30);
        i.apply_attribute(4, 20);
        i.clear_element_attr(2);
        assert_eq!(
            i.elementals,
            [ItemElemental {
                element: 4,
                value: 20
            }]
        );
        i.clear_element_attr(-1);
        assert!(i.elementals.is_empty());
    }

    #[test]
    fn without_a_template_nothing_is_a_weapon_or_an_armor() {
        let mut i = item();
        i.apply_attribute(1, 50);
        assert_eq!(i.attack_element_type(None), -2);
        assert_eq!(i.attack_element_power(None), 0);
        assert_eq!(i.element_def_attr(1, None), 0);
        assert!(!i.is_augmented());
        i.augmentation = Some(0);
        assert!(i.is_augmented());
    }
}
