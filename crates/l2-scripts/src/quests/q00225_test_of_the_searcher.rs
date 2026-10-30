//! Quest 225: Test of the Searcher (Master Luther in Talking Island: the Scavenger second class
//! change, a trail through Captain Alex, Militiaman Leirynn, Tyra, the Strong Wooden Chest and the
//! Bodyguard Jax).
// Java: dist/game/data/scripts/quests/Q00225_TestOfTheSearcher/Q00225_TestOfTheSearcher.java
//
// Pattern: the test chain of the "Test of ..." quests (see Q00222) with a step that spawns a
// monster when it is hit (`onAttack` + `isScriptValue`) and a tree that drops a chest.

use l2_protocol_game::server::SocialAction;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const CAPTAIN_ALEX: i32 = 30291;
const TYRA: i32 = 30420;
const TREE: i32 = 30627;
const STRONG_WOODEN_CHEST: i32 = 30628;
const MASTER_LUTHER: i32 = 30690;
const MILITIAMAN_LEIRYNN: i32 = 30728;
const DRUNKARD_BORYS: i32 = 30729;
const BODYGUARD_JAX: i32 = 30730;

// Items
const LUTHERS_LETTER: i32 = 2784;
const ALEXS_WARRANT: i32 = 2785;
const LEIRYNNS_1ST_ORDER: i32 = 2786;
const DELU_TOTEM: i32 = 2787;
const LEIRYNNS_2ND_ORDER: i32 = 2788;
const CHIEF_KALKIS_FANG: i32 = 2789;
const LEIRYNNS_REPORT: i32 = 2790;
const STRINGE_MAP: i32 = 2791;
const LAMBERTS_MAP: i32 = 2792;
const ALEXS_LETTER: i32 = 2793;
const ALEXS_ORDER: i32 = 2794;
const WINE_CATALOG: i32 = 2795;
const TYRAS_CONTRACT: i32 = 2796;
const RED_SPORE_DUST: i32 = 2797;
const MALRUKIAN_WINE: i32 = 2798;
const OLD_ORDER: i32 = 2799;
const JAXS_DIARY: i32 = 2800;
const TORN_MAP_PIECE_1ST: i32 = 2801;
const TORN_MAP_PIECE_2ND: i32 = 2802;
const SOLTS_MAP: i32 = 2803;
const MAKELS_MAP: i32 = 2804;
const COMBINED_MAP: i32 = 2805;
const RUSTED_KEY: i32 = 2806;
const GOLD_BAR: i32 = 2807;
const ALEXS_RECOMMEND: i32 = 2808;

// Reward
const MARK_OF_SEARCHER: i32 = 2809;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const HANGMAN_TREE: i32 = 20144;
const ROAD_SCAVENGER: i32 = 20551;
const GIANT_FUNGUS: i32 = 20555;
const DELU_LIZARDMAN_SHAMAN: i32 = 20781;

// Quest monsters
const NEER_BODYGUARD: i32 = 27092;
const DELU_CHIEF_KALKIS: i32 = 27093;

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `Q00225_TestOfTheSearcher`.
pub struct Q00225TestOfTheSearcher;

impl Script for Q00225TestOfTheSearcher {
    fn name(&self) -> &'static str {
        "Q00225_TestOfTheSearcher"
    }

    fn path(&self) -> &'static str {
        "quests/Q00225_TestOfTheSearcher"
    }

    fn id(&self) -> i32 {
        225
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[MASTER_LUTHER]);
        s.add_talk_id(&[
            MASTER_LUTHER,
            CAPTAIN_ALEX,
            TYRA,
            TREE,
            STRONG_WOODEN_CHEST,
            MILITIAMAN_LEIRYNN,
            DRUNKARD_BORYS,
            BODYGUARD_JAX,
        ]);
        s.add_kill_id(&[
            HANGMAN_TREE,
            ROAD_SCAVENGER,
            GIANT_FUNGUS,
            DELU_LIZARDMAN_SHAMAN,
            NEER_BODYGUARD,
            DELU_CHIEF_KALKIS,
        ]);
        s.add_attack_id(&[DELU_LIZARDMAN_SHAMAN]);
        s.register_quest_items(&[
            LUTHERS_LETTER,
            ALEXS_WARRANT,
            LEIRYNNS_1ST_ORDER,
            DELU_TOTEM,
            LEIRYNNS_2ND_ORDER,
            CHIEF_KALKIS_FANG,
            LEIRYNNS_REPORT,
            STRINGE_MAP,
            LAMBERTS_MAP,
            ALEXS_LETTER,
            ALEXS_ORDER,
            WINE_CATALOG,
            TYRAS_CONTRACT,
            RED_SPORE_DUST,
            MALRUKIAN_WINE,
            OLD_ORDER,
            JAXS_DIARY,
            TORN_MAP_PIECE_1ST,
            TORN_MAP_PIECE_2ND,
            SOLTS_MAP,
            MAKELS_MAP,
            COMBINED_MAP,
            RUSTED_KEY,
            GOLD_BAR,
            ALEXS_RECOMMEND,
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

        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    ctx.give_items(player, LUTHERS_LETTER, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        if ctx.player_class_id(player) == player_class::SCAVENGER {
                            ctx.give_items(player, DIMENSIONAL_DIAMOND, 82);
                        } else {
                            ctx.give_items(player, DIMENSIONAL_DIAMOND, 96);
                        }
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30690-05a.htm".to_owned());
                    } else {
                        htmltext = Some("30690-05.htm".to_owned());
                    }
                }
            }
            "30291-05.html" | "30291-01t.html" | "30291-06.html" | "30730-01a.html" | "30730-01b.html"
            | "30730-01c.html" | "30730-02.html" | "30730-02a.html" | "30730-02b.html" => {
                htmltext = Some(event.to_owned());
            }
            "30291-07.html" => {
                if ctx.has_quest_items(player, LEIRYNNS_REPORT) && ctx.has_quest_items(player, STRINGE_MAP) {
                    ctx.take_items(player, LEIRYNNS_REPORT, 1);
                    ctx.take_items(player, STRINGE_MAP, 1);
                    ctx.give_items(player, LAMBERTS_MAP, 1);
                    ctx.give_items(player, ALEXS_LETTER, 1);
                    ctx.give_items(player, ALEXS_ORDER, 1);
                    qs.set_cond(ctx, 8, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30420-01a.html" => {
                if ctx.has_quest_items(player, WINE_CATALOG) {
                    ctx.take_items(player, WINE_CATALOG, 1);
                    ctx.give_items(player, TYRAS_CONTRACT, 1);
                    qs.set_cond(ctx, 10, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30627-01a.html" => {
                let tree = npc?;
                if ctx.summoned_npc_count(tree) < 5 {
                    ctx.give_items(player, RUSTED_KEY, 1);
                    ctx.add_spawn_summoned_at(tree, STRONG_WOODEN_CHEST, tree.object_id, true, 0);
                    qs.set_cond(ctx, 17, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30628-01a.html" => {
                let chest = npc?;
                ctx.take_items(player, RUSTED_KEY, 1);
                ctx.give_items(player, GOLD_BAR, 20);
                qs.set_cond(ctx, 18, true);
                ctx.delete_npc(chest);
                htmltext = Some(event.to_owned());
            }
            "30730-01d.html" => {
                if ctx.has_quest_items(player, OLD_ORDER) {
                    ctx.take_items(player, OLD_ORDER, 1);
                    ctx.give_items(player, JAXS_DIARY, 1);
                    qs.set_cond(ctx, 14, true);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_attack(
        &self,
        ctx: &mut ScriptCtx<'_>,
        npc: ScriptNpc,
        attacker: ObjectId,
        _damage: i32,
        _is_summon: bool,
        _skill: Option<SkillKey>,
    ) {
        let Some(qs) = ctx.get_quest_state(attacker, false) else {
            return;
        };
        if qs.is_started(ctx)
            && ctx.is_script_value(npc, 0)
            && ctx.has_quest_items(attacker, LEIRYNNS_1ST_ORDER)
        {
            ctx.set_script_value(npc, 1);
            if let Some(bodyguard) = ctx.add_spawn_at(NEER_BODYGUARD, npc.object_id, true, 200_000) {
                ctx.add_attack_desire(bodyguard, attacker, 999);
            }
        }
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        let Some(qs) = ctx.get_quest_state(killer, false) else {
            return;
        };
        if qs.is_started(ctx) && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true) {
            match npc.id {
                HANGMAN_TREE => {
                    if ctx.has_quest_items(killer, JAXS_DIARY)
                        && !ctx.has_quest_items(killer, MAKELS_MAP)
                        && ctx.get_quest_items_count(killer, TORN_MAP_PIECE_2ND) < 4
                    {
                        if ctx.get_quest_items_count(killer, TORN_MAP_PIECE_2ND) < 3 {
                            if ctx.get_random(100) < 50 {
                                ctx.give_items(killer, TORN_MAP_PIECE_2ND, 1);
                                ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                            }
                        } else if ctx.get_random(100) < 50 {
                            ctx.take_items(killer, TORN_MAP_PIECE_2ND, -1);
                            ctx.give_items(killer, MAKELS_MAP, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                            if ctx.get_quest_items_count(killer, SOLTS_MAP) >= 1 {
                                qs.set_cond(ctx, 15, false);
                            }
                        }
                    }
                }
                ROAD_SCAVENGER => {
                    if ctx.has_quest_items(killer, JAXS_DIARY)
                        && !ctx.has_quest_items(killer, SOLTS_MAP)
                        && ctx.get_quest_items_count(killer, TORN_MAP_PIECE_1ST) < 4
                    {
                        if ctx.get_quest_items_count(killer, TORN_MAP_PIECE_1ST) < 3 {
                            ctx.give_items(killer, TORN_MAP_PIECE_1ST, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        } else {
                            ctx.take_items(killer, TORN_MAP_PIECE_1ST, -1);
                            ctx.give_items(killer, SOLTS_MAP, 1);
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestMiddle);
                            if ctx.get_quest_items_count(killer, MAKELS_MAP) >= 1 {
                                qs.set_cond(ctx, 15, false);
                            }
                        }
                    }
                }
                GIANT_FUNGUS => {
                    if ctx.has_quest_items(killer, TYRAS_CONTRACT)
                        && ctx.get_quest_items_count(killer, RED_SPORE_DUST) < 10
                    {
                        ctx.give_items(killer, RED_SPORE_DUST, 1);
                        if ctx.get_quest_items_count(killer, RED_SPORE_DUST) >= 10 {
                            qs.set_cond(ctx, 11, true);
                        } else {
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                DELU_LIZARDMAN_SHAMAN => {
                    if ctx.has_quest_items(killer, LEIRYNNS_1ST_ORDER)
                        && ctx.get_quest_items_count(killer, DELU_TOTEM) < 10
                    {
                        ctx.give_items(killer, DELU_TOTEM, 1);
                        // NOTE(java-quirk): Java tests `RED_SPORE_DUST` here, not `DELU_TOTEM`, so the
                        // condition moves only when the spore dust of the fungus step is in.
                        if ctx.get_quest_items_count(killer, RED_SPORE_DUST) >= 10 {
                            qs.set_cond(ctx, 4, true);
                        } else {
                            ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                        }
                    }
                }
                DELU_CHIEF_KALKIS => {
                    if ctx.has_quest_items(killer, LEIRYNNS_2ND_ORDER)
                        && !ctx.has_at_least_one_quest_item(killer, &[CHIEF_KALKIS_FANG, STRINGE_MAP])
                    {
                        ctx.give_items(killer, CHIEF_KALKIS_FANG, 1);
                        ctx.give_items(killer, STRINGE_MAP, 1);
                        qs.set_cond(ctx, 6, true);
                    }
                }
                _ => {}
            }
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == MASTER_LUTHER {
                if matches!(
                    ctx.player_class_id(player),
                    player_class::ROGUE
                        | player_class::ELVEN_SCOUT
                        | player_class::ASSASSIN
                        | player_class::SCAVENGER
                ) {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        if ctx.player_class_id(player) == player_class::SCAVENGER {
                            htmltext = "30690-04.htm".to_owned();
                        } else {
                            htmltext = "30690-03.htm".to_owned();
                        }
                    } else {
                        htmltext = "30690-02.html".to_owned();
                    }
                } else {
                    htmltext = "30690-01.html".to_owned();
                }
            }
        } else if state == STARTED {
            match npc.id {
                MASTER_LUTHER => {
                    if ctx.has_quest_items(player, LUTHERS_LETTER)
                        && !ctx.has_quest_items(player, ALEXS_RECOMMEND)
                    {
                        htmltext = "30690-06.html".to_owned();
                    } else if !ctx.has_at_least_one_quest_item(player, &[LUTHERS_LETTER, ALEXS_RECOMMEND]) {
                        htmltext = "30690-07.html".to_owned();
                    } else if !ctx.has_quest_items(player, LUTHERS_LETTER)
                        && ctx.has_quest_items(player, ALEXS_RECOMMEND)
                    {
                        ctx.give_adena(player, 161_806, true);
                        ctx.give_items(player, MARK_OF_SEARCHER, 1);
                        ctx.add_exp_and_sp(player, 894_888, 61_408);
                        qs.exit_quest(ctx, false, true);
                        ctx.world().send(
                            player,
                            &SocialAction {
                                object_id: player.0,
                                action_id: 3,
                            },
                        );
                        htmltext = "30690-08.html".to_owned();
                    }
                }
                CAPTAIN_ALEX => {
                    if ctx.has_quest_items(player, LUTHERS_LETTER) {
                        ctx.take_items(player, LUTHERS_LETTER, 1);
                        ctx.give_items(player, ALEXS_WARRANT, 1);
                        qs.set_cond(ctx, 2, true);
                        htmltext = "30291-01.html".to_owned();
                    } else if ctx.has_quest_items(player, ALEXS_WARRANT) {
                        htmltext = "30291-02.html".to_owned();
                    } else if ctx
                        .has_at_least_one_quest_item(player, &[LEIRYNNS_1ST_ORDER, LEIRYNNS_2ND_ORDER])
                    {
                        htmltext = "30291-03.html".to_owned();
                    } else if ctx.has_quest_items(player, LEIRYNNS_REPORT) {
                        htmltext = "30291-04.html".to_owned();
                    } else if ctx.has_quest_items(player, ALEXS_ORDER) {
                        if ctx.has_quest_items(player, ALEXS_LETTER) {
                            htmltext = "30291-08.html".to_owned();
                        } else if ctx.has_at_least_one_quest_item(player, &[OLD_ORDER, JAXS_DIARY]) {
                            htmltext = "30291-09.html".to_owned();
                        } else if ctx.has_quest_items(player, COMBINED_MAP) {
                            if ctx.get_quest_items_count(player, GOLD_BAR) == 20 {
                                ctx.take_items(player, ALEXS_ORDER, 1);
                                ctx.take_items(player, COMBINED_MAP, 1);
                                ctx.take_items(player, GOLD_BAR, -1);
                                ctx.give_items(player, ALEXS_RECOMMEND, 1);
                                ctx.remove_radar(player, 10133, 157_155, -2383);
                                qs.set_cond(ctx, 19, true);
                                htmltext = "30291-11.html".to_owned();
                            } else {
                                htmltext = "30291-10.html".to_owned();
                            }
                        }
                    } else if ctx.has_quest_items(player, ALEXS_RECOMMEND) {
                        htmltext = "30291-12.html".to_owned();
                    }
                }
                TYRA => {
                    if ctx.has_quest_items(player, WINE_CATALOG) {
                        htmltext = "30420-01.html".to_owned();
                    } else if ctx.has_quest_items(player, TYRAS_CONTRACT) {
                        if ctx.get_quest_items_count(player, RED_SPORE_DUST) < 10 {
                            htmltext = "30420-02.html".to_owned();
                        } else {
                            ctx.take_items(player, TYRAS_CONTRACT, 1);
                            ctx.take_items(player, RED_SPORE_DUST, -1);
                            ctx.give_items(player, MALRUKIAN_WINE, 1);
                            qs.set_cond(ctx, 12, true);
                            htmltext = "30420-03.html".to_owned();
                        }
                    } else if ctx.has_at_least_one_quest_item(
                        player,
                        &[
                            JAXS_DIARY,
                            OLD_ORDER,
                            COMBINED_MAP,
                            ALEXS_RECOMMEND,
                            MALRUKIAN_WINE,
                        ],
                    ) {
                        htmltext = "30420-04.html".to_owned();
                    }
                }
                TREE => {
                    if ctx.has_quest_items(player, COMBINED_MAP) {
                        // Java: no key and no gold bar, or the key with the 20 gold bars: the same page.
                        let no_key_or_gold =
                            !ctx.has_at_least_one_quest_item(player, &[RUSTED_KEY, GOLD_BAR]);
                        let key_and_gold = ctx.has_quest_items(player, RUSTED_KEY)
                            && ctx.get_quest_items_count(player, GOLD_BAR) >= 20;
                        if no_key_or_gold || key_and_gold {
                            htmltext = "30627-01.html".to_owned();
                        }
                    }
                }
                STRONG_WOODEN_CHEST => {
                    if ctx.has_quest_items(player, RUSTED_KEY) {
                        htmltext = "30628-01.html".to_owned();
                    }
                }
                MILITIAMAN_LEIRYNN => {
                    if ctx.has_quest_items(player, ALEXS_WARRANT) {
                        ctx.take_items(player, ALEXS_WARRANT, 1);
                        ctx.give_items(player, LEIRYNNS_1ST_ORDER, 1);
                        qs.set_cond(ctx, 3, true);
                        htmltext = "30728-01.html".to_owned();
                    } else if ctx.has_quest_items(player, LEIRYNNS_1ST_ORDER) {
                        if ctx.get_quest_items_count(player, DELU_TOTEM) < 10 {
                            htmltext = "30728-02.html".to_owned();
                        } else {
                            ctx.take_items(player, LEIRYNNS_1ST_ORDER, 1);
                            ctx.take_items(player, DELU_TOTEM, -1);
                            ctx.give_items(player, LEIRYNNS_2ND_ORDER, 1);
                            qs.set_cond(ctx, 5, true);
                            htmltext = "30728-03.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, LEIRYNNS_2ND_ORDER) {
                        if !ctx.has_quest_items(player, CHIEF_KALKIS_FANG) {
                            htmltext = "30728-04.html".to_owned();
                        } else {
                            ctx.take_items(player, LEIRYNNS_2ND_ORDER, 1);
                            ctx.take_items(player, CHIEF_KALKIS_FANG, 1);
                            ctx.give_items(player, LEIRYNNS_REPORT, 1);
                            qs.set_cond(ctx, 7, true);
                            htmltext = "30728-05.html".to_owned();
                        }
                    } else if ctx.has_quest_items(player, LEIRYNNS_REPORT) {
                        htmltext = "30728-06.html".to_owned();
                    } else if ctx.has_at_least_one_quest_item(player, &[ALEXS_RECOMMEND, ALEXS_ORDER]) {
                        htmltext = "30728-07.html".to_owned();
                    }
                }
                DRUNKARD_BORYS => {
                    if ctx.has_quest_items(player, ALEXS_LETTER) {
                        ctx.take_items(player, ALEXS_LETTER, 1);
                        ctx.give_items(player, WINE_CATALOG, 1);
                        qs.set_cond(ctx, 9, true);
                        htmltext = "30729-01.html".to_owned();
                    } else if ctx.has_quest_items(player, WINE_CATALOG)
                        && !ctx.has_quest_items(player, MALRUKIAN_WINE)
                    {
                        htmltext = "30729-02.html".to_owned();
                    } else if ctx.has_quest_items(player, MALRUKIAN_WINE)
                        && !ctx.has_quest_items(player, WINE_CATALOG)
                    {
                        ctx.take_items(player, MALRUKIAN_WINE, 1);
                        ctx.give_items(player, OLD_ORDER, 1);
                        qs.set_cond(ctx, 13, true);
                        htmltext = "30729-03.html".to_owned();
                    } else if ctx.has_quest_items(player, OLD_ORDER) {
                        htmltext = "30729-04.html".to_owned();
                    } else if ctx
                        .has_at_least_one_quest_item(player, &[JAXS_DIARY, COMBINED_MAP, ALEXS_RECOMMEND])
                    {
                        htmltext = "30729-05.html".to_owned();
                    }
                }
                BODYGUARD_JAX => {
                    if ctx.has_quest_items(player, OLD_ORDER) {
                        htmltext = "30730-01.html".to_owned();
                    } else if ctx.has_quest_items(player, JAXS_DIARY) {
                        let maps = ctx.get_quest_items_count(player, SOLTS_MAP)
                            + ctx.get_quest_items_count(player, MAKELS_MAP);
                        if maps < 2 {
                            htmltext = "30730-02.html".to_owned();
                        } else if maps == 2 {
                            ctx.take_items(player, LAMBERTS_MAP, 1);
                            ctx.take_items(player, JAXS_DIARY, 1);
                            ctx.take_items(player, SOLTS_MAP, 1);
                            ctx.take_items(player, MAKELS_MAP, -1);
                            ctx.give_items(player, COMBINED_MAP, 1);
                            qs.set_cond(ctx, 16, true);
                            htmltext = "30730-03.html".to_owned();
                        }
                    } else if ctx.has_at_least_one_quest_item(player, &[COMBINED_MAP, ALEXS_RECOMMEND]) {
                        htmltext = "30730-04.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if state == COMPLETED && npc.id == MASTER_LUTHER {
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
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00225_TestOfTheSearcher";

    const NPCS: &str = r#"
        <npc id="30690" type="Folk" level="70" name="Master Luther"/>
        <npc id="30291" type="Folk" level="70" name="Captain Alex"/>
        <npc id="30420" type="Folk" level="70" name="Tyra"/>
        <npc id="30627" type="Folk" level="70" name="Tree"/>
        <npc id="30628" type="Folk" level="70" name="Strong Wooden Chest"/>
        <npc id="30728" type="Folk" level="70" name="Militiaman Leirynn"/>
        <npc id="30729" type="Folk" level="70" name="Drunkard Borys"/>
        <npc id="30730" type="Folk" level="70" name="Bodyguard Jax"/>
        <npc id="20781" type="Monster" level="40" name="Delu Lizardman Shaman"/>
        <npc id="27092" type="Monster" level="40" name="Neer Bodyguard"/>
        <npc id="27093" type="Monster" level="40" name="Delu Chief Kalkis"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="2784" type="EtcItem" name="Luther's Letter"><set name="is_questitem" val="true"/></item>
        <item id="2785" type="EtcItem" name="Alex's Warrant"><set name="is_questitem" val="true"/></item>
        <item id="2786" type="EtcItem" name="Leirynn's 1st Order"><set name="is_questitem" val="true"/></item>
        <item id="2787" type="EtcItem" name="Delu Totem"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2788" type="EtcItem" name="Leirynn's 2nd Order"><set name="is_questitem" val="true"/></item>
        <item id="2789" type="EtcItem" name="Chief Kalkis' Fang"><set name="is_questitem" val="true"/></item>
        <item id="2790" type="EtcItem" name="Leirynn's Report"><set name="is_questitem" val="true"/></item>
        <item id="2791" type="EtcItem" name="Stringe Map"><set name="is_questitem" val="true"/></item>
        <item id="2792" type="EtcItem" name="Lambert's Map"><set name="is_questitem" val="true"/></item>
        <item id="2793" type="EtcItem" name="Alex's Letter"><set name="is_questitem" val="true"/></item>
        <item id="2794" type="EtcItem" name="Alex's Order"><set name="is_questitem" val="true"/></item>
        <item id="2805" type="EtcItem" name="Combined Map"><set name="is_questitem" val="true"/></item>
        <item id="2806" type="EtcItem" name="Rusted Key"><set name="is_questitem" val="true"/></item>
        <item id="2807" type="EtcItem" name="Gold Bar"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2808" type="EtcItem" name="Alex's Recommendation"><set name="is_questitem" val="true"/></item>
        <item id="2809" type="EtcItem" name="Mark of Searcher"><set name="is_questitem" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00225TestOfTheSearcher)
            .page("30690-03.htm", &["ACCEPT"])
            .page("30690-04.htm", &["ACCEPT"])
            .plain(&[
                "30690-01.html",
                "30690-02.html",
                "30690-05a.htm",
                "30690-05.htm",
                "30690-06.html",
                "30690-07.html",
                "30690-08.html",
            ])
            .page("30291-01.html", &[])
            .page("30291-04.html", &["30291-05.html"])
            .page("30291-05.html", &["30291-06.html"])
            .page("30291-06.html", &["30291-07.html"])
            .plain(&[
                "30291-07.html",
                "30291-02.html",
                "30291-03.html",
                "30291-08.html",
                "30291-09.html",
                "30291-10.html",
                "30291-11.html",
                "30291-12.html",
            ])
            .plain(&[
                "30728-01.html",
                "30728-02.html",
                "30728-03.html",
                "30728-04.html",
                "30728-05.html",
                "30728-06.html",
                "30728-07.html",
            ])
            .page("30627-01.html", &["30627-01a.html"])
            .plain(&["30627-01a.html"])
            .page("30628-01.html", &["30628-01a.html"])
            .plain(&["30628-01a.html"])
            .npcs(&[
                MASTER_LUTHER,
                CAPTAIN_ALEX,
                TYRA,
                TREE,
                STRONG_WOODEN_CHEST,
                MILITIAMAN_LEIRYNN,
                DRUNKARD_BORYS,
                BODYGUARD_JAX,
            ])
    }

    struct Npcs {
        luther: Oid,
        alex: Oid,
        leirynn: Oid,
        tree: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00225TestOfTheSearcher);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            luther: t.spawn(MASTER_LUTHER, 20),
            alex: t.spawn(CAPTAIN_ALEX, 30),
            leirynn: t.spawn(MILITIAMAN_LEIRYNN, 40),
            tree: t.spawn(TREE, 50),
        };
        (t, npcs)
    }

    /// Gives the player items by hand (the steps the tests do not walk).
    fn give(t: &mut ScriptTest, items: &[i32], count: i64) {
        t.world.run_as_script(NAME, |ctx| {
            for &item in items {
                ctx.give_items(Oid(1), item, count);
            }
        });
    }

    #[test]
    fn only_the_scouts_of_level_39_and_the_scavengers_are_taken() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 38, race::HUMAN, player_class::ROGUE, vec![]);
        assert!(t.quest_page(1, npcs.luther).contains("30690-02.html"));
        t.enter_as(2, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(2, npcs.luther).contains("30690-01.html"));
        t.enter_as(3, 39, race::HUMAN, player_class::ROGUE, vec![]);
        assert!(t.quest_page(3, npcs.luther).contains("30690-03.htm"));
        t.enter_as(4, 39, race::HUMAN, player_class::SCAVENGER, vec![]);
        assert!(t.quest_page(4, npcs.luther).contains("30690-04.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn a_scavenger_accepts_and_gets_the_letter_and_the_diamonds() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::SCAVENGER, vec![]);
        t.quest_page(1, npcs.luther);
        assert!(t.event(1, NAME, "ACCEPT").contains("30690-05a.htm"));
        assert_eq!(t.count(1, LUTHERS_LETTER), 1);
        // A scavenger gets 82 diamonds (other classes 96).
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 82);
        assert_eq!(
            t.take_writes(),
            [
                rows::insert(NAME, "cond", "1"),
                rows::insert(NAME, "<state>", "Started"),
                rows::insert(NAME, "memoState", "1"),
            ]
        );
    }

    #[test]
    fn alex_and_leirynn_hand_out_the_first_steps() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::SCAVENGER, vec![]);
        t.quest_page(1, npcs.luther);
        t.event(1, NAME, "ACCEPT");
        t.take_writes();

        assert!(t.quest_page(1, npcs.alex).contains("30291-01.html"));
        assert_eq!((t.count(1, LUTHERS_LETTER), t.count(1, ALEXS_WARRANT)), (0, 1));
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "2")]);

        assert!(t.quest_page(1, npcs.leirynn).contains("30728-01.html"));
        assert_eq!(
            (t.count(1, ALEXS_WARRANT), t.count(1, LEIRYNNS_1ST_ORDER)),
            (0, 1)
        );
        assert_eq!(t.take_writes(), [rows::update(NAME, "cond", "3")]);
    }

    #[test]
    fn the_totem_kills_do_not_move_the_condition_java_quirk() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::SCAVENGER, vec![]);
        t.quest_page(1, npcs.luther);
        t.event(1, NAME, "ACCEPT");
        t.quest_page(1, npcs.alex);
        t.quest_page(1, npcs.leirynn);
        t.take_writes();

        for _ in 0..10 {
            let shaman = t.spawn(DELU_LIZARDMAN_SHAMAN, 60);
            t.kill(1, shaman);
            t.world.delete_npc(shaman);
        }
        assert_eq!(t.count(1, DELU_TOTEM), 10);
        // Java checks `RED_SPORE_DUST` for the condition: no spore dust yet, so no write at all.
        assert!(t.take_writes().is_empty());

        // Leirynn takes the ten totems and gives the second order (cond 5).
        assert!(t.quest_page(1, npcs.leirynn).contains("30728-03.html"));
        assert_eq!((t.count(1, DELU_TOTEM), t.count(1, LEIRYNNS_2ND_ORDER)), (0, 1));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "5"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483625"),
            ]
        );
    }

    #[test]
    fn kalkis_fang_and_the_report_lead_to_alex_and_the_map() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::SCAVENGER, vec![]);
        t.quest_page(1, npcs.luther);
        t.event(1, NAME, "ACCEPT");
        // The second order is in hand; the letter is taken by hand so that Alex skips his first step.
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), LEIRYNNS_2ND_ORDER, 1);
            ctx.take_items(Oid(1), LUTHERS_LETTER, -1);
        });
        t.take_writes();

        // Kalkis drops the fang and the stringe map (cond 6).
        let kalkis = t.spawn(DELU_CHIEF_KALKIS, 70);
        t.kill(1, kalkis);
        t.world.delete_npc(kalkis);
        assert_eq!((t.count(1, CHIEF_KALKIS_FANG), t.count(1, STRINGE_MAP)), (1, 1));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "6"),
                rows::insert(NAME, "__compltdStateFlags", "-2147483615"),
            ]
        );

        // Leirynn trades the order and the fang for the report (cond 7).
        assert!(t.quest_page(1, npcs.leirynn).contains("30728-05.html"));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "7"),
                rows::update(NAME, "__compltdStateFlags", "-2147483551"),
            ]
        );

        // Alex: the report and the stringe map give the lambert map, the letter and the order (cond 8).
        assert!(t.quest_page(1, npcs.alex).contains("30291-04.html"));
        assert!(t.event(1, NAME, "30291-05.html").contains("30291-05.html"));
        assert!(t.event(1, NAME, "30291-06.html").contains("30291-06.html"));
        assert!(t.event(1, NAME, "30291-07.html").contains("30291-07.html"));
        assert_eq!(t.count(1, LAMBERTS_MAP), 1);
        assert_eq!(t.count(1, ALEXS_ORDER), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "8"),
                rows::update(NAME, "__compltdStateFlags", "-2147483423"),
            ]
        );
    }

    #[test]
    fn the_tree_drops_the_key_and_the_chest_gives_the_gold() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::SCAVENGER, vec![]);
        t.quest_page(1, npcs.luther);
        t.event(1, NAME, "ACCEPT");
        give(&mut t, &[COMBINED_MAP], 1);
        t.take_writes();

        assert!(t.quest_page(1, npcs.tree).contains("30627-01.html"));
        assert!(t.event(1, NAME, "30627-01a.html").contains("30627-01a.html"));
        assert_eq!(t.count(1, RUSTED_KEY), 1);
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "17"),
                rows::insert(NAME, "__compltdStateFlags", "-2147418111"),
            ]
        );
        let chests = t.npcs_of(STRONG_WOODEN_CHEST);
        assert_eq!(chests.len(), 1);
        // The tree summoned it (`addSpawn(npc, ..)`): it counts in `getSummonedNpcCount`.
        assert_eq!(t.world.summoned_npc_count(npcs.tree), 1);
        assert_eq!(t.world.summoner_of(chests[0]), Some(npcs.tree));

        // The chest opens with the key: 20 gold bars, cond 18, the chest is gone.
        let chest = chests[0];
        assert!(t.quest_page(1, chest).contains("30628-01.html"));
        assert!(t.event(1, NAME, "30628-01a.html").contains("30628-01a.html"));
        assert_eq!((t.count(1, RUSTED_KEY), t.count(1, GOLD_BAR)), (0, 20));
        assert!(!t.in_world(chest));
        assert_eq!(
            t.take_writes(),
            [
                rows::update(NAME, "cond", "18"),
                rows::update(NAME, "__compltdStateFlags", "-2147287039"),
            ]
        );
    }

    #[test]
    fn the_bodyguard_comes_out_of_a_shaman_hit_by_a_holder_of_the_first_order() {
        let (mut t, _npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::SCAVENGER, vec![]);
        give(&mut t, &[LEIRYNNS_1ST_ORDER], 1);
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), true) {
                qs.start_quest(ctx);
            }
        });
        let shaman = t.spawn(DELU_LIZARDMAN_SHAMAN, 60);
        t.take_writes();
        // The world does not fire `on_attack` for this test: the hook is called directly.
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(shaman) {
                Q00225TestOfTheSearcher.on_attack(ctx, npc, Oid(1), 1, false, None);
            }
        });
        assert!(t.world.is_npc_script_value(shaman, 1));
        assert_eq!(t.npcs_of(NEER_BODYGUARD).len(), 1);
        // A second hit does nothing: the script value is 1 already.
        t.world.run_as_script(NAME, |ctx| {
            if let Some(npc) = ctx.script_npc(shaman) {
                Q00225TestOfTheSearcher.on_attack(ctx, npc, Oid(1), 1, false, None);
            }
        });
        assert_eq!(t.npcs_of(NEER_BODYGUARD).len(), 1);
    }

    #[test]
    fn the_reward_is_the_mark_once_alex_recommends() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::SCAVENGER, vec![]);
        t.quest_page(1, npcs.luther);
        t.event(1, NAME, "ACCEPT");
        // The order, the map and the gold are in hand; the letter is gone (given back by hand).
        give(&mut t, &[ALEXS_ORDER, COMBINED_MAP], 1);
        give(&mut t, &[GOLD_BAR], 20);
        t.world.run_as_script(NAME, |ctx| {
            ctx.take_items(Oid(1), LUTHERS_LETTER, -1);
        });
        t.take_writes();
        t.take();

        assert!(t.quest_page(1, npcs.alex).contains("30291-11.html"));
        assert_eq!((t.count(1, ALEXS_RECOMMEND), t.count(1, GOLD_BAR)), (1, 0));
        t.take_writes();

        t.talk(1, npcs.luther);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30690-08.html"));
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_SEARCHER), 1);
        assert_eq!(t.count(1, 57), 161_806);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.luther)
                .contains("This quest has already been completed.")
        );
    }
}
