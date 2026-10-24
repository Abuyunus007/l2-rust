//! The fisherman's skill list: `FishSkillList` (the fishing skills of `fishingSkillTree.xml`).
// Java: entity/actor/instance/Fisherman.java (showFishSkillList)
// Java: data/xml/SkillTreeData.java (getAvailableFishingSkills, getMinLevelForNewSkill)
//
// Learning itself (`RequestAcquireSkillInfo` / `RequestAcquireSkill` of type `FISHING`) is the
// shared code in `skills/learn.rs`; this file is the list the fisherman shows and the race and
// level rules of the fishing tree.

use std::sync::Arc;

use l2_data::{ClassId, Race, SkillLearn};
use l2_protocol_game::SystemMessageId;
use l2_protocol_game::server::{AcquireSkillList, AcquireSkillType, SystemMessage, SystemMessageParam};
use l2_util::ObjectId;
use tracing::warn;

use crate::skills::learn::min_level_for_new_skill;
use crate::world::World;

impl World {
    /// Java `Player.getRace()`: the race of the base class (a sub-class of another race does not
    /// change it).
    fn race_of_player(&self, id: ObjectId) -> Option<Race> {
        let player = self.players.get(&id)?;
        let base = u16::try_from(player.data.base_class).ok().map(ClassId)?;
        base.race()
    }

    /// Java `SkillTreeData.getAvailableFishingSkills(player)`: the lines the player may learn
    /// next, in Java's order. Lines of another race are skipped (the Dwarven-only ones).
    pub(crate) fn available_fishing_skills(&self, player: ObjectId) -> Vec<SkillLearn> {
        let race = self.race_of_player(player);
        let level = self.core_of(player).map_or(1, |c| c.level);
        let existing = self.known_skills_of(player);
        let trees = Arc::clone(&self.rewards.skill_trees);
        let mut result = Vec::new();
        for skill in trees.fishing_skills() {
            // If skill is Race specific and the player's race isn't allowed, skip it.
            if !skill.races.is_empty() && !race.is_some_and(|r| skill.races.contains(&r)) {
                continue;
            }
            if skill.learned_by_npc && level >= skill.get_level {
                match existing.get(&skill.skill_id) {
                    Some(&old_level) => {
                        if old_level == skill.skill_level - 1 {
                            result.push(skill.clone());
                        }
                    }
                    None => {
                        if skill.skill_level == 1 {
                            result.push(skill.clone());
                        }
                    }
                }
            }
        }
        result
    }

    /// Java `Fisherman.showFishSkillList(player)`: the fishing skills the player may learn now,
    /// or the reason there are none.
    pub(crate) fn show_fish_skill_list(&mut self, player: ObjectId) {
        let skills = self.available_fishing_skills(player);
        let mut list = AcquireSkillList::new(AcquireSkillType::Fishing);
        let mut count = 0;
        for s in &skills {
            if !self.skill_template_exists(s.skill_id, s.skill_level) {
                continue;
            }
            count += 1;
            list.add_skill(s.skill_id, s.skill_level, s.skill_level, s.level_up_sp, 1);
        }

        if count == 0 {
            let level = self.core_of(player).map_or(1, |c| c.level);
            let trees = Arc::clone(&self.rewards.skill_trees);
            let tree: Vec<&SkillLearn> = trees.fishing_skills().iter().collect();
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
}
