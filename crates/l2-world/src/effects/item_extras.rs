//! What an item instance can carry that the item rows of the world do not model yet: an
//! augmentation and an elemental attribute. `ConvertItem` reads both (an augmented weapon is not
//! converted, the element goes to the new weapon).
// Java: entity/item/instance/Item.java (isAugmented, getElementals, setElementAttr),
//       entity/item/holders/Elementals.java
//
// NOTE: the item loader does not read the augmentation or the element columns yet (`enter.rs` sends
// every item as a plain one), so nothing fills this table at login. It is a side table keyed by the
// item's object id, like the other per-creature tables of the effect engine, so `ItemData` keeps
// its shape; whoever loads augmentations and elements (and writes them back) calls the setters.

use std::collections::HashMap;

use l2_util::ObjectId;

use crate::world::World;

/// Java `Elementals` of an item: `getElement()` and `getValue()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ItemElemental {
    pub element: i32,
    pub value: i32,
}

/// The extras of one item.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) struct ItemExtras {
    pub augmented: bool,
    pub elemental: Option<ItemElemental>,
}

/// The table (`EffectsWorld::item_extras`).
pub(crate) type ItemExtrasTable = HashMap<ObjectId, ItemExtras>;

impl World {
    /// Java `Item.isAugmented()`.
    pub fn is_item_augmented(&self, item: ObjectId) -> bool {
        self.effects.item_extras.get(&item).is_some_and(|e| e.augmented)
    }

    /// Java `Item.setAugmentation(..)` / `removeAugmentation()`, as a flag.
    pub fn set_item_augmented(&mut self, item: ObjectId, augmented: bool) {
        self.item_extras_mut(item).augmented = augmented;
        self.sweep_item_extras(item);
    }

    /// Java `Item.getElementals()[0]`.
    pub fn item_elemental(&self, item: ObjectId) -> Option<ItemElemental> {
        self.effects.item_extras.get(&item).and_then(|e| e.elemental)
    }

    /// Java `Item.setElementAttr(element, value)`.
    pub fn set_item_elemental(&mut self, item: ObjectId, element: i32, value: i32) {
        self.item_extras_mut(item).elemental = Some(ItemElemental { element, value });
    }

    /// The item is gone (destroyed, or its owner left): its extras go.
    pub(crate) fn forget_item_extras(&mut self, item: ObjectId) {
        self.effects.item_extras.remove(&item);
    }

    fn item_extras_mut(&mut self, item: ObjectId) -> &mut ItemExtras {
        self.effects.item_extras.entry(item).or_default()
    }

    /// Drops an entry that says nothing.
    fn sweep_item_extras(&mut self, item: ObjectId) {
        if self.effects.item_extras.get(&item) == Some(&ItemExtras::default()) {
            self.effects.item_extras.remove(&item);
        }
    }
}
