//! Learning skills at a class trainer: the `SkillList` bypass, `AcquireSkillList`,
//! `RequestAcquireSkillInfo`, `RequestAcquireSkill`, the SP and item costs, `AutoLearnSkills`.
// Java: network/clientpackets/{RequestAcquireSkill,RequestAcquireSkillInfo}.java
// Java: network/serverpackets/{AcquireSkillList,AcquireSkillInfo,AcquireSkillDone}.java
// Java: data/xml/SkillTreeData.java (getAvailableSkills, getAllAvailableSkills, getMinLevelForNewSkill,
//       isSkillAllowed, getSkillLearn)
// Java: mechanics/skill/holders/SkillLearn.java (getCalculatedLevelUpSp)
// Java: entity/actor/instance/Folk.java (showSkillList), entity/actor/Npc.java (showNoTeachHtml)
// Java: entity/actor/templates/NpcTemplate.java (canTeach)
// Java: dist/game/data/scripts/handlers/bypass/npc/SkillList.java
// Java: entity/actor/Player.java (giveAvailableSkills, getLearningClass)
// Java: entity/actor/holders/player/ShortCuts.java (updateShortcuts)
//
// # The flow
// The trainer's page has `bypass -h npc_%objectId%_SkillList`; [`skill_list_handler`] (registered as
// `SkillList` in [`crate::bypass::BypassHandlers::default`]) shows `AcquireSkillList`. The client then
// asks for the details of a skill (`RequestAcquireSkillInfo`, answered with `AcquireSkillInfo`) and
// for the skill itself (`RequestAcquireSkill`). Learning checks the level, the SP, the books and
// takes them, adds the skill (a `character_skills` row, D-39: the same row Java writes), sends the
// new `SkillList` and shows the list again.
//
// # What is left out, with the system that brings it
// - Only the class tree (`AcquireSkillType.CLASS`) is learned here. The fishing, collect, transform,
//   transfer, sub-class, pledge and sub-pledge trees are not loaded (the card names them out of
//   scope), so those requests find no `SkillLearn` and are ignored, which is what Java does for a
//   skill that is not in a tree. Fishermen (`showFishSkillList`) and Tolonis (collect skills) are
//   therefore silent too.
// - `isTransformed()`/`isMounted()` (M6) and `PunishmentManager.handleIllegalPlayerAction` (only
//   logged; the punishment manager is not ported).
// - The script event `ON_PLAYER_SKILL_LEARN` (D-05).
// - `isSkillAllowed` knows the class tree only; the fishing, transform, collect, hero, GM and
//   transfer trees are not loaded. That cannot change what `AutoLearnSkills` learns (their skill ids
//   are never in a class tree), see [`World::is_skill_allowed`].
// - Stopping the effect of a toggle skill that is learned again (`giveAvailableSkills`) waits for
//   the effect engine.

use std::collections::BTreeMap;

use l2_data::skill_tree::DIVINE_INSPIRATION_ID;
use l2_data::{ClassId, SkillLearn};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{
    AcquireSkillDone, AcquireSkillInfo, AcquireSkillList, AcquireSkillRequirement, AcquireSkillType,
    ActionFailed, NpcHtmlMessage, ShortcutRegister, SystemMessage, SystemMessageParam,
};
use l2_util::ObjectId;
use tracing::{debug, info, warn};

use crate::rewards::exp::SkillWrites;
use crate::world::World;

/// Java `CommonSkill.ONYX_BEAST_TRANSFORMATION`.
const ONYX_BEAST_TRANSFORMATION_ID: i32 = 617;
/// `Folk.showSkillList` shows the collect skills of this NPC (Tolonis) instead of a class list.
const TOLONIS_NPC_ID: i32 = 32611;
/// `ShortcutType.SKILL` ordinal (NONE, ITEM, SKILL, ...).
const SHORTCUT_TYPE_SKILL: i32 = 2;
/// Java `Shortcut`'s bar width: the wire slot is `slot + page * 12`.
const SHORTCUTS_PER_PAGE: i32 = 12;

/// The `.ini` values skill learning reads. `Default` is the Java default of each key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LearnSettings {
    /// `PlayerConfig.ALT_GAME_SKILL_LEARN` (`AltGameSkillLearn`): a player may learn the skills of
    /// other classes of the trainer, for a higher SP price.
    pub alt_game_skill_learn: bool,
    /// `PlayerConfig.DIVINE_SP_BOOK_NEEDED` (`DivineInspirationSpBookNeeded`).
    pub divine_sp_book_needed: bool,
    /// `PlayerConfig.AUTO_LEARN_SKILLS_WITHOUT_ITEMS` (`AutoLearnSkillsWithoutItems`).
    pub auto_learn_skills_without_items: bool,
    /// `PlayerConfig.AUTO_LEARN_FS_SKILLS` (`AutoLearnForgottenScrollSkills`).
    pub auto_learn_fs_skills: bool,
    /// `PlayerConfig.AUTO_LEARN_DIVINE_INSPIRATION` (`AutoLearnDivineInspiration`).
    pub auto_learn_divine_inspiration: bool,
}

impl Default for LearnSettings {
    fn default() -> Self {
        Self {
            alt_game_skill_learn: false,
            divine_sp_book_needed: true,
            auto_learn_skills_without_items: false,
            auto_learn_fs_skills: false,
            auto_learn_divine_inspiration: false,
        }
    }
}

/// The flags of Java's `getAvailableSkills(player, class, includeByFs, includeAutoGet,
/// includeRequiredItems, existing)` that are not the class or the known skills.
#[derive(Debug, Clone, Copy)]
struct AvailableQuery {
    player_level: i32,
    is_gm: bool,
    auto_learn_divine_inspiration: bool,
    include_by_fs: bool,
    include_auto_get: bool,
    include_required_items: bool,
}

/// Java `SkillTreeData.getAvailableSkills(...)` on a tree in Java's iteration order: the lines
/// the player may learn next. `existing` are the skills the player knows (id to level).
fn available_in<'a>(
    tree: &[&'a SkillLearn],
    query: &AvailableQuery,
    existing: &BTreeMap<i32, i32>,
) -> Vec<&'a SkillLearn> {
    let mut result = Vec::new();
    for &skill in tree {
        if skill.skill_id == DIVINE_INSPIRATION_ID
            && !query.auto_learn_divine_inspiration
            && query.include_auto_get
            && !query.is_gm
        {
            continue;
        }
        if ((!query.include_auto_get || !skill.auto_get)
            && !skill.learned_by_npc
            && (!query.include_by_fs || !skill.learned_by_fs))
            || query.player_level < skill.get_level
        {
            continue;
        }
        // Forgotten Scroll requirements checked above.
        if !query.include_required_items && !skill.required_items.is_empty() && !skill.learned_by_fs {
            continue;
        }
        match existing.get(&skill.skill_id) {
            Some(&old_level) => {
                if old_level == skill.skill_level - 1 {
                    result.push(skill);
                }
            }
            None => {
                if skill.skill_level == 1 {
                    result.push(skill);
                }
            }
        }
    }
    result
}

/// Java `SkillTreeData.getMinLevelForNewSkill(player, skillTree)`: the lowest level above the
/// player's at which a trainer teaches something new, 0 if there is none.
fn min_level_for_new_skill(tree: &[&SkillLearn], player_level: i32) -> i32 {
    let mut min_level = 0;
    for skill in tree {
        if skill.learned_by_npc
            && player_level < skill.get_level
            && (min_level == 0 || min_level > skill.get_level)
        {
            min_level = skill.get_level;
        }
    }
    min_level
}

/// Java `SkillLearn.getCalculatedLevelUpSp(playerClass, learningClass)`.
fn calculated_level_up_sp(
    learn: &SkillLearn,
    player_class: ClassId,
    learning_class: ClassId,
    alt_game_skill_learn: bool,
) -> i32 {
    let mut level_up_sp = learn.level_up_sp;
    // If the alternative skill learn system is enabled and the player is learning a skill from a
    // different class apply a fee.
    if alt_game_skill_learn && player_class != learning_class {
        // If the player is learning a skill from other class type (mage learning warrior skills or
        // vice versa) the fee is higher.
        if player_class.is_mage() != learning_class.is_mage() {
            level_up_sp = level_up_sp.wrapping_mul(3);
        } else {
            level_up_sp = level_up_sp.wrapping_mul(2);
        }
    }
    level_up_sp
}

impl World {
    // ------------------------------------------------------------------ small readers

    /// The class of a player as an id (Java `getPlayerClass()`).
    fn player_class_of(&self, id: ObjectId) -> Option<ClassId> {
        let player = self.players.get(&id)?;
        u16::try_from(player.data.class_id).ok().map(ClassId)
    }

    /// Java `getLearningClass()`.
    fn learning_class_of(&self, id: ObjectId) -> Option<ClassId> {
        let player = self.players.get(&id)?;
        player.talk.learning_class.or_else(|| self.player_class_of(id))
    }

    /// The skills a player knows, id to level (Java `getSkills()`).
    fn known_skills_of(&self, id: ObjectId) -> BTreeMap<i32, i32> {
        self.players
            .get(&id)
            .map(|p| p.core.skills.iter().copied().collect())
            .unwrap_or_default()
    }

    /// Java `Player.getSkillLevel(skillId)`: 0 when the skill is not known.
    fn known_skill_level(&self, id: ObjectId, skill_id: i32) -> i32 {
        self.players
            .get(&id)
            .and_then(|p| p.core.skills.iter().find(|(s, _)| *s == skill_id))
            .map_or(0, |&(_, level)| level)
    }

    /// `SkillData.getSkill(id, level) != null`.
    fn skill_template_exists(&self, skill_id: i32, level: i32) -> bool {
        self.data
            .stat_data()
            .is_some_and(|d| d.skills.get_skill(skill_id, level).is_some())
    }

    /// Java `NpcTemplate.getTeachInfo()` of the NPC with this template id.
    fn teach_info(&self, npc_template_id: i32) -> &[ClassId] {
        self.talk
            .skill_learn
            .as_deref()
            .and_then(|data| data.classes_of(npc_template_id))
            .unwrap_or(&[])
    }

    /// Java `NpcTemplate.canTeach(playerClass)`.
    fn can_teach(&self, npc_template_id: i32, class: ClassId) -> bool {
        // If the player is on a third class, fetch the class teacher information for its parent
        // class.
        let class = if class.class_level() == 3 {
            // A third class always has a parent; `None` can only come from an unknown id.
            match class.enum_parent() {
                Some(parent) => parent,
                None => return false,
            }
        } else {
            class
        };
        self.teach_info(npc_template_id).contains(&class)
    }

    fn npc_template_id(&self, npc: ObjectId) -> Option<i32> {
        self.npcs.get(&npc).map(|n| n.npc_id)
    }

    fn npc_type_of(&self, npc: ObjectId) -> Option<String> {
        let template_id = self.npc_template_id(npc)?;
        self.npc_world.template(template_id).map(|t| t.npc_type.clone())
    }

    /// The trainer a request is about: `getLastFolkNPC()`, an NPC the player can still talk to (or
    /// any distance for a GM).
    fn trainer_of(&self, player: ObjectId) -> Option<ObjectId> {
        let trainer = self.players.get(&player)?.talk.last_folk_npc?;
        if !self.npcs.contains_key(&trainer) {
            return None;
        }
        (self.can_interact(trainer, player) || self.is_gm(player)).then_some(trainer)
    }

    // ------------------------------------------------------------------ the trees

    fn available_query(
        &self,
        player: ObjectId,
        include_by_fs: bool,
        include_auto_get: bool,
        include_required_items: bool,
    ) -> AvailableQuery {
        AvailableQuery {
            player_level: self.core_of(player).map_or(1, |c| c.level),
            is_gm: self.is_gm(player),
            auto_learn_divine_inspiration: self.settings.learn.auto_learn_divine_inspiration,
            include_by_fs,
            include_auto_get,
            include_required_items,
        }
    }

    /// Java `SkillTreeData.getAvailableSkills(player, playerClass, includeByFs, includeAutoGet,
    /// includeRequiredItems, existingSkills)`, in Java's order.
    fn available_skills_for(
        &self,
        player: ObjectId,
        class: ClassId,
        include_by_fs: bool,
        include_auto_get: bool,
        include_required_items: bool,
        existing: &BTreeMap<i32, i32>,
    ) -> Vec<SkillLearn> {
        let trees = self.rewards.skill_trees.clone();
        let tree = trees.complete_class_tree_java_order(class);
        if tree.is_empty() {
            // The Skill Tree for this class is undefined.
            warn!(
                "SkillTreeData: Skilltree for class {} is not defined!",
                class.enum_name().unwrap_or("?")
            );
            return Vec::new();
        }
        let query = self.available_query(player, include_by_fs, include_auto_get, include_required_items);
        available_in(&tree, &query, existing)
            .into_iter()
            .cloned()
            .collect()
    }

    /// Java `SkillTreeData.isSkillAllowed(player, skill)`, for the trees that are loaded: a skill
    /// excluded from the check, or a line of the class tree of the player's class (the level
    /// counts as at most the skill's highest level).
    fn is_skill_allowed(&self, player: ObjectId, skill_id: i32, level: i32) -> bool {
        let Some(data) = self.data.stat_data() else {
            return false;
        };
        if data
            .skills
            .get(skill_id, level)
            .is_some_and(|skill| skill.excluded_from_check)
        {
            return true;
        }
        // `player.isGM() && skill.isGMSkill()`: the GM trees are not loaded.
        let Some(class) = self.player_class_of(player) else {
            return false;
        };
        let max_level = data.skills.max_level(skill_id);
        self.rewards
            .skill_trees
            .class_skill(class, skill_id, level.min(max_level))
            .is_some()
    }

    /// Java `SkillTreeData.getAllAvailableSkills(player, playerClass, includeByFs, includeAutoGet,
    /// includeRequiredItems)`: the skills the player knows that the trees allow, plus every skill
    /// the trees would let him learn one level after the other, as skill id to level.
    fn all_available_skills(
        &self,
        player: ObjectId,
        class: ClassId,
        include_by_fs: bool,
        include_auto_get: bool,
        include_required_items: bool,
    ) -> BTreeMap<i32, i32> {
        let mut result = BTreeMap::new();
        for (skill_id, level) in self.known_skills_of(player) {
            // Adding only skills that can be learned by the player.
            if self.is_skill_allowed(player, skill_id, level) {
                result.insert(skill_id, level);
            }
        }

        // Keep learning skills until no more are available.
        let mut previous: Option<Vec<(i32, i32)>> = None;
        loop {
            let learnable = self.available_skills_for(
                player,
                class,
                include_by_fs,
                include_auto_get,
                include_required_items,
                &result,
            );
            if learnable.is_empty() {
                break;
            }
            let keys: Vec<(i32, i32)> = learnable.iter().map(|s| (s.skill_id, s.skill_level)).collect();
            // Break if no new skills became learnable since last iteration.
            if previous.as_ref() == Some(&keys) {
                break;
            }
            for skill in &learnable {
                // NOTE(java-quirk): a tree line whose skill does not exist makes Java throw a
                // NullPointerException out of `rewardSkills`; here the line is skipped.
                if self.skill_template_exists(skill.skill_id, skill.skill_level) {
                    result.insert(skill.skill_id, skill.skill_level);
                } else {
                    warn!(
                        "SkillTreeData: skill {} level {} of the tree does not exist.",
                        skill.skill_id, skill.skill_level
                    );
                }
            }
            // Store reference for convergence check.
            previous = Some(keys);
        }
        result
    }

    // ------------------------------------------------------------------ AutoLearnSkills

    /// Java `Player.giveAvailableSkills(includeByFs, includeAutoGet, includeRequiredItems)`: every
    /// skill the trees offer, learned at once (`AutoLearnSkills = True`). Returns how many skills
    /// were new to the player.
    pub(crate) fn give_available_skills(
        &mut self,
        id: ObjectId,
        include_by_fs: bool,
        include_auto_get: bool,
        include_required_items: bool,
        writes: &mut SkillWrites,
    ) -> i32 {
        let Some(class) = self.player_class_of(id) else {
            return 0;
        };
        let mut skill_counter = 0;
        // Get available skills. Java walks a `HashMap` by skill id; here it is the id order.
        let skills =
            self.all_available_skills(id, class, include_by_fs, include_auto_get, include_required_items);
        for (skill_id, level) in skills {
            let known = self.known_skill_level(id, skill_id);
            if known == level {
                continue;
            }
            if known == 0 {
                skill_counter += 1;
            }
            // FOLLOW-UP(effects): "Fix when learning toggle skills": a toggle that is on is stopped.
            if !self.learn_skill(id, skill_id, level, writes) {
                warn!("Skipping null skill {skill_id}/{level} of the tree for {id}");
                continue;
            }
            // `PlayerConfig.AUTO_LEARN_SKILLS` is true here.
            self.update_shortcuts(id, skill_id, level);
        }

        if skill_counter > 0 {
            // `storeSkills` runs before the message in Java; the caller flushes `writes` right after.
            if let Some(player) = self.players.get(&id) {
                let packet = crate::enter::shortcut_init(&player.data, &*self.data);
                self.send(id, &packet);
            }
            self.send_message(id, &format!("You have learned {skill_counter} new skills."));
        }
        skill_counter
    }

    /// Java `Shortcuts.updateShortcuts(skillId, skillLevel)`: every skill shortcut of the skill
    /// shows the new level.
    pub(crate) fn update_shortcuts(&mut self, id: ObjectId, skill_id: i32, level: i32) {
        let Some(player) = self.players.get_mut(&id) else {
            return;
        };
        let mut slots = Vec::new();
        for shortcut in &mut player.data.shortcuts {
            if shortcut.kind == SHORTCUT_TYPE_SKILL && shortcut.shortcut_id == skill_id {
                shortcut.level = level;
                slots.push(shortcut.slot + shortcut.page * SHORTCUTS_PER_PAGE);
            }
        }
        // Java walks a map keyed by the wire slot.
        slots.sort_unstable();
        for slot in slots {
            self.send(
                id,
                &ShortcutRegister::Skill {
                    slot,
                    id: skill_id,
                    level,
                    character_type: 1,
                },
            );
        }
    }

    // ------------------------------------------------------------------ Folk.showSkillList

    /// Java `Npc.showNoTeachHtml(player)`.
    fn show_no_teach_html(&mut self, npc: ObjectId, player: ObjectId) {
        let Some(npc_id) = self.npc_template_id(npc) else {
            return;
        };
        let npc_type = self.npc_type_of(npc).unwrap_or_default();
        // NOTE(java-quirk): Java starts with an empty page, so an NPC that is neither a warehouse
        // keeper nor a trainer answers with an empty page.
        let mut html = Some(String::new());
        if npc_type == "Warehouse" {
            html = self.get_htm(player, &format!("data/html/warehouse/{npc_id}-noteach.htm"));
        } else if npc_type == "Trainer" {
            html = self.get_htm(player, &format!("data/html/trainer/{npc_id}-noteach.htm"));
            // Trainer Healer?
            if html.is_none() {
                html = self.get_htm(
                    player,
                    &format!("data/scripts/ai/others/HealerTrainer/{npc_id}-noteach.html"),
                );
            }
        }
        let text = match html {
            None => {
                warn!("Npc {npc_id} missing noTeach html!");
                "<html><body>I cannot teach you any skills.<br>You must find your current class teachers.</body></html>"
                    .to_owned()
            }
            Some(text) => text.replace("%objectId%", &npc.0.to_string()),
        };
        if let Some(message) = NpcHtmlMessage::new(npc.0, &text) {
            self.send_html(player, &message);
        }
    }

    /// Java `Folk.showSkillList(player, npc, playerClass)`: the skills of `class` the player may
    /// learn from this trainer now, or the reason there are none.
    pub(crate) fn show_skill_list(&mut self, player: ObjectId, npc: ObjectId, class: ClassId) {
        let Some(npc_id) = self.npc_template_id(npc) else {
            return;
        };
        if npc_id == TOLONIS_NPC_ID {
            // FOLLOW-UP: the collect skill tree (Gracia) is not loaded.
            debug!("{player}: the collect skills of Tolonis are not ported");
            return;
        }

        if !self.can_teach(npc_id, class) {
            self.show_no_teach_html(npc, player);
            return;
        }

        if self.teach_info(npc_id).is_empty() {
            let class_id = self.player_class_of(player).map_or(-1, |c| i32::from(c.0));
            let text = format!(
                "<html><body>I cannot teach you. My class list is empty.<br>Ask admin to fix it. Need add my npcid and classes to skill_learn.sql.<br>NpcId:{npc_id}, Your playerClass:{class_id}</body></html>"
            );
            if let Some(message) = NpcHtmlMessage::new(npc.0, &text) {
                self.send_html(player, &message);
            }
            return;
        }

        // Normal skills, No LearnedByFS, no AutoGet skills.
        let existing = self.known_skills_of(player);
        let skills = self.available_skills_for(player, class, false, false, true, &existing);
        let Some(player_class) = self.player_class_of(player) else {
            return;
        };
        let alt = self.settings.learn.alt_game_skill_learn;
        let mut list = AcquireSkillList::new(AcquireSkillType::Class);
        let mut count = 0;
        if let Some(p) = self.players.get_mut(&player) {
            p.talk.learning_class = Some(class);
        }
        for s in &skills {
            if self.skill_template_exists(s.skill_id, s.skill_level) {
                list.add_skill(
                    s.skill_id,
                    s.skill_level,
                    s.skill_level,
                    calculated_level_up_sp(s, player_class, class, alt),
                    0,
                );
                count += 1;
            }
        }

        if count == 0 {
            let level = self.core_of(player).map_or(1, |c| c.level);
            let trees = self.rewards.skill_trees.clone();
            let tree = trees.complete_class_tree(class).collect::<Vec<_>>();
            if tree.is_empty() {
                warn!("SkillTreeData: SkillTree is not defined for getMinLevelForNewSkill!");
            }
            let min_level = min_level_for_new_skill(&tree, level);
            if min_level > 0 {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::YouDoNotHaveAnyFurtherSkillsToLearnComeBackWhenYouHaveReachedLevelS1,
                    )
                    .with(SystemMessageParam::IntNumber(min_level)),
                );
            } else if player_class.class_level() == 1 {
                self.send(
                    player,
                    &SystemMessage::new(
                        SystemMessageId::ThereAreNoOtherSkillsToLearnPleaseComeBackAfterS1ndClassChange,
                    )
                    .with(SystemMessageParam::IntNumber(2)),
                );
            } else {
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::ThereAreNoOtherSkillsToLearn),
                );
            }
        } else {
            self.send(player, &list);
        }
    }

    /// `RequestAcquireSkill.showSkillList(trainer, player)`: the list again, after a failed or a
    /// successful lesson.
    fn show_skill_list_again(&mut self, player: ObjectId, trainer: ObjectId, skill_type: AcquireSkillType) {
        if matches!(
            skill_type,
            AcquireSkillType::Transform | AcquireSkillType::Subclass | AcquireSkillType::Transfer
        ) {
            // Managed in Datapack.
            return;
        }
        if self.npc_type_of(trainer).as_deref() == Some("Fisherman") {
            // FOLLOW-UP: `Fisherman.showFishSkillList` (the fishing tree is not loaded).
            return;
        }
        if let Some(class) = self.learning_class_of(player) {
            self.show_skill_list(player, trainer, class);
        }
    }

    // ------------------------------------------------------------------ the requests

    /// Java `RequestAcquireSkillInfo.runImpl`.
    pub(crate) fn on_request_acquire_skill_info(
        &mut self,
        player: ObjectId,
        skill_id: i32,
        level: i32,
        skill_type: AcquireSkillType,
    ) {
        if skill_id <= 0 || level <= 0 {
            warn!("RequestAcquireSkillInfo: Invalid Id: {skill_id} or level: {level}!");
            return;
        }
        if !self.players.contains_key(&player) {
            return;
        }
        let Some(trainer) = self.trainer_of(player) else {
            return;
        };
        if !self.skill_template_exists(skill_id, level) {
            warn!(
                "RequestAcquireSkillInfo: Skill Id: {skill_id} level: {level} is undefined. RequestAcquireSkillInfo failed."
            );
            return;
        }
        let Some(learning_class) = self.learning_class_of(player) else {
            return;
        };
        // Only the class tree is loaded: `getSkillLearn` finds nothing for the other types.
        if skill_type != AcquireSkillType::Class {
            debug!("{player}: RequestAcquireSkillInfo of {skill_type:?} is not ported");
            return;
        }
        let Some(learn) = self
            .rewards
            .skill_trees
            .class_skill(learning_class, skill_id, level)
            .cloned()
        else {
            return;
        };
        let Some(npc_id) = self.npc_template_id(trainer) else {
            return;
        };
        if !self.can_teach(npc_id, learning_class) {
            return;
        }
        let Some(player_class) = self.player_class_of(player) else {
            return;
        };
        let custom_sp = calculated_level_up_sp(
            &learn,
            player_class,
            learning_class,
            self.settings.learn.alt_game_skill_learn,
        );
        // The `AcquireSkillInfo(type, learn, sp)` constructor lists every required item.
        let requirements = learn
            .required_items
            .iter()
            .map(|item| AcquireSkillRequirement::item(item.item_id, item.count))
            .collect();
        self.send(
            player,
            &AcquireSkillInfo {
                skill_type,
                skill_id: learn.skill_id,
                level: learn.skill_level,
                sp_cost: custom_sp,
                requirements,
            },
        );
    }

    /// Java `RequestAcquireSkill.runImpl`.
    pub(crate) fn on_request_acquire_skill(
        &mut self,
        player: ObjectId,
        skill_id: i32,
        level: i32,
        skill_type: AcquireSkillType,
    ) {
        if !self.players.contains_key(&player) {
            return;
        }
        // FOLLOW-UP(M6): `isTransformed() || isMounted()` answers with the skill enhancing
        // message.

        if !(1..=1000).contains(&level) || skill_id < 1 {
            // Java: `PunishmentManager.handleIllegalPlayerAction(player, ..., DEFAULT_PUNISH)`.
            warn!("Recived Wrong Packet Data in Aquired Skill - id: {skill_id} level: {level} for {player}");
            return;
        }

        let Some(trainer) = self.trainer_of(player) else {
            return;
        };

        if !self.skill_template_exists(skill_id, level) {
            warn!(
                "RequestAcquireSkill: {player} is trying to learn a null skill Id: {skill_id} level: {level}!"
            );
            return;
        }

        // Hack check. Doesn't apply to all Skill Types
        let prev_skill_level = self.known_skill_level(player, skill_id);
        if prev_skill_level > 0
            && !matches!(
                skill_type,
                AcquireSkillType::Transfer | AcquireSkillType::Subpledge
            )
        {
            if prev_skill_level == level {
                return;
            } else if prev_skill_level != level - 1 {
                // The previous level skill has not been learned.
                self.send(
                    player,
                    &SystemMessage::new(SystemMessageId::ThePreviousLevelSkillHasNotBeenLearned),
                );
                warn!(
                    "{player} is requesting skill Id: {skill_id} level {level} without knowing it's previous level!"
                );
                return;
            }
        }

        // Only the class tree is loaded: `getSkillLearn` finds nothing for the other types.
        if skill_type != AcquireSkillType::Class {
            debug!("{player}: RequestAcquireSkill of {skill_type:?} is not ported");
            return;
        }
        let Some(learning_class) = self.learning_class_of(player) else {
            return;
        };
        let Some(learn) = self
            .rewards
            .skill_trees
            .class_skill(learning_class, skill_id, level)
            .cloned()
        else {
            return;
        };

        if self.check_player_skill(player, trainer, &learn, skill_type) {
            self.give_skill(player, trainer, skill_id, level, skill_type);
        }
    }

    /// Java `RequestAcquireSkill.checkPlayerSkill(player, trainer, skillLearn)`: takes the SP and
    /// the items when everything is in order.
    fn check_player_skill(
        &mut self,
        player: ObjectId,
        trainer: ObjectId,
        learn: &SkillLearn,
        skill_type: AcquireSkillType,
    ) -> bool {
        let (Some(player_level), Some(player_class), Some(learning_class)) = (
            self.core_of(player).map(|c| c.level),
            self.player_class_of(player),
            self.learning_class_of(player),
        ) else {
            return false;
        };

        // Hack check.
        if learn.get_level > player_level {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotMeetTheSkillLevelRequirements),
            );
            warn!(
                "{player}, level {player_level} is requesting skill Id: {} level {} without having minimum required level, {}!",
                learn.skill_id, learn.skill_level, learn.get_level
            );
            return false;
        }

        // First it checks that the skill require SP and the player has enough SP to learn it.
        let level_up_sp = calculated_level_up_sp(
            learn,
            player_class,
            learning_class,
            self.settings.learn.alt_game_skill_learn,
        );
        let sp = self.players.get(&player).map_or(0, |p| p.data.sp);
        if level_up_sp > 0 && i64::from(level_up_sp) > sp {
            self.send(
                player,
                &SystemMessage::new(SystemMessageId::YouDoNotHaveEnoughSpToLearnThisSkill),
            );
            self.show_skill_list_again(player, trainer, skill_type);
            return false;
        }

        // NOTE(java-quirk): without `DivineInspirationSpBookNeeded` the skill is free: Java returns
        // here, before it takes the SP, the prerequisites and the items.
        if !self.settings.learn.divine_sp_book_needed && learn.skill_id == DIVINE_INSPIRATION_ID {
            return true;
        }

        // Check for required skills.
        for &(pre_id, pre_level) in &learn.pre_req_skills {
            if self.known_skill_level(player, pre_id) < pre_level {
                let message = if pre_id == ONYX_BEAST_TRANSFORMATION_ID {
                    SystemMessageId::YouMustLearnTheOnyxBeastSkillBeforeYouCanAcquireFurtherSkills
                } else {
                    SystemMessageId::YouDoNotHaveTheNecessaryMaterialsOrPrerequisitesToLearnThisSkill
                };
                self.send(player, &SystemMessage::new(message));
                return false;
            }
        }

        // Check for required items.
        if !learn.required_items.is_empty() {
            // Then checks that the player has all the items.
            for item in &learn.required_items {
                if self.inventory_item_count(player, item.item_id) < item.count {
                    // Player doesn't have required item.
                    self.send(
                        player,
                        &SystemMessage::new(
                            SystemMessageId::YouDoNotHaveTheNecessaryMaterialsOrPrerequisitesToLearnThisSkill,
                        ),
                    );
                    self.show_skill_list_again(player, trainer, skill_type);
                    return false;
                }
            }

            // If the player has all required items, they are consumed.
            for item in &learn.required_items {
                if !self.destroy_item_by_item_id(player, item.item_id, item.count, true) {
                    // Java: handleIllegalPlayerAction(..., NONE).
                    warn!(
                        "Somehow {player}, level {player_level} lose required item Id: {} to learn skill while learning skill Id: {} level {}!",
                        item.item_id, learn.skill_id, learn.skill_level
                    );
                }
            }
        }

        // If the player has SP and all required items then consume SP.
        if level_up_sp > 0 {
            if let Some(p) = self.players.get_mut(&player) {
                // `setSp(Math.max(0, sp - levelUpSp))`.
                p.data.sp = (p.data.sp - i64::from(level_up_sp)).max(0);
            }
            self.send_user_info(player);
        }
        true
    }

    /// Java `RequestAcquireSkill.giveSkill(player, trainer, skill)`.
    fn give_skill(
        &mut self,
        player: ObjectId,
        trainer: ObjectId,
        skill_id: i32,
        level: i32,
        skill_type: AcquireSkillType,
    ) {
        // Send message.
        self.send(
            player,
            &SystemMessage::new(SystemMessageId::YouHaveEarnedS12)
                .with(SystemMessageParam::SkillName { id: skill_id, level }),
        );
        self.send(player, &AcquireSkillDone);

        // `addSkill(skill, true)`: the new level replaces the old one, the row is written.
        let mut writes = SkillWrites::default();
        if !self.learn_skill(player, skill_id, level, &mut writes) {
            warn!("{player} could not add skill {skill_id}/{level}");
        }
        self.flush_skill_writes(player, writes);
        self.send_skill_list(player);

        self.update_shortcuts(player, skill_id, level);
        self.show_skill_list_again(player, trainer, skill_type);

        // If skill is expand type then sends packet:
        if (1368..=1372).contains(&skill_id) {
            self.send_storage_max_count(player);
        }
        // `ON_PLAYER_SKILL_LEARN`: script event (D-05).
        info!("{player} learned skill {skill_id} level {level}");
    }

    // ------------------------------------------------------------------ the SkillList bypass

    /// Java `SkillList.onCommand` (the bypass handler script), for `target` an NPC.
    fn skill_list_bypass(&mut self, player: ObjectId, command: &str, npc: ObjectId) {
        let Some(player_class) = self.player_class_of(player) else {
            return;
        };
        if !self.settings.learn.alt_game_skill_learn {
            self.show_skill_list(player, npc, player_class);
            return;
        }

        // `command.substring(9).trim()`
        let id_text = command.get(9..).unwrap_or("").trim_matches(|c: char| c <= ' ');
        if !id_text.is_empty() {
            // The handler catches everything Java throws here (a bad number, an unknown class).
            match id_text.parse::<i32>() {
                Ok(id) => match u16::try_from(id).map(ClassId) {
                    Ok(class) if class.is_player_class() => self.show_skill_list(player, npc, class),
                    _ => warn!("Exception in SkillList: no player class {id}"),
                },
                Err(_) => warn!("Exception in SkillList: bad class id {id_text:?}"),
            }
            return;
        }

        let Some(npc_id) = self.npc_template_id(npc) else {
            return;
        };
        let classes_to_teach = self.teach_info(npc_id).to_vec();
        let mut own_class = false;
        for class in &classes_to_teach {
            if class.equals_or_child_of(player_class) {
                own_class = true;
                break;
            }
        }

        let mut text = String::from("<html><body><center>Skill learning:</center><br>");
        if !own_class {
            let char_type = if player_class.is_mage() { "fighter" } else { "mage" };
            text.push_str("Skills of your class are the easiest to learn.<br>Skills of another class of your race are a little harder.<br>Skills for classes of another race are extremely difficult.<br>But the hardest of all to learn are the  ");
            text.push_str(char_type);
            text.push_str("skills!<br>");
        }

        // make a list of classes
        if classes_to_teach.is_empty() {
            text.push_str("No Skills.<br>");
        } else {
            let mut count = 0;
            let mut class_check = Some(player_class);
            let existing = self.known_skills_of(player);
            while count == 0 {
                let Some(check) = class_check else {
                    break;
                };
                for &class in &classes_to_teach {
                    if class.class_level() > check.class_level() {
                        continue;
                    }
                    if self
                        .available_skills_for(player, class, false, false, true, &existing)
                        .is_empty()
                    {
                        continue;
                    }
                    text.push_str(&format!(
                        "<a action=\"bypass -h npc_%objectId%_SkillList {}\">Learn {}'s class Skills</a><br>\n",
                        class.0,
                        class.enum_name().unwrap_or("?")
                    ));
                    count += 1;
                }
                class_check = check.enum_parent();
            }
        }
        text.push_str("</body></html>");

        let text = text.replace("%objectId%", &npc.0.to_string());
        if let Some(message) = NpcHtmlMessage::new(npc.0, &text) {
            self.send_html(player, &message);
        }
        self.send(player, &ActionFailed);
    }
}

/// Java `SkillList` (bypass handler script): the `SkillList` command of a trainer's page.
pub(crate) fn skill_list_handler(
    world: &mut World,
    player: ObjectId,
    command: &str,
    target: Option<ObjectId>,
) -> bool {
    let Some(npc) = target.filter(|t| world.npcs.contains_key(t)) else {
        return false;
    };
    world.skill_list_bypass(player, command, npc);
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use l2_data::SkillTrees;

    const TREE: &str = r#"<list><skillTree type="classSkillTree" classId="0">
        <skill skillId="3" skillLevel="1" getLevel="5" levelUpSp="100" learnedByNpc="true" />
        <skill skillId="3" skillLevel="2" getLevel="9" levelUpSp="200" learnedByNpc="true" />
        <skill skillId="4" skillLevel="1" getLevel="5" autoGet="true" />
        <skill skillId="5" skillLevel="1" getLevel="5" levelUpSp="10" learnedByFS="true" />
        <skill skillId="6" skillLevel="1" getLevel="5" levelUpSp="10" learnedByNpc="true"><item id="57" count="1" /></skill>
        <skill skillId="1405" skillLevel="1" getLevel="5" levelUpSp="10" learnedByNpc="true" />
    </skillTree></list>"#;

    fn query() -> AvailableQuery {
        AvailableQuery {
            player_level: 9,
            is_gm: false,
            auto_learn_divine_inspiration: false,
            include_by_fs: false,
            include_auto_get: false,
            include_required_items: true,
        }
    }

    fn ids(lines: &[&SkillLearn]) -> Vec<(i32, i32)> {
        let mut ids: Vec<_> = lines.iter().map(|l| (l.skill_id, l.skill_level)).collect();
        ids.sort_unstable();
        ids
    }

    #[test]
    fn available_lines_follow_java_flags() {
        let trees = SkillTrees::from_xml(TREE).unwrap();
        let tree = trees.complete_class_tree_java_order(ClassId(0));
        let none = BTreeMap::new();
        // A trainer list: NPC lines only (auto-get and Forgotten Scroll lines are left out).
        assert_eq!(
            ids(&available_in(&tree, &query(), &none)),
            vec![(3, 1), (6, 1), (1405, 1)]
        );
        // Auto-learn: auto-get lines join, Divine Inspiration goes unless the setting or a GM.
        let auto = AvailableQuery {
            include_auto_get: true,
            ..query()
        };
        assert_eq!(
            ids(&available_in(&tree, &auto, &none)),
            vec![(3, 1), (4, 1), (6, 1)]
        );
        let auto_gm = AvailableQuery { is_gm: true, ..auto };
        assert_eq!(
            ids(&available_in(&tree, &auto_gm, &none)),
            vec![(3, 1), (4, 1), (6, 1), (1405, 1)]
        );
        // Forgotten Scroll lines with the setting; lines with items only when asked for.
        let fs = AvailableQuery {
            include_by_fs: true,
            include_required_items: false,
            ..query()
        };
        assert_eq!(
            ids(&available_in(&tree, &fs, &none)),
            vec![(3, 1), (5, 1), (1405, 1)]
        );
        // A known skill offers its next level, at the player's level.
        let known = BTreeMap::from([(3, 1)]);
        assert_eq!(
            ids(&available_in(&tree, &query(), &known)),
            vec![(3, 2), (6, 1), (1405, 1)]
        );
        let low = AvailableQuery {
            player_level: 8,
            ..query()
        };
        assert_eq!(ids(&available_in(&tree, &low, &known)), vec![(6, 1), (1405, 1)]);
    }

    #[test]
    fn min_level_is_the_lowest_level_above_the_players() {
        let trees = SkillTrees::from_xml(TREE).unwrap();
        let tree: Vec<_> = trees.complete_class_tree(ClassId(0)).collect();
        assert_eq!(min_level_for_new_skill(&tree, 1), 5);
        assert_eq!(min_level_for_new_skill(&tree, 5), 9);
        assert_eq!(min_level_for_new_skill(&tree, 9), 0);
        assert_eq!(min_level_for_new_skill(&[], 1), 0);
    }

    #[test]
    fn other_class_skills_cost_double_or_triple() {
        let trees = SkillTrees::from_xml(TREE).unwrap();
        let line = trees.class_skill(ClassId(0), 3, 2).unwrap();
        let (fighter, warrior, mage) = (ClassId(0), ClassId(1), ClassId(10));
        // The setting off, or the own class: the price in the tree.
        assert_eq!(calculated_level_up_sp(line, fighter, mage, false), 200);
        assert_eq!(calculated_level_up_sp(line, fighter, fighter, true), 200);
        // Another class of the same kind: x2. The other kind (a fighter learning mage skills): x3.
        assert_eq!(calculated_level_up_sp(line, fighter, warrior, true), 400);
        assert_eq!(calculated_level_up_sp(line, fighter, mage, true), 600);
        assert_eq!(calculated_level_up_sp(line, mage, fighter, true), 600);
    }
}
