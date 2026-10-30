//! Quest 120: Pavel's Last Research (the stone pile, Wendy, Yumi, the three weather masters, the bookshelf).
// Java: dist/game/data/scripts/quests/Q00120_PavelsLastResearch/Q00120_PavelsLastResearch.java
//
// Pattern: a memo state chain where almost every link page is guarded by the memo state (and
// items), and three "puzzles" (the weather masters) that keep their progress as decimal digits in
// `memoStateEx(0)`. Requires Q00114 done.

use l2_util::ObjectId;
use l2_world::script::QuestSound;
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const SUSPICIOUS_LOOKING_PILE_OF_STONES: i32 = 32046;
const WENDY: i32 = 32047;
const YUMI: i32 = 32041;
const WEATHERMASTER_1: i32 = 32042;
const WEATHERMASTER_2: i32 = 32043;
const WEATHERMASTER_3: i32 = 32044;
const DOCTOR_CHAOS_SECRET_BOOKSHELF: i32 = 32045;

// Items
const FLOWER_OF_PAVEL: i32 = 8290;
const HEART_OF_ATLANTA: i32 = 8291;
const WENDYS_NECKLACE: i32 = 8292;
const LOCKUP_RESEARCH_REPORT: i32 = 8058;
const RESEARCH_REPORT: i32 = 8059;
const KEY_OF_ENIGMA: i32 = 8060;

// Skills
const QUEST_TRAP_POWER_SHOT: (i32, i32) = (5073, 5);
const NPC_DEFAULT: (i32, i32) = (7000, 1);

// Rewards
const SEALED_PHOENIX_EARRING: i32 = 6324;

/// Java `Q00114_ResurrectionOfAnOldManager.class.getSimpleName()`.
const RESURRECTION_OF_AN_OLD_MANAGER: &str = "Q00114_ResurrectionOfAnOldManager";

/// Java `checkQ114(player)`.
fn check_q114(ctx: &ScriptCtx<'_>, player: ObjectId) -> bool {
    ctx.player_quest_state(player, RESURRECTION_OF_AN_OLD_MANAGER)
        .is_some_and(|q114| q114.is_completed())
}

/// `if (condition) { html = text; }`
fn shown(condition: bool, text: &str) -> Option<String> {
    condition.then(|| text.to_owned())
}

/// Java `Q00120_PavelsLastResearch`.
pub struct Q00120PavelsLastResearch;

impl Script for Q00120PavelsLastResearch {
    fn name(&self) -> &'static str {
        "Q00120_PavelsLastResearch"
    }

    fn path(&self) -> &'static str {
        "quests/Q00120_PavelsLastResearch"
    }

    fn id(&self) -> i32 {
        120
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[SUSPICIOUS_LOOKING_PILE_OF_STONES]);
        s.add_talk_id(&[
            SUSPICIOUS_LOOKING_PILE_OF_STONES,
            WENDY,
            YUMI,
            WEATHERMASTER_1,
            WEATHERMASTER_2,
            WEATHERMASTER_3,
            DOCTOR_CHAOS_SECRET_BOOKSHELF,
        ]);
        s.add_skill_see_id(&[WEATHERMASTER_1, WEATHERMASTER_2, WEATHERMASTER_3]);
        s.register_quest_items(&[
            FLOWER_OF_PAVEL,
            HEART_OF_ATLANTA,
            WENDYS_NECKLACE,
            LOCKUP_RESEARCH_REPORT,
            RESEARCH_REPORT,
            KEY_OF_ENIGMA,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut html = None;
        match event {
            "32046-03.html" | "32046-04.htm" | "32046-05.html" | "32046-06.html" => {
                html = shown(qs.is_created(ctx), event);
            }
            "quest_accept" => {
                if qs.is_created(ctx) && check_q114(ctx, player) {
                    if ctx.player_level(player) >= 70 {
                        qs.start_quest(ctx);
                        qs.set_memo_state(ctx, 1);
                        html = Some("32046-08.htm".to_owned());
                    } else {
                        html = Some("32046-07.htm".to_owned());
                    }
                }
            }
            "32046-10.html" => html = shown(qs.is_memo_state(ctx, 1), event),
            "32046-11.html" => {
                if qs.is_memo_state(ctx, 1) {
                    qs.set_memo_state(ctx, 2);
                    qs.set_cond(ctx, 2, true);
                    html = Some(event.to_owned());
                }
            }
            "32046-14.html" => html = shown(qs.is_memo_state(ctx, 3), event),
            "32046-15.html" => {
                if qs.is_memo_state(ctx, 3) {
                    ctx.give_items(player, FLOWER_OF_PAVEL, 1);
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 6, true);
                    html = Some(event.to_owned());
                }
            }
            "32046-18.html" | "32046-19.html" | "32046-20.html" | "32046-21.html" | "32046-22.html"
            | "32046-23.html" | "32046-24.html" => html = shown(qs.is_memo_state(ctx, 7), event),
            "32046-25.html" => {
                if qs.is_memo_state(ctx, 7) {
                    qs.set_memo_state(ctx, 8);
                    qs.set_cond(ctx, 10, true);
                    html = Some(event.to_owned());
                }
            }
            "32046-26.html" | "32046-27.html" | "32046-28.html" => {
                html = shown(qs.is_memo_state(ctx, 8), event);
            }
            "32046-30.html" | "32046-31.html" | "32046-32.html" | "32046-33.html" | "32046-34.html" => {
                html = shown(qs.is_memo_state(ctx, 11), event);
            }
            "32046-35.html" => {
                if qs.is_memo_state(ctx, 11) {
                    qs.set_memo_state(ctx, 12);
                    qs.set_cond(ctx, 13, true);
                    html = Some(event.to_owned());
                }
            }
            "32046-38.html" | "32046-39.html" | "32046-40.html" => {
                html = shown(qs.is_memo_state(ctx, 19), event);
            }
            "32046-41.html" => {
                if qs.is_memo_state(ctx, 19) {
                    qs.set_memo_state(ctx, 20);
                    qs.set_cond(ctx, 20, true);
                    html = Some(event.to_owned());
                }
            }
            "32046-44.html" => {
                if qs.is_memo_state(ctx, 22) {
                    ctx.give_items(player, HEART_OF_ATLANTA, 1);
                    qs.set_memo_state(ctx, 23);
                    qs.set_cond(ctx, 23, true);
                    html = Some(event.to_owned());
                }
            }
            "32047-02.html" | "32047-03.html" | "32047-04.html" | "32047-05.html" => {
                html = shown(qs.is_memo_state(ctx, 2), event);
            }
            "32047-06.html" => {
                if qs.is_memo_state(ctx, 2) {
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 5, true);
                    html = Some(event.to_owned());
                }
            }
            "32047-09.html" => {
                html = shown(
                    qs.is_memo_state(ctx, 4) && ctx.has_quest_items(player, FLOWER_OF_PAVEL),
                    event,
                );
            }
            "32047-10.html" => {
                if qs.is_memo_state(ctx, 4) && ctx.has_quest_items(player, FLOWER_OF_PAVEL) {
                    ctx.take_items(player, FLOWER_OF_PAVEL, -1);
                    qs.set_memo_state(ctx, 5);
                    qs.set_cond(ctx, 7, true);
                    html = Some(event.to_owned());
                }
            }
            "32047-13.html" | "32047-14.html" => html = shown(qs.is_memo_state(ctx, 6), event),
            "32047-15.html" => {
                if qs.is_memo_state(ctx, 6) {
                    qs.set_memo_state(ctx, 7);
                    qs.set_cond(ctx, 9, true);
                    html = Some(event.to_owned());
                }
            }
            "32047-18.html" => html = shown(qs.is_memo_state(ctx, 12), event),
            "32047-19.html" => {
                if qs.is_memo_state(ctx, 12) {
                    qs.set_memo_state(ctx, 13);
                    qs.set_cond(ctx, 14, true);
                    html = Some(event.to_owned());
                }
            }
            "32047-23.html" | "32047-24.html" | "32047-25.html" | "32047-26.html" => {
                html = shown(
                    qs.is_memo_state(ctx, 23) && ctx.has_quest_items(player, HEART_OF_ATLANTA),
                    event,
                );
            }
            "32047-27.html" => {
                if qs.is_memo_state(ctx, 23) && ctx.has_quest_items(player, HEART_OF_ATLANTA) {
                    ctx.take_items(player, HEART_OF_ATLANTA, -1);
                    qs.set_memo_state(ctx, 24);
                    qs.set_cond(ctx, 24, true);
                    html = Some(event.to_owned());
                }
            }
            "32047-28.html" | "32047-29.html" => html = shown(qs.is_memo_state(ctx, 24), event),
            "32047-30.html" => {
                if qs.is_memo_state(ctx, 24) {
                    qs.set_memo_state(ctx, 25);
                    html = Some(event.to_owned());
                }
            }
            "32047-31.html" | "32047-32.html" => html = shown(qs.is_memo_state(ctx, 25), event),
            "32047-33.html" => {
                if qs.is_memo_state(ctx, 25) {
                    ctx.give_items(player, WENDYS_NECKLACE, 1);
                    qs.set_memo_state(ctx, 26);
                    qs.set_cond(ctx, 25, true);
                    html = Some(event.to_owned());
                }
            }
            "32041-02.html" => html = shown(qs.is_memo_state(ctx, 2), event),
            "32041-03.html" => {
                if qs.is_memo_state(ctx, 2) && qs.is_memo_state_ex(ctx, 0, 0) {
                    qs.set_memo_state_ex(ctx, 0, 1);
                    qs.set_cond(ctx, 3, true);
                    html = Some(event.to_owned());
                }
            }
            "32041-05.html" => {
                if qs.is_memo_state(ctx, 2) && qs.is_memo_state_ex(ctx, 0, 0) {
                    qs.set_memo_state_ex(ctx, 0, 2);
                    qs.set_cond(ctx, 4, true);
                    html = Some(event.to_owned());
                }
            }
            "32041-09.html" | "32041-10.html" | "32041-11.html" | "32041-12.html" => {
                html = shown(qs.is_memo_state(ctx, 5), event);
            }
            "32041-13.html" => {
                if qs.is_memo_state(ctx, 5) {
                    qs.set_memo_state(ctx, 6);
                    qs.set_cond(ctx, 8, true);
                    html = Some(event.to_owned());
                }
            }
            "32041-16.html" => {
                html = shown(
                    qs.is_memo_state(ctx, 14) && ctx.has_quest_items(player, LOCKUP_RESEARCH_REPORT),
                    event,
                );
            }
            "32041-17.html" => {
                if qs.is_memo_state(ctx, 14) && ctx.has_quest_items(player, LOCKUP_RESEARCH_REPORT) {
                    ctx.give_items(player, KEY_OF_ENIGMA, 1);
                    qs.set_memo_state(ctx, 15);
                    qs.set_cond(ctx, 16, true);
                    html = Some(event.to_owned());
                }
            }
            "32041-20.html" => {
                html = shown(
                    qs.is_memo_state(ctx, 15) && ctx.has_all_quest_items(player, &[RESEARCH_REPORT, KEY_OF_ENIGMA]),
                    event,
                );
            }
            "pavel" | "e=mc2" => {
                html = shown(
                    qs.is_memo_state(ctx, 15) && ctx.has_all_quest_items(player, &[RESEARCH_REPORT, KEY_OF_ENIGMA]),
                    "32041-21.html",
                );
            }
            "wdl" => {
                html = shown(
                    qs.is_memo_state(ctx, 15) && ctx.has_all_quest_items(player, &[RESEARCH_REPORT, KEY_OF_ENIGMA]),
                    "32041-22.html",
                );
            }
            "32041-23.html" => {
                if qs.is_memo_state(ctx, 15) && ctx.has_all_quest_items(player, &[RESEARCH_REPORT, KEY_OF_ENIGMA]) {
                    ctx.take_items(player, KEY_OF_ENIGMA, -1);
                    qs.set_memo_state(ctx, 16);
                    qs.set_cond(ctx, 17, true);
                    html = Some(event.to_owned());
                }
            }
            "32041-24.html" | "32041-26.html" => {
                html = shown(
                    qs.is_memo_state(ctx, 16) && ctx.has_quest_items(player, RESEARCH_REPORT),
                    event,
                );
            }
            "32041-29.html" | "32041-30.html" | "32041-31.html" | "32041-32.html" | "32041-33.html" => {
                html = shown(
                    qs.is_memo_state(ctx, 26) && ctx.has_quest_items(player, WENDYS_NECKLACE),
                    event,
                );
            }
            "32041-34.html" => {
                if qs.is_memo_state(ctx, 26) && ctx.has_quest_items(player, WENDYS_NECKLACE) {
                    ctx.take_items(player, WENDYS_NECKLACE, -1);
                    ctx.reward_items(player, SEALED_PHOENIX_EARRING, 1);
                    ctx.give_adena(player, 783_720, true);
                    ctx.add_exp_and_sp(player, 3_447_315, 272_615);
                    qs.exit_quest(ctx, false, true);
                    html = Some(event.to_owned());
                }
            }
            // The first weather master: a puzzle of three words, `memoStateEx(0)` holds one digit
            // per solved word (11 after the first two, 10101 when the second part is done).
            "32042-02.html" => {
                if qs.is_memo_state(ctx, 8) {
                    qs.set_memo_state_ex(ctx, 0, 0);
                    html = Some(event.to_owned());
                }
            }
            "wm1_1_b" | "wm1_1_c" | "wm1_1_d" | "wm1_1_l" | "wm1_1_m" | "wm1_1_n" | "wm1_1_s" | "wm1_1_t"
            | "wm1_1_u" => html = shown(qs.is_memo_state(ctx, 8), "32042-03.html"),
            "wm1_1_a" => {
                if qs.is_memo_state(ctx, 8) {
                    qs.set_memo_state_ex(ctx, 0, 1);
                    html = Some("32042-03.html".to_owned());
                }
            }
            "wm1_2_a" | "wm1_2_b" | "wm1_2_c" | "wm1_2_d" | "wm1_2_l" | "wm1_2_m" | "wm1_2_n" | "wm1_2_s"
            | "wm1_2_u" => html = shown(qs.is_memo_state(ctx, 8), "32042-04.html"),
            "wm1_2_t" => {
                if qs.is_memo_state(ctx, 8) {
                    let ex = qs.get_memo_state_ex(ctx, 0);
                    qs.set_memo_state_ex(ctx, 0, 10 + (ex % 10));
                    html = Some("32042-04.html".to_owned());
                }
            }
            "wm1_3_a" | "wm1_3_b" | "wm1_3_c" | "wm1_3_d" | "wm1_3_m" | "wm1_3_n" | "wm1_3_s" | "wm1_3_t"
            | "wm1_3_u" => html = shown(qs.is_memo_state(ctx, 8), "32042-05.html"),
            "wm1_3_l" => {
                if qs.is_memo_state(ctx, 8) {
                    if qs.is_memo_state_ex(ctx, 0, 11) {
                        qs.set_memo_state(ctx, 9);
                        qs.set_cond(ctx, 11, true);
                        qs.set_memo_state_ex(ctx, 0, 0);
                        html = Some("32042-06.html".to_owned());
                    } else {
                        html = Some("32042-05.html".to_owned());
                    }
                }
            }
            "32042-15.html" | "32042-06.html" | "32042-07.html" => {
                html = shown(qs.is_memo_state(ctx, 9), event);
            }
            "32042-08.html" => {
                if qs.is_memo_state(ctx, 9) {
                    qs.set_memo_state(ctx, 10);
                    ctx.play_sound(player, QuestSound::AmbsoundPercussion01);
                    html = Some(event.to_owned());
                }
            }
            "wm1_return" => {
                if qs.is_memo_state(ctx, 10) {
                    html = Some(if qs.is_memo_state_ex(ctx, 0, 10101) { "32042-13.html" } else { "32042-09.html" }.to_owned());
                }
            }
            "32042-10.html" => {
                if qs.is_memo_state(ctx, 10) {
                    let ex = qs.get_memo_state_ex(ctx, 0);
                    qs.set_memo_state_ex(ctx, 0, ((ex / 10) * 10) + 1);
                    html = Some(event.to_owned());
                }
            }
            "32042-11.html" => {
                if qs.is_memo_state(ctx, 10) {
                    let memo_state_ex = qs.get_memo_state_ex(ctx, 0);
                    let i1 = (memo_state_ex / 1000) * 1000;
                    let i2 = (memo_state_ex % 100) + 100;
                    qs.set_memo_state_ex(ctx, 0, i1 + i2);
                    html = Some(event.to_owned());
                }
            }
            "32042-12.html" => {
                if qs.is_memo_state(ctx, 10) {
                    let ex = qs.get_memo_state_ex(ctx, 0);
                    qs.set_memo_state_ex(ctx, 0, 10_000 + (ex % 10_000));
                    html = Some(event.to_owned());
                }
            }
            "32042-14.html" => {
                if qs.is_memo_state(ctx, 10) && qs.is_memo_state_ex(ctx, 0, 10101) {
                    qs.set_memo_state(ctx, 11);
                    qs.set_cond(ctx, 12, true);
                    qs.set_memo_state_ex(ctx, 0, 0);
                    html = Some(event.to_owned());
                }
            }
            // The second weather master.
            "32043-02.html" => {
                if qs.is_memo_state(ctx, 16) {
                    qs.set_memo_state_ex(ctx, 0, 0);
                    html = Some(event.to_owned());
                }
            }
            "wm2_1_a" | "wm2_1_b" | "wm2_1_c" | "wm2_1_d" | "wm2_1_l" | "wm2_1_m" | "wm2_1_n" | "wm2_1_v"
            | "wm2_1_x" => html = shown(qs.is_memo_state(ctx, 16), "32043-03.html"),
            "wm2_1_w" => {
                if qs.is_memo_state(ctx, 16) {
                    qs.set_memo_state_ex(ctx, 0, 1);
                    html = Some("32043-03.html".to_owned());
                }
            }
            "wm2_2_a" | "wm2_2_b" | "wm2_2_c" | "wm2_2_l" | "wm2_2_m" | "wm2_2_n" | "wm2_2_v" | "wm2_2_w"
            | "wm2_2_x" => html = shown(qs.is_memo_state(ctx, 16), "32043-04.html"),
            "wm2_2_d" => {
                if qs.is_memo_state(ctx, 16) {
                    let ex = qs.get_memo_state_ex(ctx, 0);
                    qs.set_memo_state_ex(ctx, 0, 10 + (ex % 10));
                    html = Some("32043-04.html".to_owned());
                }
            }
            "wm2_3_a" | "wm2_3_b" | "wm2_3_c" | "wm2_3_d" | "wm2_3_m" | "wm2_3_n" | "wm2_3_v" | "wm2_3_w"
            | "wm2_3_x" => {
                // NOTE(java-quirk): this one tests memo state 8, the others of the puzzle 16.
                html = shown(qs.is_memo_state(ctx, 8), "32043-05.html");
            }
            "wm2_3_l" => {
                if qs.is_memo_state(ctx, 16) {
                    if qs.is_memo_state_ex(ctx, 0, 11) {
                        qs.set_memo_state(ctx, 17);
                        qs.set_cond(ctx, 18, true);
                        qs.set_memo_state_ex(ctx, 0, 0);
                        html = Some("32043-06.html".to_owned());
                    } else {
                        html = Some("32043-05.html".to_owned());
                    }
                }
            }
            "32043-31.html" | "32043-30.html" | "32043-29.html" | "32043-28.html" | "32043-06.html"
            | "32043-07.html" | "32043-08.html" => html = shown(qs.is_memo_state(ctx, 17), event),
            "32043-09.html" => {
                if qs.is_memo_state(ctx, 17) {
                    qs.set_memo_state(ctx, 18);
                    html = Some(event.to_owned());
                }
            }
            "32043-10.html" | "wm2_return" => {
                if qs.is_memo_state(ctx, 18) {
                    html = Some(if qs.is_memo_state_ex(ctx, 0, 1111) { "32043-12.html" } else { "32043-11.html" }.to_owned());
                }
            }
            "32043-13.html" => {
                if qs.is_memo_state(ctx, 18) {
                    let ex = qs.get_memo_state_ex(ctx, 0);
                    qs.set_memo_state_ex(ctx, 0, ((ex / 10) * 10) + 1);
                    html = Some(event.to_owned());
                }
            }
            "32043-14.html" => html = shown(qs.is_memo_state(ctx, 18), event),
            "wm2_output" => {
                if qs.is_memo_state(ctx, 18) {
                    html = Some(if qs.get_memo_state_ex(ctx, 0) < 1000 { "32043-15.html" } else { "32043-18.html" }.to_owned());
                }
            }
            "32043-16.html" => html = shown(qs.is_memo_state(ctx, 18), event),
            "32043-17.html" => {
                if qs.is_memo_state(ctx, 18) {
                    let memo_state_ex = qs.get_memo_state_ex(ctx, 0);
                    let i1 = (memo_state_ex / 10_000) * 10_000;
                    let i2 = (memo_state_ex % 1000) + 1000;
                    qs.set_memo_state_ex(ctx, 0, i1 + i2);
                    ctx.play_sound(player, QuestSound::AmbsoundDrone);
                    html = Some(event.to_owned());
                }
            }
            "32043-19.html" | "32043-20.html" => html = shown(qs.is_memo_state(ctx, 18), event),
            "32043-21.html" => {
                if qs.is_memo_state(ctx, 18) {
                    let memo_state_ex = qs.get_memo_state_ex(ctx, 0);
                    let i1 = (memo_state_ex / 100) * 100;
                    let i2 = (memo_state_ex % 10) + 10;
                    qs.set_memo_state_ex(ctx, 0, i1 + i2);
                    html = Some(event.to_owned());
                }
            }
            "32043-22.html" => {
                if qs.is_memo_state(ctx, 18) && qs.is_memo_state_ex(ctx, 0, 1111) {
                    qs.set_memo_state(ctx, 19);
                    qs.set_cond(ctx, 19, true);
                    qs.set_memo_state_ex(ctx, 0, 0);
                    html = Some(event.to_owned());
                }
            }
            "32043-24.html" | "32043-25.html" => html = shown(qs.is_memo_state(ctx, 18), event),
            "32043-26.html" => {
                if qs.is_memo_state(ctx, 18) {
                    let memo_state_ex = qs.get_memo_state_ex(ctx, 0);
                    let i1 = (memo_state_ex / 1000) * 1000;
                    let i2 = (memo_state_ex % 100) + 100;
                    qs.set_memo_state_ex(ctx, 0, i1 + i2);
                    html = Some(event.to_owned());
                }
            }
            "32043-27.html" => html = shown(qs.is_memo_state(ctx, 18), event),
            // The third weather master.
            "32044-02.html" => {
                if qs.is_memo_state(ctx, 20) {
                    qs.set_memo_state_ex(ctx, 0, 0);
                    html = Some(event.to_owned());
                }
            }
            "wm3_1_a" | "wm3_1_b" | "wm3_1_c" | "wm3_1_d" | "wm3_1_l" | "wm3_1_m" | "wm3_1_v" | "wm3_1_w"
            | "wm3_1_x" => html = shown(qs.is_memo_state(ctx, 20), "32044-03.html"),
            "wm3_1_n" => {
                if qs.is_memo_state(ctx, 20) {
                    qs.set_memo_state_ex(ctx, 0, 1);
                    html = Some("32044-03.html".to_owned());
                }
            }
            "wm3_2_1" | "wm3_2_2" | "wm3_2_3" | "wm3_2_5" | "wm3_2_6" | "wm3_2_7" | "wm3_2_8" | "wm3_2_9"
            | "wm3_2_10" => html = shown(qs.is_memo_state(ctx, 20), "32044-04.html"),
            "wm3_2_4" => {
                if qs.is_memo_state(ctx, 20) {
                    let ex = qs.get_memo_state_ex(ctx, 0);
                    qs.set_memo_state_ex(ctx, 0, 10 + (ex % 10));
                    html = Some("32044-04.html".to_owned());
                }
            }
            "wm3_3_1" | "wm3_3_2" | "wm3_3_3" | "wm3_3_4" | "wm3_3_6" | "wm3_3_7" | "wm3_3_8" | "wm3_3_9"
            | "wm3_3_10" => html = shown(qs.is_memo_state(ctx, 20), "32044-05.html"),
            "wm3_3_5" => {
                if qs.is_memo_state(ctx, 20) {
                    if qs.is_memo_state_ex(ctx, 0, 11) {
                        qs.set_memo_state(ctx, 21);
                        qs.set_cond(ctx, 21, true);
                        qs.set_memo_state_ex(ctx, 0, 0);
                        ctx.play_sound(player, QuestSound::AmbsoundPercussion02);
                        html = Some("32044-06.html".to_owned());
                    } else {
                        html = Some("32044-05.html".to_owned());
                    }
                }
            }
            "32044-07.html" => html = shown(qs.is_memo_state(ctx, 21), event),
            "wm3_observe" => {
                if qs.is_memo_state(ctx, 21) {
                    html = Some(if qs.get_memo_state_ex(ctx, 0) % 100 == 11 { "32044-10.html" } else { "32044-09.html" }.to_owned());
                }
            }
            "32044-11.html" => {
                if qs.is_memo_state(ctx, 21) {
                    let memo_state_ex = qs.get_memo_state_ex(ctx, 0);
                    let i1 = (memo_state_ex / 100) * 100;
                    let i2 = (memo_state_ex % 10) + 10;
                    qs.set_memo_state_ex(ctx, 0, i1 + i2);
                    html = Some(event.to_owned());
                }
            }
            "wm3_fire_of_paagrio" => {
                if qs.is_memo_state(ctx, 21) {
                    let ex = qs.get_memo_state_ex(ctx, 0);
                    if ex / 100 == 1 {
                        html = Some("32044-13.html".to_owned());
                    } else {
                        qs.set_memo_state_ex(ctx, 0, ((ex / 10) * 10) + 1);
                        html = Some("32044-12.html".to_owned());
                    }
                }
            }
            "wm3_control" => {
                if qs.is_memo_state(ctx, 21) {
                    html = Some(if qs.get_memo_state_ex(ctx, 0) / 100 == 1 { "32044-15.html" } else { "32044-14.html" }.to_owned());
                }
            }
            "32044-16.html" => {
                if qs.is_memo_state(ctx, 21) && qs.get_memo_state_ex(ctx, 0) / 100 != 1 {
                    let ex = qs.get_memo_state_ex(ctx, 0);
                    qs.set_memo_state_ex(ctx, 0, (ex % 100) + 100);
                    html = Some(event.to_owned());
                }
            }
            "32044-17.html" | "32044-18.html" | "32044-19.html" => {
                html = shown(qs.is_memo_state(ctx, 21), event);
            }
            "32044-20.html" => {
                if qs.is_memo_state(ctx, 21) && qs.get_memo_state_ex(ctx, 0) / 100 == 1 {
                    qs.set_memo_state(ctx, 22);
                    qs.set_cond(ctx, 22, true);
                    qs.set_memo_state_ex(ctx, 0, 0);
                    ctx.play_sound(player, QuestSound::AmbsoundDrone);
                    // NOTE(java-quirk): `npc.setTarget(player)` throws without an NPC.
                    let npc = npc?;
                    ctx.cast_skill(npc, player, SkillKey::new(QUEST_TRAP_POWER_SHOT.0, QUEST_TRAP_POWER_SHOT.1));
                    html = Some(event.to_owned());
                }
            }
            "32044-21.html" => html = shown(qs.is_memo_state(ctx, 22), event),
            "32045-02.html" => {
                if qs.is_memo_state(ctx, 13) {
                    ctx.give_items(player, LOCKUP_RESEARCH_REPORT, 1);
                    // The locked report is exchanged for the unlocked one with the key of enigma
                    // Wendy gives.
                    qs.set_memo_state(ctx, 14);
                    qs.set_cond(ctx, 15, true);
                    let npc = npc?;
                    ctx.cast_skill(npc, player, SkillKey::new(QUEST_TRAP_POWER_SHOT.0, QUEST_TRAP_POWER_SHOT.1));
                    html = Some(event.to_owned());
                }
            }
            _ => {}
        }
        html
    }

    fn on_skill_see(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        caster: ObjectId,
        _skill: SkillKey,
        _targets: &[ObjectId],
        _is_summon: bool,
    ) {
        let Some(qs) = ctx.get_quest_state(caster, false) else {
            return;
        };
        if qs.is_started(ctx) {
            let npc_default = SkillKey::new(NPC_DEFAULT.0, NPC_DEFAULT.1);
            ctx.cast_skill(npc, caster, npc_default);
            ctx.cast_skill(npc, caster, npc_default);
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut html = ctx.get_no_quest_msg(player);
        match npc.id {
            SUSPICIOUS_LOOKING_PILE_OF_STONES => {
                if qs.is_created(ctx) {
                    html = if check_q114(ctx, player) { "32046-01.htm" } else { "32046-02.htm" }.to_owned();
                } else if qs.is_started(ctx) {
                    match qs.get_memo_state(ctx) {
                        1 => html = "32046-09.html".to_owned(),
                        2 => html = "32046-12.html".to_owned(),
                        3 => html = "32046-13.html".to_owned(),
                        4 => {
                            if ctx.has_quest_items(player, FLOWER_OF_PAVEL) {
                                html = "32046-16.html".to_owned();
                            }
                        }
                        7 => html = "32046-17.html".to_owned(),
                        8 => html = "32046-28.html".to_owned(),
                        11 => html = "32046-29.html".to_owned(),
                        12 => html = "32046-36.html".to_owned(),
                        19 => html = "32046-37.html".to_owned(),
                        20 => html = "32046-42.html".to_owned(),
                        22 => html = "32046-43.html".to_owned(),
                        23 => {
                            if ctx.has_quest_items(player, HEART_OF_ATLANTA) {
                                html = "32046-45.html".to_owned();
                            }
                        }
                        _ => {}
                    }
                } else if check_q114(ctx, player) {
                    html = ctx.get_already_completed_msg(player);
                }
            }
            WENDY => match qs.get_memo_state(ctx) {
                2 => html = "32047-01.html".to_owned(),
                3 => html = "32047-07.html".to_owned(),
                4 => {
                    if ctx.has_quest_items(player, FLOWER_OF_PAVEL) {
                        html = "32047-08.html".to_owned();
                    }
                }
                5 => html = "32047-11.html".to_owned(),
                6 => html = "32047-12.html".to_owned(),
                7 => html = "32047-16.html".to_owned(),
                12 => html = "32047-17.html".to_owned(),
                13 => html = "32047-20.html".to_owned(),
                14 => {
                    if ctx.has_quest_items(player, LOCKUP_RESEARCH_REPORT) {
                        html = "32047-21.html".to_owned();
                    }
                }
                23 => {
                    if ctx.has_quest_items(player, HEART_OF_ATLANTA) {
                        html = "32047-22.html".to_owned();
                    }
                }
                24 => html = "32047-27.html".to_owned(),
                25 => html = "32047-30.html".to_owned(),
                26 => {
                    if ctx.has_quest_items(player, WENDYS_NECKLACE) {
                        html = "32047-34.html".to_owned();
                    }
                }
                _ => {}
            },
            YUMI => match qs.get_memo_state(ctx) {
                2 => match qs.get_memo_state_ex(ctx, 0) {
                    0 => html = "32041-01.html".to_owned(),
                    1 => html = "32041-04.html".to_owned(),
                    2 => html = "32041-06.html".to_owned(),
                    _ => {}
                },
                5 => {
                    html = if qs.get_memo_state_ex(ctx, 0) > 0 { "32041-07.html" } else { "32041-08.html" }.to_owned();
                }
                6 => html = "32041-14.html".to_owned(),
                14 => {
                    if ctx.has_quest_items(player, LOCKUP_RESEARCH_REPORT) {
                        html = "32041-15.html".to_owned();
                    }
                }
                15 => {
                    if ctx.has_quest_items(player, KEY_OF_ENIGMA) {
                        if ctx.has_quest_items(player, RESEARCH_REPORT) {
                            html = "32041-19.html".to_owned();
                        } else if ctx.has_quest_items(player, LOCKUP_RESEARCH_REPORT) {
                            html = "32041-18.html".to_owned();
                        }
                    }
                }
                16 => {
                    if ctx.has_quest_items(player, RESEARCH_REPORT) {
                        html = "32041-27.html".to_owned();
                    }
                }
                26 => {
                    if ctx.has_quest_items(player, WENDYS_NECKLACE) {
                        html = "32041-28.html".to_owned();
                    }
                }
                _ => {}
            },
            WEATHERMASTER_1 => match qs.get_memo_state(ctx) {
                8 => {
                    html = "32042-01.html".to_owned();
                    ctx.play_sound(player, QuestSound::AmbsoundCrystalLoop);
                }
                9 => html = "32042-06.html".to_owned(),
                10 => {
                    html = if qs.is_memo_state_ex(ctx, 0, 10101) { "32042-13.html" } else { "32042-09.html" }.to_owned();
                }
                11 => html = "32042-14.html".to_owned(),
                _ => {}
            },
            WEATHERMASTER_2 => match qs.get_memo_state(ctx) {
                16 => html = "32043-01.html".to_owned(),
                17 => html = "32043-06.html".to_owned(),
                18 => html = "32043-09.html".to_owned(),
                19 => html = "32043-23.html".to_owned(),
                _ => {}
            },
            WEATHERMASTER_3 => match qs.get_memo_state(ctx) {
                20 => html = "32044-01.html".to_owned(),
                21 => html = "32044-08.html".to_owned(),
                22 => html = "32044-22.html".to_owned(),
                _ => {}
            },
            DOCTOR_CHAOS_SECRET_BOOKSHELF => match qs.get_memo_state(ctx) {
                13 => html = "32045-01.html".to_owned(),
                14 => html = "32045-03.html".to_owned(),
                _ => {}
            },
            _ => {}
        }
        Some(html)
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{COMPLETED, STARTED};

    use super::*;
    use crate::testing::{Pages, ScriptTest, rows};

    const NAME: &str = "Q00120_PavelsLastResearch";

    const NPCS: &str = r#"
        <npc id="32041" type="Folk" level="70" name="Yumi"/>
        <npc id="32042" type="Folk" level="70" name="Weathermaster 1"/>
        <npc id="32046" type="Folk" level="70" name="Stones"/>
        <npc id="32047" type="Folk" level="70" name="Wendy"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="6324" type="EtcItem" name="Sealed Phoenix Earring"/>
        <item id="8290" type="EtcItem" name="Flower of Pavel"><set name="is_questitem" val="true"/></item>
        <item id="8292" type="EtcItem" name="Wendy's Necklace"><set name="is_questitem" val="true"/></item>"#;

    /// Java `Q00114_ResurrectionOfAnOldManager` as far as the stone pile reads it.
    struct Q114;

    impl Script for Q114 {
        fn name(&self) -> &'static str {
            RESURRECTION_OF_AN_OLD_MANAGER
        }
        fn path(&self) -> &'static str {
            "quests/Q00114_ResurrectionOfAnOldManager"
        }
        fn id(&self) -> i32 {
            114
        }
        fn setup(&self, _s: &mut ScriptSetup) {}
    }

    fn setup() -> (ScriptTest, Oid, Oid, Oid, Oid) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00120PavelsLastResearch);
        registry.add(Q114);
        let pages = Pages::of(&Q00120PavelsLastResearch)
            .page("32046-01.htm", &["quest_accept"])
            .plain(&["32046-02.htm", "32046-07.htm", "32046-08.htm", "32046-09.html", "32041-28.html"])
            .page("32042-01.html", &["32042-02.html"])
            .page("32042-02.html", &["wm1_1_a"])
            .page("32042-03.html", &["wm1_2_t"])
            .page("32042-04.html", &["wm1_3_l"])
            .plain(&["32042-05.html", "32042-06.html"])
            .npcs(&[SUSPICIOUS_LOOKING_PILE_OF_STONES, WENDY, YUMI, WEATHERMASTER_1]);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages);
        let stones = t.spawn(SUSPICIOUS_LOOKING_PILE_OF_STONES, 20);
        let wendy = t.spawn(WENDY, 40);
        let yumi = t.spawn(YUMI, 60);
        let wm1 = t.spawn(WEATHERMASTER_1, 80);
        (t, stones, wendy, yumi, wm1)
    }

    fn q114_done(t: &mut ScriptTest) {
        t.world.run_as_script(RESURRECTION_OF_AN_OLD_MANAGER, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.set_state(ctx, COMPLETED);
        });
        t.take_writes();
    }

    fn ev(t: &mut ScriptTest, event: &str) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            out = Q00120PavelsLastResearch.on_event(ctx, event, None, Some(Oid(1)));
        });
        out
    }

    fn talk(t: &mut ScriptTest, npc: Oid) -> Option<String> {
        let mut out = None;
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(npc) {
                out = Q00120PavelsLastResearch.on_talk(ctx, npc, Oid(1));
            }
        });
        out
    }

    fn memo(t: &mut ScriptTest, value: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, value);
            }
        });
        t.take_writes();
    }

    #[test]
    fn it_needs_quest_114_and_level_70() {
        let (mut t, stones, ..) = setup();
        t.enter(1, 70, vec![]);
        assert!(t.quest_page(1, stones).contains("32046-02.htm"));

        let (mut t, stones, ..) = setup();
        t.enter(1, 69, vec![]);
        q114_done(&mut t);
        assert!(t.quest_page(1, stones).contains("32046-01.htm"));
        assert!(t.event(1, NAME, "quest_accept").contains("32046-07.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_start_and_the_first_steps_write_the_rows() {
        let (mut t, stones, wendy, ..) = setup();
        t.enter(1, 70, vec![]);
        q114_done(&mut t);
        t.quest_page(1, stones);
        assert!(t.event(1, NAME, "quest_accept").contains("32046-08.htm"));
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
        assert_eq!(talk(&mut t, stones).as_deref(), Some("32046-09.html"));
        assert_eq!(ev(&mut t, "32046-11.html").as_deref(), Some("32046-11.html"));
        assert_eq!(ev(&mut t, "32046-14.html"), None);
        assert_eq!(talk(&mut t, wendy).as_deref(), Some("32047-01.html"));
        assert_eq!(ev(&mut t, "32047-06.html").as_deref(), Some("32047-06.html"));
        let w = t.take_writes();
        assert!(w.contains(&rows::update(NAME, "memoState", "3")));
        assert!(w.contains(&rows::update(NAME, "cond", "5")));
    }

    #[test]
    fn the_first_weather_master_puzzle_needs_the_right_letters() {
        let (mut t, _, _, _, wm1) = setup();
        t.enter(1, 70, vec![]);
        q114_done(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
        });
        memo(&mut t, 8);
        assert_eq!(talk(&mut t, wm1).as_deref(), Some("32042-01.html"));
        assert_eq!(ev(&mut t, "32042-02.html").as_deref(), Some("32042-02.html"));
        // wrong letter first: nothing is remembered and the last page is not the winning one
        assert_eq!(ev(&mut t, "wm1_3_l").as_deref(), Some("32042-05.html"));
        assert_eq!(ev(&mut t, "wm1_1_a").as_deref(), Some("32042-03.html"));
        assert_eq!(ev(&mut t, "wm1_2_t").as_deref(), Some("32042-04.html"));
        assert_eq!(ev(&mut t, "wm1_3_l").as_deref(), Some("32042-06.html"));
        let w = t.take_writes();
        assert!(w.contains(&rows::update(NAME, "memoState", "9")));
        assert!(w.contains(&rows::update(NAME, "cond", "11")));
    }

    #[test]
    fn the_necklace_pays_the_earring_and_the_exp() {
        let (mut t, _, _, yumi, _) = setup();
        t.enter(1, 70, vec![]);
        q114_done(&mut t);
        t.world.run_as_script(NAME, |ctx| {
            let qs = ctx.get_quest_state(Oid(1), true).unwrap();
            qs.start_quest(ctx);
            assert_eq!(qs.state(ctx), STARTED);
            ctx.give_items(Oid(1), WENDYS_NECKLACE, 1);
        });
        memo(&mut t, 26);
        assert_eq!(talk(&mut t, yumi).as_deref(), Some("32041-28.html"));
        assert_eq!(ev(&mut t, "32041-34.html").as_deref(), Some("32041-34.html"));
        assert_eq!(t.count(1, SEALED_PHOENIX_EARRING), 1);
        assert_eq!(t.count(1, 57), 783_720);
        assert_eq!(t.count(1, WENDYS_NECKLACE), 0);
        let w: Vec<_> = t
            .take_writes()
            .into_iter()
            .filter(|w| !format!("{w:?}").contains("__compltdStateFlags"))
            .collect();
        assert!(w.ends_with(&[
            rows::delete(NAME, false),
            rows::update(NAME, "<state>", "Completed")
        ]));
    }
}
