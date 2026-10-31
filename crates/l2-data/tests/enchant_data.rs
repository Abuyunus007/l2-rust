#![allow(clippy::unwrap_used, clippy::expect_used)]
//! The skill enchant groups and the enchant options of the real Java datapack. Skipped when the
//! folder does not exist (CI).
//!
//! The folder defaults to `dist/game`; set `L2JMOBIUS_GAME_DIR` to use another.

use std::path::PathBuf;

use l2_data::skill::{EnchantSkillGroups, SkillLoadOptions};
use l2_data::{
    EnchantItemOptions, EnchantSkillGroupsData, EnchantSkillLearn, ItemTemplates, SkillTemplates,
    augmentation::AugmentationOptions,
};

fn game_dir() -> Option<PathBuf> {
    let dir = std::env::var_os("L2JMOBIUS_GAME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../dist/game")));
    if dir.join("data").is_dir() {
        Some(dir)
    } else {
        eprintln!("skipped: {} does not exist", dir.display());
        None
    }
}

#[test]
fn the_groups_of_the_datapack() {
    let Some(dir) = game_dir() else { return };
    let data = EnchantSkillGroupsData::load(&dir, false);
    // `EnchantSkillGroups.xml` has 4 groups and 90 `<enchant>` steps.
    assert_eq!(data.group_count(), 4);
    let steps: usize = [1, 2, 5, 6]
        .into_iter()
        .filter_map(|id| data.enchant_skill_group_by_id(id))
        .map(|g| g.details.len())
        .sum();
    assert_eq!(steps, 90);

    // Group 1, step 1: the first line of the file.
    let step = data.enchant_skill_group_by_id(1).unwrap().details[0].clone();
    assert_eq!((step.level, step.adena_cost, step.sp_cost), (1, 74_250, 575_980));
    assert_eq!(step.rate(76), 82);
    assert_eq!(step.rate(77), 92);
    assert_eq!(step.rate(85), 97);

    // Skill 1, Triple Slash: `levels="37"` with seven routes of group 2.
    let triple_slash = data.skill_enchantment_by_skill_id(1).unwrap();
    assert_eq!(triple_slash.base_level, 37);
    assert_eq!(
        triple_slash.all_routes().collect::<Vec<_>>(),
        (1..=7).collect::<Vec<_>>()
    );
    assert_eq!(triple_slash.min_skill_level(101), 37);
    assert_eq!(triple_slash.min_skill_level(207), 206);
}

/// The skill loader and this table read the same attributes: every enchant level of a skill
/// template is a step of a route this table knows.
#[test]
fn every_route_has_its_skill_templates() {
    let Some(dir) = game_dir() else { return };
    let data = EnchantSkillGroupsData::load(&dir, false);
    let groups = EnchantSkillGroups::load(&dir);
    let skills = SkillTemplates::load_with_groups(&dir, &groups, &SkillLoadOptions::default());
    assert!(data.skill_count() > 100);
    let mut checked = 0;
    for template in skills.iter().filter(|t| t.level > 99) {
        let learn = data
            .skill_enchantment_by_skill_id(template.id)
            .unwrap_or_else(|| panic!("skill {} has enchant levels but no routes", template.id));
        let route = EnchantSkillLearn::enchant_route(template.level);
        assert!(
            learn.all_routes().any(|r| r == route),
            "skill {} route {route}",
            template.id
        );
        let holder = data.enchant_skill_holder(template.id, template.level).unwrap();
        // The step number of the level is the step of the group (never clamped here).
        assert_eq!(holder.level, template.level % 100);
        checked += 1;
    }
    assert!(checked > 1000, "{checked}");
    assert!(skills.is_enchantable(1));
    assert_eq!(skills.max_level(1), 37);
}

#[test]
fn the_options_of_the_datapack() {
    let Some(dir) = game_dir() else { return };
    let items = ItemTemplates::load(&dir, false);
    let skills = SkillTemplates::load(&dir, &SkillLoadOptions::default());
    let options = AugmentationOptions::load(&dir, &skills);
    let table = EnchantItemOptions::load(&dir, &items, &options);
    let mut items = items;
    items.attach_enchant_options(&table);
    assert_eq!(
        items.get(15383).unwrap().enchant_options_at(4),
        [24808, 24855, 24902]
    );
    assert!(table.len() > 10);
    // `<item id="15383">`: level 0 has one option, level 4 has three.
    assert_eq!(table.get(15383, 0), Some([24949, 0, 0]));
    assert_eq!(table.get(15383, 4), Some([24808, 24855, 24902]));
    assert_eq!(table.get(15383, 9999), None);
}
