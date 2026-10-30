//! Quest 261: Collector's Dream (Alshupes in the Talking Island area collects spider legs).
// Java: dist/game/data/scripts/quests/Q00261_CollectorsDream/Q00261_CollectorsDream.java
//
// Pattern: a kill-count quest with a drop limit (`docs/design/scripts.md`, "Quest patterns"): the
// spiders drop spider legs in the party range until 8 are in, the 8th sets cond 2; Alshupes pays
// adena and experience, moves the Newbie Guide's mission on and ends the quest.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const ALSHUPES: i32 = 30222;

// Monsters
const HOOK_SPIDER: i32 = 20308;
const CRIMSON_SPIDER: i32 = 20460;
const PINCER_SPIDER: i32 = 20466;

// Item
const SPIDER_LEG: i32 = 1087;

// Misc
const MIN_LEVEL: i32 = 15;
const MAX_LEG_COUNT: i64 = 8;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00261_CollectorsDream`.
pub struct Q00261CollectorsDream;

impl Script for Q00261CollectorsDream {
    fn name(&self) -> &'static str {
        "Q00261_CollectorsDream"
    }

    fn path(&self) -> &'static str {
        "quests/Q00261_CollectorsDream"
    }

    fn id(&self) -> i32 {
        261
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[ALSHUPES]);
        s.add_talk_id(&[ALSHUPES]);
        s.add_kill_id(&[HOOK_SPIDER, CRIMSON_SPIDER, PINCER_SPIDER]);
        s.register_quest_items(&[SPIDER_LEG]);
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
        if event == "30222-03.htm" {
            qs.start_quest(ctx);
            return Some(event.to_owned());
        }
        None
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if let Some(qs) = qs
            && qs.is_cond(ctx, 1)
            && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
            // Java's 6-argument overload: one leg a time, up to 8, always dropped.
            && ctx.give_item_randomly(killer, None, SPIDER_LEG, 1, 1, MAX_LEG_COUNT, 1.0, true)
        {
            // `setCond(2)`: no sound of its own.
            qs.set_cond(ctx, 2, false);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_level(player) >= MIN_LEVEL {
                    "30222-02.htm"
                } else {
                    "30222-01.htm"
                }
                .to_owned();
            }
            STARTED => match qs.get_cond(ctx) {
                1 => htmltext = "30222-04.html".to_owned(),
                2 => {
                    if ctx.get_quest_items_count(player, SPIDER_LEG) >= MAX_LEG_COUNT {
                        // Newbie Guide: only when `ai/others/NewbieGuide` is installed (Java skips
                        // the block the same way when the script is missing).
                        if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                            if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, 100_000);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::LastDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            } else if (ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 100_000_000) / 10_000_000
                                != 1
                            {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 10_000_000);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::LastDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            }
                            guide.set_state(ctx, COMPLETED);
                        }

                        ctx.give_adena(player, 1000, true);
                        ctx.add_exp_and_sp(player, 2000, 0);
                        qs.exit_quest(ctx, true, true);
                        htmltext = "30222-05.html".to_owned();
                    }
                }
                _ => {}
            },
            _ => {}
        }
        Some(htmltext)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00261_CollectorsDream";

    const NPCS: &str = r#"
        <npc id="30222" type="Folk" level="70" name="Alshupes"/>
        <npc id="20308" type="Monster" level="20" name="Hook Spider"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1087" type="EtcItem" name="Spider Leg"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

    /// Java `ai.others.NewbieGuide` as far as this quest needs it: a script of that name.
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            "NewbieGuide"
        }
        fn path(&self) -> &'static str {
            "ai/others/NewbieGuide"
        }
        fn setup(&self, _setup: &mut ScriptSetup) {}
    }

    fn setup(with_guide: bool) -> (ScriptTest, Oid) {
        let mut registry = ScriptRegistry::new();
        if with_guide {
            registry.add(NewbieGuide);
        }
        registry.add(Q00261CollectorsDream);
        let pages = Pages::of(&Q00261CollectorsDream)
            .page("30222-02.htm", &["30222-03.htm"])
            .plain(&["30222-01.htm", "30222-03.htm", "30222-04.html", "30222-05.html"])
            .npcs(&[ALSHUPES]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let alshupes = t.spawn(ALSHUPES, 20);
        (t, alshupes)
    }

    /// Kills hook spiders until 8 legs are in the inventory; returns the kills.
    fn hunt(t: &mut ScriptTest) -> usize {
        for kills in 1..400 {
            if t.count(1, SPIDER_LEG) >= MAX_LEG_COUNT {
                return kills;
            }
            let spider = t.spawn(HOOK_SPIDER, 60);
            t.kill(1, spider);
        }
        panic!("no 8 legs after 400 kills");
    }

    #[test]
    fn a_level_fourteen_character_is_sent_away() {
        let (mut t, alshupes) = setup(false);
        t.enter(1, 14, vec![]);
        assert!(t.quest_page(1, alshupes).contains("30222-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn eight_legs_then_adena_exp_and_the_newbie_guide() {
        let (mut t, alshupes) = setup(true);
        t.enter(1, 15, vec![]);
        assert!(t.quest_page(1, alshupes).contains("30222-02.htm"));
        assert!(t.event(1, NAME, "30222-03.htm").contains("30222-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        assert!(t.quest_page(1, alshupes).contains("30222-04.html"));

        // The 8th leg sets cond 2 (`setCond(2)`, no sound of its own).
        hunt(&mut t);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        // Alshupes: the Newbie Guide's mission 41 moves on, adena and exp, `exitQuest(true, true)`.
        t.talk(1, alshupes);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(
            pages_to(&out, 1)
                .pop()
                .unwrap_or_default()
                .contains("30222-05.html")
        );
        assert!(names_to(&out, 1).contains(&"EX_SHOW_SCREEN_MESSAGE".to_owned()));
        assert_eq!(t.count(1, 57), 1000);
        assert_eq!(t.count(1, SPIDER_LEG), 0);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::insert(NEWBIE_GUIDE, "NRmemo", "41"),
                rows::insert(NEWBIE_GUIDE, "NRmemoState41", "100000"),
            ]
        );
        assert!(writes.contains(&rows::delete(NAME, true)));
        assert!(t.world.quest_list(Oid(1)).quests.is_empty());
    }

    #[test]
    fn without_the_newbie_guide_the_reward_is_the_same() {
        let (mut t, alshupes) = setup(false);
        t.enter(1, 15, vec![]);
        assert!(t.quest_page(1, alshupes).contains("30222-02.htm"));
        t.event(1, NAME, "30222-03.htm");
        hunt(&mut t);
        t.take_writes();
        assert!(t.quest_page(1, alshupes).contains("30222-05.html"));
        assert_eq!(t.count(1, 57), 1000);
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
