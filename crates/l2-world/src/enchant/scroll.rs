//! The enchant scroll window: using a scroll, putting the item and the support item in, cancelling
//! and the enchant itself.
// Java: dist/game/data/scripts/handlers/items/EnchantScrolls.java
// Java: network/clientpackets/{RequestExTryToPutEnchantTargetItem,RequestExTryToPutEnchantSupportItem,
//       RequestExCancelEnchantItem,RequestEnchantItem}.java
// Java: entity/item/enchant/EnchantScroll.java (calculateSuccess; the odds are `l2_data::enchant`)

use std::sync::Arc;

use l2_data::enchant::EnchantOdds;
use l2_data::item::{ItemTemplate, ItemTemplates};
use l2_data::{EnchantItemData, EnchantItemGroups, EnchantScroll, EnchantSupportItem, EnchantTarget};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::skills::SkillPosition;
use l2_protocol_game::server::{
    ChooseInventoryItem, EnchantResult, ExPutEnchantSupportItemResult, ExPutEnchantTargetItemResult,
    InventoryUpdate, ItemUpdate, MagicSkillUse, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::{info, warn};

use crate::inventory::{LOC_INVENTORY, LOC_PAPERDOLL, item_name_param};
use crate::player::ItemData;
use crate::world::World;

/// `CommonSkill.FIREWORK`: the fireworks of a +6 armor or a +7 and +15 weapon.
const FIREWORK_SKILL: (i32, i32) = (5965, 1);
/// The least time between putting the item in and pressing "Enchant" (the auto enchant check).
const MIN_ENCHANT_DELAY_MS: i64 = 2000;

/// The log line kinds of `RequestEnchantItem`.
#[derive(Debug, Clone, Copy)]
enum LogKind {
    Success,
    SafeFail,
    BlessedFail,
    UnableToDestroy,
    Fail,
}

impl LogKind {
    fn text(self) -> &'static str {
        match self {
            Self::Success => "Success",
            Self::SafeFail => "Safe Fail",
            Self::BlessedFail => "Blessed Fail",
            Self::UnableToDestroy => "Unable to destroy",
            Self::Fail => "Fail",
        }
    }
}

impl World {
    /// Java `Item.isEnchantable()`: in the inventory or worn, and `ItemTemplate.isEnchantable()`
    /// (the template flag, minus the `EnchantBlackList`).
    pub(crate) fn item_is_enchantable(&self, item: &ItemData, template: &ItemTemplate) -> bool {
        (item.loc == LOC_INVENTORY || item.loc == LOC_PAPERDOLL)
            && template.enchantable
            && !self.settings.enchant.enchant_blacklist.contains(&template.id)
    }

    /// The item as the enchant rules of `l2_data` see it.
    fn enchant_target<'a>(&self, item: &ItemData, template: &'a ItemTemplate) -> EnchantTarget<'a> {
        EnchantTarget {
            template,
            enchant_level: item.enchant_level,
            enchantable: self.item_is_enchantable(item, template),
        }
    }

    /// The tables, or `None` before `set_enchant_data` (then no scroll is known, like an empty
    /// `EnchantItemData`).
    fn enchant_tables(&self) -> Option<(Arc<EnchantItemData>, Arc<EnchantItemGroups>, Arc<ItemTemplates>)> {
        Some((
            self.enchant.items.clone()?,
            self.enchant.groups.clone()?,
            self.item_templates()?,
        ))
    }

    /// Java `Item.setEnchantLevel(level)`: never below 0. A worn item changes its enchant options
    /// (`clearEnchantStats()` before, `applyEnchantStats()` after; see `stats/skills.rs`). The row
    /// is written by the caller (`updateDatabase()`).
    pub(crate) fn set_item_enchant_level(&mut self, id: ObjectId, object_id: ObjectId, level: i32) {
        let new_level = level.max(0);
        let current = self.inventory_item(id, object_id).map(|i| i.enchant_level);
        if current == Some(new_level) {
            // `if (_enchantLevel == newLevel) return;`
            return;
        }
        let mut stats = self.clear_enchant_stats(id, object_id);
        if let Some(item) = self
            .players
            .get_mut(&id)
            .and_then(|p| p.data.items.iter_mut().find(|i| i.object_id == object_id))
        {
            item.enchant_level = new_level;
        }
        stats.extend(self.apply_enchant_stats(id, object_id));
        if !stats.is_empty() {
            self.broadcast_modified_stats(id, &stats);
            self.refresh_max_vitals(id, false);
        }
    }

    // ------------------------------------------------------------------ EnchantScrolls

    /// Java `EnchantScrolls.onItemUse`: opens the scroll window.
    // `playable.isPlayer()` is always true here; `player.isMounted()` is not modelled.
    pub(crate) fn enchant_scrolls_on_use(&mut self, id: ObjectId, item: &ItemData) -> bool {
        if self.is_casting_now(id) {
            return false;
        }
        if self.is_enchanting(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::AnotherEnchantmentIsInProgressPleaseCompleteThePreviousTaskThenTryAgain,
                ),
            );
            return false;
        }
        self.set_active_enchant_item(id, Some(item.object_id));
        self.send(
            id,
            &ChooseInventoryItem {
                item_id: item.item_id,
            },
        );
        self.send(id, &SystemMessage::new(SystemMessageId::SelectItemToEnchant));
        true
    }

    // ------------------------------------------------------------------ the small packets

    /// Java `RequestExTryToPutEnchantTargetItem.runImpl`.
    pub(crate) fn on_try_to_put_enchant_target_item(&mut self, id: ObjectId, object_id: i32) {
        if object_id == 0 {
            return;
        }
        if self.is_enchanting(id) {
            return;
        }
        let Some(scroll) = self
            .active_enchant_item(id)
            .and_then(|o| self.inventory_item(id, o))
            .cloned()
        else {
            return;
        };
        let object_id = ObjectId(object_id);
        let Some(item) = self.inventory_item(id, object_id).cloned() else {
            let message = format!(
                "RequestExTryToPutEnchantTargetItem: {} tried to cheat using a packet manipulation tool! Ban this player!",
                self.player_text(id)
            );
            self.enchant_illegal_action(id, &message);
            return;
        };

        let tables = self.enchant_tables();
        let scroll_template = tables
            .as_ref()
            .and_then(|(data, _, _)| data.scroll(scroll.item_id));
        let fits = match (&tables, scroll_template) {
            (Some((data, _, templates)), Some(scroll_template)) => {
                templates.get(item.item_id).is_some_and(|t| {
                    let target = self.enchant_target(&item, t);
                    target.enchantable && scroll_template.is_valid(data, &target, None)
                })
            }
            _ => false,
        };
        if !fits {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::DoesNotFitStrengtheningConditionsOfTheScroll),
            );
            self.set_active_enchant_item(id, None);
            self.send(id, &ExPutEnchantTargetItemResult { result: 0 });
            if scroll_template.is_none() {
                warn!(
                    "RequestExTryToPutEnchantTargetItem: {} has used undefined scroll with id {}",
                    self.player_text(id),
                    scroll.item_id
                );
            }
            return;
        }

        let now = self.world_ms();
        if let Some(player) = self.players.get_mut(&id) {
            player.enchant.enchanting = true;
            player.enchant.timestamp = Some(now);
        }
        self.send(id, &ExPutEnchantTargetItemResult { result: object_id.0 });
    }

    /// Java `RequestExTryToPutEnchantSupportItem.runImpl`.
    pub(crate) fn on_try_to_put_enchant_support_item(
        &mut self,
        id: ObjectId,
        support_object_id: i32,
        enchant_object_id: i32,
    ) {
        if !self.is_enchanting(id) {
            return;
        }
        let item = self.inventory_item(id, ObjectId(enchant_object_id)).cloned();
        let scroll = self
            .active_enchant_item(id)
            .and_then(|o| self.inventory_item(id, o))
            .cloned();
        let support = self.inventory_item(id, ObjectId(support_object_id)).cloned();
        let (Some(item), Some(scroll), Some(support)) = (item, scroll, support) else {
            // message may be custom
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::InappropriateEnchantConditions),
            );
            self.set_active_enchant_support(id, None);
            return;
        };

        let tables = self.enchant_tables();
        let fits = tables.as_ref().is_some_and(|(data, _, templates)| {
            let (Some(scroll_template), Some(support_template)) =
                (data.scroll(scroll.item_id), data.support(support.item_id))
            else {
                return false;
            };
            templates.get(item.item_id).is_some_and(|t| {
                scroll_template.is_valid(data, &self.enchant_target(&item, t), Some(support_template))
            })
        });
        if !fits {
            // message may be custom
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::InappropriateEnchantConditions),
            );
            self.set_active_enchant_support(id, None);
            self.send(id, &ExPutEnchantSupportItemResult { result: 0 });
            return;
        }
        self.set_active_enchant_support(id, Some(support.object_id));
        self.send(
            id,
            &ExPutEnchantSupportItemResult {
                result: support_object_id,
            },
        );
    }

    /// Java `Player.setActiveEnchantSupportItemId(objectId)`.
    fn set_active_enchant_support(&mut self, id: ObjectId, object_id: Option<ObjectId>) {
        if let Some(player) = self.players.get_mut(&id) {
            player.enchant.active_support = object_id;
        }
    }

    /// Java `RequestExCancelEnchantItem.runImpl`.
    pub(crate) fn on_cancel_enchant_item(&mut self, id: ObjectId) {
        self.send(
            id,
            &EnchantResult {
                result: 2,
                crystal: 0,
                count: 0,
            },
        );
        self.set_active_enchant_item(id, None);
    }

    // ------------------------------------------------------------------ RequestEnchantItem

    /// `player.sendPacket(new EnchantResult(result, crystal, count))`.
    fn send_enchant_result(&self, id: ObjectId, result: i32, crystal: i32, count: i64) {
        self.send(
            id,
            &EnchantResult {
                result,
                crystal,
                count,
            },
        );
    }

    /// The `LOGGER_ENCHANT` lines of `LogItemEnchants`. `item`, `scroll` and `support` are the
    /// items as Java's objects are at that point.
    fn log_enchant(
        &self,
        id: ObjectId,
        kind: LogKind,
        item: &ItemData,
        scroll: &ItemData,
        support: Option<&ItemData>,
    ) {
        if !self.settings.enchant.log_item_enchants {
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let templates = self.item_templates();
        let name_of = |i: &ItemData| {
            templates
                .as_ref()
                .and_then(|t| t.get(i.item_id))
                .map_or_else(String::new, |t| t.name.clone())
        };
        let ip = player.client_ip.map_or_else(String::new, |ip| ip.to_string());
        let level = if item.enchant_level > 0 {
            format!("+{} ", item.enchant_level)
        } else {
            String::new()
        };
        let describe = |i: &ItemData| format!("{}({}) [{}]", name_of(i), i.count, i.object_id.0);
        let mut line = format!(
            "{}, Character:{} [{}] Account:{} IP:{}, {}{}, {}",
            kind.text(),
            player.data.name,
            id.0,
            player.data.account,
            ip,
            level,
            describe(item),
            describe(scroll)
        );
        if let Some(support) = support {
            line.push_str(", ");
            line.push_str(&describe(support));
        }
        info!(target: "enchant.items", "{line}");
    }

    /// Java `RequestEnchantItem.runImpl`.
    pub(crate) fn on_request_enchant_item(&mut self, id: ObjectId, object_id: i32) {
        if object_id == 0 {
            return;
        }
        let object_id = ObjectId(object_id);

        if self.is_processing_transaction(id) || self.is_in_store_mode(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouCannotEnchantWhileOperatingAPrivateStoreOrPrivateWorkshop,
                ),
            );
            self.set_active_enchant_item(id, None);
            return;
        }

        let item = self.inventory_item(id, object_id).cloned();
        let scroll = self
            .active_enchant_item(id)
            .and_then(|o| self.inventory_item(id, o))
            .cloned();
        let (Some(item), Some(scroll)) = (item, scroll) else {
            self.set_active_enchant_item(id, None);
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveCancelledTheEnchantingProcess),
            );
            self.send_enchant_result(id, 2, 0, 0);
            return;
        };

        // Template for scroll.
        let Some((data, groups, templates)) = self.enchant_tables() else {
            return;
        };
        let Some(scroll_template) = data.scroll(scroll.item_id) else {
            return;
        };

        let support = self
            .players
            .get(&id)
            .and_then(|p| p.enchant.active_support)
            .and_then(|o| self.inventory_item(id, o))
            .cloned();
        // Template for support item, if exist.
        let mut support_template: Option<&EnchantSupportItem> = None;
        if let Some(support) = &support {
            support_template = data.support(support.item_id);
            if support_template.is_none() {
                self.set_active_enchant_item(id, None);
                return;
            }
        }

        // First validation check, also over enchant check.
        let item_template = templates.get(item.item_id);
        let valid = item_template.is_some_and(|t| {
            scroll_template.is_valid(&data, &self.enchant_target(&item, t), support_template)
        });
        if !valid
            || (self.settings.enchant.disable_over_enchanting
                && item.enchant_level == scroll_template.max_enchant_level())
        {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::InappropriateEnchantConditions),
            );
            self.set_active_enchant_item(id, None);
            self.send_enchant_result(id, 2, 0, 0);
            return;
        }

        // Fast auto-enchant cheat check.
        let now = self.world_ms();
        let put_in = self.players.get(&id).and_then(|p| p.enchant.timestamp);
        if put_in.is_none_or(|at| now - at < MIN_ENCHANT_DELAY_MS) {
            let message = format!("{} use autoenchant program ", self.player_text(id));
            self.enchant_illegal_action(id, &message);
            self.set_active_enchant_item(id, None);
            self.send_enchant_result(id, 2, 0, 0);
            return;
        }

        // Attempting to destroy scroll.
        let Some(destroyed_scroll) = self.destroy_inventory_item(id, scroll.object_id, 1) else {
            self.send(id, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
            let message = format!(
                "{} tried to enchant with a scroll he doesn't have",
                self.player_text(id)
            );
            self.enchant_illegal_action(id, &message);
            self.set_active_enchant_item(id, None);
            self.send_enchant_result(id, 2, 0, 0);
            return;
        };
        let mut iu: Vec<ItemUpdate> = vec![self.destroyed_entry(&destroyed_scroll)];

        // Attempting to destroy support if exists.
        let mut destroyed_support: Option<ItemData> = None;
        if let Some(support) = &support {
            let Some(destroyed) = self.destroy_inventory_item(id, support.object_id, 1) else {
                self.send(id, &SystemMessage::new(SystemMessageId::IncorrectItemCount2));
                let message = format!(
                    "{} tried to enchant with a support item he doesn't have",
                    self.player_text(id)
                );
                self.enchant_illegal_action(id, &message);
                self.set_active_enchant_item(id, None);
                self.send_enchant_result(id, 2, 0, 0);
                return;
            };
            iu.push(self.destroyed_entry(&destroyed));
            destroyed_support = Some(destroyed);
        }

        // Last validation check.
        // NOTE(java-quirk): the scroll (and support item) are already used up when this fails, and
        // the `InventoryUpdate` that says so is never sent.
        let current = self.inventory_item(id, item.object_id).cloned().filter(|i| {
            templates
                .get(i.item_id)
                .is_some_and(|t| self.item_is_enchantable(i, t))
        });
        let Some(item) = current else {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::InappropriateEnchantConditions),
            );
            self.set_active_enchant_item(id, None);
            self.send_enchant_result(id, 2, 0, 0);
            return;
        };
        let Some(item_template) = templates.get(item.item_id) else {
            return;
        };

        let target = self.enchant_target(&item, item_template);
        match scroll_template.odds(&data, &groups, &target, support_template) {
            EnchantOdds::Error => {
                self.send(
                    id,
                    &SystemMessage::new(SystemMessageId::InappropriateEnchantConditions),
                );
                self.set_active_enchant_item(id, None);
                self.send_enchant_result(id, 2, 0, 0);
            }
            EnchantOdds::Chance(chance) => {
                let random = 100.0 * self.rng().next_f64();
                let scroll_after = destroyed_scroll;
                if random < chance {
                    self.enchant_success(
                        id,
                        &item,
                        (item_template, scroll_template, &groups),
                        &mut iu,
                        (&scroll_after, destroyed_support.as_ref()),
                    );
                } else if !self.enchant_failure(
                    id,
                    &item,
                    item_template,
                    scroll_template,
                    &mut iu,
                    (&scroll_after, destroyed_support.as_ref()),
                ) {
                    return;
                }
            }
        }

        // Sent directly, a delayed update can leave the equipped item shown with its old enchant.
        self.flush_inventory_update(id);
        self.send_inventory_update_now(id, iu);
        self.broadcast_user_info(id);
        self.set_active_enchant_item(id, None);
    }

    /// `iu.addModifiedItem(item)` for a scroll that was used up (`addRemovedItem` when none is
    /// left).
    fn destroyed_entry(&self, destroyed: &ItemData) -> ItemUpdate {
        let change = if destroyed.count > 0 {
            InventoryUpdate::MODIFIED
        } else {
            InventoryUpdate::REMOVED
        };
        self.update_entry(destroyed, change)
    }

    /// `case SUCCESS` of `RequestEnchantItem`.
    fn enchant_success(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        (template, scroll_template, groups): (&ItemTemplate, &EnchantScroll, &EnchantItemGroups),
        iu: &mut Vec<ItemUpdate>,
        used: (&ItemData, Option<&ItemData>),
    ) {
        // Increase enchant level only if scroll's base template has chance, some armors can
        // success over +20 but they shouldn't have increased.
        if scroll_template.chance(groups, &self.enchant_target(item, template)) > 0.0 {
            self.set_item_enchant_level(id, item.object_id, item.enchant_level + 1);
            if let Some(entry) = self.modified_entry(id, item.object_id) {
                iu.push(entry);
            }
            self.update_item_in_db(id, item.object_id);

            if item.loc == LOC_PAPERDOLL {
                // The client keeps the old enchant on an equipped item, so the full list is sent
                // as well.
                self.send_item_list(id, false);
            }
        }

        self.send_enchant_result(id, 0, 0, 0);
        let now = self
            .inventory_item(id, item.object_id)
            .cloned()
            .unwrap_or_else(|| item.clone());
        self.log_enchant(id, LogKind::Success, &now, used.0, used.1);

        // Announce the success.
        let armor = template.is_armor();
        let min_enchant_announce = if armor { 6 } else { 7 };
        let max_enchant_announce = if armor { 0 } else { 15 };
        if now.enchant_level == min_enchant_announce || now.enchant_level == max_enchant_announce {
            let name = self
                .players
                .get(&id)
                .map(|p| p.data.name.clone())
                .unwrap_or_default();
            let templates = self.item_templates();
            let sm = SystemMessage::new(SystemMessageId::C1HasSuccessfullyEnchantedAS2S3)
                .with(SystemMessageParam::Text(name))
                .with(SystemMessageParam::IntNumber(now.enchant_level))
                .with(item_name_param(templates.as_deref(), now.item_id));
            self.broadcast_to_self_and_visible(id, &sm);

            self.broadcast_firework(id);
        }

        if armor && now.enchant_level == 4 && now.loc == LOC_PAPERDOLL {
            if let Some(skill) = template.skills.enchant4_skill {
                // Add skills bestowed from +4 armor.
                self.add_skill(id, skill.id, skill.level);
                self.send_skill_list(id);
            }
        }

        self.broadcast_user_info(id); // Update user info.
    }

    /// `broadcastSkillPacket(new MagicSkillUse(player, player, skill.getId(), skill.getLevel(),
    /// skill.getHitTime(), skill.getReuseDelay()), player)` with `CommonSkill.FIREWORK`.
    fn broadcast_firework(&self, id: ObjectId) {
        let Some(skill) = self
            .data
            .stat_data()
            .and_then(|d| d.skills.get_skill(FIREWORK_SKILL.0, FIREWORK_SKILL.1))
        else {
            return;
        };
        let Some((x, y, z)) = self.position_of(id) else {
            return;
        };
        let here = SkillPosition { x, y, z };
        // The constructor reads the ground point of a player's last ground skill.
        let ground = self
            .cast_state(id)
            .and_then(|s| s.current_skill_world_position)
            .map(|(x, y, z)| SkillPosition { x, y, z });
        self.broadcast_creature_packet(
            id,
            &MagicSkillUse {
                creature_id: id.0,
                target_id: id.0,
                skill_id: skill.id,
                skill_level: skill.level,
                hit_time: skill.hit_time,
                reuse_delay: skill.reuse_delay,
                creature: here,
                ground_location: ground,
                target: here,
            },
        );
    }

    /// `case FAILURE` of `RequestEnchantItem`. `false` when the item could not be destroyed (Java
    /// returns from `runImpl` there, without the common end).
    fn enchant_failure(
        &mut self,
        id: ObjectId,
        item: &ItemData,
        template: &ItemTemplate,
        scroll_template: &EnchantScroll,
        iu: &mut Vec<ItemUpdate>,
        used: (&ItemData, Option<&ItemData>),
    ) -> bool {
        if scroll_template.is_safe {
            // Safe enchant: Remain old value.
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::EnchantFailedTheEnchantLevelForTheCorrespondingItemWillBeExactlyRetained,
                ),
            );
            self.send_enchant_result(id, 5, 0, 0);
            self.log_enchant(id, LogKind::SafeFail, item, used.0, used.1);
            return true;
        }

        // Unequip item on enchant failure to avoid item skills stack.
        if item.loc == LOC_PAPERDOLL {
            let templates = self.item_templates();
            let name = item_name_param(templates.as_deref(), item.item_id);
            let sm = if item.enchant_level > 0 {
                SystemMessage::new(SystemMessageId::TheEquipmentS1S2HasBeenRemoved)
                    .with(SystemMessageParam::IntNumber(item.enchant_level))
                    .with(name)
            } else {
                SystemMessage::new(SystemMessageId::S1HasBeenDisarmed).with(name)
            };
            self.send(id, &sm);

            // Sent on its own, a removal of the same item would replace the unequip and leave it
            // shown as equipped.
            let changed =
                self.unequip_item_in_slot_and_record(id, usize::try_from(item.loc_data).unwrap_or(0));
            let unequip_update: Vec<ItemUpdate> = changed
                .iter()
                .filter_map(|o| self.modified_entry(id, *o))
                .collect();
            self.flush_inventory_update(id);
            self.send_inventory_update_now(id, unequip_update);
        }

        if scroll_template.is_blessed {
            // Blessed enchant: Clear enchant value.
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::TheBlessedEnchantFailedTheEnchantValueOfTheItemBecame0),
            );
            self.set_item_enchant_level(id, item.object_id, 0);
            if let Some(entry) = self.modified_entry(id, item.object_id) {
                iu.push(entry);
            }
            self.update_item_in_db(id, item.object_id);
            self.send_enchant_result(id, 3, 0, 0);
            let now = self
                .inventory_item(id, item.object_id)
                .cloned()
                .unwrap_or_else(|| item.clone());
            self.log_enchant(id, LogKind::BlessedFail, &now, used.0, used.1);
            return true;
        }

        // Enchant failed, destroy item.
        let Some(destroyed) = self.destroy_inventory_item(id, item.object_id, item.count) else {
            // Unable to destroy item, cheater?
            let message = format!(
                "Unable to delete item on enchant failure from {}, possible cheater !",
                self.player_text(id)
            );
            self.enchant_illegal_action(id, &message);
            self.set_active_enchant_item(id, None);
            self.send_enchant_result(id, 2, 0, 0);
            self.log_enchant(id, LogKind::UnableToDestroy, item, used.0, used.1);
            return false;
        };
        // Item is gone, always tell the client to remove it.
        iu.push(self.update_entry(&destroyed, InventoryUpdate::REMOVED));

        let crystal_id = template.crystal_item_id();
        if crystal_id != 0 && template.is_crystallizable() {
            let mut count = i64::from(template.crystal_count_at(item.enchant_level))
                - i64::from((template.crystal_count + 1) / 2);
            count = count.max(1);
            let crystals = self.inventory_add_item_by_id(id, crystal_id, count);

            let templates = self.item_templates();
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouHaveEarnedS2S1S)
                    .with(item_name_param(templates.as_deref(), crystal_id))
                    .with(SystemMessageParam::LongNumber(count)),
            );

            // Add the crystals gained, not the destroyed item.
            // Report as modified if stacked onto an existing crystal stack, otherwise as a new item.
            if let Some(crystals) = crystals {
                let change = if crystals.change == InventoryUpdate::MODIFIED {
                    InventoryUpdate::MODIFIED
                } else {
                    InventoryUpdate::ADDED
                };
                iu.push(self.update_entry(&crystals.item, change));
            }

            self.send_enchant_result(id, 1, crystal_id, count);
        } else {
            self.send_enchant_result(id, 4, 0, 0);
        }
        self.log_enchant(id, LogKind::Fail, &destroyed, used.0, used.1);
        true
    }
}
