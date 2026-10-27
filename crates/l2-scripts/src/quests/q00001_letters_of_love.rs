//! Quest 1: Letters of Love (Darin, Roxxy and Baulro in Talking Island).
// Java: dist/game/data/scripts/quests/Q00001_LettersOfLove/Q00001_LettersOfLove.java
//
// The reference port of a quest for the batch workers (`docs/design/scripts.md`): constants as
// in Java, `setup` = the constructor, `on_event` / `on_talk` line by line.

use l2_protocol_game::NpcStringId;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const DARIN: i32 = 30048;
const ROXXY: i32 = 30006;
const BAULRO: i32 = 30033;

// Items
const DARINS_LETTER: i32 = 687;
const ROXXYS_KERCHIEF: i32 = 688;
const DARINS_RECEIPT: i32 = 1079;
const BAULROS_POTION: i32 = 1080;
const NECKLACE_OF_KNOWLEDGE: i32 = 906;

// Misc
const MIN_LEVEL: i32 = 2;
const GUIDE_MISSION: i32 = 41;
/// Java `NewbieGuide.class.getSimpleName()`.
const NEWBIE_GUIDE: &str = "NewbieGuide";

/// Java `Q00001_LettersOfLove`.
pub struct Q00001LettersOfLove;

impl Script for Q00001LettersOfLove {
    fn name(&self) -> &'static str {
        "Q00001_LettersOfLove"
    }

    fn path(&self) -> &'static str {
        "quests/Q00001_LettersOfLove"
    }

    fn id(&self) -> i32 {
        1
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[DARIN]);
        s.add_talk_id(&[DARIN, ROXXY, BAULRO]);
        s.register_quest_items(&[DARINS_LETTER, ROXXYS_KERCHIEF, DARINS_RECEIPT, BAULROS_POTION]);
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
            "30048-03.html" | "30048-04.html" | "30048-05.html" => {
                htmltext = Some(event.to_owned());
            }
            "30048-06.htm" => {
                if ctx.player_level(player) >= MIN_LEVEL {
                    qs.start_quest(ctx);
                    ctx.give_items(player, DARINS_LETTER, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let page = |name: &str| name.to_owned();
        match qs.state(ctx) {
            CREATED => {
                htmltext = page(if ctx.player_level(player) < MIN_LEVEL {
                    "30048-01.html"
                } else {
                    "30048-02.html"
                });
            }
            STARTED => match qs.get_cond(ctx) {
                1 => match npc.id {
                    DARIN => htmltext = page("30048-07.html"),
                    ROXXY => {
                        if ctx.has_quest_items(player, DARINS_LETTER) && !ctx.has_quest_items(player, ROXXYS_KERCHIEF) {
                            ctx.take_items(player, DARINS_LETTER, -1);
                            ctx.give_items(player, ROXXYS_KERCHIEF, 1);
                            qs.set_cond(ctx, 2, true);
                            htmltext = page("30006-01.html");
                        }
                    }
                    _ => {}
                },
                2 => match npc.id {
                    DARIN => {
                        if ctx.has_quest_items(player, ROXXYS_KERCHIEF) {
                            ctx.take_items(player, ROXXYS_KERCHIEF, -1);
                            ctx.give_items(player, DARINS_RECEIPT, 1);
                            qs.set_cond(ctx, 3, true);
                            htmltext = page("30048-08.html");
                        }
                    }
                    ROXXY => {
                        if ctx.has_quest_items(player, ROXXYS_KERCHIEF) {
                            htmltext = page("30006-02.html");
                        }
                    }
                    _ => {}
                },
                3 => match npc.id {
                    DARIN => {
                        if ctx.has_quest_items(player, DARINS_RECEIPT) || !ctx.has_quest_items(player, BAULROS_POTION) {
                            htmltext = page("30048-09.html");
                        }
                    }
                    ROXXY => {
                        if ctx.has_quest_items(player, DARINS_RECEIPT) || ctx.has_quest_items(player, BAULROS_POTION) {
                            htmltext = page("30006-03.html");
                        }
                    }
                    BAULRO => {
                        if ctx.has_quest_items(player, DARINS_RECEIPT) {
                            ctx.take_items(player, DARINS_RECEIPT, -1);
                            ctx.give_items(player, BAULROS_POTION, 1);
                            qs.set_cond(ctx, 4, true);
                            htmltext = page("30033-01.html");
                        } else if ctx.has_quest_items(player, BAULROS_POTION) {
                            htmltext = page("30033-02.html");
                        }
                    }
                    _ => {}
                },
                4 => match npc.id {
                    DARIN => {
                        // Newbie Guide (only once `ai/others/NewbieGuide` is ported; Java skips
                        // the block the same way when the script is missing).
                        if let Some(guide) = ctx.quest_state_of(NEWBIE_GUIDE, player, true) {
                            if !ctx.have_nr_memo(guide, GUIDE_MISSION) {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, 1);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::DeliveryDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            } else if ctx.get_nr_memo_state(guide, GUIDE_MISSION) % 10 != 1 {
                                ctx.set_nr_memo(guide, GUIDE_MISSION);
                                let state = ctx.get_nr_memo_state(guide, GUIDE_MISSION);
                                ctx.set_nr_memo_state(guide, GUIDE_MISSION, state + 1);
                                ctx.show_on_screen_npc_string(
                                    player,
                                    NpcStringId::DeliveryDutyCompleteNGoFindTheNewbieGuide,
                                    2,
                                    5000,
                                    &[],
                                );
                            }
                        }

                        ctx.give_items(player, NECKLACE_OF_KNOWLEDGE, 1);
                        ctx.add_exp_and_sp(player, 5672, 446);
                        ctx.give_adena(player, 2466, false);
                        qs.exit_quest(ctx, false, true);
                        htmltext = page("30048-10.html");
                    }
                    BAULRO => {
                        if ctx.has_quest_items(player, BAULROS_POTION) {
                            htmltext = page("30033-02.html");
                        }
                    }
                    ROXXY => {
                        if ctx.has_quest_items(player, BAULROS_POTION) {
                            htmltext = page("30006-03.html");
                        }
                    }
                    _ => {}
                },
                _ => {}
            },
            COMPLETED => htmltext = ctx.get_already_completed_msg(player),
            _ => {}
        }
        Some(htmltext)
    }
}
