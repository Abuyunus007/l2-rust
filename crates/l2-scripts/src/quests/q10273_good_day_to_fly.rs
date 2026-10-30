//! Quest 10273: Good Day to Fly (Lekon teaches the Aura Bird transformation after five Vulture Rider kills).
// Java: dist/game/data/scripts/quests/Q10273_GoodDayToFly/Q10273_GoodDayToFly.java

use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPC
const LEKON: i32 = 32557;

// Monsters
const VULTURE_RIDERS: [i32; 2] = [22614, 22615];

// Item
const MARK: i32 = 13856;

// Rewards
const TRANSFORM_FALCON_ITEM: i32 = 13553;
const TRANSFORM_OWL_ITEM: i32 = 13554;
const REWARD_ITEM: i32 = 13857;

// Skills: the Aura Bird transformations.
const AURA_BIRD_FALCON: SkillKey = SkillKey { id: 5982, level: 1 };
const AURA_BIRD_OWL: SkillKey = SkillKey { id: 5983, level: 1 };

/// Java `Q10273_GoodDayToFly`.
pub struct Q10273GoodDayToFly;

impl Script for Q10273GoodDayToFly {
    fn name(&self) -> &'static str {
        "Q10273_GoodDayToFly"
    }

    fn path(&self) -> &'static str {
        "quests/Q10273_GoodDayToFly"
    }

    fn id(&self) -> i32 {
        10273
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[LEKON]);
        s.add_talk_id(&[LEKON]);
        s.add_kill_id(&VULTURE_RIDERS);
        s.register_quest_items(&[MARK]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let Some(qs) = ctx.get_quest_state(player, false) else {
            return Some(ctx.get_no_quest_msg(player));
        };

        match event {
            "32557-06.htm" => qs.start_quest(ctx),
            "32557-09.html" => {
                qs.set(ctx, "transform", "1");
                ctx.world_mut().apply_effects(player, player, AURA_BIRD_FALCON);
            }
            "32557-10.html" => {
                qs.set(ctx, "transform", "2");
                ctx.world_mut().apply_effects(player, player, AURA_BIRD_OWL);
            }
            "32557-13.html" => match qs.get_int(ctx, "transform") {
                1 => ctx.world_mut().apply_effects(player, player, AURA_BIRD_FALCON),
                2 => ctx.world_mut().apply_effects(player, player, AURA_BIRD_OWL),
                _ => {}
            },
            _ => {}
        }
        Some(event.to_owned())
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }

        let count = ctx.get_quest_items_count(killer, MARK);
        if qs.is_cond(ctx, 1) && count < 5 {
            ctx.give_items(killer, MARK, 1);
            if count == 4 {
                qs.set_cond(ctx, 2, true);
            } else {
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let transform = qs.get_int(ctx, "transform");

        let htmltext = match qs.state(ctx) {
            COMPLETED => "32557-0a.html",
            CREATED => {
                if ctx.player_level(player) < 75 {
                    "32557-00.html"
                } else {
                    "32557-01.htm"
                }
            }
            _ => {
                if ctx.get_quest_items_count(player, MARK) >= 5 {
                    match transform {
                        1 => ctx.give_items(player, TRANSFORM_FALCON_ITEM, 1),
                        2 => ctx.give_items(player, TRANSFORM_OWL_ITEM, 1),
                        _ => {}
                    }
                    ctx.give_items(player, REWARD_ITEM, 1);
                    ctx.add_exp_and_sp(player, 25_160, 2_525);
                    qs.exit_quest(ctx, false, true);
                    "32557-14.html"
                } else if transform == 0 {
                    "32557-07.html"
                } else {
                    "32557-11.html"
                }
            }
        };
        Some(htmltext.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{QuestWrite, ScriptRegistry};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q10273_GoodDayToFly";

    const NPCS: &str = r#"
        <npc id="32557" type="Folk" level="80" name="Lekon"/>
        <npc id="22614" type="Monster" level="60" name="Vulture Rider"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="13856" type="EtcItem" name="Vulture Mark"><set name="is_questitem" val="true"/></item>
        <item id="13553" type="EtcItem" name="Falcon Transformation"/>
        <item id="13554" type="EtcItem" name="Owl Transformation"/>
        <item id="13857" type="EtcItem" name="Reward"/>"#;

    fn setup() -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q10273GoodDayToFly);
        let pages = Pages::of(&Q10273GoodDayToFly)
            .plain(&["32557-00.html", "32557-08.html", "32557-0a.html", "32557-12.html"])
            .plain(&["32557-13.html", "32557-14.html", "32557-09.html", "32557-10.html"])
            .page("32557-01.htm", &["32557-02.htm"])
            .page("32557-02.htm", &["32557-03.htm"])
            .page("32557-03.htm", &["32557-04.htm"])
            .page("32557-04.htm", &["32557-05.htm"])
            .page("32557-05.htm", &["32557-06.htm"])
            .page("32557-06.htm", &["32557-09.html", "32557-10.html", "32557-08.html"])
            .page("32557-07.html", &["32557-09.html", "32557-10.html", "32557-08.html"])
            .page("32557-11.html", &["32557-13.html", "32557-12.html"])
            .npcs(&[LEKON]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let lekon = t.spawn(LEKON, 20);
        (t, lekon)
    }

    fn transform_write(value: &str) -> QuestWrite {
        rows::insert(NAME, "transform", value)
    }

    #[test]
    fn a_character_below_level_75_is_sent_away() {
        let (mut t, lekon) = setup();
        t.enter(1, 74, vec![]);
        assert!(t.quest_page(1, lekon).contains("32557-00.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn five_vulture_riders_then_the_transformation_reward() {
        let (mut t, lekon) = setup();
        t.enter(1, 75, vec![]);
        assert!(t.quest_page(1, lekon).contains("32557-01.htm"));
        // The story pages; the last link (`32557-06.htm`) starts the quest.
        for page in ["32557-02.htm", "32557-03.htm", "32557-04.htm", "32557-05.htm", "32557-06.htm"] {
            assert!(t.event(1, NAME, page).contains(page));
        }
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
            ]
        );

        // No transformation yet: the choice page.
        assert!(t.quest_page(1, lekon).contains("32557-07.html"));
        // The falcon: the variable and the skill.
        assert!(t.event(1, NAME, "32557-09.html").contains("32557-09.html"));
        assert_eq!(t.take_writes(), [transform_write("1")]);
        assert!(t.quest_page(1, lekon).contains("32557-11.html"));

        // Five Vulture Riders: the fifth sets cond 2.
        for _ in 0..5 {
            let rider = t.spawn(22614, 60);
            t.kill(1, rider);
        }
        assert_eq!(t.count(1, MARK), 5);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // The falcon transformation, the reward, the exp and `exitQuest(false, true)`.
        assert!(t.quest_page(1, lekon).contains("32557-14.html"));
        assert_eq!(t.count(1, TRANSFORM_FALCON_ITEM), 1);
        assert_eq!(t.count(1, REWARD_ITEM), 1);
        assert_eq!(t.count(1, MARK), 0);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(t.quest_page(1, lekon).contains("32557-0a.html"));
    }
}
