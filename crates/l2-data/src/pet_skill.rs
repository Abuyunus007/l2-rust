//! The skills of summons and pets by NPC id, from `data/PetSkillData.xml`.
//!
//! Java: `gameserver/data/xml/PetSkillData.java`. Data: `data/PetSkillData.xml`,
//! `xsd/PetSkillData.xsd`.
//!
//! Every `<skill npcId=".." skillId=".." skillLevel=".."/>` gives the NPC a skill; level 0 means
//! "by the summon's level" (`getAvailableLevel`). Java keeps an entry only when the skill exists
//! (`SkillData.getSkill(skillId, skillLevel == 0 ? 1 : skillLevel)`), logging the others, and keys
//! the entries of one NPC by `getSkillHashCode(skillId, skillLevel + 1)`, so a repeated
//! `(skillId, skillLevel)` is kept once.
//!
//! NOTE(java-quirk): Java walks the entries of one NPC in `HashMap` order; here they stay in file
//! order. The order only matters for an NPC that lists the same skill both with level 0 and with
//! a fixed level, which the shipped file never does.

use std::collections::HashMap;
use std::path::Path;

use serde::Deserialize;
use tracing::{info, warn};

use crate::loader::{DataError, load_file, log_loaded};

/// One `(skillId, skillLevel)` of an NPC (Java `SkillHolder`); level 0 is "by level".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PetSkill {
    pub skill_id: i32,
    pub skill_level: i32,
}

/// Java `PetSkillData._skillTrees`. Immutable once loaded.
#[derive(Debug, Clone, Default)]
pub struct PetSkillData {
    by_npc: HashMap<i32, Vec<PetSkill>>,
}

impl PetSkillData {
    /// Loads `data/PetSkillData.xml` below `root`. `skill_exists(id, level)` is Java's
    /// `SkillData.getSkill(id, level) != null`.
    pub fn load(root: &Path, skill_exists: &dyn Fn(i32, i32) -> bool) -> Self {
        let mut table = Self::default();
        load_file::<RawFile, _>(&root.join("data").join("PetSkillData.xml"), |doc| {
            table.add_document(doc, skill_exists);
            Ok(())
        });
        log_loaded("PetSkillData", table.by_npc.len(), "skills");
        table
    }

    /// Parses one `PetSkillData.xml` text (tests, tools).
    pub fn from_xml(text: &str, skill_exists: &dyn Fn(i32, i32) -> bool) -> Result<Self, DataError> {
        let doc = quick_xml::de::from_str(text).map_err(|source| DataError::Xml {
            path: "<inline>".into(),
            source,
        })?;
        let mut table = Self::default();
        table.add_document(doc, skill_exists);
        Ok(table)
    }

    // Java: PetSkillData.parseDocument
    fn add_document(&mut self, doc: RawFile, skill_exists: &dyn Fn(i32, i32) -> bool) {
        for child in doc.children {
            let RawChild::Skill(raw) = child else { continue };
            let (Some(npc_id), Some(skill_id), Some(skill_level)) = (
                parse(&raw.npc_id),
                parse(&raw.skill_id),
                parse(&raw.skill_level),
            ) else {
                warn!("PetSkillData: skipping a <skill> without a valid npcId, skillId or skillLevel.");
                continue;
            };
            // `_skillTrees.put(npcId, new HashMap)` comes before the skill check: an NPC whose
            // skills are all unknown still counts in the loaded total.
            let tree = self.by_npc.entry(npc_id).or_default();
            if skill_exists(skill_id, if skill_level == 0 { 1 } else { skill_level }) {
                let skill = PetSkill { skill_id, skill_level };
                if !tree.contains(&skill) {
                    tree.push(skill);
                }
            } else {
                info!("PetSkillData: Could not find skill with id {skill_id}, level {skill_level} for NPC {npc_id}.");
            }
        }
    }

    /// The skills of an NPC, in file order (`_skillTrees.get(npcId).values()`).
    pub fn skills_of(&self, npc_id: i32) -> Option<&[PetSkill]> {
        self.by_npc.get(&npc_id).map(Vec::as_slice)
    }

    /// The number of NPCs with an entry (Java's "Loaded N skills").
    pub fn len(&self) -> usize {
        self.by_npc.len()
    }

    pub fn is_empty(&self) -> bool {
        self.by_npc.is_empty()
    }
}

fn parse(value: &Option<String>) -> Option<i32> {
    value.as_deref()?.trim().parse().ok()
}

#[derive(Deserialize)]
struct RawFile {
    #[serde(rename = "$value", default)]
    children: Vec<RawChild>,
}

#[derive(Deserialize)]
enum RawChild {
    #[serde(rename = "skill")]
    Skill(RawSkill),
    #[serde(other)]
    Other,
}

#[derive(Deserialize)]
struct RawSkill {
    #[serde(rename = "@npcId")]
    npc_id: Option<String>,
    #[serde(rename = "@skillId")]
    skill_id: Option<String>,
    #[serde(rename = "@skillLevel")]
    skill_level: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_skills_are_dropped_and_repeats_kept_once() {
        let data = PetSkillData::from_xml(
            r#"<list>
              <!-- a comment -->
              <skill npcId="14038" skillId="4260" skillLevel="0" />
              <skill npcId="14038" skillId="4260" skillLevel="0" />
              <skill npcId="14038" skillId="4138" skillLevel="3" />
              <skill npcId="99" skillId="1" skillLevel="1" />
            </list>"#,
            &|id, level| id != 1 && level >= 1,
        )
        .unwrap();
        assert_eq!(data.len(), 2, "the NPC with only an unknown skill still counts");
        assert_eq!(
            data.skills_of(14038).unwrap(),
            &[
                PetSkill {
                    skill_id: 4260,
                    skill_level: 0
                },
                PetSkill {
                    skill_id: 4138,
                    skill_level: 3
                }
            ]
        );
        assert_eq!(data.skills_of(99).unwrap(), &[]);
        assert!(data.skills_of(1).is_none());
    }
}
