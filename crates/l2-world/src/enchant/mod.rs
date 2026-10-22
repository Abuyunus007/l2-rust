//! Enchanting items and putting elements on them (M6-ENCHANT-01).
// Java: network/clientpackets/{RequestEnchantItem,RequestExTryToPutEnchantTargetItem,
//       RequestExTryToPutEnchantSupportItem,RequestExCancelEnchantItem,RequestExEnchantItemAttribute,
//       RequestExRemoveItemAttribute}.java
// Java: dist/game/data/scripts/handlers/items/{EnchantScrolls,EnchantAttribute}.java
// Java: dist/game/data/scripts/handlers/bypass/npc/ReleaseAttribute.java
// Java: network/serverpackets/ExShowBaseAttributeCancelWindow.java
// Java: entity/actor/Player.java (the `_activeEnchant*` fields, `isEnchanting`)
// Java: network/clientpackets/{UseItem,EnterWorld}.java (the over-enchant protection)
//
// # Model
// The data (`EnchantItemData`, `EnchantItemGroupsData`, `ElementalAttributeData`) is read only and
// lives in [`EnchantWorld`], installed with [`World::set_enchant_data`]. What Java keeps in the
// `Player` ("which scroll is open", the support item, the stone, the time the item was put in) is
// [`EnchantState`] on the [`Player`](crate::Player), so it goes away with the player.
//
// - `scroll.rs`: the scroll window: `EnchantScrolls`, `RequestExTryToPutEnchantTargetItem`,
//   `RequestExTryToPutEnchantSupportItem`, `RequestExCancelEnchantItem`, `RequestEnchantItem`.
// - `attribute.rs`: the element stones: `EnchantAttribute`, `RequestExEnchantItemAttribute`,
//   `RequestExRemoveItemAttribute`, the cancel window of the `ReleaseAttribute` NPC link.
// - `protection.rs`: `OverEnchantProtection` (`UseItem`, `EnterWorld`, `//create_item`).
// - `skill.rs`: the skill enchanting (M6-ENCHANT-02): `RequestExEnchantSkillInfo`,
//   `RequestExEnchantSkillInfoDetail` and the four ways to change an enchanted skill.
//
// The enchant options of items (`EnchantItemOptionsData`, `applyEnchantStats`) are the stat
// engine's (`stats/skills.rs`, `World::clear_enchant_stats` / `apply_enchant_stats`); the option
// ids reach the item packets through `ItemTemplate::enchant_options_at`.
//
// `//enchant` (the GM page that sets the enchant level of worn items) is `admin/enchant.rs`.
//
// # Where else it shows
// While a scroll is open the item manipulation checks refuse it (`validate_item_manipulation`,
// `check_item_manipulation`, `is_item_available`), a trade cannot be confirmed
// (`TradeDone`), logout and restart are refused (`canLogout`), `ConvertItem` and the
// `ChangeWeapon` condition give up (`isEnchanting`).
//
// # Left out on purpose
// - Mounts (`isMounted`): not modelled, so the scroll never refuses a mounted player.
// - The Java GS "enchant.items" log file: the lines go to `tracing` with the target `enchant.items`
//   (`LogItemEnchants`).

mod attribute;
mod protection;
mod scroll;
mod skill;

pub use attribute::release_attribute_handler;

use std::sync::Arc;

use l2_data::{ElementalAttributeData, EnchantItemData, EnchantItemGroups};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{EnchantResult, SystemMessage};
use l2_util::ObjectId;

use crate::punishments::IllegalActionPunishment;
use crate::world::World;

/// The values of `Player.ini` / `General.ini` this module reads. `Default` is the Java default
/// of each key.
#[derive(Debug, Clone, PartialEq)]
pub struct EnchantSettings {
    /// `PlayerConfig.OVER_ENCHANT_PROTECTION` (`OverEnchantProtection`).
    pub over_enchant_protection: bool,
    /// `PlayerConfig.DISABLE_OVER_ENCHANTING` (`DisableOverEnchanting`).
    pub disable_over_enchanting: bool,
    /// `PlayerConfig.OVER_ENCHANT_PUNISHMENT` (`OverEnchantPunishment`).
    pub over_enchant_punishment: IllegalActionPunishment,
    /// `PlayerConfig.ENCHANT_CHANCE_ELEMENT_STONE` (`EnchantChanceElementStone`), also used for
    /// the rough ores.
    pub enchant_chance_element_stone: f64,
    /// `ENCHANT_CHANCE_ELEMENT_CRYSTAL`.
    pub enchant_chance_element_crystal: f64,
    /// `ENCHANT_CHANCE_ELEMENT_JEWEL`.
    pub enchant_chance_element_jewel: f64,
    /// `ENCHANT_CHANCE_ELEMENT_ENERGY`.
    pub enchant_chance_element_energy: f64,
    /// `PlayerConfig.ENCHANT_BLACKLIST` (`EnchantBlackList`): item ids `isEnchantable()` refuses.
    pub enchant_blacklist: Vec<i32>,
    /// `GeneralConfig.LOG_ITEM_ENCHANTS` (`LogItemEnchants`).
    pub log_item_enchants: bool,
    /// `GeneralConfig.LOG_SKILL_ENCHANTS` (`LogSkillEnchants`).
    pub log_skill_enchants: bool,
    /// `PlayerConfig.ES_SP_BOOK_NEEDED` (`EnchantSkillSpBookNeeded`): the spellbook is used up.
    pub es_sp_book_needed: bool,
    /// `GeneralConfig.NORMAL_ENCHANT_COST_MULTIPLIER` (`NormalEnchantCostMultipiler`, sic).
    pub normal_enchant_cost_multiplier: i32,
    /// `GeneralConfig.SAFE_ENCHANT_COST_MULTIPLIER` (`SafeEnchantCostMultipiler`, sic).
    pub safe_enchant_cost_multiplier: i32,
}

impl Default for EnchantSettings {
    fn default() -> Self {
        Self {
            over_enchant_protection: true,
            disable_over_enchanting: true,
            over_enchant_punishment: IllegalActionPunishment::Jail,
            enchant_chance_element_stone: 50.0,
            enchant_chance_element_crystal: 30.0,
            enchant_chance_element_jewel: 20.0,
            enchant_chance_element_energy: 10.0,
            enchant_blacklist: vec![
                7816, 7817, 7818, 7819, 7820, 7821, 7822, 7823, 7824, 7825, 7826, 7827, 7828, 7829, 7830,
                7831, 13293, 13294, 13296,
            ],
            log_item_enchants: false,
            log_skill_enchants: false,
            es_sp_book_needed: true,
            normal_enchant_cost_multiplier: 1,
            safe_enchant_cost_multiplier: 5,
        }
    }
}

/// The loaded enchant tables.
#[derive(Debug, Default)]
pub struct EnchantWorld {
    pub(crate) items: Option<Arc<EnchantItemData>>,
    pub(crate) groups: Option<Arc<EnchantItemGroups>>,
    pub(crate) elementals: Option<Arc<ElementalAttributeData>>,
}

/// The enchant fields of Java's `Player`: `_isEnchanting`, `_activeEnchantItemId`,
/// `_activeEnchantSupportItemId`, `_activeEnchantAttrItemId` and `_activeEnchantTimestamp`.
/// Java's `ID_NONE` (-1) is `None`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnchantState {
    /// `isEnchanting()`: the item to enchant was put into the window.
    pub enchanting: bool,
    /// `getActiveEnchantItemId()`: the scroll in use.
    pub active_item: Option<ObjectId>,
    /// `getActiveEnchantSupportItemId()`.
    pub active_support: Option<ObjectId>,
    /// `getActiveEnchantAttrItemId()`: the element stone in use.
    pub active_attr: Option<ObjectId>,
    /// `getActiveEnchantTimestamp()` in world milliseconds (Java's 0 is `None`).
    pub timestamp: Option<i64>,
}

/// The enchant packets, as the world gets them (see `ClientInput::Enchant`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnchantInput {
    /// Java `RequestEnchantItem`.
    Enchant { object_id: i32 },
    /// Java `RequestExTryToPutEnchantTargetItem`.
    PutTargetItem { object_id: i32 },
    /// Java `RequestExTryToPutEnchantSupportItem`.
    PutSupportItem {
        support_object_id: i32,
        enchant_object_id: i32,
    },
    /// Java `RequestExCancelEnchantItem`.
    CancelEnchant,
    /// Java `RequestExEnchantItemAttribute` (`-1`: cancelled).
    EnchantAttribute { object_id: i32 },
    /// Java `RequestExRemoveItemAttribute`.
    RemoveAttribute { object_id: i32, element: i32 },
    /// Java `RequestExEnchantSkillInfo`.
    SkillInfo { skill_id: i32, skill_level: i32 },
    /// Java `RequestExEnchantSkillInfoDetail` (`kind` is `_type`).
    SkillInfoDetail {
        kind: i32,
        skill_id: i32,
        skill_level: i32,
    },
    /// Java `RequestExEnchantSkill`.
    EnchantSkill { skill_id: i32, skill_level: i32 },
    /// Java `RequestExEnchantSkillSafe`.
    EnchantSkillSafe { skill_id: i32, skill_level: i32 },
    /// Java `RequestExEnchantSkillUntrain`.
    UntrainSkill { skill_id: i32, skill_level: i32 },
    /// Java `RequestExEnchantSkillRouteChange`.
    ChangeSkillRoute { skill_id: i32, skill_level: i32 },
}

impl World {
    /// Installs `EnchantItemData`, `EnchantItemGroupsData` and `ElementalAttributeData`.
    pub fn set_enchant_data(
        &mut self,
        items: Arc<EnchantItemData>,
        groups: Arc<EnchantItemGroups>,
        elementals: Arc<ElementalAttributeData>,
    ) {
        if self.settings.enchant.over_enchant_protection {
            groups.log_max_enchants();
        }
        self.enchant.items = Some(items);
        self.enchant.groups = Some(groups);
        self.enchant.elementals = Some(elementals);
    }

    /// The enchant fields of a player.
    pub fn enchant_state(&self, id: ObjectId) -> Option<&EnchantState> {
        self.players.get(&id).map(|p| &p.enchant)
    }

    /// Java `Player.getActiveEnchantItemId()` (`None` is `ID_NONE`).
    pub fn active_enchant_item(&self, id: ObjectId) -> Option<ObjectId> {
        self.players.get(&id).and_then(|p| p.enchant.active_item)
    }

    /// Java `Player.isEnchanting()`.
    pub fn is_enchanting(&self, id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| p.enchant.enchanting)
    }

    /// Java `Player.setActiveEnchantItemId(objectId)`: without an enchant item the support item,
    /// the timestamp and the `enchanting` flag go too.
    pub(crate) fn set_active_enchant_item(&mut self, id: ObjectId, object_id: Option<ObjectId>) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let state = &mut player.enchant;
        // If we don't have a Enchant Item, we are not enchanting.
        if object_id.is_none() {
            state.active_support = None;
            state.timestamp = None;
            state.enchanting = false;
        }
        state.active_item = object_id;
    }

    /// `PlayerInventory.removeItem`: "Removes active Enchant Scroll."
    pub(crate) fn enchant_item_removed(&mut self, id: ObjectId, object_id: ObjectId) {
        if self.active_enchant_item(id) == Some(object_id) {
            self.set_active_enchant_item(id, None);
        }
    }

    /// The part of `Player.canLogout()` and `RequestRestart` about the enchant windows.
    pub(crate) fn enchant_blocks_logout(&self, id: ObjectId) -> bool {
        self.players
            .get(&id)
            .is_some_and(|p| p.enchant.active_item.is_some() || p.enchant.active_attr.is_some())
    }

    /// Java `Item.isAvailable`: the scroll, support item and stone in use are not available.
    pub(crate) fn is_enchant_item_in_use(&self, id: ObjectId, object_id: ObjectId) -> bool {
        self.players.get(&id).is_some_and(|p| {
            p.enchant.active_item == Some(object_id)
                || p.enchant.active_support == Some(object_id)
                || p.enchant.active_attr == Some(object_id)
        })
    }

    /// A packet of the enchant windows.
    pub(crate) fn on_enchant_input(&mut self, id: ObjectId, input: EnchantInput) {
        match input {
            EnchantInput::Enchant { object_id } => self.on_request_enchant_item(id, object_id),
            EnchantInput::PutTargetItem { object_id } => {
                self.on_try_to_put_enchant_target_item(id, object_id)
            }
            EnchantInput::PutSupportItem {
                support_object_id,
                enchant_object_id,
            } => self.on_try_to_put_enchant_support_item(id, support_object_id, enchant_object_id),
            EnchantInput::CancelEnchant => self.on_cancel_enchant_item(id),
            EnchantInput::EnchantAttribute { object_id } => {
                self.on_request_enchant_item_attribute(id, object_id)
            }
            EnchantInput::RemoveAttribute { object_id, element } => {
                self.on_request_remove_item_attribute(id, object_id, element);
            }
            EnchantInput::SkillInfo {
                skill_id,
                skill_level,
            } => self.on_request_ex_enchant_skill_info(id, skill_id, skill_level),
            EnchantInput::SkillInfoDetail {
                kind,
                skill_id,
                skill_level,
            } => self.on_request_ex_enchant_skill_info_detail(id, kind, skill_id, skill_level),
            EnchantInput::EnchantSkill {
                skill_id,
                skill_level,
            } => self.on_request_ex_enchant_skill(id, skill_id, skill_level),
            EnchantInput::EnchantSkillSafe {
                skill_id,
                skill_level,
            } => self.on_request_ex_enchant_skill_safe(id, skill_id, skill_level),
            EnchantInput::UntrainSkill {
                skill_id,
                skill_level,
            } => self.on_request_ex_enchant_skill_untrain(id, skill_id, skill_level),
            EnchantInput::ChangeSkillRoute {
                skill_id,
                skill_level,
            } => self.on_request_ex_enchant_skill_route_change(id, skill_id, skill_level),
        }
    }

    /// The enchant check of `SendWareHouseDepositList` and `SendWareHouseWithDrawList`: a scroll in
    /// use is cancelled (`YOU_HAVE_CANCELLED_THE_ENCHANTING_PROCESS`, `EnchantResult(2, 0, 0)`)
    /// and the request ends. `true` when the caller must return.
    pub fn warehouse_cancels_enchant(&mut self, id: ObjectId) -> bool {
        if self.active_enchant_item(id).is_none() {
            return false;
        }
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouHaveCancelledTheEnchantingProcess),
        );
        self.send(
            id,
            &EnchantResult {
                result: 2,
                crystal: 0,
                count: 0,
            },
        );
        self.set_active_enchant_item(id, None);
        true
    }

    /// The enchant check of `RequestPackageSend`: a scroll in use is an illegal action. `true`
    /// when the caller must return.
    pub fn package_send_enchant_exploit(&mut self, id: ObjectId) -> bool {
        if self.active_enchant_item(id).is_none() {
            return false;
        }
        let message = format!("{} tried to use enchant Exploit!", self.player_text(id));
        self.enchant_illegal_action(id, &message);
        true
    }

    /// `player.isEnchanting()` of the `PrivateWarehouse` and `ClanWarehouse` links.
    pub fn enchant_blocks_warehouse_link(&self, id: ObjectId) -> bool {
        self.is_enchanting(id)
    }

    /// `PunishmentManager.handleIllegalPlayerAction(player, message, DEFAULT_PUNISH)`.
    fn enchant_illegal_action(&mut self, id: ObjectId, message: &str) {
        let punishment = self.default_punish();
        self.handle_illegal_player_action(id, message, punishment);
    }
}
