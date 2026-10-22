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
// - `windows.rs`: `Maps` and `Calculator` (they open a client window).
// - `reuse.rs`: the item reuse stamps (`Creature.addTimeStampItem` and friends), the
//   "remaining re-use time" message and `ExUseSharedGroupItem`.
// - `consume.rs`: `Player.destroyItem(process, objectId, count, ...)`.
// - `shots.rs`: the soulshot and spiritshot handlers (M5-SHOT-01); `use_item_with_handler` tries
//   them first.
// - `SummonItems` (a pet collar) and `PetFood` are handlers of the pet code (M6-PET-01).
//
// # Left out on purpose
// - Pets: `SummonItems` and `PetFood` are in `summon/pet.rs` and `summon/pet_items.rs`; a pet's
//   item skills go through `item_skills.rs` with the pet as the user (its two `isPet()` branches).
// - Events (`isOnEvent`) and the olympiad (`isInOlympiadMode`) are not modelled.

mod book;
mod consume;
mod extractable;
mod item_skills;
mod registry;
mod reuse;
mod windows;

pub mod shots;

pub(crate) use registry::ItemHandlerKind;
pub(crate) use reuse::ItemStamp;
pub use reuse::SavedItemReuse;
