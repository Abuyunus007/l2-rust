//! Quest 227: Test of the Reformer (Priestess Pupina in Talking Island: the Cleric and Shillien
//! Oracle second class change, through Preacher Sla, Ramus, Katari, Kakan and Nyakuri).
// Java: dist/game/data/scripts/quests/Q00227_TestOfTheReformer/Q00227_TestOfTheReformer.java
//
// Pattern: the test chain of the "Test of ..." quests (see Q00222) with pilgrims that a monster
// chases (`addDamageHate` + `setIntentionAttack`), spawned NPCs that despawn themselves on a
// repeating timer (`onSpawn`) and a monster that dies when it is not hit by the right skill.

use l2_protocol_game::NpcStringId;
use l2_protocol_game::server::ChatType;
use l2_util::ObjectId;
use l2_world::script::{COMPLETED, CREATED, STARTED, player_class};
use l2_world::{QuestSound, Script, ScriptCtx, ScriptNpc, ScriptSetup, SkillKey};

// NPCs
const PRIESTESS_PUPINA: i32 = 30118;
const PREACHER_SLA: i32 = 30666;
const RAMUS: i32 = 30667;
const KATARI: i32 = 30668;
const KAKAN: i32 = 30669;
const NYAKURI: i32 = 30670;
const OL_MAHUM_PILGRIM: i32 = 30732;

// Items
const BOOK_OF_REFORM: i32 = 2822;
const LETTER_OF_INTRODUCTION: i32 = 2823;
const SLAS_LETTER: i32 = 2824;
const GREETINGS: i32 = 2825;
const OL_MAHUM_MONEY: i32 = 2826;
const KATARIS_LETTER: i32 = 2827;
const NYAKURIS_LETTER: i32 = 2828;
const UNDEAD_LIST: i32 = 2829;
const RAMUSS_LETTER: i32 = 2830;
const RIPPED_DIARY: i32 = 2831;
const HUGE_NAIL: i32 = 2832;
const LETTER_OF_BETRAYER: i32 = 2833;
const BONE_FRAGMENT4: i32 = 2834;
const BONE_FRAGMENT5: i32 = 2835;
const BONE_FRAGMENT6: i32 = 2836;
const BONE_FRAGMENT7: i32 = 2837;
const BONE_FRAGMENT8: i32 = 2838;
const KAKANS_LETTER: i32 = 3037;
const LETTER_GREETINGS1: i32 = 5567;
const LETTER_GREETINGS2: i32 = 5568;

// Rewards
const MARK_OF_REFORMER: i32 = 2821;
const DIMENSIONAL_DIAMOND: i32 = 7562;

// Monsters
const MISERY_SKELETON: i32 = 20022;
const SKELETON_ARCHER: i32 = 20100;
const SKELETON_MARKSMAN: i32 = 20102;
const SKELETON_LORD: i32 = 20104;
const SILENT_HORROR: i32 = 20404;

// Quest monsters
const NAMELESS_REVENANT: i32 = 27099;
const ARURAUNE: i32 = 27128;
const OL_MAHUM_INSPECTOR: i32 = 27129;
const OL_MAHUM_BETRAYER: i32 = 27130;
const CRIMSON_WEREWOLF: i32 = 27131;
const KRUDEL_LIZARDMAN: i32 = 27132;

// Skills
const DISRUPT_UNDEAD: i32 = 1031;
const SLEEP: i32 = 1069;
const VAMPIRIC_TOUCH: i32 = 1147;
const CURSE_WEAKNESS: i32 = 1164;
const CURSE_POISON: i32 = 1168;
const WIND_STRIKE: i32 = 1177;
const ICE_BOLD: i32 = 1184;
const DRYAD_ROOT: i32 = 1201;
const WIND_SHACKLE: i32 = 1206;
const SKILLS: [i32; 9] = [
    DISRUPT_UNDEAD,
    SLEEP,
    VAMPIRIC_TOUCH,
    CURSE_WEAKNESS,
    CURSE_POISON,
    WIND_STRIKE,
    ICE_BOLD,
    DRYAD_ROOT,
    WIND_SHACKLE,
];

// Location
const MOVE_TO: (i32, i32, i32) = (36787, -3709, 10000);

// Misc
const MIN_LEVEL: i32 = 39;

/// Java `Q00227_TestOfTheReformer`.
pub struct Q00227TestOfTheReformer;

impl Script for Q00227TestOfTheReformer {
    fn name(&self) -> &'static str {
        "Q00227_TestOfTheReformer"
    }

    fn path(&self) -> &'static str {
        "quests/Q00227_TestOfTheReformer"
    }

    fn id(&self) -> i32 {
        227
    }

    fn setup(&self, s: &mut ScriptSetup) {
        s.add_start_npc(&[PRIESTESS_PUPINA]);
        s.add_talk_id(&[
            PRIESTESS_PUPINA,
            PREACHER_SLA,
            RAMUS,
            KATARI,
            KAKAN,
            NYAKURI,
            OL_MAHUM_PILGRIM,
        ]);
        s.add_attack_id(&[NAMELESS_REVENANT, CRIMSON_WEREWOLF]);
        s.add_kill_id(&[
            MISERY_SKELETON,
            SKELETON_ARCHER,
            SKELETON_MARKSMAN,
            SKELETON_LORD,
            SILENT_HORROR,
            NAMELESS_REVENANT,
            ARURAUNE,
            OL_MAHUM_INSPECTOR,
            OL_MAHUM_BETRAYER,
            CRIMSON_WEREWOLF,
            KRUDEL_LIZARDMAN,
        ]);
        s.add_spawn_id(&[
            OL_MAHUM_PILGRIM,
            OL_MAHUM_INSPECTOR,
            OL_MAHUM_BETRAYER,
            CRIMSON_WEREWOLF,
            KRUDEL_LIZARDMAN,
        ]);
        s.register_quest_items(&[
            BOOK_OF_REFORM,
            LETTER_OF_INTRODUCTION,
            SLAS_LETTER,
            GREETINGS,
            OL_MAHUM_MONEY,
            KATARIS_LETTER,
            NYAKURIS_LETTER,
            UNDEAD_LIST,
            RAMUSS_LETTER,
            RIPPED_DIARY,
            HUGE_NAIL,
            LETTER_OF_BETRAYER,
            BONE_FRAGMENT4,
            BONE_FRAGMENT5,
            BONE_FRAGMENT6,
            BONE_FRAGMENT7,
            BONE_FRAGMENT8,
            KAKANS_LETTER,
            LETTER_GREETINGS1,
            LETTER_GREETINGS2,
        ]);
    }

    fn on_event(
        &self,
        ctx: &mut ScriptCtx<'_>,
        event: &str,
        npc: Option<ScriptNpc>,
        player: Option<ObjectId>,
    ) -> Option<String> {
        if event == "DESPAWN" {
            // The repeating timer of `on_spawn`: 60 rounds of 5 s, then the NPC is gone.
            let npc = npc?;
            let spawned = ctx
                .world()
                .npc_variables(npc.object_id)
                .map_or(0, |v| v.get_int("SPAWNED", 0));
            if spawned < 60 {
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_int("SPAWNED", spawned + 1);
            } else {
                ctx.delete_npc(npc);
            }
            return None;
        }

        let player = player?;
        let qs = ctx.get_quest_state(player, false)?;

        let mut htmltext = None;
        match event {
            "ACCEPT" => {
                if qs.is_created(ctx) {
                    qs.start_quest(ctx);
                    qs.set_memo_state(ctx, 1);
                    ctx.play_sound(player, QuestSound::ItemsoundQuestMiddle);
                    ctx.give_items(player, BOOK_OF_REFORM, 1);
                    if ctx.player_variable_int(player, "2ND_CLASS_DIAMOND_REWARD", 0) == 0 {
                        ctx.give_items(player, DIMENSIONAL_DIAMOND, 60);
                        ctx.set_player_variable(player, "2ND_CLASS_DIAMOND_REWARD", "1");
                        htmltext = Some("30118-04b.htm".to_owned());
                    } else {
                        htmltext = Some("30118-04.htm".to_owned());
                    }
                }
            }
            "30118-06.html" => {
                if ctx.has_quest_items(player, BOOK_OF_REFORM) {
                    ctx.take_items(player, BOOK_OF_REFORM, 1);
                    ctx.give_items(player, LETTER_OF_INTRODUCTION, 1);
                    ctx.take_items(player, HUGE_NAIL, 1);
                    qs.set_memo_state(ctx, 4);
                    qs.set_cond(ctx, 4, true);
                    htmltext = Some(event.to_owned());
                }
            }
            "30666-02.html" | "30666-03.html" | "30669-02.html" | "30669-05.html" | "30670-02.html" => {
                htmltext = Some(event.to_owned());
            }
            "30666-04.html" => {
                ctx.take_items(player, LETTER_OF_INTRODUCTION, 1);
                ctx.give_items(player, SLAS_LETTER, 1);
                qs.set_memo_state(ctx, 5);
                qs.set_cond(ctx, 5, true);
                htmltext = Some(event.to_owned());
            }
            "30669-03.html" => {
                qs.set_cond(ctx, 12, true);
                let kakan = npc?;
                if ctx.summoned_npc_count(kakan) < 1 {
                    chase_pilgrim(
                        ctx,
                        (-9282, -89975, -2331),
                        CRIMSON_WEREWOLF,
                        (-9382, -89852, -2333),
                    );
                }
                htmltext = Some(event.to_owned());
            }
            "30670-03.html" => {
                qs.set_cond(ctx, 15, true);
                let nyakuri = npc?;
                if ctx.summoned_npc_count(nyakuri) < 1 {
                    chase_pilgrim(
                        ctx,
                        (125947, -180049, -1778),
                        KRUDEL_LIZARDMAN,
                        (126019, -179983, -1781),
                    );
                }
                htmltext = Some(event.to_owned());
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
        skill: Option<SkillKey>,
    ) {
        let Some(qs) = ctx.get_quest_state(attacker, false) else {
            return;
        };
        if !qs.is_started(ctx) {
            return;
        }
        match npc.id {
            NAMELESS_REVENANT => {
                if let Some(skill) = skill {
                    if skill.id == DISRUPT_UNDEAD {
                        ctx.set_script_value(npc, 1);
                    } else {
                        ctx.set_script_value(npc, 2);
                    }
                }
            }
            CRIMSON_WEREWOLF => {
                if skill.is_none_or(|s| !SKILLS.contains(&s.id)) {
                    ctx.broadcast_npc_say(npc, ChatType::NpcGeneral, NpcStringId::CowardlyGuy, &[]);
                    ctx.delete_npc(npc);
                }
                // Java `attacker.isPlayer()` is always true here (the attacker is the player).
                ctx.set_script_value(npc, attacker.0);
            }
            _ => {}
        }
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
            MISERY_SKELETON => bone(ctx, qs, killer, BONE_FRAGMENT7),
            SKELETON_ARCHER => bone(ctx, qs, killer, BONE_FRAGMENT8),
            SKELETON_MARKSMAN => bone(ctx, qs, killer, BONE_FRAGMENT6),
            SKELETON_LORD => bone(ctx, qs, killer, BONE_FRAGMENT5),
            SILENT_HORROR => bone(ctx, qs, killer, BONE_FRAGMENT4),
            NAMELESS_REVENANT => {
                if qs.is_memo_state(ctx, 1)
                    && ctx.is_script_value(npc, 1)
                    && !ctx.has_quest_items(killer, HUGE_NAIL)
                    && ctx.has_quest_items(killer, BOOK_OF_REFORM)
                    && ctx.get_quest_items_count(killer, RIPPED_DIARY) < 7
                {
                    if ctx.get_quest_items_count(killer, RIPPED_DIARY) == 6 {
                        ctx.add_spawn_at(ARURAUNE, npc.object_id, true, 0);
                        ctx.take_items(killer, RIPPED_DIARY, -1);
                        qs.set_cond(ctx, 2, false);
                    } else {
                        ctx.give_items(killer, RIPPED_DIARY, 1);
                        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    }
                }
            }
            ARURAUNE => {
                if !ctx.has_quest_items(killer, HUGE_NAIL) {
                    ctx.broadcast_npc_say(
                        npc,
                        ChatType::NpcGeneral,
                        NpcStringId::TheConcealedTruthWillAlwaysBeRevealed,
                        &[],
                    );
                    ctx.give_items(killer, HUGE_NAIL, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                    qs.set_memo_state(ctx, 3);
                    qs.set_cond(ctx, 3, false);
                }
            }
            OL_MAHUM_INSPECTOR => {
                if qs.is_memo_state(ctx, 6) {
                    qs.set_memo_state(ctx, 7);
                    qs.set_cond(ctx, 7, true);
                }
            }
            OL_MAHUM_BETRAYER => {
                if qs.is_memo_state(ctx, 8) {
                    qs.set_memo_state(ctx, 9);
                    qs.set_cond(ctx, 9, false);
                    ctx.give_items(killer, LETTER_OF_BETRAYER, 1);
                    ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
                }
            }
            CRIMSON_WEREWOLF => {
                if ctx.is_script_value(npc, killer.0) && qs.is_memo_state(ctx, 11) {
                    qs.set_memo_state(ctx, 12);
                    qs.set_cond(ctx, 13, true);
                }
            }
            KRUDEL_LIZARDMAN => {
                if qs.is_memo_state(ctx, 13) {
                    qs.set_memo_state(ctx, 14);
                    qs.set_cond(ctx, 16, true);
                }
            }
            _ => {}
        }
    }

    fn on_talk(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc, player: ObjectId) -> Option<String> {
        let qs = ctx.get_quest_state(player, true)?;
        let memo_state = qs.get_memo_state(ctx);
        let mut htmltext = ctx.get_no_quest_msg(player);
        let state = qs.state(ctx);
        if state == CREATED {
            if npc.id == PRIESTESS_PUPINA {
                let class = ctx.player_class_id(player);
                if class == player_class::CLERIC || class == player_class::SHILLIEN_ORACLE {
                    if ctx.player_level(player) >= MIN_LEVEL {
                        htmltext = "30118-03.htm".to_owned();
                    } else {
                        htmltext = "30118-01.html".to_owned();
                    }
                } else {
                    htmltext = "30118-02.html".to_owned();
                }
            }
        } else if state == STARTED {
            match npc.id {
                PRIESTESS_PUPINA => {
                    if memo_state == 3 {
                        if ctx.has_quest_items(player, HUGE_NAIL) {
                            htmltext = "30118-05.html".to_owned();
                        }
                    } else if (1..3).contains(&memo_state) {
                        htmltext = "30118-04a.html".to_owned();
                    } else if memo_state >= 4 {
                        htmltext = "30118-07.html".to_owned();
                    }
                }
                PREACHER_SLA => {
                    if memo_state == 4 {
                        if ctx.has_quest_items(player, LETTER_OF_INTRODUCTION) {
                            htmltext = "30666-01.html".to_owned();
                        }
                    } else if (11..18).contains(&memo_state) {
                        htmltext = "30666-06b.html".to_owned();
                    } else if memo_state == 5 {
                        if ctx.has_quest_items(player, SLAS_LETTER) {
                            htmltext = "30666-05.html".to_owned();
                        }
                    } else if memo_state == 10 {
                        if ctx.has_quest_items(player, OL_MAHUM_MONEY) {
                            ctx.take_items(player, OL_MAHUM_MONEY, 1);
                            htmltext = "30666-06.html".to_owned();
                        } else {
                            htmltext = "30666-06a.html".to_owned();
                        }
                        ctx.give_items(player, GREETINGS, 1);
                        ctx.give_items(player, LETTER_GREETINGS1, 1);
                        ctx.give_items(player, LETTER_GREETINGS2, 1);
                        qs.set_memo_state(ctx, 11);
                        qs.set_cond(ctx, 11, true);
                    } else if memo_state == 18
                        && ctx.has_all_quest_items(
                            player,
                            &[KATARIS_LETTER, KAKANS_LETTER, NYAKURIS_LETTER, RAMUSS_LETTER],
                        )
                    {
                        ctx.give_adena(player, 226_528, true);
                        ctx.give_items(player, MARK_OF_REFORMER, 1);
                        ctx.add_exp_and_sp(player, 1_252_844, 85_972);
                        qs.exit_quest(ctx, false, true);
                        ctx.send_social_action(player, 3);
                        htmltext = "30666-07.html".to_owned();
                    }
                }
                RAMUS => {
                    if memo_state == 15 {
                        if ctx.has_quest_items(player, LETTER_GREETINGS2)
                            && !ctx.has_quest_items(player, UNDEAD_LIST)
                        {
                            ctx.give_items(player, UNDEAD_LIST, 1);
                            ctx.take_items(player, LETTER_GREETINGS2, 1);
                            qs.set_memo_state(ctx, 16);
                            qs.set_cond(ctx, 18, true);
                            htmltext = "30667-01.html".to_owned();
                        }
                    } else if memo_state == 16 {
                        htmltext = "30667-02.html".to_owned();
                    } else if memo_state == 17 && ctx.has_quest_items(player, UNDEAD_LIST) {
                        ctx.take_items(player, UNDEAD_LIST, 1);
                        ctx.give_items(player, RAMUSS_LETTER, 1);
                        ctx.take_items(player, BONE_FRAGMENT4, 1);
                        ctx.take_items(player, BONE_FRAGMENT5, 1);
                        ctx.take_items(player, BONE_FRAGMENT6, 1);
                        ctx.take_items(player, BONE_FRAGMENT7, 1);
                        ctx.take_items(player, BONE_FRAGMENT8, 1);
                        qs.set_memo_state(ctx, 18);
                        qs.set_cond(ctx, 20, true);
                        htmltext = "30667-03.html".to_owned();
                    }
                }
                KATARI => {
                    if memo_state == 5 || memo_state == 6 {
                        ctx.take_items(player, SLAS_LETTER, 1);
                        qs.set_memo_state(ctx, 6);
                        qs.set_cond(ctx, 6, true);
                        if ctx.summoned_npc_count(npc) < 1 {
                            chase_pilgrim(
                                ctx,
                                (-4015, 40141, -3664),
                                OL_MAHUM_INSPECTOR,
                                (-4034, 40201, -3665),
                            );
                        }
                        htmltext = "30668-01.html".to_owned();
                    } else if memo_state == 7 || memo_state == 8 {
                        if memo_state == 7 {
                            qs.set_memo_state(ctx, 8);
                        }
                        qs.set_cond(ctx, 8, true);
                        if ctx.summoned_npc_count(npc) < 3 {
                            ctx.add_spawn(OL_MAHUM_BETRAYER, -4106, 40174, -3660, 0, false, 0, 0);
                        }
                        htmltext = "30668-02.html".to_owned();
                    } else if memo_state == 9 {
                        if ctx.has_quest_items(player, LETTER_OF_BETRAYER) {
                            ctx.give_items(player, KATARIS_LETTER, 1);
                            ctx.take_items(player, LETTER_OF_BETRAYER, 1);
                            qs.set_memo_state(ctx, 10);
                            qs.set_cond(ctx, 10, true);
                            htmltext = "30668-03.html".to_owned();
                        }
                    } else if memo_state >= 10 {
                        htmltext = "30668-04.html".to_owned();
                    }
                }
                KAKAN => {
                    if memo_state == 11 {
                        if ctx.has_quest_items(player, GREETINGS) {
                            htmltext = "30669-01.html".to_owned();
                        }
                    } else if memo_state == 12
                        && ctx.has_quest_items(player, GREETINGS)
                        && !ctx.has_quest_items(player, KAKANS_LETTER)
                    {
                        ctx.take_items(player, GREETINGS, 1);
                        ctx.give_items(player, KAKANS_LETTER, 1);
                        qs.set_memo_state(ctx, 13);
                        qs.set_cond(ctx, 14, true);
                        htmltext = "30669-04.html".to_owned();
                    }
                }
                NYAKURI => {
                    if memo_state == 13 {
                        if ctx.has_quest_items(player, LETTER_GREETINGS1) {
                            htmltext = "30670-01.html".to_owned();
                        }
                    } else if memo_state == 14
                        && ctx.has_quest_items(player, LETTER_GREETINGS1)
                        && !ctx.has_quest_items(player, NYAKURIS_LETTER)
                    {
                        ctx.give_items(player, NYAKURIS_LETTER, 1);
                        ctx.take_items(player, LETTER_GREETINGS1, 1);
                        qs.set_memo_state(ctx, 15);
                        qs.set_cond(ctx, 17, true);
                        htmltext = "30670-04.html".to_owned();
                    }
                }
                OL_MAHUM_PILGRIM => {
                    if memo_state == 7 {
                        ctx.give_items(player, OL_MAHUM_MONEY, 1);
                        qs.set_memo_state(ctx, 8);
                        htmltext = "30732-01.html".to_owned();
                    }
                }
                _ => {}
            }
        } else if state == COMPLETED && npc.id == PRIESTESS_PUPINA {
            htmltext = ctx.get_already_completed_msg(player);
        }
        Some(htmltext)
    }

    fn on_spawn(&self, ctx: &mut ScriptCtx<'_>, npc: ScriptNpc) {
        match npc.id {
            OL_MAHUM_INSPECTOR | CRIMSON_WEREWOLF | KRUDEL_LIZARDMAN | OL_MAHUM_PILGRIM => {
                ctx.start_quest_timer("DESPAWN", 5000, Some(npc), None, true);
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_int("SPAWNED", 0);
            }
            OL_MAHUM_BETRAYER => {
                ctx.start_quest_timer("DESPAWN", 5000, Some(npc), None, true);
                ctx.set_running(npc);
                ctx.npc_move_to(npc, MOVE_TO.0, MOVE_TO.1, MOVE_TO.2);
                ctx.world_mut()
                    .npc_variables_mut(npc.object_id)
                    .set_int("SPAWNED", 0);
            }
            _ => {}
        }
    }
}

/// The five bone fragment kills: the fragment of this monster once, and memo 17 / cond 19 when
/// the other four are there.
fn bone(ctx: &mut ScriptCtx<'_>, qs: l2_world::Qs, killer: ObjectId, fragment: i32) {
    if qs.is_memo_state(ctx, 16) && !ctx.has_quest_items(killer, fragment) {
        ctx.give_items(killer, fragment, 1);
        ctx.play_sound(killer, QuestSound::ItemsoundQuestItemget);
        let others: Vec<i32> = [
            BONE_FRAGMENT4,
            BONE_FRAGMENT5,
            BONE_FRAGMENT6,
            BONE_FRAGMENT7,
            BONE_FRAGMENT8,
        ]
        .into_iter()
        .filter(|f| *f != fragment)
        .collect();
        if ctx.has_all_quest_items(killer, &others) {
            qs.set_memo_state(ctx, 17);
            qs.set_cond(ctx, 19, false);
        }
    }
}

/// Java `addSpawn(OL_MAHUM_PILGRIM, ..)` and a chaser next to it that gets
/// `addDamageHate(pilgrim, 99999, 99999)` and `setIntentionAttack(pilgrim)`.
/// NOTE(java-quirk): these `addSpawn` calls do not name the NPC as summoner, so
/// `getSummonedNpcCount()` of Kakan, Nyakuri and Katari stays 0 and a pilgrim comes every time.
fn chase_pilgrim(
    ctx: &mut ScriptCtx<'_>,
    pilgrim_at: (i32, i32, i32),
    chaser_id: i32,
    chaser_at: (i32, i32, i32),
) {
    let pilgrim = ctx.add_spawn(
        OL_MAHUM_PILGRIM,
        pilgrim_at.0,
        pilgrim_at.1,
        pilgrim_at.2,
        0,
        false,
        0,
        0,
    );
    let chaser = ctx.add_spawn(chaser_id, chaser_at.0, chaser_at.1, chaser_at.2, 0, false, 0, 0);
    if let (Some(pilgrim), Some(chaser)) = (pilgrim, chaser) {
        ctx.world_mut()
            .add_damage_hate(chaser.object_id, pilgrim.object_id, 99_999, 99_999);
        ctx.set_intention_attack(chaser, pilgrim.object_id);
    }
}

#[cfg(test)]
mod tests {
    use l2_util::ObjectId as Oid;
    use l2_world::ScriptRegistry;
    use l2_world::script::{player_class, race};

    use super::*;
    use crate::testing::{Pages, ScriptTest, names_to, pages_to, rows};

    const NAME: &str = "Q00227_TestOfTheReformer";

    const NPCS: &str = r#"
        <npc id="30118" type="Folk" level="70" name="Priestess Pupina"/>
        <npc id="30666" type="Folk" level="70" name="Preacher Sla"/>
        <npc id="30667" type="Folk" level="70" name="Ramus"/>
        <npc id="30668" type="Folk" level="70" name="Katari"/>
        <npc id="30732" type="Folk" level="70" name="Ol Mahum Pilgrim"/>
        <npc id="27099" type="Monster" level="40" name="Nameless Revenant"/>
        <npc id="27128" type="Monster" level="40" name="Araraune"/>
        <npc id="20022" type="Monster" level="40" name="Misery Skeleton"/>"#;

    const ITEMS: &str = r#"
        <item id="57" type="EtcItem" name="Adena"><set name="is_stackable" val="true"/></item>
        <item id="7562" type="EtcItem" name="Dimensional Diamond"><set name="is_stackable" val="true"/></item>
        <item id="2821" type="EtcItem" name="Mark of Reformer"><set name="is_questitem" val="true"/></item>
        <item id="2822" type="EtcItem" name="Book of Reform"><set name="is_questitem" val="true"/></item>
        <item id="2823" type="EtcItem" name="Letter of Introduction"><set name="is_questitem" val="true"/></item>
        <item id="2824" type="EtcItem" name="Sla's Letter"><set name="is_questitem" val="true"/></item>
        <item id="2825" type="EtcItem" name="Greetings"><set name="is_questitem" val="true"/></item>
        <item id="2826" type="EtcItem" name="Ol Mahum Money"><set name="is_questitem" val="true"/></item>
        <item id="2827" type="EtcItem" name="Katari's Letter"><set name="is_questitem" val="true"/></item>
        <item id="2828" type="EtcItem" name="Nyakuri's Letter"><set name="is_questitem" val="true"/></item>
        <item id="2829" type="EtcItem" name="Undead List"><set name="is_questitem" val="true"/></item>
        <item id="2830" type="EtcItem" name="Ramus's Letter"><set name="is_questitem" val="true"/></item>
        <item id="2831" type="EtcItem" name="Ripped Diary"><set name="is_questitem" val="true"/><set name="is_stackable" val="true"/></item>
        <item id="2832" type="EtcItem" name="Huge Nail"><set name="is_questitem" val="true"/></item>
        <item id="2833" type="EtcItem" name="Letter of Betrayer"><set name="is_questitem" val="true"/></item>
        <item id="2834" type="EtcItem" name="Bone Fragment 4"><set name="is_questitem" val="true"/></item>
        <item id="2835" type="EtcItem" name="Bone Fragment 5"><set name="is_questitem" val="true"/></item>
        <item id="2836" type="EtcItem" name="Bone Fragment 6"><set name="is_questitem" val="true"/></item>
        <item id="2837" type="EtcItem" name="Bone Fragment 7"><set name="is_questitem" val="true"/></item>
        <item id="2838" type="EtcItem" name="Bone Fragment 8"><set name="is_questitem" val="true"/></item>
        <item id="3037" type="EtcItem" name="Kakan's Letter"><set name="is_questitem" val="true"/></item>
        <item id="5567" type="EtcItem" name="Letter Greetings 1"><set name="is_questitem" val="true"/></item>
        <item id="5568" type="EtcItem" name="Letter Greetings 2"><set name="is_questitem" val="true"/></item>"#;

    /// The pages of the quest with the links of the real pages.
    fn pages() -> Pages {
        Pages::of(&Q00227TestOfTheReformer)
            .plain(&[
                "30118-01.html",
                "30118-02.html",
                "30118-04.htm",
                "30118-04a.html",
                "30118-04b.htm",
                "30118-07.html",
            ])
            .page("30118-03.htm", &["ACCEPT"])
            .page("30118-05.html", &["30118-06.html"])
            .plain(&["30118-06.html"])
            .page("30666-01.html", &["30666-02.html", "30666-03.html"])
            .page("30666-03.html", &["30666-04.html", "30666-02.html"])
            .plain(&[
                "30666-02.html",
                "30666-04.html",
                "30666-05.html",
                "30666-06.html",
                "30666-06a.html",
                "30666-06b.html",
                "30666-07.html",
            ])
            .plain(&["30667-01.html", "30667-02.html", "30667-03.html"])
            .plain(&["30668-01.html", "30668-02.html", "30668-03.html", "30668-04.html"])
            .plain(&["30732-01.html"])
            .npcs(&[PRIESTESS_PUPINA, PREACHER_SLA, RAMUS, KATARI, OL_MAHUM_PILGRIM])
    }

    struct Npcs {
        pupina: Oid,
        sla: Oid,
        ramus: Oid,
    }

    fn setup() -> (ScriptTest, Npcs) {
        let mut registry = ScriptRegistry::new();
        registry.add(Q00227TestOfTheReformer);
        let mut t = ScriptTest::with_pages(registry, NPCS, ITEMS, pages());
        let npcs = Npcs {
            pupina: t.spawn(PRIESTESS_PUPINA, 20),
            sla: t.spawn(PREACHER_SLA, 30),
            ramus: t.spawn(RAMUS, 40),
        };
        (t, npcs)
    }

    /// Talk to Pupina and accept (the world takes an event only from the page that offers it).
    fn start(t: &mut ScriptTest, pupina: Oid) {
        t.quest_page(1, pupina);
        t.event(1, NAME, "ACCEPT");
    }

    fn set_memo(t: &mut ScriptTest, memo: i32) {
        t.world.run_as_script(NAME, |ctx| {
            if let Some(qs) = ctx.get_quest_state(Oid(1), false) {
                qs.set_memo_state(ctx, memo);
            }
        });
    }

    #[test]
    fn the_start_is_gated_by_class_and_level() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 40, race::HUMAN, player_class::FIGHTER, vec![]);
        assert!(t.quest_page(1, npcs.pupina).contains("30118-02.html"));
        t.enter_as(2, 38, race::HUMAN, player_class::CLERIC, vec![]);
        assert!(t.quest_page(2, npcs.pupina).contains("30118-01.html"));
        assert!(t.take_writes().is_empty());
    }

    #[test]
    fn the_accept_gives_the_book_and_the_diamonds_once() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::CLERIC, vec![]);
        assert!(t.quest_page(1, npcs.pupina).contains("30118-03.htm"));
        assert!(t.event(1, NAME, "ACCEPT").contains("30118-04b.htm"));
        assert_eq!(t.count(1, BOOK_OF_REFORM), 1);
        assert_eq!(t.count(1, DIMENSIONAL_DIAMOND), 60);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::insert(NAME, "cond", "1")));
        assert!(writes.contains(&rows::insert(NAME, "memoState", "1")));
    }

    #[test]
    fn the_diary_pages_call_araraune_and_the_nail_goes_to_pupina() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::SHILLIEN_ORACLE, vec![]);
        start(&mut t, npcs.pupina);
        t.take_writes();

        // Six diaries, then the seventh kill calls Araraune.
        for round in 0..7 {
            let rev = t.spawn(NAMELESS_REVENANT, 50);
            t.world.run_as_script(NAME, |ctx| {
                ctx.set_script_value(
                    ScriptNpc {
                        object_id: rev,
                        id: NAMELESS_REVENANT,
                    },
                    1,
                );
            });
            t.kill(1, rev);
            t.world.delete_npc(rev);
            if round < 6 {
                assert_eq!(t.count(1, RIPPED_DIARY), round + 1);
            }
        }
        assert_eq!(t.count(1, RIPPED_DIARY), 0);
        let ara = t.npcs_of(ARURAUNE);
        assert_eq!(ara.len(), 1);
        t.kill(1, ara[0]);
        t.world.delete_npc(ara[0]);
        assert_eq!(t.count(1, HUGE_NAIL), 1);
        assert!(t.take_writes().contains(&rows::update(NAME, "memoState", "3")));

        assert!(t.quest_page(1, npcs.pupina).contains("30118-05.html"));
        assert!(t.event(1, NAME, "30118-06.html").contains("30118-06.html"));
        assert_eq!(t.count(1, LETTER_OF_INTRODUCTION), 1);
        assert_eq!(t.count(1, HUGE_NAIL), 0);
        assert!(t.take_writes().contains(&rows::update(NAME, "memoState", "4")));
    }

    #[test]
    fn the_reward_comes_with_the_four_letters() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::CLERIC, vec![]);
        start(&mut t, npcs.pupina);
        set_memo(&mut t, 18);
        t.world.run_as_script(NAME, |ctx| {
            for item in [KATARIS_LETTER, KAKANS_LETTER, NYAKURIS_LETTER, RAMUSS_LETTER] {
                ctx.give_items(Oid(1), item, 1);
            }
        });
        t.take_writes();
        t.take();

        t.talk(1, npcs.sla);
        t.bypass(1, "Script");
        let out = t.take();
        assert!(pages_to(&out, 1).last().unwrap().contains("30666-07.html"));
        assert!(names_to(&out, 1).contains(&"SOCIAL_ACTION".to_owned()));
        assert_eq!(t.count(1, MARK_OF_REFORMER), 1);
        assert_eq!(t.count(1, 57), 226_528);
        assert_eq!(
            t.take_writes(),
            [
                rows::delete(NAME, false),
                rows::update(NAME, "<state>", "Completed"),
            ]
        );
        assert!(
            t.quest_page(1, npcs.pupina)
                .contains("This quest has already been completed.")
        );
    }

    #[test]
    fn the_last_bone_fragment_moves_to_ramus() {
        let (mut t, npcs) = setup();
        t.enter_as(1, 39, race::HUMAN, player_class::CLERIC, vec![]);
        start(&mut t, npcs.pupina);
        set_memo(&mut t, 16);
        t.world.run_as_script(NAME, |ctx| {
            ctx.give_items(Oid(1), UNDEAD_LIST, 1);
            for item in [BONE_FRAGMENT4, BONE_FRAGMENT5, BONE_FRAGMENT6, BONE_FRAGMENT8] {
                ctx.give_items(Oid(1), item, 1);
            }
        });
        t.take_writes();
        let skel = t.spawn(MISERY_SKELETON, 50);
        t.kill(1, skel);
        t.world.delete_npc(skel);
        assert_eq!(t.count(1, BONE_FRAGMENT7), 1);
        let writes = t.take_writes();
        assert!(writes.contains(&rows::update(NAME, "memoState", "17")));
        assert!(writes.contains(&rows::update(NAME, "cond", "19")));

        assert!(t.quest_page(1, npcs.ramus).contains("30667-03.html"));
        assert_eq!(t.count(1, RAMUSS_LETTER), 1);
        assert_eq!(t.count(1, BONE_FRAGMENT7), 0);
    }
}
