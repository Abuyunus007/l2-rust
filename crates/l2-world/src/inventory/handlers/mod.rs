//! Item handlers: what a double click does to an item that is not worn (M5-ITEM-01).
// Java: dist/game/data/scripts/handlers/items/*.java, handler/ItemHandler.java
// Java: network/clientpackets/UseItem.java (the handler branch and the reuse checks)
//
// # How it fits together
// `registry.rs` is Java's `ItemHandler` table: the `handler` name of an etc item maps to an
// [`ItemHandlerKind`], and [`World::use_item_with_handler`] is the `else` branch of `UseItem`
// (handler lookup, run, then the item reuse stamp on success). Adding a handler is one enum
// variant, one name in `from_name`, and one arm in `run_item_handler`.
//
// - `item_skills.rs`: `ItemSkillsTemplate` / `ItemSkills` / `Elixir` (potions, scrolls, herbs).
// - `extractable.rs`: `ExtractableItems` (boxes and packs: `<capsuled_items>`).
// - `book.rs`: `Book` (the help page of an item).
// - `reuse.rs`: the item reuse stamps (`Creature.addTimeStampItem` and friends), the
//   "remaining re-use time" message and `ExUseSharedGroupItem`.
// - `consume.rs`: `Player.destroyItem(process, objectId, count, ...)`.
//
// # Left out on purpose
// - `Maps` and `Calculator` need the `ShowMiniMap` and `ShowCalculator` packets, which the
//   protocol crate does not have yet.
// - Pets: `Playable` is a player here (summons and pets are M6), so every `isPet()` branch is
//   dead and left out.
// - Events (`isOnEvent`) and the olympiad (`isInOlympiadMode`) are not modelled.
// - The item reuse stamps are not saved (`character_item_reuse_save`, Java `storeItemReuseDelay` /
//   `restoreItemReuse`): a relog forgets them.

mod book;
mod consume;
mod extractable;
mod item_skills;
mod registry;
mod reuse;

pub(crate) use registry::ItemHandlerKind;
pub(crate) use reuse::ItemStamp;
