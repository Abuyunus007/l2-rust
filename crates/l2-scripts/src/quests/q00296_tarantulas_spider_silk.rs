//! Quest 296: Tarantula's Spider Silk (Trader Mion in the Town of Gludio buys the spider silk of the tarantulas).
// Java: dist/game/data/scripts/quests/Q00296_TarantulasSpiderSilk/Q00296_TarantulasSpiderSilk.java
//
// Pattern: a collect quest with a second item (`docs/design/scripts.md`, "Quest patterns"): the
// tarantulas drop spider silk, and rarely a spinnerette; Defender Nathan turns each spinnerette into
// 15 to 23 silk. Trader Mion buys the silk for 30 adena each, with a bonus from 10 silk, and tells
// the Newbie Guide. The quest goes on until the player leaves it.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const TRADER_MION: i32 = 30519;
const DEFENDER_NATHAN: i32 = 30548;

// Monsters
const MONSTERS: [i32; 3] = [20394, 20403, 20508];

// Items
const TARANTULA_SPIDER_SILK: i32 = 1493;
const TARANTULA_SPINNERETTE: i32 = 1494;

// Misc
const MIN_LEVEL: i32 = 15;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00296_TarantulasSpiderSilk`.
pub struct Q00296TarantulasSpiderSilk;

impl Script for Q00296TarantulasSpiderSilk {
    fn name(&self) -> &'static str {
        "Q00296_TarantulasSpiderSilk"
    }

    fn path(&self) -> &'static str {
        "quests/Q00296_TarantulasSpiderSilk"
    }

    fn id(&self) -> i32 {
        296
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[TRADER_MION]);
        s.add_talk_id(&[TRADER_MION, DEFENDER_NATHAN]);
        s.add_kill_id(&MONSTERS);
        s.register_quest_items(&[TARANTULA_SPIDER_SILK, TARANTULA_SPINNERETTE]);
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
        let mut html = None;
        match event {
            "30519-03.htm" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    html = Some(event.to_owned());
                }
            }
            "30519-06.html" => {
                if qs.is_started(ctx) {
                    qs.exit_quest(ctx, true, true);
                    html = Some(event.to_owned());
                }
            }
            "30519-07.html" => {
                if qs.is_started(ctx) {
                    html = Some(event.to_owned());
                }
            }
            "30548-03.html" => {
                if qs.is_started(ctx) {
                    if ctx.has_quest_items(player, TARANTULA_SPINNERETTE) {
                        let spinnerettes = ctx.get_quest_items_count(player, TARANTULA_SPINNERETTE);
                        let silk = (15 + ctx.get_random(9)) as i64 * spinnerettes;
                        ctx.give_items(player, TARANTULA_SPIDER_SILK, silk);
                        ctx.take_items(player, TARANTULA_SPINNERETTE, -1);
                        html = Some(event.to_owned());
                    } else {
                        html = Some("30548-02.html".to_owned());
                    }
                }
            }
            _ => {}
        }
        html
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let qs = ctx.get_quest_state(killer, false);
        if qs.is_some() && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            let chance = ctx.get_random(100);
            if chance > 95 {
                // Java's 7-argument overload: one spinnerette, no limit, always dropped.
                ctx.give_item_randomly(killer, Some(npc), TARANTULA_SPINNERETTE, 1, 1, 0, 1.0, true);
            } else if chance > 45 {
                ctx.give_item_randomly(killer, Some(npc), TARANTULA_SPIDER_SILK, 1, 1, 0, 1.0, true);
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, talker: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(talker, true)?;
        let mut html = ctx.get_no_quest_msg(talker);
        if qs.state(ctx) == CREATED && npc.id == TRADER_MION {
            html = if ctx.player_level(talker) >= MIN_LEVEL {
                "30519-02.htm"
            } else {
                "30519-01.htm"
            }
            .to_owned();
        } else if qs.state(ctx) == STARTED {
            if npc.id == TRADER_MION {
                let silk = ctx.get_quest_items_count(talker, TARANTULA_SPIDER_SILK);
                if silk >= 1 {
                    let bonus = if silk >= 10 { 2000 } else { 0 };
                    ctx.give_adena(talker, silk * 30 + bonus, true);
                    ctx.take_items(talker, TARANTULA_SPIDER_SILK, -1);

                    // Newbie Guide: only when `ai/others/NewbieGuide` is installed.
                    if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, talker, true) {
                        if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                            ctx.set_nr_memo(guide, GUIDE_MISSION);
                            ctx.set_nr_memo_state(guide, GUIDE_MISSION, 100_000);
                            ctx.show_on_screen_npc_string(
                                talker,
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
                                talker,
                                NpcStringId::LastDutyCompleteNGoFindTheNewbieGuide,
                                2,
                                5000,
                                &[],
                            );
                        }
                        guide.set_state(ctx, COMPLETED);
                    }

                    html = "30519-05.html".to_owned();
                } else {
                    html = "30519-04.html".to_owned();
                }
            } else if npc.id == DEFENDER_NATHAN {
                html = "30548-01.html".to_owned();
            }
        }
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::script::{player_class, race};
    use l2_world::{Script, ScriptRegistry, ScriptSetup};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00296_TarantulasSpiderSilk";

    const NPCS: &str = r#"
        <npc id="30519" type="Folk" level="70" name="Trader Mion"/>
        <npc id="30548" type="Folk" level="70" name="Defender Nathan"/>
        <npc id="20394" type="Monster" level="20" name="Tarantula"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="1493" type="EtcItem" name="Tarantula Spider Silk"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="1494" type="EtcItem" name="Tarantula Spinnerette"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>"#;

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

    fn setup() -> (ScriptTest, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(NewbieGuide);
        registry.add(Q00296TarantulasSpiderSilk);
        let pages = Pages::of(&Q00296TarantulasSpiderSilk)
            .page("30519-02.htm", &["30519-03.htm"])
            .plain(&[
                "30519-01.htm",
                "30519-03.htm",
                "30519-04.html",
                "30519-06.html",
                "30519-07.html",
            ])
            .page("30519-05.html", &["30519-06.html", "30519-07.html"])
            .page("30548-01.html", &["30548-03.html"])
            .plain(&["30548-02.html", "30548-03.html"])
            .npcs(&[TRADER_MION, DEFENDER_NATHAN]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let mion = t.spawn(TRADER_MION, 20);
        let nathan = t.spawn(DEFENDER_NATHAN, 40);
        (t, mion, nathan)
    }

    #[test]
    fn a_level_fourteen_character_is_sent_away() {
        let (mut t, mion, _nathan) = setup();
        t.enter_as(1, 14, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, mion).contains("30519-01.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn silk_for_adena_and_the_spinnerettes_from_nathan_then_the_guide() {
        let (mut t, mion, nathan) = setup();
        t.enter_as(1, 15, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, mion).contains("30519-02.htm"));
        assert!(t.event(1, NAME, "30519-03.htm").contains("30519-03.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );
        // Mion has nothing to buy yet; Nathan has no spinnerette to trade.
        assert!(t.quest_page(1, mion).contains("30519-04.html"));
        assert!(t.quest_page(1, nathan).contains("30548-01.html"));
        assert!(t.event(1, NAME, "30548-03.html").contains("30548-02.html"));

        // Silk comes often, spinnerettes rarely: ten silk and one spinnerette.
        for _ in 0..2000 {
            if t.count(1, TARANTULA_SPIDER_SILK) >= 10 && t.count(1, TARANTULA_SPINNERETTE) >= 1 {
                break;
            }
            let tarantula = t.spawn(20394, 60);
            t.kill(1, tarantula);
        }
        assert!(t.take_writes().is_empty());

        // Nathan: 15 to 23 silk for each spinnerette.
        let silk = t.count(1, TARANTULA_SPIDER_SILK);
        assert!(t.quest_page(1, nathan).contains("30548-01.html"));
        assert!(t.event(1, NAME, "30548-03.html").contains("30548-03.html"));
        assert_eq!(t.count(1, TARANTULA_SPINNERETTE), 0);
        let gained = t.count(1, TARANTULA_SPIDER_SILK) - silk;
        assert!((15..=23).contains(&gained), "{gained}");

        // Mion buys the silk: 30 adena each and 2000 more from 10; the guide hears about it and
        // the guide's state is completed.
        let silk = t.count(1, TARANTULA_SPIDER_SILK);
        assert!(silk >= 10);
        let before = t.count(1, 57);
        assert!(t.quest_page(1, mion).contains("30519-05.html"));
        assert_eq!(t.count(1, 57) - before, silk * 30 + 2000);
        assert_eq!(t.count(1, TARANTULA_SPIDER_SILK), 0);
        let writes = t.take_writes();
        assert_eq!(
            writes[..2],
            [
                rows::insert(NEWBIE_GUIDE, "NRmemo", "41"),
                rows::insert(NEWBIE_GUIDE, "NRmemoState41", "100000"),
            ]
        );
        assert_eq!(writes.len(), 3, "{writes:?}");
        assert_eq!(writes[2], rows::insert(NEWBIE_GUIDE, "<state>", "Completed"));

        // `exitQuest(true, true)` from the page.
        assert!(t.event(1, NAME, "30519-06.html").contains("30519-06.html"));
        assert_eq!(t.take_writes(), [rows::delete(NAME, true)]);
    }
}
