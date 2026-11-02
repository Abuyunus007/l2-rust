//! `Rebirth.ini`: a character at the minimum level pays an item and starts again at level 1 and
//! its base class; every rebirth hands out tokens that buy a few skills from a configured pool.
// Java: managers/RebirthManager.java, config/custom/RebirthConfig.java,
// Java: dist/game/data/scripts/handlers/bypass/npc/Rebirth.java, entity/actor/Player.java
// Java: (`rewardSkills` and `regiveTemporarySkills` call `grantRebirthSkills`)
//
// # How it differs from Java (D-15, D-39)
// - Java reads `rebirth_system` with a blocking query every time it needs the count or the
//   skills. The session reads the row once when the character loads (`PlayerProfile::rebirth`);
//   the world keeps it up to date and writes every change as a [`RebirthWrite`] through the
//   outbox, in the statements Java uses. The row belongs to one character, so it is written even
//   side by side with a Java server (D-39).
// - Java aborts the rebirth when the first statement fails ("could not be completed due to a
//   database error"). The write is asynchronous here: a failure is only logged by the
//   persistence task.
// - Java's two "operation in progress" guards cannot be hit: the world runs one command at a time.

use std::collections::{BTreeSet, HashMap};

use l2_data::ClassId;
use l2_protocol_game::server::{ExShowScreenMessage, MagicSkillUse, SkillPosition, SocialAction};
use l2_util::ObjectId;
use tracing::warn;

use crate::bypass::BypassHandlers;
use crate::rewards::exp::SkillWrites;
use crate::world::World;

const SKILLS_PER_ROW: i32 = 3;
const SKILL_ICON_PADDING_THRESHOLD: i32 = 1000;

/// `RebirthConfig` (`Custom/Rebirth.ini`). `Default` is the Java default of each key (Java's code
/// defaults, which differ from the shipped `.ini`: `RebirthAllow = false` there).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RebirthSettings {
    /// `REBIRTH_ALLOW_REBIRTH` (`RebirthAllow`).
    pub allow_rebirth: bool,
    /// `REBIRTH_MIN_LEVEL` (`RebirthMin`).
    pub min_level: i32,
    /// `REBIRTH_MAX_COUNT` (`RebirthMaxCount`).
    pub max_count: i32,
    /// `REBIRTH_INHERIT_SKILLS_TO_SUBCLASSES` (`RebirthInheritSkillsToSubclasses`).
    pub inherit_skills_to_subclasses: bool,
    /// `REBIRTH_DELETE_ALL_SKILLS` (`RebirthDeleteAllSkills`).
    pub delete_all_skills: bool,
    /// `REBIRTH_SKILL_DELETE_WHITELIST` (`RebirthSkillDeleteWhitelist`).
    pub skill_delete_whitelist: BTreeSet<i32>,
    /// `REBIRTH_DELETE_PROTECTED_SKILL_GROUPS` (`RebirthDeleteProtectedSkillGroups`).
    pub delete_protected_skill_groups: BTreeSet<String>,
    /// `REBIRTH_DELETE_RESTORE_TEMPORARY_SKILLS` (`RebirthDeleteRestoreTemporarySkills`).
    pub delete_restore_temporary_skills: bool,
    /// `REBIRTH_ITEM_ID` (`RebirthItemId`).
    pub item_id: i32,
    /// `REBIRTH_ITEM_AMOUNT` (`RebirthItemAmount`).
    pub item_amount: i32,
    /// `REBIRTH_MAX_SELECTED_SKILLS` (`RebirthMaxSelectedSkills`).
    pub max_selected_skills: i32,
    /// `REBIRTH_REWARD_ITEM_ID` (`RebirthRewardItemId`).
    pub reward_item_id: i32,
    /// `REBIRTH_REWARD_ITEM_AMOUNT` (`RebirthRewardItemAmount`).
    pub reward_item_amount: i32,
    /// `REBIRTH_SKILL_REFUND_MODE` (`RebirthSkillRefundMode`, trimmed and upper case).
    pub skill_refund_mode: String,
    /// `REBIRTH_MAGE_SKILLS` (`RebirthMageSkills`): `id,level,cost` entries.
    pub mage_skills: Vec<String>,
    /// `REBIRTH_FIGHTER_SKILLS` (`RebirthFighterSkills`).
    pub fighter_skills: Vec<String>,
    /// `REBIRTH_FINISH_SOCIAL_ID` (`RebirthFinishSocialId`).
    pub finish_social_id: i32,
    /// `REBIRTH_FINISH_EFFECT_SKILL_ID` (`RebirthFinishEffectSkillId`).
    pub finish_effect_skill_id: i32,
    /// `REBIRTH_FINISH_EFFECT_SKILL_LVL` (`RebirthFinishEffectSkillLevel`).
    pub finish_effect_skill_lvl: i32,
    /// `REBIRTH_SELECT_SOCIAL_ID` (`RebirthSelectSocialId`).
    pub select_social_id: i32,
    /// `REBIRTH_SELECT_EFFECT_SKILL_ID` (`RebirthSelectEffectSkillId`).
    pub select_effect_skill_id: i32,
    /// `REBIRTH_SELECT_EFFECT_SKILL_LVL` (`RebirthSelectEffectSkillLevel`).
    pub select_effect_skill_lvl: i32,
    /// `REBIRTH_SCREEN_MESSAGE_ENABLED` (`RebirthScreenMessageEnabled`).
    pub screen_message_enabled: bool,
    /// `REBIRTH_SCREEN_MESSAGE` (`RebirthScreenMessage`).
    pub screen_message: String,
    /// `REBIRTH_SCREEN_MESSAGE_POSITION` (`RebirthScreenMessagePosition`, trimmed, upper case).
    pub screen_message_position: String,
    /// `REBIRTH_SCREEN_MESSAGE_TIME` (`RebirthScreenMessageTime`).
    pub screen_message_time: i32,
    /// `REBIRTH_GLOBAL_ANNOUNCEMENT_ENABLED` (`RebirthGlobalAnnouncementEnabled`).
    pub global_announcement_enabled: bool,
    /// `REBIRTH_GLOBAL_ANNOUNCEMENT` (`RebirthGlobalAnnouncement`).
    pub global_announcement: String,
    /// `REBIRTH_GLOBAL_ANNOUNCEMENT_CRITICAL` (`RebirthGlobalAnnouncementCritical`).
    pub global_announcement_critical: bool,
}

impl Default for RebirthSettings {
    fn default() -> Self {
        Self {
            allow_rebirth: true,
            min_level: 80,
            max_count: 3,
            inherit_skills_to_subclasses: true,
            delete_all_skills: true,
            skill_delete_whitelist: BTreeSet::new(),
            delete_protected_skill_groups: BTreeSet::new(),
            delete_restore_temporary_skills: true,
            item_id: 57,
            item_amount: 3000,
            max_selected_skills: 3,
            reward_item_id: 57,
            reward_item_amount: 1,
            skill_refund_mode: "MID".to_owned(),
            mage_skills: Vec::new(),
            fighter_skills: Vec::new(),
            finish_social_id: 3,
            finish_effect_skill_id: 2024,
            finish_effect_skill_lvl: 1,
            select_social_id: 3,
            select_effect_skill_id: 2024,
            select_effect_skill_lvl: 1,
            screen_message_enabled: true,
            screen_message:
                "Congratulations %player%! Rebirth status acquired. [%rebirth_count%/%max_rebirth%]".to_owned(),
            screen_message_position: "TOP_CENTER".to_owned(),
            screen_message_time: 5000,
            global_announcement_enabled: true,
            global_announcement: "Player %player% has reached Rebirth %rebirth_count% of %max_rebirth%!"
                .to_owned(),
            global_announcement_critical: false,
        }
    }
}

/// Java `String.split(regex)` for a one-character separator: trailing empty strings are dropped.
fn java_split(value: &str, separator: char) -> Vec<&str> {
    let mut parts: Vec<&str> = value.split(separator).collect();
    while parts.last().is_some_and(|p| p.is_empty()) {
        parts.pop();
    }
    parts
}

/// Java `RebirthConfig.parseSkillList`: `;` separated entries, not trimmed.
pub fn parse_skill_list(value: &str) -> Vec<String> {
    if value.trim().is_empty() {
        return Vec::new();
    }
    java_split(value, ';').into_iter().map(str::to_owned).collect()
}

/// Java `RebirthConfig.parseIntegerSet`: comma separated ids, bad tokens are logged and skipped.
pub fn parse_integer_set(value: &str) -> BTreeSet<i32> {
    let mut ids = BTreeSet::new();
    if value.trim().is_empty() {
        return ids;
    }
    for token in java_split(value, ',') {
        let token = token.trim();
        if token.is_empty() {
            continue;
        }
        match token.parse::<i32>() {
            Ok(id) => {
                ids.insert(id);
            }
            Err(_) => warn!("Invalid rebirth whitelist skill id: rawValue={token} fullValue={value}"),
        }
    }
    ids
}

/// Java `RebirthConfig.parseProtectedSkillGroups`: comma separated group names, upper case.
pub fn parse_protected_skill_groups(value: &str) -> BTreeSet<String> {
    let mut groups = BTreeSet::new();
    if value.trim().is_empty() {
        return groups;
    }
    for token in java_split(value, ',') {
        let normalized = token.trim().to_uppercase();
        if normalized.is_empty() {
            continue;
        }
        match normalized.as_str() {
            "STARTING_CLASS" | "FISHING" | "NOBLE" | "CLAN" | "HERO" | "COLLECT" | "SUBCLASS"
            | "SUBPLEDGE" | "TRANSFER" | "TRANSFORM" => {
                groups.insert(normalized);
            }
            _ => warn!("Ignoring invalid rebirth protected skill group: {normalized}"),
        }
    }
    groups
}

/// The `rebirth_system` row of a player, kept in `PlayerProfile::rebirth`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RebirthData {
    /// `rebirthCount` (0 without a row).
    pub count: i32,
    /// `selectedSkills`, in the order they were bought.
    pub selected_skills: Vec<i32>,
}

impl RebirthData {
    /// Java `RebirthManager.getSelectedSkills` on a row: ids joined by commas; a value that is not
    /// a number is logged and skipped.
    pub fn from_columns(count: i32, selected_skills: &str) -> Self {
        let mut skills = Vec::new();
        if !selected_skills.is_empty() {
            for value in java_split(selected_skills, ',') {
                match value.trim().parse::<i32>() {
                    Ok(id) => skills.push(id),
                    Err(_) => warn!(
                        "Failed to parse persisted rebirth skill id: rawValue={value} data={selected_skills}"
                    ),
                }
            }
        }
        Self {
            count,
            selected_skills: skills,
        }
    }
}

/// A `rebirth_system` statement (M9-C05).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebirthWrite {
    /// `INSERT_FIRST_REBIRTH`: the first rebirth, count 1.
    Insert { char_id: i32 },
    /// `UPDATE_REBIRTH_COUNT`.
    Count { char_id: i32, count: i32 },
    /// `UPDATE_SELECTED_SKILLS`: the ids joined by commas.
    Skills { char_id: i32, skills: String },
}

/// Java's `HashMap<Integer, String>` of the skill pool: the definition of each skill id, listed
/// in the order a Java `HashMap` of `Integer` keys iterates.
struct SkillDefs {
    /// Keys in insertion order (a duplicate keeps the place of the first).
    order: Vec<i32>,
    defs: HashMap<i32, String>,
}

impl SkillDefs {
    /// Java `createSkillDefinitionMap`.
    fn new(definitions: &[String], player: ObjectId, name: &str) -> Self {
        let mut defs = Self {
            order: Vec::new(),
            defs: HashMap::new(),
        };
        for definition in definitions {
            let skill_id = get_skill_id(definition);
            if skill_id <= 0 {
                warn!(
                    "Skipping invalid rebirth skill definition while building index: playerId={} playerName={name} definition={definition}",
                    player.0
                );
                continue;
            }
            match defs.defs.insert(skill_id, definition.clone()) {
                Some(previous) => warn!(
                    "Duplicate rebirth skill definition detected while building index: playerId={} playerName={name} skillId={skill_id} previousDefinition={previous} newDefinition={definition}",
                    player.0
                ),
                None => defs.order.push(skill_id),
            }
        }
        defs
    }

    fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    fn get(&self, skill_id: i32) -> Option<&String> {
        self.defs.get(&skill_id)
    }

    /// `map.entrySet()`: the Java `HashMap` order of the keys.
    fn entries(&self) -> Vec<(i32, &String)> {
        crate::stats::skills::java_hash_map_order(&self.order)
            .into_iter()
            .filter_map(|id| self.defs.get(&id).map(|d| (id, d)))
            .collect()
    }
}

/// Java `getSkillId`: `0` for an invalid definition.
fn get_skill_id(definition: &str) -> i32 {
    let parts = java_split(definition, ',');
    if parts.len() < 2 {
        return 0;
    }
    match parts[0].trim().parse::<i32>() {
        Ok(id) => id,
        Err(_) => {
            warn!("Failed to parse rebirth skill id from definition: definition={definition}");
            0
        }
    }
}

/// Java `getSkillLevel`: `1` for an invalid definition.
fn get_skill_level(definition: &str) -> i32 {
    let parts = java_split(definition, ',');
    if parts.len() < 2 {
        return 1;
    }
    match parts[1].trim().parse::<i32>() {
        Ok(level) => level,
        Err(_) => {
            warn!("Failed to parse rebirth skill level from definition: definition={definition}");
            1
        }
    }
}

/// Java `getSkillCost`: at least `1`.
fn get_skill_cost(definition: &str) -> i32 {
    let parts = java_split(definition, ',');
    if parts.len() < 3 {
        return 1;
    }
    match parts[2].trim().parse::<i32>() {
        Ok(cost) => cost.max(1),
        Err(_) => {
            warn!("Failed to parse rebirth skill cost from definition: definition={definition}");
            1
        }
    }
}

/// Java `getScreenMessagePosition`.
fn screen_message_position(position: &str) -> i32 {
    match position {
        "TOP_LEFT" => 1,
        "TOP_RIGHT" => 3,
        "MIDDLE_LEFT" => 4,
        "MIDDLE_CENTER" => 5,
        "MIDDLE_RIGHT" => 6,
        "BOTTOM_CENTER" => 7,
        "BOTTOM_RIGHT" => 8,
        // "TOP_CENTER" and everything else.
        _ => 2,
    }
}

/// `"0" + skillId` below 1000, as Java builds the icon name.
fn skill_icon(skill_id: i32) -> String {
    if skill_id < SKILL_ICON_PADDING_THRESHOLD {
        format!("0{skill_id}")
    } else {
        skill_id.to_string()
    }
}

const BUTTON_STYLE: &str = "back=\"L2UI_CT1.Button_DF_Down\" fore=\"L2UI_CT1.Button_DF\"";

/// Registers the handler of this module (called by `BypassHandlers::default`).
pub(crate) fn register(handlers: &mut BypassHandlers) {
    for word in [
        "rebirth_openmenu",
        "rebirth_confirmrequest",
        "rebirth_selectskills",
        "rebirth_previewSkill",
        "rebirth_confirmSkill",
        "rebirth_resetSkill",
    ] {
        handlers.register(word, rebirth_handler);
    }
}

/// Java `Rebirth.parseSkillId`: the number after the first space.
fn parse_skill_id(command: &str) -> Option<i32> {
    let space = command.find(' ').filter(|&i| i > 0)?;
    command[space + 1..].trim().parse::<i32>().ok()
}

/// Java `Rebirth.onCommand`.
fn rebirth_handler(world: &mut World, player: ObjectId, command: &str, target: Option<ObjectId>) -> bool {
    let object_id = target.map_or(0, |t| t.0);
    if command == "rebirth_openmenu" {
        world.rebirth_display_main_window(player, object_id);
        true
    } else if command == "rebirth_confirmrequest" {
        world.rebirth_request(player);
        true
    } else if command == "rebirth_selectskills" {
        world.rebirth_display_skill_selection_window(player, object_id);
        true
    } else if command.starts_with("rebirth_previewSkill") {
        match parse_skill_id(command) {
            Some(skill_id) => world.rebirth_display_skill_preview_window(player, object_id, skill_id),
            None => world.send_message(player, "Invalid skill."),
        }
        true
    } else if command.starts_with("rebirth_confirmSkill") {
        match parse_skill_id(command) {
            Some(skill_id) => world.rebirth_acquire_skill(player, object_id, skill_id),
            None => world.send_message(player, "Invalid skill."),
        }
        true
    } else if command.starts_with("rebirth_resetSkill") {
        match parse_skill_id(command) {
            Some(skill_id) => world.rebirth_reset_skill(player, object_id, skill_id),
            None => world.send_message(player, "Invalid skill."),
        }
        true
    } else {
        false
    }
}

impl World {
    /// Java `RebirthManager.getRebirthLevel(player)`.
    pub fn rebirth_level(&self, player: ObjectId) -> i32 {
        self.players
            .get(&player)
            .map_or(0, |p| p.data.profile.rebirth.count)
    }

    fn rebirth_player_name(&self, player: ObjectId) -> String {
        self.players
            .get(&player)
            .map(|p| p.data.name.clone())
            .unwrap_or_default()
    }

    /// Java `getSkillPool(player)`: the mage or the fighter list.
    fn rebirth_skill_pool(&self, player: ObjectId) -> Vec<String> {
        let class = self
            .players
            .get(&player)
            .and_then(|p| u16::try_from(p.data.class_id).ok())
            .map(ClassId);
        if class.is_some_and(ClassId::is_mage) {
            self.settings.rebirth.mage_skills.clone()
        } else {
            self.settings.rebirth.fighter_skills.clone()
        }
    }

    fn rebirth_defs(&self, player: ObjectId) -> SkillDefs {
        SkillDefs::new(
            &self.rebirth_skill_pool(player),
            player,
            &self.rebirth_player_name(player),
        )
    }

    /// Java `Skill` of the data: its name, `None` when no such skill exists.
    fn rebirth_skill(&self, skill_id: i32, level: i32) -> Option<String> {
        self.data
            .stat_data()
            .and_then(|d| d.skills.get(skill_id, level))
            .map(|s| s.name.clone())
    }

    /// Java `saveSelectedSkills(charId, skills)`: the player's list and the statement.
    fn rebirth_save_selected_skills(&mut self, player: ObjectId, skills: &[i32]) {
        if let Some(p) = self.players.get_mut(&player) {
            p.data.profile.rebirth.selected_skills = skills.to_vec();
        }
        self.outbox
            .persist(crate::PersistCommand::Rebirth(RebirthWrite::Skills {
                char_id: player.0,
                skills: skills.iter().map(i32::to_string).collect::<Vec<_>>().join(","),
            }));
    }

    /// Java `loadValidatedSelectedSkills`: the stored selection without the skills the pool no
    /// longer has (the cleaned list is saved).
    fn rebirth_validated_skills(&mut self, player: ObjectId, defs: &SkillDefs) -> Vec<i32> {
        let stored = self
            .players
            .get(&player)
            .map(|p| p.data.profile.rebirth.selected_skills.clone())
            .unwrap_or_default();
        if stored.is_empty() {
            return stored;
        }
        let name = self.rebirth_player_name(player);
        let mut valid = Vec::with_capacity(stored.len());
        let mut changed = false;
        for skill_id in stored {
            if defs.get(skill_id).is_some() {
                valid.push(skill_id);
            } else {
                changed = true;
                warn!(
                    "Removing stale rebirth skill from persisted selection: playerId={} playerName={name} skillId={skill_id}",
                    player.0
                );
            }
        }
        if changed {
            self.rebirth_save_selected_skills(player, &valid);
        }
        valid
    }

    /// Java `getItemCount(player, itemId)`: the first stack.
    fn rebirth_item_count(&self, player: ObjectId, item_id: i32) -> i64 {
        self.inventory_item_by_item_id(player, item_id)
            .map_or(0, |i| i.count)
    }

    /// Java `consumeItem`.
    fn rebirth_consume_item(&mut self, player: ObjectId, item_id: i32, amount: i64, reason: &str) -> bool {
        if item_id <= 0 || amount <= 0 || amount > i64::from(i32::MAX) {
            warn!("Invalid consumeItem request: player={player} itemId={item_id} amount={amount} reason={reason}");
            return false;
        }
        let item = self
            .inventory_item_by_item_id(player, item_id)
            .map(|i| (i.object_id, i.count));
        let Some((object_id, _)) = item.filter(|(_, count)| *count >= amount) else {
            let name = if item.is_some() {
                self.item_template_name(item_id)
                    .unwrap_or_else(|| format!("Item {item_id}"))
            } else {
                format!("Item {item_id}")
            };
            self.send_message(
                player,
                &format!("You need at least {amount} [ {name} ] for {reason}."),
            );
            return false;
        };
        // NOTE(java-quirk): `Inventory.destroyItem` sends no `InventoryUpdate` (only
        // `Player.destroyItem` does), so the client shows the old count until the next update
        // of that item or the next login.
        self.destroy_inventory_item(player, object_id, amount);
        true
    }

    /// Java `Player.addItem(process, itemId, count, reference, sendMessage)` for the token rewards.
    fn rebirth_give_item(&mut self, player: ObjectId, item_id: i32, count: i64, send_message: bool) {
        self.add_item(player, item_id, count, send_message);
    }

    // ------------------------------------------------------------------ the skill groups

    /// Java `isSkillInTree`: the skill of exactly this level is a line of the tree.
    fn rebirth_in_tree(tree: &[l2_data::SkillLearn], skill_id: i32, level: i32) -> bool {
        tree.iter().any(|s| s.skill_id == skill_id && s.skill_level == level)
    }

    /// Java `isProtectedByGroup`.
    fn rebirth_protected_by_group(&self, player: ObjectId, skill_id: i32, level: i32) -> bool {
        let trees = &self.rewards.skill_trees;
        let base_class = self
            .players
            .get(&player)
            .and_then(|p| u16::try_from(p.data.base_class).ok())
            .map(ClassId);
        for group in &self.settings.rebirth.delete_protected_skill_groups {
            let hit = match group.as_str() {
                "STARTING_CLASS" => base_class.is_some_and(|base| {
                    // The root class of the base class: `classList.xml` parents, which are the
                    // `PlayerClass` parents (Warrior's `parentClassId="0"` is Human Fighter).
                    let mut root = base;
                    while let Some(parent) = root.enum_parent() {
                        root = parent;
                    }
                    trees
                        .complete_class_tree(root)
                        .any(|s| s.skill_id == skill_id && s.skill_level == level)
                }),
                "FISHING" => Self::rebirth_in_tree(trees.fishing_skills(), skill_id, level),
                "NOBLE" => Self::rebirth_in_tree(trees.noble_skills(), skill_id, level),
                "HERO" => Self::rebirth_in_tree(trees.hero_skills(), skill_id, level),
                "CLAN" => Self::rebirth_in_tree(trees.pledge_skills(), skill_id, level),
                "COLLECT" => Self::rebirth_in_tree(trees.collect_skills(), skill_id, level),
                "SUBCLASS" => Self::rebirth_in_tree(trees.sub_class_skills(), skill_id, level),
                "SUBPLEDGE" => Self::rebirth_in_tree(trees.sub_pledge_skills(), skill_id, level),
                "TRANSFER" => base_class
                    .is_some_and(|base| Self::rebirth_in_tree(trees.transfer_skill_tree(base), skill_id, level)),
                "TRANSFORM" => Self::rebirth_in_tree(trees.transform_skills(), skill_id, level),
                _ => false,
            };
            if hit {
                return true;
            }
        }
        false
    }

    /// Java `getRefundAmount`.
    fn rebirth_refund_amount(&self, item_cost: i32) -> i32 {
        match self.settings.rebirth.skill_refund_mode.as_str() {
            "FULL" => item_cost,
            "MID" => item_cost / 2,
            _ => 0,
        }
    }

    // ------------------------------------------------------------------ the skills

    /// Java `grantRebirthSkills(player)`, called by `rewardSkills` and `regiveTemporarySkills`
    /// when `RebirthAllow` is on.
    pub(crate) fn rebirth_grant_skills(&mut self, player: ObjectId) {
        let defs = self.rebirth_defs(player);
        let selected = self.rebirth_validated_skills(player, &defs);
        self.rebirth_grant_selected(player, &defs, &selected);
    }

    /// Java `grantRebirthSkills(player, skillDefinitionMap, selectedSkills)`.
    fn rebirth_grant_selected(&mut self, player: ObjectId, defs: &SkillDefs, selected: &[i32]) {
        let in_subclass = self
            .players
            .get(&player)
            .is_some_and(|p| p.data.profile.class_index > 0);
        if !self.settings.rebirth.inherit_skills_to_subclasses && in_subclass {
            return;
        }
        let name = self.rebirth_player_name(player);
        let mut writes = SkillWrites::default();
        for &skill_id in selected {
            let Some(definition) = defs.get(skill_id) else {
                warn!(
                    "Skipping selected rebirth skill because definition was not found: playerId={} playerName={name} skillId={skill_id}",
                    player.0
                );
                continue;
            };
            let level = get_skill_level(definition);
            if self.rebirth_skill(skill_id, level).is_some() {
                self.learn_skill(player, skill_id, level, &mut writes);
            } else {
                warn!(
                    "Skipping selected rebirth skill because skill data was not found: playerId={} playerName={name} skillId={skill_id} definition={definition}",
                    player.0
                );
            }
        }
        self.flush_skill_writes(player, writes);
        self.send_skill_list(player);
    }

    // ------------------------------------------------------------------ the windows

    /// Java `buildAcquiredSkillsHtml`.
    fn rebirth_acquired_skills_html(
        &self,
        player: ObjectId,
        selected: &[i32],
        defs: &SkillDefs,
        show_reset_button: bool,
    ) -> String {
        if selected.is_empty() {
            return "<center><font color=\"AAAAAA\">You don't have any skills selected yet.</font></center>"
                .to_owned();
        }
        let name = self.rebirth_player_name(player);
        let mut html = String::from("<table width=270 border=0 cellpadding=2 cellspacing=2><tr>");
        let mut column = 0;
        for &skill_id in selected {
            let Some(definition) = defs.get(skill_id) else {
                warn!(
                    "Selected rebirth skill definition was not found while building acquired skills HTML: playerId={} playerName={name} skillId={skill_id}",
                    player.0
                );
                continue;
            };
            let level = get_skill_level(definition);
            let Some(skill_name) = self.rebirth_skill(skill_id, level) else {
                warn!(
                    "Selected rebirth skill was not found while building acquired skills HTML: playerId={} playerName={name} skillId={skill_id} skillLevel={level} definition={definition}",
                    player.0
                );
                continue;
            };
            let item_cost = get_skill_cost(definition);
            if column == SKILLS_PER_ROW {
                html.push_str("</tr><tr>");
                column = 0;
            }
            html.push_str("<td width=90 align=center>");
            html.push_str(&format!(
                "<img src=\"icon.skill{}\" width=32 height=32><br1>",
                skill_icon(skill_id)
            ));
            html.push_str(&format!("<font color=\"FFDD99\">{skill_name}</font><br1>"));
            html.push_str(&format!("<font color=\"FFFF99\">Lv. {level}</font><br1>"));
            html.push_str(&format!("<font color=\"99FF99\">Cost: {item_cost}</font><br1>"));
            if show_reset_button {
                html.push_str(&format!(
                    "<button value=\"Reset\" action=\"bypass -h rebirth_resetSkill {skill_id}\" width=65 height=18 {BUTTON_STYLE}><br1>"
                ));
            }
            html.push_str("</td>");
            column += 1;
        }
        html.push_str("</tr></table>");
        html
    }

    fn rebirth_send_page(&mut self, player: ObjectId, npc_object_id: i32, html: &str) {
        if let Some(message) = l2_protocol_game::server::NpcHtmlMessage::new(npc_object_id, html) {
            self.send_html(player, &message);
        }
    }

    /// Java `displaySkillSelectionWindow`.
    pub(crate) fn rebirth_display_skill_selection_window(&mut self, player: ObjectId, object_id: i32) {
        let rebirth_count = self.rebirth_level(player);
        if rebirth_count <= 0 {
            self.send_message(player, "You must perform at least one Rebirth before selecting skills.");
            return;
        }
        let defs = self.rebirth_defs(player);
        if defs.is_empty() {
            self.send_message(
                player,
                "There are no skills configured for your class. Contact an administrator.",
            );
            return;
        }
        let selected = self.rebirth_validated_skills(player, &defs);
        let allowed_skills = rebirth_count.min(self.settings.rebirth.max_selected_skills);
        let tokens = self.rebirth_item_count(player, self.settings.rebirth.reward_item_id);
        let name = self.rebirth_player_name(player);

        let mut html = String::from("<html><body><center>");
        html.push_str("<title>Select your Rebirth Skills</title>");
        html.push_str(&format!(
            "<br><font color=LEVEL>Maximum allowed: {allowed_skills} skill(s)</font><br1>"
        ));
        html.push_str(&format!(
            "<font color=99FF99>Tokens available: {tokens}</font><br><br>"
        ));
        html.push_str("<table width=256 cellpadding=1 cellspacing=2><tr>");
        let mut column = 0;
        for (skill_id, definition) in defs.entries() {
            let level = get_skill_level(definition);
            if level <= 0 {
                warn!(
                    "Skipping invalid rebirth skill definition in selection window: playerId={} playerName={name} definition={definition}",
                    player.0
                );
                continue;
            }
            let Some(skill_name) = self.rebirth_skill(skill_id, level) else {
                warn!(
                    "Configured rebirth skill was not found in selection window: playerId={} playerName={name} skillId={skill_id} skillLevel={level} definition={definition}",
                    player.0
                );
                continue;
            };
            let item_cost = get_skill_cost(definition);
            if column == SKILLS_PER_ROW {
                html.push_str("</tr><tr>");
                column = 0;
            }
            html.push_str("<td width=90 align=center>");
            html.push_str(&format!(
                "<img src=\"icon.skill{}\" width=32 height=32><br1>",
                skill_icon(skill_id)
            ));
            html.push_str(&format!("<font color=\"FFDD99\">{skill_name}</font><br1>"));
            html.push_str(&format!("<font color=\"FFFF99\">Lv. {level}</font><br1>"));
            html.push_str(&format!("<font color=\"99FF99\">Cost: {item_cost}</font><br1>"));
            if selected.contains(&skill_id) {
                html.push_str("<font color=AAAAAA>Owned</font>");
            } else if i32::try_from(selected.len()).unwrap_or(i32::MAX) >= allowed_skills {
                html.push_str("<font color=AAAAAA>Limit reached</font>");
            } else {
                html.push_str(&format!(
                    "<button value=\"Choose\" action=\"bypass -h rebirth_previewSkill {skill_id}\" width=40 height=20 {BUTTON_STYLE}>"
                ));
            }
            html.push_str("</td>");
            column += 1;
        }
        html.push_str("</tr></table><br>");
        html.push_str(&format!(
            "<button value=\"Return\" action=\"bypass -h rebirth_openmenu\" width=95 height=20 {BUTTON_STYLE}>"
        ));
        html.push_str("</center></body></html>");
        self.rebirth_send_page(player, object_id, &html);
    }

    /// Java `displaySkillPreviewWindow(player, objectId, skillId)`.
    pub(crate) fn rebirth_display_skill_preview_window(&mut self, player: ObjectId, object_id: i32, skill_id: i32) {
        let defs = self.rebirth_defs(player);
        let saved = self.rebirth_validated_skills(player, &defs);
        let rebirth_count = self.rebirth_level(player);
        self.rebirth_preview(player, object_id, skill_id, &defs, &saved, rebirth_count);
    }

    /// Java `displaySkillPreviewWindow(..)` with the lists.
    fn rebirth_preview(
        &mut self,
        player: ObjectId,
        object_id: i32,
        skill_id: i32,
        defs: &SkillDefs,
        saved: &[i32],
        rebirth_count: i32,
    ) {
        let Some(definition) = defs.get(skill_id) else {
            self.send_message(player, "Invalid skill.");
            return;
        };
        let target_level = get_skill_level(definition);
        let Some(skill_name) = self.rebirth_skill(skill_id, target_level) else {
            warn!(
                "Configured rebirth skill preview was not found: playerId={} playerName={} skillId={skill_id} targetLevel={target_level} definition={definition}",
                player.0,
                self.rebirth_player_name(player)
            );
            self.send_message(player, "Invalid skill.");
            return;
        };
        let item_cost = get_skill_cost(definition);
        let allowed_skills = rebirth_count.min(self.settings.rebirth.max_selected_skills);
        let available_tokens = self.rebirth_item_count(player, self.settings.rebirth.reward_item_id);
        let already_selected = saved.contains(&skill_id);
        let limit_reached = i32::try_from(saved.len()).unwrap_or(i32::MAX) >= allowed_skills;
        let has_enough_tokens = available_tokens >= i64::from(item_cost);
        let max_count = self.settings.rebirth.max_count;

        let mut html = String::from("<html><body><center>");
        html.push_str("<br>");
        html.push_str("<font color=\"LEVEL\">Rebirth Manager</font><br>");
        html.push_str("<table width=256 border=0 cellpadding=1 cellspacing=2>");
        html.push_str("<tr><td width=95 align=center>");
        html.push_str("<font color=\"66CC66\">Your Rebirths</font><br1>");
        html.push_str(&format!("<font color=\"FFFF99\">{rebirth_count}/{max_count}</font><br1>"));
        html.push_str(&format!("<font color=\"99FF99\">Tokens: {available_tokens}</font>"));
        html.push_str("</td><td width=95 align=center>");
        html.push_str(&format!(
            "<img src=\"icon.skill{}\" width=32 height=32><br1>",
            skill_icon(skill_id)
        ));
        html.push_str(&format!("<font color=\"FFDD99\">{skill_name}</font><br1>"));
        html.push_str(&format!("<font color=\"FFFF99\">Lv. {target_level}</font><br1>"));
        html.push_str(&format!("<font color=\"99FF99\">Cost: {item_cost}</font>"));
        html.push_str("</td></tr></table>");
        html.push_str("<br1>");
        html.push_str(&format!(
            "<font color=\"LEVEL\">This purchase consumes {item_cost} token(s).</font><br1>"
        ));
        html.push_str(&format!(
            "<font color=\"LEVEL\">Refund: {}</font><br1>",
            self.settings.rebirth.skill_refund_mode
        ));
        html.push_str("<font color=\"66CC66\">Rebirth Skills Obtained</font><br1>");
        html.push_str(&self.rebirth_acquired_skills_html(player, saved, defs, false));
        html.push_str("<br1>");
        if !already_selected && !limit_reached && has_enough_tokens {
            html.push_str(&format!(
                "<button value=\"Confirm\" action=\"bypass -h rebirth_confirmSkill {skill_id}\" width=95 height=20 {BUTTON_STYLE}><br1>"
            ));
        } else {
            if already_selected {
                html.push_str("<font color=AAAAAA>Already selected.</font><br1>");
            }
            if limit_reached {
                html.push_str("<font color=AAAAAA>Skill limit reached.</font><br1>");
            }
            if !has_enough_tokens {
                html.push_str("<font color=FF6666>Not enough tokens.</font><br1>");
            }
        }
        html.push_str(&format!(
            "<button value=\"Return\" action=\"bypass -h rebirth_selectskills\" width=95 height=20 {BUTTON_STYLE}>"
        ));
        html.push_str("</center></body></html>");
        self.rebirth_send_page(player, object_id, &html);
    }

    /// Java `displayMainWindow(player, objectId)`.
    pub(crate) fn rebirth_display_main_window(&mut self, player: ObjectId, object_id: i32) {
        let defs = self.rebirth_defs(player);
        let selected = self.rebirth_validated_skills(player, &defs);
        let count = self.rebirth_level(player);
        self.rebirth_main_window(player, object_id, &selected, count, &defs);
    }

    /// Java `displayMainWindow(..)` with the lists.
    fn rebirth_main_window(
        &mut self,
        player: ObjectId,
        object_id: i32,
        selected: &[i32],
        rebirth_count: i32,
        defs: &SkillDefs,
    ) {
        let token_count = self.rebirth_item_count(player, self.settings.rebirth.reward_item_id);
        let Some(mut message) = self.html_from_file(player, object_id, "data/html/default/70001_dynamic.htm")
        else {
            return;
        };
        let max_count = self.settings.rebirth.max_count;
        let mode = self.settings.rebirth.skill_refund_mode.clone();
        let icons = self.rebirth_acquired_skills_html(player, selected, defs, true);
        let button = if rebirth_count < max_count {
            format!(
                "<button value=\"Request Rebirth\" action=\"bypass -h rebirth_confirmrequest\" width=130 height=20 {BUTTON_STYLE}>"
            )
        } else {
            "<font color=AAAAAA>Max Rebirth reached.</font>".to_owned()
        };
        // `NpcHtmlMessage.replace` is a literal replace of every occurrence, in this order.
        let mut html = message.html.clone();
        for (pattern, value) in [
            ("%rebirth_count%", format!("{rebirth_count}/{max_count}")),
            ("%rebirth_icons%", icons),
            (
                "%next_skill_icon%",
                "<img src=\"icon.etc_coins_gold_i00\" width=32 height=32>".to_owned(),
            ),
            ("%next_skill_name%", "Rebirth Tokens".to_owned()),
            ("%next_skill_lvl%", token_count.to_string()),
            ("%next_skill_desc%", format!("Refund mode: {mode}")),
            ("%rebirth_button%", button),
        ] {
            html = html.replace(pattern, &value);
        }
        message.html = html;
        self.send_html(player, &message);
    }

    // ------------------------------------------------------------------ the actions

    /// Java `acquireRebirthSkill`.
    pub(crate) fn rebirth_acquire_skill(&mut self, player: ObjectId, object_id: i32, skill_id: i32) {
        let defs = self.rebirth_defs(player);
        let mut saved = self.rebirth_validated_skills(player, &defs);
        let rebirth_count = self.rebirth_level(player);
        if rebirth_count <= 0 {
            self.send_message(player, "You must perform at least one Rebirth before selecting skills.");
            self.rebirth_main_window(player, object_id, &saved, rebirth_count, &defs);
            return;
        }
        if saved.contains(&skill_id) {
            self.send_message(player, "You already own this rebirth skill.");
            self.rebirth_main_window(player, object_id, &saved, rebirth_count, &defs);
            return;
        }
        let allowed_skills = rebirth_count.min(self.settings.rebirth.max_selected_skills);
        if i32::try_from(saved.len()).unwrap_or(i32::MAX) >= allowed_skills {
            self.send_message(
                player,
                &format!("You have reached the maximum number of skills allowed. ({allowed_skills})."),
            );
            self.rebirth_main_window(player, object_id, &saved, rebirth_count, &defs);
            return;
        }
        let Some(definition) = defs.get(skill_id).cloned() else {
            self.send_message(player, "This skill is not configured for your class.");
            self.rebirth_main_window(player, object_id, &saved, rebirth_count, &defs);
            return;
        };
        let item_cost = get_skill_cost(&definition);
        let reward_item = self.settings.rebirth.reward_item_id;
        if !self.rebirth_consume_item(player, reward_item, i64::from(item_cost), "rebirth token") {
            self.rebirth_preview(player, object_id, skill_id, &defs, &saved, rebirth_count);
            return;
        }
        saved.push(skill_id);
        self.rebirth_save_selected_skills(player, &saved);
        self.rebirth_grant_selected(player, &defs, &saved);
        self.rebirth_play_select_fx(player);
        self.send_message(player, "Rebirth skill purchased successfully.");
        self.rebirth_main_window(player, object_id, &saved, rebirth_count, &defs);
    }

    /// Java `resetRebirthSkill`.
    pub(crate) fn rebirth_reset_skill(&mut self, player: ObjectId, object_id: i32, skill_id: i32) {
        let defs = self.rebirth_defs(player);
        let mut saved = self.rebirth_validated_skills(player, &defs);
        let rebirth_count = self.rebirth_level(player);
        if !saved.contains(&skill_id) {
            self.send_message(player, "You do not own this rebirth skill.");
            self.rebirth_main_window(player, object_id, &saved, rebirth_count, &defs);
            return;
        }
        let Some(definition) = defs.get(skill_id).cloned() else {
            self.send_message(player, "The configured skill data is invalid.");
            self.rebirth_main_window(player, object_id, &saved, rebirth_count, &defs);
            return;
        };
        let skill_level = get_skill_level(&definition);
        let item_cost = get_skill_cost(&definition);
        let refund_amount = self.rebirth_refund_amount(item_cost);

        // `savedSkills.remove(Integer.valueOf(skillId))`: the first one.
        if let Some(at) = saved.iter().position(|s| *s == skill_id) {
            saved.remove(at);
        }
        self.rebirth_save_selected_skills(player, &saved);

        if self.rebirth_skill(skill_id, skill_level).is_some() {
            let mut writes = SkillWrites::default();
            self.forget_skill(player, skill_id, &mut writes);
            self.flush_skill_writes(player, writes);
        } else {
            warn!(
                "Configured rebirth skill could not be removed because it was not found: playerId={} playerName={} skillId={skill_id} skillLevel={skill_level} definition={definition}",
                player.0,
                self.rebirth_player_name(player)
            );
        }
        self.send_skill_list(player);

        if refund_amount > 0 {
            let reward_item = self.settings.rebirth.reward_item_id;
            self.rebirth_give_item(player, reward_item, i64::from(refund_amount), true);
            self.send_message(player, &format!("Refunded {refund_amount} rebirth token(s)."));
        } else {
            let mode = self.settings.rebirth.skill_refund_mode.clone();
            self.send_message(
                player,
                &format!("Skill removed. Refund mode {mode} returned no tokens."),
            );
        }
        self.rebirth_main_window(player, object_id, &saved, rebirth_count, &defs);
    }

    /// Java `requestRebirth`.
    pub(crate) fn rebirth_request(&mut self, player: ObjectId) {
        if !self.settings.rebirth.allow_rebirth {
            self.send_message(player, "Rebirth is disabled by configuration.");
            return;
        }
        let min_level = self.settings.rebirth.min_level;
        if self.players.get(&player).map_or(0, |p| p.core.level) < min_level {
            self.send_message(
                player,
                &format!("You do not meet the minimum level for Rebirth ({min_level})."),
            );
            return;
        }
        if self.is_alike_dead(player) || self.is_casting_now(player) || self.is_attacking_now(player) {
            self.send_message(player, "You cannot request Rebirth right now.");
            return;
        }
        let current_count = self.rebirth_level(player);
        if current_count >= self.settings.rebirth.max_count {
            self.send_message(player, "You are currently at your maximum rebirth count.");
            return;
        }
        let (item_id, amount) = (self.settings.rebirth.item_id, self.settings.rebirth.item_amount);
        if !self.rebirth_consume_item(player, item_id, i64::from(amount), "rebirth request") {
            return;
        }
        self.rebirth_grant(player, current_count + 1, current_count == 0);
    }

    /// Java `grantRebirth`.
    fn rebirth_grant(&mut self, player: ObjectId, new_count: i32, first_birth: bool) {
        // Critical persistence step.
        if let Some(p) = self.players.get_mut(&player) {
            p.data.profile.rebirth.count = new_count;
            if first_birth {
                p.data.profile.rebirth.selected_skills.clear();
            }
        }
        self.outbox.persist(crate::PersistCommand::Rebirth(if first_birth {
            RebirthWrite::Insert { char_id: player.0 }
        } else {
            RebirthWrite::Count {
                char_id: player.0,
                count: new_count,
            }
        }));

        // Critical player mutation step.
        let current_exp = self.players.get(&player).map_or(0, |p| p.data.exp);
        let target_exp = self.exp_for_level(1).unwrap_or(0);
        if current_exp > target_exp {
            self.set_level(player, 1);
            self.remove_exp_and_sp(player, current_exp - target_exp, 0, true);
            let (max_hp, max_mp, max_cp) = self.stats(player).map_or((0.0, 0.0, 0.0), |v| {
                (f64::from(v.max_hp()), f64::from(v.max_mp()), f64::from(v.max_cp()))
            });
            self.set_current_hp_mp(player, max_hp, max_mp);
            self.set_current_cp(player, max_cp, true);
            self.broadcast_user_info(player);
        }

        let base_class = self.players.get(&player).map_or(0, |p| p.data.base_class);
        self.set_player_class(player, base_class);
        if self.settings.rebirth.delete_all_skills {
            let known = self
                .players
                .get(&player)
                .map(|p| p.core.skills.clone())
                .unwrap_or_default();
            let mut writes = SkillWrites::default();
            for (skill_id, level) in known {
                if self.settings.rebirth.skill_delete_whitelist.contains(&skill_id) {
                    continue;
                }
                if self.rebirth_protected_by_group(player, skill_id, level) {
                    continue;
                }
                self.forget_skill(player, skill_id, &mut writes);
            }
            self.flush_skill_writes(player, writes);
        }
        self.give_available_skills_now(player, true, true, true);
        if self.settings.rebirth.delete_restore_temporary_skills {
            self.regive_temporary_skills(player);
        }
        self.store_player(player);

        // Post-commit section.
        let defs = self.rebirth_defs(player);
        let selected = self.rebirth_validated_skills(player, &defs);
        self.rebirth_grant_reward_tokens(player);
        if !self.settings.rebirth.delete_restore_temporary_skills {
            self.rebirth_grant_selected(player, &defs, &selected);
        }
        self.send_skill_list(player);
        self.broadcast_user_info(player);
        self.broadcast_status_update(player);
        self.rebirth_display_congrats(player, new_count);
        self.send_message(player, "Rebirth applied successfully. You have been reborn!");
        // `routePostRebirthUi`.
        self.rebirth_main_window(player, 0, &selected, new_count, &defs);
        self.send_message(player, "Use your rebirth tokens to purchase skills.");
    }

    /// Java `grantRewardTokens`.
    fn rebirth_grant_reward_tokens(&mut self, player: ObjectId) {
        let (item_id, amount) = (
            self.settings.rebirth.reward_item_id,
            self.settings.rebirth.reward_item_amount,
        );
        if item_id <= 0 || amount <= 0 {
            return;
        }
        self.rebirth_give_item(player, item_id, i64::from(amount), true);
        self.send_message(player, &format!("You received {amount} rebirth token(s)."));
    }

    fn rebirth_creature_skill_use(&self, player: ObjectId, skill_id: i32, level: i32) {
        let Some((x, y, z)) = self.position_of(player) else {
            return;
        };
        let at = SkillPosition { x, y, z };
        self.broadcast_creature_packet(
            player,
            &MagicSkillUse {
                creature_id: player.0,
                target_id: player.0,
                skill_id,
                skill_level: level,
                hit_time: 1,
                reuse_delay: 0,
                creature: at,
                ground_location: None,
                target: at,
            },
        );
    }

    /// Java `displayCongrats`.
    fn rebirth_display_congrats(&mut self, player: ObjectId, rebirth_count: i32) {
        let settings = self.settings.rebirth.clone();
        if settings.finish_social_id > 0 {
            self.broadcast_creature_packet(
                player,
                &SocialAction {
                    object_id: player.0,
                    action_id: settings.finish_social_id,
                },
            );
        }
        if settings.finish_effect_skill_id > 0 {
            self.rebirth_creature_skill_use(
                player,
                settings.finish_effect_skill_id,
                settings.finish_effect_skill_lvl,
            );
        }
        let name = self.rebirth_player_name(player);
        let fill = |text: &str| {
            text.replace("%player%", &name)
                .replace("%rebirth_count%", &rebirth_count.to_string())
                .replace("%max_rebirth%", &settings.max_count.to_string())
        };
        let screen_message = fill(&settings.screen_message);
        let global_announcement = fill(&settings.global_announcement);
        if settings.screen_message_enabled {
            let mut packet = ExShowScreenMessage::text(&screen_message, settings.screen_message_time);
            packet.position = screen_message_position(&settings.screen_message_position);
            self.send(player, &packet);
        }
        if settings.global_announcement_enabled {
            self.broadcast_announcement(&global_announcement, settings.global_announcement_critical);
        }
        self.send_message(player, &format!("Congratulations {name}! You have been Reborn!"));
    }

    /// Java `playRebirthSelectFx`.
    fn rebirth_play_select_fx(&mut self, player: ObjectId) {
        let settings = self.settings.rebirth.clone();
        if settings.select_social_id > 0 {
            self.broadcast_creature_packet(
                player,
                &SocialAction {
                    object_id: player.0,
                    action_id: settings.select_social_id,
                },
            );
        }
        if settings.select_effect_skill_id > 0 {
            self.rebirth_creature_skill_use(
                player,
                settings.select_effect_skill_id,
                settings.select_effect_skill_lvl,
            );
        }
    }
}
