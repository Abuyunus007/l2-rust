//! The certification skill check at login (`SkillCheckEnable`): certification skills on a
//! subclass, skills and books the certification variables do not account for. Also the quest
//! state `RequestAcquireSkill` asks for when a certification skill is learned.
// Java: dist/game/data/scripts/custom/Validators/SubClassSkills.java

use l2_util::ObjectId;
use l2_world::{IllegalActionPunishment, Script, ScriptCtx, ScriptSetup};

/// `_allCertSkillIds` (sorted).
const ALL_CERT_SKILL_IDS: [i32; 38] = [
    631, 632, 633, 634, 637, 638, 639, 640, 641, 642, 643, 644, 645, 646, 647, 648, 650, 651, 652, 653, 654,
    655, 656, 657, 658, 659, 660, 661, 662, 799, 800, 801, 802, 803, 804, 1489, 1490, 1491,
];
/// `_certSkillsByLevel`, by [`VARS`] index.
const CERT_SKILLS_BY_LEVEL: [&[i32]; 4] = [
    &[631, 632, 633, 634],
    &[631, 632, 633, 634],
    &[
        637, 638, 639, 640, 641, 642, 643, 644, 645, 646, 647, 648, 650, 651, 652, 653, 654, 655, 799, 800,
        801, 802, 803, 804, 1489, 1490, 1491,
    ],
    &[656, 657, 658, 659, 660, 661, 662],
];
/// `_allCertItemIds` (sorted).
const ALL_CERT_ITEM_IDS: [i32; 16] = [
    10280, 10281, 10282, 10283, 10284, 10285, 10286, 10287, 10288, 10289, 10290, 10291, 10292, 10293, 10294,
    10612,
];
/// `_certItemsByLevel`, by [`VARS`] index.
const CERT_ITEMS_BY_LEVEL: [&[i32]; 4] = [
    &[10280],
    &[10280],
    &[10612, 10281, 10282, 10283, 10284, 10285, 10286, 10287],
    &[10288, 10289, 10290, 10291, 10292, 10293, 10294],
];
const VARS: [&str; 4] = [
    "EmergentAbility65-",
    "EmergentAbility70-",
    "ClassAbility75-",
    "ClassAbility80-",
];

/// Java `custom.Validators.SubClassSkills`.
pub struct SubClassSkills;

/// `PunishmentManager.handleIllegalPlayerAction(player, message, NONE)`.
fn illegal(ctx: &mut ScriptCtx<'_>, player: ObjectId, message: &str) {
    ctx.world_mut()
        .handle_illegal_player_action(player, message, IllegalActionPunishment::None);
}

impl Script for SubClassSkills {
    fn name(&self) -> &'static str {
        "SubClassSkills"
    }

    fn path(&self) -> &'static str {
        "custom/Validators"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.set_on_enter_world();
    }

    fn on_enter_world(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId) {
        let (enable, remove, check_gm) = ctx.world().skill_check_settings();
        if !enable {
            return;
        }
        if ctx.is_gm(player) && !check_gm {
            return;
        }
        let Some(p) = ctx.world().player(player) else {
            return;
        };
        // `getCertSkills(player)`: `(id, level)` of the known certification skills.
        let cert_skills: Vec<(i32, i32)> = p
            .core
            .skills
            .iter()
            .copied()
            .filter(|(id, _)| ALL_CERT_SKILL_IDS.binary_search(id).is_ok())
            .collect();
        // `getCertItems(player)`: `(object id, item id, count)` of the certification books.
        let cert_items: Vec<(i32, i32, i64)> = p
            .data
            .items
            .iter()
            .filter(|i| {
                (i.loc == "INVENTORY" || i.loc == "PAPERDOLL")
                    && ALL_CERT_ITEM_IDS.binary_search(&i.item_id).is_ok()
            })
            .map(|i| (i.object_id.0, i.item_id, i.count))
            .collect();
        let player_text = ctx.world().player_text(player);
        let skill_name = |ctx: &ScriptCtx<'_>, (id, level): (i32, i32)| ctx.world().skill_name(id, level);

        if ctx.world().is_subclass_active(player) {
            let class_id = p.data.class_id;
            let class_name = ctx.world().class_name(class_id);
            for skill in cert_skills {
                let name = skill_name(ctx, skill);
                illegal(
                    ctx,
                    player,
                    &format!(
                        "{player_text} has cert skill on subclass :{name}({}/{}), class:{class_name}",
                        skill.0, skill.1
                    ),
                );
                if remove {
                    ctx.world_mut().player_remove_skill(player, skill.0);
                }
            }
            return;
        }

        // skillId/skillLevel
        let mut c_skills: Vec<(i32, i32)> = cert_skills.clone();
        // objectId/number
        let mut c_items: Vec<(i32, i64)> = cert_items
            .iter()
            .map(|&(object_id, _, count)| (object_id, count.min(i64::from(i32::MAX))))
            .collect();

        // `getQuestState("SubClassSkills")`, else `newQuestState(player)` (no row is written).
        ctx.get_quest_state(player, true);

        let max_subclass = ctx.world().max_subclass();
        for i in (0..VARS.len()).rev() {
            for j in (1..=max_subclass).rev() {
                let q_name = format!("{}{j}", VARS[i]);
                let q_value = ctx
                    .world()
                    .player_variable_text(player, &q_name)
                    .unwrap_or("")
                    .to_owned();
                if q_value.is_empty() {
                    continue;
                }
                if q_value.ends_with(';') {
                    // found skill
                    let Ok(id) = q_value.replace(';', "").parse::<i32>() else {
                        illegal(
                            ctx,
                            player,
                            &format!("Invalid cert variable:{q_name}={q_value} - not a number"),
                        );
                        continue;
                    };
                    // searching skill in test array
                    let mut skill = None;
                    for index in (0..cert_skills.len()).rev() {
                        if c_skills[index].0 == id {
                            skill = Some(cert_skills[index]);
                            c_skills[index].1 -= 1;
                            break;
                        }
                    }
                    if skill.is_some() {
                        if !CERT_SKILLS_BY_LEVEL[i].contains(&id) {
                            // should remove this skill ?
                            illegal(
                                ctx,
                                player,
                                &format!(
                                    "Invalid cert variable WITH skill:{q_name}={q_value} - skill does not match certificate level"
                                ),
                            );
                        }
                    } else {
                        illegal(
                            ctx,
                            player,
                            &format!("Invalid cert variable:{q_name}={q_value} - skill not found"),
                        );
                    }
                } else {
                    // found item
                    let Ok(id) = q_value.parse::<i32>() else {
                        illegal(
                            ctx,
                            player,
                            &format!("Invalid cert variable:{q_name}={q_value} - not a number"),
                        );
                        continue;
                    };
                    if id == 0 {
                        continue;
                    }
                    // searching item in test array
                    let mut item = None;
                    for index in (0..cert_items.len()).rev() {
                        if c_items[index].0 == id {
                            item = Some(cert_items[index]);
                            c_items[index].1 -= 1;
                            break;
                        }
                    }
                    match item {
                        Some((_, item_id, _)) => {
                            if !CERT_ITEMS_BY_LEVEL[i].contains(&item_id) {
                                illegal(
                                    ctx,
                                    player,
                                    &format!(
                                        "Invalid cert variable:{q_name}={q_value} - item found but does not match certificate level"
                                    ),
                                );
                            }
                        }
                        None => {
                            illegal(
                                ctx,
                                player,
                                &format!("Invalid cert variable:{q_name}={q_value} - item not found"),
                            );
                        }
                    }
                }
            }
        }

        for i in (0..c_skills.len()).rev() {
            if c_skills[i].1 == 0 {
                continue;
            }
            let skill = cert_skills[i];
            let name = skill_name(ctx, skill);
            if c_skills[i].1 > 0 {
                if c_skills[i].1 == skill.1 {
                    illegal(
                        ctx,
                        player,
                        &format!(
                            "{player_text} has invalid cert skill :{name}({}/{})",
                            skill.0, skill.1
                        ),
                    );
                } else {
                    illegal(
                        ctx,
                        player,
                        &format!(
                            "{player_text} has invalid cert skill :{name}({}/{}), level too high",
                            skill.0, skill.1
                        ),
                    );
                }
                if remove {
                    ctx.world_mut().player_remove_skill(player, skill.0);
                }
            } else {
                illegal(
                    ctx,
                    player,
                    &format!(
                        "Invalid cert skill :{name}({}/{}), level too low",
                        skill.0, skill.1
                    ),
                );
            }
        }

        for i in (0..c_items.len()).rev() {
            if c_items[i].1 == 0 {
                continue;
            }
            let object_id = cert_items[i].0;
            illegal(
                ctx,
                player,
                &format!("Invalid cert item without variable or with wrong count:{object_id}"),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{ScriptRegistry, SkillData};

    use super::*;
    use crate::testing::{ScriptTest, item, player_data};

    #[test]
    fn the_check_is_off_by_default_and_writes_nothing() {
        let mut registry = ScriptRegistry::new();
        registry.add(SubClassSkills);
        let mut t = ScriptTest::new(registry, "", "", &[]);
        let mut data = player_data(1, 80, vec![item(100, 10280, 1)]);
        data.skills = vec![SkillData {
            skill_id: 631,
            skill_level: 1,
        }];
        t.enter_with(data);
        t.ticks(2);
        t.take();
        // `SkillCheckEnable` is off (Java's default): nothing is checked, no state is made.
        assert!(
            t.world
                .player_quests(Oid(1))
                .is_none_or(|q| !q.contains("SubClassSkills"))
        );
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_cert_tables_are_sorted_for_the_binary_search() {
        assert!(ALL_CERT_SKILL_IDS.windows(2).all(|w| w[0] < w[1]));
        assert!(ALL_CERT_ITEM_IDS.windows(2).all(|w| w[0] < w[1]));
    }
}
