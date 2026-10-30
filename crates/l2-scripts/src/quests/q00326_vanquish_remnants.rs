//! Quest 326: Vanquish Remnants (Leopold in the Rune town, the Ol Mahum mobs).
// Java: dist/game/data/scripts/quests/Q00326_VanquishRemnants/Q00326_VanquishRemnants.java
//
// Pattern: a kill-count quest with a drop chance per monster (`docs/design/scripts.md`, "Quest
// patterns"): each monster drops one of three badge kinds with its own chance and its own
// adena per badge; the NPC pays for all the badges at once, and the Black Lion Mark at 100.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const LEOPOLD: i32 = 30435;

// Items
const RED_CROSS_BADGE: i32 = 1359;
const BLUE_CROSS_BADGE: i32 = 1360;
const BLACK_CROSS_BADGE: i32 = 1361;
const BLACK_LION_MARK: i32 = 1369;

// Monsters (id, drop chance in percent, the badge)
const MONSTERS: [(i32, i32, i32); 9] = [
    (20053, 61, RED_CROSS_BADGE),   // Ol Mahum Patrol
    (20058, 61, RED_CROSS_BADGE),   // Ol Mahum Guard
    (20061, 57, BLUE_CROSS_BADGE),  // Ol Mahum Remnants
    (20063, 63, BLUE_CROSS_BADGE),  // Ol Mahum Shooter
    (20066, 59, BLACK_CROSS_BADGE), // Ol Mahum Captain
    (20436, 55, BLUE_CROSS_BADGE),  // Ol Mahum Supplier
    (20437, 59, RED_CROSS_BADGE),   // Ol Mahum Recruit
    (20438, 60, BLACK_CROSS_BADGE), // Ol Mahum General
    (20439, 62, BLUE_CROSS_BADGE),  // Ol Mahum Officer
];

// Misc
const MIN_LEVEL: i32 = 21;

/// Java `Q00326_VanquishRemnants`.
pub struct Q00326VanquishRemnants;

impl Script for Q00326VanquishRemnants {
    fn name(&self) -> &'static str {
        "Q00326_VanquishRemnants"
    }

    fn path(&self) -> &'static str {
        "quests/Q00326_VanquishRemnants"
    }

    fn id(&self) -> i32 {
        326
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LEOPOLD]);
        s.add_talk_id(&[LEOPOLD]);
        let monsters: Vec<i32> = MONSTERS.iter().map(|(id, _, _)| *id).collect();
        s.add_kill_id(&monsters);
        s.register_quest_items(&[RED_CROSS_BADGE, BLUE_CROSS_BADGE, BLACK_CROSS_BADGE]);
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
            "30435-03.htm" => {
                qs.start_quest(ctx);
                htmltext = Some(event.to_owned());
            }
            "30435-07.html" => {
                qs.exit_quest(ctx, true, true);
                htmltext = Some(event.to_owned());
            }
            "30435-08.html" => {
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_started(ctx)
        {
            // Only the registered monsters reach here.
            if let Some((_, chance, badge)) = MONSTERS.iter().find(|(id, _, _)| *id == npc.id).copied()
                && ctx.get_random(100) < chance
            {
                ctx.give_items(killer, badge, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        // Java starts with `htmltext = null`: a completed quest gets no page at all.
        let mut htmltext: Option<String> = None;
        if qs.is_created(ctx) {
            htmltext = Some(
                if ctx.player_level(player) >= MIN_LEVEL {
                    "30435-02.htm"
                } else {
                    "30435-01.htm"
                }
                .to_owned(),
            );
        } else if qs.is_started(ctx) {
            let red_badges = ctx.get_quest_items_count(player, RED_CROSS_BADGE);
            let blue_badges = ctx.get_quest_items_count(player, BLUE_CROSS_BADGE);
            let black_badges = ctx.get_quest_items_count(player, BLACK_CROSS_BADGE);
            let sum = red_badges + blue_badges + black_badges;
            if sum > 0 {
                if sum >= 100 && !ctx.has_quest_items(player, BLACK_LION_MARK) {
                    ctx.give_items(player, BLACK_LION_MARK, 1);
                }
                let bonus = if sum >= 10 { 4320 } else { 0 };
                ctx.give_adena(
                    player,
                    red_badges * 46 + blue_badges * 52 + black_badges * 58 + bonus,
                    true,
                );
                ctx.take_items_many(
                    player,
                    -1,
                    &[RED_CROSS_BADGE, BLUE_CROSS_BADGE, BLACK_CROSS_BADGE],
                );
                htmltext = Some(
                    if sum >= 100 {
                        if ctx.has_quest_items(player, BLACK_LION_MARK) {
                            "30435-09.html"
                        } else {
                            "30435-06.html"
                        }
                    } else {
                        "30435-05.html"
                    }
                    .to_owned(),
                );
            } else {
                htmltext = Some("30435-04.html".to_owned());
            }
        }
        htmltext
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, names_to, pages_to, rows};

    const NAME: &str = "Q00326_VanquishRemnants";

    const NPCS: &str = r#"
        <npc id="30435" type="Folk" level="1" name="Leopold"/>
        <npc id="20053" type="Monster" level="20" name="Ol Mahum Patrol"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1359" type="EtcItem" name="Red Cross Badge"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1360" type="EtcItem" name="Blue Cross Badge"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1361" type="EtcItem" name="Black Cross Badge"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1369" type="EtcItem" name="Black Lion Mark"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00326VanquishRemnants);
        let pages = Pages::of(&Q00326VanquishRemnants)
            .page("30435-02.htm", &["30435-03.htm"])
            .plain(&["30435-01.htm", "30435-03.htm", "30435-08.html"])
            .page("30435-04.html", &["30435-07.html", "30435-08.html"])
            .page("30435-05.html", &["30435-07.html", "30435-08.html"])
            .page("30435-06.html", &["30435-07.html", "30435-08.html"])
            .page("30435-09.html", &["30435-07.html", "30435-08.html"])
            .plain(&["30435-07.html"])
            .npcs(&[LEOPOLD]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let leopold = t.spawn(LEOPOLD, 20);
        (t, leopold)
    }

    #[test]
    fn a_level_twenty_character_is_sent_away() {
        let (mut t, leopold) = setup();
        t.enter_as(1, 20, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, leopold).contains("30435-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_badge_then_the_reward_for_the_badges() {
        let (mut t, leopold) = setup();
        t.enter_as(1, 21, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, leopold).contains("30435-02.htm"));
        assert!(t.event(1, NAME, "30435-03.htm").contains("30435-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // No badge yet.
        assert!(t.quest_page(1, leopold).contains("30435-04.html"));

        // Patrols drop a red badge at 61%: the first one ends the hunt. One badge: 46 adena,
        // no bonus, the page 05.
        for _ in 0..400 {
            let patrol = t.spawn(20053, 60);
            t.kill(1, patrol);
            if t.count(1, RED_CROSS_BADGE) >= 1 {
                break;
            }
        }
        assert_eq!(t.count(1, RED_CROSS_BADGE), 1);
        assert!(t.quest_page(1, leopold).contains("30435-05.html"));
        assert_eq!(t.count(1, 57), 46);
        assert_eq!(t.count(1, RED_CROSS_BADGE), 0);
    }

    #[test]
    fn a_hundred_badges_give_the_mark_and_the_bonus() {
        let (mut t, leopold) = setup();
        t.enter_as(
            1,
            21,
            race::HUMAN,
            player_class::FIGHTER,
            vec![item(900, 1359, 60), item(901, 1360, 40)],
        );
        t.quest_page(1, leopold);
        assert!(t.event(1, NAME, "30435-03.htm").contains("30435-03.htm"));
        t.take_writes();

        // 60 red (46) + 40 blue (52) + 10 for the bonus: the Black Lion Mark, the page 09.
        assert!(t.quest_page(1, leopold).contains("30435-09.html"));
        assert_eq!(t.count(1, 57), 60 * 46 + 40 * 52 + 4320);
        assert_eq!(t.count(1, BLACK_LION_MARK), 1);
        assert_eq!(
            (t.count(1, RED_CROSS_BADGE), t.count(1, BLUE_CROSS_BADGE)),
            (0, 0)
        );
        assert!(t.take_writes().is_empty());

        // The end: the quest ends repeatable with the finish sound.
        t.bypass(1, &format!("Script {NAME} 30435-07.html"));
        let out = t.take();
        assert!(pages_to(&out, 1).pop().unwrap().contains("30435-07.html"));
        assert!(names_to(&out, 1).contains(&"PLAY_SOUND".to_owned()));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }
}
