//! Tests of the new character plan against a small datapack (see `testdata`) and against the
//! numbers a Java server was recorded to produce.

use std::collections::HashSet;
use std::sync::Arc;
use std::sync::atomic::{AtomicI32, Ordering};

use l2_config::GameConfig;
use l2_data::{Datapack, InitialEquipments, ItemTemplates, MacroCommand, MacroType};
use l2_db::ItemLocation;
use l2_util::WorldRng;

use super::newchar::*;
use super::testdata::datapack;

const CHAR_ID: i32 = 0x6000_0000;

struct Env {
    _dir: tempfile::TempDir,
    pack: Arc<Datapack>,
    bonus: StatBonus,
    max_mp: ItemMaxMp,
}

fn env() -> Env {
    let (dir, pack) = datapack();
    let bonus = StatBonus::load(&pack.root);
    let max_mp = ItemMaxMp::load(&pack.root, &ItemMaxMp::wanted_ids(&pack), false);
    Env {
        _dir: dir,
        pack,
        bonus,
        max_mp,
    }
}

fn plan_with(
    env: &Env,
    pack: &Datapack,
    config: &GameConfig,
    class: i32,
    female: bool,
) -> Result<NewCharacterPlan, PlanError> {
    let ctx = PlanContext {
        datapack: pack,
        config,
        stat_bonus: &env.bonus,
        item_max_mp: &env.max_mp,
    };
    let next = AtomicI32::new(CHAR_ID + 1);
    let allocate = || Some(next.fetch_add(1, Ordering::Relaxed));
    let mut ids = IdSource::new(&allocate);
    let request = CreateRequest {
        account: "acc",
        name: "Hero",
        is_female: female,
        hair_style: 2,
        hair_color: 1,
        face: 1,
    };
    let template = pack.player_templates.get_by_id(class).unwrap();
    build_plan(
        &ctx,
        &request,
        template,
        CHAR_ID,
        &mut ids,
        &mut WorldRng::from_seed(1),
        1_700_000_000_000,
    )
}

fn plan(env: &Env, class: i32) -> NewCharacterPlan {
    plan_with(env, &env.pack, &GameConfig::default(), class, false).unwrap()
}

#[test]
fn a_human_fighter_gets_what_the_java_server_gave() {
    let env = env();
    let p = plan(&env, 0);
    // Level 1 stats: 80 * CON bonus 1.58, 30 * MEN bonus 1.28, 32 * 1.58.
    assert_eq!((p.insert.max_hp, p.insert.max_mp, p.insert.max_cp), (126, 38, 50));
    assert_eq!(
        (p.insert.cur_hp, p.insert.cur_mp, p.insert.cur_cp),
        (0.0, 0.0, 0.0)
    );
    assert_eq!((p.update.max_hp, p.update.max_mp, p.update.max_cp), (126, 38, 50));
    assert_eq!(
        (p.update.cur_hp, p.update.cur_mp, p.update.cur_cp),
        (126.0, 38.0, 0.0)
    );
    assert_eq!(p.update.level, 1);
    assert_eq!(p.update.title.as_deref(), Some(""));
    assert_eq!(p.insert.title, None);
    assert_eq!(p.update.vitality_points, 20000);
    assert_eq!((p.update.x, p.update.y, p.update.z), (-71338, 258271, -3104));
    assert_eq!(p.update.online, 0);
    assert_eq!(p.insert.newbie, 1);
    assert!(!p.insert.can_craft);
    assert_eq!(p.reco, (0, 20, 0));

    // Items in the order of initialEquipment.xml, ids in the same order after the character's.
    let rows: Vec<_> = p
        .items
        .iter()
        .map(|i| (i.item_id, i.count, i.loc, i.loc_data, i.object_id - CHAR_ID))
        .collect();
    assert_eq!(
        rows,
        [
            (2369, 1, ItemLocation::Paperdoll, 5, 1),
            (10, 1, ItemLocation::Inventory, 0, 2),
            (1146, 1, ItemLocation::Paperdoll, 6, 3),
            (1147, 1, ItemLocation::Paperdoll, 11, 4),
            (5588, 1, ItemLocation::Inventory, 0, 5),
            (10650, 5, ItemLocation::Inventory, 0, 6),
        ]
    );
    assert!(
        p.items
            .iter()
            .all(|i| i.owner_id == CHAR_ID && i.mana_left == -1 && i.time == -1)
    );
    assert_eq!(p.object_ids.len(), 7);

    let skills: Vec<_> = p.skills.iter().map(|s| (s.skill_id, s.skill_level)).collect();
    assert_eq!(skills, [(194, 1), (1322, 1)]);

    // Attack, pick up, sit/stand and the tutorial guide's object id.
    let bar: Vec<_> = p
        .shortcuts
        .iter()
        .map(|s| (s.slot, s.page, s.kind, s.shortcut_id))
        .collect();
    assert_eq!(
        bar,
        [(0, 0, 3, 2), (3, 0, 3, 5), (10, 0, 3, 0), (11, 0, 1, CHAR_ID + 5)]
    );
    assert!(p.macros.is_empty());
}

#[test]
fn clothes_add_max_mp_before_the_men_multiplier() {
    let env = env();
    let p = plan_with(&env, &env.pack, &GameConfig::default(), 10, true).unwrap();
    // The Java capture: created with 58 MP, stored with (40 + 19 + 10) * 1.45 = 100.
    assert_eq!((p.insert.max_hp, p.insert.max_mp), (95, 58));
    assert_eq!((p.update.max_hp, p.update.max_mp), (95, 100));
    assert_eq!((p.update.cur_hp, p.update.cur_mp), (95.0, 58.0));
    assert_eq!(p.update.sex, 1);
    let skills: Vec<_> = p.skills.iter().map(|s| s.skill_id).collect();
    assert_eq!(skills, [194, 1177, 1216, 1322]);
    let bar: Vec<_> = p
        .shortcuts
        .iter()
        .map(|s| (s.slot, s.kind, s.shortcut_id, s.level))
        .collect();
    assert_eq!(
        bar,
        [
            (0, 3, 2, 0),
            (1, 2, 1177, 1),
            (3, 3, 5, 0),
            (9, 2, 1216, 1),
            (10, 3, 0, 0),
            (11, 1, CHAR_ID + 4, 0)
        ]
    );
    let equipped: Vec<_> = p
        .items
        .iter()
        .filter(|i| i.loc == ItemLocation::Paperdoll)
        .map(|i| (i.item_id, i.loc_data))
        .collect();
    assert_eq!(equipped, [(6, 5), (425, 6), (461, 11)]);
}

#[test]
fn starting_values_follow_the_config() {
    let env = env();
    let mut config = GameConfig::default();
    config.player.starting_adena = 5_000;
    config.player.starting_level = 3;
    config.player.starting_sp = 777;
    config.player.enable_vitality = false;
    config.starting_title.enable_custom_starting_title = true;
    config.starting_title.custom_starting_title = "Newbie".into();
    config.starting_location.custom_starting_loc = true;
    config.starting_location.custom_starting_loc_x = 1;
    config.starting_location.custom_starting_loc_y = 2;
    config.starting_location.custom_starting_loc_z = 3;
    let p = plan_with(&env, &env.pack, &config, 0, false).unwrap();
    // Adena first, with the first item id.
    assert_eq!(
        (
            p.items[0].item_id,
            p.items[0].count,
            p.items[0].object_id - CHAR_ID
        ),
        (57, 5_000, 1)
    );
    assert_eq!(p.update.level, 3);
    assert_eq!(p.update.exp, 363); // experience.xml: level 3
    assert_eq!(p.update.sp, 777);
    assert_eq!(p.update.title.as_deref(), Some("Newbie"));
    assert_eq!(p.update.vitality_points, 1);
    assert_eq!((p.update.x, p.update.y, p.update.z), (1, 2, 3));
    // Level 3 stats; HP, MP and CP are full (PlayerStat.addLevel), before the items.
    assert_eq!((p.update.max_hp, p.update.max_mp, p.update.max_cp), (163, 52, 65));
    assert_eq!(
        (p.update.cur_hp, p.update.cur_mp, p.update.cur_cp),
        (163.0, 52.0, 65.0)
    );
    // Level 3 gives Expertise D (auto-get, level 1) and Power Strike (learned by NPC, level 1).
    let skills: Vec<_> = p.skills.iter().map(|s| (s.skill_id, s.skill_level)).collect();
    assert_eq!(skills, [(3, 1), (194, 1), (1322, 1)]);
}

fn skills_of(p: &NewCharacterPlan) -> Vec<(i32, i32)> {
    p.skills.iter().map(|s| (s.skill_id, s.skill_level)).collect()
}

/// `AutoLearnSkills` (Java `Player.rewardSkills` -> `giveAvailableSkills`): every level of the
/// class tree the character level allows, not just level 1 of the learned-by-NPC skills.
#[test]
fn auto_learn_skills_gives_every_level_the_tree_allows() {
    let env = env();
    let mut config = GameConfig::default();
    config.player.starting_level = 3;
    // Off: Power Strike 1 only (the tail of the plan), Expertise is not out yet.
    let p = plan_with(&env, &env.pack, &config, 0, false).unwrap();
    assert_eq!(skills_of(&p), [(3, 1), (194, 1), (1322, 1)]);

    config.player.auto_learn_skills = true;
    let p = plan_with(&env, &env.pack, &config, 0, false).unwrap();
    assert_eq!(skills_of(&p), [(3, 2), (194, 1), (1322, 1)]);

    // Level 5: Expertise D (level 4) and C (level 5) follow each other round by round.
    config.player.starting_level = 5;
    let p = plan_with(&env, &env.pack, &config, 0, false).unwrap();
    assert_eq!(skills_of(&p), [(3, 2), (194, 1), (239, 2), (1322, 1)]);

    config.player.auto_learn_skills = false;
    let p = plan_with(&env, &env.pack, &config, 0, false).unwrap();
    assert_eq!(skills_of(&p), [(3, 1), (194, 1), (239, 2), (1322, 1)]);
}

/// `AutoLearnSkillsWithoutItems` and `AutoLearnForgottenScrollSkills` decide about the skills
/// that need items (`SkillTreeData.getAvailableSkills`).
#[test]
fn auto_learn_skills_honours_the_item_and_forgotten_scroll_flags() {
    let env = env();
    let mut pack = (*env.pack).clone();
    pack.skill_trees = Arc::new(
        l2_data::SkillTrees::from_xml(
            r#"<list><skillTree type="classSkillTree" classId="0">
			<skill skillName="Plain" skillId="10" skillLevel="1" getLevel="1" learnedByNpc="true" />
			<skill skillName="Book" skillId="11" skillLevel="1" getLevel="1" learnedByNpc="true"><item id="8618" count="1" /></skill>
			<skill skillName="Book2" skillId="11" skillLevel="2" getLevel="1" learnedByNpc="true"><item id="8618" count="1" /></skill>
			<skill skillName="Scroll" skillId="12" skillLevel="1" getLevel="1" learnedByFS="true"><item id="8619" count="1" /></skill>
			<skill skillName="TooHigh" skillId="13" skillLevel="1" getLevel="4" learnedByNpc="true" />
		</skillTree></list>"#,
        )
        .unwrap(),
    );
    let mut config = GameConfig::default();
    config.player.starting_level = 3;
    config.player.auto_learn_skills = true;

    // Items required and no forgotten scrolls: the auto learn gives the plain skill only; the
    // tail of the plan (`getAvailableSkills`, items allowed) adds level 1 of the book.
    let p = plan_with(&env, &pack, &config, 0, false).unwrap();
    assert_eq!(skills_of(&p), [(10, 1), (11, 1)]);

    config.player.auto_learn_skills_without_items = true;
    let p = plan_with(&env, &pack, &config, 0, false).unwrap();
    assert_eq!(skills_of(&p), [(10, 1), (11, 2)]);

    config.player.auto_learn_fs_skills = true;
    let p = plan_with(&env, &pack, &config, 0, false).unwrap();
    assert_eq!(skills_of(&p), [(10, 1), (11, 2), (12, 1)]);

    // A forgotten scroll skill needs no `WithoutItems`.
    config.player.auto_learn_skills_without_items = false;
    let p = plan_with(&env, &pack, &config, 0, false).unwrap();
    assert_eq!(skills_of(&p), [(10, 1), (11, 1), (12, 1)]);
}

#[test]
fn a_starting_level_above_the_maximum_changes_nothing() {
    let env = env();
    let mut config = GameConfig::default();
    config.player.starting_level = 6;
    let p = plan_with(&env, &env.pack, &config, 0, false).unwrap();
    assert_eq!(p.update.level, 1);
    assert_eq!(p.update.exp, 0);
}

#[test]
fn starting_adena_is_cut_to_max_adena() {
    let env = env();
    let mut config = GameConfig::default();
    config.player.starting_adena = 1_000;
    config.player.max_adena = 400;
    let p = plan_with(&env, &env.pack, &config, 0, false).unwrap();
    assert_eq!(p.items[0].count, 400);
}

#[test]
fn running_out_of_ids_is_an_error() {
    let env = env();
    let config = GameConfig::default();
    let ctx = PlanContext {
        datapack: &env.pack,
        config: &config,
        stat_bonus: &env.bonus,
        item_max_mp: &env.max_mp,
    };
    let left = AtomicI32::new(2);
    let allocate = || (left.fetch_sub(1, Ordering::Relaxed) > 0).then_some(7);
    let mut ids = IdSource::new(&allocate);
    let request = CreateRequest {
        account: "a",
        name: "n",
        is_female: false,
        hair_style: 0,
        hair_color: 0,
        face: 0,
    };
    let template = env.pack.player_templates.get_by_id(0).unwrap();
    let result = build_plan(
        &ctx,
        &request,
        template,
        CHAR_ID,
        &mut ids,
        &mut WorldRng::from_seed(1),
        0,
    );
    assert_eq!(result.unwrap_err(), PlanError::OutOfIds);
    assert_eq!(
        ids.taken.len(),
        2,
        "the ids handed out are known, so they can be given back"
    );
}

#[test]
fn a_level_without_stat_values_gives_one_hp_like_java() {
    let env = env();
    // The mystic template of the test datapack has rows for levels 1 and 2 only; the rest of the
    // table is zero, and `calcStat` turns a result of zero into 1.
    let mut config = GameConfig::default();
    config.player.starting_level = 3;
    let p = plan_with(&env, &env.pack, &config, 10, false).unwrap();
    assert_eq!((p.update.max_hp, p.update.max_cp), (1, 1));
}

#[test]
fn equipping_follows_the_java_slot_rules() {
    let env = env();
    let mut pack = (*env.pack).clone();
    let armor = |id: i32, set: &str| {
        format!(
            "<item id=\"{id}\" type=\"Armor\" name=\"x{id}\"><set name=\"armor_type\" val=\"LIGHT\" />{set}</item>"
        )
    };
    let weapon = |id: i32, set: &str| {
        format!(
            "<item id=\"{id}\" type=\"Weapon\" name=\"w{id}\"><set name=\"weapon_type\" val=\"SWORD\" />{set}</item>"
        )
    };
    let xml = format!(
        "<list>{}{}{}{}{}</list>",
        armor(100, "<set name=\"bodypart\" val=\"fullarmor\" />"),
        armor(101, "<set name=\"bodypart\" val=\"legs\" />"),
        weapon(102, "<set name=\"bodypart\" val=\"lrhand\" />"),
        armor(103, "<set name=\"bodypart\" val=\"lhand\" />"),
        armor(104, "<set name=\"bodypart\" val=\"rfinger;lfinger\" />"),
    );
    pack.items = Arc::new(ItemTemplates::from_xml(&xml).unwrap());
    pack.initial_equipment = Arc::new(
        InitialEquipments::from_xml(
            r#"<list><equipment classId="0">
                <item id="100" count="1" equipped="true" />
                <item id="101" count="1" equipped="true" />
                <item id="102" count="1" equipped="true" />
                <item id="103" count="1" equipped="true" />
                <item id="104" count="1" equipped="true" />
                <item id="104" count="1" equipped="true" />
            </equipment></list>"#,
        )
        .unwrap(),
    );
    let p = plan_with(&env, &pack, &GameConfig::default(), 0, false).unwrap();
    let rows: Vec<_> = p.items.iter().map(|i| (i.item_id, i.loc, i.loc_data)).collect();
    assert_eq!(
        rows,
        [
            // The legs took the chest slot's full armor off.
            (100, ItemLocation::Inventory, 0),
            (101, ItemLocation::Paperdoll, 11),
            // The two-handed weapon was taken off by the shield.
            (102, ItemLocation::Inventory, 0),
            (103, ItemLocation::Paperdoll, 7),
            // Both rings: the left finger first, the second one fills the right.
            (104, ItemLocation::Paperdoll, 14),
            (104, ItemLocation::Paperdoll, 13),
        ]
    );
}

#[test]
fn stat_bonus_file_and_item_stats_are_read() {
    let bonus = StatBonus::from_xml(
        r#"<list><CON><stat value="1" bonus="0.5"/><stat value="x" bonus="9"/></CON><MEN><stat value="2" bonus="1.5"/></MEN><STR><stat value="1" bonus="7"/></STR></list>"#,
    );
    assert_eq!(bonus.con_bonus(1), 0.5);
    assert_eq!(bonus.men_bonus(2), 1.5);
    assert_eq!(bonus.con_bonus(2), 0.0);
    assert_eq!(bonus.con_bonus(500), 0.0);
    let mut stats = ItemMaxMp::default();
    let wanted: HashSet<i32> = [425].into_iter().collect();
    stats.scan(
        r#"<list><item id="424"><stats><stat type="maxMp">99</stat></stats></item>
            <item id="425"><stats><stat type="pDef">17</stat><stat type="maxMp">19</stat><stat type="maxMp">1.5</stat></stats></item></list>"#,
        &wanted,
    );
    assert_eq!(stats.max_mp_of(425), 20.5);
    assert_eq!(stats.max_mp_of(424), 0.0);
}

#[test]
fn macro_commands_are_written_like_java() {
    let command = |kind, d1, d2, text: &str| MacroCommand {
        entry: 0,
        kind,
        d1,
        d2,
        text: text.to_owned(),
    };
    let commands = vec![
        command(MacroType::Skill, 1177, 1, ""),
        command(MacroType::Text, 0, 0, "/loc"),
    ];
    assert_eq!(macro_commands(&commands), "1,1177,1;3,0,0,/loc;");
    let long: Vec<_> = (0..100).map(|_| command(MacroType::Delay, 5, 0, "")).collect();
    assert_eq!(macro_commands(&long).len(), 255);
}
