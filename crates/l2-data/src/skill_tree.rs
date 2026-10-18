//! Class skill trees, reduced to what character creation needs: skill id, skill level, the level a
//! skill is learned at and its auto-learn flags. Skill definitions and effects are not loaded here.
//!
//! Java: `gameserver/data/xml/SkillTreeData.java` (`classSkillTree` parsing, the parent class chain,
//! `getCompleteClassSkillTree`, `getAvailableSkills`) and `network/clientpackets/CharacterCreate.java`
//! (which skills a new character gets).
//! Data: `data/stats/players/skillTrees/**/*.xml` (`xsd/skillTrees.xsd`).
//!
//! Java keys every skill by `skillId * 1021 + skillLevel`. This table keys by `(skill_id, skill_level)`,
//! which orders and identifies the same skills as long as no skill level reaches 1021.

use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;
use tracing::warn;

use crate::class_list::ClassId;
use crate::loader::{DataError, load_dir, log_loaded};

/// Java `CommonSkill.DIVINE_INSPIRATION`. Auto-learn skips it unless `AutoLearnDivineInspiration`
/// is enabled in `config/Player.ini` (default `False`).
pub const DIVINE_INSPIRATION_ID: i32 = 1405;

/// One line of a skill tree. Java `mechanics/skill/holders/SkillLearn`, without the item, race,
/// pre-requisite, social class, residence and sub-class parts (not needed for character creation).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillLearn {
    /// Java `getSkillId()`.
    pub skill_id: i32,
    /// Java `getSkillLevel()`.
    pub skill_level: i32,
    /// Java `getGetLevel()`: the character level the skill is available from.
    pub get_level: i32,
    /// Java `isAutoGet()`: delivered automatically.
    pub auto_get: bool,
    /// Java `isLearnedByNpc()`.
    pub learned_by_npc: bool,
}

/// All class skill trees, immutable after loading. Java `SkillTreeData`, class part only.
#[derive(Debug, Default)]
pub struct SkillTrees {
    /// Java `_completeClassSkillTree`: for every class that has its own tree, the common skills plus
    /// the skills of the class and its ancestors (a skill of the same id and level defined lower in
    /// the chain replaces the one above it).
    complete: BTreeMap<ClassId, BTreeMap<(i32, i32), SkillLearn>>,
}

impl SkillTrees {
    const LOADER: &'static str = "SkillTreeData";

    /// Loads every file below `<root>/data/stats/players/skillTrees` and logs Java's count line
    /// (`Loaded N Class Skills for M class skill trees.`).
    pub fn load(root: &Path) -> Self {
        let mut gathered = Gathered::default();
        load_dir(
            &root.join("data/stats/players/skillTrees"),
            true,
            |doc: RawSkillTreeFile| {
                gathered.add(doc);
                Ok(())
            },
        );
        log_loaded(
            Self::LOADER,
            gathered.skill_count(),
            &format!("Class Skills for {} class skill trees", gathered.own.len()),
        );
        Self::build(gathered)
    }

    /// Builds the tables from the text of one skill tree file. For tests and tools.
    pub fn from_xml(xml: &str) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(xml).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut gathered = Gathered::default();
        gathered.add(doc);
        Ok(Self::build(gathered))
    }

    fn build(gathered: Gathered) -> Self {
        let Gathered { parents, common, own } = gathered;
        let mut complete = BTreeMap::new();
        for &class in own.keys() {
            let mut tree = common.clone();
            for ancestor in class_chain(&parents, class) {
                if let Some(own_tree) = own.get(&ancestor) {
                    tree.extend(own_tree.clone());
                }
            }
            complete.insert(class, tree);
        }
        Self { complete }
    }

    /// Java `getCompleteClassSkillTree(playerClass)`: every skill the class can have, in id and
    /// level order. Empty for a class without its own tree.
    pub fn complete_class_tree(&self, class: ClassId) -> impl Iterator<Item = &SkillLearn> + '_ {
        self.complete
            .get(&class)
            .into_iter()
            .flat_map(|tree| tree.values())
    }

    /// The skills a new character of `class` at `level` gets automatically. Java
    /// `SkillTreeData.getAvailableSkills(newChar, class, includeByFs = false, includeAutoGet = true)`
    /// as `CharacterCreate` calls it, for a character that knows no skill yet: the skill must be
    /// auto-get or learned by NPC, the level must reach its `get_level`, and only level 1 of each
    /// skill qualifies. Java does not check race here, so this does not either.
    ///
    /// `auto_learn_divine_inspiration` is the `AutoLearnDivineInspiration` setting.
    pub fn auto_get_skills(
        &self,
        class: ClassId,
        level: i32,
        auto_learn_divine_inspiration: bool,
    ) -> Vec<&SkillLearn> {
        self.complete_class_tree(class)
            .filter(|skill| auto_learn_divine_inspiration || skill.skill_id != DIVINE_INSPIRATION_ID)
            .filter(|skill| skill.auto_get || skill.learned_by_npc)
            .filter(|skill| level >= skill.get_level)
            .filter(|skill| skill.skill_level == 1)
            .collect()
    }

    /// Number of classes that have a complete tree.
    pub fn len(&self) -> usize {
        self.complete.len()
    }

    /// True when no class tree was loaded.
    pub fn is_empty(&self) -> bool {
        self.complete.is_empty()
    }
}

/// The class and its ancestors, root first. Java `getCompleteClassSkillTree` walks the same chain.
fn class_chain(parents: &BTreeMap<ClassId, Option<ClassId>>, class: ClassId) -> Vec<ClassId> {
    let mut chain = Vec::new();
    let mut current = Some(class);
    while let Some(c) = current {
        if chain.contains(&c) {
            // NOTE(java-quirk): Java loops forever on a parent cycle. The data has none; stop instead.
            warn!("SkillTreeData: parent class cycle at class {}.", c.0);
            break;
        }
        chain.push(c);
        current = parents.get(&c).copied().flatten();
    }
    chain.reverse();
    chain
}

/// Java `PlayerClass.getPlayerClass(id)`: `None` for ids that are not player classes.
fn player_class(id: i32) -> Option<ClassId> {
    u16::try_from(id)
        .ok()
        .map(ClassId)
        .filter(|class| class.race().is_some())
}

/// What the files say, before the complete trees are built.
#[derive(Default)]
struct Gathered {
    /// Java `_parentClassMap`: the first parent a class is given wins. `None` when the parent id
    /// is not a player class.
    parents: BTreeMap<ClassId, Option<ClassId>>,
    /// Java `_commonSkillTree`: `classSkillTree` entries without a `classId`.
    common: BTreeMap<(i32, i32), SkillLearn>,
    /// Java `_classSkillTrees`: the own tree of every class.
    own: BTreeMap<ClassId, BTreeMap<(i32, i32), SkillLearn>>,
}

impl Gathered {
    fn add(&mut self, doc: RawSkillTreeFile) {
        for tree in doc.trees {
            let class_id = tree.class_id.unwrap_or(-1);
            // Java reads the parent from every skill tree element that has both ids, whatever its type.
            if let Some(parent_id) = tree.parent_class_id {
                if class_id > -1 && parent_id > -1 && class_id != parent_id {
                    if let Some(class) = player_class(class_id) {
                        self.parents
                            .entry(class)
                            .or_insert_with(|| player_class(parent_id));
                    }
                }
            }
            // Only class trees are kept. Other trees (transfer, pledge, fishing, ...) are not M2 data.
            if tree.kind != "classSkillTree" {
                continue;
            }
            if class_id <= -1 {
                for skill in tree.skills {
                    let learn = skill.into_learn();
                    self.common.insert((learn.skill_id, learn.skill_level), learn);
                }
                continue;
            }
            let Some(class) = player_class(class_id) else {
                warn!("SkillTreeData: skill tree for unknown class id {class_id} skipped.");
                continue;
            };
            let own = self.own.entry(class).or_default();
            for skill in tree.skills {
                let learn = skill.into_learn();
                own.insert((learn.skill_id, learn.skill_level), learn);
            }
        }
    }

    /// Java `getClassSkillTree` count: skills in all own trees.
    fn skill_count(&self) -> usize {
        self.own.values().map(BTreeMap::len).sum()
    }
}

// ---- Raw XML shape (xsd/skillTrees.xsd) ----------------------------------------------------
// Attributes without a default are the ones Java reads without one (a missing value is an error).

#[derive(Deserialize)]
struct RawSkillTreeFile {
    #[serde(rename = "skillTree", default)]
    trees: Vec<RawSkillTree>,
}

#[derive(Deserialize)]
struct RawSkillTree {
    #[serde(rename = "@type")]
    kind: String,
    #[serde(rename = "@classId")]
    class_id: Option<i32>,
    #[serde(rename = "@parentClassId")]
    parent_class_id: Option<i32>,
    #[serde(rename = "skill", default)]
    skills: Vec<RawSkill>,
}

#[derive(Deserialize)]
struct RawSkill {
    #[serde(rename = "@skillId")]
    skill_id: i32,
    #[serde(rename = "@skillLevel")]
    skill_level: i32,
    #[serde(rename = "@getLevel")]
    get_level: i32,
    #[serde(rename = "@autoGet", default)]
    auto_get: bool,
    #[serde(rename = "@learnedByNpc", default)]
    learned_by_npc: bool,
}

impl RawSkill {
    fn into_learn(self) -> SkillLearn {
        SkillLearn {
            skill_id: self.skill_id,
            skill_level: self.skill_level,
            get_level: self.get_level,
            auto_get: self.auto_get,
            learned_by_npc: self.learned_by_npc,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<list xmlns:xsi="http://www.w3.org/2001/XMLSchema-instance" xsi:noNamespaceSchemaLocation="../../../../xsd/skillTrees.xsd">
	<skillTree type="classSkillTree" classId="0">
		<skill skillName="Lucky" skillId="194" skillLevel="1" getLevel="1" autoGet="true" />
		<skill skillName="Sword Mastery" skillId="205" skillLevel="1" getLevel="1" learnedByNpc="true" />
		<skill skillName="Expertise D" skillId="239" skillLevel="1" getLevel="20" autoGet="true" />
		<skill skillName="Plain" skillId="100" skillLevel="1" getLevel="1" />
		<skill skillName="Stun" skillId="100" skillLevel="2" getLevel="1" autoGet="true" />
		<skill skillName="Divine Inspiration" skillId="1405" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
	<skillTree type="classSkillTree" classId="1" parentClassId="0">
		<skill skillName="Warrior skill" skillId="300" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
	<skillTree type="classSkillTree" classId="4" parentClassId="0">
		<skill skillName="Knight skill" skillId="410" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
	<skillTree type="classSkillTree" classId="4" parentClassId="1">
		<skill skillName="Knight skill 2" skillId="411" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
	<skillTree type="transferSkillTree" classId="16">
		<skill skillName="Transfer" skillId="500" skillLevel="1" getLevel="1" autoGet="true" />
	</skillTree>
</list>"#;

    fn keys(skills: &[&SkillLearn]) -> Vec<(i32, i32)> {
        skills.iter().map(|s| (s.skill_id, s.skill_level)).collect()
    }

    #[test]
    fn level_one_auto_get_and_npc_skills_are_given() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        let skills = trees.auto_get_skills(ClassId(0), 1, false);
        // Lucky (auto-get) and Sword Mastery (learned by NPC). Expertise D needs level 20, the plain
        // skill has no flag, Stun level 2 needs level 1 of Stun first, Divine Inspiration is off.
        assert_eq!(keys(&skills), vec![(194, 1), (205, 1)]);
    }

    #[test]
    fn get_level_gates_the_skill() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        let skills = trees.auto_get_skills(ClassId(0), 20, false);
        assert_eq!(keys(&skills), vec![(194, 1), (205, 1), (239, 1)]);
    }

    #[test]
    fn divine_inspiration_follows_the_setting() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        let skills = trees.auto_get_skills(ClassId(0), 1, true);
        assert_eq!(keys(&skills), vec![(194, 1), (205, 1), (1405, 1)]);
    }

    #[test]
    fn parent_skills_are_inherited() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        let skills = trees.auto_get_skills(ClassId(1), 1, false);
        assert_eq!(keys(&skills), vec![(194, 1), (205, 1), (300, 1)]);
    }

    #[test]
    fn first_parent_wins_and_parent_zero_counts() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        // Class 4 names parent 0 first, so class 1's skill 300 is not inherited.
        let skills = trees.auto_get_skills(ClassId(4), 1, false);
        assert_eq!(keys(&skills), vec![(194, 1), (205, 1), (410, 1), (411, 1)]);
    }

    #[test]
    fn class_without_own_tree_has_nothing() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        assert!(trees.auto_get_skills(ClassId(2), 1, false).is_empty());
        assert_eq!(trees.complete_class_tree(ClassId(2)).count(), 0);
    }

    #[test]
    fn transfer_trees_are_not_class_trees() {
        let trees = SkillTrees::from_xml(XML).unwrap();
        assert!(trees.auto_get_skills(ClassId(16), 1, false).is_empty());
        assert_eq!(trees.len(), 3);
    }

    #[test]
    fn common_skills_reach_every_class_tree() {
        let xml = r#"<list>
			<skillTree type="classSkillTree"><skill skillId="7" skillLevel="1" getLevel="1" autoGet="true" /></skillTree>
			<skillTree type="classSkillTree" classId="0"><skill skillId="8" skillLevel="1" getLevel="1" autoGet="true" /></skillTree>
			<skillTree type="classSkillTree" classId="1" parentClassId="0"></skillTree>
		</list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        assert_eq!(
            keys(&trees.auto_get_skills(ClassId(0), 1, false)),
            vec![(7, 1), (8, 1)]
        );
        // A class with an empty own tree still counts and inherits.
        assert_eq!(
            keys(&trees.auto_get_skills(ClassId(1), 1, false)),
            vec![(7, 1), (8, 1)]
        );
    }

    #[test]
    fn parent_cycle_stops_instead_of_hanging() {
        let xml = r#"<list>
			<skillTree type="classSkillTree" classId="2" parentClassId="3"><skill skillId="20" skillLevel="1" getLevel="1" autoGet="true" /></skillTree>
			<skillTree type="classSkillTree" classId="3" parentClassId="2"><skill skillId="30" skillLevel="1" getLevel="1" autoGet="true" /></skillTree>
		</list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        assert_eq!(
            keys(&trees.auto_get_skills(ClassId(2), 1, false)),
            vec![(20, 1), (30, 1)]
        );
    }

    #[test]
    fn unknown_class_id_is_skipped() {
        let xml = r#"<list>
			<skillTree type="classSkillTree" classId="58"><skill skillId="9" skillLevel="1" getLevel="1" autoGet="true" /></skillTree>
		</list>"#;
        let trees = SkillTrees::from_xml(xml).unwrap();
        assert!(trees.is_empty());
    }

    #[test]
    fn missing_required_attribute_is_an_error() {
        let xml = r#"<list><skillTree type="classSkillTree" classId="0">
			<skill skillId="1" skillLevel="1" autoGet="true" />
		</skillTree></list>"#;
        assert!(matches!(SkillTrees::from_xml(xml), Err(DataError::Xml { .. })));
    }
}
