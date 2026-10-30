//! Quest 288: Handle With Care (Ankumi in the Town of Gludin sends level 82 players after the seer Ugoros).
// Java: dist/game/data/scripts/quests/Q00288_HandleWithCare/Q00288_HandleWithCare.java
//
// Pattern: a kill quest that ends with a choice of reward (`docs/design/scripts.md`, "Quest
// patterns"): the seer Ugoros drops a lizard scale (middle grade first); Ankumi trades the scale
// for one reward picked by a roll of ten, a high grade scale also brings a holy crystal.

use l2_util::ObjectId;
use l2_world::script::{CREATED, QuestSound, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ANKUMI: i32 = 32741;

// Monster
const SEER_UGOROS: i32 = 18863;

// Items
const HIGH_GRADE_LIZARD_SCALE: i32 = 15497;
const MIDDLE_GRADE_LIZARD_SCALE: i32 = 15498;
const SCROLL_ENCHANT_WEAPON_S_GRADE: i32 = 959;
const SCROLL_ENCHANT_ARMOR_S_GRADE: i32 = 960;
const HOLY_CRYSTAL: i32 = 9557;

/// Java `REWARDS`: `(item, count)`.
const REWARDS: [(i32, i64); 6] = [
    (SCROLL_ENCHANT_WEAPON_S_GRADE, 1),
    (SCROLL_ENCHANT_ARMOR_S_GRADE, 1),
    (SCROLL_ENCHANT_ARMOR_S_GRADE, 2),
    (SCROLL_ENCHANT_ARMOR_S_GRADE, 3),
    (HOLY_CRYSTAL, 1),
    (HOLY_CRYSTAL, 2),
];

// Misc
const MIN_LEVEL: i32 = 82;

/// Java `Q00288_HandleWithCare`.
pub struct Q00288HandleWithCare;

impl Script for Q00288HandleWithCare {
    fn name(&self) -> &'static str {
        "Q00288_HandleWithCare"
    }

    fn path(&self) -> &'static str {
        "quests/Q00288_HandleWithCare"
    }

    fn id(&self) -> i32 {
        288
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ANKUMI]);
        s.add_talk_id(&[ANKUMI]);
        s.add_kill_id(&[SEER_UGOROS]);
        s.register_quest_items(&[HIGH_GRADE_LIZARD_SCALE, MIDDLE_GRADE_LIZARD_SCALE]);
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
            "32741-03.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    htmltext = Some(event.to_owned());
                }
            }
            "32741-04.html" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
            }
            "32741-08.html" => {
                if qs.is_cond(ctx, 2) || qs.is_cond(ctx, 3) {
                    let mut reward = None;
                    if ctx.has_quest_items(player, MIDDLE_GRADE_LIZARD_SCALE) {
                        ctx.take_items(player, MIDDLE_GRADE_LIZARD_SCALE, 1);
                        reward = Some(match ctx.get_random(10) {
                            0 => REWARDS[0],
                            1..=3 => REWARDS[1],
                            4..=5 => REWARDS[2],
                            6 => REWARDS[3],
                            7..=8 => REWARDS[4],
                            _ => REWARDS[5],
                        });
                    } else if ctx.has_quest_items(player, HIGH_GRADE_LIZARD_SCALE) {
                        ctx.take_items(player, HIGH_GRADE_LIZARD_SCALE, 1);
                        reward = Some(match ctx.get_random(10) {
                            0 => REWARDS[0],
                            1..=4 => REWARDS[1],
                            5..=7 => REWARDS[2],
                            _ => REWARDS[3],
                        });
                        ctx.give_items(player, REWARDS[4].0, REWARDS[4].1);
                    }
                    if let Some((item, count)) = reward {
                        ctx.give_items(player, item, count);
                    }
                    qs.exit_quest(ctx, true, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, false)
        {
            // NOTE(java-quirk): the high grade scale is only ever given from cond 1, which the
            // middle grade scale has already moved to cond 2; Java has the same order.
            if !ctx.has_quest_items(killer, MIDDLE_GRADE_LIZARD_SCALE) {
                ctx.give_items(killer, MIDDLE_GRADE_LIZARD_SCALE, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                qs.set_cond(ctx, 2, true);
            } else if !ctx.has_quest_items(killer, HIGH_GRADE_LIZARD_SCALE) {
                ctx.give_items(killer, HIGH_GRADE_LIZARD_SCALE, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                qs.set_cond(ctx, 3, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) < MIN_LEVEL {
                    "32741-01.html"
                } else {
                    "32741-02.htm"
                }
                .to_owned();
            }
            STARTED => {
                let high = ctx.has_quest_items(player, HIGH_GRADE_LIZARD_SCALE);
                let middle = ctx.has_quest_items(player, MIDDLE_GRADE_LIZARD_SCALE);
                if qs.is_cond(ctx, 1) && !high && !middle {
                    htmltext = "32741-05.html".to_owned();
                } else if qs.is_cond(ctx, 2) && middle {
                    htmltext = "32741-06.html".to_owned();
                }
                if qs.is_cond(ctx, 2) && high {
                    htmltext = "32741-07.html".to_owned();
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
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00288_HandleWithCare";

    const NPCS: &str = r#"
        <npc id="32741" type="Folk" level="85" name="Ankumi"/>
        <npc id="18863" type="Monster" level="80" name="Seer Ugoros"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="15497" type="EtcItem" name="High Grade Lizard Scale"><set name="is_questitem" val="true"/></item>
        <item id="15498" type="EtcItem" name="Middle Grade Lizard Scale"><set name="is_questitem" val="true"/></item>
        <item id="959" type="EtcItem" name="Scroll: Enchant Weapon (S)"><set name="is_stackable" val="true"/></item>
        <item id="960" type="EtcItem" name="Scroll: Enchant Armor (S)"><set name="is_stackable" val="true"/></item>
        <item id="9557" type="EtcItem" name="Holy Crystal"><set name="is_stackable" val="true"/></item>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00288HandleWithCare);
        let pages = Pages::of(&Q00288HandleWithCare)
            .page("32741-02.htm", &["32741-03.htm"])
            .page("32741-03.htm", &["32741-04.html"])
            .plain(&[
                "32741-01.html",
                "32741-04.html",
                "32741-05.html",
                "32741-07.html",
                "32741-08.html",
            ])
            .page("32741-06.html", &["32741-08.html"])
            .npcs(&[ANKUMI]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let ankumi = t.spawn(ANKUMI, 20);
        (t, ankumi)
    }

    #[test]
    fn a_level_eighty_one_character_is_sent_away() {
        let (mut t, ankumi) = setup();
        t.enter_as(1, 81, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, ankumi).contains("32741-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_middle_scale_then_one_reward_and_the_quest_ends() {
        let (mut t, ankumi) = setup();
        t.enter_as(1, 82, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, ankumi).contains("32741-02.htm"));
        assert!(t.event(1, NAME, "32741-03.htm").contains("32741-03.htm"));
        assert!(t.event(1, NAME, "32741-04.html").contains("32741-04.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, ankumi).contains("32741-05.html"));

        // The first scale sets cond 2 (`setCond(2, true)`).
        let ugoros = t.spawn(SEER_UGOROS, 60);
        t.kill(1, ugoros);
        assert_eq!(t.count(1, MIDDLE_GRADE_LIZARD_SCALE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Ankumi trades the scale for one reward; `exitQuest(true, true)`.
        assert!(t.quest_page(1, ankumi).contains("32741-06.html"));
        assert!(t.event(1, NAME, "32741-08.html").contains("32741-08.html"));
        assert_eq!(t.count(1, MIDDLE_GRADE_LIZARD_SCALE), 0);
        let rewards: i64 = [959, 960, 9557].iter().map(|&item| t.count(1, item)).sum();
        assert!(rewards >= 1, "{rewards}");
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
