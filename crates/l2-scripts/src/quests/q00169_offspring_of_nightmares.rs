//! Quest 169: Offspring of Nightmares (Vlasty in the Dark Elven Village, Dark Horrors).
// Java: dist/game/data/scripts/quests/Q00169_OffspringOfNightmares/Q00169_OffspringOfNightmares.java

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, QuestSound, STARTED, race};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPC
const VLASTY: i32 = 30145;

// Monsters
const LESSER_DARK_HORROR: i32 = 20025;
const DARK_HORROR: i32 = 20105;

// Items
const BONE_GAITERS: i32 = 31;
const CRACKED_SKULL: i32 = 1030;
const PERFECT_SKULL: i32 = 1031;

// Misc
const MIN_LEVEL: i32 = 15;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00169_OffspringOfNightmares`.
pub struct Q00169OffspringOfNightmares;

impl Script for Q00169OffspringOfNightmares {
    fn name(&self) -> &'static str {
        "Q00169_OffspringOfNightmares"
    }

    fn path(&self) -> &'static str {
        "quests/Q00169_OffspringOfNightmares"
    }

    fn id(&self) -> i32 {
        169
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[VLASTY]);
        s.add_talk_id(&[VLASTY]);
        s.add_kill_id(&[LESSER_DARK_HORROR, DARK_HORROR]);
        s.register_quest_items(&[CRACKED_SKULL, PERFECT_SKULL]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        _npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let mut htmltext = None;
        if let Some(qs) = ctx.get_quest_state(player, false) {
            match event {
                "30145-03.htm" => {
                    qs.start_quest(ctx);
                    htmltext = Some(event.to_owned());
                }
                "30145-07.html" => {
                    if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, PERFECT_SKULL) {
                        // Newbie Guide: the mission counter of the guide, when it is in the game.
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
                    }
                    ctx.give_items(player, BONE_GAITERS, 1);
                    ctx.add_exp_and_sp(player, 17475, 818);
                    let skulls = ctx.get_quest_items_count(player, CRACKED_SKULL);
                    ctx.give_adena(player, 17030 + 10 * skulls, true);
                    qs.exit_quest(ctx, false, true);
                    htmltext = Some(event.to_owned());
                }
                _ => {}
            }
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        if let Some(qs) = ctx.get_quest_state(killer, false)
            && qs.is_started(ctx)
        {
            if ctx.get_random(10) > 7 && !ctx.has_quest_items(killer, PERFECT_SKULL) {
                ctx.give_items(killer, PERFECT_SKULL, 1);
                qs.set_cond(ctx, 2, true);
            } else if ctx.get_random(10) > 4 {
                ctx.give_items(killer, CRACKED_SKULL, 1);
                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, _npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        match qs.state(ctx) {
            CREATED => {
                htmltext = if ctx.player_race(player) == race::DARK_ELF {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        "30145-02.htm"
                    } else {
                        "30145-01.htm"
                    }
                } else {
                    "30145-00.htm"
                }
                .to_owned();
            }
            STARTED => {
                if ctx.has_quest_items(player, CRACKED_SKULL) && !ctx.has_quest_items(player, PERFECT_SKULL) {
                    htmltext = "30145-05.html".to_owned();
                } else if qs.is_cond(ctx, 2) && ctx.has_quest_items(player, PERFECT_SKULL) {
                    htmltext = "30145-06.html".to_owned();
                } else if !ctx.has_all_quest_items(player, &[CRACKED_SKULL, PERFECT_SKULL]) {
                    htmltext = "30145-04.html".to_owned();
                }
            }
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
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
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00169_OffspringOfNightmares";

    const NPCS: &str = r#"
        <npc id="30145" type="Folk" level="70" name="Vlasty"/>
        <npc id="20025" type="Monster" level="30" name="Lesser Dark Horror"/>
        <npc id="20105" type="Monster" level="31" name="Dark Horror"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="31" type="Armor" name="Bone Gaiters"><set name="bodypart" val="legs"/></item>
        <item id="1030" type="EtcItem" name="Cracked Skull"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1031" type="EtcItem" name="Perfect Skull"><set name="is_questitem" val="true"/></item>"#;

    /// Java `NewbieGuide` as far as this quest writes it: a script of that name.
    struct NewbieGuide;

    impl Script for NewbieGuide {
        fn name(&self) -> &'static str {
            NEWBIE_GUIDE
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
        registry.add(Q00169OffspringOfNightmares);
        let pages = Pages::of(&Q00169OffspringOfNightmares)
            .page("30145-02.htm", &["30145-03.htm"])
            .page("30145-06.html", &["30145-07.html"])
            .plain(&[
                "30145-00.htm",
                "30145-01.htm",
                "30145-03.htm",
                "30145-04.html",
                "30145-05.html",
                "30145-07.html",
            ])
            .npcs(&[VLASTY]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let vlasty = t.spawn(VLASTY, 20);
        (t, vlasty)
    }

    /// Kills Dark Horrors until the perfect skull is in; returns whether the cracked-skull page
    /// (cond 1, no perfect skull) was shown on the way.
    fn hunt(t: &mut ScriptTest, vlasty: Oid) -> bool {
        let mut shown = false;
        for _ in 0..300 {
            if t.count(1, PERFECT_SKULL) >= 1 {
                return shown;
            }
            if t.count(1, CRACKED_SKULL) >= 1 && !shown {
                assert!(t.quest_page(1, vlasty).contains("30145-05.html"));
                shown = true;
            }
            let horror = t.spawn(DARK_HORROR, 60);
            t.kill(1, horror);
        }
        panic!("no perfect skull after 300 kills");
    }

    #[test]
    fn the_dark_elves_of_level_15_are_offered_the_quest() {
        let (mut t, vlasty) = setup(false);
        t.enter_as(1, 15, race::DARK_ELF, 31, vec![]);
        assert!(t.quest_page(1, vlasty).contains("30145-02.htm"));
        t.enter_as(2, 14, race::DARK_ELF, 31, vec![]);
        assert!(t.quest_page(2, vlasty).contains("30145-01.htm"));
        t.enter_as(3, 20, race::HUMAN, 0, vec![]);
        assert!(t.quest_page(3, vlasty).contains("30145-00.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_skulls_then_the_gaiters_and_the_adena() {
        let (mut t, vlasty) = setup(true);
        t.enter_as(1, 15, race::DARK_ELF, 31, vec![]);
        assert!(t.quest_page(1, vlasty).contains("30145-02.htm"));
        assert!(t.event(1, NAME, "30145-03.htm").contains("30145-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        let shown = hunt(&mut t, vlasty);
        assert!(shown, "the cracked-skull page was never shown");
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        assert!(t.quest_page(1, vlasty).contains("30145-06.html"));

        // The reward: the gaiters, exp and sp, adena by the cracked skulls, `exitQuest(false, true)`.
        let skulls = t.count(1, CRACKED_SKULL);
        t.bypass(1, &format!("Script {NAME} 30145-07.html"));
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"EX_SHOW_SCREEN_MESSAGE".to_owned()));
        assert_eq!(t.count(1, BONE_GAITERS), 1);
        assert_eq!(t.count(1, 57), 17_030 + 10 * skulls);
        let guide = |var: &str, value: &str| rows::insert(NEWBIE_GUIDE, var, value);
        let writes = t.take_writes();
        assert_eq!(
            writes[..3],
            [
                guide("NRmemo", "41"),
                guide("NRmemoState41", "100000"),
                guide("<state>", "Completed")
            ]
        );
        assert_eq!(
            writes[3..],
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, vlasty).contains("already been completed"));
    }
}
