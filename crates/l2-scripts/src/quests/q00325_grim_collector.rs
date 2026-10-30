//! Quest 325: Grim Collector (Guard Curtiz, Varsak and Samed in the Talking Island / Gludin area).
// Java: dist/game/data/scripts/quests/Q00325_GrimCollector/Q00325_GrimCollector.java
//
// Pattern: a collection quest with drop chances per monster (`docs/design/scripts.md`, "Quest
// patterns"). The anatomy diagram (from Samed) lets the player collect bones and zombie parts:
// one roll per kill, compared with the monster's drop list in order. Varsak assembles a skeleton
// from five bones (4 times in 5). Samed pays for what the player brings, with a bonus at ten
// items, and the skeleton pays extra.

use l2_util::ObjectId;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const GUARD_CURTIZ: i32 = 30336;
const VARSAK: i32 = 30342;
const SAMED: i32 = 30434;

// Items
const ANATOMY_DIAGRAM: i32 = 1349;
const ZOMBIE_HEAD: i32 = 1350;
const ZOMBIE_HEART: i32 = 1351;
const ZOMBIE_LIVER: i32 = 1352;
const SKULL: i32 = 1353;
const RIB_BONE: i32 = 1354;
const SPINE: i32 = 1355;
const ARM_BONE: i32 = 1356;
const THIGH_BONE: i32 = 1357;
const COMPLETE_SKELETON: i32 = 1358;

/// Every item of the quest (Java `getRegisteredItemIds()`).
const REGISTERED: [i32; 10] = [
    ANATOMY_DIAGRAM,
    ZOMBIE_HEAD,
    ZOMBIE_HEART,
    ZOMBIE_LIVER,
    SKULL,
    RIB_BONE,
    SPINE,
    ARM_BONE,
    THIGH_BONE,
    COMPLETE_SKELETON,
];

// Misc
const MIN_LEVEL: i32 = 15;

// Monsters: (id, [(item, the roll must be below this)]) in Java's order
const MONSTER_DROPS: [(i32, &[(i32, i32)]); 9] = [
    (
        20026,
        &[(ZOMBIE_HEAD, 30), (ZOMBIE_HEART, 50), (ZOMBIE_LIVER, 75)],
    ),
    (
        20029,
        &[(ZOMBIE_HEAD, 30), (ZOMBIE_HEART, 52), (ZOMBIE_LIVER, 75)],
    ),
    (
        20035,
        &[(SKULL, 5), (RIB_BONE, 15), (SPINE, 29), (THIGH_BONE, 79)],
    ),
    (
        20042,
        &[(SKULL, 6), (RIB_BONE, 19), (ARM_BONE, 69), (THIGH_BONE, 86)],
    ),
    (
        20045,
        &[(SKULL, 9), (SPINE, 59), (ARM_BONE, 77), (THIGH_BONE, 97)],
    ),
    (20051, &[(SKULL, 9), (RIB_BONE, 59), (SPINE, 79), (ARM_BONE, 100)]),
    (
        20457,
        &[(ZOMBIE_HEAD, 40), (ZOMBIE_HEART, 60), (ZOMBIE_LIVER, 80)],
    ),
    (
        20458,
        &[(ZOMBIE_HEAD, 40), (ZOMBIE_HEART, 70), (ZOMBIE_LIVER, 100)],
    ),
    (
        20514,
        &[
            (SKULL, 6),
            (RIB_BONE, 21),
            (SPINE, 30),
            (ARM_BONE, 31),
            (THIGH_BONE, 64),
        ],
    ),
];

/// Java `Q00325_GrimCollector`.
pub struct Q00325GrimCollector;

impl Script for Q00325GrimCollector {
    fn name(&self) -> &'static str {
        "Q00325_GrimCollector"
    }

    fn path(&self) -> &'static str {
        "quests/Q00325_GrimCollector"
    }

    fn id(&self) -> i32 {
        325
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[GUARD_CURTIZ]);
        s.add_talk_id(&[GUARD_CURTIZ, VARSAK, SAMED]);
        let monsters: Vec<i32> = MONSTER_DROPS.iter().map(|(id, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&REGISTERED);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "30336-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "assembleSkeleton" => {
                let parts = [SPINE, ARM_BONE, SKULL, RIB_BONE, THIGH_BONE];
                if !ctx.has_all_quest_items(player, &parts) {
                    htmltext = Some("30342-02.html".to_owned());
                } else {
                    ctx.take_items_many(player, 1, &parts);
                    if ctx.get_random(5) < 4 {
                        ctx.give_items(player, COMPLETE_SKELETON, 1);
                        htmltext = Some("30342-03.html".to_owned());
                    } else {
                        htmltext = Some("30342-04.html".to_owned());
                    }
                }
            }
            "30434-02.htm" => {
                htmltext = Some(event.to_owned());
            }
            "30434-03.html" => {
                ctx.give_items(player, ANATOMY_DIAGRAM, 1);
                htmltext = Some(event.to_owned());
            }
            "30434-06.html" | "30434-07.html" => {
                let head = ctx.get_quest_items_count(player, ZOMBIE_HEAD);
                let heart = ctx.get_quest_items_count(player, ZOMBIE_HEART);
                let liver = ctx.get_quest_items_count(player, ZOMBIE_LIVER);
                let skull = ctx.get_quest_items_count(player, SKULL);
                let rib = ctx.get_quest_items_count(player, RIB_BONE);
                let spine = ctx.get_quest_items_count(player, SPINE);
                let arm = ctx.get_quest_items_count(player, ARM_BONE);
                let thigh = ctx.get_quest_items_count(player, THIGH_BONE);
                let complete = ctx.get_quest_items_count(player, COMPLETE_SKELETON);
                let total_count = head + heart + liver + skull + rib + spine + arm + thigh + complete;
                if total_count > 0 {
                    let mut sum = head * 30
                        + heart * 20
                        + liver * 20
                        + skull * 100
                        + rib * 40
                        + spine * 14
                        + arm * 14
                        + thigh * 14;
                    if total_count >= 10 {
                        sum += 1629;
                    }
                    if complete > 0 {
                        sum += 543 + complete * 341;
                    }
                    ctx.give_adena(player, sum, true);
                }

                // Every item but the diagram (Java's list of nine ids).
                ctx.take_items_many(player, -1, &REGISTERED[1..]);

                if event == "30434-06.html" {
                    qs.exit_quest(ctx, true, true);
                }
                htmltext = Some(event.to_owned());
            }
            "30434-09.html" => {
                let complete = ctx.get_quest_items_count(player, COMPLETE_SKELETON);
                if complete > 0 {
                    ctx.give_adena(player, complete * 341 + 543, true);
                    ctx.take_items(player, COMPLETE_SKELETON, -1);
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !qs.is_started(ctx)
            || !ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
            || !ctx.has_quest_items(killer, ANATOMY_DIAGRAM)
        {
            return;
        }

        let rnd = ctx.get_random(100);
        if let Some((_, drops)) = MONSTER_DROPS.iter().find(|(id, _)| *id == npc.id).copied() {
            for &(item, chance) in drops {
                if rnd < chance {
                    ctx.give_item_randomly(killer, Some(npc), item, 1, 1, 0, 1.0, true);
                    break;
                }
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match npc.id {
            GUARD_CURTIZ => {
                if qs.is_created(ctx) {
                    htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                        "30336-02.htm"
                    } else {
                        "30336-01.htm"
                    }
                    .to_owned();
                } else if qs.is_started(ctx) {
                    htmltext = if ctx.has_quest_items(player, ANATOMY_DIAGRAM) {
                        "30336-05.html"
                    } else {
                        "30336-04.html"
                    }
                    .to_owned();
                }
            }
            VARSAK => {
                if qs.is_started(ctx) && ctx.has_quest_items(player, ANATOMY_DIAGRAM) {
                    htmltext = "30342-01.html".to_owned();
                }
            }
            SAMED => {
                if qs.is_started(ctx) {
                    htmltext = if !ctx.has_quest_items(player, ANATOMY_DIAGRAM) {
                        "30434-01.html"
                    } else if !ctx.has_at_least_one_quest_item(player, &REGISTERED) {
                        "30434-04.html"
                    } else if !ctx.has_quest_items(player, COMPLETE_SKELETON) {
                        "30434-05.html"
                    } else {
                        "30434-08.html"
                    }
                    .to_owned();
                }
            }
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00325_GrimCollector";

    const NPCS: &str = r#"
        <npc id="30336" type="Folk" level="1" name="Guard Curtiz"/>
        <npc id="30342" type="Folk" level="1" name="Varsak"/>
        <npc id="30434" type="Folk" level="1" name="Samed"/>
        <npc id="20035" type="Monster" level="15" name="Skeleton"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1349" type="EtcItem" name="Anatomy Diagram"><set name="is_questitem" val="true"/></item>
        <item id="1350" type="EtcItem" name="Zombie Head"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1351" type="EtcItem" name="Zombie Heart"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1352" type="EtcItem" name="Zombie Liver"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1353" type="EtcItem" name="Skull"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1354" type="EtcItem" name="Rib Bone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1355" type="EtcItem" name="Spine"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1356" type="EtcItem" name="Arm Bone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1357" type="EtcItem" name="Thigh Bone"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1358" type="EtcItem" name="Complete Skeleton"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00325GrimCollector);
        let pages = Pages::of(&Q00325GrimCollector)
            .page("30336-02.htm", &["30336-03.htm"])
            .plain(&["30336-01.htm", "30336-03.htm", "30336-04.html", "30336-05.html"])
            .page("30342-01.html", &["assembleSkeleton"])
            .plain(&["30342-02.html", "30342-03.html", "30342-04.html"])
            .page("30434-01.html", &["30434-02.htm"])
            .page("30434-02.htm", &["30434-03.html"])
            .plain(&["30434-03.html", "30434-06.html", "30434-07.html"])
            .page("30434-04.html", &["30434-06.html", "30434-07.html"])
            .page("30434-05.html", &["30434-07.html", "30434-06.html"])
            .page(
                "30434-08.html",
                &["30434-09.html", "30434-07.html", "30434-06.html"],
            )
            .page("30434-09.html", &["30434-07.html", "30434-06.html"])
            .npcs(&[GUARD_CURTIZ, VARSAK, SAMED]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let curtiz = t.spawn(GUARD_CURTIZ, 20);
        let varsak = t.spawn(VARSAK, 40);
        let samed = t.spawn(SAMED, 60);
        (t, curtiz, varsak, samed)
    }

    /// Kills Skeletons until one of the bones drops; returns the number of kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            let skeleton = t.spawn(20035, 60);
            t.kill(1, skeleton);
            let bones = [SKULL, RIB_BONE, SPINE, THIGH_BONE]
                .iter()
                .map(|id| t.count(1, *id))
                .sum::<i64>();
            if bones >= 1 {
                return kills;
            }
        }
        panic!("no bone after 400 kills");
    }

    #[test]
    fn a_level_fourteen_character_is_sent_away() {
        let (mut t, curtiz, _, _) = setup();
        t.enter_as(1, 14, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, curtiz).contains("30336-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_diagram_the_bones_and_the_skeleton() {
        let (mut t, curtiz, varsak, samed) = setup();
        t.enter_as(1, 15, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, curtiz).contains("30336-02.htm"));
        assert!(t.event(1, NAME, "30336-03.htm").contains("30336-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Without the diagram Curtiz and Samed send the player away; Varsak says nothing.
        assert!(t.quest_page(1, curtiz).contains("30336-04.html"));
        assert!(t.quest_page(1, samed).contains("30434-01.html"));
        assert!(t.event(1, NAME, "30434-02.htm").contains("30434-02.htm"));
        assert!(t.event(1, NAME, "30434-03.html").contains("30434-03.html"));
        assert_eq!(t.count(1, ANATOMY_DIAGRAM), 1);
        assert!(t.take_writes().is_empty());

        // With the diagram: Curtiz and Varsak have their lines.
        assert!(t.quest_page(1, curtiz).contains("30336-05.html"));
        assert!(t.quest_page(1, varsak).contains("30342-01.html"));

        // Skeletons drop the bones; Samed has no complete skeleton yet.
        hunt(&mut t);
        assert!(t.quest_page(1, samed).contains("30434-05.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn five_bones_make_a_skeleton_or_not() {
        let (mut t, curtiz, varsak, _) = setup();
        t.enter_as(
            1,
            15,
            race::HUMAN,
            player_class::FIGHTER,
            vec![
                item(900, 1349, 1),
                item(901, 1355, 1),
                item(902, 1356, 1),
                item(903, 1353, 1),
                item(904, 1354, 1),
                item(905, 1357, 1),
            ],
        );
        t.quest_page(1, curtiz);
        assert!(t.event(1, NAME, "30336-03.htm").contains("30336-03.htm"));
        t.take_writes();

        // Five bones: one roll (4 in 5 make the skeleton); the bones are taken either way.
        assert!(t.quest_page(1, varsak).contains("30342-01.html"));
        let page = t.event(1, NAME, "assembleSkeleton");
        assert!(
            page.contains("30342-03.html") || page.contains("30342-04.html"),
            "{page}"
        );
        assert_eq!(
            t.count(1, COMPLETE_SKELETON),
            i64::from(page.contains("30342-03.html"))
        );
        assert_eq!(t.count(1, SPINE) + t.count(1, SKULL), 0);
        assert!(t.take_writes().is_empty());

        // No bones left: the NPC says so.
        assert!(t.quest_page(1, varsak).contains("30342-01.html"));
        assert!(t.event(1, NAME, "assembleSkeleton").contains("30342-02.html"));
    }

    #[test]
    fn the_reward_for_the_bones_then_the_end() {
        let (mut t, curtiz, _, samed) = setup();
        t.enter_as(
            1,
            15,
            race::HUMAN,
            player_class::FIGHTER,
            vec![
                item(900, 1349, 1),
                item(901, 1350, 2),
                item(902, 1351, 1),
                item(903, 1352, 1),
                item(904, 1353, 1),
                item(905, 1354, 1),
                item(906, 1355, 1),
                item(907, 1356, 1),
                item(908, 1357, 1),
                item(909, 1358, 1),
            ],
        );
        t.quest_page(1, curtiz);
        assert!(t.event(1, NAME, "30336-03.htm").contains("30336-03.htm"));
        t.take_writes();

        // Samed offers the hand-in; nothing is paid before the player chooses.
        assert!(t.quest_page(1, samed).contains("30434-08.html"));
        assert_eq!(t.count(1, 57), 0);
        assert!(t.take_writes().is_empty());

        // The hand-in (`30434-06.html`): ten items, so the bonus of 1629 and the complete skeleton
        // (543 + 341) on top of the parts (282). The quest ends repeatable, with the finish sound.
        t.take();
        t.bypass(1, &format!("Script {NAME} 30434-06.html"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30434-06.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.count(1, 57), 282 + 1629 + 543 + 341);
        assert_eq!(t.count(1, COMPLETE_SKELETON), 0);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn a_skeleton_is_paid_on_its_own_and_samed_takes_nothing_else() {
        let (mut t, curtiz, _, samed) = setup();
        t.enter_as(
            1,
            15,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 1349, 1), item(901, 1358, 2)],
        );
        t.quest_page(1, curtiz);
        assert!(t.event(1, NAME, "30336-03.htm").contains("30336-03.htm"));
        t.take_writes();

        // 09: the two skeletons (341 each) and 543, with no page and no end.
        assert!(t.quest_page(1, samed).contains("30434-08.html"));
        assert!(t.event(1, NAME, "30434-09.html").is_empty());
        assert_eq!(t.count(1, 57), 2 * 341 + 543);
        assert_eq!(t.count(1, COMPLETE_SKELETON), 0);
        assert_eq!(t.count(1, ANATOMY_DIAGRAM), 1);
        assert!(t.take_writes().is_empty());
    }
}
