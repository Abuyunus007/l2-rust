//! Quest 216: Trial of the Guildsman (Valkon, Norman, Altran, Pinter and Duning; the dwarf's mark).
// Java: dist/game/data/scripts/quests/Q00216_TrialOfTheGuildsman/Q00216_TrialOfTheGuildsman.java

use l2_util::ObjectId;
use l2_world::script::{QuestSound, player_class};
use l2_world::{Script, ScriptCtx, ScriptNpc, ScriptSetup};

// NPCs
const WAREHOUSE_KEEPER_VALKON: i32 = 30103;
const WAREHOUSE_KEEPER_NORMAN: i32 = 30210;
const BLACKSMITH_ALTRAN: i32 = 30283;
const BLACKSMITH_PINTER: i32 = 30298;
const BLACKSMITH_DUNING: i32 = 30688;

// Items
const RECIPE_JOURNEYMAN_RING: i32 = 3024;
const RECIPE_AMBER_BEAD: i32 = 3025;
const VALKONS_RECOMMENDATION: i32 = 3120;
const MANDRAGORA_BERRY: i32 = 3121;
const ALLTRANS_INSTRUCTIONS: i32 = 3122;
const ALLTRANS_1ST_RECOMMENDATION: i32 = 3123;
const ALLTRANS_2ND_RECOMMENDATION: i32 = 3124;
const NORMANS_INSTRUCTIONS: i32 = 3125;
const NORMANS_RECEIPT: i32 = 3126;
const DUNINGS_INSTRUCTIONS: i32 = 3127;
const DUNINGS_KEY: i32 = 3128;
const NORMANS_LIST: i32 = 3129;
const GRAY_BONE_POWDER: i32 = 3130;
const GRANITE_WHETSTONE: i32 = 3131;
const RED_PIGMENT: i32 = 3132;
const BRAIDED_YARN: i32 = 3133;
const JOURNEYMAN_GEM: i32 = 3134;
const PINTERS_INSTRUCTIONS: i32 = 3135;
const AMBER_BEAD: i32 = 3136;
const AMBER_LUMP: i32 = 3137;
const JOURNEYMAN_DECO_BEADS: i32 = 3138;
const JOURNEYMAN_RING: i32 = 3139;

// Reward
const MARK_OF_GUILDSMAN: i32 = 3119;
const DIMENSIONAL_DIAMOND: i32 = 7562;
/// Java `Inventory.ADENA_ID`.
const ADENA: i32 = 57;

// Monsters
const ANT: i32 = 20079;
const ANT_CAPTAIN: i32 = 20080;
const ANT_OVERSEER: i32 = 20081;
const GRANITE_GOLEM: i32 = 20083;
const MANDRAGORA_SPROUT1: i32 = 20154;
const MANDRAGORA_SAPLONG: i32 = 20155;
const MANDRAGORA_BLOSSOM: i32 = 20156;
const SILENOS: i32 = 20168;
const STRAIN: i32 = 20200;
const GHOUL: i32 = 20201;
const DEAD_SEEKER: i32 = 20202;
const MANDRAGORA_SPROUT2: i32 = 20223;
const BREKA_ORC: i32 = 20267;
const BREKA_ORC_ARCHER: i32 = 20268;
const BREKA_ORC_SHAMAN: i32 = 20269;
const BREKA_ORC_OVERLORD: i32 = 20270;
const BREKA_ORC_WARRIOR: i32 = 20271;

// Misc
const MIN_LEVEL: i32 = 35;
/// Java `player.getVariables()` key: the second-class diamonds are given once per character.
const DIAMOND_REWARD_VAR: &str = "2ND_CLASS_DIAMOND_REWARD";

/// Java `Q00216_TrialOfTheGuildsman`.
pub struct Q00216TrialOfTheGuildsman;

/// Java `hasQuestItems(player, a, b, ...)`: all of them.
fn all(ctx: &ScriptCtx<'_>, player: ObjectId, items: &[i32]) -> bool {
    ctx.has_all_quest_items(player, items)
}

fn has(ctx: &ScriptCtx<'_>, player: ObjectId, item: i32) -> bool {
    ctx.has_quest_items(player, item)
}

fn count(ctx: &ScriptCtx<'_>, player: ObjectId, item: i32) -> i64 {
    ctx.get_quest_items_count(player, item)
}

/// The "give n, then the middle sound at exactly 70, else the item sound" kills of the four
/// ingredient monsters (`GRANITE_WHETSTONE`, `BRAIDED_YARN`, `GRAY_BONE_POWDER`, `RED_PIGMENT`).
fn ingredient_kill(ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, item: i32, amount: i64) {
    let Some(qs) = ctx.random_party_member_state(killer, -1, 2, Some(npc)) else {
        return;
    };
    let member = qs.player();
    ctx.give_items(member, item, amount);
    if count(ctx, member, item) == 70 {
        ctx.play_sound(member, QuestSound::ItemsoundQuestMiddle);
    } else {
        ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
    }
}

impl Script for Q00216TrialOfTheGuildsman {
    fn name(&self) -> &'static str {
        "Q00216_TrialOfTheGuildsman"
    }

    fn path(&self) -> &'static str {
        "quests/Q00216_TrialOfTheGuildsman"
    }

    fn id(&self) -> i32 {
        216
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[WAREHOUSE_KEEPER_VALKON]);
        s.add_talk_id(&[
            WAREHOUSE_KEEPER_VALKON,
            WAREHOUSE_KEEPER_NORMAN,
            BLACKSMITH_ALTRAN,
            BLACKSMITH_PINTER,
            BLACKSMITH_DUNING,
        ]);
        s.add_kill_id(&[
            ANT,
            ANT_CAPTAIN,
            ANT_OVERSEER,
            GRANITE_GOLEM,
            MANDRAGORA_SPROUT1,
            MANDRAGORA_SAPLONG,
            MANDRAGORA_BLOSSOM,
            SILENOS,
            STRAIN,
            GHOUL,
            DEAD_SEEKER,
            MANDRAGORA_SPROUT2,
            BREKA_ORC,
            BREKA_ORC_ARCHER,
            BREKA_ORC_SHAMAN,
            BREKA_ORC_OVERLORD,
            BREKA_ORC_WARRIOR,
        ]);
        s.register_quest_items(&[
            RECIPE_JOURNEYMAN_RING,
            RECIPE_AMBER_BEAD,
            VALKONS_RECOMMENDATION,
            MANDRAGORA_BERRY,
            ALLTRANS_INSTRUCTIONS,
            ALLTRANS_1ST_RECOMMENDATION,
            ALLTRANS_2ND_RECOMMENDATION,
            NORMANS_INSTRUCTIONS,
            NORMANS_RECEIPT,
            DUNINGS_INSTRUCTIONS,
            DUNINGS_KEY,
            NORMANS_LIST,
            GRAY_BONE_POWDER,
            GRANITE_WHETSTONE,
            RED_PIGMENT,
            BRAIDED_YARN,
            JOURNEYMAN_GEM,
            PINTERS_INSTRUCTIONS,
            AMBER_BEAD,
            AMBER_LUMP,
            JOURNEYMAN_DECO_BEADS,
            JOURNEYMAN_RING,
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
                if count(ctx, player, ADENA) >= 2000 {
                    qs.start_quest(ctx);
                    ctx.take_items(player, ADENA, 2000);
                    if !has(ctx, player, VALKONS_RECOMMENDATION) {
                        ctx.give_items(player, VALKONS_RECOMMENDATION, 1);
                    }
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    if ctx.player_variable_int(player, DIAMOND_REWARD_VAR, 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 85);
                        ctx.set_player_variable(player, DIAMOND_REWARD_VAR, "1");
                        htmltext = Some("30103-06d.htm".to_owned());
                    } else {
                        htmltext = Some("30103-06.htm".to_owned());
                    }
                } else {
                    htmltext = Some("30103-05b.htm".to_owned());
                }
            }
            "30103-04.htm" | "30103-05.htm" | "30103-05a.html" | "30103-06a.html" | "30103-06b.html"
            | "30103-06c.html" | "30103-07a.html" | "30103-07b.html" | "30103-07c.html" | "30210-02.html"
            | "30210-03.html" | "30210-08.html" | "30210-09.html" | "30210-11a.html" | "30283-03a.html"
            | "30283-03b.html" | "30283-04.html" | "30298-03.html" | "30298-05a.html" => {
                htmltext = Some(event.to_owned());
            }
            "30103-09a.html" => {
                if has(ctx, player, ALLTRANS_INSTRUCTIONS) && count(ctx, player, JOURNEYMAN_RING) >= 7 {
                    ctx.give_adena(player, 187_606, true);
                    ctx.give_items(player, MARK_OF_GUILDSMAN, 1);
                    ctx.add_exp_and_sp(player, 1_029_478, 66_768);
                    qs.exit_quest(ctx, false, true);
                    ctx.send_social_action(player, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "30103-09b.html" => {
                if has(ctx, player, ALLTRANS_INSTRUCTIONS) && count(ctx, player, JOURNEYMAN_RING) >= 7 {
                    ctx.give_adena(player, 93_803, true);
                    ctx.give_items(player, MARK_OF_GUILDSMAN, 1);
                    ctx.add_exp_and_sp(player, 514_739, 33_384);
                    qs.exit_quest(ctx, false, true);
                    ctx.send_social_action(player, 3);
                    htmltext = Some(event.to_owned());
                }
            }
            "30210-04.html" => {
                if has(ctx, player, ALLTRANS_1ST_RECOMMENDATION) {
                    ctx.take_items(player, ALLTRANS_1ST_RECOMMENDATION, 1);
                    ctx.give_items(player, NORMANS_INSTRUCTIONS, 1);
                    ctx.give_items(player, NORMANS_RECEIPT, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30210-10.html" => {
                if has(ctx, player, NORMANS_INSTRUCTIONS) {
                    ctx.take_items(player, NORMANS_INSTRUCTIONS, 1);
                    ctx.take_items(player, DUNINGS_KEY, -1);
                    ctx.give_items(player, NORMANS_LIST, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            "30283-03.html" => {
                if all(ctx, player, &[VALKONS_RECOMMENDATION, MANDRAGORA_BERRY]) {
                    ctx.give_items(player, RECIPE_JOURNEYMAN_RING, 1);
                    ctx.take_items(player, VALKONS_RECOMMENDATION, 1);
                    ctx.take_items(player, MANDRAGORA_BERRY, 1);
                    ctx.give_items(player, ALLTRANS_INSTRUCTIONS, 1);
                    ctx.give_items(player, ALLTRANS_1ST_RECOMMENDATION, 1);
                    ctx.give_items(player, ALLTRANS_2ND_RECOMMENDATION, 1);
                    qs.set_cond(ctx, 5, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30298-04.html" => {
                if ctx.player_class_id(player) == player_class::SCAVENGER {
                    if has(ctx, player, ALLTRANS_2ND_RECOMMENDATION) {
                        ctx.take_items(player, ALLTRANS_2ND_RECOMMENDATION, 1);
                        ctx.give_items(player, PINTERS_INSTRUCTIONS, 1);
                        htmltext = Some(event.to_owned());
                    }
                } else if has(ctx, player, ALLTRANS_2ND_RECOMMENDATION) {
                    ctx.give_items(player, RECIPE_AMBER_BEAD, 1);
                    ctx.take_items(player, ALLTRANS_2ND_RECOMMENDATION, 1);
                    ctx.give_items(player, PINTERS_INSTRUCTIONS, 1);
                    htmltext = Some("30298-05.html".to_owned());
                }
            }
            "30688-02.html" => {
                if has(ctx, player, NORMANS_RECEIPT) {
                    ctx.take_items(player, NORMANS_RECEIPT, 1);
                    ctx.give_items(player, DUNINGS_INSTRUCTIONS, 1);
                    htmltext = Some(event.to_owned());
                }
            }
            _ => {}
        }
        htmltext
    }

    fn on_kill(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, killer: ObjectId, _is_summon: bool) {
        match npc.id {
            ANT | ANT_CAPTAIN | ANT_OVERSEER => {
                // NOTE(java-quirk): `checkPartyMember(Player, Npc)` below is the filter of
                // `getRandomPartyMember`, which this quest never calls; `getRandomPartyMemberState`
                // uses `checkPartyMember(QuestState, Npc)` (always true), so every started member
                // can get the drops, whatever he carries.
                let Some(qs) = ctx.random_party_member_state(killer, -1, 2, Some(npc)) else {
                    return;
                };
                let member = qs.player();
                let mut count_ = 0_i64;
                if ctx.player_class_id(member) == player_class::SCAVENGER
                    && ctx.world().is_sweep_active(npc.object_id)
                {
                    count_ += 5;
                }
                if ctx.get_random_boolean() && ctx.player_class_id(member) == player_class::ARTISAN {
                    ctx.give_items(member, AMBER_LUMP, 1);
                    ctx.play_sound(member, QuestSound::ItemsoundQuestMiddle);
                }
                if count(ctx, member, AMBER_BEAD) + count_ < 70 {
                    count_ += 5;
                }
                if count_ > 0 {
                    ctx.give_item_randomly(member, Some(npc), AMBER_BEAD, count_, count_, 70, 1.0, true);
                }
            }
            GRANITE_GOLEM => ingredient_kill(ctx, npc, killer, GRANITE_WHETSTONE, 7),
            MANDRAGORA_SPROUT1 | MANDRAGORA_SAPLONG | MANDRAGORA_BLOSSOM | MANDRAGORA_SPROUT2 => {
                let Some(qs) = ctx.get_quest_state(killer, false) else {
                    return;
                };
                if qs.is_started(ctx)
                    && ctx.check_if_in_range(ctx.alt_party_range(), npc.object_id, killer, true)
                    && has(ctx, killer, VALKONS_RECOMMENDATION)
                    && !has(ctx, killer, MANDRAGORA_BERRY)
                {
                    ctx.give_items(killer, MANDRAGORA_BERRY, 1);
                    qs.set_cond(ctx, 4, true);
                }
            }
            SILENOS => ingredient_kill(ctx, npc, killer, BRAIDED_YARN, 10),
            STRAIN | GHOUL => ingredient_kill(ctx, npc, killer, GRAY_BONE_POWDER, 5),
            DEAD_SEEKER => ingredient_kill(ctx, npc, killer, RED_PIGMENT, 7),
            BREKA_ORC | BREKA_ORC_ARCHER | BREKA_ORC_SHAMAN | BREKA_ORC_OVERLORD | BREKA_ORC_WARRIOR => {
                let Some(qs) = ctx.random_party_member_state(killer, -1, 2, Some(npc)) else {
                    return;
                };
                let member = qs.player();
                if count(ctx, member, DUNINGS_KEY) >= 29 {
                    ctx.give_items(member, DUNINGS_KEY, 1);
                    ctx.take_items(member, DUNINGS_INSTRUCTIONS, 1);
                    ctx.play_sound(member, QuestSound::ItemsoundQuestMiddle);
                } else {
                    ctx.give_items(member, DUNINGS_KEY, 1);
                    ctx.play_sound(member, QuestSound::ItemsoundQuestItemget);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let mut htmltext = ctx.get_no_quest_msg(player);
        if qs.is_created(ctx) {
            if npc.id == WAREHOUSE_KEEPER_VALKON {
                let class = ctx.player_class_id(player);
                if class == player_class::ARTISAN || class == player_class::SCAVENGER {
                    if ctx.player_level(player) < MIN_LEVEL {
                        htmltext = "30103-02.html".to_owned();
                    } else {
                        htmltext = "30103-03.htm".to_owned();
                    }
                } else {
                    htmltext = "30103-01.html".to_owned();
                }
            }
        } else if qs.is_started(ctx) {
            match npc.id {
                WAREHOUSE_KEEPER_VALKON => {
                    if has(ctx, player, VALKONS_RECOMMENDATION) {
                        qs.set_cond(ctx, 3, true);
                        htmltext = "30103-07.html".to_owned();
                    } else if has(ctx, player, ALLTRANS_INSTRUCTIONS) {
                        if count(ctx, player, JOURNEYMAN_RING) < 7 {
                            htmltext = "30103-08.html".to_owned();
                        } else {
                            htmltext = "30103-09.html".to_owned();
                        }
                    }
                }
                WAREHOUSE_KEEPER_NORMAN => {
                    if has(ctx, player, ALLTRANS_INSTRUCTIONS) {
                        if has(ctx, player, ALLTRANS_1ST_RECOMMENDATION) {
                            htmltext = "30210-01.html".to_owned();
                        } else if all(ctx, player, &[NORMANS_INSTRUCTIONS, NORMANS_RECEIPT]) {
                            htmltext = "30210-05.html".to_owned();
                        } else if all(ctx, player, &[NORMANS_INSTRUCTIONS, DUNINGS_INSTRUCTIONS]) {
                            htmltext = "30210-06.html".to_owned();
                        } else if has(ctx, player, NORMANS_INSTRUCTIONS)
                            && count(ctx, player, DUNINGS_KEY) >= 30
                        {
                            htmltext = "30210-07.html".to_owned();
                        } else if has(ctx, player, NORMANS_LIST) {
                            if count(ctx, player, GRAY_BONE_POWDER) >= 70
                                && count(ctx, player, GRANITE_WHETSTONE) >= 70
                                && count(ctx, player, RED_PIGMENT) >= 70
                                && count(ctx, player, BRAIDED_YARN) >= 70
                            {
                                ctx.take_items(player, NORMANS_LIST, 1);
                                ctx.take_items(player, GRAY_BONE_POWDER, -1);
                                ctx.take_items(player, GRANITE_WHETSTONE, -1);
                                ctx.take_items(player, RED_PIGMENT, -1);
                                ctx.take_items(player, BRAIDED_YARN, -1);
                                ctx.give_items(player, JOURNEYMAN_GEM, 7);
                                if count(ctx, player, JOURNEYMAN_DECO_BEADS) >= 7 {
                                    qs.set_cond(ctx, 6, true);
                                }
                                htmltext = "30210-12.html".to_owned();
                            } else {
                                htmltext = "30210-11.html".to_owned();
                            }
                        } else if !ctx
                            .has_at_least_one_quest_item(player, &[NORMANS_INSTRUCTIONS, NORMANS_LIST])
                            && ctx.has_at_least_one_quest_item(player, &[JOURNEYMAN_GEM, JOURNEYMAN_RING])
                        {
                            htmltext = "30210-13.html".to_owned();
                        }
                    }
                }
                BLACKSMITH_ALTRAN => {
                    if has(ctx, player, VALKONS_RECOMMENDATION) {
                        if !has(ctx, player, MANDRAGORA_BERRY) {
                            qs.set_cond(ctx, 2, true);
                            htmltext = "30283-01.html".to_owned();
                        } else {
                            htmltext = "30283-02.html".to_owned();
                        }
                    } else if has(ctx, player, ALLTRANS_INSTRUCTIONS) {
                        if count(ctx, player, JOURNEYMAN_RING) < 7 {
                            htmltext = "30283-04.html".to_owned();
                        } else {
                            htmltext = "30283-05.html".to_owned();
                        }
                    }
                }
                BLACKSMITH_PINTER => {
                    if has(ctx, player, ALLTRANS_INSTRUCTIONS) {
                        if has(ctx, player, ALLTRANS_2ND_RECOMMENDATION) {
                            htmltext = "30298-02.html".to_owned();
                        } else if has(ctx, player, PINTERS_INSTRUCTIONS) {
                            if count(ctx, player, AMBER_BEAD) < 70 {
                                htmltext = "30298-06.html".to_owned();
                            } else {
                                ctx.take_items(player, RECIPE_AMBER_BEAD, 1);
                                ctx.take_items(player, PINTERS_INSTRUCTIONS, 1);
                                ctx.take_items(player, AMBER_BEAD, -1);
                                ctx.take_items(player, AMBER_LUMP, -1);
                                ctx.give_items(player, JOURNEYMAN_DECO_BEADS, 7);
                                if count(ctx, player, JOURNEYMAN_GEM) >= 7 {
                                    qs.set_cond(ctx, 6, true);
                                }
                                htmltext = "30298-07.html".to_owned();
                            }
                        } else if !has(ctx, player, PINTERS_INSTRUCTIONS)
                            && ctx.has_at_least_one_quest_item(
                                player,
                                &[JOURNEYMAN_DECO_BEADS, JOURNEYMAN_RING],
                            )
                        {
                            htmltext = "30298-08.html".to_owned();
                        }
                    }
                }
                BLACKSMITH_DUNING => {
                    if all(ctx, player, &[ALLTRANS_INSTRUCTIONS, NORMANS_INSTRUCTIONS]) {
                        if has(ctx, player, NORMANS_RECEIPT) && !has(ctx, player, DUNINGS_INSTRUCTIONS) {
                            htmltext = "30688-01.html".to_owned();
                        }

                        if has(ctx, player, DUNINGS_INSTRUCTIONS)
                            && !has(ctx, player, NORMANS_RECEIPT)
                            && count(ctx, player, DUNINGS_KEY) < 30
                        {
                            htmltext = "30688-03.html".to_owned();
                        } else if count(ctx, player, DUNINGS_KEY) >= 30
                            && !has(ctx, player, DUNINGS_INSTRUCTIONS)
                        {
                            htmltext = "30688-04.html".to_owned();
                        }
                    } else if has(ctx, player, ALLTRANS_INSTRUCTIONS)
                        && !ctx.has_at_least_one_quest_item(
                            player,
                            &[NORMANS_INSTRUCTIONS, DUNINGS_INSTRUCTIONS],
                        )
                    {
                        htmltext = "30688-05.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if qs.is_completed(ctx) && npc.id == WAREHOUSE_KEEPER_VALKON {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }

    /// Java `checkPartyMember(player, npc)`.
    // NOTE(java-quirk): dead code in Java too, see `on_kill`; ported for the record.
    fn check_party_member(&self, ctx: &mut ScriptCtx<'_>, player: ObjectId, npc: Option<ScriptNpc>) -> bool {
        let Some(npc) = npc else {
            return false;
        };
        match npc.id {
            ANT | ANT_CAPTAIN | ANT_OVERSEER => {
                all(ctx, player, &[ALLTRANS_INSTRUCTIONS, PINTERS_INSTRUCTIONS])
                    && count(ctx, player, AMBER_BEAD) < 70
            }
            GRANITE_GOLEM => {
                all(ctx, player, &[ALLTRANS_INSTRUCTIONS, NORMANS_LIST])
                    && count(ctx, player, GRANITE_WHETSTONE) < 70
            }
            SILENOS => {
                all(ctx, player, &[ALLTRANS_INSTRUCTIONS, NORMANS_LIST])
                    && count(ctx, player, BRAIDED_YARN) < 70
            }
            STRAIN | GHOUL => {
                all(ctx, player, &[ALLTRANS_INSTRUCTIONS, NORMANS_LIST])
                    && count(ctx, player, GRAY_BONE_POWDER) < 70
            }
            DEAD_SEEKER => {
                all(ctx, player, &[ALLTRANS_INSTRUCTIONS, NORMANS_LIST])
                    && count(ctx, player, RED_PIGMENT) < 70
            }
            BREKA_ORC | BREKA_ORC_ARCHER | BREKA_ORC_SHAMAN | BREKA_ORC_OVERLORD | BREKA_ORC_WARRIOR => {
                all(
                    ctx,
                    player,
                    &[ALLTRANS_INSTRUCTIONS, NORMANS_INSTRUCTIONS, DUNINGS_INSTRUCTIONS],
                ) && count(ctx, player, DUNINGS_KEY) < 30
            }
            _ => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::race;

    use super::*;
    use crate::testing::{Pages, ScriptTest, item, rows};

    const NAME: &str = "Q00216_TrialOfTheGuildsman";

    const NPCS: &str = r#"
        <npc id="30103" type="Folk" level="70" name="Valkon"/>
        <npc id="30210" type="Folk" level="70" name="Norman"/>
        <npc id="30283" type="Folk" level="70" name="Altran"/>
        <npc id="20154" type="Monster" level="30" name="Mandragora Sprout"/>
        <npc id="20083" type="Monster" level="30" name="Granite Golem"/>"#;

    fn items() -> String {
        let mut s = String::from(
            r#"<item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="3119" type="EtcItem" name="Mark of Guildsman"><set name="is_questitem" val="true"/></item>"#,
        );
        for id in 3024..=3025 {
            s.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Q216 {id}"><set name="is_questitem" val="true"/></item>"#
            ));
        }
        for id in 3120..=3139 {
            let stack = if matches!(id, 3128 | 3130..=3133 | 3136..=3139) {
                r#"<set name="is_stackable" val="true"/>"#
            } else {
                ""
            };
            s.push_str(&format!(
                r#"<item id="{id}" type="EtcItem" name="Q216 {id}"><set name="is_questitem" val="true"/>{stack}</item>"#
            ));
        }
        s
    }

    const EVENTS: [&str; 5] = [
        "ACCEPT",
        "30283-03.html",
        "30103-09a.html",
        "30103-09b.html",
        "30210-04.html",
    ];
    const FILES: [&str; 8] = [
        "30103-01.html",
        "30103-02.html",
        "30103-03.htm",
        "30103-06d.htm",
        "30103-05b.htm",
        "30103-09.html",
        "30283-01.html",
        "30283-02.html",
    ];

    struct Npcs {
        valkon: Oid,
        altran: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00216TrialOfTheGuildsman);
        let mut pages = Pages::of(&Q00216TrialOfTheGuildsman);
        for file in FILES {
            pages = pages.page(file, &EVENTS);
        }
        for event in EVENTS.iter().filter(|e| **e != "ACCEPT") {
            pages = pages.page(event, &EVENTS);
        }
        let pages = pages.npcs(&[
            WAREHOUSE_KEEPER_VALKON,
            WAREHOUSE_KEEPER_NORMAN,
            BLACKSMITH_ALTRAN,
        ]);
        let mut t = ScriptTest::with_pages(registry, NPCS, &items(), pages);
        let npcs = Npcs {
            valkon: t.spawn(WAREHOUSE_KEEPER_VALKON, 20),
            altran: t.spawn(BLACKSMITH_ALTRAN, 40),
        };
        (t, npcs)
    }

    #[test]
    fn only_an_artisan_or_scavenger_of_level_35_is_offered_the_quest() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 35, race::DWARF, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.valkon).contains("30103-01.html"));
        t.enter_as(2, 34, race::DWARF, player_class::ARTISAN, vec![]);
        assert!(t.quest_page(2, npcs.valkon).contains("30103-02.html"));
        t.enter_as(3, 35, race::DWARF, player_class::SCAVENGER, vec![]);
        assert!(t.quest_page(3, npcs.valkon).contains("30103-03.htm"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn two_thousand_adena_start_it_and_the_mandragora_berry_goes_to_altran() {
        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            35,
            race::DWARF,
            player_class::ARTISAN,
            vec![item(900, 57, 1999)],
        );
        assert!(t.quest_page(1, npcs.valkon).contains("30103-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30103-05b.htm"));
        assert!(t.take_writes().is_empty());
        t.world.run_as_script(NAME, |ctx| ctx.give_items(Oid(1), 57, 1));
        t.take_writes();
        assert!(t.quest_page(1, npcs.valkon).contains("30103-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30103-06d.htm"));
        assert_eq!(t.count(1, 57), 0);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 85);
        assert_eq!(t.count(1, VALKONS_RECOMMENDATION), 1);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::insert(NAME, "cond", "1")));

        // Valkon: cond 3 on the recommendation; Altran: cond 2 (java order) then the berry.
        assert!(t.quest_page(1, npcs.altran).contains("30283-01.html"));
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "2")));
        let sprout = t.spawn(MANDRAGORA_SPROUT1, 60);
        t.kill(1, sprout);
        t.world.delete_npc(sprout);
        assert_eq!(t.count(1, MANDRAGORA_BERRY), 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "4")));
        assert!(t.quest_page(1, npcs.altran).contains("30283-02.html"));
        assert!(t.event(1, NAME, "30283-03.html").contains("30283-03.html"));
        assert_eq!(t.count(1, ALLTRANS_INSTRUCTIONS), 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "cond", "5")));
    }

    #[test]
    fn seven_journeyman_rings_pay_the_full_reward() {
        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            35,
            race::DWARF,
            player_class::ARTISAN,
            vec![item(900, 57, 2000)],
        );
        assert!(t.quest_page(1, npcs.valkon).contains("30103-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30103-06d.htm"));
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), ALLTRANS_INSTRUCTIONS, 1);
            ctx.give_items(Oid(1), JOURNEYMAN_RING, 7);
            ctx.take_items(Oid(1), VALKONS_RECOMMENDATION, -1);
        });
        t.take_writes();
        t.take();
        assert!(t.quest_page(1, npcs.valkon).contains("30103-09.html"));
        // The page has no "30103-09a.html" file in the stand-in set: the reward is what counts.
        t.event(1, NAME, "30103-09a.html");
        assert_eq!(t.count(1, MARK_OF_GUILDSMAN), 1);
        assert_eq!(t.count(1, 57), 187_606);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed")
            ]
        );
    }

    #[test]
    fn the_granite_golem_gives_seven_whetstones_to_a_started_player() {
        let (mut t, npcs) = setup();
        t.enter_as(
            1,
            35,
            race::DWARF,
            player_class::ARTISAN,
            vec![item(900, 57, 2000)],
        );
        assert!(t.quest_page(1, npcs.valkon).contains("30103-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30103-06d.htm"));
        t.take_writes();
        let golem = t.spawn(GRANITE_GOLEM, 60);
        t.kill(1, golem);
        t.world.delete_npc(golem);
        assert_eq!(t.count(1, GRANITE_WHETSTONE), 7);
    }
}
