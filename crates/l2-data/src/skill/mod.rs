//! Skill templates: every skill of `data/stats/skills/*.xml` as one [`SkillTemplate`] per id and
//! level, with enchant levels, effect templates and use conditions.
//!
//! Java: `gameserver/data/xml/SkillData.java` (load, `getSkillHashCode`, `getSkill`, max levels,
//! the enchantable set), `data/xml/EnchantSkillGroupsData.java` (only the number of levels a route
//! has), `util/DocumentSkill.java` and `util/DocumentBase.java` (see [`document`]).
//! Data: `data/stats/skills/*.xml` (+ `custom/` with `CustomSkillsLoad`), `data/EnchantSkillGroups.xml`
//! (`xsd/skills.xsd`).
//!
//! # Pieces
//! - [`template`]: [`SkillTemplate`] (the typed `Skill` fields, leftover `StatSet`), the enums.
//! - [`effect`]: [`EffectTemplate`] (name, scope, raw params, stat functions, conditions).
//! - [`document`]: the parser (tables, `#` lookups, enchant routes, effect lists).
//! - this file: the table, the loader (parallel, merged in file order) and the lookups.
//!
//! # What M4 uses
//! Passive skills carry their stat functions in `effects[Passive][..].funcs` (M4-STAT-01 turns them
//! into calculator entries); the effect handlers of active skills are M5. Replacing the ad-hoc
//! passive scan of `game-server/src/world_data.rs` with [`SkillOperateType::is_passive`] is
//! STAT-01's job.

mod document;
pub mod effect;
pub mod template;

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};

use tracing::{info, warn};

pub use effect::{EffectLists, EffectTemplate, ItemEntry, ItemGroup, ParamValue};
pub use template::{
    AbnormalVisualEffect, BaseStat, EffectScope, FlyType, SetError, SetValue, SkillOperateType,
    SkillTemplate, StatSet, TargetType, TraitType,
};

use crate::loader::log_loaded;
use crate::stats::node::parse_dom;

/// The config values that change what the skill loader builds. [`SkillLoadOptions::default`] holds
/// Java's *code* defaults.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillLoadOptions {
    /// `General.ini` `CustomSkillsLoad` (default false): also load `data/stats/skills/custom`.
    pub custom_skills_load: bool,
    /// `Player.ini` `MinAbnormalStateSuccessRate` (default 10): the default `minChance`.
    pub min_abnormal_state_success_rate: i32,
    /// `Player.ini` `MaxAbnormalStateSuccessRate` (default 90): the default `maxChance`.
    pub max_abnormal_state_success_rate: i32,
    /// `Player.ini` `EnableModifySkillDuration` (default false).
    pub enable_modify_skill_duration: bool,
    /// `Player.ini` `SkillDurationList`: skill id to abnormal time.
    pub skill_duration_list: HashMap<i32, i32>,
    /// `Player.ini` `EnableModifySkillReuse` (default false).
    pub enable_modify_skill_reuse: bool,
    /// `Player.ini` `SkillReuseList`: skill id to reuse delay.
    pub skill_reuse_list: HashMap<i32, i32>,
}

impl Default for SkillLoadOptions {
    fn default() -> Self {
        Self {
            custom_skills_load: false,
            min_abnormal_state_success_rate: 10,
            max_abnormal_state_success_rate: 90,
            enable_modify_skill_duration: false,
            skill_duration_list: HashMap::new(),
            enable_modify_skill_reuse: false,
            skill_reuse_list: HashMap::new(),
        }
    }
}

/// The part of Java's `EnchantSkillGroupsData` the skill documents need: how many enchant levels a
/// group has (`addNewRouteForSkill` returns the number of `<enchant>` details of the group).
///
/// The costs and chances of the groups belong to the enchant system, not to this card.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnchantSkillGroups {
    /// Group id to its number of `<enchant level=..>` details (Java `getEnchantGroupDetails().size()`).
    levels: BTreeMap<i32, usize>,
}

impl EnchantSkillGroups {
    const LOADER: &'static str = "EnchantSkillGroupsData";

    /// Builds the table from `(group id, level count)` pairs. For tests and tools.
    pub fn new(groups: impl IntoIterator<Item = (i32, usize)>) -> Self {
        let mut levels = BTreeMap::new();
        for (group, count) in groups {
            *levels.entry(group).or_insert(0) += count;
        }
        Self { levels }
    }

    /// Reads `<root>/data/EnchantSkillGroups.xml` and logs Java's count line.
    pub fn load(root: &Path) -> Self {
        let path = root.join("data/EnchantSkillGroups.xml");
        let groups = match std::fs::read(&path) {
            Ok(bytes) => Self::from_xml(&String::from_utf8_lossy(&bytes)),
            Err(error) => {
                warn!("Cannot parse EnchantSkillGroups.xml: {error}");
                Self::default()
            }
        };
        info!(
            "{}: Loaded {} groups and {} routes.",
            Self::LOADER,
            groups.levels.len(),
            groups.levels.values().sum::<usize>()
        );
        groups
    }

    /// Parses the text of `EnchantSkillGroups.xml`; a broken document gives an empty table.
    pub fn from_xml(xml: &str) -> Self {
        let mut groups = Self::default();
        let root = match parse_dom(xml) {
            Ok(root) => root,
            Err(error) => {
                warn!("Error parsing EnchantSkillGroups.xml: {error}");
                return groups;
            }
        };
        if !root.name.eq_ignore_ascii_case("list") {
            return groups;
        }
        for group in root
            .children
            .iter()
            .filter(|g| g.name.eq_ignore_ascii_case("group"))
        {
            // `parseInteger(attrs, "id")` is an `Integer.decode`; a missing id fails the file.
            let Some(id) = group.attr("id").and_then(crate::stats::jnum::decode_i32) else {
                warn!("Error parsing EnchantSkillGroups.xml: bad group id");
                return groups;
            };
            let count = group
                .children
                .iter()
                .filter(|e| e.name.eq_ignore_ascii_case("enchant"))
                .count();
            *groups.levels.entry(id).or_insert(0) += count;
        }
        groups
    }

    /// The number of enchant levels of a group, `None` for an unknown group.
    pub fn levels(&self, group: i32) -> Option<usize> {
        self.levels.get(&group).copied()
    }

    /// Java `addNewRouteForSkill(skillId, maxLvL, route, group)`: the number of enchant levels the
    /// route gets, 0 (logged) for a missing group.
    pub(crate) fn add_new_route_for_skill(&self, skill_id: i32, route: usize, group: i32) -> usize {
        match self.levels(group) {
            Some(count) => count,
            None => {
                tracing::error!(
                    "{}: Error while loading generating enchant skill id: {skill_id}; route: {route}; missing group: {group}",
                    Self::LOADER
                );
                0
            }
        }
    }
}

/// All skills, immutable after loading. Java `SkillData`.
#[derive(Debug, Default)]
pub struct SkillTemplates {
    /// Sorted by `(id, level)`.
    skills: Vec<SkillTemplate>,
    /// Java `_skillsByHash`: `id * 1021 + level` to the index in `skills`.
    by_hash: HashMap<i32, usize>,
    /// Java `_maxSkillLevels`: the highest plain level (at most 99) per skill id.
    max_levels: HashMap<i32, i32>,
    /// Java `_enchantable`: ids that have a level above 99.
    enchantable: std::collections::HashSet<i32>,
    /// Java's `count`: every skill read, a duplicate id and level included.
    loaded: usize,
}

impl SkillTemplates {
    const LOADER: &'static str = "SkillData";

    /// Java `getSkillHashCode(skillId, skillLevel)`. Two different (id, level) pairs share a hash
    /// only for levels of 1021 and up, which no datapack has.
    pub fn hash_code(skill_id: i32, skill_level: i32) -> i32 {
        skill_id.wrapping_mul(1021).wrapping_add(skill_level)
    }

    /// Loads `data/stats/skills` (and `custom` when asked) below `root` and logs Java's count line
    /// (`SkillData: Loaded N Skill templates from XML files.`). The files are parsed on several
    /// threads and merged in sorted path order, so the table does not depend on the thread count;
    /// when two files define the same id and level the later path wins.
    pub fn load(root: &Path, options: &SkillLoadOptions) -> Self {
        let groups = EnchantSkillGroups::load(root);
        Self::load_with_groups(root, &groups, options)
    }

    /// [`SkillTemplates::load`] with a group table that is already loaded.
    pub fn load_with_groups(root: &Path, groups: &EnchantSkillGroups, options: &SkillLoadOptions) -> Self {
        let mut files = Vec::new();
        collect_files(&root.join("data/stats/skills"), &mut files);
        if options.custom_skills_load {
            collect_files(&root.join("data/stats/skills/custom"), &mut files);
        }
        let parsed = parse_files(&files, groups, options);
        let table = Self::from_parsed(parsed.into_iter().flatten());
        log_loaded(Self::LOADER, table.loaded, "Skill templates from XML files");
        table
    }

    /// Builds the table from the text of one skill file. For tests and tools.
    pub fn from_xml(xml: &str, groups: &EnchantSkillGroups, options: &SkillLoadOptions) -> Self {
        Self::from_parsed(document::parse_document("<inline>", xml, groups, options))
    }

    /// Java `load()` after `loadAllSkills`: later skills replace earlier ones with the same hash,
    /// then the max levels and the enchantable set are derived.
    fn from_parsed(skills: impl IntoIterator<Item = SkillTemplate>) -> Self {
        let mut loaded = 0;
        let mut by_hash: HashMap<i32, SkillTemplate> = HashMap::new();
        for skill in skills {
            loaded += 1;
            by_hash.insert(skill.hash_code(), skill);
        }
        let mut skills: Vec<SkillTemplate> = by_hash.into_values().collect();
        skills.sort_by_key(|skill| (skill.id, skill.level));

        let mut table = Self {
            by_hash: skills
                .iter()
                .enumerate()
                .map(|(index, skill)| (skill.hash_code(), index))
                .collect(),
            skills,
            max_levels: HashMap::new(),
            enchantable: std::collections::HashSet::new(),
            loaded,
        };
        for skill in &table.skills {
            if skill.level > 99 {
                table.enchantable.insert(skill.id);
                continue;
            }
            // Only non-enchanted skills.
            let max = table.max_levels.entry(skill.id).or_insert(0);
            if skill.level > *max {
                *max = skill.level;
            }
        }
        table
    }

    /// The template of this id and level (Java `_skillsByHash.get(hash)`), no fallback.
    pub fn get(&self, skill_id: i32, level: i32) -> Option<&SkillTemplate> {
        self.by_hash
            .get(&Self::hash_code(skill_id, level))
            .and_then(|&index| self.skills.get(index))
    }

    /// Java `SkillData.getSkill`: like [`SkillTemplates::get`], but a level above the skill's max
    /// level returns the max level (with Java's warning), for the transformation scripts.
    pub fn get_skill(&self, skill_id: i32, level: i32) -> Option<&SkillTemplate> {
        if let Some(skill) = self.get(skill_id, level) {
            return Some(skill);
        }
        // Skill/level not found, fix for transformation scripts.
        let max_level = self.max_level(skill_id);
        if max_level > 0 && level > max_level {
            warn!(
                "SkillData: Call to unexisting skill level id: {skill_id} requested level: {level} max level: {max_level}."
            );
            return self.get(skill_id, max_level);
        }
        warn!("SkillData: No skill info found for skill id {skill_id} and skill level {level}.");
        None
    }

    /// Java `getMaxLevel`: the highest plain level of the skill (enchant levels do not count),
    /// 0 for an unknown id.
    pub fn max_level(&self, skill_id: i32) -> i32 {
        self.max_levels.get(&skill_id).copied().unwrap_or(0)
    }

    /// Java `isEnchantable`: the skill has levels above 99.
    pub fn is_enchantable(&self, skill_id: i32) -> bool {
        self.enchantable.contains(&skill_id)
    }

    /// Every template, ordered by id and level.
    pub fn iter(&self) -> impl Iterator<Item = &SkillTemplate> + '_ {
        self.skills.iter()
    }

    /// Number of templates in the table (a skill defined twice counts once).
    pub fn len(&self) -> usize {
        self.skills.len()
    }

    /// True when no skill was loaded.
    pub fn is_empty(&self) -> bool {
        self.skills.is_empty()
    }

    /// Java's `count`: the skills read from the files, duplicates included.
    pub fn loaded_count(&self) -> usize {
        self.loaded
    }
}

/// The `.xml` files of one folder (not recursive, like Java's `listFiles`), sorted by path so the
/// merge order does not depend on the OS. A missing folder is a warning.
fn collect_files(dir: &Path, out: &mut Vec<PathBuf>) {
    if !dir.exists() {
        warn!("Directory {} does not exist.", dir.display());
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .file_name()
                    .is_some_and(|name| name.to_string_lossy().to_lowercase().ends_with(".xml"))
        })
        .collect();
    files.sort();
    out.extend(files);
}

/// Parses every file on a few threads. The result has one entry per file, in file order.
fn parse_files(
    files: &[PathBuf],
    groups: &EnchantSkillGroups,
    options: &SkillLoadOptions,
) -> Vec<Vec<SkillTemplate>> {
    let threads = std::thread::available_parallelism()
        .map_or(4, |n| n.get())
        .clamp(1, 8)
        .min(files.len().max(1));
    let next = AtomicUsize::new(0);
    let mut parts: Vec<Vec<(usize, Vec<SkillTemplate>)>> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                scope.spawn(|| {
                    let mut done = Vec::new();
                    loop {
                        let index = next.fetch_add(1, Ordering::Relaxed);
                        let Some(path) = files.get(index) else {
                            break;
                        };
                        done.push((index, parse_file(path, groups, options)));
                    }
                    done
                })
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().unwrap_or_default())
            .collect()
    });
    let mut ordered: Vec<(usize, Vec<SkillTemplate>)> = parts.drain(..).flatten().collect();
    ordered.sort_by_key(|(index, _)| *index);
    ordered.into_iter().map(|(_, skills)| skills).collect()
}

fn parse_file(path: &Path, groups: &EnchantSkillGroups, options: &SkillLoadOptions) -> Vec<SkillTemplate> {
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    match std::fs::read(path) {
        Ok(bytes) => document::parse_document(&name, &String::from_utf8_lossy(&bytes), groups, options),
        Err(error) => {
            tracing::error!("Error loading file {}: {error}", path.display());
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_code_is_java_s() {
        assert_eq!(SkillTemplates::hash_code(141, 3), 141 * 1021 + 3);
    }

    #[test]
    fn groups_count_enchant_details() {
        let groups = EnchantSkillGroups::from_xml(
            r#"<list><group id="1"><enchant level="1"/><enchant level="2"/></group>
               <group id="2"><enchant level="1"/></group><group id="1"><enchant level="3"/></group></list>"#,
        );
        assert_eq!(groups.levels(1), Some(3));
        assert_eq!(groups.levels(2), Some(1));
        assert_eq!(groups.levels(9), None);
        assert_eq!(groups.add_new_route_for_skill(5, 1, 9), 0);
    }
}
