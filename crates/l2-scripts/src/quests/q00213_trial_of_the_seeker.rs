//! Quest 213: Trial of the Seeker (Master Dufner, Master Terry, Brunon, Viktor and Marina; the rogue's mark).
// Java: dist/game/data/scripts/quests/Q00213_TrialOfTheSeeker/Q00213_TrialOfTheSeeker.java

use l2_util::ObjectId;
use l2_world::script::{QuestSound, STARTED, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const MASTER_TERRY: i32 = 30064;
const MASTER_DUFNER: i32 = 30106;
const BLACKSMITH_BRUNON: i32 = 30526;
const TRADER_VIKTOR: i32 = 30684;
const MAGISTER_MARINA: i32 = 30715;

// Items
const DUFNERS_LETTER: i32 = 2647;
const TERRYS_1ST_ORDER: i32 = 2648;
const TERRYS_2ND_ORDER: i32 = 2649;
const TERRYS_LETTER: i32 = 2650;
const VIKTORS_LETTER: i32 = 2651;
const HAWKEYES_LETTER: i32 = 2652;
const MYSTERIOUS_SPIRIT_ORE: i32 = 2653;
const OL_MAHUM_SPIRIT_ORE: i32 = 2654;
const TUREK_SPIRIT_ORE: i32 = 2655;
const ANT_SPIRIT_ORE: i32 = 2656;
const TURAK_BUGBEAR_SPIRIT_ORE: i32 = 2657;
const TERRY_BOX: i32 = 2658;
const VIKTORS_REQUEST: i32 = 2659;
const MEDUSA_SCALES: i32 = 2660;
const SHILENS_SPIRIT_ORE: i32 = 2661;
const ANALYSIS_REQUEST: i32 = 2662;
const MARINAS_LETTER: i32 = 2663;
const EXPERIMENT_TOOLS: i32 = 2664;
const ANALYSIS_RESULT: i32 = 2665;
const TERRYS_3RD_ORDER: i32 = 2666;
const LIST_OF_HOST: i32 = 2667;
const ABYSS_SPIRIT_ORE1: i32 = 2668;
const ABYSS_SPIRIT_ORE2: i32 = 2669;
const ABYSS_SPIRIT_ORE3: i32 = 2670;
const ABYSS_SPIRIT_ORE4: i32 = 2671;
const TERRYS_REPORT: i32 = 2672;

// Reward
const MARK_OF_SEEKER: i32 = 2673;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const ANT_CAPTAIN: i32 = 20080;
const ANT_WARRIOR_CAPTAIN: i32 = 20088;
const MEDUSA: i32 = 20158;
const NEER_GHOUL_BERSERKER: i32 = 20198;
const OL_MAHUM_CAPTAIN: i32 = 20211;
const MARSH_STAKATO_DRONE: i32 = 20234;
const TURAK_BUGBEAR_WARRIOR: i32 = 20249;
const BREKA_ORC_OVERLORD: i32 = 20270;
const TUREK_ORC_WARLORD: i32 = 20495;
const LETO_LIZARDMAN_WARRIOR: i32 = 20580;

// Misc
const MIN_LEVEL: i32 = 35;
const LEVEL: i32 = 36;
/// Java `player.getVariables()` key: the second-class diamonds are given once per character.
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

/// Java `Q00213_TrialOfTheSeeker`.
pub struct Q00213TrialOfTheSeeker;

/// Java `giveItems(killer, ore, 1); playSound(MIDDLE);` of the "one ore per monster" kills: the
/// kill gives `ore` when the player holds `order` and not the ore yet, and sets `cond_to` (no
/// sound) once all of `others` are there too.
fn ore_kill(
    ctx: &mut ScriptCtx<'_>,
    qs: l2_world::script::Qs,
    killer: ObjectId,
    order: i32,
    ore: i32,
    others: &[i32],
    cond_to: i32,
) {
    if ctx.has_quest_items(killer, order) && !ctx.has_quest_items(killer, ore) {
        ctx.give_items(killer, ore, 1);
        ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
        if ctx.has_all_quest_items(killer, others) {
            qs.set_cond(ctx, cond_to, false);
        }
    }
}

impl Script for Q00213TrialOfTheSeeker {
    fn name(&self) -> &'static str {
        "Q00213_TrialOfTheSeeker"
    }

    fn path(&self) -> &'static str {
        "quests/Q00213_TrialOfTheSeeker"
    }

    fn id(&self) -> i32 {
        213
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_DUFNER]);
        s.add_talk_id(&[
            MASTER_DUFNER,
            MASTER_TERRY,
            BLACKSMITH_BRUNON,
            TRADER_VIKTOR,
            MAGISTER_MARINA,
        ]);
        s.add_kill_id(&[
            ANT_CAPTAIN,
            ANT_WARRIOR_CAPTAIN,
            MEDUSA,
            NEER_GHOUL_BERSERKER,
            OL_MAHUM_CAPTAIN,
            MARSH_STAKATO_DRONE,
            TURAK_BUGBEAR_WARRIOR,
            BREKA_ORC_OVERLORD,
            TUREK_ORC_WARLORD,
            LETO_LIZARDMAN_WARRIOR,
        ]);
        s.register_quest_items(&[
            DUFNERS_LETTER,
            TERRYS_1ST_ORDER,
            TERRYS_2ND_ORDER,
            TERRYS_LETTER,
            VIKTORS_LETTER,
            HAWKEYES_LETTER,
            MYSTERIOUS_SPIRIT_ORE,
            OL_MAHUM_SPIRIT_ORE,
            TUREK_SPIRIT_ORE,
            ANT_SPIRIT_ORE,
            TURAK_BUGBEAR_SPIRIT_ORE,
            TERRY_BOX,
            VIKTORS_REQUEST,
            MEDUSA_SCALES,
            SHILENS_SPIRIT_ORE,
            ANALYSIS_REQUEST,
            MARINAS_LETTER,
            EXPERIMENT_TOOLS,
            ANALYSIS_RESULT,
            TERRYS_3RD_ORDER,
            LIST_OF_HOST,
            ABYSS_SPIRIT_ORE1,
            ABYSS_SPIRIT_ORE2,
            ABYSS_SPIRIT_ORE3,
            ABYSS_SPIRIT_ORE4,
            TERRYS_REPORT,
        ]);
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
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    if !ctx.has_quest_items(player, DUFNERS_LETTER) {
                        ctx.give_items(player, DUFNERS_LETTER, 1);
                    }
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 128);
                        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
                        htmltext = Some("30106-05a.htm".to_owned());
                    } else {
                        htmltext = Some("30106-05.htm".to_owned());
                    }
                }
            }
            "30106-04.htm" | "30064-02.html" | "30064-07.html" | "30064-16.html" | "30064-17.html"
            | "30064-19.html" | "30684-02.html" | "30684-03.html" | "30684-04.html" | "30684-06.html"
            | "30684-07.html" | "30684-08.html" | "30684-09.html" | "30684-10.html" => {
                htmltext = Some(event.to_owned());
            }
            "30064-03.html" => {
                if ctx.has_quest_items(player, DUFNERS_LETTER) {
                    ctx.take_items(player, DUFNERS_LETTER, 1);
                    ctx.give_items(player, TERRYS_1ST_ORDER, 1);
                    qs.set_cond(ctx, 2, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30064-06.html" => {
                if ctx.has_quest_items(player, TERRYS_1ST_ORDER) {
                    ctx.take_items(player, TERRYS_1ST_ORDER, 1);
                    ctx.give_items(player, TERRYS_2ND_ORDER, 1);
                    ctx.take_items(player, MYSTERIOUS_SPIRIT_ORE, 1);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30064-10.html" => {
                ctx.give_items(player, TERRYS_LETTER, 1);
                ctx.take_items(player, OL_MAHUM_SPIRIT_ORE, 1);
                ctx.take_items(player, TUREK_SPIRIT_ORE, 1);
                ctx.take_items(player, ANT_SPIRIT_ORE, 1);
                ctx.take_items(player, TURAK_BUGBEAR_SPIRIT_ORE, 1);
                ctx.take_items(player, TERRYS_2ND_ORDER, 1);
                ctx.give_items(player, TERRY_BOX, 1);
                qs.set_cond(ctx, 6, true);
                htmltext = Some(event.to_owned());
            }
            "30064-18.html" => {
                if ctx.has_quest_items(player, ANALYSIS_RESULT) {
                    ctx.take_items(player, ANALYSIS_RESULT, 1);
                    ctx.give_items(player, LIST_OF_HOST, 1);
                    qs.set_cond(ctx, 15, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30684-05.html" => {
                if ctx.has_quest_items(player, TERRYS_LETTER) {
                    ctx.take_items(player, TERRYS_LETTER, 1);
                    ctx.give_items(player, VIKTORS_LETTER, 1);
                    qs.set_cond(ctx, 7, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30684-11.html" => {
                ctx.take_items(player, TERRYS_LETTER, 1);
                ctx.take_items(player, TERRY_BOX, 1);
                ctx.take_items(player, HAWKEYES_LETTER, 1);
                ctx.take_items(player, VIKTORS_LETTER, 1);
                ctx.give_items(player, VIKTORS_REQUEST, 1);
                qs.set_cond(ctx, 9, true);
                htmltext = Some(event.to_owned());
            }
            "30684-15.html" => {
                ctx.take_items(player, VIKTORS_REQUEST, 1);
                ctx.take_items(player, MEDUSA_SCALES, -1);
                ctx.give_items(player, SHILENS_SPIRIT_ORE, 1);
                ctx.give_items(player, ANALYSIS_REQUEST, 1);
                qs.set_cond(ctx, 11, true);
                htmltext = Some(event.to_owned());
            }
            "30715-02.html" => {
                ctx.take_items(player, SHILENS_SPIRIT_ORE, 1);
                ctx.take_items(player, ANALYSIS_REQUEST, 1);
                ctx.give_items(player, MARINAS_LETTER, 1);
                qs.set_cond(ctx, 12, true);
                htmltext = Some(event.to_owned());
            }
            "30715-05.html" => {
                ctx.take_items(player, EXPERIMENT_TOOLS, 1);
                ctx.give_items(player, ANALYSIS_RESULT, 1);
                qs.set_cond(ctx, 14, true);
                htmltext = Some(event.to_owned());
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if !(qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true))
        {
            return;
        }

        match npc.id {
            ANT_CAPTAIN => ore_kill(
                ctx,
                qs,
                killer,
                TERRYS_2ND_ORDER,
                ANT_SPIRIT_ORE,
                &[OL_MAHUM_SPIRIT_ORE, TUREK_SPIRIT_ORE, TURAK_BUGBEAR_SPIRIT_ORE],
                5,
            ),
            ANT_WARRIOR_CAPTAIN => ore_kill(
                ctx,
                qs,
                killer,
                LIST_OF_HOST,
                ABYSS_SPIRIT_ORE3,
                &[ABYSS_SPIRIT_ORE1, ABYSS_SPIRIT_ORE2, ABYSS_SPIRIT_ORE4],
                16,
            ),
            MEDUSA => {
                if ctx.has_quest_items(killer, VIKTORS_REQUEST)
                    && ctx.get_quest_items_count(killer, MEDUSA_SCALES) < 10
                {
                    ctx.give_items(killer, MEDUSA_SCALES, 1);
                    if ctx.get_quest_items_count(killer, MEDUSA_SCALES) == 10 {
                        qs.set_cond(ctx, 10, true);
                    } else {
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            NEER_GHOUL_BERSERKER => {
                if ctx.has_quest_items(killer, TERRYS_1ST_ORDER)
                    && !ctx.has_quest_items(killer, MYSTERIOUS_SPIRIT_ORE)
                    && ctx.get_random_boolean()
                {
                    ctx.give_items(killer, MYSTERIOUS_SPIRIT_ORE, 1);
                    qs.set_cond(ctx, 3, true);
                }
            }
            OL_MAHUM_CAPTAIN => ore_kill(
                ctx,
                qs,
                killer,
                TERRYS_2ND_ORDER,
                OL_MAHUM_SPIRIT_ORE,
                &[TUREK_SPIRIT_ORE, ANT_SPIRIT_ORE, TURAK_BUGBEAR_SPIRIT_ORE],
                5,
            ),
            MARSH_STAKATO_DRONE => ore_kill(
                ctx,
                qs,
                killer,
                LIST_OF_HOST,
                ABYSS_SPIRIT_ORE1,
                &[ABYSS_SPIRIT_ORE2, ABYSS_SPIRIT_ORE3, ABYSS_SPIRIT_ORE4],
                16,
            ),
            TURAK_BUGBEAR_WARRIOR => ore_kill(
                ctx,
                qs,
                killer,
                TERRYS_2ND_ORDER,
                TURAK_BUGBEAR_SPIRIT_ORE,
                &[OL_MAHUM_SPIRIT_ORE, TUREK_SPIRIT_ORE, ANT_SPIRIT_ORE],
                5,
            ),
            BREKA_ORC_OVERLORD => ore_kill(
                ctx,
                qs,
                killer,
                LIST_OF_HOST,
                ABYSS_SPIRIT_ORE2,
                &[ABYSS_SPIRIT_ORE1, ABYSS_SPIRIT_ORE3, ABYSS_SPIRIT_ORE4],
                16,
            ),
            TUREK_ORC_WARLORD => ore_kill(
                ctx,
                qs,
                killer,
                TERRYS_2ND_ORDER,
                TUREK_SPIRIT_ORE,
                &[OL_MAHUM_SPIRIT_ORE, ANT_SPIRIT_ORE, TURAK_BUGBEAR_SPIRIT_ORE],
                5,
            ),
            LETO_LIZARDMAN_WARRIOR => ore_kill(
                ctx,
                qs,
                killer,
                LIST_OF_HOST,
                ABYSS_SPIRIT_ORE4,
                &[ABYSS_SPIRIT_ORE1, ABYSS_SPIRIT_ORE2, ABYSS_SPIRIT_ORE3],
                16,
            ),
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == MASTER_DUFNER {
                let class = ctx.player_class_id(player);
                if class == player_class::ROGUE
                    || class == player_class::ELVEN_SCOUT
                    || class == player_class::ASSASSIN
                {
                    if ctx.player_level(player) < MIN_LEVEL {
                        htmltext = "30106-02.html".to_owned();
                    } else {
                        htmltext = "30106-03.htm".to_owned();
                    }
                } else {
                    htmltext = "30106-01.html".to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                MASTER_DUFNER => {
                    if ctx.has_quest_items(player, DUFNERS_LETTER)
                        && !ctx.has_quest_items(player, TERRYS_REPORT)
                    {
                        htmltext = "30106-06.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(player, &[DUFNERS_LETTER, TERRYS_REPORT]) {
                        htmltext = "30106-07.html".to_owned();
                    } else if ctx.has_quest_items(player, TERRYS_REPORT)
                        && !ctx.has_quest_items(player, DUFNERS_LETTER)
                    {
                        ctx.give_adena(player, 187_606, true);
                        ctx.give_items(player, MARK_OF_SEEKER, 1);
                        ctx.add_exp_and_sp(player, 1_029_478, 66_768);
                        qs.exit_quest(ctx, false, true);
                        ctx.send_social_action(player, 3);
                        htmltext = "30106-08.html".to_owned();
                    }
                }
                MASTER_TERRY => {
                    if ctx.has_quest_items(player, DUFNERS_LETTER) {
                        htmltext = "30064-01.html".to_owned();
                    } else if ctx.has_quest_items(player, TERRYS_1ST_ORDER) {
                        if !ctx.has_quest_items(player, MYSTERIOUS_SPIRIT_ORE) {
                            htmltext = "30064-04.html".to_owned();
                        } else {
                            htmltext = "30064-05.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, TERRYS_2ND_ORDER) {
                        let ores = ctx.get_quest_items_count(player, OL_MAHUM_SPIRIT_ORE)
                            + ctx.get_quest_items_count(player, TUREK_SPIRIT_ORE)
                            + ctx.get_quest_items_count(player, ANT_SPIRIT_ORE)
                            + ctx.get_quest_items_count(player, TURAK_BUGBEAR_SPIRIT_ORE);
                        if ores < 4 {
                            htmltext = "30064-08.html".to_owned();
                        } else {
                            htmltext = "30064-09.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, TERRYS_LETTER) {
                        htmltext = "30064-11.html".to_owned();
                    } else if ctx.has_quest_items(player, VIKTORS_LETTER) {
                        ctx.take_items(player, VIKTORS_LETTER, 1);
                        ctx.give_items(player, HAWKEYES_LETTER, 1);
                        qs.set_cond(ctx, 8, true);
                        htmltext = "30064-12.html".to_owned();
                    } else if ctx.has_quest_items(player, HAWKEYES_LETTER) {
                        htmltext = "30064-13.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            VIKTORS_REQUEST,
                            ANALYSIS_REQUEST,
                            MARINAS_LETTER,
                            EXPERIMENT_TOOLS,
                        ],
                    ) {
                        htmltext = "30064-14.html".to_owned();
                    } else if ctx.has_quest_items(player, ANALYSIS_RESULT) {
                        htmltext = "30064-15.html".to_owned();
                    } else if ctx.has_quest_items(player, TERRYS_3RD_ORDER) {
                        if ctx.player_level(player) < LEVEL {
                            htmltext = "30064-20.html".to_owned();
                        } else {
                            ctx.take_items(player, TERRYS_3RD_ORDER, 1);
                            ctx.give_items(player, LIST_OF_HOST, 1);
                            qs.set_cond(ctx, 15, true);
                            htmltext = "30064-21.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, LIST_OF_HOST) {
                        let ores = ctx.get_quest_items_count(player, ABYSS_SPIRIT_ORE1)
                            + ctx.get_quest_items_count(player, ABYSS_SPIRIT_ORE2)
                            + ctx.get_quest_items_count(player, ABYSS_SPIRIT_ORE3)
                            + ctx.get_quest_items_count(player, ABYSS_SPIRIT_ORE4);
                        if ores < 4 {
                            htmltext = "30064-22.html".to_owned();
                        } else {
                            ctx.take_items(player, LIST_OF_HOST, 1);
                            ctx.take_items(player, ABYSS_SPIRIT_ORE1, 1);
                            ctx.take_items(player, ABYSS_SPIRIT_ORE2, 1);
                            ctx.take_items(player, ABYSS_SPIRIT_ORE3, 1);
                            ctx.take_items(player, ABYSS_SPIRIT_ORE4, 1);
                            ctx.give_items(player, TERRYS_REPORT, 1);
                            qs.set_cond(ctx, 17, true);
                            htmltext = "30064-23.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, TERRYS_REPORT) {
                        htmltext = "30064-24.html".to_owned();
                    }
                }
                BLACKSMITH_BRUNON => {
                    if ctx.has_quest_items(player, MARINAS_LETTER) {
                        ctx.take_items(player, MARINAS_LETTER, 1);
                        ctx.give_items(player, EXPERIMENT_TOOLS, 1);
                        qs.set_cond(ctx, 13, true);
                        htmltext = "30526-01.html".to_owned();
                    } else if ctx.has_quest_items(player, EXPERIMENT_TOOLS) {
                        htmltext = "30526-02.html".to_owned();
                    }
                }
                TRADER_VIKTOR => {
                    if ctx.has_quest_items(player, TERRYS_LETTER) {
                        htmltext = "30684-01.html".to_owned();
                    } else if ctx.has_quest_items(player, HAWKEYES_LETTER) {
                        htmltext = "30684-12.html".to_owned();
                    } else if ctx.has_quest_items(player, VIKTORS_REQUEST) {
                        if ctx.get_quest_items_count(player, MEDUSA_SCALES) < 10 {
                            htmltext = "30684-13.html".to_owned();
                        } else {
                            htmltext = "30684-14.html".to_owned();
                        }
                    } else if ctx.has_all_quest_items(player, &[SHILENS_SPIRIT_ORE, ANALYSIS_REQUEST]) {
                        htmltext = "30684-16.html".to_owned();
                    } else if ctx.has_all_quest_items(
                        player,
                        &[MARINAS_LETTER, EXPERIMENT_TOOLS, ANALYSIS_REQUEST, TERRYS_REPORT],
                    ) {
                        htmltext = "30684-17.html".to_owned();
                    } else if ctx.has_quest_items(player, VIKTORS_LETTER) {
                        htmltext = "30684-05.html".to_owned();
                    }
                }
                MAGISTER_MARINA => {
                    if ctx.has_all_quest_items(player, &[SHILENS_SPIRIT_ORE, ANALYSIS_REQUEST]) {
                        htmltext = "30715-01.html".to_owned();
                    } else if ctx.has_quest_items(player, MARINAS_LETTER) {
                        htmltext = "30715-03.html".to_owned();
                    } else if ctx.has_quest_items(player, EXPERIMENT_TOOLS) {
                        htmltext = "30715-04.html".to_owned();
                    } else if ctx.has_quest_items(player, ANALYSIS_RESULT) {
                        htmltext = "30715-06.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == MASTER_DUFNER {
            htmltext = ctx.get_already_completed_msg(player);
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
    use crate::testing::{Pages, ScriptTest, names_to, rows};

    const NAME: &str = "Q00213_TrialOfTheSeeker";

    const NPCS: &str = r#"
        <npc id="30064" type="Folk" level="70" name="Master Terry"/>
        <npc id="30106" type="Folk" level="70" name="Master Dufner"/>
        <npc id="30526" type="Folk" level="70" name="Blacksmith Brunon"/>
        <npc id="30684" type="Folk" level="70" name="Trader Viktor"/>
        <npc id="30715" type="Folk" level="70" name="Magister Marina"/>
        <npc id="20080" type="Monster" level="30" name="Ant Captain"/>
        <npc id="20088" type="Monster" level="30" name="Ant Warrior Captain"/>
        <npc id="20158" type="Monster" level="30" name="Medusa"/>
        <npc id="20198" type="Monster" level="30" name="Neer Ghoul Berserker"/>
        <npc id="20211" type="Monster" level="30" name="Ol Mahum Captain"/>
        <npc id="20234" type="Monster" level="30" name="Marsh Stakato Drone"/>
        <npc id="20249" type="Monster" level="30" name="Turak Bugbear Warrior"/>
        <npc id="20270" type="Monster" level="30" name="Breka Orc Overlord"/>
        <npc id="20495" type="Monster" level="30" name="Turek Orc Warlord"/>
        <npc id="20580" type="Monster" level="30" name="Leto Lizardman Warrior"/>"#;

    fn items() -> String {
        let mut s = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="2673" type="EtcItem" name="Mark of Seeker"><set name="is_questitem" val="true"/></item>"#,
        );
        for id in 2647..=2672 {
            let stack = if id == MEDUSA_SCALES {
                r#"<set name="is_stackable" val="true"/>"#
            } else {
                ""
            };
            s.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Q213 {id}"><set name="is_questitem" val="true"/>{stack}</item>"#
            ));
        }
        s
    }

    /// Every page of the quest links to every event the test clicks, so the chain can be driven
    /// without the real links.
    const EVENTS: [&str; 16] = [
        "ACCEPT",
        "30064-03.html",
        "30064-06.html",
        "30064-10.html",
        "30064-18.html",
        "30684-05.html",
        "30684-11.html",
        "30684-15.html",
        "30715-02.html",
        "30715-05.html",
        "30106-04.htm",
        "30064-02.html",
        "30684-02.html",
        "30684-03.html",
        "30684-04.html",
        "30684-06.html",
    ];

    const FILES: [&str; 24] = [
        "30106-01.html",
        "30106-02.html",
        "30106-03.htm",
        "30106-04.htm",
        "30106-05.htm",
        "30106-05a.htm",
        "30106-06.html",
        "30106-08.html",
        "30064-01.html",
        "30064-12.html",
        "30064-21.html",
        "30064-23.html",
        "30684-01.html",
        "30684-05.html",
        "30684-14.html",
        "30684-16.html",
        "30715-01.html",
        "30526-01.html",
        "30064-05.html",
        "30064-09.html",
        "30064-15.html",
        "30715-04.html",
        "30684-13.html",
        "30064-24.html",
    ];

    struct Npcs {
        dufner: Oid,
        terry: Oid,
        brunon: Oid,
        viktor: Oid,
        marina: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00213TrialOfTheSeeker);
        let mut pages = Pages::of(&Q00213TrialOfTheSeeker);
        for file in FILES {
            pages = pages.page(file, &EVENTS);
        }
        for event in EVENTS.iter().filter(|e| **e != "ACCEPT" && !FILES.contains(*e)) {
            pages = pages.page(event, &EVENTS);
        }
        let pages = pages.npcs(&[
            MASTER_DUFNER,
            MASTER_TERRY,
            BLACKSMITH_BRUNON,
            TRADER_VIKTOR,
            MAGISTER_MARINA,
        ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let npcs = Npcs {
            dufner: t.spawn(MASTER_DUFNER, 20),
            terry: t.spawn(MASTER_TERRY, 25),
            brunon: t.spawn(BLACKSMITH_BRUNON, 30),
            viktor: t.spawn(TRADER_VIKTOR, 35),
            marina: t.spawn(MAGISTER_MARINA, 40),
        };
        (t, npcs)
    }

    fn kill_one(t: &mut ScriptTest, npc_id: i32) {
        let mob = t.spawn(npc_id, 60);
        t.kill(1, mob);
        t.world.delete_npc(mob);
    }

    #[test]
    fn only_a_rogue_of_level_35_is_offered_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 35, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.dufner).contains("30106-01.html"));
        t.enter_as(2, 34, race::HUMAN, player_class::ROGUE, vec![]);
        assert!(t.quest_page(2, npcs.dufner).contains("30106-02.html"));
        t.enter_as(3, 35, race::ELF, player_class::ELVEN_SCOUT, vec![]);
        assert!(t.quest_page(3, npcs.dufner).contains("30106-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_whole_trial_to_the_mark() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 36, race::HUMAN, player_class::ROGUE, vec![]);
        assert!(t.quest_page(1, npcs.dufner).contains("30106-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30106-05a.htm"));
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 128);
        assert_eq!(t.count(1, DUFNERS_LETTER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started")
            ]
        );

        // Terry's first order, a ghoul's ore (the world RNG decides how many kills).
        assert!(t.quest_page(1, npcs.terry).contains("30064-01.html"));
        assert!(t.event(1, NAME, "30064-03.html").contains("30064-03.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);
        for _ in 0..40 {
            if t.count(1, MYSTERIOUS_SPIRIT_ORE) == 1 {
                break;
            }
            kill_one(&mut t, NEER_GHOUL_BERSERKER);
        }
        assert_eq!(t.count(1, MYSTERIOUS_SPIRIT_ORE), 1);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);

        // The second order: four ores; the last sets cond 5.
        assert!(t.quest_page(1, npcs.terry).contains("30064-05.html"));
        assert!(t.event(1, NAME, "30064-06.html").contains("30064-06.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "4")]);
        for id in [
            ANT_CAPTAIN,
            OL_MAHUM_CAPTAIN,
            TUREK_ORC_WARLORD,
            TURAK_BUGBEAR_WARRIOR,
        ] {
            kill_one(&mut t, id);
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "5")]);
        assert!(t.quest_page(1, npcs.terry).contains("30064-09.html"));
        assert!(t.event(1, NAME, "30064-10.html").contains("30064-10.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "6")]);

        // Viktor, Terry, Viktor: ten medusa scales.
        assert!(t.event(1, NAME, "30684-05.html").contains("30684-05.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "7")]);
        assert!(t.quest_page(1, npcs.terry).contains("30064-12.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "8")]);
        assert!(t.event(1, NAME, "30684-11.html").contains("30684-11.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "9")]);
        for _ in 0..10 {
            kill_one(&mut t, MEDUSA);
        }
        assert_eq!(t.count(1, MEDUSA_SCALES), 10);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "10")]);
        assert!(t.quest_page(1, npcs.viktor).contains("30684-14.html"));
        assert!(t.event(1, NAME, "30684-15.html").contains("30684-15.html"));
        assert_eq!(t.count(1, MEDUSA_SCALES), 0);
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "11")]);

        // Marina, Brunon, Marina, Terry.
        assert!(t.quest_page(1, npcs.marina).contains("30715-01.html"));
        assert!(t.event(1, NAME, "30715-02.html").contains("30715-02.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "12")]);
        assert!(t.quest_page(1, npcs.brunon).contains("30526-01.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "13")]);
        assert!(t.quest_page(1, npcs.marina).contains("30715-04.html"));
        assert!(t.event(1, NAME, "30715-05.html").contains("30715-05.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "14")]);
        assert!(t.quest_page(1, npcs.terry).contains("30064-15.html"));
        assert!(t.event(1, NAME, "30064-18.html").contains("30064-18.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "15")]);

        // The four abyss ores, the report, the mark.
        for id in [
            MARSH_STAKATO_DRONE,
            BREKA_ORC_OVERLORD,
            ANT_WARRIOR_CAPTAIN,
            LETO_LIZARDMAN_WARRIOR,
        ] {
            kill_one(&mut t, id);
        }
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "16")]);
        assert!(t.quest_page(1, npcs.terry).contains("30064-23.html"));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "17")]);

        t.talk(1, npcs.dufner);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_SEEKER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
        assert!(t.quest_page(1, npcs.dufner).contains("already been completed"));
    }
}
