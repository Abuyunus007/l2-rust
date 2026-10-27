//! Skill enchanting: the info windows and the four ways to change an enchanted skill (enchant, safe
//! enchant, untrain, change route), with their SP, adena and spellbook costs.
// Java: network/clientpackets/{RequestExEnchantSkill,RequestExEnchantSkillSafe,
//       RequestExEnchantSkillUntrain,RequestExEnchantSkillRouteChange,RequestExEnchantSkillInfo,
//       RequestExEnchantSkillInfoDetail}.java
// Java: network/serverpackets/{ExEnchantSkillInfo,ExEnchantSkillInfoDetail,ExEnchantSkillResult}.java
// Java: data/xml/EnchantSkillGroupsData.java (the constants), entity/actor/Player.java
//       (isAllowedToEnchantSkills, addSkill, updateShortcuts)
//
// # The flow
// The client asks for the routes of a skill (`RequestExEnchantSkillInfo`, answered with
// `ExEnchantSkillInfo`), for what a step costs (`RequestExEnchantSkillInfoDetail`, answered with
// `ExEnchantSkillInfoDetail`) and then does the step. A step takes SP, adena and (usually) a
// spellbook, rolls the chance of the player's level, and replaces the skill by the next level of
// its route (`101`, `102`, ...), by the base level again (failure of a normal enchant), or by the
// level one step down (untrain). The new level is a `character_skills` row like any other skill
// (D-39), written through [`SkillWrites`]. Afterwards the client gets `UserInfo`, the `SkillList`,
// both info packets for the new level, and the shortcuts of the skill show the new level.
//
// # What is left out
// - `player.isSellingBuffs()` (the SellBuffs custom feature is not ported).
// - The Java GS "enchant.skills" log file: the lines go to `tracing` with the target
//   `enchant.skills` (`LogSkillEnchants`).

use std::sync::Arc;

use l2_data::enchant_skill::{
    CHANGE_ENCHANT_BOOK, NORMAL_ENCHANT_BOOK, SAFE_ENCHANT_BOOK, UNTRAIN_ENCHANT_BOOK,
};
use l2_data::{ClassId, EnchantSkillGroupsData, EnchantSkillHolder};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    ExEnchantSkillInfo, ExEnchantSkillInfoDetail, ExEnchantSkillResult, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::{info, warn};

use crate::flood::FloodKind;
use crate::rewards::exp::SkillWrites;
use crate::world::World;

/// `Inventory.ADENA_ID`.
const ADENA_ID: i32 = l2_data::item::ADENA_ID;

/// `TYPE_NORMAL_ENCHANT` .. `TYPE_CHANGE_ENCHANT` of `ExEnchantSkillInfoDetail`.
const TYPE_NORMAL_ENCHANT: i32 = 0;
const TYPE_SAFE_ENCHANT: i32 = 1;
const TYPE_UNTRAIN_ENCHANT: i32 = 2;
const TYPE_CHANGE_ENCHANT: i32 = 3;

/// The skill template `SkillData.getSkill(id, level)` found: its own id and level (a level past
/// the highest gives the highest) and `Skill.toString()` for the log.
struct SkillRef {
    id: i32,
    level: i32,
    label: String,
}

impl World {
    /// The tables of the static data, or `None` without a datapack (then nothing is enchantable,
    /// like an empty `EnchantSkillGroupsData`).
    fn enchant_skill_data(&self) -> Option<Arc<EnchantSkillGroupsData>> {
        self.data.enchant_skill_groups()
    }

    /// Java `SkillData.getSkill(skillId, skillLevel)`.
    fn enchant_skill_ref(&self, skill_id: i32, level: i32) -> Option<SkillRef> {
        let skill = self.data.stat_data()?.skills.get_skill(skill_id, level)?;
        Some(SkillRef {
            id: skill.id,
            level: skill.level,
            label: format!("Skill {}({},{})", skill.name, skill.id, skill.level),
        })
    }

    /// Java `Creature.getSkillLevel(skillId)`: 0 when the skill is not known.
    fn enchant_known_level(&self, id: ObjectId, skill_id: i32) -> i32 {
        self.players
            .get(&id)
            .and_then(|p| p.core.skills.iter().find(|(s, _)| *s == skill_id))
            .map_or(0, |&(_, level)| level)
    }

    /// Java `Player.isAllowedToEnchantSkills()`.
    pub fn is_allowed_to_enchant_skills(&self, id: ObjectId) -> bool {
        if self.is_changing_class(id) {
            return false;
        }
        if self.is_transformed(id) || self.is_in_stance(id) {
            return false;
        }
        if self.has_attack_stance_task(id) {
            return false;
        }
        if self.is_casting_now(id) || self.is_casting_simultaneously_now(id) {
            return false;
        }
        if self.is_in_boat(id) || self.is_in_air_ship(id) {
            return false;
        }
        true
    }

    /// Java `Player.getPlayerClass().level()`.
    fn enchant_class_level(&self, id: ObjectId) -> i32 {
        self.players
            .get(&id)
            .and_then(|p| u16::try_from(p.data.class_id).ok())
            .map_or(0, |class| ClassId(class).class_level())
    }

    /// The first three checks of the four changing requests: a third class, level 76 and a state
    /// that allows it. `false` ends the request (the player has been told why).
    fn enchant_skill_allowed(&mut self, id: ObjectId) -> bool {
        // requires to have 3rd class quest completed
        if self.enchant_class_level(id) < 3 {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouCannotUseTheSkillEnhancingFunctionInThisClassYouCanUseCorrespondingFunctionWhenCompletingTheThirdClassChange,
                ),
            );
            return false;
        }
        if self.core_of(id).map_or(0, |c| c.level) < 76 {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouCannotUseTheSkillEnhancingFunctionOnThisLevelYouCanUseTheCorrespondingFunctionOnLevelsHigherThan76lv,
                ),
            );
            return false;
        }
        if !self.is_allowed_to_enchant_skills(id) {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouCannotUseTheSkillEnhancingFunctionInThisClassYouCanUseTheSkillEnhancingFunctionUnderOffBattleStatusAndCannotUseTheFunctionWhileTransformingBattlingAndOnBoard,
                ),
            );
            return false;
        }
        true
    }

    // ------------------------------------------------------------------ the info packets

    /// Java `new ExEnchantSkillInfo(id, level)`: the levels the client may pick next.
    pub(crate) fn enchant_skill_info_packet(&self, skill_id: i32, level: i32) -> ExEnchantSkillInfo {
        let mut info = ExEnchantSkillInfo {
            skill_id,
            level,
            max_enchanted: false,
            routes: Vec::new(),
        };
        let Some(data) = self.enchant_skill_data() else {
            return info;
        };
        // do we have this skill?
        let Some(learn) = data.skill_enchantment_by_skill_id(skill_id) else {
            return info;
        };
        if level > 100 {
            // skill already enchanted
            info.max_enchanted = data.is_max_enchant(skill_id, level);
            // get detail for next level; if it exists add it
            if data.enchant_skill_holder(skill_id, level).is_some() {
                // current enchant add first
                info.routes.push(level);
            }
            let skill_level = level % 100;
            for route in learn.all_routes() {
                if route * 100 + skill_level == level {
                    continue;
                }
                // add other levels of all routes - same level as enchanted level
                info.routes.push(route * 100 + skill_level);
            }
        } else {
            // not already enchanted
            for route in learn.all_routes() {
                // add first level (+1) of all routes
                info.routes.push(route * 100 + 1);
            }
        }
        info
    }

    /// Java `new ExEnchantSkillInfoDetail(type, skillId, skillLevel, player)`. `None` is Java's
    /// `IllegalArgumentException` ("does not have enchant data for level").
    pub(crate) fn enchant_skill_info_detail_packet(
        &self,
        id: ObjectId,
        kind: i32,
        skill_id: i32,
        skill_level: i32,
    ) -> Option<ExEnchantSkillInfoDetail> {
        let data = self.enchant_skill_data()?;
        let esd: &EnchantSkillHolder = if data.skill_enchantment_by_skill_id(skill_id).is_some() {
            if skill_level > 100 {
                data.enchant_skill_holder(skill_id, skill_level)?
            } else {
                data.first_route_group(skill_id)?.details.first()?
            }
        } else {
            warn!("Skill {skill_id} does not have enchant data for level {skill_level}");
            return None;
        };

        let settings = &self.settings.enchant;
        let multi = match kind {
            TYPE_NORMAL_ENCHANT => settings.normal_enchant_cost_multiplier,
            TYPE_SAFE_ENCHANT => settings.safe_enchant_cost_multiplier,
            _ => 1,
        };
        let player_level = self.core_of(id).map_or(1, |c| c.level);
        let chance = i32::from(esd.rate(player_level));
        let mut sp = esd.sp_cost;
        if kind == TYPE_UNTRAIN_ENCHANT {
            sp = (0.8 * f64::from(sp)) as i32;
        }
        let adena = esd.adena_cost.wrapping_mul(multi);
        let (book_id, mut book_required) = match kind {
            TYPE_NORMAL_ENCHANT => (NORMAL_ENCHANT_BOOK, (skill_level % 100) < 2),
            TYPE_SAFE_ENCHANT => (SAFE_ENCHANT_BOOK, true),
            TYPE_UNTRAIN_ENCHANT => (UNTRAIN_ENCHANT_BOOK, true),
            TYPE_CHANGE_ENCHANT => (CHANGE_ENCHANT_BOOK, true),
            // `default: return;` in the constructor: the book fields stay as they were.
            _ => (0, false),
        };
        if matches!(
            kind,
            TYPE_NORMAL_ENCHANT | TYPE_UNTRAIN_ENCHANT | TYPE_CHANGE_ENCHANT
        ) && !settings.es_sp_book_needed
        {
            book_required = false;
        }
        Some(ExEnchantSkillInfoDetail {
            kind,
            skill_id,
            skill_level,
            sp: sp.wrapping_mul(multi),
            chance,
            adena,
            book_id,
            book_required,
        })
    }

    /// `player.sendPacket(new ExEnchantSkillInfoDetail(...))`. Java's constructor throws for a level
    /// without data, which ends the request; `false` here does the same.
    fn send_enchant_skill_info_detail(&mut self, id: ObjectId, kind: i32, skill_id: i32, level: i32) -> bool {
        match self.enchant_skill_info_detail_packet(id, kind, skill_id, level) {
            Some(packet) => {
                self.send(id, &packet);
                true
            }
            None => false,
        }
    }

    // ------------------------------------------------------------------ the info requests

    /// Java `RequestExEnchantSkillInfo.runImpl`.
    pub(crate) fn on_request_ex_enchant_skill_info(&mut self, id: ObjectId, skill_id: i32, skill_level: i32) {
        if skill_id <= 0 || skill_level <= 0 {
            return;
        }
        if !self.players.contains_key(&id) {
            return;
        }
        if self.core_of(id).map_or(0, |c| c.level) < 76 {
            return;
        }
        match self.enchant_skill_ref(skill_id, skill_level) {
            Some(skill) if skill.id == skill_id => {}
            _ => return,
        }
        let Some(data) = self.enchant_skill_data() else {
            return;
        };
        if data.skill_enchantment_by_skill_id(skill_id).is_none() {
            return;
        }
        // `playerskillLevel == -1` never happens: an unknown skill is level 0.
        let known = self.enchant_known_level(id, skill_id);
        if known != skill_level {
            return;
        }
        let packet = self.enchant_skill_info_packet(skill_id, skill_level);
        self.send(id, &packet);
    }

    /// Java `RequestExEnchantSkillInfoDetail.runImpl`.
    pub(crate) fn on_request_ex_enchant_skill_info_detail(
        &mut self,
        id: ObjectId,
        kind: i32,
        skill_id: i32,
        skill_level: i32,
    ) {
        if skill_id <= 0 || skill_level <= 0 {
            return;
        }
        if !self.players.contains_key(&id) {
            return;
        }
        let required = match kind {
            // enchant
            0 | 1 => skill_level - 1,
            // untrain
            2 => skill_level + 1,
            // change route
            3 => skill_level,
            _ => -2,
        };
        let known = self.enchant_known_level(id, skill_id);
        // does not have such skill
        if known == 0 {
            return;
        }
        // if reqlevel is 100,200,.. check base skill level enchant
        if required % 100 == 0 {
            let base = self.enchant_skill_data().and_then(|d| {
                d.skill_enchantment_by_skill_id(skill_id)
                    .map(|esl| esl.base_level)
            });
            match base {
                // if player does not have min level to enchant
                Some(base_level) if known != base_level => return,
                Some(_) => {}
                // enchant data does not exist?
                None => return,
            }
        } else if known != required && kind == 3 && (known % 100) != (skill_level % 100) {
            // change route is different skill level but same enchant
            return;
        }
        // send skill enchantment detail
        self.send_enchant_skill_info_detail(id, kind, skill_id, skill_level);
    }

    // ------------------------------------------------------------------ the costs

    /// The spellbook of the step: Java `getInventory().getItemByItemId(reqItemId)`.
    fn enchant_book_of(&self, id: ObjectId, book_id: i32) -> Option<ObjectId> {
        self.inventory_item_by_item_id(id, book_id).map(|i| i.object_id)
    }

    /// The SP, the book and the adena of a step, all taken, none checked in between (Java uses
    /// `&=` so that every part runs). `book` is the stack to use one of, when the step uses one.
    /// `sp` is `None` for the untrain, which gives SP back instead.
    fn take_enchant_costs(
        &mut self,
        id: ObjectId,
        sp: Option<i64>,
        book: Option<ObjectId>,
        adena: i32,
    ) -> bool {
        let mut check = true;
        if let Some(sp) = sp {
            check = self.remove_exp_and_sp(id, 0, sp, false);
        }
        if let Some(book) = book {
            check &= self.destroy_item_by_object_id(id, book, 1, true);
        }
        check &= self.destroy_item_by_item_id(id, ADENA_ID, i64::from(adena), true);
        check
    }

    fn send_not_enough_items(&mut self, id: ObjectId) {
        self.send(
            id,
            &SystemMessage::new(SystemMessageId::YouDoNotHaveAllOfTheItemsNeededToEnchantThatSkill),
        );
    }

    /// `LOGGER_ENCHANT.info(...)` of the changing requests.
    fn log_skill_enchant(
        &self,
        id: ObjectId,
        what: &str,
        skill: &SkillRef,
        book: Option<ObjectId>,
        rate: Option<i8>,
    ) {
        if !self.settings.enchant.log_skill_enchants {
            return;
        }
        let Some(player) = self.players.get(&id) else {
            return;
        };
        let ip = player.client_ip.map_or_else(String::new, |ip| ip.to_string());
        let spb = book
            .and_then(|b| player.data.items.iter().find(|i| i.object_id == b))
            .map_or_else(
                || "null".to_owned(),
                |item| {
                    let name = self.data.item_name(item.item_id).unwrap_or_default();
                    format!("{name}({})[{}]", item.item_id, item.object_id)
                },
            );
        let mut line = format!(
            "{what}, Character:{} [{}] Account:{} IP:{ip}, {}, SPB:{spb}",
            player.data.name, id.0, player.data.account, skill.label
        );
        if let Some(rate) = rate {
            line.push_str(&format!(", Rate:{rate}"));
        }
        info!(target: "enchant.skills", "{line}");
    }

    /// Java `player.addSkill(skill, true)`: the new level replaces the old one and its row is
    /// written.
    fn enchant_add_skill(&mut self, id: ObjectId, skill: &SkillRef) {
        let mut writes = SkillWrites::default();
        if !self.learn_skill(id, skill.id, skill.level, &mut writes) {
            warn!("{id} could not add skill {}/{}", skill.id, skill.level);
        }
        self.flush_skill_writes(id, writes);
    }

    fn skill_name_message(message: SystemMessageId, skill_id: i32) -> SystemMessage {
        SystemMessage::new(message).with(SystemMessageParam::SkillName {
            id: skill_id,
            level: 1,
        })
    }

    /// The packets that end a changing request: `UserInfo` (sent by the caller where Java does),
    /// the `SkillList`, the info of the new level.
    // Java sends `updateUserInfo()` before or after the result messages depending on the request,
    // so the callers send it themselves.
    fn finish_enchant_skill(&mut self, id: ObjectId, skill_id: i32, detail: (i32, i32)) -> i32 {
        self.send_skill_list(id);
        let after = self.enchant_known_level(id, skill_id);
        let packet = self.enchant_skill_info_packet(skill_id, after);
        self.send(id, &packet);
        // `new ExEnchantSkillInfoDetail(type, id, after + delta, player)` throws without data; the
        // shortcuts are not updated then.
        let (kind, delta) = detail;
        if !self.send_enchant_skill_info_detail(id, kind, skill_id, after + delta) {
            return after;
        }
        self.update_shortcuts(id, skill_id, after);
        after
    }

    // ------------------------------------------------------------------ the changing requests

    /// Java `RequestExEnchantSkill.runImpl`: the normal enchant. A failure sets the skill back to
    /// its base level.
    pub(crate) fn on_request_ex_enchant_skill(&mut self, id: ObjectId, skill_id: i32, skill_level: i32) {
        if !self.flood_allows(id, FloodKind::PlayerAction) {
            return;
        }
        if skill_id <= 0 || skill_level <= 0 {
            return;
        }
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.enchant_skill_allowed(id) {
            return;
        }
        // `isSellingBuffs()`: not modelled.
        let Some(skill) = self.enchant_skill_ref(skill_id, skill_level) else {
            return;
        };
        let Some(data) = self.enchant_skill_data() else {
            return;
        };
        let Some(learn) = data.skill_enchantment_by_skill_id(skill_id) else {
            return;
        };
        let before = self.enchant_known_level(id, skill_id);
        if before != learn.min_skill_level(skill_level) {
            return;
        }
        // NOTE(java-quirk): Java dereferences the holder without a null check; a level that is on
        // no route throws and nothing happens.
        let Some(esd) = data.enchant_skill_holder(skill_id, skill_level) else {
            return;
        };

        let multiplier = self.settings.enchant.normal_enchant_cost_multiplier;
        let required_sp = esd.sp_cost.wrapping_mul(multiplier);
        let sp = self.players.get(&id).map_or(0, |p| p.data.sp);
        if sp < i64::from(required_sp) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughSpToEnchantThatSkill),
            );
            return;
        }

        // only first level requires book
        let uses_book = skill_level % 100 == 1; // 101, 201, 301 ...
        let book_needed = self.settings.enchant.es_sp_book_needed && uses_book;
        let spb = self.enchant_book_of(id, NORMAL_ENCHANT_BOOK);
        if book_needed && spb.is_none() {
            // Haven't spellbook
            self.send_not_enough_items(id);
            return;
        }
        let required_adena = esd.adena_cost.wrapping_mul(multiplier);
        if self.adena_of(id) < i64::from(required_adena) {
            self.send_not_enough_items(id);
            return;
        }

        let used_book = if book_needed { spb } else { None };
        if !self.take_enchant_costs(id, Some(i64::from(required_sp)), used_book, required_adena) {
            self.send_not_enough_items(id);
            return;
        }

        // ok. Destroy ONE copy of the book
        let rate = esd.rate(self.core_of(id).map_or(1, |c| c.level));
        if self.rng().below(100) <= i32::from(rate) {
            self.log_skill_enchant(id, "Success", &skill, spb, Some(rate));
            self.enchant_add_skill(id, &skill);
            self.send(id, &ExEnchantSkillResult { enchanted: true });
            self.send(
                id,
                &Self::skill_name_message(
                    SystemMessageId::SkillEnchantWasSuccessfulS1HasBeenEnchanted,
                    skill_id,
                ),
            );
        } else {
            if let Some(base) = self.enchant_skill_ref(skill_id, learn.base_level) {
                self.enchant_add_skill(id, &base);
            }
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::SkillEnchantFailedTheSkillWillBeInitialized),
            );
            self.send(id, &ExEnchantSkillResult { enchanted: false });
            self.log_skill_enchant(id, "Failed", &skill, spb, Some(rate));
        }

        self.send_user_info(id);
        self.finish_enchant_skill(id, skill_id, (TYPE_NORMAL_ENCHANT, 1));
    }

    /// Java `RequestExEnchantSkillSafe.runImpl`: a failure keeps the level.
    pub(crate) fn on_request_ex_enchant_skill_safe(&mut self, id: ObjectId, skill_id: i32, skill_level: i32) {
        if skill_id <= 0 || skill_level <= 0 {
            return;
        }
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.enchant_skill_allowed(id) {
            return;
        }
        let Some(skill) = self.enchant_skill_ref(skill_id, skill_level) else {
            return;
        };
        let multiplier = self.settings.enchant.safe_enchant_cost_multiplier;
        let Some(data) = self.enchant_skill_data() else {
            return;
        };
        let Some(learn) = data.skill_enchantment_by_skill_id(skill_id) else {
            return;
        };
        let before = self.enchant_known_level(id, skill_id);
        if before != learn.min_skill_level(skill_level) {
            return;
        }
        let Some(esd) = data.enchant_skill_holder(skill_id, skill_level) else {
            return;
        };

        let required_sp = esd.sp_cost.wrapping_mul(multiplier);
        let required_items = esd.adena_cost.wrapping_mul(multiplier);
        let rate = esd.rate(self.core_of(id).map_or(1, |c| c.level));
        let sp = self.players.get(&id).map_or(0, |p| p.data.sp);
        if sp < i64::from(required_sp) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughSpToEnchantThatSkill),
            );
            return;
        }

        // No config option for safe enchant book consume
        let Some(spb) = self.enchant_book_of(id, SAFE_ENCHANT_BOOK) else {
            // Haven't spellbook
            self.send_not_enough_items(id);
            return;
        };
        if self.adena_of(id) < i64::from(required_items) {
            self.send_not_enough_items(id);
            return;
        }
        if !self.take_enchant_costs(id, Some(i64::from(required_sp)), Some(spb), required_items) {
            self.send_not_enough_items(id);
            return;
        }

        // ok. Destroy ONE copy of the book
        if self.rng().below(100) <= i32::from(rate) {
            self.log_skill_enchant(id, "Safe Success", &skill, Some(spb), Some(rate));
            self.enchant_add_skill(id, &skill);
            self.send(id, &ExEnchantSkillResult { enchanted: true });
            self.send(
                id,
                &Self::skill_name_message(
                    SystemMessageId::SkillEnchantWasSuccessfulS1HasBeenEnchanted,
                    skill_id,
                ),
            );
        } else {
            self.log_skill_enchant(id, "Safe Failed", &skill, Some(spb), Some(rate));
            self.send(
                id,
                &Self::skill_name_message(
                    SystemMessageId::SkillEnchantFailedCurrentLevelOfEnchantSkillS1WillRemainUnchanged,
                    skill_id,
                ),
            );
            self.send(id, &ExEnchantSkillResult { enchanted: false });
        }

        self.send_user_info(id);
        self.finish_enchant_skill(id, skill_id, (TYPE_SAFE_ENCHANT, 1));
    }

    /// Java `RequestExEnchantSkillUntrain.runImpl`: one enchant level back, for adena and a
    /// book, and 80 % of the SP of the level given back.
    pub(crate) fn on_request_ex_enchant_skill_untrain(
        &mut self,
        id: ObjectId,
        skill_id: i32,
        skill_level: i32,
    ) {
        let mut skill_level = skill_level;
        if skill_id <= 0 || skill_level <= 0 {
            return;
        }
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.enchant_skill_allowed(id) {
            return;
        }
        let Some(data) = self.enchant_skill_data() else {
            return;
        };
        let Some(learn) = data.skill_enchantment_by_skill_id(skill_id) else {
            return;
        };
        if skill_level % 100 == 0 {
            skill_level = learn.base_level;
        }
        let Some(skill) = self.enchant_skill_ref(skill_id, skill_level) else {
            return;
        };

        let before = self.enchant_known_level(id, skill_id);
        if (before - 1) != skill_level && ((before % 100) != 1 || skill_level != learn.base_level) {
            return;
        }

        let book_needed = self.settings.enchant.es_sp_book_needed;
        let spb = self.enchant_book_of(id, UNTRAIN_ENCHANT_BOOK);
        // does not have spellbook
        if book_needed && spb.is_none() {
            self.send_not_enough_items(id);
            return;
        }

        // NOTE(java-quirk): Java dereferences the holder without a null check.
        let Some(esd) = data.enchant_skill_holder(skill_id, before) else {
            return;
        };
        let required_items = esd.adena_cost;
        if self.adena_of(id) < i64::from(required_items) {
            self.send_not_enough_items(id);
            return;
        }

        let used_book = if book_needed { spb } else { None };
        if !self.take_enchant_costs(id, None, used_book, required_items) {
            self.send_not_enough_items(id);
            return;
        }

        self.add_sp(id, i64::from((f64::from(esd.sp_cost) * 0.8) as i32));
        self.log_skill_enchant(id, "Untrain", &skill, spb, None);

        self.enchant_add_skill(id, &skill);
        self.send(id, &ExEnchantSkillResult { enchanted: true });

        self.send_user_info(id);
        if skill_level > 100 {
            self.send(
                id,
                &Self::skill_name_message(
                    SystemMessageId::UntrainOfEnchantSkillWasSuccessfulCurrentLevelOfEnchantSkillS1HasBeenDecreasedBy1,
                    skill_id,
                ),
            );
        } else {
            self.send(
                id,
                &Self::skill_name_message(
                    SystemMessageId::UntrainOfEnchantSkillWasSuccessfulCurrentLevelOfEnchantSkillS1Became0AndEnchantSkillWillBeInitialized,
                    skill_id,
                ),
            );
        }

        // The info for the level before `after - 1`.
        self.finish_enchant_skill(id, skill_id, (TYPE_UNTRAIN_ENCHANT, -1));
    }

    /// Java `RequestExEnchantSkillRouteChange.runImpl`: the same step on another route, with the
    /// chance of losing up to three steps.
    pub(crate) fn on_request_ex_enchant_skill_route_change(
        &mut self,
        id: ObjectId,
        skill_id: i32,
        skill_level: i32,
    ) {
        let mut skill_level = skill_level;
        if skill_id <= 0 || skill_level <= 0 {
            return;
        }
        if !self.players.contains_key(&id) {
            return;
        }
        if !self.enchant_skill_allowed(id) {
            return;
        }
        if self.enchant_skill_ref(skill_id, skill_level).is_none() {
            return;
        }
        let Some(data) = self.enchant_skill_data() else {
            return;
        };
        let Some(learn) = data.skill_enchantment_by_skill_id(skill_id) else {
            return;
        };
        let before = self.enchant_known_level(id, skill_id);

        // do you have this skill enchanted?
        if before <= 100 {
            return;
        }
        let current_enchant_level = before % 100;
        // is the requested level valid?
        if current_enchant_level != skill_level % 100 {
            return;
        }
        // NOTE(java-quirk): Java dereferences the holder without a null check.
        let Some(esd) = data.enchant_skill_holder(skill_id, skill_level) else {
            return;
        };
        let required_sp = esd.sp_cost;
        let required_items = esd.adena_cost;
        let sp = self.players.get(&id).map_or(0, |p| p.data.sp);
        if sp < i64::from(required_sp) {
            self.send(
                id,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughSpToEnchantThatSkill),
            );
            return;
        }

        let book_needed = self.settings.enchant.es_sp_book_needed;
        let spb = self.enchant_book_of(id, CHANGE_ENCHANT_BOOK);
        // does not have spellbook
        if book_needed && spb.is_none() {
            self.send(
                id,
                &SystemMessage::new(
                    SystemMessageId::YouDoNotHaveAllOfTheItemsNeededToEnchantSkillRouteChange,
                ),
            );
            return;
        }
        if self.adena_of(id) < i64::from(required_items) {
            self.send_not_enough_items(id);
            return;
        }

        let used_book = if book_needed { spb } else { None };
        if !self.take_enchant_costs(id, Some(i64::from(required_sp)), used_book, required_items) {
            self.send_not_enough_items(id);
            return;
        }

        let level_penalty = self.rng().below(current_enchant_level.min(4));
        skill_level -= level_penalty;
        if skill_level % 100 == 0 {
            skill_level = learn.base_level;
        }

        if let Some(skill) = self.enchant_skill_ref(skill_id, skill_level) {
            self.log_skill_enchant(id, "Route Change", &skill, spb, None);
            self.enchant_add_skill(id, &skill);
            self.send(id, &ExEnchantSkillResult { enchanted: true });
        }

        self.send_user_info(id);
        if level_penalty == 0 {
            self.send(
                id,
                &Self::skill_name_message(
                    SystemMessageId::EnchantSkillRouteChangeWasSuccessfulLvOfEnchantSkillS1WillRemain,
                    skill_id,
                ),
            );
        } else {
            self.send(
                id,
                &Self::skill_name_message(
                    SystemMessageId::EnchantSkillRouteChangeWasSuccessfulLvOfEnchantSkillS1HasBeenDecreasedByS2,
                    skill_id,
                )
                .with(SystemMessageParam::IntNumber(level_penalty)),
            );
        }

        // `new ExEnchantSkillInfoDetail(3, id, afterEnchantSkillLevel, player)`: the same level.
        self.finish_enchant_skill(id, skill_id, (TYPE_CHANGE_ENCHANT, 0));
    }
}
