//! Avant-Garde: the sub-class certification skills (learning and cancelling them) and the
//! transformation skills and scrolls.
// Java: dist/game/data/scripts/ai/others/AvantGarde/AvantGarde.java
//
// # Left out, with the system that brings it
// - `addAcquireSkillId` / `onAcquireSkill` (`ON_PLAYER_SKILL_LEARN` of the trainer): the engine
//   has no such listener yet, so the list is not shown again after a skill is learned (the
//   player opens it again from the page).

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};
use tracing::warn;

// NPC
const AVANT_GARDE: i32 = 32323;

// Items
const ITEMS: [i32; 16] = [
    10280, 10281, 10282, 10283, 10284, 10285, 10286, 10287, 10288, 10289, 10290, 10291, 10292, 10293, 10294,
    10612,
];

// Misc
const QUEST_VAR_NAMES: [&str; 4] = [
    "EmergentAbility65-",
    "EmergentAbility70-",
    "ClassAbility75-",
    "ClassAbility80-",
];
/// Java `SubClassSkills.class.getSimpleName()` (`custom/Validators/SubClassSkills`).
const SUB_CLASS_SKILLS: &str = "SubClassSkills";

/// Java `ai.others.AvantGarde.AvantGarde`.
pub struct AvantGarde;

/// Java `RequestAcquireSkill.canTransform(player)`.
fn can_transform(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.world().can_transform(player)
}

/// Java `StringUtil.isNumeric(text)`.
fn is_numeric(text: &str) -> bool {
    !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit())
}

impl AvantGarde {
    /// Java `CancelCertification`, after the checks: the certification skills and books go, the
    /// variables become "0", the fee is paid.
    fn cancel_certifications(ctx: &mut ScriptCtx<'_>, player: ObjectId) -> Option<String> {
        let htmltext;
        // `getQuestState(SubClassSkills)`, else `newQuestState(player)`.
        // NOTE(java-quirk): without the script Java throws (`getScript(...)` is null).
        ctx.quest_state_of(SUB_CLASS_SKILLS, player, true)?;
        let max_subclass = ctx.world().max_subclass();
        let var = |ctx: &ScriptCtx<'_>, name: &str| {
            ctx.world()
                .player_variable_text(player, name)
                .unwrap_or("0")
                .to_owned()
        };
        let mut active_certifications = 0;
        for var_name in QUEST_VAR_NAMES {
            for i in 1..=max_subclass {
                let qvar = var(ctx, &format!("{var_name}{i}"));
                if !qvar.is_empty() && (qvar.ends_with(';') || qvar != "0") {
                    active_certifications += 1;
                }
            }
        }
        if active_certifications == 0 {
            htmltext = Some("32323-10no.html".to_owned());
        } else {
            for var_name in QUEST_VAR_NAMES {
                for i in 1..=max_subclass {
                    let qvar_name = format!("{var_name}{i}");
                    let qvar = var(ctx, &qvar_name);
                    if qvar.ends_with(';') {
                        let skill_id_var = qvar.replace(';', "");
                        if is_numeric(&skill_id_var) {
                            if let Ok(skill_id) = skill_id_var.parse::<i32>() {
                                // `SkillData.getSkill(skillId, 1) != null`: `removeSkill(sk)`.
                                if ctx.world().skill_template_exists(skill_id, 1) {
                                    ctx.world_mut().player_remove_skill(player, skill_id);
                                    ctx.world_mut().set_player_variable_text(
                                        player,
                                        &qvar_name,
                                        "0".to_owned(),
                                    );
                                }
                            }
                        } else {
                            warn!("Invalid Sub-Class Skill Id: {skill_id_var} for {player}!");
                        }
                    } else if !qvar.is_empty() && qvar != "0" {
                        if is_numeric(&qvar) {
                            let item_obj_id = ObjectId(qvar.parse::<i32>().unwrap_or(0));
                            let in_inventory = ctx.world().player(player).is_some_and(|p| {
                                p.data.items.iter().any(|it| {
                                    it.object_id == item_obj_id
                                        && (it.loc == "INVENTORY" || it.loc == "PAPERDOLL")
                                })
                            });
                            let in_warehouse = ctx.private_warehouse_item(player, item_obj_id);
                            if in_inventory {
                                ctx.world_mut().destroy_item_object(player, item_obj_id, 1, false);
                            } else if in_warehouse.is_some() {
                                let name = ctx.player_name(player);
                                warn!("Somehow {name} put a certification book into warehouse!");
                                // `getWarehouse().destroyItem(DESTROY, item, 1, player, false)`.
                                ctx.destroy_private_warehouse_item(player, item_obj_id, 1);
                            } else {
                                let name = ctx.player_name(player);
                                warn!("Somehow {name} deleted a certification book!");
                            }
                            ctx.world_mut()
                                .set_player_variable_text(player, &qvar_name, "0".to_owned());
                        } else {
                            warn!("Invalid item object Id: {qvar} for {player}!");
                        }
                    }
                }
            }
            let fee = i64::from(ctx.world().avant_garde_settings().0);
            ctx.world_mut().reduce_adena(player, fee, true);
            htmltext = Some("32323-09no.html".to_owned());
            ctx.world_mut().send_skill_list(player);
        }

        // Let's consume all certification books, even those not present in database.
        for item_id in ITEMS {
            let item = ctx.world().player(player).and_then(|p| {
                p.data
                    .items
                    .iter()
                    .find(|it| it.item_id == item_id && (it.loc == "INVENTORY" || it.loc == "PAPERDOLL"))
                    .map(|it| (it.object_id, it.count))
            });
            if let Some((object_id, count)) = item {
                warn!(
                    "AvantGarde: {player} had 'extra' certification skill books while cancelling sub-class certifications!"
                );
                ctx.world_mut()
                    .destroy_item_object(player, object_id, count, false);
            }
        }
        htmltext
    }
}

impl Script for AvantGarde {
    fn name(&self) -> &'static str {
        "AvantGarde"
    }

    fn path(&self) -> &'static str {
        "ai/others/AvantGarde"
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[AVANT_GARDE]);
        s.add_talk_id(&[AVANT_GARDE]);
        s.add_first_talk_id(&[AVANT_GARDE]);
        // `addAcquireSkillId(AVANT_GARDE)`: see the module notes.
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = None;
        match event {
            "32323-02.html" | "32323-02a.html" | "32323-02b.html" | "32323-02c.html" | "32323-05.html"
            | "32323-05a.html" | "32323-05no.html" | "32323-06.html" | "32323-06no.html" => {
                htmltext = Some(event.to_owned());
            }
            "LearnTransformationSkill" => {
                if can_transform(ctx, player) {
                    ctx.world_mut().show_transform_skill_list(player);
                } else {
                    htmltext = Some("32323-03.html".to_owned());
                }
            }
            "BuyTransformationItems" => {
                if can_transform(ctx, player) {
                    ctx.world_mut().multisell_separate_and_send(
                        32_323_001,
                        player,
                        npc.map(|n| n.object_id),
                        false,
                        1.0,
                        1.0,
                    );
                } else {
                    htmltext = Some("32323-04.html".to_owned());
                }
            }
            "LearnSubClassSkill" => {
                if !can_transform(ctx, player) {
                    htmltext = Some("32323-04.html".to_owned());
                }
                // NOTE(java-quirk): the page above is overwritten by the next checks.
                if ctx.world().is_subclass_active(player) {
                    htmltext = Some("32323-08.html".to_owned());
                } else {
                    let has_items = ITEMS.iter().any(|&id| ctx.has_quest_items(player, id));
                    if has_items {
                        ctx.world_mut().show_sub_class_skill_list(player);
                    } else {
                        htmltext = Some("32323-08.html".to_owned());
                    }
                }
            }
            "CancelCertification" => {
                if ctx.world().sub_classes(player).is_empty() {
                    htmltext = Some("32323-07.html".to_owned());
                } else if ctx.world().is_subclass_active(player) {
                    htmltext = Some("32323-08.html".to_owned());
                } else if ctx.world().adena_of(player) < i64::from(ctx.world().avant_garde_settings().0) {
                    htmltext = Some("32323-08no.html".to_owned());
                } else {
                    htmltext = Self::cancel_certifications(ctx, player);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_first_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("32323-01.html".to_owned())
    }

    fn on_talk(&self, _ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, _player: ObjectId) -> Option<String> {
        Some("32323-01.html".to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{ScriptRegistry, SubClassHolder};

    use super::*;
    use crate::testing::{ScriptTest, item, names_to, pages_to, player_data};

    const DIR: &str = "data/scripts/ai/others/AvantGarde";
    const NPCS: &str = r#"<npc id="32323" type="Folk" level="70" name="Avant-Garde"/>"#;
    const ITEMS_XML: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="10280" type="EtcItem" name="Certificate - Emergent Ability"/>"#;

    /// `SubClassSkills` as far as Avant-Garde needs it: a script of that name.
    struct SubClassSkills;

    impl Script for SubClassSkills {
        fn name(&self) -> &'static str {
            SUB_CLASS_SKILLS
        }
        fn path(&self) -> &'static str {
            "custom/Validators"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(AvantGarde);
        registry.add(SubClassSkills);
        let link = |e: &str| format!(r#"<a action="bypass -h Script AvantGarde {e}">{e}</a>"#);
        let mut pages: Vec<(String, String)> = vec![(
            format!("{DIR}/32323-01.html"),
            format!(
                "<html><body>32323-01 {}{}{}</body></html>",
                link("LearnSubClassSkill"),
                link("CancelCertification"),
                link("32323-02.html")
            ),
        )];
        for page in [
            "32323-02.html",
            "32323-07.html",
            "32323-08.html",
            "32323-08no.html",
            "32323-09no.html",
            "32323-10no.html",
        ] {
            pages.push((
                format!("{DIR}/{page}"),
                format!("<html><body>{page}</body></html>"),
            ));
        }
        let pages: Vec<(&str, &str)> = pages.iter().map(|(a, b)| (a.as_str(), b.as_str())).collect();
        let mut t = ScriptTest::new(registry, NPCS, ITEMS_XML, &pages);
        let npc = t.spawn(AVANT_GARDE, 20);
        (t, npc)
    }

    fn enter(
        t: &mut ScriptTest,
        id: i32,
        items: Vec<l2_world::ItemData>,
        variables: &[(&str, &str)],
        with_sub: bool,
    ) {
        let mut data = player_data(id, 80, items);
        data.class_id = 2;
        data.base_class = 2;
        if with_sub {
            data.profile.subclasses.subs.insert(
                1,
                SubClassHolder {
                    class_id: 12,
                    exp: 0,
                    sp: 0,
                    level: 75,
                    class_index: 1,
                },
            );
        }
        for (name, value) in variables {
            data.profile
                .variables
                .insert((*name).to_owned(), (*value).to_owned());
        }
        t.enter_with(data);
        t.take();
    }

    fn click(t: &mut ScriptTest, player: i32, npc: Oid, event: &str) -> crate::testing::Sent {
        t.talk(player, npc);
        t.take();
        t.bypass(player, &format!("Script AvantGarde {event}"));
        t.take()
    }

    #[test]
    fn the_first_page_and_the_sub_class_skill_list_without_books() {
        let (mut t, npc) = setup();
        enter(&mut t, 1, vec![], &[], true);
        t.talk(1, npc);
        assert!(pages_to(&t.take(), 1).pop().unwrap().contains("32323-01"));
        assert!(
            pages_to(&click(&mut t, 1, npc, "32323-02.html"), 1)
                .pop()
                .unwrap()
                .contains("32323-02.html")
        );
        // No certificate in the bag: 32323-08.
        assert!(
            pages_to(&click(&mut t, 1, npc, "LearnSubClassSkill"), 1)
                .pop()
                .unwrap()
                .contains("32323-08.html")
        );
        // A certificate, but no skill tree in this world: "There are no other skills to learn."
        enter(&mut t, 2, vec![item(200, 10280, 1)], &[], true);
        let out = click(&mut t, 2, npc, "LearnSubClassSkill");
        assert!(names_to(&out, 2).contains(&"SYSTEM_MESSAGE".to_owned()));
        assert!(pages_to(&out, 2).is_empty());
    }

    #[test]
    fn cancelling_the_certifications_takes_the_books_and_the_fee() {
        let (mut t, npc) = setup();
        // No subclass at all.
        enter(&mut t, 1, vec![], &[], false);
        assert!(
            pages_to(&click(&mut t, 1, npc, "CancelCertification"), 1)
                .pop()
                .unwrap()
                .contains("32323-07.html")
        );
        // Not enough adena for `FeeDeleteSubClassSkills` (10,000,000).
        enter(&mut t, 2, vec![item(201, 57, 9_999_999)], &[], true);
        assert!(
            pages_to(&click(&mut t, 2, npc, "CancelCertification"), 2)
                .pop()
                .unwrap()
                .contains("32323-08no.html")
        );
        // Nothing certified.
        enter(&mut t, 3, vec![item(202, 57, 10_000_000)], &[], true);
        assert!(
            pages_to(&click(&mut t, 3, npc, "CancelCertification"), 3)
                .pop()
                .unwrap()
                .contains("32323-10no.html")
        );
        assert_eq!(t.count(3, 57), 10_000_000);
        // A book received at 65, still in the bag.
        enter(
            &mut t,
            4,
            vec![item(203, 57, 12_000_000), item(204, 10280, 1)],
            &[("EmergentAbility65-1", "204")],
            true,
        );
        assert!(
            pages_to(&click(&mut t, 4, npc, "CancelCertification"), 4)
                .pop()
                .unwrap()
                .contains("32323-09no.html")
        );
        assert_eq!(t.count(4, 10280), 0);
        assert_eq!(t.count(4, 57), 2_000_000);
        assert_eq!(
            t.world.player_variable_text(Oid(4), "EmergentAbility65-1"),
            Some("0")
        );
    }
}
