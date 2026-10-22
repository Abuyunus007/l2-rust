//! Skill enchanting data: the enchant groups (costs and success chances per enchant level) and the
//! enchant routes of every skill.
//!
//! Java: `gameserver/data/xml/EnchantSkillGroupsData.java`,
//! `gameserver/mechanics/skill/holders/{EnchantSkillGroup,EnchantSkillLearn}.java`, and the route
//! part of `gameserver/util/DocumentSkill.java` (`parseSkill` calls `addNewRouteForSkill` for the
//! attributes `enchantGroup1`..`enchantGroup8` of a `<skill>`).
//! Data: `data/EnchantSkillGroups.xml` (`xsd/enchantSkillGroups.xsd`), `data/stats/skills/*.xml`.
//!
//! # Enchant levels
//! An enchanted skill has the level `route * 100 + n` (`101` is the first step of route 1, `201`
//! the first step of route 2). [`EnchantSkillLearn::enchant_route`] and friends are Java's static
//! helpers for that numbering. The skill templates of those levels are in [`crate::SkillTemplates`];
//! this table only knows what they cost and how likely they are.
//!
//! # Why the routes are read from the skill files again
//! `skill::EnchantSkillGroups` (the skill loader's view of the groups) only counts the levels of a
//! group, and the skill templates drop `enchantGroupN`. Java collects the routes while it parses the
//! skills, so this loader reads the `<skill>` attributes of the files that have any
//! (`enchantGroup` in the text) with the same DOM parser, in sorted file order. A skill id defined
//! twice keeps the base level of its first definition and the latest route of each number, like
//! Java's `HashMap`/`TreeMap` puts.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};

use tracing::{error, info, warn};

use crate::stats::XmlNode;
use crate::stats::jnum::{decode_i32, parse_int};
use crate::stats::node::parse_dom;

/// `EnchantSkillGroupsData.NORMAL_ENCHANT_BOOK`: "Spellbook: Skill Enchant" for a normal enchant.
pub const NORMAL_ENCHANT_BOOK: i32 = 6622;
/// `EnchantSkillGroupsData.SAFE_ENCHANT_BOOK`.
pub const SAFE_ENCHANT_BOOK: i32 = 9627;
/// `EnchantSkillGroupsData.CHANGE_ENCHANT_BOOK`.
pub const CHANGE_ENCHANT_BOOK: i32 = 9626;
/// `EnchantSkillGroupsData.UNTRAIN_ENCHANT_BOOK`.
pub const UNTRAIN_ENCHANT_BOOK: i32 = 9625;

/// The routes a skill can have (`enchantGroup1`..`enchantGroup8`).
const MAX_ROUTES: i32 = 8;
/// The first player level the success chances are listed for (`chance76`).
const FIRST_RATE_LEVEL: i32 = 76;
/// `chance76`..`chance99`.
const RATE_COUNT: usize = 24;

/// Java `EnchantSkillGroup.EnchantSkillHolder`: one enchant step of a group.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnchantSkillHolder {
    /// `level` (the step in the group, 1-based).
    pub level: i32,
    /// `adena` (default 0).
    pub adena_cost: i32,
    /// `exp` (default 0). Java reads it and never uses it.
    pub exp_cost: i32,
    /// `sp` (default 0).
    pub sp_cost: i32,
    /// `chance76`..`chance99` (default 0), the success rate by player level.
    rate: [i8; RATE_COUNT],
}

impl EnchantSkillHolder {
    /// Java `getRate(player)`: 0 below level 76, else the chance of the player's level.
    // NOTE(java-quirk): Java indexes the 24 entries with `level - 76` and throws past level 99;
    // no datapack level gets there, the rate is 0 here.
    pub fn rate(&self, player_level: i32) -> i8 {
        if player_level < FIRST_RATE_LEVEL {
            return 0;
        }
        usize::try_from(player_level - FIRST_RATE_LEVEL)
            .ok()
            .and_then(|index| self.rate.get(index))
            .copied()
            .unwrap_or(0)
    }
}

/// Java `EnchantSkillGroup`: the steps of one group, in file order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnchantSkillGroup {
    pub id: i32,
    pub details: Vec<EnchantSkillHolder>,
}

/// Java `EnchantSkillLearn`: the enchant routes of one skill.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EnchantSkillLearn {
    pub id: i32,
    /// The skill's last plain level (`levels` of the `<skill>`): the level an enchant starts from.
    pub base_level: i32,
    /// Route number to group id. Sorted, like Java's `TreeMap`.
    routes: BTreeMap<i32, i32>,
}

impl EnchantSkillLearn {
    /// Java `getEnchantRoute(level)`: `101` is route 1.
    pub fn enchant_route(level: i32) -> i32 {
        level / 100
    }

    /// Java `getEnchantIndex(level)`: the step inside the group, 0-based (`-1` for `x00`).
    pub fn enchant_index(level: i32) -> i32 {
        (level % 100) - 1
    }

    /// Java `getEnchantType(level)`.
    pub fn enchant_type(level: i32) -> i32 {
        ((level - 1) / 100) - 1
    }

    /// Java `getAllRoutes()`: the route numbers, ascending.
    pub fn all_routes(&self) -> impl Iterator<Item = i32> + '_ {
        self.routes.keys().copied()
    }

    /// Java `getMinSkillLevel(level)`: the level a player must have to take the step to `level`.
    pub fn min_skill_level(&self, level: i32) -> i32 {
        if level % 100 == 1 {
            self.base_level
        } else {
            level - 1
        }
    }
}

/// Java `EnchantSkillGroupsData`: the groups and the enchantable skills. Immutable once loaded.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EnchantSkillGroupsData {
    groups: HashMap<i32, EnchantSkillGroup>,
    trees: HashMap<i32, EnchantSkillLearn>,
}

impl EnchantSkillGroupsData {
    const LOADER: &'static str = "EnchantSkillGroupsData";

    /// Loads `<root>/data/EnchantSkillGroups.xml`, then the routes of the skill files (the folder
    /// `custom` too with `custom_skills_load`, `CustomSkillsLoad`). Logs Java's count line.
    pub fn load(root: &Path, custom_skills_load: bool) -> Self {
        let mut data = Self::default();
        let path = root.join("data/EnchantSkillGroups.xml");
        match std::fs::read(&path) {
            Ok(bytes) => data.add_groups_xml(&String::from_utf8_lossy(&bytes)),
            Err(error) => warn!("Cannot parse EnchantSkillGroups.xml: {error}"),
        }
        info!(
            "{}: Loaded {} groups and {} routes.",
            Self::LOADER,
            data.groups.len(),
            data.groups.values().map(|g| g.details.len()).sum::<usize>()
        );

        let mut files = Vec::new();
        collect_xml_files(&root.join("data/stats/skills"), &mut files);
        if custom_skills_load {
            collect_xml_files(&root.join("data/stats/skills/custom"), &mut files);
        }
        for file in files {
            let Ok(bytes) = std::fs::read(&file) else {
                continue;
            };
            let text = String::from_utf8_lossy(&bytes);
            // Most files have no enchantable skill; skip the parse for them.
            if text.contains("enchantGroup") {
                data.add_skills_xml(&text);
            }
        }
        data
    }

    /// Builds the groups from the text of `EnchantSkillGroups.xml` and the routes from the text of
    /// one skill file. For tests and tools.
    pub fn from_xml(groups_xml: &str, skills_xml: &str) -> Self {
        let mut data = Self::default();
        data.add_groups_xml(groups_xml);
        data.add_skills_xml(skills_xml);
        data
    }

    /// Java `parseDocument`: groups are merged by id, steps are appended.
    fn add_groups_xml(&mut self, xml: &str) {
        let root = match parse_dom(xml) {
            Ok(root) => root,
            Err(error) => {
                warn!("Error parsing EnchantSkillGroups.xml: {error}");
                return;
            }
        };
        if !root.name.eq_ignore_ascii_case("list") {
            return;
        }
        for group in root
            .children
            .iter()
            .filter(|g| g.name.eq_ignore_ascii_case("group"))
        {
            // `parseInteger(attrs, "id")`: a missing id fails the file, the groups before stay.
            let Some(id) = group.attr("id").and_then(decode_i32) else {
                warn!("Error parsing EnchantSkillGroups.xml: bad group id");
                return;
            };
            let entry = self.groups.entry(id).or_insert_with(|| EnchantSkillGroup {
                id,
                details: Vec::new(),
            });
            for enchant in group
                .children
                .iter()
                .filter(|e| e.name.eq_ignore_ascii_case("enchant"))
            {
                match parse_holder(enchant) {
                    Some(holder) => entry.details.push(holder),
                    None => {
                        warn!("Error parsing EnchantSkillGroups.xml: bad enchant of group {id}");
                        return;
                    }
                }
            }
        }
    }

    /// The route attributes of the `<skill>` elements of one skill file (`parseSkill`).
    fn add_skills_xml(&mut self, xml: &str) {
        let Ok(root) = parse_dom(xml) else {
            return;
        };
        if root.name.eq_ignore_ascii_case("skill") {
            self.add_skill_routes(&root);
        } else if root.name.eq_ignore_ascii_case("list") {
            for skill in root
                .children
                .iter()
                .filter(|s| s.name.eq_ignore_ascii_case("skill"))
            {
                self.add_skill_routes(skill);
            }
        }
    }

    fn add_skill_routes(&mut self, skill: &XmlNode) {
        let (Some(id), Some(levels)) = (
            skill.attr("id").and_then(parse_int),
            skill.attr("levels").and_then(parse_int),
        ) else {
            return;
        };
        for route in 1..=MAX_ROUTES {
            if let Some(group) = skill.attr(&format!("enchantGroup{route}")).and_then(parse_int) {
                self.add_new_route_for_skill(id, levels, route, group);
            }
        }
    }

    /// Java `addNewRouteForSkill`: the skill is created even when the group is missing (then it
    /// has no route and Java logs the error).
    fn add_new_route_for_skill(&mut self, skill_id: i32, max_level: i32, route: i32, group: i32) {
        let learn = self.trees.entry(skill_id).or_insert_with(|| EnchantSkillLearn {
            id: skill_id,
            base_level: max_level,
            routes: BTreeMap::new(),
        });
        if self.groups.contains_key(&group) {
            learn.routes.insert(route, group);
        } else {
            error!(
                "{}: Error while loading generating enchant skill id: {skill_id}; route: {route}; missing group: {group}",
                Self::LOADER
            );
        }
    }

    // ---- lookups -----------------------------------------------------------------------------

    /// Java `getSkillEnchantmentBySkillId(skillId)`.
    pub fn skill_enchantment_by_skill_id(&self, skill_id: i32) -> Option<&EnchantSkillLearn> {
        self.trees.get(&skill_id)
    }

    /// Java `getSkillEnchantmentForSkill(skill)`: the enchantment of a skill whose level reached
    /// the base level.
    pub fn skill_enchantment_for_skill(&self, skill_id: i32, level: i32) -> Option<&EnchantSkillLearn> {
        self.trees.get(&skill_id).filter(|esl| level >= esl.base_level)
    }

    /// Java `getEnchantSkillGroupById(id)`.
    pub fn enchant_skill_group_by_id(&self, id: i32) -> Option<&EnchantSkillGroup> {
        self.groups.get(&id)
    }

    /// Number of groups.
    pub fn group_count(&self) -> usize {
        self.groups.len()
    }

    /// Number of enchantable skills.
    pub fn skill_count(&self) -> usize {
        self.trees.len()
    }

    /// The group of route `route` of the skill.
    fn route_group(&self, learn: &EnchantSkillLearn, route: i32) -> Option<&EnchantSkillGroup> {
        learn.routes.get(&route).and_then(|g| self.groups.get(g))
    }

    /// Java `EnchantSkillLearn.getFirstRouteGroup()`: the group of the lowest route.
    pub fn first_route_group(&self, skill_id: i32) -> Option<&EnchantSkillGroup> {
        let learn = self.trees.get(&skill_id)?;
        let (_, group) = learn.routes.first_key_value()?;
        self.groups.get(group)
    }

    /// Java `EnchantSkillLearn.isMaxEnchant(level)`: the step is the last of its route.
    pub fn is_max_enchant(&self, skill_id: i32, level: i32) -> bool {
        let Some(learn) = self.trees.get(&skill_id) else {
            return false;
        };
        let route = EnchantSkillLearn::enchant_route(level);
        if route < 1 {
            return false;
        }
        let Some(group) = self.route_group(learn, route) else {
            return false;
        };
        let index = EnchantSkillLearn::enchant_index(level);
        i64::from(index) + 1 >= group.details.len() as i64
    }

    /// Java `EnchantSkillLearn.getEnchantSkillHolder(level)`: the step of `level`; below the first
    /// step the first, past the last the last. `None` for a level that is not on a route.
    pub fn enchant_skill_holder(&self, skill_id: i32, level: i32) -> Option<&EnchantSkillHolder> {
        let learn = self.trees.get(&skill_id)?;
        let route = EnchantSkillLearn::enchant_route(level);
        if route < 1 {
            return None;
        }
        let group = self.route_group(learn, route)?;
        let index = EnchantSkillLearn::enchant_index(level);
        match usize::try_from(index) {
            Err(_) => group.details.first(),
            Ok(index) => group.details.get(index).or_else(|| group.details.last()),
        }
    }

    /// Java `getEnchantSkillSpCost(skill)`: `i32::MAX` for a skill or level without data.
    pub fn enchant_skill_sp_cost(&self, skill_id: i32, level: i32) -> i32 {
        self.enchant_skill_holder(skill_id, level)
            .map_or(i32::MAX, |h| h.sp_cost)
    }

    /// Java `getEnchantSkillAdenaCost(skill)`: `i32::MAX` for a skill or level without data.
    pub fn enchant_skill_adena_cost(&self, skill_id: i32, level: i32) -> i32 {
        self.enchant_skill_holder(skill_id, level)
            .map_or(i32::MAX, |h| h.adena_cost)
    }

    /// Java `getEnchantSkillRate(player, skill)`: 0 for a skill or level without data.
    pub fn enchant_skill_rate(&self, player_level: i32, skill_id: i32, level: i32) -> i8 {
        self.enchant_skill_holder(skill_id, level)
            .map_or(0, |h| h.rate(player_level))
    }
}

/// Java `new EnchantSkillHolder(set)`: `level` is required, the rest default to 0. A number that
/// does not parse fails the file (`StatSet` throws).
fn parse_holder(node: &XmlNode) -> Option<EnchantSkillHolder> {
    let int = |name: &str, default: Option<i32>| match node.attr(name) {
        Some(text) => parse_int(text),
        None => default,
    };
    let mut rate = [0i8; RATE_COUNT];
    for (index, slot) in rate.iter_mut().enumerate() {
        let name = format!("chance{}", FIRST_RATE_LEVEL + index as i32);
        *slot = match node.attr(&name) {
            Some(text) => text.parse::<i8>().ok()?,
            None => 0,
        };
    }
    Some(EnchantSkillHolder {
        level: int("level", None)?,
        adena_cost: int("adena", Some(0))?,
        exp_cost: int("exp", Some(0))?,
        sp_cost: int("sp", Some(0))?,
        rate,
    })
}

/// The `.xml` files of one folder (not recursive), sorted.
fn collect_xml_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut files: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| {
            path.is_file()
                && path
                    .extension()
                    .is_some_and(|ext| ext.eq_ignore_ascii_case("xml"))
        })
        .collect();
    files.sort();
    out.extend(files);
}

#[cfg(test)]
mod tests {
    use super::*;

    const GROUPS: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list>
	<group id="1">
		<enchant level="1" adena="74250" exp="0" sp="575980" chance76="82" chance77="92" chance78="97" />
		<enchant level="2" adena="80000" sp="600000" chance76="80" chance85="95" />
		<enchant level="3" adena="90000" sp="700000" />
	</group>
	<group id="2">
		<enchant level="1" adena="1" sp="2" chance76="50" />
	</group>
</list>"#;

    const SKILLS: &str = r#"<list>
	<skill id="10" levels="37" name="A" enchantGroup1="1" enchantGroup2="2" enchantGroup3="9"></skill>
	<skill id="11" levels="5" name="B" enchantGroup2="2"></skill>
	<skill id="12" levels="5" name="no routes"></skill>
</list>"#;

    fn data() -> EnchantSkillGroupsData {
        EnchantSkillGroupsData::from_xml(GROUPS, SKILLS)
    }

    #[test]
    fn level_numbering_is_javas() {
        assert_eq!(EnchantSkillLearn::enchant_route(101), 1);
        assert_eq!(EnchantSkillLearn::enchant_route(237), 2);
        assert_eq!(EnchantSkillLearn::enchant_index(101), 0);
        assert_eq!(EnchantSkillLearn::enchant_index(200), -1);
        assert_eq!(EnchantSkillLearn::enchant_type(101), 0);
        assert_eq!(EnchantSkillLearn::enchant_type(201), 1);
        assert_eq!(EnchantSkillLearn::enchant_type(100), -1);
    }

    #[test]
    fn groups_and_routes_are_read() {
        let data = data();
        assert_eq!((data.group_count(), data.skill_count()), (2, 2));
        let a = data.skill_enchantment_by_skill_id(10).unwrap();
        assert_eq!(a.base_level, 37);
        // Route 3 names the missing group 9: no route, the skill still exists.
        assert_eq!(a.all_routes().collect::<Vec<_>>(), vec![1, 2]);
        assert!(
            data.skill_enchantment_by_skill_id(11)
                .is_some_and(|s| s.all_routes().eq([2]))
        );
        assert!(data.skill_enchantment_by_skill_id(12).is_none());
        assert_eq!(data.first_route_group(10).map(|g| g.id), Some(1));
        assert_eq!(data.first_route_group(11).map(|g| g.id), Some(2));
    }

    #[test]
    fn holders_follow_java_clamping() {
        let data = data();
        let step = |level| data.enchant_skill_holder(10, level).map(|h| h.level);
        assert_eq!(step(101), Some(1));
        assert_eq!(step(103), Some(3));
        // Past the last step: the last one. `x00` (index -1): the first one.
        assert_eq!(step(110), Some(3));
        assert_eq!(step(100 + 100), Some(1));
        // Not on a route (the plain levels), or a route the skill lacks.
        assert_eq!(step(37), None);
        assert_eq!(step(301), None);
        assert!(data.enchant_skill_holder(99, 101).is_none());
    }

    #[test]
    fn min_levels_and_max_enchant() {
        let data = data();
        let a = data.skill_enchantment_by_skill_id(10).unwrap();
        assert_eq!(a.min_skill_level(101), 37);
        assert_eq!(a.min_skill_level(102), 101);
        assert_eq!(a.min_skill_level(201), 37);
        assert!(!data.is_max_enchant(10, 102));
        assert!(data.is_max_enchant(10, 103));
        assert!(data.is_max_enchant(10, 150));
        assert!(!data.is_max_enchant(10, 37));
        assert!(!data.is_max_enchant(10, 301));
        assert!(data.is_max_enchant(10, 201));
    }

    #[test]
    fn costs_and_rates() {
        let data = data();
        assert_eq!(data.enchant_skill_sp_cost(10, 101), 575_980);
        assert_eq!(data.enchant_skill_adena_cost(10, 101), 74_250);
        assert_eq!(data.enchant_skill_sp_cost(10, 301), i32::MAX);
        assert_eq!(data.enchant_skill_adena_cost(99, 101), i32::MAX);
        assert_eq!(data.enchant_skill_rate(76, 10, 101), 82);
        assert_eq!(data.enchant_skill_rate(78, 10, 101), 97);
        // No value for level 79 and up in the first step.
        assert_eq!(data.enchant_skill_rate(79, 10, 101), 0);
        assert_eq!(data.enchant_skill_rate(85, 10, 102), 95);
        // Below 76, and past the 24 entries, there is no chance.
        assert_eq!(data.enchant_skill_rate(75, 10, 101), 0);
        assert_eq!(data.enchant_skill_rate(100, 10, 101), 0);
        assert_eq!(data.enchant_skill_rate(80, 99, 101), 0);
    }

    #[test]
    fn enchantment_for_skill_needs_the_base_level() {
        let data = data();
        assert!(data.skill_enchantment_for_skill(10, 36).is_none());
        assert!(data.skill_enchantment_for_skill(10, 37).is_some());
        assert!(data.skill_enchantment_for_skill(10, 103).is_some());
    }

    #[test]
    fn a_bad_group_file_keeps_what_was_read() {
        let data = EnchantSkillGroupsData::from_xml(
            r#"<list><group id="1"><enchant level="1" sp="5"/></group>
               <group id="2"><enchant level="x"/></group><group id="3"><enchant level="1"/></group></list>"#,
            "<list/>",
        );
        assert_eq!(data.group_count(), 2);
        assert!(data.enchant_skill_group_by_id(3).is_none());
    }
}
