#![allow(clippy::unwrap_used, clippy::expect_used)]
//! Armor sets, enchant HP bonus, hit condition bonus and hennas. The real datapack checks are
//! skipped when the folder does not exist (CI). The rest are small inline files for the defaults,
//! the bounds and the Java rules that are easy to get wrong.
//!
//! The folder defaults to `dist/game`; set `L2JMOBIUS_GAME_DIR` to use
//! another. Expected numbers are the Java server's: 204 armor sets (13 more `<set>` elements are
//! commented out in the files), 6 enchant HP grades, 180 hennas.

use std::path::PathBuf;

use l2_data::armor_set::SkillHolder;
use l2_data::item::types::CrystalType;
use l2_data::{ArmorSets, ClassId, Datapack, EnchantHpBonus, Hennas, HitConditionBonus};

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
fn loads_every_table_with_javas_counts() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    assert_eq!(pack.armor_sets.len(), 204);
    assert_eq!(pack.enchant_hp_bonus.len(), 6);
    assert_eq!(pack.hennas.len(), 180);
}

#[test]
fn vorpal_armor_set_matches_the_xml() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    // Set 185 in s84_vorpal.xml: chest 15592.
    assert!(pack.armor_sets.is_armor_set(15592));
    let set = pack.armor_sets.get(15592).unwrap();
    assert_eq!(set.chest_id, 15592);
    assert_eq!(set.legs, vec![15595]);
    assert_eq!(set.head, vec![15589]);
    assert_eq!(set.gloves, vec![15598]);
    assert_eq!(set.feet, vec![15601]);
    assert_eq!(set.shield, vec![15604]);
    assert_eq!(
        set.skills,
        vec![
            SkillHolder { id: 3006, level: 1 },
            SkillHolder { id: 8400, level: 1 },
        ]
    );
    assert_eq!(set.shield_skills, vec![SkillHolder { id: 8495, level: 1 }]);
    assert_eq!(set.enchant6_skills, vec![SkillHolder { id: 8461, level: 1 }]);
    assert_eq!(
        (set.str_, set.con, set.dex, set.men, set.wit, set.int_),
        (-2, 2, 0, 0, 0, 0)
    );
}

#[test]
fn set_without_legs_or_shield_has_empty_lists() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    // Set 1 in no_grade.xml: wooden breastplate, no shield and no shield skill.
    let set = pack.armor_sets.get(23).unwrap();
    assert_eq!(set.legs, vec![2386]);
    assert!(set.gloves.is_empty());
    assert!(set.shield.is_empty());
    assert!(set.shield_skills.is_empty());
    assert_eq!(set.skills.len(), 2);
    assert_eq!(set.skills[1], SkillHolder { id: 3500, level: 1 });
    assert!(pack.armor_sets.get(999_999).is_none());
}

#[test]
fn enchant_hp_bonus_matches_the_xml() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let bonus = &pack.enchant_hp_bonus;
    // NONE: 0, 0, 0, 4, 8, 16, ... (20 levels).
    assert_eq!(bonus.hp_bonus(CrystalType::None, 5, false), 8);
    // D: 12 levels, the last one is 389.
    assert_eq!(bonus.hp_bonus(CrystalType::D, 4, false), 9);
    assert_eq!(bonus.hp_bonus(CrystalType::D, 12, false), 389);
    assert_eq!(bonus.hp_bonus(CrystalType::D, 20, false), 389);
    // Full-body armor: 389 * 1.5 = 583.5, truncated.
    assert_eq!(bonus.hp_bonus(CrystalType::D, 12, true), 583);
    // S80 and S84 use the S list (Java getCrystalTypePlus).
    assert_eq!(bonus.hp_bonus(CrystalType::S, 4, false), 17);
    assert_eq!(bonus.hp_bonus(CrystalType::S80, 4, false), 17);
    assert_eq!(bonus.hp_bonus(CrystalType::S84, 4, false), 17);
    assert_eq!(bonus.hp_bonus(CrystalType::D, 0, false), 0);
}

#[test]
fn hit_condition_bonus_matches_the_xml() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let hit = *pack.hit_condition_bonus;
    assert_eq!(
        (
            hit.front, hit.side, hit.back, hit.high, hit.low, hit.dark, hit.rain
        ),
        (0, 5, 10, 3, -3, -10, -3)
    );
}

#[test]
fn henna_matches_the_xml() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let henna = pack.hennas.get(1).unwrap();
    assert_eq!((henna.dye_id, henna.dye_item_id, henna.level), (1, 4445, 28));
    assert_eq!((henna.stat_str, henna.stat_con, henna.stat_dex), (1, -3, 0));
    assert_eq!((henna.stat_int, henna.stat_men, henna.stat_wit), (0, 0, 0));
    assert_eq!((henna.wear_fee, henna.wear_count), (37000, 10));
    assert_eq!((henna.cancel_fee, henna.cancel_count), (7400, 5));
    // Warrior (1) and Human Wizard (11) may wear it, Human Fighter (0) and class 2 may not.
    assert!(henna.is_allowed_class(ClassId(1)));
    assert!(henna.is_allowed_class(ClassId(11)));
    assert!(!henna.is_allowed_class(ClassId(2)));
    assert!(pack.hennas.get(999_999).is_none());
    let for_warrior = pack.hennas.henna_list(ClassId(1));
    assert!(for_warrior.iter().any(|h| h.dye_id == 1));
    assert!(for_warrior.iter().all(|h| h.is_allowed_class(ClassId(1))));
}

#[test]
fn armor_set_without_chest_is_keyed_by_zero_and_repeats_add_up() {
    let xml = r#"<list>
	<set id="1">
		<legs id="10" />
		<legs id="11" />
		<con val="2" />
		<con val="5" />
	</set>
</list>"#;
    let sets = ArmorSets::from_xml(xml).unwrap();
    let set = sets.get(0).unwrap();
    assert_eq!(set.legs, vec![10, 11]);
    // A repeated stat element keeps its last value.
    assert_eq!(set.con, 5);
    assert_eq!((set.dex, set.int_), (0, 0));
    assert!(set.skills.is_empty());
}

#[test]
fn later_set_with_the_same_chest_replaces_the_earlier_one() {
    let xml = r#"<list>
	<set><chest id="5" /><str val="1" /></set>
	<set><chest id="5" /><str val="2" /></set>
</list>"#;
    let sets = ArmorSets::from_xml(xml).unwrap();
    assert_eq!(sets.len(), 1);
    assert_eq!(sets.get(5).unwrap().str_, 2);
}

#[test]
fn armor_set_piece_without_an_id_is_rejected() {
    let xml = r#"<list><set><chest /></set></list>"#;
    assert!(ArmorSets::from_xml(xml).is_err());
}

#[test]
fn enchant_hp_unknown_grade_is_rejected() {
    let xml = r#"<list><enchantHP grade="Z"><bonus>1</bonus></enchantHP></list>"#;
    assert!(EnchantHpBonus::from_xml(xml).is_err());
}

#[test]
fn enchant_hp_s80_uses_the_s_list_and_later_grade_wins() {
    let xml = r#"<list>
	<enchantHP grade="S"><bonus>1</bonus><bonus>2</bonus></enchantHP>
	<enchantHP grade="S"><bonus>7</bonus><bonus>8</bonus></enchantHP>
	<enchantHP grade="S80"><bonus>100</bonus></enchantHP>
</list>"#;
    let bonus = EnchantHpBonus::from_xml(xml).unwrap();
    assert_eq!(bonus.len(), 2);
    assert_eq!(bonus.hp_bonus(CrystalType::S, 1, false), 7);
    // S80 has its own entry in the file, but Java looks up the plus grade (S).
    assert_eq!(bonus.hp_bonus(CrystalType::S80, 1, false), 7);
}

#[test]
fn enchant_hp_negative_level_gives_no_bonus() {
    let xml = r#"<list><enchantHP grade="A"><bonus>3</bonus></enchantHP></list>"#;
    let bonus = EnchantHpBonus::from_xml(xml).unwrap();
    assert_eq!(bonus.hp_bonus(CrystalType::A, -1, false), 0);
    assert_eq!(bonus.hp_bonus(CrystalType::A, 1, false), 3);
    // A grade without a list gives nothing, as in Java.
    assert_eq!(bonus.hp_bonus(CrystalType::B, 1, false), 0);
}

#[test]
fn hit_condition_missing_elements_are_zero() {
    let hit = HitConditionBonus::from_xml("<hitConditionBonus />").unwrap();
    assert_eq!(hit, HitConditionBonus::default());
}

#[test]
fn hit_condition_repeated_element_keeps_the_last_value() {
    let xml = r#"<hitConditionBonus>
	<side val="5" />
	<side val="-4" />
	<dark val="-10" />
</hitConditionBonus>"#;
    let hit = HitConditionBonus::from_xml(xml).unwrap();
    assert_eq!((hit.side, hit.dark, hit.high), (-4, -10, 0));
}

#[test]
fn henna_without_wear_element_is_rejected() {
    let xml = r#"<list>
	<henna dyeId="2" dyeItemId="4446" dyeLevel="28">
		<cancel count="5" fee="7400" />
	</henna>
</list>"#;
    assert!(Hennas::from_xml(xml).is_err());
}

#[test]
fn henna_defaults_stats_to_zero_and_drops_unknown_classes() {
    let xml = r#"<list>
	<henna dyeId="2" dyeItemId="4446" dyeLevel="28">
		<wear count="10" fee="37000" />
		<cancel count="5" fee="7400" />
		<classId>200</classId>
		<classId>2</classId>
	</henna>
</list>"#;
    let hennas = Hennas::from_xml(xml).unwrap();
    let henna = hennas.get(2).unwrap();
    assert_eq!((henna.stat_str, henna.stat_con, henna.stat_dex), (0, 0, 0));
    assert_eq!(henna.allowed_classes.len(), 1);
    assert!(henna.is_allowed_class(ClassId(2)));
}

#[test]
fn henna_with_the_same_dye_id_replaces_the_earlier_one() {
    let xml = r#"<list>
	<henna dyeId="3" dyeItemId="1" dyeLevel="1">
		<wear count="1" fee="1" /><cancel count="1" fee="1" />
	</henna>
	<henna dyeId="3" dyeItemId="2" dyeLevel="1">
		<wear count="1" fee="1" /><cancel count="1" fee="1" />
	</henna>
</list>"#;
    let hennas = Hennas::from_xml(xml).unwrap();
    assert_eq!(hennas.len(), 1);
    assert_eq!(hennas.get(3).unwrap().dye_item_id, 2);
}

/// `data/stats/augmentation/options`: 24985 `<option>` elements with unique ids; option 1 adds
/// 15.45 P. Def., option 14561 gives the active skill 3203, option 14568 a chance skill.
#[test]
fn augmentation_options_match_the_xml() {
    let Some(dir) = game_dir() else { return };
    let pack = Datapack::load(&dir).unwrap();
    let options = &pack.augmentation_options;
    assert_eq!(options.len(), 24985);
    let first = options.get(1).unwrap();
    assert_eq!(first.funcs.len(), 1);
    assert_eq!(first.funcs[0].stat, l2_data::Stat::PowerDefence);
    assert_eq!(first.funcs[0].value, 15.45);
    assert_eq!(
        options.get(14561).unwrap().active_skill.map(|s| (s.id, s.level)),
        Some((3203, 1))
    );
    let chance = &options.get(14568).unwrap().activation_skills;
    assert_eq!(chance.len(), 1);
    assert_eq!((chance[0].skill.id, chance[0].chance), (3081, 1.0));
    assert!(options.get(0).is_none());
}
